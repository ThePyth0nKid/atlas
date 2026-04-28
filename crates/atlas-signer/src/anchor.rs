//! Mock-Rekor issuer (V1.5).
//!
//! Builds a deterministic Merkle tree over a batch of anchor requests,
//! signs the resulting checkpoint with the dev mock-Rekor key, and emits
//! an `AnchorEntry` per request that the verifier can validate fully
//! offline.
//!
//! The "mock" qualifier means: no network call to a live Rekor instance.
//! Everything else is real — real Ed25519 signature, real RFC 6962-style
//! Merkle path, real domain-separation. The trust property is identical
//! to a production transparency log; the only thing missing is a live
//! third party publishing the log root publicly. V1.6 adds the
//! `--rekor-url` switch that swaps this issuer for a real Sigstore POST.
//!
//! Why issue locally at all in V1.5? Because the demo path must be
//! offline-reproducible (no flaky network in `pnpm smoke`), and because
//! shipping the verification path now means V1.6 can swap the issuer
//! without touching the verifier or the trace schema.
//!
//! The issuer's signing key is derived deterministically from the seed
//! `b"atlas-mock-rekor-v1-dev-seed-001"`. The verifier in
//! `atlas-trust-core::anchor::MOCK_LOG_PUBKEY_HEX` pins the matching
//! public key. The unit test `mock_log_pubkey_matches_signer_seed`
//! asserts the two stay in sync — touch one without touching the other
//! and CI fails.

use atlas_trust_core::anchor::{
    canonical_checkpoint_bytes, leaf_hash_for, MOCK_LOG_ID, MOCK_LOG_PUBKEY_HEX,
};
use atlas_trust_core::trace_format::{AnchorEntry, AnchorKind, InclusionProof};
use base64::Engine;
use blake3::Hasher;
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};

/// Deterministic dev seed for the mock-Rekor signing key. NEVER ship a
/// production key from a constant — V1.6 reads from a sealed key store.
const MOCK_LOG_SEED: [u8; 32] = *b"atlas-mock-rekor-v1-dev-seed-001";

/// One anchor request — one item the caller wants to anchor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorRequest {
    /// What the hash refers to.
    pub kind: AnchorKind,
    /// The hex hash to anchor (event_hash or pubkey_bundle_hash).
    pub anchored_hash: String,
}

/// Wrapper for stdin input to the `anchor` subcommand.
#[derive(Debug, Deserialize)]
pub struct AnchorBatchInput {
    /// Items to anchor in this batch.
    pub items: Vec<AnchorRequest>,
    /// Unix seconds the issuer should record as the integrated time.
    /// Caller-supplied (rather than `SystemTime::now`) so smoke tests
    /// produce byte-identical output across runs.
    pub integrated_time: i64,
}

/// Issue anchors for a batch of requests. Produces one `AnchorEntry` per
/// request, all sharing the same checkpoint over a single Merkle tree.
///
/// Tree shape: leaves are listed in the order given by the caller. A
/// caller anchoring [bundle, tip0, tip1] gets log_index 0,1,2 in that
/// order; sorting is the caller's responsibility (the MCP tool sorts
/// for stability).
pub fn issue_anchors(batch: AnchorBatchInput) -> Result<Vec<AnchorEntry>, String> {
    if batch.items.is_empty() {
        return Ok(Vec::new());
    }

    let signing_key = SigningKey::from_bytes(&MOCK_LOG_SEED);
    // Defence-in-depth: assert the seed-derived pubkey matches what
    // the verifier pins. If a future refactor changes either side this
    // crashes the issuer at run time rather than producing entries that
    // appear valid at issue time but fail at verify time.
    let actual_pk = hex::encode(signing_key.verifying_key().to_bytes());
    if actual_pk != MOCK_LOG_PUBKEY_HEX {
        return Err(format!(
            "issuer/verifier key drift: signer derives {} but verifier pins {}",
            actual_pk, MOCK_LOG_PUBKEY_HEX,
        ));
    }

    let leaves: Vec<[u8; 32]> = batch
        .items
        .iter()
        .map(|req| leaf_hash_for(&req.kind, &req.anchored_hash))
        .collect::<Result<_, _>>()?;

    let tree_size = leaves.len() as u64;
    let root = merkle_root(&leaves);
    let root_hex = hex::encode(root);

    let checkpoint_bytes = canonical_checkpoint_bytes(tree_size, &root_hex);
    let sig = signing_key.sign(&checkpoint_bytes);
    let sig_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes());

    let mut out = Vec::with_capacity(batch.items.len());
    for (i, req) in batch.items.iter().enumerate() {
        let proof_hashes = audit_path(&leaves, i);
        let entry = AnchorEntry {
            kind: req.kind.clone(),
            anchored_hash: req.anchored_hash.clone(),
            log_id: MOCK_LOG_ID.clone(),
            log_index: i as u64,
            integrated_time: batch.integrated_time,
            inclusion_proof: InclusionProof {
                tree_size,
                root_hash: root_hex.clone(),
                hashes: proof_hashes,
                checkpoint_sig: sig_b64.clone(),
            },
        };
        out.push(entry);
    }
    Ok(out)
}

/// Compute the RFC 6962-style Merkle root over `leaves`, using blake3 with
/// the `"node:"` domain-separation prefix the verifier expects. Leaves are
/// already domain-separated by `leaf_hash_for`.
fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    debug_assert!(!leaves.is_empty());
    if leaves.len() == 1 {
        return leaves[0];
    }
    let split = largest_power_of_two_le(leaves.len() as u64) as usize;
    let split = if split == leaves.len() { split / 2 } else { split };
    let left = merkle_root(&leaves[..split]);
    let right = merkle_root(&leaves[split..]);
    parent_hash(&left, &right)
}

fn parent_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(b"node:");
    h.update(left);
    h.update(right);
    *h.finalize().as_bytes()
}

/// Build the audit path for `index` in `leaves`, deepest sibling first
/// (RFC 6962 §2.1.1 ordering). Returns hex-encoded sibling hashes.
fn audit_path(leaves: &[[u8; 32]], index: usize) -> Vec<String> {
    let mut path = Vec::new();
    audit_path_recursive(leaves, index, &mut path);
    path.into_iter().map(hex::encode).collect()
}

fn audit_path_recursive(leaves: &[[u8; 32]], index: usize, out: &mut Vec<[u8; 32]>) {
    if leaves.len() <= 1 {
        return;
    }
    let split = largest_power_of_two_le(leaves.len() as u64) as usize;
    let split = if split == leaves.len() { split / 2 } else { split };
    if index < split {
        audit_path_recursive(&leaves[..split], index, out);
        out.push(merkle_root(&leaves[split..]));
    } else {
        audit_path_recursive(&leaves[split..], index - split, out);
        out.push(merkle_root(&leaves[..split]));
    }
}

/// Largest power of 2 less than or equal to n, for n >= 1.
fn largest_power_of_two_le(n: u64) -> u64 {
    debug_assert!(n >= 1);
    let mut p = 1u64;
    while p * 2 <= n {
        p *= 2;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_trust_core::anchor::{default_trusted_logs, verify_anchor_entry};

    #[test]
    fn mock_log_pubkey_matches_signer_seed() {
        let sk = SigningKey::from_bytes(&MOCK_LOG_SEED);
        let pk_hex = hex::encode(sk.verifying_key().to_bytes());
        assert_eq!(
            pk_hex, MOCK_LOG_PUBKEY_HEX,
            "issuer seed and verifier-pinned pubkey have drifted: \
             update either MOCK_LOG_SEED here or MOCK_LOG_PUBKEY_HEX in atlas-trust-core",
        );
    }

    #[test]
    fn round_trip_single_leaf() {
        let req = AnchorRequest {
            kind: AnchorKind::DagTip,
            anchored_hash:
                "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
                    .to_string(),
        };
        let entries = issue_anchors(AnchorBatchInput {
            items: vec![req.clone()],
            integrated_time: 1_700_000_000,
        })
        .unwrap();
        assert_eq!(entries.len(), 1);
        let trusted = default_trusted_logs();
        let outcome = verify_anchor_entry(&entries[0], &req.anchored_hash, &trusted);
        assert!(
            outcome.ok,
            "single-leaf anchor must verify, got reason: {}",
            outcome.reason
        );
    }

    #[test]
    fn round_trip_seven_leaves_every_index() {
        // Tree of 7 — exercises the lonely-rightmost branch in audit_path.
        let items: Vec<AnchorRequest> = (0..7)
            .map(|i| AnchorRequest {
                kind: AnchorKind::DagTip,
                anchored_hash: hex::encode([i as u8; 32]),
            })
            .collect();
        let entries = issue_anchors(AnchorBatchInput {
            items: items.clone(),
            integrated_time: 1_700_000_000,
        })
        .unwrap();
        let trusted = default_trusted_logs();
        for (i, entry) in entries.iter().enumerate() {
            let outcome = verify_anchor_entry(entry, &items[i].anchored_hash, &trusted);
            assert!(
                outcome.ok,
                "leaf {i} of 7 must verify, got reason: {}",
                outcome.reason
            );
        }
    }

    #[test]
    fn mixed_kinds_round_trip() {
        let items = vec![
            AnchorRequest {
                kind: AnchorKind::BundleHash,
                anchored_hash: hex::encode([0x11u8; 32]),
            },
            AnchorRequest {
                kind: AnchorKind::DagTip,
                anchored_hash: hex::encode([0x22u8; 32]),
            },
            AnchorRequest {
                kind: AnchorKind::DagTip,
                anchored_hash: hex::encode([0x33u8; 32]),
            },
        ];
        let entries = issue_anchors(AnchorBatchInput {
            items: items.clone(),
            integrated_time: 1_700_000_000,
        })
        .unwrap();
        let trusted = default_trusted_logs();
        for (i, entry) in entries.iter().enumerate() {
            let outcome = verify_anchor_entry(entry, &items[i].anchored_hash, &trusted);
            assert!(outcome.ok, "mixed-kind leaf {i} must verify: {}", outcome.reason);
        }
    }

    #[test]
    fn tampered_anchored_hash_fails() {
        let req = AnchorRequest {
            kind: AnchorKind::DagTip,
            anchored_hash: hex::encode([0x42u8; 32]),
        };
        let mut entries = issue_anchors(AnchorBatchInput {
            items: vec![req.clone()],
            integrated_time: 1_700_000_000,
        })
        .unwrap();
        // Adversary changes the anchored hash post-issue but keeps the proof.
        entries[0].anchored_hash = hex::encode([0x43u8; 32]);
        let trusted = default_trusted_logs();
        let outcome = verify_anchor_entry(&entries[0], &entries[0].anchored_hash, &trusted);
        assert!(!outcome.ok, "tampered anchored_hash must fail");
    }
}
