//! Deterministic CBOR canonicalisation of `GraphState`.
//!
//! ## What this module produces
//!
//! - `build_canonical_bytes(state)` — pure function `&GraphState ->
//!   ProjectorResult<Vec<u8>>` producing canonical CBOR bytes
//! - `graph_state_hash(state)` — `&GraphState -> ProjectorResult<[u8;
//!   32]>` blake3-hashing those bytes
//!
//! ## Trust property (load-bearing)
//!
//! Given the same logical input, `graph_state_hash` produces the same
//! bytes every time, every Rust target, every Atlas build. This is
//! enforced by the byte-determinism CI pin
//! `tests::graph_state_hash_byte_determinism_pin` — if any future
//! change breaks the pinned hex, the build fails until the change is
//! either reverted or intentionally accompanied by an `atlas-projector`
//! crate version bump to cascade through V2 version identity.
//!
//! ## Why this matters
//!
//! V2-α's projector-state-hash CI gate (per `DECISION-ARCH-1`
//! triple-hardening, `docs/V2-MASTER-PLAN.md` §4 R-A-01) compares
//! `graph_state_hash(replay_from_events_jsonl())` against a pinned
//! `.projection-integrity.json` on every CI run. If projection
//! drifts silently, this gate fires before the drift can reach
//! production. The trust invariant is: "a fresh re-projection from
//! Layer 1 produces byte-identical Layer 2 state" — which can ONLY
//! be verified by hashing both projections and comparing.
//!
//! The `ProjectorRunAttestation` event-kind (Welle 4 candidate)
//! additionally binds `graph_state_hash` cryptographically into the
//! Layer-1 trust chain by emitting a signed Atlas event recording
//! `(projector_version, head_hash, graph_state_hash)`. That makes
//! projection-state-hash drift detectable not just at CI time but at
//! the trust-chain layer, with auditor-visible failure modes.
//!
//! ## Canonical form
//!
//! ```text
//! GraphState canonical CBOR map:
//! {
//!   "v":     "atlas-projector-v1-alpha",
//!   "nodes": [
//!     // sorted by entity_uuid (BTreeMap iteration order)
//!     { "entity_uuid": ...,
//!       "labels":   [<sorted-lex>],
//!       "properties": <CBOR-canonical-map-RFC-8949-§4.2.1>,
//!       "event_uuid": ...,
//!       "rekor_log_index": <u64>,
//!       "author_did": <omitted if None>
//!     },
//!     ...
//!   ],
//!   "edges": [
//!     // sorted by edge_id (BTreeMap iteration order)
//!     { "edge_id": ..., "from_entity": ..., "to_entity": ...,
//!       "kind": ..., "properties": ..., "event_uuid": ...,
//!       "rekor_log_index": ..., "author_did": <omitted if None>
//!     },
//!     ...
//!   ]
//! }
//! ```
//!
//! The outer map uses RFC 8949 §4.2.1 length-then-lex map key sorting
//! (`v` has encoded-key-len 2 < `nodes` encoded-key-len 6 < `edges`
//! encoded-key-len 6 with lex-tie-break `nodes` < `edges` → so order
//! on the wire is `v, nodes, edges`). The per-node and per-edge maps
//! use the same convention internally.

use ciborium::Value;
use std::collections::BTreeMap;

use crate::error::{ProjectorError, ProjectorResult};
use crate::state::{AnchorEntry, AnnotationEntry, GraphEdge, GraphNode, GraphState, PolicyEntry};
use crate::PROJECTOR_SCHEMA_VERSION;

/// Hard cap on items per array/map level. Bounds allocation under
/// hostile input. **Deliberately 10× the V1
/// `atlas_trust_core::cose::MAX_ITEMS_PER_LEVEL` cap (10_000)** —
/// graph projections legitimately have more nodes per workspace
/// than V1 events have payload-array entries. The V1 cap protects
/// per-event payload-shape canonicalisation; this cap protects
/// per-graph-state canonicalisation. Either way the bound prevents
/// memory exhaustion under hostile input — the exact number is a
/// workload-fit choice, not a security boundary.
const MAX_ITEMS_PER_LEVEL: usize = 100_000;

/// Build canonical CBOR bytes for a `GraphState`.
///
/// Runs `state.check_structural_integrity()` first; returns
/// `ProjectorError::DanglingEdge` or `MalformedEntityUuid` if the
/// graph is not internally consistent.
///
/// Per RFC 8949 §4.2.1, all map entries are sorted by encoded-key
/// length first, then bytewise lex. Per Welle 3 design invariant #1,
/// nodes + edges within the state are sorted by logical identifier
/// (`entity_uuid` / `edge_id`) — this happens for free because the
/// containers are `BTreeMap`. Labels per node are sorted
/// lexicographically.
pub fn build_canonical_bytes(state: &GraphState) -> ProjectorResult<Vec<u8>> {
    state.check_structural_integrity()?;

    if state.nodes.len() > MAX_ITEMS_PER_LEVEL {
        return Err(ProjectorError::CanonicalisationFailed(format!(
            "nodes count exceeds max ({} > {})",
            state.nodes.len(),
            MAX_ITEMS_PER_LEVEL
        )));
    }
    if state.edges.len() > MAX_ITEMS_PER_LEVEL {
        return Err(ProjectorError::CanonicalisationFailed(format!(
            "edges count exceeds max ({} > {})",
            state.edges.len(),
            MAX_ITEMS_PER_LEVEL
        )));
    }

    if state.rekor_anchors.len() > MAX_ITEMS_PER_LEVEL {
        return Err(ProjectorError::CanonicalisationFailed(format!(
            "rekor_anchors count exceeds max ({} > {})",
            state.rekor_anchors.len(),
            MAX_ITEMS_PER_LEVEL
        )));
    }

    let nodes_cbor: Vec<Value> = state
        .nodes
        .values()
        .map(canonical_node_map)
        .collect::<ProjectorResult<Vec<_>>>()?;
    let edges_cbor: Vec<Value> = state
        .edges
        .values()
        .map(canonical_edge_map)
        .collect::<ProjectorResult<Vec<_>>>()?;

    let mut entries: Vec<(Value, Value)> = vec![
        (
            Value::Text("v".into()),
            Value::Text(PROJECTOR_SCHEMA_VERSION.into()),
        ),
        (Value::Text("nodes".into()), Value::Array(nodes_cbor)),
        (Value::Text("edges".into()), Value::Array(edges_cbor)),
    ];

    // V2-β Welle 14 schema-additive: `rekor_anchors` is omitted from
    // canonical bytes when empty, mirroring the V1 backward-compat
    // pattern used for `author_did = None`. This preserves byte-
    // determinism for V1 / V2-α-shape traces — `graph_state_hash`
    // for a state with no anchors is byte-identical to pre-W14.
    if !state.rekor_anchors.is_empty() {
        let mut anchor_entries: Vec<(Value, Value)> =
            Vec::with_capacity(state.rekor_anchors.len());
        for (event_id, anchor) in &state.rekor_anchors {
            anchor_entries.push((
                Value::Text(event_id.clone()),
                canonical_anchor_entry(anchor)?,
            ));
        }
        let sorted_anchors = sort_cbor_map_entries(anchor_entries)?;
        entries.push((
            Value::Text("rekor_anchors".into()),
            Value::Map(sorted_anchors),
        ));
    }

    let sorted = sort_cbor_map_entries(entries)?;
    let envelope = Value::Map(sorted);

    let mut buf = Vec::new();
    ciborium::ser::into_writer(&envelope, &mut buf)
        .map_err(|e| ProjectorError::CanonicalisationFailed(format!("cbor serialize: {e}")))?;
    Ok(buf)
}

/// Convenience: hash the canonical bytes. Returns `[u8; 32]` (blake3
/// output width). Same hash family Atlas V1 uses throughout
/// (`atlas_trust_core::hashchain::compute_event_hash`).
#[must_use = "the graph-state hash is the function's only useful output; \
              discarding it indicates a logic bug"]
pub fn graph_state_hash(state: &GraphState) -> ProjectorResult<[u8; 32]> {
    let bytes = build_canonical_bytes(state)?;
    Ok(*blake3::hash(&bytes).as_bytes())
}

/// Build the per-node CBOR map. Map entries sorted per RFC 8949
/// §4.2.1. `author_did` is included only when `Some`, mirroring the
/// V1 `cose::build_signing_input` optional-field pattern.
fn canonical_node_map(node: &GraphNode) -> ProjectorResult<Value> {
    // Defence-in-depth: labels list is otherwise unbounded by the
    // outer node-count cap. Apply the same MAX_ITEMS_PER_LEVEL bound
    // to per-node labels to prevent a single node with millions of
    // labels from blowing memory during canonicalisation.
    if node.labels.len() > MAX_ITEMS_PER_LEVEL {
        return Err(ProjectorError::CanonicalisationFailed(format!(
            "node {} labels count exceeds max ({} > {})",
            node.entity_uuid,
            node.labels.len(),
            MAX_ITEMS_PER_LEVEL
        )));
    }
    let labels_sorted = sorted_labels(&node.labels);
    let labels_cbor = Value::Array(labels_sorted.into_iter().map(Value::Text).collect());

    let properties_cbor = json_map_to_canonical_cbor(&node.properties)?;

    let mut entries: Vec<(Value, Value)> = vec![
        (
            Value::Text("entity_uuid".into()),
            Value::Text(node.entity_uuid.clone()),
        ),
        (Value::Text("labels".into()), labels_cbor),
        (Value::Text("properties".into()), properties_cbor),
        (
            Value::Text("event_uuid".into()),
            Value::Text(node.event_uuid.clone()),
        ),
        (
            Value::Text("rekor_log_index".into()),
            Value::Integer(node.rekor_log_index.into()),
        ),
    ];
    if let Some(did) = &node.author_did {
        // Format-validate to surface issuer/upstream bugs at canonicalisation
        // boundary rather than letting a malformed DID propagate into the
        // hashed bytes. Reuses V2-α Welle 1's strict parser.
        atlas_trust_core::agent_did::validate_agent_did(did).map_err(|e| {
            ProjectorError::MalformedAuthorDid(format!("node {}: {e}", node.entity_uuid))
        })?;
        entries.push((Value::Text("author_did".into()), Value::Text(did.clone())));
    }

    // V2-β Welle 14 schema-additive: `annotations` and `policies`
    // are omitted from the per-node canonical CBOR when empty —
    // mirrors the V1 `author_did = None` omission pattern. This
    // keeps V1 / V2-α-shape traces byte-identical to pre-W14 output.
    if !node.annotations.is_empty() {
        if node.annotations.len() > MAX_ITEMS_PER_LEVEL {
            return Err(ProjectorError::CanonicalisationFailed(format!(
                "node {} annotations kind-count exceeds max ({} > {})",
                node.entity_uuid,
                node.annotations.len(),
                MAX_ITEMS_PER_LEVEL
            )));
        }
        let mut ann_entries: Vec<(Value, Value)> =
            Vec::with_capacity(node.annotations.len());
        for (kind, list) in &node.annotations {
            if list.len() > MAX_ITEMS_PER_LEVEL {
                return Err(ProjectorError::CanonicalisationFailed(format!(
                    "node {} annotation kind {} exceeds max entries ({} > {})",
                    node.entity_uuid,
                    kind,
                    list.len(),
                    MAX_ITEMS_PER_LEVEL
                )));
            }
            let list_cbor: Vec<Value> = list
                .iter()
                .map(|entry| canonical_annotation_entry(entry, &node.entity_uuid, kind))
                .collect::<ProjectorResult<Vec<_>>>()?;
            ann_entries.push((Value::Text(kind.clone()), Value::Array(list_cbor)));
        }
        let sorted_ann = sort_cbor_map_entries(ann_entries)?;
        entries.push((Value::Text("annotations".into()), Value::Map(sorted_ann)));
    }
    if !node.policies.is_empty() {
        if node.policies.len() > MAX_ITEMS_PER_LEVEL {
            return Err(ProjectorError::CanonicalisationFailed(format!(
                "node {} policies count exceeds max ({} > {})",
                node.entity_uuid,
                node.policies.len(),
                MAX_ITEMS_PER_LEVEL
            )));
        }
        let mut pol_entries: Vec<(Value, Value)> = Vec::with_capacity(node.policies.len());
        for (policy_id, policy) in &node.policies {
            pol_entries.push((
                Value::Text(policy_id.clone()),
                canonical_policy_entry(policy, &node.entity_uuid, policy_id)?,
            ));
        }
        let sorted_pol = sort_cbor_map_entries(pol_entries)?;
        entries.push((Value::Text("policies".into()), Value::Map(sorted_pol)));
    }

    let sorted = sort_cbor_map_entries(entries)?;
    Ok(Value::Map(sorted))
}

/// V2-β Welle 14: canonical encoder for an `AnnotationEntry`. Map
/// fields: `body`, `event_uuid`, plus optional `author_did` (omitted
/// when `None`; format-validated when `Some` — same pattern as
/// `canonical_node_map`). Entries sorted per RFC 8949 §4.2.1.
fn canonical_annotation_entry(
    entry: &AnnotationEntry,
    node_entity_uuid: &str,
    kind: &str,
) -> ProjectorResult<Value> {
    let mut entries: Vec<(Value, Value)> = vec![
        (Value::Text("body".into()), Value::Text(entry.body.clone())),
        (
            Value::Text("event_uuid".into()),
            Value::Text(entry.event_uuid.clone()),
        ),
    ];
    if let Some(did) = &entry.author_did {
        atlas_trust_core::agent_did::validate_agent_did(did).map_err(|e| {
            ProjectorError::MalformedAuthorDid(format!(
                "annotation on node {} kind {}: {e}",
                node_entity_uuid, kind
            ))
        })?;
        entries.push((Value::Text("author_did".into()), Value::Text(did.clone())));
    }
    let sorted = sort_cbor_map_entries(entries)?;
    Ok(Value::Map(sorted))
}

/// V2-β Welle 14: canonical encoder for a `PolicyEntry`. Map fields:
/// `policy_version`, `event_uuid`, plus optional `author_did`.
fn canonical_policy_entry(
    entry: &PolicyEntry,
    node_entity_uuid: &str,
    policy_id: &str,
) -> ProjectorResult<Value> {
    let mut entries: Vec<(Value, Value)> = vec![
        (
            Value::Text("policy_version".into()),
            Value::Text(entry.policy_version.clone()),
        ),
        (
            Value::Text("event_uuid".into()),
            Value::Text(entry.event_uuid.clone()),
        ),
    ];
    if let Some(did) = &entry.author_did {
        atlas_trust_core::agent_did::validate_agent_did(did).map_err(|e| {
            ProjectorError::MalformedAuthorDid(format!(
                "policy {} on node {}: {e}",
                policy_id, node_entity_uuid
            ))
        })?;
        entries.push((Value::Text("author_did".into()), Value::Text(did.clone())));
    }
    let sorted = sort_cbor_map_entries(entries)?;
    Ok(Value::Map(sorted))
}

/// V2-β Welle 14: canonical encoder for an `AnchorEntry`. Map fields:
/// `rekor_log_index`, `rekor_log_id`, optional `rekor_tree_id`,
/// `anchored_at`, optional `author_did`. Two optional fields, both
/// omitted from canonical bytes when `None` — preserves byte-
/// determinism for anchors built without those fields.
fn canonical_anchor_entry(entry: &AnchorEntry) -> ProjectorResult<Value> {
    let mut entries: Vec<(Value, Value)> = vec![
        (
            Value::Text("rekor_log_index".into()),
            Value::Integer(entry.rekor_log_index.into()),
        ),
        (
            Value::Text("rekor_log_id".into()),
            Value::Text(entry.rekor_log_id.clone()),
        ),
        (
            Value::Text("anchored_at".into()),
            Value::Text(entry.anchored_at.clone()),
        ),
    ];
    if let Some(tree_id) = entry.rekor_tree_id {
        entries.push((
            Value::Text("rekor_tree_id".into()),
            Value::Integer(tree_id.into()),
        ));
    }
    if let Some(did) = &entry.author_did {
        atlas_trust_core::agent_did::validate_agent_did(did)
            .map_err(|e| ProjectorError::MalformedAuthorDid(format!("rekor anchor: {e}")))?;
        entries.push((Value::Text("author_did".into()), Value::Text(did.clone())));
    }
    let sorted = sort_cbor_map_entries(entries)?;
    Ok(Value::Map(sorted))
}

/// Build the per-edge CBOR map. Same convention as `canonical_node_map`.
fn canonical_edge_map(edge: &GraphEdge) -> ProjectorResult<Value> {
    let properties_cbor = json_map_to_canonical_cbor(&edge.properties)?;

    let mut entries: Vec<(Value, Value)> = vec![
        (
            Value::Text("edge_id".into()),
            Value::Text(edge.edge_id.clone()),
        ),
        (
            Value::Text("from_entity".into()),
            Value::Text(edge.from_entity.clone()),
        ),
        (
            Value::Text("to_entity".into()),
            Value::Text(edge.to_entity.clone()),
        ),
        (Value::Text("kind".into()), Value::Text(edge.kind.clone())),
        (Value::Text("properties".into()), properties_cbor),
        (
            Value::Text("event_uuid".into()),
            Value::Text(edge.event_uuid.clone()),
        ),
        (
            Value::Text("rekor_log_index".into()),
            Value::Integer(edge.rekor_log_index.into()),
        ),
    ];
    if let Some(did) = &edge.author_did {
        atlas_trust_core::agent_did::validate_agent_did(did).map_err(|e| {
            ProjectorError::MalformedAuthorDid(format!("edge {}: {e}", edge.edge_id))
        })?;
        entries.push((Value::Text("author_did".into()), Value::Text(did.clone())));
    }

    let sorted = sort_cbor_map_entries(entries)?;
    Ok(Value::Map(sorted))
}

/// Sort labels lexicographically and deduplicate adjacent duplicates
/// post-sort. O(n log n) via sort + `Vec::dedup`; replaces an earlier
/// O(n²) `contains`-in-loop pattern (flagged by code-review of
/// Welle 3). Output is identical: sorted, unique, deterministic.
fn sorted_labels(labels: &[String]) -> Vec<String> {
    let mut v: Vec<String> = labels.to_vec();
    v.sort();
    v.dedup();
    v
}

/// Sort CBOR map entries per RFC 8949 §4.2.1: length of encoded key
/// first (shortest-first), then bytewise lex on the encoded form.
/// Identical pattern to `atlas_trust_core::cose::sort_cbor_map_entries`.
fn sort_cbor_map_entries(entries: Vec<(Value, Value)>) -> ProjectorResult<Vec<(Value, Value)>> {
    let mut with_keys: Vec<(Vec<u8>, Value, Value)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&k, &mut buf).map_err(|e| {
            ProjectorError::CanonicalisationFailed(format!("cbor key serialize: {e}"))
        })?;
        with_keys.push((buf, k, v));
    }
    with_keys.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then_with(|| a.0.cmp(&b.0)));
    Ok(with_keys.into_iter().map(|(_, k, v)| (k, v)).collect())
}

/// Convert a `BTreeMap<String, serde_json::Value>` to a canonical CBOR
/// map. Caller's `BTreeMap` is already key-sorted, but properties may
/// nest other JSON objects/arrays; the per-level sort is RFC 8949 §4.2.1.
/// Float values are rejected at this boundary (per crate doc invariant #3).
fn json_map_to_canonical_cbor(
    map: &BTreeMap<String, serde_json::Value>,
) -> ProjectorResult<Value> {
    if map.len() > MAX_ITEMS_PER_LEVEL {
        return Err(ProjectorError::CanonicalisationFailed(format!(
            "properties map exceeds max items ({} > {})",
            map.len(),
            MAX_ITEMS_PER_LEVEL
        )));
    }
    let mut entries: Vec<(Value, Value)> = Vec::with_capacity(map.len());
    for (k, v) in map {
        entries.push((Value::Text(k.clone()), json_to_canonical_cbor(v)?));
    }
    let sorted = sort_cbor_map_entries(entries)?;
    Ok(Value::Map(sorted))
}

/// Recursive `serde_json::Value -> ciborium::Value` canonicaliser.
/// Mirrors `atlas_trust_core::cose::json_to_canonical_cbor` exactly
/// for cross-crate canonicalisation consistency.
fn json_to_canonical_cbor(json: &serde_json::Value) -> ProjectorResult<Value> {
    match json {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i.into()))
            } else if let Some(u) = n.as_u64() {
                Ok(Value::Integer(u.into()))
            } else {
                Err(ProjectorError::CanonicalisationFailed(format!(
                    "non-integer number rejected by canonical CBOR: {n}. \
                     Use integer encodings (e.g. basis points)."
                )))
            }
        }
        serde_json::Value::String(s) => Ok(Value::Text(s.clone())),
        serde_json::Value::Array(arr) => {
            if arr.len() > MAX_ITEMS_PER_LEVEL {
                return Err(ProjectorError::CanonicalisationFailed(format!(
                    "array exceeds max items per level ({} > {})",
                    arr.len(),
                    MAX_ITEMS_PER_LEVEL
                )));
            }
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(json_to_canonical_cbor(item)?);
            }
            Ok(Value::Array(out))
        }
        serde_json::Value::Object(map) => {
            if map.len() > MAX_ITEMS_PER_LEVEL {
                return Err(ProjectorError::CanonicalisationFailed(format!(
                    "object exceeds max items per level ({} > {})",
                    map.len(),
                    MAX_ITEMS_PER_LEVEL
                )));
            }
            let mut entries: Vec<(Value, Value)> = Vec::with_capacity(map.len());
            for (k, v) in map {
                entries.push((Value::Text(k.clone()), json_to_canonical_cbor(v)?));
            }
            let sorted = sort_cbor_map_entries(entries)?;
            Ok(Value::Map(sorted))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{GraphEdge, GraphNode};

    fn node(uuid: &str, labels: &[&str], event_uuid: &str, log_index: u64) -> GraphNode {
        GraphNode {
            entity_uuid: uuid.to_string(),
            labels: labels.iter().map(|s| s.to_string()).collect(),
            properties: BTreeMap::new(),
            event_uuid: event_uuid.to_string(),
            rekor_log_index: log_index,
            author_did: None,
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        }
    }

    fn edge(id: &str, from: &str, to: &str, kind: &str) -> GraphEdge {
        GraphEdge {
            edge_id: id.to_string(),
            from_entity: from.to_string(),
            to_entity: to.to_string(),
            kind: kind.to_string(),
            properties: BTreeMap::new(),
            event_uuid: "01HEDGEEVENT".to_string(),
            rekor_log_index: 200,
            author_did: None,
        }
    }

    #[test]
    fn empty_state_produces_stable_hash() {
        let s = GraphState::new();
        let a = graph_state_hash(&s).unwrap();
        let b = graph_state_hash(&s).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn single_node_hash_changes_when_entity_uuid_changes() {
        let mut s1 = GraphState::new();
        s1.upsert_node(node("a", &["L"], "ev1", 1));
        let mut s2 = GraphState::new();
        s2.upsert_node(node("b", &["L"], "ev1", 1));
        assert_ne!(graph_state_hash(&s1).unwrap(), graph_state_hash(&s2).unwrap());
    }

    #[test]
    fn multi_node_insert_order_irrelevant() {
        // Load-bearing Welle-2 §3.5 invariant: hash MUST be insert-order
        // independent. Two states with identical logical content but
        // different insertion sequence produce identical hashes.
        let mut s1 = GraphState::new();
        s1.upsert_node(node("c", &["L"], "ev1", 1));
        s1.upsert_node(node("a", &["L"], "ev1", 2));
        s1.upsert_node(node("b", &["L"], "ev1", 3));

        let mut s2 = GraphState::new();
        s2.upsert_node(node("a", &["L"], "ev1", 2));
        s2.upsert_node(node("b", &["L"], "ev1", 3));
        s2.upsert_node(node("c", &["L"], "ev1", 1));

        assert_eq!(graph_state_hash(&s1).unwrap(), graph_state_hash(&s2).unwrap());
    }

    #[test]
    fn property_order_does_not_matter() {
        let mut p1 = BTreeMap::new();
        p1.insert("a".to_string(), serde_json::json!(1));
        p1.insert("b".to_string(), serde_json::json!(2));

        let mut p2 = BTreeMap::new();
        p2.insert("b".to_string(), serde_json::json!(2));
        p2.insert("a".to_string(), serde_json::json!(1));

        let mut s1 = GraphState::new();
        let mut n1 = node("a", &["L"], "ev1", 1);
        n1.properties = p1;
        s1.upsert_node(n1);

        let mut s2 = GraphState::new();
        let mut n2 = node("a", &["L"], "ev1", 1);
        n2.properties = p2;
        s2.upsert_node(n2);

        assert_eq!(graph_state_hash(&s1).unwrap(), graph_state_hash(&s2).unwrap());
    }

    #[test]
    fn label_order_does_not_matter() {
        let mut s1 = GraphState::new();
        s1.upsert_node(node("a", &["Foo", "Bar"], "ev1", 1));
        let mut s2 = GraphState::new();
        s2.upsert_node(node("a", &["Bar", "Foo"], "ev1", 1));
        assert_eq!(graph_state_hash(&s1).unwrap(), graph_state_hash(&s2).unwrap());
    }

    #[test]
    fn duplicate_labels_deduped() {
        let mut s1 = GraphState::new();
        s1.upsert_node(node("a", &["X", "X", "Y"], "ev1", 1));
        let mut s2 = GraphState::new();
        s2.upsert_node(node("a", &["X", "Y"], "ev1", 1));
        assert_eq!(graph_state_hash(&s1).unwrap(), graph_state_hash(&s2).unwrap());
    }

    #[test]
    fn author_did_present_changes_bytes() {
        // V2-α Welle 1 invariant: author_did is bound into the canonical
        // graph-state-hash. Two otherwise-identical nodes with different
        // (well-formed) author_did values MUST produce different hashes.
        let mut s1 = GraphState::new();
        let mut n1 = node("a", &["L"], "ev1", 1);
        n1.author_did =
            Some("did:atlas:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into());
        s1.upsert_node(n1);

        let mut s2 = GraphState::new();
        let mut n2 = node("a", &["L"], "ev1", 1);
        n2.author_did =
            Some("did:atlas:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into());
        s2.upsert_node(n2);

        assert_ne!(graph_state_hash(&s1).unwrap(), graph_state_hash(&s2).unwrap());
    }

    #[test]
    fn author_did_none_omits_field_from_canonical() {
        // V1 backward-compat invariant. A node with `author_did = None`
        // produces canonical bytes that do NOT include the `author_did`
        // CBOR map entry. Compare to a state with the same node but
        // `author_did = Some(valid_did)` — bytes must differ; AND the
        // None-case must not contain the substring "author_did".
        let mut s = GraphState::new();
        s.upsert_node(node("a", &["L"], "ev1", 1));
        let bytes = build_canonical_bytes(&s).unwrap();
        assert!(
            !bytes.windows(b"author_did".len()).any(|w| w == b"author_did"),
            "author_did=None must omit the field from canonical bytes"
        );
    }

    #[test]
    fn malformed_author_did_is_rejected() {
        let mut s = GraphState::new();
        let mut n = node("a", &["L"], "ev1", 1);
        n.author_did = Some("did:malformed:not-hex".to_string());
        s.upsert_node(n);
        match build_canonical_bytes(&s) {
            Err(ProjectorError::MalformedAuthorDid(_)) => {}
            other => panic!("expected MalformedAuthorDid; got {other:?}"),
        }
    }

    #[test]
    fn float_in_property_is_rejected() {
        let mut props = BTreeMap::new();
        props.insert("score".to_string(), serde_json::json!(0.78));
        let mut s = GraphState::new();
        let mut n = node("a", &["L"], "ev1", 1);
        n.properties = props;
        s.upsert_node(n);
        match build_canonical_bytes(&s) {
            Err(ProjectorError::CanonicalisationFailed(msg)) => {
                assert!(msg.contains("non-integer number"));
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn dangling_edge_is_rejected() {
        let mut s = GraphState::new();
        s.upsert_node(node("a", &["L"], "ev1", 1));
        s.upsert_edge(edge("e1", "a", "missing", "k"));
        match build_canonical_bytes(&s) {
            Err(ProjectorError::DanglingEdge { missing_endpoint, .. }) => {
                assert_eq!(missing_endpoint, "missing");
            }
            other => panic!("expected DanglingEdge; got {other:?}"),
        }
    }

    /// Cross-implementation determinism golden.
    ///
    /// Pins the exact byte-for-byte output of `build_canonical_bytes`
    /// + `graph_state_hash` for one fixed input. Any unintentional
    /// change to the canonicalisation pipeline (CBOR sort order, key
    /// encoding, struct shape, ciborium upgrade that changes encoding,
    /// PROJECTOR_SCHEMA_VERSION drift) trips this test BEFORE the
    /// change can reach the projector-state-hash CI gate or a
    /// `ProjectorRunAttestation` event payload.
    ///
    /// If you regenerate the pinned values below, you have changed
    /// projector canonicalisation semantics — bump the
    /// `atlas-projector` crate version, which cascades into V2-α
    /// version identity, so old-format graph-state-hash values are
    /// rejected with a clean schema-mismatch error rather than
    /// silently mis-comparing.
    #[test]
    fn graph_state_hash_byte_determinism_pin() {
        // Fixture: 3 nodes, 2 edges, mixed labels, mixed author_did
        // (one Some, one None — exercises both V2-α and V1-compat
        // paths).
        let mut state = GraphState::new();

        // Node A: V2-α event with author_did, two labels, simple props
        let mut node_a_props = BTreeMap::new();
        node_a_props.insert("name".to_string(), serde_json::json!("alice"));
        node_a_props.insert("count".to_string(), serde_json::json!(42));
        state.upsert_node(GraphNode {
            entity_uuid: "node-a".to_string(),
            labels: vec!["Person".to_string(), "Sensitive".to_string()],
            properties: node_a_props,
            event_uuid: "01HEVENT0001".to_string(),
            rekor_log_index: 1000,
            author_did: Some(
                "did:atlas:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            ),
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        });

        // Node B: V1-era event, no author_did, one label, no props
        state.upsert_node(GraphNode {
            entity_uuid: "node-b".to_string(),
            labels: vec!["Dataset".to_string()],
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT0002".to_string(),
            rekor_log_index: 1001,
            author_did: None,
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        });

        // Node C: edge target
        state.upsert_node(GraphNode {
            entity_uuid: "node-c".to_string(),
            labels: vec!["Model".to_string()],
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT0003".to_string(),
            rekor_log_index: 1002,
            author_did: None,
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        });

        // Edge 1: a -> b, with author_did
        state.upsert_edge(GraphEdge {
            edge_id: "edge-ab".to_string(),
            from_entity: "node-a".to_string(),
            to_entity: "node-b".to_string(),
            kind: "uses".to_string(),
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT0004".to_string(),
            rekor_log_index: 1003,
            author_did: Some(
                "did:atlas:2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
            ),
        });

        // Edge 2: b -> c, no author_did
        state.upsert_edge(GraphEdge {
            edge_id: "edge-bc".to_string(),
            from_entity: "node-b".to_string(),
            to_entity: "node-c".to_string(),
            kind: "trains".to_string(),
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT0005".to_string(),
            rekor_log_index: 1004,
            author_did: None,
        });

        // Compute canonical bytes + hash
        let bytes = build_canonical_bytes(&state).unwrap();
        let hash = graph_state_hash(&state).unwrap();

        // Determinism within a single run.
        let bytes2 = build_canonical_bytes(&state).unwrap();
        let hash2 = graph_state_hash(&state).unwrap();
        assert_eq!(bytes, bytes2);
        assert_eq!(hash, hash2);

        // Insert-order independence: rebuild via different insertion order.
        let mut state_reordered = GraphState::new();
        // Insert edges first (after nodes they reference)
        state_reordered.upsert_node(GraphNode {
            entity_uuid: "node-c".to_string(),
            labels: vec!["Model".to_string()],
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT0003".to_string(),
            rekor_log_index: 1002,
            author_did: None,
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        });
        state_reordered.upsert_node(GraphNode {
            entity_uuid: "node-b".to_string(),
            labels: vec!["Dataset".to_string()],
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT0002".to_string(),
            rekor_log_index: 1001,
            author_did: None,
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        });
        let mut node_a_props = BTreeMap::new();
        node_a_props.insert("count".to_string(), serde_json::json!(42));
        node_a_props.insert("name".to_string(), serde_json::json!("alice"));
        state_reordered.upsert_node(GraphNode {
            entity_uuid: "node-a".to_string(),
            labels: vec!["Sensitive".to_string(), "Person".to_string()], // reversed
            properties: node_a_props,
            event_uuid: "01HEVENT0001".to_string(),
            rekor_log_index: 1000,
            author_did: Some(
                "did:atlas:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            ),
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        });
        state_reordered.upsert_edge(GraphEdge {
            edge_id: "edge-bc".to_string(),
            from_entity: "node-b".to_string(),
            to_entity: "node-c".to_string(),
            kind: "trains".to_string(),
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT0005".to_string(),
            rekor_log_index: 1004,
            author_did: None,
        });
        state_reordered.upsert_edge(GraphEdge {
            edge_id: "edge-ab".to_string(),
            from_entity: "node-a".to_string(),
            to_entity: "node-b".to_string(),
            kind: "uses".to_string(),
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT0004".to_string(),
            rekor_log_index: 1003,
            author_did: Some(
                "did:atlas:2222222222222222222222222222222222222222222222222222222222222222"
                    .to_string(),
            ),
        });
        let bytes_reordered = build_canonical_bytes(&state_reordered).unwrap();
        let hash_reordered = graph_state_hash(&state_reordered).unwrap();
        assert_eq!(bytes, bytes_reordered, "insert-order must not affect canonical bytes");
        assert_eq!(hash, hash_reordered, "insert-order must not affect graph_state_hash");

        // Structural sanity: schema-version byte sequence appears.
        let schema = PROJECTOR_SCHEMA_VERSION.as_bytes();
        assert!(
            bytes.windows(schema.len()).any(|w| w == schema),
            "expected schema-version literal to appear in canonical bytes"
        );

        // Pin the hash. This locks the canonical-form output BYTES (via
        // the hash) to a specific value. Any future change that affects
        // the bytes flips the hash and trips this assertion.
        let actual_hash_hex = hex::encode(hash);
        // BEGIN PINNED HASH — DO NOT EDIT WITHOUT INTENT.
        // Captured 2026-05-12 from V2-α Welle 3 implementation run.
        // Fixture: 3 nodes (node-a/b/c) + 2 edges (edge-ab/bc) with
        // mixed labels and mixed author_did presence.
        // Canonical bytes length: 754. blake3 output:
        let expected_hash_hex =
            "8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4";
        // END PINNED.

        assert_eq!(
            actual_hash_hex, expected_hash_hex,
            "graph_state_hash drift. If intentional, update the pinned hex \
             AND bump atlas-projector's crate version so the V2-α version-identity \
             cascade propagates to old-format graph-state-hash consumers \
             (projector-state-hash CI gate, ProjectorRunAttestation event payloads)."
        );

        // Sanity: canonical byte length matches what we captured at pin-time.
        // A length change without a hash change would indicate a hash-collision
        // (vanishingly improbable) or a bug in the test.
        assert_eq!(bytes.len(), 754, "canonical bytes length drift");
    }
}
