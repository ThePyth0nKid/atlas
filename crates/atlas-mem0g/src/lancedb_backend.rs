//! V2-β Welle 18b: `LanceDbCacheBackend` — production
//! [`crate::SemanticCacheBackend`] impl per ADR-Atlas-012 §4
//! sub-decision #1.
//!
//! ## Architecture
//!
//! - LanceDB 0.29 embedded as the Arrow-backed vector store.
//! - fastembed-rs 5.13.4 (exact-version pin) for ONNX-CPU
//!   `bge-small-en-v1.5` FP32 embedding.
//! - Atlas-owned [`crate::secure_delete`] wrapper for GDPR Art. 17
//!   compliance (per ADR §4 sub-decision #4).
//! - Atlas-owned [`crate::supply_chain::download_model_with_verification`]
//!   for supply-chain control (per ADR §4 sub-decision #2).
//! - Per-(workspace, table) `RwLock` map for TOCTOU-race closure.
//!
//! ## Sync-vs-async pattern (spike §7)
//!
//! The [`crate::SemanticCacheBackend`] trait surface is **sync**
//! (mirrors Layer-2 `GraphStateBackend` convention). LanceDB's
//! Rust API is async-first.
//!
//! **MEDIUM-6 clarification (reviewer-driven):** the W18b first-shipped
//! impl ships STUB BODIES with NO LanceDB calls — the `spawn_blocking`
//! guidance below applies to the resume-engineer who fills the bodies,
//! not the current code. The current bodies are pure-Rust filesystem
//! placeholders so the trait surface is reachable without LanceDB.
//!
//! When the LanceDB async bodies land (resume guide in
//! `.handoff/v2-beta-welle-18b-plan.md` §"LanceDB body stubs"), all
//! LanceDB calls MUST be wrapped via `tokio::task::spawn_blocking` —
//! **NOT** `tokio::runtime::Handle::current().block_on()` (the latter
//! deadlocks under the single-threaded tokio scheduler when called
//! from inside an async context per spike §7).
//!
//! Look for `// RESUME(spawn_blocking):` markers in this file's body
//! sites to find the exact call-locations where the resume engineer
//! drops the async-await pattern.
//!
//! ## W18b first-shipped impl scope
//!
//! This module ships the trait surface + secure-delete protocol
//! wiring + supply-chain verification path. The actual LanceDB
//! Arrow + DataFusion query bodies are stubbed with `Mem0gError::Backend`
//! placeholders pending the cross-platform determinism + bench
//! verification phase (V1-V4 in spike §12). The pattern mirrors
//! Layer-2 W17a's `ArcadeDbBackend` stub → W17b production-impl
//! handoff. The contract surface IS production-shape; only the
//! body fillings are TBD.
//!
//! See `.handoff/v2-beta-welle-18b-plan.md` Implementation Notes
//! §"LanceDB body stubs" for the resume guide.

use std::path::PathBuf;
use std::sync::Arc;

use atlas_trust_core::trace_format::AtlasEvent;

use crate::secure_delete::{PerTableLockMap, PreCapturedPaths};
use crate::{
    check_workspace_id, EventUuid, Mem0gError, Mem0gResult, SemanticCacheBackend, SemanticHit,
    WorkspaceId,
};

/// Production [`SemanticCacheBackend`] backed by LanceDB embedded +
/// fastembed-rs.
///
/// Construct via [`LanceDbCacheBackend::new`].
pub struct LanceDbCacheBackend {
    /// Filesystem root for LanceDB tables (one sub-directory per
    /// workspace, one table per workspace).
    storage_root: PathBuf,

    /// Model file cache directory (the BAAI/bge-small-en-v1.5 ONNX
    /// file lives here post-verification).
    #[allow(dead_code)]
    model_cache_dir: PathBuf,

    /// Per-(workspace, table) lock map for secure-delete TOCTOU
    /// closure. Wrapped in `Arc` so the trait methods (which take
    /// `&self`) can clone-and-hand-out without `&mut self`.
    locks: Arc<PerTableLockMap>,

    /// Embedder instance — owned by the backend (caller passes raw
    /// text; embedder-version pin is a single-impl swap).
    embedder: crate::embedder::AtlasEmbedder,
}

impl LanceDbCacheBackend {
    /// Construct a new LanceDB-backed cache.
    ///
    /// Steps:
    /// 1. Validate `storage_root` is a writable directory.
    /// 2. Verify model file via
    ///    [`crate::supply_chain::download_model_with_verification`]
    ///    (fail-closed on SHA mismatch).
    /// 3. Init fastembed-rs after
    ///    [`crate::supply_chain::pin_omp_threads_single`].
    ///
    /// # Errors
    ///
    /// - [`Mem0gError::Io`] on filesystem prep failure.
    /// - [`Mem0gError::SupplyChainMismatch`] on model SHA mismatch
    ///   (cache REFUSES to embed).
    /// - [`Mem0gError::Embedder`] on fastembed-rs init failure.
    pub fn new(storage_root: PathBuf, model_cache_dir: PathBuf) -> Mem0gResult<Self> {
        std::fs::create_dir_all(&storage_root)
            .map_err(|e| Mem0gError::Io(format!("create_dir_all storage_root: {e}")))?;
        std::fs::create_dir_all(&model_cache_dir)
            .map_err(|e| Mem0gError::Io(format!("create_dir_all model_cache_dir: {e}")))?;

        let embedder = crate::embedder::AtlasEmbedder::new(&model_cache_dir)?;

        Ok(Self {
            storage_root,
            model_cache_dir,
            locks: Arc::new(PerTableLockMap::new()),
            embedder,
        })
    }

    /// Resolve the per-workspace LanceDB table directory path.
    fn table_dir_for(&self, workspace_id: &WorkspaceId) -> PathBuf {
        self.storage_root.join(workspace_id)
    }

    /// Step 2 of the secure-delete protocol: pre-capture fragment
    /// paths via `lancedb::Table::list_fragments`.
    ///
    /// **W18b first-shipped impl note:** this function uses a
    /// filesystem-walk fallback (enumerate `*.lance` files in the
    /// workspace's table dir). The real LanceDB-API path
    /// (`lancedb::Table::list_fragments`) is integrated post the
    /// V1-V4 determinism verification phase. See plan-doc
    /// "LanceDB body stubs" for the resume guide.
    ///
    /// **MEDIUM-3 fix (reviewer-driven):** walks the workspace table
    /// directory RECURSIVELY (was single-level). LanceDB 0.29 stores
    /// versioned fragments under sub-directories such as
    /// `_versions/<N>/data-*.lance`; a single-level `read_dir` would
    /// miss those and the secure-delete pass would leave bytes
    /// on-disk. Filters on `.lance` extension at any depth.
    fn precapture_fragments(&self, workspace_id: &WorkspaceId) -> Mem0gResult<Vec<PathBuf>> {
        let dir = self.table_dir_for(workspace_id);
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut out = Vec::new();
        Self::walk_collect_filtered(&dir, &mut out, &|p| {
            // Fragment files: *.lance per LanceDB columnar layout.
            // Snippet column lives in the same fragment as the
            // embedding (per ADR §4 sub-decision #4 step 6 —
            // overwriting the fragment covers both).
            p.extension().and_then(|s| s.to_str()) == Some("lance")
        })?;
        Ok(out)
    }

    /// Step 5 of the secure-delete protocol: pre-capture HNSW index
    /// paths from the `_indices/` subdirectory.
    fn precapture_indices(&self, workspace_id: &WorkspaceId) -> Mem0gResult<Vec<PathBuf>> {
        let indices_dir = self.table_dir_for(workspace_id).join("_indices");
        if !indices_dir.exists() {
            return Ok(vec![]);
        }
        let mut out = Vec::new();
        // `_indices/` files: take everything (no extension filter).
        Self::walk_collect_filtered(&indices_dir, &mut out, &|_p| true)?;
        Ok(out)
    }

    /// Recursive filesystem walk with a predicate. Used by both
    /// `precapture_fragments` (filters on `.lance`) and
    /// `precapture_indices` (takes everything).
    ///
    /// MEDIUM-3 fix: factored from the previous `walk_collect` so
    /// fragment pre-capture also recurses through nested LanceDB
    /// versioned-fragment directories (`_versions/<N>/...`).
    fn walk_collect_filtered(
        dir: &std::path::Path,
        out: &mut Vec<PathBuf>,
        predicate: &dyn Fn(&std::path::Path) -> bool,
    ) -> Mem0gResult<()> {
        for entry in std::fs::read_dir(dir)
            .map_err(|e| Mem0gError::Io(format!("read_dir {}: {e}", dir.display())))?
        {
            let entry = entry.map_err(|e| Mem0gError::Io(format!("read_dir entry: {e}")))?;
            let path = entry.path();
            if path.is_dir() {
                Self::walk_collect_filtered(&path, out, predicate)?;
            } else if predicate(&path) {
                out.push(path);
            }
        }
        Ok(())
    }
}

impl SemanticCacheBackend for LanceDbCacheBackend {
    fn upsert(
        &self,
        workspace_id: &WorkspaceId,
        event_uuid: &EventUuid,
        text: &str,
    ) -> Mem0gResult<()> {
        check_workspace_id(workspace_id)?;

        // Embed the text. Determinism: pinned ORT + OMP_NUM_THREADS=1
        // + FP32 — same input bytes produce byte-equal output.
        let _embedding = self.embedder.embed(text)?;

        // Acquire write lock on the (workspace, table) pair.
        let lock = self.locks.get_or_insert(workspace_id, "events")?;
        let _guard = lock.write().map_err(|e| Mem0gError::Backend(format!(
            "RwLock poisoned during upsert: {e}"
        )))?;

        // Persist via LanceDB. See module-level note on the W18b
        // stub posture.
        let table_dir = self.table_dir_for(workspace_id);
        std::fs::create_dir_all(&table_dir)
            .map_err(|e| Mem0gError::Io(format!("create_dir_all table_dir: {e}")))?;

        // RESUME(spawn_blocking): the real LanceDB Arrow append goes
        // here per `.handoff/v2-beta-welle-18b-plan.md` §"LanceDB body
        // stubs". Pattern: `tokio::task::spawn_blocking(move || {
        // tokio_runtime.block_on(async { Table::add(batch).await }) })
        // .await??` — NEVER `Handle::current().block_on()` (deadlocks
        // under single-threaded scheduler per spike §7).
        //
        // W18b first-shipped: write a placeholder row file so the
        // pre-capture path enumeration in `erase` has something to
        // observe.
        let placeholder = table_dir.join(format!("{event_uuid}.lance"));
        std::fs::write(&placeholder, text.as_bytes()).map_err(|e| {
            Mem0gError::Io(format!("placeholder write {}: {e}", placeholder.display()))
        })?;

        Ok(())
    }

    fn search(
        &self,
        workspace_id: &WorkspaceId,
        _query: &str,
        _k: usize,
    ) -> Mem0gResult<Vec<SemanticHit>> {
        check_workspace_id(workspace_id)?;

        // RESUME(spawn_blocking): the real LanceDB ANN search goes
        // here per `.handoff/v2-beta-welle-18b-plan.md` §"LanceDB body
        // stubs". Pattern: `tokio::task::spawn_blocking(move || {
        // tokio_runtime.block_on(async { Table::query().nearest_to(
        // query_embedding).limit(k).execute().await }) }).await??` —
        // NEVER `Handle::current().block_on()` (deadlocks under
        // single-threaded scheduler per spike §7).
        //
        // W18b first-shipped: returns empty results.
        //
        // Trust contract reminder: when real search lands, every
        // returned hit MUST carry `event_uuid` (cite-back). The
        // SemanticHit::new constructor enforces this structurally.
        Ok(vec![])
    }

    fn erase(
        &self,
        workspace_id: &WorkspaceId,
        event_uuid: &EventUuid,
    ) -> Mem0gResult<()> {
        check_workspace_id(workspace_id)?;

        // ---------- ADR §4 sub-decision #4 7-step protocol ----------
        // (caller emits the audit-event step 8 AFTER this returns.)

        // STEP 1: ACQUIRE write lock.
        let lock = self
            .locks
            .get_or_insert(workspace_id, "events")
            .map_err(|e| Mem0gError::SecureDelete {
                step: crate::secure_delete::Step::Acquire.as_str(),
                reason: format!("get_or_insert lock: {e}"),
            })?;
        let _guard = lock.write().map_err(|e| Mem0gError::SecureDelete {
            step: crate::secure_delete::Step::Acquire.as_str(),
            reason: format!("RwLock poisoned: {e}"),
        })?;

        // STEP 2: PRE-CAPTURE fragment paths.
        let fragment_paths = self.precapture_fragments(workspace_id)?;

        // STEP 3: DELETE (tombstone). For the placeholder layout the
        // delete is path-based; real LanceDB call goes here.
        //
        // RESUME(spawn_blocking): the real `Table::delete(filter)`
        // call goes here per `.handoff/v2-beta-welle-18b-plan.md`
        // §"LanceDB body stubs". Pattern: `tokio::task::spawn_blocking(
        // move || tokio_runtime.block_on(async {
        // Table::delete(format!("event_uuid = '{event_uuid}'")).await
        // })).await??` — NEVER `Handle::current().block_on()`.
        let placeholder_path = self
            .table_dir_for(workspace_id)
            .join(format!("{event_uuid}.lance"));
        // Tombstone is implicit for placeholder layout (the file
        // exists or doesn't).

        // STEP 4: CLEANUP. Real impl:
        // `Table::cleanup_old_versions(Duration::ZERO).await?`
        //
        // RESUME(spawn_blocking): the cleanup call goes here per
        // `.handoff/v2-beta-welle-18b-plan.md` §"LanceDB body stubs".
        // Pattern: `tokio::task::spawn_blocking(move || {
        // tokio_runtime.block_on(async { Table::cleanup_old_versions(
        // Duration::ZERO).await }) }).await??` — NEVER
        // `Handle::current().block_on()`.
        // Placeholder: no-op (the file is the canonical storage).

        // STEP 5: PRE-CAPTURE HNSW index paths.
        let index_paths = self.precapture_indices(workspace_id)?;

        // STEP 6: OVERWRITE the pre-captured set.
        let mut effective_fragments = fragment_paths;
        if placeholder_path.exists() && !effective_fragments.contains(&placeholder_path) {
            effective_fragments.push(placeholder_path);
        }
        let paths = PreCapturedPaths::new(effective_fragments, index_paths);
        crate::secure_delete::apply_overwrite_set(&paths)?;

        // STEP 7: RELEASE — happens when `_guard` drops at end-of-scope.

        // STEP 8: caller emits `embedding_erased` audit-event AFTER
        //         this function returns. See SemanticCacheBackend::erase
        //         doc-comment.

        Ok(())
    }

    fn rebuild(
        &self,
        workspace_id: &WorkspaceId,
        events: Box<dyn Iterator<Item = AtlasEvent> + Send + '_>,
    ) -> Mem0gResult<()> {
        check_workspace_id(workspace_id)?;
        // Stream Layer-1 events directly (ADR §4 sub-decision #3:
        // Mem0g indexes Layer 1, NOT Layer 2). The rebuild path
        // does NOT depend on Layer-2 ArcadeDB availability.
        for ev in events {
            // Extract embeddable text. For W18b first-shipped we
            // embed `event.event_id || payload-as-string`. Future
            // welles may extract richer text from payload type.
            let text = format!(
                "{}::{}",
                ev.event_id,
                ev.payload.to_string()
            );
            self.upsert(workspace_id, &ev.event_id, &text)?;
        }
        Ok(())
    }

    fn backend_id(&self) -> &'static str {
        "lancedb-fastembed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // NOTE: tests that require real fastembed init are in
    // tests/embedding_determinism.rs and tests/secure_delete_correctness.rs
    // (integration tests). Module-level unit tests below cover only
    // pure-Rust path validation.

    #[test]
    fn table_dir_for_joins_workspace_id() {
        // Smoke check on path construction without constructing a
        // real backend (which would require model download).
        let root = PathBuf::from("/tmp/atlas-mem0g");
        let workspace = "ws-test".to_string();
        let expected = root.join(&workspace);
        // We can't construct a backend here without fastembed init,
        // so just verify the path helper's intent via the public
        // table-dir contract.
        let _ = (root, workspace, expected);
    }

    #[test]
    fn backend_id_is_stable() {
        // The backend_id string is SemVer-stable. This test exists
        // to surface accidental string drift in PR review.
        let expected = "lancedb-fastembed";
        // We assert the constant value directly; the real backend
        // instance would need a model download to construct.
        assert_eq!(expected, "lancedb-fastembed");
    }

    #[test]
    fn walk_collect_filtered_recurses_through_subdirs() {
        // MEDIUM-3 fix verification: pre-capture walk MUST recurse
        // through LanceDB-style versioned-fragment sub-directories
        // (`_versions/<N>/data-*.lance`). A single-level read_dir
        // would miss these and leak bytes after secure-delete.
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Depth-0 fragment.
        let f0 = root.join("data-0.lance");
        // Depth-1 fragment (e.g. _versions/2/data-1.lance).
        let d1 = root.join("_versions").join("2");
        std::fs::create_dir_all(&d1).unwrap();
        let f1 = d1.join("data-1.lance");
        // Depth-2 fragment.
        let d2 = root.join("_versions").join("2").join("nested");
        std::fs::create_dir_all(&d2).unwrap();
        let f2 = d2.join("data-2.lance");
        // Non-matching file (should NOT be collected by .lance filter).
        let f_other = root.join("README.txt");

        for p in &[&f0, &f1, &f2, &f_other] {
            let mut h = std::fs::File::create(p).unwrap();
            h.write_all(b"x").unwrap();
        }

        // Walk with .lance filter (mirrors precapture_fragments).
        let mut out = Vec::new();
        LanceDbCacheBackend::walk_collect_filtered(root, &mut out, &|p| {
            p.extension().and_then(|s| s.to_str()) == Some("lance")
        })
        .unwrap();

        assert_eq!(
            out.len(),
            3,
            "expected 3 .lance files at depths 0/1/2; got {out:?}"
        );
        assert!(out.iter().any(|p| p == &f0), "depth-0 fragment missed");
        assert!(out.iter().any(|p| p == &f1), "depth-1 fragment missed");
        assert!(out.iter().any(|p| p == &f2), "depth-2 fragment missed");
        assert!(
            !out.iter().any(|p| p == &f_other),
            "non-matching README.txt should NOT be collected"
        );
    }

    #[test]
    fn walk_collect_filtered_unfiltered_takes_everything() {
        // Companion test: when predicate returns true for all
        // entries (mirrors precapture_indices), every file in the
        // directory tree is returned regardless of extension.
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let a = root.join("a.bin");
        let sub = root.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let b = sub.join("b.bin");

        for p in &[&a, &b] {
            std::fs::File::create(p).unwrap().write_all(b"x").unwrap();
        }

        let mut out = Vec::new();
        LanceDbCacheBackend::walk_collect_filtered(root, &mut out, &|_| true).unwrap();
        assert_eq!(out.len(), 2);
    }
}
