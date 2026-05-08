/**
 * High-level "write one event" pipeline.
 *
 * Tools call `writeSignedEvent`, which:
 *   1. Reads the current events.jsonl to compute current DAG tips
 *   2. Generates a fresh ULID event-id
 *   3. Spawns `atlas-signer` to produce a canonical-CBOR-signed AtlasEvent
 *   4. Appends the signed event to events.jsonl
 *
 * Writes are serialised per-workspace via an in-process mutex. Two
 * concurrent tool calls into the same workspace previously raced — they
 * both computed identical parents from a stale snapshot, both signed,
 * both appended — producing a *fork* in the DAG rather than a chain.
 * The verifier accepts forks (it's a DAG, not a chain) but the agent's
 * mental model "I wrote A then B builds on A" silently broke.
 *
 * The mutex is in-process. V2 multi-process MCP deployments need an
 * external lock service (file lock, advisory DB lock, etc.); the seam
 * is `withWorkspaceLock` and is the only place to change.
 */

import { resolveIdentityForKid } from "./keys.js";
import { signEvent, SignerError } from "./signer.js";
import { appendEvent, computeTips, readAllEvents } from "./storage.js";
import type { AtlasEvent } from "./types.js";
import { ulid } from "./ulid.js";

export type WriteEventArgs = {
  workspaceId: string;
  kid: string;
  payload: Record<string, unknown>;
  /** Optional explicit parents. If omitted, current DAG tips are used. */
  parents?: string[];
  /** Optional explicit timestamp. If omitted, current wall-clock UTC. */
  ts?: string;
};

export type WriteEventResult = {
  event: AtlasEvent;
  parentsUsed: string[];
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
 * Common write path used by every signed-event MCP tool.
 */
export async function writeSignedEvent(args: WriteEventArgs): Promise<WriteEventResult> {
  const identity = await resolveIdentityForKid(args.kid);
  if (!identity) {
    throw new Error(
      `unknown kid: ${args.kid}. Legacy V1 dev kids live in src/lib/keys.ts ` +
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
      kid: args.kid,
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
    return { event, parentsUsed: parents };
  });
}

/** UTC ISO-8601 with second-level resolution (matches signer's expected format). */
function nowIso(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}
