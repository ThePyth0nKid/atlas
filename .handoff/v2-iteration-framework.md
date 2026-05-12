# Atlas V2 — Iteration Framework

> **Meta-Dokument** für die Strategie-Findung vor V2-Implementation. Definiert wie wir iterieren, mit welchen Rollen, in welcher Reihenfolge, mit welchen Konvergenz-Kriterien. Nach Phase 4 = wiederverwendbare Arbeits-Methodik für alle weiteren Atlas-Wellen.

**Status:** v0 Draft 2026-05-12. **Sequenz:** Phase 1 → 2 → 3 → 4 → Welle-Decomposition.

---

## Warum dieses Framework existiert

Ad-hoc iteration verbraucht context-window-Zeit + tendenziell driftet (gleiche Frage wird mehrfach re-litigiert, weil keine sichtbare Konvergenz). Strukturierte Iteration:

1. Macht Konvergenz beobachtbar (jede Phase hat definierten Output)
2. Erlaubt parallele Arbeit ohne context-collision (verschiedene Agents bearbeiten verschiedene Dokumente)
3. Schafft re-usable Methodik (gleicher Flow für jedes künftige Großthema)
4. Entscheidung-Logs sind explizit (decisions.log statt verloren in chat history)

---

## Phase 1 — Foundation Documents (parallel, ~1-2 Stunden)

Sechs Dokumente die parallel von verschiedenen Subagents geschrieben werden. Jedes hat einen klar definierten Scope; keine Überschneidungen.

| # | Dokument | Path | Scope | Owner-Rolle |
|---|---|---|---|---|
| A | Strategic Positioning Vision | `.handoff/v2-vision-strategic-positioning.md` | "Was ist Atlas, warum jetzt, für wen, gegen wen, warum gewinnen wir." EU AI Act / Agent Identities / Accountability / Beyond-Storage. | Product / Strategy agent |
| B | Technical Architecture Vision | `.handoff/v2-vision-knowledge-graph-layer.md` (✅ existiert v0) | Graph-DB Layer, projector, FalkorDB, Graphiti, MCP integration, read-side API. | Architect agent |
| C | Risk Matrix | `.handoff/v2-risk-matrix.md` | 8-12 Risiken kategorisiert: Probability × Impact × Mitigation × Owner. Inkl. Post-Quantum, GDPR-Konflikt, Adoption-Tipping-Point, Vendor-Capture. | Security + Strategy agent |
| D | Competitive Landscape | `.handoff/v2-competitive-landscape.md` | Mem0 / Letta / Graphiti / Zep / Anthropic-Memory / OpenAI-Memory / Mem0-L3 / Obsidian / Notion. Features × Pricing × Trust-Property × Atlas-Differentiator. | Business agent |
| E | Demo Sketches | `.handoff/v2-demo-sketches.md` | 3-5 konkrete Demo-Szenarien für Landing-Page-Hero. Multi-Agent-Collaboration / Continuous-Audit-Mode / Agent-Passport / Mem0-Hybrid-Retrieval / Verifiable-Lineage. Jeder mit 30-90s Video-Storyboard. | Product + Marketing agent |
| F | Security Expert Outreach Pipeline | `.handoff/v2-security-expert-candidates.md` | Kuratierte Liste: Trail of Bits / Cure53 / Least Authority / OSTIF / individuelle Berater (Filippo Valsorda, Frederic Jacobs, et al.). Pro Kandidat: Expertise, Track Record, Engagement-Modell, Cost-Range, EU-Compliance-Eignung. | Security agent |

**Convergence criterion for Phase 1:** Jedes Doc existiert als v0, ist intern konsistent, hat klare offene-Fragen-Section. Keine Cross-Konsistenz erforderlich (das ist Phase-3-Job).

---

## Phase 2 — Multi-angle Critique (parallel, ~1 Stunde)

Sechs Subagents lesen alle sechs Phase-1-Dokumente. Jeder produziert eine Critique aus seiner Rolle. **Crits sind +/-, nicht "yes/no"** — sie listen Stärken, Probleme, blinde-Flecken, Risiken, missing-considerations.

| # | Agent-Rolle | Crit-Focus | Output |
|---|---|---|---|
| 1 | Architect agent | Doc B (Tech) + Doc D (Competitive) — technical feasibility, scalability, projector-determinism, multi-tenant isolation | `.handoff/crit-architect.md` |
| 2 | Security reviewer agent | Doc B + Doc C (Risk) — trust invariant integrity, key management, replay attacks, post-quantum migration, GDPR conflict | `.handoff/crit-security.md` |
| 3 | Database / performance agent | Doc B + Doc D — FalkorDB vs Neo4j vs Kuzu, performance vs Mem0, projection-rebuild-cost at scale, index strategy | `.handoff/crit-database.md` |
| 4 | Product / UX agent | Doc A (Strategy) + Doc E (Demos) — positioning coherence, user-journey realism, demo-conversion-likelihood, Obsidian-comparison-fairness | `.handoff/crit-product.md` |
| 5 | Compliance / regulatory agent | Doc A + Doc C — EU AI Act Art. 12-19 mapping accuracy, AI-Liability-Directive readiness, agent-identity / DID compatibility, jurisdictional scope | `.handoff/crit-compliance.md` |
| 6 | Business / investor agent | Doc A + Doc D + Doc E — market sizing, competitive moat, monetization paths, fundraising readiness, partnership candidates | `.handoff/crit-business.md` |

**Crit-Format-Template** (alle 6 crits folgen dem):

```
# Crit: <agent-role> on Atlas V2 Vision

## Stärken (was ist gut, sollte bleiben)
- Punkt 1
- Punkt 2

## Probleme (was muss adressiert werden — by severity)
### CRITICAL
- ...
### HIGH
- ...
### MEDIUM
- ...
### LOW
- ...

## Blinde Flecken (was wird in den docs gar nicht angesprochen)
- Punkt 1
- ...

## Konkrete Vorschläge (specific edits/additions)
- Vision Doc A §X: change "..." to "..."
- Vision Doc B §Y: add section about ...

## Offene Fragen für Phase 3
- ...
```

**Convergence criterion for Phase 2:** Alle 6 crits geliefert, jede mit mindestens 5 strukturellen Punkten + 3 konkreten Edits. Crits müssen *adressieren* sein, nicht nur "looks good".

---

## Phase 3 — Synthesis & Convergence (~30-60 min, semi-manual)

Wir lesen alle 6 crits gemeinsam und treffen drei Klassen von Entscheidungen:

1. **Crit-Punkte die wir akzeptieren** → direkt in Vision-Docs eingepflegt
2. **Crit-Punkte mit Konflikt** zwischen verschiedenen Agents → du entscheidest (mit meinen Trade-off-Analysen)
3. **Crit-Punkte die wir explizit verwerfen** → loggen in `.handoff/decisions.log` mit Begründung

Output: `.handoff/v2-master-vision-v1.md` (versioned, single coherent document — merged from Doc A + B + C + D + E + F mit alle akzeptierten Crit-Edits).

**Decision Log Format:**
```
## 2026-05-XX: <Topic>
- Crit source: <agent-role>
- Recommendation: <what they proposed>
- Decision: ACCEPT / REJECT / MODIFY-as-follows
- Rationale: <why>
- Reversibility: HIGH / MEDIUM / LOW
- Review-after: <date or trigger>
```

**Convergence criterion for Phase 3:** Master-Vision exists, alle CRITICAL und HIGH crits sind addressed, decisions.log enthält ≥10 explicite Entscheidungen.

---

## Phase 4 — Master Plan + Working Methodology

Zwei Output-Dokumente:

### `docs/V2-MASTER-PLAN.md`

Der Strategieplan als source-of-truth:
- §1 Vision (verdichtet aus Master-Vision)
- §2 Wettbewerbs-Positionierung (verdichtet aus Doc D)
- §3 Risiko-Matrix (verdichtet aus Doc C)
- §4 Demo-Roadmap (verdichtet aus Doc E)
- §5 Security-Engagement-Pipeline (verdichtet aus Doc F)
- §6 Technische Architektur Roadmap (verdichtet aus Doc B)
- §7 V2 Welle-Decomposition (V2-α / V2-β / V2-γ / V2-δ — siehe `v2-vision-knowledge-graph-layer.md` §4)
- §8 Success-Kriterien (was heißt "V2 erfolgreich")

### `docs/WORKING-METHODOLOGY.md`

Wiederverwendbare Methodik für alle künftigen Atlas-Großthemen:
- §1 Vision-First-Pattern (Phase-1-6-Doc-Setup)
- §2 Multi-Angle-Critique (Phase-2-6-Agent-Roles)
- §3 Synthesis-Convergence (Phase-3-Decision-Logging)
- §4 Plan-Documentation (Phase-4-Outputs)
- §5 Welle-Decomposition (wie wir vom Plan auf konkrete Sprints kommen)
- §6 Decision Log Discipline
- §7 Versioning (wenn wir das Framework selbst evolvieren)

**Convergence criterion for Phase 4:** Beide Dokumente reviewed by Nelson, gemerged auf master, Welle 14b/c/d roadmap im Handoff doc reflectiert sie.

---

## Was nach Phase 4 passiert

Welle-Decomposition wird zum **mechanischen Prozess**:

```
Großthema X (e.g. "post-quantum migration") auftaucht
  ↓
Phase 1: 1-3 Foundation-Docs (Scope von X dependent)
  ↓
Phase 2: 2-4 relevante Agent-Crits (security + architect für post-quantum, z.B.)
  ↓
Phase 3: Synthesis (~15-30 min)
  ↓
Phase 4: Mini-Plan-Doc (.handoff/welle-14d-post-quantum-plan.md)
  ↓
Welle-Implementation per Standing Protocol
```

Atlas's Standing Protocol bleibt unverändert (implement → parallel review → fix CRITICAL/HIGH → single coherent commit → docs PR). Die Iterations-Methodik ist *vor* der Implementation — sie sorgt dafür dass wir das richtige bauen, nicht *wie* wir bauen.

---

## Risiken dieses Frameworks selbst

Ehrlich, weil sonst ist das Self-Pitch:

1. **Over-engineering vs. action.** 6 Foundation-Docs + 6 Crits + Synthesis = real time-investment. Wenn die Frage trivial ist, ist dieser Flow Overkill. → **Mitigation:** Framework anwenden nur für Strategie-Wendepunkte (V2, Major-Repositionierungs, Pivot). Für Welle-1-bis-N-Implementation = normal Standing Protocol.

2. **Agent-Crit-Echo-Chamber.** Wenn alle Crit-Agents auf denselben Phase-1-Docs basieren, könnten ihre Crits korreliert sein (sie missen alle die gleichen blind spots). → **Mitigation:** Phase 2 includes ≥1 "outsider review" — entweder externe(r) Security-Expert ODER ein Subagent der explizit als devil's-advocate operates (z.B. "what if Atlas is wrong about market timing — write the strongest case against").

3. **Decision Log Verfall.** Logs nur valuable wenn referenced. → **Mitigation:** Standing protocol mention "before any reversal, check decisions.log entry; if reversing, append new entry with old-decision-ref".

4. **Methodik wird selbst eingefroren.** Wenn wir niemals die Methodik selbst critique'n, kalkifiziert sie. → **Mitigation:** Jährlicher Methodik-Review (oder nach 3 abgeschlossenen Großthemen — was kommt zuerst).

---

## Was ich von dir brauche um Phase 1 zu starten

Drei Klärungen:

**(a) Mem0 vs. anderes Memory-System.** Hast du Mem0 gemeint oder ein anderes System (Letta / Memlane / Hippocampus / etc.)? Doc D + Doc E hängen daran weil Atlas-+-Mem0-Hybrid-Pattern ist ein potentielles Demo + Competitive-Section.

**(b) Hermes-4 als reference-demo-agent (Phase 1 Doc E).** Ist das deine bevorzugte Wahl, oder soll Doc E mehrere Demo-Agents skizzieren und du entscheidest in Phase 3?

**(c) Security-Expert-Budget-Range.** Phase 1 Doc F enthält Cost-Estimates pro Engagement. Sagt Atlas-Welle-14d-Scope Größenordnung €5K / €25K / €100K+ ? Das ändert massiv welche Firmen relevant sind. Du kannst auch sagen "noch keine Budget-Range, list both extremes" — dann strukturieren wir das in Doc F.

Sobald du (a), (b), (c) hast, kann ich Phase 1 als **6 parallele Subagents** starten. Realistic timing: 1-2 Stunden bis alle 6 Foundation-Docs v0 vorliegen.
