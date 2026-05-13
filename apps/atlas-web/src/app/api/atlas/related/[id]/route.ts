/**
 * V2-β Welle 12 — GET /api/atlas/related/[id]
 *
 * Return outgoing + incoming edges for an entity. Returns 404 if
 * the entity has not been observed.
 *
 *   GET /api/atlas/related/{entity_uuid}?workspace={workspace_id}
 *
 *   200:  { ok: true, outgoing: ProjectedEdge[], incoming: ProjectedEdge[] }
 *   400:  { ok: false, error: string }
 *   404:  { ok: false, error: string }   (entity not found)
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
  ctx: { params: Promise<{ id: string }> },
): Promise<NextResponse> {
  const workspace = requireWorkspace(req);
  if (isResponse(workspace)) return workspace;

  const { id } = await ctx.params;
  if (typeof id !== "string" || id.length === 0) {
    return jsonError(400, "entity id is required");
  }
  if (id.length > 256) {
    return jsonError(400, "entity id is too long");
  }

  try {
    const related = await store.getRelated(workspace, id);
    if (related === null) {
      return jsonError(404, `entity not found: ${id}`);
    }
    return NextResponse.json({
      ok: true as const,
      outgoing: related.outgoing,
      incoming: related.incoming,
    });
  } catch (e) {
    return handleStoreError(e);
  }
}
