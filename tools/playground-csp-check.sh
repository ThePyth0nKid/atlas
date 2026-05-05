#!/usr/bin/env bash
#
# tools/playground-csp-check.sh
#
# V1.16 Welle A + B + C — anti-drift validator for the wasm-playground
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
# Live-check mode (V1.16 Welle C, opt-in: `--live-check <base-url>`):
#   6. The deployed Worker emits Content-Security-Policy AS AN HTTP HEADER
#      (not just meta-tag) — this is what makes frame-ancestors actually
#      enforce, and what closes the meta-tag's silent-ignore gap.
#   7. The HTTP-header CSP and the meta-tag CSP are consistent (no drift
#      between Worker-emitted policy and page-bytes policy).
#   8. Strict-Transport-Security is preload-eligible (max-age >= 31536000,
#      includeSubDomains, preload).
#   9. Cross-Origin-Opener-Policy: same-origin AND
#      Cross-Origin-Embedder-Policy: require-corp (Spectre defense).
#  10. X-Content-Type-Options: nosniff AND Referrer-Policy: no-referrer.
#  11. Cache-Control class is correct per path (root → no-cache,
#      must-revalidate; /pkg/*.wasm + /app.js → immutable).
#  12. POST /csp-report returns 204 with no body (silent-204 invariant).
#
# Operator workflow:
#   - After editing app.js, run with --update-sri to refresh the hash.
#   - In CI, run without flags; non-zero exit means page-bytes drift.
#   - Post-deploy, run with `--live-check https://<host>` to verify the
#     Worker-emitted hardening matches the source-of-truth in the repo.
#
# Cross-platform: bash + openssl + sed + grep + base64 (+ curl for
# --live-check). Git Bash on Windows works; macOS/Linux native works.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
INDEX_HTML="${REPO_ROOT}/apps/wasm-playground/index.html"
APP_JS="${REPO_ROOT}/apps/wasm-playground/app.js"

UPDATE_SRI=0
LIVE_BASE_URL=""
while [ $# -gt 0 ]; do
  case "$1" in
    --update-sri)
      UPDATE_SRI=1
      shift
      ;;
    --live-check)
      if [ $# -lt 2 ] || [ -z "${2:-}" ]; then
        echo "FAIL: --live-check requires a URL argument (e.g., https://playground.atlas-trust.dev)" >&2
        exit 2
      fi
      LIVE_BASE_URL="$2"
      shift 2
      ;;
    --live-check=*)
      LIVE_BASE_URL="${1#*=}"
      if [ -z "$LIVE_BASE_URL" ]; then
        echo "FAIL: --live-check= requires a URL value" >&2
        exit 2
      fi
      shift
      ;;
    -h|--help)
      sed -n '3,46p' "$0"
      exit 0
      ;;
    *)
      echo "unknown flag: $1" >&2
      echo "usage: $0 [--update-sri] [--live-check <base-url>]" >&2
      exit 2
      ;;
  esac
done

# --live-check requires curl. Pre-flight before any heavy lifting so the
# operator gets a clear error instead of mid-run "command not found".
if [ -n "$LIVE_BASE_URL" ]; then
  if ! command -v curl >/dev/null 2>&1; then
    echo "FAIL: --live-check requires curl, but curl is not on PATH" >&2
    exit 2
  fi
  # Strip trailing slash so we can build paths like "${LIVE_BASE_URL}/pkg/..."
  # without doubling slashes.
  LIVE_BASE_URL="${LIVE_BASE_URL%/}"
  # Reject anything that isn't an absolute https:// URL — http:// would defeat
  # the HSTS/COOP/COEP guarantees we're trying to verify, and a bare hostname
  # would silently default to https on curl but make the error message
  # confusing if it failed.
  case "$LIVE_BASE_URL" in
    https://*) : ;;
    *)
      echo "FAIL: --live-check URL must start with https:// (got: $LIVE_BASE_URL)" >&2
      exit 2
      ;;
  esac
fi

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
  # RFC 3986 §3.1 specifies scheme names as case-insensitive — coerce to
  # lowercase before the case-arm dispatch so `HTTP://` / `Https://` /
  # `HTTPS://` hit the cross-origin arm rather than slipping through to
  # the catch-all relative-path FAIL with a misleading message.
  REPORT_URI_FIRST_LC=$(printf '%s' "$REPORT_URI_FIRST" | tr '[:upper:]' '[:lower:]')
  case "$REPORT_URI_FIRST_LC" in
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
      # Catch-all: relative bare-word path like `csp-report` (no leading
      # slash). The header-comment intent for check-5 is "absolute same-
      # origin path"; relative paths resolve against the current document
      # URL — under nested-path serving (e.g. `/playground/v2/`) the
      # browser would POST to `/playground/v2/csp-report`, not to
      # `/csp-report`. This is a hard FAIL, not a WARN, because the
      # validator's promise is that a passing CSP correctly directs
      # reports to a same-origin endpoint. (Final-pass review fix —
      # closes the WARN-vs-FAIL inconsistency the security-reviewer
      # flagged.)
      fail "report-uri ($REPORT_URI_FIRST) is neither an absolute path (/...) nor an absolute URL (http(s)://...). Relative paths resolve against the document URL and are fragile under nested-path serving. Use an absolute same-origin path like /csp-report."
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

# --- live-check mode (V1.16 Welle C) ----------------------------------
# Verifies the deployed Worker actually emits the headers we promised.
# This is the post-deploy, end-to-end hardening assertion that complements
# the page-bytes-only checks above.
if [ -n "$LIVE_BASE_URL" ]; then
  echo ""
  echo "  live-check: ${LIVE_BASE_URL}"

  # Fetch headers for `/` — the index.html path. We use --max-time 15 so a
  # hung connection fails the check loudly instead of stalling CI. -sS
  # suppresses progress but preserves error output. -L follows redirects
  # (cf custom domains may 301 to canonical https).
  ROOT_HEADERS=$(curl -sSL -D - -o /dev/null --max-time 15 "${LIVE_BASE_URL}/" 2>&1) || {
    fail "live-check: GET ${LIVE_BASE_URL}/ failed (curl exit $?)"
    ROOT_HEADERS=""
  }

  if [ -n "$ROOT_HEADERS" ]; then
    # Extract a single header value, case-insensitively. cf headers may
    # vary case; the HTTP spec treats them as case-insensitive. We also
    # strip a trailing CR (curl preserves the HTTP \r\n line endings).
    extract_header() {
      local name="$1"
      printf '%s\n' "$ROOT_HEADERS" \
        | grep -iE "^${name}:" \
        | head -n 1 \
        | sed -E 's/^[^:]+:[[:space:]]*//' \
        | tr -d '\r'
    }

    HDR_CSP=$(extract_header 'Content-Security-Policy')
    HDR_HSTS=$(extract_header 'Strict-Transport-Security')
    HDR_COOP=$(extract_header 'Cross-Origin-Opener-Policy')
    HDR_COEP=$(extract_header 'Cross-Origin-Embedder-Policy')
    HDR_XCTO=$(extract_header 'X-Content-Type-Options')
    HDR_REFERRER=$(extract_header 'Referrer-Policy')
    HDR_CACHE_CONTROL=$(extract_header 'Cache-Control')

    # 6. CSP HTTP header present (this is what makes frame-ancestors enforce).
    if [ -z "$HDR_CSP" ]; then
      fail "live-check: Content-Security-Policy HTTP header missing on /"
    fi

    # 7. HTTP-header CSP and meta-tag CSP must be consistent. Strict equality
    # would be too brittle (whitespace, ordering); instead we assert that the
    # set of directives covered by the meta-tag is also covered by the header.
    # Practical check: every required directive from the meta-tag must appear
    # in the header text too. (The header may add MORE directives — that's
    # fine, it's strictly tighter, not a regression.)
    if [ -n "$HDR_CSP" ] && [ -n "$CSP" ]; then
      for required in \
        "default-src 'none'" \
        "script-src 'self' 'wasm-unsafe-eval'" \
        "connect-src 'self'" \
        "form-action 'none'" \
        "frame-ancestors 'none'" \
        "base-uri 'none'" \
        "require-trusted-types-for 'script'" \
        "trusted-types 'none'"; do
        if ! printf '%s\n' "$HDR_CSP" | grep -qE "(^|[ ;])${required}([ ;]|$)"; then
          fail "live-check: HTTP-header CSP missing required directive: \`${required}\`"
        fi
      done
    fi

    # 8. HSTS preload-eligible. The HSTS preload list requires:
    #   max-age >= 31536000 (1 year), includeSubDomains, preload.
    # We assert all three present in the header value.
    if [ -z "$HDR_HSTS" ]; then
      fail "live-check: Strict-Transport-Security HTTP header missing on /"
    else
      if ! printf '%s\n' "$HDR_HSTS" | grep -qiE 'max-age=[0-9]+'; then
        fail "live-check: HSTS missing max-age directive ($HDR_HSTS)"
      else
        HSTS_MAX_AGE=$(printf '%s\n' "$HDR_HSTS" | grep -oiE 'max-age=[0-9]+' | head -n1 | sed 's/[Mm][Aa][Xx]-[Aa][Gg][Ee]=//')
        if [ "$HSTS_MAX_AGE" -lt 31536000 ]; then
          fail "live-check: HSTS max-age=$HSTS_MAX_AGE is below preload minimum (31536000)"
        fi
      fi
      if ! printf '%s\n' "$HDR_HSTS" | grep -qi 'includeSubDomains'; then
        fail "live-check: HSTS missing \`includeSubDomains\` ($HDR_HSTS)"
      fi
      if ! printf '%s\n' "$HDR_HSTS" | grep -qi 'preload'; then
        fail "live-check: HSTS missing \`preload\` ($HDR_HSTS)"
      fi
    fi

    # 9. COOP/COEP — Spectre defense (must be exact values, not subsets).
    if [ "$HDR_COOP" != "same-origin" ]; then
      fail "live-check: Cross-Origin-Opener-Policy must be \`same-origin\` (got: $HDR_COOP)"
    fi
    if [ "$HDR_COEP" != "require-corp" ]; then
      fail "live-check: Cross-Origin-Embedder-Policy must be \`require-corp\` (got: $HDR_COEP)"
    fi

    # 10. nosniff + no-referrer.
    if [ "$HDR_XCTO" != "nosniff" ]; then
      fail "live-check: X-Content-Type-Options must be \`nosniff\` (got: $HDR_XCTO)"
    fi
    if [ "$HDR_REFERRER" != "no-referrer" ]; then
      fail "live-check: Referrer-Policy must be \`no-referrer\` (got: $HDR_REFERRER)"
    fi

    # 11a. Cache-Control on / must be `no-cache, must-revalidate` (html class).
    EXPECTED_HTML_CC="no-cache, must-revalidate"
    if [ "$HDR_CACHE_CONTROL" != "$EXPECTED_HTML_CC" ]; then
      fail "live-check: Cache-Control on / must be \`${EXPECTED_HTML_CC}\` (got: $HDR_CACHE_CONTROL)"
    fi
  fi

  # 11b. Cache-Control on /app.js must be `public, max-age=31536000, immutable`.
  APP_JS_HEADERS=$(curl -sSL -D - -o /dev/null --max-time 15 "${LIVE_BASE_URL}/app.js" 2>&1) || {
    fail "live-check: GET ${LIVE_BASE_URL}/app.js failed (curl exit $?)"
    APP_JS_HEADERS=""
  }
  if [ -n "$APP_JS_HEADERS" ]; then
    APP_JS_CC=$(printf '%s\n' "$APP_JS_HEADERS" | grep -iE '^Cache-Control:' | head -n1 | sed -E 's/^[^:]+:[[:space:]]*//' | tr -d '\r')
    EXPECTED_IMMUTABLE_CC="public, max-age=31536000, immutable"
    if [ "$APP_JS_CC" != "$EXPECTED_IMMUTABLE_CC" ]; then
      fail "live-check: Cache-Control on /app.js must be \`${EXPECTED_IMMUTABLE_CC}\` (got: $APP_JS_CC)"
    fi
    # Also verify CSP layered onto immutable assets (every response carries it).
    if ! printf '%s\n' "$APP_JS_HEADERS" | grep -qiE '^Content-Security-Policy:'; then
      fail "live-check: Content-Security-Policy header missing on /app.js (Worker should layer it onto every response)"
    fi
  fi

  # 12. POST /csp-report returns 204 (silent-204 invariant). We use a
  # well-formed minimal CSP report body so a bug in the receiver that 200s
  # on valid + 204s on invalid would surface here. -w '%{http_code}' is the
  # canonical curl trick to capture status code without parsing the headers.
  RECEIVER_BODY='{"csp-report":{"violated-directive":"script-src","blocked-uri":"https://example/x.js","document-uri":"'"${LIVE_BASE_URL}/"'"}}'
  RECEIVER_STATUS=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 15 \
    -X POST \
    -H "Content-Type: application/csp-report" \
    -H "Origin: ${LIVE_BASE_URL}" \
    --data-raw "$RECEIVER_BODY" \
    "${LIVE_BASE_URL}/csp-report" 2>&1) || {
    fail "live-check: POST ${LIVE_BASE_URL}/csp-report failed (curl exit $?)"
    RECEIVER_STATUS=""
  }
  if [ -n "$RECEIVER_STATUS" ] && [ "$RECEIVER_STATUS" != "204" ]; then
    fail "live-check: POST /csp-report must return 204 (silent-204 invariant); got HTTP $RECEIVER_STATUS"
  fi
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
if [ -n "$LIVE_BASE_URL" ]; then
  echo "  live-check: Worker-emitted headers verified at ${LIVE_BASE_URL}"
fi
