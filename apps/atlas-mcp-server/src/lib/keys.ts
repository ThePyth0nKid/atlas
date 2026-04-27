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
 * V2 replaces this entire module with a TPM/HSM-sealed key handle.
 * The `--secret-hex` flag on `atlas-signer` is removed at the build
 * level in V2; this file is the only one that needs to change.
 */

import { PUBKEY_BUNDLE_SCHEMA, type PubkeyBundle } from "./types.js";

export type SignerRole = "agent" | "human" | "anchor";

export type SignerIdentity = {
  role: SignerRole;
  kid: string;
  /** 32-byte secret as 64-char hex, fed to atlas-signer --secret-hex. */
  secretHex: string;
  /** 32-byte pubkey as base64url-no-pad. Embedded in PubkeyBundle. */
  pubkeyB64Url: string;
};

/**
 * Deterministic test identities. The byte values match
 * `seed_bank_demo.rs` exactly so a fresh MCP install and the bank demo
 * produce identical bundle hashes for the same keyset.
 */
export const TEST_IDENTITIES: Record<SignerRole, SignerIdentity> = {
  agent: {
    role: "agent",
    kid: "spiffe://atlas/agent/cursor-001",
    secretHex: "aa".repeat(32),
    // base64url(no-pad) of Ed25519 pubkey for SigningKey::from_bytes(&[0xAA; 32])
    pubkeyB64Url: "5zTqbCtiV95yNV5HKqBaTEh-a0Y8Ap7TBt8vAbVja1g",
  },
  human: {
    role: "human",
    kid: "spiffe://atlas/human/sebastian.meinhardt@bankhaus-hagedorn.de",
    secretHex: "bb".repeat(32),
    // base64url(no-pad) of Ed25519 pubkey for SigningKey::from_bytes(&[0xBB; 32])
    pubkeyB64Url: "fVnFYj3UCnSqTVoyrGRdOz-V2urkwiviVHbdakhvc4I",
  },
  anchor: {
    role: "anchor",
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
