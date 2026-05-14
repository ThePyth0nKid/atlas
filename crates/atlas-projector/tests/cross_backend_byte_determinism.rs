//! V2-β Welle 17b: cross-backend byte-determinism test.
//!
//! This test pins the central guarantee of ADR-Atlas-010 §4
//! sub-decision #6 (byte-determinism adapter contract): the
//! `graph_state_hash` produced by `ArcadeDbBackend::canonical_state()`
//! MUST be byte-identical to the hash produced by
//! `InMemoryBackend::canonical_state()` when both backends are
//! populated with the SAME logical workspace contents.
//!
//! **Why this test is `#[ignore]`-gated:**
//!
//! Running the test requires a live ArcadeDB Server instance the test
//! can `POST /api/v1/begin/{db}` against. We do NOT add Docker /
//! Testcontainers to the unit-test path because:
//!
//! 1. The byte-pin signal must work without Docker on contributor
//!    laptops (a contributor without Docker should still get a clean
//!    `cargo test` run).
//! 2. The byte-pin test on the InMemoryBackend
//!    (`backend_trait_conformance::byte_pin_through_in_memory_backend`)
//!    already provides one of the TWO paths through the pinned hex;
//!    the cross-backend test is the THIRD path (Layer 2 in
//!    ADR-Atlas-011 §8.1 watchlist).
//! 3. W17c's Docker-Compose CI workflow
//!    (`.github/workflows/atlas-arcadedb-smoke.yml`, deferred to W17c)
//!    will set `ATLAS_ARCADEDB_URL=http://arcadedb:2480` +
//!    `ATLAS_ARCADEDB_PASSWORD=<test-pw>` and run the test via
//!    `cargo test -- --ignored cross_backend`.
//!
//! Local-developer flow when an ArcadeDB instance IS available:
//! ```sh
//! export ATLAS_ARCADEDB_URL=http://localhost:2480
//! export ATLAS_ARCADEDB_USERNAME=root
//! export ATLAS_ARCADEDB_PASSWORD=playwithdata
//! cargo test -p atlas-projector --test cross_backend_byte_determinism -- --ignored
//! ```
//!
//! **Expected output:** the pinned hex
//! `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`
//! reproduced through the ArcadeDb backend's `canonical_state()` —
//! confirming that the §4.9 adapter contract is honoured AND that
//! byte-determinism survives the backend swap.

use std::collections::BTreeMap;

use atlas_projector::{
    ArcadeDbBackend, BackendEdge, BackendVertex, BasicAuth, GraphStateBackend, InMemoryBackend,
    WorkspaceId,
};
use url::Url;

/// Workspace id for the cross-backend test. ASCII-safe; passes
/// `check_workspace_id` at the boundary. The trailing `-w17b` is a
/// non-collision marker so a developer with a local ArcadeDB instance
/// won't accidentally reuse a real workspace name.
fn workspace_id() -> WorkspaceId {
    "cross-backend-pin-w17b".to_string()
}

/// EXPECTED PINNED HASH — copied verbatim from
/// `crates/atlas-projector/src/canonical.rs`
/// `tests::graph_state_hash_byte_determinism_pin`. Any change here
/// MUST be intentional + crate-version-bumped per the cascade
/// documented in `canonical.rs::graph_state_hash_byte_determinism_pin`.
const EXPECTED_HEX: &str = "8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4";

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
        workspace_id(),
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
        workspace_id(),
        from.to_string(),
        to.to_string(),
        label.to_string(),
        BTreeMap::new(),
        event_uuid.to_string(),
        rekor_log_index,
        author_did.map(String::from),
    )
}

/// Populate a backend with the 3-node + 2-edge fixture mirroring the
/// V2-α byte-pin fixture exactly.
fn populate_fixture<B: GraphStateBackend>(backend: &B) {
    let mut txn = backend.begin(&workspace_id()).expect("begin succeeded");

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
    .expect("upsert node-a");

    // Node B: V1-era event, no author_did, one label, no props.
    txn.upsert_vertex(&make_vertex(
        "node-b",
        &["Dataset"],
        "01HEVENT0002",
        Some(1001),
        None,
        BTreeMap::new(),
    ))
    .expect("upsert node-b");

    // Node C.
    txn.upsert_vertex(&make_vertex(
        "node-c",
        &["Model"],
        "01HEVENT0003",
        Some(1002),
        None,
        BTreeMap::new(),
    ))
    .expect("upsert node-c");

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
    .expect("upsert edge-ab");

    // Edge BC: no author_did.
    txn.upsert_edge(&make_edge(
        "edge-bc",
        "node-b",
        "node-c",
        "trains",
        "01HEVENT0005",
        Some(1004),
        None,
    ))
    .expect("upsert edge-bc");

    txn.commit().expect("commit succeeded");
}

/// Cross-backend byte-determinism test.
///
/// **Gating:** the test is `#[ignore]`. It runs only when the
/// `ATLAS_ARCADEDB_URL` env var is set; otherwise it `return`s early
/// after printing a diagnostic line.
///
/// **Assertions:**
/// 1. The InMemoryBackend's `canonical_state()` reproduces
///    `EXPECTED_HEX` (defence-in-depth — also covered by
///    `backend_trait_conformance::byte_pin_through_in_memory_backend`).
/// 2. The ArcadeDbBackend's `canonical_state()` reproduces
///    `EXPECTED_HEX` after being populated with the SAME fixture.
/// 3. The two hashes are byte-identical to each other.
///
/// **Cleanup:** the test does NOT clean up the per-workspace ArcadeDB
/// database. CI is expected to use ephemeral ArcadeDB containers (one
/// container per test run); a local developer running this against a
/// long-lived ArcadeDB instance may want to drop the
/// `atlas_ws_cross_backend_pin_w17b` database between runs to ensure
/// the test starts from an empty state.
#[test]
#[ignore = "requires live ArcadeDB; set ATLAS_ARCADEDB_URL to run"]
fn cross_backend_byte_determinism_pin() {
    let Ok(arcadedb_url) = std::env::var("ATLAS_ARCADEDB_URL") else {
        eprintln!(
            "skipping cross_backend_byte_determinism_pin: \
             ATLAS_ARCADEDB_URL not set (W17c CI sets it)"
        );
        return;
    };
    let username =
        std::env::var("ATLAS_ARCADEDB_USERNAME").unwrap_or_else(|_| "root".to_string());
    let password = std::env::var("ATLAS_ARCADEDB_PASSWORD").expect(
        "ATLAS_ARCADEDB_PASSWORD must be set alongside ATLAS_ARCADEDB_URL",
    );

    // -------------------------------------------------------------
    // Path 1: InMemoryBackend
    // -------------------------------------------------------------
    let in_memory = InMemoryBackend::new();
    populate_fixture(&in_memory);
    let in_memory_hex = hex::encode(
        in_memory
            .canonical_state(&workspace_id())
            .expect("in-memory canonical_state"),
    );
    assert_eq!(
        in_memory_hex, EXPECTED_HEX,
        "InMemoryBackend hex {in_memory_hex} != expected {EXPECTED_HEX}"
    );

    // -------------------------------------------------------------
    // Path 2: ArcadeDbBackend
    // -------------------------------------------------------------
    let base_url = Url::parse(&arcadedb_url).expect("ATLAS_ARCADEDB_URL is a valid URL");
    let arcadedb = ArcadeDbBackend::new(base_url, BasicAuth::new(username, password))
        .expect("ArcadeDbBackend constructs");
    populate_fixture(&arcadedb);
    let arcadedb_hex = hex::encode(
        arcadedb
            .canonical_state(&workspace_id())
            .expect("arcadedb canonical_state"),
    );

    // -------------------------------------------------------------
    // Cross-path assertions
    // -------------------------------------------------------------
    assert_eq!(
        arcadedb_hex, EXPECTED_HEX,
        "ArcadeDbBackend hex {arcadedb_hex} != expected pinned {EXPECTED_HEX}. \
         The §4.9 adapter contract was likely violated — verify Cypher \
         `ORDER BY n.entity_uuid ASC` / `ORDER BY e.edge_id ASC` and \
         that all stamping fields (event_uuid, rekor_log_index, author_did) \
         round-trip through the ArcadeDB schema correctly."
    );
    assert_eq!(
        in_memory_hex, arcadedb_hex,
        "byte-determinism violated: InMemoryBackend ({in_memory_hex}) \
         and ArcadeDbBackend ({arcadedb_hex}) produced different hashes \
         for the same logical workspace contents"
    );
}
