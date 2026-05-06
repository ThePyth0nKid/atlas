/**
 * Precision-preserving JSON parsing/serialization for anchor entries.
 *
 * V1.6 introduced Sigstore Rekor v1 anchors whose `tree_id` is a 64-bit
 * Trillian tree identifier — currently `1_193_050_959_916_656_506` for
 * the active production shard. That value exceeds
 * `Number.MAX_SAFE_INTEGER` (~2^53), so a `JSON.parse` round-trip in
 * Node silently rewrites the low digits and the entry no longer
 * verifies under the pinned shard roster (see
 * `crates/atlas-trust-core/src/anchor.rs::SIGSTORE_REKOR_V1.tree_id_roster`).
 *
 * V1.7 sidestepped the problem by gating anchor-chain extension on the
 * mock-issuer path. V1.8 closes the gap: the MCP server uses
 * `lossless-json` to parse entries with full precision, so Sigstore-path
 * anchors can extend `anchor-chain.jsonl` and round-trip through
 * `trace.json` without losing tree_id digits.
 *
 * Strategy: a custom number parser keeps the parser's output type the
 * same as today (`number`) for every value that survives the safe-
 * integer range, and only wraps oversized integers in `LosslessNumber`.
 * The Zod schemas for the small numeric fields (`log_index`,
 * `tree_size`, `integrated_time`, `batch_index`) keep validating as
 * `z.number().int()` without modification; the only field that can
 * legitimately exceed the safe range is `tree_id`, and its schema
 * accepts the `LosslessNumber` shape explicitly. This minimises churn
 * across the codebase while making the trust property work for the
 * Sigstore path.
 */

import {
  isSafeNumber,
  LosslessNumber,
  parse,
  stringify,
} from "lossless-json";

/**
 * Parse JSON text with `tree_id`-class precision preserved.
 *
 * Numbers that round-trip safely through JavaScript `number` are
 * decoded as native `number`, matching the legacy `JSON.parse` shape.
 * Numbers that would lose precision (a Sigstore Rekor v1 `tree_id`
 * exceeds `Number.MAX_SAFE_INTEGER`) are wrapped in `LosslessNumber`,
 * which preserves the exact decimal string so a downstream consumer
 * (Rust verifier or another `stringifyAnchorJson` call) can re-emit
 * the original digits.
 *
 * Throws `SyntaxError` on malformed JSON, identical to `JSON.parse`,
 * so callers can keep their existing try/catch shape.
 */
export function parseAnchorJson(text: string): unknown {
  return parse(text, undefined, parseSafeNumberOrLossless);
}

/**
 * Serialize a value to JSON text with `LosslessNumber` instances
 * emitted as their original digit string (not as the wrapper object).
 *
 * `lossless-json`'s `stringify` handles native numbers, strings,
 * booleans, `null`, arrays, plain objects, and `LosslessNumber`
 * identically to `JSON.stringify` for the first five categories, so
 * traces and bundles that contain only safe-range numbers produce
 * byte-identical output to the legacy path.
 *
 * The `indent` parameter mirrors `JSON.stringify`'s third argument
 * (spaces: number, or string). Returns the empty string when the
 * value is `undefined`, matching the pre-existing call sites that
 * never pass `undefined`.
 */
export function stringifyAnchorJson(
  value: unknown,
  indent?: string | number,
): string {
  return stringify(value, undefined, indent) ?? "";
}

/**
 * Custom `NumberParser` for `lossless-json::parse`.
 *
 * Decision tree:
 *   * Pure integer literal in safe range → native `number`. Existing
 *     `z.number().int()` schemas keep validating without modification.
 *   * Anything else (oversized integer, fractional, scientific
 *     notation) → `LosslessNumber(raw)` preserving the exact source
 *     string.
 *
 * The float/scientific case is the load-bearing one: `1.193e18`
 * happens to be an integer-valued double, so a naive `isSafeNumber`-
 * only gate would let it pass `z.number().int()`. Forcing every
 * non-integer literal through `LosslessNumber` makes the union
 * `z.number().int() | LosslessIntegerSchema` reject it (the regex on
 * `LosslessNumber.value` matches integer literals only). This catches
 * a hypothetical signer drift that emits scientific notation instead
 * of digit strings — a silent precision-loss bug otherwise.
 *
 * The pattern is non-negative-only: `tree_id`, `log_index`,
 * `tree_size`, and `batch_index` are all non-negative in the Rust
 * trust-core types, so a leading minus at this boundary is itself
 * signer drift. Any negative literal therefore takes the
 * `LosslessNumber` branch where the schema's regex rejects it with a
 * descriptive Zod error instead of silently allowing a `Number(-1)`
 * past `z.number().int()`.
 */
export const INTEGER_LITERAL_REGEX = /^(?:0|[1-9]\d*)$/;

function parseSafeNumberOrLossless(raw: string): number | LosslessNumber {
  if (INTEGER_LITERAL_REGEX.test(raw) && isSafeNumber(raw)) {
    return Number(raw);
  }
  return new LosslessNumber(raw);
}

export { isLosslessNumber, LosslessNumber } from "lossless-json";
