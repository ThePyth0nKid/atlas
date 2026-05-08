/**
 * V1.19 Welle 1 — atlas-web bridge to the Rust `atlas-signer` binary.
 *
 * DUPLICATED FROM `apps/atlas-mcp-server/src/lib/signer.ts` with the
 * surface PRUNED to the per-tenant write path. Removed:
 *   - signEvent's `secretHex` branch (legacy SPIFFE kids)
 *   - bundle-hash, anchor, chain-export, derive-key subcommands
 *
 * The web process never holds a per-tenant secret. The only paths
 * exposed here are:
 *   - `signEvent` (always with `--derive-from-workspace`)
 *   - `derivePubkeyViaSigner` (public-only)
 *
 * Single-canonicalisation rule: the Rust signer is the only producer
 * of canonical signing-input bytes. The TS side never owns the
 * canonical-JSON formatting; it only spawns the signer and validates
 * the structurally-typed result via Zod.
 *
 * Secret-via-stdin discipline: even though the per-tenant path
 * intentionally has NO secret to ship, we keep the `runProcess`
 * helper's stdin-pipe shape so a future addition (e.g. issuer-side
 * tooling) inherits the safe pattern by default. argv is for
 * NON-SECRET arguments only — `/proc/<pid>/cmdline` is world-readable.
 *
 * Why `JSON.parse` (not `parseAnchorJson`): the MCP variant uses a
 * lossless-JSON parser to preserve large integer fields in anchor
 * receipts (Rekor log indices, OIDC iat/exp). The atlas-web bridge
 * does NOT exercise the anchor surface — only `sign` and
 * `derive-pubkey`, neither of which has any field outside the safe
 * Number range. Using stdlib JSON.parse avoids pulling in the
 * lossless-json dep into the web bundle. If a future Welle adds
 * anchor-receipt handling here, this decision needs to be revisited
 * AND the surface re-audited.
 */

import { spawn } from "node:child_process";
import { resolveSignerBinary } from "./paths";
import { AtlasEventSchema, DerivedPubkeySchema } from "./schema";
import type { AtlasEvent } from "./types";

export class SignerError extends Error {
  /**
   * Captured stderr from the child process, if any. Useful for
   * server-side logging and operator diagnostics. Callers MUST NOT
   * forward this verbatim to a web client — pass it through
   * `redactPaths` (or omit it from the response body) so absolute
   * filesystem paths don't leak into a 500 response.
   */
  readonly stderr?: string;
  constructor(message: string, stderr?: string) {
    super(message);
    this.name = "SignerError";
    this.stderr = stderr;
  }
}

/**
 * Strip absolute filesystem paths (Windows + POSIX) from a string
 * before returning it to a web client. Defence in depth against
 * server-layout disclosure via 500-response messages. Mirrors
 * `storage.ts:sanitiseFsError` so both surfaces redact the same way.
 */
export function redactPaths(s: string): string {
  return s
    .replace(/['"]?[A-Za-z]:[\\/][^\s'"]+['"]?/g, "<path>")
    .replace(/['"]?\/[^\s'"]+['"]?/g, "<path>");
}

export type SignArgs = {
  workspace: string;
  eventId: string;
  ts: string;
  kid: string;
  parents: string[];
  payload: unknown;
  /** V1.9 per-tenant: signer derives the secret internally from this id. */
  deriveFromWorkspace: string;
};

/**
 * Sign an AtlasEvent via `atlas-signer sign` and validate the
 * stdout against AtlasEventSchema before returning.
 */
export async function signEvent(args: SignArgs): Promise<AtlasEvent> {
  const bin = resolveSignerBinary();
  if (!bin) {
    throw new SignerError(
      "atlas-signer binary not found. Build it with `cargo build -p atlas-signer --release` " +
        "or set ATLAS_SIGNER_PATH.",
    );
  }
  const argv = [
    "sign",
    "--workspace",
    args.workspace,
    "--event-id",
    args.eventId,
    "--ts",
    args.ts,
    "--kid",
    args.kid,
    "--parents",
    args.parents.join(","),
    "--payload",
    JSON.stringify(args.payload),
    "--derive-from-workspace",
    args.deriveFromWorkspace,
  ];
  const stdout = await runProcess(bin, argv, { timeoutMs: SIGN_TIMEOUT_MS });
  let parsed: unknown;
  try {
    parsed = JSON.parse(stdout);
  } catch (e) {
    throw new SignerError(
      `signer stdout was not valid JSON: ${(e as Error).message}`,
    );
  }
  const validated = AtlasEventSchema.safeParse(parsed);
  if (!validated.success) {
    throw new SignerError(
      `signer stdout failed AtlasEvent schema: ${validated.error.message}`,
    );
  }
  return validated.data as AtlasEvent;
}

/**
 * Derive the per-tenant `{kid, pubkey_b64url}` for `workspaceId` via
 * `atlas-signer derive-pubkey`. The secret never crosses this
 * boundary. Used by `keys.resolvePerTenantIdentity`.
 */
export async function derivePubkeyViaSigner(
  workspaceId: string,
): Promise<{ kid: string; pubkey_b64url: string }> {
  const bin = resolveSignerBinary();
  if (!bin) {
    throw new SignerError(
      "atlas-signer binary not found. Build it with `cargo build -p atlas-signer --release` " +
        "or set ATLAS_SIGNER_PATH.",
    );
  }
  const stdout = await runProcess(
    bin,
    ["derive-pubkey", "--workspace", workspaceId],
    { timeoutMs: DERIVE_TIMEOUT_MS },
  );
  let parsed: unknown;
  try {
    parsed = JSON.parse(stdout);
  } catch (e) {
    throw new SignerError(
      `derive-pubkey stdout was not valid JSON: ${(e as Error).message}`,
    );
  }
  const validated = DerivedPubkeySchema.safeParse(parsed);
  if (!validated.success) {
    throw new SignerError(
      `derive-pubkey stdout failed schema: ${validated.error.message}`,
    );
  }
  return validated.data;
}

/**
 * V1.19 Welle 1 review-fix: bound child-process resource usage.
 *
 *   * Timeout — a wedged signer must not hold an HTTP connection
 *     open indefinitely. 30 s for `sign`, 5 s for `derive-pubkey`.
 *     On timeout we kill the child and reject with a SignerError.
 *   * Stdout cap — a buggy or compromised signer must not be able
 *     to exhaust the Node heap by streaming arbitrary bytes before
 *     close. 512 KB is well above any realistic AtlasEvent JSON
 *     output (events are < 4 KB) and small enough that overrun is
 *     a clear bug, not a legitimate edge case.
 */
const STDOUT_CAP_BYTES = 512 * 1024;
const SIGN_TIMEOUT_MS = 30_000;
const DERIVE_TIMEOUT_MS = 5_000;

/**
 * Spawn `bin` with `argv`, optionally piping `stdin` into the child's
 * stdin. Resolves with stdout on exit code 0. Rejects with a
 * SignerError that includes stderr context on any non-zero exit, on
 * timeout, or if stdout exceeds STDOUT_CAP_BYTES.
 *
 * The `stdin` parameter is preserved (unused on the per-tenant path)
 * so future surfaces that DO need to pipe a secret inherit the safe
 * pattern. argv must NEVER carry secret material — `/proc/<pid>/cmdline`
 * is world-readable on Linux and shows up in `ps` output universally.
 */
function runProcess(
  bin: string,
  argv: string[],
  opts: { stdin?: string; timeoutMs: number } = { timeoutMs: SIGN_TIMEOUT_MS },
): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, argv, {
      stdio: ["pipe", "pipe", "pipe"],
    });
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];
    let stdoutBytes = 0;
    let settled = false;
    const settle = (fn: () => void) => {
      if (settled) return;
      settled = true;
      fn();
    };

    const timer = setTimeout(() => {
      try {
        child.kill();
      } catch {
        // best-effort kill; the close handler may still fire later
      }
      settle(() =>
        reject(
          new SignerError(
            `atlas-signer timed out after ${opts.timeoutMs}ms`,
          ),
        ),
      );
    }, opts.timeoutMs);

    child.stdout.on("data", (c: Buffer) => {
      stdoutBytes += c.byteLength;
      if (stdoutBytes > STDOUT_CAP_BYTES) {
        try {
          child.kill();
        } catch {
          // noop — child may have already exited
        }
        settle(() =>
          reject(
            new SignerError(
              `atlas-signer stdout exceeded ${STDOUT_CAP_BYTES} bytes`,
            ),
          ),
        );
        return;
      }
      stdoutChunks.push(c);
    });
    child.stderr.on("data", (c: Buffer) => stderrChunks.push(c));
    child.on("error", (e) =>
      settle(() => reject(new SignerError(`failed to spawn signer: ${e.message}`))),
    );
    child.on("close", (code) => {
      clearTimeout(timer);
      if (settled) return;
      const stdout = Buffer.concat(stdoutChunks).toString("utf8");
      const stderr = Buffer.concat(stderrChunks).toString("utf8");
      if (code === 0) {
        settle(() => resolve(stdout));
      } else {
        settle(() =>
          reject(
            new SignerError(
              `atlas-signer exited ${code ?? "?"}: ${stderr.trim() || "(no stderr)"}`,
              stderr.trim() || undefined,
            ),
          ),
        );
      }
    });
    if (opts.stdin !== undefined) {
      child.stdin.end(opts.stdin, "utf8");
    } else {
      child.stdin.end();
    }
  });
}

export const __signerLimitsForTest = {
  STDOUT_CAP_BYTES,
  SIGN_TIMEOUT_MS,
  DERIVE_TIMEOUT_MS,
};
