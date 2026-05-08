#!/usr/bin/env tsx
/**
 * Unit tests for `@atlas/bridge::redactPaths`.
 *
 * V1.19 Welle 3 hardening: the regex was tightened from
 * "any `/` or `[Drive]:[\\/]` followed by non-whitespace" to a stricter
 * shape requiring ≥2 path segments and excluding URL contexts. This
 * test file pins both directions:
 *
 *   * **Positive** — every realistic Node fs / signer error message
 *     shape MUST still redact to `<path>`. A regression here would
 *     re-introduce the very layout-disclosure surface the function
 *     exists to close.
 *
 *   * **Negative** — strings that look path-shaped but are NOT
 *     filesystem paths (URLs, version numbers, dates, fractions,
 *     comment markers, single-segment paths, identifier-like
 *     `Type:Foo`) must pass through untouched. Over-eager redaction
 *     mangled real diagnostic text — operator-hostile noise that
 *     undermines the "keep the diagnostic useful" half of the
 *     contract.
 *
 * Designed to run as `pnpm test:redact-paths` (or directly with
 * `tsx`); assertion failures call `process.exit(1)` so CI integration
 * is `pnpm run test:redact-paths` + non-zero exit.
 */

import { redactPaths } from "@atlas/bridge";

let failures = 0;

function check(name: string, predicate: boolean, detail?: string): void {
  if (predicate) {
    process.stdout.write(`  ok  ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(`  FAIL ${name}${detail !== undefined ? ` — ${detail}` : ""}\n`);
  }
}

function expectEqual(name: string, got: string, want: string): void {
  check(name, got === want, `\n    got:  ${JSON.stringify(got)}\n    want: ${JSON.stringify(want)}`);
}

function expectRedacted(name: string, input: string): void {
  const got = redactPaths(input);
  // Postcondition is intentionally strict: not only must `<path>`
  // appear, but no path-shaped substring may survive in the output.
  // The wider segment class mirrors signer.ts so that a partial match
  // that leaks part of a real path (e.g. truncating at `@scope`)
  // would FAIL this assertion instead of slipping through on the
  // older narrow check.
  const SEG = "[A-Za-z0-9._\\-+@~=,%]+";
  const winLeak = new RegExp(`[A-Za-z]:[\\\\/]${SEG}[\\\\/]${SEG}`);
  const posixLeak = new RegExp(`(?<![:\\w/])\\/${SEG}\\/${SEG}`);
  const uncLeak = new RegExp(`\\\\\\\\${SEG}\\\\${SEG}\\\\${SEG}`);
  check(
    name,
    got.includes("<path>")
      && !winLeak.test(got)
      && !posixLeak.test(got)
      && !uncLeak.test(got),
    `\n    in:  ${JSON.stringify(input)}\n    out: ${JSON.stringify(got)}`,
  );
}

function expectUnchanged(name: string, input: string): void {
  expectEqual(name, redactPaths(input), input);
}

// ─── Positive: realistic fs / signer error shapes MUST redact ──────────

expectRedacted(
  "Node ENOENT — POSIX quoted",
  "ENOENT: no such file or directory, open '/home/user/data/events.jsonl'",
);

expectRedacted(
  "Node EACCES — POSIX bare",
  "EACCES: permission denied, scandir /var/log/atlas",
);

expectRedacted(
  "Node ENOENT — Windows quoted",
  "ENOENT: no such file or directory, open 'C:\\Users\\nelso\\Desktop\\atlas\\data\\foo.txt'",
);

expectRedacted(
  "Node ENOENT — Windows forward-slash",
  "open 'C:/Users/nelso/data/events.jsonl'",
);

expectRedacted(
  "ESM file:// URL in stack trace",
  "Error at signEvent (file:///home/user/dist/signer.js:42:10)",
);

expectRedacted(
  "Rust signer-style — nested directory",
  "could not open keystore at /home/user/.atlas/keys/pin",
);

expectRedacted(
  "Path inside double quotes",
  'failed: "/etc/atlas/config.toml" not readable',
);

// V1.19 Welle 3b — segment-class widening (security-review follow-through).
// These cases would have partial-redacted under the narrower
// `[A-Za-z0-9._-]` class, leaking the segment tail starting at `@`/`+`.

expectRedacted(
  "npm scope segment — POSIX node_modules",
  "ENOENT: open '/home/user/node_modules/@scope/pkg/index.js'",
);

expectRedacted(
  "Nix-store-shaped path — version with `+`",
  "open '/nix/store/abc123-glib-2.74.0+20230301/lib/x.so'",
);

expectRedacted(
  "npm scope segment — Windows node_modules",
  "ENOENT: open 'C:\\Users\\nelso\\repo\\node_modules\\@scope\\pkg\\index.js'",
);

expectRedacted(
  "Windows UNC — server\\share\\path",
  "EACCES: permission denied, open '\\\\FILESERVER\\atlas\\workspace\\events.jsonl'",
);

// ─── Negative: non-paths must pass through unchanged ───────────────────

expectUnchanged(
  "version string",
  "HTTP/1.1 200 OK",
);

expectUnchanged(
  "fraction",
  "step 1/2 complete",
);

expectUnchanged(
  "shorthand",
  "n/a — no anchor available",
);

expectUnchanged(
  "ISO date with slashes",
  "issued 2026/05/08 UTC",
);

expectUnchanged(
  "single-line comment marker",
  "// TODO: handle the empty case",
);

expectUnchanged(
  "https URL with path",
  "fetched https://rekor.sigstore.dev/api/v1/log/entries: 503",
);

expectUnchanged(
  "http URL on loopback with path",
  "POST http://127.0.0.1:3000/api/atlas/write-node returned 400",
);

expectUnchanged(
  "POSIX single-segment (intentional non-match)",
  "open /tmp failed",
);

expectUnchanged(
  "Windows drive + single segment (intentional non-match)",
  "drive C:\\Temp not writable",
);

expectUnchanged(
  "type-signature shape — colon followed by name",
  "expected T:String but got T:Int",
);

expectUnchanged(
  "Node module specifier",
  "Error at Object.openSync (node:fs:603:3)",
);

// ─── Specific output — make sure surrounding context is preserved ──────

expectEqual(
  "POSIX path replaced inline, surrounding text intact",
  redactPaths("ENOENT: open '/home/user/foo.jsonl' failed"),
  "ENOENT: open '<path>' failed",
);

expectEqual(
  "Windows path replaced inline, surrounding text intact",
  redactPaths("open 'C:\\Users\\nelso\\foo' for write"),
  "open '<path>' for write",
);

expectEqual(
  "file:// URL replaced inline",
  redactPaths("at signEvent (file:///home/user/dist/x.js:1:1) more"),
  "at signEvent (<path>) more",
);

expectEqual(
  "URL preserved verbatim",
  redactPaths("fetched https://rekor.sigstore.dev/api/v1: 503"),
  "fetched https://rekor.sigstore.dev/api/v1: 503",
);

expectEqual(
  "multiple paths in same string",
  redactPaths("read '/home/a/b' and '/home/c/d' both failed"),
  "read '<path>' and '<path>' both failed",
);

expectEqual(
  "npm scope path replaced inline, surrounding text intact",
  redactPaths("ENOENT: open '/home/user/node_modules/@scope/pkg/index.js' failed"),
  "ENOENT: open '<path>' failed",
);

expectEqual(
  "UNC path replaced inline, surrounding text intact",
  redactPaths("EACCES: open '\\\\HOST\\share\\dir\\file.txt' failed"),
  "EACCES: open '<path>' failed",
);

// ─── Result ────────────────────────────────────────────────────────────

if (failures > 0) {
  process.stdout.write(`\n  ${failures} FAIL\n`);
  process.exit(1);
}
process.stdout.write("\n  all redactPaths tests passed\n");
