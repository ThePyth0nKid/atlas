/**
 * Type augmentation for `import { env } from "cloudflare:test"`.
 *
 * vitest-pool-workers reads bindings from wrangler.toml at test boot, but
 * does NOT auto-derive their TypeScript types — `env` is therefore typed as
 * the empty `ProvidedEnv` interface unless we extend it here. Mismatches
 * between this declaration and wrangler.toml will surface as test-time
 * `undefined` accesses, so keep them in sync.
 */

declare module "cloudflare:test" {
  interface ProvidedEnv {
    readonly EXPECTED_ORIGIN: string;
    readonly ENVIRONMENT: string;
    readonly ASSETS: Fetcher;
    readonly CSP_REPORTS_AE: AnalyticsEngineDataset;
    readonly CSP_REPORTS_R2: R2Bucket;
    readonly RATE_LIMIT_DO: DurableObjectNamespace;
  }
}
