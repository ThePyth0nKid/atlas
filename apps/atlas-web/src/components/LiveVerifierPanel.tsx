"use client";

/**
 * V1.19 Welle 11 — frozen `data-testid` test seam (preserved across W20a).
 * The following data-testid identifiers are pinned by the Playwright
 * E2E suite in `apps/atlas-web/tests/e2e/home.spec.ts` and
 * `tests/e2e/workspace-selector.spec.ts`. They MUST remain present
 * and semantically equivalent across refactors:
 *   - live-verifier-panel    : the outer section
 *   - verifier-status-badge  : the StatusBadge element
 *   - verifier-version       : the verifier_version chip (when present)
 *   - verifier-trace-meta    : workspace + events count line
 *   - verifier-evidence      : the evidence <ul>
 *   - verifier-error         : error message (when present)
 *
 * W20a additions (also pinned by tests):
 *   - verifier-empty-state   : rendered when the active workspace has
 *                              zero events (replaces the evidence list
 *                              and the VALID/INVALID badge with a
 *                              friendly call-to-action).
 *   - verifier-loading-workspace : rendered while the workspace context
 *                              is still resolving (no workspace set).
 *
 * Renaming or removing any of these without updating the spec files in
 * the same PR turns the atlas-web-playwright CI lane red.
 */

import { useEffect, useState } from "react";
import { runVerifier, type VerifyOutcome } from "@/lib/verifier-loader";
import { useWorkspaceContext } from "@/lib/workspace-context";

type Status =
  | "waiting-workspace"
  | "loading-wasm"
  | "fetching-trace"
  | "verifying"
  | "empty"
  | "done"
  | "error";

interface TraceShape {
  workspace_id: string;
  events: unknown[];
}

export function LiveVerifierPanel() {
  const { workspace } = useWorkspaceContext();
  const [status, setStatus] = useState<Status>("waiting-workspace");
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<VerifyOutcome | null>(null);
  const [verifierVersion, setVerifierVersion] = useState<string | null>(null);
  const [traceMeta, setTraceMeta] = useState<{ workspace: string; events: number } | null>(null);

  useEffect(() => {
    if (workspace === null) {
      // Context still loading — clear any prior state and wait.
      setStatus("waiting-workspace");
      setError(null);
      setOutcome(null);
      setVerifierVersion(null);
      setTraceMeta(null);
      return;
    }

    let cancelled = false;

    // Reset prior outcome on workspace change so the previous
    // workspace's evidence doesn't briefly flash next to the new
    // workspace's metadata.
    setError(null);
    setOutcome(null);
    setVerifierVersion(null);
    setTraceMeta(null);

    (async () => {
      try {
        setStatus("fetching-trace");
        const wsParam = encodeURIComponent(workspace);
        const [traceRes, bundleRes] = await Promise.all([
          fetch(`/api/atlas/trace?workspace=${wsParam}`),
          fetch(`/api/atlas/pubkey-bundle?workspace=${wsParam}`),
        ]);
        if (!traceRes.ok || !bundleRes.ok) {
          throw new Error(
            `could not load trace/bundle (trace: ${traceRes.status}, bundle: ${bundleRes.status})`,
          );
        }
        const traceJson = await traceRes.text();
        const bundleJson = await bundleRes.text();

        if (cancelled) return;

        // Parse minimally to inspect the events count — full parsing
        // is the WASM verifier's job.
        let trace: TraceShape;
        try {
          trace = JSON.parse(traceJson) as TraceShape;
        } catch (e) {
          throw new Error(`trace JSON parse failed: ${(e as Error).message}`);
        }
        const eventsCount = Array.isArray(trace.events) ? trace.events.length : 0;
        setTraceMeta({ workspace: trace.workspace_id, events: eventsCount });

        if (eventsCount === 0) {
          // Empty workspace — skip the verifier (it would correctly
          // pass on an empty event set, but rendering a "VALID" badge
          // with zero evidence rows is misleading UX). Show a
          // call-to-action instead.
          if (cancelled) return;
          setStatus("empty");
          return;
        }

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
  }, [workspace]);

  return (
    <section
      data-testid="live-verifier-panel"
      className="border border-[var(--border)] rounded-lg p-5 bg-[var(--bg-subtle)]"
    >
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
          <span className="hash-chip" data-testid="verifier-version">
            {verifierVersion}
          </span>
        )}
      </div>

      {status === "waiting-workspace" && (
        <div
          className="mt-3 text-[13px] text-[var(--foreground-muted)]"
          data-testid="verifier-loading-workspace"
        >
          Loading workspace…
        </div>
      )}

      {traceMeta && (
        <div
          className="mt-3 text-[13px] text-[var(--foreground-muted)]"
          data-testid="verifier-trace-meta"
        >
          Workspace <code className="hash-chip">{traceMeta.workspace}</code>{" "}
          · {traceMeta.events} events
        </div>
      )}

      {status === "empty" && (
        <div
          className="mt-4 border border-dashed border-[var(--border)] rounded-md p-4 text-[13px] text-[var(--foreground-muted)]"
          data-testid="verifier-empty-state"
        >
          This workspace has no events yet.{" "}
          <a
            href="/write"
            className="underline hover:text-[var(--foreground)]"
          >
            Write your first fact at /write →
          </a>
        </div>
      )}

      {error && (
        <div
          className="mt-3 text-[var(--accent-danger)] text-[13px]"
          role="alert"
          data-testid="verifier-error"
        >
          {error}
        </div>
      )}

      {outcome && (
        <ul className="mt-4 space-y-1.5" data-testid="verifier-evidence">
          {outcome.evidence.map((ev) => (
            <li
              key={ev.check}
              className="flex items-start gap-2 text-[13px]"
              data-testid={`verifier-evidence-${ev.check}`}
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
}) {
  if (status === "done") {
    return (
      <span
        data-testid="verifier-status-badge"
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
        data-testid="verifier-status-badge"
        data-status="error"
        className="text-[11px] px-2 py-0.5 rounded-full bg-[color-mix(in_srgb,var(--accent-danger)_15%,transparent)] text-[var(--accent-danger)]"
      >
        ERROR
      </span>
    );
  }
  if (status === "empty") {
    return (
      <span
        data-testid="verifier-status-badge"
        data-status="empty"
        className="text-[11px] px-2 py-0.5 rounded-full bg-[var(--bg-muted)] text-[var(--foreground-muted)]"
      >
        EMPTY
      </span>
    );
  }
  if (status === "waiting-workspace") {
    return (
      <span
        data-testid="verifier-status-badge"
        data-status="waiting-workspace"
        className="text-[11px] px-2 py-0.5 rounded-full bg-[var(--bg-muted)] text-[var(--foreground-muted)]"
      >
        waiting…
      </span>
    );
  }
  const labels: Record<Status, string> = {
    "waiting-workspace": "waiting…",
    "loading-wasm": "loading wasm…",
    "fetching-trace": "fetching trace…",
    verifying: "verifying…",
    empty: "",
    done: "",
    error: "",
  };
  return (
    <span
      data-testid="verifier-status-badge"
      data-status={status}
      className="text-[11px] px-2 py-0.5 rounded-full bg-[var(--bg-muted)] text-[var(--foreground-muted)]"
    >
      {labels[status]}
    </span>
  );
}
