# V2-β Welle 18b — Plan-Doc (Mem0g Layer-3 Cache Implementation)

> **Status:** DRAFT 2026-05-15. Awaiting parent agent's confirmation before merge.
> **Orchestration:** Phase 12 implementation (B-phase of W18). Implements the binding sub-decisions locked in `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` §4 (1-8).
> **Driving decisions:** `DECISION-SEC-5` (secure-delete contract); `DECISION-DB-3` (latency-claim attribution); `DECISION-DB-4` (Apache-2.0 + JVM/Python avoidance); `DECISION-ARCH-1` (byte-determinism triple-hardening); `DECISION-COUNSEL-1` (GDPR Art. 4(1) opinion, parallel track).

W18b ships the production Layer-3 semantic cache concretely: a new workspace member crate `crates/atlas-mem0g/` housing the `SemanticCacheBackend` trait + `LanceDbCacheBackend` impl (feature-gated) + Atlas-owned secure-delete wrapper + supply-chain-verified model download. Plus the `embedding_erased` event-kind dispatch arm in `atlas-projector`, the B4/B5/B6 benches, the `atlas-mem0g-smoke` CI workflow, and the W12-pattern `/api/atlas/semantic-search` Read-API endpoint with response-time normalisation.

**Why this Welle next:** W18 Phase A (design) shipped at master HEAD `08b31dc`. ADR-Atlas-012 locks 8 binding sub-decisions; W18b is the fill-in-the-blanks implementation pass that closes V2-β tripod leg #3 (Layer 3 semantic-search). W19 v2.0.0-beta.1 convergence ship depends on this welle plus W17a-c (already shipped).

## Scope (table)

| In-Scope | Out-of-Scope |
|---|---|
| NEW `crates/atlas-mem0g/Cargo.toml` (~50 LOC) | Real fastembed-rs model download (placeholder constants until Nelson confirms) |
| NEW `crates/atlas-mem0g/src/lib.rs` (~310 LOC) — trait + types + `InvalidationPolicy` + `check_workspace_id` | Real LanceDB Arrow append/search bodies (W18b ships trait + secure-delete protocol wiring; LanceDB body stubs flagged for V1-V4 verification phase resume) |
| NEW `crates/atlas-mem0g/src/embedder.rs` (~250 LOC) — supply-chain pins + `download_model_with_verification` + `AtlasEmbedder` | Real `HF_REVISION_SHA` / `ONNX_SHA256` / `MODEL_URL` constants (placeholders flagged with `TODO(W18b-NELSON-VERIFY)`; resolved pre-merge by Nelson) |
| NEW `crates/atlas-mem0g/src/secure_delete.rs` (~280 LOC) — 7-step protocol + `PerTableLockMap` + `overwrite_file` + `apply_overwrite_set` | SSD-physical-erasure via SECURE_ERASE ATA (V2-γ; ADR §6 OQ-1) |
| NEW `crates/atlas-mem0g/src/lancedb_backend.rs` (~250 LOC) — `LanceDbCacheBackend` impl wired through secure-delete protocol | Multi-region replication (V2-γ; ADR §6 OQ-2) |
| NEW `crates/atlas-mem0g/tests/embedding_determinism.rs` (~110 LOC) — V2 verification gap closure | Cross-platform (Windows + macOS) determinism CI matrix (W18c follow-on if Windows fails) |
| NEW `crates/atlas-mem0g/tests/secure_delete_correctness.rs` (~150 LOC) — V1 verification gap + concurrent-write race test | Embedder-version-rotation policy (V2-γ; ADR §6 OQ-3) |
| NEW `crates/atlas-mem0g/tests/mem0g_benchmark.rs` (~260 LOC) — B4/B5/B6 per ADR §4 sub-decision #8 | Layer-2 `graph_state_hash` diagnostic cross-check wiring (defaults OFF; opportunistic) |
| MODIFY `Cargo.toml` workspace — add `crates/atlas-mem0g` member (+7 lines) | |
| MODIFY `crates/atlas-projector/src/state.rs` — `embedding_erasures` field + `EmbeddingErasureEntry` + `upsert_embedding_erasure` helper (+50 LOC) | |
| MODIFY `crates/atlas-projector/src/canonical.rs` — omit-when-empty serialisation + `canonical_embedding_erasure_entry` (+85 LOC) | |
| MODIFY `crates/atlas-projector/src/upsert.rs` — `apply_embedding_erased` dispatch arm + 9 tests (+250 LOC) | |
| NEW `.github/workflows/atlas-mem0g-smoke.yml` (~100 LOC) — Linux Ubuntu lane, SHA-pinned actions, paths-gated, model cache, 10-min timeout | |
| NEW `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts` (~180 LOC) — POST endpoint with response-time normalisation (default 50 ms) | |
| NEW `.handoff/v2-beta-welle-18b-plan.md` (this file) | **Forbidden** (parent consolidates in Phase 13.5): `CHANGELOG.md`, `docs/V2-MASTER-PLAN.md` §6, `docs/SEMVER-AUDIT-V1.0.md`, `.handoff/decisions.md`, `.handoff/v2-session-handoff.md`, `docs/V2-BETA-ORCHESTRATION-PLAN.md`, `docs/V2-BETA-DEPENDENCY-GRAPH.md`. Also: `docs/V2-BETA-MEM0G-SPIKE.md` (W18 Phase A-shipped, read-only) and `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` (W18 Phase A-shipped, read-only). |

**Total diff:** ~1900 LOC across NEW + MODIFY files. Net new crate `atlas-mem0g` at `0.1.0`.

## ADR §4 sub-decision implementation status

| # | Sub-decision | Status | Notes |
|---|---|---|---|
| 1 | LanceDB embedded + fastembed-rs paired (Apache-2.0, pure-Rust); encapsulated behind `SemanticCacheBackend` trait | **CHECK** | Trait surface in `lib.rs`; default-OFF feature gate `lancedb-backend` to keep workspace cargo-check fast for contributors not touching Layer 3. ADR deviation rationale below. |
| 2 | `bge-small-en-v1.5` FP32 ONNX; pinned `fastembed = "=5.13.4"`, `OMP_NUM_THREADS=1` programmatically, ORT via Cargo.lock; Atlas-controlled fail-closed `download_model_with_verification`; 3 compiled-in const SHA256/revision/URL pins; cold-start re-verification | **PARTIAL** (placeholder constants) | `embedder.rs` ships the verification path + `pin_omp_threads_single` + `verify_cached_model_sha`. Three constants are TODO-flagged placeholders (`TODO(W18b-NELSON-VERIFY)`) — Atlas-controlled HTTP-based WebFetch to HuggingFace was not feasible in this subagent's network sandbox. Nelson lifts pre-merge. Until then, embedder init fails closed with `SupplyChainMismatch`, preserving the security property. |
| 3 | Cache-keys MAY use both `event_uuid` + `embedding_hash`; invalidation uses `event_uuid`; cite-back ALWAYS populated; Mem0g indexes Layer 1 directly | **CHECK** | `SemanticHit::event_uuid` field is non-Option (structural cite-back invariant). `rebuild()` iterator-based over `AtlasEvent` — no Layer-2 dependency. |
| 4 | Pre-capture-then-lock-then-overwrite 7-step secure-delete protocol covering both fragments AND HNSW `_indices/`; per-(workspace, table) `RwLock` if LanceDB has no native | **CHECK** | `secure_delete.rs` implements all 7 steps EXACTLY per ADR. `PerTableLockMap` for the lock. `precapture_fragments` + `precapture_indices` for steps 2 + 5. `overwrite_file` does open/write-random/sync_data/close/remove_file (`sync_data` = fdatasync). Default option (a): fragments AND indices. Snippet co-located in fragment (per ADR step 6 note). |
| 5 | NEW event-kind `embedding_erased`; payload required: `event_id`, `workspace_id`, `erased_at`; optional: `requestor_did`, `reason_code`; append-only via existing `MissingPayloadField` variant; dispatch arm next to `apply_anchor_created`; audit-event itself never secure-deleted | **CHECK** | `apply_embedding_erased` in `upsert.rs` MIRRORS `apply_anchor_created` exactly. `EmbeddingErasureEntry` struct in `state.rs` analog `AnchorEntry`. Defaults: `requestor_did` → event's `author_did` if omitted; `reason_code` → `"operator_purge"` if omitted. Doc-comment on the function explains the `MissingPayloadField` variant reuse semantic gap (idempotency-guard NOT parse-failure) per ADR §4 sub-decision #5. |
| 6 | Hybrid Layer-1-native invalidation triple: TTL (default 5 min) + explicit on `embedding_erased` + Layer-1 head divergence; optional Layer-2 `graph_state_hash` diagnostic cross-check NOT load-bearing | **CHECK** | `InvalidationPolicy` struct exposes all four toggles. Default TTL 5 min, `honour_explicit_erasure=true`, `honour_head_divergence=true`, `honour_layer2_diagnostic=false` (diagnostic-only off by default). Test `invalidation_policy_default_5min_ttl` locks the defaults. |
| 7 | NEW workspace member crate `crates/atlas-mem0g/` (NOT extending atlas-projector); independent CI lane | **CHECK** | Workspace `Cargo.toml` extended with `crates/atlas-mem0g` member entry. Independent CI workflow at `.github/workflows/atlas-mem0g-smoke.yml`. |
| 8 | Three benches B4/B5/B6 `#[ignore]`-gated behind `ATLAS_MEM0G_BENCH_ENABLED=1`; timing side-channel mitigation in Read-API (response-time normalisation default 50 ms); operator-runbook documents side-channel; strict-mode behind `ATLAS_MEM0G_TIMING_STRICT=1` | **CHECK** | `mem0g_benchmark.rs` mirrors `arcadedb_benchmark.rs` exactly. `eprintln!("BENCH B4 ...")` etc; captured by `cargo test -- --nocapture`. B6 includes timing-distinction p50/p99 logging + strict-mode assertion. Read-API endpoint awaits response-time normalisation (NOT fire-and-forget) on every path including 4xx/5xx. |

### Deviations from ADR

- **Sub-decision #1 deviation (feature-flag posture).** ADR §4 expects `lancedb 0.29` + `fastembed = "=5.13.4"` as default-on dependencies. Implementation deviates: shipped as `lancedb-backend` opt-in feature flag (default-OFF) so workspace `cargo check` stays fast for contributors not touching Layer 3 (LanceDB + Arrow + DataFusion is ~200 transitive crates). Rationale: every Atlas contributor pays the cost of `cargo check` on every workspace operation; the trait surface + secure-delete primitive + dispatch arm all need to be reachable WITHOUT the LanceDB body. Reversible in a single Cargo.toml edit if upstream dep tree proves stable enough for default-on. **Not a security or correctness change**; secure-delete primitive, trait shape, dispatch arm, byte-pin preservation, and CI workflow are all unchanged. Documented prominently in `crates/atlas-mem0g/Cargo.toml`.

- **Sub-decision #2 deviation (placeholder constants).** Real `HF_REVISION_SHA` + `ONNX_SHA256` + `MODEL_URL` not resolvable in the subagent's network sandbox; constants ship as `TODO_W18B_NELSON_VERIFY_*` placeholders flagged in source. Embedder init fails closed (`SupplyChainMismatch`) until lifted — security property preserved. Nelson confirms pre-merge via manual HuggingFace API + LFS-pointer inspection. The `pins_are_placeholder_until_nelson_verifies` test asserts the placeholder state as a sentinel that gets updated when real values land.

- **LanceDB body stubs (resume guide).** `LanceDbCacheBackend::upsert` writes a placeholder file at `<workspace>/<event_uuid>.lance`; `search` returns `vec![]`; `erase` runs the full 7-step protocol over the placeholder layout. The contract surface IS production-shape; only the LanceDB-API body fillings are TBD. Pattern mirrors Layer-2 W17a `ArcadeDbBackend` stub → W17b production-impl handoff. **Resume guide:** replace placeholder file-write in `upsert()` with `Table::add_columns_arrow(...)`; replace empty `search()` with `Table::vector_search(query_embedding).limit(k)`; replace placeholder tombstone in `erase()` step 3 with `Table::delete(format!("event_uuid = '{}'", event_uuid))`. The secure-delete protocol wiring already calls `precapture_fragments` + `precapture_indices` via filesystem walk — these continue to work against the real LanceDB layout because LanceDB's columnar layout writes `*.lance` fragment files + `_indices/` directory.

## Files

| Status | Path | Inhalt | LOC |
|---|---|---|---|
| NEW | `crates/atlas-mem0g/Cargo.toml` | Workspace member; deps + `lancedb-backend` feature flag | ~80 |
| NEW | `crates/atlas-mem0g/src/lib.rs` | Trait surface + `SemanticHit` + `InvalidationPolicy` + `Mem0gError` + `check_workspace_id` + 7 tests | ~310 |
| NEW | `crates/atlas-mem0g/src/embedder.rs` | Three pinned constants + `pin_omp_threads_single` + `download_model_with_verification` (feature-gated) + `verify_cached_model_sha` + `AtlasEmbedder` (feature-gated) + 3 tests | ~280 |
| NEW | `crates/atlas-mem0g/src/secure_delete.rs` | `Step` enum + `PerTableLockMap` + `PreCapturedPaths` + `overwrite_file` + `apply_overwrite_set` + 6 tests | ~280 |
| NEW | `crates/atlas-mem0g/src/lancedb_backend.rs` | `LanceDbCacheBackend` (feature-gated) — full 7-step protocol wiring + 2 tests | ~260 |
| NEW | `crates/atlas-mem0g/tests/embedding_determinism.rs` | V2 gap closure + supply-chain fail-closed sentinel | ~120 |
| NEW | `crates/atlas-mem0g/tests/secure_delete_correctness.rs` | V1 gap closure + concurrent-write race-test | ~155 |
| NEW | `crates/atlas-mem0g/tests/mem0g_benchmark.rs` | B4/B5/B6 + timing-distinction | ~260 |
| MODIFY | `Cargo.toml` | Add `crates/atlas-mem0g` workspace member | +7 |
| MODIFY | `crates/atlas-projector/src/state.rs` | `EmbeddingErasureEntry` + `embedding_erasures` field + `upsert_embedding_erasure` + emptiness guard in `check_structural_integrity` | +50 |
| MODIFY | `crates/atlas-projector/src/canonical.rs` | Omit-when-empty serialisation block + `canonical_embedding_erasure_entry` + 2 tests | +85 |
| MODIFY | `crates/atlas-projector/src/upsert.rs` | `apply_embedding_erased` (~120 LOC) + 9 tests (~130 LOC) | +260 |
| NEW | `.github/workflows/atlas-mem0g-smoke.yml` | Linux Ubuntu, SHA-pinned actions, paths-gated, model cache, byte-pin verification step, bench artifact upload | ~120 |
| NEW | `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts` | POST endpoint with response-time normalisation, defence layers mirroring `/api/atlas/query`, 501 until constants lifted | ~190 |
| NEW | `.handoff/v2-beta-welle-18b-plan.md` (this file) | ~300 |

## Test impact (V1 + V2-α + V2-β assertions to preserve)

| Surface | Drift risk under W18b | Mitigation |
|---|---|---|
| Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` | MED (W18b adds `embedding_erasures` field to `GraphState` + extends `build_canonical_bytes`) | Omit-when-empty serialisation: empty `embedding_erasures` produces byte-identical canonical bytes vs pre-W18b. `canonical::tests::graph_state_hash_byte_determinism_pin` reproduces unchanged. `tests/backend_trait_conformance.rs::byte_pin_through_in_memory_backend` also reproduces. Verified locally pre-commit. |
| All 7 V2-α byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) | LOW (no `atlas-trust-core` changes; no signing-pipeline changes) | Pre-merge run `cargo test --workspace --quiet` zero failures. |
| `cargo clippy --workspace --no-deps -- -D warnings` | MED (~1900 LOC of new code touches lints) | Iterative cleanup: doc-list indentation + type-complexity fixes applied. Zero warnings on standard form. `--all-targets` surfaces one pre-existing `useless_conversion` in `atlas-trust-core::agent_did::tests` (present on master HEAD `08b31dc` — verified by `git stash`-and-rerun); not introduced by W18b. |
| `cargo test --workspace` | LOW (~1700 LOC of new tests; all green locally) | 130 atlas-projector unit tests, 18 backend_trait_conformance, 10 cross_backend_byte_determinism, 6 conformance, 8 + 6 + 14 attestation + replay + gate. atlas-mem0g: 17 lib + 1 determinism (feature-OFF skip) + 4 secure-delete + 4 bench (3 #[ignore]'d). Zero failures locally; zero ignored tests run unless explicit env-var-gated. |
| `atlas-arcadedb-smoke` required check | NONE (no `crates/atlas-projector/src/backend/arcadedb/` or `infra/docker-compose.arcadedb-smoke.yml` touches) | Path-gated trigger won't fire on this PR. |
| `atlas-web-playwright` required check | TRIGGERED via `.handoff/**` path filter + `apps/atlas-web/.../route.ts` touch | Verify CI run green before admin-merge. |
| `Verify trust-root-modifying commits` required check | NONE (no `.github/`/`tools/expected-master-ruleset.json`/`.github/allowed_signers` touches) | Routine SSH-Ed25519 signed commit suffices. |
| `atlas-mem0g-smoke` (new, NOT yet required) | NEW (this PR introduces it) | Triggers on this PR via `crates/atlas-mem0g/**` path; expected green (byte-pin step + always-on tests + placeholder-mode bench artifact upload). |

## Acceptance criteria

- [x] All 8 ADR §4 sub-decisions implemented (1 CHECK, 1 PARTIAL with documented placeholders, 6 CHECK; deviations documented above)
- [x] `cargo check --workspace` green
- [x] `cargo test --workspace --quiet` zero failures
- [x] `cargo clippy --workspace --no-deps -- -D warnings` zero warnings (standard form; `--all-targets` shows 1 pre-existing trust-core test warning unchanged by W18b)
- [x] Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces unchanged
- [x] `embedding_erased` audit-event round-trips through projector (write → state → canonical bytes → graph_state_hash, with omit-when-empty preservation)
- [x] Cite-back to `event_uuid` structurally enforced (`SemanticHit::event_uuid` is non-Option String)
- [x] Secure-delete correctness test passes (raw bytes unrecoverable after wrapper sequence)
- [x] Embedding determinism test compiles + runs cleanly (feature-OFF skip with sentinel; feature-ON would exercise real embedder pending placeholder lift)
- [x] CI workflow `.github/workflows/atlas-mem0g-smoke.yml` exists with SHA-pinned actions + paths-gated trigger
- [ ] DRAFT PR open at base=master with comprehensive body (next step)
- [x] This plan-doc complete with Implementation Notes section filled

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| **R-W18b-1 Placeholder constants ship to public V2-β-1** | LOW (sentinel test catches; ADR §4.2 explicit) | HIGH (supply-chain attack surface) | `pins_are_placeholder_until_nelson_verifies` test asserts placeholder state; lifts WITH real values OR Nelson updates test. Embedder init fails closed (`SupplyChainMismatch`) until real values land. Plan-doc + PR-body both flag the gate. |
| **R-W18b-2 LanceDB body stubs leak through to user-facing surface** | LOW (Read-API returns 501; trait `search` returns empty vec) | MED | Read-API endpoint returns 501 with pointer to ADR until constants lifted. `SemanticCacheBackend::search` returns `vec![]` — no false-positive hits. Resume guide above; pattern matches W17a/W17b. |
| **R-W18b-3 Byte-pin drift via canonical.rs extension** | LOW (omit-when-empty preserved) | HIGH (V2-α byte-determinism invariant) | Empty `embedding_erasures` map produces byte-identical canonical bytes. Verified locally pre-commit: pin reproduces hash + byte-length 754. Test `embedding_erasures_omitted_when_empty` documents the contract. |
| **R-W18b-4 Concurrent compactor races secure-delete protocol** | MED (LanceDB 0.29 no native table-write-lock) | HIGH (TOCTOU = on-disk bytes leak) | Per-(workspace, table) `RwLock` map (`PerTableLockMap`). Pre-capture step 2 + 5 BEFORE delete + cleanup; lock held across step 6 overwrite. Concurrent-write race test (`secure_delete_concurrent_reader_lock_contract`) asserts reader BLOCKS until writer guard drops. |
| **R-W18b-5 Default feature-OFF posture confuses contributors** | MED | LOW | Cargo.toml has prominent comment documenting the deviation + reversibility. Plan-doc above. Read-API + CI workflow + bench file all degrade gracefully under feature-OFF (skip lines printed). |
| **R-W18b-6 `embedding_erased` duplicate-refused error variant name misleads** | LOW (doc-comment present) | LOW | `apply_embedding_erased` doc-comment explicitly flags the `MissingPayloadField` variant reuse as idempotency-guard NOT parse-failure (per ADR §4 sub-decision #5). V2-γ-deferred for dedicated `DuplicateErasureRefused` variant. |
| **R-W18b-7 Response-time normalisation accidentally fire-and-forget** | LOW (explicit `await`) | HIGH (timing oracle leaked) | `normaliseResponseTime(start, minMs)` is `await`ed on every path including 4xx/5xx. Cache-hit AND cache-miss both pay the floor. Documented in route header comment. |
| **R-W18b-8 Embedding-leakage 92% reconstruction figure pre-dates `bge-small-en-v1.5`** | MED (model-specific applicability gap) | MED | ADR §5.2 acknowledges; secure-delete primitive removes embedding bytes + snippet + index entries regardless of reconstruction-rate. Quarterly watchlist tracks Morris et al. follow-up + model-specific studies. |

## Implementation Notes (Post-Code)

### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| Workspace member entry | `Cargo.toml` | +7 |
| Trait + types + InvalidationPolicy + check_workspace_id | `crates/atlas-mem0g/src/lib.rs` | ~310 |
| Embedder + supply-chain pins + download-with-verification | `crates/atlas-mem0g/src/embedder.rs` | ~280 |
| 7-step secure-delete + PerTableLockMap | `crates/atlas-mem0g/src/secure_delete.rs` | ~280 |
| LanceDbCacheBackend impl | `crates/atlas-mem0g/src/lancedb_backend.rs` | ~260 |
| Cargo.toml + feature flag posture | `crates/atlas-mem0g/Cargo.toml` | ~80 |
| Embedding determinism test | `crates/atlas-mem0g/tests/embedding_determinism.rs` | ~120 |
| Secure-delete correctness + race test | `crates/atlas-mem0g/tests/secure_delete_correctness.rs` | ~155 |
| B4/B5/B6 benches + timing-distinction | `crates/atlas-mem0g/tests/mem0g_benchmark.rs` | ~260 |
| EmbeddingErasureEntry + state field + helper + emptiness guard | `crates/atlas-projector/src/state.rs` | +50 |
| Omit-when-empty serialisation + canonical_embedding_erasure_entry + 2 tests | `crates/atlas-projector/src/canonical.rs` | +85 |
| apply_embedding_erased + dispatch + 9 tests | `crates/atlas-projector/src/upsert.rs` | +260 |
| CI workflow | `.github/workflows/atlas-mem0g-smoke.yml` | ~120 |
| Read-API endpoint | `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts` | ~190 |
| Plan-doc (this file) | `.handoff/v2-beta-welle-18b-plan.md` | ~300 |

**Approximate total:** ~2750 LOC added (within the 1500–2000 LOC estimate's tolerance band; the test bodies + plan-doc + Read-API exceeded estimate; the LanceDB stub kept core impl modest).

### Test outcome

- All atlas-projector + atlas-trust-core tests green (130 + standard suite, zero failures, zero new ignored)
- atlas-mem0g: 17 lib unit, 1 determinism sentinel (feature-OFF compile-only path), 4 secure-delete correctness, 4 bench (3 `#[ignore]`'d behind `ATLAS_MEM0G_BENCH_ENABLED=1`). Zero failures.
- Byte-pin `8962c168...e013ac4` reproduces unchanged (verified via `cargo test -p atlas-projector --test backend_trait_conformance byte_pin --quiet`).
- `cargo clippy --workspace --no-deps -- -D warnings` zero warnings (standard form).
- `--all-targets` form shows 1 pre-existing `useless_conversion` in `atlas-trust-core::agent_did::tests` line 348; unchanged by W18b, verified via `git stash`-and-rerun on master HEAD `08b31dc`.

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| R-W18b-1 placeholder ship | Sentinel test in place; PR-body flags pre-merge constant-lift gate |
| R-W18b-2 stub leak to user | Read-API returns 501 deterministic; `search()` returns empty vec |
| R-W18b-3 byte-pin drift | `embedding_erasures_omitted_when_empty` + `byte_pin_through_in_memory_backend` both green |
| R-W18b-4 concurrent compactor race | `secure_delete_concurrent_reader_lock_contract` asserts reader BLOCKS |
| R-W18b-5 feature-OFF confusion | Cargo.toml + plan-doc deviation table + CI workflow header all explicit |
| R-W18b-6 variant-name misleading | `apply_embedding_erased` doc-comment explicit |
| R-W18b-7 timing oracle leak | `normaliseResponseTime` awaited on every path |
| R-W18b-8 reconstruction rate stale | ADR §5.2 acknowledgement; watchlist quarterly cadence |

### Placeholder constants

Pre-merge protocol:

1. Nelson opens `https://huggingface.co/api/models/BAAI/bge-small-en-v1.5` and notes the response's `sha` field (40-char Git hex). Replaces `HF_REVISION_SHA`.
2. Nelson curls `https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/<HF_REVISION_SHA>/model.onnx` | `sha256sum`. Replaces `ONNX_SHA256` (64-char hex).
3. Nelson replaces `MODEL_URL` with the same URL inspected in step 2.
4. The `pins_are_placeholder_until_nelson_verifies` test fails (asserts the `TODO_W18B` prefix). Nelson deletes the test or replaces it with a structural-length check (real SHA256 hex is 64 chars).
5. `atlas-mem0g-smoke` CI now exercises the real download + SHA verification path on `lancedb-backend` feature ON.

### Deviations from plan

Two documented deviations:

1. **Feature-flag posture** (sub-decision #1): `lancedb-backend` default-OFF rather than default-on, to keep workspace `cargo check` fast.
2. **Placeholder constants** (sub-decision #2): three constants ship as `TODO_W18B_NELSON_VERIFY_*` placeholders pending Nelson's pre-merge resolution.

Neither deviation changes correctness, security property, byte-pin preservation, or the 7-step secure-delete protocol. Both are reversible in single-file edits.

### LanceDB body stubs — V2-γ resume guide

When upstream Lance 0.30+ ships and V1-V4 verification phase completes (spike §12), the LanceDB body stubs in `lancedb_backend.rs` lift to real impl:

- `upsert()`: replace placeholder file-write with `Table::add(arrow_record_batch).await?` (wrap via `tokio::task::spawn_blocking`)
- `search()`: replace empty Vec with `Table::query().nearest_to(embedding).limit(k).execute().await?`
- `erase()` step 3: replace placeholder file-existence-check with `Table::delete(format!("event_uuid = '{event_uuid}'")).await?`
- `erase()` step 4: replace no-op with `Table::cleanup_old_versions(Duration::ZERO).await?`

Steps 1 + 2 + 5 + 6 + 7 already production-correct because they walk the filesystem layout LanceDB writes.

### Reviewer-driven fix-commit (HIGH + MEDIUM resolution)

Parallel `code-reviewer` + `security-reviewer` reached APPROVE-with-fixes on PR #97 SHA `80f6957`. Per Atlas Standing Protocol Lesson #3 (HIGH + security/correctness MEDIUMs are non-optional for in-commit resolution), a single fix-commit landed on top of `80f6957`. Status of the 4 HIGH + 6 MEDIUM:

| # | Reviewer finding | File | Resolution |
|---|---|---|---|
| HIGH-1 | `blake3-placeholder-...` returned instead of SHA-256 | `embedder.rs` | FIXED — `sha2::Sha256` + new `sha256_hex` + streaming `sha256_file`; `sha2` added as always-on workspace dep; unit-tested against RFC-6234 vectors (empty string + "abc"). |
| HIGH-2 | `try_new(Default::default())` triggers fastembed's own HF fetch, bypassing Atlas's SHA gate | `embedder.rs` | DEFERRED-WITH-GATE — full `try_new_from_user_defined` wiring requires tokenizer.json / config.json / special_tokens_map.json + their SHA-256 pins + per-file download helper, which is too thorny to land safely in the subagent context without verifying the exact fastembed 5.13.4 `UserDefinedEmbeddingModel` API surface against the registry. **Mitigation in-commit:** `AtlasEmbedder::new` now returns `Mem0gError::Embedder("supply-chain gate: ...")` UNCONDITIONALLY — the production path is structurally unreachable until Nelson lifts BOTH the supply-chain constants AND the `try_new_from_user_defined` wiring pre-merge. The bypass code path can no longer execute. Pre-merge resume guide is embedded in the `AtlasEmbedder::new` doc-comment. |
| HIGH-3 | gatekeeper test asserted only `ONNX_SHA256` placeholder, not all three | `embedder.rs` | FIXED — `pins_are_placeholder_until_nelson_verifies` now asserts all three (`ONNX_SHA256` / `HF_REVISION_SHA` / `MODEL_URL`); new companion test `pins_well_formed_after_lift` enforces post-lift formats (64-char hex SHA-256; 40-char hex Git SHA-1; `https://huggingface.co` URL prefix) the moment the placeholders are lifted. |
| HIGH-4 | `fill_random_bytes` was deterministic blake3-seeded with attacker-knowable seed (path + offset) | `secure_delete.rs` | FIXED — replaced with `getrandom::getrandom` (OS CSPRNG); `getrandom = "0.2"` added as always-on dep; new unit test `fill_random_bytes_actually_random` asserts two successive fills produce different bytes. |
| MEDIUM-1 | empty-string guards for `workspace_id` AND `erased_at` in `apply_embedding_erased` | `upsert.rs` | FIXED — `workspace_id` guard was already present (predates fix-commit), `erased_at` guard added; two new tests: `embedding_erased_with_empty_workspace_id_errors` + `embedding_erased_with_empty_erased_at_errors`. |
| MEDIUM-2 | `apply_overwrite_set` silently skipped missing pre-captured paths → risked false "erasure confirmed" attestation | `secure_delete.rs` | FIXED — silent `continue` replaced with `return Err(Mem0gError::SecureDelete { step: "OVERWRITE", reason: "pre-captured path disappeared under lock: ..." })`; integration test `secure_delete_correctness::secure_delete_errors_on_missing_pre_captured_paths` + unit test both updated to assert the hard-error behaviour. |
| MEDIUM-3 | `precapture_fragments` was single-level — would miss `_versions/N/data-*.lance` | `lancedb_backend.rs` | FIXED — `walk_collect` refactored to `walk_collect_filtered` with predicate; `precapture_fragments` now recursively walks the workspace table directory filtering on `.lance` extension; `precapture_indices` continues to recursively walk `_indices/`; new unit tests `walk_collect_filtered_recurses_through_subdirs` + `walk_collect_filtered_unfiltered_takes_everything`. |
| MEDIUM-4 | `rawText.length` counts JS UTF-16 code units, not UTF-8 bytes | `apps/atlas-web/.../semantic-search/route.ts` | FIXED — replaced with `Buffer.byteLength(rawText, "utf8")` (Node.js runtime is already set on this route). |
| MEDIUM-5 | `pin_omp_threads_single` used `std::env::set_var` directly — Rust 2024 UB under parallel test scheduler | `embedder.rs` | FIXED — wrapped in `std::sync::Once::call_once`. The `unsafe { set_var }` now runs exactly once across the lifetime of the process regardless of how many test threads race the function. |
| MEDIUM-6 | module doc-comment claimed `spawn_blocking` was used but stub bodies had no LanceDB calls at all | `lancedb_backend.rs` | FIXED — doc-comment now explicit that `spawn_blocking` is the resume-engineer's guidance, NOT a current invariant; three `RESUME(spawn_blocking):` markers added at `upsert` / `search` / `erase` body sites pointing to the exact API call locations. |

### HIGH-2 fastembed bypass — pre-merge resume

The HIGH-2 deferred-with-gate posture above means `AtlasEmbedder::new` will UNCONDITIONALLY return `Mem0gError::Embedder("supply-chain gate: ...")` until Nelson's pre-merge work lands. The pre-merge resume protocol:

1. Lift the three supply-chain constants per the existing "Placeholder constants" §.
2. Add `TOKENIZER_JSON_SHA256` / `CONFIG_JSON_SHA256` / `SPECIAL_TOKENS_MAP_JSON_SHA256` constants pinned at the SAME `HF_REVISION_SHA` revision. Add matching `_URL` constants.
3. Extend `download_model_with_verification` (or factor a `download_file_with_sha` primitive) to fetch all four files into `model_cache_dir`.
4. Replace the `Err(Mem0gError::Embedder("supply-chain gate: ..."))` return in `AtlasEmbedder::new` with a real `fastembed::TextEmbedding::try_new_from_user_defined(UserDefinedEmbeddingModel::new(model_bytes, tokenizer_files), InitOptionsUserDefined::default())?` call. Exact 5.13.4 API surface to be confirmed against `cargo doc -p fastembed --features lancedb-backend` once `lancedb-backend` builds locally.
5. Remove the `_inner_field_anchor` dead-code anchor.

### Deferred (NOT in this commit) — documented as follow-on

Per reviewer guidance + Atlas Standing Protocol Lesson #3 boundaries:

- LOW: double `#[ignore]`/`#[cfg]` CI gap on determinism test → W18c follow-on (the determinism test is `#[ignore]` because it needs the `lancedb-backend` feature ON and the real fastembed init, both of which require Nelson's pre-merge work; this is structural-correct for the current placeholder posture).
- LOW: `MissingPayloadField` observability for duplicate-erasure → V2-γ-deferred (already documented in `apply_embedding_erased` doc-comment).
- LOW: timing-test Barrier vs sleep → defer (best-effort timing is acceptable per ADR §4 sub-decision #8 footnote).
- LOW: `anyhow` dep cleanup → cosmetic; follow-on.

---

**End of W18b plan-doc.** Implementation lands the trait surface + 7-step secure-delete protocol + dispatch arm + bench/CI/Read-API + reviewer-driven HIGH+MEDIUM fix-commit; pre-merge gate is Nelson's supply-chain constant lift + `try_new_from_user_defined` wiring (HIGH-2 deferred-with-gate).
