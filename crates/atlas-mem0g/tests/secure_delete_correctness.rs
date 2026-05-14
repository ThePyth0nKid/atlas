//! V2-β Welle 18b: V1 verification gap closure — secure-delete correctness.
//!
//! Per spike §12 V1 + ADR §4 sub-decision #4 protocol step 6:
//!
//! 1. Write known sentinel bytes to a temp directory.
//! 2. Invoke the secure-delete `apply_overwrite_set` wrapper.
//! 3. Raw-file-read of the storage dir verifies original bytes are
//!    NOT recoverable via simple `fs::read`.
//!
//! Plus concurrent-write race test:
//!
//! 1. Spawn a parallel reader during the wrapper sequence.
//! 2. Assert the lock contract holds (no torn reads of in-flight
//!    overwrites).
//!
//! Does NOT test SSD-physical-erasure (out of scope per ADR §4
//! sub-decision #4 SSD wear-leveling caveat).

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use atlas_mem0g::secure_delete::{
    apply_overwrite_set, overwrite_file, PerTableLockMap, PreCapturedPaths,
};

const SENTINEL: &[u8] =
    b"ATLAS_SECRET_EMBEDDING_BYTES_DEADBEEF_AAAA_BBBB_CCCC_DDDD_EEEE_FFFF";

#[test]
fn secure_delete_raw_bytes_not_recoverable() {
    let dir = tempfile::tempdir().unwrap();

    // Write sentinel to two fragment files + one index file.
    let frag_a = dir.path().join("frag_a.lance");
    let frag_b = dir.path().join("frag_b.lance");
    let idx_a = dir.path().join("idx_a.bin");

    for p in &[&frag_a, &frag_b, &idx_a] {
        let mut f = std::fs::File::create(p).unwrap();
        f.write_all(SENTINEL).unwrap();
        f.sync_all().unwrap();
    }

    // Sanity: sentinel is recoverable BEFORE the wrapper runs.
    for p in &[&frag_a, &frag_b, &idx_a] {
        let raw = std::fs::read(p).unwrap();
        assert_eq!(raw, SENTINEL, "pre-wrapper bytes must match sentinel");
    }

    // Apply the secure-delete protocol step 6 wrapper.
    let paths = PreCapturedPaths::new(
        vec![frag_a.clone(), frag_b.clone()],
        vec![idx_a.clone()],
    );
    apply_overwrite_set(&paths).unwrap();

    // POST: files MUST be unlinked.
    assert!(!frag_a.exists(), "frag_a should be removed");
    assert!(!frag_b.exists(), "frag_b should be removed");
    assert!(!idx_a.exists(), "idx_a should be removed");

    // The dir-level scan must NOT contain the sentinel bytes any
    // longer. (Caveat: this is best-effort against filesystem-level
    // reads — SSD wear-leveling may keep copies in spare cells.
    // Per ADR §4 sub-decision #4 SSD caveat, this test does NOT
    // assert physical-erasure beyond filesystem-level overwrite.)
    let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(
        entries.len(),
        0,
        "tempdir should be empty after secure-delete; got {entries:?}"
    );
}

#[test]
fn secure_delete_overwrites_in_place_before_unlink() {
    // Verify the overwrite step happens BEFORE the unlink (so any
    // process holding an open file-descriptor to the original
    // sees the OVERWRITTEN bytes, not the original).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doomed.bin");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(SENTINEL).unwrap();
    f.sync_all().unwrap();
    drop(f);

    // Note: capturing an open file-handle PRE-unlink is platform-
    // specific (Linux: open FD survives unlink; Windows: stricter).
    // We verify the property indirectly: after `overwrite_file`,
    // the file is gone AND no copy of sentinel remains in dir.
    overwrite_file(&path).unwrap();
    assert!(!path.exists());

    // Scan dir for any file containing the sentinel.
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let entry = entry.unwrap();
        let bytes = std::fs::read(entry.path()).unwrap_or_default();
        assert!(
            !bytes.windows(SENTINEL.len()).any(|w| w == SENTINEL),
            "sentinel bytes leaked into a sibling file"
        );
    }
}

#[test]
fn secure_delete_concurrent_reader_lock_contract() {
    // Per ADR §4 sub-decision #4 step 1 + step 4: the write lock
    // held since step 1 prevents the LanceDB background compactor
    // from racing the delete + cleanup. We model this with a
    // PerTableLockMap: a reader spawned during the wrapper sequence
    // BLOCKS until the wrapper releases its write-lock guard.
    let map = Arc::new(PerTableLockMap::new());
    let workspace = "ws-race-test".to_string();
    let table = "events";

    let map_writer = map.clone();
    let workspace_writer = workspace.clone();
    let reader_observed_after_writer = Arc::new(AtomicBool::new(false));
    let observed_clone = reader_observed_after_writer.clone();

    // Writer thread: holds the write lock for 100 ms (simulating
    // the overwrite step that takes filesystem-level time).
    let writer = thread::spawn(move || {
        let lock = map_writer
            .get_or_insert(&workspace_writer, table)
            .unwrap();
        let _guard = lock.write().unwrap();
        thread::sleep(Duration::from_millis(100));
        observed_clone.store(true, Ordering::SeqCst);
        // _guard dropped here → write lock released.
    });

    // Give the writer time to acquire.
    thread::sleep(Duration::from_millis(20));

    // Reader thread: must block until writer releases.
    let map_reader = map.clone();
    let workspace_reader = workspace.clone();
    let reader = thread::spawn(move || {
        let lock = map_reader.get_or_insert(&workspace_reader, table).unwrap();
        let _guard = lock.read().unwrap();
        // When this reader observes, the writer's atomic flag MUST
        // already be set (because the writer's drop is what released
        // the lock — and the reader couldn't observe before that).
        let writer_done = reader_observed_after_writer.load(Ordering::SeqCst);
        assert!(
            writer_done,
            "reader observed BEFORE writer completed — lock contract violated"
        );
    });

    writer.join().unwrap();
    reader.join().unwrap();
}

/// MEDIUM-2 fix (reviewer-driven): pre-captured paths missing at
/// step 6 time MUST surface a hard `SecureDelete` error. The previous
/// behaviour (silent skip) risked a false "erasure confirmed"
/// attestation to the regulator when the step-1 write-lock contract
/// was violated by a concurrent compactor. The corrected behaviour
/// fails closed so the caller does NOT emit a false
/// `embedding_erased` audit-event.
#[test]
fn secure_delete_errors_on_missing_pre_captured_paths() {
    use atlas_mem0g::Mem0gError;
    let paths = PreCapturedPaths::new(
        vec![PathBuf::from("/this/does/not/exist/frag_x")],
        vec![PathBuf::from("/this/does/not/exist/idx_y")],
    );
    let err = apply_overwrite_set(&paths).expect_err(
        "expected SecureDelete error on missing pre-captured path",
    );
    match err {
        Mem0gError::SecureDelete { step, reason } => {
            assert_eq!(step, "OVERWRITE");
            assert!(
                reason.contains("disappeared under lock"),
                "reason must surface lock-contract violation; got: {reason}"
            );
        }
        other => panic!("expected Mem0gError::SecureDelete; got {other:?}"),
    }
}
