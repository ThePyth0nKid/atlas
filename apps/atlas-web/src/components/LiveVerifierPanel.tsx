"use client";

import { useEffect, useState } from "react";
import { runVerifier, type VerifyOutcome } from "@/lib/verifier-loader";

type Status = "loading-wasm" | "fetching-trace" | "verifying" | "done" | "error";

export function LiveVerifierPanel() {
  const [status, setStatus] = useState<Status>("loading-wasm");
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<VerifyOutcome | null>(null);
  const [verifierVersion, setVerifierVersion] = useState<string | null>(null);
  const [traceMeta, setTraceMeta] = useState<{ workspace: string; events: number } | null>(null);

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
          throw new Error("could not load golden trace fixture");
        }
        const traceJson = await traceRes.text();
        const bundleJson = await bundleRes.text();

        if (cancelled) return;
        setStatus("verifying");

        const result = await runVerifier(traceJson, bundleJson);
        if (cancelled) return;
        const trace = JSON.parse(traceJson);

        setVerifierVersion(result.verifierVersion);
        setOutcome(result.outcome);
        setTraceMeta({ workspace: trace.workspace_id, events: trace.events.length });
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
    <section className="border border-[var(--border)] rounded-lg p-5 bg-[var(--bg-subtle)]">
      <div className="flex items-start justify-between gap-6">
        <div>
          <h2 className="font-medium flex items-center gap-2">
            Live Verifier
            <StatusBadge status={status} valid={outcome?.valid ?? null} />
          </h2>
          <p className="text-[var(--foreground-muted)] mt-1 max-w-2xl">
            The Rust verifier (compiled to WebAssembly) is running in your browser, not on our server.
            Same crate as <code className="hash-chip">atlas-verify-cli</code>. Bit-identical determinism.
          </p>
        </div>
        {verifierVersion && (
          <span className="hash-chip">{verifierVersion}</span>
        )}
      </div>

      {traceMeta && (
        <div className="mt-3 text-[13px] text-[var(--foreground-muted)]">
          Workspace <code className="hash-chip">{traceMeta.workspace}</code>{" "}
          · {traceMeta.events} events
        </div>
      )}

      {error && (
        <div className="mt-3 text-[var(--accent-danger)] text-[13px]">
          {error}
        </div>
      )}

      {outcome && (
        <ul className="mt-4 space-y-1.5">
          {outcome.evidence.map((ev) => (
            <li key={ev.check} className="flex items-start gap-2 text-[13px]">
              <span className={`trust-tick ${ev.ok ? "trust-tick--ok" : "trust-tick--fail"}`}>
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
}) {
  if (status === "done") {
    return (
      <span
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
      <span className="text-[11px] px-2 py-0.5 rounded-full bg-[color-mix(in_srgb,var(--accent-danger)_15%,transparent)] text-[var(--accent-danger)]">
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
    <span className="text-[11px] px-2 py-0.5 rounded-full bg-[var(--bg-muted)] text-[var(--foreground-muted)]">
      {labels[status]}
    </span>
  );
}
