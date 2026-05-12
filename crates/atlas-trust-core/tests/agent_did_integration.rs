//! V2-α Welle 1 integration test — Agent-DID Schema end-to-end.
//!
//! Exercises the full sign-then-verify path with `author_did` present
//! and absent. Catches drift between the issuer-side path (used by
//! atlas-signer and future agent SDKs) and the verifier-side path
//! (atlas-trust-core verify_trace), which are the two surfaces that
//! must agree byte-for-byte on the V2-α signing input.
//!
//! Test cases:
//!   * `event_with_author_did_round_trips` — sign + verify with
//!     `author_did = Some(...)` succeeds.
//!   * `event_without_author_did_round_trips` — sign + verify with
//!     `author_did = None` succeeds (V1 backward-compat invariant).
//!   * `malformed_author_did_is_rejected_at_verify_time` — verifier
//!     rejects an event whose `author_did` violates the
//!     `did:atlas:<lowercase-hex-32-bytes>` format with the structured
//!     `AgentDidFormatInvalid` error reported through `errors`.
//!   * `author_did_is_bound_into_signature` — tampering with
//!     `author_did` after signing (replacing the DID, keeping the
//!     signature) fails the trust chain via `BadSignature` because the
//!     signature was computed over the original DID.

use atlas_trust_core::{
    agent_did::agent_did_for,
    cose::build_signing_input,
    pubkey_bundle::PubkeyBundle,
    trace_format::{AtlasEvent, AtlasTrace, EventSignature},
    verify_trace,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;

const WORKSPACE: &str = "ws-agent-did-test";
const KID: &str = "atlas-anchor:ws-agent-did-test";
const TS: &str = "2026-05-12T18:00:00Z";

fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn compute_event_hash(signing_input: &[u8]) -> String {
    hex::encode(blake3::hash(signing_input).as_bytes())
}

fn compute_pubkey_hash(vk: &ed25519_dalek::VerifyingKey) -> String {
    hex::encode(blake3::hash(vk.as_bytes()).as_bytes())
}

fn make_bundle(signing_key: &SigningKey) -> PubkeyBundle {
    let mut keys = HashMap::new();
    keys.insert(KID.to_string(), b64url(signing_key.verifying_key().as_bytes()));
    PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-05-12T17:59:00Z".to_string(),
        keys,
    }
}

fn build_event(
    signing_key: &SigningKey,
    event_id: &str,
    payload: serde_json::Value,
    author_did: Option<&str>,
) -> AtlasEvent {
    let signing_input =
        build_signing_input(WORKSPACE, event_id, TS, KID, &[], &payload, author_did).unwrap();
    let event_hash = compute_event_hash(&signing_input);
    let sig = signing_key.sign(&signing_input);
    AtlasEvent {
        event_id: event_id.to_string(),
        event_hash,
        parent_hashes: vec![],
        payload,
        signature: EventSignature {
            alg: "EdDSA".to_string(),
            kid: KID.to_string(),
            sig: b64url(&sig.to_bytes()),
        },
        ts: TS.to_string(),
        author_did: author_did.map(|s| s.to_string()),
    }
}

fn make_trace(bundle: &PubkeyBundle, events: Vec<AtlasEvent>) -> AtlasTrace {
    let bundle_hash = bundle.deterministic_hash().unwrap();
    // dag_tips = the event_hash of every leaf (events without children).
    // For a single-event trace, the only event is itself the tip.
    let dag_tips: Vec<String> = events.iter().map(|e| e.event_hash.clone()).collect();
    AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-05-12T18:00:30Z".to_string(),
        workspace_id: WORKSPACE.to_string(),
        pubkey_bundle_hash: bundle_hash,
        events,
        dag_tips,
        anchors: vec![],
        anchor_chain: None,
        policies: vec![],
        filters: None,
    }
}

#[test]
fn event_with_author_did_round_trips() {
    let signing_key = SigningKey::from_bytes(&[0x11; 32]);
    let bundle = make_bundle(&signing_key);

    let pubkey_hash = compute_pubkey_hash(&signing_key.verifying_key());
    let did = agent_did_for(&pubkey_hash);

    let event = build_event(
        &signing_key,
        "01H0AGENTDIDTEST1",
        serde_json::json!({"type": "node.create", "node": {"id": "n1"}}),
        Some(&did),
    );

    let trace = make_trace(&bundle, vec![event]);

    let outcome = verify_trace(&trace, &bundle);
    assert!(
        outcome.valid,
        "trace with author_did must verify clean: errors = {:?}",
        outcome.errors
    );
    assert!(outcome.errors.is_empty(), "expected no errors; got {:?}", outcome.errors);
}

#[test]
fn event_without_author_did_round_trips() {
    // V1 backward-compat: trace without any author_did must verify
    // exactly as it did pre-Welle-1.
    let signing_key = SigningKey::from_bytes(&[0x22; 32]);
    let bundle = make_bundle(&signing_key);

    let event = build_event(
        &signing_key,
        "01H0AGENTDIDTEST2",
        serde_json::json!({"type": "node.create", "node": {"id": "n2"}}),
        None,
    );

    let trace = make_trace(&bundle, vec![event]);

    let outcome = verify_trace(&trace, &bundle);
    assert!(
        outcome.valid,
        "V1-shaped trace (no author_did) must verify clean: errors = {:?}",
        outcome.errors
    );
}

#[test]
fn malformed_author_did_is_rejected_at_verify_time() {
    // Issuer signs a well-formed DID, then we tamper the event to
    // carry a malformed DID. The verifier's `validate_agent_did` check
    // surfaces an `AgentDidFormatInvalid` error before signature check
    // would have a chance to fail.
    let signing_key = SigningKey::from_bytes(&[0x33; 32]);
    let bundle = make_bundle(&signing_key);

    let pubkey_hash = compute_pubkey_hash(&signing_key.verifying_key());
    let good_did = agent_did_for(&pubkey_hash);

    let mut event = build_event(
        &signing_key,
        "01H0AGENTDIDTEST3",
        serde_json::json!({"type": "node.create"}),
        Some(&good_did),
    );

    // Tamper: replace with malformed DID (wrong method, missing hex
    // suffix). The signature was computed over the ORIGINAL signing
    // input (with the well-formed DID). The verifier path is:
    //   1. check_event_hashes recomputes signing_input from the tampered
    //      event → different bytes → HashMismatch fires
    //   2. per-event signature loop calls validate_agent_did(tampered) →
    //      AgentDidFormatInvalid fires
    // Both errors land in outcome.errors. This test asserts the
    // structured AgentDidFormatInvalid is present (not that it's the
    // first or only error); auditor tooling can switch on the variant
    // to pinpoint the format violation regardless of order.
    event.author_did = Some("did:malformed:not-hex".to_string());

    let trace = make_trace(&bundle, vec![event]);

    let outcome = verify_trace(&trace, &bundle);
    assert!(
        !outcome.valid,
        "tampered author_did must fail verification"
    );
    let combined = outcome.errors.join(" | ");
    assert!(
        combined.contains("invalid agent-DID"),
        "expected 'invalid agent-DID' error; got: {combined}"
    );
}

#[test]
fn author_did_is_bound_into_signature() {
    // Phase 2 Security H-1 demand: author_did is part of the signing
    // input. Tampering with author_did (substituting a different
    // well-formed DID, keeping the original signature) must fail
    // signature verification. This is the cross-agent-replay defence.
    let signing_key = SigningKey::from_bytes(&[0x44; 32]);
    let bundle = make_bundle(&signing_key);

    let pubkey_hash = compute_pubkey_hash(&signing_key.verifying_key());
    let original_did = agent_did_for(&pubkey_hash);

    let mut event = build_event(
        &signing_key,
        "01H0AGENTDIDTEST4",
        serde_json::json!({"type": "node.create"}),
        Some(&original_did),
    );

    // Tamper: substitute a DIFFERENT well-formed DID. Format validates
    // fine; signature recomputation fails because the new DID changes
    // the signing input bytes.
    let other_pubkey_hash =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let other_did = agent_did_for(other_pubkey_hash);
    event.author_did = Some(other_did);

    let trace = make_trace(&bundle, vec![event]);

    let outcome = verify_trace(&trace, &bundle);
    assert!(
        !outcome.valid,
        "tampered (but well-formed) author_did must fail signature check"
    );
    let combined = outcome.errors.join(" | ");
    // Either invalid signature OR hash mismatch — both are correct
    // outcomes: the signature was computed over different bytes than
    // the tampered event presents. Hash check runs before signature
    // check in atlas-trust-core, so HashMismatch is the more likely
    // surface, but either way the cross-agent-replay defence holds.
    //
    // For a more rigorous test that isolates the signing-input binding
    // specifically, see `signature_swap_between_freshly_signed_events_fails`
    // below — that one freshly signs two events with different DIDs and
    // attempts to use one's signature on the other's payload.
    assert!(
        combined.contains("invalid signature")
            || combined.contains("hash mismatch")
            || combined.contains("HashMismatch"),
        "expected signature or hash failure; got: {combined}"
    );
}

#[test]
fn signature_swap_between_freshly_signed_events_fails() {
    // Phase 2 Security H-1 rigorous test: prove that author_did is
    // cryptographically bound into the signature itself, not just into
    // event_hash. Construct two events freshly signed by the SAME key
    // with DIFFERENT well-formed DIDs (same payload, same workspace,
    // same timestamp). Attempt to substitute event_a's signature into
    // event_b's metadata. Both signatures are individually valid; the
    // crossover must fail because each signature was computed over a
    // signing-input that included its own DID.
    //
    // This test catches a hypothetical future regression where author_did
    // is added to event_hash computation but accidentally dropped from
    // the actual signing-input passed to Ed25519. In that scenario,
    // tests like `author_did_is_bound_into_signature` would still pass
    // (HashMismatch would fire), but cross-agent-replay defence at the
    // signature layer would be silently broken. This test fails only
    // when the signing-input binding holds.
    let signing_key = SigningKey::from_bytes(&[0x55; 32]);
    let bundle = make_bundle(&signing_key);

    let did_a = agent_did_for(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    );
    let did_b = agent_did_for(
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    assert_ne!(did_a, did_b);

    let payload = serde_json::json!({"type": "node.create", "node": {"id": "shared"}});

    let event_a = build_event(&signing_key, "01HAUTHORDIDA", payload.clone(), Some(&did_a));
    let event_b = build_event(&signing_key, "01HAUTHORDIDA", payload.clone(), Some(&did_b));

    // Sanity: both events individually verify clean.
    let trace_a_only = make_trace(&bundle, vec![event_a.clone()]);
    assert!(
        verify_trace(&trace_a_only, &bundle).valid,
        "event_a in isolation must verify"
    );
    let trace_b_only = make_trace(&bundle, vec![event_b.clone()]);
    assert!(
        verify_trace(&trace_b_only, &bundle).valid,
        "event_b in isolation must verify"
    );

    // Swap: take event_b's metadata (author_did = did_b, event_hash from
    // signing-input including did_b) and overwrite ITS signature with
    // event_a's signature (computed over signing-input including did_a).
    // Since the signing-inputs are different (different DIDs), the
    // signatures cannot validly cross over. If author_did were NOT in
    // the signing input, the two signing-inputs would be byte-identical
    // and the swap would succeed — that's the regression this test
    // catches.
    let mut tampered = event_b.clone();
    tampered.signature.sig = event_a.signature.sig.clone();

    let trace_tampered = make_trace(&bundle, vec![tampered]);
    let outcome = verify_trace(&trace_tampered, &bundle);
    assert!(
        !outcome.valid,
        "signature-swap across different author_did values must fail; \
         either author_did is not in the signing input (regression) or \
         signature/hash check correctly rejected"
    );
    let combined = outcome.errors.join(" | ");
    // The tampered event has did_b + did_b-derived event_hash + did_a-derived sig.
    // The verifier recomputes signing_input(did_b) and computes blake3 of that —
    // matches event_b's original event_hash, so hash check passes. Then signature
    // check: verifies sig (over signing_input_a) against signing_input_b → fails
    // with BadSignature. This is the "signature is bound to DID" property.
    assert!(
        combined.contains("invalid signature"),
        "expected BadSignature (signature-over-different-DID); got: {combined}"
    );
}
