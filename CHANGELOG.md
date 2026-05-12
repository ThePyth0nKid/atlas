# Changelog

All notable changes to Atlas are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
as of v1.0.0.

Atlas ships as a coherent system across multiple workspace crates and packages
(`atlas-trust-core`, `atlas-verify-cli`, `atlas-verify-wasm`, `atlas-signer`,
`atlas-witness`, `@atlas/bridge`, `atlas-web`, `atlas-mcp-server`,
`@atlas-trust/verify-wasm`). Version numbers move in lockstep — a `v1.0.0` tag
covers every workspace member.

The v1.0 public-API surface contract is documented in
[`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md).

## [Unreleased]

_V2-α work in flight on this line. The next release tag will be `v2.0.0-alpha.1` (major-bump pre-release) at the close-out of the V2-α welle bundle, not a v1.x continuation. V2-α-Additive surface items are listed in [`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md) §10. The strategic documentation landings below do not touch the v1.0 public-API surface._

### Added — V2-α Welle 3 (Atlas Projector Skeleton + Canonicalisation Byte-Pin, 2026-05-12)

- **NEW `crates/atlas-projector/` workspace crate** — V2-α Layer-2 graph projection canonicalisation. Public surface: `GraphState` / `GraphNode` / `GraphEdge` types (in-memory representation, `BTreeMap`-backed for load-bearing logical-identifier-sorted canonical iteration); `build_canonical_bytes()` (RFC 8949 §4.2.1 CBOR canonical encoding); `graph_state_hash()` (blake3 over canonical bytes); `ProjectorError` enum (`#[non_exhaustive]`, 5 variants); `PROJECTOR_SCHEMA_VERSION = "atlas-projector-v1-alpha"` const bound into every canonicalisation.
- **NEW `canonical::tests::graph_state_hash_byte_determinism_pin`** — pinned blake3 `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` (754 canonical bytes) for a 3-node + 2-edge fixture with mixed labels and mixed `author_did` presence. Co-equal CI gate with V1's `cose::signing_input_byte_determinism_pin`, V1.7's `anchor::chain_canonical_body_byte_determinism_pin`, V1.9's `pubkey_bundle::bundle_hash_byte_determinism_pin`, and Welle 1's `signing_input_byte_determinism_pin_with_author_did`.
- **19 unit tests** in atlas-projector covering: empty-state hash, single-node hash, multi-node insert-order independence (the load-bearing Welle 2 §3.5 invariant), property-order independence, label-order independence + dedup, `author_did` schema-additive binding into hash, V1 backward-compat for `author_did = None`, float rejection at canonicalisation boundary, dangling-edge structural-integrity rejection, malformed-DID rejection, and the byte-determinism pin.
- **Atlas-projector depends on atlas-trust-core** only for `agent_did::validate_agent_did` cross-validation. Clean DAG — atlas-trust-core does NOT depend on atlas-projector.
- **NEW `.handoff/v2-alpha-welle-3-plan.md`** (~200 lines) — Welle 3 plan-doc with scope, decisions, files table, acceptance criteria, 5-entry risks table, V1-test-impact matrix, and out-of-scope items for V2-α Welles 4-8.
- **MODIFY `Cargo.toml` workspace** — add `"crates/atlas-projector"` member entry.
- **MODIFY `docs/SEMVER-AUDIT-V1.0.md` §10** — new subsection §10.7a listing every new `atlas-projector` `pub` item with V2-α-Additive tag.

### Notes — V2-α Welle 3

- **V1 backward-compat preserved.** All 150 atlas-trust-core unit tests + 4 byte-determinism CI pins (V1's `signing_input_byte_determinism_pin`, V1.7's `chain_canonical_body_byte_determinism_pin`, V1.9's `bundle_hash_byte_determinism_pin`, Welle 1's `signing_input_byte_determinism_pin_with_author_did`) pass byte-identical after Welle 3. Zero regression.
- **Container choice is load-bearing.** `GraphState.nodes` and `.edges` use `BTreeMap` keyed by logical identifier (`entity_uuid` / `edge_id`) — iteration is sorted automatically per Rust stdlib. The Welle 2 §3.5 caveat ("`@rid` is insert-order, NOT logical identity anchor") is therefore structurally impossible to violate from within this crate's API.
- **Out of scope (deferred to later wellen):** events.jsonl reading + idempotent upsert (Welle 4), `ProjectorRunAttestation` event-kind emission (Welle 4), ArcadeDB driver integration (Welle 5), projector-state-hash CI gate enforcement (Welle 6), parallel-projection design for >10M event scenarios (Welle 5).
- **V2-α progress: 3 of 5-8 wellen shipped** (Welle 1 Agent-DID + Welle 2 DB spike + Welle 3 projector skeleton).

### Added — V2-α Welle 2 (ArcadeDB vs FalkorDB Comparative Spike, 2026-05-12)

- **`docs/V2-ALPHA-DB-SPIKE.md`** (new, ~500 lines) — master-resident V2-α DB-choice decision source-of-truth. Comparative analysis of ArcadeDB (Apache-2.0) vs FalkorDB (SSPLv1) across 10 dimensions: license (SSPLv1 §13 vs Apache-2.0 §4-§5), Cypher subset coverage, property graph model, idempotent upsert pattern, multi-tenant isolation, schema determinism, performance characteristics, operational considerations, vendor risk, and 5 Atlas-specific decision factors (projection-determinism cost, author_did stamping, ProjectorRunAttestation hooks, V2-β Mem0g integration, V2-γ federation-witness property-visibility).
- **`.handoff/v2-alpha-welle-2-plan.md`** (new, ~180 lines) — Welle 2 plan-doc with scope, decisions, files table, spike-doc target outline, acceptance criteria, risks, and out-of-scope items.

### Changed — V2-α Welle 2 (Strategic DB-Choice Flip)

- **V2-α DB primary flipped from FalkorDB to ArcadeDB** per `DECISION-DB-4` (new). Recommendation confidence MEDIUM-HIGH; deciding factor is license compatibility (SSPLv1 §13 vs Apache-2.0) for Atlas's planned open-core hosted-service monetization tier. Secondary factors: projection-determinism canonicalisation cost (~30% lower with ArcadeDB's `ORDER BY @rid` + schema-required mode) and self-hosted-tier deployment simplicity (ArcadeDB embedded mode lets Atlas ship as single-process server). Reversal cost MEDIUM (re-projection from authoritative Layer 1 `events.jsonl`, 1-2 sessions of projector rewrite, zero customer downtime via dual-write).
- **`docs/V2-MASTER-PLAN.md` §3 + §4 R-L-02 + §6 V2-α Foundation + §11 Reference Pointers** updated to reflect the ArcadeDB-primary flip. ASCII Three-Layer Architecture diagram updated.
- **`.handoff/decisions.md`** — `DECISION-DB-1` (original Kuzu→ArcadeDB-fallback) annotated as superseded; new `DECISION-DB-4` documents the primary flip with full rationale, confidence level, and reversal-cost analysis. Now 23 decisions documented.

### Notes — V2-α Welle 2

- **Spike methodology: public-knowledge-based research, no actual benchmarks executed.** If Welle 3 (Projector skeleton) implementation surfaces Cypher-subset incompatibilities in ArcadeDB OR if Nelson commissions a Welle 2b actual-benchmark validation, the recommendation may be revisited.
- **Counsel-validated SSPLv1 §13 opinion** remains on Nelson's parallel counsel-engagement track and is pre-V2-α-public-materials blocking per Master Plan §5.
- **V2-α progress: 2 of 5-8 wellen shipped (Welle 1 Agent-DID schema + Welle 2 DB spike).** Welle 3 candidate: Atlas Projector skeleton against locked ArcadeDB choice.

### Added — V2-α Welle 1 (Agent-DID Schema Foundation, 2026-05-12)

- **`crates/atlas-trust-core/src/agent_did.rs`** (new module) — W3C-DID parser, validator, and presentation-layer helpers for `did:atlas:<lowercase-hex-32-bytes>` agent identities. Public surface: `AGENT_DID_PREFIX`, `agent_did_for`, `parse_agent_did`, `validate_agent_did`. 13 unit tests covering positive + negative format-validation cases, parse roundtrip, structured-error reasons. Re-exported at crate root.
- **`AtlasEvent.author_did: Option<String>`** field (`crates/atlas-trust-core/src/trace_format.rs`) — optional agent-identity binding on every signed event. When present, canonically bound into the signing-input alongside `kid` (Phase 2 Security H-1), providing cross-agent-replay defence in addition to V1's cross-workspace-replay defence. When absent, events remain V1-shaped and byte-identical to pre-Welle-1 output.
- **`TrustError::AgentDidFormatInvalid { did, reason }`** new variant — structured reject path for malformed `author_did` values. Verifier surfaces this before signature-check so auditor tooling sees the precise failure mode. Additive under `#[non_exhaustive]` per `SEMVER-AUDIT-V1.0.md` §8.
- **`cose::tests::signing_input_byte_determinism_pin_with_author_did`** — new V2-α byte-determinism CI pin. Locks exact CBOR bytes for fixture event with `author_did = Some(...)`. Map header is `a8` (8 pairs); `author_did` entry sorts LAST per RFC 8949 §4.2.1 (longest encoded-key length, 11 bytes). The V1 pin `cose::tests::signing_input_byte_determinism_pin` is preserved byte-identical — V1-shaped events produce identical CBOR pre- and post-Welle-1.
- **`crates/atlas-trust-core/tests/agent_did_integration.rs`** (new integration test) — 4 end-to-end test cases: (1) sign+verify with `author_did = Some(...)`, (2) V1 backward-compat (no `author_did`), (3) malformed DID rejected at verify-time with `AgentDidFormatInvalid`, (4) cross-agent-replay defence (tampered well-formed DID fails signature check).

### Changed — V2-α Welle 1

- **`cose::build_signing_input` signature** — added trailing parameter `author_did: Option<&str>`. Callers passing `None` produce byte-identical CBOR to V1 (V1 byte-determinism pin holds unchanged). Source-break for direct callers; all 15 in-tree callers updated (atlas-signer CLI, atlas-signer demo, hashchain inner verify, verify.rs main loop, 6 integration tests).
- **`verify_trace` pre-signature-check hardening** — when `event.author_did` is `Some(_)`, format-validates against `did:atlas:<64-lowercase-hex>` shape and rejects with `AgentDidFormatInvalid` before downstream signature/hash checks. V1 events without `author_did` follow the unchanged verifier path.

### Notes — V2-α Welle 1

- **Workspace version unchanged at `1.0.1`.** A major-bump release (`v2.0.0-alpha.1` candidate) is deferred to the close-out of the V2-α welle bundle per [`.handoff/v2-alpha-welle-1-plan.md`](.handoff/v2-alpha-welle-1-plan.md) §"Decisions". Welle 1 lands on master; the version tag waits for Projector + FalkorDB + content-hash separation (if counsel-approved) + Agent-DID-end-to-end on atlas-signer CLI to ship as a coherent v2.0.0-alpha.1.
- **Wire-compat break for V1.0 verifiers reading V2-α events with `author_did = Some(...)`** is by design and documented in `docs/SEMVER-AUDIT-V1.0.md` §10. V1.0 verifiers deserialize via `#[serde(deny_unknown_fields)]` and will surface `unknown_field("author_did")`. V1-shaped events (no `author_did`) remain forward-compatible across both verifier generations.
- **Trust invariant preserved:** `cose::tests::signing_input_byte_determinism_pin` retains its V1 pinned hex byte-identically. All 146 atlas-trust-core unit tests + 4 new integration tests + the full workspace test suite pass green. Zero V1 regression.

### Documentation — V2 Strategic Planning (2026-05-12)

- **`docs/V2-MASTER-PLAN.md`** (new, ~300 lines) — master-resident strategic plan for Atlas V2. Distilled from Master Vision v1 with Welle decomposition tied to concrete PR-Wellen (V2-α / V2-β / V2-γ / V2-δ, total 14–20 sessions plus 6–8 weeks counsel-engagement in parallel with V2-α), top-5 V2-α blocking risks, 7-demo programme with hero-CTA-inversion (Demo 2 Continuous Regulator Witness above-the-fold primary), and explicit success criteria. Companion to `docs/WORKING-METHODOLOGY.md`.
- **`docs/WORKING-METHODOLOGY.md`** (new, ~200 lines) — reusable 4-phase iteration pattern (Foundation Docs → Multi-Angle Critique → Synthesis → Plan Documentation) with 8-entry anti-pattern table and explicit "when to skip" rules. Use for future Großthemen (e.g. post-quantum migration, V3 architecture). Independent versioning from per-Großthema Master Plans.
- **`.handoff/v2-master-vision-v1.md`** (new on master, ~615 lines) — Phase-3 synthesis output mirrored from PR #62 draft-branch for master-reference-ability. 15-section consolidated V2 vision including factual corrections from Phase-2 critique (EU AI Liability Directive WITHDRAWN Feb 2025 → fallback regime is Product Liability Directive 2024/2853; "independently verifiable" Art. 12 phrasing replaced with verbatim text; Art. 18 / Art. 19 conflation fixed). Full rationale for everything in V2-MASTER-PLAN.md.
- **`.handoff/decisions.md`** (new on master, ~284 lines) — Phase-3 decision log with 22 explicit ACCEPT/MODIFY/DEFER entries. Each carries crit-source attribution, reversibility tag (HIGH/MEDIUM/LOW), and review-after trigger. Cross-referenced from V2-MASTER-PLAN + Master Vision via stable `DECISION-<DOMAIN>-<N>` IDs.
- **`.handoff/v2-session-handoff.md`** (updated, +400 lines) — Phase 1+2+3+4 ALL SHIPPED state, V2-α Welle 1 pre-flight checklist, branch-and-PR diagram showing master-resident outputs and permanently-draft work-product archives (#59/#61/#62).

**No v1.0 public-API surface touched.** Per SemVer contract committed at v1.0.0, these are pure documentation additions. Reproducibility, signed-tag chain, npm `@atlas-trust/verify-wasm@1.0.1` byte-identical state — all unchanged.

## [1.0.1] — 2026-05-12

**SemVer-patch release — first version published to the npm registry.** No code changes; trust property, public API, and signed-tag chain are byte-identical to v1.0.0. This release corrects a `Cargo.toml` `workspace.package.repository` field that pointed at a stale organisation path (`https://github.com/ultranova/atlas`) instead of the canonical `https://github.com/ThePyth0nKid/atlas`. wasm-pack derives `package.json`'s `repository.url` from that Cargo field; npm's SLSA Build L3 provenance validator rejected the v1.0.0 publish attempt because the package.json URL did not match the GitHub Actions OIDC token's source-repository claim (`422 Unprocessable Entity — Error verifying sigstore provenance bundle: Failed to validate repository information`).

### Fixed — V1.19 Welle 14a

- `Cargo.toml` `workspace.package.repository`: `https://github.com/ultranova/atlas` → `https://github.com/ThePyth0nKid/atlas`. Flows through `wasm-pack build` into the generated `package.json` `repository.url`; the new value matches the OIDC `repository` claim emitted by GitHub Actions for `ThePyth0nKid/atlas`, unblocking SLSA Build L3 provenance verification.
- `docs/ARCHITECTURE.md` reproduce-from-source `git clone` URL updated to match.

### Changed — V1.19 Welle 14a

- Workspace version bump 1.0.0 → 1.0.1 (single source of truth via `workspace.package.version`; all 5 crates inherit through `version.workspace = true`).
- npm version bumps for `atlas-web`, `atlas-mcp-server`, `@atlas/bridge`, root monorepo manifest, and the `apps/atlas-mcp-server/src/index.ts` MCP server registration version.

### Notes

- The signed Git tag `v1.0.0` (`e97c025`, SSH-Ed25519 `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`) is preserved unmodified. Atlas's tag-immutability invariant is upheld: published-but-unreachable artefacts are corrected by SemVer-patch, not by retroactive tag mutation.
- The GitHub Release for `v1.0.0` remains live as a historical record with its byte-identical npm-pack tarballs; the release notes flag that the npm publish did not land for this tag and direct consumers to `npm install @atlas-trust/verify-wasm@1.0.1` (or `@latest`) instead.
- No `Locked` public-API surface in [`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md) is touched. Per the SemVer contract committed at v1.0.0, this is a strict patch-level release.

## [1.0.0] — 2026-05-11

**v1.0.0 Release Summary** — Atlas's first SemVer-stable public release. The verifier crate (`atlas-trust-core`) is feature-complete across all V1.0–V1.19 trust-property increments: Ed25519 + COSE_Sign1 + deterministic CBOR + 7 base check categories (V1.0), Sigstore Rekor anchoring with pinned log-pubkey (V1.5), anchor-chain linkage (V1.7), HKDF per-tenant key derivation (V1.9), opt-in strict modes for per-tenant keys / anchors / anchor-chain / witness-threshold / strict-chain (V1.10 + V1.13 + V1.19 Welle 9), HSM-optional signing via PKCS#11 (V1.10 wave-2 + V1.12 wave-3), witness cosignature attestation (V1.13), production hosting on Cloudflare Workers (V1.16), SSH-Ed25519 tag-signing + trust-root-mutation defence (V1.17), defence-in-depth + multi-issuer Sigstore tracking (V1.18), browser-rendering UI E2E coverage with WCAG 2.1 AA a11y (V1.19 Welle 11), and the user-facing `POST /api/atlas/write-node` HTTP write surface (V1.19 Welle 1). The `@atlas-trust/verify-wasm` package on npm provides the same trust property in the browser as the native CLI. The v1.0 public-API surface contract is documented in [`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md); from this release forward, any breaking change to a `Locked` item triggers a SemVer-major bump.

### Added — V1.19 Welle 13 (this release)

- Cargo workspace version bump 0.1.0 → 1.0.0 (single source of truth via `workspace.package.version`; all 5 crates inherit through `version.workspace = true`).
- npm version bumps for `atlas-web`, `atlas-mcp-server`, `@atlas/bridge`, root monorepo manifest.
- `@atlas-trust/verify-wasm@1.0.0` build pipeline (`wasm-publish.yml`) auto-fires on signed-tag push to produce byte-identical `npm pack` tarballs (web + node targets) plus a `tarball-sha256.txt` manifest, uploaded to the GitHub Release as backup-channel assets per V1.15 Welle B. **Note (2026-05-12):** the npm-registry publish step for `v1.0.0` did not land due to a `Cargo.toml` repository-URL mismatch surfaced by npm's SLSA Build L3 provenance validator (see v1.0.1 entry). The `v1.0.0` Sigstore Rekor provenance attestation (logIndex `1510551161`, re-emitted as logIndex `1517641691` / `1517706827` across retry runs) was orphaned by the failed publish — it is content-addressed against the wasm bytes and remains audit-traceable. Consumers should install `@atlas-trust/verify-wasm@1.0.1` (or `@latest`) for the byte-identical trust property delivered through the npm registry.
- Signed Git tag `v1.0.0` via the V1.17 SSH-Ed25519 path (key `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`).

### Added — V1.19 Welle 12 (PR #48, commit cdf89e84)

- `--require-strict-chain` enabled in `apps/atlas-web/scripts/e2e-write-roundtrip.ts` round-trip (Welle 10 contract symmetric pair): atlas-web write surface now exercises the verifier-side single-writer-per-workspace gate end-to-end.
- New evidence-row + `Strict flags:`-anchored flag-name regex assertions in atlas-web e2e (mirror Welle 10 smoke.ts anti-drift pattern).
- New CLI integration test `crates/atlas-verify-cli/tests/strict_mode.rs::strict_chain_passes_linear_bank_trace` — happy-path coverage at the CLI surface on the 5-event linear bank-q1-2026 fixture.
- Public-API SemVer audit: new `docs/SEMVER-AUDIT-V1.0.md` documenting every public Rust type, CLI flag, HTTP wire shape, npm export, MCP tool, on-disk format, and operator env-var with risk-tag (Locked / Locked-Behind-Flag / Internal-but-Exported / Defer-Decision).
- This `CHANGELOG.md` consolidating the full V1.0-baseline through V1.19 Welle 12 ship history.

### Fixed — V1.19 Welle 12

- `atlas-web-playwright.yml` job ID renamed from `playwright` to `atlas-web-playwright` so the GitHub check-run name matches the master-ruleset `required_status_checks` context (Welle-11 fallout — GitHub derives check-run names from job ID, not workflow `name`). Pre-merge fixup commit `6040ee2`.
- `atlas-web-playwright.yml` paths filter extended with `.handoff/**` so docs-only PRs can trigger the required check (post-merge fixup commit `cb6b930`).

## Security Advisories

The following findings were discovered and remediated during Atlas's pre-1.0 development. Documented here for downstream CVE-prep workflows. Both findings were closed in-commit during the V1.19 Welle 9 review pass (2026-05-09, PR #42, commit e650f93); v1.0.0 is the first version with the documented audit trail.

### ATLAS-2026-001 (V1.19 Welle 9 SR-H-1): Empty-trace strict-chain silent pass

- **Severity:** HIGH (CVSS-equivalent: integrity / auditor-trust). Hypothetical severity for the vulnerability pattern; see "Affected" below — no public release was ever affected.
- **Affected:** none publicly. The bug existed only in a pre-push intermediate working-tree state of the V1.19 Welle 9 implementation; the fix landed in-commit with the flag's introduction (commit `e650f93`, PR #42, both the unsquashed push `41afebc` and the squash-merge `e650f93` already include the fix). No public release contains the unfixed pattern.
- **First safe version:** v1.0.0 (this release) is the first version with the documented audit trail. The flag itself shipped in V1.19 Welle 9 under v0.1.0 — already with the fix.
- **Description:** An earlier draft of `check_strict_chain` used the shape `if events.len() != 1 { ... }` which would have silently passed an empty trace under strict mode. Strict mode pins five properties including "non-empty"; without this, an attacker who stripped events from a bundle could pass strict mode silently.
- **Remediation:** `check_strict_chain` now returns `TrustError::StrictChainViolation` with the diagnostic "trace has no events (a linear chain requires at least 1 genesis event)" as the first check, before any per-event analysis.

### ATLAS-2026-002 (V1.19 Welle 9 SR-H-2): Self-reference 1-cycle bypass

- **Severity:** HIGH (CVSS-equivalent: integrity / auditor-trust). Hypothetical severity for the vulnerability pattern; see "Affected" below.
- **Affected:** none publicly. Same disposition as ATLAS-2026-001: the bug existed only in a pre-push intermediate working-tree state; both the unsquashed push `41afebc` and the squash-merge `e650f93` (PR #42) already include the fix.
- **First safe version:** v1.0.0 (this release) is the first version with the documented audit trail.
- **Description:** A 1-event trace where the event lists its own `event_hash` as a parent (cryptographically infeasible after a successful `check_event_hashes` pass under blake3 preimage resistance, but a defence-in-depth concern when `check_strict_chain` is called standalone) would have failed with a misleading "found 0 genesis events" message instead of the structured "self-reference cycle" diagnostic.
- **Remediation:** Self-reference check positioned FIRST among per-event checks in `check_strict_chain`, so a 1-event self-ref reports the cycle diagnostic correctly before the genesis-count check fires.

## [0.1.0] — pre-1.0 development history (2026-04-27 to 2026-05-11)

The v0.1.0 line represents Atlas's pre-1.0 development history across V1.0 baseline through V1.19 Welle 12. All entries below shipped under the v0.1.0 Cargo + npm version while features and trust properties were being assembled; v1.0.0 (above) is the first version with a frozen public-API contract per `docs/SEMVER-AUDIT-V1.0.md`.

### Added — V1.19 Welle 11 (PR #46, commit 8bc9d88)

- Playwright UI E2E coverage for `apps/atlas-web`: 19 tests × Chromium + Firefox = 38 cases. Three spec files: `tests/e2e/home.spec.ts` (4 cases, LiveVerifierPanel state-machine), `write.spec.ts` (11 cases, WriteNodeForm full happy-path + error-paths + persistence), `a11y.spec.ts` (4 cases, WCAG 2.1 Level AA + keyboard tab-order).
- WCAG 2.1 AA accessibility coverage via `@axe-core/playwright`.
- Frozen `data-testid` test seam: 10 identifiers on `WriteNodeForm.tsx` + 6 + dynamic pattern on `LiveVerifierPanel.tsx`, documented via JSDoc.
- New CI lane `.github/workflows/atlas-web-playwright.yml` (Ubuntu, Chromium + Firefox, paths-filtered) joined the master-ruleset required-check set.
- `role="alert"` on error display, `role="status"` on success card, `aria-hidden="true"` on decorative ✓/✗ glyphs.
- New `--accent-trust-brand` color-token alias preserving the original sigstore-green `#3fbc78` for non-text branding surfaces.

### Fixed — V1.19 Welle 11

- Five color tokens in `apps/atlas-web/src/app/globals.css` corrected for WCAG 2.1 AA contrast on `bg-muted` and on the 15%-mix StatusBadge background: `--foreground-muted = #475569`, `--accent-trust = #166534` (green-800; buffered for Firefox `color-mix()` gamma rounding), `--accent-warn = #b45309`, `--accent-danger = #b91c1c`, `--accent-info = #1d4ed8`.

## V1.19 Welle 10 — 2026-05-11 (PR #44, commit 1e3e89f)

### Added

- `--require-strict-chain` enabled in `apps/atlas-mcp-server` smoke (step 6 + step 7). Single-writer-per-workspace CI gate active across three lanes: `hsm-wave3-smoke.yml`, `sigstore-rekor-nightly.yml`, local `pnpm smoke`.
- Anti-drift assertions in smoke.ts: evidence-row pin matching `/✓ strict-chain — \d+ event\(s\) form a strict linear chain/`, `Strict flags:`-anchored flag-name pins (`/Strict flags:[^\n]*require_strict_chain/`).
- Step 7 augmented with strict-chain alongside existing `--require-per-tenant-keys`.

### Fixed

- Property numbering in step-7 rationale comment corrected to match the canonical `crates/atlas-trust-core/src/hashchain.rs::check_strict_chain` doc-comment (property 2 = "exactly one genesis"; prior draft used "(3)" which was wrong).

## V1.19 Welle 9 — 2026-05-09 (PR #42, commit e650f93)

### Added

- Verifier-side `--require-strict-chain` opt-in flag on `atlas-verify-cli` and `VerifyOptions::require_strict_chain` on the library surface.
- `crates/atlas-trust-core::hashchain::check_strict_chain` free function pinning five properties: trace non-empty, exactly one genesis, every non-genesis has exactly one parent, no event referenced as parent by more than one other event (no sibling-fork), no event lists its own hash as parent (no self-reference).
- New `TrustError::StrictChainViolation { msg }` variant (under existing `#[non_exhaustive]`) for auditor tooling pattern-matching.
- 9 hashchain strict-chain unit tests covering empty-trace, single-genesis, two-event-linear, linear-three-events, two-genesis, zero-genesis, sibling-fork, DAG-merge, self-reference.

### Security

- SR-H-1 (empty trace silently passed strict-chain) — closed in-commit with structured `StrictChainViolation` diagnostic.
- SR-H-2 (1-event self-referential event_hash bypassed property-2 check) — closed by positioning self-reference check FIRST in `check_strict_chain`.
- CR-1 (strict-chain over preflight-failed graph could mislead) — gated on `event_hashes_ok && parent_links_ok`; explicit "skipped" evidence row otherwise.
- CR-2 (`Result<(), String>` deviated from module convention) — refactored to `TrustResult<()>`.

## V1.19 Welle 8 — 2026-05-09 (PR #40, commit 1d1fe69)

### Added

- atlas-web write-surface HTTP-level edge-case test suite: 42 assertions across `scripts/e2e-write-edge-cases.ts`. Four classes covered: (A) 4xx malformed-input rejections (Zod `.strict()`, prototype pollution, deeply-nested attributes); (B) Content-Length 256 KB cap → 413; (C) per-workspace mutex serialisation under 8 parallel POSTs; (D) workspace_id boundary class (POSIX/Windows traversal, embedded delimiters, length 0/129, GET endpoint mirror).
- `__REQUEST_BODY_MAX_BYTES_FOR_TEST` export on `apps/atlas-web/src/app/api/atlas/write-node/route.ts` for source/test drift prevention.

### Security

- FINDING-6 (chain-validation oracle used set-membership; would silently accept sibling-fork DAG) — hardened to immediate-predecessor comparison (`parents[0] === stored[i-1].event_hash`), the same regression mode Welle 9 + Welle 10 now also catch at the verifier and CI-lane surfaces.

## V1.19 Welle 7 — 2026-05-09 (PR #38, commit 19995ed)

### Added

- Shared `PATH_SEGMENT` + `POSIX_PATH_LOOKBEHIND` constants on `@atlas/bridge/src/signer.ts`, re-exported via the frozen `__redactPathConstantsForTest` test seam.

### Fixed

- Source/test drift hazard on the `redactPaths` POSIX regex — the test now imports the constants instead of redefining literals, with `Object.isFrozen` + 2 exact-equality golden assertions pinning the contract.

## V1.19 Welle 6 — 2026-05-09 (PR #36, commit 6d99012)

### Fixed

- `redactPaths` POSIX lookbehind tightened: dotted-relative paths (`./foo/bar.ts`, `../workspace/x`) now pass through verbatim — they expose only repo-internal filenames, outside the absolute-layout-disclosure threat model. Absolute paths containing dotfile segments (`/home/user/.cache/foo`) MUST still redact.

## V1.19 Welle 5 — 2026-05-09 (PR #34, commit 2c1f6f2)

### Changed

- `@atlas/bridge::ulid` refactored to pure-function + factory + singleton trio: `nextUlid(state, now, randomSource)` is pure, `createUlid({ now, randomSource })` produces a factory, `ulid()` is the singleton backward-compat wrapper. Closes the immutability convention violation in the prior implementation.

### Added

- 25 ulid contract assertions across 7 sections (purity, monotonicity, clock-advance reset, factory isolation, ms-collision, Crockford-base32 sortability, byte-rollover guard, boundary guards).

## V1.19 Welle 4 — 2026-05-09 (PR #32, commit aefde84)

### Added

- 60-second TTL cache for `resolveSignerBinary()` resolution. cwd-drift hardening: cache key includes `process.cwd()` so a `chdir` invalidates the entry.
- 12 signer-cache test assertions using synthetic clock injection via `__signerBinaryCacheForTest.setClock`.

## V1.19 Welle 3 — 2026-05-08 (PR #30, commit 02327193)

### Fixed

- `redactPaths` POSIX path-pattern tightened against false positives (URLs, fractions, dates).
- `storage.ts` duplicate definition collapsed.

## V1.19 Welle 2 — 2026-05-08 (PR #28, commit 2f726f3)

### Added

- New workspace package `packages/atlas-bridge/` (`@atlas/bridge`) extracted from inline atlas-mcp-server / atlas-web bridge code. Single source of truth for the TS-to-Rust-signer bridge plus on-disk JSONL DAG.

### Changed

- Bridge `package.json` deliberately has NO `"source"` export — consumers always resolve via `dist/`. CI runs `pnpm --filter @atlas/bridge build` before consumer tsc.

## V1.19 Welle 1 — 2026-05-08 (PR #26, commit 3853c64)

### Added

- atlas-web write surface: `POST /api/atlas/write-node` (Zod `.strict()` validation, per-workspace mutex, atlas-signer subprocess for per-tenant signing) + `GET /api/atlas/write-node?workspace_id=…` for kid-preview.
- `apps/atlas-web/scripts/e2e-write-roundtrip.ts` — end-to-end round-trip from Request → JSONL → atlas-verify-cli `--require-per-tenant-keys` → ✓ VALID.

## V1.18 (2026-04 / -05) — Defence-in-Depth Trust Posture

### Added

- Welle A: trust-root mutation pin (`tools/verify-trust-root-mutations.sh`, 17 cases, 18 PROTECTED_SURFACE paths via CODEOWNERS).
- Welle B (1–8): SSH-Ed25519 commit + tag signing pipeline (`tools/test-tag-signatures.sh`, 13 cases). Repository Rulesets with required status checks. Master ruleset migrated from classical branch protection.

## V1.17 — SSH-Ed25519 Tag Signing

### Added

- SSH-Ed25519 signing pathway for tags (key `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`). GitHub Repository Rulesets with required signed commits.

## V1.16 — Production Hosting

### Added

- Welle C: Cloudflare Workers hosting for `playground.atlas-trust.dev`. CSP + COEP/COOP headers (`tools/playground-csp-check.sh`). Worker-emitted headers + silent-204 receiver pattern (ADR-007).

## V1.14 — Witness Wave-C JSON Surface

### Added

- Scope J: `VerifyOutcome.witness_failures: Vec<WitnessFailureWire>` with `#[serde(default)]` for additive wire compat. Per-witness stable `reason_code` for auditor tooling.

## V1.13 — Witness Cosignature Attestation

### Added

- `crates/atlas-witness` binary. `WitnessSig` type, `ATLAS_WITNESS_V1_ROSTER` pinned roster.
- `--require-witness <N>` flag on atlas-verify-cli. Threshold-based witness coverage check (kid-distinct verified Ed25519 signatures across `anchor_chain`).
- `TrustError::BadWitness` variant; duplicate-kid defence.

## V1.12 — Wave-3 Sealed-Per-Workspace Signer

### Added

- atlas-signer wave-3 dispatch: sealed-per-workspace keys via PKCS#11 v3.0. `ATLAS_HSM_WAVE3_OPT_IN` env-var opt-in. Three-layer dispatcher (dev-seed → wave-2 master-HKDF → wave-3 sealed-per-workspace).
- CI lane `.github/workflows/hsm-wave3-smoke.yml` (SoftHSM2-backed).

## V1.11 — Sigstore Rekor V1 Public-Trust Anchor

### Added

- Sigstore Rekor v1 verification path with multi-issuer support. `crates/atlas-trust-core::anchor::SIGSTORE_REKOR_V1.tree_id_roster`. ECDSA P-256 over RFC 6962 SHA-256 inclusion proofs.
- `.github/workflows/sigstore-rekor-nightly.yml` nightly live-Sigstore lane.

## V1.10 — Strict-Mode Surface

### Added

- Wave 1: `--require-per-tenant-keys`, `--require-anchors`, `--require-anchor-chain` on atlas-verify-cli. `VerifyOptions` struct surface.
- Wave 2: `crates/atlas-signer/src/hsm/` PKCS#11 v3.0 master-HKDF backend.

## V1.9 — Per-Tenant Kid Derivation

### Added

- HKDF-SHA256 per-tenant Ed25519 key derivation from a single master seed (info string `"atlas-anchor-v1:" + workspace_id`).
- `PER_TENANT_KID_PREFIX = "atlas-anchor:"` constant. `perTenantKidFor`, `parse_per_tenant_kid` helpers.
- `ATLAS_DEV_MASTER_SEED` env-var positive opt-in.

## V1.7 — Anchor-Chain Linkage

### Added

- `AnchorChain` type with internal-consistency verification. `chain_head_for` + `ANCHOR_CHAIN_DOMAIN` constants. `crates/atlas-trust-core::anchor` module.
- `--require-anchor-chain` strict-mode flag.

## V1.6 — Sigstore Rekor Compatibility

### Added

- p256 + sha2 dependencies for ECDSA P-256 over RFC 6962 SHA-256 (Rekor checkpoint signatures).

## V1.5 — Anchor Inclusion Proofs

### Added

- `AnchorEntry`, `AnchorBatch` wire-format types. `--require-anchors` strict-mode flag.

## V1.0 baseline through V1.4

Pre-V1.5 foundations: trace_format (`AtlasEvent`, `AtlasTrace`, `PubkeyBundle`), hashchain (event_hash recompute, parent_links, DAG-tips computation), COSE_Sign1 + ed25519-dalek signing, Zod-schema validation at trust boundaries, JSONL append-only storage.

---

[Unreleased]: https://github.com/ThePyth0nKid/atlas/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/ThePyth0nKid/atlas/releases/tag/v1.0.0
[0.1.0]: https://github.com/ThePyth0nKid/atlas/commits/master
