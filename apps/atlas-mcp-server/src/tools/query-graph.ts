/**
 * `atlas_query_graph` — run an AST-validated Cypher query against a
 * workspace's projection.
 *
 * Validation runs at the MCP boundary (W13-local validator). Forbidden
 * tokens (DELETE / CREATE / MERGE / SET / REMOVE / LOAD CSV / USING
 * PERIODIC COMMIT / apoc.* / CALL db.*), string-concat heuristic, and
 * length cap are all enforced BEFORE any data-layer call. Driving
 * decision: `.handoff/decisions.md` DECISION-SEC-4.
 *
 * The data layer is the `ProjectionStore` — a stub at V2-β Phase-4,
 * backed by ArcadeDB starting V2-β Phase-7 (W17). The stub throws a
 * structured "Not implemented" error; the handler translates that into
 * an MCP `isError: true` response envelope so MCP clients see a clean
 * structured error instead of a 500.
 */

import { z } from "zod";
import { DEFAULT_WORKSPACE } from "@atlas/bridge";
import { optionalWorkspaceIdSchema } from "./schema.js";
import {
  CYPHER_MAX_LENGTH,
  validateReadOnlyCypher,
} from "@atlas/cypher-validator";
import {
  getProjectionStore,
  PROJECTION_STORE_STUB_MESSAGE,
} from "./_lib/projection-store.js";
import type { ToolDefinition, ToolHandlerResult } from "./types.js";

export const queryGraphInputSchema = {
  workspace_id: optionalWorkspaceIdSchema
    .describe(`Workspace id; defaults to "${DEFAULT_WORKSPACE}". [a-zA-Z0-9_-]{1,128}.`),
  cypher: z.string().min(1).max(CYPHER_MAX_LENGTH)
    .describe(
      "Read-only Cypher query. Allowed clauses: MATCH, OPTIONAL MATCH, " +
        "WHERE, WITH, UNWIND, RETURN, ORDER BY, SKIP, LIMIT. Mutation, " +
        "LOAD CSV, apoc.*, and CALL db.* are rejected at the MCP boundary.",
    ),
  params: z.record(z.string(), z.unknown()).default({})
    .describe(
      "Bound parameters for the query. Use parameter binding rather than " +
        "string concatenation; the validator rejects single-quote+plus " +
        "patterns as a defence against caller-side injection mistakes.",
    ),
};

const inputZ = z.object(queryGraphInputSchema);

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

export const queryGraphTool: ToolDefinition<typeof queryGraphInputSchema> = {
  name: "atlas_query_graph",
  description:
    "Run an AST-validated read-only Cypher query against a workspace's " +
    "projection. Mutation, file IO, and procedure-call surfaces are rejected " +
    "at the MCP boundary (DECISION-SEC-4). Returns rows as JSON. " +
    "V2-β Phase-4 backs this with a projection-store stub; ArcadeDB-backed " +
    "execution lands in V2-β Phase 7 (W17).",
  inputSchema: queryGraphInputSchema,
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

    const validation = validateReadOnlyCypher(args.cypher);
    if (!validation.ok) {
      return toolError(`cypher rejected: ${validation.reason ?? "unknown"}`);
    }

    try {
      const rows = await getProjectionStore().runCypher(
        workspaceId,
        args.cypher,
        args.params,
      );
      return {
        content: [
          {
            type: "text" as const,
            text: JSON.stringify(
              {
                ok: true,
                workspace_id: workspaceId,
                row_count: rows.length,
                rows,
              },
              null,
              2,
            ),
          },
        ],
      };
    } catch (e: unknown) {
      // Only the documented stub sentinel is allowed to pass through to
      // MCP clients. All other errors (including future ArcadeDB driver
      // exceptions in W17) are swallowed to a generic string so internal
      // detail — connection strings, schema names, file paths, version
      // strings — never reaches the caller. See decisions.md DECISION-SEC-4.
      const isKnownStub =
        e instanceof Error && e.message === PROJECTION_STORE_STUB_MESSAGE;
      return toolError(
        isKnownStub ? e.message : "projection-store call failed",
      );
    }
  },
};
