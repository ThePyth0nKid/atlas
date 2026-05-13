# V2-β Welle 16 — Plan-Doc (ArcadeDB Embedded-Mode Spike)

> **Status:** DRAFT 2026-05-13. Awaiting parent agent's confirmation before merge.
> **Orchestration:** Phase 8, SERIAL (single welle, doc-only) per `docs/V2-BETA-ORCHESTRATION-PLAN.md` §1.
> **Driving decisions:** `DECISION-DB-4` (ArcadeDB Apache-2.0 primary, FalkorDB SSPLv1 fallback, MEDIUM-HIGH confidence); ADR-Atlas-007 §6 (5 open questions for W17); V2-master-vision §5 (Three-Layer Trust Architecture).

W16 is a **doc-only architectural spike** that resolves the architectural unknowns left open after V2-α Welle 2's license-comparative spike and ADR-Atlas-007's parallel-projection design. V2-α Welle 2 picked ArcadeDB primary on license + projection-determinism + deployment-simplicity factors but explicitly noted "MEDIUM-HIGH confidence — raise to HIGH requires (a) benchmark validation, (b) counsel SSPL opinion, (c) operator-runbook deployment validation". W16 attacks the architectural side: which mode (embedded vs server), which HTTP client, what concurrent-workspace-write semantics, what tenant-isolation pattern, what CI orchestration, what `GraphStateBackend` trait shape, what byte-determinism guarantees, what perf ballpark, what FalkorDB-fallback trigger thresholds, what W17a entry criteria.

**Why this as Welle 16:** Phase 9's W17a/b/c ArcadeDB driver integration is the longest serial chain in the V2-β critical path. W17a's `GraphStateBackend` trait shape, W17b's HTTP client choice, W17c's Docker-Compose orchestration all depend on architectural decisions that W16 surfaces. Locking these in a spike-doc before W17a starts avoids reactive design pivots mid-implementation. ADR-Atlas-010 captures the binding decision; ADR-Atlas-011 (W17a) builds on it.

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| `docs/V2-BETA-ARCADEDB-SPIKE.md` (NEW, ~400 lines): spike-doc analog to V2-α Welle 2 | Any Rust/TypeScript code changes (W17a/b/c) |
| `docs/ADR/ADR-Atlas-010-arcadedb-backend-choice-and-embedded-mode-tradeoff.md` (NEW, ~300-350 lines) | `GraphStateBackend` trait implementation (W17a) |
| `.handoff/v2-beta-welle-16-plan.md` (NEW): this plan-doc | Docker-Compose YAML in `.github/workflows/` (W17c) |
| Reference resolution of ADR-Atlas-007 §6 Q1-Q5 | `CHANGELOG.md`, `docs/V2-MASTER-PLAN.md` status table, `docs/SEMVER-AUDIT-V1.0.md`, `.handoff/decisions.md`, `.handoff/v2-session-handoff.md`, `docs/V2-BETA-ORCHESTRATION-PLAN.md` (parent consolidates) |
| Resolution of Q4 (embedded vs server) and Q5 (Rust HTTP client) | Counsel-validated SSPL §13 opinion (Nelson-led parallel track) |
| FalkorDB-fallback trigger thresholds (measurable criteria) | Counsel sign-off on EU-data-residency deployment (Nelson-led) |
| W17 entry criteria + W17b/c deferral list | Actual benchmark harness execution (W17c integration tests) |

## Decisions (final, pending parent confirmation)

- **DECISION-DB-4 status:** CONFIRMED, not re-evaluated. ArcadeDB Apache-2.0 remains V2-α/β primary; FalkorDB SSPLv1 remains performance-validation fallback.
- **ArcadeDB deployment mode:** SERVER mode (not embedded). Rationale: Hermes-skill distribution constraint forbids JVM in-process; server mode preserves process isolation for multi-tenant security; HTTP API parallelisation is natural from Rust without JNI.
- **Rust HTTP client:** `reqwest` (async, tokio-aligned, ~2 MB binary cost). Rejected: `ureq` (sync, blocks async projector); hand-rolled `hyper` (no measurable benefit).
- **Tenant isolation (Q3, SECURITY):** Layered defence — per-database isolation (Layer 1 primary) + projector workspace_id parameter binding (Layer 2 active enforcer) + Cypher AST validator (Layer 3 mutation-hardening; does NOT enforce workspace_id presence). Operator runbook MUST require per-database-per-workspace deployment.
- **Concurrent-workspace-write (Q1):** ArcadeDB HTTP API serialises writes per-database (=per-workspace); concurrent writes to disjoint databases are independent (no inter-database lock). Atlas projector creates ONE ArcadeDB database per workspace, making Option-A parallel projection (per ADR-Atlas-007 §3.1) directly supported.
- **Workspace integrity (Q2):** Edge-referential integrity enforced at workspace/database level by ArcadeDB schema-required mode (when both endpoints in same workspace); cross-workspace edges are forbidden by V1's event model and the projector enforces this at application layer.
- **Streaming vs batch (Q4 corollary):** Per-workspace atomic transactions via ArcadeDB HTTP `/api/v1/begin` + `/api/v1/commit`. Each projection cycle = one transaction per workspace.
- **Native UUID indexing performance:** ArcadeDB B-tree indexes on `entity_uuid` deliver O(log n) lookups; ~1ms typical for 10M-vertex workspaces.
- **GraphStateBackend trait sketch (Q8):** Provided as ~40-line Rust pseudo-code in spike-doc §7. W17a fills the actual impl.
- **Byte-determinism preservation (Q9):** Application-layer adapter required — ArcadeDB query results MUST be sorted by `entity_uuid` (logical identifier) before canonicalisation, NOT by `@rid` (insert-order).
- **Perf ballpark (Q10):** Cold-start ~350 ms (JVM warmup + first HTTP roundtrip); warm projection ~300-500 µs/event; full re-projection of 10M events ~50-80 min single-threaded, ~6-10 min workspace-parallel (8 workers).
- **CI strategy (Q7):** New `.github/workflows/atlas-arcadedb-smoke.yml` proposed for W17c with Docker-Compose. All 7 byte-determinism CI pins remain byte-identical.
- **FalkorDB fallback trigger thresholds (Q6):** Spike-doc §9 defines 5 measurable criteria.

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `docs/V2-BETA-ARCADEDB-SPIKE.md` | Spike-doc ~400 lines |
| NEW | `docs/ADR/ADR-Atlas-010-arcadedb-backend-choice-and-embedded-mode-tradeoff.md` | ADR ~320 lines |
| NEW | `.handoff/v2-beta-welle-16-plan.md` | This plan-doc ~140 lines |

**Total estimated diff:** ~860 lines (all NEW, doc-only).

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 16 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins | NONE — doc-only welle, no code paths touched | Per-pin diff is by definition empty |
| `cargo check --workspace` | NONE | No Rust touched |
| `cargo test --workspace` | NONE | No tests touched |
| pnpm workspace tests | NONE | No TS touched |
| ProjectorRunAttestation chain | NONE | No projector code touched |

**Mandatory check:** all 7 byte-determinism CI pins MUST remain byte-identical (trivially true — doc-only welle).

## Implementation steps (doc-only TDD-equivalent: research → claim → review)

1. Pre-flight: `git fetch origin && git checkout -B feat/v2-beta/welle-16-arcadedb-spike origin/master`
2. Write `.handoff/v2-beta-welle-16-plan.md` (this doc)
3. Write `docs/V2-BETA-ARCADEDB-SPIKE.md` — 11-section structure
4. Write `docs/ADR/ADR-Atlas-010-...md` — 9-section ADR structure
5. Dispatch parallel reviewer subagents: `code-reviewer` (doc-quality lens) + `security-reviewer` (Q3 tenant-isolation rigor)
6. Fix CRITICAL/HIGH in-commit
7. Single SSH-Ed25519 signed commit: `docs(v2-beta/welle-16): ArcadeDB embedded-mode spike + ADR-Atlas-010`
8. Push + open DRAFT PR base=master, head=feat/v2-beta/welle-16-arcadedb-spike
9. Report PR number + 1-paragraph spike conclusion to parent agent

## Acceptance criteria

- [ ] `cargo check --workspace` green (trivially — no Rust touched)
- [ ] `cargo test --workspace` green; all 7 byte-determinism CI pins byte-identical
- [ ] `docs/V2-BETA-ARCADEDB-SPIKE.md` answers all 10 spike questions with confidence-level annotations (HIGH/MEDIUM/LOW)
- [ ] ADR-Atlas-010 records: ACCEPT decision, considered options A/B/C, consequences, reversibility, watchlist
- [ ] DECISION-DB-4 either CONFIRMED or RE-EVALUATED with rationale
- [ ] GraphStateBackend trait sketch (~30-50 lines Rust pseudo-code) included in spike §7
- [ ] FalkorDB fallback trigger thresholds (5 criteria) measurable + documented
- [ ] Plan-doc on welle's own branch
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched; CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit (`SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [ ] DRAFT PR open with base=master
- [ ] Forbidden-files rule honoured

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Spike concludes embedded mode is the correct choice and overturns conventional wisdom | LOW | MED | ADR-010 §3 records both options; reversibility is HIGH |
| ArcadeDB version churn between W16 and W17 invalidates HTTP-API claims | LOW | LOW | Spike pins to ArcadeDB 24.x major; W17a re-verifies |
| Counsel parallel track surfaces SSPL §13 alternative interpretation | LOW | HIGH | Spike-doc §9 trigger #5 watches for this |
| HTTP client choice (`reqwest`) conflicts with future tokio-version requirements | LOW | LOW | W17a re-verifies; `reqwest` is most-maintained option |
| Workspace-level isolation (Q3) discovered insufficient post-W17 in production | LOW | HIGH | Spike §4.3 mandates application-layer + database-isolation defence in depth; W17c integration tests MUST include cross-tenant-query negative case |

## Out-of-scope this welle (later phases)

- **W17a (Phase 9 / V2-β):** Concrete `GraphStateBackend` Rust trait implementation + scaffold; ADR-Atlas-011.
- **W17b (Phase 9 / V2-β):** ArcadeDB HTTP driver implementation against the trait.
- **W17c (Phase 9 / V2-β):** Integration tests + Docker-Compose orchestration + new CI workflow.
- **W18 (Phase 10 / V2-β):** Mem0g Layer-3 cache integration on top of ArcadeDB-backed Layer-2.
- **Counsel-track Nelson-led:** Counsel-validated SSPL §13 opinion + EU-data-residency deployment validation.

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` §1 (W16→W17a edge), §5 (critical path) |
| ADR-Atlas-007 (parallel-projection design) | `docs/ADR/ADR-Atlas-007-parallel-projection-design.md` §6 (5 open questions resolved here) |
| ADR-Atlas-006 (structure reference) | `docs/ADR/ADR-Atlas-006-multi-issuer-sigstore-tracking.md` |
| DECISION-DB-4 (ArcadeDB primary lock) | `.handoff/decisions.md` |
| Master Vision Three-Layer Architecture | `.handoff/v2-master-vision-v1.md` §5 |
| ArcadeDB project | https://arcadedb.com |
| ArcadeDB HTTP API reference | https://docs.arcadedb.com/#HTTP-API |
