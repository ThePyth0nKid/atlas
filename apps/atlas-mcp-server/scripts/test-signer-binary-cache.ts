#!/usr/bin/env tsx
/**
 * Unit tests for `@atlas/bridge::resolveSignerBinary` cache TTL contract.
 *
 * V1.19 Welle 4 hardening: the resolved binary path is cached with a
 * 60-second TTL rather than process-long. The contract this test pins:
 *
 *   * Within the TTL window, repeated calls return the cached value
 *     with no syscalls — verified by deleting the underlying file and
 *     observing that `resolveSignerBinary()` still returns the cached
 *     path until the clock advances past the TTL.
 *
 *   * After TTL expiry, resolution rebuilds from scratch — including
 *     re-reading `ATLAS_SIGNER_PATH`, so env-var rotations propagate
 *     within one TTL window without process restart.
 *
 *   * Negative results (no binary found anywhere) are also TTL'd, so
 *     an operator who runs `cargo build --release` after the process
 *     started sees the new binary picked up within 60s without restart.
 *
 *   * `__signerBinaryCacheForTest.reset()` clears the cache state so
 *     test cases are mutually independent.
 *
 * The test injects a synthetic clock via `setClock`/`restoreClock` so
 * the TTL contract is exercised deterministically without sleeping or
 * relying on real wall-time. Real binaries are not required: a tmp
 * file with arbitrary content satisfies the `existsSync` predicate
 * the resolver uses.
 *
 * Each section runs inside `withSection` which resets cache + restores
 * `ATLAS_SIGNER_PATH` afterwards, so a section that ever throws (today
 * `check`/`expectEqual` do not — they only increment `failures` — but
 * a future author might) cannot bleed env state into the next section.
 *
 * Run as `pnpm test:signer-cache` (or directly with `tsx`); assertion
 * failures call `process.exit(1)` so CI integration is non-zero exit.
 */

import { mkdtempSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  __signerBinaryCacheForTest,
  resolveSignerBinary,
} from "@atlas/bridge";

let assertions = 0;
let failures = 0;

function check(name: string, predicate: boolean, detail?: string): void {
  assertions += 1;
  if (predicate) {
    process.stdout.write(`  ok  ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(
      `  FAIL ${name}${detail !== undefined ? ` — ${detail}` : ""}\n`,
    );
  }
}

function expectEqual<T>(name: string, got: T, want: T): void {
  check(
    name,
    got === want,
    `\n    got:  ${JSON.stringify(got)}\n    want: ${JSON.stringify(want)}`,
  );
}

const tmpDir = mkdtempSync(join(tmpdir(), "atlas-signer-cache-ttl-"));
const fakeA = join(tmpDir, "atlas-signer-a");
const fakeB = join(tmpDir, "atlas-signer-b");
const missing = join(tmpDir, "atlas-signer-never-existed");

const originalEnv = process.env.ATLAS_SIGNER_PATH;

let now = 1_000_000;
__signerBinaryCacheForTest.setClock(() => now);
const TTL = __signerBinaryCacheForTest.TTL_MS;

function withSection(name: string, fn: () => void): void {
  process.stdout.write(`\n${name}\n`);
  // Section setup: fresh cache, fresh env, fresh tmp files.
  __signerBinaryCacheForTest.reset();
  writeFileSync(fakeA, "fake-binary-a\n");
  writeFileSync(fakeB, "fake-binary-b\n");
  try {
    rmSync(missing, { force: true });
  } catch {
    // ignore — file may not exist
  }
  delete process.env.ATLAS_SIGNER_PATH;
  try {
    fn();
  } finally {
    // Section teardown: ALWAYS reset env so a future section gains
    // a clean slate even if the body threw a JS error mid-flight.
    delete process.env.ATLAS_SIGNER_PATH;
    __signerBinaryCacheForTest.reset();
  }
}

process.stdout.write("Running signer-binary cache TTL contract tests.\n");

try {
  withSection("[A] within-TTL cache hit returns stale path", () => {
    process.env.ATLAS_SIGNER_PATH = fakeA;
    expectEqual("first call resolves to fakeA", resolveSignerBinary(), fakeA);
    // Delete the underlying file: a fresh resolution would now fall through.
    // The cache must still return fakeA within the TTL window.
    unlinkSync(fakeA);
    expectEqual(
      "second call within TTL still returns cached fakeA",
      resolveSignerBinary(),
      fakeA,
    );
    now += TTL - 1;
    expectEqual(
      "call at exactly TTL-1 still cached",
      resolveSignerBinary(),
      fakeA,
    );
  });

  withSection("[B] TTL expiry triggers re-resolution + env rotation", () => {
    process.env.ATLAS_SIGNER_PATH = fakeA;
    const t0 = now;
    expectEqual("seed cache with fakeA", resolveSignerBinary(), fakeA);
    process.env.ATLAS_SIGNER_PATH = fakeB;
    now = t0 + TTL - 1;
    expectEqual(
      "env rotation within TTL ignored (cache wins)",
      resolveSignerBinary(),
      fakeA,
    );
    now = t0 + TTL + 1;
    expectEqual(
      "post-TTL call picks up rotated env var",
      resolveSignerBinary(),
      fakeB,
    );
    now += 1;
    expectEqual(
      "new TTL window caches fakeB",
      resolveSignerBinary(),
      fakeB,
    );
  });

  withSection("[C] negative cache also TTL'd", () => {
    // Point at a missing path; release/debug fallbacks may or may not
    // exist on this host, so we cannot pin the absolute null. Instead we
    // pin the relative behaviour: with env=missing and no fallback ever
    // changing, the result must be stable; after writing a new binary at
    // the env path and crossing TTL, that new binary must be picked up.
    process.env.ATLAS_SIGNER_PATH = missing;
    const t0 = now;
    const initial = resolveSignerBinary();
    expectEqual(
      "missing env path falls through deterministically",
      resolveSignerBinary(),
      initial,
    );
    // Within TTL: even after the path becomes valid, cache still serves
    // the original (possibly-null) result.
    writeFileSync(missing, "now-it-exists\n");
    now = t0 + TTL - 1;
    expectEqual(
      "newly-created binary ignored within TTL window",
      resolveSignerBinary(),
      initial,
    );
    now = t0 + TTL + 1;
    expectEqual(
      "post-TTL call picks up newly-created binary",
      resolveSignerBinary(),
      missing,
    );
  });

  withSection("[D] reset() clears cache state", () => {
    process.env.ATLAS_SIGNER_PATH = fakeB;
    expectEqual(
      "after section setup, fresh resolve returns fakeB",
      resolveSignerBinary(),
      fakeB,
    );
    // Switching env immediately and resetting forces a fresh resolve
    // even within the same logical TTL window — the test seam exists
    // precisely so cache-pinning bugs cannot leak across test cases.
    process.env.ATLAS_SIGNER_PATH = fakeA;
    __signerBinaryCacheForTest.reset();
    expectEqual(
      "reset+env-change picks up new path immediately",
      resolveSignerBinary(),
      fakeA,
    );
  });
} finally {
  __signerBinaryCacheForTest.restoreClock();
  __signerBinaryCacheForTest.reset();
  if (originalEnv === undefined) {
    delete process.env.ATLAS_SIGNER_PATH;
  } else {
    process.env.ATLAS_SIGNER_PATH = originalEnv;
  }
  rmSync(tmpDir, { recursive: true, force: true });
}

process.stdout.write(
  `\n${assertions} assertion(s) total, ${failures} failure(s).\n`,
);

if (failures > 0) {
  process.exit(1);
}

process.stdout.write("All signer-binary cache assertions passed.\n");
