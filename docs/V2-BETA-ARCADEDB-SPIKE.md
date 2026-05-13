# Atlas V2-β Welle 16 — ArcadeDB Embedded-Mode Spike

> **Status:** SHIPPED 2026-05-13.
> **Welle:** W16 (V2-β Phase 8).
> **Methodology:** public-knowledge-based architectural analysis. **No actual benchmarks executed** — recommendations based on documented ArcadeDB HTTP API behaviour, JVM operational characteristics, Rust async-ecosystem conventions, and Atlas-specific architectural requirements. Confidence levels (HIGH/MEDIUM/LOW) are stated per question.
> **Related:** ADR-Atlas-007 §6 (5 open questions for W17, resolved here); ADR-Atlas-010 (this spike's binding decision doc); DECISION-DB-4 (ArcadeDB Apache-2.0 primary lock, MEDIUM-HIGH confidence, established by V2-α Welle 2 spike).
> **Unblocks:** W17a (ArcadeDB driver scaffold + `GraphStateBackend` trait + ADR-Atlas-011), W17b (driver implementation), W17c (integration tests + Docker-Compose orchestration).
> **Counsel disclaimer:** all license analysis in §3 is engineer-perspective. Counsel-validated SSPLv1 §13 + Apache-2.0 §4-§5 opinions remain on Nelson's parallel counsel-engagement track and are pre-V2-β-1-ship blocking (per DECISION-COUNSEL-1).

---

## 1. Executive summary

**Recommendation:** **ArcadeDB Apache-2.0 PRIMARY (CONFIRMED, not re-evaluated)** + **SERVER MODE deployment** + **`reqwest` async Rust HTTP client**. Confidence: **HIGH** on license + mode + client choice; **MEDIUM-HIGH** on per-workspace concurrent-write semantics (HTTP-API behaviour-documented but not Atlas-load-tested); **MEDIUM** on byte-determinism preservation under ArcadeDB query result ordering (requires application-layer adapter, §4.9).

**What this spike resolved (vs ADR-Atlas-007 §6 + V2-α Welle 2's open questions):**

1. **Q1 Concurrent-workspace-write semantics** — ArcadeDB serialises writes per-database; Atlas's one-database-per-workspace pattern means concurrent writes to disjoint workspaces produce deterministically-mergeable state. Option A parallel-projection (ADR-Atlas-007 §3.1) is directly supported. **Confidence: MEDIUM-HIGH.**
2. **Q2 Per-workspace graph integrity** — ArcadeDB schema-required mode enforces edge-referential integrity at the database (workspace) level. Cross-workspace edges are forbidden by V1's event model + application-layer enforcement. **Confidence: HIGH.**
3. **Q3 Workspace isolation at query-time (SECURITY)** — Defence in depth: (a) per-workspace database isolation native to ArcadeDB; (b) projector-side workspace_id parameter binding; (c) Cypher AST-validator (ADR-009) enforces read-only structure (mutation prevention; does NOT enforce workspace_id presence — Layer 2 does). **Confidence: HIGH** with application-layer enforcement.
4. **Q4 Embedded vs server mode** (W16 scope extension beyond ADR-Atlas-007 §6's 5 questions; the 5 ADR-007 Qs map to this spike's Q1+Q3+Q4-corollary+Q8+Q9 respectively) — **SERVER mode wins**. Hermes-skill distribution constraint (JVM-in-`npx`-installable-skill = non-starter) + process-isolation security benefit + Rust↔JNI complexity avoidance + sidecar JVM RSS isolation. Embedded mode reconsidered only if HTTP latency exceeds 15ms p99 in W17c benchmarks. **Confidence: HIGH.**
5. **Q5 Rust HTTP client** — **`reqwest`**. Async, tokio-aligned, native TLS via `rustls-tls`, connection pooling, ~2 MB binary cost, de-facto Rust standard. Rejected: `ureq` (sync); hand-rolled `hyper` (no measurable benefit). **Confidence: HIGH.**
6. **Q6 FalkorDB fallback trigger thresholds** — 5 measurable criteria documented in §9. None currently fired.
7. **Q7 Docker-Compose CI orchestration** — Sketch in §8. New workflow `.github/workflows/atlas-arcadedb-smoke.yml` proposed for W17c.
8. **Q8 `GraphStateBackend` trait sketch** — ~40 lines Rust pseudo-code in §7. W17a fills concrete impl.
9. **Q9 Byte-determinism preservation** — Adapter required: query results MUST be sorted by `entity_uuid` before canonicalisation; `@rid` (insert-order) is NOT a substitute. Spec in §4.9. **Confidence: HIGH** on adapter; **MEDIUM** until W17b implementation validates.
10. **Q10 Performance ballpark** — Cold start ~350 ms (JVM warmup + first HTTP roundtrip); warm projection ~300-500 µs/event; 10M-event re-projection ~50-80 min single-threaded, ~6-10 min workspace-parallel (8 workers). Vs V2-α in-memory baseline: HTTP overhead adds ~6-10x per-event cost; acceptable at year-1 scale (matches V2-α ADR-Atlas-007 §2.3 aspiration of "10M events in <30 min on 16 cores").

**Bottom line:** The architectural unknowns ADR-Atlas-007 §6 surfaced are resolved in favour of ArcadeDB server-mode deployment. W17 proceeds against this design.

---

## 2. Context

V2-α Welle 2 chose ArcadeDB Apache-2.0 as Layer-2 primary with FalkorDB SSPLv1 as fallback, citing license-compatibility as the decisive factor for Atlas's open-core hosted-service tier. The Welle 2 spike was license-comparative; it did NOT resolve architectural unknowns about how ArcadeDB would actually be integrated into `atlas-projector` (Rust). ADR-Atlas-007 §6 enumerated 5 explicit open questions that W16 must resolve before W17a (ArcadeDB driver scaffold) locks the implementation.

This spike is the architectural counterpart to V2-α Welle 2's license counterpart. Together they form the complete pre-W17 evidence base.

**Phase-9 dependency:** W17a/b/c is the longest serial chain in V2-β's critical path (per `V2-BETA-DEPENDENCY-GRAPH.md` §5). Locking architectural decisions in a spike-doc before W17a starts avoids reactive design pivots mid-implementation.

---

## 3. ArcadeDB primer

**Project:** ArcadeDB is an open-source multi-model database (graph + document + key-value + time-series + vector) founded by Luca Garulli (also OrientDB founder). Apache License 2.0. Active development. Italian small-team OSS-native ownership (Arcade Analytics Srl).

**Architecture:**
- **Multi-model:** native vertex/edge property graph; document model; key-value; time-series; vector indexes (Lucene-based).
- **Query languages:** Cypher (graph), SQL (multi-model), Gremlin (graph), GraphQL, MongoDB query API. Atlas constrains driver config to Cypher-only.
- **Storage:** bucket-based on-disk format inherited from OrientDB. B-tree indexes. ACID transactions via WAL + bucket-level MVCC. Schema-required-by-default mode.
- **Deployment modes:**
  - **Embedded:** Java library, embeddable in JVM applications (Atlas would need Rust↔JNI bridge).
  - **Server:** standalone Java application, exposes HTTP API + binary protocol + Gremlin Server + others.
  - **Replicated (HA):** ArcadeDB native cluster mode.
- **License:** Apache License 2.0 (OSI-approved). No copyleft propagation. Compatible with all Atlas tiers.

**HTTP API:** REST-style endpoints under `/api/v1/`:
- `POST /api/v1/server/{db}` — create database.
- `POST /api/v1/begin/{db}` — start transaction; returns session id.
- `POST /api/v1/command/{db}` — execute Cypher/SQL/Gremlin/SQLScript command.
- `POST /api/v1/query/{db}` — execute read-only query.
- `POST /api/v1/commit/{db}` — commit transaction.
- `POST /api/v1/rollback/{db}` — rollback transaction.
- `GET /api/v1/ready` — healthcheck (200 OK when DB is accepting connections).
- HTTP Basic auth or JWT bearer; per-database users + roles.

**Cypher subset:** ArcadeDB supports openCypher's MATCH, CREATE, MERGE, SET, DELETE, WHERE, RETURN, WITH, ORDER BY, LIMIT, aggregations, variable-length paths. Some openCypher features are partially supported with documented limitations. V2-α Welle 2 verified the Atlas-required subset.

---

## 4. The 10 architectural questions answered

### 4.1 Q1 — Concurrent-workspace-write semantics

**Question:** Does ArcadeDB HTTP API guarantee that concurrent writes to workspace_a and workspace_b produce a deterministically-mergeable state?

**Research summary:** ArcadeDB's transaction model operates at the **database** level — each database has its own WAL, its own bucket-level locks, and its own MVCC controller. Atlas's design choice (locked here) is **one ArcadeDB database per Atlas workspace** — making "concurrent writes to workspace_a and workspace_b" map directly to "concurrent writes to ArcadeDB-database-a and ArcadeDB-database-b", which ArcadeDB documents as fully independent and supported. Within a single database, the HTTP API supports explicit transactions via `POST /api/v1/begin/{db}` → operations → `POST /api/v1/commit/{db}` with a session id passed in the `arcadedb-session-id` header.

**Finding:** Concurrent writes to disjoint workspaces are **fully supported and deterministically mergeable** under the one-database-per-workspace pattern. Per-workspace WAL ordering preserves causal ordering (events processed serially within a workspace per ADR-Atlas-007 §3.1 ordering-requirement). No global write lock; no cross-database deadlock risk.

**Risk:** If an operator misconfigures Atlas to share a single ArcadeDB database across workspaces, the concurrent-write guarantee degrades to within-database transaction serialisation. Mitigation: Atlas operator runbook MUST document one-database-per-workspace as the supported configuration.

**Confidence:** **MEDIUM-HIGH.** Documented HTTP-API behaviour; not Atlas-load-tested. W17c integration tests MUST include a 4-workspace concurrent-write scenario.

### 4.2 Q2 — Per-workspace graph-integrity constraints

**Question:** Does ArcadeDB enforce edge-referential integrity at the workspace (database) level or globally?

**Research summary:** ArcadeDB's schema-required mode supports vertex/edge constraints. Edge-referential-integrity is enforced when both endpoints are within the same database — ArcadeDB refuses to create an edge pointing to a non-existent vertex (returns 4xx error). Cross-database edges are not supported in the graph model. V1's event model explicitly forbids cross-workspace edges, so this is not a constraint Atlas needs to relax.

**Finding:** Per-workspace graph integrity is **enforced at the database level by ArcadeDB schema-required mode**.

**Confidence:** **HIGH.**

### 4.3 Q3 — Workspace isolation at query-time (SECURITY — tenant isolation)

**Question:** If 2 workspaces share the same ArcadeDB instance, can a Cypher query omit explicit `workspace_id` filter and accidentally return cross-workspace data?

**Research summary:** ArcadeDB's primary tenant-isolation mechanism is **per-database user/role separation** — each database has its own user set + ACLs. A user authenticated to database A cannot read database B unless explicitly granted. There is NO row-level / vertex-level security within a single database (no per-vertex ACL).

**Atlas's defence in depth (locked here):**
1. **Per-workspace database isolation (primary):** Atlas creates one ArcadeDB database per workspace. Cross-workspace queries are structurally impossible because the projector connects to a specific database. Auth credentials are per-database.
2. **Application-layer workspace_id binding (secondary):** All projector + Read-API + MCP tool queries bind `workspace_id` as a parameterised Cypher constant.
3. **Cypher AST-validator enforcement (tertiary):** the consolidated validator (`packages/atlas-cypher-validator/`, ADR-Atlas-009) enforces read-only query structure (mutation-keyword rejection + procedure-namespace blocking + string-concat heuristic + 4096-char cap + opener allowlist). **The validator does NOT enforce workspace_id presence** — workspace_id binding is the projector's responsibility at Layer 2.

**Finding:** Workspace isolation is **enforced at three layers**: database-isolation (ArcadeDB native), application-layer parameter binding (projector + Read-API + MCP), and Cypher AST validation (consolidated validator). **Atlas does NOT rely on ArcadeDB vertex-level security (which doesn't exist).**

**Risk:** Operator decides to share a single ArcadeDB database across workspaces to save resources → cross-workspace leak becomes possible if AST validator is bypassed. Mitigation: operator runbook MUST forbid shared-database configuration; W17c integration tests MUST include a negative-case test (Cypher without workspace_id, expect 400 rejection).

**Confidence:** **MEDIUM-HIGH** — Layer 1 (per-database isolation) is the bedrock guarantee; Layer 2 (application-layer projector workspace_id binding) is the active enforcer; Layer 3 (Cypher AST validator) hardens against MUTATION attacks but does NOT enforce workspace_id presence. Operator runbook MUST require per-database deployment configuration.

### 4.4 Q4 — JVM-dependency footprint + embedded vs server mode trade-offs

**Question:** ArcadeDB is Java-backed. What's the JVM requirement for embedded vs server mode?

**Research summary:**

| Aspect | Embedded mode (in-process) | Server mode (sidecar process) |
|---|---|---|
| Runtime | JVM in atlas-projector process via JNI bridge | Standalone JVM process; HTTP API |
| Disk footprint | ~80-150 MB ArcadeDB jar + ~120-180 MB JRE | Same on the sidecar; atlas-projector unchanged |
| Memory baseline (idle) | ~200-300 MB RSS added to atlas-projector | ~200-300 MB RSS in sidecar; atlas-projector unchanged (~15-30 MB Rust baseline) |
| Cold-start (first projection) | ~300-500 ms JVM warmup blocks projector startup | ~300-500 ms JVM warmup in sidecar; projector waits on healthcheck |
| Rust↔Java FFI complexity | HIGH — JNI bridge or J4RS or jni-rs crate | NONE — HTTP API is language-agnostic |
| Hermes-skill compatibility | **BLOCKER** — JVM in npx package not viable | **COMPATIBLE** — Hermes-skill talks HTTP |
| Multi-tenancy security | Lower — JVM exception could crash atlas-projector | Higher — process isolation |
| Docker / K8s deployment | One process | Two processes (atlas + arcadedb); docker-compose standard |
| Testing | Integration tests need JVM in CI Rust image | Integration tests use Docker-Compose with ArcadeDB image |

**Finding:** **SERVER MODE wins decisively** on five dimensions: (a) Hermes-skill compatibility (the highest-weight blocker), (b) Rust↔Java FFI complexity avoidance, (c) process-isolation security, (d) sidecar memory profile, (e) Docker-Compose deployment naturalness. Embedded mode has a marginal latency advantage (~1-3 ms saved per query) but Atlas's expected workload absorbs HTTP overhead easily.

**Confidence:** **HIGH.**

**Embedded-mode reconsideration trigger:** If W17c integration tests show ArcadeDB HTTP-API p99 latency exceeds 15 ms for the depth-3 Read-API traversal benchmark at 10M-event workspace scale, embedded mode is reconsidered.

### 4.5 Q5 — Rust HTTP client choice

**Question:** Which Rust HTTP client library for ArcadeDB HTTP API consumption from `atlas-projector`?

**Research summary:**

| Client | Async/sync | TLS | Conn pooling | Binary cost | Atlas-fit |
|---|---|---|---|---|---|
| `reqwest` | Async (tokio) + blocking shim | `rustls-tls` or `native-tls` | Yes (built-in) | ~2 MB | **BEST** — tokio-aligned, de-facto standard |
| `ureq` | Sync only | rustls or native-tls | Limited | ~600 KB | Rejected — blocks async projector pipeline |
| `hyper` (raw) | Async (tokio) | rustls or native-tls | Manual | ~1.5 MB | Rejected — `reqwest` IS a `hyper` wrapper |
| `surf` | Async (async-std) | rustls or native-tls | Yes | ~2 MB | Rejected — async-std mismatch with tokio |
| `isahc` | Async (libcurl) | libcurl | Yes | C dep (~3 MB) | Rejected — C dependency + libcurl OpenSSL conflicts |

**Finding:** **`reqwest`** with `rustls-tls` feature. Async, tokio-aligned, native TLS without OpenSSL system dep, connection pooling, ~2 MB binary cost. De-facto Rust ecosystem standard.

**Confidence:** **HIGH.**

### 4.6 Q6 — FalkorDB fallback trigger criteria

**Question:** At what point would Atlas FLIP from ArcadeDB-primary to FalkorDB-primary?

**Findings — 5 trigger criteria:**

| # | Trigger | Measurable threshold | Source signal |
|---|---|---|---|
| T1 | ArcadeDB Cypher subset insufficient | W17b implementation discovers ≥3 Atlas projection queries cannot be expressed in ArcadeDB Cypher without rewrite | W17b implementation report |
| T2 | ArcadeDB HTTP latency exceeds target | Read-API depth-3 traversal p99 > 15 ms at 10M-event workspace | W17c benchmark + deployment telemetry |
| T3 | ArcadeDB project deprecation / acquisition | Public announcement of acquisition OR repo archive OR license pivot from Apache-2.0 | GitHub repo watch |
| T4 | JVM cold-start blocks Hermes-skill | Sidecar startup time > 5 sec in Hermes-skill `npx` install flow | Hermes-skill V2-γ acceptance tests |
| T5 | Counsel ruling on SSPL §13 favourable to Atlas | Counsel-validated opinion eliminates the license-decisive factor of DECISION-DB-4 | Counsel deliverable |

**None of T1-T5 fired as of 2026-05-13.** DECISION-DB-4 confirmed by W16. Fallback path: re-projection from `events.jsonl` (authoritative Layer 1) into FalkorDB; ~1-2 sessions projector rewrite + replay; zero customer downtime via dual-write.

**Confidence:** **HIGH** on criteria definition.

### 4.7 Q7 — Docker-Compose orchestration for CI tests

**Sketch (W17c will implement):**

```yaml
# docker-compose.arcadedb-smoke.yml
version: "3.8"
services:
  arcadedb:
    image: arcadedata/arcadedb:24.10.1
    environment:
      JAVA_OPTS: "-Xms256m -Xmx512m"
      arcadedb.server.rootPassword: "${ATLAS_ARCADEDB_ROOT_PASSWORD}"
      arcadedb.server.mode: "production"
    ports:
      - "2480:2480"  # HTTP API
    volumes:
      - arcadedb-data:/home/arcadedb/databases
    healthcheck:
      test: ["CMD", "curl", "-f", "-u", "root:${ATLAS_ARCADEDB_ROOT_PASSWORD}", "http://localhost:2480/api/v1/ready"]
      interval: 5s
      timeout: 3s
      retries: 6
      start_period: 30s
volumes:
  arcadedb-data:
```

**CI workflow proposal:** new `.github/workflows/atlas-arcadedb-smoke.yml`:
- Trigger: PRs touching `crates/atlas-projector/src/backend/`, push to master.
- Steps: checkout → docker compose up arcadedb → wait for healthcheck → `cargo test -p atlas-projector --features arcadedb-integration` → docker compose down.
- Timeout: 8 minutes.
- Integration with existing 7 byte-determinism CI pins: this NEW workflow does NOT touch the 7 pins; byte-determinism is preserved because the projector-state-hash invariant operates on the canonicalised graph state, which the §4.9 adapter ensures is identical across InMemory and ArcadeDb backends.

**Image size:** `arcadedata/arcadedb:24.10.1` ~480 MB. Startup ~15-25 sec.

**Confidence:** **HIGH.**

### 4.8 Q8 — `GraphStateBackend` trait sketch

See §7 for the ~40-line Rust trait sketch. Design-level only; W17a writes the production trait + impl scaffold.

**Confidence:** **MEDIUM-HIGH.**

### 4.9 Q9 — Byte-determinism preservation across backends

**Question:** Confirm that the `graph_state_hash` byte-pin is preserved when switching from InMemory to ArcadeDb backend.

**Research summary:** V2-α canonicalisation (Welle 3) computes `graph_state_hash` over canonical CBOR encoding with map-keys sorted per RFC 8949 §4.2.1. The InMemory backend uses `BTreeMap<EntityUuid, Vertex>` — iteration order is sorted by `entity_uuid` (logical identifier) — directly compatible with canonical CBOR sort.

ArcadeDB query result order depends on the query. A naive `MATCH (n) RETURN n` returns results in storage order (`@rid` — insert-order), which is NOT logical-identifier order. If the projector reads results in `@rid` order and canonicalises directly, the resulting CBOR bytes will differ from the InMemory backend's output → `graph_state_hash` mismatch → CI gate breakage + ProjectorRunAttestation chain invalidation.

**Adapter spec (locked here):**
- All ArcadeDB queries that produce data for canonicalisation MUST include `ORDER BY entity_uuid ASC` (or equivalent SQL `ORDER BY entity_uuid`).
- The application-layer adapter wraps query results in a `BTreeMap<EntityUuid, Vertex>` before passing to canonical-form builder.
- Edge queries: `ORDER BY edge_id ASC` where `edge_id = blake3(workspace_id || from_entity_uuid || to_entity_uuid || edge_label || nonce_or_event_uuid)`.
- The adapter is part of the `GraphStateBackend::canonical_state()` method (see §7).

> **Storage requirement (W17b implementation note):** `edge_id` MUST be persisted as a named property on each ArcadeDB edge record (`SET e.edge_id = $computed_edge_id` at insertion time). `ORDER BY edge_id ASC` in Cypher operates on this stored property, NOT on `@rid`. Relying on ArcadeDB's internal `@rid` for canonicalisation ordering is FORBIDDEN per Q9 adapter contract.

**`@rid` caveat:** `@rid` is auto-increment within a bucket. Two projector runs producing the same *logical* state via different historical insertion orders produce different `@rid` values. The adapter MUST use logical-identifier sort, NOT `@rid` sort.

**Finding:** Byte-determinism is **preserved** when the ArcadeDb backend implements the adapter contract specified above.

**Confidence:** **HIGH** on adapter design; **MEDIUM** on implementation correctness until W17b passes the byte-determinism CI pin against an ArcadeDB-backed projection.

### 4.10 Q10 — Performance ballpark

**Research summary + estimates:**

| Operation | V2-α InMemory baseline | ArcadeDB server-mode estimate | Ratio |
|---|---|---|---|
| Cold start (process boot + first projection) | ~50 ms | ~350 ms (JVM warmup ~300 ms + first HTTP roundtrip) | 7x |
| Warm projection per event (incremental upsert) | ~50 µs | ~300-500 µs | 6-10x |
| Full re-projection of 10M events (single-threaded) | ~8 min | ~50-80 min | 6-10x |
| Full re-projection of 10M events (workspace-parallel, 8 workers) | ~1 min | ~6-10 min | 6-10x |
| Read-API depth-3 traversal (10M-vertex workspace) | ~200 µs | ~3-8 ms | 15-40x |
| `graph_state_hash` computation | ~500 ms for 10M-vertex state | ~2-5 sec | 4-10x |

**Comparison to V2-α targets:** V2-α target per ADR-Atlas-007 §2.3 was "10M events in <30 min on 16 cores" with workspace-parallel. ArcadeDB workspace-parallel estimate (~6-10 min) meets the V2-α aspiration.

**Caveat:** all numbers are public-knowledge-based estimates. W17c integration tests MUST capture actual numbers.

**Confidence:** **MEDIUM** on numbers; **HIGH** on the qualitative conclusion that server-mode HTTP overhead is absorbable at year-1 scale.

---

## 5. Trade-off matrix — embedded vs server mode

| Dimension | Embedded mode | Server mode | Weight | Winner |
|---|---|---|---|---|
| JVM footprint impact on `atlas-projector` | +200-300 MB RSS | 0 MB (sidecar) | HIGH | **Server** |
| Latency (per-query, localhost HTTP) | -3 ms vs server | +3 ms vs embedded | LOW (year-1 absorbs) | Embedded (marginal) |
| Multi-tenancy / security | Lower (in-process) | Higher (process isolation) | MED-HIGH | **Server** |
| Deployment complexity (Docker / K8s) | One process | Two processes | MED | Embedded (marginal) |
| Hermes-skill compatibility | **BLOCKER** | Compatible | **DECISIVE** | **Server** |
| Rust↔Java FFI complexity | HIGH (JNI / jni-rs / J4RS) | NONE | HIGH | **Server** |
| Testing (CI integration) | JVM in Rust CI image | Docker-Compose with ArcadeDB image | MED | Server (marginal) |
| Sidecar crash recovery | Crash → projector crash | Crash → projector retries via HTTP | MED | **Server** |
| EU-data-residency deployment | Single artefact | Two artefacts; both can be EU-hosted | LOW | Tie |

**Net assessment:** **Server mode wins decisively** on 5 dimensions (one of which — Hermes-skill — is decisive on its own).

---

## 6. Recommendation

**ArcadeDB Apache-2.0 PRIMARY (CONFIRMED, not re-evaluated) + SERVER MODE + `reqwest` async Rust HTTP client.**

DECISION-DB-4's MEDIUM-HIGH confidence is updated: **license-side remains MEDIUM-HIGH (pending counsel)**; **architecture-side is now HIGH** (W16 spike). Combined operational confidence for W17 is **HIGH-on-architecture, MEDIUM-HIGH-overall**.

**What this enables for W17:**
- W17a writes `crates/atlas-projector/src/backend/mod.rs` containing the `GraphStateBackend` trait (sketched in §7) + ADR-Atlas-011.
- W17b writes `crates/atlas-projector/src/backend/arcadedb.rs` containing the `ArcadeDbBackend` impl using `reqwest` + Cypher commands.
- W17c writes `crates/atlas-projector/tests/arcadedb_integration.rs` + new `.github/workflows/atlas-arcadedb-smoke.yml` per §8.

---

## 7. GraphStateBackend trait sketch (Rust pseudo-code)

```rust
// crates/atlas-projector/src/backend/mod.rs
// SKETCH — W17a writes the production version

use anyhow::Result;
use atlas_trust_core::{EntityUuid, EdgeId, WorkspaceId};
use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub entity_uuid: EntityUuid,
    pub workspace_id: WorkspaceId,
    pub labels: Vec<String>,
    pub properties: BTreeMap<String, serde_cbor::Value>,
    pub event_uuid: String,           // V2-α Welle 1 stamping
    pub rekor_log_index: Option<u64>, // V2-α Welle 1 stamping
    pub author_did: String,           // V2-α Welle 1 stamping
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub edge_id: EdgeId,
    pub workspace_id: WorkspaceId,
    pub from: EntityUuid,
    pub to: EntityUuid,
    pub label: String,
    pub properties: BTreeMap<String, serde_cbor::Value>,
    pub event_uuid: String,
    pub rekor_log_index: Option<u64>,
    pub author_did: String,
}

#[derive(Debug)]
pub struct UpsertResult {
    pub created: bool,           // true on create, false on update
    pub entity_uuid: EntityUuid,
}

/// Per-workspace transaction handle. Drop = rollback unless commit() called.
pub trait WorkspaceTxn: Send {
    fn upsert_vertex(&mut self, v: &Vertex) -> Result<UpsertResult>;
    fn upsert_edge(&mut self, e: &Edge) -> Result<UpsertResult>;
    fn commit(self: Box<Self>) -> Result<()>;
    fn rollback(self: Box<Self>) -> Result<()>;
}

/// Backend abstraction for Layer-2 graph state.
/// V2-α: InMemoryBackend; V2-β W17: ArcadeDbBackend; V2-γ candidate: FalkorDbBackend.
pub trait GraphStateBackend: Send + Sync {
    /// Open a new per-workspace transaction.
    fn begin(&self, workspace_id: &WorkspaceId) -> Result<Box<dyn WorkspaceTxn>>;

    /// Read all vertices for a workspace, sorted by entity_uuid (logical identifier).
    /// MUST be sorted for byte-determinism (Q9 adapter contract).
    fn vertices_sorted(&self, workspace_id: &WorkspaceId)
        -> Result<Box<dyn Iterator<Item = Result<Vertex>> + Send + '_>>;

    /// Read all edges for a workspace, sorted by edge_id (logical identifier).
    fn edges_sorted(&self, workspace_id: &WorkspaceId)
        -> Result<Box<dyn Iterator<Item = Result<Edge>> + Send + '_>>;

    /// Compute the canonical graph_state_hash for a workspace.
    /// Default impl: canonical CBOR over (vertices_sorted, edges_sorted) -> blake3.
    /// Backends MAY override for performance but MUST produce identical bytes.
    fn canonical_state(&self, workspace_id: &WorkspaceId) -> Result<[u8; 32]>;

    /// Backend identity string for ProjectorRunAttestation event.
    fn backend_id(&self) -> &'static str;  // "in-memory" | "arcadedb-server" | "falkordb-fallback"
}
```

**W17a notes:**
- The trait is `Send + Sync` for tokio multi-task projection.
- `WorkspaceTxn` is per-task, not Send-required at the txn level.
- The default `canonical_state()` impl in `mod.rs` lives in the trait body so every backend gets byte-determinism for free.
- `EntityUuid` and `EdgeId` are existing V2-α types in `atlas-trust-core`.

---

## 8. CI strategy

**Existing 7 byte-determinism CI pins (preserved):**
1. COSE_Sign1 signing-input byte pin (V1)
2. COSE_Sign1 signature byte pin (V1)
3. COSE_Sign1 envelope byte pin (V1)
4. Sigstore Rekor anchor byte pin (V1)
5. Anchored-bundle byte pin (V1)
6. pubkey-bundle byte pin (V1)
7. `graph_state_hash` canonicalisation byte pin (V2-α Welle 3)

**NEW workflow for W17c (W16 proposes):** `.github/workflows/atlas-arcadedb-smoke.yml` (see §4.7 sketch).

**Cross-check matrix (W17b acceptance):** for the same input events.jsonl, both `InMemoryBackend::canonical_state()` and `ArcadeDbBackend::canonical_state()` MUST produce byte-identical hash. Encoded as a new test `tests/cross_backend_byte_determinism.rs` in W17b.

---

## 9. FalkorDB fallback trigger thresholds (measurable criteria)

See §4.6 Q6 for the 5 criteria (T1-T5). Summary:
- **T1:** ≥3 Cypher rewrites needed (W17b implementation report).
- **T2:** Read-API p99 > 15 ms at 10M-vertex (W17c benchmark).
- **T3:** ArcadeDB deprecation / acquisition (continuous monitoring).
- **T4:** Hermes-skill cold-start > 5 sec (V2-γ acceptance).
- **T5:** Counsel reverses SSPL §13 interpretation (Nelson-led).

**None fired as of 2026-05-13.** DECISION-DB-4 confirmed by W16.

---

## 10. W17 entry criteria

**Required before W17a starts (all satisfied by W16 merge):**
- [x] DECISION-DB-4 confirmed by spike-doc + ADR-Atlas-010
- [x] Embedded vs server mode resolved (server mode locked)
- [x] Rust HTTP client choice resolved (`reqwest` locked)
- [x] `GraphStateBackend` trait sketch published (§7)
- [x] Byte-determinism adapter contract specified (§4.9)
- [x] Workspace-isolation defence-in-depth pattern specified (§4.3)
- [x] Docker-Compose orchestration sketched (§4.7 + §8)

**W17a's required deliverables (ADR-Atlas-011 + scaffold):**
- Concrete `GraphStateBackend` trait in `crates/atlas-projector/src/backend/mod.rs`
- `InMemoryBackend` impl wired up to existing V2-α projection logic
- `ArcadeDbBackend` stub (compiles, returns `unimplemented!()` for all methods)
- ADR-Atlas-011 explaining trait design choices + W17b/c roadmap

**W17b/c deferred deliverables:**
- W17b: full `ArcadeDbBackend` impl using `reqwest` + Cypher; new test `tests/cross_backend_byte_determinism.rs` MUST pass (both `InMemoryBackend::canonical_state()` and `ArcadeDbBackend::canonical_state()` produce byte-identical hash for the same input events.jsonl)
- W17c: Docker-Compose CI workflow + integration tests + benchmark capture

---

## 11. Open questions for V2-γ (not blocking V2-β)

- **OQ-1:** Should Atlas operate its OWN ArcadeDB cluster as a managed-service tier offering, or expose Atlas + customer-bring-your-own-ArcadeDB only?
- **OQ-2:** Multi-region ArcadeDB replication for EU-data-residency: ArcadeDB native cluster mode vs application-layer dual-write. Defer to V2-γ when first EU customer signs.
- **OQ-3:** Mem0g (W18) integration with ArcadeDB Lucene-based vector indexes vs Mem0g's own vector store. Defer to W18 design.
- **OQ-4:** GraalVM native-image build of ArcadeDB server (~50 MB vs ~480 MB Docker image). Defer to V2-γ self-hosted-tier polishing.
- **OQ-5:** ArcadeDB authentication: HTTP Basic (per-database root password) vs JWT bearer tokens with per-tenant scoping. V2-β starts with Basic; V2-γ may want JWT.

---

**End V2-β Welle 16 ArcadeDB Spike.** Recommendation: ArcadeDB primary CONFIRMED, server-mode deployment, `reqwest` Rust HTTP client. DECISION-DB-4 architecture-side confidence raised to HIGH; license-side awaits counsel. W17a proceeds with these locks.
