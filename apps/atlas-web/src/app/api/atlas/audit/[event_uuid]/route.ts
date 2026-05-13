/**
 * V2-β Welle 12 — GET /api/atlas/audit/[event_uuid]
 *
 * Return the full signed AtlasEvent for the given `event_uuid`
 * along with a signature-verification status. Workspace-scoped.
 *
 * V2-β Phase 4 ships the **API contract surface**; full signature
 * verification (via the wasm verifier) is deferred to Phase 7 / W17
 * when the persistent backend lands. For now we return the event
 * verbatim plus `{ signature_verified: "deferred" }` so clients can
 * branch on the status field without breaking when the wasm verify
 * path lights up.
 *
 *   GET /api/atlas/audit/{event_uuid}?workspace={workspace_id}
 *
 *   200:  { ok: true, event: AtlasEvent, signature_verified: "deferred" | "valid" | "invalid" }
 *   400:  { ok: false, error: string }
 *   404:  { ok: false, error: string }
 *   500:  { ok: false, error: string }
 */

import "@/lib/bootstrap";

import { NextResponse } from "next/server";
import { EventsJsonlProjectionStore } from "../../_lib/projection-store";
import {
  handleStoreError,
  isResponse,
  jsonError,
  requireWorkspace,
} from "../../_lib/http";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const store = new EventsJsonlProjectionStore();

export async function GET(
  req: Request,
  ctx: { params: Promise<{ event_uuid: string }> },
): Promise<NextResponse> {
  const workspace = requireWorkspace(req);
  if (isResponse(workspace)) return workspace;

  const { event_uuid } = await ctx.params;
  if (typeof event_uuid !== "string" || event_uuid.length === 0) {
    return jsonError(400, "event_uuid is required");
  }
  if (event_uuid.length > 256) {
    return jsonError(400, "event_uuid is too long");
  }

  try {
    const event = await store.getEvent(workspace, event_uuid);
    if (event === null) {
      return jsonError(404, `event not found: ${event_uuid}`);
    }
    return NextResponse.json({
      ok: true as const,
      event,
      signature_verified: "deferred" as const,
    });
  } catch (e) {
    return handleStoreError(e);
  }
}
