# ADR-Atlas-009 — Cypher-Validator Consolidation Rationale

| Field             | Value                                                              |
|-------------------|--------------------------------------------------------------------|
| **Status**        | Accepted                                                           |
| **Date**          | 2026-05-13                                                         |
| **Welle**         | V2-β Welle 15                                                      |
| **Authors**       | Nelson Mehlis (`@ThePyth0nKid`); welle-15 subagent                 |
| **Replaces**      | W12 inline `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.ts`; W13 inline `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts` |
| **Superseded by** | —                                                                  |
| **Related**       | DECISION-SEC-4 (Cypher Passthrough Hardening); ADR-Atlas-007 (parallel-projection design); V2-β Phase-4 cross-batch consistency-reviewer findings (W12 PR #79, W13 PR #77) |

---

## 1. Context

### 1.1 The rule-of-three discipline

Atlas's V2-β Phase-4 parallel batch (Welles 12 + 13 + 14) dispatched three independent subagents in isolated worktrees. W12 (atlas-web Read-API, PR #79) and W13 (MCP V2 tools, PR #77) each landed their own inline Cypher AST validator. This was intentional: the "rule-of-three" engineering discipline requires at least two independent implementations to exist side-by-side before extraction. Extracting too early produces premature abstraction; extracting after seeing drift produces a consolidation grounded in real usage evidence.

Both inline validators were annotated with `DO NOT import from the other package or pre-extract a shared module — that is W15's job.`

### 1.2 Cross-batch consistency-reviewer findings

After Phase 4 merged, a cross-batch consistency-reviewer agent analysed all three PRs together. It identified **five HIGH cross-welle inconsistencies** between W12 and W13's validator implementations:

1. **Length cap:** W12 had drifted to 16 KB; W13 had 4 096 chars. Resolved in-commit (W12 fix-commit `5212abc`).
2. **Passport stub semantics:** W12 returned `{ ok: false }`; W13's initial version returned `{ ok: true }`. Resolved in-commit (W13 fix-commit `47821ff`).
3. **Agent-DID echo cap:** W12 had 512-char cap; W13 had 1 024-char cap. Resolved in-commit (W13 fix-commit `47821ff`).
4. **Procedure-namespace regex:** W12 used bare-CALL rejection + broad `db.` pattern; W13 used explicit-CALL only (`CALL db.*` + `CALL dbms.*`). **Deferred to W15.**
5. **String-concat detection:** W12 rejected any `+` token; W13 rejected only quote-adjacent `+`. **Deferred to W15.**

Two inconsistencies (#4 and #5) required understanding the consolidated validator's intended semantics before resolving, making W15 the right resolution point.

### 1.3 Parameter-naming convention split

The HTTP consumer (atlas-web) uses `workspace` as the parameter name for the workspace identifier, following the V2-α atlas-signer CLI convention established in V2-α Welle 7. The MCP consumer (atlas-mcp-server) uses `workspace_id`, following the pre-existing MCP-package convention since V1.19 Welle 1.

The cross-batch consistency-reviewer flagged this as a HIGH finding. The post-batch parent agent resolved it as a **documented per-package convention**, not a code-level bug: the shared validator operates on the Cypher string only and does not inspect parameter names. The split is deferred to V2-γ or later for explicit cross-package reconciliation.

---

## 2. Decision

**Extract the shared Cypher read-only validator (regex-based; AST-level parsing deferred to V2-γ — see §8 Open Questions) into a new monorepo package `packages/atlas-cypher-validator/` with the package name `@atlas/cypher-validator`.**

Both inline implementations are deleted. Both consumers update their imports to `@atlas/cypher-validator`.

---

## 3. Decision drivers

1. **Prevent future drift.** Two inline copies + documented divergence = high probability of future inconsistency as each consumer's tests evolve independently. One source of truth eliminates the category.
2. **Resolve deferred HIGH findings.** Consistency-reviewer HIGH-4 (procedure regex) + HIGH-5 (string-concat rule) are resolved in the consolidated implementation by applying the union of both inline behaviours.
3. **Explicit API surface.** A package boundary makes the validator's contract explicit: what it exports, what its invariants are, and what version it is. Both consumers pin `workspace:*` so they always pick up the monorepo-local version.
4. **Future consumers.** A V2-γ CLI Cypher checker, a documentation linter, or any other surface that needs Cypher validation can depend on `@atlas/cypher-validator` without copying logic.
5. **Test consolidation.** W12 had 25 test cases (vitest); W13 had 27 test cases (custom tsx runner). The package ships 43 unified cases (deduplicated + union-semantics-specific additions) in vitest, providing one green suite that proves both W12's and W13's test coverage is met.

---

## 4. Considered options

### Option A: New monorepo package `packages/atlas-cypher-validator/` (RECOMMENDED — HIGH confidence)

**Mechanism:** mirror `packages/atlas-bridge/` conventions. New `package.json` (`@atlas/cypher-validator`, private, version 2.0.0-alpha.2), `tsconfig.json`, `src/validator.ts` (merged implementation), `src/index.ts` (public exports), `src/validator.test.ts` (unified tests). Both consumers add `"@atlas/cypher-validator": "workspace:*"` to their `dependencies` and update import paths.

**Pros:**
- Single source of truth for the validator's behaviour
- Explicit versioning + API surface
- pnpm workspace already supports `packages/*` (per `pnpm-workspace.yaml`)
- Future consumers can depend on it without monorepo coupling concerns
- Unified test file in one runner (vitest) — simpler CI reporting

**Cons:**
- Adds a package; increases workspace-member count by 1
- Both consumers must be updated (minor)

**Confidence:** HIGH. The pattern is proven by `packages/atlas-bridge/` which both consumers already use.

### Option B: Mirror module in each consumer (`apps/*/src/lib/cypher-validator/`) — REJECTED

**Mechanism:** copy the merged implementation into a well-named local path in each consumer. Still two files, but identically structured.

**Pros:** no new package; consumers self-contained

**Cons:**
- No real consolidation — drift risk remains (two files to update when the validator changes)
- No explicit versioning or API surface
- Tests remain split
- Future consumers still copy, not depend

**Decision:** REJECTED. Option B is a renaming exercise that does not solve the problem.

### Option C: Leave inline (rule-of-three not yet enforced) — REJECTED

**Mechanism:** do nothing; W15 noted as optional.

**Pros:** zero engineering effort

**Cons:**
- Phase 4 already surfaced five cross-welle inconsistencies. The rule-of-three trigger has fired. Deferring further accumulates drift.
- The two deferred HIGH findings (#4 and #5) remain open and unresolved.
- Future welles adding Cypher surfaces will copy a third time, at which point extraction is more disruptive.

**Decision:** REJECTED. Phase 4 evidence makes extraction the correct action now.

---

## 5. Implementation summary

### 5.1 Package layout

```
packages/atlas-cypher-validator/
  package.json          (@atlas/cypher-validator, private, 2.0.0-alpha.2, ESM)
  tsconfig.json         (NodeNext/NodeNext; mirrors atlas-bridge)
  src/
    validator.ts        (merged implementation; 6 invariants applied)
    index.ts            (public exports: validateReadOnlyCypher, CYPHER_MAX_LENGTH,
                          CypherValidationResult)
    validator.test.ts   (43 unified test cases; vitest)
```

### 5.2 Deleted files

- `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.ts` (W12 inline)
- `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.test.ts` (W12 tests)
- `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts` (W13 inline)
- `apps/atlas-mcp-server/scripts/test-cypher-validator.ts` (W13 tsx test script)

### 5.3 Modified callsites

- `apps/atlas-web/src/app/api/atlas/query/route.ts`:
  `from "../_lib/cypher-validator"` → `from "@atlas/cypher-validator"`
- `apps/atlas-mcp-server/src/tools/query-graph.ts`:
  `from "./_lib/cypher-validator.js"` → `from "@atlas/cypher-validator"`

---

## 6. The 6 consolidation invariants (from CHANGELOG `[Unreleased]` W15 entry criteria)

### 6.1 Forbidden-keyword set

**Decision:** union of W12 + W13 lists (identical post-consistency-fix). Both already aligned on the 10 mandatory write-keywords:

```
DELETE, DETACH DELETE, CREATE, MERGE, SET, REMOVE, DROP,
FOREACH, LOAD CSV, USING PERIODIC COMMIT
```

`DETACH DELETE` is checked before bare `DELETE` in both lists (order matters for error-message clarity).

### 6.2 Length cap

**Decision:** 4 096 chars (`CYPHER_MAX_LENGTH = 4096`). Aligned post-Phase-4-consistency-fix HIGH-1. W12 had drifted to 16 384; W13 had 4 096. The stricter W13 floor was applied to W12 in fix-commit `5212abc`. W15 extracts the shared constant.

### 6.3 Comment-stripping with `.trimStart()`

**Decision:** adopt `.trimStart()` after comment-strip universally. This is W13's correctness invariant (added in fix-commit `45dfa1b`): after `stripComments(trimmed)`, a leading block-comment like `/* */MATCH (n) RETURN n` becomes ` MATCH (n) RETURN n` (leading space). Without `.trimStart()`, the opener-allowlist check (`/^MATCH\b/i`) fails as a false-negative. W12's keyword-scan-only semantic made this optional, but the consolidated validator adopts the stricter approach uniformly.

### 6.4 Procedure-namespace regex unification

**Problem (deferred from Phase-4 consistency-reviewer):**
- W12: `/\bdb\s*\./i` (broad `db.*` — catches inline `db.foo()` without explicit `CALL`) + `/\bCALL\b/i` (bare-CALL rejection)
- W13: `/\bCALL\s+db\s*\./i` + `/\bCALL\s+dbms\s*\./i` (explicit-CALL only; would NOT catch `db.foo()` without `CALL`)

**Decision:** pick the UNION of both implementations:

| Pattern | Source | Rationale |
|---------|--------|-----------|
| `/\bapoc\s*\./i` | both | APOC sandbox-escape surface |
| `/\bdb\s*\./i` | W12 | Catches inline `db.foo()` without explicit `CALL` |
| `/\bCALL\s+dbms\s*\./i` | W13 | Catches explicit `CALL dbms.*` administration surface |
| `/\bCALL\b/i` | W12 | Bare-CALL rejection (no procedure allow-list at V2-β phase) |

The ordering in the deny-list is: `apoc.*` → `db.*` → `CALL dbms.*` → `CALL` (bare). This means the most specific message appears for `CALL apoc.x` (catches `apoc.*` first) and for `CALL db.labels()` (catches `db.` first via the broad pattern).

**Trade-off documented:** The bare-`CALL` rejection is conservative. It means that legitimate stored-procedure calls (e.g. `CALL gds.algo.*` for graph algorithms) are rejected. The correct mitigation at V2-γ is an explicit allow-list. For V2-β beta.1, false positives are preferred over false negatives.

**No regression:** W13's test corpus contained no test that passed a bare-CALL invocation. W12's test corpus rejected bare CALL (already aligned). The union adds zero false negatives vs either existing implementation.

### 6.5 String-concat detection

**Problem (deferred from Phase-4 consistency-reviewer):**
- W12: reject ANY `+` token in the post-literal-strip query
- W13: reject only quote-adjacent `+` (`/'\s*\+|\+\s*'/i`)

**Decision:** take W12's stricter rule (reject ANY `+` after literal-strip).

**Rationale:**
- Arithmetic concatenation (`RETURN n.a + n.b`) is technically valid Cypher but has no legitimate use in a parameter-bound read-only query. The proper pattern is to compute and bind via `$params`.
- List concatenation (`[1] + [2]`) is similarly unlikely in the Atlas query surface.
- The stricter rule closes an entire class of caller errors (building queries via string concatenation before the `params` object was the correct channel) at zero false-negative risk for the V2-β read-only use case.
- W13's narrower quote-adjacent heuristic would pass `RETURN n.a + n.b` (no quote-adjacent `+`), which is an inconsistency vs W12's rejected-in-all-cases stance.

**Implementation note:** `+` inside string literals must not trip the rule. The validator strips single-quoted and double-quoted literals before the `+` check. This preserves `MATCH (n {label: 'a+b'}) RETURN n` as a valid query.

### 6.6 Parameter naming (NOT reconciled here)

**Background:** The HTTP consumer uses `workspace` (atlas-web, V2-α Welle 7 convention). The MCP consumer uses `workspace_id` (atlas-mcp-server, V1.19 Welle 1 convention).

**Decision:** do NOT reconcile in W15. The shared validator operates on the Cypher string only. It does not inspect, require, or validate parameter names. Each consumer continues to accept its own per-package convention for the workspace identifier in the tool/route schema layer (Zod, etc.).

**Rationale:** Reconciling the naming split requires changing the MCP tool schema (affects all 5 MCP tools' `workspace_id` parameter) or the HTTP route schema (affects all 6 Read-API routes' `workspace` parameter). Either change is a breaking API change for consumers of those surfaces. V2-β keeps both conventions stable; V2-γ can reconcile if cross-package unification is desired. The ADR documents the split explicitly so future welle authors know where it stands.

---

## 7. Consequences

### 7.1 Positive

- **Single source of truth.** One implementation, one test suite, one version.
- **Deferred HIGHs resolved.** Consistency-reviewer HIGH-4 (procedure regex) and HIGH-5 (string-concat rule) are closed.
- **Stricter than either inline.** Union-semantics means the consolidated validator rejects MORE patterns than either W12 or W13 alone. This is the correct direction for a security-critical guard.
- **Future surfaces unblocked.** Any V2-γ surface needing Cypher validation adds `"@atlas/cypher-validator": "workspace:*"` and imports the same contract.
- **Test coverage unified.** 43 cases in vitest replace 25 vitest + 27 tsx-script cases across two locations.

### 7.2 Negative / trade-offs

- **Bare-CALL is rejected.** This is intentionally conservative. A future welle adds an allow-list when the production procedure surface is defined (V2-γ roadmap item).
- **String-concat rule is strict.** Queries that use `+` for arithmetic or list operations will be rejected. Atlas's query surface is parameter-bound by design; this is not expected to cause issues in practice.
- **One more package.** `packages/atlas-cypher-validator/` increases workspace-member count. Justified by the deduplication and explicit API surface gains.

### 7.3 Reversibility

**HIGH.** The package can be inlined back into either consumer trivially if the package boundary proves counterproductive. The implementation is ~180 lines; the move is mechanical. No external API contract is broken (the package is `private: true`).

---

## 8. Open questions

| Question | Status |
|----------|--------|
| Parameter naming reconciliation (`workspace` vs `workspace_id`) | Deferred to V2-γ or never; per-package convention preserved |
| AST-level Cypher parser (replace regex pass) | Deferred to V2-γ; V2-β ships regex-based pass with fail-closed semantics |
| Procedure allow-list (relax bare-CALL rejection) | Deferred to V2-γ; depends on defining the production procedure surface against ArcadeDB (W17) |
| Cross-package workspace-identifier convention unification | Deferred to V2-γ; requires coordinated API change across HTTP + MCP surfaces |

---

## 9. Byte-determinism impact

**None.** W15 touches TypeScript source only. Zero Rust files are modified. All 7 V2-α byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) remain byte-identical from the v2.0.0-alpha.2 baseline.

---

## 10. Reference pointers

| Concept | Source-of-truth |
|---------|-----------------|
| DECISION-SEC-4 | `.handoff/decisions.md` |
| W15 plan-doc | `.handoff/v2-beta-welle-15-plan.md` |
| Consolidated package | `packages/atlas-cypher-validator/` |
| W12 callsite | `apps/atlas-web/src/app/api/atlas/query/route.ts` |
| W13 callsite | `apps/atlas-mcp-server/src/tools/query-graph.ts` |
| atlas-bridge (package pattern reference) | `packages/atlas-bridge/` |
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` §3.4 (ADR number pre-assignment) |
| Consistency-reviewer findings | CHANGELOG.md `[Unreleased]` § "W15 Cypher-validator consolidation — entry criteria" |
