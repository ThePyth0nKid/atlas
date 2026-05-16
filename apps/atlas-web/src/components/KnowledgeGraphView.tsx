"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type { ComponentType } from "react";
import dynamic from "next/dynamic";
import { useWorkspaceContext } from "@/lib/workspace-context";

// react-force-graph imports d3 ESM and reads window — must be client-only.
// We type loosely because the upstream typings for the dynamic-import path
// are awkward; the props we pass are validated by usage below.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const ForceGraph2D = dynamic<any>(
  () => import("react-force-graph-2d").then((m) => m.default ?? m),
  {
    ssr: false,
    loading: () => (
      <div className="h-[560px] flex items-center justify-center text-[var(--foreground-muted)]">
        loading graph…
      </div>
    ),
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  },
) as unknown as ComponentType<any>;

type GraphNode = {
  id: string;
  label: string;
  kind: "dataset" | "model" | "inference" | "human" | "anchor";
  hash: string;
};
type GraphLink = {
  source: string;
  target: string;
  relation: string;
};

type RawTrace = {
  events: Array<{
    event_id: string;
    event_hash: string;
    parent_hashes: string[];
    payload: Record<string, unknown> & {
      type: string;
      node?: { id?: string; kind?: string; name?: string };
      subject?: string;
    };
  }>;
};

export function KnowledgeGraphView() {
  const { workspace } = useWorkspaceContext();
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<{ w: number; h: number }>({ w: 1000, h: 560 });
  const [trace, setTrace] = useState<RawTrace | null>(null);
  const [selectedHash, setSelectedHash] = useState<string | null>(null);
  // Surface fetch failures (network error, !r.ok envelope, JSON parse
  // failure) so the user sees a real error message instead of a blank
  // canvas. Mirrors `LiveVerifierPanel`'s error pattern; see the
  // `verifier-error` testid contract in that file.
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (workspace === null) {
      // Workspace context still resolving — drop any prior trace and
      // wait for the next workspace event to refetch.
      setTrace(null);
      setError(null);
      return;
    }
    let cancelled = false;
    // Reset prior trace so the previous workspace's nodes don't
    // briefly stay on screen while the new fetch lands.
    setTrace(null);
    setSelectedHash(null);
    setError(null);
    fetch(`/api/atlas/trace?workspace=${encodeURIComponent(workspace)}`)
      .then(async (r) => {
        if (!r.ok) {
          // Try to surface the structured error envelope; fall back to
          // the HTTP status when the body isn't parseable JSON.
          let detail = `HTTP ${r.status}`;
          try {
            const body = (await r.json()) as { error?: string };
            if (typeof body.error === "string") detail = body.error;
          } catch {
            // body wasn't JSON — keep the HTTP-status fallback
          }
          throw new Error(`could not load trace: ${detail}`);
        }
        return (await r.json()) as RawTrace;
      })
      .then((j: RawTrace) => {
        if (!cancelled) setTrace(j);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [workspace]);

  useEffect(() => {
    if (!containerRef.current) return;
    const el = containerRef.current;
    const ro = new ResizeObserver(() => {
      setSize({ w: el.clientWidth, h: 560 });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const { nodes, links, selectedEvent } = useMemo(() => {
    if (!trace) return { nodes: [] as GraphNode[], links: [] as GraphLink[], selectedEvent: null };

    const nodes: GraphNode[] = trace.events.map((ev) => {
      const payload = ev.payload;
      let kind: GraphNode["kind"] = "anchor";
      let label = ev.event_id;

      if (payload.type === "node.create" && payload.node) {
        kind = (payload.node.kind as GraphNode["kind"]) ?? "dataset";
        label = payload.node.id ?? payload.node.name ?? ev.event_id;
      } else if (payload.type === "annotation.add") {
        kind = "human";
        label = `verify: ${payload.subject ?? ev.event_id}`;
      } else if (payload.type === "anchor.created") {
        kind = "anchor";
        label = "Sigstore anchor";
      }

      return {
        id: ev.event_hash,
        label,
        kind,
        hash: ev.event_hash,
      };
    });

    const links: GraphLink[] = trace.events.flatMap((ev) =>
      ev.parent_hashes.map((p) => ({
        source: p,
        target: ev.event_hash,
        relation: "parent",
      })),
    );

    const selectedEvent =
      selectedHash != null ? trace.events.find((e) => e.event_hash === selectedHash) ?? null : null;

    return { nodes, links, selectedEvent };
  }, [trace, selectedHash]);

  // Fetch-error contract: render a visible, accessible error block so
  // the user can distinguish "trace endpoint failed" from "workspace
  // is legitimately empty". Mirrors LiveVerifierPanel's
  // `verifier-error` testid + role="alert" pattern.
  if (error !== null) {
    return (
      <div
        data-testid="graph-error"
        role="alert"
        className="border border-[var(--accent-danger)] rounded-lg p-4 text-[13px] text-[var(--accent-danger)] bg-[var(--bg-subtle)]"
      >
        {error}
      </div>
    );
  }

  // W20a empty-state contract: when the trace has resolved with zero
  // events, render a call-to-action instead of an empty force-graph
  // canvas. The test seam (`graph-empty-state`) is pinned by the
  // workspace-selector Playwright spec.
  if (trace !== null && trace.events.length === 0) {
    return (
      <div
        data-testid="graph-empty-state"
        className="border border-dashed border-[var(--border)] rounded-lg p-8 text-center bg-[var(--bg-subtle)]"
      >
        <h3 className="font-medium mb-2">Your knowledge graph is empty</h3>
        <p className="text-[13px] text-[var(--foreground-muted)]">
          <a
            href="/write"
            className="underline hover:text-[var(--foreground)]"
          >
            Write a fact at /write →
          </a>{" "}
          to materialise your first node.
        </p>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 lg:grid-cols-[1fr_360px] gap-4">
      <div
        ref={containerRef}
        className="border border-[var(--border)] rounded-lg overflow-hidden bg-[var(--background)]"
        style={{ height: 560 }}
      >
        {nodes.length > 0 && (
          <ForceGraph2D
            width={size.w}
            height={size.h}
            graphData={{ nodes, links }}
            nodeLabel={(n: GraphNode) => `${n.label} (${n.kind})`}
            nodeRelSize={6}
            nodeAutoColorBy={(n: GraphNode) => n.kind}
            linkDirectionalArrowLength={4}
            linkDirectionalArrowRelPos={1}
            linkColor={() => "#94a3b8"}
            backgroundColor="#ffffff"
            cooldownTicks={120}
            onNodeClick={(node: GraphNode) => setSelectedHash(node.hash)}
            nodeCanvasObject={(
              node: GraphNode & { x?: number; y?: number },
              ctx: CanvasRenderingContext2D,
              scale: number,
            ) => {
              const n = node;
              if (n.x == null || n.y == null) return;
              const radius = 6;
              ctx.beginPath();
              ctx.arc(n.x, n.y, radius, 0, 2 * Math.PI);
              ctx.fillStyle = colorForKind(n.kind);
              ctx.fill();
              ctx.lineWidth = n.hash === selectedHash ? 2 : 1;
              ctx.strokeStyle = n.hash === selectedHash ? "#0f172a" : "#cbd5e1";
              ctx.stroke();
              if (scale > 1.2) {
                ctx.fillStyle = "#0f172a";
                ctx.font = "10px ui-sans-serif";
                ctx.textAlign = "center";
                ctx.fillText(n.label, n.x, n.y + 16);
              }
            }}
          />
        )}
      </div>

      <aside className="border border-[var(--border)] rounded-lg p-4 bg-[var(--bg-subtle)] h-[560px] overflow-y-auto">
        <h3 className="font-medium mb-3">Provenance Tree</h3>
        {selectedEvent ? (
          <div className="space-y-3 text-[13px]">
            <div>
              <div className="text-[var(--foreground-muted)] text-[11px] uppercase tracking-wide">
                event hash
              </div>
              <div className="hash-chip mt-1 break-all">
                {selectedEvent.event_hash.slice(0, 24)}…
              </div>
            </div>
            <div>
              <div className="text-[var(--foreground-muted)] text-[11px] uppercase tracking-wide">
                event id
              </div>
              <div className="font-mono text-[12px] mt-1">{selectedEvent.event_id}</div>
            </div>
            <div>
              <div className="text-[var(--foreground-muted)] text-[11px] uppercase tracking-wide">
                parent hashes
              </div>
              <div className="mt-1 space-y-1">
                {selectedEvent.parent_hashes.length === 0 ? (
                  <span className="text-[var(--foreground-muted)] italic">genesis (no parents)</span>
                ) : (
                  selectedEvent.parent_hashes.map((p) => (
                    <button
                      key={p}
                      onClick={() => setSelectedHash(p)}
                      className="block hash-chip break-all w-full text-left hover:bg-[var(--bg-muted)]"
                    >
                      {p.slice(0, 24)}…
                    </button>
                  ))
                )}
              </div>
            </div>
            <div>
              <div className="text-[var(--foreground-muted)] text-[11px] uppercase tracking-wide">
                payload
              </div>
              <pre className="mt-1 text-[11px] bg-[var(--background)] border border-[var(--border)] rounded p-2 overflow-x-auto">
                {JSON.stringify(selectedEvent.payload, null, 2)}
              </pre>
            </div>
            <p className="text-[11px] text-[var(--foreground-muted)] italic">
              This view does not run the verifier. To prove signatures, hashes, and
              parent links match, run the Live Verifier Panel on the dashboard.
            </p>
          </div>
        ) : (
          <p className="text-[var(--foreground-muted)] text-[13px]">
            Click a node to see its full provenance trail — sig, parent hashes, and payload.
          </p>
        )}
      </aside>
    </div>
  );
}

function colorForKind(kind: GraphNode["kind"]): string {
  switch (kind) {
    case "dataset":
      return "#3b82f6";
    case "model":
      return "#a855f7";
    case "inference":
      return "#0ea5e9";
    case "human":
      return "#3fbc78";
    case "anchor":
      return "#f59e0b";
  }
}
