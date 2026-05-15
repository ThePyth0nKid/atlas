//! # Atlas Mem0g — V2-β Layer-3 semantic cache crate
//!
//! This crate implements Atlas's Layer-3 semantic cache per
//! `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md`. The cache is the
//! FAST, REBUILDABLE, NEVER-AUTHORITATIVE upper layer of Atlas's V2
//! Three-Layer Trust Architecture (Layer 1 events.jsonl authoritative
//! → Layer 2 ArcadeDB projection queryable → Layer 3 Mem0g semantic
//! cache rebuildable).
//!
//! ## Trust property (load-bearing)
//!
//! Every [`SemanticHit`] returned from [`SemanticCacheBackend::search`]
//! carries `event_uuid` — the Layer-1 trust-substrate cite-back
//! identifier. The caller MUST verify the event against
//! `events.jsonl` via the offline WASM verifier before treating the
//! hit as trustworthy. The cache NEVER provides trust authority; it
//! only accelerates retrieval.
//!
//! ## Where this crate sits in the workspace
//!
//! Apache-2.0 across the stack (license-compatible with all Atlas tiers).
//! Pure-Rust embedded (no JVM / Python sidecar; distribution-friendly).
//! Independent crate boundary (ADR §4 sub-decision #7) so the LanceDB
//! plus fastembed-rs plus Arrow plus DataFusion transitive dep tree (~200
//! crates) stays isolated from `atlas-projector`'s smaller audit surface.
//! SemVer-additive on `atlas-projector` (new `embedding_erased`
//! event-kind dispatch arm added there; this crate is brand-new at `0.1.0`).
//!
//! ## Feature flags
//!
//! `default = []` is empty: the trait surface plus `InvalidationPolicy`
//! plus secure-delete primitive compile without pulling LanceDB. This
//! lets contributors who aren't touching Layer-3 `cargo check` the
//! workspace fast.
//!
//! `lancedb-backend` pulls `lancedb 0.29`, `fastembed =5.13.4`
//! (exact-version pin per ADR §4 sub-decision #2), Arrow, tokio,
//! reqwest. Enables `LanceDbCacheBackend` plus determinism + bench tests.
//!
//! ## Design invariants
//!
//! 1. **Embeddings live OUTSIDE the canonicalisation pipeline.**
//!    `atlas-projector` invariant #3 forbids floats in canonical bytes.
//!    LanceDB stores embeddings (f32 vectors) in its Arrow fragments;
//!    only the `embedding_erased` audit-event payload (strings +
//!    timestamps) enters the canonical-CBOR pipeline.
//! 2. **Cite-back is mandatory.** Every [`SemanticHit`] carries
//!    `event_uuid`. Backends that cannot honour this contract are not
//!    valid `SemanticCacheBackend` impls.
//! 3. **Layer authority.** Mem0g indexes Layer 1 events.jsonl directly.
//!    Cache rebuild does NOT depend on Layer-2 ArcadeDB availability
//!    (ADR §4 sub-decision #3).
//! 4. **Secure-delete fail-closed.** GDPR Art. 17 erasure goes through
//!    the [`secure_delete`] 7-step protocol that overwrites the
//!    on-disk bytes of both Arrow fragments AND HNSW `_indices/`
//!    files BEFORE unlinking (ADR §4 sub-decision #4).
//! 5. **Append-only erasure record.** The Layer-1 `embedding_erased`
//!    event is itself NEVER subject to secure-delete; it stays in
//!    `events.jsonl` as cryptographic evidence (ADR §4 sub-decision #5).

#![deny(unsafe_code)]
#![warn(missing_docs)]

use std::time::Duration;

use serde::{Deserialize, Serialize};

pub mod embedder;
pub mod secure_delete;
pub mod supply_chain;

#[cfg(feature = "lancedb-backend")]
pub mod lancedb_backend;

#[cfg(feature = "lancedb-backend")]
pub use lancedb_backend::LanceDbCacheBackend;

/// V2-β Layer-3 schema version. Embedded into bench / determinism
/// fixtures so a future schema-incompatible change is detectable as a
/// fixture drift rather than a silent semantic change.
pub const MEM0G_SCHEMA_VERSION: &str = "atlas-mem0g-v1-alpha";

/// Workspace identifier — opaque caller-domain string. Mirrors the
/// `atlas-projector::WorkspaceId` type alias convention; not a newtype
/// so existing call-sites stay stable across the V2-β / V2-γ boundary.
pub type WorkspaceId = String;

/// Layer-1 event-UUID — the trust-substrate cite-back identifier.
/// Kept as a transparent `String` alias for symmetry with
/// `atlas_trust_core::trace_format::AtlasEvent::event_id`.
pub type EventUuid = String;

/// Layer-2 entity-UUID — opaque projection vertex reference, optional
/// on every hit (a Layer-3 result may correspond to a Layer-1 event
/// that has no Layer-2 vertex, e.g. anchor events).
pub type EntityUuid = String;

/// A single semantic-search hit.
///
/// **Trust contract:** `event_uuid` MUST be populated. Backends that
/// return hits without a Layer-1 anchor violate Atlas's cite-back
/// invariant and are rejected at integration-test boundary.
///
/// Marked `#[non_exhaustive]` so adding new diagnostic fields in V2-γ
/// is SemVer-additive for downstream consumers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct SemanticHit {
    /// Layer-1 anchor — the trust-substrate cite-back identifier.
    /// **ALWAYS present** in every hit. The caller verifies this
    /// event_id against `events.jsonl` via the offline WASM verifier
    /// independently of the cache.
    pub event_uuid: EventUuid,

    /// Workspace the hit belongs to. Echoed back so callers can scope
    /// downstream queries.
    pub workspace_id: WorkspaceId,

    /// Optional Layer-2 vertex reference (if the hit corresponds to a
    /// projected entity). `None` for events that did not project a
    /// vertex (e.g. `anchor_created`).
    pub entity_uuid: Option<EntityUuid>,

    /// Cosine similarity score in `[0.0, 1.0]`. **Diagnostic only** —
    /// callers MUST NOT use this as a trust signal. The score is
    /// f32 because LanceDB returns f32 distances; it does NOT enter
    /// the canonical-CBOR pipeline.
    pub score: f32,

    /// Cached snippet of the original event payload. Subject to GDPR
    /// erasure — an `embedding_erased` event removes both the
    /// embedding AND this snippet (co-located in the same LanceDB
    /// Arrow fragment per ADR §4 sub-decision #4 step 6).
    pub snippet: String,
}

impl SemanticHit {
    /// Construct a new [`SemanticHit`].
    ///
    /// Required for external-crate construction because the struct is
    /// `#[non_exhaustive]`.
    #[must_use]
    pub fn new(
        event_uuid: EventUuid,
        workspace_id: WorkspaceId,
        entity_uuid: Option<EntityUuid>,
        score: f32,
        snippet: String,
    ) -> Self {
        Self {
            event_uuid,
            workspace_id,
            entity_uuid,
            score,
            snippet,
        }
    }
}

/// Layer-3 cache-invalidation policy (ADR §4 sub-decision #6).
///
/// Hybrid Layer-1-native triggers:
///
/// 1. **Background TTL.** Default 5 min; configurable per deployment.
/// 2. **Explicit invalidation on `embedding_erased`.** Immediate; not
///    within-TTL.
/// 3. **Layer-1 head-divergence detection.** Cache rebuilds if cache's
///    last-known events.jsonl head_event_uuid OR byte_length differs
///    from current Layer-1 state. Works even if Layer-2 ArcadeDB is
///    unavailable.
///
/// **Optional diagnostic fourth trigger** (NOT load-bearing): Layer-2
/// `graph_state_hash` cross-check. If Layer 2 IS available AND its
/// hash mismatches Layer 3's last-known projection, log a warning +
/// force a Layer-1-driven rebuild. Layer 3 NEVER trusts Layer 2 for
/// cache-validity decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InvalidationPolicy {
    /// Background TTL between automatic head-divergence checks.
    /// Default: 5 minutes per ADR §4 sub-decision #6.
    pub ttl: Duration,

    /// Whether to honour `embedding_erased` audit-events as immediate
    /// invalidation triggers. Default: `true`. Setting `false` is a
    /// supported-but-strongly-discouraged operator override for
    /// non-production diagnostic deployments.
    pub honour_explicit_erasure: bool,

    /// Whether to check Layer-1 head-event-uuid + byte-length for
    /// divergence detection. Default: `true`. This is the
    /// Layer-1-only path that works without Layer 2.
    pub honour_head_divergence: bool,

    /// Whether to opportunistically cross-check Layer-2
    /// `graph_state_hash` when Layer 2 IS available. Default: `false`
    /// — defence-in-depth diagnostic, NOT load-bearing. Setting
    /// `true` enables a warning-log + force-rebuild path on mismatch.
    pub honour_layer2_diagnostic: bool,
}

impl Default for InvalidationPolicy {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(5 * 60),
            honour_explicit_erasure: true,
            honour_head_divergence: true,
            honour_layer2_diagnostic: false,
        }
    }
}

impl InvalidationPolicy {
    /// Construct a new [`InvalidationPolicy`] with explicit settings.
    /// Required for external-crate construction because of
    /// `#[non_exhaustive]`.
    #[must_use]
    pub fn new(
        ttl: Duration,
        honour_explicit_erasure: bool,
        honour_head_divergence: bool,
        honour_layer2_diagnostic: bool,
    ) -> Self {
        Self {
            ttl,
            honour_explicit_erasure,
            honour_head_divergence,
            honour_layer2_diagnostic,
        }
    }
}

/// Errors surfaced by the Mem0g cache layer.
///
/// Marked `#[non_exhaustive]` so adding new failure modes in V2-γ
/// (e.g. quota exceeded, region-residency violation) is SemVer-
/// additive for downstream `match` arms.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Mem0gError {
    /// Underlying storage backend (LanceDB) returned an error.
    #[error("backend error: {0}")]
    Backend(String),

    /// Embedder pipeline (fastembed-rs or download verification) failed.
    #[error("embedder error: {0}")]
    Embedder(String),

    /// Supply-chain verification mismatch: model file SHA256 does NOT
    /// match the compiled-in `ONNX_SHA256` constant. Fails closed —
    /// the cache REFUSES to embed. Fixed by re-cache on next cold
    /// start OR by Atlas operator release with updated constants.
    #[error("supply-chain mismatch: expected SHA256 {expected}, got {actual}")]
    SupplyChainMismatch {
        /// Compiled-in expected SHA256 (hex)
        expected: String,
        /// Actually observed SHA256 (hex)
        actual: String,
    },

    /// Secure-delete protocol failed in one of the seven steps. The
    /// `step` field surfaces WHICH step (ACQUIRE / PRE-CAPTURE /
    /// DELETE / CLEANUP / OVERWRITE / RELEASE / EMIT) so operators
    /// can disambiguate (e.g. a step-6 OS error vs a step-1 lock
    /// contention bug).
    #[error("secure-delete failed at step {step}: {reason}")]
    SecureDelete {
        /// Which of the 7 steps failed
        step: &'static str,
        /// Human-readable reason
        reason: String,
    },

    /// The caller passed an invalid workspace_id (empty, non-ASCII,
    /// or otherwise structurally dangerous). Mirrors
    /// `atlas_projector::ProjectorError::InvalidWorkspaceId`.
    #[error("invalid workspace_id: {reason}")]
    InvalidWorkspaceId {
        /// Human-readable reason
        reason: String,
    },

    /// I/O failure during secure-delete overwrite or model download.
    #[error("io error: {0}")]
    Io(String),
}

/// Result alias for Mem0g operations.
pub type Mem0gResult<T> = Result<T, Mem0gError>;

/// Layer-3 semantic cache backend abstraction.
///
/// **Object-safe.** Consumers hold a `Box<dyn SemanticCacheBackend>`.
/// V2-β W18b ships [`LanceDbCacheBackend`] (default behind
/// `lancedb-backend` feature). V2-γ pivot candidate:
/// `QdrantCacheBackend` (separate crate, ADR §3.2 / §7 Pivot Path).
///
/// **Send + Sync.** Required so tokio multi-task semantic-search can
/// hold the backend behind an `Arc<dyn SemanticCacheBackend>`.
///
/// **Sync trait methods.** Mirrors the Layer-2 `GraphStateBackend`
/// convention (`reqwest::blocking`). Async LanceDB calls are wrapped
/// via `tokio::task::spawn_blocking` — NOT `Handle::current().block_on()`
/// (deadlocks under single-threaded scheduler per spike §7).
///
/// **Trust contract on `search`:** every returned [`SemanticHit`]
/// MUST carry `event_uuid`. Backends that cannot honour this contract
/// violate Atlas's cite-back invariant.
pub trait SemanticCacheBackend: Send + Sync {
    /// Upsert an embedding for a Layer-1 event. Idempotent on
    /// `event_uuid`. The embedder is owned by the backend (caller
    /// passes raw text, not pre-computed vectors) — this keeps the
    /// embedder-version pin under the backend's control.
    fn upsert(
        &self,
        workspace_id: &WorkspaceId,
        event_uuid: &EventUuid,
        text: &str,
    ) -> Mem0gResult<()>;

    /// Top-k semantic search. Returns hits sorted by descending score
    /// (cosine similarity in `[0.0, 1.0]`). Each hit MUST carry
    /// `event_uuid` for cite-back trust.
    fn search(
        &self,
        workspace_id: &WorkspaceId,
        query: &str,
        k: usize,
    ) -> Mem0gResult<Vec<SemanticHit>>;

    /// Erase the embedding + snippet for a Layer-1 event via the
    /// secure-delete protocol (ADR §4 sub-decision #4). MUST
    /// overwrite both Arrow fragments AND HNSW `_indices/` files
    /// covering the deleted row BEFORE unlinking.
    ///
    /// Caller is responsible for emitting the parallel
    /// `embedding_erased` Layer-1 audit-event AFTER this call
    /// returns. The audit-event emission is deliberately not inside
    /// the lock held by the backend (deadlock risk on the projector's
    /// own write-side mutex — see ADR step 8).
    fn erase(
        &self,
        workspace_id: &WorkspaceId,
        event_uuid: &EventUuid,
    ) -> Mem0gResult<()>;

    /// Rebuild the cache from a Layer-1 events iterator. Used for
    /// the periodic rebuild trigger and head-divergence recovery.
    /// Iterator-based so 10M-event workspaces don't load everything
    /// into memory.
    fn rebuild(
        &self,
        workspace_id: &WorkspaceId,
        events: Box<dyn Iterator<Item = atlas_trust_core::trace_format::AtlasEvent> + Send + '_>,
    ) -> Mem0gResult<()>;

    /// Backend identity string for `ProjectorRunAttestation` chain
    /// (analog `GraphStateBackend::backend_id`).
    /// Stable across crate versions — changing a returned string is a
    /// SemVer-breaking change for downstream consumers.
    /// Values: `"lancedb-fastembed"` (W18b default) or
    /// `"qdrant-fastembed"` (V2-γ pivot candidate).
    fn backend_id(&self) -> &'static str;
}

/// Validate a [`WorkspaceId`] for safe propagation to backend
/// boundaries (LanceDB filesystem-path + Cypher-parameter binding
/// + log redaction).
///
/// Mirrors `atlas_projector::check_workspace_id` semantics for
/// cross-layer consistency.
///
/// # Rules
///
/// - non-empty
/// - length ≤ 128 bytes
/// - ASCII-only
/// - no `/`, `\`, NUL, `\r`, `\n` (path / log-injection defence)
///
/// # Errors
///
/// Returns [`Mem0gError::InvalidWorkspaceId`] with a human-readable
/// reason on the first rule violation.
pub fn check_workspace_id(s: &str) -> Mem0gResult<()> {
    if s.is_empty() {
        return Err(Mem0gError::InvalidWorkspaceId {
            reason: "empty".to_string(),
        });
    }
    if s.len() > 128 {
        return Err(Mem0gError::InvalidWorkspaceId {
            reason: format!("length {} exceeds 128", s.len()),
        });
    }
    if !s.is_ascii() {
        return Err(Mem0gError::InvalidWorkspaceId {
            reason: "must be ASCII".to_string(),
        });
    }
    for ch in s.chars() {
        if ch == '/' || ch == '\\' || ch == '\0' || ch == '\r' || ch == '\n' {
            return Err(Mem0gError::InvalidWorkspaceId {
                reason: format!("contains forbidden character {ch:?}"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidation_policy_default_5min_ttl() {
        let p = InvalidationPolicy::default();
        assert_eq!(p.ttl, Duration::from_secs(300));
        assert!(p.honour_explicit_erasure);
        assert!(p.honour_head_divergence);
        assert!(!p.honour_layer2_diagnostic, "Layer-2 cross-check defaults to OFF (diagnostic-only)");
    }

    #[test]
    fn semantic_hit_always_carries_event_uuid() {
        // Compile-time + structural check: SemanticHit::new requires
        // event_uuid (not Option). The cite-back trust property is
        // structural, not optional.
        let hit = SemanticHit::new(
            "01HEVENT".to_string(),
            "ws-test".to_string(),
            None,
            0.5,
            "snippet".to_string(),
        );
        assert!(!hit.event_uuid.is_empty());
    }

    #[test]
    fn check_workspace_id_accepts_valid() {
        check_workspace_id("ws-test").unwrap();
        check_workspace_id("01HEVENT").unwrap();
        check_workspace_id("a").unwrap();
    }

    #[test]
    fn check_workspace_id_rejects_empty() {
        assert!(matches!(
            check_workspace_id(""),
            Err(Mem0gError::InvalidWorkspaceId { .. })
        ));
    }

    #[test]
    fn check_workspace_id_rejects_path_traversal() {
        assert!(matches!(
            check_workspace_id("../etc/passwd"),
            Err(Mem0gError::InvalidWorkspaceId { .. })
        ));
        assert!(matches!(
            check_workspace_id("a\\b"),
            Err(Mem0gError::InvalidWorkspaceId { .. })
        ));
    }

    #[test]
    fn check_workspace_id_rejects_log_injection() {
        assert!(matches!(
            check_workspace_id("legit\nFAKE_LOG"),
            Err(Mem0gError::InvalidWorkspaceId { .. })
        ));
        assert!(matches!(
            check_workspace_id("legit\rFAKE"),
            Err(Mem0gError::InvalidWorkspaceId { .. })
        ));
    }

    #[test]
    fn check_workspace_id_rejects_non_ascii() {
        assert!(matches!(
            check_workspace_id("café"),
            Err(Mem0gError::InvalidWorkspaceId { .. })
        ));
    }

    #[test]
    fn check_workspace_id_rejects_overlong() {
        let long = "a".repeat(129);
        assert!(matches!(
            check_workspace_id(&long),
            Err(Mem0gError::InvalidWorkspaceId { .. })
        ));
    }
}
