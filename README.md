<!--
  TODO: drop the 70s-style Atlas icon image (à la Canon-Projekt-Look) at `docs/assets/atlas-logo.png`,
  then UNCOMMENT the <p>/<img> block below to render it above the title.

<p align="center">
  <img src="docs/assets/atlas-logo.png" alt="Atlas — Verifiable Knowledge Graphs for AI Agents" width="180" />
</p>
-->

<h1 align="center">Atlas</h1>

<p align="center"><em>Verifiable Knowledge Graphs for AI Agents — knowledge the agent can prove, not just claim.</em></p>

<p align="center">
  <a href="https://www.npmjs.com/package/@atlas-trust/verify-wasm"><img alt="npm" src="https://img.shields.io/npm/v/@atlas-trust/verify-wasm.svg?style=flat-square&label=%40atlas-trust%2Fverify-wasm" /></a>
  <a href="https://opensource.org/licenses/Apache-2.0"><img alt="License: Apache-2.0 (verifier)" src="https://img.shields.io/badge/verifier_license-Apache--2.0-blue?style=flat-square" /></a>
  <a href="LICENSE-SUSTAINABLE-USE"><img alt="License: Sustainable Use (server)" src="https://img.shields.io/badge/server_license-Sustainable_Use-orange?style=flat-square" /></a>
  <a href="https://slsa.dev/spec/v1.0/levels#build-l3"><img alt="SLSA Build L3" src="https://img.shields.io/badge/SLSA-Build_L3-2ea44f?style=flat-square" /></a>
  <a href="docs/COMPLIANCE-MAPPING.md"><img alt="EU AI Act Article 12" src="https://img.shields.io/badge/EU_AI_Act-Art._12-005599?style=flat-square" /></a>
</p>

Atlas is a knowledge-graph backend where every fact is signed (Ed25519 + COSE_Sign1), every edge is hash-chained, every state is anchored to Sigstore Rekor, and every agent write is verified by an **offline WASM verifier** running in the customer's browser — not on our server.

---

## Why Atlas exists

**EU AI Act Article 12** (in force 2026-08-02) requires high-risk AI systems to maintain automatic event records that are *independently verifiable* by regulators.

Most "AI logging" today is a dashboard. Atlas makes the verifiability **structural** — the auditor doesn't trust our server, our company, or our roadmap. They run the verifier themselves and check the signatures, anchors, and witness cosignatures against pinned trust roots.

| Trust property | How Atlas delivers it |
|---|---|
| **Cryptographic integrity** | Ed25519 + COSE_Sign1 signatures, hash-chained edges, byte-deterministic CBOR signing-input (RFC 8949 §4.2.1) |
| **Independent verifiability** | Offline WASM verifier — auditor runs it in any browser, no network calls to us |
| **Tamper-evident anchoring** | Every state anchored to Sigstore Rekor with RFC 6962 Merkle inclusion proofs, validated against pinned log pubkey |
| **Witness cosignature** | Out-of-process independent witness (`atlas-witness`) signs chain heads with separate Ed25519 key — trust-domain separation by process |
| **Hardware-backed signing** | PKCS#11/HSM seal for per-workspace and witness scalars (`CKM_EDDSA`, `Sensitive=true, Extractable=false, Derive=false`); private bytes never enter Atlas address space |
| **Supply-chain provenance** | npm publishes with SLSA Build L3 OIDC provenance via Sigstore Rekor; consumers verify via `npm audit signatures` and exact-version lockfile pins |
| **Tag-signing enforcement** | Every `v*` release tag must be SSH-signed by a key in the in-repo trust root `.github/allowed_signers`; publish lane fails closed on unsigned/untrusted tags |

---

## Quick Start

### Browser (ESM, default tag)

```bash
npm install @atlas-trust/verify-wasm
```

```js
import init, { verify_trace_json } from '@atlas-trust/verify-wasm';
await init();
const outcome = verify_trace_json(traceJson, bundleJson);
// outcome.valid === true / false
// outcome.witness_failures[] — structured per-witness failure surface (V1.14 Scope J)
```

### Node.js (CommonJS, `node` dist-tag)

```bash
npm install @atlas-trust/verify-wasm@node
```

```js
const { verify_trace_json } = require('@atlas-trust/verify-wasm');
const outcome = verify_trace_json(traceJson, bundleJson);
```

### Native CLI

```bash
# Build from source — or download a signed release binary
cargo install --path crates/atlas-verify-cli
atlas-verify-cli verify-trace bundle.json -k pubkey-bundle.json
```

Sample output:

```
✓ VALID — all checks passed
✓ schema-version — trace schema atlas-trace-v1 matches verifier
✓ event-hashes — 5 events, all hashes recomputed-match
✓ event-signatures — 5 signatures verified
✓ parent-links — all parent_hashes resolved within trace
✓ dag-tips — 1 tips, match server claim
✓ anchors — 2 anchor(s) verified against pinned log keys
✓ witnesses — 2 presented / 2 verified
```

No network calls. No talking to our server. Bit-identical determinism — the same input produces byte-identical signing-input bytes whether the verifier runs on Linux, macOS, or as WASM in your browser.

### Zero-build playground

Open [`apps/wasm-playground/`](apps/wasm-playground) — drop in a `*.trace.json` + `*.pubkey-bundle.json`, click **Verify**. Same Rust verifier core, byte-identical to the native CLI.

---

## Status — v1.0.1 (2026-05-12)

**First stable release on npm.** Trust model frozen. Public verification API committed. Semver in force from here. The signed Git tag `v1.0.0` (2026-05-11) is preserved unmodified — v1.0.1 is the byte-identical SemVer-patch that corrects a `Cargo.toml` `workspace.package.repository` field which prevented the initial npm publish. See [CHANGELOG.md](CHANGELOG.md) for the full v1.0.0 → v1.0.1 narrative.

- **347 Rust tests green** across the workspace (`--features hsm`) — covering trust-core unit + anchor-chain adversary + golden trace + per-tenant-keys adversary + Sigstore golden + witness-strict-mode + V1.14 Scope J integration + V1.15 Welle A source-level anti-drift pins
- **SLSA Build L3 provenance** on every `@atlas-trust/verify-wasm` npm publish — OIDC-signed Sigstore Rekor attestation, verifiable via `npm audit signatures`
- **Signed release tags** — every `v*` tag SSH-signed (Ed25519) against in-repo trust root at `.github/allowed_signers`; `tools/verify-tag-signatures.sh` gates the publish lane
- **Consumer-side automation** — [`verify-wasm-pin-check@v1`](.github/actions/verify-wasm-pin-check) GitHub Action re-asserts exact-version pin + lockfile integrity + `npm audit signatures` on every consumer CI run (pure-bash composite, no `dist/index.js`)
- **CSP-hardened browser playground** — strict CSP + SRI + Trusted Types on [`apps/wasm-playground/`](apps/wasm-playground), Cloudflare Workers + Static Assets host with per-response security headers, executable `/csp-report` receiver, AE → R2 daily archive
- **Branch protection on `master`** — no direct push, no force-push (admins included), CODEOWNERS-required review on trust-root surfaces, `verify-trust-root-mutations` status check required
- **Backup distribution channel** — every `v*` tag uploads byte-identical npm-pack tarballs (web + node) + `tarball-sha256.txt` manifest as GitHub Release assets; auditors can `gh release download` and verify SHA against the same SLSA L3 provenance

See [CHANGELOG.md](CHANGELOG.md) for the full V1 wave history (V1.5 → V1.19, ~14 months of trust-property hardening).

---

## How verification works

```
   ┌────────────────────┐                  ┌─────────────────────────────────┐
   │ Atlas Server       │                  │ Auditor's machine (browser/CLI) │
   │                    │                  │                                 │
   │ 1. Sign trace ─────────────tarball───▶ verify-wasm                      │
   │    (Ed25519+COSE)  │                  │                                 │
   │                    │                  │ 2. Recompute event hashes       │
   │ 3. Anchor to ──────────Rekor proof───▶ 4. Validate Rekor inclusion      │
   │    Sigstore Rekor  │                  │    proofs against pinned key    │
   │                    │                  │                                 │
   │ 5. Witness (out ───────cosignature───▶ 6. Verify witness against         │
   │    of process)     │                  │    source-controlled roster     │
   │    cosigns chain   │                  │                                 │
   │    head            │                  │ 7. Emit VerifyOutcome JSON      │
   └────────────────────┘                  │    { valid, witness_failures }  │
                                           └─────────────────────────────────┘
```

**The auditor's machine does the verification.** No round-trip to our server. The trust property is structural, not transactional.

Full architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## Documentation

| Document | What's in it |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, trust property, write/export flows, V1 → V2 boundaries |
| [docs/SECURITY-NOTES.md](docs/SECURITY-NOTES.md) | Defended attack surface, per-test mapping for auditors |
| [docs/OPERATOR-RUNBOOK.md](docs/OPERATOR-RUNBOOK.md) | Production procedures: master-seed import, HSM wave-3 setup, witness commissioning, backup-channel install, CI lane reference |
| [docs/CONSUMER-RUNBOOK.md](docs/CONSUMER-RUNBOOK.md) | Downstream consumer reproducibility: exact-version pinning across npm / pnpm / Bun, SLSA L3 provenance verification, reproduce-from-source fallback |
| [docs/COMPLIANCE-MAPPING.md](docs/COMPLIANCE-MAPPING.md) | Clause-by-clause regulatory mapping (EU AI Act, GAMP 5, ICH E6(R3), DORA, GDPR) |
| [docs/SEMVER-AUDIT-V1.0.md](docs/SEMVER-AUDIT-V1.0.md) | Public-API surface audit pinning the v1.0.0 semver commitment |
| [CHANGELOG.md](CHANGELOG.md) | Full V1 wave history with per-version trust-property additions |

---

## Components

| Component | Path | License |
|---|---|---|
| `atlas-trust-core` | [`crates/atlas-trust-core`](crates/atlas-trust-core) | Apache-2.0 |
| `atlas-verify-cli` | [`crates/atlas-verify-cli`](crates/atlas-verify-cli) | Apache-2.0 |
| `atlas-verify-wasm` | [`crates/atlas-verify-wasm`](crates/atlas-verify-wasm) | Apache-2.0 |
| `atlas-signer` | [`crates/atlas-signer`](crates/atlas-signer) | Apache-2.0 |
| `atlas-witness` | [`crates/atlas-witness`](crates/atlas-witness) | Apache-2.0 |
| `atlas-mcp-server` | [`apps/atlas-mcp-server`](apps/atlas-mcp-server) | Sustainable Use |
| `atlas-web` | [`apps/atlas-web`](apps/atlas-web) | Sustainable Use |

---

## Compliance mappings

- **EU AI Act Annex IV §1(e)** — full provenance trail with named human verifiers
- **GAMP 5 Appendix D11 (July 2025)** — AI/ML system validation in GxP context
- **ICH E6(R3) §7.4** — clinical trial data lineage
- **DORA Art. 11–14** — operational resilience event logging
- **GDPR Art. 22** — automated-decision traceability

Clause-by-clause mapping with inspectable-evidence pointers: [docs/COMPLIANCE-MAPPING.md](docs/COMPLIANCE-MAPPING.md).

---

## Build from source

```bash
# Rust workspace — all crates, full test suite
cargo build --release
cargo test --features hsm   # 347 tests

# End-to-end demo: generate a signed trace + bundle, then verify it
cargo run --example seed_bank_demo -p atlas-signer --release
./target/release/atlas-verify-cli verify-trace \
  examples/golden-traces/bank-q1-2026.trace.json \
  -k examples/golden-traces/bank-q1-2026.pubkey-bundle.json
```

---

## For npm consumers — pinning + provenance verification

If you embed `@atlas-trust/verify-wasm` in production tooling, see [docs/CONSUMER-RUNBOOK.md](docs/CONSUMER-RUNBOOK.md) for the full consumer-side reproducibility guide: exact-version pinning across npm / pnpm / Bun lockfiles, SLSA L3 provenance verification via `npm audit signatures`, the GH-Releases backup-channel install flow, and the reproduce-from-source fallback (clone at tagged commit, `wasm-pack build`, byte-compare) for the both-channels-unreachable scenario.

The drop-in CI assertion lives at [`.github/actions/verify-wasm-pin-check`](.github/actions/verify-wasm-pin-check) — a pure-bash composite GitHub Action that re-asserts every CONSUMER-RUNBOOK §1 trust layer on every consumer build, with fixture-based unit tests + a weekly cron that performs a live `npm install --save-exact @atlas-trust/verify-wasm@latest` + real `npm audit signatures` round-trip to catch publisher-side or Sigstore-side regressions before they reach downstream consumers.

---

## License

Atlas uses a **fair-code split**, modelled on n8n's Sustainable Use License:

- **Verifier crates** (`crates/atlas-trust-core`, `atlas-verify-cli`, `atlas-verify-wasm`, `atlas-signer`, `atlas-witness`) are **Apache-2.0** ([LICENSE-APACHE-2.0](LICENSE-APACHE-2.0)). Any auditor, regulator, or third-party tool can fork, embed, redistribute, or rebuild them with no friction. Apache-2.0 is the standard for sigstore-rs and the Rust crypto-tooling ecosystem — Atlas joins it.

- **Server, web frontend, MCP server** (`apps/`) are licensed under the **Atlas Sustainable Use License** ([LICENSE-SUSTAINABLE-USE](LICENSE-SUSTAINABLE-USE)). Self-host for internal business use is permitted free of charge. Hosting Atlas as an as-a-service offering for third parties, removing attribution, or reselling compliance bundles requires a commercial license.

This split is deliberate: the trust property of Atlas must be structurally verifiable by any auditor, end-to-end, without buying anything from us.

**Commercial licensing inquiries:** [nelson@ultranova.io](mailto:nelson@ultranova.io)

---

<!--
TODO list (after the v1.0.0 npm-publish + public-flip lands):

- [ ] Replace the placeholder image at the top with the 70s-style Atlas icon (à la the Canon-Projekt look — small, retro, monochrome-ish, distinct silhouette). Drop it at `docs/assets/atlas-logo.png`.
- [ ] Once `@atlas-trust/verify-wasm@1.0.0` is on the npm registry, the npm badge resolves automatically (no change needed).
- [ ] Add a `## Citing Atlas` section once the BibTeX entry / Zenodo DOI is minted.
- [ ] Add a one-line link to a short demo video (≤90 s) showing trace generation → in-browser verify.
- [ ] Consider a `## Roadmap` section pointing at V2 (graph-database integration + policy-engine).
-->
