# V2-β Welle 17a — Plan-Doc (`GraphStateBackend` trait + InMemoryBackend + ArcadeDbBackend stub + ADR-Atlas-011)

> **Status:** DRAFT 2026-05-13. Awaiting parent agent's confirmation before merge.
> **Orchestration:** Phase 9 (serial — first welle in the W17a/b/c chain) per `docs/V2-BETA-ORCHESTRATION-PLAN.md`.
> **Driving decisions:** ADR-Atlas-010 (W16 — backend choice + embedded-mode trade-off, 8 sub-decisions); `DECISION-DB-4` (ArcadeDB Apache-2.0 primary); `DECISION-SEC-4` (Cypher passthrough hardening); ADR-Atlas-007 §3.1 (Option-A workspace-parallel projection).

W17a adds the production `GraphStateBackend` trait surface to `atlas-projector` and ships two impls: a behaviour-preserving `InMemoryBackend` (default; wraps the existing V2-α `GraphState`/`upsert`/`canonical` pipeline) and an `ArcadeDbBackend` stub (every method `unimplemented!("W17b: ...")`). The byte-determinism CI pin (`8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`) is preserved across the new abstraction layer, and a new conformance test exercises the pinned hex through the trait surface as a defence-in-depth signal alongside the original `canonical::tests::graph_state_hash_byte_determinism_pin`.

**Why this as Welle 17a:** ADR-Atlas-010 §4 sub-decision #8 explicitly scopes the production trait + the two impl scaffolds to W17a. W17b (full ArcadeDb HTTP+Cypher impl) and W17c (Docker-Compose CI integration tests) both depend on the trait surface being locked. W17a's design is intentionally fill-in-the-blanks-ready — no API negotiation between W17a and W17b is allowed.

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| `crates/atlas-projector/src/backend/mod.rs` (NEW) — production trait + Vertex + Edge + UpsertResult + WorkspaceTxn + default canonical_state | `crates/atlas-projector/src/backend/arcadedb.rs` body (W17b) |
| `crates/atlas-projector/src/backend/in_memory.rs` (NEW) — wraps V2-α state; byte-pin preserved | Cross-backend byte-determinism test (W17b — `tests/cross_backend_byte_determinism.rs`) |
| `crates/atlas-projector/src/backend/arcadedb.rs` (NEW) — stub; all methods unimplemented!() | `reqwest` dep addition (W17b alongside first method body) |
| `crates/atlas-projector/tests/backend_trait_conformance.rs` (NEW) — 8 tests | Docker-Compose CI workflow `.github/workflows/atlas-arcadedb-smoke.yml` (W17c) |
| `crates/atlas-projector/src/lib.rs` — `pub mod backend` + re-exports | Trait-surface extension for Welle-14 fields (annotations, policies) — W17b/W18 |
| `crates/atlas-projector/src/emission.rs` — add `_from_backend` variant alongside legacy | Workspace-parallel projection orchestration loop (W17b) |
| `crates/atlas-projector/src/gate.rs` — add `_with_backend` variant alongside legacy | `CHANGELOG.md` (parent consolidates in Phase 9.5) |
| `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` (NEW) — 9 sections | `docs/V2-MASTER-PLAN.md` status table (parent consolidates) |
| `.handoff/v2-beta-welle-17a-plan.md` (THIS file) | `docs/SEMVER-AUDIT-V1.0.md` (parent consolidates) |
| | `.handoff/decisions.md` (parent consolidates) |
| | `.handoff/v2-session-handoff.md` (parent consolidates) |
| | `docs/V2-BETA-ORCHESTRATION-PLAN.md` (parent consolidates) |

**Hard rule:** the "Out-of-Scope" column includes the V2-β-Orchestration-Plan §3.3 forbidden files (CHANGELOG.md, V2-MASTER-PLAN.md status table, SEMVER-AUDIT-V1.0.md, decisions.md, v2-session-handoff.md, ORCHESTRATION-PLAN.md, CI workflow files). Parent agent edits those post-consolidation.

## Decisions (final, pending parent confirmation)

- **Object safety:** `Box<dyn GraphStateBackend>` + `Box<dyn WorkspaceTxn>` (ADR-010 §6 OQ-1 resolved). Vtable overhead ~1 ns vs ~300-500 µs HTTP roundtrip — irrelevant; viral generics rejected.
- **Batch operations:** `WorkspaceTxn::batch_upsert(&[Vertex], &[Edge]) -> Vec<UpsertResult>`, vertices applied before edges (ADR-010 §6 OQ-2 resolved). Mirrors Option-A projection batch pattern.
- **Default `canonical_state` impl on trait body:** delegates to `vertices_sorted` + `edges_sorted` + V2-α `canonical::graph_state_hash`. InMemoryBackend overrides for performance + byte-pin proximity.
- **`Vertex` / `Edge` / `UpsertResult` are `#[non_exhaustive]` with `new(...)` constructors:** schema-additive forwards-compat; positional constructor is the documented external-crate entry point.
- **`rekor_log_index: Option<u64>`** at the trait surface; round-trips losslessly to V2-α's `u64` with sentinel 0.
- **`EntityUuid` / `EdgeId` / `WorkspaceId` are `String` aliases**, NOT newtypes (V2-α `GraphState` uses `String`; newtype conversion at every boundary would be lossy / noisy).
- **`InMemoryBackend` storage:** `Arc<Mutex<HashMap<WorkspaceId, GraphState>>>`. Transactions snapshot scratch; `commit` swaps. No `MutexGuard` held across struct boundaries (Windows `MutexGuard: !Send` resolved at design time).
- **`ArcadeDbBackend` stub:** all methods `unimplemented!("W17b: <endpoint + Cypher hint>")`. `backend_id()` returns `"arcadedb-server"` (production string, stable for ProjectorRunAttestation chain).
- **No `reqwest` / `serde_cbor` dep added in W17a.** Existing `ciborium` covers canonical-CBOR; `reqwest` lands in W17b alongside the first method body.

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW    | `crates/atlas-projector/src/backend/mod.rs` | Trait + Vertex/Edge/UpsertResult/WorkspaceTxn + default canonical_state + helpers + 3 module-tests. ~470 LOC. |
| NEW    | `crates/atlas-projector/src/backend/in_memory.rs` | InMemoryBackend impl. Arc<Mutex<HashMap>>. Per-workspace scratch txn. canonical_state override. 10 module-tests. ~370 LOC. |
| NEW    | `crates/atlas-projector/src/backend/arcadedb.rs` | Stub. Constructor + backend_id work; trait methods all `unimplemented!("W17b: ...")`. 4 module-tests (constructs, backend_id, 3× should_panic). ~150 LOC. |
| NEW    | `crates/atlas-projector/tests/backend_trait_conformance.rs` | 8 conformance tests: round-trip, byte-pin through trait, backend_id stability, ArcadeDb stub panics, batch_upsert ordering, batch_vs_individual canonical equality, sorted iteration, rollback-noop. ~280 LOC. |
| NEW    | `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` | 9-section ADR mirroring ADR-010. Resolves OQ-1 + OQ-2. Opens OQ-7..OQ-11 for W17b/V2-γ. ~280 LOC. |
| NEW    | `.handoff/v2-beta-welle-17a-plan.md` | THIS file. Plan-doc per template. |
| MODIFY | `crates/atlas-projector/src/lib.rs` | `pub mod backend;` + 6 re-exports + 2 new emission/gate function re-exports. +13 LOC. |
| MODIFY | `crates/atlas-projector/src/emission.rs` | Add `build_projector_run_attestation_payload_from_backend(...)` alongside legacy. +50 LOC. |
| MODIFY | `crates/atlas-projector/src/gate.rs` | Add `verify_attestations_in_trace_with_backend(...)` alongside legacy. +50 LOC. One `#[allow(dead_code)]` on pre-existing `FIXTURE_HEAD`. |

**Total estimated diff:** ~1,500-1,700 new LOC (production + tests + ADR + plan-doc), ~110 LOC modified.

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 17a | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins | NONE — no changes to canonical CBOR pipeline, no changes to COSE / anchor / pubkey-bundle paths | All 7 pins remain in their existing locations; the NEW conformance test exercises pin #7 (graph_state_hash) via a SECOND path through the trait surface for defence in depth |
| V2-α `emission::build_projector_run_attestation_payload` legacy entry | NONE — function signature + body unchanged; new `_from_backend` variant added beside it | Existing tests `emission::tests::*` continue to use the legacy entry; all green |
| V2-α `gate::verify_attestations_in_trace` legacy entry | NONE — function signature + body unchanged; new `_with_backend` variant added beside it | Existing tests `gate::tests::*` continue to use legacy entry; all green |
| V2-α `canonical::graph_state_hash` | NONE — function untouched | Byte-pin test `canonical::tests::graph_state_hash_byte_determinism_pin` GREEN; pinned hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduced |
| V2-α `upsert::project_events` | NONE — function untouched | All `upsert::tests::*` (32 tests) green |
| V2-α `state::GraphState` field set | NONE — struct untouched | All `state::tests::*` green |
| Trait-surface field-set determinism | LOW — `Vertex`/`Edge` `#[non_exhaustive]` + explicit `new()` constructor lock the field order; conversion in `vertex_from_graph_node` / `edge_from_graph_edge` is exhaustive | Conformance test `byte_pin_through_in_memory_backend` validates round-trip; any future field addition must update the conversion AND the byte-pin hex with a documented crate-version bump |

**Mandatory check:** all 7 byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) MUST remain byte-identical after this welle's merge. **VERIFIED**: `cargo test -p atlas-trust-core -p atlas-projector` 169 + 88 + 8 + 10 + ... tests all green.

## Implementation steps (TDD order — actually followed)

1. RED: write `backend_trait_conformance.rs` with 8 trait-conformance tests including byte-pin via trait → tests cannot link until trait + impls exist.
2. GREEN: write `backend/mod.rs` (trait + types + helpers), `backend/in_memory.rs` (InMemoryBackend), `backend/arcadedb.rs` (stub).
3. Wire into `lib.rs` (re-exports), `emission.rs` (`_from_backend` variant), `gate.rs` (`_with_backend` variant).
4. `cargo check --workspace` — GREEN.
5. `cargo test -p atlas-trust-core -p atlas-projector` — verify 169 trust-core + 88 projector unit + 8 conformance + 10 + 6 + 5 + 18 + 5 + 13 + 11 + 5 + 6 + 8 + 6 integration green (matches W17a entry-criteria; numbers logged in commit body).
6. Verify byte-pin hex exactly `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` via `cargo test graph_state_hash_byte_determinism_pin`.
7. Write `ADR-Atlas-011` + `.handoff/v2-beta-welle-17a-plan.md`.
8. Dispatch parallel `code-reviewer` + `security-reviewer` agents.
9. Fix CRITICAL + HIGH findings in-commit.
10. Single SSH-Ed25519 signed commit.
11. Push branch + open DRAFT PR with base=master.
12. Return PR number to parent agent.

## Acceptance criteria

- [x] `cargo check --workspace` green
- [x] `cargo test --workspace` (atlas-trust-core + atlas-projector subset) green; byte-determinism pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` UNCHANGED
- [x] `crates/atlas-projector/src/backend/mod.rs` exports `GraphStateBackend`, `Vertex`, `Edge`, `UpsertResult`, `WorkspaceTxn`, `EntityUuid`, `EdgeId`, `WorkspaceId`, plus public constructors
- [x] `InMemoryBackend` round-trips a vertex/edge cycle; conformance test green
- [x] `ArcadeDbBackend` stub compiles; trait methods panic with `"W17b: ..."` messages
- [x] `emission.rs` + `gate.rs` legacy entry points unchanged; new backend-aware variants added beside
- [x] `ADR-Atlas-011` documents OQ-1 (`Box<dyn>`) + OQ-2 (`batch_upsert`) resolutions
- [x] Plan-doc on welle's own branch
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched; CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit (`SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [ ] DRAFT PR open with base=master
- [x] Forbidden-files rule honoured (no touches to CHANGELOG.md, V2-MASTER-PLAN.md status, decisions.md, semver-audit, handoff doc, ORCHESTRATION-PLAN.md, CI workflows)

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Byte-pin drift introduced by trait abstraction | LOW | HIGH | New conformance test `byte_pin_through_in_memory_backend` exercises the same fixture through the trait surface; tripped before merge if drift exists |
| `MutexGuard: !Send` (Windows) breaks `WorkspaceTxn: Send` | LOW (resolved at design time) | MED | InMemoryTxn holds `Arc<Mutex<...>>` clone; locks acquired at commit time only, never held across struct boundaries |
| `#[non_exhaustive]` blocks external test construction | LOW (caught during dev-loop) | LOW | Explicit `Vertex::new` / `Edge::new` / `UpsertResult::new` constructors |
| W17b API negotiation needed after W17a merge | LOW | MED | ADR-Atlas-011 §4.1 + §4.2 lock the trait surface; W17b is fill-in-the-blanks |
| `ArcadeDbBackend` stub accidentally used in production | LOW | HIGH | `unimplemented!("W17b: ...")` panics surface immediately; conformance test asserts panic |
| Existing V2-α callers regress | LOW | HIGH | Legacy `emission::build_projector_run_attestation_payload` + `gate::verify_attestations_in_trace` signatures + bodies unchanged; existing tests cover; new variants are additive |

## Out-of-scope this welle (later phases)

- **Phase 9 W17b:** Full `ArcadeDbBackend` impl using `reqwest` + Cypher per ADR-Atlas-010. New test `tests/cross_backend_byte_determinism.rs` validates adapter contract.
- **Phase 9 W17c:** Docker-Compose CI workflow + integration tests + benchmark capture.
- **Phase 10 W18:** Mem0g integration. Decision whether to reuse the `GraphStateBackend` trait or define a new `VectorStoreBackend` trait — TBD per ADR-Atlas-012.
- **Phase 9.5 consolidation:** parent agent updates `CHANGELOG.md`, `docs/V2-MASTER-PLAN.md` §6, `.handoff/decisions.md`, `.handoff/v2-session-handoff.md`, `docs/V2-BETA-ORCHESTRATION-PLAN.md`.

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` |
| Master Plan | `docs/V2-MASTER-PLAN.md` §6 |
| Working Methodology | `docs/WORKING-METHODOLOGY.md` |
| ArcadeDB spike (W16) | `docs/V2-BETA-ARCADEDB-SPIKE.md` |
| ADR-Atlas-010 (W16 backend choice) | `docs/ADR/ADR-Atlas-010-arcadedb-backend-choice-and-embedded-mode-tradeoff.md` |
| ADR-Atlas-011 (W17a driver scaffold) | `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` |
| Byte-pin source-of-truth | `crates/atlas-projector/src/canonical.rs` tests::graph_state_hash_byte_determinism_pin |
| Trait conformance | `crates/atlas-projector/tests/backend_trait_conformance.rs` |

---

## Implementation Notes (Post-Code)

### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| `GraphStateBackend` trait + Vertex/Edge/UpsertResult/WorkspaceTxn + helpers + 3 module-tests | `crates/atlas-projector/src/backend/mod.rs` | ~470 |
| `InMemoryBackend` impl + scratch-buffer txn + 10 module-tests | `crates/atlas-projector/src/backend/in_memory.rs` | ~370 |
| `ArcadeDbBackend` + `ArcadeDbTxn` stubs + 4 module-tests | `crates/atlas-projector/src/backend/arcadedb.rs` | ~150 |
| Trait-conformance tests (8 tests) | `crates/atlas-projector/tests/backend_trait_conformance.rs` | ~280 |
| ADR-Atlas-011 | `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` | ~280 |
| Plan-doc | `.handoff/v2-beta-welle-17a-plan.md` | THIS file |
| `pub mod backend;` + re-exports | `crates/atlas-projector/src/lib.rs` | +13 |
| Backend-aware `_from_backend` variant | `crates/atlas-projector/src/emission.rs` | +50 |
| Backend-aware `_with_backend` variant | `crates/atlas-projector/src/gate.rs` | +50 |

### Test outcome

- `cargo check --workspace` — GREEN.
- `cargo test -p atlas-trust-core -p atlas-projector` — 169 trust-core + 88 projector-unit + 8 backend-trait-conformance + 10 + 6 + 5 + 18 + 5 + 13 + 11 + 5 + 6 + 8 + 6 = all green.
- `canonical::tests::graph_state_hash_byte_determinism_pin` — GREEN; pinned hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` UNCHANGED.
- `backend_trait_conformance::byte_pin_through_in_memory_backend` — GREEN; same hex reproduced via the trait surface.
- All 7 V2-α byte-determinism CI pins UNCHANGED.

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| Byte-pin drift introduced by trait abstraction | `byte_pin_through_in_memory_backend` test runs same fixture through trait — pinned hex matches exactly |
| `MutexGuard: !Send` Windows blocker | Resolved at design time — `Arc<Mutex<...>>` cloned into txn; mutex acquired only at commit; no guard held across struct boundaries |
| `#[non_exhaustive]` external construction | Caught in dev loop on first conformance-test compile; `Vertex::new` / `Edge::new` / `UpsertResult::new` added |
| W17b API negotiation risk | ADR-Atlas-011 §4 locks 9 sub-decisions; W17b is fill-in-the-blanks |

### Deviations from plan

- **`serde_cbor::Value` from the spike sketch** was replaced with `serde_json::Value` for properties (matching V2-α `GraphNode::properties` exactly). `serde_cbor` is not a workspace dep; `ciborium` is the V2-α canonical-CBOR boundary and lives at canonicalisation time, NOT at backend-surface time.
- **`EntityUuid` / `EdgeId` / `WorkspaceId`** kept as `String` aliases, NOT imported from `atlas-trust-core` (those types don't exist there — verified empirically). Documented in ADR §4.2 sub-decision #6.
- **`rekor_log_index`** uses `Option<u64>` at the trait surface (more honest) vs V2-α's `u64`-with-sentinel-0 internally. Lossless conversion documented at the boundary.

---

## Subagent dispatch prompt skeleton (anti-divergence enforcement)

This welle's prompt is captured in the parent agent's dispatch record. Subsequent welle prompts should mirror the structure: pre-flight reading list, in-scope files, forbidden files, reviewer expectations, output spec.

---

## Post-merge: reviewer findings deferred to W17b (carry-over)

External `code-reviewer` + `security-reviewer` agents reviewed PR #85 in the parent dispatch (2026-05-13). Both verdicts: **APPROVE** — 0 CRITICAL / 0 HIGH. Five MEDIUM findings split into one-in-commit-fix + four W17b carry-overs:

### Applied in-commit on PR #85 (fix commit on `feat/v2-beta/welle-17a-arcadedb-scaffold`)

1. **`#[doc(hidden)]` on `InMemoryBackend::snapshot()`** (`crates/atlas-projector/src/backend/in_memory.rs:69`). Security-reviewer MEDIUM: the method is `pub` for diagnostic use but exposes raw `GraphState` internals. Added `#[doc(hidden)]` + clarifying doc note so production paths are routed through the trait surface only.

### Deferred to W17b (when `.handoff/v2-beta-welle-17b-plan.md` is created, lift these in)

2. **`serde_json::Value` depth limit at the trait surface.** Security-reviewer MEDIUM: `Vertex::properties` / `Edge::properties` are `BTreeMap<String, serde_json::Value>` with no depth-cap. W17a path is in-memory only (depth bounded by upstream V2-α event-ingestion limits in `canonical.rs`). **W17b risk:** ArcadeDB HTTP responses deserialized into `Vertex::properties` BEFORE `canonical.rs`-limits apply could DoS the projector. **Fix-in-W17b:** apply explicit depth + size cap when parsing ArcadeDB Cypher result JSON into `Vertex` / `Edge`.

3. **`WorkspaceId` String validation at the trait boundary.** Security-reviewer MEDIUM: `WorkspaceId = String` with no validation. For `InMemoryBackend` this is harmless (HashMap key). **W17b risk:** an empty / path-traversal-like / adversarially-long `workspace_id` reaching ArcadeDB's HTTP `/api/v1/begin/{db}` endpoint or appearing as a Cypher parameter (`MATCH (n) WHERE n.workspace_id = $ws`) could behave unexpectedly. **Fix-in-W17b:** validation guard at `ArcadeDbBackend::begin()` rejecting empty / enforcing UUID-or-equivalent format BEFORE constructing the HTTP request.

4. **`begin()` lifetime bound (`'_` vs `'static`).** Code-reviewer MEDIUM: the trait signature `fn begin(&self, ...) -> ProjectorResult<Box<dyn WorkspaceTxn + '_>>` ties txn lifetime to backend reference. `InMemoryBackend` doesn't need it (txn holds an `Arc` clone). `ArcadeDbBackend`'s HTTP session handle won't borrow from `&self` either. The `'_` is artificially conservative. **W17b risk:** if `ArcadeDbBackend` needs `'static` and the trait has `'_`, the trait signature must change — that's a SemVer-breaking refactor mid-W17b. **Fix-in-W17b:** evaluate `'static` vs explicit named lifetime BEFORE writing the first `ArcadeDbBackend::begin()` body. If a change is needed, version-bump `atlas-projector` accordingly.

5. **Error-enum cleanup: `MalformedEntityUuid` umbrella variant for edges.** Code-reviewer MEDIUM: `upsert_edge_inner` returns `ProjectorError::MalformedEntityUuid` for empty `edge_id` — operator-diagnostic ergonomics, not correctness. NOT fixed in W17a because the V2-α convention already reuses this variant for both vertex + edge logical-id violations (`state.rs:289`, `state.rs:308`); changing only the W17a path would create internal inconsistency. **Defer to a broader error-enum refactor welle (V2-γ scope or whenever the existing V2-α convention is revisited).** The error MESSAGE is already correct (`"edge_id is empty (edge ...)")` — only the variant NAME is umbrella.

### LOW (documented, not actioned)

- `gate.rs:344` carries `#[allow(dead_code)]` on `FIXTURE_HEAD`. Constant is used in `emission.rs` tests but unused in `gate.rs` tests after the `*_with_backend` refactor. Minor noise; defer to dead-code sweep welle.

### Reviewer-verdict summary

Both reviewers APPROVE. Byte-determinism-pin hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` verified intact through TWO independent test paths. Multi-tenant isolation PASS (no cross-workspace bleed in any read path or commit path). V2-α stamping-field integrity (event_uuid / rekor_log_index / author_did) PASS. Send/Sync correctness PASS. No `unsafe` blocks introduced. `#[non_exhaustive]` discipline verified on `Vertex` / `Edge` / `UpsertResult` / `GateStatus`.
