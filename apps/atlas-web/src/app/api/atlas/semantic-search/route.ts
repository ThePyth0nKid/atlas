/**
 * V2-β Welle 18b/c — POST /api/atlas/semantic-search
 *
 * Run a semantic-search query against the workspace's Layer-3 Mem0g
 * cache. Returns top-k hits, each carrying the Layer-1 `event_uuid`
 * cite-back identifier (Atlas's cite-back trust property — every
 * response MUST carry an event_uuid the caller can independently
 * verify via the offline WASM verifier).
 *
 * **W18c Phase D status (2026-05-15):** the Layer-3 Rust backend
 * (`atlas-mem0g::LanceDbCacheBackend`) is fully OPERATIONAL — the
 * `upsert / search / erase / rebuild` body sites that were
 * previously `Mem0gError::Backend("not yet wired")` placeholders
 * now drive real LanceDB 0.29 ANN search via a dedicated tokio
 * runtime owned by the backend (deadlock-safe per spike §7).
 *
 * The route handler ITSELF still returns 501 because the
 * Rust → TypeScript bridge for atlas-mem0g is V2-γ scope (analog
 * the Layer-2 `/api/atlas/query` route which is also 501-stubbed
 * pending the W17b ArcadeDB driver bridge). The Rust crate is
 * verifiable today via the integration tests in
 * `crates/atlas-mem0g/tests/lancedb_body_e2e.rs` (gated behind
 * `ATLAS_MEM0G_EMBED_SMOKE_ENABLED=1`).
 *
 * Wiring this route handler to the Rust backend requires either:
 *   (a) a NAPI / wasm-bindgen Node addon exposing
 *       `LanceDbCacheBackend::search` to TypeScript, OR
 *   (b) a sidecar process (`bin/atlas-mem0g-search` over Unix
 *       socket / loopback HTTP) that the Next.js route handler
 *       calls.
 * Both are V2-γ scope; the W18c plan-doc explicitly defers them.
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

  // W18c Phase D: the Rust backend (`atlas-mem0g::LanceDbCacheBackend`)
  // is OPERATIONAL — `upsert / search / erase / rebuild` bodies are
  // wired through a backend-owned tokio runtime to LanceDB 0.29
  // (verified by `crates/atlas-mem0g/tests/lancedb_body_e2e.rs`).
  // The remaining gap is the Rust → TypeScript bridge — the Next.js
  // route handler currently has no in-process path to call into the
  // Rust crate. Wiring this is V2-γ scope (analog the parallel
  // `/api/atlas/query` route which is similarly 501-stubbed pending
  // its own bridge to the W17b ArcadeDB driver). See the route's
  // module-level doc-comment for the bridge implementation options
  // (NAPI Node addon vs sidecar process).
  //
  // Until then, this route returns 501 to keep the 400-vs-501 split
  // honest: 400 = malformed; 501 = backend bridge not yet wired (NOT
  // "Layer-3 not yet built" — the Layer-3 cache itself is fully
  // built and tested in-crate).
  await normaliseResponseTime(startMs, minLatencyMs);
  const errorResponse: ErrorResponse = {
    ok: false,
    error:
      "Mem0g Layer-3 Rust backend is OPERATIONAL " +
      "(crates/atlas-mem0g::LanceDbCacheBackend; W18c Phase D shipped 2026-05-15) " +
      "but the Rust → TypeScript bridge ships in V2-γ. Until the bridge lands, " +
      "this route returns 501. See docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md " +
      "and crates/atlas-mem0g/tests/lancedb_body_e2e.rs for the in-crate end-to-end " +
      "verification path.",
  };
  return NextResponse.json(errorResponse, { status: 501 });
}

// Type re-export for downstream test consumers.
export type { SemanticHit, SuccessResponse, ErrorResponse };
