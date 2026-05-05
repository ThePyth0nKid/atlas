/**
 * CSP violation report receiver — V1.16 Welle C.
 *
 * Implements the receiver-shape spec normatively documented in
 * docs/SECURITY-NOTES.md scope-d Bullet 6 ("Receiver-shape spec"). Every
 * branch in this file maps to a specific row of that table; the spec is
 * the source-of-truth for behaviour, this file is its executable form.
 *
 * Design principles:
 *   - Silent-204 to the caller on every validation failure (no oracle for
 *     attackers — they cannot distinguish origin-mismatch from rate-limit
 *     from schema-fail).
 *   - LOUD internally: every validation failure emits a categorised
 *     `console.error` JSON line (security review M2). Visible to the
 *     operator via `wrangler tail` or CF Workers logs.
 *   - Validate at every layer (Origin, CT, body-size pre+post, JSON-bomb,
 *     schema, rate-limit). Defence-in-depth against spec drift.
 *   - Store ONLY the normalised struct in AE — never the raw report body.
 *     Prevents stored-XSS in any log/monitoring UI from attacker-controlled
 *     fields; reduces AE row size; bounds the data carried into archive.
 */

import { safeJsonParse, type ParseFailure } from "./json-safe-parse.js";
import { checkRateLimit, type RateLimitEnv } from "./rate-limit.js";

const MAX_BODY_BYTES = 65536; // 64 KB cap per spec
const MAX_FIELD_LENGTH = 1024; // truncation for stored fields
const MAX_ORIGINAL_POLICY_LENGTH = 2048;

// Tight JSON-parse limits for the CSP-report shape specifically. Real CSP
// reports are flat ~12-key objects at depth-2 (legacy `{csp-report:{...}}`)
// or depth-3 (modern `[{type,body:{...}}]`). These caps give 2× headroom and
// are far below `DEFAULT_LIMITS` (10/100/8192) which is the safe-utility
// default for unknown JSON. Tighter caps shrink the parser's worst-case
// work per accepted payload (security review M3).
const RECEIVER_PARSE_LIMITS = {
  maxDepth: 4,
  maxKeysPerObject: 24,
  maxStringLength: 8192,
} as const;

const ALLOWED_CONTENT_TYPES = [
  "application/csp-report",
  "application/json",
  "application/reports+json", // Reporting API draft / Chrome
];

// Strip ANSI control sequences from any string field defensively (security
// review hardening). The same pattern documented in scope-d Bullet 6's
// receiver-shape spec table.
const ANSI_PATTERN =
  // eslint-disable-next-line no-control-regex
  /[\x00-\x08\x0B\x0C\x0E-\x1F\x7F\x9B\x1B][[\]()#;?]*(?:[0-9]{1,4}(?:;[0-9]{0,4})*)?[0-9A-PRZcf-ntqry=><]?/g;

export interface ReceiverEnv extends RateLimitEnv {
  readonly EXPECTED_ORIGIN: string;
  readonly ENVIRONMENT: string;
  readonly CSP_REPORTS_AE: AnalyticsEngineDataset;
}

export type ValidationFailure =
  | "origin_missing"
  | "origin_null"
  | "origin_mismatch"
  | "content_type_rejected"
  | "body_too_large_pre"
  | "body_too_large_post"
  | "json_parse_error"
  | "json_too_deep"
  | "json_too_wide"
  | "json_string_too_long"
  | "schema_invalid"
  | "rate_limited_per_ip"
  | "rate_limited_global"
  | "do_unavailable"
  | "ae_write_failed";

interface NormalisedReport {
  /** The directive that was violated (e.g. "script-src"). Used as AE index. */
  readonly violatedDirective: string;
  /** The blocked URI, parsed and serialised back to its origin only. */
  readonly blockedUri: string;
  /** The document URI, normalised to its origin only. */
  readonly documentUri: string;
  /** Source file basename (path stripped, query/fragment stripped). */
  readonly sourceFile: string;
  /** Line number where the violation occurred. 0 if not present. */
  readonly lineNumber: number;
  /** Column number where the violation occurred. 0 if not present. */
  readonly columnNumber: number;
  /** Original policy string, truncated. */
  readonly originalPolicy: string;
  /** User agent of the reporting browser. */
  readonly userAgent: string;
  /** ISO timestamp of receipt (Worker clock, not browser clock). */
  readonly receivedAt: string;
}

/**
 * Internal log emit. Visible to operator via `wrangler tail` / CF dashboard
 * Workers logs; never to the external caller.
 */
function logInternal(
  category: ValidationFailure | "accepted",
  request: Request,
  extra?: Record<string, unknown>,
): void {
  const cfRay = request.headers.get("CF-Ray") ?? "no-ray";
  const cfCountry =
    (request as Request & { cf?: { country?: string } }).cf?.country ?? "??";
  const line = {
    csp_receiver: true,
    category,
    cf_ray: cfRay,
    cf_country: cfCountry,
    ts: new Date().toISOString(),
    ...extra,
  };
  if (category === "accepted") {
    // Sample acceptance lines at low rate to avoid log volume noise; failures
    // are always logged (M2 — operator must see broken receiver immediately).
    console.log(JSON.stringify(line));
  } else {
    console.error(JSON.stringify(line));
  }
}

const SILENT_204 = (): Response =>
  // Empty body, NO custom headers (Cache-Control set by router/applySecurityHeaders).
  new Response(null, { status: 204 });

/**
 * Main receiver entry point. Returns a Response — the caller (router)
 * applies security headers and Cache-Control afterwards.
 *
 * The function is `async` because of body read + DO rate-limit call + AE
 * write; everything else is synchronous validation.
 */
export async function handleCspReport(
  request: Request,
  env: ReceiverEnv,
): Promise<Response> {
  // Method is enforced upstream by the router (index.ts only routes POST
  // here). No defence-in-depth method-check here on purpose: a 405 reply
  // would break the silent-204 invariant by giving the caller an oracle to
  // distinguish "wrong method" from "wrong Origin / wrong CT / wrong shape".

  // ── 1. Origin check (CSRF anchor — security review H1) ─────────────────
  const origin = request.headers.get("Origin");
  if (origin === null) {
    logInternal("origin_missing", request);
    return SILENT_204();
  }
  if (origin === "null") {
    // Sandboxed iframes, file:// pages, and cross-origin redirects emit
    // `Origin: null`. Explicit reject; do not silently treat as missing.
    logInternal("origin_null", request);
    return SILENT_204();
  }
  if (origin !== env.EXPECTED_ORIGIN) {
    logInternal("origin_mismatch", request, {
      seen: origin.slice(0, 256),
      expected: env.EXPECTED_ORIGIN,
    });
    return SILENT_204();
  }

  // ── 2. Content-Type allow-list ─────────────────────────────────────────
  const contentType = (request.headers.get("Content-Type") ?? "").toLowerCase();
  const ctMatched = ALLOWED_CONTENT_TYPES.some((allowed) =>
    contentType.startsWith(allowed),
  );
  if (!ctMatched) {
    logInternal("content_type_rejected", request, {
      content_type: contentType.slice(0, 128),
    });
    return SILENT_204();
  }

  // ── 3. Body-size pre-check (Content-Length) ────────────────────────────
  const contentLength = Number(request.headers.get("Content-Length") ?? "0");
  if (Number.isFinite(contentLength) && contentLength > MAX_BODY_BYTES) {
    logInternal("body_too_large_pre", request, { content_length: contentLength });
    return SILENT_204();
  }

  // ── 4. Read body + post-check ──────────────────────────────────────────
  // We read as text (raw bytes via .arrayBuffer would be cleaner for an exact
  // byte cap; .text() decodes UTF-8 first which is fine for CSP reports —
  // they're always UTF-8 JSON).
  let body: string;
  try {
    body = await request.text();
  } catch {
    logInternal("body_too_large_post", request, { reason: "read_failed" });
    return SILENT_204();
  }
  if (body.length > MAX_BODY_BYTES) {
    logInternal("body_too_large_post", request, { length: body.length });
    return SILENT_204();
  }

  // ── 5. Depth-limited JSON parse (security review H2 + M3) ──────────────
  // Tight per-receiver limits — see RECEIVER_PARSE_LIMITS.
  const parsed = safeJsonParse(body, RECEIVER_PARSE_LIMITS);
  if (!parsed.ok) {
    const cat = parseFailureToCategory(parsed.reason);
    logInternal(cat, request);
    return SILENT_204();
  }

  // ── 6. Schema validation ───────────────────────────────────────────────
  const report = extractReport(parsed.value);
  if (report === null) {
    logInternal("schema_invalid", request);
    return SILENT_204();
  }

  // ── 7. Rate limit check (post-validation, pre-write — only valid reports
  //     count toward the cost budget) ──────────────────────────────────────
  const rl = await checkRateLimit(env, request);
  if (!rl.allowed) {
    logInternal(rl.reason ?? "rate_limited_global", request);
    return SILENT_204();
  }

  // ── 8. Normalise — store ONLY allow-listed fields, after ANSI strip ────
  const normalised = normaliseReport(report, request);

  // ── 9. AE writeDataPoint ───────────────────────────────────────────────
  try {
    env.CSP_REPORTS_AE.writeDataPoint({
      indexes: [normalised.violatedDirective],
      blobs: [
        normalised.violatedDirective,
        normalised.blockedUri,
        normalised.documentUri,
        normalised.sourceFile,
        normalised.userAgent,
        normalised.originalPolicy,
        normalised.receivedAt,
      ],
      doubles: [normalised.lineNumber, normalised.columnNumber],
    });
  } catch (err) {
    logInternal("ae_write_failed", request, {
      error: err instanceof Error ? err.message.slice(0, 256) : "unknown",
    });
    // Still 204 — caller must not learn about backend failure.
    return SILENT_204();
  }

  logInternal("accepted", request, {
    violated_directive: normalised.violatedDirective,
  });
  return SILENT_204();
}

function parseFailureToCategory(
  reason: ParseFailure,
): ValidationFailure {
  switch (reason) {
    case "too_deep":
      return "json_too_deep";
    case "too_wide":
      return "json_too_wide";
    case "string_too_long":
      return "json_string_too_long";
    case "parse_error":
      return "json_parse_error";
  }
}

/**
 * Extract the inner CSP report struct, handling both legacy
 * (`{"csp-report": {...}}`) and modern Reporting API
 * (`[{"type":"csp-violation","body":{...}}]`) shapes.
 */
function extractReport(value: unknown): Record<string, unknown> | null {
  if (value === null || typeof value !== "object") return null;

  // Legacy shape: { "csp-report": { ... } }
  if ("csp-report" in value) {
    const inner = (value as { "csp-report": unknown })["csp-report"];
    if (inner !== null && typeof inner === "object") {
      return inner as Record<string, unknown>;
    }
  }

  // Modern shape: [ { "type": "csp-violation", "body": { ... } } ]
  if (Array.isArray(value) && value.length > 0) {
    const first = value[0];
    if (first !== null && typeof first === "object" && "body" in first) {
      const body = (first as { body: unknown }).body;
      if (body !== null && typeof body === "object") {
        return body as Record<string, unknown>;
      }
    }
  }

  // Direct shape: { "violated-directive": ... } at top-level
  if ("violated-directive" in value || "violatedDirective" in value) {
    return value as Record<string, unknown>;
  }

  return null;
}

/**
 * Normalise a raw report into the bounded, ANSI-stripped, allow-listed struct
 * we actually persist. Any field NOT in this allow-list is dropped.
 */
function normaliseReport(
  raw: Record<string, unknown>,
  request: Request,
): NormalisedReport {
  // Both legacy snake-case and modern camelCase keys exist in the wild.
  const get = (legacy: string, modern: string): unknown =>
    raw[legacy] ?? raw[modern];

  return {
    violatedDirective: stripAndTruncate(
      asString(get("violated-directive", "violatedDirective")),
      MAX_FIELD_LENGTH,
    ),
    blockedUri: normaliseUri(asString(get("blocked-uri", "blockedURL"))),
    documentUri: normaliseUri(asString(get("document-uri", "documentURL"))),
    sourceFile: stripAndTruncate(
      basename(asString(get("source-file", "sourceFile"))),
      MAX_FIELD_LENGTH,
    ),
    lineNumber: asNonNegativeInt(get("line-number", "lineNumber")),
    columnNumber: asNonNegativeInt(get("column-number", "columnNumber")),
    originalPolicy: stripAndTruncate(
      asString(get("original-policy", "originalPolicy")),
      MAX_ORIGINAL_POLICY_LENGTH,
    ),
    userAgent: stripAndTruncate(
      request.headers.get("User-Agent") ?? "",
      MAX_FIELD_LENGTH,
    ),
    receivedAt: new Date().toISOString(),
  };
}

function asString(v: unknown): string {
  return typeof v === "string" ? v : "";
}

function asNonNegativeInt(v: unknown): number {
  if (typeof v !== "number" || !Number.isFinite(v) || v < 0) return 0;
  return Math.floor(v);
}

function stripAndTruncate(s: string, max: number): string {
  return s.replace(ANSI_PATTERN, "").slice(0, max);
}

/**
 * Normalise a URI to its origin only (scheme + host + port). Drops path,
 * query, and fragment to bound stored data and prevent any path-injection
 * showing up in monitoring UIs that might URL-render the field.
 *
 * Falls back to the truncated raw string if URL parsing fails (e.g. the
 * report contains "inline" or "eval" as the blocked-uri).
 */
function normaliseUri(raw: string): string {
  const stripped = raw.replace(ANSI_PATTERN, "");
  if (stripped === "") return "";
  // CSP report values like "inline", "eval", "wasm-eval" are not URIs.
  if (!stripped.includes(":") && !stripped.includes("/")) {
    return stripped.slice(0, MAX_FIELD_LENGTH);
  }
  try {
    const u = new URL(stripped);
    return `${u.protocol}//${u.host}`.slice(0, MAX_FIELD_LENGTH);
  } catch {
    return stripped.slice(0, MAX_FIELD_LENGTH);
  }
}

function basename(path: string): string {
  if (path === "") return "";
  try {
    const u = new URL(path);
    const segs = u.pathname.split("/").filter(Boolean);
    return segs.length === 0 ? "" : (segs[segs.length - 1] ?? "");
  } catch {
    const segs = path.split(/[\\/]/).filter(Boolean);
    return segs.length === 0 ? "" : (segs[segs.length - 1] ?? "");
  }
}
