/**
 * Build a shippable AtlasTrace + matching PubkeyBundle from a workspace's
 * events.jsonl.
 *
 * The bundle is what the auditor receives. After this function returns,
 * the auditor can run `atlas-verify-cli verify-trace trace.json -k bundle.json`
 * with no further server interaction and reach the same ✓ VALID outcome
 * as anyone else.
 *
 * The pubkey_bundle_hash is computed by replicating the same canonical
 * JSON used by `crates/atlas-trust-core/src/pubkey_bundle.rs::deterministic_hash`,
 * then blake3 over the bytes. The byte-pinned golden in the Rust crate
 * locks the canonical-JSON format, and a TS-side smoke (the e2e test)
 * proves they agree.
 */

import { createHash } from "node:crypto";
import { blake3 } from "./blake3.js";
import { buildDevBundle } from "./keys.js";
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
  const pubkeyHash = bundleHash(bundle);

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
 * Deterministic hash of a PubkeyBundle.
 *
 * Mirrors `crates/atlas-trust-core/src/pubkey_bundle.rs::deterministic_hash`:
 *   - canonical JSON: keys sorted lex, no whitespace, no escaping changes
 *   - blake3 of the resulting bytes, hex-encoded
 *
 * The smoke test verifies that the hash this function produces is
 * byte-identical to the hash the Rust verifier recomputes. If they ever
 * disagree the smoke test fails — which is exactly what the byte-pinned
 * golden in `bundle_hash_byte_determinism_pin` exists to make
 * structurally impossible.
 */
export function bundleHash(bundle: PubkeyBundle): string {
  const canonical = canonicalJsonStringify({
    generated_at: bundle.generated_at,
    keys: bundle.keys,
    schema: bundle.schema,
  });
  const bytes = new TextEncoder().encode(canonical);
  return toHex(blake3(bytes));
}

/**
 * Canonical-JSON serialization matching the Rust crate's
 * `canonical_json_bytes`:
 *   - object keys sorted lex (recursive)
 *   - no whitespace
 *   - JSON.stringify-compatible string escaping
 *   - integer numbers rendered by `Number.toString` (matches serde_json)
 */
function canonicalJsonStringify(value: unknown): string {
  if (value === null) return "null";
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      throw new Error("non-finite number not allowed in canonical JSON");
    }
    return value.toString();
  }
  if (typeof value === "string") return JSON.stringify(value);
  if (Array.isArray(value)) {
    return "[" + value.map(canonicalJsonStringify).join(",") + "]";
  }
  if (typeof value === "object") {
    const obj = value as Record<string, unknown>;
    const keys = Object.keys(obj).sort();
    return (
      "{" +
      keys
        .map((k) => JSON.stringify(k) + ":" + canonicalJsonStringify(obj[k]))
        .join(",") +
      "}"
    );
  }
  throw new Error(`unsupported value in canonical JSON: ${typeof value}`);
}

function toHex(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}

// Re-exposed for tests / smoke.
export const _internal = { canonicalJsonStringify, bundleHash };

// `createHash` import retained for forward V1.5 compatibility
// (Rekor anchor cross-checks use SHA-256). Silence unused-import lint.
void createHash;
