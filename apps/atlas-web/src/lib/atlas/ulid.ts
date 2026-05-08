/**
 * V1.19 Welle 1 — ULID generator for atlas-web event ids.
 *
 * DUPLICATED FROM `apps/atlas-mcp-server/src/lib/ulid.ts` (byte-identical).
 *
 * ULIDs sort lexicographically by time, which keeps the events.jsonl
 * append-only file grep-friendly without parsing JSON. Monotonic
 * within a single Node process; concurrent processes can collide on
 * the random tail in the same millisecond, which is acceptable for
 * event ids (the event_hash is what matters for trust).
 */

import { randomBytes } from "node:crypto";

const ENCODING = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"; // Crockford base32, no I/L/O/U

let lastTimestamp = -1;
let lastRandom: number[] = [];

export function ulid(now: number = Date.now()): string {
  if (now <= lastTimestamp) {
    incrementRandom(lastRandom);
  } else {
    lastTimestamp = now;
    lastRandom = Array.from(randomBytes(10));
  }
  return encodeTime(lastTimestamp) + encodeRandom(lastRandom);
}

function encodeTime(ts: number): string {
  let out = "";
  for (let i = 9; i >= 0; i--) {
    const mod = ts % 32;
    out = ENCODING[mod] + out;
    ts = Math.floor(ts / 32);
  }
  return out;
}

function encodeRandom(bytes: number[]): string {
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

function incrementRandom(arr: number[]): void {
  for (let i = arr.length - 1; i >= 0; i--) {
    if (arr[i] < 0xff) {
      arr[i] += 1;
      return;
    }
    arr[i] = 0;
  }
}
