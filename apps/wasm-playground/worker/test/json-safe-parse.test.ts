/**
 * Unit tests for json-safe-parse.ts (V1.16 Welle C).
 *
 * Verifies the depth/key/string limits documented in
 * docs/V1.16-WELLE-C-PLAN.md §2.1 step 6 (security review H2 fix).
 */

import { describe, expect, it } from "vitest";
import { safeJsonParse, DEFAULT_LIMITS } from "../src/json-safe-parse.js";

describe("safeJsonParse — happy paths", () => {
  it("parses a typical CSP-report payload", () => {
    const input = JSON.stringify({
      "csp-report": {
        "violated-directive": "script-src",
        "blocked-uri": "https://evil.example/x.js",
        "document-uri": "https://playground.atlas-trust.dev/",
      },
    });
    const result = safeJsonParse(input);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value).toMatchObject({ "csp-report": { "violated-directive": "script-src" } });
    }
  });

  it("parses a Reporting API payload (array shape)", () => {
    const input = JSON.stringify([
      { type: "csp-violation", body: { "violated-directive": "script-src" } },
    ]);
    const result = safeJsonParse(input);
    expect(result.ok).toBe(true);
  });

  it("parses primitives", () => {
    expect(safeJsonParse('"hello"').ok).toBe(true);
    expect(safeJsonParse("42").ok).toBe(true);
    expect(safeJsonParse("true").ok).toBe(true);
    expect(safeJsonParse("null").ok).toBe(true);
  });

  it("parses payload exactly at maxDepth", () => {
    // Build a valid 10-deep nested object (the default limit).
    let s = '"value"';
    for (let i = 0; i < DEFAULT_LIMITS.maxDepth; i++) {
      s = `{"k":${s}}`;
    }
    const result = safeJsonParse(s);
    expect(result.ok).toBe(true);
  });
});

describe("safeJsonParse — depth limit (security H2)", () => {
  it("rejects deeply-nested objects beyond maxDepth", () => {
    let s = '"value"';
    for (let i = 0; i < 50; i++) s = `{"k":${s}}`;
    const result = safeJsonParse(s);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("too_deep");
  });

  it("rejects deeply-nested arrays beyond maxDepth", () => {
    let s = "1";
    for (let i = 0; i < 50; i++) s = `[${s}]`;
    const result = safeJsonParse(s);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("too_deep");
  });

  it("rejects mixed deeply-nested arrays/objects", () => {
    let s = '"v"';
    for (let i = 0; i < 50; i++) {
      s = i % 2 === 0 ? `[${s}]` : `{"k":${s}}`;
    }
    const result = safeJsonParse(s);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("too_deep");
  });

  it("does NOT count brackets inside strings toward depth", () => {
    // A flat object with a string containing many [/{ chars.
    const innerStr = "[".repeat(50) + "{".repeat(50);
    const input = JSON.stringify({ note: innerStr });
    const result = safeJsonParse(input);
    expect(result.ok).toBe(true);
  });

  it("handles escaped quotes in strings without confusing the scanner", () => {
    const input = JSON.stringify({ note: 'he said "[" and "{"' });
    const result = safeJsonParse(input);
    expect(result.ok).toBe(true);
  });
});

describe("safeJsonParse — key-count limit", () => {
  it("rejects objects with > maxKeysPerObject keys", () => {
    const big: Record<string, number> = {};
    for (let i = 0; i < DEFAULT_LIMITS.maxKeysPerObject + 5; i++) {
      big[`k${i}`] = i;
    }
    const result = safeJsonParse(JSON.stringify(big));
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("too_wide");
  });

  it("accepts objects exactly at maxKeysPerObject", () => {
    const ok: Record<string, number> = {};
    for (let i = 0; i < DEFAULT_LIMITS.maxKeysPerObject; i++) ok[`k${i}`] = i;
    const result = safeJsonParse(JSON.stringify(ok));
    expect(result.ok).toBe(true);
  });
});

describe("safeJsonParse — string-length limit", () => {
  it("rejects string values longer than maxStringLength", () => {
    const input = JSON.stringify({
      note: "x".repeat(DEFAULT_LIMITS.maxStringLength + 1),
    });
    const result = safeJsonParse(input);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("string_too_long");
  });

  it("rejects keys longer than maxStringLength", () => {
    const longKey = "k".repeat(DEFAULT_LIMITS.maxStringLength + 1);
    const obj: Record<string, number> = {};
    obj[longKey] = 1;
    const result = safeJsonParse(JSON.stringify(obj));
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("string_too_long");
  });
});

describe("safeJsonParse — malformed input", () => {
  it("rejects non-JSON garbage", () => {
    const result = safeJsonParse("this is not json");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("parse_error");
  });

  it("rejects empty string", () => {
    const result = safeJsonParse("");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("parse_error");
  });

  it("rejects truncated JSON", () => {
    const result = safeJsonParse('{"k":');
    expect(result.ok).toBe(false);
  });
});

describe("safeJsonParse — custom limits", () => {
  it("respects caller-provided maxDepth", () => {
    const result = safeJsonParse('{"a":{"b":{"c":1}}}', {
      maxDepth: 2,
      maxKeysPerObject: 100,
      maxStringLength: 100,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.reason).toBe("too_deep");
  });
});
