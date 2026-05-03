//! V1.11 Scope A wave-3 Phase B — sealed per-workspace signer
//! end-to-end against a live PKCS#11 token.
//!
//! Lives in `tests/` (one binary per file) so it runs in its own
//! process — same `C_Initialize`-is-global rationale as the wave-2
//! `hsm_byte_equivalence` integration test.
//!
//! ## Question this test answers
//!
//! For a `Pkcs11WorkspaceSigner` opened against a configured slot,
//! does the find-or-generate + CKM_EDDSA(Ed25519) path produce
//! signatures that:
//!
//!   1. Round-trip through `ed25519_dalek::VerifyingKey::verify_strict`
//!      under the pubkey extracted from CKA_EC_POINT?
//!   2. Are deterministic (RFC 8032 §5.1.6) across two calls with the
//!      same input?
//!   3. Persist across `open` / drop / `open` cycles, i.e. Token=true
//!      means the per-workspace key actually survives the session
//!      close that drop triggers?
//!   4. Stay isolated per-workspace (alice's signature does NOT
//!      verify under bob's pubkey)?
//!
//! All four properties are load-bearing for the wave-3 production
//! design: (1) pins API surface compatibility with the dev impl;
//! (2) pins the deterministic-Ed25519 contract the verifier
//! depends on for bundle reproducibility; (3) pins the Token=true
//! template choice — without it, restarting the signer would silently
//! rotate every workspace pubkey; (4) pins per-tenant isolation
//! against a degenerate `find_or_generate` impl that returned a
//! shared key for every workspace_id.
//!
//! ## Gates
//!
//! * **Compile gate:** `--features hsm`. Without it, the file expands
//!   to a no-op stub so the test still *compiles* and `cargo test`
//!   in default-features CI is green.
//! * **Runtime gate:** `ATLAS_TEST_HSM_WORKSPACE_SIGNER=1` AND the
//!   V1.10 HSM trio (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`,
//!   `ATLAS_HSM_PIN_FILE`). Either missing → `eprintln!("SKIP …")`
//!   and the test passes. The double gate is the same handshake
//!   the wave-2 byte-equivalence test uses.
//!
//! ## Why a SEPARATE runtime gate from `hsm_byte_equivalence`?
//!
//! Both tests mutate the token (this one persists workspace keys via
//! `Token=true`; the byte-equivalence test imports + destroys
//! ephemeral keys). A single combined gate would force operators to
//! either run both at once or split the ceremony. Splitting the
//! gates lets a CI lane that only wants the wave-3 generate-and-sign
//! path skip the import-and-destroy path (and vice versa) without
//! touching the trio. Cleanup at the end of THIS test purges the
//! workspace keys it created so a subsequent test run starts from a
//! clean slate.

#![cfg(feature = "hsm")]

use std::env;

use atlas_signer::hsm::config::HsmConfig;
use atlas_signer::hsm::pkcs11::read_pin_file;
use atlas_signer::hsm::pkcs11_workspace::Pkcs11WorkspaceSigner;
use atlas_signer::workspace_signer::WorkspaceSigner;
use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::error::{Error as Pkcs11Error, RvError};
use cryptoki::object::{Attribute, KeyType, ObjectClass};
use cryptoki::session::UserType;
use ed25519_dalek::{Signature, VerifyingKey};

/// Runtime opt-in. Distinct from `ATLAS_TEST_HSM_BYTE_EQUIV` because
/// the two tests mutate the token in different ways (wave-2 imports
/// then destroys, wave-3 generates then persists). Splitting the gate
/// lets each ceremony run independently.
const RUNTIME_GATE_ENV: &str = "ATLAS_TEST_HSM_WORKSPACE_SIGNER";

/// Per-test workspace_id pool. Distinct from the `keys::tests::workspace_pubkeys_are_pinned`
/// pinned ids ("alice", "ws-mcp-default") so the integration test
/// cannot accidentally collide with a SoftHSM2 token that was also
/// used for a software-pin regression run. Cleanup at the end of the
/// test removes objects with these labels so re-runs start clean.
const WS_ALPHA: &str = "wave3-it-alpha";
const WS_BETA: &str = "wave3-it-beta";
const WS_PERSIST: &str = "wave3-it-persist";

/// Same label prefix the production [`Pkcs11WorkspaceSigner`] uses.
/// Re-pinned here (not imported) so a refactor that renames the
/// production constant trips this test at compile time of the
/// production code (workspace_signer.rs imports + uses it) AND
/// at runtime here (cleanup misses the new prefix). The intentional
/// duplication is the canary — drift would mean cleanup leaks objects.
const WORKSPACE_LABEL_PREFIX: &str = "atlas-workspace-key-v1:";

const TEST_MESSAGE: &[u8] = b"V1.11 Scope A wave-3 phase-b integration witness";

/// V1.11 Scope A wave-3 — primary integration assertion: the
/// `Pkcs11WorkspaceSigner` produces signatures that verify under
/// the pubkey it advertises.
///
/// This is the load-bearing wave-3 production property. If this
/// passes, the sealed-key path is byte-faithful to ed25519-dalek
/// under the CKM_EDDSA mechanism on the configured token.
#[test]
fn workspace_signer_sign_verifies_under_advertised_pubkey() {
    let Some(env) = require_runtime_gate() else {
        return;
    };

    // Cleanup ceremony invariant: cleanup MUST NOT overlap with a live
    // `Pkcs11WorkspaceSigner` instance because both share the same
    // dlopen'd PKCS#11 library and a process-global C_Initialize /
    // C_Finalize lifecycle. If cleanup runs while the signer is alive,
    // the cleanup's `Pkcs11` drop calls C_Finalize, tearing down the
    // signer's session and producing CKR_CRYPTOKI_NOT_INITIALIZED on
    // the next sign call. The bracket here — pre-cleanup, scoped
    // signer, post-cleanup — keeps lifetimes disjoint.
    cleanup_test_workspaces(&env);

    let (signature_bytes, pubkey_bytes) = {
        let cfg = HsmConfig::from_env(|name| env_lookup(&env, name)).expect("HsmConfig parse");
        let cfg = cfg.expect("HSM trio present (require_runtime_gate verified)");
        let signer = Pkcs11WorkspaceSigner::open(cfg).expect("Pkcs11WorkspaceSigner::open");

        let sig = signer
            .sign(WS_ALPHA, TEST_MESSAGE)
            .expect("first-call sign generates the keypair and returns a signature");
        let pk = signer
            .pubkey(WS_ALPHA)
            .expect("pubkey resolves the cached entry from the prior sign call");
        (sig, pk)
        // signer drops here → C_CloseSession → C_Finalize completes
        // before the post-cleanup ceremony reopens the library.
    };

    let signature = Signature::from_bytes(&signature_bytes);
    let verifying_key =
        VerifyingKey::from_bytes(&pubkey_bytes).expect("HSM-extracted pubkey must be valid");
    verifying_key
        .verify_strict(TEST_MESSAGE, &signature)
        .expect("HSM signature must verify under HSM-extracted pubkey via verify_strict");

    cleanup_test_workspaces(&env);
    eprintln!(
        "V1.11 Scope A wave-3 INTEGRATION PASS — Pkcs11WorkspaceSigner sign+pubkey \
         round-trip verifies under verify_strict for workspace {WS_ALPHA:?}"
    );
}

/// V1.11 Scope A wave-3 — RFC 8032 §5.1.6 determinism contract: two
/// calls with the same (workspace_id, signing_input) MUST produce
/// byte-identical 64-byte signatures. Catches a token build that
/// uses non-deterministic Ed25519 (some vendor-patched OpenSSL
/// builds inject randomness — see the wave-2 byte-equivalence test
/// docstring for context). Non-deterministic sign would break
/// bundle reproducibility AND introduce a sidechannel surface
/// (an attacker observing two signatures can extract per-call
/// randomness).
#[test]
fn workspace_signer_sign_is_deterministic() {
    let Some(env) = require_runtime_gate() else {
        return;
    };

    cleanup_test_workspaces(&env);

    {
        let cfg = HsmConfig::from_env(|name| env_lookup(&env, name))
            .expect("HsmConfig parse")
            .expect("HSM trio present");
        let signer = Pkcs11WorkspaceSigner::open(cfg).expect("open");

        let a = signer.sign(WS_ALPHA, TEST_MESSAGE).expect("first sign");
        let b = signer.sign(WS_ALPHA, TEST_MESSAGE).expect("second sign");
        assert_eq!(
            a, b,
            "PKCS#11 CKM_EDDSA(Ed25519) MUST be deterministic per RFC 8032 §5.1.6 — \
             non-deterministic sign breaks bundle reproducibility AND leaks per-call \
             randomness through signature divergence",
        );
    }

    cleanup_test_workspaces(&env);
}

/// V1.11 Scope A wave-3 — per-tenant isolation contract: a signature
/// produced for workspace_id A MUST NOT verify under the pubkey of
/// workspace_id B. Catches a degenerate `find_or_generate` impl that
/// returned a shared key for every workspace_id (which would still
/// pass the round-trip + determinism tests above for any single
/// workspace).
#[test]
fn workspace_signer_signatures_do_not_cross_verify_across_workspaces() {
    let Some(env) = require_runtime_gate() else {
        return;
    };

    cleanup_test_workspaces(&env);

    {
        let cfg = HsmConfig::from_env(|name| env_lookup(&env, name))
            .expect("HsmConfig parse")
            .expect("HSM trio present");
        let signer = Pkcs11WorkspaceSigner::open(cfg).expect("open");

        let alpha_sig = signer.sign(WS_ALPHA, TEST_MESSAGE).expect("alpha sign");
        let beta_pub = signer.pubkey(WS_BETA).expect("beta pubkey");

        let signature = Signature::from_bytes(&alpha_sig);
        let beta_key = VerifyingKey::from_bytes(&beta_pub).expect("beta pubkey valid");
        assert!(
            beta_key.verify_strict(TEST_MESSAGE, &signature).is_err(),
            "alpha's signature MUST NOT verify under beta's pubkey — cross-tenant \
             isolation regression: a degenerate find_or_generate returning a shared \
             key for every workspace_id would silently pass other tests in this file",
        );

        let alpha_pub = signer.pubkey(WS_ALPHA).expect("alpha pubkey");
        assert_ne!(
            alpha_pub, beta_pub,
            "alpha and beta MUST derive independent pubkeys — same regression class \
             as above, caught earlier in the chain",
        );
    }

    cleanup_test_workspaces(&env);
}

/// V1.11 Scope A wave-3 — Token=true persistence contract: a
/// workspace key generated by `Pkcs11WorkspaceSigner` MUST survive
/// the session-close that drops the signer, so reopening the signer
/// resolves the SAME pubkey for the SAME workspace_id.
///
/// This is THE wave-3 production property. If keys are NOT persistent,
/// every signer restart silently rotates every workspace pubkey,
/// breaking verifier-side trust pinning on the next signed bundle.
/// The wave-2 byte-equivalence test uses Token=false (ephemeral); this
/// test pins the wave-3 Token=true contract end-to-end.
#[test]
fn workspace_signer_keys_persist_across_open_cycles() {
    let Some(env) = require_runtime_gate() else {
        return;
    };

    cleanup_test_workspaces(&env);

    let pubkey_first_session = {
        let cfg = HsmConfig::from_env(|name| env_lookup(&env, name))
            .expect("HsmConfig parse")
            .expect("HSM trio present");
        let signer = Pkcs11WorkspaceSigner::open(cfg).expect("first open");
        // Triggering pubkey() forces the find-or-generate path; the
        // resulting Token=true keys live on the token after the signer
        // drops at the end of this scope.
        signer.pubkey(WS_PERSIST).expect("first-session pubkey")
        // signer drops here → C_CloseSession → token retains the keys
        // (Token=true). Module Finalize runs after.
    };

    // Independent open: fresh `Pkcs11::new` + `C_Initialize` + new
    // session. The cache is empty (per-process state); the only way
    // pubkey() can return the same value is if the find_objects path
    // resolves the persistent on-token keypair from the previous
    // session.
    let pubkey_second_session = {
        let cfg = HsmConfig::from_env(|name| env_lookup(&env, name))
            .expect("HsmConfig parse")
            .expect("HSM trio present");
        let signer = Pkcs11WorkspaceSigner::open(cfg).expect("second open");
        signer.pubkey(WS_PERSIST).expect("second-session pubkey")
    };

    assert_eq!(
        pubkey_first_session, pubkey_second_session,
        "Token=true MUST persist the workspace key across open / drop / open cycles. \
         A divergence here means the signer is silently regenerating keys on every \
         restart, which would break verifier-side trust pinning the moment the next \
         bundle ships under a rotated pubkey. Inspect the generate_keypair template \
         — Token=true is the load-bearing attribute.",
    );

    // Defence-in-depth: a fresh sign in the second session also
    // verifies under the persisted pubkey. Catches a (very unlikely)
    // pathology where the find path resolves a stale handle whose
    // signing surface diverged. Scoped so the signer drops before the
    // trailing cleanup ceremony runs (cleanup-overlap invariant — see
    // workspace_signer_sign_verifies_under_advertised_pubkey).
    {
        let cfg = HsmConfig::from_env(|name| env_lookup(&env, name))
            .expect("HsmConfig parse")
            .expect("HSM trio present");
        let signer = Pkcs11WorkspaceSigner::open(cfg).expect("third open");
        let sig_bytes = signer
            .sign(WS_PERSIST, TEST_MESSAGE)
            .expect("third-session sign");
        let signature = Signature::from_bytes(&sig_bytes);
        let verifying_key =
            VerifyingKey::from_bytes(&pubkey_second_session).expect("pubkey valid");
        verifying_key
            .verify_strict(TEST_MESSAGE, &signature)
            .expect("third-session sign must verify under persisted pubkey");
    }

    cleanup_test_workspaces(&env);
    eprintln!(
        "V1.11 Scope A wave-3 PERSISTENCE PASS — workspace keys persist across \
         open/drop/open cycles for workspace {WS_PERSIST:?}"
    );
}

/// Resolved runtime configuration, mirrors the wave-2 RuntimeEnv.
struct RuntimeEnv {
    module_path: std::path::PathBuf,
    slot: u64,
    pin_file: std::path::PathBuf,
}

fn env_lookup(env: &RuntimeEnv, name: &str) -> Option<String> {
    match name {
        "ATLAS_HSM_PKCS11_LIB" => Some(env.module_path.display().to_string()),
        "ATLAS_HSM_SLOT" => Some(env.slot.to_string()),
        "ATLAS_HSM_PIN_FILE" => Some(env.pin_file.display().to_string()),
        _ => None,
    }
}

fn require_runtime_gate() -> Option<RuntimeEnv> {
    if !env::var(RUNTIME_GATE_ENV)
        .map(|v| v.trim() == "1")
        .unwrap_or(false)
    {
        eprintln!(
            "SKIP: {RUNTIME_GATE_ENV} is not set to 1 — V1.11 wave-3 \
             integration test does not run on dev machines without an explicit \
             opt-in (the test creates and destroys persistent (Token=true) \
             objects on the configured SoftHSM2 token; the gate is the \
             operator's handshake that this is the right token to mutate)."
        );
        return None;
    }
    let module_path = env::var("ATLAS_HSM_PKCS11_LIB").ok().map(std::path::PathBuf::from);
    let slot = env::var("ATLAS_HSM_SLOT").ok().and_then(|s| s.parse::<u64>().ok());
    let pin_file = env::var("ATLAS_HSM_PIN_FILE").ok().map(std::path::PathBuf::from);
    match (module_path, slot, pin_file) {
        (Some(m), Some(s), Some(p)) => {
            // Mirror `HsmConfig::from_env`'s absolute-path guard for
            // parity with the wave-2 byte-equivalence test (and the
            // production gate). CWD-swap defence: relative paths
            // resolve against the test runner's working directory at
            // load time, which is a library-hijack vector.
            if !m.is_absolute() {
                eprintln!(
                    "SKIP: ATLAS_HSM_PKCS11_LIB={} is not absolute — wave-3 \
                     integration test refuses relative paths for parity with \
                     HsmConfig::from_env (defends against CWD-swap library hijack).",
                    m.display()
                );
                return None;
            }
            if !p.is_absolute() {
                eprintln!(
                    "SKIP: ATLAS_HSM_PIN_FILE={} is not absolute — wave-3 \
                     integration test refuses relative paths for parity with \
                     HsmConfig::from_env (defends against CWD-swap PIN file substitution).",
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
                "SKIP: HSM trio (ATLAS_HSM_PKCS11_LIB, ATLAS_HSM_SLOT, \
                 ATLAS_HSM_PIN_FILE) not fully present even though \
                 {RUNTIME_GATE_ENV}=1. Either set the trio or unset the gate."
            );
            None
        }
    }
}

/// Destroy any leftover workspace key objects created by this test
/// (and any prior interrupted run with the same workspace_ids).
/// Opens its own RW session and reuses the production-hardened
/// `read_pin_file` so the cleanup credential read enforces the same
/// TOCTOU + Unix mode-0400 guard as production. Sharing the reader
/// instead of duplicating it pins the security policy to a single
/// code path — adding a future check (e.g. owner-uid match) lifts it
/// into the cleanup path automatically.
///
/// **Lifetime invariant:** must NOT run while a `Pkcs11WorkspaceSigner`
/// is alive in the same process. Both share the dlopen'd PKCS#11
/// library and a process-global `C_Initialize` / `C_Finalize`
/// lifecycle; an overlapping cleanup tears down the signer's session
/// when its `Pkcs11` drops. Each test brackets the signer scope so
/// cleanup runs only before-open and after-drop.
fn cleanup_test_workspaces(env: &RuntimeEnv) {
    let cfg = HsmConfig::from_env(|name| env_lookup(env, name))
        .expect("HsmConfig parse (cleanup)")
        .expect("HSM trio present (cleanup)");

    let pkcs11 = Pkcs11::new(&env.module_path).expect("PKCS#11 module load (cleanup)");
    // Tolerate `CKR_CRYPTOKI_ALREADY_INITIALIZED`: cleanup may run
    // immediately after a sibling test in the same binary that hasn't
    // dropped its `Pkcs11` yet (cargo test serializes tests in a
    // binary by default but not always with --test-threads). The
    // global C_Initialize is idempotent; treating already-init as
    // success is what the spec intends.
    match pkcs11.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK)) {
        Ok(()) => {}
        Err(Pkcs11Error::Pkcs11(RvError::CryptokiAlreadyInitialized, _)) => {}
        Err(e) => panic!("PKCS#11 C_Initialize (cleanup): {e}"),
    }
    let slot = pkcs11
        .get_slots_with_token()
        .expect("get_slots_with_token (cleanup)")
        .into_iter()
        .find(|s| s.id() == env.slot)
        .expect("configured slot present (cleanup)");
    let session = pkcs11.open_rw_session(slot).expect("open_rw_session (cleanup)");
    let pin = read_pin_file(&cfg)
        .expect("read PIN via production hardened reader (cleanup) — \
                 enforces TOCTOU-safe + Unix mode-0400 guard");
    session
        .login(UserType::User, Some(&pin))
        .expect("PKCS#11 C_Login (cleanup)");

    for ws in [WS_ALPHA, WS_BETA, WS_PERSIST] {
        let label = format!("{WORKSPACE_LABEL_PREFIX}{ws}");
        for class in [ObjectClass::PRIVATE_KEY, ObjectClass::PUBLIC_KEY] {
            let template = vec![
                Attribute::Class(class),
                Attribute::KeyType(KeyType::EC_EDWARDS),
                Attribute::Label(label.as_bytes().to_vec()),
            ];
            let handles = session
                .find_objects(&template)
                .expect("find_objects (cleanup)");
            for handle in handles {
                let _ = session.destroy_object(handle);
            }
        }
    }
    // Session drops here → C_CloseSession; pkcs11 drops after →
    // C_Finalize. Same drop-ordering invariant as the production
    // signer; if the test ever breaks the order it will surface as a
    // double-finalize / use-after-free in cryptoki, not silent breakage.
}
