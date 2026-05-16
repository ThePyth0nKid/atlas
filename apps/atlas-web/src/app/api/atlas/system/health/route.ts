/**
 * W20c — GET /api/atlas/system/health
 *
 * Returns the layer-3 honest status block consumed by
 * `<LayerStatusPanel>` (dashboard) and `<SignerStatusPanel>`
 * (/settings):
 *
 *   200: { ok: true, embedder, backend, signer }
 *   500: { ok: false, error: string }
 *
 * Status values (string-literal unions; see `lib/system-health.ts`):
 *   embedder: 'operational' | 'model_missing' | 'unsupported'
 *   backend:  'operational' | 'stub_501' | 'fault'
 *   signer:   'operational' | 'unconfigured'
 *
 * Probes are env-only — no subprocess spawn. The route layer caches
 * results with a 60-second TTL via `getCachedLayerStatus`. Test-hook
 * overrides honored only when `ATLAS_E2E_TEST_HOOKS=1` (set in
 * `playwright.config.ts`).
 *
 * Threat model:
 *   * No auth (same posture as every other /api/atlas/* route)
 *   * No client-supplied data leaks into the probe — overrides come
 *     from a custom header recognised only when test-hooks env-var
 *     is set in the SERVER process, not by attacker request
 *   * `runtime = "nodejs"` + `dynamic = "force-dynamic"` keep this
 *     route out of any Next.js static-rendering cache
 */

import "@/lib/bootstrap";

import { NextResponse } from "next/server";
import { jsonError } from "../../_lib/http";
import {
  getCachedLayerStatus,
  testHooksEnabled,
  type EmbedderStatus,
  type BackendStatus,
  type SignerStatus,
} from "@/lib/system-health";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const FORCE_SIGNER_HEADER = "x-atlas-test-force-signer";
const FORCE_EMBEDDER_HEADER = "x-atlas-test-force-embedder";
const FORCE_BACKEND_HEADER = "x-atlas-test-force-backend";

const SIGNER_VALUES: ReadonlySet<SignerStatus> = new Set([
  "operational",
  "unconfigured",
]);
const EMBEDDER_VALUES: ReadonlySet<EmbedderStatus> = new Set([
  "operational",
  "model_missing",
  "unsupported",
]);
const BACKEND_VALUES: ReadonlySet<BackendStatus> = new Set([
  "operational",
  "stub_501",
  "fault",
]);

function readSignerOverride(req: Request): SignerStatus | undefined {
  const v = req.headers.get(FORCE_SIGNER_HEADER);
  if (v === null) return undefined;
  return SIGNER_VALUES.has(v as SignerStatus) ? (v as SignerStatus) : undefined;
}

function readEmbedderOverride(req: Request): EmbedderStatus | undefined {
  const v = req.headers.get(FORCE_EMBEDDER_HEADER);
  if (v === null) return undefined;
  return EMBEDDER_VALUES.has(v as EmbedderStatus)
    ? (v as EmbedderStatus)
    : undefined;
}

function readBackendOverride(req: Request): BackendStatus | undefined {
  const v = req.headers.get(FORCE_BACKEND_HEADER);
  if (v === null) return undefined;
  return BACKEND_VALUES.has(v as BackendStatus)
    ? (v as BackendStatus)
    : undefined;
}

export async function GET(req: Request): Promise<NextResponse> {
  try {
    // Only consult headers if test-hooks are enabled. Production deploys
    // do not set ATLAS_E2E_TEST_HOOKS, so attacker-controlled headers
    // are ignored by construction.
    const override = testHooksEnabled()
      ? {
          signer: readSignerOverride(req),
          embedder: readEmbedderOverride(req),
          backend: readBackendOverride(req),
        }
      : {};
    const status = getCachedLayerStatus(override);
    return NextResponse.json({
      ok: true as const,
      embedder: status.embedder,
      backend: status.backend,
      signer: status.signer,
    });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `health probe failed: ${msg}`);
  }
}
