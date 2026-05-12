//! V1.14 Scope J-b — `VerifyOutcome.witness_failures` wire-surface tests.
//!
//! `verify_trace_with` projects the categorised
//! `witness_aggregate.failures` (V1.14 Scope J-a) onto the auditor-
//! facing `Vec<WitnessFailureWire>` field. These tests pin the
//! end-to-end wiring:
//!
//!   * empty when there are no witness failures,
//!   * populated with the right `reason_code` when a known-bad witness
//!     is presented,
//!   * `batch_index` carried through faithfully on multi-batch chains,
//!   * sanitised `witness_kid` for oversize input (defends the lenient
//!     evidence row from multi-MB blob amplification),
//!   * stable JSON shape (kebab-case `reason_code`, optional
//!     `batch_index`) — what auditor tooling will key on.
//!
//! Why integration tests in addition to the unit tests in
//! `witness.rs::tests`: the projection happens in `verify_trace_with`
//! after `aggregate_witnesses_across_chain`, and a regression that
//! built `outcome.witness_failures` from the WRONG aggregate (e.g. a
//! stale variable, or only one batch's failures) would still pass the
//! per-failure unit tests. Driving the public `verify_trace_with`
//! entry point pins the real wire that auditors will consume.

use atlas_trust_core::{
    chain_head_for,
    cose::build_signing_input,
    hashchain::compute_event_hash,
    verify_trace_with,
    witness::{WitnessFailureReason, WitnessSig, MAX_WITNESS_KID_LEN},
    AnchorBatch, AnchorChain, AnchorEntry, AtlasEvent, AtlasTrace, EventSignature,
    PubkeyBundle, VerifyOptions, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
};
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────
// Fixture builders (pattern mirrors witness_strict_mode.rs — kept local
// because cross-test-file fixtures are not part of the integration-test
// surface in this crate).
// ─────────────────────────────────────────────────────────────────────────

fn b64url_no_pad_encode(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(bytes)
}

fn build_minimal_trace_with(chain: Option<AnchorChain>) -> (AtlasTrace, PubkeyBundle) {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let verifying_key = signing_key.verifying_key();

    let mut keys = HashMap::new();
    keys.insert(
        "spiffe://atlas/test".to_string(),
        b64url_no_pad_encode(verifying_key.as_bytes()),
    );
    let bundle = PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-04-27T10:00:00Z".to_string(),
        keys,
    };
    let bundle_hash = bundle.deterministic_hash().unwrap();

    let event_id = "01H0SCOPEJBTEST";
    let ts = "2026-04-27T10:00:00Z";
    let payload = serde_json::json!({"type": "node.create", "node": {"id": "n1"}});
    let signing_input =
        build_signing_input("ws-jb", event_id, ts, "spiffe://atlas/test", &[], &payload, None)
            .unwrap();
    let event_hash = compute_event_hash(&signing_input);
    let sig = signing_key.sign(&signing_input);

    let event = AtlasEvent {
        event_id: event_id.to_string(),
        ts: ts.to_string(),
        parent_hashes: vec![],
        payload,
        event_hash: event_hash.clone(),
        signature: EventSignature {
            alg: "EdDSA".to_string(),
            kid: "spiffe://atlas/test".to_string(),
            sig: b64url_no_pad_encode(&sig.to_bytes()),
        },
        author_did: None,
    };

    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-27T10:01:00Z".to_string(),
        workspace_id: "ws-jb".to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![event.clone()],
        dag_tips: vec![event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: chain,
    };
    (trace, bundle)
}

fn single_batch_chain(witnesses: Vec<WitnessSig>) -> AnchorChain {
    let batch = AnchorBatch {
        batch_index: 0,
        integrated_time: 1_745_000_000,
        entries: Vec::<AnchorEntry>::new(),
        previous_head: ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD.to_string(),
        witnesses,
    };
    let head = chain_head_for(&batch).unwrap();
    AnchorChain {
        history: vec![batch],
        head: head.into_inner(),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Empty case — nothing to project.
// ─────────────────────────────────────────────────────────────────────────

/// Trace without an anchor_chain has no witnesses to verify, so
/// `witness_failures` must be empty. Pins the additive default so that
/// adding the field can never regress an existing pass-path.
#[test]
fn witness_failures_empty_when_no_anchor_chain() {
    let (trace, bundle) = build_minimal_trace_with(None);
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());
    assert!(outcome.valid);
    assert!(
        outcome.witness_failures.is_empty(),
        "no chain ⇒ no witness verification ⇒ empty witness_failures: {:?}",
        outcome.witness_failures,
    );
}

/// Trace WITH an anchor_chain but zero witnesses: still nothing to
/// fail. Pins that the lenient no-witnesses-presented disposition does
/// NOT inject a synthetic failure into `witness_failures`.
#[test]
fn witness_failures_empty_when_chain_has_no_witnesses() {
    let chain = single_batch_chain(vec![]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());
    assert!(outcome.valid);
    assert!(
        outcome.witness_failures.is_empty(),
        "no witnesses presented must not synthesise a failure entry",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Per-reason-code projection.
// ─────────────────────────────────────────────────────────────────────────

/// Single-batch chain, one uncommissioned witness (kid not in the
/// genesis-empty production roster). `witness_failures` must contain
/// exactly one entry with `reason_code = KidNotInRoster`,
/// `batch_index = Some(0)`, and the sanitised kid.
#[test]
fn witness_failures_populated_for_uncommissioned_kid() {
    let kid = "uncommissioned-jb-kid";
    let chain = single_batch_chain(vec![WitnessSig {
        witness_kid: kid.to_string(),
        signature: "A".repeat(86),
    }]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());

    assert_eq!(
        outcome.witness_failures.len(),
        1,
        "one uncommissioned witness ⇒ one failure entry: {:?}",
        outcome.witness_failures,
    );
    let f = &outcome.witness_failures[0];
    assert_eq!(f.witness_kid, kid);
    assert_eq!(f.batch_index, Some(0));
    assert_eq!(f.reason_code, WitnessFailureReason::KidNotInRoster);
    assert!(
        f.message.contains(kid),
        "auditor message must echo the kid: {}",
        f.message,
    );
}

/// Oversize kid (longer than `MAX_WITNESS_KID_LEN`) is the wire-side
/// untrusted-input attack: a multi-MB kid would amplify if echoed
/// verbatim into the lenient evidence row's joined detail. The kid
/// surfaced on `witness_failures` must be the SANITISED form, not the
/// raw input. Pins the SEC fix from J-a (per-batch verifier
/// sanitisation) is preserved through to the wire surface.
#[test]
fn witness_failures_oversize_kid_is_sanitised() {
    let oversize_kid = "x".repeat(MAX_WITNESS_KID_LEN + 64);
    let chain = single_batch_chain(vec![WitnessSig {
        witness_kid: oversize_kid.clone(),
        signature: "A".repeat(86),
    }]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());

    assert_eq!(outcome.witness_failures.len(), 1);
    let f = &outcome.witness_failures[0];
    assert_eq!(f.reason_code, WitnessFailureReason::OversizeKid);
    assert!(
        f.witness_kid.len() <= MAX_WITNESS_KID_LEN + 32,
        "wire kid must be sanitised, got {} bytes",
        f.witness_kid.len(),
    );
    assert_ne!(
        f.witness_kid, oversize_kid,
        "raw oversize kid must NOT pass through to the wire surface",
    );
}

/// Two witnesses in the SAME batch with an identical kid. The dedup
/// pre-pass in the per-batch verifier rejects the second occurrence
/// with `DuplicateKid`. Pins the within-batch dedup separately from
/// the cross-batch path.
#[test]
fn witness_failures_duplicate_kid_within_batch() {
    let dup_kid = "dup-within-batch";
    let chain = single_batch_chain(vec![
        WitnessSig {
            witness_kid: dup_kid.to_string(),
            signature: "A".repeat(86),
        },
        WitnessSig {
            witness_kid: dup_kid.to_string(),
            signature: "B".repeat(86),
        },
    ]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());

    // The second occurrence is the one that fails dedup; the first
    // still hits KidNotInRoster (or whatever the per-witness verifier
    // surfaces). We assert the dedup signal is present, regardless of
    // sibling-failure ordering.
    assert!(
        outcome
            .witness_failures
            .iter()
            .any(|f| f.reason_code == WitnessFailureReason::DuplicateKid
                && f.witness_kid == dup_kid),
        "DuplicateKid entry must surface on the wire: {:?}",
        outcome.witness_failures,
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Multi-batch — `batch_index` propagation.
// ─────────────────────────────────────────────────────────────────────────

/// Two batches: batch[0] empty, batch[1] carries one uncommissioned
/// witness. The wire entry must carry `batch_index = Some(1)` so the
/// auditor can localise the failure in the chain. Defends against
/// off-by-one in the projection loop (e.g. always emitting `Some(0)`).
#[test]
fn witness_failures_carries_batch_index_on_later_batch() {
    let mut chain = AnchorChain {
        history: vec![],
        head: String::new(),
    };
    chain.history.push(AnchorBatch {
        batch_index: 0,
        integrated_time: 1_745_000_000,
        entries: vec![],
        previous_head: ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD.to_string(),
        witnesses: vec![],
    });
    let head_zero = chain_head_for(&chain.history[0]).unwrap().into_inner();

    chain.history.push(AnchorBatch {
        batch_index: 1,
        integrated_time: 1_745_000_001,
        entries: vec![],
        previous_head: head_zero,
        witnesses: vec![WitnessSig {
            witness_kid: "kid-on-batch-1".to_string(),
            signature: "A".repeat(86),
        }],
    });
    chain.head = chain_head_for(&chain.history[1]).unwrap().into_inner();

    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());

    assert_eq!(outcome.witness_failures.len(), 1);
    let f = &outcome.witness_failures[0];
    assert_eq!(
        f.batch_index,
        Some(1),
        "batch_index must localise the failure to batch[1]: {:?}",
        f,
    );
    assert_eq!(f.witness_kid, "kid-on-batch-1");
    assert_eq!(f.reason_code, WitnessFailureReason::KidNotInRoster);
}

// ─────────────────────────────────────────────────────────────────────────
// Wire schema — JSON round-trip pinning.
// ─────────────────────────────────────────────────────────────────────────

/// Pin the JSON shape auditor tooling will consume:
///   * `reason_code` is kebab-case (V1 wire-stable enum encoding).
///   * `batch_index` is omitted-or-null absent? -> serialised as
///     `null` (Option<u64> default behaviour).
///   * `witness_kid`, `message` are plain strings.
///   * Round-trip must be lossless.
///
/// Auditors will key on `reason_code` for classification — a silent
/// rename would break their dashboards, so we pin the literal string.
#[test]
fn witness_failures_json_shape_is_kebab_case_and_round_trips() {
    let chain = single_batch_chain(vec![WitnessSig {
        witness_kid: "wire-shape-kid".to_string(),
        signature: "A".repeat(86),
    }]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());

    let json =
        serde_json::to_string(&outcome.witness_failures).expect("serialises cleanly");
    assert!(
        json.contains("\"reason_code\":\"kid-not-in-roster\""),
        "kebab-case reason_code expected in wire JSON: {}",
        json,
    );
    assert!(json.contains("\"witness_kid\":\"wire-shape-kid\""));
    assert!(json.contains("\"batch_index\":0"));

    // Round-trip pin: the wire form must deserialise back into the
    // same Vec<WitnessFailureWire>.
    let parsed: Vec<atlas_trust_core::WitnessFailureWire> =
        serde_json::from_str(&json).expect("deserialises cleanly");
    assert_eq!(parsed, outcome.witness_failures);
}

/// `VerifyOutcome` itself must serialise with `witness_failures` as a
/// JSON array (not an opaque field). Auditor tooling that already
/// consumes `valid`/`evidence`/`errors` should see the new field as
/// purely additive — pre-J-b consumers must still parse the new
/// payload (because we used `#[serde(default)]` on the field, missing-
/// in-input is also accepted, but that's exercised by the unit tests).
#[test]
fn verify_outcome_json_includes_witness_failures_array() {
    let chain = single_batch_chain(vec![WitnessSig {
        witness_kid: "outcome-shape-kid".to_string(),
        signature: "A".repeat(86),
    }]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());

    let json = serde_json::to_string(&outcome).expect("outcome serialises");
    assert!(
        json.contains("\"witness_failures\":["),
        "outcome JSON must expose witness_failures as an array: {}",
        json,
    );
    assert!(json.contains("kid-not-in-roster"));
}
