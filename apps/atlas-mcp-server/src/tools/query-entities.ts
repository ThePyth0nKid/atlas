/**
 * `atlas_query_entities` — list entities by kind + filter.
 *
 * V2-β Phase-4 ships the contract surface only; backing data comes from
 * the ArcadeDB-backed projection store in V2-β Phase-7 (W17). Until then
 * the stubbed store throws a structured "Not implemented" error which
 * the handler translates into an MCP `isError: true` response.
 */

import { z } from "zod";
import { DEFAULT_WORKSPACE } from "@atlas/bridge";
import { optionalWorkspaceIdSchema } from "./schema.js";
import {
  getProjectionStore,
  PROJECTION_STORE_STUB_MESSAGE,
} from "./_lib/projection-store.js";
import type { ToolDefinition, ToolHandlerResult } from "./types.js";

export const queryEntitiesInputSchema = {
  workspace_id: optionalWorkspaceIdSchema
    .describe(`Workspace id; defaults to "${DEFAULT_WORKSPACE}". [a-zA-Z0-9_-]{1,128}.`),
  kind: z.string().min(1).max(128).optional()
    .describe("Optional entity kind to filter by (e.g. 'dataset', 'model')."),
  limit: z.number().int().min(1).max(500).default(50)
    .describe("Maximum number of entities to return. Hard cap at 500 (DoS)."),
  filter: z.record(z.string(), z.unknown()).default({})
    .describe(
      "Optional attribute filter (key/value match). Values are matched " +
        "with strict equality.",
    ),
};

const inputZ = z.object(queryEntitiesInputSchema);

function toolError(reason: string): ToolHandlerResult {
  return {
    isError: true,
    content: [
      {
        type: "text" as const,
        text: JSON.stringify({ ok: false, error: reason }, null, 2),
      },
    ],
  };
}

export const queryEntitiesTool: ToolDefinition<typeof queryEntitiesInputSchema> = {
  name: "atlas_query_entities",
  description:
    "List entities in a workspace's projection, optionally filtered by " +
    "kind + attributes. Returns up to `limit` entries (hard-capped at 500). " +
    "V2-β Phase-4 backs this with a projection-store stub; ArcadeDB-backed " +
    "execution lands in V2-β Phase 7 (W17).",
  inputSchema: queryEntitiesInputSchema,
  handler: async (raw) => {
    let args: z.infer<typeof inputZ>;
    try {
      args = inputZ.parse(raw);
    } catch (e: unknown) {
      return toolError(
        e instanceof Error ? `invalid input: ${e.message}` : "invalid input",
      );
    }
    const workspaceId = args.workspace_id ?? DEFAULT_WORKSPACE;

    try {
      const entities = await getProjectionStore().listEntities(workspaceId, {
        kind: args.kind,
        filter: args.filter,
        limit: args.limit,
      });
      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify(
              {
                ok: true,
                workspace_id: workspaceId,
                kind: args.kind ?? null,
                count: entities.length,
                entities,
              },
              null,
              2,
            ),
          },
        ],
      };
    } catch (e: unknown) {
      // Allowlist: only the documented stub sentinel passes through to
      // MCP clients. All other errors swallowed to a generic string so
      // W17's ArcadeDB driver internals do not leak. See DECISION-SEC-4.
      const isKnownStub =
        e instanceof Error && e.message === PROJECTION_STORE_STUB_MESSAGE;
      return toolError(
        isKnownStub ? e.message : "projection-store call failed",
      );
    }
  },
};
