/**
 * W20a — GET /api/atlas/workspaces handler tests.
 *
 * Asserts the listing + filtering contract:
 *   - Empty data root → 200 with empty list, default = null.
 *   - Mixed directory entries → only directories are returned.
 *   - CI-pattern names (`pw-w*-…`) are stripped.
 *   - Workspace ids that fail the regex are stripped.
 *   - Results are sorted alphabetically for stable UI.
 */

import { describe, it, expect, beforeEach, vi } from "vitest";

const { readdirMock, dataDirMock } = vi.hoisted(() => ({
  readdirMock: vi.fn(),
  dataDirMock: vi.fn(() => "/tmp/atlas-data"),
}));

vi.mock("@/lib/bootstrap", () => ({}));

vi.mock("node:fs", async () => {
  const actual = await vi.importActual<typeof import("node:fs")>("node:fs");
  return {
    ...actual,
    promises: {
      ...actual.promises,
      readdir: readdirMock,
    },
  };
});

vi.mock("@atlas/bridge", async () => {
  // Defer to the real module for `isValidWorkspaceId` + `redactPaths`,
  // but stub `dataDir` so the test does not need a real fs root.
  const actual =
    await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
  return {
    ...actual,
    dataDir: dataDirMock,
  };
});

import { GET } from "./route";

interface Dirent {
  name: string;
  isDirectory: () => boolean;
}

const dir = (name: string): Dirent => ({ name, isDirectory: () => true });
const file = (name: string): Dirent => ({ name, isDirectory: () => false });

beforeEach(() => {
  readdirMock.mockReset();
  dataDirMock.mockReset();
  dataDirMock.mockReturnValue("/tmp/atlas-data");
});

describe("GET /api/atlas/workspaces", () => {
  it("returns empty list when the data root does not exist", async () => {
    const err = Object.assign(new Error("ENOENT"), { code: "ENOENT" });
    readdirMock.mockRejectedValue(err);

    const res = await GET();
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.workspaces).toEqual([]);
    expect(body.default).toBe(null);
  });

  it("filters out files, listing only directories", async () => {
    readdirMock.mockResolvedValue([
      dir("ws-alpha"),
      file("notes.txt"),
      dir("ws-beta"),
    ]);

    const res = await GET();
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.workspaces).toEqual(["ws-alpha", "ws-beta"]);
    expect(body.default).toBe("ws-alpha");
  });

  it("filters out Playwright CI-artifact workspaces", async () => {
    readdirMock.mockResolvedValue([
      dir("pw-w0-mp12xfmg-iwsz7a"),
      dir("pw-w2-mp132h5u-guh38j"),
      dir("ws-mcp-default"),
    ]);

    const res = await GET();
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.workspaces).toEqual(["ws-mcp-default"]);
    expect(body.default).toBe("ws-mcp-default");
  });

  it("filters out names that fail isValidWorkspaceId", async () => {
    readdirMock.mockResolvedValue([
      dir("ws-good"),
      dir("ws bad"), // space — fails regex
      dir(".hidden"), // dot — fails regex
      dir("a".repeat(200)), // too long — fails regex
    ]);

    const res = await GET();
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.workspaces).toEqual(["ws-good"]);
  });

  it("returns workspaces sorted alphabetically", async () => {
    readdirMock.mockResolvedValue([
      dir("ws-zulu"),
      dir("ws-alpha"),
      dir("ws-mike"),
    ]);

    const res = await GET();
    const body = await res.json();
    expect(body.workspaces).toEqual(["ws-alpha", "ws-mike", "ws-zulu"]);
    expect(body.default).toBe("ws-alpha");
  });

  it("returns null default when no workspaces survive filtering", async () => {
    readdirMock.mockResolvedValue([dir("pw-w0-only-ci-artifact")]);

    const res = await GET();
    const body = await res.json();
    expect(body.workspaces).toEqual([]);
    expect(body.default).toBe(null);
  });

  it("returns 500 on unexpected fs errors", async () => {
    readdirMock.mockRejectedValue(new Error("EACCES: permission denied"));

    const res = await GET();
    expect(res.status).toBe(500);
    const body = await res.json();
    expect(body.ok).toBe(false);
    expect(body.error).toMatch(/failed to list workspaces/);
  });
});
