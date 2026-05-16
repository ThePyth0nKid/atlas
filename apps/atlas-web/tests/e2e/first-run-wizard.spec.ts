/**
 * W20b-2 — First-run wizard E2E.
 *
 * Pins the three user-visible contracts of the
 * `<FirstRunWizard>` empty-state:
 *
 *   1. Wizard renders ON `/` when the workspaces fetch resolves with
 *      an empty list. Dashboard tier markers MUST be absent.
 *   2. Wizard submit creates the workspace via POST and redirects to
 *      `/write` with the new workspace selected.
 *   3. Duplicate workspace_id surfaces inline as `first-run-wizard-error`
 *      (no global error boundary, no crash, no redirect).
 *
 * Empty-state setup:
 *   The shared Playwright next-server (started by playwright.config.ts)
 *   writes to a process-wide data dir that accumulates non-CI
 *   workspaces across runs. To force `workspaces.length === 0` we:
 *     (a) Probe `GET /api/atlas/workspaces` before each test.
 *     (b) Test-skip if the user-facing list is non-empty — the
 *         scenario is structurally untestable when prior runs left
 *         workspaces behind. Local dev-environment behaviour;
 *         vitest covers the route contract independently.
 *     (c) Clear localStorage so the provider does not surface a
 *         pinned workspace and inflate `workspacesForUi`.
 *
 * Frozen testids (mirrors `<FirstRunWizard>` JSDoc):
 *   first-run-wizard, first-run-wizard-input, first-run-wizard-submit,
 *   first-run-wizard-error.
 */

import { test, expect } from "./fixtures";
import type { Page } from "@playwright/test";

async function userFacingWorkspaceCount(page: Page): Promise<number> {
  const res = await page.request.get("/api/atlas/workspaces");
  if (!res.ok()) return -1;
  const body = (await res.json()) as { workspaces: string[] };
  return body.workspaces.length;
}

test.describe("First-run wizard — empty workspaces", () => {
  test("first-run wizard renders when workspace list is empty", async ({
    page,
  }) => {
    const count = await userFacingWorkspaceCount(page);
    test.skip(
      count > 0,
      `data root has ${count} user workspaces — empty-state untestable`,
    );
    // Clear any pinned workspace so the provider does not surface a
    // localStorage-only id into `workspacesForUi`.
    await page.addInitScript(() => {
      window.localStorage.removeItem("atlas:active-workspace");
    });
    await page.goto("/");

    await expect(page.getByTestId("first-run-wizard")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("first-run-wizard-input")).toBeVisible();
    await expect(page.getByTestId("first-run-wizard-submit")).toBeVisible();
    // No dashboard tier markers should be present — the wizard
    // REPLACES the dashboard tree on the empty-state.
    await expect(page.getByTestId("dashboard-tier-empty")).toHaveCount(0);
    await expect(page.getByTestId("dashboard-tier-early")).toHaveCount(0);
    await expect(page.getByTestId("dashboard-tier-full")).toHaveCount(0);
  });

  test("first-run wizard creates workspace and redirects to /write", async ({
    page,
  }) => {
    const count = await userFacingWorkspaceCount(page);
    test.skip(
      count > 0,
      `data root has ${count} user workspaces — empty-state untestable`,
    );
    await page.addInitScript(() => {
      window.localStorage.removeItem("atlas:active-workspace");
    });
    await page.goto("/");
    await expect(page.getByTestId("first-run-wizard")).toBeVisible({
      timeout: 15_000,
    });

    // Build a unique workspace id that survives the bridge regex and
    // does NOT match the CI-artifact filter (so it ends up listed in
    // GET responses for future runs, but each run uses a fresh id).
    const id = `wiz${Date.now().toString(36)}${Math.random()
      .toString(36)
      .slice(2, 6)}`;

    const input = page.getByTestId("first-run-wizard-input");
    await input.fill(id);
    await page.getByTestId("first-run-wizard-submit").click();

    // The wizard pushes /write on success.
    await expect(page).toHaveURL(/\/write$/, { timeout: 15_000 });
    // The selector on /write reflects the just-created workspace.
    const current = page.getByTestId("workspace-selector-current");
    await expect(current).toContainText(id, { timeout: 10_000 });
  });

  test("first-run wizard shows error inline on duplicate workspace_id", async ({
    page,
    workspace,
  }) => {
    const count = await userFacingWorkspaceCount(page);
    test.skip(
      count > 0,
      `data root has ${count} user workspaces — empty-state untestable`,
    );

    // Pre-create a workspace under the Playwright CI-fixture id —
    // these are server-filtered out of GET responses, so the wizard
    // STILL renders (workspaces.length === 0) but the second POST
    // attempt for the same id returns 409. This drives the wizard's
    // inline-error path end-to-end without polluting the data root
    // with user-visible workspaces that would break the empty-state
    // for the next test.
    const createRes = await page.request.post("/api/atlas/workspaces", {
      data: { workspace_id: workspace },
    });
    expect(createRes.ok()).toBe(true);

    await page.addInitScript(() => {
      window.localStorage.removeItem("atlas:active-workspace");
    });
    await page.goto("/");
    await expect(page.getByTestId("first-run-wizard")).toBeVisible({
      timeout: 15_000,
    });

    // Type the SAME id that we just pre-created. The server-side 409
    // surfaces back through `createWorkspace` → inline error.
    const input = page.getByTestId("first-run-wizard-input");
    await input.fill(workspace);
    await page.getByTestId("first-run-wizard-submit").click();

    const errorBlock = page.getByTestId("first-run-wizard-error");
    await expect(errorBlock).toBeVisible({ timeout: 10_000 });
    await expect(errorBlock).toContainText(/workspace already exists/i);
    // Critical: the wizard MUST NOT have redirected on failure.
    await expect(page).toHaveURL(/\/$/);
  });
});
