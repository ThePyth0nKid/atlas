"use client";

/**
 * W20c — Disabled-controls panel for V2-γ placeholders.
 *
 * Three controls that will be wired in V2-γ:
 *   - Retention SLA (days)
 *   - Cipher-key rotation interval (days)
 *   - Semantic-search query budget (per workspace, per day)
 *
 * Each control renders as a disabled input + helper text. The
 * pattern matches the V1.19 Welle 11 coming-soon nav entries: honest
 * UX about what's not yet built instead of fake-functional controls
 * that no-op.
 *
 * Frozen testids:
 *   - settings-coming-soon              (root)
 *   - settings-coming-soon-retention    (retention control)
 *   - settings-coming-soon-rotation     (rotation control)
 *   - settings-coming-soon-budget       (budget control)
 */

export function ComingSoonPanel(): React.ReactElement {
  return (
    <section
      className="border border-[var(--border)] rounded-lg p-5"
      data-testid="settings-coming-soon"
      aria-labelledby="settings-coming-soon-heading"
    >
      <h2
        id="settings-coming-soon-heading"
        className="font-medium mb-1"
      >
        V2-γ — coming soon
      </h2>
      <p className="text-[12px] text-[var(--foreground-muted)] mb-3">
        Placeholders for the next welle of operator controls. None of
        these are wired today — toggling them in the UI would not
        change server behaviour.
      </p>
      <div className="space-y-4">
        <DisabledControl
          testid="settings-coming-soon-retention"
          label="Retention SLA (days)"
          placeholder="365"
          helper="Days an event must remain queryable before pruning is permitted. V2-γ wires this through to a janitor job."
        />
        <DisabledControl
          testid="settings-coming-soon-rotation"
          label="Cipher-key rotation interval (days)"
          placeholder="90"
          helper="Days between automatic per-tenant key rotations. V2-γ ships the rotation ceremony; today rotation is manual via atlas-signer derive-key."
        />
        <DisabledControl
          testid="settings-coming-soon-budget"
          label="Semantic-search query budget (per workspace, per day)"
          placeholder="10000"
          helper="Caps the number of /api/atlas/semantic-search queries before V2-γ enables the route."
        />
      </div>
    </section>
  );
}

interface DisabledControlProps {
  testid: string;
  label: string;
  placeholder: string;
  helper: string;
}

function DisabledControl({
  testid,
  label,
  placeholder,
  helper,
}: DisabledControlProps): React.ReactElement {
  return (
    <div data-testid={testid}>
      <label className="block text-[12px] font-medium mb-1">{label}</label>
      <input
        type="text"
        placeholder={placeholder}
        disabled
        aria-disabled="true"
        className="w-full border border-[var(--border)] rounded-md px-3 py-1.5 text-[13px] bg-[var(--bg-subtle)] text-[var(--foreground-muted)] cursor-not-allowed"
      />
      <p className="text-[12px] text-[var(--foreground-muted)] mt-1">
        {helper}
      </p>
    </div>
  );
}
