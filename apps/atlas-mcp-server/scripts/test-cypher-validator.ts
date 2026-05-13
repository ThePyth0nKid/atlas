#!/usr/bin/env tsx
/**
 * Unit tests for `tools/_lib/cypher-validator.ts` — W13's INLINE Cypher
 * AST validator. Independent of W12's parallel validator: the rule-of-three
 * pattern means W15 (later welle) consolidates W12 + W13's implementations
 * into a single shared module AFTER both ship side-by-side.
 *
 * Coverage targets (matches `.handoff/decisions.md` DECISION-SEC-4):
 *   - Reject every mutation keyword (DELETE / CREATE / MERGE / SET / REMOVE)
 *   - Reject LOAD CSV and USING PERIODIC COMMIT (DoS surface)
 *   - Reject `apoc.*` and `CALL db.*` procedure namespaces
 *   - Reject string-concatenation heuristic (single-quote-then-plus)
 *   - Reject oversized payloads (length cap)
 *   - Accept the read-only allowlist (MATCH, WHERE, WITH, RETURN, ORDER BY,
 *     LIMIT, SKIP, OPTIONAL MATCH, UNWIND)
 *
 * Designed to run as `pnpm test:cypher-validator` (or directly with `tsx`)
 * with no extra runner; assertion failures call `process.exit(1)` so CI
 * integration is `npm run test:cypher-validator` + non-zero exit.
 */

import {
  validateReadOnlyCypher,
  CYPHER_MAX_LENGTH,
} from "../src/tools/_lib/cypher-validator.js";

let failures = 0;

function check(name: string, predicate: boolean, detail?: string): void {
  if (predicate) {
    process.stdout.write(`  ok  ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(
      `  FAIL ${name}${detail !== undefined ? ` — ${detail}` : ""}\n`,
    );
  }
}

function expectOk(name: string, query: string): void {
  const result = validateReadOnlyCypher(query);
  check(name, result.ok === true, result.ok ? undefined : `reason=${result.reason}`);
}

function expectReject(name: string, query: string, reasonSubstr: string): void {
  const result = validateReadOnlyCypher(query);
  check(
    name,
    result.ok === false && result.reason !== undefined &&
      result.reason.toLowerCase().includes(reasonSubstr.toLowerCase()),
    result.ok
      ? "validator returned ok for what should have been rejected"
      : `reason=${result.reason}`,
  );
}

// ─── Allowlist (happy path) ────────────────────────────────────────────
expectOk("simple MATCH...RETURN", "MATCH (n) RETURN n LIMIT 10");
expectOk(
  "MATCH...WHERE...RETURN with ORDER BY + SKIP",
  "MATCH (n:Entity) WHERE n.kind = 'dataset' RETURN n ORDER BY n.created_at SKIP 0 LIMIT 50",
);
expectOk("OPTIONAL MATCH allowed", "MATCH (n) OPTIONAL MATCH (n)-[r]->(m) RETURN n, r, m LIMIT 100");
expectOk("UNWIND allowed", "UNWIND [1,2,3] AS x RETURN x");
expectOk("WITH pipeline", "MATCH (n) WITH n LIMIT 10 RETURN n");

// ─── Mutation keywords (must reject) ───────────────────────────────────
expectReject(
  "DELETE rejected",
  "MATCH (n) DELETE n",
  "delete",
);
expectReject(
  "DETACH DELETE rejected",
  "MATCH (n) DETACH DELETE n",
  "delete",
);
expectReject(
  "CREATE rejected",
  "CREATE (n:Node {name: 'x'}) RETURN n",
  "create",
);
expectReject(
  "MERGE rejected",
  "MERGE (n:Node {id: 1}) RETURN n",
  "merge",
);
expectReject(
  "SET rejected",
  "MATCH (n) SET n.prop = 'evil' RETURN n",
  "set",
);
expectReject(
  "REMOVE rejected",
  "MATCH (n) REMOVE n.prop RETURN n",
  "remove",
);

// ─── DoS / bulk-mutation surface ───────────────────────────────────────
expectReject(
  "LOAD CSV rejected",
  "LOAD CSV FROM 'file:///etc/passwd' AS line RETURN line",
  "load csv",
);
// USING PERIODIC COMMIT in conjunction with LOAD CSV will trip LOAD CSV
// first (the validator returns on first match). Test the standalone form
// to pin the PERIODIC COMMIT rule specifically. Note the dummy literal
// avoids the single-quote-plus concat heuristic.
expectReject(
  "USING PERIODIC COMMIT rejected (standalone)",
  "USING PERIODIC COMMIT 1000 RETURN 1",
  "periodic commit",
);

// ─── Procedure namespaces (must reject) ────────────────────────────────
expectReject(
  "apoc.* rejected — apoc.cypher.run",
  "CALL apoc.cypher.run('MATCH (n) RETURN n', {}) YIELD value RETURN value",
  "apoc",
);
expectReject(
  "apoc.* rejected — apoc.export.json",
  "CALL apoc.export.json.all('/tmp/exfil.json', {}) YIELD file RETURN file",
  "apoc",
);
expectReject(
  "CALL db.* rejected — db.labels",
  "CALL db.labels() YIELD label RETURN label",
  "db.",
);
expectReject(
  "CALL db.* rejected — db.indexes",
  "CALL db.indexes() YIELD name RETURN name",
  "db.",
);

// ─── String-concatenation heuristic ────────────────────────────────────
// Detects naive string-built queries like `"... '" + userInput + "' ..."`
// — even though parameter-binding is the correct mitigation, the heuristic
// catches a class of caller errors at the MCP boundary before the
// projection-store path.
expectReject(
  "single-quote + plus heuristic — quote-then-plus",
  "MATCH (n) WHERE n.name = '" + "' + injectedVar + '" + "' RETURN n",
  "concat",
);
expectReject(
  "single-quote + plus heuristic — plus-then-quote",
  "MATCH (n) WHERE n.name = ' + " + "'" + " + injectedVar RETURN n",
  "concat",
);

// ─── Length cap ────────────────────────────────────────────────────────
{
  const oversized = "MATCH (n) RETURN n /* " + "x".repeat(CYPHER_MAX_LENGTH) + " */";
  expectReject("length cap rejects oversized payload", oversized, "length");
}

// ─── Edge cases ────────────────────────────────────────────────────────
expectReject("empty string rejected", "", "empty");
expectReject("whitespace-only rejected", "   \n\t  ", "empty");
expectReject("non-MATCH/UNWIND opener rejected (sanity)", "SELECT * FROM nodes", "must begin");

// ─── Comment-stripping interactions with opener check ─────────────────
// Regression: a leading block-comment used to cause the opener check to
// false-negative-reject a legitimate read-only query because `stripComments`
// replaced the comment with a single space, leaving leading whitespace
// before `MATCH`. The validator now re-trims after stripping. The inverse
// (leading comment + forbidden CREATE opener) must still reject.
expectOk(
  "leading block comment before MATCH allowed after re-trim",
  "/* preface */ MATCH (n) RETURN n LIMIT 5",
);
expectOk(
  "leading block comment directly adjacent to MATCH allowed",
  "/* */MATCH (n) RETURN n",
);
expectReject(
  "leading block comment before CREATE still rejected",
  "/* preface */ CREATE (n:Node) RETURN n",
  "create",
);

// ─── Case-insensitive matching ────────────────────────────────────────
expectReject(
  "lowercase delete still rejected",
  "match (n) delete n",
  "delete",
);
expectReject(
  "mixed-case CREATE still rejected",
  "Create (n) RETURN n",
  "create",
);

if (failures > 0) {
  process.stderr.write(`\n${failures} test(s) failed.\n`);
  process.exit(1);
}
process.stdout.write(`\nall cypher-validator tests passed\n`);
