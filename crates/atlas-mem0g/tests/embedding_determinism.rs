//! V2-β Welle 18b: V2 verification gap closure — embedding determinism.
//!
//! Per spike §12 V2: "fastembed-rs determinism across ORT minor
//! versions — 2-run byte-equality test on bge-small-en-v1.5 FP32 +
//! `OMP_NUM_THREADS=1` on Linux + Windows + macOS".
//!
//! Atlas's cache-key strategy uses `event_uuid` as the canonical
//! invariant (ADR §4 sub-decision #3), NOT `embedding_hash` — so
//! embedding non-determinism would NOT cause data corruption, only
//! cache mis-hits. Still, determinism is the load-bearing claim
//! under the dual cache-key strategy's faster-lookup path; this
//! test gates that claim.
//!
//! ## Test gate
//!
//! `#[ignore]`-gated behind `ATLAS_MEM0G_DETERMINISM_ENABLED=1` so
//! CI runs it explicitly (model download is ~130 MB; not every
//! contributor cares to pay that cost on `cargo test`).
//!
//! ## TDD-RED → GREEN posture
//!
//! Without the `lancedb-backend` feature, this test exists but is
//! `#[ignore]`d and skipped — there is no embedder to exercise.
//! With the feature, the test:
//!
//! 1. Sets `OMP_NUM_THREADS=1` programmatically.
//! 2. Constructs `AtlasEmbedder::new` against a tempdir model cache.
//! 3. Calls `embed(text)` twice on the same input.
//! 4. Asserts the two output vectors are byte-equal (`Vec<f32>`
//!    structural equality).
//!
//! ## Placeholder caveat
//!
//! With placeholder `ONNX_SHA256` constants, `AtlasEmbedder::new`
//! will fail closed with `SupplyChainMismatch` — surfacing the
//! supply-chain control AS the test outcome until Nelson lifts the
//! placeholders. Document this in the plan-doc's "what's deferred"
//! section.

#![allow(unused_imports)]

use atlas_mem0g::embedder;

#[test]
#[ignore = "requires fastembed model download + ATLAS_MEM0G_DETERMINISM_ENABLED=1"]
#[cfg(feature = "lancedb-backend")]
fn embedding_determinism_two_runs_byte_equal() {
    // Run only when explicitly enabled.
    if std::env::var("ATLAS_MEM0G_DETERMINISM_ENABLED").as_deref() != Ok("1") {
        eprintln!("skipping; set ATLAS_MEM0G_DETERMINISM_ENABLED=1 to run");
        return;
    }

    embedder::pin_omp_threads_single();

    let dir = tempfile::tempdir().expect("tempdir");
    let model_cache = dir.path().to_path_buf();

    let embedder_a = embedder::AtlasEmbedder::new(&model_cache)
        .expect("first embedder init");
    let text = "Atlas trust substrate semantic search determinism check";

    let v1 = embedder_a.embed(text).expect("first embed");
    let v2 = embedder_a.embed(text).expect("second embed");

    assert_eq!(
        v1.len(),
        v2.len(),
        "embedding output vectors must have equal length"
    );
    // Byte-equality at f32 representation level. Determinism contract.
    assert_eq!(v1, v2, "two embed() calls on same input must produce byte-equal output");
}

#[test]
#[cfg(not(feature = "lancedb-backend"))]
fn embedding_determinism_skipped_without_feature() {
    // Sentinel: the determinism test compiles + runs cleanly even
    // when the lancedb-backend feature is OFF. The actual
    // determinism check requires the feature; this test just
    // documents that posture.
    eprintln!("embedding determinism test requires --features lancedb-backend");
}

/// Post-Phase-A well-formedness sentinel for the supply-chain pins.
///
/// W18c Phase A lifted the W18b `TODO_W18B_NELSON_VERIFY_*`
/// placeholders to real values resolved against HuggingFace
/// `BAAI/bge-small-en-v1.5` revision `5c38ec7c…`. W18c Phase B
/// extended the pin set with the fourth tokenizer file
/// (`TOKENIZER_CONFIG_JSON_*`) discovered during fastembed-rs
/// 5.13.4 API-surface verification.
///
/// This test replaces the W18b-era `starts_with("TODO_W18B")`
/// gatekeeper (which would now incorrectly fail post-lift) with
/// the post-lift well-formedness contract: all pins are
/// non-placeholder, conform to their respective hash-format
/// invariants, and the URL pins embed the revision SHA. Mirrors
/// the in-crate `pins_well_formed_after_lift` test but from the
/// integration-test surface (verifies the `pub const` surface is
/// the well-formedness contract that downstream consumers can
/// rely on).
#[test]
#[cfg(feature = "lancedb-backend")]
fn pins_remain_well_formed_post_phase_a_lift() {
    // SHA-256 digests: 64-char lowercase hex.
    for (label, value) in [
        ("ONNX_SHA256", embedder::ONNX_SHA256),
        ("TOKENIZER_JSON_SHA256", embedder::TOKENIZER_JSON_SHA256),
        ("CONFIG_JSON_SHA256", embedder::CONFIG_JSON_SHA256),
        (
            "SPECIAL_TOKENS_MAP_SHA256",
            embedder::SPECIAL_TOKENS_MAP_SHA256,
        ),
        (
            "TOKENIZER_CONFIG_JSON_SHA256",
            embedder::TOKENIZER_CONFIG_JSON_SHA256,
        ),
    ] {
        assert_eq!(value.len(), 64, "{label} must be 64-char SHA-256 hex");
        assert!(
            value.chars().all(|c| c.is_ascii_hexdigit()),
            "{label} must be all ASCII hex digits"
        );
        assert!(
            !value.starts_with("TODO_W18B"),
            "{label} must NOT be a W18b placeholder post-lift"
        );
    }

    // HF revision SHA-1: 40-char lowercase hex.
    assert_eq!(
        embedder::HF_REVISION_SHA.len(),
        40,
        "HF_REVISION_SHA must be 40-char SHA-1 hex"
    );

    // URL pins: must point at HuggingFace + embed the revision SHA.
    for (label, value) in [
        ("MODEL_URL", embedder::MODEL_URL),
        ("TOKENIZER_JSON_URL", embedder::TOKENIZER_JSON_URL),
        ("CONFIG_JSON_URL", embedder::CONFIG_JSON_URL),
        (
            "SPECIAL_TOKENS_MAP_URL",
            embedder::SPECIAL_TOKENS_MAP_URL,
        ),
        (
            "TOKENIZER_CONFIG_JSON_URL",
            embedder::TOKENIZER_CONFIG_JSON_URL,
        ),
    ] {
        assert!(
            value.starts_with("https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/"),
            "{label} must point at the pinned HF revision"
        );
        assert!(
            value.contains(embedder::HF_REVISION_SHA),
            "{label} must embed HF_REVISION_SHA in path"
        );
    }
}

/// W18c Phase B end-to-end embedder smoke: download model files,
/// SHA-verify, initialize fastembed via `try_new_from_user_defined`,
/// embed one sentence, assert the output is a 384-dim FP32 vector.
///
/// Gated behind `ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1` because the run
/// downloads ~130 MB of model files; CI workflows that set this env
/// var (e.g. `.github/workflows/atlas-mem0g-smoke.yml` once
/// Phase C extends it) will exercise the full path. Local
/// `cargo test --features lancedb-backend` skips by default.
#[test]
#[ignore = "requires HF model download + ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1"]
#[cfg(feature = "lancedb-backend")]
fn embed_returns_384_dim_vector() {
    if std::env::var("ATLAS_MEM0G_EMBED_SMOKE_ENABLED").as_deref() != Ok("1") {
        eprintln!("skipping; set ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 to run");
        return;
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let embedder = embedder::AtlasEmbedder::new(dir.path())
        .expect("AtlasEmbedder::new should succeed against a valid model dir");
    let vec = embedder
        .embed("Atlas trust substrate semantic search")
        .expect("embed should return Ok");
    assert_eq!(
        vec.len(),
        384,
        "bge-small-en-v1.5 returns 384-dim embedding vectors"
    );
    assert!(
        vec.iter().any(|&v| v != 0.0),
        "embedding vector must not be all zeros (degenerate output)"
    );
}
