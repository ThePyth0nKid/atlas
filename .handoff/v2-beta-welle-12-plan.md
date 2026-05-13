# V2-β Welle 12 — Plan-Doc (Read-API endpoints)

> **Status:** DRAFT 2026-05-13. Awaiting parent agent's confirmation before merge.
> **Orchestration:** part of Phase 4 (parallel batch 2) per `docs/V2-BETA-ORCHESTRATION-PLAN.md`.
> **Driving decisions:** `DECISION-SEC-4` (Cypher injection / DoS hygiene — AST-level validation, no `apoc.*`, no `CALL db.*`, no string-concat, parse-time depth caps); also `DECISION-DB-1`/`DECISION-DB-4` (ArcadeDB-primary, V2-α Welle 2 spike-doc — read-side backend not yet wired so W12 lands the **API contract surface** with an in-memory `ProjectionStore` stub; Phase 7 / W17 wires the real backend).

Welle 12 lands six Next.js App-Router read-API routes under `apps/atlas-web/src/app/api/atlas/` plus a **welle-local** Cypher AST-validator. Five routes return real data backed by an in-memory `ProjectionStore` derived from `events.jsonl`; one route (`passport/[agent_did]`) returns HTTP 501 stub deferred to V2-γ. The Cypher validator is implemented inline per the rule-of-three deferral: V2-β Welle 15 consolidates the W12 + W13 (MCP V2 tools) validator into one shared module — extracting before two copies exist would violate the rule and bake an unmotivated abstraction.

**Why this as Welle 12:** zero file-area overlap with W13 (atlas-mcp-server tools) and W14 (Rust upsert crate) — the three Phase-4 parallel wellen partition cleanly. W12 establishes the API contract surface that Welle 17 (ArcadeDB driver) will back, and that Welle 15 will consolidate the validator from. Unblocks: W15 (Cypher-validator consolidation — needs ≥2 implementations to satisfy rule-of-three), W17 (ArcadeDB driver — needs API consumer to drive real-backend acceptance).

## Scope (table)

| In-Scope | Out-of-Scope |
|---|---|
| 6 Next.js route files under `apps/atlas-web/src/app/api/atlas/` | `CHANGELOG.md` (parent consolidates) |
| `_lib/cypher-validator.ts` (welle-local; W15 consolidates) | `docs/V2-MASTER-PLAN.md` §6 status table (parent) |
| `_lib/projection-store.ts` (in-memory stub; W17 wires ArcadeDB) | `docs/SEMVER-AUDIT-V1.0.md` (parent) |
| Vitest unit-test files for validator + handlers | `.handoff/decisions.md` (parent) |
| `apps/atlas-web/vitest.config.ts` + `package.json` devDep `vitest` | `.handoff/v2-session-handoff.md` (parent) |
| `.handoff/v2-beta-welle-12-plan.md` (this file) | `docs/V2-BETA-ORCHESTRATION-PLAN.md` (parent) |
| | Shared Cypher-validator module (W15 rule-of-three) |
| | Real ArcadeDB backend (W17a/b/c) |
| | Agent-DID-gated read access (V2-γ) |
| | Full Agent Passport endpoint (V2-γ; W12 ships 501 stub) |

## Decisions (final, pending parent confirmation)

- **Welle-local validator, not shared module:** `_lib/cypher-validator.ts` lives inside `apps/atlas-web/src/app/api/atlas/_lib/`. W13 ships its own copy. W15 extracts the shared module after rule-of-three. Premature abstraction would force W15 to inherit a possibly-wrong shape.
- **Validator implementation = regex-based, defensive but not perfect:** documented as such in source comments. Real AST parse is W15's job and likely uses a Cypher parser library. W12's validator must catch all enumerated forbidden patterns (`apoc.*`, `CALL db.*`, `DELETE/DETACH DELETE/CREATE/MERGE/SET/REMOVE/LOAD CSV/USING PERIODIC COMMIT`, naive string-concatenation `+`) but is permitted false positives on edge cases (e.g. `+` inside a single-quoted string literal). The fail-closed bias is intentional — V2-β beta.1 ships before the real validator.
- **`ProjectionStore` interface, in-memory `EventsJsonlProjectionStore` impl:** reads `events.jsonl` via `@atlas/bridge`'s `readAllEvents`. Builds entity/edge/event indexes. Cypher route returns HTTP 501 with a clear message ("Cypher read backend lands in V2-β Phase 7 / W17") — the validator runs and rejects malformed input even though execution is stubbed, so the validator's correctness is testable today.
- **`passport/[agent_did]` returns HTTP 501** with the V2-γ pointer per the dispatch spec. Documented in code comment.
- **Vitest as the test runner for atlas-web:** atlas-web previously had no unit-test runner; this welle introduces vitest with config consistent with `apps/wasm-playground/worker/vitest.config.ts` (Vitest ~2.1). Playwright remains the E2E layer; vitest is for `*.test.ts` colocated with routes.
- **`runtime = "nodejs"` + `dynamic = "force-dynamic"`** on every route — mirrors `write-node/route.ts`; the routes read from disk and cannot be statically rendered or cached.
- **No auth at route level** — Atlas's V1 trust model is key-based not user-based, and `OPERATOR-RUNBOOK §17` documents that operators are responsible for the upstream auth gate. V2-γ agent-passport-DID enforcement is deferred. Threat model identical to existing `write-node/route.ts`.
- **Workspace-id validation everywhere:** every route accepts a `workspace` query / param and validates it via the same `WORKSPACE_ID_PATTERN = /^[a-zA-Z0-9_-]{1,128}$/` regex `write-node` uses. Path-traversal structurally impossible.
- **Request-body cap on the `query` POST:** 256 KB hard limit on the raw body, mirroring write-node's belt-and-braces cap.
- **Limit enforcement on timeline:** `?limit` defaults to 50, hard-capped at 500.

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.ts` | Regex-based read-only Cypher validator. `validateReadOnlyCypher(query): { ok, reason? }`. ~90 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.test.ts` | 11 vitest cases (each forbidden pattern + happy paths + edge cases). ~140 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/_lib/projection-store.ts` | `ProjectionStore` interface + `EventsJsonlProjectionStore` impl (reads events.jsonl, builds in-memory entity/edge/event indexes). ~190 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/_lib/projection-store.test.ts` | Vitest cases for in-memory projection: empty workspace, single entity, edge, timeline-window, audit lookup. ~150 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/_lib/http.ts` | Tiny shared helpers: `jsonError`, `requireWorkspace`, `parseIsoOrThrow`. ~50 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/entities/[id]/route.ts` | GET — fetch entity by id, workspace-scoped. ~50 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/entities/[id]/route.test.ts` | Happy + 404 + missing-workspace 400. ~70 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/related/[id]/route.ts` | GET — outgoing + incoming edges. ~55 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/related/[id]/route.test.ts` | Happy + 404 + missing-workspace 400. ~70 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/timeline/route.ts` | GET — events in time window. ~65 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/timeline/route.test.ts` | Default-limit + custom-limit + clamp-to-max + invalid-iso 400. ~90 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/query/route.ts` | POST — Cypher (validated, stubbed exec). ~70 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/query/route.test.ts` | Validator-rejection 400 + payload-cap 413 + happy-but-501 path. ~85 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/audit/[event_uuid]/route.ts` | GET — event JSON + signature-verification status. ~55 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/audit/[event_uuid]/route.test.ts` | Happy + 404. ~70 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/passport/[agent_did]/route.ts` | GET — 501 stub for V2-γ. ~35 lines. |
| NEW | `apps/atlas-web/src/app/api/atlas/passport/[agent_did]/route.test.ts` | 501 status + body-shape. ~35 lines. |
| NEW | `apps/atlas-web/vitest.config.ts` | Vitest config. ~25 lines. |
| MODIFY | `apps/atlas-web/package.json` | Add `vitest` devDep + `test` script. |
| NEW | `.handoff/v2-beta-welle-12-plan.md` | This plan-doc. |

**Total estimated diff:** 1300-1500 lines (mostly tests + per-route files).

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 12 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins | NONE — welle adds new TS files, touches no Rust crate, no canonicalisation surface, no event wire format | Pins remain byte-identical; no Rust artifacts rebuild differently |
| Existing `write-node` route + tests | NONE — separate route directory; no shared code paths modified | Existing playwright suite passes unchanged |
| `@atlas/bridge` exports | NONE — read-only consumer of `readAllEvents`, `isValidWorkspaceId` | No bridge mutation |
| `pnpm build` in atlas-web | LOW — new vitest devDep shouldn't pollute production bundle (vitest is dev-only) | `vitest` excluded from runtime bundle; route files are pure server handlers |

**Mandatory check:** all 7 byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) MUST remain byte-identical after this welle's merge. W12 touches zero Rust files and zero canonicalisation surface.

## Implementation steps (TDD order)

```
1. Write Cypher validator tests FIRST (RED) — 11 cases covering each forbidden pattern + happy paths
2. Implement validator (GREEN)
3. Write ProjectionStore tests (RED) — interface + in-memory impl behaviour
4. Implement ProjectionStore (GREEN)
5. Write route-handler tests (RED) — happy + error per route
6. Implement route handlers (GREEN)
7. Run full vitest suite — green
8. Run parallel reviewer agents (code-reviewer + security-reviewer) on the diff
9. Fix CRITICAL/HIGH findings in-commit
10. SSH-signed single coherent commit
11. Push + open DRAFT PR with base=master
```

## Acceptance criteria

- [ ] Vitest unit suite green (validator + projection-store + 6 handlers)
- [ ] Validator covers 8+ test cases per dispatch spec (we ship 11)
- [ ] Each route has happy + error tests per dispatch spec
- [ ] No touches to forbidden files (CHANGELOG.md, V2-MASTER-PLAN.md status, decisions.md, semver-audit, handoff doc, orchestration plan)
- [ ] All new files are under `apps/atlas-web/src/app/api/atlas/` or `apps/atlas-web/src/app/api/atlas/_lib/` (plus the welle-12 plan-doc + vitest config + package.json devDep)
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched; CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit (`SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [ ] DRAFT PR open with base=master, head=`feat/v2-beta/welle-12-read-api`

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Validator false-negative on edge case (e.g. unicode escapes hiding `DELETE`) | MED | MED | Document limitation in source; W15 consolidation uses real AST parser. Reject anything ambiguous (`+` outside obvious string literal). |
| Validator false-positive blocks legitimate `MATCH (n {prop: "a+b"}) RETURN n` | LOW | LOW | Tests assert this case passes; regex looks for `+` between two non-quote-bounded tokens. Document remaining edge cases. |
| Vitest devDep introduces resolution conflict with existing playwright | LOW | LOW | vitest 2.1 + playwright 1.59 are independent test frameworks; coexist cleanly per `wasm-playground/worker` precedent. |
| Phase 4 cross-welle file conflict | NONE | — | Per `V2-BETA-DEPENDENCY-GRAPH.md` §3 Phase 4 matrix: W12 = `apps/atlas-web/src/app/api/atlas/` only; W13 = `apps/atlas-mcp-server/`; W14 = `crates/atlas-projector/`. Zero file-level overlap. |
| Vitest test runner not in CI yet | LOW | LOW | This welle adds the `test` script; CI integration is parent's consolidation responsibility post-batch (or W15 if CI changes are needed). |

## Out-of-scope this welle (later phases)

- **W15 (Phase 5):** consolidate `_lib/cypher-validator.ts` from W12 + W13's twin into a shared package or root-level module. Real AST-based parser. ADR-Atlas-009 documents the rationale.
- **W17a/b/c (Phase 7):** replace `EventsJsonlProjectionStore` with `ArcadeDBProjectionStore` per ADR-Atlas-011. The `ProjectionStore` interface is the seam; route handlers swap backend without changes.
- **V2-γ Agent Passports:** `passport/[agent_did]/route.ts` ships as 501 stub. Full implementation in V2-γ.
- **V2-γ Agent-DID-gated read access:** W12 has no read-side auth. V2-γ wires per-request DID enforcement.
- **CI workflow integration of vitest:** parent's Phase 4 consolidation commit may add the workflow step; W12 just ships the runnable script.

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` |
| Master Plan | `docs/V2-MASTER-PLAN.md` §6 |
| Cypher passthrough hardening | `.handoff/decisions.md` DECISION-SEC-4 |
| V1 reference route pattern | `apps/atlas-web/src/app/api/atlas/write-node/route.ts` |
| `@atlas/bridge` exports | `packages/atlas-bridge/src/index.ts` |
| Vitest precedent | `apps/wasm-playground/worker/vitest.config.ts` |

---

## Implementation Notes (Post-Code) — fill AFTER tests pass

### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| Cypher validator (regex-based, fail-closed) | `_lib/cypher-validator.ts` | (filled in commit) |
| ProjectionStore interface + EventsJsonl impl | `_lib/projection-store.ts` | (filled in commit) |
| Shared HTTP helpers | `_lib/http.ts` | (filled in commit) |
| 6 route handlers | `entities/`, `related/`, `timeline/`, `query/`, `audit/`, `passport/` | (filled in commit) |
| 8 test files (validator + projection-store + 6 routes) | colocated `*.test.ts` | (filled in commit) |
| Vitest config + package.json | `vitest.config.ts`, `package.json` | (filled in commit) |

### Test outcome

- Vitest suite green
- All 7 byte-determinism CI pins unchanged (welle touches no Rust)
- No existing test surface modified

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| (filled post-test-run) | — |

### Deviations from plan

(filled post-test-run)
