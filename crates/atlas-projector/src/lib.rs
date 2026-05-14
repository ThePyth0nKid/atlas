//! # Atlas Projector — V2-α Layer-2 graph projection canonicalisation
//!
//! This crate owns the **deterministic graph-state-hash** primitive that
//! Atlas V2's Three-Layer Trust Architecture (per `docs/V2-MASTER-PLAN.md`
//! §3) depends on: given a logical graph state derived from
//! `events.jsonl` (Layer 1, authoritative), compute a stable
//! byte-for-byte canonical encoding and a `blake3` hash of it. The hash
//! is what gets pinned in the V2-α projector-state-hash CI gate (per
//! `DECISION-ARCH-1` triple-hardening) and what `ProjectorRunAttestation`
//! events (Welle 4 candidate) will cryptographically bind into the
//! trust chain.
//!
//! ## Scope through V2-α Welle 5 (current)
//!
//! **Shipped:**
//!
//! - Welle 3: load-bearing canonicalisation primitive
//!   (`canonical::build_canonical_bytes` + `canonical::graph_state_hash`,
//!   plus byte-determinism CI pin); in-memory `GraphState` /
//!   `GraphNode` / `GraphEdge` types backed by `BTreeMap` keyed on
//!   logical identifier (`entity_uuid` / `edge_id`).
//! - Welle 5: `events.jsonl` line parser (`replay::parse_events_jsonl`),
//!   idempotent event-to-state upsert (`upsert::apply_event_to_state` +
//!   `upsert::project_events`), ProjectorRunAttestation payload
//!   emitter (`emission::build_projector_run_attestation_payload`).
//!   Together: Welle 5 delivers the full pipeline `events.jsonl →
//!   GraphState → signed attestation payload`. Caller (atlas-signer)
//!   wraps the payload in an `AtlasEvent` and signs.
//! - Welle 6: projector-state-hash CI gate
//!   (`gate::verify_attestations_in_trace`) closes the V2-α security
//!   loop. Given an `AtlasTrace` containing `ProjectorRunAttestation`
//!   events, the gate re-projects the trace's other events,
//!   recomputes `graph_state_hash`, and compares against the
//!   attested values. Returns per-attestation `GateResult` with
//!   `Match` / `Mismatch` / `AttestationParseFailed` outcome.
//!   Drift detected cryptographically, not just by CI convention.
//!
//! **Out of scope for the current welle range** (deferred to V2-α
//! Welles 6-8 / V2-β):
//!
//! - ArcadeDB driver integration (Welle 6+ candidate) — replace
//!   in-memory `GraphState` with ArcadeDB-backed implementation;
//!   operator-runbook for deployment; SQL deterministic-dump
//!   adapter.
//! - Parallel-projection design for >10M event scenarios (Welle
//!   7+ candidate).
//! - DB-specific dump-to-canonical adapter for the eventual
//!   ArcadeDB Layer-2 (Welle 6+).
//! - Expanded event-kind support (annotation, policy, anchor) —
//!   V2-β.
//! - Cedar policy at write-time (V2-δ).
//!
//! ## Design invariants
//!
//! 1. **Logical-identifier sort order, NOT insert-order.** Per V2-α
//!    Welle 2 spike (`docs/V2-ALPHA-DB-SPIKE.md` §3.5), insert-order-
//!    dependent identifiers (like ArcadeDB's `@rid`) are NOT a stable
//!    canonical-hash identity anchor. Two projection runs replaying
//!    the same `events.jsonl` into a fresh database in different
//!    historical orders would produce different `@rid` values for
//!    logically identical nodes. The canonical hash MUST sort by a
//!    **stable logical identifier** — `entity_uuid` = `blake3(workspace_id
//!    || event_uuid || kind)` — that is identical across replays.
//!    This crate enforces logical-identifier sort via `BTreeMap`-backed
//!    container choice in `state.rs`.
//!
//! 2. **CBOR canonicalisation per RFC 8949 §4.2.1.** Map entries are
//!    sorted by encoded-key length first, then bytewise lex on the
//!    encoded key (identical pattern to V1's
//!    `atlas_trust_core::cose::build_signing_input`). Property maps
//!    inside nodes/edges follow the same convention.
//!
//! 3. **No floating-point in canonicalised properties.** Floats
//!    serialise non-deterministically across CBOR variants and across
//!    float libraries. Callers MUST use integer representations
//!    (e.g. basis points for fractional currency, microseconds for
//!    sub-second timestamps). Same convention V1 enforces in
//!    `cose::build_signing_input`.
//!
//! 4. **`author_did` schema-additive (Welle 1 invariant).** Every
//!    `GraphNode` and `GraphEdge` MAY carry an optional `author_did`
//!    stamping the agent identity that produced the originating event.
//!    When present, the DID is canonically bound into the
//!    graph-state-hash; when absent (V1-era events), the canonical
//!    bytes omit the field entirely (no `author_did = null`
//!    serialisation). Mirrors the V1 `cose::build_signing_input`
//!    optional-field pattern.
//!
//! 5. **No serde derives on `GraphState`.** Welle 3 intentionally
//!    isolates wire-format serialisation (CBOR canonical encoding for
//!    hashing) from on-disk or over-the-wire serialisation. The
//!    `build_canonical_bytes` function is the single canonical-CBOR
//!    boundary; serde-Serialize of `GraphState` is OUT OF SCOPE.
//!    Matches V1's `AtlasEvent` (serde for wire) vs
//!    `build_signing_input` (canonical-CBOR pure function) split.
//!
//! ## Trust property
//!
//! `graph_state_hash(state)` is a function in the strict mathematical
//! sense: same input bytes → same output bytes, every time, every
//! Rust target, every Atlas build. The byte-determinism CI pin in
//! `canonical::tests::graph_state_hash_byte_determinism_pin` locks
//! this property and fails the build if any future change breaks it.
//! Future Atlas-Projector wellen MUST preserve this property — any
//! change that breaks the pin requires (a) intentional reason
//! documented in the commit, (b) `atlas-projector` crate version bump
//! to cascade through V2 version identity.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod backend;
pub mod canonical;
pub mod emission;
pub mod error;
pub mod gate;
pub mod replay;
pub mod state;
pub mod upsert;

pub use backend::arcadedb::ArcadeDbBackend;
pub use backend::in_memory::InMemoryBackend;
pub use backend::{
    check_value_depth_and_size, check_workspace_id, Edge as BackendEdge, EdgeId, EntityUuid,
    GraphStateBackend, UpsertResult, Vertex as BackendVertex, WorkspaceId, WorkspaceTxn,
};
pub use canonical::{build_canonical_bytes, graph_state_hash};
pub use emission::{
    build_projector_run_attestation_payload, build_projector_run_attestation_payload_from_backend,
};
pub use error::{ProjectorError, ProjectorResult};
pub use gate::{
    verify_attestations_in_trace, verify_attestations_in_trace_with_backend, GateResult,
    GateStatus,
};
pub use replay::parse_events_jsonl;
pub use state::{GraphEdge, GraphNode, GraphState};
pub use upsert::{apply_event_to_state, project_events};

/// V2-α canonical-form schema identifier. Bound into every
/// `graph_state_hash` computation as the first map entry, so any
/// future schema-version-incompatible change to the canonical form
/// is structurally detectable (different `v` → different bytes →
/// different hash → byte-pin fails).
pub const PROJECTOR_SCHEMA_VERSION: &str = "atlas-projector-v1-alpha";

/// V2-α Welle 7: this crate's `CARGO_PKG_VERSION` exposed as a
/// public constant so downstream consumers (atlas-signer, future
/// SDKs) can embed it in `projector_version` payload fields
/// without relying on their own CARGO_PKG_VERSION as a proxy.
/// Resolves Welle-7-review's projector-version honesty gap: the
/// emitted `projector_version` string now structurally reflects
/// the actual atlas-projector logic version, not the consumer's.
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
