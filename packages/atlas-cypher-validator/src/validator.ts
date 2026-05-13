/**
 * @atlas/cypher-validator — shared Cypher read-only validator.
 *
 * Extracted in V2-β Welle 15 (rule-of-three) from W12's
 * `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.ts` and
 * W13's `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts`.
 * Both inline copies were aligned post-Phase-4-consistency-fix before
 * extraction. See `docs/ADR/ADR-Atlas-009-*` for full rationale.
 *
 * Driving decision: DECISION-SEC-4 (Cypher Passthrough Hardening).
 *
 * ## Threat model
 *
 * 1. **Read-side only.** Reject any write-side keyword the user could
 *    embed in a parameter, comment, or string-concatenated tail to
 *    mutate the graph: `DELETE`, `DETACH DELETE`, `CREATE`, `MERGE`,
 *    `SET`, `REMOVE`, `DROP`, `FOREACH`, `LOAD CSV`,
 *    `USING PERIODIC COMMIT`.
 *
 *    `DROP` is DDL (drops indexes/constraints in Neo4j / ArcadeDB
 *    dialects). `FOREACH` alone is benign but is the canonical
 *    composition pattern for iterated writes, so we reject it
 *    defensively — defence-in-depth even though the inner `SET` /
 *    `CREATE` / `MERGE` would also trip the deny-list.
 *
 * 2. **No procedure escape.** Reject `apoc.*`, bare `CALL`, and
 *    explicit `CALL db.*` / `CALL dbms.*` — the procedure surface is
 *    a documented sandbox-escape vector in graph DBs (Neo4j APOC,
 *    FalkorDB GRAPH.* admin functions, ArcadeDB schema operations).
 *
 *    **Union rule (ADR-Atlas-009 §6.4):** W12 used `/\bdb\s*\./i` +
 *    `/\bCALL\b/i` (catches bare-CALL); W13 used
 *    `/\bCALL\s+db\s*\./i` + `/\bCALL\s+dbms\s*\./i` (explicit-CALL
 *    only). The consolidated validator picks the UNION: bare-CALL
 *    rejection AND explicit `CALL dbms/db` rejection. This is strictly
 *    more restrictive than either implementation; no consumer test
 *    passes valid bare-CALL invocations, so there is zero regression.
 *    See ADR for trade-off analysis and path to future procedure
 *    allow-list.
 *
 * 3. **No string concatenation.** Reject any `+` token in the
 *    stripped query (ADR-Atlas-009 §6.5 — W12's stricter rule chosen
 *    over W13's quote-adjacent heuristic). Rationale: arithmetic or
 *    list concatenation has no valid use in a parameter-bound read-only
 *    Cypher query. Conservative rejection prevents caller-side
 *    injection mistakes. The `params` object is the correct channel
 *    for dynamic values.
 *
 * 4. **Allow-list for top-level statement start.** After
 *    comment-stripping + `.trimStart()` (W13's invariant, adopted
 *    universally in W15), the query MUST begin with one of: `MATCH`,
 *    `OPTIONAL MATCH`, `WITH`, `UNWIND`, `RETURN`. This coarse
 *    shield guards against unexpected statement shapes that the
 *    keyword deny-list might miss.
 *
 * ## Limitations (documented for future AST-level pass, V2-γ)
 *
 * - Regex-based. A real AST parser would catch unicode escapes hiding
 *   `DELETE`, comment-injection, and case-sensitive keyword evasions
 *   across line boundaries.
 * - The comment + string-literal strip is regex-best-effort and will
 *   mis-handle adversarial escaped-quote sequences. The check is
 *   therefore intentionally fail-closed: false positives (rejecting
 *   a valid query) are preferred over false negatives at v2.0.0-beta.1.
 * - No procedure allow-list; bare `CALL` is universally rejected.
 *   A future welle can relax this with an explicit allow-list once
 *   the production procedure surface is defined.
 *
 * ## Parameter naming
 *
 * This module validates the Cypher string only. It does NOT inspect
 * or enforce parameter names. The HTTP consumer uses `workspace`
 * (V2-α atlas-signer convention); the MCP consumer uses `workspace_id`
 * (pre-existing MCP-package convention since V1.19 Welle 1). This
 * split is intentional per-package convention; it is NOT reconciled
 * here. See ADR-Atlas-009 §6.6.
 */

export interface CypherValidationResult {
  readonly ok: boolean;
  readonly reason?: string;
}

/**
 * Hard cap on query length. Real-world legitimate read queries are
 * well under this. Anything larger is a likely DoS attempt or
 * machine-generated Cypher that should not be hitting a read API.
 *
 * Aligned across W12 + W13 post-Phase-4-consistency-fix HIGH-1.
 * W15 extracts it as the single source of truth.
 */
export const CYPHER_MAX_LENGTH = 4096;

/**
 * Forbidden write-side keyword patterns. Each is matched as a
 * whole-word token (\b boundaries), case-insensitive (per Cypher
 * spec — keywords are case-insensitive, identifiers are
 * case-sensitive).
 *
 * `DETACH DELETE` is checked before bare `DELETE` so the user gets
 * the more-specific rejection message. `LOAD CSV` before `USING
 * PERIODIC COMMIT` for the same reason.
 *
 * Union of W12 + W13 lists — both were identical post-consistency-fix.
 */
const FORBIDDEN_KEYWORDS: ReadonlyArray<{ token: RegExp; reason: string }> = [
  {
    token: /\bDETACH\s+DELETE\b/i,
    reason: "DETACH DELETE not allowed (mutation)",
  },
  { token: /\bDELETE\b/i, reason: "DELETE not allowed (mutation)" },
  { token: /\bCREATE\b/i, reason: "CREATE not allowed (mutation)" },
  { token: /\bMERGE\b/i, reason: "MERGE not allowed (mutation)" },
  { token: /\bSET\b/i, reason: "SET not allowed (mutation)" },
  { token: /\bREMOVE\b/i, reason: "REMOVE not allowed (mutation)" },
  { token: /\bDROP\b/i, reason: "DROP not allowed (DDL mutation)" },
  {
    token: /\bLOAD\s+CSV\b/i,
    reason: "LOAD CSV not allowed (DoS / file-system surface)",
  },
  {
    token: /\bUSING\s+PERIODIC\s+COMMIT\b/i,
    reason: "USING PERIODIC COMMIT not allowed",
  },
  {
    token: /\bFOREACH\b/i,
    reason: "FOREACH not allowed (mutation control flow)",
  },
];

/**
 * Forbidden procedure-namespace patterns.
 *
 * Union of W12 + W13 patterns (ADR-Atlas-009 §6.4):
 *   - `/\bapoc\s*\./i` — present in both
 *   - `/\bdb\s*\./i` — W12's broad db.* pattern (catches inline db.*)
 *   - `/\bCALL\b/i` — W12's bare-CALL rejection
 *   - `/\bCALL\s+db\s*\./i` — W13's explicit-CALL db.* pattern
 *   - `/\bCALL\s+dbms\s*\./i` — W13's explicit-CALL dbms.* pattern
 *
 * The union is strictly more restrictive than either W12 or W13 alone.
 * No consumer test that was passing before W15 passes a bare-CALL or
 * explicit `CALL db.*` / `CALL dbms.*`, so there is zero regression.
 *
 * Execution order: the write-keyword deny-list (FORBIDDEN_KEYWORDS) runs
 * FIRST in `validateReadOnlyCypher` because mutation keywords like
 * `DELETE` / `CREATE` / `SET` are more common in production-mistake
 * queries than procedure-namespace escapes; the procedure-namespace
 * deny-list runs AFTER for sandbox-escape catching. `CALL apoc.x` still
 * gets the more-specific `apoc.* procedures not allowed` reason because
 * `CALL` is not in FORBIDDEN_KEYWORDS — only mutation keywords are.
 *
 * Note on overlap: the specific `CALL dbms.*` entry overlaps with the
 * broader bare-`CALL` guard below it; both reject the same patterns.
 * This is intentional defence-in-depth — the specific entries document
 * which procedure namespaces are the canonical sandbox-escape vectors,
 * and the broader bare-CALL guard is the catch-all. Do not remove the
 * specific entries thinking they are subsumed: they encode intent.
 */
const FORBIDDEN_PROCEDURE_NAMESPACES: ReadonlyArray<{
  token: RegExp;
  reason: string;
}> = [
  {
    token: /\bapoc\s*\./i,
    reason: "apoc.* procedures not allowed",
  },
  {
    token: /\bdb\s*\./i,
    reason: "db.* not allowed (schema-introspection / index management)",
  },
  {
    token: /\bCALL\s+dbms\s*\./i,
    reason: "CALL dbms.* not allowed (server administration)",
  },
  {
    token: /\bCALL\b/i,
    reason:
      "CALL not allowed (no procedure allow-list in V2-β; V2-γ will add allow-list)",
  },
];

/**
 * Opener allow-list. After comment-stripping and `.trimStart()` the
 * query MUST begin with one of these read-only openers. This is a
 * coarse-but-effective shield against unexpected statement shapes.
 *
 * Adopted from W13's implementation; W12 lacked this check. Adding it
 * in the consolidated validator makes the shared module strictly more
 * secure than W12's inline was.
 */
const ALLOWED_OPENERS: ReadonlyArray<RegExp> = [
  /^MATCH\b/i,
  /^OPTIONAL\s+MATCH\b/i,
  /^WITH\b/i,
  /^UNWIND\b/i,
  /^RETURN\b/i,
];

/**
 * Strip Cypher block comments (`/* ... *\/`) and line comments
 * (`// ...`) plus replace string-literal contents with empty
 * placeholders so subsequent keyword matching does not false-positive
 * on a keyword inside a literal.
 *
 * Cypher string literals: single-quoted or double-quoted,
 * backslash-escaped.
 *
 * Limitation (documented for V2-γ AST pass): a malicious user can
 * embed an escape sequence the regex mis-handles. The keyword check
 * is fail-closed — false positives are preferred.
 */
function stripCommentsAndStrings(query: string): string {
  // 1. Block comments — non-greedy
  let s = query.replace(/\/\*[\s\S]*?\*\//g, " ");
  // 2. Line comments (Cypher accepts `//`)
  s = s.replace(/\/\/[^\n]*/g, " ");
  // 3. String-literal contents — replace with empty placeholders so
  //    overall structure is preserved (no keyword matches inside).
  s = s.replace(/'(?:\\.|[^'\\])*'/g, "''");
  s = s.replace(/"(?:\\.|[^"\\])*"/g, '""');
  return s;
}

/**
 * Validate that `query` is a read-only Cypher string that contains
 * none of the forbidden patterns. Returns `{ ok: true }` on accept,
 * `{ ok: false, reason }` on reject.
 *
 * Defence ordering:
 *   1. Type guard (cheap)
 *   2. Length cap (cheap)
 *   3. Empty guard
 *   4. String-concat `+` detection on raw query (pre-strip, so a
 *      smuggled concat inside a comment is also caught)
 *   5. Strip comments + string-literal contents
 *   6. `.trimStart()` after strip (W13's correctness invariant for
 *      the opener-allowlist check on leading-comment queries)
 *   7. Write-keyword deny-list
 *   8. Procedure-namespace deny-list
 *   9. Opener allow-list
 */
export function validateReadOnlyCypher(
  query: string,
): CypherValidationResult {
  if (typeof query !== "string") {
    return { ok: false, reason: "cypher: must be a string" };
  }
  if (query.length > CYPHER_MAX_LENGTH) {
    return {
      ok: false,
      reason: `cypher exceeds maximum length (${query.length} > ${CYPHER_MAX_LENGTH})`,
    };
  }
  const trimmed = query.trim();
  if (trimmed.length === 0) {
    return { ok: false, reason: "cypher: empty query" };
  }

  // String-concat `+` detection. Run on the raw (pre-strip) query so a
  // `+` smuggled inside a comment is also caught. W12's stricter rule:
  // reject ANY `+` token, not just quote-adjacent (W13's narrower rule).
  // Rationale: arithmetic/list concatenation has no valid use in a
  // parameter-bound read-only Cypher query. See ADR-Atlas-009 §6.5.
  //
  // Exception: `+` inside a string literal is benign. Strip literals
  // first to avoid false-positives on `n {label: 'a+b'}`.
  //
  // We do a targeted strip of just string literals (not comments) for
  // this check, then check for `+` in the literal-stripped form.
  const literalStripped = query
    .replace(/'(?:\\.|[^'\\])*'/g, "''")
    .replace(/"(?:\\.|[^"\\])*"/g, '""');
  if (/\+/.test(literalStripped)) {
    return {
      ok: false,
      reason:
        "cypher: '+' operator is not permitted (use parameter binding via the `params` object instead)",
    };
  }

  // Full strip: comments + string literals. Then .trimStart() per W13's
  // invariant so a leading-block-comment query like `/* */MATCH (n) RETURN n`
  // becomes `MATCH (n) RETURN n` for the opener check.
  const stripped = stripCommentsAndStrings(trimmed).trimStart();

  // Write-keyword deny-list.
  for (const { token, reason } of FORBIDDEN_KEYWORDS) {
    if (token.test(stripped)) {
      return { ok: false, reason };
    }
  }

  // Procedure-namespace deny-list — runs after keyword check because
  // the keyword check is tighter for most write operations.
  for (const { token, reason } of FORBIDDEN_PROCEDURE_NAMESPACES) {
    if (token.test(stripped)) {
      return { ok: false, reason };
    }
  }

  // Opener allow-list — the stripped, left-trimmed query must begin
  // with one of the read-only openers.
  const openerOk = ALLOWED_OPENERS.some((re) => re.test(stripped));
  if (!openerOk) {
    return {
      ok: false,
      reason:
        "cypher must begin with MATCH, OPTIONAL MATCH, WITH, UNWIND, or RETURN",
    };
  }

  return { ok: true };
}
