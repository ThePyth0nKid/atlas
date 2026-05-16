"use client";

/**
 * W20a — Workspace selector dropdown.
 *
 * Renders in the header next to the navigation. Reflects + drives the
 * `WorkspaceContext` from `lib/workspace-context.tsx`. CI test
 * workspaces (matching `pw-w*-`) are server-side filtered out of the
 * workspaces list before we ever see them.
 *
 * Frozen test-id contract:
 *   - `workspace-selector`         : the wrapping container
 *   - `workspace-selector-current` : the currently-selected workspace label
 *   - `workspace-selector-empty`   : rendered when no workspaces exist
 *
 * Plus the underlying `<select>` carries `data-testid="workspace-selector-input"`
 * for programmatic option-listing assertions in the E2E spec.
 *
 * The "+ New workspace" affordance is intentionally NOT implemented in
 * this welle — it's W20b territory (first-run wizard). The disabled
 * button is left in for visual continuity once W20b lights up.
 */

import { useWorkspaceContext } from "@/lib/workspace-context";
import type { ChangeEvent } from "react";

export function WorkspaceSelector() {
  const { workspace, setWorkspace, workspaces, loading, error } =
    useWorkspaceContext();

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

  if (workspaces.length === 0) {
    return (
      <div
        data-testid="workspace-selector"
        data-state="empty"
        className="text-[12px] text-[var(--foreground-muted)] flex items-center gap-2"
      >
        <span data-testid="workspace-selector-empty">no workspaces yet</span>
        <button
          type="button"
          disabled
          title="Coming in W20b first-run wizard"
          className="border border-[var(--border)] rounded px-2 py-0.5 opacity-50 cursor-not-allowed"
        >
          + New workspace
        </button>
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
      <button
        type="button"
        disabled
        title="Coming in W20b first-run wizard"
        className="border border-[var(--border)] rounded px-2 py-0.5 opacity-50 cursor-not-allowed"
      >
        + New
      </button>
    </div>
  );
}
