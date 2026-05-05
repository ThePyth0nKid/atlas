/**
 * Atlas Playground Worker — entry point.
 * V1.16 Welle C: Workers + Static Assets hosting.
 *
 * Routes:
 *   POST /csp-report   → CSP violation receiver (csp-receiver.ts)
 *   GET  *             → static asset binding (env.ASSETS.fetch), with
 *                        Worker-emitted security headers layered onto
 *                        every response (security-headers.ts).
 *
 * Scheduled:
 *   cron @ 03:00 UTC   → daily archive (cron-archive.ts)
 *
 * This Worker is the single audit point for security headers, the receiver
 * implementation, and the daily-archive heartbeat. Everything load-bearing
 * for Welle C composes through these handlers.
 */

import { applySecurityHeaders } from "./security-headers.js";
import { handleCspReport, type ReceiverEnv } from "./csp-receiver.js";
import { runDailyArchive, type ArchiveEnv } from "./cron-archive.js";

export { GlobalRateLimitDO } from "./rate-limit.js";

interface Env extends ReceiverEnv, ArchiveEnv {
  readonly ASSETS: Fetcher;
}

export default {
  async fetch(request: Request, env: Env, _ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);

    // CSP receiver — only accepts POST. Any other method falls through to
    // the static-asset handler (which will 404 since /csp-report is not a
    // static file), preserving REST-method semantics.
    if (url.pathname === "/csp-report" && request.method === "POST") {
      const response = await handleCspReport(request, env);
      return applySecurityHeaders(response, url.pathname);
    }

    // Static assets — the platform binding. Returns 404 for missing files
    // (because wrangler.toml `not_found_handling = "none"`), which we wrap
    // with security headers so even an attacker scanning paths sees the
    // hardening surface.
    const assetResponse = await env.ASSETS.fetch(request);
    return applySecurityHeaders(assetResponse, url.pathname);
  },

  async scheduled(
    _controller: ScheduledController,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<void> {
    ctx.waitUntil(runDailyArchive(env));
  },
} satisfies ExportedHandler<Env>;
