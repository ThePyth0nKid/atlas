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

use atlas_trust_core::per_tenant_kid_for;

use crate::keys::{
    derive_workspace_signing_key_via, master_seed_loader_with_writer,
    validate_workspace_id, MasterSeedError, MasterSeedHkdf, PerTenantIdentity,
};

#[cfg(test)]
use crate::keys::DevMasterSeedHkdf;

/// V1.11 Scope A wave-3 — env-var name that opts a deployment into
/// the sealed per-workspace signer.
///
/// **Why a separate opt-in instead of "HSM trio implies wave-3"?**
/// Wave-2 dispatched on the trio: setting `ATLAS_HSM_PKCS11_LIB` /
/// `ATLAS_HSM_SLOT` / `ATLAS_HSM_PIN_FILE` activated the sealed-seed
/// loader. Wave-3 changes the per-tenant pubkey derivation: keys are
/// HSM-generated via `CKM_EC_EDWARDS_KEY_PAIR_GEN`, not derived from
/// a master seed via HKDF. The pubkeys are NOT byte-equivalent to the
/// V1.9–V1.10 derivation. A V1.10 wave-2 deployment that already
/// pinned per-tenant pubkeys (`PubkeyBundle.keys`) would silently
/// rotate every entry on upgrade if wave-3 activated automatically
/// with the trio. The opt-in is the operator's explicit handshake
/// that they accept the rotation event.
///
/// Recognised truthy values match
/// [`crate::keys::DEV_MASTER_SEED_OPT_IN_ENV`]: `1`, `true`, `yes`,
/// `on`, case-insensitive, with leading/trailing whitespace
/// tolerated. Anything else falls through to the wave-2 / dev path
/// (per-tenant keys derived from a master seed loaded by
/// [`crate::keys::master_seed_loader`]).
pub const WORKSPACE_HSM_OPT_IN_ENV: &str = "ATLAS_HSM_WORKSPACE_SIGNER";

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

/// V1.11 Scope A wave-3 Phase C — env-driven dispatcher that returns
/// either the sealed-key [`Pkcs11WorkspaceSigner`](crate::hsm::pkcs11_workspace::Pkcs11WorkspaceSigner)
/// (when [`WORKSPACE_HSM_OPT_IN_ENV`] is truthy AND the HSM trio is
/// set) or the [`DevWorkspaceSigner`] backed by the result of
/// [`crate::keys::master_seed_loader`] (preserving wave-2 sealed-seed
/// or dev-seed semantics).
///
/// **Dispatcher layering (top to bottom):**
///
///   1. **Wave-3 sealed per-workspace signer.** Activated by
///      `ATLAS_HSM_WORKSPACE_SIGNER=1` (truthy) AND the HSM trio
///      (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`,
///      `ATLAS_HSM_PIN_FILE`) being fully set. Per-tenant Ed25519
///      keys are generated and held inside the HSM; the private
///      scalar never enters Atlas address space.
///
///   2. **Wave-2 sealed-seed dev signer.** Activated when wave-3 is
///      NOT opted into AND the HSM trio is set. Per-tenant keys are
///      derived in-process via HKDF, but the master seed lives in
///      the HSM (wave-2 invariant preserved).
///
///   3. **Dev signer.** Activated when neither wave-3 nor the HSM
///      trio is set. The master seed is the source-committed
///      [`crate::keys::DEV_MASTER_SEED`], gated behind the wave-1
///      positive opt-in (`ATLAS_DEV_MASTER_SEED=1`).
///
/// **Why wave-3 needs explicit opt-in (not "trio implies wave-3").**
/// Wave-3 changes per-tenant pubkey derivation from
/// HKDF-of-master-seed (V1.9 / V1.10) to HSM-native key generation
/// (`CKM_EC_EDWARDS_KEY_PAIR_GEN`). The pubkeys are NOT
/// byte-equivalent. A V1.10 wave-2 deployment that already pinned
/// per-tenant pubkeys via `PubkeyBundle.keys` would silently rotate
/// every entry on first wave-3 sign. The explicit
/// `ATLAS_HSM_WORKSPACE_SIGNER=1` is the operator's handshake that
/// they accept the rotation. Once accepted, the operator runs
/// `derive-pubkey` / `rotate-pubkey-bundle` (which Phase C also wires
/// through this dispatcher) to advertise the fresh wave-3 pubkeys to
/// verifier-side trust pinning.
///
/// **Failure semantics — fail-closed at every layer.** If wave-3 is
/// opted in but the HSM trio is missing or partial, the dispatcher
/// refuses with a clear remediation hint instead of falling through
/// to the dev signer (silent fallthrough is the V1.10 anti-pattern).
/// If the HSM trio is set but the PKCS#11 module fails to open, the
/// dispatcher likewise refuses — the operator must fix the HSM
/// config or unset the wave-3 opt-in, not silently sign with the dev
/// key.
pub fn workspace_signer_loader() -> Result<Box<dyn WorkspaceSigner>, String> {
    workspace_signer_loader_with(|name| std::env::var(name).ok())
}

/// Test-injection form of [`workspace_signer_loader`]. Same env-reader
/// closure shape as [`crate::keys::master_seed_loader_with`] so a
/// single env source can drive both the wave-3 opt-in check, the HSM
/// trio parse (delegated to [`crate::hsm::config::HsmConfig::from_env`]),
/// and the wave-2 / dev-seed fallback.
///
/// Forwards to [`workspace_signer_loader_with_writer`] with
/// `std::io::stderr()` as the deprecation-warning sink (the warning
/// fires for `ATLAS_PRODUCTION` set, see
/// [`crate::keys::master_seed_loader_with_writer`]).
pub fn workspace_signer_loader_with<F>(env: F) -> Result<Box<dyn WorkspaceSigner>, String>
where
    F: Fn(&str) -> Option<String>,
{
    workspace_signer_loader_with_writer(env, &mut std::io::stderr())
}

/// V1.11 wave-3 Phase C entry point that takes both an env reader AND
/// a `&mut dyn Write` for the wave-2 deprecation warning sink.
/// Mirrors [`crate::keys::master_seed_loader_with_writer`] so wave-3
/// tests can capture the warning text for assertion (or pipe it to
/// [`std::io::sink`] when the test is exercising a deliberately-
/// deprecated configuration and the noise on stderr would clutter
/// the test runner output).
pub fn workspace_signer_loader_with_writer<F, W>(
    env: F,
    warn_out: &mut W,
) -> Result<Box<dyn WorkspaceSigner>, String>
where
    F: Fn(&str) -> Option<String>,
    W: std::io::Write,
{
    // V1.11 L-8: emit the ATLAS_PRODUCTION deprecation warning *before*
    // any gate check, so an operator who sets both `ATLAS_PRODUCTION=1`
    // and `ATLAS_HSM_WORKSPACE_SIGNER=1` still sees the V1.12-removal
    // notice. Without this call the wave-3 path would silently swallow
    // the warning when wave-3 is opted in (the wave-2
    // master_seed_loader_with_writer below is only reached on the
    // fallthrough branch).
    crate::keys::emit_atlas_production_deprecation_if_set(&env, warn_out);

    // Layer 1: wave-3 opt-in. Truthy values match the wave-1 dev-seed
    // gate's allow-list so an operator who learned `ATLAS_DEV_MASTER_SEED=1`
    // can use the same spelling here without surprise rejection.
    let opt_in_raw = env(WORKSPACE_HSM_OPT_IN_ENV).unwrap_or_default();
    let opt_in_normalised = opt_in_raw.trim().to_ascii_lowercase();
    let wave3_opt_in = matches!(opt_in_normalised.as_str(), "1" | "true" | "yes" | "on");

    if wave3_opt_in {
        // Wave-3 requires the HSM trio. `HsmConfig::from_env` returns
        // `Ok(None)` when no HSM env vars are set, `Ok(Some(_))` when
        // all three are set, and `Err(_)` for partial trios. We treat
        // `Ok(None)` (trio absent) as a fail-closed refusal here:
        // wave-3 opt-in without an HSM target is operator confusion
        // worth surfacing loudly, not a silent dev-seed fallback.
        let cfg = crate::hsm::config::HsmConfig::from_env(&env)?
            .ok_or_else(|| {
                format!(
                    "{WORKSPACE_HSM_OPT_IN_ENV}={opt_in_raw:?} requested the wave-3 \
                     sealed per-workspace signer, but the HSM trio is not set. \
                     Wave-3 has NO dev fallback — set ATLAS_HSM_PKCS11_LIB / \
                     ATLAS_HSM_SLOT / ATLAS_HSM_PIN_FILE (all three) to point at \
                     the production token, OR unset {WORKSPACE_HSM_OPT_IN_ENV} \
                     to fall through to the wave-2 sealed-seed signer (still HSM \
                     for the master seed; per-tenant keys derived in-process). \
                     See docs/OPERATOR-RUNBOOK.md §wave-3 for the migration."
                )
            })?;
        let pkcs11 = crate::hsm::pkcs11_workspace::Pkcs11WorkspaceSigner::open(cfg)
            .map_err(|e| format!("wave-3 HSM workspace signer open failed: {e}"))?;
        return Ok(Box::new(pkcs11));
    }

    // Layer 2: wave-2 / dev signer. The underlying
    // `master_seed_loader_with_writer` performs its own dispatch
    // between the sealed-seed loader (HSM trio set) and the dev gate
    // (HSM trio absent + dev opt-in). Wrap whatever it returns in
    // `DevWorkspaceSigner` so the binary's call sites see one trait
    // surface regardless of where the master seed actually lives.
    let hkdf = master_seed_loader_with_writer(&env, warn_out)?;
    Ok(Box::new(DevWorkspaceSigner::new(Arc::from(hkdf))))
}

/// V1.11 wave-3 Phase C — trait-routed counterpart of
/// [`crate::keys::per_tenant_identity_via`].
///
/// Asks the [`WorkspaceSigner`] for the workspace's pubkey (the
/// sealed-key impl reads `CKA_EC_POINT` from the on-token public-key
/// object; the dev impl derives it via HKDF) and stitches it into a
/// [`PerTenantIdentity`] alongside the canonical kid
/// (`atlas-anchor:` + workspace_id). The kid construction is
/// independent of the signing backend — it is the same
/// `per_tenant_kid_for(workspace_id)` regardless of whether the key
/// material lives in HSM or in-process.
///
/// Phase-C call sites (`derive-pubkey`, `rotate-pubkey-bundle`) route
/// through this helper instead of the [`per_tenant_identity_via`](crate::keys::per_tenant_identity_via)
/// HKDF-only form so that, when the wave-3 opt-in is active, the kid
/// is paired with the SEALED pubkey (i.e. what the wave-3 signer will
/// actually produce). Without this routing, the bundle would advertise
/// stale HKDF-derived pubkeys against which wave-3 signatures would
/// fail strict-mode verification.
pub fn per_tenant_identity_via_signer(
    signer: &dyn WorkspaceSigner,
    workspace_id: &str,
) -> Result<PerTenantIdentity, WorkspaceSignerError> {
    use base64::Engine;
    let pubkey_bytes = signer.pubkey(workspace_id)?;
    let pubkey_b64url =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pubkey_bytes);
    Ok(PerTenantIdentity {
        kid: per_tenant_kid_for(workspace_id),
        pubkey_b64url,
    })
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

    // V1.11 wave-3 Phase C — `workspace_signer_loader_with` dispatcher
    // tests. Pin the three-layer dispatch order (wave-3 opt-in → HSM
    // trio → dev gate) and the refusal semantics on the obvious
    // operator footguns. Each test runs against an injected env reader
    // (`crate::test_support::env_pairs`) so the suite never mutates
    // process environment — required because cargo runs tests in
    // parallel and an env mutation would race across cases.

    use crate::keys::DEV_MASTER_SEED_OPT_IN_ENV;
    use crate::test_support::env_pairs;

    /// Helper: extract the error message from a loader call that MUST
    /// have failed. Mirrors `keys::tests::loader_err` — `Box<dyn
    /// WorkspaceSigner>` does not implement `Debug` (the trait
    /// deliberately omits it), so `.unwrap_err()` won't compile.
    fn loader_err<F>(env: F) -> String
    where
        F: Fn(&str) -> Option<String>,
    {
        match workspace_signer_loader_with(env) {
            Ok(_) => panic!("expected loader to refuse, but it succeeded"),
            Err(e) => e,
        }
    }

    #[test]
    fn loader_dev_path_when_wave3_unset_and_dev_opt_in_set() {
        // Default-shape dev test: no wave-3 opt-in, no HSM trio, dev
        // opt-in set. Loader must succeed and produce a usable
        // [`WorkspaceSigner`]. Pubkey output must match the
        // [`DevWorkspaceSigner::with_dev_seed`] convenience constructor
        // byte-for-byte — that is the regression fence that lets the
        // dispatcher swap in wave-3 only when the operator opts in.
        let signer = workspace_signer_loader_with(env_pairs(&[(
            DEV_MASTER_SEED_OPT_IN_ENV,
            "1",
        )]))
        .expect("dev loader path must succeed");
        let baseline = DevWorkspaceSigner::with_dev_seed();
        for ws in ["alice", "bob", "ws-mcp-default"] {
            assert_eq!(
                signer.pubkey(ws).expect("dev impl is infallible"),
                baseline.pubkey(ws).expect("dev impl is infallible"),
                "loader-routed dev path must be byte-equivalent to with_dev_seed for {ws:?}",
            );
        }
    }

    #[test]
    fn loader_dev_path_when_wave3_opt_in_falsy() {
        // The wave-3 opt-in is falsy when set to anything outside
        // {1, true, yes, on}. A literal "no" or empty value MUST NOT
        // activate the wave-3 layer (operator confusion guard) — the
        // loader falls through to the dev path. Mirrors the wave-1
        // dev-seed gate's truthy/falsy parsing.
        for falsy in ["", "0", "no", "false", "off", "unknown"] {
            let signer = workspace_signer_loader_with(env_pairs(&[
                (WORKSPACE_HSM_OPT_IN_ENV, falsy),
                (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
            ]))
            .expect("falsy wave-3 opt-in must fall through to dev");
            // Smoke: the resulting signer must produce SOME pubkey for
            // a known-good workspace. Detailed byte-equivalence is
            // covered by the test above; this case is about the
            // dispatch-flow predicate.
            let _ = signer
                .pubkey("alice")
                .expect("dev impl is infallible");
        }
    }

    #[test]
    fn loader_refuses_wave3_opt_in_without_hsm_trio() {
        // Operator footgun #1: opt into wave-3 but forget to set the
        // HSM trio. The loader MUST refuse with a clear
        // remediation hint that names the env var (so an operator can
        // grep stderr for `ATLAS_HSM_WORKSPACE_SIGNER`). The dev
        // opt-in is also set in this case to prove that wave-3 does
        // NOT silently fall through to dev — the refusal is structural,
        // not a side-effect of dev being missing.
        let err = loader_err(env_pairs(&[
            (WORKSPACE_HSM_OPT_IN_ENV, "1"),
            (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
        ]));
        assert!(
            err.contains(WORKSPACE_HSM_OPT_IN_ENV),
            "wave-3 trio-missing error must name the opt-in env var; got {err:?}",
        );
        assert!(
            err.contains("HSM trio is not set"),
            "error must explain WHY (trio missing); got {err:?}",
        );
    }

    #[test]
    fn loader_refuses_wave3_opt_in_with_partial_hsm_trio() {
        // Operator footgun #2: partial HSM trio. The
        // [`HsmConfig::from_env`] propagates the partial-trio refusal
        // up through the loader; we just pin that the propagation
        // path is wired correctly (the wave-3 layer must NOT silently
        // accept a partial trio and continue).
        let err = loader_err(env_pairs(&[
            (WORKSPACE_HSM_OPT_IN_ENV, "1"),
            (
                crate::hsm::config::PKCS11_LIB_ENV,
                "/usr/lib/softhsm/libsofthsm2.so",
            ),
        ]));
        assert!(
            err.contains("partial"),
            "partial wave-3 trio must surface 'partial' from HsmConfig::from_env; got {err:?}",
        );
    }

    #[test]
    fn loader_truthy_wave3_opt_in_variants_all_activate_layer() {
        // The wave-3 opt-in's truthy spelling must match the wave-1
        // dev-seed gate's allow-list so an operator who learned one
        // can use the other without surprise. Catches a regression
        // where the wave-3 layer accidentally narrows to "1" only.
        // We assert via the trio-absent refusal: each truthy value
        // MUST hit the layer-1 refusal path (not the layer-2 dev
        // fallthrough), because that proves the opt-in was honoured.
        for truthy in ["1", "true", "yes", "on", "TRUE", "Yes", "  1  "] {
            let err = loader_err(env_pairs(&[
                (WORKSPACE_HSM_OPT_IN_ENV, truthy),
                (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
            ]));
            assert!(
                err.contains(WORKSPACE_HSM_OPT_IN_ENV),
                "truthy opt-in {truthy:?} must reach wave-3 refusal; got {err:?}",
            );
        }
    }

    #[test]
    fn loader_emits_atlas_production_deprecation_when_set_alongside_wave3_opt_in() {
        // V1.11 wave-3 + L-8 interaction: an operator who sets BOTH
        // `ATLAS_PRODUCTION=1` and `ATLAS_HSM_WORKSPACE_SIGNER=1`
        // must still see the V1.12-removal notice for `ATLAS_PRODUCTION`.
        // The wave-3 layer fires before the wave-2 / dev fallthrough,
        // so without the explicit `emit_atlas_production_deprecation_if_set`
        // call at the top of `workspace_signer_loader_with_writer`, the
        // warning would be silently swallowed on every wave-3 path —
        // including the trio-missing refusal path exercised here. We
        // pair the wave-3 opt-in with no-trio so the loader refuses; the
        // test asserts only on the warning text, not the loader result.
        let mut warnings = Vec::<u8>::new();
        let _ = workspace_signer_loader_with_writer(
            env_pairs(&[
                (crate::keys::PRODUCTION_GATE_ENV, "1"),
                (WORKSPACE_HSM_OPT_IN_ENV, "1"),
                (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
            ]),
            &mut warnings,
        );
        let text = String::from_utf8(warnings).expect("warning text must be UTF-8");
        assert!(
            text.contains(crate::keys::PRODUCTION_GATE_ENV),
            "wave-3 path must surface the ATLAS_PRODUCTION deprecation warning; got {text:?}",
        );
        assert!(
            text.contains("deprecated"),
            "warning must label the var as deprecated; got {text:?}",
        );
        assert!(
            text.contains("V1.12"),
            "warning must announce the removal target; got {text:?}",
        );
    }

    #[test]
    fn loader_no_warning_when_atlas_production_unset_under_wave3_opt_in() {
        // Negative twin of the test above: without `ATLAS_PRODUCTION`,
        // the wave-3 path produces no deprecation noise. Catches a
        // future refactor that accidentally hardcodes the warning to
        // fire on every wave-3 invocation.
        let mut warnings = Vec::<u8>::new();
        let _ = workspace_signer_loader_with_writer(
            env_pairs(&[
                (WORKSPACE_HSM_OPT_IN_ENV, "1"),
                (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
            ]),
            &mut warnings,
        );
        assert!(
            warnings.is_empty(),
            "no warning expected when ATLAS_PRODUCTION is unset; got {:?}",
            String::from_utf8_lossy(&warnings),
        );
    }

    #[test]
    fn loader_wave2_fallthrough_when_trio_set_but_wave3_unset() {
        // Wave-2 fallthrough invariant: when the HSM trio is set but
        // `ATLAS_HSM_WORKSPACE_SIGNER` is NOT opted in, the loader
        // MUST fall through to the wave-2 sealed-seed path (i.e. call
        // `master_seed_loader_with_writer`, which then routes through
        // `HsmConfig::from_env` + `Pkcs11MasterSeedHkdf::open`). We
        // can't exercise a real HSM open in a unit test, so we feed
        // bogus trio values that the wave-2 path will refuse — the
        // refusal proves the dispatcher routed through wave-2 rather
        // than the wave-3 layer or the dev gate.
        //
        // We use a relative `PKCS11_LIB_ENV` path because the wave-2
        // `HsmConfig::from_env` validator refuses relative paths
        // (library-hijack defence), and that refusal text is
        // platform-portable — unlike the lib-load failure text which
        // differs across libloading (Linux) vs LoadLibrary (Windows).
        // Either failure would prove wave-2 routing; pinning on the
        // absolute-path refusal keeps the test stable across hosts.
        let err = loader_err(env_pairs(&[
            (
                crate::hsm::config::PKCS11_LIB_ENV,
                "wave2-fallthrough-witness.so",
            ),
            (crate::hsm::config::SLOT_ENV, "0"),
            (
                crate::hsm::config::PIN_FILE_ENV,
                "wave2-fallthrough-pin",
            ),
            (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
        ]));
        // Positive: the error must name the wave-2 trio's PKCS#11-lib
        // env var, which only happens when the dispatcher reached
        // `HsmConfig::from_env` (called from `master_seed_loader_with_writer`
        // — i.e. the wave-2 layer). The wave-3 layer's trio-missing
        // refusal does NOT name this var; its trio-partial refusal
        // would, but we set all three trio vars here so partial-trio
        // is structurally not the cause.
        assert!(
            err.contains(crate::hsm::config::PKCS11_LIB_ENV),
            "wave-2 fallthrough must surface a wave-2 HSM-config error; got {err:?}",
        );
        // Negative: if the dispatcher mis-routed to the wave-3 layer,
        // its refusal text mentions `WORKSPACE_HSM_OPT_IN_ENV`. The
        // wave-2 path NEVER mentions that var (it doesn't read it), so
        // its absence proves wave-2 routing.
        assert!(
            !err.contains(WORKSPACE_HSM_OPT_IN_ENV),
            "wave-2 fallthrough error MUST NOT mention the wave-3 opt-in env var \
             (would mean the dispatcher mis-routed); got {err:?}",
        );
    }

    #[test]
    fn per_tenant_identity_via_signer_matches_keys_module_form_for_dev_path() {
        // The wave-3 trait-routed `per_tenant_identity_via_signer`
        // MUST produce byte-identical output to the master-seed-
        // routed `keys::per_tenant_identity_via` for the dev path —
        // that byte-equivalence is the property that lets the
        // `derive-pubkey` and `rotate-pubkey-bundle` ceremonies
        // dispatch through the wave-3 layer without rotating any
        // V1.10-pinned per-tenant pubkey. Catches a future refactor
        // that changes the kid construction or the base64 encoding
        // out from under the loader.
        let signer = DevWorkspaceSigner::with_dev_seed();
        for ws in ["alice", "bob", "ws-mcp-default"] {
            let via_signer = per_tenant_identity_via_signer(&signer, ws)
                .expect("dev impl is infallible");
            let via_keys = crate::keys::per_tenant_identity_via(&DevMasterSeedHkdf, ws)
                .expect("dev impl is infallible");
            assert_eq!(
                via_signer.kid, via_keys.kid,
                "kid must match per_tenant_identity_via for {ws:?}",
            );
            assert_eq!(
                via_signer.pubkey_b64url, via_keys.pubkey_b64url,
                "pubkey_b64url must match per_tenant_identity_via for {ws:?}",
            );
        }
    }
}
