"use client";

/**
 * W20b-2 — home-page content branching.
 *
 * The home page is structurally a server component (no client hooks
 * at the page level — `<DashboardMetricsSection>` and
 * `<LiveVerifierPanel>` carry their own `"use client"` directives).
 * Branching wizard-vs-dashboard requires reading the workspace
 * context, which is a client hook; rather than promote the whole
 * page to a client component, we extract the branch here.
 *
 * Behaviour:
 *   - While the workspaces fetch is in flight, render a small
 *     placeholder (matches the `DashboardMetricsSection` loading
 *     style) so the page doesn't flash from wizard → dashboard or
 *     vice versa.
 *   - If the fetch resolves with `workspaces.length === 0`, render
 *     `<FirstRunWizard />` INSTEAD of the dashboard, live verifier,
 *     and status footer. The user has nothing to look at yet — the
 *     wizard is the entire affordance.
 *   - Otherwise, render the full dashboard tree.
 */

import { useEffect, useState } from "react";
import { DashboardMetricsSection } from "@/components/DashboardMetricsSection";
import { FirstRunWizard } from "@/components/FirstRunWizard";
import { LayerStatusPanel } from "@/components/LayerStatusPanel";
import { LiveVerifierPanel } from "@/components/LiveVerifierPanel";
import { StatusDisclosureFooter } from "@/components/StatusDisclosureFooter";
import { useWorkspaceContext } from "@/lib/workspace-context";
import type {
  BackendStatus,
  EmbedderStatus,
  LayerStatus,
  SignerStatus,
} from "@/lib/system-health";
import Link from "next/link";

/**
 * W20c — shared hook for the /api/atlas/system/health response.
 *
 * The home page consumes the layer status TWICE: once for the
 * `<LayerStatusPanel>` (above LiveVerifierPanel) and once for the
 * 3-tier degradation in `<DashboardMetricsSection>` (DA-4). Lifting
 * the fetch into `<HomeContent>` and passing the result down avoids
 * two network roundtrips per page load.
 */
type LayerStatusFetchState =
  | { kind: "loading" }
  | { kind: "ready"; status: LayerStatus }
  | { kind: "error"; message: string };

interface LayerStatusResponseShape {
  ok: boolean;
  signer?: unknown;
  embedder?: unknown;
  backend?: unknown;
  error?: string;
}

const SIGNER_VALUES: ReadonlySet<SignerStatus> = new Set([
  "operational",
  "unconfigured",
]);
const EMBEDDER_VALUES: ReadonlySet<EmbedderStatus> = new Set([
  "operational",
  "model_missing",
  "unsupported",
]);
const BACKEND_VALUES: ReadonlySet<BackendStatus> = new Set([
  "operational",
  "stub_501",
  "fault",
]);

function useLayerStatus(): LayerStatusFetchState {
  const [state, setState] = useState<LayerStatusFetchState>({
    kind: "loading",
  });
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await fetch("/api/atlas/system/health");
        if (!res.ok) {
          throw new Error(`health probe failed (HTTP ${res.status})`);
        }
        const body = (await res.json()) as LayerStatusResponseShape;
        if (
          !body.ok ||
          typeof body.signer !== "string" ||
          !SIGNER_VALUES.has(body.signer as SignerStatus) ||
          typeof body.embedder !== "string" ||
          !EMBEDDER_VALUES.has(body.embedder as EmbedderStatus) ||
          typeof body.backend !== "string" ||
          !BACKEND_VALUES.has(body.backend as BackendStatus)
        ) {
          throw new Error(
            typeof body.error === "string" && body.error.length > 0
              ? body.error
              : "health response missing expected fields",
          );
        }
        if (cancelled) return;
        setState({
          kind: "ready",
          status: {
            signer: body.signer as SignerStatus,
            embedder: body.embedder as EmbedderStatus,
            backend: body.backend as BackendStatus,
          },
        });
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
  }, []);
  return state;
}

export function HomeContent(): React.ReactElement {
  const { workspaces, loading } = useWorkspaceContext();
  const layerStatus = useLayerStatus();

  if (loading) {
    return (
      <div
        className="text-[13px] text-[var(--foreground-muted)]"
        data-testid="home-content-loading"
      >
        Loading workspace…
      </div>
    );
  }

  if (workspaces.length === 0) {
    return <FirstRunWizard />;
  }

  // Resolve the layer status for the metrics section (DA-4).
  // Loading and error states pass `null` so DashboardMetricsSection
  // defers to its event-count-only tier choice; once the probe
  // resolves, signer === 'unconfigured' degrades any tier to Empty.
  const resolvedLayerStatus =
    layerStatus.kind === "ready" ? layerStatus.status : null;

  return (
    <>
      <LayerStatusPanel />

      <DashboardMetricsSection layerStatus={resolvedLayerStatus} />

      <LiveVerifierPanel />

      <section className="border border-[var(--border)] rounded-lg p-5">
        <div className="flex items-start justify-between gap-6">
          <div>
            <h2 className="font-medium">Need to answer a regulator?</h2>
            <p className="text-[var(--foreground-muted)] mt-1 max-w-2xl">
              Filter by period and system, click <em>Export</em>, get a signed PDF/A bundle and
              a standalone HTML verifier the auditor can run offline — without our server.
            </p>
          </div>
          <Link
            href="/audit-export"
            className="text-[13px] font-medium border border-[var(--border)] rounded-md px-3 py-1.5 hover:bg-[var(--bg-subtle)]"
          >
            Open Audit Export →
          </Link>
        </div>
      </section>

      <StatusDisclosureFooter />
    </>
  );
}
