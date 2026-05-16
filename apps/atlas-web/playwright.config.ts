/**
 * V1.19 Welle 11 — Playwright configuration for atlas-web UI E2E + a11y.
 *
 * Production-build server (next start -p 3001) keeps the test surface
 * byte-identical to the deployed playground (no HMR noise, no dev-only
 * routes). Test workers each get a fresh ATLAS_DATA_DIR via the
 * fixtures module so workspace writes do not bleed across tests.
 *
 * Browser-matrix: Chromium + Firefox. Webkit deliberately excluded —
 * Cloudflare-Workers-hosted playground has no engine-specific path,
 * and the additional ~300 MB binary + ~5 min runtime would not buy
 * meaningful regression coverage at the v1.0 stage. Add as a nightly
 * lane in a future welle if a Webkit-specific regression is observed.
 *
 * Anti-drift contract: all assertions in tests/e2e/*.spec.ts MUST use
 * data-testid selectors documented in the component JSDoc. Tailwind
 * class assertions and prose-match assertions (except WCAG-relevant
 * label text) are forbidden.
 */

import { defineConfig, devices } from "@playwright/test";

// W20c — port override via env so worktrees on Windows hosts (where
// the default 3001 may be claimed by another service per known issue
// #114) can run the suite against a free port. CI leaves the env var
// unset → falls back to 3001 (the canonical pinned port).
const PORT = process.env.ATLAS_WEB_PORT ?? "3001";
const BASE_URL = `http://localhost:${PORT}`;

export default defineConfig({
  testDir: "./tests/e2e",
  // Tests run in parallel across files. Per-file is sequential — the
  // signer-binary spawn is the bottleneck; parallelising within a file
  // would race on the same workspace dir.
  fullyParallel: false,
  workers: process.env.CI ? 2 : undefined,
  // CI: retry once on failure to absorb the documented WASM-load flake.
  // The retry surfaces in artifact uploads via traces (on-first-retry).
  retries: process.env.CI ? 1 : 0,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI
    ? [["list"], ["html", { open: "never", outputFolder: "playwright-report" }]]
    : "list",
  use: {
    baseURL: BASE_URL,
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    // Default per-test timeout 30s — sufficient for atlas-signer spawn
    // (~1-2s) plus full WASM verifier round-trip. Increase per-test
    // only if a specific case warrants it.
    actionTimeout: 10_000,
    navigationTimeout: 30_000,
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "firefox",
      use: { ...devices["Desktop Firefox"] },
    },
  ],
  webServer: {
    // Production build, not dev. Mirrors playground.atlas-trust.dev
    // shape (next start, no HMR client). CI runs `pnpm build` in a
    // prior step so this just starts the server.
    command: `pnpm exec next start -p ${PORT}`,
    url: BASE_URL,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    // ATLAS_DEV_MASTER_SEED=1 mirrors the smoke.ts V1.12-Scope-B2 gate
    // — the per-tenant signer subcommand refuses to use the dev seed
    // unless this var is set. CI sets it in the workflow step too;
    // doubling here ensures local `pnpm e2e:playwright` works without
    // shell-level export.
    env: {
      ATLAS_DEV_MASTER_SEED: "1",
      // W20c — enables the `x-atlas-test-force-*` header overrides on
      // /api/atlas/system/health so Playwright specs can force a
      // specific signer/embedder/backend value without mutating the
      // server's actual env. Production deploys do NOT set this var,
      // so attacker-controlled headers are ignored by construction.
      ATLAS_E2E_TEST_HOOKS: "1",
    },
    stdout: "pipe",
    stderr: "pipe",
  },
});
