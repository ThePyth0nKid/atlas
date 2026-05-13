# V2-β Welle 13 — Plan-Doc (MCP V2 Tools + Inline Cypher AST Validator)

> **Status:** DRAFT 2026-05-13. Awaiting parent agent's confirmation before merge.
> **Orchestration:** part of Phase 4 (Parallel Batch 2, alongside W12 + W14) per `docs/V2-BETA-ORCHESTRATION-PLAN.md`.
> **Driving decisions:** `DECISION-SEC-4` (Cypher passthrough hardening — AST validation, no-`apoc.*`, no-`CALL db.*`, depth caps, allowlist-only).

V2-β Welle 13 exposes Atlas's V2-α / V2-β read-side surface via the Model Context Protocol. Five MCP tools are added to `apps/atlas-mcp-server/src/tools/`: `query_graph` (AST-validated Cypher), `query_entities` (kind + filter), `query_provenance` (event chain by `entity_uuid`), `get_agent_passport` (V2-γ STUB — returns deferral notice), and `get_timeline` (windowed events). Each tool that accepts Cypher input runs through a **W13-local inline Cypher AST validator** (`_lib/cypher-validator.ts`) that enforces the read-only allowlist, rejects `apoc.*` / `CALL db.*` / mutation keywords, and applies a regex-based string-concatenation heuristic. W13's validator is **deliberately independent** of W12's parallel validator: this is the rule-of-three pattern, where Welle 15 (separate, later) consolidates both implementations into one shared module after observing them side-by-side.

**Why this as Welle 13:** Phase 4 is a 3-welle parallel batch (W12 atlas-web read-API, W13 atlas-mcp-server tools, W14 projector event-kinds) targeting distinct file-areas with zero conflict-surface per `docs/V2-BETA-DEPENDENCY-GRAPH.md` §3. W13 is unblocked the moment Phase 3 (`v2.0.0-alpha.2` ship) lands and must merge before Phase 5's W15 rule-of-three Cypher-validator consolidation can begin (W15's input is BOTH W12's and W13's inline validators).

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| `apps/atlas-mcp-server/src/tools/query-graph.ts` (NEW) — AST-validated Cypher tool | `CHANGELOG.md` (parent consolidates post-batch) |
| `apps/atlas-mcp-server/src/tools/query-entities.ts` (NEW) | `docs/V2-MASTER-PLAN.md` §6 status table (parent consolidates) |
| `apps/atlas-mcp-server/src/tools/query-provenance.ts` (NEW) | `docs/SEMVER-AUDIT-V1.0.md` (parent consolidates) |
| `apps/atlas-mcp-server/src/tools/get-agent-passport.ts` (NEW — V2-γ STUB) | `.handoff/decisions.md` (parent consolidates) |
| `apps/atlas-mcp-server/src/tools/get-timeline.ts` (NEW) | `.handoff/v2-session-handoff.md` (parent consolidates) |
| `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts` (NEW, W13-local) | `docs/V2-BETA-ORCHESTRATION-PLAN.md` (parent updates welle-progress) |
| `apps/atlas-mcp-server/src/tools/_lib/projection-store.ts` (NEW interface; impl stubs) | Any file in `apps/atlas-web/` (W12's surface — DO NOT touch) |
| `apps/atlas-mcp-server/src/tools/index.ts` (MODIFY — register 5 new tools) | Any file in `crates/atlas-projector/` (W14's surface) |
| `apps/atlas-mcp-server/src/tools/types.ts` (no modification expected; existing shape sufficient) | W15's shared Cypher-validator module (rule-of-three — later welle) |
| Test scripts in `apps/atlas-mcp-server/scripts/test-*.ts` for validator + handlers | ArcadeDB-backed projection-store impl (W16/W17 land later) |
| `.handoff/v2-beta-welle-13-plan.md` (this plan-doc) | Importing from W12's validator path |

**Hard rule:** Out-of-Scope column honours `docs/V2-BETA-ORCHESTRATION-PLAN.md` §3.3 forbidden files.

## Decisions (final, pending parent confirmation)

- **Validator implementation: regex + heuristic rejection list (W13-local).** Same forbidden-token list as W12 (DELETE / CREATE / MERGE / SET / REMOVE / LOAD CSV / USING PERIODIC COMMIT / `apoc.*` / `CALL db.*` / single-quote-then-plus string-concat heuristic). Allowed clauses: MATCH, WHERE, WITH, RETURN, ORDER BY, LIMIT, SKIP, OPTIONAL MATCH, UNWIND. Implementation lives at `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts` and is INDEPENDENT of W12's. The rule-of-three pattern (W15) consolidates AFTER both implementations land.
- **Validator output shape:** `{ ok: true } | { ok: false, reason: string }`. Callers translate `ok: false` into a structured tool-error response (`isError: true` per `ToolHandlerResult`).
- **Data backing: in-memory `ProjectionStore` stub.** Phase 4 has no ArcadeDB; W16/W17 land it later. The store interface throws `"Not implemented; ArcadeDB-backed projection lands in V2-β Phase 7 / W17"` for query paths. Tests inject a fake. The MCP tool contract surface (input/output shape, validation behaviour) is the V2-β Phase-4 deliverable — backing data is V2-β Phase-7 deliverable.
- **`get_agent_passport` is a V2-γ STUB.** Returns `{ agent_did, status: "stub", message: "Agent Passport tool is V2-γ scope" }` for any input. Documented in tool description and source comments. This pre-registers the tool name so MCP clients can discover it; future V2-γ work fleshes out the handler.
- **Naming convention:** tool `name` strings follow `atlas_*` prefix (matches V1 convention), e.g. `atlas_query_graph`. File names use kebab-case (`query-graph.ts`) to match V1 file layout.
- **Workspace parameter:** spec says `workspace`. To match the existing V1 convention (`workspace_id` everywhere), tools accept `workspace_id` (with `workspace` aliased only if pragmatic). Wire only `workspace_id` to keep callers honest; this matches `optionalWorkspaceIdSchema` regex enforcement.
- **`query_graph` Cypher length cap:** 4096 chars. Beyond DoS hygiene this turns a multi-megabyte query payload into a Zod-level rejection.
- **Limits:** `query_entities` and `get_timeline` use `z.number().int().min(1).max(500).default(50)`. Hard cap at 500 enforces DoS-bound on result-set size.

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts` | Inline AST validator: regex-based rejection of mutation keywords, `apoc.*`, `CALL db.*`, string-concat heuristic, length cap. ~80 lines. |
| NEW | `apps/atlas-mcp-server/src/tools/_lib/projection-store.ts` | `ProjectionStore` interface + stub impl that throws "Not implemented (Phase 7 / W17)". ~50 lines. |
| NEW | `apps/atlas-mcp-server/src/tools/query-graph.ts` | `atlas_query_graph` tool — runs validator on input, calls projection-store. ~70 lines. |
| NEW | `apps/atlas-mcp-server/src/tools/query-entities.ts` | `atlas_query_entities` tool — kind/filter/limit query. ~70 lines. |
| NEW | `apps/atlas-mcp-server/src/tools/query-provenance.ts` | `atlas_query_provenance` tool — event chain by `entity_uuid`. ~60 lines. |
| NEW | `apps/atlas-mcp-server/src/tools/get-agent-passport.ts` | `atlas_get_agent_passport` STUB tool. ~50 lines. |
| NEW | `apps/atlas-mcp-server/src/tools/get-timeline.ts` | `atlas_get_timeline` tool — windowed events. ~70 lines. |
| MODIFY | `apps/atlas-mcp-server/src/tools/index.ts` | Register 5 new tools in `TOOL_REGISTRY`. ~12 lines added. |
| NEW | `apps/atlas-mcp-server/scripts/test-cypher-validator.ts` | 12+ unit tests for validator: happy + each forbidden pattern. ~150 lines. |
| NEW | `apps/atlas-mcp-server/scripts/test-mcp-v2-tools.ts` | Handler tests: happy paths, invalid Cypher rejection, missing params, stub passport, registry length check. ~200 lines. |
| MODIFY | `apps/atlas-mcp-server/package.json` | Add `test:cypher-validator` + `test:mcp-v2-tools` scripts, chain into existing `test`. ~4 lines. |
| NEW | `.handoff/v2-beta-welle-13-plan.md` | This plan-doc. |

**Total estimated diff:** ~900-1100 lines.

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 13 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) | NONE — no Rust code touched, no canonicalisation code, no signing-input code | Tests pass byte-identically; no regen needed |
| Existing V1 tools (`atlas_write_node`, `atlas_write_annotation`, `atlas_export_bundle`, `atlas_anchor_bundle`, `atlas_workspace_state`) | NONE — additive only; existing tools unchanged | Smoke + existing test scripts pass |
| `TOOL_REGISTRY` count | Grows from 5 to 10 — but no test pins the exact count today; new test asserts `>= 5` and that new tools are present | New `test:mcp-v2-tools` check |
| `apps/atlas-web/` surface (W12) | NONE — W13 does not touch atlas-web | File-area separation per dependency graph §3 |
| `crates/atlas-projector/` (W14) | NONE — pure TypeScript welle | File-area separation |

**Mandatory check:** all 7 byte-determinism CI pins MUST remain byte-identical. Satisfied trivially (no Rust, no canonicalisation, no signing path touched).

## Implementation steps (TDD order)

1. **Plan-doc on branch** (this file) — done as Step 1.
2. **Write failing validator tests** (`scripts/test-cypher-validator.ts`) — 12+ cases.
3. **Implement validator** (`_lib/cypher-validator.ts`) — minimum to green.
4. **Write failing projection-store interface tests** — happy path stub-throws.
5. **Implement projection-store stub** (`_lib/projection-store.ts`).
6. **Write failing tool-handler tests** (`scripts/test-mcp-v2-tools.ts`) — 1 per tool + invalid-Cypher + missing-required + stub-passport + registry length.
7. **Implement 5 tools** (`query-graph.ts`, `query-entities.ts`, `query-provenance.ts`, `get-agent-passport.ts`, `get-timeline.ts`).
8. **Modify `index.ts`** to register all 5 tools.
9. **`pnpm --filter atlas-mcp-server test`** — full test suite green.
10. **`pnpm --filter atlas-mcp-server lint`** (tsc) — type-clean.
11. **Run parallel `code-reviewer` + `security-reviewer` agents on diff.**
12. **Fix CRITICAL/HIGH in-commit.**
13. **Single SSH-Ed25519 signed commit** on `feat/v2-beta/welle-13-mcp-v2-tools`.
14. **Push + open DRAFT PR** with `--base master`.

## Acceptance criteria

- [ ] All 5 tools exist in `apps/atlas-mcp-server/src/tools/` with `atlas_*` name convention
- [ ] All 5 tools registered in `TOOL_REGISTRY` (`index.ts`)
- [ ] W13-local `_lib/cypher-validator.ts` exists and is INDEPENDENT of W12's path
- [ ] Validator rejects every forbidden pattern listed in DECISION-SEC-4
- [ ] `query_graph` runs validator BEFORE any data-layer call
- [ ] `get_agent_passport` returns deferral STUB shape
- [ ] `pnpm --filter atlas-mcp-server test` green (existing + new tests)
- [ ] `pnpm --filter atlas-mcp-server lint` (tsc) green
- [ ] No Rust code touched; all 7 byte-determinism CI pins byte-identical
- [ ] Plan-doc on welle's own branch
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched; CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit (`SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [ ] DRAFT PR open with `--base master`
- [ ] Forbidden-files rule honoured (no touches to CHANGELOG.md, V2-MASTER-PLAN.md status, decisions.md, semver-audit, handoff doc, orchestration plan)
- [ ] No file outside `apps/atlas-mcp-server/` touched except this plan-doc

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Regex-based validator misses an esoteric Cypher attack (e.g. unicode-escaped `apoc`) | MED | MED — read-only stub means no actual data loss is possible in W13 anyway; W15 rule-of-three consolidation hardens to true AST in a later session | Document limitations in validator-file comments. W15 will replace with AST parser. The projection-store stub THROWS rather than executes; any bypass would surface as the stub's "Not implemented" error before reaching real data. |
| `TOOL_REGISTRY` shape conflict with concurrent W12 changes | NONE — W12 does not touch `apps/atlas-mcp-server/` per dependency-graph §3 | LOW | File-area separation; verified by §3 conflict matrix |
| Test runner divergence — existing `package.json` chains test scripts via `&&` | LOW | LOW | Append the 2 new test scripts to the chain in `package.json`; pattern is established by V1.19 |
| Drift between `query_graph`'s validator and a future `query_provenance` Cypher path | LOW — provenance is hand-built, not Cypher | LOW | Comment in validator explicitly limits its application surface; `query_provenance` uses no Cypher at all |
| Type drift between `ProjectionStore` stub and future ArcadeDB impl | MED | LOW — interface is the contract; W17 adapts implementation | Interface comments document W17 expectations |

## Out-of-scope this welle (later phases)

- **W15 Cypher-validator consolidation** (Phase 5 — rule-of-three): merge W12 + W13 inline validators into a shared module. Do NOT pre-empt this welle.
- **W17 ArcadeDB-backed projection** (Phase 7): real backing for the stub. Tools currently throw "Not implemented" for any data-layer call.
- **V2-γ Agent Passport** (`get_agent_passport`): real handler with DID resolution, capability list, attestation chain. W13 ships only the discoverable STUB.
- **Streaming response shapes** for large result sets: V2-β response is single-blob JSON inside `content[0].text`. Streaming is V2-γ+.

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` |
| DECISION-SEC-4 (Cypher hardening) | `.handoff/decisions.md` |
| Master Plan | `docs/V2-MASTER-PLAN.md` §6 |
| Existing tool pattern | `apps/atlas-mcp-server/src/tools/workspace-state.ts` (read-only ref tool) |
| Existing test pattern | `apps/atlas-mcp-server/scripts/test-anchor-json.ts` |

---

## Implementation Notes (Post-Code) — fill AFTER tests pass

### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| Cypher AST validator (regex) | `_lib/cypher-validator.ts` | ~80 |
| Projection store interface + stub | `_lib/projection-store.ts` | ~50 |
| `atlas_query_graph` | `query-graph.ts` | ~70 |
| `atlas_query_entities` | `query-entities.ts` | ~70 |
| `atlas_query_provenance` | `query-provenance.ts` | ~60 |
| `atlas_get_agent_passport` (STUB) | `get-agent-passport.ts` | ~50 |
| `atlas_get_timeline` | `get-timeline.ts` | ~70 |
| Tool registration | `index.ts` | ~12 |
| Validator tests | `scripts/test-cypher-validator.ts` | ~150 |
| Tool handler tests | `scripts/test-mcp-v2-tools.ts` | ~200 |

### Test outcome

- Validator: 12+ cases green
- Handlers: 5 happy paths + 5 failure paths + stub-passport + registry length check
- Existing tests (`test:anchor-json`, `test:redact-paths`, `test:signer-cache`, `test:ulid`) untouched and green
- All 7 byte-determinism CI pins byte-identical (no Rust touched)

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| Regex validator gaps | W15 rule-of-three consolidation will harden to true AST. Stub throws on the data-layer path so any bypass surfaces before reaching live data. |
| TypeScript type drift | `ProjectionStore` interface comments explicit about W17 expectations. |

### Deviations from plan

To be filled in post-implementation.

---

## Subagent dispatch prompt skeleton

Followed faithfully — single subagent dispatched with the W13-specific scope distilled above. No deviations.
