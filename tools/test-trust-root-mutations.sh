#!/usr/bin/env bash
# V1.17 Welle C — anti-drift test harness for `tools/verify-trust-root-mutations.sh`.
#
# Pattern parity with `tools/test-tag-signatures.sh`: pure-bash, every
# case is built from scratch in a tempdir + torn down. Asserts every
# code path the verifier can take WITHOUT requiring an SSH signing key
# (which would be host-state-dependent and not reproducible in CI).
#
# What we cover here:
#   * Argument validation (--base / --head ref does-not-exist; bad arg).
#   * CI containment guard (env-var override under $GITHUB_ACTIONS=true).
#   * Bootstrap mode (no allowed_signers at merge-base → exit 0).
#   * Empty / comments-only allowed_signers at merge-base → exit 2.
#   * No surface-modifying commits in range → exit 0 (no-op pass).
#   * Unsigned commit modifying surface → exit 1 with diagnostic.
#   * Protected-surface list parity with `.github/CODEOWNERS`.
#
# What we do NOT cover here (host-state-dependent):
#   * Trusted-key signed commit → PASS. Requires a working
#     `git config user.signingkey` setup pointing at a key whose public
#     counterpart is in `.github/allowed_signers`. This path is
#     exercised live in CI on the first surface-modifying PR after the
#     workflow is deployed; a maintainer can also exercise it locally
#     after running `bash tools/setup-tag-signing.sh init` and creating
#     a signed test commit.
#
# Each case prints `PASS:` or `FAIL:` and contributes to a final
# summary line. Exit non-zero iff any case failed.

set -euo pipefail

REAL_REPO_ROOT="$(git rev-parse --show-toplevel)"
SCRIPT_PATH="${REAL_REPO_ROOT}/tools/verify-trust-root-mutations.sh"

if [ ! -f "${SCRIPT_PATH}" ]; then
  echo "FAIL: ${SCRIPT_PATH} not found — harness must be run from the atlas repo." >&2
  exit 2
fi

PASS=0
FAIL=0
FAILED_CASES=()

assert() {
  local name="$1"
  local cmd="$2"
  local want_exit="$3"
  local stdout_grep_pattern="${4:-}"

  # Mirror of the test-tag-signatures.sh single-invocation capture
  # pattern: `eval` runs exactly once per case. Earlier double-eval
  # variants flipped exit codes for state-creating tests.
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
    if ! printf '%s\n' "${got_output}" | grep -qE -- "${stdout_grep_pattern}"; then
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

# ----------------------------------------------------------------------
# Test-repo factory.
#
# Each case gets a fresh git repo in a tempdir. The repo is initialised
# with commit-signing DISABLED (we want manufactured unsigned commits
# for the negative-path coverage). Cases that need a populated trust
# root copy the production .github/allowed_signers from the real repo.
# ----------------------------------------------------------------------
make_test_repo() {
  local repo_dir="$1"
  rm -rf "${repo_dir}"
  mkdir -p "${repo_dir}"
  (
    cd "${repo_dir}"
    git init -q -b main
    git config user.email "test@example.com"
    git config user.name "Test Harness"
    git config commit.gpgSign false
    git config tag.gpgSign false
    # No reflog noise on the test runner.
    git config gc.auto 0
  )
}

# Cleanup tempdirs on any exit.
TEST_TMPDIR="$(mktemp -d -t atlas-trust-root-harness.XXXXXX)"
trap 'rm -rf "${TEST_TMPDIR}"' EXIT INT TERM

echo "=== V1.17 Welle C — verify-trust-root-mutations.sh test harness ==="
echo ""

# ----------------------------------------------------------------------
# 1. Argument validation.
# ----------------------------------------------------------------------
echo "--- 1. argument validation ---"

assert "unknown argument → exit 2" \
  "bash '${SCRIPT_PATH}' --bogus" \
  "2" \
  "unknown argument"

assert "--base without value → exit 2" \
  "bash '${SCRIPT_PATH}' --base" \
  "2" \
  "--base requires a ref argument"

assert "--head without value → exit 2" \
  "bash '${SCRIPT_PATH}' --head" \
  "2" \
  "--head requires a ref argument"

# Use the real repo for ref-resolution cases — easier than building a
# temp repo with controlled refs.
assert "unknown base ref → exit 2" \
  "bash '${SCRIPT_PATH}' --base does-not-exist-ref --head HEAD" \
  "2" \
  "base ref 'does-not-exist-ref' does not resolve"

assert "unknown head ref → exit 2" \
  "bash '${SCRIPT_PATH}' --base HEAD --head does-not-exist-ref" \
  "2" \
  "head ref 'does-not-exist-ref' does not resolve"

echo ""

# ----------------------------------------------------------------------
# 2. CI containment guard.
# ----------------------------------------------------------------------
echo "--- 2. CI containment guard ---"

assert "ATLAS_TRUST_ROOT_BASE under GITHUB_ACTIONS=true → exit 2" \
  "GITHUB_ACTIONS=true ATLAS_TRUST_ROOT_BASE=HEAD bash '${SCRIPT_PATH}'" \
  "2" \
  "env-var override not allowed in CI"

assert "ATLAS_TRUST_ROOT_HEAD under GITHUB_ACTIONS=true → exit 2" \
  "GITHUB_ACTIONS=true ATLAS_TRUST_ROOT_HEAD=HEAD bash '${SCRIPT_PATH}'" \
  "2" \
  "env-var override not allowed in CI"

echo ""

# ----------------------------------------------------------------------
# 3. Bootstrap mode (no allowed_signers at merge-base).
# ----------------------------------------------------------------------
echo "--- 3. bootstrap mode ---"

BOOTSTRAP_REPO="${TEST_TMPDIR}/bootstrap"
make_test_repo "${BOOTSTRAP_REPO}"
(
  cd "${BOOTSTRAP_REPO}"
  # Initial commit with no allowed_signers. This is the merge-base.
  echo "hello" > README.md
  git add README.md
  git commit -q -m "initial"
  BASE_SHA="$(git rev-parse HEAD)"

  # Second commit modifies a surface file. Allowed_signers still does
  # NOT exist at the merge-base — the verifier should bootstrap-pass.
  mkdir -p .github
  echo "# trust root" > .github/allowed_signers
  git add .github/allowed_signers
  git commit -q -m "add trust root (unsigned, but bootstrap)"
  HEAD_SHA="$(git rev-parse HEAD)"

  # Export refs to outer shell via filesystem.
  echo "${BASE_SHA}" > "${TEST_TMPDIR}/bootstrap-base.sha"
  echo "${HEAD_SHA}" > "${TEST_TMPDIR}/bootstrap-head.sha"
)

# Pre-resolve SHAs into shell variables so the eval'd command string
# does NOT contain `$(cat ...)` substitutions over /tmp paths. Earlier
# pattern was a TOCTOU footgun on multi-tenant CI runners that share
# /tmp — a colluding process could swap the .sha file's contents
# between mktemp time and eval time and inject commands. Reading once
# into a single-quoted shell variable closes the window.
BOOTSTRAP_BASE="$(cat "${TEST_TMPDIR}/bootstrap-base.sha")"
BOOTSTRAP_HEAD="$(cat "${TEST_TMPDIR}/bootstrap-head.sha")"
assert "bootstrap (no allowed_signers at merge-base) → exit 0" \
  "cd '${BOOTSTRAP_REPO}' && bash '${SCRIPT_PATH}' --base '${BOOTSTRAP_BASE}' --head '${BOOTSTRAP_HEAD}'" \
  "0" \
  "Bootstrap mode"

echo ""

# ----------------------------------------------------------------------
# 4. Empty / comments-only allowed_signers at merge-base.
# ----------------------------------------------------------------------
echo "--- 4. empty trust root at merge-base ---"

EMPTY_TR_REPO="${TEST_TMPDIR}/empty-trust-root"
make_test_repo "${EMPTY_TR_REPO}"
(
  cd "${EMPTY_TR_REPO}"
  mkdir -p .github
  # Comments-only allowed_signers at base.
  printf '# header comment\n\n# another comment\n' > .github/allowed_signers
  echo "hello" > README.md
  git add .github/allowed_signers README.md
  git commit -q -m "base with comments-only trust root"
  BASE_SHA="$(git rev-parse HEAD)"

  # Modify a surface file (the trust root itself).
  printf '# header comment\n\n# another comment\nfake-key entry\n' > .github/allowed_signers
  git add .github/allowed_signers
  git commit -q -m "modify trust root"
  HEAD_SHA="$(git rev-parse HEAD)"

  echo "${BASE_SHA}" > "${TEST_TMPDIR}/empty-tr-base.sha"
  echo "${HEAD_SHA}" > "${TEST_TMPDIR}/empty-tr-head.sha"
)

EMPTY_TR_BASE="$(cat "${TEST_TMPDIR}/empty-tr-base.sha")"
EMPTY_TR_HEAD="$(cat "${TEST_TMPDIR}/empty-tr-head.sha")"
assert "comments-only allowed_signers at merge-base → exit 2" \
  "cd '${EMPTY_TR_REPO}' && bash '${SCRIPT_PATH}' --base '${EMPTY_TR_BASE}' --head '${EMPTY_TR_HEAD}'" \
  "2" \
  "has zero signer entries"

echo ""

# ----------------------------------------------------------------------
# 5. No surface-modifying commits in range.
# ----------------------------------------------------------------------
echo "--- 5. no surface-modifying commits ---"

NO_SURFACE_REPO="${TEST_TMPDIR}/no-surface"
make_test_repo "${NO_SURFACE_REPO}"
(
  cd "${NO_SURFACE_REPO}"
  # Set up a populated trust root at base (use the real repo's file).
  mkdir -p .github
  cp "${REAL_REPO_ROOT}/.github/allowed_signers" .github/allowed_signers
  echo "hello" > README.md
  git add .github/allowed_signers README.md
  git commit -q -m "base with populated trust root"
  BASE_SHA="$(git rev-parse HEAD)"

  # Commits that don't touch the protected surface.
  echo "more" > docs.md
  git add docs.md
  git commit -q -m "add docs"
  echo "more2" > docs2.md
  git add docs2.md
  git commit -q -m "add more docs"
  HEAD_SHA="$(git rev-parse HEAD)"

  echo "${BASE_SHA}" > "${TEST_TMPDIR}/no-surface-base.sha"
  echo "${HEAD_SHA}" > "${TEST_TMPDIR}/no-surface-head.sha"
)

NO_SURFACE_BASE="$(cat "${TEST_TMPDIR}/no-surface-base.sha")"
NO_SURFACE_HEAD="$(cat "${TEST_TMPDIR}/no-surface-head.sha")"
assert "no surface-modifying commits → exit 0 + no-op message" \
  "cd '${NO_SURFACE_REPO}' && bash '${SCRIPT_PATH}' --base '${NO_SURFACE_BASE}' --head '${NO_SURFACE_HEAD}'" \
  "0" \
  "no commits in .* modify the trust-root surface"

# L-3: zero-commit range (--base X --head X) → exit 0 with "no
# commits to verify". Future-proof: a refactor of the early-exit path
# could accidentally turn this into exit 1 with no harness coverage.
assert "zero-commit range (base==head) → exit 0" \
  "cd '${NO_SURFACE_REPO}' && bash '${SCRIPT_PATH}' --base '${NO_SURFACE_HEAD}' --head '${NO_SURFACE_HEAD}'" \
  "0" \
  "no commits in .* modify the trust-root surface"

echo ""

# ----------------------------------------------------------------------
# 6. Unsigned commit modifies surface (negative path).
# ----------------------------------------------------------------------
echo "--- 6. unsigned commit modifies surface ---"

UNSIGNED_REPO="${TEST_TMPDIR}/unsigned-surface"
make_test_repo "${UNSIGNED_REPO}"
(
  cd "${UNSIGNED_REPO}"
  mkdir -p .github
  cp "${REAL_REPO_ROOT}/.github/allowed_signers" .github/allowed_signers
  echo "hello" > README.md
  git add .github/allowed_signers README.md
  git commit -q -m "base with populated trust root"
  BASE_SHA="$(git rev-parse HEAD)"

  # Unsigned commit modifying allowed_signers.
  printf '\n# attacker key\nattacker@evil.example ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAttackerKeyXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX\n' >> .github/allowed_signers
  git add .github/allowed_signers
  git commit -q -m "add attacker key (unsigned)"
  HEAD_SHA="$(git rev-parse HEAD)"

  echo "${BASE_SHA}" > "${TEST_TMPDIR}/unsigned-base.sha"
  echo "${HEAD_SHA}" > "${TEST_TMPDIR}/unsigned-head.sha"
)

UNSIGNED_BASE="$(cat "${TEST_TMPDIR}/unsigned-base.sha")"
UNSIGNED_HEAD="$(cat "${TEST_TMPDIR}/unsigned-head.sha")"
assert "unsigned commit modifying allowed_signers → exit 1" \
  "cd '${UNSIGNED_REPO}' && bash '${SCRIPT_PATH}' --base '${UNSIGNED_BASE}' --head '${UNSIGNED_HEAD}'" \
  "1" \
  "FAIL:"

# Same scenario but modifying a different surface file (the verifier
# script itself). Asserts the protected-surface list isn't a one-off
# allowed_signers special case — every entry is gated.
SCRIPT_MOD_REPO="${TEST_TMPDIR}/script-mod"
make_test_repo "${SCRIPT_MOD_REPO}"
(
  cd "${SCRIPT_MOD_REPO}"
  mkdir -p .github tools
  cp "${REAL_REPO_ROOT}/.github/allowed_signers" .github/allowed_signers
  cp "${REAL_REPO_ROOT}/tools/verify-tag-signatures.sh" tools/verify-tag-signatures.sh
  echo "hello" > README.md
  git add .github/allowed_signers tools/verify-tag-signatures.sh README.md
  git commit -q -m "base"
  BASE_SHA="$(git rev-parse HEAD)"

  # Append a malicious line to the verifier script.
  echo "# attacker comment" >> tools/verify-tag-signatures.sh
  git add tools/verify-tag-signatures.sh
  git commit -q -m "tamper with verify-tag-signatures.sh (unsigned)"
  HEAD_SHA="$(git rev-parse HEAD)"

  echo "${BASE_SHA}" > "${TEST_TMPDIR}/script-mod-base.sha"
  echo "${HEAD_SHA}" > "${TEST_TMPDIR}/script-mod-head.sha"
)

SCRIPT_MOD_BASE="$(cat "${TEST_TMPDIR}/script-mod-base.sha")"
SCRIPT_MOD_HEAD="$(cat "${TEST_TMPDIR}/script-mod-head.sha")"
assert "unsigned commit modifying tools/verify-tag-signatures.sh → exit 1" \
  "cd '${SCRIPT_MOD_REPO}' && bash '${SCRIPT_PATH}' --base '${SCRIPT_MOD_BASE}' --head '${SCRIPT_MOD_HEAD}'" \
  "1" \
  "FAIL:"

echo ""

# ----------------------------------------------------------------------
# 6a. Merge-commit detection — git diff-tree -m vs git show.
#
# Earlier `git show --name-only --format=` emits no file list for merge
# commits, which would silently exempt a merge from surface matching.
# `git diff-tree -m` correctly enumerates files relative to each parent.
# Test: build a branch where a merge commit brings a surface change in
# from a side branch, then assert verification fails because the merge
# commit (unsigned) modifies a surface file.
# ----------------------------------------------------------------------
echo "--- 6a. merge-commit detection ---"

MERGE_REPO="${TEST_TMPDIR}/merge-commit"
make_test_repo "${MERGE_REPO}"
(
  cd "${MERGE_REPO}"
  mkdir -p .github
  cp "${REAL_REPO_ROOT}/.github/allowed_signers" .github/allowed_signers
  echo "hello" > README.md
  git add .github/allowed_signers README.md
  git commit -q -m "base with populated trust root"
  BASE_SHA="$(git rev-parse HEAD)"

  # Side branch that mutates allowed_signers.
  git checkout -q -b side-branch
  echo "" >> .github/allowed_signers
  echo "# attacker key" >> .github/allowed_signers
  git add .github/allowed_signers
  git commit -q -m "side-branch: add attacker key"

  # Merge side branch back into main as a true merge commit (no FF).
  git checkout -q main
  echo "noise" > unrelated.md
  git add unrelated.md
  git commit -q -m "main: unrelated commit so merge is non-FF"
  git merge -q --no-ff -m "merge side-branch (unsigned merge commit)" side-branch
  HEAD_SHA="$(git rev-parse HEAD)"

  echo "${BASE_SHA}" > "${TEST_TMPDIR}/merge-base.sha"
  echo "${HEAD_SHA}" > "${TEST_TMPDIR}/merge-head.sha"
)

MERGE_BASE_SHA="$(cat "${TEST_TMPDIR}/merge-base.sha")"
MERGE_HEAD_SHA="$(cat "${TEST_TMPDIR}/merge-head.sha")"
assert "merge commit bringing in unsigned allowed_signers change → exit 1" \
  "cd '${MERGE_REPO}' && bash '${SCRIPT_PATH}' --base '${MERGE_BASE_SHA}' --head '${MERGE_HEAD_SHA}'" \
  "1" \
  "FAIL:"

echo ""

# ----------------------------------------------------------------------
# 6b. Prefix-match arm — .github/actions/ subtree.
#
# The composite action under `.github/actions/verify-wasm-pin-check/`
# is gated via PROTECTED_PREFIXES (not PROTECTED_SURFACE) because the
# subtree has many files. Assert an unsigned commit modifying a file
# under that prefix is detected.
# ----------------------------------------------------------------------
echo "--- 6b. prefix-match (.github/actions/ subtree) ---"

PREFIX_REPO="${TEST_TMPDIR}/prefix-match"
make_test_repo "${PREFIX_REPO}"
(
  cd "${PREFIX_REPO}"
  mkdir -p .github/actions/verify-wasm-pin-check/scripts
  cp "${REAL_REPO_ROOT}/.github/allowed_signers" .github/allowed_signers
  echo "#!/usr/bin/env bash" > .github/actions/verify-wasm-pin-check/scripts/check-provenance.sh
  echo "hello" > README.md
  git add .github/allowed_signers .github/actions/verify-wasm-pin-check/scripts/check-provenance.sh README.md
  git commit -q -m "base"
  BASE_SHA="$(git rev-parse HEAD)"

  # Tamper with the composite action script.
  echo "exit 0  # silently neutered" >> .github/actions/verify-wasm-pin-check/scripts/check-provenance.sh
  git add .github/actions/verify-wasm-pin-check/scripts/check-provenance.sh
  git commit -q -m "neuter check-provenance.sh (unsigned)"
  HEAD_SHA="$(git rev-parse HEAD)"

  echo "${BASE_SHA}" > "${TEST_TMPDIR}/prefix-base.sha"
  echo "${HEAD_SHA}" > "${TEST_TMPDIR}/prefix-head.sha"
)

PREFIX_BASE="$(cat "${TEST_TMPDIR}/prefix-base.sha")"
PREFIX_HEAD="$(cat "${TEST_TMPDIR}/prefix-head.sha")"
assert "unsigned commit under .github/actions/ prefix → exit 1" \
  "cd '${PREFIX_REPO}' && bash '${SCRIPT_PATH}' --base '${PREFIX_BASE}' --head '${PREFIX_HEAD}'" \
  "1" \
  "FAIL:"

echo ""

# ----------------------------------------------------------------------
# 6c. Anti-rewrite bootstrap defence — H-2 from V1.17 Welle C review.
#
# An admin force-push of master to a pre-bootstrap commit could
# manufacture a fresh bootstrap window. The defence:
#   `git log --diff-filter=A --all -- .github/allowed_signers`
# enumerates every commit that ever ADDED the file. If any such commit
# is an ancestor of BASE_SHA, bootstrap mode at this merge-base is
# anomalous → fail.
#
# Test: build a repo where commit 1 has no allowed_signers, commit 2
# adds it, commit 3 modifies something else. Then create a side branch
# off commit 1 that adds an attacker key. Computing merge-base of
# (commit-3, side-tip) is commit 1 → no allowed_signers at merge-base
# → naive bootstrap would pass. With the anti-rewrite check, the file
# WAS added in commit 2 (ancestor of commit 3 = BASE_SHA), so bootstrap
# is anomalous → fail.
# ----------------------------------------------------------------------
echo "--- 6c. anti-rewrite bootstrap defence (H-2) ---"

REWRITE_REPO="${TEST_TMPDIR}/anti-rewrite"
make_test_repo "${REWRITE_REPO}"
(
  cd "${REWRITE_REPO}"
  echo "hello" > README.md
  git add README.md
  git commit -q -m "commit 1: pre-bootstrap"
  PRE_BOOTSTRAP="$(git rev-parse HEAD)"

  mkdir -p .github
  cp "${REAL_REPO_ROOT}/.github/allowed_signers" .github/allowed_signers
  git add .github/allowed_signers
  git commit -q -m "commit 2: add allowed_signers (the bootstrap commit)"

  echo "more" > docs.md
  git add docs.md
  git commit -q -m "commit 3: post-bootstrap unrelated change"
  BASE_SHA="$(git rev-parse HEAD)"

  # Side branch off the pre-bootstrap commit.
  git checkout -q -b attacker-branch "${PRE_BOOTSTRAP}"
  mkdir -p .github
  printf 'attacker@evil.example ssh-ed25519 AAAAfake\n' > .github/allowed_signers
  git add .github/allowed_signers
  git commit -q -m "attacker: add attacker key as 'fresh bootstrap'"
  HEAD_SHA="$(git rev-parse HEAD)"

  echo "${BASE_SHA}" > "${TEST_TMPDIR}/rewrite-base.sha"
  echo "${HEAD_SHA}" > "${TEST_TMPDIR}/rewrite-head.sha"
)

REWRITE_BASE="$(cat "${TEST_TMPDIR}/rewrite-base.sha")"
REWRITE_HEAD="$(cat "${TEST_TMPDIR}/rewrite-head.sha")"
assert "anti-rewrite — bootstrap from pre-bootstrap commit when file exists on master → exit 2" \
  "cd '${REWRITE_REPO}' && bash '${SCRIPT_PATH}' --base '${REWRITE_BASE}' --head '${REWRITE_HEAD}'" \
  "2" \
  "anomalous bootstrap window"

echo ""

# ----------------------------------------------------------------------
# 7. Protected-surface list parity with .github/CODEOWNERS.
#
# The script's PROTECTED_SURFACE + PROTECTED_PREFIXES arrays and the
# CODEOWNERS file should enumerate the same set of paths. Drift between
# them creates a gap: either CODEOWNERS protects a file that doesn't get
# cryptographic verification, or the script verifies a file that has no
# human review requirement. Either is a defence-in-depth regression.
#
# Both arrays are unioned for the comparison because CODEOWNERS doesn't
# distinguish exact-match vs prefix-match entries — it just lists path
# globs. The script does distinguish (subtree like
# `.github/actions/verify-wasm-pin-check/` would be tedious to enumerate
# file-by-file), so the parity check unifies both arms.
# ----------------------------------------------------------------------
echo "--- 7. protected-surface parity (script ↔ CODEOWNERS) ---"

PARITY_TMPFILE="$(mktemp -t atlas-parity.XXXXXX)"
{
  # Extract protected paths from the script: lines inside either the
  # PROTECTED_SURFACE=( ... ) or PROTECTED_PREFIXES=( ... ) block that
  # look like quoted paths.
  awk '
    /^PROTECTED_SURFACE=\(/ { in_block = 1; next }
    /^PROTECTED_PREFIXES=\(/ { in_block = 1; next }
    in_block && /^\)/ { in_block = 0; next }
    in_block {
      # Extract content between double quotes.
      if (match($0, /"[^"]+"/)) {
        s = substr($0, RSTART+1, RLENGTH-2)
        print "SCRIPT:" s
      }
    }
  ' "${SCRIPT_PATH}"

  # Extract protected paths from CODEOWNERS: lines starting with `/`,
  # take the first whitespace-separated token, strip leading slash so
  # it matches the script's relative-path form.
  awk '
    /^[[:space:]]*#/ { next }
    /^[[:space:]]*$/ { next }
    /^\// {
      gsub(/^\//, "", $1)
      print "CODEOWNERS:" $1
    }
  ' "${REAL_REPO_ROOT}/.github/CODEOWNERS"
} | sort > "${PARITY_TMPFILE}"

# Pull the two sets and compare.
SCRIPT_PATHS="$(grep '^SCRIPT:' "${PARITY_TMPFILE}" | sed 's/^SCRIPT://' | sort -u)"
CODEOWNERS_PATHS="$(grep '^CODEOWNERS:' "${PARITY_TMPFILE}" | sed 's/^CODEOWNERS://' | sort -u)"
rm -f "${PARITY_TMPFILE}"

if [ "${SCRIPT_PATHS}" = "${CODEOWNERS_PATHS}" ]; then
  echo "  PASS: PROTECTED_SURFACE matches CODEOWNERS ($(printf '%s\n' "${SCRIPT_PATHS}" | wc -l | tr -d ' ') paths)"
  PASS=$((PASS + 1))
else
  echo "  FAIL: PROTECTED_SURFACE drifts from CODEOWNERS"
  echo "        Only in script:"
  comm -23 <(printf '%s\n' "${SCRIPT_PATHS}") <(printf '%s\n' "${CODEOWNERS_PATHS}") | sed 's/^/          /'
  echo "        Only in CODEOWNERS:"
  comm -13 <(printf '%s\n' "${SCRIPT_PATHS}") <(printf '%s\n' "${CODEOWNERS_PATHS}") | sed 's/^/          /'
  FAIL=$((FAIL + 1))
  FAILED_CASES+=("PROTECTED_SURFACE/CODEOWNERS parity")
fi

echo ""
echo "==="
TOTAL=$((PASS + FAIL))
echo "PASS: ${PASS} / ${TOTAL}    FAIL: ${FAIL} / ${TOTAL}"

if [ "${FAIL}" -gt 0 ]; then
  echo "FAILED CASES: ${FAILED_CASES[*]}" >&2
  exit 1
fi
exit 0
