//! Atlas Witness cosignature library (V1.13 Scope C wave 1).
//!
//! A witness is a second independent attestor that signs over the
//! recomputed anchor-chain head. The witness runs as its own binary
//! `atlas-witness` (operationally separate from `atlas-signer`) with its
//! own Ed25519 keypair — that operational separation is the entire
//! point. A witness in the same process as the signer would be "the
//! same machine signed twice", not an independent attestation.
//!
//! The `Witness` trait is dyn-safe (`Send + Sync`, no `async fn`,
//! `Result<_, String>` for a uniform dispatcher boundary) — same shape
//! as V1.11's `WorkspaceSigner` so future implementations (HSM-backed,
//! remote-service) slot into the same dispatcher pattern. (V1.11
//! footgun #20.)
//!
//! Trust property: a `WitnessSig` produced by a `Witness` whose pubkey
//! sits in `atlas_trust_core::ATLAS_WITNESS_V1_ROSTER` is evidence that
//! the named witness observed and attested the chain head. Failure to
//! verify (unknown kid, malformed sig, Ed25519 rejection) maps to
//! `TrustError::BadWitness` at the trust-core layer, distinct from
//! event-signature and Sigstore-anchor failures so auditor diagnostics
//! name the right trust domain.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub use atlas_trust_core::WitnessSig;

mod ed25519_witness;
pub use ed25519_witness::Ed25519Witness;

/// Witness trait: produces a [`WitnessSig`] over a given chain head.
///
/// Implementations sign with their own Ed25519 private key; the public
/// portion must already be registered in
/// `atlas_trust_core::ATLAS_WITNESS_V1_ROSTER` (under the `witness_kid`
/// the implementation reports) for the signature to verify on the
/// auditor's side.
///
/// Dyn-safe: no `async fn`, returns `Result<_, String>` for a uniform
/// error shape across the dispatcher boundary (mirrors V1.11
/// `WorkspaceSigner`).
pub trait Witness: Send + Sync {
    /// Witness identifier (kid) — must match the entry under which the
    /// public key is registered in `ATLAS_WITNESS_V1_ROSTER`.
    fn witness_kid(&self) -> &str;

    /// Sign over the canonical witness signing input
    /// (`ATLAS_WITNESS_DOMAIN || chain_head_bytes`).
    ///
    /// `chain_head_hex` is the verifier-visible 64-char hex form of the
    /// 32-byte `chain_head_for(batch)` digest; the implementation
    /// decodes to raw bytes internally before signing.
    fn sign_chain_head(&self, chain_head_hex: &str) -> Result<WitnessSig, String>;
}
