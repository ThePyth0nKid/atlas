"use client";

/**
 * W20a — Workspace selector dropdown.
 * W20b-2 — "+ New" affordance lit up via `<CreateWorkspaceForm>` in a
 * native `<dialog>` element.
 *
 * Renders in the header next to the navigation. Reflects + drives the
 * `WorkspaceContext` from `lib/workspace-context.tsx`. CI test
 * workspaces (matching `pw-w*-`) are server-side filtered out of the
 * workspaces list before we ever see them.
 *
 * Frozen test-id contract:
 *   - `workspace-selector`              : the wrapping container
 *   - `workspace-selector-current`      : the currently-selected workspace label
 *   - `workspace-selector-empty`        : rendered when no workspaces exist
 *   - `workspace-selector-input`        : the underlying <select>
 *   - W20b-2 added:
 *     - `workspace-selector-new-button` : "+ New" button (replaces the disabled stub)
 *     - `workspace-selector-new-dialog` : <dialog> root when open
 *     - `workspace-selector-new-input`  : workspace_id input inside the dialog
 *     - `workspace-selector-new-submit` : submit button inside the dialog
 *     - `workspace-selector-new-cancel` : cancel button inside the dialog
 *     - `workspace-selector-new-error`  : inline error (only when present)
 *
 * Dialog accessibility: uses the native HTML5 `<dialog>` element via
 * `showModal()`. The browser handles ARIA + Escape-to-close + focus
 * trap; we just plug it into React with a `useRef`. Backdrop styling
 * comes from the global stylesheet (`dialog::backdrop`) — see
 * `apps/atlas-web/src/app/globals.css`.
 */

import { useCallback, useRef, useState } from "react";
import type { ChangeEvent } from "react";
import { useWorkspaceContext } from "@/lib/workspace-context";
import { CreateWorkspaceForm } from "@/components/CreateWorkspaceForm";

export function WorkspaceSelector() {
  const { workspace, setWorkspace, workspaces, loading, error } =
    useWorkspaceContext();
  const dialogRef = useRef<HTMLDialogElement | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);

  /**
   * W20b-2 fix-commit (code-reviewer HIGH): register the `close`
   * listener via a ref-callback so it attaches WHEN the dialog
   * element actually mounts, not on first render of the component.
   *
   * The previous `useEffect` pattern ran once at first mount, when
   * `loading === true` and the loading-branch returned early —
   * `dialogRef.current` was therefore `null`, the effect's guard
   * exited, and the listener never registered. After loading flipped
   * to false the dialog mounted but no listener was ever attached,
   * so Escape closed the native dialog while `dialogOpen` React state
   * stayed `true` and the `data-open` attribute desynced.
   *
   * Ref-callback fires for every mount/unmount with the actual
   * element, regardless of which branch rendered it. The listener
   * lives for the element's lifetime; a re-mount gets a fresh
   * listener. Returned-cleanup from a ref-callback is React-19+ only,
   * so we don't rely on it — but the single-dialog single-component
   * lifecycle here makes leak avoidance straightforward.
   */
  const dialogRefCallback = useCallback(
    (el: HTMLDialogElement | null): void => {
      dialogRef.current = el;
      if (el === null) return;
      const onClose = (): void => setDialogOpen(false);
      el.addEventListener("close", onClose);
    },
    [],
  );

  const openDialog = useCallback((): void => {
    const el = dialogRef.current;
    if (el === null) return;
    if (typeof el.showModal === "function" && !el.open) {
      el.showModal();
      setDialogOpen(true);
    } else if (!el.open) {
      // Fallback for older browsers without HTMLDialogElement support.
      // The form still works; just no modal backdrop / focus trap.
      el.setAttribute("open", "");
      setDialogOpen(true);
    }
  }, []);

  const closeDialog = useCallback((): void => {
    const el = dialogRef.current;
    if (el === null) return;
    if (el.open) el.close();
    setDialogOpen(false);
  }, []);

  if (loading) {
    return (
      <div
        data-testid="workspace-selector"
        data-state="loading"
        className="text-[12px] text-[var(--foreground-muted)]"
      >
        loading workspaces…
      </div>
    );
  }

  if (error !== null) {
    return (
      <div
        data-testid="workspace-selector"
        data-state="error"
        className="text-[12px] text-[var(--accent-danger)]"
        role="alert"
      >
        workspaces unavailable
      </div>
    );
  }

  const newButton = (
    <button
      type="button"
      onClick={openDialog}
      data-testid="workspace-selector-new-button"
      className="border border-[var(--border)] rounded px-2 py-0.5 hover:bg-[var(--bg-subtle)]"
    >
      + New
    </button>
  );

  const dialog = (
    <dialog
      ref={dialogRefCallback}
      data-testid="workspace-selector-new-dialog"
      data-open={dialogOpen ? "true" : "false"}
      aria-labelledby="workspace-selector-new-title"
      className="rounded-lg border border-[var(--border)] bg-[var(--background)] text-[var(--foreground)] p-5 max-w-md w-[90vw]"
    >
      <h3
        id="workspace-selector-new-title"
        className="font-medium text-[14px] mb-1"
      >
        Create workspace
      </h3>
      <p className="text-[12px] text-[var(--foreground-muted)] mb-4">
        Creates a new audit trail with its own per-tenant signing key.
      </p>
      <CreateWorkspaceForm
        testidPrefix="workspace-selector-new"
        defaultValue=""
        submitLabel="Create"
        onSuccess={closeDialog}
        onCancel={closeDialog}
        cancelTestid="workspace-selector-new-cancel"
      />
    </dialog>
  );

  if (workspaces.length === 0) {
    return (
      <div
        data-testid="workspace-selector"
        data-state="empty"
        className="text-[12px] text-[var(--foreground-muted)] flex items-center gap-2"
      >
        <span data-testid="workspace-selector-empty">no workspaces yet</span>
        {newButton}
        {dialog}
      </div>
    );
  }

  const handleChange = (e: ChangeEvent<HTMLSelectElement>): void => {
    setWorkspace(e.target.value);
  };

  return (
    <div
      data-testid="workspace-selector"
      data-state="ready"
      className="flex items-center gap-2 text-[12px]"
    >
      <label
        htmlFor="workspace-selector-input"
        className="text-[var(--foreground-muted)] uppercase tracking-wide text-[10px]"
      >
        Workspace
      </label>
      <select
        id="workspace-selector-input"
        data-testid="workspace-selector-input"
        value={workspace ?? ""}
        onChange={handleChange}
        className="border border-[var(--border)] rounded px-2 py-0.5 bg-[var(--background)] text-[var(--foreground)]"
      >
        {workspaces.map((ws) => (
          <option key={ws} value={ws}>
            {ws}
          </option>
        ))}
      </select>
      <span
        data-testid="workspace-selector-current"
        className="text-[var(--foreground-muted)] hash-chip"
      >
        {workspace ?? "—"}
      </span>
      {newButton}
      {dialog}
    </div>
  );
}
