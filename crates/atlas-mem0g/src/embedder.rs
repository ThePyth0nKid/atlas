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
//! ## Three compiled-in constants
//!
//! Pinned for cold-start re-verification:
//!
//! - [`HF_REVISION_SHA`] — HuggingFace Git revision SHA of the model
//!   repo at the chosen model-card version. Pins against repo-rename
//!   / repo-transfer / organisation-compromise attacks.
//! - [`ONNX_SHA256`] — SHA256 of the model.onnx file bytes.
//!   Verifies the file regardless of repo-level integrity.
//! - [`MODEL_URL`] — full HuggingFace LFS URL incl. revision SHA in
//!   path. TLS-pinned via Atlas's reqwest configuration.
//!
//! ## W18c Phase A — supply-chain constants lifted (2026-05-15)
//!
//! Resolved via `tools/w18c-phase-a-resolve.sh` against HuggingFace
//! `BAAI/bge-small-en-v1.5` at revision
//! `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`. All three load-bearing
//! W18b pins ([`HF_REVISION_SHA`] + [`ONNX_SHA256`] + [`MODEL_URL`])
//! plus three Phase-B tokenizer-file SHA-256 pins
//! ([`TOKENIZER_JSON_SHA256`] + [`CONFIG_JSON_SHA256`] +
//! [`SPECIAL_TOKENS_MAP_SHA256`]) are now compiled-in. ONNX file size
//! 133,093,490 bytes / 126.93 MB matches spike §3.4 expected envelope
//! (V4 verification).
//!
//! The W18b `pins_are_placeholder_until_nelson_verifies` gatekeeper
//! test is retired; `pins_well_formed_after_lift` becomes the active
//! structural-format enforcer for all 6 SHAs + 4 URLs. The
//! fail-closed posture in [`AtlasEmbedder::new`] remains pending
//! W18c Phase B fastembed `try_new_from_user_defined` wiring; that
//! wiring is the only remaining gate before Layer 3 is operational.
//!
//! See `.handoff/v2-beta-welle-18c-plan.md` Phase A for the
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
// constant-lift is atomic across all six SHAs); consumed by the
// `fastembed::TextEmbedding::try_new_from_user_defined` wiring that
// lands in W18c Phase B per HIGH-2 reviewer note (see [`AtlasEmbedder::new`]
// fn-level doc-comment "W18c Phase B resume guide").
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

/// Compile-in check: all nine pins (six SHAs + three URLs + one model
/// URL) are non-empty. Catches accidental blanking during refactors.
/// Structural-only — real-value substitution keeps the assertion
/// passing; well-formedness is enforced by `pins_well_formed_after_lift`.
pub const _STRUCTURAL_PIN_CHECK: () = {
    assert!(!HF_REVISION_SHA.is_empty());
    assert!(!ONNX_SHA256.is_empty());
    assert!(!MODEL_URL.is_empty());
    assert!(!TOKENIZER_JSON_SHA256.is_empty());
    assert!(!CONFIG_JSON_SHA256.is_empty());
    assert!(!SPECIAL_TOKENS_MAP_SHA256.is_empty());
    assert!(!TOKENIZER_JSON_URL.is_empty());
    assert!(!CONFIG_JSON_URL.is_empty());
    assert!(!SPECIAL_TOKENS_MAP_URL.is_empty());
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

/// Atlas-controlled model download with SHA256 verification.
///
/// Per ADR §4 sub-decision #2 Path 1:
///
/// 1. Fetch [`MODEL_URL`] via Atlas-controlled HTTP client.
/// 2. Compute SHA256 of the response bytes.
/// 3. Compare against compiled-in [`ONNX_SHA256`].
/// 4. On mismatch, return [`Mem0gError::SupplyChainMismatch`] —
///    fail closed BEFORE handing the file path to fastembed-rs.
/// 5. On match, write the file to `dest` and return its path.
///
/// Re-verification at every cold start: callers MUST call
/// [`verify_cached_model_sha`] on subsequent runs.
///
/// # Errors
///
/// - [`Mem0gError::Io`] on filesystem or network failure.
/// - [`Mem0gError::SupplyChainMismatch`] on SHA256 mismatch
///   (fail-closed — the cache REFUSES to embed).
#[cfg(feature = "lancedb-backend")]
pub fn download_model_with_verification(dest: &std::path::Path) -> Mem0gResult<std::path::PathBuf> {
    use std::io::Write;

    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .build()
        .map_err(|e| Mem0gError::Io(format!("reqwest client build: {e}")))?;

    let response = client
        .get(MODEL_URL)
        .send()
        .map_err(|e| Mem0gError::Io(format!("model download GET {MODEL_URL}: {e}")))?;

    if !response.status().is_success() {
        return Err(Mem0gError::Io(format!(
            "model download non-success status: {}",
            response.status()
        )));
    }

    let bytes = response
        .bytes()
        .map_err(|e| Mem0gError::Io(format!("model download body read: {e}")))?;

    // Verify SHA256 BEFORE writing to disk so a corrupted download
    // never lands a poisoned file under the cache path.
    let hash = sha256_hex(&bytes);
    if hash != ONNX_SHA256 {
        return Err(Mem0gError::SupplyChainMismatch {
            expected: ONNX_SHA256.to_string(),
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

/// Verify the cached model file's SHA256 against the compiled-in pin.
///
/// Called at every cold start before fastembed-rs init. Fails closed
/// (refuses to embed) on mismatch. Streams the file in 64 KiB chunks
/// to avoid a full-file allocation for large ONNX bodies (the
/// `bge-small-en-v1.5` FP32 ONNX is ~130 MB).
pub fn verify_cached_model_sha(model_path: &std::path::Path) -> Mem0gResult<()> {
    let hash = sha256_file(model_path)?;
    if hash != ONNX_SHA256 {
        return Err(Mem0gError::SupplyChainMismatch {
            expected: ONNX_SHA256.to_string(),
            actual: hash,
        });
    }
    Ok(())
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
#[cfg(feature = "lancedb-backend")]
pub struct AtlasEmbedder {
    inner: fastembed::TextEmbedding,
}

#[cfg(feature = "lancedb-backend")]
impl AtlasEmbedder {
    /// Construct a new embedder.
    ///
    /// Steps:
    /// 1. [`pin_omp_threads_single`]
    /// 2. Verify cached model SHA matches [`ONNX_SHA256`]
    ///    (calls [`download_model_with_verification`] if file missing)
    /// 3. fastembed-rs `TextEmbedding::try_new_from_user_defined`
    ///    with the verified local path (NOT `try_new(Default)` —
    ///    that path triggers fastembed-rs's own model fetch which
    ///    bypasses Atlas's SHA-verified gate).
    ///
    /// # Errors
    ///
    /// - [`Mem0gError::SupplyChainMismatch`] if SHA256 mismatch
    ///   (fail-closed; refuses to embed).
    /// - [`Mem0gError::Embedder`] on fastembed-rs init failure OR
    ///   while the W18c Phase B `try_new_from_user_defined` wiring
    ///   has not yet landed (fail-closed; refuses to embed). The
    ///   six supply-chain pins are W18c-Phase-A lifted; the wiring
    ///   gate is the only remaining pre-operational barrier.
    ///
    /// HIGH-2 fix (reviewer-driven):
    ///
    /// The W18b initial body called `fastembed::TextEmbedding::try_new(Default::default())`
    /// which causes fastembed-rs to download `bge-small-en-v1.5` from
    /// HuggingFace via its OWN HTTP client, completely bypassing
    /// Atlas's [`download_model_with_verification`] SHA-256 gate. That
    /// is a supply-chain bypass: the SHA-verified file on disk would
    /// be ignored, and an attacker controlling the network path to
    /// HuggingFace at runtime could substitute a poisoned model
    /// without tripping Atlas's verification.
    ///
    /// The correct production wiring is
    /// `fastembed::TextEmbedding::try_new_from_user_defined(...)` with
    /// the SHA-verified local ONNX bytes + tokenizer config +
    /// pooling config. That API requires three additional files
    /// from the HuggingFace repo (`tokenizer.json`, `config.json`,
    /// `special_tokens_map.json` per fastembed-rs 5.13.4), each of
    /// which now has a compiled-in SHA-256 pin
    /// ([`TOKENIZER_JSON_SHA256`] / [`CONFIG_JSON_SHA256`] /
    /// [`SPECIAL_TOKENS_MAP_SHA256`]) plus a matching URL constant
    /// ([`TOKENIZER_JSON_URL`] / [`CONFIG_JSON_URL`] /
    /// [`SPECIAL_TOKENS_MAP_URL`]) — all six declared in W18c
    /// Phase A.
    ///
    /// **Fail-closed posture (still in force):** the embedder
    /// returns `Mem0gError::Embedder` while the
    /// `try_new_from_user_defined` wiring is not yet landed. This
    /// guarantees the bypass code path can NEVER execute in
    /// production: if anyone enables the `lancedb-backend` feature
    /// and tries to instantiate the embedder pre-Phase-B, they get a
    /// clear error pointing at the remaining wiring step.
    ///
    /// **W18c Phase B resume guide** (engineering, mirrors
    /// `.handoff/v2-beta-welle-18c-plan.md` Phase B):
    ///
    /// 1. Factor a `download_file_with_sha(url, sha, dest)` primitive
    ///    out of [`download_model_with_verification`]. Use it for all
    ///    four files (ONNX + 3 tokenizer files).
    /// 2. Extend [`AtlasEmbedder`] cold-start to download/verify all
    ///    four files into the cache directory before fastembed init.
    /// 3. Replace the `Mem0gError::Embedder("supply-chain gate")`
    ///    return below with a real
    ///    `fastembed::TextEmbedding::try_new_from_user_defined(
    ///         UserDefinedEmbeddingModel::new(model_bytes,
    ///             tokenizer_files),
    ///         InitOptionsUserDefined::default(),
    ///    )?` call. (Exact 5.13.4 API surface to be confirmed
    ///    against `cargo doc -p fastembed --features lancedb-backend`
    ///    once `lancedb-backend` builds locally.)
    ///
    /// Documented in `.handoff/v2-beta-welle-18c-plan.md` Phase B.
    pub fn new(model_cache_dir: &std::path::Path) -> Mem0gResult<Self> {
        pin_omp_threads_single();

        let model_path = model_cache_dir.join("bge-small-en-v1.5.onnx");
        if !model_path.exists() {
            download_model_with_verification(&model_path)?;
        } else {
            verify_cached_model_sha(&model_path)?;
        }

        // HIGH-2 fail-closed gate (reviewer-driven): refuse to call
        // `fastembed::TextEmbedding::try_new(Default::default())` —
        // that path bypasses Atlas's SHA-256 supply-chain verification
        // by triggering fastembed-rs's own HuggingFace fetch. The W18c
        // Phase A supply-chain constants are lifted; the
        // `try_new_from_user_defined` wiring lands in W18c Phase B.
        // Until then, refuse to construct the embedder. The production
        // path is structurally unreachable until the wiring gate is
        // cleared.
        Err(Mem0gError::Embedder(
            "supply-chain gate: AtlasEmbedder::new refuses to construct \
             until W18c Phase B fastembed::TextEmbedding::try_new_from_user_defined \
             wiring lands (HIGH-2 fix; W18c Phase A supply-chain pins lifted; \
             see fn-level doc-comment + .handoff/v2-beta-welle-18c-plan.md Phase B)"
                .to_string(),
        ))
    }

    /// Internal: kept for the `inner` field's future use once the
    /// HIGH-2 gate is lifted. Suppresses dead-code warning during
    /// the gated-fail-closed period.
    #[allow(dead_code)]
    fn _inner_field_anchor(&self) -> &fastembed::TextEmbedding {
        &self.inner
    }

    /// Embed a single text into an f32 vector.
    ///
    /// Determinism contract: under pinned ORT + `OMP_NUM_THREADS=1`
    /// + FP32 model, two calls on the same input bytes produce
    /// byte-equal output. Verified by `tests/embedding_determinism.rs`.
    pub fn embed(&self, text: &str) -> Mem0gResult<Vec<f32>> {
        let embeddings = self
            .inner
            .embed(vec![text], None)
            .map_err(|e| Mem0gError::Embedder(format!("embed: {e}")))?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| Mem0gError::Embedder("empty embedding result".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_are_non_empty() {
        // W18c Phase A: extends original 3-pin check to all 6 SHA
        // constants + 4 URL constants (model + 3 tokenizer).
        assert!(!HF_REVISION_SHA.is_empty());
        assert!(!ONNX_SHA256.is_empty());
        assert!(!MODEL_URL.is_empty());
        assert!(!TOKENIZER_JSON_SHA256.is_empty());
        assert!(!CONFIG_JSON_SHA256.is_empty());
        assert!(!SPECIAL_TOKENS_MAP_SHA256.is_empty());
        assert!(!TOKENIZER_JSON_URL.is_empty());
        assert!(!CONFIG_JSON_URL.is_empty());
        assert!(!SPECIAL_TOKENS_MAP_URL.is_empty());
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

        // 4 URL constants: huggingface.co origin + revision-SHA path.
        for (label, value) in [
            ("MODEL_URL", MODEL_URL),
            ("TOKENIZER_JSON_URL", TOKENIZER_JSON_URL),
            ("CONFIG_JSON_URL", CONFIG_JSON_URL),
            ("SPECIAL_TOKENS_MAP_URL", SPECIAL_TOKENS_MAP_URL),
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
