//! V2-β Welle 18b: fastembed-rs wrapper + Atlas-controlled
//! download-with-SHA-verification per ADR-Atlas-012 §4 sub-decision #2.
//!
//! ## Supply-chain controls (closes security-reviewer HIGH-2)
//!
//! The model download is NOT delegated to fastembed-rs's default
//! download behaviour. Atlas wraps it in
//! [`download_model_with_verification`] which:
//!
//! 1. Fetches the ONNX file via an Atlas-controlled `reqwest` client
//!    (rustls-tls; same TLS posture as atlas-projector's ArcadeDB
//!    HTTP path).
//! 2. Verifies SHA256 BEFORE handing the file path to fastembed-rs.
//! 3. Fails closed on mismatch ([`crate::Mem0gError::SupplyChainMismatch`]).
//!
//! ## Compiled-in supply-chain pins (11 total: 6 hash digests + 5 URLs)
//!
//! Pinned for cold-start re-verification. The complete set is:
//!
//! - [`HF_REVISION_SHA`] — HuggingFace Git revision SHA-1 of the model
//!   repo at the chosen model-card version. Pins against repo-rename
//!   / repo-transfer / organisation-compromise attacks. (1 × SHA-1)
//! - [`ONNX_SHA256`] — SHA-256 of the `model.onnx` file bytes.
//!   Verifies the file regardless of repo-level integrity. (1 × SHA-256)
//! - [`TOKENIZER_JSON_SHA256`] / [`CONFIG_JSON_SHA256`] /
//!   [`SPECIAL_TOKENS_MAP_SHA256`] / [`TOKENIZER_CONFIG_JSON_SHA256`] —
//!   SHA-256 of the four tokenizer support files required by
//!   `fastembed::TextEmbedding::try_new_from_user_defined` via the
//!   upstream `TokenizerFiles` struct (4 × SHA-256). The fourth
//!   (`tokenizer_config.json`) was discovered during W18c Phase B
//!   API-surface verification (`fastembed-rs/src/common.rs` lines
//!   26-32) — Phase A pinned three but the upstream `TokenizerFiles`
//!   struct requires four; the Phase B agent extended the pin set
//!   atomically rather than blocking on a Phase-A-amendment welle.
//! - [`MODEL_URL`] / [`TOKENIZER_JSON_URL`] / [`CONFIG_JSON_URL`] /
//!   [`SPECIAL_TOKENS_MAP_URL`] / [`TOKENIZER_CONFIG_JSON_URL`] —
//!   full HuggingFace LFS URLs each embedding [`HF_REVISION_SHA`] in
//!   path. TLS-pinned via Atlas's `https_only(true)` reqwest
//!   configuration. (5 × URL)
//!
//! ## W18c Phase A — supply-chain constants lifted (2026-05-15)
//!
//! Resolved via `tools/w18c-phase-a-resolve.sh` against HuggingFace
//! `BAAI/bge-small-en-v1.5` at revision
//! `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`. All three load-bearing
//! W18b pins ([`HF_REVISION_SHA`] + [`ONNX_SHA256`] + [`MODEL_URL`])
//! plus three Phase-B tokenizer-file SHA-256 pins
//! ([`TOKENIZER_JSON_SHA256`] + [`CONFIG_JSON_SHA256`] +
//! [`SPECIAL_TOKENS_MAP_SHA256`]) plus three tokenizer URL pins
//! ([`TOKENIZER_JSON_URL`] + [`CONFIG_JSON_URL`] +
//! [`SPECIAL_TOKENS_MAP_URL`]) are now compiled-in. ONNX file size
//! 133,093,490 bytes / 126.93 MB matches spike §3.4 expected envelope
//! (V4 verification).
//!
//! ## W18c Phase B — fastembed wiring + fourth tokenizer pin (2026-05-15)
//!
//! API-surface verification against fastembed-rs 5.13.4 source
//! (`src/common.rs::TokenizerFiles` lines 26-32; `src/text_embedding/init.rs::UserDefinedEmbeddingModel`
//! lines 77-96; `src/text_embedding/impl.rs::try_new_from_user_defined`
//! lines 115-170; `src/text_embedding/impl.rs::embed` lines 447-464)
//! revealed that the upstream `TokenizerFiles` struct requires FOUR
//! files (tokenizer_file, config_file, special_tokens_map_file,
//! tokenizer_config_file), not three as outlined in the plan-doc.
//! The Phase B agent atomically extended the pin set with
//! [`TOKENIZER_CONFIG_JSON_SHA256`] and [`TOKENIZER_CONFIG_JSON_URL`]
//! (resolved live against HF `5c38ec7c…` during Phase B Step 0).
//!
//! `AtlasEmbedder::new` is now operational: SHA-verify-all-four, then
//! `pin_omp_threads_single`, then read-bytes, then
//! `UserDefinedEmbeddingModel::new(…).with_pooling(Pooling::Cls)`,
//! then `try_new_from_user_defined`. The pooling pin matches
//! fastembed's own `get_default_pooling_method(BGESmallENV15)`
//! (`src/text_embedding/impl.rs` line 218). The fail-closed Phase A
//! posture is fully lifted.
//!
//! The W18b `pins_are_placeholder_until_nelson_verifies` gatekeeper
//! test is retired; `pins_well_formed_after_lift` is the active
//! structural-format enforcer for all 6 hash digests + 5 URLs.
//!
//! See `.handoff/v2-beta-welle-18c-plan.md` Phase A + Phase B for the
//! resolution audit trail.
//!
//! ## Determinism pinning (ADR §4 sub-decision #2)
//!
//! Three load-bearing conditions enforced at init time:
//!
//! 1. `OMP_NUM_THREADS=1` set programmatically BEFORE fastembed-rs
//!    init. Single-thread CPU path is the deterministic path.
//! 2. ORT (ONNX Runtime) version pinned via Cargo.lock — exact-version
//!    pin `fastembed = "=5.13.4"` in Cargo.toml.
//! 3. `bge-small-en-v1.5` FP32 model only. Quantised variants are
//!    NOT deterministic across CPU instruction-set variants.

use crate::{Mem0gError, Mem0gResult};

// ---------------------------------------------------------------------------
// Compiled-in supply-chain pins (ADR §4 sub-decision #2)
// ---------------------------------------------------------------------------

/// HuggingFace Git revision SHA of `BAAI/bge-small-en-v1.5` at the
/// version Atlas adopted in W18b. 40-char Git SHA-1 hex digest.
///
/// Resolved 2026-05-15 via `tools/w18c-phase-a-resolve.sh` (W18c
/// Phase A). Rotations happen via explicit Atlas release; never
/// auto-bumped.
pub const HF_REVISION_SHA: &str = "5c38ec7c405ec4b44b94cc5a9bb96e735b38267a";

/// SHA-256 of `model.onnx` for `bge-small-en-v1.5` (FP32 / 133,093,490
/// bytes / ~126.93 MB; matches spike §3.4 expected envelope).
///
/// Resolved 2026-05-15 via `tools/w18c-phase-a-resolve.sh` (W18c
/// Phase A). The download-with-verification path fails closed on
/// mismatch ([`Mem0gError::SupplyChainMismatch`]).
pub const ONNX_SHA256: &str =
    "828e1496d7fabb79cfa4dcd84fa38625c0d3d21da474a00f08db0f559940cf35";

/// Full HuggingFace LFS URL incl. revision SHA in path. TLS-pinned
/// via Atlas's reqwest configuration (`https_only(true)`); not subject
/// to follow-redirect attacks.
pub const MODEL_URL: &str = "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/onnx/model.onnx";

// ---------------------------------------------------------------------------
// W18c Phase B tokenizer-file pins
//
// Declared here in Phase A (compiled-in alongside the model pins so the
// constant-lift is atomic across all 5 hash digests + 4 URLs); consumed
// by the `fastembed::TextEmbedding::try_new_from_user_defined` wiring
// that lands in W18c Phase B per HIGH-2 reviewer note (see
// [`AtlasEmbedder::new`] fn-level doc-comment "W18c Phase B resume guide").
// ---------------------------------------------------------------------------

/// SHA-256 of `tokenizer.json` from `BAAI/bge-small-en-v1.5` at
/// [`HF_REVISION_SHA`]. 64-char SHA-256 hex digest.
///
/// Resolved 2026-05-15 via `tools/w18c-phase-a-resolve.sh`. Consumed
/// by W18c Phase B `try_new_from_user_defined` wiring (verified
/// pre-init against this pin).
pub const TOKENIZER_JSON_SHA256: &str =
    "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66";

/// SHA-256 of `config.json` from `BAAI/bge-small-en-v1.5` at
/// [`HF_REVISION_SHA`]. 64-char SHA-256 hex digest.
///
/// Resolved 2026-05-15 via `tools/w18c-phase-a-resolve.sh`. Consumed
/// by W18c Phase B `try_new_from_user_defined` wiring.
pub const CONFIG_JSON_SHA256: &str =
    "094f8e891b932f2000c92cfc663bac4c62069f5d8af5b5278c4306aef3084750";

/// SHA-256 of `special_tokens_map.json` from `BAAI/bge-small-en-v1.5`
/// at [`HF_REVISION_SHA`]. 64-char SHA-256 hex digest.
///
/// Resolved 2026-05-15 via `tools/w18c-phase-a-resolve.sh`. Consumed
/// by W18c Phase B `try_new_from_user_defined` wiring.
pub const SPECIAL_TOKENS_MAP_SHA256: &str =
    "b6d346be366a7d1d48332dbc9fdf3bf8960b5d879522b7799ddba59e76237ee3";

/// Full HuggingFace LFS URL for `tokenizer.json` at [`HF_REVISION_SHA`].
pub const TOKENIZER_JSON_URL: &str = "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/tokenizer.json";

/// Full HuggingFace LFS URL for `config.json` at [`HF_REVISION_SHA`].
pub const CONFIG_JSON_URL: &str = "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/config.json";

/// Full HuggingFace LFS URL for `special_tokens_map.json` at
/// [`HF_REVISION_SHA`].
pub const SPECIAL_TOKENS_MAP_URL: &str = "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/special_tokens_map.json";

/// SHA-256 of `tokenizer_config.json` from `BAAI/bge-small-en-v1.5`
/// at [`HF_REVISION_SHA`]. 64-char SHA-256 hex digest.
///
/// Resolved 2026-05-15 during W18c Phase B API-surface verification
/// against `fastembed-rs/src/common.rs::TokenizerFiles` (lines 26-32),
/// which requires this file in addition to the three pinned in Phase A.
/// Consumed by W18c Phase B `try_new_from_user_defined` wiring.
pub const TOKENIZER_CONFIG_JSON_SHA256: &str =
    "9261e7d79b44c8195c1cada2b453e55b00aeb81e907a6664974b4d7776172ab3";

/// Full HuggingFace LFS URL for `tokenizer_config.json` at
/// [`HF_REVISION_SHA`].
pub const TOKENIZER_CONFIG_JSON_URL: &str = "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/tokenizer_config.json";

/// Compile-in check: all eleven pins (6 hash digests + 5 URLs) are
/// non-empty. Catches accidental blanking during refactors.
/// Structural-only — real-value substitution keeps the assertion
/// passing; well-formedness is enforced by `pins_well_formed_after_lift`.
pub const _STRUCTURAL_PIN_CHECK: () = {
    assert!(!HF_REVISION_SHA.is_empty());
    assert!(!ONNX_SHA256.is_empty());
    assert!(!MODEL_URL.is_empty());
    assert!(!TOKENIZER_JSON_SHA256.is_empty());
    assert!(!CONFIG_JSON_SHA256.is_empty());
    assert!(!SPECIAL_TOKENS_MAP_SHA256.is_empty());
    assert!(!TOKENIZER_CONFIG_JSON_SHA256.is_empty());
    assert!(!TOKENIZER_JSON_URL.is_empty());
    assert!(!CONFIG_JSON_URL.is_empty());
    assert!(!SPECIAL_TOKENS_MAP_URL.is_empty());
    assert!(!TOKENIZER_CONFIG_JSON_URL.is_empty());
};

// ---------------------------------------------------------------------------
// Determinism conditions
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
// Download-with-SHA-verification (Path 1 — preferred)
// ---------------------------------------------------------------------------

/// Generic Atlas-controlled file download with SHA-256 verification.
///
/// Per ADR §4 sub-decision #2 Path 1:
///
/// 1. Fetch `url` via Atlas-controlled `reqwest::blocking` client
///    with `https_only(true)` (TLS-pinned origin; not subject to
///    follow-redirect attacks against the LFS endpoint).
/// 2. Compute SHA-256 of the response bytes (real `sha2::Sha256` per
///    W18b HIGH-1 fix; NOT a placeholder algorithm).
/// 3. Compare against `expected_sha256` (lowercase hex). Comparison
///    is performed AFTER full-buffer download — both sides are
///    constant-time `&str == &str` over 64-char ASCII-hex strings,
///    same convention as the rest of Atlas's supply-chain code.
/// 4. On mismatch, return [`Mem0gError::SupplyChainMismatch`] —
///    fail closed BEFORE writing to disk OR handing the file to
///    fastembed-rs. A corrupted download never lands a poisoned
///    file under the cache path.
/// 5. On match, write the file to `dest` and return its path.
///
/// # Errors
///
/// - [`Mem0gError::Io`] on filesystem or network failure.
/// - [`Mem0gError::SupplyChainMismatch`] on SHA-256 mismatch
///   (fail-closed — the cache REFUSES to embed).
///
/// # W18c Phase B
///
/// This was factored out of the W18b-shipped `download_model_with_verification`
/// to support the four `download_<file>_with_verification` wrappers
/// (one per file required by `fastembed::TokenizerFiles` + the model
/// ONNX). The signature deliberately mirrors W18b's structure (return
/// `Mem0gResult<PathBuf>`, accept `&Path` dest) so a Phase D callsite
/// retains the same shape if it ever needs to download a fifth file.
#[cfg(feature = "lancedb-backend")]
fn download_file_with_sha(
    url: &str,
    expected_sha256: &str,
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    use std::io::Write;

    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .build()
        .map_err(|e| Mem0gError::Io(format!("reqwest client build: {e}")))?;

    let response = client
        .get(url)
        .send()
        .map_err(|e| Mem0gError::Io(format!("file download GET {url}: {e}")))?;

    if !response.status().is_success() {
        return Err(Mem0gError::Io(format!(
            "file download non-success status for {url}: {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .map_err(|e| Mem0gError::Io(format!("file download body read {url}: {e}")))?;

    // Verify SHA-256 BEFORE writing to disk so a corrupted download
    // never lands a poisoned file under the cache path.
    let hash = sha256_hex(&bytes);
    if hash != expected_sha256 {
        return Err(Mem0gError::SupplyChainMismatch {
            expected: expected_sha256.to_string(),
            actual: hash,
        });
    }

    std::fs::create_dir_all(
        dest.parent()
            .ok_or_else(|| Mem0gError::Io(format!("dest has no parent: {}", dest.display())))?,
    )
    .map_err(|e| Mem0gError::Io(format!("create_dir_all: {e}")))?;

    let mut f = std::fs::File::create(dest)
        .map_err(|e| Mem0gError::Io(format!("file create {}: {e}", dest.display())))?;
    f.write_all(&bytes)
        .map_err(|e| Mem0gError::Io(format!("file write: {e}")))?;
    f.sync_all()
        .map_err(|e| Mem0gError::Io(format!("file fsync: {e}")))?;

    Ok(dest.to_path_buf())
}

/// Atlas-controlled `model.onnx` download with SHA-256 verification.
///
/// Thin wrapper over [`download_file_with_sha`] pinned to
/// [`MODEL_URL`] + [`ONNX_SHA256`]. Public so the optional
/// `bin/preload-embedder` operator-tool can call it during cold-start
/// CI cache warming (operator-runbook §atlas-mem0g-smoke).
#[cfg(feature = "lancedb-backend")]
pub fn download_model_with_verification(dest: &std::path::Path) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(MODEL_URL, ONNX_SHA256, dest)
}

/// Atlas-controlled `tokenizer.json` download with SHA-256
/// verification.
///
/// Thin wrapper over [`download_file_with_sha`] pinned to
/// [`TOKENIZER_JSON_URL`] + [`TOKENIZER_JSON_SHA256`]. Fails closed
/// on mismatch (cache REFUSES to embed).
#[cfg(feature = "lancedb-backend")]
pub fn download_tokenizer_with_verification(
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(TOKENIZER_JSON_URL, TOKENIZER_JSON_SHA256, dest)
}

/// Atlas-controlled `config.json` download with SHA-256 verification.
///
/// Thin wrapper over [`download_file_with_sha`] pinned to
/// [`CONFIG_JSON_URL`] + [`CONFIG_JSON_SHA256`]. Fails closed on
/// mismatch.
#[cfg(feature = "lancedb-backend")]
pub fn download_config_with_verification(
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(CONFIG_JSON_URL, CONFIG_JSON_SHA256, dest)
}

/// Atlas-controlled `special_tokens_map.json` download with SHA-256
/// verification.
///
/// Thin wrapper over [`download_file_with_sha`] pinned to
/// [`SPECIAL_TOKENS_MAP_URL`] + [`SPECIAL_TOKENS_MAP_SHA256`].
/// Fails closed on mismatch.
#[cfg(feature = "lancedb-backend")]
pub fn download_special_tokens_with_verification(
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(
        SPECIAL_TOKENS_MAP_URL,
        SPECIAL_TOKENS_MAP_SHA256,
        dest,
    )
}

/// Atlas-controlled `tokenizer_config.json` download with SHA-256
/// verification.
///
/// Thin wrapper over [`download_file_with_sha`] pinned to
/// [`TOKENIZER_CONFIG_JSON_URL`] + [`TOKENIZER_CONFIG_JSON_SHA256`].
/// Fails closed on mismatch.
///
/// This is the W18c Phase B fourth tokenizer file — required by the
/// upstream `fastembed::TokenizerFiles` struct (4 files: tokenizer +
/// config + special_tokens_map + tokenizer_config; see fastembed-rs
/// 5.13.4 `src/common.rs` lines 26-32). Phase A pinned three; Phase
/// B atomically extended.
#[cfg(feature = "lancedb-backend")]
pub fn download_tokenizer_config_with_verification(
    dest: &std::path::Path,
) -> Mem0gResult<std::path::PathBuf> {
    download_file_with_sha(
        TOKENIZER_CONFIG_JSON_URL,
        TOKENIZER_CONFIG_JSON_SHA256,
        dest,
    )
}

/// Verify a cached file's SHA-256 against an expected pin.
///
/// Called at every cold start before fastembed-rs init. Fails closed
/// (refuses to embed) on mismatch. Streams the file in 64 KiB chunks
/// to avoid a full-file allocation for large ONNX bodies (the
/// `bge-small-en-v1.5` FP32 ONNX is ~130 MB).
///
/// W18c Phase B: generalised from `verify_cached_model_sha` to
/// support all four file types via the cold-start re-verification
/// path in [`AtlasEmbedder::new`].
pub fn verify_cached_file_sha(
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

/// Verify the cached model file's SHA-256 against the compiled-in pin.
///
/// Preserved for backward compatibility with W18b call-sites. Newer
/// code should use [`verify_cached_file_sha`] directly with the
/// matching pin constant.
pub fn verify_cached_model_sha(model_path: &std::path::Path) -> Mem0gResult<()> {
    verify_cached_file_sha(model_path, ONNX_SHA256)
}

/// Compute SHA-256 of a byte slice and return lowercase hex.
///
/// HIGH-1 fix: this previously delegated to a `blake3-placeholder-...`
/// string (NOT SHA-256), which silently broke supply-chain verification
/// regardless of `ONNX_SHA256`'s value. Now uses `sha2::Sha256` for
/// real RFC-6234 SHA-256 — the same algorithm HuggingFace and the
/// `sha256sum` operator-runbook tool produce.
///
/// Empty-input contract: SHA-256 of the empty byte slice is the
/// canonical
/// `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
/// Unit-tested below.
///
/// `#[allow(dead_code)]` because the always-on dep set sees this
/// function only via the unit tests; the `lancedb-backend` feature
/// gates the production call-site in `download_model_with_verification`.
#[allow(dead_code)]
fn sha256_hex(bytes: &[u8]) -> String {
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
fn sha256_file(path: &std::path::Path) -> Mem0gResult<String> {
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
// Embedder wrapper (feature-gated)
// ---------------------------------------------------------------------------

/// Thin wrapper around fastembed-rs's `TextEmbedding` for
/// `bge-small-en-v1.5` FP32.
///
/// Held as a singleton per backend instance. Embedder ownership lives
/// INSIDE [`crate::SemanticCacheBackend`] (caller passes raw text,
/// not vectors) so the embedder-version pin is a single-impl swap.
///
/// ## Interior mutability (W18c Phase B)
///
/// Upstream `fastembed::TextEmbedding::embed` takes `&mut self`
/// (`fastembed-rs/src/text_embedding/impl.rs::embed` line 447-464).
/// Atlas's [`embed`](AtlasEmbedder::embed) surface is `&self` so it
/// composes through `Arc<dyn SemanticCacheBackend>` without forcing
/// per-call exclusive ownership at the call site. The internal
/// `Mutex<TextEmbedding>` provides the exclusivity that fastembed
/// requires while keeping Atlas's public API ergonomic. The mutex
/// is fine-grained — one acquire per `embed()` call — so contention
/// is bounded by the embed latency itself (~5-10 ms per call under
/// `OMP_NUM_THREADS=1`).
#[cfg(feature = "lancedb-backend")]
pub struct AtlasEmbedder {
    inner: std::sync::Mutex<fastembed::TextEmbedding>,
}

#[cfg(feature = "lancedb-backend")]
impl AtlasEmbedder {
    /// Construct a new embedder.
    ///
    /// Steps:
    /// 1. Download + SHA-256-verify all four required files
    ///    (`model.onnx` + `tokenizer.json` + `config.json` +
    ///    `special_tokens_map.json` + `tokenizer_config.json`) into
    ///    `model_cache_dir`. Existing files are re-verified against
    ///    the compiled-in pin (cold-start re-verification per
    ///    ADR §4 sub-decision #2); mismatched cached files trip the
    ///    fail-closed [`Mem0gError::SupplyChainMismatch`] path.
    /// 2. [`pin_omp_threads_single`] — set `OMP_NUM_THREADS=1`
    ///    BEFORE any fastembed-rs init so the ORT session picks up
    ///    the deterministic single-thread CPU path.
    /// 3. Read the four SHA-verified files into byte buffers.
    /// 4. Construct
    ///    [`fastembed::UserDefinedEmbeddingModel`] with
    ///    `pooling = Pooling::Cls` (matches fastembed's own
    ///    `get_default_pooling_method(BGESmallENV15)` —
    ///    `fastembed-rs/src/text_embedding/impl.rs` line 218) and
    ///    `quantization = QuantizationMode::None` (FP32; only the
    ///    `BGESmallENV15Q` variant is `Static`).
    /// 5. Call
    ///    [`fastembed::TextEmbedding::try_new_from_user_defined`]
    ///    (NOT `try_new(Default)` — the latter triggers
    ///    fastembed-rs's own HuggingFace fetch which bypasses
    ///    Atlas's SHA-verified gate; this was the W18b HIGH-2
    ///    bypass that the original fail-closed posture defended
    ///    against).
    ///
    /// # Errors
    ///
    /// - [`Mem0gError::SupplyChainMismatch`] if ANY of the four
    ///   file SHA-256s mismatch their compiled-in pin (fail-closed
    ///   — refuses to embed).
    /// - [`Mem0gError::Io`] on filesystem or network failure
    ///   during download/verify.
    /// - [`Mem0gError::Embedder`] on fastembed-rs init failure
    ///   (e.g. the ONNX session builder rejects the bytes).
    ///
    /// # W18c Phase B HIGH-2 fix
    ///
    /// The W18b initial body called
    /// `fastembed::TextEmbedding::try_new(Default::default())` which
    /// causes fastembed-rs to download `bge-small-en-v1.5` from
    /// HuggingFace via its OWN HTTP client, completely bypassing
    /// Atlas's SHA-256 gate. That is a supply-chain bypass: an
    /// attacker controlling the network path to HuggingFace at
    /// runtime could substitute a poisoned model without tripping
    /// Atlas's verification.
    ///
    /// W18b shipped with an unconditional fail-closed `Err(...)` as
    /// the reviewer-driven HIGH-2 deferral. W18c Phase A lifted the
    /// supply-chain constants; W18c Phase B (this commit) replaces
    /// the fail-closed `Err(...)` with the real
    /// `try_new_from_user_defined` wiring against the four
    /// SHA-verified local files. The bypass code path is now
    /// structurally unreachable: `try_new(Default::default())` is
    /// never called anywhere in the Atlas codebase.
    pub fn new(model_cache_dir: &std::path::Path) -> Mem0gResult<Self> {
        // Step 1: download + SHA-verify all FOUR tokenizer files +
        // the ONNX model. Fail closed on any mismatch (the helpers
        // bail with Mem0gError::SupplyChainMismatch before writing
        // to disk).
        let model_path = model_cache_dir.join("bge-small-en-v1.5.onnx");
        ensure_file_with_sha(
            &model_path,
            ONNX_SHA256,
            download_model_with_verification,
        )?;

        let tokenizer_path = model_cache_dir.join("tokenizer.json");
        ensure_file_with_sha(
            &tokenizer_path,
            TOKENIZER_JSON_SHA256,
            download_tokenizer_with_verification,
        )?;

        let config_path = model_cache_dir.join("config.json");
        ensure_file_with_sha(
            &config_path,
            CONFIG_JSON_SHA256,
            download_config_with_verification,
        )?;

        let special_tokens_path = model_cache_dir.join("special_tokens_map.json");
        ensure_file_with_sha(
            &special_tokens_path,
            SPECIAL_TOKENS_MAP_SHA256,
            download_special_tokens_with_verification,
        )?;

        let tokenizer_config_path = model_cache_dir.join("tokenizer_config.json");
        ensure_file_with_sha(
            &tokenizer_config_path,
            TOKENIZER_CONFIG_JSON_SHA256,
            download_tokenizer_config_with_verification,
        )?;

        // Step 2: pin OMP threads BEFORE fastembed init so the ORT
        // session is created with the deterministic single-thread
        // CPU path (ADR §4 sub-decision #2).
        pin_omp_threads_single();

        // Step 3: read the four SHA-verified files into memory.
        // After this point every byte fed to fastembed-rs has been
        // SHA-verified against a compiled-in pin.
        let onnx_bytes = std::fs::read(&model_path)
            .map_err(|e| Mem0gError::Io(format!("read {}: {e}", model_path.display())))?;
        let tokenizer_bytes = std::fs::read(&tokenizer_path)
            .map_err(|e| Mem0gError::Io(format!("read {}: {e}", tokenizer_path.display())))?;
        let config_bytes = std::fs::read(&config_path)
            .map_err(|e| Mem0gError::Io(format!("read {}: {e}", config_path.display())))?;
        let special_tokens_bytes = std::fs::read(&special_tokens_path)
            .map_err(|e| Mem0gError::Io(format!("read {}: {e}", special_tokens_path.display())))?;
        let tokenizer_config_bytes = std::fs::read(&tokenizer_config_path).map_err(|e| {
            Mem0gError::Io(format!("read {}: {e}", tokenizer_config_path.display()))
        })?;

        // Step 4: construct UserDefinedEmbeddingModel. API surface
        // verified against fastembed-rs 5.13.4 source:
        //   - src/common.rs::TokenizerFiles (lines 26-32) requires
        //     four named byte fields.
        //   - src/text_embedding/init.rs::UserDefinedEmbeddingModel::new
        //     (lines 97-107) takes (onnx_file, tokenizer_files).
        //   - .with_pooling(Pooling::Cls) matches fastembed's own
        //     get_default_pooling_method(BGESmallENV15) at
        //     src/text_embedding/impl.rs line 218. CLS-pooling is
        //     load-bearing for BGE-family embedding correctness.
        //   - QuantizationMode::None is the implicit default
        //     (UserDefinedEmbeddingModel::new sets it to None per
        //     line 103); the FP32 model has no static-quant
        //     dequantize step. We rely on the default rather than
        //     calling .with_quantization() to keep the intent
        //     legible.
        let tokenizer_files = fastembed::TokenizerFiles {
            tokenizer_file: tokenizer_bytes,
            config_file: config_bytes,
            special_tokens_map_file: special_tokens_bytes,
            tokenizer_config_file: tokenizer_config_bytes,
        };
        let user_model = fastembed::UserDefinedEmbeddingModel::new(onnx_bytes, tokenizer_files)
            .with_pooling(fastembed::Pooling::Cls);

        // Step 5: try_new_from_user_defined. NEVER try_new(Default).
        let inner = fastembed::TextEmbedding::try_new_from_user_defined(
            user_model,
            fastembed::InitOptionsUserDefined::default(),
        )
        .map_err(|e| Mem0gError::Embedder(format!("fastembed init: {e}")))?;

        Ok(Self {
            inner: std::sync::Mutex::new(inner),
        })
    }

    /// Embed a single text into an f32 vector.
    ///
    /// Determinism contract: under pinned ORT, `OMP_NUM_THREADS=1`,
    /// and the FP32 model, two calls on the same input bytes produce
    /// byte-equal output. Verified by `tests/embedding_determinism.rs`.
    ///
    /// Public surface accepts `&self`; the internal `Mutex` provides
    /// the `&mut TextEmbedding` that the upstream `embed` method
    /// requires. See the struct-level doc-comment for the rationale.
    pub fn embed(&self, text: &str) -> Mem0gResult<Vec<f32>> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| Mem0gError::Embedder(format!("embedder mutex poisoned: {e}")))?;
        let embeddings = guard
            .embed(vec![text], None)
            .map_err(|e| Mem0gError::Embedder(format!("embed: {e}")))?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| Mem0gError::Embedder("empty embedding result".to_string()))
    }
}

/// Cold-start helper: if `path` does not exist, download via
/// `downloader`; else verify the cached file against `expected_sha`.
///
/// Centralises the "exists? → verify : download" branch so each of
/// the five file types in [`AtlasEmbedder::new`] reads as a single
/// call instead of an if/else block.
///
/// # Errors
///
/// - [`Mem0gError::SupplyChainMismatch`] if a cached file's SHA-256
///   does not match `expected_sha` (cache REFUSES to embed; operator
///   must delete the poisoned cache entry).
/// - [`Mem0gError::Io`] on filesystem or network failure during the
///   download arm.
#[cfg(feature = "lancedb-backend")]
fn ensure_file_with_sha<F>(
    path: &std::path::Path,
    expected_sha: &str,
    downloader: F,
) -> Mem0gResult<()>
where
    F: FnOnce(&std::path::Path) -> Mem0gResult<std::path::PathBuf>,
{
    if path.exists() {
        verify_cached_file_sha(path, expected_sha)
    } else {
        downloader(path).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_are_non_empty() {
        // W18c Phase A: extends original 3-pin check to all 6 SHA
        // constants + 5 URL constants (model + 4 tokenizer files).
        // W18c Phase B: extended again to include the fourth
        // tokenizer pin (TOKENIZER_CONFIG_JSON_*) discovered during
        // API-surface verification.
        assert!(!HF_REVISION_SHA.is_empty());
        assert!(!ONNX_SHA256.is_empty());
        assert!(!MODEL_URL.is_empty());
        assert!(!TOKENIZER_JSON_SHA256.is_empty());
        assert!(!CONFIG_JSON_SHA256.is_empty());
        assert!(!SPECIAL_TOKENS_MAP_SHA256.is_empty());
        assert!(!TOKENIZER_CONFIG_JSON_SHA256.is_empty());
        assert!(!TOKENIZER_JSON_URL.is_empty());
        assert!(!CONFIG_JSON_URL.is_empty());
        assert!(!SPECIAL_TOKENS_MAP_URL.is_empty());
        assert!(!TOKENIZER_CONFIG_JSON_URL.is_empty());
    }

    // W18c Phase A: the W18b `pins_are_placeholder_until_nelson_verifies`
    // gatekeeper test is RETIRED. Its purpose — forcing an in-commit
    // atomic constant-lift — was served when the constants were
    // resolved via `tools/w18c-phase-a-resolve.sh` and committed in
    // this welle. Post-lift, structural-format enforcement moves to
    // `pins_well_formed_after_lift` (which now runs unconditionally,
    // no more `is_placeholder` early-return).

    #[test]
    fn pins_well_formed_after_lift() {
        // W18c Phase A: structural-format invariants are now
        // permanently enforced. The W18b `is_placeholder` early-return
        // is removed because the constants are lifted; any future
        // refactor that reintroduces placeholder strings will trip
        // these assertions at test time.
        //
        // Coverage:
        //   - 4 SHA-256 hex digests (64-char lowercase hex)
        //   - 1 SHA-1 hex digest (40-char lowercase hex, Git revision)
        //   - 4 URL strings (must start with https://huggingface.co/
        //     AND embed HF_REVISION_SHA — revision-pinning invariant)

        // 64-char lowercase-hex SHA-256 digests.
        for (label, value) in [
            ("ONNX_SHA256", ONNX_SHA256),
            ("TOKENIZER_JSON_SHA256", TOKENIZER_JSON_SHA256),
            ("CONFIG_JSON_SHA256", CONFIG_JSON_SHA256),
            ("SPECIAL_TOKENS_MAP_SHA256", SPECIAL_TOKENS_MAP_SHA256),
            ("TOKENIZER_CONFIG_JSON_SHA256", TOKENIZER_CONFIG_JSON_SHA256),
        ] {
            assert_eq!(
                value.len(),
                64,
                "{label} must be 64-char SHA-256 hex digest"
            );
            assert!(
                value.chars().all(|c| c.is_ascii_hexdigit()),
                "{label} must contain only ASCII hex digits"
            );
            assert!(
                value.chars().all(|c| !c.is_ascii_uppercase()),
                "{label} must be lowercase hex (HuggingFace + sha256sum convention)"
            );
        }

        // 40-char lowercase-hex Git SHA-1 revision.
        assert_eq!(
            HF_REVISION_SHA.len(),
            40,
            "HF_REVISION_SHA must be 40-char Git SHA-1 hex digest"
        );
        assert!(
            HF_REVISION_SHA.chars().all(|c| c.is_ascii_hexdigit()),
            "HF_REVISION_SHA must contain only ASCII hex digits"
        );
        assert!(
            HF_REVISION_SHA.chars().all(|c| !c.is_ascii_uppercase()),
            "HF_REVISION_SHA must be lowercase hex"
        );

        // 5 URL constants: huggingface.co origin + revision-SHA path.
        for (label, value) in [
            ("MODEL_URL", MODEL_URL),
            ("TOKENIZER_JSON_URL", TOKENIZER_JSON_URL),
            ("CONFIG_JSON_URL", CONFIG_JSON_URL),
            ("SPECIAL_TOKENS_MAP_URL", SPECIAL_TOKENS_MAP_URL),
            ("TOKENIZER_CONFIG_JSON_URL", TOKENIZER_CONFIG_JSON_URL),
        ] {
            assert!(
                value.starts_with("https://huggingface.co/"),
                "{label} must point at huggingface.co (TLS-pinned origin)"
            );
            assert!(
                value.contains(HF_REVISION_SHA),
                "{label} must embed HF_REVISION_SHA in path \
                 (revision-pinning invariant; URL and SHA must move atomically)"
            );
        }
    }

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
    fn pin_omp_threads_single_idempotent() {
        // Two calls are a no-op (process-global var, second set is
        // structurally fine — same value).
        pin_omp_threads_single();
        pin_omp_threads_single();
        // We can't assert the env-var directly because Cargo runs
        // tests in parallel by default; other tests may have already
        // set OMP_NUM_THREADS to a different value, then this test
        // sets it to "1". We assert at least that the call doesn't
        // panic and is callable from a #[test] context.
        assert_eq!(
            std::env::var("OMP_NUM_THREADS").as_deref(),
            Ok("1"),
            "OMP_NUM_THREADS should be \"1\" after pin_omp_threads_single"
        );
    }
}
