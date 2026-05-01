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
/// The gate fires only on the byte-exact value `"1"`. Any other value
/// (unset, empty, `"0"`, `"true"`, `"yes"`, `"on"`, `"1 "` with
/// trailing whitespace, …) allows the dev seed — V1.9 dev/CI
/// environments run with the env var unset; production rollouts set
/// `=1` and wait for the V1.10 sealed-seed loader before re-enabling
/// per-tenant commands.
///
/// **Operator footgun (documented in `OPERATOR-RUNBOOK.md` §1):** the
/// strict-`"1"` recognition is a deployment trap waiting for someone
/// who reflexively writes `ATLAS_PRODUCTION=true` (a common K8s/Docker
/// idiom). That value silently falls through and the dev seed signs.
/// V1.10 will replace this gate with positive opt-in semantics; until
/// then, deploy automation must verify the env literally equals `1`.
///
/// Implementation: forwards to `production_gate_with(|name|
/// std::env::var(name).ok())`. Tests use the injection form to avoid
/// mutating the process environment from inside a parallel cargo
/// runner (cargo defaults to a thread pool, so `std::env::set_var`
/// between tests is a data race). The injection seam also foreshadows
/// V1.10: the sealed-seed loader will inject a seed source the same
/// way this gate injects an env source.
pub fn production_gate() -> Result<(), String> {
    production_gate_with(|name| std::env::var(name).ok())
}

/// Test-injection form of `production_gate`. Takes an env reader (a
/// pure closure mapping a variable name to its optional value) so test
/// code can drive the gate without touching the global process env.
///
/// Public-but-`pub(crate)`-spirit: keep this on the binary's surface
/// for future in-crate callers (the V1.10 HSM loader will reuse the
/// same injection style for its seed-source lookup) while signalling
/// that the gate is the canonical entry point — external embedders
/// should call `production_gate`, not this one.
pub fn production_gate_with<F>(env: F) -> Result<(), String>
where
    F: Fn(&str) -> Option<String>,
{
    match env(PRODUCTION_GATE_ENV).as_deref() {
        Some("1") => Err(format!(
            "{PRODUCTION_GATE_ENV}=1 set, but atlas-signer is using the source-committed \
             DEV_MASTER_SEED. V1.9 has no sealed-seed loader; refusing to derive per-tenant \
             keys against a public dev seed. V1.10 closes this with HSM/TPM sealing — until \
             then, run with {PRODUCTION_GATE_ENV} unset only in dev/CI."
        )),
        _ => Ok(()),
    }
}

/// Maximum byte length of a `workspace_id`. Bounding the input here
/// gives every downstream consumer a predictable size budget:
///
///   * the per-tenant kid (`atlas-anchor:` + workspace_id) is at most
///     `13 + 256 = 269` bytes, well under any realistic PKCS#11
///     `info`-parameter ceiling and PubkeyBundle key-name limit;
///   * a malicious or buggy caller cannot insert a 10 MB key into the
///     `PubkeyBundle` keys map (cheap-DoS surface flagged in V1.9
///     review); and
///   * V1.10's HSM derive call gets a definite upper bound on the
///     `info` material it has to forward to the device.
///
/// 256 bytes is intentionally generous (UUIDs, structured tenant
/// names like `bank-hagedorn:prod:eu-west`, even hash-prefixed names
/// fit comfortably) but small enough to fail loudly on accidentally-
/// large input. If a real-world deployment needs longer IDs, raise the
/// constant deliberately and bump documentation; do not hot-patch
/// callers around it.
pub const WORKSPACE_ID_MAX_BYTES: usize = 256;

/// Validate a `workspace_id` for use in HKDF derivation and per-tenant
/// kid construction.
///
/// `atlas-trust-core::parse_per_tenant_kid` is intentionally lenient —
/// the trust property holds via byte-exact kid comparison and HKDF
/// determinism for any non-empty UTF-8 string. The issuer side is the
/// place to enforce ingress hygiene, because that is where ambiguous or
/// confusable IDs become operator footguns and observability holes.
///
/// We accept ASCII printable bytes (0x21..=0x7E) up to
/// `WORKSPACE_ID_MAX_BYTES` and reject:
///   * empty strings (no legitimate per-tenant kid names the empty workspace);
///   * strings longer than `WORKSPACE_ID_MAX_BYTES` (cheap DoS surface);
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
    if workspace_id.len() > WORKSPACE_ID_MAX_BYTES {
        return Err(format!(
            "workspace_id is {} bytes; the cap is {WORKSPACE_ID_MAX_BYTES}. Real-world \
             tenant identifiers are short — a longer value usually indicates an upstream \
             bug or an attempt to insert an oversized PubkeyBundle entry.",
            workspace_id.len(),
        ));
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
///
/// `pub` because V1.10's HSM-backed loader (in a sibling crate) needs
/// to assemble the same `info` string before submitting an HKDF derive
/// call to the device. Re-implementing the prefix in the HSM crate
/// would split the cryptographic-domain tag across two source-of-truth
/// sites — a drift surface the V1.9 design intentionally avoided.
///
/// **Future-prefix invariant.** Any new domain prefix added later
/// (e.g. `atlas-policy-v1:`, `atlas-witness-v1:`) MUST NOT produce an
/// `info` string that is a prefix of any pre-existing one for any
/// workspace_id. Two info strings where one is a prefix of the other
/// produce different HKDF outputs (HKDF-Expand reads the info bytes
/// verbatim with no separator), but the safety margin is conceptual:
/// ensure the namespaces are visibly disjoint at the literal-prefix
/// level so a careful reader can rule out collisions by inspection.
/// Bumping the `-vN` suffix when changing the algorithm is the
/// standard rotation hatch.
pub const HKDF_INFO_PREFIX: &str = "atlas-anchor-v1:";

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
///
/// `pub(crate)` so the dispatch gate in `main.rs` is the only entry
/// point. V1.10 will route per-tenant secret material through a
/// `MasterSeedHkdf` trait; consumers outside this crate should never
/// have a way to bypass the gate by calling the dev-seed convenience
/// directly.
pub(crate) fn derive_workspace_signing_key_default(workspace_id: &str) -> SigningKey {
    derive_workspace_signing_key(&DEV_MASTER_SEED, workspace_id)
}

/// Per-tenant identity for a workspace: the canonical kid the verifier
/// expects in `EventSignature.kid` plus the URL-safe-no-pad base64 of
/// the public key for embedding in the `PubkeyBundle`.
///
/// `Debug` is implemented manually below so accidental `dbg!()` or
/// `tracing` calls do not print `secret_hex` to logs.
#[derive(Clone)]
pub(crate) struct PerTenantIdentity {
    /// `format!("atlas-anchor:{workspace_id}")` — the per-tenant kid
    /// the verifier expects under strict mode.
    pub(crate) kid: String,
    /// 32-byte Ed25519 public key, base64url-no-pad encoded — wire
    /// format for `PubkeyBundle.keys`.
    pub(crate) pubkey_b64url: String,
    /// 32-byte secret as 64-char hex — fed to `atlas-signer sign
    /// --secret-stdin` (production) or `derive-key` JSON output (dev).
    /// Treat as sensitive; never log. The `Debug` impl below redacts
    /// this field; do not derive `Debug` automatically.
    pub(crate) secret_hex: String,
}

/// Manual `Debug` impl that redacts `secret_hex`. The derived `Debug`
/// would print the raw 64-char hex in any `dbg!(identity)` or
/// `tracing::debug!(?identity)` site — exactly the leak path V1.9
/// security review flagged. We keep `kid` and `pubkey_b64url` visible
/// because they are public material and useful for diagnostics.
impl std::fmt::Debug for PerTenantIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PerTenantIdentity")
            .field("kid", &self.kid)
            .field("pubkey_b64url", &self.pubkey_b64url)
            .field("secret_hex", &"<redacted>")
            .finish()
    }
}

/// Derive the public-facing `PerTenantIdentity` for `workspace_id`.
///
/// Wraps `derive_workspace_signing_key_default` and stitches the
/// canonical kid + base64url pubkey + hex secret into one record. The
/// MCP server consumes this via the `derive-key` JSON output.
///
/// `pub(crate)` for the same reason as `derive_workspace_signing_key_default`:
/// the binary's CLI dispatch is the only legitimate caller, and V1.10
/// adds a sibling `atlas-signer-hsm` crate that will provide its own
/// gated entry point rather than reaching into this one.
pub(crate) fn per_tenant_identity(workspace_id: &str) -> PerTenantIdentity {
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
    fn validate_workspace_id_accepts_at_max_length() {
        // The cap is inclusive: a workspace_id of exactly
        // `WORKSPACE_ID_MAX_BYTES` bytes is accepted. One byte beyond
        // the cap is rejected (see next test).
        let at_max = "a".repeat(WORKSPACE_ID_MAX_BYTES);
        assert!(
            validate_workspace_id(&at_max).is_ok(),
            "expected workspace_id of exactly {WORKSPACE_ID_MAX_BYTES} bytes to be accepted",
        );
    }

    #[test]
    fn validate_workspace_id_rejects_oversized() {
        // One byte beyond the cap. The DoS surface flagged in V1.9
        // review was a 10 MB workspace_id producing a 10 MB
        // PubkeyBundle entry; we test the boundary because the
        // boundary is what catches off-by-one regressions, not a 10 MB
        // string in CI.
        let too_long = "a".repeat(WORKSPACE_ID_MAX_BYTES + 1);
        let err = validate_workspace_id(&too_long).unwrap_err();
        assert!(
            err.contains("cap"),
            "error must mention the cap so an operator can grep for it; got {err:?}",
        );
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

    /// Helper: build an env reader that returns `value` for `key` and
    /// `None` for everything else. Keeps the test bodies tight and
    /// makes the data-vs-policy separation visible.
    fn env_with(key: &'static str, value: Option<&'static str>) -> impl Fn(&str) -> Option<String> {
        move |name| {
            if name == key { value.map(|s| s.to_string()) } else { None }
        }
    }

    // The pre-V1.10-warm-up tests for this gate flipped the process env
    // via `unsafe { std::env::set_var }`. Cargo defaults to a thread
    // pool per test binary, so two such tests racing in the same
    // module corrupt each other's view of the variable. The injection
    // form below is race-free and also pre-builds the V1.10 seam: when
    // the HSM loader needs to inject its own seed source, it will use
    // the same closure pattern.

    #[test]
    fn production_gate_blocks_when_env_set_to_one() {
        let result = production_gate_with(env_with(PRODUCTION_GATE_ENV, Some("1")));
        assert!(result.is_err(), "production gate must reject ATLAS_PRODUCTION=1");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("DEV_MASTER_SEED"),
            "error must name the constant the operator needs to grep for; got {msg:?}",
        );
    }

    #[test]
    fn production_gate_allows_when_env_unset() {
        let result = production_gate_with(env_with(PRODUCTION_GATE_ENV, None));
        assert!(result.is_ok(), "production gate must allow when env unset");
    }

    #[test]
    fn production_gate_allows_when_env_empty() {
        // Empty value is treated identically to unset — only the
        // literal "1" trips the gate. V1.9 design choice: a
        // misconfigured pipeline that emits `ATLAS_PRODUCTION=` (with
        // no value) should not silently behave like production-gated;
        // it should behave like dev. V1.10 inverts this entirely
        // (positive opt-in), so this test pins the V1.9 behaviour
        // before the inversion lands.
        let result = production_gate_with(env_with(PRODUCTION_GATE_ENV, Some("")));
        assert!(result.is_ok(), "empty ATLAS_PRODUCTION must not trip the V1.9 gate");
    }

    #[test]
    fn production_gate_allows_truthy_strings_other_than_one() {
        // Conservative-by-V1.9-design: only "1" trips the gate. The
        // OPERATOR-RUNBOOK warns prominently that "true", "yes", "on"
        // do NOT trip it today; documenting that contract via test so
        // a future "be liberal in what you accept" patch trips CI
        // before reaching production.
        for v in ["0", "true", "yes", "on", "production", "TRUE", "1 "] {
            let result = production_gate_with(env_with(PRODUCTION_GATE_ENV, Some(v)));
            assert!(
                result.is_ok(),
                "value {v:?} must not trip the V1.9 gate (only literal \"1\" does)",
            );
        }
    }

    #[test]
    fn production_gate_default_uses_process_env() {
        // Sanity: the public `production_gate()` is the
        // `production_gate_with(std::env::var)` wrapper. We don't
        // mutate the env here — we just confirm the wrapper compiles
        // and returns something. The actual policy is covered by the
        // injection-form tests above.
        let _ = production_gate();
    }

    #[test]
    fn per_tenant_identity_debug_redacts_secret() {
        // Defence: any `dbg!(identity)` or `tracing::debug!(?identity)`
        // site must NOT leak `secret_hex`. The manual `Debug` impl
        // emits "<redacted>" in place of the raw hex; we assert the
        // redacted marker is present AND the secret hex is absent.
        let identity = per_tenant_identity("alice");
        let dbg_out = format!("{identity:?}");
        assert!(
            dbg_out.contains("<redacted>"),
            "Debug must mark secret_hex as redacted; got {dbg_out:?}",
        );
        assert!(
            !dbg_out.contains(&identity.secret_hex),
            "Debug must NOT contain the raw secret_hex; got {dbg_out:?}",
        );
        // Public material stays visible for diagnostics:
        assert!(dbg_out.contains(&identity.kid));
        assert!(dbg_out.contains(&identity.pubkey_b64url));
    }
}
