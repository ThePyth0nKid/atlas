# ADR-Atlas-008 — `wasm-publish.yml` Dual-Publish Race Postmortem

| Field        | Value                                                    |
|--------------|----------------------------------------------------------|
| **Status**   | Accepted (forward-fix applied)                           |
| **Date**     | 2026-05-13                                               |
| **Wave**     | V2-β Welle 11                                            |
| **Authors**  | Nelson Mehlis (`@ThePyth0nKid`); welle-11 subagent       |
| **Replaces** | —                                                        |
| **Superseded by** | —                                                   |
| **Related**  | V1.14 Scope E (npm publish lane introduction); V1.17 Welle B (SSH tag-signing gate); V1.19 Welle 14a (npm publish completion + post-publish verification); ADR-Atlas-006 (Sigstore multi-issuer tracking — touchpoint B); V2-α Welle 8 (`v2.0.0-alpha.1` ship — first run that surfaced this bug cleanly) |

---

## 1. Context

### 1.1 The observed failure

On 2026-05-13 at 08:50 UTC, the signed-tag push of `v2.0.0-alpha.1` (commit `47b6894`, key `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`) fired `wasm-publish.yml` run `25788574299`. The run completed all pre-publish steps green (checkout, tag-signature verification, wasm-pack web + node builds, pkg-web + pkg-node smoke tests, tarball pack, artifact upload) but failed RED at the `Publish to npm` step with the following sequence:

1. **08:50:24** — pkg-web `npm publish --access public --provenance` invoked. Tarball metadata printed (`atlas-trust-verify-wasm-2.0.0-alpha.1.tgz`, sha512 `K1hBo7BzHJumx[...]AetxEPW2yCylA==`). OIDC token minted, Fulcio cert issued, Sigstore Rekor entry recorded at **logIndex `1523498404`**. npm 200 OK: `+ @atlas-trust/verify-wasm@2.0.0-alpha.1`. **Registry row created; `latest` dist-tag pointed at `2.0.0-alpha.1`.**
2. **08:50:27** — pkg-node `npm publish --access public --provenance --tag node` invoked. Different tarball bytes (sha512 `p1hXVTueivZbn[...]WBOIBs+nA/8NA==`), same `name` + same `version` in `package.json`. OIDC token minted again, Fulcio cert issued again, Sigstore Rekor entry recorded at **logIndex `1523498503` (the orphan)**. npm responded **`E403 — 403 Forbidden — PUT https://registry.npmjs.org/@atlas-trust%2fverify-wasm — You cannot publish over the previously published versions: 2.0.0-alpha.1.`**

The CI run terminated red with exit code 1. The downstream `Verify npm publish landed` step was gated on `steps.publish.outputs.did_publish == 'true'`, which was never set (the publish step exited 1 before writing the output), so the post-publish assertions did not fire.

**Despite the red CI run, the live state of the registry is:**
- `@atlas-trust/verify-wasm@2.0.0-alpha.1` IS published, IS on the `latest` dist-tag, has the pkg-web tarball bytes, has a valid SLSA Build L3 provenance attestation (logIndex `1523498404`).
- The `node` dist-tag was NOT updated for the `2.0.0-alpha.1` version and still points at whatever the last successful node-publish set it to (presumably `1.0.1` from the v1.0.1 ship, but per §1.5 this is itself unverified-history).
- A second Sigstore Rekor entry (logIndex `1523498503`) exists for a tarball that does NOT correspond to any registry row — the **orphan attestation**. Its signing-input bytes are content-addressed against the pkg-node tarball (sha512 `p1hXVTue...`); a consumer who somehow obtained those tarball bytes outside the registry could in-principle verify the Sigstore signature, but no registry-attached consumer path would surface this attestation because npm's `dist.integrity` field points at the pkg-web tarball's sha512, not the pkg-node tarball's.

### 1.2 The root cause

The `Publish to npm` step (pre-fix) invoked `npm publish` **twice** against the same `name` + `version`:

```bash
cd crates/atlas-verify-wasm/pkg-web
npm publish --access public --provenance              # → publishes 2.0.0-alpha.1 at `latest`
cd "${GITHUB_WORKSPACE}"
cd crates/atlas-verify-wasm/pkg-node
npm publish --access public --provenance --tag node   # → fails E403; same version, can't republish
```

The intent was: ship pkg-web at `latest`, ship pkg-node at the `node` dist-tag, so that browser consumers get `@atlas-trust/verify-wasm@latest` (CommonJS-incompatible but ESM-native) and Node consumers get `@atlas-trust/verify-wasm@node` (CommonJS-compatible). The implementation tried to express this via two separate `npm publish --tag X` invocations.

**This implementation is incompatible with npm's data model.** In the npm registry:
- A `version` is an IMMUTABLE row in the package's version-table. Once published, the tarball bytes, package.json contents, and signed provenance attestation are frozen forever (modulo `npm unpublish` within 72 hours, which has its own restrictions).
- A `dist-tag` is a MUTABLE pointer to an already-published version. `latest`, `node`, `beta`, `next` are all just labels that point at version rows.
- `npm publish --tag X` is `npm publish <version> + npm dist-tag add @scope/name@<version> X` fused into one operation. If `<version>` already exists as a registry row, the publish step fails E403 regardless of the dist-tag.

The pre-fix step assumed `--tag node` made the second publish operate on a different "slot" from the first publish. It does not. A version is a version; a dist-tag is a pointer; two publishes of the same version cannot both succeed.

### 1.3 Why the package is live despite the red run

The first `npm publish` (pkg-web) succeeded BEFORE the second one ran. By the time npm returned E403 to the second invocation, the registry was already in the state: `@atlas-trust/verify-wasm@2.0.0-alpha.1` exists with the pkg-web bytes, signed provenance, and `latest` dist-tag set. The workflow's red exit code did not retroactively unpublish anything (and could not — npm doesn't support that).

The failed-workflow appearance is therefore **misleading-but-non-blocking**: the package IS shipped, the SLSA Build L3 provenance IS valid (logIndex `1523498404`), and `npm install @atlas-trust/verify-wasm@latest` (or `npm install @atlas-trust/verify-wasm@2.0.0-alpha.1` explicitly) resolves correctly.

### 1.4 Why v1.0.0 did not catch this (different bug)

The v1.0.0 tag-push on 2026-05-11 also failed wasm-publish, but for a **different** root cause. Per `CHANGELOG.md` `[1.0.1] — 2026-05-12`:

> This release corrects a `Cargo.toml` `workspace.package.repository` field that pointed at a stale organisation path (`https://github.com/ultranova/atlas`) instead of the canonical `https://github.com/ThePyth0nKid/atlas`. wasm-pack derives `package.json`'s `repository.url` from that Cargo field; npm's SLSA Build L3 provenance validator rejected the v1.0.0 publish attempt because the package.json URL did not match the GitHub Actions OIDC token's source-repository claim (`422 Unprocessable Entity — Error verifying sigstore provenance bundle: Failed to validate repository information`).

The v1.0.0 first `npm publish` (pkg-web) never succeeded — it failed at the provenance-validation step with HTTP 422, BEFORE the version row was created on the registry. Therefore:
1. The registry never had a v1.0.0 row at all (no orphan publish).
2. The second `npm publish` (pkg-node) ALSO failed with the same 422 (not E403), because it tried to publish the same version against the same provenance-failed gate.
3. **The dual-publish race was masked behind the provenance-validation failure.** Both publishes failed for the same upstream reason; the second failure looked superficially similar to "publish in general doesn't work" rather than to "two publishes of the same version are incompatible."

The v1.0.0 → v1.0.1 fix corrected the repository URL. The v1.0.1 publish on 2026-05-12 evidently shipped (the npm registry has `@atlas-trust/verify-wasm@1.0.1` on `latest`, and per `CHANGELOG.md` `[1.0.1]` it was the first version published to npm). What is **unverified history** is whether the v1.0.1 run ALSO surfaced the E403 on the second invocation: the workflow log for that run is not directly inspectable from this welle's vantage, and the orchestration record does not capture step-level outcomes for v1.0.1. The publicly-observable consumer-side state for v1.0.1 is consistent with EITHER (a) the same dual-publish race happened and the run was red, OR (b) the publish was somehow fired via a single-invocation manual override (e.g. `workflow_dispatch` after pre-existing manual `npm dist-tag add`). The likely-but-unconfirmed scenario is (a): v1.0.1 shipped despite a red CI run, exactly like v2.0.0-alpha.1, but the failure was not widely surfaced because the package being live on `latest` met the operator's success criterion.

**v2.0.0-alpha.1 is therefore the first ship where the dual-publish race was cleanly isolated** (provenance URL OK from v1.0.1 carry-forward → first publish succeeds → second publish fails with E403, with no other failure-mode confounding the diagnostic).

### 1.5 Why this matters

The Atlas trust posture rests heavily on `@atlas-trust/verify-wasm`'s npm publish lane being clean (per ADR-Atlas-006 §1.1 touchpoint B). A red CI run on every release-tag push has three concrete costs:

1. **Operator confusion**: "did the release ship or not?" requires manual inspection of the npm registry to disambiguate. Every release becomes a manual reconciliation event.
2. **Consumer-side `npm audit signatures` integrity**: still works correctly because the npm registry's authoritative entry has a valid SLSA L3 provenance from logIndex `1523498404`. The orphan logIndex `1523498503` does NOT appear in any consumer-side verification path.
3. **`node` dist-tag drift**: with no successful pkg-node publish, the `node` dist-tag is not updated to point at the new release version. Consumers running `npm install @atlas-trust/verify-wasm@node` will receive whatever the last successful pre-bug pkg-node publish set, which drifts further behind `latest` on every new release.

Cost (1) is operational noise; cost (2) is unaffected; cost (3) is a real consumer-impact drift that this welle directly addresses.

---

## 2. Decision

**Replace the dual `npm publish` invocations in `wasm-publish.yml`'s publish step with a single `npm publish` for pkg-web at `latest`, followed by `npm dist-tag add` to point the `node` dist-tag at the same version.**

The post-fix publish-step body (in plain bash, abridged):

```bash
cd crates/atlas-verify-wasm/pkg-web
npm publish --access public --provenance
cd "${GITHUB_WORKSPACE}"
PUBLISHED_VERSION="$(node -p "require('./crates/atlas-verify-wasm/pkg-web/package.json').version")"
PACKAGE_NAME="${NPM_PACKAGE_SCOPE}/${NPM_PACKAGE_NAME}"
# Retry loop absorbs registry replication latency between publish + dist-tag visibility.
for i in $(seq 1 6); do
  if npm dist-tag add "${PACKAGE_NAME}@${PUBLISHED_VERSION}" node; then
    DIST_TAG_OK=true; break
  fi
  sleep 5
done
[ "${DIST_TAG_OK}" = "true" ] || exit 1
```

Trade-offs accepted with this decision are enumerated in §5.

---

## 3. Approach analysis — three candidates evaluated

### Candidate A — Single publish + `npm dist-tag add` (CHOSEN)

**Mechanism:** publish pkg-web at default `latest`, then run `npm dist-tag add @atlas-trust/verify-wasm@<version> node` as a pointer-only operation post-publish.

**Pros:**
- Matches npm's actual data model (versions immutable; dist-tags mutable pointers).
- Eliminates the dual-publish race by construction (only one `publish` call ever fires).
- Idempotent across re-runs: `npm dist-tag add` against an already-pointed-at version is a no-op exit 0.
- One Sigstore Rekor entry per release (no orphans). The consumer-facing supply-chain signal is cleaner.
- Minimal diff (~30 modified lines in the publish step; comment block expansion accounts for most of the change).
- Preserves the pkg-node build's role in the pack/upload/smoke-test pipeline (only the npm-registry upload is single-tarball; the GH-Release backup channel still carries both web and node tarballs separately).

**Cons:**
- Node consumers (`npm install @atlas-trust/verify-wasm@node`) receive the pkg-web tarball instead of a separately-built pkg-node tarball. **However:** per §1.4, no public release since v1.0.0 has actually shipped a separate pkg-node tarball — the dual-publish race always failed the second publish. The de-facto consumer-side state for the entire history of the published package has been "Node consumers get pkg-web at `node` dist-tag (when the pointer drifts forward) or nothing (when it doesn't)." This decision codifies the de-facto state into the de-jure design rather than introducing new behaviour.

### Candidate B — Build a single artefact supporting both targets

**Mechanism:** invoke `wasm-pack build --target bundler` (or some unified target) instead of separate `--target web` and `--target nodejs` builds, and ship a single tarball that uses conditional exports (`"exports": { "import": "./web.js", "require": "./node.js" }`) to serve both consumers from one publish.

**Pros:**
- Would resolve the dual-target problem at source, eliminating the need for any dist-tag distinction.
- Cleanest long-term architecture: one package, one version, one provenance attestation, two consumer entry points.

**Cons:**
- Requires deep changes to:
  - `wasm-pack` invocation in the workflow (and possibly the wasm-pack version pin, since some unified-target features are version-specific).
  - The smoke-test scaffolding, which currently tests web + node JS-side glue independently — these tests would need to be re-imagined for a single-tarball world.
  - The Cargo.toml `wasm-bindgen` configuration to ensure the unified-target output is compatible with both ESM and CommonJS consumers.
  - Consumer-side documentation (`docs/CONSUMER-RUNBOOK.md` if it exists, the verify-wasm-pin-check@v1 action's recommended setup).
- Out of scope for a Phase-1 parallel-batch fix in the V2-β orchestration plan. The orchestration plan's "in-scope" for W11 is "workflow fix"; "in-scope" for a B-style refactor would be much larger and depend on coordinated wasm-pack toolchain changes.

**Disposition:** rejected for this welle; recorded as a candidate follow-up welle in §8.

### Candidate C — Gate the second publish on first-publish-success + idempotency check

**Mechanism:** wrap the second `npm publish` in an `if`-block that checks via `npm view` whether the version is already published.

```bash
cd pkg-web && npm publish --access public --provenance && \
  PUBLISHED_VERSION="$(node -p 'require(\"./package.json\").version')" && \
  cd "${GITHUB_WORKSPACE}" && cd pkg-node && \
  if ! npm view @atlas-trust/verify-wasm@${PUBLISHED_VERSION} > /dev/null 2>&1; then \
    npm publish --access public --provenance --tag node; \
  fi
```

**Pros:**
- Preserves the two-tarball intent.
- Looks like a "defensive guard" pattern.

**Cons:**
- **Does not actually fix the bug.** The `npm view ... > /dev/null` check would ALWAYS succeed (return 0) right after the first publish, because the version IS now published. The check therefore always SKIPS the second publish, meaning the second publish never runs — but that means the second tarball (pkg-node bytes) NEVER reaches the registry at all, which is the same outcome as Candidate A except without the dist-tag pointer.
- A more sophisticated version (check via `npm view ...@node` to see if the `node` dist-tag is unset) is also broken: `npm publish --tag node` of an already-published-version FAILS E403 regardless of the dist-tag's current state — this is the original bug. The check might pass (`node` dist-tag is not yet at this version) but the publish would still fail.
- Candidate C is rejected because it's based on a misunderstanding of npm's data model. There is no `--force-tag-only` mode in `npm publish`; dist-tags must be managed via `npm dist-tag add` after the publish.

**Disposition:** rejected as architecturally unsound.

**Candidate A is the only correct fix that aligns with npm's actual semantics.**

---

## 4. Why v1.0.0 did not surface this (root-cause separation, expanded)

This section expands on §1.4 to make explicit which failure modes belong to which fixes.

| Release | Date | First publish (pkg-web) | Second publish (pkg-node) | Root cause | Resolution |
|---|---|---|---|---|---|
| v1.0.0 | 2026-05-11 | FAILED 422 — provenance URL mismatch | FAILED 422 — same root cause | `Cargo.toml` `repository` field pointed at `ultranova/atlas`; OIDC `repository` claim was `ThePyth0nKid/atlas`; SLSA Build L3 validator rejected on the URL mismatch | v1.0.1 patch (URL fix); registry never had a v1.0.0 row |
| v1.0.1 | 2026-05-12 | SUCCEEDED — provenance URL now matches | LIKELY FAILED E403 (unverified history) — would have been the same dual-publish race as v2.0.0-alpha.1, but the workflow-run log for v1.0.1 is not directly inspectable in this welle | Most likely the dual-publish race (this welle's bug), but masked by operator-side acceptance: package was live on `latest`, so the success criterion was met. | This welle (forward fix) addresses the race for future releases |
| v2.0.0-alpha.1 | 2026-05-13 | SUCCEEDED — logIndex `1523498404`, `latest` dist-tag set, package live | FAILED E403 — orphan logIndex `1523498503` on Sigstore Rekor | Dual-publish race (this welle's bug). No URL-mismatch confounding factor; the race was cleanly isolated. | This welle (forward fix) |
| v2.0.0-alpha.2 (anticipated, Phase 3 of V2-β) | TBD | Should SUCCEED (no behavioural change from alpha.1) | N/A — replaced by `npm dist-tag add` post-publish | Forward-fix from this welle applied | Validation event for this ADR |

**Key insight:** the dual-publish race is a pre-existing bug that has likely been latent since V1.14 Scope E introduced the publish lane. It did not surface until BOTH (a) the first publish was succeeding (i.e. provenance was OK) AND (b) the workflow's exit-code red was no longer masked by other failure modes. v2.0.0-alpha.1 is the first release where both conditions held cleanly, making the race observable as a discrete bug.

---

## 5. Trade-offs accepted with the chosen fix

### 5.1 The `node` dist-tag now points at a pkg-web tarball

The pre-fix intent was: Node consumers receive a CommonJS-glue tarball (pkg-node) when they install `@atlas-trust/verify-wasm@node`. The post-fix reality is: Node consumers receive the ESM-glue tarball (pkg-web) when they install `@atlas-trust/verify-wasm@node`, because the `node` dist-tag is now a pointer at the single published version.

**Why this is acceptable in V2-β:**

- The de-facto pre-fix consumer-side state was already "Node consumers get pkg-web at the `node` dist-tag (when the dist-tag drifts forward via a successful pkg-node publish) or nothing (when it doesn't)." Since the dual-publish race blocked every successful pkg-node publish from v1.0.0 onward, the pre-fix `node` dist-tag has been drifting since the start of the published package's history. The post-fix codifies the de-facto state into the de-jure design.
- The wasm-pack web build's ESM glue functions correctly under Node when the operator passes raw WASM bytes to `init({ module_or_path })`. This is exercised by the `Node.js smoke (target web — via direct WASM bytes)` step that runs earlier in this same workflow on every release. The smoke test demonstrates that the pkg-web tarball is functionally usable from Node.
- A consumer who specifically needs CommonJS-glue (because they cannot use ESM dynamic imports) is currently broken under the pre-fix workflow anyway — they receive a stale-by-N-releases pkg-node tarball or nothing. The post-fix at least guarantees they receive a usable build at the current version.

**Why this is NOT acceptable long-term:**

- A consumer who expects a CommonJS-native package (no `await import(...)` dance, no raw-WASM-bytes passing) under the `node` dist-tag is silently degraded to ESM-glue + manual init. This is a documentation gap that should be closed by a follow-up welle.

### 5.2 Orphan Sigstore Rekor logIndex `1523498503` remains in the transparency log forever

The Rekor transparency log is append-only. The orphan entry for the failed pkg-node publish on 2026-05-13 will remain in the log indefinitely. It is content-addressed against the pkg-node tarball's sha512 bytes; a consumer who obtained those bytes outside the npm registry could in-principle verify a valid Sigstore signature against them.

**Why this is acceptable:**

- `npm audit signatures` (the consumer-side primary verification path) only validates against the registry's authoritative `dist.integrity` field, which points at the pkg-web tarball's sha512 (logIndex `1523498404`), not the orphan tarball's sha512. The orphan is NOT in any registry-attached consumer verification path.
- A consumer would only encounter the orphan if they explicitly searched Sigstore Rekor by-tarball-hash for an Atlas tarball, which is not a documented or supported flow. The Atlas trust model documents npm-registry-attached `dist.integrity` + `npm audit signatures` as the consumer-side path (ADR-Atlas-006 §1.1 touchpoint B, C).
- The orphan does NOT create a forgery primitive. It is a valid signature over real Atlas-built bytes, signed by the GitHub Actions OIDC identity for `ThePyth0nKid/atlas`. It just doesn't correspond to a live registry artefact.

**Why this is documented:**

- A future auditor inspecting the Sigstore Rekor log for Atlas attestations would see two entries for `2.0.0-alpha.1` and could reasonably ask "which one is canonical?" The answer is "logIndex 1523498404; the other is an orphan from a failed publish that nonetheless completed its Sigstore signing phase before npm returned 409." This ADR is the audit trail for that question.

### 5.3 Forward-fix only — no retroactive republish of `2.0.0-alpha.1`

The published-but-incomplete state of `2.0.0-alpha.1` (live on `latest`, `node` dist-tag not updated for this version) is NOT corrected by this welle. The reasons:

- npm enforces version immutability. `npm unpublish @atlas-trust/verify-wasm@2.0.0-alpha.1` would succeed only within 72 hours of the original publish, AND would invalidate the SLSA Build L3 provenance attestation (the registry-attached entry is gone, so the consumer-side `npm audit signatures` chain breaks for any consumer who already installed 2.0.0-alpha.1).
- The post-hoc manual fix — running `npm dist-tag add @atlas-trust/verify-wasm@2.0.0-alpha.1 node` against a maintainer-authenticated session — would correct the `node` dist-tag for the current version, BUT would not produce the audit-trail signal of "the workflow ran clean" that this welle's forward-fix aims to provide.
- The cleaner forward path is: this welle merges, the next release tag (`v2.0.0-alpha.2`) ships clean (this welle's fix is exercised), and the `2.0.0-alpha.1` `node` dist-tag is allowed to remain in its current state as a historical record. Consumers using `@latest` are unaffected; consumers using `@node` for `2.0.0-alpha.1` are receiving a slightly-older version (the last successful pkg-node publish, which was v1.0.1 if that one shipped clean, or v0.x earlier). The mitigation is "advance to alpha.2 quickly."

Alternatively, Nelson may decide to run a one-off `npm dist-tag add @atlas-trust/verify-wasm@2.0.0-alpha.1 node` against a maintainer-authenticated session to align the dist-tag with `latest`. This is a stand-alone operational decision and is NOT in scope for this welle.

---

## 6. Forward-validation plan

This fix CANNOT be validated until the next signed-tag push that exercises the `wasm-publish.yml` workflow's publish step. The validation plan:

### 6.1 Pre-merge validation (best-effort, partial)

- `cargo check --workspace` green — confirms no Rust code touched (sanity).
- Workflow YAML parses — GitHub Actions will reject any syntactic error on push to the branch / PR.
- `workflow_dispatch` from the welle branch with `dry_run: true` — exercises every step up to and including `Pack tarballs (artifacts)`, but does NOT exercise the publish step (which is gated on `(push,tag) || (workflow_dispatch,tag,!dry_run)`). This validates the workflow structure, the wasm-pack build, the smoke tests, and the tarball-pack step, but NOT the publish step itself.
- `workflow_dispatch` from the welle branch with `dry_run: false` (NOT recommended pre-merge) — would still skip the publish step because the dispatch is from a branch ref, not a tag ref. The publish step requires `github.ref_type == 'tag'`.

### 6.2 First true validation event

**The next signed-tag push that fires the workflow's publish step.** Per the V2-β orchestration plan, this is Phase 3's `v2.0.0-alpha.2` ship (parent agent, post-Phase-1-consolidation). Expected behaviour:

1. Pre-publish steps complete green (no behavioural change from alpha.1).
2. First `npm publish` (only) fires. Succeeds with a single Sigstore Rekor entry (one logIndex, not two). Registry row created; `latest` dist-tag set.
3. `npm dist-tag add @atlas-trust/verify-wasm@2.0.0-alpha.2 node` fires. Succeeds (no E403 — dist-tag-add does not create a registry row, it updates a pointer).
4. `Verify npm publish landed` step completes green. Both `latest` and `node` dist-tag checks pass (they now point at the same version).
5. CI run exits 0.

**Failure modes to watch for on the first validation event:**

- `npm dist-tag add` failing due to registry replication latency. Mitigated by the 6×5s retry loop. If even 30s is insufficient, the `MAX_ATTEMPTS` / `ATTEMPT_DELAY` parameters need re-tuning.
- `npm dist-tag add` permission rejection: the `NPM_TOKEN` used for the publish must also have dist-tag-write permission on the package. This is the same token, same scope, so should succeed; but a fresh token configured with publish-only-no-dist-tag scope would silently break this fix.
- A regression in upstream npm CLI behaviour where `dist-tag add` requires a different invocation pattern. Pinned npm version in `actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020` (v4.4.0) provides a reproducible baseline; bumping the node version in a future welle should be re-validated against this fix.

### 6.3 Post-validation step

After the first clean alpha.2 publish, this ADR's Status field updates from "Accepted (forward-fix applied)" to "Validated (alpha.2 ship clean)". Append a row to §10 Decision log with the run-id and the verified registry state.

---

## 7. Honesty caveats

This section is the explicit set of caveats that an external auditor reviewing this fix would want to know.

### 7.1 The fix cannot be validated pre-merge

Workflow-level changes cannot be unit-tested in CI before they fire on a real signed-tag push. The first true validation event is the next release-tag push (Phase 3's alpha.2 ship in the V2-β orchestration). If the fix has a subtle defect that surfaces only on a real publish path (e.g. an environment-variable substitution issue, a quoting bug in the retry loop, an interaction with the verify-publish-landed step's expectations), it will surface AT THE NEXT RELEASE, not during this welle's review.

**Mitigation:** the fix is small (~30 modified lines), localised (publish step only), and uses well-understood npm operations (`npm publish`, `npm dist-tag add`) with retry logic copied from the V1.19 Welle 14a verify-publish-landed step's proven pattern. The blast-radius of a defect is "alpha.2 ship is delayed by one welle to forward-fix the forward-fix," which is operationally tolerable.

### 7.2 The orphan Rekor logIndex `1523498503` cannot be retroactively removed

The Sigstore Rekor log is append-only by design (transparency-log invariant). The orphan entry remains in the log forever. This is documented; it does not create a forgery primitive (the signed bytes are real Atlas-built bytes); but it is an asymmetric trust signal that a future auditor might surface.

**Mitigation:** §1.1 + §5.2 of this ADR document the orphan explicitly. The audit trail is intact.

### 7.3 The de-facto consumer-side state for `@node` dist-tag has been broken since v1.0.0

Pre-fix, every release's `node` dist-tag publish failed E403, meaning the dist-tag was never advanced. Consumers using `npm install @atlas-trust/verify-wasm@node` have been receiving stale-by-N-releases tarballs since v1.0.0. This was not previously documented; this welle's fix advances the state forward but does not retroactively explain to existing consumers that their `@node` installs may be stale.

**Mitigation:** the next `OPERATOR-RUNBOOK.md` revision (out of scope for this welle, in scope for Phase-2 parent consolidation or a later welle) should include a "Node consumer dist-tag drift advisory" pointing at this ADR for the historical context.

### 7.4 The v1.0.1 ship-history is unverified

§1.4 and §4 acknowledge that the v1.0.1 release likely shipped despite a red CI run from the same dual-publish race. This is not directly verified in this welle (the run log for v1.0.1 is not inspected as part of W11). If a future auditor wants to confirm or refute the inference, the workflow-run log for v1.0.1's tag-push is the source-of-truth — accessible via `gh run list --workflow=wasm-publish.yml` filtered to that date range.

**Mitigation:** the inference is recorded as inference, not as fact. A follow-up note in `OPERATOR-RUNBOOK.md` could clarify the v1.0.1 ship-state for downstream operators.

### 7.5 No retroactive fix for `2.0.0-alpha.1`'s missing `node` dist-tag pointer

Per §5.3, the welle is a forward-fix only. `2.0.0-alpha.1`'s `node` dist-tag remains stale until either (a) Nelson runs a manual `npm dist-tag add` against an authenticated session, OR (b) `v2.0.0-alpha.2` ships and the dist-tag advances to alpha.2 automatically via the fixed workflow.

**Mitigation:** the parent agent's Phase-3 ship of alpha.2 IS the natural forward-fix for this drift. Operator-action option (a) is recorded as available but not required.

---

## 8. Open questions / follow-up work

### 8.1 Conditional-exports unification of pkg-web + pkg-node into a single package

**Question:** should the long-term architecture move to a single npm package with conditional exports (`"exports": { "import": "./web.js", "require": "./node.js" }`) that serves both browser and Node consumers from one tarball?

**Disposition:** YES, eventually. This is Candidate B from §3, deferred per orchestration-plan scope-discipline. Recommended welle scope:
- V2-β post-Phase-3 (after alpha.2 ships clean, validates this welle's fix) OR
- V2-γ as part of the broader supply-chain-modernisation arc.

The work spans: `wasm-pack` invocation refactor, smoke-test re-scaffolding for a unified-target world, Cargo.toml `wasm-bindgen` configuration review, CONSUMER-RUNBOOK rewrites if any. Not a single-session welle.

### 8.2 Should orphan Rekor entries be explicitly invalidated / annotated?

**Question:** is there a Sigstore-side mechanism to mark a Rekor entry as "orphan / no corresponding registry artefact"?

**Disposition:** NO at the Atlas layer. Rekor's append-only invariant prevents post-hoc mutation; annotation is not part of the Rekor data model. The proper consumer-side mitigation is what already exists: `npm audit signatures` validates against the registry's authoritative entry, not against arbitrary Rekor entries. Atlas's own Rekor-anchor verification (per ADR-Atlas-006 §1.1 touchpoint A) does NOT touch npm-publish-provenance entries; the touchpoints are separate. No Atlas-side action.

### 8.3 Should the `OPERATOR-RUNBOOK.md` add a "wasm-publish workflow re-fire / dist-tag-repair" section?

**Question:** when a future release's workflow encounters a transient failure between the `npm publish` and the `npm dist-tag add` steps, the operator needs a documented protocol for repairing the state.

**Disposition:** YES, but out of scope for this welle. Recommended scope: a Phase-2 parent-consolidation extension or a dedicated docs welle. Content should cover:
1. How to detect the partial-failure state (`npm view ...@<version>` works; `npm view ...@node version` does not match).
2. How to fix it (`npm dist-tag add @atlas-trust/verify-wasm@<version> node` against a maintainer-authenticated session).
3. How to confirm fix landed (re-run the verify-publish-landed step's queries manually).

### 8.4 Should a dedicated `node`-CommonJS-tarball variant be re-introduced under a different package name?

**Question:** if a Node-CommonJS-only consumer cohort exists and the `--target web` build's Node-via-raw-WASM-bytes init dance is unacceptable for them, should we publish a separate `@atlas-trust/verify-wasm-node` package?

**Disposition:** DEFER until consumer-demand evidence exists. The current trade-off in §5.1 is acceptable for V2-β. If V2-γ or later surfaces a customer requirement for a true CommonJS package, this becomes a scope item.

---

## 9. Reversibility

This decision is fully reversible. The workflow change is a localised edit; the dual-publish pattern could be re-introduced by reverting the publish step. The `npm dist-tag add` approach can be replaced by the conditional-exports unification (Candidate B) at any future point without breaking consumer-side compatibility (the `node` dist-tag remains a stable pointer that any future restructure can preserve).

The ADR documents the rejected candidates so a future maintainer revisiting this decision has the context to make an informed call.

---

## 10. Decision log

| Date       | Event                                                  | Outcome |
|------------|--------------------------------------------------------|---------|
| 2026-05-13 | ADR-Atlas-008 opened. V2-β Welle 11 fix applied to `.github/workflows/wasm-publish.yml`: dual `npm publish` replaced with single publish + `npm dist-tag add`. Forward-fix only; `2.0.0-alpha.1` registry state unchanged. | Status: Accepted (forward-fix applied) |

(Future entries appended on each validation event: first clean alpha.2 ship; any post-fix issue; any follow-up welle landing.)
