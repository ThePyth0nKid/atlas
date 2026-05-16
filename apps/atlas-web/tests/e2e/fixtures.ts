/**
 * V1.19 Welle 11 — Shared Playwright fixtures for the atlas-web E2E suite.
 *
 * The single shared next-server (started by playwright.config.ts) writes
 * to a process-wide ATLAS_DATA_DIR. Parallel-running tests must NOT
 * collide on the same workspace_id — otherwise the per-workspace mutex
 * serialises them, the chain ordering is non-deterministic, and the
 * "first event is genesis" assertion in write.spec.ts becomes flaky.
 *
 * The `workspace` fixture provides a per-test unique workspace_id of
 * shape `pw-{worker}-{ts}-{rand}`, satisfying the route handler's
 * `^[a-zA-Z0-9_-]{1,128}$` regex with high entropy.
 *
 * We deliberately do NOT clean up workspace data dirs after tests —
 * the dirs are small (<10 KB per test) and the parent `data/` dir is
 * gitignored. A periodic `rm -rf apps/atlas-web/data/pw-*` is a safe
 * cleanup if disk usage ever becomes a concern in CI.
 */

import { test as base, expect } from "@playwright/test";
import type { Page } from "@playwright/test";

type Fixtures = {
  workspace: string;
};

export const test = base.extend<Fixtures>({
  workspace: async ({}, use, testInfo) => {
    const ts = Date.now().toString(36);
    const rand = Math.random().toString(36).slice(2, 8);
    const id = `pw-w${testInfo.workerIndex}-${ts}-${rand}`;
    await use(id);
  },
});

export { expect };

/**
 * Provision a workspace with one Genesis event and pin it as the
 * active workspace via localStorage for the WorkspaceProvider. Returns
 * once the seed POST has succeeded and the init-script is registered;
 * the caller is expected to invoke `page.goto(...)` immediately after.
 *
 * W20b-2 — extracted from `home.spec.ts` (V1.19 Welle 11) so other
 * specs can call the canonical seed helper instead of re-creating it.
 * The fix-commit for W20b-2 introduced `<HomeContent>` which branches
 * on `workspaces.length === 0` and renders `<FirstRunWizard />` on the
 * empty-state — any cold `page.goto("/")` that wants to assert
 * dashboard behaviour MUST seed first (or rely on the
 * server-side `pw-w*-*` filter + localStorage splice in
 * `WorkspaceProvider`, which keeps the seeded workspace visible to the
 * UI even though it's filtered out of the public `/api/atlas/workspaces`
 * GET).
 *
 * Behaviour:
 *   - POST `/api/atlas/write-node` with one `kind="dataset"` event
 *     (sufficient for a valid Genesis trace; the verifier exercises
 *     all V1.0 checks on a single-event chain).
 *   - `addInitScript` seeds `localStorage["atlas:active-workspace"]`
 *     BEFORE navigation so the `WorkspaceProvider` reads it
 *     synchronously in its mount effect.
 *
 * Why we do NOT also POST to `/api/atlas/workspaces`: the write-node
 * route auto-creates the workspace directory on first write, so a
 * separate create call would be redundant (and would surface 409 on
 * retry). Tests that need explicit workspace existence (e.g.
 * duplicate-id collision tests) call `/api/atlas/workspaces` directly
 * in their own setup.
 */
export async function provisionAndSelect(
  page: Page,
  workspace: string,
): Promise<void> {
  // Provision via the existing write-node route. One node.create
  // event with kind="dataset" is sufficient for a valid trace; the
  // verifier exercises all V1.0 checks on it.
  const writeRes = await page.request.post("/api/atlas/write-node", {
    data: {
      workspace_id: workspace,
      kind: "dataset",
      id: "home-spec-genesis",
      attributes: {},
    },
  });
  expect(writeRes.ok()).toBe(true);

  // Seed localStorage BEFORE navigation. Next.js layout mounts the
  // WorkspaceProvider on first render; the provider reads localStorage
  // synchronously in its effect.
  await page.addInitScript((ws: string) => {
    window.localStorage.setItem("atlas:active-workspace", ws);
  }, workspace);
}
