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

/// V1.19 Welle 9: verify the trace forms a strict linear chain.
///
/// Atlas is fundamentally a DAG — `check_parent_links` only requires
/// that every referenced parent exists somewhere in the trace, and the
/// default lenient mode accepts forks (multiple events referencing the
/// same parent), multiple genesis events, and multiple tips. This is
/// the right default because per-workspace mutex serialisation lives
/// in a single Node process, and the route.rs threat model explicitly
/// flags the multi-process case as forking-permitted ("deploy one
/// writer per workspace until V2 ships an external lock").
///
/// `check_strict_chain` is the opt-in flag for operators who deploy
/// exactly one writer per workspace and want to detect operational
/// misconfiguration where a second writer accidentally appears (which
/// would manifest as a sibling-fork DAG that's still DAG-valid). Pins
/// four properties:
///
///   1. Trace is non-empty (an empty trace cannot be a "linear chain";
///      claiming otherwise lets an attacker who strips events from a
///      bundle pass strict mode silently — see SECURITY review H-1).
///   2. Exactly one genesis event (`parent_hashes.is_empty()`).
///   3. Every non-genesis event has exactly one parent.
///   4. No event is referenced as a parent by more than one other
///      event (no sibling-fork — two events claiming the same parent).
///   5. No event lists its own `event_hash` as a parent (no direct
///      self-reference cycle). Indirect cycles are infeasible under
///      a successful upstream `check_event_hashes` (blake3 preimage
///      resistance), but defence-in-depth pins the trivial case here
///      so the function is sound when called standalone — see SECURITY
///      review H-2.
///
/// Combined with `check_parent_links` (which guarantees every
/// referenced parent exists in the trace), these properties imply a
/// strict linear chain: the events form a single tree rooted at the
/// genesis, each node has in-degree ≤ 1, no node points to itself, so
/// the tree is a finite path. Equivalently: exactly one tip, and the
/// trace is equivalent to its sole linearisation.
///
/// This mirrors the `parents[0] === stored[i-1].event_hash` oracle in
/// `apps/atlas-web/scripts/e2e-write-edge-cases.ts` (V1.19 Welle 8 [C.6]),
/// but lifted into the verifier so auditors can enforce the property
/// across the full trust pipeline rather than only at the write site.
///
/// Diagnostics: parent-hash strings in the error message are truncated
/// to 16 hex chars to keep auditor-facing output bounded and to limit
/// the bytes attacker-controlled wire data can inject into a downstream
/// log/UI consumer of `outcome.errors`.
pub fn check_strict_chain(events: &[AtlasEvent]) -> TrustResult<()> {
    if events.is_empty() {
        return Err(TrustError::StrictChainViolation {
            msg: "trace has no events (a linear chain requires at least 1 genesis event)".to_string(),
        });
    }

    // Direct self-reference: an event listing its own event_hash as a
    // parent. Cryptographically infeasible after a successful
    // check_event_hashes pass (blake3 preimage), but defence-in-depth
    // for callers who invoke check_strict_chain standalone — and an
    // honest invariant for the public API that the pure structural
    // properties below DON'T cover. Checked FIRST so a 1-event self-
    // reference (parent_hashes=[its_own_hash]) reports the cycle
    // diagnostic, not the misleading "found 0 genesis events".
    for ev in events {
        if ev.parent_hashes.iter().any(|p| p == &ev.event_hash) {
            return Err(TrustError::StrictChainViolation {
                msg: format!(
                    "event {} lists its own hash as a parent (self-reference cycle)",
                    ev.event_id,
                ),
            });
        }
    }

    let mut genesis_count: usize = 0;
    let mut multi_parent: Vec<&str> = Vec::new();
    for ev in events {
        match ev.parent_hashes.len() {
            0 => genesis_count += 1,
            1 => {}
            _ => multi_parent.push(ev.event_id.as_str()),
        }
    }

    if genesis_count != 1 {
        return Err(TrustError::StrictChainViolation {
            msg: format!(
                "expected exactly 1 genesis event (parent_hashes empty), found {}",
                genesis_count,
            ),
        });
    }
    if !multi_parent.is_empty() {
        return Err(TrustError::StrictChainViolation {
            msg: format!(
                "{} event(s) have more than one parent (DAG-merge not allowed in strict mode): {:?}",
                multi_parent.len(),
                multi_parent,
            ),
        });
    }

    // Sibling-fork detection: count how often each event is referenced
    // as a parent. Any reference count > 1 means two events claim the
    // same parent — the regression mode that mutex serialisation in a
    // single writer process MUST never produce, but a second concurrent
    // writer would.
    let mut ref_count: HashMap<&str, usize> = HashMap::with_capacity(events.len());
    for ev in events {
        for parent in &ev.parent_hashes {
            *ref_count.entry(parent.as_str()).or_insert(0) += 1;
        }
    }
    let mut forks: Vec<&str> = ref_count
        .iter()
        .filter_map(|(h, &n)| if n > 1 { Some(*h) } else { None })
        .collect();
    if !forks.is_empty() {
        // Sort + truncate parent-hash diagnostics to 16 hex chars each
        // (keeps the message bounded; the full hash is recoverable from
        // the trace itself, so 16 chars is enough to disambiguate).
        forks.sort_unstable();
        let preview: Vec<String> = forks
            .iter()
            .map(|h| h.chars().take(16).collect::<String>())
            .collect();
        return Err(TrustError::StrictChainViolation {
            msg: format!(
                "{} parent hash(es) referenced by more than one event (sibling-fork DAG): {:?}",
                forks.len(),
                preview,
            ),
        });
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

    // ---- V1.19 Welle 9: check_strict_chain ----

    fn strict_chain_err_msg(result: TrustResult<()>) -> String {
        match result {
            Err(TrustError::StrictChainViolation { msg }) => msg,
            other => panic!("expected StrictChainViolation, got: {other:?}"),
        }
    }

    #[test]
    fn strict_chain_empty_trace_fails() {
        // Welle 9 review-fix (SR-H-1): an empty trace cannot be a
        // "linear chain". Returning Ok would let an attacker who strips
        // events from a bundle pass `--require-strict-chain` silently.
        let result = check_strict_chain(&[]);
        assert!(matches!(result, Err(TrustError::StrictChainViolation { .. })));
        let msg = strict_chain_err_msg(result);
        assert!(msg.contains("no events"), "got: {msg}");
    }

    #[test]
    fn strict_chain_single_genesis_is_ok() {
        let g = make_event("01H001", vec![], json!({"type": "node.create"}));
        assert!(check_strict_chain(&[g]).is_ok());
    }

    #[test]
    fn strict_chain_two_event_linear_is_ok() {
        // Minimal interesting linear chain: genesis + one child.
        let g = make_event("01H001", vec![], json!({"id": 1}));
        let m = make_event("01H002", vec![g.event_hash.clone()], json!({"id": 2}));
        assert!(check_strict_chain(&[g, m]).is_ok());
    }

    #[test]
    fn strict_chain_linear_three_events_is_ok() {
        let g = make_event("01H001", vec![], json!({"id": 1}));
        let m = make_event("01H002", vec![g.event_hash.clone()], json!({"id": 2}));
        let t = make_event("01H003", vec![m.event_hash.clone()], json!({"id": 3}));
        assert!(check_strict_chain(&[g, m, t]).is_ok());
    }

    #[test]
    fn strict_chain_two_genesis_fails() {
        // Two events both with empty parent_hashes — operator
        // misconfiguration where a second writer produced its own
        // genesis instead of chaining to the existing one.
        let g1 = make_event("01H001", vec![], json!({"id": 1}));
        let g2 = make_event("01H002", vec![], json!({"id": 2}));
        let result = check_strict_chain(&[g1, g2]);
        let msg = strict_chain_err_msg(result);
        assert!(msg.contains("expected exactly 1 genesis"), "got: {msg}");
        assert!(msg.contains("found 2"), "got: {msg}");
    }

    #[test]
    fn strict_chain_zero_genesis_fails() {
        // Pathological wire: every event references a parent that's
        // also in the trace, but no event has empty parent_hashes.
        // In the verifier pipeline `check_parent_links` runs BEFORE
        // strict-chain and would catch the "fake-hash" dangling parent
        // first — so this exact shape never reaches strict-chain in
        // production. The test exercises check_strict_chain in
        // isolation to pin its own zero-genesis diagnostic, since the
        // function is also a `pub` standalone API and must produce a
        // sensible error without relying on its callers.
        let a = make_event("01H001", vec!["fake-hash".to_string()], json!({"id": 1}));
        let b = make_event("01H002", vec![a.event_hash.clone()], json!({"id": 2}));
        let result = check_strict_chain(&[a, b]);
        assert!(strict_chain_err_msg(result).contains("found 0"));
    }

    #[test]
    fn strict_chain_sibling_fork_fails() {
        // Two events both reference the same parent — the canonical
        // sibling-fork DAG. Lenient verification accepts this (it's a
        // valid DAG); strict mode MUST reject.
        let g = make_event("01H001", vec![], json!({"id": 1}));
        let s1 = make_event("01H002", vec![g.event_hash.clone()], json!({"id": 2}));
        let s2 = make_event("01H003", vec![g.event_hash.clone()], json!({"id": 3}));
        let result = check_strict_chain(&[g, s1, s2]);
        let msg = strict_chain_err_msg(result);
        assert!(msg.contains("sibling-fork"), "got: {msg}");
    }

    #[test]
    fn strict_chain_dag_merge_fails() {
        // Event with two parents — a DAG merge. Lenient verification
        // accepts; strict mode requires single-parent shape.
        // Implementation note: the multi-parent loop runs BEFORE the
        // sibling-fork ref-count, so in this fixture (where `merge`
        // has 2 parents AND `g` is referenced twice) the multi-parent
        // diagnostic fires first. Either error is correct strict-mode
        // behaviour and the assertion accepts both for resilience to
        // future check-ordering changes.
        let g = make_event("01H001", vec![], json!({"id": 1}));
        let a = make_event("01H002", vec![g.event_hash.clone()], json!({"id": 2}));
        let b = make_event("01H003", vec![g.event_hash.clone()], json!({"id": 3}));
        let merge = make_event(
            "01H004",
            vec![a.event_hash.clone(), b.event_hash.clone()],
            json!({"id": 4}),
        );
        let result = check_strict_chain(&[g, a, b, merge]);
        let msg = strict_chain_err_msg(result);
        assert!(
            msg.contains("more than one parent") || msg.contains("sibling-fork"),
            "got: {msg}",
        );
    }

    #[test]
    fn strict_chain_self_reference_fails() {
        // Welle 9 review-fix (SR-H-2): an event listing its own
        // event_hash as a parent is a 1-cycle, not a chain. Cryptographically
        // infeasible after a successful check_event_hashes (blake3
        // preimage), but the standalone strict-chain API must reject
        // it on its own to keep the function sound when called outside
        // the verify_trace_with pipeline.
        let mut ev = make_event("01H001", vec![], json!({"id": 1}));
        ev.parent_hashes = vec![ev.event_hash.clone()];
        let result = check_strict_chain(&[ev]);
        let msg = strict_chain_err_msg(result);
        assert!(msg.contains("self-reference"), "got: {msg}");
    }
}
