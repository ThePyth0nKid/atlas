//! Atlas trace bundle format (V1).
//!
//! This is the wire-format that flows from Atlas-Server → Verifier (CLI/WASM/library).
//! It must be self-contained so that an air-gapped auditor can verify it.

use serde::{Deserialize, Serialize};

/// The full trace bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtlasTrace {
    /// Schema version, must match `atlas-trace-v1`.
    pub schema_version: String,

    /// ISO-8601 timestamp when this bundle was generated server-side.
    pub generated_at: String,

    /// Workspace identifier.
    pub workspace_id: String,

    /// blake3 hex hash of the pubkey-bundle the events were signed against.
    pub pubkey_bundle_hash: String,

    /// All signed events in this bundle.
    pub events: Vec<AtlasEvent>,

    /// Current DAG-tip hashes the server claims.
    /// Verifier computes the actual tips from `events` and may diff.
    pub dag_tips: Vec<String>,

    /// Anchor entries (Sigstore Rekor inclusion proofs).
    #[serde(default)]
    pub anchors: Vec<AnchorEntry>,

    /// Cedar policies in scope at bundle generation time.
    /// Each policy is itself an event in `events` — this is just the index.
    #[serde(default)]
    pub policies: Vec<String>,

    /// Optional filters applied when generating the bundle.
    #[serde(default)]
    pub filters: Option<TraceFilters>,
}

/// One signed event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtlasEvent {
    /// ULID for this event.
    pub event_id: String,

    /// blake3 hex hash of the canonical signing-input.
    pub event_hash: String,

    /// Parent event_hashes (zero or more — DAG).
    #[serde(default)]
    pub parent_hashes: Vec<String>,

    /// Application-level payload as JSON.
    pub payload: serde_json::Value,

    /// Ed25519 signature (V1 simplified format; V2: full COSE_Sign1).
    pub signature: EventSignature,

    /// ISO-8601 timestamp the signer claims.
    pub ts: String,
}

/// Simplified V1 signature wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSignature {
    /// Always "EdDSA" in V1.
    pub alg: String,
    /// Key-id: SPIFFE-ID, email, or other resolvable identifier.
    pub kid: String,
    /// Signature bytes, base64url-no-pad encoded.
    pub sig: String,
}

/// One anchor entry — a DAG-tip hash that was submitted to Sigstore Rekor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorEntry {
    /// The DAG-tip hash that was anchored.
    pub dag_tip_hash: String,
    /// Rekor entry UUID.
    pub rekor_uuid: String,
    /// Inclusion proof (base64-encoded merkle path).
    pub rekor_inclusion_proof: String,
    /// Rekor log index.
    pub rekor_log_index: u64,
    /// ISO-8601 timestamp Rekor returned.
    pub rekor_ts: String,
}

/// Optional filters applied to a bundle (for narrower audit-export).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceFilters {
    /// Time-range filter.
    #[serde(default)]
    pub period: Option<PeriodFilter>,
    /// System filter (e.g., "CreditScoreV3").
    #[serde(default)]
    pub system: Option<String>,
    /// Specific node-IDs only.
    #[serde(default)]
    pub nodes_subset: Vec<String>,
}

/// A time-range filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeriodFilter {
    /// ISO-8601 start.
    pub start: String,
    /// ISO-8601 end.
    pub end: String,
}

/// Application-level payload variants.
/// We keep this as `serde_json::Value` in `AtlasEvent` for forward-compat,
/// but expose this enum for typed inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AtlasPayload {
    /// New node in the knowledge graph.
    NodeCreate {
        /// the full node data
        node: serde_json::Value,
    },
    /// Update to existing node.
    NodeUpdate {
        /// node id
        node_id: String,
        /// patch object
        patch: serde_json::Value,
    },
    /// New edge between two nodes.
    EdgeCreate {
        /// source node id
        from: String,
        /// destination node id
        to: String,
        /// edge relation type
        relation: String,
    },
    /// RDF-star annotation.
    AnnotationAdd {
        /// triple-subject
        subject: String,
        /// predicate
        predicate: String,
        /// object value
        object: serde_json::Value,
    },
    /// Cedar policy set.
    PolicySet {
        /// Cedar policy as text
        policy_cedar: String,
    },
    /// Anchor reference event.
    AnchorCreated {
        /// Rekor UUID
        rekor_uuid: String,
        /// Inclusion proof
        rekor_proof: String,
    },
}
