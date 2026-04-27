/**
 * Tool-input schema fragments shared across tools.
 *
 * Centralised so the workspace-id allowlist regex is defined once and
 * cannot drift between tools. Defence in depth: even though `paths.ts`
 * also enforces the allowlist, surfacing the validation error at the
 * Zod boundary gives the MCP client a clean error before the request
 * touches the filesystem.
 */

import { z } from "zod";

/**
 * Allowlist for workspace identifiers — kept in sync with
 * `WORKSPACE_ID_RE` in `lib/paths.ts`. Keep narrow: workspace ids appear
 * in filesystem paths AND in the canonical signing-input.
 */
export const WORKSPACE_ID_PATTERN = /^[a-zA-Z0-9_-]{1,128}$/;

export const workspaceIdSchema = z
  .string()
  .regex(WORKSPACE_ID_PATTERN, "workspace_id: only [a-zA-Z0-9_-], 1–128 chars");

export const optionalWorkspaceIdSchema = workspaceIdSchema.optional();
