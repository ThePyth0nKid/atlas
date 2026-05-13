//! V2-α Welle 5 integration test — end-to-end projector pipeline.
//!
//! Exercises the full chain: events.jsonl text → parse → project →
//! emit attestation payload → re-parse-and-validate via atlas-trust-core
//! Welle 4. This is the **load-bearing E2E contract** that bridges
//! all 5 V2-α wellen into one demonstrable pipeline.
//!
//! Test cases:
//!   - Full happy path: 5-event JSONL → projected GraphState →
//!     attestation payload → round-trip through atlas-trust-core's
//!     parser + validator → both green
//!   - Idempotency: project the same JSONL twice into separate
//!     states → byte-identical `graph_state_hash`
//!   - Unsupported event kind: `policy_set` event surfaces
//!     `UnsupportedEventKind` cleanly
//!   - Malformed JSONL line: surfaces `ReplayMalformed` with 1-indexed
//!     line number

use atlas_projector::{
    build_projector_run_attestation_payload, graph_state_hash, parse_events_jsonl,
    project_events, GraphState, ProjectorError,
};
use atlas_trust_core::{
    parse_projector_run_attestation, validate_projector_run_attestation,
};

const WS: &str = "ws-e2e";
const FIXTURE_HEAD: &str = "fa11a11ea11a11ea11a11ea11a11ea11a11ea11ea11ea11ea11ea11ea11ea11e";

/// 5-event JSONL fixture: 3 node_create + 1 node_update + 1 edge_create.
/// V1-shape events; no `author_did` (V1 backward-compat path).
fn fixture_jsonl() -> String {
    [
        r#"{"event_id":"01HE001","event_hash":"h1","parent_hashes":[],"payload":{"type":"node_create","node":{"id":"alice","labels":["Person"],"name":"Alice"}},"signature":{"alg":"EdDSA","kid":"atlas-anchor:ws-e2e","sig":"AA"},"ts":"2026-05-13T10:00:00Z"}"#,
        r#"{"event_id":"01HE002","event_hash":"h2","parent_hashes":["h1"],"payload":{"type":"node_create","node":{"id":"bob","labels":["Person"],"name":"Bob"}},"signature":{"alg":"EdDSA","kid":"atlas-anchor:ws-e2e","sig":"BB"},"ts":"2026-05-13T10:01:00Z"}"#,
        r#"{"event_id":"01HE003","event_hash":"h3","parent_hashes":["h2"],"payload":{"type":"node_create","node":{"id":"dataset","labels":["Dataset"]}},"signature":{"alg":"EdDSA","kid":"atlas-anchor:ws-e2e","sig":"CC"},"ts":"2026-05-13T10:02:00Z"}"#,
        r#"{"event_id":"01HE004","event_hash":"h4","parent_hashes":["h3"],"payload":{"type":"node_update","node_id":"alice","patch":{"labels":["Person","VIP"],"name":"Alice"}},"signature":{"alg":"EdDSA","kid":"atlas-anchor:ws-e2e","sig":"DD"},"ts":"2026-05-13T10:03:00Z"}"#,
        r#"{"event_id":"01HE005","event_hash":"h5","parent_hashes":["h4"],"payload":{"type":"edge_create","from":"alice","to":"dataset","relation":"owns"},"signature":{"alg":"EdDSA","kid":"atlas-anchor:ws-e2e","sig":"EE"},"ts":"2026-05-13T10:04:00Z"}"#,
    ].join("\n")
}

#[test]
fn full_pipeline_e2e_jsonl_to_attested_state() {
    // Step 1: parse the JSONL into AtlasEvents.
    let jsonl = fixture_jsonl();
    let events = parse_events_jsonl(&jsonl).expect("parse_events_jsonl failed");
    assert_eq!(events.len(), 5);

    // Step 2: project into a fresh GraphState.
    let state = project_events(WS, &events, None).expect("project_events failed");
    assert_eq!(state.nodes.len(), 3); // alice (updated), bob, dataset
    assert_eq!(state.edges.len(), 1);

    // Step 3: spot-check structural-integrity (edge endpoints exist as nodes).
    state
        .check_structural_integrity()
        .expect("integrity check must pass");

    // Step 4: emit ProjectorRunAttestation payload.
    let payload = build_projector_run_attestation_payload(
        &state,
        "atlas-projector/0.1.0",
        FIXTURE_HEAD,
        events.len() as u64,
    )
    .expect("emission failed");

    // Step 5: round-trip through atlas-trust-core's parser + validator.
    let attestation =
        parse_projector_run_attestation(&payload).expect("parse_projector_run_attestation failed");
    validate_projector_run_attestation(&attestation)
        .expect("validate_projector_run_attestation failed");

    // Step 6: verify the attestation's graph_state_hash equals the
    // canonicaliser's output for the same state. This is the
    // cryptographic chain-of-custody contract between Welle 5
    // (emission) and Welle 3 (canonicalisation).
    let direct_hash = graph_state_hash(&state).expect("graph_state_hash failed");
    assert_eq!(attestation.graph_state_hash, hex::encode(direct_hash));
    assert_eq!(attestation.projected_event_count, 5);
    assert_eq!(attestation.head_event_hash, FIXTURE_HEAD);
}

#[test]
fn idempotency_same_events_twice_byte_identical_state_hash() {
    // Welle 5 idempotency invariant + Welle 2 §3.5 insert-order
    // independence: projecting the same events into two separate
    // states produces a byte-identical graph_state_hash.
    let jsonl = fixture_jsonl();
    let events = parse_events_jsonl(&jsonl).unwrap();

    let s1 = project_events(WS, &events, None).unwrap();
    let s2 = project_events(WS, &events, None).unwrap();
    let h1 = graph_state_hash(&s1).unwrap();
    let h2 = graph_state_hash(&s2).unwrap();

    assert_eq!(h1, h2, "idempotency invariant violated");
}

#[test]
fn unsupported_event_kind_surfaces_structured_error() {
    // V2-β Welle 14 update: `policy_set` is now a supported kind.
    // Use a deliberately V2-γ-shaped placeholder kind that remains
    // unsupported, preserving this regression test's intent.
    let jsonl = r#"{"event_id":"01HE001","event_hash":"h1","parent_hashes":[],"payload":{"type":"future_v2_gamma_kind","payload":"..."},"signature":{"alg":"EdDSA","kid":"atlas-anchor:ws-e2e","sig":"AA"},"ts":"2026-05-13T10:00:00Z"}"#;
    let events = parse_events_jsonl(jsonl).unwrap();
    match project_events(WS, &events, None) {
        Err(ProjectorError::UnsupportedEventKind { kind, event_id }) => {
            assert_eq!(kind, "future_v2_gamma_kind");
            assert_eq!(event_id, "01HE001");
        }
        other => panic!("expected UnsupportedEventKind; got {other:?}"),
    }
}

#[test]
fn malformed_jsonl_surfaces_line_number() {
    let jsonl = format!(
        "{}\n{}\n",
        // line 1: valid event
        r#"{"event_id":"01HE001","event_hash":"h1","parent_hashes":[],"payload":{"type":"node_create","node":{"id":"n1"}},"signature":{"alg":"EdDSA","kid":"atlas-anchor:ws-e2e","sig":"AA"},"ts":"2026-05-13T10:00:00Z"}"#,
        // line 2: garbage
        "this-is-not-json"
    );
    match parse_events_jsonl(&jsonl) {
        Err(ProjectorError::ReplayMalformed { line_number, .. }) => {
            assert_eq!(line_number, 2, "expected 1-indexed line 2");
        }
        other => panic!("expected ReplayMalformed; got {other:?}"),
    }
}

#[test]
fn empty_jsonl_produces_empty_state_and_emission_rejects_zero_count() {
    // Edge case: empty event list → empty state → emission with
    // projected_event_count = 0 must fail (Welle 5 enforces count >= 1
    // matching Welle 4's validator).
    let events: Vec<atlas_trust_core::trace_format::AtlasEvent> = vec![];
    let state = project_events(WS, &events, None).unwrap();
    assert!(state.nodes.is_empty());
    assert!(state.edges.is_empty());

    match build_projector_run_attestation_payload(
        &state,
        "atlas-projector/0.1.0",
        FIXTURE_HEAD,
        0, // zero count — should fail
    ) {
        Err(ProjectorError::CanonicalisationFailed(reason)) => {
            assert!(reason.contains("projected_event_count must be >= 1"));
        }
        other => panic!("expected CanonicalisationFailed; got {other:?}"),
    }

    // But with count >= 1, an empty-state attestation IS allowed
    // (operator may attest "I projected 1 event and the state is
    // empty because the event was a no-op kind we don't support").
    let _ok_payload = build_projector_run_attestation_payload(
        &GraphState::new(),
        "atlas-projector/0.1.0",
        FIXTURE_HEAD,
        1,
    )
    .expect("empty-state attestation with count>=1 must succeed");
}

#[test]
fn pipeline_preserves_existing_state() {
    // Operator scenario: resume projection from a checkpoint state +
    // new events. The pipeline must extend (not replace) the existing
    // state.
    let mut existing = GraphState::new();
    use atlas_projector::GraphNode;
    use std::collections::BTreeMap;
    existing.upsert_node(GraphNode {
        entity_uuid: "checkpoint-node".to_string(),
        labels: vec![],
        properties: BTreeMap::new(),
        event_uuid: "01HEPREV".to_string(),
        rekor_log_index: 0,
        author_did: None,
        annotations: BTreeMap::new(),
        policies: BTreeMap::new(),
    });

    let jsonl = fixture_jsonl();
    let events = parse_events_jsonl(&jsonl).unwrap();
    let state = project_events(WS, &events, Some(existing)).unwrap();

    assert_eq!(state.nodes.len(), 4); // 1 checkpoint + 3 from JSONL
    assert!(state.nodes.contains_key("checkpoint-node"));
    assert!(state.nodes.contains_key("alice"));
}
