//! V2-α Welle 5: idempotent event-to-state upsert.
//!
//! Maps a single `AtlasEvent` to mutations on an in-memory `GraphState`.
//! Welle-5-MVP narrowly supports graph-shape events:
//!
//! - `node_create` — upsert a node
//! - `node_update` — REPLACE a node entirely (V2-α-MVP semantics;
//!   patch-merge deferred to a future welle — see "node_update
//!   V2-α-MVP semantics" below)
//! - `edge_create` — upsert an edge
//!
//! Other event kinds (`annotation_add`, `policy_set`, `anchor_created`)
//! produce `ProjectorError::UnsupportedEventKind`. Caller decides
//! whether to abort or skip. V2-β may extend the supported set.
//!
//! ## entity_uuid derivation convention
//!
//! For `node_create`, the convention is:
//!
//! 1. If the payload's `node.id` field is present and is a string,
//!    use it as the `entity_uuid` directly. This matches the user
//!    mental model of issuer-supplied logical identifiers (e.g.
//!    `"credit_history_q1_2026"`).
//! 2. Otherwise, derive `entity_uuid = hex(blake3(workspace_id ||
//!    0x1F || event_uuid || 0x1F || ":node"))` — a stable
//!    cryptographic identifier seeded by the event's identity.
//!    Documented as fallback per Welle 2 §3.5 caveat on logical-
//!    identifier sort keys.
//!
//! For `edge_create`, `edge_id` is always derived as
//! `hex(blake3(workspace_id || 0x1F || event_uuid || 0x1F || ":edge"))`
//! — never issuer-supplied (edges don't carry a natural identifier
//! in V1's `EdgeCreate { from, to, relation }` shape).
//!
//! For `node_update`, the convention is:
//!
//! - `payload.node_id` (string, required) is the `entity_uuid` to update
//! - `payload.patch` (object, required) provides the new properties
//! - The update is applied via `state.upsert_node()`, which **replaces**
//!   the existing node entirely (Welle-5-MVP does not patch in-place —
//!   it overwrites). Future welles may add patch-merge semantics.
//!
//! ## author_did propagation
//!
//! When `AtlasEvent.author_did` is `Some(_)`, the upsert stamps it
//! onto every `GraphNode.author_did` / `GraphEdge.author_did`
//! produced by that event. Implements the Welle 1 schema-additive
//! invariant at the projection layer.

use std::collections::BTreeMap;

use atlas_trust_core::trace_format::AtlasEvent;
use serde_json::Value;

use crate::error::{ProjectorError, ProjectorResult};
use crate::state::{GraphEdge, GraphNode, GraphState};

/// Apply a single Atlas event to the projection state.
///
/// Dispatches on the event's `payload.type` discriminator:
///
/// - `node_create` → upsert a node from `payload.node`
/// - `node_update` → upsert a node from `payload.node_id` + `payload.patch`
/// - `edge_create` → upsert an edge from `payload.from` + `payload.to` + `payload.relation`
///
/// Returns `ProjectorError::UnsupportedEventKind` for any other
/// payload type (caller policy decides skip-vs-abort).
pub fn apply_event_to_state(
    workspace_id: &str,
    event: &AtlasEvent,
    state: &mut GraphState,
) -> ProjectorResult<()> {
    let payload_type = event
        .payload
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "type".to_string(),
        })?;

    match payload_type {
        "node_create" => apply_node_create(workspace_id, event, state),
        "node_update" => apply_node_update(workspace_id, event, state),
        "edge_create" => apply_edge_create(workspace_id, event, state),
        other => Err(ProjectorError::UnsupportedEventKind {
            kind: other.to_string(),
            event_id: event.event_id.clone(),
        }),
    }
}

/// Project an entire event sequence into a fresh-or-extended `GraphState`.
///
/// Caller-friendly top-level API. Idempotent under same input order:
/// projecting the same events twice produces a `GraphState` with
/// byte-identical `graph_state_hash`. Idempotent under repeated
/// upsert of the same `entity_uuid` (the second occurrence wins).
///
/// Returns `Err` on the first event that fails to apply — the
/// state passed in is left in a partially-mutated condition for
/// operator inspection.
pub fn project_events(
    workspace_id: &str,
    events: &[AtlasEvent],
    existing: Option<GraphState>,
) -> ProjectorResult<GraphState> {
    let mut state = existing.unwrap_or_default();
    for event in events {
        apply_event_to_state(workspace_id, event, &mut state)?;
    }
    Ok(state)
}

/// Derive a fallback `entity_uuid` for a node when `payload.node.id`
/// is absent. Uses a 0x1F (US — unit-separator) byte between
/// concatenated fields to prevent length-extension ambiguity.
fn derive_node_entity_uuid(workspace_id: &str, event_uuid: &str) -> String {
    derive_with_suffix(workspace_id, event_uuid, ":node")
}

/// Derive an `edge_id`. Always blake3-derived (edges don't carry
/// a natural caller-supplied identifier in V1's `EdgeCreate` shape).
fn derive_edge_id(workspace_id: &str, event_uuid: &str) -> String {
    derive_with_suffix(workspace_id, event_uuid, ":edge")
}

fn derive_with_suffix(workspace_id: &str, event_uuid: &str, suffix: &str) -> String {
    let mut h = blake3::Hasher::new();
    h.update(workspace_id.as_bytes());
    h.update(&[0x1f]);
    h.update(event_uuid.as_bytes());
    h.update(&[0x1f]);
    h.update(suffix.as_bytes());
    hex::encode(h.finalize().as_bytes())
}

fn apply_node_create(
    workspace_id: &str,
    event: &AtlasEvent,
    state: &mut GraphState,
) -> ProjectorResult<()> {
    let node_obj = event
        .payload
        .get("node")
        .and_then(Value::as_object)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "node".to_string(),
        })?;

    // Prefer issuer-supplied node-id; fall back to blake3 derivation.
    let entity_uuid = node_obj
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| derive_node_entity_uuid(workspace_id, &event.event_id));

    let labels: Vec<String> = node_obj
        .get("labels")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // Properties = everything in `node` except `id` and `labels`.
    let properties: BTreeMap<String, Value> = node_obj
        .iter()
        .filter(|(k, _)| k.as_str() != "id" && k.as_str() != "labels")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    state.upsert_node(GraphNode {
        entity_uuid,
        labels,
        properties,
        event_uuid: event.event_id.clone(),
        // V2-α-MVP: rekor_log_index is not yet derived from an anchor
        // lookup. Welle 7+ will plumb this from anchor.created events.
        // For Welle 5, 0 is a sentinel "not-yet-anchored" value.
        rekor_log_index: 0,
        author_did: event.author_did.clone(),
    });
    Ok(())
}

fn apply_node_update(
    _workspace_id: &str,
    event: &AtlasEvent,
    state: &mut GraphState,
) -> ProjectorResult<()> {
    let payload_obj = event
        .payload
        .as_object()
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "payload (expected JSON object)".to_string(),
        })?;

    let entity_uuid = payload_obj
        .get("node_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "node_id".to_string(),
        })?;

    let patch_obj = payload_obj
        .get("patch")
        .and_then(Value::as_object)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "patch".to_string(),
        })?;

    // V2-α-MVP semantics: node_update REPLACES the node entirely with
    // the patch contents. The patch's `labels` array (if present) sets
    // labels; everything else becomes properties.
    //
    // Future welles may add patch-merge (keep existing properties, only
    // overlay patch keys). Documented in module docstring.
    let labels: Vec<String> = patch_obj
        .get("labels")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let properties: BTreeMap<String, Value> = patch_obj
        .iter()
        .filter(|(k, _)| k.as_str() != "labels")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    state.upsert_node(GraphNode {
        entity_uuid,
        labels,
        properties,
        event_uuid: event.event_id.clone(),
        rekor_log_index: 0,
        author_did: event.author_did.clone(),
    });
    Ok(())
}

fn apply_edge_create(
    workspace_id: &str,
    event: &AtlasEvent,
    state: &mut GraphState,
) -> ProjectorResult<()> {
    let payload_obj = event
        .payload
        .as_object()
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "payload (expected JSON object)".to_string(),
        })?;

    let from_entity = payload_obj
        .get("from")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "from".to_string(),
        })?;

    let to_entity = payload_obj
        .get("to")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "to".to_string(),
        })?;

    let kind = payload_obj
        .get("relation")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "relation".to_string(),
        })?;

    let edge_id = derive_edge_id(workspace_id, &event.event_id);

    state.upsert_edge(GraphEdge {
        edge_id,
        from_entity,
        to_entity,
        kind,
        properties: BTreeMap::new(),
        event_uuid: event.event_id.clone(),
        rekor_log_index: 0,
        author_did: event.author_did.clone(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_trust_core::trace_format::EventSignature;
    use serde_json::json;

    const WS: &str = "ws-test";

    fn make_event(event_id: &str, payload: Value, author_did: Option<&str>) -> AtlasEvent {
        AtlasEvent {
            event_id: event_id.to_string(),
            event_hash: "deadbeef".to_string(),
            parent_hashes: vec![],
            payload,
            signature: EventSignature {
                alg: "EdDSA".to_string(),
                kid: format!("atlas-anchor:{WS}"),
                sig: "AAAA".to_string(),
            },
            ts: "2026-05-13T10:00:00Z".to_string(),
            author_did: author_did.map(String::from),
        }
    }

    #[test]
    fn node_create_with_explicit_id_uses_it_as_entity_uuid() {
        let event = make_event(
            "01HEVENT1",
            json!({"type": "node_create", "node": {"id": "my-node", "labels": ["L"]}}),
            None,
        );
        let mut state = GraphState::new();
        apply_event_to_state(WS, &event, &mut state).unwrap();
        assert!(state.nodes.contains_key("my-node"));
        assert_eq!(state.nodes["my-node"].labels, vec!["L".to_string()]);
    }

    #[test]
    fn node_create_without_id_derives_blake3_entity_uuid() {
        let event = make_event(
            "01HEVENT1",
            json!({"type": "node_create", "node": {"name": "Anon"}}),
            None,
        );
        let mut state = GraphState::new();
        apply_event_to_state(WS, &event, &mut state).unwrap();
        assert_eq!(state.nodes.len(), 1);
        let expected = derive_node_entity_uuid(WS, "01HEVENT1");
        assert!(state.nodes.contains_key(&expected));
    }

    #[test]
    fn node_create_properties_exclude_id_and_labels() {
        let event = make_event(
            "01HEVENT1",
            json!({"type": "node_create", "node": {"id": "n1", "labels": ["L"], "name": "Alice", "age": 30}}),
            None,
        );
        let mut state = GraphState::new();
        apply_event_to_state(WS, &event, &mut state).unwrap();
        let node = &state.nodes["n1"];
        assert!(!node.properties.contains_key("id"));
        assert!(!node.properties.contains_key("labels"));
        assert_eq!(node.properties.get("name").and_then(Value::as_str), Some("Alice"));
        assert_eq!(node.properties.get("age").and_then(Value::as_i64), Some(30));
    }

    #[test]
    fn node_create_author_did_propagates() {
        let did = "did:atlas:1111111111111111111111111111111111111111111111111111111111111111";
        let event = make_event(
            "01HEVENT1",
            json!({"type": "node_create", "node": {"id": "n1"}}),
            Some(did),
        );
        let mut state = GraphState::new();
        apply_event_to_state(WS, &event, &mut state).unwrap();
        assert_eq!(state.nodes["n1"].author_did, Some(did.to_string()));
    }

    #[test]
    fn node_create_author_did_none_stays_none() {
        // V1 backward-compat: an event without author_did produces
        // a node with author_did = None (not Some("") or other accident).
        let event = make_event(
            "01HEVENT1",
            json!({"type": "node_create", "node": {"id": "n1"}}),
            None,
        );
        let mut state = GraphState::new();
        apply_event_to_state(WS, &event, &mut state).unwrap();
        assert_eq!(state.nodes["n1"].author_did, None);
    }

    #[test]
    fn node_update_author_did_propagates() {
        let did = "did:atlas:2222222222222222222222222222222222222222222222222222222222222222";
        let event = make_event(
            "01HEVENT2",
            json!({"type": "node_update", "node_id": "existing", "patch": {"name": "x"}}),
            Some(did),
        );
        let mut state = GraphState::new();
        apply_event_to_state(WS, &event, &mut state).unwrap();
        assert_eq!(state.nodes["existing"].author_did, Some(did.to_string()));
    }

    #[test]
    fn edge_create_author_did_propagates() {
        let did = "did:atlas:3333333333333333333333333333333333333333333333333333333333333333";
        let event = make_event(
            "01HEDGE1",
            json!({"type": "edge_create", "from": "a", "to": "b", "relation": "uses"}),
            Some(did),
        );
        let mut state = GraphState::new();
        apply_event_to_state(WS, &event, &mut state).unwrap();
        let edge = state.edges.values().next().unwrap();
        assert_eq!(edge.author_did, Some(did.to_string()));
    }

    #[test]
    fn node_update_uses_node_id_as_entity_uuid() {
        let event = make_event(
            "01HEVENT2",
            json!({"type": "node_update", "node_id": "existing-node", "patch": {"name": "updated"}}),
            None,
        );
        let mut state = GraphState::new();
        apply_event_to_state(WS, &event, &mut state).unwrap();
        assert!(state.nodes.contains_key("existing-node"));
        assert_eq!(
            state.nodes["existing-node"].properties.get("name").and_then(Value::as_str),
            Some("updated")
        );
    }

    #[test]
    fn edge_create_links_from_to_entities() {
        let event = make_event(
            "01HEDGE1",
            json!({"type": "edge_create", "from": "node-a", "to": "node-b", "relation": "uses"}),
            None,
        );
        let mut state = GraphState::new();
        apply_event_to_state(WS, &event, &mut state).unwrap();
        assert_eq!(state.edges.len(), 1);
        let edge = state.edges.values().next().unwrap();
        assert_eq!(edge.from_entity, "node-a");
        assert_eq!(edge.to_entity, "node-b");
        assert_eq!(edge.kind, "uses");
    }

    #[test]
    fn unsupported_event_kind_rejected() {
        let event = make_event(
            "01HEVENT1",
            json!({"type": "policy_set", "policy_cedar": "..."}),
            None,
        );
        let mut state = GraphState::new();
        match apply_event_to_state(WS, &event, &mut state) {
            Err(ProjectorError::UnsupportedEventKind { kind, .. }) => {
                assert_eq!(kind, "policy_set");
            }
            other => panic!("expected UnsupportedEventKind; got {other:?}"),
        }
    }

    #[test]
    fn missing_type_field_rejected() {
        let event = make_event("01HEVENT1", json!({"foo": "bar"}), None);
        let mut state = GraphState::new();
        match apply_event_to_state(WS, &event, &mut state) {
            Err(ProjectorError::MissingPayloadField { field, .. }) => {
                assert_eq!(field, "type");
            }
            other => panic!("expected MissingPayloadField; got {other:?}"),
        }
    }

    #[test]
    fn missing_node_object_rejected_on_create() {
        let event = make_event(
            "01HEVENT1",
            json!({"type": "node_create"}),
            None,
        );
        let mut state = GraphState::new();
        match apply_event_to_state(WS, &event, &mut state) {
            Err(ProjectorError::MissingPayloadField { field, .. }) => {
                assert_eq!(field, "node");
            }
            other => panic!("expected MissingPayloadField; got {other:?}"),
        }
    }

    #[test]
    fn idempotency_same_events_twice_same_state() {
        // Welle 5 idempotency invariant: projecting the same events
        // twice into separate fresh states produces byte-identical
        // graph state.
        let e1 = make_event(
            "01HEVENT1",
            json!({"type": "node_create", "node": {"id": "n1", "name": "alice"}}),
            None,
        );
        let e2 = make_event(
            "01HEVENT2",
            json!({"type": "node_create", "node": {"id": "n2", "name": "bob"}}),
            None,
        );
        let s1 = project_events(WS, &[e1.clone(), e2.clone()], None).unwrap();
        let s2 = project_events(WS, &[e1, e2], None).unwrap();
        let h1 = crate::canonical::graph_state_hash(&s1).unwrap();
        let h2 = crate::canonical::graph_state_hash(&s2).unwrap();
        assert_eq!(h1, h2, "idempotency invariant violated");
    }

    #[test]
    fn project_events_preserves_existing_state() {
        let mut existing = GraphState::new();
        existing.upsert_node(GraphNode {
            entity_uuid: "preexisting".to_string(),
            labels: vec![],
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT0".to_string(),
            rekor_log_index: 0,
            author_did: None,
        });
        let event = make_event(
            "01HEVENT1",
            json!({"type": "node_create", "node": {"id": "new"}}),
            None,
        );
        let state = project_events(WS, &[event], Some(existing)).unwrap();
        assert_eq!(state.nodes.len(), 2);
        assert!(state.nodes.contains_key("preexisting"));
        assert!(state.nodes.contains_key("new"));
    }

    #[test]
    fn derived_entity_uuid_is_deterministic() {
        let a = derive_node_entity_uuid("ws", "ev1");
        let b = derive_node_entity_uuid("ws", "ev1");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        let c = derive_node_entity_uuid("ws", "ev2");
        assert_ne!(a, c, "different event_uuid must produce different hash");
    }

    #[test]
    fn derived_node_and_edge_ids_differ_for_same_event() {
        let n = derive_node_entity_uuid("ws", "ev1");
        let e = derive_edge_id("ws", "ev1");
        assert_ne!(n, e, "node and edge derivations must differ for same event");
    }
}
