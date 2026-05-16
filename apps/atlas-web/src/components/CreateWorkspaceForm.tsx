"use client";

/**
 * W20b-2 — shared create-workspace form.
 *
 * Extracted from `FirstRunWizard` so the workspace-selector dialog
 * can reuse the same field layout, regex pattern, and submit
 * semantics. Two consumers:
 *   - `<FirstRunWizard>`         (full-page empty-state at `/`)
 *   - `<WorkspaceSelector>` dialog ("+ New" affordance in the header)
 *
 * The component is intentionally controlled-state-free (the input is
 * uncontrolled — value lives in DOM until submit). That keeps the
 * form trivially re-renderable and avoids the
 * controlled/uncontrolled flip when the parent resets `defaultValue`
 * across mount/unmount.
 *
 * Frozen testid contract — callers pass a `testidPrefix`. The form
 * exposes:
 *   `<prefix>-input`, `<prefix>-submit`, `<prefix>-error`
 * The wizard uses `first-run-wizard`; the selector dialog uses
 * `workspace-selector-new`. Tests pin both.
 */

import { useState, type FormEvent } from "react";
import { useWorkspaceContext } from "@/lib/workspace-context";

interface CreateWorkspaceFormProps {
  testidPrefix: string;
  defaultValue?: string;
  submitLabel?: string;
  /**
   * Called after a 200 response has been processed and the workspace
   * has been added to the context. Lets the wizard redirect to
   * `/write` and the selector dialog close itself.
   */
  onSuccess?: (id: string) => void;
  /**
   * Optional cancel handler — rendered as a secondary button when
   * provided. The wizard does NOT pass this (no escape hatch on the
   * full-page empty state); the selector dialog DOES.
   */
  onCancel?: () => void;
  cancelTestid?: string;
}

export function CreateWorkspaceForm({
  testidPrefix,
  defaultValue = "ws-personal",
  submitLabel = "Create workspace",
  onSuccess,
  onCancel,
  cancelTestid,
}: CreateWorkspaceFormProps) {
  const { createWorkspace } = useWorkspaceContext();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: FormEvent<HTMLFormElement>): Promise<void> {
    e.preventDefault();
    setError(null);
    const form = e.currentTarget;
    const formData = new FormData(form);
    const idRaw = formData.get("workspace_id");
    const id = typeof idRaw === "string" ? idRaw.trim() : "";
    if (id.length === 0) {
      setError("workspace id is required");
      return;
    }
    setSubmitting(true);
    try {
      const result = await createWorkspace(id);
      if (!result.ok) {
        setError(result.error);
        return;
      }
      onSuccess?.(id);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      <label className="block text-[12px] text-[var(--foreground-muted)] uppercase tracking-wide">
        Workspace id
        <input
          name="workspace_id"
          type="text"
          required
          pattern="^[a-zA-Z0-9_-]{1,128}$"
          maxLength={128}
          defaultValue={defaultValue}
          disabled={submitting}
          autoFocus
          data-testid={`${testidPrefix}-input`}
          className="mt-1 block w-full border border-[var(--border)] rounded px-2 py-1 bg-[var(--background)] text-[var(--foreground)] text-[13px] normal-case tracking-normal disabled:opacity-60"
          aria-describedby={error !== null ? `${testidPrefix}-error` : undefined}
        />
      </label>
      <p className="text-[11px] text-[var(--foreground-muted)]">
        Letters, digits, <code>_</code> and <code>-</code>, up to 128 chars.
      </p>
      {error !== null ? (
        <p
          role="alert"
          id={`${testidPrefix}-error`}
          data-testid={`${testidPrefix}-error`}
          className="text-[12px] text-[var(--accent-danger)]"
        >
          {error}
        </p>
      ) : null}
      <div className="flex items-center gap-2">
        <button
          type="submit"
          disabled={submitting}
          data-testid={`${testidPrefix}-submit`}
          className="border border-[var(--border)] rounded px-3 py-1.5 text-[13px] font-medium hover:bg-[var(--bg-subtle)] disabled:opacity-60 disabled:cursor-not-allowed"
        >
          {submitting ? "Creating…" : submitLabel}
        </button>
        {onCancel !== undefined ? (
          <button
            type="button"
            onClick={onCancel}
            disabled={submitting}
            data-testid={cancelTestid}
            className="border border-[var(--border)] rounded px-3 py-1.5 text-[13px] hover:bg-[var(--bg-subtle)] disabled:opacity-60"
          >
            Cancel
          </button>
        ) : null}
      </div>
    </form>
  );
}
