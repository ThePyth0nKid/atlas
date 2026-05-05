/**
 * Unit tests for security-headers.ts (V1.16 Welle C).
 *
 * Asserts the exact header invariants documented in the V1.16 plan §1.3 and
 * scope-d Bullet 6. If a future edit weakens any of these (e.g. adds
 * 'unsafe-eval' to script-src, drops 'wasm-unsafe-eval', removes COOP/COEP),
 * the test must fail loud.
 */

import { describe, expect, it } from "vitest";
import {
  BASE_SECURITY_HEADERS,
  CACHE_CONTROL,
  applySecurityHeaders,
  classifyPath,
} from "../src/security-headers.js";

describe("BASE_SECURITY_HEADERS", () => {
  const csp = BASE_SECURITY_HEADERS["Content-Security-Policy"]!;

  it("declares default-src 'none' (deny-by-default)", () => {
    expect(csp).toContain("default-src 'none'");
  });

  it("script-src allows 'self' and 'wasm-unsafe-eval' but not 'unsafe-eval' or 'unsafe-inline'", () => {
    expect(csp).toMatch(/script-src 'self' 'wasm-unsafe-eval'(?:[ ;]|$)/);
    expect(csp).not.toMatch(/script-src[^;]*'unsafe-eval'(?:[ ;]|$)/);
    expect(csp).not.toMatch(/script-src[^;]*'unsafe-inline'(?:[ ;]|$)/);
  });

  it("enforces strict Trusted Types (require + 'none' policy)", () => {
    expect(csp).toContain("require-trusted-types-for 'script'");
    expect(csp).toContain("trusted-types 'none'");
  });

  it("blocks frames, forms, base-uri", () => {
    expect(csp).toContain("frame-ancestors 'none'");
    expect(csp).toContain("form-action 'none'");
    expect(csp).toContain("base-uri 'none'");
  });

  it("declares both report-uri (legacy) and report-to (modern)", () => {
    expect(csp).toContain("report-uri /csp-report");
    expect(csp).toContain("report-to csp-endpoint");
  });

  it("declares Reporting-Endpoints with the same group", () => {
    expect(BASE_SECURITY_HEADERS["Reporting-Endpoints"]).toContain(
      'csp-endpoint="/csp-report"',
    );
  });

  it("declares legacy Report-To JSON header", () => {
    const legacy = BASE_SECURITY_HEADERS["Report-To"]!;
    const parsed = JSON.parse(legacy) as {
      group: string;
      max_age: number;
      endpoints: { url: string }[];
    };
    expect(parsed.group).toBe("csp-endpoint");
    expect(parsed.max_age).toBeGreaterThan(0);
    expect(parsed.endpoints[0]?.url).toBe("/csp-report");
  });

  it("HSTS includes max-age >= 1 year, includeSubDomains, preload", () => {
    const hsts = BASE_SECURITY_HEADERS["Strict-Transport-Security"]!;
    const match = hsts.match(/max-age=(\d+)/);
    expect(match).not.toBeNull();
    expect(Number(match![1])).toBeGreaterThanOrEqual(31536000);
    expect(hsts).toContain("includeSubDomains");
    expect(hsts).toContain("preload");
  });

  it("has COOP same-origin and COEP require-corp (Spectre cross-process defence)", () => {
    expect(BASE_SECURITY_HEADERS["Cross-Origin-Opener-Policy"]).toBe("same-origin");
    expect(BASE_SECURITY_HEADERS["Cross-Origin-Embedder-Policy"]).toBe("require-corp");
  });

  it("CORP is same-origin", () => {
    expect(BASE_SECURITY_HEADERS["Cross-Origin-Resource-Policy"]).toBe("same-origin");
  });

  it("X-Content-Type-Options is nosniff", () => {
    expect(BASE_SECURITY_HEADERS["X-Content-Type-Options"]).toBe("nosniff");
  });

  it("Referrer-Policy is no-referrer", () => {
    expect(BASE_SECURITY_HEADERS["Referrer-Policy"]).toBe("no-referrer");
  });

  it("X-Frame-Options DENY (legacy redundancy with frame-ancestors)", () => {
    expect(BASE_SECURITY_HEADERS["X-Frame-Options"]).toBe("DENY");
  });

  it("Permissions-Policy disables all dangerous browser features", () => {
    const pp = BASE_SECURITY_HEADERS["Permissions-Policy"]!;
    for (const feature of [
      "camera",
      "microphone",
      "geolocation",
      "payment",
      "usb",
      "midi",
      "publickey-credentials-get",
    ]) {
      expect(pp).toContain(`${feature}=()`);
    }
  });
});

describe("classifyPath", () => {
  it("classifies / and *.html as html", () => {
    expect(classifyPath("/")).toBe("html");
    expect(classifyPath("/index.html")).toBe("html");
    expect(classifyPath("/foo.html")).toBe("html");
  });

  it("classifies /csp-report as receiver", () => {
    expect(classifyPath("/csp-report")).toBe("receiver");
  });

  it("classifies everything else as immutable", () => {
    expect(classifyPath("/app.js")).toBe("immutable");
    expect(classifyPath("/pkg/atlas_verify_wasm_bg.wasm")).toBe("immutable");
    expect(classifyPath("/style.css")).toBe("immutable");
  });
});

describe("applySecurityHeaders", () => {
  it("adds every base header to the response", () => {
    const original = new Response("hi", { status: 200 });
    const wrapped = applySecurityHeaders(original, "/index.html");
    for (const name of Object.keys(BASE_SECURITY_HEADERS)) {
      expect(wrapped.headers.get(name)).toBe(BASE_SECURITY_HEADERS[name]);
    }
  });

  it("sets Cache-Control: no-cache, must-revalidate on html", () => {
    const wrapped = applySecurityHeaders(new Response("ok"), "/index.html");
    expect(wrapped.headers.get("Cache-Control")).toBe(CACHE_CONTROL.html);
  });

  it("sets Cache-Control: immutable on app.js", () => {
    const wrapped = applySecurityHeaders(new Response("ok"), "/app.js");
    expect(wrapped.headers.get("Cache-Control")).toBe(CACHE_CONTROL.immutable);
  });

  it("sets Cache-Control: no-store on /csp-report", () => {
    const wrapped = applySecurityHeaders(new Response(null, { status: 204 }), "/csp-report");
    expect(wrapped.headers.get("Cache-Control")).toBe(CACHE_CONTROL.receiver);
  });

  it("preserves status and statusText", () => {
    const wrapped = applySecurityHeaders(
      new Response("not found", { status: 404, statusText: "Not Found" }),
      "/nope",
    );
    expect(wrapped.status).toBe(404);
    expect(wrapped.statusText).toBe("Not Found");
  });

  it("overrides any platform-default Cache-Control with the class value", () => {
    const original = new Response("ok", {
      headers: { "Cache-Control": "public, max-age=300" },
    });
    const wrapped = applySecurityHeaders(original, "/index.html");
    expect(wrapped.headers.get("Cache-Control")).toBe(CACHE_CONTROL.html);
  });

  it("returns a NEW Response (does not mutate original)", () => {
    const original = new Response("ok");
    const wrapped = applySecurityHeaders(original, "/");
    expect(wrapped).not.toBe(original);
    expect(original.headers.get("Content-Security-Policy")).toBeNull();
  });
});
