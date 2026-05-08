/**
 * `atlas_anchor_bundle` — issue mock-Rekor anchors over the workspace's
 * current state and persist the result for inclusion in future exports.
 *
 * What gets anchored
 *   1. The `pubkey_bundle_hash` of the workspace's PubkeyBundle. Defends
 *      against post-hoc bundle-swap attacks that would re-validate forged
 *      signatures: an auditor with the anchor knows "this exact key
 *      roster was the one in use by time T".
 *   2. Every current DAG-tip `event_hash`. Defends against post-hoc tail
 *      truncation or fork: an auditor knows "this trace state existed by
 *      time T".
 *
 * Both anchor kinds share the same Merkle tree and signed checkpoint, so
 * a single batch produces N proofs against one root. The signer (Rust)
 * builds the tree, signs the checkpoint with the dev mock-Rekor key, and
 * emits one `AnchorEntry` per item. V1.6 swaps the issuer for a real
 * Sigstore POST behind `--rekor-url` without touching this tool.
 *
 * Persistence: result is written to `data/{workspace}/anchors.json`
 * atomically (tmp + rename). `exportWorkspaceBundle` reads that file and
 * places the entries in `trace.anchors`. Re-running this tool overwrites
 * the file — anchors are a snapshot of the current state, not an
 * append-only log.
 */

import { randomBytes } from "node:crypto";
import { promises as fs } from "node:fs";
import { z } from "zod";
import {
  stringifyAnchorJson,
  anchorChainPath,
  anchorsPath,
  anchorViaSigner,
  type AnchorRequest,
  ensureWorkspaceDir,
  DEFAULT_WORKSPACE,
} from "@atlas/bridge";
import { exportWorkspaceBundle } from "../lib/bundle.js";
import { optionalWorkspaceIdSchema } from "./schema.js";
import type { ToolDefinition } from "./types.js";

export const anchorBundleInputSchema = {
  workspace_id: optionalWorkspaceIdSchema
    .describe(`Workspace id; defaults to "${DEFAULT_WORKSPACE}". [a-zA-Z0-9_-]{1,128}.`),
  /**
   * Unix-seconds the issuer records as `integrated_time`. Caller-supplied
   * (rather than always `now`) so smoke tests and goldens can produce
   * byte-identical anchors across runs. Defaults to current Unix time.
   */
  integrated_time: z.number().int().nonnegative().optional()
    .describe("Unix seconds for the anchor's integrated_time. Defaults to now."),
  /**
   * Optional live-Rekor base URL. When set, anchors are issued via a
   * real Sigstore Rekor v1 instance (e.g. `https://rekor.sigstore.dev`)
   * and the resulting `AnchorEntry` rows carry the Sigstore log_id +
   * tree_id + canonical entry body. When unset, the in-process mock
   * issuer runs (default for smoke tests and offline demos). Falls
   * back to the `ATLAS_REKOR_URL` environment variable if neither
   * field nor argv supplies a value.
   *
   * Validation lives in the Rust signer: `https://` is required for
   * non-loopback hosts; plaintext `http://` is gated to localhost.
   */
  rekor_url: z
    .string()
    .url()
    // V1.19 Welle 2 hardening: require https:// at the TS boundary so
    // schemes like file://, javascript:, or data: that pass `z.string().url()`
    // are rejected before they reach the Rust signer's loopback-aware
    // gate. The Rust check remains the authoritative defence (it allows
    // plaintext http only for loopback hosts), but narrowing here turns
    // the TS layer from "passthrough" into "strict-https-only" so an
    // SSRF-style abuse of non-http schemes is blocked even if the Rust
    // parser ever loosens.
    .refine((u) => u.startsWith("https://"), {
      message: "rekor_url must use https:// at the MCP tool boundary",
    })
    .optional()
    .describe(
      "Optional Rekor URL (e.g. https://rekor.sigstore.dev). Must be " +
        "https://. Falls back to ATLAS_REKOR_URL env var; if neither is " +
        "set, the in-process mock issuer runs.",
    ),
};

const inputZ = z.object(anchorBundleInputSchema);

export const anchorBundleTool: ToolDefinition<typeof anchorBundleInputSchema> = {
  name: "atlas_anchor_bundle",
  description:
    "Issue mock-Rekor inclusion proofs for the workspace's current pubkey-bundle " +
    "hash and DAG tips, and persist the result to anchors.json. Subsequent " +
    "atlas_export_bundle calls include these anchors in trace.anchors so an " +
    "offline auditor can verify the proofs against the pinned log key.",
  inputSchema: anchorBundleInputSchema,
  handler: async (raw) => {
    const args = inputZ.parse(raw);
    const workspaceId = args.workspace_id ?? DEFAULT_WORKSPACE;
    const integratedTime = args.integrated_time ?? Math.floor(Date.now() / 1000);

    // Re-use the same code path the auditor will see. If exportWorkspaceBundle
    // changes how it derives `pubkey_bundle_hash` or `dag_tips`, the anchors
    // we issue here track that change automatically — no parallel logic to
    // keep in sync.
    const { trace } = await exportWorkspaceBundle(workspaceId);

    const items: AnchorRequest[] = [
      { kind: "bundle_hash", anchored_hash: trace.pubkey_bundle_hash },
      ...trace.dag_tips.map(
        (tip): AnchorRequest => ({ kind: "dag_tip", anchored_hash: tip }),
      ),
    ];

    // Live-Rekor opt-in: explicit field beats env, env beats mock.
    const rekorUrl = args.rekor_url ?? process.env.ATLAS_REKOR_URL;
    await ensureWorkspaceDir(workspaceId);
    // V1.8: chain extension applies to BOTH paths. The MCP server
    // reads anchor stdout via `parseAnchorJson` (lossless-json), so
    // Sigstore Rekor v1 `tree_id` values (~2^60, exceeding
    // `Number.MAX_SAFE_INTEGER`) round-trip through Node without
    // digit loss; the chain head the verifier recomputes is
    // bit-identical to the head the issuer emitted. The Rust signer
    // remains the sole writer of `anchor-chain.jsonl`.
    const chainPath = anchorChainPath(workspaceId);
    const entries = await anchorViaSigner(
      { items, integrated_time: integratedTime },
      { rekorUrl, chainPath },
    );

    const target = anchorsPath(workspaceId);
    // Lossless stringify so any Sigstore `tree_id` carried by an
    // entry survives the on-disk round-trip. Mock entries (no
    // `tree_id`) produce byte-identical output to legacy
    // `JSON.stringify`.
    const json = stringifyAnchorJson(entries, 2);
    // Atomic write — concurrent exporters either see the previous
    // anchors.json or the new one, never a half-written file. rename(2)
    // is atomic within a single filesystem on POSIX, and ReplaceFile-like
    // on Windows when source and target share a directory.
    //
    // V1.19 Welle 2 hardening: append crypto-grade entropy to the
    // suffix. The previous `pid + Date.now()` combo would collide on
    // sub-millisecond writes from the same process (the per-workspace
    // mutex serialises events.jsonl writes but NOT anchors.json), and
    // a collision would make one writer silently overwrite the other's
    // tmp file mid-write. 4 random bytes is 32 bits of entropy on top
    // of pid+ms — beyond any plausible birthday attack at this scale.
    const suffix = `.tmp-${process.pid}-${Date.now().toString(36)}-${randomBytes(4).toString("hex")}`;
    const tmp = target + suffix;
    await fs.writeFile(tmp, json, "utf8");
    await fs.rename(tmp, target);

    const bundleAnchor = entries.find((e) => e.kind === "bundle_hash");
    const tipAnchorCount = entries.filter((e) => e.kind === "dag_tip").length;

    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(
            {
              ok: true,
              workspace_id: workspaceId,
              integrated_time: integratedTime,
              anchor_count: entries.length,
              bundle_anchored_hash: bundleAnchor?.anchored_hash ?? null,
              tip_anchor_count: tipAnchorCount,
              log_id: bundleAnchor?.log_id ?? null,
              tree_size: bundleAnchor?.inclusion_proof.tree_size ?? 0,
              root_hash: bundleAnchor?.inclusion_proof.root_hash ?? null,
              anchors_path: target,
              anchor_chain_path: chainPath,
            },
            null,
            2,
          ),
        },
      ],
    };
  },
};
