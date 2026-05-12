# V2-α Welle 2 — Plan-Doc (ArcadeDB vs FalkorDB Comparative Spike)

> **Status: DRAFT 2026-05-12.** Awaiting Nelson's confirmation before merge. Becomes SHIPPED block in `.handoff/v2-session-handoff.md` once merged.
> **Master Plan reference:** `docs/V2-MASTER-PLAN.md` §6 V2-α Foundation. This is session 2 of 5–8.
> **Driving decision:** `DECISION-DB-1` — Kuzu fallback dead (Apple-acquired Oct-2025); ArcadeDB Apache-2.0 is the candidate next-viable Apache-2.0 graph-DB fallback to FalkorDB SSPLv1. Risk matrix R-L-02 marks this spike as **pre-V2-α-lock blocking**.

Welle 2 delivers a structured research-only comparative spike between FalkorDB (V2-α primary candidate, SSPLv1) and ArcadeDB (Apache-2.0 fallback candidate). Output is a single master-resident decision document (`docs/V2-ALPHA-DB-SPIKE.md`) with explicit recommendation, confidence level, and reversal-cost analysis. **No production code, no benchmark harness implementation** — those are deferred to Welle 3+ once the strategic DB choice is locked.

**Why this as Welle 2** (rather than Projector skeleton or CLI flag):
- Welle 3 (Projector skeleton) design depends on DB-specific schema patterns (FalkorDB Cypher subset semantics differ from ArcadeDB's). Locking DB choice first prevents Projector rework.
- Pre-V2-α-lock blocking item per Risk Matrix; deferring it past Welle 3 would mean projector code commits before DB is chosen.
- HIGH reversibility (research doc only).
- Independent of Welle 1's Agent-DID work — orthogonal scope.

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| NEW `docs/V2-ALPHA-DB-SPIKE.md` — comprehensive comparative analysis with explicit recommendation + confidence | Actual benchmark harness implementation (deferred to post-DB-lock) |
| License analysis (SSPLv1 vs Apache-2.0) and hosted-service implications | DB installation / setup tutorials (operator-runbook scope) |
| Feature parity for Atlas projection use-case (idempotent upsert, multi-tenant isolation, schema determinism) | Reference / read-API design (V2-β scope) |
| Performance characteristics from public sources + reasoning about Atlas-specific workload | Mem0g integration plan (V2-β scope) |
| Atlas-specific suitability (projection determinism, author_did stamping, ProjectorRunAttestation event-emission hooks) | ProjectorRunAttestation event schema design (Welle 3 candidate) |
| Operational considerations (deployment, embedded vs server, backup, observability) | Cedar policy gate (V2-δ scope) |
| Maturity + vendor risk assessment | Counsel-validated license opinion (Nelson-led counsel-engagement track) |
| Clear GO/NO-GO recommendation with confidence level + reversal-cost | Welle 3 Projector implementation |
| Update `docs/V2-MASTER-PLAN.md` if recommendation changes strategic direction (currently FalkorDB primary, ArcadeDB fallback) | atlas-signer CLI `--author-did` flag (separate trivial follow-up welle) |
| Update `CHANGELOG.md [Unreleased]` with Welle-2 entry | New crate or module structure |
| Plan-doc for Welle 2 | Pre-counsel commitment to specific DB |

---

## Decisions (final, pending Nelson confirmation)

- **Spike methodology: public-knowledge-based research, no actual benchmarks executed.** Running real benchmarks requires installing both DBs, generating workload, and validating performance. That's a separate effort (~1-2 sessions) potentially candidate for "Welle 2b" if the public-knowledge analysis surfaces a tie or a HIGH-uncertainty conclusion.
- **Recommendation MUST include confidence level.** "FalkorDB primary" with LOW confidence is significantly different from "FalkorDB primary" with HIGH confidence. The spike doc commits to a confidence assertion AND documents what additional evidence would raise/lower it.
- **No DB choice locked by this welle.** The spike's recommendation feeds into a Nelson-decision (the V2-α DB lock). Welle 3 (Projector skeleton) operates on the locked choice.
- **If the spike recommends flipping (ArcadeDB primary, FalkorDB fallback)** — update `docs/V2-MASTER-PLAN.md` §3 + §10.4 + risk-matrix R-L-02 entries to reflect. Master-Vision and decisions.md log the rationale.
- **License analysis includes "what if SSPL becomes pure-AGPL or pure-commercial" scenarios** — both DBs might pivot license, and the spike documents how Atlas's open-core monetization model survives each scenario.

---

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `docs/V2-ALPHA-DB-SPIKE.md` | Comparative analysis: License · Features · Performance · Operations · Atlas-Specific Suitability · Recommendation · Open Questions · References. ~400-500 lines. Becomes master-resident V2-α DB decision source-of-truth. |
| NEW | `.handoff/v2-alpha-welle-2-plan.md` | This plan-doc itself. |
| MODIFY | `docs/V2-MASTER-PLAN.md` | If recommendation flips primary/fallback: update §3 (Three-Layer Trust Architecture) + §4 (Risk Matrix R-L-02 reference) + §6 (V2-α Foundation dependencies) + §11 (Reference Pointers — add V2-ALPHA-DB-SPIKE.md row). If recommendation confirms current direction: small update to §11 reference pointer table only. |
| MODIFY | `CHANGELOG.md` | `[Unreleased]` gets `### Added — V2-α Welle 2` block (research deliverable). |
| MODIFY (post-merge) | `.handoff/v2-session-handoff.md` | Welle-2-SHIPPED block. |

**Total estimated diff:** ~500-700 lines (spike-doc dominant).

---

## Spike-doc structure (target outline)

```
# V2-α DB Choice — ArcadeDB vs FalkorDB Comparative Spike

## 0. Executive Summary
- Recommendation + Confidence
- Decision-reversibility cost

## 1. Why This Spike (Risk-Matrix Context)
- Master Plan §4 R-L-02 reference
- DECISION-DB-1 lineage (Kuzu archived → ArcadeDB candidate)

## 2. License Analysis
2.1 FalkorDB SSPLv1
2.2 ArcadeDB Apache-2.0
2.3 Hosted-Service Implications for Atlas Open-Core Model
2.4 License-Pivot Scenarios

## 3. Feature Parity for Atlas Use-Case
3.1 Cypher Subset Coverage
3.2 Property Graph Model
3.3 Idempotent Upsert Pattern (Projection requirement)
3.4 Multi-Tenant Isolation
3.5 Schema Determinism

## 4. Performance Characteristics
4.1 FalkorDB GraphBLAS Backend
4.2 ArcadeDB Bucket Architecture
4.3 Atlas-Workload Estimates (write-heavy projection vs read-heavy query)

## 5. Operational Considerations
5.1 Deployment
5.2 Embedded vs Server Mode
5.3 Backup / Recovery / Determinism Verification
5.4 Observability

## 6. Maturity + Vendor Risk
6.1 FalkorDB Status (Redis ownership, commercial backing, community)
6.2 ArcadeDB Status (Costa Group, OrientDB heritage, community)
6.3 Acquisition / Pivot Risk Assessment

## 7. Atlas-Specific Decision Factors
7.1 Projection Determinism Compatibility (canonicalisation byte-pin)
7.2 author_did Property-Stamping (Welle 1 dependency)
7.3 ProjectorRunAttestation Event Hooks
7.4 V2-β Mem0g Cache Integration
7.5 V2-γ Federation-Witness Property-Visibility

## 8. Recommendation
8.1 Primary Choice + Confidence Level (HIGH/MEDIUM/LOW)
8.2 Fallback Path
8.3 Decision-Reversibility Cost Analysis
8.4 What Additional Evidence Would Raise/Lower Confidence

## 9. Open Questions for V2-α Welle 3 (and Future Wellen)

## 10. Reference Pointers
```

---

## Acceptance criteria

- [ ] `docs/V2-ALPHA-DB-SPIKE.md` exists with all 10 sections per outline
- [ ] License analysis names specific clauses of SSPL §13 (the contentious one) and Apache-2.0 §4 + §5
- [ ] Recommendation includes explicit HIGH/MEDIUM/LOW confidence assertion
- [ ] Reversibility-cost analysis quantifies "what changes if we picked wrong"
- [ ] All claims about either DB are sourced (URL or specific public artefact)
- [ ] Atlas-specific section addresses Welle-1's author_did stamping in graph nodes
- [ ] Master Plan + decisions.md are updated if recommendation flips primary/fallback
- [ ] `CHANGELOG.md [Unreleased]` has the entry
- [ ] Parallel code-reviewer + security-reviewer agents dispatched (yes, even on a research doc — catches factual drift + license-claim-accuracy)
- [ ] CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-signed commit, draft-then-ready PR
- [ ] Self-merge via `gh pr merge --squash --admin --delete-branch` (established Atlas pattern)
- [ ] `.handoff/v2-session-handoff.md` Welle-2-SHIPPED block added post-merge

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| **Public-knowledge analysis is insufficient to make a confident call** | MEDIUM | MEDIUM (forces a Welle 2b actual-benchmark spike) | Spike doc must mark confidence honestly. If LOW, recommendation = "do Welle 2b before V2-α DB lock" |
| **License analysis reaches wrong conclusion** | LOW | HIGH (commercial-license obligation surprise post-launch) | Counsel-validated license opinion is on Nelson's parallel track; spike's analysis is engineer-perspective only and explicitly flags counsel review as required pre-V2-α public materials |
| **One of the two DBs pivots license or gets acquired during V2-α** | MEDIUM | MEDIUM (forces Welle 2c re-analysis) | Maturity + vendor-risk section addresses; recommendation includes "monitor X every Y" cadence |
| **Spike recommends FLIPPING primary/fallback** | MEDIUM | LOW (just a doc update across master-plan + decisions.md) | Workflow accommodates: if flip warranted, the same commit updates strategic docs to reflect |
| **Cypher subset divergence between DBs surfaces as a real Projector blocker post-Welle-3** | LOW | MEDIUM (rework Projector to abstract over DB) | Spike §3.1 documents minimum Cypher subset Atlas requires; Welle 3 designs Projector to use only that subset |

---

## Out-of-scope this welle (later V2-α wellen)

- **V2-α Welle 3 candidates:** Atlas Projector skeleton (depends on Welle 2 DB lock); ProjectorRunAttestation event schema; projector-state-hash CI gate.
- **V2-α Welle 4–8 candidates:** content-hash separation (counsel-gated), Mem0g cache integration (V2-β), Read-API endpoints (V2-β), MCP V2 tools (V2-β), Cedar policy gate (V2-δ), Agent Passport endpoint (V2-γ), Federation enrolment (V2-γ).
- **Atlas-signer CLI `--author-did` flag** — trivial follow-up Welle. Separate scope.
- **Actual benchmark harness execution** — potential Welle 2b if confidence is LOW.

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| Master Plan | `docs/V2-MASTER-PLAN.md` §3 Three-Layer Trust Architecture, §4 R-L-02, §6 V2-α Foundation, §10 Success Criteria |
| Master Vision DB context | `.handoff/v2-master-vision-v1.md` §5 Three-Layer Architecture (Layer 2 FalkorDB projection), §7.2 Graph DB landscape |
| `DECISION-DB-1` (Kuzu archived → ArcadeDB candidate) | `.handoff/decisions.md` |
| Risk Matrix R-L-02 (FalkorDB SSPL exposure) | `docs/V2-MASTER-PLAN.md` §4 + Master Vision §6 |
| Working Methodology | `docs/WORKING-METHODOLOGY.md` §"Welle Decomposition Pattern" |
| V2-α Welle 1 (Agent-DID schema, prerequisite for projector graph-node stamping) | `crates/atlas-trust-core/src/agent_did.rs`, `.handoff/v2-alpha-welle-1-plan.md` |

---

**End of Welle 2 Plan.** Spike-doc (`docs/V2-ALPHA-DB-SPIKE.md`) is the welle's primary deliverable. Plan-doc + spike-doc + targeted strategic-doc updates ship together in one coherent SSH-signed commit.
