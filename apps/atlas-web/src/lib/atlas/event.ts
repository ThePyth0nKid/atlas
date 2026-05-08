/**
 * V1.19 Welle 1 — atlas-web "write one signed event" pipeline.
 *
 * DUPLICATED FROM `apps/atlas-mcp-server/src/lib/event.ts`. Per-tenant
 * path only — the legacy SPIFFE branch is not exposed here because
 * the web write surface auto-derives `kid` from the workspace id and
 * never accepts a caller-supplied kid.
 *
 * Concurrency model: per-workspace mutex serialises writes WITHIN
 * this Node process. Two concurrent web requests against the same
 * workspace queue behind one another so they observe each other's
 * appended events when computing parents — without the lock both
 * would compute identical parents from a stale snapshot, both would
 * sign, both would append, producing a fork. The verifier accepts
 * forks (it's a DAG), but the agent's mental model "I wrote A, then
 * B builds on A" silently breaks.
 *
 * Cross-PROCESS coordination (atlas-web AND atlas-mcp-server writing
 * to the same workspace at the same time) is V2 territory. For V1
 * deployments, run one writer process per workspace.
 */

import { resolvePerTenantIdentity } from "./keys";
import { signEvent, SignerError } from "./signer";
import { appendEvent, computeTips, readAllEvents } from "./storage";
import type { AtlasEvent } from "./types";
import { ulid } from "./ulid";

export type WriteEventArgs = {
  workspaceId: string;
  payload: Record<string, unknown>;
  /** Optional explicit parents. If omitted, current DAG tips are used. */
  parents?: string[];
  /** Optional explicit timestamp. If omitted, current wall-clock UTC. */
  ts?: string;
};

export type WriteEventResult = {
  event: AtlasEvent;
  parentsUsed: string[];
  kid: string;
};

const workspaceLocks = new Map<string, Promise<unknown>>();

async function withWorkspaceLock<T>(
  workspaceId: string,
  fn: () => Promise<T>,
): Promise<T> {
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
 * Web write path: derive per-tenant kid, compute parents from current
 * tips, sign via `--derive-from-workspace` (no secret in Node heap),
 * append to events.jsonl atomically.
 */
export async function writeSignedEvent(args: WriteEventArgs): Promise<WriteEventResult> {
  const identity = await resolvePerTenantIdentity(args.workspaceId);

  return withWorkspaceLock(args.workspaceId, async () => {
    const parents = args.parents ?? computeTips(await readAllEvents(args.workspaceId));
    const ts = args.ts ?? nowIso();
    const eventId = ulid();

    let event: AtlasEvent;
    try {
      event = await signEvent({
        workspace: args.workspaceId,
        eventId,
        ts,
        kid: identity.kid,
        parents,
        payload: args.payload,
        deriveFromWorkspace: identity.workspaceId,
      });
    } catch (e) {
      if (e instanceof SignerError) throw e;
      throw new Error(`signer failed: ${(e as Error).message}`);
    }

    await appendEvent(args.workspaceId, event);
    return { event, parentsUsed: parents, kid: identity.kid };
  });
}

/** UTC ISO-8601 with second-level resolution (matches signer's expected format). */
function nowIso(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}
