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

/**
 * What kind of object an `AnchorEntry` refers to.
 *
 * Mirrors `AnchorKind` in `crates/atlas-trust-core/src/trace_format.rs`
 * with `#[serde(rename_all = "snake_case")]`. JSON values are exactly
 * `"dag_tip"` or `"bundle_hash"`.
 */
export type AnchorKind = "dag_tip" | "bundle_hash";

/**
 * Merkle inclusion proof of a leaf against a signed log checkpoint.
 * Mirrors `InclusionProof` in `crates/atlas-trust-core/src/trace_format.rs`.
 */
export type InclusionProof = {
  tree_size: number;
  root_hash: string;
  hashes: string[];
  checkpoint_sig: string;
};

/**
 * One anchor entry — proof that a specific hash was committed to a
 * transparency log at a specific time, with a Merkle inclusion proof
 * against a signed log checkpoint.
 *
 * Mirrors `AnchorEntry` in `crates/atlas-trust-core/src/trace_format.rs`.
 * V1.5 ships the offline verification path; the verifier validates the
 * proof and checkpoint signature against pinned log pubkeys.
 */
export type AnchorEntry = {
  kind: AnchorKind;
  anchored_hash: string;
  log_id: string;
  log_index: number;
  integrated_time: number;
  inclusion_proof: InclusionProof;
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
