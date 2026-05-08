#!/usr/bin/env node
/**
 * Atlas MCP server — stdio entry point.
 *
 * This binary is what an MCP host (Claude Desktop, Cursor, an Inspector
 * dev tool) connects to. It exposes the `atlas_*` tools that turn
 * agent actions into signed, append-only graph events.
 *
 * Run modes:
 *   - `pnpm dev`              — tsx, hot-reload-ish for local hacking
 *   - `pnpm build && pnpm start` — compiled, what you wire into a host
 *
 * Wiring into Claude Desktop:
 *   {
 *     "mcpServers": {
 *       "atlas": {
 *         "command": "node",
 *         "args": ["/abs/path/to/apps/atlas-mcp-server/dist/index.js"]
 *       }
 *     }
 *   }
 *
 * The signer binary (Rust) is resolved via env var `ATLAS_SIGNER_PATH`
 * or auto-discovered under `target/release/`. See lib/paths.ts.
 */

import "./bootstrap.js";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { TOOL_REGISTRY } from "./tools/index.js";

async function main(): Promise<void> {
  const server = new McpServer({
    name: "atlas-mcp-server",
    version: "0.1.0",
  });

  for (const tool of TOOL_REGISTRY) {
    server.registerTool(
      tool.name,
      {
        description: tool.description,
        inputSchema: tool.inputSchema,
      },
      async (args) => {
        try {
          return await tool.handler(args as Record<string, unknown>);
        } catch (e) {
          const msg = e instanceof Error ? e.message : String(e);
          return {
            isError: true,
            content: [
              {
                type: "text" as const,
                text: JSON.stringify({ ok: false, error: msg }, null, 2),
              },
            ],
          };
        }
      },
    );
  }

  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((e: unknown) => {
  const msg = e instanceof Error ? e.stack ?? e.message : String(e);
  // MCP servers communicate over stdout via JSON-RPC; diagnostics go to stderr.
  process.stderr.write(`atlas-mcp-server fatal: ${msg}\n`);
  process.exit(1);
});
