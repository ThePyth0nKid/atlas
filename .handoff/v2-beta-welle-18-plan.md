# V2-β Welle 18 — Plan-Doc (Mem0g Layer-3 Cache Design)

> **Status:** DRAFT 2026-05-15. Awaiting parent agent's confirmation before merge.
> **Orchestration:** part of Phase 12 (serial single-welle dispatch — design phase, no code) per `docs/V2-BETA-ORCHESTRATION-PLAN.md`. **Phase A** of W18 (design only); **Phase B** (first implementation code) deferred to W18b OR folded into W19 v2.0.0-beta.1 convergence ship per Nelson's call after design lands.
> **Driving decisions:** `DECISION-SEC-5` (Mem0g embedding-leakage secure-deletion); `DECISION-DB-3` (Mem0g latency-claim attribution honesty); `DECISION-DB-4` (Apache-2.0 license + JVM/Python avoidance pattern that informs Mem0g implementation choice); `DECISION-ARCH-1` (V2-α byte-determinism triple-hardening — W18 MUST preserve the 7 V2-α byte-pins); `DECISION-COUNSEL-1` (GDPR Art. 4(1) hash-as-PII opinion still in-flight; Mem0g embedding-as-derivative has parallel Art. 4(1) reasoning that the same counsel should advise on).

W18 designs Atlas's Layer 3 — the FAST, REBUILDABLE, NEVER-AUTHORITATIVE semantic cache that sits on top of the W17a-c-shipped Layer 2 ArcadeDB projection. Per `docs/V2-MASTER-PLAN.md` §3, Layer 3 enables semantic-search responses that **always cite back to a Layer 1 `event_uuid`** — the cache is for retrieval-speed, never trust-authority. Embeddings (floats) live OUTSIDE Atlas's canonicalisation pipeline (lib.rs invariant #3); the byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` MUST remain reproducible end-to-end after W18 ships.

**Why this as Welle 18:** W17c (Phase 11) validated ArcadeDB byte-determinism end-to-end through live CI. The Layer 2 surface is now stable — the trait + driver + integration tests are all green. W18 unblocks: (1) Atlas+Mem0g end-to-end benchmark publication (master-plan §6 success criterion); (2) the EU AI Act Art. 12 claim that Atlas's evidence is "queryable in semantic terms by humans + agents, not just by event ULID"; (3) W19 v2.0.0-beta.1 convergence — Layer 3 semantic-search is the third leg of the V2-β tripod (Layer 2 query / Layer 3 semantic / verifier-rebuild). Doing W18 as a **design-first welle** (no code) before W18b (implementation) follows the W16 → W17a/b/c pattern that successfully kept the ArcadeDB integration architecturally tight.

## Scope (table)

| In-Scope | Out-of-Scope |
|---|---|
| `docs/V2-BETA-MEM0G-SPIKE.md` (NEW, ~400-500 lines, ~11 sections analog `docs/V2-BETA-ARCADEDB-SPIKE.md`) | Production code in `crates/atlas-projector/src/` or `crates/atlas-mem0g/` (deferred to W18b) |
| `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` (NEW, ~250-400 lines, mirror ADR-Atlas-010 structure) | Real Mem0g vector-store benchmark numbers (deferred to W18b CI bench-test, per W17c B1/B2/B3 pattern) |
| `.handoff/v2-beta-welle-18-plan.md` (THIS FILE) | New event-kind dispatch arms in `crates/atlas-projector/src/upsert.rs` (e.g. `embedding_erased` audit-event) — design locked here, code deferred to W18b |
| README.md or docs/ARCHITECTURE.md updates referencing the design (only if needed for cross-link integrity) | New CI workflow `.github/workflows/atlas-mem0g-smoke.yml` (deferred to W18b) |
|  | Production `crates/atlas-mem0g/Cargo.toml` + dependency additions (deferred to W18b; the spike + ADR identify the dep set, W18b commits) |
|  | **Forbidden files (parent consolidates in Phase 12.5):** `CHANGELOG.md`, `docs/V2-MASTER-PLAN.md` (§6 status table), `docs/SEMVER-AUDIT-V1.0.md`, `.handoff/decisions.md`, `.handoff/v2-session-handoff.md`, `docs/V2-BETA-ORCHESTRATION-PLAN.md`, `docs/V2-BETA-DEPENDENCY-GRAPH.md` |

**Hard rule:** in line with W16's no-code design pattern, NO production code in this welle. The spike + ADR identify the implementation surface; W18b writes it.

## Decisions (final, pending parent confirmation)

These decisions are LOCKED in the spike + ADR. Each is documented for parent-review traceability:

- **Spike-first vs ADR-direct:** SPIKE-FIRST (Nelson decision 2026-05-15). R1 (Mem0g distribution-choice = one-way door for Hermes-skill cold-start) is HIGH-risk; spike does comparative analysis before ADR locks the choice. Mirrors W16 → ADR-Atlas-010 pattern.
- **Implementation choice:** LOCKED — **LanceDB embedded (`lancedb 0.29.0`) + fastembed-rs embedded (`fastembed-rs 5.13.4`)**, both pure-Rust Apache-2.0, both linked into a NEW workspace member crate `crates/atlas-mem0g/`. See spike §6 + ADR-Atlas-012 §4 sub-decision #1. Mem0-Python rejected on Hermes-skill distribution + cloud-default embedder + delegated secure-delete; Qdrant sidecar reserved as documented pivot (LP1-LP5 triggers in spike §9). Atlas owns secure-delete wrapper + embedder-determinism pinning + cache-invalidation strategy + GDPR-erasure audit-event emission end-to-end.
- **Embeddings outside canonicalisation pipeline:** LOCKED. Per lib.rs invariant #3 (no floats in canonicalised properties), embedding floats MUST NOT flow into `graph_state_hash` computation. Trust anchor for any Mem0g response is cite-back to `event_uuid` (Layer-1 reference). The byte-pin `8962c168...e013ac4` MUST remain reproducible end-to-end after W18 ships.
- **Cache-key strategy:** LOCKED at `event_uuid` (Layer-1 reference), NOT `embedding_hash`. Reasoning: if embeddings are non-deterministic across runs (e.g., model-version drift, CPU/GPU non-determinism), `embedding_hash` would invalidate the cache spuriously; `event_uuid` is the stable invariant.
- **Layer authority (corrects Phase 1 Doc B misreading flagged by `crit-architect.md` H-3):** Mem0g indexes Layer 1 events.jsonl directly (NOT via Layer 2 ArcadeDB). Cache rebuild is a Layer-1 → Layer-3 operation that does NOT depend on Layer 2 availability. ADR §4 documents this as binding sub-decision.
- **Secure-delete primitive:** LOCKED — overwrite-then-unlink, NOT just unlink (per `DECISION-SEC-5`). Spike §4.4 validates that the chosen tool exposes overwrite primitive OR Atlas wraps file-system fillbytes-then-unlink explicitly. ADR §4 sub-decision #4 commits the primitive.
- **GDPR-erasure parallel-audit-event shape:** new event-kind name LOCKED in ADR §4 sub-decision #5. Candidate: `embedding_erased` (parallels `anchor_created` shape — minimal payload: `event_id` of the erased Layer-1 event, `erased_at` timestamp, optional `reason_code`). Dispatch arm added in W18b to `crates/atlas-projector/src/upsert.rs`.
- **Bench-test shape:** LOCKED — same `#[ignore]`-gated pattern as W17c's `tests/arcadedb_benchmark.rs`. New `tests/mem0g_benchmark.rs` (W18b) runs B4 (cache-hit semantic-search latency p50/p95/p99), B5 (cache-miss-with-rebuild full-rebuild cost), B6 (secure-delete primitive correctness — write embedding, erase originating event, verify embedding bytes overwritten on disk).

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `docs/V2-BETA-MEM0G-SPIKE.md` | ~500 lines, 13 sections. Comparative spike analog `docs/V2-BETA-ARCADEDB-SPIKE.md`. Resolves the 6 design questions enumerated in handoff §0-NEXT. |
| NEW | `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` | ~340-380 lines, mirror ADR-Atlas-010 structure. 9 sections. **8 binding sub-decisions** in §4 (the spike's 6 design questions plus crate-boundary [#7] + bench-shape [#8] sub-decisions that the ADR promotes from the spike's exec-summary into binding §4 entries). OQs tracked in §6. |
| NEW | `.handoff/v2-beta-welle-18-plan.md` | THIS FILE. ~280 lines. |

**Total estimated diff:** ~1100-1200 lines, all NEW files. Zero MODIFY in this welle (forbidden-files rule + design-only scope).

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 18 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) | NONE — design-only welle, zero production code | Pre-merge parent runs `cargo test -p atlas-projector --test backend_trait_conformance --quiet` + `cargo test -p atlas-trust-core --quiet` and verifies all green; byte-pin `8962c168...e013ac4` MUST reproduce |
| `cargo clippy -p atlas-projector --no-deps -- -D warnings` | NONE — no Rust touched | Pre-merge parent verifies zero warnings |
| `cargo test --workspace` | NONE — no Rust touched | Pre-merge parent verifies green |
| `atlas-web-playwright` required CI check | TRIGGERED via `.handoff/**` path filter (per Atlas Lesson #11 — doc-only PRs need a `.handoff/**` touch to satisfy the required check on crates-only branches; this PR hits `.handoff/v2-beta-welle-18-plan.md` so it auto-triggers) | Verify CI run lands green before admin-merge |
| `Verify trust-root-modifying commits` required CI check | NONE — no `.github/`/`tools/expected-master-ruleset.json`/`.github/allowed_signers` touches in this welle | Routine SSH-Ed25519 signed commit suffices |
| `atlas-arcadedb-smoke` workflow | NONE — no `crates/atlas-projector/` or `infra/docker-compose.arcadedb-smoke.yml` touches | N/A (still optional, not yet required-check per §0-NEXT note) |

**Mandatory check:** all 7 byte-determinism CI pins MUST remain byte-identical after this welle's merge. Design-only welles preserve them by construction (no Rust touched), but the parent verifies anyway.

## Implementation steps (TDD order)

Adapted for design-doc welle (no test/code cycle in the traditional TDD sense):

1. **Pre-flight (already done by parent):** `git fetch origin && git checkout -B feat/v2-beta/welle-18-mem0g-design origin/master`. `git status` clean.
2. **Research:** dispatch general-purpose subagent with WebSearch for current 2026-05 state of Mem0/Mem0g/LanceDB/Qdrant/fastembed-rs/sqlite-vec/SurrealDB. (Done — agent `ae4e665cfd3645a23` running in background.)
3. **Plan-doc:** populate THIS FILE per template. (DRAFT in progress.)
4. **Spike-doc:** write `docs/V2-BETA-MEM0G-SPIKE.md` with research data integrated. Lock 6+ design questions in §6 Recommendation.
5. **ADR-Atlas-012:** distil spike's recommendations into binding sub-decisions in §4. Mirror ADR-Atlas-010's structure (Status/Date/Welle/Authors/Replaces/Superseded by/Related header → 9 sections).
6. **Local verification:** `cargo test -p atlas-projector --quiet` + `cargo test -p atlas-trust-core --quiet` + `cargo clippy --workspace --no-deps -- -D warnings` all green. (Sanity check that no inadvertent code touches happened.)
7. **Parallel reviewer dispatch (Atlas Standing Protocol Lesson #8):** parent dispatches `code-reviewer` + `security-reviewer` agents in single-message-2-Agent-calls. For doc-only PR they check claim-drift across spike ↔ ADR ↔ plan-doc ↔ master-plan ↔ existing decisions, factual accuracy on Mem0/LanceDB/etc, secure-delete-design soundness, embedding-leakage threat-model coverage, GDPR-Art-17 audit-event shape correctness.
8. **Fix CRITICAL/HIGH in-commit:** any reviewer-driven changes go into the same SSH-signed commit (or a follow-up squash before push). Per Atlas Standing Protocol Lesson #3, MEDIUMs about doc-conventions and factual-claim drift are non-optional; only V2-γ-scope-cleanups defer.
9. **SSH-signed single coherent commit:** standard `git commit -S` with conventional commit message `feat(v2-beta/welle-18): Mem0g Layer-3 cache design — V2-BETA-MEM0G-SPIKE + ADR-Atlas-012 + plan-doc`.
10. **Push + open PR:** `git push -u origin feat/v2-beta/welle-18-mem0g-design`; `gh pr create` with comprehensive body referencing spike + ADR + locked sub-decisions count + risk-mitigation status.
11. **Wait for required CI checks:** `gh run watch <run-id> --exit-status` until both `Verify trust-root-modifying commits` and `atlas-web-playwright` green. (Background, non-polling.)
12. **Admin-merge:** `gh pr merge <n> --admin --squash --delete-branch` per pre-authorised settings.
13. **Phase 12.5 consolidation (separate welle, parent-led, post-merge):** updates `CHANGELOG.md`, `docs/V2-MASTER-PLAN.md` §6 status row, `.handoff/decisions.md` (DECISION-ARCH-W18 entry), `docs/V2-BETA-ORCHESTRATION-PLAN.md` Welle-18 status flip, `docs/V2-BETA-DEPENDENCY-GRAPH.md` (W18 → W18b/W19 edge), `.handoff/v2-session-handoff.md` (refresh §0-NEXT to point to W18b OR W19 entry).

## Acceptance criteria

- [ ] `cargo check --workspace` green (sanity — no inadvertent code touches)
- [ ] `cargo test -p atlas-projector --quiet` + `cargo test -p atlas-trust-core --quiet` green; byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces
- [ ] `cargo clippy --workspace --no-deps -- -D warnings` zero warnings
- [ ] `docs/V2-BETA-MEM0G-SPIKE.md` exists; **6 design questions answered** with confidence levels (HIGH/MEDIUM-HIGH/MEDIUM/LOW per question) in §4; **13 sections total**
- [ ] `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` exists; **8 binding sub-decisions** in §4 (#1 vector-store/embedder choice; #2 embedder model + determinism + supply-chain controls; #3 cache-key + Layer authority; #4 secure-delete primitive; #5 GDPR audit-event; #6 cache-invalidation; #7 crate boundary; #8 bench-shape + timing-side-channel mitigation); ≥9 OQs in §6 for V2-γ tracking
- [ ] `.handoff/v2-beta-welle-18-plan.md` (THIS FILE) on this welle's branch
- [ ] Parallel `code-reviewer` + `security-reviewer` dispatched; CRITICAL = 0; HIGH = 0 OR fixed in-commit; reviewer-finding count + resolutions captured in PR body
- [ ] Single SSH-Ed25519 signed commit (key fingerprint `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [ ] PR open with base=master, body references spike + ADR + plan-doc + locked sub-decisions count
- [ ] Forbidden-files rule honoured (no touches to CHANGELOG.md, V2-MASTER-PLAN.md status, decisions.md, semver-audit, handoff doc, orchestration-plan, dependency-graph)
- [ ] `atlas-web-playwright` required CI check triggered via `.handoff/**` path filter and green
- [ ] `Verify trust-root-modifying commits` required CI check green
- [ ] Admin-merge succeeds via `gh pr merge --admin --squash --delete-branch`

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| **R-W18-1 Spike research surfaces unforeseen library deprecation** (e.g. LanceDB pivots license, Mem0 changes deployment model since training-data cutoff) | MED | LOW (we're in design phase; pivot cost is just spike-doc revision) | Research subagent uses WebSearch to verify current 2026-05 state; "what I could not verify" section in research output flags assumptions; spike documents fallback for each option |
| **R-W18-2 Embedder choice locks long-term cold-start budget** (one-way door for Hermes-skill `npx` install — model size + first-load time) | HIGH | MED | ADR §4 sub-decision #2 explicitly addresses; embedder-swap path documented (fastembed-rs supports multiple models; swap is a config change not a code-rewrite) |
| **R-W18-3 Secure-delete primitive design relies on tool API that may change** (LanceDB / Qdrant API for explicit overwrite-then-unlink not yet stable across major versions) | MED | HIGH (GDPR Art. 17 compliance is structural; cannot ship beta without it) | Spike §4.4 validates against current API; ADR §4 sub-decision #4 documents fallback ("if chosen tool deprecates the API, Atlas wraps explicit fillbytes-then-unlink with verification read") |
| **R-W18-4 Embedding-determinism non-determinism breaks cache-key invariants** (model-version drift, CPU/GPU non-determinism, fastembed-rs ONNX runtime variance) | MED | MED (cache mis-hit, not data corruption — cache rebuilds from Layer 1 cleanly) | LOCKED design: cache-key is `event_uuid` (Layer-1 reference), NOT `embedding_hash`. Embeddings are content-addressed by their source event, not by their byte representation. Spike validates this is robust under embedder-swap |
| **R-W18-5 Cache-rebuild concurrent-with-projector race** (projector emits new event during Mem0g rebuild → rebuild produces stale snapshot that misses the new event) | MED | LOW-MED (next rebuild trigger catches up; trust property unaffected because Layer 1 always wins) | ADR §4 sub-decision #6: rebuild is content-addressed by `events.jsonl` byte-length-at-rebuild-start; if projector appends after rebuild starts, rebuild reruns OR caches the gap and replays incrementally. Operator-runbook documents the rebuild-trigger conditions |
| **R-W18-6 Atlas+Mem0g end-to-end benchmark exceeds operational latency budget** (cache-miss-with-rebuild on a 10M-event workspace adds seconds to a Read-API query) | LOW (CI-time) / MED (ops-time) | HIGH (ops) | Bench shape (B5) captures cache-miss-with-rebuild baseline in CI; operator-runbook documents rebuild-trigger thresholds; T2-equivalent fallback trigger for Layer 3 documented in spike §9 |
| **R-W18-7 Reviewer-driven changes invalidate the spike's locked sub-decisions** (e.g. security-reviewer flags secure-delete-design as insufficient → ADR §4 needs structural rewrite) | LOW (the W17a/b/c reviewer cycle was efficient; no structural ADR rewrites surfaced) | MED (pushes W18 + 1 session) | Per Atlas Standing Protocol Lesson #8, parent dispatches reviewers EARLY (post-spike-draft, BEFORE ADR-12 final lock); allows incorporating reviewer findings into ADR §4 first-pass not as a hotfix |
| **R-W18-8 Counsel-engagement Art. 17 opinion arrives mid-W18 + invalidates Path-B assumption** (Path-A redesign per `DECISION-COMPLIANCE-3`) | LOW (counsel timeline is 6-8 weeks from engagement letter; current state is pre-engagement) | LOW (Path-A redesign is salt-management work, doesn't touch Mem0g design) | Counsel track is parallel; W18 design doesn't depend on opinion outcome. ADR §4 documents Mem0g design under Path-B-current; Path-A-fallback section notes the embedding-storage adjustment if needed |

## Out-of-scope this welle (later phases)

- **Phase 12.5 / Phase 13 W18b implementation:** new crate `crates/atlas-mem0g/` (or extension to `atlas-projector`), `cargo` dep additions per spike §6 recommendation, `crates/atlas-projector/src/upsert.rs` `embedding_erased` dispatch arm, `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts` Read-API endpoint, `crates/atlas-projector/tests/mem0g_benchmark.rs` B4/B5/B6 bench test, `.github/workflows/atlas-mem0g-smoke.yml` CI workflow.
- **Phase 13 W19 v2.0.0-beta.1 ship:** convergence milestone (Layer 2 ArcadeDB + Layer 3 Mem0g + verifier-rebuild all operational); workspace version bump + signed tag + GitHub Release + npm publish.
- **V2-γ later:** Mem0g-as-Hermes-skill operational metrics; semantic-search rate-limiting at endpoint level (DECISION-SEC-4 hardens Cypher; semantic-search is a separate operational concern); multi-region Mem0g replication; Mem0g sharding for >100M events.

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` |
| Master Plan | `docs/V2-MASTER-PLAN.md` §3 (Layer 3 spec) + §6 (Welle decomposition) |
| Working Methodology | `docs/WORKING-METHODOLOGY.md` |
| Three-Layer Trust Architecture | `docs/V2-MASTER-PLAN.md` §3 |
| Master Vision (full V2 vision + Phase-2-critique provenance) | `.handoff/v2-master-vision-v1.md` Doc B §3-5 |
| Mem0g embedding-leakage decision | `.handoff/decisions.md` `DECISION-SEC-5` |
| Mem0g latency-claim attribution | `.handoff/decisions.md` `DECISION-DB-3` |
| ArcadeDB spike (structural template) | `docs/V2-BETA-ARCADEDB-SPIKE.md` |
| ArcadeDB ADR (template for ADR-Atlas-012 structure) | `docs/ADR/ADR-Atlas-010-arcadedb-backend-choice-and-embedded-mode-tradeoff.md` |
| Layer-2 trait + dispatch surface (parallel structural reference for Layer-3 trait) | `crates/atlas-projector/src/backend/mod.rs` |
| Event-kind dispatch arm placement (W18b adds `embedding_erased` arm here) | `crates/atlas-projector/src/upsert.rs` |
| Bench-test pattern (W18b B4/B5/B6 mirror this) | `crates/atlas-projector/tests/arcadedb_benchmark.rs` |
| Phase-2 Architect H-3 dependency-graph correction (Mem0g indexes Layer 1, NOT Layer 2) | `.handoff/crit-architect.md` (lives only on Phase-2 draft branch PR #61; archived) |
| V2-α byte-determinism invariants | `crates/atlas-projector/src/lib.rs` invariants #1-#5 |

---

## Implementation Notes (Post-Code) — fill AFTER docs are written + reviewer-dispatched

```
### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| <fill after spike + ADR drafted> | <file> | <count> |

### Test outcome

- All 7 byte-determinism CI pins unchanged (verified pre-merge)
- `cargo test --workspace` green (verified pre-merge)
- `cargo clippy --workspace --no-deps -- -D warnings` zero warnings (verified pre-merge)

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| <fill from R-W18-1..8 above as each risk's mitigation is exercised> | |

### Deviations from plan

<deviations + rationale, or "None of substance">
```

---

## Subagent dispatch prompt skeleton (anti-divergence enforcement, for W18b)

When the parent agent dispatches the W18b implementation subagent, the prompt MUST include:

```text
Atlas project at C:\Users\nelso\Desktop\atlas. V2-β Welle 18b — Mem0g Layer-3 cache implementation.
Master HEAD at time of dispatch: <commit-sha-post-W18-merge>.

## Your goal
Implement the Mem0g Layer-3 cache per the W18-shipped design. The spike + ADR lock the implementation choice + secure-delete primitive + bench shape — your job is fill-in-the-blanks code, NOT design.

## Pre-flight (FIRST 3 actions — non-negotiable, Atlas Lesson #1)
1. `git fetch origin`
2. `git checkout -B feat/v2-beta/welle-18b-mem0g-impl origin/master` (master HEAD at dispatch: <current-master-sha-post-W18-merge>)
3. `git status` → clean

## Pre-flight reading (master-resident, mandatory)
1. `docs/V2-BETA-MEM0G-SPIKE.md` (W18-shipped) — design questions answered + recommendation
2. `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` (W18-shipped) — 8 binding sub-decisions
3. `.handoff/v2-beta-welle-18-plan.md` (W18-shipped) — context + risk register
4. `crates/atlas-projector/src/backend/{mod.rs,in_memory.rs,arcadedb/mod.rs}` — Layer-2 trait + impls (parallel structural reference)
5. `crates/atlas-projector/src/upsert.rs` — event-kind dispatch surface (W18b adds `embedding_erased` arm here)
6. `crates/atlas-projector/tests/arcadedb_benchmark.rs` — bench-test pattern (W18b mirrors with B4/B5/B6)
7. `.handoff/decisions.md` `DECISION-SEC-5` (secure-delete contract)

## In-scope files
<W18b-specific scope list — derived from ADR §4 sub-decisions>

## Forbidden files (parent consolidates these post-batch)
- CHANGELOG.md, docs/V2-MASTER-PLAN.md (status), docs/SEMVER-AUDIT-V1.0.md, .handoff/decisions.md, .handoff/v2-session-handoff.md, docs/V2-BETA-ORCHESTRATION-PLAN.md, docs/V2-BETA-DEPENDENCY-GRAPH.md
- docs/V2-BETA-MEM0G-SPIKE.md (W18-shipped, do NOT modify in W18b)
- docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md (W18-shipped, do NOT modify; if implementation surfaces a design adjustment, add an ADR-Atlas-012-amendment NEW file)

## Hard rules (Atlas Standing Protocol)
- Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` MUST remain reproducible. Run `cargo test -p atlas-projector --test backend_trait_conformance byte_pin --quiet` as final go/no-go.
- No floats in canonical bytes (V2-α invariant #3). Mem0g embeddings live OUTSIDE canonicalisation.
- SSH-Ed25519 signed commits only. No `--no-verify`.
- Parent ALWAYS dispatches parallel `code-reviewer` + `security-reviewer` post-implementation (Atlas Lesson #8).

## Acceptance criteria (parent verifies all before approving merge)
- ADR-Atlas-012's **8 sub-decisions** implemented faithfully; deviations documented in W18b plan-doc
- B4/B5/B6 benchmarks captured in CI artifact
- GDPR `embedding_erased` audit-event round-trips through projector + verifier
- Cite-back to `event_uuid` works end-to-end (semantic-search returns Layer-1 reference, not just embedding similarity score)
- Reviewer dispatch outputs: 0 unresolved CRITICAL, 0 unresolved HIGH

## Output (under 500 words)
PR number + URL, line counts, byte-pin-survives-W18b evidence, B4/B5/B6 numbers, secure-delete primitive validation evidence (B6 cycle: write embedding → erase originating event → verify embedding bytes overwritten on disk via raw-file-read), reviewer-finding counts + resolutions, any unexpected deviations.
```

This skeleton is mandatory; deviations are flagged by the parent agent's review.

---

**End of W18 plan-doc.** Spike + ADR are this welle's primary deliverables; implementation is W18b's job.
