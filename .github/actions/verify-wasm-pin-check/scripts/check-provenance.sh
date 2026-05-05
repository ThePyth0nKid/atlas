#!/usr/bin/env bash
# check-provenance.sh — Layer 3 of the CONSUMER-RUNBOOK §1 stack.
#
# Runs `npm audit signatures` and asserts that:
#   1. The command itself succeeds (exit 0).
#   2. The output contains a "verified attestation" line referencing
#      our package — i.e. the SLSA L3 OIDC-signed provenance is
#      present and valid.
#
# The exit-0-without-our-package case is also a fail: it would mean
# `npm audit signatures` ran but our package wasn't in the audited
# tree (probably because it wasn't installed — Layer 2 would have
# caught that, but defence-in-depth is cheap).
#
# Retry on transient outages: per CONSUMER-RUNBOOK §5, transient
# Sigstore Rekor / npm attestation endpoint outages are the most
# common false-positive cause. We retry up to N times with
# exponential backoff (10s, 30s, 90s) before giving up.
#
# Network surface: `npm audit signatures` talks to `registry.npmjs.org`
# AND `rekor.sigstore.dev`. If the runner is in an air-gapped /
# proxy-restricted network and either is blocked, this step will
# fail with a network error from npm — the consumer must either
# allow-list those endpoints or set `skip-provenance: true`
# (and accept the SLSA L3 layer is not enforced).

set -euo pipefail

# shellcheck source=./lib/log.sh
. "$(dirname "$0")/lib/log.sh"

PACKAGE="${ATLAS_PIN_CHECK_PACKAGE:-@atlas-trust/verify-wasm}"
WORKDIR="${ATLAS_PIN_CHECK_WORKDIR:-.}"
RETRIES="${ATLAS_PIN_CHECK_RETRIES:-3}"

atlas_section "Layer 3 — SLSA L3 provenance (npm audit signatures)"

# shellcheck source=./lib/canonicalize-workdir.sh
. "$(dirname "$0")/lib/canonicalize-workdir.sh"

# Bound retries to a sane range. 0 = no retry (single attempt). 10 is
# a generous upper bound; beyond that the consumer should investigate
# whether Sigstore is actually reachable rather than spinning longer.
case "$RETRIES" in
  ''|*[!0-9]*)
    atlas_fail "provenance-retries must be a non-negative integer — got '$RETRIES'"
    exit 1
    ;;
esac
if [ "$RETRIES" -gt 10 ]; then
  atlas_warn "provenance-retries=$RETRIES is unusually high — clamping to 10. If Sigstore is genuinely down for >10 attempts, accept the failure and re-run later."
  RETRIES=10
fi

if ! command -v npm >/dev/null 2>&1; then
  atlas_fail "npm not found on PATH — needed for 'npm audit signatures'. Add an actions/setup-node step before this action, or set skip-provenance: true."
  exit 1
fi

# Pre-condition: our package must actually be in the resolved
# install tree. Without this, a tree where our package is NOT
# installed but other packages ARE signed would silently pass
# Layer 3 — `npm audit signatures` reports counts, not per-package
# verdicts in the success case, so the positive grep below would
# match an OTHER package's attestation line and pass. `npm ls`
# returns 0 only when the package is present in the resolved
# tree. Pre-conditioning here closes the false-pass gap.
#
# `npm ls` exits 1 with `MISSING` for unmet/missing deps even when
# the package IS declared in package.json, so we treat any non-zero
# exit as "package not present at install time" and bail.
NPM_LS_OUTPUT="$(cd "$WORKDIR" && npm ls "$PACKAGE" --depth=0 2>&1 || true)"
NPM_LS_RC=0
( cd "$WORKDIR" && npm ls "$PACKAGE" --depth=0 >/dev/null 2>&1 ) || NPM_LS_RC=$?
if [ "$NPM_LS_RC" -ne 0 ]; then
  atlas_fail "'npm ls $PACKAGE' reports the package is NOT in the resolved install tree at $WORKDIR. Run 'npm ci' (or 'npm install') BEFORE this action so Layer 3 has something to attest. Without this guard, a tree where another package is signed would pass Layer 3 even if '$PACKAGE' itself were absent. Output:"
  printf '%s\n' "$NPM_LS_OUTPUT" >&2
  exit 1
fi

# Check npm version is >= 9.5 (the version that introduced
# attestation API support). Older npm versions silently lack
# the `signatures` subcommand and the failure mode is unhelpful.
NPM_VERSION="$(npm --version 2>/dev/null || echo '0.0.0')"
NPM_MAJOR="${NPM_VERSION%%.*}"
NPM_REST="${NPM_VERSION#*.}"
NPM_MINOR="${NPM_REST%%.*}"
if [ "$NPM_MAJOR" -lt 9 ] || { [ "$NPM_MAJOR" -eq 9 ] && [ "$NPM_MINOR" -lt 5 ]; }; then
  atlas_fail "npm $NPM_VERSION lacks attestation API ('npm audit signatures'). Need npm >= 9.5. Either upgrade npm (latest actions/setup-node ships >= 10) or set skip-provenance: true to skip this layer."
  exit 1
fi
atlas_info "npm version: $NPM_VERSION"

cd "$WORKDIR"

# Backoff sequence: 10s, 30s, 90s, then 90s for any further attempts.
# Total wait on RETRIES=3: ~130s (~2 min). Acceptable for a CI build
# step where the cost of false-positive is also a re-run.
backoff_for_attempt() {
  local n="$1"
  case "$n" in
    1) echo 10 ;;
    2) echo 30 ;;
    *) echo 90 ;;
  esac
}

ATTEMPT=0
MAX=$((RETRIES + 1))
NPM_OUTPUT=""
NPM_EXIT=0
while [ "$ATTEMPT" -lt "$MAX" ]; do
  ATTEMPT=$((ATTEMPT + 1))
  atlas_info "Attempt $ATTEMPT/$MAX — running 'npm audit signatures'…"
  # Capture both stdout + stderr; npm prints attestation results to
  # stdout but errors (network, etc.) to stderr.
  set +e
  NPM_OUTPUT="$(npm audit signatures 2>&1)"
  NPM_EXIT=$?
  set -e

  if [ "$NPM_EXIT" -eq 0 ]; then
    break
  fi

  # Classify the failure. Some failures are NEVER transient; retrying
  # is just burning runner time. Others are typically transient.
  CLASS="unknown"
  case "$NPM_OUTPUT" in
    *"missing or invalid attestation"*)  CLASS="attestation-failure" ;;
    *"missing or invalid signature"*)    CLASS="signature-failure" ;;
    *"ENOTFOUND"*|*"ECONNREFUSED"*|*"ETIMEDOUT"*|*"network"*|*"socket hang up"*|*"503"*|*"502"*|*"504"*)
                                          CLASS="transient" ;;
    *"500"*|*"Internal Server Error"*)   CLASS="transient" ;;
    *"unable to verify"*)                CLASS="transient" ;;
  esac

  if [ "$CLASS" = "attestation-failure" ] || [ "$CLASS" = "signature-failure" ]; then
    # Hard failure — retrying will not change the answer. Emit and
    # exit immediately so consumers see the real signal fast.
    atlas_fail "'npm audit signatures' reports $CLASS — this is a hard failure (will not re-attempt):"
    printf '%s\n' "$NPM_OUTPUT" >&2
    exit 1
  fi

  if [ "$ATTEMPT" -ge "$MAX" ]; then
    break
  fi

  WAIT="$(backoff_for_attempt "$ATTEMPT")"
  atlas_warn "Attempt $ATTEMPT failed (class=$CLASS, exit=$NPM_EXIT). Retrying in ${WAIT}s — see CONSUMER-RUNBOOK §5 'Transient outage' note."
  sleep "$WAIT"
done

if [ "$NPM_EXIT" -ne 0 ]; then
  atlas_fail "'npm audit signatures' failed after $ATTEMPT attempts (exit=$NPM_EXIT). Output:"
  printf '%s\n' "$NPM_OUTPUT" >&2
  atlas_fail "If this looks like a transient Sigstore / npm outage, check https://status.sigstore.dev and https://status.npmjs.org and re-run. Otherwise treat as a security incident per CONSUMER-RUNBOOK §5 step 4."
  exit 1
fi

# Print the npm output for the audit log.
printf '%s\n' "$NPM_OUTPUT"

# Now: assert the output specifically references OUR package as
# verified. `npm audit signatures` exit 0 alone is not enough — it
# would also exit 0 on an empty audit (no attestable packages in
# tree). We need to see "verified attestation" in the output and
# either no failure block referencing our package OR a top-line
# count >= 1.
#
# Output format examples (npm 10.x):
#   audited 1 package in <duration>
#   1 package has a verified registry signature
#   1 package has a verified attestation
#
# Or with multiple packages:
#   audited N packages in <duration>
#   N packages have verified registry signatures
#   N packages have verified attestations
#
# Or with a failure:
#   audit failed:
#   <pkg>@<v> has a missing or invalid attestation

# Check our package isn't in a failure block. Case-insensitive `-i`
# defends against future npm output-format changes that may alter
# casing of the failure verbs ("Missing", "INVALID", etc.).
if printf '%s\n' "$NPM_OUTPUT" | grep -iF "$PACKAGE" | grep -iE "(missing|invalid|failed|error|untrusted)" >/dev/null 2>&1; then
  atlas_fail "'npm audit signatures' output mentions '$PACKAGE' in a failure context. Treat as security incident per CONSUMER-RUNBOOK §5."
  exit 1
fi

# Check at least one attestation was verified. The npm-ls
# pre-condition above guarantees `$PACKAGE` is in the resolved
# tree, AND the negative grep above guarantees `$PACKAGE` is not
# in a failure context. Together with at least one "verified
# attestation" line, this proves Layer 3 holds for `$PACKAGE` —
# `npm audit signatures` reports counts in the success case (not
# per-package "X verified Y" lines), so we cannot grep for
# `"$PACKAGE" + "verified"` directly.
ATTESTATION_LINE="$(printf '%s\n' "$NPM_OUTPUT" | grep -iE "verified attestation" | head -1 || true)"
if [ -z "$ATTESTATION_LINE" ]; then
  # `$PACKAGE` is in the tree (asserted above) and not in a failure
  # context, but npm reported zero verified attestations. Either
  # the npm attestation API changed its output phrasing, or the
  # entire tree is unattested. Atlas has been signed since V1.14
  # Scope E, so an unattested verifier is a security incident.
  atlas_fail "'npm audit signatures' reported NO verified attestations even though '$PACKAGE' is in the resolved install tree. Treat as security incident per CONSUMER-RUNBOOK §5 unless this is a known npm output-format change."
  exit 1
fi

atlas_pass "Layer 3 OK — npm reports verified attestation: '$ATTESTATION_LINE'"
