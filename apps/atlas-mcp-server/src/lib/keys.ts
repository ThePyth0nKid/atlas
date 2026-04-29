/**
 * V1 development keys.
 *
 * These are the SAME deterministic test keys used by the bank-persona
 * demo in `crates/atlas-signer/examples/seed_bank_demo.rs`. Reusing them
 * means a developer running the MCP smoke locally produces a
 * pubkey-bundle byte-identical to the bank demo's bundle, which keeps
 * the developer story coherent: the same kids, the same pubkeys, the
 * same flows.
 *
 * V1.9 adds *per-tenant* signing identities derived from a master seed
 * via HKDF-SHA256. The legacy three-kid block stays for backwards
 * compatibility (V1.5–V1.8 traces continue to verify); per-tenant kids
 * `atlas-anchor:{workspace_id}` are added on top via
 * `buildBundleForWorkspace`. The kid prefix here MUST match the Rust
 * constant `atlas_trust_core::per_tenant::PER_TENANT_KID_PREFIX` —
 * drift means the verifier rejects every per-tenant trace.
 *
 * V2 replaces this entire module with a TPM/HSM-sealed key handle.
 * The `--secret-hex` flag on `atlas-signer` is removed at the build
 * level in V2; this file is the only one that needs to change.
 */

import { deriveKeyViaSigner, derivePubkeyViaSigner } from "./signer.js";
import { PUBKEY_BUNDLE_SCHEMA, type PubkeyBundle } from "./types.js";

/**
 * V1 legacy role taxonomy. Used as the key set of `TEST_IDENTITIES`.
 * Per-tenant identities use the broader `SignerRole` union below and
 * never appear in the static map — they are derived per workspace
 * from the master seed.
 */
export type LegacySignerRole = "agent" | "human" | "anchor";

export type SignerRole = LegacySignerRole | "per-tenant";

/**
 * V1.9: signing identity. The shape is a discriminated union on
 * `secretSource`:
 *
 *   * `"hex"`   — legacy SPIFFE kids whose 32-byte hex secret lives in
 *                 the static `TEST_IDENTITIES` map and is piped into
 *                 `atlas-signer sign --secret-stdin`. The secret
 *                 transits Node memory (this is the V1.5–V1.8 path).
 *
 *   * `"derive-from-workspace"` — V1.9 per-tenant kids. The signer
 *                 derives the secret internally via HKDF and signs
 *                 without ever emitting it. The MCP process never
 *                 holds the per-tenant secret. This is the strictly
 *                 safer path and is the default for per-tenant kids.
 *
 * `pubkeyB64Url` is supplied in both variants because the TS side
 * needs it for bundle assembly. For the per-tenant variant it comes
 * from `derive-pubkey` — that subcommand by design omits the secret.
 */
export type SignerIdentity =
  | {
      role: LegacySignerRole;
      secretSource: "hex";
      kid: string;
      /** 32-byte secret as 64-char hex, fed to atlas-signer --secret-stdin. */
      secretHex: string;
      /** 32-byte pubkey as base64url-no-pad. Embedded in PubkeyBundle. */
      pubkeyB64Url: string;
    }
  | {
      role: "per-tenant";
      secretSource: "derive-from-workspace";
      kid: string;
      /**
       * Workspace identifier the signer should derive the per-tenant
       * secret from internally. Passed as
       * `--derive-from-workspace <workspaceId>` to `atlas-signer sign`.
       */
      workspaceId: string;
      pubkeyB64Url: string;
    };

/**
 * Per-tenant kid prefix. MUST stay byte-identical to
 * `atlas_trust_core::per_tenant::PER_TENANT_KID_PREFIX` — the verifier
 * recomputes the expected kid from `trace.workspace_id` and a literal
 * string. Drift is silently caught by the V1.9 adversary suite, which
 * fails if the verifier expects a different prefix than the issuer
 * emits.
 */
export const PER_TENANT_KID_PREFIX = "atlas-anchor:";

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
 * Deterministic test identities. The byte values match
 * `seed_bank_demo.rs` exactly so a fresh MCP install and the bank demo
 * produce identical bundle hashes for the same keyset.
 */
export const TEST_IDENTITIES: Record<LegacySignerRole, SignerIdentity> = {
  agent: {
    role: "agent",
    secretSource: "hex",
    kid: "spiffe://atlas/agent/cursor-001",
    secretHex: "aa".repeat(32),
    // base64url(no-pad) of Ed25519 pubkey for SigningKey::from_bytes(&[0xAA; 32])
    pubkeyB64Url: "5zTqbCtiV95yNV5HKqBaTEh-a0Y8Ap7TBt8vAbVja1g",
  },
  human: {
    role: "human",
    secretSource: "hex",
    kid: "spiffe://atlas/human/sebastian.meinhardt@bankhaus-hagedorn.de",
    secretHex: "bb".repeat(32),
    // base64url(no-pad) of Ed25519 pubkey for SigningKey::from_bytes(&[0xBB; 32])
    pubkeyB64Url: "fVnFYj3UCnSqTVoyrGRdOz-V2urkwiviVHbdakhvc4I",
  },
  anchor: {
    role: "anchor",
    secretSource: "hex",
    kid: "spiffe://atlas/system/anchor-worker",
    secretHex: "cc".repeat(32),
    // base64url(no-pad) of Ed25519 pubkey for SigningKey::from_bytes(&[0xCC; 32])
    pubkeyB64Url: "ylfu0w5KcnTvTGSPVvWPiAsg0solcl2eXBPIPAjAmus",
  },
};

/**
 * Build the V1 dev pubkey-bundle. The `generated_at` is fixed (same as
 * the bank demo) so the bundle hash is reproducible across machines and
 * matches the bank-demo bundle byte-for-byte.
 *
 * If you change the kids, the `generated_at`, or any pubkey, the bundle
 * hash changes, and any trace bundle previously emitted against the old
 * hash will (correctly) fail to verify against the new one.
 */
export function buildDevBundle(): PubkeyBundle {
  const keys: Record<string, string> = {};
  for (const id of Object.values(TEST_IDENTITIES)) {
    keys[id.kid] = id.pubkeyB64Url;
  }
  return {
    schema: PUBKEY_BUNDLE_SCHEMA,
    generated_at: "2026-01-01T00:00:00Z",
    keys,
  };
}

export function identityForKid(kid: string): SignerIdentity | undefined {
  for (const id of Object.values(TEST_IDENTITIES)) {
    if (id.kid === kid) return id;
  }
  return undefined;
}

/**
 * Resolve the signing identity for `kid`. Legacy SPIFFE kids hit the
 * static `TEST_IDENTITIES` table and return a `secretSource: "hex"`
 * identity; per-tenant kids of shape `atlas-anchor:{workspace}` return
 * a `secretSource: "derive-from-workspace"` identity.
 *
 * Crucially, the per-tenant branch calls `derivePubkeyViaSigner` (NOT
 * `deriveKeyViaSigner`) — the public path. The TS process never holds
 * the per-tenant secret. The actual signing call routes through
 * `atlas-signer sign --derive-from-workspace`, which derives the
 * secret inside the signer process and never emits it.
 *
 * The async signature is the price of HKDF-on-demand. The cost is one
 * subprocess invocation per resolution (~10 ms warm) — acceptable at
 * MCP write rates and dwarfed by the existing `signEvent` spawn that
 * follows. Callers that need a tight loop should resolve once and
 * reuse the returned `SignerIdentity` rather than re-resolving per
 * event.
 */
export async function resolveIdentityForKid(kid: string): Promise<SignerIdentity | undefined> {
  const legacy = identityForKid(kid);
  if (legacy !== undefined) return legacy;

  const workspaceId = workspaceIdFromKid(kid);
  if (workspaceId === undefined) return undefined;

  const derived = await derivePubkeyViaSigner(workspaceId);
  // Defence-in-depth: the signer should always emit a kid that matches
  // the workspace it was asked about. If somehow it does not, surfacing
  // a clear error here beats writing a malformed event.
  if (derived.kid !== kid) {
    throw new Error(
      `derive-pubkey kid mismatch: requested ${kid}, signer returned ${derived.kid}`,
    );
  }
  return {
    role: "per-tenant",
    secretSource: "derive-from-workspace",
    kid: derived.kid,
    workspaceId,
    pubkeyB64Url: derived.pubkey_b64url,
  };
}

/**
 * Build a pubkey bundle for `workspaceId` that contains both the legacy
 * three-kid block (for backwards compatibility) and the per-tenant kid
 * derived from `atlas-signer`.
 *
 * V1.5–V1.8 traces continue to verify against the legacy kids. V1.9
 * traces signed under the per-tenant kid verify against the per-tenant
 * entry. Lenient mode accepts both; strict mode
 * (`require_per_tenant_keys`) rejects events whose kid is not the
 * per-tenant one.
 *
 * Implementation note: calls `derivePubkeyViaSigner`, NOT
 * `deriveKeyViaSigner`. Bundle assembly only needs the public key, so
 * routing through the public-only path keeps the per-tenant secret
 * entirely inside the signer process. Switching this back to
 * `deriveKeyViaSigner` would unnecessarily transit secret material
 * through Node heap on every export — a regression flagged in V1.9
 * security review.
 */
export async function buildBundleForWorkspace(workspaceId: string): Promise<PubkeyBundle> {
  const base = buildDevBundle();
  const derived = await derivePubkeyViaSigner(workspaceId);
  return {
    ...base,
    keys: {
      ...base.keys,
      [derived.kid]: derived.pubkey_b64url,
    },
  };
}
