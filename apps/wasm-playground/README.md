# Atlas WASM playground (V1.14 Scope E + V1.16 Welle A + Welle B + Welle C)

Tiny static page that loads the Atlas verifier WASM module in your browser
and verifies a trace bundle locally — no network call to any Atlas server.

## Run locally

From the repo root:

```bash
# 1. Build the WASM module into the playground's pkg/ directory.
wasm-pack build crates/atlas-verify-wasm \
  --target web --release \
  --out-dir ../../apps/wasm-playground/pkg

# 2. Serve the playground over HTTP. Any static file server works;
#    using a server (rather than file://) is required because the
#    browser refuses to fetch WASM modules from the file:// scheme.
npx serve apps/wasm-playground
# … or:
python3 -m http.server --directory apps/wasm-playground 8000
```

Open the served URL, drop in a `*.trace.json` and the matching
`*.pubkey-bundle.json`, click **Verify**.

For a known-good fixture, use the bank demo bundle in
`examples/golden-traces/`:

- `bank-q1-2026.trace.json`
- `bank-q1-2026.pubkey-bundle.json`

## Production distribution

After V1.14 release, the same WASM module ships as
`@atlas-trust/verify-wasm` on npm — see the
[wasm-publish CI lane](../../.github/workflows/wasm-publish.yml).
The playground's local-dev `./pkg/` import path can be swapped for an
unpkg / jsdelivr CDN URL once published.

## Trust property

Same Rust verifier core (`atlas-trust-core`) compiled to
`wasm32-unknown-unknown`. The WASM build is byte-deterministic per
the V1.5 / V1.7 anti-drift properties — same trace + bundle produces
the same `VerifyOutcome` whether verified via the native CLI, in this
browser playground, or via the Node.js npm package.

## Browser-runtime hardening (V1.16 Welle A)

The playground page ships defence-in-depth hardening so a hosted
deployment (e.g. `playground.atlas-trust.dev`) can resist injection
attacks even if the surrounding network/CDN/proxy is partially
compromised:

- **Strict Content Security Policy** via `<meta http-equiv>`. The policy
  travels with the page and does not depend on a hosting provider
  setting the right HTTP header. Highlights:
  - `default-src 'none'` — every undeclared fetch directive denies by
    default (img, font, media, object, frame, child, manifest, worker).
  - `script-src 'self' 'wasm-unsafe-eval'` — only same-origin scripts
    plus the WebAssembly compilation keyword. JavaScript `eval()` and
    `new Function()` are blocked.
  - `style-src 'self' 'unsafe-inline'` — small inline `<style>` block
    permitted (low-severity sink, no JS exec or DOM API access; revisit
    if a build pipeline gets introduced).
  - `connect-src 'self'` — only same-origin fetch/dynamic-import
    network egress.
  - `form-action 'none'`, `frame-ancestors 'none'`, `base-uri 'none'` —
    submission/clickjacking/base-anchor attacks blocked.
  - `require-trusted-types-for 'script'` + `trusted-types 'none'` —
    every script-related sink (`innerHTML`, `document.write`, setting
    `*.src` on script elements, `eval`, `setTimeout(string)`, …) must
    receive a TrustedHTML/TrustedScriptURL value, but no policy is
    allowed to mint one. The application is sink-free by construction
    (`app.js` uses only `textContent`, `className`, `style.display`),
    so any future regression that re-introduces a sink fails at the
    browser boundary, not at code-review time.
- **Subresource Integrity (sha384)** on the `<script src="app.js">` tag.
  If the served bytes of `app.js` don't match the pinned hash, the
  browser refuses to execute the module.
- **`crossorigin="anonymous"`** on the same tag — required for SRI to
  take effect on module scripts, and ensures no cookies are sent on the
  fetch.
- **`X-Content-Type-Options: nosniff`** + **`Referrer-Policy: no-referrer`**
  via `<meta>` — block MIME-sniffing on script content and prevent
  referrer leakage to embedded resources.

## CSP violation reporting (V1.16 Welle B)

The CSP meta-tag also declares `report-uri /csp-report`. On every
blocked violation (XSS attempt, accidental sink introduction, mis-
configured cross-origin load) the browser POSTs a JSON report to
the same-origin path `/csp-report` with `Content-Type:
application/csp-report` (Chrome/Edge/Safari) or `application/json`
(Firefox historically) — receivers MUST accept both. The CSP
enforcement is unchanged — the
violation is still BLOCKED regardless of whether the report POST
succeeds — but a deployed playground that runs a receiver at that
path will see every attempt instead of relying on operators to spot
the page break in DevTools.

**Operator setup options:**

- *Self-hosted minimal collector (~30 lines).* Cloudflare Worker /
  Lambda / Vercel Edge Function / Netlify Function that accepts POST,
  parses JSON, appends to a log sink. No vendor dependency beyond
  the hosting platform.
- *Third-party reporting service.* `report-uri.com`, Sentry security
  reports, Datadog RUM. Faster, introduces a vendor; opaque-response
  cross-origin reports have lower fidelity. If you go this route,
  override the meta-tag CSP via an HTTP-header CSP at the hosting
  layer (keeps page bytes deployment-agnostic).
- *No receiver.* Browsers POST to a 404; violation still blocked,
  report lost. Acceptable for local-dev only.

The full receiver-shape spec (HTTP method, body schema, recommended
behaviours: append-only log, per-IP rate-limit, schema validation
to reject report-spoofing) lives in
[`docs/SECURITY-NOTES.md` §scope-d covers-bullet 6](../../docs/SECURITY-NOTES.md).

The validator (`tools/playground-csp-check.sh`) asserts the
`report-uri` declaration is present AND same-origin (cross-origin
endpoints get a WARN, not a hard fail).

## Production hosting (V1.16 Welle C)

The playground deploys to a single Cloudflare Worker that hosts
the static-asset bundle, runs the receiver, and writes a daily
archive heartbeat. See `wrangler.toml` and `worker/src/`. Every
response — static asset 2xx, asset-binding 404, receiver 204 —
passes through `applySecurityHeaders` and carries:

- **CSP as an HTTP header** (same eight directives as the meta-tag
  CSP, plus `report-uri /csp-report` and `report-to reports` plus
  the matching `Reporting-Endpoints: reports="/csp-report"`
  companion). `frame-ancestors 'none'` finally takes effect because
  it is now header-delivered.
- **HSTS preload** (`max-age=31536000; includeSubDomains; preload`).
- **Cross-Origin-Opener-Policy: same-origin** + **Cross-Origin-
  Embedder-Policy: require-corp** — Spectre / cross-window leak
  defence.
- **X-Content-Type-Options: nosniff** + **Referrer-Policy:
  no-referrer** as HTTP headers (also kept as meta-tags for
  page-bytes-only fallback paths).
- **Per-path Cache-Control** — `no-cache, must-revalidate` on `/`
  and `/index.html`; `public, max-age=31536000, immutable` on
  `/app.js` (SRI-pinned) and `/pkg/*` (content-hashed); `no-store`
  on `/csp-report`.

The receiver (`worker/src/csp-receiver.ts`) is the executable form
of the receiver-shape spec from Welle B: silent-204 on every
validation failure, categorised internal logs only, Origin-anchored
CSRF defence, body cap + JSON-bomb defence (depth-4 / 24-key
per-receiver tight limits), per-IP `/64` + global rate-limit via a
Durable Object (`worker/src/rate-limit.ts`), ANSI-strip + field
allow-list, AE `writeDataPoint` persistence. AE → R2 daily
heartbeat at `0 3 * * *` UTC (one PUT/day, defends against per-
report financial-DoS amplification on R2 Class-A ops).

The `experimental_serve_directly = false` + `run_worker_first =
true` flags in `wrangler.toml` `[assets]` force Worker invocation
BEFORE the asset match — without these, Cloudflare's edge would
serve assets directly and bypass the security-header layering.
Both flags are set for forward-compat across wrangler 3.x ↔ 4.x.

**Local dev:**

```bash
cd apps/wasm-playground
npx wrangler dev    # serves on http://localhost:8787 with miniflare bindings
```

**Tests + typecheck (run before any deploy):**

```bash
cd apps/wasm-playground/worker
npm install
npx vitest run        # 91 tests
npx tsc --noEmit      # type-check clean
```

**Live-check after deploy:**

```bash
bash tools/playground-csp-check.sh --live-check https://playground.atlas-trust.dev
# Asserts every Worker-emitted hardening invariant against the
# deployed URL via curl: HTTP-header CSP consistency with meta-tag,
# HSTS preload eligibility, COOP/COEP exact values, per-path
# Cache-Control, POST /csp-report → 204.
```

**Repo-tracked git hook (one-time per clone):**

```bash
bash tools/install-git-hooks.sh
# Activates tools/git-hooks/pre-commit, which runs
# tools/playground-csp-check.sh on every commit that touches
# app.js / index.html / the wasm-bindgen glue. Catches SRI-pinning
# drift before it lands in git.
```

### Maintenance — after editing `app.js`

The SRI hash on the `<script>` tag in `index.html` must be regenerated
whenever `app.js` is modified, or browsers will refuse to execute the
new bytes. Use the helper:

```bash
# From the repo root.
tools/playground-csp-check.sh --update-sri
```

Without flags the same script runs the CI-style validation:

```bash
tools/playground-csp-check.sh
# Asserts: CSP directives intact, no 'unsafe-inline'/'unsafe-eval'
# regression on script-src, SRI hash on app.js matches actual file
# bytes, wasm-bindgen-emitted glue is TT-sink-free (when pkg/ exists),
# and (V1.16 Welle B) report-uri is declared and same-origin.
# Non-zero exit on drift.
```

This script is intentionally pure-bash (no Node/Python dependency) so
it runs in the same environment as the rest of the repo's CI shell
steps.

## License

Apache-2.0 (same as the verifier crates) — auditors and third-party
tools can fork/embed/redistribute the playground without friction.
