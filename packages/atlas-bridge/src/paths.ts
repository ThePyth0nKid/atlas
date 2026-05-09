/**
 * Path resolution that survives different invocation modes.
 *
 * The bridge can be loaded from:
 *   - atlas-mcp-server: `pnpm dev` (cwd = apps/atlas-mcp-server) or
 *     `node dist/index.js` after build (cwd = anywhere) or Claude
 *     Desktop config (cwd = wherever Claude launched from).
 *   - atlas-web: a Next.js server route handler running under
 *     `next dev` (cwd = apps/atlas-web) or compiled production output.
 *
 * We resolve everything relative to *this file's location* on disk,
 * not relative to cwd. After tsc, `import.meta.url` points at
 * `packages/atlas-bridge/dist/paths.js`; under `tsx` or Next's
 * `transpilePackages` it points at `packages/atlas-bridge/src/paths.ts`.
 * Both `dist/` and `src/` are exactly one level under the bridge root,
 * so the same `..` walk works.
 *
 * The bridge intentionally does NOT bake in an app-specific data
 * directory default — its package root is shared across consumers.
 * The MCP server stores per-developer data under
 * `apps/atlas-mcp-server/data/`; atlas-web defaults to
 * `apps/atlas-web/data/`. Each consumer calls `setDefaultDataDir()`
 * at startup with its preferred location, and `ATLAS_DATA_DIR` still
 * wins as the operator override.
 */

import { fileURLToPath } from "node:url";
import { dirname, join, relative, resolve, sep } from "node:path";
import { existsSync } from "node:fs";

const HERE = dirname(fileURLToPath(import.meta.url));
// {dist|src}/ → packages/atlas-bridge/
const BRIDGE_ROOT = resolve(HERE, "..");
// packages/atlas-bridge/ → packages/ → repo root
const REPO_ROOT = resolve(BRIDGE_ROOT, "..", "..");

/**
 * Allowlist of legal workspace-id characters. Tight on purpose — workspace
 * ids appear unescaped in filesystem paths, in the canonical signing-input,
 * and in audit-log identifiers. Restricting to `[a-zA-Z0-9_-]{1,128}` makes
 * path-traversal attacks (`../..`) and shell-metacharacter surprises
 * structurally impossible.
 */
const WORKSPACE_ID_RE = /^[a-zA-Z0-9_-]{1,128}$/;

export function isValidWorkspaceId(id: string): boolean {
  return WORKSPACE_ID_RE.test(id);
}

export class WorkspacePathError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WorkspacePathError";
  }
}

let configuredDefaultDataDir: string | null = null;

/**
 * Register a process-wide default data directory. The env var
 * `ATLAS_DATA_DIR` always wins; this fallback only takes effect when
 * the env var is unset. Each consumer (atlas-mcp-server, atlas-web)
 * calls this once at startup with its app-local default so two apps
 * sharing the bridge do not silently collide on the same data dir.
 *
 * If neither the env var nor a configured default is present,
 * `dataDir()` falls back to `<repo-root>/data` — useful for ad-hoc
 * scripts (e.g. `tsx` smoke runs) that import the bridge directly.
 */
export function setDefaultDataDir(dir: string): void {
  // V1.19 Welle 2 hardening: refuse to silently clobber a previously
  // registered default. Two consumers in the same process configuring
  // different data dirs is a misconfiguration that would otherwise
  // result in whichever bootstrap module ran last winning, with no
  // operator-visible signal. Idempotent calls (same dir twice) are
  // permitted because Node's module cache makes them benign — the
  // bootstrap module re-evaluation on hot-reload is the realistic
  // case here. Set ATLAS_DATA_DIR to override at the env layer.
  if (configuredDefaultDataDir !== null && configuredDefaultDataDir !== dir) {
    throw new Error(
      `setDefaultDataDir called twice with different values: ` +
        `first "${configuredDefaultDataDir}", now "${dir}". ` +
        `Use ATLAS_DATA_DIR to override at the env layer.`,
    );
  }
  configuredDefaultDataDir = dir;
}

export function dataDir(): string {
  if (process.env.ATLAS_DATA_DIR) return process.env.ATLAS_DATA_DIR;
  if (configuredDefaultDataDir !== null) return configuredDefaultDataDir;
  return join(REPO_ROOT, "data");
}

/**
 * Resolve `dataDir()/<workspaceId>` and verify the result stays under the
 * data root. The regex check above already eliminates `..` and separator
 * tokens, but we keep the post-resolve `relative()` test as defence in
 * depth — if an env-var future changes the data root or someone bypasses
 * the regex, this still refuses to escape.
 */
export function workspaceDir(workspaceId: string): string {
  if (!isValidWorkspaceId(workspaceId)) {
    throw new WorkspacePathError(
      `invalid workspace_id: must match ${WORKSPACE_ID_RE.source}`,
    );
  }
  const root = resolve(dataDir());
  const candidate = resolve(root, workspaceId);
  const rel = relative(root, candidate);
  if (rel.startsWith("..") || rel === "" || rel.includes(`..${sep}`)) {
    throw new WorkspacePathError(
      `workspace_id resolves outside data root: ${workspaceId}`,
    );
  }
  return candidate;
}

export function eventsLogPath(workspaceId: string): string {
  return join(workspaceDir(workspaceId), "events.jsonl");
}

/**
 * Path to the anchors snapshot file. Written atomically by the
 * `atlas_anchor_bundle` MCP tool, read by `exportWorkspaceBundle` to
 * populate `trace.anchors`. Absence is benign — the bundle simply ships
 * with `anchors: []` and the verifier passes the lenient default.
 */
export function anchorsPath(workspaceId: string): string {
  return join(workspaceDir(workspaceId), "anchors.json");
}

/**
 * V1.7 anchor-chain JSONL. One `AnchorBatch` per line, append-only
 * within the lifetime of a workspace. Written atomically (read-all +
 * tmp + rename + fsync) by the Rust signer when `--chain-path` is
 * passed; the MCP server reads but never modifies this file. Absence
 * is benign — `exportWorkspaceBundle` ships traces without a chain
 * and the lenient verifier passes them.
 */
export function anchorChainPath(workspaceId: string): string {
  return join(workspaceDir(workspaceId), "anchor-chain.jsonl");
}

/**
 * V1.19 Welle 4 hardening: the resolved binary path is cached with a
 * 60-second TTL rather than process-long. Long-running consumers
 * (Claude Desktop's MCP host, the production atlas-web Next.js server,
 * Vercel/Cloudflare workers) can outlive an operator-driven binary
 * swap by hours or days. The pre-Welle-4 process-long cache pinned a
 * stale path until restart — every subsequent sign call ENOENT'd until
 * the operator noticed and bounced the host. The TTL bounds that drift
 * window without re-introducing the per-write `existsSync` cost the
 * original cache existed to avoid: at most one filesystem probe per
 * minute per process, regardless of write rate.
 *
 * Both positive and negative results are TTL'd. Negative caching means
 * an operator who runs `cargo build --release` after the process
 * started sees the new binary picked up within 60s without restart.
 */
const SIGNER_BINARY_CACHE_TTL_MS = 60_000;

interface SignerBinaryCacheEntry {
  readonly path: string | null;
  readonly expiresAt: number;
}

let signerBinaryCache: SignerBinaryCacheEntry | null = null;

// Clock-injection seam for deterministic TTL tests. Production callers
// always observe `Date.now`; only `__signerBinaryCacheForTest.setClock`
// mutates this, and the helper restores it on `restoreClock`.
let signerCacheNow: () => number = Date.now;

/**
 * Resolve the `atlas-signer` binary. Preference order:
 *   1. ATLAS_SIGNER_PATH env var (lets ops point at a sealed-key build)
 *   2. target/release/atlas-signer{.exe} (release build)
 *   3. target/debug/atlas-signer{.exe} (dev build)
 *
 * Returns null if none exist; the caller surfaces a friendly error.
 *
 * Caching: results are cached with a 60-second TTL — see
 * `SIGNER_BINARY_CACHE_TTL_MS` above for rationale. Within the TTL
 * window the cached value is returned with no syscalls; after expiry
 * the resolution rebuilds from scratch, which also re-reads
 * `ATLAS_SIGNER_PATH` so env-var rotations take effect within one TTL.
 *
 * V1.19 Welle 1 web hardening (preserved through Welle 4): the env-var
 * override is verified with `existsSync` BEFORE caching. A
 * misconfigured `ATLAS_SIGNER_PATH` falls through to the workspace
 * candidates rather than pinning a bad path that ENOENTs every sign.
 */
export function resolveSignerBinary(): string | null {
  const now = signerCacheNow();
  if (signerBinaryCache !== null && signerBinaryCache.expiresAt > now) {
    return signerBinaryCache.path;
  }
  const resolved = doResolveSignerBinary();
  signerBinaryCache = {
    path: resolved,
    expiresAt: now + SIGNER_BINARY_CACHE_TTL_MS,
  };
  return resolved;
}

function doResolveSignerBinary(): string | null {
  if (process.env.ATLAS_SIGNER_PATH) {
    // V1.19 Welle 4 hardening: resolve to absolute BEFORE existsSync +
    // cache. The TTL means the resolved path is reused for up to 60s;
    // if a relative path were stored and the long-running process
    // changed cwd in the meantime (process.chdir, or a worker thread
    // with a different inherited cwd), the cached probe and the later
    // spawn would resolve against different bases. existsSync itself
    // is cwd-relative — the absolute resolve closes that gap and
    // makes the cache value carry the same meaning across the whole
    // TTL window regardless of cwd drift.
    const abs = resolve(process.env.ATLAS_SIGNER_PATH);
    if (existsSync(abs)) return abs;
    // Fall through to workspace candidates rather than failing hard —
    // operator may have set an old path; release/debug builds remain
    // a useful fallback.
  }
  const exe = process.platform === "win32" ? ".exe" : "";
  const candidates = [
    join(REPO_ROOT, "target", "release", `atlas-signer${exe}`),
    join(REPO_ROOT, "target", "debug", `atlas-signer${exe}`),
  ];
  for (const path of candidates) {
    if (existsSync(path)) return path;
  }
  return null;
}

/**
 * Test-only seam for the signer-binary cache. Production code MUST NOT
 * call these — the cache is a pure optimisation and the TTL constant
 * is part of the operational contract documented above. Exported so
 * the cache-behaviour test (`scripts/test-signer-binary-cache.ts`)
 * can drive cache state and a synthetic clock without sleeping or
 * relying on real wall-time.
 */
export const __signerBinaryCacheForTest = {
  TTL_MS: SIGNER_BINARY_CACHE_TTL_MS,
  reset(): void {
    signerBinaryCache = null;
  },
  setClock(fn: () => number): void {
    signerCacheNow = fn;
  },
  restoreClock(): void {
    signerCacheNow = Date.now;
  },
};

export function repoRoot(): string {
  return REPO_ROOT;
}
