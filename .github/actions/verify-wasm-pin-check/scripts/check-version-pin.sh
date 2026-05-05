#!/usr/bin/env bash
# check-version-pin.sh — Layer 1 of the CONSUMER-RUNBOOK §1 stack.
#
# Asserts that `package.json` declares the package with an EXACT
# version pin (no `^`, `~`, `>=`, `<`, `>`, `||`, `*`, `latest`,
# `next`, `x` ranges). If `expected-version` is set, also asserts
# that the pinned version matches exactly.
#
# Why this matters:
#   * `^1.15.0` allows silent minor + patch upgrades on every fresh
#     install. A maintainer-token-compromise that publishes a
#     malicious `1.15.99` lands automatically on the next CI install.
#   * `~1.15.0` is similarly permissive within a patch range.
#   * `latest` / `next` / `*` defeat the entire pinning model.
#
# Read-only. Reads `$WORKDIR/package.json` and parses the entry for
# `$PACKAGE` from `dependencies`, `devDependencies`, `peerDependencies`,
# and `optionalDependencies`. Uses `node -e` for JSON parsing —
# we're inside a GitHub Actions runner that has Node available
# (every actions/setup-node + every default ubuntu-latest image
# ships node out of the box).
#
# Why node and not jq:
#   * jq is not in every consumer's runner image (especially
#     Windows runners). Node is universal in this context.
#   * Adding `apt-get install jq` adds an action step the consumer
#     might already have done, or might not have permission to do
#     in a hardened runner image.

set -euo pipefail

# shellcheck source=./lib/log.sh
. "$(dirname "$0")/lib/log.sh"

PACKAGE="${ATLAS_PIN_CHECK_PACKAGE:-@atlas-trust/verify-wasm}"
WORKDIR="${ATLAS_PIN_CHECK_WORKDIR:-.}"
EXPECTED_VERSION="${ATLAS_PIN_CHECK_EXPECTED_VERSION:-}"

atlas_section "Layer 1 — Version pin in package.json"

# shellcheck source=./lib/canonicalize-workdir.sh
. "$(dirname "$0")/lib/canonicalize-workdir.sh"

if [ ! -f "$WORKDIR/package.json" ]; then
  atlas_fail "no package.json in $WORKDIR"
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  atlas_fail "node not found on PATH — verify-wasm-pin-check needs Node.js to parse package.json. Add an actions/setup-node step before this action."
  exit 1
fi

# Extract the declared version (or absence) from all four dep
# buckets. Output one line of `<bucket>:<version>` per match;
# empty output ⇒ not declared anywhere.
#
# Pass values via env vars instead of command-line args to dodge
# any shell-escaping concerns (the package name contains `@` and
# `/` but not whitespace, but env-var passing is the disciplined
# default for any user-supplied string flowing into a node script).
DECLARED="$(
  ATLAS_NODE_PKG="$PACKAGE" \
  ATLAS_NODE_PKGJSON="$WORKDIR/package.json" \
  node -e '
    const fs = require("fs");
    const pkgJsonPath = process.env.ATLAS_NODE_PKGJSON;
    const pkg = process.env.ATLAS_NODE_PKG;
    let raw;
    try {
      raw = fs.readFileSync(pkgJsonPath, "utf8");
    } catch (e) {
      console.error("ERROR: cannot read " + pkgJsonPath + ": " + e.message);
      process.exit(2);
    }
    let parsed;
    try {
      parsed = JSON.parse(raw);
    } catch (e) {
      console.error("ERROR: " + pkgJsonPath + " is not valid JSON: " + e.message);
      process.exit(2);
    }
    const buckets = ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"];
    for (const b of buckets) {
      const m = parsed[b];
      if (m && Object.prototype.hasOwnProperty.call(m, pkg)) {
        const v = m[pkg];
        if (typeof v !== "string") {
          console.error("ERROR: " + b + "[" + pkg + "] is not a string — got " + typeof v);
          process.exit(2);
        }
        // Print one line per bucket the package appears in.
        // Multi-bucket declaration is unusual but legal; the check
        // applies to every occurrence. ASCII Unit Separator (0x1f)
        // instead of `:` so a version string containing `:` (e.g.
        // an `npm:other-pkg@1.0.0` alias — rejected by the range
        // check below, but only after this line splits cleanly)
        // does not corrupt the BUCKET / VERSION column boundary.
        // Same rationale as the lockfile-side scripts.
        console.log(b + "\x1f" + v);
      }
    }
  '
)"

if [ -z "$DECLARED" ]; then
  atlas_fail "package '$PACKAGE' not declared in $WORKDIR/package.json (checked dependencies, devDependencies, peerDependencies, optionalDependencies)"
  exit 1
fi

# Iterate every matched line. Each line is `<bucket>\x1f<version>`.
# `\x1f` (ASCII Unit Separator) instead of `:` so a version like
# `npm:other-pkg@1.0.0` (an alias spec — rejected by the range
# check below, but the split must run cleanly first) does not
# corrupt the field boundary by yielding extra colon-split fields.
# All matched lines must pass — a single bad pin in any bucket fails.
ANY_FAIL=0
while IFS=$'\x1f' read -r BUCKET VERSION; do
  atlas_info "$BUCKET[$PACKAGE] = '$VERSION'"

  case "$VERSION" in
    \^*|\~*|\>*|\<*|\=*|\**|*\|\|*|x|X|*\.x|*\.X|latest|next|*workspace*|*"file:"*|*"link:"*|*"git+"*|*"github:"*|*"http:"*|*"https:"*)
      atlas_fail "$BUCKET[$PACKAGE] = '$VERSION' is not an exact-version pin. Use 'npm install --save-exact $PACKAGE@<version>' (or 'pnpm add --save-exact …' / 'bun add …') to write a bare-semver value (e.g. '1.15.0' — no '^', '~', '>=', '||', 'x', 'latest', 'next', 'workspace:', 'file:', 'link:', 'git+…', 'github:', 'http:', or 'https:')."
      ANY_FAIL=1
      continue
      ;;
    "")
      atlas_fail "$BUCKET[$PACKAGE] is empty"
      ANY_FAIL=1
      continue
      ;;
  esac

  # Bare-semver shape check: digits.digits.digits, optionally
  # followed by a pre-release (`-rc.1`) or build-metadata (`+sha.abc`).
  case "$VERSION" in
    [0-9]*.[0-9]*.[0-9]*|[0-9]*.[0-9]*.[0-9]*-*|[0-9]*.[0-9]*.[0-9]*+*)
      : # OK — semver shape
      ;;
    *)
      atlas_fail "$BUCKET[$PACKAGE] = '$VERSION' does not look like a bare semver (e.g. '1.15.0' or '1.15.0-rc.1')"
      ANY_FAIL=1
      continue
      ;;
  esac

  # Optional exact-match against expected-version.
  if [ -n "$EXPECTED_VERSION" ] && [ "$VERSION" != "$EXPECTED_VERSION" ]; then
    atlas_fail "$BUCKET[$PACKAGE] = '$VERSION' does not match expected-version='$EXPECTED_VERSION'"
    ANY_FAIL=1
    continue
  fi

  atlas_pass "$BUCKET[$PACKAGE] = '$VERSION' is exact-pinned"
done <<EOF
$DECLARED
EOF

if [ "$ANY_FAIL" -ne 0 ]; then
  exit 1
fi

atlas_pass "Layer 1 OK — every declaration of '$PACKAGE' is exact-pinned"
