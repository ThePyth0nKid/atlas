/**
 * W20c — GET /api/atlas/system/health route tests.
 *
 * Asserts the response shape + status enum values + cache hit on
 * repeated calls + test-hook override honored only when ATLAS_E2E_TEST_HOOKS=1.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { resolveSignerBinaryMock } = vi.hoisted(() => ({
  resolveSignerBinaryMock: vi.fn<() => string | null>(),
}));

vi.mock("@/lib/bootstrap", () => ({}));

vi.mock("@atlas/bridge", async () => {
  const actual =
    await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
  return {
    ...actual,
    resolveSignerBinary: resolveSignerBinaryMock,
  };
});

import { GET } from "./route";
import { __systemHealthCacheForTest } from "@/lib/system-health";

const ORIGINAL_ENV = { ...process.env };

function makeReq(headers: Record<string, string> = {}): Request {
  return new Request("http://localhost/api/atlas/system/health", {
    method: "GET",
    headers,
  });
}

beforeEach(() => {
  resolveSignerBinaryMock.mockReset();
  resolveSignerBinaryMock.mockReturnValue("/tmp/fake-signer");
  __systemHealthCacheForTest.reset();
  // Reset env. Tests opt-in via direct mutation.
  for (const key of Object.keys(process.env)) {
    if (
      key.startsWith("ATLAS_DEV_MASTER_SEED") ||
      key.startsWith("ATLAS_EMBEDDER") ||
      key.startsWith("ATLAS_BACKEND_MODE") ||
      key.startsWith("ATLAS_E2E_TEST_HOOKS")
    ) {
      delete process.env[key];
    }
  }
});

afterEach(() => {
  __systemHealthCacheForTest.restoreClock();
  // Restore env.
  for (const key of [
    "ATLAS_DEV_MASTER_SEED",
    "ATLAS_EMBEDDER",
    "ATLAS_BACKEND_MODE",
    "ATLAS_E2E_TEST_HOOKS",
  ]) {
    if (ORIGINAL_ENV[key] === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = ORIGINAL_ENV[key];
    }
  }
});

describe("GET /api/atlas/system/health", () => {
  it("returns 200 with the full status block", async () => {
    process.env.ATLAS_DEV_MASTER_SEED = "1";
    const res = await GET(makeReq());
    expect(res.status).toBe(200);
    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(body.signer).toBe("operational");
    expect(body.embedder).toBe("unsupported");
    expect(body.backend).toBe("stub_501");
  });

  it("returns 'unconfigured' signer when seed is missing", async () => {
    const res = await GET(makeReq());
    const body = await res.json();
    expect(body.signer).toBe("unconfigured");
  });

  it("status values are from the documented enums only", async () => {
    process.env.ATLAS_DEV_MASTER_SEED = "1";
    process.env.ATLAS_EMBEDDER = "fastembed";
    process.env.ATLAS_BACKEND_MODE = "operational";
    const res = await GET(makeReq());
    const body = await res.json();
    expect(["operational", "unconfigured"]).toContain(body.signer);
    expect(["operational", "model_missing", "unsupported"]).toContain(
      body.embedder,
    );
    expect(["operational", "stub_501", "fault"]).toContain(body.backend);
  });

  it("caches results: second call within TTL serves the same value even after env flip", async () => {
    let nowMs = 1_000_000;
    __systemHealthCacheForTest.setClock(() => nowMs);
    process.env.ATLAS_DEV_MASTER_SEED = "1";
    const first = await (await GET(makeReq())).json();
    expect(first.signer).toBe("operational");

    delete process.env.ATLAS_DEV_MASTER_SEED;
    resolveSignerBinaryMock.mockReturnValue(null);
    nowMs += 30_000;
    const second = await (await GET(makeReq())).json();
    expect(second.signer).toBe("operational");
  });

  it("re-probes after the cache TTL expires", async () => {
    let nowMs = 1_000_000;
    __systemHealthCacheForTest.setClock(() => nowMs);
    process.env.ATLAS_DEV_MASTER_SEED = "1";
    const first = await (await GET(makeReq())).json();
    expect(first.signer).toBe("operational");

    delete process.env.ATLAS_DEV_MASTER_SEED;
    nowMs += 61_000;
    const refreshed = await (await GET(makeReq())).json();
    expect(refreshed.signer).toBe("unconfigured");
  });

  it("IGNORES the x-atlas-test-force-signer header when test-hooks are off", async () => {
    process.env.ATLAS_DEV_MASTER_SEED = "1";
    const res = await GET(
      makeReq({ "x-atlas-test-force-signer": "unconfigured" }),
    );
    const body = await res.json();
    expect(body.signer).toBe("operational");
  });

  it("HONORS the x-atlas-test-force-signer header when ATLAS_E2E_TEST_HOOKS=1", async () => {
    process.env.ATLAS_DEV_MASTER_SEED = "1";
    process.env.ATLAS_E2E_TEST_HOOKS = "1";
    const res = await GET(
      makeReq({ "x-atlas-test-force-signer": "unconfigured" }),
    );
    const body = await res.json();
    expect(body.signer).toBe("unconfigured");
  });

  it("rejects unknown header values (no override applied)", async () => {
    process.env.ATLAS_DEV_MASTER_SEED = "1";
    process.env.ATLAS_E2E_TEST_HOOKS = "1";
    const res = await GET(makeReq({ "x-atlas-test-force-signer": "nonsense" }));
    const body = await res.json();
    expect(body.signer).toBe("operational");
  });
});
