/**
 * V2-β Welle 12 — GET /api/atlas/passport/[agent_did] handler tests.
 *
 * The full Agent Passport implementation is V2-γ; W12 ships a 501
 * stub. These tests assert the contract shape so V2-γ can extend
 * (and v2.0.0-beta.1 clients can rely on the 501 + body envelope).
 */

import { describe, it, expect } from "vitest";

import { GET } from "./route";

describe("GET /api/atlas/passport/[agent_did]", () => {
  it("returns 501 with stub body shape", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/passport/did:atlas:foo"),
      { params: Promise.resolve({ agent_did: "did:atlas:foo" }) },
    );
    expect(res.status).toBe(501);
    const body = await res.json();
    expect(body.ok).toBe(false);
    expect(body.status).toBe("stub");
    expect(body.agent_did).toBe("did:atlas:foo");
    expect(body.message).toMatch(/V2-γ|V2-gamma|Identity/i);
  });

  it("echoes a different agent_did verbatim", async () => {
    const res = await GET(
      new Request("http://localhost/api/atlas/passport/did:atlas:bar"),
      { params: Promise.resolve({ agent_did: "did:atlas:bar" }) },
    );
    const body = await res.json();
    expect(body.agent_did).toBe("did:atlas:bar");
  });
});
