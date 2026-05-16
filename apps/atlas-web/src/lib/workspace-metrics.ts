/**
 * W20b-1 — pure workspace metrics computation.
 *
 * Browser-safe (no Node imports). Computes the 7 KPI values rendered
 * by the `<FullTier>` of `<DashboardMetricsSection>` from a workspace's
 * raw events.jsonl content (decoded as `AtlasEvent[]` by the
 * `/api/atlas/trace` route handler).
 *
 * Threat model (TM-W20b-1):
 *   The input is user-controlled — events come from the user's local
 *   `events.jsonl`, but in a future welle they may come from a remote
 *   peer's exported trace. Every property access on `event.payload`,
 *   `event.signature.kid`, and `event.ts` must therefore be type-guarded
 *   so a single malformed event cannot crash the whole dashboard.
 *
 *   Malformed events still count toward `totalEvents` (so the
 *   indicator never under-reports). They are skipped only for the
 *   time-window aggregates and signer-set computation where the
 *   malformed field is the one being read.
 *
 * Determinism:
 *   `nowMs` is injected so tests pin the "current time". Production
 *   callers pass `Date.now()` (the default).
 *
 * Why not import `computeTips` from `@atlas/bridge`?
 *   The bridge's `storage.ts` module imports `node:fs` at load time,
 *   so importing anything from it pulls Node-only code into the
 *   browser bundle. We inline the (3-line) tip computation here. A
 *   future welle can lift it into a shared browser-safe sub-module if
 *   another consumer needs it.
 */

import type { AtlasEvent } from "@atlas/bridge";

export interface WorkspaceMetrics {
  /** Total event count, including malformed events. */
  totalEvents: number;
  /** Events with parseable `ts` falling within (now - 30d, now]. */
  eventsLast30d: number;
  /** Events with parseable `ts` falling within (now - 60d, now - 30d]. */
  eventsLast30dPrior: number;
  /** Count of events per payload.type (skips events without string type). */
  eventsByType: Record<string, number>;
  /** Unique signature.kid values seen (sorted, dedup'd). */
  uniqueSigners: ReadonlyArray<string>;
  /**
   * Longest parent-chain depth in the DAG.
   *  - 0 events → 0
   *  - genesis-only (no parents) → 1
   *  - N-long linear chain → N
   */
  dagDepth: number;
  /**
   * Count of "anchor" events: payload.type starts with `"anchor."`
   * OR payload has an `anchor_proof` field.
   */
  anchorCount: number;
  /**
   * Count of tips — events whose `event_hash` is not referenced as a
   * parent by any other event. Matches `computeTips` from the bridge.
   */
  tipCount: number;
}

const MS_PER_DAY = 86_400_000;

function isStringValue(v: unknown): v is string {
  return typeof v === "string";
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

/** Parse an ISO-8601 ts string into ms. Returns null when unparseable. */
function tryParseTsMs(ts: unknown): number | null {
  if (!isStringValue(ts)) return null;
  const ms = Date.parse(ts);
  if (Number.isNaN(ms)) return null;
  return ms;
}

/** Safely read `event.payload.type` as a string, or null. */
function tryReadPayloadType(event: AtlasEvent): string | null {
  if (!isObject(event.payload)) return null;
  const t = event.payload.type;
  return isStringValue(t) ? t : null;
}

/** True if the event looks like an anchor event. */
function isAnchorEvent(event: AtlasEvent): boolean {
  const payloadType = tryReadPayloadType(event);
  if (payloadType !== null && payloadType.startsWith("anchor.")) return true;
  if (isObject(event.payload) && "anchor_proof" in event.payload) return true;
  return false;
}

/** Tips: events whose hash is not in any other event's parent_hashes. */
function computeTipCount(events: ReadonlyArray<AtlasEvent>): number {
  const referenced = new Set<string>();
  for (const ev of events) {
    if (Array.isArray(ev.parent_hashes)) {
      for (const p of ev.parent_hashes) {
        if (isStringValue(p)) referenced.add(p);
      }
    }
  }
  let count = 0;
  for (const ev of events) {
    if (isStringValue(ev.event_hash) && !referenced.has(ev.event_hash)) {
      count += 1;
    }
  }
  return count;
}

/**
 * Compute DAG depth — the length of the longest parent-chain.
 *
 * Algorithm: iterative DFS with memoisation over the parent map.
 * `depth(node)` = 1 if node has no parents, else 1 + max(depth(parent))
 * over all parents that are also in the event set. Parents not in the
 * event set are treated as depth-0 anchors (defensive — should never
 * happen on a valid trace, but if a malformed event references an
 * unknown parent we still want a stable answer).
 *
 * For 0 events: 0. For genesis-only: 1.
 */
function computeDagDepth(events: ReadonlyArray<AtlasEvent>): number {
  if (events.length === 0) return 0;

  // Build hash → event map (skipping events with non-string event_hash).
  const byHash = new Map<string, AtlasEvent>();
  for (const ev of events) {
    if (isStringValue(ev.event_hash)) byHash.set(ev.event_hash, ev);
  }
  if (byHash.size === 0) return 0;

  const memo = new Map<string, number>();
  const inProgress = new Set<string>();

  function depthOf(hash: string): number {
    const cached = memo.get(hash);
    if (cached !== undefined) return cached;
    // Cycle guard — well-formed DAGs cannot cycle, but a malformed
    // trace could; we return 1 for any node currently on the stack to
    // bound recursion without throwing.
    if (inProgress.has(hash)) return 1;
    const ev = byHash.get(hash);
    if (ev === undefined) return 0;
    inProgress.add(hash);
    let parentsMax = 0;
    if (Array.isArray(ev.parent_hashes)) {
      for (const p of ev.parent_hashes) {
        if (!isStringValue(p)) continue;
        const pd = depthOf(p);
        if (pd > parentsMax) parentsMax = pd;
      }
    }
    const d = parentsMax + 1;
    inProgress.delete(hash);
    memo.set(hash, d);
    return d;
  }

  let max = 0;
  for (const hash of byHash.keys()) {
    const d = depthOf(hash);
    if (d > max) max = d;
  }
  return max;
}

/**
 * Compute the full metrics bundle for a workspace's events.
 *
 * @param events  raw events array (already JSON-decoded)
 * @param nowMs   optional injection point for the "now" timestamp
 *                (defaults to `Date.now()`). Tests pass a fixed value.
 */
export function computeWorkspaceMetrics(
  events: ReadonlyArray<AtlasEvent>,
  nowMs?: number,
): WorkspaceMetrics {
  const now = nowMs ?? Date.now();
  const totalEvents = events.length;

  let eventsLast30d = 0;
  let eventsLast30dPrior = 0;
  const eventsByType: Record<string, number> = {};
  const signers = new Set<string>();
  let anchorCount = 0;

  for (const ev of events) {
    // Type aggregate
    const payloadType = tryReadPayloadType(ev);
    if (payloadType !== null) {
      eventsByType[payloadType] = (eventsByType[payloadType] ?? 0) + 1;
    }
    // Signer aggregate — robust to malformed signature objects.
    if (
      isObject(ev.signature) &&
      isStringValue((ev.signature as Record<string, unknown>).kid)
    ) {
      signers.add((ev.signature as Record<string, unknown>).kid as string);
    }
    // Anchor detection.
    if (isAnchorEvent(ev)) anchorCount += 1;
    // Time-window aggregates — skip malformed ts.
    const tsMs = tryParseTsMs(ev.ts);
    if (tsMs === null) continue;
    const ageMs = now - tsMs;
    if (ageMs >= 0 && ageMs <= 30 * MS_PER_DAY) {
      eventsLast30d += 1;
    } else if (ageMs > 30 * MS_PER_DAY && ageMs <= 60 * MS_PER_DAY) {
      eventsLast30dPrior += 1;
    }
  }

  const uniqueSigners = Array.from(signers).sort();
  const dagDepth = computeDagDepth(events);
  const tipCount = computeTipCount(events);

  return {
    totalEvents,
    eventsLast30d,
    eventsLast30dPrior,
    eventsByType,
    uniqueSigners,
    dagDepth,
    anchorCount,
    tipCount,
  };
}
