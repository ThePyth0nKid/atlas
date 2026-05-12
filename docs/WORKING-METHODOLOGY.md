# Atlas Working Methodology — 4-Phase Strategic Iteration

> **Status:** v1, 2026-05-12. **Origin:** developed and stress-tested during Atlas V2 strategic planning (2026-05-12). The V2 outputs — `docs/V2-MASTER-PLAN.md` + `.handoff/v2-master-vision-v1.md` + `.handoff/decisions.md` — are this methodology's first full application.
> **When to use:** any *Großthema* requiring multi-session, multi-perspective planning before code is committed. Examples: V2 architecture pivot (this run), post-quantum migration (planned), GDPR-rearchitecture (contingent), V3 architecture (future).
> **When NOT to use:** single-Welle features, bug fixes, refactors — `.handoff/v*-welle-*-plan.md` lightweight planning is sufficient.

---

## Why this exists

Strategic planning collapses under two failure modes: (a) **premature commitment** — one perspective writes a "vision doc" that locks the team into one frame before alternatives are explored, and (b) **infinite deliberation** — no convergence criterion, so the plan never ships and never gets stress-tested.

The 4-phase pattern addresses both by structurally separating *generation* (Phase 1, broad and divergent) from *critique* (Phase 2, adversarial), from *synthesis* (Phase 3, semi-manual convergence with explicit decision-log), from *consolidation* (Phase 4, the only phase that touches master).

---

## Phase 1 — Vision-First Foundation Documents

**Goal:** broad, divergent generation of 4–6 foundation documents covering the strategic landscape from different angles.

**Pattern:**
- Dispatch 4–6 parallel subagents, each in its own git worktree on its own branch
- Each subagent writes exactly ONE foundation doc to its assigned file path — zero file-overlap → zero merge conflicts
- Each doc has an explicit "Open Questions for Phase 2 Critique" section (≥10 questions per doc)
- Subagent prompts include: pre-read order, structural section template, must-cover items, output file path, "do NOT commit, do NOT push" instruction
- Total content target: 2000–3000 lines across all docs

**Subagent selection by doc:** match `subagent_type` to doc role — strategic-positioning / general-purpose, technical-architecture / `architect`, risk-matrix / `security-reviewer`, competitive-landscape / general-purpose with WebSearch, demo-sketches / general-purpose.

**Cross-doc inconsistency is expected and NOT a Phase-1 convergence criterion.** Discrepancies are resolved in Phase 3.

**Convergence criterion:** all docs delivered, each ≥400 lines, each with substantial "Open Questions" section.

**Integration:** merge each subagent's branch into one integration branch (e.g. `v2/phase-1-foundation`). PR opened **as draft, no-merge** — this PR is the Phase-2 critique target, not a master-merge target.

**Anti-pattern:** spending Phase 1 trying to resolve cross-doc inconsistency. That belongs to Phase 3. Phase 1 is divergent generation.

---

## Phase 2 — Multi-Angle Critique

**Goal:** 6 parallel structured critiques from independent perspectives, surfacing findings the original docs missed.

**Pattern:**
- 6 parallel subagents in own worktrees, each producing one critique file (e.g. `.handoff/crit-<role>.md`)
- Each critique uses a standardised template:
  ```
  ## Stärken (what is good, should stay)
  ## Probleme (what must be addressed, by severity: CRITICAL/HIGH/MEDIUM/LOW)
  ## Blinde Flecken (what the docs miss entirely)
  ## Konkrete Vorschläge (specific edits, doc-section-tagged)
  ## Offene Fragen für Phase 3
  ```
- Standard 6 critique roles: architect, security-reviewer, database/performance, product/UX, compliance/regulatory, business/investor — adjust based on project domain

**Convergence criterion:** each critique ≥5 structural points + ≥3 concrete edit-proposals. Critiques MUST address, not just acknowledge.

**Worktree fork-base lesson:** `Agent` tool with `isolation: "worktree"` forks from master regardless of parent's current branch. To make Phase-1 docs visible to Phase-2 agents, either (a) instruct subagents to `git fetch && git checkout <phase-1-branch>` as first action, OR (b) pass critical content inline via prompt. Inline-load + reset-pre-commit is a safe fallback.

**Integration:** merge all 6 critique branches into one integration branch (e.g. `v2/phase-2-critiques`), base = Phase-1 integration branch. Stacked PR, draft, no-merge. Phase-3 synthesis-target.

**Anti-pattern:** dispatching critique-agents that produce "looks good" reviews. The agent prompt must require findings. Critique severity should be quantified (CRITICAL/HIGH/MEDIUM/LOW); a critique with zero CRITICALs and zero HIGHs is suspect.

---

## Phase 3 — Synthesis & Convergence

**Goal:** produce one consolidated coherent vision doc + explicit decision-log.

**Pattern:**
- **Semi-manual**, NOT parallel-subagent-dispatchable. Decision-making is human + Claude jointly.
- Read all Phase-2 critiques against the Phase-1 docs they target. Build a flat list of every CRITICAL + HIGH finding.
- For each finding, decide: **ACCEPT** (integrate directly) / **MODIFY-as-follows** (accept with documented modification) / **DEFER to <stage>** (legitimate concern but post-current-phase) / **REJECT** (with rationale).
- Output two artefacts:
  1. **Master Vision v1** — single consolidated doc replacing Phase-1 docs as operational source-of-truth (Phase-1 docs become read-only historical references)
  2. **decisions.md** — explicit log with ≥10 entries (each: topic / crit source / Phase-1 doc affected / recommendation / decision / rationale / reversibility HIGH/MED/LOW / review-after trigger)
- Cross-crit reconciliation: when multiple critiques surface the same weak spot from different angles (e.g., Atlas projection-determinism flagged by architect + security + database), **combine mitigations** rather than picking one. Multi-angle weak spots usually need multi-angle hardening.

**Convergence criterion:** Master Vision exists; all CRITICAL and HIGH crit-points addressed (accepted, modified, deferred, or explicitly rejected with rationale); decisions.md ≥10 entries.

**Integration:** Phase-3 outputs commit to integration branch (e.g. `v2/phase-3-master-vision`), base = Phase-2 integration branch. Stacked PR, draft, no-merge. Phase-4-derivation-target.

**Anti-pattern:** accepting every crit-point without trade-off analysis. Phase-2 critique surfaces concerns; Phase-3 synthesis decides what's actionable now vs. what's deferred. A doc that incorporates every critique point literally becomes incoherent — the critic's job is to surface; the synthesiser's job is to decide.

---

## Phase 4 — Plan Documentation

**Goal:** distill Phase-3 outputs into production-ready docs that land on master.

**Pattern:**
- **This is the ONLY phase that touches master.** Phases 1–3 produce work-in-progress on draft PRs that intentionally never merge.
- Two outputs:
  1. **Master Plan** (`docs/V2-MASTER-PLAN.md` or equivalent, ~300 lines) — distilled Master Vision with Welle decomposition tied to concrete PR-Wellen, success criteria, and reference pointers
  2. **Working Methodology** (this doc, `docs/WORKING-METHODOLOGY.md`, ~200 lines) — reusable pattern. Updated only when the methodology itself evolves (not per-Großthema)
- Master Plan structure: Vision · Positioning · Architecture · Top-5 Blocking Risks · Counsel/External-Engagement Plan · Welle Decomposition · Demo Programme · Competitive Position · GTM · Success Criteria · References
- Per Atlas standing protocol: implement → parallel `code-reviewer` + `security-reviewer` agents (yes, even for docs — they catch claim-drift between Master Vision and Master Plan) → fix CRITICAL/HIGH in-commit → SSH-signed commit → standard PR to master

**Convergence criterion:** both docs reviewed by Atlas-team-lead, merged to master via SSH-signed PR, `CHANGELOG.md [Unreleased]` reflects the merge with a clear narrative entry, any parallel-track operational items (e.g. counsel engagement kickoff) either started OR explicitly documented as deferred to <stage> in decisions.md.

**Anti-pattern:** writing a Master Plan that's effectively a copy of Master Vision. The Master Plan must be a *distillation*: shorter (~50% of Master Vision), with operational structure (Welle decomposition, success criteria), and master-resident-stable (references that don't decay).

---

## Welle Decomposition Pattern

Each strategic phase (V2-α / V2-β / V2-γ / V2-δ in the V2 case) is described in the Master Plan with:
- **Scope** — concrete deliverables, tied to specific Atlas crates / files
- **Dependencies** — serial vs parallel relationships with other Wellen and external tracks (counsel, supervisor-engagement)
- **Blocking risks** — Top-5-style, referenced to risk-matrix IDs (e.g. R-A-01)
- **Success criteria** — measurable, CI-enforceable where possible
- **Expected PR count** — calibrated estimate (Phase-2 critique typically re-baselines Phase-1 estimates 1.5–2× larger; build that in)

Atlas's V1.0–V1.19 history establishes the Welle calibration:
- Single Welle = 1 session (PR landing same-day or next-day)
- Single Welle PR count = 1 (occasional 2–3 with fixup commits)
- Welle scope = 200–800 lines of code + tests + docs in the commit, NOT counting strategic planning

Phase-2 architectural critique is the strongest tool for re-baselining Welle estimates because it surfaces concrete blocker items that Phase-1 vision-docs typically gloss over.

---

## Decision Log Discipline

`.handoff/decisions.md` (per Phase-3, one log per strategic-iteration cycle):
- Every entry dated YYYY-MM-DD
- Every entry tagged with stable `DECISION-<DOMAIN>-<N>` ID for cross-referencing from Master Vision / Master Plan / future decisions
- Reversibility: HIGH (text-only, easy revert), MEDIUM (schema or design choice, costly revert), LOW (foundational, near-irreversible post-ship)
- Review-after: explicit trigger (date OR event, e.g. "counsel opinion delivered" / "V2-α launch + 30 days")
- Cross-link liberally: when Master Vision §X references a decision, use the `[DECISION-ID]` syntax

**Decision log lives forever.** When a decision is reversed or superseded, ADD a new entry referencing the old, do not delete or edit the original. Audit trail beats neatness.

---

## Versioning + Anti-Patterns

### Versioning
- Master Vision: `v1.0` → `v1.1` (counsel-output-driven updates) → `v2` (next major strategic increment or post-quantum migration)
- Master Plan: SemVer-versioned via this file's git history; major V2-phase milestones trigger documented version increments in `CHANGELOG.md`
- Working Methodology (this doc): independent versioning — incremented when the methodology pattern itself evolves, not per-project-application

### Anti-Patterns to avoid

| Anti-pattern | Why it fails | Correct pattern |
|---|---|---|
| **Skip Phase 2** ("we know what's wrong") | Confirmation bias; Phase-1 author's blind spots stay invisible | Always 6 critiques minimum, parallel, from different angles |
| **Auto-merge draft PRs** | Phases 1–3 are work-products, not master-state | Only Phase 4 touches master. Phases 1–3 PRs stay draft FOREVER |
| **One mega-doc instead of foundation-docs** | Cognitive load on Phase-1 author × cognitive load on Phase-2 critic = serial deadlock | 4–6 focused docs, each ≤1000 lines, parallel-generatable, parallel-critiqueable |
| **No reversibility-tagging** | Future-you cannot judge whether to reopen a decision | Every decision tagged HIGH/MED/LOW reversibility + explicit review trigger |
| **Critique-agent saying "looks good"** | No critical signal generated; Phase 2 produces noise | Prompt must require findings; severity-tag every Probleme entry |
| **Master Plan = Master Vision with line-numbers shifted** | Master Plan was meant to be a distillation | Force Master Plan ≤50% of Master Vision lines; operational structure mandatory |
| **No counsel/external-engagement track** | Strategic plans bottleneck on legal/regulatory questions Claude cannot answer | Counsel + supervisor-engagement explicitly named in Master Plan with budget + scope |
| **First-10-customers / TAM-SAM-SOM "TBD"** | Fundraising-blocking. Phase 2 Business critique should catch this | Phase 1 strategic-positioning doc MUST attempt market sizing; Phase 2 Business critique re-baselines |

---

## When to Skip This Methodology

This 4-phase iteration is heavyweight (~3–5 sessions just to produce strategic plan, before any code). Don't use it for:
- Single-Welle features (use `.handoff/v*-welle-*-plan.md` pattern instead)
- Bug fixes
- Refactors
- Compliance-only collateral (use direct counsel-engagement)
- Anything where the answer is "obvious to all stakeholders" and the work is execution-only

Use it when:
- Strategic ambiguity persists across multiple sessions
- Multi-stakeholder buy-in is required before commitment
- Reversibility of the decision is LOW once made (e.g., V2 architecture lock-in)
- The Großthema spans multiple Wellen (e.g., V2-α through V2-δ)

---

**End of Working Methodology v1.** Updates to this doc indicate the methodology pattern has evolved; updates to project-specific Master Plans (e.g. `docs/V2-MASTER-PLAN.md`) indicate strategic-content updates within the existing pattern.
