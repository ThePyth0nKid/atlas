/**
 * V1.19 Welle 1 — atlas-web JSONL storage.
 *
 * DUPLICATED FROM `apps/atlas-mcp-server/src/lib/storage.ts`. Append-
 * only, one AtlasEvent per line. The atomic-append guarantee relies
 * on POSIX PIPE_BUF (4096 bytes) — a single AtlasEvent JSON line fits,
 * so concurrent appenders within the same process cannot interleave
 * bytes.
 *
 * Cross-process concurrency (atlas-web and atlas-mcp-server writing
 * to the same `events.jsonl` simultaneously) is V2 territory. For
 * V1.19 single-process deployments this isn't load-bearing.
 */

import { promises as fs } from "node:fs";
import { basename, dirname } from "node:path";
import { eventsLogPath, workspaceDir } from "./paths";
import { AtlasEventSchema } from "./schema";
import type { AtlasEvent } from "./types";

export class StorageError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "StorageError";
  }
}

export async function appendEvent(workspaceId: string, event: AtlasEvent): Promise<void> {
  const path = eventsLogPath(workspaceId);
  await fs.mkdir(dirname(path), { recursive: true });
  const line = JSON.stringify(event) + "\n";
  await fs.appendFile(path, line, "utf8");
}

export async function readAllEvents(workspaceId: string): Promise<AtlasEvent[]> {
  const path = eventsLogPath(workspaceId);
  let raw: string;
  try {
    raw = await fs.readFile(path, "utf8");
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw new StorageError(
      `failed to read ${basename(path)}: ${sanitiseFsError(e as Error)}`,
    );
  }
  const lines = raw.split(/\r?\n/).filter((l) => l.trim().length > 0);
  return lines.map((line, i) => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(line);
    } catch (e) {
      throw new StorageError(
        `events.jsonl line ${i + 1} is not valid JSON: ${(e as Error).message}`,
      );
    }
    const validated = AtlasEventSchema.safeParse(parsed);
    if (!validated.success) {
      throw new StorageError(
        `events.jsonl line ${i + 1} failed AtlasEvent schema: ${validated.error.message}`,
      );
    }
    return validated.data as AtlasEvent;
  });
}

/**
 * Compute current DAG tips: events whose hash is not referenced as a
 * parent by any other event. The Rust verifier recomputes tips and
 * rejects mismatches, so a bug here surfaces as a verification
 * failure rather than a silent semantic drift.
 */
export function computeTips(events: AtlasEvent[]): string[] {
  const referenced = new Set<string>();
  for (const ev of events) {
    for (const p of ev.parent_hashes) referenced.add(p);
  }
  const tips: string[] = [];
  for (const ev of events) {
    if (!referenced.has(ev.event_hash)) tips.push(ev.event_hash);
  }
  tips.sort();
  return tips;
}

export async function ensureWorkspaceDir(workspaceId: string): Promise<void> {
  await fs.mkdir(workspaceDir(workspaceId), { recursive: true });
}

/**
 * Strip absolute filesystem paths from a Node fs error message before
 * surfacing it to a web client. Keeps the diagnostic useful while
 * preventing layout disclosure.
 */
function sanitiseFsError(e: Error): string {
  return e.message.replace(/['"]?[A-Za-z]:[\\/][^\s'"]+['"]?/g, "<path>")
    .replace(/['"]?\/[^\s'"]+['"]?/g, "<path>");
}
