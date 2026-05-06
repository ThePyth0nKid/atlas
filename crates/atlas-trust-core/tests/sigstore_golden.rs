//! Round-trip integration test: a real Sigstore Rekor v1 entry from the
//! production log must verify cleanly against the verifier's pinned
//! pubkey, with no manual fixups in the proof or signature.
//!
//! The fixture was captured from `https://rekor.sigstore.dev/api/v1/log/
//! entries?logIndex=800000000` (a hashedrekord entry on the active
//! shard, treeID `1193050959916656506`). The fixture is checked in so
//! tests run offline; an auditor can re-fetch from the URL in
//! `source` and `diff` against the file to confirm provenance.
//!
//! What this test catches that the unit tests don't:
//!   - End-to-end shape: the verifier accepts the exact JSON shape that
//!     `rekor.sigstore.dev/api/v1` returns, no schema drift between our
//!     `AnchorEntry` and what the log actually emits.
//!   - The pinned PEM in `atlas-trust-core::anchor::SIGSTORE_REKOR_V1.pem`
//!     is the production key (if it weren't, the ECDSA verify here would
//!     fail with the real signature).
//!   - The 4-byte C2SP keyID check: the signed-note keyID `c0d23d6a` must
//!     match `SHA-256(DER SPKI)[..4]` of the pinned key.
//!   - The hashedrekord-only entry-body anti-forgery check accepts a real
//!     production hashedrekord body.
//!
//! If Sigstore ever rotates the active log key, this test starts failing
//! and forces a coordinated update of `SIGSTORE_REKOR_V1.pem` + a new
//! fixture capture.

use atlas_trust_core::anchor::{
    default_trusted_logs, verify_anchor_entry, SIGSTORE_REKOR_V1_LOG_ID,
};
use atlas_trust_core::trace_format::{AnchorEntry, AnchorKind, InclusionProof};
use serde::Deserialize;

/// Mirror of the JSON checked into `tests/fixtures/`.
#[derive(Debug, Deserialize)]
struct Fixture {
    body: String,
    anchored_hash: String,
    log_id: String,
    log_index: u64,
    tree_id: i64,
    integrated_time: i64,
    tree_size: u64,
    root_hash: String,
    hashes: Vec<String>,
    checkpoint_sig: String,
    #[serde(default)]
    #[allow(dead_code)]
    source: String,
    #[serde(default)]
    #[allow(dead_code)]
    fetched_at_unix: i64,
}

fn load_fixture() -> Fixture {
    let path = "tests/fixtures/sigstore_rekor_v1_logindex_800000000.json";
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("missing fixture {path}: {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("malformed fixture {path}: {e}"))
}

/// The fixture's `log_id` (returned by the Rekor API as `logID`) must
/// match the verifier's derived `SIGSTORE_REKOR_V1_LOG_ID`. If this
/// fails, the pinned PEM in `anchor.rs` is for a different key than
/// the production log uses — the entire Sigstore-anchor trust property
/// is broken.
#[test]
fn fixture_log_id_matches_pinned() {
    let fx = load_fixture();
    assert_eq!(
        fx.log_id, *SIGSTORE_REKOR_V1_LOG_ID,
        "fixture log_id {} does not match the verifier's pinned \
         SIGSTORE_REKOR_V1_LOG_ID {} — the pinned PEM is for the wrong key",
        fx.log_id,
        *SIGSTORE_REKOR_V1_LOG_ID,
    );
}

/// The full pipeline: build an `AnchorEntry` from the captured fixture
/// and run `verify_anchor_entry`. This passes only if every step works
/// against real Sigstore data: the entry body decodes and binds the
/// claimed hash; the leaf hash recomputes; the 31-deep inclusion path
/// reaches the claimed root under SHA-256 RFC 6962 hashing; the C2SP
/// signed-note checkpoint signature verifies under the pinned ECDSA
/// P-256 pubkey.
#[test]
fn verifies_real_sigstore_rekor_entry() {
    let fx = load_fixture();
    let entry = AnchorEntry {
        // The Sigstore entry was created over a generic hashedrekord
        // (not specifically a dag_tip or bundle_hash). For the verifier
        // contract, the only thing `kind` controls in the Sigstore path
        // is the constructed AnchorOutcome's `kind` field — the leaf
        // hash is computed from `entry_body_b64`, not from kind. We
        // pick `BundleHash` here arbitrarily and set `expected_hash`
        // to match.
        kind: AnchorKind::BundleHash,
        anchored_hash: fx.anchored_hash.clone(),
        log_id: fx.log_id.clone(),
        log_index: fx.log_index,
        integrated_time: fx.integrated_time,
        inclusion_proof: InclusionProof {
            tree_size: fx.tree_size,
            root_hash: fx.root_hash.clone(),
            hashes: fx.hashes.clone(),
            checkpoint_sig: fx.checkpoint_sig.clone(),
        },
        entry_body_b64: Some(fx.body.clone()),
        tree_id: Some(fx.tree_id),
    };

    let trusted = default_trusted_logs();
    let outcome = verify_anchor_entry(&entry, &fx.anchored_hash, &trusted);
    assert!(
        outcome.ok,
        "real Sigstore Rekor entry must verify, got reason: {}",
        outcome.reason,
    );
}

/// Tampering the entry body — even by a single byte — must invalidate
/// the entry. The leaf hash changes, the inclusion path no longer
/// reaches the claimed root, and the verifier rejects.
#[test]
fn tampered_entry_body_is_rejected() {
    let fx = load_fixture();
    // Decode, flip the last byte of the JSON, re-encode.
    let mut body_raw = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        fx.body.as_bytes(),
    )
    .unwrap();
    let last = body_raw.len() - 1;
    body_raw[last] ^= 0x01;
    let tampered_body = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &body_raw,
    );

    let entry = AnchorEntry {
        kind: AnchorKind::BundleHash,
        anchored_hash: fx.anchored_hash.clone(),
        log_id: fx.log_id.clone(),
        log_index: fx.log_index,
        integrated_time: fx.integrated_time,
        inclusion_proof: InclusionProof {
            tree_size: fx.tree_size,
            root_hash: fx.root_hash.clone(),
            hashes: fx.hashes.clone(),
            checkpoint_sig: fx.checkpoint_sig.clone(),
        },
        entry_body_b64: Some(tampered_body),
        tree_id: Some(fx.tree_id),
    };

    let trusted = default_trusted_logs();
    let outcome = verify_anchor_entry(&entry, &fx.anchored_hash, &trusted);
    assert!(!outcome.ok, "tampered entry body must be rejected");
}

/// V1.7 trusts exactly three Sigstore Rekor v1 shards (active + 2
/// historical, all sharing the same key). An anchor whose `tree_id`
/// is outside that roster must be rejected at the dispatch layer with
/// a clear "not in the trusted-shard roster" reason — not a downstream
/// signature failure. We use a crafted small integer (well outside any
/// real Sigstore shard ID range) so future roster expansion does not
/// silently re-permit this test's value.
#[test]
fn unknown_tree_id_is_rejected() {
    let fx = load_fixture();
    let entry = AnchorEntry {
        kind: AnchorKind::BundleHash,
        anchored_hash: fx.anchored_hash.clone(),
        log_id: fx.log_id.clone(),
        log_index: fx.log_index,
        integrated_time: fx.integrated_time,
        inclusion_proof: InclusionProof {
            tree_size: fx.tree_size,
            root_hash: fx.root_hash.clone(),
            hashes: fx.hashes.clone(),
            checkpoint_sig: fx.checkpoint_sig.clone(),
        },
        entry_body_b64: Some(fx.body.clone()),
        // Bogus tree-ID — must remain outside the roster forever.
        tree_id: Some(1_234_567_890),
    };
    let trusted = default_trusted_logs();
    let outcome = verify_anchor_entry(&entry, &fx.anchored_hash, &trusted);
    assert!(!outcome.ok, "unknown tree_id must be rejected");
    assert!(
        outcome.reason.contains("roster") || outcome.reason.contains("Sigstore Rekor v1"),
        "rejection reason should name the roster policy, got: {}",
        outcome.reason,
    );
}

/// A historical-shard tree-ID is now in the roster, so the dispatch
/// layer must NOT short-circuit on it. Verification proceeds to the
/// signature/inclusion checks and fails *there* (because the captured
/// fixture's signature commits to the active shard's origin line).
/// This pins the behaviour change introduced by V1.7's roster
/// expansion: historical shards are no longer rejected up-front.
///
/// MAINTENANCE NOTE: the load-bearing assertion is the three-clause
/// reason check (`checkpoint`/`signature`/`origin`), not just `!ok`.
/// If a future change adds a new pre-signature guard (e.g. a
/// per-shard `log_index` range check) the dispatch layer might start
/// rejecting before reaching the signature step, and that new
/// rejection reason would have to be added here — otherwise this
/// test would silently degrade into a generic `!ok` assertion that
/// no longer pins where the failure must happen.
#[test]
fn historical_shard_tree_id_passes_dispatch_gate() {
    let fx = load_fixture();
    let entry = AnchorEntry {
        kind: AnchorKind::BundleHash,
        anchored_hash: fx.anchored_hash.clone(),
        log_id: fx.log_id.clone(),
        log_index: fx.log_index,
        integrated_time: fx.integrated_time,
        inclusion_proof: InclusionProof {
            tree_size: fx.tree_size,
            root_hash: fx.root_hash.clone(),
            hashes: fx.hashes.clone(),
            checkpoint_sig: fx.checkpoint_sig.clone(),
        },
        entry_body_b64: Some(fx.body.clone()),
        // Historical shard — in the roster, so dispatch must accept it.
        tree_id: Some(3_904_496_407_287_907_110),
    };
    let trusted = default_trusted_logs();
    let outcome = verify_anchor_entry(&entry, &fx.anchored_hash, &trusted);
    assert!(
        !outcome.ok,
        "the captured fixture is for the active shard, so a historical \
         tree_id must still fail — but at the checkpoint stage, not \
         at the roster gate",
    );
    assert!(
        !outcome.reason.contains("roster"),
        "historical shard must NOT be rejected by the roster gate — \
         it is in the trusted set; got reason: {}",
        outcome.reason,
    );
    assert!(
        outcome.reason.contains("checkpoint")
            || outcome.reason.contains("signature")
            || outcome.reason.contains("origin"),
        "expected failure at the signature/checkpoint stage, got: {}",
        outcome.reason,
    );
}

/// Forgery defence: a server submits one hash to Rekor, gets a valid
/// proof for it, then claims that proof anchors a *different* Atlas
/// hash. The leaf-hash check still passes (because the proof matches
/// the body Rekor saw), but `entry_body_binds_anchored_hash` enforces
/// `body.spec.data.hash.value == anchored_hash` and rejects.
#[test]
fn anchored_hash_forgery_is_rejected() {
    let fx = load_fixture();
    let bogus_hash =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();
    let entry = AnchorEntry {
        kind: AnchorKind::BundleHash,
        anchored_hash: bogus_hash.clone(),
        log_id: fx.log_id.clone(),
        log_index: fx.log_index,
        integrated_time: fx.integrated_time,
        inclusion_proof: InclusionProof {
            tree_size: fx.tree_size,
            root_hash: fx.root_hash.clone(),
            hashes: fx.hashes.clone(),
            checkpoint_sig: fx.checkpoint_sig.clone(),
        },
        entry_body_b64: Some(fx.body.clone()),
        tree_id: Some(fx.tree_id),
    };

    let trusted = default_trusted_logs();
    let outcome = verify_anchor_entry(&entry, &bogus_hash, &trusted);
    assert!(
        !outcome.ok,
        "swapping anchored_hash without re-anchoring must be rejected",
    );
}
