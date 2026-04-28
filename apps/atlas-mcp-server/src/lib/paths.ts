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
import { dirname, join, relative, resolve, sep } from "node:path";
import { existsSync } from "node:fs";

const HERE = dirname(fileURLToPath(import.meta.url));
// src/lib/ → src/ → apps/atlas-mcp-server/
const PACKAGE_ROOT = resolve(HERE, "..", "..");
// apps/atlas-mcp-server/ → apps/ → repo root
const REPO_ROOT = resolve(PACKAGE_ROOT, "..", "..");

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

export function dataDir(): string {
  return process.env.ATLAS_DATA_DIR ?? join(PACKAGE_ROOT, "data");
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

let cachedSignerBinary: string | null | undefined = undefined;

/**
 * Resolve the `atlas-signer` binary. Preference order:
 *   1. ATLAS_SIGNER_PATH env var (lets ops point at a sealed-key build)
 *   2. target/release/atlas-signer{.exe} (release build)
 *   3. target/debug/atlas-signer{.exe} (dev build)
 *
 * Returns null if none exist; the caller surfaces a friendly error. The
 * result is memoised — the binary location does not change at runtime
 * and `existsSync` should not appear in every signed-write hot path.
 */
export function resolveSignerBinary(): string | null {
  if (cachedSignerBinary !== undefined) return cachedSignerBinary;
  if (process.env.ATLAS_SIGNER_PATH) {
    cachedSignerBinary = process.env.ATLAS_SIGNER_PATH;
    return cachedSignerBinary;
  }
  const exe = process.platform === "win32" ? ".exe" : "";
  const candidates = [
    join(REPO_ROOT, "target", "release", `atlas-signer${exe}`),
    join(REPO_ROOT, "target", "debug", `atlas-signer${exe}`),
  ];
  for (const path of candidates) {
    if (existsSync(path)) {
      cachedSignerBinary = path;
      return path;
    }
  }
  cachedSignerBinary = null;
  return null;
}

export function repoRoot(): string {
  return REPO_ROOT;
}
