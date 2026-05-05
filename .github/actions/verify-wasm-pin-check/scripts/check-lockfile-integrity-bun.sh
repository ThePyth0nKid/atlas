#!/usr/bin/env bash
# check-lockfile-integrity-bun.sh — bun-side Layer 2.
#
# Two paths depending on lockfile format:
#
#   1. `bun.lock` (text — bun >= 1.2): grep-able like pnpm.
#   2. `bun.lockb` (binary): delegate to `bun pm ls --json` if `bun`
#      is on PATH, else degrade with a WARN (we can't validate a
#      binary file by hand — a wrong-shape integrity field would
#      silently pass).
#
# Bun's text lockfile format (LSON-ish):
#
#   "@atlas-trust/verify-wasm": ["@atlas-trust/verify-wasm@1.15.0", "", { ... }, "sha512-..."],
#
# The integrity hash is the LAST array element. The version is the
# trailing `@<version>` of the first array element.

set -euo pipefail

# shellcheck source=./lib/log.sh
. "$(dirname "$0")/lib/log.sh"

PACKAGE="${ATLAS_PIN_CHECK_PACKAGE:-@atlas-trust/verify-wasm}"
WORKDIR="${ATLAS_PIN_CHECK_WORKDIR:-.}"
FAIL_ON_LOCAL="${ATLAS_PIN_CHECK_FAIL_ON_LOCAL:-false}"

# shellcheck source=./lib/canonicalize-workdir.sh
. "$(dirname "$0")/lib/canonicalize-workdir.sh"
# Re-export so the bun-text dispatched sub-script reads the same
# canonical path rather than re-canonicalising the raw input.
export ATLAS_PIN_CHECK_WORKDIR="$WORKDIR"

if [ -f "$WORKDIR/bun.lock" ]; then
  bash "$(dirname "$0")/check-lockfile-integrity-bun-text.sh"
  exit $?
fi

if [ ! -f "$WORKDIR/bun.lockb" ]; then
  atlas_fail "no bun.lock or bun.lockb in $WORKDIR — run 'bun install' first or set package-manager: npm/pnpm"
  exit 1
fi

# Binary lockfile path. Need `bun pm ls --json` to read it.
if ! command -v bun >/dev/null 2>&1; then
  atlas_fail "bun.lockb is binary and 'bun' not on PATH — install bun in CI (e.g. 'oven-sh/setup-bun@<sha>') or convert to text format. Cannot validate integrity without bun."
  exit 1
fi

atlas_info "Querying bun pm ls --json (cwd=$WORKDIR)…"

# `bun pm ls --json` outputs a tree; node parses it for us.
# Capture stderr alongside stdout so a non-zero bun exit produces a
# useful diagnostic instead of an opaque "no output" message. The
# subshell + `|| BUN_RC=$?` pattern lets us distinguish empty output
# (bun ran but the project is empty) from a hard bun failure.
BUN_RC=0
LIST_JSON="$(cd "$WORKDIR" && bun pm ls --json 2>&1)" || BUN_RC=$?
if [ "$BUN_RC" -ne 0 ]; then
  atlas_fail "'bun pm ls --json' failed with exit $BUN_RC. Output:"
  printf '%s\n' "$LIST_JSON" >&2
  exit 1
fi
if [ -z "$LIST_JSON" ]; then
  atlas_fail "'bun pm ls --json' produced no output — is the project installed? Try 'bun install' first."
  exit 1
fi

# Hard cap on the JSON size we will hand to node. A maliciously
# crafted bun.lockb could produce gigabyte-scale `bun pm ls --json`
# output; passing that via env var would either OOM the runner
# (V8 default heap ~1.5 GB) or trip Linux ARG_MAX (~2 MB) and
# silently truncate. 10 MB is generous for any realistic Atlas
# consumer tree (the verifier package itself has zero runtime
# dependencies; large output here would indicate either a deeply-
# nested workspace OR an attack).
LIST_JSON_BYTES="${#LIST_JSON}"
if [ "$LIST_JSON_BYTES" -gt 10485760 ]; then
  atlas_fail "'bun pm ls --json' output is $LIST_JSON_BYTES bytes — exceeds the 10 MB safety cap. This is unusual for any realistic project; investigate the lockfile for adversarial nesting."
  exit 1
fi

RESULT="$(
  ATLAS_NODE_PKG="$PACKAGE" \
  ATLAS_NODE_INPUT="$LIST_JSON" \
  node -e '
    const pkg = process.env.ATLAS_NODE_PKG;
    let tree;
    try {
      tree = JSON.parse(process.env.ATLAS_NODE_INPUT);
    } catch (e) {
      console.error("ERROR: bun pm ls --json output is not valid JSON: " + e.message);
      process.exit(2);
    }
    const matches = [];
    // Iterative walk via a queue instead of recursion — defends
    // against an adversarially-deep dependency tree triggering a
    // V8 stack overflow (RangeError: Maximum call stack size
    // exceeded). With recursion, ~10k levels of nesting crashes
    // node. With a queue, depth is bounded only by heap size,
    // which is in turn bounded by the 10 MB input cap above.
    // Visited-set defends against cycles in the bun output
    // (peerDeps can occasionally form cycles in the tree shape).
    const queue = [{ node: tree, path: "" }];
    const visited = new WeakSet();
    while (queue.length > 0) {
      const { node, path } = queue.shift();
      if (!node || typeof node !== "object") continue;
      if (visited.has(node)) continue;
      visited.add(node);
      const deps = node.dependencies || node.peerDependencies || {};
      for (const [name, info] of Object.entries(deps)) {
        if (name === pkg) {
          matches.push({
            path: path + "/" + name,
            version: (info && info.version) || "",
            resolved: (info && (info.resolved || info.tarball)) || "",
            integrity: (info && info.integrity) || "",
          });
        }
        if (info && typeof info === "object") {
          queue.push({ node: info, path: path + "/" + name });
        }
      }
    }
    if (matches.length === 0) {
      console.log("__NOT_FOUND__");
      process.exit(0);
    }
    for (const m of matches) {
      // ASCII Unit Separator (0x1f) instead of tab — see
      // check-lockfile-integrity-npm.sh for rationale.
      console.log([m.path, m.version, m.resolved, m.integrity].join("\x1f"));
    }
  '
)"

if [ "$RESULT" = "__NOT_FOUND__" ]; then
  atlas_fail "package '$PACKAGE' not found in 'bun pm ls' output — run 'bun add $PACKAGE@<version>' first"
  exit 1
fi

ANY_FAIL=0
ANY_LOCAL=0
ENTRY_COUNT=0
while IFS=$'\x1f' read -r SOURCE VERSION RESOLVED INTEGRITY; do
  [ -z "$VERSION" ] && continue
  ENTRY_COUNT=$((ENTRY_COUNT + 1))
  atlas_info "[bun:$SOURCE] version='$VERSION' resolved='$RESOLVED' integrity='${INTEGRITY:0:24}…'"

  if [ -z "$INTEGRITY" ]; then
    atlas_fail "[bun:$SOURCE] missing 'integrity' field"
    ANY_FAIL=1
    continue
  fi

  case "$INTEGRITY" in
    sha512-*|sha384-*|sha256-*)
      : # OK
      ;;
    sha1-*|md5-*)
      atlas_fail "[bun:$SOURCE] integrity uses weak hash: '${INTEGRITY%%-*}'"
      ANY_FAIL=1
      continue
      ;;
    *)
      atlas_fail "[bun:$SOURCE] integrity uses unrecognised hash format: '$INTEGRITY'"
      ANY_FAIL=1
      continue
      ;;
  esac

  case "$RESOLVED" in
    https://registry.npmjs.org*|https://registry.yarnpkg.com*)
      :
      ;;
    file:*)
      ANY_LOCAL=1
      if [ "$FAIL_ON_LOCAL" = "true" ]; then
        atlas_fail "[bun:$SOURCE] resolved to local file: '$RESOLVED'"
        ANY_FAIL=1
        continue
      else
        atlas_warn "[bun:$SOURCE] resolved to local file: '$RESOLVED' (V1.15 Welle B backup-channel install)"
      fi
      ;;
    https://*)
      atlas_warn "[bun:$SOURCE] non-canonical HTTPS origin: '$RESOLVED'"
      ;;
    "")
      atlas_warn "[bun:$SOURCE] no 'resolved' field — bun default registry assumed"
      ;;
    *)
      atlas_fail "[bun:$SOURCE] non-HTTPS origin: '$RESOLVED'"
      ANY_FAIL=1
      continue
      ;;
  esac

  atlas_pass "[bun:$SOURCE] integrity-hash present (${INTEGRITY%%-*})"
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

atlas_pass "Layer 2 OK — every bun lockfile entry for '$PACKAGE' has an integrity hash"
