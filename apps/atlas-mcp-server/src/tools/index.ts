/**
 * Tool registry. `index.ts` walks this list and binds each entry to the
 * McpServer. Adding a new tool means: write the file, append it here,
 * done.
 *
 * V2-β Welle 13 adds 5 read-side V2 tools (query_graph, query_entities,
 * query_provenance, get_agent_passport [STUB], get_timeline). They are
 * appended AFTER the V1 write-side tools so that any pre-existing MCP
 * client whose tool-index pinning relies on registration order still
 * sees the V1 entries at their original indices.
 */

import { anchorBundleTool } from "./anchor-bundle.js";
import { exportBundleTool } from "./export-bundle.js";
import { getAgentPassportTool } from "./get-agent-passport.js";
import { getTimelineTool } from "./get-timeline.js";
import { queryEntitiesTool } from "./query-entities.js";
import { queryGraphTool } from "./query-graph.js";
import { queryProvenanceTool } from "./query-provenance.js";
import type { RawShape, ToolDefinition } from "./types.js";
import { workspaceStateTool } from "./workspace-state.js";
import { writeAnnotationTool } from "./write-annotation.js";
import { writeNodeTool } from "./write-node.js";

export const TOOL_REGISTRY: ReadonlyArray<ToolDefinition<RawShape>> = [
  // V1 write-side + workspace tools (preserve original ordering)
  writeNodeTool,
  writeAnnotationTool,
  exportBundleTool,
  anchorBundleTool,
  workspaceStateTool,
  // V2-β read-side tools (Welle 13)
  queryGraphTool,
  queryEntitiesTool,
  queryProvenanceTool,
  getAgentPassportTool,
  getTimelineTool,
];
