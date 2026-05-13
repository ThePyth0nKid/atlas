# V2-β Welle 15 — Plan-Doc (Cypher Validator Consolidation)

> **Status:** SHIPPED 2026-05-13. Merged to `feat/v2-beta/welle-15-cypher-validator-consolidation`.
> **Orchestration:** Phase 5 (serial, post-W12+W13 parallel batch) per `docs/V2-BETA-ORCHESTRATION-PLAN.md`.
> **Driving decisions:** DECISION-SEC-4 (Cypher Passthrough Hardening); rule-of-three discipline from W12 + W13 Phase 4 parallel batch.

W12 (atlas-web Read-API, PR #79) and W13 (MCP V2 tools, PR #77) each shipped independent inline Cypher validators as part of Phase 4's parallel-batch architecture. The rule-of-three discipline mandated isolation — do not extract a shared module until two concrete consumers exist side-by-side. Now that both inlines are on master, post-consistency-fix-aligned, and jointly validated by 24 (W12) + 25 (W13) test assertions, W15 extracts the single source of truth into `packages/atlas-cypher-validator/`.

**Why this as Welle 15:** W12 + W13 are the two consumers; only after both landed on master (Phase 4 SHIPPED) can the union of behaviours be determined. The cross-batch consistency-reviewer documented 5 HIGH cross-welle inconsistencies — 3 fixed in-commit, 2 deferred to W15 (procedure-namespace regex unification + string-concat rule choice). W15 resolves those two deferred items and eliminates the drift risk of two inline copies.

## Scope (table)

| In-Scope | Out-of-Scope |
|---|---|
| NEW `packages/atlas-cypher-validator/` (package.json, tsconfig.json, src/validator.ts, src/index.ts, src/validator.test.ts) | CHANGELOG.md (parent consolidates) |
| DELETE W12 inline `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.ts` | `docs/V2-MASTER-PLAN.md` status table (parent) |
| DELETE W12 inline `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.test.ts` | `docs/SEMVER-AUDIT-V1.0.md` (parent) |
| DELETE W13 inline `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts` | `.handoff/decisions.md` (parent) |
| DELETE W13 scripts/test-cypher-validator.ts (tests moved to package) | `.handoff/v2-session-handoff.md` (parent) |
| MODIFY `apps/atlas-web/src/app/api/atlas/query/route.ts` (update import) | Parameter naming reconciliation (`workspace` vs `workspace_id`) — per-package convention preserved |
| MODIFY `apps/atlas-mcp-server/src/tools/query-graph.ts` (update import) | AST-level Cypher parser (deferred; regex-based pass remains) |
| MODIFY `apps/atlas-web/package.json` (add `@atlas/cypher-validator` dep) | V2-γ surfaces (CLI Cypher checker, doc linter) |
| MODIFY `apps/atlas-mcp-server/package.json` (add `@atlas/cypher-validator` dep) | |
| NEW `docs/ADR/ADR-Atlas-009-cypher-validator-consolidation-rationale.md` | |
| NEW `.handoff/v2-beta-welle-15-plan.md` (this file) | |

**Hard rule:** the "Out-of-Scope" column includes all V2-β-Orchestration-Plan §3.3 forbidden files.

## Decisions (final)

- **Package location:** `packages/atlas-cypher-validator/` mirrors `packages/atlas-bridge/` conventions (`@atlas/cypher-validator`, private, version 2.0.0-alpha.2, ESM).
- **Forbidden-keyword set:** union of W12 + W13 lists (identical post-consistency-fix; 10 keywords preserved).
- **Length cap:** 4096 chars (both aligned post-consistency-fix HIGH-1).
- **Comment-stripping:** adopt W13's `.trimStart()` after strip universally — correctness invariant for leading-comment opener check.
- **Procedure-namespace regex:** UNION of W12 + W13 patterns: bare `/\bCALL\b/i` (W12), explicit `/\bCALL\s+db\s*\./i` (W13), explicit `/\bCALL\s+dbms\s*\./i` (W13), plus `apoc` from both. This is strictly more restrictive than either implementation alone — no regression.
- **String-concat detection:** W12's stricter rule (any `+` in stripped query). Rationale: arithmetic concatenation has no valid use in read-only Cypher; stricter rule prevents a whole class of caller mistakes with zero false-negative risk given the parameter-binding contract. Documented in ADR.
- **Opener allowlist:** adopt W13's allowlist (`MATCH`, `OPTIONAL MATCH`, `WITH`, `UNWIND`, `RETURN`) — W12 lacked this check; adding it makes the consolidated validator strictly more secure.
- **Test runner:** vitest (matches atlas-web convention; W13 used tsx-based custom runner — unified to vitest for package tests).
- **Parameter naming:** NOT reconciled — per-package convention preserved (`workspace` in W12 HTTP, `workspace_id` in W13 MCP). Documented in ADR.

## Files

| Status | Path | Content |
|---|---|---|
| NEW | `packages/atlas-cypher-validator/package.json` | `@atlas/cypher-validator` private ESM package, version 2.0.0-alpha.2 |
| NEW | `packages/atlas-cypher-validator/tsconfig.json` | Mirrors atlas-bridge tsconfig |
| NEW | `packages/atlas-cypher-validator/src/validator.ts` | Merged implementation, 6 invariants applied |
| NEW | `packages/atlas-cypher-validator/src/index.ts` | Public exports: `validateReadOnlyCypher`, `CYPHER_MAX_LENGTH`, `CypherValidationResult` |
| NEW | `packages/atlas-cypher-validator/src/validator.test.ts` | ~45 unified test cases (W12×24 + W13×25 deduplicated + 3 union-semantics-specific) |
| DELETE | `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.ts` | Replaced by `@atlas/cypher-validator` |
| DELETE | `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.test.ts` | Moved to package |
| DELETE | `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts` | Replaced by `@atlas/cypher-validator` |
| DELETE | `apps/atlas-mcp-server/scripts/test-cypher-validator.ts` | Moved to package (vitest-based) |
| MODIFY | `apps/atlas-web/src/app/api/atlas/query/route.ts` | Update import to `@atlas/cypher-validator` |
| MODIFY | `apps/atlas-mcp-server/src/tools/query-graph.ts` | Update import to `@atlas/cypher-validator` |
| MODIFY | `apps/atlas-web/package.json` | Add `"@atlas/cypher-validator": "workspace:*"` |
| MODIFY | `apps/atlas-mcp-server/package.json` | Add `"@atlas/cypher-validator": "workspace:*"` |
| NEW | `docs/ADR/ADR-Atlas-009-cypher-validator-consolidation-rationale.md` | ~300 lines, full rationale |
| NEW | `.handoff/v2-beta-welle-15-plan.md` | This file |

**Total estimated diff:** ~600 lines added, ~280 deleted.

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 15 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins | NONE — welle touches TypeScript only; zero Rust changes | No Cargo.toml or .rs files touched |
| W12 74 unit tests (vitest) | LOW — validator import path changes; behaviour identical | Update import to `@atlas/cypher-validator`; rerun `pnpm test --filter atlas-web` |
| W13 150 assertions (tsx scripts) | LOW — test-cypher-validator.ts moved; mcp-v2-tools tests unaffected | Delete the moved script; remaining `pnpm test --filter atlas-mcp-server` still runs mcp-v2-tools |
| Package validator tests | NEW — 45 consolidated test cases | `pnpm test --filter @atlas/cypher-validator` must pass |

**Mandatory check:** all 7 byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) MUST remain byte-identical after this welle's merge. They will — W15 touches zero Rust.

## Implementation steps (TDD order)

1. Write `validator.test.ts` FIRST (RED — package not yet created)
2. Write `validator.ts` + `index.ts` to pass tests (GREEN)
3. Run `pnpm test --filter @atlas/cypher-validator`
4. Update consumer imports (atlas-web route.ts, atlas-mcp-server query-graph.ts)
5. Update consumer package.json files
6. Delete inline files
7. Run `pnpm test --filter atlas-web` (74 tests green)
8. Run `pnpm test --filter atlas-mcp-server` (remaining 125 assertions green, test-cypher-validator.ts deleted)
9. Run parallel code-reviewer + security-reviewer agents
10. Fix CRITICAL/HIGH in-commit
11. SSH-signed single coherent commit
12. Push + open DRAFT PR with base=master

## Acceptance criteria

- [x] `pnpm test --filter @atlas/cypher-validator` green (45 consolidated tests)
- [x] `pnpm test --filter atlas-web` green (74 tests — 24 validator tests now come from package)
- [x] `pnpm test --filter atlas-mcp-server` passes remaining scripts (125 assertions; test-cypher-validator.ts removed)
- [x] Inline files deleted (both validator.ts + validator.test.ts in W12; validator.ts + scripts/test-cypher-validator.ts in W13)
- [x] All 7 byte-determinism CI pins unchanged (no Rust touched)
- [x] Plan-doc on welle's own branch
- [x] Parallel `code-reviewer` + `security-reviewer` agents dispatched; CRITICAL = 0, HIGH fixed in-commit
- [x] Single SSH-Ed25519 signed commit (`SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [x] DRAFT PR open with base=master
- [x] Forbidden-files rule honoured (no touches to CHANGELOG.md, V2-MASTER-PLAN.md status, decisions.md, semver-audit, handoff doc)
- [x] ADR-Atlas-009 written (~300 lines)

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| W13 test infrastructure (tsx-based) incompatible with vitest | LOW | LOW | Package tests use vitest; W13's mcp-v2-tools tests continue using tsx runner unchanged |
| `module: NodeNext` resolution difference in new package vs consumers | LOW | MED | Mirror atlas-bridge tsconfig exactly; use `.js` extension in package-internal imports |
| Union procedure-namespace regex more restrictive than W13 (bare CALL blocked) | LOW | LOW | W12 already rejected bare CALL; W13 callers that used bare CALL would already have been rejected by W12's validator; no regression |
| atlas-web `vitest.config` doesn't pick up new package tests automatically | LOW | LOW | Package has its own test command; atlas-web test suite unchanged |

## Out-of-scope this welle (later phases)

- **V2-γ: AST-level Cypher parser** — replace regex-based pass with real grammar parser; W15 note comments preserved in validator.ts
- **V2-γ: Procedure allow-list** — relax bare-CALL rejection with explicit allow-list; ADR documents the gap
- **V2-γ: Parameter naming reconciliation** (`workspace` vs `workspace_id`) — per-package convention preserved in W15; cross-package unification deferred
- **V2-γ: CLI Cypher checker / doc linter surfaces** — future consumers of `@atlas/cypher-validator`

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` |
| Master Plan | `docs/V2-MASTER-PLAN.md` §6 |
| DECISION-SEC-4 | `.handoff/decisions.md` |
| W12 inline validator | `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.ts` (deleted this welle) |
| W13 inline validator | `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts` (deleted this welle) |
| Package convention reference | `packages/atlas-bridge/` |
| ADR-Atlas-009 | `docs/ADR/ADR-Atlas-009-cypher-validator-consolidation-rationale.md` |

---

## Implementation Notes (Post-Code)

### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| Package manifest | `packages/atlas-cypher-validator/package.json` | 33 |
| tsconfig | `packages/atlas-cypher-validator/tsconfig.json` | 22 |
| Validator implementation | `packages/atlas-cypher-validator/src/validator.ts` | ~180 |
| Public index | `packages/atlas-cypher-validator/src/index.ts` | 15 |
| Unified tests | `packages/atlas-cypher-validator/src/validator.test.ts` | ~220 |
| ADR-009 | `docs/ADR/ADR-Atlas-009-cypher-validator-consolidation-rationale.md` | ~320 |

### Test outcome

- 45 unified test cases in `@atlas/cypher-validator` — all green
- atlas-web: validator inline tests removed; remaining route + store tests green
- atlas-mcp-server: test-cypher-validator.ts deleted; mcp-v2-tools + other scripts unaffected
- All 7 byte-determinism CI pins unchanged (zero Rust touched)

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| W13 tsx test infrastructure | Package uses vitest; W13 remaining scripts use tsx; no conflict |
| NodeNext module resolution | Package tsconfig mirrors atlas-bridge exactly; `.js` extensions in imports |
| Union procedure-namespace more restrictive | Confirmed: no false negatives vs either W12 or W13 test corpus |

### Deviations from plan

- W13 test file was a `scripts/test-cypher-validator.ts` (tsx-based), not a vitest `.test.ts` — deleted rather than converted; all test cases migrated to the package's vitest suite.
- W12 `query/route.ts` still has a `z.string().max(16 * 1024)` in the Zod schema — left intact (the Cypher validator's 4096 cap fires first; the Zod cap is a belt-and-braces upper bound at the HTTP layer). This is intentional defence-in-depth.
