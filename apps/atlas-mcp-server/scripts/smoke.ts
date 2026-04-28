#!/usr/bin/env tsx
/**
 * End-to-end smoke for the Atlas MCP server.
 *
 * Drives the same library functions the MCP tools call, then runs the
 * Rust verifier (`atlas-verify-cli`) against the exported bundle and
 * asserts ✓ VALID. This is the single test that closes the loop:
 *
 *   TS canonicalisation + Rust signer + Rust verifier => byte-identical
 *   trust judgment.
 *
 * If this smoke fails, *something* in the determinism pipeline drifted —
 * which is the kind of failure that the byte-pinned goldens in
 * atlas-trust-core were designed to surface BEFORE you get here. So:
 * if the smoke fails, run `cargo test -p atlas-trust-core` first.
 */

import { spawnSync } from "node:child_process";
import { existsSync, promises as fs, rmSync } from "node:fs";
import { join } from "node:path";
import { exportWorkspaceBundle } from "../src/lib/bundle.js";
import { writeSignedEvent } from "../src/lib/event.js";
import { TEST_IDENTITIES } from "../src/lib/keys.js";
import { resolveSignerBinary, workspaceDir } from "../src/lib/paths.js";
import { repoRoot } from "../src/lib/paths.js";
import { anchorBundleTool } from "../src/tools/anchor-bundle.js";

const WORKSPACE = "ws-mcp-smoke";

function log(step: string, msg: string): void {
  process.stdout.write(`[smoke] ${step.padEnd(14)} ${msg}\n`);
}

function fail(msg: string): never {
  process.stderr.write(`[smoke] FAIL ${msg}\n`);
  process.exit(1);
}

async function main(): Promise<void> {
  // 1. Sanity: signer binary is on disk
  const signer = resolveSignerBinary();
  if (!signer) {
    fail(
      "atlas-signer binary not found. Run `cargo build --release -p atlas-signer` " +
        "from the repo root.",
    );
  }
  log("signer", signer);

  // 2. Reset workspace dir for a clean run
  const dir = workspaceDir(WORKSPACE);
  rmSync(dir, { recursive: true, force: true });
  await fs.mkdir(dir, { recursive: true });
  log("workspace", dir);

  // 3. Write three signed events through the MCP write path
  const agent = TEST_IDENTITIES.agent.kid;
  const human = TEST_IDENTITIES.human.kid;

  const ev1 = await writeSignedEvent({
    workspaceId: WORKSPACE,
    kid: agent,
    payload: {
      type: "node.create",
      node: {
        kind: "dataset",
        id: "smoke-dataset",
        rows: 100,
      },
    },
  });
  log("write#1", `${ev1.event.event_id} hash=${ev1.event.event_hash.slice(0, 12)}…`);

  const ev2 = await writeSignedEvent({
    workspaceId: WORKSPACE,
    kid: agent,
    payload: {
      type: "node.create",
      node: {
        kind: "model",
        id: "smoke-model",
        trained_on: "smoke-dataset",
      },
    },
  });
  log("write#2", `${ev2.event.event_id} hash=${ev2.event.event_hash.slice(0, 12)}…`);

  const ev3 = await writeSignedEvent({
    workspaceId: WORKSPACE,
    kid: human,
    payload: {
      type: "annotation.add",
      subject: "smoke-model",
      predicate: "verified_by_human",
      object: { decision: "approved", evidence: "smoke run" },
    },
  });
  log("write#3", `${ev3.event.event_id} hash=${ev3.event.event_hash.slice(0, 12)}…`);

  // 4. Issue mock-Rekor anchors over the current state via the MCP tool.
  //    Mirrors the path Claude Desktop / Cursor would drive. The tool
  //    persists data/{workspace}/anchors.json which exportWorkspaceBundle
  //    reads in step 5.
  const anchorOut = await anchorBundleTool.handler({ workspace_id: WORKSPACE });
  const anchorSummary = JSON.parse(anchorOut.content[0]?.text ?? "{}");
  if (!anchorSummary.ok) fail(`anchor-bundle did not return ok: ${anchorOut.content[0]?.text}`);
  // 1 bundle_hash anchor + 1 dag_tip anchor (single-tip workspace) = 2.
  if (anchorSummary.anchor_count !== 2) {
    fail(`expected 2 anchors (bundle + 1 tip), got ${anchorSummary.anchor_count}`);
  }
  log("anchor", `${anchorSummary.anchor_count} anchor(s) issued, root=${String(anchorSummary.root_hash).slice(0, 12)}…`);

  // 5. Export bundle (now with anchors populated from anchors.json)
  const { trace, bundle } = await exportWorkspaceBundle(WORKSPACE);
  if (trace.events.length !== 3) {
    fail(`expected 3 events in trace, got ${trace.events.length}`);
  }
  if (trace.dag_tips.length !== 1) {
    fail(`expected 1 DAG tip, got ${trace.dag_tips.length}: ${trace.dag_tips.join(",")}`);
  }
  if (trace.dag_tips[0] !== ev3.event.event_hash) {
    fail(`tip mismatch: tip=${trace.dag_tips[0]} expected=${ev3.event.event_hash}`);
  }
  if (trace.anchors.length !== 2) {
    fail(`expected 2 anchors in exported trace, got ${trace.anchors.length}`);
  }
  const tracePath = join(dir, "trace.json");
  const bundlePath = join(dir, "bundle.json");
  await fs.writeFile(tracePath, JSON.stringify(trace, null, 2), "utf8");
  await fs.writeFile(bundlePath, JSON.stringify(bundle, null, 2), "utf8");
  log("export", `${tracePath}`);
  log("export", `${bundlePath}`);

  // 6. Run atlas-verify-cli — the real Rust verifier — on the bundle
  const verifierBin = resolveVerifierBinary();
  if (!verifierBin) {
    fail(
      "atlas-verify-cli binary not found. Run `cargo build --release -p atlas-verify-cli`.",
    );
  }
  log("verify", verifierBin);
  const r = spawnSync(verifierBin, ["verify-trace", tracePath, "-k", bundlePath], {
    encoding: "utf8",
  });
  if (r.error) fail(`verifier spawn failed: ${r.error.message}`);
  process.stdout.write(r.stdout);
  if (r.stderr) process.stderr.write(r.stderr);
  if (r.status !== 0) {
    fail(`atlas-verify-cli exited with status ${r.status}`);
  }
  if (!/✓ VALID|VALID/.test(r.stdout)) {
    fail(`verifier did not report VALID. stdout above.`);
  }
  // Anchor evidence specifically — guards against a regression where the
  // verifier silently passes empty anchors but the smoke believed it was
  // exercising the anchor path.
  if (!/anchor\(s\) verified against pinned log keys/.test(r.stdout)) {
    fail(`verifier did not report anchor evidence. stdout above.`);
  }

  log("done", "✓ end-to-end smoke OK — MCP write+anchor path verifies as VALID");
}

function resolveVerifierBinary(): string | null {
  const exe = process.platform === "win32" ? ".exe" : "";
  const candidates = [
    join(repoRoot(), "target", "release", `atlas-verify-cli${exe}`),
    join(repoRoot(), "target", "debug", `atlas-verify-cli${exe}`),
  ];
  for (const p of candidates) {
    if (existsSync(p)) return p;
  }
  return null;
}

main().catch((e: unknown) => {
  fail(e instanceof Error ? e.stack ?? e.message : String(e));
});
