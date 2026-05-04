# Atlas WASM playground (V1.14 Scope E)

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

## License

Apache-2.0 (same as the verifier crates) — auditors and third-party
tools can fork/embed/redistribute the playground without friction.
