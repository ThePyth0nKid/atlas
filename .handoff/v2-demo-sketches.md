# Atlas V2 — Demo Sketches v0

> **Status:** Draft v0 (2026-05-12). Demo storyboards for atlas-trust.dev landing page + investor / customer pitches. Designed to be passed to product / brand / strategy agents for critique in Phase 2.
>
> **Reading order:** Each demo is self-contained. Methodology first, then 5 demos, then hero selection, then production-blockers per demo, then open questions.
>
> **Author's framing:** Atlas V1.0.1 is LIVE on npm. The graph-DB / query / agent-passport layer is V2-α/β/γ work. **Most demos below need projection-and-explorer (V2-α/β) before they can be shot for real.** Each demo carries a readiness flag — readers should not assume any of these can be filmed *today*.

---

## Methodology

Each demo follows a **5-block storyboard** designed for a 30-90 second video or live walk-through:

| Block | Duration | Purpose | Viewer State |
|---|---|---|---|
| **Setup** | 5-10s | Frame the world — what is on screen, who's the protagonist | "Okay, I see what kind of app this is." |
| **Action** | 15-40s | The thing happens — agent writes, graph populates, query executes | "Something is happening that I haven't seen before." |
| **Reveal** | 10-20s | The verification moment — click → cryptographic proof appears | "Wait, that's actually verified?" |
| **Implication** | 10-20s | One-sentence narrator-line / on-screen text — why this matters | "Oh, this is structural, not cosmetic." |
| **CTA** | 5s | What the viewer does next (install, request demo, read docs) | "I want to try this." |

For each demo we additionally specify:
- **Audience** — who the demo is for (the persona, not the role)
- **Target emotion** — surprise / trust / power / clarity (one primary, optional secondary)
- **Atlas surfaces involved** — which crates / apps / repos are exercised
- **Tech assets needed** — what must exist (real or mocked) to film
- **Production complexity** — 1 (today, mockable) to 5 (requires V2-γ + integrations)
- **Readiness flag** — `TODAY` / `V2-α` / `V2-β` / `V2-γ` / `V2-δ`

**Honesty constraint:** if the demo requires a capability Atlas doesn't have yet, the readiness flag must say so. We do not fake the verifier output. We can mock graph data + Hermes-Agent UX, but the cryptographic verification has to be real (this is the whole differentiator — faking it is suicide).

---

## Demo 1 — Multi-Agent Race (Verifiable Attribution)

**Tagline:** *"Every fact has a verified author. No more 'the AI said it.'"*

**Audience:** AI engineers, agent builders, multi-agent-framework adopters (LangGraph / AutoGen / CrewAI / OpenAI Agents SDK / custom MCP hosts).
**Target emotion:** trust + power (secondary: surprise — they've never seen this).
**Atlas surfaces:** atlas-mcp-server (V1.19 Welle 1 — live), atlas-web `/graph` (V2-β), atlas-trust-core, Rekor anchor display, WASM verifier inline.
**Production complexity:** **4/5**
**Readiness flag:** **Requires V2-α + V2-β** (graph projection + Cytoscape explorer with per-node Verify button). Multi-agent attribution itself works today via per-agent Ed25519 keys (V1 supports per-workspace keys; per-agent extension is a config-only change).

### Storyboard

**Setup (8s) — Frame the world.**
- **Left third of screen:** Hermes-Agent chat UI (Hermes-Agent's existing web frontend, embedded as iframe or recreated; Hermes-Agent-purple accent color). Status line: *"Hermes-instance-A connected to atlas-trust.dev/workspace/demo"*.
- **Middle third:** Claude Desktop window. Status line: *"Claude (via MCP) connected to atlas-trust.dev/workspace/demo"*.
- **Right third:** atlas-web `/graph` view — empty graph, just a workspace label "demo" at the center, plus a small legend in the corner: 🟣 Hermes / 🟠 Claude / ⚪ Unattributed.
- **Top banner:** "Two agents. One shared verifiable memory."

**Action (35s) — Race the agents.**
- User (off-camera) types the same prompt into both chats simultaneously: *"Research the EU AI Act Article 12 anchoring requirements and record what you find."*
- **0:00-0:05:** Both agents start streaming responses. Right pane is still empty but a pulsing "Listening to events.jsonl..." indicator appears in the bottom-right.
- **0:05-0:20:** Fact nodes start animating into the graph. Hermes's facts appear in **🟣 purple**, Claude's in **🟠 orange**. Edges form: `EU AI Act` --[has_article]--> `Article 12` --[requires]--> `Automatic Event Records`. Some facts overlap (both agents discover "Article 12 in force 2026-08-02") — graph shows the same entity with TWO incoming attribution edges (one from each agent's passport key).
- **0:20-0:30:** The graph has ~12 nodes / ~18 edges. Camera zooms into one specific entity: "Rekor logIndex requirement" — connected to BOTH agent passports. Bottom-left counter: "Hermes: 7 facts / Claude: 5 facts / 2 corroborated by both".
- **0:30-0:35:** Cursor hovers over the corroborated node. Tooltip preview: *"Signed by Hermes-instance-A (12:04:01.221Z) AND Claude-via-mcp-instance-1 (12:04:03.847Z). Rekor anchored."*

**Reveal (15s) — Click → cryptographic proof.**
- Cursor clicks the corroborated node. A side panel slides in (1.5s slide animation). Panel content, top-to-bottom:
  - **Fact:** "EU AI Act Article 12 requires Rekor logIndex for tamper-evident event records."
  - **Signed by:** two passport rows — `🟣 did:atlas:hermes-a-9f7c3...` (`Verify ✓`) and `🟠 did:atlas:claude-mcp-1-4a8e1...` (`Verify ✓`).
  - **Rekor anchor:** logIndex `2,718,281` — linkable `(view in Rekor)` + a small green check (offline-WASM-verifier ran live, result baked into the page).
  - **Witness cosignatures:** 2 of 2 witnesses ✓ (`atlas-witness-eu`, `atlas-witness-us`).
  - **Status banner:** `VERIFIED — independently, no network call to atlas-trust.dev`.
- Subtle browser-devtools overlay flashes for half a second showing the network tab had ZERO outbound requests during the verify click — driving home "this runs in your browser".

**Implication (15s) — Why this matters.**
- Full-screen text fades in over a darkened graph: *"Every fact, signed by the agent that wrote it. Cross-agent corroboration is now cryptographic. **Provenance is structural, not promised.**"*
- Smaller line below: *"Built on Atlas — Apache-2.0 verifier, SLSA Build L3 provenance, EU AI Act Article 12 ready."*

**CTA (5s)**
- *"npm install @atlas-trust/verify-wasm — atlas-trust.dev"*
- GitHub stars counter, npm badge, "Try the playground →" link.

### Why this is strong

- It's the **only demo where the unique structural property is visible** in the first 10 seconds (two different colors in the same graph = no other system shows this).
- Hits the AI-engineer audience squarely (the target persona for V2-γ reference-integration work).
- Naturally showcases the agent-agnostic positioning — Hermes (open-weight) + Claude (vendor) writing into the same substrate is the strongest narrative beat.

### Risks

- 35-second Action block is long. May need ruthless trimming or speed-up to keep total runtime under 90s.
- The "race" framing could feel gimmicky — alternative framing: "two specialists collaborating", "agent + human reviewer". Test framing variants with audience before locking.

---

## Demo 2 — Continuous Audit Mode (Regulator Witness)

**Tagline:** *"Compliance is structural, not periodic. The regulator's key is in the system."*

**Audience:** Compliance officers, regulatory affairs leads, banking/insurance/healthcare CTOs, EU AI Act-exposed enterprises.
**Target emotion:** trust + clarity (secondary: relief — "I don't have to chase audit logs anymore").
**Atlas surfaces:** atlas-witness (V1, live), atlas-mcp-server, atlas-web with regulator-view UI (V2-β), trust-root federation (V1.14 Scope J — live).
**Production complexity:** **3/5**
**Readiness flag:** **Mostly TODAY** for the cryptographic substrate (federation roster + cosignature is V1.14 Scope J, shipped). UI for "regulator view" is V2-β. The demo's emotional payoff (the live cosignature animation) is a UI presentation of capabilities that already exist.

### Storyboard

**Setup (10s) — Frame the world.**
- Full-screen Atlas-web UI styled as a bank's internal "Compliance Console". Subtle visual cues: navy and steel-grey palette, no-nonsense typography (Inter / Source Sans), small "BankCo Compliance" logo top-left.
- Top status bar shows three connected entities, each with a colored dot:
  - 🟢 `BankCo Atlas Workspace` — online
  - 🟢 `atlas-witness-eu` — online, cosigning
  - 🟢 `BaFin-witness-eu` — online, cosigning (the regulator's key, presented as if BaFin operates a witness node)
- Subtitle: *"All chain heads are cosigned in real-time by both the Atlas EU witness and the regulator's witness. No periodic reports. No after-the-fact reconstruction."*

**Action (25s) — Risky write under live observation.**
- A simulated agent (named in the UI as `BankCo-RiskAdvisor-v3`) writes a new credit-risk recommendation into the workspace.
- Right-side log streams the event:
  - `T+0.0s` — Event 4,892 received. Ed25519 signature OK.
  - `T+0.3s` — Hash chain append confirmed. Parent ref c9f...
  - `T+0.8s` — atlas-witness-eu cosigned ✓
  - `T+1.1s` — BaFin-witness-eu cosigned ✓ (this row gets a brief glow / highlight animation)
  - `T+1.8s` — Sigstore Rekor anchored, logIndex 2,718,294 ✓
- The "BaFin cosigned" line is what the viewer's eye should land on. **It's the surprise.** A regulator co-signing every write, live.

**Reveal (15s) — The compliance-officer click.**
- A compliance-officer persona clicks the event. The provenance panel opens, showing the **regulator's signature inline with the agent's signature** — both verified, both timestamped within sub-2-second windows.
- A subtle "Inspect with auditor's offline tool →" button is highlighted. Clicking it generates a downloadable verify-bundle.
- A second compliance-officer persona on a second monitor opens that bundle in `atlas-verify-cli` (or the WASM playground in any browser) and gets:
  ```
  ✓ VALID — all checks passed
  ✓ event-signatures — 1 signature verified (BankCo-RiskAdvisor-v3)
  ✓ anchors — 1 anchor verified (Rekor logIndex 2,718,294)
  ✓ witnesses — 2 presented / 2 verified (atlas-witness-eu, BaFin-witness-eu)
  ```
- Both monitors visible in the shot, identical green ✓ banners.

**Implication (15s) — Why this matters.**
- Full-screen overlay: *"EU AI Act Article 12 demands automatic, independently verifiable event records. Most vendors give you a dashboard. Atlas gives the regulator a key."*
- Smaller: *"BaFin's witness key is illustrative — real federation requires coordination with the supervisory authority. Atlas provides the substrate."* (legal-honesty disclaimer — important).

**CTA (5s)**
- *"Schedule a compliance briefing — atlas-trust.dev/compliance"*
- Pointer to `docs/COMPLIANCE-MAPPING.md` and the AI-Act mapping table.

### Why this is strong

- This is the demo that **closes enterprise sales**. CFO / CCO / Head-of-Compliance audiences see the "regulator-in-the-loop" framing and their next question is "how much does this cost".
- The substrate is **largely real today** — V1.14 Scope J ships witness federation; the regulator-named witness is a config-only deployment pattern.
- Hits the EU AI Act narrative without being preachy about it.

### Risks

- "BaFin witness" is illustrative — no regulator has actually committed to running a witness yet. Demo must be careful to label this as architectural capability, not a current customer. Suggest watermarking "Illustrative — see disclaimer" in the demo frame.
- Compliance-officer audiences are skeptical of vendor demos. Live cosignature pacing must be unfaked (`T+0.8s`, `T+1.1s` etc. must reflect real witness timings or the demo will read as scripted).

---

## Demo 3 — Agent Passport (Reputation Portability)

**Tagline:** *"Agents have CVs now. Cryptographic ones."*

**Audience:** Multi-tenant AI deployers, AI marketplaces / agent-app-stores, enterprises evaluating vendor agents, recruiting/hiring-AI platforms.
**Target emotion:** clarity + power (secondary: novelty — they have never seen an "agent passport" before).
**Atlas surfaces:** atlas-trust-core (per-agent Ed25519 keys — V1 supports per-workspace, per-agent is config-only extension), atlas-web `/agents/:did` page (V2-γ), did:atlas resolver (V2-α).
**Production complexity:** **4/5**
**Readiness flag:** **Requires V2-α + V2-γ.** Per-agent identity layer (did:atlas:<pubkey-hash>) is V2-α architecture work. Passport UI page is V2-γ. Underlying signing keys are V1 capability.

### Storyboard

**Setup (10s) — Frame the world.**
- An "Atlas Agent Marketplace" landing page (designed in atlas-web style). Header: *"Hire verified AI agents. Bring their track record with them."*
- Grid of agent cards — each card shows: avatar, agent name, role, agent-passport DID (truncated), and a "30-day verified history" badge.
- Cursor hovers over a card: **`Hermes-FinAnalyst-instance-7`** — "Equity research agent. 30-day Atlas-verified history."
- Click → enter agent passport page.

**Action (30s) — Agent passport walkthrough.**
- Passport page resembles a verified-LinkedIn-meets-GitHub-profile.
- **Header card:**
  - Agent name + avatar
  - DID: `did:atlas:hermes-fin-7-9f7c3b88a214...` (full hash on hover)
  - Model: `Hermes-4 / open-weight / locally deployable`
  - Operator: `FinResearch GmbH` (clickable — the org that runs this agent)
  - "Verify identity" button → opens WASM verifier inline with the passport bundle.
- **Statistics row** (animates in left-to-right, counter-style):
  - **1,247 facts written** (Atlas-recorded, all signed)
  - **0 retractions** (no event marked as "withdrawn-by-author")
  - **12 unique witness cosigners** (counts independent witness keys that have ever cosigned this agent's writes)
  - **3 organizations** (counts distinct workspace operators that have used this agent)
  - **First seen:** 2026-04-12 / **Last write:** 2026-05-12T11:42Z
- **Activity heatmap:** GitHub-style commit-heatmap, 30 days × 24 hours. Hot cells = many fact-writes.
- **Specialty cloud:** dynamically-extracted (V2-δ — Graphiti / GraphRAG retrieval) topic cloud — "EU corporate bonds, ECB rate decisions, Q1 2026 banking earnings...".

**Reveal (15s) — The cryptographic-CV click.**
- Click "Verify identity". A modal opens. WASM verifier runs in-browser against the passport bundle (a single signed CBOR document with the agent's DID, cumulative fact-count, witness-roster, anchor proofs).
- Output:
  ```
  ✓ VALID — agent passport verified
  ✓ Identity:  did:atlas:hermes-fin-7-9f7c3b88a214...
  ✓ Facts:     1,247 (Rekor-anchored across 247 anchor windows)
  ✓ Witnesses: 12 distinct keys cosigned at least one batch
  ✓ Status:    No retraction events ever recorded
  ```
- A "Download portable passport" button appears. Clicking it gives a `.atlas-passport` file (JSON+CBOR) that another organization could ingest to verify this agent's track record **without trusting atlas-trust.dev as an intermediary**.

**Implication (15s) — Why this matters.**
- Full-screen overlay: *"AI agent reputation has been vendor-locked since the beginning. Atlas gives every agent a portable, cryptographically signed history. **Hire the agent. Keep its CV.**"*
- Smaller: *"Verifier is Apache-2.0. Passport bundles are vendor-neutral. Run them through any Atlas-conformant verifier."*

**CTA (5s)**
- *"Build agent passports for your fleet — atlas-trust.dev/passports"*
- Link to dev docs for the passport-export API.

### Why this is strong

- **Novel category creation.** No competitor offers "portable agent reputation" — Anthropic Memory / OpenAI Memory are vendor-silo'd. This demo introduces a vocabulary ("agent passport") that the market doesn't have yet.
- Multi-tenant AI platforms and emerging "AI agent marketplaces" (Hugging Face Agents, Replicate, etc.) are an open market for this — they need a third-party trust layer.

### Risks

- Audience is narrower (multi-tenant AI deployers, agent-marketplace builders) — may not be a general-purpose landing-page hero.
- The demo depends on having **actually recorded** 30 days of an agent's writes to look credible. We need to run a real Hermes-Agent loop writing into a test workspace for 30 days BEFORE we can shoot this. **This is the demo with the longest lead time.**

---

## Demo 4 — Verifiable Second Brain (Obsidian Comparison)

**Tagline:** *"Your Second Brain, but trustable for the AI era."*

**Audience:** Knowledge workers, researchers, academics, journalists, individual Obsidian/Notion power users, small teams.
**Target emotion:** surprise + trust (secondary: discomfort — "wait, my current notes are not protected at all?").
**Atlas surfaces:** atlas-web personal-mode (V2-γ), atlas-trust-core, WASM verifier, MCP write surface (mobile-write app — V2-δ).
**Production complexity:** **4/5**
**Readiness flag:** **Requires V2-β + V2-γ.** Atlas's personal/Second-Brain UI doesn't exist yet — current atlas-web is operator-console-styled. A consumer-friendly note-taking shell + sync layer + multi-device flow is V2-γ scope minimum.

### Storyboard

**Setup (10s) — Frame the world.**
- Split screen, identical aspect ratio per side.
- **Left:** Obsidian vault, dark theme, one open note `Research-Notes.md` containing a paragraph of meeting notes.
- **Right:** Atlas Second Brain (atlas-web Second-Brain mode), same note, same content. Visually identical content, **subtle** trust UI: a tiny ✓ glyph next to the note title and a "signed 2 minutes ago" tooltip on hover.
- Top label: *"Same content. Different trust property."*

**Action (30s) — Three vignettes, ~10s each.**

**Vignette A (multi-device edit):**
- User picks up their phone (shown picture-in-picture). Opens Atlas mobile app. Edits the same note from phone — adds a sentence.
- On desktop, both panes update via sync. Left (Obsidian) shows the edit as a normal modification. Right (Atlas) shows the edit as a NEW signed event — both versions visible in a "history" sidebar, each with timestamp + signature.

**Vignette B (collaborator tampering):**
- A second persona, "Sam (teammate)", opens both vaults directly via file-system (not via Atlas UI). Edits both `Research-Notes.md` files at the OS level — adds a paragraph claiming a false conclusion.
- Left pane (Obsidian): no detection. The note now contains the false paragraph as if the user wrote it themselves.
- Right pane (Atlas): a **red banner** appears at the top — *"⚠ Content modified outside signed write path. Hash mismatch with last signed event. Click to investigate."*

**Vignette C (the investigation click):**
- User clicks the red banner. A diff view opens:
  - Top half: the LAST signed version of the note (with signature, timestamp, signing key fingerprint).
  - Bottom half: the CURRENT on-disk content (the tampered version) — clearly marked as unsigned/unauthored.
  - The interloping paragraph is highlighted in red.
- A "Restore signed version" button + a "Sign current version as my own (with attribution to me, T+now)" button.

**Reveal (10s) — The proof.**
- A small inline verifier output box appears under the diff:
  ```
  ✓ Last signed version VALID (Ed25519 OK, parent-hash OK, anchor verified)
  ✗ Current on-disk content INVALID (hash does not match any signed event)
  → Tampering localized to lines 14-18.
  ```
- The user clicks "Restore signed version". The note returns to its pre-tampering state. The Atlas history sidebar now shows a new event: "Restored from signed-version-N by user-X at T+now".

**Implication (15s) — Why this matters.**
- Full-screen overlay: *"Your Second Brain is feeding the AI now. If your notes can be silently tampered with, so can the AI's beliefs about you. **Atlas makes tampering visible.**"*
- Smaller: *"Personal mode is free. Sustainable Use license. Self-hostable."*

**CTA (5s)**
- *"Get the Atlas Second Brain beta — atlas-trust.dev/second-brain"*
- Waitlist signup if not yet shipped, install link if it is.

### Why this is strong

- **Visceral surprise moment** (Vignette B — Obsidian doesn't detect tampering). Most viewers have never thought about it. The demo makes them feel insecure about their current stack in 15 seconds.
- Cross-market: appeals to non-AI audiences (Obsidian power users) who are aware of AI risk but don't know what to do about it.
- Discoverable via Obsidian-related search traffic (huge SEO surface).

### Risks

- Atlas does not yet have a Second-Brain UX. **This is the demo we cannot fake — the UI must exist for the demo to feel real.**
- Obsidian community is touchy about competitor comparisons. We must be tonally generous ("Obsidian is great. Add cryptographic trust to it.") rather than dunking.
- The "tampering by teammate" framing assumes a multi-user vault, which Obsidian doesn't natively support — could feel contrived to Obsidian-savvy viewers. Alternative framing: "your machine is compromised by malware that edited your notes" — same demo, more plausible.

---

## Demo 5 — Mem0g Hybrid (Speed + Trust)

**Tagline:** *"Cryptographic trust without the speed tax."*

**Audience:** AI engineers worried about latency, agent-framework architects, technical evaluators ("yeah but is it fast enough for production?").
**Target emotion:** clarity (secondary: relief — "okay, I don't have to choose between fast and verified").
**Atlas surfaces:** Mem0g hybrid retrieval (V2-δ), atlas-web query view, atlas-trust-core (provenance pointer per result).
**Production complexity:** **3/5**
**Readiness flag:** **Requires V2-δ** (Mem0g integration). Numbers cited (1.44s vs 17.12s, 91% latency reduction) are from Mem0g's published Locomo benchmarks — they are *Mem0g's* numbers, not Atlas's. Demo must be honest that Atlas+Mem0g hybrid is a *planned* architecture and benchmarks shown reflect the upstream Mem0g capability that Atlas inherits.

### Storyboard

**Setup (10s) — Frame the world.**
- Split-screen developer terminal aesthetic. Both panes pointing at identical Atlas workspace `demo-finance-2026q1` containing ~100K signed events about financial transactions.
- **Left pane label:** `Atlas direct query (full-context, no cache)`
- **Right pane label:** `Atlas + Mem0g hybrid (verified, cached)`
- Bottom hint: *"Same data. Same workspace. Same trust property. Different retrieval path."*

**Action (25s) — Same query, two paths.**
- A developer types the same query in both panes: *"What were the top three risk factors flagged in the Q1 2026 client report for AcmeCorp?"*
- **0:00:** Both queries start.
- **Right pane (Mem0g hybrid):** Completes at **1.44s**. Results render. Each result has a small `✓ verified` glyph showing it traces back to a signed event in events.jsonl.
- **Left pane (Atlas direct):** Still running. Status: "Loading full context..." A countdown timer increments visibly. **At 17.12s** results render. Identical content.
- Side-by-side diff: results are byte-identical except for ordering metadata. Both have the same `(event_uuid, rekor_log_index)` provenance pointers per fact.
- **Bottom timing summary bar:**
  - Direct: 17.12s
  - Hybrid: 1.44s
  - Δ: **−91.6%** (Mem0g's published Locomo benchmark)

**Reveal (15s) — The trust check.**
- Cursor clicks the first result in BOTH panes simultaneously. Both open identical provenance panels showing the same signed event, same Rekor logIndex, same witness cosignatures.
- A small inline note: *"Mem0g cache is rebuildable from events.jsonl. It is never trust-authoritative. If the cache disagrees with events.jsonl, the verifier rejects."*
- A bottom-screen tagline: **"Cache for speed. Verifier for trust. Never the same thing."**

**Implication (15s) — Why this matters.**
- Full-screen overlay: *"Cryptographic memory does not have to be slow. Atlas separates **authoritative trust** (events.jsonl, signed) from **fast retrieval** (Mem0g, rebuildable). You get both."*
- Smaller: *"Benchmark numbers from Mem0g's Locomo paper (2025). Atlas+Mem0g hybrid integration is V2-δ scope."*

**CTA (5s)**
- *"Read the architecture — atlas-trust.dev/architecture"*
- Link to V2 architecture doc, especially the three-layer invariant.

### Why this is strong

- **Closes the "isn't crypto slow?" objection** in 60 seconds. This is the developer-evaluator's #1 concern.
- Shows Atlas's architectural sophistication (the three-layer model: authoritative log + queryable graph + fast cache). Engineers respect this.
- Mem0g is a real, published, third-party-validated capability. Atlas inherits credibility.

### Risks

- **The numbers are Mem0g's, not Atlas's** — we must be scrupulously honest about that. If we claim 91% latency reduction without the Mem0g attribution, that's misrepresentation.
- Mem0 is a venture-backed startup (see Risk Matrix R-Mem0-Vendor-Risk). If Mem0 changes license / pricing / availability, this demo's economics shift.
- 17-second left pane is a long time in a 60-second demo. We may need to cut to "17.12s — see comparison view" rather than show the wait in real-time.

---

## Demo Selection for Landing Page Hero

The landing-page hero needs ONE demo. Trade-offs:

| Criterion | Demo 1 (Multi-Agent Race) | Demo 2 (Regulator Witness) | Demo 3 (Agent Passport) | Demo 4 (Second Brain) | Demo 5 (Mem0g Speed) |
|---|---|---|---|---|---|
| **Emotional punch** | High — surprise + power | Medium — relief, but niche | Medium — novelty | High — discomfort + relief | Low — clarity only |
| **Audience size** | Wide (all AI engineers) | Narrow (compliance) | Narrow (deployers) | Widest (any Obsidian user) | Medium (developers) |
| **Atlas-uniqueness visible** | **Highest** — multi-agent attribution is unique | High — federation is unique | High — passport is unique | Medium — verified notes is unique-ish | Low — speed is table-stakes |
| **Readiness** | V2-α + V2-β | Mostly TODAY | V2-α + V2-γ + 30d data | V2-β + V2-γ + new UX | V2-δ |
| **30-day-to-shoot feasibility** | Possible | Possible | Hard (data collection) | Hard (UX doesn't exist) | Hard (Mem0g integration) |
| **Memorable in 10 seconds** | Yes (two colors in graph) | Yes (regulator-cosigned line) | No (passport is a page) | Yes (red tamper banner) | No (numbers blur) |

**Recommendation: Demo 1 (Multi-Agent Race) as landing-page hero, Demo 2 (Regulator Witness) as enterprise-CTA secondary.**

**Why Demo 1 wins for hero:**

1. **Visible uniqueness in <10 seconds.** Two color-coded agents writing into the same verifiable graph is a visual nobody else can show. Mem0 can't, Letta can't, Zep can't, Graphiti can't, Anthropic Memory can't. The first 10 seconds must communicate "this is different" — Demo 1 does that without any narration.
2. **Hits the target buyer.** Atlas V2 sells to AI engineers / agent builders first (Doc A's GTM Hypothesis 6a — Hermes Agent skill integration). Demo 1 speaks their vocabulary directly.
3. **Naturally validates the agent-agnostic positioning.** Demo shows Hermes + Claude working together. That single frame replaces a paragraph of "agent-agnostic" marketing copy.
4. **Easiest to make a 30-second loop of.** Multi-agent visual works on autoplay-mute (the dominant landing-page consumption mode). Demos 2/3/4/5 all require narration or text overlays to make sense without sound.
5. **Demo 2 then becomes the natural "Schedule a compliance briefing" CTA path** — different audience, different surface (enterprise contact form, not npm install).

**Hero loop spec:** 45-second silent loop, autoplay, large heading overlay "Two agents. One verified memory." Looped to ~12s before the verify-modal opens; the modal stays open for the last 8s with the "✓ VERIFIED — independently" line readable from across the room.

**Fallback hero if Demo 1 production blocks:** Demo 2 condensed to 40s. Wider enterprise CTA but narrower individual-developer pull.

---

## Production Requirements (per demo)

### Demo 1 — Multi-Agent Race

| Asset | Status | Blocker |
|---|---|---|
| Per-agent Ed25519 key support | TODAY (V1 supports per-workspace; per-agent is config-extension) | None — minor work |
| atlas-mcp-server with multiple concurrent connections | TODAY (V1.19 Welle 1) | Verify two-MCP-clients concurrent-write works |
| Hermes-Agent web UI shell embedded or recreated | NEW work | Need brand permission or recreate Hermes-Agent-style chat shell |
| Claude Desktop with Atlas MCP configured | TODAY | Need preset config file shipped as `examples/integrations/claude-desktop/` |
| Graph projection from events.jsonl → FalkorDB | **V2-α** | Whole projector crate, schema versioning, idempotent upsert |
| atlas-web `/graph` route with Cytoscape.js viewer | **V2-β** | Custom viewer with per-node color attribution + Verify button |
| Per-node "Verify" button → WASM verifier modal | **V2-β** | Wire verify-wasm into atlas-web React app, plumb provenance metadata through the projector |
| Live "two agents racing" demo script with deterministic outcome | Production design | Write the prompt, pre-record fallback for live failures |

**Blocker today:** Demo 1 cannot be filmed for real until V2-α + V2-β are at least 80% complete. **However**, a high-fidelity mocked version (real Atlas writes, mocked graph viz pre-rendered as React component) could be filmed in 1-2 sessions and would visually be 95% identical to the real version.

### Demo 2 — Continuous Audit Mode

| Asset | Status | Blocker |
|---|---|---|
| Witness federation roster (multi-witness) | TODAY (V1.14 Scope J) | None |
| Second witness key for "BaFin-witness-eu" | NEW config | Generate + commit to a demo-trust-root |
| Compliance Console UI styling | NEW UI | Atlas-web brand variant for "compliance" mode (CSS only, ~1 session) |
| Live event-stream visualization | NEW UI | Server-sent events from atlas-web to a streaming-log component |
| `atlas-verify-cli` recorded on a second monitor | TODAY | None |

**Blocker today:** Almost nothing — most of the substrate is V1 capability. UI work is ~2-3 sessions. **This is the most ship-able demo with the smallest blocking work.**

### Demo 3 — Agent Passport

| Asset | Status | Blocker |
|---|---|---|
| Per-agent Ed25519 + did:atlas resolver | **V2-α** | Architecture design for DID format + revocation chain |
| 30 days of recorded agent activity | NEW operational | Run a Hermes-Agent loop for 30 days writing into a test workspace |
| Agent passport bundle format (CBOR + signed manifest) | NEW spec | Design + crate + tests |
| `/agents/:did` page in atlas-web | **V2-γ** | New UI surface, statistics aggregation, heatmap component |
| Portable passport export endpoint | NEW API | `GET /api/atlas/agents/:did/passport.cbor` |
| Verify-wasm extended to verify passport bundles | NEW WASM API | Add `verify_passport()` function alongside `verify_trace_json()` |

**Blocker today:** Significant. **30-day lead time minimum** because the passport must show real recorded history to be credible. This is the demo we should start preparing operational data for NOW even if we don't film it for 30 days.

### Demo 4 — Verifiable Second Brain

| Asset | Status | Blocker |
|---|---|---|
| Atlas Second-Brain UX shell | **V2-γ** + new product surface | Whole new consumer-facing UX — does not currently exist |
| Multi-device sync flow | NEW infrastructure | Sync server architecture — Atlas Sustainable-Use server work |
| Tamper detection at vault-level (filesystem diff) | NEW feature | Atlas-web file-watcher + hash-mismatch detection |
| Diff view + restore flow | NEW UI | New component, fairly substantial |
| Obsidian pane for side-by-side comparison | Filming asset | Just install Obsidian; no Atlas work |

**Blocker today:** Largest. The Atlas Second-Brain product surface does not exist. **This demo describes a product that's V2-γ minimum, possibly V3 territory.** Recommend deferring this demo until we decide whether Second-Brain consumer is a primary or secondary V2 market.

### Demo 5 — Mem0g Hybrid

| Asset | Status | Blocker |
|---|---|---|
| Mem0g integration into Atlas retrieval path | **V2-δ** | Whole new layer — see Doc B §2.5 |
| Cache-invalidation logic (Mem0g rebuildable from events.jsonl) | **V2-δ** | Determinism property to specify + enforce |
| Side-by-side timing harness | NEW tooling | Simple instrumentation, ~half-session |
| Demo workspace with 100K signed events | NEW operational | Generate via `atlas-signer` example script, ~1 session |

**Blocker today:** V2-δ work. The benchmark numbers we'd cite are upstream Mem0g numbers, so we don't need to re-validate the 91% claim — we just need the integration to exist.

---

## Cross-cutting production assets needed

These are NOT per-demo but are blockers for the demo programme overall:

1. **Brand kit consistency.** Atlas does not yet have a distinct brand identity (per README TODO: 70s-style Atlas icon at `docs/assets/atlas-logo.png`). All demos will look better — and feel more coherent as a programme — once brand work is done.
2. **A consistent "verify" interaction pattern.** The verify-modal that appears across Demos 1, 2, 3, and 4 should be **identical** in layout and behavior so viewers learn it once. Design + build this single component first.
3. **A recorded "narrator voice" for video demos.** Either Nelson narrates personally (founder voice — high trust), or a neutral voice-actor. Decide before filming.
4. **Failure-mode handling.** Live demos fail. Each demo needs a pre-recorded fallback that can be cut to instantly when (not if) the live agent / network / WASM-loader misbehaves.
5. **Captions / accessibility.** All demos auto-played without sound (landing-page convention) must read clearly from caption text alone. Demo 1 passes this easily. Demo 5 needs caption rework (numbers).

---

## Open Questions for Phase 2 Critique

1. **Q: Are these demos honest about current Atlas capabilities?** Context: only Demo 2 is mostly shipable today. Four of five demos require V2-α/β/γ/δ work that hasn't started. **Status: open.** Phase 2 critique should challenge whether we should be drafting demo storyboards at all this early, or whether this is premature optimization of the marketing surface.

2. **Q: Is the multi-agent race emotionally compelling enough to lead with?** Context: alternative framings exist (collaboration, agent + human-reviewer, specialist agents). The "race" framing might trigger "gimmick" reactions from senior audiences. **Status: open.**

3. **Q: Should we have a "non-AI" demo (Demo 4 — Second Brain) at all?** Context: Second-Brain is V2's secondary market per Doc A's two-market positioning. Resource-allocation question — should we spread thin across two markets in launch demos, or stake everything on the agent-builder audience? **Status: open.**

4. **Q: Should Demo 2 use a fictional "BaFin witness" or a real federation participant we've actually onboarded?** Context: fictional regulator demos can read as vaporware if not labeled carefully. Onboarding even one real witness-operator before filming would be a material credibility upgrade. **Status: open.**

5. **Q: Are 30/60/90 second demo lengths right, or should we go ultra-short (15s GIF loop) for landing?** Context: landing-page conversion data from comparable B2B tech sites suggests 15-30s hero videos outperform 60s+. But Atlas's value is structural — viewers need to *understand* the verifier moment, not just see it. **Status: open.**

6. **Q: What's the right voice / register for narration?** Context: Atlas's positioning straddles serious compliance and developer-builder energy. A compliance-officer voice ("EU AI Act Article 12 mandates...") is too dry for AI engineers; a developer-energy voice ("watch this — two agents racing!") is too casual for CCOs. **Status: open.**

7. **Q: Should agent-passport (Demo 3) come BEFORE multi-agent-race (Demo 1) in the demo programme?** Context: agent passports are the **conceptual prerequisite** — Demo 1 only makes sense after you understand "each agent has its own key". Maybe Demo 3 should be the conceptual hero and Demo 1 the technical hero. **Status: open.**

8. **Q: Are we underweighting the verifier-as-product narrative?** Context: Atlas's V1 is the verifier itself (Apache-2.0, 347 tests, SLSA L3). None of the five demos foreground the verifier as the protagonist. Is there a sixth demo — "the verifier as customer's-own-tool" — that should exist? **Status: open.**

9. **Q: How honest should we be about V2 readiness in the demos themselves?** Context: putting "Requires V2-β — coming Q4 2026" on the demo frame is honest but kills landing-page conversion. Putting nothing reads as overclaim. Is there a third option (e.g., "Atlas roadmap →" link with no demo-level warning)? **Status: open.**

10. **Q: Should the hero demo be filmed live or hand-animated?** Context: live recording (real Atlas, real agents) is more credible but harder to make 100% reliable for a 45-second loop. Hand-animated (After Effects / Lottie) is reliable but reads as "marketing" to engineer audiences. Hybrid: real agent traces, animated graph re-rendering of those traces — credible AND reliable. **Status: open.**

11. **Q: Is Demo 5 (Mem0g speed) too defensive?** Context: it directly addresses "but is it slow?" — implicitly conceding that the worry exists. Some founders / brand consultants would argue we should not address objections in demos at all, instead controlling the narrative around our strengths. **Status: open.**

12. **Q: Should we have a "live regulator endorsement" demo?** Context: if even one regulator publicly endorses Atlas before launch, a 30-second demo of that endorsement is worth more than all five demos combined. This is a sales / BD question, not a demo-design question, but it affects demo prioritization. **Status: open.**

13. **Q: Are we covering the "developer setup" experience?** Context: none of the five demos shows the 30-second "npm install @atlas-trust/verify-wasm + one-line integration" experience. For the developer-audience segment, this might be the most important conversion driver. **Status: open.**

14. **Q: Should Demo 4 (Second Brain) explicitly position against Anthropic Memory / OpenAI Memory instead of Obsidian?** Context: Obsidian users are aware of trust but happy with their setup. AI-Memory users are increasingly worried about vendor lock-in. A "verifiable cross-vendor AI memory" framing might convert better than "trustable Second Brain". **Status: open.**

15. **Q: Are demo runtimes optimized for the right platform?** Context: a 60s demo for the website is wrong length for X/Twitter (15s autoplay), LinkedIn (30s), YouTube (90-180s), or a sales deck (could be 3 minutes). Should we plan demos per-platform from the start, or master one and trim? **Status: open.**

---

**Doc owner:** Nelson + Atlas team. **Last edited:** 2026-05-12. **Next milestone:** Phase 2 critique pass — product / brand / strategy agents annotate per demo and per Open Question, then converge to a "demo programme spec" in Phase 3.
