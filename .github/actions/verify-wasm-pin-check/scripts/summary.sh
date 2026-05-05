#!/usr/bin/env bash
# summary.sh — final-step summary for the GitHub Actions log.
#
# This step always runs (no `if:`); it prints a one-section banner
# so a consumer scanning the action log sees a clear "all OK"
# message at the bottom even if the upstream steps were green-but-
# verbose. If any prior layer failed, the workflow already aborted
# before reaching this step — its presence in the log means
# everything passed.

set -euo pipefail

# shellcheck source=./lib/log.sh
. "$(dirname "$0")/lib/log.sh"

PACKAGE="${ATLAS_PIN_CHECK_PACKAGE:-@atlas-trust/verify-wasm}"
SKIPPED_PROVENANCE="${ATLAS_PIN_CHECK_SKIPPED_PROVENANCE:-false}"

atlas_section "verify-wasm-pin-check — summary"
atlas_pass "Layer 1 (version pin) — passed"
atlas_pass "Layer 2 (lockfile integrity) — passed"
if [ "$SKIPPED_PROVENANCE" = "true" ]; then
  atlas_warn "Layer 3 (SLSA L3 provenance) — SKIPPED via skip-provenance=true. Re-enable in production CI."
else
  atlas_pass "Layer 3 (SLSA L3 provenance) — passed"
fi
atlas_info "Verifier package '$PACKAGE' is exact-pinned, lockfile-locked, and (unless skipped) SLSA L3 provenance-attested."
atlas_info "See https://github.com/ThePyth0nKid/atlas/blob/master/docs/CONSUMER-RUNBOOK.md for the full trust model."
