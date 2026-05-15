# Atlas v2.0.0-beta.1 — Release Notes

> **Released:** 2026-05-15.
> **Tag:** `v2.0.0-beta.1` (signed via SSH-Ed25519 path; key `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`).
> **Status:** First V2-β pre-release of Atlas's V2 line. Engineering / auditor / operator evaluation. Public marketing materials pending counsel-validated language refinement per [`DECISION-COUNSEL-1`](../.handoff/decisions.md) — this internal-milestone tag is NOT a public material under that gate.

## Headline

**v2.0.0-beta.1 — V2-β tripod operational (Layer 2 ArcadeDB + Layer 3 Mem0g scaffold + verifier-rebuild).**

Atlas v2.0.0-beta.1 ships the V2-β tripod end-to-end on master: **Layer 1** verifier-rebuild carrying forward V2-α-α.1 + V2-α-α.2 (signed events + Ed25519 + COSE_Sign1 + byte-deterministic CBOR + hash-chained edges + Sigstore Rekor anchoring + witness cosignature + offline WASM verifier + ProjectorRunAttestation + agent-DID); **Layer 2** ArcadeDB graph-backend (W17a `GraphStateBackend` trait + W17b `ArcadeDbBackend` driver + W17c CI smoke + bench), **operational** — `ArcadeDbBackend` reproduces the V14 graph-state-hash byte-pin through the cross-backend trait-conformance test; **Layer 3** Mem0g semantic-cache, **scaffold-shipped** — trait surface + protocol + dispatch wiring + supply-chain path all production-shape, W18c Phase A supply-chain constant pins lifted (2026-05-15), `/api/atlas/semantic-search` returns HTTP 501 until W18c Phase B + Phase D close the operational gate.

V1's trust property is **preserved unchanged**. V2-α-α.1's cryptographic-projection-state primitive is **preserved unchanged**. V2-β-1 layers the storage-backend abstraction (Layer 2) + semantic-cache scaffold (Layer 3) above the V1+V2-α core without altering any V1 trust contract.

## At-a-glance — what changed since v2.0.0-alpha.2

- **Layer 2 ArcadeDB graph backend operational.** ArcadeDB Apache-2.0 embedded mode via W17a `GraphStateBackend` trait → W17b driver impl → W17c CI smoke + bench. Cross-backend trait-conformance test pins byte-determinism through both `InMemoryBackend` AND `ArcadeDbBackend`. See `docs/V2-BETA-ARCADEDB-SPIKE.md` + ADR-Atlas-011.
- **Layer 3 Mem0g semantic cache scaffold-shipped + supply-chain pins lifted.** New workspace member `crates/atlas-mem0g/` (~2300 LOC across 4 src modules + 3 test modules). W18b implementation + W18c Phase A pin-lift (2026-05-15). Embedder still fails closed pending W18c Phase B `fastembed::TextEmbedding::try_new_from_user_defined` wiring — **scaffold-honesty disclosure mandatory** (see "Layer 3 scaffold posture" below).
- **`POST /api/atlas/semantic-search` Read-API endpoint.** Production-shape request/response contract; 501 scaffold-response until W18c Phase B + Phase D activate the LanceDB ANN body. Contract stable across V2-β minor versions.
- **`embedding_erased` event-kind dispatch arm + `GraphState.embedding_erasures` BTreeMap.** Audit-trail surface for GDPR Art. 17 erasure with V14 byte-pin invariant preserved (canonical-bytes omits-when-empty).
- **Workspace + 4 package.json version bump** `2.0.0-alpha.2` → `2.0.0-beta.1`. SemVer prerelease (`2.0.0-beta.1` < `2.0.0` per SemVer §11).

## Layer 1 — Verifier-rebuild

Carries forward unchanged from v2.0.0-alpha.2:

- Ed25519 + COSE_Sign1 signatures, byte-deterministic CBOR signing-input (RFC 8949 §4.2.1)
- blake3 hash-chained edges
- Sigstore Rekor anchoring with RFC 6962 Merkle inclusion proofs
- Witness cosignature with separate Ed25519 key (trust-domain separation by process)
- Offline WASM verifier (`@atlas-trust/verify-wasm`)
- V2-α agent-DID stamping (`did:atlas:<blake3-pubkey-hash>` bound into signing input alongside `kid`)
- V2-α `ProjectorRunAttestation` event-kind + projector-state-hash CI gate (`verify_attestations_in_trace`)

**Byte-determinism CI gates unchanged:** all 7 V1+V2-α pins (`cose::signing_input_byte_determinism_pin`, `…_with_author_did`, `…_with_projector_attestation`, `anchor::chain_canonical_body_byte_determinism_pin`, `anchor::chain_head_for_byte_determinism_pin`, `pubkey_bundle::bundle_hash_byte_determinism_pin`, `atlas_projector::canonical::graph_state_hash_byte_determinism_pin = 8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`) reproduce byte-identical.

## Layer 2 — ArcadeDB graph backend — OPERATIONAL

| Welle | Deliverable | Cryptographic / contract effect |
|---|---|---|
| W17a | `pub trait GraphStateBackend` + `InMemoryBackend` impl | Abstracts state-mutation surface (upsert_node, upsert_edge, upsert_anchor, etc.) behind object-safe `Send + Sync` trait. Reference: `crates/atlas-projector/src/backend.rs`. |
| W17b | `ArcadeDbBackend` driver | Apache-2.0 ArcadeDB embedded-mode driver implementing `GraphStateBackend`. Cypher-validator consolidation reused. Reference: `crates/atlas-projector/src/arcadedb_backend.rs`. |
| W17c | CI smoke workflow + bench | `atlas-arcadedb-smoke.yml` (workspace-internal; promotion-to-required gated by ≥3 stable runs per operator-runbook). |
| W17a-c | `backend_trait_conformance` test | Cross-backend invariant: byte-pin `8962c168…e013ac4` reproduces through BOTH `InMemoryBackend` AND `ArcadeDbBackend`. ArcadeDB hand-off is byte-deterministic. |

**Status:** OPERATIONAL. Full ArcadeDB path validated end-to-end. Reference `docs/V2-BETA-ARCADEDB-SPIKE.md` + ADR-Atlas-011.

## Layer 3 — Mem0g semantic cache — SCAFFOLD-SHIPPED (scaffold-honesty disclosure)

| Welle | Deliverable | Status |
|---|---|---|
| W18b | `crates/atlas-mem0g/` workspace member: SemanticCacheBackend trait + protocol + secure-delete protocol + `embedding_erased` event-kind dispatch + atlas-web semantic-search Read-API | SHIPPED — production-shape; W17a-pattern Phase-A-scaffold |
| W18c Phase A | Supply-chain constants lifted: `BAAI/bge-small-en-v1.5 @ 5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`, 9 compile-in pins (HF revision SHA-1 + ONNX SHA-256 + 3 tokenizer-file SHA-256 + 4 LFS URLs) | SHIPPED 2026-05-15 — pre-merge consolidated in Phase 14.5 |
| W18c Phase B | `fastembed::TextEmbedding::try_new_from_user_defined` wiring | PENDING (parallel-track) |
| W18c Phase C | V1-V4 verification CI matrix (Linux + Windows + macOS embedding determinism) | PENDING (parallel-track) |
| W18c Phase D | LanceDB ANN/search body fill-in (`tokio::task::spawn_blocking` wrapped — never `Handle::current().block_on()`) | PENDING (parallel-track) |

**Scaffold posture (LOUD):** Layer 3 Mem0g semantic-cache: trait + protocol + dispatch surface + state extension production-shape; `/api/atlas/semantic-search` Read-API returns HTTP 501 with structured `Mem0gError::Embedder("supply-chain gate: …")`-style error message pointing at the operator-runbook. The supply-chain pins ARE lifted (W18c Phase A SHIPPED via PR #100 `28700ae`) and structurally enforced (length + lowercase-hex + `https://huggingface.co/` origin + revision-SHA-in-path invariants asserted at test time), but the embedder still fails-closed until Phase B wires `fastembed::TextEmbedding::try_new_from_user_defined` over the verified ONNX + tokenizer bytes. ArcadeDB Layer 2 (W17a-c) is fully operational. W18c parallel-track activates Layer 3 operationally; v2.0.0-beta.2 (or later) carries that operational mode if the engineering pipeline chooses to ship before V2-γ.

Reference: `docs/V2-BETA-MEM0G-SPIKE.md` + ADR-Atlas-012 + `.handoff/v2-beta-welle-18c-plan.md`.

## W18c parallel-track pointer

W18c Phase A (supply-chain constant lift) SHIPPED 2026-05-15. W18c Phases B / C / D are queued as a **parallel-track to the v2.0.0-beta.1 ship**, NOT a beta.1 ship gate. The engineering-pipeline choice for the next welle is one of:

1. **W18c Phase B → C → D sequence** — activates Layer 3 operationally; ships as v2.0.0-beta.2 once green.
2. **V2-γ planning kick-off** — Agent Passports, Regulator-Witness Federation, Hermes-skill v1.

Either path keeps the v2.0.0-beta.1 internal-milestone tag stable per the tag-immutability contract (V1.17 Welle B).

Reference: `.handoff/v2-beta-welle-18c-plan.md`.

## Upgrade guide from v2.0.0-alpha.2

v2.0.0-beta.1 is SemVer-compliant prerelease (`2.0.0-beta.1` < `2.0.0` per SemVer §11; prereleases sort before final). Downstream consumers:

- **npm:** `npm install @atlas-trust/verify-wasm@2.0.0-beta.1` (or `^2.0.0-alpha.2` consumers WILL pick up beta.1 because prereleases match same-major-minor-patch caret).
- **cargo:** `[dependencies] atlas-trust-core = "2.0.0-beta.1"` (explicit prerelease pin recommended; cargo caret ranges from a prerelease — e.g. `^2.0.0-alpha.2` — DO allow newer prereleases of the same `[major.minor.patch]`, but cargo does NOT auto-pick prereleases from a non-prerelease range. For beta consumers, explicit pinning is recommended.)

**No source-break for V2-α-α.2 consumers.** The V2-α-α.1 → V2-α-α.2 → V2-β-1 path is largely additive: Layer 2 + Layer 3 scaffolds add public-API surface; no V1 trust surface changes. See [`docs/SEMVER-AUDIT-V2.0-beta.md`](SEMVER-AUDIT-V2.0-beta.md) for the V2-β-1 additive-surface contract.

**Wire-format compatibility:** V2-α-α.2 events deserialise + verify correctly through v2.0.0-beta.1 verifier. V2-β-1 events with `embedding_erased` event-kind payloads will surface `MissingPayloadField` in V2-α verifiers reading the new kind (additive event-kind under existing dispatch DAG; not a wire-format break for V1-shaped or V2-α-shaped events).

## Verifier byte-pin invariant preserved

The V14 graph-state-hash byte-determinism CI pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces through BOTH `InMemoryBackend` AND `ArcadeDbBackend` per the `backend_trait_conformance` byte-pin test. The hand-off across the storage abstraction is byte-deterministic — auditors verify identical projector-state-hash regardless of which backend produced the GraphState.

All 7 V1+V2-α byte-determinism CI pins remain pinned + unchanged from v2.0.0-alpha.2 baseline.

## What's NOT in v2.0.0-beta.1

Explicitly deferred (consistent with `docs/V2-MASTER-PLAN.md` §6 + `.handoff/decisions.md`):

- **Layer 3 Mem0g operational mode.** W18c Phase B `try_new_from_user_defined` wiring + Phase C cross-platform determinism CI + Phase D LanceDB ANN body fill-in are parallel-track to beta.1. Embedder fails-closed until Phase B lands.
- **Agent Passports** (`GET /api/atlas/passport/:agent_did`) + revocation mechanism per `DECISION-SEC-1` — V2-γ candidate.
- **Regulator-Witness Federation** (M-of-N threshold enrolment per `DECISION-SEC-3`) — V2-γ candidate.
- **Hermes-skill v1** (credibility-asset GTM positioning per `DECISION-BIZ-1`) — V2-γ candidate.
- **Graphiti optional integration** (per `DECISION-DB-5`) — V2-γ/δ candidate.
- **Cedar policy at write-time** + post-quantum hybrid Ed25519+ML-DSA-65 co-sign — V2-δ candidate.
- **Parallel-projection design implementation** (per ADR-Atlas-007; design SHIPPED, implementation deferred) — V2-β.2 / V2-γ candidate.

## Known limitations

- **`RULESET_VERIFY_TOKEN` cosmetic CI red.** Per Atlas Lesson #9, the `RULESET_VERIFY_TOKEN`-gated workflow surfaces a cosmetic-red status check that does NOT block PR merges. Atlas admin-merge protocol (`gh pr merge --admin`) treats this as expected-noise.
- **Counsel sign-off pending per `DECISION-COUNSEL-1`** for V2-β PUBLIC materials. This v2.0.0-beta.1 tag itself is an **internal engineering milestone, NOT a public material** under that gate; the counsel-blocker applies to marketing copy / regulatory positioning brief / DPIA + FRIA templates derived from v2.0.0-beta.1, not the tag itself.
- **`atlas-mem0g` crate version `0.1.0`** independent of workspace `2.0.0-beta.1` — Layer 3 crate is intentionally not on the workspace.package version because its public-API surface is **Internal** during W18c parallel-track (see SEMVER-AUDIT-V2.0-beta.md §AtlasEmbedder).

## Supply chain

`@atlas-trust/verify-wasm@2.0.0-beta.1` will be published to npm with **SLSA Build L3** provenance attached via Sigstore Rekor on the auto-fired `wasm-publish.yml` workflow (race-fix from W11 + ADR-Atlas-008 proven through V2-α-α.2 ship).

V2-β-specific supply-chain hardening:
- **W18c Phase A pins** (9 compile-in constants for the BAAI/bge-small-en-v1.5 ONNX + tokenizer bundle) are structurally enforced at test time. Any future refactor that reintroduces placeholder strings trips assertions; the embedder fails-closed until Phase B activates.
- **Atlas-controlled supply-chain verification path** (per ADR-Atlas-012 §4 sub-decision #2): real SHA-256 via `sha2::Sha256` + RFC 6234 test vectors, TLS-pinned via `reqwest::https_only(true)`, OS CSPRNG (`getrandom`) for secure-delete overwrites.

## Verify the release

Post-tag-push + post-publish:

```bash
# Verify the signed Git tag
git verify-tag v2.0.0-beta.1
# → Good "ed25519" signature from SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg

# Verify the npm publish + SLSA L3 provenance
npm audit signatures @atlas-trust/verify-wasm@2.0.0-beta.1

# Reproduce from source
git clone https://github.com/ThePyth0nKid/atlas.git
cd atlas
git checkout v2.0.0-beta.1
cargo build --workspace --release
cargo test --workspace                 # all V1+V2-α byte-determinism pins pass byte-identical
cargo test -p atlas-projector --test backend_trait_conformance byte_pin   # cross-backend byte-pin
```

Public release artefacts (post-tag-push, auto-fired via `wasm-publish.yml`):
- `@atlas-trust/verify-wasm@2.0.0-beta.1` on npm (web + node targets)
- SLSA Build L3 provenance attestation in Sigstore Rekor

## Acknowledgements / contributors

See [`CHANGELOG.md`](../CHANGELOG.md) under the `[2.0.0-beta.1] — 2026-05-15` section for the per-welle contributor narrative covering Phases 4 → 14.5 (W12 Read-API endpoints, W13 MCP V2 tools, W14 expanded projector event-kinds, W15 Cypher-validator consolidation, W16 ArcadeDB spike, W17a/b/c ArcadeDB driver, W18b Mem0g scaffold, W18c-A supply-chain lift, Phase 14.5 consolidation).

## References

- [`CHANGELOG.md`](../CHANGELOG.md) `[2.0.0-beta.1]` — per-welle narrative
- [`docs/SEMVER-AUDIT-V2.0-beta.md`](SEMVER-AUDIT-V2.0-beta.md) — V2-β-1 additive public-API surface
- [`docs/SEMVER-AUDIT-V1.0.md`](SEMVER-AUDIT-V1.0.md) — V1 baseline + V2-α additive surface
- [`docs/V2-ALPHA-1-RELEASE-NOTES.md`](V2-ALPHA-1-RELEASE-NOTES.md) — V2-α-α.1 release content (cryptographic projection-state primitive)
- [`docs/V2-ALPHA-2-RELEASE-NOTES.md`](V2-ALPHA-2-RELEASE-NOTES.md) — V2-α-α.2 release content (operator runbook + parallel-projection design ADR + wasm-publish race fix)
- [`docs/V2-MASTER-PLAN.md`](V2-MASTER-PLAN.md) — V2 strategic plan
- [`docs/V2-BETA-ARCADEDB-SPIKE.md`](V2-BETA-ARCADEDB-SPIKE.md) — Layer 2 spike + ADR-Atlas-011
- [`docs/V2-BETA-MEM0G-SPIKE.md`](V2-BETA-MEM0G-SPIKE.md) — Layer 3 spike + ADR-Atlas-012
- [`docs/V2-BETA-ORCHESTRATION-PLAN.md`](V2-BETA-ORCHESTRATION-PLAN.md) — V2-β orchestration framework
- [`.handoff/decisions.md`](../.handoff/decisions.md) — 28 ACCEPT/MODIFY/DEFER decisions incl. `DECISION-ARCH-W18c-A`
- [`.handoff/v2-beta-welle-18c-plan.md`](../.handoff/v2-beta-welle-18c-plan.md) — W18c parallel-track (Phase A SHIPPED 2026-05-15; B/C/D queued)
- Per-welle plan docs in `.handoff/v2-beta-welle-{9..19}-plan.md`

---

**End of v2.0.0-beta.1 release notes.** W18c parallel-track Phase B/C/D + V2-γ planning are the post-beta.1 work-streams.
