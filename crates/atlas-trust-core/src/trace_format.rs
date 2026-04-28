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

/// What kind of object an `AnchorEntry` refers to.
///
/// Anchors give an offline auditor a third-party witness that some hash
/// existed before time T. We anchor two things:
///   - DAG tips, so an auditor can prove "this trace state existed by T"
///   - the pubkey-bundle hash, so an auditor can prove "this exact key
///     roster was the one in use by T" (defends against post-hoc bundle
///     swaps that would re-validate forged signatures).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    /// `anchored_hash` is one of the trace's `dag_tips` (event_hash).
    DagTip,
    /// `anchored_hash` is the trace's `pubkey_bundle_hash`.
    BundleHash,
}

/// One anchor entry — proof that a specific hash was committed to a
/// transparency log at a specific time, with a Merkle inclusion proof
/// against a signed log checkpoint.
///
/// V1.5 ships the offline verification path. The anchored hash is
/// canonically bound to a tree position, the tree position is bound to
/// the root via the inclusion proof, and the root is bound to the log
/// identity via the checkpoint signature. An auditor with the pinned
/// log public key can verify all three links without network access.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorEntry {
    /// What kind of object `anchored_hash` refers to.
    pub kind: AnchorKind,
    /// The hash that was anchored (hex). For `DagTip` this is an event_hash;
    /// for `BundleHash` this is the trace's `pubkey_bundle_hash`.
    pub anchored_hash: String,
    /// Identifier of the transparency log holding this entry. For Sigstore
    /// Rekor this is the hex SHA-256 of the log's public key. The verifier
    /// uses this to look up the corresponding pinned pubkey.
    pub log_id: String,
    /// 0-indexed leaf position of the entry in the log.
    pub log_index: u64,
    /// Unix seconds the log claims this entry was integrated.
    pub integrated_time: i64,
    /// Merkle inclusion proof against a signed checkpoint.
    pub inclusion_proof: InclusionProof,
}

/// Merkle inclusion proof of a leaf against a signed log checkpoint.
///
/// The proof binds a single leaf hash (derived from `anchored_hash`) to
/// `root_hash` via `hashes` (RFC 6962 §2.1.1 sibling ordering, deepest
/// sibling first). `checkpoint_sig` is the log's Ed25519 signature over
/// the canonical checkpoint bytes built from `tree_size` + `root_hash`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InclusionProof {
    /// Tree size at which inclusion was witnessed.
    pub tree_size: u64,
    /// Hex root hash of the tree at `tree_size`.
    pub root_hash: String,
    /// Hex sibling hashes from leaf to root (RFC 6962 ordering).
    pub hashes: Vec<String>,
    /// Base64-no-pad Ed25519 signature over canonical checkpoint bytes.
    /// Canonical bytes: see `atlas_trust_core::anchor::canonical_checkpoint_bytes`.
    pub checkpoint_sig: String,
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
    /// Anchor reference event (audit log of when an anchor was created).
    AnchorCreated {
        /// What was anchored (dag_tip or bundle_hash)
        kind: AnchorKind,
        /// The hex hash that was anchored
        anchored_hash: String,
        /// Log identifier (hex SHA-256 of log pubkey)
        log_id: String,
        /// Position in the log
        log_index: u64,
    },
}
