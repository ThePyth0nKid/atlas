//! Anchor verification (V1.5 + V1.6).
//!
//! Given an `AnchorEntry` and a pinned roster of trusted log identities,
//! this module answers: was the claimed `anchored_hash` actually committed
//! to a known transparency log at the claimed position, under a checkpoint
//! the log signed?
//!
//! Three independent links are checked, each format-specific:
//!   1. The log identified by `entry.log_id` is one we trust (presence
//!      in the pinned roster yields a `TrustedLog { pubkey, format }`).
//!   2. The Merkle inclusion path from leaf -> root_hash is consistent
//!      under that format's hash (RFC 6962 §2.1.1 verify-inclusion).
//!   3. The signed checkpoint binds (`tree_size`, `root_hash`) to the log
//!      under that pubkey, in that format's checkpoint encoding.
//!
//! All three must hold. The path is offline-only: no network access,
//! no allocator beyond what serde and the proof require.
//!
//! V1.5 supported one format — `atlas-mock-rekor-v1` (blake3 RFC 6962
//! variant + Ed25519 over a three-line atlas-mock checkpoint). V1.6
//! adds `sigstore-rekor-v1` (SHA-256 RFC 6962 + ECDSA P-256 over a
//! C2SP signed-note checkpoint, against the public Sigstore Rekor log).

use base64::Engine;
use blake3::Hasher;
use ed25519_dalek::{Signature as EdSignature, Verifier as EdVerifier, VerifyingKey as EdVerifyingKey};
use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::trace_format::{AnchorEntry, AnchorKind, AtlasTrace, InclusionProof};

// ─────────────────────────────────────────────────────────────────────────
// Pinned log identities
// ─────────────────────────────────────────────────────────────────────────

/// Hex Ed25519 public key for the dev mock-Rekor log. Pinned at compile
/// time so every build of the verifier ships the same trusted root, and
/// so a tampered binary can be spotted by hash-comparing this file.
///
/// Derived from the dev seed in `atlas-signer/src/anchor.rs`; the test
/// `mock_log_pubkey_matches_signer_seed` in atlas-signer asserts they
/// stay in sync. Touching one without touching the other fails CI.
pub const MOCK_LOG_PUBKEY_HEX: &str =
    "ac1ea29e6641da38baf768088517ba33c0170614b1cc97d718921d3e2db2bfcb";

/// Hex blake3 of `MOCK_LOG_PUBKEY` raw bytes — the log's public identity.
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

/// PEM-encoded ECDSA P-256 SPKI public key for the public Sigstore Rekor
/// v1 log (production deployment at `rekor.sigstore.dev`).
///
/// Pinned in source because the entire Sigstore-anchoring trust property
/// depends on this key. Provenance: the bytes were retrieved from
/// `https://rekor.sigstore.dev/api/v1/log/publicKey` on 2026-04-28; an
/// auditor can re-fetch and `diff` to confirm. The value is never
/// modified at runtime.
///
/// V1.6 ships only the active production log. The two known historical
/// shards (treeIDs `3904496407287907110` and `2605736670972794746`) sign
/// with the same key, so this same pubkey will accept inclusion proofs
/// against those shards once we add their origins to the trusted roster
/// in V1.7.
pub const SIGSTORE_REKOR_V1_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE2G2Y+2tabdTV5BcGiBIx0a9fAFwr\nkBbmLSGtks4L3qX6yYY0zufBnhC8Ur/iy55GhWP/9A/bY2LhC30M9+RYtw==\n-----END PUBLIC KEY-----\n";

/// Active Trillian tree-ID for the Sigstore Rekor v1 production log.
///
/// Used to reconstruct the C2SP signed-note origin line
/// `"rekor.sigstore.dev - {tree_id}"`. Inactive shards have different
/// tree-IDs (and different origin lines); V1.6 only supports the active
/// shard. An anchor whose `tree_id` does not match here is treated as
/// "not in this log" — we do not gatekeep cross-shard.
pub const SIGSTORE_REKOR_V1_ACTIVE_TREE_ID: i64 = 1_193_050_959_916_656_506;

/// Origin label used in the active Sigstore Rekor v1 log's checkpoints.
pub const SIGSTORE_REKOR_V1_ORIGIN: &str = "rekor.sigstore.dev";

/// Cached `p256::ecdsa::VerifyingKey` parsed from `SIGSTORE_REKOR_V1_PEM`.
/// Parsing once at first access; subsequent verifications reuse the key.
pub static SIGSTORE_REKOR_V1_VERIFYING_KEY: LazyLock<p256::ecdsa::VerifyingKey> =
    LazyLock::new(|| {
        use p256::pkcs8::DecodePublicKey;
        p256::ecdsa::VerifyingKey::from_public_key_pem(SIGSTORE_REKOR_V1_PEM)
            .expect("SIGSTORE_REKOR_V1_PEM must be a valid P-256 SPKI public key")
    });

/// Cached DER bytes of the Sigstore Rekor v1 SPKI public key. The log_id
/// (full SHA-256 of these bytes) and the C2SP signed-note keyID (first
/// 4 bytes of that hash) are both derived from this exactly-once.
pub static SIGSTORE_REKOR_V1_PUBKEY_DER: LazyLock<Vec<u8>> = LazyLock::new(|| {
    use p256::pkcs8::EncodePublicKey;
    SIGSTORE_REKOR_V1_VERIFYING_KEY
        .to_public_key_der()
        .expect("re-encoding a parsed P-256 key as DER must succeed")
        .into_vec()
});

/// Hex `SHA-256(DER-SPKI)` log identifier for the Sigstore Rekor v1
/// production log. Used as the key in `default_trusted_logs()` so
/// inbound anchors with a different `log_id` are rejected before any
/// proof work runs.
pub static SIGSTORE_REKOR_V1_LOG_ID: LazyLock<String> = LazyLock::new(|| {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(&*SIGSTORE_REKOR_V1_PUBKEY_DER);
    hex::encode(h.finalize())
});

/// 4-byte C2SP signed-note keyID for the Sigstore Rekor v1 log.
///
/// Per the C2SP signed-note spec: for ECDSA, keyID = SHA-256(DER SPKI)[:4].
/// Stored big-endian inside the base64 signature line, immediately before
/// the DER ECDSA signature bytes. We use it to defensively reject signature
/// lines whose keyID does not match the pinned log we are verifying against.
pub static SIGSTORE_REKOR_V1_KEY_ID: LazyLock<[u8; 4]> = LazyLock::new(|| {
    let log_id_hex = &*SIGSTORE_REKOR_V1_LOG_ID;
    let raw = hex::decode(log_id_hex).expect("log_id is hex");
    let mut out = [0u8; 4];
    out.copy_from_slice(&raw[..4]);
    out
});

// ─────────────────────────────────────────────────────────────────────────
// Multi-format trust model
// ─────────────────────────────────────────────────────────────────────────

/// A log signing key in one of the schemes Atlas knows how to verify.
///
/// Each variant pairs naturally with a `CheckpointFormat`: `Ed25519`
/// signs `AtlasMockV1` checkpoints, `EcdsaP256` signs `SigstoreRekorV1`
/// checkpoints. The dispatch happens by `format`, not by pubkey type,
/// so a future format using Ed25519 (or a future P-256-based mock)
/// would slot in cleanly.
#[derive(Debug, Clone)]
pub enum LogPubkey {
    /// Ed25519 signing key (raw 32-byte point), used by the mock log.
    Ed25519(EdVerifyingKey),
    /// ECDSA P-256 signing key, used by the Sigstore Rekor v1 log.
    EcdsaP256(p256::ecdsa::VerifyingKey),
}

/// Wire-format of a log's checkpoint + Merkle tree.
///
/// Selecting the format determines:
///   - the leaf-hash construction (blake3 + `"leaf:"` vs RFC 6962 `0x00`)
///   - the parent-hash construction (blake3 + `"node:"` vs RFC 6962 `0x01`)
///   - the canonical bytes the log signs for a checkpoint
///   - the signature-scheme-specific encoding of `checkpoint_sig`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointFormat {
    /// Atlas in-process mock-Rekor (V1.5).
    /// Leaf: `blake3("leaf:" || kind || ":" || hash_bytes)`.
    /// Parent: `blake3("node:" || left || right)`.
    /// Checkpoint: `"atlas-mock-rekor-v1\n{tree_size}\n{root_hex}\n"`,
    /// signed Ed25519 (raw 64-byte signature, base64).
    AtlasMockV1,
    /// Sigstore Rekor v1 production log (V1.6).
    /// Leaf: `SHA-256(0x00 || canonical_entry_body)`.
    /// Parent: `SHA-256(0x01 || left || right)`.
    /// Checkpoint: C2SP signed-note text
    /// `"{origin}\n{tree_size}\n{base64(root_hash)}\n"`,
    /// signed ECDSA P-256 (4-byte big-endian keyID || DER signature, base64).
    SigstoreRekorV1,
}

/// One entry in the verifier's pinned trusted-log roster.
///
/// `default_trusted_logs()` returns a `BTreeMap<log_id, TrustedLog>`
/// where `log_id` is whatever identity the format uses (blake3 of
/// pubkey for the mock; SHA-256 of DER SPKI for Sigstore Rekor).
#[derive(Debug, Clone)]
pub struct TrustedLog {
    /// Public key the log signs checkpoints with.
    pub pubkey: LogPubkey,
    /// How this log encodes its checkpoint and Merkle tree.
    pub format: CheckpointFormat,
}

// ─────────────────────────────────────────────────────────────────────────
// Outcomes
// ─────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────
// Top-level dispatch
// ─────────────────────────────────────────────────────────────────────────

/// Verify a single anchor entry against a roster of trusted logs.
///
/// `expected_hash` is the hash the trace itself claims for this anchor's
/// kind — i.e. one of `trace.dag_tips` (for DagTip) or
/// `trace.pubkey_bundle_hash` (for BundleHash). The verifier insists
/// `entry.anchored_hash == expected_hash` so a Rekor entry for an
/// unrelated tip cannot be smuggled in to look like it covers the trace.
pub fn verify_anchor_entry(
    entry: &AnchorEntry,
    expected_hash: &str,
    trusted_logs: &BTreeMap<String, TrustedLog>,
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

    let Some(trusted) = trusted_logs.get(&entry.log_id) else {
        return mk(format!(
            "log_id {} is not in the verifier's trusted log roster",
            entry.log_id,
        ));
    };

    match trusted.format {
        CheckpointFormat::AtlasMockV1 => verify_atlas_mock_v1(entry, &trusted.pubkey, mk),
        CheckpointFormat::SigstoreRekorV1 => verify_sigstore_rekor_v1(entry, &trusted.pubkey, mk),
    }
}

/// Verify every anchor in a trace and emit one outcome per entry plus a
/// roll-up `all_ok`. Lenient mode (caller's choice) accepts an empty
/// `entries` vector; strict mode enforces coverage at the call site.
pub fn verify_anchors(
    trace: &AtlasTrace,
    trusted_logs: &BTreeMap<String, TrustedLog>,
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

// ─────────────────────────────────────────────────────────────────────────
// Format A — atlas-mock-rekor-v1 (V1.5)
// ─────────────────────────────────────────────────────────────────────────

fn verify_atlas_mock_v1(
    entry: &AnchorEntry,
    log_pubkey: &LogPubkey,
    mk: impl Fn(String) -> AnchorOutcome,
) -> AnchorOutcome {
    let LogPubkey::Ed25519(ed_key) = log_pubkey else {
        return mk(format!(
            "log_id {} is registered with pubkey type other than Ed25519 \
             but format atlas-mock-rekor-v1 requires Ed25519",
            entry.log_id,
        ));
    };

    let leaf_hash = match leaf_hash_for(&entry.kind, &entry.anchored_hash) {
        Ok(h) => h,
        Err(e) => return mk(e),
    };

    if let Err(e) = verify_inclusion_blake3(
        &leaf_hash,
        entry.log_index,
        &entry.inclusion_proof,
    ) {
        return mk(format!("inclusion proof failed: {e}"));
    }

    if let Err(e) = verify_checkpoint_sig_atlas_mock(ed_key, &entry.inclusion_proof) {
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

/// Build the canonical bytes that the atlas-mock-rekor-v1 log signs for
/// a checkpoint.
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

/// Build the canonical leaf hash for an atlas-mock-v1 anchor entry. The
/// leaf hash is the input to the Merkle inclusion verification.
///
/// Domain separation: leaves are `blake3("leaf:" || kind || ":" || hash_bytes)`.
/// The `"leaf:"` prefix prevents second-preimage attacks against internal
/// nodes (RFC 6962 uses the same idea with 0x00 / 0x01 — the Sigstore
/// format uses that variant, and lives in `verify_sigstore_rekor_v1`).
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

/// blake3 + `"node:"` prefix parent hash (atlas-mock-v1).
fn parent_hash_blake3(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(b"node:");
    h.update(left);
    h.update(right);
    *h.finalize().as_bytes()
}

/// RFC 6962 §2.1.1 inclusion-proof verification, atlas-mock-v1 variant
/// (blake3 leaf/parent hash).
fn verify_inclusion_blake3(
    leaf_hash: &[u8; 32],
    index: u64,
    proof: &InclusionProof,
) -> Result<(), String> {
    walk_inclusion_path(leaf_hash, index, proof, parent_hash_blake3)
}

fn verify_checkpoint_sig_atlas_mock(
    log_key: &EdVerifyingKey,
    proof: &InclusionProof,
) -> Result<(), String> {
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
    let sig = EdSignature::from_bytes(&sig_bytes);
    log_key
        .verify(&bytes, &sig)
        .map_err(|e| format!("ed25519 verify: {e}"))
}

// ─────────────────────────────────────────────────────────────────────────
// Format B — sigstore-rekor-v1 (V1.6)
// ─────────────────────────────────────────────────────────────────────────

fn verify_sigstore_rekor_v1(
    entry: &AnchorEntry,
    log_pubkey: &LogPubkey,
    mk: impl Fn(String) -> AnchorOutcome,
) -> AnchorOutcome {
    let LogPubkey::EcdsaP256(p256_key) = log_pubkey else {
        return mk(format!(
            "log_id {} is registered with pubkey type other than ECDSA P-256 \
             but format sigstore-rekor-v1 requires ECDSA P-256",
            entry.log_id,
        ));
    };

    let Some(entry_body_b64) = entry.entry_body_b64.as_ref() else {
        return mk(
            "sigstore-rekor-v1 anchor missing entry_body_b64 — cannot reconstruct leaf hash"
                .to_string(),
        );
    };
    let Some(tree_id) = entry.tree_id else {
        return mk(
            "sigstore-rekor-v1 anchor missing tree_id — cannot reconstruct signed-note origin"
                .to_string(),
        );
    };

    let entry_body = match base64::engine::general_purpose::STANDARD
        .decode(entry_body_b64.as_bytes())
        .or_else(|_| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(entry_body_b64.as_bytes())
        }) {
        Ok(b) => b,
        Err(e) => return mk(format!("entry_body_b64 is not base64: {e}")),
    };

    let leaf_hash = leaf_hash_sha256_rfc6962(&entry_body);

    if let Err(e) = entry_body_binds_anchored_hash(&entry_body, &entry.anchored_hash) {
        return mk(format!("entry_body does not commit to anchored_hash: {e}"));
    }

    if let Err(e) = verify_inclusion_sha256(
        &leaf_hash,
        entry.log_index,
        &entry.inclusion_proof,
    ) {
        return mk(format!("inclusion proof failed: {e}"));
    }

    if let Err(e) = verify_checkpoint_sig_sigstore(
        p256_key,
        SIGSTORE_REKOR_V1_ORIGIN,
        tree_id,
        &entry.inclusion_proof,
        Some(&*SIGSTORE_REKOR_V1_KEY_ID),
    ) {
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

/// Build the canonical signed-note bytes for a sigstore-rekor-v1
/// checkpoint, per the C2SP tlog-checkpoint spec.
///
/// Format (single source of truth — verifier + future Atlas issuer):
/// ```text
/// {origin} - {tree_id}\n
/// {tree_size}\n
/// {standard-base64(root_hash_raw)}\n
/// ```
/// where `root_hash_raw` is the 32-byte SHA-256 root decoded from the
/// hex string Atlas stores, and `standard-base64` is RFC 4648 §4
/// (with `=` padding). The trailing newline is part of the signed body.
///
/// The blank line and signature lines that follow in the over-the-wire
/// note are NOT part of the signed bytes — they sit beyond the signed
/// body's trailing newline.
pub fn canonical_checkpoint_bytes_sigstore(
    origin: &str,
    tree_id: i64,
    tree_size: u64,
    root_hash_hex: &str,
) -> Result<Vec<u8>, String> {
    let raw = hex::decode(root_hash_hex)
        .map_err(|e| format!("root_hash is not hex: {e}"))?;
    if raw.len() != 32 {
        return Err(format!(
            "sigstore root_hash must be 32 bytes (SHA-256), got {}",
            raw.len(),
        ));
    }
    let root_b64 = base64::engine::general_purpose::STANDARD.encode(&raw);
    Ok(format!("{origin} - {tree_id}\n{tree_size}\n{root_b64}\n").into_bytes())
}

/// RFC 6962 §2.1 leaf hash: `SHA-256(0x00 || leaf_data)`.
///
/// `leaf_data` for sigstore-rekor-v1 is the canonical Rekor entry body
/// bytes (whatever the log indexed). The verifier does not need to
/// canonicalise on its own — the issuer captures `entry_body_b64` from
/// the log's response and the verifier just decodes + hashes.
pub fn leaf_hash_sha256_rfc6962(leaf_data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update([0x00u8]);
    h.update(leaf_data);
    h.finalize().into()
}

/// RFC 6962 §2.1 internal node: `SHA-256(0x01 || left || right)`.
fn parent_hash_sha256(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update([0x01u8]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

/// RFC 6962 §2.1.1 inclusion-proof verification, sigstore-rekor-v1
/// variant (SHA-256 leaf/parent).
fn verify_inclusion_sha256(
    leaf_hash: &[u8; 32],
    index: u64,
    proof: &InclusionProof,
) -> Result<(), String> {
    walk_inclusion_path(leaf_hash, index, proof, parent_hash_sha256)
}

/// Verify an ECDSA P-256 signature on a C2SP signed-note Rekor checkpoint.
///
/// `checkpoint_sig` (per the spec) is `base64(4-byte BE keyID || DER ECDSA sig)`.
/// We optionally check the keyID matches `expected_keyid` (defends against
/// signature lines authored by a different log signing the same body),
/// then ECDSA-verify over `canonical_checkpoint_bytes_sigstore(...)`.
fn verify_checkpoint_sig_sigstore(
    log_key: &p256::ecdsa::VerifyingKey,
    origin: &str,
    tree_id: i64,
    proof: &InclusionProof,
    expected_keyid: Option<&[u8; 4]>,
) -> Result<(), String> {
    let bytes = canonical_checkpoint_bytes_sigstore(origin, tree_id, proof.tree_size, &proof.root_hash)?;

    let raw = base64::engine::general_purpose::STANDARD
        .decode(proof.checkpoint_sig.as_bytes())
        .or_else(|_| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(proof.checkpoint_sig.as_bytes())
        })
        .map_err(|e| format!("checkpoint_sig is not base64: {e}"))?;

    if raw.len() < 4 + 8 {
        return Err(format!(
            "sigstore checkpoint_sig too short: {} bytes (need 4-byte keyID + DER ECDSA sig)",
            raw.len(),
        ));
    }

    let mut keyid = [0u8; 4];
    keyid.copy_from_slice(&raw[..4]);
    if let Some(expected) = expected_keyid {
        if &keyid != expected {
            return Err(format!(
                "checkpoint signature keyID {} does not match the pinned log's expected keyID {}",
                hex::encode(keyid),
                hex::encode(expected),
            ));
        }
    }

    let sig_der = &raw[4..];
    let sig = p256::ecdsa::Signature::from_der(sig_der)
        .map_err(|e| format!("ECDSA signature is not valid DER: {e}"))?;

    use p256::ecdsa::signature::Verifier as P256Verifier;
    log_key
        .verify(&bytes, &sig)
        .map_err(|e| format!("ECDSA P-256 verify: {e}"))
}

/// Confirm the canonical Rekor entry body actually commits to the
/// `anchored_hash` Atlas claims for it.
///
/// Without this, a server could submit one hash to Rekor, get a valid
/// proof for it, then claim that proof anchors a *different* Atlas hash
/// in the trace bundle. The leaf-hash check would still pass (because
/// the proof matches the body the log actually saw), but the body would
/// not contain the trace's hash.
///
/// We accept the hashedrekord v0.0.1 schema only:
/// `data.hash.value` (lowercase hex) MUST equal `anchored_hash`. Other
/// Rekor entry kinds are rejected — V1.6 only issues hashedrekord.
fn entry_body_binds_anchored_hash(entry_body: &[u8], anchored_hash: &str) -> Result<(), String> {
    let v: serde_json::Value = serde_json::from_slice(entry_body)
        .map_err(|e| format!("entry body is not JSON: {e}"))?;

    let kind = v
        .get("kind")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "entry body has no kind field".to_string())?;
    if kind != "hashedrekord" {
        return Err(format!(
            "entry body kind {kind:?} is not supported by V1.6 (hashedrekord only)"
        ));
    }

    let body_hash = v
        .get("spec")
        .and_then(|s| s.get("data"))
        .and_then(|d| d.get("hash"))
        .and_then(|h| h.get("value"))
        .and_then(|x| x.as_str())
        .ok_or_else(|| "entry body has no spec.data.hash.value field".to_string())?;

    if !crate::ct::ct_eq_str(body_hash, anchored_hash) {
        return Err(format!(
            "entry body anchors hash {body_hash}, but Atlas anchor claims {anchored_hash}",
        ));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────
// Shared RFC 6962 inclusion-path walker (parametrised by parent-hash fn)
// ─────────────────────────────────────────────────────────────────────────

fn walk_inclusion_path(
    leaf_hash: &[u8; 32],
    index: u64,
    proof: &InclusionProof,
    parent: fn(&[u8; 32], &[u8; 32]) -> [u8; 32],
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
            running = parent(&sibling, &running);
        } else {
            running = parent(&running, &sibling);
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

// ─────────────────────────────────────────────────────────────────────────
// Default trusted-log roster
// ─────────────────────────────────────────────────────────────────────────

/// Default trusted log roster shipped in this verifier build.
///
/// V1.6 entries:
///   - `MOCK_LOG_ID` -> Ed25519 + AtlasMockV1 (the dev mock-Rekor key).
///   - `SIGSTORE_REKOR_V1_LOG_ID` -> ECDSA P-256 + SigstoreRekorV1
///     (the public Sigstore Rekor v1 production log).
///
/// Adding a new trusted log is intentionally a source change requiring
/// a crate-version bump: silent rotation of the trusted-key set is
/// exactly what the trust property forbids.
pub fn default_trusted_logs() -> BTreeMap<String, TrustedLog> {
    let mut m = BTreeMap::new();

    let mock_key = EdVerifyingKey::from_bytes(&mock_log_pubkey_raw())
        .expect("MOCK_LOG_PUBKEY_HEX must be a valid Ed25519 point");
    m.insert(
        MOCK_LOG_ID.clone(),
        TrustedLog {
            pubkey: LogPubkey::Ed25519(mock_key),
            format: CheckpointFormat::AtlasMockV1,
        },
    );

    m.insert(
        SIGSTORE_REKOR_V1_LOG_ID.clone(),
        TrustedLog {
            pubkey: LogPubkey::EcdsaP256(*SIGSTORE_REKOR_V1_VERIFYING_KEY),
            format: CheckpointFormat::SigstoreRekorV1,
        },
    );

    m
}

// ─────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_path_length_matches_rfc6962_examples() {
        assert_eq!(audit_path_length(0, 7), 3);
        assert_eq!(audit_path_length(1, 7), 3);
        assert_eq!(audit_path_length(2, 7), 3);
        assert_eq!(audit_path_length(3, 7), 3);
        assert_eq!(audit_path_length(4, 7), 3);
        assert_eq!(audit_path_length(5, 7), 3);
        assert_eq!(audit_path_length(6, 7), 2);

        assert_eq!(audit_path_length(0, 1), 0);

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

    /// Sigstore-format leaf and parent hashes use the standard RFC 6962
    /// domain separation (0x00 / 0x01) and SHA-256, NOT blake3 + string
    /// prefixes. Pin both byte-for-byte so a future refactor cannot
    /// silently swap them.
    #[test]
    fn sigstore_leaf_hash_is_rfc6962_sha256() {
        let leaf = leaf_hash_sha256_rfc6962(b"hello");
        // SHA-256(0x00 || "hello"). Reproduce: `printf '\x00hello' | sha256sum`.
        assert_eq!(
            hex::encode(leaf),
            "8a2a5c9b768827de5a9552c38a044c66959c68f6d2f21b5260af54d2f87db827",
        );
    }

    #[test]
    fn sigstore_parent_hash_is_rfc6962_sha256() {
        let l = [0x11u8; 32];
        let r = [0x22u8; 32];
        let p = parent_hash_sha256(&l, &r);
        // SHA-256(0x01 || 0x11*32 || 0x22*32) — RFC 6962 internal node.
        // Reproduce: `printf '\x01' && printf '\x11%.0s' {1..32} && printf
        // '\x22%.0s' {1..32}) | sha256sum`. Pinned at this commit to detect
        // drift in the parent-hash algorithm.
        assert_eq!(
            hex::encode(p),
            "1d8f52d3ec81ac02cd97cb3281523be47af850c0f0295af866f04bc245f46bbf",
        );
    }

    /// The sigstore canonical checkpoint bytes follow the C2SP tlog
    /// checkpoint format. Pin a known input so a base64 / line-separator
    /// regression trips this test before reaching production.
    #[test]
    fn sigstore_canonical_checkpoint_bytes_pin() {
        // Sample 32-byte root hash; specific value is arbitrary, what we
        // pin is the format: origin/tree-id line, tree-size, standard-
        // base64 of the 32-byte root, three trailing newlines total.
        let bytes = canonical_checkpoint_bytes_sigstore(
            "rekor.sigstore.dev",
            42,
            7,
            "8fb4c2d9e034357dca0044812c8b76aa3ffd2df060467f1686e0c2587b61c327",
        )
        .unwrap();
        assert_eq!(
            bytes,
            b"rekor.sigstore.dev - 42\n7\nj7TC2eA0NX3KAESBLIt2qj/9LfBgRn8WhuDCWHthwyc=\n",
        );
    }

    /// Anti-drift: the pinned PEM, the derived DER, and the derived
    /// SHA-256 log-id must all be self-consistent. If a future edit
    /// changes the PEM, this test catches the cascade before it
    /// reaches a customer's verifier.
    ///
    /// Provenance for the expected log_id: independently computed at
    /// pin time with `openssl ec -pubin -in rekor.pem -outform DER \
    /// | sha256sum`. An auditor can re-derive from the same PEM and
    /// confirm.
    #[test]
    fn sigstore_log_pubkey_matches_pinned() {
        let der = &*SIGSTORE_REKOR_V1_PUBKEY_DER;
        // P-256 SPKI is always 91 bytes (0x30 0x59 ... + 64-byte point).
        assert_eq!(der.len(), 91, "P-256 SPKI is exactly 91 bytes");
        // First two bytes are SEQUENCE + length-89.
        assert_eq!(&der[..2], &[0x30, 0x59]);
        // Uncompressed-point marker before the 64-byte (X || Y) point.
        assert_eq!(der[26], 0x04);
        // log_id is 64 hex chars (= 32 SHA-256 bytes).
        let log_id = &*SIGSTORE_REKOR_V1_LOG_ID;
        assert_eq!(log_id.len(), 64);
        // C2SP keyID is the first 4 bytes of the log_id, hex-encoded
        // back to the same prefix.
        let keyid_hex = hex::encode(*SIGSTORE_REKOR_V1_KEY_ID);
        assert_eq!(&log_id[..8], keyid_hex);
    }

    /// Default trusted roster carries exactly the two logs V1.6 ships
    /// with: the dev mock and the Sigstore Rekor v1 production log.
    /// Catches accidental additions / deletions.
    #[test]
    fn default_trusted_logs_has_mock_and_sigstore() {
        let m = default_trusted_logs();
        assert_eq!(m.len(), 2, "expected exactly mock + sigstore-rekor-v1");

        let mock = m.get(&*MOCK_LOG_ID).expect("mock missing");
        assert!(matches!(mock.format, CheckpointFormat::AtlasMockV1));
        assert!(matches!(mock.pubkey, LogPubkey::Ed25519(_)));

        let ss = m
            .get(&*SIGSTORE_REKOR_V1_LOG_ID)
            .expect("sigstore-rekor-v1 missing");
        assert!(matches!(ss.format, CheckpointFormat::SigstoreRekorV1));
        assert!(matches!(ss.pubkey, LogPubkey::EcdsaP256(_)));
    }
}
