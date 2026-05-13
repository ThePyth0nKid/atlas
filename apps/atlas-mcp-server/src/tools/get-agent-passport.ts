/**
 * `atlas_get_agent_passport` — V2-γ SCOPE STUB.
 *
 * Agent Passports are a V2-γ concept: per-agent DID + capability list +
 * attestation chain that lets a verifier confirm a runtime identity is
 * authorised to act inside a workspace. The full implementation includes
 * DID resolution, did:web/did:key support, capability-token verification,
 * and integration with the @atlas/bridge signing key roster.
 *
 * V2-β Welle 13 registers this tool name in the MCP registry as a STUB so
 * downstream tooling (Claude/Cursor/Inspector) can discover the surface
 * and warn end-users that the real implementation is V2-γ work. The
 * handler returns a stable shape every call:
 *
 *   { ok: false, agent_did, status: "stub", message: "Agent Passport tool is V2-γ scope; ..." }
 *
 * The in-payload `ok: false` mirrors the W12 HTTP route's
 * `{ ok: false, status: "stub" }` response (HTTP 501) so HTTP + MCP
 * consumers see identical "not implemented yet" semantics. The MCP
 * `isError` flag remains unset on the stub response because the handler
 * succeeded at returning the stub envelope — only `ok` signals
 * implementation status.
 *
 * DO NOT remove or rename this tool when V2-γ ships — replace the handler
 * body in-place so MCP clients with pre-discovered tool definitions
 * continue to work without re-onboarding.
 */

import { z } from "zod";
import type { ToolDefinition, ToolHandlerResult } from "./types.js";

/**
 * DID syntax — coarse check at the Zod boundary. The full RFC-3986-ish
 * grammar is large; we cap length at 512 chars (matching the W12 HTTP
 * entry point in `apps/atlas-bridge`'s passport route for echo-surface
 * consistency across HTTP + MCP) and require the `did:` scheme. V2-γ
 * will replace with a strict per-method resolver.
 */
const AGENT_DID_PATTERN = /^did:[a-z0-9]+:[A-Za-z0-9._:%-]{1,502}$/;

export const getAgentPassportInputSchema = {
  agent_did: z
    .string()
    .min(5)
    .max(512)
    .regex(AGENT_DID_PATTERN, "agent_did: must be a `did:<method>:<id>` URI")
    .describe(
      "DID identifying the agent (e.g. `did:key:z6Mki...`). " +
        "V2-γ scope: this tool is a STUB; the real handler resolves the " +
        "DID and returns the capability / attestation chain.",
    ),
};

const inputZ = z.object(getAgentPassportInputSchema);

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

export const getAgentPassportTool: ToolDefinition<typeof getAgentPassportInputSchema> = {
  name: "atlas_get_agent_passport",
  description:
    "[V2-γ STUB] Look up an agent passport (DID + capabilities + " +
    "attestation chain). This tool is registered as a stub in V2-β so " +
    "downstream MCP clients can discover the future surface. Every call " +
    "currently returns a `status: 'stub'` envelope. Full implementation " +
    "is V2-γ work; do NOT rely on this tool for production decisions.",
  inputSchema: getAgentPassportInputSchema,
  handler: async (raw) => {
    let args: z.infer<typeof inputZ>;
    try {
      args = inputZ.parse(raw);
    } catch (e: unknown) {
      return toolError(
        e instanceof Error ? `invalid input: ${e.message}` : "invalid input",
      );
    }
    // V2-γ deferral. Do NOT add real DID resolution here; the V2-γ welle
    // owns this surface end-to-end (DID resolver + capability schema +
    // attestation-chain verifier all land together to avoid an
    // incrementally-shipped half-implementation that consumers might
    // trust prematurely).
    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(
            {
              ok: false as const,
              agent_did: args.agent_did,
              status: "stub",
              message:
                "Agent Passport tool is V2-γ scope; see docs/V2-MASTER-PLAN.md §6 V2-γ Identity + Federation",
            },
            null,
            2,
          ),
        },
      ],
    };
  },
};
