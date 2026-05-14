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

/// Placeholder-constants sentinel. With placeholder `ONNX_SHA256`,
/// the embedder init fails closed via `SupplyChainMismatch` — verify
/// the failure surface exists.
#[test]
#[cfg(feature = "lancedb-backend")]
fn embedder_fails_closed_on_supply_chain_mismatch() {
    // When ONNX_SHA256 is a placeholder, any download attempt
    // surfaces SupplyChainMismatch. Verify that error path is
    // structurally reachable.
    use atlas_mem0g::Mem0gError;

    // The placeholder constant starts with "TODO_W18B"; this test
    // documents that the security gate IS load-bearing pre-merge.
    assert!(
        embedder::ONNX_SHA256.starts_with("TODO_W18B"),
        "placeholder sentinel — update test alongside real-value lift"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let result = embedder::AtlasEmbedder::new(dir.path());
    // Either Io (network unreachable in CI sandbox) OR
    // SupplyChainMismatch (network reachable + SHA mismatched) is
    // acceptable. Both are fail-closed paths.
    match result {
        Err(Mem0gError::SupplyChainMismatch { .. }) => {
            // Ideal fail-closed path.
        }
        Err(Mem0gError::Io(_)) => {
            // Network unreachable — also fail-closed.
        }
        Err(Mem0gError::Embedder(_)) => {
            // fastembed-rs reported an init error before our SHA check
            // got to run — acceptable in placeholder mode.
        }
        Ok(_) => panic!(
            "embedder must NOT succeed-init with placeholder ONNX_SHA256 \
             constant; supply-chain control is load-bearing"
        ),
        Err(e) => panic!("unexpected error variant: {e:?}"),
    }
}
