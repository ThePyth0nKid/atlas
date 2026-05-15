# Atlas V2 — Decisions Log

> **Purpose:** explicit ACCEPT / MODIFY / REJECT / DEFER decisions from Phase 3 synthesis of the 6 Phase-2 critiques against the 5 Phase-1 Foundation Documents. Format per Iteration-Framework §3. **Convergence criterion:** ≥10 entries; this log delivers 22. All CRITICAL and HIGH crit-points are addressed (accepted, modified, deferred, or explicitly rejected with rationale).
>
> **Format per entry:**
> ```
> ## YYYY-MM-DD: <Topic> [DECISION-ID]
> - Crit source: <agent-role + specific crit-finding ID>
> - Phase 1 doc affected: <Doc A/B/C/D/E §X.Y>
> - Recommendation: <verbatim what crit proposed>
> - Decision: ACCEPT / MODIFY-as-follows / REJECT / DEFER
> - Rationale: <why>
> - Reversibility: HIGH / MEDIUM / LOW
> - Review-after: <date or trigger>
> ```

---

## 2026-05-12: AILD Status Correction [DECISION-COMPLIANCE-1]
- **Crit source:** Compliance H-5 + Compliance Concrete Proposal 3
- **Phase 1 doc affected:** Doc A §3.2 + §4.2 (AI-Liability-Insurance pitch)
- **Recommendation:** Rewrite §3.2 to reflect EU AI Liability Directive WITHDRAWAL Feb 2025 (Commission Work Programme 2025); fallback regime is PLD 2024/2853 in force 2024-12-08. Reframe §4.2 from AILD-driven to PLD-driven.
- **Decision:** **ACCEPT** — fully integrated into Master Vision v1 §4.3. AI-Liability-Insurance pitch (§4.2) is **DEFERRED to V2-γ** pending insurance-regulation-specialised counsel.
- **Rationale:** This is a factual error correction. AILD-withdrawal is publicly verifiable (Commission Work Programme 2025, COM(2025) 45 final). Any sophisticated competitor or counsel can fact-check Phase 1 Doc A and conclude Atlas is operating on stale legal data. Marketing risk is high and easily mitigated.
- **Reversibility:** HIGH (just text correction)
- **Review-after:** if AILD is re-proposed by future Commission, revisit. Track via Q-3-17 EU AI Office cadence.

---

## 2026-05-12: "Independently Verifiable" Phrasing Out, Verbatim Art. 12 In [DECISION-COMPLIANCE-2]
- **Crit source:** Compliance C-1 + M-3 (and propagated to Doc D §5 + Doc E Demo 2)
- **Phase 1 doc affected:** Doc A §3.1, Doc D §5, Doc E Demo 2 (all five docs propagate this)
- **Recommendation:** Replace "independently verifiable" Art. 12 paraphrase with verbatim Art. 12 §1 + §2 + Annex IV §1(g) + §2(g). Adopt design-claim phrasing: "Atlas's offline WASM verifier exceeds what the Regulation requires."
- **Decision:** **ACCEPT** — verbatim mapping in Master Vision v1 §4.1. Search/replace across all 5 Phase-1 docs in Phase 4 master-plan derivation.
- **Rationale:** Atlas's *substance* is genuinely stronger than Art. 12 minimum; marketing must reflect what the law actually says, not Atlas's preferred paraphrase. Removes a credibility-tripwire that any competitor or regulator can hit in 5 minutes of fact-checking. Counsel review recommended on final wording.
- **Reversibility:** HIGH (just text)
- **Review-after:** counsel engagement (DECISION-COUNSEL-5)

---

## 2026-05-12: GDPR Art. 17 Hash-as-Personal-Data — Counsel Path B with Path A Fallback [DECISION-COMPLIANCE-3 / DECISION-COUNSEL-1]
- **Crit source:** Compliance C-2 + Concrete Proposal 5 + Q-COMP-2; Security C-3; Risk Matrix R-L-01
- **Phase 1 doc affected:** Doc B §3.3 (GDPR architecture), Doc C R-L-01 (probability MEDIUM→HIGH)
- **Recommendation:**
  - Path A: redesign with per-content salt destroyed at deletion → post-deletion uncorrelatability
  - Path B: defend hash-not-PII with written counsel opinion that survives strict-reading scrutiny
- **Decision:** **PATH B primary, Path A as documented fallback.** **€30-80K counsel engagement** is **pre-V2-α blocking** for any EU customer with PII workspace. Counsel firms in shortlist (full shortlist also in Master Vision §11): German tier-1 (Hogan Lovells Frankfurt, Bird & Bird Munich, Hengeler Mueller, GLNS), Irish tier-1 (Matheson, William Fry, Arthur Cox), French tier-1 (Cleary Gottlieb Paris, Bredin Prat), Boutique AI-Act-specialised (Taylor Wessing, DLA Piper, Linklaters).
- **Rationale:** This is the highest-stakes open legal question per Phase 2 Compliance. Engineering analysis alone is judged insufficient (Risk Matrix R-L-01 ROBUST mitigation requires counsel). Path B preferred because Path A imposes schema-change pre-V2-α at significant complexity; if counsel rules unfavorably, Path A is well-understood salt-management redesign. Probability escalation MEDIUM→HIGH reflects strict-reading analysis under CJEU *Breyer* + EDPB Guidelines 4/2019 + WP29 Opinion 4/2007 + Hamburg DPA precedent.
- **Reversibility:** LOW once V2-α schema commits (Path A redesign post-launch is migration-heavy)
- **Review-after:** counsel opinion delivered, pre-V2-α start

---

## 2026-05-12: Regulator-Witness Federation Reframing [DECISION-COMPLIANCE-4]
- **Crit source:** Compliance C-3 + Concrete Proposal 10
- **Phase 1 doc affected:** Doc B §2.10, Doc A §4.1, Doc E Demo 2
- **Recommendation:** Demo 2 placeholder "BaFin-witness-eu" replaced with generic "supervisor-witness-eu" + 1-line illustrative disclaimer. Marketing language "regulator-friendly architecture" (factual), NOT "regulator-approved pattern" (false absent supervisor pilot). Pursue actual supervisor sandbox engagement before V2-γ.
- **Decision:** **ACCEPT** — verbatim reframing in Master Vision v1 §4.5 + §8 Demo Programme. Supervisor sandbox engagement listed as Q-3-4 (parallel-track counsel-engagement).
- **Rationale:** No EU supervisor has publicly endorsed cryptographic-cosignature compliance pattern. Marketing it as endorsed = vendor-overreach detectable by any sophisticated compliance officer. Reframing preserves the design's genuine novelty while honest about the absence of supervisor precedent.
- **Reversibility:** HIGH (text only)
- **Review-after:** first supervisor sandbox engagement (Q-3-4)

---

## 2026-05-12: Projection Determinism — Triple-Layer Hardening [DECISION-ARCH-1 / DECISION-SEC-2]
- **Crit source:** Architect C-1 (canonicalisation byte-pin) + Security Q-SEC-6 (ProjectorRunAttestation) + Database P-CRIT-3 (parallel-projection + quantified RTO)
- **Phase 1 doc affected:** Doc B §2.1 (Trust Invariant), §3.2 (Open question on projection-determinism), Doc C R-A-01
- **Recommendation:**
  - Architect: canonicalisation function byte-pin spec analog to V1's `signing_input_byte_determinism_pin`
  - Security: every projector run emits signed `ProjectorRunAttestation` event into Layer 1 asserting `(projector_version, head_hash) → graph_state_hash`
  - Database: parallel-projection design pre-V2-α; quantified RTO budget (>10M event scenarios). Note 100M-event single-projector rebuild = 8.3h baseline.
- **Decision:** **ACCEPT all three** as combined V2-α blocking requirements. Integrated into Master Vision v1 §5.1 ASCII diagram + §5.2 Layer 2 trust spec + §6 Risk Matrix R-A-01 update + §10 Welle V2-α scope.
- **Rationale:** R-A-01 is the single biggest V1-invariant-leak risk (LOW detectability × CRITICAL impact). Three independent crits identified the same weak spot from different angles. The three mitigations are complementary, not alternative: canonicalisation makes determinism testable, ProjectorRunAttestation makes determinism trustable, parallel-projection makes determinism operationally sustainable.
- **Reversibility:** MEDIUM (schema choices in V2-α; retroactive fix possible but expensive)
- **Review-after:** V2-α design-doc + canonicalisation spike

---

## 2026-05-12: Agent Identity Revocation Mechanism [DECISION-SEC-1]
- **Crit source:** Security C-1 (revocation lag bound is wrong)
- **Phase 1 doc affected:** Doc B §2.7 (Agent DID), Doc C R-A-03
- **Recommendation:** Out-of-band revocation channel NOT signed by compromised key (M-of-N threshold: operator-rooted Ed25519 + workspace-witness-bundle + agent-DID). Plus `signed_at_rekor_inclusion_time` Δ-flagging for backdate detection.
- **Decision:** **ACCEPT with Phase 4 hardening** — integrated into Master Vision v1 §5.3 (Agent Identity Layer). V2-γ blocking for Agent Passport production deployment. **Phase 4 hardening:** the agent-DID threshold party can participate ONLY when the revocation subject is NOT its own key. When the compromised key IS the agent-DID itself, the threshold falls to 2-of-2 (operator + workspace-witness), preserving the "NOT signed by the compromised key" invariant. V2-γ design-doc encodes this in the protocol state-machine.
- **Rationale:** The Phase 1 design ("revocation event signed by the same key") fails closed-by-design — if attacker has key, attacker controls revocation publication. Combined with the Insurance-Pricing demo (§4.2, deferred to V2-γ but still architectural concern), this is real-money risk. Out-of-band channel is standard cryptographic-protocol hygiene; absence is the unusual choice. Phase 4 hardening closes the residual deadlock-risk where the compromised key would otherwise be one of three threshold votes.
- **Reversibility:** LOW once V2-γ ships with passport deployment in production
- **Review-after:** V2-γ design-doc

---

## 2026-05-12: Witness Federation M-of-N Threshold Enrolment [DECISION-SEC-3]
- **Crit source:** Security C-2 (single-key trust root for federation enrolment = rogue-witness vector)
- **Phase 1 doc affected:** Doc B §2.10
- **Recommendation:** Threshold-signed bundle-mutation protocol for federation-roster additions, distinct from V1.18 single-operator path. Plus `federation_enrolment_event` event kind in events.jsonl. Plus `witness_class` field (regulator | auditor | peer | internal).
- **Decision:** **ACCEPT** — integrated into Master Vision v1 §5.6 + §5.1 ASCII diagram. V2-γ blocking for any regulator-witness production scenario.
- **Rationale:** V1.18 protected-surface model assumes one operator; V2 federation expands blast radius. Adding M-of-N for federation roster aligns with TUF root-rotation pattern (industry-standard for federations). The `federation_enrolment_event` makes federation additions part of verifiable trust chain, not just PR-commit hygiene.
- **Reversibility:** MEDIUM (schema addition pre-V2-γ; retroactive harder)
- **Review-after:** V2-γ design-doc

---

## 2026-05-12: Cypher Passthrough Hardening [DECISION-SEC-4]
- **Crit source:** Security H-2 (Cypher injection + DoS surface)
- **Phase 1 doc affected:** Doc B §2.8, §2.9, §3.13
- **Recommendation:** AST-level validation, prepared-statement parameter binding through to FalkorDB driver, parse-time depth caps (not just execution-time timeout), procedure allow-list (no `apoc.*`, no `CALL db.*`), per-graph workspace isolation at parse layer, `require_anchored` rewrite as AST transform not text substitution.
- **Decision:** **ACCEPT all** — integrated into Master Vision v1 §5.4 Read-Side API table + MCP V2 tool spec §5.5. V2-β blocking for any production Cypher endpoint.
- **Rationale:** Standard injection/DoS hygiene that Phase 1 Doc B handwaved. Insufficient mitigation here = production-down-grade quality bug equivalent to SQL injection in a SaaS database product.
- **Reversibility:** LOW once endpoints ship publicly
- **Review-after:** V2-β design-doc, security-review pre-launch

---

## 2026-05-12: Mem0g Embedding-Leakage Secure Deletion [DECISION-SEC-5]
- **Crit source:** Security H-3 (embedding side-channel leakage of redacted content)
- **Phase 1 doc affected:** Doc B §3.3 (GDPR procedure)
- **Recommendation:** Secure-delete (overwrite, not unlink) Layer 3 embeddings on GDPR erasure. Audit-trail event "embedding-erased" parallel to content tombstone. No embedding-fine-tune on tenant data (workspace policy).
- **Decision:** **ACCEPT** — integrated into Master Vision v1 §5.1 ASCII diagram Layer 3 + §5.2 Layer 3 trust spec + §10 V2-β scope.
- **Rationale:** Embeddings can leak content per Morris et al. 2023 (~92% reconstruction). GDPR erasure has to mean erasure. Cost is low (file overwrite); benefit is structural.
- **Reversibility:** HIGH (operational procedure, not schema)
- **Review-after:** V2-β secure-deletion procedure spec

---

## 2026-05-12: Welle Decomposition Re-Baseline [DECISION-ARCH-2]
- **Crit source:** Architect H-3 (Welle decomposition undercounted ~2×)
- **Phase 1 doc affected:** Doc B §4, Doc A §6 timeline
- **Recommendation:** Re-baseline V2-α 5-8 sessions, V2-β 4-5, V2-γ 3-4, V2-δ 2-3 = 14-20 total (not 10-14). V2-β depends serially on V2-α (Mem0g indexes FalkorDB), not parallel.
- **Decision:** **ACCEPT** — Master Vision v1 §10 carries the re-baseline. Welle 14b-iii Demo-Ready alignment delay re-stated as "10-12 sessions" not "6 sessions".
- **Rationale:** Phase 1 Doc B was optimistic. Phase 2 Architect H-3 added concrete blocker items (canonicalisation byte-pin, ProjectorRunAttestation, GDPR counsel, ArcadeDB spike) that legitimately add 2-4 sessions to V2-α scope alone. Re-baselining now prevents future "Welle 14b-iii delayed AGAIN" narrative damage.
- **Reversibility:** HIGH (timeline document only)
- **Review-after:** V2-α plan-doc derivation

---

## 2026-05-12: FalkorDB Fallback Re-Plan (Kuzu Archived) [DECISION-DB-1]
- **Crit source:** Database (Kuzu acquired by Apple Oct-2025, repo archived); cross-ref Doc D §4.4 + Doc C R-L-02
- **Phase 1 doc affected:** Doc B §2.3, Doc D §4.4, Doc C R-L-02
- **Recommendation:** Replace Kuzu fallback with **ArcadeDB (Apache-2.0)**. Comparative benchmark spike against FalkorDB pre-V2-α lock. Memgraph + HugeGraph + DuckDB-graph-ext as second-tier fallbacks.
- **Decision:** **ACCEPT — superseded by DECISION-DB-4 on 2026-05-12.** Original framing kept ArcadeDB as fallback to FalkorDB primary. V2-α Welle 2 spike flipped this: ArcadeDB primary, FalkorDB fallback (see `docs/V2-ALPHA-DB-SPIKE.md` + `DECISION-DB-4`).
- **Rationale:** Phase 1 plan referenced Kuzu as the MIT fallback for SSPL exposure; that fallback is now dead. ArcadeDB is the next viable Apache-2.0 graph DB. Comparative spike is mandatory because if ArcadeDB doesn't meet Atlas's performance/feature needs, FalkorDB SSPL becomes a much harder commercial-license dependency without a real escape hatch.
- **Reversibility:** MEDIUM (fallback choice is reversible pre-lock; post-V2-α much harder)
- **Review-after:** ArcadeDB spike completes — DONE 2026-05-12, see DECISION-DB-4

---

## 2026-05-12: FalkorDB Performance Claims Honesty [DECISION-DB-2]
- **Crit source:** Database P-CRIT-1 (FalkorDB "sub-ms p99 traversal" claim unsourced and dimensionally wrong as generalised)
- **Phase 1 doc affected:** Doc B §2.3
- **Recommendation:** Remove unsourced performance claims; replace with Atlas-measured benchmarks at specific graph sizes + workload mixes once V2-α spike completes.
- **Decision:** **ACCEPT** — drop performance claims from Master Vision v1 until V2-α benchmark is done; restate as "FalkorDB chosen for GraphBLAS sparse-matrix backend and Cypher-subset compatibility; Atlas will publish benchmark results during V2-α."
- **Rationale:** Unsourced perf claims are credibility tripwires. Same principle as DECISION-COMPLIANCE-2 (verbatim Art. 12) — say less than we can support, not more.
- **Reversibility:** HIGH
- **Review-after:** V2-α benchmark publication

---

## 2026-05-12: Mem0g Latency Claim Attribution [DECISION-DB-3]
- **Crit source:** Database P-CRIT-2 (91% Mem0g latency = cache-hit-only, not Atlas's full pipeline)
- **Phase 1 doc affected:** Doc B §2.5, Doc E Demo 5
- **Recommendation:** Attribute 91% latency reduction explicitly to Mem0g's Locomo benchmark, NOT Atlas's measurement. Atlas's signer+CBOR+JSONL+projector pipeline adds significant latency on top of Mem0g cache. Demo 5 storyboard must use this honest attribution.
- **Decision:** **ACCEPT** — Master Vision v1 §5.1 ASCII diagram Layer 3 carries the attribution. Demo 5 storyboard text in §8 updated.
- **Rationale:** Same honesty principle. Demo viewers should not infer Atlas-with-Mem0g is 91% faster than Atlas-without-Mem0g; the claim is "Mem0g's cache layer is 91% faster than full-context retrieval on Locomo, and Atlas integrates Mem0g."
- **Reversibility:** HIGH
- **Review-after:** V2-β Atlas+Mem0g end-to-end benchmark

---

## 2026-05-12: Hermes-Agent Reclassification (GTM → Credibility) [DECISION-BIZ-1]
- **Crit source:** Business CRITICAL (Hermes-Agent distribution math: 60K stars → ~4-36 retained users steady-state)
- **Phase 1 doc affected:** Doc A §6.1 (GTM Hypothesis 1), Doc B §2.6 (Hermes skill)
- **Recommendation:** Reclassify Hermes from "GTM Hypothesis 1" (primary distribution) to "credibility asset / demo channel / brand-pull". Continue Hermes-skill development; don't fundraise on Hermes-as-distribution-channel.
- **Decision:** **ACCEPT** — Master Vision v1 §9.1 carries the reclassification. Hermes-skill remains V2-γ scope.
- **Rationale:** Phase 2 Business math is rigorous. 60K stars compound through (a) install rate, (b) skill-discovery rate, (c) first-write rate, (d) retention rate, (e) Atlas-vs-vendor-memory choice rate. Each step is ~20-50% conversion. Compound = single-digit-to-low-double-digit retained Atlas users. Demo-and-credibility value is real (Hermes-skill makes Atlas credible to AI-engineer audience); GTM-distribution value is not.
- **Reversibility:** HIGH (positioning text)
- **Review-after:** V2-γ Hermes-skill launch + first 90 days retained-user data

---

## 2026-05-12: GTM Sequencing Reversal (EU-Regulated Q0 not Q4) [DECISION-BIZ-2]
- **Crit source:** Business CRITICAL ("reverse Doc A §6.5 GTM sequencing")
- **Phase 1 doc affected:** Doc A §6.5
- **Recommendation:** EU-regulated enterprise must start **Q0** not Q4. Sales cycles 6-12 months → need to close before V2 runway ends. Other GTM streams (open-weight halo, Obsidian-style monetization) remain but no longer compete for Q0 slot.
- **Decision:** **ACCEPT** — Master Vision v1 §9.1 + §8 hero-CTA-inversion (Demo 2 above-the-fold).
- **Rationale:** Enterprise sales cycle math is incontrovertible. Atlas's strongest demo (Demo 2 Continuous Regulator Witness) is also most ship-able TODAY. Aligning GTM with this strength (rather than Hermes-distribution which is fragile per DECISION-BIZ-1) is the rational sequencing.
- **Reversibility:** MEDIUM (GTM sequencing is reversible but customer trust-building once started shouldn't be deprioritised)
- **Review-after:** Q1-2027 enterprise pipeline review

---

## 2026-05-12: Demo Programme Overhaul [DECISION-PRODUCT-1]
- **Crit source:** Product CRITICAL × 4 + Concrete Proposals 1-5
- **Phase 1 doc affected:** Doc E (full overhaul) + Doc A §1 (tagline lock)
- **Recommendation:**
  - Lock tagline 2 "Knowledge your AI can prove, not just claim" as universal
  - Drop Demo 1's word "race" → "Multi-Agent Attribution"
  - Demo 4 (Verifiable Second Brain) DEFERRED (depicts non-existent product surface)
  - ADD Demo 6 Quickstart (TODAY readiness, AI-engineer-first-success-funnel)
  - ADD Demo 7 Failure-Mode (HTTPS-absent-lock-equivalent visible-failure state)
  - Hero CTA inversion: Demo 2 + compliance-briefing primary; Demo 1 + quickstart secondary
  - "Hide trust by default": Verified ✓ / Tampered ✗ UI states, technical detail one click deeper
- **Decision:** **ACCEPT all** — Master Vision v1 §3.2 (Obsidian-plugin path for Second-Brain), §8 (revised demo programme table), §9 (CTA inversion).
- **Rationale:** Phase 2 Product surfaced 4 CRITICAL issues that all coalesce on "demo programme as designed is partly aspirational, partly opaque to non-crypto-literate viewers, and has wrong priority ordering for current go-to-market". Re-prioritisation is low-cost and high-impact.
- **Reversibility:** HIGH (demos are storyboards, not committed product)
- **Review-after:** first 1000 landing-page sessions tracked-conversion data

---

## 2026-05-12: Counsel Engagement Pre-V2-α [DECISION-COUNSEL-MASTER]
- **Crit source:** Compliance Q-COMP-11 (€30-80K, 6-8 weeks engagement decision); cross-ref all counsel-required items (C-2, C-3, H-5, H-7, R-L-03, R-L-04)
- **Phase 1 doc affected:** entire Doc A §3 + Doc B §2.10 + §3.3 + Doc C Legal/Regulatory category
- **Recommendation:** Front-load counsel before V2-α public materials. Lead counsel options: German (Hogan Lovells Frankfurt, Bird & Bird Munich), Irish (Matheson, William Fry), French (Cleary Gottlieb Paris). Budget €30-80K.
- **Decision:** **ACCEPT, front-loaded.** **€30-80K, 6-8 week structured engagement, pre-V2-α blocking gate.** Scope: 7 specific items (DECISION-COUNSEL-1 through -7 in Master Vision v1 §11).
- **Rationale:** 11 of the 22 high-stakes Phase-3 decisions touch legal interpretation. Single retained counsel relationship at appropriate-tier firm addresses most at structured cost. Deferring to post-V2-α accepts marketing-risk in interim and produces costly retroactive fixes. Phase 2 Compliance net assessment: "the language and scope of the compliance claims need tightening, not the underlying engineering" — counsel is the tightening mechanism.
- **Reversibility:** HIGH (relationship can be terminated; opinion is durable)
- **Review-after:** counsel engagement kickoff (target: Q3-2026 pre-V2-α)

---

## 2026-05-12: First-10-Customers Pipeline (Phase 4 Action) [DECISION-BIZ-3]
- **Crit source:** Business CRITICAL ("no first-10-customers named pipeline")
- **Phase 1 doc affected:** Doc A §6 (GTM hypotheses)
- **Recommendation:** Nelson assembles named list of 10 reachable customer-prospects in EU-regulated fintech (BaFin-supervised), healthcare-AI (under EU MDR + AI Act intersection), or insurance-AI (Munich Re portfolio). Warm intros via DB1 / 468 / Cherry investors.
- **Decision:** **ACCEPT** — flagged as Q-3-11 in Master Vision v1 §12.3. **Action assigned to Nelson, target completion before next fundraising conversation.**
- **Rationale:** Without a named pipeline, the fundraising conversation stalls regardless of TAM/SAM/SOM (Phase 2 Business CRITICAL). The named pipeline is also operationally useful: it identifies first-customer feedback channels for V2-α validation.
- **Reversibility:** HIGH (list can be updated quarterly)
- **Review-after:** before next fundraising conversation OR Q4-2026, whichever first

---

## 2026-05-12: TAM/SAM/SOM Bottom-Up Math (Phase 4 Action) [DECISION-BIZ-4]
- **Crit source:** Business CRITICAL ("no TAM/SAM/SOM anywhere — fundraising-blocking")
- **Phase 1 doc affected:** Doc A §2 (Two-Market Positioning), Doc A §6 (GTM)
- **Recommendation:** Publish bottom-up TAM/SAM/SOM math. Phase 2 Business contributed back-of-envelope: AI-memory-infra SAM ~$10-45M, EU AI Act compliance TAM €25M-€750M, Verifiable Second Brain ceiling €135K-€1.4M SOM, Atlas initial SOM target €1M-€5M ARR by end-2026.
- **Decision:** **ACCEPT, refine with analyst.** Master Vision v1 §9.2 carries Phase-2 back-of-envelope as `[Atlas-team-assumption-flag, Phase 4 verify]`. Phase 4 includes ~€5K analyst spend OR Nelson-led validation conversations to firm up.
- **Rationale:** Phase 2 Business correctly framed: fundraising-blocking gap. Math doesn't need to be perfect for first round; it needs to be defensible. Back-of-envelope refined with even 5-10 customer-prospect conversations is fundable; absence of any number is not.
- **Reversibility:** HIGH (numbers update with new data)
- **Review-after:** before next fundraising conversation

---

## 2026-05-12: Insurance-Pricing Substrate Pitch Deferred to V2-γ [DECISION-BIZ-5]
- **Crit source:** Business + Compliance (H-7 insurance regulation framing missing) + Compliance H-5 (AILD withdrawn)
- **Phase 1 doc affected:** Doc A §4.2
- **Recommendation:** Postpone AI-Liability-Insurance pitch to V2-γ until counsel work is funded + first-deployment evidence accumulates.
- **Decision:** **DEFER to V2-γ.** Master Vision v1 §4.3 + §9.1 carries the deferral.
- **Rationale:** Three counsel disciplines intersect (PLD + Solvency II + EIOPA AI Opinion). Without an insurance-underwriter counterparty conversation (Munich Re HSB unit lead candidate), the value-prop is fully speculative. Costs are higher than V2-α benefit; defer until V2-γ when real-world deployment evidence makes the insurance conversation specific instead of abstract.
- **Reversibility:** HIGH (just timeline)
- **Review-after:** V2-α launch + first 30-day deployment metrics

---

## 2026-05-12: Lyrie ATP Compatibility Commit [DECISION-BIZ-6]
- **Crit source:** Business Concrete Proposal (commit Doc A §4.3 Agent Passports to ATP-compatibility as alias)
- **Phase 1 doc affected:** Doc A §4.3 (Agent Passports), Doc D §5.3 (Lyrie ATP entry)
- **Recommendation:** Commit Atlas's Agent Passport to ATP-compatibility as alias (one schema, two namespaces) rather than parallel scheme.
- **Decision:** **ACCEPT in principle, hold final commit pending Lyrie ATP IETF-track status confirmation.** Q-3-16 in Master Vision v1 §12.3. ATP-compatibility is design constraint for V2-γ DID schema work.
- **Rationale:** Lyrie ATP is Anthropic-CVP-accepted ($2M preseed May 2026, IETF-track). Industry-standard agent-identity protocol if it lands as IETF RFC. Atlas being ATP-compatible widens reach; being parallel-but-incompatible narrows it. Cost of compatibility-as-alias is low. Risk: ATP doesn't land at IETF and Atlas wasted compatibility effort — low risk, low cost.
- **Reversibility:** MEDIUM (schema-level decision; reversible pre-V2-γ ship)
- **Review-after:** Lyrie ATP IETF-track milestones

---

## 2026-05-12: New Risk Matrix Entries [DECISION-RISK-1]
- **Crit source:** Compliance (R-L-03 Schrems II, R-L-04 Conformity-Assessment-Substitution, R-L-05 EU AI Office Implementing-Act Drift); Business (R-S-08 Anthropic/OpenAI Co-Option, R-B-01 Fundraising-Blocking Market Sizing Gap)
- **Phase 1 doc affected:** Doc C
- **Recommendation:** Add 5 new risk entries to Doc C with full Probability×Impact×Detectability×Reversibility scoring + mitigation + owner + review-cadence.
- **Decision:** **ACCEPT all 5** — integrated into Master Vision v1 §6 Risk Matrix v1 changes table.
- **Rationale:** Each entry surfaces a Phase-2-identified risk that wasn't in Phase 1's matrix. None are speculative; all are concrete based on either legal-research evidence (R-L-03 Schrems III pending, R-L-05 AI Office rolling Implementing Acts) or business-strategic-scenario (R-S-08 specific Q4-2026 competitive playbook).
- **Reversibility:** HIGH (matrix entries update with new info)
- **Review-after:** quarterly risk matrix review (NEW R-L-05 mitigation includes quarterly tracking cadence)

---

## 2026-05-12: Obsidian Plugin First (Second Brain Validation) [DECISION-PRODUCT-2]
- **Crit source:** Competitive Doc D §3.1 + Business + Product (Demo 4 deferral)
- **Phase 1 doc affected:** Doc A §2.1 (Verifiable Second Brain market entry), Doc D §3.1
- **Recommendation:** Ship Obsidian-plugin micro-MVP (1-2 weeks) before committing Atlas-native PKM product surface. Plugin: sign-every-edit + Rekor-anchor + `[verified]` badge in note UI. Validates Verifiable-Second-Brain market without V2-α/β/γ-stack maturation.
- **Decision:** **ACCEPT** — Master Vision v1 §3.2 carries the plugin-first path. Decoupled from V2-α/β/γ Welle sequence.
- **Rationale:** Obsidian has 2750+ plugins and zero signature/verification plugin. Literal white-space test. Cost ~1-2 weeks engineering; signal is fast (download counts, plugin-store rating, community feedback). Avoids the Phase-2-Product-CRITICAL phantom-commitment to a full Atlas-native PKM client that doesn't exist.
- **Reversibility:** HIGH (plugin can be deprecated; learnings are durable)
- **Review-after:** 30 days post-plugin-launch (download + retention data)

---

---

## 2026-05-12: V2-α DB Primary Flip (ArcadeDB primary, FalkorDB fallback) [DECISION-DB-4]
- **Crit source:** V2-α Welle 2 comparative spike `docs/V2-ALPHA-DB-SPIKE.md` §8 Recommendation
- **Phase 1 doc affected:** Doc B §2.3 (FalkorDB primary), Doc D §4.4 (FalkorDB ranked above ArcadeDB)
- **Master Plan affected:** §3 Three-Layer Trust Architecture (Layer 2 DB), §4 Risk Matrix R-L-02, §6 V2-α Foundation scope, §11 Reference Pointers
- **Recommendation (from Welle 2 spike):** ArcadeDB Apache-2.0 as V2-α primary; FalkorDB as performance-validation fallback. MEDIUM-HIGH confidence. Deciding factors (weighted): License (HIGH weight — ArcadeDB wins; SSPLv1 §13 structurally incompatible with Atlas open-core hosted-service tier) + Projection-determinism cost (MEDIUM-HIGH — ArcadeDB's built-in `ORDER BY @rid` + schema-required mode reduces canonicalisation tooling cost ~30%) + Self-hosted-tier deployment simplicity (MEDIUM — ArcadeDB embedded mode lets self-hosted Atlas ship as single-process server).
- **Decision:** **ACCEPT.** Master Plan + Master Vision graph-DB table flipped to ArcadeDB primary. Spike committed on master as `docs/V2-ALPHA-DB-SPIKE.md`.
- **Rationale:** License compatibility is the decisive factor for Atlas's open-core monetization model — SSPLv1 §13 would require per-deployment commercial-license negotiation OR Atlas open-sourcing the entire hosted-service operational stack under SSPL. Apache-2.0 eliminates this burden across every Atlas tier (self-hosted, Personal/Team, Enterprise, white-label partner). Projection-determinism cost is a secondary advantage. Performance differential is acceptable at year-1 scale (10M events / workspace expected); FalkorDB's GraphBLAS edge appears only at very-large-scale traversals beyond Atlas's year-1 reality.
- **Confidence:** MEDIUM-HIGH. Raise to HIGH would require (a) actual benchmark harness Welle 2b validation, (b) counsel-validated SSPL §13 opinion confirming the engineer-perspective analysis, (c) operator-runbook validation of ArcadeDB EU-data-residency deployment.
- **Reversibility:** MEDIUM-HIGH. Atlas's Three-Layer Architecture (Layer 2 derivative of Layer 1) means swapping Layer-2 DBs is a re-projection operation, not a data-migration operation. Reversal cost: 1-2 sessions of projector rewrite (Cypher dialect adjustment) + replay-from-events.jsonl + doc updates. No data loss. Customer downtime zero via dual-write transition.
- **Open question:** Should V2-α DB lock wait for Welle 2b actual-benchmark spike, or proceed on Welle 2 public-knowledge-recommendation? Nelson decision.
- **Review-after:** V2-α Welle 3 (Projector skeleton) implementation begins against the locked choice; Welle 3 should surface any Cypher-subset incompatibilities in ArcadeDB that would lower confidence.

---

---

## 2026-05-14: Welle 17b ArcadeDB Driver Implementation [DECISION-ARCH-W17b]
- **Source:** V2-β Welle 17b (PR #90, master commit `d216844`). Parallel `code-reviewer` + `security-reviewer` dispatch per Atlas Standing Protocol Lesson #8.
- **Master Plan affected:** §6 Welle Decomposition (Phase 10 SHIPPED), `V2-BETA-ORCHESTRATION-PLAN.md`, `V2-BETA-DEPENDENCY-GRAPH.md`.
- **Decision:** **ACCEPT.** `ArcadeDbBackend` shipped as production driver against ArcadeDB Server-mode HTTP API per ADR-Atlas-010 §4 sub-decisions 1-8. Sub-module split `crates/atlas-projector/src/backend/arcadedb/{mod.rs, client.rs, cypher.rs}` (~1860 LOC). `reqwest 0.12` + `rustls-tls` + `blocking` features added; matches `atlas-signer` TLS posture (uniform `rustls` across workspace, no `openssl-sys`).
- **W17a carry-over MEDIUM final disposition:**
  - **#2 serde_json depth+size cap:** CLOSED. `check_value_depth_and_size` called at every HTTP-response → `Vertex`/`Edge` boundary in `cypher.rs::parse_vertex_row` and `parse_edge_row`. Defaults `max_depth=32`, `max_bytes=64*1024`.
  - **#3 WorkspaceId validation guard:** CLOSED. `check_workspace_id` called as FIRST line of `ArcadeDbBackend::begin()` + `vertices_sorted` + `edges_sorted`. W17b reviewer-fix added a second validation layer in `db_name_for_workspace` that rejects characters incompatible with ArcadeDB db-name rules (`[a-zA-Z0-9_]` allowlist post-hyphen-replacement). Closes the `format!("create database {db_name}")` admin-command injection surface that `check_workspace_id` alone did not cover.
  - **#4 begin() lifetime evaluation:** CLOSED. ALREADY RESOLVED by W17a-cleanup (`'static`); W17b's `ArcadeDbTxn` honours via owned fields end-to-end (cloned `reqwest::Client`, owned `db_name`/`session_id`/`workspace_id`/`BasicAuth`). `assert_static::<ArcadeDbTxn>` compile-time check pins it.
  - **#5 MalformedEntityUuid umbrella variant for edges:** V2-γ-DEFERRED (unchanged from W17a + W17a-cleanup decision). Broader error-enum refactor is out of W17b scope and not blocking W18 Mem0g.
- **W17b reviewer-fix in-commit findings:**
  - **HIGH-1 (`run_command` Value-return latent bypass):** CLOSED. Return type narrowed to `ProjectorResult<()>`. All current callers discarded the value; removing the return surface prevents future callers from accidentally bypassing the ADR-011 §4.3 #12 depth/size cap.
  - **HIGH-2 (admin-command injection):** CLOSED. See #3 above.
  - **MEDIUM-1 (SecretString visibility):** CLOSED. `SecretString` + `BasicAuth.{username,password}` tightened to `pub(crate)`.
  - **MEDIUM-2/3 (Debug userinfo leak + HTTPS posture):** CLOSED. `ArcadeDbBackend::new` rejects URLs carrying userinfo and rejects schemes other than http/https. Plaintext HTTP retained for local-dev docker-compose §4.7 with documented runbook-requires-HTTPS note.
  - **LOW-1 (unbounded body read):** CLOSED. `ensure_database_exists` bounded to 512 bytes.
  - **15 clippy doc_lazy_continuation lints:** CLOSED.
- **Confidence:** HIGH. byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces through the trait surface; clippy `-D warnings` clean; 153 tests green; trait public-API unchanged (only one doc-comment paragraph in `backend/mod.rs` touched for clippy fix).
- **Reversibility:** MEDIUM-HIGH. The driver lives entirely behind `GraphStateBackend`; reverting to in-memory-only would be one trait-impl deletion + dep removal. ArcadeDB schema is lazy-created per workspace; no data-migration concern at this stage.
- **Open question (OQ for W17c):** cross-backend byte-determinism test (`tests/cross_backend_byte_determinism.rs`) EXISTS, compiles, and is `#[ignore]`-gated behind `ATLAS_ARCADEDB_URL`. The live byte-pin reproduction through the ArcadeDB path is the W17c CI gate (Docker-Compose sidecar). If W17c surfaces a byte-equivalence gap, the ADR-010 §4.9 adapter contract or ADR-011 §4 trait default needs adjustment.
- **Review-after:** W17c integration test workflow + benchmark capture replaces ADR-010 §4.10 estimates with measured numbers (embedded-mode reconsideration trigger at p99 > 15 ms).

---

---

## 2026-05-14: Welle 17c ArcadeDB Docker-Compose CI + W17b Cypher Hotfix [DECISION-ARCH-W17c]
- **Source:** V2-β Welle 17c (PR #92, master commit `61ef036`). Parallel `code-reviewer` + `security-reviewer` dispatch per Atlas Standing Protocol Lesson #8.
- **Master Plan affected:** §6 Welle Decomposition (Phase 11 SHIPPED → Phase 12 W18 next).
- **Decision:** **ACCEPT.** New `.github/workflows/atlas-arcadedb-smoke.yml` Linux lane + `infra/docker-compose.arcadedb-smoke.yml` (ArcadeDB 24.10.1 sidecar) + `tests/arcadedb_benchmark.rs` (B1/B2/B3) + `tools/run-arcadedb-smoke-local.sh`. Same PR atomically lands the W17b Cypher hotfix discovered when the CI test first ran live.
- **W17b regressions surfaced + fixed:**
  - **Cypher reserved param-name collision:** ArcadeDB 24.10.1 silently empties result sets when a query binds `$from` or `$to` (collide with SQL `CREATE EDGE ... FROM ... TO ...` keywords); `$label` raises `IllegalArgumentException` (TinkerPop `T.label` reserved token). `upsert_edge_command` renamed to `$src` / `$dst` / `$lbl` and stored edge-property `label` renamed to `edge_label` (`parse_edge_row` translates back). Trait surface + public API unchanged.
  - **Edge type not auto-registered by MERGE:** ArcadeDB Cypher's `MERGE (a)-[r:Edge]->(b)` silently no-ops if Edge type doesn't yet exist (CREATE would auto-register). `ensure_schema_types_exist` added to `ArcadeDbBackend`: single atomic Cypher `CREATE ... WITH ... DETACH DELETE` statement registers both Vertex and Edge types and cleans up sentinels in one HTTP roundtrip; idempotent across a per-(backend, db_name) `Arc<Mutex<HashSet<String>>>` cache; lock NOT held across HTTP.
- **W17c reviewer-fix findings (all in-commit):**
  - **HIGH-1 (schema-bootstrap orphan window):** CLOSED. Single combined CREATE+DETACH DELETE Cypher statement is atomic from the client's perspective.
  - **MEDIUM-1 (`dtolnay/rust-toolchain` branch-tip SHA):** DOCUMENTED. Matches Atlas convention across all workflows; pin SHA is immutable even though it's branch-tip.
  - **MEDIUM-2 (healthcheck cmdline password leak):** CLOSED. ArcadeDB `/api/v1/ready` is unauthenticated; `curl -fsS http://localhost:2480/api/v1/ready` (no credentials).
  - **MEDIUM-3 (Mutex TOCTOU doc accuracy):** CLOSED. Doc-comment now says "at most once UNDER CONTENTION-FREE CONDITIONS"; the combined CREATE+DELETE statement is idempotent under contention.
  - **MEDIUM-4 (two password env vars):** DOCUMENTED. By design.
  - **LOW-1 (missing `restart: "no"`):** CLOSED.
  - **LOW-2 (missing `set +x` guard):** CLOSED.
  - **H-2 (B1 documentation gap):** CLOSED. Module-level B1 description now explicitly says "NOT the authoritative byte-pin gate".
- **First live byte-pin reproduction through ArcadeDB:** `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduced through the live ArcadeDB driver in CI (PR #92's atlas-arcadedb-smoke run). InMemoryBackend and ArcadeDbBackend hex-identical.
- **Bench baseline (post-fix, Windows Docker Desktop + WSL2 local):**
  - B2 incremental_upsert (n=200): p50=24.3 ms / p95=47.7 ms / p99=56.7 ms (V2-α InMem baseline ~50 µs; ArcadeDB ADR-010 §4.10 estimate 300-500 µs).
  - B3 sorted_read_vertices_50v (n=100): p50=10.0 ms / p95=14.2 ms / p99=26.1 ms.
  - B3 sorted_read_edges_100e (n=100): p50=16.4 ms / p95=22.1 ms / p99=26.0 ms.
  - Linux CI numbers expected substantially faster; concrete capture in PR #92 artifact, archived 30 days.
- **T2 trigger status (ADR-010 §4.4):** **NOT FIRED at CI scale.** T2's depth-3 read p99 > 15 ms at 10M-vertex workspace is a deployment-telemetry observation; CI's 50-vertex sorted-read p99 = 26 ms ≠ T2 (different scale, different operation). Operator-runbook §16 deployment-validation captures the real T2 signal post-W19 customer-first deployment.
- **FalkorDB fallback trigger T1 (≥3 Cypher rewrites needed):** **NOT FIRED.** W17c surfaced 2 Cypher quirks (param-name reservation + Edge type auto-register), but BOTH have small in-driver workarounds that preserve the ArcadeDB Cypher subset. No need to rewrite ≥3 queries; no architectural change required.
- **Confidence:** HIGH. byte-pin reproduces; clippy `-D warnings` clean; trait surface stable; 153 tests green; CI workflow runs green in <90 s end-to-end.
- **Reversibility:** HIGH. The CI lane + bench file are additive; the driver hotfix is a small surgical change in `cypher.rs` (param renames + 1 doc-comment + 1 method addition).
- **Open question (OQ for W18 / V2-γ):** ArcadeDB's `dtolnay/rust-toolchain@<branch-tip-SHA>` pin: should Atlas adopt a tool-versioning policy that resolves to immutable tag SHAs only? Documented in workflow comment for now; review at V2-γ supply-chain hardening pass.
- **Review-after:** W18 Mem0g Layer-3 cache build-out (ADR-Atlas-012). T2 deployment-telemetry validation at first customer 10M-event workspace (post-W19 ship).

---

---

## 2026-05-15: Welle 18 Phase A Mem0g Layer-3 cache design [DECISION-ARCH-W18]
- **Source:** V2-β Welle 18 Phase A (PR #95, master commit `3f228be`). Parallel `code-reviewer` + `security-reviewer` dispatch per Atlas Standing Protocol Lesson #8.
- **Master Plan affected:** §6 Welle Decomposition (Phase 12 SHIPPED → Phase 13 W18b implementation next), `V2-BETA-ORCHESTRATION-PLAN.md`, `V2-BETA-DEPENDENCY-GRAPH.md`.
- **Decision:** **ACCEPT.** Three NEW design docs: `docs/V2-BETA-MEM0G-SPIKE.md` (~520 lines, 13 sections, comparative spike); `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` (~430 lines, 9 sections, **8 binding sub-decisions**); `.handoff/v2-beta-welle-18-plan.md` (~280 lines + ready-to-dispatch W18b subagent skeleton). 1158 insertions total; zero Rust touched.
- **Critical clarification surfaced:** *"Mem0g"* is a research-paper name (arXiv:2504.19413, mem0ai team, 2026), not a separate product — it's `mem0` configured with graph-mode enabled (the `--graph` CLI flag was removed from main branch in 2026 releases in favour of per-project config). This does NOT change the master-plan §3 Layer-3 spec (which is implementation-neutral) but it does mean Atlas's Layer-3 implementation is named via the **concept** (Mem0g) while binding to Atlas-controlled Rust components.
- **Locked implementation choice (ADR §4 sub-decision #1):** `lancedb 0.29.0` + `fastembed-rs 5.13.4` paired. Both Apache-2.0, both pure-Rust embedded, NEW workspace member crate `crates/atlas-mem0g/`. Mem0-Python rejected on three independent blockers: (a) Python runtime co-resident with Atlas violates Hermes-skill `npx` distribution constraint; (b) cloud-default OpenAI embedder is non-deterministic + third-party vendor on trust-substrate critical path; (c) secure-delete delegated, Atlas does NOT control the primitive. Mem0 the company remains a strong **partner** candidate (cross-promotional adapter without using their software internally). Qdrant sidecar reserved as documented pivot (LP1-LP5 trigger thresholds in spike §9).
- **Locked supply-chain controls (ADR §4 sub-decision #2):** `fastembed = "=5.13.4"` exact-version pin pending W18b cross-platform determinism verification. Atlas-controlled fail-closed model download via three Atlas-source-pinned `const` values (HuggingFace revision SHA + ONNX file SHA256 + URL pin); re-verification at every cold start; operator-runbook documents model rotation process.
- **Locked secure-delete primitive (ADR §4 sub-decision #4):** pre-capture-then-lock-then-overwrite protocol (closes security-reviewer HIGH-1 TOCTOU race). Sequence: acquire write lock → pre-capture fragment paths via Lance fragment-metadata API BEFORE any delete → `lancedb::Table::delete()` → `cleanup_old_versions(0)` → pre-capture HNSW `_indices/` paths → for each path (fragments + indices): random-fillbytes + `fdatasync` + `remove_file` → release lock → emit `embedding_erased` audit-event. Snippet field co-located in same Lance fragment as embedding (covered by step 6); HNSW index files overwritten by default (option (a)) to close graph-neighbourhood residual-leak. SSD wear-leveling caveat documented in `DECISION-SEC-5` footnote; V2-γ cryptographic-erasure deferral noted.
- **Locked GDPR audit-event shape (ADR §4 sub-decision #5):** new event-kind `embedding_erased` with EU-DPA-evidentiary payload (`event_id` + `workspace_id` + `erased_at` + optional `requestor_did` defaulting to operator DID + optional `reason_code` defaulting to `"operator_purge"`). The audit-event itself is a Layer-1 signed event (standard Atlas COSE_Sign1 envelope + hash chain + Rekor anchor); never subject to secure-delete itself (Layer-1 records of erasure persist for regulatory traceability). Append-only refusal of duplicates with semantic-mismatch note for `MissingPayloadField` variant reuse (broader error-enum cleanup is V2-γ-deferred consistent with `DECISION-ARCH-W17b` carry-over #5).
- **Locked cache-invalidation strategy (ADR §4 sub-decision #6, post-reviewer revision):** hybrid Layer-1-native triple — TTL + erasure-event + Layer-1 head divergence. The original draft's third trigger (Layer-2 `graph_state_hash` cross-check) was flagged by BOTH reviewers as a Layer-authority contradiction (Mem0g indexes Layer 1 directly, NOT Layer 2 — Phase-2 Architect H-3 correction). Replaced with Layer-1 head-divergence detection; Layer-2 cross-check reframed as opportunistic defence-in-depth ONLY, NOT load-bearing for cache validity.
- **Locked crate boundary (ADR §4 sub-decision #7):** NEW workspace member crate `crates/atlas-mem0g/`. Reasoning: (a) clean cargo + license boundary (LanceDB + fastembed-rs + Arrow + DataFusion are a substantial dep-tree; isolating in own crate keeps `atlas-projector`'s dep audit smaller); (b) pivot encapsulation (a future `crates/atlas-mem0g-qdrant/` parallel-crate is the cleanest swap path); (c) independent CI + reviewer dispatch lane.
- **Locked bench-shape (ADR §4 sub-decision #8):** B4 cache-hit semantic-search latency p50/p95/p99 (target <10 ms p99); B5 cache-miss-with-rebuild full-rebuild cost (target <30 sec for 10K-event workspace); B6 secure-delete primitive correctness incl. concurrent-write race-test. PLUS timing-side-channel mitigation (security-reviewer MEDIUM-5): response-time normalisation (default 50 ms minimum) in W18b's `apps/atlas-web/.../semantic-search/route.ts` Read-API endpoint; `embedding_hash` cache-key restricted to internal lookup only.
- **Reviewer-dispatch outcome:** parallel `code-reviewer` + `security-reviewer` (Atlas Standing Protocol Lesson #8). **0 CRITICAL** + **2 HIGH** (security: H-1 secure-delete TOCTOU race; H-2 model-download URL pinning) + **10 MEDIUM** (code: 5 — sub-decision count drift "6+" vs "8", fastembed version granularity, sync-vs-async trait, cache-invalidation Layer-authority contradiction, plan-doc stale "TBD"; security: 5 — snippet field overwrite coverage, HNSW index files, audit-event EU-DPA-evidentiary completeness, trust-claim language drift, embedding_hash timing side-channel) + **4 LOW** (ADR stale parenthetical, plan-doc section-count, Morris et al. 2023 model-specific applicability gap, cache-invalidation Layer-authority cross-reference). All HIGH + MEDIUM applied in-commit per Atlas Standing Protocol Lesson #3; all LOW also applied.
- **Architectural posture preserved:** embeddings live OUTSIDE canonicalisation pipeline (lib.rs invariant #3 honoured — no floats in canonical bytes); byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces (zero Rust touched); Layer 3 NEVER trust-authoritative — every `SemanticHit` cites back to Layer-1 `event_uuid`; consumer can drop to Layer 1 + offline WASM verifier independently even when Layer 3 is corrupted/unavailable; Hermes-skill `npx` distribution intact (no Python, no second JVM); all trust-claim language qualified counsel-pending per `DECISION-COUNSEL-1` / `DECISION-COMPLIANCE-3` / `DECISION-COMPLIANCE-4` (Path-B-current with Path-A salt-redesign fallback; "regulator-friendly" not "regulator-approved").
- **Confidence:** HIGH on license + distribution + Hermes-skill compatibility + cite-back trust property + bench-shape design; MEDIUM-HIGH on embedding-determinism under pinned conditions (cross-platform W18b verification test mandatory) and filesystem-level secure-delete primitive correctness; MEDIUM on SSD-physical-erasure (acknowledged limitation; V2-γ cryptographic-erasure is the longer-term mitigation); MEDIUM-HIGH on the LanceDB dep-tree audit burden (~200 transitive crates incl. Arrow + DataFusion; counsel-engagement and Atlas's quarterly dep-tree-CVE review must include it).
- **Reversibility:** MEDIUM-HIGH. LanceDB → Qdrant pivot is encapsulated behind `SemanticCacheBackend` Rust trait (~3 sessions if any LP1-LP5 trigger fires; trait + embedder + secure-delete-wrapper code reused 100%). Embedder swap is config change + rebuild (~0.5 session). Crate boundary swap is trivial. Cache-invalidation policy change is operator-runbook config.
- **Open question (OQ for W18b / V2-γ):** Atlas-side cross-platform determinism test outcomes (V1-V4 in spike §12) — if Windows fails, formalise event_uuid-only cache-key fallback policy in operator-runbook. SSD-physical-erasure + per-tenant cryptographic-erasure deferred to V2-γ. Multi-region LanceDB replication deferred (Qdrant pivot is the better answer if multi-region becomes hard requirement). Embedder-version-rotation policy locked in V2-γ operator-runbook. Re-evaluate Mem0 partnership angle for V2-γ. Lance v2.2 `_deletion_files` semantics verification before adopting Lance 0.30+. Read-API integration pattern (transparent vs explicit endpoint) decided in W18b ADR amendment.
- **Review-after:** W18b implementation + cross-platform determinism + secure-delete-correctness + B4/B5/B6 benches captured. T2-equivalent fallback trigger for Layer 3 deployment-telemetry validation at first customer 10M-event workspace (post-W19 ship).

---

## 2026-05-14: Welle 18b Mem0g Layer-3 cache implementation [DECISION-ARCH-W18b]
- **Source:** V2-β Welle 18b (PR #97, master commit `2f2238b`). Initial commit `80f6957` + reviewer fix-commit `717922c` (squash-merged). Parallel `code-reviewer` + `security-reviewer` dispatch per Atlas Standing Protocol Lesson #8.
- **Master Plan affected:** §6 Welle Decomposition (Phase 13 SHIPPED → Phase 14 W19 ship-convergence next; W18c queued as parallel-track pre-V2-β-1-ship gate), `V2-BETA-ORCHESTRATION-PLAN.md`, `V2-BETA-DEPENDENCY-GRAPH.md`.
- **Decision:** **ACCEPT.** NEW workspace member crate `crates/atlas-mem0g/` (~2300 LOC across `Cargo.toml` + 4 src modules + 3 test modules + new `atlas-mem0g-smoke` CI workflow + semantic-search Read-API endpoint + plan-doc). 9892 net-additions across 16 files; 578 tests pass workspace-wide; clippy `-D warnings` zero warnings; byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces.
- **Implementation posture (W17a-pattern Phase-A-scaffold):** trait surface + 7-step secure-delete protocol + supply-chain verification path + `embedding_erased` dispatch arm + `embedding_erasures` GraphState extension + canonical extension + Read-API endpoint with timing-normalisation + CI workflow are all production-shape. **Deferred to W18c follow-on welle:** LanceDB ANN/search body fill-in (Mem0gError::Backend placeholders return until V1-V4 verification gaps close per spike §12); fastembed `try_new_from_user_defined` wiring (requires 3 additional tokenizer-file SHA-256 pins). **Deferred to Nelson pre-V2-β-1-ship lift:** real supply-chain constants (`HF_REVISION_SHA` + `ONNX_SHA256` + `MODEL_URL` from HuggingFace `BAAI/bge-small-en-v1.5`). Fail-closed posture preserves security property end-to-end: `AtlasEmbedder::new` returns `Mem0gError::Embedder("supply-chain gate: ...")` unconditionally until Nelson lifts.
- **ADR §4 sub-decision implementation status:** 1 CHECK (trait + crate + feature flag) · 2 PARTIAL-fail-closed (supply-chain placeholders) · 3 CHECK (event_uuid cite-back ALWAYS populated) · 4 CHECK (7-step protocol; FS-walk fallback for pre-capture functionally equivalent for GDPR Art. 17 byte-overwrite — operates on real files on disk regardless of LanceDB API state) · 5 CHECK (`embedding_erased` dispatch arm mirrors `apply_anchor_created` exactly, incl. duplicate-refusal semantic-mismatch doc-comment) · 6 CHECK (hybrid TTL + on-event + Layer-1-head-divergence triple) · 7 CHECK (`crates/atlas-mem0g/` workspace member) · 8 CHECK (B4/B5/B6 + timing-normalisation default 50 ms).
- **Reviewer-dispatch outcome:** parallel `code-reviewer` + `security-reviewer`. **0 CRITICAL + 4 unique HIGH + 6 MEDIUM + several LOW**. All HIGH + security/correctness MEDIUMs applied in-commit per Atlas Standing Protocol Lesson #3. HIGH closes: (H-1) real `sha2::Sha256` replacing blake3-prefixed placeholder string (would have silently always-failed on Nelson constant lift); (H-2) `AtlasEmbedder::new` unconditional fail-closed bypassing previous `fastembed::TextEmbedding::try_new(Default::default())` which would have downloaded a SECOND unverified copy of the model independently of Atlas's SHA check (`try_new_from_user_defined` wiring + 3 tokenizer-file pins is W18c work — deferred-with-gate posture); (H-3) `pins_are_placeholder_until_nelson_verifies` gatekeeper asserts ALL three constants + post-lift well-formedness companion test (was single-constant; partial lift would have silently passed); (H-4) `getrandom` OS CSPRNG replacing deterministic blake3-keyed PRG seeded on `(path, remaining)` (adversary with workspace storage layout could have recomputed exact overwrite pattern). MEDIUM closes: empty-string `erased_at` guard symmetric with `event_id` + `workspace_id` (regulator-evidentiary completeness); non-silent secure-delete on missing pre-captured paths (was false-attestation risk); recursive fragment walk via `walk_collect_filtered` (was single-level — would have missed `_versions/N/data-*.lance`); UTF-8 `Buffer.byteLength` body-cap (was JS `.length` UTF-16 char count); `Once::call_once` wrap of `set_var` (closes Rust 2024 UB on global `environ`); `spawn_blocking` doc clarification + `RESUME(spawn_blocking)` markers at body sites.
- **Architectural posture preserved:** embeddings live OUTSIDE canonicalisation pipeline (lib.rs invariant #3 honoured); byte-pin reproduces; Layer 3 NEVER trust-authoritative — every `SemanticHit.event_uuid` is structurally non-optional `String` at every layer (Rust struct + trait + TypeScript interface), no path can return a hit without cite-back; Hermes-skill `npx` distribution intact (no Python, no second JVM); 501 stub Read-API path also obeys timing-normalisation (no distinguishing "backend not wired" from "cache miss" at API boundary); `embedding_hash` cache-key NEVER exposed externally (no headers, no status codes, no body fields).
- **W18c follow-on welle scope (queued, pre-V2-β-1-ship blocker):** (a) Nelson supply-chain constant lift (resolve `HF_REVISION_SHA` + `ONNX_SHA256` + `MODEL_URL` from HuggingFace `BAAI/bge-small-en-v1.5`); (b) 3 additional tokenizer-file SHA-256 pins (tokenizer.json + config.json + special_tokens_map.json) + per-file download helpers; (c) fastembed `try_new_from_user_defined` wiring replacing the current unconditional fail-closed in `AtlasEmbedder::new`; (d) close V1-V4 verification gaps from spike §12 (LanceDB Windows `cleanup_old_versions` behaviour; fastembed-rs cross-platform determinism on Linux + Windows + macOS CI matrix; Lance v2.2 `_deletion_files` semantics; fastembed model size on disk measurement); (e) lift LanceDB ANN/search body stubs (currently surface `Mem0gError::Backend` placeholders). ADR-Atlas-013 reserved for any W18c design amendment.
- **CI status post-merge:** all 7 required + non-required checks green — `Verify trust-root-modifying commits` ✓, `atlas-mem0g-smoke` (NEW, ✓), `atlas-arcadedb-smoke` ✓ (byte-pin reproduces through ArcadeDB live too), `atlas-web-playwright` ✓, `hsm-byte-equivalence` ✓, `hsm-wave3-smoke` ✓, `hsm-witness-smoke` ✓.
- **SemVer impact:** SemVer-additive across workspace (new `atlas-mem0g` crate, new `embedding_erased` event-kind, new `embedding_erasures` GraphState field omit-when-empty, new `Mem0gError` variants, new Read-API endpoint). Layer-2 trait surface unchanged. `atlas-mem0g` ships at `0.1.0` (V2-β internal version aligned with workspace `2.0.0-alpha.2`).
- **Confidence:** HIGH on trait surface + protocol design + dispatch arm + byte-pin preservation + cite-back trust property + timing-normalisation correctness; MEDIUM-HIGH on FS-walk-fallback functional equivalence for GDPR Art. 17 (operates on real files regardless of LanceDB API state — recursive walk closes the versioned-sub-dir leak surfaced by MEDIUM-3 reviewer); MEDIUM on Nelson constant lift timing (depends on counsel-engagement track for embedding-erased payload validation per `DECISION-COUNSEL-1` review scope per ADR-Atlas-012 §5.4).
- **Reversibility:** MEDIUM-HIGH. `SemanticCacheBackend` trait abstracts LanceDB → Qdrant pivot per spike §9 (~3 sessions, 100% trait + embedder + secure-delete wrapper code reused). Embedder model swap is config + rebuild (~0.5 session). Scaffold-to-full-impl is W18c additive (no breaking changes to trait surface). `feature = "lancedb-backend"` posture default-OFF means Layer-3 is opt-in until W18c lifts.
- **Open question (OQ for W18c / W19 / V2-γ):** OQ-W18c-1: real supply-chain constant lift timing relative to counsel-engagement Art. 17 payload validation. OQ-W18c-2: fastembed `try_new_from_user_defined` API surface against fastembed 5.13.4 (subagent could not verify cargo-doc against fastembed in worktree context). OQ-V2-γ: cross-platform determinism CI matrix expansion to Windows + macOS; SSD-physical-erasure via SECURE_ERASE ATA + per-tenant cryptographic-erasure deferred to V2-γ per ADR-012 OQ-1.
- **Review-after:** W18c follow-on welle close-out + first customer 10M-event-workspace deployment-telemetry validation (post-W19 ship).

---

## 2026-05-15: Welle 18c Phase A Mem0g supply-chain constants lifted [DECISION-ARCH-W18c-A]
- **Source:** V2-β Welle 18c Phase A (PR #100, master commit `28700ae`). Initial commit `5946a1b` (parent-direct edit using Nelson-resolved HuggingFace values) + reviewer fix-commit `a66728e` (squash-merged). Parallel `code-reviewer` + `security-reviewer` dispatch per Atlas Standing Protocol Lesson #8.
- **Master Plan affected:** §6 Welle Decomposition (Phase 14 partially complete — W18c-A SHIPPED; W18c Phase B-D queued as engineering-pipeline parallel-track to W19 ship convergence), handoff §0z6 W18c-A SHIPPED narrative, `V2-BETA-ORCHESTRATION-PLAN.md` (Phase 14 status flip), `V2-BETA-DEPENDENCY-GRAPH.md` (W18c-A → W18c-B + W19 next).
- **Decision:** **ACCEPT.** 9 compile-in supply-chain pins lifted in `crates/atlas-mem0g/src/embedder.rs` (3 W18b TODO placeholders + 6 NEW Phase-B-prep constants) against HuggingFace `BAAI/bge-small-en-v1.5` at revision `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`. ~345 net-additions across 4 files; 577 tests pass workspace-wide (delta -1 vs handoff 578: retired W18b `pins_are_placeholder_until_nelson_verifies` gatekeeper); clippy `-D warnings` zero warnings; byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces unchanged.
- **Pin values (lifted from HuggingFace via auditable helper `tools/w18c-phase-a-resolve.sh`):** `HF_REVISION_SHA = 5c38ec7c405ec4b44b94cc5a9bb96e735b38267a` (40-char Git SHA-1) · `ONNX_SHA256 = 828e1496…b0f559940cf35` (64-char SHA-256; model.onnx FP32; 133,093,490 bytes / 126.93 MB matches spike §3.4 expected envelope — closes V4 verification gap) · `MODEL_URL = https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/<HF_REVISION_SHA>/onnx/model.onnx` · `TOKENIZER_JSON_SHA256 = d241a60d…267a1f9e5c66` · `CONFIG_JSON_SHA256 = 094f8e89…78c4306aef3084750` · `SPECIAL_TOKENS_MAP_SHA256 = b6d346be…dba59e76237ee3` · 3 tokenizer URL constants embedding `HF_REVISION_SHA` (revision-pinning invariant tested).
- **Full pin reference (canonical, unabbreviated — for byte-compare auditing per security-reviewer MEDIUM-1):**
  ```text
  HF_REVISION_SHA           = 5c38ec7c405ec4b44b94cc5a9bb96e735b38267a                                  (40-char Git SHA-1)
  ONNX_SHA256               = 828e1496d7fabb79cfa4dcd84fa38625c0d3d21da474a00f08db0f559940cf35          (64-char SHA-256)
  TOKENIZER_JSON_SHA256     = d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66          (64-char SHA-256)
  CONFIG_JSON_SHA256        = 094f8e891b932f2000c92cfc663bac4c62069f5d8af5b5278c4306aef3084750          (64-char SHA-256)
  SPECIAL_TOKENS_MAP_SHA256 = b6d346be366a7d1d48332dbc9fdf3bf8960b5d879522b7799ddba59e76237ee3          (64-char SHA-256)
  MODEL_URL                 = https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/onnx/model.onnx
  TOKENIZER_JSON_URL        = https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/tokenizer.json
  CONFIG_JSON_URL           = https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/config.json
  SPECIAL_TOKENS_MAP_URL    = https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/5c38ec7c405ec4b44b94cc5a9bb96e735b38267a/special_tokens_map.json
  ONNX_FILE_SIZE_BYTES      = 133093490                                                                  (126.93 MB; spike §3.4 envelope match)
  ```
  Cross-verifiable against `crates/atlas-mem0g/src/embedder.rs` at master commit `28700ae` (PR #100). Re-resolvable via `bash tools/w18c-phase-a-resolve.sh`.
- **Implementation posture:** Phase A is a **pure constant-lift** — `AtlasEmbedder::new` body UNCHANGED at runtime level (still returns `Mem0gError::Embedder("supply-chain gate: …")`); error message + function-level docstring + module-level docstring refreshed to point at W18c Phase B (replacing W18b "Nelson pre-merge resume" language with "Phase B engineering wiring resume"). The `fastembed::TextEmbedding::try_new(Default::default())` bypass path remains structurally unreachable per HIGH-2 W18b fix. Test surface: `pins_are_placeholder_until_nelson_verifies` RETIRED (W18b gatekeeper purpose served); `pins_well_formed_after_lift` UPGRADED to unconditional enforcement across all 9 pins (length + lowercase-hex + huggingface.co origin + revision-SHA-in-path invariants — the last invariant is the atomic-lift enforcer: any future refactor that drifts revision SHA without updating URLs trips immediately at test time); `pins_are_non_empty` extended; `_STRUCTURAL_PIN_CHECK` const-eval block extended.
- **Reviewer-dispatch outcome:** parallel `code-reviewer` + `security-reviewer`. **0 CRITICAL + 0 HIGH + 1 overlapping MEDIUM + 3-4 overlapping LOWs**. Verdict: APPROVE from both. All MEDIUMs + overlapping LOWs applied in-commit per Atlas Standing Protocol Lesson #3 (fix-commit `a66728e` on top of initial `5946a1b`). **MEDIUM close (overlap):** `tools/w18c-phase-a-resolve.sh` `python` → `python3` portability (Ubuntu 22.04+ / macOS Ventura+ default missing `python`); now resolves `python3` first, falls back to `python`, errors clearly if neither found; honours `PYTHON_BIN` env override. **LOW closes:** SHA-count off-by-one ("six SHAs" → actual "5 hash digests" — 1 × SHA-1 + 4 × SHA-256) fixed in 4 doc locations; `TMPDIR` env-var shadowing renamed to `WORK_DIR` throughout helper script; missing sha256-output format validation via NEW `validate_sha256_hex` helper invoked after every `sha256sum | cut` capture (defence-in-depth — the Rust `pins_well_formed_after_lift` test catches it post-commit, helper-side gate gives Nelson immediate-time error on resolution); stale "## Three compiled-in constants" module section header replaced with "## Compiled-in supply-chain pins (9 total: 5 hash digests + 4 URLs)"; HuggingFace API `'sha'` field hardening via `d.get('sha', '')` plus enhanced error message naming schema-change as likely cause. **LOWs deferred to W18c Phase B (engineering-pipeline-tracked):** `AtlasEmbedder::new` pre-gate model download (currently triggers ~130 MB download then fails closed — inefficient but functionally correct; Phase B replaces the fail-closed return with `try_new_from_user_defined` so the download succeeds end-to-end); ONNX file size compile-time assertion (currently doc-only at 133,093,490 bytes — defensible as content-addressed SHA-256 is the real gate; Phase B could add `ONNX_FILE_SIZE_BYTES` const + test assertion for defence-in-depth).
- **Architectural posture preserved:** trust property unchanged (Layer 3 NEVER trust-authoritative); `AtlasEmbedder::new` body identical at runtime (only error string text changes); TLS pinning via `reqwest::blocking::Client::builder().https_only(true)` semantics intact; revision-pinning invariant tested across all 4 URLs; compile-time `_STRUCTURAL_PIN_CHECK` const-eval backstop against accidental blanking.
- **CI status post-merge:** all 3 triggered checks green on master HEAD `28700ae`: `Verify trust-root-modifying commits` ✓ (6 s); `atlas-web-playwright` ✓ (4 min 9 s, triggered via `.handoff/**` path-filter doc-touch per Lesson #11); `mem0g-smoke` ✓ (1 min 11 s; first live-infra validation of post-Phase-A Layer-3 code paths). `atlas-arcadedb-smoke` + HSM workflows not triggered (path-filter scope). 577 tests / 0 failed / 7 ignored workspace-wide.
- **SemVer impact:** SemVer-additive within `atlas-mem0g` 0.1.0 (NEW `pub const` items — 6 added). Layer-2 + Layer-1 trait surface unchanged. Runtime behaviour unchanged (fail-closed gate intact). No new event-kinds. No Read-API surface change. SemVer audit: covered by V2-β SemVer surface contract document (to be created in W19 as `docs/SEMVER-AUDIT-V2.0-beta.md`).
- **Confidence:** HIGH on the constant values themselves (cryptographically computed via independent `sha256sum`; HuggingFace API SHA cross-verified by separately-downloaded ONNX file matching the asserted SHA-256); HIGH on revision-pinning invariant enforcement (tested for all 4 URLs); HIGH on fail-closed gate preservation (body unchanged at runtime); MEDIUM-HIGH on Phase B unblockedness (the 9 pins + tokenizer URLs are exactly what `try_new_from_user_defined` consumes per W18b fastembed 5.13.4 research; cargo-doc verification still owes against a non-worktree environment, but pin shape is documented in plan-doc Phase B); MEDIUM on long-tail edge cases like HuggingFace revision rotation (`.handoff/v2-beta-welle-18c-plan.md` Phase A is re-runnable via `tools/w18c-phase-a-resolve.sh` for future rotations).
- **Reversibility:** HIGH. Phase A is a pure constant-lift; reverting to W18b placeholders is mechanical (`git revert 28700ae`) and the fail-closed gate ensures revert is functionally safe (no operational paths depend on the lifted values until Phase B wires them). Future revision rotations follow the same `tools/w18c-phase-a-resolve.sh` → atomic-lift workflow.
- **Open question (OQ for W18c Phase B / W19 / V2-γ):** OQ-W18c-A-1: Phase B fastembed `try_new_from_user_defined` API surface against fastembed 5.13.4 — to be verified via `cargo doc -p fastembed --features lancedb-backend` once `lancedb-backend` builds locally. OQ-W18c-A-2: optional defence-in-depth `ONNX_FILE_SIZE_BYTES` compile-time assertion. OQ-W19: W19 release notes scaffold-posture disclosure language now reads "supply-chain pins verified" instead of "supply-chain placeholders" — release notes draft should reflect this.
- **Review-after:** W18c Phase B fastembed wiring (next engineering welle) + W19 ship convergence (independent track).
- **Demo-sketches doc note:** `.handoff/v2-demo-sketches.md` (~1283 lines, V2 marketing/strategy draft v0 with 5 demo storyboards + landing-page hero analysis + 15 open Phase-2 critique questions) remains Untracked on master post-Phase-14.5. Conceptually separate from W18c supply-chain track; will land as its own doc-only PR if/when V2 strategy track is formally resumed. Not part of this DECISION-ARCH-W18c-A.

---

**End of decisions.md.** 28 decisions documented. All Phase-2 CRITICAL findings addressed (accepted/modified/deferred/rejected with rationale). V2-α Welle 1 + Welle 2 strategic decisions added 2026-05-12. V2-β Welle 17b + 17c shipped 2026-05-14 (W17a carry-overs closed in 17b; W17b regressions surfaced + closed in 17c via live CI). V2-β Welle 18 Phase A shipped 2026-05-15 (Mem0g Layer-3 cache design, 8 binding sub-decisions). V2-β Welle 18b shipped 2026-05-14 (Mem0g Layer-3 cache implementation, W17a-pattern Phase-A-scaffold). V2-β Welle 18c Phase A shipped 2026-05-15 (Mem0g supply-chain constants lifted; W18c Phase B-D queued as engineering-pipeline parallel-track to W19 ship convergence; ADR-Atlas-013 still reserved).
