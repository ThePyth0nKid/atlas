/**
 * Tool registry. `index.ts` walks this list and binds each entry to the
 * McpServer. Adding a new tool means: write the file, append it here,
 * done.
 */

import { anchorBundleTool } from "./anchor-bundle.js";
import { exportBundleTool } from "./export-bundle.js";
import type { RawShape, ToolDefinition } from "./types.js";
import { workspaceStateTool } from "./workspace-state.js";
import { writeAnnotationTool } from "./write-annotation.js";
import { writeNodeTool } from "./write-node.js";

export const TOOL_REGISTRY: ReadonlyArray<ToolDefinition<RawShape>> = [
  writeNodeTool,
  writeAnnotationTool,
  exportBundleTool,
  anchorBundleTool,
  workspaceStateTool,
];
