/**
 * V2-β Welle 12 — POST /api/atlas/query handler tests.
 *
 * The route runs full input validation + Cypher AST validation,
 * then returns 501 on accepted queries (execution backend is W17).
 * We assert each defence layer fires correctly.
 */

import { describe, it, expect, vi } from "vitest";

vi.mock("@/lib/bootstrap", () => ({}));

import { POST } from "./route";

function jsonReq(body: unknown, contentLength?: number): Request {
  const init: RequestInit = {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  };
  if (contentLength !== undefined) {
    init.headers = { ...init.headers, "content-length": String(contentLength) };
  }
  return new Request("http://localhost/api/atlas/query", init);
}

describe("POST /api/atlas/query", () => {
  it("returns 501 with validator=passed on a valid read-only query", async () => {
    const res = await POST(
      jsonReq({ workspace: "ws-test", cypher: "MATCH (n) RETURN n LIMIT 5" }),
    );
    expect(res.status).toBe(501);
    const body = await res.json();
    expect(body.ok).toBe(false);
    expect(body.validator).toBe("passed");
    expect(body.error).toMatch(/Phase 7|W17|ArcadeDB/);
  });

  it("returns 400 when Cypher contains DELETE", async () => {
    const res = await POST(
      jsonReq({ workspace: "ws-test", cypher: "MATCH (n) DELETE n" }),
    );
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error).toMatch(/DELETE/);
  });

  it("returns 400 when Cypher uses apoc.*", async () => {
    const res = await POST(
      jsonReq({ workspace: "ws-test", cypher: "RETURN apoc.text.regex('a', 'b')" }),
    );
    expect(res.status).toBe(400);
  });

  it("returns 400 when Cypher uses + concatenation", async () => {
    const res = await POST(
      jsonReq({
        workspace: "ws-test",
        cypher: "MATCH (n) WHERE n.id = 'a' + $x RETURN n",
      }),
    );
    expect(res.status).toBe(400);
  });

  it("returns 400 when workspace fails the regex", async () => {
    const res = await POST(
      jsonReq({ workspace: "../etc", cypher: "MATCH (n) RETURN n" }),
    );
    expect(res.status).toBe(400);
  });

  it("returns 400 when workspace is missing entirely", async () => {
    const res = await POST(jsonReq({ cypher: "MATCH (n) RETURN n" }));
    expect(res.status).toBe(400);
  });

  it("returns 400 when body is not JSON", async () => {
    const res = await POST(
      new Request("http://localhost/api/atlas/query", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: "not-json-at-all",
      }),
    );
    expect(res.status).toBe(400);
  });

  it("returns 413 when Content-Length exceeds cap", async () => {
    const res = await POST(jsonReq({ workspace: "ws-test", cypher: "x" }, 300_000));
    expect(res.status).toBe(413);
  });

  it("rejects extra fields per Zod strict()", async () => {
    const res = await POST(
      jsonReq({
        workspace: "ws-test",
        cypher: "MATCH (n) RETURN n",
        extra_field: "x",
      }),
    );
    expect(res.status).toBe(400);
  });
});
