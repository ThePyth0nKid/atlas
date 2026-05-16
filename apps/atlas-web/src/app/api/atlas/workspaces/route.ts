/**
 * W20a — GET /api/atlas/workspaces
 *
 * List user-facing workspaces under the atlas-web data root. The
 * data root is whatever the bridge resolved at bootstrap (see
 * `lib/bootstrap.ts`); for local dev that's `apps/atlas-web/data/`,
 * for production it's whatever `ATLAS_DATA_DIR` points at.
 *
 *   GET /api/atlas/workspaces
 *
 *   200:  { ok: true, workspaces: string[], default: string | null }
 *   500:  { ok: false, error: string }
 *
 * Filtering rules:
 *   1. Only directory entries (not files) — the data root may carry
 *      sibling `.json` artifacts in the future.
 *   2. Only names that pass `isValidWorkspaceId` — defence in depth
 *      against an accidentally-permissive directory listing.
 *   3. CI artifacts from the Playwright fixture
 *      (`pw-w{worker}-{ts}-{rand}`) are filtered out. They accumulate
 *      across CI runs and are not user workspaces; surfacing them in
 *      the selector creates UX noise and operator confusion.
 *
 * `default` is the first non-filtered workspace in the alphabetically-
 * sorted list, or `null` when the data root has no user workspaces.
 * Clients use this to seed the workspace-context provider on first
 * load.
 *
 * Threat model:
 *   * No authentication — same trust posture as every other Read-API
 *     route (auth is the deployer's responsibility at the proxy
 *     layer; see OPERATOR-RUNBOOK §17).
 *   * No path traversal: this route reads `dataDir()` (configured at
 *     bootstrap) and never accepts a caller-supplied path.
 *   * Workspace names are file-system entries; the regex validation
 *     ensures they're alphanumeric + `_-` and length-bounded — no
 *     control chars to leak into HTML/JSON responses.
 *   * Absent data root → 200 with empty array (legal on first run);
 *     other I/O errors → 500 with redacted path.
 */

import "@/lib/bootstrap";

import { promises as fs } from "node:fs";
import { NextResponse } from "next/server";
import { dataDir, isValidWorkspaceId, redactPaths } from "@atlas/bridge";
import { jsonError } from "../_lib/http";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

/**
 * Playwright test-workspace pattern. Matches the fixture in
 * `apps/atlas-web/tests/e2e/fixtures.ts`:
 *   `pw-w{workerIndex}-{ts.toString(36)}-{rand6}`
 * The exact suffix shape is not load-bearing — what matters is
 * the `pw-w<digit>-` prefix, which the fixture is the only known
 * producer of inside the data root.
 */
const CI_ARTIFACT_PATTERN = /^pw-w\d+-/;

export async function GET(): Promise<NextResponse> {
  const root = dataDir();
  let entries: { name: string; isDirectory: () => boolean }[];
  try {
    entries = await fs.readdir(root, { withFileTypes: true });
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") {
      // Fresh install: data root not yet created. Empty list is the
      // correct semantic answer — first write creates the dir.
      return NextResponse.json({
        ok: true as const,
        workspaces: [] as string[],
        default: null,
      });
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `failed to list workspaces: ${redactPaths(msg)}`);
  }

  const workspaces = entries
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .filter((name) => isValidWorkspaceId(name))
    .filter((name) => !CI_ARTIFACT_PATTERN.test(name))
    .sort();

  const defaultWorkspace = workspaces.length > 0 ? workspaces[0] : null;

  return NextResponse.json({
    ok: true as const,
    workspaces,
    default: defaultWorkspace,
  });
}
