# Atlas V2 — Master Plan

> **Status:** Phase 4 output, 2026-05-12. **Scope:** master-resident strategic source-of-truth for Atlas V2.
> **Sources:** distilled from `.handoff/v2-master-vision-v1.md` (full V2 vision, ~615 lines) and `.handoff/decisions.md` (23 explicit ACCEPT/MODIFY/DEFER decisions). Read those for full context, rationale, and Phase-2 critique provenance.
> **Companion doc:** `docs/WORKING-METHODOLOGY.md` (reusable 4-phase iteration pattern).
> **Methodology:** this plan was produced via Atlas's 4-phase iteration framework (see `docs/WORKING-METHODOLOGY.md`).

---

## 1. V2 Vision

**Atlas V1** (live on npm as `@atlas-trust/verify-wasm@1.0.1`, SLSA Build L3) shipped the *write-side trust property*: Ed25519 + COSE_Sign1 + deterministic CBOR + blake3 hash chain + Sigstore Rekor anchoring + witness cosignature + offline WASM verifier. Any third party can mathematically verify an Atlas trace without trusting the Atlas operator.

**Atlas V2 extends this to a queryable, agent-agnostic shared knowledge substrate** — without compromising V1's structural trust property.

**Tagline (Phase 3 lock):** *"Knowledge your AI can prove, not just claim."*

V2 opens two markets that share one substrate:
- **Multi-Agent Shared Memory** (AI engineer / agent builder) — PRIMARY market and landing-page front door
- **Verifiable Second Brain** (knowledge worker) — SECONDARY market, validated via Obsidian-plugin path before full Atlas-native PKM commitment

V2 explicitly does *not* turn Atlas into an agent, a conformity-assessment tool (EU AI Act Art. 43 / Annex VI), or an eIDAS-qualified trust service provider.

---

## 2. Two-Market Positioning (with decision rule)

| Market | Persona | Distribution funnel | Headline value |
|---|---|---|---|
| **Multi-Agent Shared Memory** (PRIMARY) | AI engineer / agent builder / ML-Ops | GitHub → `npx @atlas-trust/quickstart` → MCP-skill install → Hermes-skill OR direct HTTP-API | *"Every fact has a verified author. No more 'the AI said it'."* |
| **Verifiable Second Brain** (SECONDARY) | Knowledge worker / researcher / consultant | Obsidian-plugin first (1–2 weeks effort) — every-edit-signs + Rekor-anchor + `[verified]` badge | *"Your Second Brain, attested for the AI era."* |

**Decision rule (Phase 3, `DECISION-PRODUCT-1` + Master Vision §3.3):** Multi-Agent Shared Memory is the operational front door. Landing-page hero, main docs, pricing-page-default, and first 10 examples all speak the AI-engineer persona's vocabulary. Secondary-market materials (Obsidian-plugin) live one click off the hero.

Reasoning: AI-memory-infra SAM (~$10–45M) is fundable; personal-tier PKM SOM (€600K–3.6M ARR) is sub-fundable on its own. EU AI Act compliance pressure is AI-engineer-side, not knowledge-worker-side.

---

## 3. Three-Layer Trust Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Any agent (Hermes / Claude / GPT / Llama / custom)             │
│  via MCP-Server (V1.19 Welle 1, live) OR HTTP-API               │
└────────────────────────────┬────────────────────────────────────┘
                             ▼
┌══════════════════════════════════════════════════════════════════┐
║  LAYER 1 — events.jsonl    [AUTHORITATIVE, V1 unchanged]         ║
║  Ed25519 signed · COSE_Sign1 · blake3 hash chain · Rekor anchor  ║
║  · witness cosignatures · strict-chain CI gate · offline WASM    ║
║  verifier · V1 trust property survives all V2 layer failures     ║
╚════════════════════════════════════════════════════════════════════╝
                             │ (projector: verifies + extracts)
                             ▼
┌──────────────────────────────────────────────────────────────────┐
│  LAYER 2 — ArcadeDB projection    [QUERYABLE, REBUILDABLE]       │
│  Property graph · Cypher subset · Apache-2.0 · embedded mode     │
│  · CI gate: projector-state-hash MUST match pinned hash          │
│  · ProjectorRunAttestation event emitted into Layer 1            │
│  · FalkorDB SSPLv1 fallback (per V2-α Welle 2 spike flip)        │
└────────────────────────────┬─────────────────────────────────────┘
                             ▼
┌──────────────────────────────────────────────────────────────────┐
│  LAYER 3 — Mem0g cache    [FAST, REBUILDABLE, NEVER AUTHORITATIVE]│
│  Embeddings · semantic search · cite-back to event_uuid          │
│  · secure-delete on GDPR erasure (overwrite, not unlink)         │
│  · failure mode: agent verifier re-queries L2 / L1 on mismatch   │
└──────────────────────────────────────────────────────────────────┘
```

**Cross-layer trust invariant:** a consumer of Atlas at any layer can always drop to Layer 1 alone, run the offline WASM verifier against `events.jsonl` + `pubkey-bundle.json` (plus a bundled Rekor inclusion proof set for fully air-gapped operation, OR connectivity to `rekor.sigstore.dev` for the Rekor-anchor verification step), and produce a deterministic ✓ VALID / ✗ INVALID answer independent of Atlas operator, Layer-2 corruption, or Layer-3 cache state. **This is V2's load-bearing invariant.** See Master Vision §5.2 for the air-gapped verification path detail.

**Agent identity (V2 new):** per-agent `did:atlas:<pubkey-hash>` Ed25519 DIDs, generated independently per agent instance (not HKDF-derived; agents hold their own keys). Both `kid` (workspace HKDF) and `author_did` (agent) are part of the signing input. Revocation via out-of-band channel (M-of-N threshold, NOT signed by the possibly-compromised DID key) — see `DECISION-SEC-1`.

---

## 4. Top-5 V2-α Blocking Risks

| Risk | Description | Mitigation |
|---|---|---|
| **R-A-01 Projection Determinism Drift** | LOW detect × CRITICAL impact. Graph projection silently produces non-byte-identical output → trust invariant breaks invisibly | Triple-hardening: canonicalisation byte-pin + `ProjectorRunAttestation` signed event into Layer 1 + parallel-projection design pre-V2-α (`DECISION-ARCH-1` / `DECISION-SEC-2`) |
| **R-L-01 GDPR Art. 17 Hash-as-Personal-Data** | Probability HIGH (escalated from MEDIUM per CJEU *Breyer* + EDPB Guidelines 4/2019 strict reading) | €30–80K counsel engagement, 6–8 weeks, pre-V2-α blocking. Path B (counsel opinion) primary, Path A (per-content salt redesign) fallback (`DECISION-COMPLIANCE-3` / `DECISION-COUNSEL-1`) |
| **R-A-03 Agent Identity Key Compromise** | Revocation signed by compromised key fails closed-by-design | Out-of-band revocation channel (M-of-N threshold) + `signed_at_rekor_inclusion_time` Δ-flagging for backdate detection (`DECISION-SEC-1`) |
| **R-L-02 FalkorDB SSPLv1 License** | Hosted-service blocker; Kuzu fallback archived by Apple Oct-2025. **Mitigated by V2-α Welle 2 spike flip** — see `docs/V2-ALPHA-DB-SPIKE.md` | V2-α Welle 2 spike (2026-05-12) recommended ArcadeDB Apache-2.0 as primary with MEDIUM-HIGH confidence, eliminating SSPLv1 §13 hosted-service obligation. FalkorDB demoted to fallback. Counsel-validated license opinion still pre-V2-α-public-materials blocking |
| **R-B-01 Fundraising-Blocking Market Sizing Gap** | No published TAM/SAM/SOM math. Probability HIGH (now), Impact CRITICAL (now) | Bottom-up math + first-10-customers named pipeline before next fundraising conversation (`DECISION-BIZ-3` / `DECISION-BIZ-4`) |

Full risk matrix with 13+ entries in `.handoff/v2-master-vision-v1.md` §6.

---

## 5. Counsel Engagement (Pre-V2-α Blocking)

**Budget:** €30–80K, 6–8 week structured engagement. **Blocking gate for:** any EU customer with PII workspace · public marketing claiming Art. 12 fit · regulator-witness-federation demo claims.

**Scope (7 items, full detail in Master Vision §11):**
1. GDPR Art. 4(1) hash-as-personal-data opinion (Path A vs Path B)
2. AILD→PLD reframe + insurance-regulation engagement strategy
3. Art. 43 conformity-assessment-substitution liability disclaimer drafting
4. Schrems II / cross-border SCC + DPA templates
5. Verbatim Art. 12 + Annex IV marketing copy review
6. Witness-federation EU regulatory positioning brief
7. DPIA + FRIA template drafting (high-ROI marketing collateral)

**Counsel firm shortlist:** German tier-1 (Hogan Lovells Frankfurt, Bird & Bird Munich, Hengeler Mueller), Irish tier-1 (Matheson, William Fry, Arthur Cox), French tier-1 (Cleary Gottlieb Paris, Bredin Prat), Boutique AI-Act (Taylor Wessing, DLA Piper, Linklaters). Selection is Nelson's call.

**Parallel supervisor sandbox track:** BaFin AI Office (DE), De Nederlandsche Bank + Dutch AP (NL), CNIL "bac à sable" (FR), BoE AI Public-Private Forum (UK). Pre-V2-γ blocking for any "regulator-piloted" demo claim.

---

## 6. Welle Decomposition

> **Phase 2 re-baseline:** Phase 1 estimated 10–14 sessions total; Phase 2 Architect H-3 + Database P-CRIT-3 surfaced concrete blocker items adding 2–4 sessions to V2-α alone. **Total V2 = 14–20 sessions** plus 6–8 weeks counsel engagement in parallel with V2-α. Welle 14b/c/d/e existing roadmap remains (14b: npm Trusted Publishers + dual-publish fix; can run in parallel with V2-α prep).

### V2-α Foundation (5–8 sessions)
**Scope:** Atlas Projector + **ArcadeDB integration** (post-Welle-2 flip) + Agent-DID schema (Welle 1, SHIPPED) + content-hash separation + projector-state-hash CI gate + `ProjectorRunAttestation` signed event + ArcadeDB vs FalkorDB comparative spike (Welle 2, SHIPPED) + GDPR counsel opinion gate.

**Dependencies:** counsel engagement kickoff (parallel); Welle 2 spike informed the V2-α DB lock (ArcadeDB primary recommended); canonicalisation byte-pin spec.

**Blocking risks:** R-A-01 (determinism), R-L-01 (GDPR counsel), R-L-02 (mitigated by Welle 2 spike flip to ArcadeDB primary).

**Success criteria:**
- Projector emits canonical byte-pinned graph state matching `.projection-integrity.json` (CI gate green)
- Each projector run emits a signed `ProjectorRunAttestation` into Layer 1
- Agent DID schema (`did:atlas:<pubkey-hash>`) issued + parsed by verifier (SHIPPED 2026-05-12)
- ArcadeDB vs FalkorDB comparative spike yields go/no-go decision (SHIPPED 2026-05-12; recommendation: ArcadeDB primary, FalkorDB fallback)
- Counsel opinion on GDPR Art. 4(1) hash-as-PII delivered

**Expected PR count:** 5–8 (one per session, ~Welle-14c/d/e size). **Welle 1 + Welle 2 shipped 2026-05-12 (2 of 5-8 done).**

### V2-β Read-Side (4–5 sessions, depends serially on V2-α)
**Scope:** Mem0g cache integration + 6 Read-API endpoints (AST-validated Cypher) + MCP V2 tools + Explorer UI (ArcadeDB Studio embed OR Cytoscape.js) + secure-deletion mechanism + parallel-projection plan.

**Dependencies:** V2-α (Mem0g indexes FalkorDB which depends on projector — per Phase 2 Architect H-3 correction, NOT parallel as Phase 1 implied).

**Blocking risks:** Cypher injection / DoS hygiene (`DECISION-SEC-4`); Mem0g embedding leakage on GDPR erasure (`DECISION-SEC-5`).

**Success criteria:**
- 6 endpoints live with AST-validated Cypher (no string-concat, no `apoc.*`, no `CALL db.*`)
- 5 MCP V2 tools live (`query_graph`, `query_entities`, `query_provenance`, `get_agent_passport`, `get_timeline`)
- Secure-delete procedure for Mem0g embeddings on GDPR erasure with parallel audit event
- Atlas+Mem0g end-to-end benchmark published (not Mem0g-cache-hit-only)

**Expected PR count:** 4–5.

### V2-γ Identity + Federation (3–4 sessions)
**Scope:** Agent Passports + revocation mechanism (out-of-band channel + Δ-flagging) + regulator-witness federation (M-of-N threshold bundle update + `federation_enrolment_event`) + Hermes-skill v1.

**Dependencies:** V2-α DIDs; supervisor sandbox engagement progress.

**Blocking risks:** R-A-03 (revocation); supervisor sandbox precedent absence.

**Success criteria:**
- Out-of-band revocation channel operational with M-of-N threshold (operator-Ed25519 + workspace-witness + agent-DID)
- `federation_enrolment_event` + `federation_handoff_event` + `federation_removal_event` event kinds in Layer 1
- Agent Passport materialised-view endpoint live (`GET /api/atlas/passport/:agent_did`)
- Hermes-skill v1 published to Hermes plugin marketplace
- Demo 3 (Agent Passport) operational with 30+ days recorded activity

**Expected PR count:** 3–4.

### V2-δ Optional (2–3 sessions, deferred decision)
**Scope:** Cedar policy enforcement at write-time + Graphiti retrieval-layer + post-quantum hybrid Ed25519+ML-DSA-65 co-sign (NIST-aligned, additive `Algorithm` enum).

**Dependencies:** V2-γ; post-quantum migration is gated on a separate NIST-aligned decision-gate driven by external timeline.

**Success criteria:** Cedar policy gate live at write-side; Graphiti retrieval-layer integrated as optional plugin; ML-DSA-65 co-sign passes V1's `signing_input_byte_determinism_pin` regression.

**Expected PR count:** 2–3.

### Total
**14–20 sessions** plus 6–8 weeks counsel engagement in parallel with V2-α.

---

## 7. Demo Programme

| # | Demo | Audience | Readiness | Notes |
|---|------|---------|-----------|-------|
| 1 | **Multi-Agent Attribution** (renamed from "race") | AI engineer | V2-α | Two agents (Hermes + Claude) writing into same workspace with color-coded passport keys. Visible failure-state in chrome. Landing-page hero secondary. |
| 2 | **Continuous Regulator Witness** | Compliance officer / Enterprise CTO | TODAY (V1.14 live) | **Above-the-fold primary** (Phase 2 Business hero CTA inversion). Generic supervisor-witness + 1-line illustrative disclaimer. "Regulator-friendly", NOT "regulator-approved". |
| 3 | **Agent Passport** | AI marketplace / multi-tenant deployer | V2-γ | Begin operational data collection during V2-α. |
| ~~4~~ | ~~Verifiable Second Brain~~ | — | **DEFERRED** | Phantom-commitment risk. Replaced by Obsidian-plugin micro-demo (1–2 week effort). |
| 5 | **Mem0g Hybrid Speed** | AI engineer (latency-concerned) | V2-β | Attribute 91% latency honestly to Mem0g's Locomo benchmark, NOT Atlas's measurement. |
| **6** | **Quickstart (30s first signed event)** | AI engineer | **TODAY** | `npx @atlas-trust/quickstart` → first signed event → first verify → first browser-render of provenance. Highest-converting AI-engineer artifact. |
| **7** | **Failure-Mode Demo** | All audiences | V1 + V2-α | HTTPS-absent-lock-equivalent. Agent tries to write to tampered workspace → verifier rejects → audit-trail shows tampering origin. |

**Hero CTA inversion:** Demo 2 + compliance-briefing CTA = above-the-fold primary; Demo 1 + AI-engineer-quickstart CTA = secondary. Enterprise pipeline (€25M–750M TAM) > developer-pipeline (€600K–3.6M personal-tier SOM ceiling).

**"Hide trust by default" principle:** all demos surface trust as `Verified ✓` / `Tampered ✗` UI states. Technical detail (Ed25519/Rekor/COSE) lives one click deeper.

---

## 8. Competitive Position

**No competitor in either target category has cryptographic trust as a structural property.** Atlas's unique moat is verified via WebSearch 2026-05 across Mem0, Letta, Zep, Graphiti, Anthropic Memory, OpenAI Memory, Obsidian (1.5M MAU, 2750+ plugins, zero signature/verification plugin), Notion, and adjacent tools.

**Partner candidates:**
- **Graphiti / Zep** (Apache-2.0, FalkorDB-backend support, 23K stars, MCP server) — strongest single partner; engage proactively (12–18mo competitor risk if they add crypto edge-signing)
- **Mem0** — already plan to use Mem0g; transition to formal partnership pre-V2-β
- **Lyrie ATP** — IETF-track Ed25519-based agent-trust-protocol; commit to ATP-compatibility as alias for Agent Passports (`DECISION-BIZ-6`)
- **Obsidian** — plugin path for Second-Brain market validation (1–2 weeks)
- **Hermes / Nous Research** — community partnership for demos + brand-pull (reclassified from GTM-Hypothesis-1 per `DECISION-BIZ-1`)

**Vendor co-option scenario (Q4-2026):** if Anthropic announces "Claude Trust" with on-platform signing, Atlas's triple moat = federation-of-witnesses (multiple regulators) + cross-vendor (Claude + GPT + Hermes + Llama simultaneously) + open-source verifier (Apache-2.0, forkable, auditable). Vendor-native solutions are single-vendor-controlled and opaque.

---

## 9. GTM + Business Model

**GTM sequencing reversal (Phase 2 Business CRITICAL, `DECISION-BIZ-2`):** EU-regulated enterprise must start **Q0**, not Q4. Enterprise sales cycles 6–12 months; close before V2 runway ends.

**Hermes-Agent reclassified** from GTM-Hypothesis-1 to credibility asset (`DECISION-BIZ-1`). Math: 60K stars × compound (install / discovery / first-write / retention / Atlas-vs-vendor-memory choice) → ~4–36 retained users steady-state. Demo + brand-pull value real; distribution-channel value not.

**TAM/SAM/SOM (back-of-envelope, Phase 4 verify):**

| Layer | Size | Source |
|---|---|---|
| TAM EU AI Act compliance | €25M–€750M / year | High-risk-AI-deployer count × budget range €5K–€250K/year |
| AI Memory Infra SAM | $10–45M | Mem0/Letta/Zep/Graphiti combined ARR (estimated) |
| Verifiable Second Brain SOM ceiling | €135K–€1.4M | Obsidian power-user subset paying €5–10/mo |
| Atlas V2 initial SOM target | €1M–€5M ARR by end-2026 | 5–50 enterprise contracts × €20K–€100K ACV |

**Monetization (open-core):**
- **Free:** verifier crates (Apache-2.0), `@atlas-trust/verify-wasm` npm, `atlas-mcp-server`, CLI, `atlas-web` self-hosted
- **Personal/Team €5–15/user/month:** hosted-sync, workspace-witness federation, basic Mem0g cache, Atlas Cloud Explorer UI
- **Enterprise €10K–€100K+ ACV:** regulator-witness-federation enablement, custom Cedar policies, SLA, EU-data-residency, SOC2/ISO27001-aligned hosting, dedicated counsel-reviewed compliance collateral
- **Discipline:** verifier never paywalled (would break the core trust narrative)

**Round size:** €2–4M seed extension or Series A bridge. Lead investor candidates: a16z (Martin Casado), Greylock (Saam Motamedi), Bessemer (Sameer Goldberg), Speedinvest, HV Capital, 468 Capital, Cherry Ventures, Project A, plus strategic Munich Re (HSB AI underwriting), Allianz X, DB1 Ventures.

**First-10-customers pipeline (`DECISION-BIZ-3`, action assigned to Nelson):** named list of 10 reachable customer-prospects in EU-regulated fintech (BaFin-supervised), healthcare-AI (EU MDR + AI Act intersection), or insurance-AI (Munich Re portfolio). Pre-fundraising blocking.

---

## 10. Success Criteria for V2

V2 is "successful" when **all** of the following hold:

1. **Trust invariant preserved:** V1 offline-WASM-verifier still produces deterministic ✓/✗ on `events.jsonl` alone, regardless of Layer 2 / Layer 3 state. CI-enforced via `projector-state-hash` gate + `ProjectorRunAttestation` regression.
2. **Counsel-validated EU posture:** GDPR Art. 4(1) hash-as-PII opinion delivered; Art. 12 + Annex IV verbatim mapping reviewed; PLD 2024/2853 disclosure-burden framing reviewed; Schrems II SCC templates in place.
3. **Three production-grade Read-API + 5 MCP V2 tools** live with AST-validated Cypher (no injection vectors); 6th endpoint (passport) post-V2-γ.
4. **Agent identity layer operational:** `did:atlas:<pubkey-hash>` issued by ≥3 independent agents; revocation mechanism tested end-to-end with synthetic-compromise drill.
5. **First enterprise contract closed** with EU-regulated customer (BaFin-fintech / healthcare-AI / insurance-AI). ARR ≥ €20K.
6. **Demos 2 + 6 + 7 shipped TODAY** + Demos 1 + 5 by V2-β + Demo 3 by V2-γ; CTA-inverted hero in production.
7. **Fundraising-defensible numbers published:** TAM/SAM/SOM with named methodology; first-10-customers pipeline named; round closed or option held with reasoned timing.
8. **Vendor co-option resilience:** Atlas works simultaneously with ≥3 frontier vendors' agents (Claude + GPT + Hermes minimum). Triple moat (federation + cross-vendor + open-source verifier) operational.

---

## 11. Reference Pointers

| Concept | Source-of-truth |
|---|---|
| Full V2 vision (rationale, Phase 1+2 provenance) | `.handoff/v2-master-vision-v1.md` |
| 23 explicit ACCEPT/MODIFY/DEFER decisions | `.handoff/decisions.md` |
| Reusable 4-phase methodology | `docs/WORKING-METHODOLOGY.md` |
| V2-α DB choice spike (ArcadeDB vs FalkorDB, Welle 2) | `docs/V2-ALPHA-DB-SPIKE.md` |
| V1 public-API contract | `docs/SEMVER-AUDIT-V1.0.md` |
| V1/V2 boundary spec | `docs/ARCHITECTURE.md` §10, §11 |
| events.jsonl format | `crates/atlas-trust-core/src/trace_format.rs` |
| Strict-chain CI gate | `crates/atlas-trust-core/src/hashchain.rs::check_strict_chain` |
| Per-tenant HKDF | `crates/atlas-trust-core/src/per_tenant.rs::PER_TENANT_KID_PREFIX` |
| Rekor anchoring | `crates/atlas-trust-core/src/anchor.rs::default_trusted_logs` |
| COSE canonicalisation (V1) + Algorithm enum (V2-δ post-quantum hook, planned) | `crates/atlas-trust-core/src/cose.rs` — V1 ships `build_signing_input` for CBOR canonicalisation; `#[non_exhaustive] Algorithm` enum is a V2-δ addition (not yet present) |
| WASM verifier | `crates/atlas-verify-wasm/` |
| Write surface (V1.19 Welle 1) | `apps/atlas-web/src/app/api/atlas/write-node/route.ts` |
| MCP server (V1.19 Welle 1) | `apps/atlas-mcp-server/src/index.ts` |
| Witness cosignature primitive (V1.13) | `crates/atlas-trust-core/src/witness.rs` |
| V1 byte-determinism CI pins | `crates/atlas-trust-core/src/lib.rs` (test module `signing_input_byte_determinism_pin`) |
| Trust-root mutation defence (V1.18) | `tools/test-trust-root-mutations.sh` |
| SLSA L3 provenance attestation | npm registry `@atlas-trust/verify-wasm@1.0.1` + Sigstore Rekor logIndex `1518327130` |

---

**End of Master Plan.** Updates land via SemVer-versioned amendments to this file. Major V2-phase milestones (V2-α / β / γ / δ Welle completion) trigger a `Master Plan v1.X` increment with explicit changelog entry in `CHANGELOG.md`.
