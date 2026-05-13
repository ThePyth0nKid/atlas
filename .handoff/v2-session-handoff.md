# Atlas V2 — Session Handoff (V2-α SHIPPED + V2-β Phase 0–8.5 SHIPPED, v2.0.0-alpha.2 LIVE)

> **🎯 FRESH-AGENT BOOTSTRAP DOC.** Wenn du diese Datei zum ersten Mal liest mit leerem Kontext: lies §0 "Fresh-Context Onboarding" zuerst (current state), dann §0a–§0d (Phase 1–4 of strategic-iteration SHIPPED narratives, historical), dann **`docs/V2-MASTER-PLAN.md`** für den master-resident strategischen Plan, dann **`docs/V2-BETA-ORCHESTRATION-PLAN.md`** + **`docs/V2-BETA-DEPENDENCY-GRAPH.md`** für V2-β welle orchestration + dispatch architecture. Dann optional **`.handoff/v2-master-vision-v1.md`** für die volle V2-Vision-Begründung und **`.handoff/decisions.md`** für die 23 expliciten Entscheidungen. Damit bist du bereit für V2-β-Phase-6 (W15 Cypher-validator consolidation, rule-of-three extraction) ohne strategischen Kontext nochmal abzufragen.

**Erstellt:** 2026-05-12. **V2-α-α.1 SHIPPED:** 2026-05-13 (8 Welles). **V2-β Phase 0–8.5 SHIPPED:** 2026-05-13 (one extraordinary work-day, 17 PRs merged: #67-#83). **Status:** v2.0.0-alpha.2 LIVE on master + GitHub + npm `@atlas-trust/verify-wasm@2.0.0-alpha.2` with Sigstore Build L3 provenance. Master HEAD post-Phase-8.5 = the PR that merges this commit. **Was als nächstes:** V2-β Phase 9 = W17a ArcadeDB driver scaffold + `GraphStateBackend` trait + InMemoryBackend wire-up + ArcadeDbBackend stub + ADR-Atlas-011. Architectural decisions locked by W16 (`docs/V2-BETA-ARCADEDB-SPIKE.md` + ADR-010): server mode, `reqwest`, one-database-per-workspace, byte-determinism adapter contract (`ORDER BY entity_uuid ASC` + stored `edge_id` property), 3-layer tenant-isolation defence. Then W17b (full driver impl) → W17c (Docker-Compose CI + integration tests + benchmark capture) → W18 Mem0g cache → W19 v2.0.0-beta.1 ship. Counsel-Engagement-Kickoff parallel-track ongoing (Nelson-led).

---

## 0. Fresh-Context Onboarding (read THIS FIRST if you're a new session)

**Wer bist du, wo bist du, was tust du?**

- **Repo:** `C:/Users/nelso/Desktop/atlas` (Windows-Host, bash/MSYS verfügbar, `cargo` lebt unter `/c/Users/nelso/.cargo/bin/cargo.exe`, `gh` lebt unter `/c/Program Files/GitHub CLI/gh.exe` — beide NICHT im default PATH).
- **State:** Atlas v2.0.0-alpha.2 ist LIVE auf master + GitHub + npm `@atlas-trust/verify-wasm@2.0.0-alpha.2` seit 2026-05-13 (Sigstore Build L3 provenance preserved; W11 wasm-publish race fix validated end-to-end). V1 abgeschlossen (v1.0.1 LIVE 2026-05-12). V2-Strategie 4-phasig komplettiert. V2-α 8-Welles SHIPPED 2026-05-12 → 2026-05-13. **V2-β Phase 0 + 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 8.5 ALL SHIPPED 2026-05-13** in 17 PRs (#67-#83). Key landings: PR #71 orchestration framework; PRs #72/#73/#74 W9-W11 docs+workflow parallel batch (v2.0.0-alpha.2 candidate); PR #76 v2.0.0-alpha.2 release; PRs #77/#78/#79 W12-W14 code parallel batch; PR #81 W15 Cypher-validator consolidation into `packages/atlas-cypher-validator/`; PR #83 W16 ArcadeDB spike + ADR-Atlas-010 (locks W17 architectural decisions). **Nächste Implementierungsarbeit = V2-β Phase 9 = W17a ArcadeDB driver scaffold.** Pre-flight reading mandatory: (1) `docs/V2-BETA-ARCADEDB-SPIKE.md` §7 (trait sketch), §4.3 (Layer 3 actually enforces mutation hardening NOT workspace_id presence), §4.9 (byte-determinism adapter contract); (2) `docs/ADR/ADR-Atlas-010-...md` §4 sub-decisions 1-8 (binding for W17a); (3) `docs/V2-BETA-ORCHESTRATION-PLAN.md` §2 (W17a row); (4) `.handoff/v2-beta-welle-N-plan.md.template` for plan-doc skeleton.
- **Methodik:** 4-Phasen-Framework aus `.handoff/v2-iteration-framework.md` (jetzt auch dokumentiert als reusable pattern in `docs/WORKING-METHODOLOGY.md`). Phase 1 = parallel-foundation-docs. Phase 2 = parallel-multi-angle-crits. Phase 3 = semi-manual synthesis. Phase 4 = master-plan + working-methodology landen auf master.
- **Was bereits passiert ist (Phase 1+2+3+4 alle 2026-05-12):**
  - **Phase 1** — 5 Foundation Docs parallel von 5 Subagents in eigenen Worktrees geschrieben (~2811 Zeilen). Auf PR #59 (draft, no-merge — work-product).
  - **Phase 2** — 6 Multi-Angle Crits parallel von 6 Subagents in eigenen Worktrees geschrieben (~1299 Zeilen). Auf PR #61 (draft, no-merge, base=PR-59-branch — work-product).
  - **Phase 3** — Master Vision v1 (~615 Zeilen) + decisions.md (22 Entscheidungen, ~284 Zeilen) durch semi-manual synthesis erstellt. Auf PR #62 (draft, no-merge, base=PR-61-branch — work-product).
  - **Phase 4** — `docs/V2-MASTER-PLAN.md` (~300 Zeilen) + `docs/WORKING-METHODOLOGY.md` (~200 Zeilen) auf master via PR #60 gemerged. Plus `.handoff/v2-master-vision-v1.md` + `.handoff/decisions.md` mitgemerged für master-reference-ability.
- **Was als nächstes ansteht (V2-α + parallel-Counsel-Track):**
  - **V2-β Phase 9** = **W17a ArcadeDB driver scaffold + `GraphStateBackend` trait + ADR-Atlas-011.** SERIAL `architect` subagent. Writes: (1) `crates/atlas-projector/src/backend/mod.rs` — production `GraphStateBackend` trait per spike §7 sketch (Vertex/Edge structs with V2-α Welle 1 stamping fields, `WorkspaceTxn` per-task transaction handle, `vertices_sorted`/`edges_sorted` byte-determinism-adapter methods, default `canonical_state()` impl); (2) `crates/atlas-projector/src/backend/in_memory.rs` — `InMemoryBackend` impl wrapping existing V2-α state.rs+upsert.rs+canonical.rs logic (preserves byte-determinism — same `graph_state_hash` hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`); (3) `crates/atlas-projector/src/backend/arcadedb.rs` — stub (compiles, all methods `unimplemented!()` with W17b placeholder docs); (4) `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` — trait design rationale (object-safety choice for `WorkspaceTxn`, batch-upsert ergonomics for Option-A parallel projection, integration with existing ProjectorRunAttestation chain); (5) `.handoff/v2-beta-welle-17a-plan.md` plan-doc. Acceptance: existing 169+85 V2-α tests still green; new InMemoryBackend trait-conformance tests added; ArcadeDbBackend stub compiles. ~1 session.
  - **V2-β Phase 10** = W17b ArcadeDB driver impl using `reqwest` + Cypher per ADR-010 §4 sub-decisions. Cross-backend byte-determinism test `tests/cross_backend_byte_determinism.rs` MUST pass (`InMemoryBackend::canonical_state()` byte-identical to `ArcadeDbBackend::canonical_state()` for same input events.jsonl).
  - **V2-β Phase 11** = W17c integration tests + new `.github/workflows/atlas-arcadedb-smoke.yml` (Docker-Compose with ArcadeDB sidecar) + benchmark capture (replaces ADR-010 §4.10 estimates with measured numbers; embedded-mode reconsideration trigger at p99 > 15 ms).
  - **V2-β Phase 12** = W18 Mem0g Layer-3 cache. Depends on W17 ArcadeDB stable. ADR-Atlas-012 reserved.
  - **V2-β Phase 13** = W19 v2.0.0-beta.1 ship. Convergence milestone — ArcadeDB-backed Layer 2 operational, all V2-β wellen merged.
  - **Counsel-Engagement-Kickoff** (€30-80K, 6-8 Wochen, pre-V2-β-public-materials blocking per `DECISION-COUNSEL-1`) — Nelson selects 1 lead firm from shortlist (Hogan Lovells Frankfurt / Bird & Bird Munich / Matheson / William Fry / Cleary Gottlieb Paris / boutique alternatives), scope per `.handoff/v2-master-vision-v1.md` §11. **Parallel-track, Nelson-led, NOT engineering-pipeline-dispatchable.**
  - **First-10-Customers Pipeline + TAM/SAM/SOM** — Nelson-led actions per `DECISION-BIZ-3` + `DECISION-BIZ-4`
- **Was du NICHT tust ohne Nelson:** V2-α-Welle-1-Engineering NICHT auto-starten. Counsel-Engagement Vendor-Auswahl ist Nelson's call. Erstcustomer-Pipeline ist Nelson's call. Pause vor jeder Strategie-Decision die in `.handoff/decisions.md` als reversibility=LOW oder MEDIUM markiert ist.

**State diagram of branches & PRs (post-Phase-4-merge):**

```
master  <new-HEAD-after-PR-60-merge>
  ├─ docs/V2-MASTER-PLAN.md             ← Phase 4 output, primary V2 plan
  ├─ docs/WORKING-METHODOLOGY.md        ← Phase 4 output, reusable 4-phase pattern
  ├─ .handoff/v2-master-vision-v1.md    ← Phase 3 output (mirrored for reference-ability)
  ├─ .handoff/decisions.md              ← Phase 3 output (mirrored for reference-ability)
  └─ .handoff/v2-session-handoff.md     ← this file, Phase 1+2+3+4 SHIPPED state

PR #60 (MERGED on Phase 4 ship): docs/v2/phase-1-shipped-handoff-update → master
  └─ contained all Phase-4 outputs + this handoff in single SSH-signed commit

PRs #59 #61 #62 (DRAFT, permanently-no-merge — work-product archives):
  PR #59 (v2/phase-1-foundation): 5 Phase-1 Foundation Docs (~2811 lines)
  PR #61 (v2/phase-2-critiques, base=#59): 6 Phase-2 Multi-Angle Crits (~1299 lines)
  PR #62 (v2/phase-3-master-vision, base=#61): Master Vision v1 + decisions.md (~899 lines)
```

These three PRs are NOT merge targets — they are audit-trail artefacts proving the methodology was applied. Per `docs/WORKING-METHODOLOGY.md` anti-pattern table: "Only Phase 4 touches master."

**The five Phase-1 docs (READ-only-reference; Phase 3 supersedes operationally):**
- `.handoff/v2-vision-strategic-positioning.md` (Doc A, 512 lines)
- `.handoff/v2-vision-knowledge-graph-layer.md` (Doc B, 727 lines v0.5)
- `.handoff/v2-risk-matrix.md` (Doc C, 457 lines)
- `.handoff/v2-competitive-landscape.md` (Doc D, 630 lines)
- `.handoff/v2-demo-sketches.md` (Doc E, 485 lines)

**The six Phase-2 crits (READ-only-reference; Phase 3 incorporates all CRITICAL+HIGH findings):**
- `.handoff/crit-architect.md` (175 lines)
- `.handoff/crit-security.md` (217 lines)
- `.handoff/crit-database.md` (302 lines)
- `.handoff/crit-product.md` (124 lines)
- `.handoff/crit-compliance.md` (185 lines)
- `.handoff/crit-business.md` (296 lines)

**Phase-3 outputs (now mirrored to master for reference-ability):**
- **`.handoff/v2-master-vision-v1.md`** — single consolidated coherent doc (~615 lines). 15 sections from Exec Summary through Atlas-crates-refs. **Full rationale + Phase-2-critique provenance for everything in V2-MASTER-PLAN.md.**
- **`.handoff/decisions.md`** — 22 explicit ACCEPT/MODIFY/DEFER decisions (~284 lines). Every CRITICAL + HIGH Phase-2 finding addressed.

**Phase-4 outputs (master-resident, primary reads for V2-α work):**
- **`docs/V2-MASTER-PLAN.md`** — distilled strategic plan (~300 lines). Welle decomposition (V2-α/β/γ/δ), top-5 blocking risks, demo programme, success criteria, reference pointers. **PRIMARY read for V2-α planning.**
- **`docs/WORKING-METHODOLOGY.md`** — reusable 4-phase iteration pattern (~200 lines). Anti-pattern table. Use this for any future Großthema (post-quantum migration, V3 architecture).

**Pre-flight checklist for V2-α Welle 1 start:**
```bash
cd "C:/Users/nelso/Desktop/atlas"
git status                                          # → clean
git checkout master && git pull origin master       # → ensure master is current
ls docs/V2-MASTER-PLAN.md docs/WORKING-METHODOLOGY.md  # → both exist on master
ls .handoff/v2-master-vision-v1.md .handoff/decisions.md  # → both exist on master
"/c/Program Files/GitHub CLI/gh.exe" pr list \
  --state open --json number,title                  # → #59/#61/#62 still draft (archive PRs)
git verify-tag v1.0.1                               # → Good ed25519 sig
```

Then read `docs/V2-MASTER-PLAN.md` §6 (Welle Decomposition) to scope V2-α Welle 1. Counsel engagement progresses on parallel track (Nelson-driven).

**Worktree cleanup (post-Phase-4, now safe):**
11 subagent worktrees (5 Phase-1 + 6 Phase-2 + 1 architect-failed-orphan) live under `.claude/worktrees/agent-*`. PRs #59/#61/#62 are draft-archived; their branches are no longer functional dependencies. Cleanup is safe but not urgent:
```bash
git worktree list                                                # see all worktrees
git worktree remove .claude/worktrees/agent-XXX --force          # per worktree
git branch -D worktree-agent-XXX                                  # per branch
```
Keep PR branches intact (don't `git push origin --delete`) — the draft PRs themselves are the audit-trail artefacts and can be re-cloned if ever needed.

**Standing protocol reminders:**
- Master direct-push is blocked; always PR
- SSH-Ed25519 signed commits (key `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- Cargo PATH on Windows: `/c/Users/nelso/.cargo/bin/cargo.exe`
- gh CLI: `/c/Program Files/GitHub CLI/gh.exe`
- Implement → parallel code-reviewer + security-reviewer → fix CRITICAL/HIGH in-commit → single coherent commit → docs PR
- Nelson prefers German in chat; respond in German
- Standing directive: *"Geh mit A und hol es bestmöglich aus dem Produkt raus. Entscheide du, was jetzt das Bestmögliche ist anhand deiner Informationen, die du hast. Immer beste Sicherheit und beste Codequalität."*

---

## 0z. V2-β-α.2 + V2-β Phase 0-8.5 SHIPPED narrative (2026-05-13 work-day)

> **Read this first** if you're a fresh agent continuing the V2-β work. Everything below §0z is historical V2-α / strategic-iteration context. §0z captures the most recent operational state + the W17a ready-to-dispatch prompt skeleton.

### What landed today (master timeline 2026-05-13)

| Commit / Tag | PR | Welle | Brief |
|---|---|---|---|
| `47b6894` | #70 | V2-α Welle 8 | v2.0.0-alpha.1 ship — workspace version bump + signed tag + GitHub Release + npm publish |
| `7b7e7d9` | #71 | V2-β Phase 0 | Orchestration plan + Dependency graph + welle plan-doc template (HARD GATE) |
| `1e9556b` | #72 | V2-β Welle 9 | Operator runbook `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` (§1-§8, 491 lines) |
| `64c70fe` | #73 | V2-β Welle 10 | Parallel-projection design ADR-Atlas-007 (380 lines, 9 sections) |
| `9bc1ef4` | #74 | V2-β Welle 11 | wasm-publish.yml dual-publish race fix + ADR-Atlas-008 postmortem |
| `d55491e` | #75 | V2-β Phase 2 | Phase-1-batch consolidation commit |
| **`v2.0.0-alpha.2`** + `1839e82` | #76 | V2-β Phase 3 | v2.0.0-alpha.2 ship — signed tag + GH Release + npm @atlas-trust/verify-wasm@2.0.0-alpha.2 LIVE with Sigstore Build L3 provenance. **Validates W11 wasm-publish fix end-to-end.** |
| `c041160` | #79 | V2-β Welle 12 | Read-API: 6 Next.js route handlers in `apps/atlas-web/src/app/api/atlas/` + inline Cypher AST validator + 74 tests |
| `cd74129` | #77 | V2-β Welle 13 | MCP V2: 5 MCP tools in `apps/atlas-mcp-server/src/tools/` + inline Cypher AST validator + 150 assertions |
| `15ee695` | #78 | V2-β Welle 14 | Expanded projector event-kinds: `annotation_add` + `policy_set` + `anchor_created` dispatch arms in `crates/atlas-projector/src/upsert.rs` + state.rs/canonical.rs extensions + 52 new tests |
| `15b87a3` | #80 | V2-β Phase 5 | Phase-4-batch consolidation commit + W15 entry criteria |
| `77afaf8` | #81 | V2-β Welle 15 | Cypher-validator consolidation — NEW `packages/atlas-cypher-validator/` shared monorepo package + ADR-Atlas-009 (321 lines) + 43 unified tests + 2 callsite updates. Required 2 reviewer-driven hotfixes (tsc build step matching `packages/atlas-bridge/` convention + workflow build-step propagation to wave3-smoke + sigstore-rekor-nightly). |
| `f901296` | #82 | V2-β Phase 7 | Post-W15 single-welle consolidation commit |
| `4a1e431` | #83 | V2-β Welle 16 | ArcadeDB embedded-mode spike (`docs/V2-BETA-ARCADEDB-SPIKE.md`, 460 lines, 11 sections) + ADR-Atlas-010 (285 lines, 9 sections). 2 reviewer-driven HIGH fixes: Layer 3 truth correction (validator does NOT enforce workspace_id presence) + perf number consistency (~6-10 min workspace-parallel re-projection). |

**Day total:** 17 PRs merged (#67-#83), 1 GitHub Release (v2.0.0-alpha.2), 1 npm publish, 9 ADRs total in repo (5 V2-β ADRs added: 007/008/009/010 + 011 reserved for W17a). 30+ reviewer-agent dispatches (per-welle + cross-batch consistency). ~15 reviewer-driven fix-commits applied in-commit before merge. **0 CRITICAL findings missed. 0 byte-determinism CI pin drifts.** All 7 V2-α byte-determinism pins byte-identical end-to-end.

### Session lessons learned (load-bearing for future welles)

1. **Worktree-isolation leaks are real and recurring.** `Agent` tool with `isolation: "worktree"` forks worktrees from master, but subagents that don't explicitly `git checkout` their target branch can end up writing to the main worktree directory. Affected W9, W11, W14 this session (W14 didn't even commit — parent had to finish the welle). **Lesson:** subagent dispatch prompts MUST include explicit `git fetch origin && git checkout -B feat/v2-beta/welle-<N>-<name> origin/master` as FIRST 3 actions. Parent verifies pre-flight succeeded before assuming agent worked correctly.

2. **When reviewers disagree on whether code is broken, RUN the code.** On W12 PR #79, a `code-reviewer` agent reported 2 CRITICAL findings (template-literal regex broken). The `security-reviewer` behaviorally tested the same validator and reported it works. Parent ran a `node` REPL test of the EXACT regex pattern with all 8 forbidden keywords — all correctly REJECTED. The "CRITICAL" was a theoretical misreading of JS template-literal escape semantics. **No fix applied — validator works as designed.** This pattern was recorded in the Phase 5 consolidation commit and is the canonical example for future reviewer-conflict resolution.

3. **Reviewer-driven fix-commits are non-optional even for "approved" PRs.** W15's initial commit had `package.json main/types/exports` pointing to source (not dist). Code-reviewer flagged as MEDIUM ("convention divergence from `packages/atlas-bridge/`"). Parent deferred — got bitten when Next.js production build failed in CI (`Module not found: ./validator.js`). Hotfix #1 added tsc build step. Then wave3-smoke + sigstore-rekor-nightly workflows ALSO needed the build step propagated. Hotfix #2 covered those. **Lesson:** MEDIUM findings about package conventions should be applied in-commit, not deferred.

4. **Cross-batch consistency-reviewer is a NEW V2-β invariant (per Orchestration Plan §3.5) and earns its dispatch.** Phase 1's cross-batch reviewer caught zero CRITICAL but 1 LOW (W9 §-numbering gap from W9-fix-commit). Phase 4's cross-batch reviewer caught 4 HIGH cross-welle inconsistencies (validator length cap divergence, passport `ok` field flip, agent_did echo cap, workspace vs workspace_id naming). Three of four fix-forward applied in-commit; the workspace-naming convention is HTTP-vs-MCP per-package preserved and documented for W15 (which then carried it through into ADR-Atlas-009 explicitly).

5. **Architect subagent type has Read/Grep/Glob ONLY.** Cannot Write files, cannot Run git commands. W16's architect produced ~700+ lines of inline doc-content for the parent to write. **Lesson:** for code-producing welles use `general-purpose` subagent type (which has full tool surface). For DOC-only spike-style welles, `architect` produces the content; parent writes the files.

6. **Auto-mode classifier blocks `gh pr merge --admin` by default.** Today required Nelson approval mid-session, then a project-local `.claude/settings.local.json` permission rule for `Bash(gh pr merge:*)` + `Bash(git push:*)` to allow unattended admin-merges. **Lesson:** settings.local.json now persists this — next session can use admin-merge directly. Atlas standing pattern (per `~/.claude/rules/common/git-workflow.md`) is `gh pr merge --squash --admin --delete-branch`.

7. **strict_required_status_checks_policy + trust-root-verify interaction.** When `gh pr update-branch` creates a GitHub-generated merge commit (signed by GitHub's RSA key, not the Atlas SSH-Ed25519 allowed signer), trust-root-verify FAILS for PRs that touch trust-root files (.github/workflows/wasm-publish.yml in W11's case). **Fix pattern:** rebase the welle branch locally onto fresh master (preserves SSH-signed commit), force-push. W17b will likely hit this if W17b touches anything in `.github/workflows/`.

### W17a ready-to-dispatch subagent prompt skeleton

```
Atlas project at C:\Users\nelso\Desktop\atlas. V2-β Welle 17a — ArcadeDB driver scaffold + GraphStateBackend trait + InMemoryBackend wire-up + ArcadeDbBackend stub.

## Pre-flight (FIRST 3 actions)
1. `git fetch origin`
2. `git checkout -B feat/v2-beta/welle-17a-arcadedb-scaffold origin/master` (master HEAD at dispatch: <current-master-sha>)
3. `git status` → clean

## Pre-flight reading (master-resident, mandatory)
1. `docs/V2-BETA-ARCADEDB-SPIKE.md` §7 (trait sketch — your starting point), §4.3 (Layer 3 caveat — read carefully), §4.9 (byte-determinism adapter contract), §10 (W17 entry criteria), §11 (V2-γ open questions)
2. `docs/ADR/ADR-Atlas-010-arcadedb-backend-choice-and-embedded-mode-tradeoff.md` §4 (8 binding sub-decisions for W17), §5.3 (W17b/W17c dependencies), §7 (reversibility)
3. `crates/atlas-projector/src/state.rs` + `upsert.rs` + `canonical.rs` (V2-α logic you'll wrap behind the trait)
4. `crates/atlas-projector/src/emission.rs` + `gate.rs` (consumers of the projector state — should still work via InMemoryBackend)
5. `.handoff/v2-beta-welle-N-plan.md.template` (your plan-doc skeleton)

## In-scope files (write these only — plus .handoff/v2-beta-welle-17a-plan.md)
- NEW `crates/atlas-projector/src/backend/mod.rs` — production GraphStateBackend trait + Vertex/Edge structs + WorkspaceTxn handle. ~150-200 LOC.
- NEW `crates/atlas-projector/src/backend/in_memory.rs` — InMemoryBackend impl wrapping existing state.rs+upsert.rs+canonical.rs. Preserves byte-determinism (graph_state_hash hex 8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4 unchanged). ~250-350 LOC.
- NEW `crates/atlas-projector/src/backend/arcadedb.rs` — stub. All trait methods `unimplemented!()` with W17b TODO docs. ~100 LOC.
- MODIFY `crates/atlas-projector/src/lib.rs` — `pub mod backend;` export
- MODIFY `crates/atlas-projector/src/emission.rs` — refactor to consume GraphStateBackend trait (default = InMemoryBackend; same behaviour as today)
- NEW `crates/atlas-projector/tests/backend_trait_conformance.rs` — trait-conformance tests for both backends (InMemory passes; ArcadeDb stub asserts `unimplemented!()` panic)
- NEW `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` (~250-300 lines, mirror ADR-010 structure)
- NEW `.handoff/v2-beta-welle-17a-plan.md` (use template)

## Forbidden files (parent consolidates these in Phase 9.5)
- CHANGELOG.md, docs/V2-MASTER-PLAN.md status, docs/SEMVER-AUDIT-V1.0.md, .handoff/decisions.md, .handoff/v2-session-handoff.md, docs/V2-BETA-ORCHESTRATION-PLAN.md

## Acceptance criteria
- `cargo check --workspace` green
- `cargo test --workspace` green; specifically `cargo test -p atlas-trust-core -p atlas-projector` shows 169 + 85 tests pass (or higher with new trait-conformance tests added)
- `graph_state_hash_byte_determinism_pin` hex unchanged at `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`
- Existing emission.rs + gate.rs consumers work via the new trait (refactored to use trait, default InMemoryBackend; no behaviour change)
- ArcadeDbBackend stub compiles (all methods `unimplemented!()`)
- Parallel `code-reviewer` + `security-reviewer` dispatched. Fix CRITICAL/HIGH in-commit BEFORE PR. Single SSH-Ed25519 signed commit. DRAFT PR base=master.

## Reviewer focus (when you dispatch them)
- code-reviewer: trait object-safety, lifetime correctness on `vertices_sorted`/`edges_sorted` iterators, default `canonical_state()` impl uses byte-identical canonicalisation to existing canonical.rs, no public-API breakage on existing atlas-projector consumers
- security-reviewer: byte-determinism preservation (Welle 1 author_did signing-input stamping + Welle 3 canonicalisation byte-pin unchanged), Vertex/Edge struct field set matches V2-α event payloads exactly (no new attack surface), trait object-safety doesn't break #[non_exhaustive] discipline

## Output
PR number + line counts + test pass count + confirmation of byte-determinism hex unchanged. Under 250 words.
```

### Pre-flight checklist for next session (any agent)

```bash
cd C:/Users/nelso/Desktop/atlas
git status                          # → clean
git checkout master && git pull origin master   # → up-to-date with master HEAD
git log --oneline -3                # → top is Phase-8.5 consolidation commit
"/c/Program Files/GitHub CLI/gh.exe" pr list --state open --json number,title  # → ~12 ancient drafts (#59-#62 etc.); zero NEW V2-β open
"/c/Program Files/GitHub CLI/gh.exe" release view v2.0.0-alpha.2  # → confirms prerelease LIVE
curl -s "https://registry.npmjs.org/@atlas-trust/verify-wasm" | python -c "import json,sys; d=json.load(sys.stdin); print('latest:', d['dist-tags'].get('latest'))"  # → "2.0.0-alpha.2"
git verify-tag v2.0.0-alpha.2       # → Good ed25519 sig
/c/Users/nelso/.cargo/bin/cargo.exe test -p atlas-trust-core -p atlas-projector --quiet  # → 169 + 85 tests pass (byte-determinism CI pin intact)
```

### Critical files for V2-β Phase 9+ reference (read-only)

- `docs/V2-BETA-ORCHESTRATION-PLAN.md` §2 (W17a row) + §3 (dispatch architecture + forbidden-files rule)
- `docs/V2-BETA-DEPENDENCY-GRAPH.md` §5 (critical path: Phase 9 unblocks Phase 10 unblocks Phase 11)
- `docs/V2-BETA-ARCADEDB-SPIKE.md` §7 (`GraphStateBackend` trait sketch — W17a's starting point)
- `docs/ADR/ADR-Atlas-010-arcadedb-backend-choice-and-embedded-mode-tradeoff.md` §4 (8 binding sub-decisions)
- `crates/atlas-projector/src/` (V2-α state.rs+upsert.rs+canonical.rs+emission.rs+gate.rs — W17a wraps these behind the trait)
- `packages/atlas-bridge/` (V1 monorepo-package convention — W17a does NOT touch this; it's the reference pattern that W15's `packages/atlas-cypher-validator/` already mirrored)

---

## 0a. Phase 1 SHIPPED — 2026-05-12 (this session)

**Phase 1 of the V2 strategic iteration ist abgeschlossen.** 5 Foundation Documents wurden parallel von 5 isolierten Subagents in eigenen git worktrees geschrieben. Integration auf branch `v2/phase-1-foundation` (PR #59, **DRAFT**, **NICHT mergen** — das ist der Phase-2-critique-target).

**Integration PR:** https://github.com/ThePyth0nKid/atlas/pull/59 (draft state)

**Die 5 Foundation Documents (alle auf `v2/phase-1-foundation` branch):**

| # | Doc | File | Lines | Subagent |
|---|-----|------|-------|----------|
| A | Strategic Positioning Vision | `.handoff/v2-vision-strategic-positioning.md` | 512 | general-purpose |
| B | Knowledge Graph Layer Architecture (v0.5) | `.handoff/v2-vision-knowledge-graph-layer.md` | 727 (+608/-93 vs v0) | general-purpose |
| C | Risk Matrix | `.handoff/v2-risk-matrix.md` | 457 | security-reviewer |
| D | Competitive Landscape (2026-05 baseline) | `.handoff/v2-competitive-landscape.md` | 630 | general-purpose + WebSearch |
| E | Demo Sketches | `.handoff/v2-demo-sketches.md` | 485 | general-purpose |

**Headline theses (one bullet each):**
- **Doc A** — Two-market positioning (Verifiable Second Brain + Multi-Agent Shared Memory); 6 novel trust-modes (continuous regulator attestation / insurance pricing substrate / Agent Passports / Cedar write-time / AI-BOM / B2B cross-org); 4 GTM hypotheses sequenced.
- **Doc B** — Three-Layer Architecture: events.jsonl (authoritative) + FalkorDB projection (queryable) + Mem0g cache (fast retrieval); Atlas as Hermes Memory Skill (4-call API); Agent Passports as `did:atlas:<pubkey-hash>` DIDs with revocation chain; federated witness cosignature; GDPR via content/hash separation.
- **Doc C** — Top-5 risks: R-A-01 Projection Determinism Drift (LOW detect, CRITICAL impact), R-L-01 GDPR Right-to-be-Forgotten (EU privacy counsel required), R-A-03 Agent Identity Key Compromise (V2-α blocking), R-S-01 Adoption Tipping Point (structural to category), R-L-02 FalkorDB SSPL License Trap.
- **Doc D** — No competitor has cryptographic trust in either category (verified via WebSearch 2026-05). Kuzu acquired by Apple Oct-2025 — ArcadeDB is next viable Apache-2.0 fallback. Graphiti = strongest partner candidate, 12-18mo competitor-risk. Obsidian has zero signature/verification plugins — white space for fast Verifiable-Second-Brain validation.
- **Doc E** — Demo 1 Multi-Agent Race = recommended landing-page hero. Demo 2 Regulator Witness = most ship-able TODAY (V1.14 live). Demos 3-5 need V2-α/β/γ/δ work; 4 of 5 are aspirational. Honesty flag raised in own open questions.

**Total content:** 2811 lines of strategic + architectural + risk + competitive + product material across 5 Foundation Documents.

**Open-Questions surface:** every doc carries an explicit "Open Questions for Phase 2 Critique" section. Combined ~55-65 explicit open questions across all 5 docs. **Cross-doc inconsistency is expected and NOT a Phase-1 convergence criterion** (per Iteration-Framework §1) — discrepancies are resolved in Phase 3 synthesis, not in Phase 1.

**Master HEAD on Phase-1-completion:** master remains at `5f19348` (V2 strategy trilogy). Phase 1 docs ONLY live on `v2/phase-1-foundation` branch — they do not enter master until Phase 4's `docs/V2-MASTER-PLAN.md`.

**Worktrees from Phase 1 (5 doc-branches + 1 orphan from architect re-dispatch):**
- `agent-a9da7cf2b6af8198c` / branch `worktree-agent-a9da7cf2b6af8198c` (Doc A, merged)
- `agent-a47f83e4af0f7b2d5` / branch `worktree-agent-a47f83e4af0f7b2d5` (Doc B re-dispatch, merged)
- `agent-adfac218b1cda42a9` / branch `worktree-agent-adfac218b1cda42a9` (Doc C, merged)
- `agent-ad7977870e1b40ef5` / branch `worktree-agent-ad7977870e1b40ef5` (Doc D, merged)
- `agent-a880ad3bdfa5c1083` / branch `worktree-agent-a880ad3bdfa5c1083` (Doc E, merged)
- `agent-a7f0eb28efcf59ae3` (orphan from architect re-dispatch, no writes — should be cleaned)

Cleanup these worktrees post-Phase-2 (or now if disk space matters): `git worktree remove <path> && git branch -D <branch>` per branch.

---

## 0b. Phase 2 SHIPPED — 2026-05-12 (this session, after Phase 1)

**Phase 2 of the V2 strategic iteration ist abgeschlossen.** 6 structured Critiques wurden parallel von 6 isolierten Subagents in eigenen git worktrees geschrieben. Integration auf branch `v2/phase-2-critiques` (PR #61, **DRAFT**, **NICHT mergen** — das ist der Phase-3-synthesis-target). Base-Branch von PR #61 ist `v2/phase-1-foundation` (PR #59) — Phase-2-Crits stacken atop Phase-1-Docs.

**Integration PR:** https://github.com/ThePyth0nKid/atlas/pull/61 (draft, base = v2/phase-1-foundation)

**Die 6 Critique Documents (alle auf `v2/phase-2-critiques` branch):**

| # | Crit | File | Lines | Primary Targets |
|---|------|------|-------|-----------------|
| 1 | Architect | `.handoff/crit-architect.md` | 175 | Doc B + Doc D |
| 2 | Security | `.handoff/crit-security.md` | 217 | Doc B + Doc C |
| 3 | Database / Performance | `.handoff/crit-database.md` | 302 | Doc B + Doc D |
| 4 | Product / UX | `.handoff/crit-product.md` | 124 | Doc A + Doc E |
| 5 | Compliance / Regulatory | `.handoff/crit-compliance.md` | 185 | Doc A + Doc C |
| 6 | Business / Investor | `.handoff/crit-business.md` | 296 | Doc A + Doc D + Doc E |

**Total content:** 1299 lines structured critique across 6 perspectives.

**Headline CRITICAL findings (one or two per crit):**
- **Architect:** Projection determinism under-specified to unverifiability (Doc B §2.1/§3.2); Welle decomposition undercounted ~2× (realistic V2-α 5-8 sessions, total V2 14-20).
- **Security:** Revocation lag bound is wrong (`event.timestamp` agent-claimed not Rekor-pinned → backdate possible; revocation event signed by compromised key). Cypher passthrough = injection + DoS surface. WASM verifier CDN-trust gap (SLSA L3 protects npm publish, NOT CDN delivery).
- **Database:** FalkorDB "sub-ms p99 traversal" unsourced + dimensionally wrong. 91% Mem0g latency claim only cache-hit retrieval. Projection-rebuild 8.3h at 100M events, no parallel-projection plan.
- **Product:** All demos lack failure-mode equivalent to HTTPS's absent-lock state. Two-market positioning operationally undefended. Zero demos show failure modes.
- **Compliance:** "Independently verifiable" NOT in verbatim Art. 12 text. GDPR Art. 4(1) hash-as-personal-data is highest-stakes open legal question. **FACTUAL ERROR in Doc A §3.2:** EU AI Liability Directive was **WITHDRAWN Feb 2025** (Commission Work Programme 2025), not "expected 2026". Fallback regime is revised PLD 2024/2853. Regulator-witness federation has NO documented EU precedent.
- **Business:** No TAM/SAM/SOM — **fundraising-blocking**. No first-10-customers pipeline. Hermes-skill math: 60K stars → ~4-36 retained users steady-state — reclassify as "credibility asset" not "GTM Hypothesis 1".

**Convergence status:** Alle 6 Crits met or exceeded Iteration-Framework §2 criterion (≥5 strukturelle Punkte + ≥3 konkrete Edits). Quality high — genuinely surprising findings (esp. compliance AILD-withdrawal, database perf-deconstruction, business TAM/SAM/SOM gap).

**Phase 2 worktree fork-base lesson (carry into Phase 3+):** `Agent` tool with `isolation: "worktree"` forks from master regardless of parent's current branch. 4 of 6 crit-agents found workarounds (`git show v2/phase-1-foundation:<path>`); 2 (architect, product) loaded Phase-1 docs to disk as reference — those staged-adds had to be reset before commit. Mitigation for Phase 3+: instruct subagents to `git fetch && git checkout <target>` as first action, OR pass critical content inline via prompt.

---

## 0c. Phase 3 SHIPPED — 2026-05-12 (this session, after Phase 1+2)

**Phase 3 of the V2 strategic iteration ist abgeschlossen.** Semi-manual synthesis von 6 Phase-2-Crits gegen 5 Phase-1-Foundation-Docs → ein einziges koherentes Master-Vision-v1 + decisions.md mit 22 expliciten Entscheidungen. Auf branch `v2/phase-3-master-vision` (PR #62, **DRAFT**, **NICHT mergen** — das ist der Phase-4-MASTER-PLAN-derivation-target). Base-Branch von PR #62 ist `v2/phase-2-critiques` (PR #61).

**Integration PR:** https://github.com/ThePyth0nKid/atlas/pull/62 (draft, base = v2/phase-2-critiques)

**Two new Phase-3 outputs (PRIMARY READS for any future session):**
- **`.handoff/v2-master-vision-v1.md`** (~615 Zeilen) — single consolidated coherent V2 strategic vision. 15 Sections: Executive Summary / V1→V2 Pivot / Two-Market Positioning (mit operational decision rule) / EU AI Act Compliance Reality (AILD-correction + Art. 12 verbatim + GDPR Path B/A) / Three-Layer Trust Architecture (Phase-2-hardened) / Risk Matrix v1 / Competitive Landscape (Kuzu archived, ArcadeDB fallback, Lyrie ATP integration) / Demo Programme (7 demos, Demo 4 deferred, Demo 6+7 added) / GTM + Business Model (EU-regulated Q0 not Q4) / Welle Decomposition (re-baselined 14-20 sessions) / Counsel Engagement Plan / Open Strategic Questions / Atlas crates references.
- **`.handoff/decisions.md`** (~284 Zeilen, 22 entries) — explicit ACCEPT/MODIFY/DEFER/REJECT decisions per Iteration-Framework §3. Categorised by domain: COMPLIANCE-1..4, COUNSEL-1..7+MASTER, ARCH-1..2, SEC-1..5, DB-1..3, BIZ-1..6, PRODUCT-1..2, RISK-1.

**Top-12 Phase-3 decisions (for fresh-context agent quick-scan):**

| ID | Decision | Reversibility |
|----|----------|---------------|
| **COMPLIANCE-1** | AILD WITHDRAWN Feb 2025 → reframe to PLD 2024/2853; insurance-pricing defer V2-γ | HIGH |
| **COMPLIANCE-2** | Drop "independently verifiable" Art. 12 paraphrase, use verbatim text | HIGH |
| **COMPLIANCE-3 / COUNSEL-1** | GDPR Art. 17 hash-as-PII: Path B (counsel opinion) with Path A (salt redesign) fallback | LOW once V2-α schema commits |
| **COMPLIANCE-4** | Regulator-witness "friendly" not "approved" + pursue supervisor sandbox engagement | HIGH |
| **ARCH-1 / SEC-2** | Triple-hardening projection determinism (byte-pin + ProjectorRunAttestation + parallel-projection) | MEDIUM |
| **SEC-1** | Out-of-band agent-DID revocation channel + signed_at_rekor_inclusion_time Δ-flagging | LOW once V2-γ ships |
| **SEC-3** | M-of-N threshold federation enrolment + federation_enrolment_event in events.jsonl | MEDIUM |
| **DB-1** | ArcadeDB Apache-2.0 fallback (Kuzu acquired by Apple Oct-2025, archived) | MEDIUM |
| **BIZ-1** | Hermes reclassified GTM-Hypothesis-1 → credibility-asset | HIGH |
| **BIZ-2** | EU-regulated enterprise GTM start Q0 not Q4 (reverse Phase-1 sequencing) | MEDIUM |
| **PRODUCT-1** | Demo overhaul: Demo 4 deferred, Demo 6 Quickstart + Demo 7 Failure-Mode added, CTA inverted | HIGH |
| **COUNSEL-MASTER** | **€30-80K counsel engagement front-loaded, pre-V2-α blocking** for EU PII customers | HIGH |

**Convergence-Status (per Iteration-Framework §3):** ✓ Master Vision exists. ✓ All CRITICAL findings addressed (15). ✓ All HIGH findings addressed. ✓ Decisions.md ≥10 entries (delivered 22). Phase 3 SHIPPED.

**Phase 3 worktree status:** Synthesis lief im main repo (master worktree direkt) — keine isolierten Subagent-Worktrees, weil Phase 3 NOT parallel-dispatchable ist. Decision-Volume bounded by ~22 entries; semi-manual mit Claude+Nelson cooperatively.

---

## 0d. Phase 4 SHIPPED — 2026-05-12 (this session, after Phase 1+2+3)

**Phase 4 of the V2 strategic iteration ist abgeschlossen.** Master Vision v1 + decisions.md wurden zu zwei master-resident production-grade Docs destilliert. Plus die Phase-3-outputs (Master Vision + decisions.md) wurden für master-reference-ability mit-gemerged. Diese Phase ist die EINZIGE der 4 Phasen die master tatsächlich ändert — Phasen 1-3 leben permanent als draft-PR-archives.

**Integration PR:** https://github.com/ThePyth0nKid/atlas/pull/60 (gemerged auf master)

**Die 4 master-resident outputs (alle nach Phase-4-merge auf master):**

| # | File | Lines | Role |
|---|------|-------|------|
| 1 | `docs/V2-MASTER-PLAN.md` | ~300 | **PRIMARY V2 strategic plan.** Distilled Master Vision mit Welle-Decomposition tied to concrete PR-Wellen. Sections: Vision · Two-Market Positioning · Three-Layer Trust Architecture · Top-5 V2-α Blocking Risks · Counsel Engagement · Welle Decomposition (V2-α/β/γ/δ scope+success criteria+expected PR count) · Demo Programme · Competitive Position · GTM+Business · Success Criteria for V2 · Reference Pointers |
| 2 | `docs/WORKING-METHODOLOGY.md` | ~200 | **Reusable 4-phase iteration pattern.** Captures the methodology Atlas-team applied here for future Großthemen (post-quantum migration, V3 architecture). Sections: Why this exists · Phase 1-4 detailed patterns · Welle-Decomposition pattern · Decision Log Discipline · Versioning + Anti-Patterns · When to Skip |
| 3 | `.handoff/v2-master-vision-v1.md` | ~615 | **Phase-3 output, mirrored to master for reference-ability.** Full V2 vision with all Phase-2-critique provenance + factual corrections (AILD withdrawn, Art. 12 verbatim, GDPR Path B). Read this when you need the full rationale behind anything in V2-MASTER-PLAN.md. |
| 4 | `.handoff/decisions.md` | ~284 | **Phase-3 output, mirrored to master for reference-ability.** 22 explicit ACCEPT/MODIFY/DEFER entries with reversibility tags + review-after triggers. Every Phase-2 CRITICAL + HIGH finding addressed. |

**Total Phase-4 content:** ~1399 lines across 4 files. ~500 lines net-new (`V2-MASTER-PLAN.md` + `WORKING-METHODOLOGY.md`); ~899 lines mirrored from Phase-3 work-product PR #62.

**Headline Phase-4 outputs (one bullet each):**
- **`docs/V2-MASTER-PLAN.md`** — V2 Welle-Decomposition concretised: V2-α 5-8 sessions (projector + FalkorDB + DID schema + content-hash separation + projector-state-hash gate + ProjectorRunAttestation event + ArcadeDB spike + counsel gate), V2-β 4-5 sessions (Mem0g + 6 Read-API endpoints + 5 MCP V2 tools + Explorer UI + secure-deletion + parallel-projection plan), V2-γ 3-4 sessions (Agent Passports + revocation + federation enrolment + Hermes-skill v1), V2-δ 2-3 sessions (Cedar policy + Graphiti + post-quantum hybrid). **Total V2 = 14-20 sessions** plus 6-8 weeks counsel parallel-track.
- **`docs/WORKING-METHODOLOGY.md`** — 4-phase pattern (Foundation Docs → Multi-Angle Critique → Synthesis → Plan Documentation) with anti-pattern table (8 anti-patterns flagged: skip-Phase-2, auto-merge-draft-PRs, mega-doc, no-reversibility-tagging, critique-says-looks-good, plan-equals-vision, no-counsel-track, fundraising-numbers-TBD). When-to-skip rules. Reusable for any future Großthema where reversibility is LOW.

**Phase 4 convergence criterion (per Framework §4 / per `docs/WORKING-METHODOLOGY.md`):** ✓ Both docs reviewed by parallel code-reviewer + security-reviewer agents (claim-drift check across Master Vision ↔ Master Plan ↔ Decisions). ✓ CRITICAL/HIGH findings fixed in-commit. ✓ Single coherent SSH-signed commit. ✓ `CHANGELOG.md [Unreleased]` updated with Phase 4 entry. ✓ PR #60 expanded to contain all Phase-4 outputs.

**What ships on master via PR #60 merge:**
- All 4 files above (V2-MASTER-PLAN, WORKING-METHODOLOGY, master-vision-v1, decisions.md)
- Updated `.handoff/v2-session-handoff.md` (this file) with Phase 1+2+3+4 SHIPPED state + V2-α-Welle-1 pre-flight checklist
- Updated `CHANGELOG.md [Unreleased]` with Phase-4 narrative entry

**Master HEAD after Phase 4 merge:** new commit on top of `5f19348` (V2 strategy trilogy). Phases 1-3 work-product PRs (#59 / #61 / #62) stay permanently draft as audit-trail archives.

**What's next (operationally):**
- **Engineering track:** V2-α Welle 1 scoping (Atlas Projector skeleton, canonicalisation byte-pin spec, FalkorDB integration spike, ArcadeDB comparative spike). Per `docs/V2-MASTER-PLAN.md` §6.
- **Counsel track (parallel, Nelson-led):** select 1 lead firm from shortlist + sign 6-8-week structured engagement + scope per `.handoff/v2-master-vision-v1.md` §11.
- **Business track (parallel, Nelson-led):** TAM/SAM/SOM bottom-up math published + first-10-customers named pipeline assembled. Pre-fundraising blocking per `DECISION-BIZ-3` + `DECISION-BIZ-4`.

---

## 0d-DEPRECATED. Phase 4 Plan — historical reference (kept for audit-trail; see §0d SHIPPED above)

⚠️ Original Phase-4 Plan section preserved below. The plan was executed successfully in the same session that wrote this note. Outputs live in `docs/V2-MASTER-PLAN.md` + `docs/WORKING-METHODOLOGY.md` per §0d SHIPPED above. Cross-doc consistency verified by parallel code-reviewer + security-reviewer agents.

**Original Phase 4 Plan content follows.**

### Phase 4 Plan — Master-Plan + Working-Methodology landing on master

**Goal:** Master Vision v1 (~800 lines, on PR #62 draft-branch) wird zu zwei production-ready docs destilliert die **auf master landen via standard SSH-signed PR** (das ist die einzige der 4 Phasen die wirklich Master ändert).

**Pre-flight (vor Phase-4-start):**
1. `git fetch origin && git checkout v2/phase-3-master-vision` — branch lokal aktuell
2. **Read `.handoff/v2-master-vision-v1.md`** end-to-end (~800 lines)
3. **Read `.handoff/decisions.md`** end-to-end (22 entries)
4. Re-read Iteration-Framework §4 (`.handoff/v2-iteration-framework.md`) — Phase-4-Output-Spezifikation
5. **Stimme mit Nelson ab:** (a) Counsel-Engagement-Kickoff Timing — vor oder nach Phase-4-merge? (b) Welche Counsel-Firma als lead? (c) Erste-10-Kunden-Pipeline: hat Nelson Material? (d) TAM/SAM/SOM bottom-up: Analyst extern oder Nelson-led? Diese 4 Punkte sind NICHT von Phase 4 selbst dispatchbar — Nelson-decisions notwendig.

**Phase 4 Outputs (lands on master via SSH-signed PR):**

### `docs/V2-MASTER-PLAN.md` (~300 lines)
Verdichtung von Master-Vision-v1 mit Welle-Decomposition tied to concrete PR-Wellen. Sections:
- §1 V2 Vision (~50 lines, distilled from Master-Vision §2 + §3)
- §2 Competitive Positioning (~30 lines, distilled from §3 + §7)
- §3 Risk Matrix (~40 lines, distilled from §6 — top-5 V2-α blocking only)
- §4 Demo Roadmap (~30 lines, distilled from §8 — 7 demos with V2-stage gating)
- §5 Counsel-Engagement Pipeline (~30 lines, distilled from §11)
- §6 Technical Architecture Roadmap (~40 lines, distilled from §5)
- §7 V2 Welle-Decomposition (~50 lines, tied to specific PR-Wellen): V2-α / V2-β / V2-γ / V2-δ each with: scope, dependencies, blocking-risks, success criteria, expected PR count
- §8 Success Criteria for V2 (~30 lines): what "V2 successful" means measurably

### `docs/WORKING-METHODOLOGY.md` (~200 lines)
Reusable 4-phase iteration pattern. Capture the methodology Atlas-team has now refined for future Großthemen (post-quantum migration, V3 architecture, etc.). Sections:
- §1 Vision-First-Pattern (Phase 1: parallel foundation-docs, 5-6 in isolated worktrees)
- §2 Multi-Angle-Critique (Phase 2: 6 parallel critique-agents in own worktrees, structured Stärken/Probleme/Blinde-Flecken/Vorschläge/Open-Questions format)
- §3 Synthesis-Convergence (Phase 3: semi-manual Master-Vision + decisions.md)
- §4 Plan-Documentation (Phase 4: master-plan + this methodology, only Phase landing on master)
- §5 Welle-Decomposition (how to derive concrete sprints from a Master-Plan)
- §6 Decision Log Discipline
- §7 Versioning (when methodology itself evolves)

### Plus (separate operational track):
**Counsel-Engagement-Kickoff** — Nelson selects 1 lead firm from shortlist (Hogan Lovells Frankfurt / Bird & Bird Munich / Matheson / William Fry / Cleary Gottlieb Paris), signs 6-8-week structured engagement, scope per `v2-master-vision-v1.md` §11. €30-80K budget. Pre-V2-α blocking gate.

**Convergence criterion for Phase 4** (per Framework §4): Both docs reviewed by Nelson, merged to master via standard SSH-signed PR. Welle 14b/c/d/14e roadmap im handoff doc reflects them. Counsel engagement kicked off (or explicit "deferred to Y" decision in decisions.md).

**Timing:** 1-2 Sessions für die zwei docs (verdichten ist mechanisch wenn die Master-Vision schon existiert). Counsel-engagement kickoff in parallel, weeks-not-sessions.

**Standing-Protocol für Phase 4 PR (per Atlas conventions):**
- Implement (write the two docs)
- Parallel `code-reviewer` + `security-reviewer` agents (yes, even for docs — they catch claim-drift between master-vision and master-plan)
- Fix CRITICAL/HIGH in-commit
- Single coherent SSH-signed commit → docs PR on master
- Update CHANGELOG.md `[Unreleased]` with V2-Master-Plan landing entry

---

## 0e. Phase 3 Plan — DEPRECATED, see §0c SHIPPED above

⚠️ Original Phase 3 Plan section preserved for historical reference (§0c was "Phase 3 Plan" in prior version; now it's "Phase 3 SHIPPED"). The original Phase-3-plan content (top-priority cross-crit reconciliations, pre-flight checklist) was successfully executed and the outputs live in `.handoff/v2-master-vision-v1.md` + `.handoff/decisions.md`.

---

## ~~0c. Phase 3 Plan — Synthesis & Convergence~~ [REPLACED by §0c SHIPPED above]

**Goal:** Synthesize the 6 Phase-2 critiques against the 5 Phase-1 Foundation Documents into a single coherent `.handoff/v2-master-vision-v1.md`, with every accepted/rejected/modified crit-point logged in `.handoff/decisions.log`.

**Important:** Phase 3 is **semi-manual** with Nelson — NOT a parallel-subagent dispatch. Decisions belong to humans + Nelson + Claude jointly. Per Iteration-Framework §3, you read the crits together and make three classes of decisions:
1. **ACCEPT** crit-points → directly insert into the Master-Vision-v1 (modifying Phase-1 docs as needed)
2. **CONFLICT** between crits → Nelson decides with Claude's tradeoff-analysis (e.g., Architect's projection-determinism CRITICAL + Database's Mem0g latency CRITICAL both touch Doc B §2.5+§3.2 — must be reconciled, not both accepted independently)
3. **REJECT** crit-points → log in `.handoff/decisions.log` with rationale + reversibility tag

**Pre-flight (vor Phase-3-start):**
1. `git fetch origin && git checkout v2/phase-2-critiques`
2. Read all 6 crit files in `.handoff/crit-*.md` (cross-reference cited doc-sections)
3. Re-read Iteration-Framework §3 (`.handoff/v2-iteration-framework.md`)
4. Estimate decision-volume by skimming each crit's "Konkrete Vorschläge" + "Probleme" sections — probably 80-150 distinct decisions across all 6 crits

**Top-priority cross-crit reconciliations (Phase 3 will need to resolve):**
- **Projection-determinism** (Architect-C-1 + Security-Q-SEC-6 + Database-P-CRIT-3) — three perspectives on the same Doc B §2.1/§3.2 weak spot. Recommend: accept Architect's canonicalisation-byte-pin + accept Security's ProjectorRunAttestation event-binding + accept Database's parallel-projection requirement + add quantified RTO.
- **GDPR-by-design vs GDPR-mitigated** (Architect-Q-ARCH-3 + Compliance-C-2) — both flag hash-personal-data as unresolved. Recommend: downgrade matrix wording, commit to EU-counsel engagement (Compliance recommends €30-80K budget).
- **EU AI Liability Directive factual error** (Compliance-H-5) — Doc A §3.2 must be re-written. AILD was withdrawn Feb 2025; revised PLD 2024/2853 is the actual fallback regime. Doc A §4.2 (AI-Liability-Insurance pitch) needs full reframe.
- **GTM sequencing** (Business-A vs Doc A current) — Business says reverse §6.5: EU-regulated enterprise must start Q0 not Q4 because sales cycles are 6-12 months. Recommend: accept reversal.
- **Hermes-Agent reclassification** (Business + Product) — both crits agree Hermes should NOT be the primary GTM channel. Recommend: keep as credibility asset / demo channel but not GTM-Hypothesis-1.
- **Demo programme overhaul** (Product) — Doc E needs Demo 6 (Quickstart, TODAY readiness) + Failure-Mode demo replacing Demo 4. Recommend: accept; instruct Phase-3 to revise Doc E.

**Phase 3 outputs:**
1. `.handoff/v2-master-vision-v1.md` — single consolidated coherent doc (Doc A + B + C + D + E merged + all accepted crit-edits)
2. `.handoff/decisions.log` — ≥10 explicit ACCEPT/REJECT/MODIFY entries with rationale + reversibility
3. Updated handoff (this file) with Phase 3 SHIPPED block + Phase 4 plan

**Phase 3 convergence criterion** (per Framework §3): Master-Vision exists, all CRITICAL and HIGH crit-points addressed (accepted, modified, or explicitly rejected), decisions.log ≥10 entries.

**Phase 3 lands on a new integration branch:** `v2/phase-3-master-vision`, base = `v2/phase-2-critiques`. Still draft PR, still no-merge. Only Phase 4 (`docs/V2-MASTER-PLAN.md` + `docs/WORKING-METHODOLOGY.md`) lands on master.

**Timing:** ~2-3 sessions (semi-manual, decision-volume bounded by ~80-150 crit-points). Nelson + Claude work through synthesis methodically; not parallel-subagent dispatchable.

---

## 0d. Phase 1 Plan — DEPRECATED (kept for historical reference)

⚠️ The original Phase 1 plan section below was superseded by Phase 1 SHIPPED (§0a). The original Phase 2 Plan section (formerly §0b) was superseded by Phase 2 SHIPPED (§0b above) and Phase 3 Plan (§0c). Below is the original Phase-1-Start handoff content preserved for historical reference and for any future agent that needs to understand the original strategic context (§4 in particular).

---

## ~~0b. Phase 2 Plan — Multi-Angle Critique (next session entry point)~~ [REPLACED by §0b SHIPPED + §0c Plan above]

**Original goal:** 6 parallele critique-Subagents lesen alle 5 Phase-1-Docs auf PR-Branch `v2/phase-1-foundation` und produzieren strukturierte +/- Crits per Iteration-Framework §2.

**Pre-flight (vor Phase-2-dispatch):**
1. `git fetch origin && git checkout v2/phase-1-foundation` — sicherstellen die Branch ist lokal aktuell
2. Read alle 5 Phase-1-Docs (Files: `.handoff/v2-vision-strategic-positioning.md`, `.handoff/v2-vision-knowledge-graph-layer.md`, `.handoff/v2-risk-matrix.md`, `.handoff/v2-competitive-landscape.md`, `.handoff/v2-demo-sketches.md`)
3. Read Iteration-Framework §2 (`.handoff/v2-iteration-framework.md`) — critique-format template
4. **Mit Nelson abstimmen:** sind die 6 critique-Rollen unverändert (architect / security / database-performance / product-UX / compliance-regulatory / business-investor) oder Anpassung gewünscht?

**Dispatch convention (mirror Phase 1):**
- 6 parallele Agent-Calls in einer Message
- `isolation: "worktree"` für jeden (eigene Branch je crit)
- Pfade in Prompts **relativ** (`.handoff/...` NICHT `C:/Users/.../.handoff/...`)
- Subagent_types matchen die Crit-Rolle (architect → architect, security → security-reviewer, database-performance → general-purpose, product-UX → general-purpose, compliance-regulatory → general-purpose, business-investor → general-purpose)
- Each crit produces `.handoff/crit-<role>.md` (~300-500 lines)

**Crit format template** (per Iteration-Framework §2):
```
# Crit: <role> on Atlas V2 Vision
## Stärken (was ist gut, sollte bleiben)
## Probleme (was muss adressiert werden — by severity: CRITICAL/HIGH/MEDIUM/LOW)
## Blinde Flecken (was wird in den docs gar nicht angesprochen)
## Konkrete Vorschläge (specific edits/additions, doc-section-tagged)
## Offene Fragen für Phase 3
```

**The 6 critique agents + their primary doc targets:**

| # | Crit-Rolle | Subagent-Type | Primary Doc Target | Output |
|---|---|---|---|---|
| 1 | Architect | architect (Read/Grep/Glob only — produce text inline, parent writes file) | Doc B + Doc D — technical feasibility, projector-determinism, multi-tenant isolation, FalkorDB vs Kuzu-now-archived | `.handoff/crit-architect.md` |
| 2 | Security reviewer | security-reviewer | Doc B + Doc C — trust invariant integrity, key management, replay attacks, post-quantum, GDPR conflict, Agent-DID revocation | `.handoff/crit-security.md` |
| 3 | Database / performance | general-purpose | Doc B + Doc D — FalkorDB vs ArcadeDB (Kuzu archived!) vs Neo4j vs Memgraph, performance vs Mem0g, projection-rebuild-cost at scale, index strategy | `.handoff/crit-database.md` |
| 4 | Product / UX | general-purpose | Doc A + Doc E — positioning coherence, user-journey realism, demo-conversion-likelihood, Obsidian-comparison-fairness, multi-agent-race-demo-feasibility | `.handoff/crit-product.md` |
| 5 | Compliance / regulatory | general-purpose | Doc A + Doc C — EU AI Act Art. 12-19 mapping accuracy, AI-Liability-Directive readiness, agent-identity/DID compatibility, jurisdictional scope, witness-federation legal pattern | `.handoff/crit-compliance.md` |
| 6 | Business / investor | general-purpose | Doc A + Doc D + Doc E — market sizing, competitive moat, monetization paths, fundraising readiness, partnership candidates (Mem0/Graphiti/Hermes/Lyrie-ATP) | `.handoff/crit-business.md` |

**Lesson from Phase 1 (architect Read-only constraint):** the `architect` subagent_type only has Read/Grep/Glob — no Write. If using architect for Crit #1, expect inline text return; parent agent (this session's main thread) writes the file. Alternative: use `general-purpose` for all 6 crits to avoid the constraint, accepting that the architect role's specialism is lost.

**Convergence criterion for Phase 2** (per Framework §2): alle 6 crits geliefert, jede ≥5 strukturelle Punkte + ≥3 konkrete Edits. **Crits MÜSSEN adressieren, nicht nur "looks good".**

**Output:** integration branch `v2/phase-2-critiques` (analog to Phase 1), all 6 crits merged, PR opened **draft, no-merge**. Then Phase 3 synthesis (manual, with Nelson).

**Timing:** ~60-90 min for 6 parallel crits.

---

---

## 0. TL;DR für den Agent der das gerade liest

Atlas v1.0.1 ist LIVE auf npm mit SLSA Build L3 provenance (siehe `.handoff/v1.19-handoff.md` §0). V1 ist abgeschlossen. **Jetzt startet V2 — der Verifiable Second Brain + Shared AI Memory Substrate** Pivot. Nelson hat über mehrere Brainstorm-Iterationen folgendes finalisiert:

1. **Atlas ist agent-agnostisch** — wir bauen keinen Agent, wir bauen die Verification-Substrate die jeder Agent benutzen kann. MCP-Server (V1.19 Welle 1) ist bereits der universal write-side adapter.

2. **Zwei-Markt-Positionierung:** Human-Second-Brain (Obsidian-Kategorie + cryptographic trust) UND Multi-Agent-Shared-Memory (jeder Agent — Hermes, Claude, GPT, Llama, custom — schreibt in dieselbe verifizierbare Wissensbasis).

3. **Stack-Confirmation:**
   - **FalkorDB** als Graph-DB-Layer (V2-α), Cypher-subset, GraphBLAS-Backend, eigenes FalkorDB Browser UI
   - **Mem0 + Mem0g** als Fast-Retrieval-Cache on top (91% p95 latency reduction, 26% besser als OpenAI Memory)
   - **Hermes Agent** (Nous Research, 60K+ GitHub stars, MIT-license, model-agnostic, self-improving) als Primary Demo-Agent — ist seit 2026-05-10 #1 auf OpenRouter, vom Thron gestoßen "OpenClaw"
   - **Trust-Layer bleibt V1's signed events.jsonl + Sigstore Rekor anchoring** — Graph-DB und Retrieval-Cache sind beide deterministisch rebuildable Projektionen

4. **Security Experts** kommen ans ENDE (post-V2-α/β). Nicht jetzt. Volle Kraft voraus mit aktueller AI-Capability.

5. **Iteration vor Implementation** — Nelson will über die Vision iterieren bevor irgendein Code geschrieben wird. Strukturiertes 4-Phasen-Framework ist in `.handoff/v2-iteration-framework.md` festgelegt.

**Was diese Session tut:** Plant Phase 1 (Foundation Documents) sorgfältig, dann dispatched 5 parallele Subagents in isolierten Worktrees, jeder schreibt ein Foundation-Doc auf eigener Branch.

---

## 1. Mandatory pre-read order (vor jeder anderen Aktion)

Liest diese Files in dieser Reihenfolge, dann fasst kurz zusammen was du verstanden hast, BEVOR du irgendwas anderes tust:

1. **`.handoff/v1.19-handoff.md`** — Atlas state, V1 history, Standing Protocol (the §0 "Welle 14a SHIPPED" block ist der current state)
2. **`.handoff/v2-iteration-framework.md`** — 4-Phasen-Methodik mit Convergence-Kriterien (das ist deine Bibel für diese Phase)
3. **`.handoff/v2-vision-knowledge-graph-layer.md`** — Technical Architecture Vision v0 (das wird Doc B in Phase 1, schon partial geschrieben)
4. **`CLAUDE.md`** (falls vorhanden) — repo-specific instructions
5. **Quickly skim:** `docs/SEMVER-AUDIT-V1.0.md`, `docs/ARCHITECTURE.md` für Kontext (du musst nicht alles lesen, nur die V2-Boundary Section in ARCHITECTURE.md)

Nach dem pre-read: **gib Nelson eine 5-Bullet-Zusammenfassung** was du verstanden hast. Wenn Nelson sagt "weiter", dann erst Phase 1 planen.

---

## 2. Anti-drift checklist (run bevor irgendein Code geändert wird)

```bash
cd "C:/Users/nelso/Desktop/atlas"
git status                                   # → clean
git log --oneline -3                         # → top is 314b8d5 (Welle 14a SHIPPED docs)
git tag -l "v1.0.*"                          # → v1.0.0, v1.0.1
git verify-tag v1.0.1                        # → Good ed25519 signature
GH="/c/Program Files/GitHub CLI/gh.exe"
"$GH" repo view ThePyth0nKid/atlas --json visibility   # → "PUBLIC"
"$GH" release view v1.0.1 --json isDraft     # → isDraft false
npm view @atlas-trust/verify-wasm@1.0.1 dist-tags   # → { "latest": "1.0.1" }
```

Wenn irgendwas davon nicht stimmt: **stop, klär mit Nelson**. Vermutlich ist der state aktueller als dieses Doc — dann reportiere den drift und frage was als nächstes.

---

## 3. Subagent orchestration architecture (das ist Nelson's explicit goal)

**Goal:** "Einzelne Agenten in einzelnen Branches so orchestriert dass sie sich nicht gegenseitig stören oder blockieren."

**Architektur:** Jeder Phase-1-Subagent läuft in einem **eigenen git worktree** mit eigener Branch. Atlas's master bleibt unangetastet während Phase 1 läuft. Konflikt-frei weil verschiedene Files written werden.

### Branch convention

```
master                                    ← stays clean during V2 strategy work
  │
  ├─ docs/v2/phase-1-doc-A-positioning    ← Subagent A schreibt nur in .handoff/v2-vision-strategic-positioning.md
  ├─ docs/v2/phase-1-doc-B-architecture   ← Subagent B refined .handoff/v2-vision-knowledge-graph-layer.md
  ├─ docs/v2/phase-1-doc-C-risk-matrix    ← Subagent C schreibt .handoff/v2-risk-matrix.md
  ├─ docs/v2/phase-1-doc-D-competitive    ← Subagent D schreibt .handoff/v2-competitive-landscape.md
  └─ docs/v2/phase-1-doc-E-demo-sketches  ← Subagent E schreibt .handoff/v2-demo-sketches.md
```

Each branch produces exactly ONE file delta. Zero overlap. Zero merge conflict risk.

### Worktree setup

Use Agent tool with `isolation: "worktree"` parameter — that auto-creates worktree + branch. The worktree is auto-cleaned if no changes; otherwise path + branch returned in result.

### Post-Phase-1 integration

After all 5 subagents return: dispatched checks:
1. Read each subagent's output file
2. Cross-check for internal consistency (no contradictory claims about Mem0g, Hermes Agent, etc.)
3. Merge alle 5 branches sequenziell in **integration branch** `v2/phase-1-foundation` via gh API
4. Eine PR `v2/phase-1-foundation → master` mit allen 5 docs als reviewable unit
5. Phase 2 critique agents arbeiten gegen diese integration branch, NICHT gegen master direkt

**Important:** Phase 1 docs werden nicht direkt nach master gemerged. Sie warten auf Phase 2 critique → Phase 3 synthesis → erst dann landet das gemerged Master-Vision-Doc auf master.

---

## 4. Strategischer Kontext — was Nelson erreichen will (don't lose this)

Aus den vorhergehenden Sessions verdichtet:

**Vision:** Atlas wird "die TÜV-Plakette für AI-agent memory" — strukturell verifizierbare gemeinsame Wissensbasis für humans + agents. Verifier-Crates Apache-2.0 (anyone can fork/embed), Server/Web Sustainable Use (revenue from hosted-service), Open-Core analog zu Obsidian's free-local-paid-sync model.

**Wettbewerbs-Landschaft:**
- Obsidian / Notion / Roam → human Second Brain, KEIN cryptographic trust
- Mem0 / Letta / Zep → AI memory, KEIN cryptographic trust (Atlas + Mem0 ist orthogonal hybrid)
- Graphiti → temporal KG framework, KEIN cryptographic trust, supports FalkorDB als backend (gut für Atlas)
- Anthropic Memory / OpenAI Memory → vendor-silo, geschlossen, nicht cross-vendor verifiable
- **Niemand sonst macht "cryptographic memory substrate, agent-agnostic, cross-vendor"** — Greenfield für Atlas

**EU AI Act als Driver:**
- Art. 12 (in force 2026-08-02): mandatory automatic event logs, independently verifiable
- Art. 13: Transparenz gegen Deployer
- Art. 14: Human oversight
- Art. 18: 6-Monate retention
- Plus die proposed **EU AI Liability Directive** (2026 expected, in Council-Phase) — Beweislast auf Provider

**Neue Trust-Modes die Atlas strukturell ermöglicht:**
- **Continuous regulator attestation** — Aufsicht hat Witness-Key in Trust-Root, Cosignatur in Echtzeit, kein periodisches Reporting mehr
- **AI-Liability-Insurance pricing** — Atlas-attested events = clean Schadens-Substanz, signifikant günstigere Prämien möglich
- **Agent Passports** — every agent has Ed25519 keypair, verifiable history of writes, portable reputation across organizations
- **Pre-action policy enforcement** — Cedar policies enforce at write-time, Compliance wird strukturell

**Hermes Agent Integration als go-to-market:**
- Hermes Agent (Nous Research) hat Plugin/Skill-System
- Atlas könnte ein First-Class "Atlas Memory Skill for Hermes Agent" werden
- Issue #477 in Nous's repo zeigt sie sind offen für Skill-Erweiterungen
- Hermes-Adoption-Wachstum (60K stars in 2 Monaten) macht das einen riesigen Distribution-Hebel

**Riskien die wir adressieren müssen:**
1. Adoption tipping point (Catch-22 — start mit EU-regulated vertical wo Compliance Driver ist)
2. Performance overhead (mitigation: tiered anchoring — hot writes signed-only, batch-anchored zu Rekor)
3. UX-Komplexität (mitigation: hide trust by default, show only "Verified ✓" / "Tampered ✗")
4. Vendor capture (mitigation: open-weight-models als pull-faktor, vendor-erlaubnis nicht nötig)
5. GDPR right-to-be-forgotten conflict (mitigation: signed hashes, raw content separable)
6. Privacy/confidentiality (mitigation: private federation tier neben public-witness tier)
7. **Post-quantum crypto** (V1 Algorithm enum ist additive, Migration-Plan als Welle 14d candidate — NICHT Phase-1-blocking, aber im Risk-Doc-C aufnehmen)

---

## 5. Phase 1 Goal — Foundation Documents

Fünf parallele Docs, jeder von eigenem Subagent in eigener Branch:

| Doc | File | Subagent Type | Scope |
|---|---|---|---|
| **A** | `.handoff/v2-vision-strategic-positioning.md` | general-purpose | Vision + Positioning + Beyond-Storage value + EU AI Act mapping + Agent identities + Two-market story (Second Brain + Shared Memory) |
| **B** | `.handoff/v2-vision-knowledge-graph-layer.md` (revise existing v0!) | architect | Tech architecture refined: events.jsonl → projector → FalkorDB → Mem0g hybrid + MCP read-side API + Hermes Agent skill integration |
| **C** | `.handoff/v2-risk-matrix.md` | security-reviewer | 8-12 risks: probability × impact × mitigation × owner. Inkl. post-quantum, GDPR, adoption-tipping, vendor-capture, privacy/confidentiality, performance |
| **D** | `.handoff/v2-competitive-landscape.md` | general-purpose | Mem0 / Letta / Zep / Graphiti / Obsidian / Notion / Anthropic-Memory / OpenAI-Memory feature × pricing × trust-property × Atlas-differentiator matrix |
| **E** | `.handoff/v2-demo-sketches.md` | general-purpose | 5 demo scenarios with 30-90s storyboard each: Hermes-Multi-Agent / Continuous-Audit-Mode / Agent-Passport / Mem0g-Hybrid / Verifiable-Lineage |

**Phase 1 convergence criteria:** Alle 5 Files existieren als v0, intern konsistent, mit explicit "Open Questions for Phase 2" Section am Ende jedes Files (Phase 2 critique-Agents brauchen das). Cross-file consistency wird in Phase 3 hergestellt — NICHT Phase 1.

---

## 6. Subagent-Prompts (ready-to-dispatch, verbatim)

Diese Prompts sind kuratiert worden über mehrere Iterations-Runden. Bevor du sie dispatchst, **review jeden Prompt nochmal kurz** mit Nelson — falls etwas wesentliches fehlt, add it. Aber don't rewrite from scratch, sie sind solide.

Dispatch alle 5 in einer einzigen Message (Anthropic-API supports parallel tool calls):

### 6.1 Doc A — Strategic Positioning

```
subagent_type: general-purpose
isolation: worktree
description: "Atlas V2 Doc A — Strategic Positioning"

prompt:
You are writing the strategic positioning vision document for Atlas V2. Context — Atlas is a cryptographic trust-verification project. V1.0.1 just shipped on npm (2026-05-12) with SLSA Build L3 provenance. V2 pivots to "verifiable knowledge graph substrate for any AI agent + human Second Brain".

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (this entire document, especially §4)
- /Users/nelso/Desktop/atlas/.handoff/v1.19-handoff.md §0
- /Users/nelso/Desktop/atlas/README.md
- /Users/nelso/Desktop/atlas/docs/SEMVER-AUDIT-V1.0.md (skim)
- /Users/nelso/Desktop/atlas/docs/ARCHITECTURE.md (V2 boundary section)

WRITE: /Users/nelso/Desktop/atlas/.handoff/v2-vision-strategic-positioning.md (~600-1000 lines)

STRUCTURE the document as follows (use these exact section headers):

# Atlas V2 — Strategic Positioning Vision

## 1. The Pivot (was V1, was V2 wird)
Worauf V1 hat geantwortet (compliance gap, EU AI Act Art. 12). Was V2 strukturell aufmacht (agent-agnostic shared substrate, Verifiable Second Brain category). Tagline candidates (mindestens 3).

## 2. Two-Market Positioning
2a. Verifiable Second Brain (Obsidian/Notion category + crypto trust)
2b. Multi-Agent Shared Memory (Hermes/Claude/GPT/custom all couple in)
Show the market sizing logic, target persona für each, why both markets work for the same substrate.

## 3. EU AI Act Structural Fit
Art. 12/13/14/18 mapping (use the table from §4 of the session handoff). Plus the proposed EU AI Liability Directive (2026 expected) implications.

## 4. New Trust-Modes Atlas Enables (genuinely novel — not just compliance)
4a. Continuous regulator attestation (Aufsicht's witness key live in trust root)
4b. AI-Liability-Insurance pricing substrate
4c. Agent Passports — Ed25519 keypair = verifiable agent identity + reputation
4d. Pre-action policy enforcement via Cedar at write-time
4e. AI Bill of Materials (AI-BOM) substrate
4f. B2B cross-organization trust patterns

## 5. Competitive Differentiation
Mem0 / Letta / Zep / Graphiti / Anthropic Memory / OpenAI Memory / Obsidian — Atlas's unique structural property vs each. Don't make this exhaustive (Doc D will do that); just the headline differentiator. One sentence per competitor max.

## 6. Go-to-Market Hypotheses
6a. Hermes Agent skill integration als first distribution-vehicle
6b. EU-regulated verticals (Finance/Healthcare/Insurance) als Compliance-driven adoption
6c. Open-weight-model alignment as pull-factor against vendor-capture
6d. Obsidian-style open-core monetization (free verifier, paid hosted sync / enterprise)

## 7. Risks to Positioning
Acknowledge the 5-7 most strategically dangerous things (don't make this list exhaustive — Doc C does that). Focus on positioning-level risks: market timing, vendor opposition, narrative complexity.

## 8. Open Questions for Phase 2 Critique
List 10-15 explicit questions that the Phase 2 product/business/strategy critique agents should challenge. Format: "Q: <question>. Context: <1-sentence why this matters>. Status: open."

CRITICAL guidelines:
- This is a STRATEGIC positioning doc, not a tech doc. Don't dive into FalkorDB / Mem0g architecture details — that's Doc B's job. Reference them at high level only.
- Be specific, not generic. "Atlas changes the game" = bad. "Atlas enables continuous regulator attestation by federating the regulator's witness key into the in-repo trust root — currently no compliance vendor offers this" = good.
- Cite Atlas's existing V1 capabilities by reference to specific files/features (e.g., "atlas-mcp-server (V1.19 Welle 1) already provides agent-agnostic write surface for any MCP-compatible host").
- Use German for headers and short prose, English for technical terminology and citations. Mirror the style of v1.19-handoff.md.
- Acknowledge what you don't know. Don't fabricate market data.

When done: write only the file. Do NOT commit. Do NOT push. Return a 5-bullet summary of the doc's main thesis.
```

### 6.2 Doc B — Technical Architecture (REVISE existing v0)

```
subagent_type: architect
isolation: worktree
description: "Atlas V2 Doc B — Technical Architecture (revise v0)"

prompt:
You are revising the technical architecture vision for Atlas V2. A v0 of this doc EXISTS — your job is to refine, deepen, and complete it based on the strategic decisions made in subsequent brainstorming.

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (entire document, especially §4)
- /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md (the EXISTING v0 — your starting point)
- /Users/nelso/Desktop/atlas/.handoff/v1.19-handoff.md §0
- /Users/nelso/Desktop/atlas/docs/ARCHITECTURE.md (full V2 boundary section)
- /Users/nelso/Desktop/atlas/docs/SEMVER-AUDIT-V1.0.md

REVISE IN PLACE: /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md

KEY ADDITIONS / REFINEMENTS NEEDED (the v0 doc doesn't capture these yet):

1. **Mem0g hybrid architecture explicit.** Mem0g is the graph-enhanced variant of Mem0 (91% p95 latency reduction, <5pt accuracy gap vs full-context, 2.59s p95). Add §2.5 "Three-Layer Architecture: events.jsonl (authoritative) + FalkorDB projection (queryable) + Mem0g cache (fast retrieval)". Trust invariant: Mem0g cache is rebuildable from events.jsonl, never trust-authoritative.

2. **Hermes Agent skill integration path.** Hermes Agent (Nous Research, 60K+ GitHub stars, MIT license, model-agnostic) has a plugin/skill system. Add §2.6 "Atlas as Hermes Agent Memory Skill" — specify the integration surface (HTTP API endpoints the skill calls, what the skill exposes back to Hermes's reasoning loop, how skill-generated facts flow into events.jsonl with attribution to Hermes-instance-key).

3. **Agent identity layer (Ed25519-DID).** V1's per-tenant HKDF keys generalize to per-agent keys. Add §2.7 "Agent Identities as W3C DIDs (did:atlas:<pubkey-hash>)". Specify how agent passports work — public key + verifiable history + revocation chain.

4. **Read-side API surface.** V1 has POST /api/atlas/write-node. V2 needs read endpoints. Add §2.8 "Read-Side API" — propose 3-5 endpoints: GET /entities/:id, GET /related/:id?depth=N, GET /timeline/:workspace?from=&to=, POST /query (Cypher passthrough?), POST /audit/:event_uuid (full provenance trail).

5. **MCP tool surface expansion.** V1's atlas-mcp-server exposes write_node + verify_trace. V2 needs query tools. Add §2.9 "MCP V2 Tool Surface" — propose tools: query_graph (Cypher-like), query_entities (semantic), query_provenance (trace any fact to source events), get_agent_passport (verify another agent's identity + reputation).

6. **Continuous regulator attestation architecture.** Add §2.10 "Federated Witness Cosignature for Regulators" — how a regulator's witness key gets added to the federation roster, what get cosigned, what the audit-trail looks like operationally.

7. **GDPR / Right-to-be-forgotten handling.** Add §3.3 (new open question): "Signed content vs deletable content separation. Strategy: events.jsonl signs hashes only, raw content lives in separate (deletable) storage. Trust property survives content deletion: hash exists, anchor exists, original content nullable = 'redacted but verified existed at time T'."

KEEP the existing v0 §0 (intent), §1 (positioning — refresh slightly per Doc A direction), §2.1-§2.4 (existing blueprint, trust invariant, Graphiti notes, FalkorDB section), §3 (existing open questions — expand to incorporate new ones), §4 (decomposition — refine V2-α/β/γ/δ to reflect three-layer architecture), §5 (Welle alignment), §6 (iteration CTA).

CRITICAL guidelines:
- Use ASCII diagrams where they help (the v0 has one — improve / extend if more would help).
- Be VERY explicit about trust invariants. Every layer addition must explain "what if this fails — does V1's trust property survive?". The answer for all new layers must be "yes, because they're derivative not authoritative".
- Each new section should be self-contained enough that an architect/security agent in Phase 2 can crit it without needing the whole doc context.
- Add references to specific Atlas crates / files where relevant (e.g., "events.jsonl format spec: see crates/atlas-trust-core/src/wire.rs").

When done: write only the file. Do NOT commit. Do NOT push. Return a diff summary (what was added vs the v0 baseline).
```

### 6.3 Doc C — Risk Matrix

```
subagent_type: security-reviewer
isolation: worktree
description: "Atlas V2 Doc C — Risk Matrix"

prompt:
You are writing a risk matrix for Atlas V2. This is NOT a generic risk doc — it's specifically about the strategic and architectural risks of the V2 pivot (verifiable knowledge graph substrate + Mem0g hybrid + Hermes Agent integration + agent identities).

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (entire document, especially §4)
- /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md (current v0, will be refined in parallel)
- /Users/nelso/Desktop/atlas/.handoff/v2-iteration-framework.md
- /Users/nelso/Desktop/atlas/docs/SECURITY-NOTES.md
- /Users/nelso/Desktop/atlas/docs/SEMVER-AUDIT-V1.0.md

WRITE: /Users/nelso/Desktop/atlas/.handoff/v2-risk-matrix.md

STRUCTURE:

# Atlas V2 — Risk Matrix v0

## Methodology
Score each risk on: Probability (LOW/MEDIUM/HIGH/CRITICAL), Impact (LOW/MEDIUM/HIGH/CRITICAL), Detectability (HIGH/MEDIUM/LOW — how fast we'd see it materialize), Reversibility (HIGH/MEDIUM/LOW — how recoverable). Plus mitigation status and owner.

## Risk Categories
### Strategic / Market Risks
### Architectural / Trust Risks
### Cryptographic / Crypto-Agility Risks
### Operational / Adoption Risks
### Legal / Regulatory Risks
### Vendor / Ecosystem Risks

## Detailed Risks
For each risk, use this template (8-12 risks total — quality over quantity):

### R-XX: <Short Title>
- **Category:** Strategic / Architectural / Crypto / Operational / Legal / Vendor
- **Description:** 2-3 sentences. What goes wrong, in what scenario.
- **Probability:** LOW / MEDIUM / HIGH / CRITICAL
- **Impact:** LOW / MEDIUM / HIGH / CRITICAL (separately considering: financial, technical, reputational)
- **Detectability:** HIGH / MEDIUM / LOW
- **Reversibility:** HIGH / MEDIUM / LOW
- **Current Mitigation Status:** NONE / PARTIAL / ADEQUATE / ROBUST
- **Mitigation Strategy:** Specific actions. What we'd do if it materialized + what we can do proactively.
- **Owner:** Engineering / Product / Legal / External-Security / Strategy
- **Review Cadence:** Quarterly / Per-Welle / Per-Release / Continuous

MUST-COVER risks (specifically address these — Nelson identified them as concerns):

1. **R-Adoption-Tipping-Point** — Atlas is only valuable when used. Catch-22: agents only adopt if Atlas has critical mass, mass only forms if agents adopt. (Strategy)

2. **R-Performance-Overhead** — Every write does crypto + chain hash + eventual Rekor anchor. At 10K writes/sec what breaks? (Operational/Architectural)

3. **R-UX-Complexity** — "Cryptographic provenance" is a feature humans don't want to think about. If UX surfaces too much trust-machinery, adoption fails. (Operational)

4. **R-Vendor-Capture** — Major AI vendors (Anthropic / OpenAI / Google) refuse to integrate or actively oppose. Adressable market shrinks. (Vendor)

5. **R-GDPR-Right-to-be-Forgotten** — Signed events are forever. EU GDPR Art. 17 conflict. (Legal)

6. **R-Privacy-vs-Public-Anchoring** — Sigstore Rekor anchoring is public. What if enterprise data is confidential? (Architectural)

7. **R-Post-Quantum-Migration** — Ed25519 secure today, future quantum-vulnerable. (Crypto)

8. **R-Mem0-Vendor-Risk** — Atlas-+-Mem0 hybrid depends on Mem0 staying healthy. Mem0 is venture-backed startup — vendor risk. (Vendor)

9. **R-Hermes-Adoption-Reversal** — Hermes Agent grew 60K stars in 2 months. If it stalls or gets supplanted, Atlas's Hermes-skill distribution play loses value. (Vendor)

10. **R-Projection-Determinism-Drift** — Graph DB / Mem0g cache must rebuild byte-identically from events.jsonl. If projection drifts silently, trust invariant breaks invisibly. (Architectural)

PLUS: Add 2-3 risks YOU identify that I haven't listed. Independent thinking required.

## Risk Heatmap
ASCII table mapping risk severity (Probability × Impact) — make it visually scannable.

## Open Questions for Phase 2 Critique
Especially around: which risks are under-quantified, which mitigations are unrealistic, which categories are missing.

CRITICAL guidelines:
- Be honest about mitigation status. If we have NO real mitigation, say NONE.
- Don't pad. 10 well-thought risks > 30 mediocre.
- Quantify where possible (e.g., "GDPR violations carry fines up to 4% of global revenue").
- Reference Atlas's V1 trust property as the bedrock — most risks should be analyzed against "does V1 invariant still hold under this failure mode?".

When done: write only the file. Do NOT commit. Do NOT push. Return a 5-bullet summary of the highest-severity risks.
```

### 6.4 Doc D — Competitive Landscape

```
subagent_type: general-purpose
isolation: worktree
description: "Atlas V2 Doc D — Competitive Landscape"

prompt:
You are writing a competitive landscape analysis for Atlas V2. The market spans two adjacent categories: (1) AI agent memory infrastructure (Mem0, Letta, Zep, Graphiti, Anthropic Memory, OpenAI Memory) and (2) human Second Brain tools (Obsidian, Notion, Roam Research, Logseq). Atlas's unique structural property is cryptographic trust — no current competitor has it.

You MUST do current research (this is 2026-05-12). Use WebSearch to confirm current state of each competitor: pricing, features, license, latest releases, user base. Do NOT rely on knowledge cutoff.

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (entire document, especially §4)
- /Users/nelso/Desktop/atlas/README.md
- /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md

WRITE: /Users/nelso/Desktop/atlas/.handoff/v2-competitive-landscape.md

STRUCTURE:

# Atlas V2 — Competitive Landscape v0 (2026-05)

## 1. Two Market Categories
1a. AI Agent Memory Infrastructure (target persona: AI engineer / agent builder)
1b. Human Second Brain / Personal Knowledge Management (target persona: knowledge worker / researcher)
1c. Atlas's unique position — substrate für BOTH, with cryptographic trust as the bridge

## 2. AI Agent Memory Layer Competitors
For each: License, Founded, Pricing, Features, User Base, Trust Property, Atlas Differentiator

### 2.1 Mem0
- Particularly verify Mem0g (graph variant) availability and current benchmarks
- Note that we plan to USE Mem0g as a complementary retrieval layer — they're not a pure competitor, they're a potential partner

### 2.2 Letta (formerly MemGPT)
### 2.3 Zep (and their Graphiti framework — note Graphiti is OSS, separate from Zep Cloud)
### 2.4 Anthropic Memory (Claude's native memory)
### 2.5 OpenAI Memory
### 2.6 Hindsight / Supermemory / Mem0-alternatives — short coverage only

## 3. Second Brain Competitors
### 3.1 Obsidian
- Verify current pricing tiers (free / Sync / Publish / Catalyst / Business)
- User base estimate
- Plugin ecosystem size
- Atlas-relevant: does Obsidian have ANY signature / verification plugin?

### 3.2 Notion
### 3.3 Roam Research
### 3.4 Logseq
### 3.5 Capacities, Tana, Heptabase — short coverage only

## 4. Knowledge Graph Tools (overlap with both categories)
### 4.1 Graphiti (Apache-2.0, supports FalkorDB backend — potential partner not pure competitor)
### 4.2 Neo4j (graph DB + Neo4j Browser UI — could host Atlas projection)
### 4.3 FalkorDB (graph DB + Browser — our planned V2 stack)
### 4.4 Kuzu (MIT license — pure OSS alternative to FalkorDB if SSPL becomes problem)

## 5. Trust / Verification Adjacent Tools
Check if any of these explicitly target AI memory / agent trust:
### 5.1 Sigstore ecosystem (we already use Rekor)
### 5.2 SLSA framework (we already implement L3)
### 5.3 VeritasChain Protocol (VCP v1.1, Dec 2025 — adjacent cryptographic AI audit log)
### 5.4 Any 2025-2026 "AI trust" projects that emerged

## 6. Comparison Matrix
ASCII table with rows = competitors, columns = (License / Pricing-Range / Trust-Property / Open-Source / Multi-Agent / Temporal / Provenance-API / GDPR-Compliant-by-design). Atlas in last row.

## 7. Strategic Insights
- Who's the most threatening competitor (and why)
- Who's the most natural partner (Mem0, Graphiti, Hermes Agent — explore each)
- Where are the white spaces Atlas can own
- What's the most likely competitor counter-move

## 8. Open Questions for Phase 2 Critique
- Did we miss any competitor?
- Is the "verifiable Second Brain" category real or aspirational?
- Are Mem0g and Graphiti truly partners, or will they evolve into trust-property competitors?

CRITICAL guidelines:
- Cite ALL sources (URLs at end of each subsection)
- Be honest where Atlas is weaker. If Mem0 has 5K production deployments and Atlas has 0, say so.
- Don't fabricate market data. If you can't verify a number, write "estimated" or "claimed by company".
- 2026-current data only. Verify everything via WebSearch.

When done: write only the file. Do NOT commit. Do NOT push. Return a 5-bullet summary of the most strategically important findings.
```

### 6.5 Doc E — Demo Sketches

```
subagent_type: general-purpose
isolation: worktree
description: "Atlas V2 Doc E — Demo Sketches"

prompt:
You are sketching demo scenarios for Atlas V2's landing page and investor/customer pitches. These demos need to make Atlas's unique value visible in 30-90 seconds of video or live interaction.

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (entire document, especially §4)
- /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md
- /Users/nelso/Desktop/atlas/README.md

WRITE: /Users/nelso/Desktop/atlas/.handoff/v2-demo-sketches.md

STRUCTURE:

# Atlas V2 — Demo Sketches v0

## Methodology
Each demo follows a 5-block storyboard:
1. **Setup (5-10s):** What the viewer sees first.
2. **Action (15-40s):** What happens — the agent does X, the graph populates, etc.
3. **Reveal (10-20s):** The verification moment — click → cryptographic proof appears.
4. **Implication (10-20s):** Why this matters (one-sentence explainer).
5. **CTA (5s):** What the viewer does next.

For each demo, also specify: target audience, target emotion (surprise / trust / power / clarity), technical assets needed (atlas-web, FalkorDB, Hermes-Agent, etc.), and rough production complexity (1-5 scale).

## Demo 1 — Multi-Agent Race (Verifiable Attribution)
TWO agents (Hermes Agent + Claude via MCP) writing into the SAME Atlas workspace in real-time. Each fact appears in the graph with the writing agent's color-coded passport key + Sigstore Rekor logIndex. Viewer clicks any fact → modal shows: signed-by Hermes-instance-X / written at T / Rekor anchor proof / no-tampering certificate. **Audience:** AI engineers, builders. **Emotion:** trust + power. **Why it matters:** "Every fact has a verified author. No more 'the AI said it'."

## Demo 2 — Continuous Audit Mode (Regulator Witness)
Simulate a regulator's witness key federated into Atlas's trust root. Agent writes a financial recommendation. Cosignature appears in real-time from regulator-witness-key. Viewer sees: agent-signature + regulator-cosignature + timestamp + Rekor anchor. **Audience:** Compliance officers, regulators, financial services. **Emotion:** trust + clarity. **Why it matters:** "Compliance is structural, not periodic. The regulator's key is in the system."

## Demo 3 — Agent Passport (Reputation Portability)
Show an agent (Hermes-instance-X) that has written facts into Atlas for 30 days. Viewer queries the agent's passport: 1,247 facts written, 0 retractions, 12 unique witness cosigners, used by 3 organizations. Hire this agent → it brings its verifiable track record. **Audience:** Multi-tenant AI deployers, AI marketplaces. **Emotion:** clarity + power. **Why it matters:** "Agents have CVs now. Cryptographic ones."

## Demo 4 — Verifiable Second Brain (Obsidian Comparison)
Side-by-side: Obsidian vault vs. Atlas Second Brain. User types a note in both. Atlas auto-signs. User edits Atlas note from another device → previous version is preserved with signature + timestamp. User pretends to be a malicious teammate editing Obsidian directly → no signature, no detection. Atlas equivalent → tampering detection visible. **Audience:** Knowledge workers, researchers, teams. **Emotion:** surprise + trust. **Why it matters:** "Your Second Brain, but trustable for the AI era."

## Demo 5 — Mem0g Hybrid (Speed + Trust)
Side-by-side: standard Atlas query (verified, slower) vs. Atlas+Mem0g hybrid query (verified, 91% faster). Same accuracy, same cryptographic provenance, 1.44s vs 17.12s. Viewer sees both timings + identical results. **Audience:** AI engineers worried about latency. **Emotion:** clarity. **Why it matters:** "Cryptographic trust without the speed tax."

## Demo Selection for Landing Page Hero
Recommend WHICH demo should be the landing page hero (single 30-60s loop). Reason about audience-fit, emotional resonance, demo-feasibility-at-current-product-state.

## Production Requirements
Per demo: what tech needs to exist (real or mocked), what UI work is needed, what's blocking each one TODAY.

## Open Questions for Phase 2 Critique
- Are these demos honest about current Atlas capabilities, or do they require V2-α/β/γ before they're real?
- Is the multi-agent race demo emotionally compelling enough to lead with?
- Should we have a "non-AI" demo for the Second Brain market (Demo 4) at all, or focus on agent-builder audience first?

CRITICAL guidelines:
- Be REALISTIC about what's demo-able TODAY vs after V2-α / V2-β. Flag each demo's readiness.
- Don't write demos that require capabilities Atlas doesn't have. If a demo needs Mem0g integration and Mem0g isn't connected yet, say "requires V2-β" prominently.
- Think visual. Describe what's on screen at each beat. Not just "agent writes fact" but "left-pane: chat interface, right-pane: graph viz, fact node animates into existence".
- Audience-focused. A compliance-officer demo and a developer demo have completely different vocabulary.

When done: write only the file. Do NOT commit. Do NOT push. Return a 5-bullet summary of which demo is strongest and why.
```

---

## 7. Phase 1 Plan (review BEFORE dispatching subagents)

**Step 1 (current session):** Reviewer the 5 subagent prompts above with Nelson. He may want to:
- Add a sixth doc (e.g., F-Security-Experts comes back in scope earlier than expected)
- Reframe one of the prompts
- Add specific constraints / focal points

**Step 2:** Dispatch all 5 subagents in parallel (single Agent tool message with 5 calls). Each gets `isolation: "worktree"` and writes to its own branch. Expected timing: 60-120 minutes for all 5 v0 docs.

**Step 3:** As subagents complete, review each output:
- File written?
- Internally consistent?
- Open-Questions section present and substantial?
- Worktree path returned (so we know which branch to merge)

**Step 4:** Create integration branch `v2/phase-1-foundation` from master. Merge each subagent's branch sequentially. Resolve trivial conflicts (none expected — different files).

**Step 5:** Open PR `v2/phase-1-foundation → master` — but DO NOT merge. PR exists for review-visibility only. Phase 2 critique-agents work AGAINST this PR branch.

**Step 6:** Update this handoff doc:
- Mark Phase 1 complete
- Add Phase 2 plan section (which 6 critique agents, what prompts, what targets)
- Update master HEAD reference if anything changed

**Step 7:** Tell Nelson Phase 1 complete + briefly summarize each Doc's main thesis. Ask for green light on Phase 2.

---

## 8. Phase 2-4 — placeholder for future sessions

Phase 2 (critique agents) gets its OWN careful planning pass before dispatch. Don't dispatch Phase 2 from this session even if Phase 1 finishes fast — convergence-criteria matter, careful planning matters, and a fresh-eyes review of Phase 1 outputs is more valuable than rushing to Phase 2.

Phase 2-4 structure is documented in `.handoff/v2-iteration-framework.md` §2-4. The session that picks up Phase 2 should:
1. Read this handoff doc (updated post-Phase-1 by Step 6 above)
2. Read each of the 5 Phase 1 docs
3. Review the iteration-framework Phase 2 spec
4. Draft 6 critique-agent prompts (similar style to §6 above, customized for crit-role)
5. Get Nelson's green light
6. Dispatch

---

## 9. Standing Atlas conventions (don't break these in V2 work)

- **Git workflow:** Always PR. Always SSH-signed commits. Direct push to master is blocked by Rulesets.
- **Cargo PATH:** `/c/Users/nelso/.cargo/bin/cargo.exe` (not in default PATH).
- **gh CLI:** `/c/Program Files/GitHub CLI/gh.exe` (not in default PATH).
- **Standing Protocol:** implement → parallel `code-reviewer` + `security-reviewer` → fix CRITICAL/HIGH + selected MEDIUMs in-commit → SSH-signed feat commit → docs PR.
- **CHANGELOG.md curation:** Hand-curated, Keep-a-Changelog format. Each welle/feature gets 1-3 bullet narrative under Added/Changed/Fixed/Security.
- **SemVer audit gate:** Any change to items in `docs/SEMVER-AUDIT-V1.0.md` (especially Locked items) must be in-commit-updated.
- **Tag-immutability:** V1.17 Welle B contract. Signed tags are permanent. SemVer-patch is the forward-fix for post-tag-publish issues (precedent: v1.0.0 → v1.0.1 Cargo.toml URL fix, Welle 14a).

---

## 10. What "weiter" / "next" / "go" should default to (post-Phase-1)

If Nelson says "weiter" after Phase 1 completes: DO NOT auto-dispatch Phase 2. Instead:
1. Confirm Phase 1 outputs are all on the integration branch + PR
2. Show Nelson the 5 doc summaries
3. Ask if anything in Phase 1 outputs surprises him / changes the strategy
4. Then start careful Phase 2 planning per §8

The whole point of the iteration framework is **deliberate, not rushed**. Every phase gets its own careful planning pass.

---

## 11. If anything is unclear

Don't guess. Don't extrapolate from training data. Either:
- Read more of the existing files
- Ask Nelson with a specific clarifying question
- Use WebSearch to verify current external state (Hermes Agent, Mem0, etc.)

The strategic context in §4 was hard-earned over multiple brainstorming iterations. Preserve it; don't dilute it.

---

**End of handoff document.** Next agent: start with §1 (mandatory pre-read), then summarize what you understood to Nelson, then proceed.
