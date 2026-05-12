# Atlas V2 — Vision: Verifiable Knowledge Graph Layer

> **Status:** Draft v0.5 (2026-05-12). Revision of the v0 doc — adds the Mem0g hybrid three-layer architecture, Hermes-Agent skill integration surface, Ed25519-DID agent identity layer, V2 read-side API + MCP tool surface, federated regulator-witness architecture, and GDPR/right-to-be-forgotten content/hash separation. Vision document for iteration with other agents — not yet an implementation plan, not yet wave-scoped. Designed to be passed to Phase-2 architecture / product / security critique agents.

---

## 0. One-paragraph intent

Atlas V1.0.1 (npm-LIVE since 2026-05-12) ships the **write-side trust property**: every fact Ed25519+COSE-signed, hash-chained, Sigstore Rekor-anchored, offline-WASM-verifiable. What's still missing for "an agent can use Atlas as its memory and the human can audit what the agent knows" is the **read-side**: a queryable graph projection of the signed-event log, a fast-retrieval cache for agent latency budgets, a query/passport API for agents and humans, plus a UI for humans (and verifiers) to explore that graph and prove every rendered fact back to its signed event. V2's job is to build that read-side **without breaking the V1 trust invariant** — `events.jsonl` (wire-format spec: `crates/atlas-trust-core/src/trace_format.rs`) remains the cryptographic source of truth; every new layer (graph DB, retrieval cache, agent identity registry, regulator witness federation) is a **derivative, deterministically-rebuildable projection**.

---

## 1. Positioning — Atlas as agent-agnostic verification substrate

Atlas is **not an agent product**. Atlas is the *verifiable memory substrate that any agent can plug into to make its knowledge cryptographically auditable*. Tagline candidates:
- *"Knowledge graphs every AI agent can prove, not just claim."*
- *"The TÜV plate for AI agent memory."*
- *"Shared brain for humans and agents. Cryptographically yours."*

This positioning solves the Hermes-naming-collision (we never ship a branded Atlas-agent; Hermes is an agent that *uses* Atlas), aligns with V1's existing design (`atlas-mcp-server` is already the universal write-side bridge for any MCP-compatible host — Claude Desktop, Cline, Continue, Cursor, custom MCP-SDK agents), and converts the competitive positioning question ("which agent should we bet on") into an integration question ("how easy do we make it for any agent to adopt Atlas").

**Two-market story (per `.handoff/v2-session-handoff.md` §4):**
- **Human Second Brain** — Obsidian / Notion / Roam category, but with cryptographic trust as the bridge across devices, teammates, and AI-generated knowledge.
- **Multi-Agent Shared Memory** — every agent (Hermes, Claude, GPT, Llama, domain-finetunes) writes into the same verifiable knowledge base. The shared brain is *cross-vendor* by construction because nobody owns the bytes — Atlas owns only the verification primitives.

Three user journeys V2 needs to serve:

### 1a. AI Agent (write side — already mostly solved in V1)

Any agent — Hermes-4, Claude, GPT-5, Llama, a domain-fine-tuned custom model, or a multi-agent framework like LangGraph / AutoGen / CrewAI — writes structured knowledge to Atlas through one of three surfaces:

- **MCP host** (Claude Desktop, Cline, Continue, Cursor, etc.): `atlas-mcp-server` (live since V1.19 Welle 1) exposes Atlas as a set of MCP tools (`write_node`, `verify_trace`; V2 expands to `query_graph`, `query_entities`, `query_provenance`, `get_agent_passport`). Any MCP-1.29+ compatible agent gets Atlas integration for free.
- **HTTP API** (LangChain / LangGraph / AutoGen / CrewAI / OpenAI Agents SDK / custom): `POST /api/atlas/write-node` (V1.19 Welle 1 surface) accepts JSON, signs, anchors, appends. Framework-agnostic; any HTTP-capable agent integrates in <50 LOC.
- **Native library** (V2 candidate): Python / TypeScript SDK wrapping the HTTP API with retry, type-safety, agent-passport keypair management, and a `verify()` helper.

Each write produces a signed event in `events.jsonl`; every downstream graph entity/edge derived from that event carries a `(event_uuid, rekor_log_index, agent_did)` provenance tuple that any caller can cryptographically verify.

### 1b. AI Agent (read side — V2's contribution)

The same agent retrieves prior knowledge to ground its reasoning:
- **Semantic** ("what do we know about entity X")
- **Graph traversal** ("what's connected to X within N hops")
- **Temporal** ("what did we believe about X on date Y" — bi-temporal model)
- **Provenance-aware** (filter to "only Rekor-anchored facts" / "only witness-cosigned facts" / "exclude facts without N≥2 witness cosignatures" / "only facts authored by agents with passport reputation ≥ R")

V1 cannot do any of this today — agents can only re-read raw JSONL. V2-α (graph projection) + V2-β (query API + Mem0g cache) close the gap.

### 1c. Human auditor / regulator (explore + verify)

A human (operator, customer, regulator) needs to:
- **Open a graph explorer page** at `atlas-trust.dev/explorer` (or local atlas-web `/graph`) and see nodes + edges.
- **Filter / search**: by workspace, by entity type, by time window, by provenance status (signed-only / Rekor-anchored / witness-cosigned), by authoring agent DID.
- **Drill into provenance**: click any node → see signed event → see Rekor inclusion proof → see witness cosignatures (including regulator witness if federated) → all verified inside browser via existing WASM verifier (`crates/atlas-verify-wasm`).
- **Detect tampering**: if the graph DB ever diverges from `events.jsonl`, the explorer surfaces it visibly (graph rebuild from JSONL produces a different state hash → red banner).

The auditor's trust path bottoms out at the **same offline WASM verifier** that V1 ships. The graph DB never becomes trust-authoritative — it's a UX layer.

---

## 2. Architectural blueprint

```
┌──────────────────────────┐                                ┌──────────────────────────┐
│ AI Agent A (Hermes)      │                                │ AI Agent B (Claude/MCP)  │
│  did:atlas:hermes-abc…   │                                │  did:atlas:claude-def…   │
└────────────┬─────────────┘                                └────────────┬─────────────┘
             │ POST /api/atlas/write-node                                │
             │ + Ed25519 sig over body w/ agent's DID-key                │
             ▼                                                           ▼
                          ┌────────────────────────────────────┐
                          │ atlas-web write surface (V1.19 W1) │
                          │  • Zod validation                  │
                          │  • per-workspace mutex             │
                          │  • atlas-signer (Ed25519+COSE)     │
                          │  • redactPaths defensive layer     │
                          └────────────┬───────────────────────┘
                                       │ APPEND-ONLY
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────────┐
│  events.jsonl   ←  AUTHORITATIVE  (V1; spec at crates/atlas-trust-core/src/      │
│                                          trace_format.rs)                         │
│  • Ed25519 + COSE_Sign1 over deterministic CBOR                                  │
│  • hash-chained DAG (parent_hashes), strict-linear-chain opt-in (V1.19 Welle 9)  │
│  • Sigstore Rekor-anchored (V1.6; bundle_hash + dag_tip kinds)                   │
│  • anchor-chain tip-rotation (V1.7)                                              │
│  • per-tenant HKDF Ed25519 keys (V1.9; atlas-anchor:<workspace>)                 │
│  • witness cosignatures (V1.13; threshold-strict)                                │
│  • content_hash field (raw content stored separately — see §3.3 GDPR)            │
└─────────────┬────────────────────────────────────────────────────────────────────┘
              │ tail / cron / event-stream
              ▼
   ┌───────────────────────────────────┐         ┌─────────────────────────────────┐
   │ Atlas Projector  (NEW V2-α)       │────────▶│  Sigstore Rekor (anchor log,    │
   │  • verifies each event signature  │ already │  V1.6, public OR private        │
   │  • extracts entities + relations  │ in V1   │  shard — see §3.2)              │
   │  • idempotent upsert into graph DB│         └─────────────────────────────────┘
   │  • stamps {event_uuid, rekor_idx, │
   │     agent_did} onto each node/edge│
   │  • deterministic projector_version│
   │  • emits projector-state-hash gate│
   └────────────┬──────────────────────┘
                ▼
   ┌────────────────────────────────────────┐
   │  FalkorDB (V2-α; QUERYABLE projection) │
   │  • property graph, Cypher subset       │
   │  • GraphBLAS sparse-matrix backend     │
   │  • per-workspace logical graph         │
   │  • SSPLv1 (see §3.1)                   │
   └────────────┬───────────────────────────┘
                │ embed-on-projection
                ▼
   ┌────────────────────────────────────────┐
   │  Mem0g (V2-β; FAST RETRIEVAL cache)    │
   │  • graph-enhanced semantic memory      │
   │  • 91% p95 latency reduction vs FC     │
   │  • 2.59s p95, <5pt accuracy gap        │
   │  • REBUILDABLE from FalkorDB           │
   │    (which is rebuildable from JSONL)   │
   │  • NEVER trust-authoritative           │
   └────────────┬───────────────────────────┘
                ▼
   ┌────────────────────────────────────────┐         ┌────────────────────────────┐
   │ Atlas Read-Side API (NEW V2-β)         │◀────────│ Atlas MCP V2 tools         │
   │  GET    /api/atlas/entities/:id        │         │  query_graph               │
   │  GET    /api/atlas/related/:id?depth=N │         │  query_entities            │
   │  GET    /api/atlas/timeline/:workspace │         │  query_provenance          │
   │  POST   /api/atlas/query               │         │  get_agent_passport        │
   │  GET    /api/atlas/audit/:event_uuid   │         │  (atlas-mcp-server, V2)    │
   │  GET    /api/atlas/passport/:did       │         └────────────┬───────────────┘
   └────────────┬───────────────────────────┘                      │
                ▼                                                  │ retrieval
   ┌────────────────────────────────────────┐                      │
   │ Atlas Graph Explorer (NEW V2-β)        │                      │
   │  • atlas-web /graph page               │                      │
   │  • OR FalkorDB Browser embed           │                      │
   │  • OR Cytoscape.js custom viewer       │                      │
   │  • every rendered fact → "Verify"      │                      │
   │    button → opens WASM verifier        │                      │
   │  • per-node DID + reputation chip      │                      │
   └────────────────────────────────────────┘                      │
                ▲                                                  │
                │ retrieval                                        │
   ┌────────────┴─────────────┐                                    │
   │ AI Agent (read side)     │◀───────────────────────────────────┘
   │                          │   semantic + graph + temporal +
   │  Agent queries memory    │   provenance-filtered + passport-
   │                          │   weighted
   └──────────────────────────┘
```

### 2.1 Trust invariant

- `events.jsonl` is the **only authoritative source of truth**.
- Every additional layer in V2 (FalkorDB projection, Mem0g cache, agent-passport registry, regulator-witness federation) is a **derived read-projection** — destroyable, rebuildable, version-tagged.
- A consumer can always `git clone atlas + replay events.jsonl through the projector → byte-identical graph state` (modulo intentional `projector_version` upgrades, documented separately).
- **What if any V2 layer is corrupted, lost, or attacked?** V1's trust property survives unchanged. Worst case: re-run the projector on `events.jsonl`, regenerate FalkorDB; re-run Mem0g embedding on FalkorDB, regenerate the cache. All trust evidence (signatures, anchors, witness cosignatures) lives in `events.jsonl`, not in any V2 layer.
- **The CI gate:** events.jsonl → projector(version N) → graph-state-hash must be deterministic across machines and time. Diverging projector-state-hashes between any two replays = trust regression, treat as P0.

### 2.2 Graphiti — to use or not?

Graphiti (Apache-2.0, ~23K stars, FalkorDB backend officially supported since 2025) gives us:
- **Bi-temporal data model** out of the box: every edge carries `(t_valid, t_invalid)` *and* `(t'_created, t'_expired)` — meaning "when fact was true in world" vs "when we learned of it". This is the auditor's gold-standard temporal model and writing it from scratch is non-trivial.
- **LLM-driven entity/relationship extraction**: turns prose into graph deltas. Configurable LLM provider (Claude, OpenAI, Gemini, Ollama).
- **Hybrid retrieval**: semantic + BM25 + graph traversal.

But: **LLM extraction introduces non-determinism**. If we put Graphiti in the projector path, our "rebuildable graph from events.jsonl" property breaks — re-running the projector with a different LLM seed/version produces a different graph. Mitigation paths:
- (a) Cache LLM extractions keyed by event hash; rebuild reuses cache. Trust-clean, ops-burden +1.
- (b) Skip Graphiti in projector; only use it on the retrieval-side (semantic-search of node summaries, no graph writes from LLM). Simpler.
- (c) Write deterministic projectors (regex / structured-schema extraction) for the trust-load-bearing graph; layer Graphiti as an opt-in "AI-enhanced retrieval" on top. Defensible.

**Tentative recommendation:** (c). Trust-load-bearing entities/edges come from the deterministic projector. Graphiti is an optional retrieval-layer enhancement that the agent can use but the auditor doesn't need to trust. Mem0g (see §2.5) supersedes most of Graphiti's retrieval value anyway; Graphiti's bi-temporal model is the one piece worth selectively borrowing — see §2.5 for the Atlas-timestamp → bi-temporal-edge mapping.

### 2.3 FalkorDB — what we get

- **Production-ready** as of 2026 (per their April 2026 GraphRAG SDK 1.0 release, #1 on GraphRAG-Bench).
- **FalkorDB Browser** (separate Next.js app, embeddable or self-hosted) = closest analogue to Neo4j Browser. Real query workbench, schema inspector, results table.
- **Cypher-subset query language**: most Neo4j tutorials port directly.
- **GraphBLAS sparse-matrix backend**: claims sub-ms p99 traversal vs Neo4j ~90ms; meaningful for in-browser interactive exploration and Mem0g hot-path queries.
- **License: SSPLv1** — not OSI-approved, free for in-process use, **commercial license required to offer FalkorDB-as-a-service** to third parties. If `atlas-trust.dev/explorer` ever serves customers' graph data from a Nelson-hosted FalkorDB, this triggers. To validate with FalkorDB sales before committing. Fallback: Kuzu (MIT, embeddable, OSI-approved) — keep the projector's storage adapter abstract so swapping is a configuration change.

### 2.4 Atlas Graph Explorer — UI options

Three paths, in order of effort:

| Option | Effort | What we get | Trust integration |
|---|---|---|---|
| Embed FalkorDB Browser at `/explorer` route | 1-2 days | Full Cypher workbench, free | Per-node "Verify" button needs to be added on top (read node property → call WASM verifier) |
| Build minimal Cytoscape.js viewer over events.jsonl directly | 1-2 sessions | Custom, brand-aligned, no Cypher | Native (reads signed events directly, no projection trust gap) |
| Build full custom viewer over FalkorDB Cypher API | 3-5 sessions | Brand-aligned + query power | Needs explicit provenance proof per rendered node |

**Tentative recommendation:** start with embedded FalkorDB Browser for "explore" + custom minimal viewer at `/graph` for "demo" (atlas-trust.dev/explorer can route to FalkorDB Browser; atlas-trust.dev landing page embed = custom viz of a tiny demo graph). Don't reinvent FalkorDB Browser, do invent the storytelling viz.

---

### 2.5 Three-Layer Architecture — `events.jsonl` + FalkorDB + Mem0g

V2 introduces a strict three-layer hierarchy. Each layer is **derivative of the layer above** and **carries a strict invariant about trust authority**.

```
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 1 — events.jsonl       (AUTHORITATIVE,  cryptographic)       │
│   ↓ deterministic projector                                          │
│  Layer 2 — FalkorDB           (QUERYABLE,      structurally rebuilt) │
│   ↓ embedding + summarisation                                        │
│  Layer 3 — Mem0g cache        (FAST,           semantically rebuilt) │
└─────────────────────────────────────────────────────────────────────┘
```

#### Layer 1: `events.jsonl` (authoritative)

- Wire-format spec: `crates/atlas-trust-core/src/trace_format.rs` (`AtlasTrace`, `AtlasEvent`, `EventSignature`)
- Append-only, hash-chained, Ed25519+COSE-signed, Rekor-anchored
- This is the **only** layer with trust authority. If Layers 2 and 3 are wiped, V1's trust property survives — verifier still produces ✓ VALID against `events.jsonl` + pubkey bundle.
- Operational note: this is also the only layer with *legal* authority. Discovery, audit, regulator inquiries all bottom out here. Layer 2 and 3 are convenience.

#### Layer 2: FalkorDB projection (queryable)

- Built by the **deterministic projector** (V2-α) — reads `events.jsonl`, verifies signatures, extracts entities/edges via regex + structured-schema rules (NOT LLM-extraction by default), upserts idempotently.
- Every node and edge carries `{event_uuid, rekor_log_index, agent_did, projector_version}` as properties so any rendered fact is one click away from its signed source.
- **Trust property: "any node/edge in FalkorDB is reproducible from a deterministic projector run."** CI gate computes a state-hash over a canonical serialisation of the graph and fails if two projector runs over the same events diverge.
- **What if FalkorDB is corrupted, dropped, or compromised?** Re-run the projector, rebuild the graph. No trust loss. The projector itself becomes a Locked SemVer surface post-V2-α (any change to extraction rules requires `projector_version` bump + reseed CI gate).
- **What if the projector's extraction rules are wrong?** It produces a wrong graph, but still a deterministic wrong graph — and the wrongness is auditable because the source events are unchanged. Fix the rules, bump `projector_version`, replay.

#### Layer 3: Mem0g cache (fast retrieval)

- Mem0g = the graph-enhanced variant of Mem0 (Mem0Graph). Per Mem0's published benchmarks (LOCOMO benchmark, 2025): **91% p95 latency reduction vs full-context baseline, <5pt accuracy gap, 2.59s p95**, 26% better than OpenAI Memory.
- Atlas's design: Mem0g indexes Layer 2 (FalkorDB) — it consumes the projected graph, not raw events. This means Mem0g inherits Layer 2's provenance pointers automatically: every Mem0g result carries `{event_uuid, rekor_log_index, agent_did}` for the originating signed event.
- **Trust property: "any Mem0g result is rebuildable from Layer 2 by re-running the embedding/summarisation pipeline."** This is a weaker rebuildability than Layer 2 (embedding models drift; LLM summarisation is non-deterministic across model versions). To preserve it operationally, we pin `embedding_model_id` + `summariser_model_id` in Mem0g cache rows; rebuild is "deterministic given pinned model versions". Mem0g cache is therefore *eventually rebuildable* — not byte-identical across model upgrades.
- **What if Mem0g is wrong?** Worst case: agents get bad retrieval. They cite the result back to its `event_uuid`; auditor re-verifies via Layer 1; verifier says ✓ VALID on the original event (which was correctly signed) but the agent's interpretation was sloppy. Trust property survives: the **fact** is verifiable, the agent's **retrieval** is not trust-authoritative. Same situation as a human reading a wrong summary of a correct document.
- **What if Mem0g is malicious (compromised vendor)?** Mem0g returns false results. Detectable in two ways: (a) the `event_uuid` it cites doesn't actually exist in `events.jsonl` (auditor checks; verifier fails); (b) the `event_uuid` exists but Mem0g's summary contradicts the event's signed content (auditor compares). Either case = "evict Mem0g, fall back to FalkorDB", trust property unchanged.
- **What if Mem0g is wiped?** Rebuild from FalkorDB. Pure operational pain, no trust loss.

#### Bi-temporal mapping (per §2.2 open question, now resolved)

Atlas events carry four timestamps natively:
1. `event.timestamp` — when the agent claims the fact was observed/recorded.
2. `rekor_anchor.timestamp` — when the Sigstore Rekor inclusion was proven.
3. `witness_cosignature.timestamp` — when each federated witness countersigned.
4. `projector_run.timestamp` — when Layer 2 ingested the event.

Mapping to a bi-temporal `(t_valid, t_invalid, t'_created, t'_expired)` schema:
- `t_valid` = `event.timestamp` (the agent's claim of when the fact became true in the world).
- `t_invalid` = the `event.timestamp` of a later, signed retraction or supersession event that points to this event_id (if any). Computed at projector time. Lacking a retraction → `null` → "still valid".
- `t'_created` = the earlier of `rekor_anchor.timestamp` (if anchored) or `event.timestamp` (when we observe the fact's existence in the system-of-record).
- `t'_expired` = `t'_created` of the retraction event (if any).

This mapping is **derivable from Layer 1 + projector logic alone** — no Graphiti dependency needed. If Phase-2 critique surfaces a case Graphiti's model handles better, we can adopt Graphiti for the bi-temporal layer specifically while keeping deterministic projection for everything else.

---

### 2.6 Atlas as Hermes Agent Memory Skill

Hermes Agent (Nous Research, ~60K stars on GitHub by 2026-05, MIT license, model-agnostic, self-improving, currently #1 on OpenRouter per `.handoff/v2-session-handoff.md` §0) has a plugin/skill system. Atlas can be a first-class **Memory Skill** — the integration is high-leverage because every Hermes deployment becomes a free Atlas user without writing a line of code.

#### Integration surface

```
┌──────────────────────────┐   reasoning loop
│ Hermes Agent runtime     │  ─────────────────────────────┐
│  • skill registry        │                               │
│  • tool dispatcher       │                               │
└────────────┬─────────────┘                               │
             │ skill.invoke("atlas-memory.recall", args)   │
             ▼                                             │
┌──────────────────────────────────────┐                   │
│ Atlas Memory Skill (NEW V2)          │                   │
│  • TypeScript/Python skill package   │                   │
│  • bundled `atlas-mcp-server` config │                   │
│    OR direct HTTP-API client         │                   │
│  • holds the Hermes-instance DID     │                   │
│    keypair locally (skill-state dir) │                   │
└────────────┬─────────────────────────┘                   │
             │                                             │
             │ HTTP/MCP                                    │
             ▼                                             │
┌──────────────────────────────────────┐                   │
│  Atlas read-side API (§2.8)          │ ──────────────────┤
│  + Atlas write-side API (V1)         │                   │
└──────────────────────────────────────┘                   │
                                                           ▼
                                          fact returned to Hermes
                                          + Atlas provenance tuple
                                          (event_uuid, rekor_idx,
                                           authoring_agent_did)
```

#### Skill API surface (Hermes-side contract)

The skill exposes four primary calls to Hermes's reasoning loop:

- `atlas-memory.recall(query, options) → [Fact{content, event_uuid, agent_did, provenance_status, rekor_idx}]`
- `atlas-memory.remember(fact, options) → {event_uuid, rekor_idx, did_sig}`
- `atlas-memory.verify(event_uuid) → {valid: bool, evidence: [...]}` — delegated to the WASM verifier; returns the same shape as `VerifyOutcome`.
- `atlas-memory.passport(other_agent_did) → {writes_count, retractions_count, witness_cosigners, used_by_workspaces, first_seen, reputation_score}`

#### Skill-generated facts attribution

Every write the skill issues on Hermes's behalf carries:
- `agent_did = did:atlas:hermes-<pubkey_hash>` (the Hermes-instance's persistent DID — see §2.7 for full schema)
- `agent_kind = "hermes-agent"` (a free-form classifier — useful for filtering reputation queries by agent type)
- `agent_instance_metadata = {model: "Hermes-4-405B", host: "nous-cloud" | "self-hosted", skill_version: "0.x.y"}` — included as a non-signed `attributes` field so it doesn't lock Hermes into a particular model/host.

#### Trust property under skill compromise

**What if a malicious party publishes a fork of the Atlas Memory Skill to npm/PyPI?** The skill itself is just a thin client; nothing trust-load-bearing lives in skill code. The skill cannot forge events without the Hermes-instance's DID private key (stored in skill-state, isolated to the local instance). If the skill is replaced, the new skill cannot produce facts attributed to the original DID. A Hermes deployment operator who suspects skill compromise rotates the DID keypair (see §2.7 revocation chain), and any subsequent facts under the old DID are flagged as `revoked-key-window` by the verifier's passport endpoint.

#### Distribution-channel hypothesis (Nelson's §4-go-to-market lever)

GitHub Issue #477 in Nous Research's repo (per `.handoff/v2-session-handoff.md` §4) indicates Nous is open to skill contributions. Phase 2 product-critique should evaluate: do we (a) ship Atlas Memory Skill as a third-party package that Hermes users opt into, (b) propose upstream inclusion via PR, (c) both? Recommend (c): ship third-party first to iterate fast, then upstream once stable.

---

### 2.7 Agent Identities as W3C DIDs (`did:atlas:<pubkey-hash>`)

V1 already supports per-workspace HKDF-derived signing keys (`atlas-anchor:<workspace_id>`, prefix-pinned in `crates/atlas-trust-core/src/per_tenant.rs::PER_TENANT_KID_PREFIX`). V2 generalises this from per-workspace to **per-agent**, gives each agent a stable DID, and makes the DID's history queryable as an **agent passport**.

#### DID schema

```
did:atlas:<base32-truncated-blake3-of-pubkey>
```

- Method name `atlas` (W3C DID Core 1.0 compliant; method spec lives at `docs/specs/did-method-atlas.md` — drafted in V2-α).
- The method-specific identifier is the first 26 chars of `base32(blake3(ed25519_pubkey_bytes))` — same hash family Atlas already uses for event hashes, no new cryptographic primitive.
- Resolves to a DID document containing:
  - `id` — the DID itself
  - `verificationMethod` — current Ed25519 pubkey (multibase-encoded)
  - `service` — Atlas-passport-endpoint URL (`GET /api/atlas/passport/:did`)
  - `metadata.created` — `event.timestamp` of the first event ever signed by this DID
  - `metadata.revoked` — list of revoked keys (see revocation chain below) with `(revoked_at_event_uuid, replacement_did_or_null, reason)`

#### Agent passport (verifiable history)

A passport is the materialised view of "everything this agent has done" — computed by the projector from `events.jsonl` and exposed at `GET /api/atlas/passport/:did` (see §2.8). Fields:

- `writes_count` — total events authored by this DID
- `retractions_count` — events authored by this DID that have been retracted by a later signed retraction event
- `witness_cosigners` — set of witness keys that have cosigned >0 events from this DID
- `used_by_workspaces` — set of workspace_ids where this DID has authored
- `first_seen` / `last_seen` — `event.timestamp` of first/most-recent event
- `reputation_score` — a composite metric (see Phase 2 open question Q-RP-1) combining write volume, retraction ratio, witness-cosignature coverage, and tenure. Default formula stub: `log10(writes_count) * (1 - retractions_count/writes_count) * sqrt(witness_cosigners.size)`. **This is a derived projection — does not change Layer 1's trust authority.**

#### Revocation chain

A DID can revoke its keypair by signing a final event with the old key whose `kind = "did-revocation"` and whose payload contains the new DID's pubkey (or `null` for permanent retirement). The revocation event is hash-chained into the workspace's event log exactly like any other event; the projector folds it into the DID document.

- **What if the agent's private key is compromised before the operator notices?** Attacker can sign events under the DID until the revocation event is published. Existing trust property: attacker cannot forge `rekor_anchor.timestamp` (Sigstore is independent), so the window of attacker-controlled events is bounded by the anchor latency (typically minutes). Auditor query: `GET /api/atlas/audit/:event_uuid` returns the full provenance including anchor time → operator can scope the blast radius.
- **What if the operator can't rotate (lost key)?** The DID becomes "abandoned" — no further events can be signed under it. Past events remain trust-valid (signatures verify against the historical pubkey). This is the *same* failure mode as a lost Ed25519 git-signing key.

#### Cross-tenant identity vs per-workspace key

Per-workspace HKDF keys (`atlas-anchor:<workspace_id>`, V1.9) and per-agent DIDs (`did:atlas:...`, V2-α) are **complementary, not competing**. Per-workspace keys are the *workspace's* signing identity (used by `atlas-signer` + `atlas-witness`); per-agent DIDs are the *agent's* signing identity (used in the event's `author_did` field, distinct from the per-workspace anchor key). An event carries both:
- `kid` (V1) → which workspace key signed this for cross-workspace-replay defence
- `author_did` (V2) → which agent claims authorship

Verifier checks both: `kid` for the V1 invariants, `author_did` for the V2 passport-coherence check (does this event's `author_did` pubkey actually correspond to the embedded signature?).

---

### 2.8 Read-Side API

V1 has `POST /api/atlas/write-node` (V1.19 Welle 1). V2 introduces the read-side endpoints. All read endpoints carry the trust property by reference — they return Atlas data plus a provenance tuple per row that any caller can pass to the WASM verifier.

```
GET  /api/atlas/entities/:id
GET  /api/atlas/related/:id?depth=N&filter=…
GET  /api/atlas/timeline/:workspace?from=…&to=…&author_did=…
POST /api/atlas/query                            # Cypher subset, sandboxed
GET  /api/atlas/audit/:event_uuid
GET  /api/atlas/passport/:did
```

#### `GET /api/atlas/entities/:id`

Returns a single entity node from FalkorDB. Response includes `{id, type, properties, provenance: {event_uuid, rekor_log_index, author_did, projector_version}}`. The `id` here is the **stable entity identifier** chosen by the projector (canonical form: blake3 of the first-seen event's `entity_key` field) — not the FalkorDB internal node id (which is volatile across rebuilds).

#### `GET /api/atlas/related/:id?depth=N&filter=…`

Returns the N-hop ego-graph around an entity. `depth` capped at 5 server-side (Phase 2 should challenge: is 5 the right cap? Latency budget vs analyst expressiveness). `filter` is a structured DSL — supported filters: `author_did=…`, `min_witness_count=…`, `requires_anchor=true`, `time_window=[…]`, `entity_type=…`.

#### `GET /api/atlas/timeline/:workspace?from=…&to=…&author_did=…`

Returns chronologically-ordered events for a workspace. The native form is "scrub through the log for this workspace, optionally filtered by who wrote it." Pagination via `(?cursor=…)` cursor on `(event.timestamp, event_uuid)` lexicographic tuple. Each row carries the same provenance bundle for one-click verification.

#### `POST /api/atlas/query`

Sandboxed Cypher subset against FalkorDB. The body is `{cypher: "MATCH …", params: {…}, options: {timeout_ms, max_rows, require_anchored}}`. Server-side restrictions:
- No `CREATE / DELETE / MERGE / SET / REMOVE` — read-only
- No `CALL` to procedures except a whitelist (TBD Phase 2)
- Query timeout default 5s, hard cap 30s
- Result row count cap (default 1000, hard cap 10000)
- Optional `require_anchored: true` flag rewrites the query to only match nodes whose backing event is Rekor-anchored — a security-trivial filter that auditors will want by default.

Open question: do we want to expose Cypher directly, or a more opinionated GraphQL over the property graph? Phase 2 should challenge. Recommendation: ship both — Cypher for power-users, GraphQL-style typed envelope wrapping the same FalkorDB calls for normie agents.

#### `GET /api/atlas/audit/:event_uuid`

The "full provenance trail" endpoint. Returns:
- The signed event itself (raw JSON from `events.jsonl`)
- The hash-chain context (parent event hashes, descendant event hashes within N hops)
- The Rekor anchor entry (if anchored) — inclusion proof, signed checkpoint, log index
- All witness cosignatures (issuer key, signature, timestamp)
- The author DID's passport at the time of writing
- The projector run that ingested this event (run_id, projector_version, timestamp)

This is the single endpoint a regulator points their tools at. Designed to be one HTTP GET → one auditor-actionable response.

#### `GET /api/atlas/passport/:did`

Returns the materialised passport for an agent DID (per §2.7).

#### Trust property of the read-side API

**What if the API server is compromised and serves wrong data?** Every row carries `(event_uuid, ...)` provenance; the consumer can run the WASM verifier on the cited event to confirm the signature, anchor, and content. The API server can refuse to return data, or return out-of-date data, but cannot return data that doesn't trace back to a real signed event without being immediately detectable (the consumer's WASM verifier resolves the discrepancy). API server compromise degrades availability and freshness, not authenticity.

---

### 2.9 MCP V2 Tool Surface

V1 ships `atlas-mcp-server` exposing `write_node` + `verify_trace`. V2 extends to a query-and-passport surface so any MCP-1.29+ host can use Atlas as memory + identity layer.

| Tool | V1/V2 | Purpose |
|---|---|---|
| `write_node` | V1 | Append a signed event |
| `verify_trace` | V1 | Run WASM verifier on a trace |
| `query_graph` | **V2** | Cypher-subset query against FalkorDB (sandboxed, read-only) |
| `query_entities` | **V2** | Semantic search via Mem0g; returns entities ranked by similarity + provenance |
| `query_provenance` | **V2** | Given an entity_id or claim string, return the chain of signed events that support it |
| `get_agent_passport` | **V2** | Given a DID, return passport summary (writes, retractions, reputation, witnesses) |
| `get_timeline` | **V2** | Given a workspace + time range, return ordered events with provenance |

Each V2 tool's wire shape is a Locked SemVer surface from V2-α-ship onwards (see `docs/SEMVER-AUDIT-V1.0.md` §4 for the V1 MCP surface contract — V2 will add §4.x entries).

#### Trust property of MCP tools

MCP tools are **thin clients of the read-side API** (§2.8) — they call the HTTP endpoints over the local socket and return the same provenance-bundled rows. Compromise model identical to §2.8: the MCP server can refuse or stale, but cannot forge; the agent + WASM verifier remain the last word.

---

### 2.10 Federated Witness Cosignature for Regulators

Atlas V1.13 introduced witness cosignatures (`crates/atlas-witness`, threshold-strict via `VerifyOptions::require_witness_threshold`). V2 makes the federation roster **regulator-extensible** — a supervisor (BaFin, FCA, FINRA, FDA, regulator-of-choice) can be added as a witness whose signature MUST be present for high-stakes facts.

#### Architecture

```
Workspace's PubkeyBundle.witness_keys (V1.13):
  [
    {kid: "atlas-witness:internal-A", pubkey: …},
    {kid: "atlas-witness:internal-B", pubkey: …},
    {kid: "atlas-witness:regulator-bafin",  pubkey: …},   ← NEW V2 federation
    {kid: "atlas-witness:auditor-deloitte", pubkey: …}    ← NEW V2 federation
  ]

Workspace policy (V2 addition, NOT trust-load-bearing):
  required_witness_kids_for_event_kind = {
    "financial-recommendation": ["atlas-witness:regulator-bafin"],
    "patient-record-write":     ["atlas-witness:regulator-fda"],
    "default":                  []   // lenient
  }
```

#### Lifecycle

1. **Witness enrolment ceremony.** Workspace operator + regulator agree on a witness key. Regulator generates Ed25519 keypair on regulator-controlled HSM. Public key handed to operator (any non-secret channel, validated out-of-band per V1.18 SSH-Ed25519 trust-root-mutation discipline). Operator updates `PubkeyBundle.witness_keys` via the V1.18 protected-surface PR path — the bundle change is itself a trust-root mutation, signed by the operator's existing Atlas tag-signing key.
2. **Per-event cosignature flow.** When an agent writes a `kind=financial-recommendation` event, `atlas-signer` produces the COSE_Sign1 envelope as usual; `atlas-witness` (or equivalent service running at the regulator side) reads the event over the wire, performs whatever regulator-side policy check is appropriate, signs the event hash, returns the witness-sig. The signed event + witness-sig + agent-sig all land in `events.jsonl`.
3. **Verifier behaviour.** In default lenient mode, the witness cosignature is recorded but not strict-required. In strict mode (`--require-witness <N>`), the verifier rejects events missing the threshold count. V2 adds `--require-witness-kid <kid>` allowing the auditor to demand a *specific* witness kid be present (e.g. "this audit only accepts events cosigned by BaFin").
4. **Revocation.** Regulator rotates witness key → operator updates `PubkeyBundle` → past events under the old key remain valid (the historical public key is in the bundle's audit history); new events demand the new key.

#### Continuous-attestation operating model (Nelson's §4 trust-mode)

Per the strategic context, this is one of the *novel* trust-modes Atlas enables structurally: instead of periodic regulatory reporting, the regulator's key is **inside the trust root**. Every relevant event is cosigned in real-time. The regulator's audit becomes "scan for events lacking the cosignature" rather than "request a quarterly report and trust it". This works because:
- The regulator's signing window is independent of the operator (regulator controls their own HSM).
- The witness keys are public — anyone can verify that a specific BaFin key cosigned a specific event without trusting BaFin's word.
- The regulator can publish their witness key + revocation history alongside their public regulatory register, making the trust chain end-to-end auditable.

#### Trust property under regulator-witness federation

**What if a regulator's witness service goes offline?** Events authored during downtime cannot be cosigned in real-time → they land in `events.jsonl` with one fewer cosignature than policy demands. Two paths: (a) lenient — events accepted, regulator catches up by cosigning historical events post-hoc (the witness signature attests "I, regulator, attest this event existed at time T and I verified it at time T+Δ"); (b) strict — events rejected at write-time by the workspace's policy enforcement (V2 Cedar enforcement, see §3 open Q-CE-1). Both modes preserve V1's trust property because regulator-witness is additive — the agent's signature + workspace's anchor are unchanged.

**What if a regulator's witness key is compromised?** The regulator publishes a revocation event signed by their root key (out-of-band trust chain — same model as today's regulator certificate infrastructure). The workspace operator updates the PubkeyBundle; past events under the compromised key are flagged in the projector as `regulator-key-compromise-window`.

---

## 3. Open questions for iteration

These are what other agents (architect, security, product) should crit. Phase 1's v0 had 10; revised v0.5 expands and incorporates the new surfaces.

### 3.1 (existing) SSPLv1 exposure

Will Atlas ever expose graph query endpoints to customers? If yes, FalkorDB SSPL requires either commercial license OR open-sourcing the surrounding service stack. → Validate with FalkorDB sales **before** building on it. Alternatives: Kuzu (MIT), Neptune (managed-only), Neo4j Community (GPLv3 — different but also restrictive). Keep the projector's graph-store adapter abstract so swapping is configuration-time.

### 3.2 (existing) Projection determinism spec

What does "deterministically rebuildable" mean operationally? Same event log → same graph state across machines, across projector versions, across time. Needs an explicit `projector_version` const + idempotent upsert protocol + a `projector-state-hash` CI gate. Open: how do projector schema upgrades work (V1→V2 of the projector — graceful upgrade vs forced reseed)?

### 3.3 GDPR / Right-to-be-Forgotten — content vs hash separation (NEW)

**Problem.** Signed events are forever (V1.18 tag-immutability extends conceptually to event-immutability). EU GDPR Art. 17 grants data subjects the right to erasure. Direct conflict.

**Strategy.** Separate **signed metadata** from **deletable content**:

```
events.jsonl event:
  {
    event_uuid: …,
    event_hash: blake3(canonical_signing_input),
    parent_hashes: […],
    author_did: …,
    timestamp: …,
    content_hash: blake3(raw_content_bytes),     ← signed
    content_pointer: "blob://workspace/X/abc…",  ← signed pointer, not the bytes
    signature: cose_sign1(…)
  }

Separate (deletable) content store:
  blob://workspace/X/abc…  →  raw bytes (PII-bearing, deletable on request)
```

Trust property under deletion:
- Hash exists, anchor exists, signature exists → "this event existed at time T, this content_hash existed at time T, signed by author_did".
- Original content nullable = "redacted but verifiably existed".
- A redacted event still satisfies V1's chain-of-custody: the verifier checks `content_hash` is well-formed and matches what `content_pointer` resolves to (or "redacted" sentinel); it cannot read deleted content but can still confirm the event was legitimately signed and the content_hash was committed-to at time T.
- **What if the deletable content has been logged elsewhere (Sigstore Rekor)?** It hasn't — Sigstore only stores the event hash, not the content. Rekor's privacy model is content-blind by construction. Public-log inclusion is therefore not a GDPR risk.
- **What does an agent see when querying redacted content?** Mem0g serves "this fact was redacted on 2026-XX-XX, original hash was H, original event_uuid was U, original author was D" — preserves auditability without leaking content. Agents reasoning over redacted facts are explicit about the redaction (the agent gets a "this fact is redacted" signal it can include in its response).

Phase 2 open: is the projector required to *also* delete derived properties in Layer 2 + Layer 3 that may have leaked content into property values? Yes — Layer 2/3 are derivative and must be redactable. This means GDPR-erasure is a 3-step operation: (a) drop blob; (b) reseed Layer 2 from Layer 1 with new "redacted" sentinel; (c) reseed Layer 3 from Layer 2.

### 3.4 (existing) Bi-temporal mapping — resolved tentatively in §2.5

Atlas events already carry: (i) signing timestamp, (ii) Rekor anchor time, (iii) witness cosignature time, (iv) projector run time. §2.5 proposes a mapping `(t_valid, t_invalid, t'_created, t'_expired)` derivable from Layer 1 alone. Phase 2: challenge whether this is sufficient or whether Graphiti's full bi-temporal model captures something the Atlas-native mapping misses.

### 3.5 (existing) Reference-integration agent for the demo

Atlas is positioned agent-agnostic, but the landing-page demo needs *one specific* agent to show end-to-end. Per `.handoff/v2-session-handoff.md` §0, **Hermes Agent** is the strategic choice (#1 on OpenRouter, MIT, model-agnostic). Decision: Hermes Agent skill (§2.6) is the hero demo; Claude-via-MCP is the secondary "works with what you have today" demo; LangGraph recipe is the documentation example. Phase 2 product-critique should challenge whether Hermes's growth curve will hold or whether we need a "model-of-month" rotation strategy.

### 3.6 (existing) LLM extraction in projector — yes/no?

§2.2(c) tentative-recommendation argues "no in projector, yes on retrieval side". §2.5 reinforces this — Mem0g lives in Layer 3, Layer 2 stays deterministic. Phase 2: challenge whether deterministic regex/structured extraction is enough for the trust-load-bearing graph, or whether LLM extraction is load-bearing enough that we need the cache-keyed-by-event-hash mitigation (§2.2(a)).

### 3.7 (existing) Demo-first vs Production-first sequencing

V2 has two pulls: (a) credible end-to-end demo (Hermes-Skill + writes + graph + explorer) for atlas-trust.dev landing; (b) production-grade projection + verifier-integrated explorer. See `.handoff/v1.19-handoff.md` §16 + Welle-14b roadmap. Recommendation: option-2 (V2-α + V2-β rolled into Welle 14b-iii) for a 10x-stronger Demo-Ready surface.

### 3.8 (existing) GraphRAG SDK 1.0 vs Graphiti

Both extract entities, both build KGs for agents. FalkorDB's SDK is newer + vendor-aligned + #1 on GraphRAG-Bench. Graphiti is more mature in community / production use. Phase 2: head-to-head spike on (a) determinism story, (b) bi-temporal coverage, (c) Atlas-integration-burden before locking. **Note Mem0g eats most of the retrieval value either way (§2.5), so this question reduces to "do we need Graphiti's bi-temporal scaffolding?"**

### 3.9 (existing) Witness/anchor surfacing in the graph

Signed event becomes a graph node — Rekor anchor data + witness cosignatures live where? Embedded as properties (`event_node.rekor_idx`, `event_node.witness_cosignatures`)? Linked as separate `Anchor` + `Witness` nodes? Hidden behind a UI affordance until clicked? Tradeoff: visible-by-default = better trust narrative, but clutters the graph for non-trust queries. Recommendation: properties for the hot path, separate-node modelling only for cross-event-shared witnesses (a `Witness` node connected to N events it's cosigned would let auditors query "what fraction of last week's events has BaFin's cosignature?" in one Cypher hop).

### 3.10 (existing) Per-tenant key derivation in projection

V1 supports per-workspace HKDF-derived signing keys. V2 generalises with per-agent DIDs (§2.7). The graph DB needs to either (a) namespace by workspace (one FalkorDB graph per tenant), (b) tag every node with `workspace_id` and rely on query-side filtering, (c) use FalkorDB's multi-graph feature. Recommendation: (c) — FalkorDB native multi-graph isolation, with cross-workspace queries explicitly opt-in via a privileged endpoint. Phase 2 security-critique should validate the cross-workspace-replay defence still works under multi-graph.

### 3.11 (existing) Read-side API shape — partly resolved in §2.8

§2.8 proposes opinionated REST with a Cypher-passthrough escape hatch. Phase 2: challenge whether GraphQL is worth adding as a third surface (typed envelope wrapping the same calls). Recommendation: GraphQL only if Phase 2 product-critique surfaces a concrete consumer that needs it.

### 3.12 Mem0g vendor risk (NEW)

Atlas-+-Mem0g depends on Mem0 (venture-backed startup, founded 2024). Mem0g is open-source (Apache-2.0) at the time of writing — but the *hosted* Mem0 product is closed. If Atlas operationally depends on Mem0's hosted service, we inherit Mem0's vendor risk. **Mitigation:** Atlas integrates against Mem0g's OSS code in-process / self-hosted; we never depend on Mem0's hosted API in the trust path. If Mem0 the company disappears, Mem0g the OSS layer remains forkable, and worst case we replace it with our own embedding+graph-summarisation pipeline (Layer 3 is rebuildable from Layer 2 — see §2.5). Phase 2 risk-critique: validate this mitigation is realistic and not a Stockholm syndrome.

### 3.13 Cedar policy enforcement at write-time (NEW)

Per `.handoff/v2-session-handoff.md` §4 (Pre-action policy enforcement via Cedar), V2 could evaluate policies *before* signing — write rejected at the gateway if it violates policy. V1 already records `policies[]` as an event-id list but doesn't evaluate Cedar (per `docs/ARCHITECTURE.md` §11 "Not a policy engine yet"). Phase 2 architecture-critique: where in the V2 stack does Cedar live? Inside `atlas-web` write-handler? Inside `atlas-signer`? As a separate gateway in front of both? Recommend: a thin Cedar policy-evaluator crate, called from `atlas-web/route.ts` before `atlas-signer` is invoked; failed-policy events still optionally land in `events.jsonl` with a `kind=policy-rejected` envelope so the audit trail has the rejection too.

### 3.14 Privacy-vs-public-anchoring split (NEW)

Sigstore Rekor anchoring is public — anyone can scrape `rekor.sigstore.dev` and see Atlas anchor entries. Anchor entries contain only Atlas's `bundle_hash` / `dag_tip` hashes, not content (per §3.3, content is content-hash-only). But the *fact that workspace X is producing events at rate Y* is leaked by anchor frequency. For privacy-sensitive workspaces (legal discovery, M&A confidentialities, medical records), this might be unacceptable.

Mitigation: **private witness federation** as an alternative to Sigstore Rekor anchoring. Workspace operator runs an internal Rekor-shape log (the V1.5 mock-Rekor pattern, scaled up to multi-witness federation). Verifier accepts either Sigstore-public-Rekor OR private-Rekor-federation as long as the log's pubkey is in `default_trusted_logs()` (`crates/atlas-trust-core/src/anchor.rs::default_trusted_logs` BTreeMap, already extensible per V1.6 design). Trade-off: private logs sacrifice public-auditability for confidentiality.

Phase 2: validate the cryptographic strength of multi-witness private federation vs single-public-Sigstore-shard. Recommend: ship Sigstore-public as default + private-federation as opt-in flag — operator chooses on a per-workspace basis.

### 3.15 Post-quantum migration roadmap (NEW)

Per `.handoff/v2-session-handoff.md` §4-7, Ed25519 is quantum-vulnerable in the long term. V1's `Algorithm` enum (in `crates/atlas-trust-core/src/cose.rs`) is `#[non_exhaustive]` (per `docs/SEMVER-AUDIT-V1.0.md` §1.1 conventions), which means adding ML-DSA-65 (NIST FIPS 204, post-quantum signature standard) is SemVer-minor. The challenge isn't the signature primitive — it's the **migration path** for in-flight events. Phase 2 crypto-critique: design the dual-sign window (events signed with both Ed25519 and ML-DSA-65 for a transition period), the rotation ceremony, and the verifier's "accepts either, prefers both" mode. Recommend: not a V2-α-blocker, but design constraints for V2-α should not preclude post-quantum drop-in (e.g., the agent DID schema in §2.7 should be algorithm-agnostic — `did:atlas:` is keyed by hash-of-pubkey, not by algorithm).

### 3.16 Agent reputation gaming + Sybil resistance (NEW)

§2.7's reputation_score is a magnet for gaming. An attacker spins up 100 DIDs, has them write 1000 noise-events each, then claims a high reputation when joining a marketplace. Phase 2 security-critique: how does Atlas resist? Options:
- (a) Reputation requires witness-cosignature coverage from independent witness keys — Sybil writes won't accumulate witnesses they don't already control.
- (b) Reputation requires *cross-workspace* signal — a DID with 1000 events in 1 workspace counts less than a DID with 100 events across 10 workspaces, because workspaces are independently-administered trust domains.
- (c) Reputation explicitly *not* a global scalar; rendered as a structured object (writes, retractions, witness-cosigners, workspaces) and consumers compute their own scoring.

Recommend: (c) by default + (a)+(b) as standard scoring conventions the docs publish. Reputation is never *automatically trusted* — it's evidence the consumer interprets.

### 3.17 Projection schema versioning (NEW)

`projector_version` is the trust-load-bearing version stamp on every node/edge. But what happens when V2-α-projector → V2-β-projector changes the extraction logic? Options:
- (a) Hard reseed — drop FalkorDB, re-run from `events.jsonl` with the new projector. Operationally heavy but trust-clean.
- (b) Versioned co-existence — old V2-α nodes coexist with new V2-β nodes; queries optionally filter on `projector_version`. Operationally light, but auditors have to reason about which projector wrote which node.
- (c) Forward-compatible migration scripts — every projector-version bump ships a `migrate_v2alpha_to_v2beta()` that mutates FalkorDB in place. Risk: migration logic itself becomes load-bearing.

Recommend: (a) as the default — fast reseed (target <5min for 1M events) makes the operational cost tolerable, and trust-cleanliness is worth it. Phase 2: validate the 5-minute reseed target is achievable.

### 3.18 Composite Layer-1 representation — local file vs object store vs distributed log (NEW)

V1's `events.jsonl` is a local file. V2 may need to scale beyond a single host's filesystem. Options for Layer 1 at scale:
- (a) Per-workspace `events.jsonl` files in S3-compatible object storage. Concurrent-writer ⇒ requires a coordination layer (DynamoDB-Lock-Client pattern, or Atlas's existing per-workspace mutex elevated to a distributed lock).
- (b) An append-only log primitive (Kafka, Pulsar, Foundation Record Layer). Adds operational dependency.
- (c) A purpose-built distributed log built on top of Sigstore Rekor itself (anchor each event individually, log == anchor-chain). Maximalist; very high anchor frequency cost.

Recommend: (a) for V2-α — minimal new operational surface, matches the existing single-writer-per-workspace mutex model, S3 conditional-writes give us atomicity. Phase 2 architecture-critique: at what event throughput does (a) break and we need (b)?

### 3.19 (existing) Demo-feasibility lock-in

Phase 2 product+demo critique should challenge: which of the demos in `.handoff/v2-demo-sketches.md` (parallel Doc E in Phase 1) require V2-α/β/γ before they can be rendered? Don't ship demos that can't actually run.

---

## 4. Suggested decomposition for iteration (NOT a wave-plan yet)

If we go all-in, V2 work decomposes roughly into four sub-arcs reflecting the three-layer architecture:

### V2-α (Foundation Layer — Deterministic Projection, ~3-4 sessions)

- **Atlas Projector crate** (`crates/atlas-projector`): reads `events.jsonl`, verifies each event signature, extracts entities + relationships via deterministic regex / structured-schema rules, idempotent upsert into FalkorDB. Stamps `{event_uuid, rekor_log_index, author_did, projector_version}` onto every node and edge.
- **FalkorDB integration**: per-workspace logical graph, abstract storage adapter so Kuzu swap is configuration-time.
- **`projector-state-hash` CI gate**: deterministic canonicalisation of the FalkorDB graph state into a single hash. CI invariant: events.jsonl → projector(version N) → state-hash must match the previous reproducible run.
- **Agent DID schema** (§2.7): `did:atlas:<pubkey-hash>` method spec + first-pass DID document resolver.
- **`events.jsonl` content/hash separation** (§3.3): content-store abstraction, signed `content_hash` + nullable `content_pointer` field.

Acceptance: events.jsonl → graph round-trip CI gate green; state-hash reproducible across 3 machines; per-tenant isolation verified; SemVer audit Locked-surface entries for projector_version and graph schema.

### V2-β (Read Surface — API + Cache + UI, ~3-4 sessions)

- **Mem0g cache layer** wired against the FalkorDB projection (§2.5 Layer 3).
- **Read-side API** (§2.8): six endpoints (`entities`, `related`, `timeline`, `query`, `audit`, `passport`). REST + Cypher passthrough.
- **MCP V2 tool surface** (§2.9): four new tools wired through the read-side API.
- **Graph Explorer UI**: embed FalkorDB Browser at `/explorer`; build minimal Cytoscape.js demo at `/graph` for landing-page embed; "Verify this fact" button per node wired to WASM verifier.

Acceptance: explorer demos rendered + verified end-to-end; latency p95 ≤ 3s for graph queries under Mem0g; MCP V2 tools tested against Claude Desktop + Hermes Agent skill.

### V2-γ (Identity + Federation, ~2-3 sessions)

- **Agent passport materialisation** (§2.7): passport endpoint live; reputation_score scaffolding (structured object, no auto-trusted scalar — per §3.16).
- **Federated regulator witness** (§2.10): enrolment ceremony spec, `--require-witness-kid` verifier flag, witness-revocation event kind.
- **Hermes Agent Memory Skill** (§2.6): TypeScript + Python skill packages, distributed via npm + PyPI; reference integration recipe in `examples/integrations/hermes-agent/`.

Acceptance: Hermes-skill writes events with `did:atlas:hermes-*` attribution; passport endpoint shows verifiable history; mock regulator witness federation tested end-to-end on a demo workspace.

### V2-δ (Optional — Graphiti bi-temporal layer + Cedar policy + Reference Integrations, ~2-3 sessions)

- **Graphiti integration** (if §2.2(c)+§2.5 mapping survives Phase 2 critique): bi-temporal edges, narrowly scoped to time-sensitive entity types.
- **Cedar policy enforcement** at write-time (§3.13): policy crate, Atlas-web write-handler integration.
- **LangGraph / LangChain reference recipe**: `examples/integrations/langgraph/` showing Atlas HTTP API as a tool node.
- **OpenAI Memory / Anthropic Memory bridge** (stretch): shim layer for vendor-native memory APIs to forward writes into Atlas — turns vendor-silo memory into Atlas-attested memory.

Acceptance: at least three demo integrations runnable in <10 LOC of "atlas glue" beyond standard framework setup.

**Total budget: ~10-14 sessions.** Order is α → β-parallel-with-γ → δ-optional.

---

## 5. Where this fits with Welle 14b/14c

The current handoff doc (`.handoff/v1.19-handoff.md` §16) has Welle 14b as: (i) Trusted Publishers OIDC, (ii) dual-publish architecture fix, (iii) Demo-Ready surfaces (atlas-trust.dev landing + Quickstart + integrations), plus Welle 14c visual polish. This V2 vision **competes with Welle 14b-iii for the "what's the demo story" slot**. Options:

- **Defer V2 entirely** until 14b-{i,ii,iii} ships → atlas-trust.dev landing without graph layer (less compelling demo, but ships in 2-3 weeks).
- **Roll V2-α + V2-β into Welle 14b-iii** → landing page launches WITH a working graph explorer + Hermes-skill demo (delays 14b-iii by ~6 sessions but the demo lands 10x stronger).
- **Parallel tracks** → Welle 14b-{i,ii} (security/dist) ships fast; V2 work runs in parallel; 14b-iii landing waits until V2-α+β are demo-able.

Recommended: **option 2** if the goal is "credible Demo-Ready surface aligned with the agent-substrate positioning", **option 3** if the goal is "ship landing page fast + improve over time".

---

## 6. Iteration call-to-action

This doc is v0.5 (revised from v0 with three-layer architecture, Hermes skill, agent DIDs, read-side API, regulator-witness federation, and GDPR/content separation). Pass it to Phase 2 critique-agents:

- **Architect agent**: critique §2 blueprint, particularly the three-layer trust invariant chain (§2.5), the DID design (§2.7), and the read-side API shape (§2.8). Especially: are there other layers (federation? caching?) that creep into trust-load-bearing territory without us noticing?
- **Security agent**: critique §2.1 (trust invariant), §2.10 (regulator-witness federation), §3.3 (GDPR), §3.14 (privacy-vs-public-anchoring), §3.16 (Sybil), §3.15 (post-quantum), §3.18 (Layer-1 distributed storage). Especially: where does the trust property *almost* break and where do we have a near-miss?
- **Product agent**: critique §1 (user journeys), §3.5 (reference-agent), §3.7 (sequencing), §5 (Welle 14b alignment). Especially: is the two-market story (Second Brain + Multi-Agent) coherent enough for a single positioning, or does V2 need to bet harder on one market?
- **Database agent**: critique §2.3 (FalkorDB choice), §2.5 (Mem0g operational model), §3.1 (SSPL), §3.8 (GraphRAG SDK vs Graphiti), §3.17 (schema versioning), §3.18 (distributed Layer 1). Especially: what's the failure mode at 100M events?
- **Compliance / Legal agent**: critique §2.10 (regulator-witness lifecycle), §3.3 (GDPR), §3.14 (private witness federation as confidentiality control). Especially: do the EU AI Act Art. 12/13/14/18 requirements map cleanly to V2 endpoints, or do we need additional logging surfaces?

Each crit should produce specific +/-1 sentence challenges or refinements per numbered open question. Don't rewrite the doc; surgically annotate.

---

## 7. Open Questions for Phase 2 Critique (consolidated, ≥10 explicit questions)

The Phase 2 critique-agents should specifically challenge these. Format mirrors `.handoff/v2-session-handoff.md` §6.1 convention.

- **Q-TI-1 (Architect/Security):** Does the three-layer trust hierarchy hold when Layer 1 is scaled to distributed storage (§3.18)? Specifically, if `events.jsonl` becomes an S3-backed log with eventual consistency, does the "deterministically rebuildable Layer 2" claim survive a partitioned-write scenario? Status: open.
- **Q-PR-1 (Architect):** Is the `projector_version` reseed model (§3.17 option a) tolerable at 100M events, or do we need versioned co-existence (§3.17 option b)? Concretely, what's the target reseed time per million events? Status: open.
- **Q-DID-1 (Security):** The `did:atlas:` method uses blake3 (32-byte hash family). If blake3 is found weak (no current indication, but post-quantum analysis is incomplete), can the DID method be upgraded without invalidating existing passports? Status: open.
- **Q-DID-2 (Security):** Revocation chain (§2.7) requires the *compromised* private key to sign the revocation event. What's the recovery path if the operator has *no* access to the private key (lost not stolen)? Status: open.
- **Q-CE-1 (Architect):** Where does Cedar policy enforcement (§3.13) live — atlas-web handler, atlas-signer, separate gateway, or sidecar? Status: open. Recommendation in §3.13 is "atlas-web handler" but Phase 2 should challenge.
- **Q-RW-1 (Compliance):** For the federated regulator witness (§2.10), what's the legal contract pattern? Does the regulator sign a written attestation that *their* witness service binds them, or is the witness key alone the binding artifact? Status: open. Affects whether continuous-attestation is genuinely lower-burden vs current periodic reporting.
- **Q-RW-2 (Security):** What happens if a regulator's witness service is compelled (subpoena) to cosign a *false* event? Distinct from compromise — coerced cosignature is policy-valid but morally invalid. Does Atlas have any defence? Likely no — this is a regulator-trust assumption. Status: open, may be unavoidable.
- **Q-MEM-1 (Vendor):** Mem0g's hosted offering vs OSS layer split — does Atlas's operational model genuinely never depend on the hosted service (§3.12)? Specifically, is the Mem0g OSS code feature-complete enough to self-host in production, or is the hosted version a strict superset? Status: open, requires Mem0-team validation.
- **Q-GDPR-1 (Legal):** Does the §3.3 content-hash separation actually satisfy GDPR Art. 17, or do EU regulators consider the *existence* of an irrevocable hash + signature about a person to be itself personal data? Status: open, requires GDPR-counsel validation. If the answer is "the hash counts as personal data", Atlas may need a "redact-event-entirely" mode for high-PII workspaces — which breaks the chain.
- **Q-API-1 (Architect):** Should `POST /api/atlas/query` (§2.8) accept raw Cypher, GraphQL, or both? Tradeoff: power-user expressiveness vs API surface area + sandboxing burden. Status: open.
- **Q-PASS-1 (Product/Security):** Should `GET /api/atlas/passport/:did` return a single composite reputation_score, or strictly the structured-object form per §3.16(c)? Status: open. Recommend (c) — but Phase 2 should challenge whether marketplaces will *actually* accept that and not demand a single number.
- **Q-MCP-1 (Product):** Are the four new MCP V2 tools (§2.9) the right granularity, or should they be merged into a single `atlas_memory` mega-tool with a `mode` argument? Tradeoff: discoverability (separate tools = clearer model) vs token-budget (one tool description vs four). Status: open.
- **Q-EXT-1 (Architect):** §2.2(c) commits to deterministic regex/structured extraction in the projector. What's the spec for an entity-extraction rule? Concretely: a YAML/JSON DSL? A Rust trait? An external WASM plugin per workspace? Status: open. Affects how operators extend Atlas to their domain.
- **Q-PQ-1 (Crypto):** Post-quantum migration (§3.15) — when do we ship ML-DSA-65 support? Pre-emptively (cost: code complexity; benefit: future-proof) vs reactive (cost: late migration; benefit: don't pay for what we don't need yet)? Status: open.
- **Q-DEMO-1 (Product):** §3.5 commits to Hermes Agent as the reference demo. What's the contingency if Hermes Agent's star growth flattens or it's supplanted by a fast-follower in late 2026? Status: open. Recommended hedge: ship Hermes-skill + LangGraph-recipe + Claude-MCP in V2-γ so we have three viable hero-demos to rotate between.

---

## 8. References to Atlas crates / files (for Phase 2 spot-checks)

| Concept | Lives in |
|---|---|
| Event wire-format spec | `crates/atlas-trust-core/src/trace_format.rs` (`AtlasTrace`, `AtlasEvent`, `EventSignature`) |
| Hash chain + strict-chain check | `crates/atlas-trust-core/src/hashchain.rs` (`check_strict_chain`) |
| Per-tenant HKDF kid prefix | `crates/atlas-trust-core/src/per_tenant.rs::PER_TENANT_KID_PREFIX` (issuer-side mirror: `apps/atlas-mcp-server/src/lib/keys.ts`) |
| Pinned trusted logs (Sigstore + future private federation) | `crates/atlas-trust-core/src/anchor.rs::default_trusted_logs` |
| Witness signature surface | `crates/atlas-trust-core/src/witness.rs` (`WitnessSig`, `witness_signing_input`, `WitnessFailureWire`) + `crates/atlas-witness/src/*` |
| Verify entry points | `crates/atlas-trust-core/src/verify.rs` (`verify_trace`, `verify_trace_with`, `VerifyOptions`, `VerifyOutcome`, `VerifyEvidence`) |
| Algorithm enum (post-quantum extension point) | `crates/atlas-trust-core/src/cose.rs::Algorithm` (`#[non_exhaustive]`) |
| WASM verifier (browser + Mem0g cited-event verification) | `crates/atlas-verify-wasm/` (`verify_trace_json`) |
| CLI flag set (verifier) | `crates/atlas-verify-cli/src/main.rs` |
| Write-surface HTTP handler (V1 entry, V2 will extend) | `apps/atlas-web/src/app/api/atlas/write-node/route.ts` |
| MCP server (V2 will add query tools) | `apps/atlas-mcp-server/src/index.ts` |
| Bridge (path redaction, ulid, signer-cache) | `packages/atlas-bridge/src/` |
| V1.0 SemVer contract | `docs/SEMVER-AUDIT-V1.0.md` |
| Operational boundaries / V2 scope | `docs/ARCHITECTURE.md` §10 (V2 boundaries) + §11 (operational limits) |

---

**Doc owner:** Nelson + Atlas team. **Last edited:** 2026-05-12 (v0.5, Phase 1 Doc B revision). **Next milestone:** Phase 2 critique pass complete by ~2026-05-15, then converted-surviving-recommendations folded into a Welle 14b-iii (or V2-α-spec) plan-doc.
