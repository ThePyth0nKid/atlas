/**
 * Process-wide bootstrap for atlas-web.
 *
 * V1.19 Welle 2 consolidation: the bridge package no longer ships
 * an app-specific data-directory default. Each consumer registers
 * its own via `setDefaultDataDir()`; the env var `ATLAS_DATA_DIR`
 * still wins as the operator override.
 *
 * Importing this module has the side effect of registering the web
 * app's local default. The route handler imports it once — Node's
 * module cache means the side effect runs exactly once per process,
 * regardless of how many routes import the bridge.
 *
 * In production, operators set `ATLAS_DATA_DIR` explicitly (see
 * OPERATOR-RUNBOOK §17) and this default never engages. The
 * fallback only matters for `next dev` and the e2e smoke (which
 * already sets `ATLAS_DATA_DIR` to a tmp dir before importing).
 */

import { setDefaultDataDir } from "@atlas/bridge";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

// In `next dev` and tsx-driven scripts, `import.meta.url` resolves
// to this source file: `apps/atlas-web/src/lib/bootstrap.ts`. Walk
// `src/lib/` → `src/` → `apps/atlas-web/`.
const HERE = dirname(fileURLToPath(import.meta.url));
const APP_ROOT = resolve(HERE, "..", "..");
setDefaultDataDir(join(APP_ROOT, "data"));
