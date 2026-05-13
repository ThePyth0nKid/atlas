/**
 * W13-LOCAL inline Cypher validator. INDEPENDENT of W12's parallel
 * implementation. The rule-of-three pattern (W15) consolidates BOTH
 * implementations into a shared module AFTER seeing them side-by-side —
 * DO NOT import from W12's path, and DO NOT pre-extract a shared module
 * here. That is W15's deliverable.
 *
 * Driving decision: `.handoff/decisions.md` DECISION-SEC-4 (Cypher
 * Passthrough Hardening). At the V2-β Phase-4 boundary the validator
 * runs at the MCP-tool layer BEFORE any data-layer call so the
 * projection-store never sees a forbidden token.
 *
 * Implementation: regex-based. The phase-4 projection-store is a stub
 * (no live database), so the validator's job is to enforce the contract
 * that the MCP tool surface accepts only read-only Cypher within an
 * explicit allowlist. W15 will replace this with an AST-level check
 * against a Cypher parser; this regex-pass is deliberately narrow + over-
 * rejecting to keep the surface honest while the projection-store is
 * still stubbed out.
 *
 * KNOWN LIMITATIONS (documented for W15's consolidation):
 *   - Unicode-escaped tokens (e.g. `delete`) are not normalized.
 *   - Comments (/* ... *\/) are stripped via a permissive pattern; a
 *     nested or unterminated comment may smuggle forbidden tokens.
 *   - String literals containing forbidden keywords trigger false-
 *     positives. Acceptable for V2-β Phase-4 because the projection-
 *     store stub does not actually execute Cypher; W15's AST pass will
 *     resolve.
 *   - The string-concat heuristic flags any single-quote-followed-by-
 *     plus pattern. False-positive risk in queries that legitimately
 *     concatenate constants. Acceptable; parameter binding is the
 *     correct caller-side mitigation.
 */

export interface CypherValidationResult {
  readonly ok: boolean;
  readonly reason?: string;
}

/**
 * Maximum allowed length of the raw Cypher input, in characters.
 * Defence against multi-megabyte queries that would balloon the
 * projection-store path's working set even before semantic analysis.
 */
export const CYPHER_MAX_LENGTH = 4096;

const FORBIDDEN_KEYWORDS: ReadonlyArray<{ token: RegExp; reason: string }> = [
  { token: /\bDETACH\s+DELETE\b/i, reason: "DETACH DELETE not allowed (mutation)" },
  { token: /\bDELETE\b/i, reason: "DELETE not allowed (mutation)" },
  { token: /\bCREATE\b/i, reason: "CREATE not allowed (mutation)" },
  { token: /\bMERGE\b/i, reason: "MERGE not allowed (mutation)" },
  { token: /\bSET\b/i, reason: "SET not allowed (mutation)" },
  { token: /\bREMOVE\b/i, reason: "REMOVE not allowed (mutation)" },
  { token: /\bDROP\b/i, reason: "DROP not allowed (mutation)" },
  { token: /\bLOAD\s+CSV\b/i, reason: "LOAD CSV not allowed (DoS / file-system surface)" },
  { token: /\bUSING\s+PERIODIC\s+COMMIT\b/i, reason: "USING PERIODIC COMMIT not allowed" },
  { token: /\bFOREACH\b/i, reason: "FOREACH not allowed (mutation control flow)" },
];

const FORBIDDEN_PROCEDURE_NAMESPACES: ReadonlyArray<{ token: RegExp; reason: string }> = [
  // `apoc.*` library — broad surface incl. file IO, network, dynamic Cypher.
  { token: /\bapoc\s*\./i, reason: "apoc.* procedures not allowed" },
  // `CALL db.*` — schema-introspection + index management; not part of the
  // read-side contract.
  { token: /\bCALL\s+db\s*\./i, reason: "CALL db.* not allowed" },
  // Defensive: any CALL to dbms.* (server administration).
  { token: /\bCALL\s+dbms\s*\./i, reason: "CALL dbms.* not allowed" },
];

const ALLOWED_OPENERS: ReadonlyArray<RegExp> = [
  /^MATCH\b/i,
  /^OPTIONAL\s+MATCH\b/i,
  /^WITH\b/i,
  /^UNWIND\b/i,
  /^RETURN\b/i,
];

/**
 * Strip block comments (non-greedy) and line comments. Defensive enough
 * for the regex-based pass; W15 AST pass replaces this.
 */
function stripComments(query: string): string {
  // Block comments
  let out = query.replace(/\/\*[\s\S]*?\*\//g, " ");
  // Line comments (Cypher accepts `//`)
  out = out.replace(/\/\/[^\n]*/g, " ");
  return out;
}

/**
 * Detect a naive string-concatenation pattern: a single-quoted literal
 * adjacent to a `+` operator. This is the canonical "I built my query
 * via string concat instead of parameters" shape. Parameter binding is
 * the correct mitigation; flagging the shape catches the caller before
 * the data-layer.
 */
function looksLikeStringConcat(query: string): boolean {
  // 'literal' + ident   OR   ident + 'literal'
  return /'\s*\+|\+\s*'/i.test(query);
}

export function validateReadOnlyCypher(query: string): CypherValidationResult {
  if (typeof query !== "string") {
    return { ok: false, reason: "cypher must be a string" };
  }
  if (query.length > CYPHER_MAX_LENGTH) {
    return {
      ok: false,
      reason: `cypher exceeds maximum length (${query.length} > ${CYPHER_MAX_LENGTH})`,
    };
  }
  const trimmed = query.trim();
  if (trimmed.length === 0) {
    return { ok: false, reason: "cypher must not be empty" };
  }

  // Run the concat heuristic on the raw query — comments do not legitimately
  // contain the concat shape, and stripping them first would hide a smuggled
  // pattern.
  if (looksLikeStringConcat(query)) {
    return {
      ok: false,
      reason:
        "cypher appears to use string concatenation; use parameter binding instead",
    };
  }

  const stripped = stripComments(trimmed);

  for (const { token, reason } of FORBIDDEN_KEYWORDS) {
    if (token.test(stripped)) {
      return { ok: false, reason };
    }
  }
  for (const { token, reason } of FORBIDDEN_PROCEDURE_NAMESPACES) {
    if (token.test(stripped)) {
      return { ok: false, reason };
    }
  }

  // Opener allowlist — the query must begin with one of the read-only
  // openers after comment-stripping. This is a coarse-but-effective
  // shield against unexpected statement shapes.
  const openerOk = ALLOWED_OPENERS.some((re) => re.test(stripped));
  if (!openerOk) {
    return {
      ok: false,
      reason: "cypher must begin with MATCH, OPTIONAL MATCH, WITH, UNWIND, or RETURN",
    };
  }

  return { ok: true };
}
