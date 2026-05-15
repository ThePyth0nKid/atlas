# V2-β Welle 18c — Plan-Doc (Mem0g operational activation — parallel-track to W19)

> **Status:** Phase A SHIPPED 2026-05-15 (Nelson HF resolve `5c38ec7c…` via `tools/w18c-phase-a-resolve.sh` + agent constant-lift; 6 SHAs + 4 URLs compiled-in; W18b gatekeeper test retired; embedder still fails-closed pending Phase B). Phase B-D pending dispatch.
> **Welle:** W18c (V2-β parallel-track, NOT in numbered phase sequence). Pre-V2-β-1-ship-OPERATIONAL blocker; NOT W19 ship gate.
> **Orchestration:** SEMI-MANUAL (Phase A = Nelson HuggingFace verification, ~30 min Nelson + ~10 min agent) + SERIAL subagent (Phase B-D = engineering, ~3-4 sessions).
> **Driving decisions:** `DECISION-ARCH-W18b` (W18b shipped scaffold with `TODO_W18B_NELSON_VERIFY_*` placeholders + fail-closed `AtlasEmbedder::new`); ADR-Atlas-012 §4 sub-decision #2 (supply-chain controls — three Atlas-source-pinned `const` values); spike §12 verification gaps V1-V4 (LanceDB Windows behaviour + fastembed-rs cross-platform determinism + Lance v2.2 `_deletion_files` + model size).

W18c is the activation welle that lifts the W18b-deferred items: real supply-chain constants from HuggingFace `BAAI/bge-small-en-v1.5` + fastembed `try_new_from_user_defined` wiring + close V1-V4 verification gaps + lift LanceDB ANN/search body stubs. None of these block the W19 v2.0.0-beta.1 tag itself (W19 ships honest-scaffold-posture); ALL are pre-customer-deployment blockers for operational Layer 3.

**Why this as W18c (not W18d, not part of W19):** the W18b implementation shipped intentionally as W17a-pattern Phase-A-scaffold per the V1-V4 + supply-chain dependency structure (Atlas Lesson #16). W18c is the corresponding W17b-equivalent for Mem0g. W19 is the ship convergence — it's correctly orthogonal to operational activation. Numbering W18c rather than W19a / W20 because the unit-of-work is "complete the W18b scaffold," not a new chapter.

## Scope (table)

| In-Scope | Out-of-Scope |
|---|---|
| Phase A: Nelson supply-chain constant lift (`HF_REVISION_SHA` + `ONNX_SHA256` + `MODEL_URL`) | Mem0g multi-region replication (V2-γ; spike §11 OQ-2) |
| Phase B: fastembed `try_new_from_user_defined` wiring + 3 tokenizer-file SHA-256 pins | Per-tenant cryptographic-erasure (V2-γ; ADR-012 §6 OQ-1) |
| Phase C: V1-V4 verification gap closure (cross-platform CI matrix Linux + Windows + macOS) | Embedder-version-rotation policy (V2-γ operator-runbook) |
| Phase D: LanceDB ANN/search body fill-in (replace `Mem0gError::Backend("not yet wired")` placeholders) | Public-API surface changes (Layer 3 trait surface stays SemVer-stable) |
| Operator-runbook update for new Layer-3 surfaces | New event-kinds (the `embedding_erased` arm is W18b-shipped; W18c uses it) |
| ADR-Atlas-013 (RESERVED — only if implementation surfaces design amendment) | New CI required-checks (`atlas-mem0g-smoke` promotion to required is a separate operator-runbook decision; not W18c scope) |

## Phase A — Nelson supply-chain constant lift (~30 min Nelson + ~10 min agent)

**Goal:** replace the three `TODO_W18B_NELSON_VERIFY_*` placeholders in `crates/atlas-mem0g/src/embedder.rs` with real values pinned to a specific HuggingFace revision of `BAAI/bge-small-en-v1.5`.

**Pre-conditions:** none. Can happen anytime after W18b ship.

**Step 1 — Nelson resolves HuggingFace revision SHA:**

```bash
# Get the latest commit SHA on main of BAAI/bge-small-en-v1.5 from HuggingFace.
# Atlas pins THIS SHA forever; rotations happen via explicit Atlas release.
curl -s https://huggingface.co/api/models/BAAI/bge-small-en-v1.5 | python -c "import json,sys; d=json.load(sys.stdin); print(d['sha'])"

# Expected output: a 40-char hex string. THIS is HF_REVISION_SHA.
# Example (illustrative; real value as of fetch time):
#   5c38ec7c405ec4b44b94cc5a9bb96e735b38267a
```

**Step 2 — Nelson resolves ONNX file SHA-256:**

```bash
# Atlas's chosen ONNX file is bge-small-en-v1.5/onnx/model.onnx FP32.
# Fetch by revision SHA + sha256sum the bytes.
REV="<from step 1>"
curl -sL "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/$REV/onnx/model.onnx" -o /tmp/model.onnx
sha256sum /tmp/model.onnx
# Expected output: 64-char hex string. THIS is ONNX_SHA256.
# Also note file size: `ls -la /tmp/model.onnx` — this is V4 verification (~130 MB per spike §3.4).
```

**Step 3 — Nelson resolves MODEL_URL:**

```text
Pin the LFS URL incl. revision SHA in path.
Format:
  https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/<HF_REVISION_SHA>/onnx/model.onnx

Substitute the real revision SHA from step 1.
```

**Step 4 — Nelson resolves 3 tokenizer-file SHA-256s** (required for Phase B fastembed wiring):

```bash
REV="<from step 1>"
for f in tokenizer.json config.json special_tokens_map.json; do
  curl -sL "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/$REV/$f" -o "/tmp/$f"
  echo "$f: $(sha256sum /tmp/$f | cut -d' ' -f1)"
done
# Capture three 64-char hex strings for TOKENIZER_JSON_SHA256 + CONFIG_JSON_SHA256 + SPECIAL_TOKENS_MAP_SHA256.
```

**Step 5 — Agent updates the three (plus three Phase-B) consts:**

Edit `crates/atlas-mem0g/src/embedder.rs`:

```rust
// Lines ~61-82 (the three TODO_W18B_NELSON_VERIFY_* consts)
pub const HF_REVISION_SHA: &str = "<from step 1>";
pub const ONNX_SHA256: &str = "<from step 2>";
pub const MODEL_URL: &str = "https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/<HF_REVISION_SHA>/onnx/model.onnx";

// NEW for Phase B (W18c adds these — currently NOT in W18b's embedder.rs):
pub const TOKENIZER_JSON_SHA256: &str = "<from step 4>";
pub const CONFIG_JSON_SHA256: &str = "<from step 4>";
pub const SPECIAL_TOKENS_MAP_SHA256: &str = "<from step 4>";
```

**Step 6 — Watch the gatekeeper test FAIL as designed (HIGH-3 reviewer-fix contract):**

```bash
cargo test -p atlas-mem0g pins_are_placeholder_until_nelson_verifies
# Expected: FAIL — assertions `starts_with("TODO_W18B")` no longer hold.
# This is the W18b-shipped gatekeeper firing intentionally. Confirms the lift.
```

**Step 7 — Update the gatekeeper test for post-lift well-formedness:**

The W18b-shipped test has a companion "post-lift well-formedness" check. After lifting constants, swap the assertion direction:

```rust
// In crates/atlas-mem0g/src/embedder.rs tests module:
// BEFORE (W18b-shipped, fails after lift):
//   assert!(ONNX_SHA256.starts_with("TODO_W18B"), "...");
// AFTER (W18c-lifted, validates real values):
assert_eq!(ONNX_SHA256.len(), 64, "ONNX_SHA256 must be 64-char SHA-256 hex");
assert!(ONNX_SHA256.chars().all(|c| c.is_ascii_hexdigit()), "ONNX_SHA256 must be all hex digits");
assert_eq!(HF_REVISION_SHA.len(), 40, "HF_REVISION_SHA must be 40-char git SHA");
assert!(MODEL_URL.starts_with("https://huggingface.co/BAAI/bge-small-en-v1.5/resolve/"), "MODEL_URL must point to pinned revision");
```

**Step 8 — Verify `cargo test -p atlas-mem0g` passes.** The gatekeeper now asserts well-formedness, not placeholder state.

**Phase A acceptance criterion:** all three (+ three tokenizer) consts populated with real values verifiable via the HuggingFace API; gatekeeper test asserts well-formedness; commit + push as Phase-A welle (e.g. `feat(v2-beta/welle-18c-phase-a): Nelson supply-chain constant lift from BAAI/bge-small-en-v1.5 revision <SHA>`); admin-merge.

## Phase B — fastembed `try_new_from_user_defined` wiring (~1 session)

**Goal:** replace the unconditional fail-closed `Err(Mem0gError::Embedder("supply-chain gate: ..."))` in `AtlasEmbedder::new` with a real init path that uses the SHA-verified local model file.

**Pre-conditions:** Phase A complete (real constants compiled into the binary).

**Critical context from W18b reviewer fix (HIGH-2 deferred-with-gate):** the original W18b implementation called `fastembed::TextEmbedding::try_new(Default::default())`, which triggered fastembed's OWN model fetch from HuggingFace WITHOUT Atlas's SHA check. W18b's reviewer-fix changed `AtlasEmbedder::new` to unconditional fail-closed. W18c lifts this by wiring `try_new_from_user_defined` against the Atlas-controlled SHA-verified local files.

**fastembed-rs 5.13.4 API to research (subagent must verify against `cargo doc -p fastembed --open` OR upstream docs.rs):**

The `UserDefinedEmbeddingModel` constructor typically requires:
- ONNX model bytes (read from the SHA-verified `model.onnx` path)
- Tokenizer config bytes (`tokenizer.json` SHA-verified)
- Pre/post-processing config (`config.json` + `special_tokens_map.json` SHA-verified)
- Embedding dimension (384 for `bge-small-en-v1.5`)

**Implementation outline (illustrative; exact API surface verified by subagent):**

```rust
// crates/atlas-mem0g/src/embedder.rs::AtlasEmbedder::new
pub fn new(model_cache_dir: &std::path::Path) -> Mem0gResult<Self> {
    // Step 1: download + SHA-verify ALL FOUR files via download_with_verification helpers
    let model_path = download_model_with_verification(&model_cache_dir.join("model.onnx"))?;
    let tokenizer_path = download_tokenizer_with_verification(&model_cache_dir.join("tokenizer.json"))?;
    let config_path = download_config_with_verification(&model_cache_dir.join("config.json"))?;
    let special_tokens_path = download_special_tokens_with_verification(&model_cache_dir.join("special_tokens_map.json"))?;

    // Step 2: pin OMP threads BEFORE fastembed init
    pin_omp_threads_single();

    // Step 3: read SHA-verified bytes
    let model_bytes = std::fs::read(&model_path).map_err(|e| Mem0gError::Io(format!("read model: {e}")))?;
    let tokenizer_bytes = std::fs::read(&tokenizer_path).map_err(|e| Mem0gError::Io(format!("read tokenizer: {e}")))?;
    // (config + special_tokens similar)

    // Step 4: construct UserDefinedEmbeddingModel (API exact surface verified by subagent)
    let user_model = fastembed::UserDefinedEmbeddingModel { ... };
    let inner = fastembed::TextEmbedding::try_new_from_user_defined(user_model, fastembed::InitOptions::default())
        .map_err(|e| Mem0gError::Embedder(format!("fastembed init: {e}")))?;

    Ok(Self { inner, model_cache_dir: model_cache_dir.to_path_buf() })
}
```

**Phase B acceptance criterion:** `AtlasEmbedder::new` returns `Ok(_)` when called against a valid SHA-verified model dir. Unit test `embed_returns_384_dim_vector` passes. `cargo clippy --workspace --no-deps -- -D warnings` clean.

**Reviewer focus:** the four download_with_verification helpers must each fail-closed on SHA mismatch; the SHA-verified bytes must be the ones fed to fastembed (NO bypass path); the `cargo doc` claim about `try_new_from_user_defined` API surface should be cited in commit message.

## Phase C — V1-V4 verification gap closure (~1 session)

**Goal:** close the four verification gaps from W18 spike §12:

- **V1** — LanceDB `cleanup_old_versions` Windows behaviour. 50-line Rust integration test on Windows CI runner.
- **V2** — fastembed-rs determinism across (ORT-version, threads=1, FP32). 2-run byte-equality test on Linux + Windows + macOS CI matrix.
- **V3** — Lance v2.2 `_deletion_files` semantics (verify unchanged vs v2.1 before adopting Lance 0.30+). Spike test.
- **V4** — fastembed-rs model file size on disk (claimed ~130 MB; first-load measurement).

**Implementation:**

1. Expand `.github/workflows/atlas-mem0g-smoke.yml` to a matrix: `runs-on: [ubuntu-latest, windows-latest, macos-latest]`. Cache model file per OS.
2. Add `crates/atlas-mem0g/tests/lancedb_windows_behaviour.rs` (V1).
3. Update `crates/atlas-mem0g/tests/embedding_determinism.rs` to actually run feature-on (Phase A unblocked) + assert byte-equality.
4. Add `crates/atlas-mem0g/tests/lance_deletion_files_semantics.rs` (V3 — only if Lance 0.30+ is being considered; otherwise document the lock at 0.29 in plan-doc).
5. Update `crates/atlas-mem0g/tests/embedder_size_measurement.rs` (V4 — `std::fs::metadata` after first-load).

**Phase C acceptance criterion:** all three CI matrix runs (Linux + Windows + macOS) green. If Windows determinism fails, the fallback policy (event_uuid-only cache-key on Windows) is documented in operator-runbook + a release-note callout.

## Phase D — LanceDB ANN/search body fill-in (~1-2 sessions)

**Goal:** replace `Mem0gError::Backend("not yet wired")` placeholders in `crates/atlas-mem0g/src/lancedb_backend.rs::{upsert, search, erase, rebuild}` with real `tokio::task::spawn_blocking`-wrapped LanceDB calls.

**Critical: NOT `tokio::runtime::Handle::current().block_on()`** — the latter deadlocks under the single-threaded tokio scheduler when called from inside an async context. The W18b doc-comments include `RESUME(spawn_blocking)` markers at the relevant body sites; W18c lifts each marker.

**Implementation outline per body site (illustrative; exact LanceDB 0.29 API verified by subagent):**

```rust
fn upsert(&self, workspace_id: &WorkspaceId, event_uuid: &str, text: &str) -> Mem0gResult<()> {
    crate::check_workspace_id(workspace_id)?;
    let embedding = self.embedder.embed(text)?;  // Phase B-shipped
    let table_dir = self.table_dir_for(workspace_id);
    let lock = self.locks.acquire_write(workspace_id);

    // RESUME(spawn_blocking): wrap LanceDB async API
    tokio::task::block_in_place(|| {
        // OR if NOT in async context: build a tokio::runtime::Runtime locally
        // OR use spawn_blocking from a tokio context
        // SUBAGENT VERIFIES against fastembed-rs + lancedb crate examples
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        rt.block_on(async {
            let conn = lancedb::connect(&table_dir.to_string_lossy()).execute().await?;
            let table = conn.open_table("vectors").execute().await?;
            // Build Arrow RecordBatch from (event_uuid, embedding, snippet)
            // Append via table.add(record_batch).execute().await?;
            Ok::<_, lancedb::Error>(())
        }).map_err(|e| Mem0gError::Backend(format!("lancedb upsert: {e}")))?;
        Ok::<_, Mem0gError>(())
    })?;

    Ok(())
}
```

(Search + erase + rebuild similar; secure-delete protocol already implemented in W18b — Phase D wires the actual delete + cleanup_old_versions calls.)

**Phase D acceptance criterion:** Read-API `/api/atlas/semantic-search` returns real top-k `SemanticHit` results (no longer 501); B4 cache-hit semantic-search latency p99 captured in CI artifact; cite-back `event_uuid` round-trips through Read-API → verifier-rebuild via Layer 1.

## Operator-runbook update (cross-cutting concern)

After Phase A-D land, update `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` (or rename to V2-BETA-1) with:
- Layer 3 activation procedure (model cache dir setup, supply-chain constant verification)
- Response-time normalisation env var documentation (`ATLAS_SEMANTIC_SEARCH_MIN_LATENCY_MS`)
- `embedding_erased` event-kind compliance procedure (Art. 17 erasure workflow)
- Bench-shape interpretation guide (B4/B5/B6 numbers from `atlas-mem0g-smoke` artifact)
- Cross-platform fallback policy (event_uuid-only cache-key on Windows if V2 fails)
- Mem0g rebuild trigger thresholds (TTL + on-event + Layer-1-head-divergence triple)

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| **R-W18c-A1 — Nelson lifts wrong revision SHA** | LOW (process is curl-against-API + sha256sum) | HIGH (silently-wrong supply-chain pin survives indefinitely) | Step 7 well-formedness test catches structural issues; counsel-engagement review of supply-chain provenance per ADR-Atlas-012 §5.4 catches semantic issues. |
| **R-W18c-B1 — fastembed `try_new_from_user_defined` API surface differs from outline** | MED (subagent works against API contract that may have drifted since 5.13.4 release notes) | MED (Phase B re-work) | Pin exact-version `fastembed = "=5.13.4"`; subagent runs `cargo doc -p fastembed --open` BEFORE writing code; document any deviation in W18c plan-doc Implementation Notes. |
| **R-W18c-C1 — Windows determinism fails** | MED-HIGH (cross-platform float determinism is hard) | LOW (fallback to event_uuid-only cache-key on Windows is documented + reversible) | Phase C captures the failure mode; release-note callout informs operators; Linux remains the primary deployment target until V2-γ. |
| **R-W18c-D1 — LanceDB API surface drifts** | LOW (LanceDB 0.29 is a recent stable; we pin exact-minor) | MED (Phase D body fill-in re-work) | `lancedb = "0.29"` minor-version pin; subagent runs `cargo doc -p lancedb` BEFORE writing code; document deviations in plan-doc. |
| **R-W18c-D2 — `tokio::task::spawn_blocking` vs `block_on` deadlock** | LOW (W18b docstring already flags this) | HIGH (Read-API deadlocks under load) | TDD-RED test: call `SemanticCacheBackend::search` from inside a tokio multi-task context; assert no deadlock. Subagent dispatches `code-reviewer` post-implementation specifically on async-correctness. |
| **R-W18c-E1 — Atlas+Mem0g end-to-end benchmark exceeds operational latency budget** | LOW-MED | MED | B5 captures cache-miss-with-rebuild baseline in CI; operator-runbook documents rebuild-trigger thresholds; if B5 > 60s for 10K-event workspace, open follow-on ADR for cache-invalidation policy adjustment. |
| **R-W18c-F1 — W19 ship happens BEFORE W18c completes (intentional per ship-strategy)** | HIGH (intentional) | LOW (Layer 3 ships as 501 stub — honest scaffold posture documented) | W19 release notes EXPLICITLY state W18c parallel-track status; Read-API 501 response carries clear messaging; counsel-track gates public marketing per `DECISION-COUNSEL-1`. |

## Acceptance criteria (overall W18c)

- [ ] Phase A: 3 supply-chain consts + 3 tokenizer-file SHA-256 consts populated with real values
- [ ] Phase B: `AtlasEmbedder::new` returns `Ok(_)` against real model dir; embed() returns 384-dim vector
- [ ] Phase C: cross-platform CI matrix (Linux + Windows + macOS) green OR fallback documented
- [ ] Phase D: `/api/atlas/semantic-search` returns real top-k `SemanticHit` (NOT 501 stub)
- [ ] B4/B5/B6 bench numbers captured in `atlas-mem0g-smoke` CI artifact
- [ ] Cite-back end-to-end: semantic-search response → Layer-1 `event_uuid` → offline WASM verifier verifies independently
- [ ] Operator-runbook V2-β chapter updated with Layer 3 activation procedure
- [ ] Per Atlas Standing Protocol Lesson #8: parallel `code-reviewer` + `security-reviewer` dispatched for each Phase B + C + D welle; 0 unresolved CRITICAL / HIGH
- [ ] Phase 14.5-equivalent consolidation PR per phase (handoff + CHANGELOG + master-plan §6 status flip)

## ADR-Atlas-013 contingency

ADR-Atlas-013 is reserved. Open ONLY if W18c implementation surfaces a design amendment to ADR-Atlas-012 (e.g. cross-platform determinism failure changes the cache-key strategy from dual-key to event_uuid-only; LanceDB v2.2 `_deletion_files` semantics requires secure-delete protocol revision; fastembed-rs API drift requires changing the supply-chain control flow).

If ADR-Atlas-013 IS opened: mirror ADR-Atlas-010 / ADR-Atlas-012 structure; cite which sub-decision of ADR-Atlas-012 is being amended; preserve the original ADR-012 record.

## Subagent dispatch prompt skeleton (anti-divergence enforcement, for W18c Phase B-D)

(Phase A is semi-manual; Nelson + parent execute Steps 1-8 directly without subagent dispatch.)

When parent dispatches a W18c Phase-B / Phase-C / Phase-D subagent:

```text
Atlas project at C:\Users\nelso\Desktop\atlas. V2-β Welle 18c Phase <B|C|D> — <Phase description>.
Master HEAD at time of dispatch: <commit-sha-post-Phase-A-or-previous-Phase>.

## Your goal
<Phase-specific goal from W18c plan-doc>

## Pre-flight (FIRST 3 actions — Atlas Lesson #1)
1. `git fetch origin`
2. `git checkout -B feat/v2-beta/welle-18c-phase-<B|C|D> origin/master`
3. `git status` clean

## Pre-flight reading (mandatory)
1. `.handoff/v2-beta-welle-18c-plan.md` (this plan-doc) — Phase-specific implementation outline
2. `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` §4 sub-decisions
3. `crates/atlas-mem0g/src/embedder.rs` — Phase-B-relevant: AtlasEmbedder::new current fail-closed
4. `crates/atlas-mem0g/src/lancedb_backend.rs` — Phase-D-relevant: RESUME(spawn_blocking) markers
5. `crates/atlas-mem0g/tests/embedding_determinism.rs` + `secure_delete_correctness.rs` — Phase-C-relevant
6. `cargo doc -p fastembed --open` (Phase B) OR `cargo doc -p lancedb --open` (Phase D) — API surface verification

## Hard rules (Atlas Standing Protocol)
- Byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` MUST remain reproducible.
- SSH-Ed25519 signed commits.
- Parent dispatches parallel `code-reviewer` + `security-reviewer` post-implementation (Lesson #8).
- `tokio::task::spawn_blocking` ONLY for LanceDB async wrapping; NEVER `Handle::current().block_on()` (deadlocks per W18b ADR + Lesson #16-17).

## Acceptance criteria
<Phase-specific from W18c plan-doc>

## Output (under 400 words)
PR number + URL, line counts, Phase-specific verification evidence, reviewer-finding counts + resolutions, deviations from API outline (esp. fastembed `try_new_from_user_defined` or LanceDB body API surface).
```

---

## Implementation Notes (Post-Code) — fill AFTER each Phase ships

```
### Phase A — Nelson supply-chain constant lift

- HF_REVISION_SHA: <actual value>
- ONNX_SHA256: <actual value>
- MODEL_URL: <actual value>
- TOKENIZER_JSON_SHA256: <actual value>
- CONFIG_JSON_SHA256: <actual value>
- SPECIAL_TOKENS_MAP_SHA256: <actual value>
- ONNX file size: <actual MB measurement; verifies V4 from spike §12>
- Commit: <SHA>; PR: <#>; merged: <YYYY-MM-DD>

### Phase B — fastembed try_new_from_user_defined wiring

- fastembed API surface used: <exact signature observed in cargo doc>
- Deviations from outline: <None / list>
- Commit: <SHA>; PR: <#>; merged: <YYYY-MM-DD>

### Phase C — V1-V4 verification gap closure

- V1 LanceDB Windows: <PASS / FAIL+mitigation>
- V2 cross-platform determinism: <PASS on Linux+Windows+macOS / fallback policy>
- V3 Lance _deletion_files: <verified unchanged / Lance 0.30 adopted as separate welle>
- V4 model size: <actual MB>
- Commit: <SHA>; PR: <#>; merged: <YYYY-MM-DD>

### Phase D — LanceDB ANN/search body fill-in

- Read-API `/api/atlas/semantic-search` returns real hits: <YES / NO+blocker>
- B4 cache-hit p99: <ms>
- B5 cache-miss-with-rebuild p99: <s>
- B6 secure-delete primitive correctness cycles: <count, 100% pass / failures>
- Cite-back end-to-end: <verified / NOT-yet>
- Commit: <SHA>; PR: <#>; merged: <YYYY-MM-DD>
```

---

**End of W18c plan-doc.** Phase A is parallel-track-startable anytime; Phase B requires Phase A; Phase C is parallel-with-D; Phase D requires Phase A + B.
