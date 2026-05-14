# Atlas — Counsel Engagement Scope of Work

> **Date:** 2026-05-14
> **Project phase:** Atlas `v2.0.0-alpha.2` LIVE on npm; `v2.0.0-alpha.3` candidate consolidating on master; V2-β public materials targeted post-counsel-sign-off.
> **Budget envelope:** €30,000 – €80,000.
> **Engagement length:** 6 – 8 weeks from engagement-letter signature to final deliverable handover.
> **Primary contact (Atlas side):** Nelson Mehlis — nelson@ultranova.io.
> **Document status:** RFP-ready. May be sent to candidate firms after light personalisation in Section 4.

**Why this engagement, why now.** Atlas's V2-α schema (Layer-1 `events.jsonl` content-hash format; Layer-2 graph projection stamping `{event_uuid, rekor_log_index, author_did}` on every node and edge) is committed to master and is part of a published npm release with Sigstore SLSA Build L3 provenance. The reversibility of any post-hoc redesign — in particular a Path-A salt-redesign for GDPR Art. 17 erasure — is therefore **LOW**: a redesign would require a coordinated event-schema migration plus full re-projection of every customer workspace. The original counsel-engagement target was pre-V2-α; that gate has slipped, and the engagement is now pre-V2-β-public-materials blocking. The 6-week clock that starts at engagement-letter signature is therefore on Atlas's critical path for marketing, EU customer onboarding, and the regulator-witness federation positioning brief that V2-γ depends on.

---

## 1. Engagement Übersicht

**Atlas in one paragraph.** Atlas is a verifiable-knowledge-graph backend for AI agents. Every fact written by an agent is signed (Ed25519 + COSE_Sign1 over a canonical CBOR signing-input, RFC 8949 §4.2.1), hash-chained, anchored to the Sigstore Rekor public transparency log via RFC 6962 Merkle inclusion proofs, and independently verifiable via an Apache-2.0 offline WASM verifier the regulator runs in their own browser. Atlas V1 (live on npm since 2026-05-12 as `@atlas-trust/verify-wasm@1.0.1`) provides the write-side trust property. V2 adds a deterministically-rebuildable graph projection layer (Layer 2) and a fast-retrieval cache (Layer 3) without compromising the Layer-1 trust invariant.

**What is committed today.** V1.0.1 + SLSA Build L3 provenance + Sigstore Rekor anchoring + tag-signing enforcement via in-repo trust root (`.github/allowed_signers`); V2-α Layer-2 graph projection schema (Welle 1, 2026-05-12) including the stamping of `event_uuid`, `rekor_log_index`, and `author_did` on every Layer-2 node and edge; byte-deterministic CBOR canonicalisation pinned to a CI-gated hex digest (`8962c168…013ac4`); ArcadeDB Apache-2.0 selected as the Layer-2 graph database (server mode); `GraphStateBackend` Rust trait abstraction landed (Welle 17a) enabling backend-swap without affecting the Layer-1 trust property.

**Engagement budget and scope.** €30,000 – €80,000 total for a structured 6-8 week engagement covering seven discrete deliverables (Section 2). Pre-V2-β public-materials blocking per DECISION-COUNSEL-1; pre-V2-α was the original blocking target, but the V2-α schema commitment has already happened and the resulting opinion is now non-reversible without significant migration cost. Atlas prefers fixed-fee-per-deliverable contracting where feasible; hourly-rate fallback acceptable with capped hours per deliverable.

**Source references for this document.**
- `.handoff/v2-master-vision-v1.md` §11 (Counsel Engagement Plan); §4 (EU AI Act + GDPR + AILD/PLD analysis); §12.1 (Q-3-1 … Q-3-4 counsel-required questions).
- `.handoff/decisions.md` `DECISION-COUNSEL-MASTER` (2026-05-12); `DECISION-COMPLIANCE-2` (verbatim Art. 12); `DECISION-COMPLIANCE-3` (GDPR Art. 17 Path B with Path A fallback); `DECISION-COMPLIANCE-4` (regulator-witness federation reframing).
- `docs/V2-MASTER-PLAN.md` §10 Success Criterion #2: *Counsel-validated EU posture: GDPR Art. 4(1) hash-as-PII opinion delivered; Art. 12 + Annex IV verbatim mapping reviewed; PLD 2024/2853 disclosure-burden framing reviewed; Schrems II SCC templates in place.*

---

## 2. Scope of Work — 7 Deliverables

The seven deliverables below are derived from `.handoff/v2-master-vision-v1.md` §11 and are individually traceable to `DECISION-COUNSEL-1` through `DECISION-COUNSEL-7` in `.handoff/decisions.md`. SOW-1 is the primary deliverable; SOW-2 through SOW-7 are sequenceable within the 6-8 week window. Atlas accepts that a firm may propose a different sequencing if the legal dependencies argue for it (for example, Schrems II SCC drafting in SOW-4 may depend on the GDPR characterisation in SOW-1).

### 2.1 SOW-1 — GDPR Art. 4(1) Hash-as-Personal-Data Opinion (PRIMARY)
`DECISION-COUNSEL-1` · Reversibility = LOW · Schema commitment already happened

**Problem statement.** Atlas's `events.jsonl` records the BLAKE3 content-hash of every signed fact and an Ed25519 signature attesting that an identified author (via `author_did`) created the fact at a given Rekor-anchored timestamp. Under a permissive reading of GDPR Art. 4(1), these hashes — being collision-resistant digests of arbitrary plaintext bytes without a controller-destroyed salt — are not personal data once the underlying content is deleted. Under the strict reading favoured by EDPB Guidelines 4/2019, CJEU C-582/14 *Breyer*, WP29 Opinion 4/2007, and Hamburg DPA precedent, the same hashes are pseudonymous and therefore remain personal data, and the `{author_did, timestamp, workspace_id}` triple is independently personal data when the agent acts on behalf of an identifiable natural person. Atlas's V2-α Layer-2 graph projection additionally stamps `{event_uuid, rekor_log_index, author_did}` on every node and edge — this is a distinct Layer-2 erasure surface that an Art. 17 request must address in addition to Layer-1 hash handling.

**Deliverable.** A written legal opinion that (a) characterises the GDPR-Art.-4(1) status of Atlas's content-hash and `author_did` fields under at least the German BfDI, Irish DPC, and French CNIL strict-reading scrutiny, (b) issues a Path-B defence ruling or a Path-A redesign requirement, (c) explicitly addresses Layer-2 graph-property erasure as a distinct operation from Layer-1 content-hash handling, (d) includes a fallback Art. 17 erasure procedure compatible with Atlas's append-only event log (tombstone events, salt destruction, embedding secure-deletion in Layer 3).

**Inputs Atlas provides.** V2-α `events.jsonl` schema specification including `author_did` field; Layer-2 stamping schema (`event_uuid`, `rekor_log_index`, `author_did` on every node and edge); Master Vision §4.4 (GDPR analysis); the existing engineering analysis under Risk Matrix R-L-01; the V2-α Welle 1 design doc; full repository read access under NDA.

**Acceptance criteria for Atlas.** Atlas can defensibly onboard an EU customer with a PII workspace under the opinion's terms; Atlas can publish marketing claims about GDPR posture without exposure to the kinds of factual-error rebuttal that Phase-2 Compliance review identified.

**Estimated effort.** 3-4 weeks of the 6-8 week engagement (drafting + research + jurisdictional cross-check + Atlas review cycle).

**Cross-reference.** `.handoff/decisions.md` `DECISION-COMPLIANCE-3 / DECISION-COUNSEL-1` (2026-05-12); `.handoff/v2-master-vision-v1.md` §4.4.

### 2.2 SOW-2 — AILD→PLD Reframe + Insurance-Pricing Engagement Strategy
`DECISION-COUNSEL-2` · Reversibility = HIGH (positioning only)

**Problem statement.** The European Commission withdrew the AI Liability Directive proposal in February 2025 (Commission Work Programme 2025, COM(2025) 45 final). Atlas's Phase-1 Doc A §4.2 marketed AI-Liability-Insurance positioning under the assumption AILD was forthcoming; the actual fallback regime is the revised Product Liability Directive (Directive (EU) 2024/2853, in force 2024-12-08), Art. 9 of which shifts the disclosure burden when claimants cannot prove a defect.

**Deliverable.** A memo reframing Atlas's liability-insurance positioning under PLD 2024/2853 instead of the withdrawn AILD, with specific guidance on (a) whether Atlas's `events.jsonl` can be characterised as PLD-Art.-9-disclosure substance, (b) how to engage Solvency II (Directive 2009/138/EC) ORSA-aware insurance-counterparty conversations, (c) how the EIOPA June 2024 AI Opinion principles bear on the pitch.

**Inputs Atlas provides.** Master Vision §4.3; existing Phase-1 Doc A §4.2 marketing copy; the deferred-to-V2-γ rationale per `DECISION-BIZ-5`.

**Acceptance criteria for Atlas.** Atlas can re-engage Munich Re HSB or equivalent counterparty under accurate legal framing; the V2-γ AI-Liability-Insurance pitch is not based on a withdrawn Directive.

**Estimated effort.** 1 week.

**Cross-reference.** `.handoff/decisions.md` `DECISION-COMPLIANCE-1`; `.handoff/v2-master-vision-v1.md` §4.3.

### 2.3 SOW-3 — Art. 43 Conformity-Assessment-Substitution Liability Disclaimer
`DECISION-COUNSEL-3` · Reversibility = LOW once endpoints ship publicly

**Problem statement.** Atlas provides structural evidence that AI deployments can use to satisfy EU AI Act Art. 12 record-keeping and related obligations. Atlas does not perform conformity assessment under Art. 43 (Annex VI/VII) and does not assume any portion of the provider's conformity-assessment obligation. Marketing surfaces and the customer-facing documentation must make this boundary unambiguous; Risk Matrix R-L-04 identifies the substitution-liability scenario as a Phase-2-Compliance-flagged concern.

**Deliverable.** Drafted disclaimer language for (a) the Atlas landing page footer, (b) the customer-facing documentation header, (c) the engagement-contract template, plus a memo explaining the legal rationale and the limits of the disclaimer under EU consumer-protection and B2B contract law.

**Inputs Atlas provides.** Current marketing copy; the existing engagement contract template; Master Vision §4.2 (Art. 12 / Art. 43 boundary).

**Acceptance criteria for Atlas.** Atlas can publish marketing claims about Art. 12 substrate fit without exposure to a claim that Atlas has assumed the customer's Art. 43 conformity-assessment obligation.

**Estimated effort.** 1 week.

**Cross-reference.** `.handoff/v2-master-vision-v1.md` §4.2 + §11 (Risk Matrix R-L-04).

### 2.4 SOW-4 — Schrems II Cross-Border SCC + DPA Templates
`DECISION-COUNSEL-4` · Reversibility = MEDIUM

**Problem statement.** Atlas's hosted-service tier will process customer event-data on infrastructure that may include US-based components (Sigstore Rekor is operated from US-based infrastructure today; npm registry is US-based). Risk Matrix R-L-03 (Schrems II / cross-border data transfer) identifies the legal requirement for Standard Contractual Clauses and Data Processing Agreements that survive the CJEU Schrems II ruling and any pending Schrems III developments.

**Deliverable.** Two templates: (a) Standard Contractual Clauses adapted for Atlas's data-processing posture, including the Sigstore Rekor transparency-log inclusion-proof workflow as an explicit characterised processing activity; (b) Data Processing Agreement template for Atlas customers, including the Atlas-as-processor or Atlas-as-controller characterisation Atlas should adopt by tier. Plus a short memo explaining when each template applies and what Atlas's customer-onboarding workflow should require.

**Inputs Atlas provides.** Hosted-service architecture overview; customer-data-flow diagram; the Sigstore Rekor anchoring workflow.

**Acceptance criteria for Atlas.** Atlas can sign EU customers under documented cross-border transfer mechanisms without case-by-case legal review.

**Estimated effort.** 2 weeks.

**Cross-reference.** `.handoff/v2-master-vision-v1.md` §11 (Risk Matrix R-L-03).

### 2.5 SOW-5 — Verbatim Art. 12 + Annex IV §1(g) + §2(g) Marketing-Copy Review
`DECISION-COUNSEL-5` · Reversibility = HIGH

**Problem statement.** Atlas's Phase-1 Doc A §3.1 paraphrased EU AI Act Art. 12 as requiring records "independently verifiable by regulators." That paraphrase is not the verbatim Regulation text. Per `DECISION-COMPLIANCE-2`, Atlas committed to using the verbatim Art. 12 §1 + §2 text (Regulation (EU) 2024/1689) plus Annex IV §1(g) and §2(g) cross-references, with the design-claim "Atlas's offline WASM verifier exceeds what the Regulation requires." All five Phase-1 documents propagated the paraphrase and need search-replace; the README.md fix has already been applied in this same PR ahead of counsel review.

**Deliverable.** A written counsel review of (a) the verbatim Art. 12 + Annex IV §1(g) + §2(g) text Atlas uses in `docs/COMPLIANCE-MAPPING.md` and `README.md`, (b) Atlas's "exceeds the minimum" design-claim phrasing for marketing copy, (c) Atlas's avoidance of "EU AI Act compliant" language (reserved for Art. 43 conformity-assessment determinations only), (d) the exact paragraph-number citations in Master Vision §4.2 (the Article paragraph-number attributions there are pre-counsel-review).

**Inputs Atlas provides.** `docs/COMPLIANCE-MAPPING.md` (current state); `README.md` post-fix; Master Vision §4.1 + §4.2; the search-replace inventory across all marketing surfaces.

**Acceptance criteria for Atlas.** Atlas's marketing copy survives a sophisticated competitor or regulator fact-check; the verbatim Art. 12 quote is unambiguously correct; the "exceeds the minimum" phrasing is defensible.

**Estimated effort.** 1 week.

**Cross-reference.** `.handoff/decisions.md` `DECISION-COMPLIANCE-2` (2026-05-12); `.handoff/v2-master-vision-v1.md` §4.1.

### 2.6 SOW-6 — Witness-Federation EU Regulatory Positioning Brief
`DECISION-COUNSEL-6` · Reversibility = HIGH

**Problem statement.** Atlas's V2-γ design includes a regulator-witness federation: out-of-process independent witnesses (one per regulatory authority) sign chain heads alongside Atlas's operator witness. No EU supervisory authority has publicly endorsed the cryptographic-cosignature pattern. Phase-2 Compliance C-3 reframing replaced "regulator-approved pattern" marketing with "regulator-friendly architecture" and committed to pursuing actual supervisor-sandbox engagement before any V2-γ "regulator-piloted" claim.

**Deliverable.** A brief identifying (a) the existing EU supervisory patterns closest to Atlas's witness federation (§44 KWG on-site supervision, GDPR Art. 27 representative, eIDAS QTSPs, EBA outsourcing guidelines, FDA 21 CFR Part 11 §11.10(e)), (b) the regulatory positioning Atlas should adopt when approaching BaFin AI Office, De Nederlandsche Bank + Dutch AP, CNIL "bac à sable", and BoE AI Public-Private Forum, (c) the language Atlas should use to describe witness federation in V2-γ marketing materials before any supervisor pilot has launched.

**Inputs Atlas provides.** Master Vision §4.5 + §5.6 (witness federation architecture); the V1.13 witness cosignature spec; the proposed V2-γ federation enrolment event schema.

**Acceptance criteria for Atlas.** Atlas can approach a supervisory authority with a clear, accurate positioning that does not over-claim; the V2-γ marketing materials accurately describe the witness federation design.

**Estimated effort.** 1-2 weeks.

**Cross-reference.** `.handoff/decisions.md` `DECISION-COMPLIANCE-4` (2026-05-12); `.handoff/v2-master-vision-v1.md` §4.5.

### 2.7 SOW-7 — DPIA + FRIA Template Drafting
`DECISION-COUNSEL-7` · Reversibility = HIGH

**Problem statement.** EU AI Act Art. 27 requires Fundamental Rights Impact Assessment for certain high-risk deployer scenarios; GDPR Art. 35 requires Data Protection Impact Assessment for high-risk processing. Atlas's customers will need to complete both for any deployment that uses Atlas as a substrate. Atlas can ship template starter packs as high-value marketing collateral.

**Deliverable.** A DPIA template and a FRIA template, both pre-populated with the Atlas-specific characterisations a customer would otherwise have to derive themselves (data flows, processing purposes, risk-mitigation evidence drawn from Atlas's structural properties). Plus a one-page guide on how a customer integrates the templates into their own compliance documentation.

**Inputs Atlas provides.** Atlas data-flow diagram; the three-layer architecture description; the existing GAMP 5 ALCOA+ mapping in `docs/COMPLIANCE-MAPPING.md`.

**Acceptance criteria for Atlas.** Atlas can offer DPIA + FRIA template downloads as part of the V2-β marketing surface; the templates accelerate customer onboarding by reducing the customer's compliance-documentation drafting time.

**Estimated effort.** 1-2 weeks.

**Cross-reference.** `.handoff/v2-master-vision-v1.md` §11 (Phase 2 Compliance Blind Spot 3).

---

## 3. Firmenvergleichs-Matrix

| Firma | Sitz | Praxis-Schwerpunkt | Erstkontakt-Datum | Antwortzeit | Hourly-Rate-Range | Fixed-Fee-Option SOW-1 | AI-Act-Track-Record | Use-Case-Fit (1-5) | Verfügbarkeit ab |
|---|---|---|---|---|---|---|---|---|---|
| Hogan Lovells | Frankfurt, DE | EU AI Act + GDPR + multi-jurisdictional tech regulation | | | €550 – €750 | likely available | Multiple published guidance notes on AI Act since 2024; active client work on Art. 6 + Art. 12 obligations | | |
| Bird & Bird | München, DE | Tech sector + AI Act early-mover; strong GDPR base | | | €500 – €700 | likely available | Bird & Bird "AI Hub" public-facing resource; multiple Art. 12 + Annex IV client opinions in 2025 | | |
| Hengeler Mueller | Frankfurt / Düsseldorf, DE | Broad German tier-1 practice; AI Act work building but less specialised than Bird & Bird or Hogan Lovells | | | €600 – €800 | uncertain | Less public AI Act output; strong on GDPR and German-specific BfDI scrutiny | | |
| Matheson | Dublin, IE | GDPR leader (Irish DPC proximity); EU AI Act practice growing | | | €450 – €650 | likely available | Strong on GDPR-Art.-4(1) hash-as-PII territory; CJEU *Breyer* literature contributions | | |
| William Fry | Dublin, IE | EU AI Act early-mover practice; GDPR base | | | €450 – €600 | likely available | Published AI Act guidance 2024-2025; client work on Annex IV documentation | | |
| Cleary Gottlieb | Paris, FR | Multi-jurisdictional, transatlantic-strong; CNIL proximity | | | €700 – €950 | uncertain | Cross-border data-transfer leader; Schrems II SCC drafting experience | | |
| Taylor Wessing | München / London | Boutique AI-Act-specialised; tech sector deep | | | €500 – €700 | likely available | Tech-sector early specialisation; multiple Art. 12 client opinions; SLSA / supply-chain awareness | | |

**Decision criteria (how to weight cells).** Prioritise three factors over hourly rate alone. **First**, SOW-1 AI-Act-specific track record (published guidance, known client opinions on Art. 4(1) hash-as-PII, evidence the firm has thought about pseudonymisation under the strict reading). **Second**, fixed-fee-per-deliverable availability for SOW-1 (Atlas prefers predictability over hourly-cap exposure on the primary deliverable). **Third**, 6-8 week availability starting at engagement-letter signature — a firm that is great but starts in 12 weeks does not unblock V2-β public materials.

After those three, hourly rate matters for the smaller SOWs (especially SOW-2 and SOW-3 at ~1 week each), and the Use-Case-Fit score (1-5, Nelson fills after first contact) is the final tie-breaker. Boutique AI-Act-specialised firms (Taylor Wessing notably) may outperform full-service tier-1s on the specific deliverable mix Atlas needs, and Atlas should not over-anchor on brand-recognition over deliverable-specific fit.

---

## 4. Outreach-Email-Templates

### 4.1 DE-Template (für Hogan Lovells, Bird & Bird, Hengeler Mueller)

**Subject:** Atlas — EU-AI-Act + GDPR Counsel-Engagement (€30-80K, 6-8 Wochen, SOW beiliegend)

Sehr geehrte Damen und Herren,

ich leite Atlas, ein in Deutschland gegründetes Engineering-Projekt, das eine kryptographisch verifizierbare Wissensgraph-Infrastruktur für KI-Agenten bereitstellt. Atlas V1.0.1 ist seit dem 12. Mai 2026 auf npm live (`@atlas-trust/verify-wasm`), inklusive SLSA Build L3 Provenance über Sigstore Rekor. Unsere V2-α-Schicht (Layer-2-Graph-Projektion mit deterministischer Byte-Pinnung) ist auf master committet; V2-β bereitet öffentliche Marketing-Materialien vor, deren Veröffentlichung wir bis zu einer abgeschlossenen Counsel-Review zurückhalten.

Wir suchen eine Kanzlei für ein strukturiertes 6-8-Wochen-Engagement im Rahmen von €30,000 – €80,000. Das primäre Deliverable (SOW-1) ist eine Rechtsmeinung zur Einordnung unserer BLAKE3-Content-Hashes und `author_did`-Felder unter GDPR Art. 4(1) — eine Frage, deren strikte Lesart (EDPB-Leitlinien 4/2019, CJEU C-582/14 *Breyer*, WP29-Opinion 4/2007) die Klassifikation als pseudonyme personenbezogene Daten begünstigt. Unsere V2-α-Schema-Commitment macht diese Frage zeitkritisch: das Pfad-A-Redesign (Salt-basierte Hashes mit kontroller-zerstörbarem Salt) wäre nach dem Schema-Lock migrationsintensiv; eine Pfad-B-Verteidigung muss die strikte Lesart aushalten.

Die sieben Deliverables im Überblick:

1. **SOW-1 (PRIMÄR):** GDPR Art. 4(1) Hash-as-Personal-Data Rechtsmeinung — Pfad-A vs. Pfad-B Entscheidung, inkl. Layer-2-Graph-Property-Erasure-Behandlung.
2. **SOW-2:** AILD→PLD-Reframing (Directive (EU) 2024/2853) plus Strategie für Insurance-Counterparty-Gespräche unter Solvency II + EIOPA AI Opinion.
3. **SOW-3:** Art. 43 Conformity-Assessment-Substitution Haftungsdisclaimer für Marketing-Surfaces + Kundenverträge.
4. **SOW-4:** Schrems II Standardvertragsklauseln + Auftragsverarbeitungsvertrag-Templates für Atlas's Hosted-Service-Tier.
5. **SOW-5:** Wortgetreue Art. 12 + Annex IV §1(g) + §2(g) Marketing-Copy-Review.
6. **SOW-6:** Witness-Federation EU-Regulatory-Positioning-Brief — Identifikation des nächsten existierenden Aufsichtsmusters für unsere kryptographische Co-Signatur-Architektur.
7. **SOW-7:** DPIA + FRIA Template-Drafting für Atlas-Kunden.

Wir budgetieren €30,000 – €80,000 für das gesamte Initial-Engagement und bevorzugen, wo möglich, Fixed-Fee-per-Deliverable über Stundenabrechnung. Die Engagement-Dauer ist 6-8 Wochen ab Unterzeichnung des Engagement-Letters. Auf Wunsch teile ich unter NDA das Master Vision Dokument (Phase-3-Synthese, 22 dokumentierte Entscheidungen) sowie Repository-Zugang zur Due Diligence.

Hätten Sie Kapazität für ein 30-minütiges Erstgespräch innerhalb der nächsten zwei Wochen? Ich freue mich auf Ihre Rückmeldung.

Mit freundlichen Grüßen
Nelson Mehlis
Founder, Atlas
nelson@ultranova.io

### 4.2 EN-Template (for Matheson, William Fry, Cleary Gottlieb, Taylor Wessing)

**Subject:** Atlas — EU AI Act + GDPR counsel engagement (€30-80K, 6-8 weeks, SOW attached)

Dear [Partner Name / Counsel],

I lead Atlas, a Germany-founded engineering project building cryptographically verifiable knowledge-graph infrastructure for AI agents. Atlas V1.0.1 went live on npm on 2026-05-12 (`@atlas-trust/verify-wasm`) with SLSA Build L3 provenance via Sigstore Rekor. Our V2-α Layer-2 graph projection schema (with byte-deterministic canonicalisation pinned to a CI-gated hex digest) is committed on master; V2-β public marketing materials are queued behind counsel review.

We are looking for a firm to engage for a structured 6-8 week relationship within a €30,000 – €80,000 budget envelope. The primary deliverable (SOW-1) is a written legal opinion on the GDPR Art. 4(1) status of Atlas's BLAKE3 content-hashes and `author_did` fields. The strict reading (EDPB Guidelines 4/2019, CJEU C-582/14 *Breyer*, WP29 Opinion 4/2007) favours treating these as pseudonymised personal data; our V2-α schema commitment makes this question time-sensitive because a Path-A salt-redesign post-schema-lock would be migration-heavy, while a Path-B defence must survive strict-reading scrutiny.

The seven deliverables in brief:

1. **SOW-1 (PRIMARY):** GDPR Art. 4(1) hash-as-personal-data opinion — Path A versus Path B decision, including treatment of Layer-2 graph-property erasure as a distinct operation.
2. **SOW-2:** AILD→PLD reframe (Directive (EU) 2024/2853) plus engagement strategy for insurance counterparties under Solvency II + EIOPA AI Opinion.
3. **SOW-3:** Art. 43 conformity-assessment-substitution liability disclaimer drafting for marketing surfaces + customer contracts.
4. **SOW-4:** Schrems II Standard Contractual Clauses + Data Processing Agreement templates for Atlas's hosted-service tier.
5. **SOW-5:** Verbatim Art. 12 + Annex IV §1(g) + §2(g) marketing-copy review.
6. **SOW-6:** Witness-federation EU regulatory positioning brief — identifying the closest existing supervisory pattern for our cryptographic-cosignature architecture.
7. **SOW-7:** DPIA + FRIA template drafting for Atlas customers.

We budget €30,000 – €80,000 for the initial engagement and prefer fixed-fee-per-deliverable contracting where feasible. Engagement duration is 6-8 weeks from engagement-letter signature. Under NDA I can share our Master Vision document (Phase-3 synthesis, 22 documented decisions) and provide repository access for due diligence.

Would you have capacity for a 30-minute initial conversation within the next two weeks? I look forward to your response.

Best regards,
Nelson Mehlis
Founder, Atlas
nelson@ultranova.io

---

## 5. Engagement-Letter Checklist

After firm selection, before kickoff, Atlas requires the following to be in place. This list serves as the pre-kickoff checklist for either side.

- **Mutual NDA executed** before any schema specification or Master Vision document is shared. Atlas's preference is for a balanced mutual NDA; Atlas will supply a template if the firm does not have a preferred one.
- **Scope-of-work attachment** referencing this document or an agreed-redacted version. Each SOW (2.1 through 2.7) is line-itemised with deliverable, inputs, acceptance criteria, and estimated effort.
- **Hourly-rate vs fixed-fee-per-SOW decision** documented per SOW. Atlas's preference is fixed-fee-per-SOW for SOW-1 (the primary deliverable, highest predictability requirement) and SOW-5 (well-bounded scope). Hourly-rate-with-cap acceptable for SOW-2 through SOW-4, SOW-6, SOW-7.
- **Deliverable-by-deliverable timeline** ideally rendered as a Gantt chart with explicit dependency arrows (for example: SOW-4 Schrems II templates may depend on SOW-1 GDPR characterisation outcome).
- **IP and work-product ownership** clearly delineated: Atlas owns the deliverable memos and templates and the right to publish them; counsel retains copyright over its methodology and any reusable forms. Marketing references to the engaging firm are subject to firm pre-approval.
- **Conflict-check completion.** Counsel screens for adversarial conflicts including Mem0 (memory-layer competitor), Anthropic, OpenAI, Mistral, and major US cloud vendors (AWS, Azure, GCP) where their interests could create a conflict with Atlas's positioning work.
- **DPA addendum.** Counsel will process Atlas-side data including the `events.jsonl` schema specification and any customer-list snapshot shared during SOW-2 or SOW-4 work. A separate DPA between Atlas and the firm covers this processing.
- **Termination and handover-on-cause clauses.** Termination-for-convenience with reasonable notice (Atlas's preference: 30 days); handover-on-cause clauses ensuring work-in-progress deliverables are transferable to successor counsel without significant rework.
- **Communication cadence.** Atlas's preference: weekly 30-minute progress call plus async written status updates per deliverable milestone. Alternative: deliverable-based touchpoints if the firm prefers.

---

## 6. Initial-Briefing Question Sheet

The four questions below are transcribed verbatim from `.handoff/v2-master-vision-v1.md` §12.1 (the counsel-required open questions from Atlas's Phase-3 strategic synthesis). Atlas requests that the engaged firm read these immediately after engagement-letter signature and use them as the structure for the SOW-1 opening interview.

- **Q-3-1.** GDPR Art. 4(1) hash-as-PII Path A vs Path B decision — gated on counsel opinion.
- **Q-3-2.** AILD→PLD reframe verbatim language — gated on counsel.
- **Q-3-3.** Insurance-regulation-aware counterparty conversations (Munich Re HSB) — gated on Solvency II + EIOPA AI Opinion mapping work.
- **Q-3-4.** Supervisor-engagement (BaFin / Dutch AP / CNIL / BoE) — independent of GDPR counsel but parallel timeline.

---

## 7. Parallel Supervisor-Engagement Note

In parallel with this counsel engagement (and explicitly **not** part of this scope of work) Atlas pursues a separate-track supervisor-sandbox engagement covering BaFin AI Office (Germany), De Nederlandsche Bank + Dutch AP (Netherlands), CNIL "bac à sable" (France), and BoE AI Public-Private Forum (UK). This track is pre-V2-γ blocking (regulator-witness federation production-deployment claims), runs at a separate cadence, and is funded outside this engagement's €30,000 – €80,000 envelope. The engaged counsel firm should be aware of this parallel track because SOW-6 (witness-federation positioning brief) feeds into the supervisor conversations once they begin; the engaged firm is not asked to lead those conversations, but the brief it produces is the document Atlas takes into the first supervisor meeting.

---

## 8. Source References + Links

All references resolve relative to the Atlas repository root.

- `.handoff/v2-master-vision-v1.md` §4 — EU AI Act + GDPR + AILD/PLD analysis (full substance).
- `.handoff/v2-master-vision-v1.md` §11 — Counsel Engagement Plan (7-deliverable list + firm shortlist).
- `.handoff/v2-master-vision-v1.md` §12.1 — Q-3-1 … Q-3-4 counsel-required questions (verbatim source for Section 6).
- `.handoff/decisions.md` `DECISION-COUNSEL-MASTER` (2026-05-12) — primary decision committing to this engagement; cross-refs `DECISION-COUNSEL-1` through `DECISION-COUNSEL-7`.
- `.handoff/decisions.md` `DECISION-COMPLIANCE-2` (2026-05-12) — verbatim Art. 12 commitment.
- `.handoff/decisions.md` `DECISION-COMPLIANCE-3` (2026-05-12) — GDPR Art. 17 Path B primary with Path A fallback.
- `docs/V2-MASTER-PLAN.md` §10 — Success Criterion #2 (Counsel-validated EU posture).
- `README.md` — post-fix verbatim Art. 12 language (line 28).
- `docs/COMPLIANCE-MAPPING.md` — Atlas's current regulatory-compliance mapping document; carries a counsel-pending disclaimer header until SOW-5 completes.

---

*End of scope-of-work document. Versendefertig nach Personalisierung von Section 4 (Anrede + Firmenname).*
