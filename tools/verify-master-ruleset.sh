#!/usr/bin/env bash
# V1.18 Welle B (5) — Repository-Ruleset state verifier.
#
# Operator-side counterpart to V1.17 Welle C's
# `tools/verify-trust-root-mutations.sh`. The repo-side script gates
# *commits* touching PROTECTED_SURFACE files; this script gates
# *operator-side configuration drift* on the GitHub Repository
# Ruleset that BINDS the in-repo gate as a required status check.
#
# Why this exists:
#   The Welle C in-repo defence ONLY binds because the Master Ruleset
#   pins three load-bearing properties:
#     1. `enforcement: active`
#     2. `bypass_actors: []` (no admin-bypass)
#     3. `required_status_checks` includes
#        `"Verify trust-root-modifying commits"`
#     4. `pull_request.require_code_owner_review: true`
#     5. `required_signatures` rule present
#     6. `non_fast_forward` + `deletion` rules present (no force-push,
#        no branch deletion)
#   If any of these silently regress (operator misclick, manual API
#   PATCH, ruleset replaced, ruleset disabled "just for a moment and
#   forgotten"), the in-repo gate stops being load-bearing without any
#   commit landing in the repo. There would be no in-repo signal that
#   defence-in-depth has been weakened.
#
#   This script is the in-repo signal: a daily-cron + post-push +
#   on-demand check that fires red the moment the live Ruleset
#   diverges from the pinned canonical form in
#   `tools/expected-master-ruleset.json`.
#
# What it does:
#   1. Look up the "Master trust-root protection" Ruleset by name on
#      ${ATLAS_OWNER}/${ATLAS_REPO}. (Lookup-by-name, not by hardcoded
#      id, so the verifier survives a delete-and-recreate cycle —
#      which would mint a new id but is functionally indistinguishable
#      from the operator's perspective if the rule shape is identical.)
#   2. Fetch the full Ruleset definition.
#   3. Normalise: strip volatile per-instance fields (`id`, `node_id`,
#      `source`, `created_at`, `updated_at`, `_links`, and the
#      viewer-dependent `current_user_can_bypass`); sort the `rules`
#      array by `type`; sort the `allowed_merge_methods` array.
#   4. Diff against the pinned canonical form. Identical → exit 0.
#      Different → exit 1, print the unified diff.
#
# Exit codes:
#   0 — live Ruleset matches the pin (defence is intact).
#   1 — drift detected (Ruleset weakened, missing rule, or shape
#       changed). Resolution path is in OPERATOR-RUNBOOK.md §16.
#   2 — lookup error (Ruleset by that name does not exist; missing
#       jq/gh tooling; auth failure). The Ruleset being absent is
#       itself a critical defence-down state, but it's reported as
#       exit 2 to distinguish "configuration drifted" from "I cannot
#       even tell".
#
# Trust posture: this script reads operator-side configuration via
# the GitHub REST API. The script itself is in PROTECTED_SURFACE
# (see `tools/verify-trust-root-mutations.sh`), so any modification
# requires an SSH-signed commit by an allowed_signer + CODEOWNERS
# review. Tampering with the verifier to make it pass falsely is
# itself gated.

set -euo pipefail

# ----------------------------------------------------------------------
# Configuration. Defaults match the production Atlas repo. Override via
# env vars for local testing against forks or staging clones.
#
# `EXPECTED_RULESET_ID` is the production Ruleset's numeric id, pinned
# inline as a defence against the name-collision attack:
#
#   An attacker who already compromised the operator's GitHub session
#   has admin scope. They could create a SECOND Ruleset with exactly
#   the name "Master trust-root protection" but with weakened
#   parameters, then DELETE the original. Lookup-by-name alone would
#   resolve to the attacker's Ruleset; if the attacker carefully
#   crafted its shape to match the pinned canonical, the verifier
#   exits 0 while the real defence has been replaced.
#
#   Pinning the id separately closes that hole: any id mismatch fires
#   exit 1 with a clear "ID drifted — re-pin if this was intentional"
#   message. The legitimate "delete-and-recreate" recovery path
#   requires the operator to update this constant in the SAME commit
#   that re-pins the canonical JSON — both are PROTECTED_SURFACE,
#   so both edits demand an SSH-signed commit + CODEOWNERS review.
#
# Reported as security finding H-1 in the V1.18 Welle B (5) review.
# ----------------------------------------------------------------------
OWNER="${ATLAS_OWNER:-ThePyth0nKid}"
REPO="${ATLAS_REPO:-atlas}"
RULESET_NAME="Master trust-root protection"
EXPECTED_RULESET_ID="${ATLAS_EXPECTED_RULESET_ID:-15986324}"

# ----------------------------------------------------------------------
# CI containment guard. Mirrors the pattern in
# `tools/verify-trust-root-mutations.sh`: when running under
# GitHub Actions, reject any ATLAS_OWNER / ATLAS_REPO /
# ATLAS_EXPECTED_RULESET_ID env-var values that do not match the
# GitHub-set canonical values. Closes the "malicious earlier
# workflow step exports ATLAS_OWNER=attacker" attack reported as
# security finding H-2 in the V1.18 Welle B (5) review:
#
#   A compromised composite action / supply-chain attack on a
#   transitive dependency could export ATLAS_OWNER=attacker /
#   ATLAS_REPO=attacker-controlled-fork before this step runs. The
#   attacker's fork could host a Ruleset matching the pinned shape
#   exactly. Without containment, the verifier would fetch from the
#   attacker's repo, exit 0, while the real Ruleset has been
#   silently disabled.
#
# GITHUB_REPOSITORY is set by the runner from the workflow's
# `github.repository` context — not injectable by workflow steps
# (env: at the step level cannot override env vars set by the
# runner before the step executes; only the step's `env:` block
# can, and that block is part of the trusted workflow file).
# ----------------------------------------------------------------------
if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
  if [ -z "${GITHUB_REPOSITORY:-}" ]; then
    echo "FAIL: GITHUB_ACTIONS=true but GITHUB_REPOSITORY is unset — refusing to proceed" >&2
    exit 2
  fi
  EXPECTED_OWNER="${GITHUB_REPOSITORY%%/*}"
  EXPECTED_REPO="${GITHUB_REPOSITORY##*/}"
  if [ "${OWNER}" != "${EXPECTED_OWNER}" ] || [ "${REPO}" != "${EXPECTED_REPO}" ]; then
    echo "FAIL: env-var override of OWNER/REPO not allowed in CI" >&2
    echo "      runtime resolved OWNER=${OWNER} REPO=${REPO}" >&2
    echo "      GITHUB_REPOSITORY=${GITHUB_REPOSITORY} requires OWNER=${EXPECTED_OWNER} REPO=${EXPECTED_REPO}" >&2
    exit 2
  fi
fi

# ----------------------------------------------------------------------
# Resolve repo root + expected-pin path.
# ----------------------------------------------------------------------
if ! REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  echo "FAIL: not inside a git working tree" >&2
  exit 2
fi
EXPECTED="${REPO_ROOT}/tools/expected-master-ruleset.json"

if [ ! -f "${EXPECTED}" ]; then
  echo "FAIL: pinned expected file not found: ${EXPECTED}" >&2
  echo "      The Welle B (5) verifier requires a canonical pin to" >&2
  echo "      compare against. Restore it from a known-good revision." >&2
  exit 2
fi

# ----------------------------------------------------------------------
# Tooling check.
# ----------------------------------------------------------------------
if ! command -v jq >/dev/null 2>&1; then
  echo "FAIL: jq is required (used for JSON normalisation)" >&2
  exit 2
fi
if ! command -v gh >/dev/null 2>&1; then
  echo "FAIL: gh CLI is required (used for GitHub REST API access)" >&2
  echo "      In CI: ensure 'gh' is installed on the runner image." >&2
  echo "      Locally: see https://cli.github.com/" >&2
  exit 2
fi
if ! command -v diff >/dev/null 2>&1; then
  echo "FAIL: diff is required" >&2
  exit 2
fi

# ----------------------------------------------------------------------
# 1. List all rulesets on the repo, find the one by name → ruleset id.
#
# We list-then-pick rather than hardcode the id. Rationale: a
# legitimate operator action (e.g. accidental delete + recreate of the
# Ruleset) mints a new id but produces a functionally-identical
# configuration. Lookup-by-name lets that recovery path succeed
# without requiring a verifier code-edit. (The id IS surfaced in the
# diagnostic output for traceability.)
#
# `gh api` automatically picks up GH_TOKEN / GITHUB_TOKEN from env. In
# the CI workflow this is the workflow's GITHUB_TOKEN with the
# `administration: read` permission requested.
# ----------------------------------------------------------------------
RULESETS_JSON="$(
  gh api -H "Accept: application/vnd.github+json" \
    "repos/${OWNER}/${REPO}/rulesets" 2>/dev/null
)" || {
  echo "FAIL: could not list rulesets on ${OWNER}/${REPO}" >&2
  echo "      Likely causes:" >&2
  echo "        * No 'administration: read' permission on the token." >&2
  echo "        * In CI: workflow missing 'permissions: administration: read'." >&2
  echo "        * Locally: 'gh auth status' shows insufficient scopes." >&2
  exit 2
}

RULESET_ID="$(
  printf '%s' "${RULESETS_JSON}" \
    | jq -r --arg name "${RULESET_NAME}" \
        '.[] | select(.name == $name) | .id' \
    | head -n 1
)"

if [ -z "${RULESET_ID}" ] || [ "${RULESET_ID}" = "null" ]; then
  echo "FAIL: no Repository Ruleset named '${RULESET_NAME}' found on ${OWNER}/${REPO}" >&2
  echo "" >&2
  echo "      This is a CRITICAL defence-down state: the Welle C in-repo" >&2
  echo "      trust-root-mutation gate is unbound without this Ruleset" >&2
  echo "      pinning it as a required status check on master." >&2
  echo "" >&2
  echo "      Recovery: see docs/OPERATOR-RUNBOOK.md §16 → 'Recovering from" >&2
  echo "      exit 2 — Ruleset not found' for the recreate-from-pin recipe." >&2
  exit 2
fi

# ----------------------------------------------------------------------
# 1a. Defence against name-collision attack (security H-1).
#
# Lookup-by-name found a Ruleset, but is it THE Ruleset? Compare
# resolved id against EXPECTED_RULESET_ID. Mismatch is one of:
#   * Operator deliberately deleted + recreated the Ruleset (legitimate
#     recovery path) — re-pin EXPECTED_RULESET_ID in this script in the
#     same SSH-signed commit that re-pins the canonical JSON if any
#     shape changes accompanied the recreation.
#   * Attacker created a second Ruleset with the same name and weakened
#     parameters, then deleted (or planned to delete) the original.
#     Without this guard, lookup-by-name would resolve to the
#     attacker's Ruleset and the diff would proceed against the wrong
#     target.
# Either way, mismatch is a high-signal event that requires operator
# attention before the verifier's exit code can be trusted.
# ----------------------------------------------------------------------
if [ "${RULESET_ID}" != "${EXPECTED_RULESET_ID}" ]; then
  echo "FAIL: Ruleset id drift detected" >&2
  echo "      pinned EXPECTED_RULESET_ID = ${EXPECTED_RULESET_ID}" >&2
  echo "      live ruleset id            = ${RULESET_ID}" >&2
  echo "" >&2
  echo "      The Ruleset named '${RULESET_NAME}' resolves to a different" >&2
  echo "      id than the pin. This is one of:" >&2
  echo "        (a) Legitimate operator-side delete-and-recreate — re-pin" >&2
  echo "            EXPECTED_RULESET_ID in tools/verify-master-ruleset.sh in" >&2
  echo "            the same SSH-signed commit that re-pins the canonical" >&2
  echo "            JSON, then re-run." >&2
  echo "        (b) Attacker created a second Ruleset with the same name" >&2
  echo "            and is preparing to delete the original (or already" >&2
  echo "            has). Treat as compromise — see docs/OPERATOR-RUNBOOK.md" >&2
  echo "            §16 → 'Suspected compromise' for the audit-log + rotation" >&2
  echo "            playbook." >&2
  exit 1
fi

# ----------------------------------------------------------------------
# 2. Fetch full Ruleset details.
# ----------------------------------------------------------------------
ACTUAL_RAW="$(
  gh api -H "Accept: application/vnd.github+json" \
    "repos/${OWNER}/${REPO}/rulesets/${RULESET_ID}" 2>/dev/null
)" || {
  echo "FAIL: could not fetch details for ruleset id=${RULESET_ID}" >&2
  exit 2
}

# ----------------------------------------------------------------------
# 3. Normalise.
#
# *** MIRROR WARNING ***
# This jq pipeline is duplicated as prose in
# docs/OPERATOR-RUNBOOK.md §16 → "Re-pinning after a legitimate
# Ruleset change". If you change the pipeline here, change it
# there in the same commit. Future improvement: factor into a
# `--dump-pin` mode of this script and have the runbook invoke
# the script instead of duplicating the pipeline. (Reported as
# code-review finding M-2 in the V1.18 Welle B (5) review.)
# *** END MIRROR WARNING ***
#
# Strip volatile fields:
#   * id, node_id           — repo/instance-specific
#   * source                — "owner/repo" string, varies by clone target
#   * created_at, updated_at — timestamps drift on every operator edit,
#                              even no-op re-saves; not security-relevant
#   * _links                — HATEOAS URLs; derived
#   * current_user_can_bypass — VIEWER-dependent (depends on the
#                              token's permissions); pinning it would
#                              cause spurious diffs when the verifier
#                              runs under different token contexts
#   * .rules[]
#       .parameters
#       .required_status_checks[].integration_id
#                              — installation-specific (set when the
#                              check was created via a GitHub App
#                              context). Prophylactic strip — current
#                              live response does not include it,
#                              but GitHub has historically added
#                              fields like this silently. Reported
#                              as code-review finding M-1 in the
#                              V1.18 Welle B (5) review.
#
# Sort:
#   * `rules` array, by `type` — a future reorder in the API response
#     should not trigger a false positive
#   * `allowed_merge_methods` (string array) — same rationale
#   * `required_status_checks` array, by `context` — same rationale.
#     Currently a singleton, but pinning to a stable order is
#     cheap insurance against the future case where Atlas adds a
#     second required check (e.g. tag-signing). Reported as
#     code-review finding HIGH-1 in the V1.18 Welle B (5) review.
#
# `jq -S` then sorts top-level + nested object keys alphabetically.
#
# Note: `required_reviewers` (an empty array today inside
# `pull_request.parameters`) is intentionally NOT sorted. If/when
# Atlas adds reviewers, the order of reviewer ids is itself a
# deliberate operator choice (priority order) and a re-order WOULD
# be a meaningful drift signal. Keep as-is until that's no longer
# true.
# ----------------------------------------------------------------------
ACTUAL_NORMALISED="$(
  printf '%s' "${ACTUAL_RAW}" | jq -S '
    del(
      .id,
      .node_id,
      .source,
      .created_at,
      .updated_at,
      ._links,
      .current_user_can_bypass
    )
    | .rules |= (
        sort_by(.type)
        | map(
            if (.parameters? | type) == "object" then
              .parameters |= (
                (if (.allowed_merge_methods? | type) == "array" then
                  .allowed_merge_methods |= sort
                else . end)
                | (if (.required_status_checks? | type) == "array" then
                  .required_status_checks |= (
                    map(del(.integration_id?))
                    | sort_by(.context)
                  )
                else . end)
              )
            else . end
          )
      )
  '
)"

EXPECTED_NORMALISED="$(jq -S '.' "${EXPECTED}")"

# ----------------------------------------------------------------------
# 4. Diff.
# ----------------------------------------------------------------------
TMP_ACTUAL="$(mktemp -t atlas-ruleset-actual.XXXXXX)"
TMP_EXPECTED="$(mktemp -t atlas-ruleset-expected.XXXXXX)"
trap 'rm -f "${TMP_ACTUAL}" "${TMP_EXPECTED}"' EXIT INT TERM

printf '%s\n' "${ACTUAL_NORMALISED}"   > "${TMP_ACTUAL}"
printf '%s\n' "${EXPECTED_NORMALISED}" > "${TMP_EXPECTED}"

if diff -u "${TMP_EXPECTED}" "${TMP_ACTUAL}" >/dev/null 2>&1; then
  echo "PASS: '${RULESET_NAME}' (id=${RULESET_ID}) on ${OWNER}/${REPO} matches pinned configuration"
  echo "      The Welle C in-repo trust-root-mutation gate is correctly bound."
  exit 0
fi

echo "FAIL: '${RULESET_NAME}' (id=${RULESET_ID}) on ${OWNER}/${REPO} drifts from pinned configuration"
echo ""
echo "Unified diff (- expected = pinned, + actual = live):"
echo ""
diff -u \
  --label "tools/expected-master-ruleset.json" \
  --label "live ruleset (id=${RULESET_ID})" \
  "${TMP_EXPECTED}" "${TMP_ACTUAL}" || true
echo ""
echo "Resolution: see docs/OPERATOR-RUNBOOK.md §16."
echo "  * If the drift is unintended: revert the Ruleset to the pinned"
echo "    state via Settings → Rules → Rulesets → '${RULESET_NAME}'."
echo "  * If the drift is intended (e.g. promoted to multi-maintainer,"
echo "    raised approval count to 1): re-pin tools/expected-master-ruleset.json"
echo "    in an SSH-signed commit, then re-run this verifier."
exit 1
