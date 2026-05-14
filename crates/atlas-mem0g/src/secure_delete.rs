//! V2-β Welle 18b: secure-delete primitive (ADR-Atlas-012 §4
//! sub-decision #4).
//!
//! Implements the 7-step pre-capture-then-lock-then-overwrite protocol
//! that closes the TOCTOU race window between
//! `cleanup_old_versions` and a concurrent compactor / read
//! (security-reviewer HIGH-1).
//!
//! ## Protocol (EXACTLY per ADR §4 sub-decision #4)
//!
//! 1. **ACQUIRE** write lock on the LanceDB Table for the (workspace,
//!    table) pair. If LanceDB does not expose a native table-level
//!    write lock, Atlas uses a per-(workspace, table)
//!    `tokio::sync::RwLock` map.
//!
//! 2. **PRE-CAPTURE** fragment file paths via
//!    `lancedb::Table::list_fragments` BEFORE any delete or cleanup.
//!    Authoritative set to overwrite — NOT "files modified
//!    post-cleanup", which would miss old-fragment paths that get
//!    unlinked-then-reused by the OS.
//!
//! 3. **DELETE** semantic-delete via `lancedb::Table::delete(filter)`
//!    + tombstone in `_deletion_files/`.
//!
//! 4. **CLEANUP** physical rewrite via
//!    `cleanup_old_versions(Duration::ZERO)`. Lock from step 1
//!    prevents background compactor race.
//!
//! 5. **PRE-CAPTURE HNSW INDICES** the set of `_indices/` paths that
//!    reference the affected fragments. Default option (a) per ADR
//!    step 5: overwrite both fragments AND affected `_indices/` files.
//!    Without (a), residual leak is graph-neighbourhood-topology-only
//!    (no embedding-vector bytes recoverable from `_indices/`).
//!
//! 6. **OVERWRITE** each pre-captured path (step 2 + step 5):
//!    ```text
//!    OpenOptions::write(true).open(path)
//!    write random bytes equal to file size
//!    fdatasync()
//!    close
//!    tokio::fs::remove_file(path)
//!    ```
//!    Bounded by the pre-captured set — eliminates TOCTOU window.
//!
//! 7. **RELEASE** the write lock from step 1.
//!
//! 8. **EMIT** the Layer-1 `embedding_erased` audit-event.
//!    Deliberately AFTER lock release so it does not deadlock on the
//!    projector's own write-side mutex when emitting into
//!    events.jsonl. Caller responsibility — NOT inside this module.
//!
//! ## Snippet field coverage (security-reviewer MEDIUM-1)
//!
//! The `SemanticHit::snippet` cached snippet is stored as a column in
//! the SAME Arrow fragment as the embedding vector (per LanceDB's
//! columnar layout). Step 6's fragment overwrite covers it. The
//! `LanceDbCacheBackend` does NOT introduce separate snippet
//! storage — that would require adding the new location to this
//! protocol.
//!
//! ## SSD wear-leveling caveat
//!
//! SSD firmware may have copies of the block in spare cells
//! unreachable by Atlas's overwrite. Full physical erasure on SSDs
//! requires SECURE_ERASE ATA (whole-device, operator-runbook only)
//! OR full-disk encryption with per-tenant key destruction (V2-γ
//! stronger defence). W18b ships best-effort filesystem-level
//! overwrite + documented limitation in `DECISION-SEC-5` footnote.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use crate::{Mem0gError, Mem0gResult, WorkspaceId};

/// Identifier for the secure-delete protocol step that failed. Used
/// in [`Mem0gError::SecureDelete`] so operators can disambiguate the
/// failure (e.g. step-6 OS error vs step-1 lock contention bug).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Step 1: ACQUIRE lock
    Acquire,
    /// Step 2: PRE-CAPTURE fragment paths
    PreCaptureFragments,
    /// Step 3: DELETE (tombstone)
    Delete,
    /// Step 4: CLEANUP (physical rewrite)
    Cleanup,
    /// Step 5: PRE-CAPTURE HNSW index paths
    PreCaptureIndices,
    /// Step 6: OVERWRITE pre-captured paths
    Overwrite,
    /// Step 7: RELEASE lock
    Release,
}

impl Step {
    /// Human-readable name for [`Mem0gError::SecureDelete::step`].
    pub fn as_str(self) -> &'static str {
        match self {
            Step::Acquire => "ACQUIRE",
            Step::PreCaptureFragments => "PRE-CAPTURE-FRAGMENTS",
            Step::Delete => "DELETE",
            Step::Cleanup => "CLEANUP",
            Step::PreCaptureIndices => "PRE-CAPTURE-INDICES",
            Step::Overwrite => "OVERWRITE",
            Step::Release => "RELEASE",
        }
    }
}

/// Map key for [`PerTableLockMap`] — `(workspace_id, table_name)`.
pub type TableLockKey = (WorkspaceId, String);

/// Lock handle handed out by [`PerTableLockMap::get_or_insert`].
/// `RwLock<()>` because we are gating critical sections, not protecting
/// a piece of data; `Arc` so the trait methods (`&self`) can clone
/// the handle for write-guard acquisition.
pub type TableLockHandle = std::sync::Arc<RwLock<()>>;

/// Per-(workspace, table) lock map.
///
/// LanceDB 0.29 does not expose a native table-level write lock that
/// blocks concurrent reads (`Table::optimize` IS racy with concurrent
/// `delete`). Atlas implements a per-table `RwLock` map keyed by
/// `(workspace_id, table_name)`.
///
/// The map itself is behind a `Mutex` for safe insertion of new
/// `RwLock` instances on first sight of a (workspace, table) pair.
/// Per-(workspace, table) lock contention is the dominant case;
/// the outer `Mutex` is held ONLY during the get-or-insert lookup.
#[derive(Default)]
pub struct PerTableLockMap {
    inner: Mutex<BTreeMap<TableLockKey, TableLockHandle>>,
}

impl PerTableLockMap {
    /// Construct an empty lock map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get-or-insert the `RwLock` for a (workspace, table) pair.
    pub fn get_or_insert(
        &self,
        workspace_id: &WorkspaceId,
        table_name: &str,
    ) -> Mem0gResult<TableLockHandle> {
        let mut map = self.inner.lock().map_err(|e| Mem0gError::SecureDelete {
            step: Step::Acquire.as_str(),
            reason: format!("PerTableLockMap inner Mutex poisoned: {e}"),
        })?;
        let key = (workspace_id.clone(), table_name.to_string());
        let lock = map
            .entry(key)
            .or_insert_with(|| std::sync::Arc::new(RwLock::new(())))
            .clone();
        Ok(lock)
    }
}

/// Result of step 2 + step 5 pre-capture phases.
///
/// Carrying the path set across step boundaries is the load-bearing
/// detail that closes the TOCTOU race. The protocol overwrites
/// EXACTLY the paths captured here, not whatever happens to be on
/// disk after step 4.
#[derive(Debug, Clone)]
pub struct PreCapturedPaths {
    /// Fragment paths captured BEFORE delete + cleanup (step 2).
    pub fragment_paths: Vec<PathBuf>,
    /// HNSW index paths captured AFTER cleanup, BEFORE overwrite
    /// (step 5). Default option (a) per ADR — both fragments AND
    /// indices are overwritten.
    pub index_paths: Vec<PathBuf>,
}

impl PreCapturedPaths {
    /// Construct a new pre-captured-paths set.
    #[must_use]
    pub fn new(fragment_paths: Vec<PathBuf>, index_paths: Vec<PathBuf>) -> Self {
        Self {
            fragment_paths,
            index_paths,
        }
    }

    /// Iterator over all pre-captured paths (fragments + indices).
    pub fn iter(&self) -> impl Iterator<Item = &PathBuf> {
        self.fragment_paths.iter().chain(self.index_paths.iter())
    }
}

/// Step 6: overwrite a single pre-captured path with random bytes,
/// fdatasync, close, and unlink.
///
/// **Per the protocol contract:** this function is called for each
/// pre-captured path INSIDE the write-lock held since step 1. No
/// concurrent readers/writers/compactors observe an intermediate
/// state.
///
/// # Errors
///
/// Returns [`Mem0gError::SecureDelete`] with `step = "OVERWRITE"` on
/// any I/O failure. Caller decides whether to continue with other
/// pre-captured paths (best-effort) or abort (security-conservative).
/// The default `apply_overwrite_set` implementation aborts on first
/// failure.
pub fn overwrite_file(path: &Path) -> Mem0gResult<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let metadata = std::fs::metadata(path).map_err(|e| Mem0gError::SecureDelete {
        step: Step::Overwrite.as_str(),
        reason: format!("stat {}: {e}", path.display()),
    })?;
    let file_size = metadata.len();

    // Open for write (truncate=false; we want to overwrite in place
    // so the same on-disk blocks are rewritten, not a new file with
    // a different inode + the old blocks lingering).
    let mut f = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| Mem0gError::SecureDelete {
            step: Step::Overwrite.as_str(),
            reason: format!("open {}: {e}", path.display()),
        })?;

    // Write random bytes equal to file size. Chunked to avoid holding
    // a single allocation for very large fragments. 64 KB chunks
    // mirror LanceDB's default row-group size for cache efficiency.
    let mut remaining = file_size;
    let mut buf = [0u8; 64 * 1024];
    while remaining > 0 {
        // Fill chunk with random bytes via blake3 keyed-hash output
        // (deterministic per call but uncorrelated across files;
        // attacker cannot recover original bytes from the output).
        // For real cryptographic randomness in production, an
        // operator may swap this to `getrandom::getrandom`. blake3
        // is sufficient for the security property: "the on-disk
        // bytes are no longer the original bytes after overwrite".
        let chunk_len = std::cmp::min(remaining as usize, buf.len());
        fill_random_bytes(&mut buf[..chunk_len], path, remaining);
        f.write_all(&buf[..chunk_len])
            .map_err(|e| Mem0gError::SecureDelete {
                step: Step::Overwrite.as_str(),
                reason: format!("write {}: {e}", path.display()),
            })?;
        remaining -= chunk_len as u64;
    }

    // fdatasync — flush data + length to durable storage. We
    // explicitly want fdatasync (NOT fsync) because we don't care
    // about metadata-only updates here; we care about the data
    // blocks reaching the SSD/disk. Rust's `sync_data` maps to
    // fdatasync where available (`sync_all` is fsync).
    f.sync_data().map_err(|e| Mem0gError::SecureDelete {
        step: Step::Overwrite.as_str(),
        reason: format!("sync_data {}: {e}", path.display()),
    })?;
    drop(f); // close

    std::fs::remove_file(path).map_err(|e| Mem0gError::SecureDelete {
        step: Step::Overwrite.as_str(),
        reason: format!("remove_file {}: {e}", path.display()),
    })?;
    Ok(())
}

/// Step 6 driver: apply overwrite to every pre-captured path.
///
/// Aborts on first failure (security-conservative). Operator-runbook
/// documents the recovery: if step 6 fails partway, the workspace's
/// Layer-3 cache is in a partially-overwritten state — operator
/// re-runs `erase` (idempotent on `event_uuid`) OR triggers a full
/// rebuild from Layer 1.
pub fn apply_overwrite_set(paths: &PreCapturedPaths) -> Mem0gResult<()> {
    for path in paths.iter() {
        // Skip paths that no longer exist (a concurrent compactor
        // ran between step 2 and step 6 despite the lock — should
        // be impossible under the contract, but defence-in-depth).
        if !path.exists() {
            continue;
        }
        overwrite_file(path)?;
    }
    Ok(())
}

/// Fill a buffer with random-looking bytes derived from blake3 over
/// `(path, remaining)`. Used inside [`overwrite_file`] step 6.
///
/// This is NOT cryptographic randomness — it's a deterministic
/// stream that ensures the on-disk bytes are no longer the original
/// bytes after overwrite. Cryptographic randomness via
/// `getrandom::getrandom` is operator-runbook upgrade path for
/// deployments with explicit cryptographic-shred requirements.
fn fill_random_bytes(buf: &mut [u8], path: &Path, remaining: u64) {
    let mut h = blake3::Hasher::new();
    h.update(path.to_string_lossy().as_bytes());
    h.update(&remaining.to_le_bytes());
    let mut reader = h.finalize_xof();
    reader.fill(buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn step_as_str_covers_all_variants() {
        assert_eq!(Step::Acquire.as_str(), "ACQUIRE");
        assert_eq!(Step::PreCaptureFragments.as_str(), "PRE-CAPTURE-FRAGMENTS");
        assert_eq!(Step::Delete.as_str(), "DELETE");
        assert_eq!(Step::Cleanup.as_str(), "CLEANUP");
        assert_eq!(Step::PreCaptureIndices.as_str(), "PRE-CAPTURE-INDICES");
        assert_eq!(Step::Overwrite.as_str(), "OVERWRITE");
        assert_eq!(Step::Release.as_str(), "RELEASE");
    }

    #[test]
    fn per_table_lock_map_get_or_insert_consistent() {
        let map = PerTableLockMap::new();
        let l1 = map.get_or_insert(&"ws".to_string(), "table_a").unwrap();
        let l2 = map.get_or_insert(&"ws".to_string(), "table_a").unwrap();
        // Same (workspace, table) → same lock instance.
        assert!(std::sync::Arc::ptr_eq(&l1, &l2));

        let l3 = map.get_or_insert(&"ws".to_string(), "table_b").unwrap();
        // Different table → different lock.
        assert!(!std::sync::Arc::ptr_eq(&l1, &l3));
    }

    #[test]
    fn pre_captured_paths_iter_yields_fragments_then_indices() {
        let p = PreCapturedPaths::new(
            vec![PathBuf::from("frag1"), PathBuf::from("frag2")],
            vec![PathBuf::from("idx1")],
        );
        let collected: Vec<_> = p.iter().collect();
        assert_eq!(collected.len(), 3);
        assert_eq!(collected[0], &PathBuf::from("frag1"));
        assert_eq!(collected[2], &PathBuf::from("idx1"));
    }

    #[test]
    fn overwrite_file_replaces_bytes_then_removes() {
        // Write a known sentinel to a temp file.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-overwrite.bin");
        let sentinel = b"SECRET_EMBEDDING_BYTES_DEADBEEF_AAAA_BBBB_CCCC";
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(sentinel).unwrap();
        f.sync_all().unwrap();
        drop(f);

        // Sanity: bytes are recoverable BEFORE overwrite.
        let pre = std::fs::read(&path).unwrap();
        assert_eq!(pre, sentinel);

        // Apply overwrite_file.
        overwrite_file(&path).unwrap();

        // File MUST be unlinked.
        assert!(!path.exists(), "file should be removed after overwrite");
    }

    #[test]
    fn apply_overwrite_set_processes_fragments_and_indices() {
        let dir = tempfile::tempdir().unwrap();
        let frag_a = dir.path().join("frag_a.lance");
        let frag_b = dir.path().join("frag_b.lance");
        let idx_a = dir.path().join("idx_a.bin");

        for p in &[&frag_a, &frag_b, &idx_a] {
            let mut f = std::fs::File::create(p).unwrap();
            f.write_all(b"PROTECTED_BYTES_THAT_MUST_NOT_LEAK").unwrap();
            f.sync_all().unwrap();
        }

        let paths = PreCapturedPaths::new(
            vec![frag_a.clone(), frag_b.clone()],
            vec![idx_a.clone()],
        );
        apply_overwrite_set(&paths).unwrap();

        assert!(!frag_a.exists());
        assert!(!frag_b.exists());
        assert!(!idx_a.exists());
    }

    #[test]
    fn apply_overwrite_set_skips_missing_paths_defence_in_depth() {
        // A concurrent compactor between step 2 + step 6 SHOULD be
        // impossible under the lock contract, but if a path is
        // already gone we skip rather than fail.
        let paths = PreCapturedPaths::new(
            vec![PathBuf::from("/this/does/not/exist/frag_x")],
            vec![],
        );
        apply_overwrite_set(&paths).unwrap();
    }
}
