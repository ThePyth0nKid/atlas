/**
 * V2-β Welle 18b — POST /api/atlas/semantic-search
 *
 * Run a semantic-search query against the workspace's Layer-3 Mem0g
 * cache. Returns top-k hits, each carrying the Layer-1 `event_uuid`
 * cite-back identifier (Atlas's cite-back trust property — every
 * response MUST carry an event_uuid the caller can independently
 * verify via the offline WASM verifier).
 *
 * The Layer-3 backend (`atlas-mem0g::LanceDbCacheBackend`) is the
 * V2-β W18b first-shipped impl with placeholder constants. Until
 * Nelson lifts the constants pre-V2-β-1 ship, this route returns 501
 * with a clear pointer to the supply-chain verification gate. The
 * shape contract is testable + reachable today.
 *
 * Wire format:
 *
 *   Request:  application/json
 *     { workspace: string, query: string, k: number }
 *
 *   Response (200):  { ok: true, hits: SemanticHit[] }
 *     SemanticHit = {
 *       event_uuid: string,       // ALWAYS present — cite-back trust
 *       workspace_id: string,
 *       entity_uuid: string | null,
 *       score: number,             // diagnostic only; NOT a trust signal
 *       snippet: string            // GDPR-erasable
 *     }
 *   Response (400):  { ok: false, error: string }
 *   Response (413):  { ok: false, error: string }   (body too large)
 *   Response (501):  { ok: false, error: string }   (backend not yet wired)
 *
 * Timing side-channel mitigation (ADR-Atlas-012 §4 sub-decision #8):
 *   - Response-time normalisation: every response delays to a
 *     configurable per-deployment minimum latency (default 50 ms;
 *     `ATLAS_SEMANTIC_SEARCH_MIN_LATENCY_MS` env var). Cache-hit AND
 *     cache-miss responses both wait until that minimum has elapsed
 *     BEFORE returning. Eliminates the timing distinction at the API
 *     boundary.
 *   - `embedding_hash` cache-key NEVER exposed in response.
 *   - Operator-runbook documents the side-channel (`DECISION-SEC-5`
 *     footnote).
 *
 * Defence layers (mirrors `/api/atlas/query` posture per DECISION-SEC-4):
 *   1. Belt-and-braces request-body byte cap (32 KB — smaller than
 *      query route because semantic-search bodies are intrinsically
 *      small; a 32 KB query is already an abuse signal).
 *   2. Zod-strict input schema (no extra fields).
 *   3. Workspace-id regex (path-traversal structurally impossible).
 *   4. k bounded to [1, 100] (DoS prevention on the LanceDB ANN
 *      retrieval).
 *   5. Query length bounded to [1, 4096] chars.
 */

import "@/lib/bootstrap";

import { NextResponse } from "next/server";
import { z } from "zod";
import { isValidWorkspaceId } from "@atlas/bridge";
import { jsonError } from "../_lib/http";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

/**
 * Hard byte-cap on the raw request body. Semantic-search bodies are
 * intrinsically small (workspace + query + k); 32 KB is generous
 * relative to expected ~1 KB payloads.
 */
const REQUEST_BODY_MAX_BYTES = 32 * 1024;

/**
 * Response-time normalisation default. Configurable per-deployment
 * via `ATLAS_SEMANTIC_SEARCH_MIN_LATENCY_MS` env var. ADR §4
 * sub-decision #8 timing-side-channel mitigation.
 *
 * Operator MAY relax this for trusted internal callers (e.g. Atlas's
 * own MCP tools where the side-channel is moot); MUST keep it for
 * externally-accessible endpoints.
 */
const DEFAULT_MIN_LATENCY_MS = 50;

function resolveMinLatencyMs(): number {
  const env = process.env.ATLAS_SEMANTIC_SEARCH_MIN_LATENCY_MS;
  if (env === undefined || env === "") return DEFAULT_MIN_LATENCY_MS;
  const parsed = Number(env);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return DEFAULT_MIN_LATENCY_MS;
  }
  // Cap at 10 seconds (DoS prevention — an operator misconfiguration
  // setting this to a huge value would stall every request).
  return Math.min(parsed, 10_000);
}

const InputSchema = z
  .object({
    workspace: z
      .string()
      .refine(isValidWorkspaceId, "workspace: only [a-zA-Z0-9_-], 1-128 chars"),
    query: z.string().min(1).max(4096),
    k: z.number().int().min(1).max(100),
  })
  .strict();

interface SemanticHit {
  /** Layer-1 anchor — ALWAYS present per cite-back trust contract. */
  event_uuid: string;
  workspace_id: string;
  entity_uuid: string | null;
  /** Diagnostic only — NOT a trust signal. */
  score: number;
  snippet: string;
}

interface SuccessResponse {
  ok: true;
  hits: SemanticHit[];
}

interface ErrorResponse {
  ok: false;
  error: string;
}

/**
 * Sleep until at least `minMs` has elapsed since `start`. Returns a
 * Promise. Cache-hit AND cache-miss paths await this before
 * returning — eliminating the timing distinction.
 *
 * **NOT fire-and-forget.** The route awaits the sleep before
 * responding so the response actually waits even for fast paths.
 */
async function normaliseResponseTime(start: number, minMs: number): Promise<void> {
  const elapsed = Date.now() - start;
  if (elapsed >= minMs) return;
  const remaining = minMs - elapsed;
  await new Promise<void>((resolve) => setTimeout(resolve, remaining));
}

export async function POST(req: Request): Promise<NextResponse> {
  // Capture start time BEFORE any work for response-time normalisation.
  const startMs = Date.now();
  const minLatencyMs = resolveMinLatencyMs();

  // Belt-and-braces body cap (mirrors /api/atlas/query).
  const contentLength = req.headers.get("content-length");
  if (contentLength !== null) {
    const len = Number(contentLength);
    if (Number.isFinite(len) && len > REQUEST_BODY_MAX_BYTES) {
      await normaliseResponseTime(startMs, minLatencyMs);
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
    await normaliseResponseTime(startMs, minLatencyMs);
    return jsonError(400, `failed to read request body: ${(e as Error).message}`);
  }
  // MEDIUM-4 fix (reviewer-driven): `rawText.length` counts JS
  // UTF-16 code units, NOT UTF-8 bytes. A query in multi-byte UTF-8
  // (Chinese / Arabic / emoji) can exceed 32 KB bytes while reporting
  // <32 KB chars, slipping past the cap. We measure the actual UTF-8
  // byte length the wire saw (Node.js `Buffer.byteLength` is the
  // canonical primitive on the `runtime = "nodejs"` path).
  if (Buffer.byteLength(rawText, "utf8") > REQUEST_BODY_MAX_BYTES) {
    await normaliseResponseTime(startMs, minLatencyMs);
    return jsonError(
      413,
      `request body exceeds ${REQUEST_BODY_MAX_BYTES} bytes`,
    );
  }

  let body: unknown;
  try {
    body = JSON.parse(rawText);
  } catch (e) {
    await normaliseResponseTime(startMs, minLatencyMs);
    return jsonError(400, `request body is not valid JSON: ${(e as Error).message}`);
  }

  const parsed = InputSchema.safeParse(body);
  if (!parsed.success) {
    await normaliseResponseTime(startMs, minLatencyMs);
    return jsonError(400, `invalid input: ${parsed.error.message}`);
  }

  // Layer-3 backend is the V2-β W18b first-shipped impl with
  // placeholder ONNX_SHA256 / HF_REVISION_SHA / MODEL_URL constants.
  // Until Nelson confirms real values pre-V2-β-1 ship, the route
  // returns 501 with a clear pointer to the supply-chain gate. The
  // shape contract above is testable + reachable today; clients can
  // rely on the 400-vs-501 split (400 = malformed; 501 = waiting on
  // pre-merge constant lift).
  //
  // When the constants land, this branch is replaced with a real
  // `atlas-mem0g::SemanticCacheBackend::search` call via the Rust →
  // TypeScript bridge (analog the Layer-2 W17b ArcadeDB integration).
  await normaliseResponseTime(startMs, minLatencyMs);
  const errorResponse: ErrorResponse = {
    ok: false,
    error:
      "Mem0g Layer-3 backend ships in V2-β-1 after pre-merge supply-chain " +
      "constants (ONNX_SHA256, HF_REVISION_SHA, MODEL_URL) are confirmed. " +
      "See docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md §4 sub-decision #2 " +
      "and crates/atlas-mem0g/src/embedder.rs.",
  };
  return NextResponse.json(errorResponse, { status: 501 });
}

// Type re-export for downstream test consumers.
export type { SemanticHit, SuccessResponse, ErrorResponse };
