/**
 * Depth-limited, key-count-limited, string-length-limited JSON parser.
 *
 * Defends against JSON-bomb DoS within the body-size cap (security review H2):
 * a 60 KB input of `[[[[...]]]]` deeply nested arrays would blow the V8 call
 * stack via `JSON.parse` recursion before the runtime memory limit fires.
 * Pre-scan rejects such input before the native parser ever sees it.
 *
 * Strategy:
 *   1. Pre-scan input character-by-character with string-state-tracking,
 *      count maximum nesting depth of `{`/`[`. Reject if > maxDepth.
 *   2. Native JSON.parse (now safe — depth-bounded).
 *   3. Post-parse walk: count keys per object and validate string lengths.
 *
 * Pre-scan correctly handles strings (a `"{"` character INSIDE a JSON string
 * does NOT count toward nesting depth) and escape sequences (`"\\""` is a
 * literal backslash followed by an end-of-string quote).
 */

export interface ParseLimits {
  /** Maximum nesting depth of objects/arrays. */
  readonly maxDepth: number;
  /** Maximum number of keys in any single object. */
  readonly maxKeysPerObject: number;
  /** Maximum length (in JS chars) of any string value or key. */
  readonly maxStringLength: number;
}

export const DEFAULT_LIMITS: ParseLimits = Object.freeze({
  maxDepth: 10,
  maxKeysPerObject: 100,
  maxStringLength: 8192,
});

export type ParseFailure =
  | "too_deep"
  | "too_wide"
  | "string_too_long"
  | "parse_error";

export type ParseResult<T = unknown> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly reason: ParseFailure };

/**
 * Pre-scan: walk the raw JSON input and reject if structural nesting exceeds
 * `maxDepth`. Returns true if depth is within the cap, false otherwise.
 *
 * Uses a single-pass character walk with explicit string/escape state so that
 * brackets inside string literals do NOT inflate the depth count.
 */
function preScanDepth(input: string, maxDepth: number): boolean {
  let depth = 0;
  let inString = false;
  let escaped = false;

  for (let i = 0; i < input.length; i++) {
    const c = input[i];

    if (escaped) {
      escaped = false;
      continue;
    }
    if (inString) {
      if (c === "\\") escaped = true;
      else if (c === '"') inString = false;
      continue;
    }
    if (c === '"') {
      inString = true;
    } else if (c === "{" || c === "[") {
      depth++;
      if (depth > maxDepth) return false;
    } else if (c === "}" || c === "]") {
      depth--;
    }
  }
  return true;
}

/**
 * Post-parse walk: validate key count per object and string lengths.
 * Returns the failure reason on first violation, or `null` if all values pass.
 *
 * Walks objects and arrays recursively. Native JSON.parse output is already
 * depth-bounded (we pre-scanned), so this recursion is safe.
 */
function validateShape(
  value: unknown,
  limits: ParseLimits,
): ParseFailure | null {
  if (typeof value === "string") {
    if (value.length > limits.maxStringLength) return "string_too_long";
    return null;
  }
  if (Array.isArray(value)) {
    for (const item of value) {
      const fail = validateShape(item, limits);
      if (fail !== null) return fail;
    }
    return null;
  }
  if (value !== null && typeof value === "object") {
    const keys = Object.keys(value as Record<string, unknown>);
    if (keys.length > limits.maxKeysPerObject) return "too_wide";
    for (const k of keys) {
      if (k.length > limits.maxStringLength) return "string_too_long";
      const fail = validateShape(
        (value as Record<string, unknown>)[k],
        limits,
      );
      if (fail !== null) return fail;
    }
    return null;
  }
  return null;
}

/**
 * Parse a JSON string with structural and shape limits.
 *
 * On success returns `{ ok: true, value }`.
 * On any failure returns `{ ok: false, reason }` with a categorised reason
 * suitable for receiver internal-log categorisation (security review M2).
 */
export function safeJsonParse<T = unknown>(
  input: string,
  limits: ParseLimits = DEFAULT_LIMITS,
): ParseResult<T> {
  if (!preScanDepth(input, limits.maxDepth)) {
    return { ok: false, reason: "too_deep" };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(input);
  } catch {
    return { ok: false, reason: "parse_error" };
  }

  const shapeFailure = validateShape(parsed, limits);
  if (shapeFailure !== null) {
    return { ok: false, reason: shapeFailure };
  }

  return { ok: true, value: parsed as T };
}
