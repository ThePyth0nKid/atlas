/**
 * W20b-2 — home page (Audit Readiness).
 *
 * The page now branches between two top-level UIs based on whether
 * the user has at least one workspace:
 *   - No workspaces → `<FirstRunWizard>` (full-page empty state)
 *   - At least one  → existing dashboard tree (W20b-1)
 *
 * The branch lives in `<HomeContent>` (client component) so the page
 * itself can stay structurally a server component. Header + heading
 * copy remain on the server.
 *
 * Structure when workspaces > 0:
 *   - heading + accurate sub-copy
 *   - <DashboardMetricsSection>   (3-tier metrics: empty / early / full)
 *   - <LiveVerifierPanel>         (unchanged from W20a — frozen contract)
 *   - "regulator" CTA section     (unchanged — points at /audit-export,
 *                                  which is now nav-disabled but the
 *                                  CTA itself remains so users can find
 *                                  the W20c roadmap entry)
 *   - <StatusDisclosureFooter>    (W20b-1 — roadmap status)
 */

import { HomeContent } from "@/components/HomeContent";

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

      <HomeContent />
    </div>
  );
}
