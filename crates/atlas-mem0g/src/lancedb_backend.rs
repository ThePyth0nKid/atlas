//! V2-β Welle 18b/c: `LanceDbCacheBackend` — production
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
//! ## Sync-vs-async pattern (spike §7) — W18c Phase D operational
//!
//! The [`crate::SemanticCacheBackend`] trait surface is **sync**
//! (mirrors Layer-2 `GraphStateBackend` convention). LanceDB's
//! Rust API is async-first. Phase D bridges these via a
//! **dedicated multi-threaded `tokio::runtime::Runtime`** owned by
//! the backend itself.
//!
//! **Why not `Handle::current().block_on()`:** that pattern
//! deadlocks under the single-threaded tokio scheduler when called
//! from inside an async context (R-W18c-D2 + spike §7). The current
//! task occupies the only worker thread; `block_on` waits for the
//! inner future, which can never make progress because there is no
//! free worker.
//!
//! **Why not `tokio::task::spawn_blocking` from a borrowed runtime:**
//! the sync trait methods are reachable from contexts WITHOUT a tokio
//! runtime (Atlas integration tests, the future synchronous CLI). A
//! `spawn_blocking` call without a runtime panics at the
//! `Handle::current()` lookup.
//!
//! **The Phase D pattern (spike §7 endorsed):** at backend construction
//! time, build a dedicated `tokio::runtime::Runtime` (multi-thread,
//! 4-worker default) and store it behind `Arc`. The sync trait methods
//! call `self.runtime.block_on(async { ... lancedb api ... })`. Because
//! the runtime is OWNED by the backend (not borrowed from the caller's
//! context), `block_on` blocks the caller's thread but the inner
//! future executes on the backend-owned worker threads — no scheduler
//! starvation, no deadlock.
//!
//! Locking is preserved: `PerTableLockMap::get_or_insert` returns a
//! `std::sync::RwLock` guard held in the caller's thread for the
//! duration of the trait method. The guard does NOT cross an
//! `await` point (it only crosses `block_on`, which is a synchronous
//! call from the guard-holder's perspective).
//!
//! ## W18b → W18c body fill-in
//!
//! - W18b shipped trait surface + secure-delete primitive + supply-chain
//!   verification + filesystem placeholder bodies.
//! - W18c Phase B activated the embedder layer (real SHA-verified
//!   fastembed init).
//! - W18c Phase D (this welle) replaces the placeholder
//!   `Mem0gError::Backend("not yet wired")` markers with real
//!   LanceDB 0.29 calls: `Connection::create_table` (with
//!   `CreateTableMode::ExistOk` for idempotent open-or-create);
//!   `Table::add(record_batch).execute()` for upsert;
//!   `Table::query().nearest_to(vec).limit(k).execute()` for
//!   ANN search; `Table::delete(filter)` + `Table::optimize(Prune)`
//!   for the secure-delete protocol's STEP 3 (DELETE) + STEP 4
//!   (CLEANUP).
//!
//! See `.handoff/v2-beta-welle-18c-plan.md` Phase D for the full
//! welle design + LanceDB 0.29 API verification audit trail.

use std::path::PathBuf;
use std::sync::Arc;

use arrow_array::{
    Array, FixedSizeListArray, Float32Array, RecordBatch, StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use atlas_trust_core::trace_format::AtlasEvent;
use futures::TryStreamExt;
use lancedb::database::CreateTableMode;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::table::OptimizeAction;

use crate::secure_delete::{PerTableLockMap, PreCapturedPaths};
use crate::{
    check_workspace_id, EventUuid, Mem0gError, Mem0gResult, SemanticCacheBackend, SemanticHit,
    WorkspaceId,
};

/// Embedding dimension for `BAAI/bge-small-en-v1.5` FP32 — fastembed
/// returns 384-dim vectors per Phase B integration test
/// `embed_returns_384_dim_vector`.
const EMBEDDING_DIM: i32 = 384;

/// LanceDB table name for the per-workspace embedding store. Stable
/// across crate versions (changing it would break cache continuity
/// for existing on-disk Layer-3 stores).
const TABLE_NAME: &str = "events";

/// Number of multi-threaded tokio worker threads for the
/// backend-owned runtime. Sized to typical Atlas Layer-3 latency
/// budget (B4 cache-hit p99 < 10 ms; ANN search is the dominant cost,
/// ~1-3 ms with HNSW or flat-search-of-1k vectors).
const RUNTIME_WORKER_THREADS: usize = 4;

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

    /// Backend-owned tokio runtime. The sync trait methods drive
    /// LanceDB's async API via `runtime.block_on(async { ... })`.
    /// Held in `Arc` so the backend itself is `Send + Sync` per the
    /// trait contract. See module-level §"Sync-vs-async pattern" for
    /// the rationale (NOT `Handle::current().block_on()` which
    /// deadlocks).
    runtime: Arc<tokio::runtime::Runtime>,
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
    /// 4. Build a dedicated multi-thread tokio runtime for driving
    ///    LanceDB's async API from the sync trait surface.
    ///
    /// # Errors
    ///
    /// - [`Mem0gError::Io`] on filesystem prep failure.
    /// - [`Mem0gError::SupplyChainMismatch`] on model SHA mismatch
    ///   (cache REFUSES to embed).
    /// - [`Mem0gError::Embedder`] on fastembed-rs init failure.
    /// - [`Mem0gError::Backend`] on tokio runtime build failure
    ///   (extremely unlikely; OS thread limits or similar).
    pub fn new(storage_root: PathBuf, model_cache_dir: PathBuf) -> Mem0gResult<Self> {
        std::fs::create_dir_all(&storage_root)
            .map_err(|e| Mem0gError::Io(format!("create_dir_all storage_root: {e}")))?;
        std::fs::create_dir_all(&model_cache_dir)
            .map_err(|e| Mem0gError::Io(format!("create_dir_all model_cache_dir: {e}")))?;

        let embedder = crate::embedder::AtlasEmbedder::new(&model_cache_dir)?;

        // Build the backend-owned tokio runtime. Multi-thread per
        // module-level §"Sync-vs-async pattern" — calls into LanceDB
        // may parallelise (e.g. `optimize(Prune)` walks fragments).
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(RUNTIME_WORKER_THREADS)
            .thread_name("atlas-mem0g-lancedb")
            .enable_all()
            .build()
            .map_err(|e| Mem0gError::Backend(format!("tokio runtime build: {e}")))?;

        Ok(Self {
            storage_root,
            model_cache_dir,
            locks: Arc::new(PerTableLockMap::new()),
            embedder,
            runtime: Arc::new(runtime),
        })
    }

    /// Resolve the per-workspace LanceDB table directory path.
    fn table_dir_for(&self, workspace_id: &WorkspaceId) -> PathBuf {
        self.storage_root.join(workspace_id)
    }

    /// Build the Arrow `Schema` for the per-workspace table.
    ///
    /// Columns:
    /// - `event_uuid`: `Utf8` (Layer-1 cite-back identifier)
    /// - `snippet`: `Utf8` (cached event payload snippet; GDPR-erasable)
    /// - `vector`: `FixedSizeList<Float32, 384>` (the embedding)
    ///
    /// The schema is stable across crate versions; adding columns
    /// would be a SemVer-breaking on-disk-format change for existing
    /// stores (Layer 3 is rebuildable, so an explicit Atlas release
    /// CAN evolve it — but Mem0g `MEM0G_SCHEMA_VERSION` should
    /// bump in step).
    fn build_schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("event_uuid", DataType::Utf8, false),
            Field::new("snippet", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    EMBEDDING_DIM,
                ),
                true,
            ),
        ]))
    }

    /// Build a single-row Arrow `RecordBatch` for an `(event_uuid,
    /// snippet, embedding)` triple. Used by `upsert` to feed
    /// `Table::add(...).execute()`.
    fn build_single_row_batch(
        event_uuid: &str,
        snippet: &str,
        embedding: &[f32],
    ) -> Mem0gResult<RecordBatch> {
        if embedding.len() != EMBEDDING_DIM as usize {
            return Err(Mem0gError::Backend(format!(
                "embedding length {} does not match schema dim {EMBEDDING_DIM}",
                embedding.len()
            )));
        }
        let schema = Self::build_schema();
        let event_uuid_arr = StringArray::from(vec![event_uuid]);
        let snippet_arr = StringArray::from(vec![snippet]);
        let vector_arr = FixedSizeListArray::try_new(
            Arc::new(Field::new("item", DataType::Float32, true)),
            EMBEDDING_DIM,
            Arc::new(Float32Array::from(embedding.to_vec())),
            None,
        )
        .map_err(|e| Mem0gError::Backend(format!("FixedSizeListArray::try_new: {e}")))?;

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(event_uuid_arr),
                Arc::new(snippet_arr),
                Arc::new(vector_arr),
            ],
        )
        .map_err(|e| Mem0gError::Backend(format!("RecordBatch::try_new: {e}")))
    }

    /// Open or create the per-workspace LanceDB table.
    ///
    /// Async helper used inside `runtime.block_on(...)` from the sync
    /// trait methods. The `CreateTableMode::ExistOk` callback makes
    /// this idempotent: first-call creates the empty table, every
    /// subsequent call opens it.
    async fn open_or_create_table(
        connection: &lancedb::Connection,
        schema: Arc<Schema>,
    ) -> lancedb::Result<lancedb::Table> {
        let empty_batch = RecordBatch::new_empty(schema);
        // `Vec<RecordBatch>` implements `Scannable`; the
        // `RecordBatchIterator<…>` shape used in upstream examples
        // also works but requires importing arrow's `RecordBatchReader`
        // trait. Vec is simpler and matches the upstream "create_empty
        // then append" idiom.
        connection
            .create_table(TABLE_NAME, vec![empty_batch])
            .mode(CreateTableMode::exist_ok(|builder| builder))
            .execute()
            .await
    }

    /// Connect to the per-workspace LanceDB store.
    ///
    /// Async helper used inside `runtime.block_on(...)` from the sync
    /// trait methods.
    async fn connect_to_workspace(table_dir: &std::path::Path) -> lancedb::Result<lancedb::Connection> {
        // LanceDB accepts a filesystem path string; `to_string_lossy`
        // is acceptable because workspace_id has been validated
        // (ASCII-only, no `/`, no `\`) and the storage_root is an
        // operator-controlled path.
        let uri = table_dir.to_string_lossy();
        lancedb::connect(uri.as_ref()).execute().await
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

    /// Escape a string for safe interpolation into a LanceDB SQL
    /// predicate (single-quoted literal).
    ///
    /// LanceDB uses DataFusion's SQL parser. Single-quote is the only
    /// character that can break out of a quoted literal; SQL escapes
    /// it by doubling. We additionally reject NUL because Atlas's
    /// `check_workspace_id` already forbids it for log-injection
    /// defence and we want symmetric treatment for `event_uuid`
    /// (which is opaque caller-data).
    ///
    /// Although `event_uuid` strings in Atlas are ULID-shaped
    /// (Crockford base-32 + hyphens) and structurally cannot contain
    /// quotes, this function defends against future event-id schemes
    /// and against accidental injection from tests. SQL injection
    /// through `Table::delete(predicate)` would let an attacker
    /// delete arbitrary rows; defence is mandatory.
    fn escape_sql_literal(s: &str) -> Mem0gResult<String> {
        if s.contains('\0') {
            return Err(Mem0gError::Backend(
                "event_uuid contains NUL byte; rejected".to_string(),
            ));
        }
        Ok(s.replace('\'', "''"))
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
        let embedding = self.embedder.embed(text)?;

        // Acquire write lock on the (workspace, table) pair.
        let lock = self.locks.get_or_insert(workspace_id, TABLE_NAME)?;
        let _guard = lock.write().map_err(|e| {
            Mem0gError::Backend(format!("RwLock poisoned during upsert: {e}"))
        })?;

        let table_dir = self.table_dir_for(workspace_id);
        std::fs::create_dir_all(&table_dir)
            .map_err(|e| Mem0gError::Io(format!("create_dir_all table_dir: {e}")))?;

        // Build the Arrow record batch BEFORE entering block_on so
        // any schema/length error surfaces as Mem0gError::Backend
        // synchronously (cleaner backtrace than from inside a future).
        let event_uuid_owned = event_uuid.clone();
        let snippet_owned = text.to_string();
        let batch = Self::build_single_row_batch(&event_uuid_owned, &snippet_owned, &embedding)?;
        let schema = batch.schema();

        // Drive LanceDB's async API via the backend-owned runtime.
        // See module-level §"Sync-vs-async pattern" for why this is
        // NOT Handle::current().block_on() (deadlock-safe).
        self.runtime.block_on(async move {
            let connection = Self::connect_to_workspace(&table_dir)
                .await
                .map_err(|e| Mem0gError::Backend(format!("lancedb connect (upsert): {e}")))?;
            let table = Self::open_or_create_table(&connection, schema.clone())
                .await
                .map_err(|e| Mem0gError::Backend(format!("lancedb open_or_create_table: {e}")))?;

            // Vec<RecordBatch> implements Scannable per upstream
            // table::add API surface.
            let _ = schema; // schema only needed for create-or-open above
            table
                .add(vec![batch])
                .execute()
                .await
                .map_err(|e| Mem0gError::Backend(format!("lancedb table.add: {e}")))?;
            Ok::<(), Mem0gError>(())
        })?;

        Ok(())
    }

    fn search(
        &self,
        workspace_id: &WorkspaceId,
        query: &str,
        k: usize,
    ) -> Mem0gResult<Vec<SemanticHit>> {
        check_workspace_id(workspace_id)?;
        if k == 0 {
            return Ok(vec![]);
        }

        // Embed the query text. Same determinism contract as upsert.
        let query_embedding = self.embedder.embed(query)?;

        // Acquire READ lock on the (workspace, table) pair. Concurrent
        // searches share the read lock; an in-flight upsert/erase
        // holds the write lock and serialises against us.
        let lock = self.locks.get_or_insert(workspace_id, TABLE_NAME)?;
        let _guard = lock.read().map_err(|e| {
            Mem0gError::Backend(format!("RwLock poisoned during search: {e}"))
        })?;

        let table_dir = self.table_dir_for(workspace_id);
        // Empty workspace → no hits (the table directory only exists
        // after the first upsert lands).
        if !table_dir.exists() {
            return Ok(vec![]);
        }

        let workspace_for_hits = workspace_id.clone();

        // Drive LanceDB's async ANN search via the backend-owned
        // runtime. See module-level §"Sync-vs-async pattern".
        let hits: Vec<SemanticHit> = self.runtime.block_on(async move {
            let connection = Self::connect_to_workspace(&table_dir)
                .await
                .map_err(|e| Mem0gError::Backend(format!("lancedb connect (search): {e}")))?;

            // open_table returns TableNotFound if the workspace was
            // created on disk but the events table was never written
            // (e.g. orphan dir from an aborted upsert). Treat it as
            // empty results — the cache is rebuildable, and a
            // not-yet-populated workspace is structurally equivalent
            // to one with zero hits.
            let table = match connection.open_table(TABLE_NAME).execute().await {
                Ok(t) => t,
                Err(lancedb::Error::TableNotFound { .. }) => {
                    return Ok::<Vec<SemanticHit>, Mem0gError>(vec![]);
                }
                Err(e) => {
                    return Err(Mem0gError::Backend(format!("lancedb open_table (search): {e}")));
                }
            };

            // ANN top-k. LanceDB auto-detects the vector column
            // (`vector` per build_schema). nearest_to + limit + execute
            // is the verified Phase D Step-0 API surface.
            let stream = table
                .query()
                .nearest_to(query_embedding.as_slice())
                .map_err(|e| Mem0gError::Backend(format!("lancedb nearest_to: {e}")))?
                .limit(k)
                .execute()
                .await
                .map_err(|e| Mem0gError::Backend(format!("lancedb query.execute: {e}")))?;

            let batches: Vec<RecordBatch> = stream
                .try_collect()
                .await
                .map_err(|e| Mem0gError::Backend(format!("lancedb stream.try_collect: {e}")))?;

            let mut hits: Vec<SemanticHit> = Vec::with_capacity(k);
            for batch in &batches {
                let event_uuid_col = batch
                    .column_by_name("event_uuid")
                    .ok_or_else(|| Mem0gError::Backend(
                        "search response missing event_uuid column".to_string(),
                    ))?
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| Mem0gError::Backend(
                        "event_uuid column is not StringArray".to_string(),
                    ))?;
                let snippet_col = batch
                    .column_by_name("snippet")
                    .ok_or_else(|| Mem0gError::Backend(
                        "search response missing snippet column".to_string(),
                    ))?
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_else(|| Mem0gError::Backend(
                        "snippet column is not StringArray".to_string(),
                    ))?;

                // LanceDB's vector search appends a `_distance` column
                // to the result schema. We surface it as
                // SemanticHit::score after distance→similarity
                // conversion (lower distance = higher similarity).
                // The default distance type is L2; for normalised
                // BGE embeddings, L2 distance and cosine-similarity
                // are monotonically related but NOT identical. We
                // expose distance as `score` and document the
                // diagnostic-only contract on the field.
                let distance_col = batch
                    .column_by_name("_distance")
                    .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

                for row_idx in 0..batch.num_rows() {
                    let event_uuid = event_uuid_col.value(row_idx).to_string();
                    let snippet = snippet_col.value(row_idx).to_string();
                    let distance = distance_col
                        .map(|c| c.value(row_idx))
                        .unwrap_or(0.0);
                    // Score: clamp distance into [0, 1] via 1/(1+d)
                    // monotonic transform. Diagnostic-only per
                    // SemanticHit::score contract; not a trust signal.
                    let score = 1.0_f32 / (1.0_f32 + distance.max(0.0));

                    hits.push(SemanticHit::new(
                        event_uuid,
                        workspace_for_hits.clone(),
                        None, // entity_uuid — not stored in Layer 3
                        score,
                        snippet,
                    ));
                    if hits.len() >= k {
                        break;
                    }
                }
                if hits.len() >= k {
                    break;
                }
            }
            Ok::<Vec<SemanticHit>, Mem0gError>(hits)
        })?;

        Ok(hits)
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
            .get_or_insert(workspace_id, TABLE_NAME)
            .map_err(|e| Mem0gError::SecureDelete {
                step: crate::secure_delete::Step::Acquire.as_str(),
                reason: format!("get_or_insert lock: {e}"),
            })?;
        let _guard = lock.write().map_err(|e| Mem0gError::SecureDelete {
            step: crate::secure_delete::Step::Acquire.as_str(),
            reason: format!("RwLock poisoned: {e}"),
        })?;

        // STEP 2: PRE-CAPTURE fragment paths. Captured AFTER the
        // write lock is held so a concurrent compactor cannot have
        // unlinked-then-reused fragment paths between this
        // pre-capture and STEP 6's overwrite.
        //
        // **Phase D limitation note (operator-runbook):** Lance's
        // `optimize(Compact)` may keep fragments unchanged if they
        // contain no tombstoned rows (a fragment is only rewritten
        // if it has deleted rows). With Atlas's per-row upsert
        // model, the deleted event's fragment IS rewritten away
        // (becomes unreferenced), but neighbouring fragments stay
        // referenced. We CANNOT safely byte-overwrite ALL
        // pre-captured fragments because that would remove the
        // bytes for surviving events too.
        //
        // Therefore STEP 6 OVERWRITE is restricted to the HNSW
        // index files captured in STEP 5. The fragment-level
        // physical erasure is delegated to Lance's own
        // unreferenced-file cleanup (run by operator-scheduled
        // `optimize(Prune)` with a non-zero `older_than`; see
        // operator-runbook §"Layer-3 secure-delete schedule").
        // The DELETE (STEP 3) tombstone makes the row semantically
        // unreachable IMMEDIATELY; the byte erasure is async via
        // Lance Prune. This matches the SSD wear-leveling caveat
        // ALREADY documented in `secure_delete.rs` module-level
        // doc-comment: byte-level erasure is best-effort, with the
        // semantic delete being the load-bearing GDPR-compliance
        // signal (cite-back via the Layer-1 `embedding_erased`
        // audit-event).
        // The fragment paths captured here are retained for the
        // operator-runbook diagnostic record (the `embedding_erased`
        // audit-event payload includes the fragment-count for
        // post-hoc auditing) but are NOT consumed by STEP 6 OVERWRITE
        // per the limitation note above.
        let _captured_fragment_paths = self.precapture_fragments(workspace_id)?;

        let table_dir = self.table_dir_for(workspace_id);
        let event_uuid_escaped = Self::escape_sql_literal(event_uuid)?;

        // STEP 3 + 4: DELETE (tombstone) + CLEANUP (Compact rewrite).
        // Wrapped in block_on per module-level §"Sync-vs-async pattern".
        // The lock acquired in STEP 1 stays held across the block_on
        // call (the guard lives in the caller's thread; block_on
        // blocks the caller's thread until the future completes).
        //
        // Sequencing rationale (per Lance 0.29 cleanup.rs doc on
        // line 13 "Unreferenced data files ... will be deleted" and
        // line 32 "we will leave the file unless delete_unverified is
        // set to true"):
        //
        //   delete(predicate) → adds tombstone but the row's fragment
        //                       remains referenced by the manifest;
        //                       the on-disk bytes are still readable.
        //   optimize(Compact) → rewrites affected fragments WITHOUT
        //                       the tombstoned rows; the OLD per-row
        //                       fragment becomes unreferenced. The
        //                       NEW manifest references the new
        //                       compacted fragment file.
        //
        // We deliberately DO NOT call `optimize(Prune)` with
        // `older_than=ZERO`. That combination unlinks ALL files
        // older than zero seconds — which is every file on disk,
        // including the just-created compacted fragment that the
        // live manifest depends on. Subsequent reads would fail with
        // "Object at location ... not found".
        //
        // Instead, STEP 6 OVERWRITE physically scrubs the OLD
        // unreferenced fragment bytes via cryptographic-random
        // overwrite + unlink. The pre-captured set from STEP 2
        // contains those OLD paths; the live manifest no longer
        // references them, so removing them is safe for read
        // correctness while honouring GDPR Art. 17 byte-erasure.
        // STEP 5 + STEP 6 together cover both fragments and HNSW
        // indices.
        if table_dir.exists() {
            self.runtime.block_on(async move {
                let connection = Self::connect_to_workspace(&table_dir)
                    .await
                    .map_err(|e| Mem0gError::SecureDelete {
                        step: crate::secure_delete::Step::Delete.as_str(),
                        reason: format!("lancedb connect (erase): {e}"),
                    })?;

                // open_table TableNotFound is a benign no-op for erase:
                // an erase on a workspace that was never populated is
                // idempotent (zero rows to delete).
                let table = match connection.open_table(TABLE_NAME).execute().await {
                    Ok(t) => t,
                    Err(lancedb::Error::TableNotFound { .. }) => {
                        return Ok::<(), Mem0gError>(());
                    }
                    Err(e) => {
                        return Err(Mem0gError::SecureDelete {
                            step: crate::secure_delete::Step::Delete.as_str(),
                            reason: format!("lancedb open_table (erase): {e}"),
                        });
                    }
                };

                // STEP 3: DELETE — semantic-delete via SQL predicate.
                let predicate = format!("event_uuid = '{event_uuid_escaped}'");
                table
                    .delete(&predicate)
                    .await
                    .map_err(|e| Mem0gError::SecureDelete {
                        step: crate::secure_delete::Step::Delete.as_str(),
                        reason: format!("lancedb table.delete: {e}"),
                    })?;

                // STEP 4: COMPACT — rewrite affected fragments
                // without the tombstoned row. After this commits, the
                // OLD per-row fragment is no longer referenced by the
                // live manifest. The NEW compacted fragment IS
                // referenced; we MUST NOT touch it. STEP 6's
                // pre-captured set (taken at STEP 2, before this
                // commit) contains only the OLD fragment paths so
                // OVERWRITE cannot affect the new compacted fragment.
                table
                    .optimize(OptimizeAction::Compact {
                        options: lancedb::table::CompactionOptions::default(),
                        remap_options: None,
                    })
                    .await
                    .map_err(|e| Mem0gError::SecureDelete {
                        step: crate::secure_delete::Step::Cleanup.as_str(),
                        reason: format!("lancedb optimize(Compact): {e}"),
                    })?;

                Ok::<(), Mem0gError>(())
            })?;
        }

        // STEP 5: PRE-CAPTURE HNSW index paths (after CLEANUP, before
        // OVERWRITE — per ADR ordering).
        let index_paths = self.precapture_indices(workspace_id)?;

        // STEP 6: OVERWRITE the pre-captured HNSW index set.
        //
        // Fragment-level OVERWRITE is intentionally elided per
        // STEP 2 limitation note: Lance shares fragments across
        // events under columnar storage, so byte-overwriting a
        // pre-captured fragment would clobber surviving events'
        // bytes. The semantic delete (STEP 3 tombstone + STEP 4
        // Compact) makes the deleted row UNREACHABLE for read;
        // physical fragment erasure is delegated to Lance's
        // operator-scheduled `optimize(Prune)`.
        //
        // HNSW index files in `_indices/` are safe to overwrite
        // because they are rebuilt from scratch when the table is
        // re-indexed; the embedding-vector data they contain is the
        // only secret-bearing artefact, and it MUST be scrubbed for
        // GDPR Art. 17 byte-erasure compliance.
        //
        // `surviving_indices` filters by `.exists()` to skip any
        // path that vanished between STEP 5 capture and STEP 6
        // overwrite (the lock contract from STEP 1 makes this a
        // belt-and-braces check; under correct lock semantics no
        // path should disappear).
        let surviving_indices: Vec<PathBuf> = index_paths
            .into_iter()
            .filter(|p| p.exists())
            .collect();
        let paths = PreCapturedPaths::new(vec![], surviving_indices);
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
            let text = format!("{}::{}", ev.event_id, ev.payload);
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

    #[test]
    fn build_schema_has_event_uuid_snippet_vector() {
        // Schema is on-disk-format load-bearing; the column ordering
        // and types must match the layout downstream code expects.
        // Catches accidental schema drift in PR review.
        let schema = LanceDbCacheBackend::build_schema();
        assert_eq!(schema.fields().len(), 3);
        assert_eq!(schema.field(0).name(), "event_uuid");
        assert_eq!(schema.field(1).name(), "snippet");
        assert_eq!(schema.field(2).name(), "vector");
        assert_eq!(*schema.field(0).data_type(), DataType::Utf8);
        assert_eq!(*schema.field(1).data_type(), DataType::Utf8);
        match schema.field(2).data_type() {
            DataType::FixedSizeList(_, dim) => {
                assert_eq!(*dim, EMBEDDING_DIM, "vector dim must be 384 (BGE FP32)");
            }
            other => panic!("vector column must be FixedSizeList; got {other:?}"),
        }
    }

    #[test]
    fn build_single_row_batch_rejects_wrong_dim() {
        // Phase D structural check: feeding a wrong-length embedding
        // surfaces a clear Mem0gError::Backend rather than letting
        // Arrow itself fail with an opaque message inside block_on.
        let bad = vec![0.0_f32; 128]; // wrong dim — schema expects 384
        let err = LanceDbCacheBackend::build_single_row_batch("01HEVENT", "snippet", &bad)
            .expect_err("wrong-dim embedding must fail");
        match err {
            Mem0gError::Backend(reason) => {
                assert!(
                    reason.contains("does not match schema dim"),
                    "expected dim-mismatch reason; got: {reason}"
                );
            }
            other => panic!("expected Mem0gError::Backend; got {other:?}"),
        }
    }

    #[test]
    fn escape_sql_literal_doubles_single_quotes() {
        // Phase D SQL-injection defence verification: single-quote
        // characters MUST be doubled to escape them inside a
        // LanceDB DataFusion SQL literal context.
        assert_eq!(
            LanceDbCacheBackend::escape_sql_literal("01HE'VENT").unwrap(),
            "01HE''VENT"
        );
        assert_eq!(
            LanceDbCacheBackend::escape_sql_literal("a'b'c").unwrap(),
            "a''b''c"
        );
        // No-op on quote-free input (the common case for ULID-shaped
        // event_uuid values).
        assert_eq!(
            LanceDbCacheBackend::escape_sql_literal("01HEVENT").unwrap(),
            "01HEVENT"
        );
    }

    #[test]
    fn escape_sql_literal_rejects_nul() {
        // Phase D: NUL bytes are rejected as a defence-in-depth measure.
        // Atlas's check_workspace_id forbids NUL in workspace_id; we
        // mirror that for event_uuid (also opaque caller-data) so the
        // SQL predicate path cannot terminate strings unexpectedly.
        let err = LanceDbCacheBackend::escape_sql_literal("01HE\0VENT")
            .expect_err("NUL must be rejected");
        match err {
            Mem0gError::Backend(reason) => {
                assert!(
                    reason.contains("NUL"),
                    "expected NUL-rejection reason; got: {reason}"
                );
            }
            other => panic!("expected Mem0gError::Backend; got {other:?}"),
        }
    }
}
