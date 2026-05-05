#!/usr/bin/env bash
# check-lockfile-integrity-pnpm.sh — pnpm-side Layer 2.
#
# Reads `$WORKDIR/pnpm-lock.yaml` and asserts that every entry under
# `packages.<pkg>@<version>:` has a `resolution.integrity` field that
# starts with `sha512-` (or `sha256-` for some pnpm versions).
#
# pnpm-lock.yaml is YAML, but the schema is regular enough that we
# can grep + awk it without a YAML parser dependency. We extract:
#
#   '@atlas-trust/verify-wasm@1.15.0':
#     resolution:
#       integrity: sha512-...
#       tarball: https://registry.npmjs.org/...
#
# Block boundary is the next top-level package entry (line not
# indented further than the opening line) or EOF. The tarball line
# may also be `registry: https://...` in some pnpm versions.
#
# We use a small awk program to handle the block-scope properly —
# a naive grep would conflate adjacent package entries.

set -euo pipefail

# shellcheck source=./lib/log.sh
. "$(dirname "$0")/lib/log.sh"

PACKAGE="${ATLAS_PIN_CHECK_PACKAGE:-@atlas-trust/verify-wasm}"
WORKDIR="${ATLAS_PIN_CHECK_WORKDIR:-.}"
FAIL_ON_LOCAL="${ATLAS_PIN_CHECK_FAIL_ON_LOCAL:-false}"

# shellcheck source=./lib/canonicalize-workdir.sh
. "$(dirname "$0")/lib/canonicalize-workdir.sh"

LOCKFILE="$WORKDIR/pnpm-lock.yaml"

if [ ! -f "$LOCKFILE" ]; then
  atlas_fail "no pnpm-lock.yaml in $WORKDIR — run 'pnpm install' first or set package-manager: npm/bun"
  exit 1
fi

# Find every block matching `'<PACKAGE>@<ver>':` or `<PACKAGE>@<ver>:`
# (pnpm v6 used quoted, v7+ unquoted). Output: one line per match in
# format `<version>\t<integrity>\t<tarball>`.
#
# AWK script: state machine with three modes.
#   0 = outside any package block
#   1 = inside a matching block, looking for resolution
#   2 = inside resolution sub-block, capturing fields
RESULT="$(
  ATLAS_PKG="$PACKAGE" awk '
    BEGIN {
      pkg = ENVIRON["ATLAS_PKG"]
      mode = 0
      version = ""
      integrity = ""
      tarball = ""
    }
    function flush() {
      if (version != "") {
        # ASCII Unit Separator (octal \037 = 0x1f) instead of tab so
        # bash `read` does not collapse consecutive empty fields (tab
        # is whitespace IFS; \037 is not). Octal escape (not \x1f)
        # for portability across awk variants — gawk supports \x but
        # mawk and busybox awk only handle octal.
        print version "\037" integrity "\037" tarball
      }
      version = ""
      integrity = ""
      tarball = ""
    }
    # Match `'pkg@1.2.3':` or `pkg@1.2.3:` at any indent
    # (top-level packages are at zero indent in pnpm v7+, but
    # snapshot section can have nested ones; we accept both).
    {
      line = $0
      # Strip leading whitespace for matching only.
      stripped = line
      sub(/^[[:space:]]+/, "", stripped)
      sub(/[[:space:]]+$/, "", stripped)

      # Detect a package-entry header. Use index() (literal substring)
      # instead of ~ (regex match) so a `package-name` input that
      # contains AWK regex metacharacters (`.`, `+`, `*`, `[`, `(`,
      # `{`, `^`, `$`, `?`, `|`, `\`) is not mis-interpreted as a
      # pattern. Same defence applies to the prefix-strip below.
      prefix_bare   = pkg "@"
      prefix_quoted = "'\''" pkg "@"
      if (index(stripped, prefix_quoted) == 1 || index(stripped, prefix_bare) == 1) {
        # Flush previous block if any.
        flush()
        # Extract the version from `pkg@VERSION:` shape.
        s = stripped
        # Strip leading quote (if any) without using regex sub() so
        # we never run pkg through a regex engine.
        if (substr(s, 1, 1) == "'\''") s = substr(s, 2)
        # Strip trailing `:` and any trailing whitespace + closing
        # quote. Trailing-whitespace strip already happened above.
        n = length(s)
        if (n >= 1 && substr(s, n, 1) == ":") { s = substr(s, 1, n - 1); n = length(s) }
        if (n >= 1 && substr(s, n, 1) == "'\''") { s = substr(s, 1, n - 1); n = length(s) }
        # Strip the package prefix (literal, no regex) to leave just
        # version (or version+pnpm-disambiguator like
        # `1.15.0(react@18)`).
        plen = length(prefix_bare)
        if (index(s, prefix_bare) == 1) s = substr(s, plen + 1)
        version = s
        mode = 1
        next
      }
      # If we are inside a matching block, look for resolution / integrity / tarball.
      if (mode >= 1) {
        # Detect end-of-block: any line that starts at zero indent
        # AND is non-empty AND is not a continuation of our entry.
        # Heuristic: if the line is non-empty and not indented
        # (no leading space) AND not the entry header itself, the
        # block ended.
        if (line ~ /^[^[:space:]]/ && line != "") {
          flush()
          mode = 0
        } else {
          # Inside our block. Look for fields.
          if (stripped ~ /^integrity:[[:space:]]/) {
            integrity = stripped
            sub(/^integrity:[[:space:]]+/, "", integrity)
          } else if (stripped ~ /^tarball:[[:space:]]/) {
            tarball = stripped
            sub(/^tarball:[[:space:]]+/, "", tarball)
          } else if (stripped ~ /^registry:[[:space:]]/) {
            # pnpm sometimes records registry instead of tarball;
            # still useful for the origin check.
            if (tarball == "") {
              tarball = stripped
              sub(/^registry:[[:space:]]+/, "", tarball)
            }
          }
        }
      }
    }
    END {
      flush()
    }
  ' "$LOCKFILE"
)"

if [ -z "$RESULT" ]; then
  atlas_fail "package '$PACKAGE' not found in $LOCKFILE — run 'pnpm add --save-exact $PACKAGE@<version>' first"
  exit 1
fi

ANY_FAIL=0
ANY_LOCAL=0
ENTRY_COUNT=0
while IFS=$'\x1f' read -r VERSION INTEGRITY TARBALL; do
  [ -z "$VERSION" ] && continue
  ENTRY_COUNT=$((ENTRY_COUNT + 1))
  atlas_info "[pnpm:$PACKAGE@$VERSION] integrity='${INTEGRITY:0:24}…' origin='$TARBALL'"

  if [ -z "$INTEGRITY" ]; then
    atlas_fail "[pnpm:$PACKAGE@$VERSION] missing 'integrity' field"
    ANY_FAIL=1
    continue
  fi

  case "$INTEGRITY" in
    sha512-*|sha384-*|sha256-*)
      : # OK
      ;;
    sha1-*|md5-*)
      atlas_fail "[pnpm:$PACKAGE@$VERSION] integrity uses weak hash: '${INTEGRITY%%-*}'"
      ANY_FAIL=1
      continue
      ;;
    *)
      atlas_fail "[pnpm:$PACKAGE@$VERSION] integrity uses unrecognised hash format: '$INTEGRITY'"
      ANY_FAIL=1
      continue
      ;;
  esac

  if [ -z "$TARBALL" ]; then
    atlas_warn "[pnpm:$PACKAGE@$VERSION] no 'tarball' or 'registry' field — pnpm default registry is assumed"
  else
    case "$TARBALL" in
      https://registry.npmjs.org*|https://registry.yarnpkg.com*)
        :
        ;;
      file:*)
        ANY_LOCAL=1
        if [ "$FAIL_ON_LOCAL" = "true" ]; then
          atlas_fail "[pnpm:$PACKAGE@$VERSION] resolved to local file: '$TARBALL'. With fail-on-local-file=true, this is a hard fail."
          ANY_FAIL=1
          continue
        else
          atlas_warn "[pnpm:$PACKAGE@$VERSION] resolved to local file: '$TARBALL' (V1.15 Welle B backup-channel install). Re-pin via 'pnpm add --save-exact $PACKAGE@$VERSION' once registry reachable."
        fi
        ;;
      https://*)
        atlas_warn "[pnpm:$PACKAGE@$VERSION] non-canonical HTTPS origin: '$TARBALL'"
        ;;
      *)
        atlas_fail "[pnpm:$PACKAGE@$VERSION] non-HTTPS origin: '$TARBALL'"
        ANY_FAIL=1
        continue
        ;;
    esac
  fi

  atlas_pass "[pnpm:$PACKAGE@$VERSION] integrity-hash present (${INTEGRITY%%-*})"
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

atlas_pass "Layer 2 OK — every pnpm-lock.yaml entry for '$PACKAGE' has an integrity hash"
