//! V2-β Welle 18b/c: V2 verification gap closure — embedding determinism.
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
//! `#[ignore]`-gated behind `ATLAS_MEM0G_DETERMINISM_ENABLED=1`
//! (in-process determinism only, fast — Phase B Phase C local) AND
//! `ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1` (full HF download + 2-run
//! byte-equality, ~130 MB — Phase C cross-OS matrix). CI runs both;
//! contributors only pay the model download cost when explicitly
//! asked.
//!
//! ## W18c Phase C upgrade — 2-run byte-equality across 3-OS matrix
//!
//! Phase B shipped the `try_new_from_user_defined` wiring and the
//! single-run `embed_returns_384_dim_vector` smoke. Phase C adds the
//! cross-platform 2-run byte-equality assert
//! ([`embedding_byte_equal_two_runs`]) running on Linux + Windows +
//! macOS via the expanded `atlas-mem0g-smoke` workflow matrix. The
//! original `embedding_determinism_two_runs_byte_equal` (Phase B
//! placeholder, kept for backward compat with the
//! `ATLAS_MEM0G_DETERMINISM_ENABLED` operator env var) still
//! exists but the new test is the load-bearing cross-OS gate.
//!
//! ## Cross-platform fallback policy (R-W18c-C1)
//!
//! Per W18c plan-doc Risk R-W18c-C1: cross-platform float
//! determinism is hard. If Windows fails byte-equality, the
//! documented fallback (operator-runbook §"Cross-platform fallback")
//! is to switch to `event_uuid`-only cache-key (Atlas's canonical
//! invariant; no semantic loss — only loses the dual-key faster-lookup
//! optimisation). Linux remains the primary deployment target.
//!
//! Failure-with-documented-fallback meets the W18c Phase C acceptance
//! criterion; silent failure does not.

#![allow(unused_imports)]

use atlas_mem0g::{embedder, supply_chain};

#[test]
#[ignore = "requires fastembed model download + ATLAS_MEM0G_DETERMINISM_ENABLED=1"]
#[cfg(feature = "lancedb-backend")]
fn embedding_determinism_two_runs_byte_equal() {
    // Run only when explicitly enabled.
    if std::env::var("ATLAS_MEM0G_DETERMINISM_ENABLED").as_deref() != Ok("1") {
        eprintln!("skipping; set ATLAS_MEM0G_DETERMINISM_ENABLED=1 to run");
        return;
    }

    supply_chain::pin_omp_threads_single();

    // PR #107 reviewer HIGH-1: resolve model cache from env (CI cache)
    // with tempdir fallback (local). See `embedding_byte_equal_two_runs`
    // for full rationale.
    let _tmp = tempfile::tempdir().expect("tempdir");
    let model_cache: std::path::PathBuf =
        match std::env::var("ATLAS_MEM0G_MODEL_CACHE_DIR") {
            Ok(p) if !p.is_empty() => {
                let pb = std::path::PathBuf::from(p);
                std::fs::create_dir_all(&pb).expect("create persistent model cache dir");
                pb
            }
            _ => _tmp.path().to_path_buf(),
        };

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
/// var (e.g. `.github/workflows/atlas-mem0g-smoke.yml`) will
/// exercise the full path. Local `cargo test --features
/// lancedb-backend` skips by default.
#[test]
#[ignore = "requires HF model download + ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1"]
#[cfg(feature = "lancedb-backend")]
fn embed_returns_384_dim_vector() {
    if std::env::var("ATLAS_MEM0G_EMBED_SMOKE_ENABLED").as_deref() != Ok("1") {
        eprintln!("skipping; set ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 to run");
        return;
    }

    // PR #107 reviewer HIGH-1: resolve model cache from env (CI cache)
    // with tempdir fallback (local). See `embedding_byte_equal_two_runs`
    // for full rationale.
    let _tmp = tempfile::tempdir().expect("tempdir");
    let model_cache: std::path::PathBuf =
        match std::env::var("ATLAS_MEM0G_MODEL_CACHE_DIR") {
            Ok(p) if !p.is_empty() => {
                let pb = std::path::PathBuf::from(p);
                std::fs::create_dir_all(&pb).expect("create persistent model cache dir");
                pb
            }
            _ => _tmp.path().to_path_buf(),
        };
    let embedder = embedder::AtlasEmbedder::new(&model_cache)
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

/// W18c Phase C cross-platform 2-run byte-equality gate (V2).
///
/// This is the load-bearing cross-OS determinism assertion. Runs on
/// Linux + Windows + macOS via the expanded `atlas-mem0g-smoke`
/// workflow matrix. Each leg:
///
/// 1. Set `OMP_NUM_THREADS=1` programmatically (load-bearing for
///    deterministic single-thread CPU path under ORT).
/// 2. Construct `AtlasEmbedder::new` against a tempdir model cache
///    (real download path; HF revision pinned).
/// 3. Call `embed(text)` twice on the same input.
/// 4. Assert the two output vectors are byte-equal (`Vec<f32>`
///    structural equality — this is structural equality of the
///    underlying f32 bit patterns, NOT epsilon-comparison).
///
/// Per spike §3.4 + ADR §4 sub-decision #2: deterministic under
/// pinned (ORT, threads=1, FP32). Under those conditions, two calls
/// on the same input bytes produce byte-equal output.
///
/// ## Failure-mode contract
///
/// - PASS Linux + Windows + macOS → spike §12 V2 RESOLVED. Dual-key
///   cache strategy (event_uuid + embedding_hash) is operationally
///   safe.
/// - FAIL on any leg → R-W18c-C1 fallback applies: Atlas operates
///   under event_uuid-only cache-key on the failing platform.
///   Operator-runbook §"Cross-platform fallback" documents.
///
/// Gated behind `ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1` (~130 MB
/// download budget — same as the Phase B smoke). The
/// `atlas-mem0g-smoke` workflow sets it on every leg of the matrix.
#[test]
#[ignore = "requires HF model download + ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1"]
#[cfg(feature = "lancedb-backend")]
fn embedding_byte_equal_two_runs() {
    if std::env::var("ATLAS_MEM0G_EMBED_SMOKE_ENABLED").as_deref() != Ok("1") {
        eprintln!("skipping; set ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 to run");
        return;
    }

    // Pin OMP threads to 1 BEFORE init. AtlasEmbedder::new also
    // pins it internally via supply_chain::pin_omp_threads_single,
    // but doing it here too means the test ALSO covers the
    // operator-side discipline (the env var must be set BEFORE the
    // ORT session is built; subsequent changes are no-ops).
    supply_chain::pin_omp_threads_single();

    // Resolve model cache dir: prefer ATLAS_MEM0G_MODEL_CACHE_DIR (set
    // by `atlas-mem0g-smoke` to the path that `actions/cache@v4`
    // restores) so CI hits the cache; fall back to per-test tempdir
    // for local runs. PR #107 reviewer HIGH-1 — without this, every
    // 3-OS leg re-downloaded ~130 MB on every run, defeating the cache
    // step's stated purpose. The TOCTOU-free `read_and_verify` primitive
    // still runs on cache-hit (it re-verifies the cached bytes against
    // the compiled-in pin), so the supply-chain contract is preserved.
    let _tmp = tempfile::tempdir().expect("tempdir");
    let model_cache: std::path::PathBuf =
        match std::env::var("ATLAS_MEM0G_MODEL_CACHE_DIR") {
            Ok(p) if !p.is_empty() => {
                let pb = std::path::PathBuf::from(p);
                std::fs::create_dir_all(&pb).expect("create persistent model cache dir");
                pb
            }
            _ => _tmp.path().to_path_buf(),
        };
    let embedder_a = embedder::AtlasEmbedder::new(&model_cache)
        .expect("AtlasEmbedder::new should succeed against a valid model dir");
    let text = "Atlas trust substrate semantic search determinism check";

    let v1 = embedder_a.embed(text).expect("first embed");
    let v2 = embedder_a.embed(text).expect("second embed");

    // Print to stdout for CI artifact capture across the 3-OS
    // matrix. `atlas-mem0g-smoke` workflow tees test output so this
    // line is preserved as evidence per leg even if the assertion
    // passes (useful to spot upstream drift early).
    println!(
        "V2_DETERMINISM os={} text_bytes={} v1_dim={} v1_first4=[{:?}, {:?}, {:?}, {:?}] equal={}",
        std::env::consts::OS,
        text.len(),
        v1.len(),
        v1.first().copied().unwrap_or(f32::NAN),
        v1.get(1).copied().unwrap_or(f32::NAN),
        v1.get(2).copied().unwrap_or(f32::NAN),
        v1.get(3).copied().unwrap_or(f32::NAN),
        v1 == v2
    );

    assert_eq!(
        v1.len(),
        v2.len(),
        "embedding output vectors must have equal length"
    );
    // Byte-equality at f32 representation level. Determinism contract.
    // Failure here on Windows specifically triggers R-W18c-C1 fallback
    // (operator-runbook §"Cross-platform fallback"); failure on Linux
    // or macOS is unexpected and indicates upstream ORT drift.
    assert_eq!(
        v1, v2,
        "two embed() calls on same input must produce byte-equal output \
         (R-W18c-C1: if this fails on Windows, switch to event_uuid-only \
         cache-key per operator-runbook fallback policy)"
    );
}
