#!/usr/bin/env tsx
/**
 * Handler tests for V2-β Welle 13 MCP V2 read-side tools.
 *
 * Coverage:
 *   - Tool registry includes all 5 new tools and length grew by 5
 *   - `atlas_query_graph` happy path (fake projection-store returns rows)
 *   - `atlas_query_graph` Cypher validator rejection
 *   - `atlas_query_graph` missing required param
 *   - `atlas_query_entities` happy path + limit cap rejection
 *   - `atlas_query_provenance` happy path + invalid UUID rejection
 *   - `atlas_get_agent_passport` STUB shape
 *   - `atlas_get_agent_passport` invalid DID rejection
 *   - `atlas_get_timeline` happy path + reversed-window rejection
 *
 * Each handler returns `{ content: [{ type: "text", text }], isError? }`.
 * Tests parse `content[0].text` as JSON and assert on the structured
 * envelope.
 */

import { TOOL_REGISTRY } from "../src/tools/index.js";
import {
  resetProjectionStore,
  setProjectionStore,
  type CypherResultRow,
  type ProjectionEntity,
  type ProvenanceEntry,
  type TimelineEntry,
} from "../src/tools/_lib/projection-store.js";

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

function findTool(name: string) {
  const t = TOOL_REGISTRY.find((entry) => entry.name === name);
  if (!t) throw new Error(`tool not found in registry: ${name}`);
  return t;
}

function parseEnvelope(text: string): Record<string, unknown> {
  return JSON.parse(text) as Record<string, unknown>;
}

// ─── Registry sanity ───────────────────────────────────────────────────
check(
  "TOOL_REGISTRY contains at least the 5 V2 tools + 5 V1 tools",
  TOOL_REGISTRY.length >= 10,
  `got ${TOOL_REGISTRY.length}`,
);

const expectedNames = [
  "atlas_query_graph",
  "atlas_query_entities",
  "atlas_query_provenance",
  "atlas_get_agent_passport",
  "atlas_get_timeline",
];
for (const name of expectedNames) {
  check(
    `registry has ${name}`,
    TOOL_REGISTRY.some((t) => t.name === name),
  );
}

// ─── Fake projection-store for happy-path tests ───────────────────────
const FAKE_ROW: CypherResultRow = { n: { entity_uuid: "abc", kind: "dataset" } };
const FAKE_ENTITY: ProjectionEntity = {
  entity_uuid: "11111111-1111-1111-1111-111111111111",
  kind: "dataset",
  attributes: { name: "demo" },
};
const FAKE_PROV: ProvenanceEntry = {
  event_id: "evt-1",
  event_hash: "h".repeat(64),
  ts: "2026-05-13T10:00:00+00:00",
  kid: "did:key:zABC",
  type: "node.create",
};
const FAKE_TL: TimelineEntry = {
  event_id: "evt-2",
  event_hash: "k".repeat(64),
  ts: "2026-05-13T11:00:00+00:00",
  kid: "did:key:zABC",
  type: "annotation.add",
};

function installFakeStore() {
  setProjectionStore({
    async runCypher() {
      return [FAKE_ROW];
    },
    async listEntities() {
      return [FAKE_ENTITY];
    },
    async provenance() {
      return [FAKE_PROV];
    },
    async timeline() {
      return [FAKE_TL];
    },
  });
}

// ─── query_graph ───────────────────────────────────────────────────────
{
  installFakeStore();
  const tool = findTool("atlas_query_graph");
  const res = await tool.handler({
    workspace_id: "wsp-test",
    cypher: "MATCH (n) RETURN n LIMIT 1",
    params: {},
  });
  const env = parseEnvelope(res.content[0].text);
  check("query_graph happy ok", env.ok === true);
  check("query_graph happy row_count=1", env.row_count === 1);
  resetProjectionStore();
}

{
  // Validator rejection — DELETE is forbidden.
  const tool = findTool("atlas_query_graph");
  const res = await tool.handler({
    workspace_id: "wsp-test",
    cypher: "MATCH (n) DELETE n",
    params: {},
  });
  check("query_graph DELETE -> isError=true", res.isError === true);
  const env = parseEnvelope(res.content[0].text);
  check(
    "query_graph DELETE envelope ok=false",
    env.ok === false && typeof env.error === "string" && env.error.toLowerCase().includes("delete"),
    `error=${env.error}`,
  );
}

{
  // Missing required `cypher` param.
  const tool = findTool("atlas_query_graph");
  const res = await tool.handler({ workspace_id: "wsp-test" });
  check("query_graph missing cypher -> isError=true", res.isError === true);
  const env = parseEnvelope(res.content[0].text);
  check("query_graph missing cypher envelope ok=false", env.ok === false);
}

// ─── query_entities ────────────────────────────────────────────────────
{
  installFakeStore();
  const tool = findTool("atlas_query_entities");
  const res = await tool.handler({
    workspace_id: "wsp-test",
    kind: "dataset",
    limit: 10,
  });
  const env = parseEnvelope(res.content[0].text);
  check("query_entities happy ok", env.ok === true);
  check("query_entities happy count=1", env.count === 1);
  check("query_entities happy kind echoed", env.kind === "dataset");
  resetProjectionStore();
}

{
  // limit > 500 -> Zod rejection
  const tool = findTool("atlas_query_entities");
  const res = await tool.handler({ workspace_id: "wsp-test", limit: 501 });
  check("query_entities limit>500 -> isError=true", res.isError === true);
}

// ─── query_provenance ──────────────────────────────────────────────────
{
  installFakeStore();
  const tool = findTool("atlas_query_provenance");
  const res = await tool.handler({
    workspace_id: "wsp-test",
    entity_uuid: "11111111-1111-1111-1111-111111111111",
  });
  const env = parseEnvelope(res.content[0].text);
  check("query_provenance happy ok", env.ok === true);
  check("query_provenance happy event_count=1", env.event_count === 1);
  resetProjectionStore();
}

{
  // Invalid UUID
  const tool = findTool("atlas_query_provenance");
  const res = await tool.handler({
    workspace_id: "wsp-test",
    entity_uuid: "not-a-uuid",
  });
  check("query_provenance invalid uuid -> isError=true", res.isError === true);
}

// ─── get_agent_passport (STUB) ─────────────────────────────────────────
{
  const tool = findTool("atlas_get_agent_passport");
  const res = await tool.handler({ agent_did: "did:key:z6MkiABCDEF" });
  const env = parseEnvelope(res.content[0].text);
  check("get_agent_passport ok=true (stub)", env.ok === true);
  check("get_agent_passport status='stub'", env.status === "stub");
  check(
    "get_agent_passport message references V2-γ",
    typeof env.message === "string" && env.message.toLowerCase().includes("v2-γ"),
    `message=${env.message}`,
  );
  check(
    "get_agent_passport echoes agent_did",
    env.agent_did === "did:key:z6MkiABCDEF",
  );
}

{
  // Invalid DID syntax
  const tool = findTool("atlas_get_agent_passport");
  const res = await tool.handler({ agent_did: "not-a-did" });
  check("get_agent_passport invalid DID -> isError=true", res.isError === true);
}

// ─── get_timeline ──────────────────────────────────────────────────────
{
  installFakeStore();
  const tool = findTool("atlas_get_timeline");
  const res = await tool.handler({
    workspace_id: "wsp-test",
    from: "2026-05-13T00:00:00+00:00",
    to: "2026-05-13T23:59:59+00:00",
    limit: 50,
  });
  const env = parseEnvelope(res.content[0].text);
  check("get_timeline happy ok", env.ok === true);
  check("get_timeline happy count=1", env.count === 1);
  resetProjectionStore();
}

{
  // Reversed window — from > to
  const tool = findTool("atlas_get_timeline");
  const res = await tool.handler({
    workspace_id: "wsp-test",
    from: "2026-05-13T23:59:59+00:00",
    to: "2026-05-13T00:00:00+00:00",
  });
  check("get_timeline reversed window -> isError=true", res.isError === true);
  const env = parseEnvelope(res.content[0].text);
  check(
    "get_timeline reversed window message refs from/to",
    typeof env.error === "string" && env.error.includes("from"),
  );
}

// ─── Default stub throws when no fake installed ────────────────────────
{
  resetProjectionStore();
  const tool = findTool("atlas_query_entities");
  const res = await tool.handler({ workspace_id: "wsp-test" });
  check("query_entities w/ default stub -> isError=true", res.isError === true);
  const env = parseEnvelope(res.content[0].text);
  check(
    "stub error message references W17",
    typeof env.error === "string" && env.error.toUpperCase().includes("W17"),
    `error=${env.error}`,
  );
}

if (failures > 0) {
  process.stderr.write(`\n${failures} test(s) failed.\n`);
  process.exit(1);
}
process.stdout.write(`\nall MCP v2 tool handler tests passed\n`);
