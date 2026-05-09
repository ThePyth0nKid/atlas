/**
 * Spawn `atlas-signer` to perform every operation that requires the
 * canonical signing-input format or the canonical bundle-hash format.
 *
 * Why a child-process boundary? The trust property is "bit-identical
 * canonical bytes between signer and verifier". The Rust crate is the
 * single source of canonicalisation. If we ship a TS canonicaliser in
 * parallel, we add a second drift surface that the pinned goldens
 * cannot police. So: shell out, both for event signing AND for the
 * pubkey-bundle hash.
 *
 * Each subprocess invocation costs ~10 ms warm. At MCP-write frequencies
 * that is fine. V2 will replace the boundary with an in-process FFI
 * binding and *retain* the single-source rule by linking against the
 * same Rust crate.
 *
 * Secret material is passed via stdin, never via argv. argv values are
 * world-readable in `/proc/<pid>/cmdline` and `ps aux`.
 *
 * V1.19 Welle 1 atlas-web hardenings, now consolidated into the bridge:
 *
 *   * Child-process timeout (default `SIGN_TIMEOUT_MS = 30_000`, `DERIVE_TIMEOUT_MS = 5_000`).
 *     A wedged signer must not hold an HTTP connection open indefinitely.
 *     On timeout we kill the child and reject with a `SignerError`.
 *   * Stdout cap (`STDOUT_CAP_BYTES = 512 * 1024`). A buggy or compromised
 *     signer must not be able to exhaust the Node heap by streaming
 *     arbitrary bytes before close. 512 KB is well above any realistic
 *     AtlasEvent JSON output (events are < 4 KB) and small enough that
 *     overrun is a clear bug, not a legitimate edge case.
 *   * `redactPaths(s)` strips absolute filesystem paths from a string —
 *     defence in depth against server-layout disclosure via a 500 body.
 *     Web callers MUST run any propagated stderr through this before
 *     forwarding to the client.
 */

import { spawn } from "node:child_process";
import { parseAnchorJson } from "./anchor-json.js";
import { resolveSignerBinary } from "./paths.js";
import {
  AnchorChainSchema,
  AnchorEntryArraySchema,
  AtlasEventSchema,
  DerivedIdentitySchema,
  DerivedPubkeySchema,
} from "./schema.js";
import type {
  AnchorChain,
  AnchorEntry,
  AnchorKind,
  AtlasEvent,
} from "./types.js";

/**
 * V1.9: signing-secret source — exactly one of these must be supplied.
 *
 * `secretHex`     — legacy SPIFFE kids on the production path. The hex
 *                   is piped to the signer via stdin (never argv).
 * `deriveFromWorkspace` — V1.9 per-tenant kid path. The signer derives
 *                   the secret internally via HKDF and signs without
 *                   ever emitting it. The TS process never holds the
 *                   per-tenant secret.
 *
 * The shape is a discriminated union so a caller cannot accidentally
 * pass both — the signer's CLI also enforces mutual exclusion, but
 * catching it at the type system keeps the failure local rather than
 * surfacing it as a `code 2` from the child process.
 */
export type SignArgs = {
  workspace: string;
  eventId: string;
  ts: string;
  kid: string;
  parents: string[];
  payload: unknown;
} & (
  | { secretHex: string; deriveFromWorkspace?: never }
  | { deriveFromWorkspace: string; secretHex?: never }
);

export class SignerError extends Error {
  constructor(message: string, readonly stderr?: string) {
    super(message);
    this.name = "SignerError";
  }
}

/**
 * Strip absolute filesystem paths (Windows + POSIX + `file://` URLs +
 * Windows UNC) from a string before returning it to a web client or
 * MCP caller. Defence in depth against server-layout disclosure via
 * 500-response messages.
 *
 * V1.19 Welle 3: this is the SOLE redaction implementation in the
 * bridge. The previous parallel `storage.ts:sanitiseFsError` was
 * collapsed into this function — both surfaces now route here, so a
 * future tightening (or loosening) lands once and applies everywhere.
 *
 * Welle 3b (security-review follow-through): segment char-class
 * widened to include `+@~=,%`, and a UNC pattern was added. The first
 * change closes a partial-disclosure gap on `node_modules/@scope/pkg/…`
 * stack frames and Nix-store-shaped paths (`…/glib-2.74.0+20230301/…`),
 * where the narrower `[A-Za-z0-9._-]` class would truncate the match
 * at `@` or `+` and leak the tail. The second change adds coverage
 * for `\\server\share\…` paths that fs errors emit on Windows hosts
 * mounting network shares — the prior Windows pattern required a
 * drive letter and let UNC pass through unredacted.
 *
 * Recognition rules (each pattern matches absolute paths only):
 *
 *   1. `file://` URLs — the URL scheme leaks the full filesystem
 *      location of an ESM-loaded module via stack traces. Matched as
 *      a whole token so the host portion is redacted along with the
 *      path. Stops at whitespace, quote, backtick, or `)` so a path
 *      embedded in a stack-frame parenthetical leaves the wrapping
 *      `(` and `)` intact.
 *   2. Windows UNC — `\\HOST\SHARE\segment…` with ≥3 components after
 *      the leading `\\`. Two-component (`\\HOST\SHARE`) matches are
 *      intentionally rejected to avoid hitting comment-style markers
 *      and escape sequences in non-path text.
 *   3. Windows absolute — `[Drive]:[\/]segment{1,}/segment` with
 *      ≥2 components after the drive. `C:\Users\nelso\foo.txt`
 *      matches; `C:\foo` does NOT (single-component disclosure is
 *      mild platform info, not layout, and matching it produces
 *      false positives on identifiers like type signatures
 *      `T:String`).
 *   4. POSIX absolute — `/segment/segment{,}` with ≥2 components.
 *      Negative lookbehind `(?<![.:\w/])` excludes non-path contexts
 *      like URLs (`https://host/api/v1`), version strings
 *      (`HTTP/1.1`), and identifiers (`n/a`, `1/2`). The `/`
 *      character following a `:` (URL scheme separator), word
 *      character (URL host), or `.` (relative-path prefix
 *      `./foo/bar`, `../workspace/x`) cannot start a redaction
 *      match — V1.19 Welle 6 added the `.` exclusion so dotted
 *      relative paths pass through verbatim instead of being
 *      partial-redacted to operator-hostile `.<path>` /
 *      `..<path>` shapes (the prior pattern matched the leading
 *      `/` and emitted a fragment that no longer round-tripped to
 *      a usable path for diagnostics).
 *
 * Segment character class — `[A-Za-z0-9._\-+@~=,%]` — covers npm scope
 * markers (`@`), Nix-style version pluses (`+`), CSV-shaped tempdirs
 * (`,`), URL-encoded byte markers (`%`), home-shorthand (`~`), and the
 * ASCII alphanum + `._-` baseline. Structural delimiters (whitespace,
 * `/`, `\`, quotes, backtick, parens) are deliberately excluded so a
 * segment cannot eat past the path's end into surrounding diagnostic
 * text.
 *
 * Trade-offs:
 *
 *   * Single-segment paths (`/foo`, `C:\foo`) are deliberately not
 *     redacted. Real Node fs error messages always include at least
 *     directory + file (`/home/user/foo.txt`, `C:\Users\…\file`),
 *     so the trade-off pays back as far fewer false positives in
 *     non-fs error text without losing realistic redaction.
 *   * Paths containing whitespace (`/home/My Folder/foo`) get
 *     partial-redacted at the first space (`<path> Folder/foo`).
 *     Acceptable — the leaked tail without the leading
 *     drive+username is much less informative.
 *   * Relative paths (`./foo/bar.ts`, `../workspace/events.jsonl`)
 *     are not matched. They expose only filenames, not absolute
 *     layout — outside the threat model this function defends.
 *
 * NOT applied automatically to `SignerError.stderr` — the field is
 * preserved verbatim for server-side logging and operator diagnostics.
 * Web callers MUST pass the propagated stderr through this before
 * forwarding to the client.
 */
const PATH_SEGMENT = "[A-Za-z0-9._\\-+@~=,%]+";
const POSIX_PATH_LOOKBEHIND = "(?<![.:\\w/])";
const FILE_URL_PATTERN = /file:\/\/\/?[^\s'"`)]+/g;
const UNC_PATH_PATTERN = new RegExp(
  `\\\\\\\\${PATH_SEGMENT}(?:\\\\${PATH_SEGMENT}){2,}`,
  "g",
);
const WINDOWS_PATH_PATTERN = new RegExp(
  `[A-Za-z]:[\\\\/](?:${PATH_SEGMENT}[\\\\/]){1,}${PATH_SEGMENT}`,
  "g",
);
const POSIX_PATH_PATTERN = new RegExp(
  `${POSIX_PATH_LOOKBEHIND}\\/(?:${PATH_SEGMENT}\\/){1,}${PATH_SEGMENT}`,
  "g",
);

/**
 * Source-of-truth bridge → test export of the raw POSIX_PATH_PATTERN
 * building blocks. The test (`apps/atlas-mcp-server/scripts/
 * test-redact-paths.ts`) imports both to construct its `expectRedacted`
 * leak-detector regex from the same strings the source uses. Without
 * this seam the test had two physically-separate copies of the same
 * literals and could silently drift if a future char-class or
 * lookbehind change updated only one side. V1.19 Welle 7 introduced
 * this seam after Welle 6 surfaced the drift hazard during the
 * lookbehind tightening (the test-side `posixLeak` regex had to be
 * hand-updated in lockstep with the source pattern).
 *
 * SECURITY: these are non-secret regex literals. Exporting them widens
 * no auth surface; the worst-case misuse is a downstream caller
 * constructing a regex slightly differently than `redactPaths` does
 * internally, which is the caller's responsibility, not a bridge bug.
 */
// `Object.freeze` prevents an importer from mutating the surface
// (e.g., a supply-chain-compromised dependency overwriting
// `PATH_SEGMENT` to weaken a downstream test's leak detector). The
// runtime `POSIX_PATH_PATTERN` is closed over the local `const`
// values at module-load time and is unaffected either way; the freeze
// is defence-in-depth on the exported view alone. Welle 7
// security-review L-1.
export const __redactPathConstantsForTest = Object.freeze({
  PATH_SEGMENT,
  POSIX_PATH_LOOKBEHIND,
});

export function redactPaths(s: string): string {
  return s
    .replace(FILE_URL_PATTERN, "<path>")
    .replace(UNC_PATH_PATTERN, "<path>")
    .replace(WINDOWS_PATH_PATTERN, "<path>")
    .replace(POSIX_PATH_PATTERN, "<path>");
}

/**
 * V1.19 Welle 1: bound child-process resource usage. See module-level
 * doc-comment for the rationale.
 */
const STDOUT_CAP_BYTES = 512 * 1024;
// V1.19 Welle 2 hardening: stderr is also capped to avoid an unbounded
// heap allocation if a wedged or compromised signer streams diagnostics
// at line rate for the whole timeout window. 64 KB is generous for any
// human-readable signer error chain (the Rust signer's longest message
// today is ~600 bytes).
const STDERR_CAP_BYTES = 64 * 1024;
const SIGN_TIMEOUT_MS = 30_000;
const DERIVE_TIMEOUT_MS = 5_000;

export const __signerLimitsForTest = {
  STDOUT_CAP_BYTES,
  STDERR_CAP_BYTES,
  SIGN_TIMEOUT_MS,
  DERIVE_TIMEOUT_MS,
};

export async function signEvent(args: SignArgs): Promise<AtlasEvent> {
  const bin = resolveOrThrow();

  const baseArgv = [
    "sign",
    "--workspace", args.workspace,
    "--event-id", args.eventId,
    "--ts", args.ts,
    "--kid", args.kid,
    "--parents", args.parents.join(","),
    "--payload", JSON.stringify(args.payload),
  ];

  // V1.9: dispatch by secret-source mode. `deriveFromWorkspace` is the
  // hot path for per-tenant kids — the signer derives internally via
  // HKDF and the secret never crosses this subprocess boundary.
  // `secretHex` is the legacy SPIFFE-kid path; the hex is piped via
  // stdin (never argv) to keep it out of the OS process listing.
  let argv: string[];
  let stdin: string | undefined;
  if (args.deriveFromWorkspace !== undefined) {
    argv = [...baseArgv, "--derive-from-workspace", args.deriveFromWorkspace];
    stdin = undefined;
  } else {
    argv = [...baseArgv, "--secret-stdin"];
    stdin = args.secretHex;
  }

  const { stdout, stderr, code } = await runProcess(bin, argv, {
    stdin,
    timeoutMs: SIGN_TIMEOUT_MS,
  });

  if (code !== 0) {
    throw new SignerError(
      `atlas-signer sign exited with code ${code}: ${stderr.trim() || "(no stderr)"}`,
      stderr,
    );
  }

  let parsed: unknown;
  try {
    // Use the same lossless parser as anchor/chain output. AtlasEvent
    // has no large-integer fields today, so the path is observationally
    // identical to JSON.parse — but standardising every signer-stdout
    // boundary on one parser eliminates a future trap where adding such
    // a field (e.g. a nanosecond timestamp) silently truncates digits
    // through this single remaining `JSON.parse` site.
    parsed = parseAnchorJson(stdout);
  } catch (e) {
    throw new SignerError(
      `atlas-signer produced non-JSON output: ${(e as Error).message}`,
      stdout,
    );
  }

  // Runtime validation, not a type assertion. If the signer ever drifts
  // (a Rust struct rename, a shape change), we fail at this boundary
  // with a descriptive Zod error rather than silently writing a
  // malformed event into events.jsonl.
  const validated = AtlasEventSchema.safeParse(parsed);
  if (!validated.success) {
    throw new SignerError(
      `atlas-signer output failed schema validation: ${validated.error.message}`,
      stdout,
    );
  }
  return validated.data as AtlasEvent;
}

/**
 * Compute the deterministic hash of a `PubkeyBundle` by shelling out to
 * the Rust signer's `bundle-hash` subcommand. The bundle is serialised
 * here as JSON and handed to the child on stdin; the child re-parses it
 * via the same `PubkeyBundle::from_json` the verifier uses, then runs
 * the same `deterministic_hash` the verifier runs at compare-time.
 *
 * That keeps the hash rule single-sourced. The MCP server never owns
 * canonical-JSON formatting.
 */
export async function bundleHashViaSigner(bundleJson: string): Promise<string> {
  const bin = resolveOrThrow();
  const { stdout, stderr, code } = await runProcess(bin, ["bundle-hash"], {
    stdin: bundleJson,
    timeoutMs: SIGN_TIMEOUT_MS,
  });
  if (code !== 0) {
    throw new SignerError(
      `atlas-signer bundle-hash exited with code ${code}: ${stderr.trim() || "(no stderr)"}`,
      stderr,
    );
  }
  const hex = stdout.trim();
  if (!/^[0-9a-f]{64}$/.test(hex)) {
    throw new SignerError(
      `atlas-signer bundle-hash returned non-hex output: ${hex.slice(0, 80)}`,
      stdout,
    );
  }
  return hex;
}

/**
 * One item in an anchor batch. Mirrors `AnchorRequest` in
 * `crates/atlas-signer/src/anchor.rs`.
 */
export type AnchorRequest = {
  kind: AnchorKind;
  anchored_hash: string;
};

/**
 * Stdin shape for `atlas-signer anchor`. Mirrors `AnchorBatchInput` in
 * `crates/atlas-signer/src/anchor.rs`.
 *
 * `integrated_time` is caller-supplied (rather than `now`) so smoke
 * tests produce byte-identical anchor output across runs.
 */
export type AnchorBatchInput = {
  items: AnchorRequest[];
  integrated_time: number;
};

/**
 * Optional issuer-side switches for `anchorViaSigner`.
 *
 * `rekorUrl`: when set, the Rust signer POSTs each batch item to
 * `<rekorUrl>/api/v1/log/entries` and emits Sigstore-format
 * `AnchorEntry` rows. When unset, the in-process mock-Rekor issuer
 * runs unchanged. The verifier dispatches by `log_id` regardless,
 * so both shapes flow through the same trust path.
 *
 * The Rust side validates the URL: only `https://` is accepted for
 * non-loopback hosts. Plaintext `http://` is gated to localhost so
 * an operator typo cannot silently submit anchoring signatures over
 * an unencrypted wire. The TS side does NOT duplicate that check —
 * the policy lives in one place, in the signer.
 *
 * `chainPath`: when set, the signer reads the existing
 * `anchor-chain.jsonl` at this path, builds a new `AnchorBatch`
 * committing the freshly-issued entries plus `integrated_time`, and
 * atomically appends one row. Stdout shape (`[AnchorEntry]`) is
 * unchanged — this option only adds a side effect on disk. The
 * signer is the SOLE writer; the MCP server reads but never
 * modifies the chain file.
 */
export type AnchorOptions = {
  rekorUrl?: string;
  chainPath?: string;
};

/**
 * Issue anchor entries for a batch of hashes by shelling out to the
 * Rust signer's `anchor` subcommand. The signer either builds an
 * in-process mock-Rekor checkpoint (default) or POSTs to a live
 * Sigstore Rekor v1 instance (when `options.rekorUrl` is set). It
 * emits one `AnchorEntry` per request in either case.
 *
 * Same single-canonicalisation discipline as `bundleHashViaSigner`:
 * the MCP server never owns Merkle-tree construction or canonical-
 * checkpoint formatting, and the live-vs-mock dispatch happens
 * inside the Rust binary so the TS boundary stays narrow.
 */
export async function anchorViaSigner(
  batch: AnchorBatchInput,
  options: AnchorOptions = {},
): Promise<AnchorEntry[]> {
  const bin = resolveOrThrow();
  const argv = ["anchor"];
  if (options.rekorUrl !== undefined) {
    argv.push("--rekor-url", options.rekorUrl);
  }
  if (options.chainPath !== undefined) {
    argv.push("--chain-path", options.chainPath);
  }
  const { stdout, stderr, code } = await runProcess(bin, argv, {
    stdin: JSON.stringify(batch),
    timeoutMs: SIGN_TIMEOUT_MS,
  });
  if (code !== 0) {
    throw new SignerError(
      `atlas-signer anchor exited with code ${code}: ${stderr.trim() || "(no stderr)"}`,
      stderr,
    );
  }
  let parsed: unknown;
  try {
    // Lossless parse so Sigstore Rekor v1 `tree_id` values
    // (~2^60) survive the spawn boundary intact. See
    // `lib/anchor-json.ts` for the safe-vs-lossless decision rule.
    parsed = parseAnchorJson(stdout);
  } catch (e) {
    throw new SignerError(
      `atlas-signer anchor produced non-JSON output: ${(e as Error).message}`,
      stdout,
    );
  }
  const validated = AnchorEntryArraySchema.safeParse(parsed);
  if (!validated.success) {
    throw new SignerError(
      `atlas-signer anchor output failed schema validation: ${validated.error.message}`,
      stdout,
    );
  }
  return validated.data as AnchorEntry[];
}

/**
 * V1.9: derive a workspace's per-tenant Ed25519 identity by shelling
 * out to `atlas-signer derive-key --workspace <ws>`.
 *
 * The signer owns the master seed and the HKDF-SHA256 derivation rule
 * (`info = "atlas-anchor-v1:" + workspace_id`). This call returns the
 * full triple (`kid`, `pubkey_b64url`, `secret_hex`) — the secret DOES
 * cross the subprocess boundary on this path. Use only in ceremonies
 * (rotation, key inspection) where a TS-side caller genuinely needs
 * the derived secret. Routine event signing should use
 * `signEvent({ deriveFromWorkspace })`, which routes through
 * `sign --derive-from-workspace` and keeps the secret inside the
 * signer process.
 *
 * Bundle assembly should use `derivePubkeyViaSigner` — the secret is
 * not needed to populate `PubkeyBundle.keys`, so the public-only path
 * is strictly preferable.
 */
export type DerivedIdentity = {
  kid: string;
  pubkey_b64url: string;
  secret_hex: string;
};

export async function deriveKeyViaSigner(workspaceId: string): Promise<DerivedIdentity> {
  const bin = resolveOrThrow();
  const { stdout, stderr, code } = await runProcess(
    bin,
    ["derive-key", "--workspace", workspaceId],
    { timeoutMs: DERIVE_TIMEOUT_MS },
  );
  if (code !== 0) {
    throw new SignerError(
      `atlas-signer derive-key exited with code ${code}: ${stderr.trim() || "(no stderr)"}`,
      stderr,
    );
  }
  let parsed: unknown;
  try {
    // Consistent with the rest of the signer-stdout boundary: route
    // through the lossless parser so a future field that grows past
    // safe-integer range cannot silently truncate here.
    parsed = parseAnchorJson(stdout);
  } catch (e) {
    throw new SignerError(
      `atlas-signer derive-key produced non-JSON output: ${(e as Error).message}`,
      stdout,
    );
  }
  const validated = DerivedIdentitySchema.safeParse(parsed);
  if (!validated.success) {
    throw new SignerError(
      `atlas-signer derive-key output failed schema validation: ${validated.error.message}`,
      stdout,
    );
  }
  return validated.data as DerivedIdentity;
}

/**
 * V1.9: derive a workspace's per-tenant kid + pubkey by shelling out
 * to `atlas-signer derive-pubkey --workspace <ws>`. The secret never
 * leaves the signer process on this path — the wire format omits it.
 *
 * The MCP server uses this to assemble per-workspace `PubkeyBundle`s
 * without ever materialising the workspace's signing key in TS heap.
 * Compared to `deriveKeyViaSigner`, this is the strictly safer path
 * for any caller that does not actually need the secret.
 */
export type DerivedPubkey = {
  kid: string;
  pubkey_b64url: string;
};

export async function derivePubkeyViaSigner(workspaceId: string): Promise<DerivedPubkey> {
  const bin = resolveOrThrow();
  const { stdout, stderr, code } = await runProcess(
    bin,
    ["derive-pubkey", "--workspace", workspaceId],
    { timeoutMs: DERIVE_TIMEOUT_MS },
  );
  if (code !== 0) {
    throw new SignerError(
      `atlas-signer derive-pubkey exited with code ${code}: ${stderr.trim() || "(no stderr)"}`,
      stderr,
    );
  }
  let parsed: unknown;
  try {
    parsed = parseAnchorJson(stdout);
  } catch (e) {
    throw new SignerError(
      `atlas-signer derive-pubkey produced non-JSON output: ${(e as Error).message}`,
      stdout,
    );
  }
  const validated = DerivedPubkeySchema.safeParse(parsed);
  if (!validated.success) {
    throw new SignerError(
      `atlas-signer derive-pubkey output failed schema validation: ${validated.error.message}`,
      stdout,
    );
  }
  return validated.data as DerivedPubkey;
}

/**
 * Read a workspace's `anchor-chain.jsonl` content (already loaded by
 * the caller) through the Rust signer's `chain-export` subcommand,
 * returning a validated wire-format `AnchorChain` ready to embed in
 * `AtlasTrace.anchor_chain`.
 *
 * Single-canonicalisation discipline: the chain head is computed by
 * `atlas_trust_core::anchor::chain_head_for` inside the signer — the
 * MCP server never re-implements that path. The signer also re-runs
 * `verify_anchor_chain` before returning, so a chain corrupted on
 * disk fails inside the operator's domain at export time rather than
 * leaking to an offline auditor as an opaque ✗.
 *
 * The caller is responsible for skipping this call when the chain
 * file is missing or empty — the signer rejects empty input.
 */
export async function chainExportViaSigner(jsonlContent: string): Promise<AnchorChain> {
  const bin = resolveOrThrow();
  const { stdout, stderr, code } = await runProcess(bin, ["chain-export"], {
    stdin: jsonlContent,
    timeoutMs: SIGN_TIMEOUT_MS,
  });
  if (code !== 0) {
    throw new SignerError(
      `atlas-signer chain-export exited with code ${code}: ${stderr.trim() || "(no stderr)"}`,
      stderr,
    );
  }
  let parsed: unknown;
  try {
    // Lossless parse — chain-export emits AnchorChain whose history
    // batches embed AnchorEntry rows, including Sigstore tree_id
    // values that exceed JS safe-integer range. The chain head is
    // recomputed by the Rust signer over canonical bytes, so any
    // precision loss here would silently invalidate the head an
    // offline auditor recomputes.
    parsed = parseAnchorJson(stdout);
  } catch (e) {
    throw new SignerError(
      `atlas-signer chain-export produced non-JSON output: ${(e as Error).message}`,
      stdout,
    );
  }
  const validated = AnchorChainSchema.safeParse(parsed);
  if (!validated.success) {
    throw new SignerError(
      `atlas-signer chain-export output failed schema validation: ${validated.error.message}`,
      stdout,
    );
  }
  return validated.data as AnchorChain;
}

function resolveOrThrow(): string {
  const bin = resolveSignerBinary();
  if (!bin) {
    throw new SignerError(
      "atlas-signer binary not found. Run `cargo build --release -p atlas-signer` " +
        "or set ATLAS_SIGNER_PATH.",
    );
  }
  return bin;
}

type ProcResult = { stdout: string; stderr: string; code: number };
type RunProcessOptions = { stdin?: string; timeoutMs: number };

/**
 * Spawn `bin` with `argv`, optionally piping `opts.stdin` into the
 * child's stdin. Resolves with the captured stdout/stderr/exit-code on
 * close; rejects with a `SignerError` on timeout or stdout overflow.
 *
 * The exit-code branch is intentionally NOT a rejection here — every
 * caller wraps a non-zero exit with subcommand-specific context (which
 * subcommand, what argv shape) before throwing. The two synthetic
 * `SignerError` rejections from this function (`timed out`, `stdout
 * exceeded`) carry their own context and short-circuit those wrappers.
 *
 * `stdin` is preserved (unused on the per-tenant derive paths) so
 * future surfaces that DO need to pipe a secret inherit the safe
 * pattern by default. argv must NEVER carry secret material —
 * `/proc/<pid>/cmdline` is world-readable on Linux and shows up in
 * `ps` output universally.
 */
function runProcess(
  bin: string,
  argv: string[],
  opts: RunProcessOptions,
): Promise<ProcResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, argv, { stdio: ["pipe", "pipe", "pipe"] });
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];
    let stdoutBytes = 0;
    let stderrBytes = 0;
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
          new SignerError(`atlas-signer timed out after ${opts.timeoutMs}ms`),
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
    child.stderr.on("data", (c: Buffer) => {
      stderrBytes += c.byteLength;
      if (stderrBytes > STDERR_CAP_BYTES) {
        try {
          child.kill();
        } catch {
          // noop — child may have already exited
        }
        settle(() =>
          reject(
            new SignerError(
              `atlas-signer stderr exceeded ${STDERR_CAP_BYTES} bytes`,
            ),
          ),
        );
        return;
      }
      stderrChunks.push(c);
    });
    child.on("error", (e) =>
      settle(() => reject(new SignerError(`failed to spawn signer: ${e.message}`))),
    );
    child.on("close", (code) => {
      clearTimeout(timer);
      if (settled) return;
      settle(() =>
        resolve({
          stdout: Buffer.concat(stdoutChunks).toString("utf8"),
          stderr: Buffer.concat(stderrChunks).toString("utf8"),
          code: code ?? -1,
        }),
      );
    });
    if (opts.stdin !== undefined) {
      child.stdin.end(opts.stdin, "utf8");
    } else {
      child.stdin.end();
    }
  });
}
