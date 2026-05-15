//! V2-β Welle 18c Phase D: end-to-end LanceDB body integration tests.
//!
//! These tests exercise the real `LanceDbCacheBackend::{upsert, search,
//! erase, rebuild}` bodies that Phase D wires up. They cover:
//!
//! - **Body correctness:** upsert → search round-trip returns a hit
//!   carrying the upserted `event_uuid` (cite-back trust property).
//! - **Erase semantics:** post-erase search returns zero hits for the
//!   erased event_uuid; secure-delete protocol does NOT corrupt the
//!   table for surviving rows.
//! - **Deadlock safety (R-W18c-D2):** concurrent multi-task search +
//!   upsert from inside a multi-thread tokio runtime does NOT
//!   deadlock. This is the spike §7 critical property.
//! - **Cite-back end-to-end:** the `event_uuid` in a `SemanticHit`
//!   matches a Layer-1 `AtlasEvent::event_id` exactly so the offline
//!   WASM verifier can independently verify it against `events.jsonl`.
//!
//! Gated behind `ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1` because backend
//! construction triggers the ~130 MB model download. CI workflows
//! that opt in (atlas-mem0g-smoke when extended in a follow-on welle)
//! exercise the full path; local `cargo test --features lancedb-backend`
//! skips by default.
//!
//! ## TDD-RED → GREEN posture
//!
//! Before Phase D, the body sites returned `Mem0gError::Backend("not
//! yet wired")` and `Ok(vec![])` placeholder values. Running these
//! tests against the W18b/Phase-B baseline would fail the upsert →
//! search round-trip (search always returns empty). Phase D's wiring
//! flips them GREEN. Without the env-var gate they are skipped, so
//! the workspace `cargo test` posture is unchanged.

#![cfg(feature = "lancedb-backend")]
#![allow(unused_imports)]

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use atlas_mem0g::{LanceDbCacheBackend, Mem0gError, SemanticCacheBackend};
use atlas_trust_core::trace_format::{AtlasEvent, EventSignature};
use serde_json::json;

/// Returns `true` iff the embedder smoke gate is enabled. Mirrors the
/// gating used by `tests/embedding_determinism.rs::embed_returns_384_dim_vector`.
fn smoke_gate_enabled() -> bool {
    std::env::var("ATLAS_MEM0G_EMBED_SMOKE_ENABLED").as_deref() == Ok("1")
}

/// Build a backend rooted in a fresh tempdir. Caller owns the
/// tempdir for cleanup ordering (the backend's runtime + LanceDB
/// connection drop BEFORE the dir).
fn build_backend(
    tmp: &tempfile::TempDir,
) -> Option<LanceDbCacheBackend> {
    if !smoke_gate_enabled() {
        eprintln!("skipping; set ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 to run");
        return None;
    }
    let storage_root = tmp.path().join("storage");
    let model_cache = tmp.path().join("model");
    Some(
        LanceDbCacheBackend::new(storage_root, model_cache)
            .expect("backend init"),
    )
}

/// TDD-GREEN: upsert → search round-trip returns the upserted event.
///
/// This is the load-bearing Phase D property: `/api/atlas/semantic-search`
/// is meant to return real `SemanticHit` rows, not the W18b empty
/// placeholder. The hit's `event_uuid` MUST match the upserted
/// `event_id` (cite-back trust contract per `lib.rs::SemanticHit`).
#[test]
#[ignore = "requires ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 + ~130 MB model download"]
fn upsert_then_search_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let Some(backend) = build_backend(&tmp) else { return };

    let workspace = "ws-d-roundtrip".to_string();
    let event_uuid = "01HEVENT-PHASE-D-RT".to_string();
    let text = "Atlas trust substrate verified credit decision Q1 2026";

    backend
        .upsert(&workspace, &event_uuid, text)
        .expect("upsert returns Ok after Phase D body wiring");

    let hits = backend
        .search(&workspace, "verified credit decision", 10)
        .expect("search returns Ok after Phase D body wiring");

    // GREEN-state assertion: the search returns AT LEAST one hit
    // (Phase D wiring; under W18b stub this would be empty).
    assert!(
        !hits.is_empty(),
        "Phase D wiring should return real hits, not empty placeholder"
    );

    // Cite-back trust property: every hit carries event_uuid; the
    // upserted event_uuid is in the result set.
    let found_event_uuids: Vec<&str> =
        hits.iter().map(|h| h.event_uuid.as_str()).collect();
    assert!(
        found_event_uuids.contains(&event_uuid.as_str()),
        "search results must contain upserted event_uuid; got {found_event_uuids:?}"
    );

    // workspace_id MUST be echoed back (per SemanticHit contract).
    for hit in &hits {
        assert_eq!(hit.workspace_id, workspace);
        assert!(!hit.snippet.is_empty(), "snippet must be populated");
    }
}

/// TDD-GREEN: search on a workspace that was never written returns
/// empty results (NOT TableNotFound). The Read-API surface treats
/// not-yet-populated workspaces as structurally identical to empty
/// caches (Layer 3 is rebuildable; emptiness is benign).
#[test]
#[ignore = "requires ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 + ~130 MB model download"]
fn search_on_empty_workspace_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let Some(backend) = build_backend(&tmp) else { return };

    let workspace = "ws-d-empty".to_string();
    let hits = backend
        .search(&workspace, "no-events-here", 10)
        .expect("search on empty workspace returns Ok with zero hits");
    assert!(hits.is_empty(), "empty workspace should return zero hits");
}

/// TDD-GREEN: erase removes the targeted event_uuid; surviving rows
/// remain searchable. Validates the W18b secure-delete protocol's
/// STEP 3 (DELETE) wiring + STEP 4 (CLEANUP via Prune).
#[test]
#[ignore = "requires ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 + ~130 MB model download"]
fn erase_removes_only_targeted_event() {
    let tmp = tempfile::tempdir().unwrap();
    let Some(backend) = build_backend(&tmp) else { return };

    let workspace = "ws-d-erase".to_string();
    let target_uuid = "01HEVENT-ERASE-ME".to_string();
    let survivor_uuid = "01HEVENT-SURVIVOR".to_string();

    backend
        .upsert(&workspace, &target_uuid, "Atlas erasable embedding payload")
        .expect("upsert target");
    backend
        .upsert(
            &workspace,
            &survivor_uuid,
            "Atlas long-lived embedding payload",
        )
        .expect("upsert survivor");

    backend
        .erase(&workspace, &target_uuid)
        .expect("erase returns Ok after Phase D body wiring");

    // Search broadly; the survivor MUST still be reachable.
    let hits = backend
        .search(&workspace, "Atlas embedding payload", 10)
        .expect("post-erase search Ok");
    let found: Vec<&str> = hits.iter().map(|h| h.event_uuid.as_str()).collect();
    assert!(
        !found.contains(&target_uuid.as_str()),
        "target event_uuid must NOT be in post-erase search results; got {found:?}"
    );
    assert!(
        found.contains(&survivor_uuid.as_str()),
        "survivor event_uuid must still be searchable; got {found:?}"
    );
}

/// TDD-GREEN: rebuild streams Layer-1 events into the cache. Smoke
/// test that the rebuild path drives `upsert` for each event; a
/// subsequent search returns hits.
#[test]
#[ignore = "requires ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 + ~130 MB model download"]
fn rebuild_streams_layer1_events_into_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let Some(backend) = build_backend(&tmp) else { return };

    let workspace = "ws-d-rebuild".to_string();
    let events: Vec<AtlasEvent> = (0..3)
        .map(|i| AtlasEvent {
            event_id: format!("01HREBUILD-{i:03}"),
            event_hash: "deadbeef".to_string(),
            parent_hashes: vec![],
            payload: json!({"type": "test", "i": i}),
            signature: EventSignature {
                alg: "EdDSA".to_string(),
                kid: "atlas-anchor:test".to_string(),
                sig: "AA".to_string(),
            },
            ts: "2026-05-15T00:00:00Z".to_string(),
            author_did: None,
        })
        .collect();

    backend
        .rebuild(&workspace, Box::new(events.into_iter()))
        .expect("rebuild Ok after Phase D wiring");

    let hits = backend
        .search(&workspace, "test event payload", 10)
        .expect("post-rebuild search Ok");
    assert!(
        !hits.is_empty(),
        "rebuild should populate the cache; expected non-empty hits"
    );
    // The rebuild call upserts each event; at least one of the
    // event_ids should appear in the hit set.
    let any_match = hits
        .iter()
        .any(|h| h.event_uuid.starts_with("01HREBUILD-"));
    assert!(
        any_match,
        "expected at least one 01HREBUILD-* event_uuid in hits; got {:?}",
        hits.iter().map(|h| &h.event_uuid).collect::<Vec<_>>()
    );
}

/// **R-W18c-D2 deadlock-safety regression.** Spawn N concurrent
/// search tasks from inside a multi-thread tokio runtime and assert
/// they all complete (no deadlock).
///
/// This is the property that the W18b plan-doc + spike §7 explicitly
/// flagged: a sync trait method that calls
/// `Handle::current().block_on()` from inside an async context
/// deadlocks under the single-threaded tokio scheduler — the worker
/// thread is occupied by the outer task, so the inner future has
/// nowhere to run.
///
/// Phase D defends against this by using a **dedicated multi-threaded
/// tokio::runtime::Runtime owned by the backend** (NOT
/// `Handle::current()`). The backend-owned runtime has its own
/// worker threads; `block_on` blocks the caller's thread but the
/// inner future executes on the backend's threads, so no scheduler
/// starvation.
///
/// We use `tokio::task::spawn_blocking` to call the sync trait method
/// from inside the multi-thread runtime, then `try_join_all` to
/// gather all task handles with a hard timeout.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 + ~130 MB model download"]
async fn search_does_not_deadlock_under_multi_task_tokio() {
    if !smoke_gate_enabled() {
        eprintln!("skipping; set ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 to run");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let storage_root = tmp.path().join("storage");
    let model_cache = tmp.path().join("model");

    // `LanceDbCacheBackend::new` builds a tokio runtime internally
    // (Phase D requirement; see module-level §"Sync-vs-async pattern"
    // in `crates/atlas-mem0g/src/lancedb_backend.rs`). Tokio refuses
    // to drop a runtime from inside an async context. Build the
    // backend AND its embedded runtime inside `spawn_blocking` so
    // both the construction and the eventual drop happen on a
    // dedicated blocking thread, not on a tokio worker.
    let backend = Arc::new(
        tokio::task::spawn_blocking(move || {
            LanceDbCacheBackend::new(storage_root, model_cache).expect("backend init")
        })
        .await
        .expect("backend init join"),
    );

    let workspace = "ws-d-deadlock".to_string();
    {
        let backend = Arc::clone(&backend);
        let workspace = workspace.clone();
        tokio::task::spawn_blocking(move || {
            backend
                .upsert(&workspace, &"01HEVENT-LIVE".to_string(), "deadlock probe")
                .expect("seed upsert");
        })
        .await
        .expect("seed upsert join");
    }

    // Spawn 8 concurrent searches. If R-W18c-D2 regressed, at least
    // one will hang and the test will exceed its 60s deadline.
    let mut handles = Vec::with_capacity(8);
    for i in 0..8 {
        let backend = Arc::clone(&backend);
        let workspace = workspace.clone();
        handles.push(tokio::task::spawn_blocking(move || {
            backend
                .search(&workspace, &format!("probe-{i}"), 5)
                .expect("concurrent search must not deadlock")
        }));
    }

    // Wrap the join-all in a deadline. If any task is stuck inside a
    // deadlocked block_on, the timeout fires and the test fails with
    // a clear error rather than hanging the CI runner forever.
    let joined = tokio::time::timeout(
        Duration::from_secs(60),
        futures::future::try_join_all(handles),
    )
    .await
    .expect("8 concurrent searches completed within deadlock-detection timeout")
    .expect("no task panicked");

    // Sanity: every task returned a vec (possibly empty).
    assert_eq!(
        joined.len(),
        8,
        "all 8 concurrent searches must complete"
    );

    // Drop the Arc-held backend on a blocking thread so its embedded
    // runtime can release its worker threads without panicking.
    let backend_owned = Arc::try_unwrap(backend)
        .ok()
        .expect("no other Arc holders at end of test");
    tokio::task::spawn_blocking(move || drop(backend_owned))
        .await
        .expect("backend drop join");
}

/// **R-W18c-D2 deadlock-safety, write-path variant.** Concurrent
/// upsert from inside a multi-thread tokio runtime. Tests the same
/// safety property as the search variant, but exercises the write
/// path (which holds the per-workspace WRITE lock instead of READ).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 + ~130 MB model download"]
async fn upsert_does_not_deadlock_under_multi_task_tokio() {
    if !smoke_gate_enabled() {
        eprintln!("skipping; set ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1 to run");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let storage_root = tmp.path().join("storage");
    let model_cache = tmp.path().join("model");

    // See `search_does_not_deadlock_under_multi_task_tokio` for the
    // spawn_blocking-around-construction rationale.
    let backend = Arc::new(
        tokio::task::spawn_blocking(move || {
            LanceDbCacheBackend::new(storage_root, model_cache).expect("backend init")
        })
        .await
        .expect("backend init join"),
    );

    let workspace = "ws-d-deadlock-w".to_string();
    let mut handles = Vec::with_capacity(4);
    for i in 0..4 {
        let backend = Arc::clone(&backend);
        let workspace = workspace.clone();
        handles.push(tokio::task::spawn_blocking(move || {
            backend
                .upsert(
                    &workspace,
                    &format!("01HEVENT-W-{i:03}"),
                    &format!("write probe payload {i}"),
                )
                .expect("concurrent upsert must not deadlock")
        }));
    }
    let _joined = tokio::time::timeout(
        Duration::from_secs(60),
        futures::future::try_join_all(handles),
    )
    .await
    .expect("4 concurrent upserts completed within deadlock-detection timeout")
    .expect("no task panicked");

    let backend_owned = Arc::try_unwrap(backend)
        .ok()
        .expect("no other Arc holders at end of test");
    tokio::task::spawn_blocking(move || drop(backend_owned))
        .await
        .expect("backend drop join");
}
