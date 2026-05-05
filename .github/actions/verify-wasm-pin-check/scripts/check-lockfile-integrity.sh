#!/usr/bin/env bash
# check-lockfile-integrity.sh — Layer 2 of the CONSUMER-RUNBOOK §1 stack.
#
# Asserts that the lockfile (auto-detected: package-lock.json /
# pnpm-lock.yaml / bun.lockb) carries an integrity hash for the
# package, AND that the hash is `sha512-<base64>` (npm / pnpm
# canonical) or `sha256-<base64>` (some bun/pnpm versions also write
# sha256). Other hash variants (sha1, md5) are flagged as a hard fail
# — `npm install` would still work but the integrity is no longer
# defence-in-depth against registry-side replacement attacks
# (sha1 collisions are practical).
#
# Bun lockfile note: `bun.lockb` is binary. We delegate to
# `bun pm ls --json` which prints the resolved tree with integrity
# hashes if `bun` is available on PATH; if not, we degrade to a
# WARN ("could not validate bun.lockb integrity — install bun in CI
# or switch to bun.lock text format"). The text-format `bun.lock`
# is grep-able and is preferred where available.
#
# Per V1.15 Welle B (CONSUMER-RUNBOOK §2 backup-channel install
# caveat): a lockfile entry resolved to `file:atlas-…tgz` is a
# legitimate state during the backup-channel install ceremony, but
# downstream of that the consumer is expected to re-pin to the
# registry source. We warn by default; `fail-on-local-file: true`
# input promotes the WARN to a hard fail.

set -euo pipefail

# shellcheck source=./lib/log.sh
. "$(dirname "$0")/lib/log.sh"
# shellcheck source=./lib/detect-pm.sh
. "$(dirname "$0")/lib/detect-pm.sh"

PACKAGE="${ATLAS_PIN_CHECK_PACKAGE:-@atlas-trust/verify-wasm}"
WORKDIR="${ATLAS_PIN_CHECK_WORKDIR:-.}"
FAIL_ON_LOCAL="${ATLAS_PIN_CHECK_FAIL_ON_LOCAL:-false}"

atlas_section "Layer 2 — Lockfile integrity hash"

# shellcheck source=./lib/canonicalize-workdir.sh
. "$(dirname "$0")/lib/canonicalize-workdir.sh"
# Re-export the canonical WORKDIR so the dispatched sub-scripts
# (check-lockfile-integrity-{npm,pnpm,bun,bun-text}.sh) read the
# same resolved path via $ATLAS_PIN_CHECK_WORKDIR rather than
# re-canonicalising the raw input. They still source the lib for
# defence-in-depth, but the second canonicalisation is a no-op on
# an already-canonical path.
export ATLAS_PIN_CHECK_WORKDIR="$WORKDIR"

atlas_detect_pm

atlas_info "package-manager  : $ATLAS_PIN_CHECK_PM_RESOLVED"
atlas_info "lockfile         : $ATLAS_PIN_CHECK_LOCKFILE"

# Each PM has its own lockfile schema. Branch and dispatch.
case "$ATLAS_PIN_CHECK_PM_RESOLVED" in
  npm)
    bash "$(dirname "$0")/check-lockfile-integrity-npm.sh"
    ;;
  pnpm)
    bash "$(dirname "$0")/check-lockfile-integrity-pnpm.sh"
    ;;
  bun)
    bash "$(dirname "$0")/check-lockfile-integrity-bun.sh"
    ;;
  *)
    atlas_fail "internal error — unknown PM '$ATLAS_PIN_CHECK_PM_RESOLVED'"
    exit 2
    ;;
esac
