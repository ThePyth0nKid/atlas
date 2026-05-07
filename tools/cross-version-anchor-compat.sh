#!/usr/bin/env bash
# V1.18 Welle B (8) — cross-version anchor compatibility verifier.
#
# Asserts that the WORKING-TREE pin in
# `crates/atlas-trust-core/src/anchor.rs` (the proposed post-rotation
# trust root) still verifies every Sigstore Rekor v1 anchor that was
# captured under PRIOR-VERSION pins. A pin update that breaks
# acceptance of a prior-version anchor would silently invalidate the
# verifier's historical replay property — every bundle Atlas ever
# produced becomes unverifiable on the next consumer-side replay.
#
# This script is the load-bearing pre-merge gate referenced by
# `docs/OPERATOR-RUNBOOK.md` §15 (Inline-pin-update protocol, step 5
# and PR review item "Cross-version-anchor compat") and
# `docs/ADR/ADR-Atlas-006-multi-issuer-sigstore-tracking.md` §8.3
# (open question: pinned-Rekor-v1 verifier ↔ multi-issuer-only consumer
# interop). Pre-shipping it makes the next pin rotation structurally
# safer than the prior manual substitute (run cargo test by hand
# against the fixture corpus before §15 step 4 overwrites it).
#
# Mechanics:
#   1. Enumerate the legacy fixture corpus from a git ref
#      (default `origin/master`) — the fixtures that were captured
#      under the PRIOR pin and that historical Atlas bundles index
#      against. The corpus lives in
#      `crates/atlas-trust-core/tests/fixtures/sigstore_rekor_v1_logindex_*.json`.
#      We pull from git history (not on-disk) because §15 step 4
#      explicitly OVERWRITES the on-disk canonical fixture with a
#      post-rotation capture before this script runs — the prior
#      fixture only survives in git.
#   2. Save the on-disk canonical fixture path to a tempfile so the
#      EXIT trap can restore it if cargo test crashes mid-run.
#   3. For each legacy fixture: materialise it from the git ref into
#      the canonical on-disk path, then run
#      `cargo test -p atlas-trust-core --test sigstore_golden` against
#      it. The test compiles + runs against the WORKING-TREE
#      `anchor.rs` (the proposed post-rotation pin), so a PASS proves
#      the new pin still accepts the legacy anchor.
#   4. Every legacy fixture must PASS for the script to PASS. A single
#      FAIL means the proposed rotation breaks at least one
#      prior-version anchor — STOP, do not ship the rotation, see
#      OPERATOR-RUNBOOK §15 cross-version-anchor failure-mode table.
#
# When this script is run:
#   * During §15 step 5 of an actual Sigstore Rekor v1 pin rotation
#     PR. The operator has just edited `anchor.rs` (step 2) and
#     regenerated the canonical fixture to a post-rotation capture
#     (step 4). This script then verifies the new pin against the
#     PRIOR fixture corpus from `origin/master` (or whichever ref
#     the rotation branched from).
#   * Optionally on every PR that touches `crates/atlas-trust-core/src/
#     anchor.rs` as a no-op gate (passes if there's no rotation, fails
#     loud the moment a rotation tries to ship). Wired up via a
#     workflow follow-on when the first rotation lands.
#   * Locally as part of a maintainer's pre-push checklist on a
#     rotation branch.
#
# Inputs:
#   --legacy-ref <ref>   The "prior pin" git ref. The script enumerates
#                        the fixture corpus AT THIS REF. Default:
#                        $ATLAS_LEGACY_FIXTURE_REF if set (LOCAL-ONLY
#                        — the CI containment guard rejects this
#                        env-var when $GITHUB_ACTIONS=true), else
#                        `origin/master`.
#   --cargo <path>       Path to the cargo binary. Default:
#                        $CARGO if set, else `cargo` from PATH. On
#                        Windows hosts the cargo binary is typically
#                        at `/c/Users/<user>/.cargo/bin/cargo.exe` and
#                        not in the default bash PATH; setting CARGO
#                        explicitly keeps the script portable.
#   -h, --help           Print this header and exit 0.
#
# Outputs:
#   * stdout: per-fixture PASS/FAIL line + final summary.
#   * Exit code:
#       0 — every legacy fixture verified under the working-tree pin.
#       1 — at least one legacy fixture failed verification (rotation
#           breaks cross-version compat — STOP).
#       2 — misconfiguration (bad ref, missing canonical fixture, no
#           legacy fixtures found, cargo not callable, env-var
#           override attempted under GitHub Actions).
#
# Threat model addressed:
#   A maintainer (or an attacker who compromised maintainer credentials)
#   ships a Sigstore PEM rotation that swaps the pinned key without
#   preserving the prior key as a second `RekorIssuer`. The post-
#   rotation `anchor.rs` correctly verifies the post-rotation fixture
#   (caught by the standard `sigstore_golden` test in §15 step 6), but
#   silently rejects every Sigstore-anchored bundle Atlas produced
#   before the rotation. Without this gate, the regression is invisible
#   until a downstream auditor tries to replay a historical bundle
#   weeks later. With this gate, the regression is caught pre-merge and
#   the operator is forced to confront the multi-key-issuer design
#   question (ADR-006 §8.3) before the rotation ships.
#
# What this script does NOT defend (out of scope):
#   * A rotation that adds a SECOND issuer correctly but introduces a
#     bug in the second issuer's pin (e.g. wrong PEM, wrong tree-IDs).
#     Caught by the standard `sigstore_golden` test in §15 step 6 once
#     a fixture for the new issuer is captured, AND by
#     `rekor_issuer_rosters_are_pinned`.
#   * A rotation whose `legacy-ref` was force-pushed to remove the
#     prior fixture from history. Defended by the ruleset's
#     `non_fast_forward` rule (operator-side) — see OPERATOR-RUNBOOK
#     §16. If `git ls-tree` against the supplied ref returns no
#     fixtures the script exits 2; an attacker rewriting history to
#     hide the corpus produces a misconfiguration signal, not a
#     silent pass.
#   * A rotation where the legacy fixture itself has been forged
#     (operator commits an attacker-supplied "prior fixture" that
#     verifies under the new pin). Defended by:
#     `crates/atlas-trust-core/tests/fixtures/` being in
#     `PROTECTED_PREFIXES` (Welle B (6) — every fixture mutation
#     requires an SSH-signed commit) and by reviewer scrutiny of
#     `source` + `fetched_at_unix` provenance fields (OPERATOR-RUNBOOK
#     §15 PR review item "Fixture freshness").
#   * A malicious `--cargo` argument or `CARGO` env-var pointing at a
#     wrapper that lies about test results (always exits 0). For LOCAL
#     invocations this is the operator's own trust boundary — same
#     class as a tampered editor or shell. For CI invocations the
#     `GITHUB_ACTIONS` containment guard below blocks the `CARGO`
#     env-var path; the `--cargo` arg path requires a workflow-step
#     compromise that already trivially defeats most CI defences.
#     Documented as accepted residual risk.
#
# Test-suite scoping (load-bearing — read before changing):
#   The cargo invocation runs the `sigstore_golden` test binary with
#   `-- --skip fixture_log_id_matches_pinned`. The skipped test asserts
#   that the legacy fixture's `log_id` matches the working-tree pin's
#   `SIGSTORE_REKOR_V1_LOG_ID` — which by construction always fails on
#   a PEM rotation, even one that correctly preserves the prior key as
#   a second `RekorIssuer` (the multi-key path from ADR-006 §8.3). The
#   property `fixture_log_id_matches_pinned` checks (today-pin matches
#   today-fixture identity) is exercised by `cargo test --test
#   sigstore_golden` in OPERATOR-RUNBOOK §15 step 6 against the
#   POST-rotation fixture, not by this gate against the LEGACY corpus.
#   The remaining five tests
#   (`verifies_real_sigstore_rekor_entry`,
#    `tampered_entry_body_is_rejected`,
#    `anchored_hash_forgery_is_rejected`,
#    `unknown_tree_id_is_rejected`,
#    `historical_shard_tree_id_passes_dispatch_gate`)
#   are all useful under a legacy fixture: the positive verification
#   case is the load-bearing cross-version compat property, and the
#   four negative cases prove the verifier's defences (anti-forgery,
#   roster gate, signature dispatch) still trigger correctly under the
#   new pin. A future PR adding a new test to this binary should
#   consider whether it belongs in the cross-version run; if not,
#   add it to the `--skip` list.

set -euo pipefail

# ----------------------------------------------------------------------
# Repo-root resolution (mirror of verify-trust-root-mutations.sh).
# ----------------------------------------------------------------------
if ! REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  echo "FAIL: not inside a git working tree" >&2
  exit 2
fi
cd "${REPO_ROOT}"

# ----------------------------------------------------------------------
# Defaults.
# ----------------------------------------------------------------------
LEGACY_REF="${ATLAS_LEGACY_FIXTURE_REF:-origin/master}"
CARGO_BIN="${CARGO:-cargo}"

# Canonical on-disk fixture path. The `sigstore_golden` test currently
# hard-codes a single fixture path; we materialise each legacy fixture
# into this exact path so the test compiles + runs unchanged. If the
# test gains parametric fixture-loading in the future, this script
# should be updated to set the appropriate env-var rather than swap
# files in place.
FIXTURE_DIR="crates/atlas-trust-core/tests/fixtures"
CANONICAL_FIXTURE="${FIXTURE_DIR}/sigstore_rekor_v1_logindex_800000000.json"
# Pattern (regex, not glob) used to filter the git ls-tree output to
# Sigstore Rekor v1 fixtures only. Mirrors the on-disk naming
# convention `sigstore_rekor_v1_logindex_<N>.json` used by §15 step 4.
LEGACY_FIXTURE_PATTERN='sigstore_rekor_v1_logindex_[0-9]+\.json$'

# ----------------------------------------------------------------------
# CI containment: in GitHub Actions, block env-var overrides AND
# constrain the post-arg-parse value of LEGACY_REF to a tight allow-
# list of trusted ref shapes. Mirror of verify-trust-root-mutations.sh
# ATLAS_TRUST_ROOT_BASE guard, extended to also cover the arg-path
# attack vector (sec H-2): blocking only the env-var would leave a
# compromised workflow step free to inject `--legacy-ref <attacker-
# ref>` pointing at a crafted fixture corpus that verifies under an
# attacker key. Local invocations keep the env-var + arg paths for
# maintainer convenience.
#
# Allowlist shapes (all defensively narrow):
#   * `origin/<branch>` — remote-tracking refs, settable only by
#     `git fetch` from the workflow's checkout step.
#   * `refs/tags/v<...>` — release tags, signed by an allowed_signer
#     under V1.17 Welle B.
#   * 40-char hex SHA — workflow-derived from a controlled property
#     such as `github.event.pull_request.base.sha`.
# ----------------------------------------------------------------------
if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
  if [ -n "${ATLAS_LEGACY_FIXTURE_REF:-}" ]; then
    echo "FAIL: ATLAS_LEGACY_FIXTURE_REF env-var override not allowed in CI" >&2
    echo "  In GitHub Actions, --legacy-ref MUST come from an explicit arg" >&2
    echo "  set by the workflow from a controlled source (e.g." >&2
    echo "  github.event.pull_request.base.sha). An env-var override would" >&2
    echo "  let a future workflow step redirect verification to an" >&2
    echo "  attacker-controlled fixture corpus." >&2
    exit 2
  fi
  if [ -n "${CARGO:-}" ]; then
    echo "FAIL: CARGO env-var override not allowed in CI" >&2
    echo "  In GitHub Actions, --cargo must be set explicitly by the" >&2
    echo "  workflow OR cargo must be resolved from the default PATH." >&2
    echo "  An env-var override would let a future workflow step redirect" >&2
    echo "  verification to an attacker-controlled wrapper that always" >&2
    echo "  reports test PASS — silently nulling the rotation gate." >&2
    exit 2
  fi
fi

# ----------------------------------------------------------------------
# Argument parsing. --legacy-ref / --cargo override env-var defaults.
# ----------------------------------------------------------------------
while [ "$#" -gt 0 ]; do
  case "$1" in
    --legacy-ref)
      [ "$#" -ge 2 ] || { echo "FAIL: --legacy-ref requires a ref argument" >&2; exit 2; }
      LEGACY_REF="$2"
      shift 2
      ;;
    --cargo)
      [ "$#" -ge 2 ] || { echo "FAIL: --cargo requires a path argument" >&2; exit 2; }
      CARGO_BIN="$2"
      shift 2
      ;;
    -h|--help)
      sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    *)
      echo "FAIL: unknown argument: $1" >&2
      echo "Usage: $0 [--legacy-ref <ref>] [--cargo <path>]" >&2
      exit 2
      ;;
  esac
done

# ----------------------------------------------------------------------
# CI ref-shape allowlist (continuation of the CI containment block
# above — applied AFTER arg parsing so it covers both the env-var
# default and the --legacy-ref arg path). See sec H-2 rationale in the
# block above. Bash regex match keeps the patterns inline and readable.
# ----------------------------------------------------------------------
if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
  if ! [[ "${LEGACY_REF}" =~ ^(origin/[A-Za-z0-9._/-]+|[0-9a-f]{40}|refs/tags/v[A-Za-z0-9._/-]+)$ ]]; then
    echo "FAIL: --legacy-ref '${LEGACY_REF}' is not in the CI allowlist" >&2
    echo "  Allowed in CI: 'origin/<branch>', a 40-char commit SHA, or" >&2
    echo "  'refs/tags/v<...>'. The narrow allowlist prevents a compromised" >&2
    echo "  workflow step from redirecting verification to an attacker-" >&2
    echo "  controlled corpus (e.g. via 'refs/pull/<n>/head')." >&2
    exit 2
  fi
fi

# ----------------------------------------------------------------------
# Validate cargo is callable. We resolve `command -v` rather than just
# trusting the PATH lookup at invocation time so the failure mode is a
# clear early-exit-2 ("cargo not callable") instead of a confusing
# cargo-test failure mid-loop.
# ----------------------------------------------------------------------
if ! command -v "${CARGO_BIN}" >/dev/null 2>&1; then
  echo "FAIL: cargo binary '${CARGO_BIN}' not callable" >&2
  echo "  Set --cargo <path> or the CARGO env-var. On Windows hosts the" >&2
  echo "  binary is typically at /c/Users/<user>/.cargo/bin/cargo.exe" >&2
  echo "  and not in the default bash PATH." >&2
  exit 2
fi

# ----------------------------------------------------------------------
# Validate the legacy ref resolves. A typo here would otherwise produce
# an empty fixture list and a confusing "no legacy fixtures found"
# message downstream — better to fail fast with the actual cause.
# ----------------------------------------------------------------------
if ! git rev-parse --verify "${LEGACY_REF}^{commit}" >/dev/null 2>&1; then
  echo "FAIL: legacy ref '${LEGACY_REF}' does not resolve to a commit in this repo" >&2
  echo "  If the ref names a remote branch (e.g. origin/master), try:" >&2
  echo "    git fetch origin master" >&2
  echo "  before re-running this script." >&2
  exit 2
fi

# ----------------------------------------------------------------------
# Validate the canonical on-disk fixture exists. The script's
# save-and-restore swap pattern requires SOMETHING at the canonical
# path so the EXIT trap can restore it; if the operator deleted it
# manually before running the script, refuse to proceed.
# ----------------------------------------------------------------------
if [ ! -f "${CANONICAL_FIXTURE}" ]; then
  echo "FAIL: canonical fixture '${CANONICAL_FIXTURE}' not present on disk" >&2
  echo "  The script swaps legacy fixtures into this path and restores" >&2
  echo "  the original on exit. Without an original to restore, the script" >&2
  echo "  refuses to run. Restore the file (e.g. via git checkout) before" >&2
  echo "  re-running." >&2
  exit 2
fi

# ----------------------------------------------------------------------
# Save the on-disk canonical fixture so the EXIT trap can restore it
# even if cargo test crashes, the operator hits Ctrl-C, or a bash error
# trips set -e mid-loop.
#
# Order is load-bearing: populate `SAVED_FIXTURE` BEFORE installing the
# trap. `mktemp` creates a 0-byte file; if SIGINT arrived between
# `trap` and the initial `cp`, the trap would copy 0 bytes back over
# `CANONICAL_FIXTURE` and silently truncate it (code review M-1).
# Saving first guarantees `SAVED_FIXTURE` is non-empty whenever the
# trap fires from a real interrupt; the `[ -s ]` guard in the trap
# additionally ensures we never restore from an empty save (covers
# the race window between `mktemp` and the `cp` if a future refactor
# accidentally re-orders the lines).
#
# `CARGO_LOG` is also created here (once, reused per iteration) so the
# trap can clean it up on SIGKILL/OOM during a long cargo run (sec
# H-1). Allocating per-iteration tempfiles inside the loop would leak
# them on hard interrupt.
# ----------------------------------------------------------------------
SAVED_FIXTURE="$(mktemp -t atlas-canonical-fixture.XXXXXX)"
CARGO_LOG="$(mktemp -t atlas-cargo-test.XXXXXX)"
cp -f "${CANONICAL_FIXTURE}" "${SAVED_FIXTURE}"
trap '
  if [ -s "${SAVED_FIXTURE}" ]; then
    if ! cp -f "${SAVED_FIXTURE}" "${CANONICAL_FIXTURE}" 2>&1; then
      echo "WARN: failed to restore ${CANONICAL_FIXTURE} from saved copy;" >&2
      echo "  working tree may be dirty. Recover with:" >&2
      echo "    git checkout -- ${CANONICAL_FIXTURE}" >&2
    fi
  fi
  rm -f "${SAVED_FIXTURE}" "${CARGO_LOG}"
' EXIT INT TERM

# ----------------------------------------------------------------------
# Enumerate the legacy fixture corpus from the git ref. We use
# `git ls-tree -r --name-only -z` (NUL-separated) rather than reading
# on-disk because §15 step 4 explicitly overwrites the on-disk fixture
# with a post-rotation capture before this script runs — the prior
# fixture only survives in git history. NUL separation hardens the
# enumeration against (currently-impossible-by-naming-convention but
# defence-in-depth) filenames containing literal newlines.
#
# We capture stderr separately so a real git failure (shallow clone
# missing the tree object, packfile corruption) surfaces with the
# actual cause instead of the misleading "no fixtures found" exit-2
# message a swallowed error would produce (code review M-3).
#
# The pattern filter restricts to Sigstore Rekor v1 fixtures (the
# `sigstore_rekor_v1_logindex_<N>.json` naming convention from
# OPERATOR-RUNBOOK §15 step 4) so unrelated future fixtures (e.g. for
# Sigstore Rekor v2 in ADR-006 §6) don't accidentally get fed into the
# v1 sigstore_golden test harness. When that day comes, an analogous
# v2 cross-version-anchor-compat script will live alongside this one.
# Match is via bash `[[ =~ ]]` which uses ERE — same dialect as the
# original `grep -E`.
# ----------------------------------------------------------------------
# Stream `git ls-tree -z` output through a tmpfile rather than `$(...)`
# capture: bash command substitution strips embedded NUL bytes (the
# `-z` separator), which would collapse the entire output into a single
# string and break enumeration. The tmpfile preserves NULs exactly.
LS_TREE_OUT="$(mktemp -t atlas-ls-tree-out.XXXXXX)"
LS_TREE_ERR="$(mktemp -t atlas-ls-tree-err.XXXXXX)"
if ! git ls-tree -z -r --name-only "${LEGACY_REF}" -- "${FIXTURE_DIR}" \
     >"${LS_TREE_OUT}" 2>"${LS_TREE_ERR}"; then
  echo "FAIL: git ls-tree failed for ref '${LEGACY_REF}'" >&2
  sed 's/^/  /' < "${LS_TREE_ERR}" >&2
  echo "  Possible causes: shallow clone missing the tree object," >&2
  echo "  packfile corruption, or the fixture directory does not exist" >&2
  echo "  at this ref." >&2
  rm -f "${LS_TREE_OUT}" "${LS_TREE_ERR}"
  exit 2
fi
rm -f "${LS_TREE_ERR}"

# Parse NUL-separated output into a deterministic list of matched
# fixture paths. We avoid the prior printf|grep|count pipeline because
# `grep -c .` over-counts if a path contains a newline (sec M-4) — the
# explicit array build below is correctness-preserving even under the
# (currently impossible) embedded-newline case.
LEGACY_FIXTURES=()
while IFS= read -r -d '' candidate; do
  [ -z "${candidate}" ] && continue
  if [[ "${candidate}" =~ ${LEGACY_FIXTURE_PATTERN} ]]; then
    LEGACY_FIXTURES+=("${candidate}")
  fi
done < "${LS_TREE_OUT}"
rm -f "${LS_TREE_OUT}"

LEGACY_COUNT="${#LEGACY_FIXTURES[@]}"

if [ "${LEGACY_COUNT}" -eq 0 ]; then
  echo "FAIL: no Sigstore Rekor v1 legacy fixtures found at ${LEGACY_REF}" >&2
  echo "  Looked under '${FIXTURE_DIR}' for files matching" >&2
  echo "  '${LEGACY_FIXTURE_PATTERN}'." >&2
  echo "  This is misconfiguration — the cross-version compat property" >&2
  echo "  cannot be checked against an empty prior corpus. Verify the" >&2
  echo "  legacy ref is correct (--legacy-ref <ref>) and that the" >&2
  echo "  fixture corpus exists at that ref." >&2
  exit 2
fi

# ----------------------------------------------------------------------
# Per-fixture verification loop.
#
# For each legacy fixture:
#   1. Materialise via `git show <ref>:<path>` into the canonical
#      on-disk path. The test reads from the canonical path
#      unconditionally; swapping the file content is the simplest way
#      to avoid modifying the test source (which is itself in
#      PROTECTED_SURFACE — modifying it would broaden this PR's
#      surface change unnecessarily).
#   2. Run `cargo test -p atlas-trust-core --test sigstore_golden`.
#      This compiles against the WORKING-TREE anchor.rs (the proposed
#      post-rotation pin) and reads the fixture from the canonical
#      on-disk path (the legacy fixture we just materialised). PASS
#      means the new pin still accepts the legacy anchor.
#   3. Tally PASS/FAIL.
#
# We use `set +e`/`set -e` brackets around the cargo invocation so a
# fixture failure tallies cleanly instead of triggering the EXIT trap
# mid-loop. The trap still restores on real interrupts (Ctrl-C, kill).
# ----------------------------------------------------------------------
echo "Cross-version anchor compatibility check"
echo "  Legacy ref:       ${LEGACY_REF}"
echo "  Legacy fixtures:  ${LEGACY_COUNT}"
echo "  Working-tree pin: crates/atlas-trust-core/src/anchor.rs (HEAD)"
echo "---"

PASS=0
FAIL=0
FAILED_FIXTURES=()

for legacy_path in "${LEGACY_FIXTURES[@]}"; do
  [ -z "${legacy_path}" ] && continue

  # Materialise the legacy fixture into the canonical path. Capture
  # stderr separately so a real git failure (sparse-checkout pruned the
  # path, packfile corruption, ref drifted post-enumeration) surfaces
  # the actual cause under the FAIL line instead of the generic "could
  # not materialise" message a swallowed error would produce (sec L-2).
  GIT_SHOW_ERR="$(mktemp -t atlas-git-show-err.XXXXXX)"
  if ! git show "${LEGACY_REF}:${legacy_path}" > "${CANONICAL_FIXTURE}" 2>"${GIT_SHOW_ERR}"; then
    echo "  FAIL: ${legacy_path} (could not materialise from ${LEGACY_REF})"
    sed 's/^/        /' < "${GIT_SHOW_ERR}"
    rm -f "${GIT_SHOW_ERR}"
    FAIL=$((FAIL + 1))
    FAILED_FIXTURES+=("${legacy_path}")
    continue
  fi
  rm -f "${GIT_SHOW_ERR}"

  # Run cargo test against the working-tree anchor.rs + materialised
  # legacy fixture. We skip `fixture_log_id_matches_pinned` because that
  # test by construction always FAILs on a PEM rotation — even one that
  # correctly preserves the prior key as a second RekorIssuer (ADR-006
  # §8.3). Including it would produce false-positive FAILs that block
  # every legitimate multi-key rotation. See the "Test-suite scoping"
  # block in the header for the full rationale (CR-H1).
  #
  # CARGO_LOG is the script-global tempfile (allocated alongside
  # SAVED_FIXTURE before the trap install) — reused per iteration via
  # truncate-on-redirect so a SIGKILL during cargo doesn't leak a
  # per-iteration tempfile (sec H-1).
  set +e
  "${CARGO_BIN}" test -p atlas-trust-core --test sigstore_golden \
    -- --skip fixture_log_id_matches_pinned \
    >"${CARGO_LOG}" 2>&1
  CARGO_RC=$?
  set -e

  if [ "${CARGO_RC}" -eq 0 ]; then
    echo "  PASS: ${legacy_path}"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: ${legacy_path}"
    # Indent the cargo log under the FAIL line so the failure cause is
    # visible without re-running. Limit to the test result block to
    # keep the summary short.
    sed -n '/^running [0-9]\+ tests/,/^test result:/p' "${CARGO_LOG}" \
      | sed 's/^/        /'
    FAIL=$((FAIL + 1))
    FAILED_FIXTURES+=("${legacy_path}")
  fi
done

echo "---"
TOTAL=$((PASS + FAIL))
echo "PASS: ${PASS} / ${TOTAL}    FAIL: ${FAIL} / ${TOTAL}"

if [ "${FAIL}" -gt 0 ]; then
  echo "FAILED FIXTURES: ${FAILED_FIXTURES[*]}" >&2
  echo "" >&2
  echo "At least one prior-version Sigstore Rekor v1 anchor no longer" >&2
  echo "verifies under the working-tree pin. The proposed rotation" >&2
  echo "breaks cross-version anchor compatibility — historical bundles" >&2
  echo "that were verifiable under the prior pin would silently fail" >&2
  echo "to verify post-merge." >&2
  echo "" >&2
  echo "Resolution paths:" >&2
  echo "  * Confirm the failure mode in OPERATOR-RUNBOOK §15" >&2
  echo "    \"Cross-version-anchor compatibility test\" failure-mode" >&2
  echo "    table (PEM rotation without multi-key support, dropped" >&2
  echo "    historical shard, or accidental issuer-name rotation)." >&2
  echo "  * If the rotation requires changing the ACTIVE pin, design" >&2
  echo "    the multi-key-issuer extension (ADR-006 §5.2 / §8.3) so" >&2
  echo "    the prior key is preserved as a second RekorIssuer entry." >&2
  echo "  * Do NOT --no-verify around this gate or weaken it to" >&2
  echo "    accept partial PASS — every prior fixture must verify or" >&2
  echo "    the rotation breaks consumer-side replay." >&2
  exit 1
fi

echo "OK: every prior-version Sigstore Rekor v1 anchor verifies under the" \
     "working-tree pin."
exit 0
