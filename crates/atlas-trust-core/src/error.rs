//! Error types. Single canonical error type so caller can match exhaustively.

use thiserror::Error;

/// Result alias used throughout the crate.
pub type TrustResult<T> = Result<T, TrustError>;

/// All ways verification can fail.
///
/// Marked `#[non_exhaustive]` so adding new failure modes in the verifier
/// is not a SemVer-breaking change for downstream `match` arms.
///
/// `Clone` is derived (V1.13 wave-C-2) so structured failure aggregates
/// like `WitnessVerifyOutcome.failures` can store owned copies of the
/// per-failure error without forcing callers to invent placeholder
/// variants. All current variants hold only `String`, so `Clone` is
/// shallow and SemVer-stable as new variants are added.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum TrustError {
    /// Schema version mismatch between trace and verifier build.
    #[error("schema mismatch: trace claims {trace}, verifier expects {expected}")]
    SchemaMismatch {
        /// schema_version in trace
        trace: String,
        /// schema_version this verifier build supports
        expected: String,
    },

    /// Pubkey not found in pinned bundle. Most common attack signal.
    #[error("unknown signing key: kid={0}")]
    UnknownKid(String),

    /// Pubkey bundle hash didn't match what trace claimed.
    /// Means trace was generated against a different pubkey set than we have.
    #[error("pubkey bundle mismatch: trace claims {claimed}, verifier has {actual}")]
    PubkeyBundleMismatch {
        /// hash claimed in trace
        claimed: String,
        /// hash of bundle this verifier was built with
        actual: String,
    },

    /// Ed25519 signature verification failed for an event.
    #[error("invalid signature for event {event_id}: {reason}")]
    BadSignature {
        /// event ULID
        event_id: String,
        /// underlying reason
        reason: String,
    },

    /// Recomputed event hash didn't match claimed hash.
    #[error("event hash mismatch for {event_id}: claimed={claimed}, computed={computed}")]
    HashMismatch {
        /// event ULID
        event_id: String,
        /// claimed hash
        claimed: String,
        /// recomputed hash
        computed: String,
    },

    /// Parent hash references an event that isn't in the trace.
    #[error("dangling parent for event {event_id}: parent {parent_hash} not in trace")]
    DanglingParent {
        /// child event ULID
        event_id: String,
        /// missing parent hash
        parent_hash: String,
    },

    /// COSE_Sign1 envelope failed to parse or had wrong shape.
    #[error("invalid COSE envelope for event {event_id}: {reason}")]
    BadCose {
        /// event ULID
        event_id: String,
        /// underlying reason
        reason: String,
    },

    /// Anchor inclusion proof did not verify against pinned Rekor pubkey.
    #[error("invalid anchor: {reason}")]
    BadAnchor {
        /// underlying reason
        reason: String,
    },

    /// V1.13 witness cosignature failed to verify (unknown kid in pinned
    /// roster, malformed signature bytes, or Ed25519 verification rejected).
    /// Distinguished from `BadSignature` (event-level) and `BadAnchor`
    /// (Sigstore inclusion-proof) so auditor-facing diagnostics name the
    /// right trust domain.
    #[error("invalid witness {witness_kid}: {reason}")]
    BadWitness {
        /// witness kid as it appeared on the wire
        witness_kid: String,
        /// underlying reason
        reason: String,
    },

    /// Two events in the trace share the same `event_hash`. Either an honest
    /// duplicate (which trace-builders should deduplicate before emit) or a
    /// replay-attack signal.
    #[error("duplicate event_hash {event_hash} appears twice in trace")]
    DuplicateEventHash {
        /// the colliding hash
        event_hash: String,
    },

    /// Timestamp did not parse as RFC 3339.
    #[error("invalid RFC 3339 timestamp '{ts}': {reason}")]
    BadTimestamp {
        /// the offending timestamp string
        ts: String,
        /// underlying parse-error reason
        reason: String,
    },

    /// Signature algorithm in event was not one we accept.
    #[error("unsupported signature alg '{alg}' for event {event_id} (V1 accepts only EdDSA)")]
    BadAlg {
        /// event ULID
        event_id: String,
        /// alg field as it appeared on the wire
        alg: String,
    },

    /// Generic deserialization error.
    #[error("deserialization error: {0}")]
    Deserialize(String),

    /// Generic encoding error.
    #[error("encoding error: {0}")]
    Encoding(String),
}

impl From<serde_json::Error> for TrustError {
    fn from(e: serde_json::Error) -> Self {
        TrustError::Deserialize(e.to_string())
    }
}
