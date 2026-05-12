//! V2-α Welle 4 integration test — `ProjectorRunAttestation` event end-to-end.
//!
//! Exercises the full sign-then-verify path for events carrying a
//! `ProjectorRunAttestation` payload. Catches drift between:
//!   - issuer-side (event-builder) construction of the payload
//!   - verifier-side parse + format-validate via
//!     `parse_projector_run_attestation` + `validate_projector_run_attestation`
//!   - signature binding (the attestation payload is part of the
//!     signing input, so tampering breaks signature verification)
//!
//! Test cases:
//!   - well-formed ProjectorRunAttestation event verifies clean
//!   - tampered hash (well-formed but wrong value) → signature/hash mismatch
//!   - malformed payload (wrong schema_version) → ProjectorAttestationInvalid
//!   - cross-attestation-replay defence (rigorous test analog to Welle 1's
//!     `signature_swap_between_freshly_signed_events_fails`): swapping
//!     signatures between two freshly-signed attestations with different
//!     graph_state_hash values must fail

use atlas_trust_core::{
    cose::build_signing_input,
    projector_attestation::{
        PROJECTOR_RUN_ATTESTATION_KIND, PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION,
    },
    pubkey_bundle::PubkeyBundle,
    trace_format::{AtlasEvent, AtlasTrace, EventSignature},
    verify_trace,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use std::collections::HashMap;

const WORKSPACE: &str = "ws-projector-attestation";
const KID: &str = "atlas-anchor:ws-projector-attestation";
const TS: &str = "2026-05-12T20:00:00Z";

const FIXTURE_HEAD: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const FIXTURE_STATE_A: &str =
    "1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b";
const FIXTURE_STATE_B: &str =
    "2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c";

fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn compute_event_hash(signing_input: &[u8]) -> String {
    hex::encode(blake3::hash(signing_input).as_bytes())
}

fn make_bundle(signing_key: &SigningKey) -> PubkeyBundle {
    let mut keys = HashMap::new();
    keys.insert(KID.to_string(), b64url(signing_key.verifying_key().as_bytes()));
    PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-05-12T19:59:00Z".to_string(),
        keys,
    }
}

fn attestation_payload(graph_state_hash: &str, count: u64) -> serde_json::Value {
    json!({
        "type": PROJECTOR_RUN_ATTESTATION_KIND,
        "projector_version": "atlas-projector/0.1.0",
        "projector_schema_version": PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION,
        "head_event_hash": FIXTURE_HEAD,
        "graph_state_hash": graph_state_hash,
        "projected_event_count": count,
    })
}

fn build_attestation_event(
    signing_key: &SigningKey,
    event_id: &str,
    payload: serde_json::Value,
) -> AtlasEvent {
    let signing_input =
        build_signing_input(WORKSPACE, event_id, TS, KID, &[], &payload, None).unwrap();
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
        author_did: None,
    }
}

fn make_trace(bundle: &PubkeyBundle, events: Vec<AtlasEvent>) -> AtlasTrace {
    let bundle_hash = bundle.deterministic_hash().unwrap();
    let dag_tips: Vec<String> = events.iter().map(|e| e.event_hash.clone()).collect();
    AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-05-12T20:00:30Z".to_string(),
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
fn well_formed_attestation_event_verifies_clean() {
    let signing_key = SigningKey::from_bytes(&[0x77; 32]);
    let bundle = make_bundle(&signing_key);
    let event = build_attestation_event(
        &signing_key,
        "01HPATTEST0001",
        attestation_payload(FIXTURE_STATE_A, 42),
    );
    let trace = make_trace(&bundle, vec![event]);
    let outcome = verify_trace(&trace, &bundle);
    assert!(
        outcome.valid,
        "well-formed ProjectorRunAttestation event must verify clean: errors = {:?}",
        outcome.errors
    );
}

#[test]
fn malformed_attestation_schema_version_rejected_at_verify_time() {
    // Issuer signs an event whose attestation payload has the WRONG
    // schema_version. The verifier rejects via
    // `validate_projector_run_attestation` before signature check.
    let signing_key = SigningKey::from_bytes(&[0x88; 32]);
    let bundle = make_bundle(&signing_key);
    let bad_payload = json!({
        "type": PROJECTOR_RUN_ATTESTATION_KIND,
        "projector_version": "atlas-projector/0.1.0",
        "projector_schema_version": "wrong-version",
        "head_event_hash": FIXTURE_HEAD,
        "graph_state_hash": FIXTURE_STATE_A,
        "projected_event_count": 1_u64,
    });
    let event = build_attestation_event(&signing_key, "01HPATTEST0002", bad_payload);
    let trace = make_trace(&bundle, vec![event]);
    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid, "malformed attestation must fail verification");
    let combined = outcome.errors.join(" | ");
    assert!(
        combined.contains("invalid projector-attestation"),
        "expected ProjectorAttestationInvalid in errors; got: {combined}"
    );
    assert!(
        combined.contains("schema_version mismatch"),
        "expected schema_version mismatch reason; got: {combined}"
    );
}

#[test]
fn malformed_attestation_hex_hash_rejected_at_verify_time() {
    // Wrong-length head_event_hash (28 chars instead of 64).
    let signing_key = SigningKey::from_bytes(&[0x99; 32]);
    let bundle = make_bundle(&signing_key);
    let bad_payload = json!({
        "type": PROJECTOR_RUN_ATTESTATION_KIND,
        "projector_version": "atlas-projector/0.1.0",
        "projector_schema_version": PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION,
        "head_event_hash": "0a0a0a0a0a0a0a0a0a0a0a0a0a0a",
        "graph_state_hash": FIXTURE_STATE_A,
        "projected_event_count": 1_u64,
    });
    let event = build_attestation_event(&signing_key, "01HPATTEST0003", bad_payload);
    let trace = make_trace(&bundle, vec![event]);
    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid);
    let combined = outcome.errors.join(" | ");
    assert!(
        combined.contains("head_event_hash"),
        "expected head_event_hash format error; got: {combined}"
    );
}

#[test]
fn tampered_attestation_after_signing_breaks_signature() {
    // Sign event with one attestation payload, then tamper the
    // graph_state_hash field to a different (still well-formed) value.
    // Signature was computed over the original payload; verifier
    // recomputes signing-input from the tampered payload → bytes
    // differ → either HashMismatch (check_event_hashes) or BadSignature
    // (per-event signature loop) — both are correct outcomes proving
    // the attestation is bound into the signature.
    let signing_key = SigningKey::from_bytes(&[0xAA; 32]);
    let bundle = make_bundle(&signing_key);
    let mut event = build_attestation_event(
        &signing_key,
        "01HPATTEST0004",
        attestation_payload(FIXTURE_STATE_A, 10),
    );
    // Tamper: substitute graph_state_hash with different well-formed hex
    event.payload["graph_state_hash"] = json!(FIXTURE_STATE_B);

    let trace = make_trace(&bundle, vec![event]);
    let outcome = verify_trace(&trace, &bundle);
    assert!(!outcome.valid, "tampered attestation must fail verification");
    let combined = outcome.errors.join(" | ");
    assert!(
        combined.contains("invalid signature")
            || combined.contains("hash mismatch")
            || combined.contains("HashMismatch"),
        "expected signature/hash failure; got: {combined}"
    );
}

#[test]
fn signature_swap_between_freshly_signed_attestations_fails() {
    // Rigorous Phase-2-Security-H-1-style test: prove that the
    // attestation payload is cryptographically bound into BOTH the
    // event_hash (via `check_event_hashes` which recomputes from the
    // canonical signing input) AND the Ed25519 signature (which is
    // computed over the same signing input).
    //
    // Construct two events freshly signed with the SAME signing key,
    // SAME event_id, SAME parents+ts+workspace+kid, but DIFFERENT
    // attestation payload (`graph_state_hash` field differs). Because
    // the payload is part of `build_signing_input`, the two events
    // produce different signing-input bytes → different `event_hash`
    // values + different signatures. Swap event_a's signature onto
    // event_b's wire shape (event_b keeps its own payload + its own
    // event_hash). The verifier rebuilds the signing input from the
    // tampered event's payload + wire-shape → signing-input matches
    // signing_input_b → event_hash matches → hash check passes →
    // signature check verifies sig_a against signing_input_b → fails
    // (sig_a was signed over signing_input_a, not signing_input_b).
    //
    // The shared event_id is intentional: we want the only difference
    // between the two signing inputs to be the payload's graph_state_hash
    // field, so the failure mode isolates payload-binding from
    // header-field-binding.
    //
    // Expected error in tampered trace: `BadSignature` (signature
    // verification fails against rebuilt-from-tampered-payload
    // signing input). HashMismatch from `check_event_hashes` would
    // also fire if we tampered the payload AFTER signing, but here
    // we swap signatures and keep the wire-payload consistent with
    // its event_hash, so HashMismatch does NOT fire — proving the
    // signature is independently bound to the payload.
    let signing_key = SigningKey::from_bytes(&[0xBB; 32]);
    let bundle = make_bundle(&signing_key);

    let event_a = build_attestation_event(
        &signing_key,
        "01HPATTESWAP",
        attestation_payload(FIXTURE_STATE_A, 100),
    );
    let event_b = build_attestation_event(
        &signing_key,
        "01HPATTESWAP",
        attestation_payload(FIXTURE_STATE_B, 100),
    );

    // Sanity: each event verifies individually.
    let trace_a = make_trace(&bundle, vec![event_a.clone()]);
    assert!(verify_trace(&trace_a, &bundle).valid, "event_a in isolation must verify");
    let trace_b = make_trace(&bundle, vec![event_b.clone()]);
    assert!(verify_trace(&trace_b, &bundle).valid, "event_b in isolation must verify");

    // Swap: event_b shape (payload + event_hash) + event_a's signature.
    // event_b's event_hash was correctly computed from signing_input_b,
    // so check_event_hashes will PASS — proving the BadSignature failure
    // below is NOT a hash-mismatch downstream effect.
    let mut tampered = event_b.clone();
    tampered.signature.sig = event_a.signature.sig.clone();
    let trace_tampered = make_trace(&bundle, vec![tampered]);
    let outcome = verify_trace(&trace_tampered, &bundle);
    assert!(
        !outcome.valid,
        "signature-swap across different graph_state_hash values must fail; \
         attestation payload binding into signature is broken otherwise"
    );
    let combined = outcome.errors.join(" | ");
    assert!(
        combined.contains("invalid signature"),
        "expected BadSignature (signature-over-different-payload); got: {combined}"
    );
    // Confirm that HashMismatch did NOT fire — proves the BadSignature
    // is the direct evidence of payload-into-signature binding, not a
    // downstream effect of hash recomputation.
    assert!(
        !combined.contains("hash mismatch"),
        "expected NO HashMismatch (event_hash matches signing_input_b); \
         got: {combined}. If HashMismatch is present, this test is no \
         longer isolating signature binding from hash binding."
    );
}
