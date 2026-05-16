//! V2-β Welle 18c Phase C: V4 verification gap closure — fastembed-rs
//! model file size first-load measurement.
//!
//! Per W18 spike §12 V4 + ADR-Atlas-012 §3.4:
//!
//! > "fastembed-rs model size on disk (claimed ~130 MB) — first-load
//! > measurement in W18b CI."
//!
//! The spike literature claimed `bge-small-en-v1.5` FP32 ONNX is
//! ~130 MB. W18c Phase A confirmed at supply-chain-pin lift time:
//! 133,093,490 bytes / 126.93 MiB. This integration test asserts the
//! claim against the *actual on-disk file* after `AtlasEmbedder::new`
//! has run end-to-end (download + SHA-verify + cache), closing the
//! "verify the assertion against the running system, not just the
//! pre-computed pin" loop.
//!
//! ## Acceptance criterion (W18c Phase C, V4)
//!
//! - `AtlasEmbedder::new(tempdir)` succeeds against the live HF
//!   pinned revision (gated by `ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1`
//!   like the Phase B smoke test — same ~130 MB download budget).
//! - `std::fs::metadata(model_path)?.len()` reports a value within
//!   ±10% of 130 MB (130,000,000 ± 13,000,000 bytes).
//! - The actual measured size is printed to stdout so the
//!   `atlas-mem0g-smoke` CI artifact records it across all 3 OS
//!   matrix legs (Linux + Windows + macOS) for cross-platform
//!   confirmation.
//!
//! ## Scope explicitly NOT covered
//!
//! - Tokenizer-file sizes (small JSON; not the load-bearing claim).
//! - Memory-resident embedder size after init (the 130 MB claim is
//!   about *on-disk* footprint, which is what operators need to size
//!   their model-cache volume).
//! - Cross-platform byte-equality of the file (covered by the
//!   compiled-in `ONNX_SHA256` + the `read_and_verify` TOCTOU-free
//!   primitive — if Linux + Windows + macOS each pass the SHA gate
//!   then they all hold byte-identical model files by construction).
//!
//! ## Cross-platform contract
//!
//! Runs on all three OS in `atlas-mem0g-smoke` matrix. The model
//! file is the same byte-for-byte (HuggingFace is a content-addressed
//! LFS store; Atlas's `https_only(true)` reqwest config plus the
//! compiled-in `ONNX_SHA256` enforce identical bytes regardless of
//! OS). Therefore the size assertion is identical across legs.

#![allow(unused_imports)]

use atlas_mem0g::embedder;

/// Claimed envelope: ~130 MB ± 10% for the bge-small-en-v1.5 FP32
/// ONNX file. The Phase A pin-lift recorded the actual size as
/// 133,093,490 bytes (126.93 MiB), which sits well inside this
/// envelope. The ±10% tolerance allows for a future minor model
/// rotation (within the same family) to adjust file size slightly
/// without forcing a test-amend; a >10% drift indicates a
/// material upstream change that *should* trigger a re-spike.
#[cfg(feature = "lancedb-backend")]
const CLAIMED_SIZE_BYTES: u64 = 130_000_000;
#[cfg(feature = "lancedb-backend")]
const TOLERANCE_PCT: u64 = 10;

#[test]
#[ignore = "requires HF model download + ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1"]
#[cfg(feature = "lancedb-backend")]
fn fastembed_model_file_size_within_envelope() {
    if std::env::var("ATLAS_MEM0G_EMBED_SMOKE_ENABLED").as_deref() != Ok("1") {
        eprintln!("skipping; set ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 to run");
        return;
    }

    // Resolve model cache dir: prefer ATLAS_MEM0G_MODEL_CACHE_DIR (set
    // by `atlas-mem0g-smoke` to the path that `actions/cache@v4`
    // restores) so CI hits the cache; fall back to per-test tempdir
    // for local runs. PR #107 reviewer HIGH-1 — without this, every
    // CI leg re-downloaded ~130 MB unnecessarily.
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

    // Trigger the full download + SHA-verify + cache path via
    // AtlasEmbedder::new. We don't need the embedder for this test;
    // we only need the side-effect of the model file landing on disk
    // at the canonical cache path.
    let _embedder = embedder::AtlasEmbedder::new(&model_cache)
        .expect("AtlasEmbedder::new should succeed against valid HF pinned revision");

    // The model is cached at `<model_cache>/bge-small-en-v1.5.onnx`
    // per AtlasEmbedder::new step 1.
    let model_path = model_cache.join("bge-small-en-v1.5.onnx");
    let metadata = std::fs::metadata(&model_path)
        .expect("model file must exist on disk after AtlasEmbedder::new");
    let actual_size = metadata.len();

    // Print to stdout for CI artifact capture across the 3-OS matrix.
    // `atlas-mem0g-smoke` workflow tees the V1-V4 step output to
    // `target/mem0g-v1v4-${matrix.os}.log` so this line is preserved
    // as evidence per leg.
    println!(
        "V4_MEASURE os={} model_file=bge-small-en-v1.5.onnx bytes={} mib={:.2}",
        std::env::consts::OS,
        actual_size,
        actual_size as f64 / (1024.0 * 1024.0)
    );

    let tolerance_bytes = (CLAIMED_SIZE_BYTES * TOLERANCE_PCT) / 100;
    let lower = CLAIMED_SIZE_BYTES.saturating_sub(tolerance_bytes);
    let upper = CLAIMED_SIZE_BYTES.saturating_add(tolerance_bytes);

    assert!(
        actual_size >= lower && actual_size <= upper,
        "model file size {actual_size} bytes is outside claimed envelope \
         {CLAIMED_SIZE_BYTES} ± {TOLERANCE_PCT}% ([{lower}, {upper}] bytes); \
         a >10% drift indicates a material upstream change — re-spike before \
         updating the constant"
    );
}

/// Sentinel: the test compiles + runs cleanly without the
/// `lancedb-backend` feature. Mirrors the convention in
/// `embedding_determinism.rs` so `cargo test --workspace` (default
/// features) does not break.
#[test]
#[cfg(not(feature = "lancedb-backend"))]
fn embedder_size_measurement_skipped_without_feature() {
    eprintln!("V4 model-size measurement requires --features lancedb-backend");
}
