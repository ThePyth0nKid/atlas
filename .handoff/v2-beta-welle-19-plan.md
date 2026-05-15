# V2-β Welle 19 — Plan-Doc (v2.0.0-beta.1 ship convergence milestone)

> **Status:** DRAFT 2026-05-15. Awaiting Phase 14 dispatch.
> **Welle:** W19 (V2-β Phase 14)
> **Orchestration:** SERIAL single-welle dispatch (ship convergence; no parallel-batch). Pattern mirrors V2-α-α.2 ship from W11 (PR #76 `1839e82`).
> **Driving decisions:** `DECISION-ARCH-W18b` (W18b SHIPPED as W17a-pattern Phase-A-scaffold; Layer 3 Read-API returns 501 stub until W18c lifts — W19 ship is honest about this); master-plan §6 Phase 14; `docs/SEMVER-AUDIT-V1.0.md` §10 V2-α Additions (W19 EITHER appends V2-β-1 surface OR creates `SEMVER-AUDIT-V2.0-beta.md` companion); release-pattern from V2-α-α.2 (PR #76).

W19 is the v2.0.0-beta.1 ship convergence milestone. Layer 2 ArcadeDB + Layer 3 Mem0g (scaffold) + verifier-rebuild all operational on master at Phase-13.5-merge state. Workspace version bump from `2.0.0-alpha.2` to `2.0.0-beta.1`; signed annotated tag `v2.0.0-beta.1`; GitHub Release with notes distilled from CHANGELOG; npm publish `@atlas-trust/verify-wasm@2.0.0-beta.1` auto-triggered by tag push via `wasm-publish.yml` (race-fix from W11 + Sigstore Build L3 provenance per the V2-α-α.2 lane).

**Why this as Welle 19:** Phase 13.5 closed the V2-β implementation-tracking phases (Phases 0–13.5 all SHIPPED). W19 is the natural convergence ship — the act of putting V2-β-quality bits on npm + GitHub Releases under the `beta.1` tag, validating the supply chain end-to-end for the second time after V2-α-α.2, and giving external signaling (investors, counsel firms, first-customer prospects) something concrete to point at. Layer 3 scaffold-only status is honestly documented in release notes; W18c parallel-track (Nelson supply-chain constant lift + LanceDB body fill-in) is the operational-activation welle that runs alongside / after W19 ship.

## Scope (table)

| In-Scope | Out-of-Scope |
|---|---|
| Workspace `Cargo.toml` version bump `2.0.0-alpha.2` → `2.0.0-beta.1` | New code surfaces — this welle ships what's already on master |
| 4 `package.json` version bumps (bridge / cypher-validator / mcp-server / atlas-web) | W18c body fill-in (parallel-track, NOT W19 ship gate) |
| `CHANGELOG.md` `[Unreleased]` → `[2.0.0-beta.1] — YYYY-MM-DD` conversion + release-date stamp | Counsel sign-off (per `DECISION-COUNSEL-1` blocks public materials, NOT the tag) |
| Release-notes generation distilled from CHANGELOG `[2.0.0-beta.1]` section | Marketing / landing-page / Hermes-skill announcement |
| Signed annotated tag `v2.0.0-beta.1` (Ed25519 SSH per Atlas convention) | New ADRs (use ADR-Atlas-013 only if W19 surfaces design amendment — unlikely) |
| GitHub Release `v2.0.0-beta.1` published with notes | Public-API surface changes — `docs/SEMVER-AUDIT-V1.0.md` baseline §1-§9 unchanged; W19 EITHER appends §10 entry OR creates `SEMVER-AUDIT-V2.0-beta.md` companion (see "Decisions" below) |
| npm publish `@atlas-trust/verify-wasm@2.0.0-beta.1` (auto via tag push → `wasm-publish.yml`) | Welle 19 does NOT modify `wasm-publish.yml` itself (the race-fix from W11 is load-bearing — touching it requires a separate welle) |
| README.md "Current version" line update | Demos / examples / quickstart updates |
| **Forbidden files (parent consolidates in Phase 14.5):** `.handoff/decisions.md`, `.handoff/v2-session-handoff.md`, `docs/V2-BETA-ORCHESTRATION-PLAN.md`, `docs/V2-BETA-DEPENDENCY-GRAPH.md` | |

**Hard rule:** Welle 19 ships what's already on master. NO new code; NO new test surfaces; NO new docs apart from release-notes + SEMVER-AUDIT-companion + CHANGELOG conversion + README touch.

## Decisions (final, pending parent confirmation)

These decisions are LOCKED before dispatch. Each is documented for parent-review traceability:

- **Version scheme:** `2.0.0-beta.1` (SemVer prerelease). Successor: `2.0.0-beta.2` if W18c lift requires another ship before `2.0.0`. Locked.
- **Tag name:** `v2.0.0-beta.1`. Annotated, SSH-Ed25519 signed via `git tag -s v2.0.0-beta.1 -m "..."`. Locked.
- **npm dist-tag:** default `latest` (analog V2-α-α.2 from W11 PR #76). Reasoning: V2-β-1 IS the latest production-track artefact; v2.0.0-alpha.2 was also published with `latest`. If concern about alpha-users auto-upgrading: see risk R-W19-3. Locked-pending-Nelson-confirmation.
- **Release-notes source:** CHANGELOG `[2.0.0-beta.1]` section, lightly edited for marketing-quality (≤2 paragraphs intro + bullet-list of headline changes + "Scaffold posture" callout for Layer 3 + "W18c parallel-track" pointer). Locked.
- **SEMVER-AUDIT companion:** **CREATE NEW** `docs/SEMVER-AUDIT-V2.0-beta.md` rather than appending to V1.0 §10. Reasoning: V1.0 baseline contract is fully locked (§1-§9); appending V2-β-1 entries to §10 would conflate alpha-additions (already-stable) with beta-additions (newer surface). Two separate companion docs cleaner. V2-α-α.1 release notes already established `docs/V2-ALPHA-1-RELEASE-NOTES.md` pattern; W19 mirrors with `docs/V2-BETA-1-RELEASE-NOTES.md` + `docs/SEMVER-AUDIT-V2.0-beta.md`.
- **Layer 3 scaffold disclosure in release notes:** LOCKED. Notes explicitly state: *"Layer 3 Mem0g semantic-cache: trait + protocol + dispatch surface + state extension production-shape; `/api/atlas/semantic-search` Read-API returns 501 stub until W18c follow-on welle activates LanceDB ANN bodies + Nelson supply-chain constant lift. ArcadeDB Layer 2 (W17a-c) is fully operational."* Honest signalling.
- **Tag-immutability contract:** LOCKED. Per V1.17 Welle B precedent (CHANGELOG line ~"Tag-immutability"), signed tags are permanent. Post-tag fixes are forward-version SemVer-patch (v2.0.0-beta.2 etc), NOT re-tag.

## Files

| Status | Path | Change |
|---|---|---|
| MODIFY | `Cargo.toml` | line 37: `version = "2.0.0-alpha.2"` → `version = "2.0.0-beta.1"` |
| MODIFY | `packages/atlas-bridge/package.json` | `version` field |
| MODIFY | `packages/atlas-cypher-validator/package.json` | `version` field |
| MODIFY | `apps/atlas-mcp-server/package.json` | `version` field |
| MODIFY | `apps/atlas-web/package.json` | `version` field |
| MODIFY | `Cargo.lock` | auto-regenerated by `cargo check --workspace` after Cargo.toml bump |
| MODIFY | `CHANGELOG.md` | `## [Unreleased]` becomes `## [2.0.0-beta.1] — YYYY-MM-DD` (release date stamp); new `## [Unreleased]` empty section inserted above |
| MODIFY | `README.md` | "Current version" line / badge / install snippet — all locations referencing `2.0.0-alpha.2` |
| NEW | `docs/V2-BETA-1-RELEASE-NOTES.md` | analog `docs/V2-ALPHA-1-RELEASE-NOTES.md`. ~200 lines: headline narrative, Layer 2 + Layer 3 status, W18c parallel-track, scaffold honesty callout, upgrade-from-alpha guide |
| NEW | `docs/SEMVER-AUDIT-V2.0-beta.md` | analog companion to V1.0. ~150 lines: V2-β-1 additive surface entries (atlas-mem0g crate, embedding_erased event-kind, embedding_erasures GraphState field, semantic-search Read-API endpoint, atlas-mem0g-smoke workflow). Locked / Unstable / Internal tags per V1.0 §1.0 methodology |
| (auto via tag push) | `wasm-publish.yml` triggered | `@atlas-trust/verify-wasm@2.0.0-beta.1` published with `--provenance` (Sigstore Build L3) |

**Estimated diff:** ~400-500 lines net additions (release-notes + SEMVER-AUDIT-V2.0-beta + CHANGELOG section move + 5 version bumps). All MODIFY; 2 NEW companion docs.

## Test impact (V1 + V2-α invariants to preserve)

| Surface | Drift risk under Welle 19 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) | NONE — version-string change only; no Rust touched | Pre-merge parent runs `cargo test --workspace --quiet` + verifies byte-pin reproduces |
| `cargo clippy --workspace --no-deps -- -D warnings` | NONE — no Rust touched | Pre-merge parent verifies zero warnings |
| `cargo test --workspace` | NONE — no Rust touched | Pre-merge parent verifies all 577 tests pass (post-Phase-14 W18c-A retired the `pins_are_placeholder_until_nelson_verifies` gatekeeper; baseline drifted 578 → 577) |
| `atlas-web-playwright` required CI check | TRIGGERED via `.handoff/**` path-filter (W19 plan-doc touch — note: this plan-doc IS already master-resident from Phase 13.6, so W19's PR may need a separate `.handoff/v2-beta-welle-19-plan.md` Implementation Notes touch OR an explicit `apps/atlas-web/**` touch — version bump in `apps/atlas-web/package.json` covers it) | Verify CI run lands green before admin-merge |
| `Verify trust-root-modifying commits` required CI check | NONE — no `.github/`/`tools/`/allowed_signers touches in W19 | Routine SSH-Ed25519 signed commit |
| `atlas-arcadedb-smoke` workflow | NONE — no `crates/atlas-projector/**` or compose touches | N/A |
| `atlas-mem0g-smoke` workflow | NONE — no `crates/atlas-mem0g/**` touches | N/A |
| `wasm-publish.yml` (auto-triggered by tag push) | The publish lane runs `wasm-pack build` + `npm publish --provenance`. NOT a pre-merge CI check; runs POST-tag-push | Parent monitors the run via `gh run watch <id>`; npm registry `latest` flips within 5 min of completion |

**Mandatory check:** the v2.0.0-beta.1 tag-push must trigger `wasm-publish.yml` AND that workflow must publish to npm with Sigstore provenance. Parent verifies via `npm view @atlas-trust/verify-wasm@2.0.0-beta.1 dist-tags` showing `latest`.

## Implementation steps (ship-adapted; not traditional TDD)

1. **Pre-flight (parent does, BEFORE subagent dispatch):**
   - `cd /c/Users/nelso/Desktop/atlas`
   - `git fetch origin && git checkout master && git pull origin master` — be on Phase-13.6-merge state OR later
   - `git status` clean
   - `git log --oneline -3` — verify top is `<Phase 13.6 merge>` then `578f17f docs(v2-beta/phase-13.5)` then `2f2238b feat(v2-beta/welle-18b)`
   - `cargo test --workspace --quiet` — 577 tests pass
   - `cargo clippy --workspace --no-deps -- -D warnings` — zero warnings
   - `cargo test -p atlas-projector --test backend_trait_conformance byte_pin --quiet` — byte-pin reproduces
   - `git verify-tag v2.0.0-alpha.2` — Good ed25519
   - `git verify-tag v2.0.0-alpha.1` — Good ed25519
   - `npm view @atlas-trust/verify-wasm dist-tags` — shows `latest = 2.0.0-alpha.2`
2. **Subagent dispatch** (single SERIAL agent, isolation: worktree, subagent_type: general-purpose). Prompt skeleton below.
3. **Subagent's flow** (executed inside the worktree):
   1. `git fetch origin && git checkout -B feat/v2-beta/welle-19-ship origin/master`
   2. Edit `Cargo.toml` line 37: `version = "2.0.0-beta.1"`
   3. Edit 4 package.json files: bump version `2.0.0-alpha.2` → `2.0.0-beta.1`
   4. `cargo check --workspace` → regenerates `Cargo.lock` workspace version refs
   5. Convert `CHANGELOG.md` `## [Unreleased]` block → `## [2.0.0-beta.1] — YYYY-MM-DD` (use actual ship date). Insert NEW empty `## [Unreleased]` section above.
   6. Write NEW `docs/V2-BETA-1-RELEASE-NOTES.md` (~200 lines). Headline: "v2.0.0-beta.1 — V2-β tripod operational (Layer 2 ArcadeDB + Layer 3 Mem0g scaffold + verifier-rebuild)". Scaffold honesty callout. W18c parallel-track pointer. Upgrade-from-alpha guide.
   7. Write NEW `docs/SEMVER-AUDIT-V2.0-beta.md` (~150 lines). Mirror V1.0's structure: each public surface tagged Locked / Locked-Behind-Flag / Unstable / Internal. Cover atlas-mem0g crate's SemanticCacheBackend trait (Locked) + SemanticHit struct (Locked) + Mem0gError enum #[non_exhaustive] (Locked) + InvalidationPolicy (Locked) + AtlasEmbedder (Internal, deferred to V2-γ stability) + LanceDbCacheBackend (Locked-Behind-Flag `lancedb-backend`).
   8. Update README.md: replace all `2.0.0-alpha.2` references with `2.0.0-beta.1`. Update "Current version" line / badge. Update `npm install @atlas-trust/verify-wasm` example if pinned.
   9. Local verification: `cargo check --workspace` clean; `cargo test --workspace --quiet` 577 pass; `cargo clippy --workspace --no-deps -- -D warnings` zero; byte-pin reproduces.
   10. SSH-signed commit. Conventional message: `feat(v2-beta/welle-19): v2.0.0-beta.1 ship convergence — workspace version bump + CHANGELOG conversion + release notes + SEMVER-AUDIT-V2.0-beta + README update`
   11. Push branch.
   12. Open DRAFT PR. Body: link to CHANGELOG [2.0.0-beta.1] section + V2-BETA-1-RELEASE-NOTES + scaffold honesty callout + W18c parallel-track pointer.
4. **Parallel reviewer dispatch (parent, Atlas Standing Protocol Lesson #8):** `code-reviewer` + `security-reviewer`. Focus: version-string consistency across all 5 manifests + CHANGELOG; release-notes factual accuracy on Layer 3 scaffold; SEMVER-AUDIT-V2.0-beta.md tag-soundness (Locked vs Unstable boundaries); no accidental code touches.
5. **Fix CRITICAL/HIGH + MEDIUM in-commit per Lesson #3.**
6. **Mark PR ready-for-review + admin-merge** via `gh pr merge --admin --squash --delete-branch`.
7. **Post-merge: signed tag on the merge commit** (master HEAD).
   - `git checkout master && git pull origin master`
   - `git log -1 --format='%H %s'` — capture merge SHA
   - `git tag -s v2.0.0-beta.1 -m "Atlas v2.0.0-beta.1 — V2-β tripod operational (Layer 2 ArcadeDB + Layer 3 Mem0g scaffold + verifier-rebuild)"`
   - `git verify-tag v2.0.0-beta.1` — Good
   - `git push origin v2.0.0-beta.1`
8. **GitHub Release create:**
   - `"/c/Program Files/GitHub CLI/gh.exe" release create v2.0.0-beta.1 --title "v2.0.0-beta.1" --notes-file docs/V2-BETA-1-RELEASE-NOTES.md --prerelease`
   - `--prerelease` flag because `2.0.0-beta.1` is SemVer prerelease.
9. **Monitor wasm-publish.yml auto-trigger:**
   - `gh run list --workflow wasm-publish.yml --limit 3` — confirm a run was triggered by the tag push
   - `gh run watch <run-id> --exit-status` — wait for completion
   - Verify NPM publish succeeded: `npm view @atlas-trust/verify-wasm@2.0.0-beta.1` shows the new version
   - Verify Sigstore provenance: `npm audit signatures @atlas-trust/verify-wasm@2.0.0-beta.1` (or via `gh attestation verify` chain)
10. **Phase 14.5 consolidation (separate parent-led welle, post-W19-ship):** updates `docs/V2-MASTER-PLAN.md` §6 status row (Phase 14 SHIPPED), `.handoff/decisions.md` `DECISION-ARCH-W19` entry, `docs/V2-BETA-ORCHESTRATION-PLAN.md` Phase 14 status flip, `docs/V2-BETA-DEPENDENCY-GRAPH.md` (W19 SHIPPED → terminus or pivots to V2-γ planning), `.handoff/v2-session-handoff.md` §0z6 W19 SHIPPED narrative + §0-NEXT refresh to post-W19 next-target (W18c follow-on if not yet shipped; OR V2-γ planning if W18c already complete).

## Acceptance criteria

- [ ] `cargo check --workspace` clean
- [ ] `cargo test --workspace --quiet` 577 tests pass; zero failures
- [ ] `cargo clippy --workspace --no-deps -- -D warnings` zero warnings
- [ ] All 5 manifest versions are exactly `2.0.0-beta.1` (Cargo.toml + 4 package.json)
- [ ] Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces unchanged
- [ ] `CHANGELOG.md` has `## [2.0.0-beta.1] — YYYY-MM-DD` section + NEW empty `## [Unreleased]` above
- [ ] `docs/V2-BETA-1-RELEASE-NOTES.md` and `docs/SEMVER-AUDIT-V2.0-beta.md` exist with substantive content
- [ ] `README.md` shows `2.0.0-beta.1` (no leftover `2.0.0-alpha.2` references)
- [ ] Parallel `code-reviewer` + `security-reviewer` dispatched; 0 unresolved CRITICAL / HIGH
- [ ] Single SSH-Ed25519 signed commit (fingerprint `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [ ] PR ready-for-review + admin-merged via `gh pr merge --admin --squash --delete-branch`
- [ ] Forbidden-files rule honoured (no touches to decisions.md / session-handoff.md / orchestration-plan / dependency-graph in W19 PR — those land in Phase 14.5 consolidation)
- [ ] **POST-MERGE:** Signed tag `v2.0.0-beta.1` pushed; `git verify-tag` Good
- [ ] **POST-TAG-PUSH:** GitHub Release v2.0.0-beta.1 created with notes; flagged `--prerelease`
- [ ] **POST-RELEASE:** `wasm-publish.yml` auto-triggered run green; npm registry shows `@atlas-trust/verify-wasm@2.0.0-beta.1` with dist-tag `latest`; Sigstore Build L3 provenance attached
- [ ] **Sanity:** `npm install @atlas-trust/verify-wasm@2.0.0-beta.1` works against a clean test directory

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| **R-W19-1 — Beta-tag premature operational signal** | MED | LOW (engineering) / MED (ops perception) | Layer 3 returns 501 stub until W18c lifts. Release notes EXPLICITLY state scaffold posture + W18c parallel-track. The 501 stub returns clear error message pointing to operator-runbook. |
| **R-W19-2 — `wasm-publish.yml` race-fix regression** | LOW (race-fix was validated by V2-α-α.2 ship) | HIGH (publish-lane stuck or unsafe) | Parent monitors `wasm-publish.yml` run via `gh run watch`. If failure: see W11 ADR-Atlas-008 postmortem for the race-fix design. Recovery: workflow_dispatch from the already-signed tag (dispatch from tag is publish-capable per V1.17 Welle B contract). |
| **R-W19-3 — dist-tag `latest` confusion: alpha users auto-upgrade to beta** | LOW (alpha-track users are V2-β-pioneers — auto-upgrade is expected) | LOW | Document in release notes. If concern: publish initially with dist-tag `next` (`npm publish --tag next`) then promote to `latest` after 24h soak. W11 V2-α-α.2 used `latest` directly without issue. Recommend `latest` per precedent. |
| **R-W19-4 — Beta-version-comparator edge cases in package managers** | LOW (npm + cargo SemVer-compliant) | LOW | npm: `2.0.0-beta.1 < 2.0.0` per SemVer §11 (prereleases sort before final). cargo: same. Downstream `^2.0.0-alpha.2` consumers WILL pick up `2.0.0-beta.1` (prereleases match same-major-minor-patch caret). Acceptable behaviour. |
| **R-W19-5 — Layer-3 scaffold ship signals "ready" to first customers** | MED | MED (premature customer-adoption attempt) | Release notes + operator-runbook + Read-API 501 response all carry consistent scaffold-status messaging. Counsel-track per `DECISION-COUNSEL-1` blocks public marketing until counsel sign-off, providing additional gating. |
| **R-W19-6 — Tag-immutability under pre-merge hook failure** | LOW | LOW | Per V1.17 Welle B contract: signed tags are permanent. If pre-merge hook fails AFTER tag-push: forward-version SemVer-patch (v2.0.0-beta.2) is the recovery path. NEVER re-tag a published tag. Atlas-standing-protocol violation otherwise. |

## Subagent dispatch prompt skeleton (anti-divergence enforcement, for W19)

When the parent agent dispatches the W19 ship subagent, the prompt MUST include:

```text
Atlas project at C:\Users\nelso\Desktop\atlas. V2-β Welle 19 — v2.0.0-beta.1 ship convergence milestone.
Master HEAD at time of dispatch: <commit-sha-post-Phase-13.6-merge>.

## Your goal
Ship v2.0.0-beta.1: workspace version bump in 5 manifests + CHANGELOG conversion + V2-BETA-1-RELEASE-NOTES.md + SEMVER-AUDIT-V2.0-beta.md + README update. NO new code; NO new test surfaces. The signed tag + GitHub Release + npm publish happen POST-MERGE (parent-led).

## Pre-flight (FIRST 3 actions — non-negotiable, Atlas Lesson #1)
1. `git fetch origin`
2. `git checkout -B feat/v2-beta/welle-19-ship origin/master` (master HEAD at dispatch: <current-master-sha>)
3. `git status` → clean

## Pre-flight reading (master-resident, mandatory)
1. `.handoff/v2-beta-welle-19-plan.md` (this plan-doc) — full file list + acceptance criteria
2. `docs/V2-MASTER-PLAN.md` §6 (Phase 14 W19 framing)
3. `CHANGELOG.md` (current `[Unreleased]` block becomes `[2.0.0-beta.1]`)
4. `docs/SEMVER-AUDIT-V1.0.md` (template for V2-β-1 companion doc)
5. `docs/V2-ALPHA-1-RELEASE-NOTES.md` (template for V2-BETA-1-RELEASE-NOTES.md)
6. `.handoff/v2-session-handoff.md` §0-NEXT (W19 framing)

## In-scope files (write/modify only these)
- `Cargo.toml` (workspace) — version bump
- `packages/atlas-bridge/package.json` — version bump
- `packages/atlas-cypher-validator/package.json` — version bump
- `apps/atlas-mcp-server/package.json` — version bump
- `apps/atlas-web/package.json` — version bump
- `Cargo.lock` — auto-regenerated by `cargo check`
- `CHANGELOG.md` — [Unreleased] → [2.0.0-beta.1] + new empty [Unreleased] above
- `README.md` — update version references
- NEW `docs/V2-BETA-1-RELEASE-NOTES.md` — ~200 lines
- NEW `docs/SEMVER-AUDIT-V2.0-beta.md` — ~150 lines

## Forbidden files (parent consolidates in Phase 14.5)
- `.handoff/decisions.md`, `.handoff/v2-session-handoff.md`
- `docs/V2-BETA-ORCHESTRATION-PLAN.md`, `docs/V2-BETA-DEPENDENCY-GRAPH.md`
- `docs/V2-MASTER-PLAN.md` (status — §6 narrative stays unchanged until Phase 14.5)
- `.github/workflows/**` (the wasm-publish race-fix is load-bearing; touching it requires a separate welle)
- `crates/**` and `apps/**/src/**` (NO new code)

## Hard rules (Atlas Standing Protocol)
- Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` MUST remain reproducible. Run `cargo test -p atlas-projector --test backend_trait_conformance byte_pin --quiet` as final go/no-go before push.
- SSH-Ed25519 signed commits only. No `--no-verify`.
- Parent ALWAYS dispatches parallel `code-reviewer` + `security-reviewer` post-implementation (Lesson #8).
- Scaffold honesty in release notes: Layer 3 returns 501 stub until W18c lifts. State this LOUDLY.

## Acceptance criteria (parent verifies before approving merge)
- All 5 manifests show `2.0.0-beta.1` exactly
- `cargo check --workspace` clean
- `cargo test --workspace --quiet` 577 pass
- `cargo clippy --workspace --no-deps -- -D warnings` zero warnings
- Byte-pin reproduces
- `CHANGELOG.md` has `[2.0.0-beta.1] — YYYY-MM-DD` section + new empty `[Unreleased]`
- `docs/V2-BETA-1-RELEASE-NOTES.md` exists with substantive content (Layer 2 + Layer 3 status, scaffold honesty, W18c pointer, upgrade-from-alpha guide)
- `docs/SEMVER-AUDIT-V2.0-beta.md` exists with substantive content (V2-β-1 additive surface entries with Locked/Internal/Locked-Behind-Flag tags)
- `README.md` shows `2.0.0-beta.1` (no leftover `2.0.0-alpha.2`)
- Single SSH-Ed25519 signed commit
- DRAFT PR open base=master, body comprehensive

## Output (under 400 words)
PR number + URL · line counts per modified/new file · all 5 version strings · CHANGELOG section names · byte-pin reproduces (last-12-hex) · clippy summary · any unexpected deviations.

If a file deviation surfaces (e.g. version string lives in an unexpected manifest), document in the plan-doc Implementation Notes + flag for parent review.
```

This skeleton is mandatory; deviations are flagged by the parent agent's review.

---

## Implementation Notes (Post-Code) — filled at PR-open time (2026-05-15)

### What actually shipped

| Concrete | File | Lines changed |
|---|---|---|
| Workspace version bump `2.0.0-alpha.2` → `2.0.0-beta.1` | `Cargo.toml` (line 37) | 1 line modified |
| `@atlas/bridge` version bump | `packages/atlas-bridge/package.json` (line 3) | 1 line modified |
| `@atlas/cypher-validator` version bump | `packages/atlas-cypher-validator/package.json` (line 3) | 1 line modified |
| `atlas-mcp-server` version bump | `apps/atlas-mcp-server/package.json` (line 3) | 1 line modified |
| `atlas-web` version bump | `apps/atlas-web/package.json` (line 3) | 1 line modified |
| Cargo.lock workspace-version refs regenerated by `cargo check --workspace` | `Cargo.lock` | ~10 `version = "2.0.0-beta.1"` lines auto-updated |
| `[Unreleased]` → `[2.0.0-beta.1] — 2026-05-15` + new empty `[Unreleased]` section above + V2-β tripod ship summary paragraph at section top | `CHANGELOG.md` | +5 lines / -1 line (header conversion + summary paragraph insertion) |
| NEW V2-β-1 release notes (13 sections per W19 plan; ~200 lines) | `docs/V2-BETA-1-RELEASE-NOTES.md` | +145 lines (new file) |
| NEW V2-β-1 SemVer audit companion (mirrors V1.0 methodology; 8 sections) | `docs/SEMVER-AUDIT-V2.0-beta.md` | +135 lines (new file) |
| Implementation Notes filled + 6 stale `578 tests` → `577` corrections + plan-doc consistency | `.handoff/v2-beta-welle-19-plan.md` | +60 lines / -8 lines |

**Forbidden-files rule honoured.** Zero touches to `.handoff/decisions.md`, `.handoff/v2-session-handoff.md`, `docs/V2-MASTER-PLAN.md`, `docs/V2-BETA-ORCHESTRATION-PLAN.md`, `docs/V2-BETA-DEPENDENCY-GRAPH.md`, `.github/workflows/**`, `crates/**` source code, `apps/**/src/**` source code. Untracked `.handoff/v2-demo-sketches.md` was NOT staged per Nelson's "lassen Untracked" directive.

**Deviation from plan:** Plan listed exactly 5 manifests (Cargo.toml + 4 package.json). Root-level `package.json` (workspace-monorepo manifest at repo root) ALSO carries `"version": "2.0.0-alpha.2"` and was not in the plan's in-scope list. Decision: respected the plan's exact in-scope file list and DID NOT touch root-level `package.json`. Flagged for parent review — if the V2-α-α.2 ship (PR #76 `1839e82`) also left it stale, this is a precedent-aligned ship. If the parent wants consistency across all 6 manifests, a `2.0.0-alpha.2` → `2.0.0-beta.1` follow-up touch is a 1-line patch.

**Deviation from plan:** Plan listed README as in-scope ("replace ALL `2.0.0-alpha.2` references"). `grep '2\.0\.0-alpha\.2' README.md` returned **zero matches**. The README's `Status — v1.0.1 (2026-05-12)` line is stale across V2-α-α.1 + V2-α-α.2 ships (precedent-aligned). Decision: respected the plan's exact replacement rule (no alpha.2 substring → no replacement) and DID NOT update the status header. Flagged for parent review — if a `Status — v2.0.0-beta.1 (2026-05-15)` line update is desired, it is a 1-line patch.

### Test outcome (verified pre-PR-open)

- `cargo check --workspace`: clean. Output: `Finished 'dev' profile … in 27.39s`.
- `cargo test --workspace --quiet`: **passed=577 failed=0 ignored=7** (matches expected count per W18c Phase A retiring the `pins_are_placeholder_until_nelson_verifies` gatekeeper).
- `cargo clippy --workspace --no-deps -- -D warnings`: zero warnings.
- `cargo test -p atlas-projector --test backend_trait_conformance byte_pin --quiet`: **1 passed; 0 failed; 0 ignored** — byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces.
- All 7 byte-determinism CI pins unchanged (verified via full test suite green).

### Post-merge ship operations (parent-led; documented for traceability)

- Signed tag `v2.0.0-beta.1` SHA: `<filled by parent on tag-push>`
- `git verify-tag v2.0.0-beta.1`: `<filled by parent: Good / failure>`
- GitHub Release URL: https://github.com/ThePyth0nKid/atlas/releases/tag/v2.0.0-beta.1
- wasm-publish.yml run-id: `<filled by parent>`; conclusion: `<success / failure>`
- npm registry: `@atlas-trust/verify-wasm@2.0.0-beta.1` dist-tag latest: `<filled by parent>`
- Sigstore Build L3 provenance: `<filled by parent>`

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| R-W19-1 beta-tag premature operational signal | Layer 3 scaffold posture LOUDLY stated in release notes "Layer 3 — Mem0g semantic cache — SCAFFOLD-SHIPPED" section + at-a-glance + CHANGELOG summary paragraph. 501 stub posture documented across 4 cross-references (release notes, SEMVER-AUDIT, CHANGELOG, plan-doc). |
| R-W19-2 wasm-publish race-fix regression | NOT exercised pre-merge; deferred to post-tag-push parent monitoring per plan §"Post-merge ship operations". |
| R-W19-3 dist-tag latest confusion | Documented in release notes "Upgrade guide" + acknowledged as precedent-aligned with V2-α-α.2 (W11 PR #76). |
| R-W19-4 beta-version-comparator edge cases | Documented in release notes "Upgrade guide": SemVer §11 prerelease ordering + caret-matching behaviour acknowledged. |
| R-W19-5 scaffold-ship customer adoption risk | Release notes + SEMVER-AUDIT-V2.0-beta.md cross-reference operator-runbook + 501 response semantics. Counsel-track per `DECISION-COUNSEL-1` blocks public materials as additional gating layer. |
| R-W19-6 tag-immutability under hook failure | NOT exercised pre-merge; contract documented in release notes "W18c parallel-track pointer" section. |

### Deviations from plan

1. **Root-level `package.json` not touched** despite carrying `2.0.0-alpha.2` — plan's in-scope list specified exactly 5 manifests; this 6th is a workspace-monorepo manifest at repo root. Flagged for parent review.
2. **README.md not touched** — zero `2.0.0-alpha.2` references to replace. Status line shows `v1.0.1 (2026-05-12)` (stale across V2-α-α.1 + V2-α-α.2 precedent). Flagged for parent review.
3. **Cargo.lock NOT manually touched** — auto-regenerated by `cargo check --workspace` per plan. Plan listed it as MODIFY; the auto-regeneration is the modification.

All other plan items executed as-spec. No deviations of substance.

---

**End of W19 plan-doc.** Phase 14.5 consolidation is the parent-led follow-on; W18c parallel-track runs alongside (NOT W19 ship gate).
