# Atlas V2 — Master Vision v1

> **Status:** v1 (Phase 3 synthesis complete, 2026-05-12). **Source documents:** Phase 1 (5 Foundation Docs) + Phase 2 (6 Multi-Angle Critiques). **Decisions:** see `.handoff/decisions.md`. **Next phase:** Phase 4 = `docs/V2-MASTER-PLAN.md` + `docs/WORKING-METHODOLOGY.md` lands on master.
>
> **This is the consolidated Atlas V2 strategic vision.** Phase 1 Foundation Docs (`v2-vision-strategic-positioning.md`, `v2-vision-knowledge-graph-layer.md`, `v2-risk-matrix.md`, `v2-competitive-landscape.md`, `v2-demo-sketches.md`) remain as historical-reference inputs. Phase 2 critiques (`crit-architect.md`, `crit-security.md`, `crit-database.md`, `crit-product.md`, `crit-compliance.md`, `crit-business.md`) drove the corrections, additions, and reframings in this doc. This v1 is the single coherent source for Phase 4 plan derivation.

---

## 1. Executive Summary

**Atlas v1.0.1 LIVE auf npm (2026-05-12) mit SLSA Build L3 provenance.** V1 = write-side trust property (Ed25519 + COSE_Sign1 + deterministic CBOR + blake3 hash chain + Sigstore Rekor anchoring + witness cosignature + offline WASM verifier). **V2 pivot = read-side substrate + agent-agnostic shared memory**, ohne V1's Trust-Invariant zu kompromittieren.

**Tagline (Phase 3 lock):** *"Knowledge your AI can prove, not just claim."* Verworfen: TÜV-Plakette-Analogie (Markenrecht-Risiko DACH), "Independently Verifiable" als Standalone-Tagline (nicht in verbatim EU AI Act Art. 12 Text — siehe §4).

**Zwei Märkte, ein Substrat:**
- **Multi-Agent Shared Memory** (AI-Engineer / agent-builder persona) — **PRIMARY MARKET / front door** für Landing-Page + Docs + Pricing
- **Verifiable Second Brain** (knowledge-worker persona) — secondary market, validation via Obsidian-plugin path (1-2 Wochen Investment) statt full-stack Atlas-as-PKM (V2-γ-minimum, deferred)

**Trust-Architecture (Three Layers):**
- **Layer 1 — `events.jsonl`** authoritative (V1, unchanged)
- **Layer 2 — FalkorDB projection** queryable, **deterministisch rebuildbar** (CI-gate `projector-state-hash` per V1's `signing_input_byte_determinism_pin`-Pattern)
- **Layer 3 — Mem0g cache** fast-retrieval, semantisch rebuildbar, **NIE trust-authoritative** (cite back to event_uuid in Layer 1)

**Welle-Decomposition (Phase 2 Re-Baseline):** V2-α 5-8 Sessions (projector + FalkorDB + DID-schema + content-hash-separation + projector-state-hash-gate), V2-β 4-5 (Mem0g + read-API + MCP V2 tools + Explorer UI), V2-γ 3-4 (Agent Passports + Regulator-Witness + Hermes-skill), V2-δ 2-3 (optional Cedar policy + Graphiti retrieval). **Total 14-20 Sessions** (was 10-14 in Phase 1 estimate).

**Counsel-Engagement gate:** €30-80K EU-AI-Act + GDPR specialised firm relationship is **pre-V2-α blocking** for any of: (a) public marketing claiming Art. 12 fit, (b) any EU customer with PII workspace, (c) regulator-witness-federation demo claims. Recommended path: front-load 6-8 week structured engagement before V2-α public materials.

**Three FACTUAL CORRECTIONS from Phase 2 (must land before V2-α GTM):**
1. **EU AI Liability Directive was WITHDRAWN Feb 2025** (Commission Work Programme 2025) — not "expected 2026". Fallback regime is revised **Product Liability Directive (Directive (EU) 2024/2853, in force 2024-12-08)**. AI-Liability-Insurance pitch needs full reframing.
2. **"Independently verifiable"** is Atlas's phrasing, NOT verbatim Art. 12 text. Substance is correct (Atlas's offline-WASM-verifier exceeds Art. 12 minimum), but marketing must use Art. 12 verbatim + design-claim "exceeds the minimum."
3. **Art. 18 (10-year doc retention) is NOT Art. 19 §1 (6-month log retention)** — Phase 1 Doc A conflated. Defaults: log retention 7 years (longest sectoral baseline), deployer-configurable.

**Top-5 V2-α blocking risks (Risk Matrix v1):**
- **R-A-01** Projection Determinism Drift — needs byte-pin canonicalisation + `ProjectorRunAttestation` signed event (per Phase-2 Security Q-SEC-6), not just CI hash gate
- **R-L-01** GDPR Art. 17 hash-as-personal-data — **probability escalated from MEDIUM to HIGH** (per Phase-2 Compliance strict-reading analysis under CJEU *Breyer* + EDPB Guidelines 4/2019). Counsel-required, pre-V2-α blocking
- **R-A-03** Agent Identity Key Compromise + Revocation Lag — needs out-of-band revocation channel (not signed by compromised key) + `signed_at_rekor_inclusion_time` Δ-flagging
- **R-L-02** FalkorDB SSPL — counsel-required pre-hosted-service. Kuzu fallback dead (Apple-archived Oct 2025). **ArcadeDB Apache-2.0** is next viable; comparative spike before V2-α lock
- **R-S-01** Adoption Tipping Point — Hermes-Agent demoted from "GTM Hypothesis 1" to "credibility asset" (60K stars → ~4-36 retained Atlas users steady-state per Phase-2 Business math)

---

## 2. The V1 → V2 Pivot

**Was V1 gelöst hat:** *Strukturelle Compliance-Lücke* — jede Behauptung über AI-Verhalten musste vendor-kontrolliert geprüft werden ("trust me, we logged it"). V1's signed `events.jsonl` + Sigstore Rekor + offline WASM verifier macht jeden Fact strukturell beweisbar von einem unbeteiligten Dritten, ohne dass Atlas-Operator nötig ist. Verifier-Crates Apache-2.0; jeder kann embedden. SLSA Build L3 provenance (`@atlas-trust/verify-wasm@1.0.1`, npm-LIVE).

**Was V2 strukturell aufmacht:**

| Dimension | V1 (live) | V2 (vision) |
|---|---|---|
| Trust direction | Write-side (sign-on-create) | Bidirektional (sign-on-create + verify-on-retrieve) |
| Persona-Spektrum | Compliance officer + AI engineer (Art. 12 mandate) | Plus: Knowledge worker (PKM-Markt), Regulator (witness federation) |
| Agent-Modell | "One agent writes, one auditor reads" | Multi-agent shared memory: any agent (Hermes, Claude, GPT, custom) schreibt in dieselbe Wissensbasis |
| Retrieval | Re-read raw JSONL | Graph traversal (FalkorDB) + semantic search (Mem0g) + provenance-filtered queries |
| Identity | Per-workspace HKDF | Plus: per-agent Ed25519 DID (`did:atlas:<pubkey-hash>`) |
| Compliance posture | "Atlas writes verifiable logs" | "Atlas verifies the system's compliance evidence" (substrate, not the system itself) |

**Was V2 *nicht* tut (explicit out-of-scope):**
- Atlas wird *kein* Agent — agent-agnostisch bleibt. Hermes/Claude/GPT integrieren *via* MCP oder HTTP-API.
- Atlas wird *kein* Conformity-Assessment-Tool (Art. 43 / Annex VI). Customer's Art. 43 obligation bleibt customer's.
- Atlas wird *kein* eIDAS-qualified-trust-service-provider (V2-Phase) — Sigstore Rekor "advanced electronic timestamp", nicht "qualified". eIDAS-Qualified-Mode optional V3+ via QTSP-Partnerschaft.

---

## 3. Two-Market Positioning (with operational decision rule)

### 3.1 Primary market: Multi-Agent Shared Memory
**Persona:** AI engineer / agent builder / ML-Ops / platform team.
**Pain:** "Mein Agent hat behauptet X — kann ich beweisen er hat X getan, nicht halluziniert?" + "Mein Multi-Agent-System hat 5 Agents — wer hat welchen Fact reingeschrieben?" + "Wenn ich Mem0/Letta/Graphiti benutze, ist das cryptographic trustworthy?"
**Distribution-Funnel:** GitHub-discovery → `npx @atlas-trust/quickstart` (30s first signed event) → MCP-skill install → first 10 facts → review-and-verify UI in browser → retention via Hermes-skill OR direct HTTP-API integration in deployed agents
**Headline value:** *"Every fact has a verified author. No more 'the AI said it'."*

### 3.2 Secondary market: Verifiable Second Brain
**Persona:** knowledge worker / researcher / journalist / consultant — Obsidian/Notion power-user.
**Pain:** "Wenn ich KI-Drafts in mein Obsidian-Vault schreibe, gibt's keine Provenance — was war menschen-geschrieben vs AI-generiert?"
**Distribution-Funnel (Phase-2 Product crit recommendation):** **Obsidian-Plugin first** (1-2 Wochen Effort, fast validation) — every-edit-signs + Rekor-anchor + `[verified]` badge in note UI. NOT full Atlas-as-PKM client (V2-γ-minimum, deferred per Phase-2 Demo 4 risk-assessment).
**Headline value:** *"Your Second Brain, attested for the AI era."*
**Status:** Phase 3 decision: **defer Atlas-native PKM client until plugin validates the market**. Plugin development is independent of V2-α/β/γ stack maturation.

### 3.3 Decision Rule — which market is the front door?
**Front door = Multi-Agent Shared Memory** (AI engineer / agent builder persona). Landing-Page Hero + main docs + pricing-page-default + first 10 examples all speak this persona's vocabulary. Reasoning:
- Phase 1 Doc D + Doc E both implicitly weight AI-engineer demos 3:1 over Second-Brain demos
- Phase 2 Business crit math shows AI-memory-infra SAM ~$10-45M is fundable; Personal-tier PKM SOM €600K-3.6M ARR is sub-fundable on its own
- Phase 2 Product crit: "two-market positioning operationally undefended without a decision rule"
- EU AI Act Art. 12 compliance-driver is AI-engineer-side, not knowledge-worker-side

Secondary-market materials (Obsidian-plugin, "for personal users" CTA) live one click off the hero. Two-market story remains true at substrate level; **operational priority is unambiguous**.

---

## 4. EU AI Act + Compliance Reality (corrected from Phase 1)

### 4.1 Art. 12 verbatim mapping
*Phase 1 Doc A used "independently verifiable" as Art. 12 paraphrase — Phase 2 Compliance C-1 flagged this as NOT in verbatim Regulation 2024/1689 text.*

**Verbatim Art. 12 §1:** *"High-risk AI systems shall technically allow for the automatic recording of events ('logs') over the lifetime of the system."*
**Excerpt Art. 12 §2:** logging level *"appropriate to the intended purpose of the system."* (this is an excerpt — the full paragraph also addresses identification of the period of use, reference database, input data, and natural persons involved in verification; full verbatim text gated on `DECISION-COUNSEL-5` counsel review).
**Cross-ref Annex IV §1(g):** *"description of validation and testing procedures used."*
**Cross-ref Annex IV §2(g):** *"outputs and intended interpretation."*
**In-force date:** 2 August 2026 for high-risk systems (Art. 113(b)).
**Note:** all Article paragraph numbers in §4.2 mapping table are pre-counsel-review attributions; verification of exact paragraph-number citations is in scope of `DECISION-COUNSEL-5`.

**Atlas's relationship to Art. 12:**
> Atlas provides cryptographic independent verification of AI event logs — a *structural property* that **exceeds** what the Regulation requires. Where Art. 12 mandates "automatic recording" with traceability "appropriate to the intended purpose", Atlas additionally provides offline mathematical verification of (a) signature integrity, (b) hash chain continuity, (c) Sigstore Rekor inclusion proof, (d) optional witness cosignature thresholds — all of which any independent third party can verify without trusting Atlas-Operator.

**Marketing copy convention (Phase 3 decision):** use *"designed to satisfy"* and *"provides cryptographic verification exceeding the Art. 12 minimum"*. Avoid *"EU AI Act compliant"* (Art. 43 conformity-assessment determination only) and *"independently verifiable"* as Art. 12 paraphrase. Search-replace across all five docs + future marketing surfaces.

### 4.2 EU AI Act expanded mapping (11 articles, not 4)

| Article | Verbatim summary | Atlas mechanism | Status |
|---|---|---|---|
| **Art. 9** Risk management system | continuous risk-management process | `events.jsonl` records risk-assessment decisions | Substrate-relevant (Atlas provides evidence; provider responsible for system) |
| **Art. 12** Records (logs) | automatic recording of events | Signed events.jsonl + Rekor + WASM verifier | **PRIMARY anchor, structural fit** |
| **Art. 13** Transparency to deployers | sufficient transparency to interpret outputs | V2 Read-API + provenance bundle endpoint | **Contributes to**, does not deliver alone (Art. 13 covers static training-data-characteristics docs that Atlas does not provide) |
| **Art. 14** Human oversight | overseeable by natural persons | Cedar policy enforcement at write-time (V2-δ candidate) | Substrate for §3(c) intervention/override; provider remains responsible for §3(a)+(b) |
| **Art. 15 §5** Cybersecurity (resilience to alteration) | resilient to *"attempts by unauthorised third parties to alter their use, outputs or performance"* | **Direct structural match** — tamper-evident hash chain + offline verifier | **PRIMARY anchor #2** |
| **Art. 17** QMS quality management | quality management system | Atlas events.jsonl + GAMP 5 ALCOA+ mapping (V1.7 existing) | QMS-evidence material |
| **Art. 19 §1** Log retention | 6 months minimum | Configurable (default 7 years to cover longest sectoral baseline) | **Was conflated with Art. 18 in Phase 1 — corrected** |
| **Art. 26** Deployer obligations | §5-6: monitor + retain logs | Deployer-side Atlas integration | Substrate-relevant |
| **Art. 50** GPAI transparency, synthetic content marking | machine-readable mark on synthetic content | `author_did = agent:*` in events.jsonl records *which agent produced output* (provenance metadata); does NOT embed an output-layer watermark on synthetic content itself | Substrate-relevant for provenance side; full Art. 50 compliance requires additional content-marking (watermark / disclosure label) outside current Atlas scope |
| **Art. 55** Systemic-risk GPAI | adversarial testing + incident reporting + cybersecurity | events.jsonl as incident-report substrate | Substrate-relevant |
| **Art. 73** Serious-incident reporting | 15-day report to market-surveillance authority (2-day for serious + safety) | `serious-incident-report-builder` CLI/UI (V2 collateral candidate) | **High-value collateral identified** |

**Reminder:** Atlas substrates these; *does not deliver* the obligations alone. Customer's provider-obligation chain (Art. 43, Annex VI/VII conformity assessment) remains customer's.

### 4.3 EU AI Liability Directive — STATUS CORRECTION
**Was AILD ist heute (Phase 2 Compliance H-5 correction):**
> The European Commission **WITHDREW the AI Liability Directive proposal in February 2025** (Commission Work Programme 2025, COM(2025) 45 final, justification: lack of foreseeable agreement).

**Was die fallback-Regelung tatsächlich ist:**
- **Revised Product Liability Directive (Directive (EU) 2024/2853, in force 2024-12-08)** — expressly covers software including AI systems. Art. 9 shifts disclosure burden when claimant cannot prove defect.
- **Atlas's relevance:** evidence-trail is structurally aligned with PLD Art. 9 disclosure-burden-shift scenarios because the events.jsonl IS the disclosure substance.

**Implications for AI-Liability-Insurance pitch (Phase 1 Doc A §4.2):**
- Reframe from "AILD-driven" to "PLD-driven" — substantially different counterparty conversation (PLD is in force; AILD is dead)
- **Solvency II** (Directive 2009/138/EC) constrains insurer pricing inputs (ORSA requirement); EIOPA June 2024 AI Opinion adds principles
- **Counsel required:** insurance-regulation-specialised counsel work (separate engagement layer beyond GDPR/AI-Act counsel). Phase 3 recommendation: **postpone the AI-Liability-Insurance pitch to V2-γ** (after V2-α and V2-β ship and produce real-world deployment evidence). Don't fundraise on this pitch in V2-α.

### 4.4 GDPR Art. 17 Right-to-be-Forgotten — UNRESOLVED
**The single highest-stakes open legal question per Phase 2 Compliance C-2 + Security C-3.**

Doc B §3.3's "hash exists, anchor exists, signature exists → 'this event existed at time T'" architecture is **plausibly** GDPR-compatible *under permissive reading*, but the strict reading favoured by EDPB Guidelines 4/2019 + CJEU C-582/14 *Breyer* + WP29 Opinion 4/2007 suggests:
- A blake3 hash of arbitrary plaintext bytes WITHOUT a controller-destroyed salt is treated as **pseudonymous = still personal data**
- The `author_did + timestamp + workspace_id` triple is itself potentially personal data when agent acts on behalf of an identifiable natural person
- "Signature attests Person X did Y at T" survives content deletion and IS processing of personal data

**Two architecturally-honest paths:**
- **Path A — accept hash-is-PII, redesign:** per-content random salt destroyed at deletion + hash-of-salted-content stored → post-deletion uncorrelatability. Cost: schema change pre-V2-α; salt management complexity.
- **Path B — defend hash-not-PII, gate with counsel:** obtain written legal opinion from German/Irish/French DPA-specialised firm that survives strict-reading scrutiny. Cost: counsel relationship (~€30-80K, 6-8 weeks).

**Phase 3 decision: PATH B + Path A as fallback.**
- Counsel engagement budgeted **€30-80K**, **pre-V2-α blocking** for EU customers with PII workspace
- Counsel firms to engage (lead candidates): German tier-1 firm (e.g., Hogan Lovells Frankfurt, Bird & Bird Munich), Irish tier-1 (Matheson, William Fry), French tier-1 (Cleary Gottlieb Paris)
- Outcome routes:
  - If counsel rules favorably → Path B holds; design as in Doc B §3.3 but with explicit disclaimer "tested against German BfDI / Irish DPC / French CNIL specifically"
  - If counsel rules unfavorably → fall back to Path A; salt-management design in V2-α
- **Layer 2 graph exposure (Phase 4 hardening):** the Projector stamps `{event_uuid, rekor_log_index, author_did}` on every Layer-2 graph node + edge as property (§5.1). Path A salt-redesign protects Layer-1 hash uncorrelatability but does NOT cover Layer-2 graph property erasure. Counsel scope (DECISION-COUNSEL-1) explicitly includes Layer-2 graph property erasure on Art. 17 request as a distinct operation from Layer-1 content-hash handling.
- **Documented in Phase 3 decisions.md as DECISION-COUNSEL-1.**

### 4.5 Regulator-Witness Federation — REFRAMED
**Phase 2 Compliance C-3 finding:** "regulator runs a witness key" has NO documented EU regulatory precedent (no BaFin, FCA, FINMA, AMF, AustraDA, CNIL endorsement). Closest analogues are §44 KWG on-site supervision, GDPR Art. 27 representative, eIDAS QTSPs, EBA outsourcing guidelines, FDA 21 CFR Part 11 §11.10(e) — none of which run cryptographic primitives.

**Phase 3 reframing:**
- Marketing language: *"regulator-friendly architecture"* (factual: architecture supports it) — NOT *"regulator-approved pattern"* (false: no regulator approved it)
- Demo 2 placeholder: replace "BaFin-witness-eu" with generic *"supervisor-witness-eu"* + one-line disclaimer naming BaFin / FCA / FINMA as illustrative examples
- **Phase 3 commitment:** pursue supervisory-authority sandbox engagement (lead candidates: BaFin AI Office, De Nederlandsche Bank + Dutch AP, CNIL "bac à sable", BoE AI Public-Private Forum) BEFORE V2-γ demo claims "regulator-piloted"
- **Documented as DECISION-COMPLIANCE-3 + DECISION-COMPLIANCE-4.**

### 4.6 Jurisdictional scope (expanded from EU-only)

| Jurisdiction | Framework | Atlas-relevance | Counsel-validation status |
|---|---|---|---|
| EU | AI Act 2024/1689, GDPR, PLD 2024/2853, NIS2, CRA 2024/2847 | **PRIMARY** | Pre-V2-α counsel engagement budgeted |
| US (federal banking) | SR 11-7 (Fed) + OCC 2011-12 | Atlas events.jsonl directly maps to model-development-records requirement | **HIGH-ROI add post-V2-α-EU-launch** |
| US (healthcare) | HIPAA §164.312(b) + FDA 21 CFR Part 11 §11.10(e) | §11.10(e) audit-controls clean fit | Post-V2-β |
| US (state) | Colorado SB24-205 (2026-02-01), California AB-2013 (2026-01-01) | Both align well | Post-V2-β |
| UK | UK GDPR + AI Bill principles-based | Structural-trust pitch translates | Post-V2-β |
| Japan | AI Bill 2024 (METI/CAS) | Principles-based; less prescriptive | Opportunistic |
| Singapore | MAS FEAT (financial) | Aligns | Opportunistic |
| Switzerland | FINMA AI guidance | Aligns | Opportunistic |
| China | Generative AI Measures 2023, PIPL data-localisation | **STRUCTURAL INCOMPATIBILITY** — air-gapped on-China deployment only option | Out-of-scope absent dedicated effort |
| Russia | FZ 242-FZ | Out-of-scope | Out-of-scope |

---

## 5. Three-Layer Trust Architecture (Phase 2 hardened)

### 5.1 Layer overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Agent (Hermes / Claude / GPT / Llama / custom)                 │
│  via MCP-Server (V1.19 Welle 1, live) OR HTTP-API               │
└────────────────────────────┬────────────────────────────────────┘
                             │ POST /api/atlas/write-node
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│ Atlas-Web write surface (V1.19 Welle 1, live)                   │
│ - Sign Ed25519 + COSE_Sign1 (V1)                                │
│ - workspace HKDF (V1.9) + agent-DID author_did (V2)             │
│ - Cedar policy gate (V2-δ optional, write-time)                 │
└────────────────────────────┬────────────────────────────────────┘
                             ▼
┌══════════════════════════════════════════════════════════════════┐
║  LAYER 1 — events.jsonl    [AUTHORITATIVE]                       ║
║  - Append-only signed events                                     ║
║  - Sigstore Rekor anchored (V1.6, live)                          ║
║  - Witness cosignatures (V1.13, live)                            ║
║  - Federated regulator-witness (V2-γ planned)                    ║
║  - Strict-chain CI gate (V1.19 Welle 10, live)                   ║
║  ► V1 trust property — never compromised by Layer 2/3 failures   ║
╚════════════════════════════════════════════════════════════════════╝
                             │
                             │ (tail / cron / event-driven)
                             ▼
┌──────────────────────────────────────────────────────────────────┐
│  Atlas Projector (V2-α NEW)                                      │
│  - Verifies each event signature pre-projection                  │
│  - Extracts entities + relationships (deterministic regex/schema)│
│  - Idempotent upsert into Layer 2                                │
│  - Stamps {event_uuid, rekor_log_index, author_did} on every     │
│    node + edge as property                                       │
│  - Emits signed `ProjectorRunAttestation` event into Layer 1     │
│    (Phase 2 Security Q-SEC-6 — makes determinism part of trust   │
│    chain, not out-of-band CI hygiene)                            │
│  - Versioned projector_schema with byte-pin canonicalisation     │
│    (analog to V1's signing_input_byte_determinism_pin)           │
└────────────────────────────┬─────────────────────────────────────┘
                             ▼
┌──────────────────────────────────────────────────────────────────┐
│  LAYER 2 — FalkorDB projection    [QUERYABLE, REBUILDABLE]       │
│  - Property graph, Cypher subset                                 │
│  - GraphBLAS sparse-matrix backend                               │
│  - SSPLv1 license — counsel-required for hosted service          │
│  - ArcadeDB Apache-2.0 fallback (Kuzu archived Apple Oct-2025)   │
│  - CI gate: replay Layer 1 → rebuilt graph → projector-state-hash│
│    MUST equal pinned `.projection-integrity.json`                │
│  ► Failure mode: drift detected by CI gate before customer ever  │
│    sees stale data; live customers shadow-projector cross-checks │
│    (Phase 2 Database HIGH proposal)                              │
└────────────────────────────┬─────────────────────────────────────┘
                             ▼
┌──────────────────────────────────────────────────────────────────┐
│  LAYER 3 — Mem0g cache    [FAST, REBUILDABLE, NEVER AUTHORITATIVE]│
│  - 91% p95 latency reduction over full-context retrieval         │
│    (Mem0g Locomo benchmark — Atlas does NOT re-validate this     │
│    against signer+CBOR+JSONL+projector pipeline; Demo 5 must     │
│    attribute honestly)                                           │
│  - Embeddings cached; cite-back to event_uuid on every result    │
│  - Semantic rebuildable from Layer 2 (NOT byte-deterministic)    │
│  - GDPR-erasure: explicit secure-delete + audit-trail event      │
│    parallel to content tombstone (Phase 2 Security H-3)          │
│  ► Failure mode: cache returns wrong result → agent cite-back to │
│    event_uuid fails verification → exception, retry against L2/L1│
└────────────────────────────┬─────────────────────────────────────┘
                             ▲
                             │
┌────────────────────────────┴─────────────────────────────────────┐
│  Agent (read-side, V2 NEW)                                       │
│  - Read endpoints: GET /entities/:id, /related/:id, /timeline,   │
│    POST /query (Cypher AST-validated), /audit/:event_uuid        │
│  - MCP V2 tools: query_graph, query_entities, query_provenance,  │
│    get_agent_passport, get_timeline                              │
└──────────────────────────────────────────────────────────────────┘
```

### 5.2 Trust invariant per layer

**Layer 1 (events.jsonl) — V1 invariant, unchanged:**
- Every event Ed25519+COSE-signed, hash-chained (`parent_hashes[0] == prev.event_hash` in strict mode), Sigstore Rekor anchored, optionally witness-cosigned
- WASM verifier offline-verifies all of the above from `events.jsonl` + `pubkey-bundle.json` alone, no Atlas operator trust needed
- **Property survives all V2 layer failures by construction** — if Layer 2 / 3 produce wrong data, consumer always re-verifies against Layer 1

**Layer 2 (FalkorDB) — derivative + rebuildable + CI-gated:**
- Failure mode: graph drift (silent corruption / non-deterministic projection)
- Detection: `projector-state-hash` CI gate (canonical hash of fully-rebuilt graph state must match pinned value); shadow-projector cross-check in production (Phase 2 Database H-1)
- **NEW Phase 2 addition (Q-SEC-6):** every projector run emits a signed `ProjectorRunAttestation` event into Layer 1 asserting `(projector_version, head_hash) → graph_state_hash`. Makes determinism part of the trust chain, not just CI hygiene.
- Recovery: rebuild from Layer 1 events — bounded by 5min/1M-events target (with parallel-projection plan needed for >10M events; **8.3h at 100M events** per Phase 2 Database P-CRIT-3 → V2-α must spec quantified RTO + parallel-projection strategy)

**Layer 3 (Mem0g) — semantic + rebuildable + cite-back:**
- Failure mode: cache returns false result (hallucinated retrieval, embedding drift, stale cache)
- Detection: every Mem0g result MUST cite back to `event_uuid`; agent's verifier resolves discrepancy by re-querying Layer 1 directly
- Recovery: invalidate cache + reseed from Layer 2 + secure-delete old cache files for GDPR (Phase 2 Security H-3)

**Cross-layer trust invariant (consolidated):**
> A consumer of Atlas at any layer can always (a) drop to Layer 1 alone, (b) run the offline WASM verifier against `events.jsonl` + `pubkey-bundle.json` (plus a bundled Rekor inclusion proof set OR connectivity to `rekor.sigstore.dev` for the Rekor-anchor verification step), and (c) produce a deterministic ✓ VALID / ✗ INVALID answer independent of Atlas operator, Layer-2 corruption, or Layer-3 cache state. **This is the bedrock V1 property and V2 must never undermine it.** Air-gapped operation produces signature-chain-and-hash-chain validity without the Rekor inclusion-proof leg — V2-α scope includes a `--bundled-inclusion-proof` ingestion path for fully offline deployments.

### 5.3 Agent Identity Layer (Phase 2 hardened)

**V1 (live):** Per-workspace HKDF-derived Ed25519 keys (`atlas-anchor:<workspace_id>`, prefix-pinned in `crates/atlas-trust-core/src/per_tenant.rs::PER_TENANT_KID_PREFIX`).

**V2 addition:** Per-agent DID — `did:atlas:<pubkey-hash>` — generated from agent's Ed25519 public key. Two-layer event signing:
- `kid` = workspace HKDF anchor (V1 property)
- `author_did` = agent identity (V2 NEW)
- **Both included in signing input** (per Phase 2 Security H-1 demand) so workspace-replay defence + agent-replay defence are both structural, not bolted on

**Cross-workspace agent identity:** same `did:atlas:<pubkey-hash>` across workspaces (good for Agent Passport portability); cross-workspace-replay defence preserved because `workspace_id` is in the signing input regardless of which key signs.

**Agent-key derivation (Phase 2 Security H-1 demand resolved):** generated independently per agent instance (NOT HKDF-derived from a master) — gives agents real key custody on their own HSM/keychain. Atlas does NOT see agent private keys.

**Agent Passport (DID document materialised view):**
- public key (Ed25519)
- creation timestamp
- workspaces signed into
- facts written count
- retraction count
- witness cosigners
- revocation chain (NEW)

**Revocation mechanism (Phase 2 Security C-1 resolved):**
- **Out-of-band revocation channel** — NOT signed by the (possibly compromised) DID key. Two-of-three threshold from {operator-rooted Ed25519 revocation key, workspace-witness-bundle, agent-DID itself}.
- **Compromise-case rule (Phase 4 hardening):** the agent-DID party can participate in the threshold ONLY when the revocation subject is *not* its own key. When the compromised key IS the agent-DID itself (the typical case), the threshold falls to **2-of-2 (operator-rooted-Ed25519 + workspace-witness-bundle)** — preserves the stated invariant "NOT signed by the compromised key". V2-γ design-doc must encode this rule explicitly in protocol state-machine.
- **`signed_at_rekor_inclusion_time` Δ-flagging** — events whose `event.timestamp` is more than Δ before their Rekor anchor inclusion time are flagged as suspect-backdated. `event.timestamp` is agent-claimed (untrusted); Rekor inclusion-time is observable from Rekor's log (trusted).
- **Revocation event kind** in Layer 1, with `revocation_reason: compromise|deprecated|policy`
- Documented in Phase 3 decisions as **DECISION-SEC-1**.

**Agent Passport legal framing (Phase 2 Compliance M-5 correction):**
- Drop "verifiable Agent Identity" / "reputation reist mit" / "Agent Marketplace-Capability"
- Use: *"cryptographic accountability binding between the deploying organisation and the agent's actions; the cryptographic evidence trail attaches to the agent's signing key and is portable across deployments"*
- No current EU/UK/US/JP legal framework recognises AI agent as legal-personhood entity; the Ed25519 key is *evidence about an attributed actor*, not an identity in its own right

### 5.4 Read-Side API (Phase 2 hardened)

**6 endpoints** (Phase 1 Doc B §2.8 + Phase 2 Security H-2 hardening):

| Method | Endpoint | Purpose | Phase 2 hardening |
|---|---|---|---|
| GET | `/api/atlas/entities/:id` | Entity by ID with provenance bundle | Auth: workspace + agent-DID scoped |
| GET | `/api/atlas/related/:id?depth=N` | Related entities, depth-capped | Parse-time depth cap (Phase 2 Security H-2(b)) |
| GET | `/api/atlas/timeline/:workspace?from=&to=` | Time-windowed events | Bi-temporal index plan required (Phase 2 Database HIGH) |
| POST | `/api/atlas/query` | Cypher subset, AST-validated | **No string-concat into Cypher**, prepared params only, allow-list procedures, no `apoc.*`, no `CALL db.*` (Phase 2 Security H-2 full demand) |
| POST | `/api/atlas/audit/:event_uuid` | Full provenance trail for one event | Provenance-bundle response shape |
| GET | `/api/atlas/passport/:agent_did` | Agent Passport materialised view | Workspace-scoped (`used_by_workspaces`) |

**Auth model:** workspace + agent-DID scoped. Rate-limit per (workspace_id, author_did, IP). Cache-coherency across L1/L2/L3 via `head_hash` ETag.

### 5.5 MCP V2 Tool Surface

V1 has `write_node` + `verify_trace`. V2 adds:
- `query_graph` — Cypher subset, AST-validated (same constraints as POST /query)
- `query_entities` — semantic search (Mem0g-backed)
- `query_provenance` — trace any fact to source events
- `get_agent_passport` — verify another agent's identity + reputation
- `get_timeline` — workspace events in time range

**Trust-property statement:** MCP V2 tools are thin clients of §5.4 endpoints. No tool-side trust authority. Per Phase 2 Security: all Cypher passes the same AST validation regardless of HTTP-vs-MCP entry point.

### 5.6 Federated Witness Cosignature for Regulators (V2-γ)

**Operating model (Phase 2 hardened):**
1. **Enrolment** — Regulator generates witness keypair on regulator-controlled HSM. Public key handed out-of-band to Atlas operator. **Phase 2 Security C-2 fix: enrolment requires M-of-N threshold-signed bundle update (NOT single operator V1.18 path) + emits `federation_enrolment_event` into Layer 1 so addition is part of verifiable record.** **Note (Phase 4 hardening):** enrolment agreement with the regulator MUST address the permanent public auditability of the regulator's `witness_kid` in the Rekor-anchored event log — any third party with Layer-1 read access will see which workspaces the regulator participates in and at what times. Data-handling and public-disclosure terms are scope of the enrolment contract, not solved by Atlas's cryptographic primitive alone.
2. **Per-event flow** — regulator-witness service tails events.jsonl, cosigns events matching subscription criteria (e.g., all "financial-recommendation" event kinds), publishes cosignature back into events.jsonl as separate event. Witness has `witness_class: regulator` field (Phase 2 Security C-2 demand) distinguishing from `auditor | peer | internal`.
3. **Verifier behaviour** — `--require-witness-kid <kid>` flag enforces presence of regulator cosignature; `--require-witness-class regulator` is a higher-level convenience.
4. **Revocation/regulator-removal lifecycle** (Phase 2 Compliance M-4 addition):
   - Regulator restructures → key handoff via threshold-signed bundle update + `federation_handoff_event`
   - Workspace downgrades from high-risk → operator removes regulator-witness requirement via `federation_removal_event` (past events with old cosignatures remain valid; new events do not require)
   - Regulator withdraws → similar removal flow

**Marketing language (Phase 3 decision):** "regulator-friendly architecture" (factual). NOT "regulator-approved pattern" (false absent supervisor pilot).

---

## 6. Risk Matrix v1 (Phase 1 + Phase 2 consolidated)

**Methodology:** Probability (L/M/H/CRITICAL) × Impact (L/M/H/CRITICAL) × Detectability (H/M/L) × Reversibility (H/M/L) + Mitigation status + Owner + Review cadence.

**Changes vs Phase 1 Doc C v0 (per Phase 2 critiques):**

| Risk | Phase 1 → Phase 2 change |
|---|---|
| R-A-01 Projection Determinism Drift | **Add ProjectorRunAttestation signed event into trust chain** (Q-SEC-6); **add shadow-projector production cross-check** (Database H-1); **quantify rebuild RTO + parallel-projection plan** (P-CRIT-3) |
| R-A-03 Agent Identity Key Compromise | **Specify out-of-band revocation channel + Δ-flagging** (Security C-1); **NOT signed by compromised key**; revocation event kind into Layer 1 |
| R-L-01 GDPR Art. 17 hash-as-PII | **Probability escalated MEDIUM → HIGH** (strict-reading analysis per Breyer + EDPB); **legal counsel listed as named mitigation step**, not just review |
| R-L-02 FalkorDB SSPL | **Kuzu fallback dead (Apple-archived Oct 2025)**; **ArcadeDB Apache-2.0 is next viable**; comparative spike before V2-α lock |
| R-Performance-Overhead | **Quantify 100M-event rebuild = 8.3h with current single-projector plan**; require parallel-projection design pre-V2-α |
| R-Mem0g-Vendor-Risk | **Mem0g embedding leakage of redacted content as side-channel** (Security H-3); secure-delete + audit event required |
| R-Hermes-Adoption-Reversal | **Hermes math: 60K stars → ~4-36 retained users steady-state**; **reclassify Hermes from GTM Hypothesis 1 to credibility asset** (Business CRITICAL) |
| R-S-01 Adoption Tipping Point | **Reverse GTM sequencing: EU-regulated Q0 not Q4** (Business CRITICAL) |
| **NEW R-L-03** Cross-Border Data Transfer / Schrems II | Probability M, Impact H, Detectability M, Reversibility M. Mitigation: SCC + DPA templates; data-residency config; monitor Schrems III progress |
| **NEW R-L-04** Conformity-Assessment-Substitution Derivative Liability | Probability L, Impact H, Detectability L, Reversibility L. Mitigation: explicit "not a conformity-assessment tool" language everywhere customer-facing |
| **NEW R-L-05** EU AI Office Implementing-Act Drift | Probability M, Impact M, Detectability M. Mitigation: quarterly tracking of AI Office publications; named owner |
| **NEW R-S-08** Anthropic/OpenAI "Verified Memory" Vendor Co-Option (Q4-2026 scenario) | Probability M, Impact H. Mitigation: triple-moat = federation-of-witnesses + cross-vendor + open-source-verifier |
| **NEW R-B-01** Fundraising-Blocking Market Sizing Gap | Probability H (now), Impact CRITICAL (now). Mitigation: TAM/SAM/SOM bottom-up math published before next round |

**Top-5 V2-α blocking (Phase 3 lock):**
1. R-A-01 (determinism CI gate + ProjectorRunAttestation) — engineering
2. R-L-01 (GDPR counsel opinion) — legal + engineering
3. R-A-03 (revocation mechanism design) — engineering
4. R-L-02 (FalkorDB SSPL + ArcadeDB spike) — legal + engineering
5. R-B-01 (TAM/SAM/SOM math published) — business

**Top blockers for V2-β (post-α):**
- R-L-03 (Schrems II SCCs ready for hosted-sync tier)
- R-L-04 (disclaimers in customer-facing surfaces)
- R-Performance-Overhead (parallel-projection plan + RTO quantified)

---

## 7. Competitive Landscape (Phase 1 D + Phase 2 updates)

### 7.1 Two market categories
**AI Agent Memory Infrastructure** (AI engineer persona): Mem0 (52K stars, Mem0g graph variant), Letta (formerly MemGPT, 22K), Zep (Cloud + Graphiti OSS), Anthropic Memory (Claude native), OpenAI Memory, Supermemory, Hindsight.

**Human Second Brain / PKM** (knowledge worker persona): Obsidian (1.5M MAU, 2750+ plugins, ZERO signature/verification plugin), Notion (30M+), Roam, Logseq, Capacities, Tana, Heptabase.

**Atlas's unique structural property in both:** cryptographic trust. **No current competitor in either category has it.**

### 7.2 Graph DB landscape (Phase 2 Database update)
| DB | License | Status |
|---|---|---|
| FalkorDB | SSPLv1 | V2-α primary choice; counsel-required for hosted service |
| Kuzu | MIT (was) | **ARCHIVED — Apple acquisition Oct 2025**; not a fallback |
| ArcadeDB | Apache-2.0 | **Next viable Apache-2.0 fallback**; comparative spike pre-V2-α lock |
| Neo4j Community | GPLv3 | Permissive-license issue similar to SSPL for SaaS |
| Memgraph | BSL → Apache-2.0 (older versions) | Comparable performance to FalkorDB |
| DuckDB graph extension | MIT | Embedded-only; not a hosted-service option |

### 7.3 Partner candidates (Phase 2 Business + Phase 1 D)
- **Mem0 (Taranjeet Singh)** — already plan to use Mem0g; transition to formal partnership pre-V2-β
- **Graphiti / Zep (Daniel Chalef)** — Apache-2.0, FalkorDB-backend support, 23K stars, MCP server with 100K+ weekly users. **Strongest single partner candidate**; risk: 12-18mo could add crypto-edge-signing → competitor. **Engage proactively**.
- **Hermes / Nous Research (Teknium)** — community partnership, MIT-license. Reclassified from GTM-Hypothesis-1 to credibility asset (Phase 2 Business CRITICAL).
- **Obsidian (Erica Xu, Shida Li)** — plugin path for Verifiable Second Brain market validation. 1-2 week effort.
- **Lyrie ATP** — $2M preseed May 2026, Anthropic CVP-accepted, Ed25519-based IETF-track agent-trust-protocol. **Atlas should commit to ATP-compatibility as alias for Agent Passports** (Phase 2 Business recommendation). Integration-not-competition.
- **VeritasChain VCP v1.2** — trading-vertical compliance audit log, EU AI Act Art. 12/19/26/72 aligned. Doesn't overlap Atlas scope. Integration-not-competition.
- **Sigstore (Linux Foundation)** — already partner via Rekor + cosign. Continue.
- **FalkorDB** — commercial license relationship needed for hosted-service tier.

### 7.4 Vendor co-option scenario (NEW Phase 2 Business)
**Concrete Q4-2026 scenario:** Anthropic announces "Claude Trust" with on-platform signing.
**Atlas response (triple moat):**
1. Federation-of-witnesses — Atlas works with multiple regulators / auditors; Anthropic's on-platform signing is single-vendor-controlled
2. Cross-vendor — Atlas works with Claude + GPT + Hermes + Llama simultaneously; vendor-native solutions don't
3. Open-source verifier — Atlas's WASM verifier is Apache-2.0 forkable + auditable; vendor-native is opaque

### 7.5 Comparison matrix
Rows: each major competitor + Atlas. Columns: License / Pricing / Trust-Property / Open-Source / Multi-Agent / Temporal / Provenance-API / GDPR-Compliant-by-design.
**Phase 2 Business correction (D §6 self-favoring flag):** split Atlas into "V1.0.1 today" vs "V2 planned" rows. Don't compare Atlas's V2 future-state to competitors' V1 current-state.

---

## 8. Demo Programme (Phase 2 Product overhaul)

**Phase 1 had 5 demos; Phase 2 Product surfaced 4 CRITICAL issues:**
1. No "verified" climax is meaningful for non-crypto-literate users without an HTTPS-absent-lock-equivalent failure state
2. Two-market positioning operationally undefended without a decision rule (resolved in §3.3)
3. Demo 4 (Verifiable Second Brain) requires V2-γ-minimum product surface that doesn't exist → phantom-commitment risk
4. Zero demos show failure modes

**Phase 3 revised demo programme:**

| # | Demo | Audience | Status | Notes |
|---|------|---------|--------|-------|
| 1 | Multi-Agent Race (renamed) | AI engineer | Recommended landing-page hero | DROP word "race" — "Multi-Agent Attribution" or "Verified Multi-Agent Workspace". Two agents (Hermes + Claude) writing into same workspace with color-coded passport keys. Per Phase 2 Product: visible-failure-state visible in chrome. |
| 2 | Continuous Regulator Witness | Compliance officer / Enterprise CTO | **Above-the-fold primary** (Phase 2 Business reversal) | Most ship-able TODAY (V1.14 Scope J live). Replace BaFin placeholder with generic supervisor-witness + 1-line disclaimer. Per Phase 2 Compliance: rephrase from "regulator-approved" to "regulator-friendly". |
| 3 | Agent Passport | AI marketplace / multi-tenant deployer | V2-γ readiness, start data-collection now | Requires V2-α DIDs + 30 days recorded agent activity. Begin operational data collection during V2-α. |
| ~~4~~ | ~~Verifiable Second Brain~~ | knowledge worker | **DEFERRED** | Per Phase 2 Product CRITICAL — depicts V2-γ-minimum product surface that doesn't exist. Replace with Obsidian-plugin micro-demo (1-2 week effort, validates Second-Brain market before full Atlas-PKM client). |
| 5 | Mem0g Hybrid Speed | AI engineer (latency-concerned) | V2-β readiness | Attribute 91% latency honestly (Mem0g's Locomo benchmark, NOT Atlas's measurement). |
| **NEW 6** | **Quickstart (30s first signed event)** | AI engineer | **TODAY readiness** | Per Phase 2 Product top concrete proposal. `npx @atlas-trust/quickstart` → first signed event → first verify → first browser-render of provenance. Highest-converting AI-engineer artifact with zero V2-α/β dependency. |
| **NEW 7** | **Failure-Mode Demo** | All audiences (compliance + AI engineer) | V1 + V2-α | Per Phase 2 Product CRITICAL — trust products live or die on how they fail. Show: agent tries to write to a tampered workspace → verifier rejects, audit-trail shows tampering origin. Equivalent to HTTPS's absent-lock for cryptographic provenance. |

**Hero CTA inversion (Phase 2 Business):** Demo 2 (Continuous Regulator Witness) + compliance-briefing CTA = above-the-fold primary. Demo 1 + AI-engineer-quickstart CTA = secondary (still strong, but #2). Enterprise pipeline is fundamental for V2-α GTM (€25M-750M TAM); developer pipeline is freemium-feed (€600K-3.6M SOM personal-tier ceiling).

**"Hide trust by default" principle (Phase 2 Product):** ALL demos (1, 2, 3, 5, 6, 7) must surface trust as `Verified ✓` / `Tampered ✗` UI states, NOT as raw Ed25519/Rekor/COSE technical detail. Technical detail lives one click deeper for users who want it.

---

## 9. GTM + Business Model (Phase 2 Business reversal)

### 9.1 Phase 2 corrections to Phase 1 Doc A §6
- **REVERSE §6.5 GTM sequencing:** EU-regulated enterprise must start **Q0**, not Q4. Reasoning: enterprise sales cycles 6-12 months; need to close before V2 runway ends. Phase 2 Business CRITICAL.
- **Hermes-Agent reclassification:** GTM Hypothesis 1 → credibility asset. Hermes math: 60K stars → ~4-36 retained Atlas users in steady-state (compound discount). Continue using Hermes for demos, MCP-skill ecosystem, brand-pull. Don't fund-raise on Hermes-distribution-channel claim.
- **Personal-tier monetization:** caps at €600K-3.6M ARR. Sub-fundable on its own. **DECISION: personal-tier is freemium-to-enterprise-feed**, not standalone revenue.

### 9.2 TAM/SAM/SOM (Phase 2 Business CRITICAL gap — back-of-envelope math)

> **Status:** all figures `[Atlas-team-assumption-flag, Phase 4 verify]` unless marked otherwise.

| Layer | Size | Source / Logic |
|---|---|---|
| **TAM (Total)** | €25M-€750M / year | EU AI Act compliance market 2026-2030, top-down BCG-style |
| **AI Memory Infra SAM** | $10-45M | Mem0 / Letta / Zep / Graphiti combined publicly-disclosed ARR (estimated) |
| **EU AI Act Compliance TAM** | €25M-€750M | High-risk-AI-deployer count × budget range €5K-€250K/year |
| **Verifiable Second Brain SOM (ceiling)** | €135K-€1.4M | Obsidian power-user subset willing to pay €5-10/mo, conservative bottom-up |
| **Atlas V2 initial SOM target** | €1M-€5M ARR by end-2026 | 5-50 enterprise contracts × €20K-€100K ACV (EU-regulated verticals) |

### 9.3 Funding plan (Phase 2 Business addition)
**Round size:** €2-4M seed extension or Series A bridge (current runway + V2-α/β/γ/δ = 14-20 sessions × team cost).
**Lead investor candidates (Phase 2 Business named list):**
- **Tier 1 (AI-infra deep):** a16z (Martin Casado), Greylock (Saam Motamedi), Bessemer (Sameer Goldberg)
- **Tier 1 (EU):** Speedinvest (Vienna), HV Capital (Munich), 468 Capital (Berlin), Cherry Ventures (Berlin), Project A (Berlin)
- **Strategic:** Munich Re (HSB AI underwriting unit), Allianz X, DB1 Ventures (Deutsche Börse)

**First-10-customers pipeline (Phase 2 Business CRITICAL gap):**
- *Phase 3 ACTION:* Nelson assembles a named list of 10 reachable customer-prospects in EU-regulated fintech (BaFin-supervised), healthcare-AI, or insurance pre-V2-α GTM. Without this, fundraising stalls.
- Lead-vertical candidates: BaFin-supervised fintechs (warm intros via DB1 / 468), early-stage insurance-AI (Munich Re portfolio), AI-as-medical-device companies (under EU MDR + AI Act intersection).

### 9.4 Monetization (open-core)
- **Free:** verifier crates (Apache-2.0), `atlas-trust-verify-wasm` npm package, `atlas-mcp-server`, CLI, `atlas-web` self-hosted. Open-source.
- **Paid Personal/Team (€5-15/user/month):** hosted-sync (multi-device), workspace-witness federation, basic Mem0g cache, Atlas Cloud Explorer UI
- **Paid Enterprise (€10K-€100K+ ACV):** regulator-witness-federation enablement, custom Cedar policies, SLA, EU-data-residency, SOC2/ISO27001-aligned hosting, dedicated counsel-reviewed compliance collateral
- **Open-Core Discipline:** verifier never paywalled (would break Atlas's core trust narrative); hosted-service paywalled (SSPL/Apache split work).

---

## 10. Welle Decomposition (Phase 2 Architect re-baseline)

| Welle | Phase 1 estimate | Phase 2 re-baseline | Scope |
|---|---|---|---|
| **V2-α Foundation** | 3-4 sessions | **5-8 sessions** | Atlas Projector + FalkorDB integration + Agent-DID schema + content-hash separation + projector-state-hash CI gate + ProjectorRunAttestation signed event + ArcadeDB comparative spike + GDPR counsel opinion gate |
| **V2-β Read-Side** | 2-3 | **4-5** | Mem0g cache integration + Read-API (6 endpoints with AST-validated Cypher) + MCP V2 tools + Explorer UI (FalkorDB Browser embed OR Cytoscape.js) + secure-deletion mechanism + parallel-projection plan |
| **V2-γ Identity + Federation** | 2-3 | **3-4** | Agent Passports + revocation mechanism (out-of-band channel + Δ-flagging) + regulator-witness federation (M-of-N threshold bundle update + federation_enrolment_event) + Hermes-skill v1 |
| **V2-δ Optional** | 2-3 | **2-3** | Cedar policy at write-time + Graphiti retrieval-layer + post-quantum hybrid Ed25519+ML-DSA-65 co-sign (NIST-aligned, additive Algorithm enum) |
| **Total** | 10-14 | **14-20** | Plus 6-8 weeks counsel engagement in parallel with V2-α |

**V2-β depends serially on V2-α** (Mem0g indexes FalkorDB which depends on projector). Phase 2 Architect H-3 correction: not parallel as Phase 1 implied.

**Welle-14b (npm Trusted Publishers + dual-publish fix) remains in flight orthogonally** — can run in parallel with V2-α prep but should land before V2-α public materials.

---

## 11. Counsel Engagement Plan (NEW Phase 3 commitment)

**Budget:** €30-80K, 6-8 week structured engagement. **Pre-V2-α blocking** for: EU customer with PII workspace, public marketing claiming Art. 12 fit, regulator-witness-federation demo claims.

**Scope (consolidated from Phase 2 Compliance Q-COMP-11):**
1. **DECISION-COUNSEL-1:** GDPR Art. 4(1) hash-as-personal-data opinion (Path A redesign vs Path B defence) — **PRIMARY**
2. **DECISION-COUNSEL-2:** AILD→PLD reframe of §4.2 + insurance-regulation engagement strategy
3. **DECISION-COUNSEL-3:** Art. 43 conformity-assessment-substitution liability disclaimer drafting (NEW R-L-04)
4. **DECISION-COUNSEL-4:** Schrems II / cross-border SCC + DPA templates (NEW R-L-03)
5. **DECISION-COUNSEL-5:** Verbatim Art. 12 + Annex IV §1(g) + §2(g) marketing copy review (C-1 fix)
6. **DECISION-COUNSEL-6:** Witness-federation EU regulatory positioning brief — "what existing supervisory pattern is closest" (C-3 reframe)
7. **DECISION-COUNSEL-7:** DPIA + FRIA template drafting (Phase 2 Compliance Blind Spot 3, high-ROI marketing collateral)

**Counsel firm candidates (Phase 3 shortlist):**
- **German tier-1:** Hogan Lovells Frankfurt, Bird & Bird Munich, Hengeler Mueller, GLNS
- **Irish tier-1:** Matheson, William Fry, Arthur Cox
- **French tier-1:** Cleary Gottlieb Paris, Bredin Prat
- **Boutique AI-Act-specialised:** Taylor Wessing (UK), DLA Piper (multi-jurisdiction), Linklaters (London + Frankfurt + Paris)

**Q-COMP-3 Supervisory engagement (parallel, pre-V2-γ blocking):**
- BaFin AI Office (Germany)
- De Nederlandsche Bank + Dutch AP (Netherlands)
- CNIL "bac à sable" (France)
- BoE AI Public-Private Forum (UK)

---

## 12. Open Strategic Questions (consolidated)

These survived Phase 2 critique and are NOT decided in Phase 3 — they go forward to Phase 4 + V2-α work + counsel engagement.

### 12.1 Counsel-required (€30-80K engagement gates these)
- **Q-3-1** GDPR Art. 4(1) hash-as-PII Path A vs Path B decision — gated on counsel opinion
- **Q-3-2** AILD→PLD reframe verbatim language — gated on counsel
- **Q-3-3** Insurance-regulation-aware counterparty conversations (Munich Re HSB) — gated on Solvency II + EIOPA AI Opinion mapping work
- **Q-3-4** Supervisor-engagement (BaFin / Dutch AP / CNIL / BoE) — independent of GDPR counsel but parallel timeline

### 12.2 Engineering-required (V2-α + V2-β scope)
- **Q-3-5** Projector canonicalisation byte-pin spec — concrete spec required pre-V2-α start
- **Q-3-6** ArcadeDB vs FalkorDB comparative spike — V2-α blocking
- **Q-3-7** Out-of-band revocation channel design — Q-SEC-6 + revocation chain in §5.3
- **Q-3-8** ProjectorRunAttestation event schema — Phase 2 Security signed-attestation pattern
- **Q-3-9** Parallel-projection design for >10M event rebuild — quantified RTO budget
- **Q-3-10** Mem0g secure-deletion + audit-event procedure — Phase 2 Security H-3

### 12.3 Product/Business-required
- **Q-3-11** First-10-customers named pipeline — Nelson assembles before next round
- **Q-3-12** TAM/SAM/SOM bottom-up math published — €30K-€80K counsel + €5K analyst could deliver
- **Q-3-13** Obsidian-plugin micro-MVP (1-2 weeks) — Second-Brain market validation route
- **Q-3-14** Hermes-skill v1 release timing — V2-γ vs earlier
- **Q-3-15** Mem0 formal partnership vs informal — pre-V2-β decision
- **Q-3-16** Lyrie ATP compatibility commit — Phase 2 Business recommendation

### 12.4 Strategic-uncertain (no specific gate)
- **Q-3-17** EU AI Office Implementing-Act tracking cadence + owner — R-L-05 mitigation
- **Q-3-18** Conformity-Assessment-Substitution disclaimer language across all customer surfaces — R-L-04 mitigation
- **Q-3-19** Anthropic/OpenAI "Verified Memory" Q4-2026 scenario monitoring + triple-moat reinforcement
- **Q-3-20** China + Russia jurisdictional out-of-scope explicit policy

---

## 13. References to Atlas crates / files (Phase 4 anchor points)

| Concept (V2 spec) | Source-of-truth in repo |
|---|---|
| events.jsonl format | `crates/atlas-trust-core/src/trace_format.rs` |
| Strict-chain CI gate | `crates/atlas-trust-core/src/hashchain.rs::check_strict_chain` |
| Per-tenant HKDF | `crates/atlas-trust-core/src/per_tenant.rs::PER_TENANT_KID_PREFIX` |
| Rekor anchoring | `crates/atlas-trust-core/src/anchor.rs::default_trusted_logs` |
| COSE canonicalisation (V1) + Algorithm enum (V2-δ post-quantum migration hook, planned not yet present) | `crates/atlas-trust-core/src/cose.rs` — V1 contains `build_signing_input` for CBOR canonicalisation; `#[non_exhaustive] Algorithm` enum is a V2-δ addition |
| WASM verifier | `crates/atlas-verify-wasm/` |
| Write-surface (V1.19 Welle 1) | `apps/atlas-web/src/app/api/atlas/write-node/route.ts` |
| MCP server (V1.19 Welle 1) | `apps/atlas-mcp-server/src/index.ts` |
| Witness cosignature primitive (V1.13) | `crates/atlas-trust-core/src/witness.rs` (relevant section) |
| V1 public-API contract | `docs/SEMVER-AUDIT-V1.0.md` |
| V1/V2 boundaries | `docs/ARCHITECTURE.md` §10, §11 |
| V1 compliance mapping | `docs/COMPLIANCE-MAPPING.md` (V1.7) |
| V1 byte-determinism CI pins | `crates/atlas-trust-core/src/lib.rs` (test module `signing_input_byte_determinism_pin` etc.) |
| Trust-root mutation defence (V1.18) | `tools/test-trust-root-mutations.sh` |
| SLSA L3 provenance attestation | npm registry @atlas-trust/verify-wasm@1.0.1 + Sigstore Rekor logIndex 1518327130 |

---

## 14. What This Doc Is NOT

- **NOT a Phase 4 Master Plan.** That's `docs/V2-MASTER-PLAN.md` (Phase 4 output, lands on master) which compresses this v1 into ~300 lines + welle-decomposition tied to specific PR-Wellen.
- **NOT a Working Methodology.** That's `docs/WORKING-METHODOLOGY.md` (Phase 4 output) which captures the 4-phase iteration framework for reuse on future Großthemen (e.g., post-quantum migration).
- **NOT counsel-vetted.** All §4 + §11 references to legal interpretation are pre-counsel. Phase 4 work runs in parallel with counsel engagement; counsel-output refines this v1 into v1.1 / v2.
- **NOT a sprint-plan.** §10 Welle decomposition is the strategic scope; specific Welle planning happens per-Welle when it starts (V2-α plan-doc separately).

---

## 15. Decision-Log Pointer

All Phase 3 ACCEPT / MODIFY / REJECT / DEFER decisions are documented in `.handoff/decisions.md`. Each entry includes: topic, crit source, recommendation, decision, rationale, reversibility (HIGH/MED/LOW), review-after trigger. Target: ≥10 entries; actual count: see `decisions.md`.

---

**End of Master Vision v1.** Next phase = Phase 4 = `docs/V2-MASTER-PLAN.md` lands on master with this v1 distilled to ~300 lines + Welle-decomposition + success criteria, plus `docs/WORKING-METHODOLOGY.md` capturing the reusable 4-phase pattern.
