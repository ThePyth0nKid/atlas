/**
 * V2-β Welle 12 — POST /api/atlas/query
 *
 * Run a read-only Cypher query against the workspace's projected
 * graph. The Cypher AST validator is welle-local
 * (`../_lib/cypher-validator.ts`); W15 consolidates the shared
 * module after rule-of-three.
 *
 * The actual Cypher *execution* backend is NOT in V2-β Phase 4 —
 * it lands in Phase 7 (W17 ArcadeDB driver). Until then, the route
 * runs full input validation, runs the Cypher AST validator,
 * returns 400 on rejection, and returns HTTP 501 ("not implemented
 * yet") for accepted queries with a clear pointer to V2-β Phase 7.
 * This means the validator is testable + reachable today even
 * though execution is stubbed — exactly the API contract surface
 * Phase 4 is meant to ship.
 *
 * Wire format:
 *
 *   Request:  application/json
 *     { workspace: string, cypher: string, params?: object }
 *
 *   Response (501):  { ok: false, error: string, validator: "passed" }
 *   Response (400):  { ok: false, error: string }
 *   Response (413):  { ok: false, error: string }   (body too large)
 *
 * Defence layers (per `DECISION-SEC-4`):
 *   1. Belt-and-braces request-body byte cap (256 KB)
 *   2. Zod-strict input schema (no extra fields)
 *   3. Workspace-id regex (path-traversal structurally impossible)
 *   4. Cypher AST validator (write-keyword + apoc/db.* + concat reject)
 *   5. params-only binding (parameters never reach validator string)
 */

import "@/lib/bootstrap";

import { NextResponse } from "next/server";
import { z } from "zod";
import { isValidWorkspaceId } from "@atlas/bridge";
import { validateReadOnlyCypher } from "../_lib/cypher-validator";
import { jsonError } from "../_lib/http";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

/**
 * Hard byte-cap on the raw request body. Mirrors `write-node`'s
 * defence: rejects oversized bodies BEFORE `req.json()` reads them.
 */
const REQUEST_BODY_MAX_BYTES = 256 * 1024;

const InputSchema = z
  .object({
    workspace: z
      .string()
      .refine(isValidWorkspaceId, "workspace: only [a-zA-Z0-9_-], 1-128 chars"),
    cypher: z.string().min(1).max(16 * 1024),
    params: z.record(z.string(), z.unknown()).default({}),
  })
  .strict();

export async function POST(req: Request): Promise<NextResponse> {
  // Belt-and-braces body cap. We check Content-Length first as a cheap
  // pre-read gate, then *also* enforce the cap on the raw body length
  // after reading. Some HTTP clients (and chunked / streaming clients)
  // omit Content-Length entirely; without the post-read check those
  // requests would bypass the cap and reach `JSON.parse` unbounded.
  const contentLength = req.headers.get("content-length");
  if (contentLength !== null) {
    const len = Number(contentLength);
    if (Number.isFinite(len) && len > REQUEST_BODY_MAX_BYTES) {
      return jsonError(
        413,
        `request body exceeds ${REQUEST_BODY_MAX_BYTES} bytes`,
      );
    }
  }

  let rawText: string;
  try {
    rawText = await req.text();
  } catch (e) {
    return jsonError(400, `failed to read request body: ${(e as Error).message}`);
  }
  if (rawText.length > REQUEST_BODY_MAX_BYTES) {
    return jsonError(
      413,
      `request body exceeds ${REQUEST_BODY_MAX_BYTES} bytes`,
    );
  }

  let body: unknown;
  try {
    body = JSON.parse(rawText);
  } catch (e) {
    return jsonError(400, `request body is not valid JSON: ${(e as Error).message}`);
  }

  const parsed = InputSchema.safeParse(body);
  if (!parsed.success) {
    return jsonError(400, `invalid input: ${parsed.error.message}`);
  }
  const args = parsed.data;

  const validation = validateReadOnlyCypher(args.cypher);
  if (!validation.ok) {
    return jsonError(400, validation.reason ?? "cypher: rejected by validator");
  }

  // Cypher execution backend is V2-β Phase 7 / W17. Until then we
  // return 501 with a clear pointer. The validator has already
  // accepted the query — clients can rely on the 400-vs-501 split.
  return NextResponse.json(
    {
      ok: false as const,
      error:
        "Cypher read backend lands in V2-β Phase 7 / Welle 17 (ArcadeDB driver). " +
        "See docs/V2-BETA-ORCHESTRATION-PLAN.md §2 and ADR-Atlas-010.",
      validator: "passed" as const,
    },
    { status: 501 },
  );
}
