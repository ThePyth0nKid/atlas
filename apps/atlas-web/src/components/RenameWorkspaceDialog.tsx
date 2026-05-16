"use client";

/**
 * W20c — Rename-workspace dialog (native `<dialog>`).
 *
 * Mounted by `<WorkspaceListPanel>` when a per-row "Rename" button is
 * clicked. Closes itself via `onClose` callback after successful
 * rename or cancel.
 *
 * Why a controlled `useEffect` open + ref-callback pattern?
 *   See `<WorkspaceSelector>` dialog comment — the `useEffect` would
 *   miss the mount race in strict-mode + Next.js dev. The
 *   ref-callback registers the `close` listener at mount time, and
 *   the effect calls `showModal()` once the ref is attached.
 *
 * Frozen testids:
 *   - settings-rename-dialog            (root)
 *   - settings-rename-input             (new-id input)
 *   - settings-rename-submit            (submit button)
 *   - settings-rename-cancel            (cancel button)
 *   - settings-rename-error             (inline error)
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { useWorkspaceContext, WORKSPACE_ID_RE } from "@/lib/workspace-context";

export interface RenameWorkspaceDialogProps {
  /** Workspace id being renamed (current value). */
  oldId: string;
  /** Close callback — invoked on cancel, success, or backdrop dismiss. */
  onClose: () => void;
}

export function RenameWorkspaceDialog({
  oldId,
  onClose,
}: RenameWorkspaceDialogProps): React.ReactElement {
  const { renameWorkspace } = useWorkspaceContext();
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const [newId, setNewId] = useState<string>(oldId);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState<boolean>(false);

  // Ref-callback registers the close listener at mount time.
  const dialogRefCallback = useCallback(
    (el: HTMLDialogElement | null): void => {
      dialogRef.current = el;
      if (el === null) return;
      const onCloseEvent = (): void => onClose();
      el.addEventListener("close", onCloseEvent);
      // Cleanup is the el-being-null branch on the next callback fire
      // (React 19 ref-callback cleanup); the listener auto-clears when
      // the element unmounts because the element itself goes away.
    },
    [onClose],
  );

  // Open the dialog once on mount.
  useEffect(() => {
    const el = dialogRef.current;
    if (el === null) return;
    if (typeof el.showModal === "function" && !el.open) {
      el.showModal();
    } else if (!el.open) {
      el.setAttribute("open", "");
    }
  }, []);

  const handleCancel = useCallback((): void => {
    const el = dialogRef.current;
    if (el !== null && el.open) el.close();
    else onClose();
  }, [onClose]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent<HTMLFormElement>): Promise<void> => {
      e.preventDefault();
      if (submitting) return;
      setError(null);
      const trimmed = newId.trim();
      if (!WORKSPACE_ID_RE.test(trimmed)) {
        setError("Workspace id must match [a-zA-Z0-9_-]{1,128}");
        return;
      }
      if (trimmed === oldId) {
        setError("New id must differ from the current id");
        return;
      }
      setSubmitting(true);
      const result = await renameWorkspace(oldId, trimmed);
      setSubmitting(false);
      if (!result.ok) {
        setError(result.error);
        return;
      }
      // Success — close the dialog.
      const el = dialogRef.current;
      if (el !== null && el.open) el.close();
      else onClose();
    },
    [newId, oldId, onClose, renameWorkspace, submitting],
  );

  return (
    <dialog
      ref={dialogRefCallback}
      data-testid="settings-rename-dialog"
      aria-labelledby="settings-rename-title"
      className="border border-[var(--border)] rounded-lg p-5 min-w-[360px] bg-[var(--background)]"
    >
      <form onSubmit={handleSubmit} className="space-y-4">
        <h3
          id="settings-rename-title"
          className="font-medium"
        >
          Rename workspace
        </h3>
        <p className="text-[12px] text-[var(--foreground-muted)]">
          Renames the workspace directory on disk and re-derives the
          per-tenant pubkey. Existing events keep their signatures (the
          signing input includes the workspace id at signing time, so
          historical events remain verifiable against the renamed kid).
        </p>
        <div>
          <label
            htmlFor="settings-rename-input"
            className="block text-[12px] font-medium mb-1"
          >
            New workspace id
          </label>
          <input
            id="settings-rename-input"
            data-testid="settings-rename-input"
            type="text"
            value={newId}
            onChange={(e) => setNewId(e.target.value)}
            disabled={submitting}
            autoFocus
            className="w-full border border-[var(--border)] rounded-md px-3 py-1.5 text-[13px] font-mono"
          />
        </div>
        {error !== null ? (
          <p
            className="text-[12px] text-[var(--accent-danger)]"
            role="alert"
            data-testid="settings-rename-error"
          >
            {error}
          </p>
        ) : null}
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={handleCancel}
            disabled={submitting}
            className="text-[13px] border border-[var(--border)] rounded-md px-3 py-1.5 hover:bg-[var(--bg-subtle)]"
            data-testid="settings-rename-cancel"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={submitting}
            className="text-[13px] font-medium border border-[var(--border)] rounded-md px-3 py-1.5 hover:bg-[var(--bg-subtle)] disabled:opacity-50"
            data-testid="settings-rename-submit"
          >
            {submitting ? "Renaming…" : "Rename"}
          </button>
        </div>
      </form>
    </dialog>
  );
}
