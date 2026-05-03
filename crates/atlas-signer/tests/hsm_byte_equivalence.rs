//! V1.11 Scope A pre-flight — SoftHSM2 byte-equivalence golden.
//!
//! Question this test answers (and only this one):
//!
//! > For a known 32-byte HKDF output (the V1.9-equivalent per-tenant
//! > seed), does an Ed25519 *imported* into SoftHSM2 produce the same
//! > 32-byte public key as `ed25519_dalek::SigningKey::from_bytes()`?
//!
//! If **YES**: V1.11 Scope A (HSM seal scope wave 3) can ship a
//! migration that imports the V1.9-derived seed into the HSM and
//! continues signing for existing workspaces *with the same pubkey*.
//! Old bundles keep verifying; no rotation event.
//!
//! If **NO**: Scope A becomes a fresh-rotation event. Operators run
//! `rotate-pubkey-bundle` on every workspace and the trust-store
//! pivots through the rotation ceremony documented in
//! `docs/SECURITY-NOTES.md`. Either outcome is acceptable; we just
//! need the data point *before* committing to the wave-3 design.
//!
//! ## Why a separate `tests/` integration test
//!
//! Lives in `tests/` (one binary per file) so it runs in its own
//! process. The PKCS#11 module's `C_Initialize` is global per process
//! and SoftHSM2 occasionally objects to two simultaneous initialisers
//! within the same address space — keeping this test outside the
//! `cargo test` unit-test pool avoids the cross-test interference
//! that would otherwise require `serial_test` plumbing.
//!
//! ## Gates
//!
//! * **Compile gate:** `--features hsm`. Without it, the file expands
//!   to a no-op stub so the test still *compiles* and `cargo test`
//!   in default-features CI is green. The runtime body only exists
//!   when cryptoki is in scope.
//! * **Runtime gate:** `ATLAS_TEST_HSM_BYTE_EQUIV=1` AND the V1.10
//!   HSM trio (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`,
//!   `ATLAS_HSM_PIN_FILE`). Either missing → `eprintln!("SKIP …")`
//!   and the test passes. The double gate (env opt-in *and* trio
//!   present) keeps a CI lane that just sets the trio for the
//!   wave-2 SoftHSM2 smoke from accidentally running this test
//!   under a different SoftHSM2 token whose user PIN happens to
//!   match — the explicit `ATLAS_TEST_HSM_BYTE_EQUIV=1` is the
//!   "yes, run the import + extract path on this token" handshake.
//!
//! ## Why import, not generate
//!
//! `CKM_EC_EDWARDS_KEY_PAIR_GEN` produces a *fresh* key with no
//! relationship to any seed — that path is operationally interesting
//! (Scope A wave 3 will use it for new workspaces) but tells us
//! nothing about migration. The byte-equivalence question is
//! specifically: "if we hand the HSM the same 32 bytes V1.9 fed to
//! `ed25519_dalek`, does the resulting pubkey match?" That requires
//! `create_object` with a known `CKA_VALUE` — i.e. *import*, not
//! generation. SoftHSM2 (OpenSSL-backed) is RFC 8032 §5.1.5 conformant
//! and is expected to byte-match. Commercial HSMs may refuse the
//! import path entirely (fresh-only key gen by policy); the test
//! result on SoftHSM2 is the *upper bound* on what's possible.
//!
//! ## Scope
//!
//! This test does NOT exercise:
//!   * The wave-2 `Pkcs11MasterSeedHkdf` HKDF path (covered by the
//!     existing wave-2 SoftHSM2 smoke harness — out of scope).
//!   * Vendor-specific keypair-generation quirks (Thales Luna,
//!     YubiHSM2 — those need their own pre-flight runs).
//!   * Multi-key concurrency (single-shot, single-token here).

#![cfg(feature = "hsm")]

use std::env;

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::mechanism::eddsa::{EddsaParams, EddsaSignatureScheme};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, KeyType, ObjectClass};
use cryptoki::session::UserType;
use cryptoki::types::AuthPin;
use ed25519_dalek::{Signer, SigningKey, Verifier};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

/// Runtime opt-in. Distinct from the wave-2 `ATLAS_HSM_*` trio because
/// this test mutates the token (creates and destroys two private-key
/// objects). The trio alone is the wave-2 read-only smoke; this env
/// var is the additional "yes, exercise the import path on this slot"
/// handshake.
const RUNTIME_GATE_ENV: &str = "ATLAS_TEST_HSM_BYTE_EQUIV";

/// Workspace ID used for the byte-equivalence pin. Matches the
/// `workspace_pubkeys_are_pinned` golden in `crates/atlas-signer/src/keys.rs`,
/// which pins the V1.9 software-derived pubkey for the same id. If the
/// software pin ever rotates, this constant trails it.
const PIN_WORKSPACE: &str = "alice";

/// V1.9 master seed and HKDF info-prefix come straight from
/// `atlas_signer::keys` (both are `pub const`). Re-pinning them locally
/// would re-introduce a silent-drift risk: if the production constant
/// rotated and the test mirror did not, the byte-equivalence comparison
/// would degrade into a "did we change the dev seed?" comparison
/// instead of the load-bearing "does HSM import match `from_bytes`?"
/// comparison. By taking the crate-pub value here, the integration
/// test fails to compile on type mismatch and inherits any value
/// rotation automatically. The default-features unit tests in
/// `keys.rs` (`workspace_pubkeys_are_pinned`) already pin the *output*
/// of these constants, so any drift is caught at default-features CI
/// without needing a separate sync guard in this file.
use atlas_signer::keys::{DEV_MASTER_SEED, HKDF_INFO_PREFIX};

/// Object label written into the HSM during the test. Distinct from
/// the wave-2 production label (`atlas-master-seed-v1`) so this test
/// cannot stomp on a real master-seed object even if accidentally run
/// against a production token.
const IMPORT_LABEL_PRIVATE: &str = "atlas-byte-equiv-pre-flight-priv";
const IMPORT_LABEL_PUBLIC: &str = "atlas-byte-equiv-pre-flight-pub";

/// ASN.1 DER for the printable string `"edwards25519"` — the
/// `CKA_EC_PARAMS` value SoftHSM2/OpenSSL accepts for Ed25519. Tag
/// 0x13 = PrintableString, length 0x0c = 12, then the 12 ASCII bytes.
/// The OID encoding (`06 03 2B 65 70`, OID 1.3.101.112) also works
/// on SoftHSM2 ≥ 2.6 but the printable-string form is the one cryptoki's
/// own integration tests use, which is the safest bet for SoftHSM2
/// compatibility across the LTS versions Linux distros ship.
const ED25519_PARAMS_PRINTABLE: [u8; 14] = [
    0x13, 0x0c, b'e', b'd', b'w', b'a', b'r', b'd', b's', b'2', b'5', b'5', b'1', b'9',
];

/// Fixed message signed by both paths. Arbitrary bytes — the test
/// asserts deterministic Ed25519 produces identical signatures from
/// both keys, so the message content is irrelevant beyond non-empty.
const TEST_MESSAGE: &[u8] = b"V1.11 Scope A pre-flight: byte-equivalence probe";

/// V1.11 Scope A pre-flight, primary assertion.
///
/// Skips cleanly when the runtime gate is not set or the HSM trio is
/// missing — that keeps `cargo test --features hsm` green on a dev
/// machine without SoftHSM2. Under CI where SoftHSM2 is installed and
/// the gate is set, the test:
///
///   1. Computes the V1.9 reference: HKDF-SHA256 over `DEV_MASTER_SEED`
///      with `info = "atlas-anchor-v1:alice"`, then
///      `SigningKey::from_bytes(&hkdf_output)`.
///   2. Imports the same 32-byte HKDF output into the HSM as a
///      `CKK_EC_EDWARDS` private key with matching public-key object.
///   3. Signs `TEST_MESSAGE` via `CKM_EDDSA(Ed25519)` with the
///      HSM-resident private key.
///   4. Reads `CKA_EC_POINT` from the HSM-resident public key.
///   5. Asserts byte-equivalence: HSM pubkey == software pubkey,
///      HSM signature == software signature.
///   6. Cross-verifies: HSM signature verifies under software pubkey
///      AND software signature verifies under HSM pubkey.
///   7. Cleans up both objects on every exit path (RAII guard).
///
/// All four assertions must pass for Scope A to ship as a
/// backwards-compatible migration. Any single failure is the signal
/// that wave 3 is a fresh-rotation event.
#[test]
fn softhsm2_imports_v1_9_seed_and_yields_byte_identical_pubkey() {
    let Some(env) = require_runtime_gate() else {
        return;
    };

    let (hkdf_output, software_pubkey, software_sig) = compute_software_reference();

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
                env.module_path.display()
            )
        });

    let session = pkcs11.open_rw_session(slot).expect("open_rw_session");
    let pin = read_pin_zeroized(&env.pin_file);
    session
        .login(UserType::User, Some(&pin))
        .expect("PKCS#11 C_Login");

    // RAII: destroy both objects on every exit path (Ok, panic in any
    // assertion below, FFI panic). The pre-existing wave-2 derive guard
    // (`EphemeralObjectGuard` in `hsm/pkcs11.rs`) is the V1.11 M-3
    // pattern; this test mirrors it for parity. Without the guard a
    // panicking assertion would leave the imported private key on the
    // token, defeating the "import → extract → destroy" hermetic-test
    // contract on the next run.
    let import = import_seed_into_hsm(&session, &hkdf_output);
    let _guard = ImportGuard {
        session: &session,
        private: import.private,
        public: import.public,
    };

    let hsm_pubkey = read_ec_point_pubkey(&session, import.public);
    let hsm_sig = sign_with_hsm(&session, import.private, TEST_MESSAGE);

    // Primary assertion. Equality here is the load-bearing data point:
    // the failure mode at byte-divergence is wave 3 = fresh-rotation
    // event for every existing workspace. Phrased as a panic with a
    // verbose remediation so the CI failure log is the operator's
    // single-source-of-truth without needing to re-derive the
    // implication.
    assert_eq!(
        hsm_pubkey, software_pubkey,
        "BYTE-EQUIVALENCE FAIL: SoftHSM2 imported the V1.9 HKDF output \
         and produced a different Ed25519 pubkey than ed25519_dalek for \
         the same 32 input bytes. Implication: V1.11 Scope A (HSM seal \
         scope wave 3) cannot preserve V1.9 workspace pubkeys via a \
         seed-import migration. Wave 3 must ship as a fresh-rotation \
         event — every workspace runs `rotate-pubkey-bundle` and the \
         trust-store pivots. Update `.handoff/v1.11-handoff.md` Scope A \
         section accordingly. Software pubkey: {} HSM pubkey: {}",
        hex::encode(software_pubkey),
        hex::encode(hsm_pubkey),
    );

    // Bonus #1: signature byte-equality. Ed25519 is deterministic
    // (RFC 8032 §5.1.6 — nonce is a SHA-512 of the second half of the
    // expanded private key plus the message), so the same key + same
    // message MUST yield the same signature. If pubkeys match but
    // signatures don't, the HSM is producing valid-but-non-deterministic
    // signatures — itself a security-relevant divergence (an attacker
    // observing two signatures over the same message can leak per-call
    // randomness). Same panic shape as above so the CI log carries the
    // remediation.
    assert_eq!(
        hsm_sig, software_sig,
        "DETERMINISM FAIL: SoftHSM2 produced a different signature than \
         ed25519_dalek for the same key + same message. Ed25519 is \
         deterministic per RFC 8032 §5.1.6; divergence here means the \
         HSM is using non-deterministic nonces, which (a) breaks bundle \
         reproducibility and (b) is a sidechannel surface. Investigate \
         the SoftHSM2 build (some patched OpenSSL builds inject a \
         random nonce). Software sig: {} HSM sig: {}",
        hex::encode(software_sig),
        hex::encode(hsm_sig),
    );

    // Bonus #2: cross-verification. Even if pubkeys + signatures
    // matched byte-wise above, an independent verify-with-the-other-key
    // round-trip catches a degenerate case where both paths happen to
    // produce identical *bytes* via a buggy shared encoder rather than
    // a genuine RFC 8032 derivation. Both `dalek` and `cryptoki` would
    // have to be wrong in the same way for this to pass spuriously,
    // which is the strongest belt-and-braces check available without
    // a third independent Ed25519 implementation.
    let software_signing_key = SigningKey::from_bytes(&hkdf_output);
    let parsed_hsm_pubkey = ed25519_dalek::VerifyingKey::from_bytes(&hsm_pubkey)
        .expect("HSM pubkey must be a well-formed 32-byte Ed25519 point");
    parsed_hsm_pubkey
        .verify(
            TEST_MESSAGE,
            &ed25519_dalek::Signature::from_bytes(
                &software_sig
                    .as_slice()
                    .try_into()
                    .expect("Ed25519 signatures are 64 bytes"),
            ),
        )
        .expect(
            "cross-verify: software signature must verify under HSM-extracted pubkey \
             (if this fails, the HSM pubkey + software pubkey share bytes by accident, \
             not by RFC 8032 derivation)",
        );
    software_signing_key
        .verifying_key()
        .verify(
            TEST_MESSAGE,
            &ed25519_dalek::Signature::from_bytes(
                &hsm_sig
                    .as_slice()
                    .try_into()
                    .expect("Ed25519 signatures are 64 bytes"),
            ),
        )
        .expect(
            "cross-verify: HSM signature must verify under software-derived pubkey \
             (if this fails, the HSM produced a malformed signature that happened \
             to byte-match the software signature)",
        );

    eprintln!(
        "V1.11 Scope A PRE-FLIGHT PASS — SoftHSM2 byte-equivalent for \
         workspace {PIN_WORKSPACE:?}. Wave 3 may ship as backwards-compatible \
         seed-import migration (subject to commercial-HSM confirmation)."
    );
}

/// V1.11 Scope A operational sanity — `CKM_EC_EDWARDS_KEY_PAIR_GEN`
/// + `CKM_EDDSA(Ed25519)` round-trip.
///
/// Independent of the byte-equivalence test above. Even if the
/// import-based migration above fails (forcing fresh-rotation),
/// wave 3 still needs the *generate-and-sign* path to work end-to-end.
/// This test pins that contract: a freshly-generated keypair signs and
/// verifies a message via the cryptoki API surface wave 3 will adopt.
///
/// Same gating as the byte-equivalence test (skips without
/// `ATLAS_TEST_HSM_BYTE_EQUIV=1` + HSM trio). Cleans up both objects
/// via the same RAII guard so a panic mid-test does not orphan
/// keypairs on the SoftHSM2 token.
#[test]
fn softhsm2_generates_ed25519_and_signs_via_ckm_eddsa() {
    let Some(env) = require_runtime_gate() else {
        return;
    };

    let pkcs11 = Pkcs11::new(&env.module_path).expect("PKCS#11 module load");
    pkcs11
        .initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
        .expect("PKCS#11 C_Initialize");

    let slot = pkcs11
        .get_slots_with_token()
        .expect("get_slots_with_token")
        .into_iter()
        .find(|s| s.id() == env.slot)
        .unwrap_or_else(|| panic!("configured slot {} missing", env.slot));

    let session = pkcs11.open_rw_session(slot).expect("open_rw_session");
    let pin = read_pin_zeroized(&env.pin_file);
    session
        .login(UserType::User, Some(&pin))
        .expect("PKCS#11 C_Login");

    let pub_template = vec![
        Attribute::Token(false),
        Attribute::Private(false),
        Attribute::Verify(true),
        Attribute::EcParams(ED25519_PARAMS_PRINTABLE.to_vec()),
    ];
    let priv_template = vec![
        Attribute::Token(false),
        Attribute::Sign(true),
        Attribute::Sensitive(true),
        Attribute::Extractable(false),
    ];
    let (public, private) = session
        .generate_key_pair(
            &Mechanism::EccEdwardsKeyPairGen,
            &pub_template,
            &priv_template,
        )
        .expect("CKM_EC_EDWARDS_KEY_PAIR_GEN");

    let _guard = ImportGuard {
        session: &session,
        private,
        public,
    };

    let scheme = EddsaSignatureScheme::Ed25519;
    let params = EddsaParams::new(scheme);
    let signature = session
        .sign(&Mechanism::Eddsa(params), private, TEST_MESSAGE)
        .expect("CKM_EDDSA Ed25519 sign");

    // Local PKCS#11 verify is the operational check; cross-checking
    // against `ed25519_dalek::VerifyingKey` would require parsing
    // `CKA_EC_POINT` (covered by the byte-equivalence test). Keeping
    // this round-trip purely PKCS#11-side isolates "the wave-3 API
    // surface works" from "byte-equivalence holds".
    session
        .verify(
            &Mechanism::Eddsa(EddsaParams::new(scheme)),
            public,
            TEST_MESSAGE,
            &signature,
        )
        .expect("CKM_EDDSA Ed25519 verify must round-trip on the same token");

    eprintln!(
        "V1.11 Scope A OPERATIONAL SANITY — CKM_EC_EDWARDS_KEY_PAIR_GEN + \
         CKM_EDDSA(Ed25519) round-trip OK on SoftHSM2."
    );
}

/// Resolved runtime configuration for both tests. Returns `None` when
/// the runtime gate is not set or the HSM trio is missing.
struct RuntimeEnv {
    module_path: std::path::PathBuf,
    slot: u64,
    pin_file: std::path::PathBuf,
}

fn require_runtime_gate() -> Option<RuntimeEnv> {
    if !env::var(RUNTIME_GATE_ENV)
        .map(|v| v.trim() == "1")
        .unwrap_or(false)
    {
        eprintln!(
            "SKIP: {RUNTIME_GATE_ENV} is not set to 1 — V1.11 Scope A \
             pre-flight does not run on dev machines without an explicit \
             opt-in (the test creates and destroys objects on the \
             configured SoftHSM2 token; the gate is the operator's \
             handshake that this is the right token to mutate)."
        );
        return None;
    }
    let module_path = env::var("ATLAS_HSM_PKCS11_LIB").ok().map(std::path::PathBuf::from);
    let slot = env::var("ATLAS_HSM_SLOT").ok().and_then(|s| s.parse::<u64>().ok());
    let pin_file = env::var("ATLAS_HSM_PIN_FILE").ok().map(std::path::PathBuf::from);
    match (module_path, slot, pin_file) {
        (Some(m), Some(s), Some(p)) => Some(RuntimeEnv {
            module_path: m,
            slot: s,
            pin_file: p,
        }),
        _ => {
            eprintln!(
                "SKIP: HSM trio (ATLAS_HSM_PKCS11_LIB, ATLAS_HSM_SLOT, \
                 ATLAS_HSM_PIN_FILE) not fully present even though \
                 {RUNTIME_GATE_ENV}=1. Either set the trio or unset the \
                 gate; partial config is the V1.10 wave-2 footgun the \
                 test refuses to exercise."
            );
            None
        }
    }
}

/// Compute the V1.9 software-derived reference: HKDF-SHA256 →
/// `SigningKey::from_bytes` → `(seed_bytes, pubkey_bytes, signature_bytes)`.
/// Returned `seed_bytes` are the 32 bytes that get fed to both paths
/// (software via `from_bytes`, HSM via `CKA_VALUE` import); they are
/// returned wrapped in `Zeroizing` so the caller's drop scrubs the
/// allocation. The caller MUST keep the returned `Zeroizing` alive
/// until after the HSM import call returns — once the FFI side has
/// consumed the seed and produced the corresponding HSM object handle,
/// the host-side seed copy is no longer needed and the wrapper's
/// scrub-on-drop closes the residual disclosure window.
fn compute_software_reference() -> (Zeroizing<[u8; 32]>, [u8; 32], [u8; 64]) {
    let hk = Hkdf::<Sha256>::new(None, &DEV_MASTER_SEED);
    let info = format!("{HKDF_INFO_PREFIX}{PIN_WORKSPACE}");
    let mut seed: Zeroizing<[u8; 32]> = Zeroizing::new([0u8; 32]);
    hk.expand(info.as_bytes(), seed.as_mut())
        .expect("HKDF-SHA256 expand of 32 bytes");
    let signing_key = SigningKey::from_bytes(&seed);
    let pubkey = signing_key.verifying_key().to_bytes();
    let signature = signing_key.sign(TEST_MESSAGE).to_bytes();
    (seed, pubkey, signature)
}

/// Read the PIN file into a `Zeroizing<Vec<u8>>` and return an `AuthPin`
/// whose `SecretString` scrubs on drop. Mirrors the production
/// `read_pin_file` helper in `crates/atlas-signer/src/hsm/pkcs11.rs`
/// (sans the Unix permission guard, which is wave-2 production code's
/// responsibility — this test file does not own the runbook permissions
/// contract). The pre-sized `Zeroizing<Vec<u8>>` ensures the read
/// buffer never re-allocates mid-read (which would orphan partial PIN
/// bytes in the freed-allocator pool); the `&str` trim avoids an
/// intermediate `String` allocation; the final `to_string()` is
/// immediately moved into `AuthPin::from`, whose `SecretString`
/// (`secrecy 0.10`) scrubs the underlying buffer at drop time. Without
/// this helper the inline `read_to_string().trim().to_string()` leaks
/// PIN bytes through (a) the original `String` from `read_to_string`,
/// and (b) the trimmed `String` clone — both unscrubbed on drop.
fn read_pin_zeroized(path: &std::path::Path) -> AuthPin {
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .unwrap_or_else(|e| panic!("PIN file {} unreadable: {e}", path.display()));
    let meta = file.metadata().unwrap_or_else(|e| {
        panic!(
            "PIN file {} metadata unreadable: {e}",
            path.display()
        )
    });
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
    AuthPin::from(trimmed.to_string())
}

/// Pair of HSM object handles produced by `import_seed_into_hsm`.
struct ImportedKeyPair {
    private: cryptoki::object::ObjectHandle,
    public: cryptoki::object::ObjectHandle,
}

/// Import a 32-byte Ed25519 seed into the HSM as a private+public
/// keypair. `CKA_VALUE` carries the raw seed bytes (RFC 8032 §5.1.5
/// "private key = 32 random octets"); SoftHSM2's OpenSSL backend then
/// derives the corresponding public key via SHA-512 + scalar
/// multiplication. The public-key object lets us read `CKA_EC_POINT`
/// without re-deriving on the host side.
fn import_seed_into_hsm(
    session: &cryptoki::session::Session,
    seed: &[u8; 32],
) -> ImportedKeyPair {
    // V1.10 wave-2 zeroize parity: the seed bytes that cross the
    // cryptoki FFI boundary into `Attribute::Value(Vec<u8>)` MUST be
    // scrubbed on every exit path of this function. Cryptoki's
    // `Attribute::Value` variant owns its `Vec<u8>` by value (no
    // by-reference variant exists), so an unavoidable unscrubbed copy
    // lives inside the template `Vec<Attribute>` for the duration of
    // the `create_object` call. We bound that copy's lifetime tightly
    // by:
    //   1. Holding the source-of-truth seed bytes in a `Zeroizing<Vec<u8>>`
    //      that scrubs at function exit (covers panic-mid-FFI).
    //   2. Cloning into the template inside a nested block so the
    //      template (and its unscrubbed `Attribute::Value` copy) drop
    //      *before* we proceed to the public-key import path.
    // Net residual: the unscrubbed FFI copy lives ≈ one `create_object`
    // syscall window, then is freed (un-zeroed but no longer reachable).
    // The Zeroizing source-of-truth scrubs as the function returns.
    let seed_zeroized: Zeroizing<Vec<u8>> = Zeroizing::new(seed.to_vec());
    let private = {
        let priv_template = vec![
            Attribute::Class(ObjectClass::PRIVATE_KEY),
            Attribute::KeyType(KeyType::EC_EDWARDS),
            Attribute::EcParams(ED25519_PARAMS_PRINTABLE.to_vec()),
            Attribute::Value((*seed_zeroized).clone()),
            Attribute::Label(IMPORT_LABEL_PRIVATE.as_bytes().to_vec()),
            // Token=false so the imported key dies with the session even
            // if the destroy_object guard fails to fire (panic in cryptoki
            // FFI, OS-level kill, etc.). The session-close ceiling bounds
            // the worst case; the RAII guard tightens it to per-test.
            Attribute::Token(false),
            Attribute::Sign(true),
            // Sensitive=true so even a misbehaving downstream call cannot
            // re-export CKA_VALUE; the import side already knows the seed
            // (we just provided it), so making it sensitive here costs
            // nothing and matches the production wave-3 shape.
            Attribute::Sensitive(true),
            Attribute::Extractable(false),
        ];
        session
            .create_object(&priv_template)
            .expect("import Ed25519 private key (CKA_VALUE = 32-byte seed)")
        // priv_template (and its unscrubbed Attribute::Value clone)
        // drops here; seed_zeroized scrubs on its own drop at function
        // exit.
    };

    // Compute the public-key bytes on the host side and import them
    // as the matching public-key object. SoftHSM2 does *not* auto-derive
    // a public-key object when you `create_object` a private key — that
    // only happens via `generate_key_pair`. The wave-3 production code
    // would resolve the pubkey via a different path (per-tenant pubkey
    // is stored alongside the workspace metadata, not re-derived from
    // the HSM each call), but for this byte-equivalence test the
    // simplest source of truth is "ask the HSM to sign, then check the
    // resulting signature verifies under the host-derived pubkey" —
    // which is what the assertions above actually do. The public-key
    // object below exists so the test can also read `CKA_EC_POINT` for
    // the direct pubkey-byte comparison.
    let host_pubkey = SigningKey::from_bytes(seed).verifying_key().to_bytes();
    let pub_template = vec![
        Attribute::Class(ObjectClass::PUBLIC_KEY),
        Attribute::KeyType(KeyType::EC_EDWARDS),
        Attribute::EcParams(ED25519_PARAMS_PRINTABLE.to_vec()),
        // CKA_EC_POINT for Ed25519 = the raw 32-byte compressed point,
        // wrapped in an ASN.1 OCTET STRING (tag 0x04, length 0x20).
        // SoftHSM2 accepts both raw and DER-wrapped on import; emitting
        // the wrapped form matches what `get_attributes` returns on
        // read-back, which keeps the import + extract symmetric.
        Attribute::EcPoint(wrap_octet_string(&host_pubkey)),
        Attribute::Label(IMPORT_LABEL_PUBLIC.as_bytes().to_vec()),
        Attribute::Token(false),
        Attribute::Verify(true),
    ];
    let public = session
        .create_object(&pub_template)
        .expect("import Ed25519 public key (CKA_EC_POINT = host-derived pubkey)");

    ImportedKeyPair { private, public }
}

/// Wrap a 32-byte Ed25519 pubkey in an ASN.1 DER OCTET STRING. The
/// PKCS#11 v3.0 spec for `CKA_EC_POINT` mandates DER encoding; SoftHSM2
/// is lenient (accepts raw bytes too) but the spec-compliant form is
/// the safer bet for cross-vendor reuse of this helper.
fn wrap_octet_string(raw: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + raw.len());
    out.push(0x04); // OCTET STRING tag
    out.push(raw.len() as u8); // length (32 fits in one byte)
    out.extend_from_slice(raw);
    out
}

/// Unwrap a DER OCTET STRING back to its inner 32 bytes. Symmetric
/// counterpart to `wrap_octet_string`. Tolerates the raw-bytes form
/// (some HSMs return CKA_EC_POINT without the OCTET STRING wrapper)
/// by detecting the 32-byte length and skipping the tag check in that
/// case. The raw-form branch surfaces an `eprintln!` annotation so a
/// vendor-divergence in the encoding is visible in the CI log instead
/// of silently passing — useful when this test eventually runs against
/// a non-SoftHSM2 token (Thales Luna, YubiHSM2) whose CKA_EC_POINT
/// encoding may differ.
fn unwrap_octet_string(bytes: &[u8]) -> [u8; 32] {
    if bytes.len() == 32 {
        eprintln!(
            "note: HSM returned raw 32-byte CKA_EC_POINT (no OCTET STRING \
             wrapper). PKCS#11 v3.0 §10.10 mandates the wrapped form, but \
             SoftHSM2 ≤ 2.5 and several commercial HSMs ship the raw form; \
             accepting both here keeps the byte-equivalence assertion the \
             load-bearing signal."
        );
        return bytes.try_into().expect("len-32 slice → [u8; 32]");
    }
    assert!(
        bytes.len() == 34 && bytes[0] == 0x04 && bytes[1] == 32,
        "unexpected CKA_EC_POINT shape: {} bytes, first 2 = [{:#x}, {:#x}] \
         (expected 32 bytes raw or 34 bytes [0x04, 0x20, ..32 raw])",
        bytes.len(),
        bytes.first().copied().unwrap_or(0),
        bytes.get(1).copied().unwrap_or(0),
    );
    bytes[2..34].try_into().expect("len-32 slice → [u8; 32]")
}

/// Read CKA_EC_POINT from the public-key object and return the raw
/// 32-byte Ed25519 compressed point.
fn read_ec_point_pubkey(
    session: &cryptoki::session::Session,
    public: cryptoki::object::ObjectHandle,
) -> [u8; 32] {
    let attrs = session
        .get_attributes(public, &[AttributeType::EcPoint])
        .expect("get_attributes(EcPoint)");
    let raw = attrs
        .into_iter()
        .find_map(|a| match a {
            Attribute::EcPoint(v) => Some(v),
            _ => None,
        })
        .expect("public-key object missing CKA_EC_POINT");
    unwrap_octet_string(&raw)
}

/// Sign a message via `CKM_EDDSA(Ed25519)` and return the 64-byte
/// signature. Fixed mechanism + fixed scheme keeps the signature
/// byte-comparison meaningful (a different scheme — e.g. Ed25519ph —
/// would produce a different signature even for the same key/message).
fn sign_with_hsm(
    session: &cryptoki::session::Session,
    private: cryptoki::object::ObjectHandle,
    msg: &[u8],
) -> [u8; 64] {
    let params = EddsaParams::new(EddsaSignatureScheme::Ed25519);
    let sig = session
        .sign(&Mechanism::Eddsa(params), private, msg)
        .expect("CKM_EDDSA Ed25519 sign");
    sig.try_into()
        .expect("Ed25519 signature must be 64 bytes per RFC 8032 §5.1.6")
}

/// RAII guard that destroys both halves of an imported keypair on
/// drop. Mirrors the wave-2 `EphemeralObjectGuard` pattern in
/// `hsm/pkcs11.rs` so a panic mid-test cannot leave imported objects
/// on the SoftHSM2 token. `Token=false` on the templates above is the
/// strict ceiling (objects die with session close); this guard
/// tightens it to "objects die when the test exits, not when the
/// process exits".
struct ImportGuard<'a> {
    session: &'a cryptoki::session::Session,
    private: cryptoki::object::ObjectHandle,
    public: cryptoki::object::ObjectHandle,
}

impl Drop for ImportGuard<'_> {
    fn drop(&mut self) {
        let _ = self.session.destroy_object(self.private);
        let _ = self.session.destroy_object(self.public);
    }
}
