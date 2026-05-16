# Next-Session Bootstrap — Atlas product-track W20b-2 SHIPPED

> **Status:** Session-end 2026-05-16 evening Berlin. **W20b-2 SHIPPED** as `ba4e27f` on master via PR #113 (admin-squash). Branch `feat/product/welle-20b-2-first-run-wizard` deleted (local + remote). All 3 required CI checks green. Atlas product-track is in a **clean ship-state** — no in-flight branches, no half-finished commits.
>
> **Use:** copy-paste as opening prompt next session. Supersedes `.handoff/next-session-bootstrap-w20b-2-in-flight.md` (delete that file when convenient — see §7 cleanup).

---

═══════════════════════════════════════════════════════════════════════════════
0. TL;DR — 60 seconds
═══════════════════════════════════════════════════════════════════════════════

**Was diese session geleistet hat (2026-05-16 nachmittags + abends):**

- ✓ Nelson W20b-1 manuell validiert (alle 5 tiers)
- ✓ W20b-2 implementer subagent dispatched + completed (`70ead19`)
- ✓ 3 parallel reviewers (code + security + tdd) — Approve with 2 CRITICAL + 3 HIGH + 9 MEDIUM + 8 LOW findings
- ✓ Fix-commit subagent dispatched + completed (`8dc0ec5`) — all CRITICAL/HIGH/MEDIUM addressed
- ✓ Regression-fix subagent dispatched + completed (`551d16e`) — cold-goto e2e tests seeded (per `wondrous-rolling-hearth.md` plan)
- ✓ PR #113 opened, CI green (verify-trust-root + atlas-web-playwright 4m24s + mem0g-smoke-required), admin-merged
- ✓ Master ist jetzt `ba4e27f`, W20b-2 branch deleted local + remote, worktree pruned

**Master state JETZT:**
- `master` @ `ba4e27f` (`feat(product/welle-20b-2): atlas-web first-run workspace-creation wizard + bundled fixes (#113)`)
- 0 in-flight product branches
- Open PRs: nur #108 (Phase D engineering parallel, unchanged) + 3 archive (#59/#61/#62)

**Next-Action Candidates** (Nelson chooses):

| Option | Effort | Description |
|---|---|---|
| **A — W20c planning** | medium | Settings UI + Layer 3 honest status + orphan-workspace UX from W20b-2 security Q. Natural product continuation. |
| **B — PR #108 fix-commit** | medium | Engineering parallel (Layer 3 LanceDB ANN/search fill-in). Brief pre-staged in `.handoff/pr-108-fix-commit-subagent-brief.md`. Could ship alongside W20c. |
| **C — `.handoff/` + Phase 14.8 consolidation** | small-medium | Delete 4 superseded bootstrap docs, codify Lessons #25/#26/#27, write 3 environmental issues (LOWs from this session) into GitHub issues, optionally pop `stash@{0}` for Lessons #20-#24 doc consolidation. |
| **D — Environmental issue triage** | small | File 2 GitHub issues from subagent's flagged LOWs (playwright port-3001 collision + wasm artifact bootstrap for worktrees). Could roll into Option C. |

**Recommend:** Start with Option C (small cleanup; gives a clean slate for W20c) → then Option A (W20c planning) → then optionally dispatch Option B in parallel while W20c implementer runs.

═══════════════════════════════════════════════════════════════════════════════
1. PRE-FLIGHT (mandatory first 5 actions)
═══════════════════════════════════════════════════════════════════════════════

```bash
cd /c/Users/nelso/Desktop/atlas

# Master state — should be ba4e27f, clean (except cosmetic next-env.d.ts drift)
git status
git log --oneline -3   # → ba4e27f W20b-2, 0460f5a W20b-1, 6978286 W20a

# Confirm no W20b-2 leftovers
git branch --list "feat/product/welle-20b-2*"   # → empty
git worktree list | grep welle-20b-2            # → empty

# Open PRs unchanged
"/c/Program Files/GitHub CLI/gh.exe" pr list --state open --json number,title  # → #108 + 3 archive

# Stash + handoff state
git stash list                                  # → stash@{0} session-end-2026-05-15 (Lessons #20-#24); still preserved
ls .handoff/                                    # → 6 untracked .md files (see §7)
```

**Wenn dev server für manuelle Validierung benötigt:**

```bash
cd /c/Users/nelso/Desktop/atlas/apps/atlas-web
ATLAS_DEV_MASTER_SEED=1 pnpm dev    # → http://localhost:3000
```

═══════════════════════════════════════════════════════════════════════════════
2. WHAT GOT SHIPPED IN W20b-2 (for reference)
═══════════════════════════════════════════════════════════════════════════════

**Squashed commit `ba4e27f`** — 17 files, +1622/-127:

- `POST /api/atlas/workspaces` — Zod-strict-validated, 409 on dup, 413 on oversize, 500 redacted, `redactPaths` on all 5xx
- `WorkspaceContext.createWorkspace` + pure helper `requestCreateWorkspace`
- `FirstRunWizard` rendered when `workspaces.length === 0` (via new client wrapper `HomeContent`)
- `+ New` button in `WorkspaceSelector` → native `<dialog>` modal with shared `CreateWorkspaceForm`
- Bundled fix: EarlyTier node-id column (W20b-1 spec gap — `ev.payload.node.id` was unrendered)
- Bundled fix: cold-goto e2e tests seeded via shared `provisionAndSelect` helper (extracted to `fixtures.ts`)
- Bundled fix: a11y home-page test split into dashboard-state + wizard-state (Option B)
- 14 new Playwright specs (7 × 2 browsers), 15 new vitest tests (115 total, was 100)

**Reviewer findings addressed:** 2 CRITICAL + 3 HIGH + 9 MEDIUM (8 LOWs deferred, documented in PR body)

**Open product questions** (from security review, defer to W20c):
1. Rate limiting on `POST /api/atlas/workspaces` — currently proxy-gated per OPERATOR-RUNBOOK §17. Document explicit accept OR add per-IP rate limit.
2. Orphaned workspace dir on signer failure (`ATLAS_DEV_MASTER_SEED` not set in prod) — silent retry vs unconfigured-workspace state in next GET? W20c scope.

═══════════════════════════════════════════════════════════════════════════════
3. ENVIRONMENTAL LOWs flagged by regression-fix subagent (file as issues)
═══════════════════════════════════════════════════════════════════════════════

Both are infrastructure/devloop issues, NOT code defects. Subagent worked around them locally but flagged for orchestrator decision.

### LOW-ENV-1 — Playwright port-3001 collision risk

`apps/atlas-web/playwright.config.ts` uses port `3001` with `reuseExistingServer: !process.env.CI`. If ANY other local service occupies 3001 (e.g. Docker container forwarding), Playwright will not start its own Next server and will silently hit the wrong app → all tests fail with confusing 500s. Subagent encountered exactly this trap (Docker container `milde_partner_frontend` was on 3001).

**Suggested fix:** switch e2e port to `4001` (less likely to clash), OR set `reuseExistingServer: false` with a preflight check that fails fast on port-busy. CI unaffected (`CI=1` sets `reuseExistingServer: false`).

### LOW-ENV-2 — WASM verifier artifacts missing in fresh worktrees

`apps/atlas-web/public/wasm/atlas_verify_wasm.{js,wasm,d.ts}` are gitignored. Fresh worktrees don't have them → LiveVerifierPanel shows `ERROR: Failed to fetch dynamically imported module` → home.spec.ts assertions waiting on `verifier-version` time out. Subagent worked around by copying from main repo's `public/wasm/`.

**Suggested fix:** either (a) document a worktree-creation step that runs `bash scripts/build-wasm.sh` OR copies from main repo, OR (b) make the wasm build a `predev`/`pretest:e2e` script in `apps/atlas-web/package.json` so it's automatic. CI gets these via the build step today.

═══════════════════════════════════════════════════════════════════════════════
4. LESSONS — 4 candidates to codify (Phase 14.8 work)
═══════════════════════════════════════════════════════════════════════════════

Existing Lessons #1-#19 in `.handoff/v2-session-handoff.md`. Lessons #20-#24 in `stash@{0}`. Still to codify in master `.handoff/v2-session-handoff.md`:

- **#25 — Aggregator pattern for required CI checks** (from W20a-meta) — write status-aggregator job that depends on matrix children and is what the branch ruleset gates on. Avoids "matrix child name unstable across path-filter skips" footgun.
- **#26 — Branched UI state requires existing-test audit** (from W20b-2) — When introducing conditional rendering on a heavily-tested route, implementer brief MUST require: "run the FULL playwright suite for the touched route(s), not just the new specs you added. Pre-existing tests that fail because the rendered tree changed are regressions, not pre-existing failures."
- **#27 — Commit-precise terminology, ban "pre-existing"** (from W20b-2) — agent reports must use commit SHA attribution ("introduced by X, not addressed by Y, fixed by this commit"). Bare term "pre-existing" is ambiguous across multi-commit branches.
- **#28 (tentative) — Native `<dialog>` ref-callback pattern** (from W20b-2) — for React wrappers around native imperative-API elements (`<dialog>`, `<video>`, `<canvas>`) where listener registration depends on element being mounted, use ref-callback, not `useRef`+`useEffect`. Promote to full Lesson if another welle hits this trap; otherwise downgrade to code-review checklist item.

═══════════════════════════════════════════════════════════════════════════════
5. ROADMAP STATUS
═══════════════════════════════════════════════════════════════════════════════

| Welle | Scope | Status |
|---|---|---|
| W18c-A/B/C | Layer 3 fastembed + LanceDB cross-platform | SHIPPED ✓ |
| W18c-D | Layer 3 LanceDB ANN/search body fill-in | OPEN (PR #108, brief pre-staged) |
| W20a | atlas-web real-workspace data + selector | SHIPPED ✓ |
| W20a-meta | CI ruleset aggregator pattern | SHIPPED ✓ |
| W20b-1 | atlas-web dashboard real-data + 3-tier UX + /demo/bank + coming-soon nav | SHIPPED ✓ |
| **W20b-2** | **first-run workspace-creation wizard + bundled fixes** | **SHIPPED ✓ (`ba4e27f`)** |
| W20c | settings UI + Layer 3 honest status + orphan-workspace UX | NEXT product candidate |
| W20d | quick-start docs + acceptance validation | after W20c |
| W30 | HTML vault MVP (Layer 0) | after W20 complete |
| W40 | Desktop installer (Electron) | after W30 |
| Phase 14.8 | cross-doc consolidation (Lessons #20-#28, master plan, decisions log) | natural break point — could happen now |
| v2.0.0-beta.2 | tag/publish | after W30 |

═══════════════════════════════════════════════════════════════════════════════
6. ENGINEERING STATE (preserved but not primary)
═══════════════════════════════════════════════════════════════════════════════

- **PR #108 (Phase D — LanceDB ANN/search body fill-in):** STILL OPEN at `885eacc`. 13 reviewer findings unaddressed. Subagent brief pre-staged at `.handoff/pr-108-fix-commit-subagent-brief.md`. Dispatch can happen any time — orthogonal to product-track.
- **`stash@{0}`:** still present (Lessons #20-#24 + §0z9 + §0-NEXT). Pop during Phase 14.8 — NOT yet.
- **Issue #110:** V2 pagination tracking issue. W20b-2 added no new pagination concerns (workspaces list is alphabet-sorted directory listing).

═══════════════════════════════════════════════════════════════════════════════
7. CLEANUP CHECKLIST (small housekeeping for next session)
═══════════════════════════════════════════════════════════════════════════════

**Superseded `.handoff/` bootstrap docs** (4 files, all untracked — safe to delete):

```bash
rm .handoff/next-session-bootstrap-v2-beta-1-live.md
rm .handoff/next-session-bootstrap-w20a-shipped.md
rm .handoff/next-session-bootstrap-w20b-1-shipped.md
rm .handoff/next-session-bootstrap-w20b-2-in-flight.md
# Keep:
#   .handoff/next-session-bootstrap-w20b-2-shipped.md  ← THIS FILE (current)
#   .handoff/pr-108-fix-commit-subagent-brief.md       ← still relevant
#   .handoff/v2-demo-sketches.md                       ← still relevant
```

**Other house items:**

- `apps/atlas-web/next-env.d.ts` is showing as modified in `git status` — cosmetic master drift, ignore unless `next build` complains
- Other W20a/W20a-meta/W20b-1 worktrees are still locked in `.claude/worktrees/` — they survive harmlessly; clean up when convenient with `git worktree remove --force` (the W20b-2 directory itself may still be on disk at `.claude/worktrees/agent-a58b148f1f6ed3c01` because of a Windows file lock during teardown — git no longer tracks it, but `rm -rf` it manually if it bothers you)

═══════════════════════════════════════════════════════════════════════════════
8. ANTI-DRIFT CHECKLIST
═══════════════════════════════════════════════════════════════════════════════

- ❌ NIEMALS v2.0.0-beta.2 tag ohne W20 + W30 shipped
- ❌ NIEMALS bank-demo entfernen — bleibt als `/demo/bank` opt-in showcase
- ❌ NIEMALS LiveVerifierPanel frozen-testid contract verletzen
- ❌ NIEMALS subagent ohne Lessons #1/#17/#21/#22/#23/#26/#27 dispatchen
- ❌ NIEMALS "✓ green" claim akzeptieren ohne `gh pr checks <PR#>` (Lesson #20)
- ❌ NIEMALS atlas-web dev server killen ohne Nelson zu fragen
- ❌ NIEMALS counsel firms kontaktieren bis Nelson explicit consent
- ❌ NIEMALS untracked .handoff/* committen ohne explicit OK
- ❌ NIEMALS ruleset modifizieren ohne Nelson consent
- ❌ NIEMALS frozen testids ändern/entfernen — nur add (Lesson aus W20a-meta + W20b-1 + W20b-2)

═══════════════════════════════════════════════════════════════════════════════
9. OPENING SCRIPT FÜR NELSON
═══════════════════════════════════════════════════════════════════════════════

1. **Pre-flight verify** (§1) — master HEAD `ba4e27f`, 0 in-flight product branches, stash@{0} preserved, only #108 open (engineering parallel)
2. **Bestätige clean ship-state** — W20a/meta/b-1/b-2 alle shipped, kein in-flight
3. **Frage Nelson:** "W20b-2 ist live. 4 next-action candidates (§0 TL;DR table): A=W20c planning, B=PR #108 fix-commit, C=Phase 14.8 + .handoff cleanup, D=env-issue triage. Welche Reihenfolge?"
4. Default empfehlung: **C → A** (cleanup first for clean slate, dann W20c). Option B kann parallel laufen.
5. Bei Option A — verwende `planner` agent oder Plan mode für W20c
6. Bei Option B — copy-paste den brief aus `.handoff/pr-108-fix-commit-subagent-brief.md` als-is, dispatch mit `isolation: worktree`, `run_in_background: true`
7. Bei Option C — single-message-batch: `rm` superseded handoffs + `git stash pop` (if Nelson agrees) + write Lessons #25/#26/#27 to `.handoff/v2-session-handoff.md` + file 2 GitHub issues for LOW-ENV-1 + LOW-ENV-2

═══════════════════════════════════════════════════════════════════════════════
END OF NEXT-SESSION BOOTSTRAP
═══════════════════════════════════════════════════════════════════════════════

Master HEAD: `ba4e27f` (W20b-2 SHIPPED)
In-flight branches: NONE
Open PRs: #108 (Phase D engineering parallel) + 3 archive
Roadmap: W20b-2 ✓ → W20c next (or PR #108 parallel) → W20d → W30 → W40
Lessons codified: 24 (Lessons #25-#28 candidates pending Phase 14.8)
Decisions: 31 in decisions.md (additions pending Phase 14.8)
Date written: 2026-05-16 evening Berlin
Today's session: 1 long session — W20b-1 validation + W20b-2 implement + review + fix + regression-fix + ship cascade — **1 ship-event delivered**
