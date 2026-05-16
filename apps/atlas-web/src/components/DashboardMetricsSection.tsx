"use client";

/**
 * W20b-1 — 3-tier dashboard metrics section.
 *
 * Replaces the hard-coded marketing KPIs that lived inline in
 * `apps/atlas-web/src/app/page.tsx` (W20a). The component fetches the
 * current workspace's `/api/atlas/trace` response, computes metrics
 * locally via the pure `computeWorkspaceMetrics` module, and renders
 * one of three tiers based on `totalEvents`:
 *
 *   - <EmptyTier>   totalEvents = 0       → welcome + CTA, no cards
 *   - <EarlyTier>   1 ≤ events ≤ 10        → recent-events list
 *   - <FullTier>    events ≥ 11           → 7 real KPI cards
 *
 * The tier thresholds (10/11) are intentionally calibrated for the
 * V2-β-1 pivot: with a handful of events the KPI cards read as
 * misleading ("12.4% vs prior period" with 3 events is noise), while
 * the recent-events list shows the user exactly what they wrote. The
 * threshold can move as we learn from real users; the file-level
 * constants below are the single source of truth.
 *
 * Frozen testids (Playwright dashboard-tiers.spec.ts):
 *   - dashboard-tier-empty
 *   - dashboard-tier-early
 *   - dashboard-tier-full
 *   - dashboard-metrics-loading
 *   - dashboard-metrics-error
 *   - kpi-card                  (inherited from <KpiCard>)
 *
 * Threat model (TM-W20b-1):
 *   Trace responses are JSON-parsed inside a try/catch. A failed parse
 *   surfaces as an error block (`role="alert"`) instead of crashing
 *   the whole page. The `computeWorkspaceMetrics` function is itself
 *   defensive about malformed event fields — see its module-level
 *   JSDoc for the malformed-input contract.
 */

import { useEffect, useState } from "react";
import Link from "next/link";
import type { AtlasEvent } from "@atlas/bridge";
import { KpiCard } from "@/components/KpiCard";
import {
  computeWorkspaceMetrics,
  type WorkspaceMetrics,
} from "@/lib/workspace-metrics";
import { useWorkspaceContext } from "@/lib/workspace-context";
import { isAtlasEventShape } from "@/lib/atlas-event-guard";
import type { LayerStatus } from "@/lib/system-health";

const EARLY_TIER_MAX = 10;

/**
 * W20c props.
 *
 * `layerStatus` is provided by the parent (`<HomeContent>`) so the
 * fetch is shared across LayerStatusPanel + DashboardMetricsSection.
 * `null` means "probe not yet resolved" — in that state we defer to
 * the event-count-only tier choice (DA-4).
 */
export interface DashboardMetricsSectionProps {
  layerStatus?: LayerStatus | null;
}

type FetchState =
  | { kind: "loading" }
  | { kind: "ready"; events: AtlasEvent[]; metrics: WorkspaceMetrics }
  | { kind: "error"; message: string };

interface TraceShape {
  workspace_id: string;
  events: unknown[];
}

export function DashboardMetricsSection({
  layerStatus = null,
}: DashboardMetricsSectionProps = {}): React.ReactElement {
  const { workspace, workspaces } = useWorkspaceContext();
  const [state, setState] = useState<FetchState>({ kind: "loading" });

  useEffect(() => {
    if (workspace === null) {
      setState({ kind: "loading" });
      return;
    }

    let cancelled = false;
    setState({ kind: "loading" });

    (async () => {
      try {
        const wsParam = encodeURIComponent(workspace);
        const res = await fetch(`/api/atlas/trace?workspace=${wsParam}`);
        if (!res.ok) {
          throw new Error(`could not load trace (HTTP ${res.status})`);
        }
        const json = await res.text();
        if (cancelled) return;

        let trace: TraceShape;
        try {
          trace = JSON.parse(json) as TraceShape;
        } catch {
          // W20b-1 fix-commit (security-reviewer TM-W20b-8): do NOT
          // surface the raw SyntaxError message to the DOM. If the
          // proxy ever returns an HTML error page, JSON.parse's error
          // text would include a snippet of that HTML, which would then
          // be rendered inside the dashboard error block. A fixed
          // message avoids leaking response-body content into the UI.
          throw new Error("trace response was not valid JSON");
        }
        const rawEvents = Array.isArray(trace.events) ? trace.events : [];
        // W20b-1 fix-commit (code-reviewer HIGH): replace the unchecked
        // `as AtlasEvent[]` cast with a runtime filter via the
        // `isAtlasEventShape` guard. Malformed entries are silently
        // dropped — `computeWorkspaceMetrics` already counts all
        // accepted events, so any drop here is bounded to entries that
        // could not have produced meaningful metrics anyway.
        const events: AtlasEvent[] = rawEvents.filter(isAtlasEventShape);
        const metrics = computeWorkspaceMetrics(events);
        if (cancelled) return;
        setState({ kind: "ready", events, metrics });
      } catch (e) {
        if (cancelled) return;
        setState({
          kind: "error",
          message: e instanceof Error ? e.message : String(e),
        });
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [workspace]);

  if (state.kind === "loading") {
    return (
      <div
        className="text-[13px] text-[var(--foreground-muted)]"
        data-testid="dashboard-metrics-loading"
      >
        Loading workspace metrics…
      </div>
    );
  }

  if (state.kind === "error") {
    return (
      <div
        className="border border-[var(--border)] rounded-lg p-4 text-[13px] text-[var(--accent-danger)]"
        role="alert"
        data-testid="dashboard-metrics-error"
      >
        Failed to load workspace metrics: {state.message}
      </div>
    );
  }

  const { events, metrics } = state;

  // W20c (DA-4) — honest tier resolution. The effective tier is the
  // minimum of event-count tier and layer-readiness tier. A workspace
  // with 200 events but `signer=unconfigured` renders Empty (with the
  // signer-not-ready banner) because the dashboard would be lying:
  // the user cannot sign new events even though stale ones exist.
  //
  // Layer-status `null` means the health probe is still in flight or
  // failed; in that state we DO NOT degrade — the operator gets the
  // event-count tier they would have seen pre-W20c, and the separate
  // <LayerStatusPanel> surfaces the loading/error state. This avoids
  // a flash of Empty during the brief health-probe RTT.
  const signerUnconfigured =
    layerStatus !== null && layerStatus.signer !== "operational";

  if (metrics.totalEvents === 0) {
    return <EmptyTier degraded={signerUnconfigured} />;
  }
  if (signerUnconfigured) {
    return <EmptyTier degraded={true} />;
  }
  if (metrics.totalEvents <= EARLY_TIER_MAX) {
    return <EarlyTier events={events} />;
  }
  return <FullTier metrics={metrics} workspaces={workspaces} />;
}

// ───────────────────────── EmptyTier ─────────────────────────

interface EmptyTierProps {
  /**
   * W20c (DA-4) — true when the tier was downgraded from Full/Early
   * to Empty because `signer !== 'operational'`. In that case we add
   * a banner explaining the degradation and pointing to /settings.
   */
  degraded?: boolean;
}

function EmptyTier({ degraded = false }: EmptyTierProps): React.ReactElement {
  return (
    <section
      data-testid="dashboard-tier-empty"
      className="border border-dashed border-[var(--border)] rounded-lg p-10 text-center"
    >
      {degraded ? (
        <div
          className="mb-4 mx-auto max-w-md text-[12px] border rounded-md px-3 py-2"
          style={{
            background: "color-mix(in srgb, var(--accent-warn) 15%, transparent)",
            // W20c — amber-900 for AA on the 15%-mix banner bg.
            color: "var(--accent-warn-on-mix)",
            borderColor: "var(--accent-warn)",
          }}
          data-testid="dashboard-layer-not-ready"
          role="alert"
        >
          Layer 3 signer is unconfigured. Configure{" "}
          <code className="font-mono">ATLAS_DEV_MASTER_SEED</code> and revisit{" "}
          <Link href="/settings" className="underline">
            Settings
          </Link>{" "}
          to re-check status.
        </div>
      ) : null}
      <h2 className="text-xl font-semibold tracking-tight mb-2">
        Welcome to Atlas
      </h2>
      <p className="text-[var(--foreground-muted)] max-w-md mx-auto mb-6">
        Your audit trail starts with your first signed fact.
      </p>
      <Link
        href="/write"
        className="inline-block text-[13px] font-medium border border-[var(--border)] rounded-md px-4 py-2 hover:bg-[var(--bg-subtle)]"
        data-testid="dashboard-empty-cta"
      >
        Write your first fact →
      </Link>
    </section>
  );
}

// ───────────────────────── EarlyTier ─────────────────────────

interface EarlyTierProps {
  events: ReadonlyArray<AtlasEvent>;
}

function EarlyTier({ events }: EarlyTierProps): React.ReactElement {
  // Sort newest-first by ts. Malformed ts sinks to the bottom — we use
  // -Infinity for parse failures so they sort last under descending order.
  const sorted = [...events]
    .map((ev) => ({ ev, tsMs: tryParseTs(ev.ts) }))
    .sort((a, b) => b.tsMs - a.tsMs)
    .slice(0, 10);

  return (
    <section
      data-testid="dashboard-tier-early"
      className="border border-[var(--border)] rounded-lg p-5"
    >
      <h2 className="font-medium mb-3">Recent events</h2>
      <ul className="space-y-2">
        {/*
          W20b-1 fix-commit (code-reviewer MEDIUM): carry the parsed
          `tsMs` value through from the sort step into the render
          instead of re-parsing `ev.ts` per row. Previously each event
          paid two `Date.parse` calls (sort + render); this halves
          that.

          W20b-2 fix-commit (code-reviewer MEDIUM): same pattern for
          `payloadNodeIdLabel`. It was called twice per row (once for
          `title`, once for the rendered text). Extract once per row
          so the defensive narrowing chain runs N times instead of 2N.
        */}
        {sorted.map(({ ev, tsMs }) => {
          const nodeId = payloadNodeIdLabel(ev);
          return (
            <li
              key={ev.event_hash}
              className="flex items-center gap-3 text-[13px]"
              data-testid="dashboard-early-event"
            >
              <span className="text-[var(--foreground-muted)] w-20 shrink-0">
                {formatRelative(tsMs)}
              </span>
              <span className="font-medium w-32 shrink-0">
                {payloadKindLabel(ev)}
              </span>
              <span
                data-testid="dashboard-early-event-id"
                className="flex-1 truncate text-[var(--foreground-muted)]"
                title={nodeId}
              >
                {nodeId}
              </span>
              <code className="hash-chip break-all">
                {typeof ev.event_hash === "string"
                  ? ev.event_hash.slice(0, 12)
                  : "—"}
              </code>
            </li>
          );
        })}
      </ul>
      <p className="text-[12px] text-[var(--foreground-muted)] mt-3">
        The full KPI dashboard activates once you have 11+ events.
      </p>
    </section>
  );
}

// ───────────────────────── FullTier ─────────────────────────

interface FullTierProps {
  metrics: WorkspaceMetrics;
  workspaces: ReadonlyArray<string>;
}

function FullTier({ metrics, workspaces }: FullTierProps): React.ReactElement {
  const eventsLast30dValue = metrics.eventsLast30d.toLocaleString();
  const eventsLast30dSub =
    metrics.eventsLast30dPrior === 0
      ? "no prior data"
      : describeDelta(metrics.eventsLast30d, metrics.eventsLast30dPrior);

  const anchorPct =
    metrics.totalEvents === 0
      ? 0
      : Math.round((metrics.anchorCount / metrics.totalEvents) * 100);
  const anchorSub =
    metrics.anchorCount === 0
      ? "anchoring V2-γ (currently 0%)"
      : "Sigstore-anchored events";

  const workspacesSub = (() => {
    if (workspaces.length === 0) return "no workspaces listed";
    const joined = workspaces.join(", ");
    return joined.length > 60 ? `${joined.slice(0, 57)}…` : joined;
  })();

  return (
    <div className="space-y-4" data-testid="dashboard-tier-full">
      <section className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <KpiCard
          label="Events (last 30d)"
          value={eventsLast30dValue}
          sub={eventsLast30dSub}
        />
        <KpiCard label="Sig-valid" value="—" sub="run Live Verifier panel" />
        <KpiCard
          label="Anchor coverage"
          value={`${anchorPct}%`}
          sub={anchorSub}
        />
        <KpiCard
          label="Pending policy violations"
          value="0"
          sub="policy engine V2-δ"
        />
      </section>
      <section className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <KpiCard
          label="Active workspaces"
          value={workspaces.length.toLocaleString()}
          sub={workspacesSub}
        />
        <KpiCard
          label="Unique signers"
          value={metrics.uniqueSigners.length.toLocaleString()}
          sub="DIDs signing into this workspace"
        />
        <KpiCard
          label="DAG depth"
          value={`${metrics.dagDepth} hops`}
          sub="max parent-chain depth"
        />
      </section>
    </div>
  );
}

// ───────────────────────── helpers ─────────────────────────

function tryParseTs(ts: unknown): number {
  if (typeof ts !== "string") return Number.NEGATIVE_INFINITY;
  const ms = Date.parse(ts);
  return Number.isNaN(ms) ? Number.NEGATIVE_INFINITY : ms;
}

function payloadKindLabel(ev: AtlasEvent): string {
  if (
    typeof ev.payload === "object" &&
    ev.payload !== null &&
    !Array.isArray(ev.payload)
  ) {
    const t = (ev.payload as Record<string, unknown>).type;
    if (typeof t === "string") return t;
  }
  return "(untyped)";
}

/**
 * W20b-2 — surface the user-supplied node id (e.g. "dataset-x" from a
 * `node.create` event) in the EarlyTier event row. Defensive at every
 * level — `payload`, `payload.node`, `payload.node.id` are all
 * narrowed before use; any malformed shape sinks to "—" rather than
 * crashing the dashboard. Mirrors the discipline in `payloadKindLabel`.
 */
function payloadNodeIdLabel(ev: AtlasEvent): string {
  if (
    typeof ev.payload === "object" &&
    ev.payload !== null &&
    !Array.isArray(ev.payload)
  ) {
    const node = (ev.payload as Record<string, unknown>).node;
    if (typeof node === "object" && node !== null && !Array.isArray(node)) {
      const id = (node as Record<string, unknown>).id;
      if (typeof id === "string" && id.length > 0) return id;
    }
  }
  return "—";
}

function describeDelta(current: number, prior: number): string {
  const diff = current - prior;
  const sign = diff >= 0 ? "+" : "";
  return `${sign}${diff.toLocaleString()} vs prior period`;
}

function formatRelative(tsMs: number): string {
  if (!Number.isFinite(tsMs)) return "—";
  const diffMs = Date.now() - tsMs;
  if (diffMs < 0) return "future";
  const sec = Math.floor(diffMs / 1000);
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day}d ago`;
  const month = Math.floor(day / 30);
  if (month < 12) return `${month}mo ago`;
  const year = Math.floor(day / 365);
  return `${year}y ago`;
}
