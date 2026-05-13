# Atlas v2.0.0-alpha.1 — Release Notes

> **Released:** 2026-05-13.
> **Tag:** `v2.0.0-alpha.1` (signed via SSH-Ed25519 path; key `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`).
> **Status:** First pre-release of Atlas's V2 line. Engineering / auditor / operator evaluation. Public marketing materials pending counsel-validated language refinement per [`docs/V2-MASTER-PLAN.md`](V2-MASTER-PLAN.md) §5.

## Headline

Atlas v2.0.0-alpha.1 ships the **cryptographic projection-state verification primitive end-to-end**. A third-party verifier with the offline WASM verifier + `events.jsonl` + `pubkey-bundle.json` can now independently re-project a trace and produce a structured `Match` / `Mismatch` outcome per signed `ProjectorRunAttestation` event — drift detected cryptographically, not just by CI convention.

V1's trust property (signed events + Ed25519 + COSE_Sign1 + deterministic CBOR + blake3 hash chain + Sigstore Rekor anchoring + witness cosignature + offline WASM verifier) is **preserved unchanged**. V2-α-alpha.1 is an additive cryptographic layer above V1, not a replacement.

## What's in v2.0.0-alpha.1

The release packages 8 V2-α wellen shipped on 2026-05-12 through 2026-05-13:

| Welle | Deliverable | Cryptographic primitive |
|---|---|---|
| 1 | Agent-DID Schema Foundation | `did:atlas:<blake3-pubkey-hash>` agent identity bound into signing input alongside `kid` (Phase 2 Security H-1) |
| 2 | ArcadeDB vs FalkorDB Comparative Spike | Strategic Layer-2 DB choice (ArcadeDB Apache-2.0 primary, FalkorDB SSPLv1 fallback) per `DECISION-DB-4` |
| 3 | Atlas Projector Skeleton + Canonicalisation Byte-Pin | `graph_state_hash` deterministic over canonical CBOR (RFC 8949 §4.2.1) |
| 4 | `ProjectorRunAttestation` Event-Schema + Verifier-Side Parsing | Signed event-kind asserting `(projector_version, head_event_hash) → graph_state_hash`; binds projection state into Layer-1 trust chain |
| 5 | Emission Pipeline | `events.jsonl → GraphState → attestation payload` library-side end-to-end |
| 6 | Projector-State-Hash CI Gate | `verify_attestations_in_trace` — re-projects + compares against attested hash; closes V2-α security loop |
| 7 | atlas-signer `emit-projector-attestation` CLI | Producer-side ergonomic: ONE shell command to read events.jsonl, project, build attestation, sign, emit |
| 8 | v2.0.0-alpha.1 Ship | Version bump + CHANGELOG promotion + this release-notes doc |

Per-welle details in [`CHANGELOG.md`](../CHANGELOG.md) under the `[2.0.0-alpha.1]` section.

## The V2-α security model in one diagram

```
+-------------------------------+
| Issuer (Atlas operator)       |
|                               |
| 1. Sign events                |
|    (V1 mechanism, unchanged)  |
| 2. atlas-projector projects   |
|    events into GraphState     |
|    (Welles 3 + 5)             |
| 3. atlas-signer emits a       |
|    signed ProjectorRunAttestation|
|    (Welles 4 + 7)             |
+--------------+----------------+
               |
               v
+-------------------------------+
| events.jsonl (Layer 1)         |
|  - signed events               |
|  - ProjectorRunAttestation     |
|    events interspersed         |
|  - all Rekor-anchored          |
+--------------+----------------+
               |
               v
+-------------------------------+
| Third-party verifier           |
|                               |
| 1. atlas-trust-core verify_trace |
|    (V1 mechanism, unchanged):  |
|    - signatures                |
|    - hash chain                |
|    - anchors                   |
|    - V2-α: also validates the  |
|      ProjectorRunAttestation   |
|      payload format            |
|      (Welle 4)                 |
| 2. atlas-projector             |
|    verify_attestations_in_trace|
|    (Welle 6):                  |
|    - re-project the events     |
|    - recompute graph_state_hash|
|    - compare against attested  |
|    - return GateResult         |
|      { Match | Mismatch | Failed } |
+-------------------------------+
```

After v2.0.0-alpha.1: **Match outcome = cryptographic verification that the issuer's projector and a fresh re-projection agree on what the graph state is for this set of signed events.** Drift = either the trace was tampered (V1's signature/hash chain catches this) or the projector implementation diverged (byte-determinism CI pins catch this upstream).

## New public-API surface

Cross-reference [`docs/SEMVER-AUDIT-V1.0.md`](SEMVER-AUDIT-V1.0.md) §10 for every new `pub` item with V2-α-Additive tag. Highlights:

**atlas-trust-core (new):**
- `pub mod agent_did` — Welle 1 (`AGENT_DID_PREFIX`, `agent_did_for`, `parse_agent_did`, `validate_agent_did`)
- `pub mod projector_attestation` — Welle 4 (`PROJECTOR_RUN_ATTESTATION_KIND`, `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION`, `ProjectorRunAttestation`, `parse_projector_run_attestation`, `validate_projector_run_attestation`)
- `AtlasEvent.author_did: Option<String>` field — Welle 1 (additive schema; `#[serde(deny_unknown_fields)]` policy unchanged = SemVer-major break for V1.0 readers, by design)
- `AtlasPayload::ProjectorRunAttestation { ... }` enum variant — Welle 4
- `TrustError::AgentDidFormatInvalid { did, reason }` — Welle 1
- `TrustError::ProjectorAttestationInvalid { reason }` — Welle 4

**atlas-projector (NEW crate):**
- `pub const PROJECTOR_SCHEMA_VERSION = "atlas-projector-v1-alpha"` — Welle 3
- `pub const CRATE_VERSION = env!("CARGO_PKG_VERSION")` — Welle 7-fix (structural binding for downstream consumers)
- `pub struct GraphState` + `GraphNode` + `GraphEdge` — Welle 3
- `pub fn build_canonical_bytes(state)` + `graph_state_hash(state)` — Welle 3
- `pub mod replay` — Welle 5 (`parse_events_jsonl`)
- `pub mod upsert` — Welle 5 (`apply_event_to_state`, `project_events`)
- `pub mod emission` — Welle 5 (`build_projector_run_attestation_payload`)
- `pub mod gate` — Welle 6 (`verify_attestations_in_trace`, `GateResult`, `GateStatus`)
- `pub enum ProjectorError` (`#[non_exhaustive]`) — 8 variants across Welles 3+5+6

**atlas-signer (new CLI surface):**
- `emit-projector-attestation` subcommand — Welle 7

**Byte-determinism CI gates (7 total covering V1 + V2-α):**
1. `cose::tests::signing_input_byte_determinism_pin` (V1)
2. `cose::tests::signing_input_byte_determinism_pin_with_author_did` (Welle 1)
3. `anchor::tests::chain_canonical_body_byte_determinism_pin` (V1.7)
4. `anchor::tests::chain_head_for_byte_determinism_pin` (V1.7)
5. `pubkey_bundle::tests::bundle_hash_byte_determinism_pin` (V1.9)
6. `atlas_projector::canonical::tests::graph_state_hash_byte_determinism_pin` (Welle 3)
7. `cose::tests::signing_input_byte_determinism_pin_with_projector_attestation` (Welle 4)

## Wire-format compatibility

**V1.0 verifiers reading V2-α events:**
- V1-shaped events (no `author_did`, V1 payload kinds only) → forward-compatible, deserialise + verify correctly
- V2-α events with `author_did = Some(_)` OR `payload.type == "projector_run_attestation"` → V1.0 verifier rejects via `unknown_field` (`#[serde(deny_unknown_fields)]` policy)

This is the explicit SemVer-major break committed by v2.0.0-alpha.1. Downstream consumers MUST upgrade to v2.0.0-alpha.1 to read V2-α events.

**V2-α verifiers reading V1 events:**
- Fully backward-compatible. All V1-shaped events deserialise + verify correctly through the V2-α verifier path. No regression.

## Operator quickstart

**Sign an Atlas event (V1 pattern unchanged):**
```bash
atlas-signer sign --workspace ws-q1-2026 --derive-from-workspace ws-q1-2026 \
  --payload '{"type":"node_create","node":{"id":"alice"}}'
```

**Emit a signed `ProjectorRunAttestation` (V2-α new):**
```bash
atlas-signer emit-projector-attestation \
  --events-jsonl trace/events.jsonl \
  --workspace ws-q1-2026 \
  --derive-from-workspace ws-q1-2026 \
  --head-event-hash <hex> \
  >> trace/events.jsonl
```

`--kid` auto-derives from `--derive-from-workspace`. The output JSON line is the signed AtlasEvent ready for append.

**Verify a trace's attestations (V2-α new, library API):**
```rust
use atlas_projector::{verify_attestations_in_trace, GateStatus};
let results = verify_attestations_in_trace(workspace_id, &trace)?;
for r in &results {
    match r.status {
        GateStatus::Match => println!("✓ attestation {} verified", r.event_id),
        GateStatus::Mismatch => println!("✗ DRIFT on {}: attested {} vs recomputed {}",
            r.event_id, r.attested_hash, r.recomputed_hash),
        GateStatus::AttestationParseFailed => println!("✗ malformed attestation {}", r.event_id),
    }
}
```

## What's NOT in v2.0.0-alpha.1

Per [`docs/V2-MASTER-PLAN.md`](V2-MASTER-PLAN.md) §6 V2-α Foundation scope is "5-8 sessions"; v2.0.0-alpha.1 ships 8 sessions of work. The following are explicitly deferred to V2-β / V2-γ / V2-δ:

- **ArcadeDB driver integration.** Layer-2 storage is currently in-memory only. ArcadeDB-backed `GraphState` is a V2-β candidate; the v2.0.0-alpha.1 security primitive works equally well on either storage backend (per Welle 6 design).
- **Mem0g Layer-3 cache.** 91% latency reduction per Mem0g's Locomo benchmark (cited honestly per `DECISION-DB-3`); V2-β candidate.
- **6 Read-API endpoints** (per Master Vision §5.4) — V2-β candidate.
- **5 MCP V2 tools** (per Master Vision §5.5) — V2-β candidate.
- **Expanded event-kind support** (`annotation_add`, `policy_set`, `anchor_created` in atlas-projector upsert layer) — V2-β candidate.
- **Agent Passports** (`GET /api/atlas/passport/:agent_did`) + revocation mechanism per `DECISION-SEC-1` — V2-γ candidate.
- **Regulator-Witness Federation** (M-of-N threshold enrolment per `DECISION-SEC-3`) — V2-γ candidate.
- **Hermes-skill v1** (credibility-asset GTM positioning per `DECISION-BIZ-1`) — V2-γ candidate.
- **Cedar policy at write-time** + post-quantum hybrid Ed25519+ML-DSA-65 co-sign — V2-δ candidate.
- **Parallel-projection design** for >10M event scenarios (completes `DECISION-ARCH-1` triple-hardening's third leg) — V2-β candidate.

## Pre-counsel-review disclaimer

Per [`docs/V2-MASTER-PLAN.md`](V2-MASTER-PLAN.md) §5 + [`DECISION-COUNSEL-1`](../.handoff/decisions.md), Atlas commits to a €30-80K counsel engagement (6-8 weeks structured) pre-V2-α-public-materials covering:
1. GDPR Art. 4(1) hash-as-personal-data opinion (Path A redesign vs Path B defence)
2. AILD→PLD reframe + insurance-regulation engagement strategy
3. Art. 43 conformity-assessment-substitution liability disclaimer drafting
4. Schrems II / cross-border SCC + DPA templates
5. Verbatim Art. 12 + Annex IV marketing copy review
6. Witness-federation EU regulatory positioning brief
7. DPIA + FRIA template drafting

**This release-notes doc is engineering-perspective.** Any public marketing material derived from it MUST be counsel-validated before publication. The technical claims (cryptographic primitives, byte-determinism, signature binding) are stable; the regulatory-claim phrasing is the layer subject to counsel review.

## Reproducibility

Atlas v2.0.0-alpha.1 is reproducible from source. Recipe (Windows + Linux + macOS, Rust 1.85+):

```bash
git clone https://github.com/ThePyth0nKid/atlas.git
cd atlas
git checkout v2.0.0-alpha.1
git verify-tag v2.0.0-alpha.1    # → Good ed25519 signature
cargo build --workspace --release
cargo test --workspace
# All 7 byte-determinism CI pins must pass byte-identical.
```

Public release artefacts (post-tag-push, auto-fired via `wasm-publish.yml`):
- `@atlas-trust/verify-wasm@2.0.0-alpha.1` on npm (web + node targets)
- SLSA Build L3 provenance attestation in Sigstore Rekor

## Migration from v1.0.1

V2-α is an additive layer. Existing v1.0.1 deployments can adopt v2.0.0-alpha.1 incrementally:

1. **No-op upgrade path:** continue producing V1-shaped events (no `author_did`, V1 payload kinds only). v2.0.0-alpha.1 verifier accepts these unchanged. No behaviour change.
2. **V2-α-aware producers:** opt-in to `--derive-from-workspace` + signed `ProjectorRunAttestation` emission via the new CLI subcommand. Resulting events.jsonl carries V2-α attestations that V2-α-aware verifiers can independently verify.
3. **V2-α-aware verifiers:** upgrade `@atlas-trust/verify-wasm` to v2.0.0-alpha.1. New `verify_attestations_in_trace` library API available for projection-integrity verification.

V1.0 verifiers WILL reject V2-α events with `author_did` or V2-α-only payload kinds (per `deny_unknown_fields` policy). This is the SemVer-major break.

## What comes next

V2-β (2026-Q3 target, scope-bounded per future Master-Plan iterations) candidates:
- ArcadeDB Layer-2 storage backend
- Mem0g Layer-3 semantic cache
- Read-API + MCP V2 tools
- Expanded event-kind support
- Parallel-projection design

V2-γ (regulator-witness federation; Agent Passports; Hermes-skill v1) and V2-δ (Cedar policy + post-quantum) per Master Plan §6.

Per Master Plan §5, **counsel engagement** progresses on a parallel Nelson-led track and is **pre-V2-α-public-materials blocking**. v2.0.0-alpha.1 is the first release that engineering-side can hand to counsel for opinion-drafting input.

## References

- [`docs/V2-MASTER-PLAN.md`](V2-MASTER-PLAN.md) — V2 strategic plan (source-of-truth)
- [`docs/WORKING-METHODOLOGY.md`](WORKING-METHODOLOGY.md) — reusable 4-phase iteration pattern
- [`docs/SEMVER-AUDIT-V1.0.md`](SEMVER-AUDIT-V1.0.md) §10 — V2-α additive public-API surface
- [`.handoff/v2-master-vision-v1.md`](../.handoff/v2-master-vision-v1.md) — Phase-3 synthesis output
- [`.handoff/decisions.md`](../.handoff/decisions.md) — 23 ACCEPT/MODIFY/DEFER decisions
- Per-welle plan docs in `.handoff/v2-alpha-welle-{1..8}-plan.md`

---

**End of v2.0.0-alpha.1 release notes.** Operator runbook updates + demo materials forthcoming as separate follow-up wellen.
