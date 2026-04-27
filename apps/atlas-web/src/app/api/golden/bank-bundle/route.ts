import { NextResponse } from "next/server";
import { promises as fs } from "node:fs";
import path from "node:path";

/**
 * Serve the golden pubkey-bundle that pins the keys used to sign the bank trace.
 * Verifier checks this bundle's hash against `pubkey_bundle_hash` in the trace.
 */
export async function GET() {
  const filePath = path.resolve(
    process.cwd(),
    "../../examples/golden-traces/bank-q1-2026.pubkey-bundle.json",
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
      { error: `could not read pubkey bundle: ${(e as Error).message}` },
      { status: 500 },
    );
  }
}
