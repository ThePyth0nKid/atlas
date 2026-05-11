/**
 * V1.19 Welle 11 — Home page E2E.
 *
 * Pins the LiveVerifierPanel state-machine and final ✓ VALID rendering.
 * The panel fetches a golden trace + bundle via `/api/golden/*`, loads
 * the WASM verifier (`atlas-verify-wasm` package), and verifies in-
 * browser. This spec asserts the user-visible contract — the deeper
 * byte-equivalence between Rust-CLI and WASM verifier outputs is
 * covered by `cargo test -p atlas-trust-core`.
 *
 * Selectors are all `data-testid`-anchored per the Welle 11 frozen-
 * seam JSDoc in `LiveVerifierPanel.tsx`.
 */

import { test, expect } from "@playwright/test";

test.describe("Home — Live Verifier panel", () => {
  test("page renders Audit Readiness heading and verifier panel mounts", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(
      page.getByRole("heading", { level: 1, name: "Audit Readiness" }),
    ).toBeVisible();
    await expect(page.getByTestId("live-verifier-panel")).toBeVisible();
  });

  test("verifier reaches done state with ✓ VALID badge", async ({ page }) => {
    await page.goto("/");
    const badge = page.getByTestId("verifier-status-badge");
    // The badge starts in loading-wasm / fetching-trace / verifying. The
    // contract is: it MUST reach data-status="done" with data-valid="true"
    // for the golden bank-q1-2026 fixture within the timeout budget.
    await expect(badge).toHaveAttribute("data-status", "done", { timeout: 20_000 });
    await expect(badge).toHaveAttribute("data-valid", "true");
    await expect(badge).toHaveText("VALID");
  });

  test("verifier_version chip + trace meta appear after verification", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(page.getByTestId("verifier-version")).toBeVisible({
      timeout: 20_000,
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

  test("evidence list contains the expected V1.5+ check labels", async ({
    page,
  }) => {
    await page.goto("/");
    // Wait for done state via the version chip (proxy for "verification
    // completed and outcome rendered").
    await expect(page.getByTestId("verifier-version")).toBeVisible({
      timeout: 20_000,
    });
    const evidence = page.getByTestId("verifier-evidence");
    await expect(evidence).toBeVisible();

    // The golden bank fixture covers the V1.0 + V1.5 + V1.7 evidence
    // rows. Pin a representative subset — every label here MUST appear
    // for the verifier to be doing its job. The atlas-trust-core
    // verifier always emits these in this order.
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
