import Link from "next/link";
import { LiveVerifierPanel } from "@/components/LiveVerifierPanel";

export default function Page() {
  return (
    <div className="space-y-8">
      <section>
        <h1 className="text-2xl font-semibold tracking-tight mb-1">Audit Readiness</h1>
        <p className="text-[var(--foreground-muted)]">
          Workspace state at a glance. The cards below are illustrative dashboard
          values — the only cryptographic verification on this page runs in the
          Live Verifier Panel further down.
        </p>
      </section>

      <section className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <KpiCard label="Events (last 30d)" value="47,291" sub="+12.4% vs prior period" />
        <KpiCard label="Sig-valid" value="—" sub="run Live Verifier panel" />
        <KpiCard label="Anchor coverage" value="98.7%" sub="dashboard preview" />
        <KpiCard label="Pending policy violations" value="0" sub="dashboard preview" />
      </section>

      <section className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <KpiCard label="Active workspaces" value="3" sub="Bank prod / dev / sandbox" />
        <KpiCard label="Human verifiers" value="12" sub="SPIFFE-bound identities" />
        <KpiCard label="Data lineage depth" value="7 hops" sub="max DAG path observed" />
      </section>

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
    </div>
  );
}

function KpiCard({
  label,
  value,
  sub,
  trust,
}: {
  label: string;
  value: string;
  sub: string;
  trust?: "ok" | "fail";
}) {
  return (
    <div className="border border-[var(--border)] rounded-lg p-4 bg-[var(--background)]">
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
