"use client";

/**
 * W20b-1 — shared KpiCard component.
 *
 * Extracted verbatim from the inline definition that lived in
 * `apps/atlas-web/src/app/page.tsx` (W20a). The shape and visual
 * language is preserved; the only addition is a stable
 * `data-testid="kpi-card"` on the outer div so the Playwright
 * `dashboard-tiers.spec.ts` can count KPI cards without coupling to
 * the per-label text (which now varies by tier and workspace).
 *
 * Consumers:
 *   - `/` home page (W20b-1 FullTier — real workspace metrics)
 *   - `/demo/bank` showcase page (W20b-1 — illustrative fixture)
 *
 * Frozen contract: `data-testid="kpi-card"` is asserted by
 * `tests/e2e/dashboard-tiers.spec.ts`. Renaming or removing it without
 * updating the spec turns the atlas-web-playwright lane red.
 */

interface KpiCardProps {
  label: string;
  value: string;
  sub: string;
  trust?: "ok" | "fail";
}

export function KpiCard({
  label,
  value,
  sub,
  trust,
}: KpiCardProps): React.ReactElement {
  return (
    <div
      data-testid="kpi-card"
      className="border border-[var(--border)] rounded-lg p-4 bg-[var(--background)]"
    >
      <div className="flex items-center gap-2">
        <span className="text-[12px] uppercase tracking-wide text-[var(--foreground-muted)]">
          {label}
        </span>
        {trust === "ok" && <span className="trust-tick trust-tick--ok">✓</span>}
        {trust === "fail" && <span className="trust-tick trust-tick--fail">✗</span>}
      </div>
      <div className="mt-2 text-2xl font-semibold tracking-tight">{value}</div>
      <div className="mt-1 text-[12px] text-[var(--foreground-muted)]">{sub}</div>
    </div>
  );
}
