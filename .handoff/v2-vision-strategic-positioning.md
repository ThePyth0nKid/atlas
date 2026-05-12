# Atlas V2 — Strategic Positioning Vision

> **Status:** Phase 1 Doc A v0 (Foundation Document). Erstellt 2026-05-12 im isolated worktree `docs/v2/phase-1-doc-A-positioning`. Subject to Phase 2 critique (product / business / strategy reviewers) before any synthesis. Sister-Docs in flight: Doc B (technical architecture), Doc C (risk matrix), Doc D (competitive landscape), Doc E (demo sketches). Cross-file convergence happens in Phase 3 per `.handoff/v2-iteration-framework.md`.
>
> **Verwandtschaft zu V1:** Dieses Doc setzt voraus, dass V1.0.1 SHIPPED ist (`@atlas-trust/verify-wasm@1.0.1` LIVE auf npm, SLSA Build L3 provenance, Sigstore Rekor logIndex `1518327130`, signed Git tag `v1.0.1` SSH-Ed25519 — siehe `.handoff/v1.19-handoff.md` §0). V1 ist eine in-sich geschlossene Auslieferung; V2 ist ein Pivot der Adoption-Story, nicht ein Bruch der Trust-Property. Jede V2-Aussage in diesem Doc muss die V1-Trust-Invariante respektieren: events.jsonl bleibt authoritative, jede V2-Schicht ist deterministisch rebuildable Projection.
>
> **Stil-Konvention** (gespiegelt aus v1.19-handoff.md): German für Headers + kurze Prosa, English für technical terminology, file paths, citations, code identifiers. Eine "Welle" ist eine in-sich geschlossene Auslieferung (Plan-Doc → Implementation → Review → Ship). V2-α, V2-β, V2-γ sind grobe Phasen, definiert in Doc B.

---

## 1. The Pivot (was V1 war, was V2 wird)

### 1.1 Worauf V1 strukturell geantwortet hat

Atlas V1 (V1.0 → V1.19, ~14 Monate von V1.5 Anchoring bis V1.19 Welle 14a npm publish) hatte eine sehr enge Mission: **eine einzige Compliance-Lücke strukturell schließen, nämlich EU AI Act Article 12 "automatic recording of events, independently verifiable"**. V1 hat das geschafft. Das Beweisstück steht im Repository:

- **Cryptographic integrity** — Ed25519 + COSE_Sign1, hash-chained edges, byte-deterministic CBOR signing-input (`crates/atlas-trust-core/src/wire.rs`)
- **Independent verifiability** — Offline WASM verifier (`@atlas-trust/verify-wasm`, ~150KB), der im Browser des Auditors läuft ohne Netzwerk-Call zu unserem Server. Same Rust core, same byte-output, Linux / macOS / Windows-MSVC / Cloudflare Workers
- **Tamper-evident anchoring** — Sigstore Rekor v1 mit RFC 6962 Merkle inclusion proofs gegen pinned log pubkey
- **Witness cosignature** — Out-of-process Witness mit separater Ed25519-Key (`crates/atlas-witness`), Trust-Domain-Trennung by process
- **Hardware-backed signing** — PKCS#11/HSM (V1.10 Wave-2, V1.11 Wave-3), `CKM_EDDSA`, sealed key material, `Sensitive=true, Extractable=false, Derive=false`
- **Supply-chain provenance** — SLSA Build L3 OIDC + Sigstore (V1.19 Welle 14a: `npm audit signatures` verifies registry + attestation)
- **Tag-signing enforcement** — Jeder `v*` tag SSH-Ed25519 signed gegen in-repo trust root `.github/allowed_signers`

V1 ist damit der **strukturell verifizierbare Audit-Log-Layer** für AI-Systeme. Es ist kein Dashboard. Es ist eine Substanz, die jeder Auditor — Regulator, Insurance, internal compliance — unabhängig nachbauen kann.

Aber: V1 ist *nur* ein Audit-Log-Layer. V1 hat keinen Query-Surface. V1 ist keine Wissensbasis. V1 hat keine Agent-Identitäten. V1 hat kein Policy-Engine-Enforcement. V1 ist die Trust-Substanz — aber Trust-Substanz allein verkauft sich nicht außer an Compliance-Officers in regulierten Industrien.

### 1.2 Was V2 strukturell aufmacht

V2 ist **kein neuer Trust-Layer**. V2 ist das Aufmachen der Substanz für einen viel größeren Adressraum, ohne die V1-Trust-Property zu schwächen. Drei strukturelle Verschiebungen:

**(1) Agent-agnostic shared substrate.** V1 hatte einen MCP-Server (V1.19 Welle 1, `apps/atlas-mcp-server/`), der die Write-Surface universell für jeden MCP-kompatiblen Host gemacht hat. V2 baut das aus zu einer **gemeinsamen verifizierbaren Wissensbasis, in die jeder AI-Agent schreiben kann — Hermes Agent (Nous Research), Claude (via MCP), GPT (via OpenAI's MCP-Bridge), Llama-basierte custom Agents, und menschliche Operator-CLI-Tools — und alle Reads gehen über dieselbe verifizierbare Trust-Substanz**.

Wir bauen keinen Agent. Wir bauen das Substrat, das jeder Agent benutzen kann. Das ist nicht "noch ein Mem0". Das ist "die Verification-Schicht UNTER Mem0, Letta, Zep, oder jeder anderen Memory-Implementation".

**(2) Verifiable Second Brain als neue Marktkategorie.** Obsidian, Notion, Roam Research, Logseq, Capacities, Tana, Heptabase — die ganze "Personal Knowledge Management"-Kategorie hat einen blinden Fleck: sie sind alle plain-text markdown / proprietary database, ohne kryptografische Trust-Property. Für den Solo-Knowledge-Worker mit nicht-AI-relevantem Use-Case ist das okay. Für die nächste Welle ("ich lasse meinen AI-Agent in meinem Second Brain arbeiten, ich teile mein Second Brain mit meinem Team / meiner Beraterin / meinem Compliance-Auditor") ist plain-text markdown strukturell ungeeignet.

Atlas V2 zielt auf die Verifiable-Second-Brain-Kategorie: cryptographic trust als strukturelles Feature, nicht als Add-on. Jede Notiz signed. Jedes Edit hash-chained. Jeder externe Reader kann verifizieren, dass die Notiz vom angegebenen Author / Agent stammt und nicht im Transit oder im Storage manipuliert wurde.

**(3) Cross-vendor Trust-Substrate für die Open-Weight-Welle.** Anthropic Memory ist Claude-only. OpenAI Memory ist ChatGPT-only. Beide sind Vendor-Silos: closed, nicht cross-vendor verifiable, und das Geschäftsmodell zwingt sie strukturell zur Vendor-Capture. Die wachsende Bewegung in Richtung **Open-Weight-Modelle** (Hermes Agent #1 auf OpenRouter seit 2026-05-10, Llama 3+/4, Mistral, DeepSeek, Qwen) hat aber kein Memory-Equivalent — und genau hier sitzt das Greenfield: ein cross-vendor verifiable Memory Substrate, vendor-neutral, der nicht durch Anthropic / OpenAI "owned" werden kann.

### 1.3 Tagline-Kandidaten

Tagline ist eine 8-12 word strukturelle Behauptung, die V2 differenziert. Phase 2 Critique soll diese gegeneinander testen.

1. **"The verifiable substrate for AI memory and human Second Brains."**
   *Pro:* explicit zwei-Markt, "substrate" macht clear dass Atlas Infrastruktur ist nicht App. *Con:* "verifiable" ist abstrakt für non-technical audience.

2. **"Knowledge your AI can prove, not just claim."**
   *Pro:* sehr klar, sehr kurz, "prove vs claim" ist die Atlas-Differenzierung in einem Satz. *Con:* fokussiert auf AI-Agent-Case, lässt Second-Brain-Markt im Schatten.

3. **"Cryptographic Second Brain for the agent era — trust is structural, not transactional."**
   *Pro:* positioniert in der Obsidian-Kategorie + macht clear dass es um die AI-Era geht. *Con:* "cryptographic" schreckt vielleicht ab.

4. **"The TÜV-Plakette for AI agent memory."** (German-rooted)
   *Pro:* sehr griffig im EU-Kontext, "TÜV" ist universal-verständlich in DACH, expandable als "AI compliance seal". *Con:* TÜV-Metapher übersetzt schlecht ins Englische ("MOT", "inspection sticker", "compliance certificate" — alle weniger griffig).

5. **"Every fact has an author. Every author is verifiable. Every read is auditable."**
   *Pro:* dreigeteilt, rhythmisch, jede Hälfte adressiert eine andere Persona (engineer / compliance / auditor). *Con:* 3 Zeilen ist eher Sub-Headline als Tagline.

6. **"Where AI agents write knowledge that holds up in court."**
   *Pro:* legal-grounded, evokes EU AI Liability Directive. *Con:* "in court" ist sehr Compliance-zentriert; for engineers / second-brain users zu nieche.

**Empfehlung für Phase 2:** Kandidaten 1, 2, 3 sind die "neutral and broad" Optionen; 4 ist ein European-DACH-spezifischer Hero-Hook für regulierte Industrien; 5 ist Sub-Headline-Material. Kandidat 6 nur falls Compliance-vertical erste Adoption-Welle ist (siehe §6.2). Final tagline ist eine **business / GTM decision** — nicht ein engineering decision.

---

## 2. Two-Market Positioning

V2 spielt explizit in zwei Märkten gleichzeitig. Das ist riskant (zwei Narrative parallel zu kommunizieren ist hart), aber strukturell richtig, weil **dasselbe Substrat beiden dient**. Die Trust-Property ist die Brücke.

### 2.1 Verifiable Second Brain (Obsidian/Notion-Kategorie + crypto trust)

**Markt:** Personal Knowledge Management / Second Brain Tools. Aktuelle Player: Obsidian (estimated ~1M+ aktive User basierend auf Forum-Aktivität und Plugin-Downloads — Phase-2 sollte das via WebSearch verifizieren), Notion (40M+ users claimed durch Notion AG, Stand 2024), Roam Research, Logseq, Capacities, Tana, Heptabase. Markt-Größe für PKM-Tools ist niedriger als Productivity (Notion / Office365), aber growth-rate ist hoch in der Knowledge-Worker-/-Researcher-/-Indie-Maker-Demographie.

**Target Persona:**
- **Researcher mit AI-Assist** — schreibt Notizen, hat Claude / Hermes Agent / ChatGPT als Co-Pilot, will am Ende des Projekts nachweisen können was *sie* geschrieben hat und was die AI generiert hat. Plain Obsidian kann das nicht beweisen.
- **Knowledge-Team in regulated Field** — Anwaltskanzlei, Beratung, Wissenschaftler, Compliance-Consultants — teilt Notizen-Vault mit Klient*innen, muss zeigen können dass nichts im Transit manipuliert wurde.
- **Indie Maker / Solopreneur, der seinen "wissens-graph" als CV verkauft** — verifizierbares Portfolio of Thinking, nicht nur ein selbst-geschriebenes Markdown-File.
- **Privacy-conscious Power-User** — will Local-First + cryptographic-tamper-evidence + optional public anchor; lehnt vendor-locked cloud-PKM (Notion) ab, will aber mehr als plain-text Markdown.

**Why this market works for Atlas:** Obsidian's Wachstum (free local-first + paid Sync $5/mo / paid Publish $10/mo) zeigt dass es eine reife Nachfrage nach Local-First-First-PKM mit optional-paid-Cloud-Layer gibt. Atlas könnte direkt in diese Demo-Mechanik einkoppeln: Verifier ist Apache-2.0 + lokal, Server-Sync ist hosted-paid. Open-Core-Modell ist beweisbar funktionsfähig in dieser Demographie.

**Was Atlas anbieten würde (V2-α/β Scope):**
- Atlas-as-Vault — markdown-kompatible directory structure ABER mit events.jsonl als Authoritative-Layer
- Obsidian-Plugin-Adapter, der Edits durch atlas-mcp-server schickt (Plugin liest read-side via Atlas Read-API)
- "Verified by Atlas" badge auf öffentlich-shared notes (z.B. "Mein Newsletter ist signed-by-this-key")
- Multi-Author-Support mit per-Author Ed25519-Identity (kein Plain-Text-Username)

**Open question — siehe §8 — :** Ist die Verifiable-Second-Brain-Kategorie ein echter Markt (Bedarf existiert, kaufbereitschaft existiert) oder eine **aspirational category** (Bedarf wird gesehen wenn das Produkt da ist, aber nicht aktiv gesucht)? Phase 2 muss das stresstesten.

### 2.2 Multi-Agent Shared Memory (Hermes / Claude / GPT / Llama / custom alle koppeln rein)

**Markt:** AI agent memory infrastructure. Aktuelle Player: Mem0 (Y-Combinator, ~$5M seed in 2024), Letta (formerly MemGPT, UC Berkeley spinout), Zep (and ihr open-source Graphiti-Framework), Anthropic Memory (Claude-native, since 2025), OpenAI Memory (ChatGPT-native). Plus die kommende Welle: open-weight agent stacks (Hermes Agent, LangChain mit custom Memory, AutoGen, etc.).

**Target Persona:**
- **AI Engineer / Agent Builder** — baut Multi-Agent-Systeme, hat das Problem dass Agent A schreibt etwas → Agent B liest es → unklar, ob B den genuinen Output von A liest oder einen Halluzinationsschatten. Will strukturell wissen "this fact has author X, written at time T, signed".
- **AI Startup mit B2B-Customers in regulierten Verticals** — verkauft AI-Lösungen in Finance / Health / Legal — Customer fragt "kannst du beweisen, dass dein Agent das wirklich gesagt hat?". Atlas ist die Antwort.
- **Multi-Tenant AI Platform** — hosted AI-Agent-Marketplace (Hermes-style, oder Hugging Face Spaces, oder LangServe-hosted), braucht cross-tenant trust property so dass Tenant A nicht Tenant B's Memory tampern kann.
- **Enterprise IT mit Vendor-Diversification-Mandate** — will nicht in Anthropic-Lock oder OpenAI-Lock, sucht vendor-neutral Memory Layer der mit verschiedenen LLM-Backends arbeitet.

**Why this market works for Atlas:** Memory-Infrastruktur ist die heißeste AI-Infra-Kategorie 2025-2026 (Mem0 Series A, Letta funded, Zep raising). Aber: alle aktuellen Player haben *keine* kryptografische Trust-Property. Die Memory ist "trust by company". Atlas's Pitch ist orthogonal — wir sind nicht "ein besseres Mem0", wir sind "die Trust-Schicht UNTER Mem0". Mem0g (graph-enhanced Mem0, claimed 91% p95 latency reduction, <5pt accuracy gap vs full-context, 2.59s p95 — siehe Doc B für tech details + Doc D für source verification) kann auf Atlas events.jsonl projektieren; Letta agent state kann signed events emittieren; Zep's Graphiti supports FalkorDB backend — passt zu Atlas's V2-α stack.

**Was Atlas anbieten würde (V2-β/γ Scope):**
- atlas-mcp-server expanded mit query-tools (`query_graph`, `query_entities`, `query_provenance`, `get_agent_passport`) — siehe Doc B §2.9
- Agent Passport API — Ed25519-keypair pro Agent-instance, verifiable write history, portable across deployments
- FalkorDB-Projection + Mem0g-Cache hybrid (siehe Doc B §2.5) — schnelle Retrieval ohne den Trust-Layer zu schwächen
- Multi-tenant federation: per-workspace HKDF-derived keys (already V1.9), cross-workspace queries gated by Cedar policies

### 2.3 Warum beide Märkte denselben Substrat brauchen

Das ist die strategisch wichtigste Behauptung dieses Docs: **dasselbe Substrat dient beiden Märkten, weil die Trust-Property strukturell ist, nicht use-case-spezifisch**.

Konkret: ein menschlicher Knowledge-Worker, der eine Notiz signed mit seinem Ed25519-Key, ist *strukturell dasselbe* wie ein AI-Agent, der einen Fact signed mit seinem Ed25519-Key. Beide sind "ein Identity, ein signed event, eine hash-chain, ein anchor zu Sigstore Rekor". Der einzige Unterschied ist der Identity-Type (`human:nelson@ultranova.io` vs `agent:hermes-instance-xyz`) — und das ist nur ein String im Identity-Field.

Das hat zwei Konsequenzen:

**(1) Engineering economy of scale.** Atlas baut nicht zwei Produkte. Atlas baut ein Substrat + zwei distinct GTM-Narratives + zwei distinct UI-Surfaces. Verifier-Crate ist identical für beide. atlas-mcp-server ist identical. Nur die Wrapper-Apps unterscheiden sich (atlas-web-personal für Second Brain UX, atlas-mcp-server-agent für Agent-API).

**(2) Cross-market viral expansion.** Ein menschlicher Researcher mit Atlas-Vault, der seinen AI-Agent dazuholst → Agent schreibt automatisch in dasselbe Vault, dasselbe Trust-Format, derselbe Verifier. Kein Adapter-Layer. Kein Vendor-Boundary. Das ist die *Story*: "your Second Brain and your AI's Memory share the same verifiable substrate, by design".

**Caveat (siehe §7):** Zwei-Markt-Positioning ist auch zwei-Mal Risiko. Wenn weder Markt sich entscheidet zu adoptieren, sind wir doppelt-niemand. Marketing-Disziplin: jede einzelne Kommunikation wählt EINEN Markt als Primary-Story, der andere als "by the way ...". Phase 2 muss entscheiden welcher Markt initial-primär ist (vermutlich Multi-Agent, weil EU AI Act drives Compliance-buying, aber Phase-2 should challenge this).

---

## 3. EU AI Act Structural Fit

### 3.1 Artikel-Mapping zur V1+V2-Substanz

V1 hat das Compliance-Mapping bereits in [`docs/COMPLIANCE-MAPPING.md`](../docs/COMPLIANCE-MAPPING.md) angesprochen (EU AI Act Annex IV §1(e), GAMP 5, ICH E6(R3), DORA, GDPR Art. 22). Die folgende Tabelle ist die V2-Verdichtung, mit Fokus auf die Article-12/13/14/18-Achse, weil das die Compliance-Treiber ist die EU-regulated Verticals zum Buying zwingen.

| EU AI Act Artikel | Compliance-Anforderung | Atlas V1 Provides | Atlas V2 Adds |
|---|---|---|---|
| **Art. 12** (in Kraft 2026-08-02) | Automatic event logs, independently verifiable über die Lebenszeit des Systems | Signed events.jsonl + Sigstore Rekor anchoring (V1.5–V1.7) + offline WASM verifier — auditor needs no trust in our server | Cross-agent query API: Auditor can query "show me every fact this agent ever wrote, with provenance" |
| **Art. 13** (Transparency to Deployer) | Provider must give Deployer "transparent information" about system behavior | V1.0 verifier output `VerifyOutcome` ist self-explanatory: 7-check baseline + strict-mode flags | V2 Read-API (Doc B §2.8) lets Deployer query "what did this agent do for this user in this session" with cryptographic answer |
| **Art. 14** (Human Oversight) | High-risk systems require human-in-the-loop with effective intervention capability | V1 doesn't directly enforce oversight; events-log just records | V2 **Cedar policy enforcement at write-time** (Doc B §2.X): policies REQUIRE human-cosign for sensitive event-types — pre-action enforcement, not just post-hoc audit |
| **Art. 18** (Record retention, 6 months minimum) | Retain automatic logs for 6 months after creation | V1 events.jsonl is append-only, no built-in retention policy | V2 retention strategy: events.jsonl never deleted (signature integrity preserved), but raw content separable into deletable storage — preserves the trust property post-deletion ("hash exists, content nullable = redacted but verified existed at T") |

**Quellenanmerkung:** Die exakte Auslegung dieser Artikel entwickelt sich. EU AI Act Annex IV §1(e) ist die konkreteste Vorgabe für "log records". Atlas's V1 README listet zusätzlich GAMP 5, ICH E6(R3), DORA Art. 11-14, und GDPR Art. 22 — siehe [`docs/COMPLIANCE-MAPPING.md`](../docs/COMPLIANCE-MAPPING.md) für die clause-by-clause Mapping. Phase 2 should verify with a Compliance-Officer / EU AI Act counsel that these mappings hold under formal legal interpretation.

### 3.2 EU AI Liability Directive (proposed, expected 2026 enactment)

Wichtiger als Art. 12 selbst ist die proposed **EU AI Liability Directive** (im Council-Phase 2025/2026, expected enactment 2026 — Phase 2 should verify current status via WebSearch). Die Direktive verschiebt die Beweislast: bei AI-induced harm muss der Provider nachweisen, dass das System ordnungsgemäß gehandhabt wurde. Wenn das nicht beweisbar ist, geht Haftung auf den Provider.

**Atlas's strukturelle Antwort:** signed events.jsonl + Sigstore Rekor anchor + Witness cosignature = ein cryptographically verifiable Audit-Trail, den der Provider in einer Liability-Klage als Beweis vorlegen kann. Der Auditor (Gericht / Gutachter / Versicherer) kann den Trail unabhängig verifizieren ohne uns / unsere Software / unsere Server zu vertrauen. Das ist nicht "wir machen Haftung weg"; das ist "wenn die Frage 'was ist passiert' kommt, ist die Antwort byte-deterministic und mathematisch beweisbar".

**Implikation für GTM:** AI Liability ist möglicherweise stärkerer Buying-Driver als Art. 12 selbst, weil Haftungsversicherer aktiv nach risk-pricing-friendly Substraten suchen (siehe §4.2). Phase 2 should explore: ist AI-Liability-Insurance-pricing eine eigene GTM-Säule (siehe §4.2 + §6.2)?

### 3.3 Was V2 *nicht* tut

- Atlas V2 ist **kein** Risk-Classification-Tool (high-risk vs limited-risk system Klassifizierung — das ist eine Provider-Pflicht, kein Trust-Substrat-Problem)
- Atlas V2 ist **kein** Conformity-Assessment-Tool (Annex VI / VII checklists — das ist eine separate Compliance-Workflow-Anwendung)
- Atlas V2 ist **kein** Data Governance Layer im Annex IV §3 Sinne (data quality, training-data-governance, das ist upstream of where Atlas operates)

Atlas's Scope ist **runtime event-recording + provenance + read-side query**. Phase 2 Critique soll bewerten ob dieser Scope kommerziell ausreichend ist oder ob wir uns selbst zu eng definieren.

---

## 4. New Trust-Modes Atlas Enables (genuinely novel — nicht nur Compliance)

Das ist der Abschnitt, in dem Atlas's structurelle Position **echte neue Trust-Modes ermöglicht, die kein bestehender Compliance-Vendor anbietet**. Wenn Atlas nur "EU AI Act Compliance Vendor Nummer 47" wäre, würden wir verlieren — das ist commoditisable. Stattdessen sind das genuin neue Trust-Patterns die nur durch das verifiable substrate möglich werden.

### 4.1 Continuous Regulator Attestation

**Status quo:** Regulatorische Aufsicht ist heute periodisch. Audit kommt einmal im Jahr, schaut sich Logs an, gibt grünes / rotes Licht. Zwischen Audits weiß die Aufsicht nichts.

**Atlas's Innovation:** **Aufsichts-Witness-Key wird in die Trust-Root des Provider-Workspace federiert.** Der Regulator hat einen Ed25519-Key. Dieser Key cosignt jeden Chain-Head des regulated Provider-Workspaces in Echtzeit. Trust-Domain-Trennung durch Process (V1's `atlas-witness` already implements this pattern — siehe `crates/atlas-witness`).

Operational: der Provider-Workspace fires bei jedem N-tem Chain-Update eine HTTP-Request an die Aufsichts-Witness-API. Witness signs den Head, returns die cosignature. Provider integriert sie in den nächsten Anchor-Batch. Wenn Provider die Aufsicht aussperrt (Witness-API-Calls werden geblockt), wird der Verifier-Output strict-mode-flag-able: "die letzten N Tage haben keine Regulator-Cosignature; nicht-conform mit Continuous-Audit-Mode".

**Warum kein anderer das macht:** ein traditioneller Compliance-Vendor (Drata, Vanta, OneTrust) kann das nicht anbieten, weil ihre Architektur Trust-by-Vendor-API ist. Ein Regulator kann nicht "die Drata-API teilweise trauen". Atlas's Trust-by-Cryptography macht das partial-trust-relationship strukturell möglich.

**Implikation für Atlas's Business:** wir sind nicht der Regulator und wollen das nicht sein. Wir sind das Substrat, das Regulator + Provider gemeinsam benutzen können. Das ist eine sehr defensible Position weil neither side benefitiert davon, uns rauszunehmen.

**Demo:** siehe Doc E §2 — "Continuous Audit Mode (Regulator Witness)".

### 4.2 AI-Liability-Insurance Pricing Substrate

**Status quo:** AI-Liability-Insurance ist eine entstehende Kategorie (Munich Re, Swiss Re, Hartford, AIG haben 2025-2026 erste AI-spezifische Policies gelauncht). Pricing ist heute **Pauschalprämie based on stated controls** (provider sagt "we have X, Y, Z controls", insurer charges base rate). Es gibt keinen objektiven Mechanismus, um clean-claims-history zu verifizieren.

**Atlas's Innovation:** **Atlas-attested Events ergeben einen mathematisch beweisbaren Claims-Substanz-Datensatz.** Ein Versicherer kann die Verifizier-Outputs eines Provider-Workspaces lesen (read-side API, no trust in provider needed), und basierend darauf differenzierte Prämien berechnen.

Konkret: zwei Provider mit gleich-skalierten AI-Systemen. Provider A: Atlas-attested, 12 Monate ohne strict-mode-Verletzung, full-policy-compliance-rate 99.8%. Provider B: same scale, traditional logging, can't prove anything. Versicherer kann Provider A einen 30-50% Rabatt geben weil das Risk-Pricing mathematisch beweisbar besser ist.

**Warum kein anderer das macht:** Insurance-Risikomodellierung braucht **mathematische Beweisbarkeit**, nicht "vendor sagt es ist gut". Atlas's offline-verifizierbar-byte-deterministisch Property ist genau der Substrat, den Versicherer für differenzierte Pricing brauchen.

**Implikation für Atlas's Business:** dreigeteilte Wert-Proposition für regulated Provider: (a) Compliance (EU AI Act), (b) operational excellence (continuous regulator audit), (c) **insurance premium reduction**. (c) ist quantifizierbar in Euro — vermutlich der stärkste ROI-Hebel.

**Open Question:** welche Versicherer sind ready für diese Architektur? Phase 2 should sondieren: Munich Re? Hartford? Phase 2 sollte mit jemandem aus der AI-Insurance-Welt sprechen können (Nelson's Netzwerk-Frage).

### 4.3 Agent Passports — Ed25519-Keypair = verifiable Agent Identity + Reputation

**Status quo:** Agent identity ist heute "API-Key in Vendor-Dashboard". Mein Claude-API-Account-Key. Mein OpenAI-Project-API-Key. Keine portable Reputation. Wenn ich von Anthropic zu Hermes Agent wechsle, verliere ich die "track record" of facts that agent wrote.

**Atlas's Innovation:** **jeder Agent hat eine Ed25519-Identität (`did:atlas:<pubkey-hash>`), unter der seine kompletten verifizierten Writes anfallen.** Ein Hermes-Instance schreibt 3 Monate lang Facts in einen Workspace; alle signed mit seinem Agent-Key. Wechselt der Operator den Hermes-Instance auf ein anderes Deployment, wandert der Key mit — der Agent's Reputation reist mit.

Operational features:
- Per-Agent verifiable Stats: "wrote 1247 facts, 0 retractions, 12 unique witness cosigners across 3 organizations"
- Agent Marketplace-Capability: ein Hiring-Person kann einen Agent's history einsehen und entscheiden ob sie ihm vertraut
- Cross-org agent portability: derselbe Agent-Key wird in mehreren Organizations adopted; jede Organization sieht seine kumulative reputation
- Revocation-chain: wenn ein Agent compromised wird, gibt es einen Revocation-Event mit timestamp; downstream queries können "facts written BEFORE compromise" als trusted-noch behandeln

**Warum kein anderer das macht:** vendor-locked memory (Anthropic / OpenAI) **kann das strukturell nicht anbieten**, weil ihre Architektur reputation = vendor-account = silo ist. W3C DID-Spec existiert seit 2022, aber kein AI-Memory-Vendor hat es integriert (zu wenig Bedarf in vendor-locked context). Atlas's vendor-neutral position macht das natural.

**Implikation für Atlas's Business:** Agent Passports öffnen eine **Marketplace-Kategorie** — long-term das größte structural potential. Wenn AI-Agents commodities werden (zunehmend wahrscheinlich), wird Reputation die Differenzierung. Atlas kann der Reputation-Standard werden.

**Caveat:** das ist eine Wette auf einen sich entwickelnden Markt. Im 2026 ist Agent-Marketplace-Kategorie zu neu für hard data. Phase 2 should challenge: wie real ist diese Marketplace-Welle?

### 4.4 Pre-Action Policy Enforcement via Cedar at Write-Time

**Status quo:** Compliance-Engines sind heute fast immer **post-hoc** ("after the fact, did this comply?"). Pre-action enforcement ist hart, weil typische Vendor-Architecture nicht in den Write-Path eingreift.

**Atlas's Innovation:** **Cedar policy enforcement at write-time.** events.jsonl write-path geht durch atlas-mcp-server. atlas-mcp-server kann Cedar policies konsultieren, die im Workspace selbst gespeichert sind (signed by workspace owner, V1.0 already has `policies[]` field in trace_format). Wenn die Policy "writes von kind `financial-recommendation` requirieren human-cosign within 5 minutes" sagt, lehnt der Server die Write ab oder hält sie pending bis cosign arrives.

V1 hat die *Schema-Capability* dafür (Cedar policies als event_id list im Trace-Format), V2 implementiert die *Engine* (Cedar runtime im atlas-mcp-server). Siehe Doc B für die Architektur-Detail.

**Warum kein anderer das macht:** Cedar (AWS open source policy language, June 2023) ist ein junger Standard. AWS hat es für IAM gepusht, aber kaum jemand baut es in den Write-Path von AI-Memory ein. Atlas's V1-Foundation (events.jsonl mit policies[]-field) ist perfekt vorbereitet dafür.

**Implikation für Atlas's Business:** Compliance ist **strukturell** nicht **auditiert**. Das eliminiert die ganze Audit-Workflow-Category. EU AI Act Art. 14 (Human Oversight) wird strukturell erfüllt statt nachweis-pflichtig.

### 4.5 AI Bill of Materials (AI-BOM) Substrate

**Status quo:** Software Bill of Materials (SBOM) ist seit 2021 mandatory in regulierten Deployments (US Executive Order 14028, EU NIS2). AI Bill of Materials (AI-BOM) ist die natürliche Evolution: "welche Model-Weights, welche Training-Data-Lineage, welche Fine-Tunes, welche Tools, welche Prompts wurden in diesem AI-System used?". 2025-2026 ist die Standardisation noch nicht abgeschlossen (CycloneDX hat ein AI-Extension; SPDX hat AI/ML profile; beide draft).

**Atlas's Innovation:** signed events.jsonl mit Schema-Erweiterung für **AI-BOM Events**:
- `model:<hash>` — Identity of the model that produced an output
- `training-data:<merkle-root>` — Lineage to training-data set (referenced by hash, not stored locally)
- `fine-tune:<delta-hash>` — Fine-tune delta applied on top of base model
- `prompt:<canonical-hash>` — Prompt-template that generated the output

Jeder Inference-Event linkt zu seinem AI-BOM-Substrat. Auditor can verify "this output was produced by model X with prompt Y on training data set Z, all cryptographically attested".

**Warum kein anderer das macht:** AI-BOM ist eine emerging Standardisation. Atlas's events.jsonl wire-format ist **per Konstruktion extendable** (V1.0 schema allows new fields via `#[serde(default)]`). Atlas kann als Substrat für AI-BOM auftreten *bevor* die Standardisation final ist.

**Implikation für Atlas's Business:** AI-BOM compliance wird die nächste Compliance-Welle nach EU AI Act Art. 12. Atlas, der bereits das Substrat liefert, sitzt da strategisch perfekt. Aber: das ist 2027+ Timing. Phase 2 should bewerten ob wir das **jetzt** positionieren oder erst 2027.

### 4.6 B2B Cross-Organization Trust Patterns

**Status quo:** Cross-org data sharing ist heute "Kontrakt + VPN + API-Key". Compliance-Aufwand ist enorm. Vertrauensbedarf hoch. Kein Mechanismus für post-hoc "did the data flow correctly?" außer Eigenrecherche.

**Atlas's Innovation:** **Federation-Tier zwischen Atlas-Workspaces.** Org A hat Workspace W_A, Org B hat W_B. Beide haben ihre eigenen Trust-Roots (pubkey-bundles). Sie können ein **federated subset** definieren: bestimmte event-kinds in W_A werden auch in W_B verifiable, weil B's verifier W_A's pubkey-bundle hash hat. Cross-witness Tests: B's witness signs A's chain-heads für die federated event-kinds. Wenn A versucht etwas zu fälschen, ist die cross-witness Anomalie sofort visible für B.

Trust-Domain bleibt clean — A trusts B nicht implizit, B trusts A nicht implizit. Aber sie können **selektiv** Trust etablieren für definierte event-kinds.

**Warum kein anderer das macht:** API-based data sharing kann das nicht — die Trust-Property hängt strukturell an den signed events, nicht an API-Auth.

**Use Cases:**
- Healthcare: Hospital A teilt patient-related AI-decisions mit Hospital B für continuity-of-care, beide compliance-auditable
- Finance: Bank A teilt fraud-detection signals mit Bank B (consortium model)
- Supply Chain: Manufacturer + Distributor sharen AI-driven inventory decisions
- AI-Marketplace: Agent-Provider sharen agent-passports mit Agent-Consumer (siehe §4.3)

**Implikation für Atlas's Business:** B2B-Federation ist die "enterprise expansion" Story — adoption beginnt single-tenant, wächst zu federated multi-tenant. Pricing-Hebel.

---

## 5. Competitive Differentiation (Headline-Form — Doc D macht das exhaustive)

Atlas's strukturelle Position vs jeder Major Player. Eine Zeile, eine Hauptbehauptung. Doc D wird das exhaustive ausarbeiten mit pricing, features, license, current state via WebSearch. Hier nur das Differenzierungs-Bullet.

- **Mem0 / Mem0g** — Atlas und Mem0 sind orthogonal: Mem0 macht fast retrieval, Atlas macht trust-substrate. Mem0g ist als **rebuildable cache on top of events.jsonl** im V2-stack vorgesehen — Mem0 ist nicht Konkurrent, sondern potential Partner / Layer / wrappable Komponente.
- **Letta (ehemals MemGPT)** — Atlas ist trust-substrate UNTER Letta. Letta könnte Atlas adoptieren als verifiable backing store; sie haben aktuell keine cryptographic-trust-property.
- **Zep / Graphiti** — Graphiti supports FalkorDB backend (Atlas V2-α stack); Atlas's events.jsonl könnte Graphiti's data source sein. Graphiti ist Apache-2.0 — Open partner potential. Zep Cloud is separate (vendor-locked, no Atlas-style trust).
- **Anthropic Memory** — Claude-only, vendor-silo, **kein** cross-vendor verifiable. Atlas ist vendor-neutral, sub Claude-Workflows können via MCP in Atlas schreiben.
- **OpenAI Memory** — ChatGPT-only, opaque storage, **kein** verifiable export. Atlas ist anti-vendor-lock-in by design.
- **Obsidian** — local-first markdown, dominant in PKM, **kein** cryptographic trust, **kein** Multi-Author-Identity. Atlas bietet die Obsidian-Mechanik (local-first + paid sync) PLUS cryptographic trust PLUS multi-agent-write.
- **Notion** — vendor-cloud-locked, no local-first, no cryptographic trust. Atlas ist strukturell die Anti-Notion-These.
- **Roam Research / Logseq** — graph-PKM, smaller market, no cryptographic trust. Atlas könnte als Backend für Logseq-style Frontend agieren (Apache-2.0 verifier + open file format makes that natural).

**Headline-Differentiator über alle:** **Atlas ist der einzige Player mit cryptographic-verifiability als strukturelle, agent-agnostische, cross-vendor Property.** Keiner sonst hat das. Greenfield.

Doc D should challenge: gibt es ein cryptographic-AI-trust Project das wir noch nicht gesehen haben? VeritasChain Protocol (VCP) wurde im Dec 2025 als adjacent erwähnt — Phase 2 muss WebSearch-verifizieren.

---

## 6. Go-to-Market Hypotheses

V2-GTM ist eine Frage von "wo finden wir den ersten 100 Customers, der Geld zahlt für Trust?". Vier parallele Hypothesen, alle in Phase 2 mit Daten zu validieren oder zu killen.

### 6.1 Hermes Agent Skill Integration als first Distribution-Vehicle

**Hypothese:** Hermes Agent (Nous Research, MIT license, model-agnostic, since 2026-05-10 #1 auf OpenRouter, ~60K GitHub stars — these are claims from the session handoff §0, Phase 2 should verify current numbers) hat ein Plugin/Skill-System. Atlas baut den "Atlas Memory Skill for Hermes" und tritt damit in die Hermes-Adopter-Welle ein. Nous Research's Issue #477 zeigt Offenheit für Skill-Erweiterungen.

**Pro:**
- Tiefer Distribution-Hebel — 60K+ stars in 2 Monaten ist hyper-growth
- MIT license heißt keine vendor-friction
- Hermes Agent ist model-agnostic → adopters sind disproportional die "vendor-diversification"-conscious crowd (Atlas's natural fit)
- Wenn Hermes-stack als "OpenAI-Killer" wahrgenommen wird, profitieren wir vom Halo

**Con:**
- Risk-of-coupling: wenn Hermes adoption stalls (vendor-risk siehe Doc C R-Hermes-Adoption-Reversal), trägt Atlas mit
- Hermes-User sind early-adopter / engineer-heavy → not the EU-regulated-vertical buyer who has budget

**GTM-Sub-Strategie:**
1. Build Atlas Memory Skill für Hermes (Apache-2.0)
2. Submit als upstream Pull-Request zum Hermes Plugin Catalog
3. Demo-Folge: Two Hermes instances, dieselbe Atlas-Wissensbasis, full-provenance, demoable in 60 seconds
4. Cross-promotion mit Nous Research Twitter/X

### 6.2 EU-Regulated Verticals als Compliance-driven Adoption

**Hypothese:** Finance / Healthcare / Insurance haben Compliance-budget. EU AI Act Art. 12 + AI Liability Directive zwingen sie zu kaufen. Atlas's structural compliance (siehe §3) ist die best-of-class Antwort. Sales-Cycle ist lang (6-12 Monate) aber Deal-Size ist high (5-6-figure EUR Annual Contract Value).

**Pro:**
- Predictable buying behaviour (compliance budget vorhanden, Approval-Prozess bekannt)
- Hohe Deal-Size justifies extended Sales-Cycle
- Niedrige churn nach Initial-Adoption (compliance ist sticky)
- Reference-Customers in regulated Verticals = enormous Halo (alle anderen Compliance-buyer schauen auf diese Reference)

**Con:**
- Lange Cycle bedeutet Cashflow-Risiko bis erste Verträge close
- Compliance-Buyer-Persona ist konservativ, wollen nicht "the new thing", wollen "the proven thing"
- Atlas ist V1.0.1 — has zero enterprise references aktuell. Lots of education needed.

**GTM-Sub-Strategie:**
1. ICP-Identifikation: welche Banken / Versicherer / Hospitals haben aktive EU-AI-Act-Compliance-Programme? Phase 2 should research-list 20-30 candidates
2. Reference-customer-acquisition: ein early Adopter mit gratis Lizenz + Co-Marketing-Vereinbarung
3. EU AI Act compliance white paper (long-form, deutsch + englisch)
4. Speaker-Slots auf EU compliance events (RegTech Forum, AI Compliance Summit, BaFin AI events)

### 6.3 Open-Weight-Model Alignment als Pull-Factor gegen Vendor-Capture

**Hypothese:** Open-weight Welle (Llama, Mistral, Hermes, Qwen, DeepSeek) hat strukturellen Bedarf nach vendor-neutral Memory weil sie strukturell anti-vendor-capture sind. Atlas's vendor-neutralität ist eine **strukturelle Komplementärität**, nicht nur eine GTM-Behauptung.

**Pro:**
- Open-weight Welle wächst (2025 markant; 2026 erwartet weiter)
- Adopters sind technically-sophisticated → understand Atlas's value-proposition ohne deep education
- "Vendor-locked Anthropic Memory vs Vendor-neutral Atlas Memory" ist eine sehr griffige Pitch-Line

**Con:**
- Open-weight community ist preisbewusst → revenue-extraction hart
- Größenordnung des Markts kleiner als enterprise EU-regulated
- Risk: Open-source community erwartet open-source server-component → schwächt Sustainable-Use-Lizenz

**GTM-Sub-Strategie:**
1. Conference-Speaks bei Open-Source AI events (Hugging Face Open Source AI Day, Mistral AI events)
2. Reddit / Hacker News / r/LocalLLaMA presence
3. Blog-Series: "Why your open-weight stack needs verifiable memory"
4. Free / cheap tier for individual developers (similar to Obsidian's free local-first model)

### 6.4 Obsidian-style Open-Core Monetization

**Hypothese:** Open-core mit free Verifier (Apache-2.0, already shipped V1.0.1) + paid hosted-sync / Enterprise-features. Mirroring Obsidian's Free-Local + Paid-Sync-or-Publish ($5/mo / $10/mo) model. Obsidian has proven this works for PKM-tools.

**Pro:**
- Atlas's verifier license-split is already there (Apache-2.0 verifier, Sustainable-Use server) — minimal additional engineering
- Obsidian's model is **proven** in adjacent market
- Pricing-mechanics scale from individual ($5/mo) to team to enterprise (per-seat)
- Low friction adoption (free tier removes friction)

**Con:**
- Open-Core balance is tricky — too much in free tier kills paid conversions, too little blocks adoption
- Paid-sync market is competitive (Obsidian Sync, Notion Sync, iCloud, Dropbox)
- Atlas's paid features need to be clearly Sustainable-Use-licensed, which is more friction than pure SaaS

**GTM-Sub-Strategie:**
1. Free Tier — verifier + local atlas-web (single-user, single-vault, no sync)
2. Personal Tier — paid hosted sync, multi-device, per-user-keypair (€5-10/mo)
3. Team Tier — multi-author workspace, cross-author signature verification, federated witness (€20-50/seat/mo)
4. Enterprise Tier — Continuous-Audit-Mode, Cedar policy enforcement, custom witness federation, SLA (€10k-100k/year)

### 6.5 Kombinierte GTM-These

**Empfehlung:** §6.1 (Hermes) + §6.4 (Open-Core) als Early-Adopter-Phase (Quarters 1-3 post-V2-α-Release), §6.2 (EU-regulated) als Enterprise-Phase (Quarters 4+, parallel zur Open-Core-Wachstum). §6.3 als kontinuierlicher Halo-Booster.

**Phase-2-question:** ist das die richtige Sequencing? Möglich auch: §6.2 first weil das die Burn-Rate financiert. Phase 2 Critique muss das mit cashflow-Daten challenging.

---

## 7. Risks to Positioning (Positioning-Level, nicht exhaustive — Doc C macht das exhaustive)

Diese 6 Risiken sind specifically positioning-level. Doc C wird sie + zusätzliche operational/crypto/legal-Risiken in der formalen Risk-Matrix-Form mit Probability × Impact × Mitigation × Owner aufnehmen.

### R-P-1 — Two-Market Narrative Complexity

**Risiko:** Two-market positioning ist hart zu kommunizieren. Wenn jede einzelne Kommunikation nicht klar EINE Story wählt, klingt Atlas wie "ein Allheilmittel" → consumer und enterprise switch off.

**Mitigation:** Marketing-Discipline. Jede einzelne Demo, Pitch, Landing-Page-Variante wählt EINE Persona als Primary. Doc E Demo-Sketches sind bewusst persona-segmented.

### R-P-2 — Market Timing für "Verifiable Second Brain"

**Risiko:** Verifiable Second Brain ist eine **aspirational category**. Bedarf wird gesehen wenn das Produkt da ist, nicht aktiv gesucht. Wenn wir zu früh kommunizieren, ist die Nachfrage zu klein. Wenn wir zu spät, hat Obsidian eigene "trust" Features integriert.

**Mitigation:** Mehrere Probepunkte vor commit: a) WebSearch market-validation (Phase 2 Doc D), b) Interview 5-10 PKM-power-user, c) Pre-Launch landing-page A/B-Test.

### R-P-3 — Vendor Opposition (Anthropic / OpenAI / Google)

**Risiko:** Major AI vendors haben aktive Interesse, vendor-locked Memory zu pushen. Sie könnten:
- Atlas's MCP-Bridge in proprietären API-Conditions verbieten
- "Native Memory" mehr aggressiv pushen mit subsidized pricing
- Atlas's Apache-2.0 Verifier fork-and-rebrand (unwahrscheinlich aber möglich)

**Mitigation:** Atlas's Position ist explizit Apache-2.0 + cross-vendor; das macht "Atlas verbieten" rufschädigend. Open-weight-Stack-Alignment (siehe §6.3) ist defensive Pull. Plus: structurally, vendors NEED an audit-trail-substrate für EU AI Act — sie können Atlas opposing UND Atlas needing nicht gleichzeitig durchhalten.

### R-P-4 — Wahrgenommene "Cryptographic Complexity" als Adoption-Bremse

**Risiko:** Knowledge-Worker und AI-Engineer wollen oft nicht über crypto nachdenken. "WASM verifier" / "Sigstore Rekor" / "Ed25519" sind technical-buyer terms. Wenn UX surface zu viel davon zeigt, drop-off.

**Mitigation:** Hide-the-machinery-by-default UI principles. Surface zeigt nur "Verified ✓" / "Tampered ✗"; das technical detail ist click-through. Inspiration: HTTPS-lock icon — billions of users vertrauen ihm, fast keiner versteht TLS.

### R-P-5 — Mem0g / Graphiti Werden Konkurrenten Statt Partner

**Risiko:** Wir behaupten in §5 dass Mem0g und Graphiti potential partner sind. Wenn sie aber 2026-2027 eigene cryptographic-trust-Features entwickeln, wandeln sie sich strukturell zu Konkurrenten. Mem0 ist VC-backed → has incentive to maximize feature footprint.

**Mitigation:** Schnell partnership-formalize (legal MoU, joint demo, co-marketing) bevor sie eigene Roadmap entwickeln. Aber: nicht abhängig machen von einer Partnership — Atlas muss standalone funktionieren.

### R-P-6 — "Compliance Vendor"-Wahrnehmung Reduziert die Vision

**Risiko:** wenn der erste Customer-Cluster aus EU-regulated Verticals kommt (siehe §6.2), könnte Atlas als "yet another compliance vendor" wahrgenommen werden. Das verengt die Vision von "Verifiable Substrate für AI + Humans" zu "EU AI Act Add-on".

**Mitigation:** Parallel-Building in §6.1 (Hermes) + §6.3 (Open-Weight) als Brand-positioning Counterweights. Marketing-Disziplin: never use the word "compliance" without pairing it with "novel trust modes" (siehe §4).

### R-P-7 — Phase 1 Documentation Drift

**Risiko (meta):** dieser Doc ist Phase 1 v0. Phase 2 critique kann ergeben dass Hauptthese falsch ist. Phase 3 Synthesis kann eine völlig andere Vision produzieren. Wenn wir uns hier zu früh auf Tagline / Pricing / Persona committen, blockieren wir die Iteration.

**Mitigation:** Dieser Doc ist explizit v0. Keine Decisions sind locked. Phase 2 critique-agents sollen die Open-Questions in §8 herausfordern.

---

## 8. Open Questions for Phase 2 Critique

Format: "Q: <question>. Context: <1-sentence why this matters>. Status: open." — Phase 2 critique-agents (product, business, strategy reviewers) should systematically work through these.

**Q-1: Ist "Verifiable Second Brain" ein echter Markt (Bedarf existiert, kaufbereit) oder eine aspirational category (Bedarf wird gesehen nachdem Produkt da ist)?**
Context: §2.1 macht die Behauptung, aber wenn der Markt aspirational ist, müssen wir die Phase-2-α/β Resources anders allokieren (mehr Education, mehr long-tail Content vs faster Product-Velocity). Status: open.

**Q-2: Welche Welle der Adoption ist Primary-Story, EU-regulated-Vertical (§6.2) oder Hermes-Open-Weight-Welle (§6.1)?**
Context: Two-market positioning ist riskant; eine Primary-Welle muss klar sein. Cash-Flow-Implikationen sind sehr unterschiedlich (enterprise long-cycle high-ACV vs open-source short-cycle low-ACV). Status: open.

**Q-3: Ist AI-Liability-Insurance-Pricing (§4.2) ein realistischer GTM-Hebel im 2026/2027 Zeitfenster, oder zu unreif?**
Context: Wenn ja, wir sollten einen Insurance-Industry-Aufklärer früh anfragen. Wenn nein, deferren wir §4.2 nach 2027+. Phase 2 should ideally interview einen AI-Insurance-Underwriter. Status: open.

**Q-4: Ist die Hermes-Agent-Adoption (60K stars, MIT, #1 OpenRouter) ein nachhaltiger Trend oder ein 2-Monats-Spike?**
Context: §6.1 macht Hermes als first distribution vehicle. Wenn Hermes-adoption peakt und zurückgeht, müssen wir auf §6.2 verschieben. Phase 2 should WebSearch confirm Hermes's current state and growth trajectory. Status: open.

**Q-5: Hat Atlas's Apache-2.0-Verifier + Sustainable-Use-Server Lizenz-Split die Adoption-Friction die wir behaupten sie hat?**
Context: §6.4 baut auf Obsidian-Model. Aber Obsidian ist proprietary (closed source) mit free-tier; Atlas ist hybrid Apache-2.0 + Sustainable-Use. Letzteres ist source-available aber kommerziell-restricted. Manche Open-Source-Communities reagieren allergisch (siehe MongoDB SSPL Backlash). Status: open.

**Q-6: Ist die "Agent Passport" Vision (§4.3) zu früh? Existiert die Agent-Marketplace-Welle 2026/2027 in der wir behaupten?**
Context: Wenn Agent Marketplaces nicht entstehen (AI-Agents bleiben vendor-owned), ist §4.3 abgewandt. Phase 2 should sich an LangServe, Hugging Face Spaces, Agent-as-a-Service trends orientieren. Status: open.

**Q-7: Ist die Continuous-Regulator-Attestation (§4.1) ein realistischer Trust-Mode oder eine Architektur-die-niemand-fordert?**
Context: Hört sich strukturell mächtig an, aber: würde eine BaFin / FCA / FINMA tatsächlich einen Witness-Key federieren? Phase 2 should sondieren via Regulator-Beratungs-Netzwerk (Nelson hat hier Connections?). Status: open.

**Q-8: Wie reagieren wir auf eine Anthropic / OpenAI / Google Counter-Strategie (z.B. "Anthropic Memory mit cryptographic export" announced 6 months post-Atlas-V2-launch)?**
Context: §7 R-P-3 erwähnt Vendor-Opposition. Aber Vendor-Co-Option (sie *übernehmen* unser Feature) ist auch ein Risiko. Defensive Moats? Patent-Strategy? Open-Standards-Strategy? Status: open.

**Q-9: Ist FalkorDB + Mem0g + Hermes Agent Stack-Choice optimal, oder bauen wir uns mit drei VC-backed-startup-Dependencies in vendor-risk-stack-of-three?**
Context: Doc B handles technical architecture; aber die strategic-positioning-Implikation ist: wenn einer der drei stalls, Atlas trägt mit. Alternative open-source-only Stacks (Kuzu statt FalkorDB, Graphiti vs Mem0g, LangChain stat Hermes)? Status: open.

**Q-10: Welche Compliance-Frameworks NEBEN EU AI Act sind unser strategischer Fit?**
Context: §3 fokussiert auf EU AI Act. Aber GAMP 5 (Pharma), ICH E6(R3) (Clinical Trials), DORA (Financial Resilience), HIPAA (US Healthcare), SOC 2 (B2B SaaS standard) sind alle adjacent. Welche bilden wir explizit ab als Compliance-Mapping (über V1's docs/COMPLIANCE-MAPPING.md hinaus)? Status: open.

**Q-11: Brauchen wir eine "Atlas Foundation" / non-profit governance structure für vendor-neutralität?**
Context: Wenn Atlas zur Industry-Standard-Substanz wird (Hope-Case), könnte Stewardship-Konzern-Branding ("Atlas Inc kontrolliert den Standard") ein Adoption-Blocker werden. Vorbild: CNCF / Apache Software Foundation. Phase 2 should bewerten ob das jetzt schon ein Strategie-Item ist oder erst post-Year-2. Status: open.

**Q-12: Wo sitzt unser Pricing-Optimum?**
Context: §6.4 listet €5/mo (Personal) bis €100k/year (Enterprise). Aber: Atlas's Wert ist Compliance-Verträglichkeit (worth 5-7-figure EUR per regulated customer) UND Personal-Tool-Convenience (worth $5/mo). Das ist eine 1000:1 Price-Spread. Welche Tiers sind aktiv, welche sind Marketing-Sirenen? Status: open.

**Q-13: Ist die Tagline "Knowledge your AI can prove, not just claim" (Kandidat 2 in §1.3) klar genug für die Two-Market-Story, oder bevorzugen wir markt-spezifische Sub-Taglines?**
Context: Ein-Tagline-für-zwei-Märkte ist hart. Phase 2 sollte tagline-tests vorschlagen (LinkedIn-Polls? Landing-Page-A/B? Brand-Strategist-Konsultation?). Status: open.

**Q-14: Wie positionieren wir uns gegenüber Sigstore (auf dem wir aufbauen) und SLSA (das wir erfüllen)?**
Context: Sigstore + SLSA sind unsere upstream dependencies. Aber sie sind Industry-Standards. Sind wir "the AI-Memory-instance of Sigstore"? Oder "we use Sigstore as one of many anchoring options"? Strategic positioning-question. Status: open.

**Q-15: Wann committen wir auf eine konkrete V2-α/β/γ Roadmap mit Datum-Targets?**
Context: Phase 1 Foundation Docs sind Vision. Wann übersetzen wir das in Quarter-by-Quarter Roadmap mit Ship-Dates? Phase 3 Synthesis? Phase 4 Implementation-Planning? Wenn wir zu früh committen, lock uns in; zu spät, weisses Pferd. Status: open.

**Q-16: Ist V2 ein "V2 release" oder zwei separate Tracks (V2-Personal + V2-Enterprise)?**
Context: Zwei-Markt-Positioning könnte auch zwei-Produkt-Lines bedeuten. Atlas (Enterprise, Sustainable-Use) + Atlas Personal (Open-Core mit free tier, separater Repo, separates Branding?). Phase 2 should bewerten ob das die Komplexität reduziert oder erhöht. Status: open.

---

## 9. Appendix — Cross-References

### 9.1 Phase 1 Sister-Docs (Foundation Documents)

- `.handoff/v2-vision-knowledge-graph-layer.md` — **Doc B (Technical Architecture)**. Defines events.jsonl → projector → FalkorDB → Mem0g hybrid; MCP read/write API; Hermes Agent skill integration surface; Agent Identity Layer (Ed25519-DID); GDPR right-to-be-forgotten handling.
- `.handoff/v2-risk-matrix.md` — **Doc C (Risk Matrix)**. Formal Probability × Impact × Mitigation × Owner matrix für 8-12 risks incl. post-quantum, GDPR, adoption-tipping, vendor-capture, performance.
- `.handoff/v2-competitive-landscape.md` — **Doc D (Competitive Landscape)**. Detailliertes feature × pricing × trust-property × Atlas-differentiator matrix mit WebSearch-verified 2026-data.
- `.handoff/v2-demo-sketches.md` — **Doc E (Demo Sketches)**. 5 demo scenarios mit 30-90s storyboard each: Multi-Agent-Race, Continuous-Audit-Mode, Agent-Passport, Verifiable-Second-Brain, Mem0g-Hybrid.

### 9.2 V1 Artifacts Referenced

- `README.md` — V1.0.1 public-facing pitch; trust property summary; component license split
- `docs/ARCHITECTURE.md` §10 (V1 / V1.5 / V1.6 / V1.7 / V1.8 / V1.9 / V2 boundaries) — die historische technische Roadmap; V2-section in §1833+ lists "full COSE + policy + SPIFFE" als ursprüngliche V2-Vision (das ist die *engineering*-V2; dieses Doc ist die *strategic*-V2 und expanded that scope)
- `docs/ARCHITECTURE.md` §11 — "Not a graph database. Not a policy engine yet." Die explicit-listed V1-Limitierungen sind genau die V2-α/β Targets
- `docs/COMPLIANCE-MAPPING.md` — V1's existing EU AI Act + GAMP 5 + ICH E6(R3) + DORA + GDPR Mapping (clause-by-clause)
- `docs/SEMVER-AUDIT-V1.0.md` — V1.0 public-API contract; V2 forward-break candidates (z.B. `--require-strict-chain` default-flip für single-writer profiles)
- `crates/atlas-trust-core/src/verify.rs` — V1 verifier core; V2-α/β extends, doesn't break
- `crates/atlas-witness/` — V1 Witness pattern; V2 Continuous-Regulator-Attestation extends
- `apps/atlas-mcp-server/` — V1 MCP write-tool surface; V2 adds query-tools (siehe Doc B §2.9)

### 9.3 V1.19 Handoff Cross-Reference

- `.handoff/v1.19-handoff.md` §0 — V1.0.1 SHIPPED state, npm publish, SLSA L3 verification details. This is the substrate Atlas V2 builds on; nothing in this Doc A undermines V1's trust property.

### 9.4 Iteration Framework

- `.handoff/v2-iteration-framework.md` — 4-Phasen-Methodik. Dieses Doc ist Phase 1 Doc A. Phase 2 (Critique) → Phase 3 (Synthesis) → Phase 4 (Roadmap commitment) folgen sequentiell, jeweils mit own planning pass.

---

## 10. Closing Note

Dieser Doc ist v0. Phase 2 critique should be **aggressive** — preserve nothing for ego's sake. Wenn die Two-Market-Positioning bricht unter Critique, bricht sie. Wenn die Tagline-Kandidaten alle abgelehnt werden, brauchen wir bessere. Wenn die GTM-Sequencing umgekehrt werden sollte, umkehren.

Aber: die Kern-These — **Atlas ist das verifiable substrate für AI agent memory + human Second Brains, beide auf demselben events.jsonl + Sigstore Rekor + Witness cosignature Trust-Layer** — ist die strategic crown jewel. Jede Critique sollte diese These respektieren oder explizit challengen (mit Begründung warum). Inkrementeller Erosion durch Compromise ist der Tod der Vision.

**End of Doc A v0.** Phase 2 begins after Phase 1 Foundation Documents integration branch is created.
