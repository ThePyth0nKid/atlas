/**
 * `atlas_query_provenance` — return the signed-event chain that produced
 * the current projection state of an entity.
 *
 * This tool intentionally does NOT take a Cypher query. The provenance
 * chain is a first-class concept derived from the workspace's AtlasEvent
 * log (the source-of-truth); exposing it via a hand-built endpoint
 * keeps the tool boundary narrow and avoids re-deriving the chain via
 * Cypher.
 *
 * V2-β Phase-4 ships the contract surface only; backing data comes from
 * the ArcadeDB-backed projection store in V2-β Phase-7 (W17). Until then
 * the stubbed store throws a structured "Not implemented" error which
 * the handler translates into an MCP `isError: true` response.
 */

import { z } from "zod";
import { DEFAULT_WORKSPACE } from "@atlas/bridge";
import { optionalWorkspaceIdSchema } from "./schema.js";
import { getProjectionStore } from "./_lib/projection-store.js";
import type { ToolDefinition, ToolHandlerResult } from "./types.js";

/**
 * UUIDs come in a few flavours; we accept the standard 36-char hyphen-
 * separated form plus the 32-char hexless form. Strict regex at the Zod
 * boundary closes off arbitrary string-injection.
 */
const ENTITY_UUID_PATTERN =
  /^[a-fA-F0-9]{8}-?[a-fA-F0-9]{4}-?[a-fA-F0-9]{4}-?[a-fA-F0-9]{4}-?[a-fA-F0-9]{12}$/;

export const queryProvenanceInputSchema = {
  workspace_id: optionalWorkspaceIdSchema
    .describe(`Workspace id; defaults to "${DEFAULT_WORKSPACE}". [a-zA-Z0-9_-]{1,128}.`),
  entity_uuid: z.string().regex(ENTITY_UUID_PATTERN, "entity_uuid: invalid UUID")
    .describe("Target entity's UUID (with or without hyphens)."),
};

const inputZ = z.object(queryProvenanceInputSchema);

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

export const queryProvenanceTool: ToolDefinition<typeof queryProvenanceInputSchema> = {
  name: "atlas_query_provenance",
  description:
    "Return the signed-event chain that produced an entity's current " +
    "projection state. Use this to answer 'how did we get here?' for any " +
    "node — events are returned in chronological order with hashes and " +
    "key-ids. V2-β Phase-4 backs this with a projection-store stub; " +
    "ArcadeDB-backed execution lands in V2-β Phase 7 (W17).",
  inputSchema: queryProvenanceInputSchema,
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
      const chain = await getProjectionStore().provenance(
        workspaceId,
        args.entity_uuid,
      );
      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify(
              {
                ok: true,
                workspace_id: workspaceId,
                entity_uuid: args.entity_uuid,
                event_count: chain.length,
                events: chain,
              },
              null,
              2,
            ),
          },
        ],
      };
    } catch (e: unknown) {
      return toolError(
        e instanceof Error ? e.message : "projection-store call failed",
      );
    }
  },
};
