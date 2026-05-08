/**
 * V1.19 Welle 1 — /write page.
 *
 * Manual write surface for the Atlas knowledge graph. The form
 * captures one node-creation event at a time and POSTs to
 * `/api/atlas/write-node`, which signs the event via `atlas-signer`
 * and appends it to `data/{workspace}/events.jsonl`.
 *
 * Why a server-rendered shell + client form? The shell carries no
 * sensitive state; the form needs `useState` for input + result, so
 * it's a client component. Keeping the route's outer page server-
 * rendered means crawlers and operators can hit `/write` and see the
 * "deployer security boundary" notice even with JS disabled.
 *
 * No client-side auth. None. The deployer is responsible for putting
 * this behind a proxy / VPN / IP allowlist. See OPERATOR-RUNBOOK §17.
 */

import { WriteNodeForm } from "@/components/WriteNodeForm";

export default function WritePage() {
  return (
    <div className="space-y-8">
      <section>
        <h1 className="text-2xl font-semibold tracking-tight mb-1">Write Surface</h1>
        <p className="text-[var(--foreground-muted)] max-w-2xl">
          Record a new node in the workspace's append-only DAG. The event is
          signed with the workspace's per-tenant Ed25519 key and appended to{" "}
          <code className="hash-chip">events.jsonl</code>. The Live Verifier on
          the home page reads the same trace and confirms{" "}
          <span className="trust-tick trust-tick--ok">✓</span>.
        </p>
      </section>

      <DeployerNotice />

      <WriteNodeForm />
    </div>
  );
}

function DeployerNotice() {
  return (
    <section className="border border-[var(--border)] rounded-lg p-4 text-[13px] text-[var(--foreground-muted)]">
      <div className="font-medium text-[var(--foreground)] mb-1">
        Deployer security boundary
      </div>
      <p>
        This route is unauthenticated by design — Atlas's trust model is
        key-based, not user-based. Every event records its signing{" "}
        <code className="hash-chip">kid</code>; auditors verify the signature
        chain, not a session cookie. If you expose this surface on a public
        address without a network/proxy auth gate, anyone can write under your
        workspace's per-tenant kid. See OPERATOR-RUNBOOK §17 for the deployment
        checklist.
      </p>
    </section>
  );
}
