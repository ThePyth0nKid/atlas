/**
 * V2-β Welle 12 — GET /api/atlas/related/[id] handler tests.
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import type { ProjectedEdge } from "../../_lib/projection-store";

const { getRelatedMock } = vi.hoisted(() => ({
  getRelatedMock: vi.fn<
    (
      workspaceId: string,
      entityUuid: string,
    ) => Promise<{ outgoing: ProjectedEdge[]; incoming: ProjectedEdge[] } | null>
  >(),
}));

vi.mock("../../_lib/projection-store", () => ({
  EventsJsonlProjectionStore: class {
    getRelated = getRelatedMock;
  },
}));

vi.mock("@/lib/bootstrap", () => ({}));

import { GET } from "./route";

beforeEach(() => {
  getRelatedMock.mockReset();
});

function makeReq(workspace = "ws-test"): Request {
  return new Request(`http://localhost/api/atlas/related/a?workspace=${workspace}`);
}

describe("GET /api/atlas/related/[id]", () => {
  it("returns 200 + outgoing/incoming on happy path", async () => {
    const edge: ProjectedEdge = {
      from: "a",
      to: "b",
      kind: "derived_from",
      properties: {},
      author_did: "atlas-anchor:ws-test",
      created_event_uuid: "evt-2",
      created_at: "2026-05-13T10:01:00Z",
    };
    getRelatedMock.mockResolvedValue({ outgoing: [edge], incoming: [] });

    const res = await GET(makeReq(), { params: Promise.resolve({ id: "a" }) });
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.outgoing).toHaveLength(1);
    expect(body.incoming).toHaveLength(0);
  });

  it("returns 404 when the entity is missing", async () => {
    getRelatedMock.mockResolvedValue(null);
    const res = await GET(makeReq(), { params: Promise.resolve({ id: "missing" }) });
    expect(res.status).toBe(404);
  });

  it("returns 400 when workspace param is missing", async () => {
    const res = await GET(new Request("http://localhost/api/atlas/related/a"), {
      params: Promise.resolve({ id: "a" }),
    });
    expect(res.status).toBe(400);
  });

  it("returns 400 when entity id is empty", async () => {
    const res = await GET(makeReq(), { params: Promise.resolve({ id: "" }) });
    expect(res.status).toBe(400);
  });
});
