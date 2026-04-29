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
  const isDerive = "deriveFromWorkspace" in args && args.deriveFromWorkspace !== undefined;
  const argv = isDerive
    ? [...baseArgv, "--derive-from-workspace", args.deriveFromWorkspace as string]
    : [...baseArgv, "--secret-stdin"];
  const stdin = isDerive ? undefined : (args.secretHex as string);

  const { stdout, stderr, code } = await runProcess(bin, argv, stdin);

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
  const { stdout, stderr, code } = await runProcess(bin, ["bundle-hash"], bundleJson);
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
  const { stdout, stderr, code } = await runProcess(
    bin,
    argv,
    JSON.stringify(batch),
  );
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
  const { stdout, stderr, code } = await runProcess(bin, ["chain-export"], jsonlContent);
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

function runProcess(bin: string, argv: string[], stdin?: string): Promise<ProcResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, argv, { stdio: ["pipe", "pipe", "pipe"] });
    const out: Buffer[] = [];
    const err: Buffer[] = [];
    child.stdout.on("data", (b: Buffer) => out.push(b));
    child.stderr.on("data", (b: Buffer) => err.push(b));
    child.on("error", reject);
    child.on("close", (code) => {
      resolve({
        stdout: Buffer.concat(out).toString("utf8"),
        stderr: Buffer.concat(err).toString("utf8"),
        code: code ?? -1,
      });
    });
    if (stdin !== undefined) {
      child.stdin.end(stdin, "utf8");
    } else {
      child.stdin.end();
    }
  });
}
