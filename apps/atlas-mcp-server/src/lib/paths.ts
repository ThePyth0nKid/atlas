/**
 * Path resolution that survives different invocation modes.
 *
 * The MCP server can be launched from:
 *   - `pnpm dev` (cwd = apps/atlas-mcp-server)
 *   - `node dist/index.js` after build (cwd = anywhere)
 *   - Claude Desktop config (cwd = wherever Claude launched from)
 *
 * We resolve everything relative to *this file's location* on disk,
 * not relative to cwd. Override paths via env vars if you need to
 * isolate dev data, or to point at a different signer binary.
 */

import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";
import { existsSync } from "node:fs";

const HERE = dirname(fileURLToPath(import.meta.url));
// src/lib/ → src/ → apps/atlas-mcp-server/
const PACKAGE_ROOT = resolve(HERE, "..", "..");
// apps/atlas-mcp-server/ → apps/ → repo root
const REPO_ROOT = resolve(PACKAGE_ROOT, "..", "..");

export function dataDir(): string {
  return process.env.ATLAS_DATA_DIR ?? join(PACKAGE_ROOT, "data");
}

export function workspaceDir(workspaceId: string): string {
  return join(dataDir(), workspaceId);
}

export function eventsLogPath(workspaceId: string): string {
  return join(workspaceDir(workspaceId), "events.jsonl");
}

/**
 * Resolve the `atlas-signer` binary. Preference order:
 *   1. ATLAS_SIGNER_PATH env var (lets ops point at a sealed-key build)
 *   2. target/release/atlas-signer{.exe} (release build)
 *   3. target/debug/atlas-signer{.exe} (dev build)
 *
 * Returns null if none exist; the caller surfaces a friendly error.
 */
export function resolveSignerBinary(): string | null {
  if (process.env.ATLAS_SIGNER_PATH) {
    return process.env.ATLAS_SIGNER_PATH;
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

export function repoRoot(): string {
  return REPO_ROOT;
}
