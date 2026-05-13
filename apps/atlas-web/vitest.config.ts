/**
 * V2-β Welle 12 — Vitest config for atlas-web unit tests.
 *
 * Scope: server-side route handlers + `_lib/*` helpers. Playwright
 * remains the E2E layer; this config does NOT run browser tests.
 *
 * Coverage threshold matches the standing testing.md guidance:
 * 80% lines/functions/statements, 75% branches.
 */

import { defineConfig } from "vitest/config";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
      // Point `@atlas/bridge` at the workspace package's *source* so
      // vitest doesn't need the compiled `dist/` to exist. The Next.js
      // app's runtime resolution still uses the package's `main`/
      // `exports` entry; this alias is vitest-only.
      "@atlas/bridge": path.resolve(__dirname, "../../packages/atlas-bridge/src/index.ts"),
    },
  },
  test: {
    // Server route handlers use Node-native `Request` / `Response`
    // (Next.js's App Router primitives). Vitest's `node` env
    // delivers these via undici.
    environment: "node",
    include: ["src/**/*.test.ts"],
    exclude: ["tests/e2e/**", "scripts/**", "node_modules/**"],
    coverage: {
      provider: "v8",
      reporter: ["text", "html"],
      // W12 scope only — `write-node/route.ts` (V1.19) is covered by
      // tsx smoke scripts + playwright suite, not vitest.
      include: [
        "src/app/api/atlas/_lib/**/*.ts",
        "src/app/api/atlas/entities/**/*.ts",
        "src/app/api/atlas/related/**/*.ts",
        "src/app/api/atlas/timeline/**/*.ts",
        "src/app/api/atlas/query/**/*.ts",
        "src/app/api/atlas/audit/**/*.ts",
        "src/app/api/atlas/passport/**/*.ts",
      ],
      exclude: ["src/app/api/atlas/**/*.test.ts"],
      thresholds: {
        lines: 80,
        functions: 80,
        branches: 75,
        statements: 80,
      },
    },
  },
});
