/**
 * V2-β Welle 12 — GET /api/atlas/audit/[event_uuid] handler tests.
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import type { AtlasEvent } from "@atlas/bridge";

const { getEventMock } = vi.hoisted(() => ({
  getEventMock: vi.fn<
    (workspaceId: string, eventUuid: string) => Promise<AtlasEvent | null>
  >(),
}));

vi.mock("../../_lib/projection-store", () => ({
  EventsJsonlProjectionStore: class {
    getEvent = getEventMock;
  },
}));

vi.mock("@/lib/bootstrap", () => ({}));

import { GET } from "./route";

beforeEach(() => {
  getEventMock.mockReset();
});

function makeReq(workspace = "ws-test"): Request {
  return new Request(`http://localhost/api/atlas/audit/evt-1?workspace=${workspace}`);
}

const SAMPLE: AtlasEvent = {
  event_id: "evt-1",
  event_hash: "hash-evt-1",
  parent_hashes: [],
  ts: "2026-05-13T10:00:00Z",
  payload: { type: "node.create", node: { id: "a", kind: "dataset" } },
  signature: { alg: "EdDSA", kid: "atlas-anchor:ws-test", sig: "fake" },
};

describe("GET /api/atlas/audit/[event_uuid]", () => {
  it("returns 200 + full event + signature_verified='deferred' on happy path", async () => {
    getEventMock.mockResolvedValue(SAMPLE);
    const res = await GET(makeReq(), {
      params: Promise.resolve({ event_uuid: "evt-1" }),
    });
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.event.event_id).toBe("evt-1");
    expect(body.signature_verified).toBe("deferred");
  });

  it("returns 404 when the event is missing", async () => {
    getEventMock.mockResolvedValue(null);
    const res = await GET(makeReq(), {
      params: Promise.resolve({ event_uuid: "missing" }),
    });
    expect(res.status).toBe(404);
  });

  it("returns 400 when workspace param is missing", async () => {
    const res = await GET(new Request("http://localhost/api/atlas/audit/evt-1"), {
      params: Promise.resolve({ event_uuid: "evt-1" }),
    });
    expect(res.status).toBe(400);
  });

  it("returns 400 when event_uuid is empty", async () => {
    const res = await GET(makeReq(), {
      params: Promise.resolve({ event_uuid: "" }),
    });
    expect(res.status).toBe(400);
  });
});
