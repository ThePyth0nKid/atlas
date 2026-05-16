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

const {
  readdirMock,
  statMock,
  rmMock,
  renameMock,
  dataDirMock,
  derivePubkeyMock,
  ensureWorkspaceDirMock,
} = vi.hoisted(() => ({
  readdirMock: vi.fn(),
  statMock: vi.fn(),
  rmMock: vi.fn(),
  renameMock: vi.fn(),
  dataDirMock: vi.fn(() => "/tmp/atlas-data"),
  derivePubkeyMock: vi.fn(),
  ensureWorkspaceDirMock: vi.fn(),
}));

vi.mock("@/lib/bootstrap", () => ({}));

vi.mock("node:fs", async () => {
  const actual = await vi.importActual<typeof import("node:fs")>("node:fs");
  // W20b-2 fix-commit (tdd-guide HIGH): default `stat` delegates to the
  // real implementation so the success / 409 paths keep exercising real
  // filesystem state. Individual tests reassign `statMock` to inject
  // error branches (WorkspacePathError, EACCES) without polluting other
  // tests.
  statMock.mockImplementation((p: string) => actual.promises.stat(p));
  // W20c — rm and rename default to real-impl delegation so rollback
  // and rename success paths exercise the real filesystem. Tests that
  // need synthetic error branches reassign via `mockRejectedValueOnce`.
  rmMock.mockImplementation((p: string, opts?: object) =>
    actual.promises.rm(p, opts as unknown as Parameters<typeof actual.promises.rm>[1]),
  );
  renameMock.mockImplementation((oldP: string, newP: string) =>
    actual.promises.rename(oldP, newP),
  );
  return {
    ...actual,
    promises: {
      ...actual.promises,
      readdir: readdirMock,
      stat: statMock,
      rm: rmMock,
      rename: renameMock,
    },
  };
});

vi.mock("@atlas/bridge", async () => {
  // Defer to the real module for `isValidWorkspaceId`, `redactPaths`,
  // `WORKSPACE_ID_RE`, `perTenantKidFor`, `workspaceDir`, etc., but stub
  // `dataDir` so tests don't need a real fs root, and stub
  // `derivePubkeyViaSigner` so tests don't shell out to the Rust binary.
  // W20b-2 fix-commit (tdd-guide HIGH): also expose
  // `ensureWorkspaceDirMock` so error-branch tests can inject
  // `WorkspacePathError` / `StorageError` rejections without spinning
  // up a real broken disk. Default delegates to the real impl so the
  // happy path keeps creating the dir on disk.
  const actual =
    await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
  ensureWorkspaceDirMock.mockImplementation((id: string) =>
    actual.ensureWorkspaceDir(id),
  );
  return {
    ...actual,
    dataDir: dataDirMock,
    derivePubkeyViaSigner: derivePubkeyMock,
    ensureWorkspaceDir: ensureWorkspaceDirMock,
  };
});

import { GET, POST, PATCH, DELETE } from "./route";

interface Dirent {
  name: string;
  isDirectory: () => boolean;
}

const dir = (name: string): Dirent => ({ name, isDirectory: () => true });
const file = (name: string): Dirent => ({ name, isDirectory: () => false });

beforeEach(async () => {
  readdirMock.mockReset();
  // W20c — readdir defaults to real-impl delegation so PATCH/DELETE
  // tests (which exercise the workspace-count gate) can run against
  // real tmpdir contents. GET-route tests explicitly reassign
  // readdirMock for their own fixtures.
  const actualFsForReaddir =
    await vi.importActual<typeof import("node:fs")>("node:fs");
  readdirMock.mockImplementation(
    (p: string, opts?: object) =>
      actualFsForReaddir.promises.readdir(
        p,
        opts as unknown as Parameters<typeof actualFsForReaddir.promises.readdir>[1],
      ) as unknown as Promise<string[]>,
  );
  dataDirMock.mockReset();
  dataDirMock.mockReturnValue("/tmp/atlas-data");
  derivePubkeyMock.mockReset();
  // W20b-2 fix-commit: re-hydrate the stat + ensureWorkspaceDir
  // defaults so each test gets fresh real-impl delegation. Tests that
  // need a synthetic error branch reassign these via
  // `*Mock.mockImplementationOnce(...)` or `mockRejectedValueOnce`.
  const actualFs = await vi.importActual<typeof import("node:fs")>("node:fs");
  statMock.mockReset();
  statMock.mockImplementation((p: string) => actualFs.promises.stat(p));
  rmMock.mockReset();
  rmMock.mockImplementation((p: string, opts?: object) =>
    actualFs.promises.rm(
      p,
      opts as unknown as Parameters<typeof actualFs.promises.rm>[1],
    ),
  );
  renameMock.mockReset();
  renameMock.mockImplementation((oldP: string, newP: string) =>
    actualFs.promises.rename(oldP, newP),
  );
  const actualBridge =
    await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
  ensureWorkspaceDirMock.mockReset();
  ensureWorkspaceDirMock.mockImplementation((id: string) =>
    actualBridge.ensureWorkspaceDir(id),
  );
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
    // W20b-2 fix-commit (tdd-guide HIGH, finding #5): the mock returns
    // a DIFFERENT kid than `perTenantKidFor("ws-fresh")` would compute,
    // so the assertion below proves the route uses `perTenantKidFor`
    // (the canonical source) rather than echoing `derived.kid`. Without
    // this gap-closer the success test would pass even if a future
    // refactor swapped the two sources, because both produced the same
    // string in the original test setup.
    derivePubkeyMock.mockResolvedValue({
      kid: "WRONG-kid-from-signer",
      pubkey_b64url: "abcd-base64url",
    });
    const res = await POST(post({ workspace_id: "ws-fresh" }));
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.workspace_id).toBe("ws-fresh");
    expect(body.kid).toBe("atlas-anchor:ws-fresh");
    // Intent-explicit: the route MUST NOT echo the signer's kid. The
    // canonical kid is `perTenantKidFor(workspaceId)`.
    expect(body.kid).not.toBe("WRONG-kid-from-signer");
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

  // ───────── Error-branch coverage (W20b-2 fix-commit, tdd-guide HIGH) ─────────

  it("returns 400 when fs.stat throws WorkspacePathError", async () => {
    // Construct a synthetic WorkspacePathError from `fs.stat`. In
    // production this could surface if a future bridge refactor
    // delegated stat through a bridge-level path-validating helper;
    // the route catch-block handles it generically. The mock injects
    // the error directly to exercise that branch.
    const { WorkspacePathError } =
      await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
    statMock.mockRejectedValueOnce(
      new WorkspacePathError("workspace_id resolves outside data root"),
    );
    const res = await POST(post({ workspace_id: "ws-stat-path-err" }));
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.ok).toBe(false);
    expect(body.error).toMatch(/workspace_id resolves outside data root/);
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 500 with redacted path when fs.stat fails with non-ENOENT error", async () => {
    // EACCES is a representative non-ENOENT errno — production
    // operators see this when the data root is owned by another user
    // and the Next.js process lacks read perms. The raw `fs.stat`
    // error embeds the absolute path verbatim; the route MUST run it
    // through `redactPaths` before serialising into the 500 response.
    const eaccesErr = Object.assign(
      new Error("EACCES: permission denied, stat '/sensitive/path/ws-x'"),
      { code: "EACCES" },
    );
    statMock.mockRejectedValueOnce(eaccesErr);
    const res = await POST(post({ workspace_id: "ws-stat-eacces" }));
    expect(res.status).toBe(500);
    const body = await res.json();
    expect(body.error).toMatch(/^stat:/);
    // Absolute path MUST have been stripped — operator filesystem
    // layout is not a client-visible surface.
    expect(body.error).not.toContain("/sensitive/path/ws-x");
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 400 when ensureWorkspaceDir throws WorkspacePathError", async () => {
    const { WorkspacePathError } =
      await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
    ensureWorkspaceDirMock.mockRejectedValueOnce(
      new WorkspacePathError("workspace_id resolves outside data root"),
    );
    const res = await POST(post({ workspace_id: "ws-mkdir-path-err" }));
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toMatch(/workspace_id resolves outside data root/);
    // The route bails BEFORE deriving the pubkey when the
    // ensureWorkspaceDir step fails.
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 500 with redacted path when ensureWorkspaceDir throws StorageError", async () => {
    // Pairs with the redactPaths-on-StorageError defence-in-depth fix
    // (finding #6). Without that fix the absolute path in the error
    // message would have been echoed verbatim into the 500 response.
    const { StorageError } =
      await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
    ensureWorkspaceDirMock.mockRejectedValueOnce(
      new StorageError("storage failed at /secret/path/x"),
    );
    const res = await POST(post({ workspace_id: "ws-mkdir-storage-err" }));
    expect(res.status).toBe(500);
    const body = await res.json();
    expect(body.error).toMatch(/^storage:/);
    expect(body.error).not.toContain("/secret/path/x");
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 400 with sanitized message on unrecognized keys", async () => {
    // W20b-2 fix-commit (security-reviewer MEDIUM, finding #9): the
    // `.strict()` Zod schema rejects extra keys with a message that
    // embeds attacker-controlled key names verbatim. The route MUST
    // collapse that to a static message so a log-pipeline rendering
    // attacker-controlled `<script>` content cannot trip up an
    // unattended ingestor. The response is JSON-encoded (XSS-safe),
    // but the static message is defence-in-depth at the log layer.
    const res = await POST(
      post({ workspace_id: "ok", "<script>alert(1)</script>": "x" }),
    );
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.ok).toBe(false);
    expect(body.error).toBe("invalid input: body contains unexpected keys");
    // The attacker-supplied key name MUST NOT have been echoed.
    expect(body.error).not.toContain("<script>");
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  // ───────── W20c (DA-1): signer-failure triggers atomic rollback ─────────

  it("rolls back the freshly-created workspace dir when the signer fails", async () => {
    // Setup: signer fails AFTER mkdir. The route must `fs.rm` the
    // freshly-created dir before returning 500 — eliminating the
    // orphan "unconfigured" workspace UX that ba4e27f introduced and
    // security-reviewer flagged in PR #113.
    const { SignerError } =
      await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
    derivePubkeyMock.mockRejectedValue(
      new SignerError("ATLAS_DEV_MASTER_SEED unset"),
    );
    const res = await POST(post({ workspace_id: "ws-orphan-test" }));
    expect(res.status).toBe(500);
    const body = await res.json();
    expect(body.error).toMatch(/^signer:/);
    // The freshly-mkdir'd directory must no longer exist on disk.
    await expect(
      realFs.stat(join(tmpRoot, "ws-orphan-test")),
    ).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("returns 500 partial_rollback when fs.rm fails after signer failure", async () => {
    // The rare case: signer failed, but `fs.rm` also failed (e.g.
    // concurrent process held a handle on Windows). Surface a typed
    // `partial_rollback:` prefix so operators can clean up.
    const { SignerError } =
      await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
    derivePubkeyMock.mockRejectedValue(
      new SignerError("ATLAS_DEV_MASTER_SEED unset"),
    );
    rmMock.mockRejectedValueOnce(
      Object.assign(new Error("EBUSY: resource busy or locked"), { code: "EBUSY" }),
    );
    const res = await POST(post({ workspace_id: "ws-rm-fails" }));
    expect(res.status).toBe(500);
    const body = await res.json();
    expect(body.error).toMatch(/^partial_rollback:/);
    expect(body.error).toMatch(/signer failed/);
  });

  it("does NOT roll back when ensureWorkspaceDir fails (dir was never created)", async () => {
    // If mkdir fails, there's nothing to roll back. The route must NOT
    // call `fs.rm` (no orphan can exist).
    const { StorageError } =
      await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
    ensureWorkspaceDirMock.mockRejectedValueOnce(
      new StorageError("storage failed"),
    );
    rmMock.mockRejectedValue(new Error("rm should not have been called"));
    const res = await POST(post({ workspace_id: "ws-mkdir-fails" }));
    expect(res.status).toBe(500);
    expect(rmMock).not.toHaveBeenCalled();
  });
});

// ─────────────────── PATCH /api/atlas/workspaces ───────────────────

describe("PATCH /api/atlas/workspaces", () => {
  let tmpRoot: string;
  let originalDataDirEnv: string | undefined;

  beforeEach(() => {
    tmpRoot = mkdtempSync(join(tmpdir(), "atlas-ws-patch-test-"));
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

  const patch = (body: unknown): Request =>
    new Request("http://localhost/api/atlas/workspaces", {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });

  it("returns 400 when the old id fails the regex", async () => {
    const res = await PATCH(
      patch({ workspace_id: "bad space", new_workspace_id: "ws-new" }),
    );
    expect(res.status).toBe(400);
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 400 when the new id fails the regex", async () => {
    const res = await PATCH(
      patch({ workspace_id: "ws-old", new_workspace_id: "bad space" }),
    );
    expect(res.status).toBe(400);
    expect(derivePubkeyMock).not.toHaveBeenCalled();
  });

  it("returns 400 when old and new ids are identical", async () => {
    const res = await PATCH(
      patch({ workspace_id: "ws-same", new_workspace_id: "ws-same" }),
    );
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toMatch(/must differ/);
  });

  it("returns 404 when the source workspace does not exist", async () => {
    const res = await PATCH(
      patch({ workspace_id: "ws-missing", new_workspace_id: "ws-new" }),
    );
    expect(res.status).toBe(404);
    const body = await res.json();
    expect(body.error).toBe("workspace not found");
  });

  it("returns 409 when the target id already exists", async () => {
    await realFs.mkdir(join(tmpRoot, "ws-src"));
    await realFs.mkdir(join(tmpRoot, "ws-target"));
    const res = await PATCH(
      patch({ workspace_id: "ws-src", new_workspace_id: "ws-target" }),
    );
    expect(res.status).toBe(409);
    const body = await res.json();
    expect(body.error).toBe("workspace already exists");
  });

  it("renames the directory and returns the new kid + pubkey on success", async () => {
    await realFs.mkdir(join(tmpRoot, "ws-old"));
    derivePubkeyMock.mockResolvedValue({
      kid: "WRONG-kid-from-signer",
      pubkey_b64url: "renamed-pubkey",
    });
    const res = await PATCH(
      patch({ workspace_id: "ws-old", new_workspace_id: "ws-renamed" }),
    );
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.workspace_id).toBe("ws-renamed");
    expect(body.kid).toBe("atlas-anchor:ws-renamed");
    expect(body.kid).not.toBe("WRONG-kid-from-signer");
    expect(body.pubkey_b64url).toBe("renamed-pubkey");
    // Old dir gone; new dir present.
    await expect(realFs.stat(join(tmpRoot, "ws-old"))).rejects.toMatchObject({
      code: "ENOENT",
    });
    const newStat = await realFs.stat(join(tmpRoot, "ws-renamed"));
    expect(newStat.isDirectory()).toBe(true);
    expect(derivePubkeyMock).toHaveBeenCalledWith("ws-renamed");
  });

  it("returns 500 cross_mount_rename_unsupported on EXDEV", async () => {
    await realFs.mkdir(join(tmpRoot, "ws-src"));
    renameMock.mockRejectedValueOnce(
      Object.assign(new Error("EXDEV: cross-device link"), { code: "EXDEV" }),
    );
    const res = await PATCH(
      patch({ workspace_id: "ws-src", new_workspace_id: "ws-new" }),
    );
    expect(res.status).toBe(500);
    const body = await res.json();
    expect(body.error).toMatch(/cross_mount_rename_unsupported/);
  });

  it("returns 413 on oversized Content-Length", async () => {
    const oversized = (4 * 1024 + 1).toString();
    const req = new Request("http://localhost/api/atlas/workspaces", {
      method: "PATCH",
      headers: {
        "content-type": "application/json",
        "content-length": oversized,
      },
      body: JSON.stringify({ workspace_id: "a", new_workspace_id: "b" }),
    });
    const res = await PATCH(req);
    expect(res.status).toBe(413);
  });

  it("returns 400 sanitized message on unrecognized keys", async () => {
    const res = await PATCH(
      patch({
        workspace_id: "ws-x",
        new_workspace_id: "ws-y",
        "<script>": "x",
      }),
    );
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toBe("invalid input: body contains unexpected keys");
  });
});

// ─────────────────── DELETE /api/atlas/workspaces ───────────────────

describe("DELETE /api/atlas/workspaces", () => {
  let tmpRoot: string;
  let originalDataDirEnv: string | undefined;

  beforeEach(() => {
    tmpRoot = mkdtempSync(join(tmpdir(), "atlas-ws-delete-test-"));
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

  const del = (body: unknown): Request =>
    new Request("http://localhost/api/atlas/workspaces", {
      method: "DELETE",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });

  it("returns 400 when the id fails the regex", async () => {
    const res = await DELETE(del({ workspace_id: "bad space" }));
    expect(res.status).toBe(400);
  });

  it("returns 404 when the workspace does not exist", async () => {
    // Mkdir a second workspace so the last-workspace gate would not
    // even apply to the deleted (non-existent) one.
    await realFs.mkdir(join(tmpRoot, "ws-other"));
    const res = await DELETE(del({ workspace_id: "ws-missing" }));
    expect(res.status).toBe(404);
    const body = await res.json();
    expect(body.error).toBe("workspace not found");
  });

  it("refuses to delete the last user-facing workspace (409)", async () => {
    await realFs.mkdir(join(tmpRoot, "ws-only"));
    const res = await DELETE(del({ workspace_id: "ws-only" }));
    expect(res.status).toBe(409);
    const body = await res.json();
    expect(body.error).toBe("cannot delete last workspace");
    // Dir must still exist — the 409 is a hard refusal.
    const stat = await realFs.stat(join(tmpRoot, "ws-only"));
    expect(stat.isDirectory()).toBe(true);
  });

  it("deletes the workspace directory when 2+ exist", async () => {
    await realFs.mkdir(join(tmpRoot, "ws-keep"));
    await realFs.mkdir(join(tmpRoot, "ws-delete"));
    const res = await DELETE(del({ workspace_id: "ws-delete" }));
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.workspace_id).toBe("ws-delete");
    await expect(
      realFs.stat(join(tmpRoot, "ws-delete")),
    ).rejects.toMatchObject({ code: "ENOENT" });
    // Other workspace untouched.
    const kept = await realFs.stat(join(tmpRoot, "ws-keep"));
    expect(kept.isDirectory()).toBe(true);
  });

  it("returns 413 on oversized Content-Length", async () => {
    const oversized = (4 * 1024 + 1).toString();
    const req = new Request("http://localhost/api/atlas/workspaces", {
      method: "DELETE",
      headers: {
        "content-type": "application/json",
        "content-length": oversized,
      },
      body: JSON.stringify({ workspace_id: "a" }),
    });
    const res = await DELETE(req);
    expect(res.status).toBe(413);
  });

  it("returns 400 sanitized message on unrecognized keys", async () => {
    const res = await DELETE(
      del({ workspace_id: "ok", "<script>": "x" }),
    );
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toBe("invalid input: body contains unexpected keys");
  });
});
