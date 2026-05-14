//! V2-β Welle 17a: `GraphStateBackend` trait-conformance tests.
//!
//! Four invariants:
//!
//! 1. [`in_memory_round_trip`] — `InMemoryBackend` accepts a vertex
//!    upsert and surfaces it via `vertices_sorted` in logical-id
//!    order.
//! 2. [`byte_pin_through_in_memory_backend`] — exercising the SAME
//!    3-node-2-edge fixture used by
//!    `canonical::tests::graph_state_hash_byte_determinism_pin`
//!    through the `InMemoryBackend` trait surface produces the same
//!    pinned hex (`8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`).
//!    This is the LOAD-BEARING regression for the byte-determinism CI
//!    pin — any backend-trait change that breaks it trips here BEFORE
//!    the original byte-pin test.
//! 3. [`arcadedb_stub_panics`] — the `ArcadeDbBackend` stub's
//!    `unimplemented!()` is reachable from the trait surface.
//! 4. [`batch_upsert_orders_vertices_before_edges`] — OQ-2 contract.

use std::collections::BTreeMap;

use atlas_projector::{
    check_value_depth_and_size, check_workspace_id, ArcadeDbBackend, BackendEdge, BackendVertex,
    GraphStateBackend, InMemoryBackend, ProjectorError, UpsertResult, WorkspaceId, WorkspaceTxn,
};

fn ws() -> WorkspaceId {
    "ws-conformance-test".to_string()
}

fn make_vertex(
    entity_uuid: &str,
    labels: &[&str],
    event_uuid: &str,
    rekor_log_index: Option<u64>,
    author_did: Option<&str>,
    properties: BTreeMap<String, serde_json::Value>,
) -> BackendVertex {
    BackendVertex::new(
        entity_uuid.to_string(),
        ws(),
        labels.iter().map(|s| s.to_string()).collect(),
        properties,
        event_uuid.to_string(),
        rekor_log_index,
        author_did.map(String::from),
    )
}

fn make_edge(
    edge_id: &str,
    from: &str,
    to: &str,
    label: &str,
    event_uuid: &str,
    rekor_log_index: Option<u64>,
    author_did: Option<&str>,
) -> BackendEdge {
    BackendEdge::new(
        edge_id.to_string(),
        ws(),
        from.to_string(),
        to.to_string(),
        label.to_string(),
        BTreeMap::new(),
        event_uuid.to_string(),
        rekor_log_index,
        author_did.map(String::from),
    )
}

#[test]
fn in_memory_round_trip() {
    let b = InMemoryBackend::new();
    let mut txn = b.begin(&ws()).unwrap();
    let r1: UpsertResult = txn
        .upsert_vertex(&make_vertex(
            "node-x",
            &["L"],
            "ev1",
            Some(10),
            None,
            BTreeMap::new(),
        ))
        .unwrap();
    assert!(r1.created);
    assert_eq!(r1.entity_uuid, "node-x");
    txn.commit().unwrap();

    let vs = b.vertices_sorted(&ws()).unwrap();
    assert_eq!(vs.len(), 1);
    assert_eq!(vs[0].entity_uuid, "node-x");
    assert_eq!(vs[0].event_uuid, "ev1");
    assert_eq!(vs[0].rekor_log_index, Some(10));
}

#[test]
fn byte_pin_through_in_memory_backend() {
    // Mirror the exact 3-node + 2-edge fixture from
    // canonical::tests::graph_state_hash_byte_determinism_pin and
    // assert the InMemoryBackend trait surface produces the same
    // pinned hex. THIS IS THE LOAD-BEARING TEST FOR W17A.
    let b = InMemoryBackend::new();
    let mut txn = b.begin(&ws()).unwrap();

    // Node A: V2-α event with author_did, two labels, simple props.
    let mut node_a_props = BTreeMap::new();
    node_a_props.insert("name".to_string(), serde_json::json!("alice"));
    node_a_props.insert("count".to_string(), serde_json::json!(42));
    txn.upsert_vertex(&make_vertex(
        "node-a",
        &["Person", "Sensitive"],
        "01HEVENT0001",
        Some(1000),
        Some("did:atlas:1111111111111111111111111111111111111111111111111111111111111111"),
        node_a_props,
    ))
    .unwrap();

    // Node B: V1-era event, no author_did, one label, no props.
    txn.upsert_vertex(&make_vertex(
        "node-b",
        &["Dataset"],
        "01HEVENT0002",
        Some(1001),
        None,
        BTreeMap::new(),
    ))
    .unwrap();

    // Node C.
    txn.upsert_vertex(&make_vertex(
        "node-c",
        &["Model"],
        "01HEVENT0003",
        Some(1002),
        None,
        BTreeMap::new(),
    ))
    .unwrap();

    // Edge AB: with author_did.
    txn.upsert_edge(&make_edge(
        "edge-ab",
        "node-a",
        "node-b",
        "uses",
        "01HEVENT0004",
        Some(1003),
        Some("did:atlas:2222222222222222222222222222222222222222222222222222222222222222"),
    ))
    .unwrap();

    // Edge BC: no author_did, but rekor_log_index=1004 to match
    // the V2-α byte-pin fixture exactly.
    txn.upsert_edge(&make_edge(
        "edge-bc",
        "node-b",
        "node-c",
        "trains",
        "01HEVENT0005",
        Some(1004),
        None,
    ))
    .unwrap();

    txn.commit().unwrap();

    // EXPECTED PINNED HASH — copied verbatim from
    // crates/atlas-projector/src/canonical.rs
    // tests::graph_state_hash_byte_determinism_pin. If the
    // backend trait abstraction changes canonical bytes, this
    // assertion fires alongside the source-of-truth byte-pin test
    // and the CI signal is preserved either way.
    //
    // Fixture parity: every (entity_uuid, labels, properties,
    // event_uuid, rekor_log_index, author_did) tuple matches the
    // original byte-pin fixture. The Option<u64> ↔ u64 sentinel
    // mapping in `vertex_from_graph_node` / `edge_from_graph_edge`
    // means Some(n) round-trips for n > 0; the fixture exercises
    // only n > 0 values to keep the cross-check unambiguous.
    let actual = b.canonical_state(&ws()).unwrap();
    let actual_hex = hex::encode(actual);
    let expected_hex = "8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4";
    assert_eq!(
        actual_hex, expected_hex,
        "InMemoryBackend trait surface broke graph_state_hash byte-determinism pin. \
         Any change here MUST be intentional + crate-version-bumped per the cascade \
         documented in canonical.rs::graph_state_hash_byte_determinism_pin."
    );
}

#[test]
fn backend_id_strings_are_stable() {
    assert_eq!(InMemoryBackend::new().backend_id(), "in-memory");
    assert_eq!(ArcadeDbBackend::new().backend_id(), "arcadedb-server");
}

#[test]
#[should_panic(expected = "W17b: ArcadeDbBackend::vertices_sorted")]
fn arcadedb_stub_panics() {
    let b = ArcadeDbBackend::new();
    // vertices_sorted is the simplest method to invoke that does not
    // require holding a txn first.
    let _ = b.vertices_sorted(&ws());
}

#[test]
fn batch_upsert_orders_vertices_before_edges() {
    // OQ-2 contract: vertices applied before edges so a fresh-
    // workspace cycle never sees a transient edge-without-endpoint
    // state. We assert two things:
    //   (a) the returned results vec has length vertices.len() +
    //       edges.len(), with vertices first.
    //   (b) all upserts surface as `created == true` on a fresh
    //       backend.
    let b = InMemoryBackend::new();
    let mut txn = b.begin(&ws()).unwrap();
    let vs = vec![
        make_vertex("a", &[], "ev1", None, None, BTreeMap::new()),
        make_vertex("b", &[], "ev2", None, None, BTreeMap::new()),
    ];
    let es = vec![make_edge("e1", "a", "b", "rel", "ev3", None, None)];
    let results = txn.batch_upsert(&vs, &es).unwrap();
    assert_eq!(results.len(), 3, "results = vertices + edges");
    // First two are vertices (in input order; logical-id sort is
    // applied at the storage layer, not the return slice).
    assert_eq!(results[0].entity_uuid, "a");
    assert_eq!(results[1].entity_uuid, "b");
    // Third is the edge.
    assert_eq!(results[2].entity_uuid, "e1");
    assert!(results.iter().all(|r| r.created));
    txn.commit().unwrap();
}

#[test]
fn batch_upsert_then_canonical_matches_individual_upserts() {
    // Verify that batch_upsert and a sequence of individual upserts
    // produce byte-identical canonical state — the OQ-2 batch path
    // is a pure performance optimisation, not a semantics change.
    let b1 = InMemoryBackend::new();
    let mut txn1 = b1.begin(&ws()).unwrap();
    txn1.upsert_vertex(&make_vertex("a", &["L"], "ev1", Some(1), None, BTreeMap::new()))
        .unwrap();
    txn1.upsert_vertex(&make_vertex("b", &["L"], "ev2", Some(2), None, BTreeMap::new()))
        .unwrap();
    txn1.upsert_edge(&make_edge("e1", "a", "b", "rel", "ev3", Some(3), None))
        .unwrap();
    txn1.commit().unwrap();

    let b2 = InMemoryBackend::new();
    let mut txn2 = b2.begin(&ws()).unwrap();
    txn2.batch_upsert(
        &[
            make_vertex("a", &["L"], "ev1", Some(1), None, BTreeMap::new()),
            make_vertex("b", &["L"], "ev2", Some(2), None, BTreeMap::new()),
        ],
        &[make_edge("e1", "a", "b", "rel", "ev3", Some(3), None)],
    )
    .unwrap();
    txn2.commit().unwrap();

    assert_eq!(
        b1.canonical_state(&ws()).unwrap(),
        b2.canonical_state(&ws()).unwrap()
    );
}

#[test]
fn vertices_sorted_emits_logical_id_order_not_insert_order() {
    // Welle 2 §3.5 invariant lifted to the trait surface: any backend
    // MUST surface vertices in entity_uuid-sorted order, NOT
    // insertion order. (For InMemoryBackend this is BTreeMap-guaranteed;
    // for ArcadeDbBackend W17b will enforce via Cypher ORDER BY.)
    let b = InMemoryBackend::new();
    let mut txn = b.begin(&ws()).unwrap();
    txn.upsert_vertex(&make_vertex("z", &[], "ev1", None, None, BTreeMap::new()))
        .unwrap();
    txn.upsert_vertex(&make_vertex("a", &[], "ev2", None, None, BTreeMap::new()))
        .unwrap();
    txn.upsert_vertex(&make_vertex("m", &[], "ev3", None, None, BTreeMap::new()))
        .unwrap();
    txn.commit().unwrap();

    let ids: Vec<String> = b
        .vertices_sorted(&ws())
        .unwrap()
        .into_iter()
        .map(|v| v.entity_uuid)
        .collect();
    assert_eq!(ids, vec!["a", "m", "z"]);
}

#[test]
fn rollback_does_not_pollute_canonical_state() {
    // Compute canonical hash of empty workspace, do a txn + rollback,
    // re-compute — must be identical.
    let b = InMemoryBackend::new();
    let h1 = b.canonical_state(&ws()).unwrap();

    let mut txn = b.begin(&ws()).unwrap();
    txn.upsert_vertex(&make_vertex(
        "pollutant",
        &["L"],
        "ev1",
        Some(99),
        None,
        BTreeMap::new(),
    ))
    .unwrap();
    txn.rollback().unwrap();

    let h2 = b.canonical_state(&ws()).unwrap();
    assert_eq!(h1, h2, "rollback must leave workspace state unchanged");
}

// ---------------------------------------------------------------------
// W17a-cleanup tests: boundary-validation helpers + 'static lifetime
// (ADR-Atlas-011 §4 sub-decisions #10 + #11).
// ---------------------------------------------------------------------

#[test]
fn check_workspace_id_accepts_typical_shapes() {
    // ULID-shape, UUID-shape, opaque slug — all OK.
    assert!(check_workspace_id("01J5M9V8K9X7T2QH4Z6A1P3R5W").is_ok());
    assert!(check_workspace_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
    assert!(check_workspace_id("acme-corp-prod-eu-west-1").is_ok());
    // 128-char boundary (length == 128 is allowed)
    let max = "a".repeat(128);
    assert!(check_workspace_id(&max).is_ok());
}

#[test]
fn check_workspace_id_rejects_empty() {
    match check_workspace_id("") {
        Err(ProjectorError::InvalidWorkspaceId { reason }) => {
            assert!(reason.contains("empty"), "reason was {reason:?}");
        }
        other => panic!("expected InvalidWorkspaceId(empty); got {other:?}"),
    }
}

#[test]
fn check_workspace_id_rejects_too_long() {
    let too_long = "x".repeat(129);
    match check_workspace_id(&too_long) {
        Err(ProjectorError::InvalidWorkspaceId { reason }) => {
            assert!(
                reason.contains("129") && reason.contains("128"),
                "reason was {reason:?}"
            );
        }
        other => panic!("expected InvalidWorkspaceId(length); got {other:?}"),
    }
}

#[test]
fn check_workspace_id_rejects_path_traversal_chars() {
    for forbidden in ["../etc/passwd", "ws/sub", "ws\\sub", "ws\0null"] {
        match check_workspace_id(forbidden) {
            Err(ProjectorError::InvalidWorkspaceId { reason }) => {
                assert!(
                    reason.contains("forbidden character"),
                    "input {forbidden:?}, reason was {reason:?}"
                );
            }
            other => panic!(
                "expected InvalidWorkspaceId(forbidden char) for {forbidden:?}; got {other:?}"
            ),
        }
    }
}

#[test]
fn check_workspace_id_rejects_non_ascii() {
    match check_workspace_id("workspace-überprüfung") {
        Err(ProjectorError::InvalidWorkspaceId { reason }) => {
            assert!(reason.contains("ASCII"), "reason was {reason:?}");
        }
        other => panic!("expected InvalidWorkspaceId(ASCII); got {other:?}"),
    }
}

#[test]
fn check_value_depth_and_size_accepts_typical_payload() {
    // Mimics a realistic property payload — 2 levels of nesting, ~100 bytes.
    let v = serde_json::json!({
        "name": "alice",
        "tags": ["sensitive", "audit"],
        "meta": { "source": "ingest", "rev": 3 }
    });
    assert!(check_value_depth_and_size(&v, 32, 64 * 1024).is_ok());
}

#[test]
fn check_value_depth_and_size_rejects_deep_nesting() {
    // Build a deeply-nested array: [[[...]]] 50 levels deep.
    let mut v = serde_json::Value::Null;
    for _ in 0..50 {
        v = serde_json::Value::Array(vec![v]);
    }
    match check_value_depth_and_size(&v, 32, 64 * 1024) {
        Err(ProjectorError::CanonicalisationFailed(msg)) => {
            assert!(msg.contains("depth"), "msg was {msg:?}");
        }
        other => panic!("expected CanonicalisationFailed(depth); got {other:?}"),
    }
}

#[test]
fn check_value_depth_and_size_rejects_oversized() {
    // Build a long string property — easily exceeds 1 KiB max_bytes.
    let big = "x".repeat(4096);
    let v = serde_json::json!({ "blob": big });
    match check_value_depth_and_size(&v, 32, 1024) {
        Err(ProjectorError::CanonicalisationFailed(msg)) => {
            assert!(
                msg.contains("size") && msg.contains("exceeds"),
                "msg was {msg:?}"
            );
        }
        other => panic!("expected CanonicalisationFailed(size); got {other:?}"),
    }
}

#[test]
fn begin_returns_static_txn_handle() {
    // W17a-cleanup sub-decision #10: the trait's `begin()` returns
    // `Box<dyn WorkspaceTxn + 'static>`. We verify both at the type
    // level (the explicit `let _: Box<...>` binding below would fail
    // to compile if the lifetime regressed) and at runtime (the txn
    // is usable, commitable, and outlives the natural-lifetime scope
    // it was created in).
    let b = InMemoryBackend::new();
    let mut txn: Box<dyn WorkspaceTxn + 'static> = b.begin(&ws()).unwrap();
    let r = txn
        .upsert_vertex(&make_vertex(
            "node-static",
            &["L"],
            "ev1",
            Some(7),
            None,
            BTreeMap::new(),
        ))
        .unwrap();
    assert!(r.created);
    txn.commit().unwrap();

    // Re-open + read back — confirms the commit took.
    let vs = b.vertices_sorted(&ws()).unwrap();
    assert_eq!(vs.len(), 1);
    assert_eq!(vs[0].entity_uuid, "node-static");
}
