/**
 * V2-β Welle 12 — GET /api/atlas/passport/[agent_did]
 *
 * STUB for V2-γ Agent Passports. Full Agent Passport implementation
 * is V2-γ scope per `docs/V2-MASTER-PLAN.md` §6 V2-γ Identity +
 * Federation and `docs/V2-BETA-ORCHESTRATION-PLAN.md` §7 "Deferred
 * to later iteration phases".
 *
 * This route exists in V2-β so:
 *   1. The full Read-API surface is mounted (clients can probe
 *      `/api/atlas/passport/...` and receive a structured 501
 *      instead of a Next.js 404).
 *   2. Discovery: a client `GET`-ing this endpoint learns where
 *      to find the full implementation roadmap.
 *
 * Always returns HTTP 501 Not Implemented.
 *
 *   GET /api/atlas/passport/{agent_did}
 *
 *   501:  { ok: false, agent_did, status: "stub", message: string }
 */

import { NextResponse } from "next/server";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(
  _req: Request,
  ctx: { params: Promise<{ agent_did: string }> },
): Promise<NextResponse> {
  const { agent_did } = await ctx.params;
  return NextResponse.json(
    {
      ok: false as const,
      agent_did,
      status: "stub" as const,
      message:
        "Agent Passport endpoint is V2-γ scope; see docs/V2-MASTER-PLAN.md §6 V2-γ Identity + Federation",
    },
    { status: 501 },
  );
}
