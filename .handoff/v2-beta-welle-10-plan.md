# V2-β Welle 10 — Parallel-Projection Design ADR

> **Status:** DRAFT 2026-05-13. Awaiting parent agent's confirmation before merge.
> **Orchestration:** part of Phase 1 (parallel batch W9/W10/W11) per `docs/V2-BETA-ORCHESTRATION-PLAN.md`.
> **Driving decisions:** `DECISION-ARCH-1` (Triple-hardening: canonicalisation byte-pin ✓ + ProjectorRunAttestation ✓ + parallel-projection ← THIS leg), `DECISION-SEC-2` (Projection determinism). Pre-assigned ADR number: `ADR-Atlas-007`.

This welle produces an Architecture Decision Record (ADR-Atlas-007) that **designs** (does NOT implement) parallel/streaming projection to scale beyond V2-α's single-threaded projector (8.3h rebuild at 100M events per `docs/V2-MASTER-PLAN.md` §4 risk R-A-01). The ADR prioritizes **preservation of byte-determinism** (Welle 3's CI pin invariant) and completion before Welle 17 (ArcadeDB driver) locks the backend's concurrent-write semantics.

**Why this as Welle 10:** W9+W10+W11 are Phase 1 docs-only welles that unblock Phase 2 consolidation + Phase 3 alpha.2 ship. W10 specifically bridges V2-α's completed canonicalisation + ProjectorRunAttestation work with V2-β's parallel-projection requirements. No code touches; pure ADR design.

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| **ADR-Atlas-007 design document** (~250–400 lines) covering: context (8.3h rebuild problem), decision drivers (byte-determinism + scale + ArcadeDB compatibility), three architectural options (workspace-parallel / entity-shard / batch-streaming), trade-off analysis, recommendation + confidence, open questions for W17 | V2-β Phase-2 forbidden files: CHANGELOG.md, V2-MASTER-PLAN.md status table, SEMVER-AUDIT-V1.0.md, decisions.md, v2-session-handoff.md (parent consolidates) |
| **Welle 10 plan-doc** (this file, `.handoff/v2-beta-welle-10-plan.md`) | Implementation code (zero code touches; W10 is design-only) |
| Cross-references to Welles 3+4+5+6 (canonicalisation + ProjectorRunAttestation + projector state-hash CI gate + projector schema version) | Full parallel-projection implementation (candidate for post-W18 scope, not W10) |
| Honest confidence levels (HIGH/MEDIUM/LOW) on each option's feasibility + reversibility analysis | Operator-tunable worker-count configuration (operational, deferred to V2-β-Welle-N candidate) |
| | Incremental-projection semantics (orthogonal scope; separate ADR pre-W20 if needed) |

**Total estimated diff:** ~400–500 lines (ADR only; this plan-doc ~150 lines).

---

## Decisions (final, pending parent confirmation)

- **ADR format:** Context → Decision Drivers → Considered Options (3 options: A/B/C) → Decision (recommend ONE with confidence) → Consequences → Open Questions (for W17 ArcadeDB spike)
- **Byte-determinism preservation:** parallel design MUST guarantee per-welle-2 canonical sort-order post-merge invariant; no silent non-determinism
- **ArcadeDB compatibility:** design must not presuppose any concurrent-write guarantees beyond what ArcadeDB's embedded-mode HTTP API provides; W17 spike validates
- **Recommendation strategy:** pick the option with highest confidence for 10M–100M event range on 16-core host; explicitly state reversibility (can we switch options post-W18 if operational data contradicts projections)

---

## Files

| Status | Path | Content |
|---|---|---|
| NEW | `docs/ADR/ADR-Atlas-007-parallel-projection-design.md` | Architecture Decision Record (~250–400 lines); Welle 10 deliverable |
| NEW | `.handoff/v2-beta-welle-10-plan.md` | This welle's plan-doc |

**Total estimated diff:** ~400–500 lines.

---

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 10 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) | NONE — this welle is design-only; zero code changes | Design must preserve canonicalisation sort-order invariant; ADR explicitly calls out options that would break it |
| `projector-state-hash` pin (Welle 3, canonicalisation byte-pin) | NONE — ADR discusses parallelism *after* canonical form established | Three options all defer to post-merge canonicalisation; no parallel-isation of the canonicalisation step itself |
| `ProjectorRunAttestation` schema (Welle 4, events.jsonl integration) | NONE — ADR does not modify event kinds | But ADR must flag if any option's merge strategy affects `ProjectorRunAttestation` attestation timing |

**Mandatory check:** all 7 byte-determinism CI pins MUST remain byte-identical after this welle (which is zero code, so automatic). Future implementation welles (V2-β candidates) will be gated by the same test suite.

---

## Implementation steps (TDD order; N/A — design-only)

```
1. Write ADR with all three options fully articulated (no code)
2. Ensure cross-references to Welles 3+4+5+6 correct + verifiable
3. Open questions section must list concrete W17 ArcadeDB spike questions
4. Run `cargo check --workspace` (sanity — workspace state unchanged)
5. Review ADR for tone consistency with ADR-006 (reference ADR)
6. Dispatch parallel `code-reviewer` + `security-reviewer` agents
7. Fix CRITICAL/HIGH findings (if any reference-invalidation or logical holes)
8. SSH-signed single coherent commit
9. Push + open DRAFT PR with base=master
10. Parent agent dispatches consistency-reviewer + decides merge timing
```

---

## Acceptance criteria

- [ ] `cargo check --workspace` green (no code changes, workspace state unchanged)
- [ ] ADR-Atlas-007 covers Context, Decision Drivers, ≥3 Considered Options, Decision, Consequences, Open Questions (minimum ADR sections)
- [ ] Three options (workspace-parallel / entity-shard / batch-streaming) each have trade-off table (effort / bytedeterminism-risk / ArcadeDB-compatibility / reversibility)
- [ ] Recommendation includes confidence level (HIGH/MEDIUM/LOW) + reversibility analysis
- [ ] Open questions section lists ≥3 concrete W17 ArcadeDB spike items (e.g. "does ArcadeDB support concurrent-writer batching?")
- [ ] Cross-references to Welles 3+4+5+6 verifiable (paths exist, quotes accurate)
- [ ] Cross-references to DECISION-ARCH-1 + DECISION-DB-4 present + correct
- [ ] Tone consistency with ADR-006 (reference ADR for style)
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched; CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit (`SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [ ] DRAFT PR open with base=master
- [ ] Forbidden-files rule honoured (no touches to CHANGELOG.md, V2-MASTER-PLAN.md status, decisions.md, semver-audit, handoff doc)

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| ADR recommends option that W17 ArcadeDB spike invalidates | MEDIUM | MEDIUM | Open questions section must flag validation gates for each option; W16 spike (pre-W17) can do preliminary ArcadeDB compatibility check if time permits |
| Recommendation confidence is LOW across all 3 options → parent defers ADR to post-W17 | LOW | LOW | If architect working the ADR finds all options equally fraught, recommend waiting for W17 spike data; escalate to parent for timeline decision |
| Canonical sort-order invariant is subtle; option A/B/C descriptions accidentally imply non-determinism | MEDIUM | CRITICAL | Mitigated by strict peer review against Welle 3 canonical.rs code; security-reviewer MUST validate canonical-order preservation |
| Cross-references to future Welles 17+ create forward-dependency debt | LOW | LOW | Mitigated by opening explicit follow-on ADR (ADR-Atlas-011) for ArcadeDB driver design (W17a); W17 ADR can revise W10 open questions post-spike |

---

## Out-of-scope this welle (later phases)

- **V2-β Welles 12–15:** Read-API + MCP V2 tools + Cypher consolidation (depends on projector design but not on parallelisation strategy)
- **V2-β Welle 16:** ArcadeDB spike (pre-W17); can provide preliminary ArcadeDB concurrency validation for W10 open questions
- **V2-β Welles 17a/b/c:** ArcadeDB driver integration (depends on W10 design; W17a opens ADR-Atlas-011 with parallel-projection implementation plan)
- **V2-β Welle 18:** Mem0g cache (depends on projector being stable, not on parallelisation strategy)

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` |
| Projection Determinism risk (R-A-01) | `docs/V2-MASTER-PLAN.md` §4 + `.handoff/v2-master-vision-v1.md` §6 |
| DECISION-ARCH-1 triple-hardening | `.handoff/decisions.md` |
| Welle 3 canonicalisation design | `crates/atlas-projector/src/canonical.rs` + doc comments |
| Welle 4 ProjectorRunAttestation | (future; referenced conceptually in DECISION-ARCH-1) |
| Welle 5 upsert pattern (single-threaded) | `crates/atlas-projector/src/upsert.rs` |
| Welle 6 CI gate (`projector-state-hash` pin) | `crates/atlas-projector/tests/graph_state_hash_byte_determinism_pin` (inferred from canonical.rs doc comments) |
| V2-α Database spike (ArcadeDB choice) | `docs/V2-MASTER-PLAN.md` §4, DECISION-DB-1, DECISION-DB-4 |
| ADR-006 (reference ADR for style) | `docs/ADR/ADR-Atlas-006-multi-issuer-sigstore-tracking.md` |

---

## Implementation Notes (to be filled AFTER ADR review)

### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| ADR-Atlas-007 | `docs/ADR/ADR-Atlas-007-parallel-projection-design.md` | ~300–400 |

### Test outcome

- Welle 10 is design-only; no tests added.
- All 7 byte-determinism CI pins unchanged (cargo check --workspace green).
- No V1 + V2-α surface impact.

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| Canonical sort-order invariant subtle; option descriptions could imply non-determinism | Code reviewers + security reviewers validated against Welle 3 canonical.rs |
| W17 ArcadeDB spike invalidates recommendation | ADR open-questions section queued W17 validation gates |

### Deviations from plan

None of substance.
