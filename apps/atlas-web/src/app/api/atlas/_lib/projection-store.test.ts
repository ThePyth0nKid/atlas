/**
 * V2-β Welle 12 — ProjectionStore unit tests.
 *
 * Tests mock `@atlas/bridge.readAllEvents` so the in-memory
 * projection logic is exercised without touching disk. The same
 * `ProjectionStore` interface will be implemented in Phase 7 (W17b)
 * by an ArcadeDB-backed store; this test file remains relevant
 * because every impl must satisfy the same contract semantics.
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import type { AtlasEvent } from "@atlas/bridge";

// Hoist-safe mock of `@atlas/bridge`. We only need `readAllEvents`
// for the projection-store impl.
const readAllEventsMock = vi.fn<(workspaceId: string) => Promise<AtlasEvent[]>>();
vi.mock("@atlas/bridge", () => ({
  readAllEvents: (workspaceId: string) => readAllEventsMock(workspaceId),
}));

// Import the SUT AFTER the mock is set up.
import { EventsJsonlProjectionStore } from "./projection-store";

function makeNodeCreateEvent(args: {
  event_id: string;
  ts: string;
  kid?: string;
  parent_hashes?: string[];
  node: { id: string; kind: string; [k: string]: unknown };
}): AtlasEvent {
  return {
    event_id: args.event_id,
    event_hash: `hash-${args.event_id}`,
    parent_hashes: args.parent_hashes ?? [],
    ts: args.ts,
    payload: {
      type: "node.create",
      node: args.node,
    },
    signature: {
      alg: "EdDSA",
      kid: args.kid ?? "atlas-anchor:ws-test",
      sig: "fake-sig",
    },
  };
}

function makeEdgeCreateEvent(args: {
  event_id: string;
  ts: string;
  kid?: string;
  edge: { from: string; to: string; kind?: string; [k: string]: unknown };
}): AtlasEvent {
  return {
    event_id: args.event_id,
    event_hash: `hash-${args.event_id}`,
    parent_hashes: [],
    ts: args.ts,
    payload: {
      type: "edge.create",
      edge: args.edge,
    },
    signature: {
      alg: "EdDSA",
      kid: args.kid ?? "atlas-anchor:ws-test",
      sig: "fake-sig",
    },
  };
}

beforeEach(() => {
  readAllEventsMock.mockReset();
});

describe("EventsJsonlProjectionStore.getEntity", () => {
  it("returns null when the workspace is empty", async () => {
    readAllEventsMock.mockResolvedValue([]);
    const store = new EventsJsonlProjectionStore();
    expect(await store.getEntity("ws-test", "missing-id")).toBeNull();
  });

  it("projects a node.create event to a ProjectedEntity", async () => {
    readAllEventsMock.mockResolvedValue([
      makeNodeCreateEvent({
        event_id: "evt-1",
        ts: "2026-05-13T10:00:00Z",
        node: { id: "dataset-1", kind: "dataset", licence: "CC-BY" },
      }),
    ]);
    const store = new EventsJsonlProjectionStore();
    const e = await store.getEntity("ws-test", "dataset-1");
    expect(e).not.toBeNull();
    expect(e?.entity_uuid).toBe("dataset-1");
    expect(e?.kind).toBe("dataset");
    expect(e?.properties).toEqual({ licence: "CC-BY" });
    expect(e?.author_did).toBe("atlas-anchor:ws-test");
    expect(e?.created_event_uuid).toBe("evt-1");
    expect(e?.created_at).toBe("2026-05-13T10:00:00Z");
  });

  it("returns null for an unknown entity in a populated workspace", async () => {
    readAllEventsMock.mockResolvedValue([
      makeNodeCreateEvent({
        event_id: "evt-1",
        ts: "2026-05-13T10:00:00Z",
        node: { id: "dataset-1", kind: "dataset" },
      }),
    ]);
    const store = new EventsJsonlProjectionStore();
    expect(await store.getEntity("ws-test", "no-such-id")).toBeNull();
  });
});

describe("EventsJsonlProjectionStore.getRelated", () => {
  it("returns null when the entity does not exist", async () => {
    readAllEventsMock.mockResolvedValue([]);
    const store = new EventsJsonlProjectionStore();
    expect(await store.getRelated("ws-test", "missing")).toBeNull();
  });

  it("partitions edges into outgoing and incoming", async () => {
    readAllEventsMock.mockResolvedValue([
      makeNodeCreateEvent({
        event_id: "evt-1",
        ts: "2026-05-13T10:00:00Z",
        node: { id: "a", kind: "dataset" },
      }),
      makeEdgeCreateEvent({
        event_id: "evt-2",
        ts: "2026-05-13T10:01:00Z",
        edge: { from: "a", to: "b", kind: "derived_from" },
      }),
      makeEdgeCreateEvent({
        event_id: "evt-3",
        ts: "2026-05-13T10:02:00Z",
        edge: { from: "c", to: "a", kind: "annotated_by" },
      }),
    ]);
    const store = new EventsJsonlProjectionStore();
    const r = await store.getRelated("ws-test", "a");
    expect(r).not.toBeNull();
    expect(r?.outgoing).toHaveLength(1);
    expect(r?.outgoing[0].to).toBe("b");
    expect(r?.incoming).toHaveLength(1);
    expect(r?.incoming[0].from).toBe("c");
  });
});

describe("EventsJsonlProjectionStore.getTimeline", () => {
  const baseEvents = [
    makeNodeCreateEvent({
      event_id: "evt-1",
      ts: "2026-05-13T09:00:00Z",
      node: { id: "a", kind: "dataset" },
    }),
    makeNodeCreateEvent({
      event_id: "evt-2",
      ts: "2026-05-13T10:00:00Z",
      node: { id: "b", kind: "model" },
    }),
    makeNodeCreateEvent({
      event_id: "evt-3",
      ts: "2026-05-13T11:00:00Z",
      node: { id: "c", kind: "inference" },
    }),
  ];

  it("returns all events under the limit when no window is set", async () => {
    readAllEventsMock.mockResolvedValue(baseEvents);
    const store = new EventsJsonlProjectionStore();
    const tl = await store.getTimeline("ws-test", { limit: 50 });
    expect(tl).toHaveLength(3);
    expect(tl[0].kind).toBe("node.create");
    expect(tl[0].event_uuid).toBe("evt-1");
  });

  it("filters by [from, to) window", async () => {
    readAllEventsMock.mockResolvedValue(baseEvents);
    const store = new EventsJsonlProjectionStore();
    const tl = await store.getTimeline("ws-test", {
      from: "2026-05-13T10:00:00Z",
      to: "2026-05-13T11:00:00Z",
      limit: 50,
    });
    expect(tl).toHaveLength(1);
    expect(tl[0].event_uuid).toBe("evt-2");
  });

  it("respects the limit", async () => {
    readAllEventsMock.mockResolvedValue(baseEvents);
    const store = new EventsJsonlProjectionStore();
    const tl = await store.getTimeline("ws-test", { limit: 2 });
    expect(tl).toHaveLength(2);
  });

  it("excludes events with unparseable ts", async () => {
    readAllEventsMock.mockResolvedValue([
      ...baseEvents,
      makeNodeCreateEvent({
        event_id: "evt-bad",
        ts: "not-a-date",
        node: { id: "d", kind: "other" },
      }),
    ]);
    const store = new EventsJsonlProjectionStore();
    const tl = await store.getTimeline("ws-test", { limit: 50 });
    expect(tl.map((e) => e.event_uuid)).not.toContain("evt-bad");
  });
});

describe("EventsJsonlProjectionStore.getEvent", () => {
  it("returns the full AtlasEvent by event_uuid", async () => {
    const ev = makeNodeCreateEvent({
      event_id: "evt-1",
      ts: "2026-05-13T10:00:00Z",
      node: { id: "a", kind: "dataset" },
    });
    readAllEventsMock.mockResolvedValue([ev]);
    const store = new EventsJsonlProjectionStore();
    const got = await store.getEvent("ws-test", "evt-1");
    expect(got).toEqual(ev);
  });

  it("returns null for an unknown event_uuid", async () => {
    readAllEventsMock.mockResolvedValue([]);
    const store = new EventsJsonlProjectionStore();
    expect(await store.getEvent("ws-test", "missing")).toBeNull();
  });
});
