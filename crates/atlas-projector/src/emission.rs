//! V2-α Welle 5: ProjectorRunAttestation payload emission.
//!
//! Given a finalised `GraphState` plus caller-supplied
//! `projector_version`, `head_event_hash`, and `projected_event_count`,
//! constructs a JSON payload matching atlas-trust-core's
//! `PROJECTOR_RUN_ATTESTATION_KIND` shape (per Welle 4). The
//! caller is responsible for wrapping the payload in an
//! `AtlasEvent` and signing it with their workspace key — Welle 5
//! does NOT sign.
//!
//! ## Why payload-only, not full AtlasEvent
//!
//! Signing requires:
//! - access to the per-tenant signing key (atlas-signer / HSM)
//! - choice of `event_id` (typically ULID-generated at signing time)
//! - choice of `parent_hashes` (typically the previous trace tip)
//! - `ts` clock-reading
//!
//! All of these are issuer-environment concerns separated from
//! projection. Welle 5 produces just the deterministic payload;
//! atlas-signer or downstream SDKs assemble + sign the wrapping
//! event.
//!
//! ## Round-trip invariant
//!
//! `build_projector_run_attestation_payload` produces a
//! `serde_json::Value` that:
//!
//! - `atlas_trust_core::parse_projector_run_attestation` accepts
//!   without error
//! - `atlas_trust_core::validate_projector_run_attestation`
//!   validates green
//!
//! Both halves are exercised by the
//! `crates/atlas-projector/tests/projector_pipeline_integration.rs`
//! end-to-end test.

use atlas_trust_core::projector_attestation::{
    PROJECTOR_RUN_ATTESTATION_KIND, PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION,
};
use serde_json::{json, Value};

use crate::canonical::graph_state_hash;
use crate::error::{ProjectorError, ProjectorResult};
use crate::state::GraphState;

/// Build a `ProjectorRunAttestation` payload from a finalised
/// `GraphState`. Computes `graph_state_hash` internally via
/// Welle 3's canonicalisation.
///
/// ## Caller-supplied fields
///
/// - `projector_version`: must be non-empty. Typically
///   `"atlas-projector/0.1.0"`.
/// - `head_event_hash`: must be exactly 64 lowercase-hex characters
///   (blake3 width). The `event_hash` of the last Atlas event the
///   projector consumed before this attestation. **Validated at the
///   emission boundary** — a malformed value caught here prevents
///   a bad attestation from being signed + Rekor-anchored into
///   Layer 1 (post-sign rejection by the verifier is too late;
///   the signed bad event already entered the trust chain).
///   Defense-in-depth: atlas-trust-core's
///   `validate_projector_run_attestation` ALSO enforces the format
///   on the verifier side, but emission is the producer's last
///   chance to catch the issue pre-signature.
/// - `projected_event_count`: must be >= 1. Caller's responsibility
///   to track the count during projection.
///
/// ## Returned shape
///
/// ```json
/// {
///   "type": "projector_run_attestation",
///   "projector_version": "atlas-projector/0.1.0",
///   "projector_schema_version": "atlas-projector-run-attestation/v1-alpha",
///   "head_event_hash": "<64-lowercase-hex>",
///   "graph_state_hash": "<64-lowercase-hex>",
///   "projected_event_count": <u64>
/// }
/// ```
///
/// The `projector_schema_version` is bound to atlas-trust-core's
/// constant — emission stays in lockstep with validation.
pub fn build_projector_run_attestation_payload(
    state: &GraphState,
    projector_version: &str,
    head_event_hash: &str,
    projected_event_count: u64,
) -> ProjectorResult<Value> {
    if projector_version.trim().is_empty() {
        return Err(ProjectorError::CanonicalisationFailed(
            "projector_version is empty".to_string(),
        ));
    }
    if projected_event_count == 0 {
        return Err(ProjectorError::CanonicalisationFailed(
            "projected_event_count must be >= 1".to_string(),
        ));
    }
    // Defense-in-depth: head_event_hash format validated at emission
    // site so a malformed value cannot enter a signed + Rekor-anchored
    // Layer-1 event. Verifier-side validation in atlas-trust-core's
    // validate_projector_run_attestation is the second line; this is
    // the first.
    if !is_blake3_hex(head_event_hash) {
        return Err(ProjectorError::CanonicalisationFailed(format!(
            "head_event_hash must be exactly 64 lowercase-hex characters (got {} chars)",
            head_event_hash.len()
        )));
    }
    let state_hash_bytes = graph_state_hash(state)?;
    let state_hash_hex = hex::encode(state_hash_bytes);

    Ok(json!({
        "type": PROJECTOR_RUN_ATTESTATION_KIND,
        "projector_version": projector_version,
        "projector_schema_version": PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION,
        "head_event_hash": head_event_hash,
        "graph_state_hash": state_hash_hex,
        "projected_event_count": projected_event_count,
    }))
}

/// blake3-hex format check: exactly 64 lowercase-hex characters.
/// Mirrors atlas-trust-core's verifier-side validator predicate so
/// emission and validation use identical acceptance rules.
fn is_blake3_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_trust_core::{
        parse_projector_run_attestation, validate_projector_run_attestation,
    };

    fn fixture_state() -> GraphState {
        use crate::state::GraphNode;
        use std::collections::BTreeMap;
        let mut s = GraphState::new();
        s.upsert_node(GraphNode {
            entity_uuid: "node-a".to_string(),
            labels: vec!["L".to_string()],
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT1".to_string(),
            rekor_log_index: 0,
            author_did: None,
        });
        s
    }

    const FIXTURE_HEAD: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";

    #[test]
    fn emission_produces_payload_that_round_trips_through_atlas_trust_core() {
        // The Welle 5 ↔ Welle 4 round-trip invariant: emission output
        // MUST be acceptable input to the verifier-side parser +
        // validator. This is the load-bearing E2E contract.
        let state = fixture_state();
        let payload = build_projector_run_attestation_payload(
            &state,
            "atlas-projector/0.1.0",
            FIXTURE_HEAD,
            1,
        )
        .unwrap();

        let att = parse_projector_run_attestation(&payload).expect("parse failed");
        validate_projector_run_attestation(&att).expect("validate failed");
    }

    #[test]
    fn emission_rejects_empty_projector_version() {
        let state = fixture_state();
        match build_projector_run_attestation_payload(&state, "   ", FIXTURE_HEAD, 1) {
            Err(ProjectorError::CanonicalisationFailed(reason)) => {
                assert!(reason.contains("projector_version is empty"));
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn emission_rejects_zero_event_count() {
        let state = fixture_state();
        match build_projector_run_attestation_payload(
            &state,
            "atlas-projector/0.1.0",
            FIXTURE_HEAD,
            0,
        ) {
            Err(ProjectorError::CanonicalisationFailed(reason)) => {
                assert!(reason.contains("projected_event_count must be >= 1"));
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn emission_schema_version_matches_atlas_trust_core_constant() {
        let state = fixture_state();
        let payload = build_projector_run_attestation_payload(
            &state,
            "atlas-projector/0.1.0",
            FIXTURE_HEAD,
            1,
        )
        .unwrap();
        assert_eq!(
            payload["projector_schema_version"].as_str().unwrap(),
            PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION
        );
    }

    #[test]
    fn emission_kind_matches_atlas_trust_core_constant() {
        let state = fixture_state();
        let payload = build_projector_run_attestation_payload(
            &state,
            "atlas-projector/0.1.0",
            FIXTURE_HEAD,
            1,
        )
        .unwrap();
        assert_eq!(
            payload["type"].as_str().unwrap(),
            PROJECTOR_RUN_ATTESTATION_KIND
        );
    }

    #[test]
    fn emission_rejects_malformed_head_event_hash() {
        // Defense-in-depth at emission boundary: a malformed
        // head_event_hash must be rejected BEFORE the payload is
        // signed + anchored. Welle-4 verifier-side validation is
        // the second line; this is the first.
        let state = fixture_state();

        // Too short
        match build_projector_run_attestation_payload(
            &state,
            "atlas-projector/0.1.0",
            "abc",
            1,
        ) {
            Err(ProjectorError::CanonicalisationFailed(r)) => {
                assert!(r.contains("head_event_hash"));
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }

        // Uppercase hex
        let upper = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        match build_projector_run_attestation_payload(
            &state,
            "atlas-projector/0.1.0",
            upper,
            1,
        ) {
            Err(ProjectorError::CanonicalisationFailed(r)) => {
                assert!(r.contains("head_event_hash"));
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }

        // Non-hex character
        let non_hex = "g000000000000000000000000000000000000000000000000000000000000000";
        match build_projector_run_attestation_payload(
            &state,
            "atlas-projector/0.1.0",
            non_hex,
            1,
        ) {
            Err(ProjectorError::CanonicalisationFailed(r)) => {
                assert!(r.contains("head_event_hash"));
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }

        // Empty
        match build_projector_run_attestation_payload(&state, "atlas-projector/0.1.0", "", 1) {
            Err(ProjectorError::CanonicalisationFailed(r)) => {
                assert!(r.contains("head_event_hash"));
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn emission_graph_state_hash_matches_canonical() {
        // The emitted graph_state_hash must equal what
        // canonical::graph_state_hash produces for the same state.
        let state = fixture_state();
        let direct_hash = graph_state_hash(&state).unwrap();
        let payload = build_projector_run_attestation_payload(
            &state,
            "atlas-projector/0.1.0",
            FIXTURE_HEAD,
            1,
        )
        .unwrap();
        let payload_hash = payload["graph_state_hash"].as_str().unwrap();
        assert_eq!(payload_hash, hex::encode(direct_hash));
    }
}
