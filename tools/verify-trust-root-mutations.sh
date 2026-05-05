#!/usr/bin/env bash
# V1.17 Welle C — Trust-Root Mutation Defence verifier.
#
# Asserts that every commit between a base ref and a head ref that
# modifies any file in the trust-root protected surface is signed by an
# SSH key listed in `.github/allowed_signers` AS IT EXISTED AT THE BASE
# COMMIT. The "old trust root" semantic is the entire point: it closes
# the bootstrap-attack vector where a malicious commit adds an
# attacker-controlled key to the trust root in the same commit that
# changes (or relies on) the trust root.
#
# Used by:
#   * `.github/workflows/verify-trust-root-mutations.yml` — PR-triggered
#     enforcement gate. The workflow runs from the BASE branch's
#     workflow file (a `pull_request` event property), so an attacker
#     modifying this script or the workflow inside their PR cannot
#     change which logic runs against their PR.
#   * Locally — a maintainer can `bash tools/verify-trust-root-mutations.sh
#     --base origin/master --head HEAD` before pushing a PR branch to
#     catch a mis-signing locally before the CI red-fail.
#
# Why this defence exists (the threat addressed):
#   Welle B verifies `v*` tags against `.github/allowed_signers`. If an
#   attacker can mutate `.github/allowed_signers` itself — e.g. via a
#   compromised maintainer GitHub PAT pushing a commit that adds an
#   attacker-controlled key — then verify-tag becomes ceremonial:
#     1. attacker pushes commit modifying allowed_signers (adds their key),
#     2. attacker creates `v*` tag signed with their key,
#     3. verify-tag-signatures.yml runs against the NEW allowed_signers,
#     4. attacker's key is now "trusted" — verification passes,
#     5. `wasm-publish.yml` first-step gate ALSO uses NEW allowed_signers,
#     6. publish fires; smuggled bytes ship to npm.
#
#   This script breaks step (1): the commit modifying allowed_signers
#   must itself be signed by a key in the OLD allowed_signers (i.e. the
#   trust root as it existed at the PR base). The attacker would need
#   to already hold a private key matching an entry in the in-repo trust
#   root — which is the V1.15-residual hard-key-compromise threat,
#   defended by hardware tokens + key rotation, not by this script.
#
# Scope: protected-surface defined below. Non-surface files are
# unaffected. A PR that doesn't touch the surface no-ops in <1s.
#
# Inputs (preferred form: explicit args):
#   --base <ref>   The "old trust root" ref. Default: $ATLAS_TRUST_ROOT_BASE
#                  if set (LOCAL-ONLY — the CI containment guard rejects
#                  this env-var when $GITHUB_ACTIONS=true), else
#                  origin/master.
#   --head <ref>   The "new trust root" ref. Default:
#                  $ATLAS_TRUST_ROOT_HEAD if set (LOCAL-ONLY), else HEAD.
#
# Outputs:
#   * stdout: per-commit PASS/FAIL line + final summary.
#   * Exit code: 0 if all surface-modifying commits verify (or none
#     exist); 1 if any commit fails verification; 2 on misconfiguration
#     (bad ref, missing trust root, etc.).
#
# What this script does NOT defend (out-of-scope):
#   * Direct push to master — branch protection ("require PR before
#     merge to master") is the operator-side complement to this
#     defence. Without that branch-protection rule, a compromised PAT
#     can `git push origin master` and bypass the PR-triggered gate
#     entirely. See docs/OPERATOR-RUNBOOK.md §14.
#   * Web-UI commits (GitHub.com "Edit this file" + "Commit changes").
#     GitHub signs web-UI commits with its own GPG key, which is not in
#     `.github/allowed_signers` (and the format is GPG, not SSH).
#     Maintainers must make trust-root changes via local commit.
#   * Squash-merge or rebase-merge that produces a new GitHub-signed
#     commit at merge time. Use "merge commit" or operator-rebases-locally
#     for trust-root surface PRs. Documented.
#   * Hard private-key compromise — once the attacker holds a private
#     key matching an entry in `.github/allowed_signers`, this script
#     cannot tell them apart from the legitimate maintainer. Hardware
#     tokens + key rotation are the V1.15-residual defence-in-depth.

set -euo pipefail

# ----------------------------------------------------------------------
# Protected surface — files that require commit-signature verification
# against the merge-base trust root.
#
# Adding to this list is a tightening operation (more commits get
# verified). Removing from it is a loosening operation and should be
# treated as a security review. Any change to this list requires a
# trust-root-signed commit (because this script itself is on the list).
#
# Two match modes:
#   * PROTECTED_SURFACE       — exact full-path match (grep -Fx).
#   * PROTECTED_PREFIXES      — directory-prefix match. Used for
#     subtrees where every file underneath is in the trust chain
#     (composite action, anti-drift harnesses sourced into the same
#     trust story).
#
# Order: alphabetical, so a new entry has a deterministic insertion
# point and the diff is small.
# ----------------------------------------------------------------------
PROTECTED_SURFACE=(
  ".github/CODEOWNERS"
  ".github/allowed_signers"
  ".github/workflows/verify-tag-signatures.yml"
  ".github/workflows/verify-trust-root-mutations.yml"
  ".github/workflows/wasm-publish.yml"
  "tools/setup-tag-signing.sh"
  "tools/test-tag-signatures.sh"
  "tools/test-trust-root-mutations.sh"
  "tools/verify-tag-signatures.sh"
  "tools/verify-trust-root-mutations.sh"
)

# Subtree prefixes — every file under one of these counts as
# protected-surface. The composite action `.github/actions/verify-
# wasm-pin-check/` is downstream-consumer-facing (not in the npm
# publish path), but tampering with its scripts (e.g. neutering
# `check-provenance.sh` to silently exit 0) silently revokes the
# Layer-3 trust guarantee for every downstream consumer running the
# action. Gating modifications closes that gap.
PROTECTED_PREFIXES=(
  ".github/actions/verify-wasm-pin-check/"
)

# ----------------------------------------------------------------------
# Repo-root resolution (mirrors verify-tag-signatures.sh).
# ----------------------------------------------------------------------
if ! REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  echo "FAIL: not inside a git working tree" >&2
  exit 2
fi
cd "${REPO_ROOT}"

# ----------------------------------------------------------------------
# Default base/head.
# ----------------------------------------------------------------------
BASE="${ATLAS_TRUST_ROOT_BASE:-origin/master}"
HEAD_REF="${ATLAS_TRUST_ROOT_HEAD:-HEAD}"

# ----------------------------------------------------------------------
# CI containment: reject env-var override of base/head in GitHub Actions.
# Local invocations keep the env-var hooks for the test harness's
# negative-path coverage. Mirror of verify-tag-signatures.sh's
# ATLAS_ALLOWED_SIGNERS guard.
# ----------------------------------------------------------------------
if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
  if [ -n "${ATLAS_TRUST_ROOT_BASE:-}" ] || [ -n "${ATLAS_TRUST_ROOT_HEAD:-}" ]; then
    echo "FAIL: ATLAS_TRUST_ROOT_BASE/HEAD env-var override not allowed in CI" >&2
    echo "  In GitHub Actions, base + head MUST come from explicit --base / --head" >&2
    echo "  args set by the workflow from \${{ github.event.pull_request.base.sha }}" >&2
    echo "  and \${{ github.event.pull_request.head.sha }}. An env-var override would" >&2
    echo "  let a future workflow step redirect verification to attacker-controlled" >&2
    echo "  refs (see scope-l for the threat model)." >&2
    exit 2
  fi
fi

# ----------------------------------------------------------------------
# Argument parsing. --base / --head override the env-var defaults.
# ----------------------------------------------------------------------
while [ "$#" -gt 0 ]; do
  case "$1" in
    --base)
      [ "$#" -ge 2 ] || { echo "FAIL: --base requires a ref argument" >&2; exit 2; }
      BASE="$2"
      shift 2
      ;;
    --head)
      [ "$#" -ge 2 ] || { echo "FAIL: --head requires a ref argument" >&2; exit 2; }
      HEAD_REF="$2"
      shift 2
      ;;
    -h|--help)
      sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    *)
      echo "FAIL: unknown argument: $1" >&2
      echo "Usage: $0 [--base <ref>] [--head <ref>]" >&2
      exit 2
      ;;
  esac
done

# ----------------------------------------------------------------------
# Validate refs resolve to commits.
# ----------------------------------------------------------------------
if ! BASE_SHA="$(git rev-parse --verify "${BASE}^{commit}" 2>/dev/null)"; then
  echo "FAIL: base ref '${BASE}' does not resolve to a commit in this repo" >&2
  exit 2
fi
if ! HEAD_SHA="$(git rev-parse --verify "${HEAD_REF}^{commit}" 2>/dev/null)"; then
  echo "FAIL: head ref '${HEAD_REF}' does not resolve to a commit in this repo" >&2
  exit 2
fi

# ----------------------------------------------------------------------
# Compute merge-base. This is the "fork point" of the PR branch from
# the base branch — the last commit both refs share. Trust-root verify
# happens against THIS commit's allowed_signers, not the tip of the
# base branch (which could itself have been mutated since the PR was
# branched). For a typical fast-forward PR, merge-base == base tip;
# for a long-running PR rebased onto current master, merge-base is the
# latest shared commit.
# ----------------------------------------------------------------------
if ! MERGE_BASE="$(git merge-base "${BASE_SHA}" "${HEAD_SHA}" 2>/dev/null)"; then
  echo "FAIL: cannot compute merge-base of '${BASE}' and '${HEAD_REF}'" >&2
  echo "  Are the two refs from unrelated histories? (a recently-imported" >&2
  echo "  fork or a force-pushed branch can produce this.)" >&2
  exit 2
fi

# ----------------------------------------------------------------------
# Git ≥ 2.34 required for first-class SSH signing support
# (`gpg.format = ssh` and `gpg.ssh.allowedSignersFile`). Earlier
# versions silently fall through to GPG and verify-commit would falsely
# report "commit not signed" against an SSH-signed commit. Mirror of
# the guard in `tools/verify-tag-signatures.sh`.
# ----------------------------------------------------------------------
GIT_VERSION="$(git --version | awk '{print $3}')"
GIT_MAJOR="$(printf '%s' "${GIT_VERSION}" | awk -F. '{print $1}')"
GIT_MINOR="$(printf '%s' "${GIT_VERSION}" | awk -F. '{print $2}')"
if [ "${GIT_MAJOR}" -lt 2 ] || { [ "${GIT_MAJOR}" -eq 2 ] && [ "${GIT_MINOR}" -lt 34 ]; }; then
  echo "FAIL: git ${GIT_VERSION} too old; SSH commit-signing requires git >= 2.34" >&2
  exit 2
fi

# ----------------------------------------------------------------------
# Bootstrap mode: if .github/allowed_signers did not exist at merge-base,
# there is no trust root to verify against. This is the first-time-
# adding-the-file path (the V1.17 Welle B initial-ship PR). Accept
# unconditionally — but ONLY if no commit anywhere in reachable history
# from BASE_SHA has ever introduced the file. Otherwise an admin force-
# push of master back to a pre-bootstrap commit could manufacture a
# fresh bootstrap window (H-2 from V1.17 Welle C parallel review): an
# attacker would force-push master to a commit before allowed_signers
# was added, then open a PR adding their attacker-controlled key as
# "the bootstrap" → bootstrap mode passes → trust root replaced.
#
# The defence: `git log --diff-filter=A --all --format=%H -- .github/
# allowed_signers` enumerates every commit that has ever ADDED the
# file. If at least one such commit exists AND it is reachable from
# BASE_SHA (i.e. the file was on master's history at some point), then
# bootstrap mode at this merge-base is anomalous — fail closed.
# ----------------------------------------------------------------------
if ! git show "${MERGE_BASE}:.github/allowed_signers" >/dev/null 2>&1; then
  # Anti-rewrite check: enumerate every commit anywhere in the repo
  # that ADDED .github/allowed_signers. If any of them is an ancestor
  # of BASE_SHA, the file existed on master's history line — bootstrap
  # is anomalous.
  HISTORICAL_ADDS="$(git log --diff-filter=A --all --format=%H -- .github/allowed_signers 2>/dev/null || true)"
  if [ -n "${HISTORICAL_ADDS}" ]; then
    while IFS= read -r add_commit; do
      [ -z "${add_commit}" ] && continue
      if git merge-base --is-ancestor "${add_commit}" "${BASE_SHA}" 2>/dev/null; then
        echo "FAIL: bootstrap mode triggered, but .github/allowed_signers exists on" >&2
        echo "  the base branch's history (added at ${add_commit:0:12}, ancestor of" >&2
        echo "  base ${BASE_SHA:0:12}). This is an anomalous bootstrap window —" >&2
        echo "  typically caused by an admin force-push of master to a pre-bootstrap" >&2
        echo "  commit. Refusing to accept unsigned trust-root mutations." >&2
        echo "" >&2
        echo "  See docs/OPERATOR-RUNBOOK.md §14: 'Allow force pushes' must be" >&2
        echo "  disabled on the master branch (including for administrators)." >&2
        exit 2
      fi
    done <<< "${HISTORICAL_ADDS}"
  fi
  echo "INFO: .github/allowed_signers did not exist at merge-base ${MERGE_BASE:0:12}."
  echo "  Bootstrap mode — accepting all changes (no trust root to verify against)."
  echo "  Once .github/allowed_signers exists at any merge-base, every subsequent"
  echo "  PR is gated by this script."
  exit 0
fi

# ----------------------------------------------------------------------
# Materialise the merge-base trust root into a tempfile so we can pass
# it to `git -c gpg.ssh.allowedSignersFile=…`. The tempfile lives until
# the EXIT trap runs. The trap also covers SIGINT/SIGTERM so an
# interrupted run leaves no `/tmp/atlas-old-trust-root.*` debris.
# ----------------------------------------------------------------------
OLD_TRUST_ROOT="$(mktemp -t atlas-old-trust-root.XXXXXX)"
ERR_LOG="$(mktemp -t atlas-verify-commit-err.XXXXXX)"
trap 'rm -f "${OLD_TRUST_ROOT}" "${ERR_LOG}"' EXIT INT TERM

git show "${MERGE_BASE}:.github/allowed_signers" > "${OLD_TRUST_ROOT}"

# Sanity: a populated trust root at merge-base. An empty / comments-only
# allowed_signers at the merge-base is a misconfiguration — there is no
# valid pre-mutation trust root to verify against.
KEY_COUNT="$(grep -v -c -E '^[[:space:]]*(#|$)' "${OLD_TRUST_ROOT}" || true)"
if [ "${KEY_COUNT}" -lt 1 ]; then
  echo "FAIL: .github/allowed_signers at merge-base ${MERGE_BASE:0:12} has zero signer entries" >&2
  echo "  Cannot verify against an empty trust root. Restore at least one signer entry" >&2
  echo "  in the merge-base commit before opening a PR that touches the trust-root surface." >&2
  exit 2
fi

# ----------------------------------------------------------------------
# Find all commits in (merge-base, head] that modify any protected file.
#
# `git rev-list <range>` returns one SHA per line. For each, we use
# `git diff-tree --no-commit-id -r --name-only -m` (NOT `git show
# --name-only`) — `git show` on a merge commit emits no file list by
# default, which would let a merge commit silently slip past surface
# matching. `diff-tree -m` enumerates files relative to each parent so
# merges are correctly inspected.
#
# Surface match has two modes:
#   * Exact full-line match against PROTECTED_SURFACE via `grep -Fx`
#     (defends against regex metachars in paths + substring false-
#     positives like `tools/setup-tag-signing.sh.bak`).
#   * Prefix match against PROTECTED_PREFIXES via case-glob (covers
#     subtrees like the verify-wasm-pin-check composite action).
#
# Hard-fail on diff-tree error — earlier `2>/dev/null || true` would
# silently treat a `diff-tree`-failure as zero-files-changed, which is
# a silent-pass code path against an attacker-controlled state (e.g.
# corrupted object). Treat any diff-tree failure as misconfiguration.
# ----------------------------------------------------------------------
COMMITS_TO_VERIFY=()
while IFS= read -r commit; do
  [ -z "${commit}" ] && continue
  if ! CHANGED_FILES="$(git diff-tree --no-commit-id -r --name-only -m "${commit}")"; then
    echo "FAIL: could not enumerate changed files for commit ${commit:0:12}" >&2
    echo "  diff-tree returned non-zero — repository corruption or shallow clone?" >&2
    exit 2
  fi
  matched=0
  # Exact-match arm.
  for protected in "${PROTECTED_SURFACE[@]}"; do
    if printf '%s\n' "${CHANGED_FILES}" | grep -qFx "${protected}"; then
      COMMITS_TO_VERIFY+=("${commit}")
      matched=1
      break
    fi
  done
  # Prefix-match arm (only checked if no exact match found).
  if [ "${matched}" -eq 0 ]; then
    while IFS= read -r changed_file; do
      [ -z "${changed_file}" ] && continue
      for prefix in "${PROTECTED_PREFIXES[@]}"; do
        case "${changed_file}" in
          "${prefix}"*)
            COMMITS_TO_VERIFY+=("${commit}")
            matched=1
            break 2
            ;;
        esac
      done
    done <<< "${CHANGED_FILES}"
  fi
done < <(git rev-list "${MERGE_BASE}..${HEAD_SHA}")

if [ "${#COMMITS_TO_VERIFY[@]}" -eq 0 ]; then
  echo "INFO: no commits in ${BASE_SHA:0:12}..${HEAD_SHA:0:12} modify the trust-root surface."
  echo "  Trust-root mutation defence has no commits to verify; passing."
  exit 0
fi

# ----------------------------------------------------------------------
# Per-commit verification loop.
#
# `git verify-commit` exits:
#   * 0 — commit has a signature AND the signature verifies against a
#         key in the allowed-signers file.
#   * non-zero — unsigned, or invalid sig, or untrusted-key sig.
# We treat all three as failure (threat model doesn't distinguish).
# ----------------------------------------------------------------------
echo "Verifying ${#COMMITS_TO_VERIFY[@]} commit(s) against trust root at merge-base ${MERGE_BASE:0:12}"
echo "  (.github/allowed_signers at merge-base: ${KEY_COUNT} signer entries)"
echo "---"

PASS=0
FAIL=0
FAILED_COMMITS=()

for commit in "${COMMITS_TO_VERIFY[@]}"; do
  : > "${ERR_LOG}"
  SHORT="${commit:0:12}"
  SUBJECT="$(git log -1 --format=%s "${commit}" 2>/dev/null || echo '<unknown>')"
  if git \
      -c "gpg.format=ssh" \
      -c "gpg.ssh.allowedSignersFile=${OLD_TRUST_ROOT}" \
      verify-commit "${commit}" >/dev/null 2>"${ERR_LOG}"; then
    echo "  PASS: ${SHORT}  ${SUBJECT}"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: ${SHORT}  ${SUBJECT}"
    sed 's/^/        /' < "${ERR_LOG}"
    FAIL=$((FAIL + 1))
    FAILED_COMMITS+=("${SHORT}")
  fi
done

echo "---"
TOTAL=$((PASS + FAIL))
echo "PASS: ${PASS} / ${TOTAL}    FAIL: ${FAIL} / ${TOTAL}"

if [ "${FAIL}" -gt 0 ]; then
  echo "FAILED COMMITS: ${FAILED_COMMITS[*]}" >&2
  echo "" >&2
  echo "Each failed commit modifies a trust-root-protected file but is" >&2
  echo "either unsigned, signed by an SSH key not in .github/allowed_signers" >&2
  echo "as it existed at merge-base ${MERGE_BASE:0:12}, or has an invalid" >&2
  echo "signature." >&2
  echo "" >&2
  echo "Resolution paths (for legitimate maintainer changes):" >&2
  echo "  * Configure SSH commit signing locally: bash tools/setup-tag-signing.sh init" >&2
  echo "  * Re-create the commit signed: git rebase --exec 'git commit --amend --no-edit -S' ${MERGE_BASE:0:12}" >&2
  echo "  * Force-push the rebased branch to the PR." >&2
  echo "" >&2
  echo "See docs/SECURITY-NOTES.md scope-l for the full threat model." >&2
  exit 1
fi

exit 0
