#!/usr/bin/env bash
# V1.17 Welle B — anti-drift test harness for `tools/verify-tag-signatures.sh`.
#
# Pattern parity with `.github/actions/verify-wasm-pin-check/test/run-tests.sh`:
# pure-bash, no fixtures-on-disk, every case is built from scratch in
# a tempdir + torn down. Asserts every code path the verifier can take
# WITHOUT requiring an SSH signing key (which would be host-state-
# dependent and not reproducible in CI).
#
# What we can cover here:
#   * Trust-root validation (missing / empty / comments-only).
#   * Tag-existence check (unknown tag arg).
#   * Lightweight tag (no tag object → not verifiable).
#   * Annotated unsigned tag (tag object exists, no signature).
#   * "No tags yet" early-exit.
#   * Status/init subcommands of `setup-tag-signing.sh`.
#
# What we CANNOT cover here (host-state-dependent):
#   * Tag signed by trusted key → PASS. Requires the maintainer to
#     have configured `git config user.signingkey` and to have a key
#     whose public counterpart is in `.github/allowed_signers`. This
#     path is exercised live in CI on the first signed `v*` tag push,
#     and locally by the maintainer running `git tag -s vX.Y.Z`
#     followed by `bash tools/verify-tag-signatures.sh vX.Y.Z` — the
#     setup-tag-signing.sh status subcommand confirms readiness.
#
# Each case prints `PASS:` or `FAIL:` and contributes to a final
# summary line. Exit non-zero iff any case failed.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

PASS=0
FAIL=0
FAILED_CASES=()

assert() {
  local name="$1"
  local cmd="$2"
  local want_exit="$3"
  local stdout_grep_pattern="${4:-}"

  # Single-invocation capture of stdout+stderr+exit-code. Earlier
  # double-eval pattern executed each command twice — for cases that
  # mutate state (e.g. create-then-delete a test tag), the second run
  # observed the post-cleanup state and the exit code corresponded to
  # a different code path than the captured output. The trailing
  # `__ATLAS_TEST_EXIT__:N` marker is parsed off after capture so
  # got_output stays clean for the subsequent grep-pattern check.
  local got_exit
  local got_output
  got_output="$(eval "${cmd}" 2>&1; echo "__ATLAS_TEST_EXIT__:$?")"
  got_exit="${got_output##*__ATLAS_TEST_EXIT__:}"
  got_output="${got_output%__ATLAS_TEST_EXIT__:*}"

  if [ "${got_exit}" != "${want_exit}" ]; then
    echo "  FAIL: ${name}"
    echo "        wanted exit ${want_exit}, got ${got_exit}"
    echo "        cmd: ${cmd}"
    echo "        output:"
    printf '%s\n' "${got_output}" | sed 's/^/          /'
    FAIL=$((FAIL + 1))
    FAILED_CASES+=("${name}")
    return 1
  fi

  if [ -n "${stdout_grep_pattern}" ]; then
    if ! printf '%s\n' "${got_output}" | grep -qE "${stdout_grep_pattern}"; then
      echo "  FAIL: ${name}"
      echo "        exit ${got_exit} matched, but stdout did not match pattern: ${stdout_grep_pattern}"
      echo "        output:"
      printf '%s\n' "${got_output}" | sed 's/^/          /'
      FAIL=$((FAIL + 1))
      FAILED_CASES+=("${name}")
      return 1
    fi
  fi

  echo "  PASS: ${name}"
  PASS=$((PASS + 1))
}

# Save then restore: the harness creates lightweight + annotated test
# tags; on any exit (success, failure, ^C) clean them up so the repo
# isn't left in a polluted state.
TEST_TAGS=(
  v0.0.0-test-lightweight
  v0.0.0-test-annotated
)
cleanup() {
  for t in "${TEST_TAGS[@]}"; do
    git tag -d "${t}" >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT INT TERM

# Pre-clean in case a prior run aborted before its trap.
cleanup

echo "=== V1.17 Welle B — verify-tag-signatures.sh test harness ==="
echo ""

echo "--- 1. trust-root validation ---"
assert "missing allowed_signers file" \
  "ATLAS_ALLOWED_SIGNERS=/nonexistent/path/allowed_signers bash tools/verify-tag-signatures.sh" \
  "2" \
  "FAIL: allowed-signers file missing"

EMPTY_FILE="$(mktemp -t atlas-test-empty.XXXXXX)"
assert "empty allowed_signers file" \
  "ATLAS_ALLOWED_SIGNERS='${EMPTY_FILE}' bash tools/verify-tag-signatures.sh" \
  "2" \
  "contains zero signer entries"
rm -f "${EMPTY_FILE}"

COMMENTS_ONLY_FILE="$(mktemp -t atlas-test-comments.XXXXXX)"
printf '# header comment\n\n  # indented comment\n\n' > "${COMMENTS_ONLY_FILE}"
assert "comments-only allowed_signers" \
  "ATLAS_ALLOWED_SIGNERS='${COMMENTS_ONLY_FILE}' bash tools/verify-tag-signatures.sh" \
  "2" \
  "contains zero signer entries"
rm -f "${COMMENTS_ONLY_FILE}"

echo ""
echo "--- 2. early-exit paths ---"
# "No tags yet" path: with no v* tags AND a populated allowed_signers,
# the verifier should INFO + exit 0. We assert against the production
# allowed_signers file (the one committed to the repo).
# Pre-condition: NO v* tags exist. Cleanup() above guarantees this for
# the test tags; if the repo has real v* tags this case will skip.
ACTUAL_VSTAR_TAGS="$(git tag -l 'v*' | wc -l | tr -d ' ')"
if [ "${ACTUAL_VSTAR_TAGS}" -eq 0 ]; then
  assert "no v* tags → INFO + exit 0" \
    "bash tools/verify-tag-signatures.sh" \
    "0" \
    "no v\\* tags in this repo yet"
else
  echo "  SKIP: no-tags-yet (repo already has ${ACTUAL_VSTAR_TAGS} v* tag(s))"
fi

echo ""
echo "--- 3. tag-existence check ---"
assert "unknown tag arg → FAIL" \
  "bash tools/verify-tag-signatures.sh v999.999.999-does-not-exist" \
  "1" \
  "tag does not exist in this repo"

echo ""
echo "--- 4. lightweight tag (commit-pointing) ---"
git tag v0.0.0-test-lightweight HEAD >/dev/null 2>&1
assert "lightweight tag → FAIL with non-tag-object diagnostic" \
  "bash tools/verify-tag-signatures.sh v0.0.0-test-lightweight" \
  "1" \
  "lightweight tag — no tag object"
# Sub-assertion: the diagnostic must remain actionable for the two
# real-world causes (local maintainer used `git tag` not `git tag -s`,
# OR CI environment fetched the ref without the annotated tag object).
# Failing this regression catches a future "helpful refactor" that
# strips the actionable hints from the error path.
assert "lightweight tag diagnostic includes local + CI remediation" \
  "bash tools/verify-tag-signatures.sh v0.0.0-test-lightweight 2>&1" \
  "1" \
  "git fetch --tags --force origin"
git tag -d v0.0.0-test-lightweight >/dev/null 2>&1

echo ""
echo "--- 5. annotated unsigned tag ---"
git tag -a -m "test annotated unsigned" v0.0.0-test-annotated HEAD >/dev/null 2>&1
assert "annotated unsigned tag → FAIL with no-signature diagnostic" \
  "bash tools/verify-tag-signatures.sh v0.0.0-test-annotated" \
  "1" \
  "(no signature found|FAIL: v0.0.0-test-annotated)"
git tag -d v0.0.0-test-annotated >/dev/null 2>&1

echo ""
echo "--- 6. setup-tag-signing.sh subcommands ---"
assert "status subcommand → exit 0 + reports trust root" \
  "bash tools/setup-tag-signing.sh status" \
  "0" \
  "Trust root"

assert "no subcommand → usage + exit 0" \
  "bash tools/setup-tag-signing.sh" \
  "0" \
  "Usage:"

assert "unknown subcommand → exit 2 + usage" \
  "bash tools/setup-tag-signing.sh nonsense" \
  "2" \
  "unknown subcommand"

assert "init --key /nonexistent → exit 2" \
  "bash tools/setup-tag-signing.sh init --key /nonexistent/key.pub" \
  "2" \
  "pubkey file does not exist"

assert "add with too few args → exit 2" \
  "bash tools/setup-tag-signing.sh add only-one-arg" \
  "2" \
  "requires <principal> <pubkey-path>"

# Test 'add' with a valid-format file but unsupported key type.
BAD_KEY_FILE="$(mktemp -t atlas-test-badkey.XXXXXX)"
printf 'ssh-dss AAAAB3NzaC1kc3MAAACB= test@example\n' > "${BAD_KEY_FILE}"
assert "add with deprecated key type → exit 2" \
  "bash tools/setup-tag-signing.sh add test@example '${BAD_KEY_FILE}'" \
  "2" \
  "not on the allow-list"
rm -f "${BAD_KEY_FILE}"

echo ""
echo "==="
TOTAL=$((PASS + FAIL))
echo "PASS: ${PASS} / ${TOTAL}    FAIL: ${FAIL} / ${TOTAL}"

if [ "${FAIL}" -gt 0 ]; then
  echo "FAILED CASES: ${FAILED_CASES[*]}" >&2
  exit 1
fi
exit 0
