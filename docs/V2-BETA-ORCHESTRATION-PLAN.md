# V2-β Wave Orchestration Plan

> **Status:** Phase-0 output, master-resident. **Effective 2026-05-13.**
>
> **Welle progress (updated 2026-05-13, Phase 7 consolidation commit):**
> - Phase 0 SHIPPED (PR #71) — this plan + Dependency Graph + welle plan-doc template
> - Phase 1 SHIPPED — W9 Operator Runbook (PR #72), W10 Parallel-Projection ADR-Atlas-007 (PR #73), W11 wasm-publish.yml race fix + ADR-Atlas-008 (PR #74)
> - Phase 2 SHIPPED — Phase-1-batch consolidation (PR #75)
> - Phase 3 SHIPPED — `v2.0.0-alpha.2` released (PR #76); signed tag + GitHub Release + npm publish validates W11 fix end-to-end
> - Phase 4 SHIPPED — W12 Read-API (PR #79), W13 MCP V2 tools (PR #77), W14 expanded event-kinds (PR #78). Three parallel subagents in isolated worktrees, 6 per-welle reviewer agents + 1 cross-batch consistency-reviewer. Three reviewer-driven fix-commits per branch. All 7 byte-determinism CI pins byte-identical post-merge.
> - Phase 5 SHIPPED — Phase-4-batch consolidation (PR #80)
> - Phase 6 SHIPPED — W15 Cypher-validator consolidation (PR #81). NEW `packages/atlas-cypher-validator/` shared monorepo package + ADR-Atlas-009. Two reviewer-driven hotfixes (tsc build step + workflow propagation).
> - Phase 7 SHIPPED — this consolidation commit (CHANGELOG + master-plan §6 + orchestration-plan welle-progress + handoff doc).
> - Phase 8 next — W16 ArcadeDB embedded-mode spike-doc.
> **Companion docs:** [`V2-BETA-DEPENDENCY-GRAPH.md`](V2-BETA-DEPENDENCY-GRAPH.md) (welle dependency edges + Mermaid diagram); [`../.handoff/v2-beta-welle-N-plan.md.template`](../.handoff/v2-beta-welle-N-plan.md.template) (per-welle plan-doc skeleton).
> **Methodology baseline:** [`WORKING-METHODOLOGY.md`](WORKING-METHODOLOGY.md) — Atlas's 4-phase iteration framework, proven by V2-α Welles 1-8.
> **Strategic context:** Atlas `v2.0.0-alpha.1` shipped 2026-05-13 (master commit `47b6894`; signed tag pushed; npm `@atlas-trust/verify-wasm@2.0.0-alpha.1` LIVE). V2-α delivered the cryptographic projection-state verification primitive end-to-end. V2-β scope per [`V2-MASTER-PLAN.md`](V2-MASTER-PLAN.md) §6.

This plan defines **how the next 11 wellen of V2-β work are orchestrated** to maximise parallel subagent dispatch in isolated worktrees without inter-welle blockage or merge-conflict thrash. The output is `v2.0.0-beta.1` after Welles 9-19 ship.

## 1. The framework — 10 phases, 11 wellen, 3 parallel batches

```
Phase 0  (this doc on master)
   │
   ▼
Phase 1  (PARALLEL BATCH 1: W9 + W10 + W11, docs-only)
   │
   ▼
Phase 2  (post-batch consolidation, parent agent)
   │
   ▼
Phase 3  (v2.0.0-alpha.2 ship — workflow hotfix promoted)
   │
   ▼
Phase 4  (PARALLEL BATCH 2: W12 + W13 + W14, code wellen)
   │
   ▼
Phase 5  (W15: Cypher-validator consolidation, rule-of-three post-merge)
   │
   ▼
Phase 6  (W16: ArcadeDB pre-flight spike-doc, analog V2-α Welle 2)
   │
   ▼
Phase 7  (W17a/b/c: ArcadeDB driver integration, 2-3 sessions serial)
   │
   ▼
Phase 8  (W18: Mem0g Layer-3 cache, depends on W17)
   │
   ▼
Phase 9  (W19: v2.0.0-beta.1 ship)
```

Per-welle pattern unchanged from V2-α: plan-doc → implementation → tests → parallel `code-reviewer` + `security-reviewer` → fix CRITICAL/HIGH in-commit → single SSH-signed commit → squash-merge with admin override.

## 2. Welle inventory + dispatch classification

| Welle | Title | Phase | Mode | Subagent type | Branch | File-area |
|---|---|---|---|---|---|---|
| W9 | Operator runbook v2.0.0-alpha.1 | 1 | PARALLEL | `doc-updater` | `feat/v2-beta/welle-9-operator-runbook` | `docs/` only |
| W10 | Parallel-projection design ADR | 1 | PARALLEL | `architect` | `feat/v2-beta/welle-10-parallel-projection-adr` | `docs/ADR/ADR-Atlas-007-*` |
| W11 | wasm-publish.yml dual-publish race fix | 1 | PARALLEL | `general-purpose` | `feat/v2-beta/welle-11-wasm-publish-fix` | `.github/workflows/` + `docs/` |
| W12 | Read-API endpoints (6 routes) | 4 | PARALLEL | `general-purpose` | `feat/v2-beta/welle-12-read-api` | `apps/atlas-web/src/app/api/atlas/` |
| W13 | MCP V2 tools (5 tools) | 4 | PARALLEL | `general-purpose` | `feat/v2-beta/welle-13-mcp-v2-tools` | `apps/atlas-mcp-server/src/tools/` |
| W14 | Expanded event-kind support | 4 | PARALLEL | `general-purpose` | `feat/v2-beta/welle-14-expanded-event-kinds` | `crates/atlas-projector/src/upsert.rs` |
| W15 | Cypher validator consolidation | 5 | SERIAL | `refactor-cleaner` | `feat/v2-beta/welle-15-cypher-validator-consolidation` | both web + mcp |
| W16 | ArcadeDB embedded-mode spike-doc | 6 | SERIAL | `architect` | `feat/v2-beta/welle-16-arcadedb-spike` | `docs/V2-BETA-ARCADEDB-SPIKE.md` |
| W17a | ArcadeDB planning + scaffold | 7 | SERIAL | `architect` | `feat/v2-beta/welle-17a-arcadedb-scaffold` | `crates/atlas-projector/src/backend/` |
| W17b | ArcadeDB driver implementation | 7 | SERIAL | `general-purpose` | `feat/v2-beta/welle-17b-arcadedb-impl` | same |
| W17c | ArcadeDB integration tests | 7 | SERIAL | `e2e-runner` | `feat/v2-beta/welle-17c-arcadedb-tests` | `crates/atlas-projector/tests/` |
| W18 | Mem0g Layer-3 cache | 8 | SERIAL | `architect` | `feat/v2-beta/welle-18-mem0g-cache` | new crate or `crates/atlas-projector/` |
| W19 | v2.0.0-beta.1 ship | 9 | SERIAL | `general-purpose` | `feat/v2-beta/welle-19-beta1-ship` | mechanical version bumps |

**Mode legend:** PARALLEL = dispatched as subagent-in-worktree alongside others in the same phase. SERIAL = exactly one agent at a time; subsequent wellen wait for completion.

## 3. Parallel-dispatch architecture

### 3.1 Subagent dispatch via `Agent` tool

```
parent_agent.Agent({
  description: "<welle-short-title>",
  subagent_type: "<role>",
  isolation: "worktree",         // CRITICAL — forks fresh worktree from master
  prompt: <self-contained subagent prompt; see .handoff/v2-beta-welle-N-plan.md.template>
})
```

The `isolation: "worktree"` flag creates `.claude/worktrees/agent-<hash>/` subdir with its own checkout of master. The subagent operates there, commits there, and (depending on prompt instructions) creates its own branch + PR.

### 3.2 Worktree fork-base lesson (V2-α inherited)

`Agent` tool with `isolation: "worktree"` **always forks from master regardless of parent's current branch.** Subagent prompts MUST include: *"`git fetch && git checkout master` as first action"* OR have all required content inlined in the prompt. Foundation-doc subagents (V2-α Phase 1) hit this; mitigation is now documented in the prompt template.

### 3.3 Anti-divergence enforcement (subagent prompt MUST forbid)

Touching any of these files in a parallel-batch welle is **forbidden**; parent agent consolidates them post-batch:

- `CHANGELOG.md`
- `docs/V2-MASTER-PLAN.md` (status table in §6)
- `docs/SEMVER-AUDIT-V1.0.md`
- `.handoff/decisions.md`
- `.handoff/v2-session-handoff.md`

These shared files are EXPECTED to be touched by every welle conceptually but their EDITS happen in a single parent-consolidation commit after the batch lands.

### 3.4 ADR-number pre-assignment

V2-β ADRs (Architecture Decision Records) reserved in advance to prevent parallel-subagent number-races:

| ADR # | Reserved for | Phase | Status |
|---|---|---|---|
| ADR-Atlas-007 | Parallel-projection design (W10) | 1 | **SHIPPED 2026-05-13** (PR #73) |
| ADR-Atlas-008 | wasm-publish.yml race postmortem (W11) | 1 | **SHIPPED 2026-05-13** (PR #74) |
| ADR-Atlas-009 | Cypher-validator consolidation rationale (W15) | 6 | **SHIPPED 2026-05-13** (PR #81) |
| ADR-Atlas-009 | Cypher-validator consolidation rationale (W15) | 5 | available |
| ADR-Atlas-010 | ArcadeDB backend choice + embedded-mode trade-off (W16) | 6 | available |
| ADR-Atlas-011 | ArcadeDB driver scaffold + trait design (W17a) | 7 | available |
| ADR-Atlas-012 | Mem0g cache invariants (W18) | 8 | available |
| ADR-Atlas-013 to ADR-Atlas-017 | Reserved for V2-γ/V2-δ welle ADRs | future | reserved |

Existing high-watermark: `ADR-Atlas-006-multi-issuer-sigstore-tracking.md`. V2-β starts at 007.

### 3.5 Cross-welle consistency-reviewer

After each parallel batch ends + per-welle reviewers complete, dispatch ONE additional `consistency-reviewer` agent reading all PRs in the batch together. Per the multi-perspective pattern in `~/.claude/rules/common/agents.md`. Catches cross-welle inconsistencies (e.g. W9 runbook contradicting W10 design) that per-welle reviewers cannot see.

## 4. Convergence criteria

### 4.1 Per-welle gate

- All 7 byte-determinism CI pins byte-identical after merge
- Parallel `code-reviewer` + `security-reviewer` dispatched; CRITICAL = 0, HIGH fixed in-commit
- Plan-doc on welle's own branch (`.handoff/v2-beta-welle-N-plan.md`)
- SSH-Ed25519 signed commit (`SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- Squash-merge via admin override on master

### 4.2 Per-parallel-batch gate

- All wellen in batch merged
- Parent consolidation commit on master updates: `CHANGELOG.md`, `docs/V2-MASTER-PLAN.md` §6 status table, `docs/SEMVER-AUDIT-V1.0.md` §10 additions, `.handoff/v2-session-handoff.md` phase-tracker
- Workspace test green; all 7 byte-determinism pins byte-identical

### 4.3 v2.0.0-alpha.2 ship gate (Phase 3)

- W9 + W10 + W11 all merged
- W11 fix has eliminated dual-publish race in `wasm-publish.yml` log
- Workspace version bumped `2.0.0-alpha.1` → `2.0.0-alpha.2`
- Signed tag + GitHub Release + npm publish landed clean

### 4.4 v2.0.0-beta.1 ship gate (Phase 9)

- W12 + W13 + W14 + W15 + W16 + W17a + W17b + W17c + W18 all merged
- ArcadeDB-backed Layer 2 demonstrably operational (Welle 6 gate passes against ArcadeDB-projected state)
- All 7 byte-determinism pins byte-identical
- `docs/V2-BETA-1-RELEASE-NOTES.md` comprehensive
- Counsel-track has signed-off on agent-DID revocation channel + Rekor-anchor data-residency stance (per `DECISION-COUNSEL-1`) — **THIS IS A NELSON-LED GATE, NOT ENGINEERING-DISPATCHABLE**

## 5. Critical invariants (non-negotiable)

1. **Phase 0 plan-doc on master is HARD GATE.** No subagent dispatch before this PR merges. Subagents fork their worktrees from master; if the orchestration plan isn't ON master, parallel work diverges from a shared methodology.
2. **Anti-divergence file-list (§3.3) enforced in every subagent prompt.** Parent does ALL consolidation commits.
3. **ADR numbers (§3.4) pre-assigned per welle.** No subagent invents an ADR number.
4. **Reviewer-dispatch per welle is non-negotiable.** V2-α showed reviewers catch 2-5 HIGH findings per welle; skipping = quality regression.
5. **Cross-welle consistency-reviewer (§3.5) after each parallel batch.**
6. **Worktrees fork from master.** Subagent prompts include `git fetch && git checkout master` as first action OR have content inlined.
7. **All 7 byte-determinism CI pins** verified byte-identical after EVERY welle merge.
8. **SSH-Ed25519 signed commits + admin-override squash-merge.** Per Atlas standing protocol.

## 6. Already-shipped capabilities (do NOT re-implement in V2-β)

The plan-agent stress-test surfaced false-positive welle candidates. These are confirmed already-shipped:

- **`ProjectorRunAttestation` event-kind wiring into Layer 1** — Welles 4 + 5 + 7 (verifier-side parser; emission pipeline; atlas-signer CLI signing into events.jsonl)
- **Cryptographic projection-state verification end-to-end** — Welles 3 + 4 + 5 + 6 + 7 (canonicalisation byte-pin; attestation event-schema; emission pipeline; CI gate; producer CLI)
- **`author_did` Welle 1 schema-additive field** — bound into signing input per Phase 2 Security H-1

## 7. Deferred to later iteration phases (V2-γ / V2-δ)

- **Agent-DID enforcement at Read-API / MCP V2 read-access level** — V2-γ (Agent Passports). V2-β plumbs `author_did` through signing input (V2-α Welle 1, shipped) but does NOT gate read-access by DID.
- **Regulator-Witness Federation (M-of-N threshold enrolment)** — V2-γ per `DECISION-SEC-3`
- **Hermes-skill v1** — V2-γ (credibility-asset GTM positioning per `DECISION-BIZ-1`)
- **Cedar policy at write-time** — V2-δ
- **Post-quantum hybrid Ed25519+ML-DSA-65 co-sign** — V2-δ

## 8. Counsel-track (parallel, Nelson-led, NOT engineering-pipeline)

Per `docs/V2-MASTER-PLAN.md` §5 + `DECISION-COUNSEL-1`: €30-80K counsel engagement, 6-8 weeks structured. **Pre-V2-α-public-materials blocking** for EU customers with PII workspaces. Scope per Master Vision §11 (7 items: GDPR Path A/B, AILD→PLD reframe, Art. 43 disclaimer, Schrems II, Art. 12 marketing copy, witness-federation positioning, DPIA/FRIA templates).

V2-β ship-gate (Phase 9) requires counsel sign-off on agent-DID revocation channel + Rekor-anchor data-residency stance. This is a Nelson-led ADD to the convergence-criteria list, not an engineering-dispatch.

---

## What comes next (Phase 1 readiness)

After this Phase-0 plan + dependency graph + welle template + ADR reservation merge to master, the parent agent is cleared to dispatch Phase 1's 3 parallel subagents (W9 + W10 + W11). Each subagent prompt follows the template in [`../.handoff/v2-beta-welle-N-plan.md.template`](../.handoff/v2-beta-welle-N-plan.md.template). Each subagent produces ONE draft PR. Parent dispatches reviewers per PR + 1 cross-batch consistency-reviewer at the end + merges in sequence + writes the consolidation commit.
