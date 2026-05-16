"use client";

/**
 * W20c — Workspace list with rename + delete affordances.
 *
 * Reads `workspaces` + `workspace` from `WorkspaceContext`. Per-row:
 *   - Active marker on the currently-selected workspace
 *   - "Rename" button opens <RenameWorkspaceDialog>
 *   - "Delete" button opens <DeleteWorkspaceDialog>
 *
 * Frozen testids:
 *   - settings-workspace-list           (root)
 *   - settings-workspace-row            (each row; carries
 *                                        `data-workspace-id` attribute)
 *   - settings-workspace-id             (id text, per row)
 *   - settings-workspace-active-marker  (only on the active row)
 *   - settings-rename-button            (per row)
 *   - settings-delete-button            (per row; disabled when
 *                                        workspaces.length === 1)
 *   - settings-delete-disabled-hint     (helper text when delete is
 *                                        disabled because it would be
 *                                        the last workspace)
 *   - settings-workspace-empty          (empty state)
 *
 * The delete button is client-disabled on `workspaces.length === 1`
 * (DA-6 defence-in-depth) — the server enforces the 409 too.
 */

import { useState } from "react";
import { useWorkspaceContext } from "@/lib/workspace-context";
import { RenameWorkspaceDialog } from "@/components/RenameWorkspaceDialog";
import { DeleteWorkspaceDialog } from "@/components/DeleteWorkspaceDialog";

export function WorkspaceListPanel(): React.ReactElement {
  const { workspaces, workspace, loading, error } = useWorkspaceContext();
  const [renameTarget, setRenameTarget] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  return (
    <section
      className="border border-[var(--border)] rounded-lg p-5"
      aria-labelledby="settings-workspaces-heading"
    >
      <h2
        id="settings-workspaces-heading"
        className="font-medium mb-3"
      >
        Workspaces
      </h2>
      {error !== null ? (
        <div
          className="text-[13px] text-[var(--accent-danger)] mb-3"
          role="alert"
          data-testid="settings-workspace-error"
        >
          {error}
        </div>
      ) : null}
      {loading ? (
        <div
          className="text-[13px] text-[var(--foreground-muted)]"
          data-testid="settings-workspace-loading"
        >
          Loading workspaces…
        </div>
      ) : workspaces.length === 0 ? (
        <div
          className="text-[13px] text-[var(--foreground-muted)]"
          data-testid="settings-workspace-empty"
        >
          No workspaces yet. Create one from the home page.
        </div>
      ) : (
        <ul
          className="space-y-2"
          data-testid="settings-workspace-list"
        >
          {workspaces.map((id) => {
            const isActive = id === workspace;
            const canDelete = workspaces.length > 1;
            return (
              <li
                key={id}
                className="flex items-center gap-3 border border-[var(--border)] rounded-md px-3 py-2"
                data-testid="settings-workspace-row"
                data-workspace-id={id}
              >
                <span
                  className="font-mono text-[13px]"
                  data-testid="settings-workspace-id"
                >
                  {id}
                </span>
                {isActive ? (
                  <span
                    className="text-[11px] uppercase tracking-wide text-[var(--accent-trust)]"
                    data-testid="settings-workspace-active-marker"
                  >
                    active
                  </span>
                ) : null}
                <div className="ml-auto flex items-center gap-2">
                  <button
                    type="button"
                    className="text-[12px] border border-[var(--border)] rounded-md px-2.5 py-1 hover:bg-[var(--bg-subtle)]"
                    data-testid="settings-rename-button"
                    onClick={() => setRenameTarget(id)}
                  >
                    Rename
                  </button>
                  <button
                    type="button"
                    className="text-[12px] border rounded-md px-2.5 py-1 disabled:opacity-50 disabled:cursor-not-allowed"
                    style={
                      canDelete
                        ? {
                            borderColor: "var(--accent-danger)",
                            color: "var(--accent-danger)",
                          }
                        : {
                            borderColor: "var(--border)",
                            color: "var(--foreground-muted)",
                          }
                    }
                    data-testid="settings-delete-button"
                    disabled={!canDelete}
                    aria-disabled={!canDelete}
                    title={
                      canDelete
                        ? "Delete this workspace"
                        : "Cannot delete the last workspace"
                    }
                    onClick={() => {
                      if (!canDelete) return;
                      setDeleteTarget(id);
                    }}
                  >
                    Delete
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      )}

      {workspaces.length === 1 ? (
        <p
          className="text-[12px] text-[var(--foreground-muted)] mt-3"
          data-testid="settings-delete-disabled-hint"
        >
          The last workspace cannot be deleted — the dashboard would have
          nothing to render. Create a new workspace first.
        </p>
      ) : null}

      {renameTarget !== null ? (
        <RenameWorkspaceDialog
          oldId={renameTarget}
          onClose={() => setRenameTarget(null)}
        />
      ) : null}
      {deleteTarget !== null ? (
        <DeleteWorkspaceDialog
          id={deleteTarget}
          onClose={() => setDeleteTarget(null)}
        />
      ) : null}
    </section>
  );
}
