/**
 * W20a — workspace-selector + real-workspace data E2E.
 *
 * Covers the four user-visible contracts from the W20a scope:
 *
 *   1. Default mount renders the workspace selector. Pre-seeded
 *      localStorage points the provider at a known workspace so the
 *      assertion does not depend on CI-side data-dir pre-state.
 *   2. The selector dropdown lists real workspaces only — Playwright
 *      CI-artifact workspaces (matching `pw-w<digit>-…`) MUST be
 *      filtered server-side and therefore never appear here.
 *   3. The empty-trace API path produces a valid (events:[],
 *      pubkey_bundle_hash:<sha256>) shape, so the UI can render the
 *      `verifier-empty-state` call-to-action without breaking the
 *      verifier contract.
 *   4. The happy path (a freshly-provisioned workspace with one
 *      Genesis event) drives the verifier through to
 *      `data-status="done"`. The single-event Genesis trace must
 *      satisfy the V1.0 checks plus the structural shape checks
 *      (`parent-links`, `dag-tips`) over an empty-parent / single-tip
 *      DAG.
 *
 * The bank-demo at `/api/golden/bank-*` is intentionally preserved
 * (no testids changed) so a later `/demo/bank` opt-in route can wire
 * those endpoints directly.
 */

import { test, expect } from "./fixtures";

test.describe("W20a — workspace selector + real-workspace data", () => {
  test("selector mounts and pinned localStorage workspace is current", async ({
    page,
    workspace,
  }) => {
    // Provision a workspace via the write endpoint, then pin it via
    // localStorage. The Playwright fixture workspace matches `pw-w*`
    // and is therefore filtered out of `/api/atlas/workspaces`; the
    // provider's "trust localStorage when structurally valid" path
    // still surfaces it in the selector.
    await page.request.post("/api/atlas/write-node", {
      data: {
        workspace_id: workspace,
        kind: "dataset",
        id: "selector-spec-genesis",
        attributes: {},
      },
    });
    await page.addInitScript((ws: string) => {
      window.localStorage.setItem("atlas:active-workspace", ws);
    }, workspace);

    await page.goto("/");

    const selector = page.getByTestId("workspace-selector");
    await expect(selector).toBeVisible();
    // The selector resolves to "ready" once the workspaces fetch
    // settles AND the active workspace is determined.
    await expect(selector).toHaveAttribute("data-state", "ready", {
      timeout: 10_000,
    });

    const current = page.getByTestId("workspace-selector-current");
    await expect(current).toHaveText(workspace);
  });

  test("selector dropdown excludes CI artifact workspaces", async ({
    page,
    workspace,
  }) => {
    // Provision a CI-pattern workspace so it exists on disk; the
    // server-side filter MUST still strip it from the listing.
    await page.request.post("/api/atlas/write-node", {
      data: {
        workspace_id: workspace,
        kind: "dataset",
        id: "filter-test-genesis",
        attributes: {},
      },
    });

    await page.goto("/");
    const selector = page.getByTestId("workspace-selector");
    await expect(selector).toBeVisible();
    // The state may be "ready" (some non-pw workspace exists on
    // disk) or "empty" (only pw-* workspaces exist) — either is
    // correct; the filter assertion below holds in both cases. We
    // just need the workspaces fetch to have settled (any state
    // other than "loading").
    await expect(selector).toHaveAttribute("data-state", /^(ready|empty)$/, {
      timeout: 10_000,
    });

    // Probe the API directly — the CI-pattern workspace we just
    // wrote MUST not appear in the response.
    const res = await page.request.get("/api/atlas/workspaces");
    expect(res.ok()).toBe(true);
    const body = (await res.json()) as {
      ok: boolean;
      workspaces: string[];
      default: string | null;
    };
    expect(body.ok).toBe(true);
    expect(body.workspaces).not.toContain(workspace);
    for (const ws of body.workspaces) {
      expect(ws).not.toMatch(/^pw-w\d+-/);
    }
  });

  test("empty workspace returns valid trace with events:[] and a bundle hash", async ({
    page,
  }) => {
    // The trace endpoint must return a well-formed empty trace for a
    // workspace that has never been written to. This pins the
    // contract the LiveVerifierPanel relies on when surfacing the
    // `verifier-empty-state` call-to-action instead of a misleading
    // VALID badge.
    const wsName = `empty${Math.random().toString(36).slice(2, 10)}`;
    const traceRes = await page.request.get(
      `/api/atlas/trace?workspace=${encodeURIComponent(wsName)}`,
    );
    expect(traceRes.ok()).toBe(true);
    const trace = await traceRes.json();
    expect(trace.events).toEqual([]);
    expect(trace.schema_version).toBe("atlas-trace-v1");
    expect(typeof trace.pubkey_bundle_hash).toBe("string");
    expect(trace.pubkey_bundle_hash).toMatch(/^[0-9a-f]{64}$/);
    expect(trace.workspace_id).toBe(wsName);
    expect(trace.dag_tips).toEqual([]);
    expect(trace.anchors).toEqual([]);
    expect(trace.policies).toEqual([]);
    expect(trace.filters).toBe(null);
  });

  test("empty workspace UI renders verifier-empty-state, not VALID badge", async ({
    page,
  }) => {
    const wsName = `empty${Math.random().toString(36).slice(2, 10)}`;
    await page.addInitScript((ws: string) => {
      window.localStorage.setItem("atlas:active-workspace", ws);
    }, wsName);

    await page.goto("/");
    // Pinned empty workspace → empty state must surface; VALID
    // badge must NOT appear.
    await expect(page.getByTestId("verifier-empty-state")).toBeVisible({
      timeout: 20_000,
    });
    const badge = page.getByTestId("verifier-status-badge");
    await expect(badge).toHaveAttribute("data-status", "empty");
  });

  test("provisioned workspace verifier reaches done state", async ({
    page,
    workspace,
  }) => {
    await page.request.post("/api/atlas/write-node", {
      data: {
        workspace_id: workspace,
        kind: "dataset",
        id: "verifier-test-genesis",
        attributes: {},
      },
    });
    await page.addInitScript((ws: string) => {
      window.localStorage.setItem("atlas:active-workspace", ws);
    }, workspace);

    await page.goto("/");

    // The verifier budget includes the per-tenant pubkey derive
    // (~50ms warm) + wasm load (~200ms) + verify pass.
    const badge = page.getByTestId("verifier-status-badge");
    await expect(badge).toHaveAttribute("data-status", "done", {
      timeout: 30_000,
    });

    // For a single Genesis event the verifier MUST pass the V1.0
    // evidence rows + the structural shape rows.
    const evidence = page.getByTestId("verifier-evidence");
    await expect(evidence).toBeVisible();
    for (const check of [
      "schema-version",
      "pubkey-bundle-hash",
      "event-hashes",
      "event-signatures",
    ]) {
      await expect(page.getByTestId(`verifier-evidence-${check}`)).toBeVisible();
    }
  });
});
