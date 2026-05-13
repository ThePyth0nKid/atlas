/**
 * V2-β Welle 12 — GET /api/atlas/entities/[id] handler tests.
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import type { ProjectedEntity } from "../../_lib/projection-store";

// `vi.hoisted` keeps the mock function out of the TDZ when
// `vi.mock`'s factory is hoisted above the import.
const { getEntityMock } = vi.hoisted(() => ({
  getEntityMock: vi.fn<
    (workspaceId: string, entityUuid: string) => Promise<ProjectedEntity | null>
  >(),
}));

// Mock the projection-store module so the route handler's
// module-scoped `new EventsJsonlProjectionStore()` uses our stub.
vi.mock("../../_lib/projection-store", () => ({
  EventsJsonlProjectionStore: class {
    getEntity = getEntityMock;
  },
}));

// `@/lib/bootstrap` is a side-effect module that calls
// `setDefaultDataDir`. Stub it to a no-op in tests.
vi.mock("@/lib/bootstrap", () => ({}));

import { GET } from "./route";

beforeEach(() => {
  getEntityMock.mockReset();
});

function makeReq(workspace: string | null = "ws-test"): Request {
  const url =
    workspace === null
      ? "http://localhost/api/atlas/entities/dataset-1"
      : `http://localhost/api/atlas/entities/dataset-1?workspace=${workspace}`;
  return new Request(url);
}

describe("GET /api/atlas/entities/[id]", () => {
  it("returns 200 + entity on happy path", async () => {
    const entity: ProjectedEntity = {
      entity_uuid: "dataset-1",
      kind: "dataset",
      properties: { licence: "CC-BY" },
      author_did: "atlas-anchor:ws-test",
      created_event_uuid: "evt-1",
      created_at: "2026-05-13T10:00:00Z",
    };
    getEntityMock.mockResolvedValue(entity);

    const res = await GET(makeReq(), { params: Promise.resolve({ id: "dataset-1" }) });
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.entity.entity_uuid).toBe("dataset-1");
    expect(getEntityMock).toHaveBeenCalledWith("ws-test", "dataset-1");
  });

  it("returns 404 when the entity is missing", async () => {
    getEntityMock.mockResolvedValue(null);
    const res = await GET(makeReq(), { params: Promise.resolve({ id: "missing" }) });
    expect(res.status).toBe(404);
    const body = await res.json();
    expect(body.ok).toBe(false);
    expect(body.error).toMatch(/not found/);
  });

  it("returns 400 when workspace param is missing", async () => {
    const res = await GET(makeReq(null), { params: Promise.resolve({ id: "dataset-1" }) });
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.ok).toBe(false);
    expect(body.error).toMatch(/workspace/);
  });

  it("returns 400 when workspace fails the regex", async () => {
    const res = await GET(makeReq("../etc"), {
      params: Promise.resolve({ id: "dataset-1" }),
    });
    expect(res.status).toBe(400);
  });

  it("returns 400 when entity id is empty", async () => {
    const res = await GET(makeReq(), { params: Promise.resolve({ id: "" }) });
    expect(res.status).toBe(400);
  });

  it("returns 400 when entity id is too long", async () => {
    const longId = "x".repeat(300);
    const res = await GET(makeReq(), { params: Promise.resolve({ id: longId }) });
    expect(res.status).toBe(400);
  });

  it("returns 500 on unexpected projection-store failure", async () => {
    getEntityMock.mockRejectedValue(new Error("boom"));
    const res = await GET(makeReq(), { params: Promise.resolve({ id: "dataset-1" }) });
    expect(res.status).toBe(500);
  });
});
