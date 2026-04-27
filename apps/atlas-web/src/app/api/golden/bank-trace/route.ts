import { NextResponse } from "next/server";
import { promises as fs } from "node:fs";
import path from "node:path";

/**
 * Serve the golden Sebastian-Meinhardt-bank trace bundle as JSON.
 * This is the same bundle the WASM verifier in the browser will check.
 *
 * In production this endpoint will be replaced by `/api/bundle/export?period=...&system=...`
 * which generates a fresh bundle from the live FalkorDB graph.
 */
export async function GET() {
  // Resolve path relative to repo root: this file is at
  //   apps/atlas-web/src/app/api/golden/bank-trace/route.ts
  // Repo root is 6 directories up; but we use process.cwd() (Next.js dev runs
  // from apps/atlas-web/), so the trace lives at ../../examples/golden-traces/.
  const filePath = path.resolve(
    process.cwd(),
    "../../examples/golden-traces/bank-q1-2026.trace.json",
  );

  try {
    const data = await fs.readFile(filePath, "utf8");
    return new NextResponse(data, {
      headers: {
        "content-type": "application/json",
        "cache-control": "no-store",
      },
    });
  } catch (e) {
    return NextResponse.json(
      { error: `could not read golden trace: ${(e as Error).message}` },
      { status: 500 },
    );
  }
}
