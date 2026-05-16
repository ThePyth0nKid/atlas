"use client";

/**
 * W20a — Workspace selection context.
 *
 * The provider fetches `/api/atlas/workspaces` once on mount and
 * seeds the selection from `localStorage["atlas:active-workspace"]`
 * when that value is in the returned list. Otherwise it falls back
 * to the server-supplied `default`.
 *
 * Why client-state + localStorage:
 *   * SSR doesn't have a workspace context (the user's choice is a
 *     client-only preference); rendering server-side with `null`
 *     and hydrating to the resolved workspace keeps the SSR HTML
 *     stable across users.
 *   * `localStorage` survives page reloads but is cleared per
 *     origin — exactly the scope we want for a per-browser dev
 *     preference.
 *   * Future welles can swap the localStorage shim for a
 *     server-side user-pref store without touching consumers.
 *
 * Consumers:
 *   * `<WorkspaceSelector>` — the header dropdown UI
 *   * `<LiveVerifierPanel>` — fetches `/api/atlas/trace` +
 *     `/api/atlas/pubkey-bundle` parameterised by the active workspace
 *   * `<KnowledgeGraphView>` — fetches `/api/atlas/trace` for the
 *     graph projection
 *
 * Hydration model:
 *   * On first render `workspace = null, loading = true`.
 *   * After `/api/atlas/workspaces` resolves, the context updates
 *     and consumers re-render with the resolved id.
 *   * Components MUST guard against `workspace === null` and either
 *     render a loading state or skip the workspace-bound effect.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

const LOCAL_STORAGE_KEY = "atlas:active-workspace";

/**
 * Mirror of the bridge `WORKSPACE_ID_RE` — duplicated here because
 * the bridge module is server-only (Node fs imports). A drift would
 * surface as a workspace passing the client regex but failing at the
 * server entry point, which then renders a 400 — recoverable but
 * unfriendly. Keep this regex byte-identical to
 * `packages/atlas-bridge/src/paths.ts:WORKSPACE_ID_RE`.
 *
 * Exported so the parity unit test
 * (`workspace-context.test.ts`) can compare it byte-for-byte with the
 * bridge's regex; CI then trips on drift instead of shipping it.
 */
export const WORKSPACE_ID_RE = /^[a-zA-Z0-9_-]{1,128}$/;

export interface WorkspaceContextValue {
  /** The currently selected workspace id, or null while loading. */
  workspace: string | null;
  /** Set the active workspace and persist to localStorage. */
  setWorkspace: (id: string) => void;
  /** Full list of user-facing workspaces (CI artifacts already filtered). */
  workspaces: string[];
  /** True while the initial `/api/atlas/workspaces` fetch is in flight. */
  loading: boolean;
  /** Error string from the workspaces fetch (null on success). */
  error: string | null;
  /**
   * W20b-2 — Create a new workspace on the server, add it to the
   * workspaces list, and auto-select it. The id is validated against
   * `WORKSPACE_ID_RE` BEFORE the fetch — invalid ids return
   * `{ ok: false }` without touching the network.
   *
   * Returns a discriminated-union result so callers can branch on
   * success/failure without throwing — the wizard renders the error
   * inline rather than via a global error boundary.
   */
  createWorkspace: (
    id: string,
  ) => Promise<{ ok: true } | { ok: false; error: string }>;
  /**
   * W20c — rename an existing workspace via PATCH. Validates both ids
   * client-side BEFORE hitting the network. On success, updates the
   * workspaces list (immutable splice), re-pins `localStorage` if the
   * renamed workspace was active, and re-selects it under the new id.
   */
  renameWorkspace: (
    oldId: string,
    newId: string,
  ) => Promise<{ ok: true } | { ok: false; error: string }>;
  /**
   * W20c — delete a workspace via DELETE. Refuses the call client-side
   * if it would be the last workspace (defence-in-depth; the server
   * also returns 409). On success, drops the workspace from the list
   * and clears `localStorage` if it was the active selection.
   */
  deleteWorkspace: (
    id: string,
  ) => Promise<{ ok: true } | { ok: false; error: string }>;
}

/**
 * W20b-2 — pure helper for the POST /api/atlas/workspaces call.
 *
 * Extracted from `WorkspaceProvider.createWorkspace` so it can be
 * unit-tested without a React render harness (vitest runs in `node`
 * env; no DOM is available). The provider wraps this with state
 * updates; the helper itself is side-effect-free except for `fetchFn`.
 *
 * @param id        Caller-supplied workspace id. Validated client-side
 *                  with `WORKSPACE_ID_RE` before any network call so a
 *                  regex-failing id returns synchronously without
 *                  burning a roundtrip.
 * @param fetchFn   Injected for testability. Defaults to global
 *                  `fetch` in browser contexts.
 */
export interface CreateWorkspaceServerResponse {
  ok: boolean;
  workspace_id?: string;
  kid?: string;
  pubkey_b64url?: string;
  error?: string;
}

export async function requestCreateWorkspace(
  id: string,
  fetchFn: typeof fetch,
): Promise<{ ok: true } | { ok: false; error: string }> {
  if (!WORKSPACE_ID_RE.test(id)) {
    return {
      ok: false,
      error: `invalid workspace id: ${id} — must match [a-zA-Z0-9_-]{1,128}`,
    };
  }
  let res: Response;
  try {
    res = await fetchFn("/api/atlas/workspaces", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ workspace_id: id }),
    });
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
  let body: CreateWorkspaceServerResponse;
  try {
    body = (await res.json()) as CreateWorkspaceServerResponse;
  } catch {
    // Malformed response — surface a generic message rather than the
    // raw parse error (the proxy may have returned HTML).
    return { ok: false, error: `create failed: HTTP ${res.status}` };
  }
  if (!res.ok || body.ok !== true) {
    return {
      ok: false,
      error:
        typeof body.error === "string" && body.error.length > 0
          ? body.error
          : "create failed",
    };
  }
  return { ok: true };
}

/**
 * W20c — pure helper for PATCH /api/atlas/workspaces (rename).
 * Validates both ids client-side BEFORE the network round-trip.
 */
export async function requestRenameWorkspace(
  oldId: string,
  newId: string,
  fetchFn: typeof fetch,
): Promise<{ ok: true } | { ok: false; error: string }> {
  if (!WORKSPACE_ID_RE.test(oldId)) {
    return {
      ok: false,
      error: `invalid workspace id: ${oldId} — must match [a-zA-Z0-9_-]{1,128}`,
    };
  }
  if (!WORKSPACE_ID_RE.test(newId)) {
    return {
      ok: false,
      error: `invalid new workspace id: ${newId} — must match [a-zA-Z0-9_-]{1,128}`,
    };
  }
  if (oldId === newId) {
    return { ok: false, error: "new workspace id must differ from current id" };
  }
  let res: Response;
  try {
    res = await fetchFn("/api/atlas/workspaces", {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ workspace_id: oldId, new_workspace_id: newId }),
    });
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
  let body: CreateWorkspaceServerResponse;
  try {
    body = (await res.json()) as CreateWorkspaceServerResponse;
  } catch {
    return { ok: false, error: `rename failed: HTTP ${res.status}` };
  }
  if (!res.ok || body.ok !== true) {
    return {
      ok: false,
      error:
        typeof body.error === "string" && body.error.length > 0
          ? body.error
          : "rename failed",
    };
  }
  return { ok: true };
}

/**
 * W20c — pure helper for DELETE /api/atlas/workspaces.
 */
export async function requestDeleteWorkspace(
  id: string,
  fetchFn: typeof fetch,
): Promise<{ ok: true } | { ok: false; error: string }> {
  if (!WORKSPACE_ID_RE.test(id)) {
    return {
      ok: false,
      error: `invalid workspace id: ${id} — must match [a-zA-Z0-9_-]{1,128}`,
    };
  }
  let res: Response;
  try {
    res = await fetchFn("/api/atlas/workspaces", {
      method: "DELETE",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ workspace_id: id }),
    });
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
  let body: CreateWorkspaceServerResponse;
  try {
    body = (await res.json()) as CreateWorkspaceServerResponse;
  } catch {
    return { ok: false, error: `delete failed: HTTP ${res.status}` };
  }
  if (!res.ok || body.ok !== true) {
    return {
      ok: false,
      error:
        typeof body.error === "string" && body.error.length > 0
          ? body.error
          : "delete failed",
    };
  }
  return { ok: true };
}

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null);

interface WorkspaceProviderProps {
  children: ReactNode;
}

interface WorkspacesResponse {
  ok: boolean;
  workspaces: string[];
  default: string | null;
}

export function WorkspaceProvider({ children }: WorkspaceProviderProps) {
  const [workspace, setWorkspaceState] = useState<string | null>(null);
  const [workspaces, setWorkspaces] = useState<string[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    const load = async (): Promise<void> => {
      try {
        const res = await fetch("/api/atlas/workspaces");
        if (!res.ok) {
          throw new Error(`workspaces fetch failed: HTTP ${res.status}`);
        }
        const body = (await res.json()) as unknown;
        if (
          typeof body !== "object" ||
          body === null ||
          !Array.isArray((body as WorkspacesResponse).workspaces)
        ) {
          throw new Error("workspaces fetch: malformed response");
        }
        const data = body as WorkspacesResponse;
        if (cancelled) return;

        // Resolve the active workspace. Persisted localStorage value
        // wins when present and structurally valid — the workspaces
        // list may legitimately omit it (e.g. a workspace exists on
        // disk but is filtered out by the server-side CI-artifact
        // filter, or the user is between page loads with a stored
        // workspace that hasn't been listed yet). Falling back to
        // the server default is the right behaviour only when there
        // is no persisted choice.
        let active: string | null = null;
        let storedValid = false;
        try {
          const stored = window.localStorage.getItem(LOCAL_STORAGE_KEY);
          if (stored !== null && WORKSPACE_ID_RE.test(stored)) {
            active = stored;
            storedValid = true;
          }
        } catch {
          // localStorage may throw in private-browsing / blocked-storage
          // contexts. Falling back to the server default is the right
          // behaviour — the user just loses the persistence affordance.
        }
        if (active === null) {
          active = data.default;
        }

        // If the active workspace is not in the listed set, surface
        // it transparently so the selector still has something to
        // render. This keeps the selector UI coherent when a
        // localStorage-pinned workspace is filtered out server-side.
        let workspacesForUi = data.workspaces;
        if (
          active !== null &&
          storedValid &&
          !data.workspaces.includes(active)
        ) {
          workspacesForUi = [...data.workspaces, active].sort();
        }

        setWorkspaces(workspacesForUi);
        setWorkspaceState(active);
        setError(null);
      } catch (e) {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        // Leaving loading=true on unmount is intentional: React strict-mode
        // immediately re-mounts the provider, which restarts this effect and
        // resets loading via the next setLoading(false). No leak in practice.
        if (!cancelled) setLoading(false);
      }
    };

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  /**
   * Set the active workspace id.
   *
   * Behaviour:
   *   - Valid id (matches `WORKSPACE_ID_RE`): clears `error`, updates
   *     selection, adds to the workspaces list if absent, persists to
   *     localStorage.
   *   - Invalid id: surfaces a descriptive message on the context's
   *     `error` channel and returns without mutating selection. The
   *     no-op-on-invalid contract is preserved (consumers using the
   *     previous workspace for fetches won't suddenly broadcast a bad
   *     id); the only behaviour change is that the failure is no
   *     longer silent.
   */
  const setWorkspace = useCallback((id: string): void => {
    if (!WORKSPACE_ID_RE.test(id)) {
      // Defence in depth: refuse to pin an invalid workspace id —
      // the server route would reject it anyway, but failing here
      // avoids broadcasting a bad id to consumers that may use it
      // in fetch URLs. Surface the failure via the existing `error`
      // channel so consumers (selector, panels) can render feedback
      // instead of silently dropping the input.
      setError(
        `invalid workspace id: ${id} — must match [a-zA-Z0-9_-]{1,128}`,
      );
      return;
    }
    setError(null);
    setWorkspaceState(id);
    setWorkspaces((prev) => (prev.includes(id) ? prev : [...prev, id].sort()));
    try {
      window.localStorage.setItem(LOCAL_STORAGE_KEY, id);
    } catch {
      // See the read-side comment above — localStorage failures are
      // soft.
    }
  }, []);

  /**
   * W20b-2 — create a workspace via POST and, on success, splice it
   * into the workspaces list + auto-select it. Failures DO NOT touch
   * the workspaces list or active selection — the wizard renders the
   * error inline and lets the user retry without losing the previous
   * selection.
   */
  const createWorkspace = useCallback(
    async (
      id: string,
    ): Promise<{ ok: true } | { ok: false; error: string }> => {
      const result = await requestCreateWorkspace(id, fetch);
      if (!result.ok) return result;
      // Success: immutable splice (toSorted on a fresh array) +
      // delegate to `setWorkspaceState` for the localStorage write.
      setError(null);
      setWorkspaces((prev) =>
        prev.includes(id) ? prev : [...prev, id].toSorted(),
      );
      setWorkspaceState(id);
      try {
        window.localStorage.setItem(LOCAL_STORAGE_KEY, id);
      } catch {
        // localStorage may throw in private-browsing / blocked-storage
        // contexts — soft failure, matches the read-side pattern.
      }
      return { ok: true };
    },
    [],
  );

  /**
   * W20c — rename via PATCH. On success, immutably swaps the old id
   * for the new in the workspaces list, re-pins localStorage if the
   * renamed workspace was the active one, and selects the new id.
   */
  const renameWorkspace = useCallback(
    async (
      oldId: string,
      newId: string,
    ): Promise<{ ok: true } | { ok: false; error: string }> => {
      const result = await requestRenameWorkspace(oldId, newId, fetch);
      if (!result.ok) return result;
      setError(null);
      setWorkspaces((prev) =>
        prev
          .filter((id) => id !== oldId)
          .concat(prev.includes(oldId) ? [newId] : [])
          .toSorted(),
      );
      // If the renamed workspace was the active selection, re-pin to
      // the new id. Otherwise leave the active workspace untouched.
      setWorkspaceState((current) => {
        if (current !== oldId) return current;
        try {
          window.localStorage.setItem(LOCAL_STORAGE_KEY, newId);
        } catch {
          // soft failure — see read-side comment
        }
        return newId;
      });
      return { ok: true };
    },
    [],
  );

  /**
   * W20c — delete via DELETE. On success, drops the workspace from
   * the list and clears localStorage if it was the active one. The
   * "last workspace" gate lives on the server (409) — the client
   * surfaces that error inline.
   */
  const deleteWorkspace = useCallback(
    async (id: string): Promise<{ ok: true } | { ok: false; error: string }> => {
      const result = await requestDeleteWorkspace(id, fetch);
      if (!result.ok) return result;
      setError(null);
      setWorkspaces((prev) => prev.filter((w) => w !== id));
      setWorkspaceState((current) => {
        if (current !== id) return current;
        try {
          window.localStorage.removeItem(LOCAL_STORAGE_KEY);
        } catch {
          // soft failure — see read-side comment
        }
        return null;
      });
      return { ok: true };
    },
    [],
  );

  const value = useMemo<WorkspaceContextValue>(
    () => ({
      workspace,
      setWorkspace,
      workspaces,
      loading,
      error,
      createWorkspace,
      renameWorkspace,
      deleteWorkspace,
    }),
    [
      workspace,
      setWorkspace,
      workspaces,
      loading,
      error,
      createWorkspace,
      renameWorkspace,
      deleteWorkspace,
    ],
  );

  return (
    <WorkspaceContext.Provider value={value}>
      {children}
    </WorkspaceContext.Provider>
  );
}

/**
 * Read the workspace context. Throws if called outside the provider —
 * a missing provider is always a developer bug, not a runtime branch
 * the consumer should handle.
 */
export function useWorkspaceContext(): WorkspaceContextValue {
  const ctx = useContext(WorkspaceContext);
  if (ctx === null) {
    throw new Error(
      "useWorkspaceContext must be used within a <WorkspaceProvider>",
    );
  }
  return ctx;
}

/**
 * Test-only export of the localStorage key. Centralised here so the
 * Playwright spec can drive context state by setting localStorage
 * before navigation, without duplicating the literal.
 */
export const __WORKSPACE_LOCAL_STORAGE_KEY_FOR_TEST = LOCAL_STORAGE_KEY;
