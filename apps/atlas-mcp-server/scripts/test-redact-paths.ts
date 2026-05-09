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

import { redactPaths, __redactPathConstantsForTest } from "@atlas/bridge";

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

// V1.19 Welle 7: source-of-truth bridge → test seam. Both `SEG` and
// the POSIX lookbehind are imported from `@atlas/bridge` rather than
// duplicated here. Pre-Welle-7 each lived as a separate string literal
// in this file and could silently drift from `signer.ts` on a future
// char-class or lookbehind change. The seam closes the drift hazard.
const SEG = __redactPathConstantsForTest.PATH_SEGMENT;
const POSIX_LB = __redactPathConstantsForTest.POSIX_PATH_LOOKBEHIND;

function expectRedacted(name: string, input: string): void {
  const got = redactPaths(input);
  // Postcondition is intentionally strict: not only must `<path>`
  // appear, but no path-shaped substring may survive in the output.
  // The detector regexes are constructed from the same `SEG` and
  // `POSIX_LB` strings the source uses, so a future change to either
  // propagates here automatically.
  const winLeak = new RegExp(`[A-Za-z]:[\\\\/]${SEG}[\\\\/]${SEG}`);
  const posixLeak = new RegExp(`${POSIX_LB}\\/${SEG}\\/${SEG}`);
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

// ─── Welle 7 — bridge constants seam contract ────────────────────────
//
// Pin the shape of the imported constants so a future bridge edit that
// accidentally renames or restructures `__redactPathConstantsForTest`
// fails this file at module-load rather than at a downstream regex
// constructor where the error message would be opaque. Also pin that
// the lookbehind contains the `.` exclusion (Welle 6 contract): if a
// future contributor reverts that change, this assertion catches it
// before any positive/negative redaction case runs.

check(
  "bridge SEG constant is non-empty regex char-class string",
  typeof SEG === "string" && SEG.length > 0 && SEG.includes("A-Za-z"),
  `got: ${JSON.stringify(SEG)}`,
);
check(
  "bridge POSIX_PATH_LOOKBEHIND contains `.` exclusion (Welle 6 contract)",
  typeof POSIX_LB === "string" && POSIX_LB.startsWith("(?<!") && POSIX_LB.includes("."),
  `got: ${JSON.stringify(POSIX_LB)}`,
);
// Welle 7 security-review M-2: exact-equality "golden" pins that turn
// any future change to either constant into an intentional test
// update. The structural checks above catch coarse renames; these
// catch silent char-class widening (e.g. inserting `\s` into SEG)
// that the structural smoke tests would miss.
expectEqual(
  "bridge SEG constant exact-equality golden pin",
  SEG,
  "[A-Za-z0-9._\\-+@~=,%]+",
);
expectEqual(
  "bridge POSIX_PATH_LOOKBEHIND exact-equality golden pin",
  POSIX_LB,
  "(?<![.:\\w/])",
);
// L-1 follow-through: pin that the export object is frozen so a
// future contributor cannot accidentally remove `Object.freeze` and
// re-introduce the mutability hazard documented in security review.
check(
  "bridge __redactPathConstantsForTest is Object.frozen (Welle 7 L-1 contract)",
  Object.isFrozen(__redactPathConstantsForTest),
  `got: isFrozen=${Object.isFrozen(__redactPathConstantsForTest)}`,
);

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

// V1.19 Welle 6 — pin that ABSOLUTE paths containing dotfile segments
// (`.cache/`, `.config/`, `.git/`) still redact. The Welle 6 lookbehind
// addition `(?<![.:\w/])` only suppresses redaction when `.` is the
// character DIRECTLY preceding the matched leading `/`. In an absolute
// path like `/home/user/.cache/foo`, the matched `/` (before `home`) is
// preceded by whitespace, NOT by `.`. A future regression that
// over-tightens the lookbehind to also exclude any `.` mid-path would
// break this pin.

expectRedacted(
  "POSIX absolute with dotfile segment — .cache",
  "ENOENT: open '/home/user/.cache/atlas/keystore'",
);

expectRedacted(
  "POSIX absolute with dotfile segment — .config",
  "could not read /home/user/.config/atlas/key",
);

expectRedacted(
  "POSIX absolute with dotfile segment — .git refs",
  "EACCES: open '/home/user/repo/.git/refs/heads/master'",
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

// V1.19 Welle 6 — dotted-relative paths (`./` and `../`) must pass
// through verbatim. They expose only filenames, not absolute layout, so
// they're outside the threat model. The prior pattern partial-redacted
// them to operator-hostile `.<path>` / `..<path>` shapes; the lookbehind
// addition `(?<![.:\w/])` closes that.

expectUnchanged(
  "POSIX dotted-relative — single dot",
  "loaded ./scripts/build.js successfully",
);

expectUnchanged(
  "POSIX dotted-relative — double dot",
  "open ../workspace/events.jsonl",
);

expectUnchanged(
  "POSIX dotted-relative — deep parent traversal",
  "import from ../../packages/atlas-bridge/src/foo.ts",
);

expectUnchanged(
  "POSIX dotted-relative — embedded in error message",
  "Error at ./scripts/build.js:42 — unexpected token",
);

expectUnchanged(
  "POSIX dotted-relative — quoted",
  "open './data/events.jsonl' for read",
);

expectUnchanged(
  "POSIX bare relative — no leading dot",
  "compiled src/lib/atlas/storage.ts in 1.2s",
);

expectUnchanged(
  "POSIX bare relative — multi-segment",
  "tsc found packages/atlas-bridge/src/index.ts unused",
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
