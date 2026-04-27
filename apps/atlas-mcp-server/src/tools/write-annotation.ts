/**
 * `atlas_write_annotation` — record a human-in-the-loop assertion about
 * an existing graph node.
 *
 * The annotation event is signed with a *human* kid (not the agent's
 * key). That separation is the load-bearing trust property: a verifier
 * inspecting a trace can prove which decisions were made by an agent
 * versus which were made by a named, accountable human signer. This is
 * what GDPR Article 22 ("right to a human in the loop") and EU AI Act
 * Article 14 ("human oversight") require *evidence* of, not just process
 * documentation.
 */

import { z } from "zod";
import { writeSignedEvent } from "../lib/event.js";
import { DEFAULT_WORKSPACE } from "../lib/types.js";
import { optionalWorkspaceIdSchema } from "./schema.js";
import type { ToolDefinition } from "./types.js";

export const writeAnnotationInputSchema = {
  workspace_id: optionalWorkspaceIdSchema
    .describe(`Workspace id; defaults to "${DEFAULT_WORKSPACE}". [a-zA-Z0-9_-]{1,128}.`),
  kid: z.string().min(1)
    .describe("Signer key-id of the asserting human. MUST be a `human/*` SPIFFE-id."),
  subject: z.string().min(1)
    .describe("Node id this annotation refers to (e.g. `CreditScoreV3.ckpt`)."),
  predicate: z.string().min(1)
    .describe("Verb of the assertion (e.g. `verified_by_human`, `approved_for_release`)."),
  object: z.record(z.string(), z.unknown()).default({})
    .describe("Object payload describing the assertion."),
};

const inputZ = z.object(writeAnnotationInputSchema);

export const writeAnnotationTool: ToolDefinition<typeof writeAnnotationInputSchema> = {
  name: "atlas_write_annotation",
  description:
    "Record a human-signed annotation against an existing graph node. " +
    "Use this for human approvals, verifications, or sign-offs — anything " +
    "that needs to be attributable to a specific accountable person.",
  inputSchema: writeAnnotationInputSchema,
  handler: async (raw) => {
    const args = inputZ.parse(raw);
    const workspaceId = args.workspace_id ?? DEFAULT_WORKSPACE;
    // V1 does NOT enforce kid role; V2 will pull role from the cosigned
    // identity bundle and refuse non-human kids here.
    const payload = {
      type: "annotation.add",
      subject: args.subject,
      predicate: args.predicate,
      object: args.object,
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
              note:
                "Annotation signed and persisted. The verifier will refuse the " +
                "trace if this signature, kid, or hash-chain link breaks.",
            },
            null,
            2,
          ),
        },
      ],
    };
  },
};
