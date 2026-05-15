//! V2-β Welle 18b: fastembed-rs wrapper + Atlas-controlled
//! download-with-SHA-verification per ADR-Atlas-012 §4 sub-decision #2.
//!
//! ## Supply-chain controls (closes security-reviewer HIGH-2)
//!
//! The model download is NOT delegated to fastembed-rs's default
//! download behaviour. Atlas wraps it in
//! [`download_model_with_verification`](crate::supply_chain::download_model_with_verification)
//! which:
//!
//! 1. Fetches the ONNX file via an Atlas-controlled `reqwest` client
//!    (rustls-tls; same TLS posture as atlas-projector's ArcadeDB
//!    HTTP path) with `https_only(true)` + 300s/30s timeouts.
//! 2. Streams the body to a sibling `.partial` path (no full-body
//!    in-memory buffering of the 130 MB ONNX).
//! 3. Reads-and-verifies SHA-256 in a SINGLE buffer (TOCTOU-free —
//!    the bytes fed to fastembed-rs ARE the bytes that were hashed).
//! 4. Fails closed on mismatch ([`crate::Mem0gError::SupplyChainMismatch`]).
//!
//! The primitives ([`crate::supply_chain::download_file_with_sha`],
//! [`crate::supply_chain::read_and_verify`],
//! [`crate::supply_chain::ensure_and_read_verified`],
//! [`crate::supply_chain::pin_omp_threads_single`], and the five
//! `download_<file>_with_verification` wrappers) live in
//! [`crate::supply_chain`]; this module hosts the pin CONSTANTS (the
//! supply-chain contract) and the `AtlasEmbedder` struct that
//! consumes them.
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
//! `AtlasEmbedder::new` is now operational: SHA-verify-all-four-and-read,
//! then `pin_omp_threads_single`, then
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
//! ## W18c Phase B fix-commit (2026-05-15)
//!
//! Reviewer-driven follow-up commit on top of the Phase B implementation:
//!
//! - **TOCTOU defence (security HIGH-1):** the previous code path
//!   verified the cached file's SHA via `verify_cached_file_sha`,
//!   then SEPARATELY called `std::fs::read(path)` to load bytes for
//!   fastembed. Between those two calls the file on disk CAN be
//!   atomically swapped (rename/symlink-swap). The fix collapses
//!   verify-and-use into a single primitive `read_and_verify`:
//!   read once into a `Vec<u8>`, hash the in-memory bytes, compare.
//!   The bytes fed to fastembed ARE the bytes that were verified.
//! - **reqwest timeouts (code HIGH-2):** the client builder now
//!   pins `timeout(300s)` + `connect_timeout(30s)`. Prevents a
//!   stalled HF endpoint from blocking `AtlasEmbedder::new`
//!   indefinitely (5 serial downloads).
//! - **Streaming download (security MEDIUM-1):** the ONNX body
//!   (~130 MB) streams to disk via `Response::copy_to` instead of
//!   `Response::bytes().into_vec()`. No double-allocation.
//! - **Module split (code MEDIUM-2):** supply-chain primitives
//!   moved to [`crate::supply_chain`]; this module is now under
//!   the 800-LOC hard limit.
//! - **Visibility (code MEDIUM-3, MEDIUM-4):** 4 tokenizer download
//!   wrappers + `verify_cached_file_sha` demoted from `pub` to
//!   `pub(crate)`. Only `download_model_with_verification` (planned
//!   `bin/preload-embedder` consumer) stays `pub`.
//! - **Phase A resolver script (security MEDIUM-2):** extended to
//!   include `tokenizer_config.json` in the iterated tokenizer set
//!   (4 files now, not 3). Strengthens auditable provenance for
//!   future supply-chain rotations.
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

#[cfg(feature = "lancedb-backend")]
use crate::Mem0gError;
use crate::Mem0gResult;

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
// constant-lift is atomic across all 6 hash digests + 5 URLs); consumed
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
///
/// Phase B fix-commit (security MEDIUM-2): `tools/w18c-phase-a-resolve.sh`
/// has been extended to include this fourth file in the resolver
/// loop, strengthening the auditable-provenance chain for future
/// rotations.
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
// Cached-model SHA verifier (W18b backward-compat surface)
// ---------------------------------------------------------------------------

/// Verify the cached model file's SHA-256 against the compiled-in pin.
///
/// Preserved as `pub` for backward compatibility with W18b call-sites
/// and the documented `bin/preload-embedder` operator-tool surface.
/// Newer in-crate code should use
/// [`crate::supply_chain::read_and_verify`] (TOCTOU-free contract for
/// downstream byte use).
pub fn verify_cached_model_sha(model_path: &std::path::Path) -> Mem0gResult<()> {
    crate::supply_chain::verify_cached_file_sha(model_path, ONNX_SHA256)
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
    /// 1. Download + SHA-256-verify-and-read all five files (ONNX
    ///    model + four tokenizer files:
    ///    `model.onnx` + `tokenizer.json` + `config.json` +
    ///    `special_tokens_map.json` + `tokenizer_config.json`) into
    ///    `model_cache_dir`. Existing files are re-verified against
    ///    the compiled-in pin (cold-start re-verification per
    ///    ADR §4 sub-decision #2); mismatched cached files trip the
    ///    fail-closed [`Mem0gError::SupplyChainMismatch`] path.
    ///    The verify-and-read fuses into a single
    ///    [`crate::supply_chain::ensure_and_read_verified`] call per
    ///    file so the bytes consumed in step 4 ARE the bytes that
    ///    were SHA-verified (TOCTOU-free; security HIGH-1 fix).
    /// 2. [`crate::supply_chain::pin_omp_threads_single`] — set
    ///    `OMP_NUM_THREADS=1` BEFORE any fastembed-rs init so the
    ///    ORT session picks up the deterministic single-thread CPU
    ///    path.
    /// 3. Hand the five SHA-verified byte buffers directly to
    ///    fastembed (no intermediate `std::fs::read` step).
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
    /// - [`Mem0gError::SupplyChainMismatch`] if ANY of the five
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
    /// supply-chain constants; W18c Phase B replaced the fail-closed
    /// `Err(...)` with the real `try_new_from_user_defined` wiring
    /// against the four SHA-verified local files. The Phase B
    /// fix-commit additionally collapsed verify-and-read into a
    /// single TOCTOU-free primitive
    /// ([`crate::supply_chain::ensure_and_read_verified`]) so the
    /// bytes fastembed receives ARE the bytes that produced the
    /// matching SHA — no separate `std::fs::read` window for an
    /// atomic file swap. The bypass code path is now structurally
    /// unreachable: `try_new(Default::default())` is never called
    /// anywhere in the Atlas codebase.
    pub fn new(model_cache_dir: &std::path::Path) -> Mem0gResult<Self> {
        use crate::supply_chain::{
            download_config_with_verification, download_model_with_verification,
            download_special_tokens_with_verification,
            download_tokenizer_config_with_verification, download_tokenizer_with_verification,
            ensure_and_read_verified, pin_omp_threads_single,
        };

        // Step 1: download + SHA-verify-AND-READ all FIVE files
        // (ONNX + four tokenizer files) via the TOCTOU-free
        // ensure_and_read_verified primitive. The returned Vec<u8>
        // bytes ARE the bytes whose SHA was matched against the
        // compiled-in pin — no separate `fs::read` window for an
        // atomic file swap between verify and use.
        let model_path = model_cache_dir.join("bge-small-en-v1.5.onnx");
        let onnx_bytes = ensure_and_read_verified(
            &model_path,
            ONNX_SHA256,
            download_model_with_verification,
        )?;

        let tokenizer_path = model_cache_dir.join("tokenizer.json");
        let tokenizer_bytes = ensure_and_read_verified(
            &tokenizer_path,
            TOKENIZER_JSON_SHA256,
            download_tokenizer_with_verification,
        )?;

        let config_path = model_cache_dir.join("config.json");
        let config_bytes = ensure_and_read_verified(
            &config_path,
            CONFIG_JSON_SHA256,
            download_config_with_verification,
        )?;

        let special_tokens_path = model_cache_dir.join("special_tokens_map.json");
        let special_tokens_bytes = ensure_and_read_verified(
            &special_tokens_path,
            SPECIAL_TOKENS_MAP_SHA256,
            download_special_tokens_with_verification,
        )?;

        let tokenizer_config_path = model_cache_dir.join("tokenizer_config.json");
        let tokenizer_config_bytes = ensure_and_read_verified(
            &tokenizer_config_path,
            TOKENIZER_CONFIG_JSON_SHA256,
            download_tokenizer_config_with_verification,
        )?;

        // Step 2: pin OMP threads BEFORE fastembed init so the ORT
        // session is created with the deterministic single-thread
        // CPU path (ADR §4 sub-decision #2).
        pin_omp_threads_single();

        // Step 3 + 4: construct UserDefinedEmbeddingModel. API
        // surface verified against fastembed-rs 5.13.4 source:
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
        //   - 5 SHA-256 hex digests (64-char lowercase hex)
        //   - 1 SHA-1 hex digest (40-char lowercase hex, Git revision)
        //   - 5 URL strings (must start with https://huggingface.co/
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
}
