/**
 * W20b-1 — unit tests for the pure workspace-metrics module.
 *
 * The metric pipeline drives the 7 dashboard KPIs visible on the home
 * page; bugs here would silently misreport workspace state. Coverage
 * targets:
 *   - empty / genesis / linear-chain / Y-fork DAG shapes
 *   - malformed `ts`, missing payload.type, missing signature.kid
 *   - 30d-window correctness with a frozen `nowMs`
 *   - multi-signer uniqueness + dedup
 *   - anchor detection by both signals (payload.type prefix + anchor_proof)
 */

import { describe, it, expect } from "vitest";
import type { AtlasEvent } from "@atlas/bridge";
import { computeWorkspaceMetrics } from "./workspace-metrics";

const NOW = Date.parse("2026-05-16T12:00:00Z");
const DAY = 86_400_000;

function makeEvent(overrides: Partial<AtlasEvent> = {}): AtlasEvent {
  return {
    event_id: overrides.event_id ?? "ev-0",
    event_hash: overrides.event_hash ?? "hash-0",
    parent_hashes: overrides.parent_hashes ?? [],
    payload: overrides.payload ?? { type: "node.create" },
    signature: overrides.signature ?? {
      alg: "EdDSA",
      kid: "atlas-anchor:ws-test",
      sig: "deadbeef",
    },
    ts: overrides.ts ?? "2026-05-16T11:00:00Z",
  };
}

describe("computeWorkspaceMetrics — empty + minimal", () => {
  it("empty events array yields all-zero metrics", () => {
    const m = computeWorkspaceMetrics([], NOW);
    expect(m.totalEvents).toBe(0);
    expect(m.eventsLast30d).toBe(0);
    expect(m.eventsLast30dPrior).toBe(0);
    expect(m.eventsByType).toEqual({});
    expect(m.uniqueSigners).toEqual([]);
    expect(m.dagDepth).toBe(0);
    expect(m.anchorCount).toBe(0);
    expect(m.tipCount).toBe(0);
  });

  it("single genesis event yields totalEvents=1, dagDepth=1, tipCount=1", () => {
    const events = [
      makeEvent({
        event_hash: "g",
        parent_hashes: [],
        payload: { type: "node.create" },
      }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.totalEvents).toBe(1);
    expect(m.dagDepth).toBe(1);
    expect(m.tipCount).toBe(1);
    expect(m.uniqueSigners).toEqual(["atlas-anchor:ws-test"]);
    expect(m.eventsByType).toEqual({ "node.create": 1 });
    expect(m.eventsLast30d).toBe(1);
    expect(m.eventsLast30dPrior).toBe(0);
  });
});

describe("computeWorkspaceMetrics — DAG shapes", () => {
  it("3-event linear chain → dagDepth=3, tipCount=1", () => {
    const events = [
      makeEvent({ event_hash: "a", parent_hashes: [] }),
      makeEvent({ event_hash: "b", parent_hashes: ["a"] }),
      makeEvent({ event_hash: "c", parent_hashes: ["b"] }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.dagDepth).toBe(3);
    expect(m.tipCount).toBe(1);
    expect(m.totalEvents).toBe(3);
  });

  it("Y-fork (1 root, 2 tips) → dagDepth=2, tipCount=2", () => {
    const events = [
      makeEvent({ event_hash: "root", parent_hashes: [] }),
      makeEvent({ event_hash: "left", parent_hashes: ["root"] }),
      makeEvent({ event_hash: "right", parent_hashes: ["root"] }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.dagDepth).toBe(2);
    expect(m.tipCount).toBe(2);
  });

  it("diamond → dagDepth=3, tipCount=1", () => {
    const events = [
      makeEvent({ event_hash: "r", parent_hashes: [] }),
      makeEvent({ event_hash: "l", parent_hashes: ["r"] }),
      makeEvent({ event_hash: "ri", parent_hashes: ["r"] }),
      makeEvent({ event_hash: "join", parent_hashes: ["l", "ri"] }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.dagDepth).toBe(3);
    expect(m.tipCount).toBe(1);
  });
});

describe("computeWorkspaceMetrics — malformed input resilience", () => {
  it("malformed ts counted in total, skipped in window aggregates", () => {
    const events = [
      makeEvent({ event_hash: "ok", ts: "2026-05-15T00:00:00Z" }),
      makeEvent({ event_hash: "bad-ts", ts: "not-an-iso-date" }),
      makeEvent({
        event_hash: "bad-ts-type",
        // @ts-expect-error — deliberately corrupt
        ts: 12345,
      }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.totalEvents).toBe(3);
    expect(m.eventsLast30d).toBe(1);
    expect(m.eventsLast30dPrior).toBe(0);
  });

  it("missing payload.type does not contribute to eventsByType", () => {
    const events = [
      makeEvent({ event_hash: "a", payload: {} }),
      makeEvent({ event_hash: "b", payload: { type: "node.create" } }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.eventsByType).toEqual({ "node.create": 1 });
    expect(m.totalEvents).toBe(2);
  });

  it("missing signature.kid does not pollute uniqueSigners", () => {
    const events = [
      makeEvent({ event_hash: "a" }),
      makeEvent({
        event_hash: "b",
        // @ts-expect-error — deliberately corrupt
        signature: { alg: "EdDSA" },
      }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.uniqueSigners).toEqual(["atlas-anchor:ws-test"]);
  });
});

describe("computeWorkspaceMetrics — multi-signer + windows", () => {
  it("multiple kids produce sorted, dedup'd uniqueSigners", () => {
    const events = [
      makeEvent({
        event_hash: "a",
        signature: { alg: "EdDSA", kid: "atlas-anchor:b", sig: "..." },
      }),
      makeEvent({
        event_hash: "b",
        signature: { alg: "EdDSA", kid: "atlas-anchor:a", sig: "..." },
      }),
      makeEvent({
        event_hash: "c",
        signature: { alg: "EdDSA", kid: "atlas-anchor:a", sig: "..." },
      }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.uniqueSigners).toEqual(["atlas-anchor:a", "atlas-anchor:b"]);
  });

  it("prior-period window covers (now-60d, now-30d]", () => {
    const events = [
      // 5d ago → last 30d
      makeEvent({
        event_hash: "recent",
        ts: new Date(NOW - 5 * DAY).toISOString(),
      }),
      // 35d ago → prior
      makeEvent({
        event_hash: "prior-a",
        ts: new Date(NOW - 35 * DAY).toISOString(),
      }),
      // 55d ago → prior
      makeEvent({
        event_hash: "prior-b",
        ts: new Date(NOW - 55 * DAY).toISOString(),
      }),
      // 90d ago → out of both windows
      makeEvent({
        event_hash: "ancient",
        ts: new Date(NOW - 90 * DAY).toISOString(),
      }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.eventsLast30d).toBe(1);
    expect(m.eventsLast30dPrior).toBe(2);
    expect(m.totalEvents).toBe(4);
  });
});

describe("computeWorkspaceMetrics — anchor detection", () => {
  it("payload.type starting with anchor. counts as anchor", () => {
    const events = [
      makeEvent({ event_hash: "n", payload: { type: "node.create" } }),
      makeEvent({ event_hash: "a", payload: { type: "anchor.created" } }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.anchorCount).toBe(1);
  });

  it("payload.anchor_proof presence counts as anchor", () => {
    const events = [
      makeEvent({
        event_hash: "x",
        payload: { type: "node.create", anchor_proof: { kind: "rekor" } },
      }),
    ];
    const m = computeWorkspaceMetrics(events, NOW);
    expect(m.anchorCount).toBe(1);
  });
});
