//! In-memory `GraphState` representation for V2-α Welle 3.
//!
//! ## Container-choice rationale (load-bearing)
//!
//! `GraphState.nodes` uses `BTreeMap<String, GraphNode>` keyed by
//! `entity_uuid`. `GraphState.edges` uses `BTreeMap<String, GraphEdge>`
//! keyed by `edge_id`. The `BTreeMap` iteration order is sorted by
//! key. This is what makes the V2-α Welle 2 spike §3.5 critical caveat
//! ("@rid is insert-order, NOT logical identity anchor") structurally
//! impossible to violate within this crate: the canonical encoding
//! walks `BTreeMap` iteration order, which is identical regardless of
//! the order in which `upsert_node` / `upsert_edge` were called.
//!
//! `HashMap` would have been wrong here even with a sort-before-encode
//! pass, because:
//!   * extra sort step on every canonicalisation = O(n log n) overhead
//!     vs O(n) walk
//!   * the sort key would be a runtime choice (entity_uuid or @rid),
//!     and a misimpl that sorted by an insert-order field would be
//!     silently wrong; with BTreeMap the key choice IS the data
//!     structure invariant
//!
//! ## entity_uuid derivation
//!
//! `entity_uuid` is intended to be `hex::encode(blake3(workspace_id ||
//! event_uuid || kind))`, but Welle 3 does NOT compute it — Welle 4
//! (idempotent upsert from events) will. Welle 3 just stores the
//! caller-supplied string and uses it as the canonical-sort key.
//!
//! ## edge_id derivation
//!
//! `edge_id` is similarly intended to be `hex::encode(blake3(from_entity
//! || to_entity || edge_kind))`. Same deferral story.
//!
//! ## Why not derive serde::{Serialize, Deserialize} on these types?
//!
//! Welle 3 isolates wire-format concerns from in-memory-state concerns.
//! The single canonical-CBOR boundary is `canonical::build_canonical_bytes`.
//! Adding serde derives would create the temptation to use `serde_json::to_vec`
//! as a "convenient" canonicalisation shortcut, which would NOT produce
//! the deterministic CBOR form required by the byte-pin invariant.
//! V1's `AtlasEvent` (serde for wire) vs `build_signing_input` (canonical
//! CBOR for hashing) split is the pattern we mirror.

use std::collections::BTreeMap;

use crate::error::{ProjectorError, ProjectorResult};

/// V2-β Welle 14: a single annotation attached to a graph entity.
/// Plain data — no serde derives (mirrors `GraphNode`/`GraphEdge`
/// convention; canonical encoding lives in `canonical.rs`).
///
/// Multiple `AnnotationEntry` values may live under the same
/// `annotation_kind` key in `GraphNode.annotations` — the projector
/// appends them in event-arrival order (which is `events.jsonl`
/// order, the deterministic Layer-1 sequence). Canonicalisation
/// preserves that order; idempotent replay reproduces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationEntry {
    /// Free-text annotation body (no schema imposed at this layer;
    /// caller-domain meaning).
    pub body: String,

    /// Layer-1 event that emitted this annotation.
    pub event_uuid: String,

    /// V2-α Welle 1 optional agent-identity of the annotation
    /// emitter. `None` for V1-era events.
    pub author_did: Option<String>,
}

/// V2-β Welle 14: a policy reference attached to a graph entity.
/// Last-write-wins per `policy_id` (the BTreeMap key in
/// `GraphNode.policies`). The reference is opaque to the projector
/// — policy evaluation is V2-δ scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEntry {
    /// Caller-provided policy version (defaults to `"v1"` in
    /// upsert dispatch when payload omits the field).
    pub policy_version: String,

    /// Layer-1 event that most recently set this policy.
    pub event_uuid: String,

    /// V2-α Welle 1 optional agent-identity of the policy-set
    /// emitter. `None` for V1-era events.
    pub author_did: Option<String>,
}

/// V2-β Welle 18b: a GDPR Art. 17 erasure record for a Layer-3
/// embedding that was secure-deleted from the Mem0g cache. Keyed by
/// the `event_id` whose embedding was erased (in
/// `GraphState.embedding_erasures`). Append-only per event_id —
/// `apply_embedding_erased` refuses to re-erase an event (the cached
/// Layer-1 erasure record is itself an append-only auditable artefact
/// for regulator evidence per ADR-Atlas-012 §4 sub-decision #5).
///
/// The audit-event itself is NEVER subject to secure-delete; this
/// record stays in `events.jsonl` as cryptographic evidence of the
/// erasure for regulator-evidentiary completeness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingErasureEntry {
    /// Per-tenant workspace identifier whose data was erased. Lets a
    /// regulator scope the erasure to a specific tenant in cross-
    /// reference with the Subject Access Request log.
    pub workspace_id: String,

    /// ISO-8601 timestamp the erasure was performed at.
    pub erased_at: String,

    /// Optional DID of the operator or data-subject-identified
    /// requestor. Defaults to the event's `author_did` if omitted from
    /// payload (resolved at projection time).
    pub requestor_did: Option<String>,

    /// Canonical reason code: `"gdpr_art_17"`, `"operator_purge"`, or
    /// `"retention_policy_expiry"`. Defaults to `"operator_purge"` if
    /// omitted from payload (resolved at projection time).
    pub reason_code: String,

    /// V2-α Welle 1 optional agent-identity of the erasure emitter.
    /// `None` for V1-era events without agent attribution.
    pub author_did: Option<String>,
}

/// V2-β Welle 14: a Sigstore Rekor anchor reference for a previously-
/// emitted event. Keyed by `event_id` (the event being anchored) in
/// `GraphState.rekor_anchors`. Append-only per event_id —
/// `apply_anchor_created` refuses to re-anchor an event (Sigstore
/// transparency-log entries are append-only; a second anchor for
/// the same event with different log-index would indicate either
/// tampering or replay-attack).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorEntry {
    /// Rekor log index.
    pub rekor_log_index: u64,

    /// Rekor log identifier (string handle for the log instance).
    pub rekor_log_id: String,

    /// Optional Rekor tree-id (CT-style tree identity).
    pub rekor_tree_id: Option<u64>,

    /// ISO-8601 timestamp the anchor was recorded at.
    pub anchored_at: String,

    /// V2-α Welle 1 optional agent-identity of the anchor emitter.
    /// `None` for V1-era events.
    pub author_did: Option<String>,
}

/// A single node in the V2-α graph projection.
///
/// `entity_uuid` is the canonical-sort key (BTreeMap-keyed in
/// `GraphState.nodes`). `labels` are stored as a `Vec<String>` for
/// caller-convenience but are sorted at canonicalisation time so
/// insert-order does not affect the hash.
///
/// The Welle 1 `author_did` field is OPTIONAL — V1-era events without
/// agent-identity produce `None`; V2-α events with agent-DID produce
/// `Some(did:atlas:...)`. When present, the DID is canonically bound
/// into the graph-state-hash (`canonical::build_canonical_bytes`
/// includes the field in the per-node CBOR map only when `Some`).
///
/// V2-β Welle 14 schema-additive fields: `annotations` and `policies`.
/// Both default to empty `BTreeMap` and are omitted from canonical
/// bytes when empty — preserving byte-determinism for V1 / V2-α
/// shape traces (mirrors the `author_did = None` omission pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    /// Logical identity of the entity this node represents. Stable
    /// across re-projections from `events.jsonl`. Canonical-sort key.
    pub entity_uuid: String,

    /// Atlas-domain labels for this node (e.g. `["Dataset", "Sensitive"]`).
    /// Sorted at canonicalisation time; insert-order ignored.
    pub labels: Vec<String>,

    /// Application properties. JSON values; floats rejected at
    /// canonicalisation boundary (per crate doc invariant #3).
    /// Sorted per RFC 8949 §4.2.1 at canonicalisation time.
    pub properties: BTreeMap<String, serde_json::Value>,

    /// The `event_uuid` of the Layer-1 event that created or most
    /// recently updated this node. Stamped per `DECISION-ARCH-1`
    /// projection-determinism requirement.
    pub event_uuid: String,

    /// The Sigstore Rekor `logIndex` of the anchor for the creating
    /// or last-updating event. Stamped for provenance.
    pub rekor_log_index: u64,

    /// V2-α Welle 1 optional agent-identity (`did:atlas:<hex>`).
    /// `None` for V1-era events without agent attribution.
    pub author_did: Option<String>,

    /// V2-β Welle 14 schema-additive: annotations attached to this
    /// entity. Keyed by `annotation_kind`; each kind maps to a `Vec`
    /// of `AnnotationEntry` appended in event-arrival order.
    /// Empty for V1 / V2-α-shape entities (omitted from canonical bytes).
    pub annotations: BTreeMap<String, Vec<AnnotationEntry>>,

    /// V2-β Welle 14 schema-additive: policy references attached to
    /// this entity. Keyed by `policy_id`; last-write-wins per id.
    /// Empty for V1 / V2-α-shape entities (omitted from canonical bytes).
    pub policies: BTreeMap<String, PolicyEntry>,
}

/// A single edge in the V2-α graph projection. Directed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    /// Logical identity of the edge. Stable across re-projections.
    /// Canonical-sort key (BTreeMap-keyed in `GraphState.edges`).
    pub edge_id: String,

    /// Source endpoint — references a `GraphNode.entity_uuid`.
    pub from_entity: String,

    /// Target endpoint — references a `GraphNode.entity_uuid`.
    pub to_entity: String,

    /// Edge kind (e.g. `"derived_from"`, `"signed_by"`).
    pub kind: String,

    /// Application properties on the edge. Same canonicalisation
    /// rules as `GraphNode.properties`.
    pub properties: BTreeMap<String, serde_json::Value>,

    /// Layer-1 event that created or last updated this edge.
    pub event_uuid: String,

    /// Sigstore Rekor `logIndex` for the anchor.
    pub rekor_log_index: u64,

    /// V2-α Welle 1 optional agent-identity.
    pub author_did: Option<String>,
}

/// In-memory canonical graph state. Pure data — no DB connection,
/// no I/O, no async. Welle 3 scope; V2-β Welle 14 schema-additive
/// `rekor_anchors` field appended.
///
/// **Container invariant:** `nodes` and `edges` are `BTreeMap` so
/// iteration produces logical-identifier-sorted output without an
/// explicit sort step at canonicalisation time. See module
/// docstring for why this is load-bearing.
///
/// V2-β Welle 14 adds `rekor_anchors: BTreeMap<String, AnchorEntry>`
/// — keyed by the `event_id` being anchored (anchors live at the
/// top level rather than per-entity because they describe a specific
/// Layer-1 event rather than a graph entity; one anchor = one event).
/// Empty for V1 / V2-α-shape traces (omitted from canonical bytes).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphState {
    /// Nodes keyed by `entity_uuid`. `BTreeMap` so iteration is sorted.
    pub nodes: BTreeMap<String, GraphNode>,

    /// Edges keyed by `edge_id`. `BTreeMap` so iteration is sorted.
    pub edges: BTreeMap<String, GraphEdge>,

    /// V2-β Welle 14 schema-additive: Rekor anchor references keyed
    /// by `event_id` (the event being anchored). One anchor per event
    /// — re-anchor attempts surface a structured error rather than
    /// last-write-wins (Sigstore transparency-log entries are
    /// append-only by spec). Empty for V1 / V2-α-shape traces (omitted
    /// from canonical bytes).
    pub rekor_anchors: BTreeMap<String, AnchorEntry>,

    /// V2-β Welle 18b schema-additive: GDPR Art. 17 erasure records
    /// keyed by `event_id` (the event whose Layer-3 embedding was
    /// secure-deleted). One erasure record per event — duplicate
    /// erasure attempts surface a structured error rather than
    /// last-write-wins (the erasure record itself is append-only
    /// regulator evidence per ADR-Atlas-012 §4 sub-decision #5).
    /// Empty for V1 / V2-α-shape / V2-β-pre-W18b traces (omitted
    /// from canonical bytes, preserving byte-pin compatibility).
    pub embedding_erasures: BTreeMap<String, EmbeddingErasureEntry>,
}

impl GraphState {
    /// Build an empty `GraphState`. Equivalent to `Default::default()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a node. Returns the previous node if
    /// the `entity_uuid` was already present (caller can decide
    /// whether to treat as update or duplicate based on context).
    ///
    /// Welle 3 does NOT enforce duplicate-rejection here; that's the
    /// idempotent-upsert layer's responsibility (Welle 4). This
    /// function just provides the BTreeMap-insert primitive.
    pub fn upsert_node(&mut self, node: GraphNode) -> Option<GraphNode> {
        let key = node.entity_uuid.clone();
        self.nodes.insert(key, node)
    }

    /// Insert or replace an edge. Returns the previous edge if
    /// the `edge_id` was already present.
    pub fn upsert_edge(&mut self, edge: GraphEdge) -> Option<GraphEdge> {
        let key = edge.edge_id.clone();
        self.edges.insert(key, edge)
    }

    /// V2-β Welle 14: insert a Rekor anchor. Returns the previous
    /// anchor if the `event_id` was already anchored — caller's
    /// dispatch layer (`upsert::apply_anchor_created`) treats a
    /// non-`None` return as a security-conservative error (Sigstore
    /// transparency-log entries are append-only). Welle 14 keeps the
    /// `BTreeMap::insert` primitive here and delegates the
    /// duplicate-rejection policy to the upsert layer (mirrors the
    /// `upsert_node` + Welle-4-duplicate-policy split documented in
    /// `error::ProjectorError::DuplicateNode`).
    pub fn upsert_anchor(&mut self, event_id: String, anchor: AnchorEntry) -> Option<AnchorEntry> {
        self.rekor_anchors.insert(event_id, anchor)
    }

    /// V2-β Welle 18b: insert a Layer-3 embedding-erasure record.
    /// Returns the previous record if the `event_id` was already
    /// erased — caller's dispatch layer (`upsert::apply_embedding_erased`)
    /// treats a non-`None` return as a security-conservative error
    /// (the audit record itself is append-only regulator evidence
    /// per ADR-Atlas-012 §4 sub-decision #5). Mirrors the
    /// `upsert_anchor` + W14-duplicate-policy split.
    pub fn upsert_embedding_erasure(
        &mut self,
        event_id: String,
        erasure: EmbeddingErasureEntry,
    ) -> Option<EmbeddingErasureEntry> {
        self.embedding_erasures.insert(event_id, erasure)
    }

    /// Structural-integrity check: every edge's `from_entity` and
    /// `to_entity` MUST reference an `entity_uuid` present in `nodes`.
    /// Called by `canonical::build_canonical_bytes` before encoding
    /// so a dangling-edge failure surfaces as a structured
    /// `ProjectorError::DanglingEdge` rather than a downstream
    /// canonicalisation surprise.
    ///
    /// `entity_uuid` non-emptiness is also enforced here as a
    /// minimal structural check; richer format-validation may be
    /// added in a later welle.
    ///
    /// V2-β Welle 14 extension: anchor keys (event_ids in
    /// `rekor_anchors`) are also non-emptiness-checked here.
    pub fn check_structural_integrity(&self) -> ProjectorResult<()> {
        for node in self.nodes.values() {
            if node.entity_uuid.is_empty() {
                return Err(ProjectorError::MalformedEntityUuid(
                    "entity_uuid is empty".to_string(),
                ));
            }
        }
        for anchor_key in self.rekor_anchors.keys() {
            if anchor_key.is_empty() {
                return Err(ProjectorError::MalformedEntityUuid(
                    "rekor_anchor event_id key is empty".to_string(),
                ));
            }
        }
        // V2-β Welle 18b: same emptiness guard for embedding-erasure
        // event_id keys (symmetric to rekor_anchors).
        for erasure_key in self.embedding_erasures.keys() {
            if erasure_key.is_empty() {
                return Err(ProjectorError::MalformedEntityUuid(
                    "embedding_erasure event_id key is empty".to_string(),
                ));
            }
        }
        for edge in self.edges.values() {
            if edge.edge_id.is_empty() {
                // Symmetric to entity_uuid emptiness check. An edge with
                // empty edge_id would canonicalise as a CBOR text-string
                // key of zero length — structurally valid but semantically
                // broken (would collapse all empty-id edges into one in
                // any downstream consumer keyed on edge_id).
                return Err(ProjectorError::MalformedEntityUuid(
                    format!(
                        "edge_id is empty (edge {}-{}-{})",
                        edge.from_entity, edge.kind, edge.to_entity
                    ),
                ));
            }
            if !self.nodes.contains_key(&edge.from_entity) {
                return Err(ProjectorError::DanglingEdge {
                    edge_id: edge.edge_id.clone(),
                    missing_endpoint: edge.from_entity.clone(),
                });
            }
            if !self.nodes.contains_key(&edge.to_entity) {
                return Err(ProjectorError::DanglingEdge {
                    edge_id: edge.edge_id.clone(),
                    missing_endpoint: edge.to_entity.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_node(uuid: &str) -> GraphNode {
        GraphNode {
            entity_uuid: uuid.to_string(),
            labels: vec!["Test".to_string()],
            properties: BTreeMap::new(),
            event_uuid: "01HEVENTAAA".to_string(),
            rekor_log_index: 100,
            author_did: None,
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        }
    }

    fn sample_edge(id: &str, from: &str, to: &str) -> GraphEdge {
        GraphEdge {
            edge_id: id.to_string(),
            from_entity: from.to_string(),
            to_entity: to.to_string(),
            kind: "test_edge".to_string(),
            properties: BTreeMap::new(),
            event_uuid: "01HEVENTBBB".to_string(),
            rekor_log_index: 101,
            author_did: None,
        }
    }

    #[test]
    fn new_state_is_empty() {
        let s = GraphState::new();
        assert!(s.nodes.is_empty());
        assert!(s.edges.is_empty());
    }

    #[test]
    fn upsert_node_returns_previous_on_collision() {
        let mut s = GraphState::new();
        assert!(s.upsert_node(sample_node("a")).is_none());
        let prev = s.upsert_node(sample_node("a"));
        assert!(prev.is_some());
    }

    #[test]
    fn upsert_edge_returns_previous_on_collision() {
        let mut s = GraphState::new();
        s.upsert_node(sample_node("a"));
        s.upsert_node(sample_node("b"));
        assert!(s.upsert_edge(sample_edge("e1", "a", "b")).is_none());
        let prev = s.upsert_edge(sample_edge("e1", "a", "b"));
        assert!(prev.is_some());
    }

    #[test]
    fn integrity_check_succeeds_on_well_formed_state() {
        let mut s = GraphState::new();
        s.upsert_node(sample_node("a"));
        s.upsert_node(sample_node("b"));
        s.upsert_edge(sample_edge("e1", "a", "b"));
        assert!(s.check_structural_integrity().is_ok());
    }

    #[test]
    fn integrity_check_rejects_dangling_edge() {
        let mut s = GraphState::new();
        s.upsert_node(sample_node("a"));
        // No node "b"
        s.upsert_edge(sample_edge("e1", "a", "b"));
        match s.check_structural_integrity() {
            Err(ProjectorError::DanglingEdge { missing_endpoint, .. }) => {
                assert_eq!(missing_endpoint, "b");
            }
            other => panic!("expected DanglingEdge; got {other:?}"),
        }
    }

    #[test]
    fn integrity_check_rejects_empty_entity_uuid() {
        let mut s = GraphState::new();
        s.upsert_node(sample_node(""));
        match s.check_structural_integrity() {
            Err(ProjectorError::MalformedEntityUuid(_)) => {}
            other => panic!("expected MalformedEntityUuid; got {other:?}"),
        }
    }

    #[test]
    fn integrity_check_rejects_empty_edge_id() {
        let mut s = GraphState::new();
        s.upsert_node(sample_node("a"));
        s.upsert_node(sample_node("b"));
        s.upsert_edge(sample_edge("", "a", "b"));
        match s.check_structural_integrity() {
            Err(ProjectorError::MalformedEntityUuid(msg)) => {
                assert!(msg.contains("edge_id is empty"));
            }
            other => panic!("expected MalformedEntityUuid for empty edge_id; got {other:?}"),
        }
    }

    #[test]
    fn btreemap_iteration_is_sorted_by_key() {
        // Load-bearing invariant: BTreeMap iteration sorts by key.
        // Documents the canonical-sort foundation.
        let mut s = GraphState::new();
        s.upsert_node(sample_node("c"));
        s.upsert_node(sample_node("a"));
        s.upsert_node(sample_node("b"));
        let keys: Vec<&str> = s.nodes.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }
}
