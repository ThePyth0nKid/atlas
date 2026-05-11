#!/usr/bin/env tsx
/**
 * V1.19 Welle 1 — atlas-web write-surface end-to-end smoke.
 *
 * Closes the loop the user-visible feature claims:
 *
 *     Form → POST /api/atlas/write-node → events.jsonl → atlas-verify-cli ✓ VALID
 *
 * The script exercises three layers without booting Next.js:
 *
 *   1. Route handler — invokes the exported `POST` from
 *      `src/app/api/atlas/write-node/route.ts` directly with a
 *      `Request`. Proves Zod input validation, payload composition,
 *      and the per-tenant signing flow that any HTTP client would
 *      hit.
 *
 *   2. Storage — reads `data/{workspace}/events.jsonl` back and
 *      asserts the line count + shape match the writes we issued.
 *
 *   3. Verifier — exports a bundle via the MCP server's
 *      `exportWorkspaceBundle` (same on-disk format) and runs
 *      `atlas-verify-cli verify-trace --require-per-tenant-keys`
 *      against it. Asserts ✓ VALID.
 *
 * We intentionally bypass Next.js's HTTP server: the route handler
 * is a pure function `(Request) => Promise<NextResponse>` with no
 * middleware in V1.19, so calling it directly is byte-equivalent to
 * a real fetch — minus a TCP roundtrip we don't need to test. A
 * Playwright-level test that exercises the UI form is queued for
 * V1.19 Welle 2.
 *
 * Why import the MCP exporter rather than write our own? The
 * bundle/trace shape is V1.5–V1.9-stable canonical-CBOR-via-Rust
 * territory; duplicating an exporter here would be drift-prone for
 * zero benefit. atlas-web's bridge writes to the same `events.jsonl`
 * format the MCP exporter already understands, so we just borrow it.
 *
 * Run:
 *   ATLAS_DEV_MASTER_SEED=1 pnpm tsx scripts/e2e-write-roundtrip.ts
 */

import { spawnSync } from "node:child_process";
import { existsSync, promises as fs, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// V1.10 gate: per-tenant signer subcommands require this. Set BEFORE
// the first import that touches the signer (the bridge resolves the
// binary lazily, so this ordering is sufficient as long as it's set
// before `POST` runs — but doing it at the top makes the intent
// obvious and survives a future eager-resolve refactor).
process.env.ATLAS_DEV_MASTER_SEED ??= "1";

// Isolated tmp data dir so the smoke never collides with a developer's
// local workspace state. Both atlas-web's bridge and the MCP exporter
// honour `ATLAS_DATA_DIR`, so a single env var coordinates them.
const TMP_DATA = mkdtempSync(join(tmpdir(), "atlas-web-e2e-"));
process.env.ATLAS_DATA_DIR = TMP_DATA;

const WORKSPACE = "ws-web-e2e";

function log(step: string, msg: string): void {
  process.stdout.write(`[e2e] ${step.padEnd(16)} ${msg}\n`);
}

function fail(msg: string): never {
  process.stderr.write(`[e2e] FAIL ${msg}\n`);
  cleanup();
  process.exit(1);
}

function cleanup(): void {
  try {
    rmSync(TMP_DATA, { recursive: true, force: true });
  } catch {
    // tmp leak is acceptable on cleanup error — the user can rm later.
  }
}

async function main(): Promise<void> {
  log("data-dir", TMP_DATA);

  // 1. Sanity: signer + verifier present (otherwise the test is
  //    asserting on a stale binary location instead of behaviour).
  const { resolveSignerBinary, repoRoot } = await import("@atlas/bridge");
  const signer = resolveSignerBinary();
  if (!signer) {
    fail(
      "atlas-signer binary not found. Run `cargo build --release -p atlas-signer` " +
        "from the repo root.",
    );
  }
  log("signer", signer);
  const verifier = resolveVerifierBinary(repoRoot());
  if (!verifier) {
    fail("atlas-verify-cli binary not found. Run `cargo build --release -p atlas-verify-cli`.");
  }
  log("verifier", verifier);

  // 2. Drive the route handler twice with valid input.
  const { POST, GET } = await import("../src/app/api/atlas/write-node/route");

  // 2a. GET probe — the kid-preview endpoint should not require
  //     a master seed to be set; this guards against an accidental
  //     coupling between the GET path and the signer.
  const probe = await GET(
    new Request(`http://test/api/atlas/write-node?workspace_id=${WORKSPACE}`),
  );
  if (probe.status !== 200) {
    fail(`GET probe returned ${probe.status}: ${await probe.text()}`);
  }
  const probeJson = (await probe.json()) as Record<string, unknown>;
  if (probeJson.kid !== `atlas-anchor:${WORKSPACE}`) {
    fail(`GET probe returned unexpected kid: ${JSON.stringify(probeJson)}`);
  }
  log("GET probe", `kid=${String(probeJson.kid)}`);

  // 2b. Two POSTs. Second one should chain off the first
  //     (parents = [first.event_hash]) — proves the per-workspace
  //     mutex serialised them and that the second one observed the
  //     first's write before computing tips.
  const r1 = await postWriteNode({
    workspace_id: WORKSPACE,
    kind: "dataset",
    id: "web-e2e-corpus",
    attributes: { rows: 42 },
  });
  log("POST#1", `event_hash=${r1.event_hash.slice(0, 12)}…`);

  const r2 = await postWriteNode({
    workspace_id: WORKSPACE,
    kind: "model",
    id: "web-e2e-model",
    attributes: { trained_on: "web-e2e-corpus" },
  });
  log("POST#2", `event_hash=${r2.event_hash.slice(0, 12)}…`);

  if (r2.parents.length !== 1 || r2.parents[0] !== r1.event_hash) {
    fail(
      `POST#2 should chain off POST#1: expected parents=[${r1.event_hash}], ` +
        `got parents=${JSON.stringify(r2.parents)}`,
    );
  }
  log("chain", `POST#2.parents = [POST#1.event_hash] ✓`);

  // 3. Negative path: malformed input must NOT touch the signer.
  const bad = await POST(
    new Request("http://test/api/atlas/write-node", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        workspace_id: "../../etc",
        kind: "dataset",
        id: "x",
        attributes: {},
      }),
    }),
  );
  if (bad.status !== 400) {
    fail(`malformed workspace_id should yield 400, got ${bad.status}`);
  }
  log("input-reject", `path-traversal workspace_id rejected at boundary (400)`);

  // 4. Storage check — events.jsonl must have exactly two lines, each
  //    a structurally valid AtlasEvent (the storage layer's reader
  //    re-validates with Zod, so a bad write would have raised here).
  const { readAllEvents } = await import("@atlas/bridge");
  const stored = await readAllEvents(WORKSPACE);
  if (stored.length !== 2) {
    fail(`expected 2 events on disk, got ${stored.length}`);
  }
  if (stored[0].event_hash !== r1.event_hash) {
    fail(`events.jsonl[0].event_hash mismatch: ${stored[0].event_hash} vs ${r1.event_hash}`);
  }
  log("events.jsonl", `2 events, hashes match POST results`);

  // 5. Bundle export via the MCP server's exporter — same on-disk
  //    format, no duplication. The exporter respects ATLAS_DATA_DIR
  //    so it reads our tmp dir.
  const { exportWorkspaceBundle } = await import(
    "../../atlas-mcp-server/src/lib/bundle"
  );
  const { stringifyAnchorJson } = await import("@atlas/bridge");
  const { trace, bundle } = await exportWorkspaceBundle(WORKSPACE);
  if (trace.events.length !== 2) {
    fail(`exporter saw ${trace.events.length} events; expected 2`);
  }
  if (trace.dag_tips.length !== 1 || trace.dag_tips[0] !== r2.event_hash) {
    fail(
      `expected dag_tips=[${r2.event_hash}], got ${JSON.stringify(trace.dag_tips)}`,
    );
  }
  if (!Object.keys(bundle.keys).includes(`atlas-anchor:${WORKSPACE}`)) {
    fail(`per-tenant kid missing from exported bundle.keys`);
  }

  const wsDir = join(TMP_DATA, WORKSPACE);
  await fs.mkdir(wsDir, { recursive: true });
  const tracePath = join(wsDir, "trace.json");
  const bundlePath = join(wsDir, "bundle.json");
  await fs.writeFile(tracePath, stringifyAnchorJson(trace, 2), "utf8");
  await fs.writeFile(bundlePath, JSON.stringify(bundle, null, 2), "utf8");
  log("export", `${tracePath}`);

  // 6. Verifier — strict mode (--require-per-tenant-keys +
  //    --require-strict-chain) so the smoke fails closed if a future
  //    regression either silently emitted a legacy SPIFFE kid for the
  //    workspace OR produced a sibling-fork DAG instead of the single-
  //    writer linear chain.
  //
  // V1.19 Welle 12 carry-forward (Welle 10 contract symmetry): the
  // atlas-mcp-server smoke turned `--require-strict-chain` on across
  // its three lanes in Welle 10. This e2e is the atlas-web symmetric
  // pair — same gate, same anti-drift assertions, same prose anchor.
  // The 2-event WORKSPACE leg is structurally guaranteed linear by the
  // per-workspace mutex in `writeSignedEvent` (route.ts § "Mutex"); any
  // future regression breaking that serialisation (multi-process
  // writer, broken mutex, race in tip computation) would surface a
  // sibling-fork DAG that the verifier now catches as a structured
  // `TrustError::StrictChainViolation` instead of silently passing as
  // "valid DAG".
  //
  // Anti-drift assertions mirror smoke.ts (apps/atlas-mcp-server):
  //   - evidence-row pin: leading "✓" distinguishes happy-path from
  //     "✗ strict-chain — strict chain violation: ..."; count + prose
  //     pin the print_human evidence rendering.
  //   - flag-name pin: anchored to "Strict flags:" prefix to prevent
  //     vacuous pass if the bare identifier "require_strict_chain"
  //     appears elsewhere in stdout (e.g. echoed in a future clap
  //     error body). Order-tolerant `[^\n]*` between prefix and
  //     identifier reflects that `flags.join(", ")` ordering is not
  //     stable across re-orderings of the flags vec in print_human.
  //
  // Structural note: smoke.ts step-6 runs ONLY `--require-strict-chain`
  // (one flag, one identifier pin). This e2e is the step-7 analogue:
  // both `--require-per-tenant-keys` (Welle 1) and `--require-strict-
  // chain` (Welle 12) are active, so the `Strict flags:` line carries
  // both identifiers in either order. The two flag-name pins below
  // mirror smoke.ts step-7's two-flag block exactly; the apparent
  // asymmetry with step-6 is intentional — different lanes test
  // different combined-flag surfaces.
  const r = spawnSync(
    verifier,
    [
      "verify-trace",
      tracePath,
      "-k",
      bundlePath,
      "--require-per-tenant-keys",
      "--require-strict-chain",
    ],
    { encoding: "utf8" },
  );
  if (r.error) fail(`verifier spawn failed: ${r.error.message}`);
  process.stdout.write(r.stdout);
  if (r.stderr) process.stderr.write(r.stderr);
  if (r.status !== 0) fail(`atlas-verify-cli exited with status ${r.status}`);
  if (!/✓ VALID|VALID/.test(r.stdout)) {
    fail(`verifier did not report VALID. stdout above.`);
  }
  if (!/strict mode/.test(r.stdout)) {
    fail(`strict-mode advertisement missing — verifier may be running lenient`);
  }
  if (!/✓ strict-chain — \d+ event\(s\) form a strict linear chain/.test(r.stdout)) {
    fail(`strict-chain evidence row missing — Welle 12 anti-drift assertion`);
  }
  if (!/Strict flags:[^\n]*require_strict_chain/.test(r.stdout)) {
    fail(`Strict flags line missing 'require_strict_chain' — Welle 12 anti-drift assertion`);
  }
  if (!/Strict flags:[^\n]*require_per_tenant_keys/.test(r.stdout)) {
    fail(`Strict flags line missing 'require_per_tenant_keys' — Welle 1 anti-drift assertion`);
  }
  log("verify", `✓ VALID (strict per-tenant + strict-chain)`);

  cleanup();
  log("done", "✓ atlas-web write-surface round-trip OK");
}

async function postWriteNode(input: {
  workspace_id: string;
  kind: string;
  id: string;
  attributes: Record<string, unknown>;
}): Promise<{
  event_id: string;
  event_hash: string;
  parents: string[];
  kid: string;
  workspace_id: string;
}> {
  const { POST } = await import("../src/app/api/atlas/write-node/route");
  const res = await POST(
    new Request("http://test/api/atlas/write-node", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(input),
    }),
  );
  const body = (await res.json()) as Record<string, unknown>;
  if (res.status !== 200 || body.ok !== true) {
    fail(`POST failed (${res.status}): ${JSON.stringify(body)}`);
  }
  return body as {
    event_id: string;
    event_hash: string;
    parents: string[];
    kid: string;
    workspace_id: string;
  } & { ok: true };
}

function resolveVerifierBinary(repo: string): string | null {
  const exe = process.platform === "win32" ? ".exe" : "";
  const candidates = [
    join(repo, "target", "release", `atlas-verify-cli${exe}`),
    join(repo, "target", "debug", `atlas-verify-cli${exe}`),
  ];
  for (const p of candidates) {
    if (existsSync(p)) return p;
  }
  return null;
}

main().catch((e: unknown) => {
  fail(e instanceof Error ? e.stack ?? e.message : String(e));
});
