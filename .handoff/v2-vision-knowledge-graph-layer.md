# Atlas V2 — Vision: Verifiable Knowledge Graph Layer

> **Status:** Draft v0 (2026-05-12). Vision document for iteration with other agents — not yet an implementation plan, not yet wave-scoped. Designed to be passed to architecture / product / security agents for critique.

---

## 0. One-paragraph intent

Atlas V1.0.1 (npm-LIVE since 2026-05-12) ships the **write-side trust property**: every fact Ed25519+COSE-signed, hash-chained, Sigstore Rekor-anchored, offline-WASM-verifiable. What's still missing for "an agent can use Atlas as its memory and the human can audit what the agent knows" is the **read-side**: a queryable graph projection of the signed-event log plus a UI for humans (and verifiers) to explore that graph and prove every rendered fact back to its signed event. V2's job is to build that read-side without breaking the V1 trust invariant — events.jsonl remains the cryptographic source of truth; the graph DB is a derivative read-projection that must be **deterministically rebuildable** from events.jsonl.

---

## 1. Positioning — Atlas as agent-agnostic verification substrate

Atlas is **not an agent product**. Atlas is the *verifiable memory substrate that any agent can plug into to make its knowledge cryptographically auditable*. Tagline candidate: *"Knowledge graphs every AI agent can prove, not just claim."*

This positioning solves the Hermes-naming-collision (we never ship a branded agent), aligns with V1's existing design (atlas-mcp-server is already the universal write-side bridge for any MCP-compatible host — Claude Desktop, Cline, Continue, Cursor, custom MCP-SDK agents), and converts the competitive positioning question ("which agent should we bet on") into an integration question ("how easy do we make it for any agent to adopt Atlas").

Three user journeys V2 needs to serve:

### 1a. AI Agent (write side — already mostly solved in V1)

Any agent — Hermes-4, Claude, GPT-5, Llama, a domain-fine-tuned custom model, or a multi-agent framework like LangGraph/AutoGen — writes structured knowledge to Atlas through one of three surfaces:

- **MCP host** (Claude Desktop, Cline, Continue, Cursor, etc.): atlas-mcp-server (already live since V1.19 Welle 1) exposes Atlas as a set of MCP tools (`write_node`, `verify_trace`, future: `query_graph`, `query_entities`). Any MCP-1.29+ compatible agent gets Atlas integration for free.
- **HTTP API** (LangChain / LangGraph / AutoGen / CrewAI / OpenAI Agents SDK / custom): `POST /api/atlas/write-node` (V1.19 Welle 1 surface) accepts JSON, signs, anchors, appends. Framework-agnostic; any HTTP-capable agent integrates in <50 LOC.
- **Native library** (V2 candidate): a Python / TypeScript SDK wrapping the HTTP API with retry, type-safety, and a `verify()` helper.

Each write produces a signed event in `events.jsonl`; every downstream graph entity/edge derived from that event carries a `(event_uuid, rekor_log_index)` provenance pointer that any caller can cryptographically verify.

### 1b. AI Agent (read side — V2's contribution)

The same agent retrieves prior knowledge to ground its reasoning:
- **Semantic** ("what do we know about entity X")
- **Graph traversal** ("what's connected to X within N hops")
- **Temporal** ("what did we believe about X on date Y" — bi-temporal model)
- **Provenance-aware** (filter to "only Rekor-anchored facts" / "only witness-cosigned facts" / "exclude facts without N≥2 witness cosignatures")

V1 cannot do any of this today — agents can only re-read raw JSONL. V2-α (graph projection) and V2-β (query API) close the gap.

### 1c. Human auditor / regulator (explore + verify)

A human (operator, customer, regulator) needs to:
- **Open a graph explorer page** at `atlas-trust.dev/explorer` (or local atlas-web `/graph`) and see nodes + edges.
- **Filter / search**: by workspace, by entity type, by time window, by provenance status (signed-only / Rekor-anchored / witness-cosigned).
- **Drill into provenance**: click any node → see signed event → see Rekor inclusion proof → see witness cosignature → all verified inside browser via existing WASM verifier.
- **Detect tampering**: if the graph DB ever diverges from `events.jsonl`, the explorer surfaces it visibly (graph rebuild from JSONL produces a different hash → red banner).

The auditor's trust path bottoms out at the **same offline WASM verifier** that V1 ships. The graph DB never becomes trust-authoritative — it's a UX layer.

---

## 2. Architectural blueprint

```
┌──────────────────────────┐
│ AI Agent (write side)    │
│                          │
│  Agent decides to write  │
│  "fact F about entity E" │
└────────────┬─────────────┘
             │ POST /api/atlas/write-node
             ▼
┌──────────────────────────┐    APPEND-ONLY      ┌────────────────────────────────┐
│ atlas-web write surface  │────────────────────▶│  events.jsonl  (V1, AUTHORITATIVE) │
│ (V1.19 Welle 1 — live)   │   Ed25519+COSE      │  Sigstore Rekor anchored       │
└──────────────────────────┘                     │  witness-cosigned               │
                                                 └─────────────┬──────────────────┘
                                                               │
                                                               │ (tail / cron / event-driven)
                                                               ▼
                                              ┌────────────────────────────────────┐
                                              │ Atlas Projector (NEW V2)           │
                                              │ • verifies each event signature    │
                                              │ • extracts entities + relationships│
                                              │ • idempotent upsert into graph DB  │
                                              │ • stamps {event_uuid, rekor_idx}   │
                                              │   onto every node + edge property  │
                                              │ • deterministic projection version │
                                              └─────────────┬──────────────────────┘
                                                            ▼
                                              ┌────────────────────────────────────┐
                                              │ FalkorDB (V2 graph storage)        │
                                              │ • property graph, Cypher subset    │
                                              │ • GraphBLAS backend (fast traversal)│
                                              │ • SSPLv1 (commercial-hosting note) │
                                              └─────────────┬──────────────────────┘
                                                            ▼
                                              ┌────────────────────────────────────┐
                                              │ Atlas Graph Explorer (NEW V2)       │
                                              │ • atlas-web /graph page             │
                                              │ • OR embed FalkorDB Browser         │
                                              │ • OR Cytoscape.js / React Flow      │
                                              │ • every rendered fact → "Verify"    │
                                              │   button → opens WASM verifier      │
                                              └────────────────────────────────────┘
                                                            ▲
                                                            │ retrieval
┌──────────────────────────┐                                │
│ AI Agent (read side)     │────────────────────────────────┘
│                          │   semantic + graph + temporal
│  Agent queries memory    │   query layer
└──────────────────────────┘
```

### 2.1 Trust invariant

- `events.jsonl` is the **only authoritative source of truth**.
- The graph DB is a **derived read-projection** — destroyable, rebuildable, version-tagged.
- Graph DB content carrying a property not derivable from `events.jsonl` = trust regression.
- A consumer can always `git clone atlas + replay events.jsonl through the projector → byte-identical graph state` (modulo intentional projector-version-N upgrades, documented separately).

### 2.2 Graphiti — to use or not?

Graphiti (Apache-2.0, ~23K stars, FalkorDB backend officially supported since 2025) gives us:
- **Bi-temporal data model** out of the box: every edge carries `(t_valid, t_invalid)` *and* `(t'_created, t'_expired)` — meaning "when fact was true in world" vs "when we learned of it". This is the auditor's gold-standard temporal model and writing it from scratch is non-trivial.
- **LLM-driven entity/relationship extraction**: turns prose into graph deltas. Configurable LLM provider (Claude, OpenAI, Gemini, Ollama).
- **Hybrid retrieval**: semantic + BM25 + graph traversal.

But: **LLM extraction introduces non-determinism**. If we put Graphiti in the projector path, our "rebuildable graph from events.jsonl" property breaks — re-running the projector with a different LLM seed/version produces a different graph. Mitigation paths:
- (a) Cache LLM extractions keyed by event hash; rebuild reuses cache. Trust-clean, ops-burden +1.
- (b) Skip Graphiti in projector; only use it on the retrieval-side (semantic-search of node summaries, no graph writes from LLM). Simpler.
- (c) Write deterministic projectors (regex / structured-schema extraction) for the trust-load-bearing graph; layer Graphiti as an opt-in "AI-enhanced retrieval" on top. Defensible.

**Tentative recommendation:** (c). Trust-load-bearing entities/edges come from the deterministic projector. Graphiti is an optional retrieval-layer enhancement that the agent can use but the auditor doesn't need to trust.

### 2.3 FalkorDB — what we get

- **Production-ready** as of 2026 (per their April 2026 GraphRAG SDK 1.0 release, #1 on GraphRAG-Bench).
- **FalkorDB Browser** (separate Next.js app, embeddable or self-hosted) = closest analogue to Neo4j Browser. Real query workbench, schema inspector, results table.
- **Cypher-subset query language**: most Neo4j tutorials port directly.
- **GraphBLAS sparse-matrix backend**: claims sub-ms p99 traversal vs Neo4j ~90ms; meaningful for in-browser interactive exploration.
- **License: SSPLv1** — not OSI-approved, free for in-process use, **commercial license required to offer FalkorDB-as-a-service** to third parties. If `atlas-trust.dev/explorer` ever serves customers' graph data from a Nelson-hosted FalkorDB, this triggers. To validate with FalkorDB sales before committing.

### 2.4 Atlas Graph Explorer — UI options

Three paths, in order of effort:

| Option | Effort | What we get | Trust integration |
|---|---|---|---|
| Embed FalkorDB Browser at `/explorer` route | 1-2 days | Full Cypher workbench, free | Per-node "Verify" button needs to be added on top (read node property → call WASM verifier) |
| Build minimal Cytoscape.js viewer over events.jsonl directly | 1-2 sessions | Custom, brand-aligned, no Cypher | Native (reads signed events directly, no projection trust gap) |
| Build full custom viewer over FalkorDB Cypher API | 3-5 sessions | Brand-aligned + query power | Needs explicit provenance proof per rendered node |

**Tentative recommendation:** start with embedded FalkorDB Browser for "explore" + custom minimal viewer at `/graph` for "demo" (atlas-trust.dev/explorer can route to FalkorDB Browser; atlas-trust.dev landing page embed = custom viz of a tiny demo graph). Don't reinvent FalkorDB Browser, do invent the storytelling viz.

---

## 3. Open questions for iteration

These are what other agents (architect, security, product) should crit:

1. **SSPLv1 exposure.** Will Atlas ever expose graph query endpoints to customers? If yes, FalkorDB SSPL requires either commercial license OR open-sourcing the surrounding service stack. → Validate with FalkorDB sales **before** building on it. Alternative: Kuzu (MIT), Neptune (managed-only), Neo4j Community (GPLv3 — different but also restrictive).

2. **Projection determinism spec.** What does "deterministically rebuildable" mean operationally? Same event log → same graph state across machines, across projector versions, across time. Needs an explicit projector version + idempotent upsert protocol. Open: how do projector schema upgrades work (V1→V2 of the projector)?

3. **Bi-temporal mapping.** Atlas events already carry: (i) signing timestamp, (ii) Rekor anchor time, (iii) witness cosignature time. Graphiti's bi-temporal model wants `(t_valid, t_invalid, t'_created, t'_expired)`. Open: which Atlas timestamps map to which Graphiti coordinates? Is there a clean isomorphism, or does this need a custom temporal layer?

4. **Reference-integration agent for the demo.** Atlas is positioned agent-agnostic, but the landing-page demo needs *one specific* agent to show end-to-end. Candidates: (a) Claude via MCP through Claude Desktop (zero-code-on-our-side, ship MCP server config), (b) Hermes-4 / Llama-4 with custom system prompt + atlas-mcp-server (open-weight differentiator — independent of any model vendor), (c) Custom React-Native-Agent loop calling HTTP API directly (most self-contained demo, least dependency on third-party MCP host). Trade-off: (a) easiest demo + biggest distribution audience, but ties demo to Anthropic's UX; (b) strongest "vendor-neutral / works with open weights" narrative; (c) cleanest live-rendering control (we own the agent loop UI). → **Decision drives which framework gets the first reference-integration recipe; doesn't lock Atlas's positioning.**

5. **LLM extraction in projector — yes/no?** §2.2 (c) tentative-recommendation argues "no in projector, yes on retrieval side". Other agents should challenge: is deterministic regex/structured extraction enough? Or is LLM extraction load-bearing for the actual value? If LLM is needed, how do we keep the trust invariant?

6. **Demo-first vs Production-first sequencing.** V2 has two pulls: (a) build a credible end-to-end demo (Hermes-equivalent agent + writes + graph + explorer) for atlas-trust.dev landing; (b) build the production-grade projection + verifier-integrated explorer. Which comes first? See Welle-14b roadmap discussion separately.

7. **GraphRAG SDK 1.0 (FalkorDB-native) vs Graphiti.** Both extract entities, both build KGs for agents. FalkorDB's SDK is newer + vendor-aligned + #1 on GraphRAG-Bench. Graphiti is more mature in community / production use. Worth a head-to-head spike before locking in Graphiti.

8. **Witness/anchor surfacing in the graph.** A signed event becomes a graph node — but where do the Rekor anchor data and witness cosignatures live in the graph schema? Embedded as properties? Linked as separate "evidence" nodes? Hidden behind a UI affordance? Tradeoff: more visible = better trust narrative, but clutters the graph.

9. **Per-tenant key derivation in projection.** V1 already supports per-workspace HKDF-derived signing keys. The graph DB needs to either (a) namespace by workspace (one graph per tenant), (b) tag every node with `workspace_id` and rely on query-side filtering, (c) use FalkorDB's multi-graph feature. Tradeoff: isolation vs cross-workspace query.

10. **Read-side API shape.** When the agent (or atlas-web) queries the graph, what's the wire shape? Three options: (i) GraphQL over the property graph, (ii) Cypher-passthrough, (iii) opinionated REST surfaces (`/api/atlas/entities/:id`, `/api/atlas/related/:id`, etc.). Tradeoff: ergonomic vs expressive vs cacheable.

---

## 4. Suggested decomposition for iteration (NOT a wave-plan yet)

If we go all-in, the V2 work decomposes roughly into:

- **V2-α (Foundation, ~3-4 sessions):** FalkorDB integration as a derivable side-projection; deterministic projector with explicit schema version; events.jsonl → graph round-trip CI gate.
- **V2-β (Explorer UI, ~2-3 sessions):** Embed FalkorDB Browser at `/explorer`; build minimal Cytoscape.js demo at `/graph` for landing-page embed; "Verify this fact" button wired to WASM verifier per rendered node.
- **V2-γ (Reference integrations, ~2-3 sessions):** Three example integrations under `examples/integrations/` demonstrating Atlas's agent-agnosticism: (i) **Claude via MCP** — Claude Desktop config + atlas-mcp-server, zero-code-on-consumer-side; (ii) **LangGraph / LangChain** — Python recipe wiring Atlas HTTP API as a tool node; (iii) **Open-weight model (Hermes-4 or Llama-4)** — TypeScript or Python loop calling Atlas via HTTP + function calling. The third one is the Demo-Ready landing-page hero — vendor-neutral story, "watch this open-weight model write into the graph, verify every fact". Each integration is self-contained, runnable in <10 LOC of "atlas glue" beyond standard framework setup.
- **V2-δ (Optional — Graphiti integration, ~2-3 sessions):** If §2.2(c) tentative survives review, layer Graphiti as opt-in retrieval enhancement. Bi-temporal mapping spec'd against Atlas timestamps.

Total budget: ~9-13 sessions. Order is α → β-parallel-with-γ → δ-optional.

---

## 5. Where this fits with Welle 14b/14c

The current handoff doc §16 has Welle 14b as: (i) Trusted Publishers OIDC, (ii) dual-publish architecture fix, (iii) Demo-Ready surfaces (atlas-trust.dev landing + Quickstart + integrations), plus Welle 14c visual polish. This V2 vision **competes with Welle 14b-iii for the "what's the demo story" slot**. Options:

- **Defer V2 entirely** until 14b-{i,ii,iii} ships → atlas-trust.dev landing without graph layer (less compelling demo, but ships in 2-3 weeks).
- **Roll V2-α + V2-β into Welle 14b-iii** → landing page launches WITH a working graph explorer (delays 14b-iii by ~3 sessions but the demo lands 10x stronger).
- **Parallel tracks** → Welle 14b-{i,ii} (security/dist) ships fast; V2 work runs in parallel; 14b-iii landing waits until V2-α+β are demo-able.

Recommended: **option 2** if the goal is "credible Demo-Ready surface", **option 3** if the goal is "ship landing page fast + improve over time".

---

## 6. Iteration call-to-action

This doc is v0. Pass it to:
- **Architect agent**: critique §2 blueprint, particularly projector design + trust invariant.
- **Security agent**: critique §2.1 (trust invariant), §2.4 (SSPL exposure), §3.1 (license validation).
- **Product agent**: critique §1 (user journeys), §3.4 (naming), §3.6 (sequencing), §5 (Welle 14b alignment).
- **Database agent**: critique §2.3 (FalkorDB choice), §3.7 (GraphRAG SDK vs Graphiti).

Each crit should produce specific +/-1 sentence challenges or refinements per numbered open question. Don't rewrite the doc; surgically annotate.

---

**Doc owner:** Nelson + Atlas team. **Last edited:** 2026-05-12. **Next milestone:** iteration pass complete by 2026-05-15, then convert surviving recommendations into a Welle 14b-iv (or V2-α-spec) plan-doc.
