"use client";

/**
 * W20b-1 — Atlas roadmap status disclosure footer.
 *
 * Renders the public-facing roadmap status on the home page so users
 * know which layers are operational vs. coming-soon. Atlas's
 * transparency posture: be explicit about what's shipped and what's
 * not, both in marketing copy and in the dashboard chrome.
 *
 * Threat model (TM-W20b-3): mentions PR numbers in human-readable
 * status. PR numbers are public on GitHub — no info disclosure risk.
 * The component is a static render with no fetch/data path.
 *
 * Frozen testid: `status-disclosure-footer` (Playwright
 * dashboard-tiers.spec.ts).
 */

export function StatusDisclosureFooter(): React.ReactElement {
  return (
    <section
      className="border-t border-[var(--border)] pt-6 mt-12 text-[12px] text-[var(--foreground-muted)]"
      data-testid="status-disclosure-footer"
    >
      <h3 className="text-[12px] uppercase tracking-wide font-medium mb-2">
        Atlas roadmap status
      </h3>
      <ul className="grid grid-cols-1 md:grid-cols-2 gap-y-1 gap-x-6">
        <li>
          <span
            className="text-[var(--accent-trust)]"
            aria-label="operational"
          >
            ●
          </span>{" "}
          Layer 1 verifier (events) — operational
        </li>
        <li>
          <span
            className="text-[var(--accent-trust)]"
            aria-label="operational"
          >
            ●
          </span>{" "}
          Layer 2 graph (ArcadeDB) — operational
        </li>
        <li>
          <span
            className="text-[var(--accent-trust)]"
            aria-label="operational"
          >
            ●
          </span>{" "}
          Layer 3 embedder — operational (PR #108 polish pending)
        </li>
        <li>
          <span className="text-[var(--accent-warn)]" aria-label="planned">
            ●
          </span>{" "}
          HTML vault format — W30 roadmap
        </li>
        <li>
          <span className="text-[var(--accent-warn)]" aria-label="planned">
            ●
          </span>{" "}
          Desktop installer — W40 roadmap
        </li>
        <li>
          <span className="text-[var(--accent-warn)]" aria-label="planned">
            ●
          </span>{" "}
          First-run wizard — W20b-2 next
        </li>
      </ul>
    </section>
  );
}
