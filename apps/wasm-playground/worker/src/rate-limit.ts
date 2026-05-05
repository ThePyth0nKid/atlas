/**
 * Rate-limit primitives — V1.16 Welle C.
 *
 * Two layers (security review H3 + M6):
 *   1. Per-prefix limit: 100 req/min keyed on /64 IPv6 prefix or /32 IPv4.
 *      IPv6 /64 (not /128) blocks the trivial IPv6-rotation bypass — a single
 *      attacker on a /64 cannot get more than 100 req/min total even with
 *      18 quintillion address rotations.
 *   2. Global cap: 1000 req/min summed across all prefixes. This caps the
 *      total Class-A op cost on R2 and the AE writeDataPoint volume even if a
 *      large botnet defeats per-prefix limits.
 *
 * Both limits are enforced by a single Durable Object that holds a small
 * in-memory Map<key, [count, windowEndMs]>. Window is fixed (60s, reset on
 * each new window). Single DO instance is the bottleneck — for V1.16's traffic
 * profile (auditor-grade playground) this is comfortably correct. If quota
 * pressure ever becomes a real concern, sharding to a fleet of DOs by hash
 * of the prefix is straightforward (Welle D candidate).
 *
 * Trust-property: the rate-limit is a cost-DoS mitigation, not a CSP policy
 * enforcement. A throttled request is silent-204'd (caller cannot distinguish
 * "rate limited" from "validation failed" — security review's silent-204
 * principle holds). Operator-visibility comes from the categorised internal
 * log line emitted by the receiver.
 */

const PER_PREFIX_LIMIT = 100;
const GLOBAL_LIMIT = 1000;
const WINDOW_MS = 60_000;

const GLOBAL_KEY = "__GLOBAL__";

interface CounterEntry {
  count: number;
  windowEnd: number;
}

/**
 * Durable Object holding the in-memory rate-limit counters for both layers.
 * One singleton instance addressed by `idFromName("global")`.
 */
export class GlobalRateLimitDO {
  private readonly counters = new Map<string, CounterEntry>();

  constructor(_state: DurableObjectState, _env: unknown) {
    // No persistent state — counters are best-effort and reset on DO eviction.
    // Window-based reset means an evict-and-rehydrate during a window costs
    // an attacker AT MOST the per-window limit again, never more.
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const key = url.searchParams.get("key");
    const limit = Number(url.searchParams.get("limit") ?? "0");

    if (!key || !Number.isFinite(limit) || limit <= 0) {
      return new Response(JSON.stringify({ allowed: false, reason: "bad_request" }), {
        status: 400,
        headers: { "content-type": "application/json" },
      });
    }

    const now = Date.now();
    const entry = this.counters.get(key);

    if (!entry || entry.windowEnd <= now) {
      this.counters.set(key, { count: 1, windowEnd: now + WINDOW_MS });
      this.maybeCleanup(now);
      return new Response(JSON.stringify({ allowed: true, count: 1 }), {
        headers: { "content-type": "application/json" },
      });
    }

    if (entry.count >= limit) {
      return new Response(JSON.stringify({ allowed: false, count: entry.count }), {
        headers: { "content-type": "application/json" },
      });
    }

    // Immutable update — replace the map entry rather than mutating it in
    // place. Single-isolate single-thread DO semantics make in-place mutation
    // safe today, but the project rule (typescript/coding-style.md) forbids
    // mutation as defence-in-depth against future refactors that extract this
    // increment path.
    const nextCount = entry.count + 1;
    this.counters.set(key, { count: nextCount, windowEnd: entry.windowEnd });
    return new Response(JSON.stringify({ allowed: true, count: nextCount }), {
      headers: { "content-type": "application/json" },
    });
  }

  /**
   * Drop expired entries opportunistically. Bounded work per call to avoid
   * pathological GC pauses; under realistic load the map size is O(active
   * prefixes per minute) which is small (hundreds).
   */
  private maybeCleanup(now: number): void {
    if (this.counters.size < 1024) return;
    for (const [k, v] of this.counters) {
      if (v.windowEnd <= now) this.counters.delete(k);
    }
  }
}

/**
 * Compute the rate-limit key for a request:
 * - IPv4: `<addr>/32`
 * - IPv6: `<first 64 bits>/64`
 *
 * Uses the `CF-Connecting-IP` header which Cloudflare sets to the client's
 * real IP (and which clients cannot spoof at the edge).
 *
 * Returns `"unknown"` if the header is absent — that bucket gets its own
 * shared limit, which is acceptable because legitimate browsers always send
 * the header at Cloudflare's edge.
 */
export function rateLimitKey(request: Request): string {
  const ip = request.headers.get("CF-Connecting-IP");
  if (!ip) return "unknown/0";

  if (ip.includes(":")) {
    const expanded = expandIpv6(ip);
    if (expanded === null) return `${ip}/128`;
    const hextets = expanded.split(":");
    return `${hextets.slice(0, 4).join(":")}/64`;
  }
  return `${ip}/32`;
}

/**
 * Expand a possibly-compressed IPv6 address (with `::`) to all 8 hextets.
 * Returns null if the input is malformed.
 *
 * Examples:
 *   "2001:db8::1"        → "2001:db8:0:0:0:0:0:1"
 *   "::1"                → "0:0:0:0:0:0:0:1"
 *   "2001:db8:1:2:3:4:5:6" (no compression) → unchanged
 */
function expandIpv6(ip: string): string | null {
  const dcIdx = ip.indexOf("::");
  if (dcIdx === -1) {
    const parts = ip.split(":");
    return parts.length === 8 ? ip : null;
  }
  const leftStr = ip.slice(0, dcIdx);
  const rightStr = ip.slice(dcIdx + 2);
  const left = leftStr === "" ? [] : leftStr.split(":");
  const right = rightStr === "" ? [] : rightStr.split(":");
  const missing = 8 - left.length - right.length;
  if (missing < 0) return null;
  const middle: string[] = new Array<string>(missing).fill("0");
  return [...left, ...middle, ...right].join(":");
}

export interface RateLimitResult {
  /** True if the request is within the limits and may proceed. */
  readonly allowed: boolean;
  /**
   * Categorised reason when `allowed === false`. Used for receiver
   * internal-log categorisation (security review M2).
   */
  readonly reason?: "rate_limited_per_ip" | "rate_limited_global" | "do_unavailable";
}

export interface RateLimitEnv {
  readonly RATE_LIMIT_DO: DurableObjectNamespace;
}

/**
 * Check both rate-limit layers for a request. Returns `{ allowed: true }` if
 * BOTH the per-prefix and the global limits are within bounds; otherwise
 * returns `{ allowed: false, reason }` with the layer that triggered.
 *
 * Semantics: the per-prefix check runs FIRST so that one noisy prefix does
 * not consume the global budget against legitimate other clients. If the
 * per-prefix check rejects, the global counter is NOT incremented.
 *
 * Both checks go to the same DO instance via `idFromName("global")` — single
 * round-trip to a single edge location. Latency ~5-15 ms typical.
 */
export async function checkRateLimit(
  env: RateLimitEnv,
  request: Request,
): Promise<RateLimitResult> {
  const stub = getRateLimitStub(env);
  const prefixKey = rateLimitKey(request);

  try {
    const perPrefix = await callDo(stub, prefixKey, PER_PREFIX_LIMIT);
    if (!perPrefix.allowed) {
      return { allowed: false, reason: "rate_limited_per_ip" };
    }
    const global = await callDo(stub, GLOBAL_KEY, GLOBAL_LIMIT);
    if (!global.allowed) {
      return { allowed: false, reason: "rate_limited_global" };
    }
    return { allowed: true };
  } catch {
    // DO unavailable — fail-closed (silent-204 the request) rather than
    // silently lifting the rate limit. Operator sees the failure via the
    // categorised internal log.
    return { allowed: false, reason: "do_unavailable" };
  }
}

function getRateLimitStub(env: RateLimitEnv): DurableObjectStub {
  const id = env.RATE_LIMIT_DO.idFromName("global");
  return env.RATE_LIMIT_DO.get(id);
}

interface DoResponse {
  allowed: boolean;
  count?: number;
  reason?: string;
}

async function callDo(
  stub: DurableObjectStub,
  key: string,
  limit: number,
): Promise<DoResponse> {
  const url = new URL("https://do/check");
  url.searchParams.set("key", key);
  url.searchParams.set("limit", String(limit));
  const resp = await stub.fetch(url.toString());
  return (await resp.json()) as DoResponse;
}
