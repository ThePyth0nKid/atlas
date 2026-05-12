# Atlas V2 — Session Handoff (Strategic Iteration Mode, Phase 1 SHIPPED → Phase 2 pending)

> **Bootstrap prompt für die nächste Claude Code Session.** Self-contained: ein Fresh Agent kann dieses Dokument + die referenzierten Files lesen und Phase 2 starten ohne irgendetwas neu zu derivieren. Designed um über mehrere Sessions stabil zu sein.

**Erstellt:** 2026-05-12. **Phase 1 shipped:** 2026-05-12 (this session). **Status:** Phase 1 ABGESCHLOSSEN, Phase 2 startbereit. **Next session entry point:** dieses Dokument §0a (Phase 1 SHIPPED block) zuerst lesen, dann die 5 Phase-1-Docs auf PR #59 review'n, dann Phase 2 critique-agent dispatch per §0b planen.

---

## 0a. Phase 1 SHIPPED — 2026-05-12 (this session)

**Phase 1 of the V2 strategic iteration ist abgeschlossen.** 5 Foundation Documents wurden parallel von 5 isolierten Subagents in eigenen git worktrees geschrieben. Integration auf branch `v2/phase-1-foundation` (PR #59, **DRAFT**, **NICHT mergen** — das ist der Phase-2-critique-target).

**Integration PR:** https://github.com/ThePyth0nKid/atlas/pull/59 (draft state)

**Die 5 Foundation Documents (alle auf `v2/phase-1-foundation` branch):**

| # | Doc | File | Lines | Subagent |
|---|-----|------|-------|----------|
| A | Strategic Positioning Vision | `.handoff/v2-vision-strategic-positioning.md` | 512 | general-purpose |
| B | Knowledge Graph Layer Architecture (v0.5) | `.handoff/v2-vision-knowledge-graph-layer.md` | 727 (+608/-93 vs v0) | general-purpose |
| C | Risk Matrix | `.handoff/v2-risk-matrix.md` | 457 | security-reviewer |
| D | Competitive Landscape (2026-05 baseline) | `.handoff/v2-competitive-landscape.md` | 630 | general-purpose + WebSearch |
| E | Demo Sketches | `.handoff/v2-demo-sketches.md` | 485 | general-purpose |

**Headline theses (one bullet each):**
- **Doc A** — Two-market positioning (Verifiable Second Brain + Multi-Agent Shared Memory); 6 novel trust-modes (continuous regulator attestation / insurance pricing substrate / Agent Passports / Cedar write-time / AI-BOM / B2B cross-org); 4 GTM hypotheses sequenced.
- **Doc B** — Three-Layer Architecture: events.jsonl (authoritative) + FalkorDB projection (queryable) + Mem0g cache (fast retrieval); Atlas as Hermes Memory Skill (4-call API); Agent Passports as `did:atlas:<pubkey-hash>` DIDs with revocation chain; federated witness cosignature; GDPR via content/hash separation.
- **Doc C** — Top-5 risks: R-A-01 Projection Determinism Drift (LOW detect, CRITICAL impact), R-L-01 GDPR Right-to-be-Forgotten (EU privacy counsel required), R-A-03 Agent Identity Key Compromise (V2-α blocking), R-S-01 Adoption Tipping Point (structural to category), R-L-02 FalkorDB SSPL License Trap.
- **Doc D** — No competitor has cryptographic trust in either category (verified via WebSearch 2026-05). Kuzu acquired by Apple Oct-2025 — ArcadeDB is next viable Apache-2.0 fallback. Graphiti = strongest partner candidate, 12-18mo competitor-risk. Obsidian has zero signature/verification plugins — white space for fast Verifiable-Second-Brain validation.
- **Doc E** — Demo 1 Multi-Agent Race = recommended landing-page hero. Demo 2 Regulator Witness = most ship-able TODAY (V1.14 live). Demos 3-5 need V2-α/β/γ/δ work; 4 of 5 are aspirational. Honesty flag raised in own open questions.

**Total content:** 2811 lines of strategic + architectural + risk + competitive + product material across 5 Foundation Documents.

**Open-Questions surface:** every doc carries an explicit "Open Questions for Phase 2 Critique" section. Combined ~55-65 explicit open questions across all 5 docs. **Cross-doc inconsistency is expected and NOT a Phase-1 convergence criterion** (per Iteration-Framework §1) — discrepancies are resolved in Phase 3 synthesis, not in Phase 1.

**Master HEAD on Phase-1-completion:** master remains at `5f19348` (V2 strategy trilogy). Phase 1 docs ONLY live on `v2/phase-1-foundation` branch — they do not enter master until Phase 4's `docs/V2-MASTER-PLAN.md`.

**Worktrees from Phase 1 (5 doc-branches + 1 orphan from architect re-dispatch):**
- `agent-a9da7cf2b6af8198c` / branch `worktree-agent-a9da7cf2b6af8198c` (Doc A, merged)
- `agent-a47f83e4af0f7b2d5` / branch `worktree-agent-a47f83e4af0f7b2d5` (Doc B re-dispatch, merged)
- `agent-adfac218b1cda42a9` / branch `worktree-agent-adfac218b1cda42a9` (Doc C, merged)
- `agent-ad7977870e1b40ef5` / branch `worktree-agent-ad7977870e1b40ef5` (Doc D, merged)
- `agent-a880ad3bdfa5c1083` / branch `worktree-agent-a880ad3bdfa5c1083` (Doc E, merged)
- `agent-a7f0eb28efcf59ae3` (orphan from architect re-dispatch, no writes — should be cleaned)

Cleanup these worktrees post-Phase-2 (or now if disk space matters): `git worktree remove <path> && git branch -D <branch>` per branch.

---

## 0b. Phase 2 Plan — Multi-Angle Critique (next session entry point)

**Goal:** 6 parallele critique-Subagents lesen alle 5 Phase-1-Docs auf PR-Branch `v2/phase-1-foundation` und produzieren strukturierte +/- Crits per Iteration-Framework §2.

**Pre-flight (vor Phase-2-dispatch):**
1. `git fetch origin && git checkout v2/phase-1-foundation` — sicherstellen die Branch ist lokal aktuell
2. Read alle 5 Phase-1-Docs (Files: `.handoff/v2-vision-strategic-positioning.md`, `.handoff/v2-vision-knowledge-graph-layer.md`, `.handoff/v2-risk-matrix.md`, `.handoff/v2-competitive-landscape.md`, `.handoff/v2-demo-sketches.md`)
3. Read Iteration-Framework §2 (`.handoff/v2-iteration-framework.md`) — critique-format template
4. **Mit Nelson abstimmen:** sind die 6 critique-Rollen unverändert (architect / security / database-performance / product-UX / compliance-regulatory / business-investor) oder Anpassung gewünscht?

**Dispatch convention (mirror Phase 1):**
- 6 parallele Agent-Calls in einer Message
- `isolation: "worktree"` für jeden (eigene Branch je crit)
- Pfade in Prompts **relativ** (`.handoff/...` NICHT `C:/Users/.../.handoff/...`)
- Subagent_types matchen die Crit-Rolle (architect → architect, security → security-reviewer, database-performance → general-purpose, product-UX → general-purpose, compliance-regulatory → general-purpose, business-investor → general-purpose)
- Each crit produces `.handoff/crit-<role>.md` (~300-500 lines)

**Crit format template** (per Iteration-Framework §2):
```
# Crit: <role> on Atlas V2 Vision
## Stärken (was ist gut, sollte bleiben)
## Probleme (was muss adressiert werden — by severity: CRITICAL/HIGH/MEDIUM/LOW)
## Blinde Flecken (was wird in den docs gar nicht angesprochen)
## Konkrete Vorschläge (specific edits/additions, doc-section-tagged)
## Offene Fragen für Phase 3
```

**The 6 critique agents + their primary doc targets:**

| # | Crit-Rolle | Subagent-Type | Primary Doc Target | Output |
|---|---|---|---|---|
| 1 | Architect | architect (Read/Grep/Glob only — produce text inline, parent writes file) | Doc B + Doc D — technical feasibility, projector-determinism, multi-tenant isolation, FalkorDB vs Kuzu-now-archived | `.handoff/crit-architect.md` |
| 2 | Security reviewer | security-reviewer | Doc B + Doc C — trust invariant integrity, key management, replay attacks, post-quantum, GDPR conflict, Agent-DID revocation | `.handoff/crit-security.md` |
| 3 | Database / performance | general-purpose | Doc B + Doc D — FalkorDB vs ArcadeDB (Kuzu archived!) vs Neo4j vs Memgraph, performance vs Mem0g, projection-rebuild-cost at scale, index strategy | `.handoff/crit-database.md` |
| 4 | Product / UX | general-purpose | Doc A + Doc E — positioning coherence, user-journey realism, demo-conversion-likelihood, Obsidian-comparison-fairness, multi-agent-race-demo-feasibility | `.handoff/crit-product.md` |
| 5 | Compliance / regulatory | general-purpose | Doc A + Doc C — EU AI Act Art. 12-19 mapping accuracy, AI-Liability-Directive readiness, agent-identity/DID compatibility, jurisdictional scope, witness-federation legal pattern | `.handoff/crit-compliance.md` |
| 6 | Business / investor | general-purpose | Doc A + Doc D + Doc E — market sizing, competitive moat, monetization paths, fundraising readiness, partnership candidates (Mem0/Graphiti/Hermes/Lyrie-ATP) | `.handoff/crit-business.md` |

**Lesson from Phase 1 (architect Read-only constraint):** the `architect` subagent_type only has Read/Grep/Glob — no Write. If using architect for Crit #1, expect inline text return; parent agent (this session's main thread) writes the file. Alternative: use `general-purpose` for all 6 crits to avoid the constraint, accepting that the architect role's specialism is lost.

**Convergence criterion for Phase 2** (per Framework §2): alle 6 crits geliefert, jede ≥5 strukturelle Punkte + ≥3 konkrete Edits. **Crits MÜSSEN adressieren, nicht nur "looks good".**

**Output:** integration branch `v2/phase-2-critiques` (analog to Phase 1), all 6 crits merged, PR opened **draft, no-merge**. Then Phase 3 synthesis (manual, with Nelson).

**Timing:** ~60-90 min for 6 parallel crits.

---

---

## 0. TL;DR für den Agent der das gerade liest

Atlas v1.0.1 ist LIVE auf npm mit SLSA Build L3 provenance (siehe `.handoff/v1.19-handoff.md` §0). V1 ist abgeschlossen. **Jetzt startet V2 — der Verifiable Second Brain + Shared AI Memory Substrate** Pivot. Nelson hat über mehrere Brainstorm-Iterationen folgendes finalisiert:

1. **Atlas ist agent-agnostisch** — wir bauen keinen Agent, wir bauen die Verification-Substrate die jeder Agent benutzen kann. MCP-Server (V1.19 Welle 1) ist bereits der universal write-side adapter.

2. **Zwei-Markt-Positionierung:** Human-Second-Brain (Obsidian-Kategorie + cryptographic trust) UND Multi-Agent-Shared-Memory (jeder Agent — Hermes, Claude, GPT, Llama, custom — schreibt in dieselbe verifizierbare Wissensbasis).

3. **Stack-Confirmation:**
   - **FalkorDB** als Graph-DB-Layer (V2-α), Cypher-subset, GraphBLAS-Backend, eigenes FalkorDB Browser UI
   - **Mem0 + Mem0g** als Fast-Retrieval-Cache on top (91% p95 latency reduction, 26% besser als OpenAI Memory)
   - **Hermes Agent** (Nous Research, 60K+ GitHub stars, MIT-license, model-agnostic, self-improving) als Primary Demo-Agent — ist seit 2026-05-10 #1 auf OpenRouter, vom Thron gestoßen "OpenClaw"
   - **Trust-Layer bleibt V1's signed events.jsonl + Sigstore Rekor anchoring** — Graph-DB und Retrieval-Cache sind beide deterministisch rebuildable Projektionen

4. **Security Experts** kommen ans ENDE (post-V2-α/β). Nicht jetzt. Volle Kraft voraus mit aktueller AI-Capability.

5. **Iteration vor Implementation** — Nelson will über die Vision iterieren bevor irgendein Code geschrieben wird. Strukturiertes 4-Phasen-Framework ist in `.handoff/v2-iteration-framework.md` festgelegt.

**Was diese Session tut:** Plant Phase 1 (Foundation Documents) sorgfältig, dann dispatched 5 parallele Subagents in isolierten Worktrees, jeder schreibt ein Foundation-Doc auf eigener Branch.

---

## 1. Mandatory pre-read order (vor jeder anderen Aktion)

Liest diese Files in dieser Reihenfolge, dann fasst kurz zusammen was du verstanden hast, BEVOR du irgendwas anderes tust:

1. **`.handoff/v1.19-handoff.md`** — Atlas state, V1 history, Standing Protocol (the §0 "Welle 14a SHIPPED" block ist der current state)
2. **`.handoff/v2-iteration-framework.md`** — 4-Phasen-Methodik mit Convergence-Kriterien (das ist deine Bibel für diese Phase)
3. **`.handoff/v2-vision-knowledge-graph-layer.md`** — Technical Architecture Vision v0 (das wird Doc B in Phase 1, schon partial geschrieben)
4. **`CLAUDE.md`** (falls vorhanden) — repo-specific instructions
5. **Quickly skim:** `docs/SEMVER-AUDIT-V1.0.md`, `docs/ARCHITECTURE.md` für Kontext (du musst nicht alles lesen, nur die V2-Boundary Section in ARCHITECTURE.md)

Nach dem pre-read: **gib Nelson eine 5-Bullet-Zusammenfassung** was du verstanden hast. Wenn Nelson sagt "weiter", dann erst Phase 1 planen.

---

## 2. Anti-drift checklist (run bevor irgendein Code geändert wird)

```bash
cd "C:/Users/nelso/Desktop/atlas"
git status                                   # → clean
git log --oneline -3                         # → top is 314b8d5 (Welle 14a SHIPPED docs)
git tag -l "v1.0.*"                          # → v1.0.0, v1.0.1
git verify-tag v1.0.1                        # → Good ed25519 signature
GH="/c/Program Files/GitHub CLI/gh.exe"
"$GH" repo view ThePyth0nKid/atlas --json visibility   # → "PUBLIC"
"$GH" release view v1.0.1 --json isDraft     # → isDraft false
npm view @atlas-trust/verify-wasm@1.0.1 dist-tags   # → { "latest": "1.0.1" }
```

Wenn irgendwas davon nicht stimmt: **stop, klär mit Nelson**. Vermutlich ist der state aktueller als dieses Doc — dann reportiere den drift und frage was als nächstes.

---

## 3. Subagent orchestration architecture (das ist Nelson's explicit goal)

**Goal:** "Einzelne Agenten in einzelnen Branches so orchestriert dass sie sich nicht gegenseitig stören oder blockieren."

**Architektur:** Jeder Phase-1-Subagent läuft in einem **eigenen git worktree** mit eigener Branch. Atlas's master bleibt unangetastet während Phase 1 läuft. Konflikt-frei weil verschiedene Files written werden.

### Branch convention

```
master                                    ← stays clean during V2 strategy work
  │
  ├─ docs/v2/phase-1-doc-A-positioning    ← Subagent A schreibt nur in .handoff/v2-vision-strategic-positioning.md
  ├─ docs/v2/phase-1-doc-B-architecture   ← Subagent B refined .handoff/v2-vision-knowledge-graph-layer.md
  ├─ docs/v2/phase-1-doc-C-risk-matrix    ← Subagent C schreibt .handoff/v2-risk-matrix.md
  ├─ docs/v2/phase-1-doc-D-competitive    ← Subagent D schreibt .handoff/v2-competitive-landscape.md
  └─ docs/v2/phase-1-doc-E-demo-sketches  ← Subagent E schreibt .handoff/v2-demo-sketches.md
```

Each branch produces exactly ONE file delta. Zero overlap. Zero merge conflict risk.

### Worktree setup

Use Agent tool with `isolation: "worktree"` parameter — that auto-creates worktree + branch. The worktree is auto-cleaned if no changes; otherwise path + branch returned in result.

### Post-Phase-1 integration

After all 5 subagents return: dispatched checks:
1. Read each subagent's output file
2. Cross-check for internal consistency (no contradictory claims about Mem0g, Hermes Agent, etc.)
3. Merge alle 5 branches sequenziell in **integration branch** `v2/phase-1-foundation` via gh API
4. Eine PR `v2/phase-1-foundation → master` mit allen 5 docs als reviewable unit
5. Phase 2 critique agents arbeiten gegen diese integration branch, NICHT gegen master direkt

**Important:** Phase 1 docs werden nicht direkt nach master gemerged. Sie warten auf Phase 2 critique → Phase 3 synthesis → erst dann landet das gemerged Master-Vision-Doc auf master.

---

## 4. Strategischer Kontext — was Nelson erreichen will (don't lose this)

Aus den vorhergehenden Sessions verdichtet:

**Vision:** Atlas wird "die TÜV-Plakette für AI-agent memory" — strukturell verifizierbare gemeinsame Wissensbasis für humans + agents. Verifier-Crates Apache-2.0 (anyone can fork/embed), Server/Web Sustainable Use (revenue from hosted-service), Open-Core analog zu Obsidian's free-local-paid-sync model.

**Wettbewerbs-Landschaft:**
- Obsidian / Notion / Roam → human Second Brain, KEIN cryptographic trust
- Mem0 / Letta / Zep → AI memory, KEIN cryptographic trust (Atlas + Mem0 ist orthogonal hybrid)
- Graphiti → temporal KG framework, KEIN cryptographic trust, supports FalkorDB als backend (gut für Atlas)
- Anthropic Memory / OpenAI Memory → vendor-silo, geschlossen, nicht cross-vendor verifiable
- **Niemand sonst macht "cryptographic memory substrate, agent-agnostic, cross-vendor"** — Greenfield für Atlas

**EU AI Act als Driver:**
- Art. 12 (in force 2026-08-02): mandatory automatic event logs, independently verifiable
- Art. 13: Transparenz gegen Deployer
- Art. 14: Human oversight
- Art. 18: 6-Monate retention
- Plus die proposed **EU AI Liability Directive** (2026 expected, in Council-Phase) — Beweislast auf Provider

**Neue Trust-Modes die Atlas strukturell ermöglicht:**
- **Continuous regulator attestation** — Aufsicht hat Witness-Key in Trust-Root, Cosignatur in Echtzeit, kein periodisches Reporting mehr
- **AI-Liability-Insurance pricing** — Atlas-attested events = clean Schadens-Substanz, signifikant günstigere Prämien möglich
- **Agent Passports** — every agent has Ed25519 keypair, verifiable history of writes, portable reputation across organizations
- **Pre-action policy enforcement** — Cedar policies enforce at write-time, Compliance wird strukturell

**Hermes Agent Integration als go-to-market:**
- Hermes Agent (Nous Research) hat Plugin/Skill-System
- Atlas könnte ein First-Class "Atlas Memory Skill for Hermes Agent" werden
- Issue #477 in Nous's repo zeigt sie sind offen für Skill-Erweiterungen
- Hermes-Adoption-Wachstum (60K stars in 2 Monaten) macht das einen riesigen Distribution-Hebel

**Riskien die wir adressieren müssen:**
1. Adoption tipping point (Catch-22 — start mit EU-regulated vertical wo Compliance Driver ist)
2. Performance overhead (mitigation: tiered anchoring — hot writes signed-only, batch-anchored zu Rekor)
3. UX-Komplexität (mitigation: hide trust by default, show only "Verified ✓" / "Tampered ✗")
4. Vendor capture (mitigation: open-weight-models als pull-faktor, vendor-erlaubnis nicht nötig)
5. GDPR right-to-be-forgotten conflict (mitigation: signed hashes, raw content separable)
6. Privacy/confidentiality (mitigation: private federation tier neben public-witness tier)
7. **Post-quantum crypto** (V1 Algorithm enum ist additive, Migration-Plan als Welle 14d candidate — NICHT Phase-1-blocking, aber im Risk-Doc-C aufnehmen)

---

## 5. Phase 1 Goal — Foundation Documents

Fünf parallele Docs, jeder von eigenem Subagent in eigener Branch:

| Doc | File | Subagent Type | Scope |
|---|---|---|---|
| **A** | `.handoff/v2-vision-strategic-positioning.md` | general-purpose | Vision + Positioning + Beyond-Storage value + EU AI Act mapping + Agent identities + Two-market story (Second Brain + Shared Memory) |
| **B** | `.handoff/v2-vision-knowledge-graph-layer.md` (revise existing v0!) | architect | Tech architecture refined: events.jsonl → projector → FalkorDB → Mem0g hybrid + MCP read-side API + Hermes Agent skill integration |
| **C** | `.handoff/v2-risk-matrix.md` | security-reviewer | 8-12 risks: probability × impact × mitigation × owner. Inkl. post-quantum, GDPR, adoption-tipping, vendor-capture, privacy/confidentiality, performance |
| **D** | `.handoff/v2-competitive-landscape.md` | general-purpose | Mem0 / Letta / Zep / Graphiti / Obsidian / Notion / Anthropic-Memory / OpenAI-Memory feature × pricing × trust-property × Atlas-differentiator matrix |
| **E** | `.handoff/v2-demo-sketches.md` | general-purpose | 5 demo scenarios with 30-90s storyboard each: Hermes-Multi-Agent / Continuous-Audit-Mode / Agent-Passport / Mem0g-Hybrid / Verifiable-Lineage |

**Phase 1 convergence criteria:** Alle 5 Files existieren als v0, intern konsistent, mit explicit "Open Questions for Phase 2" Section am Ende jedes Files (Phase 2 critique-Agents brauchen das). Cross-file consistency wird in Phase 3 hergestellt — NICHT Phase 1.

---

## 6. Subagent-Prompts (ready-to-dispatch, verbatim)

Diese Prompts sind kuratiert worden über mehrere Iterations-Runden. Bevor du sie dispatchst, **review jeden Prompt nochmal kurz** mit Nelson — falls etwas wesentliches fehlt, add it. Aber don't rewrite from scratch, sie sind solide.

Dispatch alle 5 in einer einzigen Message (Anthropic-API supports parallel tool calls):

### 6.1 Doc A — Strategic Positioning

```
subagent_type: general-purpose
isolation: worktree
description: "Atlas V2 Doc A — Strategic Positioning"

prompt:
You are writing the strategic positioning vision document for Atlas V2. Context — Atlas is a cryptographic trust-verification project. V1.0.1 just shipped on npm (2026-05-12) with SLSA Build L3 provenance. V2 pivots to "verifiable knowledge graph substrate for any AI agent + human Second Brain".

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (this entire document, especially §4)
- /Users/nelso/Desktop/atlas/.handoff/v1.19-handoff.md §0
- /Users/nelso/Desktop/atlas/README.md
- /Users/nelso/Desktop/atlas/docs/SEMVER-AUDIT-V1.0.md (skim)
- /Users/nelso/Desktop/atlas/docs/ARCHITECTURE.md (V2 boundary section)

WRITE: /Users/nelso/Desktop/atlas/.handoff/v2-vision-strategic-positioning.md (~600-1000 lines)

STRUCTURE the document as follows (use these exact section headers):

# Atlas V2 — Strategic Positioning Vision

## 1. The Pivot (was V1, was V2 wird)
Worauf V1 hat geantwortet (compliance gap, EU AI Act Art. 12). Was V2 strukturell aufmacht (agent-agnostic shared substrate, Verifiable Second Brain category). Tagline candidates (mindestens 3).

## 2. Two-Market Positioning
2a. Verifiable Second Brain (Obsidian/Notion category + crypto trust)
2b. Multi-Agent Shared Memory (Hermes/Claude/GPT/custom all couple in)
Show the market sizing logic, target persona für each, why both markets work for the same substrate.

## 3. EU AI Act Structural Fit
Art. 12/13/14/18 mapping (use the table from §4 of the session handoff). Plus the proposed EU AI Liability Directive (2026 expected) implications.

## 4. New Trust-Modes Atlas Enables (genuinely novel — not just compliance)
4a. Continuous regulator attestation (Aufsicht's witness key live in trust root)
4b. AI-Liability-Insurance pricing substrate
4c. Agent Passports — Ed25519 keypair = verifiable agent identity + reputation
4d. Pre-action policy enforcement via Cedar at write-time
4e. AI Bill of Materials (AI-BOM) substrate
4f. B2B cross-organization trust patterns

## 5. Competitive Differentiation
Mem0 / Letta / Zep / Graphiti / Anthropic Memory / OpenAI Memory / Obsidian — Atlas's unique structural property vs each. Don't make this exhaustive (Doc D will do that); just the headline differentiator. One sentence per competitor max.

## 6. Go-to-Market Hypotheses
6a. Hermes Agent skill integration als first distribution-vehicle
6b. EU-regulated verticals (Finance/Healthcare/Insurance) als Compliance-driven adoption
6c. Open-weight-model alignment as pull-factor against vendor-capture
6d. Obsidian-style open-core monetization (free verifier, paid hosted sync / enterprise)

## 7. Risks to Positioning
Acknowledge the 5-7 most strategically dangerous things (don't make this list exhaustive — Doc C does that). Focus on positioning-level risks: market timing, vendor opposition, narrative complexity.

## 8. Open Questions for Phase 2 Critique
List 10-15 explicit questions that the Phase 2 product/business/strategy critique agents should challenge. Format: "Q: <question>. Context: <1-sentence why this matters>. Status: open."

CRITICAL guidelines:
- This is a STRATEGIC positioning doc, not a tech doc. Don't dive into FalkorDB / Mem0g architecture details — that's Doc B's job. Reference them at high level only.
- Be specific, not generic. "Atlas changes the game" = bad. "Atlas enables continuous regulator attestation by federating the regulator's witness key into the in-repo trust root — currently no compliance vendor offers this" = good.
- Cite Atlas's existing V1 capabilities by reference to specific files/features (e.g., "atlas-mcp-server (V1.19 Welle 1) already provides agent-agnostic write surface for any MCP-compatible host").
- Use German for headers and short prose, English for technical terminology and citations. Mirror the style of v1.19-handoff.md.
- Acknowledge what you don't know. Don't fabricate market data.

When done: write only the file. Do NOT commit. Do NOT push. Return a 5-bullet summary of the doc's main thesis.
```

### 6.2 Doc B — Technical Architecture (REVISE existing v0)

```
subagent_type: architect
isolation: worktree
description: "Atlas V2 Doc B — Technical Architecture (revise v0)"

prompt:
You are revising the technical architecture vision for Atlas V2. A v0 of this doc EXISTS — your job is to refine, deepen, and complete it based on the strategic decisions made in subsequent brainstorming.

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (entire document, especially §4)
- /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md (the EXISTING v0 — your starting point)
- /Users/nelso/Desktop/atlas/.handoff/v1.19-handoff.md §0
- /Users/nelso/Desktop/atlas/docs/ARCHITECTURE.md (full V2 boundary section)
- /Users/nelso/Desktop/atlas/docs/SEMVER-AUDIT-V1.0.md

REVISE IN PLACE: /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md

KEY ADDITIONS / REFINEMENTS NEEDED (the v0 doc doesn't capture these yet):

1. **Mem0g hybrid architecture explicit.** Mem0g is the graph-enhanced variant of Mem0 (91% p95 latency reduction, <5pt accuracy gap vs full-context, 2.59s p95). Add §2.5 "Three-Layer Architecture: events.jsonl (authoritative) + FalkorDB projection (queryable) + Mem0g cache (fast retrieval)". Trust invariant: Mem0g cache is rebuildable from events.jsonl, never trust-authoritative.

2. **Hermes Agent skill integration path.** Hermes Agent (Nous Research, 60K+ GitHub stars, MIT license, model-agnostic) has a plugin/skill system. Add §2.6 "Atlas as Hermes Agent Memory Skill" — specify the integration surface (HTTP API endpoints the skill calls, what the skill exposes back to Hermes's reasoning loop, how skill-generated facts flow into events.jsonl with attribution to Hermes-instance-key).

3. **Agent identity layer (Ed25519-DID).** V1's per-tenant HKDF keys generalize to per-agent keys. Add §2.7 "Agent Identities as W3C DIDs (did:atlas:<pubkey-hash>)". Specify how agent passports work — public key + verifiable history + revocation chain.

4. **Read-side API surface.** V1 has POST /api/atlas/write-node. V2 needs read endpoints. Add §2.8 "Read-Side API" — propose 3-5 endpoints: GET /entities/:id, GET /related/:id?depth=N, GET /timeline/:workspace?from=&to=, POST /query (Cypher passthrough?), POST /audit/:event_uuid (full provenance trail).

5. **MCP tool surface expansion.** V1's atlas-mcp-server exposes write_node + verify_trace. V2 needs query tools. Add §2.9 "MCP V2 Tool Surface" — propose tools: query_graph (Cypher-like), query_entities (semantic), query_provenance (trace any fact to source events), get_agent_passport (verify another agent's identity + reputation).

6. **Continuous regulator attestation architecture.** Add §2.10 "Federated Witness Cosignature for Regulators" — how a regulator's witness key gets added to the federation roster, what get cosigned, what the audit-trail looks like operationally.

7. **GDPR / Right-to-be-forgotten handling.** Add §3.3 (new open question): "Signed content vs deletable content separation. Strategy: events.jsonl signs hashes only, raw content lives in separate (deletable) storage. Trust property survives content deletion: hash exists, anchor exists, original content nullable = 'redacted but verified existed at time T'."

KEEP the existing v0 §0 (intent), §1 (positioning — refresh slightly per Doc A direction), §2.1-§2.4 (existing blueprint, trust invariant, Graphiti notes, FalkorDB section), §3 (existing open questions — expand to incorporate new ones), §4 (decomposition — refine V2-α/β/γ/δ to reflect three-layer architecture), §5 (Welle alignment), §6 (iteration CTA).

CRITICAL guidelines:
- Use ASCII diagrams where they help (the v0 has one — improve / extend if more would help).
- Be VERY explicit about trust invariants. Every layer addition must explain "what if this fails — does V1's trust property survive?". The answer for all new layers must be "yes, because they're derivative not authoritative".
- Each new section should be self-contained enough that an architect/security agent in Phase 2 can crit it without needing the whole doc context.
- Add references to specific Atlas crates / files where relevant (e.g., "events.jsonl format spec: see crates/atlas-trust-core/src/wire.rs").

When done: write only the file. Do NOT commit. Do NOT push. Return a diff summary (what was added vs the v0 baseline).
```

### 6.3 Doc C — Risk Matrix

```
subagent_type: security-reviewer
isolation: worktree
description: "Atlas V2 Doc C — Risk Matrix"

prompt:
You are writing a risk matrix for Atlas V2. This is NOT a generic risk doc — it's specifically about the strategic and architectural risks of the V2 pivot (verifiable knowledge graph substrate + Mem0g hybrid + Hermes Agent integration + agent identities).

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (entire document, especially §4)
- /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md (current v0, will be refined in parallel)
- /Users/nelso/Desktop/atlas/.handoff/v2-iteration-framework.md
- /Users/nelso/Desktop/atlas/docs/SECURITY-NOTES.md
- /Users/nelso/Desktop/atlas/docs/SEMVER-AUDIT-V1.0.md

WRITE: /Users/nelso/Desktop/atlas/.handoff/v2-risk-matrix.md

STRUCTURE:

# Atlas V2 — Risk Matrix v0

## Methodology
Score each risk on: Probability (LOW/MEDIUM/HIGH/CRITICAL), Impact (LOW/MEDIUM/HIGH/CRITICAL), Detectability (HIGH/MEDIUM/LOW — how fast we'd see it materialize), Reversibility (HIGH/MEDIUM/LOW — how recoverable). Plus mitigation status and owner.

## Risk Categories
### Strategic / Market Risks
### Architectural / Trust Risks
### Cryptographic / Crypto-Agility Risks
### Operational / Adoption Risks
### Legal / Regulatory Risks
### Vendor / Ecosystem Risks

## Detailed Risks
For each risk, use this template (8-12 risks total — quality over quantity):

### R-XX: <Short Title>
- **Category:** Strategic / Architectural / Crypto / Operational / Legal / Vendor
- **Description:** 2-3 sentences. What goes wrong, in what scenario.
- **Probability:** LOW / MEDIUM / HIGH / CRITICAL
- **Impact:** LOW / MEDIUM / HIGH / CRITICAL (separately considering: financial, technical, reputational)
- **Detectability:** HIGH / MEDIUM / LOW
- **Reversibility:** HIGH / MEDIUM / LOW
- **Current Mitigation Status:** NONE / PARTIAL / ADEQUATE / ROBUST
- **Mitigation Strategy:** Specific actions. What we'd do if it materialized + what we can do proactively.
- **Owner:** Engineering / Product / Legal / External-Security / Strategy
- **Review Cadence:** Quarterly / Per-Welle / Per-Release / Continuous

MUST-COVER risks (specifically address these — Nelson identified them as concerns):

1. **R-Adoption-Tipping-Point** — Atlas is only valuable when used. Catch-22: agents only adopt if Atlas has critical mass, mass only forms if agents adopt. (Strategy)

2. **R-Performance-Overhead** — Every write does crypto + chain hash + eventual Rekor anchor. At 10K writes/sec what breaks? (Operational/Architectural)

3. **R-UX-Complexity** — "Cryptographic provenance" is a feature humans don't want to think about. If UX surfaces too much trust-machinery, adoption fails. (Operational)

4. **R-Vendor-Capture** — Major AI vendors (Anthropic / OpenAI / Google) refuse to integrate or actively oppose. Adressable market shrinks. (Vendor)

5. **R-GDPR-Right-to-be-Forgotten** — Signed events are forever. EU GDPR Art. 17 conflict. (Legal)

6. **R-Privacy-vs-Public-Anchoring** — Sigstore Rekor anchoring is public. What if enterprise data is confidential? (Architectural)

7. **R-Post-Quantum-Migration** — Ed25519 secure today, future quantum-vulnerable. (Crypto)

8. **R-Mem0-Vendor-Risk** — Atlas-+-Mem0 hybrid depends on Mem0 staying healthy. Mem0 is venture-backed startup — vendor risk. (Vendor)

9. **R-Hermes-Adoption-Reversal** — Hermes Agent grew 60K stars in 2 months. If it stalls or gets supplanted, Atlas's Hermes-skill distribution play loses value. (Vendor)

10. **R-Projection-Determinism-Drift** — Graph DB / Mem0g cache must rebuild byte-identically from events.jsonl. If projection drifts silently, trust invariant breaks invisibly. (Architectural)

PLUS: Add 2-3 risks YOU identify that I haven't listed. Independent thinking required.

## Risk Heatmap
ASCII table mapping risk severity (Probability × Impact) — make it visually scannable.

## Open Questions for Phase 2 Critique
Especially around: which risks are under-quantified, which mitigations are unrealistic, which categories are missing.

CRITICAL guidelines:
- Be honest about mitigation status. If we have NO real mitigation, say NONE.
- Don't pad. 10 well-thought risks > 30 mediocre.
- Quantify where possible (e.g., "GDPR violations carry fines up to 4% of global revenue").
- Reference Atlas's V1 trust property as the bedrock — most risks should be analyzed against "does V1 invariant still hold under this failure mode?".

When done: write only the file. Do NOT commit. Do NOT push. Return a 5-bullet summary of the highest-severity risks.
```

### 6.4 Doc D — Competitive Landscape

```
subagent_type: general-purpose
isolation: worktree
description: "Atlas V2 Doc D — Competitive Landscape"

prompt:
You are writing a competitive landscape analysis for Atlas V2. The market spans two adjacent categories: (1) AI agent memory infrastructure (Mem0, Letta, Zep, Graphiti, Anthropic Memory, OpenAI Memory) and (2) human Second Brain tools (Obsidian, Notion, Roam Research, Logseq). Atlas's unique structural property is cryptographic trust — no current competitor has it.

You MUST do current research (this is 2026-05-12). Use WebSearch to confirm current state of each competitor: pricing, features, license, latest releases, user base. Do NOT rely on knowledge cutoff.

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (entire document, especially §4)
- /Users/nelso/Desktop/atlas/README.md
- /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md

WRITE: /Users/nelso/Desktop/atlas/.handoff/v2-competitive-landscape.md

STRUCTURE:

# Atlas V2 — Competitive Landscape v0 (2026-05)

## 1. Two Market Categories
1a. AI Agent Memory Infrastructure (target persona: AI engineer / agent builder)
1b. Human Second Brain / Personal Knowledge Management (target persona: knowledge worker / researcher)
1c. Atlas's unique position — substrate für BOTH, with cryptographic trust as the bridge

## 2. AI Agent Memory Layer Competitors
For each: License, Founded, Pricing, Features, User Base, Trust Property, Atlas Differentiator

### 2.1 Mem0
- Particularly verify Mem0g (graph variant) availability and current benchmarks
- Note that we plan to USE Mem0g as a complementary retrieval layer — they're not a pure competitor, they're a potential partner

### 2.2 Letta (formerly MemGPT)
### 2.3 Zep (and their Graphiti framework — note Graphiti is OSS, separate from Zep Cloud)
### 2.4 Anthropic Memory (Claude's native memory)
### 2.5 OpenAI Memory
### 2.6 Hindsight / Supermemory / Mem0-alternatives — short coverage only

## 3. Second Brain Competitors
### 3.1 Obsidian
- Verify current pricing tiers (free / Sync / Publish / Catalyst / Business)
- User base estimate
- Plugin ecosystem size
- Atlas-relevant: does Obsidian have ANY signature / verification plugin?

### 3.2 Notion
### 3.3 Roam Research
### 3.4 Logseq
### 3.5 Capacities, Tana, Heptabase — short coverage only

## 4. Knowledge Graph Tools (overlap with both categories)
### 4.1 Graphiti (Apache-2.0, supports FalkorDB backend — potential partner not pure competitor)
### 4.2 Neo4j (graph DB + Neo4j Browser UI — could host Atlas projection)
### 4.3 FalkorDB (graph DB + Browser — our planned V2 stack)
### 4.4 Kuzu (MIT license — pure OSS alternative to FalkorDB if SSPL becomes problem)

## 5. Trust / Verification Adjacent Tools
Check if any of these explicitly target AI memory / agent trust:
### 5.1 Sigstore ecosystem (we already use Rekor)
### 5.2 SLSA framework (we already implement L3)
### 5.3 VeritasChain Protocol (VCP v1.1, Dec 2025 — adjacent cryptographic AI audit log)
### 5.4 Any 2025-2026 "AI trust" projects that emerged

## 6. Comparison Matrix
ASCII table with rows = competitors, columns = (License / Pricing-Range / Trust-Property / Open-Source / Multi-Agent / Temporal / Provenance-API / GDPR-Compliant-by-design). Atlas in last row.

## 7. Strategic Insights
- Who's the most threatening competitor (and why)
- Who's the most natural partner (Mem0, Graphiti, Hermes Agent — explore each)
- Where are the white spaces Atlas can own
- What's the most likely competitor counter-move

## 8. Open Questions for Phase 2 Critique
- Did we miss any competitor?
- Is the "verifiable Second Brain" category real or aspirational?
- Are Mem0g and Graphiti truly partners, or will they evolve into trust-property competitors?

CRITICAL guidelines:
- Cite ALL sources (URLs at end of each subsection)
- Be honest where Atlas is weaker. If Mem0 has 5K production deployments and Atlas has 0, say so.
- Don't fabricate market data. If you can't verify a number, write "estimated" or "claimed by company".
- 2026-current data only. Verify everything via WebSearch.

When done: write only the file. Do NOT commit. Do NOT push. Return a 5-bullet summary of the most strategically important findings.
```

### 6.5 Doc E — Demo Sketches

```
subagent_type: general-purpose
isolation: worktree
description: "Atlas V2 Doc E — Demo Sketches"

prompt:
You are sketching demo scenarios for Atlas V2's landing page and investor/customer pitches. These demos need to make Atlas's unique value visible in 30-90 seconds of video or live interaction.

Read FIRST:
- /Users/nelso/Desktop/atlas/.handoff/v2-session-handoff.md (entire document, especially §4)
- /Users/nelso/Desktop/atlas/.handoff/v2-vision-knowledge-graph-layer.md
- /Users/nelso/Desktop/atlas/README.md

WRITE: /Users/nelso/Desktop/atlas/.handoff/v2-demo-sketches.md

STRUCTURE:

# Atlas V2 — Demo Sketches v0

## Methodology
Each demo follows a 5-block storyboard:
1. **Setup (5-10s):** What the viewer sees first.
2. **Action (15-40s):** What happens — the agent does X, the graph populates, etc.
3. **Reveal (10-20s):** The verification moment — click → cryptographic proof appears.
4. **Implication (10-20s):** Why this matters (one-sentence explainer).
5. **CTA (5s):** What the viewer does next.

For each demo, also specify: target audience, target emotion (surprise / trust / power / clarity), technical assets needed (atlas-web, FalkorDB, Hermes-Agent, etc.), and rough production complexity (1-5 scale).

## Demo 1 — Multi-Agent Race (Verifiable Attribution)
TWO agents (Hermes Agent + Claude via MCP) writing into the SAME Atlas workspace in real-time. Each fact appears in the graph with the writing agent's color-coded passport key + Sigstore Rekor logIndex. Viewer clicks any fact → modal shows: signed-by Hermes-instance-X / written at T / Rekor anchor proof / no-tampering certificate. **Audience:** AI engineers, builders. **Emotion:** trust + power. **Why it matters:** "Every fact has a verified author. No more 'the AI said it'."

## Demo 2 — Continuous Audit Mode (Regulator Witness)
Simulate a regulator's witness key federated into Atlas's trust root. Agent writes a financial recommendation. Cosignature appears in real-time from regulator-witness-key. Viewer sees: agent-signature + regulator-cosignature + timestamp + Rekor anchor. **Audience:** Compliance officers, regulators, financial services. **Emotion:** trust + clarity. **Why it matters:** "Compliance is structural, not periodic. The regulator's key is in the system."

## Demo 3 — Agent Passport (Reputation Portability)
Show an agent (Hermes-instance-X) that has written facts into Atlas for 30 days. Viewer queries the agent's passport: 1,247 facts written, 0 retractions, 12 unique witness cosigners, used by 3 organizations. Hire this agent → it brings its verifiable track record. **Audience:** Multi-tenant AI deployers, AI marketplaces. **Emotion:** clarity + power. **Why it matters:** "Agents have CVs now. Cryptographic ones."

## Demo 4 — Verifiable Second Brain (Obsidian Comparison)
Side-by-side: Obsidian vault vs. Atlas Second Brain. User types a note in both. Atlas auto-signs. User edits Atlas note from another device → previous version is preserved with signature + timestamp. User pretends to be a malicious teammate editing Obsidian directly → no signature, no detection. Atlas equivalent → tampering detection visible. **Audience:** Knowledge workers, researchers, teams. **Emotion:** surprise + trust. **Why it matters:** "Your Second Brain, but trustable for the AI era."

## Demo 5 — Mem0g Hybrid (Speed + Trust)
Side-by-side: standard Atlas query (verified, slower) vs. Atlas+Mem0g hybrid query (verified, 91% faster). Same accuracy, same cryptographic provenance, 1.44s vs 17.12s. Viewer sees both timings + identical results. **Audience:** AI engineers worried about latency. **Emotion:** clarity. **Why it matters:** "Cryptographic trust without the speed tax."

## Demo Selection for Landing Page Hero
Recommend WHICH demo should be the landing page hero (single 30-60s loop). Reason about audience-fit, emotional resonance, demo-feasibility-at-current-product-state.

## Production Requirements
Per demo: what tech needs to exist (real or mocked), what UI work is needed, what's blocking each one TODAY.

## Open Questions for Phase 2 Critique
- Are these demos honest about current Atlas capabilities, or do they require V2-α/β/γ before they're real?
- Is the multi-agent race demo emotionally compelling enough to lead with?
- Should we have a "non-AI" demo for the Second Brain market (Demo 4) at all, or focus on agent-builder audience first?

CRITICAL guidelines:
- Be REALISTIC about what's demo-able TODAY vs after V2-α / V2-β. Flag each demo's readiness.
- Don't write demos that require capabilities Atlas doesn't have. If a demo needs Mem0g integration and Mem0g isn't connected yet, say "requires V2-β" prominently.
- Think visual. Describe what's on screen at each beat. Not just "agent writes fact" but "left-pane: chat interface, right-pane: graph viz, fact node animates into existence".
- Audience-focused. A compliance-officer demo and a developer demo have completely different vocabulary.

When done: write only the file. Do NOT commit. Do NOT push. Return a 5-bullet summary of which demo is strongest and why.
```

---

## 7. Phase 1 Plan (review BEFORE dispatching subagents)

**Step 1 (current session):** Reviewer the 5 subagent prompts above with Nelson. He may want to:
- Add a sixth doc (e.g., F-Security-Experts comes back in scope earlier than expected)
- Reframe one of the prompts
- Add specific constraints / focal points

**Step 2:** Dispatch all 5 subagents in parallel (single Agent tool message with 5 calls). Each gets `isolation: "worktree"` and writes to its own branch. Expected timing: 60-120 minutes for all 5 v0 docs.

**Step 3:** As subagents complete, review each output:
- File written?
- Internally consistent?
- Open-Questions section present and substantial?
- Worktree path returned (so we know which branch to merge)

**Step 4:** Create integration branch `v2/phase-1-foundation` from master. Merge each subagent's branch sequentially. Resolve trivial conflicts (none expected — different files).

**Step 5:** Open PR `v2/phase-1-foundation → master` — but DO NOT merge. PR exists for review-visibility only. Phase 2 critique-agents work AGAINST this PR branch.

**Step 6:** Update this handoff doc:
- Mark Phase 1 complete
- Add Phase 2 plan section (which 6 critique agents, what prompts, what targets)
- Update master HEAD reference if anything changed

**Step 7:** Tell Nelson Phase 1 complete + briefly summarize each Doc's main thesis. Ask for green light on Phase 2.

---

## 8. Phase 2-4 — placeholder for future sessions

Phase 2 (critique agents) gets its OWN careful planning pass before dispatch. Don't dispatch Phase 2 from this session even if Phase 1 finishes fast — convergence-criteria matter, careful planning matters, and a fresh-eyes review of Phase 1 outputs is more valuable than rushing to Phase 2.

Phase 2-4 structure is documented in `.handoff/v2-iteration-framework.md` §2-4. The session that picks up Phase 2 should:
1. Read this handoff doc (updated post-Phase-1 by Step 6 above)
2. Read each of the 5 Phase 1 docs
3. Review the iteration-framework Phase 2 spec
4. Draft 6 critique-agent prompts (similar style to §6 above, customized for crit-role)
5. Get Nelson's green light
6. Dispatch

---

## 9. Standing Atlas conventions (don't break these in V2 work)

- **Git workflow:** Always PR. Always SSH-signed commits. Direct push to master is blocked by Rulesets.
- **Cargo PATH:** `/c/Users/nelso/.cargo/bin/cargo.exe` (not in default PATH).
- **gh CLI:** `/c/Program Files/GitHub CLI/gh.exe` (not in default PATH).
- **Standing Protocol:** implement → parallel `code-reviewer` + `security-reviewer` → fix CRITICAL/HIGH + selected MEDIUMs in-commit → SSH-signed feat commit → docs PR.
- **CHANGELOG.md curation:** Hand-curated, Keep-a-Changelog format. Each welle/feature gets 1-3 bullet narrative under Added/Changed/Fixed/Security.
- **SemVer audit gate:** Any change to items in `docs/SEMVER-AUDIT-V1.0.md` (especially Locked items) must be in-commit-updated.
- **Tag-immutability:** V1.17 Welle B contract. Signed tags are permanent. SemVer-patch is the forward-fix for post-tag-publish issues (precedent: v1.0.0 → v1.0.1 Cargo.toml URL fix, Welle 14a).

---

## 10. What "weiter" / "next" / "go" should default to (post-Phase-1)

If Nelson says "weiter" after Phase 1 completes: DO NOT auto-dispatch Phase 2. Instead:
1. Confirm Phase 1 outputs are all on the integration branch + PR
2. Show Nelson the 5 doc summaries
3. Ask if anything in Phase 1 outputs surprises him / changes the strategy
4. Then start careful Phase 2 planning per §8

The whole point of the iteration framework is **deliberate, not rushed**. Every phase gets its own careful planning pass.

---

## 11. If anything is unclear

Don't guess. Don't extrapolate from training data. Either:
- Read more of the existing files
- Ask Nelson with a specific clarifying question
- Use WebSearch to verify current external state (Hermes Agent, Mem0, etc.)

The strategic context in §4 was hard-earned over multiple brainstorming iterations. Preserve it; don't dilute it.

---

**End of handoff document.** Next agent: start with §1 (mandatory pre-read), then summarize what you understood to Nelson, then proceed.
