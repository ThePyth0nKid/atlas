/**
 * Worker-emitted security headers — V1.16 Welle C.
 *
 * The CSP value is intentionally identical to the page-bytes meta-tag CSP
 * declared in `apps/wasm-playground/index.html`, plus the two header-only
 * additions that meta-tag delivery cannot carry:
 *   1. `report-to csp-endpoint`        — modern Reporting API directive
 *   2. `Reporting-Endpoints` header    — required to declare the endpoint group
 * These two could not be in Welle B because they require HTTP-header
 * delivery; Welle C delivers them.
 *
 * Anti-drift: tools/playground-csp-check.sh asserts the page-bytes meta-tag
 * CSP and the Worker-emitted CSP stay token-identical (modulo the two
 * header-only additions named above).
 *
 * Trust-property: Worker emits these headers on every response. A static-asset
 * served directly by the Cloudflare runtime never bypasses Worker code in
 * Workers + Static Assets — the Worker can rewrite/wrap any asset response.
 */

const CSP =
  "default-src 'none'; " +
  "script-src 'self' 'wasm-unsafe-eval'; " +
  "style-src 'self' 'unsafe-inline'; " +
  "connect-src 'self'; " +
  "form-action 'none'; " +
  "frame-ancestors 'none'; " +
  "base-uri 'none'; " +
  "require-trusted-types-for 'script'; " +
  "trusted-types 'none'; " +
  "report-uri /csp-report; " +
  "report-to csp-endpoint";

const REPORTING_ENDPOINTS = 'csp-endpoint="/csp-report"';

const REPORT_TO_LEGACY = JSON.stringify({
  group: "csp-endpoint",
  max_age: 10886400,
  endpoints: [{ url: "/csp-report" }],
});

/**
 * Permissions-Policy — disable every browser feature this app does NOT use.
 * Defence-in-depth: a future regression that adds a sink (e.g. a button that
 * calls navigator.geolocation) will fail at the browser boundary, not at
 * code-review time.
 */
const PERMISSIONS_POLICY = [
  "accelerometer=()",
  "ambient-light-sensor=()",
  "autoplay=()",
  "battery=()",
  "camera=()",
  "display-capture=()",
  "document-domain=()",
  "encrypted-media=()",
  "execution-while-not-rendered=()",
  "execution-while-out-of-viewport=()",
  "fullscreen=()",
  "geolocation=()",
  "gyroscope=()",
  "hid=()",
  "identity-credentials-get=()",
  "idle-detection=()",
  "magnetometer=()",
  "microphone=()",
  "midi=()",
  "navigation-override=()",
  "payment=()",
  "picture-in-picture=()",
  "publickey-credentials-get=()",
  "screen-wake-lock=()",
  "serial=()",
  "speaker-selection=()",
  "sync-xhr=()",
  "usb=()",
  "web-share=()",
  "xr-spatial-tracking=()",
].join(", ");

/**
 * Headers applied to EVERY response (static asset or Worker route).
 *
 * Notes:
 * - HSTS uses `preload` directive — eligible for hstspreload.org submission.
 *   Submission itself is deferred (V1.16 Welle D) because it is irreversible
 *   on a multi-month timescale; the directive in the header is harmless.
 * - COOP/COEP enable cross-origin isolation, which closes Spectre cross-process
 *   read risk on the WASM cryptographic verifier (security review M5). All
 *   page assets are same-origin, so `require-corp` does not break anything.
 * - CORP `same-origin` blocks any cross-origin context from embedding the
 *   page's resources directly.
 * - Referrer-Policy `no-referrer` removes the entire Referer header on
 *   navigation away — the verifier UI carries no PII in URL fragments, but
 *   leaking the playground hostname to third parties via Referer is needless.
 */
export const BASE_SECURITY_HEADERS: Readonly<Record<string, string>> = Object.freeze({
  "Content-Security-Policy": CSP,
  "Reporting-Endpoints": REPORTING_ENDPOINTS,
  "Report-To": REPORT_TO_LEGACY,
  "Strict-Transport-Security": "max-age=63072000; includeSubDomains; preload",
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
  "Cross-Origin-Resource-Policy": "same-origin",
  "Permissions-Policy": PERMISSIONS_POLICY,
  // X-Frame-Options is redundant given `frame-ancestors 'none'` in CSP, but
  // older browsers honour it and ignoring the modern directive — keep both.
  "X-Frame-Options": "DENY",
});

/**
 * Cache-Control class per asset type.
 * - html      : SRI-pinned scripts mean index.html must be revalidated to
 *               pick up new app.js hash on every load.
 * - immutable : content-hashed/SRI-pinned bytes — safe to cache forever.
 * - receiver  : Worker route response — must never be cached.
 */
export const CACHE_CONTROL = Object.freeze({
  html: "no-cache, must-revalidate",
  immutable: "public, max-age=31536000, immutable",
  receiver: "no-store",
});

export type AssetClass = keyof typeof CACHE_CONTROL;

/**
 * Classify a request path into a cache class. Pure function, exported for
 * unit testing.
 */
export function classifyPath(pathname: string): AssetClass {
  if (pathname === "/csp-report") return "receiver";
  if (pathname === "/" || pathname === "" || pathname.endsWith(".html")) return "html";
  return "immutable";
}

/**
 * Apply security headers to a response, replacing any platform-default
 * Cache-Control with the class-appropriate value. Returns a NEW Response
 * (immutable input) — this is the central seam where Worker-emitted headers
 * are layered onto every static asset and every Worker-route response.
 *
 * Note: we explicitly do NOT set `Vary` here. Cloudflare may inject
 * `Vary: Accept-Encoding` for compression; that interaction has been
 * verified safe (security review L2) but if anything goes wrong, the
 * `Vary` injection would only affect compressed-vs-uncompressed cache
 * variants of the same response, never strip security headers.
 */
export function applySecurityHeaders(response: Response, pathname: string): Response {
  const klass = classifyPath(pathname);
  const headers = new Headers(response.headers);

  // Layer security headers (Worker-emitted overrides any platform default).
  for (const [name, value] of Object.entries(BASE_SECURITY_HEADERS)) {
    headers.set(name, value);
  }
  headers.set("Cache-Control", CACHE_CONTROL[klass]);

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}
