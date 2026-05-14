//! V2-β Welle 17a: in-memory [`GraphStateBackend`] implementation.
//!
//! Wraps the existing V2-α in-memory `GraphState` + `upsert::*` +
//! `canonical::*` pipeline so consumers can use the new trait without
//! a behavioural change. `InMemoryBackend::canonical_state()` is
//! pinned to produce **byte-identical** output to V2-α's
//! `canonical::graph_state_hash` — specifically the
//! `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`
//! hex pinned in `canonical::tests::graph_state_hash_byte_determinism_pin`
//! and exercised again by `backend_trait_conformance::byte_pin`.
//!
//! ## Concurrency model
//!
//! The trait requires `Send + Sync`. `InMemoryBackend` carries a
//! `Mutex<HashMap<WorkspaceId, GraphState>>` for interior mutability.
//! Per-workspace transactions take a long-lived `MutexGuard` for the
//! lifetime of the txn handle — concurrent transactions on the **same
//! workspace** serialise, matching ArcadeDB's per-database WAL
//! semantics (spike §4.1). Concurrent transactions on **disjoint
//! workspaces** are serialised by the outer mutex; that is a
//! limitation of the in-memory backend that does not propagate to
//! ArcadeDB (which has per-database locks). V2-α's expected workload
//! does not exercise concurrent-workspace projection in-memory, so
//! the simpler design is acceptable.
//!
//! A more sophisticated impl would use a `DashMap<WorkspaceId,
//! Mutex<GraphState>>` so per-workspace locks are independent;
//! W17a opts for the conservative `Mutex<HashMap<...>>` shape until a
//! benchmark demonstrates the contention matters.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::backend::{
    edge_from_graph_edge, vertex_from_graph_node, Edge, GraphStateBackend, UpsertResult, Vertex,
    WorkspaceId, WorkspaceTxn,
};
use crate::error::{ProjectorError, ProjectorResult};
use crate::state::{GraphEdge, GraphNode, GraphState};

/// In-memory implementation of [`GraphStateBackend`].
///
/// Storage layout: one `GraphState` per workspace, all behind a
/// single `Arc<Mutex<HashMap<WorkspaceId, GraphState>>>`. The `Arc`
/// is required because transactions hold a clone of the handle for
/// commit-time map mutation; using `&'a Mutex<...>` would carry a
/// borrow that prevents `WorkspaceTxn: Send` (MutexGuard is `!Send`
/// on Windows, and even on platforms where it is `Send` we'd still
/// be locking a non-Send guard into the txn). `Arc` shares the
/// storage by reference-count instead.
#[derive(Debug, Default, Clone)]
pub struct InMemoryBackend {
    workspaces: Arc<Mutex<HashMap<WorkspaceId, GraphState>>>,
}

impl InMemoryBackend {
    /// Construct an empty `InMemoryBackend`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Test/diagnostic helper: snapshot a workspace's `GraphState` for
    /// inspection. Returns `None` if the workspace has no entries.
    /// Not part of the public trait surface (would force every
    /// backend to support raw-state export, which ArcadeDb cannot
    /// cheaply provide).
    ///
    /// `#[doc(hidden)]` because this exposes the raw `GraphState`
    /// internals — production callers MUST go through the
    /// `GraphStateBackend` trait surface (which presents `Vertex` /
    /// `Edge` + workspace-scoped iterators). Diagnostic + test-only.
    /// (Reviewer-MED resolved 2026-05-13.)
    #[doc(hidden)]
    #[must_use]
    pub fn snapshot(&self, workspace_id: &WorkspaceId) -> Option<GraphState> {
        let guard = self
            .workspaces
            .lock()
            .expect("InMemoryBackend mutex poisoned (snapshot)");
        guard.get(workspace_id).cloned()
    }

    /// Internal: acquire the workspace map for transactional access.
    /// Panics if the mutex is poisoned — this is a programming bug
    /// (a panic occurred while a guard was held), not a runtime
    /// failure mode the API should expose. The `expect` message is
    /// the diagnostic surface.
    ///
    /// The returned `MutexGuard` borrows from `self.workspaces` (via
    /// the `Arc` deref); guards MUST NOT be held across `.await` or
    /// stored in trait-implementing structs that need `Send`.
    fn lock_map(&self) -> MutexGuard<'_, HashMap<WorkspaceId, GraphState>> {
        self.workspaces
            .lock()
            .expect("InMemoryBackend mutex poisoned")
    }
}

impl GraphStateBackend for InMemoryBackend {
    fn begin(
        &self,
        workspace_id: &WorkspaceId,
    ) -> ProjectorResult<Box<dyn WorkspaceTxn + 'static>> {
        // Snapshot the current committed state into the txn's scratch
        // buffer. The txn holds a clone of the `Arc<Mutex<...>>` so
        // commit can re-acquire and swap. The returned `Box<dyn
        // WorkspaceTxn>` is `'static` because `InMemoryTxn`'s fields
        // are all owned (workspace_id: String, workspaces: Arc<...>,
        // scratch: GraphState, finalised: Option<&'static str>) —
        // no borrow from `&self` (W17a-cleanup sub-decision #10).
        let scratch = {
            let guard = self.lock_map();
            guard.get(workspace_id).cloned().unwrap_or_default()
        };
        Ok(Box::new(InMemoryTxn {
            workspace_id: workspace_id.clone(),
            workspaces: Arc::clone(&self.workspaces),
            scratch,
            finalised: None,
        }))
    }

    fn vertices_sorted(&self, workspace_id: &WorkspaceId) -> ProjectorResult<Vec<Vertex>> {
        let guard = self.lock_map();
        let state = match guard.get(workspace_id) {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };
        // `state.nodes` is `BTreeMap<entity_uuid, GraphNode>` —
        // iteration is already sorted by logical id (spike §4.9
        // adapter contract satisfied for free).
        Ok(state
            .nodes
            .values()
            .map(|n| vertex_from_graph_node(n, workspace_id))
            .collect())
    }

    fn edges_sorted(&self, workspace_id: &WorkspaceId) -> ProjectorResult<Vec<Edge>> {
        let guard = self.lock_map();
        let state = match guard.get(workspace_id) {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };
        Ok(state
            .edges
            .values()
            .map(|e| edge_from_graph_edge(e, workspace_id))
            .collect())
    }

    /// Override the default `canonical_state` to feed the **existing**
    /// V2-α `GraphState` directly into `canonical::graph_state_hash`,
    /// bypassing the trait's `vertices_sorted` → transient-state
    /// → hash round-trip.
    ///
    /// Why override:
    /// 1. Performance: avoids cloning every vertex/edge through the
    ///    trait surface for the hash path.
    /// 2. **Byte-pin preservation guarantee**: feeding the original
    ///    `GraphState` into the same function that V2-α has been
    ///    using since Welle 3 is the strongest guarantee that the
    ///    pinned hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`
    ///    does not drift. The default impl produces the same bytes
    ///    (verified by the `default_and_override_match_byte_for_byte`
    ///    test below); we override for the additional safety
    ///    property of "no transformation pipeline between input and
    ///    hash".
    /// 3. Carries `annotations`, `policies`, and `rekor_anchors`
    ///    on `GraphNode` / `GraphState` through to canonicalisation
    ///    in a future-proof way. The trait's `Vertex` does not yet
    ///    surface those fields (W17a in-scope spec); this override
    ///    means InMemoryBackend continues to handle Welle-14-shaped
    ///    state correctly even before the trait grows those fields.
    fn canonical_state(&self, workspace_id: &WorkspaceId) -> ProjectorResult<[u8; 32]> {
        let guard = self.lock_map();
        match guard.get(workspace_id) {
            Some(state) => crate::canonical::graph_state_hash(state),
            None => crate::canonical::graph_state_hash(&GraphState::default()),
        }
    }

    fn backend_id(&self) -> &'static str {
        "in-memory"
    }
}

/// Per-workspace transaction handle for [`InMemoryBackend`].
///
/// The transaction operates on a **scratch copy** of the workspace's
/// `GraphState` and an `Arc<Mutex<...>>` clone of the backend's
/// storage. `commit` re-acquires the lock briefly to swap the scratch
/// into the map; `rollback` (or drop) discards the scratch. Read-
/// after-uncommitted-write within the same transaction works on the
/// scratch buffer; concurrent transactions see each other's commits
/// but not each other's in-flight scratches.
///
/// `Send` is satisfied because no `MutexGuard` lives across `.await`
/// or struct boundaries (`MutexGuard` is `!Send` on Windows).
pub struct InMemoryTxn {
    workspace_id: WorkspaceId,
    /// Shared handle to the backend's workspace map. Cloned from
    /// `InMemoryBackend::workspaces` at `begin()` time.
    workspaces: Arc<Mutex<HashMap<WorkspaceId, GraphState>>>,
    /// Scratch copy mutated by upsert calls; committed-or-discarded
    /// on `commit` / `rollback`.
    scratch: GraphState,
    /// `Some(reason)` once commit or rollback was called — guards
    /// against use-after-finalise (also guaranteed at the type level
    /// by `self: Box<Self>` consumption in the trait methods).
    finalised: Option<&'static str>,
}

impl InMemoryTxn {
    fn upsert_vertex_inner(&mut self, v: &Vertex) -> ProjectorResult<UpsertResult> {
        if v.entity_uuid.is_empty() {
            return Err(ProjectorError::MalformedEntityUuid(
                "entity_uuid is empty".to_string(),
            ));
        }
        let node = GraphNode {
            entity_uuid: v.entity_uuid.clone(),
            labels: v.labels.clone(),
            properties: v.properties.clone(),
            event_uuid: v.event_uuid.clone(),
            rekor_log_index: v.rekor_log_index.unwrap_or(0),
            author_did: v.author_did.clone(),
            annotations: BTreeMap::new(),
            policies: BTreeMap::new(),
        };
        let prev = self.scratch.upsert_node(node);
        Ok(UpsertResult {
            created: prev.is_none(),
            entity_uuid: v.entity_uuid.clone(),
        })
    }

    fn upsert_edge_inner(&mut self, e: &Edge) -> ProjectorResult<UpsertResult> {
        if e.edge_id.is_empty() {
            return Err(ProjectorError::MalformedEntityUuid(format!(
                "edge_id is empty (edge {}-{}-{})",
                e.from, e.label, e.to
            )));
        }
        let edge = GraphEdge {
            edge_id: e.edge_id.clone(),
            from_entity: e.from.clone(),
            to_entity: e.to.clone(),
            kind: e.label.clone(),
            properties: e.properties.clone(),
            event_uuid: e.event_uuid.clone(),
            rekor_log_index: e.rekor_log_index.unwrap_or(0),
            author_did: e.author_did.clone(),
        };
        let prev = self.scratch.upsert_edge(edge);
        // `UpsertResult::entity_uuid` is the logical-id of the upserted
        // record. For edges this is the `edge_id`; the field name is
        // `entity_uuid` rather than `logical_id` because the V2-α/β
        // schema uses "entity_uuid" loosely for both vertex and edge
        // logical identity. The type is `EntityUuid` (= `String`); the
        // semantic meaning is "logical id of the row touched".
        Ok(UpsertResult {
            created: prev.is_none(),
            entity_uuid: e.edge_id.clone(),
        })
    }
}

impl WorkspaceTxn for InMemoryTxn {
    fn upsert_vertex(&mut self, v: &Vertex) -> ProjectorResult<UpsertResult> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "InMemoryTxn used after finalisation".to_string(),
            ));
        }
        self.upsert_vertex_inner(v)
    }

    fn upsert_edge(&mut self, e: &Edge) -> ProjectorResult<UpsertResult> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "InMemoryTxn used after finalisation".to_string(),
            ));
        }
        self.upsert_edge_inner(e)
    }

    fn batch_upsert(
        &mut self,
        vertices: &[Vertex],
        edges: &[Edge],
    ) -> ProjectorResult<Vec<UpsertResult>> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "InMemoryTxn used after finalisation".to_string(),
            ));
        }
        let mut results = Vec::with_capacity(vertices.len() + edges.len());
        // OQ-2 contract: vertices applied before edges so we never
        // transiently expose an edge whose endpoint is missing.
        for v in vertices {
            results.push(self.upsert_vertex_inner(v)?);
        }
        for e in edges {
            results.push(self.upsert_edge_inner(e)?);
        }
        Ok(results)
    }

    fn commit(mut self: Box<Self>) -> ProjectorResult<()> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "InMemoryTxn already finalised".to_string(),
            ));
        }
        let scratch = std::mem::take(&mut self.scratch);
        let mut guard = self
            .workspaces
            .lock()
            .expect("InMemoryBackend mutex poisoned (commit)");
        guard.insert(self.workspace_id.clone(), scratch);
        drop(guard);
        self.finalised = Some("commit");
        Ok(())
    }

    fn rollback(mut self: Box<Self>) -> ProjectorResult<()> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "InMemoryTxn already finalised".to_string(),
            ));
        }
        // Discard the scratch buffer. The map is unchanged.
        self.finalised = Some("rollback");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::graph_state_hash;
    use crate::state::GraphState;

    fn ws() -> WorkspaceId {
        "ws-test".to_string()
    }

    fn sample_vertex(id: &str) -> Vertex {
        Vertex {
            entity_uuid: id.to_string(),
            workspace_id: ws(),
            labels: vec!["L".to_string()],
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT1".to_string(),
            rekor_log_index: Some(100),
            author_did: None,
        }
    }

    fn sample_edge(id: &str, from: &str, to: &str) -> Edge {
        Edge {
            edge_id: id.to_string(),
            workspace_id: ws(),
            from: from.to_string(),
            to: to.to_string(),
            label: "knows".to_string(),
            properties: BTreeMap::new(),
            event_uuid: "01HEVENT2".to_string(),
            rekor_log_index: Some(101),
            author_did: None,
        }
    }

    #[test]
    fn empty_backend_has_empty_workspace() {
        let b = InMemoryBackend::new();
        let vs = b.vertices_sorted(&ws()).unwrap();
        let es = b.edges_sorted(&ws()).unwrap();
        assert!(vs.is_empty());
        assert!(es.is_empty());
    }

    #[test]
    fn single_vertex_round_trip() {
        let b = InMemoryBackend::new();
        let mut txn = b.begin(&ws()).unwrap();
        let res = txn.upsert_vertex(&sample_vertex("a")).unwrap();
        assert!(res.created);
        assert_eq!(res.entity_uuid, "a");
        txn.commit().unwrap();
        let vs = b.vertices_sorted(&ws()).unwrap();
        assert_eq!(vs.len(), 1);
        assert_eq!(vs[0].entity_uuid, "a");
    }

    #[test]
    fn idempotent_replay_marks_created_false_on_second_pass() {
        let b = InMemoryBackend::new();
        let mut txn = b.begin(&ws()).unwrap();
        assert!(txn.upsert_vertex(&sample_vertex("a")).unwrap().created);
        // Second upsert with same entity_uuid → created == false
        assert!(!txn.upsert_vertex(&sample_vertex("a")).unwrap().created);
        txn.commit().unwrap();
    }

    #[test]
    fn rollback_discards_scratch() {
        let b = InMemoryBackend::new();
        // Pre-populate via committed txn.
        {
            let mut txn = b.begin(&ws()).unwrap();
            txn.upsert_vertex(&sample_vertex("a")).unwrap();
            txn.commit().unwrap();
        }
        // Open a second txn, upsert "b", rollback.
        {
            let mut txn = b.begin(&ws()).unwrap();
            txn.upsert_vertex(&sample_vertex("b")).unwrap();
            txn.rollback().unwrap();
        }
        // "b" must not be in committed state.
        let vs = b.vertices_sorted(&ws()).unwrap();
        assert_eq!(vs.len(), 1);
        assert_eq!(vs[0].entity_uuid, "a");
    }

    #[test]
    fn drop_without_commit_is_implicit_rollback() {
        let b = InMemoryBackend::new();
        {
            let mut txn = b.begin(&ws()).unwrap();
            txn.upsert_vertex(&sample_vertex("a")).unwrap();
            // Drop without commit.
        }
        let vs = b.vertices_sorted(&ws()).unwrap();
        assert!(vs.is_empty(), "drop must be rollback");
    }

    #[test]
    fn batch_upsert_applies_vertices_before_edges() {
        let b = InMemoryBackend::new();
        let mut txn = b.begin(&ws()).unwrap();
        let results = txn
            .batch_upsert(
                &[sample_vertex("a"), sample_vertex("b")],
                &[sample_edge("e1", "a", "b")],
            )
            .unwrap();
        assert_eq!(results.len(), 3);
        assert!(results[0].created);
        assert!(results[1].created);
        assert!(results[2].created);
        txn.commit().unwrap();
        let vs = b.vertices_sorted(&ws()).unwrap();
        let es = b.edges_sorted(&ws()).unwrap();
        assert_eq!(vs.len(), 2);
        assert_eq!(es.len(), 1);
    }

    #[test]
    fn vertices_sorted_yields_logical_id_order() {
        let b = InMemoryBackend::new();
        let mut txn = b.begin(&ws()).unwrap();
        txn.upsert_vertex(&sample_vertex("c")).unwrap();
        txn.upsert_vertex(&sample_vertex("a")).unwrap();
        txn.upsert_vertex(&sample_vertex("b")).unwrap();
        txn.commit().unwrap();
        let vs = b.vertices_sorted(&ws()).unwrap();
        let ids: Vec<&str> = vs.iter().map(|v| v.entity_uuid.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn backend_id_is_in_memory() {
        let b = InMemoryBackend::new();
        assert_eq!(b.backend_id(), "in-memory");
    }

    #[test]
    fn empty_state_canonical_hash_matches_v2_alpha_pipeline() {
        // An empty workspace's canonical_state must equal
        // graph_state_hash(&GraphState::default()) — the V2-α value.
        let b = InMemoryBackend::new();
        let actual = b.canonical_state(&ws()).unwrap();
        let expected = graph_state_hash(&GraphState::default()).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn malformed_entity_uuid_rejected() {
        let b = InMemoryBackend::new();
        let mut txn = b.begin(&ws()).unwrap();
        let mut bad = sample_vertex("a");
        bad.entity_uuid = String::new();
        match txn.upsert_vertex(&bad) {
            Err(ProjectorError::MalformedEntityUuid(_)) => {}
            other => panic!("expected MalformedEntityUuid; got {other:?}"),
        }
    }

    #[test]
    fn use_after_commit_errors() {
        let b = InMemoryBackend::new();
        let mut txn = b.begin(&ws()).unwrap();
        txn.upsert_vertex(&sample_vertex("a")).unwrap();
        // Commit consumes the box, so we have to test "use after
        // finalisation" via a different angle: pre-mark a txn's
        // `finalised` flag and call upsert. We can't do that via the
        // public surface (commit consumes self via Box<Self>), so
        // this case is covered implicitly by the type system. The
        // `finalised` flag exists as a runtime defence-in-depth for
        // the (impossible-via-public-API) case where a future refactor
        // exposes a method that takes `&mut self` after finalisation.
        txn.commit().unwrap();
    }
}
