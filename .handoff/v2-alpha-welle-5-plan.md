# V2-α Welle 5 — Plan-Doc (Atlas-Projector Emission: events.jsonl → GraphState + ProjectorRunAttestation construction)

> **Status: DRAFT 2026-05-13.** Awaiting Nelson's confirmation before merge.
> **Master Plan reference:** `docs/V2-MASTER-PLAN.md` §6 V2-α Foundation. Session 5 of 5–8.
> **Driving decisions:** `DECISION-ARCH-1` triple-hardening (canonicalisation byte-pin ✓ Welle 3 + ProjectorRunAttestation ✓ Welle 4 verifier + emission this welle); Welle 2 §3.5 critical caveat (logical-identifier sort key per `entity_uuid = blake3(workspace_id || event_uuid || kind)`, NOT `@rid`).

Welle 5 **bridges Welles 1 + 3 + 4 into one working pipeline**: read Atlas-signed events from `events.jsonl`, idempotently project them onto an in-memory `GraphState`, and construct a `ProjectorRunAttestation` payload that any third-party verifier can consume. After Welle 5, Atlas can demonstrate the full chain end-to-end: signed events → projection → emitted attestation → independent verification.

**Why this as Welle 5** (rather than ArcadeDB integration OR atlas-signer CLI flag):
- Brückt all 4 prior wellen — schema (1) + canonicalisation (3) + attestation envelope (4) → working pipeline (5)
- Independent of ArcadeDB driver setup (operates entirely on in-memory `GraphState` from Welle 3)
- HIGH reversibility (code-only; no Trust-Property-Layer-1 touch)
- Unblockt Welle 6 (projector-state-hash CI gate enforcement needs an emission code path to compare against)
- Scope-bounded for 1 session: 3 new modules + integration tests

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| NEW `crates/atlas-projector/src/replay.rs` — events.jsonl line reader; deserialise into `Vec<AtlasEvent>`; surface malformed JSON / wrong shape as `ProjectorError::ReplayMalformed { line, reason }` | Streaming-mode replay (Welle 5 reads whole file into memory; >1M-event scenarios deferred to a future parallel-projection welle) |
| NEW `crates/atlas-projector/src/upsert.rs` — idempotent upsert primitives mapping a single `AtlasEvent` to `GraphState` mutations | `policy.set`, `annotation.add`, `anchor.created` events (V2-α-MVP skips these with structured `ProjectorError::UnsupportedEventKind` rather than silently dropping) |
| Event-kind handling: `node.create` (node upsert), `node.update` (node patch), `edge.create` (edge upsert) | Verifier-side signature recomputation during replay (events are assumed pre-verified by atlas-trust-core; replay does NOT re-verify signatures because that's the consumer's responsibility) |
| `entity_uuid` derivation: `hex::encode(blake3(workspace_id \|\| event_uuid \|\| ":node"))` — per Welle 2 §3.5 logical-identifier sort key | ArcadeDB driver integration — replace in-memory `GraphState` with ArcadeDB-backed implementation (Welle 6+ candidate) |
| `edge_id` derivation: `hex::encode(blake3(workspace_id \|\| event_uuid \|\| ":edge"))` | Cross-event integrity (cross-trace dangling-edge detection beyond the structural-integrity check Welle 3 already does) |
| `author_did` propagation: when event carries `author_did`, stamp it onto every node/edge created/updated by that event | atlas-signer CLI emission flag (separate trivial welle) |
| NEW `crates/atlas-projector/src/emission.rs` — given a finalised `GraphState` + `projector_version` + `head_event_hash` + `projected_event_count`, construct an `AtlasPayload::ProjectorRunAttestation` payload (typed enum variant from Welle 4) ready for caller-side signing | Actual Ed25519 signing of the attestation payload — caller's responsibility (atlas-signer or future SDK) |
| `pub fn project_events(events: &[AtlasEvent], existing: Option<GraphState>) -> ProjectorResult<GraphState>` — top-level projection function | Sigstore Rekor anchoring of the attestation — orthogonal V1.5+ anchoring pipeline |
| `pub fn build_projector_run_attestation_payload(state: &GraphState, projector_version: &str, head_event_hash: &str, projected_event_count: u64) -> ProjectorResult<serde_json::Value>` — emission API | atlas-projector CLI binary (Welle 5 is library-only; binary deferred) |
| Integration test E2E: 5 events → project → emit attestation payload → re-parse via atlas-trust-core's `parse_projector_run_attestation` → format-validate green | Sigstore witness cosignature emission |
| Unit tests covering: replay malformed-line rejection; node.create/update/edge.create upsert idempotency; unsupported event-kind rejection; entity_uuid derivation determinism; emission payload shape | Mem0g cache integration (V2-β) |
| Update `docs/SEMVER-AUDIT-V1.0.md` §10 with V2-α Welle 5 atlas-projector additions | Read-API endpoints (V2-β) |
| Update `CHANGELOG.md [Unreleased]` | New event-kind additions beyond `node.create` / `node.update` / `edge.create` |
| Plan-doc (this file) | |

---

## Decisions (final, pending Nelson confirmation)

- **`entity_uuid` and `edge_id` derivation formula:** logical-identifier hash `blake3(workspace_id || event_uuid || ":node")` for nodes, `blake3(workspace_id || event_uuid || ":edge")` for edges. The `:` separator follows Atlas naming convention (analog to `atlas-anchor:` per-tenant kid prefix) and prevents collision between node-uuid and edge-uuid derived from the same event. Stable across re-projections.
- **Workspace_id is NOT taken from the AtlasEvent.** The events have no `workspace_id` field — workspace identity is bound at the trace level (`AtlasTrace.workspace_id`) AND through the per-tenant kid (`atlas-anchor:<workspace_id>`). The `project_events` function takes `workspace_id: &str` as an explicit parameter so callers must supply it. Documented in module docstring.
- **Idempotency policy:** `node.create` followed by `node.create` for the same `entity_uuid` is treated as `node.update`. This matches Welle 3's `GraphState::upsert_node` return-`Option<previous>` semantics — the upsert "wins" with the second occurrence's data. Rationale: in distributed projection scenarios, the same logical event might be replayed multiple times (e.g. resume after CI failure); idempotency must hold.
- **Unsupported event kinds** (annotation.add, policy.set, anchor.created): return `ProjectorError::UnsupportedEventKind` rather than panic or silent-skip. Caller decides whether to skip or abort. V2-α-MVP narrowly supports graph-shape events only; V2-β may extend.
- **`projector_version` string:** caller-supplied. Welle 5 does NOT pin a version internally — that's deployment policy. Typically `"atlas-projector/{CARGO_PKG_VERSION}"` per V1 `VERIFIER_VERSION` convention.
- **`head_event_hash` semantics:** caller-supplied as the `event_hash` of the last event consumed before computing the attestation. Welle 5 emission function does NOT auto-derive (would couple emission to replay-state in a way that complicates Welle 6 use-cases like "attest current state without consuming new events"). Caller passes it explicitly.
- **`author_did` propagation:** when an `AtlasEvent.author_did` is `Some(_)`, the upsert stamps it onto the resulting `GraphNode.author_did` / `GraphEdge.author_did`. When `None`, the field stays `None`. This implements the Welle 1 schema-additive invariant at the projection layer.
- **No serde derive on emission helper** — the emission function returns `serde_json::Value` directly. Caller wraps in an `AtlasEvent` + signs it. The output shape MUST match atlas-trust-core's `parse_projector_run_attestation` strict acceptance contract — that's tested via the integration round-trip.
- **No file-I/O in `atlas-projector` library** — `replay.rs` operates on `&str` (the contents of an events.jsonl file). Callers (CLI binaries, atlas-signer integrations) handle file I/O. This keeps atlas-projector free of `std::fs` dependencies and friendly to WASM targets in future wellen.
- **Version bump in this welle:** NONE. Workspace stays at `1.0.1`. Deferred to V2-α welle-bundle close-out.

---

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `crates/atlas-projector/src/replay.rs` (~180 lines) | `pub fn parse_events_jsonl(contents: &str) -> ProjectorResult<Vec<AtlasEvent>>` — line-by-line `serde_json::from_str::<AtlasEvent>`; malformed lines surface `ProjectorError::ReplayMalformed { line_number, reason }`. Empty-line-tolerant; blank-line-tolerant. Skip lines starting with `//` for operator-runbook compatibility (commented-out events). Unit tests: well-formed roundtrip, malformed JSON rejection, mixed-shape rejection. |
| NEW | `crates/atlas-projector/src/upsert.rs` (~300 lines) | `pub fn apply_event_to_state(workspace_id: &str, event: &AtlasEvent, state: &mut GraphState) -> ProjectorResult<()>` — dispatches on event.payload.type. Handles `node.create`, `node.update`, `edge.create`. Returns `UnsupportedEventKind` for other kinds. `entity_uuid` / `edge_id` derived deterministically. `author_did` propagated. Unit tests covering each event-kind upsert + idempotency + unsupported-kind rejection. |
| NEW | `crates/atlas-projector/src/emission.rs` (~120 lines) | `pub fn build_projector_run_attestation_payload(state: &GraphState, projector_version: &str, head_event_hash: &str, projected_event_count: u64) -> ProjectorResult<serde_json::Value>` — computes `graph_state_hash(state)` via Welle 3, assembles JSON object matching `PROJECTOR_RUN_ATTESTATION_KIND` shape per Welle 4. Returns `serde_json::Value` ready for caller-side signing (atlas-signer). Unit tests: shape validation, schema_version matches atlas-trust-core constant, round-trip via `parse_projector_run_attestation`. |
| MODIFY | `crates/atlas-projector/src/lib.rs` | `pub mod replay; pub mod upsert; pub mod emission;` + re-exports. Update crate docstring §"Scope" with Welle-5 additions. |
| MODIFY | `crates/atlas-projector/src/error.rs` | NEW variants: `ReplayMalformed { line_number: usize, reason: String }`, `UnsupportedEventKind { kind: String, event_id: String }`, `MissingPayloadField { event_id: String, field: String }`. All under `#[non_exhaustive]`. |
| MODIFY | `crates/atlas-projector/Cargo.toml` | Add `serde_json` workspace dep (already in atlas-trust-core, just verifying it's accessible). |
| NEW | `crates/atlas-projector/tests/projector_pipeline_integration.rs` (~280 lines) | E2E: synthetic 5-event events.jsonl → `parse_events_jsonl` → `project_events` (top-level convenience) → assert GraphState shape → `build_projector_run_attestation_payload` → verify round-trip via atlas-trust-core's `parse_projector_run_attestation` + `validate_projector_run_attestation` (both green). Plus: idempotency (project same events twice → same state); unsupported event-kind path. |
| MODIFY | `docs/SEMVER-AUDIT-V1.0.md` | §10.7c new subsection listing every new atlas-projector `pub` item with V2-α-Additive tag. |
| MODIFY | `CHANGELOG.md` | `[Unreleased]` gets `### Added — V2-α Welle 5` block ordered above Welle 4. |
| NEW | `.handoff/v2-alpha-welle-5-plan.md` | This plan-doc. |

**Total estimated diff:** ~1000-1400 lines Rust + tests + docs.

---

## Acceptance criteria

- [ ] `cargo check --workspace` green
- [ ] `cargo test --workspace` green; **all 7 byte-determinism pins byte-identical** (V1 cose + V1 cose-with-author_did + V1.7 anchor-chain-canonical-body + V1.7 anchor-chain-head + V1.9 pubkey-bundle + Welle 3 graph-state-hash + Welle 4 attestation-signing-input)
- [ ] `parse_events_jsonl` correctly handles well-formed multi-line JSONL, rejects malformed JSON with structured error, tolerates blank lines and `//` comment lines
- [ ] `apply_event_to_state` handles `node.create` / `node.update` / `edge.create`; returns `UnsupportedEventKind` for other kinds with structured reason
- [ ] `entity_uuid` derivation is deterministic — same `(workspace_id, event_uuid)` produces same hash across runs
- [ ] `edge_id` derivation is deterministic
- [ ] `author_did` from Welle 1 propagates to `GraphNode.author_did` / `GraphEdge.author_did` correctly
- [ ] Idempotency: projecting the same events twice produces byte-identical `graph_state_hash`
- [ ] `build_projector_run_attestation_payload` produces a `serde_json::Value` that `atlas_trust_core::parse_projector_run_attestation` accepts AND `validate_projector_run_attestation` validates green
- [ ] Integration test demonstrates E2E pipeline: events → project → emit → re-parse → validate
- [ ] `SEMVER-AUDIT-V1.0.md` §10 lists every new `pub` item
- [ ] `CHANGELOG.md [Unreleased]` has the Welle 5 entry
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched
- [ ] CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit, draft-then-ready PR, self-merge via `gh pr merge --squash --admin --delete-branch`

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| `entity_uuid` formula `blake3(workspace_id \|\| event_uuid \|\| ":node")` collides with edge-derived `:edge` form | NEGLIGIBLE | LOW | Different separator suffixes prevent collision; even if the input is identical, blake3 inputs are different. Documented in module docstring. |
| `node.update` payload requires referencing a `node_id` but Welle-5-MVP derives entity_uuid from event_uuid (not from node_id) — semantic gap | MEDIUM | MEDIUM | Welle-5-MVP interpretation: `node.update` is a NEW entity (different event_uuid → different entity_uuid). Future wellen may add `target_entity_uuid` field for cross-event-update semantics. Documented in plan + module-doc. |
| `serde_json::from_str::<AtlasEvent>` reject behavior depends on `#[serde(deny_unknown_fields)]` (set on AtlasEvent) — V2-α events with `author_did` deserialise correctly (added to struct in Welle 1) but V1.0 verifiers (if ever swapped in) would reject | LOW | LOW (by-design V2 wire-break per Welle 1) | Welle 5 only runs in V2-α-aware builds; cross-version compatibility is the verifier's concern, not the projector's |
| Replay performance on >100K events (in-memory whole-file read) | LOW (year-1 scale) | LOW | Welle-5-MVP scope; parallel/streaming replay deferred per plan |
| Empty events list passed to `project_events` | LOW | LOW | Returns empty `GraphState` cleanly; emission with 0 projected_event_count would fail validate-rule (count >= 1) by design; caller responsibility to check |

---

## Test impact (V1 assertions to preserve)

| V1 surface | Drift risk under Welle 5 | Mitigation |
|---|---|---|
| All 7 byte-determinism pins (cose, cose-with-author_did, anchor-canonical-body, anchor-head, pubkey-bundle, graph-state-hash, attestation-signing-input) | NONE — Welle 5 adds new modules to atlas-projector; does NOT modify atlas-trust-core source | `cargo test --workspace` includes all 7 pins; any drift catches |
| Welle 3 atlas-projector tests (20 unit tests) | NONE — new modules added; existing canonicalisation untouched | Tests confirm |
| Welle 4 attestation parser+validator (18 unit tests + 5 integration tests) | NONE — emission constructs same payload shape Welle 4 parses; round-trip test enforces compatibility | Round-trip test in projector_pipeline_integration.rs |
| Verifier-side validation in `verify_trace` | NONE — Welle 5 does not modify verify.rs | Tests confirm |

---

## Out-of-scope this welle (V2-α later wellen)

- **V2-α Welle 6 candidate:** ArcadeDB driver integration — replace in-memory `GraphState` with ArcadeDB-backed implementation; operator-runbook for deployment; SQL `ORDER BY entity_uuid` deterministic dump.
- **V2-α Welle 7 candidate:** projector-state-hash CI gate enforcement — compare attested `graph_state_hash` from a `ProjectorRunAttestation` event against locally-recomputed value from fresh re-projection. Uses Welle 5's `project_events` + Welle 3's `graph_state_hash` + Welle 4's `parse_projector_run_attestation`.
- **V2-α Welle 8 candidate:** content-hash separation (counsel-gated per `DECISION-COUNSEL-1`); atlas-signer CLI `--author-did` + `--emit-projector-attestation` flags; parallel-projection design for >10M event scenarios; expanded event-kind support (annotation, policy, anchor).

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| Master Plan §3 Three-Layer Trust Architecture | `docs/V2-MASTER-PLAN.md` |
| Master Plan §6 V2-α Foundation (Welle 5 = this) | `docs/V2-MASTER-PLAN.md` |
| `DECISION-ARCH-1` triple-hardening (canonicalisation + attestation + parallel-projection) | `.handoff/decisions.md` |
| Welle 1 Agent-DID Schema (`author_did` source for stamping) | `crates/atlas-trust-core/src/agent_did.rs` |
| Welle 2 §3.5 critical caveat (logical-identifier sort key) | `docs/V2-ALPHA-DB-SPIKE.md` |
| Welle 3 `graph_state_hash` + `GraphState` (projection target) | `crates/atlas-projector/src/canonical.rs` + `state.rs` |
| Welle 4 ProjectorRunAttestation envelope shape (emission target) | `crates/atlas-trust-core/src/projector_attestation.rs` |
| V1 AtlasEvent format (replay input) | `crates/atlas-trust-core/src/trace_format.rs::AtlasEvent` |
| V1 AtlasPayload typed enum (event-kind dispatch) | `crates/atlas-trust-core/src/trace_format.rs::AtlasPayload` |

---

**End of Welle 5 Plan.** Implementation proceeds on branch `feat/v2-alpha/welle-5-projector-emission` in TDD order: write integration test FIRST (RED), implement replay+upsert+emission (GREEN), refactor (IMPROVE). Single coherent SSH-signed commit per Atlas standing protocol.
