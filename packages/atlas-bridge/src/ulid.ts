/**
 * Minimal Crockford-base32 ULID generator.
 *
 * ULIDs sort lexicographically by time, which makes the events.jsonl
 * append-only file grep-friendly without parsing JSON. We only need
 * `monotonic-within-process` for V1; V2 (multi-process) uses the
 * server's wall-clock with a high-resolution counter.
 *
 * Why not pull in `ulid` from npm? Three lines of crypto-PRNG +
 * 26-char encoding aren't worth a dep + transitive surface for an
 * MCP-server boundary that already runs `atlas-signer` per write.
 *
 * V1.19 Welle 5 refactor: the previous implementation kept
 * `lastTimestamp` and `lastRandom` as module-scoped mutable state,
 * and `incrementRandom` mutated its argument in-place. Both violate
 * the immutability convention from `~/.claude/rules/common/coding-style.md`
 * (callers must never observe mutation of values they pass in or
 * receive). The refactor splits the surface into three layers:
 *
 *   * `nextUlid(prev, now, randomSource)` — pure: same inputs → same
 *     outputs; returns `{ id, state }` with no shared references to
 *     `prev`. Suitable for snapshot-style testing and for callers
 *     that want to thread monotonicity state explicitly (e.g. a
 *     future per-workspace sequencer).
 *   * `createUlid(randomSource?)` — factory returning a `(now?) =>
 *     string` callable with state encapsulated in a closure rather
 *     than at module top-level. Each factory call yields an isolated
 *     generator: useful for tests (no cross-test leakage) and for
 *     multi-tenant scenarios where one tenant's ms-collision
 *     shouldn't poke another tenant's random counter.
 *   * `ulid(now?)` — thin wrapper over a singleton `createUlid()`
 *     for backward compatibility with the existing call sites in
 *     `event.ts` and the public package surface.
 */

import { randomBytes } from "node:crypto";

const ENCODING = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"; // Crockford base32, no I/L/O/U
const RANDOM_BYTES = 10; // 80 bits → 16 base32 chars
const TIME_CHARS = 10;

/**
 * Frozen-by-convention state snapshot threaded through `nextUlid`.
 * The fields are `readonly` to express the immutability contract at
 * the type level: a caller who holds a `UlidState` reference cannot
 * mutate it through the public API, and `nextUlid` always returns
 * a brand-new state object rather than mutating its input.
 *
 * Invariants enforced by `nextUlid` at runtime, not just at the
 * type-level: `random.length === RANDOM_BYTES` (10) and `timestamp`
 * is a non-negative finite integer. A caller who hand-constructs a
 * `UlidState` with a shorter `random` would otherwise produce
 * silently-truncated 16-char encodings — see the boundary check.
 */
export interface UlidState {
  readonly timestamp: number;
  readonly random: readonly number[];
}

/**
 * Source of 10 random bytes, called on each clock-advance step. The
 * production default is `crypto.randomBytes`; the seam exists for
 * snapshot-style tests that need deterministic IDs.
 *
 * SECURITY: Production callers MUST leave this defaulted. Injecting a
 * weak or deterministic source produces predictable IDs that flow into
 * `events.jsonl` and degrades collision-resistance in adversarial
 * settings. The signing layer is the actual auth boundary, so a
 * predictable id alone does not enable event forgery — but it does
 * weaken the defence-in-depth that 80 bits of entropy provides for
 * within-ms collision avoidance and audit-log correlation. The
 * runtime guard in `nextUlid` rejects sources that return arrays of
 * the wrong length, but cannot detect low-entropy sources.
 */
export type RandomSource = () => Uint8Array;

const defaultRandomSource: RandomSource = () => randomBytes(RANDOM_BYTES);

/**
 * Pure single-step ULID generator. Given the prior state (or `null`
 * for the first call) and a wall-clock millisecond, returns the
 * encoded id AND a fresh state object suitable for the next call.
 *
 * Monotonicity: when `now <= prev.timestamp` (clock did not advance
 * — common within the same millisecond), the random buffer is
 * incremented by one and the timestamp is held; otherwise the
 * timestamp is updated and the random buffer is regenerated from
 * `randomSource`.
 *
 * `randomSource` is injectable so tests can pin the random bytes
 * for snapshot-style assertions; production callers leave it default
 * (see `RandomSource` SECURITY note).
 *
 * Boundary checks (V1.19 Welle 5 review correction): `now` must be a
 * non-negative finite integer, `prev.random` must have length 10,
 * `randomSource()` must return exactly 10 bytes. Each violation throws
 * `RangeError` rather than silently encoding a malformed id. The
 * within-ms counter overflow case (2^80 increments in the same ms —
 * physically impossible, but the type permits hand-crafted states
 * that approach it) also throws rather than wrapping to all-zeros and
 * silently breaking the strict-monotonic-within-process contract.
 */
export function nextUlid(
  prev: UlidState | null,
  now: number,
  randomSource: RandomSource = defaultRandomSource,
): { readonly id: string; readonly state: UlidState } {
  if (!Number.isInteger(now) || now < 0) {
    throw new RangeError(
      `nextUlid: 'now' must be a non-negative integer, got ${String(now)}`,
    );
  }
  if (prev !== null && prev.random.length !== RANDOM_BYTES) {
    throw new RangeError(
      `nextUlid: 'prev.random' must have length ${RANDOM_BYTES}, got ${prev.random.length}`,
    );
  }
  const collide = prev !== null && now <= prev.timestamp;
  const timestamp = collide ? prev.timestamp : now;
  const random = collide
    ? incrementRandom(prev.random)
    : Array.from(sampleRandom(randomSource));
  const state: UlidState = { timestamp, random };
  return { id: encodeTime(timestamp) + encodeRandom(random), state };
}

function sampleRandom(source: RandomSource): Uint8Array {
  const bytes = source();
  if (bytes.length !== RANDOM_BYTES) {
    throw new RangeError(
      `RandomSource must return exactly ${RANDOM_BYTES} bytes, got ${bytes.length}`,
    );
  }
  return bytes;
}

/**
 * Factory for an isolated monotonic ULID generator. Each call returns
 * a fresh closure with its own state; useful for tests (no
 * cross-suite leakage of the singleton state at module scope) and
 * for future multi-tenant scenarios that want per-tenant
 * monotonicity (avoids cross-tenant coupling of the
 * "ms-collision → counter increment" trigger).
 */
export function createUlid(
  randomSource: RandomSource = defaultRandomSource,
): (now?: number) => string {
  let state: UlidState | null = null;
  return (now: number = Date.now()): string => {
    const result = nextUlid(state, now, randomSource);
    state = result.state;
    return result.id;
  };
}

const defaultUlid = createUlid();

/**
 * Backward-compatible singleton ULID generator. Internally delegates
 * to a process-wide closure created by `createUlid()`; the API is
 * unchanged from the pre-refactor version so existing call sites in
 * `event.ts` and the public package surface continue to work.
 */
export function ulid(now: number = Date.now()): string {
  return defaultUlid(now);
}

function incrementRandom(arr: readonly number[]): number[] {
  // Returns a NEW array; never mutates the input. The previous
  // implementation mutated in-place, which made the module-level
  // `lastRandom` a shared-mutable hazard.
  //
  // V1.19 Welle 5 review correction: the loop falling through (all
  // bytes were 0xff) used to wrap to all-zeros silently. That breaks
  // the strict-monotonic-within-process contract — the wrapped id
  // sorts BEFORE the prior id at the same ms. The wrap requires 2^80
  // increments in the same ms (physically impossible at any real
  // clock rate), but a hand-crafted `UlidState` can put us one step
  // away from it. Throwing is safer than silently breaking sort
  // order; the singleton `ulid()` path can never reach this in
  // practice, so the cost is zero for production callers.
  const out = arr.slice();
  for (let i = out.length - 1; i >= 0; i--) {
    if (out[i] < 0xff) {
      out[i] += 1;
      return out;
    }
    out[i] = 0;
  }
  throw new RangeError(
    "ulid: random counter overflow within a single millisecond — " +
      "monotonicity contract would break. Refusing to emit.",
  );
}

function encodeTime(ts: number): string {
  let out = "";
  let remaining = ts;
  for (let i = 0; i < TIME_CHARS; i++) {
    out = ENCODING[remaining % 32] + out;
    remaining = Math.floor(remaining / 32);
  }
  return out;
}

function encodeRandom(bytes: readonly number[]): string {
  // 10 random bytes = 80 bits → 16 base32 chars exactly. With a
  // shorter input the trailing-bits branch below would silently emit
  // a partial char and the slice() would mask the underrun. The
  // `nextUlid` boundary guard already enforces this length, but a
  // direct caller of `encodeRandom` (none today) would otherwise
  // produce a malformed id silently. Defence-in-depth.
  if (bytes.length !== RANDOM_BYTES) {
    throw new RangeError(
      `encodeRandom: input must have length ${RANDOM_BYTES}, got ${bytes.length}`,
    );
  }
  let out = "";
  let bits = 0;
  let value = 0;
  for (const b of bytes) {
    value = (value << 8) | b;
    bits += 8;
    while (bits >= 5) {
      out += ENCODING[(value >>> (bits - 5)) & 0x1f];
      bits -= 5;
    }
  }
  if (bits > 0) {
    out += ENCODING[(value << (5 - bits)) & 0x1f];
  }
  return out.slice(0, 16);
}
