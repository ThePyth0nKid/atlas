//! Adversary tests for the V1.7 anchor-chain.
//!
//! Each test constructs a chain (or trace-with-chain) that an attacker —
//! or a buggy emitter — might produce, and asserts the verifier rejects
//! it. These tests are the load-bearing assertion that the chain
//! actually defends against post-hoc rewriting of past anchored state;
//! a failure here means the V1.7 trust property is broken.
//!
//! For chain-internal tests we drive `verify_anchor_chain` directly
//! (no signed events, no bundle, no per-entry inclusion proofs needed
//! to exercise the chain's hash-link logic). For end-to-end strict-mode
//! and lenient-mode tests we go through `verify_trace_with` with a
//! minimal signed trace.

use atlas_trust_core::{
    anchor::{SIGSTORE_REKOR_V1_LOG_ID, SIGSTORE_REKOR_V1_TREE_IDS},
    chain_head_for,
    cose::build_signing_input,
    hashchain::compute_event_hash,
    verify_trace_with, AnchorBatch, AnchorChain, AnchorEntry, AnchorKind, AtlasEvent, AtlasTrace,
    EventSignature, InclusionProof, PubkeyBundle, VerifyOptions, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
};
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────
// Fixture builders
// ─────────────────────────────────────────────────────────────────────────

/// Deterministic placeholder entry. The chain hash depends on the
/// entry's full canonical bytes, but the per-entry inclusion-proof
/// machinery is not exercised here — these tests pin chain semantics,
/// not inclusion-proof verification (covered separately in
/// tests/sigstore_golden.rs and the unit tests in src/anchor.rs).
fn fixture_entry(seed: u64) -> AnchorEntry {
    AnchorEntry {
        kind: AnchorKind::DagTip,
        anchored_hash: format!("{:064x}", seed),
        log_id: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
        log_index: seed,
        integrated_time: 1_745_000_000 + seed as i64,
        inclusion_proof: InclusionProof {
            tree_size: seed + 1,
            root_hash: format!("{:064x}", seed.wrapping_mul(7)),
            hashes: vec![format!("{:064x}", seed.wrapping_mul(13))],
            checkpoint_sig: "AAAA".to_string(),
        },
        entry_body_b64: None,
        tree_id: None,
    }
}

fn fixture_batch(batch_index: u64, integrated_time: i64, previous_head: &str) -> AnchorBatch {
    AnchorBatch {
        batch_index,
        integrated_time,
        entries: vec![fixture_entry(batch_index * 10), fixture_entry(batch_index * 10 + 1)],
        previous_head: previous_head.to_string(),
        witnesses: Vec::new(),
    }
}

/// Build a valid N-batch chain. Each batch's `previous_head` is the
/// `chain_head_for` of the prior batch; the convenience `head` is the
/// `chain_head_for` of the final batch. This is what an honest issuer
/// would produce.
fn build_valid_chain(n: usize) -> AnchorChain {
    assert!(n > 0, "chain must have at least one batch");
    let mut history: Vec<AnchorBatch> = Vec::with_capacity(n);
    let mut prev_head = ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD.to_string();
    for i in 0..n {
        let batch = fixture_batch(i as u64, 1_745_000_000 + i as i64, &prev_head);
        // V1.13 wave-C-2: chain_head_for returns ChainHeadHex; unwrap
        // to wire-side String for AnchorBatch.previous_head + AnchorChain.head.
        prev_head = chain_head_for(&batch)
            .expect("chain_head_for fixture")
            .into_inner();
        history.push(batch);
    }
    AnchorChain {
        history,
        head: prev_head,
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Chain-internal verification (drives verify_anchor_chain directly)
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn chain_round_trip_three_batches_verifies() {
    let chain = build_valid_chain(3);
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(
        outcome.ok,
        "valid 3-batch chain must verify; errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.batches_walked, 3);
}

#[test]
fn chain_single_genesis_batch_verifies() {
    let chain = build_valid_chain(1);
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(
        outcome.ok,
        "single-batch chain must verify; errors: {:?}",
        outcome.errors
    );
}

/// Empty history is malformed by construction — issuers never emit
/// empty chains. Verifier rejects with a clear error.
#[test]
fn chain_empty_history_rejected() {
    let chain = AnchorChain {
        history: vec![],
        head: ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD.to_string(),
    };
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(!outcome.ok);
    assert!(
        outcome.errors.iter().any(|e| e.contains("empty")),
        "expected empty-history error, got: {:?}",
        outcome.errors
    );
}

/// Adversary: drop the middle batch. The third batch's previous_head
/// (which points at the second batch's recomputed head) no longer
/// matches the recomputed head of what is now in position 1. The
/// batch_index field also no longer matches its array position.
#[test]
fn chain_missing_middle_batch_rejected() {
    let mut chain = build_valid_chain(3);
    chain.history.remove(1); // drop batch[1]
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(!outcome.ok, "dropped middle batch must be rejected");
    // Two ways this surfaces: batch_index gap AND previous_head mismatch.
    let combined = outcome.errors.join("\n");
    assert!(
        combined.contains("batch_index") || combined.contains("previous_head"),
        "expected batch_index or previous_head error, got: {:?}",
        outcome.errors
    );
}

/// Adversary: swap two consecutive batches. batch_index sequence
/// breaks (1, 0, 2, ...) and the chain links are scrambled.
#[test]
fn chain_reordered_batches_rejected() {
    let mut chain = build_valid_chain(3);
    chain.history.swap(0, 1);
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(!outcome.ok);
    let combined = outcome.errors.join("\n");
    assert!(
        combined.contains("batch_index"),
        "expected batch_index error, got: {:?}",
        outcome.errors
    );
}

/// Adversary: rewrite a non-tip batch's entries (e.g. swap the
/// anchored_hash). The recomputed head for that batch changes, which
/// makes the next batch's previous_head no longer match.
#[test]
fn chain_tampered_past_entry_rejected() {
    let mut chain = build_valid_chain(3);
    // Tamper batch[1] in place.
    chain.history[1].entries[0].anchored_hash =
        "f000000000000000f000000000000000f000000000000000f000000000000000".to_string();
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(
        !outcome.ok,
        "tampering with a past batch's entries must be rejected"
    );
    let combined = outcome.errors.join("\n");
    assert!(
        combined.contains("previous_head") || combined.contains("convenience head"),
        "expected previous_head or tip mismatch, got: {:?}",
        outcome.errors
    );
}

/// Adversary: rewrite a batch's `previous_head` so it no longer points
/// at the predecessor's recomputed head. The verifier rejects with an
/// explicit previous_head mismatch error.
#[test]
fn chain_previous_head_mismatch_rejected() {
    let mut chain = build_valid_chain(3);
    chain.history[2].previous_head =
        "0101010101010101010101010101010101010101010101010101010101010101".to_string();
    // The convenience head no longer matches either, but we want the
    // primary error to be the previous_head mismatch — fix .head so
    // the test isolates the link-mismatch error specifically.
    chain.head = chain_head_for(&chain.history[2]).unwrap().into_inner();
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(!outcome.ok);
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("previous_head mismatch")),
        "expected previous_head mismatch error, got: {:?}",
        outcome.errors
    );
}

/// Adversary: bump a batch's `batch_index` so the sequence has a gap
/// (0, 1, 3 instead of 0, 1, 2). The verifier rejects with a
/// batch_index error.
#[test]
fn chain_batch_index_gap_rejected() {
    let mut chain = build_valid_chain(3);
    chain.history[2].batch_index = 99;
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(!outcome.ok);
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("batch_index=99")),
        "expected batch_index=99 error, got: {:?}",
        outcome.errors
    );
}

/// Adversary: mutate `chain.head` so it no longer matches the
/// recomputed tip. The convenience field is never trusted; the
/// verifier rejects.
#[test]
fn chain_head_mismatch_rejected() {
    let mut chain = build_valid_chain(2);
    chain.head = "deadbeef00000000deadbeef00000000deadbeef00000000deadbeef00000000".to_string();
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(!outcome.ok);
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("convenience head mismatch")),
        "expected convenience head mismatch, got: {:?}",
        outcome.errors
    );
}

/// Adversary: rewrite the genesis batch's `previous_head` from the
/// all-zero sentinel to anything else. Genesis is identified by index
/// 0; its previous_head must be the sentinel.
#[test]
fn chain_genesis_previous_head_must_be_sentinel() {
    let mut chain = build_valid_chain(1);
    chain.history[0].previous_head =
        "1111111111111111111111111111111111111111111111111111111111111111".to_string();
    chain.head = chain_head_for(&chain.history[0]).unwrap().into_inner();
    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(
        !outcome.ok,
        "genesis batch must carry the all-zero previous_head sentinel"
    );
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("previous_head mismatch")),
        "expected previous_head mismatch, got: {:?}",
        outcome.errors
    );
}

// ─────────────────────────────────────────────────────────────────────────
// End-to-end via verify_trace_with (chain wired into VerifyOptions)
// ─────────────────────────────────────────────────────────────────────────

fn b64url_no_pad_encode(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(bytes)
}

fn build_minimal_trace_with(
    chain: Option<AnchorChain>,
    anchors: Vec<AnchorEntry>,
) -> (AtlasTrace, PubkeyBundle) {
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

    // One signed event, no anchors needed for the chain check.
    let event_id = "01H0CHAINTEST";
    let ts = "2026-04-27T10:00:00Z";
    let payload = serde_json::json!({"type": "node.create", "node": {"id": "n1"}});
    let signing_input =
        build_signing_input("ws-chain", event_id, ts, "spiffe://atlas/test", &[], &payload)
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
        workspace_id: "ws-chain".to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![event.clone()],
        dag_tips: vec![event.event_hash.clone()],
        anchors,
        policies: vec![],
        filters: None,
        anchor_chain: chain,
    };
    (trace, bundle)
}

/// Lenient mode: a trace with NO `anchor_chain` continues to verify
/// (V1.5/V1.6 compatibility). This is the load-bearing
/// backwards-compat assertion for V1.7.
#[test]
fn trace_without_chain_passes_in_lenient_mode() {
    let (trace, bundle) = build_minimal_trace_with(None, vec![]);
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());
    assert!(
        outcome.valid,
        "V1.5/V1.6 trace without anchor_chain must verify under V1.7; errors: {:?}",
        outcome.errors,
    );
}

/// Strict mode: same trace, but with `require_anchor_chain = true` ⇒
/// rejected.
#[test]
fn trace_without_chain_fails_in_strict_mode() {
    let (trace, bundle) = build_minimal_trace_with(None, vec![]);
    let opts = VerifyOptions {
        require_anchor_chain: true,
        ..Default::default()
    };
    let outcome = verify_trace_with(&trace, &bundle, &opts);
    assert!(!outcome.valid);
    assert!(
        outcome.errors.iter().any(|e| e.contains("require_anchor_chain")),
        "expected require_anchor_chain error, got: {:?}",
        outcome.errors,
    );
}

/// Adversary: substitute TWO consecutive batches with attacker-chosen
/// content where the substitute pair is internally consistent (the
/// second batch's previous_head == chain_head_for(first substitute)).
/// Without the loop's break-on-mismatch fix, the verifier would
/// surface only one error at the seam between the honest predecessor
/// and the substitutes, and the recomputed_head exposed in the
/// outcome would point at the attacker's tip. With the fix, walking
/// stops at the first mismatch.
#[test]
fn chain_coordinated_two_batch_rewrite_rejected() {
    // Honest 4-batch chain.
    let honest = build_valid_chain(4);

    // Build 2 substitute batches that link to each OTHER but NOT to
    // honest[0].
    let stray_prev =
        "f00dface00000000f00dface00000000f00dface00000000f00dface00000000".to_string();
    let sub_1 = fixture_batch(1, 1_745_000_500, &stray_prev);
    let sub_1_head = chain_head_for(&sub_1).unwrap().into_inner();
    let sub_2 = fixture_batch(2, 1_745_000_600, &sub_1_head);
    let sub_2_head = chain_head_for(&sub_2).unwrap().into_inner();

    // Splice: keep honest[0], replace honest[1] and honest[2] with
    // substitutes, keep honest[3] as-is. honest[3].previous_head no
    // longer matches anything we recompute, but the verifier should
    // already have stopped at batch[1].
    let mut history = honest.history.clone();
    history[1] = sub_1;
    history[2] = sub_2;
    let chain = AnchorChain {
        history,
        head: sub_2_head, // attacker's tip
    };

    let outcome = atlas_trust_core::anchor::verify_anchor_chain(&chain);
    assert!(!outcome.ok, "coordinated two-batch substitution must fail");
    assert_eq!(
        outcome.batches_walked, 1,
        "verifier must stop walking at the first link break; got batches_walked={}",
        outcome.batches_walked,
    );
    assert!(
        outcome.errors.iter().any(|e| e.contains("previous_head mismatch")),
        "expected previous_head mismatch error, got: {:?}",
        outcome.errors,
    );
    // Convenience-head check is intentionally NOT performed when the
    // walk aborts early — surfacing it would give the attacker a
    // second pseudo-error to hide behind.
    assert!(
        !outcome.errors.iter().any(|e| e.contains("convenience head mismatch")),
        "verifier must NOT surface convenience-head mismatch when the \
         walk aborted early; got: {:?}",
        outcome.errors,
    );
}

/// Coverage check: a trace claims an anchor that's NOT in any chain
/// batch. Rejected as a coverage violation.
#[test]
fn trace_with_anchor_not_in_chain_rejected() {
    // Build a chain with entries indexed 0 and 1.
    let chain = build_valid_chain(1);

    // Build a trace.anchors entry that is NOT in any chain batch.
    let stray_anchor = AnchorEntry {
        kind: AnchorKind::DagTip,
        anchored_hash: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            .to_string(),
        log_id: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
        log_index: 9999, // not in any chain batch
        integrated_time: 1_745_000_999,
        inclusion_proof: InclusionProof {
            tree_size: 10000,
            root_hash:
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            hashes: vec![],
            checkpoint_sig: "AAAA".to_string(),
        },
        entry_body_b64: None,
        tree_id: None,
    };

    let (trace, bundle) = build_minimal_trace_with(Some(chain), vec![stray_anchor]);
    // Use lenient mode for anchors (skips per-entry inclusion proof
    // verification — those would fail since the fixture isn't a real
    // anchor; we are isolating chain-coverage semantics here).
    // The cleanest way is to expect chain-coverage to fire BEFORE the
    // per-entry log_id check fails. Both will produce errors.
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());
    assert!(!outcome.valid);
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("not present in any chain batch")),
        "expected chain-coverage rejection, got: {:?}",
        outcome.errors,
    );
}

/// Coverage proof-swap defence: a chain entry and a trace.anchors
/// entry share trace coordinates (kind, anchored_hash, log_id,
/// log_index) but carry DIFFERENT inclusion proofs. The coverage key
/// includes the proof root_hash + tree_size so this kind of swap is
/// rejected as if the trace anchor were missing entirely from the
/// chain. Without this defence, an attacker could place proof-A in
/// the chain (which never sees per-entry inclusion verification) and
/// proof-B in trace.anchors (which does), and coverage would falsely
/// pass on coordinates while the proofs disagree.
#[test]
fn trace_anchor_with_swapped_proof_rejected_by_coverage() {
    // Build a chain whose only entry has root_hash = 0x77...
    let chain_entry = AnchorEntry {
        kind: AnchorKind::DagTip,
        anchored_hash: format!("{:064x}", 1u64),
        log_id: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
        log_index: 5,
        integrated_time: 1_745_000_001,
        inclusion_proof: InclusionProof {
            tree_size: 8,
            root_hash:
                "7777777777777777777777777777777777777777777777777777777777777777".to_string(),
            hashes: vec![],
            checkpoint_sig: "AAAA".to_string(),
        },
        entry_body_b64: None,
        tree_id: None,
    };
    let batch = AnchorBatch {
        batch_index: 0,
        integrated_time: 1_745_000_001,
        entries: vec![chain_entry.clone()],
        previous_head: ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD.to_string(),
        witnesses: Vec::new(),
    };
    let head = chain_head_for(&batch).unwrap().into_inner();
    let chain = AnchorChain {
        history: vec![batch],
        head,
    };

    // Trace anchor shares (kind, anchored_hash, log_id, log_index) with
    // chain_entry but carries a DIFFERENT root_hash.
    let mut swapped = chain_entry.clone();
    swapped.inclusion_proof.root_hash =
        "8888888888888888888888888888888888888888888888888888888888888888".to_string();

    let (trace, bundle) = build_minimal_trace_with(Some(chain), vec![swapped]);
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());
    assert!(!outcome.valid);
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("not present in any chain batch")),
        "swapped-proof entry must be treated as missing from the \
         chain, not coincidentally covered by the matching-coordinates \
         entry; errors: {:?}",
        outcome.errors,
    );
}

// ─────────────────────────────────────────────────────────────────────────
// V1.8 Sigstore carve-out
// ─────────────────────────────────────────────────────────────────────────

/// Helper for the carve-out tests: build an `AnchorEntry` whose
/// `log_id` matches the pinned Sigstore Rekor v1 production log.
///
/// The fixture is intentionally not a real Sigstore anchor: the
/// inclusion proof and `entry_body_b64` are placeholder bytes, so the
/// `verify_anchors` step ahead of the coverage check rejects the
/// entry on its own merits. That is fine and *not* what these tests
/// pin — coverage logic is independent of per-entry proof validity,
/// because the verifier emits one evidence row per check and we
/// inspect the coverage row directly.
fn fake_sigstore_entry(seed: u64) -> AnchorEntry {
    // Active production shard tree-ID — within roster. Pinned at the
    // construction site so a roster restructure doesn't silently route
    // these tests through the unknown-tree_id rejection branch instead
    // of the inclusion-proof-failure branch they're meant to exercise.
    const ACTIVE_TREE_ID: i64 = 1_193_050_959_916_656_506;
    debug_assert!(
        SIGSTORE_REKOR_V1_TREE_IDS.contains(&ACTIVE_TREE_ID),
        "ACTIVE_TREE_ID {} not in SIGSTORE_REKOR_V1_TREE_IDS — fixture would test the wrong rejection branch",
        ACTIVE_TREE_ID,
    );
    AnchorEntry {
        kind: AnchorKind::DagTip,
        anchored_hash: format!("{:064x}", 0xfeed_face_u64.wrapping_add(seed)),
        log_id: SIGSTORE_REKOR_V1_LOG_ID.clone(),
        log_index: 100_000_000 + seed,
        integrated_time: 1_745_500_000 + seed as i64,
        inclusion_proof: InclusionProof {
            tree_size: 100_000_001 + seed,
            root_hash: format!("{:064x}", 0xc0de_d00d_u64.wrapping_add(seed)),
            hashes: vec![format!("{:064x}", 0xa1b2_c3d4_u64.wrapping_add(seed))],
            checkpoint_sig: "AAAA".to_string(),
        },
        entry_body_b64: Some("AAAA".to_string()),
        tree_id: Some(ACTIVE_TREE_ID),
    }
}

/// V1.8 carve-out: a Sigstore-format anchor that lives in
/// `trace.anchors` but is absent from `trace.anchor_chain` must be
/// accepted by the coverage check. A V1.7 issuer could not extend the
/// chain on the Sigstore path (see V1.7 issuer gate), but Sigstore
/// Rekor v1's publicly-witnessed transparency log gives the same
/// monotonicity guarantee for the entry on its own — coverage must
/// not require chain presence.
///
/// We assert on the coverage *evidence* row (not on `valid`) because
/// the placeholder inclusion proof in the fake Sigstore entry fails
/// per-entry verification upstream, so the trace as a whole is
/// invalid. Coverage is an independent check whose outcome is what
/// this test pins.
#[test]
fn sigstore_anchor_not_in_chain_accepted_by_coverage() {
    let chain = build_valid_chain(2);
    let sigstore_entry = fake_sigstore_entry(0);

    let (trace, bundle) = build_minimal_trace_with(Some(chain), vec![sigstore_entry]);
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());

    let coverage = outcome
        .evidence
        .iter()
        .find(|e| e.check == "anchor-chain-coverage")
        .expect("anchor-chain-coverage evidence row must be present");
    assert!(
        coverage.ok,
        "Sigstore-deferred entry must pass coverage; detail: {}",
        coverage.detail,
    );
    assert!(
        coverage.detail.contains("Sigstore"),
        "coverage evidence must explicitly name the Sigstore deferral; got: {}",
        coverage.detail,
    );
    // No coverage error must surface in the error list — that would
    // mean the carve-out branch ran but still pushed the message.
    assert!(
        !outcome
            .errors
            .iter()
            .any(|e| e.contains("not present in any chain batch")),
        "coverage error must not fire for Sigstore-deferred entry; errors: {:?}",
        outcome.errors,
    );
}

/// Mixed-mode trace: chain holds the mock entries, `trace.anchors`
/// holds the same mock entries (must be in chain) plus a Sigstore
/// entry (deferred). Coverage must accept the Sigstore entry while
/// still asserting the mock entries are present in chain history.
#[test]
fn mixed_mode_mock_in_chain_plus_sigstore_deferred() {
    let chain = build_valid_chain(2);
    // Lift the chain's mock entries into trace.anchors verbatim — the
    // coverage cross-check uses byte-level keys, so cloning is the
    // simplest way to ensure they match.
    let mock_anchors: Vec<AnchorEntry> = chain
        .history
        .iter()
        .flat_map(|b| b.entries.iter().cloned())
        .collect();
    let sigstore_anchor = fake_sigstore_entry(7);
    let mut trace_anchors = mock_anchors.clone();
    trace_anchors.push(sigstore_anchor);

    let (trace, bundle) = build_minimal_trace_with(Some(chain), trace_anchors);
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());

    let coverage = outcome
        .evidence
        .iter()
        .find(|e| e.check == "anchor-chain-coverage")
        .expect("coverage evidence row must be present");
    assert!(
        coverage.ok,
        "mixed-mode (mock-in-chain + Sigstore-deferred) must pass coverage; detail: {}",
        coverage.detail,
    );
    assert!(
        coverage.detail.contains("Sigstore") && coverage.detail.contains("deferred"),
        "coverage detail must call out the deferred Sigstore tail; got: {}",
        coverage.detail,
    );
}

/// Carve-out regression: the carve-out is keyed on
/// `log_id == SIGSTORE_REKOR_V1_LOG_ID`. An anchor with any other
/// `log_id` (here the fixture `deadbeef…` value, neither mock nor
/// Sigstore) that is missing from the chain must still be rejected.
/// Without this assertion, a future refactor could widen the carve-out
/// (e.g. "any non-mock entry passes") and silently break the trust
/// property for unknown logs.
#[test]
fn non_sigstore_anchor_not_in_chain_still_rejected() {
    let chain = build_valid_chain(1);

    let stray = AnchorEntry {
        kind: AnchorKind::DagTip,
        anchored_hash: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            .to_string(),
        // Same as the existing fixture: not the Sigstore log_id.
        log_id: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
        log_index: 9999,
        integrated_time: 1_745_000_999,
        inclusion_proof: InclusionProof {
            tree_size: 10000,
            root_hash:
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            hashes: vec![],
            checkpoint_sig: "AAAA".to_string(),
        },
        entry_body_b64: None,
        tree_id: None,
    };

    let (trace, bundle) = build_minimal_trace_with(Some(chain), vec![stray]);
    let outcome = verify_trace_with(&trace, &bundle, &VerifyOptions::default());
    assert!(!outcome.valid, "non-Sigstore stray entry must still fail");
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("not present in any chain batch")),
        "expected coverage rejection for non-Sigstore stray entry; errors: {:?}",
        outcome.errors,
    );
}
