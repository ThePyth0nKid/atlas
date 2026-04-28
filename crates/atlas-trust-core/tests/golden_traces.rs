//! End-to-end round-trip test:
//!   1. Generate a deterministic Ed25519 keypair.
//!   2. Build a small DAG of events (genesis + child).
//!   3. Sign them with our keypair.
//!   4. Build a pubkey-bundle pinning that key.
//!   5. Build an AtlasTrace bundle.
//!   6. Run `verify_trace` → expect VALID.
//!   7. Tamper with one event's payload → expect INVALID.
//!
//! This test is the load-bearing proof that signer + verifier agree
//! on the canonical signing-input format.

use atlas_trust_core::{
    cose::build_signing_input,
    hashchain::compute_event_hash,
    pubkey_bundle::PubkeyBundle,
    trace_format::{AnchorEntry, AtlasEvent, AtlasTrace, EventSignature},
    verify::verify_trace,
};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;

const TEST_KID: &str = "spiffe://atlas/test/agent-001";
const TEST_WORKSPACE: &str = "ws-test";

fn b64url_no_pad_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn build_signed_event(
    signing_key: &SigningKey,
    event_id: &str,
    ts: &str,
    parents: Vec<String>,
    payload: serde_json::Value,
) -> AtlasEvent {
    let signing_input = build_signing_input(TEST_WORKSPACE, event_id, ts, TEST_KID, &parents, &payload).unwrap();
    let event_hash = compute_event_hash(&signing_input);
    let signature = signing_key.sign(&signing_input);

    AtlasEvent {
        event_id: event_id.to_string(),
        event_hash,
        parent_hashes: parents,
        payload,
        signature: EventSignature {
            alg: "EdDSA".to_string(),
            kid: TEST_KID.to_string(),
            sig: b64url_no_pad_encode(&signature.to_bytes()),
        },
        ts: ts.to_string(),
    }
}

fn build_test_bundle(verifying_key: &ed25519_dalek::VerifyingKey) -> PubkeyBundle {
    let mut keys = HashMap::new();
    keys.insert(
        TEST_KID.to_string(),
        b64url_no_pad_encode(verifying_key.as_bytes()),
    );
    PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-04-27T10:00:00Z".to_string(),
        keys,
    }
}

#[test]
fn round_trip_two_event_dag_verifies() {
    // 1. Deterministic keypair
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let verifying_key = signing_key.verifying_key();

    // 2. Bundle
    let bundle = build_test_bundle(&verifying_key);
    let bundle_hash = bundle.deterministic_hash().unwrap();

    // 3. Genesis event
    let genesis = build_signed_event(
        &signing_key,
        "01H001GENESIS",
        "2026-04-27T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create", "node": {"id": "n1", "name": "GenesisFact"}}),
    );

    // 4. Child event
    let child = build_signed_event(
        &signing_key,
        "01H002CHILD",
        "2026-04-27T10:00:01Z",
        vec![genesis.event_hash.clone()],
        serde_json::json!({"type": "node.create", "node": {"id": "n2", "name": "ChildFact"}}),
    );

    // 5. Trace
    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-27T10:01:00Z".to_string(),
        workspace_id: "ws-test".to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![genesis, child.clone()],
        dag_tips: vec![child.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
    };

    // 6. Verify
    let outcome = verify_trace(&trace, &bundle);
    assert!(
        outcome.valid,
        "expected VALID outcome, got errors: {:#?}",
        outcome.errors
    );
    assert!(outcome.errors.is_empty());
    assert!(!outcome.evidence.is_empty());
    // All checks should be ok=true.
    for ev in &outcome.evidence {
        assert!(ev.ok, "evidence check {} failed: {}", ev.check, ev.detail);
    }
}

#[test]
fn tampered_payload_detected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let bundle = build_test_bundle(&verifying_key);
    let bundle_hash = bundle.deterministic_hash().unwrap();

    let mut genesis = build_signed_event(
        &signing_key,
        "01H001GENESIS",
        "2026-04-27T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create", "node": {"id": "n1"}}),
    );

    // Tamper the payload AFTER signing
    genesis.payload = serde_json::json!({"type": "node.create", "node": {"id": "TAMPERED"}});

    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-27T10:01:00Z".to_string(),
        workspace_id: "ws-test".to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![genesis.clone()],
        dag_tips: vec![genesis.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
    };

    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid, "tampered payload must fail verification");
    assert!(!outcome.errors.is_empty());
}

#[test]
fn unknown_kid_detected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let verifying_key = signing_key.verifying_key();

    // Bundle with DIFFERENT kid than the event will use
    let mut keys = HashMap::new();
    keys.insert(
        "spiffe://atlas/different-key".to_string(),
        b64url_no_pad_encode(verifying_key.as_bytes()),
    );
    let bundle = PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-04-27T10:00:00Z".to_string(),
        keys,
    };
    let bundle_hash = bundle.deterministic_hash().unwrap();

    let event = build_signed_event(
        &signing_key,
        "01H001",
        "2026-04-27T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create"}),
    );

    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-27T10:01:00Z".to_string(),
        workspace_id: "ws-test".to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![event.clone()],
        dag_tips: vec![event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
    };

    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid);
    assert!(outcome.errors.iter().any(|e| e.contains("unknown signing key")));
}

#[test]
fn schema_mismatch_detected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let bundle = build_test_bundle(&verifying_key);
    let bundle_hash = bundle.deterministic_hash().unwrap();

    let event = build_signed_event(
        &signing_key,
        "01H001",
        "2026-04-27T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create"}),
    );

    let trace = AtlasTrace {
        schema_version: "atlas-trace-v999".to_string(), // wrong
        generated_at: "2026-04-27T10:01:00Z".to_string(),
        workspace_id: "ws-test".to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![event.clone()],
        dag_tips: vec![event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
    };

    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid);
    assert!(outcome.errors.iter().any(|e| e.contains("schema mismatch")));
}

// ─────────────────────────────────────────────────────────────────────────
// Phase C — adversary tests.
// Each of these constructs a trace that an attacker (or buggy emitter)
// might produce. The verifier MUST reject every one of them.
// ─────────────────────────────────────────────────────────────────────────

/// Helper: build a fully signed trace for a given workspace_id, with N events
/// each chained off the previous. Returns (trace, bundle).
fn make_chain(
    signing_key: &SigningKey,
    workspace_id: &str,
    n: usize,
) -> (AtlasTrace, PubkeyBundle) {
    let bundle = build_test_bundle(&signing_key.verifying_key());
    let bundle_hash = bundle.deterministic_hash().unwrap();
    let mut events = Vec::new();
    let mut prev_hash: Option<String> = None;
    for i in 0..n {
        let parents = match &prev_hash {
            Some(h) => vec![h.clone()],
            None => vec![],
        };
        let ev = build_signed_event(
            signing_key,
            &format!("01H{:03}", i),
            "2026-04-27T10:00:00Z",
            parents,
            serde_json::json!({"type": "node.create", "n": i as u64}),
        );
        prev_hash = Some(ev.event_hash.clone());
        events.push(ev);
    }
    let tips = vec![prev_hash.unwrap_or_default()];
    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-27T10:01:00Z".to_string(),
        workspace_id: workspace_id.to_string(),
        pubkey_bundle_hash: bundle_hash,
        events,
        dag_tips: tips,
        anchors: vec![],
        policies: vec![],
        filters: None,
    };
    (trace, bundle)
}

/// Adversary: an event signed for workspace A is presented inside a trace
/// claiming workspace B. workspace_id is bound into the signing-input, so
/// recomputing the hash with B's id MUST diverge.
#[test]
fn cross_workspace_replay_rejected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);

    // Build trace correctly for workspace A.
    let (mut trace, bundle) = make_chain(&signing_key, "ws-A", 2);

    // Now lie: claim this trace is for workspace B.
    trace.workspace_id = "ws-B".to_string();

    let outcome = verify_trace(&trace, &bundle);
    assert!(
        !outcome.valid,
        "trace with relabelled workspace_id MUST fail (cross-workspace replay defence)"
    );
    assert!(
        outcome.errors.iter().any(|e| e.contains("hash mismatch")),
        "expected hash mismatch, got: {:#?}",
        outcome.errors
    );
}

/// Adversary: trace claims an anchor but the proof material is bogus.
/// V1.5 verifier validates Merkle inclusion + checkpoint signature against
/// the pinned log roster. A made-up proof must fail, not silently pass.
#[test]
fn anchor_with_bogus_proof_is_rejected() {
    use atlas_trust_core::trace_format::{AnchorKind, InclusionProof};
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let (mut trace, bundle) = make_chain(&signing_key, "ws-test", 1);
    trace.anchors.push(AnchorEntry {
        kind: AnchorKind::DagTip,
        anchored_hash: trace.dag_tips[0].clone(),
        // log_id not in the trusted roster — verifier must reject upfront.
        log_id: "0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        log_index: 0,
        integrated_time: 1_700_000_000,
        inclusion_proof: InclusionProof {
            tree_size: 1,
            root_hash:
                "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
                    .to_string(),
            hashes: vec![],
            checkpoint_sig: "AA".to_string(),
        },
    });
    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid, "anchor with bogus proof must be rejected");
    assert!(
        outcome.errors.iter().any(|e| e.contains("anchor:")),
        "expected an anchor: error, got: {:#?}",
        outcome.errors,
    );
}

/// Adversary: signature alg field is something other than EdDSA.
/// V1 only accepts EdDSA — every other value is either a downgrade attempt
/// or a misconfigured signer. Reject with explicit error.
#[test]
fn wrong_alg_rejected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let (mut trace, bundle) = make_chain(&signing_key, "ws-test", 1);
    trace.events[0].signature.alg = "RS256".to_string(); // wrong
    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid, "non-EdDSA alg must be rejected");
    assert!(
        outcome.errors.iter().any(|e| e.contains("unsupported signature alg")),
        "expected unsupported-alg error, got: {:#?}",
        outcome.errors
    );
}

/// Adversary: timestamp field does not parse as RFC 3339.
/// A signer pumping free-form date strings is buggy or hostile — reject.
#[test]
fn non_rfc3339_timestamp_rejected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let (mut trace, bundle) = make_chain(&signing_key, "ws-test", 1);
    trace.events[0].ts = "yesterday at noon".to_string();
    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid, "non-RFC3339 ts must be rejected");
    assert!(
        outcome.errors.iter().any(|e| e.contains("RFC 3339")),
        "expected RFC 3339 parse error, got: {:#?}",
        outcome.errors
    );
}

/// Adversary: two events in the same trace carry the same event_hash.
/// Either a buggy emitter or a replay-collision attempt. `check_event_hashes`
/// must reject.
#[test]
fn duplicate_event_hash_rejected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let (mut trace, bundle) = make_chain(&signing_key, "ws-test", 2);
    // Force the second event to claim the first event's hash.
    let dup_hash = trace.events[0].event_hash.clone();
    trace.events[1].event_hash = dup_hash.clone();
    trace.dag_tips = vec![dup_hash];
    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid, "duplicate event_hashes must be rejected");
    assert!(
        outcome.errors.iter().any(|e| e.contains("duplicate event_hash")),
        "expected duplicate event_hash error, got: {:#?}",
        outcome.errors
    );
}

/// Adversary: trace claims a different DAG-tip than the events compute.
/// The tip-mismatch check defends against an emitter rewriting history
/// after-the-fact.
#[test]
fn dag_tip_mismatch_rejected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let (mut trace, bundle) = make_chain(&signing_key, "ws-test", 2);
    // Forge: replace claimed tip with a totally fake hash.
    trace.dag_tips = vec!["0".repeat(64)];
    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid, "dag-tip mismatch must reject");
    assert!(outcome.errors.iter().any(|e| e.contains("dag-tip-mismatch")));
}

/// Adversary: schema_version with a rogue prefix, e.g. an attacker hopes
/// the verifier does `starts_with("atlas-trace-v1")` instead of `==`.
#[test]
fn schema_version_prefix_attack_rejected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let (mut trace, bundle) = make_chain(&signing_key, "ws-test", 1);
    trace.schema_version = "atlas-trace-v1-extended".to_string();
    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid, "schema version prefix attack must reject");
    assert!(outcome.errors.iter().any(|e| e.contains("schema mismatch")));
}

/// Adversary: empty pubkey bundle — no keys at all. The first event's kid
/// will be unknown.
#[test]
fn empty_pubkey_bundle_rejected() {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let (mut trace, _full_bundle) = make_chain(&signing_key, "ws-test", 1);

    let empty_bundle = PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-04-27T10:00:00Z".to_string(),
        keys: HashMap::new(),
    };
    // Re-claim the empty bundle's hash so we get past the bundle-hash check
    // and exercise the unknown-kid path. (If the bundle hash mismatches first,
    // we wouldn't reach the sig loop — that's fine, but we want to prove the
    // empty-bundle case fails for the right reason.)
    trace.pubkey_bundle_hash = empty_bundle.deterministic_hash().unwrap();

    let outcome = verify_trace(&trace, &empty_bundle);
    assert!(!outcome.valid, "empty bundle must reject");
    assert!(outcome.errors.iter().any(|e| e.contains("unknown signing key")));
}

/// Honesty regression: a bundle generated against pubkey-set A but verified
/// against pubkey-set B must fail at the bundle-hash check, before any
/// per-event work. (Defends against silent bundle rotation.)
#[test]
fn bundle_hash_mismatch_rejected() {
    let signing_key_a = SigningKey::from_bytes(&[42u8; 32]);
    let (mut trace, _bundle_a) = make_chain(&signing_key_a, "ws-test", 1);

    // Build a different bundle (different key).
    let signing_key_b = SigningKey::from_bytes(&[99u8; 32]);
    let bundle_b = build_test_bundle(&signing_key_b.verifying_key());

    // Trace still claims bundle A's hash. Verifying against B must reject.
    // (We don't tamper trace.pubkey_bundle_hash — the mismatch comes from
    // verifier holding a different bundle than the trace was emitted against.)
    let outcome = verify_trace(&trace, &bundle_b);
    assert!(!outcome.valid);
    assert!(outcome.errors.iter().any(|e| e.contains("pubkey bundle mismatch")));
    // Silence unused-mut warning when we don't end up tampering.
    trace.workspace_id.clear();
}
