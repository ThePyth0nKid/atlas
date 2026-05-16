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

import { DashboardMetricsSection } from "@/components/DashboardMetricsSection";
import { FirstRunWizard } from "@/components/FirstRunWizard";
import { LiveVerifierPanel } from "@/components/LiveVerifierPanel";
import { StatusDisclosureFooter } from "@/components/StatusDisclosureFooter";
import { useWorkspaceContext } from "@/lib/workspace-context";
import Link from "next/link";

export function HomeContent(): React.ReactElement {
  const { workspaces, loading } = useWorkspaceContext();

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

  return (
    <>
      <DashboardMetricsSection />

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
