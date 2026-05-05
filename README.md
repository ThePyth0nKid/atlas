# Atlas — Verifiable Knowledge Graphs for AI Agents

> *"Knowledge graphs the agent can prove, not just claim."*

Atlas is a knowledge graph backend where every fact is signed (Ed25519 + COSE_Sign1),
every edge is hash-chained, every state is anchored to Sigstore Rekor, and every
agent-write is verified by an offline WASM verifier — running in the customer's
browser, not on our server.

## Why?

EU AI Act Article 12 (in force 2026-08-02) requires high-risk AI systems to
maintain automatic event records that are *independently verifiable* by regulators.

Atlas makes that **structurally true** — not a checkbox in a compliance dashboard.

## Status

**V1.16 Welle A + Welle B + Welle C shipped — browser-runtime hardening for `apps/wasm-playground/`: strict CSP via `<meta http-equiv>` (`default-src 'none'`, `script-src 'self' 'wasm-unsafe-eval'` — no `'unsafe-inline'`, no `'unsafe-eval'`), Subresource Integrity (sha384) on the application JS, Trusted Types enforcement (`require-trusted-types-for 'script'; trusted-types 'none'` against a sink-free `app.js`), `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, AND (Welle B) `report-uri /csp-report` so deployed playgrounds surface every blocked CSP violation as a same-origin JSON POST instead of failing silently, AND (Welle C) a Cloudflare Workers + Static Assets host that emits the full security-header set as HTTP headers on every response (HSTS preload, COOP `same-origin`, COEP `require-corp`, the same CSP as a header so `frame-ancestors` finally takes effect, `report-to` + `Reporting-Endpoints` for forward-compat, per-path Cache-Control), runs an executable receiver at `/csp-report` (silent-204 + categorised internal logs + Origin-anchored CSRF + per-IP `/64` + global rate-limit via Durable Object + JSON-bomb defence + ANSI-strip + field allow-list + Workers Analytics Engine `writeDataPoint` persistence), and writes a daily AE → R2 archive heartbeat. Anti-drift pin in `tools/playground-csp-check.sh` (pure-bash CI validator + `--live-check <url>` for post-deploy Worker validation) covers all three Wellen, plus a repo-tracked `tools/git-hooks/pre-commit` activated by `bash tools/install-git-hooks.sh`. V1.15 CLOSED previously (Welle A const-time KID-equality invariant, Welle B dual-channel WASM distribution via GitHub Releases — see [OPERATOR-RUNBOOK §12](docs/OPERATOR-RUNBOOK.md), Welle C consumer-side reproducibility runbook — see [CONSUMER-RUNBOOK.md](docs/CONSUMER-RUNBOOK.md)). V1.14 Scope I + Scope J + Scope E shipped — HSM-backed witness (witness scalar sealed inside PKCS#11 token) + auditor wire-surface (structured `witness_failures` in `VerifyOutcome` JSON) + WASM verifier on npm + browser playground.**

Trust-core crate + Rekor anchoring + per-tenant key derivation + HSM-backed signing + independent
witness attestor (V1.5 mock-issuer, V1.6 live Sigstore Rekor v1, V1.7 anchor-chain + shard
expansion, V1.8 per-tenant key separation, V1.9 strict-mode + master-seed positive opt-in,
V1.10 in-HSM HKDF derivation, V1.11 wave-3 sealed-signer (per-workspace scalar never enters
Atlas address space), V1.12 CI lane promotion + nightly Sigstore lane, V1.13 wave C-1 lenient
witness + wave C-2 strict mode, V1.14 Scope I HSM-backed witness — witness Ed25519 scalar
never enters atlas-witness address space, signing routes through `CKM_EDDSA` against a
sealed `Sensitive=true, Extractable=false, Derive=false` keypair).

Current state:

- 347 Rust tests green across the workspace (`--features hsm`): 121 trust-core unit
  (incl. V1.14 Scope J `WitnessFailureReason` per-variant pinning + cross-batch dup
  sanitisation pin) + 18 anchor-chain adversary + 13 golden trace + 11 per-tenant-keys
  adversary + 6 Sigstore golden + 6 witness-strict-mode + 8 V1.14 Scope J
  `witness_failures` integration + 5 V1.15 Welle A `const_time_kid_invariant`
  source-level anti-drift (KID-equality audit + helper sanity pins) in
  `atlas-trust-core`; 115 issuer/anchor/HSM in `atlas-signer`; 5 strict-mode +
  3 V1.14 Scope J `--output json` in `atlas-verify-cli`; 29 unit + 1
  byte-equivalence integration in `atlas-witness` including V1.14 Scope I
  HSM-backed witness
- Signing-input is deterministic CBOR per RFC 8949 §4.2.1
  (length-first map sort, no floats, byte-pinned golden)
- Pubkey-bundle hash is canonical-JSON, byte-pinned golden — silent
  bundle-rotation drift trips the test before reaching production
- Anchor verification: RFC 6962-style Merkle inclusion proofs +
  Ed25519-signed checkpoints, validated against a pinned log pubkey.
  Anchored objects: `bundle_hash` (defends bundle swap) and `dag_tip`
  (defends tail truncation). Lenient by default,
  `VerifyOptions::require_anchors` strict mode for high-assurance audit
- Anchor-chain tip-rotation: consecutive `AnchorBatch`es are cross-linked
  via `prev_anchor_head`, recomputed deterministically, and the chain
  head is signable by an independent witness (V1.13)
- Independent witness cosignature (V1.13): `atlas-witness` binary signs
  the recomputed chain head with its own Ed25519 key in a separate
  process from `atlas-signer` (trust-domain separation by process).
  `ATLAS_WITNESS_DOMAIN` prefix prevents cross-domain replay,
  `ATLAS_WITNESS_V1_ROSTER` is source-controlled (genesis-empty per the
  V1.7 boundary rule). `--require-witness <N>` promotes wave-C-1's
  lenient evidence row to a hard error when fewer than `N` distinct
  roster-resolved witnesses verify
- HSM-backed witness (V1.14 Scope I): `atlas-witness sign-chain-head --hsm`
  routes signing through a PKCS#11 token's `CKM_EDDSA(Ed25519)` against
  a sealed (`Sensitive=true, Extractable=false, Derive=false`) keypair —
  the witness Ed25519 scalar never enters atlas-witness address space.
  Trust-domain separation extends to the env-var trio
  (`ATLAS_WITNESS_HSM_*` distinct from `ATLAS_HSM_*`) and label prefix
  (`atlas-witness-key-v1:` distinct from `atlas-workspace-key-v1:`).
  `atlas-witness extract-pubkey-hex --kid X` retrieves the rostering
  hex per OPERATOR-RUNBOOK §11
- Auditor wire-surface (V1.14 Scope J): `VerifyOutcome.witness_failures`
  is a structured `Vec<WitnessFailureWire>` (additive,
  `#[serde(default)]`) carrying `{ witness_kid, batch_index,
  reason_code, message }`. `WitnessFailureReason` is a
  `#[non_exhaustive]` kebab-case enum (`kid-not-in-roster`,
  `duplicate-kid`, `cross-batch-duplicate-kid`,
  `invalid-signature-format`, …). Auditor tooling switches on
  `reason_code` instead of fragile wording match against the lenient
  evidence row's `detail`. `atlas-verify-cli verify-trace --output json`
  carries the field; the field is end-to-end exercised by the TS
  smoke lane in `apps/atlas-mcp-server/scripts/smoke.ts` step 8
- `workspace_id` bound into the signing-input + per-workspace key
  derivation (HKDF-SHA256) — cross-workspace replay structurally
  impossible AND no shared signing key across tenants
- HSM-backed signing (V1.11 wave-3): per-workspace scalar lives only
  inside the HSM token; `CKM_EDDSA(Ed25519)` signs without exposing
  private bytes to the Atlas address space (`atlas-signer --features hsm`)
- Constant-time hash AND KID equality (V1.15 Welle A extends the V1.5 hash
  invariant to every wire-side KID compare reachable from the verifier
  API), alg-downgrade rejection, RFC 3339 timestamp validation,
  duplicate-event-hash detection, `deny_unknown_fields` on the wire format
- Bank demo bundle verifies `✓ VALID` end-to-end through both the native
  CLI and the in-browser WASM verifier, including
  `✓ anchors — N anchor(s) verified against pinned log keys` and
  `✓ witnesses — M presented / K verified` when witness sigs are attached

V1.6 ships live Sigstore submission. V1.7 adds anchor-chain tip-rotation + Sigstore shard
roster expansion. V1.8/V1.9/V1.10/V1.11 ship per-tenant key separation and HSM-backed
signing through to wave-3 (per-workspace scalar never enters Atlas address space). V1.12
promotes the HSM and Sigstore CI lanes to `pull_request:` and adds a nightly live-Rekor
sentry. V1.13 ships the independent witness cosignature attestor (wave C-1 lenient,
wave C-2 strict-mode + commissioning ceremony). V1.14 Scope I closes the witness-side
residual by sealing the witness Ed25519 scalar inside a PKCS#11 token (signing routes
through `CKM_EDDSA`, scalar never enters atlas-witness address space). V1.14 Scope J
replaces V1.13 wave-C-2's string-match diagnostic surface with a structured
`witness_failures` JSON wire so auditor tooling can classify per-witness failures
without keying on human-readable wording. V1.14 Scope E publishes the WASM verifier
to npm as `@atlas-trust/verify-wasm` (browser + Node.js targets) and ships a
zero-build-step browser playground at `apps/wasm-playground/` — the same Rust
verifier core, packaged for in-browser auditor tooling without a server round-trip.
V1.15 Welle A extends the V1.5 const-time-hash-equality invariant to every
wire-side KID compare reachable from the verifier API: the V1.9 per-tenant-keys
strict-mode check now routes through `crate::ct::ct_eq_str`, joining the V1.13
wave-C-2 witness-roster compare. A new source-level anti-drift test
(`tests/const_time_kid_invariant.rs`) audits both `verify.rs` and `witness.rs`
for forbidden raw-`==` patterns on KID fields and fails the build at the next
CI run if a future caller re-introduces one. V1.15 Welle B adds a backup
distribution channel for the WASM verifier: every `v*` tag push uploads the
byte-identical `npm pack` tarballs (web + node) plus a `tarball-sha256.txt`
manifest as GitHub Release assets alongside the existing npm publish, so an
auditor whose primary channel is unreachable can `gh release download`,
`sha256sum --check`, and `npm install ./local.tgz` against the same SLSA L3
provenance attestation. V1.15 Welle C closes the V1.15 distribution-resilience
story on the consumer side: a new `docs/CONSUMER-RUNBOOK.md` documents
exact-version pinning across npm / pnpm / Bun lockfiles, SLSA L3 provenance
verification via `npm audit signatures`, the GH-Releases backup-channel
install flow, and the reproduce-from-source fallback (clone at the tagged
commit, `cargo install wasm-pack --locked`, `wasm-pack build`, byte-compare)
for the both-channels-unreachable scenario. V1.16 Welle A hardens the WASM
playground at `apps/wasm-playground/` for any deployment beyond pure
local-dev: application code is extracted from inline `<script>` into
`app.js` so the page can ship a strict `<meta http-equiv>` CSP without
`'unsafe-inline'` / `'unsafe-eval'` on `script-src`, with sha384 SRI on
the loading `<script>` tag and Trusted Types enforced (sink-free
`app.js` discipline; any future regression that introduces `innerHTML`
/ `eval` / `setTimeout(string)` / `*.src = userInput` fails at the
browser boundary). V1.16 Welle B closes the Welle-A residual gap that
CSP violations were silent in production by adding `report-uri
/csp-report` to the meta-tag CSP — a deployed playground that runs a
minimal receiver at the same-origin path (receiver-shape spec in
[SECURITY-NOTES §scope-d](docs/SECURITY-NOTES.md)) now sees every
blocked violation as a JSON POST. V1.16 Welle C is the deployment-
side closure: a Cloudflare Workers + Static Assets host
(`apps/wasm-playground/wrangler.toml` + `apps/wasm-playground/
worker/`) emits the full security-header set on every response
(HSTS preload, COOP `same-origin`, COEP `require-corp`, the same
CSP as an HTTP header so `frame-ancestors` finally takes effect,
plus `report-to` + `Reporting-Endpoints` for forward-compat with
the Reporting API, plus per-path Cache-Control), runs an executable
receiver at `/csp-report` (silent-204 + categorised internal logs +
Origin-anchored CSRF + per-IP `/64` + global rate-limit via Durable
Object + JSON-bomb defence + ANSI-strip + field allow-list + AE
`writeDataPoint` persistence), and writes a daily AE → R2 archive
heartbeat. The pure-bash anti-drift validator
(`tools/playground-csp-check.sh`) re-asserts the CSP directives +
SRI hash + `report-uri` declaration on every CI run; with
`--live-check <url>` it asserts every Worker-emitted hardening
invariant against the deployed URL post-deploy. A repo-tracked
`tools/git-hooks/pre-commit` activated by `bash tools/install-git-
hooks.sh` runs the validator at commit time. Graph-database
integration and policy-engine follow in V2.

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — full system design,
  trust property, write/export flows, V1 → V1.16 → V2 boundaries.
- [docs/SECURITY-NOTES.md](docs/SECURITY-NOTES.md) — defended attack
  surface, per-test mapping for auditors.
- [docs/OPERATOR-RUNBOOK.md](docs/OPERATOR-RUNBOOK.md) — production
  operator procedures: master-seed import, HSM wave-3 setup, witness
  commissioning (verifier-side §10 + HSM-backed witness §11),
  WASM verifier backup-channel install (V1.15 Welle B §12),
  CI lane reference.
- [docs/CONSUMER-RUNBOOK.md](docs/CONSUMER-RUNBOOK.md) — downstream
  npm-consumer reproducibility: exact-version pinning, lockfile
  integrity, SLSA L3 provenance verification, reproduce-from-source
  fallback (V1.15 Welle C).
- [docs/COMPLIANCE-MAPPING.md](docs/COMPLIANCE-MAPPING.md) —
  clause-by-clause regulatory mapping (EU AI Act, GAMP 5, ICH E6(R3),
  DORA, GDPR).

## Try it offline

```bash
# Build atlas-verify-cli from source (or download a release binary)
# Get any Atlas trace bundle (.json) and matching pubkey-bundle (.json)
atlas-verify-cli verify-trace bundle.json -k pubkey-bundle.json

# Output:
#   ✓ VALID — all checks passed
#   ✓ schema-version — trace schema atlas-trace-v1 matches verifier
#   ✓ event-hashes — 5 events, all hashes recomputed-match
#   ✓ event-signatures — 5 signatures verified
#   ✓ parent-links — all parent_hashes resolved within trace
#   ✓ dag-tips — 1 tips, match server claim
#   ✓ anchors — 2 anchor(s) verified against pinned log keys
#   ✓ witnesses — 0 presented / 0 verified  (lenient until --require-witness)
```

## Try it in the browser

```bash
# `@atlas-trust/verify-wasm` ships under two npm dist-tags so the
# same package name covers both runtimes:
#
#   * default tag (`latest`) — ESM + browser-first build (web target)
#   * `node` dist-tag — CommonJS build for Node.js consumers
#
# Browser (ESM, default tag):
#   npm install @atlas-trust/verify-wasm
#   import init, { verify_trace_json } from '@atlas-trust/verify-wasm';
#   await init();
#   const outcome = verify_trace_json(traceJson, bundleJson);
#   // outcome.valid === true / false
#   // outcome.witness_failures[] — V1.14 Scope J classification surface
#
# Node.js (CommonJS, `node` dist-tag — pin the tag to get the
# Node-target build instead of the browser ESM build):
#   npm install @atlas-trust/verify-wasm@node
#   const m = require('@atlas-trust/verify-wasm');
#   const outcome = m.verify_trace_json(traceJson, bundleJson);
```

Or open the zero-build-step playground at `apps/wasm-playground/` — drop
a `*.trace.json` + `*.pubkey-bundle.json`, click Verify. Same Rust
verifier core, byte-identical to the native CLI's output.

### For npm consumers — pinning + provenance verification

If you embed `@atlas-trust/verify-wasm` in production tooling, see
[docs/CONSUMER-RUNBOOK.md](docs/CONSUMER-RUNBOOK.md) for the full
consumer-side reproducibility guide: exact-version pinning across
npm / pnpm / Bun lockfiles, SLSA L3 provenance verification via
`npm audit signatures`, the V1.15 Welle B GitHub-Releases backup-
channel install flow, and the reproduce-from-source fallback when
both channels are unreachable.

No network calls. No talking to our server. Bit-identical determinism —
the same input produces byte-identical signing-input bytes whether the
verifier ran on Linux, on macOS, or as WASM in your browser. Three
anti-drift properties in `crates/` lock the trust model at the build
step:
`atlas-trust-core/src/cose.rs::signing_input_byte_determinism_pin` for
the per-event signing-input;
`atlas-trust-core/src/pubkey_bundle.rs::bundle_hash_byte_determinism_pin`
for the pubkey-bundle hash that binds a trace to a keyset; and
`atlas-signer/src/anchor.rs::mock_log_pubkey_matches_signer_seed` which
asserts the issuer-side seed and the verifier-pinned log pubkey stay in
sync — touching one without the other fails CI.

## Components

| Component | Path | License |
|---|---|---|
| `atlas-trust-core` | `crates/atlas-trust-core` | Apache-2.0 |
| `atlas-verify-cli` | `crates/atlas-verify-cli` | Apache-2.0 |
| `atlas-verify-wasm` | `crates/atlas-verify-wasm` | Apache-2.0 |
| `atlas-signer` | `crates/atlas-signer` | Apache-2.0 |
| `atlas-witness` | `crates/atlas-witness` | Apache-2.0 |
| `atlas-mcp-server` | `apps/atlas-mcp-server` | Sustainable Use |
| `atlas-web` | `apps/atlas-web` | Sustainable Use |

## Compliance Mappings

- EU AI Act Annex IV §1(e) — full provenance trail with named human verifiers
- GAMP 5 Appendix D11 (July 2025) — AI/ML system validation in GxP context
- ICH E6(R3) §7.4 — clinical trial data lineage
- DORA Art. 11–14 — operational resilience event logging
- GDPR Art. 22 — automated-decision traceability

A clause-by-clause mapping with inspectable-evidence pointers lives at
[docs/COMPLIANCE-MAPPING.md](docs/COMPLIANCE-MAPPING.md).

## Build

```bash
# Rust workspace
cargo build --release
cargo test

# End-to-end demo: generate a trace + bundle, then verify it
cargo run --example seed_bank_demo -p atlas-signer --release
./target/release/atlas-verify-cli verify-trace \
  examples/golden-traces/bank-q1-2026.trace.json \
  -k examples/golden-traces/bank-q1-2026.pubkey-bundle.json
```

## License

Atlas uses a fair-code split, modelled on n8n's Sustainable Use License:

- **Verifier crates** (`crates/atlas-trust-core`, `atlas-verify-cli`,
  `atlas-verify-wasm`, `atlas-signer`, `atlas-witness`) are **Apache-2.0**.
  Any auditor, regulator, or third-party tool can fork, embed, redistribute,
  or rebuild them with no friction. Apache-2.0 is the standard for sigstore-rs
  and the Rust crypto tooling ecosystem — Atlas joins it.

- **Server, web frontend, and MCP server** (`apps/`) are licensed under the
  **Atlas Sustainable Use License** (see `LICENSE-SUSTAINABLE-USE`).
  Self-host for internal business use is permitted free of charge. Hosting
  Atlas as an as-a-service offering for third parties, removing attribution,
  or reselling compliance bundles requires a commercial license.

This split is deliberate: the trust-property of Atlas must be structurally
verifiable by any auditor, end-to-end, without buying anything from us.

Commercial licensing inquiries: nelson@ultranova.io
