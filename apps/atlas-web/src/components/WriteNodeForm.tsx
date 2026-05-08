"use client";

/**
 * V1.19 Welle 1 — write-node form.
 *
 * Captures one `node.create` event per submission:
 *   - workspace_id: [a-zA-Z0-9_-]{1,128}
 *   - kind: dataset | model | inference | document | other
 *   - id: caller-chosen stable identifier
 *   - attributes: free-form JSON object (validated as JSON, never
 *     interpolated into a shell command — the route handler passes it
 *     through to atlas-signer as a structured payload)
 *
 * The form is intentionally minimal: no autosave, no draft store, no
 * client-side hash computation. The trust property is "the server
 * signs and the verifier confirms" — anything richer here would risk
 * users believing the client computed something verifiable.
 */

import { useState } from "react";

type Status = "idle" | "submitting" | "success" | "error";

type SuccessResult = {
  workspace_id: string;
  event_id: string;
  event_hash: string;
  parents: string[];
  kid: string;
};

const NODE_KINDS = ["dataset", "model", "inference", "document", "other"] as const;
type NodeKind = (typeof NODE_KINDS)[number];

const DEFAULT_WORKSPACE = "ws-mcp-default";
const DEFAULT_ATTRIBUTES = "{}";

export function WriteNodeForm() {
  const [workspaceId, setWorkspaceId] = useState(DEFAULT_WORKSPACE);
  const [kind, setKind] = useState<NodeKind>("dataset");
  const [id, setId] = useState("");
  const [attributes, setAttributes] = useState(DEFAULT_ATTRIBUTES);
  const [status, setStatus] = useState<Status>("idle");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [result, setResult] = useState<SuccessResult | null>(null);

  async function handleSubmit(e: React.FormEvent<HTMLFormElement>): Promise<void> {
    e.preventDefault();
    setErrorMsg(null);
    setResult(null);
    setStatus("submitting");

    let parsedAttributes: Record<string, unknown>;
    try {
      const candidate = attributes.trim() === "" ? {} : JSON.parse(attributes);
      if (
        typeof candidate !== "object" ||
        candidate === null ||
        Array.isArray(candidate)
      ) {
        throw new Error("attributes must be a JSON object, not an array or scalar");
      }
      parsedAttributes = candidate as Record<string, unknown>;
    } catch (err) {
      setStatus("error");
      setErrorMsg(`attributes parse: ${(err as Error).message}`);
      return;
    }

    try {
      const res = await fetch("/api/atlas/write-node", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          workspace_id: workspaceId,
          kind,
          id,
          attributes: parsedAttributes,
        }),
      });
      const json = (await res.json()) as
        | { ok: true } & SuccessResult
        | { ok: false; error: string };
      if (!res.ok || json.ok !== true) {
        const msg = json.ok === false ? json.error : `HTTP ${res.status}`;
        setStatus("error");
        setErrorMsg(msg);
        return;
      }
      setResult({
        workspace_id: json.workspace_id,
        event_id: json.event_id,
        event_hash: json.event_hash,
        parents: json.parents,
        kid: json.kid,
      });
      setStatus("success");
      // Clear node id so a follow-up write doesn't accidentally
      // duplicate the same identifier; keep workspace + attributes
      // for batch-of-similar workflows.
      setId("");
    } catch (err) {
      setStatus("error");
      setErrorMsg((err as Error).message);
    }
  }

  return (
    <section className="border border-[var(--border)] rounded-lg p-5 space-y-5">
      <div>
        <h2 className="font-medium">New node</h2>
        <p className="text-[13px] text-[var(--foreground-muted)] mt-1">
          The server derives the per-tenant kid from{" "}
          <code className="hash-chip">workspace_id</code> and signs the event
          before appending. The Rust signer is the only producer of canonical
          bytes — the browser computes nothing verifiable.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-4">
        <Field label="Workspace ID" htmlFor="ws">
          <input
            id="ws"
            name="workspace_id"
            type="text"
            required
            pattern="^[a-zA-Z0-9_-]{1,128}$"
            value={workspaceId}
            onChange={(e) => setWorkspaceId(e.target.value)}
            className={inputClass}
            autoComplete="off"
          />
          <p className={hintClass}>
            Allowed: <code className="hash-chip">[a-zA-Z0-9_-]{"{1,128}"}</code>.
            Per-tenant kid:{" "}
            <code className="hash-chip">atlas-anchor:{workspaceId || "…"}</code>
          </p>
        </Field>

        <Field label="Node kind" htmlFor="kind">
          <select
            id="kind"
            name="kind"
            required
            value={kind}
            onChange={(e) => setKind(e.target.value as NodeKind)}
            className={inputClass}
          >
            {NODE_KINDS.map((k) => (
              <option key={k} value={k}>
                {k}
              </option>
            ))}
          </select>
        </Field>

        <Field label="Node ID" htmlFor="id">
          <input
            id="id"
            name="id"
            type="text"
            required
            maxLength={256}
            value={id}
            onChange={(e) => setId(e.target.value)}
            placeholder="e.g. dataset/customer_orders_q1_2026"
            className={inputClass}
            autoComplete="off"
          />
        </Field>

        <Field label="Attributes (JSON object)" htmlFor="attrs">
          <textarea
            id="attrs"
            name="attributes"
            rows={6}
            value={attributes}
            onChange={(e) => setAttributes(e.target.value)}
            className={`${inputClass} font-mono text-[12px]`}
            spellCheck={false}
          />
          <p className={hintClass}>
            JSON object. No floats — use basis-points (×10000) for fractions so
            the canonical-CBOR encoding is bit-stable across implementations.
          </p>
        </Field>

        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={status === "submitting"}
            className="text-[13px] font-medium border border-[var(--border)] rounded-md px-4 py-1.5 hover:bg-[var(--bg-subtle)] disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {status === "submitting" ? "Signing…" : "Sign and append"}
          </button>
          {status === "error" && (
            <span className="text-[13px] text-[var(--accent-danger)]">
              {errorMsg}
            </span>
          )}
        </div>
      </form>

      {status === "success" && result && <SuccessCard result={result} />}
    </section>
  );
}

function SuccessCard({ result }: { result: SuccessResult }): React.ReactElement {
  return (
    <div className="border border-[var(--border)] rounded-md p-4 bg-[var(--bg-subtle)] space-y-2 text-[13px]">
      <div className="flex items-center gap-2 font-medium text-[var(--foreground)]">
        <span className="trust-tick trust-tick--ok">✓</span>
        Signed and appended
      </div>
      <KeyValue k="workspace" v={result.workspace_id} />
      <KeyValue k="kid" v={result.kid} />
      <KeyValue k="event_id" v={result.event_id} />
      <KeyValue k="event_hash" v={result.event_hash} mono />
      <KeyValue
        k="parents"
        v={result.parents.length === 0 ? "(genesis)" : result.parents.join(", ")}
        mono
      />
      <p className="text-[12px] text-[var(--foreground-muted)] mt-2">
        Verify by running{" "}
        <code className="hash-chip">atlas-verify-cli</code> against the
        workspace's exported bundle, or open the home-page Live Verifier panel
        once you've exported a fresh trace.
      </p>
    </div>
  );
}

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: React.ReactNode;
}): React.ReactElement {
  return (
    <label htmlFor={htmlFor} className="block space-y-1">
      <span className="block text-[13px] font-medium">{label}</span>
      {children}
    </label>
  );
}

function KeyValue({
  k,
  v,
  mono,
}: {
  k: string;
  v: string;
  mono?: boolean;
}): React.ReactElement {
  return (
    <div className="flex gap-3">
      <span className="text-[var(--foreground-muted)] uppercase tracking-wide text-[11px] w-20 shrink-0 mt-0.5">
        {k}
      </span>
      <span className={mono ? "font-mono text-[12px] break-all" : "break-all"}>
        {v}
      </span>
    </div>
  );
}

const inputClass =
  "w-full border border-[var(--border)] rounded-md px-3 py-1.5 bg-[var(--background)] text-[13px] focus:outline-none focus:border-[var(--foreground-muted)]";

const hintClass = "text-[12px] text-[var(--foreground-muted)] mt-1";
