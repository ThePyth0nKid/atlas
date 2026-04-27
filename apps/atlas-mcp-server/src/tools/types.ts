/**
 * Shared shape for an MCP tool definition. Each tool exports an object
 * matching this contract; `index.ts` walks the registry and binds them
 * to the McpServer.
 *
 * The handler returns the MCP-canonical `{ content: [...] }` shape so
 * callers (Claude/Cursor/Inspector) render it consistently. We use
 * structured JSON-as-text for machine-parseability AND human-readability
 * — bots and humans both consume MCP tool output.
 */

import type { z } from "zod";

export type RawShape = Record<string, z.ZodType<unknown>>;

export type ToolHandlerResult = {
  content: Array<{ type: "text"; text: string }>;
  isError?: boolean;
};

export type ToolDefinition<S extends RawShape> = {
  name: string;
  description: string;
  inputSchema: S;
  handler: (input: Record<string, unknown>) => Promise<ToolHandlerResult>;
};
