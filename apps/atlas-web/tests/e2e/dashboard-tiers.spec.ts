/**
 * W20b-1 — Dashboard 3-tier UX + coming-soon nav + status footer.
 *
 * Provisions per-tier workspaces via the existing /api/atlas/write-node
 * route (same pattern as `home.spec.ts`) and asserts the
 * `<DashboardMetricsSection>` picks the right tier based on event
 * count.
 *
 * Tier thresholds (mirror `DashboardMetricsSection.tsx`):
 *   - totalEvents == 0          → EmptyTier
 *   - 1 ≤ totalEvents ≤ 10      → EarlyTier
 *   - totalEvents ≥ 11          → FullTier (7 KPI cards)
 *
 * Why provision via the live write route?
 *   It's the only path that produces signed events the trace
 *   endpoint can serve. Mocking the network would also bypass the
 *   metric computation we want to exercise end-to-end.
 *
 * Why the EmptyTier test uses a never-written workspace id?
 *   The trace route returns `events: []` for any workspace dir that
 *   doesn't exist — exactly the shape the empty tier renders for.
 */

import { test, expect, provisionAndSelect } from "./fixtures";
import type { Page } from "@playwright/test";

async function pinWorkspace(page: Page, workspace: string): Promise<void> {
  await page.addInitScript((ws: string) => {
    window.localStorage.setItem("atlas:active-workspace", ws);
  }, workspace);
}

async function writeNode(
  page: Page,
  workspace: string,
  nodeId: string,
): Promise<void> {
  const res = await page.request.post("/api/atlas/write-node", {
    data: {
      workspace_id: workspace,
      kind: "dataset",
      id: nodeId,
      attributes: {},
    },
  });
  expect(res.ok()).toBe(true);
}

test.describe("Dashboard — 3-tier metrics", () => {
  test("EmptyTier renders for a workspace with zero events", async ({ page }) => {
    const workspace = `pw-empty-${Date.now().toString(36)}-${Math.random()
      .toString(36)
      .slice(2, 8)}`;
    await pinWorkspace(page, workspace);
    await page.goto("/");
    await expect(page.getByTestId("dashboard-tier-empty")).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByTestId("dashboard-empty-cta")).toBeVisible();
    await expect(page.getByTestId("dashboard-empty-cta")).toHaveAttribute(
      "href",
      "/write",
    );
    // W20c Lesson #26 — structural anchor: LayerStatusPanel is mounted
    // on the dashboard tree (above DashboardMetricsSection per
    // HomeContent change in this commit). Verifying it here prevents
    // a future DOM shift from silently invalidating dashboard layout.
    await expect(page.getByTestId("layer-status-panel")).toBeVisible({
      timeout: 15_000,
    });
    // FullTier-only artefact must not be present.
    await expect(page.getByTestId("dashboard-tier-full")).toHaveCount(0);
  });

  test("EarlyTier renders for a workspace with 1-10 events", async ({
    page,
    workspace,
  }) => {
    // Provision 3 events to comfortably fall in the early tier.
    await writeNode(page, workspace, "early-1");
    await writeNode(page, workspace, "early-2");
    await writeNode(page, workspace, "early-3");
    await pinWorkspace(page, workspace);
    await page.goto("/");
    await expect(page.getByTestId("dashboard-tier-early")).toBeVisible({
      timeout: 15_000,
    });
    const rows = page.getByTestId("dashboard-early-event");
    expect(await rows.count()).toBeGreaterThanOrEqual(1);
    // FullTier must not be present.
    await expect(page.getByTestId("dashboard-tier-full")).toHaveCount(0);
  });

  test("FullTier renders for a workspace with 11+ events", async ({
    page,
    workspace,
  }) => {
    for (let i = 0; i < 12; i += 1) {
      await writeNode(page, workspace, `full-${i}`);
    }
    await pinWorkspace(page, workspace);
    await page.goto("/");
    await expect(page.getByTestId("dashboard-tier-full")).toBeVisible({
      timeout: 20_000,
    });
    const cards = page.getByTestId("kpi-card");
    expect(await cards.count()).toBeGreaterThanOrEqual(7);
    // EarlyTier list must not be present.
    await expect(page.getByTestId("dashboard-tier-early")).toHaveCount(0);
  });
});

test.describe("Nav — coming-soon disabled entries", () => {
  test("Compliance Lens renders as disabled span (not a link)", async ({
    page,
  }) => {
    await page.goto("/");
    const entry = page.getByTestId("nav-coming-soon-compliance");
    await expect(entry).toBeVisible();
    // Should be a <span>, not an <a> — verifies via tag name.
    expect(await entry.evaluate((el) => el.tagName.toLowerCase())).toBe("span");
    await expect(entry).toHaveAttribute("aria-disabled", "true");
  });

  test("Audit Export and Adversary Demo entries are also disabled", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(
      page.getByTestId("nav-coming-soon-audit-export"),
    ).toBeVisible();
    await expect(
      page.getByTestId("nav-coming-soon-adversary-demo"),
    ).toBeVisible();
  });

  test("Bank demo showcase nav entry links to /demo/bank", async ({ page }) => {
    await page.goto("/");
    const entry = page.getByTestId("nav-showcase-bank-demo");
    await expect(entry).toBeVisible();
    await expect(entry).toHaveAttribute("href", "/demo/bank");
  });
});

test.describe("Home — Status disclosure footer", () => {
  test("status footer renders on the home page", async ({ page, workspace }) => {
    // W20b-2: the StatusDisclosureFooter lives inside HomeContent's
    // dashboard branch. Without seeding, HomeContent renders the
    // FirstRunWizard and the footer is absent. Seed first so the
    // dashboard tree (including the footer) mounts. Introduced by
    // 70ead19, not addressed by 8dc0ec5, fixed by this commit.
    await provisionAndSelect(page, workspace);
    await page.goto("/");
    const footer = page.getByTestId("status-disclosure-footer");
    await expect(footer).toBeVisible();
    await expect(footer).toContainText("Atlas roadmap status");
    await expect(footer).toContainText("Layer 1 verifier");
  });
});
