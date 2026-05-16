/**
 * W20c — /settings route.
 *
 * Server-component shell that delegates to <SettingsContent> (client
 * component). The /settings page collects four panels:
 *
 *   1. Workspace list (rename + delete via context)
 *   2. Supply-chain pins (read-only display of 11 compile-in constants)
 *   3. Signer / embedder / backend status (sub-route of /api/atlas/system/health)
 *   4. V2-γ coming-soon placeholders (retention SLA, key rotation, semantic-
 *      search budget)
 *
 * Why a separate route vs. an inline panel on `/`?
 *   The home page is "Audit Readiness" — a dashboard of the
 *   workspace's signed state. Settings are deployer-side
 *   configuration of WHICH workspaces exist and HOW the layer-3
 *   stack is wired. Mixing those concerns on one page would
 *   conflate "is my data trustworthy?" with "is my install
 *   configured correctly?". Settings warrants its own route.
 */

import { SettingsContent } from "@/components/SettingsContent";

export default function SettingsPage() {
  return (
    <div className="space-y-8">
      <section>
        <h1 className="text-2xl font-semibold tracking-tight mb-1">
          Settings
        </h1>
        <p className="text-[var(--foreground-muted)]">
          Workspace management and layer-3 health. Changes here affect every
          surface in atlas-web.
        </p>
      </section>

      <SettingsContent />
    </div>
  );
}
