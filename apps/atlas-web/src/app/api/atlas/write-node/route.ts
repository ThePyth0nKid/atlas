/**
 * V1.19 Welle 1 — POST /api/atlas/write-node
 *
 * Server-side route handler that turns a structured node-creation
 * form submission into a signed AtlasEvent on the workspace's
 * append-only DAG. The implementation mirrors `atlas_write_node` in
 * `apps/atlas-mcp-server/src/tools/write-node.ts` but is reachable
 * from a browser instead of an MCP host.
 *
 * Threat model & deliberate non-goals:
 *
 *   * NO authentication. Atlas's trust model is key-based (every
 *     event records `signature.kid`), not user-based. The deployer
 *     gates exposure at the network/proxy layer — see
 *     OPERATOR-RUNBOOK §17. Surfacing this route on a public address
 *     without an upstream auth gate IS the operator's choice and
 *     responsibility.
 *
 *   * Per-tenant kid auto-derived from `workspace_id`. The route
 *     never accepts a caller-supplied `kid`. This eliminates the
 *     "sign as somebody else" affordance the MCP tool intentionally
 *     reserves for trusted MCP hosts (Cursor, Claude Desktop), and
 *     keeps the web write surface narrow.
 *
 *   * Single-process locking only. The per-workspace mutex in
 *     `@atlas/bridge` serialises writes within ONE Node process.
 *     Running atlas-web AND atlas-mcp-server against the same
 *     workspace concurrently can fork the DAG. The verifier accepts
 *     forks (it's a DAG) but the user's mental model breaks; deploy
 *     one writer per workspace until V2 ships an external lock.
 *
 *   * `ATLAS_DEV_MASTER_SEED=1` (or the wave-3 HSM opt-in) must be
 *     set in the Next.js process environment, otherwise per-tenant
 *     subcommands of `atlas-signer` refuse to start. Surfaced as a
 *     500 with a clear message when missing.
 *
 * Wire format:
 *
 *   Request:  application/json
 *     { workspace_id: string,
 *       kind: "dataset"|"model"|"inference"|"document"|"other",
 *       id: string,
 *       attributes: Record<string, unknown> }
 *
 *   Response (200):  application/json
 *     { ok: true,
 *       workspace_id, event_id, event_hash, parents: string[], kid }
 *
 *   Response (4xx/5xx):  application/json
 *     { ok: false, error: string }
 */

// V1.19 Welle 2: register the web app's data-dir default with the
// bridge BEFORE any other bridge call. ATLAS_DATA_DIR still wins as
// the operator override; this only sets the dev/local fallback.
import "@/lib/bootstrap";

import { NextResponse } from "next/server";
import { z } from "zod";
import {
  writeSignedEvent,
  perTenantKidFor,
  SignerError,
  redactPaths,
  StorageError,
  WorkspacePathError,
} from "@atlas/bridge";

export const runtime = "nodejs";
// This route spawns a child process and writes to the local
// filesystem; it cannot be statically rendered or cached.
export const dynamic = "force-dynamic";

/**
 * V1.19 Welle 1 review-fix: belt-and-braces request-body cap. The
 * Zod `attributes` refine already caps the JSON-serialised attribute
 * payload at 64 KB, but this hard byte cap on the raw request body
 * runs BEFORE `req.json()` even reads the stream — so a malicious
 * client cannot fan-out memory by streaming a 10 MB JSON tree that
 * Zod would have rejected at the structural layer. 256 KB leaves
 * generous headroom over the 64 KB attributes cap (header overhead,
 * top-level keys, base64 expansion in `id`) without enabling any
 * abuse vector. The check exits before the JSON parser runs.
 */
const REQUEST_BODY_MAX_BYTES = 256 * 1024;

const WORKSPACE_ID_PATTERN = /^[a-zA-Z0-9_-]{1,128}$/;
const NODE_KIND = z.enum(["dataset", "model", "inference", "document", "other"]);

/**
 * V1.19 Welle 1 review fix: bound the JSON-serialised size of
 * `attributes`. The route passes `payload` (which embeds attributes)
 * to `atlas-signer` as a single `--payload <json>` argv string. On
 * Linux, exceeding ARG_MAX (~2 MB on most kernels) makes `spawn`
 * throw E2BIG, surfacing as a generic 500. A 64 KB cap is well
 * within ARG_MAX, well above any realistic compliance-event payload,
 * and gives a clean Zod error at the boundary instead of a child-
 * process spawn failure deeper in the call chain.
 *
 * Defence in depth against unbounded request bodies too: even though
 * Next.js's App Router enforces an upper bound at the HTTP layer,
 * Zod runs before any payload bytes reach the signer, so an attacker
 * cannot fan-out large signer subprocesses by sending one large JSON
 * blob through this route.
 */
const ATTRIBUTES_MAX_BYTES = 64 * 1024;

const InputSchema = z
  .object({
    workspace_id: z
      .string()
      .regex(WORKSPACE_ID_PATTERN, "workspace_id: only [a-zA-Z0-9_-], 1–128 chars"),
    kind: NODE_KIND,
    id: z.string().min(1, "id is required").max(256, "id is too long"),
    attributes: z
      .record(z.string(), z.unknown())
      .default({})
      .refine(
        (v) => JSON.stringify(v).length <= ATTRIBUTES_MAX_BYTES,
        `attributes exceeds ${ATTRIBUTES_MAX_BYTES} bytes when JSON-serialised`,
      ),
  })
  .strict();

export async function POST(req: Request): Promise<NextResponse> {
  // Belt-and-braces request-body cap (see REQUEST_BODY_MAX_BYTES).
  // `Content-Length` is advisory — a streaming client can omit or
  // misrepresent it — but checking it cheaply rejects the obvious
  // cases before reading the body. The Zod refine on attributes is
  // the structural cap; this is the byte-count cap.
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

  let body: unknown;
  try {
    body = await req.json();
  } catch (e) {
    return jsonError(400, `request body is not valid JSON: ${(e as Error).message}`);
  }

  const parsed = InputSchema.safeParse(body);
  if (!parsed.success) {
    return jsonError(400, `invalid input: ${parsed.error.message}`);
  }
  const args = parsed.data;

  // Spread `attributes` FIRST so the validated `kind` and `id` win on
  // key collision — never let a caller-supplied attribute object
  // overwrite the schema-checked node identity. Silent override of
  // these fields would corrupt the signed payload.
  const payload: Record<string, unknown> = {
    type: "node.create",
    node: {
      ...args.attributes,
      kind: args.kind,
      id: args.id,
    },
  };

  try {
    const result = await writeSignedEvent({
      workspaceId: args.workspace_id,
      payload,
    });
    return NextResponse.json({
      ok: true as const,
      workspace_id: args.workspace_id,
      event_id: result.event.event_id,
      event_hash: result.event.event_hash,
      parents: result.parentsUsed,
      kid: result.kid,
    });
  } catch (e) {
    if (e instanceof WorkspacePathError) {
      // Defence in depth — already caught by the regex, but if a
      // future env-var change loosens the data root, this still
      // refuses to escape.
      return jsonError(400, e.message);
    }
    if (e instanceof SignerError) {
      // Most signer failures are operator config issues
      // (binary missing, ATLAS_DEV_MASTER_SEED unset, HSM unreachable).
      // Surface them as 500 with the message — they're not user input
      // problems. `redactPaths` strips absolute filesystem paths from
      // the message so a 500 response can't be used to map server
      // layout (defence in depth: the signer should not write paths
      // to stderr in the first place, but redacting here means we
      // don't depend on that discipline).
      return jsonError(500, `signer: ${redactPaths(e.message)}`);
    }
    if (e instanceof StorageError) {
      // StorageError messages are already path-sanitised at source
      // (storage.ts:sanitiseFsError); pass through unchanged.
      return jsonError(500, `storage: ${e.message}`);
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `unexpected: ${redactPaths(msg)}`);
  }
}

// GET handler for client-side health checks: returns the per-tenant
// kid that WOULD be used for `workspace_id` (sanity check before
// posting a write).
export async function GET(req: Request): Promise<NextResponse> {
  const url = new URL(req.url);
  const workspaceId = url.searchParams.get("workspace_id");
  if (workspaceId === null) {
    return jsonError(400, "missing workspace_id query parameter");
  }
  if (!WORKSPACE_ID_PATTERN.test(workspaceId)) {
    return jsonError(400, "workspace_id: only [a-zA-Z0-9_-], 1–128 chars");
  }
  return NextResponse.json({
    ok: true as const,
    workspace_id: workspaceId,
    kid: perTenantKidFor(workspaceId),
  });
}

function jsonError(status: number, message: string): NextResponse {
  return NextResponse.json({ ok: false as const, error: message }, { status });
}
