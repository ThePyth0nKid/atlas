/**
 * Process-wide bootstrap for atlas-mcp-server.
 *
 * V1.19 Welle 2 consolidation: the bridge package no longer ships
 * an app-specific data-directory default — its `__dirname` lives in
 * `packages/atlas-bridge/`, which would resolve to a shared
 * `<repo-root>/data` if used as the fallback. Each consumer instead
 * registers its own default via `setDefaultDataDir()`. The env var
 * `ATLAS_DATA_DIR` still wins as the operator override.
 *
 * Importing this module has the side effect of registering the MCP
 * server's local default. Every entry point that may touch
 * workspace storage must import it before any other bridge call:
 *   - `src/index.ts` (the MCP stdio binary)
 *   - `scripts/smoke.ts`
 *
 * Pure data-format scripts (e.g. `scripts/test-anchor-json.ts`) do
 * not need the bootstrap because they never call `dataDir()`.
 */

import { setDefaultDataDir } from "@atlas/bridge";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

// `import.meta.url` after tsc is `apps/atlas-mcp-server/dist/bootstrap.js`;
// under tsx it is `apps/atlas-mcp-server/src/bootstrap.ts`. Both `dist/`
// and `src/` are exactly one level under the app root.
const HERE = dirname(fileURLToPath(import.meta.url));
const PACKAGE_ROOT = resolve(HERE, "..");
setDefaultDataDir(join(PACKAGE_ROOT, "data"));
