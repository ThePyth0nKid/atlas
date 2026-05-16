/**
 * W20c — /settings route e2e.
 *
 * Validates:
 *   - Page renders four panels (workspace list, signer status, supply-chain
 *     pins, coming-soon)
 *   - Rename happy path: typing a new id + Submit renames the workspace
 *   - Rename invalid path: regex failure surfaces an inline error
 *   - Delete typed-confirmation: submit button stays disabled until
 *     the user types the exact id; final click removes the row
 *   - Supply-chain pins panel lists 11 rows
 *   - Signer-status panel renders three status rows
 *   - Last-workspace gate: with 1 workspace, the Delete button is
 *     disabled and a hint is rendered
 */

import {
  test,
  expect,
  provisionAndSelect,
  provisionAndSelectMany,
} from "./fixtures";

test.describe("Settings — initial render", () => {
  test("renders the four-panel layout when a workspace exists", async ({
    page,
    workspace,
  }) => {
    await provisionAndSelect(page, workspace);
    await page.goto("/settings");

    await expect(page.getByTestId("settings-content")).toBeVisible();
    await expect(page.getByTestId("settings-workspace-list")).toBeVisible();
    await expect(page.getByTestId("settings-signer-status")).toBeVisible();
    await expect(page.getByTestId("settings-supply-chain-pins")).toBeVisible();
    await expect(page.getByTestId("settings-coming-soon")).toBeVisible();
  });

  test("workspace row shows the active marker on the selected workspace", async ({
    page,
    workspace,
  }) => {
    await provisionAndSelect(page, workspace);
    await page.goto("/settings");
    const row = page.locator(
      `[data-testid="settings-workspace-row"][data-workspace-id="${workspace}"]`,
    );
    await expect(row).toBeVisible();
    await expect(
      row.getByTestId("settings-workspace-active-marker"),
    ).toBeVisible();
  });

  test("delete button is disabled when only one workspace exists", async ({
    page,
    workspace,
  }) => {
    await provisionAndSelect(page, workspace);
    await page.goto("/settings");
    // The workspaces context may have other workspaces visible from
    // parallel tests in the same data dir; this test makes sense only
    // when the list shows exactly one user-facing workspace. Skip
    // otherwise — race with parallel suite.
    const rows = page.getByTestId("settings-workspace-row");
    const count = await rows.count();
    test.skip(
      count !== 1,
      `parallel test left ${count} workspaces visible; last-workspace gate untestable`,
    );
    const delBtn = page.getByTestId("settings-delete-button");
    await expect(delBtn).toBeDisabled();
    await expect(
      page.getByTestId("settings-delete-disabled-hint"),
    ).toBeVisible();
  });
});

test.describe("Settings — rename", () => {
  test("renames a workspace via the dialog (happy path)", async ({
    page,
    workspace,
  }) => {
    const ids = await provisionAndSelectMany(page, workspace, 2);
    await page.goto("/settings");

    const renameTarget = ids[1];
    const newId = `${renameTarget}-renamed`;

    const row = page.locator(
      `[data-testid="settings-workspace-row"][data-workspace-id="${renameTarget}"]`,
    );
    await row.getByTestId("settings-rename-button").click();

    await expect(page.getByTestId("settings-rename-dialog")).toBeVisible();
    await page.getByTestId("settings-rename-input").fill(newId);
    await page.getByTestId("settings-rename-submit").click();

    // After success the dialog closes and the new id appears in the list.
    await expect(page.getByTestId("settings-rename-dialog")).not.toBeVisible({
      timeout: 10_000,
    });
    await expect(
      page.locator(`[data-workspace-id="${newId}"]`).first(),
    ).toBeVisible({ timeout: 10_000 });
    // The old id should no longer be present.
    await expect(
      page.locator(`[data-workspace-id="${renameTarget}"]`),
    ).toHaveCount(0);
  });

  test("rename surfaces an inline error on regex failure", async ({
    page,
    workspace,
  }) => {
    const ids = await provisionAndSelectMany(page, workspace, 2);
    await page.goto("/settings");

    const row = page.locator(
      `[data-testid="settings-workspace-row"][data-workspace-id="${ids[1]}"]`,
    );
    await row.getByTestId("settings-rename-button").click();

    await expect(page.getByTestId("settings-rename-dialog")).toBeVisible();
    await page.getByTestId("settings-rename-input").fill("bad space");
    await page.getByTestId("settings-rename-submit").click();

    await expect(page.getByTestId("settings-rename-error")).toBeVisible();
    // Dialog must still be open — the submit failed, retryable.
    await expect(page.getByTestId("settings-rename-dialog")).toBeVisible();
  });
});

test.describe("Settings — delete with typed confirmation", () => {
  test("submit button is disabled until the exact id is typed", async ({
    page,
    workspace,
  }) => {
    const ids = await provisionAndSelectMany(page, workspace, 2);
    await page.goto("/settings");

    const target = ids[1];
    const row = page.locator(
      `[data-testid="settings-workspace-row"][data-workspace-id="${target}"]`,
    );
    await row.getByTestId("settings-delete-button").click();

    await expect(page.getByTestId("settings-delete-dialog")).toBeVisible();
    // Submit is disabled with empty input.
    await expect(page.getByTestId("settings-delete-submit")).toBeDisabled();
    // Type a partial match — still disabled.
    await page.getByTestId("settings-delete-input").fill(target.slice(0, -2));
    await expect(page.getByTestId("settings-delete-submit")).toBeDisabled();
    // Exact match — enabled.
    await page.getByTestId("settings-delete-input").fill(target);
    await expect(page.getByTestId("settings-delete-submit")).toBeEnabled();
  });

  test("delete removes the workspace from the list", async ({
    page,
    workspace,
  }) => {
    const ids = await provisionAndSelectMany(page, workspace, 2);
    await page.goto("/settings");

    const target = ids[1];
    const row = page.locator(
      `[data-testid="settings-workspace-row"][data-workspace-id="${target}"]`,
    );
    await row.getByTestId("settings-delete-button").click();
    await expect(page.getByTestId("settings-delete-dialog")).toBeVisible();
    await page.getByTestId("settings-delete-input").fill(target);
    await page.getByTestId("settings-delete-submit").click();

    await expect(page.getByTestId("settings-delete-dialog")).not.toBeVisible({
      timeout: 10_000,
    });
    await expect(
      page.locator(`[data-workspace-id="${target}"]`),
    ).toHaveCount(0);
  });
});

test.describe("Settings — supply-chain pins panel", () => {
  test("lists all 11 supply-chain pin rows", async ({ page, workspace }) => {
    await provisionAndSelect(page, workspace);
    await page.goto("/settings");
    // Wait for the loading to settle.
    await expect(page.getByTestId("settings-supply-chain-pins")).toBeVisible();
    const rows = page.getByTestId("settings-supply-chain-pin-row");
    await expect(rows).toHaveCount(11, { timeout: 10_000 });
  });
});

test.describe("Settings — signer status panel", () => {
  test("renders three rows (signer/embedder/backend)", async ({
    page,
    workspace,
  }) => {
    await provisionAndSelect(page, workspace);
    await page.goto("/settings");
    await expect(page.getByTestId("settings-status-signer")).toBeVisible();
    await expect(page.getByTestId("settings-status-embedder")).toBeVisible();
    await expect(page.getByTestId("settings-status-backend")).toBeVisible();
  });
});
