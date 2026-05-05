#!/usr/bin/env bash
# setup.sh — pre-flight validation for verify-wasm-pin-check.
#
# Validates inputs, verifies the package.json + lockfile exist, and
# prints a one-line summary so the GitHub Actions log is readable
# even when subsequent steps short-circuit.

set -euo pipefail

# shellcheck source=./lib/log.sh
. "$(dirname "$0")/lib/log.sh"
# shellcheck source=./lib/detect-pm.sh
. "$(dirname "$0")/lib/detect-pm.sh"

PACKAGE="${ATLAS_PIN_CHECK_PACKAGE:-@atlas-trust/verify-wasm}"
WORKDIR="${ATLAS_PIN_CHECK_WORKDIR:-.}"
EXPECTED_VERSION="${ATLAS_PIN_CHECK_EXPECTED_VERSION:-}"

atlas_section "verify-wasm-pin-check — setup"
atlas_info "package          : $PACKAGE"
atlas_info "working-directory: $WORKDIR"
if [ -n "$EXPECTED_VERSION" ]; then
  atlas_info "expected-version : $EXPECTED_VERSION"
else
  atlas_info "expected-version : (any exact-pinned version)"
fi

# Reject empty WORKDIR explicitly. `cd ""` in bash silently changes
# to $HOME, which would cause subsequent steps to operate on the
# wrong directory and emit confusing diagnostics. Empty input is
# operator error, not a runtime condition we should tolerate.
if [ -z "$WORKDIR" ]; then
  atlas_fail "working-directory input is empty — must be a non-empty path."
  exit 1
fi

# Canonicalize WORKDIR + (when running under GitHub Actions) enforce
# containment within $GITHUB_WORKSPACE. The lib mutates $WORKDIR in
# place to the resolved-and-validated form. Every layer script also
# sources this lib independently — composite-action `env:` blocks
# re-evaluate `${{ inputs.working-directory }}` for every step, so
# a $GITHUB_ENV-written canonical value would be silently overridden
# on the next step. Defence-in-depth: every script defends itself.
# shellcheck source=./lib/canonicalize-workdir.sh
. "$(dirname "$0")/lib/canonicalize-workdir.sh"

# Validate package.json exists. Without this, Layer 1 cannot run.
if [ ! -f "$WORKDIR/package.json" ]; then
  atlas_fail "no package.json in $WORKDIR — verify-wasm-pin-check requires a Node.js project."
  exit 1
fi

# Validate expected-version shape if set: bare semver, no caret/tilde
# /comparator. Catches the mistake of passing `^1.15.0` as the
# expected-version, which would defeat the exact-match check.
if [ -n "$EXPECTED_VERSION" ]; then
  case "$EXPECTED_VERSION" in
    [0-9]*.[0-9]*.[0-9]*|[0-9]*.[0-9]*.[0-9]*-*|[0-9]*.[0-9]*.[0-9]*+*)
      : # OK — semver shape (with optional pre-release / build metadata)
      ;;
    *)
      atlas_fail "expected-version must be bare semver (e.g. '1.15.0', '1.15.0-rc.1') — got: '$EXPECTED_VERSION'"
      exit 1
      ;;
  esac
fi

# Detect package manager + lockfile. Sets ATLAS_PIN_CHECK_PM_RESOLVED
# and ATLAS_PIN_CHECK_LOCKFILE — but those are subprocess locals and
# don't survive into the next composite step. The next steps re-detect
# from their own env. We do the detection here too so a missing
# lockfile fails fast at the setup step (better error message than
# a confusing Layer 2 failure).
atlas_detect_pm

atlas_info "package-manager  : $ATLAS_PIN_CHECK_PM_RESOLVED"
atlas_info "lockfile         : $ATLAS_PIN_CHECK_LOCKFILE"

atlas_pass "setup OK"
