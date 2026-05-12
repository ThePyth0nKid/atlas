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

**End of decisions.md.** 23 decisions documented. All Phase-2 CRITICAL findings addressed (accepted/modified/deferred/rejected with rationale). V2-α Welle 1 + Welle 2 strategic decisions added 2026-05-12.
