/**
 * V2-β Welle 12 — In-memory projection store for the Read-API.
 *
 * V2-β Phase 4 establishes the **API contract surface**; the real
 * backend lands in Phase 7 / Welle 17 (ArcadeDB driver) per
 * `docs/V2-BETA-ORCHESTRATION-PLAN.md` §2 and ADR-Atlas-010/011.
 *
 * Until then, the Read-API routes consume this `ProjectionStore`
 * interface, and the supplied `EventsJsonlProjectionStore` builds
 * an in-memory projection by reading `events.jsonl` via
 * `@atlas/bridge.readAllEvents`. The interface is the seam — W17a
 * scaffolds the trait, W17b swaps the impl for `ArcadeDBProjection
 * Store`, and the route handlers stay byte-identical.
 *
 * Projection semantics (V2-β beta.1):
 *
 *   - `node.create` events produce entities. `entity_uuid` = the
 *     `node.id` field; later events MAY supersede properties via
 *     `node.update` (last-write-wins by event ordering).
 *
 *   - `edge.create` events produce edges. Edge endpoints are
 *     `payload.edge.from` and `payload.edge.to` (entity ids).
 *
 *   - Every event is retrievable by `event_uuid` (= `event_id`).
 *
 *   - The timeline is a chronological slice over events, filtered
 *     by ISO-8601 [from, to] window. ts is parsed defensively; any
 *     unparseable event ts is excluded from windowed slices.
 *
 * Threat model:
 *
 *   - Workspace ids are filesystem-path tokens at the storage
 *     layer; `@atlas/bridge.readAllEvents` validates the id via
 *     `isValidWorkspaceId` (regex `^[a-zA-Z0-9_-]{1,128}$`). The
 *     store layer therefore does not need to re-validate, but
 *     callers must — see the per-route handlers.
 *
 *   - This store is stateless across requests; every read re-scans
 *     `events.jsonl`. The handler-cache welle (post-W17) addresses
 *     the cost.
 */

import { readAllEvents } from "@atlas/bridge";
import type { AtlasEvent } from "@atlas/bridge";

/**
 * One projected entity (node) in the graph state.
 */
export interface ProjectedEntity {
  /** Stable identity from `node.id` in the originating event. */
  entity_uuid: string;
  /** The `node.kind` field — dataset / model / inference / document / other. */
  kind: string;
  /** Free-form attribute bag from the originating event(s). */
  properties: Record<string, unknown>;
  /** Signer key-id of the event that created this entity. */
  author_did: string;
  /** event_id of the originating `node.create` event. */
  created_event_uuid: string;
  /** ts of the originating event (ISO-8601). */
  created_at: string;
}

/**
 * One projected edge between two entities.
 */
export interface ProjectedEdge {
  /** Source entity id. */
  from: string;
  /** Target entity id. */
  to: string;
  /** Edge label / relationship kind. */
  kind: string;
  /** Free-form attribute bag from the originating event. */
  properties: Record<string, unknown>;
  /** Signer key-id of the originating event. */
  author_did: string;
  /** event_id of the originating event. */
  created_event_uuid: string;
  /** ts of the originating event. */
  created_at: string;
}

/**
 * One event in the timeline view, with the verifier-relevant fields
 * flattened for client display. The full event is retrievable via
 * `getEvent(event_uuid)` if a verifier-relevant audit is needed.
 */
export interface ProjectedTimelineEvent {
  event_uuid: string;
  event_hash: string;
  ts: string;
  kind: string;
  author_did: string;
}

export interface TimelineWindow {
  /** ISO-8601 inclusive lower bound. If omitted, no lower bound. */
  from?: string;
  /** ISO-8601 exclusive upper bound. If omitted, no upper bound. */
  to?: string;
  /** Maximum events to return. */
  limit: number;
}

/**
 * Read-side projection contract. W17 replaces the impl; the
 * interface stays stable. Route handlers depend on the interface,
 * not the impl.
 */
export interface ProjectionStore {
  getEntity(
    workspaceId: string,
    entityUuid: string,
  ): Promise<ProjectedEntity | null>;
  getRelated(
    workspaceId: string,
    entityUuid: string,
  ): Promise<{ outgoing: ProjectedEdge[]; incoming: ProjectedEdge[] } | null>;
  getTimeline(
    workspaceId: string,
    window: TimelineWindow,
  ): Promise<ProjectedTimelineEvent[]>;
  getEvent(workspaceId: string, eventUuid: string): Promise<AtlasEvent | null>;
}

/**
 * In-memory projection-store impl that derives state from
 * `events.jsonl` on each call. Suitable for V2-β beta.1 — Phase 7
 * (W17b) replaces this with an ArcadeDB-backed impl.
 *
 * No caching: we accept the per-request scan cost in exchange for
 * the simplicity property that every request reflects the most
 * recently appended event. The cost is O(n_events) per route call;
 * Phase 7 fixes this with a persistent backend.
 */
export class EventsJsonlProjectionStore implements ProjectionStore {
  async getEntity(
    workspaceId: string,
    entityUuid: string,
  ): Promise<ProjectedEntity | null> {
    const events = await readAllEvents(workspaceId);
    return projectEntity(events, entityUuid);
  }

  async getRelated(
    workspaceId: string,
    entityUuid: string,
  ): Promise<{ outgoing: ProjectedEdge[]; incoming: ProjectedEdge[] } | null> {
    const events = await readAllEvents(workspaceId);
    const entity = projectEntity(events, entityUuid);
    if (entity === null) return null;
    const edges = events
      .map((ev) => projectEdgeFromEvent(ev))
      .filter((e): e is ProjectedEdge => e !== null);
    return {
      outgoing: edges.filter((e) => e.from === entityUuid),
      incoming: edges.filter((e) => e.to === entityUuid),
    };
  }

  async getTimeline(
    workspaceId: string,
    window: TimelineWindow,
  ): Promise<ProjectedTimelineEvent[]> {
    const events = await readAllEvents(workspaceId);
    const fromMs = window.from ? Date.parse(window.from) : Number.NEGATIVE_INFINITY;
    const toMs = window.to ? Date.parse(window.to) : Number.POSITIVE_INFINITY;
    const projected: ProjectedTimelineEvent[] = [];
    for (const ev of events) {
      const tsMs = Date.parse(ev.ts);
      if (!Number.isFinite(tsMs)) continue;
      if (tsMs < fromMs) continue;
      if (tsMs >= toMs) continue;
      const kind = typeof ev.payload?.type === "string" ? ev.payload.type : "unknown";
      projected.push({
        event_uuid: ev.event_id,
        event_hash: ev.event_hash,
        ts: ev.ts,
        kind,
        author_did: ev.signature.kid,
      });
      if (projected.length >= window.limit) break;
    }
    return projected;
  }

  async getEvent(
    workspaceId: string,
    eventUuid: string,
  ): Promise<AtlasEvent | null> {
    const events = await readAllEvents(workspaceId);
    return events.find((ev) => ev.event_id === eventUuid) ?? null;
  }
}

/**
 * Reduce a slice of events to a single projected entity by
 * last-write-wins semantics on `node.create` + `node.update`
 * events. Returns null if no `node.create` event for `entityUuid`
 * is found.
 */
function projectEntity(
  events: readonly AtlasEvent[],
  entityUuid: string,
): ProjectedEntity | null {
  let entity: ProjectedEntity | null = null;
  for (const ev of events) {
    const type = ev.payload?.type;
    if (type === "node.create" && readNodeId(ev) === entityUuid) {
      const node = ev.payload.node as Record<string, unknown> | undefined;
      if (node === undefined) continue;
      const kindRaw = node.kind;
      const kind = typeof kindRaw === "string" ? kindRaw : "unknown";
      const { id: _id, kind: _kind, ...properties } = node;
      entity = {
        entity_uuid: entityUuid,
        kind,
        properties,
        author_did: ev.signature.kid,
        created_event_uuid: ev.event_id,
        created_at: ev.ts,
      };
    } else if (type === "node.update" && readNodeId(ev) === entityUuid) {
      if (entity === null) continue;
      const node = ev.payload.node as Record<string, unknown> | undefined;
      if (node === undefined) continue;
      const { id: _id, kind: _kind, ...newProps } = node;
      // Build the merged result against an explicit non-null view
      // — TS otherwise widens `entity` back to include `null` across
      // the surrounding loop's mutation seam.
      const merged: ProjectedEntity = {
        entity_uuid: entity.entity_uuid,
        kind: entity.kind,
        properties: { ...entity.properties, ...newProps },
        author_did: entity.author_did,
        created_event_uuid: entity.created_event_uuid,
        created_at: entity.created_at,
      };
      entity = merged;
    }
  }
  return entity;
}

function readNodeId(ev: AtlasEvent): string | null {
  const node = ev.payload?.node;
  if (typeof node !== "object" || node === null) return null;
  const id = (node as Record<string, unknown>).id;
  return typeof id === "string" ? id : null;
}

/**
 * Reduce one event to an edge, or null if it is not an
 * `edge.create` event with a well-formed payload.
 */
function projectEdgeFromEvent(ev: AtlasEvent): ProjectedEdge | null {
  if (ev.payload?.type !== "edge.create") return null;
  const edge = ev.payload.edge;
  if (typeof edge !== "object" || edge === null) return null;
  const e = edge as Record<string, unknown>;
  const from = typeof e.from === "string" ? e.from : null;
  const to = typeof e.to === "string" ? e.to : null;
  const kind = typeof e.kind === "string" ? e.kind : "related";
  if (from === null || to === null) return null;
  const { from: _f, to: _t, kind: _k, ...properties } = e;
  return {
    from,
    to,
    kind,
    properties,
    author_did: ev.signature.kid,
    created_event_uuid: ev.event_id,
    created_at: ev.ts,
  };
}
