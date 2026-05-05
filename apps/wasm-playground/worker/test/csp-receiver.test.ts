/**
 * Unit tests for csp-receiver.ts (V1.16 Welle C).
 *
 * Each describe block maps to one numbered step in handleCspReport's pipeline
 * (1. origin, 2. content-type, 3-4. body cap, 5. JSON parse, 6. schema,
 * 7. rate-limit, 8. normalise, 9. AE write). The receiver must silent-204
 * every validation failure (no oracle for attackers — security review) AND
 * emit a categorised internal log line (M2 — operator visibility).
 *
 * Method enforcement is the router's job (index.ts only routes POST to the
 * receiver), so there is no method-level test here on purpose: the receiver
 * cannot be reached with a non-POST request without the router being broken,
 * and a defence-in-depth 405 here would break the silent-204 invariant.
 *
 * Env is mocked with a deterministic stub so tests are isolated from the
 * Durable Object state and the AE binding. The integration test
 * (integration.test.ts) exercises the receiver against the real bindings via
 * SELF.fetch.
 */

import { describe, expect, it } from "vitest";
import { handleCspReport, type ReceiverEnv } from "../src/csp-receiver.js";

// ─────────────────────────────────────────────────────────────────────────────
// Mock env helpers
// ─────────────────────────────────────────────────────────────────────────────

interface MockOptions {
  expectedOrigin?: string;
  rateLimitResponse?: { allowed: boolean; count?: number; reason?: string };
  rateLimitThrows?: boolean;
  aeThrows?: boolean;
}

interface CapturedDataPoint {
  indexes?: readonly string[];
  blobs?: readonly string[];
  doubles?: readonly number[];
}

interface MockEnvHandle {
  env: ReceiverEnv;
  writes: CapturedDataPoint[];
  doCalls: { url: string }[];
}

function makeMockEnv(opts: MockOptions = {}): MockEnvHandle {
  const writes: CapturedDataPoint[] = [];
  const doCalls: { url: string }[] = [];
  const rateLimitResponse = opts.rateLimitResponse ?? { allowed: true, count: 1 };

  const stub = {
    fetch: async (input: string | Request): Promise<Response> => {
      const url = typeof input === "string" ? input : input.url;
      doCalls.push({ url });
      if (opts.rateLimitThrows) throw new Error("DO unavailable");
      return new Response(JSON.stringify(rateLimitResponse), {
        headers: { "content-type": "application/json" },
      });
    },
  } as unknown as DurableObjectStub;

  const namespace = {
    idFromName: (_name: string) => ({ toString: () => "id" }) as unknown as DurableObjectId,
    get: (_id: DurableObjectId) => stub,
  } as unknown as DurableObjectNamespace;

  const ae = {
    writeDataPoint: (p: CapturedDataPoint) => {
      if (opts.aeThrows) throw new Error("AE write failed");
      writes.push(p);
    },
  } as unknown as AnalyticsEngineDataset;

  return {
    env: {
      EXPECTED_ORIGIN: opts.expectedOrigin ?? "https://example.test",
      ENVIRONMENT: "test",
      CSP_REPORTS_AE: ae,
      RATE_LIMIT_DO: namespace,
    },
    writes,
    doCalls,
  };
}

interface RequestOptions {
  method?: string;
  origin?: string | null;
  contentType?: string | null;
  body?: string;
  contentLength?: number | null;
  ip?: string;
  userAgent?: string;
}

function makeRequest(opts: RequestOptions): Request {
  const headers = new Headers();
  if (opts.origin !== null && opts.origin !== undefined) {
    headers.set("Origin", opts.origin);
  }
  if (opts.contentType !== null && opts.contentType !== undefined) {
    headers.set("Content-Type", opts.contentType);
  }
  if (opts.contentLength !== null && opts.contentLength !== undefined) {
    headers.set("Content-Length", String(opts.contentLength));
  }
  if (opts.ip) headers.set("CF-Connecting-IP", opts.ip);
  if (opts.userAgent) headers.set("User-Agent", opts.userAgent);
  return new Request("http://example.test/csp-report", {
    method: opts.method ?? "POST",
    headers,
    body: opts.body,
  });
}

const VALID_LEGACY_REPORT = JSON.stringify({
  "csp-report": {
    "violated-directive": "script-src",
    "blocked-uri": "https://evil.example/x.js",
    "document-uri": "https://example.test/",
    "source-file": "https://example.test/app.js",
    "line-number": 42,
    "column-number": 7,
    "original-policy": "default-src 'none'; script-src 'self'",
  },
});

const VALID_MODERN_REPORT = JSON.stringify([
  {
    type: "csp-violation",
    body: {
      "violated-directive": "script-src",
      "blocked-uri": "https://evil.example/x.js",
      "document-uri": "https://example.test/",
    },
  },
]);

const VALID_DIRECT_REPORT = JSON.stringify({
  "violated-directive": "script-src",
  "blocked-uri": "https://evil.example/x.js",
  "document-uri": "https://example.test/",
});

const ORIGIN = "https://example.test";

// ─────────────────────────────────────────────────────────────────────────────
// 1. Origin (CSRF anchor — security review H1)
// ─────────────────────────────────────────────────────────────────────────────

describe("handleCspReport — Origin check (security H1)", () => {
  it("silent-204 when Origin header is missing", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: null,
        contentType: "application/csp-report",
        body: VALID_LEGACY_REPORT,
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(r.body).toBeNull();
    expect(writes).toHaveLength(0);
  });

  it("silent-204 when Origin is the literal string 'null' (sandbox/file://)", async () => {
    // Critical: 'null' must be explicitly rejected. If we only checked for
    // missing-Origin, a sandboxed iframe attacker could submit reports.
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: "null",
        contentType: "application/csp-report",
        body: VALID_LEGACY_REPORT,
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("silent-204 when Origin is a different scheme/host", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: "https://attacker.example",
        contentType: "application/csp-report",
        body: VALID_LEGACY_REPORT,
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("silent-204 on near-match (subdomain) — strict equality only", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: "https://evil.example.test", // suffix attack
        contentType: "application/csp-report",
        body: VALID_LEGACY_REPORT,
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("accepts when Origin matches EXPECTED_ORIGIN exactly", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: ORIGIN,
        contentType: "application/csp-report",
        body: VALID_LEGACY_REPORT,
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2. Content-Type allow-list
// ─────────────────────────────────────────────────────────────────────────────

describe("handleCspReport — Content-Type allow-list", () => {
  it("rejects text/plain", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "text/plain", body: VALID_LEGACY_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("rejects text/html (smuggling attempt)", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "text/html", body: VALID_LEGACY_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("rejects application/x-www-form-urlencoded (CSRF-style submission)", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: ORIGIN,
        contentType: "application/x-www-form-urlencoded",
        body: VALID_LEGACY_REPORT,
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("rejects when Content-Type header is absent", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: null, body: VALID_LEGACY_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("accepts application/csp-report (legacy)", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: VALID_LEGACY_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
  });

  it("accepts application/json", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/json", body: VALID_LEGACY_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
  });

  it("accepts application/reports+json (Reporting API)", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/reports+json", body: VALID_MODERN_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
  });

  it("accepts application/csp-report; charset=utf-8 (suffix tolerance)", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: ORIGIN,
        contentType: "application/csp-report; charset=utf-8",
        body: VALID_LEGACY_REPORT,
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
  });

  it("accepts uppercase content-type (case-insensitive match)", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: ORIGIN,
        contentType: "APPLICATION/CSP-REPORT",
        body: VALID_LEGACY_REPORT,
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3-4. Body size cap
// ─────────────────────────────────────────────────────────────────────────────

describe("handleCspReport — body-size cap", () => {
  it("rejects when Content-Length declares > 64 KB (pre-check)", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: ORIGIN,
        contentType: "application/csp-report",
        body: VALID_LEGACY_REPORT,
        contentLength: 65537,
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("rejects when body length exceeds 64 KB despite Content-Length lying about it (post-check)", async () => {
    // Build a body > 64 KB with a small declared Content-Length. The
    // post-check (read.length > MAX_BODY_BYTES) catches this.
    const bigField = "x".repeat(70000);
    const fatBody = JSON.stringify({ "csp-report": { "violated-directive": bigField } });
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({
        origin: ORIGIN,
        contentType: "application/csp-report",
        body: fatBody,
        contentLength: 100, // lying
      }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("accepts a typical-sized report body (well under the 64 KB cap)", async () => {
    // Note: a true at-the-boundary test is constrained by the JSON parser's
    // own per-string cap (DEFAULT_LIMITS.maxStringLength = 8192), which the
    // body cap (65 536) sits ABOVE — so the boundary is actually enforced by
    // json-safe-parse, not the body-size pre-check. We test a realistic
    // typical-size payload here and rely on json-safe-parse.test.ts for the
    // per-string limit and on the rejection tests above for the body cap.
    const body = JSON.stringify({
      "csp-report": {
        "violated-directive": "script-src",
        "blocked-uri": "https://evil.example/x.js",
        "document-uri": "https://example.test/",
        "original-policy": "default-src 'none'; script-src 'self'",
      },
    });
    expect(body.length).toBeLessThan(65536);
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5. JSON safety (depth bomb — security review H2)
// ─────────────────────────────────────────────────────────────────────────────

describe("handleCspReport — JSON safety (security H2)", () => {
  it("silent-204 on a deep-nest JSON bomb", async () => {
    let s = '"v"';
    for (let i = 0; i < 50; i++) s = `{"k":${s}}`;
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/json", body: s }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("silent-204 on malformed JSON", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/json", body: "{not: valid" }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("silent-204 on empty body", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/json", body: "" }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 6. Schema validation
// ─────────────────────────────────────────────────────────────────────────────

describe("handleCspReport — schema extraction", () => {
  it("silent-204 on JSON null", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/json", body: "null" }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("silent-204 on a JSON primitive", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/json", body: '"hi"' }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("silent-204 on an empty object (no recognised keys)", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/json", body: "{}" }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("accepts legacy {'csp-report': {...}} shape", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: VALID_LEGACY_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
    expect(writes[0]?.indexes).toEqual(["script-src"]);
  });

  it("accepts modern Reporting API array shape", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/reports+json", body: VALID_MODERN_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
    expect(writes[0]?.indexes).toEqual(["script-src"]);
  });

  it("accepts direct top-level shape", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/json", body: VALID_DIRECT_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(1);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 7. Rate-limit (security review H3 + M6)
// ─────────────────────────────────────────────────────────────────────────────

describe("handleCspReport — rate-limit", () => {
  it("silent-204 with reason rate_limited_per_ip when DO returns per-IP rejection", async () => {
    const { env, writes } = makeMockEnv({
      expectedOrigin: ORIGIN,
      rateLimitResponse: { allowed: false, reason: "per_ip" },
    });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: VALID_LEGACY_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("silent-204 (DO unavailable, fail-closed) when DO throws", async () => {
    const { env, writes } = makeMockEnv({
      expectedOrigin: ORIGIN,
      rateLimitThrows: true,
    });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: VALID_LEGACY_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });

  it("calls DO with the prefix key derived from CF-Connecting-IP", async () => {
    const { env, doCalls } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({
        origin: ORIGIN,
        contentType: "application/csp-report",
        body: VALID_LEGACY_REPORT,
        ip: "203.0.113.42",
      }),
      env,
    );
    // First DO call = per-IP, second = global
    expect(doCalls.length).toBeGreaterThanOrEqual(1);
    expect(doCalls[0]?.url).toContain("key=203.0.113.42%2F32");
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 8. Normalisation (ANSI strip, URL → origin only, basename, length truncation)
// ─────────────────────────────────────────────────────────────────────────────

describe("handleCspReport — normalisation", () => {
  it("strips ANSI escape sequences from violatedDirective", async () => {
    const reportWithAnsi = JSON.stringify({
      "csp-report": {
        "violated-directive": "\x1b[31mscript-src\x1b[0m",
        "blocked-uri": "https://evil.example/x.js",
        "document-uri": "https://example.test/",
      },
    });
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: reportWithAnsi }),
      env,
    );
    expect(writes).toHaveLength(1);
    expect(writes[0]?.indexes?.[0]).toBe("script-src");
    expect(writes[0]?.blobs?.[0]).toBe("script-src");
  });

  it("normalises blocked-uri to origin only (drops path/query/fragment)", async () => {
    const report = JSON.stringify({
      "csp-report": {
        "violated-directive": "script-src",
        "blocked-uri": "https://evil.example/path/to/file.js?q=1#frag",
        "document-uri": "https://example.test/page?a=b",
      },
    });
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: report }),
      env,
    );
    expect(writes[0]?.blobs?.[1]).toBe("https://evil.example");
    expect(writes[0]?.blobs?.[2]).toBe("https://example.test");
  });

  it("preserves CSP keyword tokens like 'inline'/'eval' verbatim (not URLs)", async () => {
    const report = JSON.stringify({
      "csp-report": {
        "violated-directive": "script-src",
        "blocked-uri": "inline",
        "document-uri": "https://example.test/",
      },
    });
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: report }),
      env,
    );
    expect(writes[0]?.blobs?.[1]).toBe("inline");
  });

  it("reduces source-file URL to its basename only", async () => {
    const report = JSON.stringify({
      "csp-report": {
        "violated-directive": "script-src",
        "blocked-uri": "inline",
        "document-uri": "https://example.test/",
        "source-file": "https://example.test/static/v2/app.js",
      },
    });
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: report }),
      env,
    );
    expect(writes[0]?.blobs?.[3]).toBe("app.js");
  });

  it("truncates excessive originalPolicy to MAX_ORIGINAL_POLICY_LENGTH (2048)", async () => {
    const longPolicy = "a".repeat(5000);
    const report = JSON.stringify({
      "csp-report": {
        "violated-directive": "script-src",
        "blocked-uri": "inline",
        "document-uri": "https://example.test/",
        "original-policy": longPolicy,
      },
    });
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: report }),
      env,
    );
    expect(writes[0]?.blobs?.[5]?.length).toBe(2048);
  });

  it("captures User-Agent from request headers, not from report body", async () => {
    const report = JSON.stringify({
      "csp-report": {
        "violated-directive": "script-src",
        "blocked-uri": "inline",
        "document-uri": "https://example.test/",
      },
    });
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({
        origin: ORIGIN,
        contentType: "application/csp-report",
        body: report,
        userAgent: "TestUA/1.0",
      }),
      env,
    );
    expect(writes[0]?.blobs?.[4]).toBe("TestUA/1.0");
  });

  it("converts negative line/column numbers to 0", async () => {
    const report = JSON.stringify({
      "csp-report": {
        "violated-directive": "script-src",
        "blocked-uri": "inline",
        "document-uri": "https://example.test/",
        "line-number": -1,
        "column-number": -999,
      },
    });
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: report }),
      env,
    );
    expect(writes[0]?.doubles).toEqual([0, 0]);
  });

  it("floors fractional line/column numbers to integers", async () => {
    const report = JSON.stringify({
      "csp-report": {
        "violated-directive": "script-src",
        "blocked-uri": "inline",
        "document-uri": "https://example.test/",
        "line-number": 42.7,
        "column-number": 8.9,
      },
    });
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: report }),
      env,
    );
    expect(writes[0]?.doubles).toEqual([42, 8]);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 9. AE writeDataPoint — failure handling
// ─────────────────────────────────────────────────────────────────────────────

describe("handleCspReport — AE write failure", () => {
  it("silent-204 (no oracle) when AE writeDataPoint throws", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN, aeThrows: true });
    const r = await handleCspReport(
      makeRequest({ origin: ORIGIN, contentType: "application/csp-report", body: VALID_LEGACY_REPORT }),
      env,
    );
    expect(r.status).toBe(204);
    expect(writes).toHaveLength(0);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// AE datapoint shape
// ─────────────────────────────────────────────────────────────────────────────

describe("handleCspReport — AE datapoint shape", () => {
  it("writes index = violatedDirective and 7 blobs + 2 doubles", async () => {
    const { env, writes } = makeMockEnv({ expectedOrigin: ORIGIN });
    await handleCspReport(
      makeRequest({
        origin: ORIGIN,
        contentType: "application/csp-report",
        body: VALID_LEGACY_REPORT,
        userAgent: "Mozilla/5.0",
      }),
      env,
    );
    expect(writes).toHaveLength(1);
    const dp = writes[0]!;
    expect(dp.indexes).toEqual(["script-src"]);
    expect(dp.blobs).toHaveLength(7);
    expect(dp.doubles).toHaveLength(2);
    // blobs ordering: directive, blockedUri, documentUri, sourceFile, userAgent,
    //                 originalPolicy, receivedAt
    expect(dp.blobs?.[0]).toBe("script-src");
    expect(dp.blobs?.[1]).toBe("https://evil.example");
    expect(dp.blobs?.[2]).toBe("https://example.test");
    expect(dp.blobs?.[4]).toBe("Mozilla/5.0");
    // receivedAt is an ISO timestamp
    expect(dp.blobs?.[6]).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/);
  });
});
