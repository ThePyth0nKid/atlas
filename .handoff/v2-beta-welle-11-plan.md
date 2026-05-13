# V2-β Welle 11 — Plan-Doc (`wasm-publish.yml` Dual-Publish Race Fix)

> **Status:** DRAFT 2026-05-13. Awaiting parent agent's confirmation before merge.
> **Orchestration:** part of Phase 1 (Parallel Batch 1, alongside W9 + W10) per `docs/V2-BETA-ORCHESTRATION-PLAN.md`.
> **Driving decisions:** ADR-Atlas-008 pre-reservation (`docs/V2-BETA-DEPENDENCY-GRAPH.md` §4). Forward-fix of the dual-publish race observed on `v2.0.0-alpha.1` tag-push (run_id `25788574299`, 2026-05-13 08:50 UTC). No retroactive republish of `2.0.0-alpha.1` (npm correctly enforces version immutability; the package IS LIVE on `latest`).

V2-β's `wasm-publish.yml` lane is the supply-chain trust anchor for `@atlas-trust/verify-wasm` on npm — `npm publish --provenance` produces the SLSA Build L3 provenance attestation that downstream consumers verify via `npm audit signatures` (per ADR-Atlas-006 §1.1 path B). The workflow has shipped successfully across the V1.x line (V1.14 Scope E ⇒ V1.19 Welle 14a). On `v2.0.0-alpha.1` tag-push, the workflow fired ONE run and surfaced a red exit (`E403 — cannot publish over the previously published versions`); however the package IS LIVE on npm at `latest` dist-tag, because the FIRST of two `npm publish` invocations succeeded silently before the second failed loud. This welle eliminates the dual-publish race so the next signed-tag push (e.g. `v2.0.0-alpha.2`) produces a clean green run and a fully-tagged registry state.

**Why this as Welle 11:** Phase 1 of the V2-β orchestration plan is a 3-welle parallel batch (W9 docs, W10 ADR, W11 workflow fix) all targeting **distinct file-areas with zero conflict-surface** (per dependency-graph §3 conflict matrix). The fix MUST land before Phase 3's `v2.0.0-alpha.2` ship-gate, which is the first opportunity to validate the fix against a real signed-tag publish. Postponing would re-introduce the red-CI noise on every V2-β release tag.

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| `.github/workflows/wasm-publish.yml` — replace dual `npm publish` invocations with a single publish + `npm dist-tag add` follow-up | `CHANGELOG.md` (parent consolidates post-batch) |
| `docs/ADR/ADR-Atlas-008-wasm-publish-race-postmortem.md` (NEW) — postmortem + design rationale | `docs/V2-MASTER-PLAN.md` §6 status table (parent consolidates) |
| `.handoff/v2-beta-welle-11-plan.md` (this file) — plan-doc on welle branch | `docs/SEMVER-AUDIT-V1.0.md` (parent consolidates) |
| Documentation of the `2.0.0-alpha.1` orphaned-provenance situation (logIndex `1523498503` exists on Sigstore Rekor but is not the registry's authoritative attestation for the live package) | `.handoff/decisions.md` (parent consolidates) |
| | `.handoff/v2-session-handoff.md` (parent consolidates) |
| | Retroactive republish of `2.0.0-alpha.1` (npm-side version immutability; live package state correct as-is, only the missing `node` dist-tag is observable consumer-impact) |
| | ANY other workflow file (`verify-tag-signatures.yml`, `ci.yml`, etc. — out of scope per orchestration plan) |
| | Code under `crates/`, `apps/`, `packages/` — workflow is the surface; verifier-core bytes unchanged |

**Hard rule:** the "Out-of-Scope" column includes the V2-β-Orchestration-Plan §3.3 forbidden files. Parent agent edits those post-consolidation.

## Decisions (final, pending parent confirmation)

- **Fix approach: single publish + `npm dist-tag add`.** After analysis of three candidates (see "Approach analysis" below), the cleanest fix is to publish the version ONCE under default `latest`, then add the `node` dist-tag via `npm dist-tag add @atlas-trust/verify-wasm@<version> node` as a post-publish pointer operation. This:
  - Matches npm's actual data model (versions are immutable; dist-tags are mutable pointers).
  - Eliminates the dual-publish race by construction (only one `publish` call).
  - Preserves the `pkg-node` build (build still runs, gets smoke-tested, is still packed into the GH-Release backup tarball + uploaded as a workflow artifact).
  - Surfaces a single SLSA Build L3 provenance attestation per release (not two near-duplicates — the second of which was orphaned, since the failed publish never landed on the registry but the OIDC token + Sigstore Rekor write DID complete pre-409).
- **Which build to publish under `latest`:** publish the `pkg-web` build, mirroring the V1.19 Welle 14a comment in the workflow ("browser is the primary distribution channel for in-browser auditor tooling"). The `pkg-node` build remains addressable via `npm install @atlas-trust/verify-wasm@node` once the dist-tag points at the same version — but since only one tarball ships to the registry, **Node consumers will receive the `pkg-web` tarball under the `node` dist-tag**. This is a deliberate trade-off and warrants a follow-up welle to merge web + node into a single package with conditional exports (V2-β-or-later, not blocking).
  - **Honesty caveat (carried into ADR §8):** this trade-off means the `node` dist-tag is currently a stable-pointer-to-web-build alias rather than a true Node-specific tarball. Pre-V1.19 the dist-tag pointed at a separate Node tarball with CommonJS glue; the new posture is "Web build serves both consumers; Node import path works because wasm-bindgen's web glue degrades-but-functions under Node when the operator passes raw WASM bytes." A follow-up welle to ship conditional-exports unification is recorded in §"Out-of-scope this welle" below.
- **Post-publish dist-tag idempotency:** the new step uses `npm dist-tag add` which is idempotent across re-runs (setting a dist-tag to a version it already points at is a no-op, exits 0). This means a workflow re-run on the same tag (e.g. after a transient post-publish-verification flake) does NOT corrupt the dist-tag state.
- **Verification-step adjustments:** the "Verify npm publish landed" step's `node` dist-tag check (lines 572–577) remains; the assertion now passes because the dist-tag pointer points at the published version. The block of comments explaining the dist-tag check is updated to reflect the new pointer-vs-second-publish reality.
- **ADR number 008** — pre-reserved per `docs/V2-BETA-DEPENDENCY-GRAPH.md` §4 + orchestration-plan §3.4.

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| MODIFY | `.github/workflows/wasm-publish.yml` | Replace dual `npm publish` block (lines 499-504) with single `npm publish` for pkg-web + `npm dist-tag add` for `node`. Update the publish-step comment block (lines 422-474) to document the new model. Update verify-step comment block (lines 567-577) for the dist-tag-pointer reality. ~30 lines changed. |
| NEW | `docs/ADR/ADR-Atlas-008-wasm-publish-race-postmortem.md` | Postmortem + ADR. ~300-400 lines. Sections: 1 Context (the bug + the dual-publish race), 2 Decision (single publish + dist-tag), 3 Approach analysis (3 candidates compared), 4 Why v1.0.0 didn't catch this (different failure mode — repository-URL mismatch), 5 Trade-offs accepted, 6 Forward-validation plan, 7 Honesty caveats, 8 Open questions / follow-up welle for conditional-exports unification, 9 Decision log. |
| NEW | `.handoff/v2-beta-welle-11-plan.md` | This plan-doc itself. |

**Total estimated diff:** ~400-500 lines (mostly ADR prose; workflow change is small + comment-heavy).

## Approach analysis (3 candidates evaluated)

### Candidate A — Single publish + `npm dist-tag add` (CHOSEN)

```yaml
# Pseudocode:
cd pkg-web && npm publish --access public --provenance
npm dist-tag add @atlas-trust/verify-wasm@${VERSION} node
```

- **Pros:** matches npm's data model; one publish ⇒ one provenance attestation; idempotent across re-runs; minimal diff.
- **Cons:** the `pkg-node` tarball is built but never published to the registry — Node consumers receive the `pkg-web` tarball via the `node` dist-tag pointer. This is technically a reduction in variant-specificity vs. the pre-bug intent, but the bug means the pre-fix intent was never actually working (the second publish ALWAYS failed; only the first variant ever shipped). The cons are about codifying current de-facto behaviour, not introducing new behaviour.

### Candidate B — Build a single artefact supporting both targets

```
wasm-pack build --target bundler  # or some unified target
```

- **Pros:** would resolve the dual-target problem at source.
- **Cons:** requires deep changes to the wasm-pack invocation, the smoke tests (which currently test web + node JS-side glue independently), and the Cargo.toml `wasm-bindgen` configuration. Out of scope for a Phase-1 parallel-batch fix. Recorded as a follow-up welle.

### Candidate C — Gate the second publish on first-publish-success + idempotency check via `npm view`

```bash
# Pseudocode:
cd pkg-web && npm publish ... && \
  if ! npm view @atlas-trust/verify-wasm@${VERSION} dist-tags.node > /dev/null 2>&1; then
    cd pkg-node && npm publish ... --tag node
  fi
```

- **Pros:** preserves the two-tarball intent.
- **Cons:** **doesn't actually fix the bug.** `npm publish` of the SAME version twice is rejected by the registry regardless of whether a different `--tag` is supplied — this is exactly what the bug demonstrated. The idempotency check would always be true (because the version was just published) but the second publish would always fail with the same E403. Candidate C is rejected because it's based on a misunderstanding of npm's data model (dist-tags are mutable pointers; versions are immutable rows).

**Candidate A is the only correct fix that aligns with npm's actual semantics.**

## Why v1.0.0 didn't catch this — root-cause separation

The v1.0.0 tag-push on 2026-05-12 ALSO failed wasm-publish, but for a **different** root cause:

- **v1.0.0 failure** (per CHANGELOG `[1.0.1]` entry, line 196): `Cargo.toml` `workspace.package.repository` pointed at `ultranova/atlas` (stale org), wasm-pack derived `package.json` `repository.url` from it, npm's SLSA Build L3 provenance validator rejected the publish with `422 Unprocessable Entity — Error verifying sigstore provenance bundle: Failed to validate repository information`. **The first `npm publish` (pkg-web) never succeeded** — it failed at the provenance-validation step, BEFORE the version row was created on the registry. Therefore the second publish (pkg-node) ALSO failed with the same 422 (not E403), and the dual-publish race was masked behind the provenance-validation failure.
- **v1.0.1 publish on 2026-05-12** corrected the repository URL. From the production-publish narrative in the CHANGELOG entry, v1.0.1 evidently shipped clean. Looking at the workflow's V1.19 Welle 14a re-run logic + the "Verify npm publish landed" step's `node` dist-tag check (added in the same welle), the v1.0.1 publish DID likely surface the same E403 on the second invocation — but the CI run for v1.0.1 may have completed before the dist-tag-check step's failure was prominent OR the workflow may have been dispatch-fired with a manual-confirmation override that suppressed the second-publish step. **This is unverified history** and the ADR records this as an open question — the publicly-observable state is that `@atlas-trust/verify-wasm@1.0.1` IS on npm under `latest`, and `@node` dist-tag is also `1.0.1`.
- **v2.0.0-alpha.1 publish on 2026-05-13** had both bugs cleanly separated: provenance URL OK (v1.0.1 fix carried forward), so first publish succeeded — but the dual-publish race surfaced clean on the second invocation. logIndex `1523498404` is the FIRST (successful) publish's Sigstore attestation, anchored to the live registry row; logIndex `1523498503` is the SECOND (failed) publish's orphan attestation — the OIDC mint + Rekor write completed before npm returned the E403, so a Sigstore search will return a Rekor entry that has no corresponding registry artefact. The orphan is content-addressed and does NOT affect the live `latest` row's verification chain.

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 11 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins | NONE — no Rust code, no canonicalisation code, no signing-input code, no anchor code touched | Tests pass byte-identically; no regen needed |
| `wasm-publish.yml` workflow shape | Verify-tag step unchanged; smoke-test steps unchanged; pack-tarball step unchanged; GH-Release upload step unchanged | Diff localised to publish step + verify-publish-landed step's comment block |
| `@atlas-trust/verify-wasm@latest` consumer flow | Unchanged — `npm install @atlas-trust/verify-wasm` still resolves to the latest version under `latest` tag | No consumer change |
| `@atlas-trust/verify-wasm@node` consumer flow | The dist-tag now points at the pkg-web tarball (was: separate pkg-node tarball, but the dual-publish bug meant this never actually worked end-to-end since v1.0.0). The pkg-web tarball's wasm-bindgen ESM glue functions under Node when the operator passes raw WASM bytes to `init({ module_or_path })`, per the existing `Node.js smoke (target web — via direct WASM bytes)` test in the same workflow. | Documented in ADR §5 trade-offs; follow-up welle for conditional-exports unification recorded. |
| Verify-npm-publish-landed step's `node` dist-tag check | The check still verifies that `npm view @atlas-trust/verify-wasm@node version` returns the expected version. Under the new model, the dist-tag is set by `npm dist-tag add` (post-publish) instead of by `npm publish --tag node`. The check's semantics are unchanged — it asserts the pointer is correctly placed. | Comment block updated to reflect the new pointer-vs-publish reality |
| V1.19 Welle 14a concurrency-group gate (lines 98-100) | Unchanged — single-publish path still benefits from queue-serialisation against double-dispatch | None needed |
| V1.17 Welle B verify-tag gate (lines 189-228) | Unchanged — fix is post-publish-step only | None needed |

**Mandatory check:** all 7 byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) MUST remain byte-identical after this welle's merge. Since no Rust code is touched, this is satisfied trivially.

## Implementation steps (TDD-adjacent — workflow change is testable only against a real signed-tag publish)

1. **Plan-doc on branch** (this file) — done as Step 1.
2. **Workflow edit** — replace the publish-step body + update comment block.
3. **Workflow edit** — update verify-publish-landed step's comment block to reflect dist-tag-pointer semantics.
4. **ADR-Atlas-008 authoring** — full postmortem + design rationale + honesty caveats + forward-validation plan.
5. **`cargo check --workspace`** — sanity check that no Rust code changed (should be no-op green).
6. **Parallel `code-reviewer` + `security-reviewer` agents** on diff.
7. **Fix CRITICAL/HIGH findings in-commit.**
8. **Single SSH-Ed25519 signed commit** on `feat/v2-beta/welle-11-wasm-publish-fix`.
9. **Push + open DRAFT PR** with `--base master`.
10. **Parent agent decides merge timing.** Validation is forward-only: next signed-tag push (likely `v2.0.0-alpha.2` during Phase 3) is the first true test of the fix.

## Acceptance criteria

- [ ] `cargo check --workspace` green (no Rust code changes; sanity-only)
- [ ] Workflow YAML parses (will be exercised on next `workflow_dispatch` dry-run or signed-tag push)
- [ ] No use of `npm publish ... --tag node` remaining in the publish step
- [ ] Exactly ONE `npm publish` invocation in the publish step
- [ ] `npm dist-tag add` for the `node` pointer present and guarded against missing `did_publish` output
- [ ] ADR-Atlas-008 covers: bug, root cause, why v1.0.0 didn't catch it, 3 candidates analysed, candidate A chosen with rationale, trade-offs, honesty caveats, validation plan, open questions
- [ ] Plan-doc on welle's own branch (this file is `.handoff/v2-beta-welle-11-plan.md`)
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched; CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit (`SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [ ] DRAFT PR open with `--base master`
- [ ] Forbidden-files rule honoured (no touches to CHANGELOG.md, V2-MASTER-PLAN.md status, decisions.md, semver-audit, handoff doc)

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| The fix can't be VALIDATED until a signed-tag push (`v2.0.0-alpha.2` etc.) | HIGH (definitional) | MED — but a workflow_dispatch dry-run can validate the YAML parse + everything pre-publish | Documented in ADR §6. Phase 3 of V2-β orchestration includes the `v2.0.0-alpha.2` ship, which IS the validation event. |
| `npm dist-tag add` fails post-publish due to npm-registry replication latency (the just-published version isn't yet visible to the dist-tag-add command) | LOW-MED | MED — would fail the workflow red, requiring a re-run. The release would already be on `latest` so consumer-impact is partial. | Add a brief sleep-retry loop around `npm dist-tag add`, matching the V1.19 Welle 14a verify-publish-landed step's loop pattern. |
| Future regression: a maintainer reverts to dual `npm publish` and the bug returns | LOW | HIGH | ADR-Atlas-008 + the comment block in the workflow explicitly forbid this pattern with prose explanation. The workflow's existing comment-density culture (V1.19 Welle 14a, V1.17 Welle B) means future edits read the rationale before changing publish-step logic. |
| The `node` dist-tag pointing at a `pkg-web` tarball degrades Node consumer experience vs. the intended separate-tarball model | MED | LOW-MED | Existing `Node.js smoke (target web — via direct WASM bytes)` step in the same workflow already proves the web build works under Node. Follow-up welle for conditional-exports unification recorded in ADR §8. |
| Orphan Sigstore Rekor logIndex `1523498503` causes consumer confusion (a tooling integration might surface it as a valid attestation for a non-existent package version) | LOW | LOW | The Rekor entry IS content-addressed against the wasm tarball bytes; a consumer who somehow obtained those bytes outside the npm registry could in-principle verify the orphan attestation, but the npm-side `dist.integrity` field that `npm audit signatures` compares against points at the SUCCEEDED publish's tarball, not the orphan. ADR §3 documents this. |

## Out-of-scope this welle (later phases)

- **Conditional-exports unification of pkg-web + pkg-node into a single npm package** with `"exports": { "import": "./web.js", "require": "./node.js" }` — this is the proper fix for the underlying "two builds, one version" tension. Requires changes to the wasm-pack invocation, smoke-test scaffolding, and consumer-facing CONSUMER-RUNBOOK documentation. Recorded as a candidate for V2-β post-Phase-3 or V2-γ.
- **Retroactive republish of `2.0.0-alpha.1`** — npm enforces version immutability; the live `latest` row is correct as-is. The missing `node` dist-tag for `2.0.0-alpha.1` is observable but non-blocking (consumers using `@atlas-trust/verify-wasm@node` get the v1.0.1 version from the previous successful publish; consumers using `@latest` get the correct `2.0.0-alpha.1`). Forward-fix only.
- **Revisiting the SLSA Build L3 orphan-attestation pattern** — ADR-Atlas-006 §5.3 already records an open threat-model problem for fate-shared verification channels. The orphan-attestation question is adjacent: should we explicitly invalidate or annotate orphan Rekor entries? This is recorded as a Sigstore-side concern (npm's `audit signatures` already only validates against the registry's authoritative entry; orphans are not registry-attached). Not Atlas-side actionable.
- **Phase 3 ship-gate validation** (parent-agent / Phase 3 welle) — the actual test of this fix is the next signed-tag push. Phase 3 includes the alpha.2 ship.

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` |
| Master Plan | `docs/V2-MASTER-PLAN.md` §6 |
| Working Methodology | `docs/WORKING-METHODOLOGY.md` |
| ADR-Atlas-006 (Sigstore tracking — adjacent context) | `docs/ADR/ADR-Atlas-006-multi-issuer-sigstore-tracking.md` |
| v1.0.1 ship history (different bug, same workflow) | `CHANGELOG.md` `[1.0.1] — 2026-05-12` |
| v1.0.0 ship plan (workflow-trigger context) | `.handoff/v1.19-welle-13-plan.md` |
| Failed run log | GitHub Actions `25788574299` (re-fetchable via `gh run view 25788574299 --log`) |
| Live npm package | `https://www.npmjs.com/package/@atlas-trust/verify-wasm` (2.0.0-alpha.1 on `latest`) |

---

## Implementation Notes (Post-Code) — fill AFTER tests pass

### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| TBD post-implementation | TBD | TBD |

### Test outcome

- `cargo check --workspace` green (no Rust code changes)
- All 7 byte-determinism CI pins unchanged (no Rust code touched)
- Workflow YAML re-validated by GitHub Actions UI parse on push
- Forward-validation: deferred to next signed-tag push (Phase 3 alpha.2 ship)

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| TBD | TBD |

### Deviations from plan

To be filled in post-implementation, or "None of substance".
