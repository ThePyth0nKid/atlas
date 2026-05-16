/**
 * W20a — Regex parity guard between the bridge and the client mirror.
 *
 * `apps/atlas-web/src/lib/workspace-context.tsx` carries a duplicate
 * of the bridge's `WORKSPACE_ID_RE` because the bridge module is
 * server-only (it imports `node:fs` at load time). Drift between the
 * two copies is a silent UX bug: a workspace id that passes the
 * client check but fails the server check renders a confusing 400
 * after the optimistic UI update.
 *
 * This test fails the moment the two regexes diverge — pattern or
 * flags. The fix is always to bring the client mirror back into sync
 * with the bridge source-of-truth (or vice versa, with a deliberate
 * coordinated change).
 */

import { describe, it, expect } from "vitest";
import { WORKSPACE_ID_RE as bridgeRe } from "@atlas/bridge";
import { WORKSPACE_ID_RE as clientRe } from "./workspace-context";

describe("WORKSPACE_ID_RE regex parity", () => {
  it("client regex matches bridge regex byte-for-byte (source)", () => {
    expect(clientRe.source).toBe(bridgeRe.source);
  });

  it("client regex matches bridge regex (flags)", () => {
    expect(clientRe.flags).toBe(bridgeRe.flags);
  });

  it("client regex toString matches bridge regex toString", () => {
    expect(clientRe.toString()).toBe(bridgeRe.toString());
  });
});
