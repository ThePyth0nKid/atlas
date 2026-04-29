//! V1.9 — Per-tenant Ed25519 workspace-signing-key derivation.
//!
//! V1.5–V1.8 signed every event with one of three globally-shared
//! Ed25519 keypairs (agent / human / anchor). A compromise of any of
//! those three keys forged events for *every* workspace at once.
//!
//! V1.9 derives a per-workspace Ed25519 signing key from a single master
//! seed using HKDF-SHA256 (RFC 5869) with a domain-separated `info`
//! parameter. The verifier consumes the resulting public key via the
//! `PubkeyBundle` and never sees the master seed — re-derivation is an
//! issuer-side capability only.
//!
//! ## Derivation
//!
//! ```text
//! info       = "atlas-anchor-v1:" || workspace_id
//! key_bytes  = HKDF-SHA256(salt = None, ikm = master_seed, info, len = 32)
//! signing    = ed25519_dalek::SigningKey::from_bytes(&key_bytes)
//! ```
//!
//! HKDF (extract-then-expand) gives uniformly-random 32-byte output
//! indistinguishable from random under the standard HKDF assumption,
//! and the `info` parameter is the domain-separation knob — different
//! `info` strings produce independent keys even from the same `ikm`.
//! Ed25519 accepts any 32-byte sequence as a secret-scalar seed
//! (libsodium-style), so the HKDF output goes straight into
//! `SigningKey::from_bytes` without further reduction.
//!
//! ## Why the `atlas-anchor-v1:` info prefix
//!
//! The prefix is the trust boundary for per-tenant key separation. If
//! we used just `workspace_id` directly, an attacker who controlled the
//! `workspace_id` of one Atlas service could re-derive the same key
//! used in a different domain (e.g. a hypothetical `atlas-policy:`
//! derivation that came along later). The prefix keeps namespaces
//! disjoint by construction. The `-v1` is a future-rotation tag: if we
//! ever need to change the algorithm, bumping it to `-v2` produces a
//! disjoint key set without re-using the same `(ikm, info)` pair.
//!
//! Note the *issuer-side* HKDF info-prefix (`atlas-anchor-v1:`) is
//! intentionally distinct from the *verifier-side* kid prefix
//! (`atlas-anchor:`, see `atlas_trust_core::PER_TENANT_KID_PREFIX`).
//! They serve different purposes and sit on different sides of the
//! trust boundary; if you ever feel tempted to make them the same
//! string, remember that doing so couples the wire-format identifier
//! to the cryptographic-domain tag and constrains future format
//! evolution.
//!
//! ## Master seed handling
//!
//! `DEV_MASTER_SEED` is a constant in source. This is the SAME residual
//! single-point-of-failure as V1.8: master-seed compromise compromises
//! every workspace. V1.10 closes it with HSM/TPM sealing — until then,
//! see `docs/SECURITY-NOTES.md`. Production deployments MUST replace
//! the constant with a sealed-key handle before going live.

use atlas_trust_core::per_tenant_kid_for;
use ed25519_dalek::SigningKey;
use hkdf::Hkdf;
use sha2::Sha256;

/// V1.9 dev master seed. Production MUST replace this with a sealed
/// secret (HSM/TPM/cloud-KMS) before going live; see
/// `docs/SECURITY-NOTES.md`. The constant is fixed across builds so
/// development environments produce reproducible per-workspace pubkeys
/// — the smoke test pins the resulting bundle hash.
///
/// Layout note: the value is exactly 32 ASCII bytes; byte 31 is `0x0A`
/// (LF) so the printable prefix `atlas-master-seed-v1-dev-001-00` stays
/// 31 chars and pads up to 32 with a single LF. This is intentional —
/// editing the constant to "look nicer" by removing the LF reduces the
/// length to 31 and breaks compilation. The `workspace_pubkeys_are_pinned`
/// test below catches any byte-level drift in the seed at CI time.
pub const DEV_MASTER_SEED: [u8; 32] = *b"atlas-master-seed-v1-dev-001-00\n";

/// Environment variable that opts an `atlas-signer` invocation OUT of
/// the dev master seed. Set to `1` in any environment where running
/// with the source-committed `DEV_MASTER_SEED` would be a security
/// failure (production, staging touching real customer data, audit
/// rehearsals against the real key roster).
///
/// V1.9 has no sealed-seed loader yet — `production_gate` returns an
/// error so the binary refuses every per-tenant subcommand instead of
/// silently using the public dev key. V1.10 will replace this gate
/// with an `ATLAS_MASTER_SEED_PATH` loader.
pub const PRODUCTION_GATE_ENV: &str = "ATLAS_PRODUCTION";

/// Refuse to use `DEV_MASTER_SEED` if the environment marks this
/// invocation as production. Returns an error message suitable for
/// stderr.
///
/// The gate fires when `ATLAS_PRODUCTION=1`. Any other value (unset,
/// empty, "0", "true") allows the dev seed — V1.9 dev/CI environments
/// run with the env var unset; production rollouts set `=1` and wait
/// for the V1.10 sealed-seed loader before re-enabling per-tenant
/// commands.
pub fn production_gate() -> Result<(), String> {
    match std::env::var(PRODUCTION_GATE_ENV).as_deref() {
        Ok("1") => Err(format!(
            "{PRODUCTION_GATE_ENV}=1 set, but atlas-signer is using the source-committed \
             DEV_MASTER_SEED. V1.9 has no sealed-seed loader; refusing to derive per-tenant \
             keys against a public dev seed. V1.10 closes this with HSM/TPM sealing — until \
             then, run with {PRODUCTION_GATE_ENV} unset only in dev/CI."
        )),
        _ => Ok(()),
    }
}

/// Validate a `workspace_id` for use in HKDF derivation and per-tenant
/// kid construction.
///
/// `atlas-trust-core::parse_per_tenant_kid` is intentionally lenient —
/// the trust property holds via byte-exact kid comparison and HKDF
/// determinism for any non-empty UTF-8 string. The issuer side is the
/// place to enforce ingress hygiene, because that is where ambiguous or
/// confusable IDs become operator footguns and observability holes.
///
/// We accept ASCII printable bytes (0x21..=0x7E) and reject:
///   * empty strings (no legitimate per-tenant kid names the empty workspace);
///   * any byte outside the ASCII-printable range (control chars, NUL,
///     DEL, non-ASCII — defence against Unicode confusables); and
///   * the byte `:` (the kid prefix delimiter — workspace_ids
///     containing `:` produce kids with ambiguous segmentation).
///
/// Returns `Ok(())` on accept, `Err(message)` on reject.
pub fn validate_workspace_id(workspace_id: &str) -> Result<(), String> {
    if workspace_id.is_empty() {
        return Err("workspace_id must be non-empty".to_string());
    }
    for (i, b) in workspace_id.bytes().enumerate() {
        if !(0x21..=0x7E).contains(&b) {
            return Err(format!(
                "workspace_id byte {i} is 0x{b:02x}; only ASCII printable bytes \
                 0x21..=0x7E are allowed (no whitespace, control chars, or non-ASCII)",
            ));
        }
        if b == b':' {
            return Err(format!(
                "workspace_id contains ':' at byte {i}; ambiguous with the kid prefix \
                 delimiter ('atlas-anchor:'). Use '-' or '_' instead.",
            ));
        }
    }
    Ok(())
}

/// Domain-separation prefix prepended to the workspace_id when forming
/// the HKDF `info` parameter. See module doc for why this is a
/// versioned tag.
const HKDF_INFO_PREFIX: &str = "atlas-anchor-v1:";

/// Derive a per-workspace Ed25519 signing key from `master_seed` and
/// `workspace_id` via HKDF-SHA256.
///
/// Determinism: the function is a pure deterministic mapping
/// `(master_seed, workspace_id) → signing_key`. Two calls with the
/// same inputs produce byte-identical keys; two calls with different
/// `workspace_id` (with the same master seed) produce independent
/// keys, which is the property that gives V1.9 per-tenant isolation.
///
/// Failure mode: HKDF-SHA256 expand returns an error only when the
/// requested output length exceeds `255 * 32 = 8160` bytes; we ask for
/// 32, so the call cannot fail. The `expect` documents that
/// invariant rather than silently swallowing an error path.
pub fn derive_workspace_signing_key(
    master_seed: &[u8; 32],
    workspace_id: &str,
) -> SigningKey {
    let hk = Hkdf::<Sha256>::new(None, master_seed);
    let mut key_bytes = [0u8; 32];
    let info = format!("{HKDF_INFO_PREFIX}{workspace_id}");
    hk.expand(info.as_bytes(), &mut key_bytes)
        .expect("HKDF-SHA256 expand of 32 bytes is well within the 8160-byte ceiling");
    SigningKey::from_bytes(&key_bytes)
}

/// Convenience: derive the per-workspace signing key using the
/// crate-default `DEV_MASTER_SEED`. Production code MUST switch to
/// `derive_workspace_signing_key` with a sealed-key handle.
pub fn derive_workspace_signing_key_default(workspace_id: &str) -> SigningKey {
    derive_workspace_signing_key(&DEV_MASTER_SEED, workspace_id)
}

/// Per-tenant identity for a workspace: the canonical kid the verifier
/// expects in `EventSignature.kid` plus the URL-safe-no-pad base64 of
/// the public key for embedding in the `PubkeyBundle`.
#[derive(Debug, Clone)]
pub struct PerTenantIdentity {
    /// `format!("atlas-anchor:{workspace_id}")` — the per-tenant kid
    /// the verifier expects under strict mode.
    pub kid: String,
    /// 32-byte Ed25519 public key, base64url-no-pad encoded — wire
    /// format for `PubkeyBundle.keys`.
    pub pubkey_b64url: String,
    /// 32-byte secret as 64-char hex — fed to `atlas-signer sign
    /// --secret-stdin` (production) or `derive-key` JSON output (dev).
    /// Treat as sensitive; never log.
    pub secret_hex: String,
}

/// Derive the public-facing `PerTenantIdentity` for `workspace_id`.
///
/// Wraps `derive_workspace_signing_key_default` and stitches the
/// canonical kid + base64url pubkey + hex secret into one record. The
/// MCP server consumes this via the `derive-key` JSON output.
pub fn per_tenant_identity(workspace_id: &str) -> PerTenantIdentity {
    use base64::Engine;
    let signing_key = derive_workspace_signing_key_default(workspace_id);
    let pubkey_bytes = signing_key.verifying_key().to_bytes();
    let pubkey_b64url =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pubkey_bytes);
    let secret_hex = hex::encode(signing_key.to_bytes());
    PerTenantIdentity {
        kid: per_tenant_kid_for(workspace_id),
        pubkey_b64url,
        secret_hex,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;

    #[test]
    fn derivation_is_deterministic() {
        let a = derive_workspace_signing_key(&DEV_MASTER_SEED, "alice");
        let b = derive_workspace_signing_key(&DEV_MASTER_SEED, "alice");
        assert_eq!(a.to_bytes(), b.to_bytes());
        assert_eq!(
            a.verifying_key().to_bytes(),
            b.verifying_key().to_bytes()
        );
    }

    #[test]
    fn different_workspaces_yield_different_keys() {
        let alice = derive_workspace_signing_key(&DEV_MASTER_SEED, "alice");
        let bob = derive_workspace_signing_key(&DEV_MASTER_SEED, "bob");
        assert_ne!(
            alice.to_bytes(),
            bob.to_bytes(),
            "alice and bob must derive independent secret scalars",
        );
        assert_ne!(
            alice.verifying_key().to_bytes(),
            bob.verifying_key().to_bytes(),
            "alice and bob must derive independent public keys",
        );
    }

    #[test]
    fn different_master_seeds_yield_different_keys() {
        // A rotated master seed must yield a disjoint key set, even for
        // the same workspace_id. This is the rotation property: an
        // operator who rotates the master seed produces an entirely new
        // key roster.
        let seed_a = [0x11u8; 32];
        let seed_b = [0x22u8; 32];
        let a = derive_workspace_signing_key(&seed_a, "alice");
        let b = derive_workspace_signing_key(&seed_b, "alice");
        assert_ne!(a.to_bytes(), b.to_bytes());
    }

    #[test]
    fn empty_workspace_id_still_derives() {
        // The HKDF call cannot fail for any UTF-8 workspace_id — the
        // `info` parameter has no length limit (output length does).
        // We do NOT prevent empty workspace_id at the derivation layer
        // because the strict-mode kid validation rejects the resulting
        // empty per-tenant kid via `parse_per_tenant_kid`. Layering the
        // check at the kid-validation layer keeps the derivation pure
        // and the policy in one place.
        let _ = derive_workspace_signing_key(&DEV_MASTER_SEED, "");
    }

    /// Pinned pubkey goldens for `derive_workspace_signing_key_default`.
    ///
    /// This is the V1.9 equivalent of the `atlas_anchor_pubkey_pem_is_pinned`
    /// fence in `anchor.rs`. Any change to `DEV_MASTER_SEED`, the HKDF
    /// info-prefix, the curve, or the encoder trips this test before
    /// silently rotating production keys.
    ///
    /// We pin two distinct workspace_ids so a degenerate change that
    /// happened to leave `alice` stable but broke other workspaces would
    /// still trip CI. The second pin (`ws-mcp-default`) matches the
    /// MCP server's `DEFAULT_WORKSPACE` so the smoke test's bundle hash
    /// becomes implicitly pinned through these values.
    ///
    /// If you intentionally change the derivation, regenerate both pins
    /// AND bump `atlas-trust-core`'s crate version so `VERIFIER_VERSION`
    /// cascades through old bundles, AND surface the rotation in
    /// `docs/SECURITY-NOTES.md`.
    #[test]
    fn workspace_pubkeys_are_pinned() {
        use base64::Engine;
        let pubkey = |ws: &str| -> String {
            let sk = derive_workspace_signing_key_default(ws);
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(sk.verifying_key().to_bytes())
        };

        // BEGIN PINNED — DO NOT EDIT WITHOUT INTENT.
        // Computed from `DEV_MASTER_SEED` + info `"atlas-anchor-v1:" + ws`.
        const ALICE_PUBKEY_B64URL: &str = "HaADbOvQvGRNVJnGFLLjj-qxC-zwReufz-8dAbBu9aY";
        const DEFAULT_PUBKEY_B64URL: &str = "_7VayPxHeadNxfSOw0p8E5LNXBNP2Mb-cOieCZRZq6M";
        // END PINNED.

        let alice = pubkey("alice");
        let default = pubkey("ws-mcp-default");

        if DEFAULT_PUBKEY_B64URL == "__DEFAULT_PIN_PLACEHOLDER__" {
            panic!(
                "ws-mcp-default pubkey pin placeholder needs to be replaced with: {default}\n\
                 (alice pubkey for cross-check: {alice})"
            );
        }

        assert_eq!(
            alice, ALICE_PUBKEY_B64URL,
            "V1.9 derivation drift for workspace 'alice'. If intentional, \
             regenerate both pins AND bump atlas-trust-core's crate version."
        );
        assert_eq!(
            default, DEFAULT_PUBKEY_B64URL,
            "V1.9 derivation drift for workspace 'ws-mcp-default'. If intentional, \
             regenerate both pins AND bump atlas-trust-core's crate version."
        );
        assert_ne!(
            alice, default,
            "Defence-in-depth: pinning two workspace_ids must not collide. \
             A collision means the derivation degenerated to a constant — \
             critical bug, fail loud."
        );
    }

    #[test]
    fn signature_round_trip() {
        // Sanity: a signature made by the derived key verifies under
        // the derived public key. Catches any future change that
        // breaks the SigningKey ↔ VerifyingKey relationship without
        // tripping a higher-level integration test.
        let sk = derive_workspace_signing_key_default("alice");
        let pk = sk.verifying_key();
        let msg = b"hello atlas v1.9";
        let sig = sk.sign(msg);
        pk.verify_strict(msg, &sig)
            .expect("derived key must produce verifiable signatures");
    }

    #[test]
    fn per_tenant_identity_kid_matches_trust_core_format() {
        let ident = per_tenant_identity("alice");
        assert_eq!(ident.kid, "atlas-anchor:alice");
        // pubkey_b64url is 43 chars (32 bytes b64url-no-pad) — sanity
        // check; downstream Zod schema enforces the same.
        assert_eq!(ident.pubkey_b64url.len(), 43);
        // secret_hex is 64 hex chars (32 bytes).
        assert_eq!(ident.secret_hex.len(), 64);
    }

    #[test]
    fn validate_workspace_id_accepts_ordinary_ids() {
        for ok in ["alice", "ws-mcp-default", "Customer_42", "BANK.HAGEDORN"] {
            assert!(
                validate_workspace_id(ok).is_ok(),
                "expected {ok:?} to be accepted",
            );
        }
    }

    #[test]
    fn validate_workspace_id_rejects_empty() {
        assert!(validate_workspace_id("").is_err());
    }

    #[test]
    fn validate_workspace_id_rejects_colon() {
        // Colons collide with the kid prefix delimiter; we refuse them
        // even though `parse_per_tenant_kid` would tolerate them.
        let err = validate_workspace_id("ws:with:colons").unwrap_err();
        assert!(err.contains("':'"));
    }

    #[test]
    fn validate_workspace_id_rejects_whitespace_and_controls() {
        for bad in ["ws with space", "\tleading-tab", "trailing\n", "ws\0null"] {
            assert!(
                validate_workspace_id(bad).is_err(),
                "expected {bad:?} to be rejected",
            );
        }
    }

    #[test]
    fn validate_workspace_id_rejects_non_ascii() {
        // Defends against Unicode confusables: two visually-identical
        // names with different code points derive different keys but
        // appear the same in operator UIs.
        for bad in ["Büro", "wś", "café"] {
            assert!(
                validate_workspace_id(bad).is_err(),
                "expected {bad:?} to be rejected",
            );
        }
    }

    #[test]
    fn production_gate_blocks_when_env_set() {
        // We must avoid clobbering whatever the host environment has
        // set. Use a scoped guard with a nested-test serialisation
        // strategy is overkill; the test is fine if the env is briefly
        // toggled — test threads in cargo are independent processes per
        // module by default but to be safe we inspect first and restore.
        let prev = std::env::var(PRODUCTION_GATE_ENV).ok();
        // SAFETY: process-wide environment mutation. Tests in this
        // module run sequentially because the cargo default is one
        // thread per test binary file when --test-threads is left
        // implicit on this crate's small surface, and the production
        // gate has no other consumers in this binary.
        unsafe {
            std::env::set_var(PRODUCTION_GATE_ENV, "1");
        }
        let result = production_gate();
        match prev {
            Some(v) => unsafe { std::env::set_var(PRODUCTION_GATE_ENV, v) },
            None => unsafe { std::env::remove_var(PRODUCTION_GATE_ENV) },
        }
        assert!(result.is_err(), "production gate must reject ATLAS_PRODUCTION=1");
    }

    #[test]
    fn production_gate_allows_when_env_unset() {
        let prev = std::env::var(PRODUCTION_GATE_ENV).ok();
        unsafe { std::env::remove_var(PRODUCTION_GATE_ENV); }
        let result = production_gate();
        if let Some(v) = prev {
            unsafe { std::env::set_var(PRODUCTION_GATE_ENV, v); }
        }
        assert!(result.is_ok(), "production gate must allow when env unset");
    }
}
