#!/usr/bin/env bash
#
# tools/install-git-hooks.sh
#
# Activates the repo-tracked git hooks under tools/git-hooks/ by pointing
# git's `core.hooksPath` at that directory. This is the modern (git 2.9+)
# alternative to symlinking individual hooks into .git/hooks/, which has
# the advantage of being repo-tracked, reviewable, and auditable.
#
# Usage:
#   bash tools/install-git-hooks.sh
#
# Idempotent — re-running is safe.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_DIR="${REPO_ROOT}/tools/git-hooks"

if [ ! -d "$HOOKS_DIR" ]; then
  echo "FAIL: hooks directory not found: $HOOKS_DIR" >&2
  exit 1
fi

# git stores core.hooksPath as a path string. Use a relative path (relative
# to repo root) so the config stays portable across clones — an absolute
# path would break the moment the repo is cloned to a different machine.
RELATIVE_HOOKS_PATH="tools/git-hooks"

# Make every hook script executable. On Windows / NTFS, chmod is a no-op
# but harmless — git on Windows interprets executability via the
# core.fileMode bit, which is auto-detected.
for hook in "$HOOKS_DIR"/*; do
  if [ -f "$hook" ]; then
    chmod +x "$hook" 2>/dev/null || true
  fi
done

# Set core.hooksPath. Using `git config --local` so this only affects the
# current clone — operators retain control over their own git config.
git config --local core.hooksPath "$RELATIVE_HOOKS_PATH"

echo "installed: core.hooksPath = $RELATIVE_HOOKS_PATH"
echo "active hooks:"
for hook in "$HOOKS_DIR"/*; do
  if [ -f "$hook" ] && [ -x "$hook" ]; then
    echo "  - $(basename "$hook")"
  fi
done
