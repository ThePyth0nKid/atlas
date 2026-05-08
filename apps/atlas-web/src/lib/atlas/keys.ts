/**
 * V1.19 Welle 1 — atlas-web per-tenant identity resolution.
 *
 * DUPLICATED FROM `apps/atlas-mcp-server/src/lib/keys.ts` with the
 * legacy SPIFFE V1 identities REMOVED. The web write surface only
 * supports per-tenant kids (`atlas-anchor:{workspace_id}`) — there is
 * no UI affordance to write under an `agent`, `human`, or `anchor`
 * legacy kid, and removing the static secret table from the web
 * process keeps deterministic test secrets out of the Next.js bundle.
 *
 * The MCP server retains the legacy table for backwards compatibility
 * with V1.5–V1.8 traces; web is V1.9-only.
 */

import { derivePubkeyViaSigner } from "./signer";

/**
 * Per-tenant kid prefix. MUST stay byte-identical to
 * `atlas_trust_core::per_tenant::PER_TENANT_KID_PREFIX` and to
 * `apps/atlas-mcp-server/src/lib/keys.ts::PER_TENANT_KID_PREFIX`.
 * Drift means the verifier rejects every per-tenant trace this surface
 * produces.
 */
export const PER_TENANT_KID_PREFIX = "atlas-anchor:";

export type PerTenantIdentity = {
  kid: string;
  workspaceId: string;
  pubkeyB64Url: string;
};

/** Build the canonical per-tenant kid for `workspaceId`. */
export function perTenantKidFor(workspaceId: string): string {
  return `${PER_TENANT_KID_PREFIX}${workspaceId}`;
}

/** Return the workspace_id encoded in `kid`, or `undefined` if `kid` is not per-tenant. */
export function workspaceIdFromKid(kid: string): string | undefined {
  if (!kid.startsWith(PER_TENANT_KID_PREFIX)) return undefined;
  const suffix = kid.slice(PER_TENANT_KID_PREFIX.length);
  return suffix.length === 0 ? undefined : suffix;
}

/**
 * Resolve the per-tenant identity for `workspaceId` by shelling out to
 * `atlas-signer derive-pubkey`. The secret intentionally never leaves
 * the signer process — only the kid + pubkey come back. The actual
 * `sign` call later routes through `--derive-from-workspace`, so the
 * per-tenant secret is materialised only inside the Rust signer's
 * memory.
 *
 * Implementation note: this is the public-only path
 * (`derivePubkeyViaSigner`), NOT the secret-emitting `derive-key`
 * path. Switching this back to `derive-key` would unnecessarily
 * transit secret material through Node heap on every web write — a
 * regression flagged in V1.9 security review (mirrored here so the
 * same property holds in the web process).
 */
export async function resolvePerTenantIdentity(
  workspaceId: string,
): Promise<PerTenantIdentity> {
  const expectedKid = perTenantKidFor(workspaceId);
  const derived = await derivePubkeyViaSigner(workspaceId);
  if (derived.kid !== expectedKid) {
    throw new Error(
      `derive-pubkey kid mismatch: expected ${expectedKid}, signer returned ${derived.kid}`,
    );
  }
  return {
    kid: derived.kid,
    workspaceId,
    pubkeyB64Url: derived.pubkey_b64url,
  };
}
