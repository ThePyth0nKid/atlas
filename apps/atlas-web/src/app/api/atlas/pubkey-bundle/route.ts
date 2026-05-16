/**
 * W20a — GET /api/atlas/pubkey-bundle?workspace={id}
 *
 * Return the canonical pubkey bundle for a workspace. This is the
 * exact byte stream the WASM verifier consumes to validate signatures
 * on the workspace's trace.
 *
 *   GET /api/atlas/pubkey-bundle?workspace={id}
 *
 *   200:  PubkeyBundle (raw JSON, NOT wrapped in {ok:true,...})
 *   400:  { ok: false, error: string }
 *   500:  { ok: false, error: string }
 *
 * Note: the 200 body is the bare bundle JSON — `{ schema, generated_at,
 * keys }` — not an envelope. The verifier expects the bundle as a
 * standalone document so its hash is recomputable byte-for-byte. The
 * 4xx/5xx error envelopes deliberately differ in shape; clients
 * dispatch on HTTP status, not body shape.
 *
 * Threat model:
 *   * `workspace` is validated via `isValidWorkspaceId` at the entry
 *     point (path-traversal structurally impossible).
 *   * The route calls `buildBundleForWorkspace`, which uses the
 *     PUBLIC-key derive path (`derivePubkeyViaSigner`) — the
 *     per-tenant secret never crosses the subprocess boundary on
 *     this route.
 *   * No caching (`cache-control: no-store`) — the bundle changes
 *     when the operator rotates legacy keys; we serve fresh bytes
 *     every time. Verification cost (~50ms spawn for the per-tenant
 *     pubkey derive) is acceptable given the route is hit once per
 *     dashboard mount.
 */

import "@/lib/bootstrap";

import { NextResponse } from "next/server";
import {
  buildBundleForWorkspace,
  redactPaths,
  SignerError,
  WorkspacePathError,
} from "@atlas/bridge";
import { isResponse, jsonError, requireWorkspace } from "../_lib/http";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(req: Request): Promise<NextResponse> {
  const workspace = requireWorkspace(req);
  if (isResponse(workspace)) return workspace;

  try {
    const bundle = await buildBundleForWorkspace(workspace);
    // Serialise the bundle ourselves so we control the exact bytes.
    // `NextResponse.json` would also work, but going via a hand-built
    // `new NextResponse(JSON.stringify(...))` makes the byte-stream
    // contract explicit at the call site — the verifier hashes these
    // bytes (after re-canonicalisation in Rust), so any silent change
    // in serialisation would be a verification regression.
    const body = JSON.stringify(bundle);
    return new NextResponse(body, {
      status: 200,
      headers: {
        "content-type": "application/json",
        "cache-control": "no-store",
      },
    });
  } catch (e) {
    if (e instanceof WorkspacePathError) {
      return jsonError(400, redactPaths(e.message));
    }
    if (e instanceof SignerError) {
      return jsonError(500, `signer: ${redactPaths(e.message)}`);
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `unexpected: ${redactPaths(msg)}`);
  }
}
