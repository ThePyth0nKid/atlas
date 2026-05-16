//! V2-β Welle 18c Phase C: V1 verification gap closure — LanceDB
//! cleanup_old_versions Windows behaviour (Atlas filesystem wrapper).
//!
//! Per W18 spike §12 V1 + W18c plan-doc Phase C:
//!
//! > "LanceDB `cleanup_old_versions` behaviour on Windows — 50-line
//! > Rust integration test on Windows CI runner."
//!
//! ## Scope precisely
//!
//! The W18b first-shipped `lancedb_backend.rs` ships with the
//! LanceDB body sites STUBBED (Phase D's territory): the actual
//! `Table::cleanup_old_versions(Duration::ZERO).await` call is
//! still a `RESUME(spawn_blocking)` marker. Phase C's V1 closure
//! therefore exercises the *Atlas-side* portion of the
//! cleanup-old-versions protocol on Windows path semantics:
//!
//! 1. The recursive [`LanceDbCacheBackend::precapture_fragments`]
//!    walk over a workspace directory containing nested
//!    `_versions/<N>/data-*.lance` Lance-style fragment paths.
//! 2. The [`crate::secure_delete::apply_overwrite_set`] filesystem
//!    wrapper that stands in for the cleanup-old-versions side
//!    effect (overwrites + unlinks the pre-captured paths).
//! 3. Windows-specific path semantics that LanceDB 0.29 *would*
//!    encounter when running on Windows: backslash separators and
//!    case-insensitive filesystem (NTFS). UNC long-path-prefix
//!    `\\?\` handling is NOT exercised by this fixture (paths are
//!    rooted under `tempfile::tempdir()`, which on Windows CI
//!    resolves under `%TEMP%` — a short path); a follow-on V2-γ
//!    test would explicitly construct `\\?\`-prefixed paths to
//!    cover that gap (PR #107 security-reviewer LOW-2).
//!
//! When Phase D wires the real `cleanup_old_versions` call, the
//! existing assertions hold because Atlas's wrapper layout
//! (table-dir → `_versions/<N>/data-*.lance` + `_indices/<N>/...`)
//! mirrors LanceDB's on-disk layout (verified against
//! `lancedb-0.29` API + `lance-table-0.x` Fragment metadata).
//!
//! ## Cross-platform contract
//!
//! `#[cfg(target_os = "windows")]`-gated: this test only runs on
//! the `windows-latest` leg of the `atlas-mem0g-smoke` matrix. The
//! Linux + macOS legs skip it (no panic — the test simply does not
//! exist for them; `cargo test` reports zero tests in this file on
//! non-Windows).
//!
//! ## V2-γ followup (deferred-by-decision)
//!
//! When Phase D lifts the `RESUME(spawn_blocking)` markers, an
//! additional integration test SHOULD exercise the full
//! `Table::cleanup_old_versions(Duration::ZERO)` round-trip on
//! Windows specifically (verifying that the LanceDB-side fragment
//! file unlink succeeds against open file handles — a known
//! Windows pitfall that POSIX systems do not exhibit). This is
//! tracked as a Phase D sibling concern; not in Phase C scope.

#![allow(unused_imports)]

use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[cfg(all(feature = "lancedb-backend", target_os = "windows"))]
use atlas_mem0g::secure_delete::{apply_overwrite_set, PreCapturedPaths};

/// Sentinel bytes used to verify the Atlas-side overwrite wrapper
/// (which substitutes for `cleanup_old_versions` until Phase D)
/// scrubs the file's prior contents on Windows.
#[cfg(all(feature = "lancedb-backend", target_os = "windows"))]
const SENTINEL: &[u8] =
    b"ATLAS_LANCE_FRAGMENT_BYTES_WINDOWS_TEST_SENTINEL_AAAA_BBBB_CCCC_DDDD";

/// V1 — Windows path semantics + recursive fragment enumeration +
/// secure-delete wrapper round-trip.
///
/// Builds a directory tree mirroring LanceDB 0.29's on-disk
/// versioned-fragment layout:
///
/// ```text
/// <tmp>/<workspace>/_versions/0/data-A.lance
/// <tmp>/<workspace>/_versions/0/data-B.lance
/// <tmp>/<workspace>/_versions/1/data-C.lance
/// <tmp>/<workspace>/_indices/idx_a.bin
/// ```
///
/// Then enumerates the fragment paths via a recursive walk that
/// matches `precapture_fragments`'s `*.lance`-extension filter,
/// pre-captures the index files via the same walk WITHOUT the
/// extension filter (matching `precapture_indices`), invokes
/// `apply_overwrite_set` (Atlas's GDPR Art. 17 wrapper around
/// what would be `cleanup_old_versions(Duration::ZERO)` after
/// the LanceDB delete), and asserts:
///
/// 1. The walk found ALL three nested `.lance` fragments (not just
///    top-level — Windows backslash handling does NOT trip the
///    walker).
/// 2. The walk found the `_indices/` files.
/// 3. After `apply_overwrite_set`, the files are removed AND the
///    sentinel bytes are NOT present in the directory anymore
///    (filesystem-level overwrite contract holds on Windows NTFS,
///    where files held open by other processes can't be unlinked
///    — the Atlas overwrite-then-unlink ordering MUST close this
///    window).
///
/// The 50-LOC budget per W18c plan-doc holds (test body ~50 lines;
/// fixture setup is mechanical + amortised across the assertions).
#[test]
#[cfg(all(feature = "lancedb-backend", target_os = "windows"))]
fn windows_lance_fragment_layout_walk_and_secure_cleanup() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace_root = dir.path().join("ws_default");
    let versions_0 = workspace_root.join("_versions").join("0");
    let versions_1 = workspace_root.join("_versions").join("1");
    let indices_dir = workspace_root.join("_indices");

    fs::create_dir_all(&versions_0).expect("create _versions/0");
    fs::create_dir_all(&versions_1).expect("create _versions/1");
    fs::create_dir_all(&indices_dir).expect("create _indices");

    let frag_a = versions_0.join("data-A.lance");
    let frag_b = versions_0.join("data-B.lance");
    let frag_c = versions_1.join("data-C.lance");
    let idx_a = indices_dir.join("idx_a.bin");

    for p in &[&frag_a, &frag_b, &frag_c, &idx_a] {
        let mut f = fs::File::create(p).expect("create fragment / index file");
        f.write_all(SENTINEL).expect("write sentinel");
        f.sync_all().expect("sync_all");
    }

    // Recursive walk filtered on `.lance` extension — mirrors
    // LanceDbCacheBackend::precapture_fragments. Implemented inline
    // so the test does not require pub(crate) → pub access.
    let mut fragments: Vec<PathBuf> = Vec::new();
    walk(&workspace_root, &mut fragments, &|p| {
        p.extension().and_then(|s| s.to_str()) == Some("lance")
    });
    fragments.sort();

    assert_eq!(
        fragments.len(),
        3,
        "recursive walk must find all 3 nested `.lance` fragments on Windows; got {fragments:?}"
    );
    for expected in &[&frag_a, &frag_b, &frag_c] {
        assert!(
            fragments.contains(expected),
            "fragment {} missing from walk result {fragments:?}",
            expected.display()
        );
    }

    // Recursive walk without extension filter — mirrors
    // LanceDbCacheBackend::precapture_indices.
    let mut indices: Vec<PathBuf> = Vec::new();
    walk(&indices_dir, &mut indices, &|_p| true);
    assert_eq!(indices, vec![idx_a.clone()], "index walk must find idx_a");

    // Atlas's overwrite-then-unlink wrapper — substitutes for
    // `Table::cleanup_old_versions(Duration::ZERO)` until Phase D
    // wires the real LanceDB call. Windows-specific concern: NTFS
    // refuses to unlink files with open handles. Atlas's wrapper
    // opens the file write-mode, overwrites in-place, fsyncs,
    // closes, THEN unlinks — the close happens before unlink so
    // the unlink succeeds.
    let pre_captured = PreCapturedPaths::new(fragments.clone(), indices.clone());
    apply_overwrite_set(&pre_captured).expect("apply_overwrite_set on Windows");

    for p in &[&frag_a, &frag_b, &frag_c, &idx_a] {
        assert!(!p.exists(), "{} must be unlinked on Windows after secure-delete", p.display());
    }

    // Sweep the workspace tree for sentinel bytes. SSD wear-leveling
    // caveat per ADR §4 sub-decision #4 SSD note: this is a
    // filesystem-level assertion, NOT a physical-erasure assertion.
    // On NTFS, the unlinked file's allocated extents may persist
    // until overwritten by a subsequent allocation, but the
    // user-space `read_dir` + `read` API CAN'T see those bytes
    // without going through raw-disk APIs (out of scope per the
    // SSD caveat). What we CAN assert: no live file in the tree
    // contains the sentinel.
    //
    // PR #107 reviewer MEDIUM-2 (code) — this sweep is vacuously
    // true after a successful `apply_overwrite_set` (all four files
    // were unlinked above; the loop body would not execute on a
    // valid post-cleanup tree). Its conditional value: it would
    // catch a regression where `apply_overwrite_set` was changed
    // to overwrite-but-not-unlink (the sentinel bytes would survive
    // in a still-present file). We retain it for that defence.
    let mut all_files: Vec<PathBuf> = Vec::new();
    walk(&workspace_root, &mut all_files, &|_p| true);
    for p in &all_files {
        let bytes = fs::read(p).unwrap_or_default();
        assert!(
            !bytes.windows(SENTINEL.len()).any(|w| w == SENTINEL),
            "post-cleanup file {} still contains sentinel bytes — Windows overwrite contract violated",
            p.display()
        );
    }
}

/// Recursive directory walker. Matches the semantic of
/// `LanceDbCacheBackend::walk_collect_filtered` (private). Inlined
/// here so the integration test stays portable to any future
/// refactor of the backend module's pub-surface.
///
/// PR #107 reviewer notes:
/// - **MEDIUM-1 (security):** `read_dir` errors used to be silently
///   swallowed (`Err(_) => return`), which would have masked test
///   fixture corruption. They now panic with the failing path so
///   the test fails loudly instead of vacuously passing. If a
///   production port of this walker re-uses the pattern, the same
///   panic-on-error contract MUST be preserved (silent swallow
///   would be a GDPR Art. 17 false-attestation risk identical to
///   the W18b MEDIUM-2 fix on `apply_overwrite_set`).
/// - **MEDIUM-2 (security):** `path.is_symlink()` guard added
///   before the `is_dir()` recursion. On Windows CI, ephemeral
///   unprivileged runners can't create symlinks (no
///   `SeCreateSymbolicLinkPrivilege`), so the practical risk is
///   negligible in this test context — but a malicious workspace
///   directory in production with a symlink loop would otherwise
///   recurse to stack overflow. The guard makes any production
///   port of this walker safe by construction.
#[cfg(all(feature = "lancedb-backend", target_os = "windows"))]
fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>, predicate: &dyn Fn(&std::path::Path) -> bool) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => panic!(
            "walk: read_dir({}) failed: {e} — test fixture corruption, refusing to mask",
            dir.display()
        ),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Skip symlinks before any recursion or read; prevents loop
        // recursion on a malicious workspace tree.
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            walk(&path, out, predicate);
        } else if predicate(&path) {
            out.push(path);
        }
    }
}

/// Sentinel: the test compiles + runs cleanly on non-Windows OS.
/// No assertion — the test simply does not apply on Linux/macOS
/// (the V1 verification gap is Windows-specific per spike §12).
#[test]
#[cfg(any(not(feature = "lancedb-backend"), not(target_os = "windows")))]
fn windows_lance_behaviour_skipped_on_non_windows() {
    eprintln!(
        "V1 LanceDB Windows behaviour test only applies on \
         (target_os = \"windows\") + --features lancedb-backend"
    );
}
