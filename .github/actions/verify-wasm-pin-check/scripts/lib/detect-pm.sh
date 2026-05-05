# Package-manager detection helper.
#
# Source via: . "$(dirname "$0")/lib/detect-pm.sh"
#
# Reads ATLAS_PIN_CHECK_PM (auto | npm | pnpm | bun) and
# ATLAS_PIN_CHECK_WORKDIR, sets:
#
#   ATLAS_PIN_CHECK_PM_RESOLVED  — concrete: npm | pnpm | bun
#   ATLAS_PIN_CHECK_LOCKFILE     — full path to detected lockfile
#
# Auto-detect order: npm > pnpm > bun. Rationale: npm is the most
# common case in the consumer base; if both npm and pnpm lockfiles
# exist (a monorepo migration mid-flight), npm wins by default and
# the consumer can override via the explicit `package-manager` input.
# Conflict gets a WARN so the consumer notices.
#
# Bun's lockfile is binary (`bun.lockb`) — we can't grep its content
# from bash, so the bun path delegates to `bun pm ls --json` for the
# integrity-hash check (separate concern, lives in
# check-lockfile-integrity.sh).

# shellcheck source=./log.sh
. "$(dirname "${BASH_SOURCE[0]}")/log.sh"

atlas_detect_pm() {
  local workdir="${ATLAS_PIN_CHECK_WORKDIR:-.}"
  local explicit="${ATLAS_PIN_CHECK_PM:-auto}"

  if [ ! -d "$workdir" ]; then
    atlas_fail "working-directory does not exist: $workdir"
    return 1
  fi

  local has_npm=0 has_pnpm=0 has_bun=0
  [ -f "$workdir/package-lock.json" ] && has_npm=1
  [ -f "$workdir/pnpm-lock.yaml" ]    && has_pnpm=1
  [ -f "$workdir/bun.lockb" ]         && has_bun=1
  # Bun also writes bun.lock (text) in newer versions — accept either.
  [ -f "$workdir/bun.lock" ]          && has_bun=1

  case "$explicit" in
    npm)
      if [ "$has_npm" -ne 1 ]; then
        atlas_fail "package-manager=npm but no package-lock.json in $workdir"
        return 1
      fi
      ATLAS_PIN_CHECK_PM_RESOLVED="npm"
      ATLAS_PIN_CHECK_LOCKFILE="$workdir/package-lock.json"
      ;;
    pnpm)
      if [ "$has_pnpm" -ne 1 ]; then
        atlas_fail "package-manager=pnpm but no pnpm-lock.yaml in $workdir"
        return 1
      fi
      ATLAS_PIN_CHECK_PM_RESOLVED="pnpm"
      ATLAS_PIN_CHECK_LOCKFILE="$workdir/pnpm-lock.yaml"
      ;;
    bun)
      if [ "$has_bun" -ne 1 ]; then
        atlas_fail "package-manager=bun but no bun.lockb / bun.lock in $workdir"
        return 1
      fi
      ATLAS_PIN_CHECK_PM_RESOLVED="bun"
      if [ -f "$workdir/bun.lock" ]; then
        ATLAS_PIN_CHECK_LOCKFILE="$workdir/bun.lock"
      else
        ATLAS_PIN_CHECK_LOCKFILE="$workdir/bun.lockb"
      fi
      ;;
    auto)
      local total=$((has_npm + has_pnpm + has_bun))
      if [ "$total" -eq 0 ]; then
        atlas_fail "no lockfile found in $workdir (looked for package-lock.json, pnpm-lock.yaml, bun.lockb, bun.lock)"
        return 1
      fi
      if [ "$total" -gt 1 ]; then
        atlas_warn "multiple lockfiles detected in $workdir — set 'package-manager:' explicitly to disambiguate. Defaulting to npm > pnpm > bun precedence."
      fi
      if [ "$has_npm" -eq 1 ]; then
        ATLAS_PIN_CHECK_PM_RESOLVED="npm"
        ATLAS_PIN_CHECK_LOCKFILE="$workdir/package-lock.json"
      elif [ "$has_pnpm" -eq 1 ]; then
        ATLAS_PIN_CHECK_PM_RESOLVED="pnpm"
        ATLAS_PIN_CHECK_LOCKFILE="$workdir/pnpm-lock.yaml"
      else
        if [ -f "$workdir/bun.lock" ]; then
          ATLAS_PIN_CHECK_LOCKFILE="$workdir/bun.lock"
        else
          ATLAS_PIN_CHECK_LOCKFILE="$workdir/bun.lockb"
        fi
        ATLAS_PIN_CHECK_PM_RESOLVED="bun"
      fi
      ;;
    *)
      atlas_fail "unknown package-manager: '$explicit' (expected: auto | npm | pnpm | bun)"
      return 1
      ;;
  esac

  export ATLAS_PIN_CHECK_PM_RESOLVED ATLAS_PIN_CHECK_LOCKFILE
}
