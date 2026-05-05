# Shared logging helpers for verify-wasm-pin-check scripts.
#
# Source via: . "$(dirname "$0")/lib/log.sh"
#
# Conventions:
#   atlas_info  — neutral status line. exit 0 on its own.
#   atlas_pass  — green PASS line for a layer / sub-check.
#   atlas_warn  — yellow WARN line. NOT fatal.
#   atlas_fail  — red FAIL line. Caller MUST `exit <non-zero>` after.
#
# Colour discipline: tput is the portable way to get ANSI escapes
# without hardcoding `\033[…]` sequences that break under non-ANSI
# terminals. If tput is missing or terminal can't render colour
# (TERM=dumb, no tty), all helpers degrade to plain text. GitHub
# Actions sets TERM=dumb sometimes, so plain text is the safe
# fallback — the leading `[PASS] / [FAIL]` token is what makes log
# scanning work either way.

set -euo pipefail

if [ -z "${_ATLAS_PIN_CHECK_LOG_LOADED:-}" ]; then
  _ATLAS_PIN_CHECK_LOG_LOADED=1

  if [ -t 1 ] && command -v tput >/dev/null 2>&1 && tput colors >/dev/null 2>&1; then
    _ATLAS_C_RED="$(tput setaf 1 2>/dev/null || true)"
    _ATLAS_C_GREEN="$(tput setaf 2 2>/dev/null || true)"
    _ATLAS_C_YELLOW="$(tput setaf 3 2>/dev/null || true)"
    _ATLAS_C_BLUE="$(tput setaf 4 2>/dev/null || true)"
    _ATLAS_C_BOLD="$(tput bold 2>/dev/null || true)"
    _ATLAS_C_RESET="$(tput sgr0 2>/dev/null || true)"
  else
    _ATLAS_C_RED=""
    _ATLAS_C_GREEN=""
    _ATLAS_C_YELLOW=""
    _ATLAS_C_BLUE=""
    _ATLAS_C_BOLD=""
    _ATLAS_C_RESET=""
  fi

  atlas_info() {
    printf '%s[INFO]%s %s\n' "${_ATLAS_C_BLUE}" "${_ATLAS_C_RESET}" "$*"
  }

  atlas_pass() {
    printf '%s[PASS]%s %s\n' "${_ATLAS_C_GREEN}" "${_ATLAS_C_RESET}" "$*"
  }

  atlas_warn() {
    printf '%s[WARN]%s %s\n' "${_ATLAS_C_YELLOW}" "${_ATLAS_C_RESET}" "$*" >&2
  }

  atlas_fail() {
    printf '%s[FAIL]%s %s\n' "${_ATLAS_C_RED}" "${_ATLAS_C_RESET}" "$*" >&2
  }

  atlas_section() {
    printf '\n%s%s== %s ==%s\n' "${_ATLAS_C_BOLD}" "${_ATLAS_C_BLUE}" "$*" "${_ATLAS_C_RESET}"
  }
fi
