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
import { resolveSignerBinary } from "./paths.js";
import { AnchorEntryArraySchema, AtlasEventSchema } from "./schema.js";
import type { AnchorEntry, AnchorKind, AtlasEvent } from "./types.js";

export type SignArgs = {
  workspace: string;
  eventId: string;
  ts: string;
  kid: string;
  parents: string[];
  payload: unknown;
  /** 32-byte secret as 64-char hex. Passed to the child via stdin. */
  secretHex: string;
};

export class SignerError extends Error {
  constructor(message: string, readonly stderr?: string) {
    super(message);
    this.name = "SignerError";
  }
}

export async function signEvent(args: SignArgs): Promise<AtlasEvent> {
  const bin = resolveOrThrow();

  const argv = [
    "sign",
    "--workspace", args.workspace,
    "--event-id", args.eventId,
    "--ts", args.ts,
    "--kid", args.kid,
    "--parents", args.parents.join(","),
    "--payload", JSON.stringify(args.payload),
    "--secret-stdin",
  ];

  const { stdout, stderr, code } = await runProcess(bin, argv, args.secretHex);

  if (code !== 0) {
    throw new SignerError(
      `atlas-signer sign exited with code ${code}: ${stderr.trim() || "(no stderr)"}`,
      stderr,
    );
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(stdout);
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
 * Issue mock-Rekor anchor entries for a batch of hashes by shelling out
 * to the Rust signer's `anchor` subcommand. The signer builds the Merkle
 * tree, signs the checkpoint with the dev mock-Rekor key, and emits one
 * `AnchorEntry` per request.
 *
 * Same single-canonicalisation discipline as `bundleHashViaSigner`: the
 * MCP server never owns Merkle-tree construction or canonical-checkpoint
 * formatting. V1.6 swaps the issuer for a real Sigstore POST without
 * touching this boundary.
 */
export async function anchorViaSigner(
  batch: AnchorBatchInput,
): Promise<AnchorEntry[]> {
  const bin = resolveOrThrow();
  const { stdout, stderr, code } = await runProcess(
    bin,
    ["anchor"],
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
    parsed = JSON.parse(stdout);
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
