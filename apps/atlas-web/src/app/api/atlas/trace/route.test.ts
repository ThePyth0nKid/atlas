/**
 * W20a — GET /api/atlas/trace handler tests.
 *
 * Verifies the canonical trace shape:
 *   - 200 with full AtlasTrace structure for valid workspace
 *   - workspace_id is echoed back as path-validated
 *   - empty workspace returns events:[] + bundle hash
 *   - 400 on invalid workspace
 *   - 400 when workspace query is missing
 *   - 500 with redacted error on signer failure
 */

import { describe, it, expect, beforeEach, vi } from "vitest";

const { readAllEventsMock, buildBundleMock, bundleHashMock } = vi.hoisted(
  () => ({
    readAllEventsMock: vi.fn(),
    buildBundleMock: vi.fn(),
    bundleHashMock: vi.fn(),
  }),
);

vi.mock("@/lib/bootstrap", () => ({}));

vi.mock("@atlas/bridge", async () => {
  const actual =
    await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
  return {
    ...actual,
    readAllEvents: readAllEventsMock,
    buildBundleForWorkspace: buildBundleMock,
    bundleHashViaSigner: bundleHashMock,
  };
});

import { GET } from "./route";

beforeEach(() => {
  readAllEventsMock.mockReset();
  buildBundleMock.mockReset();
  bundleHashMock.mockReset();
});

const SAMPLE_EVENT = {
  event_id: "01HRQ001GENESIS",
  event_hash: "ba0e720697362eb396a4d76dbfcb4e4ca7e468c8b91ff6a9c56f65f5f3c0069c",
  parent_hashes: [] as string[],
  payload: { type: "node.create", node: { id: "first", kind: "dataset" } },
  signature: {
    alg: "EdDSA" as const,
    kid: "atlas-anchor:ws-test",
    sig: "efPza_TB8GvKwF4xoAZLSl_VFT7U7AIGH9tbJe1A-rxyBZXXRCbvWig7hdQYtUrOKndxhGWUH8CtbNyJeVKGCg",
  },
  ts: "2026-05-16T08:43:46Z",
};

const SAMPLE_BUNDLE_HASH =
  "8a6f96e961d02141aae6ec9b3a799e16903e5fdb38e43ee3e50e0fb9d7d41b77";

describe("GET /api/atlas/trace", () => {
  it("returns 200 with full AtlasTrace shape for a workspace with one event", async () => {
    readAllEventsMock.mockResolvedValue([SAMPLE_EVENT]);
    buildBundleMock.mockResolvedValue({
      schema: "atlas-pubkey-bundle-v1",
      generated_at: "2026-01-01T00:00:00Z",
      keys: { "atlas-anchor:ws-test": "abc" },
    });
    bundleHashMock.mockResolvedValue(SAMPLE_BUNDLE_HASH);

    const res = await GET(
      new Request("http://localhost/api/atlas/trace?workspace=ws-test"),
    );
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toMatch(/application\/json/);

    const body = await res.json();
    expect(body.schema_version).toBe("atlas-trace-v1");
    expect(body.workspace_id).toBe("ws-test");
    expect(body.pubkey_bundle_hash).toBe(SAMPLE_BUNDLE_HASH);
    expect(body.events).toHaveLength(1);
    expect(body.events[0].event_id).toBe(SAMPLE_EVENT.event_id);
    expect(body.dag_tips).toEqual([SAMPLE_EVENT.event_hash]);
    expect(body.anchors).toEqual([]);
    expect(body.policies).toEqual([]);
    expect(body.filters).toBe(null);
    expect(typeof body.generated_at).toBe("string");
  });

  it("returns 200 with empty events when workspace has no events", async () => {
    readAllEventsMock.mockResolvedValue([]);
    buildBundleMock.mockResolvedValue({
      schema: "atlas-pubkey-bundle-v1",
      generated_at: "2026-01-01T00:00:00Z",
      keys: {},
    });
    bundleHashMock.mockResolvedValue(SAMPLE_BUNDLE_HASH);

    const res = await GET(
      new Request("http://localhost/api/atlas/trace?workspace=ws-empty"),
    );
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.events).toEqual([]);
    expect(body.dag_tips).toEqual([]);
    expect(body.pubkey_bundle_hash).toBe(SAMPLE_BUNDLE_HASH);
    expect(body.workspace_id).toBe("ws-empty");
  });

  it("returns 400 when workspace query parameter is missing", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/trace"),
    );
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.ok).toBe(false);
  });

  it("returns 400 when workspace fails the regex (path traversal)", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/trace?workspace=..%2Fetc"),
    );
    expect(res.status).toBe(400);
  });

  it("returns 400 when workspace contains a space", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/trace?workspace=foo%20bar"),
    );
    expect(res.status).toBe(400);
  });

  it("returns 500 when the bundle-hash signer call fails", async () => {
    readAllEventsMock.mockResolvedValue([]);
    buildBundleMock.mockResolvedValue({
      schema: "atlas-pubkey-bundle-v1",
      generated_at: "2026-01-01T00:00:00Z",
      keys: {},
    });
    const { SignerError } =
      await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
    bundleHashMock.mockRejectedValue(new SignerError("signer binary missing"));

    const res = await GET(
      new Request("http://localhost/api/atlas/trace?workspace=ws-test"),
    );
    expect(res.status).toBe(500);
    const body = await res.json();
    expect(body.error).toMatch(/signer/);
  });
});
