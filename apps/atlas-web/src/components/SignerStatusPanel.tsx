"use client";

/**
 * W20c — Three rows: signer / embedder / backend status.
 *
 * Same data source as `<LayerStatusPanel>` (the dashboard variant) but
 * a tall layout suitable for /settings: one row per probe, each with
 * a status pill and a one-line explanation of what that status means
 * for the deployer.
 *
 * Frozen testids:
 *   - settings-signer-status        (root)
 *   - settings-status-signer        (signer row pill)
 *   - settings-status-embedder      (embedder row pill)
 *   - settings-status-backend       (backend row pill)
 */

import { useEffect, useState } from "react";
import type {
  BackendStatus,
  EmbedderStatus,
  LayerStatus,
  SignerStatus,
} from "@/lib/system-health";

interface HealthResponse {
  ok: boolean;
  signer?: unknown;
  embedder?: unknown;
  backend?: unknown;
  error?: string;
}

type FetchState =
  | { kind: "loading" }
  | { kind: "ready"; status: LayerStatus }
  | { kind: "error"; message: string };

const SIGNER_VALUES: ReadonlySet<SignerStatus> = new Set([
  "operational",
  "unconfigured",
]);
const EMBEDDER_VALUES: ReadonlySet<EmbedderStatus> = new Set([
  "operational",
  "model_missing",
  "unsupported",
]);
const BACKEND_VALUES: ReadonlySet<BackendStatus> = new Set([
  "operational",
  "stub_501",
  "fault",
]);

const SIGNER_EXPLANATION: Record<SignerStatus, string> = {
  operational:
    "atlas-signer binary is reachable and ATLAS_DEV_MASTER_SEED is set. New events can be signed.",
  unconfigured:
    "Either ATLAS_DEV_MASTER_SEED is unset or the binary is missing. New events cannot be signed; existing events stay verifiable.",
};

const EMBEDDER_EXPLANATION: Record<EmbedderStatus, string> = {
  operational:
    "Embedder is wired and the BAAI/bge-small-en-v1.5 artifact is present.",
  model_missing:
    "Embedder is enabled but the model artifact is not yet wired through the JS bridge (V2-γ work).",
  unsupported:
    "Embedder is disabled (default). Semantic search is V2-γ; no embedding work is performed today.",
};

const BACKEND_EXPLANATION: Record<BackendStatus, string> = {
  operational: "Backend services are wired (V2-γ flag set).",
  stub_501:
    "Semantic-search returns 501. Trace + write paths are operational; semantic-search is V2-γ.",
  fault:
    "Backend is explicitly marked as faulty (operator-set ATLAS_BACKEND_MODE=fault).",
};

export function SignerStatusPanel(): React.ReactElement {
  const [state, setState] = useState<FetchState>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await fetch("/api/atlas/system/health");
        if (!res.ok) {
          throw new Error(`health probe failed (HTTP ${res.status})`);
        }
        const body = (await res.json()) as HealthResponse;
        if (
          !body.ok ||
          typeof body.signer !== "string" ||
          !SIGNER_VALUES.has(body.signer as SignerStatus) ||
          typeof body.embedder !== "string" ||
          !EMBEDDER_VALUES.has(body.embedder as EmbedderStatus) ||
          typeof body.backend !== "string" ||
          !BACKEND_VALUES.has(body.backend as BackendStatus)
        ) {
          throw new Error(
            typeof body.error === "string" && body.error.length > 0
              ? body.error
              : "health response missing expected fields",
          );
        }
        if (cancelled) return;
        setState({
          kind: "ready",
          status: {
            signer: body.signer as SignerStatus,
            embedder: body.embedder as EmbedderStatus,
            backend: body.backend as BackendStatus,
          },
        });
      } catch (e) {
        if (cancelled) return;
        setState({
          kind: "error",
          message: e instanceof Error ? e.message : String(e),
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <section
      className="border border-[var(--border)] rounded-lg p-5"
      data-testid="settings-signer-status"
      aria-labelledby="settings-signer-status-heading"
    >
      <h2
        id="settings-signer-status-heading"
        className="font-medium mb-3"
      >
        Layer 3 — signer / embedder / backend
      </h2>
      {state.kind === "loading" ? (
        <div className="text-[13px] text-[var(--foreground-muted)]">
          Probing layer-3 status…
        </div>
      ) : null}
      {state.kind === "error" ? (
        <div
          className="text-[13px] text-[var(--accent-danger)]"
          role="alert"
        >
          Failed to read layer-3 status: {state.message}
        </div>
      ) : null}
      {state.kind === "ready" ? (
        <ul className="space-y-3">
          <StatusRow
            testid="settings-status-signer"
            label="Signer"
            value={state.status.signer}
            explanation={SIGNER_EXPLANATION[state.status.signer]}
          />
          <StatusRow
            testid="settings-status-embedder"
            label="Embedder"
            value={state.status.embedder}
            explanation={EMBEDDER_EXPLANATION[state.status.embedder]}
          />
          <StatusRow
            testid="settings-status-backend"
            label="Backend"
            value={state.status.backend}
            explanation={BACKEND_EXPLANATION[state.status.backend]}
          />
        </ul>
      ) : null}
    </section>
  );
}

interface StatusRowProps {
  testid: string;
  label: string;
  value: string;
  explanation: string;
}

function StatusRow({
  testid,
  label,
  value,
  explanation,
}: StatusRowProps): React.ReactElement {
  // Tone matches LayerStatusPanel — green for operational, amber for
  // warning-y states, muted for V2-γ stubs.
  const tone =
    value === "operational"
      ? "success"
      : value === "unconfigured" ||
          value === "model_missing" ||
          value === "stub_501"
        ? "warning"
        : "muted";
  const toneStyle: React.CSSProperties =
    tone === "success"
      ? {
          background:
            "color-mix(in srgb, var(--accent-trust) 15%, transparent)",
          color: "var(--accent-trust)",
          borderColor: "var(--accent-trust)",
        }
      : tone === "warning"
        ? {
            background:
              "color-mix(in srgb, var(--accent-warn) 15%, transparent)",
            // W20c — amber-900 for AA on the 15%-mix pill bg
            // (axe-core: amber-700 = 4.08:1, amber-900 = ~7.2:1).
            color: "var(--accent-warn-on-mix)",
            borderColor: "var(--accent-warn)",
          }
        : {
            background: "var(--bg-subtle)",
            color: "var(--foreground-muted)",
            borderColor: "var(--border)",
          };
  return (
    <li
      className="flex items-start gap-3"
      data-testid={testid}
    >
      <div
        className="inline-flex items-center gap-2 border rounded-md px-2.5 py-1 text-[12px] min-w-[170px] shrink-0"
        style={toneStyle}
      >
        <span className="font-medium">{label}</span>
        <span aria-hidden="true">·</span>
        <span>{value}</span>
      </div>
      <p className="text-[12px] text-[var(--foreground-muted)] flex-1">
        {explanation}
      </p>
    </li>
  );
}
