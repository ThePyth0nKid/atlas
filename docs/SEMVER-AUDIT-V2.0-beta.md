# SemVer Audit — Atlas v2.0.0-beta.1 Public-API Surface (V2-β-1 Additive)

> **Status:** V2-β Welle 19 deliverable, finalised 2026-05-15. **Companion** doc to [`docs/SEMVER-AUDIT-V1.0.md`](SEMVER-AUDIT-V1.0.md) (which covers V1 baseline + V2-α additive surface in its §10). This doc documents the V2-β-1 ADDITIVE surface — items new in `v2.0.0-beta.1` that did NOT exist in `v2.0.0-alpha.2`.
>
> **Reading order:** read V1.0 §1-§9 for V1 baseline → V1.0 §10 for V2-α additive surface → this doc for V2-β-1 additive surface. Future V2-β.N / V2-γ wellen will continue appending to a successor doc without rewriting V1.0 §1-§9 or this V2.0-beta doc.
>
> **Methodology:** mirrors V1.0 §1-§9 risk-tag framework. Each new public surface item gets one of: **Locked** / **Locked-Behind-Flag** / **Internal** / **Defer-Decision**.

## Tag legend (V2-β-1 alignment)

| Tag | Meaning under V2-β-1 |
|---|---|
| **Locked** | V2-β-1 stable contract. Any breaking change → SemVer-major bump (`3.0.0`-class). Adding fields under `#[non_exhaustive]` enums or with `#[serde(default)]` is SemVer-minor. |
| **Locked-Behind-Flag** | Opt-in feature flag (e.g. `lancedb-backend`). Default behaviour is stable; flag turns on additional surface. Adding new flags = SemVer-minor. |
| **Internal** | Deferred-to-V2-γ stability. Surface MAY shift in `2.0.0-beta.2` / `2.0.0-rc.*` / `2.1.x` without SemVer-major bump. Documented here for cross-doc traceability; consumers reaching for these accept churn. |
| **Defer-Decision** | V2-β-1 commitment deferred. Covered by stability disclaimer in code/docs or scoped to a future minor. |

---

## 1. `atlas-mem0g` crate (NEW workspace member)

> Crate-version `0.1.0` independent of workspace `2.0.0-beta.1` — Layer 3 crate's public-API surface is **Internal** during W18c parallel-track per the `AtlasEmbedder` tag below. Once W18c Phase B + Phase D land, the crate version will join the workspace lockstep.

### 1.1 Public types from `crates/atlas-mem0g/src/lib.rs`

| Item | Tag | Notes |
|---|---|---|
| `pub trait SemanticCacheBackend: Send + Sync` | **Locked** | Object-safe trait per spike §7. Sync methods (mirrors Layer-2 `GraphStateBackend` convention) for `tokio::task::spawn_blocking` wrapping — explicitly NOT `Handle::current().block_on()` (deadlocks under single-threaded scheduler; see W18b ADR + Lessons #16-17). Methods: `upsert`, `search`, `erase`, `invalidate`. Adding methods is SemVer-major; adding default-method-bodies is SemVer-minor. |
| `pub struct SemanticHit { event_uuid: String, workspace_id: String, entity_uuid: Option<String>, score: f32, snippet: String }` | **Locked** | `event_uuid` is ALWAYS present — cite-back trust property. `entity_uuid` is `Option` to support pre-entity events. `#[non_exhaustive]` allows future field additions. |
| `pub enum Mem0gError` (with `#[non_exhaustive]`) — variants: `Io(std::io::Error)`, `Embedder(String)`, `SupplyChainMismatch { expected, actual }`, `SecureDelete { step, reason }`, `Backend(String)`, `InvalidWorkspaceId(String)` | **Locked** | `#[non_exhaustive]` makes adding variants SemVer-minor. Auditor tooling switches on variant discriminant, not text. Removing or renaming any current variant = SemVer-major. |
| `pub struct InvalidationPolicy { ttl: Option<Duration>, on_event: bool, on_layer1_head_divergence: bool }` | **Locked** | TTL + on-event + Layer-1-head-divergence triple per ADR-Atlas-012 §4 sub-decision #6. `#[non_exhaustive]` allows future fields. Default = all three off (`Default::default()`). |
| `pub type Mem0gResult<T> = Result<T, Mem0gError>` | **Locked** | |
| Re-exports `check_workspace_id`, `check_value_depth_and_size` from `atlas-projector` | **Locked** | Boundary-defence helpers; consumers MUST call at every backend-boundary `serde_json::Value` parse per ADR-Atlas-012 §4 sub-decision #1. |

### 1.2 `pub struct AtlasEmbedder` (from `crates/atlas-mem0g/src/embedder.rs`)

| Item | Tag | Notes |
|---|---|---|
| `pub struct AtlasEmbedder` | **Internal** | **Deferred to V2-γ stability.** Fail-closed pending W18c Phase B `fastembed::TextEmbedding::try_new_from_user_defined` wiring. Internal API surface MAY shift in `2.0.0-beta.2` without SemVer-major. |
| `pub const HF_REVISION_SHA: &str` (40-char SHA-1) | **Locked** (as the supply-chain pin contract) | Pins HuggingFace `BAAI/bge-small-en-v1.5` revision `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`. Changing this constant is operationally **NOT a SemVer event** — it is a supply-chain rotation that requires a new SEMVER-AUDIT-V2.0-beta entry + Phase A re-execution per `tools/w18c-phase-a-resolve.sh`. The CONSTANT NAME + format (40-char lowercase hex) is Locked. |
| `pub const ONNX_SHA256: &str` (64-char SHA-256) | **Locked** (supply-chain pin contract) | SHA-256 of `model.onnx` (133,093,490 bytes FP32). Same rotation policy as HF_REVISION_SHA. |
| `pub const MODEL_URL: &str` | **Locked** (supply-chain pin contract) | Revision-pinned huggingface.co LFS URL. `https_only(true)` TLS pin. Same rotation policy as HF_REVISION_SHA. |
| `pub const TOKENIZER_JSON_SHA256`, `CONFIG_JSON_SHA256`, `SPECIAL_TOKENS_MAP_SHA256` (W18c-NEW) | **Locked** (supply-chain pin contract) | SHA-256 of the 3 tokenizer companion files. Consumed by W18c Phase B `try_new_from_user_defined` wiring. |
| `pub const TOKENIZER_JSON_URL`, `CONFIG_JSON_URL`, `SPECIAL_TOKENS_MAP_URL` (W18c-NEW) | **Locked** (supply-chain pin contract) | Revision-pinned LFS URLs for the 3 tokenizer files. |

**Atomic-lift contract:** all 9 compile-in pins (1 × SHA-1: `HF_REVISION_SHA`; 4 × SHA-256: `ONNX_SHA256` + `TOKENIZER_JSON_SHA256` + `CONFIG_JSON_SHA256` + `SPECIAL_TOKENS_MAP_SHA256`; 4 × URL: `MODEL_URL` + `TOKENIZER_JSON_URL` + `CONFIG_JSON_URL` + `SPECIAL_TOKENS_MAP_URL`) lift atomically per Phase A. The test `pins_well_formed_after_lift` enforces all-or-nothing.

### 1.3 `pub struct LanceDbCacheBackend` (from `crates/atlas-mem0g/src/lancedb_backend.rs`)

| Item | Tag | Notes |
|---|---|---|
| `pub struct LanceDbCacheBackend` | **Locked-Behind-Flag** `lancedb-backend` | Feature default-OFF. Consumer opts in via `atlas-mem0g = { version = "0.1", features = ["lancedb-backend"] }` in their Cargo.toml. Gates ~200 transitive crates (Arrow + DataFusion). Adding feature-flagged surface = SemVer-minor. |
| `impl SemanticCacheBackend for LanceDbCacheBackend` | **Locked-Behind-Flag** `lancedb-backend` | Trait-impl plumbs through `secure_delete` for erasure + uses `precapture_fragments` / `precapture_indices` helpers (depth-recursive `walk_collect_filtered`). Body sites surface `Mem0gError::Backend("not yet wired")` placeholders until W18c Phase D lifts. The IMPL EXISTS; the bodies are placeholders. |

### 1.4 Secure-delete protocol (from `crates/atlas-mem0g/src/secure_delete.rs`)

| Item | Tag | Notes |
|---|---|---|
| 7-step pre-capture-then-lock-then-overwrite protocol | **Locked** (as protocol semantics) | Per ADR-Atlas-012 §4 sub-decision #4. Sequence: ACQUIRE per-`(workspace, table)` `tokio::sync::RwLock` write lock → PRE-CAPTURE fragment paths → `lancedb::Table::delete()` → `cleanup_old_versions(Duration::ZERO)` → PRE-CAPTURE HNSW `_indices/` paths → OVERWRITE each pre-captured path (random bytes equal to file size + `fdatasync` + `remove_file`) → RELEASE lock → emit `embedding_erased` audit-event OUTSIDE the lock. Adding steps or changing order = SemVer-major (auditor-observable). |
| `PerTableLockMap`, `PreCapturedPaths` helper types | **Internal** | Implementation detail; not consumer-facing. |
| OS CSPRNG (`getrandom::getrandom`) for overwrite bytes | **Locked** (security contract) | Replaces W18b's deterministic blake3-seeded path (HIGH-4 reviewer fix). Non-replayable by adversary with workspace storage layout. |

---

## 2. `atlas-projector` additions for V2-β-1

> Building on V2-α surface (§10.7a-e in SEMVER-AUDIT-V1.0.md), V2-β-1 adds W17a/b/c backend abstraction + W18b embedding_erased dispatch.

### 2.1 `pub trait GraphStateBackend` (W17a)

| Item | Tag | Notes |
|---|---|---|
| `pub trait GraphStateBackend: Send + Sync` | **Locked** | Object-safe trait abstracting state-mutation surface (upsert_node, upsert_edge, upsert_anchor, upsert_embedding_erasure, etc.). Sync methods (consistent with `SemanticCacheBackend` design). Adding methods = SemVer-major; adding default-method-bodies = SemVer-minor. |
| `pub struct InMemoryBackend` | **Locked** | Default impl using `GraphState` directly. Carries forward V2-α byte-pin invariant. |
| `pub struct ArcadeDbBackend` | **Locked** (Apache-2.0 ArcadeDB embedded mode) | W17b deliverable. Cypher-validator consolidation (W15) reused. ArcadeDB JNI driver wrapping. Cross-backend `backend_trait_conformance` test pins byte-determinism. |
| `BackendError` enum (with `#[non_exhaustive]`) | **Locked (SemVer-minor under non_exhaustive)** | Variants for path-resolution / driver-init / Cypher-validation failures. |

### 2.2 `embedding_erased` event-kind dispatch arm (W18b)

| Item | Tag | Notes |
|---|---|---|
| `apply_embedding_erased` dispatch arm in `crates/atlas-projector/src/upsert.rs` | **Locked** | Payload contract: `event_id` + `workspace_id` + `erased_at` (REQUIRED, empty-string-guarded per MEDIUM-1 reviewer fix); `requestor_did` + `reason_code` (OPTIONAL, default `"operator_purge"`). Mirrors `apply_anchor_created` structure exactly. Append-only refusal of duplicates via existing `MissingPayloadField` variant pattern (semantic-mismatch doc-comment per ADR-Atlas-012 §4 sub-decision #5; dedicated `DuplicateErasureRefused` variant is V2-γ-deferred). |
| `pub struct EmbeddingErasureEntry { workspace_id, erased_at, requestor_did, reason_code, event_uuid, author_did }` | **Locked** (`#[non_exhaustive]`) | Per-erasure state-projection entry. |
| `GraphState.embedding_erasures: BTreeMap<String, EmbeddingErasureEntry>` field | **Locked** | New field on `GraphState`; canonical-bytes omits-when-empty preserving V14 byte-pin invariant (key invariant per Phase 14.5 cross-doc consolidation). Analog `rekor_anchors` field convention. |
| `GraphState::upsert_embedding_erasure` helper | **Locked** | |

### 2.3 Byte-determinism CI gates

| Item | Tag | Notes |
|---|---|---|
| `crates/atlas-projector/tests/backend_trait_conformance::byte_pin` | **CI gate** | Cross-backend invariant: byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduces through BOTH `InMemoryBackend` AND `ArcadeDbBackend`. This is THE V2-β-1 storage-abstraction byte-pin contract. |

---

## 3. `atlas-web` API additions for V2-β-1

### 3.1 `POST /api/atlas/semantic-search` Read-API endpoint (W18b)

| Item | Tag | Notes |
|---|---|---|
| `POST /api/atlas/semantic-search` route | **Locked** (contract; 501 scaffold-response behaviour stable across V2-β minor versions) | Request schema (Zod, `.strict()`): `workspace_id` (required) + `query_text` (required) + `top_k` (default 10, max 100) + `min_score` (default 0.0). Response schema: `{ ok: true, hits: SemanticHit[] }` on success; `{ ok: false, error: string, code: "MEM0G_SCAFFOLD" }` on 501 scaffold-response. W18c Phase B wires the body without changing the request/response contract — consumers see `code: "MEM0G_SCAFFOLD"` flip to actual results without breaking. |
| 256 KB request body cap → 413 (mirrors V1 write-node convention) | **Locked** | |
| `redactPaths` wrapping of any signer/embedder stderr on 5xx response (mirrors §2.1.2 in V1.0 audit) | **Locked** (security contract; caller-responsibility surface) | New callsite without `redactPaths` = security regression per V1.0 §2.1.2 convention. |

### 3.2 Existing Read-API endpoints (W12)

These were SHIPPED in V2-α-α.2's lineage but their public-API contract is reaffirmed under V2-β-1's `[2.0.0-beta.1]` ship as part of CHANGELOG `[Unreleased] → [2.0.0-beta.1]` conversion. See V2-MASTER-PLAN.md §5.4 for the full Read-API endpoint inventory; this V2-β-1 audit reaffirms `POST /api/atlas/semantic-search` as the NEW 6th endpoint.

---

## 4. CI workflow additions for V2-β-1

| Workflow | Tag | Notes |
|---|---|---|
| `.github/workflows/atlas-mem0g-smoke.yml` | **Internal** | Not a required-check yet. Promoted to required after ≥3 stable PR runs per operator-runbook V2-β chapter. |
| `.github/workflows/atlas-arcadedb-smoke.yml` (W17c-shipped) | **Internal** | Same promotion-gate policy. Smoke run validates ArcadeDB driver + compose-up + byte-pin reproduction. |
| `.github/workflows/wasm-publish.yml` | **Locked** (race-fix from W11; load-bearing) | V2-β-1 ship REUSES the W11 race-fix unchanged. Touching this workflow requires a separate welle per W19 in-scope-files rule. |

---

## 5. Verifier-WASM additions vs V2-α-α.2

Surface changes minimal. V2-α-α.1 → V2-α-α.2 → V2-β-1 deltas are largely additive (Layer 2 + Layer 3 scaffolds; no V1 trust surface changes). Refer V2-α-1 + V2-α-2 release notes for V1 → V2-α delta narrative.

| Item | Tag | Notes |
|---|---|---|
| `verify_trace_json(trace, bundle) -> Result<JsValue, JsValue>` wasm-bindgen export | **Locked** | Unchanged from V1 baseline. V2-β-1 verifier accepts V1 + V2-α + V2-β-1 events; rejects V1.0 verifiers reading V2-α / V2-β-1 events via `deny_unknown_fields` (by-design wire-break, SemVer-major commitment from v2.0.0-alpha.1). |
| `verifier_version() -> String` wasm-bindgen export | **Locked** | Returns `atlas-trust-core/2.0.0-beta.1`. Format `crate-name/semver` — auditor tooling switches on this. |
| `@atlas-trust/verify-wasm@2.0.0-beta.1` npm publish | **Locked** | Default dist-tag `latest` per V2-α-α.2 precedent. `node` dist-tag for Node.js CommonJS consumers (unchanged from V2-α-α.2). |

---

## 6. Breaking-change-relevant findings summary

After surveying §1-§5, **no items require a breaking change before v2.0.0-beta.1 ships**. The audit recommends shipping `v2.0.0-beta.1` with the surface as-is. Three items are documented as `Internal` and are NOT V2-β-1 SemVer commitments:

1. **`AtlasEmbedder`** — fails-closed; surface MAY shift in `2.0.0-beta.2` once W18c Phase B `try_new_from_user_defined` wiring lands. Not a public-API commitment yet.
2. **`atlas-mem0g` crate at version `0.1.0`** — joins workspace lockstep once W18c Phase D lifts. Until then, the crate's surface is opt-in via Cargo dep.
3. **`atlas-mem0g-smoke.yml` + `atlas-arcadedb-smoke.yml` CI workflows** — promoted to required-check after ≥3 stable PR runs per operator-runbook.

---

## 7. V2-β post-1 SemVer policy

**v2.0.0-beta.N minor/patch bumps:**
- New `Mem0gError` variants (under `#[non_exhaustive]`) — SemVer-minor
- New `SemanticHit` / `InvalidationPolicy` fields (under `#[non_exhaustive]`) — SemVer-minor
- New `BackendError` variants — SemVer-minor
- New event-kind dispatch arms (analog `apply_embedding_erased`) — SemVer-minor
- New Read-API endpoints — SemVer-minor
- W18c Phase B body fill-in of `AtlasEmbedder` — SemVer-minor (Internal surface → Locked transition is observable but NOT a break; consumers opting in see fail-closed → operational flip)
- W18c Phase D body fill-in of `LanceDbCacheBackend` — SemVer-minor (Locked-Behind-Flag stays Locked-Behind-Flag; placeholder → operational flip)

**v2.0.0-beta.N patch bumps:**
- Internal refactors with no public-API impact
- Performance improvements with identical outputs
- Documentation fixes
- Error-message text rewording (auditor tooling switches on variant discriminant)

**v3.0.0 major bumps (planned scope, not v2.x):**
- Removing or renaming any **Locked** item above
- Changing `SemanticCacheBackend` trait method signatures
- Changing `embedding_erased` payload contract
- Removing any operator env-var
- Schema-version-bump on `atlas-projector-v1-alpha` → `v2`

---

## 8. Audit governance

After v2.0.0-beta.1 ships, any PR adding/removing exports on the V2-β-1 surfaces listed in §1-§5 MUST update this document in the same commit. Enforced by reviewer convention; not gated by CI (consistent with V1.0 §9 policy).

Reviewer checklist for V2-β-1-surface-touching PRs:
- [ ] Identified the surface category (§1-§5) the change touches
- [ ] Confirmed the change is additive (SemVer-minor) or breaking (SemVer-major / `3.0.0`-class)
- [ ] Updated `docs/SEMVER-AUDIT-V2.0-beta.md` (this doc) with the new item + tag
- [ ] Updated `CHANGELOG.md` under the appropriate `[Unreleased]` heading
- [ ] If `Internal → Locked` transition (W18c Phase B/D): confirmed companion test surface + documented in the affected section header

---

**Audit author:** V2-β Welle 19 implementer.
**Audit reviewer:** parallel `code-reviewer` + `security-reviewer` agents (parent-led dispatch).
**Effective date:** finalised at v2.0.0-beta.1 tag (V2-β Welle 19 ship).
**Companion docs:** [`docs/SEMVER-AUDIT-V1.0.md`](SEMVER-AUDIT-V1.0.md) (V1 baseline + V2-α additive) + [`docs/V2-BETA-1-RELEASE-NOTES.md`](V2-BETA-1-RELEASE-NOTES.md) (V2-β-1 release content).
