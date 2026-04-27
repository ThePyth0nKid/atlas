/**
 * `atlas_workspace_state` — read-only summary of a workspace.
 *
 * Useful for the agent to introspect "what have I already recorded?"
 * before deciding whether a follow-up event is needed. Does NOT return
 * full payloads to keep tool output bounded — for full traces use
 * `atlas_export_bundle`.
 */

import { z } from "zod";
import { computeTips, readAllEvents } from "../lib/storage.js";
import { DEFAULT_WORKSPACE } from "../lib/types.js";
import { optionalWorkspaceIdSchema } from "./schema.js";
import type { ToolDefinition } from "./types.js";

export const workspaceStateInputSchema = {
  workspace_id: optionalWorkspaceIdSchema
    .describe(`Workspace id; defaults to "${DEFAULT_WORKSPACE}". [a-zA-Z0-9_-]{1,128}.`),
  limit: z.number().int().min(1).max(200).default(20)
    .describe("Maximum number of recent events to summarise."),
};

const inputZ = z.object(workspaceStateInputSchema);

export const workspaceStateTool: ToolDefinition<typeof workspaceStateInputSchema> = {
  name: "atlas_workspace_state",
  description:
    "Return a read-only summary of a workspace: event count, current DAG tips, " +
    "and a tail of recent events (id / hash / type / kid / ts). Use this to " +
    "orient before deciding what to write next.",
  inputSchema: workspaceStateInputSchema,
  handler: async (raw) => {
    const args = inputZ.parse(raw);
    const workspaceId = args.workspace_id ?? DEFAULT_WORKSPACE;
    const events = await readAllEvents(workspaceId);
    const tips = computeTips(events);
    const tail = events.slice(-args.limit).map((ev) => ({
      event_id: ev.event_id,
      event_hash: ev.event_hash,
      ts: ev.ts,
      kid: ev.signature.kid,
      type: typeof ev.payload?.type === "string" ? ev.payload.type : "unknown",
    }));
    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(
            {
              ok: true,
              workspace_id: workspaceId,
              event_count: events.length,
              dag_tips: tips,
              recent: tail,
            },
            null,
            2,
          ),
        },
      ],
    };
  },
};
