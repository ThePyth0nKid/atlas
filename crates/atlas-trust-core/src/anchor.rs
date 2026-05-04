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
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::error::{TrustError, TrustResult};
use crate::trace_format::{
    AnchorBatch, AnchorChain, AnchorEntry, AnchorKind, AtlasTrace, InclusionProof,
    ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
};

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
/// V1.7 trusts the active production log plus the two known historical
/// Sigstore Rekor v1 shards (treeIDs `3904496407287907110` and
/// `2605736670972794746`). All three shards sign with this same key —
/// the only per-shard difference is the tree-ID embedded in the signed
/// origin line — so adding shards is a roster widening, not a key
/// rotation. See `SIGSTORE_REKOR_V1_TREE_IDS` for the enforced roster.
pub const SIGSTORE_REKOR_V1_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE2G2Y+2tabdTV5BcGiBIx0a9fAFwr\nkBbmLSGtks4L3qX6yYY0zufBnhC8Ur/iy55GhWP/9A/bY2LhC30M9+RYtw==\n-----END PUBLIC KEY-----\n";

/// Active Trillian tree-ID for the Sigstore Rekor v1 production log.
///
/// Issuer-side constant: `atlas-signer anchor --rekor-url …` always
/// posts to the active shard (Rekor's public API only accepts new
/// entries on the active shard), so the value the issuer embeds in
/// fresh `AnchorEntry.tree_id` rows is always this one.
///
/// Verifier-side acceptance is broader: see
/// `SIGSTORE_REKOR_V1_TREE_IDS` for the roster of tree-IDs accepted by
/// `verify_sigstore_rekor_v1`. Historical shards are accepted because
/// their checkpoints — pre-existing on-disk anchors that an auditor
/// may have captured years ago — were signed by this same key, just
/// with a different `tree_id` baked into the C2SP origin line.
pub const SIGSTORE_REKOR_V1_ACTIVE_TREE_ID: i64 = 1_193_050_959_916_656_506;

/// Roster of Sigstore Rekor v1 Trillian tree-IDs accepted by the
/// verifier.
///
/// Index 0 is load-bearing: it must remain `SIGSTORE_REKOR_V1_ACTIVE_TREE_ID`
/// so the issuer (which posts to the active shard) and the verifier (which
/// accepts the active shard plus historical reads) cannot drift apart
/// silently. The remaining indices are unordered membership entries —
/// `is_known_sigstore_rekor_v1_tree_id` does linear scan, and the
/// `sigstore_tree_id_roster_is_pinned` test enforces both the index-0
/// invariant and the exact set.
///
/// Members:
///   * `1_193_050_959_916_656_506` — active production shard (post-2024).
///   * `3_904_496_407_287_907_110` — historical shard.
///   * `2_605_736_670_972_794_746` — earliest historical shard.
///
/// Provenance for the historical IDs: Sigstore's public shard rotation
/// history is published in the `sigstore/root-signing` repository (the
/// TUF trust root, GitHub: <https://github.com/sigstore/root-signing>)
/// and announced in Sigstore release blog posts. An auditor can confirm
/// the values by inspecting that repository's published `targets`
/// metadata for Rekor public-log shards. The Atlas crate pins them in
/// source so a silent registry change cannot retroactively widen what
/// this verifier trusts.
///
/// All three shards sign with `SIGSTORE_REKOR_V1_PEM`. The tree-ID is
/// part of the C2SP signed-note origin line
/// `"rekor.sigstore.dev - {tree_id}\n"`, so a checkpoint signed for
/// shard A will not verify under origin B even with the right key —
/// the verifier therefore must know which tree-IDs are legitimately
/// part of the Sigstore Rekor v1 deployment to decide which origin to
/// reconstruct. This roster encodes that knowledge.
///
/// Adding a new shard is intentionally a source change requiring a
/// crate-version bump. Silent acceptance of unknown tree-IDs is
/// exactly what the trust property forbids — an attacker who could
/// stand up a same-key shard with a tree-ID we trust by default would
/// have a forgery primitive against pre-existing anchors.
pub const SIGSTORE_REKOR_V1_TREE_IDS: &[i64] = &[
    SIGSTORE_REKOR_V1_ACTIVE_TREE_ID,
    3_904_496_407_287_907_110,
    2_605_736_670_972_794_746,
];

/// Membership test for the Sigstore Rekor v1 tree-ID roster.
///
/// `pub(crate)` because the canonical public surface is the constant
/// `SIGSTORE_REKOR_V1_TREE_IDS` itself; external callers either inspect
/// the slice directly or do their own membership check. Keeping this
/// helper crate-private avoids cementing an internal sugar function in
/// the public API.
///
/// Constant-time is not required here: the roster is a public list and
/// timing leaks reveal nothing the attacker does not already know.
/// Linear scan over 3 elements is faster than any hash-set lookup at
/// this size and keeps the binary smaller.
pub(crate) fn is_known_sigstore_rekor_v1_tree_id(tree_id: i64) -> bool {
    SIGSTORE_REKOR_V1_TREE_IDS.contains(&tree_id)
}

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

/// Domain-separated SHA-256 derivation of an Atlas blake3 hash into the
/// SHA-256 hex string that Sigstore Rekor's `hashedrekord` schema requires
/// for `data.hash.value` (algorithm = "sha256").
///
/// Atlas internally hashes events and pubkey-bundles with blake3, but
/// Rekor's hashedrekord v0.0.1 spec only accepts SHA-1/256/384/512. The
/// V1.6 verifier pins `algorithm == "sha256"` (see
/// `entry_body_binds_anchored_hash`), so when we anchor a blake3 hash to
/// Sigstore we have to commit to a derived SHA-256 that an offline
/// auditor can independently recompute from the trace's blake3.
///
/// Why a single tiny derivation function rather than "SHA-256 of the
/// canonical signing-input"? Because the latter would force the verifier
/// to re-canonicalise the underlying event/bundle before checking the
/// anchor. That logic exists, but routing it through here doubles the
/// canonical-bytes drift surface that the Phase 1 trust property
/// minimised. One pure pre-hash function shared by issuer + verifier is
/// the smallest possible bind.
///
/// Domain separation by `kind`: `dag_tip` and `bundle_hash` use
/// different prefixes, so even if Atlas ever produced a blake3 collision
/// across a tip and a bundle (it cannot under blake3, but defence in
/// depth) the derived Sigstore hashes still differ — preventing an
/// adversary from re-targeting one anchor as the other.
///
/// The version suffix `-v1` lets V1.7+ add new domains (e.g.
/// `atlas-policy-set-v2:`) without retroactively changing what older
/// trace bundles' anchors commit to.
///
/// **Single source of truth**: every Sigstore-format issuer (atlas-signer
/// `--rekor-url`) MUST call this helper to compute the `value` it
/// submits to Rekor. Every Sigstore-format verifier MUST call this
/// helper to recompute the expected hash it compares against
/// `entry.anchored_hash`. Drift between the two is exactly what the
/// trust property forbids.
pub fn sigstore_anchored_hash_for(kind: &AnchorKind, blake3_hex: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(sigstore_artifact_prefix_for(kind));
    h.update(blake3_hex.as_bytes());
    hex::encode(h.finalize())
}

/// Sister of [`sigstore_anchored_hash_for`]: returns the **artifact bytes**
/// the issuer signs with the Atlas anchoring key before submitting to
/// Rekor as a `hashedrekord`. By construction:
///
///   sigstore_anchored_hash_for(kind, hex)
///     == hex(SHA-256(sigstore_artifact_bytes_for(kind, hex)))
///
/// Rekor's `hashedrekord` schema commits the SHA-256 hash of an opaque
/// artifact and an asymmetric signature over that artifact. The verifier
/// only needs the hash (it checks `body.spec.data.hash.value ==
/// anchored_hash`); the signature is verified by Sigstore at submit time
/// and the verifier does not re-check it. So the only requirement on the
/// artifact bytes is that the issuer and verifier agree byte-for-byte on
/// what hashes to `anchored_hash`. This helper centralises the agreement.
pub fn sigstore_artifact_bytes_for(kind: &AnchorKind, blake3_hex: &str) -> Vec<u8> {
    let prefix = sigstore_artifact_prefix_for(kind);
    let mut v = Vec::with_capacity(prefix.len() + blake3_hex.len());
    v.extend_from_slice(prefix);
    v.extend_from_slice(blake3_hex.as_bytes());
    v
}

/// Internal: domain-separation prefix shared by `sigstore_anchored_hash_for`
/// and `sigstore_artifact_bytes_for`. Defined once so a future edit cannot
/// silently desynchronise the two helpers.
#[inline]
fn sigstore_artifact_prefix_for(kind: &AnchorKind) -> &'static [u8] {
    match kind {
        AnchorKind::DagTip => b"atlas-dag-tip-v1:",
        AnchorKind::BundleHash => b"atlas-bundle-hash-v1:",
    }
}

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
    /// Hash this entry vouched for, exactly as serialised in the
    /// `AnchorEntry`. For `atlas-mock-rekor-v1` this is the trace's
    /// blake3; for `sigstore-rekor-v1` this is the derived SHA-256
    /// (see `sigstore_anchored_hash_for`).
    pub anchored_hash: String,
    /// The trace-native blake3 hash this anchor commits to — identical
    /// to `anchored_hash` for `atlas-mock-rekor-v1`, and the underlying
    /// blake3 (`dag_tip` or `pubkey_bundle_hash`) it was derived from
    /// for `sigstore-rekor-v1`. Strict-mode coverage uses this so
    /// "every dag_tip is anchored" still works across formats.
    pub trace_hash: String,
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
    // `trace_hash` defaults to `expected_hash` — the caller-supplied
    // trace-side hash. For atlas-mock-rekor-v1 this is the blake3 the
    // trace claims (`expected_hash == entry.anchored_hash` after the
    // bind-check below). For sigstore-rekor-v1 the wrapper
    // `verify_anchors` overrides it with the matched dag_tip / bundle
    // blake3 so strict-mode coverage works across formats.
    let trace_hash_default = expected_hash.to_string();
    let mk = |reason: String| AnchorOutcome {
        ok: false,
        kind: entry.kind.clone(),
        anchored_hash: entry.anchored_hash.clone(),
        trace_hash: trace_hash_default.clone(),
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
        CheckpointFormat::AtlasMockV1 => {
            verify_atlas_mock_v1(entry, &trusted.pubkey, &trace_hash_default, mk)
        }
        CheckpointFormat::SigstoreRekorV1 => {
            verify_sigstore_rekor_v1(entry, &trusted.pubkey, &trace_hash_default, mk)
        }
    }
}

/// Verify every anchor in a trace and emit one outcome per entry.
/// Lenient by default (empty `trace.anchors` returns empty); strict
/// coverage is enforced by the caller via `VerifyOptions::require_anchors`.
///
/// Two responsibilities here that `verify_anchor_entry` cannot do alone,
/// because both depend on trace-level context:
///
///   1. **Coverage check** — `entry.anchored_hash` MUST commit to a hash
///      the trace itself claims (`trace.dag_tips` for `DagTip`,
///      `trace.pubkey_bundle_hash` for `BundleHash`). Otherwise a server
///      could attach a valid-but-unrelated Rekor proof and pass it off
///      as covering the trace.
///
///   2. **Format-specific derivation** — for `sigstore-rekor-v1`,
///      `entry.anchored_hash` is a SHA-256 derived from the trace's
///      blake3 via `sigstore_anchored_hash_for`. The wrapper recomputes
///      the derivation, finds which trace-side blake3 matches, and
///      records that blake3 in `outcome.trace_hash` so strict-mode
///      coverage works uniformly across formats.
pub fn verify_anchors(
    trace: &AtlasTrace,
    trusted_logs: &BTreeMap<String, TrustedLog>,
) -> Vec<AnchorOutcome> {
    let mut out = Vec::with_capacity(trace.anchors.len());
    for entry in &trace.anchors {
        // Look up the log first. Unknown log → reject before doing any
        // coverage / derivation work; the failure reason is identical to
        // what `verify_anchor_entry` would produce, so callers see one
        // canonical error message.
        let Some(trusted) = trusted_logs.get(&entry.log_id) else {
            out.push(AnchorOutcome {
                ok: false,
                kind: entry.kind.clone(),
                anchored_hash: entry.anchored_hash.clone(),
                trace_hash: entry.anchored_hash.clone(),
                log_id: entry.log_id.clone(),
                reason: format!(
                    "log_id {} is not in the verifier's trusted log roster",
                    entry.log_id,
                ),
            });
            continue;
        };
        let log_format = trusted.format;

        // Resolve coverage: for which trace-side blake3 (`trace_hash`) does
        // `entry.anchored_hash` claim to be a witness, and what value does
        // `verify_anchor_entry` expect for the per-entry equality check?
        // Pair them so we never pass the wrong one to the inner call.
        let resolution = match (&entry.kind, log_format) {
            (AnchorKind::DagTip, CheckpointFormat::AtlasMockV1) => trace
                .dag_tips
                .iter()
                .find(|t| crate::ct::ct_eq_str(t, &entry.anchored_hash))
                .map(|t| (entry.anchored_hash.clone(), t.clone())),
            (AnchorKind::DagTip, CheckpointFormat::SigstoreRekorV1) => trace
                .dag_tips
                .iter()
                .find(|t| {
                    crate::ct::ct_eq_str(
                        &sigstore_anchored_hash_for(&AnchorKind::DagTip, t),
                        &entry.anchored_hash,
                    )
                })
                .map(|t| (entry.anchored_hash.clone(), t.clone())),
            (AnchorKind::BundleHash, CheckpointFormat::AtlasMockV1) => {
                if crate::ct::ct_eq_str(
                    &trace.pubkey_bundle_hash,
                    &entry.anchored_hash,
                ) {
                    Some((entry.anchored_hash.clone(), trace.pubkey_bundle_hash.clone()))
                } else {
                    None
                }
            }
            (AnchorKind::BundleHash, CheckpointFormat::SigstoreRekorV1) => {
                let derived = sigstore_anchored_hash_for(
                    &AnchorKind::BundleHash,
                    &trace.pubkey_bundle_hash,
                );
                if crate::ct::ct_eq_str(&derived, &entry.anchored_hash) {
                    Some((entry.anchored_hash.clone(), trace.pubkey_bundle_hash.clone()))
                } else {
                    None
                }
            }
        };

        let Some((expected, trace_hash)) = resolution else {
            let reason = match entry.kind {
                AnchorKind::DagTip => format!(
                    "anchored dag_tip hash {} does not cover any trace.dag_tips entry under format {:?}",
                    &entry.anchored_hash, log_format,
                ),
                AnchorKind::BundleHash => format!(
                    "anchored bundle_hash {} does not match trace.pubkey_bundle_hash under format {:?}",
                    &entry.anchored_hash, log_format,
                ),
            };
            out.push(AnchorOutcome {
                ok: false,
                kind: entry.kind.clone(),
                anchored_hash: entry.anchored_hash.clone(),
                trace_hash: entry.anchored_hash.clone(),
                log_id: entry.log_id.clone(),
                reason,
            });
            continue;
        };

        // Inner per-entry verification. trace_hash is overridden after
        // the call: verify_anchor_entry's default sets it to expected
        // (correct for AtlasMockV1, equal-hash; wrong for SigstoreRekorV1
        // where expected is sha256-derived and trace_hash is blake3).
        let outcome = verify_anchor_entry(entry, &expected, trusted_logs);
        out.push(AnchorOutcome {
            trace_hash,
            ..outcome
        });
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────
// Format A — atlas-mock-rekor-v1 (V1.5)
// ─────────────────────────────────────────────────────────────────────────

fn verify_atlas_mock_v1(
    entry: &AnchorEntry,
    log_pubkey: &LogPubkey,
    trace_hash: &str,
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
        trace_hash: trace_hash.to_string(),
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
    trace_hash: &str,
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

    // V1.7 trusts the active Sigstore Rekor v1 shard plus the two
    // known historical shards. Reject any other tree_id explicitly so
    // the auditor sees a precise reason (rather than a generic
    // "ECDSA P-256 verify" failure later in the path) when somebody
    // tries to substitute a same-key but unrecognised-shard checkpoint.
    if !is_known_sigstore_rekor_v1_tree_id(tree_id) {
        return mk(format!(
            "anchor tree_id {tree_id} is not in the Sigstore Rekor v1 trusted-shard roster ({:?})",
            SIGSTORE_REKOR_V1_TREE_IDS,
        ));
    }

    // Sigstore returns the entry body as RFC 4648 §4 STANDARD base64
    // (with `=` padding). Strict-only — a deviation here means we are
    // looking at non-Sigstore data and should refuse, not paper over.
    let entry_body = match base64::engine::general_purpose::STANDARD
        .decode(entry_body_b64.as_bytes())
    {
        Ok(b) => b,
        Err(e) => return mk(format!("entry_body_b64 is not standard base64: {e}")),
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
        trace_hash: trace_hash.to_string(),
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
    // Defensive: the C2SP signed-note format uses `\n` as the line
    // separator and ` - ` as the origin/tree-id separator on line 1.
    // An attacker-controlled origin containing either could splice a
    // forged origin/tree-id pair into the signed body. Origin is a
    // public function arg (not always a constant), so reject those
    // characters explicitly. Empty origin is also nonsensical.
    if origin.is_empty() {
        return Err("origin must not be empty".to_string());
    }
    if origin.contains('\n') {
        return Err("origin must not contain a newline".to_string());
    }
    if origin.contains(" - ") {
        return Err(
            "origin must not contain ' - ' (would collide with the tree-id separator)"
                .to_string(),
        );
    }

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

// ─────────────────────────────────────────────────────────────────────────
// Anchor-chain head computation (V1.7)
// ─────────────────────────────────────────────────────────────────────────

/// Domain-separation prefix for `chain_head_for`. Mixed in with blake3 so
/// the chain hash cannot collide with any other hash in the system (event
/// hash, leaf hash, bundle hash, checkpoint signing input).
///
/// Versioned (`-v1`) so a future canonicalization change can ship under a
/// new prefix without invalidating already-issued chains. If the format
/// here changes incompatibly, bump to `-v2` AND bump `atlas-trust-core`'s
/// crate version so `VERIFIER_VERSION` cascades.
pub const ANCHOR_CHAIN_DOMAIN: &[u8] = b"atlas-anchor-chain-v1:";

/// Subset of `AnchorBatch` fields that participate in `chain_head_for`.
///
/// V1.13 added `AnchorBatch.witnesses: Vec<WitnessSig>` — but a witness
/// signs OVER the chain head, so the chain head computation MUST exclude
/// witnesses (otherwise infinite regress: head depends on witness depends
/// on head). Defining the canonical input as a separate struct makes the
/// chain-head contract explicit: adding a new field to `AnchorBatch` is
/// a deliberate choice about whether it joins this view (changes the
/// head) or stays out of it (does not).
///
/// Field NAMES here MUST match `AnchorBatch` exactly — canonical JSON
/// sorts keys lex, so the resulting bytes are byte-identical to a
/// pre-V1.13 serialisation of `AnchorBatch` (no `witnesses` field). This
/// preserves already-issued chain bytes verbatim — the `chain_head_for`
/// pin test in this file is the load-bearing assertion.
#[derive(Serialize)]
struct ChainHeadInput<'a> {
    batch_index: u64,
    integrated_time: i64,
    entries: &'a [AnchorEntry],
    previous_head: &'a str,
}

/// Canonical bytes for an `AnchorBatch` — the input to the chain hash.
///
/// Implementation strategy: project the batch into `ChainHeadInput`
/// (which omits `witnesses` — see that struct's docs), serialize the
/// projection through `serde_json` (which honors `#[serde(rename_all)]`
/// and `#[serde(skip_serializing_if)]` on `AnchorEntry`), then re-emit
/// those bytes through the same canonical-JSON pipeline
/// `PubkeyBundle::deterministic_hash` uses — keys sorted lex, no
/// whitespace, numbers as `Number::to_string`, strings JSON-escaped.
/// One implementation, one set of pin tests, one source of truth.
///
/// JSON-escaping defends against splicing: if `log_id` ever contained an
/// embedded newline or quote, JSON's escape rules render those bytes
/// unambiguously, so two batches that differ in log_id contents cannot
/// produce the same canonical bytes.
///
/// `#[deny(unused_variables)]` makes the destructure inside load-bearing:
/// a future contributor can't silence the unused-binding warning with
/// `let _ = batch.new_field;` or `#[allow(unused)]`. Adding a new field
/// to `AnchorBatch` MUST land here as either a positional binding (joins
/// `ChainHeadInput`, changes the head) or `field: _` (intentionally
/// excluded). No silent third option.
#[deny(unused_variables)]
fn canonical_chain_batch_body(batch: &AnchorBatch) -> TrustResult<Vec<u8>> {
    // Exhaustive field-audit destructure: rust requires every field of
    // `AnchorBatch` to be named here, so adding a new field to
    // `AnchorBatch` is a compile-fail at THIS site — forcing the next
    // contributor to make a deliberate decision about whether the new
    // field joins `ChainHeadInput` (changes the head) or stays out of
    // it (does not). Without this, a new field would silently land in
    // the canonical body via `serde_json::to_value(&view)` without any
    // signal at the chain-head contract layer.
    //
    // Match ergonomics auto-borrows from `&AnchorBatch`, so the
    // unprefixed bindings below all become `&Field` — no moves, no
    // explicit `ref` markers needed (Rust 2024 forbids them in
    // implicitly-borrowing patterns).
    let AnchorBatch {
        batch_index,
        integrated_time,
        entries,
        previous_head,
        // INTENTIONALLY OMITTED FROM ChainHeadInput. A witness signs
        // OVER chain_head_for(batch); if the head depended on
        // `witnesses`, signing would be impossible without infinite
        // regress. The `chain_head_invariant_under_witnesses` test
        // pins this property at runtime; this destructure pins it at
        // compile time.
        witnesses: _,
    } = batch;

    let view = ChainHeadInput {
        batch_index: *batch_index,
        integrated_time: *integrated_time,
        entries: entries.as_slice(),
        previous_head: previous_head.as_str(),
    };
    let v = serde_json::to_value(&view).map_err(|e| TrustError::Encoding(e.to_string()))?;
    crate::pubkey_bundle::canonical_json_bytes(&v)
}

/// 64-character lowercase hex of a 32-byte blake3 chain-head digest.
///
/// V1.13 wave-C-2 newtype around the canonical chain-head representation
/// produced by [`chain_head_for`]. Two purposes:
///
///   1. **Type discipline at the call site.** Without the newtype,
///      `verify_witness_against_roster(witness, batch.previous_head.as_str(), ..)`
///      and `verify_witness_against_roster(witness, chain_head_for(batch)?.as_str(), ..)`
///      have indistinguishable signatures, even though the semantic
///      ("a previously-recorded head from the wire" vs "the head we
///      just recomputed") is materially different. With the newtype,
///      recomputed heads are typed as `ChainHeadHex` and wire fields
///      stay `String`, surfacing the boundary in the type system.
///
///   2. **Refactor safety.** A future change that moves
///      `chain_head_for` to return raw bytes would today silently
///      compile against any caller that used the `String` return as
///      arbitrary bytes (e.g. assigning to a `head: String` wire
///      field that expected hex). With the newtype, the hex/bytes
///      distinction is explicit at every boundary: ask for the hex
///      view via [`ChainHeadHex::as_str`] / [`Display`], ask for the
///      raw bytes via [`ChainHeadHex::to_bytes`].
///
/// Construction enforces shape (64 lowercase hex chars). Constructor
/// failure surfaces as `TrustError::Encoding`, mirroring the
/// `decode_chain_head` failure path. Direct production-side
/// construction is internal to this module — `chain_head_for` is the
/// canonical producer and bypasses validation because hex-encoding a
/// 32-byte blake3 digest always yields exactly 64 lowercase hex chars
/// by construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChainHeadHex(String);

impl ChainHeadHex {
    /// Construct from a wire-side hex string after validating shape.
    /// Used by callers that read a hex head from JSON or another
    /// untrusted source. Returns `TrustError::Encoding` if the input
    /// is not exactly 64 lowercase hex chars.
    pub fn new(hex: String) -> TrustResult<Self> {
        if hex.len() != 64 {
            return Err(TrustError::Encoding(format!(
                "ChainHeadHex must be 64 hex chars (32 bytes), got {} chars",
                hex.len()
            )));
        }
        // Lowercase-only: matches `chain_head_for`'s output and prevents
        // wire-side ambiguity (mixed case would let two byte-different
        // strings represent the same head).
        if !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(TrustError::Encoding(
                "ChainHeadHex must be all-lowercase hex (0-9, a-f)".to_string(),
            ));
        }
        Ok(Self(hex))
    }

    /// Borrow the underlying lowercase-hex string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decode to the raw 32-byte blake3 digest. Infallible by
    /// construction — the constructor enforces 64 lowercase hex chars,
    /// so `hex::decode` cannot fail and `try_into` cannot under-fill.
    pub fn to_bytes(&self) -> [u8; 32] {
        let raw = hex::decode(&self.0).expect("ChainHeadHex constructor invariant");
        raw.try_into().expect("ChainHeadHex constructor invariant")
    }

    /// Move out the underlying `String`. Used at the wire-emit boundary
    /// when assigning to `AnchorChain.head` / `AnchorBatch.previous_head`,
    /// which stay `String`-typed for serde compatibility.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for ChainHeadHex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<ChainHeadHex> for String {
    fn from(h: ChainHeadHex) -> String {
        h.0
    }
}

// Symmetric `PartialEq<&str>` / `PartialEq<String>` impls (both
// directions, see also the reverse-direction block below) exist
// PURELY for test-assertion ergonomics — `assert_eq!(head, "abc..")`
// reads naturally without forcing every test site onto `head.as_str()`.
//
// These impls are NOT a trust-domain primitive: they perform a plain
// `==` byte compare and MUST NOT be used to compare two trust-relevant
// hex heads on the verification path. For trust-boundary equality
// (e.g. verifying that `chain.head` matches a recomputed tip), use
// `crate::ct::ct_eq_str` against `head.as_str()` directly so a
// timing-side-channel cannot leak prefix-match length about the
// honest-issuer chain head. The verifier does this in `verify.rs`
// (search for `ct_eq_str(&chain.head, tip.as_str())`).
//
// The newtype's load-bearing job is at the function-signature boundary
// (preventing wire-side `String` from silently flowing into a
// recomputed-head slot); these PartialEq sugar impls do not weaken
// that property because callers wanting trust-domain equality already
// route through `ct_eq_str`.
impl PartialEq<str> for ChainHeadHex {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for ChainHeadHex {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for ChainHeadHex {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

// Reverse-direction comparisons so callers can write `assert_eq!(s, h)`
// where `s: &str | String` and `h: ChainHeadHex`. Without these, only
// `assert_eq!(h, s)` would compile and existing tests would need
// reordered arguments.
impl PartialEq<ChainHeadHex> for str {
    fn eq(&self, other: &ChainHeadHex) -> bool {
        self == other.0.as_str()
    }
}

impl PartialEq<ChainHeadHex> for &str {
    fn eq(&self, other: &ChainHeadHex) -> bool {
        *self == other.0.as_str()
    }
}

impl PartialEq<ChainHeadHex> for String {
    fn eq(&self, other: &ChainHeadHex) -> bool {
        self == &other.0
    }
}

/// Compute the chain head for an `AnchorBatch`.
///
/// `head_n = blake3(ANCHOR_CHAIN_DOMAIN || canonical_chain_batch_body(batch_n))`
///
/// The batch's `previous_head` field is part of the canonical body, so this
/// commits to the entire history transitively: tampering with any past
/// batch changes `head_{i}` for that batch, which changes `previous_head`
/// in `batch_{i+1}`, which changes `head_{i+1}`, and so on up to the tip.
/// Returns the [`ChainHeadHex`] newtype carrying the lowercase hex of the
/// 32-byte blake3 digest. See `ChainHeadHex` docs for the type-discipline
/// rationale (V1.13 wave-C-2).
pub fn chain_head_for(batch: &AnchorBatch) -> TrustResult<ChainHeadHex> {
    let body = canonical_chain_batch_body(batch)?;
    let mut hasher = Hasher::new();
    hasher.update(ANCHOR_CHAIN_DOMAIN);
    hasher.update(&body);
    // Hex-encoding a 32-byte digest always yields exactly 64 lowercase
    // hex chars — bypass `ChainHeadHex::new`'s validation on the hot path.
    let head = hex::encode(hasher.finalize().as_bytes());
    // Defensive guard: catches a future `hex` crate behavioural drift
    // (e.g. switching default casing, returning a non-64-char encoding
    // for a 32-byte input) immediately in dev/test rather than letting
    // a malformed `ChainHeadHex` flow downstream and silently break
    // the `decode_chain_head` round-trip. Cheap (length + lowercase
    // byte scan) and stripped from release builds.
    debug_assert_eq!(
        head.len(),
        64,
        "hex::encode(32 bytes) must yield 64 chars, got {}",
        head.len(),
    );
    debug_assert!(
        head.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        "hex::encode must yield lowercase-only hex; ChainHeadHex invariant broken",
    );
    Ok(ChainHeadHex(head))
}

/// Result of chain-internal verification (V1.7).
///
/// Returned by `verify_anchor_chain`. `ok = errors.is_empty()`. Carrying
/// the recomputed tip allows the verifier to surface it in
/// `VerifyOutcome::evidence` for auditor display.
#[derive(Debug, Clone)]
pub struct ChainVerifyOutcome {
    /// Did all chain-internal checks pass?
    pub ok: bool,
    /// Number of batches walked.
    pub batches_walked: usize,
    /// Recomputed tip (head of the final successfully-walked batch),
    /// for evidence display. `None` if walking aborted before the
    /// first batch produced a head. Typed as `ChainHeadHex` (V1.13
    /// wave-C-2) to surface that this is a freshly-recomputed head,
    /// not a wire-side value from `chain.head`.
    pub recomputed_head: Option<ChainHeadHex>,
    /// Per-batch error strings; empty on full success.
    pub errors: Vec<String>,
}

/// Verify the internal consistency of an `AnchorChain`.
///
/// Checks, in order:
///   1. `history` is non-empty (the issuer never emits empty chains).
///   2. For each batch at index `i`:
///      - `batch_index == i` (no gaps, skips, or duplicates).
///      - `previous_head == chain_head_for(history[i-1])`, or the
///        all-zero genesis sentinel for `i == 0`.
///   3. `chain.head == chain_head_for(history.last())` — the
///      convenience tip is never trusted as a shortcut; the verifier
///      always recomputes from `history`.
///
/// All checks are independent of network state. The trust property
/// holds against any auditor with the verifier code and the chain
/// bytes — no log-side cooperation needed.
///
/// Cross-coverage between `trace.anchors` and chain entries is enforced
/// at the `verify_trace_with` layer (it requires both fields), not
/// here. This function answers only "is this chain self-consistent?".
pub fn verify_anchor_chain(chain: &AnchorChain) -> ChainVerifyOutcome {
    let mut errors = Vec::new();
    let mut recomputed_head: Option<ChainHeadHex> = None;

    if chain.history.is_empty() {
        errors.push(
            "anchor_chain.history is empty; issuer never emits empty chains".to_string(),
        );
        return ChainVerifyOutcome {
            ok: false,
            batches_walked: 0,
            recomputed_head,
            errors,
        };
    }

    let mut expected_prev = ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD.to_string();
    let mut batches_walked = 0usize;
    for (i, batch) in chain.history.iter().enumerate() {
        // Sequential index check — gaps, skips, duplicates all rejected.
        if batch.batch_index != i as u64 {
            errors.push(format!(
                "anchor_chain: batch[{i}] has batch_index={}, expected {}",
                batch.batch_index, i,
            ));
        }

        // previous_head links to predecessor's recomputed head (or the
        // genesis sentinel for the first batch). Constant-time compare
        // so a side-channel attacker cannot probe partial matches.
        //
        // STOP walking on mismatch: once the link is broken,
        // `expected_prev` is no longer the "honest" head, and continuing
        // would let an attacker hide a coordinated multi-batch rewrite
        // (where batches `i` and `i+1` are both substituted with a
        // consistent internal link) behind a single error report. We
        // surface the first break and refuse to keep walking.
        if !crate::ct::ct_eq_str(&batch.previous_head, &expected_prev) {
            errors.push(format!(
                "anchor_chain: batch[{i}] previous_head mismatch: claimed={}, expected={}",
                batch.previous_head, expected_prev,
            ));
            break;
        }

        match chain_head_for(batch) {
            Ok(head) => {
                // `expected_prev` mirrors the wire-side `previous_head`
                // shape (String) for the next iteration's ct compare;
                // recomputed_head retains the typed newtype for the
                // tip-equality check + evidence display.
                expected_prev = head.as_str().to_string();
                recomputed_head = Some(head);
                batches_walked += 1;
            }
            Err(e) => {
                errors.push(format!(
                    "anchor_chain: batch[{i}] head computation failed: {e}"
                ));
                // Stop walking; subsequent previous_head checks would
                // be meaningless without a recomputed predecessor.
                break;
            }
        }
    }

    // Tip check is meaningful only when the full history walked
    // cleanly. A short walk's tip is the head of wherever we stopped,
    // not the chain tip — comparing it to chain.head would surface a
    // confusing second error on top of the link/index errors that
    // caused the early break.
    if batches_walked == chain.history.len() {
        if let Some(tip) = &recomputed_head {
            if !crate::ct::ct_eq_str(&chain.head, tip.as_str()) {
                errors.push(format!(
                    "anchor_chain: convenience head mismatch (chain.head={}, recomputed from history={})",
                    chain.head, tip,
                ));
            }
        }
    }

    ChainVerifyOutcome {
        ok: errors.is_empty(),
        batches_walked,
        recomputed_head,
        errors,
    }
}

/// Extract the C2SP signed-note signature line that matches `expected_origin`,
/// returning the standard-base64 signature blob (`base64(4-byte BE keyID ||
/// DER ECDSA sig)`) ready to drop into `InclusionProof::checkpoint_sig`.
///
/// The Rekor REST API returns the FULL signed-note in
/// `verification.inclusionProof.checkpoint`. The verifier's
/// `checkpoint_sig` field is just the base64 signature (the third token of
/// the matching signature line), so the issuer must extract it. Putting
/// the parser here (atlas-trust-core, not atlas-signer) keeps it in the
/// same crate as the verifier-side checkpoint canonicalisation — one
/// place, one set of pin tests, one source of truth for the C2SP format.
///
/// The format (per C2SP signed-note v1) is:
///
/// ```text
/// <origin> - <tree-id>          <- line 1: signed body header
/// <tree-size>                   <- line 2
/// <base64(rootHash)>            <- line 3
///                               <- line 4: blank separator
/// — <name1> <base64-sig1>       <- line 5+: one or more signature lines
/// — <name2> <base64-sig2>
/// …
/// ```
///
/// Each signature line begins with U+2014 (em-dash) + U+0020 (space).
/// Splitting the remainder on ASCII whitespace yields `[name, base64_sig]`.
/// We pick the line whose `name` equals `expected_origin` (Rekor signs
/// each shard's checkpoints with its origin as the signer name).
///
/// **Defensive checks**:
/// - Returns an error if the blank-line separator is missing.
/// - Returns an error if no signature line matches `expected_origin`.
/// - Returns an error if the matched line cannot be split into exactly
///   three whitespace-separated tokens.
/// - Returns an error if the base64 cannot be decoded under STANDARD
///   base64 (RFC 4648 §4 with `=` padding — same encoding the verifier
///   uses; reject deviations).
/// - Returns an error if the decoded signature is shorter than `4 + 8`
///   bytes (need a 4-byte keyID + a minimum-length DER ECDSA signature).
/// - If `expected_keyid` is `Some`, returns an error when the first 4
///   bytes of the decoded signature do not match. The verifier re-checks
///   this; failing fast at issue time produces a clearer error than
///   waiting for the verifier to reject the trace later.
pub fn extract_signature_line_sigstore(
    checkpoint: &str,
    expected_origin: &str,
    expected_keyid: Option<&[u8; 4]>,
) -> Result<String, String> {
    // C2SP requires the body and signature lines to be separated by a
    // single blank line. `\n\n` is therefore the only valid separator;
    // `\r\n\r\n` would imply the issuer added CRLF, which is not C2SP.
    let (_body, sig_block) = checkpoint
        .split_once("\n\n")
        .ok_or_else(|| "checkpoint missing blank-line separator between body and signatures".to_string())?;

    if sig_block.is_empty() {
        return Err("checkpoint has no signature lines after the blank separator".to_string());
    }

    // C2SP signature line marker: em-dash (U+2014) + space.
    const SIG_PREFIX: &str = "\u{2014} ";

    let mut last_err: Option<String> = None;
    for line in sig_block.lines() {
        if line.is_empty() {
            // Trailing blank line is permitted; skip.
            continue;
        }
        let Some(rest) = line.strip_prefix(SIG_PREFIX) else {
            // Some signed-notes carry comments after the signatures;
            // skip lines that do not start with the C2SP marker rather
            // than rejecting the whole document.
            continue;
        };
        // Two ASCII whitespace-separated tokens: name and base64 sig.
        // splitn(2, char) splits exactly once, so a single space suffices.
        let mut parts = rest.splitn(2, ' ');
        let name = match parts.next() {
            Some(n) if !n.is_empty() => n,
            _ => {
                last_err = Some(format!(
                    "malformed signature line (missing name): {line:?}"
                ));
                continue;
            }
        };
        let sig_b64 = match parts.next() {
            Some(s) if !s.is_empty() => s,
            _ => {
                last_err = Some(format!(
                    "malformed signature line (missing base64 token): {line:?}"
                ));
                continue;
            }
        };
        if name != expected_origin {
            // Different signer (e.g. an additional witness) — skip.
            continue;
        }

        // Strict standard base64 — same dialect the verifier expects.
        let raw = base64::engine::general_purpose::STANDARD
            .decode(sig_b64.as_bytes())
            .map_err(|e| format!("signature line is not standard base64: {e}"))?;
        if raw.len() < 4 + 8 {
            return Err(format!(
                "signature blob too short: {} bytes (need 4-byte keyID + DER ECDSA sig)",
                raw.len(),
            ));
        }
        if let Some(expected) = expected_keyid {
            let mut keyid = [0u8; 4];
            keyid.copy_from_slice(&raw[..4]);
            if &keyid != expected {
                // Same origin, different keyID. This is the
                // key-rotation-overlap window: a checkpoint may carry
                // signatures from both the rotating-out and rotating-in
                // keys for one tick. Skip-and-remember rather than
                // return Err so a later matching-keyID line for the
                // same origin can still succeed. The last_err captures
                // the rejection so the final "no match" error names
                // what we actually saw.
                last_err = Some(format!(
                    "signature keyID {} does not match expected log keyID {} \
                     (same origin {:?}, different key — possibly a rotation \
                     witness; ignoring)",
                    hex::encode(keyid),
                    hex::encode(expected),
                    expected_origin,
                ));
                continue;
            }
        }
        return Ok(sig_b64.to_string());
    }

    Err(last_err.unwrap_or_else(|| {
        format!(
            "checkpoint contains no signature line for origin {:?}",
            expected_origin,
        )
    }))
}

/// Parse the active tree-ID off the first line of a C2SP signed-note
/// body: `<origin> - <tree-id>\n…`. Errors on any deviation from that
/// exact shape so a truncated, reformatted, or wrong-origin checkpoint
/// fails loud rather than supplying a default tree-id the verifier
/// would later reject (yielding an opaque "tree mismatch" instead of
/// the clearer parse error).
///
/// Lives here in `atlas-trust-core` (next to the other C2SP parsers)
/// so issuer and verifier read the same canonical interpretation; a
/// drift in the issuer-side parser would otherwise silently feed the
/// verifier a tree-id the issuer never actually saw.
pub fn parse_sigstore_checkpoint_tree_id(checkpoint: &str) -> Result<i64, String> {
    let first_line = checkpoint
        .lines()
        .next()
        .ok_or_else(|| "checkpoint has no lines".to_string())?;
    let (origin, tree_id_str) = first_line
        .rsplit_once(" - ")
        .ok_or_else(|| format!("checkpoint first line missing ' - ' separator: {first_line:?}"))?;
    if origin != SIGSTORE_REKOR_V1_ORIGIN {
        return Err(format!(
            "checkpoint first-line origin {origin:?} does not match the pinned Sigstore origin {:?}",
            SIGSTORE_REKOR_V1_ORIGIN,
        ));
    }
    tree_id_str
        .parse::<i64>()
        .map_err(|e| format!("checkpoint tree-id is not a valid i64: {e} ({tree_id_str:?})"))
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

    // C2SP signed-note signatures use RFC 4648 §4 STANDARD base64 with
    // `=` padding. Reject anything else — the format is exact, not
    // permissive, and a deviating encoding signals non-Sigstore data.
    let raw = base64::engine::general_purpose::STANDARD
        .decode(proof.checkpoint_sig.as_bytes())
        .map_err(|e| format!("checkpoint_sig is not standard base64: {e}"))?;

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
///   - `kind` MUST equal `"hashedrekord"` (other Rekor types are not
///     issued by Atlas in V1.6).
///   - `spec.data.hash.algorithm` MUST equal `"sha256"`. The schema
///     also accepts sha1/sha384/sha512, but Atlas only ever submits
///     sha256-content via Phase 2's hashedrekord builder; pinning the
///     algorithm rules out an attacker forging a hashedrekord whose
///     `value` happens to collide with an `anchored_hash` interpreted
///     under a weaker hash function.
///   - `spec.data.hash.value` (lowercase hex) MUST equal `anchored_hash`.
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

    // Pin apiVersion explicitly. Future hashedrekord schema versions could
    // re-shape spec.data.hash with the same field names but different
    // semantics — without this gate, the value-equality check below would
    // silently succeed against a body whose meaning has shifted under our
    // feet. V1.7 may admit additional pinned versions; until then, any
    // body that is not hashedrekord/v0.0.1 is rejected loud.
    let api_version = v
        .get("apiVersion")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "entry body has no apiVersion field".to_string())?;
    if api_version != "0.0.1" {
        return Err(format!(
            "entry body apiVersion {api_version:?} is not supported by V1.6 (hashedrekord/v0.0.1 only)"
        ));
    }

    let hash_obj = v
        .get("spec")
        .and_then(|s| s.get("data"))
        .and_then(|d| d.get("hash"))
        .ok_or_else(|| "entry body has no spec.data.hash object".to_string())?;

    let algorithm = hash_obj
        .get("algorithm")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "entry body has no spec.data.hash.algorithm field".to_string())?;
    if algorithm != "sha256" {
        return Err(format!(
            "entry body hash algorithm {algorithm:?} is not supported (V1.6 pins sha256)"
        ));
    }

    let body_hash = hash_obj
        .get("value")
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
        // log_id is 64 hex chars (= 32 SHA-256 bytes), and equals the
        // public log identity Rekor returns in `entry.logID`. Pinning
        // the value catches accidental edits to SIGSTORE_REKOR_V1_PEM.
        let log_id = &*SIGSTORE_REKOR_V1_LOG_ID;
        assert_eq!(
            log_id,
            "c0d23d6ad406973f9559f3ba2d1ca01f84147d8ffc5b8445c224f98b9591801d",
        );
        // C2SP keyID is the first 4 bytes of the log_id, hex-encoded
        // back to the same prefix.
        let keyid_hex = hex::encode(*SIGSTORE_REKOR_V1_KEY_ID);
        assert_eq!(&log_id[..8], keyid_hex);
    }

    /// `entry_body_binds_anchored_hash` must reject a hashedrekord whose
    /// hash algorithm is not sha256 (e.g. sha1 or sha512), even when the
    /// `value` happens to equal `anchored_hash`. This rules out a forgery
    /// where the attacker chooses a weaker algorithm and crafts a value
    /// collision under that algorithm.
    #[test]
    fn entry_body_with_non_sha256_algorithm_is_rejected() {
        let body = serde_json::json!({
            "apiVersion": "0.0.1",
            "kind": "hashedrekord",
            "spec": {
                "data": {
                    "hash": {
                        "algorithm": "sha512",
                        "value": "deadbeef",
                    }
                }
            }
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();
        let err = entry_body_binds_anchored_hash(&body_bytes, "deadbeef")
            .expect_err("non-sha256 hashedrekord must be rejected");
        assert!(err.contains("sha256"), "error must mention sha256 pin: {err}");
    }

    /// `entry_body_binds_anchored_hash` must reject a hashedrekord whose
    /// `apiVersion` is not "0.0.1". A future schema bump might preserve the
    /// `kind`+`spec.data.hash` field names but redefine the hash semantics
    /// (e.g. switch from artifact-hash to digest-of-canonical-body); without
    /// this gate the value-equality check would silently accept the new
    /// shape.
    #[test]
    fn entry_body_with_wrong_api_version_is_rejected() {
        let body = serde_json::json!({
            "apiVersion": "0.0.2",
            "kind": "hashedrekord",
            "spec": {
                "data": {
                    "hash": {
                        "algorithm": "sha256",
                        "value": "deadbeef",
                    }
                }
            }
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();
        let err = entry_body_binds_anchored_hash(&body_bytes, "deadbeef")
            .expect_err("non-v0.0.1 hashedrekord must be rejected");
        assert!(
            err.contains("apiVersion") && err.contains("0.0.1"),
            "error must call out apiVersion pin: {err}",
        );
    }

    /// `entry_body_binds_anchored_hash` must reject a hashedrekord that
    /// omits the `apiVersion` field entirely. Falling back to "ok" when
    /// the field is missing would re-introduce the same drift surface
    /// the explicit pin closes.
    #[test]
    fn entry_body_with_missing_api_version_is_rejected() {
        let body = serde_json::json!({
            "kind": "hashedrekord",
            "spec": {
                "data": {
                    "hash": {
                        "algorithm": "sha256",
                        "value": "deadbeef",
                    }
                }
            }
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();
        let err = entry_body_binds_anchored_hash(&body_bytes, "deadbeef")
            .expect_err("hashedrekord without apiVersion must be rejected");
        assert!(
            err.contains("apiVersion"),
            "error must mention apiVersion: {err}",
        );
    }

    /// `entry_body_binds_anchored_hash` must reject a hashedrekord with
    /// no `algorithm` field — the hashedrekord schema requires it, and
    /// silently skipping the algorithm check (treating "missing" as
    /// "ok") would let a malformed body slip past.
    #[test]
    fn entry_body_with_missing_algorithm_is_rejected() {
        let body = serde_json::json!({
            "apiVersion": "0.0.1",
            "kind": "hashedrekord",
            "spec": {
                "data": {
                    "hash": {
                        "value": "deadbeef",
                    }
                }
            }
        });
        let body_bytes = serde_json::to_vec(&body).unwrap();
        let err = entry_body_binds_anchored_hash(&body_bytes, "deadbeef")
            .expect_err("hashedrekord without algorithm must be rejected");
        assert!(err.contains("algorithm"), "error must mention algorithm: {err}");
    }

    /// `canonical_checkpoint_bytes_sigstore` must reject origins that
    /// contain the C2SP separators. An attacker-controlled origin
    /// containing `\n` could splice an extra line into the signed body;
    /// one containing ` - ` could pin a different tree-id than claimed.
    #[test]
    fn checkpoint_bytes_sigstore_rejects_injection_in_origin() {
        let root_hex = "0000000000000000000000000000000000000000000000000000000000000000";
        let err_nl = canonical_checkpoint_bytes_sigstore("rekor.sigstore.dev\n42", 1, 1, root_hex)
            .expect_err("origin with newline must be rejected");
        assert!(err_nl.contains("newline"), "got: {err_nl}");

        let err_dash = canonical_checkpoint_bytes_sigstore("rekor.sigstore.dev - 42", 1, 1, root_hex)
            .expect_err("origin with ' - ' must be rejected");
        assert!(err_dash.contains("' - '"), "got: {err_dash}");

        let err_empty = canonical_checkpoint_bytes_sigstore("", 1, 1, root_hex)
            .expect_err("empty origin must be rejected");
        assert!(err_empty.contains("empty"), "got: {err_empty}");
    }

    /// Pin every output of `sigstore_anchored_hash_for`. Drift here is
    /// indistinguishable from a silent break of the trust property:
    /// every Sigstore-format issuer and verifier hashes through this
    /// function, and the two halves only meet via byte-for-byte
    /// equality. A change in prefix string, byte-vs-string ordering,
    /// or hash algorithm would split the issuer from the verifier
    /// without any other test catching it.
    ///
    /// Provenance: each expected value was independently re-derived
    /// at pin time with `printf '<prefix><blake3_hex>' | sha256sum`.
    /// An auditor can repeat the same command and confirm.
    #[test]
    fn sigstore_anchored_hash_for_is_pinned() {
        // All-zero blake3 — covers the "trivial input" case.
        let zeros = "0".repeat(64);
        assert_eq!(
            sigstore_anchored_hash_for(&AnchorKind::DagTip, &zeros),
            "893af4ccf889f69bed1a770b0bd0a66c71f5924f430f804eee18eae8e305a72a",
        );
        assert_eq!(
            sigstore_anchored_hash_for(&AnchorKind::BundleHash, &zeros),
            "f973e87c35ee55ed6a2db553291cd479b2e20b050704bacb7c0b52d5a9fec437",
        );

        // Non-trivial blake3 — distinct nibbles to catch byte-order
        // regressions in the SHA-256 update path.
        let mixed = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        assert_eq!(
            sigstore_anchored_hash_for(&AnchorKind::DagTip, mixed),
            "b90e10ef48f535c10de4dc3ea040fb055ad6b60cc5c5e9d4df2f41bfa0ee0df6",
        );
        assert_eq!(
            sigstore_anchored_hash_for(&AnchorKind::BundleHash, mixed),
            "74d4b1fedf3030f735e46b73ace6622fc6bae0ffd5364b0faf023bc549bdf33c",
        );

        // Domain separation: same blake3, different kind ⇒ different
        // SHA-256. Defence in depth against an adversary who manages
        // to collide a tip and a bundle hash upstream.
        assert_ne!(
            sigstore_anchored_hash_for(&AnchorKind::DagTip, mixed),
            sigstore_anchored_hash_for(&AnchorKind::BundleHash, mixed),
            "tip and bundle prefixes must yield different anchored hashes",
        );
    }

    /// `sigstore_artifact_bytes_for` is the issuer-side counterpart of
    /// `sigstore_anchored_hash_for`. The two MUST stay in lockstep:
    /// the SHA-256 of the artifact bytes is exactly the anchored hash.
    /// This test pins both shapes and the hash relationship explicitly,
    /// so a future edit cannot silently desynchronise them.
    #[test]
    fn sigstore_artifact_bytes_for_is_pinned() {
        use sha2::{Digest, Sha256};

        let mixed = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

        // Exact byte layout: prefix (ASCII), then blake3 hex (ASCII).
        let tip_bytes = sigstore_artifact_bytes_for(&AnchorKind::DagTip, mixed);
        assert_eq!(
            tip_bytes,
            b"atlas-dag-tip-v1:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        );

        let bundle_bytes = sigstore_artifact_bytes_for(&AnchorKind::BundleHash, mixed);
        assert_eq!(
            bundle_bytes,
            b"atlas-bundle-hash-v1:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        );

        // Lockstep check: SHA-256(artifact_bytes) MUST equal the
        // anchored hash hex-decoded. If either helper is edited
        // independently this assertion catches the drift.
        let tip_digest = hex::encode(Sha256::digest(&tip_bytes));
        assert_eq!(
            tip_digest,
            sigstore_anchored_hash_for(&AnchorKind::DagTip, mixed),
            "sha256(artifact_bytes) MUST equal sigstore_anchored_hash_for output",
        );
        let bundle_digest = hex::encode(Sha256::digest(&bundle_bytes));
        assert_eq!(
            bundle_digest,
            sigstore_anchored_hash_for(&AnchorKind::BundleHash, mixed),
            "sha256(artifact_bytes) MUST equal sigstore_anchored_hash_for output",
        );
    }

    /// `extract_signature_line_sigstore` parses a synthetic C2SP signed-
    /// note and returns exactly the base64 signature blob for the origin
    /// the caller asked for. Pinning concrete inputs catches whitespace
    /// drift, line-separator regressions, and prefix-marker errors.
    #[test]
    fn extract_signature_line_sigstore_returns_matching_origin_sig() {
        // 4-byte keyID = [0xCA, 0xFE, 0xBA, 0xBE] + 8 bytes of DER stand-in.
        // (Real signatures are longer but we only need ≥ 4+8 to pass the
        // length check; the issuer is responsible for ensuring it submitted
        // a real DER ECDSA sig.)
        let raw = [
            0xCAu8, 0xFE, 0xBA, 0xBE, // keyID
            0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02, // 8-byte DER stub
        ];
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(raw);

        let checkpoint = format!(
            "rekor.sigstore.dev - 42\n7\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n\n\u{2014} rekor.sigstore.dev {sig_b64}\n",
        );

        let key_id = [0xCAu8, 0xFE, 0xBA, 0xBE];
        let extracted = extract_signature_line_sigstore(
            &checkpoint,
            "rekor.sigstore.dev",
            Some(&key_id),
        )
        .expect("matching origin must extract");
        assert_eq!(extracted, sig_b64);
    }

    /// Multiple signature lines: pick the one whose name matches the
    /// requested origin and ignore lines authored by other witnesses.
    #[test]
    fn extract_signature_line_sigstore_skips_non_matching_origins() {
        let raw_a = {
            let mut v = vec![0xAA, 0xAA, 0xAA, 0xAA];
            v.extend_from_slice(&[0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02]);
            v
        };
        let raw_b = {
            let mut v = vec![0xBB, 0xBB, 0xBB, 0xBB];
            v.extend_from_slice(&[0x30, 0x06, 0x02, 0x01, 0x03, 0x02, 0x01, 0x04]);
            v
        };
        let sig_a = base64::engine::general_purpose::STANDARD.encode(&raw_a);
        let sig_b = base64::engine::general_purpose::STANDARD.encode(&raw_b);
        let checkpoint = format!(
            "rekor.sigstore.dev - 42\n7\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n\n\
             \u{2014} witness.example.com {sig_a}\n\
             \u{2014} rekor.sigstore.dev {sig_b}\n",
        );
        let extracted = extract_signature_line_sigstore(
            &checkpoint,
            "rekor.sigstore.dev",
            Some(&[0xBB, 0xBB, 0xBB, 0xBB]),
        )
        .expect("matching origin must be found among multi-signer body");
        assert_eq!(extracted, sig_b);
    }

    /// keyID enforcement: a signature whose first 4 bytes don't match
    /// the expected keyID is rejected — defends against a neighbouring
    /// signer line being authored by a different log.
    #[test]
    fn extract_signature_line_sigstore_rejects_keyid_mismatch() {
        let raw = {
            let mut v = vec![0x11, 0x22, 0x33, 0x44];
            v.extend_from_slice(&[0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02]);
            v
        };
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(raw);
        let checkpoint = format!(
            "rekor.sigstore.dev - 42\n7\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n\n\u{2014} rekor.sigstore.dev {sig_b64}\n",
        );
        let err = extract_signature_line_sigstore(
            &checkpoint,
            "rekor.sigstore.dev",
            Some(&[0x55, 0x66, 0x77, 0x88]),
        )
        .expect_err("keyID mismatch must be rejected");
        assert!(err.contains("keyID"), "got: {err}");
    }

    /// Key-rotation overlap: a checkpoint with two same-origin signature
    /// lines (rotating-out keyID first, rotating-in keyID second) must
    /// succeed by scanning past the mismatching first line. Rejecting
    /// fast on the first mismatch would lock Atlas verifiers out of any
    /// real-world Sigstore key rotation that overlaps two trees.
    #[test]
    fn extract_signature_line_sigstore_skips_keyid_mismatch_within_origin() {
        let raw_old = {
            let mut v = vec![0x11, 0x22, 0x33, 0x44];
            v.extend_from_slice(&[0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02]);
            v
        };
        let raw_new = {
            let mut v = vec![0x55, 0x66, 0x77, 0x88];
            v.extend_from_slice(&[0x30, 0x06, 0x02, 0x01, 0x03, 0x02, 0x01, 0x04]);
            v
        };
        let sig_old = base64::engine::general_purpose::STANDARD.encode(&raw_old);
        let sig_new = base64::engine::general_purpose::STANDARD.encode(&raw_new);
        let checkpoint = format!(
            "rekor.sigstore.dev - 42\n7\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n\n\
             \u{2014} rekor.sigstore.dev {sig_old}\n\
             \u{2014} rekor.sigstore.dev {sig_new}\n",
        );
        let extracted = extract_signature_line_sigstore(
            &checkpoint,
            "rekor.sigstore.dev",
            Some(&[0x55, 0x66, 0x77, 0x88]),
        )
        .expect("matching keyID line must be found past the rotated-out line");
        assert_eq!(extracted, sig_new);
    }

    /// Missing blank-line separator: not a valid C2SP signed-note.
    #[test]
    fn extract_signature_line_sigstore_rejects_missing_separator() {
        let err = extract_signature_line_sigstore(
            "rekor.sigstore.dev - 42\n7\nAAAA=\n\u{2014} rekor.sigstore.dev abcd",
            "rekor.sigstore.dev",
            None,
        )
        .expect_err("no blank line ⇒ reject");
        assert!(err.contains("blank-line separator"), "got: {err}");
    }

    /// No matching origin line: must error out, not silently return
    /// some other line's signature.
    #[test]
    fn extract_signature_line_sigstore_rejects_no_match() {
        let raw = {
            let mut v = vec![0xAA, 0xAA, 0xAA, 0xAA];
            v.extend_from_slice(&[0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02]);
            v
        };
        let sig = base64::engine::general_purpose::STANDARD.encode(raw);
        let checkpoint = format!(
            "rekor.sigstore.dev - 42\n7\nAAAA=\n\n\u{2014} witness.example.com {sig}\n",
        );
        let err = extract_signature_line_sigstore(
            &checkpoint,
            "rekor.sigstore.dev",
            None,
        )
        .expect_err("no matching origin ⇒ reject");
        assert!(err.contains("no signature line"), "got: {err}");
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

    /// Roster pin: V1.7 trusts exactly the active shard plus the two
    /// known historical shards. Any change to this set is a deliberate
    /// trust-property change and must be a source edit, not silent
    /// drift; this test forces the change to surface in code review.
    #[test]
    fn sigstore_tree_id_roster_is_pinned() {
        assert_eq!(
            SIGSTORE_REKOR_V1_TREE_IDS,
            &[
                1_193_050_959_916_656_506_i64,
                3_904_496_407_287_907_110_i64,
                2_605_736_670_972_794_746_i64,
            ],
            "SIGSTORE_REKOR_V1_TREE_IDS roster changed — review the trust property",
        );
        // The active-shard constant must be the first roster member: the
        // issuer always submits to the active shard, so verifier roster
        // and issuer choice cannot diverge silently.
        assert_eq!(
            SIGSTORE_REKOR_V1_TREE_IDS[0], SIGSTORE_REKOR_V1_ACTIVE_TREE_ID,
            "active tree-ID must lead the roster — issuer/verifier alignment invariant",
        );
    }

    /// `is_known_sigstore_rekor_v1_tree_id` accepts every roster member
    /// and rejects a value adjacent to a real shard (catches an
    /// off-by-one in the membership scan) and an obviously-bogus value.
    #[test]
    fn known_sigstore_tree_id_membership() {
        for &t in SIGSTORE_REKOR_V1_TREE_IDS {
            assert!(
                is_known_sigstore_rekor_v1_tree_id(t),
                "roster member {t} must be accepted",
            );
        }
        // One off from the active shard — must reject.
        assert!(!is_known_sigstore_rekor_v1_tree_id(
            SIGSTORE_REKOR_V1_ACTIVE_TREE_ID + 1
        ));
        // Negative — must reject.
        assert!(!is_known_sigstore_rekor_v1_tree_id(-1));
        // Zero — must reject.
        assert!(!is_known_sigstore_rekor_v1_tree_id(0));
        // A small unrelated value — must reject.
        assert!(!is_known_sigstore_rekor_v1_tree_id(1_234_567_890));
    }

    // ────────────────────────────────────────────────────────────────────
    // Anchor-chain head pin tests (V1.7)
    //
    // Cross-implementation goldens. The chain head is the load-bearing
    // hash for V1.7's anti-rewrite property: silent mutation of any past
    // batch must change the head and break verification. These pins
    // anchor the wire format so a future canonicalization change trips
    // CI before it desyncs issuer from verifier.
    // ────────────────────────────────────────────────────────────────────

    /// Helper: minimal fixture batch covering both anchor kinds and one
    /// AnchorEntry per kind, with deterministic field values. Re-used by
    /// the byte-determinism, domain-separation, and previous-head pin
    /// tests so a single field tweak surfaces in all of them at once.
    fn fixture_batch(batch_index: u64, previous_head: &str) -> AnchorBatch {
        AnchorBatch {
            batch_index,
            integrated_time: 1_745_000_000,
            witnesses: Vec::new(),
            entries: vec![AnchorEntry {
                kind: AnchorKind::DagTip,
                anchored_hash:
                    "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff".to_string(),
                log_id: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
                    .to_string(),
                log_index: 7,
                integrated_time: 1_745_000_000,
                inclusion_proof: InclusionProof {
                    tree_size: 8,
                    root_hash:
                        "cafebabe00000000cafebabe00000000cafebabe00000000cafebabe00000000"
                            .to_string(),
                    hashes: vec![
                        "1111111111111111111111111111111111111111111111111111111111111111"
                            .to_string(),
                        "2222222222222222222222222222222222222222222222222222222222222222"
                            .to_string(),
                    ],
                    checkpoint_sig: "AAAA".to_string(),
                },
                entry_body_b64: None,
                tree_id: None,
            }],
            previous_head: previous_head.to_string(),
        }
    }

    /// Pin the canonical-bytes output of `canonical_chain_batch_body`
    /// for the genesis fixture. If `serde_json::to_value` ever changes
    /// how it serializes the batch (field name, number formatting,
    /// `skip_serializing_if` behavior) or if `canonical_json_bytes`
    /// changes its sort/whitespace rules, this test trips immediately.
    #[test]
    fn chain_canonical_body_byte_determinism_pin() {
        let batch = fixture_batch(0, crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        let body = canonical_chain_batch_body(&batch).unwrap();

        // BEGIN PINNED — DO NOT EDIT WITHOUT INTENT.
        // Canonical JSON of the genesis fixture: top-level keys sorted
        // (batch_index, entries, integrated_time, previous_head); each
        // entry's keys sorted (anchored_hash, inclusion_proof, kind,
        // log_id, log_index, integrated_time); inclusion_proof keys
        // sorted (checkpoint_sig, hashes, root_hash, tree_size). No
        // whitespace, no trailing newline. `entry_body_b64` and
        // `tree_id` are absent (Option::None + skip_serializing_if).
        let expected = r#"{"batch_index":0,"entries":[{"anchored_hash":"00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff","inclusion_proof":{"checkpoint_sig":"AAAA","hashes":["1111111111111111111111111111111111111111111111111111111111111111","2222222222222222222222222222222222222222222222222222222222222222"],"root_hash":"cafebabe00000000cafebabe00000000cafebabe00000000cafebabe00000000","tree_size":8},"integrated_time":1745000000,"kind":"dag_tip","log_id":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef","log_index":7}],"integrated_time":1745000000,"previous_head":"0000000000000000000000000000000000000000000000000000000000000000"}"#;
        // END PINNED.

        assert_eq!(
            std::str::from_utf8(&body).unwrap(),
            expected,
            "anchor-chain canonical body wire-format drift. If \
             intentional, update the pinned string AND bump \
             atlas-trust-core's crate version so VERIFIER_VERSION \
             cascades to old-format chains."
        );
    }

    /// Pin `chain_head_for` for the genesis fixture. This is the
    /// blake3 of `ANCHOR_CHAIN_DOMAIN || canonical_chain_batch_body`.
    /// An auditor can re-derive: `printf 'atlas-anchor-chain-v1:<body>'
    /// | b3sum`.
    #[test]
    fn chain_head_for_byte_determinism_pin() {
        let batch = fixture_batch(0, crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        let head = chain_head_for(&batch).unwrap();

        // BEGIN PINNED — DO NOT EDIT WITHOUT INTENT.
        // blake3("atlas-anchor-chain-v1:" || canonical_body) hex.
        let expected = "b3a7c15ad1d47a5e324d431501a20bd5defa78b734ef14bb612e1c0f2f5ddfd6";
        // END PINNED.

        assert_eq!(
            head, expected,
            "anchor-chain head wire-format drift. Issuer/verifier byte \
             agreement on this hash is the V1.7 trust property."
        );
    }

    /// Domain-separation prefix sanity check. We verify the easiest
    /// case: blake3 of the canonical body WITHOUT the
    /// `ANCHOR_CHAIN_DOMAIN` prefix differs from `chain_head_for`. This
    /// alone does not prove cross-system collision resistance against
    /// other blake3 inputs in the codebase (event signing inputs, leaf
    /// hashes, bundle hashes), but it does prove the prefix is part of
    /// the input — the load-bearing precondition for cross-system
    /// separation given that those other inputs use distinct prefixes.
    #[test]
    fn chain_head_includes_domain_prefix() {
        let batch = fixture_batch(0, crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        let head = chain_head_for(&batch).unwrap();

        let body = canonical_chain_batch_body(&batch).unwrap();
        let undomained = hex::encode(blake3::hash(&body).as_bytes());

        assert_ne!(
            head, undomained,
            "chain head must include ANCHOR_CHAIN_DOMAIN prefix; \
             dropping it would expose the chain hash to collision with \
             any other blake3-of-canonical-JSON value in the system",
        );
    }

    /// `previous_head` is part of the canonical body, so changing it
    /// (which is what an attacker rewriting history would have to do)
    /// MUST change the chain head. This is the load-bearing property
    /// for the entire chain construction.
    #[test]
    fn chain_head_changes_with_previous_head() {
        let h_genesis = chain_head_for(&fixture_batch(
            1,
            crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
        ))
        .unwrap();
        let h_other = chain_head_for(&fixture_batch(
            1,
            "0101010101010101010101010101010101010101010101010101010101010101",
        ))
        .unwrap();
        assert_ne!(
            h_genesis, h_other,
            "chain head must depend on previous_head; otherwise \
             rewriting the predecessor would not change the head",
        );
    }

    /// Same fixture, different `batch_index`. The index is part of the
    /// canonical body, so the head must differ. Defends against an
    /// attacker who tries to splice a batch into a different position
    /// of the history.
    #[test]
    fn chain_head_changes_with_batch_index() {
        let h0 = chain_head_for(&fixture_batch(
            0,
            crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
        ))
        .unwrap();
        let h1 = chain_head_for(&fixture_batch(
            1,
            crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
        ))
        .unwrap();
        assert_ne!(h0, h1, "chain head must depend on batch_index");
    }

    /// V1.13 invariant: adding witnesses to an `AnchorBatch` MUST NOT
    /// change `chain_head_for(batch)`. A witness signs OVER the head;
    /// if the head depended on witnesses, signing would be impossible
    /// without infinite regress. The `ChainHeadInput` view in this file
    /// is the load-bearing mechanism for this invariance — this test
    /// catches any future change that accidentally adds `witnesses` to
    /// the canonicalisation.
    #[test]
    fn chain_head_invariant_under_witnesses() {
        use crate::witness::WitnessSig;

        let mut empty = fixture_batch(0, crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        let h_empty = chain_head_for(&empty).unwrap();

        empty.witnesses = vec![WitnessSig {
            witness_kid: "test-witness-1".to_string(),
            // 64-byte Ed25519 sig under URL_SAFE_NO_PAD = 86 chars
            // (matches the wire dialect; STANDARD-padded would be 88).
            signature: "A".repeat(86),
        }];
        let h_with_one = chain_head_for(&empty).unwrap();
        assert_eq!(
            h_empty, h_with_one,
            "chain head must NOT depend on witnesses; otherwise the \
             witness sig would be over a head that depends on itself \
             (infinite regress)",
        );

        empty.witnesses.push(WitnessSig {
            witness_kid: "test-witness-2".to_string(),
            signature: "B".repeat(86),
        });
        let h_with_two = chain_head_for(&empty).unwrap();
        assert_eq!(
            h_empty, h_with_two,
            "chain head invariance must hold for any number of witnesses",
        );
    }

    // ─────────────────────────────────────────────────────────────────
    // V1.13 wave-C-2: ChainHeadHex newtype contract.
    // ─────────────────────────────────────────────────────────────────

    /// `chain_head_for` returns a `ChainHeadHex` whose `as_str()` view
    /// is exactly 64 lowercase hex chars (same shape the pre-newtype
    /// `String` return promised). Pins the type-level contract that
    /// downstream consumers rely on.
    #[test]
    fn chain_head_hex_shape_contract() {
        let batch = fixture_batch(0, crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        let head = chain_head_for(&batch).unwrap();
        let s = head.as_str();
        assert_eq!(s.len(), 64, "ChainHeadHex must be 64 hex chars: {s}");
        assert!(
            s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "ChainHeadHex must be all-lowercase hex: {s}",
        );
    }

    /// `to_bytes` returns exactly 32 bytes — the raw blake3 digest
    /// that producers (witnesses, downstream signers) sign over.
    /// Defends a future caller that switches from hex to raw bytes
    /// from accidentally reading the hex chars themselves as bytes.
    #[test]
    fn chain_head_hex_to_bytes_round_trip() {
        let batch = fixture_batch(0, crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        let head = chain_head_for(&batch).unwrap();
        let bytes = head.to_bytes();
        assert_eq!(bytes.len(), 32);
        // Round-trip: hex-encoding the bytes back yields the original
        // hex view byte-for-byte.
        assert_eq!(hex::encode(bytes), head.as_str());
    }

    /// `Display` and `as_str` produce identical text. Both must match
    /// the hex view, not surface debug-style annotations.
    #[test]
    fn chain_head_hex_display_matches_as_str() {
        let batch = fixture_batch(0, crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        let head = chain_head_for(&batch).unwrap();
        assert_eq!(format!("{}", head), head.as_str());
    }

    /// `into_inner` and `From<ChainHeadHex> for String` produce the
    /// same `String` — both are the wire-emit boundary helpers used
    /// when assigning to `AnchorChain.head` / `AnchorBatch.previous_head`.
    #[test]
    fn chain_head_hex_into_inner_and_from_agree() {
        let batch = fixture_batch(0, crate::trace_format::ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        let h_a = chain_head_for(&batch).unwrap();
        let h_b = chain_head_for(&batch).unwrap();
        let from_into: String = h_a.into();
        let from_method: String = h_b.into_inner();
        assert_eq!(from_into, from_method);
    }

    /// `ChainHeadHex::new` accepts a 64-char lowercase-hex String. This
    /// is the wire-side construction path (e.g. parsing a stored head
    /// from JSON before passing it through type-checked APIs).
    #[test]
    fn chain_head_hex_new_accepts_valid_shape() {
        let valid = "a".repeat(64);
        let h = ChainHeadHex::new(valid.clone()).expect("valid 64-char lowercase hex must accept");
        assert_eq!(h.as_str(), valid);
    }

    /// Wrong-length hex is rejected with a length-specific encoding
    /// error. Defends against a wire-side String containing the right
    /// chars but wrong total — would otherwise silently mismatch
    /// every downstream comparison.
    #[test]
    fn chain_head_hex_new_rejects_wrong_length() {
        // 63 chars (one short).
        let short = "a".repeat(63);
        match ChainHeadHex::new(short) {
            Err(TrustError::Encoding(msg)) => assert!(
                msg.contains("64 hex chars") && msg.contains("63"),
                "length error must name expected and actual: {msg}",
            ),
            other => panic!("expected Encoding error, got {other:?}"),
        }

        // 65 chars (one over).
        let long = "a".repeat(65);
        assert!(matches!(
            ChainHeadHex::new(long),
            Err(TrustError::Encoding(_)),
        ));
    }

    /// Uppercase hex is rejected. Lowercase-only is enforced because
    /// `chain_head_for` always emits lowercase, and accepting both
    /// would let two byte-different wire strings represent the same
    /// head — a `ct_eq_str` divergence waiting to happen.
    #[test]
    fn chain_head_hex_new_rejects_uppercase_hex() {
        let upper = "A".repeat(64);
        match ChainHeadHex::new(upper) {
            Err(TrustError::Encoding(msg)) => assert!(
                msg.contains("lowercase"),
                "case-violation error must name lowercase: {msg}",
            ),
            other => panic!("expected Encoding error, got {other:?}"),
        }
    }

    /// Non-hex char is rejected (e.g. 'g' or punctuation).
    #[test]
    fn chain_head_hex_new_rejects_non_hex_chars() {
        let bad = "g".repeat(64); // 'g' is past hex range
        assert!(matches!(
            ChainHeadHex::new(bad),
            Err(TrustError::Encoding(_)),
        ));

        let punct = ":".repeat(64);
        assert!(matches!(
            ChainHeadHex::new(punct),
            Err(TrustError::Encoding(_)),
        ));
    }

    /// `PartialEq` between `ChainHeadHex` and `&str` / `String` works
    /// in both directions. Pins the assertion-ergonomics impls so
    /// existing test patterns (e.g. `assert_eq!(s, head)` and
    /// `assert_eq!(head, s)`) keep compiling.
    #[test]
    fn chain_head_hex_partial_eq_with_str_both_directions() {
        let valid = "b".repeat(64);
        let h = ChainHeadHex::new(valid.clone()).unwrap();
        // ChainHeadHex == &str / String
        assert_eq!(h, valid.as_str());
        assert_eq!(h, valid);
        // &str / String == ChainHeadHex (reverse direction)
        assert_eq!(valid.as_str(), h);
        assert_eq!(valid, h);
    }
}
