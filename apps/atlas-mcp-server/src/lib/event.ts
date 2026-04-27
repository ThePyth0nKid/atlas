/**
 * High-level "write one event" pipeline.
 *
 * Tools call `writeSignedEvent`, which:
 *   1. Reads the current events.jsonl to compute current DAG tips
 *   2. Generates a fresh ULID event-id
 *   3. Spawns `atlas-signer` to produce a canonical-CBOR-signed AtlasEvent
 *   4. Appends the signed event to events.jsonl
 *
 * The pipeline is intentionally serial within a single MCP process. V1
 * does not multiplex writes; tools are expected to be called sequentially
 * by the host (Claude/Cursor) which already serialises tool dispatch.
 *
 * Wall-clock and ULID generation happen here, not inside the signer.
 * That keeps the signer side-effect-free: same inputs → same signature.
 */

import { identityForKid } from "./keys.js";
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

/**
 * Common write path used by every signed-event MCP tool.
 *
 * Throws on:
 *   - unknown kid (no matching dev identity)
 *   - signer binary missing or non-zero exit
 *   - storage append failure
 */
export async function writeSignedEvent(args: WriteEventArgs): Promise<WriteEventResult> {
  const identity = identityForKid(args.kid);
  if (!identity) {
    throw new Error(
      `unknown kid: ${args.kid}. V1 dev keys: see src/lib/keys.ts (agent / human / anchor).`,
    );
  }

  const parents = args.parents ?? computeTips(await readAllEvents(args.workspaceId));
  const ts = args.ts ?? nowIso();
  const eventId = ulid();

  let event: AtlasEvent;
  try {
    event = await signEvent({
      workspace: args.workspaceId,
      eventId,
      ts,
      kid: args.kid,
      parents,
      payload: args.payload,
      secretHex: identity.secretHex,
    });
  } catch (e) {
    if (e instanceof SignerError) throw e;
    throw new Error(`signer failed: ${(e as Error).message}`);
  }

  await appendEvent(args.workspaceId, event);
  return { event, parentsUsed: parents };
}

/** UTC ISO-8601 with second-level resolution (matches signer's expected format). */
function nowIso(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}
