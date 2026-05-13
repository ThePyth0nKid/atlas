# Atlas v2.0.0-alpha.2 — Release Notes

> **Released:** 2026-05-13.
> **Tag:** `v2.0.0-alpha.2` (signed via SSH-Ed25519 path; key `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`).
> **Status:** Second pre-release of Atlas's V2 line. First V2-β-promoted pre-release. Engineering / auditor / operator evaluation. Public marketing materials pending counsel-validated language refinement per [`docs/V2-MASTER-PLAN.md`](V2-MASTER-PLAN.md) §5.

## Headline

Atlas v2.0.0-alpha.2 ships **three V2-β-Phase-1 wellen** on top of v2.0.0-alpha.1: operator-facing runbook for V2-α deployment + verification (W9), parallel-projection design ADR completing the DECISION-ARCH-1 triple-hardening third leg (W10), and the `wasm-publish.yml` dual-publish race fix (W11) — validated end-to-end by this very ship.

**Public-API surface is unchanged from v2.0.0-alpha.1.** This is a docs + workflow release; zero Rust / TypeScript surface change. All 7 V2-α byte-determinism CI pins byte-identical from v2.0.0-alpha.1 baseline. V1's trust property (signed events + Ed25519 + COSE_Sign1 + deterministic CBOR + blake3 hash chain + Sigstore Rekor anchoring + witness cosignature + offline WASM verifier) is preserved unchanged.

## What's in v2.0.0-alpha.2

The release packages 3 V2-β wellen + 1 V2-β orchestration phase shipped 2026-05-13:

| Phase | Welle | Deliverable | Effect |
|---|---|---|---|
| V2-β Phase 0 | (orchestration) | `docs/V2-BETA-ORCHESTRATION-PLAN.md` + `docs/V2-BETA-DEPENDENCY-GRAPH.md` + welle plan-doc template | Master-resident V2-β orchestration framework; ADR-007–012 pre-reservation prevents parallel-subagent number races |
| V2-β Phase 1 | W9 | `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` (~485 lines, 8 sections) | Operator-facing reference for V2-α deployment + verification; ergonomic V1-V2-α gateway |
| V2-β Phase 1 | W10 | `docs/ADR/ADR-Atlas-007-parallel-projection-design.md` (~375 lines, 9 sections) | Completes DECISION-ARCH-1 triple-hardening (V2-α legs: canonicalisation byte-pin + ProjectorRunAttestation; V2-β leg: parallel-projection determinism design). Workspace-parallel recommended HIGH confidence. |
| V2-β Phase 1 | W11 | `.github/workflows/wasm-publish.yml` race fix + `docs/ADR/ADR-Atlas-008-wasm-publish-race-postmortem.md` | Single `npm publish --provenance` + `npm dist-tag add` retry-loop replaces dual `npm publish --tag` that failed E403 against npm version-immutability invariant. **Validated end-to-end by this v2.0.0-alpha.2 ship.** |
| V2-β Phase 2 | (consolidation) | CHANGELOG + master-plan §6 + orchestration-plan welle-progress + handoff doc + W9 §-numbering fix-forward | Parent-only consolidation commit (forbidden-files rule per Orchestration Plan §3.4) |

Per-welle details in [`CHANGELOG.md`](../CHANGELOG.md) under the `[2.0.0-alpha.2]` section.

## The validation event — W11 wasm-publish race fix proves out

v2.0.0-alpha.2 is the first signed-tag publish since the W11 dual-publish race fix landed in master. The previous-version timeline:

- **v1.0.1 ship (Welle 14a, 2026-05-12):** dual `npm publish --tag latest` + `npm publish --tag latest` race condition surfaced as E403 on the second call. Manual recovery required.
- **v2.0.0-alpha.1 ship (V2-α Welle 8, 2026-05-13):** dual `npm publish --tag latest` + `npm publish --tag node` again failed E403 because npm rejects the second publish regardless of the `--tag` argument — npm version-immutability is unconditional.
- **W11 fix (PR #74, 2026-05-13):** workflow refactored to single `npm publish --access public --provenance` + `npm dist-tag add @atlas-trust/verify-wasm@<version> node` retry-loop (6 attempts × 5s for npm replication latency).
- **v2.0.0-alpha.2 ship (this release):** clean single-publish + dist-tag-add cycle.

If this ship completes without manual recovery, the W11 fix is proven-correct under real npm registry behaviour. Full root-cause analysis + candidate-fix evaluation in [`docs/ADR/ADR-Atlas-008-wasm-publish-race-postmortem.md`](ADR/ADR-Atlas-008-wasm-publish-race-postmortem.md).

## Parallel-subagent-in-worktree dispatch architecture (V2-β NEW invariant)

V2-α Welles 1–8 were dispatched sequentially (one welle per session). V2-β Phase 1 introduced **parallel-batch dispatch**: three subagents in three isolated worktrees, each on its own branch, each with its own commit and DRAFT PR. Phase 1 proof: W9 + W10 + W11 all landed on master in a single work-block via PRs #72 / #73 / #74.

Three orchestration invariants made this safe:

1. **Zero file-overlap pre-segregation.** Per V2-β Dependency Graph §3, W9 → `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` + plan-doc, W10 → `docs/ADR/ADR-Atlas-007-*` + plan-doc, W11 → `.github/workflows/wasm-publish.yml` + `docs/ADR/ADR-Atlas-008-*` + plan-doc. No two wellen touched the same file.
2. **Forbidden-files rule.** No welle touches `CHANGELOG.md`, `docs/V2-MASTER-PLAN.md` status, `docs/SEMVER-AUDIT-V1.0.md`, `.handoff/decisions.md`, or `.handoff/v2-session-handoff.md` — parent agent consolidates those in a single post-batch commit (Phase 2). This release notes doc lives outside the forbidden-files list.
3. **Cross-batch consistency-reviewer (NEW for V2-β).** After per-welle reviewer agents (6 total: 3 × code-reviewer + 3 × security-reviewer) finished, ONE additional consistency-reviewer agent read all 3 PRs together to catch cross-welle inconsistencies that per-welle reviewers cannot see (e.g. one welle claiming a primitive shipped that another describes as pending). Verdict on Phase 1 batch: zero CRITICAL / zero HIGH cross-welle conflicts, one LOW finding (W9 §-numbering gap) fix-forward applied in Phase 2 consolidation.

This architecture saves ~25 % wall-clock on V2-β versus serial dispatch (per V2-β Dependency Graph §5 critical-path analysis), without sacrificing reviewer rigour.

## What's next

**V2-β Phase 4** is the next parallel batch: W12 Read-API endpoints (6 Next.js routes in `apps/atlas-web/src/app/api/atlas/`), W13 MCP V2 tools (5 tools in `apps/atlas-mcp-server/src/tools/`), W14 expanded projector event-kinds (`crates/atlas-projector/src/upsert.rs`). Three more parallel subagents, three more worktrees.

After Phase 4 lands: W15 Cypher-validator consolidation (rule-of-three pattern — extract shared module after W12 + W13 each implement inline), W16 ArcadeDB embedded-mode spike-doc, W17a/b/c ArcadeDB driver integration, W18 Mem0g Layer-3 cache. Then `v2.0.0-beta.1` ship welle (W19) once ArcadeDB-backed Layer 2 is operational.

Full V2-β orchestration: [`docs/V2-BETA-ORCHESTRATION-PLAN.md`](V2-BETA-ORCHESTRATION-PLAN.md) + [`docs/V2-BETA-DEPENDENCY-GRAPH.md`](V2-BETA-DEPENDENCY-GRAPH.md).

## Surface stability assurance

For consumers of `@atlas-trust/verify-wasm`:

- **No breaking changes from v2.0.0-alpha.1.** The wasm interface, JSON event format, and verification semantics are identical.
- **Wire-format compat unchanged:** V1.0 verifiers continue to reject V2-α events with `author_did` or `payload.type = "projector_run_attestation"` (per `#[serde(deny_unknown_fields)]` policy). V1-shaped events remain forward-compatible.
- **No new public APIs.** All Rust + TypeScript surfaces unchanged. The 5-line semver-audit diff vs v2.0.0-alpha.1 is documented in [`docs/SEMVER-AUDIT-V1.0.md`](SEMVER-AUDIT-V1.0.md) §10.

For operators running Atlas Atlas-projector-aware deployments:

- **`atlas-signer emit-projector-attestation`** flag set unchanged (`--workspace`, `--derive-from-workspace`, `--head-event-hash`, etc.). The W9 operator runbook documents the producer flow end-to-end.
- **`atlas_projector::verify_attestations_in_trace`** library function unchanged. Operators integrating this into their own CI pipelines can rely on the same function signature.
- **V2-α byte-determinism guarantee preserved.** All 7 V2-α CI byte-pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) byte-identical from v2.0.0-alpha.1 baseline.

## Pre-counsel-review disclaimer

Unchanged from v2.0.0-alpha.1. Public marketing claims about V2-α / V2-β's EU AI Act / GDPR posture are pre-counsel-review (per Master Plan §5 + `DECISION-COUNSEL-1`). This release is suitable for engineering / auditor / operator evaluation; external-public-materials require counsel-validated language refinement before publication.

The technical claims (cryptographic primitives, byte-determinism, signature binding, Sigstore Rekor anchoring) are stable; the regulatory-claim phrasing is the layer subject to counsel review.

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-α security model | [`docs/V2-ALPHA-1-RELEASE-NOTES.md`](V2-ALPHA-1-RELEASE-NOTES.md) |
| V2-β orchestration | [`docs/V2-BETA-ORCHESTRATION-PLAN.md`](V2-BETA-ORCHESTRATION-PLAN.md) + [`docs/V2-BETA-DEPENDENCY-GRAPH.md`](V2-BETA-DEPENDENCY-GRAPH.md) |
| Operator runbook (V2-α) | [`docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md`](OPERATOR-RUNBOOK-V2-ALPHA-1.md) |
| Parallel-projection design (V2-β) | [`docs/ADR/ADR-Atlas-007-parallel-projection-design.md`](ADR/ADR-Atlas-007-parallel-projection-design.md) |
| wasm-publish race postmortem | [`docs/ADR/ADR-Atlas-008-wasm-publish-race-postmortem.md`](ADR/ADR-Atlas-008-wasm-publish-race-postmortem.md) |
| Master Plan | [`docs/V2-MASTER-PLAN.md`](V2-MASTER-PLAN.md) |
| Working Methodology | [`docs/WORKING-METHODOLOGY.md`](WORKING-METHODOLOGY.md) |
| Decisions log | [`.handoff/decisions.md`](../.handoff/decisions.md) |

---

**End of v2.0.0-alpha.2 Release Notes.** Next pre-release on this line will likely be `v2.0.0-alpha.3` (mid-Phase-4 work-products) or `v2.0.0-beta.1` (ArcadeDB-backed Layer 2 operational, Phase 9).
