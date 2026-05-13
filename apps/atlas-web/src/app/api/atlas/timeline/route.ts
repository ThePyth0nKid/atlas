/**
 * V2-β Welle 12 — GET /api/atlas/timeline
 *
 * Return events for a workspace within an optional [from, to)
 * time window.
 *
 *   GET /api/atlas/timeline?workspace={id}&from={iso}&to={iso}&limit={n}
 *
 *   200:  { ok: true, events: ProjectedTimelineEvent[] }
 *   400:  { ok: false, error: string }   (missing/bad workspace, bad ISO, bad limit)
 *   500:  { ok: false, error: string }
 *
 * Limit defaults to 50, hard-capped at 500 per dispatch spec.
 */

import "@/lib/bootstrap";

import { NextResponse } from "next/server";
import { EventsJsonlProjectionStore } from "../_lib/projection-store";
import {
  handleStoreError,
  isResponse,
  jsonError,
  parseIsoMs,
  requireWorkspace,
} from "../_lib/http";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const DEFAULT_LIMIT = 50;
const MAX_LIMIT = 500;

const store = new EventsJsonlProjectionStore();

export async function GET(req: Request): Promise<NextResponse> {
  const workspace = requireWorkspace(req);
  if (isResponse(workspace)) return workspace;

  const url = new URL(req.url);

  // Validate `from`/`to` ISO-8601 windows. Empty → unbounded.
  const fromRaw = url.searchParams.get("from");
  const toRaw = url.searchParams.get("to");
  const fromMs = parseIsoMs(fromRaw);
  const toMs = parseIsoMs(toRaw);
  if (Number.isNaN(fromMs)) {
    return jsonError(400, `invalid 'from' (must be ISO-8601): ${fromRaw}`);
  }
  if (Number.isNaN(toMs)) {
    return jsonError(400, `invalid 'to' (must be ISO-8601): ${toRaw}`);
  }

  // Validate limit.
  const limitRaw = url.searchParams.get("limit");
  let limit = DEFAULT_LIMIT;
  if (limitRaw !== null) {
    const parsed = Number(limitRaw);
    if (!Number.isInteger(parsed) || parsed < 1) {
      return jsonError(400, `invalid 'limit' (must be a positive integer): ${limitRaw}`);
    }
    limit = Math.min(parsed, MAX_LIMIT);
  }

  try {
    const events = await store.getTimeline(workspace, {
      from: fromRaw ?? undefined,
      to: toRaw ?? undefined,
      limit,
    });
    return NextResponse.json({ ok: true as const, events });
  } catch (e) {
    return handleStoreError(e);
  }
}
