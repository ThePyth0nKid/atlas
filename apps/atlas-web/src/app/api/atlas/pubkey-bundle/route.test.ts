/**
 * W20a — GET /api/atlas/pubkey-bundle handler tests.
 *
 * Verifies the raw-bundle-body contract:
 *   - 200 with bare PubkeyBundle JSON (NOT wrapped in {ok:true,...})
 *   - 400 on invalid / missing workspace
 *   - 500 with redacted error on signer failure
 */

import { describe, it, expect, beforeEach, vi } from "vitest";

const { buildBundleMock } = vi.hoisted(() => ({
  buildBundleMock: vi.fn(),
}));

vi.mock("@/lib/bootstrap", () => ({}));

vi.mock("@atlas/bridge", async () => {
  const actual =
    await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
  return {
    ...actual,
    buildBundleForWorkspace: buildBundleMock,
  };
});

import { GET } from "./route";

beforeEach(() => {
  buildBundleMock.mockReset();
});

describe("GET /api/atlas/pubkey-bundle", () => {
  it("returns 200 with the bare PubkeyBundle (no ok envelope)", async () => {
    const bundle = {
      schema: "atlas-pubkey-bundle-v1" as const,
      generated_at: "2026-01-01T00:00:00Z",
      keys: { "atlas-anchor:ws-test": "abc" },
    };
    buildBundleMock.mockResolvedValue(bundle);

    const res = await GET(
      new Request("http://localhost/api/atlas/pubkey-bundle?workspace=ws-test"),
    );
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toMatch(/application\/json/);
    expect(res.headers.get("cache-control")).toBe("no-store");

    const body = await res.json();
    expect(body).toEqual(bundle);
    expect(body.ok).toBeUndefined();
  });

  it("returns 400 when workspace is missing", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/pubkey-bundle"),
    );
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.ok).toBe(false);
  });

  it("returns 400 when workspace fails the regex", async () => {
    const res = await GET(
      new Request(
        "http://localhost/api/atlas/pubkey-bundle?workspace=..%2Fetc",
      ),
    );
    expect(res.status).toBe(400);
  });

  it("returns 500 with signer-prefixed message on signer failure", async () => {
    const { SignerError } =
      await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
    buildBundleMock.mockRejectedValue(
      new SignerError("atlas-signer binary not found"),
    );

    const res = await GET(
      new Request("http://localhost/api/atlas/pubkey-bundle?workspace=ws-test"),
    );
    expect(res.status).toBe(500);
    const body = await res.json();
    expect(body.error).toMatch(/^signer:/);
  });
});
