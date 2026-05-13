/**
 * V2-β Welle 12 — small shared HTTP helpers for the Read-API routes.
 *
 * Intentionally tiny — only the seams that recur across all six
 * routes (workspace-id validation, ISO-8601 parsing, error
 * envelope). Anything bigger goes in its own module.
 */

import { NextResponse } from "next/server";
import {
  isValidWorkspaceId,
  redactPaths,
  StorageError,
  WorkspacePathError,
} from "@atlas/bridge";

export const WORKSPACE_ID_DESCRIPTION = "workspace_id: only [a-zA-Z0-9_-], 1-128 chars";

/**
 * Build a JSON error envelope. Mirrors the shape used by
 * `write-node/route.ts` so client code can switch on `ok`.
 */
export function jsonError(status: number, message: string): NextResponse {
  return NextResponse.json({ ok: false as const, error: message }, { status });
}

/**
 * Extract a `workspace` query parameter from the request and validate
 * it. Returns either a 400 NextResponse or the validated id.
 */
export function requireWorkspace(
  req: Request,
  paramName: "workspace" | "workspace_id" = "workspace",
): NextResponse | string {
  const url = new URL(req.url);
  const value = url.searchParams.get(paramName);
  if (value === null) {
    return jsonError(400, `missing ${paramName} query parameter`);
  }
  if (!isValidWorkspaceId(value)) {
    return jsonError(400, WORKSPACE_ID_DESCRIPTION);
  }
  return value;
}

/**
 * Parse an ISO-8601 timestamp into epoch-millis. Returns NaN on
 * unparseable input — callers branch on `Number.isFinite`.
 */
export function parseIsoMs(value: string | null): number | null {
  if (value === null) return null;
  const ms = Date.parse(value);
  return Number.isFinite(ms) ? ms : NaN;
}

/**
 * Type guard for "this was a response, not a workspace id".
 */
export function isResponse(v: unknown): v is NextResponse {
  return v instanceof NextResponse;
}

/**
 * Map a thrown error from the projection-store / bridge layer to
 * the appropriate JSON error envelope. Applies `redactPaths` to the
 * catch-all so a kernel-surfaced filesystem error cannot leak the
 * server's absolute path layout to the client.
 *
 * Centralised here so every read route gets identical error
 * handling; matches the conventions already used by `write-node`.
 */
export function handleStoreError(e: unknown): NextResponse {
  if (e instanceof WorkspacePathError) {
    return jsonError(400, e.message);
  }
  if (e instanceof StorageError) {
    return jsonError(500, `storage: ${e.message}`);
  }
  const msg = e instanceof Error ? e.message : String(e);
  return jsonError(500, `unexpected: ${redactPaths(msg)}`);
}
