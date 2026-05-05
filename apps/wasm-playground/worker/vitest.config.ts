import { defineWorkersConfig } from "@cloudflare/vitest-pool-workers/config";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineWorkersConfig({
  test: {
    poolOptions: {
      workers: {
        // Run tests inside the actual Workers runtime via Miniflare, with the
        // exact bindings declared in wrangler.toml (Static Assets, AE, R2,
        // Durable Object). Tests therefore exercise the real binding shapes,
        // not hand-rolled mocks.
        wrangler: { configPath: "../wrangler.toml" },
        miniflare: {
          compatibilityDate: "2026-05-04",
          // Override the wrangler.toml `[assets] directory = "."` for tests:
          // pointing at the parent dir would also scan `worker/node_modules/`,
          // which contains a 78 MiB workerd.exe binary and trips miniflare's
          // 25 MiB asset-size guard. The fixture directory is tiny and
          // intentional — production uses the real `apps/wasm-playground/`
          // root via wrangler.toml unchanged.
          //
          // NOTE on routingConfig: vitest-pool-workers v0.5.41 hard-overwrites
          // `assets.routingConfig` to `{ has_user_worker: <bool> }` only,
          // stripping any `invoke_user_worker_ahead_of_assets` flag we'd set
          // here (see node_modules/@cloudflare/vitest-pool-workers/dist/
          // pool/index.mjs lines 345–352). That means SELF.fetch() in this
          // pool short-circuits matching static-asset paths to the asset
          // binding directly — the OPPOSITE of production behaviour, where
          // `experimental_serve_directly = false` forces the Worker to wrap
          // every response. The static-asset integration tests work around
          // this by calling worker.fetch() directly with a mock ASSETS
          // binding (see test/integration.test.ts). The production routing
          // invariant is verified post-deploy by playground-csp-check.sh.
          assets: {
            directory: path.resolve(__dirname, "test/__fixtures__/public"),
            binding: "ASSETS",
            assetConfig: {
              html_handling: "auto-trailing-slash",
              not_found_handling: "none",
            },
          },
        },
      },
    },
    // Coverage threshold per project standing (testing.md: 80% minimum).
    coverage: {
      provider: "istanbul",
      reporter: ["text", "html"],
      thresholds: {
        lines: 80,
        functions: 80,
        branches: 75,
        statements: 80,
      },
      include: ["src/**/*.ts"],
      exclude: ["src/**/*.test.ts", "test/**/*"],
    },
  },
});
