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

pub mod agent_did;
pub mod anchor;
pub mod ed25519;
pub mod cose;
pub mod ct;
pub mod hashchain;
pub mod per_tenant;
pub mod pubkey_bundle;
pub mod trace_format;
pub mod verify;
pub mod witness;
pub mod error;

pub use error::{TrustError, TrustResult};
pub use trace_format::{
    AnchorBatch, AnchorChain, AnchorEntry, AnchorKind, AtlasEvent, AtlasPayload, AtlasTrace,
    EventSignature, InclusionProof, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
};
pub use verify::{VerifyOutcome, VerifyEvidence, VerifyOptions, verify_trace, verify_trace_with};
pub use pubkey_bundle::PubkeyBundle;
pub use anchor::{chain_head_for, ChainHeadHex, ANCHOR_CHAIN_DOMAIN};
pub use per_tenant::{parse_per_tenant_kid, per_tenant_kid_for, PER_TENANT_KID_PREFIX};
pub use agent_did::{agent_did_for, parse_agent_did, validate_agent_did, AGENT_DID_PREFIX};
pub use witness::{
    decode_chain_head, verify_witness_against_roster, verify_witnesses_against_roster,
    witness_signing_input, WitnessFailure, WitnessFailureReason, WitnessFailureWire,
    WitnessSig, WitnessVerifyOutcome, ATLAS_WITNESS_DOMAIN, ATLAS_WITNESS_V1_ROSTER,
    MAX_WITNESS_KID_LEN,
};

/// Schema version this build of the crate produces and accepts.
pub const SCHEMA_VERSION: &str = "atlas-trace-v1";

/// Build identity for reproducibility, derived from `Cargo.toml`.
/// Bumping the crate version is the canonical signal that verification
/// semantics may have changed.
pub const VERIFIER_VERSION: &str = concat!("atlas-trust-core/", env!("CARGO_PKG_VERSION"));
