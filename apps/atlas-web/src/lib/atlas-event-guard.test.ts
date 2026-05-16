/**
 * W20b-1 fix-commit — unit tests for the `isAtlasEventShape` runtime
 * type-guard.
 *
 * The guard is the single gate between an untrusted JSON-deserialised
 * trace event and the dashboard's typed reader code, so its accept /
 * reject decisions are load-bearing. We exercise both the happy path
 * and the realistic malformed-shape rejections.
 */

import { describe, it, expect } from "vitest";
import { isAtlasEventShape } from "./atlas-event-guard";

function wellFormed(): unknown {
  return {
    event_id: "ev-1",
    event_hash: "hash-1",
    parent_hashes: [],
    payload: { type: "node.create" },
    signature: { alg: "EdDSA", kid: "atlas-anchor:ws-x", sig: "abc" },
    ts: "2026-05-16T11:00:00Z",
  };
}

describe("isAtlasEventShape — accepts well-formed shapes", () => {
  it("accepts a minimal well-formed AtlasEvent", () => {
    expect(isAtlasEventShape(wellFormed())).toBe(true);
  });

  it("accepts events with extra fields (forwards-compat)", () => {
    const ev = { ...(wellFormed() as Record<string, unknown>), extra: 42 };
    expect(isAtlasEventShape(ev)).toBe(true);
  });

  it("accepts events with empty-object payload (anchor-style)", () => {
    const ev = { ...(wellFormed() as Record<string, unknown>), payload: {} };
    expect(isAtlasEventShape(ev)).toBe(true);
  });
});

describe("isAtlasEventShape — rejects non-objects", () => {
  it("rejects null", () => {
    expect(isAtlasEventShape(null)).toBe(false);
  });

  it("rejects undefined", () => {
    expect(isAtlasEventShape(undefined)).toBe(false);
  });

  it("rejects strings", () => {
    expect(isAtlasEventShape("event")).toBe(false);
  });

  it("rejects numbers", () => {
    expect(isAtlasEventShape(42)).toBe(false);
  });

  it("rejects arrays", () => {
    // Arrays are typeof "object", so this is the trap a naive guard
    // would fall into. Our guard relies on the field-shape checks to
    // reject these, since arrays do not carry the required keys.
    expect(isAtlasEventShape([])).toBe(false);
  });
});

describe("isAtlasEventShape — rejects malformed required fields", () => {
  it("rejects event_id of wrong type", () => {
    const ev = { ...(wellFormed() as Record<string, unknown>), event_id: 1 };
    expect(isAtlasEventShape(ev)).toBe(false);
  });

  it("rejects missing event_hash", () => {
    const { event_hash: _h, ...rest } = wellFormed() as Record<string, unknown>;
    expect(isAtlasEventShape(rest)).toBe(false);
  });

  it("rejects missing ts", () => {
    const { ts: _t, ...rest } = wellFormed() as Record<string, unknown>;
    expect(isAtlasEventShape(rest)).toBe(false);
  });

  it("rejects null payload", () => {
    const ev = {
      ...(wellFormed() as Record<string, unknown>),
      payload: null,
    };
    expect(isAtlasEventShape(ev)).toBe(false);
  });

  it("rejects string payload (must be object)", () => {
    const ev = {
      ...(wellFormed() as Record<string, unknown>),
      payload: "node.create",
    };
    expect(isAtlasEventShape(ev)).toBe(false);
  });

  it("rejects null signature", () => {
    const ev = {
      ...(wellFormed() as Record<string, unknown>),
      signature: null,
    };
    expect(isAtlasEventShape(ev)).toBe(false);
  });

  it("rejects string signature", () => {
    const ev = {
      ...(wellFormed() as Record<string, unknown>),
      signature: "deadbeef",
    };
    expect(isAtlasEventShape(ev)).toBe(false);
  });
});
