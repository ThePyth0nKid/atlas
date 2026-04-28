//! Atlas Trust Core
//!
//! Portable verifier for Atlas verifiable knowledge graphs.
//! Compiles to: native (CLI), WASM (browser), library (server-side cross-check).
//!
//! Trust property: given the same trace and the same pinned pubkey-bundle,
//! every build of this crate must produce bit-identical verification output.
//! Determinism is the entire reason this crate exists.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod anchor;
pub mod ed25519;
pub mod cose;
pub mod ct;
pub mod hashchain;
pub mod pubkey_bundle;
pub mod trace_format;
pub mod verify;
pub mod error;

pub use error::{TrustError, TrustResult};
pub use trace_format::{
    AnchorEntry, AnchorKind, AtlasEvent, AtlasPayload, AtlasTrace, EventSignature, InclusionProof,
};
pub use verify::{VerifyOutcome, VerifyEvidence, VerifyOptions, verify_trace, verify_trace_with};
pub use pubkey_bundle::PubkeyBundle;

/// Schema version this build of the crate produces and accepts.
pub const SCHEMA_VERSION: &str = "atlas-trace-v1";

/// Build identity for reproducibility, derived from `Cargo.toml`.
/// Bumping the crate version is the canonical signal that verification
/// semantics may have changed.
pub const VERIFIER_VERSION: &str = concat!("atlas-trust-core/", env!("CARGO_PKG_VERSION"));
