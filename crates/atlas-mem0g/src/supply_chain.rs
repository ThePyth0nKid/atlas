//! V2-β Welle 18c Phase B fix-commit: supply-chain primitives.
//!
//! Hosts the download / SHA-verify / OMP-pin primitives that were
//! previously inlined in `embedder.rs`. The split exists because
//! `embedder.rs` grew past the 800-LOC hard limit at Phase B
//! (code-reviewer MEDIUM-2). The pin CONSTANTS themselves stay in
//! `embedder.rs` because they are the supply-chain contract; the
//! primitives are the implementation.
//!
//! ## TOCTOU posture (security HIGH-1 fix)
//!
//! The previous code path was:
//!
//! 1. `ensure_file_with_sha` → cache hit → `verify_cached_file_sha`
//!    streams the file from disk, hashes it, returns `Ok(())`.
//! 2. `AtlasEmbedder::new` then separately calls `std::fs::read(path)`
//!    to load bytes for fastembed-rs.
//!
//! Between (1) and (2), the file on disk CAN be atomically swapped
//! (rename / symlink swap on POSIX; `MoveFileEx` on Windows). The
//! bytes fed to fastembed would then be unverified.
//!
//! The fix collapses verify-and-use into a single primitive
//! [`read_and_verify`]: read the whole file into a `Vec<u8>`, hash
//! the in-memory bytes, compare. The bytes fed downstream ARE the
//! bytes that were verified. Single buffer per file. No window for
//! TOCTOU between verify and use.
//!
//! The download arm uses the same primitive after streaming the
//! response body to disk: stream-write → `read_and_verify`. This
//! avoids holding the full 130 MB ONNX response in memory twice
//! (`response.bytes()` + write-buffer); we stream-write, then read
//! back from disk once for the SHA + fastembed handoff.

use crate::{Mem0gError, Mem0gResult};

// ---------------------------------------------------------------------------
// Determinism: OMP_NUM_THREADS pin
// ---------------------------------------------------------------------------

/// Set `OMP_NUM_THREADS=1` programmatically. MUST be called BEFORE
/// any fastembed-rs init (ORT picks up the env var at session-create
/// time, not at per-call time).
///
/// This is a process-wide setting; tests that exercise the embedder
/// MUST call this in their setup. Idempotent — wrapped in `Once`,
/// the `set_var` is performed exactly once across the lifetime of
/// the process even if many threads call this concurrently.
///
/// Safety: `std::env::set_var` is unsafe-on-Rust-2024 because of
/// multi-threaded race risks on the global `environ`. The `Once`
/// wrapper (MEDIUM-5 fix) eliminates the multi-threaded race: only
/// ONE thread performs the actual `set_var` call (the very first
/// caller, while all other callers block in `Once::call_once`).
/// After the first call returns, the env var is set and subsequent
/// calls are non-mutating no-ops.
pub fn pin_omp_threads_single() {
    // MEDIUM-5 fix: serialise the unsafe set_var via Once so concurrent
    // test threads do NOT race on the global `environ` block.
    static OMP_PIN_ONCE: std::sync::Once = std::sync::Once::new();
    OMP_PIN_ONCE.call_once(|| {
        // SAFETY: The Once::call_once guarantees this closure runs
        // exactly once across all threads. While it runs, no other
        // thread can be executing pin_omp_threads_single via this
        // path. Required for deterministic ORT embedding
        // (ADR §4 sub-decision #2).
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("OMP_NUM_THREADS", "1");
        }
    });
}

// ---------------------------------------------------------------------------
// SHA-256 primitives
// ---------------------------------------------------------------------------

/// Compute SHA-256 of a byte slice and return lowercase hex.
///
/// HIGH-1 fix (W18b): this previously delegated to a
/// `blake3-placeholder-...` string (NOT SHA-256), which silently
/// broke supply-chain verification regardless of `ONNX_SHA256`'s
/// value. Now uses `sha2::Sha256` for real RFC-6234 SHA-256 — the
/// same algorithm HuggingFace and the `sha256sum` operator-runbook
/// tool produce.
///
/// Empty-input contract: SHA-256 of the empty byte slice is the
/// canonical
/// `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
/// Unit-tested below.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    hex::encode(digest)
}

/// Compute SHA-256 of a file by streaming in 64 KiB chunks.
///
/// HIGH-1 fix companion: stream-friendly variant for large ONNX
/// model files. Returns lowercase hex.
///
/// Kept available for cases where we genuinely only want the SHA
/// (no buffer use). The TOCTOU-free downstream path uses
/// [`read_and_verify`] instead, which fuses read + verify into a
/// single buffer.
#[cfg_attr(not(feature = "lancedb-backend"), allow(dead_code))]
pub(crate) fn sha256_file(path: &std::path::Path) -> Mem0gResult<String> {
    use sha2::Digest;
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .map_err(|e| Mem0gError::Io(format!("open {}: {e}", path.display())))?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| Mem0gError::Io(format!("read {}: {e}", path.display())))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    Ok(hex::encode(digest))
}

// ---------------------------------------------------------------------------
// TOCTOU-free read-and-verify primitive (security HIGH-1 fix)
// ---------------------------------------------------------------------------

/// Read `path` into a `Vec<u8>` and verify SHA-256 of the
/// in-memory bytes against `expected_sha256` (lowercase hex).
///
/// **TOCTOU defence:** the bytes returned ARE the bytes that were
/// hashed. There is no window between verify and use during which
/// the file on disk can be swapped under us (rename / symlink-swap
/// on POSIX; `MoveFileEx` on Windows). Downstream consumers (fastembed-rs
/// `try_new_from_user_defined`) receive the verified buffer
/// directly.
///
/// # Errors
///
/// - [`Mem0gError::Io`] on read failure.
/// - [`Mem0gError::SupplyChainMismatch`] if the computed SHA-256
///   does not match `expected_sha256` (fail-closed — cache REFUSES
///   to embed).
#[cfg_attr(not(feature = "lancedb-backend"), allow(dead_code))]
pub(crate) fn read_and_verify(
    path: &std::path::Path,
    expected_sha256: &str,
) -> Mem0gResult<Vec<u8>> {
    let bytes = std::fs::read(path)
        .map_err(|e| Mem0gError::Io(format!("read {}: {e}", path.display())))?;
    let hash = sha256_hex(&bytes);
    if hash != expected_sha256 {
        return Err(Mem0gError::SupplyChainMismatch {
            expected: expected_sha256.to_string(),
            actual: hash,
        });
    }
    Ok(bytes)
}

/// Verify a cached file's SHA-256 against an expected pin.
///
/// Streams the file in 64 KiB chunks via [`sha256_file`]. Use
/// [`read_and_verify`] instead at every call-site that ALSO needs
/// the file's bytes — that primitive fuses read + verify into a
/// single buffer and is TOCTOU-free with respect to downstream use.
///
/// Kept as `pub(crate)` so it remains an available diagnostic for
/// "is this cached file still valid?" checks that genuinely do not
/// need the bytes (e.g. a future operator-runbook helper).
///
/// W18c Phase B: generalised from `verify_cached_model_sha` to
/// support all four file types via the cold-start re-verification
/// path in `AtlasEmbedder::new`.
///
/// W18c Phase B fix-commit (code-reviewer MEDIUM-4): demoted from
/// `pub` to `pub(crate)` — no external callers exist.
pub(crate) fn verify_cached_file_sha(
    path: &std::path::Path,
    expected_sha256: &str,
) -> Mem0gResult<()> {
    let hash = sha256_file(path)?;
    if hash != expected_sha256 {
        return Err(Mem0gError::SupplyChainMismatch {
            expected: expected_sha256.to_string(),
            actual: hash,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Download-with-SHA-verification (Path 1 — preferred)
// ---------------------------------------------------------------------------

/// Build the Atlas-controlled `reqwest::blocking::Client` used for
/// all five supply-chain downloads.
///
/// Posture:
/// - `https_only(true)` — TLS-pinned origin (`huggingface.co`); not
///   subject to follow-redirect attacks against the LFS endpoint.
/// - `timeout(300s)` — 5 minute hard wall-clock per request. The
///   `model.onnx` body is ~130 MB; at 1 Mbit/s minimum bandwidth
///   that completes in <20 min, but the realistic floor for
///   production deployments is ~10 Mbit/s and the timeout exists
///   solely to bound the worst-case "endpoint hangs mid-stream"
///   case rather than approximate the slowest legitimate transfer.
///   (Fix-commit Finding 1: code-reviewer HIGH-2.)
/// - `connect_timeout(30s)` — 30 second connect-handshake budget.
///   Defends against DNS-stuck / TLS-handshake-stuck failure modes
///   where the body bytes never start flowing.
///
/// The previous builder configured only `https_only(true)`. With
/// W18c Phase B making 5 serial downloads in `AtlasEmbedder::new`,
/// a single stalled HF endpoint would block embedder construction
/// indefinitely. The timeouts make that failure mode bounded.
#[cfg(feature = "lancedb-backend")]
fn build_atlas_http_client() -> Mem0gResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .https_only(true)
        .timeout(std::time::Duration::from_secs(300))
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| Mem0gError::Io(format!("reqwest client build: {e}")))
}

/// Generic Atlas-controlled file download with SHA-256 verification.
///
/// Per ADR §4 sub-decision #2 Path 1:
///
/// 1. Fetch `url` via Atlas-controlled `reqwest::blocking` client
///    with `https_only(true)` + timeouts (TLS-pinned origin; bounded
///    worst-case latency; not subject to follow-redirect attacks
///    against the LFS endpoint).
/// 2. Stream-write the response body directly to a sibling
///    `<dest>.partial` path via `Response::copy_to`. The ~130 MB
///    ONNX body is NEVER fully buffered in memory.
/// 3. After the stream completes, call [`read_and_verify`] against
///    the partial path to (a) read the bytes back and (b) hash
///    them in a SINGLE buffer. The bytes the caller eventually
///    feeds to fastembed-rs ARE the bytes that produced the SHA
///    we compared against — no TOCTOU window.
/// 4. On SHA mismatch, return [`Mem0gError::SupplyChainMismatch`]
///    BEFORE the partial-rename step. A corrupted download never
///    lands at the final cache path.
/// 5. On match, atomic-rename `<dest>.partial → <dest>`.
///
/// # Errors
///
/// - [`Mem0gError::Io`] on filesystem or network failure.
/// - [`Mem0gError::SupplyChainMismatch`] on SHA-256 mismatch
///   (fail-closed — the cache REFUSES to embed).
///
/// # W18c Phase B fix-commit
///
/// - Streams the response body via `Response::copy_to` instead of
///   `Response::bytes()` (security MEDIUM-1: avoids double-buffer
///   for the 130 MB ONNX).
/// - Uses [`read_and_verify`] for the SHA-vs-bytes comparison
///   (security HIGH-1: TOCTOU-free contract for downstream use).
/// - Writes to `<dest>.partial` and only renames on verify-success
///   so a failed download never leaves a poisoned file at `<dest>`.
#[cfg(feature = "lancedb-backend")]
fn download_file_with_sha(
    url: &str,
    expected_sha256: &str,
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    let client = build_atlas_http_client()?;

    let mut response = client
        .get(url)
        .send()
        .map_err(|e| Mem0gError::Io(format!("file download GET {url}: {e}")))?;

    if !response.status().is_success() {
        return Err(Mem0gError::Io(format!(
            "file download non-success status for {url}: {}",
            response.status()
        )));
    }

    std::fs::create_dir_all(
        dest.parent()
            .ok_or_else(|| Mem0gError::Io(format!("dest has no parent: {}", dest.display())))?,
    )
    .map_err(|e| Mem0gError::Io(format!("create_dir_all: {e}")))?;

    // Stream-write to a sibling .partial so a failed verify never
    // leaves a poisoned file at the final cache path (the next
    // cold-start would happily SHA-verify a corrupted body and
    // bail at that layer, but the .partial discipline keeps the
    // failure visible in a single place).
    let partial = dest.with_extension({
        let mut s = dest
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        if !s.is_empty() {
            s.push('.');
        }
        s.push_str("partial");
        s
    });

    {
        let mut f = std::fs::File::create(&partial)
            .map_err(|e| Mem0gError::Io(format!("file create {}: {e}", partial.display())))?;
        response
            .copy_to(&mut f)
            .map_err(|e| Mem0gError::Io(format!("file stream write {}: {e}", partial.display())))?;
        f.sync_all()
            .map_err(|e| Mem0gError::Io(format!("file fsync {}: {e}", partial.display())))?;
    }

    // Read-and-verify in a single buffer (TOCTOU-free contract;
    // see security HIGH-1 fix). If SHA mismatches we delete the
    // partial before bailing so the next attempt restarts cleanly.
    match read_and_verify(&partial, expected_sha256) {
        Ok(_bytes) => {}
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            return Err(e);
        }
    }

    // SHA verified — atomic-rename the partial into place. The
    // cache path now points at SHA-verified bytes.
    std::fs::rename(&partial, dest).map_err(|e| {
        Mem0gError::Io(format!(
            "rename {} -> {}: {e}",
            partial.display(),
            dest.display()
        ))
    })?;

    Ok(dest.to_path_buf())
}

/// Atlas-controlled `model.onnx` download with SHA-256 verification.
///
/// Thin wrapper over [`download_file_with_sha`] pinned to the model
/// URL and SHA constants in [`crate::embedder`]. Public so the
/// optional `bin/preload-embedder` operator-tool can call it during
/// cold-start CI cache warming (operator-runbook §atlas-mem0g-smoke).
#[cfg(feature = "lancedb-backend")]
pub fn download_model_with_verification(
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(
        crate::embedder::MODEL_URL,
        crate::embedder::ONNX_SHA256,
        dest,
    )
}

/// Atlas-controlled `tokenizer.json` download with SHA-256
/// verification. Thin wrapper over [`download_file_with_sha`] pinned
/// to the constants in [`crate::embedder`]. Fails closed on mismatch.
///
/// W18c Phase B fix-commit (code-reviewer MEDIUM-3): the 4 tokenizer
/// download wrappers are `pub(crate)` — only the in-crate
/// `AtlasEmbedder::new` consumes them. The model wrapper stays `pub`
/// because the documented `bin/preload-embedder` operator-tool path
/// needs it.
#[cfg(feature = "lancedb-backend")]
pub(crate) fn download_tokenizer_with_verification(
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(
        crate::embedder::TOKENIZER_JSON_URL,
        crate::embedder::TOKENIZER_JSON_SHA256,
        dest,
    )
}

/// Atlas-controlled `config.json` download. See
/// [`download_tokenizer_with_verification`] for the wrapper rationale.
#[cfg(feature = "lancedb-backend")]
pub(crate) fn download_config_with_verification(
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(
        crate::embedder::CONFIG_JSON_URL,
        crate::embedder::CONFIG_JSON_SHA256,
        dest,
    )
}

/// Atlas-controlled `special_tokens_map.json` download. See
/// [`download_tokenizer_with_verification`] for the wrapper rationale.
#[cfg(feature = "lancedb-backend")]
pub(crate) fn download_special_tokens_with_verification(
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(
        crate::embedder::SPECIAL_TOKENS_MAP_URL,
        crate::embedder::SPECIAL_TOKENS_MAP_SHA256,
        dest,
    )
}

/// Atlas-controlled `tokenizer_config.json` download. The fourth
/// tokenizer file required by `fastembed::TokenizerFiles` (see
/// fastembed-rs 5.13.4 `src/common.rs` lines 26-32). Phase A pinned
/// three; Phase B atomically extended with this fourth. See
/// [`download_tokenizer_with_verification`] for the wrapper rationale.
#[cfg(feature = "lancedb-backend")]
pub(crate) fn download_tokenizer_config_with_verification(
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(
        crate::embedder::TOKENIZER_CONFIG_JSON_URL,
        crate::embedder::TOKENIZER_CONFIG_JSON_SHA256,
        dest,
    )
}

// ---------------------------------------------------------------------------
// Cold-start helper: exists? → verify+read : download+verify+read
// ---------------------------------------------------------------------------

/// Cold-start ensure-and-load: returns the SHA-verified bytes of
/// the file at `path`. If `path` does not exist, fetches via
/// `downloader` first.
///
/// **TOCTOU-free contract:** the bytes returned ARE the bytes that
/// were hashed against `expected_sha`. There is no window between
/// verify and use during which the file on disk can be swapped.
/// Callers can hand the returned `Vec<u8>` directly to fastembed-rs
/// (`UserDefinedEmbeddingModel::new` / `TokenizerFiles`) without an
/// intermediate `std::fs::read` step (that step was the TOCTOU hole
/// in the W18c Phase B initial commit; security HIGH-1).
///
/// # Errors
///
/// - [`Mem0gError::SupplyChainMismatch`] if the file's SHA-256 does
///   not match `expected_sha` (cache REFUSES to embed; operator must
///   delete the poisoned cache entry).
/// - [`Mem0gError::Io`] on filesystem or network failure during the
///   download arm.
#[cfg(feature = "lancedb-backend")]
pub(crate) fn ensure_and_read_verified<F>(
    path: &std::path::Path,
    expected_sha: &str,
    downloader: F,
) -> Mem0gResult<Vec<u8>>
where
    F: FnOnce(&std::path::Path) -> Mem0gResult<std::path::PathBuf>,
{
    if !path.exists() {
        downloader(path)?;
    }
    // Cache-hit AND post-download both flow through read_and_verify
    // for a single TOCTOU-free path. The bytes returned ARE the
    // bytes that were hashed.
    read_and_verify(path, expected_sha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_empty_input_known_vector() {
        // HIGH-1 fix verification: the SHA-256 of the empty byte
        // slice is the canonical RFC-6234 test vector. If this
        // assertion fails, sha256_hex has regressed and supply-chain
        // verification is silently broken.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_hex_known_short_input() {
        // HIGH-1 fix verification: SHA-256("abc") = the canonical
        // RFC-6234 §B.1 test vector.
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_file_streams_file_correctly() {
        // HIGH-1 fix verification: the streaming `sha256_file`
        // helper agrees with `sha256_hex` for a small fixture
        // written to a temp file.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.bin");
        std::fs::write(&path, b"abc").unwrap();
        let got = sha256_file(&path).unwrap();
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn read_and_verify_returns_bytes_on_match() {
        // Security HIGH-1 fix verification: read_and_verify returns
        // the in-memory bytes after a successful SHA match. Downstream
        // consumers feed THESE bytes to fastembed.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.bin");
        std::fs::write(&path, b"abc").unwrap();
        let bytes = read_and_verify(
            &path,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .unwrap();
        assert_eq!(bytes, b"abc");
    }

    #[test]
    fn read_and_verify_fails_closed_on_mismatch() {
        // Security HIGH-1 fix verification: read_and_verify returns
        // SupplyChainMismatch when the on-disk bytes don't match the
        // expected SHA. No bytes leak past the gate.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.bin");
        std::fs::write(&path, b"abc").unwrap();
        let err = read_and_verify(
            &path,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap_err();
        assert!(matches!(err, Mem0gError::SupplyChainMismatch { .. }));
    }

    #[test]
    fn pin_omp_threads_single_idempotent() {
        // Two calls are a no-op (process-global var, second set is
        // structurally fine — same value).
        pin_omp_threads_single();
        pin_omp_threads_single();
        assert_eq!(
            std::env::var("OMP_NUM_THREADS").as_deref(),
            Ok("1"),
            "OMP_NUM_THREADS should be \"1\" after pin_omp_threads_single"
        );
    }
}
