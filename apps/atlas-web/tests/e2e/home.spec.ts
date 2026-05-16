/**
 * V1.19 Welle 11 — Home page E2E.
 *
 * W20a update: the LiveVerifierPanel now consumes
 * `/api/atlas/trace?workspace=<id>` + `/api/atlas/pubkey-bundle?workspace=<id>`,
 * driven by the active workspace from `WorkspaceContext`. Each test
 * provisions a dedicated workspace via the write endpoint, sets
 * localStorage to point the WorkspaceProvider at it, and asserts the
 * verifier reaches `done` over that workspace's signed event.
 *
 * Why per-test workspaces:
 *   - The Playwright fixture in `fixtures.ts` already emits unique
 *     `pw-w*-*` workspace ids per test for the write spec; we reuse
 *     it here so home tests stop relying on any pre-existing
 *     `ws-mcp-default` folder. CI starts with an empty data dir.
 *   - The W20a server-side filter strips `pw-w*-*` entries from
 *     `/api/atlas/workspaces`, so the provider's `workspaces` list
 *     does NOT include the test workspace. BUT the provider honours
 *     a localStorage selection that exists outside the listed set
 *     when we drive it manually — for these tests we treat the
 *     panel as a workspace-driven primitive: we set the active
 *     workspace via localStorage, and the LiveVerifierPanel issues
 *     /api/atlas/trace?workspace=<id> regardless of selector
 *     visibility.
 *
 * Selectors are all `data-testid`-anchored per the Welle 11 frozen-
 * seam JSDoc in `LiveVerifierPanel.tsx` (which W20a explicitly
 * extended, never broke).
 *
 * For the single-event Genesis trace, the V1.0 evidence rows are
 * always present; `parent-links` + `dag-tips` also pass (empty-parent
 * + single-tip is a valid DAG shape).
 */

import { test, expect, provisionAndSelect } from "./fixtures";

test.describe("Home — Live Verifier panel", () => {
  test("page renders Audit Readiness heading and verifier panel mounts", async ({
    page,
    workspace,
  }) => {
    // W20b-2: HomeContent branches on workspaces.length === 0 →
    // FirstRunWizard. Seed a workspace so the dashboard tree (with
    // the LiveVerifierPanel) renders. Introduced by 70ead19, not
    // addressed by 8dc0ec5, fixed by this commit.
    await provisionAndSelect(page, workspace);
    await page.goto("/");
    await expect(
      page.getByRole("heading", { level: 1, name: "Audit Readiness" }),
    ).toBeVisible();
    await expect(page.getByTestId("live-verifier-panel")).toBeVisible();
  });

  test("verifier reaches done state with ✓ VALID badge", async ({
    page,
    workspace,
  }) => {
    await provisionAndSelect(page, workspace);
    await page.goto("/");
    const badge = page.getByTestId("verifier-status-badge");
    // The badge starts in waiting-workspace / fetching-trace / verifying.
    // The contract is: it MUST reach data-status="done" with
    // data-valid="true" for the provisioned Genesis trace within
    // the timeout budget.
    await expect(badge).toHaveAttribute("data-status", "done", { timeout: 30_000 });
    await expect(badge).toHaveAttribute("data-valid", "true");
    await expect(badge).toHaveText("VALID");
  });

  test("verifier_version chip + trace meta appear after verification", async ({
    page,
    workspace,
  }) => {
    await provisionAndSelect(page, workspace);
    await page.goto("/");
    await expect(page.getByTestId("verifier-version")).toBeVisible({
      timeout: 30_000,
    });
    // The chip text is the crate semver string like "atlas-trust-core/<semver>"
    // (e.g. "atlas-trust-core/1.0.0"). Anchor on the structural prefix, not
    // the exact version, to avoid brittle assertion drift on every cargo
    // version bump.
    await expect(page.getByTestId("verifier-version")).toHaveText(
      /^atlas-trust-core\//,
    );

    const meta = page.getByTestId("verifier-trace-meta");
    await expect(meta).toBeVisible();
    await expect(meta).toContainText("Workspace");
    await expect(meta).toContainText("events");
  });

  test("evidence list contains the V1.0 + structural check labels", async ({
    page,
    workspace,
  }) => {
    await provisionAndSelect(page, workspace);
    await page.goto("/");
    // Wait for done state via the version chip (proxy for "verification
    // completed and outcome rendered").
    await expect(page.getByTestId("verifier-version")).toBeVisible({
      timeout: 30_000,
    });
    const evidence = page.getByTestId("verifier-evidence");
    await expect(evidence).toBeVisible();

    // W20a: the Genesis trace covers the V1.0 evidence rows + the
    // structural shape rows. Every label below MUST appear for the
    // verifier to be doing its job. The atlas-trust-core verifier
    // emits these in this order regardless of event count.
    for (const checkLabel of [
      "schema-version",
      "pubkey-bundle-hash",
      "event-hashes",
      "event-signatures",
      "parent-links",
      "dag-tips",
    ]) {
      await expect(
        page.getByTestId(`verifier-evidence-${checkLabel}`),
      ).toBeVisible();
    }
  });
});
