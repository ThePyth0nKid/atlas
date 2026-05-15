# Changelog

All notable changes to Atlas are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
as of v1.0.0.

Atlas ships as a coherent system across multiple workspace crates and packages
(`atlas-trust-core`, `atlas-verify-cli`, `atlas-verify-wasm`, `atlas-signer`,
`atlas-witness`, `@atlas/bridge`, `atlas-web`, `atlas-mcp-server`,
`@atlas-trust/verify-wasm`). Version numbers move in lockstep — a `v1.0.0` tag
covers every workspace member.

The v1.0 public-API surface contract is documented in
[`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md).

## [Unreleased]

**V2-β Phase 4 + 5 + 6 + 7 + 8 + 9 + 10 + 11 + 12 + 13 + 13.5 + 13.6 + W18c-A all landed on master 2026-05-13 → 2026-05-15 — `v2.0.0-beta.1` candidate.** W18c Phase A (supply-chain constant lift) clears the W18b `TODO_W18B_NELSON_VERIFY_*` placeholders against `BAAI/bge-small-en-v1.5` at HF revision `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`; the embedder still fails closed pending W18c Phase B `try_new_from_user_defined` wiring. Next welle: W18c Phase B OR W19 ship convergence (parallel-tracks; sequencing per parent).

### Changed — V2-β Welle 18c Phase A (Mem0g supply-chain constants lifted, 2026-05-15)

- **MODIFY `crates/atlas-mem0g/src/embedder.rs`** — replaced three W18b `TODO_W18B_NELSON_VERIFY_*` placeholders with real values resolved against HuggingFace `BAAI/bge-small-en-v1.5` at revision `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`:
  - `HF_REVISION_SHA` — 40-char Git SHA-1 hex digest pinning the model-card version.
  - `ONNX_SHA256` — 64-char SHA-256 of `model.onnx` (FP32, 133,093,490 bytes / 126.93 MB; matches spike §3.4 expected envelope as W18c V4 verification).
  - `MODEL_URL` — full HuggingFace LFS URL embedding the revision SHA in path; TLS-pinned via Atlas's `https_only(true)` reqwest configuration.
- **ADD three Phase-B-prep constants in same file** — `TOKENIZER_JSON_SHA256` / `CONFIG_JSON_SHA256` / `SPECIAL_TOKENS_MAP_SHA256` (64-char SHA-256 each, consumed by W18c Phase B `fastembed::TextEmbedding::try_new_from_user_defined` wiring) plus matching `TOKENIZER_JSON_URL` / `CONFIG_JSON_URL` / `SPECIAL_TOKENS_MAP_URL` (revision-pinned huggingface.co LFS URLs). Atomic lift across all 5 hash digests (1 × SHA-1 + 4 × SHA-256) + 4 URLs = 9 compile-in pins.
- **`_STRUCTURAL_PIN_CHECK` const block extended** to assert non-emptiness of all nine compile-in pins.
- **Test `pins_are_placeholder_until_nelson_verifies` retired** (W18b gatekeeper purpose served); **`pins_well_formed_after_lift` upgraded** to unconditional structural enforcement across all 5 hash digests + 4 URLs (length + lowercase-hex + `https://huggingface.co/` origin + revision-SHA-in-path invariants). The W18b `is_placeholder` early-return is removed; any future refactor that reintroduces placeholder strings trips the assertions at test time.
- **`pins_are_non_empty` test extended** to cover all 9 pin constants.
- **AtlasEmbedder::new docstring + error-message refresh** — replaced "Pre-merge resume guide (Nelson)" with "W18c Phase B resume guide (engineering)" pointing at `.handoff/v2-beta-welle-18c-plan.md` Phase B; the `Mem0gError::Embedder("supply-chain gate: …")` return text now references Phase A complete / Phase B pending posture. Fail-closed semantics unchanged — the embedder still refuses to construct until Phase B wiring lands.
- **NEW `tools/w18c-phase-a-resolve.sh`** (~80 lines) — auditable helper script that resolved the six values: fetches HF API revision SHA, downloads + sha256sums the ONNX file, downloads + sha256sums the 3 tokenizer files, prints all values + ONNX file-size for V4 spike-§12 verification. Re-runnable for future revision rotations; deterministic-by-revision-SHA.
- **Module-level docstring refresh** — `## TODO(W18b-NELSON-VERIFY)` section replaced with `## W18c Phase A — supply-chain constants lifted (2026-05-15)` documenting the resolution audit trail + spike §3.4 envelope match + W18c Phase B remaining-gate clarification.
- **Atlas-web-playwright trigger:** this PR touches `.handoff/v2-beta-welle-18c-plan.md` (status-note for Phase A SHIPPED) to satisfy the path-filter required-check per Lesson #11.

### Added — V2-β Phase 13.6 (handoff-prep — W19 + W18c plan-docs master-resident, 2026-05-15)

- **NEW `.handoff/v2-beta-welle-19-plan.md`** (~280 lines) — W19 v2.0.0-beta.1 ship convergence plan-doc. Complete with: scope-table (5 manifest version bumps + CHANGELOG conversion + 2 NEW companion docs `docs/V2-BETA-1-RELEASE-NOTES.md` + `docs/SEMVER-AUDIT-V2.0-beta.md` + README update); locked decisions (version scheme `2.0.0-beta.1` SemVer prerelease, dist-tag `latest` per V2-α-α.2 precedent, signed annotated tag, prerelease-flagged GitHub Release, separate SEMVER-AUDIT companion doc rather than V1.0 §10 append); file-by-file MODIFY list with line-numbers; test impact matrix (7 byte-pins NONE drift; atlas-web-playwright triggered via `apps/atlas-web/package.json` version bump); 13-step implementation outline (pre-flight → 5 version bumps → CHANGELOG conversion → 2 NEW companion docs → README → local verify → SSH-signed commit → reviewer-dispatch → admin-merge → POST-MERGE signed tag + Release + npm publish + Sigstore Build L3 provenance verification); 16-item acceptance criteria; 6-risk register (R-W19-1 beta-tag premature operational signal, R-W19-2 wasm-publish race-fix regression, R-W19-3 dist-tag latest confusion, R-W19-4 beta-version-comparator edge cases, R-W19-5 scaffold-ship customer adoption risk, R-W19-6 tag-immutability under hook failure); subagent dispatch prompt skeleton (anti-divergence enforcement); Implementation Notes template for post-ship completion. Scaffold-honesty disclosure mandate for release notes (Layer 3 returns 501 stub until W18c lifts).
- **NEW `.handoff/v2-beta-welle-18c-plan.md`** (~200 lines) — W18c parallel-track plan-doc for Mem0g operational activation. Four phases: **Phase A** (Nelson HuggingFace constant lift, ~30 min Nelson + ~10 min agent; step-by-step curl + sha256sum + 6 const updates incl. 3 W18c-NEW tokenizer-file SHA-256 pins); **Phase B** (fastembed `try_new_from_user_defined` wiring replacing W18b's unconditional fail-closed `AtlasEmbedder::new`; ~1 session); **Phase C** (V1-V4 verification gap closure with cross-platform CI matrix Linux + Windows + macOS; ~1 session; fallback policy event_uuid-only cache-key on Windows if V2 fails); **Phase D** (LanceDB ANN/search body fill-in replacing `Mem0gError::Backend("not yet wired")` placeholders with `tokio::task::spawn_blocking`-wrapped LanceDB calls — NEVER `Handle::current().block_on()` per W18b ADR + Lessons #16-17; ~1-2 sessions). 7-risk register (R-W18c-A1 wrong revision SHA, R-W18c-B1 fastembed API drift, R-W18c-C1 Windows determinism fail, R-W18c-D1 LanceDB API drift, R-W18c-D2 block_on deadlock, R-W18c-E1 latency budget, R-W18c-F1 intentional ship-before-W18c). Phase-specific subagent dispatch prompt skeleton. ADR-Atlas-013 contingency open if implementation surfaces design amendment. Operator-runbook V2-β chapter update as cross-cutting concern post Phase-D.
- **MODIFY `.handoff/v2-session-handoff.md`** — §0-NEXT prominent "🎯 RECOMMENDED PICKUP" callout pointing at the two new plan-docs; worktree cleanup note for `.claude/worktrees/agent-a598572963378b967` (locked at admin-merge time per Atlas Lesson #9 cosmetic-error pattern; double-force unlock commands documented but flagged as destructive).
- **Strategic rationale:** the W18 plan-doc pattern (master-resident `.handoff/v2-beta-welle-18-plan.md` with ready-to-dispatch subagent skeleton, shipped in PR #95) saved W18b dispatch ~60-90 min cognitive load. Phase 13.6 replicates the pattern for both upcoming work-streams (W19 ship + W18c activation) so a fresh agent enters either with immediate-action clarity. No code touched; no CI risk; reversibility HIGH.

### Added — V2-β Welle 18b (Mem0g Layer-3 cache implementation, 2026-05-14)

- **NEW workspace member crate `crates/atlas-mem0g/`** (~2300 LOC across `Cargo.toml` + 4 src modules + 3 test modules). Implements ADR-Atlas-012 §4 sub-decisions 1-8 as a W17a-pattern Phase-A-scaffold: trait surface + protocol + dispatch wiring + supply-chain path all production-shape; LanceDB ANN/search bodies surface `Mem0gError::Backend` placeholders until W18c follow-on closes the spike §12 V1-V4 verification gaps. `lancedb-backend` feature default-OFF so workspace `cargo check` stays fast for contributors not touching Layer 3 (~200 transitive crates incl. Arrow + DataFusion gated behind feature; single-edit reversible).
- **`crates/atlas-mem0g/src/lib.rs`** (~492 LOC) — production `SemanticCacheBackend` trait per spike §7. Trait is object-safe + `Send + Sync`; sync methods (mirrors Layer-2 `GraphStateBackend` convention) for the eventual `tokio::task::spawn_blocking` wrapping (NOT `Handle::current().block_on()` — deadlocks under single-threaded scheduler). Public types: `SemanticHit { event_uuid (ALWAYS present — cite-back trust), workspace_id, entity_uuid: Option, score: f32, snippet }`, `InvalidationPolicy` (TTL + on-event + Layer-1-head-divergence triple), `Mem0gError` (`#[non_exhaustive]` enum covering `Io` / `Embedder` / `SupplyChainMismatch` / `SecureDelete { step, reason }` / `Backend` / `InvalidWorkspaceId`). Re-exports projector helpers `check_workspace_id` + `check_value_depth_and_size` for boundary defence at every backend-boundary `serde_json::Value` parse.
- **`crates/atlas-mem0g/src/embedder.rs`** (~570 LOC) — Atlas-controlled supply-chain verification path per ADR §4 sub-decision #2. Three compiled-in `const` values (`HF_REVISION_SHA` + `ONNX_SHA256` + `MODEL_URL`) ship as `TODO_W18B_NELSON_VERIFY_*` placeholders with fail-closed posture: `AtlasEmbedder::new` returns `Mem0gError::Embedder("supply-chain gate: ...")` unconditionally until Nelson lifts. **HIGH-1 reviewer fix:** real SHA-256 via `sha2::Sha256::Digest::digest` (was blake3-prefixed placeholder — would silently always fail on Nelson constant lift). Unit-tested against RFC-6234 vectors. **HIGH-3 reviewer fix:** `pins_are_placeholder_until_nelson_verifies` gatekeeper test asserts ALL three constants + post-lift well-formedness companion test (was single-constant gate; partial-lift would have silently passed). `pin_omp_threads_single` wraps `std::env::set_var("OMP_NUM_THREADS", "1")` in `std::sync::Once::call_once` (**MEDIUM-5 reviewer fix:** Rust 2024 `set_var` UB closure under parallel test scheduler — previous unsafe block raced on global `environ`). `download_model_with_verification` streams via `sha256_file`; fail-closed re-verification at every cold start.
- **`crates/atlas-mem0g/src/secure_delete.rs`** (~475 LOC) — production 7-step pre-capture-then-lock-then-overwrite protocol per ADR §4 sub-decision #4. Sequence: ACQUIRE per-`(workspace, table)` `tokio::sync::RwLock` write lock → PRE-CAPTURE fragment paths → `lancedb::Table::delete()` → `cleanup_old_versions(Duration::ZERO)` → PRE-CAPTURE HNSW `_indices/` paths → OVERWRITE each pre-captured path (random bytes equal to file size + `fdatasync` + `remove_file`) → RELEASE lock → emit `embedding_erased` audit-event OUTSIDE the lock (deadlock avoidance). **HIGH-4 reviewer fix:** `getrandom::getrandom` OS CSPRNG (was deterministic blake3-seeded keyed on `(path, remaining)` — adversary with workspace storage layout could recompute exact overwrite pattern). **MEDIUM-2 reviewer fix:** missing pre-captured path under lock surfaces `Mem0gError::SecureDelete { step: "OVERWRITE", reason: "disappeared under lock: ..." }` rather than silently continuing (was false-attestation risk). `PerTableLockMap` + `PreCapturedPaths` helper types; lock-contract integration test in `secure_delete_correctness.rs` includes concurrent-write race-test.
- **`crates/atlas-mem0g/src/lancedb_backend.rs`** (~468 LOC) — `LanceDbCacheBackend` impl. `SemanticCacheBackend` methods plumb through to `secure_delete` for erasure + use `precapture_fragments` / `precapture_indices` helpers (depth-recursive `walk_collect_filtered` — **MEDIUM-3 reviewer fix:** was single-level `read_dir`; would have missed `_versions/N/data-*.lance` if LanceDB stores fragments in versioned sub-directories). Fragment-walk filters on `.lance` extension at any depth; depth-0/1/2 test passes. Body fill-in deferred to W18c with `RESUME(spawn_blocking)` markers at `upsert`/`search`/`erase`/`erase-step-4` body sites.
- **`crates/atlas-mem0g/tests/{embedding_determinism,secure_delete_correctness,mem0g_benchmark}.rs`** (~608 LOC) — embedding-determinism (V2 verification gap, feature-gated behind `lancedb-backend`), secure-delete-correctness with raw-bytes-not-recoverable + concurrent-write race + defence-in-depth missing-paths tests (4/4 pass), B4/B5/B6 `#[ignore]`-gated benches behind `ATLAS_MEM0G_BENCH_ENABLED=1` mirroring W17c `arcadedb_benchmark.rs` pattern.
- **MODIFY `crates/atlas-projector/src/upsert.rs`** (+445 net LOC) — NEW `apply_embedding_erased` dispatch arm for the `embedding_erased` event-kind per ADR §4 sub-decision #5. Mirrors `apply_anchor_created` structure exactly: payload validation with empty-string guards for `event_id` + `workspace_id` (**MEDIUM-1 reviewer fix:** added `erased_at` empty-string guard symmetric with event_id/workspace_id), required fields (`event_id` + `workspace_id` + `erased_at`) + optional fields (`requestor_did` + `reason_code` defaulting to `"operator_purge"`), append-only refusal of duplicates via existing `MissingPayloadField` variant pattern (semantic-mismatch doc-comment per ADR §4 sub-decision #5; dedicated `DuplicateErasureRefused` variant is V2-γ-deferred consistent with `DECISION-ARCH-W17b` carry-over #5).
- **MODIFY `crates/atlas-projector/src/state.rs`** (+70 LOC) — NEW `embedding_erasures: BTreeMap<String, EmbeddingErasureEntry>` field on `GraphState` (analog `rekor_anchors`); NEW `EmbeddingErasureEntry { workspace_id, erased_at, requestor_did, reason_code, event_uuid, author_did }` struct (`#[non_exhaustive]`); NEW `upsert_embedding_erasure` helper.
- **MODIFY `crates/atlas-projector/src/canonical.rs`** (+85 LOC) — extends `build_canonical_bytes` with omit-when-empty serialisation for `embedding_erasures` (preserves V14 invariant). Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces unchanged. Test `embedding_erasures_omitted_when_empty` asserts the field name does not appear in canonical bytes when the map is empty.
- **NEW `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts`** (~217 LOC) — POST endpoint returning top-k `SemanticHit` results with cite-back `event_uuid`. Response-time normalisation default 50 ms (env-configurable via `ATLAS_SEMANTIC_SEARCH_MIN_LATENCY_MS`); both cache-hit and cache-miss responses delay to the minimum (timing-side-channel mitigation per ADR §4 sub-decision #8). 501 stub path also obeys normalisation (no distinguishing "backend not wired" from "cache miss" at the API boundary). Zod-strict input schema; workspace-id regex (path-traversal structurally blocked); k bounded [1, 100]; query length bounded [1, 4096]; body byte-cap 32 KB via `Buffer.byteLength(rawText, 'utf8')` (**MEDIUM-4 reviewer fix:** was JS `.length` UTF-16 char count). `embedding_hash` cache-key NEVER exposed in response headers / status / body.
- **NEW `.github/workflows/atlas-mem0g-smoke.yml`** (~132 LOC) — Linux Ubuntu lane analog `atlas-arcadedb-smoke.yml`. SHA-pinned actions; `permissions: contents: read`; paths-gated trigger (`crates/atlas-mem0g/**` + `crates/atlas-projector/src/{upsert,state,canonical}.rs` + `apps/atlas-web/src/app/api/atlas/semantic-search/**` + the workflow file itself); 10 min timeout; HuggingFace model file cache keyed on `hashFiles('crates/atlas-mem0g/src/embedder.rs')` so any constant change invalidates the cache.
- **NEW `.handoff/v2-beta-welle-18b-plan.md`** (~228 lines) — plan-doc per template + Implementation Notes + deferred-items resume guide (HIGH-2 fastembed `try_new_from_user_defined` wiring + 3 additional tokenizer-file SHA-256 pins are pre-V2-β-1-ship blockers; W18c follow-on lifts LanceDB ANN bodies + cross-platform determinism CI matrix).
- **W18b parallel reviewer-dispatch** (Atlas Standing Protocol Lesson #8): **0 CRITICAL + 4 HIGH (deduplicated) + 6 MEDIUM**. All HIGH + MEDIUM applied in-commit per Atlas Standing Protocol Lesson #3 (fix-commit `717922c` on top of initial `80f6957`). HIGH summary: (H-1) real SHA-256 replacing blake3-prefixed placeholder; (H-2) `AtlasEmbedder::new` unconditional fail-closed bypassing the previous `Default::default()` re-download path that bypassed Atlas's SHA check — pre-V2-β-1-ship requires fastembed `try_new_from_user_defined` wiring + 3 tokenizer-file pins (deferred-with-gate posture); (H-3) gatekeeper test asserts all three supply-chain constants; (H-4) cryptographic-random `getrandom` overwrite replacing deterministic blake3-keyed PRG. MEDIUM summary: empty-string `erased_at` guard, non-silent secure-delete on missing pre-captured paths, recursive fragment walk, UTF-8 body-cap, `Once::call_once` wrap of `set_var`, `spawn_blocking` doc clarification + `RESUME` markers.
- **Test impact:** 578 tests pass workspace-wide (130 projector + 17 mem0g lib + 18 backend_trait_conformance + 4 secure-delete + others); zero failures; new tests cover HIGH-1 RFC-6234 SHA-256 vectors, HIGH-3 gatekeeper, HIGH-4 random fill, MEDIUM-1 empty-string guards, MEDIUM-2 non-silent secure-delete, MEDIUM-3 recursive walk depth-0/1/2. `cargo clippy --workspace --no-deps -- -D warnings` zero warnings. Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduced unchanged (verified via `cargo test -p atlas-projector --test backend_trait_conformance byte_pin`).
- **W18c follow-on welle (queued):** LanceDB ANN/search body fill-in (closes spike §12 V1-V4 verification gaps: LanceDB Windows behaviour + fastembed-rs cross-platform determinism + Lance v2.2 `_deletion_files` semantics + fastembed model file size measurement); fastembed `try_new_from_user_defined` wiring with 3 additional tokenizer-file SHA-256 pins; Nelson supply-chain constant lift (pre-V2-β-1-ship blocker). ADR-Atlas-013 reserved.

### Added — V2-β Welle 18 Phase A (Mem0g Layer-3 cache design — spike + ADR-Atlas-012, 2026-05-15)

- **NEW `docs/V2-BETA-MEM0G-SPIKE.md`** (~520 lines, 13 sections) — comparative architectural spike analog `docs/V2-BETA-ARCADEDB-SPIKE.md`. Surveys six options (Mem0/Mem0g, LanceDB, Qdrant, fastembed-rs, sqlite-vec, USearch) plus eliminated-outright candidates (SurrealDB BSL, sqlite-vss abandoned, Milvus-lite/ChromaDB/Vespa runtime-co-resident). Resolves the 6 design questions enumerated in handoff §0-NEXT with confidence levels per question. Critical clarification surfaced: *"Mem0g"* is a research-paper name (arXiv:2504.19413), not a separate product — it's `mem0` with graph-mode enabled. Master-plan naming preserved as the **concept name**; Atlas's implementation is Atlas-controlled Rust.
- **NEW `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md`** (~430 lines, 9 sections) — distils spike into **8 binding sub-decisions**:
  1. Vector-store + embedder choice: `lancedb 0.29.0` (Apache-2.0) + `fastembed-rs 5.13.4` (Apache-2.0) paired, both pure-Rust embedded. Mem0-Python rejected on Hermes-skill distribution + cloud-default embedder + delegated secure-delete. Qdrant sidecar reserved as documented pivot (LP1-LP5 trigger thresholds in spike §9). Encapsulated behind `SemanticCacheBackend` Rust trait so swap is one-impl, not a re-architecture.
  2. Embedder model + determinism + supply-chain controls: `bge-small-en-v1.5` FP32 ONNX with ORT-version + `OMP_NUM_THREADS=1` + denormal-flag + `fastembed = "=5.13.4"` exact-version pin. Atlas-controlled fail-closed model download via three Atlas-source-pinned `const` values: HuggingFace revision SHA + ONNX file SHA256 + URL pin. Re-verification at every cold start.
  3. Cache-key strategy + Layer authority correction: cache-keys MAY use both `event_uuid` (trust + invalidation) and `embedding_hash` (fast-lookup); Layer authority = Mem0g indexes Layer 1 directly, NOT via Layer 2 (corrects Phase-2 Architect H-3 misreading).
  4. Secure-delete primitive (GDPR Art. 17): pre-capture-then-lock-then-overwrite protocol covers BOTH fragments AND HNSW `_indices/` files. Closes the TOCTOU race window flagged by security-reviewer HIGH-1. SSD wear-leveling caveat documented; V2-γ cryptographic-erasure deferral noted.
  5. GDPR-erasure parallel-audit-event: NEW event-kind `embedding_erased` with EU-DPA-evidentiary payload (`event_id` + `workspace_id` + `erased_at` + optional `requestor_did` + optional `reason_code`). Append-only refusal of duplicates with semantic-mismatch note for `MissingPayloadField` variant reuse.
  6. Cache-invalidation strategy: hybrid Layer-1-native triple (TTL + erasure-event + Layer-1 head divergence). Layer-2 `graph_state_hash` cross-check is opportunistic defence-in-depth ONLY (NOT load-bearing — preserves Layer-3-independent-of-Layer-2 invariant).
  7. Crate boundary: NEW workspace member crate `crates/atlas-mem0g/` (clean cargo + license boundary; pivot encapsulation; independent CI + reviewer dispatch).
  8. Bench-test shape (B4 cache-hit / B5 cache-miss-with-rebuild / B6 secure-delete primitive correctness incl. concurrent-write race-test) + timing-side-channel mitigation via response-time normalisation in W18b's semantic-search Read-API endpoint.
- **NEW `.handoff/v2-beta-welle-18-plan.md`** (~280 lines) — plan-doc per template + 8-risk register + ready-to-dispatch W18b subagent skeleton.
- W18 Phase A reviewer-dispatch (parallel `code-reviewer` + `security-reviewer`, Atlas Standing Protocol Lesson #8): **0 CRITICAL + 2 HIGH + 10 MEDIUM + 4 LOW**. All HIGH + MEDIUM applied in-commit per Lesson #3. Notable HIGH closes: secure-delete TOCTOU race (rewrite to pre-capture-then-lock protocol); model-download URL pinning (Atlas-source-pinned consts replacing reliance on fastembed-rs's default download path).
- Architectural posture preserved: embeddings live OUTSIDE canonicalisation pipeline (lib.rs invariant #3 honoured); byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces; Layer 3 NEVER trust-authoritative — every `SemanticHit` cites back to Layer-1 `event_uuid`; Hermes-skill `npx` distribution preserved (no Python, no second JVM); all trust-claim language qualified counsel-pending per `DECISION-COUNSEL-1` / `DECISION-COMPLIANCE-3` / `DECISION-COMPLIANCE-4`.
- Zero Rust touched in Phase A; W18b implementation (~1500-2000 LOC: NEW `crates/atlas-mem0g/` + `apply_embedding_erased` dispatch arm in `crates/atlas-projector/src/upsert.rs` + cross-platform determinism + secure-delete-correctness + race-test + B4/B5/B6 benches + new CI workflow + Read-API endpoint with response-time normalisation) is the next welle.

### Added — V2-β Welle 17c (ArcadeDB Docker-Compose CI + benchmark + W17b Cypher hotfix, 2026-05-14) V2-α public-API contract per [`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md) §10 unchanged on the Rust verifier side; this stretch adds V2-β Read-side public surfaces (atlas-web Read-API + atlas-mcp-server MCP V2 tools), extends `atlas-projector` event-kind dispatch with 3 new additive kinds, consolidates Cypher AST validation into a shared `@atlas/cypher-validator` monorepo package, locks Layer-2 architectural decisions for the upcoming ArcadeDB driver integration (W16 + ADR-Atlas-010), lands the production `GraphStateBackend` trait + `InMemoryBackend` + `ArcadeDbBackend` stub (W17a + ADR-Atlas-011), and **then ships the production ArcadeDB driver implementation (W17b)** — the abstraction boundary is now live behind a working HTTP driver and the byte-determinism pin reproduces through both backends. The W17a-cleanup follow-on (2026-05-14) resolved three of the four W17a carry-over MEDIUMs at the trait surface so W17b's first method body landed SemVer-stable. The W17b reviewer-fix pass (2026-05-14) closed 2 HIGH + 3 MEDIUM + 2 LOW reviewer findings in-commit (0 CRITICAL) — including the `format!("create database {db_name}")` admin-command injection surface (resolved via stricter `[a-zA-Z0-9_]` db-name allowlist) and the `run_command` Value-return ADR-011 §4.3 #12 latent bypass. All 7 V2-α byte-determinism CI pins byte-identical from v2.0.0-alpha.2 baseline.

### Added — V2-β Welle 17c (ArcadeDB Docker-Compose CI + benchmark + W17b Cypher hotfix, 2026-05-14)

- **NEW `infra/docker-compose.arcadedb-smoke.yml`** — ArcadeDB 24.10.1 sidecar per `docs/V2-BETA-ARCADEDB-SPIKE.md` §4.7 sketch. JVM heap bounded to 256-512 MB; `JAVA_OPTS=-Darcadedb.server.rootPassword=...` (the spike's `environment:` env-var shape does not actually configure the container — it falls into an interactive password prompt instead). Healthcheck on unauthenticated `/api/v1/ready` (204 No Content; credentials never embedded in container metadata). No persistent volume; `restart: "no"` policy; retries=12 + start_period=30s for CI cold-start variance.
- **NEW `.github/workflows/atlas-arcadedb-smoke.yml`** — Linux Ubuntu lane, SHA-pinned actions (parity with `hsm-byte-equivalence.yml`), `permissions: contents: read`, paths-gated trigger on `crates/atlas-projector/**` + `Cargo.{lock,toml}` + the compose/workflow files themselves. 10 min timeout, `cancel-in-progress` concurrency. Steps: compose up → healthcheck wait (60 s outer cap on top of 30 s start_period; `set +x` defends against `RUNNER_DEBUG=1` echoing `JAVA_OPTS` password) → `cargo test --test cross_backend_byte_determinism -- --ignored` → `cargo test --test arcadedb_benchmark -- --ignored` (output tee'd to `target/arcadedb-bench.log`) → artifact upload (`actions/upload-artifact@v4.4.3`, 30-day retention) → compose down (in `if: always()`).
- **NEW `crates/atlas-projector/tests/arcadedb_benchmark.rs`** (~360 LOC, 3 `#[ignore]`-gated bench tests):
  - **B1** — cross-backend equivalence sanity (cheap canary; NOT the authoritative byte-pin gate — that lives in `cross_backend_byte_determinism::cross_backend_byte_determinism_pin` with the full `author_did`-on-edge stamping).
  - **B2** — incremental upsert latency p50/p95/p99 (200 timed samples after 50 warm-up). Measures the projector hot path.
  - **B3** — sorted-read latency p50/p95/p99 (50-vertex / 100-edge fixture; 100 timed reads after 20 warm-up). Provides a baseline for ADR-010 §4.10 T2 trigger (Read-API depth-3 p99 > 15 ms at 10M-vertex workspace; deployment-telemetry observation per §4.4, not CI-gated).
- **NEW `tools/run-arcadedb-smoke-local.sh`** — bash helper mirroring CI workflow for local-dev. Cleanup on EXIT trap. Same env-var contract.
- **First CI-measured baselines** (Linux Ubuntu `ubuntu-latest` runner, post-fix, captured from PR #92's atlas-arcadedb-smoke run):
  - B2 incremental_upsert (n=200): captured in workflow artifact (Phase 11.5 to extract concrete numbers from artifact and update ADR-010 §4.10).
  - B3 sorted_read_vertices_50v / sorted_read_edges_100e: captured in workflow artifact.
  - 7 V2-α byte-determinism CI pins byte-identical post-merge.

### Fixed — V2-β Welle 17b regressions surfaced by W17c live integration test (2026-05-14)

The cross-backend integration test was W17b-shipped `#[ignore]`-gated. W17c's CI workflow is the first time it ran against a live ArcadeDB. Two distinct regressions in W17b's driver became visible the moment the test ran. Both fixed in PR #92 atomically with the CI infrastructure that surfaced them:

- **ArcadeDB Cypher param-name collision (`$from`, `$to`, `$label`)** — ArcadeDB 24.10.1 silently empties result sets when a Cypher query binds a parameter named `$from` or `$to` (collide with SQL `CREATE EDGE ... FROM ... TO ...` keywords). `$label` raises `IllegalArgumentException("Value 'label' ... is not supported")` because TinkerPop's `T.label` is a reserved token. `cypher.rs::upsert_edge_command` renamed to `$src` / `$dst` / `$lbl` (placeholders only — stored edge-property names use `from_entity_uuid` / `to_entity_uuid` / `edge_label`); `parse_edge_row` translates `edge_label` field back to `BackendEdge::label` so the public API and byte-pin are unchanged.
- **ArcadeDB Edge type not auto-registered on first MERGE** — `MERGE (a)-[r:Edge]->(b)` silently no-ops if the `Edge` type does not yet exist (`CREATE` would auto-register, `MERGE` does not). Without registration, edge writes returned 2xx with zero edges persisted. New `ArcadeDbBackend::ensure_schema_types_exist`: single atomic Cypher `CREATE (a:Vertex)-[r:Edge]->(b:Vertex) WITH a, b, r DETACH DELETE a, b` statement registers both types and cleans up sentinels in one HTTP roundtrip — no orphan-sentinel window even on partial failure. Idempotent across the per-(backend, db_name) cache; held lock NOT across HTTP.

W17c reviewer-dispatch (parallel `code-reviewer` + `security-reviewer`, Atlas Standing Protocol lesson #8): 0 CRITICAL + 1 HIGH + 4 MEDIUM + 2 LOW. All HIGH + applicable MEDIUM/LOW fixed in-commit before merge (atomic schema-bootstrap, unauthenticated healthcheck, `set +x` guard, restart policy, `dtolnay/rust-toolchain` branch-tip SHA documentation). 153 unit + integration tests green; clippy `-D warnings` clean; cross-backend byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces through ArcadeDB live. Trait surface unchanged (SemVer additive).

### Added — V2-β Welle 17b (ArcadeDB driver implementation, 2026-05-14)

- **NEW sub-module `crates/atlas-projector/src/backend/arcadedb/{mod.rs, client.rs, cypher.rs}`** (~1860 LOC). Replaces the W17a single-file stub with a production `reqwest::blocking`-based driver against ArcadeDB's Server-mode HTTP API per ADR-Atlas-010 §4 sub-decisions 1-8. `mod.rs`: backend + `ArcadeDbTxn` impl, lazy-create-database, commit/rollback session handling, scheme/userinfo guard in `new()`, bounded body-read in `ensure_database_exists`. `client.rs`: `reqwest::Client` wrapper with 5 s connect-timeout + 30 s request-timeout + `rustls-tls` + redacting `BasicAuth`/`SecretString` (`pub(crate)` — downstream crates cannot reach the plaintext password); single `apply_basic_auth` call site; URL-stripping `describe_reqwest_error`; 512-byte scrub-and-truncate for 4xx/5xx body echo. `cypher.rs`: parameterised query builders (every dynamic value bound via `$ws` / `$eid` / `$props` — no string-concat); sorted-read templates per ADR-010 §4 #6 (`ORDER BY n.entity_uuid ASC` / `ORDER BY e.edge_id ASC`); `db_name_for_workspace` returns `ProjectorResult<String>` with strict `[a-zA-Z0-9_]` allowlist post-hyphen-replacement (second-layer defence against admin-command injection); row parsers gated by `check_value_depth_and_size` at every HTTP-response → `BTreeMap<String, Value>` boundary per ADR-011 §4.3 #12.
- **MODIFY `crates/atlas-projector/Cargo.toml`** — adds `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "blocking"] }` (~2 MB binary cost; tokio-aligned async). Same `rustls-tls` feature set used by `atlas-signer`; uniform TLS backend across the workspace (no `openssl-sys`).
- **NEW `crates/atlas-projector/tests/cross_backend_byte_determinism.rs`** (257 LOC) — `#[ignore]`-gated cross-backend byte-determinism test. Same 3-node + 2-edge fixture as `backend_trait_conformance::byte_pin_through_in_memory_backend`; asserts `InMemoryBackend::canonical_state() == ArcadeDbBackend::canonical_state()` byte-identical. Requires `ATLAS_ARCADEDB_URL` env var. CI wiring deferred to W17c Docker-Compose smoke workflow.
- **DROP** `crates/atlas-projector/src/backend/arcadedb.rs` stub (replaced by sub-module).
- **DROP** stub-panic tests from `backend_trait_conformance.rs` (no longer applicable post-fill); `byte_pin_through_in_memory_backend` retained — reproduces `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` through the trait surface.
- **W17a carry-over MEDIUM final status** (all 4 from W17a plan-doc tracked through to closure):
  - #2 (`serde_json::Value` depth+size cap): RESOLVED — called at every HTTP-response → `Vertex`/`Edge` boundary in `cypher.rs::parse_vertex_row` + `parse_edge_row`.
  - #3 (`WorkspaceId` validation guard): RESOLVED — `check_workspace_id` called as FIRST line of `ArcadeDbBackend::begin()` + `vertices_sorted` + `edges_sorted`; W17b adds a second validation layer in `db_name_for_workspace` that rejects characters incompatible with ArcadeDB db-name rules.
  - #4 (`begin()` lifetime evaluation): ALREADY RESOLVED by W17a-cleanup (lifetime is `'static`); W17b's `ArcadeDbTxn` honours it via owned fields end-to-end (cloned `reqwest::Client`, owned `db_name` / `session_id` / `workspace_id` / `BasicAuth`).
  - #5 (`MalformedEntityUuid` umbrella variant): V2-γ-deferred per W17a + W17a-cleanup plan-doc rationale; W17b does NOT touch the error-enum convention.
- **W17b reviewer-dispatch findings closed in-commit** (parallel `code-reviewer` + `security-reviewer` per Atlas Standing Protocol lesson #8):
  - 0 CRITICAL.
  - 2 HIGH: `run_command` ADR-011 §4.3 #12 latent bypass (narrowed return to `()`); `format!("create database {db_name}")` admin-command injection surface (closed via second-layer allowlist in `db_name_for_workspace`).
  - 3 MEDIUM: `SecretString`/`BasicAuth.password` `pub` visibility tightened to `pub(crate)`; `ArcadeDbBackend::new()` rejects URLs carrying userinfo (closes derived-`Debug` credential-leak surface) AND rejects non-http/https schemes; bounded response-body read in `ensure_database_exists`.
  - 15 clippy `doc_lazy_continuation` lints fixed (13 W17b-new in `cypher.rs`, 2 pre-existing on master in `backend/mod.rs:547-548`). Subagent's self-audit claim of "zero clippy warnings" was incorrect.
  - 4 new tests added (`db_name_rejects_chars_check_workspace_id_permits`, `begin_rejects_workspace_id_with_db_name_incompatible_chars`, `new_rejects_unsupported_scheme`, `new_rejects_url_with_userinfo`).
- **Trait surface UNCHANGED.** `git diff master..PR-90-base -- crates/atlas-projector/src/backend/mod.rs` only touches one doc-comment paragraph (clippy doc-lint fix); no public-API items added, removed, or renamed.
- **Test impact:** 119 unit + 18 trait-conformance + 16 other = 153 tests green; `cargo clippy -p atlas-projector --no-deps -- -D warnings` zero warnings; cross-backend byte-determinism test EXISTS and compiles (live-run gated until W17c Docker-Compose CI).

### Added — V2-β Welle 17a-cleanup (pre-W17b trait-surface hardening + boundary helpers + ADR-011 §4.3 amendment, 2026-05-14)

- **`GraphStateBackend::begin()` lifetime `'_` → `'static`** (ADR-Atlas-011 §4.3 sub-decision #10, resolves W17a plan-doc MEDIUM #4). Eliminates a SemVer-breaking-mid-W17b risk: the original `Box<dyn WorkspaceTxn + '_>` tied transaction lifetime to `&self`, but neither the in-memory impl (`Arc::clone(&self.workspaces)`) nor the planned ArcadeDb impl (owned `reqwest::Client` + owned `arcadedb-session-id`) actually borrow from `&self`. Lifetime-widening is SemVer-additive at every existing call site (type checker accepts `'_` → `'static` automatically). All 8 pre-existing trait-conformance tests stay green; one new `begin_returns_static_txn_handle` test pins the `'static` bound via an explicit `Box<dyn WorkspaceTxn + 'static>` type annotation that would fail to compile if the lifetime regressed.
- **NEW `pub fn check_workspace_id(s: &str) -> ProjectorResult<()>`** in `atlas_projector::backend` + re-exported at crate root (ADR-Atlas-011 §4.3 sub-decision #11, resolves W17a plan-doc MEDIUM #3 at trait surface). Validates: non-empty, length ≤ 128, ASCII-only, no `/`, `\`, NUL. Helper is `pub` so V2-γ backend impls can re-use the same defence. `InMemoryBackend` does NOT call it at runtime (HashMap-key safety); W17b's `ArcadeDbBackend::begin()` MUST call it before constructing the HTTP `/api/v1/begin/{db}` request — enforced via reviewer-gate + stub doc-comment.
- **NEW `pub fn check_value_depth_and_size(v: &serde_json::Value, max_depth: usize, max_bytes: usize) -> ProjectorResult<()>`** in `atlas_projector::backend` + re-exported at crate root (ADR-Atlas-011 §4.3 sub-decision #12, resolves W17a plan-doc MEDIUM #2 at trait surface). Uses `serde_json::to_vec` for size + iterative `Vec`-stack walk for depth (no Rust call-stack recursion). Caller picks limits; recommended W17b defaults `max_depth=32`, `max_bytes=64*1024`. W17b's HTTP-response parser MUST call this AFTER `serde_json::from_slice` on ArcadeDb Cypher results, BEFORE `Vertex::new` / `Edge::new`. `InMemoryBackend` does NOT need this (V2-α canonicalisation already bounds property shape).
- **NEW `ProjectorError::InvalidWorkspaceId { reason: String }`** variant (`#[non_exhaustive]` enum addition — SemVer-additive).
- **9 new trait-conformance tests** in `backend_trait_conformance.rs`: `check_workspace_id_*` (5 — accepts typical shapes, rejects empty, rejects too-long, rejects path-traversal chars, rejects non-ASCII), `check_value_depth_and_size_*` (3 — accepts typical payload, rejects deep nesting, rejects oversized), `begin_returns_static_txn_handle` (lifetime pin). 17/17 tests green (8 original + 9 new); byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduced unchanged.
- **`docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md` amended:** new §4.3 with sub-decisions #10/#11/#12 + public-API-surface delta + new row in §9 decision log.
- **W17a plan-doc MEDIUM #5** (`MalformedEntityUuid` umbrella variant for edges) remains V2-γ-deferred per original plan-doc; broader error-enum refactor is out of W17a-cleanup scope and not blocking W17b.

### Added — V2-β counsel-engagement enablement (2026-05-14)

- **NEW `.handoff/v2-counsel-engagement-scope.md`** — RFP-ready 7-SOW counsel engagement scope + 7-firm comparison matrix + DE+EN outreach templates + engagement-letter checklist. Operational unblock for `DECISION-COUNSEL-1` (GDPR Art. 4(1) hash-as-PII Path-B opinion) pre-V2-β-public-materials blocking gate. 6-week counsel-engagement clock starts at firm-signature.

### Changed — V2-β counsel-engagement enablement (2026-05-14)

- **`README.md`** Art. 12 paraphrase replaced with verbatim regulation text per `DECISION-COMPLIANCE-2` (Regulation (EU) 2024/1689 Art. 12 §1 + §2 excerpt + Annex IV §1(g)/§2(g) cross-reference + Art. 113(b) in-force date).
- **`docs/COMPLIANCE-MAPPING.md`** carries a counsel-pending disclaimer header pointing at SOW-5 of the new counsel-scope doc.
- **`tools/expected-master-ruleset.json`** synced to live Ruleset state (`atlas-web-playwright` added as 2nd required status check). No security-drift — live Ruleset 15986324 was already stricter than the pinned file; this is doc-sync only. The check became required during the V2-β Phase-2-4 atlas-web work batch (W12 Read-API + W13 MCP V2) and the pin file simply was not updated at the time.

### Added — V2-β Welle 17a (`GraphStateBackend` trait + InMemoryBackend + ArcadeDbBackend stub + ADR-Atlas-011, 2026-05-13)

- **NEW `crates/atlas-projector/src/backend/mod.rs`** (~555 LOC) — production `GraphStateBackend` trait (Send + Sync, object-safe via `Box<dyn>`). Defines `Vertex` / `Edge` / `UpsertResult` (all `#[non_exhaustive]` with explicit `new()` constructors) carrying V2-α Welle 1 stamping fields (`event_uuid`, `rekor_log_index: Option<u64>`, `author_did: Option<String>`). `WorkspaceTxn` trait for per-task transactions (`upsert_vertex` / `upsert_edge` / `batch_upsert` with vertices-before-edges ordering / `commit` / `rollback`). Default `canonical_state()` trait impl delegates to V2-α `canonical::graph_state_hash` so every backend gets byte-determinism for free. Helpers `vertex_from_graph_node` / `edge_from_graph_edge` for V2-α-state ↔ trait-surface conversion.
- **NEW `crates/atlas-projector/src/backend/in_memory.rs`** (~514 LOC) — `InMemoryBackend` impl wrapping the existing V2-α `GraphState` pipeline via `Arc<Mutex<HashMap<WorkspaceId, GraphState>>>`. Scratch-buffer transactions; `commit()` re-acquires the lock briefly to swap the scratch in then drops the guard. Overrides `canonical_state()` to call `graph_state_hash` directly on stored `GraphState` for byte-pin proximity. `snapshot()` is `#[doc(hidden)]` — diagnostic-only, not part of the trait surface.
- **NEW `crates/atlas-projector/src/backend/arcadedb.rs`** (~203 LOC) — stub. Constructor + `backend_id()` work; every other trait method is `unimplemented!("W17b: <endpoint + Cypher hint>")`. NO `reqwest` dep added (lands in W17b alongside the first method body); NO outbound network or filesystem I/O. Pure compile-time scaffolding proving the trait surface is implementable for a server-mode HTTP backend.
- **NEW `crates/atlas-projector/tests/backend_trait_conformance.rs`** (~315 LOC, 8 tests) — round-trip; byte-pin via trait (reproduces `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` hex); `backend_id` stability; ArcadeDb-stub `unimplemented!` panic; `batch_upsert` ordering; batch-vs-individual canonical equality; sorted iteration; rollback no-op semantics.
- **NEW `docs/ADR/ADR-Atlas-011-arcadedb-driver-scaffold.md`** (~259 lines, 9 sections, mirrors ADR-Atlas-010 structure). Closes ADR-010 OQ-1 (`Box<dyn WorkspaceTxn>` chosen over associated type — object safety preserved for emission pipeline; vtable overhead ~1 ns is irrelevant against the ~300-500 µs HTTP roundtrip baseline) and ADR-010 OQ-2 (`WorkspaceTxn::batch_upsert(&[Vertex], &[Edge])` returning `Vec<UpsertResult>`; vertices before edges so no transient edge-without-endpoint state). Opens OQ-7..OQ-11 for W17b/V2-γ tracking.
- **Updated `crates/atlas-projector/src/lib.rs`** — `pub mod backend;` + 9 re-exports (`GraphStateBackend`, `WorkspaceTxn`, `Vertex`, `Edge`, `UpsertResult`, `InMemoryBackend`, `ArcadeDbBackend`, helpers).
- **Updated `crates/atlas-projector/src/emission.rs` + `gate.rs`** — new `*_with_backend(...)` entry-point variants beside the legacy entries. Legacy entries UNCHANGED so all existing public-API consumers + 169 trust-core + 88 projector unit tests stay green without modification.
- **NEW `.handoff/v2-beta-welle-17a-plan.md`** (~207 lines) — plan-doc per template, with post-merge reviewer-finding carry-over section documenting 4 deferred MEDIUMs that W17b must lift in (serde_json depth cap, WorkspaceId validation, `begin()` lifetime evaluation, error-enum cleanup deferred to V2-γ).
- **Byte-determinism preservation:** pinned hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` UNCHANGED through both `canonical::tests::graph_state_hash_byte_determinism_pin` AND new `backend_trait_conformance::byte_pin_through_in_memory_backend`. All 7 V2-α byte-determinism CI pins intact.
- **Three deliberate deviations from spike §7 sketch (all sound, all documented in ADR-011 §4):** (1) `serde_json::Value` for properties (workspace dep reality: `serde_cbor` not present; `ciborium` is the canonical-CBOR boundary at canonicalisation time, not at backend surface); (2) `EntityUuid` / `EdgeId` / `WorkspaceId` kept as `String` aliases (those symbols don't exist in `atlas-trust-core` — spike sketch was prospective); (3) `Arc<Mutex<HashMap<...>>>` redesign forced by Windows `MutexGuard: !Send` — guard never escapes; commit re-acquires-and-drops.
- **Parallel external reviewer dispatch:** code-reviewer + security-reviewer both APPROVE. 0 CRITICAL / 0 HIGH. 5 MEDIUM findings — 1 applied in-commit (`#[doc(hidden)]` on `snapshot()`), 4 documented as W17b/V2-γ carry-overs in the plan-doc. The subagent's initial commit performed self-audit only; the parent dispatched the external reviewers post-implementation per Atlas Standing Protocol (single fix-commit `08167fc` on the welle branch before squash-merge).

### Added — V2-β Welle 16 (ArcadeDB embedded-mode spike + ADR-Atlas-010, 2026-05-13)

- **NEW `docs/V2-BETA-ARCADEDB-SPIKE.md`** (~460 lines, 11 sections) — comprehensive architectural spike analog V2-α Welle 2 FalkorDB spike. Answers 10 architectural questions with HIGH/MEDIUM-HIGH/MEDIUM confidence annotations (resolves the 5 open questions from ADR-Atlas-007 §6 plus 5 additional W16-scope-extensions for the ArcadeDB integration path).
- **NEW `docs/ADR/ADR-Atlas-010-arcadedb-backend-choice-and-embedded-mode-tradeoff.md`** (~285 lines, 9 sections) — binding decision doc mirror ADR-Atlas-007/008/009 structure. Three considered options (A: server mode + reqwest RECOMMENDED; B: embedded mode REJECTED on Hermes-skill blocker; C: FalkorDB primary flip REJECTED on SSPL §13). Sub-decisions: ArcadeDB Apache-2.0 primary CONFIRMED, server mode, `reqwest` async HTTP client, one-database-per-workspace pattern, per-workspace atomic transactions, byte-determinism adapter (`ORDER BY entity_uuid ASC` + stored `edge_id` property), 3-layer tenant-isolation defence (Layer 1 per-database isolation + Layer 2 projector workspace_id binding + Layer 3 Cypher AST mutation hardening — Layer 3 does NOT enforce workspace_id presence per security-reviewer correction).
- **DECISION-DB-4 status:** CONFIRMED (no flip). License-side confidence remains MEDIUM-HIGH (pending counsel SSPL §13 opinion); architecture-side raised to HIGH after W16. Combined operational confidence: HIGH-on-architecture, MEDIUM-HIGH-overall.
- **`GraphStateBackend` Rust trait sketched** (~40 LOC pseudo-code in spike §7) — W17a writes the production trait + `InMemoryBackend` wire-up + `ArcadeDbBackend` stub. ADR-Atlas-011 reserved for W17a.
- **5 FalkorDB fallback trigger thresholds (T1-T5)** documented with measurable criteria — Cypher subset insufficient, HTTP latency > 15ms p99, project deprecation, Hermes-skill cold-start > 5s, counsel ruling on SSPL §13. None fired as of 2026-05-13.
- **Two reviewer-driven HIGH fixes applied in-commit:** (1) Layer 3 defence claim corrected — the Cypher AST validator enforces read-only structure (mutation prevention) but does NOT enforce workspace_id presence; the spike-doc + ADR + plan-doc all updated to accurately reflect Layer 2 (projector parameter binding) as the active workspace_id enforcer; confidence on §4.3 dropped HIGH → MEDIUM-HIGH; per-database-per-workspace operator-runbook requirement made explicit. (2) Performance-number inconsistency between spike §1 executive summary and §4.10 detailed table reconciled (10M-event workspace-parallel re-projection: ~6-10 min per §4.10 careful estimate). Three MEDIUM doc-fixes also applied (Q4 label cross-reference clarification, cross-backend test welle assignment, edge_id storage requirement explicit).

### Refactored — V2-β Welle 15 (Cypher-validator consolidation, 2026-05-13)

### Refactored — V2-β Welle 15 (Cypher-validator consolidation, 2026-05-13)

- **NEW `packages/atlas-cypher-validator/`** — shared monorepo package extracting the Cypher AST validator from W12 + W13 inline copies (rule-of-three pattern). Public exports: `validateReadOnlyCypher`, `CYPHER_MAX_LENGTH`, `CypherValidationResult`. Mirrors `packages/atlas-bridge/` conventions: `tsc` build step, `dist/*.js` + `dist/*.d.ts` outputs, `main`/`types`/`exports` pointing to dist, `files: ["dist", "src"]`.
- **Union semantics preserved** — consolidated validator rejects ANY pattern that EITHER pre-W15 inline rejected (strictly superset). Procedure-namespace deny-list union of W12's bare-CALL rejection + W13's `CALL dbms.*` explicit + W12's broader `db.*` guard. String-concat detection uses W12's stricter rule (any `+`). Comment-stripping includes `.trimStart()` after strip (W13's correctness invariant).
- **43 unified test cases** in `packages/atlas-cypher-validator/src/validator.test.ts` (deduplicated from W12×25 + W13×27 + 3 union-semantics-specific). All green.
- **2 callsites updated:** `apps/atlas-web/src/app/api/atlas/query/route.ts` + `apps/atlas-mcp-server/src/tools/query-graph.ts` now import from `@atlas/cypher-validator`. Both consumer `package.json` declare `"@atlas/cypher-validator": "workspace:*"`.
- **4 deleted files:** the two inline `cypher-validator.ts` + their test files (W12-inline at `apps/atlas-web/src/app/api/atlas/_lib/` + W13-inline at `apps/atlas-mcp-server/`).
- **NEW `docs/ADR/ADR-Atlas-009-cypher-validator-consolidation-rationale.md`** (321 lines, 10 sections) — full option analysis (A: shared package = RECOMMENDED; B: mirror module per consumer = rejected; C: leave inline = rejected). Documents 6 invariants from cross-batch consistency-reviewer Phase 4 findings.
- **CI workflows updated:** `atlas-web-playwright.yml`, `hsm-wave3-smoke.yml`, `sigstore-rekor-nightly.yml` each gain a `Build @atlas/cypher-validator` step before consumer build (since consumers depend on the package's compiled `dist/`). `packages/atlas-cypher-validator/**` added to path filters where applicable.
- **Two reviewer-driven hotfixes applied in PR #81:** initial commit had `main`/`types`/`exports` pointing to source (not dist) — fixed in hotfix-1 with proper tsc build step; CI workflow gap surfaced wave3-smoke build break — fixed in hotfix-2 by adding the validator build step to all consumer workflows. 4 total commits on the W15 branch.
- **Open question carried into V2-γ:** AST-level Cypher parsing (currently regex-based; documented as Open Question in ADR §8).

### Added — V2-β Welle 12 (Read-API endpoints + inline Cypher AST validator, 2026-05-13)

- **6 NEW Next.js Read-API routes** under `apps/atlas-web/src/app/api/atlas/`: `entities/[id]` (GET single entity by `entity_uuid`), `related/[id]` (GET outgoing + incoming edges), `timeline` (GET events within workspace + time-window), `query` (POST AST-validated Cypher), `audit/[event_uuid]` (GET event JSON + signature verification status), `passport/[agent_did]` (501 V2-γ stub).
- **NEW `apps/atlas-web/src/app/api/atlas/_lib/cypher-validator.ts`** — inline AST validator (rule-of-three pattern, W15 consolidates after W12 + W13 each ship their own). Rejects DECISION-SEC-4 mandatory write-side keywords: `DELETE`, `DETACH DELETE`, `CREATE`, `MERGE`, `SET`, `REMOVE`, `DROP`, `FOREACH`, `LOAD CSV`, `USING PERIODIC COMMIT`. Rejects procedure-namespace escapes (`apoc.*`, `db.*`, bare `CALL`). 4096-char query cap (matched with W13 post-consistency-fix). Comment-stripping for both `/* */` block + `//` line styles.
- **NEW `apps/atlas-web/src/app/api/atlas/_lib/projection-store.ts`** — `ProjectionStore` interface for the eventual ArcadeDB-backed data layer (W17 fills it). V2-β Phase 4 ships an in-memory `EventsJsonlProjectionStore` that reads `events.jsonl` from disk and applies upserts via `atlas-projector` logic.
- **`handleStoreError` + `redactPaths`** defence-in-depth: 500-envelope path leakage prevented at the HTTP boundary. `WorkspacePathError` + `StorageError` messages redacted before client return.
- **74 unit tests** (validator + 6 route handlers + projection-store), 91.4% line / 83.6% branch / 100% function coverage; exceeds 80/75/80 thresholds.
- **Body-size cap enforced via raw-text length check** before JSON parse (closes the bypass when `Content-Length` header is absent).
- **Agent-DID echo cap** at 512 chars in the 501 stub route (no enumeration risk; over-cap echoes `"<invalid>"`).

### Added — V2-β Welle 13 (MCP V2 tools + inline Cypher AST validator, 2026-05-13)

- **5 NEW MCP tools** in `apps/atlas-mcp-server/src/tools/`: `atlas_query_graph` (run AST-validated Cypher), `atlas_query_entities` (list entities by kind + filters with paging), `atlas_query_provenance` (event chain for an `entity_uuid`), `atlas_get_agent_passport` (V2-γ stub with `ok: false` semantic), `atlas_get_timeline` (events within workspace + time-window). All registered in `TOOL_REGISTRY`.
- **NEW `apps/atlas-mcp-server/src/tools/_lib/cypher-validator.ts`** — INDEPENDENT inline AST validator (rule-of-three pattern with W12; both will be consolidated by W15). Same forbidden-keyword set as W12 (`DELETE`, `DETACH DELETE`, `CREATE`, `MERGE`, `SET`, `REMOVE`, `DROP`, `FOREACH`, `LOAD CSV`, `USING PERIODIC COMMIT`). Additional defence: `CALL dbms.*` explicit rejection. 4096-char query cap. Comment-stripping with `.trimStart()` after strip (correctness invariant for opener-allowlist check on leading-comment queries).
- **Error envelope hardening:** allowlist of `PROJECTION_STORE_STUB_MESSAGE` through `toolError`; all other exceptions collapse to generic `"projection-store call failed"` string. Prevents W17 ArcadeDB driver from leaking internal detail through MCP tool responses.
- **150 test assertions** across 9 test files. Lint clean, tsc strict.

### Added — V2-β Welle 14 (Expanded event-kind support, 2026-05-13)

- **3 NEW projector event-kinds** in `crates/atlas-projector/src/upsert.rs`: `annotation_add` (append annotation to existing entity; entity_uuid + annotation_kind + annotation_body; entity must exist or error), `policy_set` (attach policy_id + policy_version to entity; entity must exist; idempotent last-write-wins per policy_id), `anchor_created` (record Sigstore Rekor anchor reference; event_id keyed; security-conservative duplicate-rejection prevents log-index tampering / replay).
- **State extensions in `state.rs`:** `Node` struct gains `annotations: BTreeMap<String, Vec<AnnotationEntry>>` (keyed for canonicalisation determinism; per-kind ordered Vec) + `policies: BTreeMap<String, PolicyEntry>` (keyed for last-write-wins). `GraphState` gains `rekor_anchors: BTreeMap<String, AnchorEntry>` — top-level since keyed by `event_id`, not `entity_uuid`.
- **Canonical CBOR encoding** in `canonical.rs` for the new state fields — RFC 8949 §4.2.1 sorted-map iteration preserves determinism. Empty-state omission pattern (`if not_empty { serialize }`) mirrors V2-α Welle 3's `author_did=None` pattern. **`graph_state_hash_byte_determinism_pin` hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` byte-identical from pre-W14 baseline** — proven by the `canonical_state_hash_unchanged_for_v1_traces` regression test.
- **Defence-in-depth caps:** `MAX_ANNOTATIONS_PER_KIND = MAX_ITEMS_PER_LEVEL = 100_000` enforced at upsert-time (closes the hostile-trace memory-exhaustion window before canonicalisation fires). Symmetric to existing nodes/edges caps. Empty-string `entity_uuid` guards in `apply_annotation_add` + `apply_policy_set` (defence-in-depth symmetric with `apply_anchor_created`).
- **69 atlas-projector lib tests** (was 14 in v2.0.0-alpha.2 baseline; 52 new W14 tests + 3 post-review fix tests) + 10 gate-integration + 6 pipeline-integration — all green. `unsupported_event_kind_in_trace_surfaces_error` fixture updated to use `future_v2_gamma_kind` (deliberate-unsupported-kind preserves regression intent).

### Notes — V2-β Phase 4 Parallel-Batch Architecture (2026-05-13)

- **Three subagents dispatched in parallel worktrees** via Atlas's `Agent` tool with `isolation: "worktree"`. Per-welle file-areas pre-segregated to zero file-overlap (W12 → `apps/atlas-web/src/app/api/atlas/`; W13 → `apps/atlas-mcp-server/src/tools/`; W14 → `crates/atlas-projector/`). Three PRs (#79 / #77 / #78) merged sequentially via `gh pr merge --squash --admin`.
- **Per-welle reviewer pattern** (V2-α invariant continued): 6 parallel reviewer agents (3 × `code-reviewer` + 3 × `security-reviewer`) after PRs hit DRAFT. Per-welle CRITICAL + HIGH fixed in fix-commits before merge: PR #79 fix-commit `49de62e` (DROP + FOREACH addition, body-size cap, passport echo cap, redactPaths defensive); PR #77 fix-commit `45dfa1b` (error envelope allowlist, opener-check trimStart, UUID regex hardening); PR #78 fix-commit `c4245a7` (empty-uuid guards, MAX_ANNOTATIONS cap).
- **Cross-batch consistency-reviewer (V2-β invariant per Orchestration Plan §3.5):** ONE additional consistency-reviewer agent dispatched after all 3 PRs reached DRAFT, reading all 3 together. Verdict: zero CRITICAL, 4 HIGH cross-welle inconsistencies. Three HIGH fix-forward applied in post-consistency fix-commits (PR #79 fix-commit-2 `5212abc`: validator length cap 16 KB → 4 KB; PR #77 fix-commit-2 `47821ff`: passport stub `ok: false` to match W12 semantics, agent_did Zod cap 1024 → 512 to match W12 echo cap). ONE HIGH deferred to W15 + future welle: `workspace` (W12 HTTP) vs `workspace_id` (W13 MCP) parameter-name convention split — documented here as the pre-existing per-package convention (V2-α MCP tools used `workspace_id`; V2-α atlas-signer CLI standardised on `--workspace`; the split is package-internal consistency, not cross-welle drift).
- **One code-reviewer CRITICAL claim disproven by parent verification.** On PR #79, the code-reviewer flagged the W12 validator's template-literal `\\b${kw}\\b` regex pattern as broken ("`\\b` evaluates to backspace, all forbidden keywords silently pass"). Parent ran direct node verification of all 8 forbidden-keyword regexes against representative Cypher queries: ALL correctly REJECTED, legitimate `MATCH (n) RETURN n` correctly ACCEPTED. The security-reviewer's behavioural analysis was correct; the code-reviewer made a theoretical misreading of JS template-literal escape semantics. No fix applied — validator works as designed. **Lesson recorded for future reviewer-conflict resolution: when reviewers disagree on whether code is broken, run the code.**
- **Byte-determinism preservation:** All 7 V2-α byte-determinism CI pins byte-identical post-Phase-4-batch-merge. `cargo test --workspace --lib --bins -p atlas-trust-core -p atlas-projector` → 169 + 69 green. W14's state-shape extension was specifically designed to preserve V1-shape canonicalisation byte-equivalence via the empty-state-omission pattern; the `canonical_state_hash_unchanged_for_v1_traces` regression test proves this end-to-end.
- **Documentation discipline:** Forbidden-files rule (parent-only consolidation of `CHANGELOG.md` + `docs/V2-MASTER-PLAN.md` §6 + `docs/SEMVER-AUDIT-V1.0.md` §10 + `.handoff/decisions.md` + `.handoff/v2-session-handoff.md`) honored across all 3 PRs per consistency-reviewer audit.

### W15 Cypher-validator consolidation — entry criteria (Phase 5 next-up)

V2-β Phase 5 (Welle W15) extracts the shared Cypher AST validator across W12 + W13 + (future) W14 surfaces. W15 entry criteria from this consolidation:

1. **Forbidden-keyword set:** union of W12 + W13 lists (already aligned post-consistency-fix; W12 + W13 both reject the 10 mandatory write-keywords + procedure-namespace escapes).
2. **Length cap:** 4096 chars (aligned post-consistency-fix).
3. **Comment-stripping:** must include `.trimStart()` after strip (W13's correctness invariant for opener-allowlist check on leading-comment queries; W12's keyword-scan-only semantic makes this optional but harmless to standardise).
4. **Procedure-namespace regex unification:** W12 uses `/\bdb\s*\./i` + `/\bCALL\b/i` (catches bare-CALL); W13 uses `/\bCALL\s+db\s*\./i` + `/\bCALL\s+dbms\s*\./i` (explicit-CALL only). W15 picks the union or the stricter of the two.
5. **String-concat detection:** W12 stricter (any `+`); W13 narrower (quote-adjacent `+`). W15 picks the architecturally cleaner rule + documents the trade-off.
6. **Parameter naming:** HTTP path uses `workspace` (single noun, post-V2-α-Welle-7 atlas-signer convention); MCP path uses `workspace_id` (pre-existing MCP-package convention since V1.19 Welle 1). W15 does NOT reconcile the parameter name (per-package consistency wins); but the shared validator code accepts either as input.

## [2.0.0-alpha.2] — 2026-05-13

**V2-α-α.2 Release Summary.** Atlas's first V2-β-promoted pre-release. Ships **three docs / workflow / design wellen** on top of v2.0.0-alpha.1: operator-facing runbook for V2-α deployment + verification (W9), parallel-projection design ADR completing the DECISION-ARCH-1 triple-hardening third leg (W10), and the `wasm-publish.yml` dual-publish race fix validated end-to-end by this very ship (W11). Public-API surface unchanged from v2.0.0-alpha.1 per [`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md) §10; this is a docs + workflow release. V1 trust property (signed events + Ed25519 + COSE_Sign1 + deterministic CBOR + blake3 hash chain + Sigstore Rekor anchoring + witness cosignature + offline WASM verifier) is preserved unchanged. All 7 V2-α byte-determinism CI pins byte-identical from v2.0.0-alpha.1 baseline (Welles 9 + 10 + 11 touched zero Rust/TypeScript surface).

The V2-α-α.2 release packages 3 V2-β wellen + 1 V2-β orchestration phase shipped on 2026-05-13: V2-β Phase 0 (orchestration plan + dependency graph + welle plan-doc template) → V2-β Phase 1 parallel batch (W9 operator runbook + W10 parallel-projection design ADR + W11 wasm-publish race fix + ADR-008 postmortem) → V2-β Phase 2 consolidation (CHANGELOG, master-plan §6, orchestration-plan welle-progress, handoff doc, W9 §-numbering fix-forward). Parallel-subagent-in-worktree dispatch architecture proven through 3 simultaneous wellen with zero file-overlap and a NEW cross-batch consistency-reviewer invariant (per V2-β Orchestration Plan §3.5).

**Validation event:** v2.0.0-alpha.2 ship is the first signed-tag publish since the W11 wasm-publish.yml dual-publish race fix landed. The fixed workflow (single `npm publish --provenance` + `npm dist-tag add` retry-loop for replication latency) replaces the dual `npm publish --tag` pattern that failed E403 on v1.0.1 + v2.0.0-alpha.1 (per ADR-Atlas-008 postmortem). Success of this publish proves the fix end-to-end.

**Wire-format compat:** identical to v2.0.0-alpha.1 — V1.0 verifiers reject events with `author_did` or `payload.type = "projector_run_attestation"` per `#[serde(deny_unknown_fields)]`; V1-shaped events remain forward-compatible. Full details in [`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md) §10 + [`docs/V2-ALPHA-2-RELEASE-NOTES.md`](docs/V2-ALPHA-2-RELEASE-NOTES.md).

**Pre-counsel-review disclaimer:** unchanged from v2.0.0-alpha.1. Public marketing claims about V2-α / V2-β's EU AI Act / GDPR posture are pre-counsel-review (per Master Plan §5 + `DECISION-COUNSEL-1`). This release is suitable for engineering / auditor / operator evaluation; external-public-materials require counsel-validated language refinement before publication.

### Added — V2-β Phase 3 (v2.0.0-alpha.2 Ship, 2026-05-13)

- **Cargo workspace version bump 2.0.0-alpha.1 → 2.0.0-alpha.2.** Single source of truth via `workspace.package.version`; all 6 workspace crates inherit through `version.workspace = true`.
- **npm version bumps** for `atlas-web`, `atlas-mcp-server`, `@atlas/bridge`, root monorepo manifest, and MCP SDK introspection string in `apps/atlas-mcp-server/src/index.ts`.
- **NEW `docs/V2-ALPHA-2-RELEASE-NOTES.md`** — engineering-perspective release notes for v2.0.0-alpha.2. V2-β Phase 0-1-2 narrative, validation-event framing for the W11 wasm-publish fix, surface-stability assurance.
- **CHANGELOG.md `[Unreleased]` promoted to `[2.0.0-alpha.2]`** with this release-summary header.

### Added — V2-β Welle 9 (Operator Runbook for v2.0.0-alpha.1, 2026-05-13)

- **NEW `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md`** (~485 lines, 8 sections §1–§8). Engineering-perspective operator reference for V2-α features additive to V1 [`OPERATOR-RUNBOOK.md`](docs/OPERATOR-RUNBOOK.md): in-memory Layer-2 + ArcadeDB-deferral disclosure (§1), `atlas-signer emit-projector-attestation` end-to-end CLI walkthrough with `--workspace` + `--derive-from-workspace` (§2), `verify_attestations_in_trace` library invocation + 3 `GateStatus` failure-mode remediation including explicit "do NOT edit events.jsonl" guardrail (§3), `@atlas-trust/verify-wasm` consumer integration (§4), Sigstore Rekor anchor flow with inclusion-proof verification call-out (§5), wire-format compat boundary with V1.0 verifiers (`#[serde(deny_unknown_fields)]` rejection of `author_did` + `projector_run_attestation`, §6), pre-counsel-review disclaimer (§7), V2-α quick-reference command table (§8).
- **Cross-references** to all 7 V2-α welle deliverables and `docs/V2-ALPHA-1-RELEASE-NOTES.md` baseline. Operators / auditors / regulators have a single document to consult for V2-α deployment + verification workflow.
- **Forward-references** to V2-β Phase 6–7 (ArcadeDB-backed persistent Layer 2) so operators understand which capabilities are alpha.1 vs upcoming beta.1.

### Added — V2-β Welle 10 (Parallel-Projection Design ADR-Atlas-007, 2026-05-13)

- **NEW `docs/ADR/ADR-Atlas-007-parallel-projection-design.md`** (~375 lines, 9 sections). Completes DECISION-ARCH-1 triple-hardening (V2-α legs: canonicalisation byte-pin + ProjectorRunAttestation; V2-β leg: parallel-projection determinism guarantees). Three options compared with HIGH/MEDIUM/LOW confidence levels and trade-off matrices: Option A workspace-parallel (HIGH confidence, recommended), Option B hash-shard-parallel (MEDIUM, deferred), Option C batch-pipeline (LOW, rejected). Option A recommendation rests on graph-topology disjointness with explicit `author_did` cross-workspace identity surface qualifier + per-workspace intra-event ordering preservation requirement + DECISION-ARCH-1 W3+W4+W5+W7 precondition. Five W17 ArcadeDB-spike-deferred open questions including SECURITY-flagged tenant isolation question.
- **Pre-implementation discipline.** No code changes; pure design doc. Implementation lands in V2-β Phase 7 (W17 ArcadeDB driver) once W16 ArcadeDB spike resolves the 5 open questions.

### Added — V2-β Welle 11 (wasm-publish.yml Dual-Publish Race Fix + ADR-Atlas-008 Postmortem, 2026-05-13)

- **`.github/workflows/wasm-publish.yml` race-condition fix** (+117 / −25 lines). Replaces the dual `npm publish --tag node` (which fails E403 against npm's version-immutability invariant regardless of tag distinction) with single `npm publish --access public --provenance` followed by `npm dist-tag add` retry-loop (6 attempts × 5s for replication latency). Sigstore Build L3 provenance preserved on the single publish. Workflow log evidence: v1.0.1 + v2.0.0-alpha.1 publishes had failed with `npm error code E403 — cannot publish over previously published versions`; gh run 25788574299 is the cited validation event. Atlas's `@atlas-trust/verify-wasm` npm publish on signed tags is now race-free; will prove out on `v2.0.0-alpha.2` ship in Phase 3.
- **NEW `docs/ADR/ADR-Atlas-008-wasm-publish-race-postmortem.md`** (~350 lines, 10 sections). Postmortem of the V1.19 + V2-α-Welle-8 publish-time race condition: root cause (npm version-immutability + dual `npm publish --tag` call), evidence chain (Sigstore Rekor logIndex 1523498404 / 1523498503), three candidate fixes considered (rejected web-only-publish, accepted single-publish + dist-tag, deferred conditional-exports unification), and a §8 follow-up welle reservation for the deferred conditional-exports work.

### Notes — V2-β Phase 1 Parallel-Batch Architecture (2026-05-13)

- **Three subagents dispatched in parallel worktrees via Atlas's `Agent` tool with `isolation: "worktree"`.** Per-welle file-areas pre-segregated to zero file-overlap (W9 → `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` + plan-doc; W10 → `docs/ADR/ADR-Atlas-007-*` + plan-doc; W11 → `.github/workflows/wasm-publish.yml` + `docs/ADR/ADR-Atlas-008-*` + plan-doc). Each welle landed as its own SSH-Ed25519-signed commit + DRAFT PR with base=master. Three PRs (#72 / #73 / #74) merged sequentially via `gh pr merge --squash --admin --delete-branch`.
- **Per-welle reviewer pattern (V2-α invariant continued).** 6 parallel reviewer agents (3 × `code-reviewer` + 3 × `security-reviewer`) dispatched after PRs hit DRAFT; per-welle CRITICAL + HIGH fixed in 2 fix-commits before merge. PR #72 fix-commit `3f3e546`: HIGH `--workspace-id` → `--workspace` flag-name correction (every bash example was non-executable), MEDIUM disclaimer §-renumber, MEDIUM "do-not-edit-events.jsonl" remediation guardrail. PR #73 fix-commit `b96ca50`: HIGH `author_did` cross-workspace identity qualifier, MEDIUM Welle 4 + Welle 6 shipped-status scoping, MEDIUM intra-event-ordering requirement, LOW ADR §6 open-questions promotion.
- **Cross-batch consistency-reviewer (V2-β NEW invariant per Orchestration Plan §3.5).** ONE additional consistency-reviewer agent dispatched after all 3 PRs reached DRAFT, reading all 3 together. Verified 10 cross-welle consistency checks (V2-α shipped-state claims uniform across docs; CLI surface `--workspace` flag identical; forbidden-files honored across all 3 PRs; pre-counsel disclaimer wording; ADR-007/008 number reservation; etc.). Verdict: zero CRITICAL or HIGH cross-welle conflicts. ONE LOW finding (W9 §7-numbering-gap from disclaimer renumber) fix-forward applied in this Phase-2-consolidation commit.
- **Byte-determinism preservation.** All 7 V2-α byte-determinism CI pins (cose × 3 + anchor × 2 + pubkey-bundle × 1 + graph-state-hash × 1) byte-identical post-Welle-11-merge — verified via `cargo test --workspace --lib --bins -p atlas-trust-core -p atlas-projector` green (169 + 14 unit-test pass-count unchanged from v2.0.0-alpha.1 baseline). Welles 9 + 10 + 11 are docs + workflow only, so this is mechanically expected; verification is the standing per-welle gate per Orchestration Plan §4.1.
- **Documentation discipline.** Forbidden-files rule (parent-only consolidation of `CHANGELOG.md` + `docs/V2-MASTER-PLAN.md` §6 status + `docs/SEMVER-AUDIT-V1.0.md` §10 + `.handoff/decisions.md` + `.handoff/v2-session-handoff.md`) honored across all 3 PRs per consistency-reviewer audit.

### Process — V2-β Phase 0 SHIPPED (Orchestration Plan + Dependency Graph + Welle Template, PR #71, 2026-05-13)

- **NEW `docs/V2-BETA-ORCHESTRATION-PLAN.md`** (~190 lines). Master-resident V2-β orchestration framework: welle inventory W9-W19 with subagent-type assignments, ADR-007 through ADR-012 pre-reservation (prevents parallel-subagent ADR-number races), anti-divergence forbidden-files rule, 8 critical orchestration invariants, cross-welle consistency-reviewer protocol.
- **NEW `docs/V2-BETA-DEPENDENCY-GRAPH.md`** (~170 lines + Mermaid). Node-edge representation of 11 wellen + 10 phases. File-area conflict matrix per parallel batch. Critical-path analysis: 16 sessions sequential → 12 with parallel dispatch (25% wall-clock saving). Rollback / re-plan trigger criteria.
- **NEW `.handoff/v2-beta-welle-N-plan.md.template`** (~180 lines). 10-section plan-doc skeleton validated through V2-α Welles 1-8. Mandatory subagent-dispatch prompt skeleton with `git fetch && git checkout master` pre-flight to honor the worktree-fork-base lesson from V2-α.

## [2.0.0-alpha.1] — 2026-05-13

**V2-α-alpha.1 Release Summary.** Atlas's first pre-release of the V2 line. Ships the **cryptographic projection-state verification primitive end-to-end**: a third-party verifier with the offline WASM verifier + `events.jsonl` + `pubkey-bundle.json` can independently re-project a trace and produce a structured `Match` / `Mismatch` outcome per `ProjectorRunAttestation` event — drift detected cryptographically, not just by CI convention. Atlas's V1 trust property (signed events + Ed25519 + COSE_Sign1 + deterministic CBOR + blake3 hash chain + Sigstore Rekor anchoring + witness cosignature + offline WASM verifier) is preserved unchanged; V2-α-alpha.1 is an additive cryptographic layer on top.

The V2-α-alpha.1 surface delivers 8 wellen shipped in one sprint (2026-05-12 to 2026-05-13): Agent-DID Schema Foundation (`did:atlas:<blake3-pubkey-hash>`) → ArcadeDB vs FalkorDB spike with ArcadeDB-primary recommendation flip → Atlas Projector skeleton with canonicalisation byte-determinism pin → `ProjectorRunAttestation` event-schema + verifier-side parser → emission pipeline (`events.jsonl` → `GraphState` → attestation payload) → projector-state-hash CI gate (closes V2-α security loop) → `atlas-signer emit-projector-attestation` CLI subcommand → v2.0.0-alpha.1 ship. **7 byte-determinism CI gates** now cover V1 + V2-α canonicalisation surfaces.

**Wire-format note:** V2-α events with `author_did` field set OR `payload.type = "projector_run_attestation"` are intentionally non-deserialisable by V1.0 verifiers (per `#[serde(deny_unknown_fields)]` policy). This is the explicit SemVer-major break committed by V2.0.0-alpha.1. V1-shaped events (no `author_did`, no V2-α-only payload kinds) remain forward-compatible across both verifier generations. Full details in [`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md) §10 + [`docs/V2-ALPHA-1-RELEASE-NOTES.md`](docs/V2-ALPHA-1-RELEASE-NOTES.md).

**Pre-counsel-review disclaimer:** Public marketing claims about V2-α-alpha.1's EU AI Act / GDPR posture are pre-counsel-review (per Master Plan §5 + `DECISION-COUNSEL-1`). This release is suitable for engineering / auditor / operator evaluation; external-public-materials require counsel-validated language refinement before publication.

### Added — V2-α Welle 8 (v2.0.0-alpha.1 Ship, 2026-05-13)

- **Cargo workspace version bump 1.0.1 → 2.0.0-alpha.1.** Single source of truth via `workspace.package.version`; all 6 workspace crates inherit through `version.workspace = true`.
- **npm version bumps** for `atlas-web`, `atlas-mcp-server`, `@atlas/bridge`, root monorepo manifest, and MCP SDK introspection string.
- **NEW `docs/V2-ALPHA-1-RELEASE-NOTES.md`** (~250 lines) — comprehensive engineering-perspective release notes: V2-α security model, public-API additions per Welle, V1-backward-compat boundary, operator-runbook pointers, demo CLI invocation, pre-counsel-review disclaimer.
- **CHANGELOG.md `[Unreleased]` promoted to `[2.0.0-alpha.1]`** with this release-summary header.

### Added — V2-α Welle 7 (atlas-signer `emit-projector-attestation` Subcommand, 2026-05-13)

- **NEW `atlas-signer emit-projector-attestation` subcommand** (clap-registered). Operators can now in one shell command read an `events.jsonl` file, project the events via atlas-projector, build a ProjectorRunAttestation payload, and emit a signed AtlasEvent on stdout ready for `>> events.jsonl` append. Closes the V2-α producer-side CLI ergonomic.
- **atlas-signer becomes the first in-tree consumer of atlas-projector.** `Cargo.toml` adds `atlas-projector = { path = "../atlas-projector" }` + `chrono` + `ulid`. Clean DAG: `atlas-trust-core ← atlas-projector ← atlas-signer`.
- **CLI flags:** `--events-jsonl <path>` (required), `--workspace <id>` (required), `--head-event-hash <hex>` (required, 64-lowercase-hex), `--projector-version <string>` (optional; default `atlas-projector/<crate-version>`), `--ts <iso8601>` (optional; default `chrono::Utc::now().to_rfc3339()`), `--event-id <ulid>` (optional; default freshly generated), plus standard signer args (`--kid` OR `--derive-from-workspace` + master-seed gate).
- **NEW pure orchestration helper** `build_projector_attestation_payload_from_jsonl(events_jsonl, workspace_id, head_event_hash, projector_version_override)` — testable boundary; no I/O, no signing, no stdout. Filters existing attestation events before projection.
- **NEW dispatcher `run_emit_projector_attestation_dispatch(args)`** reuses the existing `run_sign_dispatch` machinery via synthesised `SignArgs` — zero duplication of key-management, secret-handling, signing-pipeline code.
- **8 unit tests** in `welle_7_tests` module covering:
  - `happy_path_builds_well_formed_attestation_payload` (round-trip parse + validate)
  - `malformed_jsonl_surfaces_error`
  - `malformed_head_event_hash_rejected_at_emission_boundary` (defence-in-depth)
  - `existing_attestation_events_filtered_out_before_projection`
  - `default_projector_version_uses_crate_version`
  - `empty_jsonl_with_count_zero_would_be_rejected_by_emission_boundary`
  - `payload_kind_matches_atlas_trust_core_constant`
  - **`output_passes_welle_6_gate_in_round_trip`** — headline V2-α contract test: an attestation produced by Welle 7's producer code, when included in a trace with its source events, MUST pass Welle 6's `verify_attestations_in_trace` gate with `GateStatus::Match`. Closes the V2-α producer-consumer loop end-to-end.

### Notes — V2-α Welle 7

- **End-to-end producer-side CLI complete.** After Welle 7 the V2-α producer-side flow is one shell command: `atlas-signer emit-projector-attestation --events-jsonl trace/events.jsonl --workspace ws-q1-2026 --derive-from-workspace ws-q1-2026 --head-event-hash <hex> >> trace/events.jsonl`. The `--kid` flag is auto-derived from `--derive-from-workspace` (Welle 7 reviewer fix; mirrors operator expectation that `--derive-from-workspace` alone is sufficient for the hot path). Operators / auditors / regulators see an immediate, demonstrable signed attestation in one invocation.
- **Structural `projector_version` honesty.** Default `--projector-version` binds to `atlas_projector::CRATE_VERSION` (newly exported as `pub const`), NOT atlas-signer's `CARGO_PKG_VERSION`. The attested `projector_version` field now structurally reflects the actual projection-logic version even if the two crates ever diverge (Welle 7 reviewer fix).
- **First in-tree atlas-projector consumer.** Welle 7 exercises atlas-projector's public API (`parse_events_jsonl`, `project_events`, `build_projector_run_attestation_payload`) from a real downstream binary, surfacing any ergonomic gaps that would otherwise wait until external consumers adopted the crate. No gaps surfaced; existing surface holds.
- **Round-trip property cemented in unit test.** `output_passes_welle_6_gate_in_round_trip` proves the Welle 7 producer → Welle 6 consumer chain is correct by construction: producer output matches consumer expectations, validated at compile-test-time on every CI run.
- **V1 backward-compat preserved.** atlas-trust-core untouched. atlas-projector untouched. All 7 byte-determinism CI pins byte-identical. 169 atlas-trust-core unit tests unchanged. 52 atlas-projector unit tests unchanged. 10 atlas-projector gate integration tests unchanged. 6 atlas-projector pipeline integration tests unchanged. Welle 7 adds 8 atlas-signer unit tests; zero V1 or V2-α regression.
- **V2-α progress: 7 of 5-8 wellen shipped** (Welle 1 Agent-DID, Welle 2 DB spike, Welle 3 projector skeleton, Welle 4 attestation parser, Welle 5 emission pipeline, Welle 6 CI gate, Welle 7 signer CLI). **V2-α producer + consumer surfaces both complete end-to-end.**

### Added — V2-α Welle 6 (Projector-State-Hash CI Gate Enforcement, 2026-05-13)

- **NEW `crates/atlas-projector/src/gate.rs`** (~280 lines + 2 unit tests) — closes the V2-α security loop. `verify_attestations_in_trace(workspace_id, trace) -> ProjectorResult<Vec<GateResult>>` partitions the trace's events into projectable + attestation sets, re-projects all projectable events via Welle 5's `project_events`, recomputes `graph_state_hash` via Welle 3, then parses each `ProjectorRunAttestation` event via Welle 4 and compares attested vs recomputed. Returns one `GateResult` per attestation event with structured `Match` / `Mismatch` / `AttestationParseFailed` status.
- **`GateResult` struct** with per-attestation comparison fields: `event_id`, `attested_hash`, `recomputed_hash`, `attested_event_count`, `actual_event_count`, `status`. Auditor-friendly diagnostics.
- **`GateStatus` enum** (`#[non_exhaustive]`) — additive-safe outcome discriminator. Welle-6-MVP variants: `Match` / `Mismatch` / `AttestationParseFailed`. Future welles may add states like `HeadEventHashNotFound` or `IncrementalCoverageGap` additively.
- **NEW `crates/atlas-projector/tests/projection_gate_integration.rs`** (~210 lines, 8 E2E tests):
  - `happy_path_attestation_matches_reprojection` — full V2-α security loop green
  - `tampered_attestation_hash_mismatch_detected` — flipping the attested hash flips status to Mismatch
  - `mismatched_projected_event_count_detected` — claim N events but trace has M ≠ N
  - `multiple_attestation_events_each_verified` — N attestations → N GateResults
  - `malformed_attestation_payload_surfaces_parse_failed_status`
  - `trace_without_attestation_events_returns_empty_vec`
  - `unsupported_event_kind_in_trace_surfaces_error` — re-projection fails cleanly
  - `end_to_end_jsonl_parse_project_emit_then_gate_verifies` — headline demo path

### Notes — V2-α Welle 6

- **DECISION-ARCH-1 triple-hardening completion advances.** Welle 3 ✓ canonicalisation byte-pin, Welles 4+5 ✓ ProjectorRunAttestation event-binding, Welle 6 ✓ projector-state-hash CI gate. Combined: drift between an issuer's projector and a verifier's fresh re-projection is now **detectable cryptographically**, not just by CI convention. (The remaining triple-hardening leg — parallel-projection design for >10M event scenarios — is deferred to V2-β / Welle 8+.)
- **Strategic milestone:** Welle 6 is the cryptographic-verification primitive Atlas's V2-α architecture has been building toward. After Welle 6 a third-party verifier can independently re-project a trace and produce a structured `Match` / `Mismatch` outcome per ProjectorRunAttestation event. **This is what makes V2-α projection "trustworthy without trusting Atlas operator."**
- **Caller contract:** trace must be pre-verified by `atlas_trust_core::verify_trace` (V1 signature + hash chain + anchor checks). Welle 6 does not re-check signatures — it focuses on projection-state verification. Documented in module-doc.
- **Welle-6-MVP semantics:** each `ProjectorRunAttestation` event asserts the FULL projection state at its `head_event_hash`. For incremental-attestation semantics (each attestation only covers events since the last one), future welles may add a different comparison mode. Welle 6 covers the common case.
- **V1 backward-compat preserved.** atlas-trust-core untouched. All 7 byte-determinism CI pins byte-identical. 169 atlas-trust-core unit tests + 5 attestation integration tests unchanged. 52 atlas-projector unit tests (was 50, +2 gate tests). 14 atlas-projector integration tests (was 6, +8 gate integration tests). Zero V1 or V2-α regression.
- **V2-α progress: 6 of 5-8 wellen shipped** (Welle 1 Agent-DID, Welle 2 DB spike, Welle 3 projector skeleton, Welle 4 attestation parser, Welle 5 emission pipeline, Welle 6 CI gate). **V2-α range now upper-mid complete.**

### Added — V2-α Welle 5 (Atlas-Projector Emission Pipeline: events.jsonl → GraphState → Attestation, 2026-05-13)

- **NEW `crates/atlas-projector/src/replay.rs`** (~150 lines + tests). `parse_events_jsonl(contents: &str) -> ProjectorResult<Vec<AtlasEvent>>` library-only JSONL parser with 1-indexed line-number diagnostics, blank-line + `//` comment tolerance, fail-fast on first malformed line. 7 unit tests.
- **NEW `crates/atlas-projector/src/upsert.rs`** (~390 lines + tests). Idempotent event-to-state upsert. `apply_event_to_state(workspace_id, event, state)` dispatches on `payload.type`: `node_create` (with `node.id` → entity_uuid OR blake3-derived fallback), `node_update` (with `node_id` → existing-entity overwrite), `edge_create` (with `from` + `to` + `relation`). `project_events(workspace_id, events, existing)` top-level convenience. author_did from Welle 1 propagates onto every node/edge upserted by an event. Unsupported event-kinds (`policy_set`, `annotation_add`, `anchor_created`) surface `UnsupportedEventKind` cleanly. 14 unit tests covering each upsert path + idempotency + author_did propagation + entity_uuid determinism + edge/node id-separation.
- **NEW `crates/atlas-projector/src/emission.rs`** (~150 lines + tests). `build_projector_run_attestation_payload(state, projector_version, head_event_hash, projected_event_count) -> ProjectorResult<serde_json::Value>` — payload-only emission API (caller signs). Computes graph_state_hash via Welle 3, constructs JSON matching Welle 4's `PROJECTOR_RUN_ATTESTATION_KIND` shape. Schema-version + payload-kind bound to atlas-trust-core constants — emission stays in lockstep with validation. 5 unit tests including round-trip through atlas-trust-core's `parse_projector_run_attestation` + `validate_projector_run_attestation`.
- **NEW `crates/atlas-projector/tests/projector_pipeline_integration.rs`** (~190 lines, 6 E2E tests):
  - `full_pipeline_e2e_jsonl_to_attested_state` — 5-event JSONL → parse → project → emit → round-trip green
  - `idempotency_same_events_twice_byte_identical_state_hash`
  - `unsupported_event_kind_surfaces_structured_error`
  - `malformed_jsonl_surfaces_line_number`
  - `empty_jsonl_produces_empty_state_and_emission_rejects_zero_count`
  - `pipeline_preserves_existing_state` (checkpoint-resume scenario)
- **`ProjectorError` new variants:** `ReplayMalformed { line_number, reason }`, `UnsupportedEventKind { kind, event_id }`, `MissingPayloadField { event_id, field }`. Additive under `#[non_exhaustive]`.

### Notes — V2-α Welle 5

- **End-to-end pipeline complete.** After Welle 5 Atlas demonstrates the full chain: signed events.jsonl → parse → idempotent projection → emit attestation payload → round-trip through verifier. Welles 1 + 3 + 4 now have a working producer connecting them.
- **entity_uuid convention:** prefer issuer-supplied `node.id` (matches user mental model); fall back to `hex(blake3(workspace_id || 0x1f || event_uuid || 0x1f || ":node"))` when absent (per Welle 2 §3.5 logical-identifier sort key requirement). edge_id always blake3-derived (`":edge"` suffix). Documented in `upsert.rs` module docstring.
- **node_update V2-α-MVP semantics:** REPLACE (not merge). Future welles may add patch-merge; documented as Welle-5 limitation.
- **Library-only — no `std::fs` dependency.** Callers handle file I/O. Keeps atlas-projector WASM-friendly for future wellen.
- **Welle 5 does NOT sign.** Emission produces a `serde_json::Value` payload; caller (atlas-signer or future SDK) assembles + signs the wrapping AtlasEvent.
- **Welle 5 does NOT re-verify event signatures during replay.** That's the verifier's responsibility (`atlas_trust_core::verify_trace`). Replay assumes the caller has chosen whether to validate.
- **V1 backward-compat preserved.** All 7 byte-determinism pins byte-identical (V1 cose + Welle 1 author_did + V1.7 anchor-canonical-body + V1.7 anchor-head + V1.9 pubkey-bundle + Welle 3 graph-state-hash + Welle 4 attestation-signing-input). 169 atlas-trust-core unit tests unchanged. 5 attestation integration tests unchanged. 46 atlas-projector unit tests (was 20, +26 for Welle 5). 6 new E2E integration tests. Zero regression.
- **V2-α progress: 5 of 5-8 wellen shipped** (Welle 1 Agent-DID, Welle 2 DB spike, Welle 3 projector skeleton, Welle 4 attestation, Welle 5 emission pipeline). **Lower bound of V2-α range complete.**

### Added — V2-α Welle 4 (ProjectorRunAttestation Event-Schema + Verifier-Side Parsing, 2026-05-12)

- **NEW `crates/atlas-trust-core/src/projector_attestation.rs` module** (~370 lines + tests). Public surface: `PROJECTOR_RUN_ATTESTATION_KIND = "projector_run_attestation"` (payload `type` discriminator); `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION = "atlas-projector-run-attestation/v1-alpha"` (envelope schema-version, separate from `atlas-projector::PROJECTOR_SCHEMA_VERSION` which versions the GraphState canonical form); `ProjectorRunAttestation` typed struct (5 fields: `projector_version`, `projector_schema_version`, `head_event_hash`, `graph_state_hash`, `projected_event_count`); `parse_projector_run_attestation(payload)` JSON-to-typed parser with strict-mode unknown-field rejection; `validate_projector_run_attestation(att)` strict format-validator (non-empty projector_version, schema-version match, 64-lowercase-hex hashes, non-zero event count).
- **NEW `AtlasPayload::ProjectorRunAttestation { ... }`** enum variant in `trace_format.rs` for typed inspection of attestation events. Underlying `AtlasEvent.payload` remains `serde_json::Value` for forward-compat.
- **NEW `TrustError::ProjectorAttestationInvalid { reason }`** variant. Structured reject path for malformed attestation payload. Additive under `#[non_exhaustive]`.
- **NEW verifier-side validation in `verify_trace`** — when an event's `payload.type` is `projector_run_attestation`, run `parse_projector_run_attestation` + `validate_projector_run_attestation` BEFORE signing-input construction (mirrors Welle 1's `validate_agent_did` placement). Failure surfaces structured `ProjectorAttestationInvalid` ahead of downstream errors. V1 payloads (`node.create` / `node.update` / etc.) pass through unchanged.
- **NEW byte-determinism CI pin** `cose::tests::signing_input_byte_determinism_pin_with_projector_attestation` — pinned blake3 `8fbe734511c6347a5fe18476d7fb32a6b6650652e9319dcb8f91d4ba70865557` for a fixture event with ProjectorRunAttestation payload. Co-equal with V1's `signing_input_byte_determinism_pin`, V1.7's `anchor::chain_canonical_body_byte_determinism_pin`, V1.9's `pubkey_bundle::bundle_hash_byte_determinism_pin`, Welle 1's `signing_input_byte_determinism_pin_with_author_did`, and Welle 3's `graph_state_hash_byte_determinism_pin`. Atlas now has **6 byte-determinism CI gates** covering V1 + V2-α.
- **NEW 15 unit tests** in projector_attestation.rs covering parse roundtrip, missing-fields rejection, unknown-field rejection (strict-mode), wrong-type-discriminator rejection, schema-version mismatch rejection, malformed hex (wrong length / uppercase / non-hex chars), zero-count rejection.
- **NEW 5 integration tests** in `crates/atlas-trust-core/tests/projector_attestation_integration.rs` covering: (1) well-formed attestation event verifies clean, (2) malformed schema_version rejected at verify time, (3) malformed hex hash rejected, (4) tampered attestation breaks signature/hash check, (5) **rigorous signature-swap test** (analog to Welle 1's `signature_swap_between_freshly_signed_events_fails`): two freshly-signed attestation events with different `graph_state_hash` values cannot cross-substitute signatures.

### Notes — V2-α Welle 4

- **Cryptographic chain-of-custody for projection-state.** Welle 3 produced the `graph_state_hash` primitive. Welle 4 elevates it from CI-gate-only material to a Layer-1 trust-chain artefact: any third party with the offline WASM verifier + `events.jsonl` + `pubkey-bundle.json` can confirm "this projector run on this event-head produced this graph state at time T" without trusting Atlas operator. Implements `DECISION-SEC-2` Phase-2-Security-Q-SEC-6 requirement.
- **V1 backward-compat preserved.** All 6 byte-determinism pins byte-identical (V1 cose + V1.7 anchor + V1.9 pubkey_bundle + Welle 1 author_did + Welle 3 graph_state_hash + Welle 4 attestation). 166 atlas-trust-core unit tests pass (was 150 in Welle 3, +16 new for attestation). 20 atlas-projector tests unchanged. 5 new attestation integration tests pass. Zero V1 or V2-α regression.
- **Welle 4 scope is consumer/verifier side only.** Emission (the producer side — actual projector code reading `events.jsonl` and signing an attestation event as a side-effect of every run) arrives with Welle 5. Cross-trace integrity (head_event_hash actually points to an event in the same `events.jsonl`) enforced by a later welle when emission ships.
- **V2-α progress: 4 of 5-8 wellen shipped** (Welle 1 Agent-DID + Welle 2 DB spike + Welle 3 projector skeleton + Welle 4 attestation, all 2026-05-12).

### Added — V2-α Welle 3 (Atlas Projector Skeleton + Canonicalisation Byte-Pin, 2026-05-12)

- **NEW `crates/atlas-projector/` workspace crate** — V2-α Layer-2 graph projection canonicalisation. Public surface: `GraphState` / `GraphNode` / `GraphEdge` types (in-memory representation, `BTreeMap`-backed for load-bearing logical-identifier-sorted canonical iteration); `build_canonical_bytes()` (RFC 8949 §4.2.1 CBOR canonical encoding); `graph_state_hash()` (blake3 over canonical bytes); `ProjectorError` enum (`#[non_exhaustive]`, 5 variants); `PROJECTOR_SCHEMA_VERSION = "atlas-projector-v1-alpha"` const bound into every canonicalisation.
- **NEW `canonical::tests::graph_state_hash_byte_determinism_pin`** — pinned blake3 `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` (754 canonical bytes) for a 3-node + 2-edge fixture with mixed labels and mixed `author_did` presence. Co-equal CI gate with V1's `cose::signing_input_byte_determinism_pin`, V1.7's `anchor::chain_canonical_body_byte_determinism_pin`, V1.9's `pubkey_bundle::bundle_hash_byte_determinism_pin`, and Welle 1's `signing_input_byte_determinism_pin_with_author_did`.
- **19 unit tests** in atlas-projector covering: empty-state hash, single-node hash, multi-node insert-order independence (the load-bearing Welle 2 §3.5 invariant), property-order independence, label-order independence + dedup, `author_did` schema-additive binding into hash, V1 backward-compat for `author_did = None`, float rejection at canonicalisation boundary, dangling-edge structural-integrity rejection, malformed-DID rejection, and the byte-determinism pin.
- **Atlas-projector depends on atlas-trust-core** only for `agent_did::validate_agent_did` cross-validation. Clean DAG — atlas-trust-core does NOT depend on atlas-projector.
- **NEW `.handoff/v2-alpha-welle-3-plan.md`** (~200 lines) — Welle 3 plan-doc with scope, decisions, files table, acceptance criteria, 5-entry risks table, V1-test-impact matrix, and out-of-scope items for V2-α Welles 4-8.
- **MODIFY `Cargo.toml` workspace** — add `"crates/atlas-projector"` member entry.
- **MODIFY `docs/SEMVER-AUDIT-V1.0.md` §10** — new subsection §10.7a listing every new `atlas-projector` `pub` item with V2-α-Additive tag.

### Notes — V2-α Welle 3

- **V1 backward-compat preserved.** All 150 atlas-trust-core unit tests + 4 byte-determinism CI pins (V1's `signing_input_byte_determinism_pin`, V1.7's `chain_canonical_body_byte_determinism_pin`, V1.9's `bundle_hash_byte_determinism_pin`, Welle 1's `signing_input_byte_determinism_pin_with_author_did`) pass byte-identical after Welle 3. Zero regression.
- **Container choice is load-bearing.** `GraphState.nodes` and `.edges` use `BTreeMap` keyed by logical identifier (`entity_uuid` / `edge_id`) — iteration is sorted automatically per Rust stdlib. The Welle 2 §3.5 caveat ("`@rid` is insert-order, NOT logical identity anchor") is therefore structurally impossible to violate from within this crate's API.
- **Out of scope (deferred to later wellen):** events.jsonl reading + idempotent upsert (Welle 4), `ProjectorRunAttestation` event-kind emission (Welle 4), ArcadeDB driver integration (Welle 5), projector-state-hash CI gate enforcement (Welle 6), parallel-projection design for >10M event scenarios (Welle 5).
- **V2-α progress: 3 of 5-8 wellen shipped** (Welle 1 Agent-DID + Welle 2 DB spike + Welle 3 projector skeleton).

### Added — V2-α Welle 2 (ArcadeDB vs FalkorDB Comparative Spike, 2026-05-12)

- **`docs/V2-ALPHA-DB-SPIKE.md`** (new, ~500 lines) — master-resident V2-α DB-choice decision source-of-truth. Comparative analysis of ArcadeDB (Apache-2.0) vs FalkorDB (SSPLv1) across 10 dimensions: license (SSPLv1 §13 vs Apache-2.0 §4-§5), Cypher subset coverage, property graph model, idempotent upsert pattern, multi-tenant isolation, schema determinism, performance characteristics, operational considerations, vendor risk, and 5 Atlas-specific decision factors (projection-determinism cost, author_did stamping, ProjectorRunAttestation hooks, V2-β Mem0g integration, V2-γ federation-witness property-visibility).
- **`.handoff/v2-alpha-welle-2-plan.md`** (new, ~180 lines) — Welle 2 plan-doc with scope, decisions, files table, spike-doc target outline, acceptance criteria, risks, and out-of-scope items.

### Changed — V2-α Welle 2 (Strategic DB-Choice Flip)

- **V2-α DB primary flipped from FalkorDB to ArcadeDB** per `DECISION-DB-4` (new). Recommendation confidence MEDIUM-HIGH; deciding factor is license compatibility (SSPLv1 §13 vs Apache-2.0) for Atlas's planned open-core hosted-service monetization tier. Secondary factors: projection-determinism canonicalisation cost (~30% lower with ArcadeDB's `ORDER BY @rid` + schema-required mode) and self-hosted-tier deployment simplicity (ArcadeDB embedded mode lets Atlas ship as single-process server). Reversal cost MEDIUM (re-projection from authoritative Layer 1 `events.jsonl`, 1-2 sessions of projector rewrite, zero customer downtime via dual-write).
- **`docs/V2-MASTER-PLAN.md` §3 + §4 R-L-02 + §6 V2-α Foundation + §11 Reference Pointers** updated to reflect the ArcadeDB-primary flip. ASCII Three-Layer Architecture diagram updated.
- **`.handoff/decisions.md`** — `DECISION-DB-1` (original Kuzu→ArcadeDB-fallback) annotated as superseded; new `DECISION-DB-4` documents the primary flip with full rationale, confidence level, and reversal-cost analysis. Now 23 decisions documented.

### Notes — V2-α Welle 2

- **Spike methodology: public-knowledge-based research, no actual benchmarks executed.** If Welle 3 (Projector skeleton) implementation surfaces Cypher-subset incompatibilities in ArcadeDB OR if Nelson commissions a Welle 2b actual-benchmark validation, the recommendation may be revisited.
- **Counsel-validated SSPLv1 §13 opinion** remains on Nelson's parallel counsel-engagement track and is pre-V2-α-public-materials blocking per Master Plan §5.
- **V2-α progress: 2 of 5-8 wellen shipped (Welle 1 Agent-DID schema + Welle 2 DB spike).** Welle 3 candidate: Atlas Projector skeleton against locked ArcadeDB choice.

### Added — V2-α Welle 1 (Agent-DID Schema Foundation, 2026-05-12)

- **`crates/atlas-trust-core/src/agent_did.rs`** (new module) — W3C-DID parser, validator, and presentation-layer helpers for `did:atlas:<lowercase-hex-32-bytes>` agent identities. Public surface: `AGENT_DID_PREFIX`, `agent_did_for`, `parse_agent_did`, `validate_agent_did`. 13 unit tests covering positive + negative format-validation cases, parse roundtrip, structured-error reasons. Re-exported at crate root.
- **`AtlasEvent.author_did: Option<String>`** field (`crates/atlas-trust-core/src/trace_format.rs`) — optional agent-identity binding on every signed event. When present, canonically bound into the signing-input alongside `kid` (Phase 2 Security H-1), providing cross-agent-replay defence in addition to V1's cross-workspace-replay defence. When absent, events remain V1-shaped and byte-identical to pre-Welle-1 output.
- **`TrustError::AgentDidFormatInvalid { did, reason }`** new variant — structured reject path for malformed `author_did` values. Verifier surfaces this before signature-check so auditor tooling sees the precise failure mode. Additive under `#[non_exhaustive]` per `SEMVER-AUDIT-V1.0.md` §8.
- **`cose::tests::signing_input_byte_determinism_pin_with_author_did`** — new V2-α byte-determinism CI pin. Locks exact CBOR bytes for fixture event with `author_did = Some(...)`. Map header is `a8` (8 pairs); `author_did` entry sorts LAST per RFC 8949 §4.2.1 (longest encoded-key length, 11 bytes). The V1 pin `cose::tests::signing_input_byte_determinism_pin` is preserved byte-identical — V1-shaped events produce identical CBOR pre- and post-Welle-1.
- **`crates/atlas-trust-core/tests/agent_did_integration.rs`** (new integration test) — 4 end-to-end test cases: (1) sign+verify with `author_did = Some(...)`, (2) V1 backward-compat (no `author_did`), (3) malformed DID rejected at verify-time with `AgentDidFormatInvalid`, (4) cross-agent-replay defence (tampered well-formed DID fails signature check).

### Changed — V2-α Welle 1

- **`cose::build_signing_input` signature** — added trailing parameter `author_did: Option<&str>`. Callers passing `None` produce byte-identical CBOR to V1 (V1 byte-determinism pin holds unchanged). Source-break for direct callers; all 15 in-tree callers updated (atlas-signer CLI, atlas-signer demo, hashchain inner verify, verify.rs main loop, 6 integration tests).
- **`verify_trace` pre-signature-check hardening** — when `event.author_did` is `Some(_)`, format-validates against `did:atlas:<64-lowercase-hex>` shape and rejects with `AgentDidFormatInvalid` before downstream signature/hash checks. V1 events without `author_did` follow the unchanged verifier path.

### Notes — V2-α Welle 1

- **Workspace version unchanged at `1.0.1`.** A major-bump release (`v2.0.0-alpha.1` candidate) is deferred to the close-out of the V2-α welle bundle per [`.handoff/v2-alpha-welle-1-plan.md`](.handoff/v2-alpha-welle-1-plan.md) §"Decisions". Welle 1 lands on master; the version tag waits for Projector + FalkorDB + content-hash separation (if counsel-approved) + Agent-DID-end-to-end on atlas-signer CLI to ship as a coherent v2.0.0-alpha.1.
- **Wire-compat break for V1.0 verifiers reading V2-α events with `author_did = Some(...)`** is by design and documented in `docs/SEMVER-AUDIT-V1.0.md` §10. V1.0 verifiers deserialize via `#[serde(deny_unknown_fields)]` and will surface `unknown_field("author_did")`. V1-shaped events (no `author_did`) remain forward-compatible across both verifier generations.
- **Trust invariant preserved:** `cose::tests::signing_input_byte_determinism_pin` retains its V1 pinned hex byte-identically. All 146 atlas-trust-core unit tests + 4 new integration tests + the full workspace test suite pass green. Zero V1 regression.

### Documentation — V2 Strategic Planning (2026-05-12)

- **`docs/V2-MASTER-PLAN.md`** (new, ~300 lines) — master-resident strategic plan for Atlas V2. Distilled from Master Vision v1 with Welle decomposition tied to concrete PR-Wellen (V2-α / V2-β / V2-γ / V2-δ, total 14–20 sessions plus 6–8 weeks counsel-engagement in parallel with V2-α), top-5 V2-α blocking risks, 7-demo programme with hero-CTA-inversion (Demo 2 Continuous Regulator Witness above-the-fold primary), and explicit success criteria. Companion to `docs/WORKING-METHODOLOGY.md`.
- **`docs/WORKING-METHODOLOGY.md`** (new, ~200 lines) — reusable 4-phase iteration pattern (Foundation Docs → Multi-Angle Critique → Synthesis → Plan Documentation) with 8-entry anti-pattern table and explicit "when to skip" rules. Use for future Großthemen (e.g. post-quantum migration, V3 architecture). Independent versioning from per-Großthema Master Plans.
- **`.handoff/v2-master-vision-v1.md`** (new on master, ~615 lines) — Phase-3 synthesis output mirrored from PR #62 draft-branch for master-reference-ability. 15-section consolidated V2 vision including factual corrections from Phase-2 critique (EU AI Liability Directive WITHDRAWN Feb 2025 → fallback regime is Product Liability Directive 2024/2853; "independently verifiable" Art. 12 phrasing replaced with verbatim text; Art. 18 / Art. 19 conflation fixed). Full rationale for everything in V2-MASTER-PLAN.md.
- **`.handoff/decisions.md`** (new on master, ~284 lines) — Phase-3 decision log with 22 explicit ACCEPT/MODIFY/DEFER entries. Each carries crit-source attribution, reversibility tag (HIGH/MEDIUM/LOW), and review-after trigger. Cross-referenced from V2-MASTER-PLAN + Master Vision via stable `DECISION-<DOMAIN>-<N>` IDs.
- **`.handoff/v2-session-handoff.md`** (updated, +400 lines) — Phase 1+2+3+4 ALL SHIPPED state, V2-α Welle 1 pre-flight checklist, branch-and-PR diagram showing master-resident outputs and permanently-draft work-product archives (#59/#61/#62).

**No v1.0 public-API surface touched.** Per SemVer contract committed at v1.0.0, these are pure documentation additions. Reproducibility, signed-tag chain, npm `@atlas-trust/verify-wasm@1.0.1` byte-identical state — all unchanged.

## [1.0.1] — 2026-05-12

**SemVer-patch release — first version published to the npm registry.** No code changes; trust property, public API, and signed-tag chain are byte-identical to v1.0.0. This release corrects a `Cargo.toml` `workspace.package.repository` field that pointed at a stale organisation path (`https://github.com/ultranova/atlas`) instead of the canonical `https://github.com/ThePyth0nKid/atlas`. wasm-pack derives `package.json`'s `repository.url` from that Cargo field; npm's SLSA Build L3 provenance validator rejected the v1.0.0 publish attempt because the package.json URL did not match the GitHub Actions OIDC token's source-repository claim (`422 Unprocessable Entity — Error verifying sigstore provenance bundle: Failed to validate repository information`).

### Fixed — V1.19 Welle 14a

- `Cargo.toml` `workspace.package.repository`: `https://github.com/ultranova/atlas` → `https://github.com/ThePyth0nKid/atlas`. Flows through `wasm-pack build` into the generated `package.json` `repository.url`; the new value matches the OIDC `repository` claim emitted by GitHub Actions for `ThePyth0nKid/atlas`, unblocking SLSA Build L3 provenance verification.
- `docs/ARCHITECTURE.md` reproduce-from-source `git clone` URL updated to match.

### Changed — V1.19 Welle 14a

- Workspace version bump 1.0.0 → 1.0.1 (single source of truth via `workspace.package.version`; all 5 crates inherit through `version.workspace = true`).
- npm version bumps for `atlas-web`, `atlas-mcp-server`, `@atlas/bridge`, root monorepo manifest, and the `apps/atlas-mcp-server/src/index.ts` MCP server registration version.

### Notes

- The signed Git tag `v1.0.0` (`e97c025`, SSH-Ed25519 `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`) is preserved unmodified. Atlas's tag-immutability invariant is upheld: published-but-unreachable artefacts are corrected by SemVer-patch, not by retroactive tag mutation.
- The GitHub Release for `v1.0.0` remains live as a historical record with its byte-identical npm-pack tarballs; the release notes flag that the npm publish did not land for this tag and direct consumers to `npm install @atlas-trust/verify-wasm@1.0.1` (or `@latest`) instead.
- No `Locked` public-API surface in [`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md) is touched. Per the SemVer contract committed at v1.0.0, this is a strict patch-level release.

## [1.0.0] — 2026-05-11

**v1.0.0 Release Summary** — Atlas's first SemVer-stable public release. The verifier crate (`atlas-trust-core`) is feature-complete across all V1.0–V1.19 trust-property increments: Ed25519 + COSE_Sign1 + deterministic CBOR + 7 base check categories (V1.0), Sigstore Rekor anchoring with pinned log-pubkey (V1.5), anchor-chain linkage (V1.7), HKDF per-tenant key derivation (V1.9), opt-in strict modes for per-tenant keys / anchors / anchor-chain / witness-threshold / strict-chain (V1.10 + V1.13 + V1.19 Welle 9), HSM-optional signing via PKCS#11 (V1.10 wave-2 + V1.12 wave-3), witness cosignature attestation (V1.13), production hosting on Cloudflare Workers (V1.16), SSH-Ed25519 tag-signing + trust-root-mutation defence (V1.17), defence-in-depth + multi-issuer Sigstore tracking (V1.18), browser-rendering UI E2E coverage with WCAG 2.1 AA a11y (V1.19 Welle 11), and the user-facing `POST /api/atlas/write-node` HTTP write surface (V1.19 Welle 1). The `@atlas-trust/verify-wasm` package on npm provides the same trust property in the browser as the native CLI. The v1.0 public-API surface contract is documented in [`docs/SEMVER-AUDIT-V1.0.md`](docs/SEMVER-AUDIT-V1.0.md); from this release forward, any breaking change to a `Locked` item triggers a SemVer-major bump.

### Added — V1.19 Welle 13 (this release)

- Cargo workspace version bump 0.1.0 → 1.0.0 (single source of truth via `workspace.package.version`; all 5 crates inherit through `version.workspace = true`).
- npm version bumps for `atlas-web`, `atlas-mcp-server`, `@atlas/bridge`, root monorepo manifest.
- `@atlas-trust/verify-wasm@1.0.0` build pipeline (`wasm-publish.yml`) auto-fires on signed-tag push to produce byte-identical `npm pack` tarballs (web + node targets) plus a `tarball-sha256.txt` manifest, uploaded to the GitHub Release as backup-channel assets per V1.15 Welle B. **Note (2026-05-12):** the npm-registry publish step for `v1.0.0` did not land due to a `Cargo.toml` repository-URL mismatch surfaced by npm's SLSA Build L3 provenance validator (see v1.0.1 entry). The `v1.0.0` Sigstore Rekor provenance attestation (logIndex `1510551161`, re-emitted as logIndex `1517641691` / `1517706827` across retry runs) was orphaned by the failed publish — it is content-addressed against the wasm bytes and remains audit-traceable. Consumers should install `@atlas-trust/verify-wasm@1.0.1` (or `@latest`) for the byte-identical trust property delivered through the npm registry.
- Signed Git tag `v1.0.0` via the V1.17 SSH-Ed25519 path (key `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`).

### Added — V1.19 Welle 12 (PR #48, commit cdf89e84)

- `--require-strict-chain` enabled in `apps/atlas-web/scripts/e2e-write-roundtrip.ts` round-trip (Welle 10 contract symmetric pair): atlas-web write surface now exercises the verifier-side single-writer-per-workspace gate end-to-end.
- New evidence-row + `Strict flags:`-anchored flag-name regex assertions in atlas-web e2e (mirror Welle 10 smoke.ts anti-drift pattern).
- New CLI integration test `crates/atlas-verify-cli/tests/strict_mode.rs::strict_chain_passes_linear_bank_trace` — happy-path coverage at the CLI surface on the 5-event linear bank-q1-2026 fixture.
- Public-API SemVer audit: new `docs/SEMVER-AUDIT-V1.0.md` documenting every public Rust type, CLI flag, HTTP wire shape, npm export, MCP tool, on-disk format, and operator env-var with risk-tag (Locked / Locked-Behind-Flag / Internal-but-Exported / Defer-Decision).
- This `CHANGELOG.md` consolidating the full V1.0-baseline through V1.19 Welle 12 ship history.

### Fixed — V1.19 Welle 12

- `atlas-web-playwright.yml` job ID renamed from `playwright` to `atlas-web-playwright` so the GitHub check-run name matches the master-ruleset `required_status_checks` context (Welle-11 fallout — GitHub derives check-run names from job ID, not workflow `name`). Pre-merge fixup commit `6040ee2`.
- `atlas-web-playwright.yml` paths filter extended with `.handoff/**` so docs-only PRs can trigger the required check (post-merge fixup commit `cb6b930`).

## Security Advisories

The following findings were discovered and remediated during Atlas's pre-1.0 development. Documented here for downstream CVE-prep workflows. Both findings were closed in-commit during the V1.19 Welle 9 review pass (2026-05-09, PR #42, commit e650f93); v1.0.0 is the first version with the documented audit trail.

### ATLAS-2026-001 (V1.19 Welle 9 SR-H-1): Empty-trace strict-chain silent pass

- **Severity:** HIGH (CVSS-equivalent: integrity / auditor-trust). Hypothetical severity for the vulnerability pattern; see "Affected" below — no public release was ever affected.
- **Affected:** none publicly. The bug existed only in a pre-push intermediate working-tree state of the V1.19 Welle 9 implementation; the fix landed in-commit with the flag's introduction (commit `e650f93`, PR #42, both the unsquashed push `41afebc` and the squash-merge `e650f93` already include the fix). No public release contains the unfixed pattern.
- **First safe version:** v1.0.0 (this release) is the first version with the documented audit trail. The flag itself shipped in V1.19 Welle 9 under v0.1.0 — already with the fix.
- **Description:** An earlier draft of `check_strict_chain` used the shape `if events.len() != 1 { ... }` which would have silently passed an empty trace under strict mode. Strict mode pins five properties including "non-empty"; without this, an attacker who stripped events from a bundle could pass strict mode silently.
- **Remediation:** `check_strict_chain` now returns `TrustError::StrictChainViolation` with the diagnostic "trace has no events (a linear chain requires at least 1 genesis event)" as the first check, before any per-event analysis.

### ATLAS-2026-002 (V1.19 Welle 9 SR-H-2): Self-reference 1-cycle bypass

- **Severity:** HIGH (CVSS-equivalent: integrity / auditor-trust). Hypothetical severity for the vulnerability pattern; see "Affected" below.
- **Affected:** none publicly. Same disposition as ATLAS-2026-001: the bug existed only in a pre-push intermediate working-tree state; both the unsquashed push `41afebc` and the squash-merge `e650f93` (PR #42) already include the fix.
- **First safe version:** v1.0.0 (this release) is the first version with the documented audit trail.
- **Description:** A 1-event trace where the event lists its own `event_hash` as a parent (cryptographically infeasible after a successful `check_event_hashes` pass under blake3 preimage resistance, but a defence-in-depth concern when `check_strict_chain` is called standalone) would have failed with a misleading "found 0 genesis events" message instead of the structured "self-reference cycle" diagnostic.
- **Remediation:** Self-reference check positioned FIRST among per-event checks in `check_strict_chain`, so a 1-event self-ref reports the cycle diagnostic correctly before the genesis-count check fires.

## [0.1.0] — pre-1.0 development history (2026-04-27 to 2026-05-11)

The v0.1.0 line represents Atlas's pre-1.0 development history across V1.0 baseline through V1.19 Welle 12. All entries below shipped under the v0.1.0 Cargo + npm version while features and trust properties were being assembled; v1.0.0 (above) is the first version with a frozen public-API contract per `docs/SEMVER-AUDIT-V1.0.md`.

### Added — V1.19 Welle 11 (PR #46, commit 8bc9d88)

- Playwright UI E2E coverage for `apps/atlas-web`: 19 tests × Chromium + Firefox = 38 cases. Three spec files: `tests/e2e/home.spec.ts` (4 cases, LiveVerifierPanel state-machine), `write.spec.ts` (11 cases, WriteNodeForm full happy-path + error-paths + persistence), `a11y.spec.ts` (4 cases, WCAG 2.1 Level AA + keyboard tab-order).
- WCAG 2.1 AA accessibility coverage via `@axe-core/playwright`.
- Frozen `data-testid` test seam: 10 identifiers on `WriteNodeForm.tsx` + 6 + dynamic pattern on `LiveVerifierPanel.tsx`, documented via JSDoc.
- New CI lane `.github/workflows/atlas-web-playwright.yml` (Ubuntu, Chromium + Firefox, paths-filtered) joined the master-ruleset required-check set.
- `role="alert"` on error display, `role="status"` on success card, `aria-hidden="true"` on decorative ✓/✗ glyphs.
- New `--accent-trust-brand` color-token alias preserving the original sigstore-green `#3fbc78` for non-text branding surfaces.

### Fixed — V1.19 Welle 11

- Five color tokens in `apps/atlas-web/src/app/globals.css` corrected for WCAG 2.1 AA contrast on `bg-muted` and on the 15%-mix StatusBadge background: `--foreground-muted = #475569`, `--accent-trust = #166534` (green-800; buffered for Firefox `color-mix()` gamma rounding), `--accent-warn = #b45309`, `--accent-danger = #b91c1c`, `--accent-info = #1d4ed8`.

## V1.19 Welle 10 — 2026-05-11 (PR #44, commit 1e3e89f)

### Added

- `--require-strict-chain` enabled in `apps/atlas-mcp-server` smoke (step 6 + step 7). Single-writer-per-workspace CI gate active across three lanes: `hsm-wave3-smoke.yml`, `sigstore-rekor-nightly.yml`, local `pnpm smoke`.
- Anti-drift assertions in smoke.ts: evidence-row pin matching `/✓ strict-chain — \d+ event\(s\) form a strict linear chain/`, `Strict flags:`-anchored flag-name pins (`/Strict flags:[^\n]*require_strict_chain/`).
- Step 7 augmented with strict-chain alongside existing `--require-per-tenant-keys`.

### Fixed

- Property numbering in step-7 rationale comment corrected to match the canonical `crates/atlas-trust-core/src/hashchain.rs::check_strict_chain` doc-comment (property 2 = "exactly one genesis"; prior draft used "(3)" which was wrong).

## V1.19 Welle 9 — 2026-05-09 (PR #42, commit e650f93)

### Added

- Verifier-side `--require-strict-chain` opt-in flag on `atlas-verify-cli` and `VerifyOptions::require_strict_chain` on the library surface.
- `crates/atlas-trust-core::hashchain::check_strict_chain` free function pinning five properties: trace non-empty, exactly one genesis, every non-genesis has exactly one parent, no event referenced as parent by more than one other event (no sibling-fork), no event lists its own hash as parent (no self-reference).
- New `TrustError::StrictChainViolation { msg }` variant (under existing `#[non_exhaustive]`) for auditor tooling pattern-matching.
- 9 hashchain strict-chain unit tests covering empty-trace, single-genesis, two-event-linear, linear-three-events, two-genesis, zero-genesis, sibling-fork, DAG-merge, self-reference.

### Security

- SR-H-1 (empty trace silently passed strict-chain) — closed in-commit with structured `StrictChainViolation` diagnostic.
- SR-H-2 (1-event self-referential event_hash bypassed property-2 check) — closed by positioning self-reference check FIRST in `check_strict_chain`.
- CR-1 (strict-chain over preflight-failed graph could mislead) — gated on `event_hashes_ok && parent_links_ok`; explicit "skipped" evidence row otherwise.
- CR-2 (`Result<(), String>` deviated from module convention) — refactored to `TrustResult<()>`.

## V1.19 Welle 8 — 2026-05-09 (PR #40, commit 1d1fe69)

### Added

- atlas-web write-surface HTTP-level edge-case test suite: 42 assertions across `scripts/e2e-write-edge-cases.ts`. Four classes covered: (A) 4xx malformed-input rejections (Zod `.strict()`, prototype pollution, deeply-nested attributes); (B) Content-Length 256 KB cap → 413; (C) per-workspace mutex serialisation under 8 parallel POSTs; (D) workspace_id boundary class (POSIX/Windows traversal, embedded delimiters, length 0/129, GET endpoint mirror).
- `__REQUEST_BODY_MAX_BYTES_FOR_TEST` export on `apps/atlas-web/src/app/api/atlas/write-node/route.ts` for source/test drift prevention.

### Security

- FINDING-6 (chain-validation oracle used set-membership; would silently accept sibling-fork DAG) — hardened to immediate-predecessor comparison (`parents[0] === stored[i-1].event_hash`), the same regression mode Welle 9 + Welle 10 now also catch at the verifier and CI-lane surfaces.

## V1.19 Welle 7 — 2026-05-09 (PR #38, commit 19995ed)

### Added

- Shared `PATH_SEGMENT` + `POSIX_PATH_LOOKBEHIND` constants on `@atlas/bridge/src/signer.ts`, re-exported via the frozen `__redactPathConstantsForTest` test seam.

### Fixed

- Source/test drift hazard on the `redactPaths` POSIX regex — the test now imports the constants instead of redefining literals, with `Object.isFrozen` + 2 exact-equality golden assertions pinning the contract.

## V1.19 Welle 6 — 2026-05-09 (PR #36, commit 6d99012)

### Fixed

- `redactPaths` POSIX lookbehind tightened: dotted-relative paths (`./foo/bar.ts`, `../workspace/x`) now pass through verbatim — they expose only repo-internal filenames, outside the absolute-layout-disclosure threat model. Absolute paths containing dotfile segments (`/home/user/.cache/foo`) MUST still redact.

## V1.19 Welle 5 — 2026-05-09 (PR #34, commit 2c1f6f2)

### Changed

- `@atlas/bridge::ulid` refactored to pure-function + factory + singleton trio: `nextUlid(state, now, randomSource)` is pure, `createUlid({ now, randomSource })` produces a factory, `ulid()` is the singleton backward-compat wrapper. Closes the immutability convention violation in the prior implementation.

### Added

- 25 ulid contract assertions across 7 sections (purity, monotonicity, clock-advance reset, factory isolation, ms-collision, Crockford-base32 sortability, byte-rollover guard, boundary guards).

## V1.19 Welle 4 — 2026-05-09 (PR #32, commit aefde84)

### Added

- 60-second TTL cache for `resolveSignerBinary()` resolution. cwd-drift hardening: cache key includes `process.cwd()` so a `chdir` invalidates the entry.
- 12 signer-cache test assertions using synthetic clock injection via `__signerBinaryCacheForTest.setClock`.

## V1.19 Welle 3 — 2026-05-08 (PR #30, commit 02327193)

### Fixed

- `redactPaths` POSIX path-pattern tightened against false positives (URLs, fractions, dates).
- `storage.ts` duplicate definition collapsed.

## V1.19 Welle 2 — 2026-05-08 (PR #28, commit 2f726f3)

### Added

- New workspace package `packages/atlas-bridge/` (`@atlas/bridge`) extracted from inline atlas-mcp-server / atlas-web bridge code. Single source of truth for the TS-to-Rust-signer bridge plus on-disk JSONL DAG.

### Changed

- Bridge `package.json` deliberately has NO `"source"` export — consumers always resolve via `dist/`. CI runs `pnpm --filter @atlas/bridge build` before consumer tsc.

## V1.19 Welle 1 — 2026-05-08 (PR #26, commit 3853c64)

### Added

- atlas-web write surface: `POST /api/atlas/write-node` (Zod `.strict()` validation, per-workspace mutex, atlas-signer subprocess for per-tenant signing) + `GET /api/atlas/write-node?workspace_id=…` for kid-preview.
- `apps/atlas-web/scripts/e2e-write-roundtrip.ts` — end-to-end round-trip from Request → JSONL → atlas-verify-cli `--require-per-tenant-keys` → ✓ VALID.

## V1.18 (2026-04 / -05) — Defence-in-Depth Trust Posture

### Added

- Welle A: trust-root mutation pin (`tools/verify-trust-root-mutations.sh`, 17 cases, 18 PROTECTED_SURFACE paths via CODEOWNERS).
- Welle B (1–8): SSH-Ed25519 commit + tag signing pipeline (`tools/test-tag-signatures.sh`, 13 cases). Repository Rulesets with required status checks. Master ruleset migrated from classical branch protection.

## V1.17 — SSH-Ed25519 Tag Signing

### Added

- SSH-Ed25519 signing pathway for tags (key `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`). GitHub Repository Rulesets with required signed commits.

## V1.16 — Production Hosting

### Added

- Welle C: Cloudflare Workers hosting for `playground.atlas-trust.dev`. CSP + COEP/COOP headers (`tools/playground-csp-check.sh`). Worker-emitted headers + silent-204 receiver pattern (ADR-007).

## V1.14 — Witness Wave-C JSON Surface

### Added

- Scope J: `VerifyOutcome.witness_failures: Vec<WitnessFailureWire>` with `#[serde(default)]` for additive wire compat. Per-witness stable `reason_code` for auditor tooling.

## V1.13 — Witness Cosignature Attestation

### Added

- `crates/atlas-witness` binary. `WitnessSig` type, `ATLAS_WITNESS_V1_ROSTER` pinned roster.
- `--require-witness <N>` flag on atlas-verify-cli. Threshold-based witness coverage check (kid-distinct verified Ed25519 signatures across `anchor_chain`).
- `TrustError::BadWitness` variant; duplicate-kid defence.

## V1.12 — Wave-3 Sealed-Per-Workspace Signer

### Added

- atlas-signer wave-3 dispatch: sealed-per-workspace keys via PKCS#11 v3.0. `ATLAS_HSM_WAVE3_OPT_IN` env-var opt-in. Three-layer dispatcher (dev-seed → wave-2 master-HKDF → wave-3 sealed-per-workspace).
- CI lane `.github/workflows/hsm-wave3-smoke.yml` (SoftHSM2-backed).

## V1.11 — Sigstore Rekor V1 Public-Trust Anchor

### Added

- Sigstore Rekor v1 verification path with multi-issuer support. `crates/atlas-trust-core::anchor::SIGSTORE_REKOR_V1.tree_id_roster`. ECDSA P-256 over RFC 6962 SHA-256 inclusion proofs.
- `.github/workflows/sigstore-rekor-nightly.yml` nightly live-Sigstore lane.

## V1.10 — Strict-Mode Surface

### Added

- Wave 1: `--require-per-tenant-keys`, `--require-anchors`, `--require-anchor-chain` on atlas-verify-cli. `VerifyOptions` struct surface.
- Wave 2: `crates/atlas-signer/src/hsm/` PKCS#11 v3.0 master-HKDF backend.

## V1.9 — Per-Tenant Kid Derivation

### Added

- HKDF-SHA256 per-tenant Ed25519 key derivation from a single master seed (info string `"atlas-anchor-v1:" + workspace_id`).
- `PER_TENANT_KID_PREFIX = "atlas-anchor:"` constant. `perTenantKidFor`, `parse_per_tenant_kid` helpers.
- `ATLAS_DEV_MASTER_SEED` env-var positive opt-in.

## V1.7 — Anchor-Chain Linkage

### Added

- `AnchorChain` type with internal-consistency verification. `chain_head_for` + `ANCHOR_CHAIN_DOMAIN` constants. `crates/atlas-trust-core::anchor` module.
- `--require-anchor-chain` strict-mode flag.

## V1.6 — Sigstore Rekor Compatibility

### Added

- p256 + sha2 dependencies for ECDSA P-256 over RFC 6962 SHA-256 (Rekor checkpoint signatures).

## V1.5 — Anchor Inclusion Proofs

### Added

- `AnchorEntry`, `AnchorBatch` wire-format types. `--require-anchors` strict-mode flag.

## V1.0 baseline through V1.4

Pre-V1.5 foundations: trace_format (`AtlasEvent`, `AtlasTrace`, `PubkeyBundle`), hashchain (event_hash recompute, parent_links, DAG-tips computation), COSE_Sign1 + ed25519-dalek signing, Zod-schema validation at trust boundaries, JSONL append-only storage.

---

[Unreleased]: https://github.com/ThePyth0nKid/atlas/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/ThePyth0nKid/atlas/releases/tag/v1.0.0
[0.1.0]: https://github.com/ThePyth0nKid/atlas/commits/master
