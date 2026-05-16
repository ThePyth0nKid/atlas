/**
 * W20b-1 — home page (Audit Readiness).
 *
 * The page now renders real workspace state via
 * `<DashboardMetricsSection>`, replacing the hard-coded marketing
 * KPIs that lived inline in W20a. The page header copy is updated
 * accordingly — the "illustrative values" disclaimer has been
 * removed because every number on the page below is computed from
 * the active workspace's signed events.
 *
 * Structure:
 *   - heading + accurate sub-copy
 *   - <DashboardMetricsSection>   (3-tier metrics: empty / early / full)
 *   - <LiveVerifierPanel>         (unchanged from W20a — frozen contract)
 *   - "regulator" CTA section     (unchanged — points at /audit-export,
 *                                  which is now nav-disabled but the
 *                                  CTA itself remains so users can find
 *                                  the W20c roadmap entry)
 *   - <StatusDisclosureFooter>    (W20b-1 — roadmap status)
 */

import Link from "next/link";
import { DashboardMetricsSection } from "@/components/DashboardMetricsSection";
import { LiveVerifierPanel } from "@/components/LiveVerifierPanel";
import { StatusDisclosureFooter } from "@/components/StatusDisclosureFooter";

export default function Page() {
  return (
    <div className="space-y-8">
      <section>
        <h1 className="text-2xl font-semibold tracking-tight mb-1">
          Audit Readiness
        </h1>
        <p className="text-[var(--foreground-muted)]">
          Workspace state at a glance. All numbers below are computed from
          your actual signed events.
        </p>
      </section>

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
    </div>
  );
}
