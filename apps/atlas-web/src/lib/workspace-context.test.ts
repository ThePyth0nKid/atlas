/**
 * W20a — Regex parity guard between the bridge and the client mirror.
 *
 * `apps/atlas-web/src/lib/workspace-context.tsx` carries a duplicate
 * of the bridge's `WORKSPACE_ID_RE` because the bridge module is
 * server-only (it imports `node:fs` at load time). Drift between the
 * two copies is a silent UX bug: a workspace id that passes the
 * client check but fails the server check renders a confusing 400
 * after the optimistic UI update.
 *
 * This test fails the moment the two regexes diverge — pattern or
 * flags. The fix is always to bring the client mirror back into sync
 * with the bridge source-of-truth (or vice versa, with a deliberate
 * coordinated change).
 *
 * W20b-2 — `requestCreateWorkspace` helper coverage.
 *
 * Unit-tests the pure helper extracted from `createWorkspace` so the
 * fetch + validation contract is locked without a DOM render harness
 * (vitest runs in `node` env). The provider wraps this helper with
 * React state updates; the helper itself is the seam where the wire
 * shape, regex fast-path, and error envelope live.
 */

import { describe, it, expect, vi } from "vitest";
import { WORKSPACE_ID_RE as bridgeRe } from "@atlas/bridge";
import {
  WORKSPACE_ID_RE as clientRe,
  requestCreateWorkspace,
} from "./workspace-context";

describe("WORKSPACE_ID_RE regex parity", () => {
  it("client regex matches bridge regex byte-for-byte (source)", () => {
    expect(clientRe.source).toBe(bridgeRe.source);
  });

  it("client regex matches bridge regex (flags)", () => {
    expect(clientRe.flags).toBe(bridgeRe.flags);
  });

  it("client regex toString matches bridge regex toString", () => {
    expect(clientRe.toString()).toBe(bridgeRe.toString());
  });
});

describe("requestCreateWorkspace", () => {
  const happyResponse = (): Response =>
    new Response(
      JSON.stringify({
        ok: true,
        workspace_id: "ws-x",
        kid: "atlas-anchor:ws-x",
        pubkey_b64url: "p",
      }),
      { status: 200, headers: { "content-type": "application/json" } },
    );

  it("returns ok:true on a 200 success response", async () => {
    const fetchFn = vi.fn().mockResolvedValue(happyResponse());
    const result = await requestCreateWorkspace("ws-fresh", fetchFn);
    expect(result).toEqual({ ok: true });
    expect(fetchFn).toHaveBeenCalledTimes(1);
    const [url, init] = fetchFn.mock.calls[0];
    expect(url).toBe("/api/atlas/workspaces");
    expect(init?.method).toBe("POST");
    expect(JSON.parse(init?.body as string)).toEqual({
      workspace_id: "ws-fresh",
    });
  });

  it("rejects regex-failing ids WITHOUT calling fetch", async () => {
    const fetchFn = vi.fn();
    const result = await requestCreateWorkspace("bad space", fetchFn);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toMatch(/invalid workspace id/);
    }
    expect(fetchFn).not.toHaveBeenCalled();
  });

  it("surfaces server error on 409 conflict", async () => {
    const fetchFn = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({ ok: false, error: "workspace already exists" }),
        { status: 409 },
      ),
    );
    const result = await requestCreateWorkspace("ws-dup", fetchFn);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("workspace already exists");
    }
  });

  it("surfaces generic message when server omits the error field", async () => {
    const fetchFn = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ ok: false }), { status: 500 }),
    );
    const result = await requestCreateWorkspace("ws-x", fetchFn);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("create failed");
    }
  });

  it("surfaces generic message when the response body is not JSON", async () => {
    const fetchFn = vi.fn().mockResolvedValue(
      new Response("<html>504 gateway timeout</html>", { status: 504 }),
    );
    const result = await requestCreateWorkspace("ws-x", fetchFn);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      // Must NOT contain raw HTML — that would let a misconfigured
      // proxy leak its error page into the dashboard error block.
      expect(result.error).not.toContain("<html>");
      expect(result.error).toMatch(/create failed/);
    }
  });

  it("returns the network error message when fetch throws", async () => {
    const fetchFn = vi.fn().mockRejectedValue(new Error("network down"));
    const result = await requestCreateWorkspace("ws-x", fetchFn);
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).toBe("network down");
    }
  });
});
