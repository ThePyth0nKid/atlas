/**
 * V1.19 Welle 11 — Accessibility E2E.
 *
 * Pins WCAG 2.1 Level AA compliance for the two user-facing surfaces:
 *   - `/` (Audit Readiness dashboard + LiveVerifierPanel)
 *   - `/write` (DeployerNotice + WriteNodeForm), initial + post-success
 *
 * Uses `@axe-core/playwright`. Any violation found here is documented
 * as a Welle-12+ carry-forward (the Welle 11 scope is the gate, not
 * fixes — see plan-doc §"Risks").
 *
 * Keyboard-nav assertion checks the WriteNodeForm tab order is logical
 * (workspace → kind → id → attributes → submit). Atlas's deployer-
 * security-boundary notice means screen-reader users need to reach
 * the submit button without trap; this test pins that.
 */

import AxeBuilder from "@axe-core/playwright";
import { test, expect, provisionAndSelect } from "./fixtures";

test.describe("A11y — WCAG 2.1 AA", () => {
  // W20b-2 — split decision (Option B): the home `/` route renders one
  // of two trees depending on whether any workspace exists. Both
  // states are reachable by real users; auditing only one would lie
  // about coverage. The original test is renamed to make the seeded
  // path explicit, and a sibling test pins the wizard path. Introduced
  // by 70ead19, not addressed by 8dc0ec5, fixed by this commit.
  test("home page (dashboard state) passes axe WCAG 2.1 AA on initial render", async ({
    page,
    workspace,
  }) => {
    await provisionAndSelect(page, workspace);
    await page.goto("/");
    // Wait for the verifier panel to mount so we audit the full
    // rendered tree, not the loading skeleton.
    await expect(page.getByTestId("live-verifier-panel")).toBeVisible();
    // W20c Lesson #26 — anchor assertion. LayerStatusPanel mounts on
    // the dashboard tree in this commit; ensure it's part of the
    // audited DOM so a11y violations on the new panel are caught.
    await expect(page.getByTestId("layer-status-panel")).toBeVisible();

    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();

    expect.soft(results.violations, axeMessage(results.violations)).toEqual([]);
  });

  test("settings page passes axe WCAG 2.1 AA on initial render", async ({
    page,
    workspace,
  }) => {
    // W20c — new /settings route. Seeded so the workspace-list panel
    // is non-empty and all four panels render.
    await provisionAndSelect(page, workspace);
    await page.goto("/settings");
    await expect(page.getByTestId("settings-content")).toBeVisible();
    // Wait for the data-bound panels to settle so axe audits the
    // ready DOM, not the loading skeleton.
    await expect(page.getByTestId("settings-workspace-list")).toBeVisible();
    await expect(page.getByTestId("settings-supply-chain-pins")).toBeVisible();

    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();

    expect.soft(results.violations, axeMessage(results.violations)).toEqual([]);
  });

  test("home page (first-run wizard state) passes axe WCAG 2.1 AA", async ({
    page,
  }) => {
    // No seed: when the user-facing workspaces list is empty the
    // FirstRunWizard replaces the dashboard tree. Skip when the data
    // root already has user-facing workspaces (same gate as
    // first-run-wizard.spec.ts) so the assertion is honest about what
    // it audits.
    //
    // Race: parallel first-run-wizard tests create user-facing
    // workspaces mid-run; the pre-goto probe can read 0 but the
    // render-time HomeContent fetch reads >0 and swaps to the
    // dashboard tree. We probe BEFORE goto for the obvious skip case,
    // and re-detect post-goto by polling for the wizard root with a
    // short budget; if it never appears within 3s and a dashboard
    // tier marker does, skip rather than fail — both signals are
    // legitimate; the wizard-axe assertion only makes sense in the
    // empty-state.
    const res = await page.request.get("/api/atlas/workspaces");
    const count = res.ok()
      ? ((await res.json()) as { workspaces: string[] }).workspaces.length
      : -1;
    test.skip(
      count > 0,
      `data root has ${count} user workspaces — wizard state untestable`,
    );
    await page.addInitScript(() => {
      window.localStorage.removeItem("atlas:active-workspace");
    });
    await page.goto("/");
    // Post-goto race detection: if HomeContent picks dashboard
    // because a sibling test landed a workspace between probe and
    // navigation, skip without failing.
    const wizardVisible = await page
      .getByTestId("first-run-wizard")
      .isVisible({ timeout: 3_000 })
      .catch(() => false);
    if (!wizardVisible) {
      const dashboardVisible = await page
        .getByTestId("dashboard-tier-empty")
        .or(page.getByTestId("dashboard-tier-early"))
        .or(page.getByTestId("dashboard-tier-full"))
        .first()
        .isVisible()
        .catch(() => false);
      test.skip(
        dashboardVisible,
        "race with first-run-wizard.spec.ts: workspace appeared between probe and goto",
      );
    }
    await expect(page.getByTestId("first-run-wizard")).toBeVisible({
      timeout: 15_000,
    });

    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();

    expect.soft(results.violations, axeMessage(results.violations)).toEqual([]);
  });

  test("write page passes axe WCAG 2.1 AA on initial render", async ({
    page,
  }) => {
    await page.goto("/write");
    await expect(page.getByTestId("write-node-form")).toBeVisible();

    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();

    expect.soft(results.violations, axeMessage(results.violations)).toEqual([]);
  });

  test("write page passes axe WCAG 2.1 AA after successful submit", async ({
    page,
    workspace,
  }) => {
    await page.goto("/write");
    await page.getByTestId("write-workspace-id").fill(workspace);
    await page.getByTestId("write-node-id").fill("a11y-post-submit");
    await page.getByTestId("write-submit").click();

    await expect(page.getByTestId("write-success-card")).toBeVisible({
      timeout: 30_000,
    });

    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
      .analyze();

    expect.soft(results.violations, axeMessage(results.violations)).toEqual([]);
  });

  test("write form keyboard tab order is workspace → kind → id → attributes → submit", async ({
    page,
  }) => {
    await page.goto("/write");
    // Focus the first form input directly, then walk Tab and confirm
    // each focused element's data-testid.
    await page.getByTestId("write-workspace-id").focus();
    await expect(page.getByTestId("write-workspace-id")).toBeFocused();

    await page.keyboard.press("Tab");
    await expect(page.getByTestId("write-node-kind")).toBeFocused();

    await page.keyboard.press("Tab");
    await expect(page.getByTestId("write-node-id")).toBeFocused();

    await page.keyboard.press("Tab");
    await expect(page.getByTestId("write-attributes")).toBeFocused();

    await page.keyboard.press("Tab");
    await expect(page.getByTestId("write-submit")).toBeFocused();
  });
});

function axeMessage(
  violations: Array<{ id: string; impact?: string | null; nodes: unknown[] }>,
): string {
  if (violations.length === 0) return "no violations";
  return (
    `Found ${violations.length} axe WCAG-2.1-AA violation(s):\n` +
    violations
      .map(
        (v) =>
          `  - ${v.id} (impact=${v.impact ?? "unknown"}, ${v.nodes.length} node(s))`,
      )
      .join("\n")
  );
}
