/**
 * V2-β Welle 12 — GET /api/atlas/timeline handler tests.
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import type {
  ProjectedTimelineEvent,
  TimelineWindow,
} from "../_lib/projection-store";

const { getTimelineMock } = vi.hoisted(() => ({
  getTimelineMock: vi.fn<
    (
      workspaceId: string,
      window: TimelineWindow,
    ) => Promise<ProjectedTimelineEvent[]>
  >(),
}));

vi.mock("../_lib/projection-store", () => ({
  EventsJsonlProjectionStore: class {
    getTimeline = getTimelineMock;
  },
}));

vi.mock("@/lib/bootstrap", () => ({}));

import { GET } from "./route";

beforeEach(() => {
  getTimelineMock.mockReset();
});

const SAMPLE_EVENT: ProjectedTimelineEvent = {
  event_uuid: "evt-1",
  event_hash: "hash-evt-1",
  ts: "2026-05-13T10:00:00Z",
  kind: "node.create",
  author_did: "atlas-anchor:ws-test",
};

describe("GET /api/atlas/timeline", () => {
  it("returns 200 + events with default limit when no window", async () => {
    getTimelineMock.mockResolvedValue([SAMPLE_EVENT]);
    const res = await GET(new Request("http://localhost/api/atlas/timeline?workspace=ws-test"));
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.events).toHaveLength(1);
    expect(getTimelineMock).toHaveBeenCalledWith("ws-test", {
      from: undefined,
      to: undefined,
      limit: 50,
    });
  });

  it("forwards a custom limit", async () => {
    getTimelineMock.mockResolvedValue([]);
    await GET(
      new Request("http://localhost/api/atlas/timeline?workspace=ws-test&limit=20"),
    );
    expect(getTimelineMock).toHaveBeenCalledWith("ws-test", expect.objectContaining({ limit: 20 }));
  });

  it("clamps limit to MAX_LIMIT (500)", async () => {
    getTimelineMock.mockResolvedValue([]);
    await GET(
      new Request("http://localhost/api/atlas/timeline?workspace=ws-test&limit=9999"),
    );
    expect(getTimelineMock).toHaveBeenCalledWith("ws-test", expect.objectContaining({ limit: 500 }));
  });

  it("rejects non-integer limit with 400", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/timeline?workspace=ws-test&limit=abc"),
    );
    expect(res.status).toBe(400);
  });

  it("rejects zero / negative limit with 400", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/timeline?workspace=ws-test&limit=0"),
    );
    expect(res.status).toBe(400);
  });

  it("forwards a [from, to) window", async () => {
    getTimelineMock.mockResolvedValue([]);
    await GET(
      new Request(
        "http://localhost/api/atlas/timeline?workspace=ws-test&from=2026-05-13T09:00:00Z&to=2026-05-13T11:00:00Z",
      ),
    );
    expect(getTimelineMock).toHaveBeenCalledWith("ws-test", {
      from: "2026-05-13T09:00:00Z",
      to: "2026-05-13T11:00:00Z",
      limit: 50,
    });
  });

  it("rejects invalid ISO-8601 'from' with 400", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/timeline?workspace=ws-test&from=not-a-date"),
    );
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toMatch(/from/);
  });

  it("rejects invalid ISO-8601 'to' with 400", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/timeline?workspace=ws-test&to=not-a-date"),
    );
    expect(res.status).toBe(400);
  });

  it("returns 400 when workspace is missing", async () => {
    const res = await GET(new Request("http://localhost/api/atlas/timeline"));
    expect(res.status).toBe(400);
  });
});
