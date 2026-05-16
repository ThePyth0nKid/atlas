# W20c — Settings UI + Layer 3 honest status + orphan-workspace UX

> **Status:** PLAN READY — executor dispatch staged 2026-05-16 evening Berlin.
> **Master baseline:** `480bc42` (Phase 14.8 SHIPPED on top of W20b-2 `ba4e27f`).
> **Target branch:** `feat/product/welle-20c-bundle`.
> **Bundle pattern:** same as W20b-2 (3 surfaces, one squash-merged PR).
> **Estimated executor runtime:** 3-4h.
> **Author:** orchestrator + `planner` subagent (a28f927d024c8e683).

---

## 0. One-paragraph synopsis

W20c gives Atlas users a real `/settings` route to manage workspaces (rename + delete with typed-confirmation), inspect compiled-in supply-chain pins, and see honest signer status — alongside a new `LayerStatusPanel` on `/` that surfaces the real state of L3 embedder/backend/signer (no more "ready" lies). The third surface closes the W20b-2 security follow-up: if `POST /api/atlas/workspaces` succeeds at `mkdir` but `derivePubkeyViaSigner` fails (e.g. `ATLAS_DEV_MASTER_SEED` not set), the route now atomically rolls back the freshly-created directory and returns 500, so users never see an orphan "unconfigured" workspace. All three surfaces ship in one squash-merged PR (`feat/product/welle-20c-bundle`) following the W20b-2 cascade pattern. The 3-tier dashboard UX is upgraded from event-count-only to event-count × layer-readiness, so "ready" only shows when L3 is actually wired.

---

## 1. Blocking assumptions surfaced during plan (reality-corrections vs bootstrap brief)

Reading the codebase versus the planning brief surfaced four corrections — flagged here so the executor cannot diverge by reading the brief literally:

**A1. There is NO `/api/atlas/dashboard` route.** The bootstrap §0 mentioned extending it; the dashboard metrics section computes locally from `/api/atlas/trace` (see `apps/atlas-web/src/components/DashboardMetricsSection.tsx`). **Decision:** add a NEW route `/api/atlas/system/health/route.ts` for the layer status block; do not pretend to extend a route that doesn't exist.

**B1. Supply-chain pins are 11, not 9.** Per `crates/atlas-mem0g/src/embedder.rs` lines 166-244, the compile-in set is `HF_REVISION_SHA` + 5 SHA-256 (ONNX + 4 tokenizer) + 5 URLs = 11 constants. The settings UI lists all 11.

**C1. WorkspaceContext exposes `workspace` (selected id) + `setWorkspace` + `workspaces` + `createWorkspace`.** No `selectedWorkspace`. Plan refers to `workspace` accordingly. Context will be extended with `renameWorkspace` + `deleteWorkspace` in W20c.

**D1. The Rust signer is invoked via spawn from `packages/atlas-bridge/src/signer.ts`; there is no in-process FFI.** Rollback path therefore lives in TypeScript and is `fs.rm({ recursive: true, force: true })` of the freshly-mkdir'd dir on signer error — safe because `ensureWorkspaceDir` only creates an empty directory (no events.jsonl yet).

None of these are blocking *for the plan* — the plan absorbs them. They ARE blocking if the executor reads the brief literally.

---

## 2. Architecture decisions (locked — executor MUST NOT deviate without orchestrator OK)

- **DA-1 (orphan UX):** Server-side atomic rollback on signer failure. POST mkdir → if signer fails → `fs.rm(workspaceDir, {recursive, force})` → 500 with `signer:` prefix. Rationale: dir at that moment contains only the empty workspace skeleton (no signed events); rolling back leaves the user with the same on-disk state they had before the request. Path B (mark as "unconfigured" + UI retry button) leaves dead state under `dataDir()` that users can't discover or clean up — fails standing directive A. One exception: if `fs.rm` itself fails (rare; concurrent process held a handle), surface `partial_rollback` in the 500 body.
- **DA-2 (rename mechanism):** PATCH endpoint, not client-only label. Workspace ids ARE the filesystem path segment (`workspaceDir(id)`); a client-only rename would desync the UI label from the on-disk truth. PATCH does an atomic `fs.rename(oldDir, newDir)` after validating both old + new ids against `WORKSPACE_ID_RE` and asserting the new dir doesn't exist. Per-tenant kid stays workspace-id-derived, so rename triggers re-derivation of the pubkey (which the response returns); the client-side `localStorage["atlas:active-workspace"]` is updated to the new id.
- **DA-3 (status surface placement):** New route `/api/atlas/system/health/route.ts` returns `{ embedder, backend, signer }` shape. Cleanly separable from per-workspace trace data; cacheable (60-second probe TTL); easy to test in isolation. Settings UI consumes for signer status; dashboard renders new `<LayerStatusPanel>` reading the same endpoint.
- **DA-4 (3-tier readiness):** Promote the existing `DashboardMetricsSection` tier choice from `totalEvents`-only to `min(eventCountTier, layerReadinessTier)`. Workspace with 200 events but signer=unconfigured renders Empty (with a banner pointing to `/settings`), not Full. A "Full" tier when the verifier can't actually sign new events is a lie. Keep frozen testids — only ADD `dashboard-layer-not-ready` testid for the new banner.
- **DA-5 (testid contract):** All `LiveVerifierPanel` testids stay frozen (Lesson #19). New testids exclusively prefix `settings-` and `layer-status-`. No re-use of existing prefixes.
- **DA-6 (DELETE confirmation):** Typed-confirmation modal — user must type the exact workspace id into a separate input to enable the delete button. The DELETE endpoint refuses to delete the last user-facing workspace (returns 409 with `cannot delete last workspace`) — prevents the orphan-state where the UI flips back to FirstRunWizard mid-session.

---

## 3. File-by-file change list

### Server (API routes + bridge)

| Status | File | Purpose |
|--------|------|---------|
| **MODIFY** | `apps/atlas-web/src/app/api/atlas/workspaces/route.ts` | (a) POST: wrap signer call in try-rollback (DA-1). (b) Add PATCH handler (rename, DA-2). (c) Add DELETE handler (DA-6). |
| **MODIFY** | `apps/atlas-web/src/app/api/atlas/workspaces/route.test.ts` | New tests: PATCH happy/409/404/400; DELETE happy/404/409-last; POST signer-failure-triggers-rollback. |
| **NEW** | `apps/atlas-web/src/app/api/atlas/system/health/route.ts` | GET endpoint returning `{ embedder, backend, signer }` block (DA-3). 60-second in-memory cache. `runtime = "nodejs"`. |
| **NEW** | `apps/atlas-web/src/app/api/atlas/system/health/route.test.ts` | Contract: shape, status enum values, env-toggle probes, cache hit. |
| **NEW** | `apps/atlas-web/src/app/api/atlas/system/supply-chain-pins/route.ts` | GET endpoint returning 11 pin constants. Reads from a generated TS module. `runtime = "nodejs"`. |
| **NEW** | `apps/atlas-web/src/app/api/atlas/system/supply-chain-pins/route.test.ts` | Contract: shape, 11 constants present, hex format. |
| **NEW** | `apps/atlas-web/src/lib/supply-chain-pins.ts` | TypeScript mirror of the Rust constants in `crates/atlas-mem0g/src/embedder.rs`. Plain typed export — no codegen for V2-β-1; codegen welle is V2-γ. |
| **NEW** | `apps/atlas-web/src/lib/supply-chain-pins.test.ts` | Parity smoke: assert all 11 hex strings match regex. |
| **MODIFY** | `apps/atlas-web/src/lib/workspace-context.tsx` | Extend `WorkspaceContextValue` with `renameWorkspace(oldId, newId)` and `deleteWorkspace(id)`. Add pure helpers `requestRenameWorkspace`/`requestDeleteWorkspace`. |
| **MODIFY** | `apps/atlas-web/src/lib/workspace-context.test.ts` | New tests: rename happy/invalid-id/server-error; delete happy/last-workspace-refusal; localStorage migrations. |

### Client (UI)

| Status | File | Purpose |
|--------|------|---------|
| **NEW** | `apps/atlas-web/src/app/settings/page.tsx` | App-router server component shell. Delegates to `<SettingsContent>` client component. |
| **NEW** | `apps/atlas-web/src/components/SettingsContent.tsx` | Top-level client component. Composes 4 panels. |
| **NEW** | `apps/atlas-web/src/components/WorkspaceListPanel.tsx` | Lists workspaces from context; per-row Rename + Delete buttons. |
| **NEW** | `apps/atlas-web/src/components/RenameWorkspaceDialog.tsx` | Native `<dialog>` modal with ref-callback pattern (Lesson candidate #28). |
| **NEW** | `apps/atlas-web/src/components/DeleteWorkspaceDialog.tsx` | Typed-confirmation modal (DA-6). |
| **NEW** | `apps/atlas-web/src/components/SupplyChainPinsPanel.tsx` | Static read-only display of 11 pins. |
| **NEW** | `apps/atlas-web/src/components/SignerStatusPanel.tsx` | Reads `/api/atlas/system/health`. 3 rows. |
| **NEW** | `apps/atlas-web/src/components/LayerStatusPanel.tsx` | Same data source as SignerStatusPanel but compact rendering for the dashboard. |
| **NEW** | `apps/atlas-web/src/components/ComingSoonPanel.tsx` | 3 disabled controls (Retention SLA / Cipher-key rotation / Semantic-search budget). |
| **MODIFY** | `apps/atlas-web/src/components/HomeContent.tsx` | Wire in `<LayerStatusPanel>` above `<DashboardMetricsSection>`. Pass health status into DashboardMetricsSection. |
| **MODIFY** | `apps/atlas-web/src/components/DashboardMetricsSection.tsx` | Accept optional `layerStatus` prop; tier degrades to Empty when `signer !== 'operational'`. |
| **MODIFY** | `apps/atlas-web/src/app/layout.tsx` | Add `/settings` link to NAV array. |

### Helper modules + types

| Status | File | Purpose |
|--------|------|---------|
| **NEW** | `apps/atlas-web/src/lib/system-health.ts` | `LayerStatus` types + pure probe helpers (`probeSigner`/`probeEmbedder`/`probeBackend`). |
| **NEW** | `apps/atlas-web/src/lib/system-health.test.ts` | Vitest unit tests for each probe + cache eviction semantics. |

### Playwright (e2e)

| Status | File | Purpose |
|--------|------|---------|
| **NEW** | `apps/atlas-web/tests/e2e/settings.spec.ts` | Page renders + rename happy/invalid + delete typed-confirm + supply-chain pins + signer status. |
| **NEW** | `apps/atlas-web/tests/e2e/orphan-workspace.spec.ts` | With signer-failure env, POST returns 500 + subsequent GET excludes the failed id. |
| **NEW** | `apps/atlas-web/tests/e2e/layer-status-panel.spec.ts` | Dashboard renders LayerStatusPanel; DA-4 tier degradation. |
| **MODIFY** | `apps/atlas-web/tests/e2e/a11y.spec.ts` | Add WCAG sweep for `/settings` (initial + with each dialog open). |
| **MODIFY** | `apps/atlas-web/tests/e2e/dashboard-tiers.spec.ts` | Add `layer-status-panel` visibility anchor; add DA-4 degradation test. |
| **MODIFY** | `apps/atlas-web/tests/e2e/fixtures.ts` | Add `provisionAndSelectMany(page, count)` + `forceSignerUnconfigured(page)`. |

### Docs

| Status | File | Purpose |
|--------|------|---------|
| **MODIFY** | `docs/OPERATOR-RUNBOOK.md` | New §18 "Signer health probe + L3 status surface". |
| **MODIFY** | `CHANGELOG.md` | New W20c entry under unreleased. |

---

## 4. API surface table

| Method | Path | Request schema | Response 2xx | Errors |
|--------|------|----------------|--------------|--------|
| GET | `/api/atlas/workspaces` | (unchanged W20a) | (unchanged) | (unchanged) |
| POST | `/api/atlas/workspaces` | `{ workspace_id: WORKSPACE_ID_RE }` (Zod-strict, ≤4 KB) | `{ ok:true, workspace_id, kid, pubkey_b64url }` | 400 invalid; 409 exists; 413 oversize; **500 `signer: <redacted>` AFTER rollback** OR `500 partial_rollback:` if rm fails |
| **PATCH** | `/api/atlas/workspaces` | `{ workspace_id, new_workspace_id }` (both regex-validated, ≤4 KB) | `{ ok:true, workspace_id: new, kid, pubkey_b64url }` | 400 invalid; 404 source missing; 409 target exists; 413 oversize; 500 redacted |
| **DELETE** | `/api/atlas/workspaces` | `{ workspace_id }` (regex-validated, ≤4 KB) | `{ ok:true, workspace_id }` | 400 invalid; 404 not found; **409 `cannot delete last workspace`**; 500 redacted |
| **GET** | `/api/atlas/system/health` | (none) | `{ ok:true, embedder, backend, signer }` | 500 redacted |
| **GET** | `/api/atlas/system/supply-chain-pins` | (none) | `{ ok:true, hf_revision_sha, onnx_sha256, tokenizer_json_sha256, config_json_sha256, special_tokens_map_sha256, tokenizer_config_json_sha256, model_url, tokenizer_json_url, config_json_url, special_tokens_map_url, tokenizer_config_json_url }` | 500 (compile-in constants — shouldn't happen) |

**Status enum values:**
- `embedder: 'operational' | 'model_missing' | 'unsupported'`
- `backend: 'operational' | 'stub_501' | 'fault'`
- `signer: 'operational' | 'unconfigured'`

**Threat model** (all new routes):
- `runtime = "nodejs"` + `dynamic = "force-dynamic"` (no caching in Next.js layer)
- All bodies subject to 4 KB cap before parse
- All paths via `workspaceDir(id)` (regex-validated) — path-traversal structurally impossible
- All error responses through `redactPaths()`
- PATCH rename: validate both old + new, stat both, refuse cross-mount with explicit error, use `fs.rename` (atomic on same volume)
- DELETE: refuse to delete the last workspace (409); `force: false` so a vanished dir reads as 404

---

## 5. Render-tree audit (Lesson #26 binding)

Existing Playwright specs that touch routes we modify:

### Route `/` (home — modified by W20c via `<LayerStatusPanel>` addition + DashboardMetricsSection `layerStatus` prop)

| Spec | Existing cold-goto safety | W20c required change |
|------|----------------------------|----------------------|
| `home.spec.ts` | Uses `provisionAndSelect` (W20b-2 fixture) | Verify pass with `<LayerStatusPanel>` mounted. Add `await expect(page.getByTestId('layer-status-panel')).toBeVisible()` to at least one test. |
| `dashboard-tiers.spec.ts` | `pinWorkspace` + `writeNode` helpers | Existing FullTier test should still pass under `ATLAS_DEV_MASTER_SEED=1` (signer=operational). Add new DA-4 test pinned to signer=unconfigured force. |
| `a11y.spec.ts` — "home page (dashboard state)" | `provisionAndSelect` | Re-run after `<LayerStatusPanel>` mounts; panel pills MUST hit ≥4.5:1 contrast. |
| `a11y.spec.ts` — "home page (first-run wizard state)" | Cold-goto empty | First-run wizard tree unchanged. No work needed. |
| `first-run-wizard.spec.ts` | Cold-goto | Wizard tree does NOT include LayerStatusPanel. Add regression assertion that `layer-status-panel` does NOT render in wizard state. |
| `workspace-selector.spec.ts` | `provisionAndSelect` | Add new test: clicking "+ New" → settings page lists new workspace. |
| `workspace-selector-dialog.spec.ts` (W20b-2) | Cold-goto + dialog open | No change. |

### Route `/settings` (NEW — no prior specs)

| Spec | Status |
|------|--------|
| `settings.spec.ts` (NEW) | Cold-goto with `provisionAndSelectMany(page, 2)` fixture seeding |
| `a11y.spec.ts` extensions | Cold-goto on `/settings` with workspace seeded |

**Fixture changes required:**
- `provisionAndSelectMany(page, count)` — for rename/delete tests (needs ≥2 workspaces)
- `forceSignerUnconfigured(page)` — addInitScript that sets `window.__atlasForceSignerUnconfigured = true` which `/api/atlas/system/health` checks via a test-only request header. ONLY honored when `ATLAS_E2E_TEST_HOOKS === "1"` (set by `playwright.config.ts`)

---

## 6. Phase-by-phase implementation order

Each phase is independently verifiable. Dependencies explicit.

### Phase 1 — Bridge + types (25 min)
- `supply-chain-pins.ts` + test
- `system-health.ts` + test (pure probes, env-only)
- Run vitest → green
- **Exit gate:** vitest green, typecheck clean

### Phase 2 — Server API routes (50 min)
- `/api/atlas/system/health/route.ts` + test (60s TTL cache)
- `/api/atlas/system/supply-chain-pins/route.ts` + test
- Modify `workspaces/route.ts`: POST rollback + PATCH + DELETE
- Extend `workspaces/route.test.ts`: 9 new test groups
- **Exit gate:** vitest green for all route tests

### Phase 3 — Context extension (30 min)
- Extend WorkspaceContext: renameWorkspace + deleteWorkspace + pure helpers
- localStorage migration on rename; clear on delete-of-active
- Extend workspace-context.test.ts
- **Exit gate:** vitest green

### Phase 4 — Layer status UI (35 min)
- `LayerStatusPanel.tsx` (reads `/api/atlas/system/health`, 3 pills)
- Modify `HomeContent.tsx`: mount above DashboardMetricsSection; new `useSystemHealth` hook
- Modify `DashboardMetricsSection.tsx`: accept layerStatus prop; tier degrades on unconfigured
- **Exit gate:** `pnpm dev` manual smoke shows panel + tier degradation

### Phase 5 — Settings UI (55 min)
- `app/settings/page.tsx` + `SettingsContent.tsx`
- `WorkspaceListPanel.tsx` + `RenameWorkspaceDialog.tsx` + `DeleteWorkspaceDialog.tsx`
- `SupplyChainPinsPanel.tsx` + `SignerStatusPanel.tsx` + `ComingSoonPanel.tsx`
- Modify `layout.tsx`: add `/settings` nav entry
- **Exit gate:** `pnpm dev` manual smoke — all 4 panels render + rename/delete work

### Phase 6 — Playwright + a11y (45 min)
- Extend `fixtures.ts`: `provisionAndSelectMany` + `forceSignerUnconfigured`
- New `settings.spec.ts` + `orphan-workspace.spec.ts` + `layer-status-panel.spec.ts`
- Modify `a11y.spec.ts` + `dashboard-tiers.spec.ts` (Lesson #26 audit)
- Run `pnpm test:e2e` chromium + firefox
- **Exit gate:** ALL e2e specs pass (including pre-existing)

### Phase 7 — Docs + commit + PR (25 min)
- Modify `docs/OPERATOR-RUNBOOK.md` §18
- Modify `CHANGELOG.md`
- Run full local gates:
  - `pnpm verify-wasm` (byte-pin invariant)
  - `pnpm test` (vitest)
  - `pnpm test:e2e` (Playwright, both browsers)
  - `cargo clippy --workspace --all-targets` (zero warnings)
- Single squash commit (template below)
- Push branch, `gh pr create`, wait for 3 required CI gates
- **Exit gate:** PR open + CI green on all 3 required checks

---

## 7. Risk register (top 5, ranked likelihood × impact)

| # | Risk | L × I | Mitigation |
|---|------|-------|------------|
| **R1** | Rollback `fs.rm` races with concurrent process holding a file handle on Windows | M × H | `force: false` surfaces underlying error; `partial_rollback:` prefix in 500 message; vitest mock test |
| **R2** | Health probe spawns Rust signer → dashboard latency | M × M | Probes are env-only (no spawn). 60s TTL on response. No process boundary crossed. |
| **R3** | PATCH rename across mountpoints fails (EXDEV) | L × H | Detect EXDEV; surface 500 `cross_mount_rename_unsupported`. Documented in OPERATOR-RUNBOOK §18. |
| **R4** | Lesson #26 violation — DOM shift breaks Playwright `nth()` selector silently | M × M | Phase 6 step audits home/dashboard-tiers/a11y. Full e2e suite run before commit. |
| **R5** | DELETE-last-workspace contract breaks first-run flow | L × H | Server enforces 409 (truth); client disables Delete on count=1; banner hint; even if defences fail, 409 prevents bad state. |

---

## 8. Subagent dispatch brief (copy-paste verbatim into `Agent({...})`)

```
You are executing W20c for Atlas. Single-PR bundle. Branch:
feat/product/welle-20c-bundle. Time budget: ~3-4 hours.

═══════════════════════════════════════════════════════════════
PRE-FLIGHT (DO THIS FIRST — Lesson #1 worktree-isolation)
═══════════════════════════════════════════════════════════════
1. cd to the assigned worktree path (passed in cwd).
2. git fetch origin
3. git checkout -B feat/product/welle-20c-bundle origin/master
   (Master HEAD must be 480bc42 — Phase 14.8 SHIPPED on top of W20b-2.)
4. Verify: git log --oneline -3 → 480bc42, ba4e27f, 0460f5a
5. Copy WASM artifacts from main repo if not present:
   ls apps/atlas-web/public/wasm/ → must contain atlas_verify_wasm.{js,wasm,d.ts}
   If missing: copy from main repo's apps/atlas-web/public/wasm/.
   This is LOW-ENV-2 from W20b-2 (issue #115) — wasm artifacts gitignored.

═══════════════════════════════════════════════════════════════
SCOPE — 3 surfaces in one PR
═══════════════════════════════════════════════════════════════
S1. /settings UI route (workspace rename/delete + supply-chain pins
    display + signer status + V2-γ coming-soon placeholders)
S2. Layer 3 honest status — new /api/atlas/system/health route +
    <LayerStatusPanel> above <LiveVerifierPanel> on / +
    DashboardMetricsSection tier degrades when signer unconfigured
S3. Orphan-workspace UX — POST /api/atlas/workspaces atomically
    rolls back (fs.rm of empty workspaceDir) on signer failure.

═══════════════════════════════════════════════════════════════
ARCHITECTURE DECISIONS (LOCKED — do not deviate without orchestrator OK)
═══════════════════════════════════════════════════════════════
DA-1: Rollback (not retry, not unconfigured-state) on POST signer fail.
DA-2: PATCH endpoint for rename (atomic fs.rename + signer re-derive).
DA-3: New /api/atlas/system/health route — env-only probes, 60-sec TTL.
DA-4: 3-tier = min(eventCountTier, layerReadinessTier).
DA-5: All LiveVerifierPanel testids frozen; new testids prefixed
      "settings-" or "layer-status-".
DA-6: DELETE refuses 409 on last user-facing workspace + typed-
      confirmation client gating.

═══════════════════════════════════════════════════════════════
FILE LIST — see .handoff/v2-beta-welle-20c-plan.md §3 for full table
═══════════════════════════════════════════════════════════════
NEW (16): see plan §3.
MODIFY (11): workspaces/route.ts + .test.ts, workspace-context.tsx +
  .test.ts, DashboardMetricsSection.tsx, HomeContent.tsx, layout.tsx,
  a11y.spec.ts, dashboard-tiers.spec.ts, fixtures.ts, OPERATOR-
  RUNBOOK.md, CHANGELOG.md.

═══════════════════════════════════════════════════════════════
EXECUTION ORDER (7 phases, each independently verifiable)
═══════════════════════════════════════════════════════════════
P1 Bridge+types (25min)     → vitest green
P2 API routes (50min)        → vitest green
P3 Context extension (30min) → vitest green
P4 LayerStatusPanel (35min)  → manual smoke + DashboardMetrics test
P5 Settings UI (55min)       → manual smoke
P6 Playwright + a11y (45min) → FULL e2e green, BOTH browsers
P7 Docs + commit + PR (25min)→ all gates green, PR open

═══════════════════════════════════════════════════════════════
LESSON #26 — branched-UI existing-test audit MANDATORY
═══════════════════════════════════════════════════════════════
You are adding <LayerStatusPanel> above <LiveVerifierPanel> in
<HomeContent>. The / route has 4 existing e2e specs that exercise
that tree (home.spec.ts, dashboard-tiers.spec.ts, a11y.spec.ts,
first-run-wizard.spec.ts). You MUST run the FULL e2e suite (not
just new specs) before committing. Pre-existing tests that fail
because the DOM tree shifted are regressions, not "pre-existing
failures." Per plan §5 render-tree audit, add structural anchor
assertions for layer-status-panel visibility in dashboard tests.

═══════════════════════════════════════════════════════════════
LESSON #27 — commit-precise terminology
═══════════════════════════════════════════════════════════════
In your final self-report and commit body, use commit-SHA
attribution. Examples:
  "POST orphan path introduced by ba4e27f, security-reviewer
   flagged in PR #113 review, fixed by this commit."
  "DashboardMetricsSection signature changed in this commit;
   existing testids preserved per frozen-testid contract."
Never use bare "pre-existing" or "regression" — always tie to SHAs.

═══════════════════════════════════════════════════════════════
ACCEPTANCE GATES (all required before opening PR)
═══════════════════════════════════════════════════════════════
[ ] cd apps/atlas-web && pnpm test            → all vitest green
[ ] cd apps/atlas-web && pnpm test:e2e        → both browsers green
[ ] pnpm verify-wasm                          → byte-pin reproduces
[ ] cargo clippy --workspace --all-targets    → zero warnings
[ ] git diff --stat origin/master..HEAD       → file list matches plan §3
[ ] All commits SSH-Ed25519 signed
[ ] No console.log in production code
[ ] 80%+ coverage for new code

═══════════════════════════════════════════════════════════════
PR + COMMIT
═══════════════════════════════════════════════════════════════
- Single squashed commit. Conventional-commits format.
- NO Co-Authored-By: trailer (settings.json disables attribution
  globally — see atlas_admin_merge_settings memory).
- Use gh pr create with the standard template; set
  base=master, head=feat/product/welle-20c-bundle.
- Wait for 3 required CI checks to go green:
    verify-trust-root, atlas-web-playwright, mem0g-smoke-required.

Commit message template:
```
feat(product/welle-20c): atlas-web settings UI + L3 honest status + orphan-workspace POST rollback

- /settings route with workspace rename + delete (typed-confirmation),
  11-pin supply-chain panel, signer/embedder/backend status block,
  V2-γ coming-soon placeholders
- LayerStatusPanel on / above LiveVerifierPanel (frozen testids)
- DashboardMetricsSection.tier = min(eventCountTier, layerReadinessTier)
- POST /api/atlas/workspaces signer-failure now triggers atomic rollback
  (fs.rm of empty workspaceDir) — no orphan unconfigured workspaces
- New /api/atlas/system/{health,supply-chain-pins} routes
- New PATCH + DELETE on /api/atlas/workspaces
- 4 new e2e specs (settings, orphan, layer-status, a11y extensions)
- 9 new vitest test groups across 4 route+lib tests
- Frozen testid contract extension: settings-*, layer-status-*
- Lesson #26 sweep: dashboard-tiers + a11y specs re-audited for
  route-/ tree changes (LayerStatusPanel mount)
- Lesson #27: orphan-rollback path introduced by ba4e27f, identified
  by security-reviewer in W20b-2 PR body, fixed by this commit.
```

═══════════════════════════════════════════════════════════════
SELF-REPORT FORMAT (return to orchestrator at completion)
═══════════════════════════════════════════════════════════════
1. Commit SHA on branch tip.
2. PR number.
3. Files-touched summary: count of new + modify per plan §3.
4. Test summary: vitest count delta, playwright spec count delta.
5. Coverage delta (if available).
6. Lesson #26 + #27 audit confessions: any pre-existing tests that
   failed-then-passed; any commit-SHA attributions made.
7. Open questions or scope adjustments (with rationale).
8. CI status at hand-off (green / pending / failing).

═══════════════════════════════════════════════════════════════
ANTI-PATTERNS (do NOT)
═══════════════════════════════════════════════════════════════
- Do NOT modify LiveVerifierPanel testids (Lesson #19 frozen contract).
- Do NOT touch crates/* — no Rust changes in W20c.
- Do NOT add Co-Authored-By trailers.
- Do NOT commit .handoff/* files unless orchestrator says so.
- Do NOT use --skip-permissions or similar bypass flags.
- Do NOT extend /api/atlas/dashboard — that route does not exist.
- Do NOT use enum for status; string-literal union per coding-style.md.
- Do NOT mutate workspaces[] in place; immutable updates only.

═══════════════════════════════════════════════════════════════
END OF BRIEF
═══════════════════════════════════════════════════════════════
```

---

## 9. Success criteria

- [ ] `/settings` route renders and lists workspaces from `WorkspaceContext`
- [ ] Workspace rename via PATCH works end-to-end (UI → API → fs.rename → context update → localStorage migration)
- [ ] Workspace delete via DELETE works with typed-confirmation; refuses to delete the last workspace (409)
- [ ] All 11 supply-chain pins display on `/settings`
- [ ] `<LayerStatusPanel>` renders 3 honest status pills above `<LiveVerifierPanel>` on `/`
- [ ] When signer is unconfigured, dashboard renders `EmptyTier` regardless of event count (DA-4)
- [ ] POST `/api/atlas/workspaces` with signer failure leaves NO orphan directory on disk + returns 500
- [ ] All existing playwright specs (`home`, `dashboard-tiers`, `a11y`, `first-run-wizard`, `workspace-selector*`, `write`) still pass
- [ ] New specs `settings`, `orphan-workspace`, `layer-status-panel` all pass on both chromium + firefox
- [ ] `pnpm verify-wasm` byte-pin reproduces
- [ ] `cargo clippy --workspace --all-targets` zero warnings
- [ ] Single coherent PR opened with green CI on all 3 required gates
- [ ] No new HIGH/CRITICAL findings from security-reviewer
- [ ] Lesson #26 + #27 self-report sections present in dispatch agent's return message
