//! V2-α Welle 6 integration tests — projector-state-hash CI gate.
//!
//! Exercises the full chain: synthetic events.jsonl with both
//! projectable events AND a ProjectorRunAttestation event → gate
//! re-projects → compares attested vs recomputed → produces
//! per-attestation `GateResult`s. Tests cover the happy path,
//! both tampering vectors (hash mismatch + count mismatch),
//! malformed attestation payload, multiple attestations in one
//! trace, and absence of attestations.

use atlas_projector::{
    build_projector_run_attestation_payload, graph_state_hash, parse_events_jsonl,
    project_events, verify_attestations_in_trace, GateStatus, GraphState,
};
use atlas_trust_core::trace_format::{AtlasEvent, AtlasTrace, EventSignature};
use serde_json::{json, Value};

const WS: &str = "ws-gate-test";
const FIXTURE_HEAD: &str = "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b";

fn make_event(event_id: &str, payload: Value) -> AtlasEvent {
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
        author_did: None,
    }
}

fn make_trace(events: Vec<AtlasEvent>) -> AtlasTrace {
    AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-05-13T10:00:00Z".to_string(),
        workspace_id: WS.to_string(),
        pubkey_bundle_hash: "h".to_string(),
        events,
        dag_tips: vec![],
        anchors: vec![],
        anchor_chain: None,
        policies: vec![],
        filters: None,
    }
}

/// 3 projectable events forming a small graph.
fn projectable_events() -> Vec<AtlasEvent> {
    vec![
        make_event(
            "01HEV1",
            json!({"type": "node_create", "node": {"id": "alice", "name": "Alice"}}),
        ),
        make_event(
            "01HEV2",
            json!({"type": "node_create", "node": {"id": "bob", "name": "Bob"}}),
        ),
        make_event(
            "01HEV3",
            json!({"type": "edge_create", "from": "alice", "to": "bob", "relation": "knows"}),
        ),
    ]
}

/// Build a well-formed attestation event for the given projection
/// state + event count. The signed `event_hash` and `signature` are
/// fixture placeholders — Welle 6 gate does NOT re-verify signatures
/// (caller's responsibility upstream).
fn make_attestation_event(
    event_id: &str,
    state: &GraphState,
    projected_count: u64,
) -> AtlasEvent {
    let payload = build_projector_run_attestation_payload(
        state,
        "atlas-projector/0.1.0",
        FIXTURE_HEAD,
        projected_count,
    )
    .expect("emission failed");
    make_event(event_id, payload)
}

#[test]
fn happy_path_attestation_matches_reprojection() {
    let events = projectable_events();
    let state = project_events(WS, &events, None).unwrap();
    let attestation = make_attestation_event("01HATT1", &state, events.len() as u64);

    let mut all_events = events.clone();
    all_events.push(attestation);
    let trace = make_trace(all_events);

    let results = verify_attestations_in_trace(WS, &trace).unwrap();
    assert_eq!(results.len(), 1, "exactly one attestation in trace");
    let r = &results[0];
    assert_eq!(r.event_id, "01HATT1");
    assert_eq!(r.status, GateStatus::Match);
    assert_eq!(r.attested_hash, r.recomputed_hash);
    assert_eq!(r.attested_event_count, r.actual_event_count);
    assert_eq!(r.actual_event_count, 3);
}

#[test]
fn tampered_attestation_hash_mismatch_detected() {
    let events = projectable_events();
    let state = project_events(WS, &events, None).unwrap();
    let mut attestation = make_attestation_event("01HATT1", &state, events.len() as u64);
    // Tamper the attested hash to a different (still-well-formed) value
    attestation.payload["graph_state_hash"] =
        json!("f00d0000000000000000000000000000000000000000000000000000000000f0");

    let mut all_events = events.clone();
    all_events.push(attestation);
    let trace = make_trace(all_events);

    let results = verify_attestations_in_trace(WS, &trace).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, GateStatus::Mismatch);
    assert_ne!(results[0].attested_hash, results[0].recomputed_hash);
    // Counts still match — only the hash is tampered.
    assert_eq!(results[0].attested_event_count, results[0].actual_event_count);
}

#[test]
fn mismatched_projected_event_count_detected() {
    let events = projectable_events();
    let state = project_events(WS, &events, None).unwrap();
    // Claim 99 events when actually 3 — count mismatch
    let attestation = make_attestation_event("01HATT1", &state, 99);

    let mut all_events = events.clone();
    all_events.push(attestation);
    let trace = make_trace(all_events);

    let results = verify_attestations_in_trace(WS, &trace).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, GateStatus::Mismatch);
    assert_eq!(results[0].attested_event_count, 99);
    assert_eq!(results[0].actual_event_count, 3);
    // Hash itself matches — only the count is wrong.
    assert_eq!(results[0].attested_hash, results[0].recomputed_hash);
}

#[test]
fn multiple_attestation_events_each_verified() {
    let events = projectable_events();
    let state = project_events(WS, &events, None).unwrap();
    let attestation1 = make_attestation_event("01HATT1", &state, events.len() as u64);
    let attestation2 = make_attestation_event("01HATT2", &state, events.len() as u64);

    let mut all_events = events.clone();
    all_events.push(attestation1);
    all_events.push(attestation2);
    let trace = make_trace(all_events);

    let results = verify_attestations_in_trace(WS, &trace).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].event_id, "01HATT1");
    assert_eq!(results[1].event_id, "01HATT2");
    assert_eq!(results[0].status, GateStatus::Match);
    assert_eq!(results[1].status, GateStatus::Match);
}

#[test]
fn malformed_attestation_payload_surfaces_parse_failed_status() {
    let events = projectable_events();
    // Construct an event with type=projector_run_attestation but
    // missing required fields → parse will fail.
    let bad_attestation = make_event(
        "01HATTBAD",
        json!({
            "type": "projector_run_attestation",
            "projector_version": "atlas-projector/0.1.0",
            // missing schema_version, head_event_hash, graph_state_hash, count
        }),
    );

    let mut all_events = events.clone();
    all_events.push(bad_attestation);
    let trace = make_trace(all_events);

    let results = verify_attestations_in_trace(WS, &trace).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, GateStatus::AttestationParseFailed);
    assert_eq!(results[0].event_id, "01HATTBAD");
    // For parse-failed status, attested_hash is empty.
    assert_eq!(results[0].attested_hash, "");
}

#[test]
fn trace_without_attestation_events_returns_empty_vec() {
    let events = projectable_events();
    let trace = make_trace(events);
    let results = verify_attestations_in_trace(WS, &trace).unwrap();
    assert!(
        results.is_empty(),
        "no attestation events → no GateResults, even though events were projectable"
    );
}

#[test]
fn unsupported_event_kind_in_trace_surfaces_error() {
    // V2-β Welle 14 update: `policy_set`, `annotation_add`, and
    // `anchor_created` are now SUPPORTED kinds. Use a deliberately
    // V2-γ-shaped placeholder kind that remains unsupported, so this
    // test continues to verify the fallthrough-to-error path.
    let mut events = projectable_events();
    events.push(make_event(
        "01HEVPOL",
        json!({"type": "future_v2_gamma_kind", "payload": "..."}),
    ));
    let state = project_events(WS, &events[..3], None).unwrap();
    let attestation = make_attestation_event("01HATT1", &state, 3);
    events.push(attestation);
    let trace = make_trace(events);

    let result = verify_attestations_in_trace(WS, &trace);
    assert!(
        result.is_err(),
        "trace with unsupported event kind must surface error"
    );
}

#[test]
fn semantically_invalid_attestation_surfaces_parse_failed_not_mismatch() {
    // V2-α Welle 6 review-pass regression test: a payload that
    // parses structurally but fails semantic validation (e.g.
    // wrong schema_version) must produce `AttestationParseFailed`
    // status — NOT silently surface as `Mismatch`. The earlier
    // gate skipped `validate_projector_run_attestation` and so
    // semantically-invalid payloads with the right shape would
    // proceed to hash comparison, masking the real failure mode.
    let events = projectable_events();
    let bad_attestation = make_event(
        "01HATTBAD",
        json!({
            "type": "projector_run_attestation",
            "projector_version": "atlas-projector/0.1.0",
            // Wrong schema_version → semantically invalid per Welle 4
            "projector_schema_version": "not-a-real-version",
            "head_event_hash": "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a",
            "graph_state_hash": "1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a",
            "projected_event_count": 3_u64,
        }),
    );

    let mut all_events = events.clone();
    all_events.push(bad_attestation);
    let trace = make_trace(all_events);
    let results = verify_attestations_in_trace(WS, &trace).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].status,
        GateStatus::AttestationParseFailed,
        "semantically invalid attestation must surface as ParseFailed (not Mismatch)"
    );
}

#[test]
fn zero_projectable_events_with_one_attestation_compares_against_empty_state() {
    // Edge case: trace with NO projectable events + one
    // attestation. Re-projection produces empty GraphState; gate
    // compares against the attested claim. Welle 4's
    // validate_projector_run_attestation rejects
    // projected_event_count == 0, so an honest attestation MUST
    // have count >= 1. If the issuer claims count >= 1 but the
    // trace has 0 projectable events → Mismatch (count differs).
    let empty_state = project_events(WS, &[], None).unwrap();
    let attestation = make_attestation_event("01HATTEMP", &empty_state, 1);

    let trace = make_trace(vec![attestation]);
    let results = verify_attestations_in_trace(WS, &trace).unwrap();
    assert_eq!(results.len(), 1);
    // Hash matches (empty state hash = recomputed empty state hash)
    // but count differs (attested 1, actual 0) → Mismatch
    assert_eq!(results[0].status, GateStatus::Mismatch);
    assert_eq!(results[0].attested_hash, results[0].recomputed_hash);
    assert_eq!(results[0].attested_event_count, 1);
    assert_eq!(results[0].actual_event_count, 0);
}

#[test]
fn end_to_end_jsonl_parse_project_emit_then_gate_verifies() {
    // Full lifecycle test: JSONL → parse → project → emit → assemble
    // into a trace alongside the source events → gate verifies match.
    //
    // This is the headline V2-α Welle 6 demonstration: third-party
    // verifier reads the SAME events.jsonl the issuer signed, runs
    // the gate, and gets cryptographic confirmation that the
    // attested graph_state_hash matches.
    let jsonl = [
        r#"{"event_id":"01HE001","event_hash":"h1","parent_hashes":[],"payload":{"type":"node_create","node":{"id":"alice"}},"signature":{"alg":"EdDSA","kid":"atlas-anchor:ws-gate-test","sig":"AA"},"ts":"2026-05-13T10:00:00Z"}"#,
        r#"{"event_id":"01HE002","event_hash":"h2","parent_hashes":["h1"],"payload":{"type":"node_create","node":{"id":"bob"}},"signature":{"alg":"EdDSA","kid":"atlas-anchor:ws-gate-test","sig":"BB"},"ts":"2026-05-13T10:01:00Z"}"#,
    ].join("\n");

    let projectable = parse_events_jsonl(&jsonl).unwrap();
    let state = project_events(WS, &projectable, None).unwrap();
    let _direct_hash = graph_state_hash(&state).unwrap();
    let attestation = make_attestation_event("01HATT1", &state, projectable.len() as u64);

    let mut all_events = projectable.clone();
    all_events.push(attestation);
    let trace = make_trace(all_events);

    let results = verify_attestations_in_trace(WS, &trace).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, GateStatus::Match);
}
