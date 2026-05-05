#!/usr/bin/env bash
# run-tests.sh — fixture-based test harness for verify-wasm-pin-check.
#
# Exercises each helper script against synthetic fixtures and asserts
# the exit code matches the per-fixture expectation.
#
# Layer 3 (check-provenance.sh) is NOT covered here — it requires a
# live `npm audit signatures` round-trip to npmjs.org + rekor.sigstore.dev
# and a real installed package. Layer 3 is exercised only by the
# self-test workflow (.github/workflows/verify-wasm-pin-check-self-test.yml)
# which runs against the published `@atlas-trust/verify-wasm` package.
#
# Why exit-code-only and not output-content matching:
#   * Output strings are user-facing copy that may be tweaked over time
#     (for clarity, for i18n, for log-density). The CONTRACT is the
#     exit code — that's what the GitHub Actions composite step gates
#     on. Tightly coupling tests to exact output strings would generate
#     noise on every cosmetic change.
#   * On unexpected exit, we DUMP the captured output so a human
#     debugging the test failure has the context they need.
#
# Usage:
#   bash test/run-tests.sh
#
# Exit code: 0 if all cases pass, 1 if any case fails. CI gates on
# exit code.

set -uo pipefail
# NOT -e: we WANT to capture script exit codes for assertion.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES="$ROOT/test/fixtures"
SCRIPTS="$ROOT/scripts"

if [ ! -d "$FIXTURES" ]; then
  echo "FATAL: fixtures dir not found: $FIXTURES" >&2
  exit 2
fi

if [ ! -d "$SCRIPTS" ]; then
  echo "FATAL: scripts dir not found: $SCRIPTS" >&2
  exit 2
fi

# ------------------------------------------------------------------
# Test harness state
# ------------------------------------------------------------------
PASS_COUNT=0
FAIL_COUNT=0
FAILED_CASES=()

# run_case <name> <fixture> <script> <expected_exit> [env KEY=VAL ...]
#
# Runs $SCRIPTS/$script with ATLAS_PIN_CHECK_WORKDIR set to the
# fixture's directory. Additional env vars are passed through `env`.
# Captures combined stdout+stderr; on mismatch, dumps it for debug.
run_case() {
  local name="$1"; shift
  local fixture="$1"; shift
  local script="$1"; shift
  local expected_exit="$1"; shift

  local workdir="$FIXTURES/$fixture"

  if [ ! -d "$workdir" ]; then
    FAIL_COUNT=$((FAIL_COUNT + 1))
    FAILED_CASES+=("$name (fixture missing: $workdir)")
    printf '  [FAIL] %s — fixture missing: %s\n' "$name" "$workdir" >&2
    return
  fi

  local output
  local actual_exit

  # Build env array. Always include WORKDIR; pass the rest verbatim.
  local env_args=("ATLAS_PIN_CHECK_WORKDIR=$workdir")
  while [ "$#" -gt 0 ]; do
    env_args+=("$1")
    shift
  done

  set +e
  output="$(env -i \
    PATH="$PATH" \
    HOME="${HOME:-/tmp}" \
    TERM="${TERM:-dumb}" \
    "${env_args[@]}" \
    bash "$SCRIPTS/$script" 2>&1)"
  actual_exit=$?
  set -e

  if [ "$actual_exit" = "$expected_exit" ]; then
    PASS_COUNT=$((PASS_COUNT + 1))
    printf '  [PASS] %s (exit=%s)\n' "$name" "$actual_exit"
  else
    FAIL_COUNT=$((FAIL_COUNT + 1))
    FAILED_CASES+=("$name (exit=$actual_exit, expected=$expected_exit)")
    printf '  [FAIL] %s (exit=%s, expected=%s)\n' "$name" "$actual_exit" "$expected_exit" >&2
    printf '         ----- captured output -----\n' >&2
    printf '%s\n' "$output" | sed 's/^/         | /' >&2
    printf '         ----- end output -----\n' >&2
  fi
}

section() {
  printf '\n=== %s ===\n' "$*"
}

# ------------------------------------------------------------------
# Setup-phase tests — input validation + lockfile detection.
# ------------------------------------------------------------------
section "setup.sh — input validation + lockfile detection"

run_case "setup: npm-pinned-good"            "npm-pinned-good"            "setup.sh" 0
run_case "setup: pnpm-pinned-good"           "pnpm-pinned-good"           "setup.sh" 0
run_case "setup: bun-text-pinned-good"       "bun-text-pinned-good"       "setup.sh" 0
run_case "setup: dev-dependencies-pinned"    "dev-dependencies-pinned"    "setup.sh" 0
run_case "setup: multi-lockfile (auto WARN)" "multi-lockfile"             "setup.sh" 0
run_case "setup: rejects bogus expected-version" \
  "npm-pinned-good" "setup.sh" 1 \
  "ATLAS_PIN_CHECK_EXPECTED_VERSION=^1.15.0"
run_case "setup: explicit pm=npm against pnpm-only fails" \
  "pnpm-pinned-good" "setup.sh" 1 \
  "ATLAS_PIN_CHECK_PM=npm"
run_case "setup: explicit pm=pnpm against npm-only fails" \
  "npm-pinned-good" "setup.sh" 1 \
  "ATLAS_PIN_CHECK_PM=pnpm"

# ------------------------------------------------------------------
# Layer 1 — check-version-pin.sh
# ------------------------------------------------------------------
section "Layer 1 — check-version-pin.sh"

# Happy paths
run_case "L1: npm-pinned-good"               "npm-pinned-good"            "check-version-pin.sh" 0
run_case "L1: dev-dependencies-pinned"       "dev-dependencies-pinned"    "check-version-pin.sh" 0
run_case "L1: pnpm-pinned-good"              "pnpm-pinned-good"           "check-version-pin.sh" 0
run_case "L1: bun-text-pinned-good"          "bun-text-pinned-good"       "check-version-pin.sh" 0
run_case "L1: npm-v1-lockfile"               "npm-v1-lockfile"            "check-version-pin.sh" 0
run_case "L1: multi-lockfile"                "multi-lockfile"             "check-version-pin.sh" 0
run_case "L1: npm-local-file"                "npm-local-file"             "check-version-pin.sh" 0
run_case "L1: npm-weak-hash (Layer1 only)"   "npm-weak-hash"              "check-version-pin.sh" 0
run_case "L1: npm-no-integrity (Layer1 only)" "npm-no-integrity"          "check-version-pin.sh" 0

# Expected-version exact match
run_case "L1: expected-version match (1.15.0)" \
  "npm-pinned-good" "check-version-pin.sh" 0 \
  "ATLAS_PIN_CHECK_EXPECTED_VERSION=1.15.0"

# Failure paths
run_case "L1: caret pin rejected"            "npm-caret-bad"              "check-version-pin.sh" 1
run_case "L1: tilde pin rejected"            "npm-tilde-bad"              "check-version-pin.sh" 1
run_case "L1: package not declared"          "npm-not-installed"          "check-version-pin.sh" 1
run_case "L1: expected-version mismatch (1.14.0 vs 1.15.0)" \
  "npm-mismatched-version" "check-version-pin.sh" 1 \
  "ATLAS_PIN_CHECK_EXPECTED_VERSION=1.15.0"
# Mismatched-version WITHOUT expected-version should pass (1.14.0 is a valid bare semver pin).
run_case "L1: mismatched fixture without expected-version passes" \
  "npm-mismatched-version" "check-version-pin.sh" 0

# ------------------------------------------------------------------
# Layer 2 — check-lockfile-integrity.sh (router + per-PM dispatch)
# ------------------------------------------------------------------
section "Layer 2 — check-lockfile-integrity.sh"

# Happy paths
run_case "L2: npm-pinned-good"               "npm-pinned-good"            "check-lockfile-integrity.sh" 0
run_case "L2: dev-dependencies-pinned"       "dev-dependencies-pinned"    "check-lockfile-integrity.sh" 0
run_case "L2: npm-v1-lockfile"               "npm-v1-lockfile"            "check-lockfile-integrity.sh" 0
run_case "L2: pnpm-pinned-good"              "pnpm-pinned-good"           "check-lockfile-integrity.sh" 0
run_case "L2: bun-text-pinned-good"          "bun-text-pinned-good"       "check-lockfile-integrity.sh" 0

# multi-lockfile — auto-detect picks npm (which is good), warns about pnpm presence
run_case "L2: multi-lockfile auto picks npm" "multi-lockfile"             "check-lockfile-integrity.sh" 0
run_case "L2: multi-lockfile explicit pnpm"  "multi-lockfile"             "check-lockfile-integrity.sh" 0 \
  "ATLAS_PIN_CHECK_PM=pnpm"

# Local-file backup-channel — WARN by default, FAIL with fail-on-local-file=true
run_case "L2: npm-local-file (WARN by default)" \
  "npm-local-file" "check-lockfile-integrity.sh" 0
run_case "L2: npm-local-file (FAIL with fail-on-local-file=true)" \
  "npm-local-file" "check-lockfile-integrity.sh" 1 \
  "ATLAS_PIN_CHECK_FAIL_ON_LOCAL=true"

# Failure paths
run_case "L2: npm missing integrity"         "npm-no-integrity"           "check-lockfile-integrity.sh" 1
run_case "L2: npm weak hash (sha1)"          "npm-weak-hash"              "check-lockfile-integrity.sh" 1
run_case "L2: pnpm missing integrity"        "pnpm-no-integrity-bad"      "check-lockfile-integrity.sh" 1
run_case "L2: bun-text missing integrity"    "bun-text-no-integrity-bad"  "check-lockfile-integrity.sh" 1

# ------------------------------------------------------------------
# Summary
# ------------------------------------------------------------------
section "Test summary"
TOTAL=$((PASS_COUNT + FAIL_COUNT))
printf 'PASS: %d / %d\n' "$PASS_COUNT" "$TOTAL"
printf 'FAIL: %d / %d\n' "$FAIL_COUNT" "$TOTAL"

if [ "$FAIL_COUNT" -ne 0 ]; then
  printf '\nFailed cases:\n'
  for c in "${FAILED_CASES[@]}"; do
    printf '  - %s\n' "$c"
  done
  exit 1
fi

printf '\nAll cases passed.\n'
exit 0
