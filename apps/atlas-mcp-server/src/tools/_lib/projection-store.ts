/**
 * `ProjectionStore` — the data-layer contract that V2-β Phase-4 MCP tools
 * call into. Phase-4 (this welle) ships only the CONTRACT SURFACE; the
 * real ArcadeDB-backed implementation lands in V2-β Phase-7 (W17a/b/c).
 *
 * For Phase-4, the default impl throws a structured "not implemented"
 * error. Tests inject a fake implementation. Consumers (MCP tool
 * handlers) translate the throw into a tool-error response envelope so
 * MCP clients receive a clean structured error rather than a 500.
 *
 * The interface is read-only by design. Write paths exist elsewhere
 * (`writeSignedEvent`, etc. in `@atlas/bridge`); the projection store is
 * a derivative view of those events. See Master Vision v1 §5.4 + §5.5.
 */

/**
 * A graph entity (node) projection. Schema is intentionally loose at this
 * Phase-4 stage — W17 (ArcadeDB) will firm it up against the real
 * schema. Tools surface this shape inside the MCP `content[0].text`
 * JSON envelope.
 */
export interface ProjectionEntity {
  readonly entity_uuid: string;
  readonly kind: string;
  readonly attributes: Readonly<Record<string, unknown>>;
}

/**
 * A provenance event link — the chain of signed AtlasEvents that
 * contributed to an entity's current projection state.
 */
export interface ProvenanceEntry {
  readonly event_id: string;
  readonly event_hash: string;
  readonly ts: string;
  readonly kid: string;
  readonly type: string;
}

/**
 * A timeline entry — an AtlasEvent within a time window. Same shape as
 * `workspace_state`'s `recent` array, intentional alignment so MCP
 * clients see a consistent envelope.
 */
export interface TimelineEntry {
  readonly event_id: string;
  readonly event_hash: string;
  readonly ts: string;
  readonly kid: string;
  readonly type: string;
}

/**
 * A raw Cypher query result row. Phase-4 stub never produces a real row;
 * V2-β Phase-7's ArcadeDB driver returns concrete rows per the Cypher
 * RETURN clause's column structure.
 */
export type CypherResultRow = Readonly<Record<string, unknown>>;

export interface ProjectionStore {
  /**
   * Run a validated Cypher query. The validator (`validateReadOnlyCypher`)
   * MUST run BEFORE this call — the store assumes the input is already
   * allowlisted.
   */
  runCypher(
    workspaceId: string,
    cypher: string,
    params: Readonly<Record<string, unknown>>,
  ): Promise<ReadonlyArray<CypherResultRow>>;

  /**
   * List entities filtered by kind + attribute filter.
   */
  listEntities(
    workspaceId: string,
    options: Readonly<{
      kind?: string;
      filter?: Readonly<Record<string, unknown>>;
      limit: number;
    }>,
  ): Promise<ReadonlyArray<ProjectionEntity>>;

  /**
   * Return the provenance chain for an entity by its uuid.
   */
  provenance(
    workspaceId: string,
    entityUuid: string,
  ): Promise<ReadonlyArray<ProvenanceEntry>>;

  /**
   * Return a windowed slice of events for a workspace.
   */
  timeline(
    workspaceId: string,
    options: Readonly<{
      from?: string;
      to?: string;
      limit: number;
    }>,
  ): Promise<ReadonlyArray<TimelineEntry>>;
}

/**
 * Sentinel message used by both the default stub and tool error paths so
 * MCP clients see a recognisable string. Documented for V2-β consumers.
 */
export const PROJECTION_STORE_STUB_MESSAGE =
  "Not implemented; ArcadeDB-backed projection lands in V2-β Phase 7 / W17";

class StubProjectionStore implements ProjectionStore {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async runCypher(
    _workspaceId: string,
    _cypher: string,
    _params: Readonly<Record<string, unknown>>,
  ): Promise<ReadonlyArray<CypherResultRow>> {
    throw new Error(PROJECTION_STORE_STUB_MESSAGE);
  }
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async listEntities(
    _workspaceId: string,
    _options: Readonly<{
      kind?: string;
      filter?: Readonly<Record<string, unknown>>;
      limit: number;
    }>,
  ): Promise<ReadonlyArray<ProjectionEntity>> {
    throw new Error(PROJECTION_STORE_STUB_MESSAGE);
  }
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async provenance(
    _workspaceId: string,
    _entityUuid: string,
  ): Promise<ReadonlyArray<ProvenanceEntry>> {
    throw new Error(PROJECTION_STORE_STUB_MESSAGE);
  }
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  async timeline(
    _workspaceId: string,
    _options: Readonly<{ from?: string; to?: string; limit: number }>,
  ): Promise<ReadonlyArray<TimelineEntry>> {
    throw new Error(PROJECTION_STORE_STUB_MESSAGE);
  }
}

/**
 * Process-wide projection-store instance. Tools call `getProjectionStore()`
 * rather than newing the stub directly so tests can swap a fake in via
 * `setProjectionStore()`. W17 will replace the default with the
 * ArcadeDB-backed driver — touching ONE function (`setProjectionStore`)
 * to install it rather than touching every tool file.
 */
let activeStore: ProjectionStore = new StubProjectionStore();

export function getProjectionStore(): ProjectionStore {
  return activeStore;
}

export function setProjectionStore(store: ProjectionStore): void {
  activeStore = store;
}

/**
 * Reset to the default stub. Used by tests for isolation between cases.
 */
export function resetProjectionStore(): void {
  activeStore = new StubProjectionStore();
}
