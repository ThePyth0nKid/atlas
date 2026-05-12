# Atlas V2 — Competitive Landscape v0 (2026-05)

> **Status:** Draft v0 (2026-05-12). Phase-1 foundation document for the V2 iteration framework. Designed to be passed to Phase-2 critique agents (business, product, security). Not yet a strategy document — just an honest market map.
>
> **Methodology note:** every numeric claim is either (a) verified via 2026-05 WebSearch with source URL at the end of the subsection, or (b) marked "estimated" / "claimed by company" if the source is vendor-self-reported and uncorroborated. Atlas has zero production deployments today and ~0 GitHub stars on the public repo (just-flipped public 2026-05-11). We will say so explicitly where it matters.

---

## 1. Two Market Categories

Atlas V2 sits at the structural intersection of two adjacent categories that, today, do not overlap at the product level — but share an underlying primitive ("persistent structured knowledge that survives the LLM context window"). Atlas's pivot thesis is that *cryptographic trust* is the connective tissue that lets one substrate serve both.

### 1a. AI Agent Memory Infrastructure

- **Target persona:** AI engineer, agent builder, platform team integrating agents into a product
- **Primary purchase driver:** retrieval accuracy, latency, framework integrations, multi-tenant isolation
- **Market shape (2026):** dominated by Mem0 (~52K stars, AWS Strands SDK partnership), Letta (~22K stars, MemGPT lineage), Zep+Graphiti (~23K stars on the OSS graph engine, plus a managed cloud). Incumbents Anthropic and OpenAI ship "native memory" inside their consumer apps that is purposefully vendor-locked.
- **Common trust property:** *none*. Every player is plaintext-JSON-in-Postgres-or-vector-DB. No cryptographic chain, no third-party verifiability. "Trust" means SOC 2 / HIPAA at the vendor level, not at the data level.

### 1b. Human Second Brain / Personal Knowledge Management (PKM)

- **Target persona:** knowledge worker, researcher, student, indie creator
- **Primary purchase driver:** UX polish, plugin ecosystem, offline-first / privacy posture, cross-device sync
- **Market shape (2026):** Obsidian leads with ~1.5M MAU and ~$25M ARR (estimated, vendor-reported), Notion ~30M+ users, Logseq is the OSS underdog, Roam is in maintenance mode, Capacities / Tana / Heptabase carve niches.
- **Common trust property:** *none*. Files-on-disk markdown is the closest thing to a verification model. Obsidian has GPG **encryption** plugins (gpgCrypt, GPG Encrypt) but as of 2026-05 there is **no signature / provenance verification plugin in the community catalog** — gpgCrypt's README explicitly says "currently has no signature support."

### 1c. Atlas's Unique Position — Cryptographic Bridge Between Both Categories

Atlas's structural property — Ed25519-signed events, hash-chained edges, Sigstore Rekor anchoring, offline WASM verifier — is **orthogonal to both categories' existing value propositions**. Atlas does not need to beat Mem0 on retrieval latency or Obsidian on UX polish to win — it needs to offer something neither category has: *cryptographic auditability that survives the source company going away*.

Two-market positioning works because:
1. The substrate is the same in both cases (signed event log → projected knowledge graph).
2. The buyer is different but the proof-of-property is identical.
3. The regulatory tailwind (EU AI Act Art. 12 effective 2026-08-02, EU AI Liability Directive in Council) creates structural demand on the agent-side; the personal-data-sovereignty trend creates pull on the human-side.

Whether the "verifiable Second Brain" category is *real* or *aspirational* is an open question we list in §8 — honestly, today it is aspirational. The AI-memory side has a clearer near-term demand signal.

---

## 2. AI Agent Memory Layer Competitors

### 2.1 Mem0 (and Mem0g graph variant)

- **License:** Apache-2.0 (core OSS) + commercial managed platform
- **Founded:** 2023. Series A: $24M (Oct 2025, lead investors not fully public)
- **Pricing (2026-05):**
  - Hobby — free, 10K memories, 1K retrievals/month
  - Starter — $19/month, 50K memories, *no graph memory*
  - Pro — $249/month, unlimited + graph memory (Mem0g) + analytics
  - Enterprise — custom, on-prem + SSO + SLA
  - **Note:** the $19 → $249 jump for graph access is the most-cited reason developers churn away
- **Features:**
  - Two retrieval modes: Mem0 (vector-only, 1.44s p95) and Mem0g (graph-enhanced, 2.59s p95)
  - Mem0g benchmark: 68.4% LLM Score on LOCOMO vs Mem0's 66.9%; ~91% p95 latency reduction vs full-context
  - Entity extraction + relationship inference + conflict detection
  - SOC 2 + HIPAA compliance on managed platform
  - **AWS Strands SDK exclusive memory provider** (significant distribution channel)
- **User Base:** ~52K GitHub stars (most-starred in category), ~14M Python downloads, "thousands of developers, startups, enterprises in production" (claimed), API call growth 35M Q1 → 186M Q3 2025 (claimed). Native integrations with CrewAI, Flowise, Langflow.
- **Trust Property:** **none cryptographic.** Vendor-managed SOC 2 / HIPAA at the platform level. No event signatures, no anchored audit log, no offline verification.
- **Atlas Differentiator:** Atlas is *not* a Mem0 competitor on retrieval primitives — Mem0g is faster, more mature, and broadly integrated. Atlas can sit *underneath* or *next to* Mem0 as the trust-authoritative layer: agent writes through Mem0 for fast retrieval, but the *durable, regulator-presentable* record is in Atlas's events.jsonl. Mem0g cache rebuildable from Atlas log = strong story.
- **Partnership posture:** strong partner candidate. Mem0 has no incentive to build crypto-trust; we have no incentive to build retrieval-tuned graph indexes. Orthogonal.
- **Honest weakness for Atlas:** Mem0 has 52K stars, AWS partnership, hundreds of integrations. Atlas has 0 production deployments today.

Sources:
- https://github.com/mem0ai/mem0
- https://mem0.ai/pricing
- https://arxiv.org/abs/2504.19413 (Mem0 / Mem0g paper)
- https://atlan.com/know/mem0-alternatives/
- https://mem0.ai/series-a

### 2.2 Letta (formerly MemGPT)

- **License:** Apache-2.0 (open-source server + SDKs)
- **Founded:** 2023 as MemGPT (UC Berkeley Sky Lab), rebranded to Letta 2024
- **Pricing (2026-05):**
  - Free self-host (Apache-2.0)
  - Pro — $20/month (personal managed cloud)
  - Max Lite — $100/month
  - Max — $200/month
  - API Plan — $20/month base + usage
  - Enterprise — custom
- **Features:**
  - "OS for agent memory" model: core/recall/archival tiers, agent manages allocation
  - Letta Code: memory-first coding agent, #1 model-agnostic OSS agent on Terminal-Bench
  - Skills + subagents as first-class primitives (April 2026 release)
  - Production REST APIs + Python/JS/Rust SDKs
  - Model-agnostic; runs against OpenAI, Anthropic, Gemini, Ollama, open-weights
- **User Base:** ~22.4K GitHub stars. Production-ready claim with REST APIs + SDKs. Specific deployment counts not public.
- **Trust Property:** none cryptographic. Memory state is plaintext, vendor-managed, no signature chain.
- **Atlas Differentiator:** Letta competes with Mem0 directly on the memory-OS abstraction. Atlas is again orthogonal — Letta could ship Atlas as a "verifiable archival memory tier" without touching its core context-window-management thesis. Both Apache-2.0 makes integration legally trivial.
- **Partnership posture:** medium-strong. Letta's "agents that self-improve" thesis benefits from a tamper-evident log of *what the agent learned* — useful for both debugging and reputation portability. Letta Code's git-backed memory is the closest existing thing to a "structured durable memory" pattern in the OSS landscape; Atlas can be the cryptographic layer underneath.
- **Honest weakness for Atlas:** Letta has 22K stars and a clear UC Berkeley research pedigree. Atlas has no academic pedigree, no managed cloud, no agent SDK.

Sources:
- https://github.com/letta-ai/letta
- https://www.letta.com/
- https://www.letta.com/blog/letta-code
- https://xyzeo.com/product/letta-memgpt

### 2.3 Zep (managed cloud) + Graphiti (OSS graph engine)

- **Graphiti license:** Apache-2.0 (OSS, separate from Zep Cloud)
- **Zep Cloud license:** proprietary managed service
- **Founded:** Zep raised €7.5M seed; Graphiti grew out of Zep's R&D
- **Pricing (2026-05):**
  - Graphiti — free, self-host
  - Zep Cloud — free tier (no credit card), enterprise pricing not public (likely usage-based)
  - Knowledge Graph MCP server — free
- **Features:**
  - **Bi-temporal model:** every edge has `(t_valid, t_invalid)` + `(t'_created, t'_expired)` — "when fact was true in world" vs "when we learned of it"
  - LLM-driven entity/relationship extraction (provider-configurable: Claude / OpenAI / Gemini / Ollama)
  - Hybrid retrieval: semantic + BM25 + graph traversal
  - **FalkorDB backend officially supported since 2025** — directly relevant to Atlas's V2 stack
  - MCP server with "hundreds of thousands of weekly users" (claimed)
- **User Base:** ~23K GitHub stars on Graphiti (~20K Apr 2026, ~23K Feb 2026), 25K weekly PyPI downloads, MCP server 1.0 shipped 2026. "Rapid adoption across startups and enterprises" (claimed).
- **Trust Property:** none cryptographic. Bi-temporal model is for *correctness* (what did we believe when), not for *verifiability* (can someone else prove what we believed).
- **Atlas Differentiator:**
  - Atlas's trust property is structural (signatures + Rekor anchors); Graphiti's temporal property is semantic (timestamps in graph edges).
  - These are **stackable, not competing**. An Atlas event → projected into a Graphiti graph → still verifiable back to the signed event.
  - The bi-temporal model is genuinely valuable and re-inventing it would be wasteful.
- **Partnership posture:** **strongest single partner candidate**. They support FalkorDB (our planned backend), they're Apache-2.0, they have the bi-temporal model we'd otherwise build ourselves, and the maintainers (Zep team) are clearly OSS-aligned. Atlas could ship a `graphiti-atlas-adapter` that makes Graphiti edges Atlas-verifiable.
- **Honest weakness for Atlas:** Graphiti's bi-temporal model is more mature than anything we have. If they ship cryptographic signing first, they become a competitor instead of a partner.
- **Watch-item:** does Graphiti's LLM-extraction non-determinism (different LLM seeds → different graphs) collide with Atlas's "deterministically rebuildable projection" trust invariant? See Doc B §2.2 for the mitigation analysis (option c: deterministic projector for trust-load-bearing extraction, Graphiti as opt-in retrieval enhancement).

Sources:
- https://github.com/getzep/graphiti
- https://blog.getzep.com/graphiti-hits-20k-stars-mcp-server-1-0/
- https://www.getzep.com/product/open-source/
- https://help.getzep.com/graphiti/getting-started/overview
- https://arxiv.org/abs/2501.13956 (Zep paper)

### 2.4 Anthropic Memory (Claude's native memory)

- **License:** proprietary (Anthropic-only)
- **Pricing (2026-05):** included in Claude Free + Claude Pro ($20/month) + Team + Enterprise. Activated for all accounts March 2026.
- **Features:**
  - **Auto-memory:** Claude scans chat history, generates a synthesized summary (~24h cadence)
  - **Explicit memory tool:** user says "remember X" → immediate update
  - **Claude Managed Agents:** persistent memory as filesystem (public beta, April 2026 announcement)
  - **Dreaming:** research preview — Claude reviews past sessions for patterns, self-improvement (April 23, 2026 announcement)
  - **Import from rivals:** free Claude users can now import context from ChatGPT/Gemini (March 2026)
  - **Constraint:** claude.ai + Claude app only — **does not apply to API access or Claude Code**
- **User Base:** part of Anthropic's overall consumer/enterprise footprint (not separately broken out). Claude Pro estimated millions of subscribers.
- **Trust Property:** **none cryptographic, fully vendor-controlled.** Memory is Anthropic-managed, Anthropic-revocable, Anthropic-non-portable. No cross-vendor verifiability. The user cannot prove to a third party what Claude "remembered" at time T.
- **Atlas Differentiator:** This is the textbook "vendor silo" memory model — high UX polish, zero portability, zero verifiability. Atlas is the inverse: cross-vendor (any agent writes through MCP / HTTP), cryptographically verifiable, portable across organizations.
- **Competitive threat assessment:** **HIGH** for the consumer Claude.ai use case (Atlas is not competing with this directly), **LOW** for the agent-memory-substrate use case. Anthropic's roadmap (Dreaming, Managed Agents) is moving toward agent-internal memory, not industry-shared substrate. As long as Anthropic remains a vendor-silo memory provider, Atlas's cross-vendor pitch is uncontested.
- **Partnership posture:** indirect. Anthropic's Lyrie.ai Cyber Verification Program (announced May 2026) shows they care about agent-trust adjacent topics — they may eventually integrate ATP-style protocols or even Atlas-style provenance, but unlikely to ship cryptographic memory verification themselves.

Sources:
- https://support.claude.com/en/articles/12138966-release-notes
- https://9to5mac.com/2026/05/07/anthropic-updates-claude-managed-agents-with-three-new-features/
- https://www.edtechinnovationhub.com/news/anthropic-brings-persistent-memory-to-claude-managed-agents-in-public-beta
- https://9to5mac.com/2026/03/02/free-claude-users-can-now-use-memory-and-import-context-from-rivals/
- https://lumichats.com/blog/claude-memory-2026-complete-guide-how-to-use

### 2.5 OpenAI Memory

- **License:** proprietary
- **Pricing (2026-05, ChatGPT tiers):**
  - Free — limited memory
  - Plus — $20/month
  - Pro — $200/month
  - Business — per-seat
  - Enterprise — custom, industry-reported $60–$100+ per seat/month, annual commitment
- **Features:**
  - Memory in ChatGPT (across conversations)
  - Memory in API for select models
  - Custom Instructions + Saved Memories distinct surfaces
  - Memory feature for Teams (consistency, reduced repetition)
- **User Base:** ChatGPT has ~hundreds of millions of weekly active users (OpenAI-reported, broadest consumer AI footprint).
- **Trust Property:** none cryptographic. Same vendor-silo model as Anthropic. Even larger blast radius if it goes wrong (more data, more dependency).
- **Atlas Differentiator:** Same as Anthropic — cross-vendor + verifiable vs single-vendor + opaque. OpenAI's enterprise sales motion is increasingly hitting buyers who *need* portability and auditability for regulatory reasons (EU AI Act Art. 12, sectoral regs). Atlas's pitch is the same buyer's natural answer.
- **Competitive threat assessment:** same as Anthropic — high consumer threat, low substrate threat. The bigger concern is *displacement* — buyers might think "ChatGPT Enterprise has memory, I don't need anything else" without realizing they have no audit trail.

Sources:
- https://openai.com/business/chatgpt-pricing/
- https://chatgpt.com/pricing/
- https://intuitionlabs.ai/articles/chatgpt-plans-comparison

### 2.6 Hindsight / Supermemory / Cognee / MemPalace — Short Coverage

| Product | License | Position | Trust Property | Atlas Threat |
|---|---|---|---|---|
| **Hindsight** (Vectorize.io) | MIT | Multi-strategy retrieval (semantic + BM25 + graph + temporal) with cross-encoder reranking. Highest LongMemEval score (91.4% with Gemini-3 Pro, claimed). | None cryptographic. | Low — Apache-style OSS competing on retrieval quality, not trust. Potential partner. |
| **Supermemory** | proprietary, closed-source | Vector-first similarity, personalization focus. Self-hosting requires enterprise contract. 85.4% LongMemEval (vendor-claimed, GPT-4o). | None. SOC-level vendor trust only. | Low. Closed-source moat in the opposite direction from Atlas's open-substrate thesis. |
| **Cognee** | Apache-2.0 | Knowledge-graph-first AI memory, OSS. | None cryptographic. | Low. Another partner candidate. |
| **MemPalace** | open-source (recent entrant) | Newer, less mature. | None. | Low. |
| **SuperLocalMemory** | Apache-2.0 | Local-first agent memory. | None. | Low. Philosophically aligned (local-first), no overlap on crypto-trust. |

**Strategic takeaway from §2.6:** the standalone-memory category is *crowded* (8+ named players in 2026 benchmarks) but **uniformly devoid of cryptographic trust properties**. This is Atlas's actual moat — not "we built a memory layer," but "we built the only memory layer where an auditor doesn't have to trust the vendor."

Sources:
- https://vectorize.io/articles/best-ai-agent-memory-systems
- https://vectorize.io/articles/supermemory-alternatives
- https://atlan.com/know/mem0-alternatives/
- https://dev.to/varun_pratapbhardwaj_b13/5-ai-agent-memory-systems-compared-mem0-zep-letta-supermemory-superlocalmemory-2026-benchmark-59p3

---

## 3. Second Brain Competitors

### 3.1 Obsidian

- **License:** proprietary app, free for personal use + commercial license for businesses (>2 users in commercial context). Plugins are community-licensed (mostly MIT/Apache).
- **Founded:** 2020 (by Erica Xu + Shida Li, Dynalist team)
- **Pricing (2026-05):**
  - Personal — **free** (full app, all features, all 2,500-2,750+ plugins, Bases included)
  - Sync — $4/month annual or $5/month
  - Publish — $8/month annual or $10/month
  - Catalyst — one-time $25+ (early-access + badge)
  - Commercial — required for >2 users in a business setting (separate per-seat pricing)
- **Features:**
  - Markdown files on local disk = ownership + portability
  - Bidirectional links, graph view, canvas, Bases (database-style views, new 2025)
  - 2,500-2,750+ community plugins (as of 2026-05)
  - Mobile + desktop, iOS / Android / macOS / Windows / Linux
- **User Base:** ~1.5M monthly active users, ~$25M ARR (estimated, vendor-reported). Heavy power-user + researcher concentration.
- **Trust Property:** **markdown files on disk** — closest existing thing to "self-sovereign trust." User owns the data. But: zero signature, zero tamper-evidence, zero verifiability. If a teammate edits your vault, you have no cryptographic record.
- **Plugin ecosystem check for Atlas-relevant signature/verification:**
  - **gpgCrypt** — GPG encryption of notes, **explicitly says no signature support**
  - **GPG Encrypt** — inline encryption only, no signatures
  - **git-crypt** workflows — encrypt-at-rest, no signing
  - **Conclusion:** **as of 2026-05, there is no signature/verification plugin in the Obsidian community catalog**. This is a genuine white space.
- **Atlas Differentiator:** Atlas's Verifiable Second Brain pitch is: "Obsidian-style ownership + cryptographic tamper-evidence + cross-device verifiable history." The natural product shape is *not* "compete with Obsidian" — it's "Atlas Obsidian Plugin" that signs every edit + anchors to Rekor + exposes a "Verify" button in the editor.
- **Threat assessment:** **LOW direct competitive threat** — Obsidian is a markdown editor, not a memory substrate. **MEDIUM partnership opportunity** — the plugin model is open and the 1.5M power-user audience is exactly the kind of buyer who'd opt into cryptographic trust voluntarily.
- **Honest assessment:** Building "Atlas-as-Obsidian-plugin" might be the cheapest, fastest distribution route to validate the "verifiable Second Brain" category exists at all. Open question for Phase 2.

Sources:
- https://obsidian.md/pricing
- https://fueler.io/blog/obsidian-usage-revenue-valuation-growth-statistics
- https://github.com/tejado/obsidian-gpgCrypt (gpgCrypt plugin)
- https://www.obsidianstats.com/plugins/gpg-encrypt

### 3.2 Notion

- **License:** proprietary SaaS
- **Founded:** 2016
- **Pricing (2026-05):**
  - Free — personal + small team basic
  - Plus — $12/user/month
  - Business — $20/user/month annual (or $24 monthly) — **includes unlimited Notion AI + Notion Agent**
  - Enterprise — custom
  - **Important change:** AI add-on ($8/user/month) was discontinued in May 2025; full AI is now bundled into Business
- **Features:**
  - Block-based docs + databases + wikis + AI search
  - **Notion AI** + **Notion Agent** (multi-step task automation, document drafting, DB querying)
  - Strong team-collaboration features
  - Templates marketplace
- **User Base:** **30M+ users worldwide** (vendor-reported). Strong product-led-growth motion, big in startups + design-leaning teams.
- **Trust Property:** none. Cloud-hosted SaaS, vendor-controlled, no signatures, no on-disk ownership. Notion goes down → your knowledge is unreachable.
- **Atlas Differentiator:** Notion is the *opposite philosophy* — cloud-first, vendor-managed, optimized for collaboration. Atlas is local-first / federated, verifiable, optimized for proving. Same buyer might use both (Notion for the live wiki, Atlas for the regulatory-grade signed record), but they don't compete on the same dimension.
- **Threat assessment:** **LOW.** Notion is not going to ship cryptographic memory verification — it's against their hosted-SaaS DNA. The risk is *attention* — knowledge workers default to Notion, never think to add a trust layer.

Sources:
- https://www.notion.com/pricing (cited via reseller pages — primary URL redirects)
- https://costbench.com/software/knowledge-management/notion-teams/
- https://felloai.com/notion-ai-pricing/

### 3.3 Roam Research

- **License:** proprietary
- **Founded:** 2019 (Conor White-Sullivan)
- **Pricing (2026-05):**
  - Pro — $15/month
  - Believer — $41.67/month ($500/5yr lifetime, works out to $8.33/month)
  - (No free tier)
- **Features:** bidirectional links, daily notes, block references, queries (originator of the "linked-thought" PKM aesthetic that Obsidian / Logseq inherited)
- **User Base:** ~1M monthly visitors (claimed). "Maintenance and refinement phase" — slow development since 2023, many power users migrated to Logseq/Obsidian.
- **Trust Property:** none.
- **Atlas Differentiator:** N/A — Roam is a fading incumbent, not a meaningful threat or partner.
- **Threat assessment:** **VERY LOW.** Atlas can name-drop Roam for category context but should not engineer for it.

Sources:
- https://costbench.com/software/note-taking/roam-research/
- https://blog.thefix.it.com/do-people-still-use-roam-research-discover-its-2026-status/

### 3.4 Logseq

- **License:** open-source (AGPL-3.0)
- **Pricing (2026-05):** **completely free**, sync still in beta
- **Features:** local-first (markdown + EDN), outliner-style, bidirectional links, queries, plugins. Philosophically the closest OSS analog to Roam.
- **User Base:** "open-source enthusiasts" niche — privacy-focused, no public user count. Smaller than Obsidian but devoted.
- **Trust Property:** none. Files-on-disk (like Obsidian) but no signature plugin in the ecosystem.
- **Atlas Differentiator:** Logseq is AGPL-3.0 (copy-left), which limits integration patterns — but they're philosophically aligned with Atlas (local-first + open). A Logseq plugin or fork-based integration could be a low-effort OSS-credibility play.
- **Threat assessment:** **VERY LOW direct threat, MEDIUM-LOW partnership opportunity.** Same shape as Obsidian but smaller audience.

Sources:
- https://toolradar.com/tools/logseq

### 3.5 Capacities, Tana, Heptabase — Short Coverage

| Product | License | Pricing | Position | Trust Property |
|---|---|---|---|---|
| **Capacities** | proprietary | Free + Pro $9.99/mo + Believer $12.49/mo | Object-based PKM, mobile-first, polished UX | None |
| **Tana** | proprietary | Free + Plus $10/user/mo + Pro $18/user/mo | Power-user outliner + DB hybrid, automation-heavy | None |
| **Heptabase** | proprietary | $8.99/mo annual + Lifetime $659 | Spatial whiteboard PKM, visual-first | None |

**Strategic takeaway from §3.5:** Each is a polished consumer/prosumer PKM tool with no trust/verification angle. None of them will ship cryptographic provenance in 2026. They're niche-targeting, not platform-aspiring — low overlap with Atlas's substrate thesis.

Sources:
- https://www.sollmannkann.com/project-management-and-notes/capacities-vs-tana/
- https://productivitystack.io/tools/tana/
- https://toolchase.com/tool/heptabase/

---

## 4. Knowledge Graph Tools (overlap with both categories)

### 4.1 Graphiti (Apache-2.0)

Already covered in §2.3. Summary in this context: **Apache-2.0 OSS, FalkorDB backend supported, no trust property. Strongest single partner candidate for V2.**

### 4.2 Neo4j

- **License:** Neo4j Community Edition — **GPLv3** (yes, copy-left, *not* AGPL despite common confusion). Neo4j Enterprise Edition — commercial closed-source.
- **Pricing (2026-05):**
  - Community Edition — free, GPL-3.0
  - Self-managed Enterprise — $3,000–$6,000 per core/year list; 16-core production typically $80K–$200K+/year
  - AuraDB managed cloud — $65–$146/GB/month (Professional → Business Critical)
- **Features:** industry-standard graph DB, Cypher query language (Neo4j originated Cypher), Neo4j Browser UI, broad GraphRAG ecosystem.
- **User Base:** dominant enterprise graph DB, ~10K+ enterprise customers (claimed).
- **Trust Property:** none beyond standard DB ACID + Neo4j Enterprise audit logs.
- **Atlas Use Case:** could *host* an Atlas graph projection (instead of FalkorDB) for organizations that already have Neo4j licenses. The Atlas projector would output Cypher writes; trust property still lives in events.jsonl.
- **Threat assessment:** **LOW direct threat** (Neo4j is a DB, not a memory substrate). **HIGH integration value** for enterprise customers — many large orgs already have Neo4j licenses and graph teams.
- **Tradeoff for V2:** Neo4j Community Edition GPL-3.0 has the same kind of copy-left concern as SSPL but better-understood; AuraDB managed pricing is enterprise-only.

Sources:
- https://neo4j.com/pricing/
- https://github.com/neo4j/neo4j
- https://news.ycombinator.com/item?id=14433330

### 4.3 FalkorDB (V2 planned stack)

- **License:** **SSPLv1** — not OSI-approved, free for in-process use, commercial license required to offer FalkorDB-as-a-service to third parties.
- **Pricing (2026-05):** Cloud pricing inside the cloud instance UI; sales-gated for enterprise. ~$0.001/query retrieval cost reported (vendor-claimed).
- **Features:**
  - Property graph, Cypher subset
  - GraphBLAS sparse-matrix backend — sub-ms p99 traversal claim (vendor)
  - FalkorDB Browser (separate Next.js app, embeddable or self-hosted)
  - **GraphRAG SDK 1.0 (April 2026) — ranked #1 on GraphRAG-Bench across all 4 task types** (Fact Retrieval, Complex Reasoning, Contextual Summarization, Creative Generation). Vendor-claimed 69.73 overall score, 14pt lead over next competitor.
- **User Base:** growing fast in 2025-2026 (GraphRAG-Bench #1 is a meaningful credential), no public user-count.
- **Trust Property:** none cryptographic. Standard DB ACID.
- **Atlas Use Case:** **planned V2 graph storage layer.** Atlas projector writes here; FalkorDB Browser optionally serves as the Atlas Graph Explorer UI surface (with a "Verify" button bolt-on).
- **License risk for Atlas:** SSPL is the single biggest license-risk in V2. If Atlas ever hosts FalkorDB-backed services for third-party customers (the obvious atlas-trust.dev/explorer path), SSPL triggers and requires either a commercial license OR open-sourcing the surrounding service stack. **Must validate with FalkorDB sales before committing.**

Sources:
- https://github.com/FalkorDB/FalkorDB
- https://www.openpr.com/news/4494136/falkordb-ships-graphrag-sdk-1-0-ranks-1-on-graphrag-bench
- https://www.falkordb.com/plans/

### 4.4 Kuzu (no longer pure OSS option — **status change since planning**)

- **License:** MIT (when active)
- **STATUS UPDATE 2026-05:** **Kuzu was acquired by Apple and the GitHub repository was archived in October 2025.** This is a significant change since the V2 planning doc was written.
- **Implication for Atlas:** Kuzu is no longer a viable "pure MIT alternative to FalkorDB if SSPL becomes a problem." Community forks exist (some listed on the GitHub mirrors) but none have meaningful maintenance velocity.
- **Replacement candidates** (require fresh evaluation):
  - **ArcadeDB** (Apache-2.0, multi-model graph + document) — most credible pure-OSS alternative for V2
  - **Memgraph** (BSL — Business Source License with eventual Apache-2.0 conversion)
  - **HugeGraph** (Apache-2.0, Baidu-backed) — less common in Western stacks
- **Threat assessment:** **the loss of Kuzu changes Atlas's OSS-fallback strategy.** If FalkorDB SSPL becomes blocking, the next option is no longer "swap to a similar MIT graph DB" — it's a bigger lift. This should escalate the priority of validating FalkorDB SSPL exposure with their sales team.

Sources:
- https://github.com/kuzudb/kuzu
- https://www.theregister.com/2025/10/14/kuzudb_abandoned/
- https://arcadedb.com/blog/neo4j-alternatives-in-2026-a-fair-look-at-the-open-source-options/

---

## 5. Trust / Verification Adjacent Tools

### 5.1 Sigstore Ecosystem

- **License:** Apache-2.0 (sigstore-rs, cosign, rekor, fulcio, etc.)
- **Status (2026-05):**
  - **Rekor v2 — General Availability:** redesigned to tile-backed transparency log, lower operational costs
  - **Adoption:** Homebrew (May 2024), PyPI (Nov 2024), Maven Central (Jan 2025), NVIDIA NGC model signing (July 2025)
  - **Sigstore-a2a:** Python lib + CLI for keyless signing of A2A (Agent-to-Agent) AgentCards using Sigstore + SLSA — directly adjacent to Atlas's positioning
  - **Sigstore Model Transparency:** ML supply-chain security project
- **Atlas relationship:** **already integrated.** Atlas V1 anchors to Sigstore Rekor; V1.19 ships with SLSA L3 build provenance verified via `npm audit signatures`. Sigstore is *infrastructure Atlas builds on*, not a competitor.
- **Implication:** Sigstore ecosystem expansion validates Atlas's foundational bet. The emergence of Sigstore-a2a (agent-card signing) shows the community is converging on Sigstore + SLSA for agent-side trust. Atlas's positioning is consistent and additive (events-and-memory layer on top of Sigstore-a2a's identity-and-cards layer).

Sources:
- https://github.com/sigstore/rekor
- https://github.com/sigstore/sigstore-a2a
- https://github.com/sigstore/model-transparency
- https://blog.sigstore.dev/

### 5.2 SLSA Framework

- **License:** OpenSSF specification (open)
- **Status:** SLSA Build L3 broadly adopted in 2026 (GitHub Actions native, OneUptime tutorials, Java/Maven adoption)
- **Atlas relationship:** **already implemented at L3 for npm publish lane.** Same as Sigstore — infrastructure, not competition. Atlas is one of the few "memory" projects with any SLSA implementation at all; this is a unique credibility signal in the AI-memory category.

Sources:
- https://github.blog/security/supply-chain-security/slsa-3-compliance-with-github-actions/
- https://oneuptime.com/blog/post/2026-02-09-slsa-level3-build-provenance/view

### 5.3 VeritasChain Protocol (VCP)

- **License:** open standard via VeritasChain Standards Organization (VSO), reference implementation on GitHub
- **Latest version:** VCP v1.1 released January 2026, **v1.2 Public Review Draft expected May/June 2026** — aligns with EU AI Act Articles 12, 19, 26, 72 + ESMA February 2026 Algorithmic Trading Supervisory Briefing
- **Position:** "cryptographic audit standards for algorithmic & AI-driven trading systems"
- **Features:**
  - Tamper-evident logs verifiable by third parties
  - "Flight recorder for algorithmic decisions" — captures outcomes + context + parameters
  - VCP-XREF dual-logging architecture for non-repudiable financial trade verification
  - AI transparency module (algorithm governance + human oversight + explainability)
  - Companion VAP (Verifiable AI Protocol) for non-trading AI accountability
- **User Base:** standards body, not a product — adoption metrics unclear. Vertical focus is financial trading + algorithmic compliance.
- **Atlas overlap:** **significant, vertical-specific.** VCP is solving the same shape of problem (cryptographic AI audit logs, EU AI Act alignment) but explicitly scoped to algorithmic trading. They publish a standard; Atlas ships a runtime.
- **Threat assessment:** **MEDIUM but vertical-specific.** If a financial-services buyer says "we need EU AI Act compliance for our trading AI," VCP is a credible standards-based answer. Atlas's answer is "we implement the same trust property, but our scope is broader (any agent memory, not just trading)." There may be a future where Atlas implements VCP's wire format as one of multiple output formats.
- **Partnership posture:** plausible. They're a standards body, not a product company — collaboration is likely easier than competition.
- **Insight:** VCP's emergence (and v1.2's explicit EU AI Act alignment) validates Atlas's regulatory-driver thesis. We are *not the only people* seeing this market.

Sources:
- https://veritaschain.org/
- https://veritaschain.org/what-is-vcp/
- https://www.opensourceforu.com/2026/01/veritaschain-releases-vcp-v1-1-for-verifiable-ai-trading-audit-trails/
- https://dev.to/veritaschain/vcp-v11-building-cryptographic-audit-trails-for-ai-trading-systems-after-the-2026-silver-crash-1gkp

### 5.4 Other 2025-2026 "AI Trust" Projects

**Lyrie / Agent Trust Protocol (ATP)** — *most strategically important new entrant*
- **Announced:** May 11, 2026 by OTT Cybersecurity LLC (Dubai-founded)
- **Funding:** $2M preseed (May 2026)
- **License:** open + royalty-free; reference implementation MIT at github.com/OTT-Cybersecurity-LLC/lyrie-ai
- **Standardization track:** IETF submission planned
- **Notable:** **accepted into Anthropic's Cyber Verification Program (CVP)** — first batch
- **Scope:** identity / scope / attestation / delegation / revocation for AI agents
- **Crypto primitives:** Ed25519-signed agent identity, action receipts, scope declarations, trust chain rules
- **Stated thesis:** "the trust layer underneath the agentic AI economy, like SSL/TLS for the web"
- **Atlas overlap:** **significant but adjacent, not overlapping.** ATP is about *agent identity* (who is the agent, what can it do); Atlas is about *agent memory* (what does the agent know, can we prove it). The natural pattern: an agent has an ATP identity AND writes through Atlas to a verifiable knowledge log. Both Ed25519-based, both IETF-track-adjacent.
- **Threat assessment:** **LOW direct overlap, HIGH ecosystem-shaping influence.** If ATP becomes the IETF standard for agent identity, Atlas needs to integrate (use ATP identities as the signer keys for events).
- **Partnership posture:** strong — different scope, complementary primitives, both crypto-trust-aligned.

**DigiCert AI Trust Architecture**
- **Announced:** April 30, 2026
- **Scope:** authentication + governance for AI agents + cryptographic protection/verification for AI models + content authenticity (C2PA-aligned)
- **Position:** enterprise CA-aligned, B2B sales motion via existing DigiCert customer base
- **Atlas overlap:** moderate. DigiCert is moving into the same trust-substrate space but with a top-down CA/PKI model rather than Atlas's federation-of-witnesses model. Both can co-exist; DigiCert customers might prefer the CA path, federation purists prefer Atlas.
- **Threat assessment:** **MEDIUM enterprise sales threat in 2027+.** DigiCert has existing relationships with Fortune 500 compliance teams. Atlas's defense is the open-substrate property — DigiCert needs trust-in-DigiCert, Atlas needs trust-in-math.

**C2PA / Content Credentials**
- Open technical standard for content provenance ("nutrition label for digital content")
- Adjacent (content-side trust), not directly competitive (memory-side trust)
- Atlas could output C2PA-formatted attestations as one of multiple sink formats

**AI-BOM ecosystem**
- AI Bill of Materials standards (CycloneDX MLBOM, SPDX 3.0.1 AI Profile, CISA SBOM for AI)
- The Register (May 2026): "AI-BOMs replace SBOMs as way to track AI agents and bots"
- Atlas relevance: an AI-BOM that lists "memory substrate: atlas v2.x.y" is the natural enterprise-procurement entry point for Atlas. Atlas itself can also *output* AI-BOM fragments from its event log.

Sources:
- https://securityboulevard.com/2026/05/news-alert-lyrie-ai-joins-anthropic-verification-program-unveils-protocol-for-securing-ai-agents/
- https://techstartups.com/2026/05/11/lyrie-completes-2-million-preseed-round-to-build-the-security-layer-for-the-ai-agent-era/
- https://github.com/OTT-Cybersecurity-LLC/lyrie-ai
- https://www.globenewswire.com/news-release/2026/04/30/3284921/0/en/DigiCert-Introduces-New-AI-Trust-Architecture-for-Securing-AI-Agents-Models-and-Content.html
- https://www.theregister.com/2026/05/04/ai_bom_supply_chain/
- https://www.wiz.io/academy/ai-security/ai-bom-ai-bill-of-materials

---

## 6. Comparison Matrix

Rows = competitors, columns = (License / Pricing-Range / Trust-Property / Open-Source / Multi-Agent / Temporal / Provenance-API / GDPR-Compliant-by-design).
Legend: ✓ = yes / strong, ✗ = no / absent, ~ = partial / claim-only / vendor-controlled, ? = unclear.

```
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Product              | License        | Pricing            | Trust Property  | OSS?  | Multi-   | Temp- | Provenance   | GDPR-by- |
|                      |                |                    |                 |       | Agent?   | oral? | API?         | design?  |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Mem0 / Mem0g         | Apache-2.0 +   | Free → $249/mo +   | None (SOC2/HIPAA| ✓     | ✓        | ~     | ✗            | ~        |
|                      | commercial     | Enterprise         | vendor only)    |       | (multi-  |       |              | (vendor  |
|                      |                |                    |                 |       | tenant)  |       |              | claim)   |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Letta (MemGPT)       | Apache-2.0     | Free OSS,          | None            | ✓     | ✓        | ✗     | ✗            | ~        |
|                      |                | $20-$200/mo cloud  |                 |       |          |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Zep + Graphiti       | Graphiti       | Free OSS,          | None            | ✓     | ✓        | ✓     | ✗            | ~        |
|                      | Apache-2.0;    | Zep Cloud usage    | (bi-temporal !=|       |          | (bi-  |              |          |
|                      | Zep proprietary| based              |  verifiable)    |       |          | temp) |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Anthropic Memory     | Proprietary    | Bundled $0-$20+/mo | None (vendor    | ✗     | ✗        | ✗     | ✗            | ✗        |
|                      |                |                    | silo)           |       | (Claude  |       |              |          |
|                      |                |                    |                 |       | only)    |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| OpenAI Memory        | Proprietary    | $0-$200/mo + Ent.  | None (vendor    | ✗     | ✗        | ✗     | ✗            | ✗        |
|                      |                | $60-100+ /seat/mo  | silo)           |       |          |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Hindsight            | MIT            | Free OSS           | None            | ✓     | ✓        | ✓     | ✗            | ~        |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Supermemory          | Proprietary    | Closed, ent. only  | None            | ✗     | ✓        | ~     | ✗            | ~        |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Obsidian             | Proprietary    | Free + $4-$10/mo   | Files-on-disk   | ✗     | ✗        | ✗     | ✗            | ~        |
|                      | (app),         | sync/publish       | (no signature)  |       | (single  |       |              | (local)  |
|                      | community      |                    |                 |       | user)    |       |              |          |
|                      | plugins        |                    |                 |       |          |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Notion               | Proprietary    | Free + $12-$24/    | None            | ✗     | ~        | ✗     | ✗            | ~        |
|                      | SaaS           | user/mo            |                 |       | (collab) |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Logseq               | AGPL-3.0       | Free               | Files-on-disk   | ✓     | ✗        | ✗     | ✗            | ~        |
|                      |                |                    | (no signature)  |       |          |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Roam Research        | Proprietary    | $15-$41.67/mo      | None            | ✗     | ✗        | ✗     | ✗            | ~        |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| FalkorDB             | SSPLv1         | Free in-process,   | None            | ~     | ✓ (DB    | ✗     | ✗            | ~        |
|                      |                | cloud usage-based  | (DB ACID only)  | (SSPL)| level)   |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Neo4j (Community)    | GPL-3.0        | Free / $80K+/yr    | None            | ✓     | ✓ (DB    | ✗     | ✗            | ~        |
|                      | / commercial   | enterprise         | (DB audit log)  | (GPL) | level)   |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| VCP (VeritasChain)   | Open standard  | Free standard      | ✓ Crypto audit  | ✓     | ?        | ✓     | ✓ (trading-  | ✓        |
|                      | (VSO)          | (no product)       | log (trading-   |       | (multi-  |       | specific)    |          |
|                      |                |                    | scoped)         |       | system)  |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Lyrie / ATP          | Open + MIT     | Free standard +    | ✓ Agent         | ✓     | ✓        | ~     | ~ (identity- | ~        |
|                      | ref. impl.     | ref impl           | identity +      |       |          |       | scoped)      |          |
|                      |                |                    | attestation     |       |          |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| DigiCert AI Trust    | Proprietary    | Enterprise CA      | ✓ CA-rooted     | ✗     | ✓        | ~     | ✓            | ~        |
|                      |                | pricing            | agent/model     |       |          |       |              |          |
|                      |                |                    | trust           |       |          |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| Sigstore-a2a         | Apache-2.0     | Free OSS           | ✓ Agent card    | ✓     | ✓        | ✗     | ✓ (cards)    | ~        |
|                      |                |                    | signing + SLSA  |       |          |       |              |          |
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
| ATLAS V2 (planned)   | Apache-2.0     | Free OSS verifier  | ✓ Ed25519+COSE  | ✓     | ✓        | ✓     | ✓ (per-event| ✓ (signed|
|                      | verifier +     | + hosted-service   | + Rekor anchor  |       | (any     | (V2-δ | + audit      | hash    |
|                      | Sustainable    | tier (TBD)         | + WASM offline  |       | agent /  | bi-   | trail API)   | sep.    |
|                      | Use server     |                    | + witness       |       | MCP/HTTP)| temp) |              | from raw)|
+----------------------+----------------+--------------------+-----------------+-------+----------+-------+--------------+----------+
```

**Reading the matrix:** Atlas is the only row with all of (cryptographic trust property, multi-agent multi-vendor, OSS verifier, GDPR-compliant-by-design via signed-hash / raw-content separation, public provenance API). The closest peers are VCP (trading-only scope) and Sigstore-a2a (agent-card-only scope) — both are *narrower than Atlas's substrate ambition*. The closest *broader-scope* peer is DigiCert AI Trust — but that's proprietary and CA-rooted, not federation-rooted.

---

## 7. Strategic Insights

### 7.1 Most threatening competitor

**Anthropic Memory + OpenAI Memory, together, by default-displacement.**

Neither is a direct competitor to Atlas's substrate thesis. Both are mortal threats by *capturing the buyer's mental model first*. The risk isn't "Anthropic ships cryptographic memory" — extremely unlikely, against their hosted-vendor DNA. The risk is "every enterprise buyer says 'we already use ChatGPT Enterprise + Claude for Work, what else do we need?'" without realizing they have zero audit trail.

Defense: **regulatory specificity.** EU AI Act Art. 12 ("independently verifiable") cannot be satisfied by vendor-controlled memory. Sectoral regs (DORA, GAMP 5, ICH E6 R3) similarly require third-party verifiability. Atlas's pitch is the buyer's natural answer once they read the regulation closely.

**Runner-up threat: Lyrie ATP becoming an IETF standard and absorbing the "agent trust" mindshare** before Atlas reaches the same conversation. Defense: integrate ATP rather than compete with it. Both Ed25519, both Apache/MIT-aligned, both pre-standards.

### 7.2 Most natural partners

In rank order of strategic value:

1. **Graphiti (Zep team)** — Apache-2.0, FalkorDB backend, bi-temporal model, 23K stars, MCP server with hundreds of thousands of users. Single best partnership-and-not-competition fit in the entire landscape. Concrete next step: `graphiti-atlas-adapter` proof-of-concept showing Graphiti edges with Atlas signatures.

2. **Mem0** — Apache-2.0 core, AWS Strands SDK integration, 52K stars. Mem0g as fast-retrieval cache on top of Atlas as trust-authoritative log = the canonical hybrid architecture. Concrete next step: spec the cache-rebuild-from-events.jsonl protocol; outreach to Mem0 team.

3. **Hermes Agent (Nous Research)** — 60K stars, MIT, model-agnostic, #1 on OpenRouter as of 2026-05. **Note:** Hermes wasn't explicitly searched in this doc; per the handoff §4, Hermes is the planned primary demo agent and a strong partnership candidate. Distribution-multiplier potential is significant if Atlas ships as a first-class Hermes skill.

4. **Letta (MemGPT)** — Apache-2.0, UC Berkeley pedigree, 22K stars, model-agnostic. Letta's "git-backed memory" pattern is the closest existing thing to durable structured memory; Atlas can be the cryptographic layer underneath.

5. **Sigstore community + Lyrie ATP** — both standards-track, both Ed25519, both already adjacent to Atlas's wire format. Integration-not-competition. Lyrie's CVP acceptance shows Anthropic is paying attention to this layer.

6. **Obsidian plugin ecosystem** — 1.5M MAU, 2,750+ plugins, no signature plugin in the catalog as of 2026-05. Cheapest, fastest distribution route to validate the "Verifiable Second Brain" category.

### 7.3 White spaces Atlas can own

1. **Cross-vendor verifiable memory substrate** — *no current player offers this*. Every AI-memory player is either vendor-siloed (Anthropic/OpenAI) or vendor-neutral-but-trust-blind (Mem0/Letta/Zep/Hindsight/Supermemory).

2. **EU AI Act Art. 12 "independently verifiable" — out of the box** — VCP comes closest but is trading-vertical-scoped. Atlas's substrate is general.

3. **Agent passports / portable cryptographic reputation** — Lyrie ATP handles identity, but no one is shipping "verifiable history of what this agent wrote, across organizations." Atlas's events.jsonl + per-agent Ed25519 keys natively provides this primitive.

4. **Signed-hash-separable-from-raw-content GDPR pattern** — the architecture that lets Atlas claim "GDPR-compliant by design" (right-to-be-forgotten survives because raw content is deletable while signed hash + anchor remain). No memory player has this; Notion/Obsidian don't think about it; vendor-silo memories punt to "delete on request."

5. **Verifiable Second Brain for individuals** — aspirational, but real. Obsidian has the audience, gpgCrypt has *encryption* but explicitly no *signatures*. A "Sign every edit + anchor to Rekor" plugin pattern is currently a literal white space in the 2,750-plugin catalog.

### 7.4 Most likely competitor counter-moves

1. **Mem0 ships "audit mode"** with simple HMAC-signed events. Most probable: medium probability, medium-low impact (Atlas's structural depth — Rekor anchoring + witness cosignature + WASM verifier — is hard to replicate as a feature; Mem0 would need to fork its core data model).

2. **Anthropic / OpenAI ship "export your memory in signed format"** as a regulator-pacification feature. Low-medium probability. If they ship vendor-signed exports without third-party witness cosignature, the audit property is still vendor-controlled — Atlas's federation-of-witnesses pitch still beats it.

3. **Graphiti adds cryptographic edge signing.** Medium probability over 12-18 months — they have the technical chops and the right ecosystem position. Defense: partner with them *now*, before they decide to compete.

4. **Lyrie ATP absorbs the agent-memory verification niche** by extending ATP scope from identity to action receipts to memory writes. Medium probability over 12-24 months. Defense: integrate ATP early, contribute to the IETF draft, make Atlas-events-using-ATP-identities the reference implementation pattern.

5. **DigiCert ships "AI memory trust" as part of AI Trust Architecture.** Low probability — DigiCert's enterprise CA model is philosophically opposite to federation-of-witnesses. If they did, the natural defense is the open-substrate property (mathematically verifiable vs trust-in-DigiCert).

6. **A new well-funded startup combines Mem0 + Sigstore + Lyrie ATP** into one product. Probability: rising, given Lyrie's $2M preseed in May 2026 and the AI-BOM tailwind in May 2026 trade press. Defense: ship V2 fast, take credit for being first, lean on the V1 trust-property maturity (347 tests, SLSA L3, signed tags) as defensive moat.

---

## 8. Open Questions for Phase 2 Critique

1. **Did we miss any competitor?** Specifically:
   - **Cognee** (Apache-2.0, KG-first memory) was mentioned in §2.6 but not deeply researched. Worth a dedicated subsection?
   - **EverMind**, **MemPalace**, **SuperLocalMemory** were name-dropped. Any of them ship cryptographic features?
   - **Memgraph** as Neo4j alternative? Not researched here.
   - **Cisco's open-source AI-BOM tool** mentioned in The Register article — does it overlap with Atlas's "AI-BOM substrate" angle?

2. **Is the "verifiable Second Brain" category real or aspirational?** Honestly today it is aspirational. The 1.5M Obsidian MAU includes many privacy-aware users, but how many would *pay* for cryptographic trust as a feature? Phase 2 should challenge: is there real demand, or is this market we *believe should exist* without evidence?

3. **Are Mem0g and Graphiti truly partners, or future competitors?** Both are open-source, both have raised venture capital ($24M for Mem0, €7.5M for Zep). Venture-backed OSS frequently encroaches on adjacent value. Specifically: how likely is Graphiti to add edge signing in the next 18 months, and does Atlas's V1-trust-property maturity (V1.0.1 LIVE on npm, 347 tests, SLSA L3) provide enough moat?

4. **Hermes Agent's 60K-stars-in-2-months trajectory — is it real adoption or hype?** The handoff cites Hermes as #1 on OpenRouter and a primary demo partner. If Hermes plateaus or gets supplanted (Letta Code is currently #1 on Terminal-Bench, suggesting Letta could pull mindshare), the distribution play needs to pivot.

5. **Does FalkorDB SSPL block the Atlas hosted-service tier?** This is the single biggest license-risk in V2. Kuzu's acquisition by Apple eliminated the obvious MIT fallback. Phase 2 needs to either (a) get a written commercial-license quote from FalkorDB sales, (b) commit to ArcadeDB (Apache-2.0) as the OSS fallback, or (c) decide hosted service is post-V2.

6. **Is "Atlas Obsidian Plugin" a faster validation path than full V2-α/β/γ?** Building a working sign-every-edit + anchor-to-Rekor plugin for Obsidian could ship in 1-2 weeks, validates the verifiable-Second-Brain demand signal, and gives Atlas its first real users *before* the V2 graph layer is mature.

7. **Should Atlas integrate Lyrie ATP as the agent-identity layer, or build a parallel Atlas-DID scheme?** Per Doc B §2.7 the plan is `did:atlas:<pubkey-hash>`. But ATP is also Ed25519-based, also IETF-track, also MIT reference impl. Integration cost might be lower than a parallel scheme — and aligns with the partnership posture.

8. **Does Atlas need to ship a benchmark on LOCOMO / LongMemEval / GraphRAG-Bench to be taken seriously?** Every competitor cites these benchmarks (Mem0g 68.4%, Hindsight 91.4%, FalkorDB GraphRAG SDK 69.73 overall). Atlas's value proposition is *not* retrieval accuracy, but absence from these benchmark leaderboards may be a credibility gap for AI-engineer buyers. Tradeoff: spend cycles on a benchmark vs. spend cycles on the trust differentiation that benchmarks don't capture.

9. **Is the matrix in §6 fair to competitors, or is the "Trust Property = ✓" column unfairly Atlas-coded?** A skeptical reviewer might argue Mem0's SOC 2 / HIPAA constitutes a trust property in practice. Phase 2 should challenge the column definitions and propose a more granular trust-property scoring.

10. **Should we segment "AI Agent Memory" further?** The category contains at least three distinct sub-personas: (a) consumer-app builders (need polish, vendor-managed), (b) enterprise platform teams (need SOC 2, multi-tenant, integration breadth), (c) regulated-industry builders (need EU AI Act compliance, audit trails). Atlas's pitch is strongest for (c), weakest for (a). Does the positioning need to be sharpened to lead with regulated-vertical buyers?

11. **The Kuzu acquisition by Apple — is this a one-off, or a pattern?** Big-tech absorption of OSS graph/memory infrastructure could happen again (FalkorDB, Mem0, Letta are all venture-backed and acquirable). Atlas's open-substrate strategy is defensive against this, but Phase 2 should check: what's our story if our recommended backend gets acquired and closed-sourced?

12. **Does the matrix capture all 8 column dimensions correctly for Atlas itself?** Specifically the "Temporal" column — Atlas's V1 already has signing-timestamps + Rekor-anchor-times + witness-cosignature-times. Is that *temporal* in the same sense as Graphiti's bi-temporal model? Or is Atlas's temporal property different in kind, and the matrix is implying parity that doesn't exist?

13. **Are we under-weighting the threat of vertical-specific compliance solutions** like VCP for trading? An enterprise CFO comparing "Atlas (general substrate)" vs "VCP (trading-specific, ESMA-aligned)" for a trading-AI use case might rationally pick VCP. Does Atlas need a "trading vertical" packaging, or does it lean into "general substrate, VCP-compatible output format"?

---

**Doc owner:** general-purpose subagent (Doc D, Phase 1). **Last edited:** 2026-05-12. **Next milestone:** Phase 2 critique agents challenge the open questions in §8; cross-consistency check against Docs A (positioning), B (architecture), C (risks), E (demos) in Phase 3.
