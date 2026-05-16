/**
 * W20a — GET /api/atlas/trace?workspace={id}
 *
 * Export a workspace's events.jsonl as a canonical `AtlasTrace` JSON
 * document, ready for the WASM verifier to consume in the browser.
 *
 *   GET /api/atlas/trace?workspace={id}
 *
 *   200:  AtlasTrace (raw JSON, NOT wrapped in {ok:true,...})
 *   400:  { ok: false, error: string }
 *   500:  { ok: false, error: string }
 *
 * Trace shape (matches `crates/atlas-trust-core/src/trace_format.rs`):
 *
 *   {
 *     schema_version: "atlas-trace-v1",
 *     generated_at: ISO-8601 string,
 *     workspace_id: string,
 *     pubkey_bundle_hash: 64-char hex of canonical bundle hash,
 *     events: AtlasEvent[],
 *     dag_tips: string[],   // empty when no events
 *     anchors: [],          // empty — atlas-web doesn't anchor yet
 *     policies: [],         // empty — policy layer is post-W20
 *     filters: null
 *   }
 *
 * Empty-workspace handling: when the workspace has no `events.jsonl`
 * (genesis) OR an existing file with zero events, the route returns a
 * valid trace with `events: []` and a bundle hash for an empty
 * derivation. The verifier handles 0-event traces correctly (lenient
 * mode passes; strict mode rejects). The empty-trace path is
 * exercised by the W20a Playwright spec.
 *
 * Threat model:
 *   * `workspace` validated via `isValidWorkspaceId` at the entry
 *     point — path-traversal structurally impossible.
 *   * `pubkey_bundle_hash` is computed via `bundleHashViaSigner`,
 *     which routes through the Rust signer for canonicalisation. The
 *     TS side never canonicalises bundle JSON itself — drift between
 *     this hash and the verifier's recomputed hash would invalidate
 *     every trace, so we single-source via the signer.
 *   * `buildBundleForWorkspace` uses the public-only derive path
 *     (`derivePubkeyViaSigner`) — the per-tenant secret never
 *     crosses the subprocess boundary on this route.
 *   * Workspace folder absent (ENOENT) → 200 + empty trace. Other
 *     I/O errors → 500 with redacted message.
 */

import "@/lib/bootstrap";

import { NextResponse } from "next/server";
import {
  bundleHashViaSigner,
  buildBundleForWorkspace,
  computeTips,
  readAllEvents,
  redactPaths,
  SCHEMA_VERSION,
  SignerError,
  StorageError,
  WorkspacePathError,
} from "@atlas/bridge";
import type { AtlasEvent, AtlasTrace } from "@atlas/bridge";
import { isResponse, jsonError, requireWorkspace } from "../_lib/http";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(req: Request): Promise<NextResponse> {
  const workspace = requireWorkspace(req);
  if (isResponse(workspace)) return workspace;

  let events: AtlasEvent[];
  try {
    // `readAllEvents` returns [] when the events.jsonl is absent —
    // the genesis case is benign, not an error.
    events = await readAllEvents(workspace);
  } catch (e) {
    if (e instanceof WorkspacePathError) {
      return jsonError(400, redactPaths(e.message));
    }
    if (e instanceof StorageError) {
      return jsonError(500, `storage: ${redactPaths(e.message)}`);
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `unexpected: ${redactPaths(msg)}`);
  }

  let bundleHash: string;
  try {
    const bundle = await buildBundleForWorkspace(workspace);
    bundleHash = await bundleHashViaSigner(JSON.stringify(bundle));
  } catch (e) {
    if (e instanceof SignerError) {
      return jsonError(500, `signer: ${redactPaths(e.message)}`);
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `unexpected: ${redactPaths(msg)}`);
  }

  const dagTips = computeTips(events);

  const trace: AtlasTrace = {
    schema_version: SCHEMA_VERSION,
    generated_at: new Date().toISOString(),
    workspace_id: workspace,
    pubkey_bundle_hash: bundleHash,
    events,
    dag_tips: dagTips,
    anchors: [],
    policies: [],
    filters: null,
  };

  // Hand-build the response body so we own the exact byte stream
  // returned to the verifier. See the analogous comment in
  // `pubkey-bundle/route.ts` for the rationale.
  const body = JSON.stringify(trace);
  return new NextResponse(body, {
    status: 200,
    headers: {
      "content-type": "application/json",
      "cache-control": "no-store",
    },
  });
}
