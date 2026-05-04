//! V1.14 Scope I — HSM-backed witness byte-equivalence golden.
//!
//! Question this integration test answers (and only this one):
//!
//! > For the same 32-byte Ed25519 seed, does an Ed25519 *imported* into
//! > SoftHSM2 produce a `WitnessSig` that verifies against the same
//! > pubkey as the file-backed `Ed25519Witness`?
//!
//! If **YES**: V1.14 Scope I (HSM-backed witness) ships with the same
//! roster contract as V1.13 — operators commission a witness key in
//! the HSM, paste the pubkey into `ATLAS_WITNESS_V1_ROSTER`, and the
//! verifier accepts both file-backed and HSM-backed signatures
//! interchangeably under the same `witness_kid`.
//!
//! If **NO**: the HSM produces a non-RFC-8032 signature shape, which
//! breaks the roster contract. Investigation required before Scope I
//! can ship.
//!
//! ## Why a separate `tests/` integration test
//!
//! Lives in `tests/` (one binary per file) so it runs in its own
//! process. PKCS#11's `C_Initialize` is global per process and
//! SoftHSM2 occasionally objects to two simultaneous initialisers
//! within the same address space — keeping this test outside the
//! `cargo test` unit-test pool avoids cross-test interference. Mirrors
//! the V1.11 Scope A pre-flight pattern in
//! `crates/atlas-signer/tests/hsm_byte_equivalence.rs`.
//!
//! ## Gates
//!
//! * **Compile gate:** `--features hsm`. Without it, the file expands
//!   to a no-op stub so `cargo test` in default-features CI is green.
//! * **Runtime gate:** `ATLAS_TEST_HSM_WITNESS_BYTE_EQUIV=1` AND the
//!   witness HSM trio (`ATLAS_WITNESS_HSM_PKCS11_LIB`,
//!   `ATLAS_WITNESS_HSM_SLOT`, `ATLAS_WITNESS_HSM_PIN_FILE`). Either
//!   missing → `eprintln!("SKIP …")` and the test passes. Distinct
//!   from the wave-3 sealed-signer gate because (a) this test mutates
//!   the token (creates and destroys two key objects) and (b) the
//!   witness binary uses its own env-var prefix for trust-domain
//!   isolation from atlas-signer.
//!
//! ## Why import, not generate
//!
//! `CKM_EC_EDWARDS_KEY_PAIR_GEN` produces a *fresh* key with no
//! relationship to any seed — useful for the production-flow
//! commissioning ceremony but tells us nothing about the
//! verifier-roster contract. The byte-equivalence question is
//! specifically: "if both the file-backed witness and the HSM-backed
//! witness consume the same 32-byte seed, do they produce signatures
//! that verify under the same roster-pinned pubkey?" That requires
//! `create_object` with a known `CKA_VALUE` — i.e. *import*, not
//! generation. SoftHSM2 (OpenSSL-backed) is RFC 8032 §5.1.5 conformant
//! and is expected to produce a deterministic, byte-identical
//! signature. Commercial HSMs may refuse the import path entirely
//! (fresh-only by policy); for those, the production ceremony is
//! `pkcs11-tool --keypairgen` followed by `Pkcs11Witness::open`,
//! which this test does NOT exercise.

#![cfg(feature = "hsm")]

use std::env;
use std::path::PathBuf;

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::object::{Attribute, KeyType, ObjectClass};
use cryptoki::session::UserType;
use cryptoki::types::AuthPin;
use ed25519_dalek::{Signer, SigningKey};
use zeroize::Zeroizing;

use atlas_trust_core::{decode_chain_head, verify_witness_against_roster, witness_signing_input};
use atlas_witness::hsm::config::{
    HsmWitnessConfig, PIN_FILE_ENV, PKCS11_LIB_ENV, SLOT_ENV, WITNESS_LABEL_PREFIX,
};
use atlas_witness::hsm::Pkcs11Witness;
use atlas_witness::{Ed25519Witness, Witness};

/// Runtime opt-in. Distinct from the production trio because this test
/// mutates the token (creates and destroys an Ed25519 keypair); the
/// trio alone is the production-binary smoke, this env var is the
/// additional "yes, exercise the import path on this slot" handshake.
const RUNTIME_GATE_ENV: &str = "ATLAS_TEST_HSM_WITNESS_BYTE_EQUIV";

/// Witness kid used in the byte-equivalence pin. Arbitrary stable string
/// — the test asserts the file-backed and HSM-backed witnesses agree on
/// the signature shape under the *same* kid, so a future rename here
/// only affects the test's local roster construction, never the wire
/// format.
const TEST_WITNESS_KID: &str = "byte-equiv-test-witness";

/// Fixed seed both witnesses consume. `[42u8; 32]` mirrors the
/// `Ed25519Witness::round_trip_against_roster` unit test in
/// `crates/atlas-witness/src/ed25519_witness.rs` so any future seed
/// change there cascades naturally; bytes are arbitrary beyond
/// "deterministic, well-known".
const TEST_SEED: [u8; 32] = [42u8; 32];

/// ASN.1 DER for the printable string `"edwards25519"` — the
/// `CKA_EC_PARAMS` value SoftHSM2/OpenSSL accepts for Ed25519.
/// Mirrors `crates/atlas-signer/tests/hsm_byte_equivalence.rs`.
const ED25519_PARAMS_PRINTABLE: [u8; 14] = [
    0x13, 0x0c, b'e', b'd', b'w', b'a', b'r', b'd', b's', b'2', b'5', b'5', b'1', b'9',
];

/// Primary assertion: both witnesses produce signatures that the
/// verifier accepts against the SAME pubkey under the same kid.
///
/// Steps:
///   1. Compute the file-backed reference: `Ed25519Witness::new(kid, seed)`
///      → `sign_chain_head(head)` → `WitnessSig`.
///   2. Compute the file-backed pubkey from the seed.
///   3. Import the same seed into SoftHSM2 under
///      `WITNESS_LABEL_PREFIX || kid` so `Pkcs11Witness::open` resolves
///      it via `find_objects`.
///   4. Open `Pkcs11Witness` against the same slot/PIN trio and call
///      `sign_chain_head(head)` → `WitnessSig`.
///   5. Build a roster `[(kid, file_backed_pubkey)]` and verify BOTH
///      signatures against it via `verify_witness_against_roster`.
///   6. Both must verify; both signatures should also be byte-identical
///      (RFC 8032 deterministic).
///   7. Cleanup: destroy both keypair halves on every exit path
///      (RAII guard).
#[test]
fn hsm_and_file_backed_witnesses_produce_roster_compatible_sigs() {
    let Some(env) = require_runtime_gate() else {
        return;
    };

    let chain_head_hex = compute_chain_head();
    let _chain_head_bytes = decode_chain_head(&chain_head_hex)
        .expect("test chain head must round-trip through the strict decoder");

    // Step 1+2: file-backed reference. The seed is wrapped in
    // Zeroizing inside Ed25519Witness; the surrounding test body holds
    // a plain `[u8; 32]` only long enough to feed the constructor,
    // mirroring the operator-side ergonomics.
    let file_witness = Ed25519Witness::new(TEST_WITNESS_KID.to_string(), TEST_SEED);
    let file_pubkey = SigningKey::from_bytes(&TEST_SEED).verifying_key().to_bytes();
    let file_sig = file_witness
        .sign_chain_head(&chain_head_hex)
        .expect("file-backed witness must sign cleanly");

    // The label used both to import the keypair (Phase A) and to
    // resolve it from inside `Pkcs11Witness::open` (Phase B). Built
    // once here and threaded through both phases plus the cleanup
    // guard so a future label-shape change has a single touch site.
    let label = format!("{WITNESS_LABEL_PREFIX}{TEST_WITNESS_KID}");

    // ----- Phase A: import the seed into SoftHSM2 under Token=true -----
    //
    // Using a *direct cryptoki session* here (not Pkcs11Witness) so
    // the test's setup phase is independent of the code under test —
    // a bug in `Pkcs11Witness::open` cannot mask a bug in the import
    // path. The whole phase lives in a nested scope so the
    // `Pkcs11` context drops (and `C_Finalize` runs) before
    // `Pkcs11Witness::open` is called: PKCS#11 §5.4 mandates that
    // every `C_Initialize` is paired with a `C_Finalize` *before*
    // the next `C_Initialize` in the same process. Without the
    // explicit phase boundary, two contexts would overlap and
    // SoftHSM2 returns `CKR_CRYPTOKI_ALREADY_INITIALIZED` (or worse,
    // produces UB on commercial modules).
    {
        let pkcs11 = Pkcs11::new(&env.module_path).expect("PKCS#11 module load");
        pkcs11
            .initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
            .expect("PKCS#11 C_Initialize");
        let slot = pkcs11
            .get_slots_with_token()
            .expect("get_slots_with_token")
            .into_iter()
            .find(|s| s.id() == env.slot)
            .unwrap_or_else(|| {
                panic!(
                    "configured slot {} not present in module {}",
                    env.slot,
                    env.module_path.display(),
                )
            });
        let session = pkcs11.open_rw_session(slot).expect("open_rw_session");
        let pin = read_pin_zeroized(&env.pin_file);
        session
            .login(UserType::User, Some(&pin))
            .expect("PKCS#11 C_Login");

        // `Token=true` so the imported keypair survives the session
        // close at the end of this nested block — `Pkcs11Witness::open`
        // (Phase B) will resolve it from a fresh session via
        // `find_objects` on `(Class, KeyType, Label)`. Cleanup happens
        // via the `TokenKeyCleanup` declared *after* this scope ends.
        import_seed_into_hsm(&session, &TEST_SEED, &label, file_pubkey);
        // session + pkcs11 drop here → C_CloseSession + C_Finalize.
    }

    // RAII cleanup that destroys the imported keypair on every exit
    // path of the test (success, panic, early return). Holds OWNED
    // data, not a session borrow — its `Drop` opens a fresh PKCS#11
    // context, finds the keypair by label, destroys both halves, and
    // finalises. Declared BEFORE `hsm_witness` so LIFO drop order
    // runs `hsm_witness::Drop` (releases its own ctx) FIRST, then
    // this guard's `Drop` (which can then claim a fresh ctx without
    // overlap).
    let _cleanup = TokenKeyCleanup {
        module_path: env.module_path.clone(),
        slot: env.slot,
        pin_file: env.pin_file.clone(),
        label: label.clone(),
    };

    // Step 4: code under test. `Pkcs11Witness::open` re-opens its own
    // PKCS#11 context against the same module + slot trio, finds the
    // imported key by label, and signs the chain head. The signature
    // path goes through `CKM_EDDSA(Ed25519)` inside the token.
    let cfg = HsmWitnessConfig::from_env_for_test(
        env.module_path.clone(),
        env.slot,
        env.pin_file.clone(),
    );
    let hsm_witness =
        Pkcs11Witness::open(cfg, TEST_WITNESS_KID.to_string()).expect("Pkcs11Witness::open");
    let hsm_sig = hsm_witness
        .sign_chain_head(&chain_head_hex)
        .expect("HSM-backed witness must sign cleanly");

    // Step 5: roster verification. Both signatures must verify under
    // the SAME (kid, pubkey) entry in the roster — that's the load-
    // bearing contract: a verifier built from V1.13 trust-core source
    // accepts witnesses signed by either backend interchangeably. If
    // the HSM produces a non-roster-compatible signature, this is
    // where it surfaces.
    let roster: &[(&str, [u8; 32])] = &[(TEST_WITNESS_KID, file_pubkey)];
    verify_witness_against_roster(&file_sig, &chain_head_hex, roster)
        .expect("file-backed sig must verify against the file-backed pubkey roster");
    verify_witness_against_roster(&hsm_sig, &chain_head_hex, roster).expect(
        "BYTE-EQUIVALENCE FAIL: HSM-backed witness produced a sig that does NOT verify \
         against the file-backed pubkey for the same seed. Implication: V1.14 Scope I \
         cannot ship the unified roster contract — operators would need a separate \
         pubkey paste-step for HSM-backed witnesses, defeating the V1.13 commissioning \
         ceremony's single-source-of-truth roster.",
    );

    // Step 6: signature byte-equality. Ed25519 is RFC 8032 §5.1.6
    // deterministic; same key + same message must yield byte-identical
    // signatures. Verifying both is the strongest single-test
    // observable that the HSM is producing genuine RFC 8032 sigs and
    // not, say, valid-but-randomised sigs (which would itself be a
    // sidechannel — an attacker observing two sigs over the same head
    // can leak per-call randomness). Same panic shape as Step 5 so the
    // CI log carries the remediation.
    assert_eq!(
        file_sig.signature, hsm_sig.signature,
        "DETERMINISM FAIL: file-backed and HSM-backed witnesses produced different \
         signatures for the same key + same chain head. Ed25519 is deterministic per \
         RFC 8032 §5.1.6; divergence here means the HSM is using non-deterministic \
         nonces, which (a) breaks witness reproducibility across attestor instances \
         and (b) is a sidechannel surface. Investigate the SoftHSM2 build (some \
         patched OpenSSL builds inject a random nonce). file_sig: {} hsm_sig: {}",
        file_sig.signature, hsm_sig.signature,
    );

    // Cross-check: independently confirm the HSM signature decodes via
    // the same signing-input shape `witness_signing_input` produces. If
    // both sigs verify above but the signing input went through
    // different encoders (e.g. Pkcs11Witness accidentally signed
    // `chain_head_bytes` directly without the `ATLAS_WITNESS_DOMAIN`
    // prefix, and somehow still happened to produce a sig that the
    // verifier accepts under a degenerate code path), we would catch
    // it here by re-deriving the expected signing input on this side
    // and verifying via dalek directly.
    let chain_head_bytes = decode_chain_head(&chain_head_hex).expect("strict decode");
    let signing_input = witness_signing_input(&chain_head_bytes);
    let dalek_sig = SigningKey::from_bytes(&TEST_SEED).sign(&signing_input);
    let hsm_sig_bytes = base64_url_decode_no_pad(&hsm_sig.signature);
    let hsm_sig_array: [u8; 64] = hsm_sig_bytes
        .as_slice()
        .try_into()
        .expect("HSM witness signature must be 64 bytes (RFC 8032 §5.1.6)");
    assert_eq!(
        hsm_sig_array,
        dalek_sig.to_bytes(),
        "CROSS-CHECK FAIL: HSM-backed signature differs from a fresh dalek signature \
         over the same signing input. Either (a) Pkcs11Witness signs a different \
         input than `witness_signing_input(chain_head_bytes)`, or (b) the HSM \
         encoded the sig with extra framing. Both are V1.14 Scope I bugs.",
    );

    eprintln!(
        "V1.14 Scope I PRE-FLIGHT PASS — SoftHSM2 produces roster-compatible witness \
         sigs for kid {TEST_WITNESS_KID:?} under label {label:?}. Scope I may ship \
         with the unified V1.13 roster contract."
    );
}

/// Resolved runtime configuration for the test. Returns `None` when
/// the runtime gate is not set or the witness HSM trio is missing.
struct RuntimeEnv {
    module_path: PathBuf,
    slot: u64,
    pin_file: PathBuf,
}

fn require_runtime_gate() -> Option<RuntimeEnv> {
    if !env::var(RUNTIME_GATE_ENV)
        .map(|v| v.trim() == "1")
        .unwrap_or(false)
    {
        eprintln!(
            "SKIP: {RUNTIME_GATE_ENV} is not set to 1 — V1.14 Scope I pre-flight does \
             not run on dev machines without an explicit opt-in (the test creates and \
             destroys objects on the configured SoftHSM2 token; the gate is the \
             operator's handshake that this is the right token to mutate)."
        );
        return None;
    }
    let module_path = env::var(PKCS11_LIB_ENV).ok().map(PathBuf::from);
    let slot = env::var(SLOT_ENV).ok().and_then(|s| s.parse::<u64>().ok());
    let pin_file = env::var(PIN_FILE_ENV).ok().map(PathBuf::from);
    match (module_path, slot, pin_file) {
        (Some(m), Some(s), Some(p)) => {
            // Mirror `HsmWitnessConfig::from_env`'s absolute-path guard:
            // refuse relative paths to defend against CWD-swap library
            // / PIN file substitution (footgun-class identical to
            // atlas-signer; see `hsm_byte_equivalence.rs` for the long
            // form rationale).
            if !m.is_absolute() {
                eprintln!(
                    "SKIP: {PKCS11_LIB_ENV}={} is not absolute — V1.14 \
                     pre-flight refuses relative paths for parity with \
                     HsmWitnessConfig::from_env (defends against CWD-swap library \
                     hijack).",
                    m.display()
                );
                return None;
            }
            if !p.is_absolute() {
                eprintln!(
                    "SKIP: {PIN_FILE_ENV}={} is not absolute — V1.14 \
                     pre-flight refuses relative paths for parity with \
                     HsmWitnessConfig::from_env (defends against CWD-swap PIN file \
                     substitution).",
                    p.display()
                );
                return None;
            }
            Some(RuntimeEnv {
                module_path: m,
                slot: s,
                pin_file: p,
            })
        }
        _ => {
            eprintln!(
                "SKIP: witness HSM trio ({PKCS11_LIB_ENV}, {SLOT_ENV}, {PIN_FILE_ENV}) \
                 not fully present even though {RUNTIME_GATE_ENV}=1. Either set the \
                 trio or unset the gate; partial config is the same footgun the \
                 production binary refuses."
            );
            None
        }
    }
}

/// Build the test chain head. Const-context cannot call `repeat`, so
/// the actual string is constructed once at runtime.
fn compute_chain_head() -> String {
    "abcd".repeat(16)
}

/// Read PIN file into a Zeroizing buffer + AuthPin. Mirrors the
/// production `read_pin_file_for_witness` in `src/hsm/pkcs11.rs` (sans
/// Unix permission guard — runbook owns that contract for the prod
/// binary; tests should not refuse a CI-provisioned PIN file just
/// because GitHub Actions umask differs).
fn read_pin_zeroized(path: &std::path::Path) -> AuthPin {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .unwrap_or_else(|e| panic!("PIN file {} unreadable: {e}", path.display()));
    let meta = file
        .metadata()
        .unwrap_or_else(|e| panic!("PIN file {} metadata unreadable: {e}", path.display()));
    let len_hint = (meta.len() as usize).max(1);
    let mut bytes: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::with_capacity(len_hint));
    file.read_to_end(&mut bytes)
        .unwrap_or_else(|e| panic!("PIN file {} read failed: {e}", path.display()));
    let trimmed = std::str::from_utf8(&bytes)
        .unwrap_or_else(|_| panic!("PIN file {} is not UTF-8", path.display()))
        .trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
    assert!(
        !trimmed.is_empty(),
        "PIN file {} is empty after trim",
        path.display()
    );
    // V1.14 Scope I (security-review HIGH-1): mirror the production
    // `read_pin_file_for_witness` zeroize discipline so the test
    // path doesn't accidentally validate weaker PIN-handling than
    // prod. Build the trimmed PIN inside a Zeroizing<String>;
    // `mem::take` moves the inner String into AuthPin, which wraps
    // it in SecretString that zeroizes on drop.
    let mut pin_buf: Zeroizing<String> = Zeroizing::new(trimmed.to_owned());
    AuthPin::from(std::mem::take(&mut *pin_buf))
}

/// Import a 32-byte Ed25519 seed into the HSM as a private+public
/// keypair under the supplied label. Mirrors the atlas-signer
/// byte-equivalence pattern: `CKA_VALUE` carries the raw seed bytes;
/// the public-key object's `CKA_EC_POINT` carries the host-derived
/// pubkey wrapped in an ASN.1 OCTET STRING.
///
/// Returns nothing — the test resolves the keypair from a fresh
/// session via label-based `find_objects` (Phase B's
/// `Pkcs11Witness::open` and Phase C's `TokenKeyCleanup`), so the
/// import-time `ObjectHandle` values would dangle on the next phase
/// boundary anyway.
fn import_seed_into_hsm(
    session: &cryptoki::session::Session,
    seed: &[u8; 32],
    label: &str,
    host_pubkey: [u8; 32],
) {
    // V1.10 footgun #17 parity: source-of-truth seed bytes scrub on
    // function exit; the unscrubbed FFI clone inside the template
    // lives only for one `create_object` syscall window.
    let seed_zeroized: Zeroizing<Vec<u8>> = Zeroizing::new(seed.to_vec());
    {
        let priv_template = vec![
            Attribute::Class(ObjectClass::PRIVATE_KEY),
            Attribute::KeyType(KeyType::EC_EDWARDS),
            Attribute::EcParams(ED25519_PARAMS_PRINTABLE.to_vec()),
            Attribute::Value((*seed_zeroized).clone()),
            Attribute::Label(label.as_bytes().to_vec()),
            // Token=true so `Pkcs11Witness::open` (which opens its own
            // session in Phase B) can find the imported key from a
            // fresh session via `find_objects` on `(Class, Label)`.
            // Cleanup happens via the `TokenKeyCleanup` guard whose
            // Drop reconnects in Phase C; a panic mid-test that
            // bypasses that guard would orphan the key on the token,
            // but the smoke-lane token is throwaway and the next run's
            // `find_one_private_key` ambiguity check catches the
            // orphan loudly.
            Attribute::Token(true),
            Attribute::Sign(true),
            Attribute::Sensitive(true),
            Attribute::Extractable(false),
            // V1.14 Scope I — pin CKA_DERIVE=false so the imported
            // smoke-test key matches the runbook §11 commissioning
            // contract bit-for-bit (private template: Sensitive=true,
            // Extractable=false, Derive=false). A future review-side
            // tightening that adds a CKA_DERIVE readback in
            // `find_one_private_key` must see this attribute when it
            // resolves the imported key, otherwise the byte-equivalence
            // lane would diverge from prod by accepting derive-capable
            // keys in CI but refusing them at runtime.
            Attribute::Derive(false),
            Attribute::Id(label.as_bytes().to_vec()),
        ];
        session
            .create_object(&priv_template)
            .expect("import Ed25519 private key (CKA_VALUE = 32-byte seed)");
    }

    let pub_template = vec![
        Attribute::Class(ObjectClass::PUBLIC_KEY),
        Attribute::KeyType(KeyType::EC_EDWARDS),
        Attribute::EcParams(ED25519_PARAMS_PRINTABLE.to_vec()),
        // OCTET STRING wrapper per PKCS#11 v3.0 §10.10. SoftHSM2
        // accepts the raw form too; emitting wrapped matches what
        // `get_attributes` returns on read-back.
        Attribute::EcPoint(wrap_octet_string(&host_pubkey)),
        Attribute::Label(label.as_bytes().to_vec()),
        Attribute::Token(true),
        Attribute::Verify(true),
        Attribute::Id(label.as_bytes().to_vec()),
    ];
    session
        .create_object(&pub_template)
        .expect("import Ed25519 public key (CKA_EC_POINT = host-derived pubkey)");
}

/// Wrap a 32-byte Ed25519 pubkey in ASN.1 DER OCTET STRING.
fn wrap_octet_string(raw: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + raw.len());
    out.push(0x04);
    out.push(raw.len() as u8);
    out.extend_from_slice(raw);
    out
}

/// Decode URL-safe base64 (no padding) — same dialect as
/// `Ed25519Witness::sign_chain_head` and the verifier's
/// `verify_witness_against_roster` decoder.
fn base64_url_decode_no_pad(s: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .expect("WitnessSig.signature must be URL-safe base64 (no padding)")
}

/// RAII cleanup that destroys the imported keypair on every test exit
/// path. Holds OWNED data (no session borrow) so it can outlive the
/// import-time `Pkcs11` context: PKCS#11 forbids two simultaneous
/// `C_Initialize` calls in the same process, so the cleanup must run
/// in its own freshly-opened context AFTER `Pkcs11Witness::open`'s
/// internal context has been finalised.
///
/// Differs structurally from `atlas-signer`'s `ImportGuard` (which
/// borrows the session) because that test uses `Token=false` keys
/// that die with the session anyway — here we use `Token=true` so
/// `Pkcs11Witness::open` can find the keypair from a separate
/// session, and cleanup must therefore reconnect.
///
/// Best-effort: every cryptoki call uses `let _ = …`. A failure to
/// destroy (e.g. token rotated under us, network HSM dropped the
/// connection) leaves dangling objects on the throwaway smoke-lane
/// token; that's the worst-case ceiling and the next test run's
/// `find_objects` returns the orphans, which the duplicate-check in
/// `find_one_private_key` catches loudly. So a leaked object turns
/// into a loud "ambiguous" failure on next run, not a silent
/// regression.
struct TokenKeyCleanup {
    module_path: PathBuf,
    slot: u64,
    pin_file: PathBuf,
    label: String,
}

impl Drop for TokenKeyCleanup {
    fn drop(&mut self) {
        let _ = (|| -> Result<(), String> {
            let pkcs11 = Pkcs11::new(&self.module_path).map_err(|e| e.to_string())?;
            pkcs11
                .initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
                .map_err(|e| e.to_string())?;
            let slot = pkcs11
                .get_slots_with_token()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|s| s.id() == self.slot)
                .ok_or_else(|| format!("slot {} missing during cleanup", self.slot))?;
            let session = pkcs11
                .open_rw_session(slot)
                .map_err(|e| e.to_string())?;
            let pin = read_pin_zeroized(&self.pin_file);
            session
                .login(UserType::User, Some(&pin))
                .map_err(|e| e.to_string())?;

            // Destroy any object (private + public) that carries our
            // label. Filter by `(Class, Label)` rather than just
            // `Label` so we don't accidentally walk over a future
            // unrelated object that happens to share the label
            // namespace; the trade-off is two find_objects calls
            // instead of one, which is fine for cleanup latency.
            for class in [ObjectClass::PRIVATE_KEY, ObjectClass::PUBLIC_KEY] {
                let template = vec![
                    Attribute::Class(class),
                    Attribute::Label(self.label.as_bytes().to_vec()),
                ];
                let handles = session.find_objects(&template).map_err(|e| e.to_string())?;
                for h in handles {
                    let _ = session.destroy_object(h);
                }
            }
            Ok(())
        })();
    }
}
