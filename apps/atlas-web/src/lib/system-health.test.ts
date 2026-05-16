/**
 * W20c — Vitest unit tests for `system-health` probes.
 *
 * Asserts the env-only probe contracts:
 *   * probeSigner branches on ATLAS_DEV_MASTER_SEED + binary resolution
 *   * probeEmbedder branches on ATLAS_EMBEDDER
 *   * probeBackend branches on ATLAS_BACKEND_MODE with fail-closed default
 *   * Test-hook overrides honored only when ATLAS_E2E_TEST_HOOKS=1
 *   * Cache: TTL 60s, override bypass, manual reset
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

const { resolveSignerBinaryMock } = vi.hoisted(() => ({
  resolveSignerBinaryMock: vi.fn<() => string | null>(),
}));

vi.mock("@atlas/bridge", async () => {
  const actual =
    await vi.importActual<typeof import("@atlas/bridge")>("@atlas/bridge");
  return {
    ...actual,
    resolveSignerBinary: resolveSignerBinaryMock,
  };
});

import {
  buildLayerStatus,
  getCachedLayerStatus,
  probeBackend,
  probeEmbedder,
  probeSigner,
  testHooksEnabled,
  __systemHealthCacheForTest,
} from "./system-health";

beforeEach(() => {
  resolveSignerBinaryMock.mockReset();
  resolveSignerBinaryMock.mockReturnValue("/tmp/fake-signer");
  __systemHealthCacheForTest.reset();
});

afterEach(() => {
  __systemHealthCacheForTest.restoreClock();
});

describe("probeSigner", () => {
  it("returns 'operational' when seed and binary are both present", () => {
    const env = { ATLAS_DEV_MASTER_SEED: "1" } as unknown as NodeJS.ProcessEnv;
    expect(probeSigner(env)).toBe("operational");
  });

  it("returns 'unconfigured' when the seed env-var is unset", () => {
    const env = {} as unknown as NodeJS.ProcessEnv;
    expect(probeSigner(env)).toBe("unconfigured");
  });

  it("returns 'unconfigured' when the seed env-var is the empty string", () => {
    const env = { ATLAS_DEV_MASTER_SEED: "" } as unknown as NodeJS.ProcessEnv;
    expect(probeSigner(env)).toBe("unconfigured");
  });

  it("returns 'unconfigured' when the signer binary is not resolvable", () => {
    resolveSignerBinaryMock.mockReturnValue(null);
    const env = { ATLAS_DEV_MASTER_SEED: "1" } as unknown as NodeJS.ProcessEnv;
    expect(probeSigner(env)).toBe("unconfigured");
  });
});

describe("probeEmbedder", () => {
  it("returns 'unsupported' when ATLAS_EMBEDDER is unset", () => {
    expect(probeEmbedder({} as unknown as NodeJS.ProcessEnv)).toBe("unsupported");
  });

  it("returns 'unsupported' when ATLAS_EMBEDDER=disabled", () => {
    expect(
      probeEmbedder({ ATLAS_EMBEDDER: "disabled" } as unknown as NodeJS.ProcessEnv),
    ).toBe("unsupported");
  });

  it("returns 'model_missing' when ATLAS_EMBEDDER=fastembed (V2-β-1 stub)", () => {
    expect(
      probeEmbedder({ ATLAS_EMBEDDER: "fastembed" } as unknown as NodeJS.ProcessEnv),
    ).toBe("model_missing");
  });

  it("returns 'unsupported' for unknown ATLAS_EMBEDDER values", () => {
    expect(
      probeEmbedder({ ATLAS_EMBEDDER: "huggingface" } as unknown as NodeJS.ProcessEnv),
    ).toBe("unsupported");
  });
});

describe("probeBackend", () => {
  it("returns 'stub_501' when ATLAS_BACKEND_MODE is unset (fail-closed)", () => {
    expect(probeBackend({} as unknown as NodeJS.ProcessEnv)).toBe("stub_501");
  });

  it("returns 'stub_501' when ATLAS_BACKEND_MODE=stub", () => {
    expect(
      probeBackend({ ATLAS_BACKEND_MODE: "stub" } as unknown as NodeJS.ProcessEnv),
    ).toBe("stub_501");
  });

  it("returns 'operational' when ATLAS_BACKEND_MODE=operational", () => {
    expect(
      probeBackend({
        ATLAS_BACKEND_MODE: "operational",
      } as unknown as NodeJS.ProcessEnv),
    ).toBe("operational");
  });

  it("returns 'fault' when ATLAS_BACKEND_MODE=fault", () => {
    expect(
      probeBackend({ ATLAS_BACKEND_MODE: "fault" } as unknown as NodeJS.ProcessEnv),
    ).toBe("fault");
  });

  it("falls back to 'stub_501' on unknown ATLAS_BACKEND_MODE values", () => {
    expect(
      probeBackend({
        ATLAS_BACKEND_MODE: "nonsense",
      } as unknown as NodeJS.ProcessEnv),
    ).toBe("stub_501");
  });
});

describe("testHooksEnabled", () => {
  it("is true ONLY when ATLAS_E2E_TEST_HOOKS=1", () => {
    expect(testHooksEnabled({ ATLAS_E2E_TEST_HOOKS: "1" } as unknown as NodeJS.ProcessEnv)).toBe(
      true,
    );
    expect(testHooksEnabled({ ATLAS_E2E_TEST_HOOKS: "0" } as unknown as NodeJS.ProcessEnv)).toBe(
      false,
    );
    expect(testHooksEnabled({ ATLAS_E2E_TEST_HOOKS: "true" } as unknown as NodeJS.ProcessEnv)).toBe(
      false,
    );
    expect(testHooksEnabled({} as unknown as NodeJS.ProcessEnv)).toBe(false);
  });
});

describe("buildLayerStatus", () => {
  it("honors override.signer when test hooks are enabled", () => {
    const env = {
      ATLAS_DEV_MASTER_SEED: "1",
      ATLAS_E2E_TEST_HOOKS: "1",
    } as unknown as NodeJS.ProcessEnv;
    const status = buildLayerStatus({ signer: "unconfigured" }, env);
    expect(status.signer).toBe("unconfigured");
  });

  it("IGNORES override.signer when test hooks are NOT enabled", () => {
    const env = { ATLAS_DEV_MASTER_SEED: "1" } as unknown as NodeJS.ProcessEnv;
    const status = buildLayerStatus({ signer: "unconfigured" }, env);
    // Hooks off → real probe wins.
    expect(status.signer).toBe("operational");
  });

  it("composes all three probes into one block", () => {
    const env = {
      ATLAS_DEV_MASTER_SEED: "1",
      ATLAS_EMBEDDER: "fastembed",
      ATLAS_BACKEND_MODE: "operational",
    } as unknown as NodeJS.ProcessEnv;
    expect(buildLayerStatus({}, env)).toEqual({
      signer: "operational",
      embedder: "model_missing",
      backend: "operational",
    });
  });
});

describe("getCachedLayerStatus", () => {
  it("caches the first call's result within the TTL", () => {
    let nowMs = 1_000_000;
    __systemHealthCacheForTest.setClock(() => nowMs);
    const env = { ATLAS_DEV_MASTER_SEED: "1" } as unknown as NodeJS.ProcessEnv;

    const first = getCachedLayerStatus({}, env);
    expect(first.signer).toBe("operational");

    // Flip the env mid-TTL — the cache must still serve the old value.
    resolveSignerBinaryMock.mockReturnValue(null);
    nowMs += 30_000;
    const cached = getCachedLayerStatus({}, env);
    expect(cached.signer).toBe("operational");
  });

  it("evicts and re-probes after the TTL expires", () => {
    let nowMs = 1_000_000;
    __systemHealthCacheForTest.setClock(() => nowMs);
    const env = { ATLAS_DEV_MASTER_SEED: "1" } as unknown as NodeJS.ProcessEnv;

    const first = getCachedLayerStatus({}, env);
    expect(first.signer).toBe("operational");

    resolveSignerBinaryMock.mockReturnValue(null);
    // Step past the TTL boundary.
    nowMs += 61_000;
    const refreshed = getCachedLayerStatus({}, env);
    expect(refreshed.signer).toBe("unconfigured");
  });

  it("bypasses the cache when a test-hook override is supplied", () => {
    let nowMs = 1_000_000;
    __systemHealthCacheForTest.setClock(() => nowMs);
    const baseEnv = {
      ATLAS_DEV_MASTER_SEED: "1",
      ATLAS_E2E_TEST_HOOKS: "1",
    } as unknown as NodeJS.ProcessEnv;

    // Seed the cache with a real-probe value.
    const first = getCachedLayerStatus({}, baseEnv);
    expect(first.signer).toBe("operational");

    // Now request with an override — must NOT serve the cached value.
    const overridden = getCachedLayerStatus(
      { signer: "unconfigured" },
      baseEnv,
    );
    expect(overridden.signer).toBe("unconfigured");
  });
});
