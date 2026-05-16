/**
 * W20a — GET /api/atlas/workspaces handler tests.
 *
 * Asserts the listing + filtering contract:
 *   - Empty data root → 200 with empty list, default = null.
 *   - Mixed directory entries → only directories are returned.
 *   - CI-pattern names (`pw-w*-…`) are stripped.
 *   - Workspace ids that fail the regex are stripped.
 *   - Results are sorted alphabetically for stable UI.
 *
 * W20b-2 — POST /api/atlas/workspaces handler tests.
 *
 * Asserts the create-workspace contract:
 *   - 400 on missing body / invalid JSON / regex fail / extra keys.
 *   - 413 on oversized Content-Length.
 *   - 409 when the directory already exists.
 *   - 200 on success — directory created on disk, response carries
 *     the derived kid + pubkey.
 *   - 500 on signer failure (path-redacted).
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { promises as realFs } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const { readdirMock, dataDirMock, derivePubkeyMock } = vi.hoisted(() => ({
  readdirMock: vi.fn(),
  dataDirMock: vi.fn(() => "/tmp/atlas-data"),
  derivePubkeyMock: vi.fn(),
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
  // Defer to the real module for `isValidWorkspaceId`, `redactPaths`,
  // `WORKSPACE_ID_RE`, `perTenantKidFor`, `workspaceDir`, etc., but stub
  // `dataDir` so tests don't need a real fs root, and stub
  // `derivePubkeyViaSigner` so tests don't shell out to the Rust binary.
  const actual =
    await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
  return {
    ...actual,
    dataDir: dataDirMock,
    derivePubkeyViaSigner: derivePubkeyMock,
  };
});

import { GET, POST } from "./route";

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
  derivePubkeyMock.mockReset();
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

// ─────────────────── POST /api/atlas/workspaces ───────────────────

describe("POST /api/atlas/workspaces", () => {
  let tmpRoot: string;
  let originalDataDirEnv: string | undefined;

  beforeEach(() => {
    // Each test gets its own scratch data dir so directory state is
    // isolated. The bridge's `workspaceDir(id)` resolves against
    // `dataDir()`, which is governed by the `ATLAS_DATA_DIR` env var
    // — setting that wins over both the mocked `dataDir` export AND
    // the bootstrap-registered default, so the route's internal
    // `workspaceDir` (the actual bridge implementation, not the
    // mocked surface) lands inside the tmp tree.
    tmpRoot = mkdtempSync(join(tmpdir(), "atlas-ws-post-test-"));
    dataDirMock.mockReturnValue(tmpRoot);
    originalDataDirEnv = process.env.ATLAS_DATA_DIR;
    process.env.ATLAS_DATA_DIR = tmpRoot;
  });

  afterEach(() => {
    if (originalDataDirEnv === undefined) {
      delete process.env.ATLAS_DATA_DIR;
    } else {
      process.env.ATLAS_DATA_DIR = originalDataDirEnv;
    }
    rmSync(tmpRoot, { recursive: true, force: true });
  });

  const post = (
    body: unknown,
    init: { stringify?: boolean; contentLength?: string } = {},
  ): Request => {
    const headers: Record<string, string> = {
      "content-type": "application/json",
    };
    const raw =
      init.stringify === false ? (body as string) : JSON.stringify(body);
    if (init.contentLength !== undefined) {
      headers["content-length"] = init.contentLength;
    }
    return new Request("http://localhost/api/atlas/workspaces", {
      method: "POST",
      headers,
      body: raw,
    });
  };

  it("returns 400 on invalid JSON body", async () => {
    derivePubkeyMock.mockResolvedValue({
      kid: "atlas-anchor:ws-x",
      pubkey_b64url: "p",
    });
    const req = post("not-json{", { stringify: false });
    const res = await POST(req);
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.ok).toBe(false);
    expect(body.error).toMatch(/not valid JSON/);
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 400 on missing workspace_id", async () => {
    const res = await POST(post({}));
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toMatch(/invalid input/);
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 400 on workspace_id failing the regex", async () => {
    const res = await POST(post({ workspace_id: "bad space" }));
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toMatch(/invalid input/);
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 400 on extra keys (.strict)", async () => {
    const res = await POST(
      post({ workspace_id: "ws-ok", malicious: true }),
    );
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toMatch(/invalid input/);
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 413 on oversized Content-Length", async () => {
    const oversized = (4 * 1024 + 1).toString();
    const res = await POST(
      post({ workspace_id: "ws-big" }, { contentLength: oversized }),
    );
    expect(res.status).toBe(413);
    const body = await res.json();
    expect(body.error).toMatch(/exceeds/);
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 409 when the workspace directory already exists", async () => {
    await realFs.mkdir(join(tmpRoot, "ws-already"));
    const res = await POST(post({ workspace_id: "ws-already" }));
    expect(res.status).toBe(409);
    const body = await res.json();
    expect(body.error).toBe("workspace already exists");
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("creates the workspace and returns 200 with derived kid + pubkey", async () => {
    derivePubkeyMock.mockResolvedValue({
      kid: "atlas-anchor:ws-fresh",
      pubkey_b64url: "abcd-base64url",
    });
    const res = await POST(post({ workspace_id: "ws-fresh" }));
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.workspace_id).toBe("ws-fresh");
    expect(body.kid).toBe("atlas-anchor:ws-fresh");
    expect(body.pubkey_b64url).toBe("abcd-base64url");
    const stat = await realFs.stat(join(tmpRoot, "ws-fresh"));
    expect(stat.isDirectory()).toBe(true);
    expect(derivePubkeyMock).toHaveBeenCalledWith("ws-fresh");
  });

  it("returns 500 with redacted path when the signer fails", async () => {
    const { SignerError } =
      await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
    derivePubkeyMock.mockRejectedValue(
      new SignerError(
        "ATLAS_DEV_MASTER_SEED unset; refusing to derive at /home/op/secrets/x",
      ),
    );
    const res = await POST(post({ workspace_id: "ws-no-seed" }));
    expect(res.status).toBe(500);
    const body = await res.json();
    expect(body.error).toMatch(/^signer:/);
    // redactPaths must have stripped the absolute path; the segment
    // should no longer carry the secrets dir verbatim.
    expect(body.error).not.toContain("/home/op/secrets/x");
  });
});
