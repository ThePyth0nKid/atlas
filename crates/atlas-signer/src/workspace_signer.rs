//! V1.11 Scope A wave-3 — per-workspace signing abstraction.
//!
//! Wave 1 ([`crate::keys::DEV_MASTER_SEED`]) and wave 2
//! ([`crate::hsm::pkcs11::Pkcs11MasterSeedHkdf`]) drove the
//! *master seed* into a sealed HSM container. Wave 3 (this module)
//! moves the **per-workspace Ed25519 private key** itself into the
//! sealed container — the goal is that for an HSM-backed deployment,
//! no per-tenant secret bytes ever reach Atlas address space, even
//! transiently. Today (Phase A), this module ships:
//!
//!   * The [`WorkspaceSigner`] trait — sign + pubkey, indexed by
//!     `workspace_id`. The two ops are the entire surface the binary's
//!     `run_sign` / `run_derive_pubkey` flows need from a per-tenant
//!     key store. Constraining the trait this tightly is the point:
//!     a sealed-key impl (Phase B) can satisfy this surface without
//!     ever exposing the secret scalar, because every legitimate
//!     consumer of the per-tenant key currently asks for one of these
//!     two operations.
//!
//!   * The [`DevWorkspaceSigner`] impl — wraps a
//!     [`MasterSeedHkdf`](crate::keys::MasterSeedHkdf) and delegates
//!     to the existing
//!     [`derive_workspace_signing_key_via`](crate::keys::derive_workspace_signing_key_via)
//!     leaf. This is the V1.9 / V1.10 path expressed in wave-3
//!     terminology. The HKDF-derived per-tenant 32-byte input is
//!     scrubbed by the underlying leaf via `Zeroizing<[u8; 32]>`;
//!     the `SigningKey` itself has dalek's own zeroize-on-drop
//!     semantics. The dev impl produces byte-identical signatures
//!     and pubkeys to the V1.10 hot path — that property is asserted
//!     against the V1.10 pinned-pubkey goldens in this file's
//!     `tests` module.
//!
//! ## Why a separate trait, not just `MasterSeedHkdf`?
//!
//! `MasterSeedHkdf::derive_for` returns 32 bytes of HKDF output —
//! material that an in-process caller still has to feed into
//! `SigningKey::from_bytes` to get a signature. A sealed-key impl
//! that satisfies *that* trait would still have to leak the
//! per-tenant scalar across the trust boundary in order to be useful.
//! `WorkspaceSigner` collapses the two operations the caller
//! actually needs (sign, pubkey) into the trait surface, so the
//! sealed-key Phase-B impl can keep the per-tenant scalar inside the
//! HSM and only return signatures + verifying-key bytes — both of
//! which are public material by definition.
//!
//! ## Phase A scope
//!
//! Phase A introduces the trait and the dev impl ONLY. The
//! Phase-B PKCS#11 impl ([`Pkcs11WorkspaceSigner`], TODO) and the
//! Phase-C `run_sign` dispatcher wire-up ship as separate commits so
//! each landing is reviewable independently. Until Phase C lands, the
//! binary continues to call `derive_workspace_signing_key_via`
//! directly — so this module is dead code from `main.rs`'s
//! perspective, but lib-API consumers (and the test suite) can
//! exercise it today.

use std::sync::Arc;

use ed25519_dalek::Signer;

use crate::keys::{
    derive_workspace_signing_key_via, validate_workspace_id, MasterSeedError, MasterSeedHkdf,
};

#[cfg(test)]
use crate::keys::DevMasterSeedHkdf;

/// Per-workspace Ed25519 signing surface.
///
/// Implementations are the boundary between Atlas's per-tenant key
/// management and whatever backing store actually holds the keys —
/// in-process HKDF for dev/legacy, sealed PKCS#11 token for production
/// (Phase B). The trait surface is deliberately tiny: the two
/// operations every legitimate consumer of a per-tenant key needs to
/// perform, and nothing else. A sealed impl satisfies this surface
/// without ever exposing the secret scalar.
///
/// **`Send + Sync`.** Required so the binary can hold the dispatcher
/// inside an `Arc` and pass it across async tasks (V1.10's MCP-server
/// pattern). The trait is also dyn-safe (no generics, no `Self`
/// returns, no `async fn`) so the dispatcher can return
/// `Box<dyn WorkspaceSigner>` and select the dev-vs-sealed impl at
/// runtime.
///
/// **Errors.** [`WorkspaceSignerError`] mirrors
/// [`MasterSeedError`](crate::keys::MasterSeedError)'s three
/// categories (locked, unavailable, derive/sign failed) plus a fresh
/// `SigningFailed` variant that the Phase-B PKCS#11 impl will use to
/// surface PKCS#11 RV codes from `C_Sign`. The dev impl propagates
/// only the categories that `derive_workspace_signing_key_via` can
/// produce.
///
/// **Implementor contract — workspace_id validation.** Implementations
/// MUST call [`validate_workspace_id`](crate::keys::validate_workspace_id)
/// on `workspace_id` BEFORE issuing any key-material operation, and
/// MUST return [`WorkspaceSignerError::DeriveFailed`] (with the
/// validation error string as the payload) on rejection. Without this
/// check the trait becomes a bypass vector for the V1.9 workspace_id
/// hardening (DoS via 10 MB workspace_id, Unicode confusables across
/// tenants, control-character injection): `main.rs` validates at the
/// CLI parse layer, but the lib's `workspace_signer` module is part
/// of the `pub` surface and other crates calling it directly would
/// otherwise hit the unchecked HKDF leaf. The stock dev impl
/// ([`DevWorkspaceSigner`]) honours this contract; the Phase-B
/// PKCS#11 impl will too.
///
/// **Implementor contract — internal caching.** Each `sign` and
/// `pubkey` call is independently consistent (same inputs ⇒ same
/// outputs). The trait makes NO promise to callers about whether
/// implementations memoise across calls — that decision is per-impl
/// and per-deployment. **However**, sealed-key impls SHOULD cache
/// per-workspace key handles internally: the dev impl's per-call
/// HKDF-Expand-32 is cheap (sub-µs), but a naive Phase-B impl that
/// re-fetches the PKCS#11 key handle on every `sign` would pay
/// O(PKCS#11-RTT × per-tenant-call-frequency) — a session round-trip
/// per signature, which is enough to tank an MCP-server's per-second
/// throughput on a network-attached HSM. The Phase-B impl will
/// keep a `Mutex<HashMap<workspace_id, ObjectHandle>>` on the impl
/// struct and look up handles in O(1) after the first derive. Caller
/// code can rely on this without holding a separate cache itself.
pub trait WorkspaceSigner: Send + Sync {
    /// Sign `signing_input` with the per-workspace Ed25519 secret key
    /// for `workspace_id` and return the 64-byte raw RFC 8032
    /// signature.
    ///
    /// **Determinism.** Ed25519 signatures are deterministic
    /// (RFC 8032 §5.1.6) — two calls with the same `workspace_id` and
    /// identical `signing_input` produce byte-identical 64-byte
    /// outputs. The dev impl honours this directly via
    /// `ed25519_dalek::SigningKey::sign`; a Phase-B HSM impl honours
    /// it via PKCS#11 `CKM_EDDSA` (PKCS#11 v3.0 §6.5.5 mandates
    /// deterministic signing).
    fn sign(
        &self,
        workspace_id: &str,
        signing_input: &[u8],
    ) -> Result<[u8; 64], WorkspaceSignerError>;

    /// Return the 32-byte raw Ed25519 verifying-key bytes for
    /// `workspace_id`.
    ///
    /// Pure public material. The byte layout matches
    /// `ed25519_dalek::VerifyingKey::to_bytes` (the encoded curve
    /// point in compressed form, RFC 8032 §5.1.5). Encoders that need
    /// base64url-no-pad (e.g. `PubkeyBundle.keys`) apply the encoding
    /// at the consumer side; this surface stays format-neutral.
    fn pubkey(&self, workspace_id: &str) -> Result<[u8; 32], WorkspaceSignerError>;
}

/// Error returned by [`WorkspaceSigner`] operations.
///
/// **Variant cleaving rule (read before adding).** The four variants
/// split along an *operator-remediation* axis, not along a
/// transport-layer axis. Two errors that demand different operator
/// actions get different variants; two errors that demand the same
/// remediation collapse into one variant even if they originate in
/// different PKCS#11 functions:
///
/// * `Locked` → operator action: re-authenticate / supply PIN.
/// * `Unavailable` → operator action: fix deployment config (driver
///   path, slot id, network reachability).
/// * `DeriveFailed` → the per-workspace key cannot be produced.
///   Operator action: ensure the workspace_id is provisioned, or
///   generate the key. Both PKCS#11 `C_FindObjects` (key absent) and
///   `C_GenerateKeyPair` (generation refused) map here — same
///   remediation either way.
/// * `SigningFailed` → the per-workspace key exists and was usable,
///   but the signing operation itself failed (PKCS#11 `C_Sign` /
///   `C_SignFinal`). Often transient (HSM session expired, internal
///   retry needed). Different remediation from `DeriveFailed`:
///   retry vs. provision.
///
/// **Implementor obligation.** Pick the variant that matches the
/// remediation, not the call site. A `C_Sign` failure caused by an
/// underlying key-handle invalidation is `SigningFailed` (operator
/// retries); a key-not-found surfaced from inside a sign call is
/// `DeriveFailed` (operator provisions). When in doubt, `SigningFailed`
/// is the safer default for a sign() path because it implies "retry
/// is meaningful."
///
/// **Dev impl scope.** The dev impl ([`DevWorkspaceSigner`]) can only
/// produce `Locked`, `Unavailable`, and `DeriveFailed` — propagated
/// via the [`From<MasterSeedError>`] conversion or the workspace_id
/// validation guard. `SigningFailed` is structurally unreachable in
/// the dev path because in-process Ed25519 signing via
/// `ed25519-dalek` is infallible.
///
/// `#[non_exhaustive]` so future sealed-key backends can add
/// granular variants (e.g. `KeyRevoked`, `RateLimited`) without a
/// SemVer break of downstream consumers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WorkspaceSignerError {
    /// Sealed-key store reachable but locked. Operator-recoverable
    /// by re-authenticating (login, supply PIN, refresh session).
    #[error("workspace signer locked: {0}")]
    Locked(String),
    /// Sealed-key store unreachable. Operator-recoverable by fixing
    /// deployment config (driver path wrong, token slot empty,
    /// network HSM down, missing dependency).
    #[error("workspace signer unavailable: {0}")]
    Unavailable(String),
    /// Per-workspace key cannot be produced (not provisioned, not
    /// derivable, generation refused). Operator-recoverable by
    /// provisioning the workspace_id or fixing the derivation
    /// pipeline. The dev impl surfaces the (impossible)
    /// HKDF-Expand-too-long case here AND the workspace_id
    /// validation refusal. Phase-B HSM impl surfaces
    /// `CKR_KEY_HANDLE_INVALID` from `C_FindObjects` and
    /// `CKR_*` errors from `C_GenerateKeyPair` here.
    #[error("workspace key derivation failed: {0}")]
    DeriveFailed(String),
    /// Signing operation failed against a usable key. Operator
    /// remediation is "retry" (or "page someone if it persists"),
    /// not "provision a key." Reserved for Phase-B HSM impl
    /// (PKCS#11 `C_Sign` / `C_SignFinal` returning `CKR_*`); the
    /// dev impl never produces this variant because in-process
    /// Ed25519 signing is infallible.
    #[error("workspace signing operation failed: {0}")]
    SigningFailed(String),
}

impl From<MasterSeedError> for WorkspaceSignerError {
    /// Propagate [`MasterSeedError`] failures from the underlying
    /// HKDF leaf into the [`WorkspaceSigner`] surface. The mapping
    /// is straight-through for `Locked` / `Unavailable`; the
    /// `DeriveFailed` variant absorbs the (impossible-in-practice)
    /// HKDF-Expand failure mode the dev impl can theoretically
    /// produce.
    ///
    /// **Exhaustive, deliberately.** `MasterSeedError` is
    /// `#[non_exhaustive]` for downstream-crate consumers, but
    /// `workspace_signer` lives in the same crate, so the
    /// non_exhaustive marker is a no-op here. A wildcard arm would
    /// be flagged unreachable today AND would silently swallow any
    /// new variant added in a future commit, mapping it to a
    /// generic `DeriveFailed`. Exhaustive matching forces a compile
    /// error when a new variant lands, surfacing the mapping gap in
    /// code review where the categorisation can be made
    /// deliberately.
    fn from(err: MasterSeedError) -> Self {
        match err {
            MasterSeedError::Locked(s) => WorkspaceSignerError::Locked(s),
            MasterSeedError::Unavailable(s) => WorkspaceSignerError::Unavailable(s),
            MasterSeedError::DeriveFailed(s) => WorkspaceSignerError::DeriveFailed(s),
        }
    }
}

/// V1.9-compatible [`WorkspaceSigner`] backed by an in-process HKDF
/// leaf.
///
/// Wraps an [`Arc<dyn MasterSeedHkdf>`]: the dispatcher in Phase C
/// will hand the same `Arc` produced by
/// [`crate::keys::master_seed_loader`] to either this impl
/// ([`DevMasterSeedHkdf`] inside the Arc) or — once V1.11 Scope A
/// byte-equivalence is confirmed —
/// [`Pkcs11MasterSeedHkdf`](crate::hsm::pkcs11::Pkcs11MasterSeedHkdf)
/// inside the Arc, with the same `WorkspaceSigner` impl on the outside
/// (because the HKDF-then-from-bytes dance gives byte-identical
/// pubkeys regardless of where the seed lives). The Phase-B
/// `Pkcs11WorkspaceSigner` is a sibling impl for the case where the
/// per-workspace key itself is sealed — that path requires the HSM to
/// own `C_Sign` end-to-end.
///
/// **Why `Arc<dyn MasterSeedHkdf>`, not generic over `H`?** Phase C's
/// dispatcher returns `Box<dyn WorkspaceSigner>` — a trait object. A
/// trait object cannot be parameterised over a generic type, so the
/// concrete struct underneath must use erased storage (`Arc<dyn
/// MasterSeedHkdf>`) or a `Box`. `Arc` over `Box` because the binary's
/// signing path may eventually share the same loader across multiple
/// async tasks (V1.10's MCP server pattern); cloning an `Arc` is free,
/// cloning a `Box` is not.
pub struct DevWorkspaceSigner {
    hkdf: Arc<dyn MasterSeedHkdf>,
}

impl DevWorkspaceSigner {
    /// Construct a [`DevWorkspaceSigner`] over a caller-owned HKDF
    /// leaf. Phase-C dispatcher takes the `Arc<dyn MasterSeedHkdf>`
    /// returned by [`crate::keys::master_seed_loader`] and feeds it
    /// here (with `Arc::from(box_hkdf)` to convert the Box into an
    /// Arc).
    pub fn new(hkdf: Arc<dyn MasterSeedHkdf>) -> Self {
        Self { hkdf }
    }

    /// Test-only convenience: build a [`DevWorkspaceSigner`] backed
    /// by [`DevMasterSeedHkdf`](crate::keys::DevMasterSeedHkdf) (the
    /// source-committed [`DEV_MASTER_SEED`](crate::keys::DEV_MASTER_SEED)).
    ///
    /// **Why `#[cfg(test)]`?** The function bypasses the V1.10
    /// [`master_seed_gate`](crate::keys::master_seed_gate) positive-
    /// opt-in check. Exposing it from non-test builds would let a
    /// downstream lib consumer (or an internal binary refactor) wire
    /// the dev seed into a production signing path silently — the
    /// exact failure mode V1.10 wave-1 inverted to defeat. The
    /// Phase-C dispatcher uses [`DevWorkspaceSigner::new`] paired
    /// with [`crate::keys::master_seed_loader`] instead, which means
    /// no production code path needs this convenience. Confining it
    /// to `#[cfg(test)]` makes the "no production caller" property
    /// type-system-enforced rather than convention-enforced.
    #[cfg(test)]
    pub(crate) fn with_dev_seed() -> Self {
        Self::new(Arc::new(DevMasterSeedHkdf))
    }
}

impl WorkspaceSigner for DevWorkspaceSigner {
    fn sign(
        &self,
        workspace_id: &str,
        signing_input: &[u8],
    ) -> Result<[u8; 64], WorkspaceSignerError> {
        // Trait contract: validate workspace_id BEFORE issuing any
        // key-material operation. `validate_workspace_id` enforces
        // the V1.9 hardening (non-empty, ASCII-only, no colons /
        // whitespace / controls, ≤ WORKSPACE_ID_MAX_BYTES). Without
        // this guard, a downstream lib consumer that didn't go
        // through the binary's CLI parse layer could feed an
        // unbounded or attacker-shaped workspace_id straight into
        // the HKDF leaf. Map the validator's `Err(String)` into
        // `DeriveFailed` because the validation failure aborts the
        // derive step before any key material is touched — same
        // category as a downstream HKDF refusal.
        validate_workspace_id(workspace_id)
            .map_err(WorkspaceSignerError::DeriveFailed)?;
        // `derive_workspace_signing_key_via` zeroizes the 32-byte
        // HKDF output internally (`Zeroizing<[u8; 32]>`); the
        // returned `SigningKey` carries dalek's own zeroize-on-drop.
        // We sign in this scope and let the `SigningKey` drop at
        // function exit so the secret scalar's lifetime is bounded
        // by the call.
        let signing_key = derive_workspace_signing_key_via(&*self.hkdf, workspace_id)?;
        let signature = signing_key.sign(signing_input);
        Ok(signature.to_bytes())
    }

    fn pubkey(&self, workspace_id: &str) -> Result<[u8; 32], WorkspaceSignerError> {
        // Same workspace_id contract as `sign`. The pubkey path is
        // public-material-out, but an unbounded workspace_id still
        // costs an HKDF-Expand and a PKCS#11 round-trip in the
        // Phase-B impl — defending against that DoS surface lives
        // here, before the derive call.
        validate_workspace_id(workspace_id)
            .map_err(WorkspaceSignerError::DeriveFailed)?;
        // Same derive-then-extract pattern as `sign`. The
        // verifying-key bytes are pure public material; we still
        // route through `derive_workspace_signing_key_via` (rather
        // than caching) because the per-tenant key set is sized by
        // the active workspace count and the dev path's per-derive
        // cost is a single HKDF-Expand-32. Caching becomes relevant
        // for the Phase-B HSM impl where each pubkey extract is a
        // PKCS#11 `C_GetAttributeValue` round-trip; that caching
        // lives in the Phase-B impl, not here.
        let signing_key = derive_workspace_signing_key_via(&*self.hkdf, workspace_id)?;
        Ok(signing_key.verifying_key().to_bytes())
    }
}

#[cfg(test)]
mod tests {
    //! Phase-A coverage: prove that the dev impl is a faithful
    //! re-expression of the V1.9 / V1.10 hot path. The acceptance
    //! criteria for "faithful" are:
    //!
    //!   * Same pubkey bytes as `derive_workspace_signing_key_default`
    //!     for every workspace_id (anchored against the V1.10
    //!     pinned-pubkey goldens — see
    //!     `keys::tests::workspace_pubkeys_are_pinned`).
    //!   * Deterministic: two `sign` calls with identical inputs
    //!     produce byte-identical 64-byte outputs (RFC 8032 §5.1.6).
    //!   * Round-trip: a signature produced by `sign` verifies under
    //!     the pubkey produced by `pubkey` (catches any future change
    //!     that breaks the SigningKey ↔ VerifyingKey relationship).
    //!
    //! These tests are the regression fence that lets the Phase-C
    //! dispatcher swap the dev impl for the sealed impl with
    //! confidence — if the swap rotates a per-workspace pubkey, that
    //! is a property failure, and these tests AND
    //! `keys::tests::workspace_pubkeys_are_pinned` will both fire.

    use super::*;
    use crate::keys::{derive_workspace_signing_key, DEV_MASTER_SEED};
    use ed25519_dalek::{Signature, VerifyingKey};

    #[test]
    fn pubkey_matches_v1_9_derivation_for_dev_seed() {
        // The dev WorkspaceSigner MUST produce byte-identical pubkeys
        // to the V1.9 explicit-seed derivation — that is the
        // backwards-compat property that lets the Phase-C dispatcher
        // swap call sites without rotating any production pubkey.
        let signer = DevWorkspaceSigner::with_dev_seed();
        for ws in ["alice", "bob", "ws-mcp-default", "Customer_42"] {
            let from_signer = signer.pubkey(ws).expect("dev impl is infallible");
            let expected = derive_workspace_signing_key(&DEV_MASTER_SEED, ws)
                .verifying_key()
                .to_bytes();
            assert_eq!(
                from_signer, expected,
                "DevWorkspaceSigner::pubkey({ws:?}) drifted from V1.9 derivation",
            );
        }
    }

    #[test]
    fn sign_is_deterministic() {
        // RFC 8032 §5.1.6: Ed25519 signatures are deterministic. Two
        // sign() calls on the same (workspace_id, signing_input) pair
        // MUST produce byte-identical 64-byte outputs. Catches any
        // future change that swaps in a randomised signing scheme
        // (e.g. someone "upgrading" to ed25519ph or RFC 8032 batch
        // mode without realising those have different signature
        // shapes).
        let signer = DevWorkspaceSigner::with_dev_seed();
        let msg = b"v1.11 wave-3 phase-a determinism witness";
        let a = signer.sign("alice", msg).expect("dev impl is infallible");
        let b = signer.sign("alice", msg).expect("dev impl is infallible");
        assert_eq!(a, b, "Ed25519 signatures must be deterministic");
    }

    #[test]
    fn sign_then_verify_round_trip() {
        // Sanity round-trip: signature produced by sign() verifies
        // under the pubkey produced by pubkey(). Catches any future
        // change that breaks the SigningKey ↔ VerifyingKey
        // relationship without tripping a higher-level integration
        // test (e.g. an accidental swap of `verifying_key()` for a
        // freshly-derived independent key).
        let signer = DevWorkspaceSigner::with_dev_seed();
        let msg = b"v1.11 wave-3 phase-a round-trip witness";
        let sig_bytes = signer.sign("alice", msg).expect("dev impl is infallible");
        let pub_bytes = signer.pubkey("alice").expect("dev impl is infallible");

        let signature = Signature::from_bytes(&sig_bytes);
        let verifying_key =
            VerifyingKey::from_bytes(&pub_bytes).expect("32-byte ed25519 pubkey");
        verifying_key
            .verify_strict(msg, &signature)
            .expect("sign→pubkey round-trip must verify under verify_strict");
    }

    #[test]
    fn different_workspaces_yield_different_pubkeys() {
        // Defence-in-depth fence for the Phase-A surface: two
        // different workspace_ids MUST produce independent pubkeys.
        // A collision would mean the WorkspaceSigner trait surface
        // accidentally degenerates to a constant — the kind of bug
        // that's invisible in single-workspace smoke tests but
        // catastrophic in production.
        let signer = DevWorkspaceSigner::with_dev_seed();
        let alice = signer.pubkey("alice").expect("dev impl is infallible");
        let bob = signer.pubkey("bob").expect("dev impl is infallible");
        assert_ne!(
            alice, bob,
            "alice and bob must derive independent verifying keys",
        );
    }

    #[test]
    fn signature_does_not_verify_under_other_workspaces_pubkey() {
        // Negative round-trip: a signature made for "alice" MUST NOT
        // verify under "bob"'s pubkey. This is the property that
        // gives V1.9 per-tenant isolation operationally meaningful
        // — without this fence, a degenerate impl that returned the
        // same SigningKey for every workspace_id would pass every
        // other test in this file (each is workspace-local).
        let signer = DevWorkspaceSigner::with_dev_seed();
        let msg = b"v1.11 wave-3 phase-a cross-tenant isolation witness";
        let sig_bytes = signer.sign("alice", msg).expect("dev impl is infallible");
        let bob_pub = signer.pubkey("bob").expect("dev impl is infallible");

        let signature = Signature::from_bytes(&sig_bytes);
        let bob_key =
            VerifyingKey::from_bytes(&bob_pub).expect("32-byte ed25519 pubkey");
        assert!(
            bob_key.verify_strict(msg, &signature).is_err(),
            "alice's signature must NOT verify under bob's pubkey — \
             cross-tenant isolation regression",
        );
    }

    #[test]
    fn pubkey_is_pinned_against_v1_10_goldens() {
        // Anchor the Phase-A dev impl against the V1.10 pinned
        // pubkeys (see `keys::tests::workspace_pubkeys_are_pinned`).
        // If this test trips alongside that one, the underlying HKDF
        // derivation rotated and both pin sets need a coordinated
        // bump (and a `VERIFIER_VERSION` bump in atlas-trust-core).
        // If THIS test trips alone, the WorkspaceSigner trait
        // surface drifted from the V1.10 hot path — Phase-C
        // dispatcher swap would silently rotate production pubkeys.
        use base64::Engine;

        let signer = DevWorkspaceSigner::with_dev_seed();
        let pubkey_b64 = |ws: &str| -> String {
            let bytes = signer.pubkey(ws).expect("dev impl is infallible");
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        };

        // Pins live in `crate::test_support` so `keys::tests` and
        // `workspace_signer::tests` reference a single source of
        // truth. A drift between the two test sites would mask a
        // production pubkey rotation; sharing the constant makes
        // drift impossible at compile time.
        assert_eq!(
            pubkey_b64("alice"),
            crate::test_support::PINNED_PUBKEY_B64URL_ALICE,
        );
        assert_eq!(
            pubkey_b64("ws-mcp-default"),
            crate::test_support::PINNED_PUBKEY_B64URL_WS_MCP_DEFAULT,
        );
    }

    #[test]
    fn arc_dyn_constructor_yields_same_pubkeys_as_with_dev_seed() {
        // The Phase-C dispatcher will call `DevWorkspaceSigner::new(
        // Arc::from(box_hkdf))` where `box_hkdf` came from
        // `master_seed_loader`. Confirm that path produces
        // byte-identical output to the convenience constructor —
        // catches any future change to the field layout that breaks
        // the Arc-based wiring (e.g. a refactor that switches `hkdf`
        // to a generic and breaks `Arc::from(Box<dyn ...>)`
        // coercion).
        let dev_path = DevWorkspaceSigner::with_dev_seed();
        let arc_path: DevWorkspaceSigner = {
            let boxed: Box<dyn MasterSeedHkdf> = Box::new(DevMasterSeedHkdf);
            DevWorkspaceSigner::new(Arc::from(boxed))
        };
        for ws in ["alice", "bob", "ws-mcp-default"] {
            assert_eq!(
                dev_path.pubkey(ws).expect("dev impl is infallible"),
                arc_path.pubkey(ws).expect("dev impl is infallible"),
                "Arc<dyn MasterSeedHkdf> path must be byte-equivalent \
                 to with_dev_seed for {ws:?}",
            );
        }
    }

    #[test]
    fn cross_workspace_signatures_diverge_for_same_input() {
        // Closes the last logical gap in the cross-tenant isolation
        // story: pubkeys-differ is necessary but not sufficient.
        // This test asserts that signatures over the SAME message
        // differ between two workspace_ids, so a degenerate impl
        // that returned independent pubkeys but a single shared
        // SigningKey for sign() (e.g. a Phase-B caching bug) would
        // still trip CI. Without this, the cross-tenant property is
        // only proved transitively (different pubkeys ⇒ different
        // scalars ⇒ different signatures); explicit assertion is
        // cheaper than the transitive argument and catches the
        // direct-implementation regression class.
        let signer = DevWorkspaceSigner::with_dev_seed();
        let msg = b"v1.11 wave-3 phase-a cross-tenant signature witness";
        let alice_sig = signer.sign("alice", msg).expect("dev impl is infallible");
        let bob_sig = signer.sign("bob", msg).expect("dev impl is infallible");
        assert_ne!(
            alice_sig, bob_sig,
            "alice and bob must produce different signatures over identical input \
             — cross-tenant isolation regression",
        );
    }

    #[test]
    fn sign_rejects_invalid_workspace_id() {
        // V1.11 wave-3 implementor contract: workspace_id MUST be
        // validated before any key-material operation. The dev impl
        // routes through `validate_workspace_id` and surfaces the
        // refusal as `WorkspaceSignerError::DeriveFailed`. Catches
        // a future refactor that drops the guard call.
        let signer = DevWorkspaceSigner::with_dev_seed();
        let msg = b"any input";
        for bad in ["", "ws:colon", "ws with space", "ws\nlf", "Büro"] {
            let err = signer
                .sign(bad, msg)
                .expect_err(&format!("workspace_id {bad:?} must be rejected"));
            match err {
                WorkspaceSignerError::DeriveFailed(_) => { /* expected */ }
                other => panic!(
                    "workspace_id {bad:?} rejection must use DeriveFailed; got {other:?}",
                ),
            }
        }
    }

    #[test]
    fn pubkey_rejects_invalid_workspace_id() {
        // Same contract as sign(): pubkey() validates before
        // derive. The pubkey path is public-out, but unbounded /
        // attacker-shaped workspace_id still costs HKDF + (in
        // Phase-B) a PKCS#11 round-trip — defending the DoS surface
        // here means Phase B inherits the guard for free.
        let signer = DevWorkspaceSigner::with_dev_seed();
        for bad in ["", "ws:colon", "ws\tcontrol", "café"] {
            let err = signer
                .pubkey(bad)
                .expect_err(&format!("workspace_id {bad:?} must be rejected"));
            match err {
                WorkspaceSignerError::DeriveFailed(_) => { /* expected */ }
                other => panic!(
                    "workspace_id {bad:?} rejection must use DeriveFailed; got {other:?}",
                ),
            }
        }
    }

    #[test]
    fn workspace_signer_is_dyn_safe() {
        // Compile-time fence: WorkspaceSigner MUST be dyn-safe so the
        // Phase-C dispatcher can return Box<dyn WorkspaceSigner>. A
        // future refactor that adds a generic method or `Self`-typed
        // return to the trait would break this assignment with an
        // E0038 (the trait cannot be made into an object).
        let _signer: Box<dyn WorkspaceSigner> =
            Box::new(DevWorkspaceSigner::with_dev_seed());
    }
}
