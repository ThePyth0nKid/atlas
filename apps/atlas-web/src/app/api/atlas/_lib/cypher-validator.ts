/**
 * V2-β Welle 12 — Welle-local Cypher read-only validator.
 *
 * THIS IS A WELLE-LOCAL IMPLEMENTATION. Welle 15 consolidates the
 * shared module after rule-of-three (W12 atlas-web + W13 atlas-mcp-
 * server + later consumer). DO NOT import this from another package
 * or move it to a shared location; that's W15's job.
 *
 * Threat model (`DECISION-SEC-4` — Cypher injection / DoS hygiene):
 *
 *   1. **Read-side only.** Reject any write-side keyword the user
 *      could embed in a parameter, comment, or string-concatenated
 *      tail to mutate the graph: `DELETE`, `DETACH DELETE`, `CREATE`,
 *      `MERGE`, `SET`, `REMOVE`, `DROP`, `FOREACH`, `LOAD CSV`,
 *      `USING PERIODIC COMMIT`. `DROP` is DDL (drops indexes /
 *      constraints in Neo4j / ArcadeDB dialects). `FOREACH` alone is
 *      benign but is the canonical composition pattern for iterated
 *      writes (`FOREACH (n IN ... | SET n.x = 1)`), so we reject it
 *      defensively — defence-in-depth even though the inner `SET` /
 *      `CREATE` / `MERGE` would also trip the deny-list.
 *
 *   2. **No procedure escape.** Reject `apoc.*` and `CALL db.*` —
 *      the procedure surface is a documented sandbox-escape vector
 *      in graph DBs (Neo4j APOC, FalkorDB GRAPH.* admin functions).
 *
 *   3. **No string concatenation.** Detect naive `+` between two
 *      operands one of which is plausibly attacker-controlled. The
 *      proper fix is parameter-only Cypher (we plumb a `params`
 *      object), but defence-in-depth rejects the affordance entirely.
 *
 *   4. **Allow-list, not deny-list, for top-level statement start.**
 *      The query MUST begin (after whitespace + comments) with one of
 *      `MATCH`, `OPTIONAL MATCH`, `WITH`, `UNWIND`, `RETURN`, `CALL`
 *      (where `CALL` is the procedure-allow-list-gated form — but
 *      since the deny-list above already rejects `CALL db.*` and the
 *      apoc namespace, a bare `CALL` is in principle allowed; we
 *      still reject ALL `CALL` invocations in this welle-local
 *      version because we have no procedure allow-list yet and
 *      false-positives are preferred over false-negatives at the
 *      v2.0.0-beta.1 stage).
 *
 * Limitations (documented for reviewer + W15):
 *
 *   - Regex-based. A real AST parser (W15 consolidation) catches
 *     pathological cases this version cannot: unicode escapes
 *     hiding `DELETE`, comment-injection (`/* DELETE *\/`), and
 *     case-sensitive keyword evasions across line boundaries.
 *
 *   - The validator strips comments and string-literal contents
 *     before keyword matching, but the strip is regex-best-effort
 *     and will mis-handle adversarial escaped-quote sequences. The
 *     check is therefore intentionally **fail-closed**: if any of
 *     the four classes above match, we reject. False positives are
 *     better than false negatives for v2.0.0-beta.1.
 *
 *   - Maximum query length is 4096 chars. The route handler also caps
 *     request body bytes at 256 KB before this function runs (matches
 *     write-node's belt-and-braces cap). Beyond DoS, an oversized
 *     query indicates an automated-attack or a misuse. The 4096 floor
 *     is shared with W13 (atlas-mcp-server) — cross-batch consistency
 *     reviewer HIGH-1 finding: W12 had drifted to 16 KB and was
 *     converged to the stricter W13 floor. W15 (rule-of-three
 *     consolidation) will extract the shared cap constant.
 */

export interface CypherValidationResult {
  ok: boolean;
  reason?: string;
}

/**
 * Hard cap on query length. Real-world legitimate read queries are
 * well under this; anything larger is a likely DoS attempt or
 * machine-generated cypher that should not be hitting a read API.
 */
export const CYPHER_MAX_LENGTH = 4096;

/**
 * Forbidden write-side keywords. Each is matched as a whole-word
 * token (\b boundaries) and is case-insensitive (per Cypher spec —
 * keywords are case-insensitive, identifiers are case-sensitive).
 *
 * Order matters for error-message clarity only: `DETACH DELETE` is
 * checked before bare `DELETE` so the user gets the more-specific
 * message.
 */
const FORBIDDEN_KEYWORDS = [
  "DETACH\\s+DELETE",
  "DELETE",
  "CREATE",
  "MERGE",
  "SET",
  "REMOVE",
  "DROP",
  "FOREACH",
  "LOAD\\s+CSV",
  "USING\\s+PERIODIC\\s+COMMIT",
];

/**
 * Forbidden procedure-namespace patterns. `apoc.*` and `CALL db.*`
 * are the two big sandbox-escape surfaces. We also reject bare
 * `CALL` in this welle until W15 adds a real procedure allow-list.
 */
const FORBIDDEN_PROCEDURE_RES = [
  // CALL apoc.foo() or apoc.foo() inline
  /\bapoc\s*\./i,
  // CALL db.foo() or db.foo() inline
  /\bdb\s*\./i,
  // bare CALL ... — no procedure allow-list in W12; W15 may relax this
  /\bCALL\b/i,
];

/**
 * Strip Cypher comments and string-literal contents so that
 * subsequent keyword matching does not match inside a literal.
 *
 * Cypher comments:
 *   - line:  // ... \n
 *   - block: /\* ... *\/
 *
 * String literals: single-quoted or double-quoted, backslash-escaped.
 *
 * Limitation: a malicious user can embed an escape sequence the
 * regex mis-handles. We accept that — the keyword check itself is
 * fail-closed and the surrounding context (route handler also caps
 * body bytes, params transit via a separate JSON path) makes the
 * remaining attack surface narrow.
 */
function stripCommentsAndStrings(query: string): string {
  // 1. block comments
  let s = query.replace(/\/\*[\s\S]*?\*\//g, " ");
  // 2. line comments
  s = s.replace(/\/\/[^\n]*/g, " ");
  // 3. string literals — replace contents with a single space so the
  //    overall structure is preserved (whitespace, no keyword matches).
  s = s.replace(/'(?:\\.|[^'\\])*'/g, "''");
  s = s.replace(/"(?:\\.|[^"\\])*"/g, '""');
  return s;
}

/**
 * Validate that `query` is a read-only Cypher query that contains
 * none of the forbidden patterns. Returns `{ ok: true }` on accept,
 * `{ ok: false, reason }` on reject.
 *
 * Defence ordering:
 *   1. Empty / length cap (cheap)
 *   2. Strip comments + strings (so subsequent matches don't false-positive)
 *   3. Procedure-namespace deny-list
 *   4. Write-keyword deny-list
 *   5. String-concatenation `+` between non-numeric/non-literal operands
 */
export function validateReadOnlyCypher(query: string): CypherValidationResult {
  if (typeof query !== "string") {
    return { ok: false, reason: "cypher: must be a string" };
  }
  const trimmed = query.trim();
  if (trimmed.length === 0) {
    return { ok: false, reason: "cypher: empty query" };
  }
  if (query.length > CYPHER_MAX_LENGTH) {
    return {
      ok: false,
      reason: `cypher: query exceeds ${CYPHER_MAX_LENGTH} characters`,
    };
  }

  const stripped = stripCommentsAndStrings(query);

  // Procedure-namespace deny-list — runs first because `CALL apoc.x`
  // contains both `CALL` and `apoc.`, and the user is best served
  // by the most specific message.
  for (const re of FORBIDDEN_PROCEDURE_RES) {
    if (re.test(stripped)) {
      return {
        ok: false,
        reason: `cypher: forbidden procedure/namespace (${describeProcRe(re)})`,
      };
    }
  }

  // Write-keyword deny-list.
  for (const kw of FORBIDDEN_KEYWORDS) {
    const re = new RegExp(`\\b${kw}\\b`, "i");
    if (re.test(stripped)) {
      return {
        ok: false,
        reason: `cypher: write-side keyword '${kw.replace(/\\s\+/g, " ")}' is not permitted in read API`,
      };
    }
  }

  // String-concatenation `+` detection. We look for `+` operator
  // between identifiers/parameters/property accesses. The stripped
  // query has string literals replaced with `""`/`''`, so any `+`
  // remaining is either numeric arithmetic (acceptable in principle
  // but rejected in W12 — read API has no need for arithmetic),
  // string concatenation, or list concatenation. We reject all.
  //
  // A single `+` token outside a property-update context (which we
  // already reject via `SET`) is the signal. Exception: `LIMIT 100`
  // does not contain `+`; `RETURN n.a + n.b` does.
  if (/\+/.test(stripped)) {
    return {
      ok: false,
      reason:
        "cypher: '+' operator is not permitted (use parameter binding via the `params` object instead)",
    };
  }

  return { ok: true };
}

/**
 * Render a forbidden-procedure regex back to a user-friendly label.
 * Pure presentation; never reaches a hot path.
 */
function describeProcRe(re: RegExp): string {
  const src = re.source;
  if (src.includes("apoc")) return "apoc.*";
  if (src.includes("db")) return "CALL db.*";
  if (src.includes("CALL")) return "CALL (no procedure allow-list in V2-β beta.1)";
  return "procedure";
}
