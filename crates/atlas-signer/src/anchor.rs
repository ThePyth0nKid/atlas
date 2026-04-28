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
    canonical_checkpoint_bytes, extract_signature_line_sigstore, leaf_hash_for,
    parse_sigstore_checkpoint_tree_id, sigstore_anchored_hash_for, sigstore_artifact_bytes_for,
    MOCK_LOG_ID, MOCK_LOG_PUBKEY_HEX, SIGSTORE_REKOR_V1_KEY_ID, SIGSTORE_REKOR_V1_LOG_ID,
    SIGSTORE_REKOR_V1_ORIGIN,
};
use atlas_trust_core::trace_format::{AnchorEntry, AnchorKind, InclusionProof};
use base64::Engine;
use blake3::Hasher;
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};

use crate::rekor_client::{
    HashedRekordData, HashedRekordHash, HashedRekordPublicKey, HashedRekordRequest,
    HashedRekordSignature, HashedRekordSpec, RekorClient,
};

/// Deterministic dev seed for the mock-Rekor signing key. NEVER ship a
/// production key from a constant — V1.6 reads from a sealed key store.
const MOCK_LOG_SEED: [u8; 32] = *b"atlas-mock-rekor-v1-dev-seed-001";

/// Deterministic dev seed for the V1.6 Atlas anchoring key (the key
/// embedded in `signature.publicKey.content` of every Sigstore-bound
/// hashedrekord). The verifier does NOT validate this key — Sigstore
/// Rekor verifies the signature at submit time and admits the entry
/// only if the signature checks out. From the verifier's perspective
/// this key is arbitrary; the trust property holds because the
/// pinned ECDSA P-256 log key signs the inclusion checkpoint that
/// covers the entry body.
///
/// SHA-256 of the seed gives a uniformly random 32-byte scalar that
/// is, with overwhelming probability, in `[1, n-1]` for the P-256
/// curve order. We pin the resulting public key in a unit test so
/// any future edit to the seed (or the derivation) trips a test
/// before the issuer ships.
///
/// V1.7 will replace this with a sealed key store and a rotation
/// story; the verifier remains unaffected because it does not pin
/// the Atlas anchoring key.
const ATLAS_ANCHOR_SEED: &[u8] = b"atlas-rekor-anchor-v1-dev-seed";

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
            // Atlas mock-rekor-v1 format: leaf hash is constructed from
            // (kind, anchored_hash) directly via leaf_hash_for, so no
            // entry_body is needed. tree_id is unused outside Sigstore.
            entry_body_b64: None,
            tree_id: None,
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

// ─────────────────────────────────────────────────────────────────────────
// V1.6 — Sigstore Rekor v1 issuer (live POST behind --rekor-url)
// ─────────────────────────────────────────────────────────────────────────

/// Derive the deterministic Atlas anchoring key from `ATLAS_ANCHOR_SEED`.
///
/// SHA-256(seed) gives 32 uniformly-random bytes; we feed those into
/// `p256::ecdsa::SigningKey::from_bytes` which validates that the
/// scalar is in `[1, n-1]` for the P-256 curve order `n`. For our
/// fixed dev seed the derivation succeeds unconditionally; a unit
/// test pins the resulting public-key DER so any drift trips CI.
fn atlas_anchor_signing_key() -> Result<p256::ecdsa::SigningKey, String> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(ATLAS_ANCHOR_SEED);
    let scalar_bytes: [u8; 32] = h.finalize().into();
    p256::ecdsa::SigningKey::from_bytes(&scalar_bytes.into())
        .map_err(|e| format!("atlas anchoring key derivation failed: {e}"))
}

/// PEM-encode the Atlas anchoring key's public component as
/// `SubjectPublicKeyInfo`. Rekor accepts standard PEM (`-----BEGIN
/// PUBLIC KEY-----`); the returned string carries the surrounding
/// markers + a trailing newline, ready to be base64-encoded for
/// `signature.publicKey.content`.
fn atlas_anchor_public_pem(signing_key: &p256::ecdsa::SigningKey) -> Result<String, String> {
    use p256::pkcs8::EncodePublicKey;
    let verifying_key = signing_key.verifying_key();
    verifying_key
        .to_public_key_pem(p256::pkcs8::LineEnding::LF)
        .map_err(|e| format!("could not PEM-encode Atlas anchoring pubkey: {e}"))
}

/// Issue anchors via a live Sigstore Rekor v1 instance.
///
/// For each request:
///   1. Compute the Sigstore-format `anchored_hash` via
///      `sigstore_anchored_hash_for(kind, blake3_hex)`.
///   2. Compute the artifact bytes via `sigstore_artifact_bytes_for`.
///      The hashedrekord schema commits the SHA-256 of these bytes to
///      `data.hash.value`; the verifier re-derives the same value and
///      asserts byte-for-byte equality.
///   3. Sign the artifact bytes with the Atlas anchoring key (ECDSA
///      P-256 over SHA-256 — matches Rekor's hashedrekord verifier).
///   4. POST the hashedrekord to `<rekor_url>/api/v1/log/entries`.
///   5. Map the response to `AnchorEntry`, extracting the C2SP
///      checkpoint signature line via the canonical helper.
///
/// The function fails loud on the first error — partial issuance
/// would leave the trace bundle in a state where some anchors live
/// in the log and others don't. Better to surface the error and let
/// the operator retry with a fresh batch.
pub fn issue_anchors_via_rekor(
    batch: AnchorBatchInput,
    rekor_url: &str,
) -> Result<Vec<AnchorEntry>, String> {
    if batch.items.is_empty() {
        return Ok(Vec::new());
    }

    let signing_key = atlas_anchor_signing_key()?;
    let public_pem = atlas_anchor_public_pem(&signing_key)?;
    let public_pem_b64 = base64::engine::general_purpose::STANDARD.encode(public_pem.as_bytes());

    let client = RekorClient::new(rekor_url)?;

    let mut entries = Vec::with_capacity(batch.items.len());
    for req in &batch.items {
        let entry = issue_one_via_rekor(req, &client, &signing_key, &public_pem_b64)?;
        entries.push(entry);
    }
    Ok(entries)
}

fn issue_one_via_rekor(
    req: &AnchorRequest,
    client: &RekorClient,
    signing_key: &p256::ecdsa::SigningKey,
    public_pem_b64: &str,
) -> Result<AnchorEntry, String> {
    use p256::ecdsa::signature::Signer as _;

    // Steps 1+2: derive the Sigstore-format anchored_hash and the
    // artifact bytes that hash to it. Both helpers live in
    // atlas-trust-core so the verifier-side derivation is
    // bit-identical by construction.
    let sigstore_hash = sigstore_anchored_hash_for(&req.kind, &req.anchored_hash);
    let artifact_bytes = sigstore_artifact_bytes_for(&req.kind, &req.anchored_hash);

    // Step 3: sign the artifact bytes. The chain on the wire is:
    //   issuer:  sig = ECDSA_sign(SHA-256(artifact_bytes))    [p256
    //            Signer::sign hashes with SHA-256 internally before
    //            ECDSA-signing — this is standard ECDSA-with-hash, not
    //            a separate prehash step]
    //   body:    data.hash.value = hex(SHA-256(artifact_bytes))
    //   Rekor:   ECDSA_verify(hex_decode(data.hash.value), sig)  via
    //            `LoadVerifierWithOpts(..., WithHash(SHA256))` — i.e.
    //            Rekor takes value AS the digest and ECDSA-verifies
    //            the signature over it. It does NOT re-hash the digest.
    // The digest seen by sign and verify is therefore the same SHA-256
    // bytes; the chain closes only because both sides treat
    // SHA-256(artifact_bytes) as the message digest.
    let sig: p256::ecdsa::Signature = signing_key.sign(&artifact_bytes);
    let sig_der = sig.to_der();
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig_der.as_bytes());

    let request = HashedRekordRequest {
        api_version: "0.0.1",
        kind: "hashedrekord",
        spec: HashedRekordSpec {
            data: HashedRekordData {
                hash: HashedRekordHash {
                    algorithm: "sha256",
                    value: sigstore_hash.clone(),
                },
            },
            signature: HashedRekordSignature {
                content: sig_b64,
                public_key: HashedRekordPublicKey {
                    content: public_pem_b64.to_string(),
                },
            },
        },
    };

    // Step 4: POST. Errors propagate verbatim with Rekor's response
    // body excerpt so the caller can debug submission rejections.
    let response = client.submit_hashedrekord(&request)?;

    // Defensive: Rekor must echo the same log_id we pin. If a future
    // mis-routing returns an entry from a different shard, fail at
    // issue time rather than ship a trace the verifier will reject.
    if response.log_id != *SIGSTORE_REKOR_V1_LOG_ID {
        return Err(format!(
            "rekor returned entry with log_id {} but Atlas pins {}",
            response.log_id, *SIGSTORE_REKOR_V1_LOG_ID,
        ));
    }

    // Defensive: Rekor's response root_hash must hex-decode to 32 bytes.
    // Catches truncated / malformed values before they reach the verifier.
    let root_raw = hex::decode(&response.verification.inclusion_proof.root_hash)
        .map_err(|e| format!("rekor returned non-hex root_hash: {e}"))?;
    if root_raw.len() != 32 {
        return Err(format!(
            "rekor root_hash is {} bytes (need 32)",
            root_raw.len()
        ));
    }

    // Step 5: extract the C2SP signature line for the Sigstore origin.
    // The verifier re-checks the keyID, but failing fast at issue
    // time produces a clearer error than letting the trace fail
    // verification later.
    let checkpoint_sig = extract_signature_line_sigstore(
        &response.verification.inclusion_proof.checkpoint,
        SIGSTORE_REKOR_V1_ORIGIN,
        Some(&*SIGSTORE_REKOR_V1_KEY_ID),
    )?;

    // Pull the active tree-id out of the FIRST line of the signed
    // checkpoint body. The verifier pins SIGSTORE_REKOR_V1_ACTIVE_TREE_ID,
    // so an entry whose checkpoint names a different tree is doomed at
    // verify time. We extract here so the trace bundle can carry it for
    // the verifier; we do NOT default to the constant, because doing so
    // would mask shard-mismatches the verifier is supposed to catch.
    // The parser lives in atlas-trust-core so issuer and verifier never
    // read the C2SP first line through diverging code paths.
    let tree_id = parse_sigstore_checkpoint_tree_id(
        &response.verification.inclusion_proof.checkpoint,
    )?;

    Ok(AnchorEntry {
        kind: req.kind.clone(),
        anchored_hash: sigstore_hash,
        log_id: response.log_id,
        log_index: response.log_index,
        integrated_time: response.integrated_time,
        inclusion_proof: InclusionProof {
            tree_size: response.verification.inclusion_proof.tree_size,
            root_hash: response.verification.inclusion_proof.root_hash,
            hashes: response.verification.inclusion_proof.hashes,
            checkpoint_sig,
        },
        entry_body_b64: Some(response.body),
        tree_id: Some(tree_id),
    })
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

    /// Pin the Atlas anchoring public key derived from `ATLAS_ANCHOR_SEED`.
    ///
    /// Why pin: the seed is currently a constant in source. If anyone edits
    /// the seed, the derivation, or the PEM-encoder version, every Sigstore
    /// anchor produced by future builds will be signed by a different key
    /// — and Rekor would happily admit those entries (it does not pin the
    /// submitter's key). The verifier doesn't pin the Atlas anchoring key
    /// either (Sigstore only commits to the LOG key), so a silent drift
    /// would split the issuer population without any test catching it.
    ///
    /// This pin is a fence: any seed-or-derivation change forces an
    /// explicit, reviewed update of this constant, which makes the
    /// rotation auditable in git history.
    #[test]
    fn atlas_anchor_pubkey_pem_is_pinned() {
        const ATLAS_ANCHOR_PUBKEY_PEM: &str = "-----BEGIN PUBLIC KEY-----\n\
            MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEEcnOZ1Vw8vVyRaJaFJ3nmZVPc0tC\n\
            I8PhAI7kjzJpCVKTFs7Uj0ZdkPJR/8FFru5cbKTHwhbH69w6BQzoRK6jxw==\n\
            -----END PUBLIC KEY-----\n";
        let sk = atlas_anchor_signing_key().expect("seed must derive a valid P-256 scalar");
        let pem = atlas_anchor_public_pem(&sk).expect("must PEM-encode");
        assert_eq!(
            pem, ATLAS_ANCHOR_PUBKEY_PEM,
            "Atlas anchoring pubkey drift: ATLAS_ANCHOR_SEED or the \
             derivation has changed. Update this pin only after auditing \
             who/why and rotating any production traces signed by the \
             previous key."
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

    // ─────────────────────────────────────────────────────────────────────
    // V1.6 Sigstore issuer — wiremock-backed integration test
    // ─────────────────────────────────────────────────────────────────────
    //
    // Scope: assert the request/response wire shape and the AnchorEntry
    // mapping. We do NOT verify the resulting AnchorEntry through
    // `verify_anchor_entry` here, because that would require a real
    // Sigstore-key-signed checkpoint over a real Merkle tree; that round-
    // trip is exercised at full strength by `tests/sigstore_golden.rs`
    // against an actual rekor.sigstore.dev entry. Bridging both at once
    // would push the test into "rebuild Rekor's signing internals"
    // territory — defeats the point of using the real Rekor data for the
    // verifier-side fixture.

    fn build_synthetic_checkpoint(tree_size: u64, root_hex: &str, tree_id: i64) -> String {
        use atlas_trust_core::anchor::SIGSTORE_REKOR_V1_KEY_ID;
        // C2SP signed-note: `<origin> - <tree-id>\n<tree-size>\n<root-b64>\n\n— <origin> <b64-keyID-and-sig>\n`
        let root_raw = hex::decode(root_hex).expect("test fixture: root must be hex");
        let root_b64 = base64::engine::general_purpose::STANDARD.encode(&root_raw);
        // Synthesise a "signature blob" carrying the pinned keyID + 64
        // arbitrary bytes — enough to pass the issuer's length-and-keyID
        // checks. The blob does NOT verify cryptographically; the issuer
        // does not verify, only extracts.
        let mut sig_blob = SIGSTORE_REKOR_V1_KEY_ID.to_vec();
        sig_blob.extend(vec![0xAAu8; 64]);
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(&sig_blob);
        format!(
            "rekor.sigstore.dev - {tree_id}\n{tree_size}\n{root_b64}\n\n\u{2014} rekor.sigstore.dev {sig_b64}\n",
        )
    }

    /// Round-trip POST → AnchorEntry mapping against a wiremock-backed
    /// fake Rekor. Validates:
    ///   • the request body is a hashedrekord/v0.0.1 with `data.hash.value`
    ///     equal to `sigstore_anchored_hash_for(kind, blake3_hex)`;
    ///   • the response is mapped into a well-formed `AnchorEntry` with
    ///     `entry_body_b64` and `tree_id` populated.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn issuer_round_trip_against_mock_rekor() {
        use atlas_trust_core::anchor::SIGSTORE_REKOR_V1_LOG_ID;
        use serde_json::Value;
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, Request, ResponseTemplate};

        let server = MockServer::start().await;

        // Synthetic-but-shape-correct response payload.
        let fake_root_hex = "11".repeat(32);
        let fake_tree_id = 1_193_050_959_916_656_506_i64;
        let fake_tree_size = 1_u64;
        let checkpoint = build_synthetic_checkpoint(fake_tree_size, &fake_root_hex, fake_tree_id);
        let fake_body_b64 =
            base64::engine::general_purpose::STANDARD.encode(b"fake-canonical-entry-body");

        let response_json = serde_json::json!({
            "00112233aabbccdd": {
                "body": fake_body_b64,
                "integratedTime": 1_700_000_000_i64,
                "logID": &*SIGSTORE_REKOR_V1_LOG_ID,
                "logIndex": 42_u64,
                "verification": {
                    "inclusionProof": {
                        "checkpoint": checkpoint,
                        "hashes": ["aa".repeat(32), "bb".repeat(32)],
                        "rootHash": &fake_root_hex,
                        "treeSize": fake_tree_size,
                    }
                }
            }
        });

        Mock::given(method("POST"))
            .and(path("/api/v1/log/entries"))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(201).set_body_json(response_json.clone()))
            .mount(&server)
            .await;

        let url = server.uri();

        let blake3_hex = "33".repeat(32);
        let batch = AnchorBatchInput {
            items: vec![AnchorRequest {
                kind: AnchorKind::DagTip,
                anchored_hash: blake3_hex.clone(),
            }],
            integrated_time: 1_700_000_000,
        };

        // The issuer is blocking; tokio's runtime handles wiremock's
        // request loop. spawn_blocking lets the runtime keep ticking
        // while the issuer waits on the (fake) network call.
        let entries = tokio::task::spawn_blocking({
            let url = url.clone();
            move || issue_anchors_via_rekor(batch, &url)
        })
        .await
        .expect("blocking task must not panic")
        .expect("issuer must succeed against mock rekor");

        // Inspect the request the issuer actually sent.
        let received: Vec<Request> = server.received_requests().await.expect("must capture");
        assert_eq!(received.len(), 1, "exactly one POST per item");
        let body: Value = serde_json::from_slice(&received[0].body)
            .expect("request body must be valid JSON");
        assert_eq!(body["apiVersion"], "0.0.1");
        assert_eq!(body["kind"], "hashedrekord");
        assert_eq!(body["spec"]["data"]["hash"]["algorithm"], "sha256");
        let expected_value = sigstore_anchored_hash_for(&AnchorKind::DagTip, &blake3_hex);
        assert_eq!(
            body["spec"]["data"]["hash"]["value"], expected_value,
            "data.hash.value must equal sigstore_anchored_hash_for(kind, input)",
        );
        // signature.publicKey.content must be a non-empty base64 of the
        // PEM-encoded Atlas anchoring pubkey.
        let pk_b64 = body["spec"]["signature"]["publicKey"]["content"]
            .as_str()
            .expect("publicKey.content must be a string");
        let pk_pem = base64::engine::general_purpose::STANDARD
            .decode(pk_b64.as_bytes())
            .expect("publicKey.content must be base64");
        let pk_pem_str = String::from_utf8(pk_pem).expect("PEM is UTF-8");
        assert!(
            pk_pem_str.starts_with("-----BEGIN PUBLIC KEY-----"),
            "publicKey.content must decode to a PEM SPKI block, got: {pk_pem_str:?}",
        );

        // Inspect the AnchorEntry the issuer produced.
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.kind, AnchorKind::DagTip);
        assert_eq!(e.anchored_hash, expected_value);
        assert_eq!(e.log_id, *SIGSTORE_REKOR_V1_LOG_ID);
        assert_eq!(e.log_index, 42);
        assert_eq!(e.integrated_time, 1_700_000_000);
        assert_eq!(e.entry_body_b64.as_deref(), Some(fake_body_b64.as_str()));
        assert_eq!(e.tree_id, Some(fake_tree_id));
        assert_eq!(e.inclusion_proof.tree_size, fake_tree_size);
        assert_eq!(e.inclusion_proof.root_hash, fake_root_hex);
        assert_eq!(e.inclusion_proof.hashes.len(), 2);
        // checkpoint_sig is the EXTRACTED base64 of the signature line —
        // not the full em-dash line, not the whole checkpoint.
        let raw =
            base64::engine::general_purpose::STANDARD
                .decode(e.inclusion_proof.checkpoint_sig.as_bytes())
                .expect("checkpoint_sig must be standard base64");
        assert!(raw.len() >= 4 + 8, "extracted sig blob too short");
    }

    /// Issuer must surface a clear error when Rekor returns an entry
    /// from a different log (defence-in-depth: even though the verifier
    /// would also reject, we'd rather fail at issue time than ship a
    /// trace doomed to fail verification).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn issuer_rejects_wrong_log_id() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        let fake_root_hex = "22".repeat(32);
        let checkpoint =
            build_synthetic_checkpoint(1, &fake_root_hex, 1_193_050_959_916_656_506);
        let response_json = serde_json::json!({
            "deadbeef": {
                "body": base64::engine::general_purpose::STANDARD.encode(b"x"),
                "integratedTime": 1_700_000_000_i64,
                "logID": "0000000000000000000000000000000000000000000000000000000000000000",
                "logIndex": 0_u64,
                "verification": {
                    "inclusionProof": {
                        "checkpoint": checkpoint,
                        "hashes": [],
                        "rootHash": fake_root_hex,
                        "treeSize": 1_u64,
                    }
                }
            }
        });

        Mock::given(method("POST"))
            .and(path("/api/v1/log/entries"))
            .respond_with(ResponseTemplate::new(201).set_body_json(response_json))
            .mount(&server)
            .await;

        let url = server.uri();
        let batch = AnchorBatchInput {
            items: vec![AnchorRequest {
                kind: AnchorKind::DagTip,
                anchored_hash: "44".repeat(32),
            }],
            integrated_time: 1_700_000_000,
        };

        let err = tokio::task::spawn_blocking(move || issue_anchors_via_rekor(batch, &url))
            .await
            .expect("blocking task must not panic")
            .expect_err("must reject mismatched log_id");
        assert!(
            err.contains("log_id") && err.contains("but Atlas pins"),
            "error must call out log_id mismatch, got: {err}",
        );
    }
}
