# Crit: architect on Atlas V2 Vision

> **Reviewer role:** independent technical architect. **Targets:** Doc B (Knowledge Graph Layer) primary, Doc D (Competitive Landscape) secondary; cross-reference Docs A/C/E for consistency against the V1 trust invariant (`docs/ARCHITECTURE.md` §10–§11, `docs/SEMVER-AUDIT-V1.0.md`).
>
> **Stance:** Atlas V1's trust property is what makes V2 worth building. Any V2 layer that compromises "events.jsonl is the only authoritative source of truth" — even subtly, even in failure modes — is flagged CRITICAL. I am unafraid to disagree with the docs where they hand-wave.

---

## Stärken (was ist gut, sollte bleiben)

1. **Doc B §2.1 + §2.5 trust-invariant chain is properly stratified.** The three-layer architecture (events.jsonl → FalkorDB → Mem0g) makes the rebuild-direction explicit and assigns each layer an explicit "what survives if I lose it" failure mode. This is the right shape — Layer 1 carries authority, Layers 2/3 are convenience. Worth keeping verbatim into the master vision.

2. **Doc B §2.5 bi-temporal mapping is derivable from Atlas-native timestamps alone.** Removing the Graphiti hard-dependency for bi-temporality (`t_valid` / `t_invalid` / `t'_created` / `t'_expired` all derive from `event.timestamp` + `rekor_anchor.timestamp` + signed retraction events) is a clean architectural decision. It preserves the Graphiti partnership upside (Doc D §2.3) without making Atlas dependent on a third-party data model.

3. **Doc B §2.7 keeps per-workspace HKDF (V1.9) and per-agent DIDs orthogonal.** The "`kid` for V1 invariants, `author_did` for V2 passport-coherence" split (Doc B §2.7 last paragraph) avoids collapsing two distinct identity axes into one — the cross-workspace replay defence (V1.9, `PER_TENANT_KID_PREFIX`) survives unchanged, and DIDs are a *strictly additive* signing identity. Compatible with `docs/SEMVER-AUDIT-V1.0.md` §1.1 ("adding fields = SemVer-minor").

4. **Doc B §3.3 (content/hash separation) is the only architecturally honest GDPR answer.** Signed `content_hash` + nullable `content_pointer` + tombstone-event pattern is the correct shape. The chain remains intact post-deletion; the *fact-of-redaction* itself becomes a signed event. This is also the right hook for R-L-01 in Doc C.

5. **Doc B §2.3 keeps the graph-store adapter abstract.** "Storage adapter so Kuzu swap is configuration-time" is the kind of pre-emptive decoupling that pays compound interest. Combined with Doc D §4.4's Kuzu-archived note, this matters more than the doc currently flags.

6. **Doc D §6 matrix is rigorous about "Trust Property: none" across the board.** The competitive analysis correctly identifies that Mem0/Letta/Zep/Graphiti/Anthropic/OpenAI Memory all share *zero cryptographic trust property* — making Atlas's moat structural, not feature-shaped. Doc D §7.3 "white spaces Atlas can own" is well-argued.

7. **Doc B §2.10 federated regulator-witness lifecycle is well-specified.** The enrolment ceremony (operator + regulator HSM out-of-band) reuses V1.18's SSH-Ed25519 trust-root-mutation discipline; revocation cleanly maps to existing PubkeyBundle update flow. The continuous-attestation operating-mode (regulator key *inside* the trust root) is the cleanest articulation of why this architecture beats periodic-audit alternatives like Vanta/Drata.

---

## Probleme (was muss adressiert werden — by severity)

### CRITICAL

- **C-1: Projection determinism is under-specified to the point of being unverifiable.** Doc B §2.1 + §3.2 claim "deterministically rebuildable" and propose a "projector-state-hash CI gate"; Doc C R-A-01 correctly identifies this as HIGH-probability / CRITICAL-impact / **LOW detectability**. But the proposed mitigation (a CI hash gate over a test corpus) is *necessary but insufficient*:
  - Determinism MUST be defined over: (a) FalkorDB upsert ordering (Cypher `MERGE` is not order-stable across concurrent transactions); (b) entity-ID stability across projector runs (Doc B §2.8 uses `blake3 of first-seen event's entity_key` — but "first-seen" depends on event-ingestion order, which under Doc B §3.18 option-a S3-backed distributed Layer 1 is *not* guaranteed monotonic); (c) float-formatting / locale / timezone in any property value derived from event payload; (d) any stable-sort that falls back to insertion order on equal keys.
  - The CI gate proposed in §3.2 hashes a "canonical serialisation" — but the canonicalisation function itself is not specified. This is the same trap V1 avoided with `signing_input_byte_determinism_pin` (which pins the bytes, not the data model). Doc B §3.2 needs a `projected_graph_canonical_form_v1` spec analogous to the V1 CBOR canonicalisation spec, plus a byte-pin test.
  - **The deepest problem:** under the V2 architecture, an auditor running the WASM verifier against `events.jsonl` sees ✓ VALID *while the graph explorer shows different facts*. The trust property holds at the event layer; the read-side surface is silently wrong. Doc C Q1 and Q8 surface this — it must be answered before V2-α ships, not after.

- **C-2: The `kid` (V1.9 per-workspace) vs `author_did` (V2.7 per-agent) interaction is not enforced in any signature.** Doc B §2.7 says "Verifier checks both: `kid` for the V1 invariants, `author_did` for the V2 passport-coherence check." But the cross-binding mechanism is absent. Concretely: nothing in the current spec prevents an attacker who has compromised one workspace's HKDF-derived key from forging events claiming to be authored by a *different* agent's DID. The signature is over the COSE_Sign1 envelope; the `author_did` is just a string in the payload. Unless the agent's DID-key *also* signs the event (multi-signer pattern — `docs/ARCHITECTURE.md` §10 V2 §"Multi-signer events"), the DID is unauthenticated metadata. Doc B §2.7 quietly assumes both signatures are present but never specifies the envelope format.

- **C-3: GDPR content-hash separation may not be GDPR-sufficient — and Doc B treats this as resolved.** Doc B §3.3 declares the design "satisfies V1's chain-of-custody" and Doc C Q2 honestly flags it as legally open. **Architecturally,** a hash of a single-subject record (e.g., one named patient) is *itself* potentially personal data under GDPR Art. 4(1) since it can identify the data subject in context (hash-collision-free identification of a known record). If EU counsel rules the hash is personal data, the proposed architecture *doesn't fix the problem* — it just narrows the leaked surface. Doc B claims trust-clean GDPR-compliance-by-design; the truthful claim is "GDPR-mitigated by design, not GDPR-compliant by design pending legal opinion." This phrasing matters because Doc A §3.1 and Doc D §6 matrix both row-claim "GDPR-by-design" for Atlas — which is currently an aspirational claim, not a verified one.

### HIGH

- **H-1: Multi-tenant isolation under FalkorDB native multi-graph (§3.10 option c) is not validated against cross-workspace replay.** V1's defence against cross-workspace replay (`kid` byte-equal to `atlas-anchor:<workspace_id>`, V1.9) lives at the *event-signature-verifier* layer. The proposed V2 architecture has read-side queries (Doc B §2.8) hitting FalkorDB graphs directly; if the query layer accepts a `workspace_id` parameter and the projector tags every node with `workspace_id`, then a bug in the API authz layer (or a confused-deputy attack via the MCP V2 tools — Doc B §2.9) could leak nodes from one workspace into another's query results *without ever touching the signature verifier*. The trust property survives (the auditor can re-verify), but the read-side surface returns wrong data. Needs:
  - An explicit multi-tenant isolation invariant: "every read endpoint MUST resolve workspace from auth context, never from request body."
  - A property test that demonstrates one workspace cannot query another's graph even when the API returns successfully.
  - A spec for what happens during a *legitimate* cross-workspace query (Doc B §3.10 mentions "explicitly opt-in via a privileged endpoint" — needs design).

- **H-2: MCP V2 tool surface (§2.9) couples atlas-mcp-server to the read-side HTTP API in a way that breaks V1's offline-verifier promise.** V1's atlas-mcp-server `verify_trace` is fully offline (WASM verifier). V2 adds five MCP tools that are "thin clients of the read-side API" (Doc B §2.9 trust-property paragraph). If atlas-mcp-server is to remain operable in air-gapped / disconnected mode (Doc C R-A-02 mitigation §2 lists "air-gapped, offline, WASM-only verification" as a first-class anchoring mode), then `query_graph` / `query_entities` / `query_provenance` need a local-only mode that doesn't require the read-side HTTP API. Doc B doesn't specify this. The honest architectural answer is either: (a) the MCP V2 tools are *online-only* and that's documented as a V1→V2 capability narrowing; or (b) atlas-mcp-server embeds a FalkorDB read replica + Mem0g cache locally, multiplying the operator's deployment burden.

- **H-3: Welle decomposition (§4) has hidden critical-path dependencies and an aggressive session count.** The doc claims "V2-α ~3-4 sessions" for the Atlas Projector crate + FalkorDB integration + state-hash CI gate + Agent DID schema + content-hash separation. This is unrealistic by an order of magnitude:
  - The projector crate alone is comparable in scope to `atlas-trust-core` (which took multiple welle to harden). Realistic estimate: 5-8 sessions for a deterministic projector with a byte-pinned canonical-graph-form test, given the C-1 above.
  - V2-α + V2-β are listed as "α → β-parallel-with-γ → δ-optional" but V2-β's Mem0g cache (§2.5 Layer 3) is described as "indexes Layer 2 (FalkorDB)" — i.e., it strictly depends on V2-α completion, not parallel with γ. The DAG has α → β; γ (passport materialisation, regulator-witness federation, Hermes Memory Skill) depends on V2-α's DID schema; only V2-δ is truly optional. The realistic critical path is α → β AND γ-pieces-that-need-DID → δ-optional, totalling 12-18 sessions, not 10-14.
  - Welle 14b alignment (§5 option-2) is "delays 14b-iii by ~6 sessions". That estimate assumes the V2-α-β estimate holds, which per the above it doesn't.

- **H-4: Read-side API §2.8 has no caching / rate-limiting / authn-authz spec.** Six endpoints, but:
  - No spec for who can call them. `GET /api/atlas/passport/:did` is a reputation query — is it public? Workspace-private? Authenticated only for the workspace that hosts the DID?
  - `POST /api/atlas/query` accepts sandboxed Cypher with `max_rows: 10000` hard cap — a malicious agent (or compromised Hermes skill, Doc B §2.6) could DOS the read-side by issuing many concurrent expensive queries.
  - No caching strategy. The `GET /api/atlas/audit/:event_uuid` response is content-addressable (output is a deterministic function of one event UUID + projector state), so it's a natural CDN cacheable. The spec doesn't mention this.
  - Production estimate of read-side throughput is not specified. Doc C R-O-01 covers write-side; the read-side has its own performance envelope which is unspecified.

- **H-5: Doc D §4.4 Kuzu acquisition + archived repo materially weakens the fallback chain.** The "FalkorDB SSPL → fallback to Kuzu MIT" plan no longer exists; Kuzu was acquired by Apple Oct-2025 and the repo archived. Doc B §2.3 + §3.1 still names Kuzu as the fallback. The honest fallback chain is now: **FalkorDB SSPL (primary, license-risk) → ArcadeDB Apache-2.0 (untested in Atlas integration) → Memgraph BSL (license-risk again, eventual-Apache-2.0 conversion uncertain) → HugeGraph Apache-2.0 (small Western mindshare)**. None of these have been validated for: bi-temporal modelling, Cypher subset coverage, performance, browser-UI equivalent. The "swap is configuration-time" claim (Doc B §2.3) is unproven against ArcadeDB specifically. If FalkorDB SSPL becomes blocking (Doc C R-L-02), the migration cost is no longer a config change — it's days-to-weeks of integration + a graph-explorer-UI rewrite (FalkorDB Browser doesn't generalise).

### MEDIUM

- **M-1: Hermes Memory Skill (§2.6) is treated as architecturally trustworthy when it's just a third-party npm/PyPI package.** Doc B §2.6 "Trust property under skill compromise" correctly notes the skill is "a thin client; nothing trust-load-bearing lives in skill code." But: the skill *holds the DID private key* in "skill-state, isolated to the local instance" (§2.6). If the skill package is compromised (typosquat, dependency-confusion, post-install script), the attacker exfils the private key and forges events under that DID for the full pre-revocation window (Doc B §2.7 + Doc C R-A-03). The blast-radius reasoning in §2.6 doesn't acknowledge this — it treats key exfiltration as "future writes blocked by revocation" but the *correct* analysis is "anything within the anchor-latency window (typically minutes) is undetectable forgery; longer windows possible if revocation tooling fails." Architecturally, the skill should never hold the private key directly — it should delegate to a local atlas-signer subprocess (V1 pattern; HSM-capable per V1.10/V1.11) so the key never touches the npm-distributable surface.

- **M-2: Witness-federation cost model and bootstrap problem are missing (§2.10).** Doc B specifies the regulator-witness *protocol* but not the *operating model*:
  - **Aggregation cost:** if a workspace policy demands cosignature from 3-4 different witness kids per event (e.g., internal-A + internal-B + regulator-bafin + auditor-deloitte), each event triggers 3-4 synchronous HTTP round-trips before it can be committed. At Doc C R-O-01's 1K-writes/sec target, that's 3-4K concurrent witness-HTTP-calls. No batching or pre-signing is specified.
  - **Bootstrap problem:** the first event in a workspace cannot be cosigned by a witness that doesn't yet know about the workspace. Doc B §2.10 lifecycle starts at "operator + regulator agree on a witness key" — but the *first signed event* (workspace-genesis) precedes that handshake. Either workspace-genesis is exempt (creating a small unwitnessed window) or witness federation is bootstrapped via out-of-band ceremony before any agent can write (creating an operator-burden mismatch with Atlas's "write through the HTTP API in <50 LOC" pitch in Doc B §1a).
  - **Witness compromise blast radius:** if a witness key is compromised, every event cosigned with it during the compromise window is forensically suspect. Doc B mentions the recovery path (revocation) but not the *audit cost* — re-evaluating N months of witnessed events to determine which were observed-during-compromise is unspecified.

- **M-3: Doc B §3.18 distributed Layer 1 changes the trust model and is under-specified.** "Per-workspace events.jsonl in S3-compatible object storage" with "S3 conditional-writes give us atomicity" — S3 PutObject-If-None-Match (the standard conditional-write primitive) was only generally available in 2024-11 and *only on AWS S3 itself*, not all S3-compatible stores (Cloudflare R2, Backblaze B2, MinIO, Wasabi, Garage, Tigris have differing support and semantics). The "minimal new operational surface" claim doesn't account for: (a) cross-region replication consistency (S3 default is eventual); (b) S3 server-side encryption interacting with Atlas's per-tenant key model (does the workspace operator hold the KMS key, or AWS?); (c) the per-workspace mutex (Doc B reference to `@atlas/bridge::writeSignedEvent` mutex, also `docs/SEMVER-AUDIT-V1.0.md` §1.1) is currently in-process — distributing it requires a coordination primitive that is *not* yet listed as a V2 dependency.

- **M-4: Doc D §6 matrix overclaims for the Atlas row in two columns.**
  - "Temporal: ✓ (V2-δ bi-temporal)" — V2-δ is "optional" per Doc B §4. Putting a definitive ✓ in the competitor matrix for an explicitly-optional welle is overclaim. Should be "~" or "✓ (V2-δ — optional)".
  - "GDPR-by-design: ✓" — see C-3 above. The honest answer is "~" pending EU counsel opinion (Doc C R-L-01).
  - Doc D §8 Q12 honestly asks this question; Doc D §6 should reflect the open-question status until resolved.

- **M-5: Doc B §2.6 Hermes-as-#1-OpenRouter is a single-point-of-architectural-bet.** The doc justifies Hermes Memory Skill prominence with "Hermes Agent #1 on OpenRouter per `.handoff/v2-session-handoff.md` §0". This is a snapshot, not a structural fact. Doc C R-V-03 acknowledges this as MEDIUM risk; Doc B §3.5 lists "Hermes-skill + LangGraph-recipe + Claude-MCP" as hedge. But V2-γ acceptance criteria specifically test "Hermes Agent skill" — if Hermes is supplanted between now and V2-γ ship, the acceptance criteria need to be reframed around the *generic agent-skill pattern* rather than the Hermes-specific package. Doc B should specify the skill API surface as a *generic protocol* (analogous to MCP itself) and ship Hermes-skill, LangGraph-recipe, and Claude-MCP as three concrete implementations of that protocol — not as one canonical + two backups.

### LOW

- **L-1: §2.8 endpoint `GET /api/atlas/related/:id?depth=N` caps depth at 5. The cap value is asserted without justification.** "Phase 2 should challenge: is 5 the right cap?" — yes, and the answer depends on average node-degree in production traces. Default to 3 with explicit operator override would be safer. Latency budgets bite hard at higher depths.

- **L-2: §2.6 `agent_instance_metadata` is non-signed `attributes` — clean, but the doc doesn't explain why.** Mixing signed and non-signed fields in the same event is a known footgun (auditors confuse them). Add a single-line "signed fields are the source of authority; non-signed `attributes` are operator-discretion metadata that may differ between projector replays" caveat.

- **L-3: §2.7 `reputation_score` default formula stub uses `log10(writes_count) * (1 - retractions/writes) * sqrt(witness_cosigners.size)`. Two issues:** (a) `writes_count = 0` produces `log10(0) = -∞`; (b) reputation is monotonically non-decreasing in `writes_count`, incentivising noise-writes (Doc B §3.16 Sybil-resistance). Should ship without a default scalar (Doc B §3.16 option (c)) — but if a default *is* shipped, it should at minimum be Sybil-aware.

- **L-4: Doc D §6 matrix has Mem0/Letta/Zep as "Multi-Agent: ✓" but the column definition ("multi-tenant") is mixing concepts.** A multi-tenant SaaS isn't the same property as a substrate that supports multi-agent shared memory. Atlas's column claim is the latter; Mem0's is the former. The column should split or the definitions should be made explicit.

- **L-5: §2.9 MCP V2 tool count (5 new on top of V1's 2) is on the edge of MCP host token-budgets.** Each tool description costs context tokens at the host. 7 Atlas tools is fine in isolation but in a host that has 30+ other tools competing for token budget, Atlas's tools may be deprioritised by the LLM. Doc B Q-MCP-1 surfaces this; recommend Phase 3 collapse `query_graph` / `query_entities` / `query_provenance` / `get_timeline` into a single `atlas_query` tool with a `mode` parameter, keeping `write_node`, `verify_trace`, `get_agent_passport` as separate tools.

---

## Blinde Flecken (was wird in den docs gar nicht angesprochen)

1. **Projector concurrency and replay correctness.** The projector reads `events.jsonl` and writes to FalkorDB. What happens when two projector instances run concurrently against the same FalkorDB (HA deployment, accidental double-start)? Idempotent upsert is mentioned (Doc B §2-blueprint), but idempotence under interleaved writes is non-trivial. Locking? Leader election? Eventual consistency window? Unaddressed.

2. **Projector schema-version-coexistence.** Doc B §3.17 considers three options (hard-reseed, versioned-coexistence, migration-scripts) and recommends hard-reseed. But the read-side API doesn't have a "projector_version" filter on its endpoints. If a workspace mid-reseed serves queries, what does the auditor see — partial-old-projector + partial-new-projector results? No design.

3. **WASM verifier evolution under V2 schema changes.** V1's `atlas-verify-wasm` is locked-API per SEMVER-AUDIT §1.3. V2 adds DID-author-bound signatures (per C-2 above, if the cross-binding is fixed), redacted-content events (§3.3), retraction events (§2.5 bi-temporal), and witness-kid-required policy (§2.10). The WASM verifier needs to evolve. Doc B doesn't enumerate which V2 features require WASM-verifier changes; some may be SemVer-major (new variants on `VerifyEvidence::check`).

4. **Anchoring rate-limits at scale.** Sigstore Rekor publishes rate-limit guidance (typical: ~few hundred requests/sec total across the whole world). Doc C R-O-01 mentions "tiered anchoring — batch Rekor every N events" but the tier-2 latency-budget vs trust-window trade-off is unspecified. At 1K Atlas writes/sec, every event-individually-anchored would exhaust Sigstore's shared rate-limit in <1 second. Doc B §3.14 mentions "private witness federation as alternative" — but doesn't address: what's the rate-limit on the *private* federation? Is it federated-write-rate × N witnesses?

5. **Cost of holding 100M events.** Doc C R-O-03 covers projection-rebuild cost at scale, but not the carrying cost of Layer 1 itself. 100M events × average-event-size = ? bytes. Per-workspace events.jsonl growth is linear-forever (append-only). What's the operator-side storage burden trajectory? Garbage-collection / compaction strategy? Cold-tier archival to S3 Glacier? Architectural decisions deferred to "operator concerns" but they shape what Atlas can claim about long-term retention.

6. **Mem0g embedding-model drift over time.** Doc B §2.5 Layer-3 paragraph says "embedding_model_id + summariser_model_id pinned in Mem0g cache rows." This pins individual rows but doesn't address: when OpenAI deprecates `text-embedding-3-large`, the operator must re-embed everything (which model? at what cost?). Doc B says "deterministic given pinned model versions" — but the *pinned model versions* themselves age out. Long-term plan unaddressed.

7. **Composability with Lyrie ATP / DIDs that aren't `did:atlas:`.** Doc D §5.4 identifies Lyrie ATP as "strong partner". Doc B §2.7 specifies `did:atlas:` as the agent-identity scheme. If a Hermes operator wants their existing `did:lyrie:` agent to write to Atlas, what's the bridging story? Doc B §2.7 footnotes "algorithm-agnostic" but doesn't address "DID-method-agnostic." Without this, Atlas's DID scheme is a parallel-island and the partnership thesis is weaker than Doc D claims.

8. **Failure mode: events.jsonl exists but Rekor anchor never lands.** V1.6 lenient mode handles this. V2 with the read-side API and the regulator-witness federation creates a new failure shape: an event lands in Layer 1, the projector picks it up, the graph shows it as a fact, but the Rekor anchor request silently failed and nobody noticed. The read-side filter `require_anchored: true` (Doc B §2.8) helps if the consumer remembers to use it — but the *default* read-API response surfaces the fact as if it were anchored. UX-level mitigation needed.

9. **Operational secret management for the read-side service.** atlas-web in V2-β hosts the read-side API. atlas-web is a Next.js process. What credentials does it hold? At minimum: FalkorDB connection credential, Mem0g credential, witness-aggregation auth tokens (if §2.10 federation is online-cosigning), possibly per-workspace HKDF read-only keys for verifying author signatures on incoming queries (do queries need to be signed?). The credential lifecycle, rotation, and HSM-binding (V1.10/V1.11 patterns) for atlas-web's read-side surface is unaddressed.

10. **Time-source trust.** Multiple Atlas timestamps (Doc B §2.5: `event.timestamp`, `rekor_anchor.timestamp`, `witness_cosignature.timestamp`, `projector_run.timestamp`) are produced by *different parties* with *different clocks*. Bi-temporal correctness assumes monotonic ordering of these timestamps within tolerance. NTP drift, deliberate clock-skew attack, leap seconds — none addressed. V1 events have a `timestamp` field signed by the agent; the agent can lie about its clock. Atlas's Rekor anchor binds events to *Sigstore's* time, so there's a partial defence — but Doc B §2.5's bi-temporal mapping treats `event.timestamp` as ground-truth. Auditors will challenge this.

---

## Konkrete Vorschläge (specific edits/additions)

- **Doc B §2.1 → add explicit projector-determinism spec subsection (`§2.1.1 Determinism Contract`):** define the four sources of non-determinism (concurrent upsert ordering, entity-ID-collision resolution under reordered first-seen, floating-point/locale-dependent property serialisation, stable-sort fallback to insertion order) and require each be addressed by V2-α with a named test. The CI gate (§3.2) should hash a *canonical projected-graph form* whose spec is byte-pinned analogously to V1's `signing_input_byte_determinism_pin`.

- **Doc B §2.7 → add `§2.7.x Author-DID Signature Binding`:** specify the exact COSE_Sign1 envelope for V2 events. Either (a) the agent's DID Ed25519 key signs in addition to (counter-signs) the workspace's HKDF key, producing a multi-signer event (compatible with `docs/ARCHITECTURE.md` §10 V2 "Multi-signer events" planned surface), or (b) the V2 envelope binds `author_did` into the signing-input bytes so the workspace-key signature *commits* to the claimed DID. Currently the doc has `author_did` as an unauthenticated string in the payload — close this gap before V2-α ship.

- **Doc B §3.3 → soften the GDPR claim and cross-reference Doc C R-L-01:** change "satisfies V1's chain-of-custody" to "preserves V1's chain-of-custody while reducing GDPR exposure surface to the content_hash itself; whether content_hash alone constitutes personal data under GDPR Art. 4(1) is an open legal question (Doc C R-L-01, Doc C Q2)." Doc A §3.1 and Doc D §6 matrix "GDPR-by-design: ✓" should be downgraded to "GDPR-mitigated-by-design (pending EU counsel opinion)" until resolved.

- **Doc B §4 → re-baseline session estimates:** V2-α 5-8 sessions (not 3-4), V2-β 4-5 sessions (not 3-4 — read-side API + Mem0g + Explorer is more than three concerns), V2-γ 3-4 sessions, V2-δ 2-3 sessions if pursued. Total 14-20 sessions. Adjust Doc B §5 Welle 14b-iii alignment ("delays by ~6 sessions") to "delays by ~10-12 sessions" or move V2 to a separate parallel track.

- **Doc B §2.6 → require the Hermes Memory Skill to delegate signing to a local atlas-signer subprocess, not hold the DID private key directly:** add "Skill MUST NOT hold private keys in skill-state. Key material lives in the local atlas-signer instance (V1 pattern; HSM-capable per V1.10/V1.11). Skill calls `atlas-signer sign --derive-from-did <did>` analogous to `--derive-from-workspace`." Eliminates M-1.

- **Doc B §2.10 → add `§2.10.x Witness Performance & Bootstrap`:** specify (a) batch-cosignature protocol where N events are cosigned in one HTTP round-trip (signing-input is the merkle-root of the batch — analogous to Atlas's existing bundle_hash pattern); (b) workspace-genesis-event is exempt from witness threshold (or witnesses are pre-enrolled before the genesis event is admitted); (c) the cost model for 3-4 concurrent witnesses at 1K events/sec.

- **Doc B §2.8 → add an "Authorization model" subsection:** spec who can call each endpoint, default auth requirements, rate-limit defaults, and caching semantics. `GET /api/atlas/audit/:event_uuid` should be content-addressable (cacheable); `POST /api/atlas/query` needs per-caller rate-limits; `GET /api/atlas/passport/:did` needs an opinion on whether passport is workspace-private or public.

- **Doc B §2.3 + §3.1 → update fallback chain after Kuzu-archived:** replace "Kuzu MIT" with "ArcadeDB Apache-2.0 (Atlas-integration-untested), Memgraph BSL, HugeGraph Apache-2.0". Add explicit acceptance criterion: "Before V2-α merges, ArcadeDB compatibility spike must be completed (1-2 days) to verify the 'storage adapter is configuration-time' claim holds for at least one fallback."

- **Doc D §6 matrix → fix overclaims:**
  - Atlas Temporal column: "~" or "✓ (V2-δ — optional)" not bare "✓"
  - Atlas GDPR-by-design: "~ (mitigated, pending counsel)" not bare "✓"
  - Add a footnote explaining V2-δ-optional status so the reader doesn't assume the bi-temporal feature ships in V2-α/β/γ.

- **Doc B §3.18 → re-evaluate S3-conditional-write-claim:** acknowledge that PutObject-If-None-Match is AWS-S3-only (as of 2026-05; check current state of MinIO/R2/B2 conditional-write support). Either (a) Atlas commits to "AWS S3 only" for distributed Layer 1, (b) Atlas ships an abstraction over conditional-write primitives with adapter implementations per store, or (c) the per-workspace mutex is elevated to a distributed lock service (Postgres advisory locks, etcd, DynamoDB-Lock-Client). Pick one and document.

- **Doc B (new §2.11 or appendix) → enumerate WASM verifier changes required for V2:** which V2 features add new `VerifyEvidence::check` labels, which add new `TrustError` variants, which require WASM-API additions. Maps onto SEMVER-AUDIT-V1.0.md §1.1 and §1.3 — confirms which V2 changes are SemVer-minor (additive, default-lenient) vs SemVer-major (rare; should be avoided in V2).

---

## Offene Fragen für Phase 3

- **Q-ARCH-1: Should V2-α ship a `projected_graph_canonical_form_v1` spec + byte-pin test, analogous to V1's signing-input pin?** Context: Doc B §2.1 + §3.2 claim "deterministically rebuildable" but the canonicalisation function is unspecified, leaving R-A-01 LOW-detectability. Recommendation-for-Phase-3: **accept** — make this a V2-α blocker. The CI gate alone is insufficient without a byte-pinned canonical form.

- **Q-ARCH-2: Should V2 events carry an explicit author-DID signature, or bind `author_did` into the workspace-key signing-input bytes?** Context: Doc B §2.7 has `author_did` as unauthenticated payload metadata, breaking the passport-coherence claim under workspace-key compromise (C-2). Recommendation-for-Phase-3: **accept** — option (b) (signing-input binding) is the lighter weight; option (a) (multi-signer event) is the more general V2 pattern already named in `docs/ARCHITECTURE.md` §10. Pick (a) if multi-signer is otherwise on the roadmap; (b) otherwise.

- **Q-ARCH-3: Should "GDPR-by-design" claims in Doc A §3.1 and Doc D §6 matrix be downgraded to "GDPR-mitigated" pending EU counsel opinion?** Context: C-3 + Doc C Q2. The architecturally honest answer is yes. Recommendation-for-Phase-3: **accept** — also blocks public marketing material making the strong claim until the legal opinion lands.

- **Q-ARCH-4: Should the Hermes Memory Skill delegate signing to a local atlas-signer subprocess, or hold the DID private key in skill state?** Context: M-1; the doc currently allows the key in skill-state, creating a typosquat blast-radius. Recommendation-for-Phase-3: **accept** the subprocess pattern. Adopt the V1 atlas-signer interface (already HSM-capable) as the mandatory signing surface for the skill.

- **Q-ARCH-5: Should the fallback graph-store chain be re-validated post-Kuzu, with a mandatory ArcadeDB compatibility spike before V2-α merge?** Context: H-5 + Doc D §4.4. The "storage adapter is configuration-time" claim is unverified for any non-FalkorDB store. Recommendation-for-Phase-3: **accept** + plan a 1-2 day spike in V2-α scope.

- **Q-ARCH-6: Should V2's session estimate be re-baselined from 10-14 to 14-20, and Welle 14b-iii alignment re-evaluated?** Context: H-3 — the V2-α 3-4-session estimate is optimistic by 2x given the projector's determinism-spec scope. Recommendation-for-Phase-3: **accept** the larger estimate; let Nelson decide whether option-2 (roll V2-α+β into Welle 14b-iii, delay landing 10-12 sessions) or option-3 (parallel tracks, landing ships without graph layer) is preferred.

- **Q-ARCH-7: Should atlas-mcp-server's V2 MCP tools be online-only, or carry a local FalkorDB/Mem0g replica?** Context: H-2. Doc B leaves this implicit. Recommendation-for-Phase-3: **defer** to V2-β scoping — but flag that "online-only" is a capability narrowing from V1's offline `verify_trace` and must be documented as such.

- **Q-ARCH-8: Should witness federation support batch-cosignature (N events, one merkle-root cosignature) to keep R-O-01 1K-writes/sec target reachable?** Context: M-2. Without batching, 3-4 witness round-trips × 1K events/sec is operationally hostile. Recommendation-for-Phase-3: **accept** as a V2-γ design requirement; the merkle-root-cosignature pattern is already adjacent to Atlas's bundle_hash anchor pattern (V1.5).

- **Q-ARCH-9: Should Atlas adopt Lyrie ATP DIDs (`did:lyrie:*`) as an interoperable agent-identity scheme, in addition to `did:atlas:*`?** Context: Bind-spot #7 + Doc D §5.4 ("strong partner candidate"). Recommendation-for-Phase-3: **research-more** in Phase 3 — confirm Lyrie ATP's stability + IETF-draft status; if stable, ship Atlas DID resolution that accepts either method and Atlas-attests `did:lyrie:*` agents in the passport.

- **Q-ARCH-10: Should the MCP V2 tool count be collapsed (`query_graph + query_entities + query_provenance + get_timeline → single atlas_query with mode`) to preserve LLM-host token budget?** Context: L-5 + Doc B Q-MCP-1. Recommendation-for-Phase-3: **accept** — the resulting tool surface (write_node, verify_trace, atlas_query, get_agent_passport) is cleaner and four-tools-not-seven is friendlier to host-side LLM context budgets.

- **Q-ARCH-11: Should the read-side API specify caching headers (ETag / Cache-Control / Vary) per endpoint, and rate-limit defaults?** Context: H-4. Recommendation-for-Phase-3: **accept** as V2-β scoping — production deployments will need this; spec it pre-implementation.

- **Q-ARCH-12: Should `events.jsonl` distributed Layer 1 (§3.18) be deferred to V2-post-α (V2.1+) rather than V2-α, given S3-conditional-write portability concerns?** Context: M-3. The V2-α scope can ship with single-file-per-workspace events.jsonl and ship distributed-Layer-1 later. Recommendation-for-Phase-3: **accept** the deferral; V2-α is already overscoped (H-3).

---

**Reviewer:** Architect critique agent. **Date:** 2026-05-12. **Convergence-criteria check:** ≥5 Stärken (7 ✓), ≥3 CRITICAL/HIGH (3 + 5 ✓), ≥5 blind spots (10 ✓), ≥3 concrete edits (10 ✓), ≥5 open questions for Phase 3 (12 ✓).
