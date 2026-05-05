#!/usr/bin/env bash
# check-lockfile-integrity-npm.sh — npm-side Layer 2.
#
# Reads `$WORKDIR/package-lock.json` and asserts that every entry
# under `node_modules/<package>` has:
#   * a `version` (matches the version we pinned in Layer 1)
#   * a `resolved` URL (registry or, with WARN, `file:`)
#   * an `integrity` hash that starts with `sha512-` or `sha256-`
#
# npm v9+ lockfile format (lockfileVersion 3) puts entries under
# the `packages` map keyed by `node_modules/...` path. Older
# lockfile formats (v1, v2) put entries under `dependencies`
# tree. We support both shapes — auditor pipelines often have older
# repos.

set -euo pipefail

# shellcheck source=./lib/log.sh
. "$(dirname "$0")/lib/log.sh"

PACKAGE="${ATLAS_PIN_CHECK_PACKAGE:-@atlas-trust/verify-wasm}"
WORKDIR="${ATLAS_PIN_CHECK_WORKDIR:-.}"
FAIL_ON_LOCAL="${ATLAS_PIN_CHECK_FAIL_ON_LOCAL:-false}"

# shellcheck source=./lib/canonicalize-workdir.sh
. "$(dirname "$0")/lib/canonicalize-workdir.sh"

LOCKFILE="$WORKDIR/package-lock.json"

if [ ! -f "$LOCKFILE" ]; then
  atlas_fail "no package-lock.json in $WORKDIR — run 'npm install' first or set package-manager: pnpm/bun"
  exit 1
fi

# Use node to parse + extract. JSON output of the form
# `<key>=<value>` per line; absence of expected key ⇒ fail.
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
    let lock;
    try {
      lock = JSON.parse(raw);
    } catch (e) {
      console.error("ERROR: " + lockPath + " is not valid JSON: " + e.message);
      process.exit(2);
    }

    const matches = [];

    // Lockfile v2 / v3: `packages` map keyed by path.
    if (lock.packages && typeof lock.packages === "object") {
      for (const [key, entry] of Object.entries(lock.packages)) {
        // Entries look like "node_modules/<pkg>" (top-level) or
        // "node_modules/<scope>/<pkg>" (scoped) or
        // "<workspace>/node_modules/<pkg>" (workspace nested).
        // We match any path ending in `/node_modules/<pkg>` or the
        // bare `node_modules/<pkg>` form.
        if (key === "node_modules/" + pkg ||
            key.endsWith("/node_modules/" + pkg)) {
          matches.push({ source: "packages:" + key, entry });
        }
      }
    }

    // Lockfile v1: `dependencies` recursive tree.
    if (lock.dependencies && typeof lock.dependencies === "object") {
      function walk(deps, prefix) {
        for (const [name, entry] of Object.entries(deps)) {
          if (name === pkg) {
            matches.push({ source: "dependencies:" + prefix + name, entry });
          }
          if (entry && entry.dependencies) {
            walk(entry.dependencies, prefix + name + "/");
          }
        }
      }
      walk(lock.dependencies, "");
    }

    if (matches.length === 0) {
      console.log("__NOT_FOUND__");
      process.exit(0);
    }

    for (const { source, entry } of matches) {
      const v = entry.version || "";
      const r = entry.resolved || "";
      const i = entry.integrity || "";
      // Output format: source<US>version<US>resolved<US>integrity
      // ASCII Unit Separator (0x1f) — non-whitespace IFS so bash
      // `read` does NOT collapse consecutive empty fields. (Tab is
      // whitespace and `read` collapses adjacent whitespace IFS chars,
      // which would shift fields if `resolved` or any middle field
      // is empty — a real risk for unusual lockfile states.)
      console.log([source, v, r, i].join("\x1f"));
    }
  '
)"

if [ "$RESULT" = "__NOT_FOUND__" ]; then
  atlas_fail "package '$PACKAGE' not found in $LOCKFILE — run 'npm install --save-exact $PACKAGE@<version>' first"
  exit 1
fi

ANY_FAIL=0
ANY_LOCAL=0
ENTRY_COUNT=0
while IFS=$'\x1f' read -r SOURCE VERSION RESOLVED INTEGRITY; do
  ENTRY_COUNT=$((ENTRY_COUNT + 1))
  atlas_info "[$SOURCE] version='$VERSION' resolved='$RESOLVED' integrity='${INTEGRITY:0:24}…'"

  if [ -z "$VERSION" ]; then
    atlas_fail "[$SOURCE] missing 'version' field"
    ANY_FAIL=1
    continue
  fi

  if [ -z "$RESOLVED" ]; then
    atlas_fail "[$SOURCE] missing 'resolved' field — refusing to trust an entry without origin tracking"
    ANY_FAIL=1
    continue
  fi

  if [ -z "$INTEGRITY" ]; then
    atlas_fail "[$SOURCE] missing 'integrity' field — registry-side replacement-attack protection is missing. Re-run 'npm install --save-exact $PACKAGE@$VERSION' against a fresh registry to re-populate the integrity hash."
    ANY_FAIL=1
    continue
  fi

  case "$INTEGRITY" in
    sha512-*|sha384-*|sha256-*)
      : # OK — strong hash
      ;;
    sha1-*|md5-*)
      atlas_fail "[$SOURCE] integrity uses weak hash: '${INTEGRITY%%-*}' — sha1/md5 collisions are practical and do not defend against registry-side replacement. Re-run 'npm install --save-exact $PACKAGE@$VERSION' to upgrade."
      ANY_FAIL=1
      continue
      ;;
    *)
      atlas_fail "[$SOURCE] integrity uses unrecognised hash format: '$INTEGRITY' (expected sha512-/sha384-/sha256-/sha1-/md5- prefix)"
      ANY_FAIL=1
      continue
      ;;
  esac

  case "$RESOLVED" in
    https://registry.npmjs.org/*|https://*/atlas-trust/*|https://registry.yarnpkg.com/*)
      atlas_pass "[$SOURCE] origin OK ($RESOLVED)"
      ;;
    file:*)
      ANY_LOCAL=1
      if [ "$FAIL_ON_LOCAL" = "true" ]; then
        atlas_fail "[$SOURCE] resolved to local file: '$RESOLVED'. With fail-on-local-file=true, this is treated as a hard fail. Re-run 'npm install --save-exact $PACKAGE@$VERSION' against the registry to re-pin."
        ANY_FAIL=1
        continue
      else
        atlas_warn "[$SOURCE] resolved to local file: '$RESOLVED'. This is the V1.15 Welle B backup-channel install path. Re-pin to registry once npmjs.org is reachable: 'npm install --save-exact $PACKAGE@$VERSION'."
      fi
      ;;
    https://*)
      # Other HTTPS registry — OK (private registry, mirror, etc.)
      # but warn so consumers running through a corporate mirror
      # are aware the integrity hash is mirror-served, not
      # registry-served.
      atlas_warn "[$SOURCE] resolved to non-canonical HTTPS origin: '$RESOLVED'. This is OK if you trust the mirror, but the integrity hash is mirror-served — consider cross-verifying against npmjs.org via 'npm view $PACKAGE@$VERSION dist'."
      ;;
    *)
      atlas_fail "[$SOURCE] resolved to non-HTTPS origin: '$RESOLVED' — refusing (HTTP, git+, github:, etc. are all defeats of the integrity model)"
      ANY_FAIL=1
      continue
      ;;
  esac

  atlas_pass "[$SOURCE] integrity-hash present (${INTEGRITY%%-*})"
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
  atlas_warn "Layer 2 PASS with backup-channel WARN — re-pin to registry source when npm reachable"
fi

atlas_pass "Layer 2 OK — every lockfile entry for '$PACKAGE' has an integrity hash"
