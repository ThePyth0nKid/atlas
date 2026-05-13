# ADR-Atlas-007 — Parallel-Projection Design for >10M-Event Scaling

| Field        | Value                                                    |
|--------------|----------------------------------------------------------|
| **Status**   | Design (not adopted)                                     |
| **Date**     | 2026-05-13                                               |
| **Welle**    | V2-β Welle 10                                            |
| **Authors**  | Nelson Mehlis (`@ThePyth0nKid`)                          |
| **Replaces** | —                                                        |
| **Superseded by** | —                                                   |
| **Related**  | V2-α Welle 3 (canonicalisation byte-pin), Welle 4 (ProjectorRunAttestation), Welle 5 (single-threaded upsert pattern), Welle 6 (CI gate); DECISION-ARCH-1 (triple-hardening), DECISION-SEC-2 (projection determinism) |

---

## 1. Context

### 1.1 The 8.3-hour rebuild problem

**Current state:** Atlas V2-α ships a single-threaded projector that rebuilds the FalkorDB graph by replaying Layer 1 events (events.jsonl) through deterministic Cypher-equivalent mutations. At 100M events, this rebuilds in ~8.3 hours on a standard 16-core host (`docs/V2-MASTER-PLAN.md` §4, risk R-A-01: "8.3h rebuild at 100M events baseline").

**Why it matters:**
- **RTO impact:** A production FalkorDB corruption, shard-rebalance, or schema-migration scenario forces a full replay to recover a known-good state. 8.3 hours of stale Layer-2 reads is operationally unacceptable for customers with SLA constraints (especially enterprise deployments with agent-swarm use cases).
- **Deployment friction:** Atlas-as-a-service deployments (post-V2-β) will need zero-downtime migrations. Rebuilding Layer 2 serially blocks read-path cutover. The V2-β Read-API (endpoints + MCP V2 tools) depends on Layer 2 availability; single-threaded projection is a gating concern.
- **Developer velocity:** local development rebuilds on 1M–10M event traces take 2–15 minutes in serial mode. Parallel speedup to sub-minute local rebuilds improves the development UX.

**V2-α shipped successfully with single-threaded projection** because the initial scope (Mem0g integration + Read-API + Explorer UI) did not expose projection speed as a customer-visible blocker at launch. But the risk matrix identifies this as a precondition for scaling.

### 1.2 Byte-determinism as a load-bearing invariant

Atlas's three-layer trust architecture (Master Vision §5.2, Welle 3 design invariant #1) rests on a core property: **parallel projection MUST produce byte-identical `graph_state_hash` as single-threaded equivalent.**

**Why byte-determinism is load-bearing:**

1. **Layer 1 remains authoritative:** events.jsonl is signed, Rekor-anchored, and offline-verifiable. Layer 2 (FalkorDB projection) is derivative — any silent corruption or non-determinism degrades to "cache divergence" and must be detectable.

2. **CI gate enforcement:** Welle 6 provides the `verify_attestations_in_trace` library function in `atlas-projector` (gate.rs) which operators invoke against a re-projection to detect drift; an automated CI gate against a pinned `.projection-integrity.json` is a deferred V2-β welle [open question: which Welle wires this?]. The byte-determinism pin in `canonical.rs` exists today and is one of the 7 V2-α CI byte-pins — analog to V1's `signing_input_byte_determinism_pin`. If projection silently drifts non-deterministically, the byte-pin fires on canonical-form changes; the operator-invoked attestation verification fires on hash drift at replay time. **Both depend on deterministic canonicalisation.**

3. **ProjectorRunAttestation event binding:** Welle 4/5 defines the `ProjectorRunAttestation` payload shape (`emission.rs`) and Welle 7 wires it through the `atlas-signer emit-projector-attestation` CLI; the full Layer-1 attestation chain is operational in v2.0.0-alpha.1, asserting `(projector_version, head_hash) → graph_state_hash`. Parallel projection MUST preserve this binding — a multi-threaded rebuild that produces a different hash than the single-threaded reference invalidates the attestation.

**Consequence for parallel design:** Parallel projection strategies that sort *after* merge (rather than during merge) risk byte-ordering non-determinism under concurrent writes. The canonical form in Welle 3 (`canonical.rs` §) specifies RFC 8949 §4.2.1 CBOR map-key sorting (length then lex). This sort is applied *to the final merged state*, which means any worker-state merge must preserve **logical identifier ordering** (entity_uuid, edge_id) even when workers operate in parallel.

### 1.3 ArcadeDB backend compatibility timing

V2-α chose **ArcadeDB (Apache-2.0)** as the primary graph backend (per DECISION-DB-4, V2-α Welle 2 spike, 2026-05-12) with FalkorDB SSPLv1 as a fallback. ArcadeDB's embedded-mode HTTP API operates differently from in-memory-only projection — some concurrent-write patterns may or may not be supported.

**Critical timing:** Welle 17 (ArcadeDB driver integration) must start from a solid parallel-projection *design* that does not presuppose unsupported ArcadeDB concurrency patterns. This ADR must document the design before W17 locks implementation details. If W17 discovers that "option A requires per-vertex write locks that ArcadeDB doesn't expose", the ADR has already identified the risk and the next welle can pivot to option B without rework.

---

## 2. Decision Drivers

### 2.1 MUST preserve byte-determinism

Parallel projection MUST produce byte-identical `graph_state_hash` as single-threaded equivalent for the same input event sequence. Non-determinism is a disqualifier — it breaks the CI gate and the ProjectorRunAttestation binding.

**Consequence:** Options that introduce load-dependent ordering (e.g. "hash merge order depends on which worker finishes first") are categorically rejected unless the hash is re-sorted post-merge to canonical form. Re-sorting post-merge is allowable (option C candidate) but adds complexity.

### 2.2 MUST be deterministic across re-runs

Same input event sequence → same graph state regardless of worker count. If option A produces state S with 4 workers and state S' with 8 workers, option A fails. The projector must guarantee that `workspace_id + events.jsonl` uniquely determines the output state.

**Consequence:** Worker-count tuning must be an operational parameter (set at rebuild time or in config), not a source of non-determinism. Per-worker local state must be independent of other workers' progress.

### 2.3 SHOULD scale to ~10M events in <30 min on 16 cores

Operational aspirations: a 10M-event trace should rebuild in sub-30-minute wall-clock on commodity hardware (16-core machine, 64GB RAM, SSD storage). At 100M events, sub-4-hour rebuild (vs. current 8.3h) is meaningful progress. **This is a SHOULD, not a MUST** — if the best design option is sub-optimal on latency, the trade-off is acceptable provided RTO improves substantially.

### 2.4 SHOULD support both in-memory and ArcadeDB backends

The projector abstraction (Welle 5's `project_events()` function and callers) currently targets in-memory `GraphState`. V2-β Welle 17 will introduce an ArcadeDB `GraphStateBackend` trait with HTTP-based mutations. The parallel-projection design should not presuppose ArcadeDB-specific features (transactions, MVCC, write locks) that may not be available in embedded mode.

**Consequence:** Options that rely on ArcadeDB's transaction-isolation guarantees are lower-confidence (marked MEDIUM-LOW) unless W17 spike validates those guarantees pre-production.

### 2.5 Failure-recovery: partial-projection-then-crash scenario

If a parallel-projection process crashes mid-rebuild, the recovery semantics must be clear. Option A (workspace-parallel) naturally checkpoints per workspace. Option B (entity-shard) naturally checkpoints per shard. Option C (batch-streaming) checkpoints per batch.

**Consequence:** The ADR must document recovery strategy for each option. "Resume from event N" is simpler than "resume from event N, re-apply worker 3's partial state".

---

## 3. Considered Options

### 3.1 Option A: Workspace-Parallel Projection

**Concept:** Each workspace projects independently in its own worker thread. After all workers finish, canonicalisation walks the finalised per-workspace states and merges at the trace level (post-projection).

**Architecture:**
```
Input: events.jsonl (across multiple workspaces)

Thread 1: workspace_a events → GraphState_a
Thread 2: workspace_b events → GraphState_b
Thread 3: workspace_c events → GraphState_c
    ...
[Thread pool waits for all]
    │
    ▼
Merge: combine all GraphState_a + GraphState_b + GraphState_c
    │
    ▼
Canonicalisation: build_canonical_bytes(merged_state)
    │
    ▼
Output: graph_state_hash (deterministic)
```

**Effort level:** LOW. Minimal changes to the current `project_events()` API. The projector already idempotently processes per-workspace (workspace_id is a parameter). Wrap in a thread-pool dispatcher + merge logic.

**Byte-determinism risk:** NONE if implemented correctly. Each workspace is processed independently (order within workspace matters, but inter-workspace order is irrelevant for the final state). Merge is deterministic because workspaces are logically disjoint **in graph-topology terms** — no cross-workspace edges or shared `entity_uuid` values in V1's event model. `author_did` fields (V2-α agent-identity, `did:atlas:<blake3(ed25519_pubkey)>`, see `crates/atlas-trust-core/src/agent_did.rs`) MAY be shared across workspaces — the same agent DID can author events in multiple workspaces — but this does NOT affect merge determinism or `graph_state_hash` because `author_did` is bound into the per-event signing input (cose.rs), not into the projected graph topology. Canonicalisation sorts by entity_uuid (BTreeMap iteration order is deterministic).

**Per-workspace intra-event-ordering requirement (security-critical):** Per-workspace event dispatch MUST preserve the original `events.jsonl` sequential order within each workspace. Sorting events by sequence number before dispatch to the worker is mandatory — upsert operations are idempotent but NOT commutative (a `node-update` applied before `node-create` produces a different state than the reverse). This invariant is what makes "inter-workspace order is irrelevant" safe to claim.

**ArcadeDB compatibility:** MEDIUM-HIGH. In-memory projection is unaffected. When migrating to ArcadeDB (Welle 17), each worker writes to its own workspace partition (ArcadeDB supports multi-tenant isolation). Concurrent writes to disjoint partitions are supported by most databases. W17 spike must validate whether ArcadeDB's embedded-mode HTTP API handles concurrent-workspace writes without deadlock.

**Failure-recovery:** SIMPLE. If a worker crashes, restart it on the same workspace. No inter-worker dependencies.

**Reversibility:** HIGH. If performance is suboptimal, fall back to single-threaded projection by disabling the thread pool (set worker count = 1 in config).

**Operational complexity:** LOW. Workspace-parallel tuning is intuitive ("run N workers, each processing a workspace").

**Trade-offs table:**

| Dimension | Rating | Notes |
|---|---|---|
| Implementation effort | LOW | Minimal API surface changes |
| Byte-determinism risk | NONE | Workspaces are logically disjoint |
| ArcadeDB compatibility | MEDIUM-HIGH | Depends on concurrent-workspace-write support; W17 must validate |
| Scale to 10M events in <30min | MEDIUM-LOW | Speedup limited by number of active workspaces; if single-workspace trace, no parallelism |
| Failure-recovery | LOW | Per-workspace checkpoints natural |
| Reversibility | HIGH | Trivial to disable |
| Operational simplicity | HIGH | Worker count = number of active workspaces |

**Confidence:** HIGH (implementation straightforward; determinism is algebraic).

**Dependency on W17 validation:** ArcadeDB concurrent-workspace-write semantics.

---

### 3.2 Option B: Entity-UUID Shard Partitioning

**Concept:** Hash-shard events by `entity_uuid` MSB (e.g. first 4 bits → 16 shards) across N workers. Each worker projects its shard of events. After projection, per-worker states are merged by entity_uuid (already the canonical-form sort key).

**Architecture:**
```
Input: events.jsonl

Partition phase: for each event {
  if event.payload.type in [node_create, node_update]:
    shard_id = u64::from_be_bytes(blake3(entity_uuid)[0..8]) % N
  else if event.payload.type == edge_create:
    shard_id = u64::from_be_bytes(blake3(from_entity_uuid)[0..8]) % N  // or to_entity_uuid; pick consistently
  enqueue event → shard_id
}

Thread 1: events for shard_0 → GraphState_shard_0
Thread 2: events for shard_1 → GraphState_shard_1
    ...
[Thread pool waits for all]
    │
    ▼
Merge: for each entity_uuid in sorted_order {
  merge(shard_0[entity_uuid], shard_1[entity_uuid], ...)
}
    │
    ▼
Canonicalisation: build_canonical_bytes(merged_state)
    │
    ▼
Output: graph_state_hash
```

**Effort level:** MEDIUM. Requires:
  1. Partitioning logic (hash-shard events by entity_uuid).
  2. Per-worker state structures (likely still in-memory `GraphState` but scoped to shard).
  3. Merge logic (iterate sorted entity_uuids, combine shard states).
  4. Edge-handling logic: if an edge connects entities in different shards, both shards must process it idempotently (or the merge must handle cross-shard edges).

**Byte-determinism risk:** LOW-MEDIUM. If all shards are processed and merged deterministically (sorted by entity_uuid, which is the canonical form's sort key), the output is deterministic. **Risk:** edge handling under shard partitioning. If `edge_create` event targets entities in different shards, both shards will process the event (idempotently, since upsert is idempotent). The merge must ensure edge is represented exactly once in the final state, not duplicated. This is solvable but adds logic complexity.

**ArcadeDB compatibility:** MEDIUM. Shards can be mapped to ArcadeDB vertex partitions or sub-graphs. Concurrent writes to disjoint entity-uuid ranges are supported by most databases. **Risk:** ArcadeDB may not expose a native sharding API; embedding-mode HTTP might not expose shard-local writes. W17 spike must validate.

**Failure-recovery:** MEDIUM. If a shard worker crashes, restart it on the same shard range. Cross-shard edges complicate recovery ("which shard do I retry if the edge creation failed?"). Idempotency of upsert mitigates but adds observability burden.

**Reversibility:** MEDIUM. If performance is poor, fall back to single-threaded by setting shard count = 1. But operational observability (which shard is slow?) requires instrumentation.

**Operational complexity:** MEDIUM. Shard-count tuning requires understanding the entity-uuid distribution in the workload. Skewed distributions (one shard is 90% of entities) limit parallelism.

**Trade-offs table:**

| Dimension | Rating | Notes |
|---|---|---|
| Implementation effort | MEDIUM | Shard routing + merge logic required; edge-handling non-trivial |
| Byte-determinism risk | LOW-MEDIUM | Merge is deterministic if sorted; edge-dedup logic must be correct |
| ArcadeDB compatibility | MEDIUM | Depends on ArcadeDB shard-local write support; W17 must validate |
| Scale to 10M events in <30min | MEDIUM | Good scaling if entity-uuid distribution is balanced |
| Failure-recovery | MEDIUM | Per-shard recovery is natural but edge-handling adds complexity |
| Reversibility | MEDIUM | Fallback to shard count = 1 is possible but operational observability needed |
| Operational simplicity | MEDIUM | Shard-count tuning depends on workload entity-uuid distribution |

**Confidence:** MEDIUM (solvable but requires more careful edge-handling validation).

**Dependency on W17 validation:** ArcadeDB shard-local write semantics + edge-handling correctness under concurrent writes.

---

### 3.3 Option C: Batch-Streaming Projection

**Concept:** Read events.jsonl in sequential batches of B events (e.g. 10K events per batch). For each batch, spin up N workers that project the batch in parallel (each worker processing 10K/N events from the batch). After batch completes, merge per-batch partial states into a single in-memory `GraphState`. Repeat for next batch.

**Architecture:**
```
Input: events.jsonl

For each batch of B events {
  Dispatch N workers: each processes B/N events from the batch
  [Workers project locally, building per-worker GraphState_batch_worker_i]
  Merge: combine all GraphState_batch_worker_i into GraphState_batch
  
  Canonicalisation: walk GraphState_batch (no hash yet; just accumulate)
  Append events to FalkorDB or checkpoint to disk
}

Final canonicalisation: walk complete GraphState
Build final graph_state_hash
```

**Effort level:** MEDIUM-HIGH. Requires:
  1. Event batching (read B events at a time from events.jsonl).
  2. Per-batch worker dispatch (spin up N workers, each processing B/N events).
  3. Per-batch merge (combine partial states).
  4. Streaming canonical form (build canonical bytes incrementally across batches, or re-canonicalise at the end).

**Byte-determinism risk:** LOW if per-batch merge is deterministic. Within a batch, worker assignment (which events go to which worker) must be deterministic. If `events[i..i+B/N]` always go to worker 0, `events[i+B/N..i+2B/N]` always go to worker 1, etc., the output is deterministic. **Risk:** the final canonicalisation must walk the merged state in canonical order (sorted by entity_uuid). If the per-batch merge produces partial states out of order, the final canonicalisation must re-sort, which is expensive.

**ArcadeDB compatibility:** MEDIUM. Batch-streaming maps naturally to ArcadeDB's streaming-insert semantics (if ArcadeDB supports them). Per-batch atomic commits are a natural checkpoint boundary. **Risk:** ArcadeDB embedded-mode may not expose per-batch transaction boundaries; W17 spike must validate.

**Failure-recovery:** HIGH. If a batch worker crashes, restart from batch N. Checkpoints are natural at batch boundaries. No complex edge-handling logic.

**Reversibility:** HIGH. If parallelism within batches is slow, increase batch size or reduce worker count per batch. Single-threaded fallback is trivial (batch size = all events, workers = 1).

**Operational complexity:** MEDIUM. Batch-size and per-batch worker count are two tuning parameters. Distribution of events across batch-workers must be deterministic (e.g. round-robin or hash-based).

**Trade-offs table:**

| Dimension | Rating | Notes |
|---|---|---|
| Implementation effort | MEDIUM-HIGH | Batching logic + per-batch worker dispatch required; streaming canonical form non-trivial |
| Byte-determinism risk | LOW | Per-batch merge is deterministic if event distribution is deterministic; final re-canonicalisation ensures byte-identity |
| ArcadeDB compatibility | MEDIUM | Depends on ArcadeDB's per-batch transaction support; W17 must validate |
| Scale to 10M events in <30min | MEDIUM-HIGH | Good scaling if batch size is tuned; per-batch parallelism is flexible |
| Failure-recovery | HIGH | Natural checkpoint boundaries at batch end |
| Reversibility | HIGH | Trivial fallback by adjusting batch size + worker count |
| Operational simplicity | MEDIUM | Two tuning parameters (batch size, per-batch workers); both affect throughput |

**Confidence:** MEDIUM (determinism is guaranteed by final re-sort, but streaming-canonical implementation is complex).

**Dependency on W17 validation:** ArcadeDB batch-transaction semantics.

---

## 4. Decision

### 4.1 Recommended Option: A (Workspace-Parallel Projection)

**Atlas recommends Option A (workspace-parallel) as the V2-β design,** with confidence level **HIGH**.

**Precondition:** This design is safe to implement only after V2-α Welle 3 (canonicalisation byte-pin) and Welles 4+5+7 (ProjectorRunAttestation end-to-end: payload shape + emission + signer CLI) are production-active — all are V2-α Welle 1–8 shipped state in v2.0.0-alpha.1. Without these, parallel projection cannot be detected as drifting.

**Rationale:**

1. **Byte-determinism is algebraic.** Workspaces are logically disjoint **in graph-topology terms** in V1's event model (no cross-workspace edges, no shared `entity_uuid` values). `author_did` may be shared across workspaces but is bound into the per-event signing input — not into the projected graph topology — so cross-workspace agent identity does not affect merge determinism (see §3.1 Option A). Processing workspaces in parallel and merging deterministically is straightforward, provided per-workspace intra-event order is preserved (see §3.1 ordering requirement). The canonical form (sorted by entity_uuid) naturally applies per-workspace, and merging at the workspace level preserves the sort order.

2. **Operational simplicity wins the early stage.** Atlas is pre-launch V2-β; operational tuning headroom (e.g., entity-uuid-distribution balancing for option B) is a future concern. Workspace-parallel is maximally simple: "N workers, each processes a workspace". Operational teams grok this immediately.

3. **Scales the right cases.** Many Atlas customers will have multi-workspace deployments (shared audit traces across multiple agents, multiple teams, multiple tenants). Option A gains parallelism on these workloads automatically. Single-workspace traces (option A's worst case) remain single-threaded, but those are development / small-customer scenarios where 8.3 hours is already acceptable (or reaccepted if parallelism buys nothing).

4. **ArcadeDB compatibility is most likely.** Multi-tenant concurrent writes to partitioned sub-graphs are a standard pattern. ArcadeDB's embedded-mode will almost certainly support concurrent writes to disjoint workspaces. This is lower risk than shard-local semantics (option B) or per-batch transaction boundaries (option C). **(W16 spike must validate; see §6 open questions, item 1 and item 3.)**

5. **Failure-recovery is trivial.** Checkpointing per workspace is natural. No complex edge-deduplication or batch-boundary logic.

6. **Reversibility is high.** If performance data in V2-β contradicts projections (e.g. "our customers are 90% single-workspace deployments"), the next welle can pivot to option B or C with a clear migration path. Configuration knob (worker count = number of active workspaces or explicit tuning) allows gradual rollout.

### 4.2 Fallback options

- **Option B (entity-UUID shard partitioning)** is the fallback if V2-β data shows that workspace-parallel does not scale single-workspace traces. Effort cost is medium; implementation is more complex but solvable.
- **Option C (batch-streaming projection)** is the fallback if both A and B are found to have ArcadeDB incompatibilities or failure-recovery issues. Effort cost is higher; determinism is still guaranteed by final re-sort.

### 4.3 What this decision does NOT decide

This ADR **does not decide** on:
- **Actual implementation schedule.** Option A's implementation is a candidate for V2-β Welle N (post-W18) or V2-γ, depending on blocking dependencies and customer demand. This ADR is the *design* pre-W17 lock, not the engineering plan.
- **Operator-tunable worker-count config.** Worker count is an operational / configurational concern (set in a .toml file or environment variable). The ADR specifies the algorithm (workspace-parallel); the config layout is a V2-β candidate task.
- **Incremental-projection semantics.** If Layer 1 grows by N new events, can the projector rebuild only from event M+1 onward, rather than re-replaying from event 1? This is orthogonal to parallelism and is a future welle candidate (post-V2-β).
- **Hybrid parallel-streaming mode for ArcadeDB.** When ArcadeDB-backed (Welle 17), the projector might choose a different strategy (e.g. batch-streaming to ArcadeDB, workspace-parallel to in-memory fallback). Welle 17 ADR (ADR-Atlas-011) will document that trade-off.

---

## 5. Consequences

### 5.1 Accepted today

- Single-threaded projection remains the baseline until option A is implemented.
- 8.3-hour rebuild at 100M events is a known operational constraint.
- RTO for corruption scenarios remains ~8–10 hours until implementation.

### 5.2 Mitigated by this design

- **Risk R-A-01 (Projection Determinism Drift):** Option A design preserves byte-determinism algebraically. When implemented, the same CI gate that validates single-threaded projection will validate parallel projection. No new CI risk.
- **Risk R-A-01 operational impact (8.3h rebuild):** Option A, if implemented, targets <30 min for 10M events, <4 hours for 100M events on 16 cores. Actual performance depends on workspace-distribution in customer workloads.

### 5.3 Dependencies on Welle 17 (ArcadeDB driver)

Option A assumes concurrent writes to disjoint workspaces are supported in ArcadeDB's embedded-mode HTTP API. **Welle 17 spike (W16) must validate:**
  - Does ArcadeDB embedded HTTP allow concurrent POST requests to `/graph/{workspace_id}/vertices` without deadlock or lost updates?
  - Are per-workspace transactions supported (all-or-nothing semantics per workspace)?
  - Does concurrent writes to multiple workspaces maintain graph invariants (dangling-edge detection, etc.)?

If W16 spike finds that ArcadeDB does NOT support concurrent-workspace writes, the design remains sound (fallback to single-threaded), but RTO improvement is deferred to option B or C.

---

## 6. Open questions for W17 (ArcadeDB) design

1. **Concurrent-workspace-write semantics:** Does ArcadeDB HTTP API guarantee that concurrent writes to workspace_a and workspace_b produce a deterministically-mergeable state?

2. **Per-workspace graph-integrity constraints:** Does ArcadeDB enforce edge-referential-integrity at the workspace level or globally? If a workspace contains a dangling edge (vertex deleted but edge remains), does ArcadeDB's consistency model catch it?

3. **Workspace isolation at query-time (SECURITY — tenant isolation):** If two workspaces share the same ArcadeDB instance, can queries accidentally leak data between workspaces (e.g. a Cypher query that doesn't explicitly filter by workspace_id)? The projector must ensure workspace isolation at the projection layer, but ArcadeDB must not make this harder.

4. **Streaming inserts vs. batch atomic writes:** Does ArcadeDB embedded-mode support per-workspace atomic transaction boundaries? Option A projects each workspace in its own transaction; if ArcadeDB doesn't expose transaction control in HTTP API, the projector must handle eventual consistency (Worker A commits, Worker B is still in-flight → merge is non-deterministic).

5. **Shard-aware backend trait:** Welle 17's `GraphStateBackend` trait will abstract in-memory vs. ArcadeDB. If option A implementation chooses to shard workspace writes at the driver level (e.g., route workspace_a updates to ArcadeDB shard 0, workspace_b to shard 1), the trait must expose shard-routing hooks. W17 ADR (ADR-Atlas-011) will document the trait design; this ADR flags "shard-aware routing support" as a nice-to-have for high-throughput multi-workspace scenarios.

---

## 7. Reversibility

**Option A is HIGH reversibility.**

- If performance data (V2-β customer workloads) contradicts projections, the next welle can implement option B or C without reworking the event model or Layer 1 architecture.
- Fallback to single-threaded (set worker pool size = 1) is a configuration knob with no code rework required.
- The canonical form (Welle 3) is unaffected; determinism guarantees hold regardless of implementation.

---

## 8. Watchlist + review cadence

### 8.1 Tracking items for W17 (ArcadeDB spike)

- Concurrent-workspace-write validation (baseline requirement for option A)
- Per-workspace transaction-boundary support (baseline requirement for option A)
- Workspace-isolation query guarantees (security requirement, separate from parallelism)
- Shard-aware backend-trait design sketch (V2-β candidate, not blocking)

### 8.2 Tracking items for V2-β customer data

- Workspace distribution in first 10 customer deployments (single-workspace vs. multi-workspace ratio)
- Observed rebuild latency under option A (if implemented during V2-β)
- Whether single-workspace deployments become a common case (triggers option B fallback planning)

### 8.3 Review cadence

- **Pre-W17 start:** Review this ADR against W16 ArcadeDB spike findings. If findings invalidate option A, update §4 Decision and open follow-on ADR (ADR-Atlas-011, W17a) with revised recommendation.
- **Post-V2-β launch:** Review workspace-distribution data from first 10 customers. If multi-workspace is rare, open V2-γ spike to evaluate option B ROI.
- **Incident-triggered refresh:** If a customer hits the 8.3-hour rebuild ceiling in V2-β (unforeseen at-scale single-workspace deployment), expedite option B design + implementation.

---

## 9. Decision log

| Date       | Event                                                  | Outcome |
|------------|--------------------------------------------------------|---------|
| 2026-05-13 | ADR-Atlas-007 opened. Initial status: Design. | —       |
| TBD        | W16 ArcadeDB spike completes. Validates option A concurrent-workspace assumptions. | Option A recommendation confirmed or revised. |
| TBD        | W17a ArcadeDB driver design (ADR-Atlas-011) references this ADR's option A. | W17a proceeds with confidence or escalates to option B if W16 invalidates. |

---

**End ADR-Atlas-007**
