"use client";

/**
 * W20c — Read-only display of the 11 supply-chain pins.
 *
 * Reads from `/api/atlas/system/supply-chain-pins`. The constants are
 * compile-in (mirrored from `crates/atlas-mem0g/src/embedder.rs`), so
 * this is informational only — there is no rotation affordance in
 * the V2-β-1 UI. Rotations happen via an explicit Atlas release.
 *
 * Frozen testids:
 *   - settings-supply-chain-pins      (root container)
 *   - settings-supply-chain-pin-row   (per pin row)
 *   - settings-supply-chain-pin-label (per pin label)
 *   - settings-supply-chain-pin-value (per pin value)
 */

import { useEffect, useState } from "react";

interface PinsResponse {
  ok: boolean;
  hf_revision_sha?: unknown;
  onnx_sha256?: unknown;
  tokenizer_json_sha256?: unknown;
  config_json_sha256?: unknown;
  special_tokens_map_sha256?: unknown;
  tokenizer_config_json_sha256?: unknown;
  model_url?: unknown;
  tokenizer_json_url?: unknown;
  config_json_url?: unknown;
  special_tokens_map_url?: unknown;
  tokenizer_config_json_url?: unknown;
  error?: string;
}

interface PinRow {
  label: string;
  value: string;
}

type FetchState =
  | { kind: "loading" }
  | { kind: "ready"; rows: ReadonlyArray<PinRow> }
  | { kind: "error"; message: string };

function isStringField(v: unknown): v is string {
  return typeof v === "string" && v.length > 0;
}

export function SupplyChainPinsPanel(): React.ReactElement {
  const [state, setState] = useState<FetchState>({ kind: "loading" });

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await fetch("/api/atlas/system/supply-chain-pins");
        if (!res.ok) {
          throw new Error(`supply-chain-pins fetch failed (HTTP ${res.status})`);
        }
        const body = (await res.json()) as PinsResponse;
        if (
          !body.ok ||
          !isStringField(body.hf_revision_sha) ||
          !isStringField(body.onnx_sha256) ||
          !isStringField(body.tokenizer_json_sha256) ||
          !isStringField(body.config_json_sha256) ||
          !isStringField(body.special_tokens_map_sha256) ||
          !isStringField(body.tokenizer_config_json_sha256) ||
          !isStringField(body.model_url) ||
          !isStringField(body.tokenizer_json_url) ||
          !isStringField(body.config_json_url) ||
          !isStringField(body.special_tokens_map_url) ||
          !isStringField(body.tokenizer_config_json_url)
        ) {
          throw new Error("supply-chain-pins response was malformed");
        }
        const rows: ReadonlyArray<PinRow> = [
          { label: "HF revision SHA", value: body.hf_revision_sha },
          { label: "model.onnx SHA-256", value: body.onnx_sha256 },
          { label: "tokenizer.json SHA-256", value: body.tokenizer_json_sha256 },
          { label: "config.json SHA-256", value: body.config_json_sha256 },
          {
            label: "special_tokens_map.json SHA-256",
            value: body.special_tokens_map_sha256,
          },
          {
            label: "tokenizer_config.json SHA-256",
            value: body.tokenizer_config_json_sha256,
          },
          { label: "model.onnx URL", value: body.model_url },
          { label: "tokenizer.json URL", value: body.tokenizer_json_url },
          { label: "config.json URL", value: body.config_json_url },
          {
            label: "special_tokens_map.json URL",
            value: body.special_tokens_map_url,
          },
          {
            label: "tokenizer_config.json URL",
            value: body.tokenizer_config_json_url,
          },
        ];
        if (cancelled) return;
        setState({ kind: "ready", rows });
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
      data-testid="settings-supply-chain-pins"
      aria-labelledby="settings-supply-chain-heading"
    >
      <h2
        id="settings-supply-chain-heading"
        className="font-medium mb-1"
      >
        Supply-chain pins
      </h2>
      <p className="text-[12px] text-[var(--foreground-muted)] mb-3">
        Compile-in constants from <code className="font-mono">crates/atlas-mem0g/src/embedder.rs</code>.
        Rotations land in an explicit Atlas release; this panel is read-only.
      </p>
      {state.kind === "loading" ? (
        <div className="text-[13px] text-[var(--foreground-muted)]">
          Loading supply-chain pins…
        </div>
      ) : null}
      {state.kind === "error" ? (
        <div
          className="text-[13px] text-[var(--accent-danger)]"
          role="alert"
        >
          Failed to load supply-chain pins: {state.message}
        </div>
      ) : null}
      {state.kind === "ready" ? (
        <ul className="space-y-1">
          {state.rows.map((row) => (
            <li
              key={row.label}
              className="flex items-baseline gap-3 text-[12px]"
              data-testid="settings-supply-chain-pin-row"
            >
              <span
                className="text-[var(--foreground-muted)] w-64 shrink-0"
                data-testid="settings-supply-chain-pin-label"
              >
                {row.label}
              </span>
              <code
                className="font-mono break-all"
                data-testid="settings-supply-chain-pin-value"
              >
                {row.value}
              </code>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
