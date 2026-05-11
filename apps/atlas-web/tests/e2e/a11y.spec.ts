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
import { test, expect } from "./fixtures";

test.describe("A11y — WCAG 2.1 AA", () => {
  test("home page passes axe WCAG 2.1 AA on initial render", async ({ page }) => {
    await page.goto("/");
    // Wait for the verifier panel to mount so we audit the full
    // rendered tree, not the loading skeleton.
    await expect(page.getByTestId("live-verifier-panel")).toBeVisible();

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
