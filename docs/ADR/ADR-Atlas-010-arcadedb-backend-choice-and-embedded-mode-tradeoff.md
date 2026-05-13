# ADR-Atlas-010 — ArcadeDB Backend Choice + Embedded-Mode Trade-off

| Field             | Value                                                              |
|-------------------|--------------------------------------------------------------------|
| **Status**        | Accepted                                                           |
| **Date**          | 2026-05-13                                                         |
| **Welle**         | V2-β Welle 16 (Phase 8)                                            |
| **Authors**       | Nelson Mehlis (`@ThePyth0nKid`); welle-16 subagent (architect role) |
| **Replaces**      | —                                                                  |
| **Superseded by** | —                                                                  |
| **Related**       | `DECISION-DB-4` (ArcadeDB Apache-2.0 primary lock, V2-α Welle 2 spike); ADR-Atlas-007 §6 (5 open questions resolved here); `docs/V2-BETA-ARCADEDB-SPIKE.md` (W16 architectural spike — companion doc); ADR-Atlas-011 (W17a, forthcoming — driver scaffold + trait design); `DECISION-SEC-4` (Cypher passthrough hardening); ADR-Atlas-009 (`@atlas/cypher-validator` consolidation) |

---

## 1. Context

### 1.1 The unresolved architectural surface after V2-α Welle 2

V2-α Welle 2 (2026-05-12) chose ArcadeDB Apache-2.0 as Atlas's Layer-2 graph database with FalkorDB SSPLv1 as fallback. The spike's confidence was **MEDIUM-HIGH** with explicit pre-conditions for raising to HIGH: (a) actual benchmark harness, (b) counsel-validated SSPL §13 opinion, (c) operator-runbook EU-data-residency validation. The Welle 2 spike was **license-comparative** — it picked the DB on legal grounds. It did NOT resolve **architectural** unknowns about how ArcadeDB would actually be integrated into `atlas-projector` (Rust).

Five open questions accumulated in ADR-Atlas-007 §6 (V2-β Welle 10, parallel-projection design):
1. Concurrent-workspace-write semantics — does ArcadeDB serialise per-workspace or globally?
2. Per-workspace graph-integrity constraints — workspace-level vs global edge-referential-integrity?
3. Workspace isolation at query-time (SECURITY) — tenant isolation guarantees?
4. Streaming inserts vs batch atomic writes — per-workspace atomic transaction boundaries?
5. Native UUID indexing performance for graph-state-hash lookups.

W16's job: resolve all five, plus the embedded-vs-server-mode trade-off, plus the Rust HTTP client choice, plus the byte-determinism preservation guarantee, plus the Docker-Compose CI orchestration sketch, plus the `GraphStateBackend` trait shape, plus measurable FalkorDB fallback trigger thresholds.

### 1.2 Why this is one ADR, not several

The 5 ADR-007 §6 questions + the embedded/server + the HTTP client + the byte-determinism adapter all converge on a single architectural decision: **how does Atlas integrate ArcadeDB into its Rust projection pipeline?** Splitting this into 3-5 ADRs would force near-duplicate Context sections and produce drift over time. The single binding decision recorded here is "ArcadeDB Apache-2.0 primary CONFIRMED + server mode + `reqwest` Rust HTTP client + per-database-per-workspace pattern". All sub-decisions follow from this triple-lock.

### 1.3 Why this matters for Atlas specifically

- **Atlas's open-core monetization** depends on license compatibility across all tiers. ArcadeDB Apache-2.0 is structurally compatible. FalkorDB SSPLv1 §13 is structurally incompatible without per-deployment commercial-license negotiation.
- **Hermes-skill distribution channel** (V2-γ scope) ships as npm package consumed by `npx`. Bundling a JVM is a non-starter (size + platform-binaries + license complexity) — this is the decisive constraint for embedded-vs-server.
- **V1's bedrock trust property** (offline WASM verifier reads `events.jsonl` deterministically) must survive any Layer-2 backend choice. Byte-determinism preservation (Q9 adapter) is load-bearing for the CI gate + the ProjectorRunAttestation chain.
- **Multi-tenant security** (Q3) is a SECURITY-critical requirement for any Atlas customer with PII workspaces.

---

## 2. Decision drivers

- **License compatibility:** DECISION-DB-4 lock (ArcadeDB Apache-2.0) MUST be respected unless evidence overturns. W16 spike found no such evidence.
- **Hermes-skill distribution constraint:** no JVM in `npx` package → embedded mode is structurally incompatible with the V2-γ distribution channel.
- **Byte-determinism preservation:** `graph_state_hash` invariant (V2-α Welle 3) MUST hold across any backend swap. Adapter contract is non-negotiable.
- **Tenant-isolation defence in depth:** SECURITY-critical for multi-tenant SaaS. Per-database isolation alone is insufficient if operators mis-configure.
- **Rust async-runtime alignment:** Atlas's projector uses tokio (per V2-α Welle 4-5 emission pipeline). The HTTP client must be tokio-compatible.
- **Operational simplicity:** server-mode sidecar is the well-trodden path for "Rust app + Java database"; embedded mode requires Rust↔Java FFI bridge.
- **CI cost:** new workflow + Docker-Compose adds ~5 min to PR check time; acceptable.

---

## 3. Considered options

### 3.1 Option A: ArcadeDB server mode + `reqwest` async HTTP client + one-database-per-workspace (RECOMMENDED)

**Mechanism:**
- ArcadeDB runs as standalone server process (Docker `arcadedata/arcadedb:24.x` in production; same in CI).
- `atlas-projector` (Rust) talks to ArcadeDB via HTTP API using `reqwest` with `rustls-tls` feature.
- One ArcadeDB database = one Atlas workspace. Atlas creates a new database on first event in a workspace; deletes the database on workspace deletion.
- Per-workspace atomic transactions via `POST /api/v1/begin/{db}` → operations → `POST /api/v1/commit/{db}` (session id in `arcadedb-session-id` header).
- Cypher queries are parameter-bound (workspace_id always present in `WHERE` clause as defence in depth).
- Byte-determinism adapter: `ORDER BY entity_uuid` on all queries that feed canonicalisation.

**Pros:**
- License-compatible across all Atlas tiers (Apache-2.0).
- Hermes-skill compatible (skill talks HTTP; sidecar runs the server).
- Rust ecosystem stable (`reqwest` is de-facto standard).
- Process isolation provides defence-in-depth security.
- Docker-Compose orchestration trivial for CI integration tests.
- Workspace-parallel projection (ADR-Atlas-007 Option A) directly supported by per-database concurrent writes.
- Per-workspace transaction primitive solves Q4 streaming-vs-batch cleanly.

**Cons:**
- HTTP overhead adds ~3 ms per query vs hypothetical embedded mode; absorbed at year-1 scale.
- Sidecar process adds operational complexity (2 processes instead of 1).
- JVM cold-start (~300 ms) blocks first projection; not a steady-state concern.
- New CI workflow `.github/workflows/atlas-arcadedb-smoke.yml` needed.

**Confidence:** **HIGH.**

### 3.2 Option B: ArcadeDB embedded mode + Rust↔Java FFI bridge (REJECTED)

**Mechanism:**
- ArcadeDB Java library embedded in `atlas-projector` process via JNI bridge (`jni-rs` or `J4RS` crate).
- No HTTP overhead.
- Single process.

**Pros:**
- Lowest possible latency (no HTTP).
- Single artefact for deployment.
- Marginal Docker-Compose simplification.

**Cons:**
- **BLOCKER: Hermes-skill incompatible.** Hermes-skill ships as npm package; bundling JVM is a non-starter (size + platform-binaries + license).
- **BLOCKER: Rust↔Java FFI complexity.** JNI lifetime management, exception conversion, thread attachment — Atlas has no in-house expertise.
- JVM crash crashes `atlas-projector` (no process isolation).
- `atlas-projector` RSS bloats by 200-300 MB.
- CI Rust image needs JVM installed (more maintenance).
- No mature Rust+ArcadeDB-embedded prior art to copy.

**Decision:** **REJECTED.** Two blockers (Hermes-skill, FFI complexity) and four major cons. The latency advantage is marginal.

### 3.3 Option C: FalkorDB SSPLv1 primary (i.e. flip DECISION-DB-4) (REJECTED)

**Mechanism:**
- Use FalkorDB as Layer-2 primary; defer ArcadeDB to fallback.
- Either negotiate commercial license OR open-source entire Atlas operational stack under SSPL §13.

**Pros:**
- FalkorDB Cypher is more mature (RedisGraph heritage).
- GraphBLAS theoretical edge for very-large-scale traversals.
- Mem0g has documented Redis-family integration.

**Cons:**
- **DECISIVE: SSPL §13 structurally incompatible with Atlas's open-core hosted-service tier** without per-deployment commercial-license negotiation (V2-α Welle 2 §2 + DECISION-DB-4). This is the same blocker that drove the V2-α flip.
- Atlas hosted-service tier directly triggers SSPL §13.
- FalkorDB requires separate Redis server (operational complexity comparable to ArcadeDB sidecar).
- Atlas's year-1 scale (10M events / workspace) does not need GraphBLAS-edge performance.

**Decision:** **REJECTED — DECISION-DB-4 CONFIRMED.** W16 surfaces no evidence to overturn V2-α Welle 2's license-comparative conclusion. FalkorDB remains the documented fallback (per spike §9 trigger thresholds T1-T2).

---

## 4. Decision

**Atlas adopts ArcadeDB Apache-2.0 + server mode + `reqwest` async Rust HTTP client + one-database-per-workspace pattern as the Layer-2 backend architecture for V2-β.**

Sub-decisions (binding for W17a/b/c):
1. **License + DB choice:** ArcadeDB Apache-2.0 PRIMARY (DECISION-DB-4 CONFIRMED).
2. **Deployment mode:** SERVER mode (Docker sidecar in production; Docker-Compose in CI). Embedded mode REJECTED on Hermes-skill + FFI blockers.
3. **Rust HTTP client:** `reqwest` with `rustls-tls` feature (async, tokio-aligned, ~2 MB binary cost).
4. **Database-per-workspace:** ONE ArcadeDB database per Atlas workspace. Cross-workspace data sharing is forbidden by V1 event model + enforced by isolation.
5. **Transaction model:** per-workspace atomic transactions via ArcadeDB HTTP `/api/v1/begin/{db}` + `/api/v1/commit/{db}`. Each projection cycle = one transaction per workspace.
6. **Byte-determinism adapter:** all queries that feed canonicalisation MUST include `ORDER BY entity_uuid ASC` (vertices) / `ORDER BY edge_id ASC` (edges). `@rid` is NOT a valid sort key for canonicalisation.
7. **Tenant isolation:** layered defence — per-database isolation (ArcadeDB native, primary) + application-layer workspace_id parameter binding (projector + Read-API + MCP, active enforcer) + Cypher AST validator (mutation hardening; does NOT enforce workspace_id presence). Operator runbook requirement: per-database-per-workspace deployment configuration (shared-database mode forbidden).
8. **`GraphStateBackend` trait:** sketched in spike §7 (~40 LOC); W17a writes the production trait + InMemoryBackend impl + ArcadeDbBackend stub.

---

## 5. Consequences

### 5.1 Accepted today

- ArcadeDB sidecar process required for any Atlas deployment beyond in-memory dev mode.
- ~300 ms cold-start latency on first projection (JVM warmup).
- ~3-5 ms per-query HTTP overhead vs hypothetical embedded mode.
- ~480 MB Docker image for the ArcadeDB sidecar.
- New CI workflow `.github/workflows/atlas-arcadedb-smoke.yml` (W17c).
- `reqwest` added to `crates/atlas-projector/Cargo.toml` (W17b).

### 5.2 Mitigated by this design

- **R-L-02 (FalkorDB SSPLv1 hosted-service exposure):** ELIMINATED by ArcadeDB Apache-2.0 primary.
- **ADR-Atlas-007 §6 Q1-Q5 (parallel-projection unknowns):** RESOLVED. Option-A workspace-parallel projection is directly supported by per-database concurrent writes.
- **Hermes-skill JVM-distribution blocker (V2-γ):** AVOIDED by server-mode (skill ships as JS-only HTTP client).
- **Tenant-isolation risk (Q3, SECURITY):** Defence in depth (Layer 1 per-database isolation primary; Layer 2 projector workspace_id binding active enforcer; Layer 3 AST validator hardens against mutation attacks) plus operator-runbook requirement (per-database-per-workspace deployment). Cross-tenant read leak prevented in correctly-configured deployments; shared-database misconfiguration breaks Layer 1 and is explicitly forbidden by runbook.
- **Byte-determinism preservation (Q9):** Adapter contract eliminates `@rid`-ordering risk; `BTreeMap` semantics carry over from InMemory backend.

### 5.3 Dependencies on W17 (driver implementation)

- **W17a** (Phase 9 — ADR-Atlas-011): writes the `GraphStateBackend` trait per spike §7 + InMemoryBackend wire-up + ArcadeDbBackend stub.
- **W17b** (Phase 9): full ArcadeDbBackend impl using `reqwest` + Cypher per this ADR. MUST pass cross-backend byte-determinism test (§4.9 adapter validation).
- **W17c** (Phase 9): Docker-Compose CI workflow + integration tests + benchmark capture.

### 5.4 Dependencies on counsel track (Nelson-led, parallel)

- **DECISION-COUNSEL-1 / SSPL §13 opinion:** raises DECISION-DB-4 confidence from MEDIUM-HIGH to HIGH on license-side.
- **EU-data-residency deployment validation:** raises confidence to HIGH on operational-side.
- Neither blocks W17a/b/c implementation; both block V2-β-1 ship-gate.

---

## 6. Open questions (for W17a planning + V2-γ tracking)

- **OQ-1:** Should W17a make the `GraphStateBackend` trait generic over the transaction handle (associated type `type Txn: WorkspaceTxn`) or use `Box<dyn WorkspaceTxn>` for object safety? Trade-off: associated type is more efficient but limits dynamic dispatch; `Box<dyn>` is more flexible but adds vtable overhead. W17a decides.
- **OQ-2:** How does the backend trait expose batch operations for the Option-A parallel projection's per-workspace batch upsert? Pure trait can have a `batch_upsert(vertices: &[Vertex], edges: &[Edge])` method; ArcadeDb impl maps it to a multi-statement Cypher transaction.
- **OQ-3:** Multi-region ArcadeDB replication for EU-data-residency: ArcadeDB native cluster vs application-layer dual-write. Defer to V2-γ when first EU customer signs.
- **OQ-4:** GraalVM native-image build of ArcadeDB sidecar (~50 MB vs ~480 MB). Defer to V2-γ resource-tier polishing.
- **OQ-5:** ArcadeDB authentication: HTTP Basic vs JWT bearer with per-tenant scoping. V2-β starts with Basic; V2-γ may want JWT.
- **OQ-6:** Mem0g (W18) integration with ArcadeDB Lucene-based vector indexes vs Mem0g's own vector store. Defer to W18 design (ADR-Atlas-012).

---

## 7. Reversibility

**Decision is MEDIUM-HIGH reversibility.**

- **License-side reversibility:** if counsel ruling reverses (T5 trigger), Atlas re-projects from `events.jsonl` (authoritative Layer 1) into FalkorDB. Cost: 1-2 sessions projector rewrite + replay; zero customer downtime via dual-write. The `GraphStateBackend` trait abstraction (spike §7) makes this re-implementation a swap of one trait impl, not a re-architecture.
- **Mode-side reversibility (server → embedded):** if W17c benchmarks fire the embedded-mode reconsideration threshold (>15 ms p99 read), a follow-on ADR documents the embedded-mode adoption. Cost: ~3-5 sessions (Rust↔Java FFI bridge implementation); incompatible with Hermes-skill so V2-γ distribution-channel design would need re-think.
- **HTTP-client-side reversibility (reqwest → other):** trivial (single dep swap; ~0.5 session).
- **Database-per-workspace pattern reversibility:** swapping to shared-database-with-workspace_id-column would lose Option-A workspace-parallel speedup but is implementation-localised. ~1-2 sessions if ever needed.

No technical debt incurred; reversibility paths documented per dimension.

---

## 8. Watchlist + review cadence

### 8.1 Specific tracking signals

- **ArcadeDB GitHub releases:** https://github.com/ArcadeData/arcadedb/releases — watch for 24.x → 25.x major bumps; verify HTTP API stability.
- **ArcadeDB license commitments:** Costa Group / Arcade Analytics Srl communications. Apache-2.0 mission statement remains intact at time of spike.
- **`reqwest` releases:** https://github.com/seanmonstar/reqwest/releases — tokio-version alignment.
- **FalkorDB releases:** https://github.com/FalkorDB/FalkorDB/releases — license pivot signals.
- **Counsel-track deliverables:** SSPL §13 opinion (DECISION-COUNSEL-1) — quarterly Nelson update.

### 8.2 Review cadence

- **Pre-W17a start:** verify ArcadeDB latest stable + `reqwest` latest stable; no breaking changes.
- **Post-W17c:** review benchmark numbers against §4.10 estimates; if ratio >10x worse, open follow-on ADR.
- **Post-V2-β-1 ship (W19):** review first customer-deployment telemetry against §9 trigger thresholds (T1-T5).
- **Quarterly:** re-verify §8.1 tracking signals; refresh this ADR if any signal materially changes.

### 8.3 Out-of-cadence triggers

Any one of the spike §9 thresholds T1-T5 firing automatically opens a follow-on ADR documenting the response (either fallback adoption or scope adjustment).

---

## 9. Decision log

| Date       | Event                                                  | Outcome |
|------------|--------------------------------------------------------|---------|
| 2026-05-12 | DECISION-DB-4 (V2-α Welle 2): ArcadeDB Apache-2.0 primary lock, MEDIUM-HIGH confidence. | License-side decision recorded. |
| 2026-05-13 | ADR-Atlas-007 §6 enumerates 5 architectural open questions for W17. | W16 spike scoped. |
| 2026-05-13 | ADR-Atlas-010 opened. W16 spike completes. Server mode + `reqwest` + one-database-per-workspace locked. DECISION-DB-4 CONFIRMED. | Architecture-side confidence raised to HIGH. W17a unblocked. |
| TBD        | W17a (ADR-Atlas-011): `GraphStateBackend` trait + InMemoryBackend wire-up + ArcadeDbBackend stub. | Trait shape validated; W17b unblocked. |
| TBD        | W17b: full ArcadeDbBackend impl. Cross-backend byte-determinism test passes. | Adapter contract validated; W17c unblocked. |
| TBD        | W17c: integration tests + Docker-Compose CI workflow + benchmark capture. | Performance estimates validated or embedded-mode threshold triggered. |

(Future quarterly refreshes append rows here.)

---

**End ADR-Atlas-010.**
