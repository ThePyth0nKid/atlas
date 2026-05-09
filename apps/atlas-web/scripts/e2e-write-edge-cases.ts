#!/usr/bin/env tsx
/**
 * V1.19 Welle 8 — atlas-web write-surface HTTP-level edge case coverage.
 *
 * Covers the four edge-case classes that `e2e-write-roundtrip.ts`
 * does not exercise:
 *
 *   [A] 4xx paths — malformed JSON, Zod validation failures
 *       (workspace_id regex, kind enum, missing/extra fields,
 *       attributes > 64 KB, id length boundaries).
 *
 *   [B] Content-Length cap — `REQUEST_BODY_MAX_BYTES = 256 KB` at
 *       the HTTP boundary BEFORE `req.json()` reads the stream.
 *
 *   [C] Concurrent writes — N parallel POSTs against the same
 *       workspace; per-workspace mutex must serialise them so each
 *       event chains off the prior one's hash and on-disk count
 *       equals N (no lost or duplicated writes).
 *
 *   [D] workspace_id boundary class — path-traversal tokens
 *       (`../etc`, `..\\etc`, embedded `/`), char-class violations
 *       (`foo bar`, `foo!`), length boundaries (0, 1, 128, 129).
 *
 * Why a separate file from `e2e-write-roundtrip.ts`: the round-trip
 * smoke proves the happy path end-to-end (form → POST → verifier
 * VALID); this file proves the failure-mode contract that downstream
 * operators rely on (every 4xx is a structured error, never a
 * sign/storage side-effect; concurrent traffic does not corrupt the
 * DAG). Splitting them keeps each test fast and the failure mode
 * obvious — round-trip turning red means "the product broke", edge
 * cases turning red means "a security/contract regression".
 *
 * Why HTTP-level (Request) instead of Playwright UI: every edge case
 * here is server-side. The route handler is a pure function
 * `(Request) => Promise<NextResponse>`, so calling it directly is
 * byte-equivalent to a real fetch minus a TCP roundtrip we don't
 * need to test. Playwright would add ~300 MB of browser binaries to
 * CI for zero additional coverage of these scenarios. UI-level
 * coverage of the React form's error rendering is queued as a
 * separate Welle 9 candidate (different concern: form rendering,
 * a11y, error UI — not security boundary).
 *
 * Run:
 *   ATLAS_DEV_MASTER_SEED=1 pnpm tsx scripts/e2e-write-edge-cases.ts
 */

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// V1.10 gate: per-tenant signer subcommands require this. Set BEFORE
// the first import that touches the signer.
process.env.ATLAS_DEV_MASTER_SEED ??= "1";

// Isolated tmp data dir so the test never collides with developer
// state. Both atlas-web's bridge and the MCP exporter honour
// ATLAS_DATA_DIR.
const TMP_DATA = mkdtempSync(join(tmpdir(), "atlas-web-edge-"));
process.env.ATLAS_DATA_DIR = TMP_DATA;

let assertions = 0;
let failures = 0;

function check(name: string, ok: boolean, detail?: string): void {
  assertions += 1;
  if (ok) {
    process.stdout.write(`  ✓ ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(`  ✗ ${name}${detail ? ` — ${detail}` : ""}\n`);
  }
}

function section(name: string): void {
  process.stdout.write(`\n[${name}]\n`);
}

function cleanup(): void {
  try {
    rmSync(TMP_DATA, { recursive: true, force: true });
  } catch {
    // tmp leak acceptable; user can rm later.
  }
}

type RouteModule = typeof import("../src/app/api/atlas/write-node/route");

function makeRequest(body: BodyInit | null, headers: Record<string, string> = {}): Request {
  return new Request("http://test/api/atlas/write-node", {
    method: "POST",
    headers: { "content-type": "application/json", ...headers },
    body,
  });
}

async function bodyJson(res: Response): Promise<Record<string, unknown>> {
  return (await res.json()) as Record<string, unknown>;
}

async function main(): Promise<void> {
  process.stdout.write(`[edge-cases] data-dir ${TMP_DATA}\n`);

  // Sanity: signer present (otherwise concurrent-write section asserts
  // on stale binary location, not behaviour).
  const { resolveSignerBinary } = await import("@atlas/bridge");
  if (!resolveSignerBinary()) {
    process.stderr.write(
      "[edge-cases] FAIL atlas-signer binary not found. " +
        "Run `cargo build --release -p atlas-signer` from repo root.\n",
    );
    cleanup();
    process.exit(1);
  }

  const route: RouteModule = await import("../src/app/api/atlas/write-node/route");
  // Shared-identity import of the production cap — no local literal,
  // no source/test drift. (V1.19 Welle 7 pattern: anti-drift via
  // frozen `__*_FOR_TEST` re-export.)
  const REQUEST_BODY_MAX_BYTES = route.__REQUEST_BODY_MAX_BYTES_FOR_TEST;

  // --------------------------------------------------------------- [A]
  section("A: 4xx malformed-input rejections");

  // [A.1] Body is not valid JSON (truncated object).
  {
    const res = await route.POST(makeRequest('{"workspace_id":"ws-edge-a"'));
    check("malformed JSON → 400", res.status === 400, `got ${res.status}`);
    const body = await bodyJson(res);
    check(
      "malformed JSON: ok=false + error mentions JSON",
      body.ok === false && typeof body.error === "string" && /JSON/i.test(body.error as string),
      JSON.stringify(body),
    );
  }

  // [A.2] Body is empty.
  {
    const res = await route.POST(makeRequest(""));
    check("empty body → 400", res.status === 400, `got ${res.status}`);
  }

  // [A.3] Missing required field `kind`.
  {
    const res = await route.POST(
      makeRequest(JSON.stringify({ workspace_id: "ws-edge-a", id: "x", attributes: {} })),
    );
    check("missing kind → 400", res.status === 400, `got ${res.status}`);
  }

  // [A.4] Unknown kind enum value.
  {
    const res = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "ws-edge-a",
          kind: "bogus-kind",
          id: "x",
          attributes: {},
        }),
      ),
    );
    check("unknown kind → 400", res.status === 400, `got ${res.status}`);
  }

  // [A.5] Strict-mode: extra unknown key at top level.
  {
    const res = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "ws-edge-a",
          kind: "dataset",
          id: "x",
          attributes: {},
          extra_field: "not-allowed",
        }),
      ),
    );
    check("extra top-level field (.strict()) → 400", res.status === 400, `got ${res.status}`);
  }

  // [A.6] attributes > 64 KB JSON-serialised → Zod refine rejects.
  {
    // 80 KB string is well above ATTRIBUTES_MAX_BYTES (64 KB).
    const big = "x".repeat(80 * 1024);
    const res = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "ws-edge-a",
          kind: "dataset",
          id: "big-attrs",
          attributes: { payload: big },
        }),
      ),
    );
    check("attributes > 64 KB → 400", res.status === 400, `got ${res.status}`);
    const body = await bodyJson(res);
    check(
      "attributes-too-big error mentions 65536-byte cap",
      body.ok === false &&
        typeof body.error === "string" &&
        /65536/.test(body.error as string),
      JSON.stringify(body).slice(0, 200),
    );
  }

  // [A.7] id too long (> 256 chars) → Zod max rejects.
  {
    const res = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "ws-edge-a",
          kind: "dataset",
          id: "x".repeat(257),
          attributes: {},
        }),
      ),
    );
    check("id > 256 chars → 400", res.status === 400, `got ${res.status}`);
  }

  // [A.8] id empty string → Zod min rejects.
  {
    const res = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "ws-edge-a",
          kind: "dataset",
          id: "",
          attributes: {},
        }),
      ),
    );
    check("id empty → 400", res.status === 400, `got ${res.status}`);
  }

  // [A.9] id at-boundary happy path: exactly 256 chars (max) and
  //       exactly 1 char (min) must be accepted. Locks the inclusive
  //       boundary so a future `.max(255)` typo turns red.
  {
    const res256 = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "ws-edge-a",
          kind: "dataset",
          id: "x".repeat(256),
          attributes: {},
        }),
      ),
    );
    check("id 256 chars (boundary max) → 200", res256.status === 200, `got ${res256.status}`);

    const res1 = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "ws-edge-a",
          kind: "dataset",
          id: "y",
          attributes: {},
        }),
      ),
    );
    check("id 1 char (boundary min) → 200", res1.status === 200, `got ${res1.status}`);
  }

  // [A.10] Prototype-pollution contract: an attribute literally named
  //        `__proto__` MUST NOT mutate `Object.prototype`. Modern
  //        `JSON.parse` already treats `__proto__` as an own property
  //        rather than the prototype slot, but Zod's
  //        `z.record(z.string(), z.unknown())` is a passthrough on key
  //        names, so the route's safety relies entirely on JSON.parse
  //        + structured cloning behaviour. This test pins that
  //        contract — if a future refactor swaps the parser or merges
  //        attributes via `Object.assign({}, attrs)` (which DOES walk
  //        `__proto__`), the assertion turns red.
  {
    const beforePolluted = (Object.prototype as Record<string, unknown>)["polluted"];
    const res = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "ws-edge-a",
          kind: "dataset",
          id: "proto-test",
          attributes: { __proto__: { polluted: "yes" } },
        }),
      ),
    );
    const afterPolluted = (Object.prototype as Record<string, unknown>)["polluted"];
    check(
      "__proto__ in attributes did not pollute Object.prototype",
      afterPolluted === beforePolluted,
      `Object.prototype.polluted = ${String(afterPolluted)}`,
    );
    check(
      "__proto__ request returned a structured response (not a crash)",
      res.status >= 200 && res.status < 600 && (await bodyJson(res)).ok !== undefined,
      `got ${res.status}`,
    );
  }

  // [A.11] Deeply-nested attributes — small in bytes, deep in
  //        structure. Atlas-signer (serde_json) defaults to a 128-
  //        level recursion limit; the route must surface this as a
  //        structured response (200 if accepted upstream, 4xx/5xx
  //        with `ok:false` body otherwise) rather than crash the
  //        Node process. ~200 levels is well over serde_json's cap
  //        but ~few KB total — well under the 64 KB attributes cap
  //        and 256 KB body cap, so this exercises depth, not size.
  {
    const DEPTH = 200;
    let nested: Record<string, unknown> = {};
    let cursor = nested;
    for (let i = 0; i < DEPTH; i++) {
      const next: Record<string, unknown> = {};
      cursor.a = next;
      cursor = next;
    }
    const res = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "ws-edge-a",
          kind: "dataset",
          id: "deep-nest",
          attributes: { tree: nested },
        }),
      ),
    );
    const body = await bodyJson(res);
    check(
      `deeply-nested attributes (${DEPTH} levels) → structured response`,
      res.status >= 200 && res.status < 600 && typeof body.ok === "boolean",
      `status=${res.status} body=${JSON.stringify(body).slice(0, 120)}`,
    );
  }

  // --------------------------------------------------------------- [B]
  section("B: Content-Length 256 KB cap (returns 413, never reads body)");

  // [B.1] Content-Length header > 256 KB → 413 BEFORE body parsed.
  // Real body is small but we lie about its size — proves the check
  // runs on the header, not on the actual stream length. (Production
  // clients lying about Content-Length is unusual; the contract is
  // that the header is advisory but cheap to reject early.)
  {
    const fakeBig = String(REQUEST_BODY_MAX_BYTES + 1);
    const res = await route.POST(
      makeRequest(JSON.stringify({ workspace_id: "ws-edge-b", kind: "dataset", id: "x" }), {
        "content-length": fakeBig,
      }),
    );
    check(
      "Content-Length > 256 KB → 413",
      res.status === 413,
      `got ${res.status}`,
    );
    const body = await bodyJson(res);
    check(
      "413 error mentions byte cap",
      body.ok === false &&
        typeof body.error === "string" &&
        /262144|256/.test(body.error as string),
      JSON.stringify(body),
    );
  }

  // [B.2] Content-Length exactly at cap → passes the header check
  //       (the cap is `>`, not `>=`). Proves the boundary semantics:
  //       cap-N bytes is allowed, cap+1 is not. The body itself is a
  //       valid minimal write, so the request proceeds past the cap
  //       check and (with `attributes` defaulting to `{}` via Zod)
  //       lands a real event — the assertion intentionally only
  //       checks `!== 413` because the contract under test is the
  //       cap, not the downstream success path.
  {
    const res = await route.POST(
      makeRequest(JSON.stringify({ workspace_id: "ws-edge-b", kind: "dataset", id: "x" }), {
        "content-length": String(REQUEST_BODY_MAX_BYTES),
      }),
    );
    check(
      "Content-Length === cap → not 413 (allowed)",
      res.status !== 413,
      `got ${res.status}`,
    );
  }

  // [B.3] Content-Length non-numeric → header ignored, request proceeds.
  //       (`Number("abc")` is NaN, `Number.isFinite(NaN)` is false.)
  {
    const res = await route.POST(
      makeRequest(JSON.stringify({ workspace_id: "ws-edge-b", kind: "dataset", id: "x", attributes: {} }), {
        "content-length": "not-a-number",
      }),
    );
    check(
      "Content-Length non-numeric → not 413 (header ignored)",
      res.status !== 413,
      `got ${res.status}`,
    );
  }

  // [B.4] Missing Content-Length → header check skipped, request proceeds.
  {
    const req = new Request("http://test/api/atlas/write-node", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        workspace_id: "ws-edge-b",
        kind: "dataset",
        id: "no-cl-header",
        attributes: {},
      }),
    });
    // Force-remove content-length if Request added it.
    req.headers.delete("content-length");
    const res = await route.POST(req);
    check("missing Content-Length → not 413", res.status !== 413, `got ${res.status}`);
  }

  // --------------------------------------------------------------- [C]
  section("C: Concurrent writes serialised by per-workspace mutex");

  // [C.1] N parallel POSTs against same workspace must:
  //       - all return 200
  //       - produce N unique event_hashes
  //       - chain forms a strict total order (each event's parents
  //         points to the previous in append order)
  //       - on-disk events.jsonl has exactly N lines
  {
    const WORKSPACE = "ws-edge-c";
    const N = 8;

    const promises = Array.from({ length: N }, (_, i) =>
      route.POST(
        makeRequest(
          JSON.stringify({
            workspace_id: WORKSPACE,
            kind: "dataset",
            id: `concurrent-${i}`,
            attributes: { idx: i },
          }),
        ),
      ),
    );
    const responses = await Promise.all(promises);

    const allOk = responses.every((r) => r.status === 200);
    check(`all ${N} concurrent POSTs returned 200`, allOk);

    const bodies = await Promise.all(responses.map((r) => bodyJson(r)));
    const hashes = bodies.map((b) => b.event_hash as string);
    const uniqueHashes = new Set(hashes);
    check(`all ${N} event_hashes unique (no lost writes)`, uniqueHashes.size === N, `${uniqueHashes.size}/${N}`);

    // Read back from disk in stored order. Per-workspace mutex
    // guarantees mutual exclusion on the append; the storage order is
    // the linearisation. Each event after the first must reference
    // exactly one parent that appeared earlier in the stored sequence.
    const { readAllEvents } = await import("@atlas/bridge");
    const stored = await readAllEvents(WORKSPACE);
    check(`events.jsonl has exactly ${N} lines`, stored.length === N, `got ${stored.length}`);

    const storedHashes = stored.map((e) => e.event_hash);
    const hashSet = new Set(storedHashes);
    check(
      "every POST hash also appears on disk",
      hashes.every((h) => hashSet.has(h)),
    );

    // Genesis check: the first stored event under mutex serialisation
    // is the workspace's genesis — `parent_hashes` MUST be empty.
    // Asserting this separately means a regression that lets a fork
    // appear at index 0 (e.g. picking up tips from a sibling
    // workspace) turns red here, not silently inside the loop below.
    check(
      "first stored event is genesis (parent_hashes empty)",
      stored[0].parent_hashes.length === 0,
      `got [${stored[0].parent_hashes.join(", ")}]`,
    );

    // Strict linear chain: under mutex serialisation, each event after
    // the first MUST chain to the IMMEDIATE predecessor in the stored
    // sequence — `parents[0] === stored[i-1].event_hash`. Set
    // membership ("any earlier hash") would silently accept a sibling-
    // fork DAG (event_i parents=event_0, event_{i+1} also parents=
    // event_0), which is exactly the regression mode this test is
    // meant to catch (security-reviewer FINDING-6).
    let chainValid = true;
    let chainFailureDetail = "";
    for (let i = 1; i < stored.length; i++) {
      const event = stored[i];
      const parents = event.parent_hashes;
      if (parents.length !== 1 || parents[0] !== storedHashes[i - 1]) {
        chainValid = false;
        chainFailureDetail = `entry ${i}: parents=[${parents.join(", ")}] expected [${storedHashes[i - 1]}]`;
        break;
      }
    }
    check(
      "strict linear chain (each parents[0] === stored[i-1].event_hash)",
      chainValid,
      chainFailureDetail,
    );
  }

  // --------------------------------------------------------------- [D]
  section("D: workspace_id boundary class (path-traversal + char-class)");

  const badIds: Array<[string, string]> = [
    ["../../etc", "POSIX path-traversal"],
    ["..\\..\\etc", "Windows path-traversal"],
    ["foo/bar", "embedded slash"],
    ["foo\\bar", "embedded backslash"],
    ["foo bar", "embedded space"],
    ["foo!", "non-allowed punctuation"],
    ["foo.bar", "embedded dot"],
    ["", "empty string"],
    ["a".repeat(129), "129 chars (one over max)"],
    [".", "single dot"],
    ["..", "double dot"],
  ];

  for (const [id, desc] of badIds) {
    const res = await route.POST(
      makeRequest(
        JSON.stringify({ workspace_id: id, kind: "dataset", id: "x", attributes: {} }),
      ),
    );
    check(`workspace_id "${desc}" → 400`, res.status === 400, `got ${res.status}`);
  }

  // [D.boundary] exactly 128 chars (max allowed) and 1 char (min) must succeed.
  {
    const max128 = "a".repeat(128);
    const res128 = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: max128,
          kind: "dataset",
          id: "boundary-max",
          attributes: {},
        }),
      ),
    );
    check("workspace_id 128 chars (boundary) → 200", res128.status === 200, `got ${res128.status}`);

    const res1 = await route.POST(
      makeRequest(
        JSON.stringify({
          workspace_id: "a",
          kind: "dataset",
          id: "boundary-min",
          attributes: {},
        }),
      ),
    );
    check("workspace_id 1 char (boundary) → 200", res1.status === 200, `got ${res1.status}`);
  }

  // [D.GET] GET path mirrors the same workspace_id validation contract.
  {
    const noParam = await route.GET(new Request("http://test/api/atlas/write-node"));
    check("GET without workspace_id → 400", noParam.status === 400, `got ${noParam.status}`);

    const traversal = await route.GET(
      new Request("http://test/api/atlas/write-node?workspace_id=../etc"),
    );
    check("GET with traversal workspace_id → 400", traversal.status === 400, `got ${traversal.status}`);

    const valid = await route.GET(
      new Request("http://test/api/atlas/write-node?workspace_id=ws-edge-d"),
    );
    check("GET with valid workspace_id → 200", valid.status === 200, `got ${valid.status}`);
  }

  // --------------------------------------------------------------- end
  process.stdout.write(
    `\n[edge-cases] ${assertions} assertion(s) total, ${failures} failure(s).\n`,
  );

  cleanup();

  if (failures > 0) {
    process.exit(1);
  }
}

main().catch((e: unknown) => {
  process.stderr.write(
    `[edge-cases] FAIL ${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`,
  );
  cleanup();
  process.exit(1);
});
