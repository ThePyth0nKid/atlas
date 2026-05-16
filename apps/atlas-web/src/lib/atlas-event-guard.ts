/**
 * W20b-1 fix-commit (code-reviewer HIGH) — runtime type-guard for
 * untrusted `AtlasEvent` shapes deserialised from a trace endpoint
 * response.
 *
 * Extracted from `DashboardMetricsSection.tsx` into its own .ts module
 * so it can be unit-tested in vitest's node environment without
 * pulling React hooks into the test runtime. The dashboard component
 * re-exports the guard for backwards compatibility with the inline
 * import path used by the original W20b-1 implementation.
 *
 * Contract: the guard checks only the five fields the dashboard
 * actually reads (`event_id`, `event_hash`, `ts`, `payload`,
 * `signature`). It deliberately does NOT enforce the full Zod schema
 * from `@atlas/bridge/schema.ts` — importing that here would pull
 * `node:fs` into the browser bundle. For strict boundary validation,
 * use the Zod schema in the trace endpoint's server-side route
 * handler.
 *
 * Threat model (TM-W20b-1):
 *   A malformed entry (wrong type for any required field) is silently
 *   dropped by the consumer's `Array.prototype.filter`. This is safe
 *   because `computeWorkspaceMetrics` is itself defensive about
 *   malformed inputs — the filter only narrows the type for
 *   downstream consumers that read `state.events` directly.
 */

import type { AtlasEvent } from "@atlas/bridge";

export function isAtlasEventShape(item: unknown): item is AtlasEvent {
  if (typeof item !== "object" || item === null) return false;
  const r = item as Record<string, unknown>;
  return (
    typeof r.event_id === "string" &&
    typeof r.event_hash === "string" &&
    typeof r.ts === "string" &&
    typeof r.payload === "object" &&
    r.payload !== null &&
    typeof r.signature === "object" &&
    r.signature !== null
  );
}
