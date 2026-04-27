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

import { buildDevBundle } from "./keys.js";
import { bundleHashViaSigner } from "./signer.js";
import { readAllEvents, computeTips } from "./storage.js";
import {
  SCHEMA_VERSION,
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
 */
export async function exportWorkspaceBundle(workspaceId: string): Promise<ExportedBundle> {
  const events: AtlasEvent[] = await readAllEvents(workspaceId);
  const bundle = buildDevBundle();
  const pubkeyHash = await bundleHash(bundle);

  const trace: AtlasTrace = {
    schema_version: SCHEMA_VERSION,
    generated_at: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    workspace_id: workspaceId,
    pubkey_bundle_hash: pubkeyHash,
    events,
    dag_tips: computeTips(events),
    anchors: [],
    policies: [],
    filters: null,
  };
  return { trace, bundle };
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
