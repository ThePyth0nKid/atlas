/**
 * V1.19 Welle 1 — atlas-web bridge surface.
 *
 * DUPLICATED FROM `apps/atlas-mcp-server/src/lib/types.ts`.
 *
 * The MCP server is the historical source of truth for these types.
 * We duplicate the *minimal subset* that atlas-web's write surface
 * needs (no Anchor*, AtlasTrace, or chain types) so the web app does
 * not pull in MCP-only dependencies.
 *
 * V1.19 Welle 2 will extract the canonical bridge into
 * `packages/atlas-bridge/` and both consumers will depend on it. Until
 * then: any change to AtlasEvent / EventSignature MUST be mirrored in
 * `apps/atlas-mcp-server/src/lib/types.ts` and verified by the Rust
 * verifier round-trip — the trust property is preserved by the Rust
 * signer + verifier, so drift here surfaces as a verification failure
 * rather than a silent semantic mismatch.
 */

export type EventSignature = {
  alg: "EdDSA";
  kid: string;
  sig: string;
};

export type AtlasEvent = {
  event_id: string;
  event_hash: string;
  parent_hashes: string[];
  payload: Record<string, unknown>;
  signature: EventSignature;
  ts: string;
};

export const DEFAULT_WORKSPACE = "ws-mcp-default" as const;
