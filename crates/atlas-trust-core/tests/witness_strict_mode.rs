//! V1.13 Scope C wave-C-2 strict-mode threshold tests.
//!
//! `opts.require_witness_threshold: usize` adds an M-of-N witness
//! requirement on top of the wave-C-1 lenient default. These tests pin
//! the contract end-to-end through `verify_trace_with`:
//!
//!   * threshold == 0 (default)  → lenient (wave-C-1 behaviour preserved).
//!   * threshold >= 1 with `verified < threshold` → invalid; `errors`
//!     gains a `witnesses-threshold` row naming the verified/required
//!     counts.
//!   * threshold >= 1 with `verified >= threshold` → valid (covered by
//!     the unit tests in `verify.rs::tests` because the production
//!     `ATLAS_WITNESS_V1_ROSTER` is genesis-empty — no witness can
//!     verify in the integration path until commissioning lands).
//!
//! Why integration tests in addition to the verify.rs unit tests: the
//! strict-mode check is wired in `verify_trace_with` after the
//! chain-walking step, and an off-by-one (e.g. only checking the
//! threshold inside the chain-Some branch) would silently let a
//! chain-less trace pass strict mode. The integration tests pin that
//! pathway too.

use atlas_trust_core::{
    chain_head_for,
    cose::build_signing_input,
    hashchain::compute_event_hash,
    verify_trace_with,
    witness::WitnessSig,
    AnchorBatch, AnchorChain, AnchorEntry, AtlasEvent, AtlasTrace, EventSignature,
    PubkeyBundle, VerifyOptions, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
};
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────
// Fixture builders (mirror the shape used in anchor_chain_adversary.rs;
// duplicated rather than shared because cross-test-file fixtures are not
// part of the integration-test surface in this crate).
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

    let event_id = "01H0WITNESSTEST";
    let ts = "2026-04-27T10:00:00Z";
    let payload = serde_json::json!({"type": "node.create", "node": {"id": "n1"}});
    let signing_input =
        build_signing_input("ws-witness", event_id, ts, "spiffe://atlas/test", &[], &payload)
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
    };

    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-27T10:01:00Z".to_string(),
        workspace_id: "ws-witness".to_string(),
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

/// Build a single-batch chain. Witnesses optionally attached; the
/// previous_head is the genesis sentinel.
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
// Lenient default (threshold == 0) preserves wave-C-1 behaviour.
// ─────────────────────────────────────────────────────────────────────────

/// Trace WITHOUT anchor_chain, default `VerifyOptions`. Lenient passes:
/// neither the chain nor the witness check fires. Pins that wave C-2's
/// new field defaults to 0 (the lenient sentinel).
#[test]
fn lenient_default_no_chain_no_witness_passes() {
    let (trace, bundle) = build_minimal_trace_with(None);
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());
    assert!(
        outcome.valid,
        "lenient default must pass: errors={:?}",
        outcome.errors,
    );
}

/// Trace WITH anchor_chain (no witnesses), default options. The
/// witnesses evidence row appears with the "no witnesses presented"
/// detail (wave-C-1 no-op disposition); strict-mode threshold is NOT
/// checked.
#[test]
fn lenient_default_with_chain_no_witness_passes() {
    let chain = single_batch_chain(vec![]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());
    assert!(
        outcome.valid,
        "wave-C-1 lenient behaviour must be preserved: errors={:?}",
        outcome.errors,
    );
    let witness_row = outcome
        .evidence
        .iter()
        .find(|e| e.check == "witnesses")
        .expect("witnesses evidence row must be present when chain is");
    assert!(
        witness_row.ok,
        "no-witnesses-presented row must be ok=true: {witness_row:?}",
    );
    assert!(
        outcome
            .evidence
            .iter()
            .all(|e| e.check != "witnesses-threshold"),
        "no witnesses-threshold row when threshold==0",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Strict mode (threshold >= 1) — failure cases reachable from
// verify_trace_with under the genesis-empty production roster.
// ─────────────────────────────────────────────────────────────────────────

/// Trace without anchor_chain + `require_witness_threshold = 1`.
/// Strict mode rejects: zero witnesses can possibly verify. Defends
/// against an off-by-one where the threshold check were nested inside
/// the `Some(chain)` arm and silently passed for chain-less traces.
#[test]
fn strict_threshold_one_no_chain_fails() {
    let (trace, bundle) = build_minimal_trace_with(None);
    let opts = VerifyOptions {
        require_witness_threshold: 1,
        ..Default::default()
    };
    let outcome = verify_trace_with(&trace, &bundle, &opts);
    assert!(
        !outcome.valid,
        "strict threshold=1 with no chain must fail; got evidence={:?}",
        outcome.evidence,
    );
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("witness") && e.contains("0") && e.contains("1")),
        "error must name verified/required counts (0 of 1): {:?}",
        outcome.errors,
    );
    let row = outcome
        .evidence
        .iter()
        .find(|e| e.check == "witnesses-threshold")
        .expect("witnesses-threshold evidence row must be present in strict mode");
    assert!(!row.ok, "threshold-row must be ok=false on miss: {row:?}");
}

/// Trace WITH anchor_chain but no witnesses + `require_witness_threshold = 2`.
/// Strict mode rejects (0 verified < 2 required). Pins the higher
/// threshold path so the check isn't a hard-coded `< 1`.
#[test]
fn strict_threshold_two_with_chain_no_witness_fails() {
    let chain = single_batch_chain(vec![]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let opts = VerifyOptions {
        require_witness_threshold: 2,
        ..Default::default()
    };
    let outcome = verify_trace_with(&trace, &bundle, &opts);
    assert!(!outcome.valid, "strict threshold=2 + 0 verified must fail");
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("0") && e.contains("2")),
        "error must name (0 of 2): {:?}",
        outcome.errors,
    );
}

/// Trace with chain + uncommissioned witness (kid not in production
/// roster) + threshold=1. Strict mode rejects: 0 verified, 1 required.
/// The lenient evidence row (`witnesses` check) ALSO surfaces ok=false
/// with the per-failure breakdown — wave-C-1 behaviour preserved.
#[test]
fn strict_threshold_one_with_uncommissioned_witness_fails() {
    let uncommissioned = WitnessSig {
        witness_kid: "uncommissioned-test-witness".to_string(),
        // Any 64-byte payload — verification fails on the kid lookup
        // before the signature is even decoded fully (kid is checked
        // first), so signature contents don't matter.
        signature: "A".repeat(86),
    };
    let chain = single_batch_chain(vec![uncommissioned]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));
    let opts = VerifyOptions {
        require_witness_threshold: 1,
        ..Default::default()
    };
    let outcome = verify_trace_with(&trace, &bundle, &opts);
    assert!(
        !outcome.valid,
        "uncommissioned witness must NOT count toward threshold",
    );

    // Lenient row (wave-C-1) MUST still surface the failure detail.
    let lenient_row = outcome
        .evidence
        .iter()
        .find(|e| e.check == "witnesses")
        .expect("lenient witnesses evidence row must be present");
    assert!(
        !lenient_row.ok,
        "lenient row must surface uncommissioned-kid failure as ok=false",
    );
    assert!(
        lenient_row.detail.contains("uncommissioned-test-witness"),
        "lenient detail must name the failed kid: {}",
        lenient_row.detail,
    );

    // Strict-mode row.
    let strict_row = outcome
        .evidence
        .iter()
        .find(|e| e.check == "witnesses-threshold")
        .expect("witnesses-threshold row required in strict mode");
    assert!(!strict_row.ok);
}

/// `require_witness_threshold = 0` is explicitly the lenient sentinel:
/// no `witnesses-threshold` row is added, even if the lenient row
/// reports failures. Pins that we don't accidentally add a strict row
/// for the default value.
#[test]
fn threshold_zero_does_not_add_threshold_row() {
    let uncommissioned = WitnessSig {
        witness_kid: "uncommissioned-test-witness".to_string(),
        signature: "A".repeat(86),
    };
    let chain = single_batch_chain(vec![uncommissioned]);
    let (trace, bundle) = build_minimal_trace_with(Some(chain));

    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());

    assert!(
        outcome
            .evidence
            .iter()
            .all(|e| e.check != "witnesses-threshold"),
        "threshold==0 must not emit witnesses-threshold row",
    );

    // Lenient stays valid even with the failed witness (wave-C-1
    // disposition: ok=false on evidence, but trace.valid stays true).
    assert!(
        outcome.valid,
        "lenient mode keeps trace valid despite witness failures: errors={:?}",
        outcome.errors,
    );
}
