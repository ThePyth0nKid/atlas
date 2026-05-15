# Atlas V2 — Session Handoff (V2-α SHIPPED + V2-β Phase 0–14.6 SHIPPED, v2.0.0-beta.1 LIVE)

> **🎯 FRESH-AGENT BOOTSTRAP DOC.** **READ §0-NEXT FIRST** (2026-05-15-after next-session entry — 5-min snapshot + **W18c Phase B fastembed wiring** as primary engineering-pipeline path post-v2.0.0-beta.1-LIVE + Nelson-only items). Then §0z7 (Phase 14 W19 v2.0.0-beta.1 ship convergence SHIPPED narrative, 2026-05-15), §0z6 (Phase 14 W18c Phase A supply-chain constants lifted SHIPPED narrative, 2026-05-15), §0z5 (Phase 13 W18b Mem0g implementation SHIPPED narrative, 2026-05-14), §0z4 (Phase 12 W18 Phase A Mem0g design SHIPPED narrative, 2026-05-15), §0z3 (Phase 11 W17c SHIPPED narrative, 2026-05-14 late-day), §0z2 (Phase 10 W17b SHIPPED narrative, 2026-05-14), §0 "Fresh-Context Onboarding", §0z (V2-β Phase 0–9.5 SHIPPED, 2026-05-13), §0-NOW (HISTORICAL: 2026-05-14 Docker-restart breakpoint resume), §0a–§0d (Phase 1–4 strategic-iteration SHIPPED, historical). Then **`docs/V2-MASTER-PLAN.md`** + **`docs/V2-BETA-ORCHESTRATION-PLAN.md`** + **`docs/V2-BETA-DEPENDENCY-GRAPH.md`** + **`docs/V2-BETA-MEM0G-SPIKE.md`** + **`docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md`**. Optional: **`.handoff/v2-master-vision-v1.md`** + **`.handoff/decisions.md`** (29 explicit decisions).

**Erstellt:** 2026-05-12. **V2-α-α.1 SHIPPED:** 2026-05-13 (8 Welles). **V2-β Phase 0–9.5 SHIPPED:** 2026-05-13 (18 PRs merged: #67-#86). **V2-β Phase 10-counsel + 10-cleanup SHIPPED:** 2026-05-14 (PRs #87/#88). **Phase 10-breakpoint SHIPPED:** 2026-05-14 (PR #89). **Phase 10 (W17b) SHIPPED:** 2026-05-14 (PR #90 `d216844`). **Phase 10.5 SHIPPED:** 2026-05-14 (PR #91 `b02ef2a`). **Phase 11 (W17c) SHIPPED:** 2026-05-14 (PR #92 `61ef036`). **Phase 11.5 SHIPPED:** 2026-05-14 (PR #93 `8bbc729`). **Next-session handoff prep SHIPPED:** 2026-05-14 (PR #94 `c16199b`). **Phase 12 (W18 Phase A Mem0g design) SHIPPED:** 2026-05-15 (PR #95 `3f228be`). **Phase 12.5 SHIPPED:** 2026-05-15 (PR #96 `08b31dc`). **Phase 13 (W18b Mem0g implementation) SHIPPED:** 2026-05-14 (PR #97 `2f2238b`). **Phase 13.5 SHIPPED:** 2026-05-15 (PR #98 `578f17f`). **Phase 13.6 SHIPPED:** 2026-05-15 (PR #99 `11dcae8` — handoff-prep with W19 + W18c plan-docs master-resident). **Phase 14 (W18c Phase A — Mem0g supply-chain constants lifted) SHIPPED:** 2026-05-15 (PR #100 `28700ae`). **Phase 14.5 SHIPPED:** 2026-05-15 (PR #101 `31e3756` — W18c-A consolidation: CHANGELOG + master-plan §6 + decisions `DECISION-ARCH-W18c-A` + handoff §0z6 + §0-NEXT refresh). **Phase 14 W19 (v2.0.0-beta.1 ship convergence) SHIPPED:** 2026-05-15 (PR #102 `1f5a51f` ship + PR #103 `81d363e` post-merge polish). **Phase 14.6 SHIPPED:** 2026-05-15 (THIS PR — W19 SHIPPED consolidation: CHANGELOG `[Unreleased]` + master-plan §6 + decisions `DECISION-ARCH-W19` + orchestration + dependency-graph + handoff §0z7 + §0-NEXT refresh). **Status:** **v2.0.0-beta.1 LIVE on master + GitHub + npm** (signed tag SHA `81d363e58eb9ec6b5234d1f4c4c091683e754a17`; npm dist-tag `latest = 2.0.0-beta.1` + `node = 2.0.0-beta.1`; GitHub Release prerelease at https://github.com/ThePyth0nKid/atlas/releases/tag/v2.0.0-beta.1; Sigstore Build L3 provenance attached, shasum `60d9160d43e3e4de89a236b40ee584522b020c56`). Master HEAD post-Phase-14.5 contains: W18b NEW workspace member crate `crates/atlas-mem0g/` (~2300 LOC, 8 ADR sub-decisions implemented as W17a-pattern Phase-A-scaffold with production-shape trait + 7-step secure-delete protocol + supply-chain verification + `embedding_erased` Layer-1 dispatch + state extension + Read-API endpoint + atlas-mem0g-smoke CI workflow + plan-doc) + **W18c Phase A supply-chain constants LIFTED** (9 compile-in pins — 5 hash digests + 4 URLs — against HuggingFace `BAAI/bge-small-en-v1.5` @ revision `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`; embedder still fails-closed pending W18c Phase B `try_new_from_user_defined` wiring) + this consolidation PR's CHANGELOG + master-plan §6 + decisions + handoff updates. **Reviewer-dispatch (W18b):** parallel `code-reviewer` + `security-reviewer` per Lesson #8 → 0 CRITICAL + 4 unique HIGH + 6 MEDIUM, all applied in-commit (fix-commit `717922c` on top of initial `80f6957`). **Reviewer-dispatch (W18c Phase A):** parallel reviewers → 0 CRITICAL + 0 HIGH + 1 overlapping MEDIUM (`python` → `python3` portability) + 3-4 overlapping LOWs, all applied in-commit (fix-commit `a66728e` on top of initial `5946a1b`). **Byte-pin reproduces:** `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` unchanged after W18b + W18c-A ships; 577 tests pass workspace-wide (delta -1 vs W18b 578: retired `pins_are_placeholder_until_nelson_verifies` gatekeeper); clippy `-D warnings` zero warnings; all triggered CI checks green incl. NEW `atlas-mem0g-smoke`. **Scaffold posture:** LanceDB ANN/search body fill-in still deferred to W18c Phase D follow-on welle (closes spike §12 V1-V4 verification gaps); supply-chain pins are now **lifted** but the fastembed `try_new_from_user_defined` wiring remains the only pre-operational gate (W18c Phase B). **Was als nächstes:** **W19 v2.0.0-beta.1 ship convergence milestone** (primary; workspace version bump + signed tag + GitHub Release + npm publish — see §0-NEXT) **OR W18c Phase B fastembed wiring** (alternate engineering-pipeline path; ~1 session agent-only — replaces `AtlasEmbedder::new` fail-closed gate with real init using the 9 Phase-A-lifted pins). W18c Phase C (V1-V4 cross-platform CI matrix) + Phase D (LanceDB body fill-in) follow Phase B. Counsel-Engagement-Kickoff continues parallel-track Nelson-led (per `DECISION-COUNSEL-1`).

---

## 0-NEXT. 2026-05-15+-after-Phase-14.6 next-session entry — W18c Phase B fastembed wiring (post-v2.0.0-beta.1-LIVE)

> **Read this section first when resuming Atlas work after 2026-05-15.** Brings a fresh agent from cold context to actionable W18c Phase B engineering work in <10 min. v2.0.0-beta.1 is LIVE end-to-end (signed tag, GitHub Release, npm registry, Sigstore Build L3 provenance attached); the next engineering-pipeline welle is W18c Phase B which **activates Layer 3 operational mode** by replacing the fail-closed embedder gate with a real init using the 9 Phase-A-lifted pins.
>
> **🎯 RECOMMENDED PICKUP:** **`.handoff/v2-beta-welle-18c-plan.md`** Phase B section (master-resident, ~200 lines total; Phase B is ~1 session agent-only — replaces `AtlasEmbedder::new` fail-closed gate with `fastembed::TextEmbedding::try_new_from_user_defined` using the 9 Phase-A-lifted pins). Phase C (V1-V4 cross-platform CI matrix Linux + Windows + macOS) + Phase D (LanceDB ANN/search body fill-in) follow Phase B.

### 5-min snapshot

- **Master HEAD:** `81d363e` (PR #103 W19-polish merged; THIS Phase 14.6 consolidation PR's merge-commit follows). Branch protections active; admin-merge pre-authorised in `.claude/settings.local.json`.
- **V2 status:** **v2.0.0-beta.1 LIVE on master + GitHub + npm.** V2-β Phase 0–14.6 all SHIPPED + **v2.0.0-beta.1 LIVE end-to-end** (signed tag SHA `81d363e58eb9ec6b5234d1f4c4c091683e754a17`, Good Ed25519 `nelson@ultranova.io` fingerprint `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`; GitHub Release prerelease https://github.com/ThePyth0nKid/atlas/releases/tag/v2.0.0-beta.1; npm registry `@atlas-trust/verify-wasm@2.0.0-beta.1` LIVE with dist-tags `latest = 2.0.0-beta.1` + `node = 2.0.0-beta.1`; Sigstore Build L3 provenance attached, shasum `60d9160d43e3e4de89a236b40ee584522b020c56`; wasm-publish.yml W11 race-fix validated 2nd time end-to-end). **Layer 2 ArcadeDB operational** (W17a trait + W17b driver + W17c CI/bench). **Layer 3 Mem0g SCAFFOLD-SHIPPED with W18c Phase A supply-chain pins lifted** (9 compile-in pins against HuggingFace `BAAI/bge-small-en-v1.5` @ `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`; embedder still fails-closed pending W18c Phase B fastembed `try_new_from_user_defined` wiring; semantic-search Read-API returns 501 stub until Phase B activates real init). 29 decisions logged in `.handoff/decisions.md`. 12 ADRs total; ADR-Atlas-012 SHIPPED, ADR-Atlas-013 reserved for W18c Phase B-D implementation amendments.
- **CI required-checks for any new PR:** (1) `Verify trust-root-modifying commits` — SSH-Ed25519 enforced; (2) `atlas-web-playwright` — path-filter auto-triggers on `.handoff/**` + `.github/workflows/**` + `apps/atlas-web/**` + `crates/atlas-{signer,trust-core,verify-wasm}/**` + `packages/*/**`. **Crates-only PRs need a `.handoff/**` doc-touch to trigger playwright** (Atlas Lesson #11). `atlas-arcadedb-smoke` + `atlas-mem0g-smoke` workflows are NOT required-checks yet (promote when ≥3 unrelated PR runs stable). All triggered CI checks GREEN on PR #102 + #103 — `mem0g-smoke` ✓, `atlas-web-playwright` ✓, `Verify trust-root` ✓.
- **Active welle:** **W18c Phase B fastembed wiring** (primary; engineering-pipeline parallel-track to v2.0.0-beta.1 LIVE; activates Layer 3 operational mode; ~1 session agent-only). v2.0.0-beta.1 ship convergence is COMPLETE (W19 SHIPPED via PR #102/#103). Next post-Phase-B targets are Phase C (cross-platform CI matrix) + Phase D (LanceDB ANN/search body fill-in).
- **Phase A unblocked the W18c parallel-track (Nelson-led work complete):** 9 supply-chain pins resolved via `tools/w18c-phase-a-resolve.sh` (auditable helper, re-runnable for revision rotations). Remaining W18c Phase B-D items are ALL agent-only: (b) **fastembed `try_new_from_user_defined` wiring** — replaces `AtlasEmbedder::new` fail-closed gate with real init using the 9 Phase-A-lifted pins; (c) **close V1-V4 verification gaps** from spike §12 (cross-platform CI matrix Linux + Windows + macOS + Lance v2.2 `_deletion_files` + model size measurement); (d) **lift LanceDB ANN/search body stubs** — currently `Mem0gError::Backend("not yet wired")`. ADR-Atlas-013 still reserved.
- **Blocked-on-Nelson (parallel, not engineering-pipeline-blocking):** Counsel-engagement firm selection + outreach (`.handoff/v2-counsel-engagement-scope.md` is RFP-ready; the `embedding_erased` audit-event payload is in counsel's review scope per ADR-Atlas-012 §5.4; **counsel-track now MORE valuable post-v2.0.0-beta.1-LIVE** — the beta tag itself is internal engineering milestone per `DECISION-COUNSEL-1` / `DECISION-COMPLIANCE-3` but the V2-β-1 LIVE release provides external signal for counsel/investors/customer-prospects); `RULESET_VERIFY_TOKEN` PAT configuration per `docs/OPERATOR-RUNBOOK.md` §16 (cosmetic-only, does not block merges; W19-R6 risk mitigated by ship completing without it). **W18c Phase A supply-chain lift complete** (~30 min Nelson task done 2026-05-15) — no remaining Nelson-blocked engineering items in the immediate W18c pipeline.

### Pre-flight checklist (bash, run from repo root)

```bash
cd /c/Users/nelso/Desktop/atlas
git status                                                # → clean
git checkout master && git pull origin master             # → up-to-date with master HEAD ≥ post-Phase-14.6
git log --oneline -6                                      # → top:
#   <Phase 14.6 consolidation merge>
#   81d363e docs(v2-beta/welle-19-polish): pre-tag-push doc edits (#103)
#   1f5a51f feat(v2-beta/welle-19): v2.0.0-beta.1 ship convergence (#102)
#   31e3756 docs(v2-beta/phase-14.5): consolidate W18c-A ... (#101)
#   28700ae feat(v2-beta/welle-18c-A): supply-chain constants lifted (#100)
#   11dcae8 docs(v2-beta/phase-13.6): handoff-prep W19 + W18c (#99)

"/c/Program Files/GitHub CLI/gh.exe" pr list --state open --json number,title  # → only archive PRs #59/#61/#62
/c/Users/nelso/.cargo/bin/cargo.exe test --workspace --quiet                   # → 577 tests green (post-Phase-14.6; delta -1 vs W18b 578: retired pins_are_placeholder_until_nelson_verifies gatekeeper)
/c/Users/nelso/.cargo/bin/cargo.exe clippy --workspace --no-deps -- -D warnings  # → 0 warnings
git verify-tag v2.0.0-beta.1                              # → Good ed25519 sig (signed tag SHA 81d363e58eb9ec6b5234d1f4c4c091683e754a17)
/c/Users/nelso/.cargo/bin/cargo.exe test -p atlas-projector --test backend_trait_conformance byte_pin --quiet  # → byte-pin reproduces

# Optional (~30 s; requires Docker): full ArcadeDB integration validation
bash tools/run-arcadedb-smoke-local.sh                    # → cross-backend + B1/B2/B3 green

# Optional: verify v2.0.0-beta.1 is LIVE on npm
npm view @atlas-trust/verify-wasm@2.0.0-beta.1 dist-tags   # → latest = 2.0.0-beta.1, node = 2.0.0-beta.1
```

If any check fails, STOP and investigate before starting W18c Phase B work. Atlas Standing Protocol Lesson #2: *"When in doubt, RUN the code."*

### W19 retrospective — v2.0.0-beta.1 LIVE end-to-end (SHIPPED 2026-05-15)

**W19 SHIPPED via PR #102 (`1f5a51f` ship) + PR #103 (`81d363e` post-merge polish).** Layer 2 ArcadeDB + Layer 3 Mem0g (scaffold + W18c-A supply-chain pins lifted) + verifier-rebuild all operational on master at ship time. Workspace version bumped from `2.0.0-alpha.2` to `2.0.0-beta.1`; signed tag pushed; GitHub Release published prerelease; npm publish auto-triggered via wasm-publish.yml W11 race-fix (validated 2nd time end-to-end). Analog V2-α-α.2 ship pattern from W11 (PR #76 `1839e82`).

**W19 delivered:**
- Workspace `Cargo.toml` + 5 lockstep package.json manifests version bumped `2.0.0-alpha.2` → `2.0.0-beta.1` (HIGH-1 reviewer fix added root `package.json` to lockstep scope; 6 manifests total)
- `CHANGELOG.md` `[Unreleased]` block converted to `## [2.0.0-beta.1] — 2026-05-15` + V2-β tripod ship summary paragraph; new empty `[Unreleased]` section inserted above
- NEW `docs/V2-BETA-1-RELEASE-NOTES.md` (~145 lines, 13 sections, scaffold-posture LOUDLY stated)
- NEW `docs/SEMVER-AUDIT-V2.0-beta.md` (~135 lines, 8 sections, mirrors V1.0 methodology — atlas-mem0g SemanticCacheBackend Locked, AtlasEmbedder Internal-deferred, LanceDbCacheBackend Locked-Behind-Flag `lancedb-backend`)
- README.md badge alt-text update (M-2 polish in PR #103)
- Signed annotated tag `v2.0.0-beta.1` SHA `81d363e58eb9ec6b5234d1f4c4c091683e754a17` (Ed25519 via `git tag -s -m ...`, Good signature)
- GitHub Release prerelease at https://github.com/ThePyth0nKid/atlas/releases/tag/v2.0.0-beta.1
- npm publish `@atlas-trust/verify-wasm@2.0.0-beta.1` via wasm-publish.yml (race-fix from W11 validated 2nd time); dist-tags `latest = 2.0.0-beta.1` + `node = 2.0.0-beta.1`
- Sigstore Build L3 provenance attached, shasum `60d9160d43e3e4de89a236b40ee584522b020c56`

**W19 success criteria (all met):**
- All required CI checks green at merge time on both PR #102 + PR #103 ✓
- Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces through both InMemoryBackend AND ArcadeDbBackend ✓
- Sigstore Build L3 provenance attached to npm publish (per W11 wasm-publish.yml race-fix) ✓
- `git verify-tag v2.0.0-beta.1` returns Good Ed25519 signature ✓
- npm registry shows `latest = 2.0.0-beta.1` ✓ (W11 race-fix validated 2nd time end-to-end)

**Posture preserved through W19 ship:**
- Byte-pin reproducing through both backends ✓
- All required CI checks green on master ✓
- 577 tests pass workspace-wide ✓
- Clippy `-D warnings` zero ✓
- Tag-immutability contract honored per V1.17 Welle B (signed tags permanent; post-tag fixes are forward-version SemVer-patch, NOT re-tag)

### W18c parallel-track framing (post-v2.0.0-beta.1-LIVE engineering-pipeline activation)

**W18c is the welle that closes the W18b-deferred items.** Phase A (Nelson supply-chain constant lift) is COMPLETE. Phases B-D are all agent-only engineering-pipeline items. The v2.0.0-beta.1 ship completed with W18c-B/C/D pending; what W18c-B/C/D activates is the **operational** Layer 3 (semantic-search actually returning real hits), not affecting the published v2.0.0-beta.1 tag (forward-version SemVer-patch v2.0.0-beta.2 is the recovery path if Phase B activation surfaces design amendments per ADR-Atlas-013 reservation).

**W18c's required deliverables:**
1. **Nelson supply-chain constant lift:** ✓ COMPLETE via PR #100 W18c Phase A SHIPPED 2026-05-15 — 9 compile-in pins lifted against HuggingFace `BAAI/bge-small-en-v1.5` @ revision `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a` via auditable `tools/w18c-phase-a-resolve.sh`. The 3 W18b `TODO_W18B_NELSON_VERIFY_*` placeholders + 6 Phase-B-prep constants (3 tokenizer-file SHA-256 pins + 3 tokenizer URL constants) all in place; `pins_are_placeholder_until_nelson_verifies` gatekeeper retired; `pins_well_formed_after_lift` UPGRADED to unconditional enforcement.
2. **fastembed `try_new_from_user_defined` wiring (Phase B):** PRIMARY POST-V2-β-1-LIVE WELLE. Replace the unconditional `Mem0gError::Embedder("supply-chain gate: ...")` fail-closed in `AtlasEmbedder::new` with a real init path using the 9 Phase-A-lifted pins (per-file download helpers + verification at every cold start; 3 tokenizer-file pins already in place from Phase A).
3. **V1-V4 verification gap closure** per spike §12: V1 LanceDB Windows `cleanup_old_versions` behaviour test on Windows CI runner; V2 fastembed-rs cross-platform determinism test on Linux + Windows + macOS CI matrix (assert 2 runs byte-equal); V3 Lance v2.2 `_deletion_files` semantics verification if adopting Lance 0.30+; V4 fastembed model size first-load measurement.
4. **LanceDB ANN/search body fill-in:** replace `Mem0gError::Backend("not yet wired")` placeholders in `crates/atlas-mem0g/src/lancedb_backend.rs::{upsert, search, erase, rebuild}` with real `tokio::task::spawn_blocking`-wrapped LanceDB calls (NOT `Handle::current().block_on()` — deadlocks per spike §7). `RESUME(spawn_blocking)` markers already placed at body sites.

**ADR-Atlas-013 reserved** for W18c implementation amendments (e.g. if LanceDB v0.30+ adoption changes `_deletion_files` semantics, or fastembed-rs API surface differs from W18b's expected `try_new_from_user_defined` shape).

### Nelson-only parallel-track items (engineering pipeline does NOT wait)

1. **Supply-chain constant lift (NEW post-W18b):** part of W18c (above). 3 real `const` values from HuggingFace `BAAI/bge-small-en-v1.5`. ~30 min Nelson task with `gh api` + curl.
2. **Counsel-engagement firm selection + outreach kickoff.** `.handoff/v2-counsel-engagement-scope.md` is RFP-ready (269 lines, 7 SOWs, 7-firm comparison matrix, DE + EN outreach templates). Selecting one or several firms from {Hogan Lovells Frankfurt, Bird & Bird Munich, Hengeler Mueller, Matheson, William Fry, Cleary Gottlieb Paris, Taylor Wessing} starts the 6-8-week clock for the GDPR Art. 4(1) hash-as-PII Path-B opinion. **Blocks V2-β public materials per `DECISION-COUNSEL-1`** — but does NOT block W18c / W19 engineering ship. Counsel review of `embedding_erased` audit-event payload is in scope per ADR-Atlas-012 §5.4.
3. **`RULESET_VERIFY_TOKEN` PAT configuration** per `docs/OPERATOR-RUNBOOK.md` §16. Fine-grained PAT with "Repository administration: read" scope, set as repo secret. Without it, `verify-branch-protection.yml` keeps firing red (exit 2: PAT-scope-insufficient). Cosmetic-only; does NOT block merges. ~5 min Nelson task.
4. **First-10-customers pipeline + TAM/SAM/SOM groundwork** (per `DECISION-BIZ-3` + `DECISION-BIZ-4`). Independent of engineering.

**Worktree cleanup (cosmetic; harness usually auto-cleans):** the W18b subagent worktree at `.claude/worktrees/agent-a598572963378b967` was reported locked by pid 54760 at admin-merge time. Local branch `feat/v2-beta/welle-18b-mem0g-impl` could not be deleted because of the locked worktree (remote branch IS deleted). Force-cleanup commands (run only if Claude harness has not auto-cleaned):
```bash
git worktree remove .claude/worktrees/agent-a598572963378b967 -f -f  # double-force unlocks
git branch -D feat/v2-beta/welle-18b-mem0g-impl                       # then delete local branch
```
Same shape for `agent-a3bdf20c434a613f8` (reviewer-fix subagent) if it persisted. Per Atlas standing protocol, force-removal is destructive — only run if the worktrees are confirmed orphaned (no active processes).

### Atlas Standing Protocol Lessons — consolidated through W19 (2026-05-15)

Numbered for cross-reference. Lessons #1-#9 from V2-β Phase 0-9.5. **Lessons #10-#13 from W17b/W17c. Lessons #14-#15 from W18 Phase A. Lessons #16-#18 from W18b. Lesson #19 NEW from W19.**

1. **Worktree-isolation leaks are real and recurring.** Subagent dispatch prompts MUST include explicit `git fetch origin && git checkout -B feat/<branch> origin/master` as first 3 actions. Parent verifies pre-flight before assuming agent worked correctly.
2. **When reviewers disagree on whether code is broken, RUN the code.** Theoretical findings can be wrong; behavioural tests are authoritative.
3. **Reviewer-driven MEDIUMs are non-optional for package conventions and boundary correctness.** Don't defer. (Exception: V2-γ-scope clean-ups are explicit defer-by-decision.)
4. **Cross-batch consistency-reviewer is a load-bearing V2-β invariant** (Orchestration Plan §3.5). Earns its dispatch every Phase consolidation.
5. **Architect subagent type has Read/Grep/Glob ONLY.** For doc-only spike welles, architect produces content; parent writes files. For code welles use `general-purpose`.
6. **`gh pr merge --admin` is pre-authorised in `.claude/settings.local.json`.** Use directly; no prompt needed.
7. **`strict_required_status_checks_policy: true` + trust-root-verify:** When `gh pr update-branch` creates a bot-signed merge commit, trust-root-verify fails. **Fix:** rebase locally onto fresh master (preserves SSH-Ed25519 signatures), `git push --force-with-lease`. Never use `gh pr update-branch` for trust-root-touching PRs.
8. **Subagent self-audit is best-effort, never load-bearing.** Parent ALWAYS runs the external `code-reviewer` + `security-reviewer` dispatch post-implementation in parallel (single message, 2 Agent calls). The W17b subagent self-reported "zero clippy warnings" — was wrong by 15 lints. The W17c subagent ditto.
9. **Branch protection blocks admin-merge while required CI checks are in-progress** (even with admin override). Wait for green via `gh run watch <id> --exit-status` in background; don't poll.
10. **`#[ignore]`-gated integration tests are blind spots until CI runs them.** W17b's `cross_backend_byte_determinism` existed and compiled cleanly but never ran against a live backend until W17c shipped the CI workflow that runs it. Two driver regressions surfaced immediately. **Lesson:** ship the CI infrastructure that runs `#[ignore]`-gated tests alongside the gated tests themselves. Don't let "deferred to next welle" become "deferred forever".
11. **`atlas-web-playwright` is path-filter-gated AND is a required CI check.** Crates-only PRs may not trigger it; without a triggered run, the required-check is "expected" but never fires, blocking the merge with `mergeStateStatus: BLOCKED`. **Workaround:** include any `.handoff/**` doc-touch in the PR (e.g. updating the W17b plan-doc with a status note). The W17b reviewer-fix and W17c reviewer-fix PRs both relied on this.
12. **ArcadeDB Cypher subset has reserved param names that ArcadeDB does not document.** `$from`, `$to` collide with SQL `CREATE EDGE ... FROM ... TO ...` keywords and silently empty result sets. `$label` collides with TinkerPop `T.label` token and raises `IllegalArgumentException`. Future Atlas Cypher work MUST avoid SQL-keyword param names. Suspected-reserved list (be defensive): `$from`, `$to`, `$where`, `$order`, `$label`, `$key`, `$value`, `$id`, `$select`. Verify any new param name against a live ArcadeDB instance before relying on it.
13. **Atomic Cypher pattern for schema-type bootstrap.** ArcadeDB Cypher's `MERGE (a)-[r:Edge]->(b)` silently no-ops if the Edge type does not yet exist. A single-statement `CREATE (a:Vertex)-[r:Edge]->(b:Vertex) WITH a, b, r DETACH DELETE a, b` registers both Vertex and Edge types as a side effect of the CREATE phase AND cleans up sentinels in one atomic HTTP roundtrip. Reusable pattern for any future ArcadeDB schema-type registration on fresh databases.
14. **Doc-only PRs benefit massively from parallel-reviewer dispatch.** The W18 Phase A design-only welle surfaced 16 substantive findings (2 HIGH security, 10 MEDIUM, 4 LOW) including a load-bearing TOCTOU race in the secure-delete protocol design that would have shipped silently to W18b's implementation if not caught. **Lesson:** parent ALWAYS runs the parallel-reviewer dispatch even on doc-only PRs — the cost is one parallel-agent-pair invocation, the value is catching design-time bugs before they hit code.
15. **Library-name vs concept-name conflation can mislead the entire welle.** Master-plan §3 named the layer "Mem0g cache" as a concept-reference; W18's research surfaced that Mem0g is a paper-name not a product. **Lesson:** when an external library or concept name appears in master-plan, the welle that builds on it should explicitly research-verify the current upstream packaging state, not trust the name as a stable artefact. Document the clarification in both spike + ADR so future agents don't re-derive the surprise.
16. **Scaffold-first impl is a legitimate W17a-pattern when verification gaps + supply-chain dependencies are open.** W18b shipped trait + 7-step secure-delete protocol + supply-chain verification path + dispatch surface as production-shape with body stubs + fail-closed posture. This is engineering judgment, NOT "incomplete work" — the alternative (block on V1-V4 verification + Nelson constant lift before any code lands) would have delayed Layer 3 ship by sessions. **Lesson:** trait + protocol + dispatch surface are reviewable + testable INDEPENDENTLY of the body bodies; reviewer-dispatch catches design defects before implementation defects compound them. Reserved follow-on welle (W18c-pattern) lifts the body stubs.
17. **Stub-equivalence: FS-walk fallback IS the production primitive for filesystem-level properties.** W18b's `precapture_fragments` uses `std::fs::read_dir` recursion instead of the eventual `lancedb::Table::list_fragments` API. This is functionally equivalent for GDPR Art. 17 byte-overwrite — the secure-delete protocol operates on real disk files regardless of which library wrote them. **Lesson:** test against the SECURITY PROPERTY (e.g. "bytes not recoverable via raw `fs::read`"), not the IMPLEMENTATION (e.g. "LanceDB API call sequence"). When the property is filesystem-level, the stub provides the property; when the property requires library-level semantics, the stub is a placeholder.
18. **Reviewer-dispatch HIGH findings cluster in supply-chain code.** The W18b reviewer-dispatch found 4 unique HIGH findings, ALL in the supply-chain verification path: real SHA-256 vs blake3-placeholder, `fastembed::TextEmbedding::try_new(Default::default())` bypassing the SHA-verified local path, gatekeeper test asserting only one of three constants, deterministic-PRG overwrite. None of these were in the secure-delete protocol or dispatch arm — those code paths reviewed clean. **Lesson:** supply-chain verification code is high-risk + high-leverage; budget extra reviewer focus on the embedder-init + constant-handling + verification-path-fail-closed flow specifically. The MEDIUM closures spread across other surfaces (empty-string guards, recursive walk, UTF-8 body-cap, `set_var` UB) were broader but lower-impact.
19. **Pre-tag-push multi-perspective review is load-bearing for irreversible-public-action ship events.** Before pushing the signed `v2.0.0-beta.1` tag, parent dispatched 4 parallel reviewers (architect + security + compliance/counsel + consistency/drift); all 4 returned GO with M-1 + M-2 + L-2 polish findings; the polish landed as PR #103 BEFORE tag-push. Once a signed tag is pushed and a GitHub Release published, the action is functionally irreversible (forward-version SemVer-patch is the only recovery per Atlas standing protocol — V1.17 Welle B tag-immutability contract). **Lesson:** for any ship event with public-registry write side effects (npm publish, signed tag push, GitHub Release), the parent ALWAYS runs a 4-perspective pre-action review even AFTER per-welle code+security reviewers have approved the merge. The cost is one parallel-agent-quad invocation; the value is catching slip-throughs (compliance phrasing, accessibility, dependency-graph drift) before they hit the public record. Standing-directive aligned: "Geh mit A, beste Sicherheit + Codequalität".

### Critical files / references (W18c Phase B start-of-session reading list)

- `docs/V2-MASTER-PLAN.md` §6 (Welle Decomposition + Phase 14 + Phase 14.6 SHIPPED) + §3 (Three-Layer Trust Architecture).
- `.handoff/decisions.md` `DECISION-ARCH-W19` (29th decision, entry post-Phase-14.6) + `DECISION-ARCH-W18c-A` + `DECISION-ARCH-W18b` + `DECISION-COUNSEL-1` + `DECISION-COMPLIANCE-3`.
- `docs/V2-BETA-1-RELEASE-NOTES.md` — V2-β-1 release notes (scaffold-posture LOUDLY stated; W18c parallel-track pointer; upgrade-from-alpha guide).
- `docs/SEMVER-AUDIT-V2.0-beta.md` — V2-β-1 SemVer surface contract companion; atlas-mem0g SemanticCacheBackend Locked / AtlasEmbedder Internal-deferred / LanceDbCacheBackend Locked-Behind-Flag.
- **W18c Phase B-relevant (PRIMARY engineering-pipeline welle):**
  - `crates/atlas-mem0g/src/embedder.rs::AtlasEmbedder::new` — the unconditional `Mem0gError::Embedder("supply-chain gate: ...")` to replace with `try_new_from_user_defined` wiring using the 9 Phase-A-lifted pins.
  - `crates/atlas-mem0g/src/embedder.rs` lines ~61-82 — the 9 lifted compile-in pins (3 W18b TODO replacements + 6 Phase-B-prep constants).
  - `crates/atlas-mem0g/src/lancedb_backend.rs` — `RESUME(spawn_blocking)` markers at the body-stub sites (Phase D scope).
  - `.handoff/v2-beta-welle-18c-plan.md` Phase B section — fastembed 5.13.4 `UserDefinedEmbeddingModel` constructor signature + 3 tokenizer-file SHA-256 pins consumption pattern.
  - `docs/V2-BETA-MEM0G-SPIKE.md` §12 — V1-V4 verification gaps (Phase C scope).
- **Layer-2 ship-template (W19 SHIPPED — analog v2.0.0-alpha.2 from W11):**
  - PR #102 (`1f5a51f`) + PR #103 (`81d363e`) — v2.0.0-beta.1 ship as template for any future V2-β-2 / V2-γ tag ship.
  - PR #76 (`1839e82`) — v2.0.0-alpha.2 ship (original template).
  - `.github/workflows/wasm-publish.yml` — Sigstore Build L3 provenance + race-fix from W11 (validated 2nd time end-to-end through W19).
- `crates/atlas-projector/src/lib.rs` invariants #1-#5 (load-bearing constraints W18c Phase B MUST preserve — esp. byte-pin reproducing through both backends).

### Risk register for W18c Phase B + remaining post-v2.0.0-beta.1 engineering-pipeline

- **W18c-R1 — Cross-platform embedding-determinism (MEDIUM, Phase C scope).** fastembed-rs determinism under pinned (ORT-version, threads=1, FP32) is documented but not formally guaranteed across Linux + Windows + macOS. **Verification gap V2** (spike §12). Mitigation: W18c-shipped TDD-RED test in `crates/atlas-mem0g/tests/embedding_determinism.rs` runs on CI matrix. If Windows fails, fallback to event_uuid-only cache-key on Windows builds; documented in operator-runbook.
- **W18c-R2 — LanceDB `cleanup_old_versions` Windows behaviour (MEDIUM, Phase C scope).** **Verification gap V1**. Mitigation: 50-line Rust integration test on Windows CI runner before lifting body stubs.
- **W18c-R3 — Real supply-chain constants drift over time (LOW with mitigation).** HuggingFace `BAAI/bge-small-en-v1.5` repo may receive new revisions; the pinned SHA may go stale. Mitigation: operator-runbook documents model rotation process; `tools/w18c-phase-a-resolve.sh` re-runnable for revision rotations; `pins_well_formed_after_lift` unconditional structural enforcement catches accidental constant regression.
- **W18c-R4 — fastembed `try_new_from_user_defined` API drift (LOW, Phase B scope).** Atlas pins `fastembed = "=5.13.4"` exact-version; any minor/patch upgrade requires re-verification of `UserDefinedEmbeddingModel` constructor signature. Mitigation: exact-version pin makes drift explicit at every dep-lock change. Phase B subagent dispatch should `cargo doc -p fastembed` to verify the constructor signature before wiring.
- **W18c-R5 — LanceDB v0.30+ `_deletion_files` semantics drift (LOW, Phase D scope).** If adopting Lance 0.30+ between Phase B and Phase D, the deletion-files semantics may differ from W18b's assumed shape. Mitigation: ADR-Atlas-013 reserved for this kind of design amendment.
- **W18c-R6 — Tag-immutability under Phase B design amendment (LOW).** If Phase B activation surfaces a fastembed API surface incompatibility requiring a SemVer-breaking change in atlas-mem0g, the recovery path is forward-version SemVer-patch (v2.0.0-beta.2). The v2.0.0-beta.1 tag is immutable per V1.17 Welle B contract.

---

## 0z7. V2-β Phase 14 (W19 v2.0.0-beta.1 ship convergence) SHIPPED — 2026-05-15

> **W19 SHIPPED narrative.** §0-NEXT above is the actionable next-session entry point (now W18c-Phase-B-focused as primary engineering-pipeline path post-v2.0.0-beta.1-LIVE); read this section if you need the W19 ship historical detail (what landed in PR #102 + PR #103, reviewer-dispatch outcomes, post-merge ship cascade, multi-perspective pre-tag review, fix-commit `1789e58` on `8287b97`). Phase 14 W19 closes the V2-β-1 ship convergence milestone; W18c Phase B-D remain queued as engineering-pipeline parallel-track to v2.0.0-beta.1 LIVE.

### What landed in PR #102 + PR #103

PR #102 (`1f5a51f`, ship convergence) — 12 files changed, +411/-36 lines (initial `8287b97` + reviewer fix-commit `1789e58`, squash-merged).

| File | Status | LOC | Brief |
|---|---|---|---|
| `Cargo.toml` (workspace) | MODIFY | 1 line | `version = "2.0.0-alpha.2"` → `version = "2.0.0-beta.1"` |
| `package.json` (root workspace manifest) | MODIFY | 1 line | Version bump (HIGH-1 reviewer fix added root to lockstep scope; was not in initial in-scope list) |
| `packages/atlas-bridge/package.json` | MODIFY | 1 line | `@atlas/bridge` version bump |
| `packages/atlas-cypher-validator/package.json` | MODIFY | 1 line | `@atlas/cypher-validator` version bump |
| `apps/atlas-mcp-server/package.json` | MODIFY | 1 line | `atlas-mcp-server` version bump |
| `apps/atlas-web/package.json` | MODIFY | 1 line | `atlas-web` version bump |
| `Cargo.lock` | MODIFY | ~12 lines | Auto-regenerated workspace-version refs from `cargo check --workspace` |
| `CHANGELOG.md` | MODIFY | +5/-1 | `[Unreleased]` → `[2.0.0-beta.1] — 2026-05-15` + V2-β tripod ship summary paragraph + new empty `[Unreleased]` section above |
| `docs/V2-BETA-1-RELEASE-NOTES.md` | NEW | ~145 lines | 13 sections — headline, Layer 1/2/3 status, scaffold-posture LOUDLY stated, W18c parallel-track pointer, upgrade-from-alpha guide, MCP SDK bumped per M-2 |
| `docs/SEMVER-AUDIT-V2.0-beta.md` | NEW | ~135 lines | 8 sections — mirrors V1.0 methodology; atlas-mem0g SemanticCacheBackend Locked + SemanticHit Locked + Mem0gError #[non_exhaustive] Locked + AtlasEmbedder Internal-deferred + LanceDbCacheBackend Locked-Behind-Flag `lancedb-backend` |
| `.handoff/v2-beta-welle-19-plan.md` | MODIFY | +60/-8 | Implementation Notes filled + 6 stale `578 tests` → `577` corrections + post-merge ship cascade verification placeholders |
| `README.md` | (not in PR #102) | — | (Polish landed in PR #103 — badge alt-text fix per M-2) |

PR #103 (`81d363e`, post-merge polish) — 3 files changed, +6/-2 lines.

| File | Status | LOC | Brief |
|---|---|---|---|
| `docs/V2-BETA-1-RELEASE-NOTES.md` | MODIFY | +4 | M-1 compliance polish: `embedding_erased` bullet rephrased (removes "GDPR-alignment-as-property" assertion; adds `DECISION-COUNSEL-1` + W18c Phase D pending qualifiers) |
| `README.md` | MODIFY | +1/-1 | M-2 accessibility polish: badge alt-text "Article 12" → "Art. 12 — design-aligned" |
| `.handoff/v2-beta-welle-19-plan.md` | MODIFY | +4 | Post-merge polish landed via PR #103 entry added to Implementation Notes |

### W19 reviewer-dispatch outcome

PR #102 (ship convergence) — parallel `code-reviewer` + `security-reviewer` per Atlas Standing Protocol Lesson #8 (single message, 2 Agent calls). **0 CRITICAL / 1 HIGH (overlap) / 3 MEDIUM (overlap) / 1 LOW.** All applied in-commit per Lesson #3 (fix-commit `1789e58` on top of initial `8287b97`):

| Severity | Code | Security | Unique | Disposition |
|---|---|---|---|---|
| CRITICAL | 0 | 0 | **0** | — |
| HIGH | 1 | 1 | **1** (overlap on root `package.json` lockstep) | Applied in-commit |
| MEDIUM | 2 | 2 | **3** (MCP SDK introspection bump + SEMVER-AUDIT draft artifact + cargo semver clarity) | Applied in-commit |
| LOW | 1 | 1 | **1** (CHANGELOG self-reference) | Applied in-commit |

**HIGH fix (root `package.json` lockstep):** initial commit followed plan exactly which listed 5 manifests; root `package.json` also carried `"version": "2.0.0-alpha.2"` and would have been left stale. Reviewer caught the inconsistency; fix-commit added root to the lockstep scope.

**MEDIUM closes:** (M-1) MCP server SDK introspection version string bumped to match new package version; (M-2) `docs/SEMVER-AUDIT-V2.0-beta.md` draft-artifact warning header added; (M-3) cargo SemVer prerelease ordering note clarified in release notes upgrade guide.

PR #103 (post-merge polish) — parallel `code-reviewer` + `security-reviewer` post-tag-push pre-publication. **6/6 PASS APPROVE both reviewers.** M-1 Release Notes GDPR phrasing + M-2 README badge alt-text + L-2 Phase D parenthetical (combined with M-1) — all flagged in the multi-perspective pre-tag review and landed atomically as PR #103 BEFORE the tag-push action.

### Post-merge ship cascade outcome

Parent-led shipping operations executed post-merge per W19 plan-doc step 7-9:

| Step | Outcome |
|---|---|
| Signed tag `v2.0.0-beta.1` pushed | SHA `81d363e58eb9ec6b5234d1f4c4c091683e754a17`; `git verify-tag` Good Ed25519 (`nelson@ultranova.io`, fingerprint `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`) |
| GitHub Release created | https://github.com/ThePyth0nKid/atlas/releases/tag/v2.0.0-beta.1 (prerelease, notes from `docs/V2-BETA-1-RELEASE-NOTES.md`) |
| wasm-publish.yml auto-triggered | All steps green: Publish to npm ✓ + Verify npm publish landed ✓ + Upload tarballs to GitHub Release backup channel ✓ (run-id `25919934805`) |
| npm registry verification | `@atlas-trust/verify-wasm@2.0.0-beta.1` LIVE; dist-tags `latest = 2.0.0-beta.1`, `node = 2.0.0-beta.1` (W11 dual-publish race-fix validated 2nd time end-to-end) |
| Sigstore Build L3 provenance | Attestations ✓ + signatures ✓; shasum `60d9160d43e3e4de89a236b40ee584522b020c56` |

### W19 posture preserved

- **Byte-pin reproduces unchanged** through both InMemoryBackend AND ArcadeDbBackend: `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` ✓
- **Tag-immutability contract honored** per V1.17 Welle B (signed tags permanent; post-tag fixes are forward-version SemVer-patch, NOT re-tag).
- **W11 wasm-publish.yml race-fix validated 2nd time** end-to-end (Phase 3 V2-α-α.2 ship was 1st validation; W19 is 2nd; race-fix design now has 2 independent live-registry success runs).
- **All required CI checks green at merge time** on both PR #102 + PR #103.
- **577 tests pass workspace-wide** (delta unchanged from post-W18c-A baseline).
- **Clippy `-D warnings` zero warnings** through workspace.
- **`embedding_erased` event-kind in Layer 1 dispatch** unchanged (W18b dispatch arm intact); `embedding_erasures` GraphState extension intact.

### Multi-perspective pre-tag review (4 perspectives, all GO)

Before pushing the signed tag — an irreversible-public-action step per Atlas Standing Protocol Lesson #19 — parent dispatched 4 parallel reviewers (per `~/.claude/rules/common/agents.md` multi-perspective pattern):

| Perspective | Verdict | Findings |
|---|---|---|
| architect | GO | Surface contract clean; no architectural regressions; trait surface unchanged |
| security | GO | No new attack surface; supply-chain verification path unchanged; fail-closed posture preserved |
| compliance/counsel | GO with M-1 polish | `embedding_erased` bullet phrasing required GDPR-counsel-pending qualifier; `DECISION-COUNSEL-1` + W18c Phase D pending added |
| consistency/drift | GO with M-2 + L-2 polish | README badge alt-text accessibility (M-2); Phase D parenthetical drift in release notes (L-2, combined with M-1 fix) |

All 4 perspectives returned **GO**. M-1 + M-2 + L-2 polish landed as PR #103 BEFORE tag-push (per standing directive "Geh mit A, beste Sicherheit + Codequalität"). The pattern is now codified as Lesson #19 — pre-tag-push multi-perspective review is load-bearing for irreversible-public-action ship events.

### What's next (W18c Phase B + Phase C/D follow-ons + counsel kickoff)

1. **W18c Phase B (fastembed wiring)** — PRIMARY POST-V2-β-1-LIVE engineering-pipeline welle. Replaces `AtlasEmbedder::new` fail-closed gate with `fastembed::TextEmbedding::try_new_from_user_defined` using the 9 Phase-A-lifted pins. Plan-doc `.handoff/v2-beta-welle-18c-plan.md` Phase B section. ~1 session agent-only. Activates Layer 3 operational mode (semantic-search returns real hits instead of 501 stub).
2. **W18c Phase C (V1-V4 verification gap closure)** — engineering-pipeline parallel-track. Cross-platform CI matrix Linux + Windows + macOS for fastembed-rs determinism, Lance v2.2 `_deletion_files` semantics, LanceDB Windows `cleanup_old_versions` behaviour. ~1 session.
3. **W18c Phase D (LanceDB ANN/search body fill-in)** — engineering-pipeline parallel-track. Replaces `Mem0gError::Backend("not yet wired")` placeholders with `tokio::task::spawn_blocking`-wrapped LanceDB calls per spike §7 (NOT `Handle::current().block_on()` — deadlocks). ~1-2 sessions.
4. **Counsel-engagement firm selection + outreach kickoff** — parallel-track Nelson-led per `DECISION-COUNSEL-1`. v2.0.0-beta.1 LIVE provides external signal for counsel/investors/customer-prospects; the kickoff is now MORE valuable post-ship.

---

## 0z6. V2-β Phase 14 (W18c Phase A Mem0g supply-chain constants lifted) SHIPPED — 2026-05-15

> **W18c Phase A SHIPPED narrative.** §0-NEXT above is the actionable next-session entry point (now W19-focused OR W18c-Phase-B-focused depending on engineering-pipeline choice); read this section if you need the W18c-Phase-A historical detail (what landed, parent-direct constant-lift posture, reviewer-dispatch outcome, fix-commit `a66728e`, demo-sketches doc kept separate scope). Phase 14 W18c-A closes the W18b deferred supply-chain-constant work; W18c Phase B-D remain queued as engineering-pipeline parallel-track to W19.

### What landed in PR #100 (`28700ae`)

Two commits squash-merged: initial `5946a1b` + reviewer fix-commit `a66728e`. ~345 net-additions across 4 files.

| File | Status | LOC | Brief |
|---|---|---|---|
| `crates/atlas-mem0g/src/embedder.rs` | MODIFY | ~360 lines | Three W18b `TODO_W18B_NELSON_VERIFY_*` placeholders lifted to real values pinned to HuggingFace `BAAI/bge-small-en-v1.5` revision `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`. Three NEW Phase-B-prep tokenizer-file SHA-256 constants (`TOKENIZER_JSON_SHA256` + `CONFIG_JSON_SHA256` + `SPECIAL_TOKENS_MAP_SHA256`) + three NEW tokenizer URL constants (revision-pinned). `_STRUCTURAL_PIN_CHECK` extended to assert non-emptiness of all 9 compile-in pins (5 hash digests + 4 URLs). Test surface: `pins_are_placeholder_until_nelson_verifies` RETIRED (W18b gatekeeper purpose served); `pins_well_formed_after_lift` UPGRADED to unconditional enforcement (W18b `is_placeholder` early-return removed); `pins_are_non_empty` extended. `AtlasEmbedder::new` body UNCHANGED — still fail-closed pending Phase B `try_new_from_user_defined` wiring; error message + docstring refreshed to point at Phase B resume guide. |
| `tools/w18c-phase-a-resolve.sh` | NEW | ~110 lines | Auditable helper script that resolved the 5 hash digest values (1 × SHA-1 + 4 × SHA-256) + ONNX file-size envelope check. Re-runnable for future revision rotations; deterministic-by-revision-SHA. Resolves `python3` first (falls back to `python`); validates every sha256sum output via inline `validate_sha256_hex` helper; uses `WORK_DIR` (not `TMPDIR`) per security-reviewer LOW-2; `d.get('sha', '')` hardening against HuggingFace API schema drift. |
| `CHANGELOG.md` | MODIFY | +17 | `[Unreleased]` "Changed — V2-β Welle 18c Phase A" entry with full pin enumeration + ONNX file-size envelope match + test surface changes + fail-closed posture preservation. |
| `.handoff/v2-beta-welle-18c-plan.md` | MODIFY | 2 | Status line flipped Phase A SHIPPED (9 compile-in pins; embedder still fails-closed pending Phase B). |

### W18c Phase A reviewer-dispatch outcome

Parallel `code-reviewer` + `security-reviewer` per Atlas Standing Protocol Lesson #8 (single message, 2 Agent calls). Both reviewers: **APPROVE** — 0 CRITICAL / 0 HIGH. Overlapping findings all fixed in-commit per Lesson #3 (fix-commit `a66728e` on top of initial `5946a1b`):

| Severity | Code | Security | Unique | Disposition |
|---|---|---|---|---|
| CRITICAL | 0 | 0 | **0** | — |
| HIGH | 0 | 0 | **0** | — |
| MEDIUM | 1 | 1 | **1** (overlap on `python` → `python3` portability) | Applied in-commit |
| LOW | 3 | 4 | several | Most applied in-commit; 2 LOWs deferred to W18c Phase B with rationale |

**MEDIUM fix:** `tools/w18c-phase-a-resolve.sh` now resolves `python3` first, falls back to `python` if present, errors clearly if neither found. Honours `PYTHON_BIN` env override.

**LOWs applied in-commit:**
1. SHA-count off-by-one ("six SHAs" → actual "5 hash digests = 1 × SHA-1 + 4 × SHA-256") fixed in 4 doc locations (embedder.rs module docstring + Phase B header comment + `_STRUCTURAL_PIN_CHECK` doc-comment + CHANGELOG + welle-18c plan-doc Status line + PR description).
2. `TMPDIR` env-var shadowing renamed to `WORK_DIR` throughout helper script.
3. Missing sha256-output format validation in helper script — added `validate_sha256_hex` helper invoked after every `sha256sum | cut` capture.
4. Stale "## Three compiled-in constants" module section header replaced with "## Compiled-in supply-chain pins (9 total: 5 hash digests + 4 URLs)" + extended enumerated bullet list.
5. HuggingFace API `'sha'` field hardening via `d.get('sha', '')` plus enhanced error message naming schema-change as the likely cause.

**LOWs deferred to W18c Phase B (engineering-pipeline-tracked):**
- `AtlasEmbedder::new` pre-gate model download (currently triggers a 130 MB download then fails closed — inefficient but functionally correct; Phase B replaces the fail-closed return with `try_new_from_user_defined` so the download succeeds end-to-end).
- ONNX file size compile-time assertion (currently documented as 133,093,490 bytes in docstring but not asserted in code — defensible as content-addressed SHA-256 is the real gate; Phase B could add an `ONNX_FILE_SIZE_BYTES` const + test assertion for defence-in-depth).

### CI results post-merge

All 3 triggered checks GREEN on master HEAD `28700ae`:
- ✓ `Verify trust-root-modifying commits` (required, SSH-Ed25519, 6 s)
- ✓ `atlas-web-playwright` (required, 4 min 9 s)
- ✓ `mem0g-smoke` (NEW W18b CI workflow; not-yet-required-check, 1 min 11 s) — first live-infra validation of post-Phase-A Layer-3 code paths

`atlas-arcadedb-smoke` + HSM workflows not triggered (path-filter scope). 577 tests pass workspace-wide (delta -1 vs handoff 578: retired `pins_are_placeholder_until_nelson_verifies`); clippy `-D warnings` zero warnings; byte-pin `8962c168…e013ac4` reproduces unchanged through both InMemoryBackend AND ArcadeDbBackend.

### W18c Phase A posture preserved

- **Trust property unchanged:** Layer 3 NEVER trust-authoritative; `AtlasEmbedder::new` body UNCHANGED at runtime level (still fails closed pending Phase B). No bypass path introduced.
- **TLS pinning intact:** `reqwest::blocking::Client::builder().https_only(true)` semantics preserved; all 4 URLs are `https://huggingface.co/...` revision-pinned.
- **Atomic-lift invariant intact:** `pins_well_formed_after_lift` enforces length + lowercase-hex + huggingface.co-origin + revision-SHA-in-path across all 9 pins; any single constant reverted to a placeholder fails the length or hex check.
- **Compile-time backstop:** `_STRUCTURAL_PIN_CHECK` const-eval block prevents accidental blanking via refactor.

### Demo-sketches doc kept separate scope (not in this PR)

`.handoff/v2-demo-sketches.md` (~1283 lines, V2 marketing/strategy draft v0 covering 5 demo storyboards + landing-page hero selection + production requirements + 15 open Phase-2 critique questions) remains Untracked on master. Conceptually separate from supply-chain work; will land as its own doc-only PR if/when the strategy track is formally resumed.

### What's next (Phase 14 + W18c Phase B-D)

1. **W19 v2.0.0-beta.1 ship convergence milestone** — workspace version bump + signed tag + GitHub Release + npm publish (per §0-NEXT W19 framing). Plan-doc `.handoff/v2-beta-welle-19-plan.md` master-resident + ready-to-dispatch. Phase A completion makes the release-notes scaffold-posture disclosure stronger ("supply-chain pins verified, embedder wiring pending" vs "all placeholders").
2. **W18c Phase B (fastembed wiring)** — engineering-pipeline parallel-track. Replaces `AtlasEmbedder::new` fail-closed gate with `fastembed::TextEmbedding::try_new_from_user_defined` using the 9 Phase-A-lifted pins. Plan-doc `.handoff/v2-beta-welle-18c-plan.md` Phase B section. ~1 session.
3. **W18c Phase C (V1-V4 verification gap closure)** — cross-platform CI matrix Linux + Windows + macOS for fastembed-rs determinism, Lance v2.2 `_deletion_files` semantics, LanceDB Windows `cleanup_old_versions` behaviour. ~1 session.
4. **W18c Phase D (LanceDB ANN/search body fill-in)** — replaces `Mem0gError::Backend("not yet wired")` placeholders with `tokio::task::spawn_blocking`-wrapped LanceDB calls per spike §7. ~1-2 sessions.

---

## 0z5. V2-β Phase 13 (W18b Mem0g Layer-3 cache implementation) SHIPPED — 2026-05-14

> **W18b SHIPPED narrative.** §0-NEXT above is the actionable next-session entry point (now W19-focused with W18c parallel-track items); read this section if you need the W18b historical detail (what landed, the W17a-pattern Phase-A-scaffold posture, reviewer-dispatch outcome, fix-commit `717922c`, W18b-derived Lessons #16-#18). Phase 13 wraps the Layer-3 implementation as a W17a-pattern scaffold; Phase 14 W19 ships v2.0.0-beta.1; W18c follow-on welle (parallel-track) closes the deferred body fill-in + supply-chain constant lift.

### What landed in PR #97 (`2f2238b`)

Two commits squash-merged: initial `80f6957` + reviewer fix-commit `717922c`. 9892 net-additions across 16 files (incl. Cargo.lock +6307 lines from `lancedb` + `fastembed` transitive deps).

| File | Status | LOC | Brief |
|---|---|---|---|
| `crates/atlas-mem0g/Cargo.toml` | NEW | 95 | NEW workspace member crate; deps `lancedb` (feature-gated) + `fastembed` + `sha2` (HIGH-1 fix) + `getrandom` (HIGH-4 fix) + workspace-shared. `lancedb-backend` feature default-OFF so workspace `cargo check` stays fast for contributors not touching Layer 3. |
| `crates/atlas-mem0g/src/lib.rs` | NEW | 492 | Production `SemanticCacheBackend` trait per spike §7 (object-safe + Send + Sync; sync methods for `spawn_blocking` wrapping). Public types: `SemanticHit` (cite-back `event_uuid` ALWAYS present), `InvalidationPolicy` (TTL + on-event + Layer-1-head-divergence triple per ADR §4 sub-decision #6), `Mem0gError` (`#[non_exhaustive]`). Re-exports projector helpers `check_workspace_id` + `check_value_depth_and_size`. |
| `crates/atlas-mem0g/src/embedder.rs` | NEW | 570 | Atlas-controlled supply-chain verification path per ADR §4 sub-decision #2. Three `TODO_W18B_NELSON_VERIFY_*` placeholder consts with fail-closed posture: `AtlasEmbedder::new` returns `Mem0gError::Embedder("supply-chain gate: ...")` UNCONDITIONALLY until Nelson lifts. **HIGH-1 reviewer fix:** real `sha2::Sha256` replacing blake3-prefixed placeholder (would have silently always-failed on Nelson constant lift). **HIGH-3 reviewer fix:** `pins_are_placeholder_until_nelson_verifies` asserts ALL three constants + post-lift well-formedness companion test. **MEDIUM-5 reviewer fix:** `Once::call_once` wrap of `set_var("OMP_NUM_THREADS", "1")` closing Rust 2024 UB on global `environ`. |
| `crates/atlas-mem0g/src/secure_delete.rs` | NEW | 475 | 7-step pre-capture-then-lock-then-overwrite protocol per ADR §4 sub-decision #4: ACQUIRE per-(workspace, table) `tokio::sync::RwLock` → PRE-CAPTURE fragment paths → `delete()` → `cleanup_old_versions(0)` → PRE-CAPTURE HNSW `_indices/` paths → OVERWRITE each path → RELEASE → emit audit-event OUTSIDE lock. **HIGH-4 reviewer fix:** `getrandom::getrandom` OS CSPRNG replacing deterministic blake3-seeded keyed on `(path, remaining)`. **MEDIUM-2 reviewer fix:** missing pre-captured path surfaces `Mem0gError::SecureDelete { step, reason }` (was silent continue — false-attestation risk). |
| `crates/atlas-mem0g/src/lancedb_backend.rs` | NEW | 468 | `LanceDbCacheBackend` impl. Trait methods + `precapture_fragments` + `precapture_indices` helpers (depth-recursive `walk_collect_filtered` — **MEDIUM-3 reviewer fix**, was single-level). LanceDB ANN/search body fill-in deferred to W18c with `RESUME(spawn_blocking)` markers at body sites. |
| `crates/atlas-mem0g/tests/embedding_determinism.rs` | NEW | 125 | V2 verification gap test (feature-gated behind `lancedb-backend`); embedder fail-closed unit-test path. |
| `crates/atlas-mem0g/tests/secure_delete_correctness.rs` | NEW | 187 | V1 verification gap + raw-bytes-not-recoverable + concurrent-write race-test + defence-in-depth missing-paths (4/4 pass). |
| `crates/atlas-mem0g/tests/mem0g_benchmark.rs` | NEW | 296 | B4/B5/B6 `#[ignore]`-gated behind `ATLAS_MEM0G_BENCH_ENABLED=1`. Mirrors W17c `arcadedb_benchmark.rs` exactly. |
| `crates/atlas-projector/src/upsert.rs` | MODIFY | +445 net | NEW `apply_embedding_erased` dispatch arm per ADR §4 sub-decision #5. Mirrors `apply_anchor_created` structure with empty-string guards for `event_id` + `workspace_id` + `erased_at` (**MEDIUM-1 reviewer fix:** added `erased_at` guard). Required fields + optional fields; append-only duplicate-refusal via `MissingPayloadField` variant pattern. |
| `crates/atlas-projector/src/state.rs` | MODIFY | +70 | NEW `embedding_erasures: BTreeMap<String, EmbeddingErasureEntry>` field on `GraphState`; NEW `EmbeddingErasureEntry` struct (`#[non_exhaustive]`); NEW `upsert_embedding_erasure` helper. |
| `crates/atlas-projector/src/canonical.rs` | MODIFY | +85 | Extends `build_canonical_bytes` with omit-when-empty serialisation for `embedding_erasures` (preserves V14 invariant). Byte-pin reproduces unchanged. |
| `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts` | NEW | 217 | POST endpoint returning top-k `SemanticHit` with cite-back `event_uuid`. Response-time normalisation default 50 ms (env-configurable); both cache-hit + cache-miss + 501 stub paths obey timing-normalisation (no API-boundary timing distinction). Zod-strict input; workspace-id regex; k bounded [1,100]; query length bounded [1,4096]; body byte-cap 32 KB via `Buffer.byteLength(rawText, 'utf8')` (**MEDIUM-4 reviewer fix:** was JS `.length` UTF-16 char count). |
| `.github/workflows/atlas-mem0g-smoke.yml` | NEW | 132 | Linux Ubuntu lane analog `atlas-arcadedb-smoke.yml`. SHA-pinned actions; `permissions: contents: read`; paths-gated; 10 min timeout; HuggingFace model file cache keyed on `hashFiles('crates/atlas-mem0g/src/embedder.rs')`. |
| `.handoff/v2-beta-welle-18b-plan.md` | NEW | 228 | Plan-doc per template + Implementation Notes + deferred-items resume guide (HIGH-2 fastembed wiring + 3 tokenizer-file pins are pre-V2-β-1-ship blockers; W18c follow-on lifts LanceDB ANN bodies + cross-platform CI matrix). |
| `Cargo.toml` (workspace) | MODIFY | +6 | Add `crates/atlas-mem0g` to members. |

### W18b reviewer-dispatch outcome

Parallel `code-reviewer` + `security-reviewer` per Atlas Standing Protocol Lesson #8 (single message, 2 Agent calls). Aggregated findings:

| Severity | Code | Security | Unique | Disposition |
|---|---|---|---|---|
| CRITICAL | 0 | 0 | **0** | — |
| HIGH | 2 | 3 | **4** (1 overlap on SHA-256→blake3) | All applied in-commit (fix-commit `717922c`) |
| MEDIUM | 3 | 4 | **6** | All applied in-commit |
| LOW | 2 | 4 | several | Selected fixes applied; remainder deferred-with-rationale to W18c / V2-γ |

**HIGH summary (all in `embedder.rs` supply-chain path — Lesson #18):**
1. **H-1 SHA-256 was blake3:** `sha256_via_reqwest_stack` returned `"blake3-placeholder-{hex}"` regardless of input. Function contracted to produce SHA-256 (compared against `ONNX_SHA256` constant which per ADR §4 sub-decision #2 is SHA-256). Would have silently always-failed on Nelson constant lift. Fix: `sha2::Sha256` + streaming `sha256_file` + RFC-6234 unit tests.
2. **H-2 fastembed bypass:** `AtlasEmbedder::new` called `fastembed::TextEmbedding::try_new(Default::default())` which triggered fastembed's OWN model fetch from HuggingFace WITHOUT Atlas's SHA check, bypassing the entire supply-chain gate. Fix (deferred-with-gate posture): unconditional `Mem0gError::Embedder("supply-chain gate: ...")` until Nelson lifts + `try_new_from_user_defined` wiring (requires 3 additional tokenizer-file SHA-256 pins; W18c).
3. **H-3 gatekeeper incomplete:** `pins_are_placeholder_until_nelson_verifies` asserted only `ONNX_SHA256`. Partial lift would silently pass. Fix: assert all three constants + post-lift well-formedness companion test (64-char hex SHA-256, 40-char hex revision SHA, `https://huggingface.co` URL prefix).
4. **H-4 deterministic random in secure-delete:** `fill_random_bytes` blake3-seeded keyed on `(path, remaining)`. Adversary with workspace storage layout could recompute exact overwrite pattern. Fix: `getrandom::getrandom` OS CSPRNG; `fill_random_bytes_actually_random` test asserts two fills produce non-identical bytes.

**MEDIUM summary:** empty-string `erased_at` guard symmetric with event_id/workspace_id; non-silent secure-delete on missing pre-captured paths; recursive fragment walk (depth 0/1/2); UTF-8 body-cap; `Once::call_once` wrap of `set_var`; `spawn_blocking` doc clarification + `RESUME(spawn_blocking)` markers at body sites.

### CI results post-merge

All 7 checks GREEN (4 always-on + 3 path-gated):
- ✓ `Verify trust-root-modifying commits` (required, SSH-Ed25519)
- ✓ `atlas-web-playwright` (required, ~7 min)
- ✓ `atlas-mem0g-smoke` (NEW W18b CI workflow; not-yet-required-check, ~2 min)
- ✓ `atlas-arcadedb-smoke` (path-triggered by `crates/atlas-projector/**` touches; byte-pin reproduces through ArcadeDB live)
- ✓ `hsm-byte-equivalence` (~2 min)
- ✓ `hsm-wave3-smoke`
- ✓ `hsm-witness-smoke`

578 tests pass workspace-wide; clippy `-D warnings` zero; byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces unchanged.

### W18b session lessons (now Lessons #16-#18 in §0-NEXT)

1. **Lesson #16 — Scaffold-first impl is legitimate W17a-pattern when verification gaps + supply-chain dependencies are open.** Production-shape trait + protocol + dispatch surface + fail-closed body stubs is reviewable + testable INDEPENDENTLY of body fill-in. Catches design defects before implementation defects compound them.
2. **Lesson #17 — Stub-equivalence: FS-walk fallback IS the production primitive for filesystem-level properties.** Test against the SECURITY PROPERTY (bytes not recoverable via raw `fs::read`), not the IMPLEMENTATION (specific LanceDB API call sequence).
3. **Lesson #18 — Reviewer-dispatch HIGH findings cluster in supply-chain code.** All 4 W18b HIGH findings were in `embedder.rs`; the secure-delete protocol + dispatch arm + state extension reviewed clean. Budget extra reviewer focus on embedder-init + constant-handling + verification-path-fail-closed flow.

### What's next (Phase 14 + W18c parallel-track)

1. **W19 v2.0.0-beta.1 ship convergence milestone** — workspace version bump + signed tag + GitHub Release + npm publish (per §0-NEXT W19 framing). Layer 2 + Layer 3 (scaffold) operational; analog V2-α-α.2 ship pattern from W11.
2. **W18c parallel-track** (pre-V2-β-1-ship-operational blocker, NOT W19 ship gate): Nelson supply-chain constant lift + fastembed `try_new_from_user_defined` wiring + close V1-V4 verification gaps + lift LanceDB ANN/search body stubs. ADR-Atlas-013 reserved.
3. **Counsel-Engagement-Kickoff** (parallel, Nelson-led). Per `DECISION-COUNSEL-1` blocks V2-β public materials (NOT the tag itself).

---

## 0z4. V2-β Phase 12 (W18 Phase A Mem0g Layer-3 cache design) SHIPPED — 2026-05-15

> **W18 Phase A SHIPPED narrative.** §0-NEXT above is the actionable next-session entry point (now W19-focused); read this section if you need the W18 Phase A historical detail (what landed, what reviewer-findings surfaced, session lessons, Mem0g-naming clarification). Phase 12 wraps the Layer-3 design phase end-to-end with all 8 binding sub-decisions locked; Phase 13 W18b implements against this design (now SHIPPED — see §0z5).

### What landed in PR #95 (`3f228be`)

| File | Status | Brief |
|---|---|---|
| `docs/V2-BETA-MEM0G-SPIKE.md` | NEW | ~520 lines, 13 sections. Comparative spike of LanceDB / Qdrant / Mem0 / fastembed-rs / sqlite-vec / USearch / SurrealDB; resolves 6 design questions; recommendation locked. Includes the Mem0g-naming clarification (paper-name not product). |
| `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` | NEW | ~430 lines, 9 sections. **8 binding sub-decisions** in §4 (vector-store choice + embedder + supply-chain + cache-key + secure-delete + GDPR audit-event + cache-invalidation + crate boundary + bench-shape). |
| `.handoff/v2-beta-welle-18-plan.md` | NEW | ~280 lines. Plan-doc per template + 8-risk register + ready-to-dispatch W18b subagent skeleton. |

**Total diff:** 1158 insertions, 3 NEW files. Zero Rust touched. All 7 V2-α byte-determinism CI pins unchanged; clippy `-D warnings` clean; trait surface stable.

### Critical clarification surfaced (master-plan continuity preserved)

*"Mem0g"* is **not a separate product** — it is a research-paper name (arXiv:2504.19413, mem0ai team, 2026) describing the graph-augmented memory mode in the open-source `mem0` Python package. As of 2026-05, the `--graph` CLI flag was removed from main branch in favour of per-project config; "Mem0g" the product is `mem0` configured with graph-mode enabled. Master-plan §3 naming preserved as the **concept name**; Atlas's implementation is Atlas-controlled Rust (`lancedb` + `fastembed-rs` paired). Mem0 the company remains a strong **partner** candidate per master-plan §8 (cross-promotional adapter pattern, no technical dependency).

### Locked recommendation (ADR §4)

**Atlas Layer 3 = pure-Rust embedded stack: `lancedb 0.29.0` (Apache-2.0) + `fastembed-rs 5.13.4` (Apache-2.0) with `bge-small-en-v1.5` FP32 ONNX.** Both linked into NEW workspace member crate `crates/atlas-mem0g/` (W18b implements). Encapsulated behind `SemanticCacheBackend` Rust trait (sketch in spike §7); Qdrant pivot path documented (LP1-LP5 trigger thresholds in spike §9; ~3 sessions to migrate via trait swap).

**Mem0 the product rejected** on three independent blockers: (a) Python runtime co-resident with Atlas violates Hermes-skill `npx` distribution constraint; (b) cloud-default OpenAI embedder is non-deterministic + third-party vendor on trust-substrate critical path; (c) secure-delete delegated, Atlas does NOT control the primitive.

### W18 Phase A reviewer-dispatch outcome (Atlas Standing Protocol Lesson #8)

Parallel `code-reviewer` + `security-reviewer` post-spike-draft (single message, 2 Agent calls):

| Severity | Code | Security | Total | Disposition |
|---|---|---|---|---|
| CRITICAL | 0 | 0 | **0** | — |
| HIGH | 0 | 2 | **2** | All applied in-commit |
| MEDIUM | 5 | 5 | **10** | All applied in-commit (Lesson #3) |
| LOW | 2 | 2 | **4** | All applied in-commit |

**Notable HIGH findings closed:**
1. **H-1 Secure-delete TOCTOU race** (security): wrapper sequence rewritten with pre-capture-then-lock-then-overwrite protocol — fragment paths captured BEFORE `cleanup_old_versions`, write lock held for entire wrapper, no enumerate-post-cleanup gap. Concurrent-write race-test added to W18b acceptance.
2. **H-2 Model download URL pinning** (security): three Atlas-controlled `const` values bound for W18b — HuggingFace revision SHA + ONNX file SHA256 + Atlas-source location. Re-verification at every cold start. Operator-runbook documents model rotation process.

**Cross-doc-flagged MEDIUM (both reviewers):** original Layer-3 cache-invalidation trigger #3 referenced Layer-2 `graph_state_hash` — contradicts the Layer-authority correction (Mem0g is Layer-1-direct per Phase-2 Architect H-3). Replaced with Layer-1-head-divergence trigger; Layer-2 cross-check reframed as opportunistic defence-in-depth ONLY, NOT load-bearing for cache validity.

### W18 session lessons (load-bearing for W18b+)

These extend the existing Atlas Standing Protocol Lessons #1-#13 (consolidated in §0-NEXT). Lessons #14-#15 are new from W18 Phase A.

14. **Doc-only PRs benefit massively from parallel-reviewer dispatch.** W18 was a "design-only welle, no code" — the surface naive expectation was "write some markdown + merge." The parallel `code-reviewer` + `security-reviewer` dispatch surfaced 16 substantive findings (2 HIGH security, 10 MEDIUM, 4 LOW) including a load-bearing TOCTOU race in the secure-delete protocol design that would have shipped silently to W18b's implementation if not caught. **Lesson:** parent ALWAYS runs the parallel-reviewer dispatch even on doc-only PRs — the cost is one parallel-agent-pair invocation, the value is catching design-time bugs before they hit code.
15. **Library-name vs concept-name conflation can mislead the entire welle.** Master-plan §3 named the layer "Mem0g cache" as a concept-reference; Atlas's W18 design assumed (correctly) it was implementation-neutral but the research subagent's first pass surfaced that Mem0g is a paper-name not a product, which materially changed the framing of the comparative spike. **Lesson:** when an external library or concept name appears in master-plan, the welle that builds on it should explicitly research-verify the current upstream packaging state, not trust the name as a stable artefact. Document the clarification in both spike + ADR so future agents don't re-derive the surprise.

### What's next (Phase 13+, in priority order)

1. **W18b Mem0g implementation** — ADR-Atlas-013 reserved (optional, only if implementation surfaces design amendment). New `crates/atlas-mem0g/` (~1500-2000 LOC). Per ADR §4 sub-decisions; ready-to-dispatch subagent skeleton in `.handoff/v2-beta-welle-18-plan.md`. Atlas-side verification gaps V1-V4 from spike §12 close as TDD-RED tests.
2. **W19 v2.0.0-beta.1 ship** — convergence milestone. Layer 2 ArcadeDB + Layer 3 Mem0g + verifier-rebuild all operational; signed tag + GitHub Release + npm publish (analog V2-α-α.1 ship pattern from W8).
3. **Counsel-Engagement-Kickoff** (parallel, Nelson-led). Per `DECISION-COUNSEL-1` blocks V2-β public materials. 6-8-week clock starts at engagement-letter signature. Counsel review of `embedding_erased` audit-event payload is now in scope per ADR-Atlas-012 §5.4.

### Pre-flight checklist for W18b session

Superseded by **§0-NEXT** above (post-Phase-12.5 master HEAD includes this consolidation PR's merge + W18b entry checklist). The §0-NEXT pre-flight is the canonical command set; this §0z4 placeholder is preserved for cross-reference only.

---

## 0z3. V2-β Phase 11 (W17c ArcadeDB CI + bench) SHIPPED — 2026-05-14 late-day

> **W17c-shipped narrative.** §0-NEXT above is the actionable next-session entry point; read this section if you need the W17c historical detail (what landed, what regressions surfaced, reviewer-dispatch outcome, session lessons). Phase 11 wraps the Layer-2 ArcadeDB integration story end-to-end: driver (W17b) + CI infrastructure that validates it (W17c).

### What landed in PR #92 (`61ef036`)

| File | Status | Brief |
|---|---|---|
| `infra/docker-compose.arcadedb-smoke.yml` | NEW | ArcadeDB 24.10.1 sidecar; `JAVA_OPTS=-D...rootPassword=...` (env-var shape doesn't work — interactive password prompt blocks startup); unauthenticated `/api/v1/ready` healthcheck (no credentials in `docker inspect`); `restart: "no"`; retries=12 + start_period=30s |
| `.github/workflows/atlas-arcadedb-smoke.yml` | NEW | Linux Ubuntu lane; SHA-pinned actions; `permissions: contents: read`; paths-gated; 10 min timeout; compose up → healthcheck wait → cross_backend test → bench → artifact → compose down |
| `crates/atlas-projector/tests/arcadedb_benchmark.rs` | NEW | 3 `#[ignore]`-gated benches: B1 sanity, B2 incremental_upsert p50/p95/p99, B3 sorted_read p50/p95/p99 over 50v/100e |
| `tools/run-arcadedb-smoke-local.sh` | NEW | Bash helper mirroring CI for local dev |
| `crates/atlas-projector/src/backend/arcadedb/cypher.rs` | MODIFY | `upsert_edge_command` params renamed: `$from`/`$to`/`$label` → `$src`/`$dst`/`$lbl`; stored edge property `label` → `edge_label`; `parse_edge_row` translates back. Trait surface unchanged. |
| `crates/atlas-projector/src/backend/arcadedb/mod.rs` | MODIFY | New `schema_initialized: Arc<Mutex<HashSet<String>>>` cache + `ensure_schema_types_exist` method (single atomic Cypher `CREATE ... WITH ... DETACH DELETE` registers Vertex + Edge types via sentinel); called from `begin()` after `ensure_database_exists` |

### Two W17b regressions surfaced + closed atomically in W17c

1. **ArcadeDB Cypher 24.10.1 reserved param names:** `$from` and `$to` (collide with SQL `CREATE EDGE ... FROM ... TO ...` keywords) silently empty result sets; `$label` raises `IllegalArgumentException` (TinkerPop `T.label` token). Diagnosis: extensive curl-against-live-ArcadeDB probing isolated each trigger; renames preserve the public API (only the Cypher placeholder names and the stored ArcadeDB property name change; `BackendEdge::label` and `from_entity_uuid` / `to_entity_uuid` stored properties are unchanged).
2. **ArcadeDB Edge type not auto-registered by MERGE:** `MERGE (a)-[r:Edge]->(b)` silently no-ops if Edge type doesn't yet exist (CREATE auto-registers; MERGE does not). Symptom: edge writes returned 2xx + COMMIT succeeded but zero edges persisted; `canonical_state()` produced vertex-only hash that diverged from InMemory. Fix: single atomic Cypher `CREATE (a)-[r:Edge]->(b) WITH a, b, r DETACH DELETE a, b` statement registers both types and cleans up sentinels in one HTTP roundtrip. Idempotent across per-(backend, db_name) cache.

### W17c reviewer-dispatch outcome (Atlas Standing Protocol lesson #8)

Parallel `code-reviewer` + `security-reviewer`. **0 CRITICAL.**
- **1 HIGH** (schema-bootstrap orphan window with separate CREATE+DELETE HTTP calls) — fixed: single-statement Cypher.
- **4 MEDIUM:** (1) `dtolnay/rust-toolchain` branch-tip SHA — documented; (2) healthcheck cmdline password — fixed via unauthenticated `/ready`; (3) Mutex TOCTOU doc accuracy — fixed; (4) two password env vars — documented as by-design.
- **2 LOW:** (1) missing `restart: "no"` — fixed; (2) missing `set +x` guard — fixed.
- **H-2** (B1 documentation gap) — fixed: B1 explicitly labeled "NOT the authoritative byte-pin gate".

### CI / verification results

- `cross_backend_byte_determinism_pin` reproduces `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` through ArcadeDB live (in CI Linux + local Windows Docker Desktop).
- 119 unit + 18 trait-conformance + 16 other = 153 tests green; clippy `-D warnings` clean.
- Trait surface (`backend/mod.rs` public items) unchanged. SemVer additive.
- Workflow timing: compose up + healthcheck + 2 cargo-test runs + artifact upload + compose down completes in ~75-90 s on Linux runners.

### Bench baseline (Windows Docker Desktop + WSL2, post-fix; CI Linux numbers will differ)

| Bench | n | p50 | p95 | p99 |
|---|---|---|---|---|
| B2 incremental_upsert | 200 | 24.3 ms | 47.7 ms | 56.7 ms |
| B3 sorted_read_vertices_50v | 100 | 10.0 ms | 14.2 ms | 26.1 ms |
| B3 sorted_read_edges_100e | 100 | 16.4 ms | 22.1 ms | 26.0 ms |

V2-α InMemoryBackend baseline ~50 µs/event (B2 reference). ADR-010 §4.10 ArcadeDB estimate 300-500 µs/event. Local Windows Docker Desktop adds substantial HTTP-loopback overhead vs native Linux CI; Linux CI numbers will be archived as artifact and reflect the operational baseline.

### W17c session lessons (load-bearing for W18+)

1. **`#[ignore]`-gated integration tests are blind spots until CI runs them.** W17b's cross_backend_byte_determinism existed and compiled cleanly but never ran against a live backend. W17c's first run surfaced two real driver regressions immediately. **Lesson:** ship the CI infrastructure that runs `#[ignore]`-gated tests alongside the gated tests themselves; don't let "deferred to W17c" become "deferred forever".
2. **ArcadeDB Cypher subset has reserved param names that are not documented.** `$from`, `$to`, `$label` all collide with SQL keywords or TinkerPop tokens and silently break queries that bind them. Future Atlas Cypher work in `cypher.rs` MUST avoid SQL-keyword param names (`$from`, `$to`, `$where`, `$order`, etc.). The W17c reviewer-dispatch did not flag this as a regression risk for future welles — should be added to the W18 Mem0g checklist.
3. **W17c bench shape:** measurement-only tests can be `#[ignore]`-gated with the same env-var contract as the cross-backend test; output goes to stderr via `eprintln!` and is captured by `cargo test -- --nocapture` and tee'd to a workflow artifact. No criterion / no `[[bench]]` overhead. Pattern reusable for W18 Mem0g latency benches.
4. **Atomic-Cypher pattern for schema bootstrap.** `CREATE ... WITH ... DETACH DELETE` in one statement is atomic from the client side AND registers the schema-type as a side effect of the CREATE phase. This pattern works for any ArcadeDB schema-type registration that doesn't have a direct DDL path on fresh databases.

### What's next (Phase 12+, in priority order)

1. **W18 Mem0g Layer-3 cache** — ADR-Atlas-012 reserved. Now unblocked by W17c-validated ArcadeDB stability. Design questions: (a) where does Mem0g cache invalidation hook into the projector pipeline? (b) what's the byte-determinism story for embedding outputs? (c) is the cache hit-rate measurement separate from Atlas+Mem0g end-to-end benchmark or combined?
2. **W19 v2.0.0-beta.1 ship** — convergence milestone. ArcadeDB Layer 2 + Mem0g Layer 3 operational; all V2-β wellen merged; signed tag + GitHub Release + npm publish (analog V2-α-α.1 ship pattern from W8).
3. **Counsel-Engagement-Kickoff** (parallel, Nelson-led). Per `DECISION-COUNSEL-1` blocks V2-β public materials. 6-8-week clock starts at engagement-letter signature.
4. **W17 post-mortem (optional)** — operator-runbook §16 update documenting ArcadeDB Cypher quirks discovered in W17c (reserved param names, MERGE-vs-CREATE for edges) for future Atlas integrations.

### Pre-flight checklist for W18 session

Superseded by **§0-NEXT** above (post-Phase-11.5 master HEAD `8bbc729` + risk-aware W18 entry checklist). The §0-NEXT pre-flight is the canonical command set; this §0z3 placeholder is preserved for cross-reference only.

---

## 0z2. V2-β Phase 10 (W17b ArcadeDB driver) SHIPPED — 2026-05-14 post-Docker-restart

> **Read this first** if you're a fresh agent continuing V2-β work after 2026-05-14. The Docker-restart breakpoint (§0-NOW below) has been resolved — W17b is merged and on master. This section is the operational summary of what landed and what's queued next.

### What landed today (2026-05-14 master timeline)

| Commit | PR | Welle | Brief |
|---|---|---|---|
| `36975af` | #87 | counsel-enablement | RFP-ready 7-SOW counsel scope-doc + README Art. 12 verbatim fix + pin-file sync |
| `44c5102` | #88 | W17a-cleanup | begin() lifetime → `'static`; `check_workspace_id` + `check_value_depth_and_size` boundary helpers; `ProjectorError::InvalidWorkspaceId` variant; ADR-011 §4.3 amendment. 3 of 4 W17a carry-over MEDIUMs resolved at trait surface. |
| `ddfe3d0` | #89 | Phase-10-breakpoint | §0-NOW Docker-restart resume guide added to handoff. |
| `d216844` | #90 | **W17b** | **ArcadeDB driver implementation.** Sub-module split `crates/atlas-projector/src/backend/arcadedb/{mod.rs, client.rs, cypher.rs}` (~1860 LOC). `reqwest 0.12` + `rustls-tls` + `blocking` features added. Cross-backend byte-determinism test `#[ignore]`-gated behind `ATLAS_ARCADEDB_URL`. Parallel `code-reviewer` + `security-reviewer` dispatch (Lesson #8) found 0 CRITICAL + 2 HIGH + 3 MEDIUM + 2 LOW; all fixed in single in-commit fix-commit (`483709a` post-rebase). 153 tests green; clippy `-D warnings` clean; byte-pin `8962c168...e013ac4` reproduced. |
| THIS commit | (this PR) | Phase 10.5 | Consolidation — CHANGELOG + V2-MASTER-PLAN §6 + decisions.md + V2-BETA-ORCHESTRATION-PLAN + V2-BETA-DEPENDENCY-GRAPH + handoff §0z2. |

### What W17b actually closed

- **All 4 W17a carry-over MEDIUMs disposition is final.** #2 (depth+size cap), #3 (WorkspaceId validation), #4 (`begin()` lifetime) all CLOSED at call-sites. #5 (`MalformedEntityUuid` umbrella) V2-γ-DEFERRED per original plan-doc rationale.
- **W17b's own reviewer findings** all closed in-commit: HIGH-1 (`run_command` Value-return latent bypass — narrowed to `()`); HIGH-2 (`format!("create database {db_name}")` admin-command injection — closed via stricter `[a-zA-Z0-9_]` db-name allowlist in `db_name_for_workspace`); MEDIUM-1 (SecretString visibility tightened to `pub(crate)`); MEDIUM-2/3 (`ArcadeDbBackend::new` rejects userinfo URLs + non-http/https schemes); LOW-1 (bounded `ensure_database_exists` body read); 15 clippy `doc_lazy_continuation` lints (13 W17b-new + 2 pre-existing on master, all fixed opportunistically).
- **Trait surface UNCHANGED.** Only one doc-comment paragraph in `backend/mod.rs` was touched (clippy fix). All public items in `GraphStateBackend` / `WorkspaceTxn` / `Vertex` / `Edge` / `UpsertResult` are identical to pre-W17b state. SemVer additive.

### W17b session lessons (load-bearing for W17c+)

1. **Subagent self-audit claim "zero clippy warnings" was incorrect.** 15 lints surfaced under `-D warnings`. Lesson: parent ALWAYS runs `cargo clippy --no-deps -- -D warnings` as part of Step 1 (local verification) before opening a PR; don't trust the subagent's self-audit lint claim.
2. **Strict-required-status-checks-policy + `BEHIND` mergeState.** When master advances during W17b's branch lifetime (PR #89 landed `ddfe3d0` AFTER W17b branched), the W17b PR shows `mergeStateStatus: BEHIND` and admin-merge fails with "2 of 2 required status checks are expected." Fix: local rebase + force-push-with-lease (preserves SSH-signed commits) — Atlas Standing Protocol Lesson #7 applied successfully.
3. **Both required checks (`Verify trust-root-modifying commits` + `atlas-web-playwright`) re-ran cleanly after force-push** and both green within ~4 min combined. The `.handoff/**` change in the PR auto-triggers atlas-web-playwright's path filter (no workaround-touch needed).

### What's next (queued, in priority order)

1. **W17c Docker-Compose CI workflow** — `.github/workflows/atlas-arcadedb-smoke.yml` spins up an ArcadeDB sidecar, sets `ATLAS_ARCADEDB_URL`, runs `cargo test -p atlas-projector --test cross_backend_byte_determinism -- --ignored`. Expected: byte-pin reproduces through ArcadeDB path → completes the cross-backend equivalence story. **Plus benchmark capture** replacing ADR-010 §4.10 estimates with measured numbers; embedded-mode reconsideration trigger at p99 > 15 ms.
2. **W18 Mem0g Layer-3 cache** — ADR-Atlas-012 reserved. Depends on W17c-validated ArcadeDB stability.
3. **W19 v2.0.0-beta.1 ship** — convergence milestone. ArcadeDB-backed Layer 2 operational + all V2-β wellen merged.
4. **Counsel-Engagement-Kickoff** (parallel, Nelson-led) — Nelson selects lead firm from 7-firm matrix in `.handoff/v2-counsel-engagement-scope.md` and sends outreach. 6-8-week clock starts at engagement-letter signature. NOT engineering-pipeline-blocking but pre-V2-β-public-materials gating per `DECISION-COUNSEL-1`.

### Pre-flight checklist for W17c session

```bash
cd /c/Users/nelso/Desktop/atlas
git status                                          # → clean
git checkout master && git pull origin master       # → up-to-date with master HEAD d216844 (or later)
git log --oneline -3                                # → expect:
#   <Phase 10.5 consolidation merge>
#   d216844 feat(v2-beta/welle-17b): ArcadeDB driver implementation (#90)
#   ddfe3d0 docs(v2-beta/phase-10-breakpoint): handoff §0-NOW ... (#89)
"/c/Program Files/GitHub CLI/gh.exe" pr list --state open --json number,title  # → archive PRs only (#59/#61/#62)
/c/Users/nelso/.cargo/bin/cargo.exe test -p atlas-projector --quiet  # → 153 tests green
/c/Users/nelso/.cargo/bin/cargo.exe clippy -p atlas-projector --no-deps -- -D warnings  # → zero warnings
```

Read `.handoff/v2-beta-welle-17b-plan.md` (lives on W17b branch — fetch via `git checkout d216844 -- .handoff/...` if needed, or read commit-level via gh) for full reviewer-dispatch outcome detail.

---

## 0-NOW. 2026-05-14 Docker-Restart Breakpoint — Resume Guide [HISTORICAL — W17b NOW SHIPPED]

> **Status (post-2026-05-14 ~12:08 UTC):** RESOLVED. W17b shipped as PR #90 (`d216844`) per §0z2 above. This section is preserved as the operational record of how the breakpoint was bridged. Future agents: read §0z2 first; this §0-NOW is historical.

> **Read this section first when resuming after Nelson's computer restart on 2026-05-14.** Brings you from cold context to actionable next step in <5 min.

### Where Atlas is right now

**Master HEAD: `44c5102`** (W17a-cleanup PR #88 merge commit). Master is clean, all CI required-checks green.

**Today's merges (2026-05-14, both squash-merged via `gh pr merge --admin --squash --delete-branch`):**
- **PR #87** (`36975af`) — `docs(v2-beta/counsel-enablement): RFP-ready counsel scope + README Art. 12 verbatim fix + pin-file sync`. Ships `.handoff/v2-counsel-engagement-scope.md` (269 lines, RFP-ready, 7 SOWs + 7-firm matrix + DE+EN outreach templates + engagement-letter checklist + verbatim Q-3-1..Q-3-4 from `v2-master-vision-v1.md` §12.1 + parallel-supervisor-engagement note). README.md:28 verbatim Art. 12 fix per DECISION-COMPLIANCE-2. `docs/COMPLIANCE-MAPPING.md` counsel-pending disclaimer header. `tools/expected-master-ruleset.json` synced to live Ruleset state (atlas-web-playwright added as 2nd required-check; no security drift — live was already stricter than pinned).
- **PR #88** (`44c5102`) — `feat(v2-beta/welle-17a-cleanup): trait-surface hardening + boundary helpers ahead of W17b`. Resolves 3 of 4 W17a carry-over MEDIUMs at the trait surface (ADR-Atlas-011 §4.3 sub-decisions #10/#11/#12): `begin()` lifetime `'_` → `'static` (SemVer-additive widening at 18 existing call sites — code-reviewer-corrected count); `pub fn check_workspace_id(s)` boundary helper (rules: non-empty + len≤128 + ASCII + no `/`,`\`,NUL,`\r`,`\n` — CRLF deny added in-commit per parallel security-reviewer MED to close log-injection surface); `pub fn check_value_depth_and_size(v, max_depth, max_bytes)` boundary helper. NEW `ProjectorError::InvalidWorkspaceId { reason: String }` variant (`#[non_exhaustive]` enum addition — SemVer-additive). ADR-Atlas-011 amended with §4.3 + public-API-surface delta + new §9 decision-log row. 19 trait-conformance tests green (8 original + 9 new + 1 CRLF + 1 byte-pin); byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduced unchanged. W17a plan-doc §"Status update (2026-05-14)" block confirms #2/#3/#4 RESOLVED at trait surface (W17b consumes helpers at call-sites; #5 V2-γ-deferred).

**W17b WIP at breakpoint (2026-05-14 ~11:16):** branch `feat/v2-beta/welle-17b-arcadedb-impl` exists on origin at SSH-signed commit `5382d3c2ff297e87af00425a6dd3ff14ea1e0494`. NO PR opened yet. Subagent stopped voluntarily at clippy-clean state (zero warnings, 18/18 trait-conformance green) BEFORE plan-doc finalisation and BEFORE parent-led reviewer dispatch — to give Nelson a clean Docker-restart breakpoint.

**What W17b WIP contains:**
- `crates/atlas-projector/src/backend/arcadedb/{mod.rs, client.rs, cypher.rs}` (NEW sub-module split — replaces W17a stub `arcadedb.rs` single file). 2035 LOC total: `mod.rs` (686) = backend + txn impl + error mapping; `client.rs` (418) = reqwest wrapper + Basic auth + timeouts + JSON parse helpers (calls `check_value_depth_and_size`); `cypher.rs` (674) = parameterised query builders (vertex/edge upsert MERGE templates; sorted-read MATCH templates per §4.9 adapter).
- `crates/atlas-projector/Cargo.toml` (+27 LOC) — adds `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }`.
- `crates/atlas-projector/tests/cross_backend_byte_determinism.rs` (NEW, 257 LOC) — `#[ignore]`-gated test (runs only with `ATLAS_ARCADEDB_URL` env var set; W17c wires Docker-Compose CI). Same 3-node + 2-edge fixture as `byte_pin_through_in_memory_backend`; asserts byte-identical canonical-state hex through both backends.
- `crates/atlas-projector/tests/backend_trait_conformance.rs` — DROPS the `should_panic` stub tests (no longer applicable).
- `crates/atlas-projector/src/lib.rs` — re-export path update for sub-module.
- `Cargo.lock` (+3 LOC) — reqwest transitive deps auto-resolved.
- `.handoff/v2-beta-welle-17b-plan.md` (114 lines) — full plan-doc with "What was DONE / What is NOT YET DONE / Resume-from-breakpoint guide" sections.

**W17b WIP verification state (verified on parent worktree post-stop, before push):**
- `cargo check -p atlas-projector` → clean
- `cargo test -p atlas-projector --test backend_trait_conformance` → 18/18 green (post-WIP, after dropping stub-panic tests + adding cross-backend test)
- Subagent self-reported zero clippy warnings on the new arcadedb code

### What to do NEXT session (entry-point)

```bash
cd /c/Users/nelso/Desktop/atlas
git fetch origin                              # see breakpoint branch
git status                                    # should be clean on master
git log --oneline -3                          # expect:
#   <Phase 10 breakpoint handoff PR merge sha>
#   44c5102 feat(v2-beta/welle-17a-cleanup): ...  (PR #88)
#   36975af docs(v2-beta/counsel-enablement): ... (PR #87)

# Verify W17b WIP branch is on origin
git ls-remote origin feat/v2-beta/welle-17b-arcadedb-impl
# expect: 5382d3c2ff297e87af00425a6dd3ff14ea1e0494

# Check out the W17b WIP for review
git checkout feat/v2-beta/welle-17b-arcadedb-impl

# Verify clippy-clean still holds
/c/Users/nelso/.cargo/bin/cargo.exe check -p atlas-projector
/c/Users/nelso/.cargo/bin/cargo.exe test -p atlas-projector --test backend_trait_conformance --quiet
# expect: 18/18 green; no warnings
```

**Then execute Phase 2 of the [now-completed] sprightly-yawning-pelican plan:**

1. **Open draft PR #89** (if not already opened) for W17b WIP — base=master, head=`feat/v2-beta/welle-17b-arcadedb-impl`. PR body should reference `.handoff/v2-beta-welle-17b-plan.md` "Resume-from-breakpoint guide" + acceptance criteria + the carry-over MEDIUM resolution status.

2. **Parent-led parallel reviewer dispatch** per Atlas Standing Protocol lesson #8 (single message, 2 Agent calls):
   - `code-reviewer` agent: focus areas in `.handoff/v2-beta-welle-17b-plan.md` §"Reviewer focus suggestions" (lifetime correctness, Cypher template parameterisation, error-path mapping, `check_workspace_id` placement first-in-`begin()`, `check_value_depth_and_size` at every HTTP-response → BTreeMap boundary).
   - `security-reviewer` agent: credential redaction (grep for `password`/`token`/`auth` echo in error strings), tenant isolation, Cypher-injection paths (parameter binding correctness), panic-path audit, Basic-auth credentials never logged.

3. **Fix CRITICAL/HIGH/applicable-MEDIUMs in-commit** per reviewer dispatch outcome. Atlas Standing Protocol: parallel review → fix in-commit → single SSH-signed commit (or commit-series squashed cleanly).

4. **Admin-merge PR #89** via `gh pr merge --squash --admin --delete-branch`. CI gates: `verify-trust-root-modifying-commits` + `atlas-web-playwright` (the latter requires `.handoff/**` change in the PR to trigger via path filter — the plan-doc covers that).

5. **Phase 10.5 consolidation PR (parent-led, separate)** post-W17b-merge: updates `.handoff/v2-session-handoff.md` (refresh §0-NOW → §0z2 V2-β-Phase-10-SHIPPED narrative; mark 4 of 4 W17a carry-over MEDIUMs handled), `CHANGELOG.md` `[Unreleased]`, `docs/V2-MASTER-PLAN.md` §6 Welle-17 status row, `.handoff/decisions.md` Welle-17 closure rows, `docs/V2-BETA-ORCHESTRATION-PLAN.md` Welle-17 status flip.

### Important context for the resume

**Today's worktree-isolation incidents (Atlas handoff lesson #1 recurring):**
- The Phase 1c subagent (counsel-enablement docs) violated worktree-isolation: instead of writing in its agent-worktree, it checked-out `docs/v2-beta/counsel-enablement` in the PARENT worktree AND ran `git checkout -- crates/` against the parent worktree's in-progress Phase-1b engineering edits ("treated as unrelated parallel-agent modifications"). Parent had to redo all Phase-1b edits.
- Mitigation for W17b dispatch: the dispatch prompt for the W17b subagent included extra-stringent pre-flight enforcement (3 mandatory git commands as first 3 actions; verify origin/master HEAD `44c5102` matches; explicit prohibition of `git checkout -- crates/` or `git reset --hard` on parent-worktree-files). The W17b subagent honoured isolation correctly this time (its commits stayed on its agent-worktree branch).

**Today's merge-gate experience:**
- Atlas Ruleset 15986324 has `strict_required_status_checks_policy: true` + 2 required checks: `Verify trust-root-modifying commits` (always triggers on PRs) + `atlas-web-playwright` (path-filter excludes pure `crates/atlas-projector/` PRs).
- Workaround for crates-only PRs: add a `.handoff/**` file touch (e.g. updating the W17a plan-doc with a status note) — that path is in the workflow's path-filter (per V1.19 Welle 12 fix, allowing doc-only PRs to satisfy the required check).
- `gh pr update-branch` produces a github-bot-signed merge commit that fails `verify-trust-root-modifying-commits` (the bot key is not in `.github/allowed_signers`). Fix per Atlas handoff §0z lesson #7: rebase locally onto fresh master (preserves SSH-signed commits), `git push --force-with-lease`.

**Outstanding Nelson-actions (not agent-dispatchable):**
1. **`RULESET_VERIFY_TOKEN` PAT configuration** per `docs/OPERATOR-RUNBOOK.md` §16 — fine-grained PAT with "Repository administration: read" scope, set as repo secret. Without it, `verify-branch-protection.yml` keeps firing red (exit 2: "PAT scope insufficient — live Ruleset response missing 'bypass_actors' field"). Does NOT block merges; just means the meta-verifier-of-ruleset-state can't see the `bypass_actors` field to confirm `[]`.
2. **Counsel-engagement outreach kickoff** with `.handoff/v2-counsel-engagement-scope.md` as RFP basis. Select lead firm from 7-firm matrix (Hogan Lovells Frankfurt / Bird & Bird Munich / Hengeler Mueller / Matheson / William Fry / Cleary Gottlieb Paris / Taylor Wessing) — or several in parallel for comparative quotes. DE template for German firms, EN template for IE/FR/UK. Engagement-letter signature starts 6-8-week clock for GDPR Art. 4(1) hash-as-PII Path-B opinion (now non-reversible-without-migration-pain since V2-α schema is committed).

---

## 0. Fresh-Context Onboarding (read THIS FIRST if you're a new session)

**Wer bist du, wo bist du, was tust du?**

- **Repo:** `C:/Users/nelso/Desktop/atlas` (Windows-Host, bash/MSYS verfügbar, `cargo` lebt unter `/c/Users/nelso/.cargo/bin/cargo.exe`, `gh` lebt unter `/c/Program Files/GitHub CLI/gh.exe` — beide NICHT im default PATH).
- **State:** Atlas v2.0.0-alpha.2 ist LIVE auf master + GitHub + npm `@atlas-trust/verify-wasm@2.0.0-alpha.2` seit 2026-05-13 (Sigstore Build L3 provenance preserved; W11 wasm-publish race fix validated end-to-end). V1 abgeschlossen (v1.0.1 LIVE 2026-05-12). V2-Strategie 4-phasig komplettiert. V2-α 8-Welles SHIPPED 2026-05-12 → 2026-05-13. **V2-β Phase 0 + 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 8.5 + 9 + 9.5 ALL SHIPPED 2026-05-13** in 18 PRs (#67-#85, plus the Phase-9.5 consolidation PR that merges this commit). Key landings: PR #71 orchestration framework; PRs #72/#73/#74 W9-W11 docs+workflow parallel batch (v2.0.0-alpha.2 candidate); PR #76 v2.0.0-alpha.2 release; PRs #77/#78/#79 W12-W14 code parallel batch; PR #81 W15 Cypher-validator consolidation into `packages/atlas-cypher-validator/`; PR #83 W16 ArcadeDB spike + ADR-Atlas-010 (locks W17 architectural decisions); **PR #85 W17a `GraphStateBackend` trait + `InMemoryBackend` + `ArcadeDbBackend` stub + ADR-Atlas-011 (the Layer-2 abstraction boundary that lets W17b wire the ArcadeDB driver behind a one-trait-impl swap).** **Nächste Implementierungsarbeit = V2-β Phase 10 = W17b ArcadeDB driver implementation.** Pre-flight reading mandatory: (1) `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` (W17a's trait shape + the binding OQ-1/OQ-2 resolutions: `Box<dyn WorkspaceTxn>` for object safety + `batch_upsert` vertices-before-edges); (2) `crates/atlas-projector/src/backend/{mod.rs,in_memory.rs,arcadedb.rs}` (production trait surface + the stub W17b fills); (3) `.handoff/v2-beta-welle-17a-plan.md` "Post-merge: reviewer findings deferred to W17b" section (4 carry-over MEDIUMs W17b MUST address: serde_json depth cap on HTTP responses, WorkspaceId validation guard at `begin()`, `begin()` lifetime `'_` vs `'static` decision BEFORE first method body, error-enum variant naming defer-to-V2-γ); (4) `docs/ADR/ADR-Atlas-010-...md` §4 sub-decisions 1-8 (binding); (5) `crates/atlas-projector/tests/backend_trait_conformance.rs` (existing tests W17b extends with cross-backend byte-determinism); (6) `.handoff/v2-beta-welle-N-plan.md.template` for plan-doc skeleton.
- **Methodik:** 4-Phasen-Framework aus `.handoff/v2-iteration-framework.md` (jetzt auch dokumentiert als reusable pattern in `docs/WORKING-METHODOLOGY.md`). Phase 1 = parallel-foundation-docs. Phase 2 = parallel-multi-angle-crits. Phase 3 = semi-manual synthesis. Phase 4 = master-plan + working-methodology landen auf master.
- **Was bereits passiert ist (Phase 1+2+3+4 alle 2026-05-12):**
  - **Phase 1** — 5 Foundation Docs parallel von 5 Subagents in eigenen Worktrees geschrieben (~2811 Zeilen). Auf PR #59 (draft, no-merge — work-product).
  - **Phase 2** — 6 Multi-Angle Crits parallel von 6 Subagents in eigenen Worktrees geschrieben (~1299 Zeilen). Auf PR #61 (draft, no-merge, base=PR-59-branch — work-product).
  - **Phase 3** — Master Vision v1 (~615 Zeilen) + decisions.md (22 Entscheidungen, ~284 Zeilen) durch semi-manual synthesis erstellt. Auf PR #62 (draft, no-merge, base=PR-61-branch — work-product).
  - **Phase 4** — `docs/V2-MASTER-PLAN.md` (~300 Zeilen) + `docs/WORKING-METHODOLOGY.md` (~200 Zeilen) auf master via PR #60 gemerged. Plus `.handoff/v2-master-vision-v1.md` + `.handoff/decisions.md` mitgemerged für master-reference-ability.
- **Was als nächstes ansteht (V2-α + parallel-Counsel-Track):**
  - **V2-β Phase 9 SHIPPED 2026-05-13** = W17a ArcadeDB driver scaffold + `GraphStateBackend` trait + ADR-Atlas-011 (PR #85). Production trait surface + `InMemoryBackend` impl + `ArcadeDbBackend` stub, all SSH-Ed25519 signed via fix-commit `08167fc` on top of subagent's `4bac0b3`, squash-merged as commit `dec6234`. Parallel external code-reviewer + security-reviewer both APPROVED (0 CRITICAL / 0 HIGH); 1 MEDIUM applied in-commit (`#[doc(hidden)]` on `snapshot()`), 4 MEDIUMs documented as W17b/V2-γ carry-overs in `.handoff/v2-beta-welle-17a-plan.md`. Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduced through trait surface.
  - **V2-β Phase 10 next** = **W17b ArcadeDB driver implementation.** SERIAL `general-purpose` subagent. Fills the `ArcadeDbBackend` stub in `crates/atlas-projector/src/backend/arcadedb.rs` with `reqwest`-based HTTP calls to ArcadeDB's `/api/v1/begin/{db}` + `/api/v1/command/{db}` + `/api/v1/commit/{db}` endpoints per ADR-Atlas-010 §4 sub-decisions 1-8. Adds `reqwest` with `rustls-tls` feature to `crates/atlas-projector/Cargo.toml` (~2 MB binary cost, server-mode constraint). MUST apply the 4 carry-over MEDIUMs from W17a plan-doc: (a) `serde_json::Value` depth + size cap before `Vertex::properties` / `Edge::properties` deserialisation; (b) `WorkspaceId` validation guard at `ArcadeDbBackend::begin()` rejecting empty / enforcing UUID-or-equivalent format BEFORE HTTP request construction; (c) evaluate `begin()` lifetime `'_` vs `'static` BEFORE first method body (SemVer-breaking trait change risk if delayed); (d) ProjectorError variant naming defer-to-V2-γ acceptable. Cross-backend byte-determinism test `tests/cross_backend_byte_determinism.rs` MUST pass (`InMemoryBackend::canonical_state()` byte-identical to `ArcadeDbBackend::canonical_state()` for same input `events.jsonl`). ~2-3 sessions.
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

## 0z. V2-β-α.2 + V2-β Phase 0-9.5 SHIPPED narrative (2026-05-13 work-day)

> **Read this first** if you're a fresh agent continuing the V2-β work. Everything below §0z is historical V2-α / strategic-iteration context. §0z captures the most recent operational state + the W17b ready-to-dispatch prompt skeleton.

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
| `eb16631` | #84 | V2-β Phase 8.5 | Phase-8 consolidation commit + bulletproof W17a handoff. CHANGELOG + master-plan §6 + orchestration-plan + handoff §0z + dependency-graph + W17a entry-criteria. |
| `dec6234` | #85 | V2-β Welle 17a | **GraphStateBackend trait + InMemoryBackend + ArcadeDbBackend stub + ADR-Atlas-011 + 8 trait-conformance tests + plan-doc.** ~2177 LOC across 9 files. Parallel external code-reviewer + security-reviewer both APPROVE (0 CRITICAL / 0 HIGH; 1 MEDIUM in-commit-fix `#[doc(hidden)]` on snapshot, 4 MEDIUMs documented as W17b/V2-γ carry-overs in plan-doc). Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduced through trait surface (new `byte_pin_through_in_memory_backend` test). OQ-1 resolved: `Box<dyn WorkspaceTxn>` over associated type. OQ-2 resolved: `batch_upsert` with vertices-before-edges. 2 SSH-Ed25519 signed commits squashed (subagent `4bac0b3` + fix-commit `08167fc`). |

**Day total:** 18 PRs merged (#67-#85, plus the Phase-9.5 consolidation PR that merges this commit), 1 GitHub Release (v2.0.0-alpha.2), 1 npm publish, 10 ADRs total in repo (5 V2-β ADRs added: 007/008/009/010/011). 32+ reviewer-agent dispatches (per-welle + cross-batch consistency + W17a parent-dispatched external reviewers after subagent self-audit). ~16 reviewer-driven fix-commits applied in-commit before merge. **0 CRITICAL findings missed. 0 byte-determinism CI pin drifts.** All 7 V2-α byte-determinism pins byte-identical end-to-end through both the legacy test path AND the new W17a trait-conformance test.

### Session lessons learned (load-bearing for future welles)

1. **Worktree-isolation leaks are real and recurring.** `Agent` tool with `isolation: "worktree"` forks worktrees from master, but subagents that don't explicitly `git checkout` their target branch can end up writing to the main worktree directory. Affected W9, W11, W14 this session (W14 didn't even commit — parent had to finish the welle). **Lesson:** subagent dispatch prompts MUST include explicit `git fetch origin && git checkout -B feat/v2-beta/welle-<N>-<name> origin/master` as FIRST 3 actions. Parent verifies pre-flight succeeded before assuming agent worked correctly.

2. **When reviewers disagree on whether code is broken, RUN the code.** On W12 PR #79, a `code-reviewer` agent reported 2 CRITICAL findings (template-literal regex broken). The `security-reviewer` behaviorally tested the same validator and reported it works. Parent ran a `node` REPL test of the EXACT regex pattern with all 8 forbidden keywords — all correctly REJECTED. The "CRITICAL" was a theoretical misreading of JS template-literal escape semantics. **No fix applied — validator works as designed.** This pattern was recorded in the Phase 5 consolidation commit and is the canonical example for future reviewer-conflict resolution.

3. **Reviewer-driven fix-commits are non-optional even for "approved" PRs.** W15's initial commit had `package.json main/types/exports` pointing to source (not dist). Code-reviewer flagged as MEDIUM ("convention divergence from `packages/atlas-bridge/`"). Parent deferred — got bitten when Next.js production build failed in CI (`Module not found: ./validator.js`). Hotfix #1 added tsc build step. Then wave3-smoke + sigstore-rekor-nightly workflows ALSO needed the build step propagated. Hotfix #2 covered those. **Lesson:** MEDIUM findings about package conventions should be applied in-commit, not deferred.

4. **Cross-batch consistency-reviewer is a NEW V2-β invariant (per Orchestration Plan §3.5) and earns its dispatch.** Phase 1's cross-batch reviewer caught zero CRITICAL but 1 LOW (W9 §-numbering gap from W9-fix-commit). Phase 4's cross-batch reviewer caught 4 HIGH cross-welle inconsistencies (validator length cap divergence, passport `ok` field flip, agent_did echo cap, workspace vs workspace_id naming). Three of four fix-forward applied in-commit; the workspace-naming convention is HTTP-vs-MCP per-package preserved and documented for W15 (which then carried it through into ADR-Atlas-009 explicitly).

5. **Architect subagent type has Read/Grep/Glob ONLY.** Cannot Write files, cannot Run git commands. W16's architect produced ~700+ lines of inline doc-content for the parent to write. **Lesson:** for code-producing welles use `general-purpose` subagent type (which has full tool surface). For DOC-only spike-style welles, `architect` produces the content; parent writes the files.

6. **Auto-mode classifier blocks `gh pr merge --admin` by default.** Today required Nelson approval mid-session, then a project-local `.claude/settings.local.json` permission rule for `Bash(gh pr merge:*)` + `Bash(git push:*)` to allow unattended admin-merges. **Lesson:** settings.local.json now persists this — next session can use admin-merge directly. Atlas standing pattern (per `~/.claude/rules/common/git-workflow.md`) is `gh pr merge --squash --admin --delete-branch`.

7. **strict_required_status_checks_policy + trust-root-verify interaction.** When `gh pr update-branch` creates a GitHub-generated merge commit (signed by GitHub's RSA key, not the Atlas SSH-Ed25519 allowed signer), trust-root-verify FAILS for PRs that touch trust-root files (.github/workflows/wasm-publish.yml in W11's case). **Fix pattern:** rebase the welle branch locally onto fresh master (preserves SSH-signed commit), force-push. W17b will likely hit this if W17b touches anything in `.github/workflows/`.

8. **Subagent's `Agent` tool may not have access to dispatch reviewer subagents — parent MUST dispatch externals post-implementation.** W17a's subagent reported "parallel `code-reviewer` + `security-reviewer` Agent dispatch was not possible in this environment" and only performed self-audit. Parent dispatched both reviewers post-implementation (in parallel, single message), both APPROVED with 4 W17b-carry-over MEDIUMs documented, 1 in-commit fix applied. **Lesson:** parent's W17 dispatch prompt MUST treat the subagent's self-audit as a "best-effort" pre-check, and parent ALWAYS runs the external code-reviewer + security-reviewer dispatch post-implementation as a non-optional Standing Protocol step. Don't assume the subagent succeeded at this — verify by parent-dispatch every time.

9. **Branch protection rules block admin-merge while required CI checks are in-progress (even with admin override).** Atlas-web-playwright is a required check; even though W17a had zero atlas-web changes, the rule fired. **Resolution:** `gh run watch <run-id> --exit-status` in background lets parent agent wait for completion without polling. Once the run was green, `gh pr merge 85 --squash --admin --delete-branch` succeeded (the post-merge local cleanup `gh` does fails on Windows multi-worktree setups — error is cosmetic, merge already went through; verify with `gh pr view <n> --json state`).

### W17b ready-to-dispatch subagent prompt skeleton

```
Atlas project at C:\Users\nelso\Desktop\atlas. V2-β Welle 17b — ArcadeDB driver implementation. Fills the W17a-shipped ArcadeDbBackend stub with reqwest-based HTTP calls.

## Pre-flight (FIRST 3 actions — non-negotiable)
1. `git fetch origin`
2. `git checkout -B feat/v2-beta/welle-17b-arcadedb-impl origin/master` (master HEAD at dispatch: <current-master-sha-post-W17a-merge>)
3. `git status` → clean

## Pre-flight reading (master-resident, mandatory)
1. `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` (W17a's trait shape + the binding OQ-1/OQ-2 resolutions: Box<dyn WorkspaceTxn> for object safety + batch_upsert vertices-before-edges + open questions OQ-7..OQ-11 for W17b/V2-γ tracking)
2. `docs/ADR/ADR-Atlas-010-arcadedb-backend-choice-and-embedded-mode-tradeoff.md` §4 (8 binding sub-decisions — esp. #5 transaction model, #6 byte-determinism adapter ORDER BY entity_uuid/edge_id, #7 layered tenant isolation)
3. `.handoff/v2-beta-welle-17a-plan.md` "Post-merge: reviewer findings deferred to W17b" section — the 4 carry-over MEDIUMs W17b MUST address (serde_json depth cap, WorkspaceId validation, begin() lifetime evaluation, error-enum cleanup is V2-γ-deferred)
4. `crates/atlas-projector/src/backend/mod.rs` (production GraphStateBackend trait + WorkspaceTxn trait + Vertex/Edge/UpsertResult — the surface you implement)
5. `crates/atlas-projector/src/backend/in_memory.rs` (reference impl — your ArcadeDbBackend must produce byte-identical canonical_state() output for the same workspace contents)
6. `crates/atlas-projector/src/backend/arcadedb.rs` (the stub you fill — unimplemented!() bodies show the contract)
7. `crates/atlas-projector/tests/backend_trait_conformance.rs` (existing 8 tests; W17b extends with cross-backend byte-determinism test)
8. `docs/V2-BETA-ARCADEDB-SPIKE.md` §3 (ArcadeDB primer — Cypher subset, HTTP API endpoints), §8 (CI strategy — Docker-Compose sketch W17c implements)
9. `.handoff/v2-beta-welle-N-plan.md.template` (plan-doc skeleton — you write `.handoff/v2-beta-welle-17b-plan.md`)

## In-scope files (write/modify these only — plus .handoff/v2-beta-welle-17b-plan.md)
- MODIFY `crates/atlas-projector/src/backend/arcadedb.rs` — replace all `unimplemented!()` bodies with `reqwest`-based HTTP calls per ADR-Atlas-010 §4 sub-decisions. ~600-900 LOC. INCLUDE: HTTP client with rustls-tls + Basic auth (OQ-5 starts with HTTP Basic; JWT is V2-γ); per-workspace database management (create + connect on demand per sub-decision #4); transaction handle wrapping `/api/v1/begin/{db}` session; Cypher query builders for vertex/edge upsert + sorted reads (ORDER BY entity_uuid/edge_id per sub-decision #6); commit/rollback via `/api/v1/commit/{db}` + `/api/v1/rollback/{db}`; canonical_state() override calling vertices_sorted + edges_sorted + delegating to V2-α canonical::graph_state_hash for byte-identical output.
- MODIFY `crates/atlas-projector/Cargo.toml` — add `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }` (~2 MB binary cost; tokio-aligned async). Adds 1 dep; no other dep changes.
- MODIFY `crates/atlas-projector/src/backend/mod.rs` IF necessary for the 4 W17a carry-over MEDIUMs — apply BEFORE first method body in arcadedb.rs lands so SemVer-breaking changes don't bite mid-implementation: (a) evaluate begin() lifetime — switch `'_` → `'static` OR explicit named lifetime if needed; (b) consider depth+size cap helper for serde_json::Value deserialisation from HTTP (could live in arcadedb.rs as a private fn).
- NEW `crates/atlas-projector/src/backend/arcadedb/` (sub-module dir, optional) — break up arcadedb.rs into client.rs, cypher.rs, transaction.rs sub-modules if size exceeds ~800 LOC.
- NEW `crates/atlas-projector/tests/cross_backend_byte_determinism.rs` — projects same `events.jsonl` fixture through BOTH InMemoryBackend AND ArcadeDbBackend, asserts byte-identical canonical_state() output. MUST run against a real ArcadeDB instance (gated behind env var, e.g. `ATLAS_ARCADEDB_URL` — skipped in local cargo test unless env set; W17c wires Docker-Compose to set it in CI).
- NEW `.handoff/v2-beta-welle-17b-plan.md` (use template; carry forward W17a's "open questions for W17c" section)

## Forbidden files (parent consolidates in Phase 10.5)
- CHANGELOG.md, docs/V2-MASTER-PLAN.md (status), docs/SEMVER-AUDIT-V1.0.md, .handoff/decisions.md, .handoff/v2-session-handoff.md, docs/V2-BETA-ORCHESTRATION-PLAN.md
- .github/workflows/* (W17c's job — DON'T add atlas-arcadedb-smoke.yml or Docker-Compose CI in W17b; that's a separate welle to keep blast radius small)

## Acceptance criteria (parent verifies all before approving merge)
- `cargo check --workspace` green
- `cargo test -p atlas-trust-core -p atlas-projector` green — including 169 trust-core unit + 8 trait-conformance unchanged (byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` still reproduced through trait)
- `cargo test -p atlas-projector --test cross_backend_byte_determinism -- --ignored` green AGAINST A LIVE ArcadeDB instance (W17b dispatcher provides instance URL or notes a follow-up validation step)
- 4 W17a carry-over MEDIUMs all addressed: serde_json depth cap applied at HTTP-response boundary; WorkspaceId validation guard at ArcadeDbBackend::begin() rejecting empty / enforcing format; begin() lifetime evaluated + decided BEFORE method bodies land (with note in ADR if signature changes); error-enum cleanup defer-to-V2-γ acceptable.
- ArcadeDbBackend NO LONGER calls `unimplemented!()` in any method
- Parent dispatches parallel `code-reviewer` + `security-reviewer` post-implementation (subagent's self-audit insufficient per W17a lesson #8). Fix CRITICAL/HIGH in-commit. Single SSH-Ed25519 signed commit (squash-merge will collapse subagent-commit + parent-fix-commit). DRAFT PR base=master.

## Reviewer focus (when parent dispatches them)
- code-reviewer: reqwest async lifecycle correctness, Cypher query construction (no string-concat injection paths even for non-user-supplied input — use parameterised queries throughout per ADR-010 #6), error-mapping from reqwest errors to ProjectorError variants without leaking internal HTTP detail, byte-determinism preservation (canonical_state() output exact-match InMemoryBackend), no public-API breakage on the trait surface.
- security-reviewer: tenant isolation (per-database-per-workspace operator runbook requirement enforced at ArcadeDbBackend::begin(); no cross-tenant query paths), serde_json::Value depth attack surface (W17a carry-over MED #1) addressed at HTTP-response boundary, WorkspaceId validation (W17a carry-over MED #2) at the trait boundary closes the W17a-flagged gap, no panic paths reachable from public API via HTTP error mapping, Cypher parameter binding ensures workspace_id-as-Cypher-param cannot be sub'd to leak data, Basic-Auth credentials redacted from any log output (OQ-5).

## Output (under 350 words)
PR number + URL, line counts per new/modified file (totals only), test count, cross-backend byte-determinism evidence (hex from both backends), 4 W17a carry-over MEDIUMs resolution status each, reviewer-finding counts + resolutions, any unexpected deviations (e.g. ArcadeDB Cypher subset surprise).
```

### Pre-flight checklist for next session (any agent)

```bash
cd C:/Users/nelso/Desktop/atlas
git status                          # → clean
git checkout master && git pull origin master   # → up-to-date with master HEAD
git log --oneline -3                # → top is Phase-9.5 consolidation commit, then dec6234 W17a feat, then eb16631 Phase-8.5 consolidation
"/c/Program Files/GitHub CLI/gh.exe" pr list --state open --json number,title  # → ~12 ancient drafts (#59-#62 etc.); zero NEW V2-β open
"/c/Program Files/GitHub CLI/gh.exe" release view v2.0.0-alpha.2  # → confirms prerelease LIVE
curl -s "https://registry.npmjs.org/@atlas-trust/verify-wasm" | python -c "import json,sys; d=json.load(sys.stdin); print('latest:', d['dist-tags'].get('latest'))"  # → "2.0.0-alpha.2"
git verify-tag v2.0.0-alpha.2       # → Good ed25519 sig
/c/Users/nelso/.cargo/bin/cargo.exe test -p atlas-trust-core -p atlas-projector --quiet  # → 169 trust-core + 88 projector unit + 8 trait-conformance + integration binaries all pass (byte-determinism CI pin intact through trait surface)
```

### Critical files for V2-β Phase 10+ reference (read-only)

- `docs/V2-BETA-ORCHESTRATION-PLAN.md` §2 (W17b row) + §3 (dispatch architecture + forbidden-files rule)
- `docs/V2-BETA-DEPENDENCY-GRAPH.md` §5 (critical path: Phase 10 unblocks Phase 11 unblocks Phase 12)
- `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` (W17a's trait shape; W17b implements the contract)
- `docs/ADR/ADR-Atlas-010-arcadedb-backend-choice-and-embedded-mode-tradeoff.md` §4 (8 binding sub-decisions for W17b implementation)
- `crates/atlas-projector/src/backend/` (W17a's mod.rs trait + in_memory.rs reference impl + arcadedb.rs stub W17b fills)
- `crates/atlas-projector/tests/backend_trait_conformance.rs` (8 existing tests; W17b extends with cross-backend byte-determinism)
- `.handoff/v2-beta-welle-17a-plan.md` "Post-merge: reviewer findings deferred to W17b" section (the 4 carry-over MEDIUMs W17b MUST address)
- `docs/V2-BETA-ARCADEDB-SPIKE.md` §3 (ArcadeDB Cypher subset + HTTP API primer), §8 (W17c CI sketch — W17b reads as forward-context)

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
