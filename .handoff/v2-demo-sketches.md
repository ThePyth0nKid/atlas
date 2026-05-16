# Atlas V2 — Demo Sketches v0

> **Status:** Draft v0 (2026-05-15). Phase 1 Foundation Doc E for the V2 iteration framework. Five 30–90 s demo storyboards spanning Atlas's near-term landing-page surface (atlas-trust.dev) and live-pitch scenarios for investors / compliance buyers / AI-engineering audiences. Each storyboard is REALITY-CHECKED against the actual V1.0.1 + V2-α + V2-β-Phase-13.5 shipment state — every demo carries an explicit readiness flag so Phase 2 critique agents can challenge over-promising. Not a wave-plan; not yet a production brief.

---

## 0. One-paragraph intent

Atlas's trust property is structurally novel but visually invisible — signatures are bytes, anchors are log indices, witness cosignatures are background-process artefacts. Demos exist to make the invisible visible in 30–90 seconds for audiences that DO NOT have time to read SLSA-L3 docs or the v1.19 handoff. Each demo below answers: *what does the viewer see on screen at each beat, what's the emotional payoff, and is this demo-able TODAY or does it depend on V2-α / V2-β / V2-γ / W18c shipping first?* The doc deliberately separates "demo concept" from "demo readiness" — concepts can be specced even when the substrate isn't ready, so Phase 2 critique can challenge the concepts independent of the engineering blockers.

---

## 1. Methodology

### 1.1 Five-block storyboard schema

Every demo below follows the same 5-block structure:

1. **Setup (5–10s):** What the viewer sees the moment the demo loads. Establishes the scene — the panes, the personas, the empty/initial state. No motion yet.
2. **Action (15–40s):** What HAPPENS — agent writes, graph populates, witness cosigns, query runs. This is the bulk of the demo and must be *visually narrative* (not "agent writes fact" but "left-pane: chat interface emits text; right-pane: fact node materialises in graph viz with a coloured passport ring").
3. **Reveal (10–20s):** The verification moment. The viewer clicks / hovers / inspects → a modal slides in showing cryptographic proof, signatures, anchor proofs, witness cosignatures. This is the "wait, what?" beat.
4. **Implication (10–20s):** One-sentence caption appears overlaying the scene, explaining *why this matters*. Reading speed ~4 words/s — keep to ≤20 words.
5. **CTA (5s):** Where the viewer goes next. Single button or two-line text overlay. No more than one verb.

### 1.2 Per-demo metadata

Each demo carries:
- **Target audience** — exactly who should feel something
- **Target emotion** — surprise / trust / power / clarity (sometimes 2 of 4, never 3+)
- **Technical assets needed** — what infrastructure (V1 / V2-α / V2-β / V2-γ / W18c) must exist for this demo to be REAL (not mocked)
- **Production complexity** — 1 (trivial) to 5 (consumer product) scale
- **Readiness flag** — `DEMO-ABLE TODAY` / `DEMO-ABLE WITH MINOR UI WORK` / `REQUIRES V2-β EXPLORER` / `REQUIRES V2-β READ-API` / `REQUIRES W18c` / `REQUIRES V2-γ CONSUMER PRODUCT`

### 1.3 Visual conventions across all demos

Consistent visual language helps the viewer transfer trust between demos when watching multiple:
- **Agent passport rings:** every fact node carries a coloured ring matching the writing agent's pubkey hash. Stable across all demos.
- **Provenance badges:** green checkmark = signed; blue chain link = Rekor-anchored; orange shield = witness-cosigned; purple star = regulator-cosigned.
- **Time indicators:** all timestamps render as relative ("2s ago" / "T+47ms") on screen; ISO-8601 only in modal-reveal.
- **Failure states:** never green-by-default. Always show one "tampering detected → red banner" moment in any demo that has time, even Demo 5 — it's the negative-space proof that the green checkmarks mean something.

---

## 2. Demo 1 — Multi-Agent Race (Verifiable Attribution)

**Concept:** Two agents writing into the same Atlas workspace simultaneously; viewer sees provenance per agent in real time.

### 2.1 Storyboard

**Setup (8s).** Three-pane layout:
- **Top-left pane:** Claude Desktop chat window. Prompt visible at top: *"Research Bank XYZ Q1 2026 earnings. Write key facts to Atlas."* Claude's avatar coloured **blue**.
- **Top-right pane:** Custom agent UI (LangGraph React loop or Cursor MCP). Same prompt. Agent avatar coloured **orange**.
- **Bottom pane (full-width):** Atlas Graph Explorer (`atlas-web /graph`) showing the workspace "bank-research-q1-2026". Currently EMPTY — just the workspace name + a 0-node graph canvas.

**Action (35s).** Both agents start streaming. Every ~3 seconds a new fact materialises:
- T+3s: Claude writes "Revenue: $4.2B" → blue-ringed node animates from top-left into graph canvas with a soft pop. Edge connects to entity node "Bank XYZ".
- T+5s: Orange agent writes "EPS: $1.23" → orange-ringed node materialises. Edge connects to same "Bank XYZ" entity.
- T+8s: Claude writes "Operating margin: 22.4%" → blue node.
- T+11s: Orange agent writes "Cost-to-income ratio: 54.8%" → orange node.
- T+15s: Claude writes "ESG score (MSCI): A" → blue.
- T+18s: Orange agent writes "FDIC deposit insurance status: insured" → orange.
- ...continues for 30s total, ~10-12 facts, alternating colours, growing graph in real time.
- **Small heatmap counter** at bottom: "Claude facts: 6 | Custom agent: 5 | Conflicts: 0".

**Reveal (15s).** Viewer's mouse hovers over the **first blue fact** (Revenue: $4.2B). Tooltip shows: `Signed by: did:atlas:claude-instance-A · 2s ago`. Viewer clicks. Modal slides in from right:

```
┌─────────────────────────────────────────────────────────┐
│  Fact: "Revenue: $4.2B"                                 │
│  ──────────────────────────────────────────────────────  │
│  Signed by:    did:atlas:claude-instance-A-2f8a91…      │
│  Signed at:    2026-05-15T11:42:17.014Z                 │
│  Event hash:   sha256:c4f9…b21d                          │
│  Parent hash:  sha256:7a32…ef89  (verified ✓)            │
│  Rekor anchor: logIndex 187,234,521  (verified ✓)        │
│  Witnesses:    2/2 cosigned ✓                            │
│                 • witness-internal-1  (T+47ms)           │
│                 • witness-external-cosi  (T+412ms)       │
│  ──────────────────────────────────────────────────────  │
│  [ Verify in browser (WASM) ]   [ View full trace ]      │
└─────────────────────────────────────────────────────────┘
```

Clicking "Verify in browser" launches the WASM verifier in an iframe → spinner → "✓ VALID — all checks passed".

**Implication (15s).** Caption fades in over the graph:

> **"Every fact has a verified author. No more 'the AI said it' — cryptographic attribution per agent."**

Sub-caption smaller: *Two agents wrote into the same workspace. Atlas signed each fact with the WRITER's key, not the workspace's key. Reputation is portable.*

**CTA (5s).** Button: **"Try the multi-agent quickstart →"**  Sub-text: *npm install @atlas-trust/verify-wasm*.

### 2.2 Metadata

- **Target audience:** AI engineers building multi-agent / multi-vendor systems; AI-safety researchers; LangGraph/CrewAI/AutoGen power-users.
- **Target emotion:** trust + power.
- **Technical assets needed:**
  - `atlas-mcp-server` (V1.19 — LIVE) for Claude Desktop write path
  - HTTP API `POST /api/atlas/write-node` (V1.19 — LIVE) for custom agent
  - Per-agent Ed25519 keypair (V1 per-tenant HKDF generalises — **partially V1, full per-agent-passport is V2-β scope**)
  - **Atlas Graph Explorer at `/graph`** (V2-β scope — DOES NOT EXIST YET; spec'd in `v2-vision-knowledge-graph-layer.md` §2.4)
  - **Click-to-verify modal with WASM iframe** (V2-β scope — verifier exists, modal-glue is new)
  - Sigstore Rekor anchoring (V1 — LIVE)
  - Witness cosignature path (V1 — LIVE)
- **Production complexity:** 4/5 — graph explorer + per-agent-key extension are the load-bearing new builds; substrate is V1.
- **Readiness:** **REQUIRES V2-β EXPLORER** + per-agent-key surfacing. Write/sign/anchor/verify all work TODAY; the graph viz that makes "agents are coloured rings" visible is the gap. Not demo-able TODAY without significant mocking. **Earliest real demo:** post-V2-β-Phase-{explorer-ship} (~2-3 sessions of UI work over V1 substrate).

---

## 3. Demo 2 — Continuous Audit Mode (Regulator Witness)

**Concept:** A regulator's witness key is federated into the trust root. Every agent decision is cosigned by the regulator in real time. Compliance becomes structural.

### 3.1 Storyboard

**Setup (10s).** Single-window atlas-web dashboard, "compliance mode" theme (dark blue accent, conservative typography). Top banner:

```
┌──────────────────────────────────────────────────────────────┐
│  Workspace:   ultranova-finance-prod                          │
│  Trust root:  3 keys                                          │
│     • internal-witness     (your-org-key)         ✓ healthy   │
│     • external-cosi        (cosi-witness-roster)  ✓ healthy   │
│     • bafin-regulator      (BaFin pubkey 7f3a…)   ✓ healthy   │
│  Last regulator-key health check: 2 min ago                   │
└──────────────────────────────────────────────────────────────┘
```

Below: agent chat pane (left), live event timeline (right, currently empty).

User-prompt in chat: *"Recommend AAA-bond allocation for client portfolio Müller-Industriebeteiligungen GmbH (risk profile: conservative, EUR 2.4M)."*

**Action (40s).** User clicks "Run". Agent streams a recommendation: *"Recommend 40% German Bunds (10y), 30% French OATs (5–7y), 20% EU corporate AAA, 10% cash. Rationale: conservative risk profile, EUR-currency exposure, ECB rate stability through 2027…"*

As the agent writes, the right-pane timeline animates each provenance event:

- **T+0ms** — `agent-signature` (blue dot): "Signed by: did:atlas:financial-agent-v3"
- **T+47ms** — `internal-witness` (green dot): "Cosigned by: witness-internal-1"
- **T+89ms** — `rekor-anchor` (chain icon): "Anchored: Sigstore logIndex 187,234,832"
- **T+412ms** — `bafin-regulator-witness` (**purple star, pulses, larger than others**): "Cosigned by: BaFin-supervisor-key 7f3a91…"
- **T+501ms** — `final-state` (gold checkmark): "Audit-Anchored ✓ — recommendation visible to regulator"

Each event slides into the timeline with a soft sound (optional in live demo).

**Reveal (20s).** Click on the **purple star** (BaFin cosignature). Modal:

```
┌────────────────────────────────────────────────────────────────┐
│  Regulator Cosignature                                          │
│  ───────────────────────────────────────────────────────────    │
│  Witness:        BaFin (Bundesanstalt für                       │
│                  Finanzdienstleistungsaufsicht)                 │
│  Pubkey:         7f3a91b2…ce8d  (federated 2026-05-01)          │
│  Cosigned at:    2026-05-15T11:42:17.412Z                       │
│  Cosignature:    ed25519:a47e…b9f1                              │
│  Anchor proof:   Sigstore Rekor logIndex 187,234,832            │
│  ───────────────────────────────────────────────────────────    │
│  ⚠  This cosignature was issued LIVE during recommendation     │
│     generation. No batch/end-of-day reporting was required.     │
│                                                                  │
│  [ Download cosignature ]  [ Verify against BaFin trust root ]  │
└────────────────────────────────────────────────────────────────┘
```

Beneath the modal a sub-caption ticks up: *"Regulator cosignatures this week: 14,371 / 14,371 (100% coverage)."*

**Implication (20s).** Caption:

> **"Compliance is structural, not periodic. The regulator's key is IN the system."**

Sub-caption: *Witness cosignatures land in <500ms. No quarterly reporting. No selective sampling. The supervisor sees every decision as it happens — and can revoke their key at any time.*

**CTA (5s).** Button: **"Book a compliance briefing →"**  Sub-text: *EU AI Act Art. 12 + DORA Art. 11 mapping included.*

### 3.2 Metadata

- **Target audience:** Compliance officers, regulators, financial-services CROs/CISOs, regtech buyers, EU-AI-Act-stressed enterprise procurement. (Secondary: insurance underwriters — Demo 2's regulator-witness pattern generalises to the AI-Liability-Insurance pricing thesis from `v2-session-handoff.md` §4 "Trust-Modes 4b".)
- **Target emotion:** trust + clarity.
- **Technical assets needed:**
  - `atlas-witness` federation with multi-key trust root (V1 — LIVE)
  - Witness roster configuration with named external witnesses (V1 — LIVE, `cosi-witness-roster` pattern exists)
  - Sub-500ms witness cosignature path (V1 — LIVE)
  - `atlas-web` UI surface for "live witness timeline" component (**NEW — must build over existing verify-trace API**)
  - "Regulator-witness simulator" for the demo recording — a real witness service running with a fake-BaFin key (**SIMULATABLE TODAY** — atlas-witness is process-isolated, can run with any key configured as a roster entry)
- **Production complexity:** 3/5 — substrate is V1; UI is the timeline widget + modal. ~1 session of focused UI work.
- **Readiness:** **DEMO-ABLE WITH MINOR UI WORK (~1 session).** This is the cheapest demo to ship as REAL (not mocked) because every cryptographic primitive is V1-shipped. The demo's "regulator key" is a real Ed25519 keypair in a real federation roster; the only fiction is that the key happens to be labelled "BaFin" in the demo. **Strongest near-term demo candidate.**

---

## 4. Demo 3 — Agent Passport (Reputation Portability)

**Concept:** An agent's signing key is a portable identity. Over time, the agent accrues a verifiable track record. Hiring an agent = importing its passport.

### 4.1 Storyboard

**Setup (10s).** atlas-web agent detail page. Header strip:

```
┌────────────────────────────────────────────────────────────────┐
│  🤖  hermes-instance-3  ·  research-bot v2.4                    │
│  did:atlas:hermes-instance-3-2f8a91…                            │
│  Active since 2026-04-15  (30 days)                             │
│  Workspaces: ultranova-research · acme-due-diligence · 1 more   │
└────────────────────────────────────────────────────────────────┘
```

Below: empty scorecard grid (4 tiles), greyed out.

**Action (30s).** Scorecard tiles populate one by one with subtle counter animations:

- **Tile 1 — "Facts written"** counter rolls from 0 → **1,247**. Tiny sparkline below shows daily write-volume (steady ~40/day).
- **Tile 2 — "Retractions"** counter shows **0** in green. Below: *"Zero facts retracted across 30 days."*
- **Tile 3 — "Unique witness cosigners"** shows **12**. Small avatar cluster below — 12 witness-key thumbnails.
- **Tile 4 — "Used by organisations"** shows **3**. Org logos animate in: Ultranova, Acme-Capital, Müller-Industriebeteiligungen.

Below the tiles, a "Skill domains" section auto-derives from fact topics:
- **financial-research** (487 facts) — 39%
- **macro-economics** (312 facts) — 25%
- **ESG-reporting** (198 facts) — 16%
- **client-due-diligence** (162 facts) — 13%
- **other** (88 facts) — 7%

Below that, a horizontal mini-bar-chart titled *"Daily Rekor-anchor inclusion (last 30 days)"* — 30 green bars at 100% height each.

**Reveal (20s).** Bottom-right button: **"Export Passport"**. Click → modal:

```
┌────────────────────────────────────────────────────────────────┐
│  Download Agent Passport                                        │
│  ───────────────────────────────────────────────────────────    │
│  File:        hermes-instance-3.passport.signed.json            │
│  Size:        14.7 KB                                           │
│  Contents:                                                       │
│     • Public key + DID                                          │
│     • 30-day write-history hash chain (1,247 entries)           │
│     • All Rekor anchor proofs                                   │
│     • Witness-cosigner roster (12 keys)                         │
│     • Skill-domain attestations (auto-derived + counter-signed) │
│     • Atlas trust-root snapshot at export time                  │
│  ───────────────────────────────────────────────────────────    │
│  This passport is verifiable at ANY Atlas instance without      │
│  contacting the origin Atlas server. No central authority.      │
│                                                                  │
│  [ Download .json ]   [ View verification command (CLI) ]       │
└────────────────────────────────────────────────────────────────┘
```

Below the modal: a small terminal-style preview:

```
$ atlas-verify-cli verify-passport hermes-instance-3.passport.signed.json \
    -k atlas-trust-root.pubkey-bundle.json
✓ VALID — passport verified against pinned trust root
✓ 1,247 events validated, all signatures verified
✓ 30/30 daily Rekor anchors verified
✓ 12/12 witness cosigners cross-validated
✓ 0 retractions detected in history
```

**Implication (20s).** Caption:

> **"Agents have CVs now. Cryptographic ones."**

Sub-caption: *Hire an agent → it brings its verifiable track record. Fire it → revoke its key. No central registry. No vendor lock-in.*

**CTA (5s).** Button: **"Browse the agent registry →"** Sub-text: *atlas-trust.dev/passports*.

### 4.2 Metadata

- **Target audience:** Multi-tenant AI deployers (think: an enterprise procuring research agents from multiple vendors); AI marketplaces (Hugging Face, Together, Anthropic, OpenAI agent stores); AI procurement teams at large orgs; the open-weight community (Hermes / Llama / Mistral fine-tuners who want differentiation against closed-vendor offerings).
- **Target emotion:** clarity + power.
- **Technical assets needed:**
  - **Per-agent Ed25519 keypair** (V1 per-tenant HKDF generalises — **agent-as-DID layer is V2-β scope per `v2-vision-knowledge-graph-layer.md` §2.7 — DOES NOT EXIST YET**)
  - **Aggregation queries** over signed events ("count facts by agent", "list distinct cosigners", "skill-domain auto-derivation") (REQUIRES V2-β read-API + ArcadeDB queries — ArcadeDB Layer 2 is W17c-shipped, but the aggregation queries are NEW)
  - **Retraction-tracking schema** (NOT YET DESIGNED — needs ADR; what does "retract" mean cryptographically? Soft-delete with anti-event? Cosigned correction?)
  - **Passport export format** (NEW — needs spec; what's IN the passport JSON, what's the signature envelope, what's the verification CLI command shape)
  - **`atlas-verify-cli verify-passport`** subcommand (NEW — extends the existing `verify-trace` model)
  - Cross-Atlas-instance trust root verification (V1 trust root model — LIVE, but cross-instance verification UX is new)
- **Production complexity:** 4/5 — the passport schema + verify-passport CLI + agent-DID layer are non-trivial design + implementation work; the UI is the easy part.
- **Readiness:** **REQUIRES V2-β READ-API + AGENT-IDENTITY LAYER.** Substrate-fit is good (every V1 event is already signed with a key that could be a per-agent key); the missing pieces are (a) agent-identity-as-DID surfacing, (b) aggregation read-API, (c) passport schema. ~3-4 sessions post-W19. **Not demo-able TODAY.**

---

## 5. Demo 4 — Verifiable Second Brain (Obsidian Comparison)

**Concept:** Side-by-side comparison: Obsidian vault vs. Atlas Second Brain. Same notes, but Atlas signs everything and detects tampering. Speaks to the "AI-era trust" thesis for human knowledge workers.

### 5.1 Storyboard

**Setup (10s).** Two-pane horizontal split:
- **Left:** Obsidian.md with vault "personal-research" loaded. Familiar Obsidian UI (graph view in sidebar, markdown editor centre).
- **Right:** Atlas Second Brain with same vault mirrored. Atlas UI is similar in structure (graph + editor) but with a green **"Signed ✓"** indicator next to each note title.
- **Top strip:** device selector with 3 icons — `[Laptop A]` (highlighted) `[Phone]` `[Laptop B]`.

User on Laptop A.

**Action (35s).** Three vignettes, each ~10–12s:

**Vignette 5a — Normal write (10s).** User types a new note in both panes simultaneously: *"Meeting notes 2026-05-15: Discussed Q3 OKR pivot. Decision pending CFO review by 2026-05-22."*
- Both panes save. **Obsidian: silent.** **Atlas: green "Signed ✓" badge animates in next to the note title, tooltip: "Signed by laptop-A-key · Rekor anchor pending".** Two seconds later the Atlas badge upgrades to "Signed ✓ + Anchored" (chain icon appears).

**Vignette 5b — Cross-device edit (12s).** User clicks device-selector → `[Laptop B]`. Both panes re-sync to show Laptop-B state. User edits the note: appends *"\n\n2026-05-15 18:30: CFO approved verbally. Written confirmation expected tomorrow."*. Save.
- **Obsidian:** silent. Just the diff in the editor.
- **Atlas:** new badge "Signed ✓ (revision 2)" appears. Below the note, a **"Revision history"** panel slides up showing:
  - **Rev 1** — Signed by `laptop-A-key` · Anchor: logIndex 187,234,512 · 2 h ago
  - **Rev 2** — Signed by `laptop-B-key` · Anchor: logIndex 187,234,891 · just now
  - Each revision has a **"Verify"** button.

**Vignette 5c — Tampering attempt (13s).** Off-camera narration overlay: *"Now: malicious teammate edits the vault files directly via filesystem."* Both vaults receive an out-of-band file-write that changes "CFO approved verbally" to "CFO denied".
- **Obsidian:** reloads. Shows the tampered content. **No alarm. No detection. The note now says what the attacker wanted.**
- **Atlas:** reloads. **Red banner slides down from top:**

```
⚠  TAMPERING DETECTED
    Note: "Meeting notes 2026-05-15"
    Expected hash: sha256:7f3a91b2…ce8d  (from Rekor anchor)
    Found hash:    sha256:c2e5f8a4…91d2
    
    The on-disk content does not match the signed revision.
    
    [ Restore from signed anchor ]  [ View tampering source ]
```

User clicks "View tampering source" → modal: *"Filesystem write at 2026-05-15T20:14:01Z. No matching signature from any device key. Source: external write (not via Atlas sync). Restored content has been pulled from Rekor anchor logIndex 187,234,891."*

**Reveal (15s).** Caption fades in over the split:

> **"Your Second Brain, but trustable for the AI era."**

Sub-caption: *AI agents reading your notes only see signed content. Tampering is visible. History is permanent. No vault lock-in.*

**CTA (5s).** Button: **"Try Atlas Second Brain (beta) →"** Sub-text: *Free tier · local-first · same trust property as the enterprise tier.*

### 5.2 Metadata

- **Target audience:** Knowledge workers, researchers, Obsidian/Notion/Roam power-users, AI-power-users who pipe notes into Claude/ChatGPT, journalists, lawyers (note-integrity-matters), academic researchers (citation-integrity). Per `v2-session-handoff.md` §4, this is the "Verifiable Second Brain" market category — adjacent to but DIFFERENT from the agent-builder audience for Demos 1/2/3/5.
- **Target emotion:** surprise + trust. The surprise lands in Vignette 5c — viewer didn't expect the Obsidian vault to be silently corrupted; the relief lands in the Atlas red banner.
- **Technical assets needed:**
  - **Atlas Second Brain consumer-facing UI** (DOES NOT EXIST — this would be a new product surface; Atlas today is positioned as agent-substrate, not as a consumer note-taking app)
  - **Local-first sync layer** (NOT YET DESIGNED — V1 is server-only via HTTP API; local-first would need a sync protocol, conflict resolution, multi-device key management)
  - **Tampering-detection UI on top of WASM verify** (the verify primitive exists; the "red banner + restore-from-anchor" UX is new)
  - **Multi-device key management** (per-device signing keys with shared trust root — V1 has per-workspace HKDF; per-device-key for a single user is a new variant)
  - **Revision history with signature chain rendering** (V1 has the chain in `events.jsonl`; the UI is new)
  - **Obsidian-equivalent markdown editor + graph view** (entire consumer product — significant build)
- **Production complexity:** 5/5 — most ambitious of the five. This is essentially "build an Obsidian competitor" + Atlas trust layer on top.
- **Readiness:** **REQUIRES V2-γ+ CONSUMER PRODUCT.** The trust substrate exists; the consumer product does not. **Two paths forward:** (a) build a real Second Brain product (5–15 sessions, depends on scope); (b) mock the demo for marketing today (the cryptographic interactions are simulatable with the V1 substrate — the Obsidian comparison is choreography). Phase 2 critique should challenge: *do we go consumer-product at all, or is Demo 4 a marketing-only demo for the audience-broadening narrative?*

---

## 6. Demo 5 — Mem0g Hybrid (Speed + Trust)

**Concept:** Side-by-side benchmark: verified Atlas query without cache vs. with Mem0g cache. Same provenance, same accuracy, ~12× speedup. Defeats the "cryptographic = slow" objection.

### 6.1 Storyboard

**Setup (8s).** Two-pane horizontal split, identical UI styling:
- **Left pane header:** *"Atlas standard query (no cache)"* — ArcadeDB traversal + event-verification path.
- **Right pane header:** *"Atlas + Mem0g hybrid query"* — Mem0g cache hit with provenance pointers.
- **Both panes:** same query input field, pre-filled with: *"What did `did:atlas:hermes-instance-3` learn about Bank XYZ in Q1 2026?"*
- **Below each pane:** empty result panel + a stopwatch reading 0.00s.

**Action (25s).** User clicks **"Run on both"** (single button spanning both panes). Both stopwatches start simultaneously.

- **Right pane (Mem0g):** Progress bar fills in <1 second. At **T+1.44s** the result panel populates with 12 facts, each carrying a Rekor logIndex pill. Stopwatch freezes at **1.44s**. Status: "Cache hit · provenance verified".
- **Left pane (standard):** Progress bar fills slowly. ArcadeDB traversal indicator → event-verification indicator → result hydration indicator. At **T+17.12s** the result panel populates with 12 facts, identical content, identical Rekor logIndex pills. Stopwatch freezes at **17.12s**. Status: "Full traversal · no cache".

A small numeric overlay between the panes ticks up live: *"Speedup: 11.9× · Accuracy delta: 0.0% · Provenance delta: 0 bytes"*.

**Reveal (20s).** Centre button: **"Show diff between results"**. Click → modal:

```
┌─────────────────────────────────────────────────────────────┐
│  Result comparison                                           │
│  ──────────────────────────────────────────────────────────  │
│  Facts returned:        12 · 12        (byte-identical)      │
│  Rekor anchors:         12 · 12        (byte-identical)      │
│  Witness cosignatures:  24 · 24        (byte-identical)      │
│  Result hash:           sha256:9c3a…f2b1                     │
│                         sha256:9c3a…f2b1  ✓ match            │
│  ──────────────────────────────────────────────────────────  │
│  → No diff. Results byte-identical. Same provenance.         │
│                                                               │
│  Mem0g is a derived cache; events.jsonl remains the only     │
│  authoritative source. Cache rebuilds from events.jsonl       │
│  deterministically. Trust invariant preserved.                │
│                                                               │
│  [ Rebuild cache from events.jsonl ]  [ View Mem0g schema ]  │
└─────────────────────────────────────────────────────────────┘
```

Below the modal, a small caption: *"Want to verify? `atlas-mem0g rebuild --from events.jsonl` produces a byte-identical cache."*

**Implication (15s).** Caption:

> **"Cryptographic trust without the speed tax."**

Sub-caption: *Use Atlas like any vector DB. Verified provenance is structural, not transactional. 91% p95 latency reduction vs. the verified base case — same answer, same proof.*

**CTA (5s).** Button: **"See the full benchmark suite →"** Sub-text: *Mem0g vs. Mem0 vs. Atlas-no-cache — all numbers reproducible*.

### 6.2 Metadata

- **Target audience:** AI engineers building RAG / memory systems, ML platform teams, anyone who's looked at Atlas and thought "but crypto-verification must be slow", Mem0/Letta/Zep evaluators. This is an **anti-objection demo** — its job is to remove the speed barrier from the buyer's mental model.
- **Target emotion:** clarity. Pure cognitive payoff — *"oh, it's not slow, never mind"*. Not a wow-demo; a confidence-demo.
- **Technical assets needed:**
  - **Mem0g LanceDB cache fully operational** (**REQUIRES W18c** — Layer 3 is currently scaffold-shipped per V2-session-handoff §0-NEXT: `Mem0gError::Backend("not yet wired")` placeholders in `lancedb_backend.rs`)
  - **Real fastembed initialisation** (REQUIRES W18c — supply-chain constants `HF_REVISION_SHA` + `ONNX_SHA256` + `MODEL_URL` are placeholders awaiting Nelson lift; `AtlasEmbedder::new` returns `supply-chain gate: ...` until lifted)
  - **ArcadeDB query path for the "standard" pane** (V2-β Layer 2 W17a/W17b/W17c — SHIPPED)
  - **Atlas Read-API endpoint surfacing both paths** (Phase 13.5 added a Read-API endpoint per the Mem0g handoff; the semantic-search endpoint is scaffold-only until W18c)
  - **atlas-web query-comparison UI** (NEW — two side-by-side query panes with stopwatches; ~1 session of UI work)
  - **Realistic benchmark fixture** — a workspace with enough facts that the 17s vs 1.4s gap is real, not theatrical (~10K events with semantic embeddings)
- **Production complexity:** 4/5 — most of the complexity is waiting on W18c (LanceDB body fill-in + supply-chain pin lift). Post-W18c the UI is straightforward.
- **Readiness:** **REQUIRES W18c COMPLETION.** This is the demo most-blocked by an explicit shipping welle. The exact numbers cited in the prompt (1.44s vs 17.12s, 91% speedup) are the Mem0g paper's headline figures — Atlas's actual numbers will likely be in the same neighbourhood but should be measured before the demo ships. **Phase 2 critique should challenge: is it honest to cite the Mem0g paper numbers before measuring Atlas's actual performance?** Recommend measuring first, demo-storyboarding second.

---

## 7. Demo Selection for Landing Page Hero

### 7.1 Recommendation

**Lead with Demo 2 (Continuous Audit Mode) as the landing-page hero.** Reasoning across four axes:

**7.1.1 Audience-fit.** Atlas's nearest-term, highest-ACV buyer is the compliance-driven enterprise — EU AI Act Art. 12 enters force 2026-08-02 (per `v2-session-handoff.md` §4 + README.md), DORA Art. 11–14 is already live for financial-services, GAMP 5 (July 2025) governs AI/ML in GxP context. These buyers have hard regulatory deadlines and a procurement budget; AI engineers building multi-agent systems have neither (yet). Demo 2 speaks directly to the buyer with money. Demos 1/3/5 speak to the dev who *recommends* a procurement; Demo 4 speaks to a consumer who pays $10/month.

**7.1.2 Emotional resonance.** *"The regulator's key is IN the system"* is the strongest "wait, what?" beat in the five-demo set. Trust + clarity hits harder than trust + power on a landing page where the viewer's default state is mild scepticism. Compare:
- Demo 1: *"Two agents writing into the same graph"* → reaction: *"OK, but why do I care?"* (requires the viewer to already care about multi-agent systems).
- Demo 2: *"The regulator's key is cosigning every decision in <500ms"* → reaction: *"Wait — that's structurally different from how compliance works today."*
- Demo 3: *"Agent has a verifiable CV"* → reaction: *"Interesting, but how does it work?"* (requires reading; loses the 30s window).
- Demo 4: *"My Obsidian vault got tampered"* → reaction: *"Oh that's clever"* — but the viewer needs to already use Obsidian for the comparison to bite.
- Demo 5: *"It's not slow"* → reaction: cognitive relief; not emotional hook.

Demo 2's surprise is *categorical* — it reframes what compliance can be — not *technical* — it doesn't require the viewer to understand graphs, vector DBs, or agent identities.

**7.1.3 Demo-feasibility-at-current-state.** Demo 2 needs ~1 session of UI work over fully-V1-shipped substrate. Demos 1, 3, 5 each need 2–4 weeks of V2-β / W18c shipping before they're REAL (not mocked). Demo 4 needs a consumer-product build (5–15 sessions). The landing page can ship in 2–3 weeks with Demo 2; the same landing page with Demo 1 as hero would either ship later or ship with a fundamentally-mocked recording, which is a brand risk (Atlas's whole positioning is *"this is provable, not claimed"* — a mocked hero demo on the landing page undermines that exact claim).

**7.1.4 Narrative compression.** Demo 2 lands "structural compliance" in 60 seconds without requiring the viewer to understand:
- Graph databases (Demo 1)
- DIDs / agent identities (Demo 3)
- Consumer note-taking apps (Demo 4)
- Vector databases / RAG / Mem0g paper context (Demo 5)

Lowest cognitive load → highest conversion rate on a cold-traffic landing page. The viewer's only pre-requisite is: *"I know compliance is a thing and I know it's painful."* Every enterprise viewer knows that.

**7.1.5 Competitive differentiation.** **No competitor — Mem0 / Letta / Zep / Anthropic Memory / OpenAI Memory / Obsidian / Notion / Roam / Graphiti / Neo4j — can show a regulator's cosignature in real-time.** The demo IS the differentiation; it visibly demonstrates a category Atlas owns alone. Demos 1/3/5 demonstrate features that competitors could plausibly bolt on; Demo 2 demonstrates an architectural property that requires Atlas's specific trust-model.

### 7.2 Risk in this recommendation

**Risk:** Leading with the compliance pitch risks pigeonholing Atlas as a regtech-only tool, losing the AI-engineer audience that drives developer-community / OSS-momentum / bottom-up adoption.

**Mitigation:** Pair the hero demo with a **secondary "demo strip"** below the hero — three smaller looping demos (Demo 1, Demo 3, Demo 5) at ~150-200px height each, captioned for the dev audience. The hero captures the enterprise buyer; the strip captures the developer browsing under their lunch break. Demo 4 stays off the landing page until/unless Atlas decides to ship the Second Brain consumer product.

### 7.3 Sequencing recommendation (post-Phase-2)

If Phase 2 critique agrees with Demo 2 as hero:
1. **Build the "regulator-witness timeline" UI component** over V1 substrate (~1 session).
2. **Record a 60s hero loop** with a real (non-mocked) regulator-named witness in a real federation roster.
3. **Ship the demo strip** placeholders with "Coming with V2-β" / "Coming with W18c" labels — honest about readiness; turn each into a real demo as the engineering ships.
4. **Don't gate the landing page on all five demos being real.** The hero alone is enough to launch.

---

## 8. Production Requirements

Per-demo dependency matrix — what must exist, what's blocked, what's the path forward.

### 8.1 Demo 1 (Multi-Agent Race)

| Asset | Status | Blocker |
|---|---|---|
| `atlas-mcp-server` write path | V1.19 LIVE | none |
| HTTP API `/write-node` | V1.19 LIVE | none |
| Per-agent Ed25519 keys | V1 per-tenant generalises | V2-β agent-DID surfacing required |
| Atlas Graph Explorer `/graph` | DOES NOT EXIST | **V2-β Phase {explorer}** — graph viz + click-to-verify modal |
| Per-fact colour-coded passport ring rendering | DOES NOT EXIST | depends on graph explorer + agent-DID |
| WASM verify iframe on click | partial | verifier exists; iframe-glue is new (~0.5 sessions) |
| Two-agent demo recording fixture | DOES NOT EXIST | needs orchestrated script: Claude Desktop + Cursor (or custom MCP client) writing to same workspace; trivial post-explorer |

**Blocking-today:** V2-β explorer + agent-DID layer. **Earliest demo-able:** ~2-3 sessions post-W19 (assuming explorer is V2-β's next major welle).

### 8.2 Demo 2 (Continuous Audit Mode)

| Asset | Status | Blocker |
|---|---|---|
| `atlas-witness` federation | V1 LIVE | none |
| Multi-key trust root | V1 LIVE | none |
| External witness roster (`cosi-witness-roster`) | V1 LIVE | none |
| Sub-500ms cosignature latency | V1 LIVE | none |
| Live witness timeline UI component | DOES NOT EXIST | **~1 session of UI work** over existing verify API |
| Regulator-witness simulator (real witness, fake name) | SIMULATABLE | just configure an extra witness with a chosen pubkey + display name |
| Trust-root health-status surfacing | partial | need `/api/atlas/trust-root-status` endpoint + UI ticker |

**Blocking-today:** none. **Earliest demo-able:** ~1 session post-W19. **THIS IS THE CHEAPEST DEMO TO SHIP REAL.**

### 8.3 Demo 3 (Agent Passport)

| Asset | Status | Blocker |
|---|---|---|
| Per-agent Ed25519 keypair | V1 per-tenant generalises | V2-β agent-DID required |
| Agent-DID layer (`did:atlas:<pubkey-hash>`) | DOES NOT EXIST | V2-β scope per `v2-vision-knowledge-graph-layer.md` §2.7 (to be added by Doc B revision) |
| Aggregation queries (count, distinct, skill-domain) | DOES NOT EXIST | V2-β read-API + ArcadeDB query layer (Layer 2 W17c shipped but queries are new) |
| Retraction-tracking schema | NOT YET DESIGNED | needs ADR; what's a "retraction" cryptographically? |
| Passport export format | NEW | needs spec; signature envelope, contained fields |
| `atlas-verify-cli verify-passport` subcommand | NEW | extends existing `verify-trace` CLI |
| Agent registry (`atlas-trust.dev/passports`) | DOES NOT EXIST | new public-discovery surface |

**Blocking-today:** all of agent-DID + read-API + passport schema. **Earliest demo-able:** ~3-4 sessions post-W19. **Not in the near-term landing-page critical path.**

### 8.4 Demo 4 (Verifiable Second Brain)

| Asset | Status | Blocker |
|---|---|---|
| Atlas Second Brain consumer UI | DOES NOT EXIST | new consumer product surface — significant build |
| Local-first sync layer | NOT YET DESIGNED | new sync protocol + conflict resolution |
| Per-device key management | partial | V1 per-workspace HKDF exists; per-device variant is new |
| Tampering-detection UI | partial | verify primitive exists; "red banner + restore" UX is new |
| Revision history UI | partial | chain is in `events.jsonl`; UI is new |
| Obsidian-equivalent markdown editor + graph | DOES NOT EXIST | full consumer-product build |

**Blocking-today:** entire consumer product. **Earliest demo-able (real):** 5–15 sessions depending on scope. **Earliest demo-able (marketing-mock):** today, but raises brand-honesty concerns. **Phase 2 question:** do we go consumer-product at all?

### 8.5 Demo 5 (Mem0g Hybrid)

| Asset | Status | Blocker |
|---|---|---|
| Mem0g LanceDB cache | scaffold-shipped (W18b) | **W18c** — body fill-in (currently `not yet wired`) |
| Real fastembed initialisation | scaffold-shipped (W18b) | **W18c** — supply-chain pins lift (Nelson task) |
| ArcadeDB query path (Layer 2) | V2-β Layer 2 W17c SHIPPED | none |
| Atlas Read-API endpoint | partial (Phase 13.5) | semantic-search endpoint returns 501 until W18c |
| Query-comparison UI | NEW | ~1 session of UI work |
| Realistic benchmark fixture (~10K events with embeddings) | DOES NOT EXIST | new — required to make the 17s vs 1.4s gap real |
| Honest performance measurement | NEEDED | **don't cite Mem0g paper numbers** — measure Atlas's actuals |

**Blocking-today:** W18c (per `v2-session-handoff.md` §0-NEXT W18c parallel-track). **Earliest demo-able:** post-W18c + ~1 session of UI + benchmark-fixture work.

### 8.6 Cross-demo shared assets

These are not demo-specific but are prerequisites multiple demos share:

| Asset | Status | Shared by |
|---|---|---|
| Atlas Graph Explorer (graph viz) | DOES NOT EXIST | Demos 1, 3 |
| Per-agent Ed25519 key surfacing as DID | DOES NOT EXIST | Demos 1, 3 |
| Read-API aggregation queries | DOES NOT EXIST | Demos 1, 3, 5 |
| WASM verify embedded in UI modals | partial | All demos |
| atlas-web "demo-recording mode" (clean state, deterministic timestamps) | DOES NOT EXIST | All demos |

Building the cross-demo shared assets unlocks multiple demos at once and is the highest-leverage post-W19 engineering investment for the landing-page surface.

---

## 9. Open Questions for Phase 2 Critique

Phase 2 critique agents (product / business / engineering / design) should challenge these explicitly. Format: `Q: <question>. Context: <why this matters>. Status: open.`

**Q1: Are these demos honest about current Atlas capabilities, or do they require V2-α/β/γ + W18c before they're real?**
Context: §8 production-requirements matrix says four of five demos are blocked on V2-β / W18c / V2-γ. Marketing-mock demos are easy to build today but undermine Atlas's "trust is structural, not claimed" positioning if they end up on the landing page as the hero. Phase 2 should adjudicate: is it OK to ship a marketing-mock hero, or must the hero be real on day-1?
Status: open.

**Q2: Is the multi-agent race demo (Demo 1) emotionally compelling enough to lead with, OR is Demo 2 (Continuous Audit) the right hero?**
Context: §7 recommends Demo 2 across all four axes (audience-fit, emotional resonance, feasibility, narrative compression). But Demo 1 is the more "AI-zeitgeist" demo — it speaks to the multi-agent moment that's currently driving a lot of AI-engineering attention (LangGraph / CrewAI / AutoGen / OpenAI Agents SDK / Anthropic's MAS RFCs). Phase 2 should challenge: is the enterprise-compliance hero too narrow for a 2026-mid landing page when the AI conversation is dominated by multi-agent systems?
Status: open.

**Q3: Should we have a "non-AI / human-consumer" demo (Demo 4 — Second Brain) at all, or focus exclusively on the agent-builder + compliance-buyer audiences first?**
Context: Demo 4 implies building a whole consumer product to compete with Obsidian/Notion/Roam. The "two-market positioning" thesis (per `v2-session-handoff.md` §4 + Doc A) argues Atlas serves both Second Brain AND multi-agent shared memory. But shipping both means double the surface area, double the support, double the marketing. Phase 2 should challenge: is the two-market thesis right, or should V2 focus on one market for the next 12 months?
Status: open.

**Q4: Are the cited performance numbers in Demo 5 (1.44s vs 17.12s, 91% speedup) honest if they're imported from the Mem0g paper rather than measured on Atlas?**
Context: Demo 5's numeric overlay is load-bearing for the credibility of the entire demo. If Atlas's actual Mem0g-hybrid performance differs from the paper's by even 30%, citing the paper numbers becomes misleading. Phase 2 + W18c engineering should agree: measure first, demo second; OR cite both ("paper claim: 91%; Atlas measured: X%") for transparency.
Status: open.

**Q5: Do agent passports (Demo 3) need a centralised registry, or can they be fully decentralised (each Atlas instance hosts its agents' passports)?**
Context: Demo 3's CTA points to `atlas-trust.dev/passports` (centralised). But Atlas's positioning is *"no central authority"*. If we ship a central registry, we contradict our positioning. If we don't ship a registry, the demo's CTA breaks — there's nowhere for the viewer to "browse" agents. Phase 2 + architecture should resolve.
Status: open.

**Q6: For Demo 2 (Continuous Audit), is the "regulator key in trust root" pattern legally accepted by actual regulators, or is it a positioning aspiration?**
Context: The demo implies BaFin (or any regulator) WOULD federate their witness key into private-company trust roots. This requires regulator buy-in that does not exist today and may take years of regulator-relations work. Counsel-engagement scope (per `.handoff/v2-counsel-engagement-scope.md`) covers GDPR Art. 4(1) hash-as-PII but not regulator-federation legality. Phase 2 should flag: is Demo 2's narrative honest about being aspirational vs. operational?
Status: open.

**Q7: Should the demo-strip on the landing page include readiness labels ("Coming in V2-β" / "Coming with W18c")?**
Context: §7.3 mitigation suggests pairing Demo 2 (hero) with three smaller demos (1, 3, 5) on the landing page. If those are mocked while the hero is real, do we label them? Pro-label: brand-honesty consistency. Con-label: dilutes "ready now" impression. Phase 2 should adjudicate brand-tone.
Status: open.

**Q8: Is a 60-second hero demo too long for a landing-page video, given current cold-traffic attention spans (median ~8s before bounce)?**
Context: Atlas's narrative is dense — "regulator cosignature in real time" needs context to land. But landing-page videos that exceed 30s typically lose 60%+ of viewers before the implication beat. Phase 2 + design should challenge: can we compress Demo 2 to 30s without losing the reveal? Or do we run a 30s "teaser" + a 60s "deep dive" behind a click?
Status: open.

**Q9: For Demo 4 (Second Brain), is a marketing-only mock acceptable IF labelled as a concept (e.g., "Coming 2027") with a waitlist signup?**
Context: Demo 4's concept (verifiable Second Brain for AI-era) is strong but the product doesn't exist. A waitlist demo signals market intent without claiming product capability. Phase 2 should challenge: is a clearly-labelled-concept demo on the landing page brand-honest, or does it still undermine "trust is structural"?
Status: open.

**Q10: Should Demo 3 (Agent Passport) include a "skill-domain attestation" auto-derivation step, or is that overstating what Atlas can verifiably attest to?**
Context: Demo 3's storyboard shows skill-domains auto-derived from fact topics ("financial-research: 487 facts"). The signing/anchoring proves the facts WERE written; it doesn't prove the agent is GOOD at financial-research. The demo blurs "verified that it wrote facts labelled as financial-research" with "verified that it's a competent financial researcher". Phase 2 + product should clarify the attestation boundary.
Status: open.

**Q11: For the landing-page demo recordings — are they pre-recorded videos, live in-browser interactive demos, or both?**
Context: Pre-recorded videos load fast and look polished but can't be interacted with. Live interactive demos prove the product works but risk performance issues, state contamination, abuse. Phase 2 + engineering should pick: video-first (simpler), live-first (proves the substrate), or both (most work).
Status: open.

**Q12: Should any of the demos include a deliberate "tampering attempt" moment (like Demo 4's Vignette 5c), or is that too negative for the hero / strip?**
Context: §1.3 visual conventions argue every demo with time should include one "tampering detected → red banner" moment as the negative-space proof. But on a hero demo specifically, a red banner mid-flow may confuse first-time viewers. Phase 2 + design should adjudicate: red-banner in hero (proves the system catches tampering) or red-banner only in deep-dive (keeps hero entirely positive)?
Status: open.

**Q13: Is the German-regulator framing (BaFin in Demo 2) the right choice, or should the hero use a US/UK/global regulator name for international audience reach?**
Context: Nelson is German + Ultranova is based in DE + the EU AI Act is the strongest near-term regulatory driver. BaFin is the natural choice. But landing-page viewers from US fintech, UK regtech, APAC compliance won't recognise BaFin. Phase 2 + business should challenge: do we localise the hero per-market, or pick a global-recognisable regulator (SEC? FCA? ESMA?), or use a generic label ("your regulator's key")?
Status: open.

**Q14: Should Atlas ship an "open-weight agent reference integration" (per `v2-vision-knowledge-graph-layer.md` §4 V2-γ) for the hero demo, or is Claude Desktop sufficient as the demo agent?**
Context: V2-γ §4 candidate (c) is "open-weight model (Hermes-4 or Llama-4) — strongest 'vendor-neutral / works with open weights' narrative". If the hero demo uses ONLY Claude Desktop, Atlas ties its first impression to Anthropic. If the hero uses an open-weight model, the vendor-neutrality story lands harder. Phase 2 + positioning should challenge: which agent appears in the hero, and what does that signal?
Status: open.

**Q15: For Demo 5 (Mem0g Hybrid), should we benchmark against Mem0 / Letta / Zep directly, or only against Atlas-without-cache?**
Context: Atlas-without-cache vs. Atlas-with-cache shows the engineering win (no speed tax). Atlas-with-cache vs. Mem0/Letta/Zep shows the market positioning (Atlas matches their speed + adds trust). Phase 2 + competitive should adjudicate: which comparison wins more in the 60s demo window?
Status: open.

---

## 10. Cross-cutting design notes

### 10.1 What every demo must avoid

- **Generic stock visuals.** No "AI brain" graphics. No glowing nodes-and-edges abstract animations. Every visual element must be grounded in actual Atlas UI surfaces.
- **Vendor-coupling-by-default.** When the demo needs to show an agent, use multiple vendors (Claude + custom-MCP + open-weight) rather than only one. This reinforces Atlas's agent-agnostic positioning per `v2-vision-knowledge-graph-layer.md` §1.
- **Trust-talk without trust-show.** Every "verified" claim in caption MUST be accompanied by a visible cryptographic artefact (Rekor logIndex, signature fingerprint, witness key) — not just a green checkmark on its own.
- **Mocked timestamps.** All times shown must be plausible (sub-500ms for cosignatures, multi-second for full traversal). Don't show "1ms" for things that can't physically happen that fast.
- **Hidden failure modes.** The demos optimise for the green-path; but every demo (per §1.3) should include at least one "what happens when something breaks" moment when time allows. Demo 4's Vignette 5c is the model.

### 10.2 What every demo should reuse

- **Visual passport-ring colour scheme** (agent → colour, stable across all demos)
- **Provenance badge vocabulary** (green check / blue chain / orange shield / purple star — §1.3)
- **Modal layout** for reveal-beats (consistent header, monospace pubkey hashes, separator lines, action buttons at bottom)
- **Caption typography** for implication-beats (one bold line + one sub-line in italic; reads in ~4s)
- **CTA button styling** (single verb, sub-text below, brand-aligned colour)

### 10.3 Recording vs. live-interactive tradeoff

Strong recommendation: **hero demo is a pre-recorded 60s video with a "Try it live" button below**. The video loads fast, looks polished, and primes the viewer; clicking "Try it live" launches an in-browser sandbox where they can write a fact, see it sign, click to verify. Hero-page conversion comes from the video; deep-engagement comes from the live sandbox.

### 10.4 Demo state management

For the live-interactive sandbox specifically: every demo session needs its own ephemeral Atlas workspace (HKDF-derived from a session token) so different viewers don't see each other's writes. The trust property still holds (each session has its own signed event log + Rekor anchors), but workspaces are auto-pruned after ~24 hours.

---

## 11. What this doc deliberately did NOT do

For Phase 2 critique transparency, here's what was scoped OUT:

- **Production-grade copywriting.** Captions are first-draft. Copy review is a separate pass.
- **Voiceover / soundtrack design.** All demos described assume silent or text-overlay-only; if voiceover is needed, that's a separate brief.
- **Localisation (DE / EN / FR / ES).** Demos described in English; localisation is a separate workstream tied to per-market landing-page variants.
- **Accessibility (screen-reader narration, captions, colour-blind palette).** Required for production; out of scope for v0 storyboards.
- **Specific agent vendors beyond the 1-3 examples per demo.** Atlas is agent-agnostic; final agent choice per demo is a positioning decision (see Q14).
- **A/B testing strategy.** Which hero converts better is empirical; demos described here are the candidates, not the winners.
- **Customer-success demos.** Demos described are net-new-user-acquisition focused. Customer-onboarding / upsell demos are a separate doc.

---

**Doc owner:** Phase 1 Foundation Document E. **Last edited:** 2026-05-15. **Next milestone:** Phase 2 critique pass per `v2-session-handoff.md` §8 (deliberate, not rushed). Critique focal points: §7 hero-selection adjudication, §8 production-requirements honesty, §9 open questions Q1–Q15.
