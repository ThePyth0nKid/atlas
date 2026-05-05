#!/usr/bin/env bash
# V1.17 Welle B — Tag-Signing Enforcement verifier.
#
# Asserts that every `v*` tag in this repo is cryptographically signed
# by an SSH key listed in `.github/allowed_signers`. Used by:
#
#   * `.github/workflows/verify-tag-signatures.yml` — push/cron/dispatch
#     full-history re-verification.
#   * `.github/workflows/wasm-publish.yml` — first-step gate before any
#     `npm publish`. Fails closed if the just-pushed tag isn't signed.
#   * Locally — `bash tools/verify-tag-signatures.sh` (no args = all
#     `v*` tags) or `bash tools/verify-tag-signatures.sh v1.17.0` (one).
#
# Why this script lives in the repo (not in the workflow inline):
#   * One verification path, one set of edge cases, one trust root —
#     workflow-inline-bash drifts silently between two callers.
#   * Local-runnable: a maintainer can `bash tools/verify-tag-signatures.sh`
#     before pushing a tag, catching a misconfigured local signing key
#     before the CI red-fail.
#   * Auditable: this file is reviewable as a single artefact.
#
# Inputs:
#   * Args: zero or more tag names. If zero, verifies every `v*` tag in
#     the repo. If one or more, verifies exactly those.
#   * `ATLAS_ALLOWED_SIGNERS` env var (optional): override the default
#     `.github/allowed_signers` path. Tests use this to point at a
#     fixture file.
#
# Outputs:
#   * stdout: per-tag PASS/FAIL line + final summary `PASS: N / TOTAL`.
#   * Exit code: 0 if all tags verify; non-zero if any tag fails.
#
# Threat model (what this script defends, in priority order):
#   1. Repo-takeover / compromised maintainer GitHub PAT — attacker
#      pushes `v*` tag pointing at a smuggled commit. Without the SSH
#      private key matching an entry in `.github/allowed_signers`, the
#      tag cannot be signed and verification fails.
#   2. Force-push of an existing `v*` tag onto a different commit —
#      `git verify-tag` checks the signature against the tag-object's
#      *current* commit SHA. A force-pushed tag has either no signature
#      (rejected) or a signature over the original commit that no
#      longer matches (rejected as invalid).
#   3. CI runner compromise that tries to skip verification — the
#      verification step is the FIRST step in `wasm-publish.yml`, so
#      bypass requires a workflow-file mutation, which itself requires
#      a tag-or-branch push (and branch protection rules cover master).
#
# What this script does NOT defend:
#   * Compromised SSH signing key — once the key is in
#     `.github/allowed_signers`, anyone holding the corresponding
#     private key can sign. Defence-in-depth: keep the signing key
#     in a hardware token (YubiKey FIDO2 / `sk-ssh-ed25519`) and rotate
#     `.github/allowed_signers` on any suspected compromise.
#   * Mutation of `.github/allowed_signers` itself by a compromised
#     maintainer — the trust root's bytes are at the mercy of whoever
#     can commit to master. A future Welle could require commit-signing
#     on changes to this file via a CODEOWNERS + branch-protection rule.

set -euo pipefail

# Resolve repo root regardless of where this script is invoked from.
# Use git's own rev-parse — the only mode where this falls back is on
# a non-git tree, in which case we error out cleanly.
if ! REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  echo "FAIL: not inside a git working tree" >&2
  exit 2
fi
cd "${REPO_ROOT}"

ALLOWED_SIGNERS="${ATLAS_ALLOWED_SIGNERS:-${REPO_ROOT}/.github/allowed_signers}"

# CI containment guard: in GitHub Actions, the trust root MUST be the
# committed `.github/allowed_signers` file. An ATLAS_ALLOWED_SIGNERS
# env-var override in CI is either a misconfiguration or an attack
# (e.g. a future workflow step that sets `ATLAS_ALLOWED_SIGNERS=…` via
# `$GITHUB_ENV` to redirect verification to an attacker-controlled
# fixture, bypassing the gate entirely). Local invocations (where
# `$GITHUB_ACTIONS` is unset) keep the env-var hook for the test
# harness to validate the negative paths.
if [ "${GITHUB_ACTIONS:-}" = "true" ] && [ -n "${ATLAS_ALLOWED_SIGNERS:-}" ]; then
  EXPECTED="$(realpath "${REPO_ROOT}/.github/allowed_signers" 2>/dev/null || echo "")"
  RESOLVED="$(realpath "${ATLAS_ALLOWED_SIGNERS}" 2>/dev/null || echo "")"
  if [ -z "${EXPECTED}" ] || [ "${RESOLVED}" != "${EXPECTED}" ]; then
    echo "FAIL: ATLAS_ALLOWED_SIGNERS=${ATLAS_ALLOWED_SIGNERS} not allowed in CI" >&2
    echo "  In GitHub Actions, the trust root MUST be the committed" >&2
    echo "  ${REPO_ROOT}/.github/allowed_signers file (an env-var override" >&2
    echo "  in CI would be a defence gap — see scope-l for the threat model)." >&2
    exit 2
  fi
fi

if [ ! -f "${ALLOWED_SIGNERS}" ]; then
  echo "FAIL: allowed-signers file missing at ${ALLOWED_SIGNERS}" >&2
  echo "  Tag-signing enforcement requires a populated trust root." >&2
  echo "  See docs/OPERATOR-RUNBOOK.md §13 for the setup flow." >&2
  exit 2
fi

# Empty-or-comment-only file is also a misconfiguration: the trust
# root must contain at least one signer entry. `grep -v -c` returns:
#   * 0 when at least one non-matching line exists (i.e. real keys),
#   * 0 with count `0` when all lines match (comments-only),
#   * exit-1 only on a truly-empty file across some grep variants —
# hence `|| true` to convert the empty-file exit-1 into "0 entries".
KEY_LINE_COUNT="$(grep -v -c -E '^[[:space:]]*(#|$)' "${ALLOWED_SIGNERS}" || true)"
if [ "${KEY_LINE_COUNT}" -lt 1 ]; then
  echo "FAIL: ${ALLOWED_SIGNERS} contains zero signer entries" >&2
  echo "  Add at least one signer via tools/setup-tag-signing.sh add ..." >&2
  exit 2
fi

# Git 2.34+ is required for first-class SSH signing support
# (`gpg.format = ssh` and `gpg.ssh.allowedSignersFile`). Earlier
# versions silently fall through to GPG and the verification step
# would falsely report "tag not signed" against an SSH-signed tag.
GIT_VERSION="$(git --version | awk '{print $3}')"
GIT_MAJOR="$(printf '%s' "${GIT_VERSION}" | awk -F. '{print $1}')"
GIT_MINOR="$(printf '%s' "${GIT_VERSION}" | awk -F. '{print $2}')"
if [ "${GIT_MAJOR}" -lt 2 ] || { [ "${GIT_MAJOR}" -eq 2 ] && [ "${GIT_MINOR}" -lt 34 ]; }; then
  echo "FAIL: git ${GIT_VERSION} too old; SSH tag-signing requires git >= 2.34" >&2
  exit 2
fi

# Collect tags to verify. Args win; otherwise enumerate all `v*` tags.
if [ "$#" -gt 0 ]; then
  TAGS=("$@")
else
  # `git tag -l 'v*'` returns one tag per line; mapfile is bash 4+.
  # Pure-bash fallback via while-read for portability.
  TAGS=()
  while IFS= read -r line; do
    [ -n "${line}" ] && TAGS+=("${line}")
  done < <(git tag -l 'v*')
fi

if [ "${#TAGS[@]}" -eq 0 ]; then
  echo "INFO: no v* tags in this repo yet — nothing to verify."
  echo "  This is expected before the first signed tag is cut."
  echo "  After the first signed tag, this script asserts every"
  echo "  v* tag is signed by a key in ${ALLOWED_SIGNERS}."
  exit 0
fi

PASS=0
FAIL=0
FAILED_TAGS=()

# Single tempfile reused across iterations + a single EXIT trap that
# references it. Earlier loop-internal `trap` accumulated traps that
# all closed over the same shell variable, so a SIGKILL mid-loop would
# leak every prior iteration's tempfile. Pre-allocating once + reusing
# is simpler AND torn-down-on-signal-correctly.
ERR_LOG="$(mktemp -t atlas-verify-tag-err.XXXXXX)"
trap 'rm -f "${ERR_LOG}"' EXIT INT TERM

# `git verify-tag` invokes `ssh-keygen -Y verify` under the hood when
# the tag is SSH-signed. We pass the allowed-signers file via `-c
# gpg.ssh.allowedSignersFile=<path>` so the verification doesn't depend
# on any per-clone git config — the script is self-contained.
#
# `git verify-tag` exits:
#   * 0 — tag has a signature AND the signature verifies against a key
#         in the allowed-signers file.
#   * non-zero — tag is unsigned OR signature is invalid OR signing
#         key is not in the allowed-signers file. We treat all three
#         as failure (the threat model doesn't distinguish between
#         "no signature" and "untrusted-key signature").
#
# stderr from `git verify-tag` is captured to a per-tag log so the
# diagnostic surfaces in CI logs without spamming stdout. We re-emit
# the relevant tail on FAIL.
echo "Verifying ${#TAGS[@]} tag(s) against ${ALLOWED_SIGNERS} ..."
echo "---"
for tag in "${TAGS[@]}"; do
  # Existence check — `git verify-tag` on a non-existent tag prints a
  # confusing "fatal: bad object" rather than our crisp diagnostic.
  if ! git rev-parse --verify --quiet "refs/tags/${tag}" >/dev/null 2>&1; then
    echo "  FAIL: ${tag} (tag does not exist in this repo)"
    FAIL=$((FAIL + 1))
    FAILED_TAGS+=("${tag}")
    continue
  fi

  # Lightweight-tag pre-check. A lightweight tag is a ref pointing
  # directly at a commit object, with no tag object in between — and
  # therefore no signature surface. `git verify-tag` rejects these with
  # the cryptic "cannot verify a non-tag object of type commit". We
  # pre-detect via `git cat-file -t` and emit an actionable diagnostic.
  #
  # Three real-world causes:
  #   1. Maintainer used `git tag <name>` instead of `git tag -s <name>`.
  #      The tag was never annotated and therefore never signed.
  #   2. CI: `actions/checkout@v4` with `fetch-tags: true` under
  #      partial-clone fetches the tag REF but not the annotated tag
  #      OBJECT. The local ref then resolves to the commit. Workflow
  #      fix: add `git fetch --tags --force origin` after checkout so
  #      the tag objects are repopulated. (V1.17 Welle B v1.17.0
  #      tag-cut #1 hit exactly this — run 25394785761.)
  #   3. INCIDENT: a previously-annotated v* tag has been replaced with
  #      a lightweight ref pointing at a different (or same) commit.
  #      Operator's first action: `git fetch --tags --force origin` AND
  #      `git for-each-ref --format="%(refname) %(objecttype)" refs/tags/v*`
  #      to confirm types match expectations. If a remote-side tag has
  #      genuinely been overwritten, treat as a potential trust-root
  #      compromise and consult `docs/SECURITY-NOTES.md` scope-l.
  REF_TYPE="$(git cat-file -t "refs/tags/${tag}" 2>/dev/null || echo "")"
  if [ "${REF_TYPE}" != "tag" ]; then
    echo "  FAIL: ${tag} (lightweight tag — no tag object, no signature)"
    echo "        ref type: ${REF_TYPE:-<unknown>} (annotated tag would be 'tag')"
    echo "        Cause 1 (local): tag created with 'git tag <name>'"
    echo "          instead of 'git tag -s <name>'."
    echo "        Cause 2 (CI): actions/checkout fetched the ref but not"
    echo "          the tag object. Add 'git fetch --tags --force origin'"
    echo "          after checkout to repopulate annotated tag objects."
    echo "        Cause 3 (incident): a remote-side annotated tag has"
    echo "          been overwritten with a lightweight ref. If the local"
    echo "          fetch + reverify still shows commit-typed ref, treat"
    echo "          as potential trust-root compromise — see scope-l."
    FAIL=$((FAIL + 1))
    FAILED_TAGS+=("${tag}")
    continue
  fi

  # Reuse the pre-allocated ERR_LOG; truncate per iteration. The EXIT
  # trap (set above the loop) handles cleanup on any exit path.
  : > "${ERR_LOG}"
  if git \
      -c "gpg.format=ssh" \
      -c "gpg.ssh.allowedSignersFile=${ALLOWED_SIGNERS}" \
      verify-tag "${tag}" >/dev/null 2>"${ERR_LOG}"; then
    echo "  PASS: ${tag}"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: ${tag}"
    # Indent stderr lines so they're visually grouped under the FAIL.
    sed 's/^/        /' < "${ERR_LOG}"
    FAIL=$((FAIL + 1))
    FAILED_TAGS+=("${tag}")
  fi
done

echo "---"
TOTAL=$((PASS + FAIL))
echo "PASS: ${PASS} / ${TOTAL}    FAIL: ${FAIL} / ${TOTAL}"

if [ "${FAIL}" -gt 0 ]; then
  echo "FAILED TAGS: ${FAILED_TAGS[*]}" >&2
  echo "" >&2
  echo "Each failed tag is either unsigned, signed by an SSH key not in" >&2
  echo "${ALLOWED_SIGNERS}, or has an invalid signature." >&2
  echo "See docs/SECURITY-NOTES.md scope-l for the threat model." >&2
  exit 1
fi

exit 0
