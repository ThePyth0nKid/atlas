/**
 * W20c — GET /api/atlas/system/supply-chain-pins
 *
 * Returns the 11 compile-in supply-chain pins from
 * `lib/supply-chain-pins.ts` (mirror of `crates/atlas-mem0g/src/embedder.rs`).
 * Powers the SupplyChainPinsPanel on /settings.
 *
 *   200: { ok: true, hf_revision_sha, onnx_sha256, …, tokenizer_config_json_url }
 *   500: { ok: false, error: string }
 *
 * Threat model:
 *   * Constants are public information (shipped in every release
 *     binary). Exposing them adds no new attack surface.
 *   * `runtime = "nodejs"` + `dynamic = "force-dynamic"` keep the
 *     route out of any Next.js static-rendering cache.
 *   * No request body; no user-controlled state.
 */

import "@/lib/bootstrap";

import { NextResponse } from "next/server";
import { jsonError } from "../../_lib/http";
import { SUPPLY_CHAIN_PINS } from "@/lib/supply-chain-pins";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(): Promise<NextResponse> {
  try {
    return NextResponse.json({
      ok: true as const,
      ...SUPPLY_CHAIN_PINS,
    });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `supply-chain-pins: ${msg}`);
  }
}
