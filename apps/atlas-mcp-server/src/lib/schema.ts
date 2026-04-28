/**
 * Runtime validation schemas for wire-format types.
 *
 * The MCP server has two trust boundaries where untyped JSON crosses
 * into typed code:
 *
 *   1. The Rust signer's stdout — we must not assume the child process
 *      always emits a well-formed AtlasEvent.
 *   2. The on-disk events.jsonl log — the file may have been rotated,
 *      truncated, hand-edited, or corrupted by a partial write.
 *
 * In both places we previously did `JSON.parse(s) as AtlasEvent`, which
 * is a TypeScript type assertion with zero runtime effect. These
 * schemas replace those casts with real runtime checks. A malformed
 * input now throws at the boundary instead of producing a structurally
 * broken object that crashes deeper in the call chain.
 */

import { z } from "zod";

const Hex64 = z.string().regex(/^[0-9a-f]{64}$/, "expected 64-char lowercase hex");
const Base64UrlNoPad = z.string().regex(/^[A-Za-z0-9_-]+$/, "expected base64url-no-pad");
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
 * AnchorEntry validation for the boundary where `atlas-signer anchor`
 * stdout crosses into typed TS code. Mirrors `AnchorEntry` in
 * `crates/atlas-trust-core/src/trace_format.rs` with the same
 * `#[serde(deny_unknown_fields)]` strictness via `.strict()`.
 *
 * If the Rust schema drifts (rename, field added/removed), this fails at
 * the MCP-server boundary with a descriptive Zod error rather than
 * writing a malformed `anchors.json`.
 */
export const AnchorKindSchema = z.enum(["dag_tip", "bundle_hash"]);

export const InclusionProofSchema = z
  .object({
    tree_size: z.number().int().nonnegative(),
    root_hash: Hex64,
    hashes: z.array(Hex64),
    checkpoint_sig: Base64UrlNoPad,
  })
  .strict();

export const AnchorEntrySchema = z
  .object({
    kind: AnchorKindSchema,
    anchored_hash: Hex64,
    log_id: Hex64,
    log_index: z.number().int().nonnegative(),
    integrated_time: z.number().int(),
    inclusion_proof: InclusionProofSchema,
  })
  .strict();

export const AnchorEntryArraySchema = z.array(AnchorEntrySchema);
