//! Error types for the `atlas-projector` crate. Local to V2-α Welle 3
//! scope (canonicalisation only); does NOT alias or wrap
//! `atlas_trust_core::error::TrustError`. Future wellen that integrate
//! projection failures into the verifier's trust-chain reporting may
//! add conversion impls, but Welle 3 keeps the two error spaces
//! cleanly separated.

use thiserror::Error;

/// Result alias used throughout the crate.
pub type ProjectorResult<T> = Result<T, ProjectorError>;

/// All ways canonicalisation can fail.
///
/// Marked `#[non_exhaustive]` so adding new failure modes in future
/// V2-α wellen (event-replay, idempotent upsert, ArcadeDB sync) is
/// not a SemVer-breaking change for downstream `match` arms. Mirrors
/// `atlas_trust_core::error::TrustError` convention.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum ProjectorError {
    /// CBOR encoding produced an error during canonicalisation. Either
    /// a property value contained a float (rejected at the canonicaliser
    /// boundary; see `lib.rs` crate-doc invariant #3), or a per-level
    /// container exceeded the size cap.
    #[error("canonicalisation failed: {0}")]
    CanonicalisationFailed(String),

    /// The `author_did` field on a graph node or edge was present but
    /// did not parse against the `did:atlas:<lowercase-hex-32-bytes>`
    /// shape per V2-α Welle 1. Distinguished from
    /// `MalformedEntityUuid` so callers can surface a per-failure
    /// diagnostic.
    #[error("malformed agent-DID on graph element: {0}")]
    MalformedAuthorDid(String),

    /// The `entity_uuid` field on a graph node was empty or otherwise
    /// invalid. Welle 3 enforces minimal length-check only; richer
    /// format-validation may be added in a later welle.
    #[error("malformed entity_uuid: {0}")]
    MalformedEntityUuid(String),

    /// Two graph nodes shared the same `entity_uuid`. Either an
    /// honest issuer bug (multiple events claiming the same logical
    /// entity identity) or a projection-replay corruption signal.
    /// Either way the projector MUST refuse the upsert.
    ///
    /// **Welle 4 use:** raised by the idempotent-upsert layer that
    /// Welle 4 will add atop the Welle 3 skeleton. The current
    /// `GraphState::upsert_node` API returns `Option<previous>`
    /// without raising this error — that allows the Welle 4
    /// upsert-policy layer to distinguish "first sighting" vs
    /// "update existing" semantics. Welle 3 reserves the variant
    /// here so the `#[non_exhaustive]` enum's shape is stable
    /// across the upcoming Welle 4 addition.
    #[allow(dead_code)]
    #[error("duplicate entity_uuid in graph state: {entity_uuid}")]
    DuplicateNode {
        /// the colliding entity_uuid
        entity_uuid: String,
    },

    /// An edge referenced an endpoint (`from_entity` or `to_entity`)
    /// not present in the node set. Welle 3 enforces this at
    /// canonicalisation boundary as a structural-integrity gate.
    #[error("dangling edge {edge_id}: missing endpoint {missing_endpoint}")]
    DanglingEdge {
        /// the offending edge_id
        edge_id: String,
        /// the absent endpoint entity_uuid
        missing_endpoint: String,
    },
}
