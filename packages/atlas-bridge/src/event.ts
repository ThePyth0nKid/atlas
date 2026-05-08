/**
 * High-level "write one signed event" pipeline, consolidated for both
 * atlas-mcp-server tools and atlas-web's POST /api/atlas/write-node.
 *
 * Callers invoke `writeSignedEvent`, which:
 *   1. Resolves the signing identity (per-tenant kid auto-derived from
 *      `workspaceId` if no `kid` is supplied; otherwise the explicit
 *      `kid` is looked up via the legacy + per-tenant resolver).
 *   2. Reads the current events.jsonl to compute current DAG tips
 *      (unless explicit `parents` are passed).
 *   3. Generates a fresh ULID event-id.
 *   4. Spawns `atlas-signer` to produce a canonical-CBOR-signed
 *      AtlasEvent. The per-tenant secret never crosses the subprocess
 *      boundary; the legacy SPIFFE secret is piped via stdin (never
 *      argv).
 *   5. Appends the signed event to events.jsonl atomically.
 *
 * Concurrency model: per-workspace mutex serialises writes WITHIN this
 * Node process. Two concurrent calls into the same workspace queue
 * behind one another so they observe each other's appended events when
 * computing parents. Without the lock, both would compute identical
 * parents from a stale snapshot, both would sign, both would append —
 * producing a fork in the DAG. The verifier accepts forks (it's a
 * DAG), but the agent's mental model "I wrote A, then B builds on A"
 * silently breaks.
 *
 * Cross-PROCESS coordination (atlas-web AND atlas-mcp-server writing
 * to the same workspace concurrently) is V2 territory — the seam is
 * `withWorkspaceLock` and is the only place that needs to change.
 * For V1 deployments, run one writer process per workspace.
 */

import { perTenantKidFor, resolveIdentityForKid } from "./keys.js";
import { signEvent, SignerError } from "./signer.js";
import { appendEvent, computeTips, readAllEvents } from "./storage.js";
import type { AtlasEvent } from "./types.js";
import { ulid } from "./ulid.js";

/**
 * Write-time arguments. `kid` is optional — when omitted, the bridge
 * derives the canonical per-tenant kid from `workspaceId`
 * (`atlas-anchor:{workspaceId}`). Atlas-web's web write surface relies
 * on this auto-derivation: the route handler never accepts a caller-
 * supplied kid because letting the browser choose its kid would
 * un-narrow the trust boundary the per-tenant scheme exists to
 * enforce. MCP tool callers that need to write under a specific
 * legacy SPIFFE kid (agent / human / anchor) supply `kid` explicitly.
 */
export type WriteEventArgs = {
  workspaceId: string;
  payload: Record<string, unknown>;
  /** Optional explicit kid. If omitted, the per-tenant kid for `workspaceId` is used. */
  kid?: string;
  /** Optional explicit parents. If omitted, current DAG tips are used. */
  parents?: string[];
  /** Optional explicit timestamp. If omitted, current wall-clock UTC. */
  ts?: string;
};

export type WriteEventResult = {
  event: AtlasEvent;
  parentsUsed: string[];
  /**
   * The kid that actually signed this event. Identical to `args.kid`
   * when one was supplied; otherwise the auto-derived per-tenant kid.
   * Always present so the API response can echo it back without the
   * caller re-deriving.
   */
  kid: string;
};

const workspaceLocks = new Map<string, Promise<unknown>>();

/**
 * Run `fn` while holding the per-workspace lock. Subsequent writes to
 * the same workspace queue behind this one. Different workspaces run
 * concurrently.
 */
async function withWorkspaceLock<T>(workspaceId: string, fn: () => Promise<T>): Promise<T> {
  const prev = workspaceLocks.get(workspaceId) ?? Promise.resolve();
  let release!: () => void;
  const next = new Promise<void>((resolve) => {
    release = resolve;
  });
  // V1.19 Welle 1 review fix: capture the chained promise once. The
  // previous code did `workspaceLocks.set(id, prev.then(() => next))`
  // and later `workspaceLocks.get(id) === prev.then(() => next)` — but
  // each `prev.then(...)` call returns a *new* Promise object, so the
  // identity check was always false and the map grew without bound on
  // every write. Storing the tail in a local before mutating the map
  // makes the cleanup check actually identity-equal.
  const tail = prev.then(() => next);
  workspaceLocks.set(workspaceId, tail);
  try {
    await prev;
    return await fn();
  } finally {
    release();
    // Best-effort cleanup: only delete the entry if we are still the
    // most-recently-registered tail. A newer waiter that arrived
    // between our `release()` and this check has already overwritten
    // the entry; leave it for that waiter's own finally-block.
    if (workspaceLocks.get(workspaceId) === tail) {
      workspaceLocks.delete(workspaceId);
    }
  }
}

/**
 * Common write path used by every signed-event MCP tool AND by the
 * atlas-web write surface. See module-level doc-comment for the full
 * pipeline, locking model, and security properties.
 */
export async function writeSignedEvent(args: WriteEventArgs): Promise<WriteEventResult> {
  const effectiveKid = args.kid ?? perTenantKidFor(args.workspaceId);
  const identity = await resolveIdentityForKid(effectiveKid);
  if (!identity) {
    throw new Error(
      `unknown kid: ${effectiveKid}. Legacy V1 dev kids live in keys.ts ` +
        `(agent / human / anchor); per-tenant kids must match the shape ` +
        `'atlas-anchor:{workspace_id}'.`,
    );
  }

  return withWorkspaceLock(args.workspaceId, async () => {
    const parents = args.parents ?? computeTips(await readAllEvents(args.workspaceId));
    const ts = args.ts ?? nowIso();
    const eventId = ulid();

    // V1.9: dispatch by identity.secretSource. Legacy SPIFFE kids pipe
    // their hex secret through stdin; per-tenant kids let the signer
    // derive internally so no secret crosses this process boundary.
    const baseSign = {
      workspace: args.workspaceId,
      eventId,
      ts,
      kid: effectiveKid,
      parents,
      payload: args.payload,
    } as const;

    let event: AtlasEvent;
    try {
      event =
        identity.secretSource === "hex"
          ? await signEvent({ ...baseSign, secretHex: identity.secretHex })
          : await signEvent({ ...baseSign, deriveFromWorkspace: identity.workspaceId });
    } catch (e) {
      if (e instanceof SignerError) throw e;
      throw new Error(`signer failed: ${(e as Error).message}`);
    }

    await appendEvent(args.workspaceId, event);
    return { event, parentsUsed: parents, kid: effectiveKid };
  });
}

/** UTC ISO-8601 with second-level resolution (matches signer's expected format). */
function nowIso(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}
