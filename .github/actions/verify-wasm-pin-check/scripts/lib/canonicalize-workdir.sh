#!/usr/bin/env bash
# canonicalize-workdir.sh — shared lib sourced by every layer script.
#
# Reads `$WORKDIR` (set by the calling script from
# `$ATLAS_PIN_CHECK_WORKDIR`), resolves symlinks, and (when running
# under GitHub Actions) asserts the resolved path is inside
# `$GITHUB_WORKSPACE`. On success, mutates `$WORKDIR` in place to
# the canonical form. On failure, prints a diagnostic and exits.
#
# Why every layer script calls this independently (not just setup.sh):
#   composite-action `env:` blocks re-evaluate
#   `${{ inputs.working-directory }}` for EVERY step. A canonical
#   value written to $GITHUB_ENV by setup.sh is silently overridden
#   by the raw input on the next step. So defence has to be per-step
#   or moved into the action.yml plumbing — per-step is simpler and
#   keeps the YAML readable.
#
# This is defence-in-depth against a fork-PR-poisoning scenario:
#   a malicious PR commits `packages/foo` as a symlink to
#   `../../../../etc` and sets `working-directory: packages/foo`.
#   Without canonicalization, our scripts would `cd` into a path
#   outside the checkout root; with this guard, every step refuses.

# shellcheck disable=SC2154 # WORKDIR is set by the calling script

if [ -z "${WORKDIR:-}" ]; then
  atlas_fail "internal: canonicalize-workdir.sh sourced with empty \$WORKDIR"
  exit 1
fi

if command -v realpath >/dev/null 2>&1; then
  ATLAS_RESOLVED_WORKDIR="$(realpath "$WORKDIR" 2>/dev/null || true)"
elif command -v readlink >/dev/null 2>&1; then
  ATLAS_RESOLVED_WORKDIR="$(readlink -f "$WORKDIR" 2>/dev/null || true)"
else
  ATLAS_RESOLVED_WORKDIR="$(cd "$WORKDIR" 2>/dev/null && pwd)" || ATLAS_RESOLVED_WORKDIR=""
fi

if [ -z "$ATLAS_RESOLVED_WORKDIR" ] || [ ! -d "$ATLAS_RESOLVED_WORKDIR" ]; then
  atlas_fail "working-directory '$WORKDIR' does not resolve to an existing directory."
  exit 1
fi

if [ -n "${GITHUB_WORKSPACE:-}" ]; then
  if command -v realpath >/dev/null 2>&1; then
    ATLAS_RESOLVED_WORKSPACE="$(realpath "$GITHUB_WORKSPACE" 2>/dev/null || echo "$GITHUB_WORKSPACE")"
  else
    ATLAS_RESOLVED_WORKSPACE="$GITHUB_WORKSPACE"
  fi
  case "$ATLAS_RESOLVED_WORKDIR" in
    "$ATLAS_RESOLVED_WORKSPACE"|"$ATLAS_RESOLVED_WORKSPACE"/*)
      : # OK — inside the runner's checkout root
      ;;
    *)
      atlas_fail "working-directory '$WORKDIR' resolves to '$ATLAS_RESOLVED_WORKDIR' which is OUTSIDE \$GITHUB_WORKSPACE ('$ATLAS_RESOLVED_WORKSPACE'). Refusing to run — most often a symlink-based path-traversal attempt."
      exit 1
      ;;
  esac
  unset ATLAS_RESOLVED_WORKSPACE
fi

WORKDIR="$ATLAS_RESOLVED_WORKDIR"
unset ATLAS_RESOLVED_WORKDIR
