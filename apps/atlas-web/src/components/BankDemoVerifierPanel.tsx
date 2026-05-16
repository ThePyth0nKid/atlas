"use client";

/**
 * W20b-1 — self-contained verifier panel for the /demo/bank showcase
 * route.
 *
 * Mirrors the PRE-W20a shape of `LiveVerifierPanel`: hard-coded to the
 * golden bank-demo fixture, no `WorkspaceContext` coupling. This is the
 * pre-W20a marketing demo that lived inline on `/`; we preserve it as a
 * dedicated showcase route so prospects can still see the
 * polished-with-a-real-dataset experience without confusing it with
 * their own workspace state.
 *
 * Threat model (TM-W20b-2):
 *   Same-origin with the real workspace UI — the disclaimer copy on
 *   the surrounding `/demo/bank` page makes it unambiguous that this
 *   is illustrative data. The component itself uses distinct
 *   `bank-demo-*` testids so the Playwright suite cannot
 *   accidentally cross-match against `live-verifier-panel` selectors.
 *
 * Frozen testids (Playwright suite):
 *   - bank-demo-verifier-panel
 *   - bank-demo-verifier-status-badge
 *   - bank-demo-verifier-version
 *   - bank-demo-verifier-evidence
 *   - bank-demo-verifier-error
 */

import { useEffect, useState } from "react";
import { runVerifier, type VerifyOutcome } from "@/lib/verifier-loader";

type Status =
  | "loading-wasm"
  | "fetching-trace"
  | "verifying"
  | "done"
  | "error";

export function BankDemoVerifierPanel(): React.ReactElement {
  const [status, setStatus] = useState<Status>("fetching-trace");
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<VerifyOutcome | null>(null);
  const [verifierVersion, setVerifierVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        setStatus("fetching-trace");
        const [traceRes, bundleRes] = await Promise.all([
          fetch("/api/golden/bank-trace"),
          fetch("/api/golden/bank-bundle"),
        ]);
        if (!traceRes.ok || !bundleRes.ok) {
          throw new Error(
            `could not load bank-demo fixture (trace: ${traceRes.status}, bundle: ${bundleRes.status})`,
          );
        }
        const traceJson = await traceRes.text();
        const bundleJson = await bundleRes.text();
        if (cancelled) return;

        setStatus("verifying");
        const result = await runVerifier(traceJson, bundleJson);
        if (cancelled) return;
        setVerifierVersion(result.verifierVersion);
        setOutcome(result.outcome);
        setStatus("done");
      } catch (e) {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
        setStatus("error");
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <section
      data-testid="bank-demo-verifier-panel"
      className="border border-[var(--border)] rounded-lg p-5 bg-[var(--bg-subtle)]"
    >
      <div className="flex items-start justify-between gap-6">
        <div>
          <h2 className="font-medium flex items-center gap-2">
            Live Verifier (bank-demo fixture)
            <StatusBadge status={status} valid={outcome?.valid ?? null} />
          </h2>
          <p className="text-[var(--foreground-muted)] mt-1 max-w-2xl">
            Same WASM verifier as the home-page panel, running against a
            static bank-persona fixture so you can see a fully-populated
            evidence list. Not your workspace.
          </p>
        </div>
        {verifierVersion && (
          <span className="hash-chip" data-testid="bank-demo-verifier-version">
            {verifierVersion}
          </span>
        )}
      </div>

      {error && (
        <div
          className="mt-3 text-[var(--accent-danger)] text-[13px]"
          role="alert"
          data-testid="bank-demo-verifier-error"
        >
          {error}
        </div>
      )}

      {outcome && (
        <ul
          className="mt-4 space-y-1.5"
          data-testid="bank-demo-verifier-evidence"
        >
          {outcome.evidence.map((ev) => (
            <li
              key={ev.check}
              className="flex items-start gap-2 text-[13px]"
              data-testid={`bank-demo-verifier-evidence-${ev.check}`}
            >
              <span
                className={`trust-tick ${ev.ok ? "trust-tick--ok" : "trust-tick--fail"}`}
                aria-hidden="true"
              >
                {ev.ok ? "✓" : "✗"}
              </span>
              <span className="font-medium min-w-[150px]">{ev.check}</span>
              <span className="text-[var(--foreground-muted)]">{ev.detail}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function StatusBadge({
  status,
  valid,
}: {
  status: Status;
  valid: boolean | null;
}): React.ReactElement {
  if (status === "done") {
    return (
      <span
        data-testid="bank-demo-verifier-status-badge"
        data-status="done"
        data-valid={valid ? "true" : "false"}
        className={`text-[11px] px-2 py-0.5 rounded-full ${
          valid
            ? "bg-[color-mix(in_srgb,var(--accent-trust)_15%,transparent)] text-[var(--accent-trust)]"
            : "bg-[color-mix(in_srgb,var(--accent-danger)_15%,transparent)] text-[var(--accent-danger)]"
        }`}
      >
        {valid ? "VALID" : "INVALID"}
      </span>
    );
  }
  if (status === "error") {
    return (
      <span
        data-testid="bank-demo-verifier-status-badge"
        data-status="error"
        className="text-[11px] px-2 py-0.5 rounded-full bg-[color-mix(in_srgb,var(--accent-danger)_15%,transparent)] text-[var(--accent-danger)]"
      >
        ERROR
      </span>
    );
  }
  const labels: Record<Status, string> = {
    "loading-wasm": "loading wasm…",
    "fetching-trace": "fetching trace…",
    verifying: "verifying…",
    done: "",
    error: "",
  };
  return (
    <span
      data-testid="bank-demo-verifier-status-badge"
      data-status={status}
      className="text-[11px] px-2 py-0.5 rounded-full bg-[var(--bg-muted)] text-[var(--foreground-muted)]"
    >
      {labels[status]}
    </span>
  );
}
