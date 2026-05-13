/**
 * `atlas_get_timeline` — return signed events in a workspace within an
 * optional time window.
 *
 * Operationally complementary to `atlas_workspace_state` (which returns
 * a count + tail); the timeline tool is the windowed, paged read.
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

/**
 * ISO-8601 timestamp validator. Zod's `.datetime()` enforces the
 * standard format and rejects naive strings — defensive against
 * malformed time-window inputs that would otherwise reach the data
 * layer.
 */
const isoDateTime = z.string().datetime({ offset: true });

export const getTimelineInputSchema = {
  workspace_id: optionalWorkspaceIdSchema
    .describe(`Workspace id; defaults to "${DEFAULT_WORKSPACE}". [a-zA-Z0-9_-]{1,128}.`),
  from: isoDateTime.optional()
    .describe("Lower bound (inclusive). ISO-8601 datetime with offset."),
  to: isoDateTime.optional()
    .describe("Upper bound (inclusive). ISO-8601 datetime with offset."),
  limit: z.number().int().min(1).max(500).default(50)
    .describe("Maximum number of events to return. Hard cap at 500 (DoS)."),
};

const inputZ = z.object(getTimelineInputSchema);

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

export const getTimelineTool: ToolDefinition<typeof getTimelineInputSchema> = {
  name: "atlas_get_timeline",
  description:
    "Return AtlasEvents within an optional time window for a workspace. " +
    "Bounds (`from`, `to`) are ISO-8601 datetimes with offset. Hard-capped " +
    "at 500 entries per call. V2-β Phase-4 backs this with a projection-store " +
    "stub; ArcadeDB-backed execution lands in V2-β Phase 7 (W17).",
  inputSchema: getTimelineInputSchema,
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

    // Window sanity: `from` must be <= `to` when both are supplied.
    // Surfacing this here gives MCP clients an immediate, structured
    // error rather than the projection-store interpreting a reversed
    // window as an empty result.
    if (args.from !== undefined && args.to !== undefined) {
      if (new Date(args.from).getTime() > new Date(args.to).getTime()) {
        return toolError("`from` must be <= `to`");
      }
    }

    try {
      const events = await getProjectionStore().timeline(workspaceId, {
        from: args.from,
        to: args.to,
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
                from: args.from ?? null,
                to: args.to ?? null,
                count: events.length,
                events,
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
