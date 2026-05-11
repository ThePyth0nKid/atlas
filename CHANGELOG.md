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

### Added — V1.19 Welle 12 (this entry, finalised at v1.0.0 tag)

- `--require-strict-chain` enabled in `apps/atlas-web/scripts/e2e-write-roundtrip.ts` round-trip (Welle 10 contract symmetric pair): atlas-web write surface now exercises the verifier-side single-writer-per-workspace gate end-to-end.
- New evidence-row + `Strict flags:`-anchored flag-name regex assertions in atlas-web e2e (mirror Welle 10 smoke.ts anti-drift pattern).
- New CLI integration test `crates/atlas-verify-cli/tests/strict_mode.rs::strict_chain_passes_linear_bank_trace` — happy-path coverage at the CLI surface on the 5-event linear bank-q1-2026 fixture.
- Public-API SemVer audit: new `docs/SEMVER-AUDIT-V1.0.md` documenting every public Rust type, CLI flag, HTTP wire shape, npm export, MCP tool, on-disk format, and operator env-var with risk-tag (Locked / Locked-Behind-Flag / Internal-but-Exported / Defer-Decision).
- This `CHANGELOG.md` consolidating the full V1.0-baseline through V1.19 Welle 12 ship history.

## [0.1.0] — V1.19 Welle 11 ship state (2026-05-11)

The v0.1.0 line represents Atlas's pre-1.0 development history. The shipped feature set is functionally v1.0-ready; v1.0.0 (forthcoming as V1.19 Welle 13) bumps version numbers and pins the SemVer contract.

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
[0.1.0]: https://github.com/ThePyth0nKid/atlas/releases/tag/v0.1.0
