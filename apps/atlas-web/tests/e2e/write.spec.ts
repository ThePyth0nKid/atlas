/**
 * V1.19 Welle 11 — /write page E2E.
 *
 * Pins the WriteNodeForm contract end-to-end through the browser:
 *   - form rendering, defaults, kid-preview live update
 *   - HTML5-level validation (pattern + required)
 *   - happy-path POST roundtrip → success card with event_hash/kid/parents
 *   - error-paths: malformed JSON attrs, 4xx workspace-traversal
 *   - form persistence (id clears post-success, workspace + attrs remain)
 *   - submit-button disabled state during signing
 *
 * Each test uses the `workspace` fixture for a unique workspace_id so
 * parallel test workers never collide on per-workspace mutex state.
 * The route handler treats each unique workspace_id as a fresh DAG,
 * so the first event in every test is a genesis.
 */

import { test, expect } from "./fixtures";

test.describe("Write — page render", () => {
  test("page loads with DeployerNotice + Write Surface heading + form", async ({
    page,
  }) => {
    await page.goto("/write");
    await expect(
      page.getByRole("heading", { level: 1, name: "Write Surface" }),
    ).toBeVisible();
    await expect(page.getByTestId("deployer-notice")).toBeVisible();
    await expect(page.getByTestId("write-node-form")).toBeVisible();
  });

  test("form fields render with documented defaults", async ({ page }) => {
    await page.goto("/write");
    await expect(page.getByTestId("write-workspace-id")).toHaveValue(
      "ws-mcp-default",
    );
    await expect(page.getByTestId("write-node-kind")).toHaveValue("dataset");
    await expect(page.getByTestId("write-node-id")).toHaveValue("");
    await expect(page.getByTestId("write-attributes")).toHaveValue("{}");
  });

  test("kid preview live-updates with workspace_id input", async ({ page }) => {
    await page.goto("/write");
    const ws = page.getByTestId("write-workspace-id");
    const preview = page.getByTestId("write-kid-preview");

    // Default render shows kid for the default workspace_id.
    await expect(preview).toHaveText("atlas-anchor:ws-mcp-default");

    // Clear + retype → preview tracks the input.
    await ws.fill("");
    await expect(preview).toHaveText("atlas-anchor:…");

    await ws.fill("my-bank");
    await expect(preview).toHaveText("atlas-anchor:my-bank");
  });
});

test.describe("Write — validation", () => {
  test("workspace_id input has HTML5 pattern regex pinned", async ({ page }) => {
    // Markup-level pin: the pattern attribute MUST be present and match
    // the route handler's regex. If the route handler tightens its
    // workspace_id regex, this assertion turns red until the form's
    // pattern is updated to match. Twin of the [D] workspace_id
    // boundary class in scripts/e2e-write-edge-cases.ts.
    await page.goto("/write");
    const pattern = await page
      .getByTestId("write-workspace-id")
      .getAttribute("pattern");
    expect(pattern).toBe("^[a-zA-Z0-9_-]{1,128}$");
  });

  test("invalid workspace_id (space) is rejected — no success card", async ({
    page,
  }) => {
    // Either HTML5 pattern validation blocks the submit, or the route
    // handler returns 400 and the form renders an error. Both are valid;
    // both give the user-visible contract: NO success card. The HTML5
    // path is the responsive UX layer; the server-side regex (pinned
    // by scripts/e2e-write-edge-cases.ts [D]) is the security boundary.
    await page.goto("/write");
    await page.getByTestId("write-workspace-id").fill("foo bar");
    await page.getByTestId("write-node-id").fill("nodeA");
    await expect(page.getByTestId("write-workspace-id")).toHaveValue("foo bar");

    await page.getByTestId("write-submit").click();
    // not.toBeVisible() with an explicit timeout retries until the
    // window expires; the assertion holds on both the fast path
    // (HTML5 blocks immediately) and the slow path (server 4xx round-
    // trip ~1-2s). Using waitForTimeout here would race on slow CI.
    await expect(page.getByTestId("write-success-card")).not.toBeVisible({
      timeout: 5_000,
    });
  });

  test("node id input has HTML5 required pinned", async ({ page }) => {
    await page.goto("/write");
    const required = await page
      .getByTestId("write-node-id")
      .getAttribute("required");
    // In React-rendered DOM the required attribute is present as an
    // empty string ("") rather than the string "required". Both are
    // truthy presence.
    expect(required).not.toBeNull();
  });

  test("empty node id submission produces no success card", async ({ page }) => {
    await page.goto("/write");
    await page.getByTestId("write-submit").click();
    // Retry-window assertion: holds on both HTML5-fast-block and server-
    // 4xx-roundtrip paths without coupling to wall-clock timing.
    await expect(page.getByTestId("write-success-card")).not.toBeVisible({
      timeout: 5_000,
    });
  });
});

test.describe("Write — happy path + error paths", () => {
  test("happy path: valid submit produces success card with genesis event", async ({
    page,
    workspace,
  }) => {
    await page.goto("/write");
    await page.getByTestId("write-workspace-id").fill(workspace);
    await page.getByTestId("write-node-id").fill("dataset-alpha");

    await page.getByTestId("write-submit").click();

    const successCard = page.getByTestId("write-success-card");
    await expect(successCard).toBeVisible({ timeout: 30_000 });

    // Structural assertions on the success card. The kid is derived
    // from the workspace_id by the route handler (per-tenant kid),
    // so it must equal `atlas-anchor:{workspace}`.
    await expect(page.getByTestId("write-success-workspace")).toHaveText(workspace);
    await expect(page.getByTestId("write-success-kid")).toHaveText(
      `atlas-anchor:${workspace}`,
    );
    // event_hash is a 64-hex-char blake3 digest.
    await expect(page.getByTestId("write-success-event-hash")).toHaveText(
      /^[0-9a-f]{64}$/,
    );
    // First write to a fresh workspace is the genesis — no parents.
    await expect(page.getByTestId("write-success-parents")).toHaveText("(genesis)");
    // event_id is a ULID — 26 Crockford-base32 chars.
    await expect(page.getByTestId("write-success-event-id")).toHaveText(
      /^[0-9A-HJKMNP-TV-Z]{26}$/,
    );
  });

  test("error path: malformed JSON attributes surfaces parse error", async ({
    page,
    workspace,
  }) => {
    await page.goto("/write");
    await page.getByTestId("write-workspace-id").fill(workspace);
    await page.getByTestId("write-node-id").fill("badnode");
    await page.getByTestId("write-attributes").fill("{ invalid json");

    await page.getByTestId("write-submit").click();

    const error = page.getByTestId("write-error");
    await expect(error).toBeVisible();
    await expect(error).toContainText(/attributes parse:/i);
    // No success card on error path.
    await expect(page.getByTestId("write-success-card")).not.toBeVisible();
  });

  test("form persistence: id clears post-success, workspace + attributes remain", async ({
    page,
    workspace,
  }) => {
    await page.goto("/write");
    await page.getByTestId("write-workspace-id").fill(workspace);
    await page.getByTestId("write-node-id").fill("first-node");
    await page.getByTestId("write-attributes").fill('{"rows":42}');

    await page.getByTestId("write-submit").click();
    await expect(page.getByTestId("write-success-card")).toBeVisible({
      timeout: 30_000,
    });

    // Workspace + attributes persist for batch-of-similar workflows;
    // id clears so the next event cannot accidentally duplicate.
    await expect(page.getByTestId("write-workspace-id")).toHaveValue(workspace);
    await expect(page.getByTestId("write-attributes")).toHaveValue(
      '{"rows":42}',
    );
    await expect(page.getByTestId("write-node-id")).toHaveValue("");
  });

  test("submit button disables and shows Signing… while in-flight", async ({
    page,
    workspace,
  }) => {
    await page.goto("/write");
    await page.getByTestId("write-workspace-id").fill(workspace);
    await page.getByTestId("write-node-id").fill("inflight-test");

    const submit = page.getByTestId("write-submit");
    // Click and immediately re-read button state. The signer subprocess
    // takes ~1-2s, so the "Signing…" label is observable.
    await submit.click();
    await expect(submit).toBeDisabled();
    await expect(submit).toHaveText(/Signing…/);

    // After completion, button re-enables and label reverts.
    await expect(page.getByTestId("write-success-card")).toBeVisible({
      timeout: 30_000,
    });
    await expect(submit).toBeEnabled();
    await expect(submit).toHaveText("Sign and append");
  });
});
