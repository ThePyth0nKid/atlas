//! Anchor verification (V1.5).
//!
//! Given an `AnchorEntry` and a pinned set of trusted log pubkeys, this
//! module answers: was the claimed `anchored_hash` actually committed to
//! a known transparency log at the claimed position, under a checkpoint
//! the log signed?
//!
//! Three independent links are checked:
//!   1. The log key identified by `entry.log_id` is one we trust (presence
//!      in the pinned roster).
//!   2. The Merkle inclusion path from leaf -> root_hash is consistent
//!      (RFC 6962 §2.1.1 verify-inclusion).
//!   3. The signed checkpoint binds (`tree_size`, `root_hash`) to the log
//!      under that pubkey (Ed25519 over canonical checkpoint bytes).
//!
//! All three must hold. The path is offline-only: no network access,
//! no allocator beyond what serde and the proof require.
//!
//! V1.5 ships one trusted log key — the dev `mock-rekor` key used by
//! `atlas-signer anchor`. V1.6 adds the public Sigstore Rekor key, and
//! `--rekor-url` in the signer becomes a real network call.

use base64::Engine;
use blake3::Hasher;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::trace_format::{AnchorEntry, AnchorKind, AtlasTrace, InclusionProof};

/// Hex Ed25519 public key for the dev mock-Rekor log. Pinned at compile
/// time so every build of the verifier ships the same trusted root, and
/// so a tampered binary can be spotted by hash-comparing this file.
///
/// Derived from the dev seed in `atlas-signer/src/anchor.rs`; the test
/// `mock_log_pubkey_matches_signer_seed` in atlas-signer asserts they
/// stay in sync. Touching one without touching the other fails CI.
pub const MOCK_LOG_PUBKEY_HEX: &str =
    "ac1ea29e6641da38baf768088517ba33c0170614b1cc97d718921d3e2db2bfcb";

/// Hex SHA-256 of `MOCK_LOG_PUBKEY` raw bytes — the log's public identity.
/// Verifier rejects any anchor whose `log_id` is not in the trusted set.
pub static MOCK_LOG_ID: LazyLock<String> = LazyLock::new(|| {
    let pk_raw = mock_log_pubkey_raw();
    let mut h = Hasher::new();
    h.update(&pk_raw);
    hex::encode(h.finalize().as_bytes())
});

fn mock_log_pubkey_raw() -> [u8; 32] {
    let raw = hex::decode(MOCK_LOG_PUBKEY_HEX)
        .expect("MOCK_LOG_PUBKEY_HEX is hex");
    let mut out = [0u8; 32];
    out.copy_from_slice(&raw);
    out
}

/// Outcome of anchor verification for a single entry.
#[derive(Debug, Clone)]
pub struct AnchorOutcome {
    /// Did this entry verify completely?
    pub ok: bool,
    /// What kind it was (re-emitted for evidence formatting).
    pub kind: AnchorKind,
    /// Hash this entry vouched for.
    pub anchored_hash: String,
    /// Log it was anchored to.
    pub log_id: String,
    /// On failure, the specific reason. Empty on success.
    pub reason: String,
}

/// Verify a single anchor entry against a roster of trusted log pubkeys.
///
/// `expected_hash` is the hash the trace itself claims for this anchor's
/// kind — i.e. one of `trace.dag_tips` (for DagTip) or
/// `trace.pubkey_bundle_hash` (for BundleHash). The verifier insists
/// `entry.anchored_hash == expected_hash` so a Rekor entry for an
/// unrelated tip cannot be smuggled in to look like it covers the trace.
pub fn verify_anchor_entry(
    entry: &AnchorEntry,
    expected_hash: &str,
    trusted_logs: &BTreeMap<String, VerifyingKey>,
) -> AnchorOutcome {
    let mk = |reason: String| AnchorOutcome {
        ok: false,
        kind: entry.kind.clone(),
        anchored_hash: entry.anchored_hash.clone(),
        log_id: entry.log_id.clone(),
        reason,
    };

    if !crate::ct::ct_eq_str(&entry.anchored_hash, expected_hash) {
        return mk(format!(
            "anchored_hash {} does not match trace's claimed hash {}",
            &entry.anchored_hash, expected_hash,
        ));
    }

    let Some(log_key) = trusted_logs.get(&entry.log_id) else {
        return mk(format!(
            "log_id {} is not in the verifier's trusted log roster",
            entry.log_id,
        ));
    };

    let leaf_hash = match leaf_hash_for(&entry.kind, &entry.anchored_hash) {
        Ok(h) => h,
        Err(e) => return mk(e),
    };

    if let Err(e) = verify_inclusion(
        &leaf_hash,
        entry.log_index,
        &entry.inclusion_proof,
    ) {
        return mk(format!("inclusion proof failed: {e}"));
    }

    if let Err(e) = verify_checkpoint_sig(log_key, &entry.inclusion_proof) {
        return mk(format!("checkpoint signature failed: {e}"));
    }

    AnchorOutcome {
        ok: true,
        kind: entry.kind.clone(),
        anchored_hash: entry.anchored_hash.clone(),
        log_id: entry.log_id.clone(),
        reason: String::new(),
    }
}

/// Verify every anchor in a trace and emit one outcome per entry plus a
/// roll-up `all_ok`. Lenient mode (caller's choice) accepts an empty
/// `entries` vector; strict mode enforces coverage at the call site.
pub fn verify_anchors(
    trace: &AtlasTrace,
    trusted_logs: &BTreeMap<String, VerifyingKey>,
) -> Vec<AnchorOutcome> {
    let mut out = Vec::with_capacity(trace.anchors.len());
    let dag_tip_set: std::collections::BTreeSet<&str> =
        trace.dag_tips.iter().map(String::as_str).collect();
    for entry in &trace.anchors {
        let expected = match entry.kind {
            AnchorKind::DagTip => {
                if !dag_tip_set.contains(entry.anchored_hash.as_str()) {
                    out.push(AnchorOutcome {
                        ok: false,
                        kind: entry.kind.clone(),
                        anchored_hash: entry.anchored_hash.clone(),
                        log_id: entry.log_id.clone(),
                        reason: format!(
                            "anchored dag_tip {} not present in trace.dag_tips",
                            &entry.anchored_hash,
                        ),
                    });
                    continue;
                }
                entry.anchored_hash.as_str()
            }
            AnchorKind::BundleHash => trace.pubkey_bundle_hash.as_str(),
        };
        out.push(verify_anchor_entry(entry, expected, trusted_logs));
    }
    out
}

/// Build the canonical bytes that a log signs for a checkpoint.
///
/// Format (single source of truth — verifier and issuer share this):
/// ```text
/// atlas-mock-rekor-v1\n
/// {tree_size}\n
/// {root_hash_hex}\n
/// ```
/// Three lines, each terminated by `\n`. Versioned at line 1 so a future
/// log format can be told apart cleanly.
pub fn canonical_checkpoint_bytes(tree_size: u64, root_hash_hex: &str) -> Vec<u8> {
    format!("atlas-mock-rekor-v1\n{tree_size}\n{root_hash_hex}\n").into_bytes()
}

/// Build the canonical leaf hash for an anchor entry. The leaf hash is
/// the input to the Merkle inclusion verification.
///
/// Domain separation: leaves are blake3(`"leaf:"` || kind || `":"` || hash_bytes).
/// The `"leaf:"` prefix prevents second-preimage attacks against internal
/// nodes (RFC 6962 uses the same idea with 0x00 / 0x01).
pub fn leaf_hash_for(kind: &AnchorKind, anchored_hash_hex: &str) -> Result<[u8; 32], String> {
    let raw = hex::decode(anchored_hash_hex)
        .map_err(|e| format!("anchored_hash is not hex: {e}"))?;
    let kind_label = match kind {
        AnchorKind::DagTip => "dag_tip",
        AnchorKind::BundleHash => "bundle_hash",
    };
    let mut h = Hasher::new();
    h.update(b"leaf:");
    h.update(kind_label.as_bytes());
    h.update(b":");
    h.update(&raw);
    Ok(*h.finalize().as_bytes())
}

/// Hash two children into a parent node. Domain-separated from leaves
/// via the `"node:"` prefix.
fn parent_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(b"node:");
    h.update(left);
    h.update(right);
    *h.finalize().as_bytes()
}

/// RFC 6962 §2.1.1 inclusion-proof verification, adapted to blake3.
///
/// `leaf_hash` is the hashed leaf (domain-separated, see `leaf_hash_for`).
/// `index` is the 0-indexed leaf position. `proof.hashes` are sibling
/// hashes from deepest to shallowest. We walk up the tree, combining
/// the running hash with each sibling on the correct side based on the
/// current index bit, and assert the final value equals `proof.root_hash`.
fn verify_inclusion(
    leaf_hash: &[u8; 32],
    index: u64,
    proof: &InclusionProof,
) -> Result<(), String> {
    if index >= proof.tree_size {
        return Err(format!(
            "log_index {index} out of range for tree_size {}",
            proof.tree_size,
        ));
    }
    let expected_path_len = audit_path_length(index, proof.tree_size);
    if proof.hashes.len() != expected_path_len {
        return Err(format!(
            "inclusion path length {} does not match expected {} for index {} in tree of size {}",
            proof.hashes.len(),
            expected_path_len,
            index,
            proof.tree_size,
        ));
    }

    let mut running = *leaf_hash;
    let mut idx = index;
    let mut last = proof.tree_size - 1;

    for sibling_hex in &proof.hashes {
        let raw = hex::decode(sibling_hex)
            .map_err(|e| format!("sibling hash not hex: {e}"))?;
        if raw.len() != 32 {
            return Err(format!(
                "sibling hash wrong length: got {}, want 32",
                raw.len(),
            ));
        }
        let mut sibling = [0u8; 32];
        sibling.copy_from_slice(&raw);

        if idx & 1 == 1 || idx == last {
            // Right child OR the lonely-rightmost case at this level:
            // sibling is on the left.
            running = parent_hash(&sibling, &running);
        } else {
            running = parent_hash(&running, &sibling);
        }
        idx >>= 1;
        last >>= 1;
    }

    let actual_root = hex::encode(running);
    if !crate::ct::ct_eq_str(&actual_root, &proof.root_hash) {
        return Err(format!(
            "computed root {} does not match claimed {}",
            actual_root, proof.root_hash,
        ));
    }
    Ok(())
}

/// Length of the inclusion audit path for `index` in a tree of `tree_size`.
///
/// RFC 6962 path length: number of bits needed to walk index up to root,
/// adjusted for the lonely-right-edge case where last node at a level has
/// no sibling and is just promoted.
fn audit_path_length(index: u64, tree_size: u64) -> usize {
    let mut last = tree_size - 1;
    let mut idx = index;
    let mut len = 0;
    while last > 0 {
        if idx & 1 == 1 || idx < last {
            len += 1;
        }
        idx >>= 1;
        last >>= 1;
    }
    len
}

fn verify_checkpoint_sig(log_key: &VerifyingKey, proof: &InclusionProof) -> Result<(), String> {
    let bytes = canonical_checkpoint_bytes(proof.tree_size, &proof.root_hash);
    let sig_raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(proof.checkpoint_sig.as_bytes())
        .or_else(|_| {
            base64::engine::general_purpose::STANDARD
                .decode(proof.checkpoint_sig.as_bytes())
        })
        .map_err(|e| format!("checkpoint_sig is not base64: {e}"))?;
    if sig_raw.len() != 64 {
        return Err(format!(
            "checkpoint_sig wrong length: got {}, want 64",
            sig_raw.len(),
        ));
    }
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&sig_raw);
    let sig = Signature::from_bytes(&sig_bytes);
    log_key
        .verify(&bytes, &sig)
        .map_err(|e| format!("ed25519 verify: {e}"))
}

/// Default trusted log roster shipped in this verifier build.
pub fn default_trusted_logs() -> BTreeMap<String, VerifyingKey> {
    let mut m = BTreeMap::new();
    let key = VerifyingKey::from_bytes(&mock_log_pubkey_raw())
        .expect("MOCK_LOG_PUBKEY_HEX must be a valid Ed25519 point");
    m.insert(MOCK_LOG_ID.clone(), key);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_path_length_matches_rfc6962_examples() {
        // Tree of 7 leaves: indices 0..3 sit in the full left subtree
        // (depth 3 path); indices 4..5 sit in the partial right subtree
        // (depth 3 path: sibling at L0, then h(6) at L1, then left
        // subtree root at L2); index 6 is lonely rightmost so its L0
        // step adds nothing — path is 2.
        assert_eq!(audit_path_length(0, 7), 3);
        assert_eq!(audit_path_length(1, 7), 3);
        assert_eq!(audit_path_length(2, 7), 3);
        assert_eq!(audit_path_length(3, 7), 3);
        assert_eq!(audit_path_length(4, 7), 3);
        assert_eq!(audit_path_length(5, 7), 3);
        assert_eq!(audit_path_length(6, 7), 2);

        // Tree of 1 leaf — no audit path (only the leaf, root = leaf).
        assert_eq!(audit_path_length(0, 1), 0);

        // Power-of-two tree of size 8 — every leaf has a 3-step path.
        for i in 0..8 {
            assert_eq!(audit_path_length(i, 8), 3, "i={i}");
        }
    }

    #[test]
    fn leaf_hash_is_domain_separated() {
        let h_tip = leaf_hash_for(
            &AnchorKind::DagTip,
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        )
        .unwrap();
        let h_bundle = leaf_hash_for(
            &AnchorKind::BundleHash,
            "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        )
        .unwrap();
        assert_ne!(
            h_tip, h_bundle,
            "leaf hashes must differ across kinds for the same anchored_hash",
        );
    }

    #[test]
    fn checkpoint_canonical_bytes_are_stable() {
        let bytes = canonical_checkpoint_bytes(7, "deadbeef");
        assert_eq!(bytes, b"atlas-mock-rekor-v1\n7\ndeadbeef\n");
    }
}
