#!/usr/bin/env bash
# check-lockfile-integrity-bun-text.sh — text-format bun.lock side.
#
# Bun >= 1.2 ships a text lockfile (`bun.lock`) — a JSONC-ish syntax
# that we can parse via node (it is mostly JSON with comments).
# Older bun versions ship `bun.lockb` (binary) — handled by
# check-lockfile-integrity-bun.sh.
#
# bun.lock structure (simplified):
#
#   {
#     "lockfileVersion": 1,
#     "packages": {
#       "@atlas-trust/verify-wasm": [
#         "@atlas-trust/verify-wasm@1.15.0",
#         "",
#         {},
#         "sha512-…"
#       ]
#     }
#   }
#
# The hash is the LAST array element. The version is the suffix of
# the FIRST array element (`name@version`).

set -euo pipefail

# shellcheck source=./lib/log.sh
. "$(dirname "$0")/lib/log.sh"

PACKAGE="${ATLAS_PIN_CHECK_PACKAGE:-@atlas-trust/verify-wasm}"
WORKDIR="${ATLAS_PIN_CHECK_WORKDIR:-.}"
FAIL_ON_LOCAL="${ATLAS_PIN_CHECK_FAIL_ON_LOCAL:-false}"

# shellcheck source=./lib/canonicalize-workdir.sh
. "$(dirname "$0")/lib/canonicalize-workdir.sh"

LOCKFILE="$WORKDIR/bun.lock"

if [ ! -f "$LOCKFILE" ]; then
  atlas_fail "no bun.lock in $WORKDIR"
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  atlas_fail "node not found on PATH — needed to parse bun.lock JSONC"
  exit 1
fi

# Parse JSONC by stripping line comments + trailing commas before
# JSON.parse. Bun's lockfile is a strict subset of JSONC (no block
# comments, no trailing commas in current versions, but defensive
# stripping is harmless).
RESULT="$(
  ATLAS_NODE_PKG="$PACKAGE" \
  ATLAS_NODE_LOCKFILE="$LOCKFILE" \
  node -e '
    const fs = require("fs");
    const lockPath = process.env.ATLAS_NODE_LOCKFILE;
    const pkg = process.env.ATLAS_NODE_PKG;
    let raw;
    try {
      raw = fs.readFileSync(lockPath, "utf8");
    } catch (e) {
      console.error("ERROR: cannot read " + lockPath + ": " + e.message);
      process.exit(2);
    }
    // Common case: bun emits valid JSON (no comments, no trailing
    // commas). Try plain JSON.parse first to avoid the JSONC stripper
    // touching the bytes at all.
    let lock;
    try {
      lock = JSON.parse(raw);
    } catch (firstErr) {
      // Fall back to JSONC: strip // line comments and /* block */
      // comments + trailing commas. Must be string-aware so we do not
      // mis-treat // inside a URL like "https://registry.npmjs.org/..."
      // as the start of a line comment.
      const stripped = stripJsonc(raw);
      const cleaned = stripped.replace(/,(\s*[}\]])/g, "$1");
      try {
        lock = JSON.parse(cleaned);
      } catch (secondErr) {
        console.error("ERROR: bun.lock not parseable as JSON or JSONC.");
        console.error("  plain JSON: " + firstErr.message);
        console.error("  JSONC:      " + secondErr.message);
        process.exit(2);
      }
    }

    // String-aware JSONC stripper. Walks the input character by
    // character, tracks whether we are inside a "..." string (with
    // backslash escape handling), and only treats // and /* outside
    // of strings as comment starts. Newlines inside line-comments
    // are preserved so the line numbers in any subsequent JSON.parse
    // error message still match the source.
    function stripJsonc(input) {
      let out = "";
      let inString = false;
      let i = 0;
      const n = input.length;
      while (i < n) {
        const c = input[i];
        const nx = i + 1 < n ? input[i + 1] : "";
        if (inString) {
          out += c;
          if (c === "\\" && i + 1 < n) {
            out += input[i + 1];
            i += 2;
            continue;
          }
          if (c === "\"") {
            inString = false;
          }
          i += 1;
          continue;
        }
        if (c === "\"") {
          inString = true;
          out += c;
          i += 1;
          continue;
        }
        if (c === "/" && nx === "/") {
          // Line comment: skip to newline (preserve the newline).
          i += 2;
          while (i < n && input[i] !== "\n") i += 1;
          continue;
        }
        if (c === "/" && nx === "*") {
          // Block comment: skip to */.
          i += 2;
          while (i < n && !(input[i] === "*" && input[i + 1] === "/")) i += 1;
          if (i >= n) {
            // Unterminated block comment — surface a specific
            // diagnostic instead of letting the truncated `out`
            // produce a generic JSON parse error two levels up.
            throw new Error("unterminated /* … */ block comment in bun.lock at offset " + i);
          }
          i += 2; // skip past */
          continue;
        }
        out += c;
        i += 1;
      }
      return out;
    }
    const packages = lock.packages || {};
    const matches = [];
    for (const [key, value] of Object.entries(packages)) {
      // Key can be "<pkg>" (top-level) or "<workspace>/<pkg>" (nested).
      // Match by suffix.
      if (key === pkg || key.endsWith("/" + pkg)) {
        if (!Array.isArray(value)) {
          console.error("ERROR: packages[" + key + "] is not an array");
          process.exit(2);
        }
        const head = value[0] || "";
        const integrity = value[value.length - 1] || "";
        // Extract version from "name@version" trailing.
        const at = head.lastIndexOf("@");
        const version = at > 0 ? head.slice(at + 1) : "";
        // Bun stores the resolved URL in middle elements; pick first
        // that looks like a URL.
        let resolved = "";
        for (let i = 1; i < value.length - 1; i++) {
          const v = value[i];
          if (typeof v === "string" && (v.startsWith("https://") || v.startsWith("file:"))) {
            resolved = v;
            break;
          }
        }
        matches.push({ key, version, resolved, integrity });
      }
    }
    if (matches.length === 0) {
      console.log("__NOT_FOUND__");
      process.exit(0);
    }
    for (const m of matches) {
      // ASCII Unit Separator (0x1f) instead of tab — see
      // check-lockfile-integrity-npm.sh for rationale.
      console.log([m.key, m.version, m.resolved, m.integrity].join("\x1f"));
    }
  '
)"

if [ "$RESULT" = "__NOT_FOUND__" ]; then
  atlas_fail "package '$PACKAGE' not found in $LOCKFILE — run 'bun add $PACKAGE@<version>' first"
  exit 1
fi

ANY_FAIL=0
ANY_LOCAL=0
ENTRY_COUNT=0
while IFS=$'\x1f' read -r KEY VERSION RESOLVED INTEGRITY; do
  [ -z "$KEY" ] && continue
  ENTRY_COUNT=$((ENTRY_COUNT + 1))
  atlas_info "[bun:$KEY] version='$VERSION' resolved='$RESOLVED' integrity='${INTEGRITY:0:24}…'"

  if [ -z "$INTEGRITY" ]; then
    atlas_fail "[bun:$KEY] missing integrity (last array element)"
    ANY_FAIL=1
    continue
  fi

  # Per-algo minimum-length check — see check-lockfile-integrity-npm.sh
  # for full rationale. Defends against `"integrity": "sha512-"` (empty
  # payload) silently passing the prefix glob.
  case "$INTEGRITY" in
    sha512-*)
      if [ "${#INTEGRITY}" -lt 95 ]; then
        atlas_fail "[bun:$KEY] integrity prefix is 'sha512-' but payload is too short (${#INTEGRITY} chars; minimum is 95). Likely an attacker-controlled or corrupted lockfile. Value: '${INTEGRITY}'"
        ANY_FAIL=1
        continue
      fi
      ;;
    sha384-*)
      if [ "${#INTEGRITY}" -lt 71 ]; then
        atlas_fail "[bun:$KEY] integrity prefix is 'sha384-' but payload is too short (${#INTEGRITY} chars; minimum is 71). Likely an attacker-controlled or corrupted lockfile. Value: '${INTEGRITY}'"
        ANY_FAIL=1
        continue
      fi
      ;;
    sha256-*)
      if [ "${#INTEGRITY}" -lt 51 ]; then
        atlas_fail "[bun:$KEY] integrity prefix is 'sha256-' but payload is too short (${#INTEGRITY} chars; minimum is 51). Likely an attacker-controlled or corrupted lockfile. Value: '${INTEGRITY}'"
        ANY_FAIL=1
        continue
      fi
      ;;
    sha1-*|md5-*)
      atlas_fail "[bun:$KEY] integrity uses weak hash: '${INTEGRITY%%-*}'"
      ANY_FAIL=1
      continue
      ;;
    *)
      atlas_fail "[bun:$KEY] integrity uses unrecognised hash format: '$INTEGRITY'"
      ANY_FAIL=1
      continue
      ;;
  esac

  case "$RESOLVED" in
    https://registry.npmjs.org*|https://registry.yarnpkg.com*|"")
      : # OK or unspecified (default registry)
      ;;
    file:*)
      ANY_LOCAL=1
      if [ "$FAIL_ON_LOCAL" = "true" ]; then
        atlas_fail "[bun:$KEY] resolved to local file: '$RESOLVED'"
        ANY_FAIL=1
        continue
      else
        atlas_warn "[bun:$KEY] resolved to local file: '$RESOLVED' (V1.15 Welle B backup-channel install)"
      fi
      ;;
    https://*)
      atlas_warn "[bun:$KEY] non-canonical HTTPS origin: '$RESOLVED'"
      ;;
    *)
      atlas_fail "[bun:$KEY] non-HTTPS origin: '$RESOLVED'"
      ANY_FAIL=1
      continue
      ;;
  esac

  atlas_pass "[bun:$KEY] integrity-hash present (${INTEGRITY%%-*})"
done <<EOF
$RESULT
EOF

if [ "$ENTRY_COUNT" -eq 0 ]; then
  atlas_fail "internal error — extracted no entries from non-empty result"
  exit 2
fi

if [ "$ANY_FAIL" -ne 0 ]; then
  exit 1
fi

if [ "$ANY_LOCAL" -ne 0 ]; then
  atlas_warn "Layer 2 PASS with backup-channel WARN"
fi

atlas_pass "Layer 2 OK — every bun.lock entry for '$PACKAGE' has an integrity hash"
