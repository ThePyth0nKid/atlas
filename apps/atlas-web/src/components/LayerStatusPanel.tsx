"use client";

/**
 * W20c — LayerStatusPanel.
 *
 * Surfaces the honest layer-3 status on the dashboard above the
 * LiveVerifierPanel. Reads from `/api/atlas/system/health`. Three
 * compact pills: signer, embedder, backend.
 *
 * "Honest" means: when the signer is unconfigured we say so —
 * previous releases ("V1.19 Welle 1") would render dashboards that
 * claimed "all systems operational" while POST /api/atlas/write-node
 * would 500. The pills are colour-coded:
 *
 *   * operational → success
 *   * model_missing / stub_501 / unconfigured → warning
 *   * fault / unsupported → muted (informational; not a fault yet)
 *
 * Frozen testids (Lesson #19 — new contract, never re-use existing):
 *   - layer-status-panel        (root container)
 *   - layer-status-signer       (signer pill)
 *   - layer-status-embedder     (embedder pill)
 *   - layer-status-backend      (backend pill)
 *   - layer-status-loading      (during fetch)
 *   - layer-status-error        (on fetch failure)
 *
 * Threat model: the response shape is parsed via shape-narrowing
 * before render. A misbehaving proxy that returns HTML cannot inject
 * scripts; we never `dangerouslySetInnerHTML` and the JSON envelope
 * is validated via duck-typing before any field is rendered.
 */

import { useEffect, useState } from "react";
import type {
  BackendStatus,
  EmbedderStatus,
  LayerStatus,
  SignerStatus,
} from "@/lib/system-health";

type FetchState =
  | { kind: "loading" }
  | { kind: "ready"; status: LayerStatus }
  | { kind: "error"; message: string };

interface LayerStatusResponse {
  ok: boolean;
  embedder?: unknown;
  backend?: unknown;
  signer?: unknown;
  error?: string;
}

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
const SIGNER_VALUES: ReadonlySet<SignerStatus> = new Set([
  "operational",
  "unconfigured",
]);

function isLayerStatus(body: LayerStatusResponse): body is LayerStatusResponse & {
  embedder: EmbedderStatus;
  backend: BackendStatus;
  signer: SignerStatus;
} {
  return (
    typeof body.embedder === "string" &&
    EMBEDDER_VALUES.has(body.embedder as EmbedderStatus) &&
    typeof body.backend === "string" &&
    BACKEND_VALUES.has(body.backend as BackendStatus) &&
    typeof body.signer === "string" &&
    SIGNER_VALUES.has(body.signer as SignerStatus)
  );
}

export function LayerStatusPanel(): React.ReactElement {
  const [state, setState] = useState<FetchState>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await fetch("/api/atlas/system/health");
        if (!res.ok) {
          throw new Error(`health probe failed (HTTP ${res.status})`);
        }
        let body: LayerStatusResponse;
        try {
          body = (await res.json()) as LayerStatusResponse;
        } catch {
          throw new Error("health response was not valid JSON");
        }
        if (!body.ok || !isLayerStatus(body)) {
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
            embedder: body.embedder,
            backend: body.backend,
            signer: body.signer,
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

  if (state.kind === "loading") {
    return (
      <section
        className="border border-[var(--border)] rounded-lg p-4 text-[13px] text-[var(--foreground-muted)]"
        data-testid="layer-status-loading"
      >
        Probing layer-3 status…
      </section>
    );
  }

  if (state.kind === "error") {
    return (
      <section
        className="border border-[var(--border)] rounded-lg p-4 text-[13px] text-[var(--accent-danger)]"
        role="alert"
        data-testid="layer-status-error"
      >
        Failed to read layer-3 status: {state.message}
      </section>
    );
  }

  const { status } = state;
  return (
    <section
      className="border border-[var(--border)] rounded-lg p-4"
      data-testid="layer-status-panel"
      aria-labelledby="layer-status-heading"
    >
      <h2
        id="layer-status-heading"
        className="text-[13px] font-medium mb-3"
      >
        Layer 3 — signer / embedder / backend
      </h2>
      <div className="flex flex-wrap gap-2">
        <StatusPill
          testid="layer-status-signer"
          label="Signer"
          value={status.signer}
          tone={pillToneFor(status.signer)}
        />
        <StatusPill
          testid="layer-status-embedder"
          label="Embedder"
          value={status.embedder}
          tone={pillToneFor(status.embedder)}
        />
        <StatusPill
          testid="layer-status-backend"
          label="Backend"
          value={status.backend}
          tone={pillToneFor(status.backend)}
        />
      </div>
    </section>
  );
}

type PillTone = "success" | "warning" | "muted";

function pillToneFor(
  value: SignerStatus | EmbedderStatus | BackendStatus,
): PillTone {
  if (value === "operational") return "success";
  if (
    value === "unconfigured" ||
    value === "model_missing" ||
    value === "stub_501"
  ) {
    return "warning";
  }
  return "muted";
}

interface StatusPillProps {
  testid: string;
  label: string;
  value: string;
  tone: PillTone;
}

function StatusPill({
  testid,
  label,
  value,
  tone,
}: StatusPillProps): React.ReactElement {
  // Colour tokens come from `globals.css`:
  //   - `--accent-trust` (green-800)  — AA on bg-muted at ≥6.5:1
  //   - `--accent-warn`  (amber-700) — AA on bg-muted at ≥4.5:1
  //   - `--foreground-muted` (slate-600) — AA on bg-muted at ≥7.2:1
  // Each tone uses the 15%-mix-on-white pattern already proven in
  // `.trust-tick--ok` / `.trust-tick--fail` (globals.css lines 92-99)
  // so a Welle 11 cross-browser gamma audit does not need re-running.
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
            // W20c — amber-900 instead of amber-700 to hit WCAG AA on
            // the 15%-mix-on-white pill background (axe-core flagged
            // amber-700 at 4.08:1 in this commit).
            color: "var(--accent-warn-on-mix)",
            borderColor: "var(--accent-warn)",
          }
        : {
            background: "var(--bg-subtle)",
            color: "var(--foreground-muted)",
            borderColor: "var(--border)",
          };
  return (
    <div
      className="inline-flex items-center gap-2 border rounded-md px-2.5 py-1 text-[12px]"
      style={toneStyle}
      data-testid={testid}
    >
      <span className="font-medium">{label}</span>
      <span aria-hidden="true">·</span>
      <span>{value}</span>
    </div>
  );
}
