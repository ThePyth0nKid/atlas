# V2-α Welle 8 — Plan-Doc (v2.0.0-alpha.1 Ship)

> **Status: DRAFT 2026-05-13.** Awaiting Nelson's confirmation before merge.
> **Master Plan reference:** `docs/V2-MASTER-PLAN.md` §10 Success Criteria; analog to V1.19 Welle 13 (the v1.0.0 ship welle).
> **Atlas pattern:** Welles 1-7 delivered the V2-α architectural increments. Welle 8 packages them as a versioned release-milestone — workspace version bump, CHANGELOG promotion, release-notes doc. Signed git tag + GitHub Release happen post-merge.

Welle 8 closes V2-α as **v2.0.0-alpha.1**, the first pre-release of Atlas's V2 line. Mechanically straightforward — version bumps + CHANGELOG finalisation + release-notes doc. The intellectual work was done across Welles 1-7. Welle 8 is execution + crisp packaging.

**Why this as Welle 8** (rather than ArcadeDB integration):
- V2-α as designed (per Master Plan §6) is already feature-complete for the security primitive: cryptographic projection-state verification end-to-end via Welles 3+4+5+6, producer CLI via Welle 7
- Master Plan §10 Success Criterion #1 (trust invariant preserved + CI-enforced gate + ProjectorRunAttestation regression) is ACHIEVED
- ArcadeDB integration is legitimately separable as a **Layer-2 storage-backend swap** — not a V2-α security primitive. Suitable for V2-β / post-alpha-1 work where it deserves the 2-3 sessions it actually needs (operator-runbook + Docker-Compose orchestration + Rust HTTP client + integration tests)
- A versioned release is a tangible artefact for regulator engagement (Counsel-Track parallel to V2-α per Master Plan §5), demo materials, and Atlas's external trust narrative
- Matches V1's pattern: V1.19 Welle 13 was the v1.0.0 ship welle, separate from the architectural increments before it

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| Workspace `Cargo.toml` `workspace.package.version`: `1.0.1` → `2.0.0-alpha.1` (single source of truth; all 6 workspace crates inherit via `version.workspace = true`) | ArcadeDB driver integration (V2-β candidate; storage-backend swap separable from V2-α security primitive) |
| npm `package.json` version bumps: root + `apps/atlas-web` + `apps/atlas-mcp-server` + `packages/atlas-bridge` — all `1.0.1` → `2.0.0-alpha.1` | npm publish to registry (V1 used `wasm-publish.yml` auto-fire on signed-tag push for `@atlas-trust/verify-wasm`; V2-α-alpha.1 may or may not auto-publish — verify the workflow's tag-pattern matches `v2.0.0-alpha.1`. If yes, publish auto-fires; if not, defer to a follow-up fix-welle) |
| `apps/atlas-mcp-server/src/index.ts` MCP SDK introspection string: `"1.0.1"` → `"2.0.0-alpha.1"` | Cargo publish to crates.io (V2-α-α stays unpublished to crates.io per V1's choice; npm `@atlas-trust/verify-wasm` is the customer-facing artefact) |
| `crates/atlas-trust-core/src/verify.rs` doc-comment example version bump (illustrative only) | Mem0g cache integration (V2-β) |
| `Cargo.lock` + `pnpm-lock.yaml` regen | Read-API endpoints (V2-β) |
| `CHANGELOG.md`: promote `[Unreleased]` Welle 1-7 entries to `[2.0.0-alpha.1] — 2026-05-13` section with release-summary header. Re-establish empty `[Unreleased]` section. | atlas-web UI integration of V2-α event-kinds (V2-β) |
| NEW `docs/V2-ALPHA-1-RELEASE-NOTES.md` (~250 lines) — comprehensive release-notes covering V2-α-alpha.1 contents, security model, public-API additions, V1-backward-compat, operator-runbook pointer | Counsel-validated marketing language (pre-V2-α-public-materials per Master Plan §5; release notes are engineering-perspective until counsel reviews) |
| Update `docs/SEMVER-AUDIT-V1.0.md` — add a v2.0.0-alpha.1 release marker note at the top; preserve §1-§9 as the v1.0 contract baseline; preserve §10 as the V2-α additive log | Cedar policy gate (V2-δ) |
| Update `.handoff/v2-session-handoff.md` Phase 4 SHIPPED section to acknowledge V2-α-alpha.1 milestone | Sigstore Rekor anchoring of V2-α-alpha.1 release artefacts (auto-fires via existing `wasm-publish.yml` if applicable; otherwise V1 pattern preserved) |
| Plan-doc (this file) | Signed git tag `v2.0.0-alpha.1` (POST-MERGE; tag operates on master, not the PR branch) |

**Total estimated diff:** ~400-600 lines (mostly docs; ~10 lines of code-version-string changes).

---

## Decisions (final, pending Nelson confirmation)

- **Version identifier:** `2.0.0-alpha.1`. Pre-release suffix per SemVer 2.0.0 §9. `2.0.0` reflects the SemVer-major break Welles 1+4+5+6+7 explicitly committed (per `docs/SEMVER-AUDIT-V1.0.md` §10 — `AtlasEvent.author_did` schema-additive break; `deny_unknown_fields` policy intentionally rejects V1 verifiers reading V2-α events; new event-kind `projector_run_attestation`).
- **`-alpha.1` suffix rationale:** Signals "first pre-release of V2.0 line, subject to refinement before V2.0.0 stable." Atlas commits to additional V2-α + V2-β + V2-γ + V2-δ work per Master Plan §6+§10 before declaring V2.0.0 stable. Counsel-validated marketing language is pre-V2-α-public-materials blocking per `DECISION-COUNSEL-1` + Master Plan §5; until counsel signs off, `-alpha.X` is the appropriate maturity signal.
- **Signed tag operates post-merge.** Tag `v2.0.0-alpha.1` cannot exist on a PR branch — it must be created on master after merge. Tag creation flow:
  1. Merge this PR via squash-and-admin (established Atlas pattern)
  2. Locally: `git pull origin master`
  3. `git tag -s v2.0.0-alpha.1 -m "V2-α-alpha.1: cryptographic projection-state verification end-to-end"`
  4. `git push origin v2.0.0-alpha.1`
  5. GitHub Release UI: paste release notes (from `docs/V2-ALPHA-1-RELEASE-NOTES.md`)
- **`wasm-publish.yml` auto-fire:** V1 pattern was push-tag → workflow runs → `@atlas-trust/verify-wasm@X.Y.Z` auto-publishes to npm via SLSA Build L3 provenance. The workflow's `on: push: tags: ['v*']` matches `v2.0.0-alpha.1`. So pushing the tag SHOULD auto-publish `@atlas-trust/verify-wasm@2.0.0-alpha.1` to npm. If npm prerelease-tagging policy needs adjustment (e.g. publish to `next` instead of `latest`), that's a follow-up fix-welle — Welle 8 doesn't enforce npm tag-strategy decisions.
- **No public-API surface lock change.** `docs/SEMVER-AUDIT-V1.0.md` §1-§9 remain the **v1.0 baseline contract**. §10 V2-α Additions documents the new surface. v2.0.0-alpha.1 ships the additive surface; future V2 wellen may further evolve it pre-V2.0.0-stable.
- **CHANGELOG release-summary content:** lead-paragraph narrative of V2-α achievement (trust property end-to-end, Welles 1-7 enumerated). Atlas convention is operator/auditor-friendly prose at the top of each release section, followed by per-welle Added/Changed/Notes subsections (which already exist from Welles 1-7).
- **Test impact: zero.** Welle 8 is mechanical version-string changes + docs + lock regen. All 7 byte-determinism CI pins MUST remain byte-identical (verified post-bump via `cargo test --workspace`). Test version-string assertions checked via grep — `VERIFIER_VERSION` const derives from `CARGO_PKG_VERSION` so bumps automatically, but consumer regex-prefix-only matching (e.g. atlas-web e2e's `/^atlas-trust-core\//`) is unaffected.

---

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| MODIFY | `Cargo.toml` | `workspace.package.version = "1.0.1"` → `"2.0.0-alpha.1"` (line 31). All 6 workspace crates inherit via `version.workspace = true`. |
| MODIFY | `package.json` (root) | `"version": "1.0.1"` → `"2.0.0-alpha.1"` (line 3) |
| MODIFY | `apps/atlas-web/package.json` | `"version": "1.0.1"` → `"2.0.0-alpha.1"` (line 3) |
| MODIFY | `apps/atlas-mcp-server/package.json` | `"version": "1.0.1"` → `"2.0.0-alpha.1"` (line 3) |
| MODIFY | `packages/atlas-bridge/package.json` | `"version": "1.0.1"` → `"2.0.0-alpha.1"` (line 3) |
| MODIFY | `apps/atlas-mcp-server/src/index.ts` | `version: "1.0.1"` → `"2.0.0-alpha.1"` (line 35, MCP SDK introspection string) |
| MODIFY | `crates/atlas-trust-core/src/verify.rs` | Doc-comment example version (line 137, illustrative only) |
| MODIFY | `Cargo.lock` | Auto-regen via `cargo build --workspace` |
| MODIFY | `pnpm-lock.yaml` | Auto-regen via `pnpm install` |
| MODIFY | `CHANGELOG.md` | Promote `[Unreleased]` Welle 1-7 entries to `[2.0.0-alpha.1] — 2026-05-13` with lead release-summary paragraph. Re-establish empty `[Unreleased]` section. |
| NEW | `docs/V2-ALPHA-1-RELEASE-NOTES.md` (~250 lines) | Comprehensive release-notes: what V2-α-alpha.1 ships, security model, V2 public-API additions (cross-ref `SEMVER-AUDIT-V1.0.md` §10), V1-backward-compat boundary, operator-runbook pointers, demo CLI invocation. |
| MODIFY | `docs/SEMVER-AUDIT-V1.0.md` | Add a v2.0.0-alpha.1 release-marker header note (top of doc); preserve §1-§9 as v1.0 baseline; preserve §10 as V2-α additive log. |
| MODIFY | `.handoff/v2-session-handoff.md` | Phase 4 SHIPPED section acknowledges V2-α-alpha.1 milestone (post-merge). |
| NEW | `.handoff/v2-alpha-welle-8-plan.md` | This plan-doc. |

---

## Acceptance criteria

- [ ] `cargo check --workspace` green at version `2.0.0-alpha.1`
- [ ] `cargo test --workspace` green; **all 7 byte-determinism CI pins byte-identical** (V1 cose + Welle 1 cose-with-author_did + V1.7 anchor-canonical-body + V1.7 anchor-head + V1.9 pubkey-bundle + Welle 3 graph-state-hash + Welle 4 attestation-signing-input)
- [ ] All version-string consumers updated; no orphaned `1.0.1` references in code
- [ ] `VERIFIER_VERSION` constant derives correctly: `"atlas-trust-core/2.0.0-alpha.1"`
- [ ] `CHANGELOG.md` has the `[2.0.0-alpha.1]` release section with lead-summary
- [ ] `[Unreleased]` section reset to empty-placeholder
- [ ] `docs/V2-ALPHA-1-RELEASE-NOTES.md` ~250 lines comprehensive
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched
- [ ] CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit, PR opened, self-merge via admin override
- [ ] **Post-merge:** signed git tag `v2.0.0-alpha.1` (via SSH-Ed25519 path)
- [ ] **Post-merge:** GitHub Release with release notes
- [ ] **Post-tag-push:** `wasm-publish.yml` auto-fires (or operator manually triggers); `@atlas-trust/verify-wasm@2.0.0-alpha.1` lands on npm

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Version bump breaks one of the 7 byte-determinism CI pins | NEGLIGIBLE (V1.19 Welle 13 already validated this pattern; the pins lock CBOR bytes, not version strings) | LOW (would surface immediately in test run) | Run `cargo test --workspace` after bump; pin assertions are byte-comparisons not version-string comparisons |
| `VERIFIER_VERSION` change breaks consumer assertions | LOW (V1.19 Welle 12 audit §1.1 documented this as "Locked format `crate-name/semver`"; consumers prefix-match `/^atlas-trust-core\//`) | LOW | Same as above; assertions are prefix-only by convention |
| `wasm-publish.yml` rejects `2.0.0-alpha.1` as invalid SemVer | LOW (npm accepts pre-release per SemVer 2.0.0) | LOW | If publish fails, follow-up fix-welle adjusts workflow; doesn't block the master merge |
| Pre-release version on npm `latest` tag is undesirable | MEDIUM (operator preference) | LOW | Default npm publish puts pre-release on `next` tag, not `latest` — current `wasm-publish.yml` doesn't override, so this works correctly by default |
| Counsel-validated marketing language deferred → release notes contain claims that need counsel review later | EXPECTED (per Master Plan §5 counsel engagement is pre-V2-α-public-materials blocking, parallel-track to engineering) | LOW (release notes are operator/auditor/engineer-facing, NOT public marketing; flagged in the notes themselves) | Release notes carry an explicit "pre-counsel-review" disclaimer at the top per Master Vision §11 |

---

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 8 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins | NONE — pins lock CBOR bytes; version strings are NOT part of the canonical form (Welle 3's `PROJECTOR_SCHEMA_VERSION = "atlas-projector-v1-alpha"` is bound in but is a SEPARATE constant from `CRATE_VERSION`; Welle 4's `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION` is also separate). The crate-version bump does NOT affect canonical encoding. | Tests confirm |
| `VERIFIER_VERSION` derived constant | Changes from `atlas-trust-core/1.0.1` → `atlas-trust-core/2.0.0-alpha.1`. Atlas-web e2e regex `/^atlas-trust-core\//` is prefix-only, unaffected. atlas-web LiveVerifierPanel chip text auto-updates. | V1.19 Welle 13 already validated this drift path; no regression expected |
| atlas-projector `CRATE_VERSION` const (Welle 7-fix) | Changes from `1.0.1` → `2.0.0-alpha.1`. atlas-signer's emit-projector-attestation default produces `"atlas-projector/2.0.0-alpha.1"`. Welle 7's structural-binding test (`default_projector_version_uses_atlas_projector_crate_version`) asserts the binding, not a literal string value. | Test confirms |
| `signing_input_byte_determinism_pin_with_projector_attestation` (Welle 4 pin) | Fixture uses LITERAL `"atlas-projector/0.1.0"` projector_version in the payload. NOT affected by crate-version bump because the pin's fixture is a hard-coded string, not a derived value. | Test confirms |

---

## Out-of-scope this welle (V2-β candidates and beyond)

- **V2-β candidate: ArcadeDB driver integration** — replace in-memory `GraphState` with ArcadeDB-backed implementation; operator-runbook for deployment; SQL `ORDER BY entity_uuid` deterministic dump; Docker-Compose orchestration for integration tests; Rust HTTP client. 2-3 sessions realistic.
- **V2-β candidate: parallel-projection design** — for >10M event scenarios; completes `DECISION-ARCH-1` triple-hardening's third leg
- **V2-β candidate: Mem0g Layer-3 cache** — 91% latency reduction per Locomo benchmark (cited honestly per `DECISION-DB-3`); cite-back-to-event_uuid invariant
- **V2-β candidate: Read-API endpoints** — 6 endpoints per Master Vision §5.4 with AST-validated Cypher
- **V2-β candidate: MCP V2 tools** — 5 tools per Master Vision §5.5
- **V2-β candidate: expanded event-kind support** — `annotation_add`, `policy_set`, `anchor_created` in atlas-projector upsert layer
- **V2-γ candidate: Agent Passports** — `GET /api/atlas/passport/:agent_did` endpoint + revocation mechanism per `DECISION-SEC-1`
- **V2-γ candidate: Regulator-Witness Federation** — M-of-N threshold enrolment per `DECISION-SEC-3`
- **V2-γ candidate: Hermes-skill v1** — credibility-asset GTM positioning per `DECISION-BIZ-1`
- **V2-δ candidate: Cedar policy at write-time** + post-quantum hybrid Ed25519+ML-DSA-65 co-sign
- **Counsel-gated:** content-hash separation (per `DECISION-COUNSEL-1`); counsel-validated marketing language for public materials

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| Master Plan §10 Success Criteria (#1 = V2-α core) | `docs/V2-MASTER-PLAN.md` |
| V2-α architectural work (Welles 1-7) | `CHANGELOG.md [Unreleased]` (to be promoted) + per-welle plan-docs in `.handoff/` |
| V1 Welle 13 v1.0.0 ship pattern (analog) | `.handoff/v1.19-welle-13-plan.md` |
| V2-α SemVer audit (additive surface) | `docs/SEMVER-AUDIT-V1.0.md` §10 |
| Working Methodology Welle-Decomposition Pattern | `docs/WORKING-METHODOLOGY.md` |
| Counsel-engagement parallel track (pre-V2-α-public-materials blocking) | `docs/V2-MASTER-PLAN.md` §5 + `DECISION-COUNSEL-1` |

---

**End of Welle 8 Plan.** Implementation: mechanical version bumps + CHANGELOG promotion + release-notes doc. Single coherent SSH-signed commit per Atlas standing protocol. Post-merge: signed tag + GitHub Release.
