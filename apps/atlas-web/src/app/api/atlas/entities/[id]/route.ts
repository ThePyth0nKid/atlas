/**
 * V2-β Welle 12 — GET /api/atlas/entities/[id]
 *
 * Return the projected entity for a given `entity_uuid` within the
 * supplied workspace. Returns 404 if the entity has not been observed
 * (no `node.create` event with `node.id === entity_uuid`).
 *
 * Backend: in-memory projection over events.jsonl (W12). Phase 7
 * (W17b) swaps this for ArcadeDB without changing the route.
 *
 * Wire format:
 *
 *   GET /api/atlas/entities/{entity_uuid}?workspace={workspace_id}
 *
 *   200:  { ok: true, entity: { entity_uuid, kind, properties, author_did, created_event_uuid, created_at } }
 *   400:  { ok: false, error: string }   (missing workspace, bad workspace id)
 *   404:  { ok: false, error: string }   (entity not found)
 *   500:  { ok: false, error: string }   (unexpected)
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
    const entity = await store.getEntity(workspace, id);
    if (entity === null) {
      return jsonError(404, `entity not found: ${id}`);
    }
    return NextResponse.json({ ok: true as const, entity });
  } catch (e) {
    return handleStoreError(e);
  }
}
