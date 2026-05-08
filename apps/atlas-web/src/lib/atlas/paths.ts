/**
 * V1.19 Welle 1 — atlas-web filesystem paths.
 *
 * DUPLICATED FROM `apps/atlas-mcp-server/src/lib/paths.ts` with two
 * deliberate adjustments for the web context:
 *
 *   1. The default `dataDir()` resolves to `apps/atlas-web/data/`
 *      rather than `apps/atlas-mcp-server/data/`. To share storage
 *      with the MCP server, set `ATLAS_DATA_DIR` to a shared location
 *      in BOTH processes. See OPERATOR-RUNBOOK §17.
 *
 *   2. `resolveSignerBinary()` walks five parents up from this file
 *      to find the repo root: src/lib/atlas/ → src/lib/ → src/ →
 *      apps/atlas-web/ → apps/ → repo. Internally split into
 *      `PACKAGE_ROOT` (3 parents up, points at apps/atlas-web/) and
 *      `REPO_ROOT` (2 more, points at the repo). The MCP variant is
 *      one level deeper because of an extra `lib/` segment.
 *
 * The workspace-id allowlist regex `[a-zA-Z0-9_-]{1,128}` is byte-
 * identical to the MCP variant — workspace ids appear in canonical
 * signing-input AND in filesystem paths, so any drift would silently
 * break verification AND open path-traversal vectors. Defence in depth:
 * the post-resolve `relative()` test refuses to escape the data root
 * even if the regex were ever loosened.
 */

import { fileURLToPath } from "node:url";
import { dirname, join, relative, resolve, sep } from "node:path";
import { existsSync } from "node:fs";

const HERE = dirname(fileURLToPath(import.meta.url));
// src/lib/atlas/ → src/lib/ → src/ → apps/atlas-web/
const PACKAGE_ROOT = resolve(HERE, "..", "..", "..");
// apps/atlas-web/ → apps/ → repo root
const REPO_ROOT = resolve(PACKAGE_ROOT, "..", "..");

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

let cachedSignerBinary: string | null | undefined = undefined;

/**
 * Resolve the `atlas-signer` binary. Preference order:
 *   1. ATLAS_SIGNER_PATH env var (explicit override)
 *   2. target/release/atlas-signer{.exe} (release build)
 *   3. target/debug/atlas-signer{.exe} (dev build)
 *
 * Returns null if none exist; the caller surfaces a friendly error.
 */
export function resolveSignerBinary(): string | null {
  if (cachedSignerBinary !== undefined) return cachedSignerBinary;
  if (process.env.ATLAS_SIGNER_PATH) {
    // Verify the override actually points at an existing binary
    // BEFORE caching. Otherwise a misconfigured ATLAS_SIGNER_PATH
    // would surface as a generic spawn ENOENT on the first signer
    // call and never recover (the cache pinned `null` semantically),
    // forcing a process restart. Caching only after a successful
    // existsSync makes the next request retry against the workspace
    // candidates instead.
    if (existsSync(process.env.ATLAS_SIGNER_PATH)) {
      cachedSignerBinary = process.env.ATLAS_SIGNER_PATH;
      return cachedSignerBinary;
    }
    // Fall through to the workspace candidates rather than failing
    // hard — operator may have set an old path; release/debug builds
    // remain a useful fallback.
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
