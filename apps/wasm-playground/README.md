# Atlas WASM playground (V1.14 Scope E + V1.16 Welle A)

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
# regression on script-src, SRI hash on app.js matches actual file bytes.
# Non-zero exit on drift.
```

This script is intentionally pure-bash (no Node/Python dependency) so
it runs in the same environment as the rest of the repo's CI shell
steps.

## License

Apache-2.0 (same as the verifier crates) — auditors and third-party
tools can fork/embed/redistribute the playground without friction.
