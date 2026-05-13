//! V2-β Welle 17a: `ArcadeDbBackend` STUB.
//!
//! Every method on this impl is currently `unimplemented!()` with a
//! `"W17b: ..."` message. The stub exists to:
//!
//! 1. Prove the [`GraphStateBackend`](super::GraphStateBackend) trait
//!    surface is implementable for a real HTTP-bound backend (no
//!    hidden lifetime / object-safety problems lurking).
//! 2. Lock in the field set + struct shape so W17b implementation is
//!    a pure fill-in-the-blanks job — no API negotiation between
//!    welles.
//! 3. Reserve the public name `ArcadeDbBackend` in the
//!    `atlas-projector` SemVer surface so downstream code can
//!    pattern-match on the type during the W17b transition without
//!    re-naming churn.
//!
//! ## What W17b will do
//!
//! - Add `reqwest = { version = "0.12", features = ["rustls-tls"] }`
//!   to `Cargo.toml`.
//! - Replace the `()` placeholder fields with `reqwest::Client`,
//!   per-workspace ArcadeDB credentials, session-id cache, etc.
//! - Implement each `unimplemented!()` method via the HTTP API
//!   endpoints documented in `docs/V2-BETA-ARCADEDB-SPIKE.md` §3:
//!   - `begin(&workspace_id)` → POST `/api/v1/begin/{db}`, session id
//!     stored in `arcadedb-session-id` header for subsequent calls.
//!   - `vertices_sorted` → POST `/api/v1/query/{db}` with Cypher
//!     `MATCH (n) RETURN n ORDER BY n.entity_uuid ASC` (§4.9 adapter).
//!   - `edges_sorted` → POST `/api/v1/query/{db}` with Cypher
//!     `MATCH ()-[e]->() RETURN e ORDER BY e.edge_id ASC` (§4.9 adapter).
//!   - `WorkspaceTxn::commit` → POST `/api/v1/commit/{db}`.
//!   - `WorkspaceTxn::rollback` → POST `/api/v1/rollback/{db}`.
//!   - `WorkspaceTxn::batch_upsert` → single multi-statement Cypher
//!     transaction (OQ-2 batching rationale, see `mod.rs` docstring).
//!
//! ## Why `unimplemented!()` and not `todo!()`
//!
//! `unimplemented!()` is the canonical Rust idiom for "this method is
//! permanent in the API surface but not yet filled in". `todo!()` is
//! the idiom for "this is scaffolding I will replace before merge".
//! The trait method is part of the permanent surface; only the body
//! is W17b's job — so `unimplemented!()` is the right choice.
//!
//! ## Why no `reqwest` dep in W17a
//!
//! The stub does not need it (every method panics before touching
//! HTTP). Adding `reqwest` to `Cargo.toml` in W17a would: (a) bloat
//! the `Cargo.lock` diff by ~20 transitive deps, (b) couple W17a's
//! merge to the `reqwest`-features audit that W17b will do anyway,
//! (c) be wasted compile time on every CI run between W17a merge and
//! W17b merge. W17b adds the dep alongside the first method body
//! that uses it.

use crate::backend::{Edge, GraphStateBackend, UpsertResult, Vertex, WorkspaceId, WorkspaceTxn};
use crate::error::ProjectorResult;

/// Stub [`GraphStateBackend`] implementation for ArcadeDB.
///
/// All trait methods are `unimplemented!()`. W17b fills them in.
///
/// The struct carries no fields today (other than the placeholder
/// described in `_marker`); W17b will add `reqwest::Client`,
/// base-URL, credentials, etc. Construction goes through
/// [`ArcadeDbBackend::new`] so downstream code can rely on a stable
/// constructor signature.
#[derive(Debug, Default)]
pub struct ArcadeDbBackend {
    /// Placeholder field. W17b removes this and adds the real
    /// connection state. Kept as `()` rather than absent so
    /// `#[derive(Default)]` does not pick up a no-field shape that
    /// would change semantics when the first real field lands.
    #[allow(dead_code)]
    _marker: (),
}

impl ArcadeDbBackend {
    /// Construct a new (stub) `ArcadeDbBackend`. W17b will replace
    /// this with a constructor accepting base URL + credentials.
    ///
    /// Note: this constructor itself does NOT panic — only the trait
    /// methods do. That is intentional: `cargo check` consumers that
    /// instantiate the type for compile-time wiring tests should not
    /// be blocked.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl GraphStateBackend for ArcadeDbBackend {
    fn begin(&self, _workspace_id: &WorkspaceId) -> ProjectorResult<Box<dyn WorkspaceTxn + '_>> {
        unimplemented!(
            "W17b: ArcadeDbBackend::begin — POST /api/v1/begin/{{db}}, session id in arcadedb-session-id header"
        )
    }

    fn vertices_sorted(&self, _workspace_id: &WorkspaceId) -> ProjectorResult<Vec<Vertex>> {
        unimplemented!(
            "W17b: ArcadeDbBackend::vertices_sorted — Cypher MATCH (n) RETURN n ORDER BY n.entity_uuid ASC (spike §4.9 adapter)"
        )
    }

    fn edges_sorted(&self, _workspace_id: &WorkspaceId) -> ProjectorResult<Vec<Edge>> {
        unimplemented!(
            "W17b: ArcadeDbBackend::edges_sorted — Cypher MATCH ()-[e]->() RETURN e ORDER BY e.edge_id ASC (spike §4.9 adapter)"
        )
    }

    // `canonical_state` deliberately does NOT have a stub override here
    // — it falls through to the trait-default impl, which calls
    // `vertices_sorted` + `edges_sorted` (which panic). So calling
    // `canonical_state` on the stub ALSO panics, with the underlying
    // `vertices_sorted` panic message bubbling up. That is the
    // intended semantic for a stub backend.

    fn backend_id(&self) -> &'static str {
        // Returning the production string here (rather than e.g.
        // "arcadedb-stub") is intentional: the trait method's
        // identity is part of the ProjectorRunAttestation contract.
        // Code reading `backend_id` is testing for the production
        // identity; the stub stays consistent so wiring tests don't
        // need a special case.
        "arcadedb-server"
    }
}

/// Stub `WorkspaceTxn` for `ArcadeDbBackend`. W17b replaces with the
/// production HTTP-session-bound implementation. All methods panic.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct ArcadeDbTxn {
    _marker: (),
}

impl WorkspaceTxn for ArcadeDbTxn {
    fn upsert_vertex(&mut self, _v: &Vertex) -> ProjectorResult<UpsertResult> {
        unimplemented!(
            "W17b: ArcadeDbTxn::upsert_vertex — Cypher MERGE (n:Vertex {{entity_uuid: $eid}}) SET n += $props"
        )
    }

    fn upsert_edge(&mut self, _e: &Edge) -> ProjectorResult<UpsertResult> {
        unimplemented!(
            "W17b: ArcadeDbTxn::upsert_edge — Cypher MATCH (a {{entity_uuid: $from}}),(b {{entity_uuid: $to}}) MERGE (a)-[r:LABEL {{edge_id: $eid}}]->(b)"
        )
    }

    fn batch_upsert(
        &mut self,
        _vertices: &[Vertex],
        _edges: &[Edge],
    ) -> ProjectorResult<Vec<UpsertResult>> {
        unimplemented!(
            "W17b: ArcadeDbTxn::batch_upsert — single multi-statement Cypher transaction per OQ-2 (vertices before edges)"
        )
    }

    fn commit(self: Box<Self>) -> ProjectorResult<()> {
        unimplemented!("W17b: ArcadeDbTxn::commit — POST /api/v1/commit/{{db}}")
    }

    fn rollback(self: Box<Self>) -> ProjectorResult<()> {
        unimplemented!("W17b: ArcadeDbTxn::rollback — POST /api/v1/rollback/{{db}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arcadedb_backend_constructs() {
        // Constructor must not panic — only trait methods do.
        let _b = ArcadeDbBackend::new();
    }

    #[test]
    fn arcadedb_backend_id_is_arcadedb_server() {
        let b = ArcadeDbBackend::new();
        assert_eq!(b.backend_id(), "arcadedb-server");
    }

    #[test]
    #[should_panic(expected = "W17b: ArcadeDbBackend::vertices_sorted")]
    fn arcadedb_vertices_sorted_panics_as_stub() {
        let b = ArcadeDbBackend::new();
        let _ = b.vertices_sorted(&"ws".to_string());
    }

    #[test]
    #[should_panic(expected = "W17b: ArcadeDbBackend::edges_sorted")]
    fn arcadedb_edges_sorted_panics_as_stub() {
        let b = ArcadeDbBackend::new();
        let _ = b.edges_sorted(&"ws".to_string());
    }

    #[test]
    #[should_panic(expected = "W17b: ArcadeDbBackend::begin")]
    fn arcadedb_begin_panics_as_stub() {
        let b = ArcadeDbBackend::new();
        let _ = b.begin(&"ws".to_string());
    }
}
