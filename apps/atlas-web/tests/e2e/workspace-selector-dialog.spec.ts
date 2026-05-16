/**
 * W20b-2 — Workspace-selector "+ New" dialog E2E.
 *
 * Covers the four user-visible contracts of the `<WorkspaceSelector>`'s
 * dialog surface introduced in W20b-2 (and the close-listener race
 * fix from the fix-commit, finding #3):
 *
 *   1. "+ New" button opens the dialog, Cancel closes it AND React
 *      state (`data-open`) re-syncs.
 *   2. "+ New" button opens the dialog, Escape closes it AND React
 *      state re-syncs. This is the regression pin for the
 *      `useEffect`-with-empty-deps close-listener race (the fix
 *      moves listener registration to a ref-callback).
 *   3. Dialog submit creates the workspace, closes the dialog, and
 *      the new id surfaces in the selector dropdown.
 *   4. Dialog shows inline error on duplicate workspace_id.
 *
 * The fixture's `workspace` id is the per-test unique id, used for
 * both the seed (provision an existing non-empty selector) and any
 * duplicate-id collision pre-creation.
 *
 * Frozen testids (mirrors `<WorkspaceSelector>` JSDoc):
 *   workspace-selector, workspace-selector-current,
 *   workspace-selector-new-button, workspace-selector-new-dialog,
 *   workspace-selector-new-input, workspace-selector-new-submit,
 *   workspace-selector-new-cancel, workspace-selector-new-error.
 */

import { test, expect } from "./fixtures";
import type { Page } from "@playwright/test";

/**
 * Provision the per-test workspace and pin it as the active selection
 * so the selector renders in `ready` state with a current workspace
 * BEFORE we open the "+ New" dialog. Reused across tests.
 */
async function seedAndPin(page: Page, workspace: string): Promise<void> {
  const writeRes = await page.request.post("/api/atlas/write-node", {
    data: {
      workspace_id: workspace,
      kind: "dataset",
      id: "dialog-spec-genesis",
      attributes: {},
    },
  });
  expect(writeRes.ok()).toBe(true);
  await page.addInitScript((ws: string) => {
    window.localStorage.setItem("atlas:active-workspace", ws);
  }, workspace);
}

test.describe("Workspace selector — + New dialog", () => {
  test("+ New dialog opens on button click and closes on cancel", async ({
    page,
    workspace,
  }) => {
    await seedAndPin(page, workspace);
    await page.goto("/");

    const selector = page.getByTestId("workspace-selector");
    await expect(selector).toBeVisible({ timeout: 15_000 });
    await expect(selector).toHaveAttribute("data-state", "ready", {
      timeout: 10_000,
    });

    const dialog = page.getByTestId("workspace-selector-new-dialog");
    // Dialog exists in the DOM but is not open yet.
    await expect(dialog).toHaveAttribute("data-open", "false");

    await page.getByTestId("workspace-selector-new-button").click();
    await expect(dialog).toHaveAttribute("data-open", "true");
    await expect(page.getByTestId("workspace-selector-new-input")).toBeVisible();

    await page.getByTestId("workspace-selector-new-cancel").click();
    await expect(dialog).toHaveAttribute("data-open", "false");
  });

  test("+ New dialog opens and closes on Escape key", async ({
    page,
    workspace,
  }) => {
    // Regression pin for the W20b-2 fix-commit's ref-callback fix.
    // Before that fix the native dialog closed on Escape, but the
    // React `dialogOpen` state stayed true (close-listener never
    // attached because the useEffect ran while loading=true and the
    // <dialog> element was unmounted). After the fix the
    // `data-open` attribute re-syncs to "false" on Escape.
    await seedAndPin(page, workspace);
    await page.goto("/");

    const selector = page.getByTestId("workspace-selector");
    await expect(selector).toHaveAttribute("data-state", "ready", {
      timeout: 10_000,
    });

    const dialog = page.getByTestId("workspace-selector-new-dialog");
    await page.getByTestId("workspace-selector-new-button").click();
    await expect(dialog).toHaveAttribute("data-open", "true");

    await page.keyboard.press("Escape");
    // Native dialog closed AND React state synced — this is the bug
    // the fix-commit closed.
    await expect(dialog).toHaveAttribute("data-open", "false", {
      timeout: 5_000,
    });
  });

  test("+ New dialog creates workspace and closes on success", async ({
    page,
    workspace,
  }) => {
    await seedAndPin(page, workspace);
    await page.goto("/");

    const selector = page.getByTestId("workspace-selector");
    await expect(selector).toHaveAttribute("data-state", "ready", {
      timeout: 10_000,
    });

    await page.getByTestId("workspace-selector-new-button").click();
    const dialog = page.getByTestId("workspace-selector-new-dialog");
    await expect(dialog).toHaveAttribute("data-open", "true");

    const newId = `dlg${Date.now().toString(36)}${Math.random()
      .toString(36)
      .slice(2, 6)}`;
    await page.getByTestId("workspace-selector-new-input").fill(newId);
    await page.getByTestId("workspace-selector-new-submit").click();

    // Success closes the dialog…
    await expect(dialog).toHaveAttribute("data-open", "false", {
      timeout: 10_000,
    });
    // …and the new workspace is auto-selected as current.
    const current = page.getByTestId("workspace-selector-current");
    await expect(current).toContainText(newId, { timeout: 5_000 });
  });

  test("+ New dialog shows error inline on duplicate workspace_id", async ({
    page,
    workspace,
  }) => {
    await seedAndPin(page, workspace);
    await page.goto("/");

    const selector = page.getByTestId("workspace-selector");
    await expect(selector).toHaveAttribute("data-state", "ready", {
      timeout: 10_000,
    });

    await page.getByTestId("workspace-selector-new-button").click();
    const dialog = page.getByTestId("workspace-selector-new-dialog");
    await expect(dialog).toHaveAttribute("data-open", "true");

    // Type the SAME id the fixture pre-provisioned — POST returns 409.
    await page.getByTestId("workspace-selector-new-input").fill(workspace);
    await page.getByTestId("workspace-selector-new-submit").click();

    const errorBlock = page.getByTestId("workspace-selector-new-error");
    await expect(errorBlock).toBeVisible({ timeout: 10_000 });
    await expect(errorBlock).toContainText(/workspace already exists/i);
    // Dialog stays open on failure — user can correct the id.
    await expect(dialog).toHaveAttribute("data-open", "true");
  });
});
