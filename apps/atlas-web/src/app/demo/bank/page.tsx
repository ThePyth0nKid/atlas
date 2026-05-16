/**
 * W20b-1 — /demo/bank showcase route.
 *
 * Preserves the pre-W20a polished marketing dashboard against a
 * static bank-persona fixture. The route is explicitly framed as
 * "illustrative dataset" so prospects can see the populated KPI cards
 * and verifier panel without confusing them with their own workspace.
 *
 * Threat model (TM-W20b-2): same-origin with the real app. The
 * disclaimer paragraph + the "(demo)" suffix on each KPI's `sub` line
 * + the link back to the user's actual dashboard at the bottom keep
 * the showcase legibly separate from real data.
 *
 * No client-side state. The single dynamic block is the
 * `BankDemoVerifierPanel`, which is itself a "use client" component
 * that fetches /api/golden/bank-* on mount.
 */

import Link from "next/link";
import { BankDemoVerifierPanel } from "@/components/BankDemoVerifierPanel";
import { KpiCard } from "@/components/KpiCard";

export default function BankDemoPage(): React.ReactElement {
  return (
    <div className="space-y-8">
      <section>
        <h1 className="text-2xl font-semibold tracking-tight mb-1">
          Bank demo — illustrative dataset
        </h1>
        <p className="text-[var(--foreground-muted)]">
          This page shows a static fixture from our bank-persona demo. The
          workspace selector in the header still points to your actual
          workspace.
        </p>
      </section>
      <section
        className="grid grid-cols-1 md:grid-cols-4 gap-4"
        data-testid="bank-demo-kpi-row-primary"
      >
        <KpiCard
          label="Events (last 30d)"
          value="47,291"
          sub="+12.4% vs prior period"
        />
        <KpiCard label="Sig-valid" value="—" sub="run Live Verifier panel" />
        <KpiCard
          label="Anchor coverage"
          value="98.7%"
          sub="bank-demo fixture"
        />
        <KpiCard
          label="Pending policy violations"
          value="0"
          sub="bank-demo fixture"
        />
      </section>
      <section
        className="grid grid-cols-1 md:grid-cols-3 gap-4"
        data-testid="bank-demo-kpi-row-secondary"
      >
        <KpiCard
          label="Active workspaces"
          value="3"
          sub="Bank prod / dev / sandbox (demo)"
        />
        <KpiCard
          label="Human verifiers"
          value="12"
          sub="SPIFFE-bound identities (demo)"
        />
        <KpiCard
          label="Data lineage depth"
          value="7 hops"
          sub="max DAG path observed (demo)"
        />
      </section>
      <BankDemoVerifierPanel />
      <p className="text-[12px] text-[var(--foreground-muted)] italic">
        Return to your workspace dashboard:{" "}
        <Link href="/" className="underline">
          → Audit Readiness
        </Link>
      </p>
    </div>
  );
}
