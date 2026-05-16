"use client";

/**
 * W20b-2 — first-run workspace-creation wizard.
 *
 * Rendered on the home page (`/`) WHEN the workspaces fetch has
 * completed AND returned an empty list. Replaces the W20b-1
 * EmptyTier-on-empty-workspace failure mode (where the dashboard
 * tier was empty because there was no workspace, not because the
 * workspace had no events).
 *
 * Behaviour:
 *   - Renders a centered card mirroring the EmptyTier visual
 *     language (dashed border, large header, max-width copy).
 *   - On submit: calls `createWorkspace` from the workspace context;
 *     on 200, redirects to `/write` with the new workspace active.
 *   - Validation: regex `^[a-zA-Z0-9_-]{1,128}$`, enforced via
 *     `pattern` on the input + server-side Zod. Failed submits
 *     render the error inline; the form is re-enabled after.
 *
 * Frozen testids:
 *   - `first-run-wizard`         : the outer section
 *   - `first-run-wizard-input`   : the workspace_id input
 *   - `first-run-wizard-submit`  : the submit button
 *   - `first-run-wizard-error`   : the inline error (only when present)
 */

import { useRouter } from "next/navigation";
import { CreateWorkspaceForm } from "@/components/CreateWorkspaceForm";

export function FirstRunWizard(): React.ReactElement {
  const router = useRouter();

  return (
    <section
      data-testid="first-run-wizard"
      className="border border-dashed border-[var(--border)] rounded-lg p-10 max-w-xl mx-auto"
    >
      <h2 className="text-xl font-semibold tracking-tight mb-2">
        Welcome to Atlas
      </h2>
      <p className="text-[var(--foreground-muted)] mb-6">
        Create your first workspace to start your audit trail.
      </p>
      <CreateWorkspaceForm
        testidPrefix="first-run-wizard"
        defaultValue="ws-personal"
        submitLabel="Create workspace"
        onSuccess={() => {
          router.push("/write");
        }}
      />
    </section>
  );
}
