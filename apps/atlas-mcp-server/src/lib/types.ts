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
 *
 * `entry_body_b64` and `tree_id` are V1.6 Sigstore-format-only fields
 * (`Option<...>` on the Rust side, `#[serde(skip_serializing_if = ...)]`).
 * They are absent for the atlas-mock format and present for Sigstore
 * Rekor v1 entries — the verifier dispatches by `log_id` and demands
 * them only when the format requires.
 *
 * Caveat: `tree_id` arrives over the wire as a JSON number. Sigstore's
 * current shard tree-id (~2^60) exceeds `Number.MAX_SAFE_INTEGER`
 * (~2^53), so a `JSON.parse` round-trip in this Node process can
 * silently truncate the low digits. V1.7 sidesteps the problem by
 * gating `anchor-chain` extension on the mock-only path; V1.8 lifts
 * the limitation with a precision-preserving JSON parser.
 */
export type AnchorEntry = {
  kind: AnchorKind;
  anchored_hash: string;
  log_id: string;
  log_index: number;
  integrated_time: number;
  inclusion_proof: InclusionProof;
  entry_body_b64?: string;
  tree_id?: number;
};

/**
 * One anchor batch in the V1.7 chain. Mirrors `AnchorBatch` in
 * `crates/atlas-trust-core/src/trace_format.rs`.
 *
 * The on-disk representation lives at
 * `data/{workspace}/anchor-chain.jsonl`, one batch per line. The
 * issuer is the sole writer; the MCP exporter reads but never
 * mutates. `previous_head` cross-links each batch to the prior one
 * via blake3 over canonical batch bytes — the verifier walks the
 * history and rejects any gap, reorder, or mutation.
 */
export type AnchorBatch = {
  batch_index: number;
  integrated_time: number;
  entries: AnchorEntry[];
  previous_head: string;
};

/**
 * Hash-chain witness over the workspace's anchor batches (V1.7).
 * Mirrors `AnchorChain` in
 * `crates/atlas-trust-core/src/trace_format.rs`.
 *
 * `head` is `chain_head_for(history.last())`; the verifier
 * recomputes it locally and never trusts the field as a verification
 * shortcut. Lenient mode tolerates traces without a chain (V1.5/V1.6
 * round-trip); strict mode (`require_anchor_chain = true`) demands a
 * present, valid chain.
 */
export type AnchorChain = {
  history: AnchorBatch[];
  head: string;
};

export type AtlasTrace = {
  schema_version: "atlas-trace-v1";
  generated_at: string;
  workspace_id: string;
  pubkey_bundle_hash: string;
  events: AtlasEvent[];
  dag_tips: string[];
  anchors: AnchorEntry[];
  anchor_chain?: AnchorChain;
  policies: string[];
  filters: null;
};

export const SCHEMA_VERSION = "atlas-trace-v1" as const;
export const PUBKEY_BUNDLE_SCHEMA = "atlas-pubkey-bundle-v1" as const;
export const DEFAULT_WORKSPACE = "ws-mcp-default" as const;
