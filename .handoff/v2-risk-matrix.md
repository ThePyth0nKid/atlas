# Atlas V2 — Risk Matrix v0

> **Status:** Draft v0 (2026-05-12). Written by security-reviewer agent as Phase 1 Doc C.
> Designed for Phase 2 critique. This is NOT a generic risk doc — it addresses
> the specific failure modes of the V2 pivot: verifiable knowledge graph substrate,
> Mem0g hybrid, Hermes Agent integration, and agent identities.
>
> **Bedrock principle for all risk analysis:** V1's trust invariant states that
> `events.jsonl` is the sole authoritative source of truth — Ed25519+COSE-signed,
> hash-chained, Sigstore Rekor-anchored, offline-WASM-verifiable. Every V2 risk
> is evaluated against "does the V1 trust invariant still hold under this failure mode?"

---

## Methodology

Each risk is scored on four independent dimensions:

| Dimension | Scale | Meaning |
|---|---|---|
| **Probability** | LOW / MEDIUM / HIGH / CRITICAL | Likelihood this materialises within 18 months without active mitigation |
| **Impact** | LOW / MEDIUM / HIGH / CRITICAL | Worst-case severity if it materialises (financial, technical, reputational assessed separately) |
| **Detectability** | HIGH / MEDIUM / LOW | How quickly we would observe the failure (HIGH = visible within hours/days; LOW = could run undetected for months) |
| **Reversibility** | HIGH / MEDIUM / LOW | How recoverable the situation is after detection (HIGH = fix and ship; LOW = market/reputation damage largely permanent) |

Mitigation status uses: NONE (no current protection) / PARTIAL (direction identified, not implemented) / ADEQUATE (implemented but not battle-tested) / ROBUST (implemented, tested, validated in adversarial conditions).

Risk IDs follow category prefix convention: S- (Strategic), A- (Architectural), C- (Crypto), O- (Operational), L- (Legal), V- (Vendor).

---

## Risk Categories

### Strategic / Market Risks
R-S-01: Adoption Tipping Point

### Architectural / Trust Risks
R-A-01: Projection Determinism Drift
R-A-02: Privacy vs. Public Anchoring

### Cryptographic / Crypto-Agility Risks
R-C-01: Post-Quantum Migration

### Operational / Adoption Risks
R-O-01: Performance Overhead at Scale
R-O-02: UX Complexity Barrier

### Legal / Regulatory Risks
R-L-01: GDPR Right to be Forgotten
R-L-02: FalkorDB SSPL License Trap

### Vendor / Ecosystem Risks
R-V-01: Vendor Capture by Major AI Providers
R-V-02: Mem0 Vendor Risk
R-V-03: Hermes Adoption Reversal

### Independently Identified Risks
R-A-03: Agent Identity Key Compromise and Revocation Lag
R-S-02: Regulatory Narrative Capture (EU AI Act Interpretation Lock-In)
R-O-03: Projection Rebuild Cost at Production Scale

---

## Detailed Risks

---

### R-S-01: Adoption Tipping Point

- **Category:** Strategic
- **Description:** Atlas's value compounds with usage — an agent writing into a shared, verifiable workspace is more useful when other agents and humans can query and audit that workspace. The tipping-point problem: agents adopt only if there is critical mass of verified content; critical mass only forms if agents adopt. EU-regulated verticals require Atlas but represent a narrow bootstrap wedge; general-purpose agent builders will wait for demonstrated adoption before integrating.
- **Probability:** HIGH
- **Impact:** CRITICAL (financial: no revenue; technical: V1 trust property becomes irrelevant in practice; reputational: "technically elegant but unused")
- **Detectability:** HIGH (npm download counts, GitHub stars, and integration PRs give early signal within weeks)
- **Reversibility:** MEDIUM (adoption flywheels are slow to start but, once started, do not easily reverse; failure to gain traction within 12 months is partially recoverable via pivot but strategy value is lost)
- **Current Mitigation Status:** PARTIAL
- **Mitigation Strategy:**
  - **Proactive:** Target EU-regulated verticals (Finance / Healthcare / Insurance) as the bootstrap wedge where compliance is a mandate, not a choice. One live enterprise deployment provides social proof that general-purpose builders need. EU AI Act Art. 12 (mandatory automatic event logs) comes into force 2026-08-02 — there is a hard deadline creating genuine demand.
  - **Proactive:** Hermes Agent skill integration converts Hermes's 60K+ GitHub star community into potential Atlas users. A one-click Atlas Memory Skill for Hermes lowers adoption friction to near zero for that community without requiring Atlas to build its own user base independently.
  - **Proactive:** Apache-2.0 verifier crates mean zero friction for any agent builder who wants to embed Atlas verification. Distribution is not gated on a business relationship.
  - **If it materialises:** Pivot from general-purpose substrate to narrow compliance-first product (i.e., double down on the EU AI Act niche even if general adoption stalls). Survival path exists even at low adoption volume if the compliance vertical is captured.
- **Owner:** Strategy + Product
- **Review Cadence:** Per-Welle (track adoption metrics every wave)

---

### R-A-01: Projection Determinism Drift

- **Category:** Architectural / Trust
- **Description:** The graph DB (FalkorDB) and Mem0g retrieval cache are declared deterministically rebuildable projections of `events.jsonl`. If the projection algorithm drifts — due to an LLM extraction step with non-deterministic output, a projector schema upgrade without version gating, a FalkorDB upsert ordering change, or silent data corruption — the graph DB diverges from the authoritative event log without any visible signal. The V1 trust invariant breaks **invisibly**: the graph shows facts the signed log does not support, or vice versa. An auditor running the WASM verifier against events.jsonl would get a different picture than the graph UI shows.
- **Probability:** HIGH (multiple non-determinism injection points exist; projector is not yet built and will be complex)
- **Impact:** CRITICAL (financial: enterprise customers would be misled by unverifiable graph data; technical: V1 trust invariant broken at the read-side; reputational: "Atlas claimed verifiability but drifted silently" is a fatal credibility wound)
- **Detectability:** LOW (divergence can accumulate for weeks; no current CI gate compares graph DB state against events.jsonl hash; only a full rebuild-and-compare detects it)
- **Reversibility:** MEDIUM (the log is still authoritative — a full projection rebuild recovers truth; but any period of drift has already misled users/auditors, and that reputational exposure does not recover)
- **Current Mitigation Status:** PARTIAL (V2-vision-knowledge-graph-layer.md §2.1 names the invariant and §2.2(c) recommends deterministic projectors for trust-bearing paths, but no CI gate exists yet)
- **Mitigation Strategy:**
  - **Proactive (critical):** Projection integrity CI gate in V2-α. Every CI run: (a) replay events.jsonl through the projector into a fresh FalkorDB instance; (b) compute a canonical hash of the resulting graph state (sorted entity list + property fingerprint); (c) compare against the expected hash stored in a `.projection-integrity.json` file pinned in the repo. This is analogous to the `signing_input_byte_determinism_pin` test in SECURITY-NOTES.md that already locks V1's signing pipeline.
  - **Proactive:** Projector schema versioning with explicit upgrade paths. Every projector version change produces a different `projection_schema_version` tag on all derived nodes. Auditor tooling rejects a graph node whose `projection_schema_version` does not match the projector the auditor is running.
  - **Proactive:** Strict separation: NO LLM extraction in the trust-bearing projector path. LLM extraction (e.g., Graphiti) goes on the retrieval-enhancement layer only. The authoritative projector uses deterministic structured extraction from event payloads. This is V2-vision §2.2(c) — it must be a hard rule, not a recommendation.
  - **If it materialises:** Full projection rebuild from events.jsonl (the log is authoritative). Publish post-mortem documenting the drift period. Implement the CI gate that should have been there. The trust invariant recovers; the reputational cost does not fully recover.
- **Owner:** Engineering
- **Review Cadence:** Per-Welle (the CI gate must exist before V2-α ships anything to users)

---

### R-A-02: Privacy vs. Public Anchoring

- **Category:** Architectural / Trust
- **Description:** Sigstore Rekor is a public, transparency-log-style append-only ledger. Any payload submitted to Rekor is permanently public. Atlas currently anchors `dag_tip` and `bundle_hash` hashes (not raw content), but the metadata embedded in a Rekor entry (the `atlas-dag-tip-v1:` domain prefix + hex hash) could allow correlation attacks against known content. Enterprise customers (Finance / Healthcare / Insurance — the exact verticals Atlas targets for compliance-driven adoption) will have confidentiality requirements that may be incompatible with any public anchoring. A single compliance officer saying "you write anything to a public log? Rejected." blocks an enterprise deal.
- **Probability:** MEDIUM (hashes alone are pseudonymous, not plaintext; but enterprise legal and compliance teams often reject any public external disclosure regardless of technical analysis)
- **Impact:** HIGH (financial: closes off the enterprise compliance vertical; technical: requires architectural split between public-witness and private-federation tiers; reputational: creates a "Atlas is not enterprise-ready" narrative if publicly surfaced by a competitor)
- **Detectability:** HIGH (will become apparent in the first enterprise procurement conversation that reaches a legal/compliance review)
- **Reversibility:** HIGH (private federation tier is a planned V2 feature, not a re-architecture; it is additive; but the narrative "we built public-only first" has to be managed)
- **Current Mitigation Status:** PARTIAL (session handoff §4 mentions "private federation tier next to public-witness tier" but no design exists yet)
- **Mitigation Strategy:**
  - **Proactive:** Design the private federation tier as a first-class V2 concept, not an afterthought. The architecture should have three anchoring modes from day one: (a) public Sigstore Rekor (full transparency, open-source projects / regulatory attestations); (b) federated private witness (enterprise self-hosted witness key, no public log); (c) air-gapped (offline, WASM-only verification, no external anchoring). Customers choose per-workspace.
  - **Proactive:** Lead enterprise sales conversations with "we support private witness keys — the public Sigstore log is opt-in." Do not let prospects assume public-only.
  - **If it materialises:** The V1 trust invariant survives: the WASM verifier validates signed events offline regardless of whether a Rekor anchor exists. "No public anchor" is already a valid Atlas workspace state (lenient mode). The enterprise story becomes "verifiable without external disclosure."
- **Owner:** Engineering + Product
- **Review Cadence:** Per-Welle

---

### R-C-01: Post-Quantum Migration

- **Category:** Cryptographic / Crypto-Agility
- **Description:** Atlas V1's entire cryptographic trust chain is built on Ed25519 (EdDSA over Curve25519). Ed25519 is currently secure but is vulnerable to a sufficiently powerful quantum computer running Shor's algorithm. NIST finalised FIPS 203 (ML-KEM), FIPS 204 (ML-DSA / Dilithium), and FIPS 205 (SLH-DSA / SPHINCS+) in August 2024. The "harvest now, decrypt later" attack is a real threat for long-lived records: an adversary who archives Atlas traces today can attempt to break them when quantum hardware matures (estimated 10-15 years, though timeline is uncertain). For Atlas specifically, the immutability of events.jsonl means we cannot retroactively re-sign old events with a post-quantum algorithm.
- **Probability:** LOW-MEDIUM (quantum threat to Ed25519 is real but timelines are >5 years; the risk is higher for long-lived archival use cases, lower for short-lived agent memory)
- **Impact:** HIGH (technical: all historic events become retroactively unverifiable via signature; financial: enterprise customers in regulated industries with long data retention requirements will demand PQ readiness before signing contracts; reputational: "Atlas built on an algorithm the industry is migrating away from")
- **Detectability:** HIGH (NIST standards are public; the transition timeline is trackable; no surprise materialisation)
- **Reversibility:** LOW (immutable events.jsonl — you cannot go back and re-sign with a new algorithm; forward migration requires a V2 schema transition with a "legacy events: trusted-at-signing-time" annotation)
- **Current Mitigation Status:** PARTIAL (V1's `Algorithm` enum in `atlas-trust-core` is `#[non_exhaustive]` — adding variants is SemVer-minor, which is the structural hook for crypto-agility. Session handoff §4 identifies this as "V2-risk-doc item". No PQ implementation exists.)
- **Mitigation Strategy:**
  - **Proactive (architectural):** Confirm that the `Algorithm` enum's `#[non_exhaustive]` + additive-variant SemVer-minor policy is explicitly designed as the PQ migration hook. Document this in SECURITY-NOTES.md and the V2 master plan. This costs nothing to do now and establishes the migration path.
  - **Proactive (medium-term):** Design a hybrid signing scheme for V2 events: Ed25519 + ML-DSA co-signature. Both algorithms sign the same signing input. Old verifiers continue to validate via Ed25519; new verifiers can validate via ML-DSA. This is the standard NIST-recommended migration approach.
  - **Proactive (forward-compatibility):** New `schema_version = "atlas-trace-v2"` is already a planned V2 break (per SEMVER-AUDIT §8). The V2 schema upgrade is the natural insertion point for PQ co-signatures — plan it in V2 now, even if implementation is Welle 14d or later.
  - **If it materialises (quantum breakthrough):** New events forward-signed with PQ only. Historic events annotated as "Ed25519-era: verified at time of signing, algorithm superseded." Publish migration guide. Trust property for future events restored immediately. Historic events have an asterisk — this is unavoidable for any signature system.
- **Owner:** Engineering + External-Security
- **Review Cadence:** Quarterly (track NIST PQ standards adoption; track quantum hardware progress reports)

---

### R-O-01: Performance Overhead at Scale

- **Category:** Operational / Architectural
- **Description:** Every Atlas write performs: payload schema validation, Ed25519 signing via atlas-signer subprocess, CBOR canonicalisation, blake3 hash computation, parent-chain resolution, JSONL append, and eventual Rekor HTTP submission (async). At low write volumes (<100/sec) this is invisible. At 10K writes/sec — a plausible multi-agent scenario where dozens of agents write concurrently into shared workspaces — the JSONL append serialisation mutex (`@atlas/bridge::writeSignedEvent` has a per-workspace mutex), subprocess spawning overhead, and Rekor rate limits become hard bottlenecks. The current per-workspace mutex in `@atlas/bridge` is documented in SEMVER-AUDIT §1.1 as structurally guaranteeing a linear chain for single-writer profiles — this same mutex is a throughput ceiling for multi-writer scenarios.
- **Probability:** MEDIUM (10K writes/sec is above current usage but entirely plausible for a multi-agent shared memory scenario at scale; even 1K writes/sec may hit the mutex ceiling)
- **Impact:** HIGH (technical: latency spikes and write drops under load; financial: enterprise SLAs broken; reputational: "Atlas can't handle production multi-agent scale")
- **Detectability:** HIGH (load testing will reveal it before production; benchmarks exist to surface this in V2-α)
- **Reversibility:** HIGH (tiered anchoring architecture solves this without touching the trust invariant; it is a known problem with known solutions)
- **Current Mitigation Status:** PARTIAL (session handoff §4 mentions "tiered anchoring — hot writes signed-only, batch-anchored to Rekor" as a mitigation direction)
- **Mitigation Strategy:**
  - **Proactive:** Tiered anchoring design in V2-α: (a) "hot tier" — every write Ed25519-signed + hash-chained in <5ms (no Rekor submission); (b) "warm tier" — batch Rekor anchoring every N seconds or M events (async, non-blocking); (c) "cold tier" — anchor chain tip rotation on a slower cadence. Trust invariant: the signed event is authoritative from the moment of signing; the Rekor anchor is the public witness that follows. This is already the V1 design for the async path.
  - **Proactive:** Benchmark the per-workspace mutex ceiling in V2-α. If single-mutex throughput is insufficient, architect multi-workspace sharding (one mutex per workspace, concurrent writes across workspaces) — which is already the V1 workspace isolation model.
  - **Proactive:** The `atlas-signer` subprocess model is a known overhead. V2-β candidate: embed the signer as an in-process library call (the Rust signer is already a crate, not just a binary). This eliminates subprocess spawn overhead.
  - **Quantified target:** Define V2 write SLO as p99 < 50ms at 1K concurrent writes/sec. Instrument and track from V2-α CI benchmarks.
- **Owner:** Engineering
- **Review Cadence:** Per-Welle (benchmark every wave that touches the write path)

---

### R-O-02: UX Complexity Barrier

- **Category:** Operational / Adoption
- **Description:** Atlas's value proposition requires users to understand or at least trust cryptographic provenance: signed events, Rekor anchors, WASM verifiers, hash chains. For developer-builders this is comprehensible and appealing. For compliance officers, knowledge workers, and non-technical stakeholders — particularly in the Second Brain market segment — "cryptographic provenance" is a feature they do not want to think about. If the UX surfaces trust machinery (key IDs, hash values, log indices) as part of normal operation, users will disengage. Worse: if Atlas appears to require deep crypto understanding to trust its output, adoption in the enterprise segment stalls because no one wants to be the person responsible for a system they don't fully understand.
- **Probability:** HIGH (this is a well-documented pattern for any security-first consumer product; SSL padlock history is instructive — the mechanism is invisible to users, the state is visible)
- **Impact:** HIGH (financial: Second Brain market is inaccessible; technical: none; reputational: "cryptographic memory tool no one can use")
- **Detectability:** HIGH (first user testing session will reveal this immediately)
- **Reversibility:** HIGH (UX is fixable; the underlying trust machinery is correct; this is a presentation problem not an architecture problem)
- **Current Mitigation Status:** PARTIAL (session handoff §4 mentions "hide trust by default, show only Verified ✓ / Tampered ✗" but no UX design exists)
- **Mitigation Strategy:**
  - **Proactive:** Define a "trust UI vocabulary" with two states visible to non-technical users: "Verified" (green checkmark, no further explanation required) and "Tampered" (red X, explanation of what this means in plain language). All cryptographic detail is one click away for those who want it — not surfaced by default.
  - **Proactive:** The landing page demo (Doc E) must demonstrate the "invisible trust" pattern. The user experience should be "you write a note, it gets a checkmark." The demo should NOT lead with "you write a note, it gets an Ed25519 signature with a COSE envelope and a Sigstore Rekor inclusion proof."
  - **Proactive:** Separate the developer API documentation (full crypto detail, for integrators) from the user-facing product surface (zero crypto detail, for end users). These are different audiences and must not share the same mental model.
  - **If it materialises:** User research to identify where the trust machinery breaks through into the UX. Systematic hiding of all crypto detail below the fold. This is a product/design iteration, not an engineering re-architecture.
- **Owner:** Product
- **Review Cadence:** Per-Welle (every UI addition should be reviewed against the "would a non-technical user be confused?" criterion)

---

### R-V-01: Vendor Capture by Major AI Providers

- **Category:** Vendor
- **Description:** Anthropic, OpenAI, and Google each have native memory products (Claude memory, OpenAI Memory, Google's agent memory in Gemini) that are deeply integrated into their ecosystems. If any of these vendors actively opposes Atlas integration — for example, by declining to support Atlas in their MCP host, refusing Hermes Agent a featured listing, or launching a competing "trusted memory" product with their own key infrastructure — Atlas's addressable market in those ecosystems shrinks significantly. The risk is not just competition; it is active opposition that forecloses integration surface.
- **Probability:** MEDIUM (vendor memory products are siloed and not interoperable by design; Atlas's open-substrate positioning is a direct challenge to vendor lock-in; active opposition is less likely than passive neglect, but non-zero)
- **Impact:** HIGH (financial: the Claude/GPT/Gemini user bases are massive distribution channels; technical: MCP host support is the path-of-least-resistance for those users; reputational: if vendors frame Atlas as a security risk, that narrative is hard to counter)
- **Detectability:** MEDIUM (vendor stance will be visible in their MCP compatibility lists and public developer relations, but decision-making is opaque until it surfaces publicly)
- **Reversibility:** MEDIUM (vendor opposition can be partially mitigated by routing around closed ecosystems via open-weight models; but the closed-ecosystem users are not reachable without vendor cooperation)
- **Current Mitigation Status:** PARTIAL (Atlas's agent-agnostic design is the primary structural defence: vendor permission is not required for open-weight model integration)
- **Mitigation Strategy:**
  - **Proactive:** Atlas's deepest structural defence is that it does NOT require vendor cooperation for the core value proposition. Open-weight models (Hermes-4, Llama-4, Mistral, Qwen) can integrate with Atlas via the HTTP API or the atlas-mcp-server without needing Anthropic/OpenAI/Google to do anything. The "open-weight model community" is Atlas's primary distribution channel — target it first.
  - **Proactive:** Frame Atlas as complementary to, not competitive with, vendor memory. "Atlas makes YOUR agent's memory auditable" is a different value proposition than "Atlas replaces your vendor's memory." The EU AI Act compliance angle makes Atlas useful even for users of vendor memory products — they need the audit log regardless.
  - **Proactive:** Do not require exclusivity from integrators. The more AI vendors whose agents can write to Atlas, the stronger the network effect. Position Atlas as infrastructure (like HTTPS), not as a competing product (like a browser).
  - **If it materialises:** Route distribution entirely through open-weight model ecosystem. Hermes-4 + Llama-4 + Mistral community is large enough to build a viable user base independently. The compliance-driven enterprise vertical does not depend on consumer-tier vendor cooperation.
- **Owner:** Strategy + Product
- **Review Cadence:** Quarterly (track vendor memory product announcements and MCP compatibility)

---

### R-L-01: GDPR Right to be Forgotten

- **Category:** Legal / Regulatory
- **Description:** EU GDPR Article 17 grants data subjects the right to erasure of personal data. Atlas's signed event log is designed to be immutable and append-only — that is the trust property. If personal data is written into an Atlas event payload (e.g., an agent records a person's name, health information, or financial details), a GDPR erasure request creates a structural conflict: GDPR demands deletion; the signed hash chain demands immutability. Attempting to delete or redact a signed event breaks the hash chain verification. Fines for GDPR non-compliance: up to 4% of global annual revenue (Art. 83 GDPR), or €20M — whichever is higher.
- **Probability:** HIGH (any Atlas deployment handling European user data where agents write personally identifiable information will face this; the V2 Second Brain market segment is explicitly personal data territory)
- **Impact:** CRITICAL (financial: regulatory fines up to 4% of global revenue + potential class-action exposure in EU; technical: requires architectural separation of content and hash; reputational: "Atlas's privacy model is non-GDPR-compliant" is a killshot for EU enterprise sales)
- **Detectability:** HIGH (GDPR requests are explicit; the conflict will be visible the moment the first erasure request arrives)
- **Reversibility:** LOW (GDPR fines are not easily reversed; the architecture fix is available but the window to avoid a fine closes the moment personal data is written without erasure capability)
- **Current Mitigation Status:** PARTIAL (V2-vision-knowledge-graph-layer.md §3.7 mentions "signed hashes, raw content separable" as a mitigation direction; no implementation exists)
- **Mitigation Strategy:**
  - **Proactive (architectural — must be V2-α):** Content-hash separation: Atlas events sign and chain the hash of the payload, not the payload itself. Raw content lives in a separate, deletable content store (key-value store indexed by content hash). To fulfil GDPR Art. 17: delete the raw content from the content store. The hash chain remains intact and verifiable — the anchor still proves "something was written here at time T, signed by key K, and it had hash H." The content is gone; the evidence of the write remains. This is the only architecturally sound resolution.
  - **Proactive:** Implement a "tombstone" event type: an atlas event that asserts "content referenced by hash H has been deleted per GDPR Art. 17 request, erasure timestamp T, erasure key K." This makes the deletion itself part of the verifiable record.
  - **Proactive:** Privacy review of the write-node schema. Add `content_storage_tier` field to route sensitive data to the deletable content store vs. embedding directly in the event payload. Default for V2 must be content-hash separation; direct-embed should be explicitly opt-in with a documented "no GDPR erasure possible" warning.
  - **Legal:** Obtain a formal legal opinion from EU-specialised privacy counsel on whether hash-only immutability + content deletion satisfies GDPR Art. 17 before V2-α ships to European customers. This is not a question that can be resolved by engineering analysis alone.
  - **If it materialises:** Immediate legal counsel engagement. Delete all erasure-requested raw content from the content store. Document the tombstone events. Notify the DPA proactively. The architectural fix is available in V2; retroactive compliance for V1 deployments requires a data audit.
- **Owner:** Legal + Engineering
- **Review Cadence:** Per-Welle (the content-hash separation architecture must be confirmed designed into V2-α before any personal data flows through Atlas)

---

### R-L-02: FalkorDB SSPL License Trap

- **Category:** Legal / Vendor
- **Description:** FalkorDB is licensed under Server Side Public License v1 (SSPLv1), which is not an OSI-approved open-source license. SSPLv1 requires that anyone offering FalkorDB functionality as a service to third parties must either: (a) obtain a commercial license from FalkorDB Ltd, or (b) open-source the entire service stack under SSPL — including Atlas's proprietary server code. If `atlas-trust.dev/explorer` serves customers' graph queries from a Nelson-hosted FalkorDB instance, this triggers. If atlas-web routes customer graph queries through a hosted FalkorDB backend, this triggers. This is a legal risk that could force open-sourcing of Atlas's server under SSPL or require a commercial license before any revenue-generating service launch.
- **Probability:** MEDIUM (the trigger is specific and well-defined; whether Atlas's hosted service constitutes "offering FalkorDB as a service" is a legal interpretation question; MongoDB has historically enforced SSPL aggressively)
- **Impact:** HIGH (financial: commercial FalkorDB license cost unknown but potentially substantial; forced SSPL open-sourcing would eliminate proprietary server-side differentiation; reputational: legal uncertainty blocks enterprise deals)
- **Detectability:** HIGH (this is a known, documented risk in V2-vision-knowledge-graph-layer.md §2.3; it surfaces in the first enterprise contract review)
- **Reversibility:** HIGH (Kuzu, MIT-licensed, is the documented alternative; migration cost is real but bounded; the underlying trust model does not depend on FalkorDB)
- **Current Mitigation Status:** PARTIAL (V2-vision-knowledge-graph-layer.md §3.1 explicitly flags this and recommends "validate with FalkorDB sales before building on it")
- **Mitigation Strategy:**
  - **Proactive (pre-V2-α blocking):** Before any V2-α code is written that depends on FalkorDB, obtain a formal legal opinion on whether Atlas's planned hosted-service architecture triggers SSPLv1. Contact FalkorDB sales/legal for a clarifying statement in writing.
  - **Proactive:** Evaluate Kuzu (MIT license, comparable property graph, active development) as the primary graph DB if FalkorDB SSPL exposure is confirmed. The V2 architecture should abstract over the graph DB layer such that swapping FalkorDB for Kuzu is a configuration change, not a re-architecture. The V1 trust invariant does not depend on which graph DB is used.
  - **Proactive:** Track SSPLv1 case law. If FalkorDB Ltd. is acquired or undergoes a license change (as happened with MongoDB's relicensing in 2018), reassess.
  - **If it materialises:** Switch to Kuzu or negotiate a commercial FalkorDB license. The migration window is measured in days of engineering work, not months — if the abstraction layer is in place.
- **Owner:** Legal + Engineering
- **Review Cadence:** Per-Welle (must resolve before V2-α ships any hosted graph query service)

---

### R-V-02: Mem0 Vendor Risk

- **Category:** Vendor
- **Description:** Atlas V2's fast-retrieval layer depends on Mem0 / Mem0g (the graph-enhanced variant). Mem0 is a venture-backed startup. Venture-backed startups face acquisition, pivot, shutdown, or license change as existential events. If Mem0 is acquired by a major AI vendor (Anthropic/OpenAI/Google), the license could change from Apache-2.0 to proprietary, or the product could be shuttered in favour of the acquirer's native memory stack. If Mem0 raises insufficient capital, it could reduce maintenance velocity or abandon Mem0g specifically (the graph variant, which is newer and less battle-tested than the core Mem0 product). Atlas's V2 latency story (91% p95 reduction, 2.59s p95) is directly dependent on Mem0g remaining functional and maintained.
- **Probability:** MEDIUM (venture-backed startup failure/acquisition rate is high; Mem0's current trajectory is positive but 18-month outlook is uncertain)
- **Impact:** MEDIUM (technical: fallback to direct FalkorDB query is available but slower; financial: no direct financial exposure unless Atlas has a paid dependency; reputational: "Atlas's fast-retrieval layer was abandoned" creates uncertainty)
- **Detectability:** HIGH (startup financials and acquisition announcements are public; GitHub activity is trackable)
- **Reversibility:** HIGH (the V1 trust invariant does not depend on Mem0g; the graph DB is the authoritative projection; Mem0g is a performance cache; removing it degrades latency but not trust correctness)
- **Current Mitigation Status:** ADEQUATE (the V2 architecture explicitly treats Mem0g as a non-authoritative cache: "Mem0g cache is rebuildable from events.jsonl, never trust-authoritative" — so the trust invariant survives Mem0g failure; only performance is affected)
- **Mitigation Strategy:**
  - **Proactive:** Design the retrieval layer with an explicit interface that Mem0g implements. This allows substituting Zep, Graphiti, or a custom retrieval layer without touching the trust-bearing event log or graph DB layers.
  - **Proactive:** Track Mem0g's Apache-2.0 license status. If Mem0 announces a relicense to proprietary or BSL (as happened with Terraform/HashiCorp), immediately evaluate alternatives.
  - **Proactive:** Do not market "91% p95 latency reduction via Mem0g" as a core Atlas feature until the retrieval layer is decoupled and multiple implementations are supported. Market it as "optional high-performance retrieval layer."
  - **If it materialises:** Remove Mem0g dependency. Fall back to direct FalkorDB traversal queries. The trust invariant and core value proposition are unaffected. Update performance benchmarks to reflect direct-query latency.
- **Owner:** Engineering + Strategy
- **Review Cadence:** Quarterly (monitor Mem0 funding, GitHub activity, and license status)

---

### R-V-03: Hermes Adoption Reversal

- **Category:** Vendor
- **Description:** Atlas's go-to-market distribution plan depends significantly on Hermes Agent (Nous Research, 60K+ GitHub stars, #1 on OpenRouter as of 2026-05-10). The plan is for an Atlas Memory Skill for Hermes to distribute Atlas to Hermes's user community. The risk is that Hermes's growth stalls, its community fragments, or a competing open-weight agent framework (e.g., Llama-4-based frameworks, DeepSeek Agent, or a new OpenRouter leader) displaces Hermes before Atlas's Hermes skill achieves meaningful distribution. "60K stars in 2 months" is exceptional momentum, but the AI agent framework landscape has demonstrated rapid leaderboard changes.
- **Probability:** MEDIUM (the AI agent framework space is highly volatile; 2-month momentum does not guarantee 18-month dominance; history shows rapid reversals)
- **Impact:** MEDIUM (financial: distribution channel shrinks; technical: no impact on Atlas's architecture or trust property; reputational: "Atlas bet on the wrong horse" narrative if Hermes declines quickly)
- **Detectability:** HIGH (GitHub stars, OpenRouter rankings, and community activity are continuously visible)
- **Reversibility:** HIGH (the HTTP API and MCP server work with any agent framework; adding a skill for a new framework leader is a bounded engineering task; Atlas's positioning as agent-agnostic means no single framework dependency is permanent)
- **Current Mitigation Status:** ADEQUATE (Atlas's core positioning is "agent-agnostic substrate" — the Hermes skill is one distribution channel, not the product itself; MCP host support means any MCP-compatible agent framework gets Atlas for free)
- **Mitigation Strategy:**
  - **Proactive:** Build the Hermes skill as a reference implementation, not as Atlas's only integration. Simultaneously develop the LangGraph + Llama-4/custom-HTTP-agent integration sketches (V2-γ in V2-vision). Three reference integrations hedge against any single framework declining.
  - **Proactive:** Track OpenRouter rankings monthly. If a new framework achieves >10K stars within 60 days and Hermes stalls, begin that framework's Atlas integration immediately.
  - **Proactive:** Invest in the MCP protocol path: MCP 1.29+ compatibility gives automatic Atlas integration to any MCP-compatible agent host, independent of which specific framework is popular. The MCP investment compounds across framework changes.
  - **If it materialises:** Redirect skill distribution effort to the next leading framework. The engineering cost is bounded. The risk is time loss (3-6 Wellen of distribution work needs re-targeting), not permanent damage.
- **Owner:** Engineering + Product
- **Review Cadence:** Per-Welle

---

### R-A-03: Agent Identity Key Compromise and Revocation Lag (Independently Identified)

- **Category:** Architectural / Trust
- **Description:** V2 introduces per-agent Ed25519 keypairs ("Agent Passports") as W3C DIDs. Once an agent has written events into Atlas workspaces under its keypair, compromising that keypair allows an attacker to write fraudulent events under that agent's identity — and those events will verify as legitimate. The revocation problem is structural: since events.jsonl is immutable, post-compromise events signed by the compromised key cannot be "unsigned." Revocation must be communicated through a separate channel (a "revocation event" signed by a different key), and consumers must check the revocation list before trusting any signed event. If the revocation propagation lags behind attacker activity, a window of fraudulent-but-verifying events exists. For the "AI-Liability-Insurance pricing substrate" use case, a compromised agent key that writes fraudulent attestations could result in actual financial harm to insurance underwriters.
- **Probability:** MEDIUM (key compromise is a well-known attack vector; agent keys stored on compute infrastructure are more exposed than hardware-secured keys; the multi-agent deployment model multiplies the attack surface)
- **Impact:** HIGH (financial: fraudulent attestations downstream of a compromised key create liability exposure; technical: V1 trust invariant holds for the compromised-key events themselves — they have valid signatures — but the attribution is now meaningless; reputational: "Atlas's agent identity was compromised" directly attacks the trust narrative)
- **Detectability:** LOW (without an out-of-band revocation check mechanism, there is no way to detect that events signed by key K are fraudulent after K is compromised; a verifier with only the events.jsonl sees valid signatures)
- **Reversibility:** LOW (fraudulent events that have already been verified and acted upon cannot be un-acted upon; revocation prevents future harm but not historical harm)
- **Current Mitigation Status:** NONE (V2-vision-knowledge-graph-layer.md §2.7 mentions "revocation chain" as part of Agent Passport design but no design has been specified; the V1 architecture has no revocation mechanism)
- **Mitigation Strategy:**
  - **Proactive (must be designed in V2-α):** Specify the revocation mechanism before Agent Passports are deployed. Options: (a) revocation event type in events.jsonl (agent self-revokes or operator revokes by signing a tombstone event with the superseding key); (b) separate revocation registry (OCSP-style, external to events.jsonl); (c) expiring keys with mandatory rotation (keys have a TTL; expired-key events are automatically suspect). Option (a) preserves the "events.jsonl as single source of truth" invariant and should be preferred.
  - **Proactive:** Hardware-backed key storage for high-stakes agent identities. The HSM integration in V1 (ATLAS_HSM_PKCS11_LIB) provides the infrastructure for this — extend it to agent keypairs, not just operator signing keys.
  - **Proactive:** Automatic key rotation policy. Agent keys should have configurable TTLs (default: 90 days). An agent that has not rotated its key in >90 days should be flagged in the graph explorer.
  - **Proactive:** Out-of-band revocation check in the WASM verifier. When verifying a trace, the verifier should optionally check against a revocation endpoint or locally cached revocation list for any key in the bundle.
  - **If it materialises:** Publish revocation event immediately. All downstream consumers who rely on the compromised-key events receive an explicit signal. Legal counsel on liability exposure for the window of fraudulent attestations. Rotate all agent keys in affected workspaces.
- **Owner:** Engineering + External-Security
- **Review Cadence:** Per-Welle (revocation design must exist before any Agent Passport feature ships to production)

---

### R-S-02: Regulatory Narrative Capture (EU AI Act Interpretation Lock-In) (Independently Identified)

- **Category:** Strategic / Legal
- **Description:** Atlas's compliance go-to-market is built on a specific interpretation of EU AI Act Art. 12, 13, 14, 18 — specifically, that "automatically generated event logs, independently verifiable" (Art. 12) maps to Atlas's signed events.jsonl + Sigstore Rekor architecture. This interpretation has not been validated by any EU supervisory authority, DPA, or notified body. If the relevant national competent authority or the EU AI Office issues guidance that interprets Art. 12 differently — for example, requiring vendor-certified audit logs, mandating a specific log format (e.g., Common Log Format), or treating self-attested cryptographic logs as insufficient — Atlas's compliance value proposition could be invalidated without any change to Atlas's architecture. The EU AI Liability Directive (currently in Council phase, expected 2026) adds a second interpretive risk.
- **Probability:** LOW-MEDIUM (EU AI Act implementing acts are still being developed; ambiguity in Art. 12 is real; but the regulation's general direction — verifiable, tamper-evident, auditable — aligns well with Atlas's architecture; adverse interpretation is possible but not the base case)
- **Impact:** HIGH (financial: entire EU compliance-driven go-to-market invalidated; reputational: "Atlas misread the regulation it was built for")
- **Detectability:** MEDIUM (regulatory guidance is published publicly but may emerge slowly and ambiguously; a DPA enforcement action against a competitor using a similar approach would be the first clear signal)
- **Reversibility:** MEDIUM (Atlas's architecture is strong on the technical dimensions most regulations care about; adaptation to specific regulatory format requirements is engineering work; but the time-to-market of compliance-driven adoption is lost during adaptation)
- **Current Mitigation Status:** NONE (no external legal or regulatory validation of the Art. 12 interpretation has occurred; session handoff §0 explicitly notes "security experts come at the END post-V2-α/β")
- **Mitigation Strategy:**
  - **Proactive:** Obtain a formal opinion from EU AI Act-specialised counsel on whether Atlas's architecture satisfies Art. 12 as currently interpreted. This is not a post-V2-β activity — it should be a pre-V2-α gate for the compliance marketing narrative.
  - **Proactive:** Engage with the EU AI Office's consultation processes and published Q&A. The EU AI Office has an obligation to publish guidance on Art. 12 interpretation. Track this actively.
  - **Proactive:** Frame Atlas's marketing language carefully: "designed to satisfy the verifiability and tamper-evidence requirements of EU AI Act Art. 12" rather than "EU AI Act compliant." The latter is a certification claim; the former is a design claim. Only certified compliance products can claim the latter.
  - **If it materialises:** Atlas's architecture adapts to specific format requirements faster than any competitor because the trust layer is modular. But the adaptation time is 1-2 Wellen minimum; the go-to-market timeline slips.
- **Owner:** Legal + Strategy
- **Review Cadence:** Quarterly (track EU AI Office guidance publications)

---

### R-O-03: Projection Rebuild Cost at Production Scale (Independently Identified)

- **Category:** Operational
- **Description:** The "deterministically rebuildable projection" invariant requires that a consumer can replay all of events.jsonl through the projector to obtain the current graph state. At launch, this is trivial. At production scale — a workspace with 10M+ events accumulated over years — a full projection rebuild becomes an hours-long or days-long operation. The rebuild would be needed in disaster recovery scenarios (graph DB corruption, FalkorDB instance failure, projector schema upgrade requiring a full re-projection). If the rebuild time is measured in hours, the RTO (Recovery Time Objective) for the graph query service is unacceptable for enterprise SLAs. More subtly: if incremental projection is implemented incorrectly, a partial rebuild after a projector bug may produce an inconsistent graph state that validates on partial coverage but diverges on the full rebuild.
- **Probability:** MEDIUM (production scale is a function of adoption; this is not an immediate risk for V2-α, but becomes critical by V2-γ/δ if Atlas gains meaningful adoption)
- **Impact:** HIGH (technical: graph DB unavailability during full rebuild means read-side is down for enterprise customers; financial: SLA breach; reputational: "Atlas's trust property is unrecoverable at scale")
- **Detectability:** MEDIUM (rebuild time will be visible during disaster recovery drills, but may not be tested until an actual incident triggers it)
- **Reversibility:** HIGH (incremental projection with snapshot checkpointing solves this; it is a standard database pattern; the solution is well-understood)
- **Current Mitigation Status:** NONE (the V2-vision doc notes the projection invariant but does not address rebuild performance at scale)
- **Mitigation Strategy:**
  - **Proactive:** Projection snapshot checkpoints. Every N events (configurable, e.g. 10K), the projector writes a snapshot of the current graph state along with the event ULID it was computed from. A rebuild from failure starts from the last valid snapshot + replays only the events since that snapshot. The snapshot itself must be hash-verified against the events it covers.
  - **Proactive:** Incremental projection as the default operating mode. The projector tracks a watermark (last processed event ULID) and only processes new events on each run. Full rebuild becomes an explicit operator action (`atlas-projector rebuild --from-scratch`), not the normal operating mode.
  - **Proactive:** Benchmark projection throughput during V2-α. Establish: how many events/second can the projector process? What is the rebuild time for 100K / 1M / 10M events? Set an explicit RTO requirement before V2-α ships to enterprise customers.
  - **If it materialises:** Restore from most recent valid snapshot + incremental replay. If no snapshot exists, full rebuild is unavoidable. The V1 trust invariant is unchanged — the event log is the source of truth and the rebuild result is authoritative once complete. Service is degraded but not wrong during the rebuild window.
- **Owner:** Engineering
- **Review Cadence:** Per-Welle (benchmark every wave that adds events or changes projection schema)

---

## Risk Heatmap

Risk severity = Probability × Impact. Use this to prioritise remediation order.

```
                     IMPACT
                  LOW    MEDIUM    HIGH    CRITICAL
               ┌───────┬──────────┬───────┬──────────┐
    CRITICAL   │       │          │       │          │
               ├───────┼──────────┼───────┼──────────┤
    HIGH       │       │ R-V-03   │R-O-01 │  R-S-01  │
               │       │ R-V-02   │R-O-02 │          │
               │       │          │R-V-01 │          │
               ├───────┼──────────┼───────┼──────────┤
    MEDIUM     │       │ R-O-03   │R-A-02 │  R-L-01  │
               │       │          │R-L-02 │  R-A-01  │
               │       │          │R-S-02 │          │
               │       │          │R-C-01 │          │
               │       │          │R-A-03 │          │
               ├───────┼──────────┼───────┼──────────┤
    LOW        │       │          │       │          │
               └───────┴──────────┴───────┴──────────┘

CRITICAL ZONE (Probability MEDIUM+ × Impact CRITICAL):
  R-L-01  GDPR Right to be Forgotten           [MEDIUM prob, CRITICAL impact]
  R-A-01  Projection Determinism Drift          [HIGH prob, CRITICAL impact]
  R-S-01  Adoption Tipping Point               [HIGH prob, CRITICAL impact]

HIGH ZONE (Probability HIGH × Impact HIGH):
  R-O-01  Performance Overhead at Scale
  R-O-02  UX Complexity Barrier
  R-V-01  Vendor Capture

ELEVATED ZONE (Probability MEDIUM × Impact HIGH):
  R-A-02  Privacy vs. Public Anchoring
  R-A-03  Agent Identity Key Compromise        ← detectability: LOW (elevates urgency)
  R-C-01  Post-Quantum Migration
  R-L-02  FalkorDB SSPL License Trap
  R-S-02  Regulatory Narrative Capture

MONITOR ZONE (Probability MEDIUM × Impact MEDIUM):
  R-O-03  Projection Rebuild Cost at Scale
  R-V-02  Mem0 Vendor Risk
  R-V-03  Hermes Adoption Reversal
```

---

## Risk Ownership Summary

| Risk | Owner | Mitigation Status | Blocking V2-α? |
|---|---|---|---|
| R-S-01 Adoption Tipping Point | Strategy + Product | PARTIAL | No (continuous) |
| R-A-01 Projection Determinism Drift | Engineering | PARTIAL | **YES** — CI gate required before any event replayed to graph DB |
| R-A-02 Privacy vs. Public Anchoring | Engineering + Product | PARTIAL | No (V2-β feature) |
| R-C-01 Post-Quantum Migration | Engineering + External-Security | PARTIAL | No (V2-schema-transition candidate) |
| R-O-01 Performance Overhead | Engineering | PARTIAL | No (benchmark required, not fix) |
| R-O-02 UX Complexity | Product | PARTIAL | No (design gate) |
| R-L-01 GDPR Right to be Forgotten | Legal + Engineering | PARTIAL | **YES** — architecture decision required before EU personal data flows |
| R-L-02 FalkorDB SSPL | Legal + Engineering | PARTIAL | **YES** — legal opinion required before hosted FalkorDB service launches |
| R-V-01 Vendor Capture | Strategy + Product | PARTIAL | No (structural defence is Atlas's design) |
| R-V-02 Mem0 Vendor Risk | Engineering + Strategy | ADEQUATE | No (trust invariant survives Mem0 failure) |
| R-V-03 Hermes Adoption Reversal | Engineering + Product | ADEQUATE | No (agent-agnostic positioning is the hedge) |
| R-A-03 Agent Key Revocation | Engineering + External-Security | NONE | **YES** — must be designed before Agent Passports ship to production |
| R-S-02 Regulatory Narrative Capture | Legal + Strategy | NONE | No (pre-V2-marketing gate) |
| R-O-03 Projection Rebuild Cost | Engineering | NONE | No (V2-β operational requirement) |

**V2-α blocking risks (must resolve before first production user touches V2):**
1. R-A-01 — Projection determinism CI gate
2. R-L-01 — GDPR architecture decision (content-hash separation)
3. R-L-02 — FalkorDB SSPL legal opinion
4. R-A-03 — Agent key revocation mechanism design

---

## Open Questions for Phase 2 Critique

The following are specific questions where Phase 2 critique agents should apply adversarial pressure. Each question identifies a gap in the current risk analysis.

**Q1: Is R-A-01 (Projection Determinism Drift) adequately mitigated by a CI gate, or is there a deeper issue?**
Context: The proposed mitigation is a CI gate that checks the hash of a projected graph state against a pinned expected value. But this only detects known drift in a known test corpus. A projection that is deterministic on test data but non-deterministic on production data (e.g., because of time-dependent projector logic, environment-dependent float behaviour, or database write ordering) would pass CI and fail in production. The mitigation may be necessary but insufficient.
Status: Open. Needs architect-level critique of what "deterministic projection" actually requires in the FalkorDB context.

**Q2: Is the GDPR Art. 17 mitigation (content-hash separation) legally sufficient?**
Context: The proposed architecture separates content (deletable) from hash (immutable). The legal question is whether a hash of deleted content constitutes "personal data" under GDPR's broad definition (Art. 4(1)). If a hash can be reversed (rainbow table attack) or if the hash uniquely identifies the data subject in context, it may still be personal data, and its immutability in events.jsonl may still violate Art. 17. This is a legal question, not an engineering question.
Status: Open. Requires EU privacy counsel opinion before any V2 deployment involving personal data.

**Q3: Are there additional legal risks in non-EU jurisdictions that the current risk matrix does not cover?**
Context: The current matrix focuses on EU GDPR and EU AI Act. US CCPA (California Consumer Privacy Act), HIPAA (healthcare data), PCI-DSS (financial data), and China's PIPL (Personal Information Protection Law) each have different erasure, audit, and data sovereignty requirements. Atlas's agent-agnostic design means it could be deployed globally. Non-EU exposure may be as large as EU exposure.
Status: Open. The current matrix has no non-EU legal risk coverage.

**Q4: Is R-A-03 (Agent Key Revocation) classified at the correct severity?**
Context: The risk is classified as MEDIUM probability, HIGH impact, LOW detectability, LOW reversibility. The combination of LOW detectability and LOW reversibility arguably makes this a CRITICAL risk even at MEDIUM probability — because by the time it is detected, fraudulent events have already been used for consequential purposes (insurance pricing, compliance attestations). Should this risk be escalated to CRITICAL combined severity?
Status: Open. Phase 2 security critique should challenge the classification.

**Q5: What is the supply chain attack surface for the Atlas projector and Mem0g integration?**
Context: V2 adds significant new dependencies: FalkorDB (graph DB), Mem0g (retrieval layer), and potentially Graphiti (entity extraction). Each dependency is a supply chain attack vector. A compromised FalkorDB release could inject malicious projector logic that silently alters the graph state while passing V1 event verification. A compromised Mem0g could return falsified retrieval results while claiming verified provenance. The current risk matrix does not explicitly address supply chain attacks on V2's new dependencies.
Status: Open. Requires a dependency security audit of the V2 stack analogous to `npm audit` but for the full supply chain.

**Q6: Does Atlas's immutability property survive a FalkorDB data loss event?**
Context: The V1 trust invariant holds because events.jsonl is the authoritative source. But in a V2 deployment where users interact with Atlas primarily through the graph explorer (not directly with events.jsonl), a FalkorDB data loss event — even if fully recoverable by re-projection — may be indistinguishable to users from a "tampered" event. The UX of "graph is temporarily unavailable during rebuild" vs "graph was tampered and is being restored" must be clearly differentiated. If users interpret unavailability as tampering, Atlas loses trust even when the system is behaving correctly.
Status: Open. Product and UX critique needed on the "rebuild vs. tamper" user experience distinction.

**Q7: Is "60K GitHub stars in 2 months" for Hermes Agent a durable signal or a hype spike?**
Context: R-V-03 assesses Hermes Adoption Reversal at MEDIUM probability. This assumes the 60K star trajectory is real usage signal, not a hype spike driven by a single viral moment. If the 60K stars reflect passive interest rather than active production deployment, the distribution value of a Hermes skill may be significantly lower than projected. Phase 2 competitive analysis (Doc D) should attempt to quantify Hermes's active deployment base vs. passive star count.
Status: Open. Requires the Doc D competitive landscape research to validate.

**Q8: Are there risks from the interaction of multiple V2 layers that are not visible in single-layer risk analysis?**
Context: This risk matrix analyses each risk category separately. But V2 adds three new layers simultaneously (graph DB, Mem0g, agent identities), and their interaction may create emergent failure modes not visible in single-layer analysis. For example: a Mem0g cache that is stale + a projector that is deterministically incorrect + an agent key that has been compromised could produce a situation where every verification layer says "valid" but the actual data is fraudulent. The trust invariant holds at the event layer but the composed system is untrustworthy. This is a systems-level risk that requires end-to-end adversarial analysis.
Status: Open. External security audit of the V2 composed trust model is the appropriate mitigation, but session handoff §0 defers this to post-V2-α/β.

**Q9: Is the "agent-agnostic" positioning itself a risk?**
Context: "Agent-agnostic" means Atlas has no first-party agent to drive adoption, no captive user base, and no natural distribution channel except integrations. Every other memory layer in the competitive landscape is backed by a product with its own users. Atlas is asking agents and their builders to add a dependency for a trust property they may not currently value. The "agent-agnostic" positioning may be structurally correct but commercially weak in the early adoption phase. Phase 2 product critique should challenge whether a narrow, opinionated Atlas-for-Hermes product would gain traction faster than a broad, agent-agnostic substrate.
Status: Open. Strategy-level question for Phase 2 product and business critique.

**Q10: What happens to Atlas's trust model if Sigstore Rekor is shut down or undergoes a key rotation that invalidates historic anchors?**
Context: R-A-02 addresses privacy concerns about public Rekor anchoring. But there is a separate risk: Atlas's historic Rekor anchors depend on Sigstore Rekor v1's continued operation and the validity of the pinned ECDSA P-256 public key (`SIGSTORE_REKOR_V1.pem` in SECURITY-NOTES.md). Sigstore is an open-source project maintained by a Linux Foundation effort — not a commercial service with SLAs. A Rekor key rotation or service sunset would leave all historic Atlas Rekor-anchored events with anchors that cannot be verified against a live public endpoint. SECURITY-NOTES.md already pins `active_tree_id = 1_193_050_959_916_656_506` — this is a potential single point of failure.
Status: Open. Mitigation exists (Atlas includes offline Merkle proof data in every anchor, so verification is possible even without a live Rekor endpoint), but this needs explicit confirmation from an architect reviewing SECURITY-NOTES.md.

---

**Doc owner:** Security-reviewer agent + Nelson. **Created:** 2026-05-12. **Status:** v0, ready for Phase 2 critique.
**Next action:** Phase 2 security-reviewer and compliance-reviewer critique agents should address Q1, Q2, Q4, Q5, and Q8 as CRITICAL priority questions. Q3, Q6, Q7, Q9, Q10 are HIGH priority.
