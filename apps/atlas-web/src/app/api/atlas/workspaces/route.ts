/**
 * W20c CHANGES (this commit):
 *   * POST: signer-failure path now atomically rolls back the
 *     freshly-mkdir'd workspace directory before returning 500.
 *     Eliminates the orphan "unconfigured" workspace UX that landed
 *     in ba4e27f (W20b-2) and was flagged by security-reviewer in
 *     PR #113. If the rollback itself fails (e.g. concurrent process
 *     held a handle), the response carries a `partial_rollback:`
 *     prefix so operators can clean up by hand.
 *   * PATCH: rename a workspace via `fs.rename` (atomic on same
 *     volume). Validates both old and new ids against
 *     `WORKSPACE_ID_RE`. EXDEV (cross-mount) surfaces as 500.
 *   * DELETE: tear down a workspace via `fs.rm`. Refuses to delete
 *     the last user-facing workspace (returns 409) so the UI cannot
 *     flip back to FirstRunWizard mid-session.
 *
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
 *
 * W20b-2 — POST /api/atlas/workspaces
 *
 * Create a new workspace directory and derive its per-tenant kid /
 * pubkey via the Rust signer. Powers the first-run wizard and the
 * "+ New" affordance in the workspace selector — both are browser
 * paths; the MCP server bootstraps workspaces on first write.
 *
 *   POST /api/atlas/workspaces
 *   Content-Type: application/json
 *   Body: { workspace_id: string }   // matches WORKSPACE_ID_PATTERN
 *
 *   200:  { ok: true, workspace_id, kid, pubkey_b64url }
 *   400:  invalid body / id / shape
 *   409:  workspace dir already exists
 *   413:  request body exceeds REQUEST_BODY_MAX_BYTES
 *   500:  signer / storage / unexpected error (path-redacted)
 *
 * Threat model:
 *   * No new auth surface — same trust posture as GET (operator gates
 *     at the proxy layer; see OPERATOR-RUNBOOK §17).
 *   * Workspace id is validated by Zod via `isValidWorkspaceId` regex
 *     (`^[a-zA-Z0-9_-]{1,128}$`) BEFORE any filesystem call. This is
 *     the same regex the bridge uses to refuse path traversal — see
 *     `packages/atlas-bridge/src/paths.ts:WORKSPACE_ID_RE`.
 *   * Hard 4 KB body cap (`REQUEST_BODY_MAX_BYTES`) keeps a malicious
 *     client from streaming megabytes the Zod parser would have
 *     rejected anyway. Checked from `Content-Length` BEFORE reading
 *     the request stream. Mirrors the pattern in write-node/route.ts.
 *   * 409 on existing directory: `fs.stat` + ENOENT check; NOT
 *     race-free between stat and mkdir. The race is benign — if two
 *     concurrent requests both pass the stat, `ensureWorkspaceDir`'s
 *     `mkdir({ recursive: true })` succeeds on both, and both will
 *     return 200 with the same derived kid/pubkey. The 409 is the UX
 *     guard against duplicate-name confusion in the browser, not a
 *     strong uniqueness invariant.
 *   * `derivePubkeyViaSigner` fails fast when `ATLAS_DEV_MASTER_SEED`
 *     is unset, surfaced as 500 with `signer:` prefix and paths
 *     redacted. No new ambient state is created — the directory was
 *     already mkdir'd above, but it's empty and harmless; the user
 *     simply needs to configure the signer and retry.
 */

import "@/lib/bootstrap";

import { promises as fs } from "node:fs";
import { NextResponse } from "next/server";
import { z } from "zod";
import {
  dataDir,
  derivePubkeyViaSigner,
  ensureWorkspaceDir,
  isValidWorkspaceId,
  perTenantKidFor,
  redactPaths,
  SignerError,
  StorageError,
  workspaceDir,
  WorkspacePathError,
  WORKSPACE_ID_RE,
} from "@atlas/bridge";
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

/**
 * W20b-2 — belt-and-braces request-body cap. The Zod schema accepts
 * a tiny object (`{ workspace_id }`); 4 KB leaves generous headroom
 * for header overhead while rejecting malicious streams at the byte
 * layer BEFORE `req.json()` reads the body. Smaller than write-node's
 * 256 KB because this route has no `attributes` payload to carry.
 */
const REQUEST_BODY_MAX_BYTES = 4 * 1024;

/** Frozen test-only export of the body cap — mirrors write-node's pattern. */
export const __REQUEST_BODY_MAX_BYTES_FOR_TEST = REQUEST_BODY_MAX_BYTES;

const CreateWorkspaceSchema = z
  .object({
    workspace_id: z
      .string()
      .regex(
        WORKSPACE_ID_RE,
        "workspace_id: only [a-zA-Z0-9_-], 1–128 chars",
      ),
  })
  .strict();

/**
 * W20c — schemas for PATCH (rename) and DELETE.
 * Both `.strict()` for the same reason as the create schema: log-
 * pipeline safety on the unrecognized-keys path.
 */
const RenameWorkspaceSchema = z
  .object({
    workspace_id: z
      .string()
      .regex(
        WORKSPACE_ID_RE,
        "workspace_id: only [a-zA-Z0-9_-], 1–128 chars",
      ),
    new_workspace_id: z
      .string()
      .regex(
        WORKSPACE_ID_RE,
        "new_workspace_id: only [a-zA-Z0-9_-], 1–128 chars",
      ),
  })
  .strict();

const DeleteWorkspaceSchema = z
  .object({
    workspace_id: z
      .string()
      .regex(
        WORKSPACE_ID_RE,
        "workspace_id: only [a-zA-Z0-9_-], 1–128 chars",
      ),
  })
  .strict();

/**
 * W20c — `.strict()` Zod failures may embed attacker-controlled
 * unrecognized key names verbatim. Collapse those into a static
 * message before they reach the JSON envelope. Mirrors the existing
 * POST-path branch (W20b-2 security-reviewer finding #9). Other Zod
 * failures carry the original error message because it includes the
 * helpful regex description that the client can surface.
 */
function safeZodErrorMessage(err: z.ZodError): string {
  const hasUnrecognizedKeys = err.issues.some(
    (issue) => issue.code === "unrecognized_keys",
  );
  if (hasUnrecognizedKeys) {
    return "invalid input: body contains unexpected keys";
  }
  return `invalid input: ${err.message}`;
}

/**
 * W20c — count user-facing workspaces on disk. Mirrors the GET-time
 * filter but is a lighter helper (we only need the count, not the
 * list). Used by DELETE to refuse to remove the last workspace
 * (DA-6).
 *
 * `CI_ARTIFACT_PATTERN` is intentionally excluded from the count so
 * a Playwright cleanup run that's deleting the last pw-* workspace
 * does not flip the UI into wizard mode. The count we care about
 * for the 409 gate is "user-visible workspaces after the delete".
 */
async function countUserFacingWorkspaces(): Promise<number> {
  const root = dataDir();
  let entries: { name: string; isDirectory: () => boolean }[];
  try {
    entries = await fs.readdir(root, { withFileTypes: true });
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return 0;
    throw e;
  }
  return entries.filter(
    (e) =>
      e.isDirectory() &&
      isValidWorkspaceId(e.name) &&
      !CI_ARTIFACT_PATTERN.test(e.name),
  ).length;
}

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
    // toSorted (ES2023 / Node 20+) returns a new array — respects the
    // repo's immutability standing rule. The preceding .filter/.map
    // chain already produces a fresh array, so the practical effect is
    // identical; this keeps the pattern consistent with the rest of
    // the codebase.
    .toSorted();

  const defaultWorkspace = workspaces.length > 0 ? workspaces[0] : null;

  return NextResponse.json({
    ok: true as const,
    workspaces,
    default: defaultWorkspace,
  });
}

export async function POST(req: Request): Promise<NextResponse> {
  // Byte-layer cap BEFORE reading the body — see REQUEST_BODY_MAX_BYTES.
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
    return jsonError(
      400,
      `request body is not valid JSON: ${(e as Error).message}`,
    );
  }

  const parsed = CreateWorkspaceSchema.safeParse(body);
  if (!parsed.success) {
    // W20c — delegated to `safeZodErrorMessage` so PATCH + DELETE share
    // the same unrecognized-keys discipline as POST (W20b-2 finding #9).
    return jsonError(400, safeZodErrorMessage(parsed.error));
  }
  const workspaceId = parsed.data.workspace_id;

  // 409 collision check — see threat-model comment at top of file for
  // why the stat→mkdir race is intentionally benign.
  try {
    const stat = await fs.stat(workspaceDir(workspaceId));
    if (stat.isDirectory()) {
      return jsonError(409, "workspace already exists");
    }
    // A non-directory entry at this path is a misconfiguration; surface
    // it as 409 rather than mkdir'ing on top of it.
    return jsonError(409, "workspace already exists");
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code !== "ENOENT") {
      // Surface ANY unexpected stat error rather than swallowing it.
      // W20b-2 fix-commit (security-reviewer MEDIUM, defence-in-depth):
      // wrap with `redactPaths` even though `WorkspacePathError.message`
      // does not contain absolute paths today — matches the discipline
      // in `_lib/http.ts:handleStoreError`. Closes drift before it ships.
      if (e instanceof WorkspacePathError) {
        return jsonError(400, redactPaths(e.message));
      }
      const msg = e instanceof Error ? e.message : String(e);
      return jsonError(500, `stat: ${redactPaths(msg)}`);
    }
    // ENOENT is the happy path — workspace does not yet exist.
  }

  // W20c (DA-1): track whether the workspace directory was created in
  // THIS request. If signer derivation fails AFTER mkdir, we must
  // atomically remove the empty dir so the user does not end up with
  // an orphan "unconfigured" workspace under `dataDir()`. The mkdir
  // step itself comes before signer derivation; ensureWorkspaceDir
  // failures (StorageError, WorkspacePathError) leave nothing behind
  // and skip the rollback.
  let dirCreated = false;
  try {
    await ensureWorkspaceDir(workspaceId);
    dirCreated = true;
    const derived = await derivePubkeyViaSigner(workspaceId);
    return NextResponse.json({
      ok: true as const,
      workspace_id: workspaceId,
      kid: perTenantKidFor(workspaceId),
      pubkey_b64url: derived.pubkey_b64url,
    });
  } catch (e) {
    // W20c (DA-1): SignerError after mkdir is the orphan-workspace
    // path. Roll back the directory before surfacing the 500. The
    // rollback uses `force: false` so a vanished dir (concurrent
    // delete) does not mask the original signer error; if rm itself
    // fails, surface the partial-rollback so operators can clean up.
    if (e instanceof SignerError && dirCreated) {
      try {
        await fs.rm(workspaceDir(workspaceId), { recursive: true, force: false });
        return jsonError(500, `signer: ${redactPaths(e.message)}`);
      } catch (rmErr) {
        const rmMsg =
          rmErr instanceof Error ? rmErr.message : String(rmErr);
        return jsonError(
          500,
          `partial_rollback: signer failed and workspace directory could not be removed: ${redactPaths(rmMsg)}`,
        );
      }
    }
    // W20b-2 fix-commit (security-reviewer MEDIUM, defence-in-depth):
    // route every error message through `redactPaths`, matching
    // `_lib/http.ts:handleStoreError`. None of these messages contain
    // absolute paths today — but the bridge's error surface evolves
    // independently of this route, and a one-line wrap closes that
    // drift permanently.
    if (e instanceof WorkspacePathError) {
      return jsonError(400, redactPaths(e.message));
    }
    if (e instanceof SignerError) {
      // Signer failure before mkdir (e.g. binary missing) — no
      // rollback needed.
      return jsonError(500, `signer: ${redactPaths(e.message)}`);
    }
    if (e instanceof StorageError) {
      return jsonError(500, `storage: ${redactPaths(e.message)}`);
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `unexpected: ${redactPaths(msg)}`);
  }
}

// ──────────────── PATCH /api/atlas/workspaces (rename, W20c) ────────────────

export async function PATCH(req: Request): Promise<NextResponse> {
  // Byte-layer cap BEFORE reading the body — mirrors POST.
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
    return jsonError(
      400,
      `request body is not valid JSON: ${(e as Error).message}`,
    );
  }

  const parsed = RenameWorkspaceSchema.safeParse(body);
  if (!parsed.success) {
    return jsonError(400, safeZodErrorMessage(parsed.error));
  }
  const { workspace_id: oldId, new_workspace_id: newId } = parsed.data;

  if (oldId === newId) {
    return jsonError(400, "invalid input: new_workspace_id must differ from workspace_id");
  }

  // Validate source exists.
  let oldDirPath: string;
  let newDirPath: string;
  try {
    oldDirPath = workspaceDir(oldId);
    newDirPath = workspaceDir(newId);
  } catch (e) {
    if (e instanceof WorkspacePathError) {
      return jsonError(400, redactPaths(e.message));
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `unexpected: ${redactPaths(msg)}`);
  }

  try {
    const stat = await fs.stat(oldDirPath);
    if (!stat.isDirectory()) {
      return jsonError(404, "workspace not found");
    }
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "ENOENT") {
      return jsonError(404, "workspace not found");
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `stat: ${redactPaths(msg)}`);
  }

  // Refuse if the target dir already exists — never silently overwrite.
  try {
    await fs.stat(newDirPath);
    return jsonError(409, "workspace already exists");
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code !== "ENOENT") {
      const msg = e instanceof Error ? e.message : String(e);
      return jsonError(500, `stat: ${redactPaths(msg)}`);
    }
    // ENOENT is the happy path — target does not exist.
  }

  try {
    await fs.rename(oldDirPath, newDirPath);
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "EXDEV") {
      return jsonError(
        500,
        "cross_mount_rename_unsupported: source and target on different volumes",
      );
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `rename: ${redactPaths(msg)}`);
  }

  // Re-derive pubkey under the new id. Signer failure here is a
  // surprise (the rename succeeded so the workspace is still on
  // disk under the new id); surface as 500 but do NOT roll back the
  // rename — the rename itself is durable and the user can retry
  // pubkey derivation by reloading.
  try {
    const derived = await derivePubkeyViaSigner(newId);
    return NextResponse.json({
      ok: true as const,
      workspace_id: newId,
      kid: perTenantKidFor(newId),
      pubkey_b64url: derived.pubkey_b64url,
    });
  } catch (e) {
    if (e instanceof SignerError) {
      return jsonError(500, `signer: ${redactPaths(e.message)}`);
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `unexpected: ${redactPaths(msg)}`);
  }
}

// ──────────────── DELETE /api/atlas/workspaces (W20c) ────────────────

export async function DELETE(req: Request): Promise<NextResponse> {
  // Byte-layer cap.
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
    return jsonError(
      400,
      `request body is not valid JSON: ${(e as Error).message}`,
    );
  }

  const parsed = DeleteWorkspaceSchema.safeParse(body);
  if (!parsed.success) {
    return jsonError(400, safeZodErrorMessage(parsed.error));
  }
  const { workspace_id: targetId } = parsed.data;

  let targetDirPath: string;
  try {
    targetDirPath = workspaceDir(targetId);
  } catch (e) {
    if (e instanceof WorkspacePathError) {
      return jsonError(400, redactPaths(e.message));
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `unexpected: ${redactPaths(msg)}`);
  }

  // Existence check BEFORE the last-workspace gate — a 404 on a
  // non-existent id is friendlier than a 409 the client cannot resolve.
  try {
    const stat = await fs.stat(targetDirPath);
    if (!stat.isDirectory()) {
      return jsonError(404, "workspace not found");
    }
  } catch (e) {
    const code = (e as NodeJS.ErrnoException).code;
    if (code === "ENOENT") {
      return jsonError(404, "workspace not found");
    }
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `stat: ${redactPaths(msg)}`);
  }

  // DA-6: refuse to delete the last user-facing workspace. Counts the
  // post-delete user-facing total — if it's exactly 1 right now AND
  // the target itself is user-facing, the delete would leave the data
  // root empty and flip the UI into FirstRunWizard mode mid-session.
  let totalUserFacing: number;
  try {
    totalUserFacing = await countUserFacingWorkspaces();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `list: ${redactPaths(msg)}`);
  }
  const targetIsUserFacing =
    isValidWorkspaceId(targetId) && !CI_ARTIFACT_PATTERN.test(targetId);
  if (targetIsUserFacing && totalUserFacing <= 1) {
    return jsonError(409, "cannot delete last workspace");
  }

  try {
    // `force: false` so a vanished dir (concurrent delete) surfaces
    // the real error rather than masquerading as a successful delete.
    await fs.rm(targetDirPath, { recursive: true, force: false });
    return NextResponse.json({
      ok: true as const,
      workspace_id: targetId,
    });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return jsonError(500, `delete: ${redactPaths(msg)}`);
  }
}
