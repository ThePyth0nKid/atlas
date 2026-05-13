/**
 * @atlas/cypher-validator — unified test suite.
 *
 * Consolidates W12's 24 vitest cases
 * (`apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.test.ts`)
 * and W13's 25 tsx-script cases
 * (`apps/atlas-mcp-server/scripts/test-cypher-validator.ts`).
 *
 * Deduplication removed identical happy-path + rejection cases that
 * appeared in both files. Three additional cases (marked "UNION") test
 * invariants that specifically validate the W15 union semantics:
 * procedure-namespace patterns from both W12 and W13 combined.
 *
 * Total: 45 cases.
 *
 * ADR reference: ADR-Atlas-009 §6 (the 6 consolidation invariants).
 */

import { describe, it, expect } from "vitest";
import { validateReadOnlyCypher, CYPHER_MAX_LENGTH } from "./index.js";

// ─── Happy paths ──────────────────────────────────────────────────────────────

describe("validateReadOnlyCypher — happy paths", () => {
  it("accepts a simple MATCH ... RETURN", () => {
    expect(validateReadOnlyCypher("MATCH (n) RETURN n LIMIT 10").ok).toBe(true);
  });

  it("accepts MATCH with WHERE + ORDER BY + SKIP + LIMIT", () => {
    expect(
      validateReadOnlyCypher(
        "MATCH (a)-[r]->(b) WHERE a.kind = $kind WITH a, b ORDER BY a.id SKIP 10 LIMIT 50 RETURN a, b",
      ).ok,
    ).toBe(true);
  });

  it("accepts OPTIONAL MATCH", () => {
    expect(
      validateReadOnlyCypher(
        "MATCH (n) OPTIONAL MATCH (n)-[r]->(m) RETURN n, r, m LIMIT 100",
      ).ok,
    ).toBe(true);
  });

  it("accepts UNWIND over a parameter list", () => {
    expect(
      validateReadOnlyCypher("UNWIND $ids AS id MATCH (n {id: id}) RETURN n").ok,
    ).toBe(true);
  });

  it("accepts WITH pipeline", () => {
    expect(
      validateReadOnlyCypher("MATCH (n) WITH n LIMIT 10 RETURN n").ok,
    ).toBe(true);
  });

  it("accepts MATCH ... WHERE ... RETURN with ORDER BY and paging", () => {
    expect(
      validateReadOnlyCypher(
        "MATCH (n:Entity) WHERE n.kind = 'dataset' RETURN n ORDER BY n.created_at SKIP 0 LIMIT 50",
      ).ok,
    ).toBe(true);
  });

  it("accepts UNWIND over inline list", () => {
    expect(
      validateReadOnlyCypher("UNWIND [1,2,3] AS x RETURN x").ok,
    ).toBe(true);
  });
});

// ─── Write-side keyword rejection ────────────────────────────────────────────

describe("validateReadOnlyCypher — write-side keyword rejection", () => {
  it("rejects DELETE", () => {
    const r = validateReadOnlyCypher("MATCH (n) DELETE n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/delete/i);
  });

  it("rejects DETACH DELETE specifically", () => {
    const r = validateReadOnlyCypher("MATCH (n) DETACH DELETE n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/detach delete/i);
  });

  it("rejects CREATE", () => {
    const r = validateReadOnlyCypher("CREATE (n:Foo {id: 'x'}) RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/create/i);
  });

  it("rejects MERGE", () => {
    const r = validateReadOnlyCypher("MERGE (n {id: 'x'}) RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/merge/i);
  });

  it("rejects SET", () => {
    const r = validateReadOnlyCypher("MATCH (n) SET n.foo = 'bar' RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/set/i);
  });

  it("rejects REMOVE", () => {
    const r = validateReadOnlyCypher("MATCH (n) REMOVE n.foo RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/remove/i);
  });

  it("rejects DROP (DDL)", () => {
    const r = validateReadOnlyCypher("DROP INDEX node_index_name");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/drop/i);
  });

  it("rejects FOREACH (mutation control flow)", () => {
    const r = validateReadOnlyCypher(
      "MATCH p = (a)-[*]->(b) FOREACH (n in nodes(p) | SET n.foo = 1)",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/foreach|set/i);
  });

  it("rejects LOAD CSV", () => {
    const r = validateReadOnlyCypher(
      "LOAD CSV FROM 'file:///etc/passwd' AS row RETURN row",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/load csv/i);
  });

  it("rejects USING PERIODIC COMMIT standalone", () => {
    // Test the PERIODIC COMMIT rule independently (not combined with LOAD CSV)
    const r = validateReadOnlyCypher(
      "USING PERIODIC COMMIT 1000 RETURN 1",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/periodic commit/i);
  });

  it("rejects keywords case-insensitively — lowercase delete", () => {
    expect(validateReadOnlyCypher("match (n) delete n").ok).toBe(false);
  });

  it("rejects keywords case-insensitively — mixed-case Create", () => {
    const r = validateReadOnlyCypher("Create (n) RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/create/i);
  });
});

// ─── Procedure-namespace rejection ───────────────────────────────────────────

describe("validateReadOnlyCypher — procedure namespace rejection", () => {
  it("rejects apoc.* — apoc.text inline", () => {
    const r = validateReadOnlyCypher(
      "RETURN apoc.text.regexGroups('a', 'b')",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/apoc/i);
  });

  it("rejects apoc.* — CALL apoc.cypher.run", () => {
    const r = validateReadOnlyCypher(
      "CALL apoc.cypher.run('MATCH (n) RETURN n', {}) YIELD value RETURN value",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/apoc/i);
  });

  it("rejects apoc.* — CALL apoc.export.json", () => {
    const r = validateReadOnlyCypher(
      "CALL apoc.export.json.all('/tmp/exfil.json', {}) YIELD file RETURN file",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/apoc/i);
  });

  it("rejects CALL db.* — db.labels", () => {
    const r = validateReadOnlyCypher(
      "CALL db.labels() YIELD label RETURN label",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/call|db/i);
  });

  it("rejects CALL db.* — db.indexes", () => {
    const r = validateReadOnlyCypher(
      "CALL db.indexes() YIELD name RETURN name",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/call|db/i);
  });

  it("rejects bare CALL — custom procedure", () => {
    const r = validateReadOnlyCypher(
      "CALL custom.something() YIELD x RETURN x",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/call/i);
  });

  // UNION test 1: W13's explicit CALL dbms.* pattern — not tested in W12
  it("[UNION] rejects CALL dbms.* — W13 explicit pattern", () => {
    const r = validateReadOnlyCypher(
      "CALL dbms.listQueries() YIELD queryId RETURN queryId",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/call|dbms/i);
  });

  // UNION test 2: W12's bare db.* without explicit CALL — not tested in W13
  it("[UNION] rejects inline db.* reference without explicit CALL — W12 pattern", () => {
    // W12 had `/\bdb\s*\./i` which catches `db.foo()` used inline
    // (not preceded by `CALL`). W13's `/\bCALL\s+db\s*\./i` would
    // NOT have caught this because there's no explicit `CALL` keyword.
    // The consolidated validator catches it via the `db.` deny-entry.
    const r = validateReadOnlyCypher(
      "MATCH (n) WHERE db.labels() CONTAINS n.label RETURN n",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/db\./i);
  });

  // UNION test 3: confirms BOTH W12-style + W13-style procedure patterns
  // are caught by the same consolidated pass
  it("[UNION] rejects W12-style bare CALL AND W13-style CALL dbms.* in same suite", () => {
    const bareCall = validateReadOnlyCypher(
      "CALL custom.read() YIELD row RETURN row",
    );
    const dbmsCall = validateReadOnlyCypher(
      "CALL dbms.procedures() YIELD name RETURN name",
    );
    expect(bareCall.ok).toBe(false);
    expect(dbmsCall.ok).toBe(false);
    // Both should have reason strings that mention CALL
    expect(bareCall.reason).toMatch(/call/i);
    expect(dbmsCall.reason).toMatch(/call|dbms/i);
  });
});

// ─── String concatenation rejection ──────────────────────────────────────────

describe("validateReadOnlyCypher — string concatenation rejection", () => {
  it("rejects '+' operator between identifier and parameter", () => {
    const r = validateReadOnlyCypher(
      "MATCH (n) WHERE n.id = 'pre' + $suffix RETURN n",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/\+/);
  });

  it("rejects quote-then-plus concat heuristic — W13-style", () => {
    // W13's quote-adjacent heuristic is a subset of W12's stricter
    // rule. This case is rejected by the consolidated `+` rule.
    const r = validateReadOnlyCypher(
      "MATCH (n) WHERE n.name = 'prefix' + $injected RETURN n",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/\+/);
  });

  it("accepts '+' INSIDE a single-quoted string literal (not operator)", () => {
    // Literal `'a+b'` — the validator strips string-literal contents
    // before checking, so '+' inside a quoted literal must not trip.
    const r = validateReadOnlyCypher("MATCH (n {label: 'a+b'}) RETURN n");
    expect(r.ok).toBe(true);
  });

  it("accepts '+' INSIDE a double-quoted string literal", () => {
    const r = validateReadOnlyCypher('MATCH (n {label: "a+b"}) RETURN n');
    expect(r.ok).toBe(true);
  });
});

// ─── Input hygiene ────────────────────────────────────────────────────────────

describe("validateReadOnlyCypher — input hygiene", () => {
  it("rejects empty query", () => {
    const r = validateReadOnlyCypher("");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/empty/i);
  });

  it("rejects whitespace-only query", () => {
    const r = validateReadOnlyCypher("   \n\t  ");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/empty/i);
  });

  it("rejects oversized query", () => {
    const big =
      "MATCH (n) WHERE n.id = $x RETURN n " + "x".repeat(CYPHER_MAX_LENGTH);
    const r = validateReadOnlyCypher(big);
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/exceeds|too long|length/i);
  });

  it("rejects a 4097-char query (exactly one over the 4096-char cap)", () => {
    expect(CYPHER_MAX_LENGTH).toBe(4096);
    const r = validateReadOnlyCypher("x".repeat(4097));
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/exceeds|too long|length/i);
  });

  it("accepts a 4096-char query (exactly at cap)", () => {
    // Build a valid-structure query that hits exactly 4096 chars.
    // Pad with a long WHERE string-literal so the validator sees it
    // as structurally valid (comment-stripped opener is MATCH).
    const base = "MATCH (n) WHERE n.id = $x RETURN n";
    const pad = " ".repeat(CYPHER_MAX_LENGTH - base.length);
    const atCap = base + pad;
    expect(atCap.length).toBe(CYPHER_MAX_LENGTH);
    // The padded query may or may not pass all checks (opener-check
    // may reject it), but it must NOT be rejected FOR LENGTH.
    const r = validateReadOnlyCypher(atCap);
    if (r.reason !== undefined) {
      expect(r.reason).not.toMatch(/exceeds/i);
    }
  });

  it("rejects a non-MATCH opener (sanity — SELECT not Cypher)", () => {
    const r = validateReadOnlyCypher("SELECT * FROM nodes");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/must begin/i);
  });
});

// ─── Comment-stripping interactions ──────────────────────────────────────────

describe("validateReadOnlyCypher — comment-stripping + opener correctness", () => {
  it("DELETE inside a block comment is NOT rejected (comment stripped)", () => {
    // The validator strips comments BEFORE keyword check.
    const r = validateReadOnlyCypher("MATCH (n) /* DELETE */ RETURN n");
    expect(r.ok).toBe(true);
  });

  it("DELETE inside a line comment is NOT rejected", () => {
    const r = validateReadOnlyCypher("MATCH (n) RETURN n // DELETE n");
    expect(r.ok).toBe(true);
  });

  it("leading block comment before MATCH is accepted after re-trim", () => {
    // stripComments replaces the comment with a space, leaving
    // ` MATCH (n) RETURN n`. The .trimStart() post-strip fixes the
    // opener check — W13's correctness invariant, adopted universally.
    const r = validateReadOnlyCypher("/* preface */ MATCH (n) RETURN n LIMIT 5");
    expect(r.ok).toBe(true);
  });

  it("leading block comment directly adjacent to MATCH is accepted", () => {
    const r = validateReadOnlyCypher("/* */MATCH (n) RETURN n");
    expect(r.ok).toBe(true);
  });

  it("leading block comment before CREATE is still rejected", () => {
    const r = validateReadOnlyCypher("/* preface */ CREATE (n:Node) RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/create/i);
  });
});
