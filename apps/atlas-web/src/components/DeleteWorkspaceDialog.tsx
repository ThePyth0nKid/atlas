"use client";

/**
 * W20c — Delete-workspace dialog with typed-confirmation gate (DA-6).
 *
 * The submit button is disabled until the user types the exact
 * workspace id into the confirmation input. Deletion is permanent —
 * `fs.rm({ recursive: true })` of the workspace directory removes the
 * events log and the anchor chain. The user is also warned about
 * verifier consequences (offline auditors holding signed exports can
 * still verify those exports, but new events can't be appended).
 *
 * Frozen testids:
 *   - settings-delete-dialog        (root)
 *   - settings-delete-input         (typed-confirm input)
 *   - settings-delete-submit        (submit button; disabled until
 *                                    confirm-text matches id)
 *   - settings-delete-cancel        (cancel button)
 *   - settings-delete-error         (inline error)
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { useWorkspaceContext } from "@/lib/workspace-context";

export interface DeleteWorkspaceDialogProps {
  /** Workspace id being deleted. */
  id: string;
  /** Close callback — invoked on cancel, success, or backdrop dismiss. */
  onClose: () => void;
}

export function DeleteWorkspaceDialog({
  id,
  onClose,
}: DeleteWorkspaceDialogProps): React.ReactElement {
  const { deleteWorkspace } = useWorkspaceContext();
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const [confirm, setConfirm] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState<boolean>(false);

  const dialogRefCallback = useCallback(
    (el: HTMLDialogElement | null): void => {
      dialogRef.current = el;
      if (el === null) return;
      const onCloseEvent = (): void => onClose();
      el.addEventListener("close", onCloseEvent);
    },
    [onClose],
  );

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

  const canSubmit = confirm === id && !submitting;

  const handleSubmit = useCallback(
    async (e: React.FormEvent<HTMLFormElement>): Promise<void> => {
      e.preventDefault();
      if (!canSubmit) return;
      setError(null);
      setSubmitting(true);
      const result = await deleteWorkspace(id);
      setSubmitting(false);
      if (!result.ok) {
        setError(result.error);
        return;
      }
      const el = dialogRef.current;
      if (el !== null && el.open) el.close();
      else onClose();
    },
    [canSubmit, deleteWorkspace, id, onClose],
  );

  return (
    <dialog
      ref={dialogRefCallback}
      data-testid="settings-delete-dialog"
      aria-labelledby="settings-delete-title"
      className="border border-[var(--border)] rounded-lg p-5 min-w-[360px] bg-[var(--background)]"
    >
      <form onSubmit={handleSubmit} className="space-y-4">
        <h3
          id="settings-delete-title"
          className="font-medium"
          style={{ color: "var(--accent-danger)" }}
        >
          Delete workspace
        </h3>
        <p className="text-[12px] text-[var(--foreground-muted)]">
          This removes the workspace directory, its events log, and its
          anchor chain. Existing signed exports remain verifiable — but
          new events cannot be appended. This action cannot be undone.
        </p>
        <p className="text-[12px]">
          Type{" "}
          <code className="font-mono bg-[var(--bg-muted)] px-1 rounded">
            {id}
          </code>{" "}
          below to confirm:
        </p>
        <div>
          <label
            htmlFor="settings-delete-input"
            className="sr-only"
          >
            Type the workspace id to confirm deletion
          </label>
          <input
            id="settings-delete-input"
            data-testid="settings-delete-input"
            type="text"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            disabled={submitting}
            autoFocus
            className="w-full border border-[var(--border)] rounded-md px-3 py-1.5 text-[13px] font-mono"
          />
        </div>
        {error !== null ? (
          <p
            className="text-[12px] text-[var(--accent-danger)]"
            role="alert"
            data-testid="settings-delete-error"
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
            data-testid="settings-delete-cancel"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={!canSubmit}
            aria-disabled={!canSubmit}
            className="text-[13px] font-medium border rounded-md px-3 py-1.5 disabled:opacity-50 disabled:cursor-not-allowed"
            style={{
              borderColor: "var(--accent-danger)",
              color: "var(--accent-danger)",
            }}
            data-testid="settings-delete-submit"
          >
            {submitting ? "Deleting…" : "Delete"}
          </button>
        </div>
      </form>
    </dialog>
  );
}
