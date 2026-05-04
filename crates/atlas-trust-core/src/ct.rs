//! Constant-time comparison helpers.
//!
//! Hash equality checks compare 32-byte blake3 outputs (or their 64-char hex
//! encodings). Standard `==` on `&str` short-circuits on the first differing
//! byte, which leaks prefix-match length over enough samples. For an offline
//! verifier this is mostly theoretical, but the property "byte-identical
//! verification regardless of input shape" is exactly what Atlas claims, and
//! the cost of `subtle::ConstantTimeEq` is nil. Pay it.
//!
//! Length differences are NOT side-channel-protected — a hash of the wrong
//! length is trivially detectable from the input itself.
//!
//! # Protected boundaries (V1.15 Welle A invariant)
//!
//! Every wire-side equality below MUST route through this module's helpers.
//! The invariant is enforced by `tests/const_time_kid_invariant.rs` (source-
//! level audit) and the per-module byte-pinning tests
//! (`bundle_hash_byte_determinism_pin`, `signing_input_byte_determinism_pin`,
//! etc., which catch any hash-shape drift that would bypass the const-time
//! compare):
//!
//! 1. **`pubkey_bundle_hash`** — `verify::verify_trace_with` compares the
//!    trace-claimed bundle hash against the locally-recomputed hash via
//!    [`ct_eq_str`]. (V1.5 invariant.)
//! 2. **`event_hash`** — `hashchain::verify_chain` recomputes per-event
//!    hashes and compares via [`ct_eq_str`]. (V1.5 invariant.)
//! 3. **`anchored_hash`** — `anchor::verify_anchor_proof` and friends
//!    compare anchored-hash claims via [`ct_eq_str`]. (V1.7 invariant.)
//! 4. **chain `head` / `previous_head`** — anchor-chain walking compares
//!    every `previous_head ↔ chain_head_for(prev_batch)` link via
//!    [`ct_eq_str`]. (V1.7 invariant.)
//! 5. **per-tenant `kid`** — `verify::verify_trace_with` strict-mode
//!    per-tenant-keys check compares `event.signature.kid` against the
//!    deterministic `per_tenant_kid_for(workspace_id)` via [`ct_eq_str`].
//!    (V1.15 Welle A invariant — closed the last `==` on a wire-side KID.)
//! 6. **witness `kid`** — `witness::verify_witness_against_roster_categorized`
//!    walks the pinned roster and matches `witness.witness_kid` against each
//!    entry's kid via [`ct_eq_str`]. (V1.13 wave-C-2 invariant.)
//!
//! Boundaries NOT covered (and why):
//!
//! - `BTreeMap` / `BTreeSet` lookups (e.g. `verified_kids.contains(&kid)`,
//!   `kid_counts.entry(kid)`). These are scope-local accumulators built from
//!   the trace itself within a single verification call; their contents are
//!   either (a) the witness kids of the current batch (cross-batch dedup) or
//!   (b) the witness kids of already-verified earlier batches. A timing
//!   leak from these structures could only surface kids already present in
//!   the same trace — no new attacker information.
//! - Integer / `usize` / `u64` field equality (e.g. batch indices, threshold
//!   counts). Const-time compare is structurally not applicable; these
//!   carry no secret-byte content.
//! - String comparisons in operator-facing diagnostic Display/Debug paths
//!   (e.g. `WitnessFailure::Display`). These do not gate trust decisions
//!   and a leak from them is a leak of error-message content, which is
//!   already visible to the operator running the verifier.

use subtle::ConstantTimeEq;

/// Constant-time equality of two byte slices (after a length-equal check).
pub fn ct_eq_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

/// Constant-time equality of two strings (interpreted as bytes).
pub fn ct_eq_str(a: &str, b: &str) -> bool {
    ct_eq_bytes(a.as_bytes(), b.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_strings_match() {
        assert!(ct_eq_str("abc", "abc"));
        assert!(ct_eq_str("", ""));
    }

    #[test]
    fn differing_strings_dont_match() {
        assert!(!ct_eq_str("abc", "abd"));
        assert!(!ct_eq_str("abc", "ab"));
        assert!(!ct_eq_str("ab", "abc"));
    }
}
