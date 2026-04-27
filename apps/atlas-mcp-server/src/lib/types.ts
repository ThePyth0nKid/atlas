/**
 * TypeScript mirrors of the wire-format types from
 * `crates/atlas-trust-core/src/trace_format.rs`.
 *
 * These types are *parsed*, never *encoded for hashing*. The canonical
 * signing-input is built only by the Rust crate. Drift between these
 * mirrors and the Rust types would be a parse failure on the verifier
 * side, not a silent semantic mismatch — which is the whole point of
 * the single-canonicalisation rule.
 */

export type EventSignature = {
  alg: "EdDSA";
  kid: string;
  sig: string;
};

export type AtlasEvent = {
  event_id: string;
  event_hash: string;
  parent_hashes: string[];
  payload: Record<string, unknown>;
  signature: EventSignature;
  ts: string;
};

export type AtlasPayloadType =
  | "node.create"
  | "node.update"
  | "edge.create"
  | "annotation.add"
  | "policy.set"
  | "anchor.created";

export type PubkeyBundle = {
  schema: "atlas-pubkey-bundle-v1";
  generated_at: string;
  keys: Record<string, string>;
};

export type AnchorEntry = {
  dag_tip_hash: string;
  rekor_uuid: string;
  rekor_inclusion_proof: string;
  rekor_log_index: number;
  rekor_ts: string;
};

export type AtlasTrace = {
  schema_version: "atlas-trace-v1";
  generated_at: string;
  workspace_id: string;
  pubkey_bundle_hash: string;
  events: AtlasEvent[];
  dag_tips: string[];
  anchors: AnchorEntry[];
  policies: string[];
  filters: null;
};

export const SCHEMA_VERSION = "atlas-trace-v1" as const;
export const PUBKEY_BUNDLE_SCHEMA = "atlas-pubkey-bundle-v1" as const;
export const DEFAULT_WORKSPACE = "ws-mcp-default" as const;
