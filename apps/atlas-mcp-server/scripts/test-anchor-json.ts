#!/usr/bin/env tsx
/**
 * Unit tests for `lib/anchor-json.ts`.
 *
 * Pins the V1.8 trust-property closure: a Sigstore Rekor v1 anchor
 * entry whose `tree_id` exceeds `Number.MAX_SAFE_INTEGER` must
 * round-trip through `parseAnchorJson` → `AnchorEntrySchema` →
 * `stringifyAnchorJson` with the original digit string preserved
 * byte-identical. Without this guarantee, the chain head an offline
 * auditor recomputes would diverge from the head the issuer emitted.
 *
 * Designed to run as `pnpm test:anchor-json` (or directly with `tsx`)
 * with no extra runner; assertion failures call `process.exit(1)` so
 * CI integration is `npm run test:anchor-json` + non-zero exit.
 */

import {
  isLosslessNumber,
  parseAnchorJson,
  stringifyAnchorJson,
} from "../src/lib/anchor-json.js";
import {
  AnchorEntrySchema,
  AnchorEntryArraySchema,
} from "../src/lib/schema.js";

let failures = 0;

function check(name: string, predicate: boolean, detail?: string): void {
  if (predicate) {
    process.stdout.write(`  ok  ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(`  FAIL ${name}${detail !== undefined ? ` — ${detail}` : ""}\n`);
  }
}

function expect<T>(label: string, fn: () => T): T {
  try {
    return fn();
  } catch (e) {
    failures += 1;
    process.stdout.write(
      `  FAIL ${label} — threw ${(e as Error).message ?? String(e)}\n`,
    );
    throw e;
  }
}

const SIGSTORE_TREE_ID = "1193050959916656506";

// ─── Test 1: lossless parser preserves oversized integers ──────────────
{
  const input = `{ "tree_id": ${SIGSTORE_TREE_ID} }`;
  const parsed = expect("parse Sigstore tree_id", () =>
    parseAnchorJson(input) as { tree_id: unknown },
  );
  check(
    "tree_id parsed as LosslessNumber",
    isLosslessNumber(parsed.tree_id),
    `got: ${typeof parsed.tree_id}`,
  );
  if (isLosslessNumber(parsed.tree_id)) {
    check(
      "LosslessNumber.value preserves digits",
      parsed.tree_id.value === SIGSTORE_TREE_ID,
      `got: ${parsed.tree_id.value}`,
    );
  }
}

// ─── Test 2: safe-range integers stay native `number` ──────────────────
{
  const input = `{ "log_index": 12345, "tree_size": 100 }`;
  const parsed = expect("parse safe-range integers", () =>
    parseAnchorJson(input) as { log_index: unknown; tree_size: unknown },
  );
  check(
    "safe log_index stays number",
    typeof parsed.log_index === "number" && parsed.log_index === 12345,
  );
  check(
    "safe tree_size stays number",
    typeof parsed.tree_size === "number" && parsed.tree_size === 100,
  );
}

// ─── Test 3: stringify round-trip preserves exact bytes ────────────────
{
  // Build the input JSON as a literal string so `tree_id` is unambiguously
  // an unquoted numeric token — exactly what `atlas-signer anchor
  // --rekor-url …` writes to stdout. Using `JSON.stringify` with a string
  // `tree_id` and patching the quotes off afterwards would couple the test
  // to property-emit order; a literal string sidesteps that entirely.
  const inputJson =
    `{"kind":"bundle_hash",` +
    `"anchored_hash":"${"1".repeat(64)}",` +
    `"log_id":"${"2".repeat(64)}",` +
    `"log_index":800000000,` +
    `"integrated_time":1745500000,` +
    `"inclusion_proof":{` +
    `"tree_size":800000001,` +
    `"root_hash":"${"3".repeat(64)}",` +
    `"hashes":["${"4".repeat(64)}"],` +
    `"checkpoint_sig":"AAAA"` +
    `},` +
    `"entry_body_b64":"eyJraW5kIjoiaGFzaGVkcmVrb3JkIn0=",` +
    `"tree_id":${SIGSTORE_TREE_ID}}`;
  const parsed = expect("parse Sigstore-shaped entry", () =>
    parseAnchorJson(inputJson),
  );
  const validated = AnchorEntrySchema.safeParse(parsed);
  check(
    "Zod schema accepts LosslessNumber tree_id",
    validated.success,
    !validated.success ? validated.error.message : undefined,
  );
  const restringified = expect("stringify validated entry", () =>
    stringifyAnchorJson(validated.success ? validated.data : parsed),
  );
  check(
    "round-trip preserves Sigstore tree_id digits",
    restringified.includes(`"tree_id":${SIGSTORE_TREE_ID}`),
    `output: ${restringified.slice(0, 200)}…`,
  );
}

// ─── Test 4: float / scientific-notation tree_id rejected ──────────────
// Defends against a signer-side drift that would emit `tree_id: 1.193e18`
// instead of an integer literal. The schema must reject this rather than
// silently accept a floating-point value as a tree_id.
{
  const input = `{ "kind":"bundle_hash","anchored_hash":"${"a".repeat(64)}","log_id":"${"b".repeat(64)}","log_index":1,"integrated_time":1,"inclusion_proof":{"tree_size":1,"root_hash":"${"c".repeat(64)}","hashes":[],"checkpoint_sig":"AAAA"},"tree_id":1.193e18 }`;
  const parsed = expect("parse float tree_id", () => parseAnchorJson(input));
  const validated = AnchorEntrySchema.safeParse(parsed);
  check(
    "Zod schema rejects non-integer tree_id",
    !validated.success,
    validated.success ? "schema unexpectedly accepted scientific notation" : undefined,
  );
}

// ─── Test 5: array round-trip (the actual stdout shape) ────────────────
{
  const arrayJson = `[
    { "kind":"bundle_hash","anchored_hash":"${"a".repeat(64)}","log_id":"${"b".repeat(64)}","log_index":1,"integrated_time":1,"inclusion_proof":{"tree_size":1,"root_hash":"${"c".repeat(64)}","hashes":[],"checkpoint_sig":"AAAA"},"entry_body_b64":"AAAA","tree_id":${SIGSTORE_TREE_ID} }
  ]`;
  const parsed = expect("parse Sigstore array", () => parseAnchorJson(arrayJson));
  const validated = AnchorEntryArraySchema.safeParse(parsed);
  check(
    "AnchorEntryArraySchema accepts lossless tree_id in array",
    validated.success,
    !validated.success ? validated.error.message : undefined,
  );
  if (validated.success) {
    const restringified = stringifyAnchorJson(validated.data);
    check(
      "array round-trip preserves tree_id digits",
      restringified.includes(`"tree_id":${SIGSTORE_TREE_ID}`),
      `output excerpt: ${restringified.slice(0, 200)}…`,
    );
  }
}

if (failures > 0) {
  process.stderr.write(`\n${failures} test(s) failed.\n`);
  process.exit(1);
}
process.stdout.write(`\nall lossless-json round-trip tests passed\n`);
