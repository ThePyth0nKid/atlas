/**
 * Thin wrapper around `@noble/hashes/blake3` so the rest of the MCP
 * server can `import { blake3 } from "./blake3.js"` without leaking the
 * crypto-lib choice. If we ever swap implementations (e.g. for a native
 * binding), this is the only file that changes.
 *
 * @noble/hashes is pure JS, audited, and zero-deps — appropriate for a
 * tiny per-write hash on the MCP boundary. The Rust verifier uses the
 * official `blake3` crate; both implement RFC-equivalent BLAKE3 and
 * produce byte-identical 32-byte digests for the same input.
 */

import { blake3 as nobleBlake3 } from "@noble/hashes/blake3";

export function blake3(input: Uint8Array): Uint8Array {
  return nobleBlake3(input);
}
