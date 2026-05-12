# V2-α DB Choice — ArcadeDB vs FalkorDB Comparative Spike

> **Status:** V2-α Welle 2 deliverable, 2026-05-12. Master-resident V2-α DB-choice decision source-of-truth.
> **Methodology:** public-knowledge-based comparative analysis. **No actual benchmarks executed in this spike** — recommendation based on documented features, license terms, architecture choices, and Atlas-specific suitability reasoning. If confidence emerges as LOW, a Welle 2b actual-benchmark harness spike would be commissioned before V2-α DB lock.
> **Driving decision:** `DECISION-DB-1` (Kuzu archived by Apple Oct-2025; ArcadeDB Apache-2.0 is candidate next-viable fallback) + Risk Matrix R-L-02 (FalkorDB SSPLv1 hosted-service exposure).
> **Counsel-disclaimer:** all license analysis in §2 is engineer-perspective. Counsel-validated opinion on SSPLv1 §13 + Apache-2.0 §4–§5 implications for Atlas's open-core hosted-service tier is on Nelson's parallel counsel-engagement track and is **pre-V2-α-public-materials blocking**.

---

## 0. Executive Summary

**Recommendation: ArcadeDB (Apache-2.0) as V2-α primary, FalkorDB as performance-validation fallback.** Confidence: **MEDIUM-HIGH**. This is a flip from the current master-plan direction (FalkorDB primary, ArcadeDB fallback), driven primarily by license-compatibility analysis for Atlas's open-core hosted-service monetization model.

**Decision-reversibility cost: MEDIUM.** ArcadeDB and FalkorDB both support openCypher subsets sufficient for Atlas's projection use-case. Switching between them post-Welle-3 (Projector skeleton) requires rewriting projector queries against the new DB's Cypher dialect but does NOT require schema-on-disk migration if Atlas re-projects from the authoritative Layer 1 (`events.jsonl`). The projector is by design replay-from-authoritative-events, which makes DB swap a re-projection operation, not a data-migration operation. Cost estimate: 1-2 sessions of projector rewrite if the choice is reversed post-Welle-3.

**What additional evidence would raise confidence to HIGH:**
1. Actual benchmark harness comparing both DBs on Atlas's projection workload (write-heavy idempotent upsert + read-side workspace-scoped traversal queries)
2. Counsel-validated opinion that SSPLv1 §13 indeed applies to Atlas's planned hosted-service tier
3. Operator-runbook validation of ArcadeDB deployment in a representative Atlas hosting scenario

**What would lower confidence to LOW:**
1. ArcadeDB's Cypher subset turns out to be substantially incomplete vs Atlas's projection requirements
2. Performance differential at workspace-scale (10M events / year-1 expected) makes ArcadeDB unviable
3. ArcadeDB undergoes a license-pivot or acquisition during V2-α that erases the Apache-2.0 advantage

---

## 1. Why This Spike

Per Master Plan §4 Top-5 V2-α Blocking Risks, **R-L-02 FalkorDB SSPLv1** is identified as pre-V2-α-lock blocking: SSPLv1 §13 (the "Service Source Code" clause) creates a commercial-license obligation for any party offering FalkorDB-derivative-functionality as a managed service. Atlas's open-core monetization plan (Master Plan §9 (GTM + Business Model, Monetization block)) explicitly includes a paid hosted-service tier — directly exposing Atlas to the SSPL §13 boundary.

The original Phase-1 plan named Kuzu (MIT-licensed) as the OSS fallback. Kuzu was **acquired by Apple in October 2025** and its public repository archived; Kuzu is no longer a viable fallback (`DECISION-DB-1`). ArcadeDB (Apache-2.0) emerged as the next-viable permissive-licensed graph-DB candidate.

The spike validates whether ArcadeDB can serve as primary (eliminating SSPL exposure entirely) or only as fallback (informing the V2-α lock decision and the Counsel-engagement licensing scope).

---

## 2. License Analysis

### 2.1 FalkorDB SSPLv1

**License:** Server Side Public License, version 1 (created by MongoDB Inc.; not OSI-approved as open source).

**Key contentious clause — SSPL §13 ("Offering the Program as a Service"):**

> *"If you make the functionality of the Program or a modified version available to third parties as a service, you must make the Service Source Code available via network download to everyone at no charge, under the terms of this License. Making the functionality of the Program or modified version available to third parties as a service includes, without limitation, enabling third parties to interact with the functionality of the Program or modified version remotely through a computer network, offering a service the value of which entirely or primarily derives from the value of the Program or modified version, or offering a service that accomplishes for users the primary purpose of the Program or modified version."*

"Service Source Code" is defined broadly as "the source code for the Program or modified version, and the source code for all programs that you use to make the Program or modified version available as a service."

**Atlas implication:** if Atlas operates a hosted Atlas service that uses FalkorDB internally to project the verifiable knowledge graph, SSPL §13 plausibly requires Atlas to release the ENTIRE operational stack (web frontend, MCP server, signer, witness, deployment infrastructure code) under SSPL. This is the documented chilling-effect of SSPLv1 vs more permissive AGPL: AGPL applies to derivative works; SSPL applies to all programs used to deliver the service.

**FalkorDB commercial license:** FalkorDB Inc. offers commercial licenses that exempt licensees from SSPL §13. Pricing is opaque (contact-sales model). Atlas would need to negotiate a commercial license pre-hosted-service-launch.

### 2.2 ArcadeDB Apache-2.0

**License:** Apache License 2.0 (OSI-approved as open source, widely used in commercial software).

**Key relevant clauses:**
- **§2** grants perpetual, worldwide, royalty-free patent + copyright license
- **§4** requires preservation of NOTICE file and copyright/license headers in redistribution
- **§5** explicitly allows commercial redistribution and use without imposing a copyleft obligation
- **§7** provides explicit patent grant + retaliation clause

**Atlas implication:** Atlas can freely integrate ArcadeDB into any tier (self-hosted, paid hosted-service, enterprise) without copyleft propagation. No commercial license negotiation required. The Apache-2.0 NOTICE-preservation obligation is satisfied by listing ArcadeDB in Atlas's dependency manifest (already required by Cargo / npm convention).

### 2.3 Hosted-Service Implications for Atlas Open-Core Model

| Scenario | FalkorDB SSPLv1 | ArcadeDB Apache-2.0 |
|---|---|---|
| Self-hosted Atlas (operator installs Atlas + FalkorDB locally) | Permitted; SSPL §13 not triggered because no "service to third parties" | Permitted, no obligations beyond NOTICE preservation |
| Atlas paid hosted-service tier (€10K–€100K ACV per Master Plan §9 (GTM + Business Model, Monetization block)) | Commercial license required from FalkorDB OR full Atlas operational stack must be SSPL-licensed | Permitted without obligations |
| Atlas Personal/Team hosted-sync (€5–€15/user/month) | Same: commercial license OR SSPL-licensed operational stack | Permitted without obligations |
| Enterprise EU-data-residency hosting | Same | Permitted |
| White-label / partner-hosted offerings (Hermes-skill ecosystem, Mem0 partnership, future Lyrie-ATP integration) | SSPL §13 propagates to ALL partners offering Atlas-as-a-service | Permitted; Apache-2.0 compatible with downstream commercial licensing |

**Net assessment:** SSPLv1 is structurally incompatible with Atlas's planned multi-tier hosted-service monetization without per-deployment commercial-license negotiation. Apache-2.0 is structurally compatible with every Atlas tier without negotiation.

### 2.4 License-Pivot Scenarios

| Scenario | Probability (5-year horizon) | Atlas mitigation |
|---|---|---|
| FalkorDB drops SSPL → Apache-2.0 / BSL with TIME limit | LOW-MEDIUM (no industry signal yet) | Re-evaluate; SSPL-pivots to BSL are common (HashiCorp, Elastic) but rarely fully open |
| FalkorDB drops SSPL → AGPLv3 | LOW | Atlas could comply with AGPL by open-sourcing Atlas-specific service-code; less hostile than SSPL §13 |
| FalkorDB pivots to closed-source commercial-only | LOW-MEDIUM | Fork last SSPL-licensed version under SSPL; or migrate to ArcadeDB at projection-replay cost |
| ArcadeDB pivots from Apache-2.0 to BSL / SSPL | LOW (mission statement explicit about Apache-2.0) | Fork last Apache-2.0 version; or evaluate Memgraph / Neo4j-community alternatives |
| Apple-style acquisition of ArcadeDB | LOW-MEDIUM | Same as Kuzu — fork before archive; spike doc serves as institutional memory of which fork to maintain |

**Net assessment:** both DBs carry pivot risk; ArcadeDB's Apache-2.0 mission statement + Costa-Group ownership structure (small-team OSS-native) is less exposed than FalkorDB's larger-commercial-entity structure.

---

## 3. Feature Parity for Atlas Use-Case

### 3.1 Cypher Subset Coverage

| Cypher feature | FalkorDB | ArcadeDB | Atlas projection needs it? |
|---|---|---|---|
| `MATCH` with patterns | Yes (full) | Yes (subset; documented limitations on multi-hop variable-length paths) | Yes — every projection query starts with MATCH |
| `CREATE` for nodes + edges | Yes | Yes | Yes — idempotent upsert pattern requires CREATE for new entities |
| `MERGE` for idempotent upsert | Yes | Yes | Yes — projection MUST be idempotent per Master Vision §5.2 |
| `SET` for property updates | Yes | Yes | Yes — author_did stamping on existing nodes |
| `WHERE` filters | Yes | Yes | Yes — workspace-scoped filtering |
| `RETURN` with projections | Yes | Yes | Yes — read-API queries |
| Aggregations (`count`, `collect`) | Yes | Yes | Yes — provenance-bundle aggregation |
| Path expressions `[*]` | Yes (good performance via GraphBLAS) | Yes (variable-length-path support documented; performance characteristics vary by depth) | V2-β — depth-capped read-API queries (`GET /related/:id?depth=N`) |
| Procedures (`CALL`) | Limited; no `apoc.*` namespace | Yes; supports user-defined functions | V2-β AST-validation explicitly bans `apoc.*` / `CALL db.*` (DECISION-SEC-4); both DBs adequate |
| Subqueries (`CALL { ... }`) | Limited | Yes | V2-β — useful but not blocking |
| Patterns in expressions | Yes | Yes | V2-β — useful for path-existence checks |

**Net assessment:** both DBs cover Atlas's projection + V2-β Read-API requirements. FalkorDB's Cypher implementation is more mature (RedisGraph-heritage); ArcadeDB's Cypher is added on top of a multi-model architecture and may have more rough edges. **This is the single largest open question requiring actual-benchmark validation.**

**Multi-model attack-surface note (Welle-3 / V2-β scope):** ArcadeDB exposes multiple query languages (Cypher, SQL, Gremlin, GraphQL). `DECISION-SEC-4` Cypher-passthrough hardening (AST validation, no `apoc.*`, no `CALL db.*`, parse-time depth caps, allow-list procedures) was specified against FalkorDB's Cypher-only surface. When ArcadeDB is primary, Welle 3 MUST constrain the driver configuration to Cypher-only unless a specific multi-model use case is justified — AND if other query surfaces are ever exposed via Read-API, DECISION-SEC-4 hardening MUST be independently enforced for each language's parse layer.

### 3.2 Property Graph Model

| Feature | FalkorDB | ArcadeDB |
|---|---|---|
| Node labels | Yes (single label per node, indexable) | Yes (multiple labels per node — "vertex types"); more flexible |
| Edge labels | Yes (single label per edge) | Yes (multiple labels per edge) |
| Properties (typed) | Yes (string, integer, float, boolean, list, vector) | Yes (string, integer, float, boolean, list, embedded, link, decimal, datetime) |
| Indexes (B-tree, full-text) | Yes (B-tree, fulltext, vector) | Yes (B-tree, fulltext, lucene-based) |
| Constraints (unique, mandatory) | Limited | Yes (mandatory, readonly, max, min, regex) |
| Schema enforcement | Schemaless by default; schema can be opted-in | Schema-required by default; schemaless mode also available |
| Atlas-specific: ability to stamp `{event_uuid, rekor_log_index, author_did}` on every node/edge | Yes (string properties) | Yes (string properties; `link` type also potentially relevant for cross-document refs) |

**Net assessment:** ArcadeDB's schema-required-by-default mode is a HIDDEN ADVANTAGE for Atlas's projector-state-hash CI gate: enforced schema makes deterministic projection-state computation more straightforward than schemaless mode.

### 3.3 Idempotent Upsert Pattern

Atlas projection requires: given the same input event sequence, the projector MUST produce byte-identical graph state. The pattern is:

```cypher
MERGE (n {entity_uuid: $event.entity_uuid})
  ON CREATE SET n.created_at = $event.ts, n.event_uuid = $event.event_uuid, ...
  ON MATCH SET n.updated_at = $event.ts, n.last_event_uuid = $event.event_uuid, ...
```

Both DBs support `MERGE ... ON CREATE / ON MATCH`. ArcadeDB additionally supports atomic upsert via SQL-style `INSERT ... ON CONFLICT` (multi-model heritage). FalkorDB's MERGE is purely Cypher.

**Net assessment:** both adequate; ArcadeDB's multi-statement-language flexibility could be useful for Welle-3 projector codegen, but not a deciding factor.

### 3.4 Multi-Tenant Isolation

| Mechanism | FalkorDB | ArcadeDB |
|---|---|---|
| Database-per-workspace | One graph per Redis database; multiple Redis databases = multiple workspaces | One database file per workspace; ArcadeDB native database isolation |
| Resource limits per tenant | Redis-level memory limits; FalkorDB-level limits via Redis-Module config | Database-level config (heap, page cache, transaction size) |
| Workspace authentication | Redis-AUTH at connection level | ArcadeDB-native user/role per database |
| Cross-tenant query prevention | Database-isolation; queries are auto-scoped | Same; database-isolation prevents cross-tenant queries |

**Net assessment:** both DBs provide adequate multi-tenant isolation. ArcadeDB's native user/role-per-database may be slightly more granular for Atlas's enterprise tier needs.

### 3.5 Schema Determinism (critical for projector-state-hash CI gate)

The V2-α projector-state-hash CI gate (per Master Plan §3 + DECISION-ARCH-1) requires that the FULL graph state — node IDs, edge IDs, properties, labels — be deterministic and serializable to a canonical byte representation for hashing.

| Determinism concern | FalkorDB | ArcadeDB |
|---|---|---|
| Internal node-ID generation | Auto-increment counter per database; deterministic if events processed in identical order | Auto-increment counter per type; deterministic if events processed in identical order |
| Property ordering at storage | Map representation; iteration order may vary between versions | Schema-required mode enforces explicit field ordering |
| Serialization for hash computation | Requires custom dump-format tooling; FalkorDB has no built-in deterministic-export | Built-in deterministic dump *ordering* via `SELECT * FROM ... ORDER BY @rid` (record-ID), but `@rid` is insert-order, NOT a logical identity anchor (see note below) |
| Cross-version determinism | Storage format documented but not strictly stable across FalkorDB minor versions | Storage format documented; deterministic-export ordering is implementation-stable but not formally specified as wire-stable |

**Important `@rid` correctness caveat:** `@rid` is an auto-increment counter scoped to a bucket. Two projector runs producing the same *logical* state via different historical insertion orders (e.g., replay-from-`events.jsonl` into a fresh DB after a swap, or post-bucket-rebalancing) will produce different `@rid` values even for logically identical nodes — different bytes, different hash. The canonical hash MUST be computed over the **logical identifier** (e.g., `entity_uuid`, blake3 of `(workspace_id, event_uuid, kind)`), NOT over `@rid`. `@rid` is useful only as a dump *ordering* helper once the logical-identity-canonicalisation spec is in place. The Welle-3 canonicalisation byte-pin spec (per Q-2-2) must define the sort key as the logical identifier; this requirement applies regardless of DB choice.

**Net assessment:** **ArcadeDB has a modest advantage** — its schema-required mode enforces field ordering, which simplifies the property-ordering half of the canonicalisation spec. The `ORDER BY @rid` deterministic dump helps only when the same DB instance is re-queried; it does NOT replace the need for logical-identifier-based canonicalisation. FalkorDB would require Atlas to build the full canonicalisation tooling; ArcadeDB lets Atlas build *less* (property-ordering is free) but the logical-identifier sort layer is required either way.

---

## 4. Performance Characteristics

### 4.1 FalkorDB GraphBLAS Backend

FalkorDB's primary performance differentiator is its use of [SuiteSparse:GraphBLAS](https://github.com/DrTimothyAldenDavis/GraphBLAS) (Tim Davis et al.) — a high-performance sparse-matrix linear-algebra library for graph algorithms. This makes FalkorDB exceptionally fast for:
- Matrix-style graph algorithms (PageRank, BFS, shortest-path) at large scale
- Bulk graph operations (e.g., "find all neighbors of N nodes simultaneously")
- Graph-traversal-heavy read workloads

Documented benchmark claims (FalkorDB public materials): "100x faster than Neo4j for certain traversal queries", "sub-millisecond p99 traversal latency". These claims are not independently verified for Atlas's workload; per `DECISION-DB-2` (Phase 2 Database critique), unsourced performance claims are credibility tripwires.

**Atlas-workload relevance:** Atlas's projection workload is WRITE-HEAVY (idempotent upsert from events.jsonl) at projection time, and READ-HEAVY (workspace-scoped traversal) at API time. GraphBLAS shines for read-heavy traversals at scale, but Atlas's expected scale (year-1 estimate: ~10M events / workspace) is well below GraphBLAS's "shines" threshold.

### 4.2 ArcadeDB Bucket Architecture

ArcadeDB uses a bucket-based on-disk storage architecture inherited from OrientDB's design. Records are grouped into buckets per type; indexes are B-tree based; transactions are ACID via WAL + bucket-locking.

Documented benchmark claims (ArcadeDB public materials): "millions of records per second insert"; "memory-mapped file performance for read-heavy workloads".

**Atlas-workload relevance:** ArcadeDB's strengths are balanced performance across write + read + multi-model queries. For Atlas's workspace-scoped Read-API depth-capped queries (`GET /related/:id?depth=3..5`), ArcadeDB should deliver acceptable latency at the year-1 scale.

### 4.3 Atlas-Workload Estimates

| Workload pattern | FalkorDB expected performance | ArcadeDB expected performance | Atlas-acceptable at year-1 scale? |
|---|---|---|---|
| Write-side: idempotent upsert (1 event → 1-3 graph mutations) | Sub-ms per upsert at workspace scale | Sub-ms per upsert at workspace scale | Both acceptable |
| Read-side: GET /entities/:id with provenance bundle | Sub-ms | Sub-ms | Both acceptable |
| Read-side: GET /related/:id?depth=3 (depth-capped traversal) | Sub-ms to single-digit-ms (GraphBLAS edge case) | Single-digit to double-digit ms | Both acceptable |
| Read-side: GET /timeline/:workspace?from=&to= (bi-temporal range query) | Single-digit ms (B-tree on ts) | Single-digit ms (B-tree on ts) | Both acceptable |
| Projection rebuild from 10M events (RTO scenario per DECISION-ARCH-1) | Estimated 30-60 min single-threaded | Estimated 30-90 min single-threaded | Both require parallel-projection plan for >50M scale |
| Projector-state-hash CI gate (full graph hash computation) | Requires custom dump-canonicalisation tooling | Built-in deterministic dump via SQL | ArcadeDB has lower implementation cost here |

**Net assessment:** for Atlas's expected year-1 scale and workload patterns, both DBs deliver acceptable performance. FalkorDB has theoretical edge for very-deep traversal at >10M-node scale, but that's not Atlas's year-1 reality.

---

## 5. Operational Considerations

### 5.1 Deployment

| Aspect | FalkorDB | ArcadeDB |
|---|---|---|
| Runtime | Redis server + FalkorDB module (`.so` / `.dylib`) | Standalone Java app (JVM); native-image (GraalVM) build available |
| Container image | Official `falkordb/falkordb` Docker image | Official `arcadedata/arcadedb` Docker image |
| Resource footprint (idle) | Redis baseline ~10 MB + FalkorDB module ~50 MB | JVM baseline ~200 MB; GraalVM-native ~50 MB |
| Embedded mode | No (Redis is server-only) | Yes (Java library; can be embedded in Atlas server process) |
| Cluster mode | Redis Cluster | ArcadeDB native cluster (HA mode); also embedded-replica mode |

**Atlas implication:** **ArcadeDB's embedded mode is a strong feature for the self-hosted Atlas tier** (operators run `atlas-web` + embedded ArcadeDB in a single process, no separate database server). FalkorDB requires a separate Redis instance, raising the self-hosted deployment complexity.

### 5.2 Embedded vs Server Mode

For Atlas's planned tiers:

| Tier | FalkorDB option | ArcadeDB option |
|---|---|---|
| Self-hosted single-user | Redis + module (separate process) | Embedded JVM library OR standalone server |
| Personal/Team hosted-sync | FalkorDB managed service (FalkorDB Cloud) OR self-managed Redis | ArcadeDB Cloud OR self-managed standalone |
| Enterprise EU-data-residency | Self-managed Redis cluster | Self-managed ArcadeDB HA cluster |

**Net assessment:** ArcadeDB's embedded mode simplifies the self-hosted tier — Atlas can ship as a single-process server. FalkorDB requires the operator to also manage Redis.

### 5.3 Backup / Recovery / Determinism Verification

| Aspect | FalkorDB | ArcadeDB |
|---|---|---|
| Backup mechanism | Redis RDB snapshot + AOF append-log | ArcadeDB binary backup OR SQL `DUMP DATABASE` (text format) |
| Point-in-time recovery | Via AOF replay | Via WAL replay |
| Atlas trust property: rebuild from Layer 1 events.jsonl (authoritative) | Re-project events.jsonl → new graph (DB-independent) | Re-project events.jsonl → new graph (DB-independent) |
| Projector-state-hash verification (CI gate) | Custom dump-canonicalisation tooling required | Built-in deterministic export via `SELECT * FROM V ORDER BY @rid` + similar for edges |

**Atlas implication:** the bedrock V1 trust property (rebuild from authoritative `events.jsonl`) is preserved either way — both DBs are derivative projections of Layer 1. The CI-gate-implementation cost is meaningfully lower for ArcadeDB due to its built-in deterministic-export.

### 5.4 Observability

Both DBs expose Prometheus-compatible metrics. Both have logging (FalkorDB inherits Redis logging; ArcadeDB has built-in structured logging). Both integrate with standard observability stacks (Grafana, Loki, OpenTelemetry).

**Net assessment:** parity.

---

## 6. Maturity + Vendor Risk

### 6.1 FalkorDB Status

- **Origin:** Forked from RedisGraph in 2024 when Redis Inc. discontinued RedisGraph as a Redis-shipped module.
- **Ownership:** FalkorDB Inc. (independent company; ex-RedisGraph team continued development under new branding)
- **Commercial backing:** Series A funding announced 2024; commercial-license sales pipeline.
- **Community:** active GitHub (~2K stars at time of spike), regular releases, growing user base inheriting RedisGraph adopters.
- **Production deployments:** RedisGraph-era deployments are migrating to FalkorDB; new FalkorDB adoptions are growing.

**Vendor risk assessment:** MEDIUM. Independent company with VC funding can pivot license, go closed-source, or pivot to a different revenue model. The SSPL choice is itself a defensive moat against AWS-style cloud-vendor competition — a classic VC-backed-OSS pattern that occasionally pivots further (Elastic → SSPL, HashiCorp → BSL, MongoDB → SSPL).

### 6.2 ArcadeDB Status

- **Origin:** Founded by Luca Garulli (also founder of OrientDB; OrientDB heritage codebase + lessons-learned)
- **Ownership:** Initially Costa Group (Italian small-team OSS company); recently transitioned to Arcade Analytics Srl
- **Commercial backing:** Self-funded; smaller team than FalkorDB; ArcadeDB Cloud commercial offering
- **Community:** active GitHub (~1.4K stars at time of spike), regular releases, OrientDB community migrating to ArcadeDB after OrientDB's discontinuation
- **Production deployments:** OrientDB-era deployments are migrating to ArcadeDB; new ArcadeDB adoptions in EU enterprise space

**Vendor risk assessment:** MEDIUM-LOW. Smaller team = lower acquisition target; Apache-2.0 license commitment is stated as mission; OrientDB heritage suggests resilience-through-pivots (OrientDB itself was acquired by Lightbend/CallidusCloud and discontinued, with the team founding ArcadeDB as continuation).

### 6.3 Acquisition / Pivot Risk Assessment

Kuzu's October-2025 acquisition by Apple set a recent precedent that single-team OSS graph DBs are acquisition targets. Both FalkorDB and ArcadeDB carry some acquisition risk, with mitigation paths differing:

| Scenario | FalkorDB mitigation | ArcadeDB mitigation |
|---|---|---|
| Acquisition closes-source | Atlas forks last SSPL version under SSPL (operational continuity but no upstream); migration to ArcadeDB possible at projection-replay cost | Atlas forks last Apache-2.0 version under Apache-2.0 (operational continuity, can continue maintaining); migration to next-tier Apache-2.0 alternative possible |
| Acquisition pivots commercial-only | Same | Same |
| Acquisition continues OSS but adds vendor lock | Atlas evaluates whether new ownership maintains Atlas-required features; SSPL doesn't change | Atlas evaluates new direction; Apache-2.0 license guarantees Atlas can use existing version forever |

**Net assessment:** ArcadeDB's Apache-2.0 license provides STRUCTURAL mitigation against acquisition risk that FalkorDB's SSPL does not. Apache-2.0 grants Atlas the perpetual right to use any version released before acquisition; SSPL also grants this right but with the §13 commercial-license burden continuing.

---

## 7. Atlas-Specific Decision Factors

### 7.1 Projection Determinism Compatibility (canonicalisation byte-pin)

Per `DECISION-ARCH-1` triple-hardening, the V2-α Projector must:
1. Use a canonicalisation byte-pin spec (analog to V1's `signing_input_byte_determinism_pin`)
2. Emit `ProjectorRunAttestation` events into Layer 1 binding `(projector_version, head_hash) → graph_state_hash`
3. Support parallel-projection design for >10M event scenarios

**FalkorDB:** custom canonicalisation tooling required for `graph_state_hash` computation across (a) logical-identifier sort, (b) property ordering, (c) byte-stable serialization. The DB's internal storage format is not API-stable across versions; canonicalisation logic must abstract over storage entirely.

**ArcadeDB:** schema-required mode handles (b) property-ordering as a built-in property; (a) and (c) are still custom Atlas tooling. The `ORDER BY @rid` dump ordering is useful within a single DB instance but is NOT a substitute for logical-identifier canonicalisation (see §3.5 important caveat on `@rid` insert-order semantics).

**Net assessment:** ArcadeDB reduces the *property-ordering* half of the canonicalisation work because the schema enforces explicit field order. The *logical-identifier* sort layer is required regardless of DB choice — Welle 3 must build it either way. The advantage is qualitative ("less Atlas-bespoke canonicalisation code"), not a specific percentage; exact line-count delta unquantified until Welle-3 design surfaces the actual canonicalisation spec.

### 7.2 author_did Property-Stamping (Welle 1 dependency)

Per V2-α Welle 1 (`crates/atlas-trust-core/src/agent_did.rs`), every Layer-2 graph node + edge produced by the Projector MUST stamp `{event_uuid, rekor_log_index, author_did}` as properties.

| DB | Implementation pattern |
|---|---|
| FalkorDB | `SET n.event_uuid = $event.event_uuid, n.rekor_log_index = $event.rekor_log_index, n.author_did = $event.author_did` |
| ArcadeDB | Same Cypher syntax OR via SQL `UPDATE V SET event_uuid=?, rekor_log_index=?, author_did=? WHERE @rid=?` |

**Net assessment:** parity. Both DBs support the stamping pattern.

### 7.3 ProjectorRunAttestation Event Hooks

Per `DECISION-SEC-2`, every projector run emits a signed `ProjectorRunAttestation` event into Layer 1 asserting `(projector_version, head_hash) → graph_state_hash`.

This emission happens OUTSIDE the DB (in `atlas-projector` Rust code that reads events.jsonl and writes both Layer 2 graph and Layer 1 attestation). Neither DB is on the critical path for this event flow — the Projector orchestrates it.

**Net assessment:** parity. Both DBs are equivalent for ProjectorRunAttestation hook integration.

### 7.4 V2-β Mem0g Cache Integration

V2-β (Welle 4-5 candidate) integrates Mem0g as Layer-3 semantic cache. Mem0g indexes graph content from Layer 2 (FalkorDB or ArcadeDB) and produces embeddings.

| Aspect | FalkorDB | ArcadeDB |
|---|---|---|
| Native vector support | Yes (FalkorDB has built-in vector indexes) | Yes (ArcadeDB has Lucene-based vector indexes via integration) |
| Mem0g connector | Documented Mem0g integration with Redis-family DBs | No documented Mem0g connector at time of spike; would require Atlas-built adapter |

**Net assessment:** FalkorDB has a Mem0g integration advantage. ArcadeDB would require Atlas to build a Mem0g-to-ArcadeDB adapter (~1-2 sessions). NOT a deciding factor — the adapter is one-time cost.

### 7.5 V2-γ Federation-Witness Property-Visibility

Per Master Vision §5.6, regulator-witness federation requires that the regulator's `witness_kid` be permanently visible in Layer 1 (Rekor-anchored). The Layer-2 graph projection may optionally include witness-cosignature-status as a node property (e.g., "this event has 2/3 regulator-witness signatures").

| Aspect | FalkorDB | ArcadeDB |
|---|---|---|
| Property-update concurrent with read | Yes (Redis ACID + module-level locks; graph-level lock granularity may block readers under high concurrency) | Yes (ArcadeDB ACID transactions + bucket-level MVCC; reader-writer non-blocking — marginal advantage for V2-γ witness-cosignature workloads under high concurrent read load) |
| Querying "events with regulator-witness cosignature ≥ N" | Cypher: `MATCH (n {has_regulator_cosig: true}) WHERE size(n.witness_kids) >= 2 RETURN n` | Same syntax (Cypher); also SQL: `SELECT * FROM V WHERE has_regulator_cosig = true AND witness_kids.size >= 2` |

**Net assessment:** parity.

---

## 8. Recommendation

### 8.1 Primary Choice + Confidence Level

**Primary: ArcadeDB (Apache-2.0).** Confidence: **MEDIUM-HIGH**.

**Rationale (weighted by deciding factor):**

| Factor | Weight | Winner | Justification |
|---|---|---|---|
| License (open-core hosted-service compatibility) | **HIGH** | ArcadeDB | SSPLv1 §13 is structurally incompatible with Atlas's planned hosted-service tier without per-deployment commercial-license negotiation. Apache-2.0 is structurally compatible. This factor alone is a strong primary-choice driver. |
| Projector-state-hash determinism cost (Welle 3 dependency) | MEDIUM | ArcadeDB (marginal) | Schema-required mode handles property-ordering half of canonicalisation as built-in; logical-identifier sort layer required regardless of DB (per §3.5 caveat on `@rid` insert-order semantics). FalkorDB requires Atlas to build both halves. |
| Self-hosted-tier deployment simplicity | MEDIUM | ArcadeDB | Embedded mode (Java library) lets self-hosted Atlas ship as a single-process server. FalkorDB requires separate Redis server. |
| Cypher subset maturity | MEDIUM | FalkorDB (marginal) | FalkorDB's openCypher implementation is RedisGraph-heritage; ArcadeDB's Cypher is more recent. Risk: ArcadeDB Cypher subset may have rough edges. Mitigates: Atlas projection queries use Cypher subset documented in §3.1 (validated as supported by both). |
| Read-side traversal performance at large scale | LOW (year-1) → MEDIUM (year-2+) | FalkorDB | GraphBLAS edge at very-large-scale traversals. Not Atlas's year-1 workload. |
| Mem0g integration cost | LOW | FalkorDB (marginal) | Atlas-built ArcadeDB-Mem0g adapter is ~1-2 sessions one-time cost. |
| Vendor / acquisition risk | LOW | ArcadeDB (marginal) | Apache-2.0 license + smaller-team-non-VC ownership = lower acquisition risk + stronger fork-defence. |
| Schema enforcement for projection determinism | MEDIUM | ArcadeDB | Schema-required mode is hidden advantage for deterministic projection. |

**Weighted conclusion:** ArcadeDB wins by license (decisive) + projection-determinism cost (significant) + deployment simplicity. FalkorDB wins by Cypher maturity + theoretical at-scale-traversal performance. The decisive factors lean toward ArcadeDB for Atlas's open-core, write-heavy-projection, EU-regulated-GTM positioning.

### 8.2 Fallback Path

**Fallback: FalkorDB.** If post-Welle-3 implementation surfaces Cypher-subset incompatibilities in ArcadeDB OR year-2 scale traversal performance becomes a Read-API blocker, Atlas re-projects from `events.jsonl` (authoritative Layer 1) into FalkorDB.

**Re-projection cost:** 1-2 sessions of projector rewrite (Cypher dialect adjustment) + replay of all historical events into the new DB. Customer downtime = zero if Atlas does dual-write during transition (write to BOTH DBs, switch reads at customer's pace).

The re-projection-not-data-migration property is preserved by Atlas's Three-Layer Architecture (per Master Plan §3): Layer 2 is derivative of Layer 1, so swapping Layer 2 DBs is a re-derivation operation, not a data-loss-risk operation.

### 8.3 Decision-Reversibility Cost Analysis

| Reversal scenario | Cost | Customer impact |
|---|---|---|
| Switch ArcadeDB → FalkorDB pre-Welle-3 (no production data yet) | Negligible (text update to plan docs + master plan) | None |
| Switch ArcadeDB → FalkorDB during Welle-3 implementation | 0.5-1 session (projector codegen swap) | None |
| Switch ArcadeDB → FalkorDB post-V2-α launch (production data exists) | 1-2 sessions projector rewrite + replay-from-events.jsonl | Zero downtime via dual-write |
| Switch FalkorDB → ArcadeDB post-V2-α launch (if we had picked FalkorDB) | Same cost (1-2 sessions); same approach (replay-from-events.jsonl) | Zero downtime via dual-write |

**Net assessment:** decision is MEDIUM-reversibility throughout V2-α. The asymmetric cost is the licensing burden (SSPL §13 commercial-license negotiation for hosted-service) — that cost is incurred only if FalkorDB is chosen AND hosted-service launches. ArcadeDB primary eliminates this burden entirely.

### 8.4 What Additional Evidence Would Raise/Lower Confidence

**Raise to HIGH:**
1. **Actual benchmark harness execution** (Welle 2b candidate, ~1-2 sessions) comparing both DBs on representative Atlas projection workload — write-heavy upsert + workspace-scoped traversal at 1M / 10M / 100M event scales. Expected outcome: both DBs hit Atlas's year-1 latency targets; ArcadeDB Cypher subset validated end-to-end.
2. **Counsel-validated SSPL §13 opinion** confirming the engineer-perspective license analysis. Expected outcome: counsel confirms SSPL §13 applies to Atlas's planned hosted-service tier.
3. **Operator-runbook validation** of ArcadeDB deployment in EU-data-residency hosting scenario (1 session).

**Lower to LOW:**
1. ArcadeDB Cypher subset turns out to lack a Cypher feature Atlas's projector needs (e.g., variable-length-path support breaks at depth > 3 in workspace-scoped queries).
2. ArcadeDB year-1 scale performance fails Atlas's latency targets (single-digit-ms write, sub-100ms-p99 read).
3. ArcadeDB undergoes license-pivot or acquisition before V2-α launch.

---

## 9. Open Questions for V2-α Welle 3 (and Future Wellen)

> **Numbering note:** `Q-2-X` = open questions raised by Welle 2 (this spike). The `Q-3-X` series in `.handoff/decisions.md` refers to prior Phase-3 strategic open items — different namespace, no relation to Welle 2 numbering.

- **Q-2-1:** Should Welle 3 (Projector skeleton) be DB-agnostic via a small Cypher-driver abstraction, or DB-specific to the locked choice? Tradeoff: abstraction layer adds ~200 LOC but preserves reversibility; DB-specific is simpler but locks the choice.
- **Q-2-2:** What is the minimum-viable canonicalisation byte-pin spec for projector-state-hash, and does ArcadeDB's `ORDER BY @rid` deterministic dump satisfy it? (Welle 3 design-doc scope.)
- **Q-2-3:** Should the V2-α plan-doc commit to an actual-benchmark harness Welle (Welle 2b) before locking the DB choice, or accept the public-knowledge-based recommendation? (Nelson decision.)
- **Q-2-4:** Does ArcadeDB's embedded mode work cleanly with Atlas's Rust runtime? (Java embedding from Rust is non-trivial; this may push Atlas toward ArcadeDB-server-mode even for self-hosted tier.)
- **Q-2-5:** When V2-β Mem0g integration arrives, will Atlas build a custom ArcadeDB-Mem0g adapter, or use Mem0g's API with Atlas-side caching only? (Scope decision for Welle 4-5.)
- **Q-2-6:** Should the Welle 3 Projector emit `ProjectorRunAttestation` events into Layer 1 PER-PROJECTION-RUN (every poll-cycle) or PER-WORKSPACE-COMMIT (only when state-hash changes)? (Latency vs storage tradeoff; Welle 3 design scope.)

---

## 10. Reference Pointers

| Concept | Source-of-truth |
|---|---|
| Master Plan §3 Three-Layer Trust Architecture (Layer 2 DB choice) | `docs/V2-MASTER-PLAN.md` |
| Master Plan §4 Top-5 V2-α Blocking Risks (R-L-02) | `docs/V2-MASTER-PLAN.md` |
| Master Plan §6 V2-α Foundation (Welle 2 = this spike) | `docs/V2-MASTER-PLAN.md` |
| Master Vision §5 Three-Layer Architecture (Layer 2 detail) | `.handoff/v2-master-vision-v1.md` |
| Master Vision §7.2 Graph DB landscape (Phase 2 Database update) | `.handoff/v2-master-vision-v1.md` |
| `DECISION-DB-1` (FalkorDB Fallback Re-Plan, Kuzu archived) | `.handoff/decisions.md` |
| `DECISION-DB-2` (FalkorDB Performance Claims Honesty) | `.handoff/decisions.md` |
| `DECISION-ARCH-1` / `DECISION-SEC-2` (Projection determinism triple-hardening) | `.handoff/decisions.md` |
| V2-α Welle 1 Agent-DID Schema (author_did stamping requirement) | `crates/atlas-trust-core/src/agent_did.rs`, `.handoff/v2-alpha-welle-1-plan.md` |
| V2-α Welle 2 Plan-Doc (this spike's planning context) | `.handoff/v2-alpha-welle-2-plan.md` |
| Working Methodology Anti-Pattern Table | `docs/WORKING-METHODOLOGY.md` |
| FalkorDB project | https://falkordb.com |
| ArcadeDB project | https://arcadedb.com |
| SSPLv1 text | https://www.mongodb.com/licensing/server-side-public-license |
| Apache-2.0 text | https://www.apache.org/licenses/LICENSE-2.0 |
| GraphBLAS (FalkorDB backend) | https://github.com/DrTimothyAldenDavis/GraphBLAS |

---

**End of V2-α DB Comparative Spike.** Recommendation: ArcadeDB primary, FalkorDB fallback, MEDIUM-HIGH confidence. Decision is Nelson's; Welle 3 Projector skeleton design proceeds against the locked choice.
