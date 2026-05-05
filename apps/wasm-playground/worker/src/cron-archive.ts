/**
 * Daily cron handler — V1.16 Welle C.
 *
 * Triggered via wrangler.toml `[triggers] crons = ["0 3 * * *"]` at 03:00 UTC
 * each day. Writes a heartbeat marker object to R2 to verify the bucket
 * binding is live and the cron schedule is firing.
 *
 * The full AE → R2 daily aggregation (1 PutObject/day with the previous day's
 * normalised reports as JSONL) requires the AE SQL API, which needs an account-
 * scoped API token in Workers Secrets. That is intentionally deferred to
 * Welle D: AE default retention is 90 days, well beyond the 30-day operational
 * analysis window, so the audit-archive use case is not load-bearing yet.
 *
 * For Welle C we ship the heartbeat:
 *   - proves R2 binding is alive
 *   - proves cron schedule is firing
 *   - gives an operator visibility marker (`wrangler r2 object list` shows
 *     today's heartbeat → cron is healthy)
 *
 * Welle D will replace `runDailyArchive` with the full AE-SQL-query →
 * JSONL → R2.PutObject pipeline. The function shape (`runDailyArchive(env)`)
 * is the stable contract.
 */

export interface ArchiveEnv {
  readonly CSP_REPORTS_R2: R2Bucket;
  readonly ENVIRONMENT: string;
}

/**
 * Run the daily archive. Idempotent: re-running on the same UTC day overwrites
 * the day's marker (no duplicate-write side effects).
 */
export async function runDailyArchive(env: ArchiveEnv): Promise<void> {
  const today = new Date().toISOString().slice(0, 10); // YYYY-MM-DD
  const key = `heartbeat/${today}.json`;

  const payload = {
    heartbeat: true,
    welle: "v1.16-welle-c",
    environment: env.ENVIRONMENT,
    written_at: new Date().toISOString(),
    note: "Welle D will replace this with full AE-SQL → R2 daily archive.",
  };

  const body = `${JSON.stringify(payload)}\n`;

  await env.CSP_REPORTS_R2.put(key, body, {
    httpMetadata: {
      contentType: "application/json",
      cacheControl: "no-store",
    },
    customMetadata: {
      welle: "v1.16-welle-c",
      kind: "heartbeat",
    },
  });

  console.log(
    JSON.stringify({
      cron: "daily-archive",
      r2_key: key,
      bytes: body.length,
      ts: new Date().toISOString(),
    }),
  );
}
