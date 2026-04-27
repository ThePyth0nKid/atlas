/**
 * JSONL-on-disk persistence for AtlasEvents.
 *
 * Append-only — every signed event is one line. There is no edit
 * path. Deletes are modelled as superseding events at the
 * application layer; the storage layer never removes a row.
 *
 * V2 swaps this for a pluggable backend (Postgres, S3, FalkorDB).
 * The interface defined here is what the rest of the MCP server
 * depends on; the JSONL impl is just one implementation.
 */

import { promises as fs } from "node:fs";
import { dirname } from "node:path";
import { eventsLogPath, workspaceDir } from "./paths.js";
import type { AtlasEvent } from "./types.js";

export class StorageError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "StorageError";
  }
}

/**
 * Append a single event. Creates the workspace directory and log file
 * if they don't yet exist.
 *
 * Append is atomic at the OS level for short writes (< PIPE_BUF = 4096
 * on POSIX). Each AtlasEvent JSON line easily fits, so concurrent
 * appenders cannot interleave bytes. We do not currently serialize
 * tip-computation reads against appends — see compute_tips_for() for
 * the read-side caveat.
 */
export async function appendEvent(workspaceId: string, event: AtlasEvent): Promise<void> {
  const path = eventsLogPath(workspaceId);
  await fs.mkdir(dirname(path), { recursive: true });
  const line = JSON.stringify(event) + "\n";
  await fs.appendFile(path, line, "utf8");
}

/**
 * Read all events for a workspace. Returns empty array if the log
 * doesn't exist yet (genesis case).
 *
 * V2 swaps this for a paginated reader; V1 just slurps the file.
 */
export async function readAllEvents(workspaceId: string): Promise<AtlasEvent[]> {
  const path = eventsLogPath(workspaceId);
  let raw: string;
  try {
    raw = await fs.readFile(path, "utf8");
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return [];
    throw new StorageError(`failed to read ${path}: ${(e as Error).message}`);
  }
  const lines = raw.split(/\r?\n/).filter((l) => l.trim().length > 0);
  return lines.map((line, i) => {
    try {
      return JSON.parse(line) as AtlasEvent;
    } catch (e) {
      throw new StorageError(
        `events.jsonl line ${i + 1} is not valid JSON: ${(e as Error).message}`,
      );
    }
  });
}

/**
 * Compute the current DAG tips: events whose hash is not referenced as
 * a parent by any other event.
 *
 * NOTE: this is a TS-side helper used to choose `parents` for the next
 * write. It does NOT participate in trust verification — the Rust
 * verifier recomputes tips and rejects mismatches. If this helper has a
 * bug, the verifier catches it via the dag-tip-mismatch check.
 *
 * Read-time race: `appendEvent` may have flushed bytes that this read
 * has not yet seen if both ran concurrently in the same process. For
 * V1 single-process MCP this isn't load-bearing; concurrent multi-MCP
 * deployments are V2.
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
