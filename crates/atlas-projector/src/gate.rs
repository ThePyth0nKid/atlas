//! V2-α Welle 6: projector-state-hash CI gate.
//!
//! Closes the V2-α security loop. Given an `AtlasTrace` containing
//! `ProjectorRunAttestation` events (Welle 4) interleaved with
//! projectable events (`node_create` / `node_update` / `edge_create`,
//! Welle 5 supported set), this gate:
//!
//! 1. Partitions events into projectable + attestation sets
//! 2. Re-projects all projectable events into a fresh `GraphState`
//!    via Welle 5's `project_events`
//! 3. Computes `graph_state_hash` of the recomputed state via Welle 3
//! 4. For each `ProjectorRunAttestation` event, parses the payload
//!    via Welle 4's `parse_projector_run_attestation`, then compares:
//!    - attested `graph_state_hash` vs recomputed
//!    - attested `projected_event_count` vs actual count
//! 5. Returns a `Vec<GateResult>` — one entry per attestation event
//!
//! The result is the **cryptographic equivalent** of "did the issuer's
//! projector and a fresh re-projection agree on what the graph state
//! is for this set of signed events". If yes (`GateStatus::Match`),
//! the attestation is structurally trustworthy. If no, drift —
//! either the trace was tampered (signature would also catch this
//! at the V1 verifier layer) or the projector implementation
//! diverged between issuer and verifier (which the byte-determinism
//! CI pins are supposed to prevent).
//!
//! ## Consumer responsibility (caller-contract)
//!
//! - The trace passed in **MUST be pre-verified** by
//!   `atlas_trust_core::verify_trace`. Welle 6 does NOT re-verify
//!   Ed25519 signatures — that's V1's job and is upstream of
//!   projection-state verification.
//! - The trace MUST contain ONLY events whose payload kinds the
//!   Welle-5 upsert layer supports (`node_create`, `node_update`,
//!   `edge_create`) plus `ProjectorRunAttestation`. Events of other
//!   kinds (`policy_set`, `annotation_add`, `anchor_created`)
//!   surface `ProjectorError::UnsupportedEventKind` during
//!   re-projection. Caller must pre-filter if those events should
//!   be ignored.
//! - The `workspace_id` parameter MUST match the trace's
//!   `workspace_id` field for entity_uuid derivation consistency.
//!
//! ## Welle-6-MVP semantics: full-projection comparison
//!
//! Each `ProjectorRunAttestation` event asserts the FULL projection
//! state at its `head_event_hash`. The gate re-projects ALL
//! non-attestation events in the trace, then compares the
//! recomputed hash against EVERY attestation in the trace.
//!
//! For traces with multiple attestation events emitted at different
//! head heads (incremental projection), this V2-α-MVP semantic will
//! produce mismatches for all but the most-recent attestation —
//! that's the correct outcome for a "current projection state"
//! check. Future welles may add incremental-attestation semantics
//! (each attestation only covers events since the last one), which
//! would change this comparison logic.
//!
//! ## Out of scope for Welle 6
//!
//! - Cross-event integrity: `head_event_hash` actually pointing to
//!   an event in the trace is NOT verified. Future welle may add this.
//! - Re-verification of attestation event's Ed25519 signature —
//!   caller already did this via `verify_trace`.
//! - Operator-runbook integration: Welle 6 is a library; CLI
//!   binding (`atlas-verify-cli --check-projection-attestations`)
//!   is a future-welle CLI ergonomic.

use atlas_trust_core::projector_attestation::{
    parse_projector_run_attestation, validate_projector_run_attestation,
    PROJECTOR_RUN_ATTESTATION_KIND,
};
use atlas_trust_core::trace_format::{AtlasEvent, AtlasTrace};
use serde_json::Value;

use crate::canonical::graph_state_hash;
use crate::error::ProjectorResult;
use crate::upsert::project_events;

/// Outcome of comparing an attestation against re-projection.
///
/// Marked `#[non_exhaustive]` so adding new states in future welles
/// (e.g. `HeadEventHashNotFound`, `IncrementalCoverageGap`) is
/// SemVer-additive.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GateStatus {
    /// Attestation parsed cleanly AND attested hash equals recomputed
    /// hash AND attested `projected_event_count` equals actual count.
    Match,
    /// Attestation parsed cleanly BUT some claim differs from
    /// recomputed reality (either hash differs, or count differs,
    /// or both).
    Mismatch,
    /// Attestation payload failed `parse_projector_run_attestation`.
    /// Note: V1 `verify_trace` would also reject the trace if the
    /// attestation payload is malformed (Welle 4's pre-signature
    /// validation). This status arises if the gate is called on
    /// an unverified trace.
    AttestationParseFailed,
}

/// Per-attestation gate outcome. One `GateResult` per
/// `ProjectorRunAttestation` event in the input trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    /// The `event_id` of the `ProjectorRunAttestation` event.
    pub event_id: String,
    /// The hash the attestation claims (empty string if parse failed).
    pub attested_hash: String,
    /// The hash the gate recomputed from the trace's other events.
    pub recomputed_hash: String,
    /// The event-count the attestation claims (0 if parse failed).
    pub attested_event_count: u64,
    /// The actual count of projectable (non-attestation) events
    /// in the trace.
    pub actual_event_count: u64,
    /// Outcome enum.
    pub status: GateStatus,
}

/// Top-level gate function. Returns one `GateResult` per
/// `ProjectorRunAttestation` event in the input trace. Empty
/// `Vec` if the trace has no attestation events.
///
/// Re-projection of the trace's non-attestation events uses
/// `project_events` (Welle 5). If that re-projection fails (e.g.
/// trace contains an `UnsupportedEventKind` payload), the error
/// surfaces as `Err(_)` — caller decides whether to filter the
/// trace first or treat as a hard failure.
///
/// # Security
///
/// The input trace **MUST be pre-verified** by
/// `atlas_trust_core::verify_trace` before this gate runs.
/// Welle 6 does NOT re-check Ed25519 signatures — that's V1's
/// responsibility and is upstream of projection-state verification.
/// If a caller skips `verify_trace` and uses this gate alone, a
/// `GateStatus::Match` result is **NOT a trust signal** — it is
/// only a projection-consistency signal (i.e., "the attestation
/// payload claims a hash that matches a fresh re-projection of
/// the trace's other events"). Without upstream signature
/// verification, the trace itself is untrusted and an attacker
/// could craft an entire signed-looking-but-unverified trace that
/// passes the gate.
///
/// Semantic validation of each attestation event's payload (hex
/// hash format, non-zero event count, schema-version match) is
/// performed via `atlas_trust_core::validate_projector_run_attestation`.
/// Failure of either parse OR validate surfaces as
/// `GateStatus::AttestationParseFailed`.
pub fn verify_attestations_in_trace(
    workspace_id: &str,
    trace: &AtlasTrace,
) -> ProjectorResult<Vec<GateResult>> {
    // Step 1: partition by payload type. Owned clones only for
    // the projectable set (project_events requires &[AtlasEvent]);
    // attestation set holds references to avoid unnecessary cloning.
    let mut projectable_events: Vec<AtlasEvent> = Vec::new();
    let mut attestation_events: Vec<&AtlasEvent> = Vec::new();
    for ev in &trace.events {
        if event_is_projector_attestation(ev) {
            attestation_events.push(ev);
        } else {
            projectable_events.push(ev.clone());
        }
    }

    // Short-circuit: no attestations means nothing to verify; return
    // empty result list. This is intentional — calling the gate on
    // a V1-shaped trace (no V2-α projector events) is a no-op success.
    if attestation_events.is_empty() {
        return Ok(Vec::new());
    }

    // Step 2: re-project the non-attestation events.
    let state = project_events(workspace_id, &projectable_events, None)?;
    let recomputed_hash_bytes = graph_state_hash(&state)?;
    let recomputed_hex = hex::encode(recomputed_hash_bytes);
    let actual_count = projectable_events.len() as u64;

    // Step 3: per-attestation comparison.
    let mut results = Vec::with_capacity(attestation_events.len());
    for att_event in attestation_events {
        let result = compare_one_attestation(att_event, &recomputed_hex, actual_count);
        results.push(result);
    }

    Ok(results)
}

fn event_is_projector_attestation(ev: &AtlasEvent) -> bool {
    ev.payload
        .get("type")
        .and_then(Value::as_str)
        .map(|t| t == PROJECTOR_RUN_ATTESTATION_KIND)
        .unwrap_or(false)
}

fn compare_one_attestation(
    att_event: &AtlasEvent,
    recomputed_hex: &str,
    actual_count: u64,
) -> GateResult {
    match parse_projector_run_attestation(&att_event.payload) {
        Err(_) => GateResult {
            event_id: att_event.event_id.clone(),
            attested_hash: String::new(),
            recomputed_hash: recomputed_hex.to_string(),
            attested_event_count: 0,
            actual_event_count: actual_count,
            status: GateStatus::AttestationParseFailed,
        },
        Ok(att) => {
            // Semantic validation: enforce 64-lowercase-hex format on
            // both hashes, schema-version equality, non-empty
            // projector_version, non-zero count. Without this step,
            // a structurally-valid-but-semantically-invalid payload
            // would silently surface as `Mismatch` instead of the
            // accurate `AttestationParseFailed` diagnostic
            // (regression caught by both reviewers in V2-α Welle 6
            // review pass — failure to call validator was the
            // gap).
            if validate_projector_run_attestation(&att).is_err() {
                return GateResult {
                    event_id: att_event.event_id.clone(),
                    attested_hash: att.graph_state_hash,
                    recomputed_hash: recomputed_hex.to_string(),
                    attested_event_count: att.projected_event_count,
                    actual_event_count: actual_count,
                    status: GateStatus::AttestationParseFailed,
                };
            }
            let hash_matches = att.graph_state_hash == recomputed_hex;
            let count_matches = att.projected_event_count == actual_count;
            let status = if hash_matches && count_matches {
                GateStatus::Match
            } else {
                GateStatus::Mismatch
            };
            GateResult {
                event_id: att_event.event_id.clone(),
                attested_hash: att.graph_state_hash,
                recomputed_hash: recomputed_hex.to_string(),
                attested_event_count: att.projected_event_count,
                actual_event_count: actual_count,
                status,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_trust_core::trace_format::EventSignature;
    use serde_json::json;

    fn make_event(event_id: &str, payload: Value) -> AtlasEvent {
        AtlasEvent {
            event_id: event_id.to_string(),
            event_hash: "deadbeef".to_string(),
            parent_hashes: vec![],
            payload,
            signature: EventSignature {
                alg: "EdDSA".to_string(),
                kid: "atlas-anchor:ws-test".to_string(),
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
            workspace_id: "ws-test".to_string(),
            pubkey_bundle_hash: "h".to_string(),
            events,
            dag_tips: vec![],
            anchors: vec![],
            anchor_chain: None,
            policies: vec![],
            filters: None,
        }
    }

    const FIXTURE_HEAD: &str = "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";

    #[test]
    fn empty_trace_returns_empty_results() {
        let trace = make_trace(vec![]);
        let results = verify_attestations_in_trace("ws-test", &trace).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn trace_without_attestation_events_returns_empty_results() {
        let trace = make_trace(vec![
            make_event("01HEV1", json!({"type": "node_create", "node": {"id": "n1"}})),
        ]);
        let results = verify_attestations_in_trace("ws-test", &trace).unwrap();
        assert!(results.is_empty(), "no attestation events → no GateResults");
    }
}
