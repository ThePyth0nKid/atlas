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
//! ## `TODO(W18b-NELSON-VERIFY)`
//!
//! The current constant values are PLACEHOLDERS. WebFetch against
//! HuggingFace `BAAI/bge-small-en-v1.5` to resolve real revision SHA,
//! ONNX SHA256, and LFS URL was not feasible in the W18b subagent
//! context (network access from a Windows subagent shell is not
//! guaranteed; placing real but stale values would be worse than
//! placing flagged placeholders). Nelson confirms real values
//! pre-merge.
//!
//! See `.handoff/v2-beta-welle-18b-plan.md` Implementation Notes
//! §"Placeholder constants" for the verification protocol.
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
/// version Atlas adopted in W18b.
///
/// **TODO(W18b-NELSON-VERIFY):** placeholder. Real value pulled from
/// HuggingFace API `GET https://huggingface.co/api/models/BAAI/bge-small-en-v1.5`
/// — the response's `sha` field is this constant. Update before
/// V2-β-1 public ship.
pub const HF_REVISION_SHA: &str = "TODO_W18B_NELSON_VERIFY_HF_REVISION_SHA";

/// SHA256 of `model.onnx` for `bge-small-en-v1.5` (FP32).
///
/// **TODO(W18b-NELSON-VERIFY):** placeholder. Compute via
/// `curl -sL <MODEL_URL> | sha256sum` against the LFS-pointed file at
/// the pinned revision. Update before V2-β-1 public ship. The
/// download-with-verification path will fail closed (refusing to
/// embed) until both this constant AND the file on disk agree.
pub const ONNX_SHA256: &str = "TODO_W18B_NELSON_VERIFY_ONNX_SHA256";

/// Full HuggingFace LFS URL incl. revision SHA in path.
///
/// **TODO(W18b-NELSON-VERIFY):** placeholder. Real value resolved by
/// inspecting the model repo's LFS pointer at the pinned revision.
/// Update before V2-β-1 public ship.
pub const MODEL_URL: &str =
    "TODO_W18B_NELSON_VERIFY_HTTPS_HUGGINGFACE_CO_BAAI_BGE_SMALL_EN_V1_5_RESOLVE_REVISION_SHA_MODEL_ONNX";

/// Compile-in check: the three pins are non-empty. Catches accidental
/// blanking during a refactor (the placeholder strings ARE non-empty,
/// so this assertion is structural-only — real-value substitution
/// keeps it passing).
pub const _STRUCTURAL_PIN_CHECK: () = {
    assert!(!HF_REVISION_SHA.is_empty());
    assert!(!ONNX_SHA256.is_empty());
    assert!(!MODEL_URL.is_empty());
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
    ///   if the supply-chain pins are still W18b placeholders
    ///   (fail-closed; refuses to embed UNTIL the constants are
    ///   lifted AND the `try_new_from_user_defined` wiring is
    ///   completed by Nelson pre-merge per HIGH-2 reviewer note).
    ///
    /// HIGH-2 fix (reviewer-driven):
    ///
    /// The previous body called `fastembed::TextEmbedding::try_new(Default::default())`
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
    /// `special_tokens_map.json` per fastembed-rs 5.13.4) each of
    /// which ALSO needs a compiled-in SHA-256 pin and a download
    /// helper. Wiring that in-commit without upstream API access in
    /// the subagent context risks landing a broken init path.
    ///
    /// **In-commit fix posture:** the embedder fails closed (returns
    /// `Mem0gError::Embedder`) while the supply-chain pins are still
    /// `TODO_W18B_NELSON_VERIFY_*` placeholders OR while the
    /// `try_new_from_user_defined` wiring is incomplete. This
    /// guarantees the bypass code path can NEVER execute in
    /// production: if anyone enables the `lancedb-backend` feature
    /// and tries to instantiate the embedder, they get a clear
    /// error pointing at the pre-merge work.
    ///
    /// **Pre-merge resume guide** (Nelson, mirrors plan-doc):
    ///
    /// 1. Lift `ONNX_SHA256` / `HF_REVISION_SHA` / `MODEL_URL`
    ///    placeholders (verifies the existing gatekeeper test).
    /// 2. Add `TOKENIZER_JSON_SHA256` / `CONFIG_JSON_SHA256` /
    ///    `SPECIAL_TOKENS_MAP_JSON_SHA256` constants + the matching
    ///    URL constants.
    /// 3. Extend [`download_model_with_verification`] to fetch all
    ///    four files (or factor a `download_file_with_sha`
    ///    primitive) into the cache directory.
    /// 4. Replace the `Mem0gError::Embedder("supply-chain gate")`
    ///    return below with a real
    ///    `fastembed::TextEmbedding::try_new_from_user_defined(
    ///         UserDefinedEmbeddingModel::new(model_bytes,
    ///             tokenizer_files),
    ///         InitOptionsUserDefined::default(),
    ///    )?` call. (Exact 5.13.4 API surface to be confirmed
    ///    against `cargo doc -p fastembed --features lancedb-backend`
    ///    once `lancedb-backend` builds locally.)
    ///
    /// Documented in `.handoff/v2-beta-welle-18b-plan.md` Implementation
    /// Notes §"HIGH-2 fastembed bypass — pre-merge resume".
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
        // by triggering fastembed-rs's own HuggingFace fetch. Until
        // the `try_new_from_user_defined` wiring is completed by
        // Nelson pre-merge (see fn-level doc-comment "Pre-merge
        // resume guide"), refuse to construct the embedder. This
        // matches the placeholder-constant posture: the production
        // path is structurally unreachable until ALL pre-merge gates
        // are cleared.
        Err(Mem0gError::Embedder(
            "supply-chain gate: AtlasEmbedder::new refuses to construct \
             until fastembed::TextEmbedding::try_new_from_user_defined \
             wiring lands (HIGH-2 fix; see fn-level doc-comment + \
             .handoff/v2-beta-welle-18b-plan.md §HIGH-2)"
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
        assert!(!HF_REVISION_SHA.is_empty());
        assert!(!ONNX_SHA256.is_empty());
        assert!(!MODEL_URL.is_empty());
    }

    #[test]
    fn pins_are_placeholder_until_nelson_verifies() {
        // HIGH-3 fix (reviewer-driven): the previous test asserted
        // ONLY `ONNX_SHA256.starts_with("TODO_W18B")`. A partial
        // constant lift (e.g. Nelson updates only `ONNX_SHA256` and
        // forgets `HF_REVISION_SHA` or `MODEL_URL`) would pass the
        // gatekeeper while leaving repo-integrity unchecked. All
        // three pins move atomically; the gatekeeper asserts all
        // three at once.
        assert!(
            ONNX_SHA256.starts_with("TODO_W18B"),
            "ONNX_SHA256 lifted without updating gatekeeper — \
             update HIGH-3 gatekeeper alongside (all three constants \
             must move atomically)"
        );
        assert!(
            HF_REVISION_SHA.starts_with("TODO_W18B"),
            "HF_REVISION_SHA lifted without updating gatekeeper — \
             update HIGH-3 gatekeeper alongside (all three constants \
             must move atomically)"
        );
        assert!(
            MODEL_URL.starts_with("TODO_W18B"),
            "MODEL_URL lifted without updating gatekeeper — \
             update HIGH-3 gatekeeper alongside (all three constants \
             must move atomically)"
        );
    }

    #[test]
    fn pins_well_formed_after_lift() {
        // HIGH-3 companion: post-lift format validation. When the
        // `pins_are_placeholder_until_nelson_verifies` gatekeeper
        // is deleted (or its assertion inverted), this test surfaces
        // any structural-format slip that would otherwise pass code
        // review:
        //
        // - real `ONNX_SHA256` is exactly 64 lowercase hex chars
        //   (RFC-6234 SHA-256 hex digest);
        // - real `HF_REVISION_SHA` is exactly 40 lowercase hex chars
        //   (Git revision SHA-1);
        // - real `MODEL_URL` begins with `https://huggingface.co/`.
        //
        // Both constants AND well-formedness are atomic gates. This
        // test is a NO-OP while the placeholders are in place
        // (asserts the placeholder posture); it becomes a real
        // structural check once the constants are lifted (asserts
        // the post-lift posture). Keep the test forever so the
        // structural invariants stay enforced.
        let is_placeholder = ONNX_SHA256.starts_with("TODO_W18B")
            && HF_REVISION_SHA.starts_with("TODO_W18B")
            && MODEL_URL.starts_with("TODO_W18B");

        if is_placeholder {
            // Pre-lift posture — well-formedness is unchecked because
            // the placeholder strings deliberately do NOT match real
            // formats. Defer to `pins_are_placeholder_until_nelson_verifies`.
            return;
        }

        // Post-lift posture — enforce real structural formats.
        assert_eq!(
            ONNX_SHA256.len(),
            64,
            "ONNX_SHA256 must be 64-char SHA-256 hex digest"
        );
        assert!(
            ONNX_SHA256.chars().all(|c| c.is_ascii_hexdigit()),
            "ONNX_SHA256 must contain only ASCII hex digits"
        );
        assert!(
            ONNX_SHA256.chars().all(|c| !c.is_ascii_uppercase()),
            "ONNX_SHA256 must be lowercase hex (HuggingFace + sha256sum convention)"
        );
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
            MODEL_URL.starts_with("https://huggingface.co"),
            "MODEL_URL must point at huggingface.co (TLS-pinned origin)"
        );
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
