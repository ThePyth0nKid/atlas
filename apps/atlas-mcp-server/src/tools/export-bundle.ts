/**
 * `atlas_export_bundle` — emit a shippable AtlasTrace + matching pubkey
 * bundle for the workspace, ready for offline auditor verification.
 *
 * The bundle is what the auditor receives. From the moment this tool
 * returns, the auditor has everything to reproduce the verifier's
 * judgment without further communication with the MCP server. That
 * "audit-by-mail" property is the entire reason Atlas exists.
 *
 * Output shape: a single text blob containing the trace JSON, the bundle
 * JSON, and a SHA-256 of each so the caller can copy them verbatim into
 * any vault/object store with integrity guarantees on the storage tier.
 */

import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { exportWorkspaceBundle } from "../lib/bundle.js";
import { workspaceDir } from "../lib/paths.js";
import { DEFAULT_WORKSPACE } from "../lib/types.js";
import { optionalWorkspaceIdSchema } from "./schema.js";
import type { ToolDefinition } from "./types.js";

export const exportBundleInputSchema = {
  workspace_id: optionalWorkspaceIdSchema
    .describe(`Workspace id; defaults to "${DEFAULT_WORKSPACE}". [a-zA-Z0-9_-]{1,128}.`),
  /**
   * If true, write trace.json + bundle.json into the workspace dir on
   * disk in addition to returning them inline. Useful for handing off
   * the bundle by file path to `atlas-verify-cli`.
   */
  write_to_disk: z.boolean().default(true)
    .describe("Also write trace.json + bundle.json to the workspace directory on disk."),
};

const inputZ = z.object(exportBundleInputSchema);

export const exportBundleTool: ToolDefinition<typeof exportBundleInputSchema> = {
  name: "atlas_export_bundle",
  description:
    "Export the workspace's AtlasTrace and matching PubkeyBundle for offline " +
    "auditor verification. Output is the bundle the auditor verifies against — " +
    "after this returns, no further interaction with the MCP server is required.",
  inputSchema: exportBundleInputSchema,
  handler: async (raw) => {
    const args = inputZ.parse(raw);
    const workspaceId = args.workspace_id ?? DEFAULT_WORKSPACE;
    const { trace, bundle } = await exportWorkspaceBundle(workspaceId);
    const traceJson = JSON.stringify(trace, null, 2);
    const bundleJson = JSON.stringify(bundle, null, 2);
    const traceSha256 = sha256Hex(traceJson);
    const bundleSha256 = sha256Hex(bundleJson);

    let writtenPaths: { trace_path: string; bundle_path: string } | null = null;
    if (args.write_to_disk) {
      const dir = workspaceDir(workspaceId);
      await fs.mkdir(dir, { recursive: true });
      const tracePath = join(dir, "trace.json");
      const bundlePath = join(dir, "bundle.json");
      // Write to a per-process temp suffix then rename. rename(2) is
      // atomic on the same filesystem, so a concurrent reader sees
      // either the old or the new file — never a half-written one.
      // Auditor reading trace.json mid-export must not see a trace that
      // doesn't match the bundle.json next to it.
      const suffix = `.tmp-${process.pid}-${Date.now().toString(36)}`;
      const traceTmp = tracePath + suffix;
      const bundleTmp = bundlePath + suffix;
      await fs.writeFile(traceTmp, traceJson, "utf8");
      await fs.writeFile(bundleTmp, bundleJson, "utf8");
      await fs.rename(traceTmp, tracePath);
      await fs.rename(bundleTmp, bundlePath);
      writtenPaths = { trace_path: tracePath, bundle_path: bundlePath };
    }

    return {
      content: [
        {
          type: "text" as const,
          text: JSON.stringify(
            {
              ok: true,
              workspace_id: workspaceId,
              event_count: trace.events.length,
              dag_tip_count: trace.dag_tips.length,
              pubkey_bundle_hash: trace.pubkey_bundle_hash,
              trace_sha256: traceSha256,
              bundle_sha256: bundleSha256,
              written_paths: writtenPaths,
              verify_command: writtenPaths
                ? `atlas-verify-cli verify-trace "${writtenPaths.trace_path}" -k "${writtenPaths.bundle_path}"`
                : null,
            },
            null,
            2,
          ),
        },
      ],
    };
  },
};

function sha256Hex(s: string): string {
  return createHash("sha256").update(s, "utf8").digest("hex");
}
