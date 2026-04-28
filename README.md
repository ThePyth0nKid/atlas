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

**V1.6 shipped — live Sigstore submission + offline-complete verification.**

Trust-core crate + Rekor anchoring (V1.5 mock-issuer and V1.6 live Sigstore Rekor v1).
Current state:

- 73 Rust tests green: 41 unit + 13 integration adversary tests + 5
  Sigstore round-trip golden tests in `atlas-trust-core`, plus 14
  issuer/anchor tests in `atlas-signer` (mock-Rekor + live-Sigstore
  wiremock round-trip + Atlas anchoring pubkey PEM pin)
- Signing-input is deterministic CBOR per RFC 8949 §4.2.1
  (length-first map sort, no floats, byte-pinned golden)
- Pubkey-bundle hash is canonical-JSON, byte-pinned golden — silent
  bundle-rotation drift trips the test before reaching production
- Anchor verification: RFC 6962-style Merkle inclusion proofs +
  Ed25519-signed checkpoints, validated against a pinned log pubkey.
  Anchored objects: `bundle_hash` (defends bundle swap) and `dag_tip`
  (defends tail truncation). Lenient by default,
  `VerifyOptions::require_anchors` strict mode for high-assurance audit
- `workspace_id` bound into the signing-input — cross-workspace replay is
  structurally impossible
- Constant-time hash equality, alg-downgrade rejection, RFC 3339 timestamp
  validation, duplicate-event-hash detection, `deny_unknown_fields` on the
  wire format
- Bank demo bundle verifies `✓ VALID` end-to-end through both the native
  CLI and the in-browser WASM verifier, including
  `✓ anchors — N anchor(s) verified against pinned log keys`

V1.6 ships live Sigstore submission: `atlas-signer anchor --rekor-url https://rekor.sigstore.dev`
anchors events against the public Sigstore Rekor v1 log. The verifier accepts both V1.5
mock-Rekor anchors (for offline demos) and V1.6 Sigstore anchors (for production audit
trails). Graph-database integration and policy-engine follow in V2.

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — full system design,
  trust property, write/export flows, V1/V1.5/V1.6/V2 boundaries.
- [docs/SECURITY-NOTES.md](docs/SECURITY-NOTES.md) — defended attack
  surface, per-test mapping for auditors.
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
```

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
  `atlas-verify-wasm`, `atlas-signer`) are **Apache-2.0**. Any auditor,
  regulator, or third-party tool can fork, embed, redistribute, or rebuild
  them with no friction. Apache-2.0 is the standard for sigstore-rs and the
  Rust crypto tooling ecosystem — Atlas joins it.

- **Server, web frontend, and MCP server** (`apps/`) are licensed under the
  **Atlas Sustainable Use License** (see `LICENSE-SUSTAINABLE-USE`).
  Self-host for internal business use is permitted free of charge. Hosting
  Atlas as an as-a-service offering for third parties, removing attribution,
  or reselling compliance bundles requires a commercial license.

This split is deliberate: the trust-property of Atlas must be structurally
verifiable by any auditor, end-to-end, without buying anything from us.

Commercial licensing inquiries: nelson@ultranova.io
