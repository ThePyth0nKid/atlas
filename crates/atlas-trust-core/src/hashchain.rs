//! Hash-chain integrity (linear + DAG).
//!
//! For Atlas V1 we support DAG: an event has zero-or-more parent_hashes.
//! A "tip" is an event whose hash is referenced by no other event's parents.

use blake3::Hasher;
use std::collections::{HashMap, HashSet};

use crate::error::{TrustError, TrustResult};
use crate::trace_format::AtlasEvent;

/// Compute the blake3 hash of canonical signing-input bytes.
///
/// This is the value stored as `event_hash`. It MUST be reproducible bit-for-bit
/// from the same inputs across the Rust verifier, the Node.js signer,
/// and the WASM-in-browser path.
pub fn compute_event_hash(signing_input: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(signing_input);
    let hash = hasher.finalize();
    hex::encode(hash.as_bytes())
}

/// Verify each event's claimed `event_hash` matches the recomputed hash
/// over its canonical signing-input, and reject any duplicate event_hashes.
///
/// `workspace_id` is bound into the signing-input. Verifying with a different
/// workspace id will produce a different recomputed hash, which is the
/// intended defence against cross-workspace replay.
///
/// Hash comparison is constant-time. Two events sharing an event_hash
/// trigger `DuplicateEventHash`.
pub fn check_event_hashes(workspace_id: &str, events: &[AtlasEvent]) -> TrustResult<()> {
    let mut seen: HashSet<&str> = HashSet::with_capacity(events.len());
    for ev in events {
        if !seen.insert(ev.event_hash.as_str()) {
            return Err(TrustError::DuplicateEventHash {
                event_hash: ev.event_hash.clone(),
            });
        }
        let signing_input = crate::cose::build_signing_input(
            workspace_id,
            &ev.event_id,
            &ev.ts,
            &ev.signature.kid,
            &ev.parent_hashes,
            &ev.payload,
        )?;
        let computed = compute_event_hash(&signing_input);
        if !crate::ct::ct_eq_str(&computed, &ev.event_hash) {
            return Err(TrustError::HashMismatch {
                event_id: ev.event_id.clone(),
                claimed: ev.event_hash.clone(),
                computed,
            });
        }
    }
    Ok(())
}

/// Verify that every parent_hash referenced by any event is itself
/// the event_hash of some event in the trace (no dangling parents).
///
/// Genesis events have empty parent_hashes — that's allowed.
pub fn check_parent_links(events: &[AtlasEvent]) -> TrustResult<()> {
    let known: HashSet<&str> = events.iter().map(|e| e.event_hash.as_str()).collect();

    for ev in events {
        for parent in &ev.parent_hashes {
            if !known.contains(parent.as_str()) {
                return Err(TrustError::DanglingParent {
                    event_id: ev.event_id.clone(),
                    parent_hash: parent.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Compute current DAG-tips: events whose hash is referenced by no parent_hashes.
pub fn compute_tips(events: &[AtlasEvent]) -> Vec<String> {
    let mut referenced: HashMap<&str, bool> = events.iter().map(|e| (e.event_hash.as_str(), false)).collect();
    for ev in events {
        for parent in &ev.parent_hashes {
            if let Some(slot) = referenced.get_mut(parent.as_str()) {
                *slot = true;
            }
        }
    }
    let mut tips: Vec<String> = referenced
        .into_iter()
        .filter_map(|(h, is_referenced)| if !is_referenced { Some(h.to_string()) } else { None })
        .collect();
    tips.sort();
    tips
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace_format::EventSignature;
    use serde_json::json;

    const TEST_WORKSPACE: &str = "ws-test";

    fn make_event(id: &str, parents: Vec<String>, payload: serde_json::Value) -> AtlasEvent {
        let signing_input = crate::cose::build_signing_input(
            TEST_WORKSPACE,
            id,
            "2026-04-27T10:00:00Z",
            "spiffe://atlas/test",
            &parents,
            &payload,
        )
        .unwrap();
        let event_hash = compute_event_hash(&signing_input);
        AtlasEvent {
            event_id: id.to_string(),
            event_hash,
            parent_hashes: parents,
            payload,
            signature: EventSignature {
                alg: "EdDSA".to_string(),
                kid: "spiffe://atlas/test".to_string(),
                sig: "00".to_string(),
            },
            ts: "2026-04-27T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn hash_matches() {
        let ev = make_event("01H001", vec![], json!({"type": "node.create"}));
        assert!(check_event_hashes(TEST_WORKSPACE, &[ev]).is_ok());
    }

    #[test]
    fn tampered_payload_detected() {
        let mut ev = make_event("01H001", vec![], json!({"type": "node.create"}));
        ev.payload = json!({"type": "node.update"}); // tamper
        let result = check_event_hashes(TEST_WORKSPACE, &[ev]);
        assert!(matches!(result, Err(TrustError::HashMismatch { .. })));
    }

    #[test]
    fn dangling_parent_detected() {
        let ev = make_event(
            "01H002",
            vec!["nonexistent_hash".to_string()],
            json!({"type": "node.create"}),
        );
        let result = check_parent_links(&[ev]);
        assert!(matches!(result, Err(TrustError::DanglingParent { .. })));
    }

    #[test]
    fn tips_computed_correctly() {
        let genesis = make_event("01H001", vec![], json!({"type": "node.create", "id": 1}));
        let child = make_event(
            "01H002",
            vec![genesis.event_hash.clone()],
            json!({"type": "node.create", "id": 2}),
        );
        let tips = compute_tips(&[genesis.clone(), child.clone()]);
        assert_eq!(tips, vec![child.event_hash]);
    }
}
