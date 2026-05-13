# V2-β Welle 14 — Plan-Doc (Expanded event-kind support)

> **Status:** DRAFT 2026-05-13. Awaiting parent agent's confirmation before merge.
> **Orchestration:** part of Phase 4 (parallel batch 2) per `docs/V2-BETA-ORCHESTRATION-PLAN.md`.
> **Driving decisions:** V2-β scope per `docs/V2-MASTER-PLAN.md` §6; extends the V2-α Welle 5 idempotent-upsert pipeline.

The V2-α Welle 5 projector narrowly supports 3 graph-shape payload kinds (`node_create`, `node_update`, `edge_create`). Every other event-kind currently surfaces `ProjectorError::UnsupportedEventKind`. Welle 14 extends the dispatch with 3 additional payload kinds — `annotation_add`, `policy_set`, `anchor_created` — required by V2-β read-API / MCP-V2 consumers (W12 + W13) that need to surface annotations + policy attachments + Rekor anchors at the Layer-2 projection level. Welle 14 is purely additive: existing event-kind handling is byte-identical and the 7 byte-determinism CI pins remain locked.

**Why this as Welle 14:** unblocks Phase 6 (W16 ArcadeDB spike) which considers how to project these new event-kinds onto a persistent backend; orthogonal to W12 (Read API) + W13 (MCP V2 tools) at the file-area level (only touches `crates/atlas-projector/*`).

## Scope (table)

| In-Scope | Out-of-Scope |
|---|---|
| `crates/atlas-projector/src/upsert.rs` — add 3 dispatch arms + tests | `CHANGELOG.md` (parent consolidates) |
| `crates/atlas-projector/src/state.rs` — extend `GraphNode` with `annotations` + `policies`; add `GraphState.rekor_anchors` top-level map | `docs/V2-MASTER-PLAN.md` status table (parent consolidates) |
| `crates/atlas-projector/src/canonical.rs` — extend canonical CBOR with new fields; preserve byte-determinism for V1-shape (empty) traces | `docs/SEMVER-AUDIT-V1.0.md` (parent consolidates) |
| `crates/atlas-projector/tests/projection_gate_integration.rs` — update `unsupported_event_kind_in_trace_surfaces_error` to use a still-unsupported kind | `.handoff/decisions.md` (parent consolidates) |
| `crates/atlas-projector/tests/projector_pipeline_integration.rs` — update `unsupported_event_kind_surfaces_structured_error` to use a still-unsupported kind | `.handoff/v2-session-handoff.md` (parent consolidates) |
| New unit tests for the 3 new dispatch arms + tests confirming canonical-bytes byte-identity for V1-shape traces | `docs/V2-BETA-ORCHESTRATION-PLAN.md` welle-progress (parent consolidates) |
|  | atlas-trust-core, atlas-signer, atlas-witness, atlas-verify-cli, atlas-verify-wasm (untouched) |

## Decisions (final, pending parent confirmation)

- **State extension shape:** `GraphNode` gains `annotations: BTreeMap<String, Vec<AnnotationEntry>>` (keyed by `annotation_kind`, multiple entries per kind appended in event-order then sort-stable for canonicalisation) and `policies: BTreeMap<String, PolicyEntry>` (keyed by `policy_id`, last-write-wins). Rekor anchors keyed by `event_id` (NOT entity-attached) → live as new top-level `GraphState.rekor_anchors: BTreeMap<String, AnchorEntry>`. Anchors are about the event being anchored, not about a graph-entity.
- **`annotation_add` semantics:** entity MUST exist (per spec). Surfaces `ProjectorError::MissingPayloadField` if absent. (Reusing an existing variant keeps the `#[non_exhaustive]` enum shape stable — no new variants needed; uses `field: "entity_uuid (not found in state)"` to disambiguate.) Multiple annotations per kind preserved as `Vec` (append-only). For canonicalisation, the Vec is hashed in insertion order; this is deterministic under idempotent replay because event-order is fixed in `events.jsonl`.
- **`policy_set` semantics:** last-write-wins per `policy_id`. `policy_version` defaults to `"v1"` if absent. Entity MUST exist (same rationale as `annotation_add`).
- **`anchor_created` semantics:** **security-conservative choice — anchor_created for the same event_id twice ERRORS** rather than last-write-wins. Rationale: Sigstore Rekor entries are tamper-evident append-only logs; a second anchor for the same event with different log-index would indicate either tampering or a replay attack. Erroring forces operator inspection. Documented in code.
- **Byte-determinism preservation for V1-shape traces:** the 3 new state fields (`GraphNode.annotations`, `GraphNode.policies`, `GraphState.rekor_anchors`) are **omitted from canonical CBOR when empty** — mirroring the V1 backward-compat pattern used for `author_did = None`. A V1-shape trace projects to a state with empty `annotations` / `policies` / `rekor_anchors` → canonical bytes byte-identical to current → graph-state-hash byte-identical → CI pin stays green.
- **`AnnotationEntry` / `PolicyEntry` / `AnchorEntry` shape:** simple plain-data structs in `state.rs`, no serde derives (mirrors `GraphNode` / `GraphEdge` convention).
- **No new `ProjectorError` variants:** all failure modes reuse existing variants (`MissingPayloadField` for entity-not-found, `UnsupportedEventKind` for the 1 remaining unsupported kind documented in module-docs).

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| MODIFY | `crates/atlas-projector/src/upsert.rs` | 3 new dispatch arms (`apply_annotation_add`, `apply_policy_set`, `apply_anchor_created`) + ~10 unit tests (~+300 lines) |
| MODIFY | `crates/atlas-projector/src/state.rs` | `AnnotationEntry`/`PolicyEntry`/`AnchorEntry` structs; `GraphNode.annotations`/`GraphNode.policies` fields; `GraphState.rekor_anchors` field; `upsert_anchor` helper (~+80 lines) |
| MODIFY | `crates/atlas-projector/src/canonical.rs` | Canonical CBOR extension: per-node `annotations`/`policies` omitted when empty; top-level `rekor_anchors` omitted when empty (~+60 lines) |
| MODIFY | `crates/atlas-projector/tests/projection_gate_integration.rs` | Update `unsupported_event_kind_in_trace_surfaces_error` to use `future_v2_gamma_kind` (still unsupported) |
| MODIFY | `crates/atlas-projector/tests/projector_pipeline_integration.rs` | Update `unsupported_event_kind_surfaces_structured_error` similarly |
| NEW | `.handoff/v2-beta-welle-14-plan.md` | this file |

**Total estimated diff:** ~450-500 lines added; ~10 lines modified.

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 14 | Mitigation |
|---|---|---|
| All 7 byte-determinism CI pins (cose ×3 + anchor ×2 + pubkey-bundle ×1 + graph-state-hash ×1) | NONE — new state fields are omitted from canonical CBOR when empty (V1-shape traces produce empty fields → unchanged bytes) | Dedicated test `canonical_state_hash_unchanged_for_v1_traces` re-projects the pipeline-test fixture and asserts byte-identical hash against the existing `graph_state_hash_byte_determinism_pin` value |
| 14 atlas-projector lib tests (Welle 3+5+6+7) | None — all existing tests project events that don't include the 3 new kinds | Re-run full suite after each refactor |
| 10 gate integration tests | One test (`unsupported_event_kind_in_trace_surfaces_error`) needs its fixture kind changed | Use `future_v2_gamma_kind` — guaranteed unsupported until V2-γ |
| 6 pipeline integration tests | One test (`unsupported_event_kind_surfaces_structured_error`) needs the same fixture-kind update | Same approach |
| atlas-trust-core 169 tests | None — atlas-trust-core is untouched | Re-run full workspace test |
| atlas-projector emitter / replay / gate / validator | None — no API or shape changes to public surfaces other than additive state fields | Verified by green test suite |

**Mandatory check:** all 7 byte-determinism CI pins MUST remain byte-identical after this welle's merge. The `graph_state_hash_byte_determinism_pin` test in `canonical.rs` is the sentinel.

## Implementation steps (TDD order)

1. **RED:** add 9 failing unit tests in `upsert.rs` `mod tests` covering the 3 new payload kinds (success + error cases + idempotency)
2. **RED:** add `canonical_state_hash_unchanged_for_v1_traces` test to confirm canonicalisation drift is zero for empty-field states
3. **GREEN — state.rs:** add `AnnotationEntry` / `PolicyEntry` / `AnchorEntry`; extend `GraphNode` and `GraphState`; add `upsert_anchor` helper
4. **GREEN — canonical.rs:** extend `canonical_node_map` to include `annotations` / `policies` when non-empty; extend `build_canonical_bytes` to include `rekor_anchors` when non-empty
5. **GREEN — upsert.rs:** add 3 dispatch arms with helper functions
6. **VERIFY:** run full atlas-projector + atlas-trust-core tests; confirm 169 + 14+N green; confirm `graph_state_hash_byte_determinism_pin` byte-identical
7. **UPDATE:** integration tests' unsupported-kind tests
8. Parallel `code-reviewer` + `security-reviewer` agents; fix CRITICAL/HIGH in-commit
9. SSH-Ed25519 signed single coherent commit
10. Push + open DRAFT PR with base=master

## Acceptance criteria

- [ ] `cargo check --workspace` green
- [ ] `cargo test --workspace --lib --bins -p atlas-trust-core -p atlas-projector` green; total = 169 + 14+N projector tests; all 7 byte-determinism CI pins byte-identical
- [ ] 3 new event-kinds (`annotation_add`, `policy_set`, `anchor_created`) accepted by `project_events` with correct state-mutation semantics
- [ ] Existing 14 atlas-projector unit tests pass byte-identical
- [ ] Existing 10 gate integration tests + 6 pipeline integration tests pass (with 1 fixture-kind update each)
- [ ] `graph_state_hash_byte_determinism_pin` produces the exact same hex as before Welle 14
- [ ] Plan-doc on welle's own branch
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched; CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit (`SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`)
- [ ] DRAFT PR open with base=master
- [ ] Forbidden-files rule honoured (no touches to CHANGELOG.md, V2-MASTER-PLAN.md status, decisions.md, semver-audit, handoff doc, orchestration plan)

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Adding fields to `GraphNode` accidentally changes canonical bytes for V1-shape traces | LOW | HIGH (breaks 7 byte-determinism CI pins) | Empty fields omitted from canonical CBOR; dedicated regression test pins hex; existing `graph_state_hash_byte_determinism_pin` runs unchanged |
| `annotation_add` with no `entity_uuid` arg silently no-ops | LOW | MED | Surfaces `MissingPayloadField` per spec; covered by test `annotation_add_on_missing_entity_errors` |
| `anchor_created` replay produces inconsistent state across projector runs | LOW | HIGH (trust-chain violation) | Erroring on duplicate anchor for same event_id forces operator inspection; covered by test |
| New state structs need serde derives later | LOW | LOW | Crate-doc invariant #5 forbids serde derives on state types; future welles can add canonical encoders for new shapes as needed |
| Subtle canonical-byte drift via map-key ordering | LOW | HIGH | Reuses existing `sort_cbor_map_entries` (RFC 8949 §4.2.1); empty-map omission tested |

## Out-of-scope this welle (later phases)

- **Phase 5 (W15):** Cypher validator consolidation — orthogonal
- **Phase 7 (W17a-c):** ArcadeDB driver — annotations/policies/anchors may need backend-side schema; deferred to backend phase
- **V2-γ:** policy-evaluation at write-time (Cedar) — `policy_set` just records the reference, doesn't evaluate
- **V2-δ:** rekor-anchor verification at write-time (TUF/transparency-log fetch)
- **Patch-merge semantics for `annotation_add`:** current behavior is append. A future welle may add "annotation update by (kind, sub-id)" — but V2-β scope is append-only.

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` |
| Master Plan | `docs/V2-MASTER-PLAN.md` §6 |
| V2-α Welle 5 dispatch pattern | `crates/atlas-projector/src/upsert.rs` |
| V2-α Welle 3 canonicalisation | `crates/atlas-projector/src/canonical.rs` |
| V1 optional-field pattern (`author_did`) | `canonical_node_map` in `canonical.rs` |
| Welle template | `.handoff/v2-beta-welle-N-plan.md.template` |

---

## Implementation Notes (Post-Code) — fill AFTER tests pass

### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| `AnnotationEntry` / `PolicyEntry` / `AnchorEntry` plain-data structs; `GraphNode.annotations`/`policies` fields; `GraphState.rekor_anchors` field; `upsert_anchor` helper | `crates/atlas-projector/src/state.rs` | ~+90 |
| `canonical_node_map` extended with optional `annotations`/`policies` CBOR maps (omitted when empty); `build_canonical_bytes` extended with optional `rekor_anchors` top-level map (omitted when empty); helper canonicalisers `canonical_annotation_entry` / `canonical_policy_entry` / `canonical_anchor_entry` | `crates/atlas-projector/src/canonical.rs` | ~+150 |
| `apply_annotation_add` / `apply_policy_set` / `apply_anchor_created` dispatch arms + 11 new unit tests + canonical-byte regression test | `crates/atlas-projector/src/upsert.rs` | ~+340 |
| `unsupported_event_kind_in_trace_surfaces_error` updated to use `future_v2_gamma_kind` | `crates/atlas-projector/tests/projection_gate_integration.rs` | ~3 lines changed |
| `unsupported_event_kind_surfaces_structured_error` updated similarly | `crates/atlas-projector/tests/projector_pipeline_integration.rs` | ~3 lines changed |

### Test outcome

- 11 new atlas-projector unit tests added (total 25 lib tests); all green
- 10 gate integration tests + 6 pipeline integration tests still green (with the 2 fixture-kind updates)
- `graph_state_hash_byte_determinism_pin` produces hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` byte-identical
- All 7 V2-α byte-determinism CI pins byte-identical
- atlas-trust-core 169 tests untouched + green

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| Canonical-byte drift for V1-shape traces | New regression test `canonical_state_hash_unchanged_for_v1_traces` + existing byte-determinism pin both pass |
| `annotation_add` silent no-op | Errors with `MissingPayloadField` surfaced; tested |
| `anchor_created` duplicate-anchor | Errors with `MissingPayloadField` (security-conservative); tested |

### Deviations from plan

None of substance — implementation matched plan-doc.
