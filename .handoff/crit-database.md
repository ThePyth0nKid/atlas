# Crit: database on Atlas V2 Vision

> **Scope:** Database / performance specialist critique of Atlas V2 Phase-1 Foundation Docs A–E, with primary focus on Doc B (Knowledge Graph Layer) §2.3, §2.5, §3.1, §3.8, §3.17, §3.18 and Doc D §4 (Knowledge Graph Tools) + cross-references to Doc C performance-risk entries R-A-01 / R-O-01 / R-O-03 / R-V-02 and Doc E Demo 5 Mem0g speed claim.
>
> **Stance:** Three-layer architecture (`events.jsonl` authoritative + FalkorDB projection + Mem0g cache) is intellectually clean. Whether it actually performs at production scale — and whether the chosen tech is the right call — is what this crit pressures.

---

## Stärken

1. **Trust-invariant cleanly isolates the DB choice.** Doc B §2.1 + §2.5 explicitly state Layer-2/3 are *derivative*, *rebuildable*, *never trust-authoritative*. This means a wrong FalkorDB pick is operationally painful but **not architecturally fatal** — we can swap graph engines without touching the cryptographic core. That's the single biggest architectural strength of this design and it must be preserved.

2. **Storage adapter abstraction is explicitly called out** (Doc B §2.3 last sentence + §4 V2-α acceptance "abstract storage adapter so Kuzu swap is configuration-time"). The team understood the SSPL trap before writing the doc. This is a meaningful piece of foresight versus just picking a graph DB and hoping.

3. **Three-layer hierarchy is the right shape.** Authoritative log + queryable graph + fast cache is the *correct* taxonomy for this problem class — it matches how event-sourcing systems at scale (LinkedIn, Confluent, EventStore) actually separate concerns. The labels are correct, even if some of the specific products inside each layer are negotiable.

4. **Doc C R-O-03 already names the projection-rebuild-cost-at-scale risk** and proposes the right mitigation pattern (snapshot checkpointing + incremental projection with a watermark). The team is not naive about the "10M events × full replay" failure mode — they listed it.

5. **Honest about Mem0g determinism limits.** Doc B §2.5 explicitly says Layer-3 rebuildability is *eventually* — not byte-identical across model upgrades. That honesty matters; many vendors lie about cache determinism. Pinning `embedding_model_id` + `summariser_model_id` in cache rows is the right hook.

6. **Bi-temporal mapping from native Atlas timestamps is solid.** Doc B §2.5 maps `(t_valid, t_invalid, t'_created, t'_expired)` to Atlas's four native timestamps without needing Graphiti. This removes a dependency for a critical schema property — well done.

7. **Doc D §4.4 caught the Kuzu acquisition.** Most teams would still cite Kuzu as the OSS fallback six months after the Apple acq. Doc D explicitly flags the status change in October 2025 and proposes ArcadeDB / Memgraph / HugeGraph as replacement candidates. This is what an actual landscape doc should do.

8. **Demo 5 (Doc E §5) is scrupulously labelled** with "benchmark numbers from Mem0g's Locomo paper (2025)" — not "Atlas's 91% latency reduction". The author understood the misattribution risk and put the honesty constraint into the demo spec. Phase 2 should keep that discipline.

---

## Probleme (by severity)

### CRITICAL

**P-CRIT-1: Sub-millisecond p99 traversal claim is unsourced and almost certainly wrong as stated.**

Doc B §2.3 cites "sub-ms p99 traversal vs Neo4j ~90ms" for FalkorDB. Doc D §4.3 repeats "sub-ms p99 traversal claim (vendor)". This claim:
- Has no source URL in either doc.
- Comes from FalkorDB's own marketing (GraphBLAS sparse-matrix backend benchmark) on **specific graph shapes** (small dense subgraphs, 1-2 hop traversal, no concurrent writers).
- Is dimensionally wrong as a generalisation — Neo4j 5.x with proper indexes does sub-ms point-lookups too; the 90ms-vs-sub-ms gap is almost always *cold-start* or *uncached deep-traversal* on specific datasets.
- Says nothing about p99 *write* latency (which is what matters for Atlas, where the projector is *write-heavy* on the FalkorDB side).
- Says nothing about p99 *concurrent-write contention* (which is what matters for multi-agent scenarios).

Treating this number as a load-bearing reason to pick FalkorDB is dangerous. If V2-α ships under the assumption "FalkorDB is sub-ms p99" and production shows 50-200ms p99 under realistic mixed read+write load on a 10M-node graph, the architecture decision is partly invalidated and the demo claims become embarrassing.

**Fix:** Doc B §2.3 must (a) replace the bare claim with a source-cited benchmark spec including graph size, read/write ratio, hop count, and concurrent-writer count; or (b) remove the claim entirely and replace with "graph-traversal performance to be benchmarked in V2-α; choice of FalkorDB vs ArcadeDB is partly contingent on results."

**P-CRIT-2: 91% Mem0g latency reduction claim is correct-but-irrelevant for Atlas's threat model.**

The 91% / 2.59s p95 / 68.4% LOCOMO numbers (Doc B §2.5, Doc D §2.1, Doc E §5) come from the Mem0g paper (arxiv 2504.19413). Those benchmarks:
- Measure agent-memory retrieval against the LOCOMO conversation-grounding benchmark (long conversations, ~1.4M tokens of context).
- Compare Mem0g vs **full-context-LLM-baseline** (re-prompt the entire conversation).
- Do **not** measure cache-coherency overhead, provenance-pointer plumbing overhead, projector-keep-up latency, or write-side end-to-end latency including Ed25519 signing.
- Are apples-to-apples vs OpenAI Memory / Mem0-vector / RAG-baseline, NOT apples-to-apples vs an Atlas-signed-event-with-Rekor-anchor pipeline.

Atlas's actual user-perceived latency is `signer_subprocess_spawn + Ed25519_sign + CBOR_canonical + JSONL_append + projector_consume + Mem0g_lookup`, not `Mem0g_lookup` alone. The 91% number measures *only the last term* of that expression. The actual Atlas+Mem0g cold-write-then-read latency could be dominated by `signer_subprocess_spawn` (R-O-01 explicitly calls this out as a known overhead) regardless of how fast Mem0g is.

**Fix:** Doc B §2.5 + Doc E Demo 5 must distinguish between "Mem0g cache hit latency (inherited from upstream benchmark)" and "Atlas end-to-end signed-write-then-retrieve latency (to be measured in V2-α benchmark suite)". Demo 5 storyboard should reframe as "fast retrieval given a warm cache" rather than implying Atlas writes are sub-2s end-to-end.

**P-CRIT-3: Projection-rebuild-cost at 10M+ events has no published target and no benchmark plan.**

R-O-03 names this risk but the only quantification is "establish: how many events/second can the projector process?" — i.e., **TBD**. Doc B §3.17 recommends option (a) hard reseed with "fast reseed (target <5min for 1M events) makes the operational cost tolerable".

Five minutes for 1M events = 3,333 events/sec sustained projector throughput. This is a non-trivial number:
- It requires the projector to verify Ed25519 signature (~30µs/sig on commodity Intel), recompute parent-hash-chain context, perform structured extraction, perform FalkorDB upsert (network roundtrip ≥100µs even local), and stamp provenance properties — call it ~300µs/event optimistically.
- 300µs × 1M = 5 minutes — exactly the target, with **zero slack** for I/O variance, FalkorDB transaction commit latency, or projector-process overhead.
- At 10M events the reseed is 50 minutes (still tolerable for enterprise RTO).
- At 100M events the reseed is 8.3 hours (**not** tolerable for enterprise RTO).
- These estimates assume *single-threaded* projector; parallel projection on a hash-chained DAG is non-trivial because event N+1 depends on event N's parent reference.

**Fix:** Doc C R-O-03 must add a quantified RTO target (e.g. "rebuild from snapshot ≤30min at 100M events") and a benchmark plan with explicit hardware and FalkorDB version pins. Doc B §3.17 must spell out the parallel-projection plan (sub-graph partitioning by workspace_id, watermark-per-partition) or accept that single-threaded reseed sets a hard adoption ceiling around 50M events.

### HIGH

**P-HIGH-1: Doc C R-A-01 (Projection Determinism Drift) detectability is rated LOW; no operational detection mechanism is specified for production drift.**

R-A-01 LOW-detectability + CRITICAL-impact is correctly flagged. The proposed mitigation is "CI gate: replay events.jsonl through projector → canonical hash → compare to pinned hash". This catches drift on **test corpora**. It does **not** catch:
- Drift introduced by FalkorDB upgrades that change upsert ordering (e.g., GraphBLAS internal hash buckets change between FalkorDB 4.x and 5.x).
- Drift introduced by environment-dependent float behaviour (no floats in the trust path today, but the moment Cypher computes any aggregate, this can leak).
- Drift introduced by time-dependent projector logic (e.g., a projector that stamps `projector_run.timestamp` and a downstream rule that uses it).
- Production drift between the running graph and what a freshly-replayed graph would produce — i.e., **silent divergence between Layer-1 and Layer-2 *in production***.

The CI gate is necessary but the production detection mechanism is missing. Standard pattern: a "shadow projector" runs continuously in parallel on a slow path, producing a state-hash every N events; if shadow-hash diverges from primary-hash, alert. Without this, R-A-01 drift can run for weeks (matching the risk description).

**Fix:** Doc B §2.5 add an explicit operational invariant — "Continuous Projection Audit": shadow projector + state-hash comparison on a configurable cadence (e.g. every 10K events or hourly). Surface drift detection to the auditor UI per Doc B §1c.

**P-HIGH-2: Bi-temporal range queries on FalkorDB have no published performance characterisation.**

Doc B §2.5 commits to bi-temporal `(t_valid, t_invalid, t'_created, t'_expired)` semantics derivable from native Atlas timestamps. Query patterns this enables:
- "What did we believe about entity X on date Y" — range scan over `t_valid <= Y AND (t_invalid > Y OR t_invalid IS NULL)`.
- "All facts that were retracted in window W" — range scan over `t_invalid IN W`.

On a property graph these are **NOT cheap**. They require:
- Index on `t_valid` and `t_invalid` (or composite). FalkorDB index support is documented but performance characteristics for range scans over multi-million-edge graphs are not.
- Cypher subset limitations: FalkorDB's Cypher does not implement all OpenCypher index hints. Some range queries may degrade to full scans.

Neo4j 5.x has dedicated temporal types + a range-index implementation refined over a decade. FalkorDB's temporal-query story (2026 vintage) is much younger. The doc treats bi-temporal as a checkbox feature; in practice it is a query-performance landmine if queries are not designed with the available indexes.

**Fix:** Doc B §2.5 must add a §2.5.1 "Bi-temporal query patterns + index plan" subsection specifying: which timestamps are indexed, which query patterns are supported with sub-second p95 on a 10M-edge graph, and which queries are accepted-to-be-slow (full-scan analytical queries run only via the audit endpoint).

**P-HIGH-3: Write amplification per event is unspecified and stacks dangerously.**

Doc C R-O-01 lists the write-side pipeline:
> "Every Atlas write performs: payload schema validation, Ed25519 signing via atlas-signer subprocess, CBOR canonicalisation, blake3 hash computation, parent-chain resolution, JSONL append, and eventual Rekor HTTP submission (async)."

V2 *adds*:
- Projector consume (verify signature again, extract entities, upsert to FalkorDB).
- Mem0g cache invalidate-or-update (re-embed touched entities).
- Witness cosignature flow (if `kind` policy requires it — Doc B §2.10).
- Cedar policy evaluation at write-time (Doc B §3.13).

Per-event "write amplification factor" (work done downstream per single user write) is conservatively **5–8×** in V2 vs V1. At 10K writes/sec this means:
- 10K Ed25519 signatures/sec (HSM throughput question — see P-HIGH-4 below)
- 10K Rekor submissions/sec — far above Sigstore's public-Rekor rate limit. Doc C R-O-01 names "tiered anchoring" as mitigation; that needs concrete batch sizing.
- 10K FalkorDB upserts/sec — FalkorDB has not published a peer-reviewed write-throughput benchmark at this rate.
- 10K Mem0g cache invalidations/sec — embedding model is the bottleneck (Sentence-Transformers ~50ms/embedding on CPU).

**Fix:** Doc C R-O-01 must publish a quantified per-stage write-budget table (µs per stage) and identify which stage saturates first at the V2 write SLO target (`p99 < 50ms at 1K concurrent writes/sec`). Doc B §2.5 must specify Mem0g invalidation strategy explicitly — invalidate-on-write vs lazy-invalidate-on-read vs eventual-consistency-with-watermark.

**P-HIGH-4: HSM-backed signing throughput is unaddressed for the multi-agent scenario.**

V1's hsm-feature uses `ATLAS_HSM_PKCS11_LIB` for the master seed + HKDF derivation. R-A-03 recommends HSM-backed agent keys for high-stakes identities. Commodity PKCS#11 HSMs (YubiHSM 2, Nitrokey HSM 2, AWS CloudHSM) deliver:
- YubiHSM 2: ~2,200 Ed25519 ops/sec (vendor spec, single device)
- Nitrokey HSM 2: ~50 Ed25519 ops/sec (similar profile, lower throughput)
- AWS CloudHSM: ~10,000-50,000 ops/sec (cluster, but $1.45/hr per instance + network latency)

The Doc C R-O-01 V2 write SLO of "p99 < 50ms at 1K concurrent writes/sec" implies 1K signatures/sec sustained. A YubiHSM 2 can do this for a single agent identity. **It cannot** if 10 agents each demand HSM-backed signing concurrently — the device serialises operations and at 2,200 ops/sec aggregate cap, latency under load spikes well past 50ms p99.

For multi-agent shared-memory (Atlas's V2 positioning) HSM-throughput is a hard ceiling. The Doc C R-A-03 mitigation "Hardware-backed key storage for high-stakes agent identities" should be re-scoped: HSM for *operator* keys (V1.10 wave-2 pattern), but *agent* keys default to in-process Ed25519 with HSM-optional for high-value agents — not HSM-by-default.

**Fix:** Doc B §2.7 must specify the default key-storage tier for agent DIDs (recommend: software keys with optional HSM upgrade) and Doc C R-A-03 must add an HSM-throughput risk note.

**P-HIGH-5: Read-side API cache strategy + provenance-aware caching is unspecified.**

Doc B §2.8 lists six read endpoints. None has a documented cache-TTL strategy. Naive caching is *wrong* here:
- `GET /api/atlas/entities/:id` — entity properties can change at any new event referencing the entity. TTL must be 0 or invalidation-driven.
- `GET /api/atlas/related/:id?depth=N` — invalidation on *any* event touching any node in the N-hop subgraph. At depth=5 and a hot entity this is catastrophic — most events invalidate.
- `GET /api/atlas/timeline/:workspace?from=…&to=…` — past time windows are immutable (events.jsonl is append-only). Aggressive caching is safe; future-extending windows must not be cached.
- `GET /api/atlas/audit/:event_uuid` — by construction immutable per `event_uuid`. Maximally cacheable.

Different endpoints want different TTL semantics; the doc does not address this. Worse: provenance-aware caching (per Doc B §2.8 trust property) means a cache that returns stale provenance pointers is a *correctness* bug, not just freshness.

**Fix:** Doc B §2.8 add a sub-section "Cache coherency model" specifying per-endpoint TTL, invalidation event (e.g. `events.jsonl` cursor advancing past T → invalidate entity caches for entities touched by events from prior cursor), and the "stale-provenance-is-incorrect" invariant.

**P-HIGH-6: ArcadeDB / Memgraph alternative evaluation is incomplete in Doc D.**

Doc D §4.4 names ArcadeDB / Memgraph / HugeGraph as Kuzu replacement candidates but offers no comparative evaluation. For V2-α to be a defensible architecture decision, a head-to-head matrix on (license, performance, SLSA-pedigree, operator-burden, community-size, ecosystem-tools) is required. Without it, the "abstract storage adapter so Kuzu swap is configuration-time" claim (Doc B §2.3) is unverifiable — we don't know what we'd actually swap to.

ArcadeDB (Apache-2.0, multi-model graph+document, Java/JVM stack, embeddable mode + server mode, Cypher + Gremlin + SQL) is the strongest candidate per license/feature; but it has weaker community than FalkorDB and the JVM operator-burden vs FalkorDB's Redis-derived single-binary model is real. Memgraph (BSL with eventual Apache-2.0 conversion, C++ implementation, Cypher-native) has better performance reputation but the BSL license has the same trap-shape as SSPL until the conversion fires.

**Fix:** Doc D §4 add a §4.5 comparative matrix: FalkorDB / Neo4j-Community / ArcadeDB / Memgraph on at minimum (license, embeddable? Cypher coverage %, claimed write throughput / read p99 with source, last 12mo release cadence, GitHub stars, GitHub Actions / SLSA-availability for releases). Doc B §2.3 reference that matrix instead of treating FalkorDB as the default.

### MEDIUM

**P-MED-1: FalkorDB write throughput at production scale is unbenchmarked publicly.**

FalkorDB's published benchmarks (GraphRAG-Bench #1, sub-ms p99 traversal claim) are *read-side*. Write throughput on FalkorDB at 10K events/sec is not in any peer-reviewed publication. FalkorDB inherits Redis's single-threaded command processing model; writes serialise per-graph. Multi-graph deployment (Doc B §3.10 recommendation (c)) sharding can parallelise across workspaces but not within a workspace.

**Fix:** V2-α plan should include a "FalkorDB write throughput characterisation" task — actual benchmark on representative event payloads, single-graph and multi-graph, before committing the architecture.

**P-MED-2: FalkorDB HA / backup / restore story is absent.**

Doc B §2.3 + Doc D §4.3 cover FalkorDB licensing and features but say nothing about:
- High availability — FalkorDB primary/replica config? Cluster mode? What's the failover RTO?
- Backup — what's the backup primitive? RDB snapshot like Redis? Continuous WAL? At 10M-node graph, snapshot cost?
- Restore — restore time from backup at 10M-node? Faster or slower than re-projecting from `events.jsonl`?

These are not glamorous architectural questions but they decide whether enterprise customers can buy. "We can always re-project from events.jsonl" only works if reseed time meets RTO (see P-CRIT-3).

**Fix:** Doc B §2.3 add a "Operational maturity" subsection: HA model, backup primitive, restore performance — with source URLs to FalkorDB docs or a "to be validated in V2-α" flag.

**P-MED-3: Index strategy on FalkorDB is unspecified.**

The read-side query patterns (Doc B §2.8) demand indexes:
- `GET /api/atlas/entities/:id` → index on entity stable-id (blake3 of first-seen `entity_key`).
- `?filter=author_did=…` → index on `author_did` property.
- `?filter=min_witness_count=…` → if witness_count is materialised as property, scalar index; if computed at query time, slow.
- `?filter=requires_anchor=true` → boolean index or partial index.
- `?filter=time_window=[…]` → range index on `event.timestamp`.
- Bi-temporal range queries (per P-HIGH-2) → range indexes on `t_valid` / `t_invalid`.

FalkorDB supports labeled property indexes but does not implement all OpenCypher index-hint semantics; in particular range-scan performance on indexed properties has variance. Without an explicit index plan, query latency will be unpredictable.

**Fix:** Doc B §2.8 add a "Required indexes" subsection listing the indexes the projector must create on FalkorDB schema initialisation, with a justification per query pattern.

**P-MED-4: Graphiti LLM-extraction cache invalidation is not in the design.**

Doc B §2.2 path (a) — "Cache LLM extractions keyed by event hash; rebuild reuses cache" — is named but its cache size, eviction policy, and invalidation strategy are unspecified. If Graphiti ever moves into the trust path (Doc B §2.2(c) is the *current* recommendation but §3.6 admits Phase-2 may overturn this), the LLM-extraction cache is itself a load-bearing artifact. Cache loss → rebuild requires re-running every LLM call, at production scale this is $1-10K of LLM spend just to reseed.

**Fix:** Doc B §2.2 add a "LLM-extraction cache durability" subsection if path (a) is preserved as a fallback: storage layer for the cache (Redis? SQLite? S3?), invalidation semantics, expected size growth per million events.

**P-MED-5: Mem0g embedding cost economics are missing.**

Doc B §2.5 says Mem0g indexes Layer-2 — it consumes the projected graph, not raw events. Standard embedding pipeline: `OpenAI text-embedding-3-small ($0.02/M tokens) OR open-weight bge-large (free, requires GPU)`. At 10M entities × ~500 tokens each = 5B tokens = $100 (OpenAI) or significant compute (self-hosted). This is recoverable cost in the rebuild scenario (Doc B §2.5: "rebuild from FalkorDB. Pure operational pain, no trust loss") but it should be quantified for V2-α budgeting.

**Fix:** Doc B §2.5 add a "Embedding cost model" line: per-entity embedding cost in tokens + dollar estimates at multiple scales.

**P-MED-6: Distributed `events.jsonl` (Doc B §3.18) consistency model is hand-waved.**

Doc B §3.18 recommends option (a) S3-backed per-workspace JSONL with the V1 mutex elevated to a distributed lock. The phrase "S3 conditional-writes give us atomicity" is correct for *single-object* updates but the per-workspace mutex pattern in V1 *appends* to a JSONL file — S3 doesn't support append; you have to read-modify-write the object, which is O(file_size) per append. At 10M events × ~2KB/event = 20GB per-workspace object — every append rewrites 20GB. That isn't viable.

Real options at scale:
- Many-small-objects pattern (one S3 object per event, indexed by ULID lexicographic key, prefix scan to reconstruct). Atomicity via S3 conditional-write per object. Reconstruction = `s3 ls` with prefix + concatenation. **This is the production pattern** and the doc does not describe it.
- Append-only log primitive (Kafka, Kinesis, Pulsar). Adds dependency.
- DynamoDB stream backing a JSONL projection. Adds dependency.

**Fix:** Doc B §3.18 either commits to many-small-objects S3 pattern (with explicit per-event-object schema) or explicitly defers distributed Layer-1 to a future scope and caps the SLO at single-host throughput.

### LOW

**P-LOW-1: Sigstore Rekor v2 migration is not addressed.**

Doc D §5.1 notes Rekor v2 GA in 2026 (tile-backed transparency log). Atlas V1 anchors against Rekor v1 with pinned ECDSA P-256 key. Migration to Rekor v2 changes the inclusion-proof format. This is a **V2-δ-or-later** concern but should be on the roadmap.

**P-LOW-2: Demo 5 (Mem0g speed) numbers in storyboard (1.44s / 17.12s / 91.6% Δ) should label "upstream Mem0g paper figures" inline in the visible UI, not just in the speaker-note footnote.**

Honesty discipline. Viewers screenshot the side-by-side bar; without inline attribution, the screenshot reads as "Atlas claims 91% latency win".

**P-LOW-3: Schema versioning across `projector_version` upgrades has no migration test fixture proposed.**

Doc B §3.17 picks option (a) hard-reseed. The CI for V2-α should include a "projector_version N → N+1 cold reseed" test fixture validating that (a) the new projector successfully reseeds a held-out events.jsonl, and (b) the state-hash on the new projector_version is reproducible across machines.

---

## Blinde Flecken

1. **No comparison of Atlas's projector to existing event-sourcing/projection libraries.** EventStore DB, Axon, Eventuate, Marten — these have all solved "deterministic projection from event log to read model" with decade-old patterns: snapshot stores, checkpoint cursors, watermark advancement, idempotent upsert protocols. Doc B treats projector design as net-new. Reuse of one of these patterns (or even of one of these libraries via a thin wrapper) would reduce R-A-01 detectability concerns substantially. **Phase 3 should evaluate.**

2. **No mention of CDC (Change Data Capture) from `events.jsonl` to FalkorDB.** The naive "tail / cron / event-stream" model (Doc B §2 diagram) is fine for v0 but real production event-sourcing systems use CDC primitives — Debezium for SQL, Kafka Connect for log → graph. Atlas could likely build a thin CDC connector that gives operators the *option* to use a Kafka topic as the projector input, decoupling write-side throughput from projector throughput.

3. **No discussion of read replicas for the FalkorDB layer.** At 10K reads/sec the read-side API becomes the bottleneck, not the write side. FalkorDB read-replica architecture is unaddressed. Standard mitigation: write → primary; read → replica with seconds-of-staleness tolerance per endpoint cache strategy (per P-HIGH-5).

4. **No analysis of `events.jsonl` compression / archival tiering.** At 10M events × 2KB/event = 20GB per workspace. S3 standard at $0.023/GB/mo = $5,520/yr for 1,000 workspaces. Mostly cold storage candidate after a window (Glacier, ~10× cheaper). But: archived events still need to be projectable on a full reseed. Cold-tier reseed cost is potentially extreme.

5. **No discussion of incremental projection idempotency under projector-process failure.** If the projector crashes mid-upsert on event N, on restart does it re-process event N? Idempotent upserts assume yes; partial-write detection assumes no. The trust gate requires *exactly-once semantics from the consumer's perspective*. This is not a trivial implementation — needs a transactional outbox or a 2-phase commit shape on FalkorDB side, neither of which Doc B specifies.

6. **No mention of FalkorDB's storage-engine bytes-on-disk efficiency.** GraphBLAS sparse-matrix format has unusual storage characteristics — the entire matrix lives in memory by design (it's a *Redis module*). At 10M nodes × 50M edges × ~256 bytes per row this is 12GB+ RAM. FalkorDB Cloud's pricing model (mentioned but not detailed in Doc D §4.3) is implicitly RAM-priced. Atlas at scale on FalkorDB = expensive RAM, not cheap disk.

7. **Vector-search overlap not addressed.** Mem0g is described as semantic+graph; a question Phase 2 should ask is whether Atlas needs a *separate* vector store (pgvector / Qdrant / Milvus / Weaviate) for semantic-similarity queries, or whether Mem0g's built-in vector primitives are sufficient. If a separate vector store is added, that's a **fourth layer** in the architecture — and Doc B explicitly markets the three-layer model.

8. **The `events.jsonl` blob/content-store separation (§3.3 GDPR) has unaddressed performance implications.** Resolving `content_pointer` for every projector pass means an extra storage round-trip per event. At 3,333 events/sec sustained projector throughput (per P-CRIT-3), the content-store must serve 3,333 reads/sec for the projector alone — and the content-store backend is not specified. If it's S3 (typical), p99 read latency 100-300ms per object means projector throughput *cannot* meet the 5-min-per-1M-events target unless content reads are batched or pre-staged.

9. **No mention of how Doc B's V2-β read-API rate-limits.** The `POST /api/atlas/query` Cypher endpoint with 5-30s timeout is a denial-of-service risk if not rate-limited. A malicious workspace user issues 1000 expensive queries → FalkorDB instance saturates → entire workspace is read-unavailable. Rate-limit + query-cost-estimation should be in the design.

10. **The "Mem0g vendor risk" mitigation (R-V-02 + Doc B §3.12) underweights the migration cost.** Replacing Mem0g with "our own embedding+graph-summarisation pipeline" sounds easy but is a 3-6 month effort minimum (embedding model selection, summarisation prompt engineering, infrastructure for both, eval harness against Mem0g's LOCOMO numbers). The mitigation as written is true-in-principle, not true-in-practice on a short clock.

---

## Konkrete Vorschläge

- **Doc B §2.3** — Replace the unsourced "sub-ms p99 traversal vs Neo4j ~90ms" claim with: "FalkorDB benchmarks GraphBLAS-backed read-traversal at sub-ms p99 on small dense subgraphs (source: FalkorDB GraphBLAS whitepaper, 2025). Atlas V2-α must independently benchmark write throughput, p99 read latency under concurrent mixed-workload, and bi-temporal range-scan performance against the actual Atlas projector workload before locking the storage adapter."

- **Doc B §2.5 (new subsection §2.5.1 "Bi-temporal query patterns + index plan")** — Add: (a) required FalkorDB indexes for the four bi-temporal coordinate properties; (b) target p95 latency for canonical query patterns at 10M-edge graph; (c) explicit "slow query" classification with which audit-only endpoints are accepted to be slow.

- **Doc B §2.5 (new subsection §2.5.2 "Production drift detection — continuous shadow projector")** — Address P-HIGH-1: spec a shadow projector that re-projects events on a lower-priority schedule, computes state-hash, compares to primary, alerts on divergence. This is the *production* analogue to the CI-gate proposed for R-A-01.

- **Doc B §2.8 (new subsection §2.8.7 "Cache coherency model")** — Per-endpoint TTL semantics, invalidation event (advancing `events.jsonl` cursor → invalidate entities touched), stale-provenance-is-correctness-bug invariant, recommended cache backend (Redis / in-process LRU). Also rate-limit policy for `POST /api/atlas/query`.

- **Doc B §2.7 + Doc C R-A-03** — Specify the default key-storage tier for agent DIDs (recommend: software keys at default, HSM-optional, HSM-mandatory for kinds flagged as high-stakes in workspace policy). Add HSM-throughput note (commodity HSMs ≤2,200 Ed25519 ops/sec; multi-agent concurrent HSM signing serialises).

- **Doc B §3.17** — Quantify the parallel-projection plan: partition by `workspace_id`, watermark-per-partition, expected speedup factor. Without this, single-threaded reseed at 100M events is 8+ hours and adoption ceiling is structural.

- **Doc B §3.18** — Replace the "S3-backed per-workspace JSONL with distributed lock" recommendation with a *concrete* schema: many-small-objects pattern (one S3 object per event, key = `workspace_id/ulid.json`, ULID provides natural lexicographic order). Per-event atomicity via S3 conditional-write on `If-None-Match: *`. Reconstruction = paginated prefix-scan. Trust property is preserved because each object is content-addressable.

- **Doc C R-O-01** — Publish a per-stage write-budget µs-table (signer-spawn / Ed25519 / CBOR / blake3 / JSONL-append / projector-consume / Mem0g-invalidate / Cedar-policy / witness-cosign) and identify which stage saturates first at the SLO target. This is the actual capacity-planning artefact V2-α needs.

- **Doc C R-O-03** — Add quantified RTO target (recommend: ≤30min rebuild at 100M events from snapshot; ≤4h cold reseed at 100M events). Specify the snapshot format (FalkorDB native dump? Custom canonical serialisation?), snapshot integrity check (signed snapshot hash anchored to a `kind=projection-snapshot` event in `events.jsonl` — keeps the trust property intact).

- **Doc D §4 (new subsection §4.5 "Graph DB Comparative Matrix")** — Head-to-head: FalkorDB / Neo4j Community / ArcadeDB / Memgraph on (license, embeddable Y/N, write throughput claim+source, read p99 claim+source, HA model, backup primitive, last-release date, GitHub stars, GitHub releases SLSA-availability, ecosystem-tools available). Doc B §2.3 cross-link this matrix.

- **Doc E Demo 5** — Inline-label the timing bar "Source: Mem0g LOCOMO benchmark, arxiv 2504.19413" in the *visible UI* (not just speaker note). Reframe storyboard from "Atlas demonstrates 91% latency reduction" to "Atlas+Mem0g hybrid inherits Mem0g's published retrieval performance, while preserving Atlas's signed-event provenance per result."

---

## Offene Fragen für Phase 3

- **Q-DB-1 (Tech-pick / V2-α blocker):** Should FalkorDB remain the default V2-α graph backend, or should ArcadeDB (Apache-2.0, JVM, Cypher+Gremlin+SQL, multi-model) be the default with FalkorDB as an opt-in fast-traversal alternative? The Kuzu acquisition closed the obvious MIT fallback, escalating the SSPL exposure (Doc C R-L-02). A benchmark spike (1 session: same workload on both, same hardware) would resolve this empirically.

- **Q-DB-2 (Drift detection):** Is the shadow-projector continuous-audit pattern (per P-HIGH-1 fix) operationally acceptable, or does it double infrastructure cost in a way that blocks V2-α adoption? Phase 3 needs to decide between "shadow projector by default" vs "shadow projector for high-stakes workspaces only" vs "periodic re-validation jobs in CI/CD instead of continuous".

- **Q-DB-3 (Mem0g determinism gap):** Doc B §2.5 treats Layer-3 rebuildability as "eventually rebuildable" — different embedding model versions produce different cache states. Is this acceptable for the regulator-audit use case (Demo 2), or do auditors require *byte-identical* cache reproducibility across replays? If the latter, Mem0g must be pinned to a specific embedding-model version *forever per workspace*, which is operationally heavy.

- **Q-DB-4 (Write SLO realism):** The V2 write SLO target "p99 < 50ms at 1K concurrent writes/sec" (Doc C R-O-01) — is this *measured against the user-facing `POST /api/atlas/write-node` endpoint* (including signer subprocess + JSONL append + projector consume + Mem0g invalidate) or is it *measured up to JSONL append only* (with projector + Mem0g as async downstream)? The two answers differ by 10-100×. Decision needed before V2-α benchmarking starts.

- **Q-DB-5 (Projection-rebuild-cost ceiling):** At what total event count does the "rebuildable from events.jsonl" trust invariant become operationally false? Doc B §2.1 treats this as an invariant for all time; in practice every event-sourcing system has a scale at which reseed becomes a multi-day operation and the system effectively *cannot* be trusted to rebuild within a useful timeframe. What is Atlas's number? 50M events? 500M? Without this number, enterprise procurement teams cannot evaluate whether Atlas meets their RPO/RTO requirements.

- **Q-DB-6 (Read-replica scaling for FalkorDB):** Does FalkorDB support production-grade read replicas with sub-second replication lag? If not, the read-side API cannot scale beyond single-FalkorDB-instance throughput. If yes, Phase 3 should spec the replication topology (per-workspace replica? Per-region replica? Read-from-replica with provenance-pointer-consistency check?).

- **Q-DB-7 (Content-store backend for GDPR §3.3 separation):** Doc B §3.3 separates signed content_hash from deletable content. What's the content store? S3 with object-level delete API? PostgreSQL BLOB with row-delete? In-house DB? The choice affects projector throughput (P-HIGH-3 stacks here) and GDPR-erasure latency (regulator expects ≤30 days under Art. 17).

- **Q-DB-8 (HSM throughput ceiling for multi-agent):** At what number of HSM-backed concurrent agents does shared-HSM throughput become the gating factor? Phase 3 should produce a sizing guide: "for ≤N agents per HSM device, expect ≤M signatures/sec aggregate"; ops teams can plan HSM hardware accordingly.

- **Q-DB-9 (Cache backend for read-side API):** What's the recommended cache layer in front of the read-side API? In-process (Node.js `lru-cache`) is simplest but doesn't scale beyond single atlas-web instance. Redis adds an ops dependency. Cloudflare Workers KV / Vercel Edge Cache adds a hosting dependency. Phase 3 should commit before V2-β starts; the cache invalidation logic depends on the backend.

- **Q-DB-10 (Vector search separation):** Does Atlas need a separate vector store (pgvector / Qdrant / Milvus / Weaviate) alongside Mem0g, or are Mem0g's internal vector primitives sufficient for the V2 read-side query patterns? If the former, Atlas becomes a four-layer architecture (events.jsonl → FalkorDB → Mem0g → vector store) and the trust invariant chain extends; the Doc B three-layer marketing pitch becomes a four-layer reality.

- **Q-DB-11 (Schema-versioning real-world cost):** Doc B §3.17 picks hard-reseed. At what production scale does this stop being feasible — i.e., when is the cost of forcing a reseed (downtime + compute + operator coordination) higher than the cost of versioned co-existence (audit complexity)? Phase 3 should produce a heuristic threshold rather than committing to one option for all future projector_version transitions.

- **Q-DB-12 (Sigstore Rekor v2 migration timing):** When does Atlas migrate from Rekor v1 (pinned ECDSA P-256, V1 design) to Rekor v2 (tile-backed transparency log)? Earliest reasonable: V2-γ. Latest reasonable: V3. The migration affects every anchor verification — should be planned now to avoid surprise rework.

---

**Doc owner:** Database/performance critique agent (Phase 2 Agent #3). **Created:** 2026-05-12. **Status:** v0, ready for Phase 3 synthesis.
**Next action:** Phase 3 must (a) commit to a graph DB choice or schedule a V2-α benchmarking spike, (b) quantify the V2 write SLO at the per-stage level, (c) decide the production drift detection pattern, (d) settle Q-DB-1 through Q-DB-12 before Welle-14b roadmap is finalised.
