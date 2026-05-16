"use client";

/**
 * W20c — Top-level /settings client component.
 *
 * Composition only — each panel owns its own data fetch / mutation
 * logic. Centralising those concerns in the top-level component
 * would force a server round-trip for state changes that only one
 * panel cares about (e.g. signer-status refresh shouldn't bust the
 * supply-chain pins cache).
 */

import { WorkspaceListPanel } from "@/components/WorkspaceListPanel";
import { SupplyChainPinsPanel } from "@/components/SupplyChainPinsPanel";
import { SignerStatusPanel } from "@/components/SignerStatusPanel";
import { ComingSoonPanel } from "@/components/ComingSoonPanel";

export function SettingsContent(): React.ReactElement {
  return (
    <div
      className="space-y-6"
      data-testid="settings-content"
    >
      <WorkspaceListPanel />
      <SignerStatusPanel />
      <SupplyChainPinsPanel />
      <ComingSoonPanel />
    </div>
  );
}
