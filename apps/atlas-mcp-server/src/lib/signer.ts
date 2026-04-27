/**
 * Spawn `atlas-signer` to build the canonical signing-input and produce
 * a fully signed `AtlasEvent`.
 *
 * Why a child-process boundary here, instead of re-implementing the
 * canonicalisation in TypeScript? Because the trust property is
 * "bit-identical signing-input across signer and verifier". The Rust
 * crate is the single source of canonicalisation. If we ship a TS
 * canonicaliser in parallel, we add a second drift surface that the
 * pinned goldens cannot police. So: shell out.
 *
 * This is a child-process boundary that takes ~10 ms warm. At MCP-write
 * frequencies (one tool call per agent action) that is fine. V2 will
 * replace the boundary with an in-process FFI binding and *retain* the
 * single-source rule by linking against the same Rust crate.
 */

import { spawn } from "node:child_process";
import { resolveSignerBinary } from "./paths.js";
import type { AtlasEvent } from "./types.js";

export type SignArgs = {
  workspace: string;
  eventId: string;
  ts: string;
  kid: string;
  parents: string[];
  payload: unknown;
  secretHex: string;
};

export class SignerError extends Error {
  constructor(message: string, readonly stderr?: string) {
    super(message);
    this.name = "SignerError";
  }
}

export async function signEvent(args: SignArgs): Promise<AtlasEvent> {
  const bin = resolveSignerBinary();
  if (!bin) {
    throw new SignerError(
      "atlas-signer binary not found. Run `cargo build --release -p atlas-signer` " +
        "or set ATLAS_SIGNER_PATH.",
    );
  }

  const argv = [
    "--workspace", args.workspace,
    "--event-id", args.eventId,
    "--ts", args.ts,
    "--kid", args.kid,
    "--parents", args.parents.join(","),
    "--payload", JSON.stringify(args.payload),
    "--secret-hex", args.secretHex,
  ];

  const { stdout, stderr, code } = await runProcess(bin, argv);

  if (code !== 0) {
    throw new SignerError(
      `atlas-signer exited with code ${code}: ${stderr.trim() || "(no stderr)"}`,
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

  return parsed as AtlasEvent;
}

type ProcResult = { stdout: string; stderr: string; code: number };

function runProcess(bin: string, argv: string[]): Promise<ProcResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, argv, { stdio: ["ignore", "pipe", "pipe"] });
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
  });
}
