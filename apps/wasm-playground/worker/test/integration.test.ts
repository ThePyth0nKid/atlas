/**
 * Integration tests — V1.16 Welle C.
 *
 * Exercises the full Worker entry point through `SELF.fetch` from
 * @cloudflare/vitest-pool-workers. Bindings (ASSETS, CSP_REPORTS_AE,
 * CSP_REPORTS_R2, RATE_LIMIT_DO) are the real ones from wrangler.toml,
 * provided by miniflare. This catches integration issues that mocked-env
 * unit tests cannot:
 *   - Router (POST /csp-report vs static GET) wiring
 *   - applySecurityHeaders is layered onto every response (also onto 204
 *     receiver responses + onto static-asset 404s)
 *   - Cache-Control class is correct per path (html/immutable/receiver)
 *   - Cron handler executes runDailyArchive and writes a heartbeat to R2
 *
 * The tests use unique CF-Connecting-IP values per request to avoid sharing
 * a /64 prefix-bucket with other tests (rate-limit DO state persists across
 * tests within the same run).
 */

import { describe, expect, it } from "vitest";
import {
  SELF,
  env,
  createExecutionContext,
  waitOnExecutionContext,
} from "cloudflare:test";
import worker from "../src/index.js";

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

const TEST_ORIGIN = "http://localhost:8787";
let ipCounter = 0x2001_0db8_0001_0000n;

/** Returns a unique IPv6 address each call (different /64 prefix every time). */
function nextTestIp(): string {
  ipCounter += 0x0001_0000_0000_0000n;
  const hex = ipCounter.toString(16).padStart(16, "0");
  return `${hex.slice(0, 4)}:${hex.slice(4, 8)}:${hex.slice(8, 12)}:${hex.slice(12, 16)}::1`;
}

const VALID_REPORT_BODY = JSON.stringify({
  "csp-report": {
    "violated-directive": "script-src",
    "blocked-uri": "https://evil.example/x.js",
    "document-uri": `${TEST_ORIGIN}/`,
  },
});

function makeReportRequest(overrides: {
  method?: string;
  origin?: string | null;
  contentType?: string | null;
  body?: string;
} = {}): Request {
  const headers = new Headers();
  if (overrides.origin !== null) {
    headers.set("Origin", overrides.origin ?? TEST_ORIGIN);
  }
  if (overrides.contentType !== null) {
    headers.set("Content-Type", overrides.contentType ?? "application/csp-report");
  }
  headers.set("CF-Connecting-IP", nextTestIp());
  return new Request(`${TEST_ORIGIN}/csp-report`, {
    method: overrides.method ?? "POST",
    headers,
    body: overrides.body ?? VALID_REPORT_BODY,
  });
}

function expectBaseSecurityHeaders(response: Response): void {
  // The exact values are unit-tested in security-headers.test.ts; here we
  // only assert the layering invariant — every response carries them.
  expect(response.headers.get("Content-Security-Policy")).toBeTruthy();
  expect(response.headers.get("Strict-Transport-Security")).toBeTruthy();
  expect(response.headers.get("Cross-Origin-Opener-Policy")).toBe("same-origin");
  expect(response.headers.get("Cross-Origin-Embedder-Policy")).toBe("require-corp");
  expect(response.headers.get("X-Content-Type-Options")).toBe("nosniff");
  expect(response.headers.get("Referrer-Policy")).toBe("no-referrer");
}

// ─────────────────────────────────────────────────────────────────────────────
// Router: POST /csp-report
// ─────────────────────────────────────────────────────────────────────────────

describe("Worker router — POST /csp-report", () => {
  it("returns 204 with security headers for a valid report", async () => {
    const r = await SELF.fetch(makeReportRequest());
    expect(r.status).toBe(204);
    expectBaseSecurityHeaders(r);
    expect(r.headers.get("Cache-Control")).toBe("no-store");
  });

  it("returns 204 (silent-204 invariant) for a wrong-origin report", async () => {
    const r = await SELF.fetch(
      makeReportRequest({ origin: "https://attacker.example" }),
    );
    expect(r.status).toBe(204);
    expectBaseSecurityHeaders(r);
  });

  it("returns 204 for a wrong content-type", async () => {
    const r = await SELF.fetch(makeReportRequest({ contentType: "text/plain" }));
    expect(r.status).toBe(204);
    expectBaseSecurityHeaders(r);
  });

  it("returns 204 for malformed JSON", async () => {
    const r = await SELF.fetch(makeReportRequest({ body: "{not-json" }));
    expect(r.status).toBe(204);
    expectBaseSecurityHeaders(r);
  });

  it("falls through to ASSETS on non-POST /csp-report (GET → 404 with headers)", async () => {
    const r = await SELF.fetch(
      new Request(`${TEST_ORIGIN}/csp-report`, {
        method: "GET",
        headers: { "CF-Connecting-IP": nextTestIp() },
      }),
    );
    // /csp-report is not a static file → ASSETS returns 404
    expect(r.status).toBe(404);
    expectBaseSecurityHeaders(r);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Router: static asset paths
//
// These tests invoke the Worker's fetch handler directly with a mock ASSETS
// binding instead of going through SELF.fetch. Reason: in production, the
// `experimental_serve_directly = false` flag in wrangler.toml [assets] forces
// the Worker to run BEFORE the asset match, so applySecurityHeaders wraps
// every static-asset response. miniflare honours that flag at runtime, but
// @cloudflare/vitest-pool-workers (v0.5.41) hard-overwrites the
// `assets.routingConfig` to `{ has_user_worker: ... }` only — stripping the
// `invoke_user_worker_ahead_of_assets` flag before miniflare sees it (see
// node_modules/@cloudflare/vitest-pool-workers/dist/pool/index.mjs lines
// 345–352). So SELF.fetch() in this pool would short-circuit static-asset
// requests directly to the asset binding and bypass the Worker entirely,
// which is the OPPOSITE of production behaviour.
//
// Calling worker.fetch directly with a mock ASSETS binding tests the
// production-correct path: the Worker IS invoked, and applySecurityHeaders
// IS layered onto whatever the asset binding returns. The production
// `experimental_serve_directly = false` invariant is verified post-deploy
// via the playground-csp-check.sh validator (Task #8).
// ─────────────────────────────────────────────────────────────────────────────

describe("Worker router — static assets via ASSETS binding", () => {
  /** Build a mock env that satisfies the Worker's Env interface. */
  function makeStaticAssetEnv(
    assetResponse: Response,
  ): Parameters<typeof worker.fetch>[1] {
    return {
      ASSETS: {
        fetch: async (_req: Request) => assetResponse,
      } as unknown as Fetcher,
      // Receiver/cron bindings are unused on static-asset paths but the
      // Worker's Env type requires them. We pass through the real test env's
      // bindings to keep the type contract honest without mocking them out.
      EXPECTED_ORIGIN: env.EXPECTED_ORIGIN,
      ENVIRONMENT: env.ENVIRONMENT,
      CSP_REPORTS_AE: env.CSP_REPORTS_AE,
      CSP_REPORTS_R2: env.CSP_REPORTS_R2,
      RATE_LIMIT_DO: env.RATE_LIMIT_DO,
    } as Parameters<typeof worker.fetch>[1];
  }

  function makeCtx(): ExecutionContext {
    return createExecutionContext();
  }

  it("layers security headers + html cache-control on / requests", async () => {
    const assetResponse = new Response("<!doctype html><html></html>", {
      status: 200,
      headers: { "Content-Type": "text/html; charset=utf-8" },
    });
    const r = await worker.fetch(
      new Request(`${TEST_ORIGIN}/`),
      makeStaticAssetEnv(assetResponse),
      makeCtx(),
    );
    expect(r.status).toBe(200);
    expectBaseSecurityHeaders(r);
    expect(r.headers.get("Cache-Control")).toBe("no-cache, must-revalidate");
  });

  it("layers immutable cache-control on /pkg/*.wasm", async () => {
    const assetResponse = new Response(new Uint8Array([0x00, 0x61, 0x73, 0x6d]), {
      status: 200,
      headers: { "Content-Type": "application/wasm" },
    });
    const r = await worker.fetch(
      new Request(`${TEST_ORIGIN}/pkg/atlas_verify_wasm_bg.wasm`),
      makeStaticAssetEnv(assetResponse),
      makeCtx(),
    );
    expect(r.status).toBe(200);
    expectBaseSecurityHeaders(r);
    expect(r.headers.get("Cache-Control")).toBe("public, max-age=31536000, immutable");
  });

  it("layers immutable cache-control on /app.js", async () => {
    const assetResponse = new Response("export const x = 1;", {
      status: 200,
      headers: { "Content-Type": "application/javascript" },
    });
    const r = await worker.fetch(
      new Request(`${TEST_ORIGIN}/app.js`),
      makeStaticAssetEnv(assetResponse),
      makeCtx(),
    );
    expect(r.status).toBe(200);
    expectBaseSecurityHeaders(r);
    expect(r.headers.get("Cache-Control")).toBe("public, max-age=31536000, immutable");
  });

  it("layers security headers onto a 404 from the asset binding", async () => {
    // Production behaviour: when wrangler.toml `not_found_handling = "none"`
    // the asset binding returns 404, which the Worker still wraps with full
    // security headers. This is critical — even unauthenticated path scans
    // must see the hardening surface.
    const assetResponse = new Response("not found", { status: 404 });
    const r = await worker.fetch(
      new Request(`${TEST_ORIGIN}/does-not-exist`),
      makeStaticAssetEnv(assetResponse),
      makeCtx(),
    );
    expect(r.status).toBe(404);
    expectBaseSecurityHeaders(r);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Cron: scheduled handler writes a daily heartbeat to R2
// ─────────────────────────────────────────────────────────────────────────────

describe("Worker scheduled — daily archive heartbeat", () => {
  // We invoke worker.scheduled directly (not via SELF.scheduled, which the
  // vitest-pool-workers RPC bridge does not expose). createExecutionContext +
  // waitOnExecutionContext ensure ctx.waitUntil-scheduled work is awaited
  // before assertions run.

  it("writes a heartbeat object to R2 with kind=heartbeat", async () => {
    const ctrl = {
      cron: "0 3 * * *",
      noRetry: () => undefined,
      scheduledTime: Date.now(),
    } as unknown as ScheduledController;

    const ctx = createExecutionContext();
    await worker.scheduled(ctrl, env, ctx);
    await waitOnExecutionContext(ctx);

    const today = new Date().toISOString().slice(0, 10);
    const key = `heartbeat/${today}.json`;
    const obj = await env.CSP_REPORTS_R2.get(key);

    expect(obj).not.toBeNull();
    if (obj === null) return;
    expect(obj.customMetadata?.kind).toBe("heartbeat");
    expect(obj.customMetadata?.welle).toBe("v1.16-welle-c");

    const body = await obj.text();
    const parsed = JSON.parse(body) as {
      heartbeat: boolean;
      welle: string;
      written_at: string;
    };
    expect(parsed.heartbeat).toBe(true);
    expect(parsed.welle).toBe("v1.16-welle-c");
    expect(parsed.written_at).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  });

  it("is idempotent on the same UTC day (re-run overwrites, no duplicate keys)", async () => {
    const ctrl = {
      cron: "0 3 * * *",
      noRetry: () => undefined,
      scheduledTime: Date.now(),
    } as unknown as ScheduledController;

    const ctx1 = createExecutionContext();
    await worker.scheduled(ctrl, env, ctx1);
    await waitOnExecutionContext(ctx1);

    const ctx2 = createExecutionContext();
    await worker.scheduled(ctrl, env, ctx2);
    await waitOnExecutionContext(ctx2);

    const today = new Date().toISOString().slice(0, 10);
    const list = await env.CSP_REPORTS_R2.list({ prefix: `heartbeat/${today}` });
    expect(list.objects).toHaveLength(1);
  });
});
