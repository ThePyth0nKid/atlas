# V2-α Welle 3 — Plan-Doc (Atlas Projector Skeleton + Canonicalisation Byte-Pin)

> **Status: DRAFT 2026-05-12.** Awaiting Nelson's confirmation before merge. Becomes SHIPPED block in `.handoff/v2-session-handoff.md` once merged.
> **Master Plan reference:** `docs/V2-MASTER-PLAN.md` §6 V2-α Foundation. Session 3 of 5–8.
> **Driving decisions:** `DECISION-ARCH-1` triple-hardening (canonicalisation byte-pin + ProjectorRunAttestation + parallel-projection) + Welle 2 §3.5 important caveat (`@rid` is insert-order, NOT logical identity anchor; canonical hash MUST sort by logical identifier `entity_uuid`).

Welle 3 establishes the **Atlas Projector crate skeleton** as a new Rust workspace member, with the load-bearing primitive: a **byte-deterministic graph-state-hash function** that takes an in-memory canonical graph representation and produces a stable hash. The canonicalisation byte-pin pattern is analog to V1's `cose::tests::signing_input_byte_determinism_pin` — it locks the exact byte sequence the hash is computed over, so any future drift in canonicalisation breaks the pin BEFORE shipping mis-hashing logic.

**Why this as Welle 3** (rather than ProjectorRunAttestation event-kind OR full Projector implementation):
- The byte-pin spec is the load-bearing security primitive — every later Welle (event-emission, ArcadeDB integration, parallel-projection) builds on it
- Establishes the projector crate boundary (separates from atlas-trust-core which is verifier-only)
- Independent of ArcadeDB driver setup (in-memory state representation; DB-specific export deferred to Welle 5)
- Independent of events.jsonl reading (canonicalisation works on synthetic graph states; Layer-1-event-replay deferred to Welle 4)
- Tightly scoped + reviewable in 1 session (analog to Welle 1)

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| NEW crate `crates/atlas-projector/` workspace member | ArcadeDB driver integration (Welle 5 candidate) |
| `src/lib.rs` — public API surface | `events.jsonl` reading + parsing (Welle 4 candidate) |
| `src/state.rs` — in-memory `GraphState` + `GraphNode` + `GraphEdge` types with logical-identifier (`entity_uuid`) BTreeMap-backed ordering | `ProjectorRunAttestation` event-kind emission to Layer 1 (Welle 4 candidate) |
| `src/canonical.rs` — deterministic CBOR canonicalisation of `GraphState` + `graph_state_hash(state) -> [u8; 32]` blake3 hash function | Idempotent upsert logic for graph mutation from events (Welle 4) |
| `src/error.rs` — `ProjectorError` enum (local) | Parallel-projection design for >10M event scenarios (Welle 5) |
| **Byte-determinism CI pin** `graph_state_hash_byte_determinism_pin` — locks exact CBOR bytes + hash for a fixture `GraphState` | DB-specific dump-to-canonical adapter (Welle 5) |
| Unit tests: empty-state hash, single-node hash, multi-node hash, edge-only hash, property-ordering determinism, label-ordering determinism, sort-by-logical-id determinism (NOT by `@rid`-style insert-order) | atlas-signer CLI `--author-did` flag (separate follow-up welle) |
| Update workspace `Cargo.toml` to include new crate member | Cedar policy gate (V2-δ) |
| Update `docs/SEMVER-AUDIT-V1.0.md` §10 with atlas-projector crate listed under V2-α Additions | content-hash separation (counsel-gated) |
| Update `CHANGELOG.md [Unreleased]` | Mem0g cache integration (V2-β) |
| Plan-doc for Welle 3 (this file) | New event kinds in events.jsonl |

---

## Decisions (final, pending Nelson confirmation)

- **Crate name:** `atlas-projector`. Follows Atlas naming convention (`atlas-trust-core`, `atlas-signer`, `atlas-verify-cli`, `atlas-witness`). Reserved namespace for V2 Layer-2 projection logic.
- **Logical identifier:** `entity_uuid` — a string formed as `blake3(workspace_id || event_uuid || kind)` per Welle-2 spike §3.5 caveat. Stable across re-projections from `events.jsonl`. Stored as `String` in `GraphNode.entity_uuid`.
- **Container choice:** `BTreeMap<String, GraphNode>` for nodes (sorted by `entity_uuid`) and `BTreeMap<String, GraphEdge>` for edges (sorted by `edge_id` = `blake3(from_entity || to_entity || edge_kind)`). The `BTreeMap` ordering invariant is the canonicalisation foundation — iteration order is deterministic.
- **Property representation:** `BTreeMap<String, serde_json::Value>` per node + per edge. Properties are sorted by key per RFC 8949 §4.2.1 (length-then-lex) at canonicalisation time, not at storage time (matches V1's `cose::build_signing_input` pattern).
- **Labels:** `Vec<String>` per node — but sorted at canonicalisation time (BTreeSet-equivalent ordering). Atlas convention: nodes MAY have multiple labels (per Welle-2 spike §3.2 noting ArcadeDB supports multi-label out of the box).
- **CBOR encoding:** reuse `ciborium` (already workspace dependency for V1 canonicalisation). The canonical encoding follows V1's `sort_cbor_map_entries` pattern (length-then-lex on encoded keys).
- **No serde derives on GraphState yet** — Welle 3 keeps `GraphState` as a pure in-memory representation. Wire-format serialization (for projector-state-hash CI gate comparison across builds, or for `ProjectorRunAttestation` event payload encoding) lives entirely in `canonical.rs`'s `build_canonical_bytes` function. This isolates wire concerns from internal-state concerns, matching V1's `AtlasEvent` (serde-Serialize/Deserialize) vs `build_signing_input` (canonical-CBOR pure function) split.
- **Byte-pin fixture choice:** a small but non-trivial 3-node, 2-edge fixture with mixed labels + author_did-stamped properties. Documents the V2-α Welle-1 schema-addition propagation into graph-state-hash. Concrete byte sequence pinned in test.
- **Version bump in this welle:** NONE. Workspace stays at `1.0.1`. Deferred to V2-α welle-bundle close-out.
- **Public API surface:** `pub` items in `atlas-projector` become V2-α-Additive surface per `docs/SEMVER-AUDIT-V1.0.md` §10. NOT consumed by atlas-trust-core (clean DAG, no cycle); atlas-trust-core does NOT depend on atlas-projector.

---

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `crates/atlas-projector/Cargo.toml` | Crate manifest. Dependencies: `atlas-trust-core` (for `AGENT_DID_PREFIX`, error types), `ciborium` + `blake3` + `serde_json` (canonicalisation), `thiserror` (error types). No dependency on ArcadeDB drivers (deferred to Welle 5). |
| NEW | `crates/atlas-projector/src/lib.rs` | Crate-level docstring + `pub mod` declarations + re-exports of `GraphState`, `GraphNode`, `GraphEdge`, `ProjectorError`, `graph_state_hash`, `build_canonical_bytes`. |
| NEW | `crates/atlas-projector/src/state.rs` | `GraphNode { entity_uuid: String, labels: Vec<String>, properties: BTreeMap<String, serde_json::Value>, event_uuid: String, rekor_log_index: u64, author_did: Option<String> }`. `GraphEdge { edge_id: String, from_entity: String, to_entity: String, kind: String, properties: BTreeMap<String, serde_json::Value>, event_uuid: String, rekor_log_index: u64, author_did: Option<String> }`. `GraphState { nodes: BTreeMap<String, GraphNode>, edges: BTreeMap<String, GraphEdge> }` with `pub fn new()`, `pub fn upsert_node()`, `pub fn upsert_edge()`. |
| NEW | `crates/atlas-projector/src/canonical.rs` | `pub fn build_canonical_bytes(state: &GraphState) -> ProjectorResult<Vec<u8>>` — CBOR canonical encoding per RFC 8949 §4.2.1. `pub fn graph_state_hash(state: &GraphState) -> ProjectorResult<[u8; 32]>` — blake3 over canonical bytes. Internal `sort_cbor_map_entries` helper (analog to `cose::sort_cbor_map_entries`). The canonical encoding pins: (a) nodes sorted by `entity_uuid` ascending, (b) properties sorted per RFC 8949 §4.2.1 within each node, (c) labels sorted lexicographically, (d) edges sorted by `edge_id` ascending, (e) `author_did` included only if `Some` (V1-compat-style optional-field pattern). |
| NEW | `crates/atlas-projector/src/error.rs` | `ProjectorError` enum + `ProjectorResult<T>` alias. Variants: `CanonicalisationFailed(String)`, `MalformedAuthorDid(String)`, `MalformedEntityUuid(String)`, `DuplicateNode { entity_uuid: String }`, `DanglingEdge { edge_id: String, missing_endpoint: String }`. `#[non_exhaustive]` per Atlas convention. |
| NEW | `crates/atlas-projector/src/canonical.rs` tests | Unit tests: `empty_state_produces_stable_hash`, `single_node_hash`, `multi_node_sort_by_entity_uuid` (insert in different orders → identical hash), `property_order_does_not_matter` (BTreeMap-backed sort handles), `label_order_does_not_matter`, `author_did_present_changes_bytes` (Welle 1 invariant), `author_did_none_byte_identical_to_no_did_at_all`. |
| NEW | `crates/atlas-projector/src/canonical.rs` byte-pin test | `graph_state_hash_byte_determinism_pin` — fixture 3 nodes + 2 edges with mixed labels + author_did stamping. Pinned hex of canonical CBOR + pinned blake3 hex. Analog to V1's `signing_input_byte_determinism_pin`. |
| MODIFY | `Cargo.toml` (workspace) | Add `"crates/atlas-projector"` to `members`. |
| MODIFY | `docs/SEMVER-AUDIT-V1.0.md` | §10 V2-α Additions: new subsection §10.8 `atlas-projector` crate (V2-α-Additive). |
| MODIFY | `CHANGELOG.md` | `[Unreleased]` gets `### Added — V2-α Welle 3` block. |
| NEW | `.handoff/v2-alpha-welle-3-plan.md` | This plan-doc. |

**Total estimated diff:** ~600-900 lines Rust + tests + docs.

---

## Acceptance criteria

- [ ] `cargo check --workspace` green
- [ ] `cargo test --workspace` green (zero V1 regression — verify by checking V1's `signing_input_byte_determinism_pin` still passes byte-identically)
- [ ] New `atlas-projector` crate compiles standalone (no atlas-trust-core circular dependency)
- [ ] `graph_state_hash` produces deterministic output across re-runs with the same input
- [ ] `graph_state_hash` produces IDENTICAL output for the same logical state inserted in different orders (insertion-order independence — the critical Welle-2 §3.5 caveat is structurally enforced by the BTreeMap+sort pipeline)
- [ ] `graph_state_hash_byte_determinism_pin` test pins exact CBOR hex + blake3 hex
- [ ] At least 7 unit tests + 1 byte-pin test for the canonicalisation
- [ ] `ProjectorError` enum is `#[non_exhaustive]` and has ≥4 variants
- [ ] `SEMVER-AUDIT-V1.0.md` §10 lists every new `pub` item with V2-α-Additive tag
- [ ] `CHANGELOG.md [Unreleased]` has the Welle-3 entry
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched
- [ ] CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit, push, PR opened, self-merge via `gh pr merge --squash --admin --delete-branch`
- [ ] `.handoff/v2-session-handoff.md` updated post-merge with Welle-3 SHIPPED block

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| `BTreeMap` iteration order changes across Rust stdlib versions | LOW | HIGH (would break byte-pin invisibly) | `BTreeMap` ordering is API-stable across Rust versions per stdlib SemVer; the rust-version pin in workspace Cargo.toml (1.85) is a guardrail. Byte-pin test catches any drift. |
| CBOR canonicalisation library (`ciborium`) version bump changes output | LOW | HIGH | Pin `ciborium` minor in workspace deps; the V1 `signing_input_byte_determinism_pin` already exercises ciborium so an upstream change breaks V1 pin first (early warning). |
| `entity_uuid` formula `blake3(workspace_id || event_uuid || kind)` makes nodes from the same event but different `kind` distinct — is this the right semantic? | MEDIUM | MEDIUM | Document the formula in `state.rs` module docstring. Welle 4 (event-to-graph mapping) will surface real use-cases; if formula changes, byte-pin breaks → forced design conversation before ship. |
| Properties holding non-canonicalisable values (floats, nested non-JSON types) cause hash failures at edge cases | MEDIUM | LOW (graceful error, not silent corruption) | Reject floats at canonicaliser boundary (matches V1 pattern); document caller-contract. |
| Edge representation as `BTreeMap<edge_id, GraphEdge>` adds canonical-form complexity vs `Vec<GraphEdge>` | LOW | LOW | BTreeMap chosen for sort-determinism; edge_id = blake3(from || to || kind) is stable across insertions. |

---

## Test impact (V1 assertions to preserve)

| V1 surface | Drift risk under Welle 3 | Mitigation |
|---|---|---|
| `cose::tests::signing_input_byte_determinism_pin` (V1 byte-pin) | None — Welle 3 adds a new crate; does NOT modify `atlas-trust-core` source | `cargo test --workspace` includes V1 pin; regression catches any accidental modification |
| `cose::tests::signing_input_byte_determinism_pin_with_author_did` (Welle 1 byte-pin) | None — Welle 1's signing-input shape unchanged | Same |
| `agent_did::tests::*` (Welle 1 unit tests) | None — Welle 3 may use `validate_agent_did` for input-checking author_did but does not modify the module | Same |
| `verify_trace` end-to-end | None — verifier is not yet aware of `ProjectorRunAttestation` event kind (Welle 4 work); current verifier behavior preserved | Same |

---

## Out-of-scope this welle (V2-α later wellen)

- **V2-α Welle 4 candidate:** `events.jsonl` reading + idempotent upsert from events to `GraphState` + `ProjectorRunAttestation` event-kind emission + verifier-side `ProjectorRunAttestation` validation (in `atlas-trust-core`).
- **V2-α Welle 5 candidate:** ArcadeDB driver integration — replace in-memory `GraphState` with ArcadeDB-backed implementation; SQL `SELECT * FROM <type> ORDER BY entity_uuid` for deterministic dump (per Welle-2 §3.5 logical-identifier sort key); operator-runbook for ArcadeDB deployment.
- **V2-α Welle 6 candidate:** projector-state-hash CI gate enforcement (compares `graph_state_hash` from a full re-projection against a pinned `.projection-integrity.json`).
- **V2-α Welle 7-8 candidates:** content-hash separation (counsel-gated per `DECISION-COUNSEL-1`); parallel-projection design for >10M event scenarios; atlas-signer CLI `--author-did` flag.

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| Master Plan §3 Three-Layer Architecture (Layer 2 ArcadeDB) | `docs/V2-MASTER-PLAN.md` |
| Master Plan §6 V2-α Foundation (Welle 3 = this) | `docs/V2-MASTER-PLAN.md` |
| `DECISION-ARCH-1` projection determinism triple-hardening | `.handoff/decisions.md` |
| `DECISION-DB-4` ArcadeDB primary | `.handoff/decisions.md` |
| Welle 2 spike §3.5 `@rid` caveat (logical-identifier sort required) | `docs/V2-ALPHA-DB-SPIKE.md` |
| Welle 1 Agent-DID Schema (`author_did` stamping on nodes/edges) | `crates/atlas-trust-core/src/agent_did.rs` |
| V1 canonicalisation pattern (analog) | `crates/atlas-trust-core/src/cose.rs::build_signing_input` + `signing_input_byte_determinism_pin` |
| V1 `per_tenant.rs` (naming convention analog) | `crates/atlas-trust-core/src/per_tenant.rs` |
| Working Methodology Welle-Decomposition Pattern | `docs/WORKING-METHODOLOGY.md` |

---

**End of Welle 3 Plan.** Implementation proceeds on branch `feat/v2-alpha/welle-3-projector-skeleton` in TDD order: write byte-determinism pin test FIRST (RED), implement canonicalisation (GREEN), refactor (IMPROVE). Single coherent SSH-signed commit per Atlas standing protocol.
