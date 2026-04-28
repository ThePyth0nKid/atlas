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
import {
  INTEGER_LITERAL_REGEX,
  isLosslessNumber,
  type LosslessNumber,
} from "./anchor-json.js";

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
 *
 * Sigstore-format-only fields (V1.6):
 *   - `entry_body_b64`: standard base64 (RFC 4648 §4 with padding) of
 *     the canonical Rekor entry body. Optional and absent for the
 *     atlas-mock format. The base64 alphabet differs from
 *     `Base64UrlNoPad`, so we accept the standard alphabet here.
 *   - `tree_id`: Trillian tree-ID. Optional. Sigstore production
 *     tree-IDs (e.g. `1_193_050_959_916_656_506`) exceed
 *     `Number.MAX_SAFE_INTEGER` (~2^53), so V1.8 routes anchor-entry
 *     JSON through `parseAnchorJson` (lossless-json) and accepts
 *     either a native `number` (mock or any safe-range int) or a
 *     `LosslessNumber` (the wrapper produced by `parseAnchorJson` for
 *     oversized integers). The Zod check rejects both `NaN` and any
 *     `LosslessNumber.value` that is not a clean signed integer
 *     literal, so a malformed wrapper still fails at the boundary.
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

// RFC 4648 §4 strict: 4-char groups optionally followed by a 2-char + "==" tail
// or a 3-char + "=" tail. Rejects empty strings (Sigstore entry bodies are
// always non-empty), missing/wrong padding, and unpadded variants — those are
// `Base64UrlNoPad` territory. A loose `[A-Za-z0-9+/]+={0,2}` would accept
// `AAAA=` (wrong-pad) and silently fail to detect signer-side drift toward
// the no-pad alphabet.
const Base64Standard = z.string().regex(
  /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=|[A-Za-z0-9+/]{4})$/,
  "expected RFC 4648 §4 standard base64 with correct padding",
);

/**
 * Trillian `tree_id` magnitude ceiling. The Rust verifier deserializes
 * `tree_id` as `i64`, whose decimal width is at most 19 digits. A
 * crafted entry carrying e.g. a 500-digit literal would clear the
 * integer-literal regex but later fail Rust deserialization with a
 * cryptic overflow error. Bounding the digit count at the boundary
 * surfaces that drift here, with a useful Zod error.
 */
const TREE_ID_MAX_DIGITS = 19;

const LosslessIntegerSchema = z.custom<LosslessNumber>(
  (v) => {
    if (!isLosslessNumber(v)) return false;
    const raw = (v as LosslessNumber).value;
    return INTEGER_LITERAL_REGEX.test(raw) && raw.length <= TREE_ID_MAX_DIGITS;
  },
  { message: "expected a lossless integer literal within i64 magnitude" },
);

/**
 * `tree_id` accepts either a native `number` (mock entries omit the
 * field; safe-range Sigstore values stay native) or a `LosslessNumber`
 * (oversized Sigstore production `tree_id`s preserved by
 * `parseAnchorJson`). See `lib/anchor-json.ts` for the parser
 * decision boundary.
 */
const TreeIdSchema = z.union([z.number().int(), LosslessIntegerSchema]);

export const AnchorEntrySchema = z
  .object({
    kind: AnchorKindSchema,
    anchored_hash: Hex64,
    log_id: Hex64,
    log_index: z.number().int().nonnegative(),
    integrated_time: z.number().int(),
    inclusion_proof: InclusionProofSchema,
    entry_body_b64: Base64Standard.optional(),
    tree_id: TreeIdSchema.optional(),
  })
  .strict();

export const AnchorEntryArraySchema = z.array(AnchorEntrySchema);

/**
 * V1.7 anchor-chain row. Mirrors `AnchorBatch` in
 * `crates/atlas-trust-core/src/trace_format.rs`. Each batch records
 * one `atlas_anchor_bundle` invocation: a sequential `batch_index`,
 * the issuer's `integrated_time`, the issued `entries`, and the
 * `previous_head` linking back to the previous batch.
 *
 * `previous_head` is 64-char lowercase hex (genesis: 32 zero bytes
 * hex-encoded); the verifier asserts
 * `history[i].previous_head == chain_head_for(history[i-1])`.
 */
export const AnchorBatchSchema = z
  .object({
    batch_index: z.number().int().nonnegative(),
    integrated_time: z.number().int(),
    entries: z.array(AnchorEntrySchema),
    previous_head: Hex64,
  })
  .strict();

/**
 * V1.7 anchor-chain wire-format. Mirrors `AnchorChain` in
 * `crates/atlas-trust-core/src/trace_format.rs`.
 *
 * `head` is `chain_head_for(history.last())` — the verifier
 * recomputes it locally and only treats this field as a fast-fail
 * convenience, never as a trust shortcut. Empty `history` is
 * structurally invalid; the export-side signer subcommand
 * (`chain-export`) rejects empty input before producing this shape,
 * so a non-empty array is enforced here too.
 */
export const AnchorChainSchema = z
  .object({
    history: z.array(AnchorBatchSchema).min(1, "AnchorChain.history must be non-empty"),
    head: Hex64,
  })
  .strict();
