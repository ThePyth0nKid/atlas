#!/usr/bin/env bash
#
# tools/playground-csp-check.sh
#
# V1.16 Welle A + Welle B — anti-drift validator for the wasm-playground
# browser hardening posture.
#
# What this script enforces (CI mode, default):
#   1. apps/wasm-playground/index.html ships a Content-Security-Policy
#      meta-tag with the V1.16 Welle A required directives intact.
#   2. The CSP does NOT regress to 'unsafe-inline' or 'unsafe-eval' on
#      script-src (only 'wasm-unsafe-eval' is permitted, which is the
#      dedicated WebAssembly compilation keyword).
#   3. Trusted Types enforcement is on (`require-trusted-types-for 'script'`
#      AND `trusted-types 'none'`).
#   4. The <script type="module" src="app.js" integrity="sha384-...">
#      SRI hash matches the actual sha384 of apps/wasm-playground/app.js.
#   5. (V1.16 Welle B) The CSP declares a `report-uri` for violation
#      reporting, and that URI is same-origin (a relative path or starts
#      with /). A cross-origin reporting endpoint is not a hard fail —
#      it just downgrades report fidelity (browsers send opaque-response
#      reports cross-origin) and creates a new vendor dependency. We WARN.
#
# Operator workflow:
#   - After editing app.js, run with --update-sri to refresh the hash.
#   - In CI, run without flags; non-zero exit means drift.
#
# Cross-platform: bash + openssl + sed + grep + base64 (Git Bash on
# Windows works; macOS/Linux native works).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
INDEX_HTML="${REPO_ROOT}/apps/wasm-playground/index.html"
APP_JS="${REPO_ROOT}/apps/wasm-playground/app.js"

UPDATE_SRI=0
for arg in "$@"; do
  case "$arg" in
    --update-sri) UPDATE_SRI=1 ;;
    -h|--help)
      sed -n '3,30p' "$0"
      exit 0
      ;;
    *)
      echo "unknown flag: $arg" >&2
      echo "usage: $0 [--update-sri]" >&2
      exit 2
      ;;
  esac
done

# Sanity: required files exist.
for f in "$INDEX_HTML" "$APP_JS"; do
  if [ ! -f "$f" ]; then
    echo "FAIL: missing file: $f" >&2
    exit 1
  fi
done

# --- Compute actual sha384 of app.js -----------------------------------
ACTUAL_HASH_B64=$(openssl dgst -sha384 -binary "$APP_JS" | openssl base64 -A)
ACTUAL_INTEGRITY="sha384-${ACTUAL_HASH_B64}"

# --- --update-sri path -------------------------------------------------
if [ "$UPDATE_SRI" -eq 1 ]; then
  # Pre-flight: the <script> tag must already declare an integrity= attr
  # before we can refresh it. Without this guard, the awk sub() below
  # silently no-ops (sub() returns 0 matches), the file is unchanged,
  # cmp -s reports identical, and the script exits 0 with a misleading
  # "no change — SRI already matches" message — a false-positive PASS
  # on a tag-without-integrity. Fixed: assert presence first, refuse
  # to "update" what isn't there. (Code-reviewer M2 fix.)
  if ! grep -E 'src="app\.js"[^>]*integrity="sha(256|384|512)-' "$INDEX_HTML" >/dev/null; then
    echo "FAIL: --update-sri requires an existing integrity=\"sha{256,384,512}-...\" attribute on the app.js <script> tag" >&2
    echo "  found: $(grep -E 'src="app\.js"' "$INDEX_HTML" || echo '(no app.js script tag)')" >&2
    echo "  fix:   add integrity=\"sha384-PLACEHOLDER\" crossorigin=\"anonymous\" to the tag, then re-run --update-sri" >&2
    exit 1
  fi

  # Replace the integrity= attribute on the app.js <script> tag.
  # Match: integrity="sha384-...."  on the line containing src="app.js"
  # The regex is anchored on src="app.js" to avoid surprising matches if
  # a future edit adds other <script integrity="..."> tags. The base64
  # character class [A-Za-z0-9+\/=] is RFC 4648 §4 standard alphabet;
  # the `=` matches padding which only appears at end of base64 strings
  # but the `+` quantifier accepting it mid-string is harmless because
  # invalid base64 won't appear in the input.
  #
  # mktemp in the SAME directory as INDEX_HTML so the final `mv` is
  # atomic on the same filesystem (rename(2)). Using $TMPDIR (the default)
  # could land on a tmpfs / different filesystem, where `mv` falls back to
  # copy-then-unlink — opens a TOCTOU window where a symlinked attacker
  # path could replace the destination. Same-dir mktemp closes that.
  # (security-reviewer L4 fix.)
  TMP=$(mktemp "${INDEX_HTML}.tmp.XXXXXX")
  awk -v new="$ACTUAL_INTEGRITY" '
    /src="app\.js"/ {
      sub(/integrity="sha(256|384|512)-[A-Za-z0-9+\/=]+"/, "integrity=\"" new "\"")
    }
    { print }
  ' "$INDEX_HTML" > "$TMP"

  # Post-flight: confirm the new integrity= is actually present in the
  # output file before overwriting. Belt-and-braces against any awk-side
  # surprise (e.g., if mktemp returned a path on a tmpfs that filled up,
  # awk would write a partial file; cmp would still be useful but an
  # explicit assertion is safer).
  if ! grep -F "integrity=\"$ACTUAL_INTEGRITY\"" "$TMP" >/dev/null; then
    echo "FAIL: post-update sanity check — new integrity attribute not found in regenerated file" >&2
    rm -f "$TMP"
    exit 1
  fi

  if cmp -s "$TMP" "$INDEX_HTML"; then
    echo "no change — SRI already matches: $ACTUAL_INTEGRITY"
    rm -f "$TMP"
    exit 0
  fi

  mv "$TMP" "$INDEX_HTML"
  echo "updated index.html SRI to: $ACTUAL_INTEGRITY"
  exit 0
fi

# --- Validation path (default — CI mode) -------------------------------

FAIL=0

fail() {
  echo "FAIL: $*" >&2
  FAIL=1
}

# Extract the CSP string from the meta tag.
# We grep for the http-equiv line, then sed-extract the content="…" payload.
CSP_LINE=$(grep -E 'http-equiv="Content-Security-Policy"' "$INDEX_HTML" || true)
if [ -z "$CSP_LINE" ]; then
  fail "no Content-Security-Policy meta tag found in index.html"
  CSP=""
else
  # Pull out content="..." — the policy text between the first content=" and
  # the next ".  This breaks if the policy itself contains a literal ", but
  # the V1.16 policy uses only single quotes inside, so this is safe.
  CSP=$(printf '%s\n' "$CSP_LINE" | sed -E 's/.*content="([^"]*)".*/\1/')
fi

# Helper: returns 0 if the CSP contains the literal token (whitespace-tolerant).
csp_has() {
  local needle="$1"
  printf '%s\n' "$CSP" | grep -qE "(^|[ ;])${needle}([ ;]|$)"
}

# Required directives + tokens.
csp_has "default-src 'none'"                  || fail "CSP must declare \`default-src 'none'\`"
csp_has "script-src 'self' 'wasm-unsafe-eval'"|| fail "CSP must declare \`script-src 'self' 'wasm-unsafe-eval'\`"
csp_has "connect-src 'self'"                  || fail "CSP must declare \`connect-src 'self'\`"
csp_has "form-action 'none'"                  || fail "CSP must declare \`form-action 'none'\`"
csp_has "frame-ancestors 'none'"              || fail "CSP must declare \`frame-ancestors 'none'\`"
csp_has "base-uri 'none'"                     || fail "CSP must declare \`base-uri 'none'\`"
csp_has "require-trusted-types-for 'script'"  || fail "CSP must declare \`require-trusted-types-for 'script'\`"
csp_has "trusted-types 'none'"                || fail "CSP must declare \`trusted-types 'none'\`"

# Forbidden tokens — these would silently re-open the holes V1.16 closes.
# Note: 'wasm-unsafe-eval' contains the substring 'unsafe-eval' but is NOT
# the same keyword; we match it as a complete token to avoid false positives.
if printf '%s\n' "$CSP" | grep -qE "'unsafe-inline'"; then
  # 'unsafe-inline' on script-src is a hard fail. On style-src it is the
  # documented V1.16 tradeoff (small inline <style> block, low-severity sink),
  # so we narrow the failure to the script-src directive specifically.
  if printf '%s\n' "$CSP" | grep -qE "script-src[^;]*'unsafe-inline'"; then
    fail "CSP regressed: 'unsafe-inline' present on script-src (forbidden)"
  fi
fi
if printf '%s\n' "$CSP" | grep -qE "(^|[ ;])'unsafe-eval'([ ;]|$)"; then
  fail "CSP regressed: 'unsafe-eval' present (only 'wasm-unsafe-eval' is allowed)"
fi

# --- SRI integrity check ----------------------------------------------
EXPECTED_INTEGRITY=$(grep -E 'src="app\.js"' "$INDEX_HTML" \
  | sed -E 's/.*integrity="([^"]+)".*/\1/' \
  | head -n 1 || true)

if [ -z "$EXPECTED_INTEGRITY" ] || [ "$EXPECTED_INTEGRITY" = "$(grep -E 'src="app\.js"' "$INDEX_HTML")" ]; then
  fail "no integrity= attribute found on the app.js <script> tag"
elif [ "$EXPECTED_INTEGRITY" != "$ACTUAL_INTEGRITY" ]; then
  fail "SRI drift on app.js"
  echo "  index.html says: $EXPECTED_INTEGRITY" >&2
  echo "  actual app.js  : $ACTUAL_INTEGRITY" >&2
  echo "  fix: run \`tools/playground-csp-check.sh --update-sri\`" >&2
fi

# Also check the <script> tag carries crossorigin="anonymous" — required
# for SRI to actually take effect on module scripts in some browsers.
if ! grep -E 'src="app\.js"' "$INDEX_HTML" | grep -q 'crossorigin="anonymous"'; then
  fail "app.js <script> tag missing crossorigin=\"anonymous\" (required for SRI on modules)"
fi

# --- wasm-bindgen TT-compat audit (security-reviewer F-1) -------------
# scope-d's CSP includes `require-trusted-types-for 'script'` + `trusted-types
# 'none'`. The application code (app.js) is sink-free by construction —
# documented in app.js:8-19. But the wasm-pack-emitted glue at
# pkg/atlas_verify_wasm.js is generated code from wasm-bindgen and we don't
# control its sinks directly. If a future wasm-bindgen release adds a
# TT-protected sink (innerHTML, document.write, eval, new Function,
# setTimeout("string"), assignment to script.src/.text), the page will
# break at runtime under TT enforcement — fail-loud is silent here because
# the catch block in app.js will surface a generic "Failed to load WASM
# module" message and the actual TypeError lives in the console.
#
# This audit grep-scans the emitted glue (if present) for the same set of
# sinks app.js documents as forbidden. Result is informational if pkg/ is
# absent (we don't force a wasm-pack build in CI), informational-failing if
# pkg/ is present and the glue carries any sink. Verified clean at
# wasm-bindgen 0.2.118 (V1.16 Welle A baseline).
PKG_GLUE="${REPO_ROOT}/apps/wasm-playground/pkg/atlas_verify_wasm.js"
if [ -f "$PKG_GLUE" ]; then
  GLUE_SINKS=$(grep -nE "\beval\(|new Function\b|innerHTML|outerHTML|document\.write|insertAdjacentHTML|setTimeout\(['\"]|setInterval\(['\"]|\.text\s*=" "$PKG_GLUE" || true)
  if [ -n "$GLUE_SINKS" ]; then
    fail "wasm-bindgen-emitted glue at pkg/atlas_verify_wasm.js carries TT-protected sinks (incompatible with scope-d CSP)"
    echo "  matches:" >&2
    printf '%s\n' "$GLUE_SINKS" | sed 's/^/    /' >&2
    echo "  fix: pin a wasm-bindgen version whose --target web emitter is sink-free, OR" >&2
    echo "       relax the CSP to allow a named TT policy and create one in app.js for the WASM loader" >&2
  fi
else
  echo "  note: pkg/atlas_verify_wasm.js absent — wasm-bindgen TT-compat audit skipped (run \`wasm-pack build\` to populate)"
fi

# --- report-uri validation (V1.16 Welle B) ----------------------------
# CSP `report-uri <url>` directs browsers to POST a JSON report (Content-
# Type: application/csp-report) to <url> on every violation. Without this
# directive, violations are silent in production: the browser blocks the
# violation but no operator sees the report. Welle B mandates report-uri.
#
# We assert (a) the directive is present, and (b) the URL is same-origin
# (a relative path or starts with `/`). Cross-origin reporting endpoints
# are technically allowed by the spec but downgrade report fidelity
# (opaque-response cross-origin) and introduce a vendor dependency that
# breaks the page-bytes-portability win — we WARN, not fail.
REPORT_URI=$(printf '%s\n' "$CSP" | grep -oE "report-uri[[:space:]]+[^;]+" | sed -E 's/^report-uri[[:space:]]+//' | head -n1 || true)
REPORT_URI_FIRST=""
if [ -z "$REPORT_URI" ]; then
  fail "CSP must declare \`report-uri <url>\` (V1.16 Welle B — silent CSP violations are not acceptable in production)"
else
  # Strip optional surrounding whitespace, take first token if multiple.
  REPORT_URI_FIRST=$(printf '%s\n' "$REPORT_URI" | awk '{print $1}')
  case "$REPORT_URI_FIRST" in
    //*)
      # Schemeless protocol-relative URL like //attacker/csp — browsers
      # treat this as absolute (scheme inherited from page). MUST NOT be
      # silently accepted as same-origin. (code-reviewer M2 fix.)
      echo "  WARN: report-uri ($REPORT_URI_FIRST) is a protocol-relative URL — browsers"
      echo "        treat this as absolute with the page scheme, directing reports to a"
      echo "        third-party host. Recommend an absolute same-origin path like /csp-report."
      ;;
    /*)
      # Same-origin absolute path — the desired Welle B shape.
      :
      ;;
    http://*|https://*)
      echo "  WARN: report-uri is cross-origin ($REPORT_URI_FIRST). Two costs:"
      echo "        (1) browsers send opaque-response reports cross-origin (lower fidelity)"
      echo "            with several fields stripped to prevent fingerprinting;"
      echo "        (2) the chosen reporting vendor is now publicly disclosed in the page"
      echo "            CSP (HTML source is world-readable) — an information disclosure"
      echo "            about the operator's monitoring infrastructure."
      echo "        And a third-party endpoint introduces a vendor dependency that breaks the"
      echo "        page-bytes-portability win of meta-tag-delivered CSP."
      echo "        Recommend a same-origin /<path> instead."
      ;;
    *)
      echo "  WARN: report-uri ($REPORT_URI_FIRST) is neither an absolute path nor an absolute URL —"
      echo "        relative paths resolve against the current document URL, which is fragile under"
      echo "        nested-path serving. Recommend an absolute same-origin path like /csp-report."
      ;;
  esac
fi

# --- frame-ancestors meta-tag warning (security-reviewer F-2) ---------
# `frame-ancestors` is meta-tag-ignored by every major browser (only
# enforced when delivered as an HTTP header). The CSP declares it for
# defence-in-depth and so a hosting provider applying the policy as a
# header will honour it, but a CI pass on the meta-tag alone provides
# zero clickjacking protection. We declare this loudly so the operator
# does not conflate "validator green" with "clickjacking-protected".
if csp_has "frame-ancestors 'none'"; then
  echo "  WARN: frame-ancestors 'none' is declared in <meta http-equiv> CSP, but ALL major browsers"
  echo "        ignore frame-ancestors when delivered via meta-tag. Clickjacking protection requires"
  echo "        the hosting provider to ALSO send Content-Security-Policy as an HTTP header."
fi

if [ "$FAIL" -ne 0 ]; then
  echo ""
  echo "playground-csp-check.sh: FAIL — see errors above" >&2
  exit 1
fi

echo "playground-csp-check.sh: PASS"
echo "  CSP directives intact (meta-delivered)"
echo "  SRI matches: $ACTUAL_INTEGRITY"
if [ -f "$PKG_GLUE" ]; then
  echo "  wasm-bindgen glue: TT-sink-free"
fi
if [ -n "$REPORT_URI_FIRST" ]; then
  echo "  report-uri: $REPORT_URI_FIRST"
fi
