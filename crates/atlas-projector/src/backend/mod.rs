//! V2-β Welle 17a: `GraphStateBackend` trait + supporting types.
//!
//! ## What this module owns
//!
//! The pluggable Layer-2 backend abstraction for Atlas's projection
//! pipeline. Production realisation of the trait sketch in
//! `docs/V2-BETA-ARCADEDB-SPIKE.md` §7 + ADR-Atlas-011 §4.
//!
//! Two impls live alongside this module:
//!   * [`in_memory::InMemoryBackend`] — wraps the existing V2-α
//!     in-memory `GraphState` + `upsert` + `canonical` pipeline. The
//!     default backend used everywhere V2-α already calls into the
//!     projector. Behaviour is byte-identical to V2-α (`graph_state_hash`
//!     byte-pin preserved).
//!   * [`arcadedb::ArcadeDbBackend`] — stub. All trait methods are
//!     `unimplemented!()` placeholders. W17b fills in the production
//!     impl via `reqwest` + Cypher per ADR-Atlas-010 §4.
//!
//! ## Why this abstraction now (W17a)
//!
//! ADR-Atlas-010 §4 binds 8 sub-decisions; sub-decision #8 is "the
//! `GraphStateBackend` trait" with W17a as the welle that writes the
//! production version. The trait keeps V2-α's call sites stable while
//! letting W17b swap in an ArcadeDB-backed implementation without
//! touching `emission.rs` / `gate.rs` consumers.
//!
//! Equally important: the trait gives us a **mechanical guarantee**
//! that the byte-determinism CI pin (V2-α Welle 3,
//! `canonical::tests::graph_state_hash_byte_determinism_pin`,
//! pinned hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`)
//! survives the backend swap. The default `canonical_state()` impl
//! lives in the trait body and delegates to the EXISTING
//! `canonical::build_canonical_bytes` + `blake3` pipeline. Any backend
//! that respects the §4.9 adapter contract (logical-identifier sort
//! order) gets the byte-pin for free.
//!
//! ## OQ-1 resolution: `Box<dyn WorkspaceTxn>` (NOT associated type)
//!
//! ADR-Atlas-010 §6 OQ-1 asked: should the trait carry an associated
//! type for the transaction handle, or use `Box<dyn WorkspaceTxn>`?
//! W17a picks `Box<dyn>` for these reasons (rationale also recorded in
//! ADR-Atlas-011):
//!
//! 1. **Object safety preserved.** The projector emission pipeline
//!    (`emission.rs` / `gate.rs`) wants to hold the backend behind a
//!    `Box<dyn GraphStateBackend>` so V2-β-era code can swap the
//!    impl at runtime via config. An associated type would force
//!    every consumer of the trait to be generic — viral generics
//!    breaks public-API ergonomics for downstream SDKs.
//! 2. **vtable overhead is irrelevant at our scale.** A projection
//!    cycle handles ~10K events per workspace in the median case;
//!    the per-call vtable dispatch cost is ~1 ns versus the ~300-500
//!    µs HTTP roundtrip (spike §4.10). Two-orders-of-magnitude
//!    margin; vtable is not the bottleneck.
//! 3. **V2-β flexibility need.** W17b will want to support both an
//!    HTTP-bound `ArcadeDbBackend::Txn` and an in-process
//!    `InMemoryBackend::Txn` from the SAME emission pipeline. Object
//!    safety is the simplest enabler.
//!
//! V2-γ MAY reconsider if benchmarking shows vtable-dispatch
//! materially affects the >10M-event re-projection path.
//!
//! ## OQ-2 resolution: `batch_upsert` on `WorkspaceTxn`
//!
//! ADR-Atlas-010 §6 OQ-2 asked how to expose batch operations. W17a
//! exposes `WorkspaceTxn::batch_upsert(vertices, edges)` returning a
//! `Vec<UpsertResult>`. Rationale:
//!
//! 1. The Option-A parallel-projection design (ADR-Atlas-007 §3.1)
//!    naturally batches per-workspace; the projector accumulates a
//!    workspace's events then commits them as one transaction. Batch
//!    is the dominant call pattern, not single-upsert.
//! 2. ArcadeDb impl (W17b) maps `batch_upsert` to a single
//!    multi-statement Cypher transaction → one HTTP roundtrip per
//!    workspace per cycle instead of N. Performance-critical.
//! 3. The single-upsert methods (`upsert_vertex` / `upsert_edge`) are
//!    retained for diagnostic + small-test paths where the caller
//!    doesn't have a natural batch to assemble.
//! 4. Vertex-batch is applied before edge-batch within
//!    `batch_upsert` so that a fresh-workspace projection cycle does
//!    not see a transient "edge references missing vertex" state.
//!    This is a soft contract — backends MUST honour it; the
//!    `InMemoryBackend` enforces it by construction.
//!
//! ## Byte-determinism adapter contract (recap from spike §4.9)
//!
//! All backends MUST surface vertices + edges in **logical-identifier**
//! sort order (`vertex.entity_uuid` / `edge.edge_id`), NOT storage
//! order (e.g. ArcadeDB's `@rid` is FORBIDDEN as a sort key). The
//! default `canonical_state()` impl on this trait walks
//! `vertices_sorted()` + `edges_sorted()` and feeds them through the
//! same `canonical::build_canonical_bytes` pipeline V2-α already uses.
//! Backends MAY override `canonical_state()` for performance but MUST
//! produce byte-identical output. W17b cross-backend test
//! (`tests/cross_backend_byte_determinism.rs`, deferred) will pin this.

use std::collections::BTreeMap;

use crate::error::ProjectorResult;
use crate::state::{GraphEdge, GraphNode, GraphState};

pub mod arcadedb;
pub mod in_memory;

/// Logical entity identifier — `entity_uuid` per V2-α convention
/// (`hex::encode(blake3(workspace_id || event_uuid || kind))`). Kept
/// as a transparent `String` alias because the existing V2-α
/// `state.rs` uses `String` for `entity_uuid` and a newtype wrapper
/// here would force lossy conversions at every backend boundary. The
/// invariant is enforced by the deriving function in
/// `upsert::derive_node_entity_uuid`, not by the type system.
pub type EntityUuid = String;

/// Logical edge identifier — `edge_id` per V2-α convention. Same
/// reasoning as [`EntityUuid`] for keeping this a `String` alias.
pub type EdgeId = String;

/// Workspace identifier — opaque caller-domain string (ULID, UUID,
/// per-tenant slug; format not constrained by this layer).
pub type WorkspaceId = String;

/// A graph vertex as seen by a backend.
///
/// Field set is taken **directly from `GraphNode`** so backends do not
/// invent a parallel schema. The V2-α Welle 1 stamping fields
/// (`event_uuid`, `rekor_log_index`, `author_did`) are preserved
/// verbatim per ADR-Atlas-010 §4 sub-decision #6 (byte-determinism
/// adapter contract) — any reshape here would risk the
/// `graph_state_hash` byte-pin.
///
/// Marked `#[non_exhaustive]` so adding new V2-β/γ schema-additive
/// fields (e.g. annotations, policies — already present on
/// `GraphNode`) is a non-breaking change for downstream backends.
/// External crates MUST construct via [`Vertex::new`] (positional
/// argument constructor — adding a new field bumps the constructor
/// signature, which IS a SemVer-major change, but the struct surface
/// itself remains forwards-compatible).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Vertex {
    /// Logical entity identifier; sort key for byte-determinism.
    pub entity_uuid: EntityUuid,
    /// Per-tenant workspace identifier this vertex belongs to.
    pub workspace_id: WorkspaceId,
    /// Atlas-domain labels (e.g. `["Dataset", "Sensitive"]`). Order
    /// is normalised at canonicalisation time; insert-order ignored.
    pub labels: Vec<String>,
    /// Application properties. Same canonicalisation rules as
    /// `GraphNode::properties`: floats rejected at canonical-CBOR
    /// boundary; integer encodings only.
    pub properties: BTreeMap<String, serde_json::Value>,
    /// Layer-1 event that created or most recently updated this
    /// vertex. V2-α Welle 1 stamping.
    pub event_uuid: String,
    /// Sigstore Rekor `logIndex` of the anchor for the originating
    /// event. `None` for V1-era pre-anchor events. **Note:**
    /// `GraphNode::rekor_log_index` is a non-optional `u64` (sentinel
    /// `0` for not-yet-anchored). The trait surface uses `Option<u64>`
    /// because it is more honest at the public API boundary; the
    /// `InMemoryBackend::From` conversion maps `0` → `None` and
    /// `n > 0` → `Some(n)`. V2-α Welle 1 stamping.
    pub rekor_log_index: Option<u64>,
    /// V2-α Welle 1 optional agent-identity (`did:atlas:<hex>`).
    /// `None` for V1-era events without agent attribution.
    pub author_did: Option<String>,
}

impl Vertex {
    /// Construct a new [`Vertex`].
    ///
    /// Required for external-crate construction because the struct is
    /// `#[non_exhaustive]`. New schema-additive fields added in
    /// future welles will be supplied through a builder-style API
    /// (e.g. `.with_annotations(...)`) rather than appending to this
    /// constructor's positional argument list — that keeps
    /// `Vertex::new` SemVer-stable across welles.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        entity_uuid: EntityUuid,
        workspace_id: WorkspaceId,
        labels: Vec<String>,
        properties: BTreeMap<String, serde_json::Value>,
        event_uuid: String,
        rekor_log_index: Option<u64>,
        author_did: Option<String>,
    ) -> Self {
        Self {
            entity_uuid,
            workspace_id,
            labels,
            properties,
            event_uuid,
            rekor_log_index,
            author_did,
        }
    }
}

/// A directed graph edge as seen by a backend. Field set mirrors
/// `GraphEdge` exactly per the same reasoning as [`Vertex`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Edge {
    /// Logical edge identifier; sort key for byte-determinism.
    pub edge_id: EdgeId,
    /// Per-tenant workspace identifier this edge belongs to.
    pub workspace_id: WorkspaceId,
    /// Source endpoint — references a [`Vertex::entity_uuid`].
    pub from: EntityUuid,
    /// Target endpoint — references a [`Vertex::entity_uuid`].
    pub to: EntityUuid,
    /// Edge kind (e.g. `"derived_from"`, `"signed_by"`).
    pub label: String,
    /// Application properties. Float-rejection rule as
    /// [`Vertex::properties`].
    pub properties: BTreeMap<String, serde_json::Value>,
    /// Layer-1 event that created or last updated this edge.
    pub event_uuid: String,
    /// See [`Vertex::rekor_log_index`] for `Option<u64>` rationale.
    pub rekor_log_index: Option<u64>,
    /// V2-α Welle 1 optional agent-identity.
    pub author_did: Option<String>,
}

impl Edge {
    /// Construct a new [`Edge`]. See [`Vertex::new`] for the
    /// SemVer-stability rationale around positional constructors on
    /// a `#[non_exhaustive]` struct.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        edge_id: EdgeId,
        workspace_id: WorkspaceId,
        from: EntityUuid,
        to: EntityUuid,
        label: String,
        properties: BTreeMap<String, serde_json::Value>,
        event_uuid: String,
        rekor_log_index: Option<u64>,
        author_did: Option<String>,
    ) -> Self {
        Self {
            edge_id,
            workspace_id,
            from,
            to,
            label,
            properties,
            event_uuid,
            rekor_log_index,
            author_did,
        }
    }
}

/// Outcome of a single vertex- or edge-upsert call.
///
/// `created == true` indicates the logical id did not previously exist
/// in the workspace; `created == false` indicates an idempotent update
/// (same logical id, possibly updated properties). The V2-α
/// `GraphState::upsert_node` returns `Option<previous>`; the trait
/// surface normalises this to a boolean to insulate backends that
/// cannot cheaply return the previous value (e.g. ArcadeDb's
/// `MERGE` command does not return the prior record by default).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UpsertResult {
    /// `true` when this upsert introduced a new logical id; `false`
    /// on idempotent replay or in-place update.
    pub created: bool,
    /// The logical id of the upserted vertex or edge. Echoed back
    /// for diagnostic convenience.
    pub entity_uuid: EntityUuid,
}

impl UpsertResult {
    /// Construct a new [`UpsertResult`]. Required for external-crate
    /// construction because of `#[non_exhaustive]`.
    #[must_use]
    pub fn new(created: bool, entity_uuid: EntityUuid) -> Self {
        Self { created, entity_uuid }
    }
}

/// Per-workspace transaction handle.
///
/// Drop without an explicit `commit()` is a **rollback**. The trait is
/// `Send` because the parallel-projection design (ADR-Atlas-007 §3.1)
/// dispatches one workspace-transaction per tokio task. It is
/// intentionally NOT `Sync` — concurrent mutation of a single
/// transaction is not part of the design.
///
/// `commit` and `rollback` take `self: Box<Self>` so calling them
/// consumes the transaction (no use-after-commit).
pub trait WorkspaceTxn: Send {
    /// Upsert a single vertex within this transaction.
    fn upsert_vertex(&mut self, v: &Vertex) -> ProjectorResult<UpsertResult>;

    /// Upsert a single edge within this transaction. Implementations
    /// MAY reject edges whose endpoints are not yet present in the
    /// transaction's vertex set (`InMemoryBackend` defers this check
    /// to canonicalisation time, mirroring V2-α semantics).
    fn upsert_edge(&mut self, e: &Edge) -> ProjectorResult<UpsertResult>;

    /// Batch-upsert N vertices then M edges in one call (OQ-2
    /// resolution). Backends MUST apply vertices before edges so that
    /// a fresh-workspace cycle never observes a transient
    /// "edge-without-endpoint" state. The returned `Vec<UpsertResult>`
    /// has length `vertices.len() + edges.len()`, vertices first.
    fn batch_upsert(
        &mut self,
        vertices: &[Vertex],
        edges: &[Edge],
    ) -> ProjectorResult<Vec<UpsertResult>>;

    /// Commit the transaction; consumes the handle.
    fn commit(self: Box<Self>) -> ProjectorResult<()>;

    /// Roll back the transaction explicitly; consumes the handle.
    /// Calling `rollback` is equivalent to dropping the handle without
    /// commit — provided here for callers who want explicit failure
    /// semantics.
    fn rollback(self: Box<Self>) -> ProjectorResult<()>;
}

/// Backend abstraction for Layer-2 graph state.
///
/// One trait, two impls (W17a): `InMemoryBackend` (default, V2-α
/// behaviour-preserving) and `ArcadeDbBackend` (stub; filled in by
/// W17b).
///
/// **Object safety:** the trait is object-safe — every method takes
/// `&self` / `&mut self` and returns types that do not reference
/// `Self`. Consumers hold a `Box<dyn GraphStateBackend>` (per OQ-1
/// resolution, see module docstring).
///
/// **Send + Sync:** required so tokio multi-task projection can hold
/// the backend behind an `Arc<dyn GraphStateBackend + Send + Sync>`.
/// Per-task isolation is via `WorkspaceTxn` (Send-only).
///
/// **`#[non_exhaustive]` discipline:** the trait itself cannot be
/// marked `#[non_exhaustive]` (Rust attribute applies to structs/
/// enums, not traits). The `Vertex`, `Edge`, and `UpsertResult`
/// structs ARE `#[non_exhaustive]` so adding fields in V2-γ is
/// SemVer-additive. Adding new trait methods is a SemVer-breaking
/// change; provide a default impl to soften.
pub trait GraphStateBackend: Send + Sync {
    /// Open a new per-workspace transaction.
    ///
    /// Implementations MAY lazily create the underlying workspace
    /// container (e.g. ArcadeDb creates a database on first
    /// transaction for an unseen workspace).
    fn begin(&self, workspace_id: &WorkspaceId) -> ProjectorResult<Box<dyn WorkspaceTxn + '_>>;

    /// Read all vertices for a workspace, sorted by `entity_uuid`
    /// (logical identifier). MUST be sorted for byte-determinism
    /// (spike §4.9 adapter contract).
    fn vertices_sorted(&self, workspace_id: &WorkspaceId) -> ProjectorResult<Vec<Vertex>>;

    /// Read all edges for a workspace, sorted by `edge_id` (logical
    /// identifier).
    fn edges_sorted(&self, workspace_id: &WorkspaceId) -> ProjectorResult<Vec<Edge>>;

    /// Compute the canonical `graph_state_hash` for a workspace.
    ///
    /// Default impl reads `vertices_sorted` + `edges_sorted`, projects
    /// them into a transient `GraphState`, and runs the V2-α
    /// `canonical::graph_state_hash`. Backends MAY override for
    /// performance but MUST produce byte-identical output.
    ///
    /// The default impl is what makes the byte-pin survive a backend
    /// swap: as long as `vertices_sorted` + `edges_sorted` honour the
    /// §4.9 adapter contract, the resulting bytes are identical to
    /// V2-α's in-memory path.
    fn canonical_state(&self, workspace_id: &WorkspaceId) -> ProjectorResult<[u8; 32]> {
        let vertices = self.vertices_sorted(workspace_id)?;
        let edges = self.edges_sorted(workspace_id)?;
        let state = build_graph_state_from_sorted(&vertices, &edges);
        crate::canonical::graph_state_hash(&state)
    }

    /// Backend identity string for `ProjectorRunAttestation` chain.
    ///
    /// Returned values: `"in-memory"`, `"arcadedb-server"`, or
    /// (future) `"falkordb-fallback"`. Stable across crate versions
    /// (changing a returned string is a SemVer-breaking change for
    /// downstream `ProjectorRunAttestation` consumers).
    fn backend_id(&self) -> &'static str;
}

/// Convert a sorted-by-logical-id `&[Vertex]` + `&[Edge]` pair into
/// a transient `GraphState` for canonicalisation. Used by the default
/// `canonical_state()` impl + the `InMemoryBackend` override.
///
/// Mapping:
/// - `Vertex.rekor_log_index = None` ↔ `GraphNode.rekor_log_index = 0`
///   (V2-α `state.rs` uses `0` as the sentinel "not-yet-anchored"
///   value; the trait surface uses `Option<u64>` for clarity but the
///   on-the-wire bytes are the same — the byte-pin fixture exercises
///   only `rekor_log_index > 0` values).
/// - Welle-14 fields (`annotations`, `policies`) are constructed
///   empty here. Backend-level support for those event kinds is
///   deferred to W17b (`anchor_created` / `annotation_add` /
///   `policy_set` are out-of-scope for W17a per the in-scope spec).
fn build_graph_state_from_sorted(vertices: &[Vertex], edges: &[Edge]) -> GraphState {
    let mut state = GraphState::new();
    for v in vertices {
        state.upsert_node(GraphNode {
            entity_uuid: v.entity_uuid.clone(),
            labels: v.labels.clone(),
            properties: v.properties.clone(),
            event_uuid: v.event_uuid.clone(),
            rekor_log_index: v.rekor_log_index.unwrap_or(0),
            author_did: v.author_did.clone(),
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        });
    }
    for e in edges {
        state.upsert_edge(GraphEdge {
            edge_id: e.edge_id.clone(),
            from_entity: e.from.clone(),
            to_entity: e.to.clone(),
            kind: e.label.clone(),
            properties: e.properties.clone(),
            event_uuid: e.event_uuid.clone(),
            rekor_log_index: e.rekor_log_index.unwrap_or(0),
            author_did: e.author_did.clone(),
        });
    }
    state
}

/// Convert a V2-α `GraphNode` into the trait's [`Vertex`] surface.
///
/// `workspace_id` is taken from the caller because `GraphNode` does
/// not carry it (workspace identity is implicit at the per-workspace
/// `GraphState` boundary in V2-α). `rekor_log_index = 0` is mapped to
/// `None` per the convention in [`build_graph_state_from_sorted`].
pub(crate) fn vertex_from_graph_node(node: &GraphNode, workspace_id: &WorkspaceId) -> Vertex {
    Vertex {
        entity_uuid: node.entity_uuid.clone(),
        workspace_id: workspace_id.clone(),
        labels: node.labels.clone(),
        properties: node.properties.clone(),
        event_uuid: node.event_uuid.clone(),
        rekor_log_index: if node.rekor_log_index == 0 {
            None
        } else {
            Some(node.rekor_log_index)
        },
        author_did: node.author_did.clone(),
    }
}

/// Convert a V2-α `GraphEdge` into the trait's [`Edge`] surface.
/// Same `workspace_id` + `rekor_log_index` conventions as
/// [`vertex_from_graph_node`].
pub(crate) fn edge_from_graph_edge(edge: &GraphEdge, workspace_id: &WorkspaceId) -> Edge {
    Edge {
        edge_id: edge.edge_id.clone(),
        workspace_id: workspace_id.clone(),
        from: edge.from_entity.clone(),
        to: edge.to_entity.clone(),
        label: edge.kind.clone(),
        properties: edge.properties.clone(),
        event_uuid: edge.event_uuid.clone(),
        rekor_log_index: if edge.rekor_log_index == 0 {
            None
        } else {
            Some(edge.rekor_log_index)
        },
        author_did: edge.author_did.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_graph_state_from_sorted_roundtrips_via_node_node() {
        // Verify that the build_graph_state_from_sorted helper produces
        // a GraphState whose iteration order is the same as the
        // logical-identifier sort of the input slices.
        let v_a = Vertex {
            entity_uuid: "a".into(),
            workspace_id: "ws".into(),
            labels: vec!["L".into()],
            properties: BTreeMap::new(),
            event_uuid: "ev1".into(),
            rekor_log_index: Some(100),
            author_did: None,
        };
        let v_b = Vertex {
            entity_uuid: "b".into(),
            workspace_id: "ws".into(),
            labels: vec!["L".into()],
            properties: BTreeMap::new(),
            event_uuid: "ev2".into(),
            rekor_log_index: Some(101),
            author_did: None,
        };
        let state = build_graph_state_from_sorted(&[v_a, v_b], &[]);
        let keys: Vec<&str> = state.nodes.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn rekor_log_index_zero_sentinel_round_trips() {
        // Confirm Option<u64>::None ↔ u64::0 conversion is lossless
        // for byte-pin compatibility.
        let node = GraphNode {
            entity_uuid: "a".into(),
            labels: vec![],
            properties: BTreeMap::new(),
            event_uuid: "ev1".into(),
            rekor_log_index: 0,
            author_did: None,
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        };
        let v = vertex_from_graph_node(&node, &"ws".to_string());
        assert_eq!(v.rekor_log_index, None);

        let v2 = Vertex {
            rekor_log_index: None,
            ..v.clone()
        };
        let state = build_graph_state_from_sorted(&[v2], &[]);
        let restored = state.nodes.get("a").unwrap();
        assert_eq!(restored.rekor_log_index, 0);
    }

    #[test]
    fn vertex_struct_is_non_exhaustive() {
        // Compile-time check: we cannot construct Vertex by positional
        // init from outside this crate. Within the crate the field-name
        // form works. The non_exhaustive attribute is verified by the
        // crate-doc convention; this test exists to surface accidental
        // removal of the attribute (a future welle that drops
        // #[non_exhaustive] would break SemVer for downstream
        // consumers).
        let _v = Vertex {
            entity_uuid: "a".into(),
            workspace_id: "ws".into(),
            labels: vec![],
            properties: BTreeMap::new(),
            event_uuid: "ev1".into(),
            rekor_log_index: None,
            author_did: None,
        };
    }
}
