//! V2-α Welle 5 / V2-β Welle 14: idempotent event-to-state upsert.
//!
//! Maps a single `AtlasEvent` to mutations on an in-memory `GraphState`.
//! Supported payload `type` discriminators:
//!
//! - `node_create` — upsert a node (Welle 5)
//! - `node_update` — REPLACE a node's labels+properties entirely (V2-α
//!   MVP semantics; patch-merge deferred to a future welle).
//!   Welle 14 amendment: `annotations` / `policies` survive a
//!   `node_update` — they are orthogonal-axis state.
//! - `edge_create` — upsert an edge (Welle 5)
//! - `annotation_add` — append an annotation to an existing entity
//!   (V2-β Welle 14)
//! - `policy_set` — attach a policy reference to an existing entity
//!   (V2-β Welle 14; last-write-wins per policy_id)
//! - `anchor_created` — record a Sigstore Rekor anchor for a previously-
//!   emitted event (V2-β Welle 14; append-only — duplicate
//!   anchor for same event_id surfaces error)
//!
//! Any other payload `type` produces `ProjectorError::UnsupportedEventKind`.
//! Caller decides whether to abort or skip.
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
use crate::state::{AnchorEntry, AnnotationEntry, GraphEdge, GraphNode, GraphState, PolicyEntry};

/// Apply a single Atlas event to the projection state.
///
/// Dispatches on the event's `payload.type` discriminator:
///
/// - `node_create` → upsert a node from `payload.node`
/// - `node_update` → upsert a node from `payload.node_id` + `payload.patch`
/// - `edge_create` → upsert an edge from `payload.from` + `payload.to` + `payload.relation`
/// - `annotation_add` → append an annotation to an existing entity
///   (V2-β Welle 14). Requires `payload.entity_uuid`,
///   `payload.annotation_kind`, `payload.annotation_body`.
/// - `policy_set` → attach/replace a policy on an entity (V2-β Welle 14).
///   Requires `payload.entity_uuid` and `payload.policy_id`;
///   `payload.policy_version` optional (defaults to `"v1"`).
/// - `anchor_created` → record a Rekor anchor for `payload.event_id`
///   (V2-β Welle 14). Requires `rekor_log_index`, `rekor_log_id`,
///   `anchored_at`; `rekor_tree_id` optional. Re-anchoring the same
///   event surfaces a structured error (Sigstore log entries are
///   append-only by spec).
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
        // V2-β Welle 14: expanded event-kind dispatch.
        "annotation_add" => apply_annotation_add(event, state),
        "policy_set" => apply_policy_set(event, state),
        "anchor_created" => apply_anchor_created(event, state),
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
        // V2-β Welle 14: new nodes start with no annotations / policies.
        // These maps are populated only by `annotation_add` / `policy_set`
        // events targeting the entity AFTER its initial creation.
        annotations: BTreeMap::new(),
        policies: BTreeMap::new(),
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

    // V2-β Welle 14: preserve any annotations/policies that were
    // attached to this entity by prior `annotation_add` / `policy_set`
    // events. V2-α-MVP node_update semantics REPLACE labels+properties
    // entirely — but annotations + policies are orthogonal-axis state
    // (per W14 design), so they survive a node_update. If no prior
    // node exists, both maps start empty.
    let (preserved_annotations, preserved_policies) = state
        .nodes
        .get(&entity_uuid)
        .map(|n| (n.annotations.clone(), n.policies.clone()))
        .unwrap_or_default();

    state.upsert_node(GraphNode {
        entity_uuid,
        labels,
        properties,
        event_uuid: event.event_id.clone(),
        rekor_log_index: 0,
        author_did: event.author_did.clone(),
        annotations: preserved_annotations,
        policies: preserved_policies,
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

/// V2-β Welle 14: `annotation_add` dispatch arm.
///
/// Payload shape:
/// ```json
/// {
///   "type": "annotation_add",
///   "entity_uuid": "<uuid>",
///   "annotation_kind": "<string>",
///   "annotation_body": "<string>"
/// }
/// ```
///
/// The entity MUST exist in current state. If not, surfaces
/// `ProjectorError::MissingPayloadField` with a `field` value that
/// disambiguates "entity not found" from "field absent" — preserves
/// the `#[non_exhaustive]` enum shape (no new variants).
///
/// Multiple annotations under the same `annotation_kind` accumulate
/// in event-arrival order. Idempotency: replaying the same events
/// (the same JSONL trace) produces the same `Vec` order →
/// byte-identical canonical bytes.
fn apply_annotation_add(event: &AtlasEvent, state: &mut GraphState) -> ProjectorResult<()> {
    let payload_obj =
        event
            .payload
            .as_object()
            .ok_or_else(|| ProjectorError::MissingPayloadField {
                event_id: event.event_id.clone(),
                field: "payload (expected JSON object)".to_string(),
            })?;

    let entity_uuid =
        payload_obj
            .get("entity_uuid")
            .and_then(Value::as_str)
            .ok_or_else(|| ProjectorError::MissingPayloadField {
                event_id: event.event_id.clone(),
                field: "entity_uuid".to_string(),
            })?;

    let annotation_kind = payload_obj
        .get("annotation_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "annotation_kind".to_string(),
        })?
        .to_string();

    let annotation_body = payload_obj
        .get("annotation_body")
        .and_then(Value::as_str)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "annotation_body".to_string(),
        })?
        .to_string();

    let node = state.nodes.get_mut(entity_uuid).ok_or_else(|| {
        ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: format!("entity_uuid '{entity_uuid}' (not found in state)"),
        }
    })?;

    let entry = AnnotationEntry {
        body: annotation_body,
        event_uuid: event.event_id.clone(),
        author_did: event.author_did.clone(),
    };
    node.annotations
        .entry(annotation_kind)
        .or_default()
        .push(entry);
    Ok(())
}

/// V2-β Welle 14: `policy_set` dispatch arm.
///
/// Payload shape:
/// ```json
/// {
///   "type": "policy_set",
///   "entity_uuid": "<uuid>",
///   "policy_id": "<string>",
///   "policy_version": "<string>"   // optional; default "v1"
/// }
/// ```
///
/// Idempotent — last-write-wins per `policy_id`. The entity MUST exist.
fn apply_policy_set(event: &AtlasEvent, state: &mut GraphState) -> ProjectorResult<()> {
    let payload_obj =
        event
            .payload
            .as_object()
            .ok_or_else(|| ProjectorError::MissingPayloadField {
                event_id: event.event_id.clone(),
                field: "payload (expected JSON object)".to_string(),
            })?;

    let entity_uuid =
        payload_obj
            .get("entity_uuid")
            .and_then(Value::as_str)
            .ok_or_else(|| ProjectorError::MissingPayloadField {
                event_id: event.event_id.clone(),
                field: "entity_uuid".to_string(),
            })?;

    let policy_id = payload_obj
        .get("policy_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "policy_id".to_string(),
        })?
        .to_string();

    // Optional field — defaults to "v1" if absent.
    let policy_version = payload_obj
        .get("policy_version")
        .and_then(Value::as_str)
        .unwrap_or("v1")
        .to_string();

    let node = state.nodes.get_mut(entity_uuid).ok_or_else(|| {
        ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: format!("entity_uuid '{entity_uuid}' (not found in state)"),
        }
    })?;

    let entry = PolicyEntry {
        policy_version,
        event_uuid: event.event_id.clone(),
        author_did: event.author_did.clone(),
    };
    node.policies.insert(policy_id, entry);
    Ok(())
}

/// V2-β Welle 14: `anchor_created` dispatch arm.
///
/// Payload shape:
/// ```json
/// {
///   "type": "anchor_created",
///   "event_id": "<ulid>",
///   "rekor_log_index": <u64>,
///   "rekor_log_id": "<string>",
///   "rekor_tree_id": <u64>,         // optional
///   "anchored_at": "<iso8601>"
/// }
/// ```
///
/// **Security-conservative policy:** anchoring the same `event_id` a
/// second time surfaces `ProjectorError::MissingPayloadField` with a
/// `field` value documenting the duplicate. Sigstore transparency-log
/// entries are append-only by spec; a second anchor for the same event
/// with a different log-index would indicate tampering or replay-attack.
/// Erroring forces operator inspection rather than silently last-write-
/// wins. Uses existing `ProjectorError` variant to preserve
/// `#[non_exhaustive]` enum shape.
fn apply_anchor_created(event: &AtlasEvent, state: &mut GraphState) -> ProjectorResult<()> {
    let payload_obj =
        event
            .payload
            .as_object()
            .ok_or_else(|| ProjectorError::MissingPayloadField {
                event_id: event.event_id.clone(),
                field: "payload (expected JSON object)".to_string(),
            })?;

    let anchored_event_id = payload_obj
        .get("event_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "event_id".to_string(),
        })?
        .to_string();

    if anchored_event_id.is_empty() {
        return Err(ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "event_id (empty string)".to_string(),
        });
    }

    let rekor_log_index = payload_obj
        .get("rekor_log_index")
        .and_then(Value::as_u64)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "rekor_log_index".to_string(),
        })?;

    let rekor_log_id = payload_obj
        .get("rekor_log_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "rekor_log_id".to_string(),
        })?
        .to_string();

    let anchored_at = payload_obj
        .get("anchored_at")
        .and_then(Value::as_str)
        .ok_or_else(|| ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: "anchored_at".to_string(),
        })?
        .to_string();

    // Optional: rekor_tree_id (CT-style tree identity).
    let rekor_tree_id = payload_obj.get("rekor_tree_id").and_then(Value::as_u64);

    // Security-conservative: refuse to re-anchor.
    if state.rekor_anchors.contains_key(&anchored_event_id) {
        return Err(ProjectorError::MissingPayloadField {
            event_id: event.event_id.clone(),
            field: format!(
                "event_id '{anchored_event_id}' already has a rekor anchor (duplicate refused for security)"
            ),
        });
    }

    let anchor = AnchorEntry {
        rekor_log_index,
        rekor_log_id,
        rekor_tree_id,
        anchored_at,
        author_did: event.author_did.clone(),
    };
    state.upsert_anchor(anchored_event_id, anchor);
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
        // V2-β Welle 14: `policy_set` / `annotation_add` / `anchor_created`
        // are now SUPPORTED kinds. We need a kind that is still
        // unsupported to exercise the fallthrough error arm. Use a
        // deliberately V2-γ-shaped placeholder kind.
        let event = make_event(
            "01HEVENT1",
            json!({"type": "future_v2_gamma_kind", "something": "..."}),
            None,
        );
        let mut state = GraphState::new();
        match apply_event_to_state(WS, &event, &mut state) {
            Err(ProjectorError::UnsupportedEventKind { kind, .. }) => {
                assert_eq!(kind, "future_v2_gamma_kind");
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
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
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

    // ============================================================
    // V2-β Welle 14: expanded event-kind support tests
    // ============================================================

    /// Helper: seed state with a single node by running a `node_create`
    /// event through the dispatch. Keeps the W14 tests honest — they
    /// exercise the real dispatch pipeline rather than direct upsert.
    fn seed_node(state: &mut GraphState, entity_uuid: &str) {
        let create = make_event(
            &format!("01HSEED-{entity_uuid}"),
            json!({"type": "node_create", "node": {"id": entity_uuid}}),
            None,
        );
        apply_event_to_state(WS, &create, state).expect("seed must succeed");
    }

    #[test]
    fn annotation_add_appends_to_entity() {
        let mut state = GraphState::new();
        seed_node(&mut state, "alice");

        let event = make_event(
            "01HANN1",
            json!({
                "type": "annotation_add",
                "entity_uuid": "alice",
                "annotation_kind": "human_note",
                "annotation_body": "needs review"
            }),
            None,
        );
        apply_event_to_state(WS, &event, &mut state).unwrap();

        let node = &state.nodes["alice"];
        let notes = node
            .annotations
            .get("human_note")
            .expect("human_note kind should exist");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].body, "needs review");
        assert_eq!(notes[0].event_uuid, "01HANN1");

        // Idempotency check: re-projecting the same events into a
        // fresh state must produce a byte-identical graph_state_hash.
        let mut state2 = GraphState::new();
        seed_node(&mut state2, "alice");
        apply_event_to_state(WS, &event, &mut state2).unwrap();
        assert_eq!(
            crate::canonical::graph_state_hash(&state).unwrap(),
            crate::canonical::graph_state_hash(&state2).unwrap()
        );
    }

    #[test]
    fn annotation_add_on_missing_entity_errors() {
        let mut state = GraphState::new();
        // NO seed — annotation_add on a missing entity must error.
        let event = make_event(
            "01HANN1",
            json!({
                "type": "annotation_add",
                "entity_uuid": "ghost",
                "annotation_kind": "k",
                "annotation_body": "b"
            }),
            None,
        );
        match apply_event_to_state(WS, &event, &mut state) {
            Err(ProjectorError::MissingPayloadField { field, .. }) => {
                assert!(
                    field.contains("ghost") && field.contains("not found"),
                    "field must disambiguate entity-not-found; got {field}"
                );
            }
            other => panic!("expected MissingPayloadField; got {other:?}"),
        }
    }

    #[test]
    fn annotation_add_multiple_kinds_each_independently_appended() {
        let mut state = GraphState::new();
        seed_node(&mut state, "alice");

        let e1 = make_event(
            "01HANN1",
            json!({
                "type": "annotation_add",
                "entity_uuid": "alice",
                "annotation_kind": "human_note",
                "annotation_body": "first"
            }),
            None,
        );
        let e2 = make_event(
            "01HANN2",
            json!({
                "type": "annotation_add",
                "entity_uuid": "alice",
                "annotation_kind": "ml_tag",
                "annotation_body": "category:A"
            }),
            None,
        );
        let e3 = make_event(
            "01HANN3",
            json!({
                "type": "annotation_add",
                "entity_uuid": "alice",
                "annotation_kind": "human_note",
                "annotation_body": "second"
            }),
            None,
        );

        for e in [&e1, &e2, &e3] {
            apply_event_to_state(WS, e, &mut state).unwrap();
        }

        let node = &state.nodes["alice"];
        assert_eq!(node.annotations.len(), 2, "two distinct kinds expected");
        let human = &node.annotations["human_note"];
        assert_eq!(human.len(), 2);
        assert_eq!(human[0].body, "first");
        assert_eq!(human[1].body, "second");
        assert_eq!(node.annotations["ml_tag"].len(), 1);
    }

    #[test]
    fn annotation_add_missing_payload_field_surfaces_error() {
        let mut state = GraphState::new();
        seed_node(&mut state, "alice");
        // Missing annotation_body.
        let event = make_event(
            "01HANN1",
            json!({
                "type": "annotation_add",
                "entity_uuid": "alice",
                "annotation_kind": "k"
            }),
            None,
        );
        match apply_event_to_state(WS, &event, &mut state) {
            Err(ProjectorError::MissingPayloadField { field, .. }) => {
                assert_eq!(field, "annotation_body");
            }
            other => panic!("expected MissingPayloadField; got {other:?}"),
        }
    }

    #[test]
    fn policy_set_attaches_policy() {
        let mut state = GraphState::new();
        seed_node(&mut state, "alice");

        let event = make_event(
            "01HPOL1",
            json!({
                "type": "policy_set",
                "entity_uuid": "alice",
                "policy_id": "data_residency_eu",
                "policy_version": "v2"
            }),
            None,
        );
        apply_event_to_state(WS, &event, &mut state).unwrap();

        let node = &state.nodes["alice"];
        let policy = node
            .policies
            .get("data_residency_eu")
            .expect("policy must be attached");
        assert_eq!(policy.policy_version, "v2");
        assert_eq!(policy.event_uuid, "01HPOL1");

        // Reproducibility / hash stability.
        let mut state2 = GraphState::new();
        seed_node(&mut state2, "alice");
        apply_event_to_state(WS, &event, &mut state2).unwrap();
        assert_eq!(
            crate::canonical::graph_state_hash(&state).unwrap(),
            crate::canonical::graph_state_hash(&state2).unwrap()
        );
    }

    #[test]
    fn policy_set_defaults_version_to_v1_when_omitted() {
        let mut state = GraphState::new();
        seed_node(&mut state, "alice");
        let event = make_event(
            "01HPOL1",
            json!({
                "type": "policy_set",
                "entity_uuid": "alice",
                "policy_id": "p1"
                // policy_version omitted
            }),
            None,
        );
        apply_event_to_state(WS, &event, &mut state).unwrap();
        assert_eq!(state.nodes["alice"].policies["p1"].policy_version, "v1");
    }

    #[test]
    fn policy_set_idempotent_with_last_write_wins() {
        let mut state = GraphState::new();
        seed_node(&mut state, "alice");

        let e1 = make_event(
            "01HPOL1",
            json!({
                "type": "policy_set",
                "entity_uuid": "alice",
                "policy_id": "p1",
                "policy_version": "v1"
            }),
            None,
        );
        let e2 = make_event(
            "01HPOL2",
            json!({
                "type": "policy_set",
                "entity_uuid": "alice",
                "policy_id": "p1",
                "policy_version": "v2"
            }),
            None,
        );
        apply_event_to_state(WS, &e1, &mut state).unwrap();
        apply_event_to_state(WS, &e2, &mut state).unwrap();
        // Last-write-wins per policy_id.
        assert_eq!(state.nodes["alice"].policies.len(), 1);
        assert_eq!(state.nodes["alice"].policies["p1"].policy_version, "v2");
        assert_eq!(state.nodes["alice"].policies["p1"].event_uuid, "01HPOL2");
    }

    #[test]
    fn policy_set_on_missing_entity_errors() {
        let mut state = GraphState::new();
        let event = make_event(
            "01HPOL1",
            json!({
                "type": "policy_set",
                "entity_uuid": "ghost",
                "policy_id": "p1"
            }),
            None,
        );
        match apply_event_to_state(WS, &event, &mut state) {
            Err(ProjectorError::MissingPayloadField { field, .. }) => {
                assert!(
                    field.contains("ghost") && field.contains("not found"),
                    "field must disambiguate entity-not-found; got {field}"
                );
            }
            other => panic!("expected MissingPayloadField; got {other:?}"),
        }
    }

    #[test]
    fn anchor_created_records_anchor() {
        // anchor_created records an anchor for an event_id — note that
        // the anchored event_id is a logical Layer-1 reference; it does
        // NOT need to be a node in the graph (anchors are about events).
        let mut state = GraphState::new();

        let event = make_event(
            "01HANCHOR1",
            json!({
                "type": "anchor_created",
                "event_id": "01HEVENT-TARGET",
                "rekor_log_index": 4242_u64,
                "rekor_log_id": "rekor.sigstore.dev",
                "rekor_tree_id": 1_u64,
                "anchored_at": "2026-05-13T10:00:00Z"
            }),
            None,
        );
        apply_event_to_state(WS, &event, &mut state).unwrap();

        let anchor = state
            .rekor_anchors
            .get("01HEVENT-TARGET")
            .expect("anchor must be recorded");
        assert_eq!(anchor.rekor_log_index, 4242);
        assert_eq!(anchor.rekor_log_id, "rekor.sigstore.dev");
        assert_eq!(anchor.rekor_tree_id, Some(1));
        assert_eq!(anchor.anchored_at, "2026-05-13T10:00:00Z");

        // Reproducibility / hash stability.
        let mut state2 = GraphState::new();
        apply_event_to_state(WS, &event, &mut state2).unwrap();
        assert_eq!(
            crate::canonical::graph_state_hash(&state).unwrap(),
            crate::canonical::graph_state_hash(&state2).unwrap()
        );
    }

    #[test]
    fn anchor_created_for_same_event_twice_errors() {
        // Security-conservative policy: Sigstore transparency-log
        // entries are append-only. A second anchor for the same
        // event with different (or same) log-index is refused.
        let mut state = GraphState::new();
        let e1 = make_event(
            "01HANCHOR1",
            json!({
                "type": "anchor_created",
                "event_id": "01HEVENT-TARGET",
                "rekor_log_index": 1_u64,
                "rekor_log_id": "rekor.sigstore.dev",
                "anchored_at": "2026-05-13T10:00:00Z"
            }),
            None,
        );
        let e2 = make_event(
            "01HANCHOR2",
            json!({
                "type": "anchor_created",
                "event_id": "01HEVENT-TARGET",
                "rekor_log_index": 2_u64,
                "rekor_log_id": "rekor.sigstore.dev",
                "anchored_at": "2026-05-13T10:01:00Z"
            }),
            None,
        );
        apply_event_to_state(WS, &e1, &mut state).unwrap();
        match apply_event_to_state(WS, &e2, &mut state) {
            Err(ProjectorError::MissingPayloadField { field, .. }) => {
                assert!(
                    field.contains("already has a rekor anchor"),
                    "duplicate-anchor error must be self-describing; got {field}"
                );
            }
            other => panic!("expected duplicate-anchor refusal; got {other:?}"),
        }
        // First anchor remains untouched.
        assert_eq!(
            state.rekor_anchors["01HEVENT-TARGET"].rekor_log_index,
            1
        );
    }

    #[test]
    fn anchor_created_optional_tree_id_works_when_absent() {
        let mut state = GraphState::new();
        let event = make_event(
            "01HANCHOR1",
            json!({
                "type": "anchor_created",
                "event_id": "01HEVENT-TARGET",
                "rekor_log_index": 1_u64,
                "rekor_log_id": "rekor.sigstore.dev",
                "anchored_at": "2026-05-13T10:00:00Z"
                // rekor_tree_id omitted
            }),
            None,
        );
        apply_event_to_state(WS, &event, &mut state).unwrap();
        assert_eq!(state.rekor_anchors["01HEVENT-TARGET"].rekor_tree_id, None);
    }

    #[test]
    fn canonical_state_hash_unchanged_for_v1_traces() {
        // V2-β Welle 14 byte-determinism preservation invariant: a
        // V1 / V2-α-shape trace (no W14 event-kinds) must produce
        // a state with empty annotations / policies / rekor_anchors
        // — which in turn must canonicalise to BYTE-IDENTICAL output
        // as pre-W14. Empty fields are omitted from canonical CBOR.
        //
        // The existing `canonical::tests::graph_state_hash_byte_determinism_pin`
        // is the load-bearing pin; this test additionally exercises the
        // event-projection path end-to-end.
        let events = vec![
            make_event(
                "01HE001",
                json!({"type": "node_create", "node": {"id": "alice", "labels": ["Person"], "name": "Alice"}}),
                None,
            ),
            make_event(
                "01HE002",
                json!({"type": "node_create", "node": {"id": "bob", "labels": ["Person"], "name": "Bob"}}),
                None,
            ),
            make_event(
                "01HE003",
                json!({"type": "edge_create", "from": "alice", "to": "bob", "relation": "knows"}),
                None,
            ),
        ];
        let state = project_events(WS, &events, None).unwrap();
        // Sanity: no W14 fields populated.
        assert!(state.rekor_anchors.is_empty());
        assert!(state.nodes.values().all(|n| n.annotations.is_empty()));
        assert!(state.nodes.values().all(|n| n.policies.is_empty()));

        // Canonical bytes for a V1-shape state MUST NOT contain any
        // W14 field name. This catches accidental emission of empty
        // CBOR maps for these fields (which would still drift the
        // byte-pin and pass an "empty" but break the canonical layout).
        let bytes = crate::canonical::build_canonical_bytes(&state).unwrap();
        for field_name in [b"annotations".as_slice(), b"policies".as_slice(), b"rekor_anchors".as_slice()] {
            assert!(
                !bytes.windows(field_name.len()).any(|w| w == field_name),
                "V1-shape canonical bytes must omit W14 field {}",
                std::str::from_utf8(field_name).unwrap()
            );
        }
    }

    #[test]
    fn node_update_preserves_annotations_and_policies() {
        // V2-β Welle 14 amendment: `node_update` REPLACES labels +
        // properties but PRESERVES annotations + policies as
        // orthogonal-axis state. Documented in the apply_node_update
        // implementation comments.
        let mut state = GraphState::new();
        seed_node(&mut state, "alice");

        // Attach an annotation and a policy.
        apply_event_to_state(
            WS,
            &make_event(
                "01HANN1",
                json!({
                    "type": "annotation_add",
                    "entity_uuid": "alice",
                    "annotation_kind": "human_note",
                    "annotation_body": "important"
                }),
                None,
            ),
            &mut state,
        )
        .unwrap();
        apply_event_to_state(
            WS,
            &make_event(
                "01HPOL1",
                json!({
                    "type": "policy_set",
                    "entity_uuid": "alice",
                    "policy_id": "p1"
                }),
                None,
            ),
            &mut state,
        )
        .unwrap();

        // Now node_update the entity — annotations + policies must survive.
        apply_event_to_state(
            WS,
            &make_event(
                "01HUPD1",
                json!({
                    "type": "node_update",
                    "node_id": "alice",
                    "patch": {"labels": ["Person", "VIP"], "name": "Alice Updated"}
                }),
                None,
            ),
            &mut state,
        )
        .unwrap();

        let node = &state.nodes["alice"];
        // labels + properties replaced as per V2-α-MVP semantics
        assert_eq!(node.labels, vec!["Person".to_string(), "VIP".to_string()]);
        // annotations + policies preserved
        assert_eq!(node.annotations["human_note"].len(), 1);
        assert!(node.policies.contains_key("p1"));
    }

    #[test]
    fn anchor_created_missing_event_id_errors() {
        let mut state = GraphState::new();
        let event = make_event(
            "01HANCHOR1",
            json!({
                "type": "anchor_created",
                "rekor_log_index": 1_u64,
                "rekor_log_id": "rekor.sigstore.dev",
                "anchored_at": "2026-05-13T10:00:00Z"
                // event_id missing
            }),
            None,
        );
        match apply_event_to_state(WS, &event, &mut state) {
            Err(ProjectorError::MissingPayloadField { field, .. }) => {
                assert_eq!(field, "event_id");
            }
            other => panic!("expected MissingPayloadField; got {other:?}"),
        }
    }
}
