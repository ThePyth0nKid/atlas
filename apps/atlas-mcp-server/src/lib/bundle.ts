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
import { buildDevBundle } from "./keys.js";
import { anchorsPath } from "./paths.js";
import { AnchorEntryArraySchema } from "./schema.js";
import { bundleHashViaSigner } from "./signer.js";
import { readAllEvents, computeTips } from "./storage.js";
import {
  SCHEMA_VERSION,
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
 */
export async function exportWorkspaceBundle(workspaceId: string): Promise<ExportedBundle> {
  const events: AtlasEvent[] = await readAllEvents(workspaceId);
  const bundle = buildDevBundle();
  const pubkeyHash = await bundleHash(bundle);
  const anchors = await readAnchors(workspaceId);

  const trace: AtlasTrace = {
    schema_version: SCHEMA_VERSION,
    generated_at: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    workspace_id: workspaceId,
    pubkey_bundle_hash: pubkeyHash,
    events,
    dag_tips: computeTips(events),
    anchors,
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
    parsed = JSON.parse(raw);
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
