/**
 * Build a shippable AtlasTrace + matching PubkeyBundle from a workspace's
 * events.jsonl.
 *
 * The bundle is what the auditor receives. After this function returns,
 * the auditor can run `atlas-verify-cli verify-trace trace.json -k bundle.json`
 * with no further server interaction and reach the same ✓ VALID outcome
 * as anyone else.
 *
 * The pubkey-bundle hash is computed by shelling out to
 * `atlas-signer bundle-hash`, which is the *single* canonicalisation
 * source — see `signer.ts::bundleHashViaSigner`. There is no parallel
 * TypeScript canonicaliser. Drift between TS and Rust is structurally
 * impossible because TS never owns canonical-JSON formatting.
 */

import { promises as fs } from "node:fs";
import { basename } from "node:path";
import { parseAnchorJson } from "./anchor-json.js";
import { buildBundleForWorkspace } from "./keys.js";
import { anchorChainPath, anchorsPath } from "./paths.js";
import { AnchorEntryArraySchema } from "./schema.js";
import { bundleHashViaSigner, chainExportViaSigner } from "./signer.js";
import { readAllEvents, computeTips } from "./storage.js";
import {
  SCHEMA_VERSION,
  type AnchorChain,
  type AnchorEntry,
  type AtlasEvent,
  type AtlasTrace,
  type PubkeyBundle,
} from "./types.js";

export type ExportedBundle = {
  trace: AtlasTrace;
  bundle: PubkeyBundle;
};

/**
 * Assemble a bundle for the given workspace from on-disk events.
 *
 * If `data/{workspace}/anchors.json` exists (written by the
 * `atlas_anchor_bundle` tool), its entries populate `trace.anchors`;
 * otherwise the bundle ships with an empty anchors list and the verifier
 * passes the lenient default. Stale anchors (which no longer match the
 * current `pubkey_bundle_hash` or any current `dag_tips`) are still
 * shipped — the verifier rejects them with a precise reason rather than
 * the server silently filtering. That keeps the trust property
 * explicit: the server cannot quietly drop inconvenient evidence.
 *
 * If `data/{workspace}/anchor-chain.jsonl` exists and is non-empty
 * (written by the V1.7 issuer-side chain extension), it is read,
 * validated through the Rust signer's `chain-export` subcommand, and
 * embedded as `trace.anchor_chain`. Absence is benign — V1.5/V1.6
 * traces continue to round-trip through the lenient verifier. A
 * present-but-corrupt chain fails the export loudly so the operator
 * notices before the bundle reaches an auditor.
 */
export async function exportWorkspaceBundle(workspaceId: string): Promise<ExportedBundle> {
  const events: AtlasEvent[] = await readAllEvents(workspaceId);
  // V1.9: bundle pins the legacy kids AND the per-tenant kid for this
  // workspace, so events signed under either path verify. The bundle
  // hash therefore varies per workspace — this is intentional: the
  // verifier recomputes the hash of the bundle the auditor receives,
  // so a per-workspace shape stays self-consistent end-to-end.
  const bundle = await buildBundleForWorkspace(workspaceId);
  const pubkeyHash = await bundleHash(bundle);
  const anchors = await readAnchors(workspaceId);
  const anchorChain = await readAnchorChain(workspaceId);

  const trace: AtlasTrace = {
    schema_version: SCHEMA_VERSION,
    generated_at: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    workspace_id: workspaceId,
    pubkey_bundle_hash: pubkeyHash,
    events,
    dag_tips: computeTips(events),
    anchors,
    ...(anchorChain !== undefined ? { anchor_chain: anchorChain } : {}),
    policies: [],
    filters: null,
  };
  return { trace, bundle };
}

/**
 * Read and validate `anchors.json` for a workspace. Returns `[]` if the
 * file does not exist (genesis case — anchors are optional). Any other
 * read or parse error is surfaced so the caller can fail the export
 * loudly rather than silently shipping an empty `anchors` field that
 * masks a corrupted file.
 */
async function readAnchors(workspaceId: string): Promise<AnchorEntry[]> {
  const path = anchorsPath(workspaceId);
  let raw: string;
  try {
    raw = await fs.readFile(path, "utf8");
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw new Error(`failed to read ${basename(path)}: ${(e as Error).message}`);
  }
  let parsed: unknown;
  try {
    // Lossless parse so anchors.json round-trips Sigstore tree_id
    // values without truncation. Mock entries omit `tree_id` so the
    // parser still produces native numbers for every other field.
    parsed = parseAnchorJson(raw);
  } catch (e) {
    throw new Error(`${basename(path)} is not valid JSON: ${(e as Error).message}`);
  }
  const validated = AnchorEntryArraySchema.safeParse(parsed);
  if (!validated.success) {
    throw new Error(
      `${basename(path)} failed AnchorEntry[] schema: ${validated.error.message}`,
    );
  }
  return validated.data as AnchorEntry[];
}

/**
 * Read a workspace's `anchor-chain.jsonl` and return a validated
 * `AnchorChain` ready to embed in `AtlasTrace.anchor_chain`.
 *
 * Skip rules:
 *   * File missing (ENOENT): return `undefined`. V1.5/V1.6 workspaces
 *     and any workspace that has not yet been anchored have no chain
 *     file — the lenient verifier passes traces without a chain.
 *   * File present but empty (0 bytes or whitespace-only): return
 *     `undefined`. Same lenient behaviour: an empty file is treated as
 *     "no chain witness yet" rather than a malformed bundle. The
 *     issuer never produces an empty file (`extend_chain_with_batch`
 *     refuses empty entries), so this case only arises from manual
 *     creation or pre-existing tooling.
 *
 * Otherwise, the file content is handed to the Rust signer's
 * `chain-export` subcommand which parses each line as `AnchorBatch`,
 * recomputes the head via `chain_head_for`, runs
 * `verify_anchor_chain`, and emits a wire-format `AnchorChain`. Any
 * verification failure (corruption, gap, reorder) surfaces here as a
 * thrown error, failing the export rather than shipping a broken
 * trace.
 */
async function readAnchorChain(workspaceId: string): Promise<AnchorChain | undefined> {
  const path = anchorChainPath(workspaceId);
  // Stat-first guard: refuse to allocate a multi-gigabyte string for an
  // attacker-grown chain file. A workspace's chain is dozens to hundreds
  // of batches at a few KB each, so 50 MB is two orders of magnitude
  // above any plausible operational ceiling. Failing here is operator-
  // visible (the export errors loudly) rather than crashing the Node
  // process or the spawned signer with OOM.
  const MAX_CHAIN_FILE_BYTES = 50 * 1024 * 1024;
  let stat: import("node:fs").Stats;
  try {
    stat = await fs.stat(path);
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw new Error(`failed to stat ${basename(path)}: ${(e as Error).message}`);
  }
  if (stat.size === 0) return undefined;
  if (stat.size > MAX_CHAIN_FILE_BYTES) {
    throw new Error(
      `${basename(path)} is ${stat.size} bytes, exceeds the ${MAX_CHAIN_FILE_BYTES}-byte ceiling`,
    );
  }
  let raw: string;
  try {
    raw = await fs.readFile(path, "utf8");
  } catch (e) {
    throw new Error(`failed to read ${basename(path)}: ${(e as Error).message}`);
  }
  if (raw.trim().length === 0) return undefined;
  return chainExportViaSigner(raw);
}

/**
 * Deterministic hash of a `PubkeyBundle`, computed by the Rust signer.
 *
 * This function exists so callers don't have to know about the
 * subprocess seam. Internally it serialises the bundle to JSON and
 * shells out to `atlas-signer bundle-hash`, which re-parses and runs
 * the same `deterministic_hash` path the verifier runs at compare time.
 */
export async function bundleHash(bundle: PubkeyBundle): Promise<string> {
  const json = JSON.stringify(bundle);
  return bundleHashViaSigner(json);
}
