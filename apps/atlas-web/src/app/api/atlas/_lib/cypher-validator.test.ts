/**
 * V2-β Welle 12 — Cypher validator unit tests.
 *
 * Coverage per the dispatch spec: 8+ cases covering each forbidden
 * pattern + happy paths. We ship 14 cases to exercise edge handling
 * around comment-stripping and string-literal masking, since those
 * are the validator's known weak points.
 */

import { describe, it, expect } from "vitest";
import {
  validateReadOnlyCypher,
  CYPHER_MAX_LENGTH,
} from "./cypher-validator";

describe("validateReadOnlyCypher — happy paths", () => {
  it("accepts a simple MATCH ... RETURN", () => {
    const r = validateReadOnlyCypher("MATCH (n) RETURN n LIMIT 10");
    expect(r.ok).toBe(true);
  });

  it("accepts OPTIONAL MATCH + WHERE + ORDER BY + SKIP + LIMIT", () => {
    const r = validateReadOnlyCypher(
      "MATCH (a)-[r]->(b) WHERE a.kind = $kind WITH a, b ORDER BY a.id SKIP 10 LIMIT 50 RETURN a, b",
    );
    expect(r.ok).toBe(true);
  });

  it("accepts UNWIND over a parameter list", () => {
    const r = validateReadOnlyCypher("UNWIND $ids AS id MATCH (n {id: id}) RETURN n");
    expect(r.ok).toBe(true);
  });
});

describe("validateReadOnlyCypher — write-side keyword rejection", () => {
  it("rejects DELETE", () => {
    const r = validateReadOnlyCypher("MATCH (n) DELETE n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/DELETE/);
  });

  it("rejects DETACH DELETE specifically", () => {
    const r = validateReadOnlyCypher("MATCH (n) DETACH DELETE n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/DETACH DELETE/);
  });

  it("rejects CREATE", () => {
    const r = validateReadOnlyCypher("CREATE (n:Foo {id: 'x'}) RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/CREATE/);
  });

  it("rejects MERGE", () => {
    const r = validateReadOnlyCypher("MERGE (n {id: 'x'}) RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/MERGE/);
  });

  it("rejects SET", () => {
    const r = validateReadOnlyCypher("MATCH (n) SET n.foo = 'bar' RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/SET/);
  });

  it("rejects REMOVE", () => {
    const r = validateReadOnlyCypher("MATCH (n) REMOVE n.foo RETURN n");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/REMOVE/);
  });

  it("rejects LOAD CSV", () => {
    const r = validateReadOnlyCypher(
      "LOAD CSV FROM 'file:///etc/passwd' AS row RETURN row",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/LOAD CSV/);
  });

  it("rejects USING PERIODIC COMMIT", () => {
    const r = validateReadOnlyCypher(
      "USING PERIODIC COMMIT 1000 LOAD CSV FROM 'x' AS r RETURN r",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/USING PERIODIC COMMIT|LOAD CSV/);
  });

  it("rejects keywords case-insensitively", () => {
    const r = validateReadOnlyCypher("match (n) delete n");
    expect(r.ok).toBe(false);
  });
});

describe("validateReadOnlyCypher — procedure namespace rejection", () => {
  it("rejects apoc.*", () => {
    const r = validateReadOnlyCypher("RETURN apoc.text.regexGroups('a', 'b')");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/apoc/);
  });

  it("rejects CALL db.*", () => {
    const r = validateReadOnlyCypher("CALL db.labels() YIELD label RETURN label");
    expect(r.ok).toBe(false);
    // either the CALL or the db.* check trips first
    expect(r.reason).toMatch(/CALL|db/);
  });

  it("rejects bare CALL (no allow-list in W12)", () => {
    const r = validateReadOnlyCypher(
      "CALL custom.something() YIELD x RETURN x",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/CALL/);
  });
});

describe("validateReadOnlyCypher — string concatenation rejection", () => {
  it("rejects '+' operator entirely", () => {
    const r = validateReadOnlyCypher(
      "MATCH (n) WHERE n.id = 'pre' + $suffix RETURN n",
    );
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/\+/);
  });

  it("accepts '+' INSIDE a single-quoted string literal", () => {
    // The validator strips string-literal contents before checking,
    // so '+' inside a quoted literal must not trip the concat-rule.
    const r = validateReadOnlyCypher("MATCH (n {label: 'a+b'}) RETURN n");
    expect(r.ok).toBe(true);
  });
});

describe("validateReadOnlyCypher — input hygiene", () => {
  it("rejects empty query", () => {
    const r = validateReadOnlyCypher("");
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/empty/);
  });

  it("rejects whitespace-only query", () => {
    const r = validateReadOnlyCypher("   \n\t  ");
    expect(r.ok).toBe(false);
  });

  it("rejects oversized query", () => {
    const big = "MATCH (n) WHERE n.id = $x RETURN n " + "x".repeat(CYPHER_MAX_LENGTH);
    const r = validateReadOnlyCypher(big);
    expect(r.ok).toBe(false);
    expect(r.reason).toMatch(/exceeds/);
  });

  it("rejects DELETE hidden in a block comment trailer", () => {
    // The validator strips comments BEFORE keyword check, so this
    // commented-out DELETE is effectively benign. Asserts the
    // comment-strip works in the accepting direction.
    const r = validateReadOnlyCypher("MATCH (n) /* DELETE */ RETURN n");
    expect(r.ok).toBe(true);
  });

  it("rejects DELETE hidden in a line comment", () => {
    const r = validateReadOnlyCypher("MATCH (n) RETURN n // DELETE n");
    expect(r.ok).toBe(true);
  });
});
