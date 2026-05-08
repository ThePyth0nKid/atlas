/**
 * V1.19 Welle 1 — atlas-web Zod schemas (write-path subset).
 *
 * DUPLICATED FROM `apps/atlas-mcp-server/src/lib/schema.ts`. The web
 * write surface only validates AtlasEvent (signer-stdout boundary)
 * and DerivedPubkey (derive-pubkey-stdout boundary). Anchor / chain
 * schemas live only in the MCP server because atlas-web does not
 * issue anchors.
 *
 * The two trust boundaries this file covers are:
 *   1. The Rust signer's stdout (signer.ts) — the child process must
 *      emit a structurally well-formed AtlasEvent or we refuse to
 *      append.
 *   2. The on-disk events.jsonl log (storage.ts) — the file may have
 *      been rotated, truncated, hand-edited, or corrupted by a
 *      partial write; runtime schema check catches all of those at
 *      the read boundary instead of crashing deeper.
 */

import { z } from "zod";

const Hex64 = z.string().regex(/^[0-9a-f]{64}$/, "expected 64-char lowercase hex");
const Base64UrlNoPad = z.string().regex(/^[A-Za-z0-9_-]+$/, "expected base64url-no-pad");

/**
 * Ed25519 32-byte public key in base64url-no-pad: exactly 43 chars.
 *
 * Pinning to 43 chars at the trust boundary catches a hypothetical
 * signer regression that emits a truncated pubkey (e.g. 42 chars
 * from a buggy trim) before the bad pubkey is ever written to a
 * bundle. 32 bytes encoded in base64url-no-pad is exactly
 * `ceil(32 * 4 / 3) = 43` characters.
 */
const Base64UrlEd25519Pubkey = z
  .string()
  .length(43, "expected exactly 43 chars (Ed25519 pubkey, base64url-no-pad)")
  .regex(/^[A-Za-z0-9_-]+$/, "expected base64url-no-pad alphabet");

const IsoTimestamp = z.string().regex(
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/,
  "expected ISO-8601 UTC timestamp",
);

export const EventSignatureSchema = z.object({
  alg: z.literal("EdDSA"),
  kid: z.string().min(1),
  sig: Base64UrlNoPad,
});

export const AtlasEventSchema = z.object({
  event_id: z.string().min(1),
  event_hash: Hex64,
  parent_hashes: z.array(Hex64),
  payload: z.record(z.string(), z.unknown()),
  signature: EventSignatureSchema,
  ts: IsoTimestamp,
});

export type AtlasEventValidated = z.infer<typeof AtlasEventSchema>;

/**
 * V1.9 per-tenant kid prefix — MUST stay byte-identical to
 * `atlas_trust_core::per_tenant::PER_TENANT_KID_PREFIX` and to
 * `apps/atlas-mcp-server/src/lib/keys.ts::PER_TENANT_KID_PREFIX`.
 */
export const PerTenantKidSchema = z
  .string()
  .min("atlas-anchor:".length + 1, "expected an `atlas-anchor:{workspace}` kid")
  .startsWith("atlas-anchor:", "kid must start with `atlas-anchor:`");

/**
 * Trust-boundary check on `atlas-signer derive-pubkey` stdout. The
 * secret never leaves the signer process on this path; the wire
 * format omits it by design.
 */
export const DerivedPubkeySchema = z
  .object({
    kid: PerTenantKidSchema,
    pubkey_b64url: Base64UrlEd25519Pubkey,
  })
  .strict();
