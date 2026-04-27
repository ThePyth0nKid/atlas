/**
 * `atlas_write_node` — record the creation of a graph node.
 *
 * Maps the agent's structured `node` payload into a canonical
 * `node.create` AtlasEvent and signs it with the calling identity. The
 * resulting event becomes part of the workspace's append-only DAG and
 * future events will reference it transitively via parent-hashes.
 *
 * Why a dedicated tool rather than a generic "write_event"? Because each
 * payload `type` corresponds to a distinct compliance event class. EU AI
 * Act Article 12 records-keeping wants explicit semantics, not opaque
 * blobs. Keeping the tool boundary narrow forces the caller to be honest
 * about *what kind* of event it is recording.
 */

import { z } from "zod";
import { writeSignedEvent } from "../lib/event.js";
import { DEFAULT_WORKSPACE } from "../lib/types.js";
import { optionalWorkspaceIdSchema } from "./schema.js";
import type { ToolDefinition } from "./types.js";

const NodeKind = z.enum(["dataset", "model", "inference", "document", "other"]);

export const writeNodeInputSchema = {
  workspace_id: optionalWorkspaceIdSchema
    .describe(`Workspace id; defaults to "${DEFAULT_WORKSPACE}". [a-zA-Z0-9_-]{1,128}.`),
  kid: z.string().min(1)
    .describe("Signer key-id (SPIFFE-style). Must exist in the dev pubkey-bundle."),
  kind: NodeKind.describe("Node kind — drives compliance classification."),
  id: z.string().min(1).describe("Caller-chosen stable id for the node."),
  attributes: z.record(z.string(), z.unknown()).default({})
    .describe("Free-form node attributes. No floats — use basis-points (×10000) for fractions."),
};

const inputZ = z.object(writeNodeInputSchema);

export const writeNodeTool: ToolDefinition<typeof writeNodeInputSchema> = {
  name: "atlas_write_node",
  description:
    "Record creation of a graph node (dataset, model, inference, document) " +
    "as a signed AtlasEvent. Returns the event-id and event-hash. " +
    "Use this BEFORE downstream actions reference the node, so the DAG link is auditable.",
  inputSchema: writeNodeInputSchema,
  handler: async (raw) => {
    const args = inputZ.parse(raw);
    const workspaceId = args.workspace_id ?? DEFAULT_WORKSPACE;
    // Spread attributes FIRST so the validated kind/id win on key collision —
    // never let a caller-supplied `attributes` object overwrite the schema-checked
    // node identity. Silent override of these fields would corrupt the signed payload.
    const payload = {
      type: "node.create",
      node: {
        ...args.attributes,
        kind: args.kind,
        id: args.id,
      },
    };
    const { event, parentsUsed } = await writeSignedEvent({
      workspaceId,
      kid: args.kid,
      payload,
    });
    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(
            {
              ok: true,
              workspace_id: workspaceId,
              event_id: event.event_id,
              event_hash: event.event_hash,
              parents: parentsUsed,
            },
            null,
            2,
          ),
        },
      ],
    };
  },
};
