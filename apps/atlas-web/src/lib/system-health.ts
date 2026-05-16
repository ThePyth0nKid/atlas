/**
 * W20c — Layer-3 honest status: pure probe helpers + types.
 *
 * Three lightweight, env-only probes for the L3 stack:
 *   * `probeSigner`   — operational when ATLAS_DEV_MASTER_SEED is set
 *                       (and a signer binary is resolvable).
 *   * `probeEmbedder` — operational when an embedder is wired; stub
 *                       returns 'unsupported' until V2-γ ships the real
 *                       inference path (the Rust crate is in tree but
 *                       not yet wired through the JS bridge).
 *   * `probeBackend`  — operational when the configured backend exists
 *                       (atlas-mcp-server is the canonical writer);
 *                       'stub_501' for semantic-search until V2-γ;
 *                       'fault' on misconfiguration.
 *
 * Probes do NOT spawn the signer or any other child process — they
 * read env vars and use `resolveSignerBinary` (which itself only
 * checks file existence, no spawn). This keeps the dashboard probe
 * latency bounded to filesystem stat (~ms) instead of subprocess
 * start (~tens of ms).
 *
 * Caching:
 *   The route layer (`/api/atlas/system/health`) wraps these probes
 *   with a 60-second TTL in-memory cache. The probes themselves are
 *   pure (no side effects beyond env reads) so the cache is safe.
 */

import { resolveSignerBinary } from "@atlas/bridge";

/** Discriminated status types — string-literal unions per coding-style.md. */
export type EmbedderStatus = "operational" | "model_missing" | "unsupported";
export type BackendStatus = "operational" | "stub_501" | "fault";
export type SignerStatus = "operational" | "unconfigured";

export interface LayerStatus {
  /** L3 embedder (currently always 'unsupported' until V2-γ wires fastembed). */
  embedder: EmbedderStatus;
  /** L3 backend service (Atlas MCP server or semantic-search). */
  backend: BackendStatus;
  /** L3 signer (Rust `atlas-signer` binary + dev master seed). */
  signer: SignerStatus;
}

/**
 * Optional override hook for tests / Playwright fixtures. Honored ONLY
 * when `ATLAS_E2E_TEST_HOOKS === "1"` is set in the process env. The
 * hook lets a Playwright spec force a specific signer/embedder/backend
 * value without mutating filesystem state.
 */
export interface ProbeOverride {
  signer?: SignerStatus;
  embedder?: EmbedderStatus;
  backend?: BackendStatus;
}

/**
 * W20c-specific test-hook env-var name. Mirrors the e2e fixture's
 * `forceSignerUnconfigured` helper. Set to "1" in `playwright.config.ts`
 * so production deployments cannot have probe-overrides flipped from
 * an attacker-controlled header.
 */
export const TEST_HOOK_ENV = "ATLAS_E2E_TEST_HOOKS";

export function testHooksEnabled(env: NodeJS.ProcessEnv = process.env): boolean {
  return env[TEST_HOOK_ENV] === "1";
}

/**
 * Probe the signer status.
 *
 *   * 'operational'  → ATLAS_DEV_MASTER_SEED env var set AND binary
 *                       resolvable.
 *   * 'unconfigured' → either missing seed or missing binary.
 *
 * Note: 'operational' here means the signer CAN run, not that any
 * specific signing call has succeeded. A live spawn-and-validate probe
 * would catch run-time failures (mis-permissioned binary, etc.) but
 * costs ~10ms and would defeat the dashboard's low-latency promise.
 * The trade-off favours static probing — the LiveVerifierPanel will
 * surface a downstream failure if signing actually breaks.
 */
export function probeSigner(
  env: NodeJS.ProcessEnv = process.env,
): SignerStatus {
  const seed = env.ATLAS_DEV_MASTER_SEED;
  if (seed === undefined || seed === "") {
    return "unconfigured";
  }
  const bin = resolveSignerBinary();
  if (bin === null) {
    return "unconfigured";
  }
  return "operational";
}

/**
 * Probe the embedder status.
 *
 * V2-β-1: the bge-small-en-v1.5 model lives in the Rust crate but is
 * not wired through the TS bridge. Always returns 'unsupported' to
 * surface that honestly. V2-γ flips this to a real probe (model
 * artifact present + dimensions verified).
 *
 *   * `ATLAS_EMBEDDER=disabled` (default) → 'unsupported'
 *   * `ATLAS_EMBEDDER=fastembed`           → 'model_missing' or
 *                                             'operational' depending on
 *                                             whether the artifact root
 *                                             exists.
 */
export function probeEmbedder(
  env: NodeJS.ProcessEnv = process.env,
): EmbedderStatus {
  const mode = env.ATLAS_EMBEDDER;
  if (mode === undefined || mode === "" || mode === "disabled") {
    return "unsupported";
  }
  if (mode === "fastembed") {
    // V2-β-1: even when the operator opts into fastembed, we do not
    // actually wire it through the JS bridge yet. Mark as
    // `model_missing` so the UI surfaces it as "not yet operational"
    // rather than claiming 'operational' on an env-var that doesn't
    // do anything yet. V2-γ adds the real artifact existence check.
    return "model_missing";
  }
  return "unsupported";
}

/**
 * Probe the backend status.
 *
 *   * 'operational' — semantic-search wired (V2-γ flag set)
 *   * 'stub_501'    — default; semantic-search returns 501 not implemented
 *   * 'fault'       — explicit misconfiguration flag
 */
export function probeBackend(
  env: NodeJS.ProcessEnv = process.env,
): BackendStatus {
  const mode = env.ATLAS_BACKEND_MODE;
  if (mode === undefined || mode === "" || mode === "stub") {
    return "stub_501";
  }
  if (mode === "operational") {
    return "operational";
  }
  if (mode === "fault") {
    return "fault";
  }
  // Unknown values fall back to stub_501 — fail closed.
  return "stub_501";
}

/**
 * Compose the full LayerStatus, honoring test-hook overrides when
 * enabled.
 */
export function buildLayerStatus(
  override: ProbeOverride = {},
  env: NodeJS.ProcessEnv = process.env,
): LayerStatus {
  const hooksOn = testHooksEnabled(env);
  return {
    signer: hooksOn && override.signer !== undefined ? override.signer : probeSigner(env),
    embedder:
      hooksOn && override.embedder !== undefined ? override.embedder : probeEmbedder(env),
    backend:
      hooksOn && override.backend !== undefined ? override.backend : probeBackend(env),
  };
}

// ─────────────────────── In-memory cache ───────────────────────

/**
 * 60-second TTL cache for the assembled LayerStatus block. The probes
 * themselves are cheap but cache-skipping is unnecessary work for the
 * dashboard, which polls on each cold-load. The TTL is short enough
 * that an env-var change (operator setting ATLAS_DEV_MASTER_SEED)
 * takes effect within one minute.
 *
 * Cache is process-local — Next.js routes are stateless but in the
 * same Node process; this is safe.
 */
const CACHE_TTL_MS = 60_000;

interface CacheEntry {
  status: LayerStatus;
  expiresAt: number;
}

let cache: CacheEntry | null = null;
let cacheNow: () => number = Date.now;

export function getCachedLayerStatus(
  override: ProbeOverride = {},
  env: NodeJS.ProcessEnv = process.env,
): LayerStatus {
  const now = cacheNow();
  // Test-hook overrides bypass cache so spec-by-spec hooks don't bleed
  // across tests in the same worker.
  if (testHooksEnabled(env) && override.signer !== undefined) {
    return buildLayerStatus(override, env);
  }
  if (cache !== null && cache.expiresAt > now) {
    return cache.status;
  }
  const status = buildLayerStatus(override, env);
  cache = { status, expiresAt: now + CACHE_TTL_MS };
  return status;
}

/** Test-only helpers — frozen surface for vitest. */
export const __systemHealthCacheForTest = {
  setClock(now: () => number): void {
    cacheNow = now;
  },
  restoreClock(): void {
    cacheNow = Date.now;
  },
  reset(): void {
    cache = null;
  },
  CACHE_TTL_MS,
};
