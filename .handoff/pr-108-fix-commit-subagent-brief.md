# PR #108 fix-commit subagent brief — pre-staged 2026-05-15-evening

> **For next-session parent:** Copy the entire `--- BEGIN PROMPT ---` to `--- END PROMPT ---` block below into the `prompt` parameter of an `Agent` tool call (subagent_type: `general-purpose`, isolation: `worktree`, run_in_background: `true`). Do NOT modify the prompt unless you have read this entire file and §0z9 of the handoff doc and decided an amendment is warranted.
>
> **Pre-dispatch parent checks:**
> 1. `cd /c/Users/nelso/Desktop/atlas` (Lesson #23 — explicit cd; harness reroutes CWD)
> 2. `git checkout master && git pull origin master` — confirm master HEAD is post-PR-#107-merge
> 3. `gh pr view 108` — confirm PR #108 still at HEAD `885eacc` (Phase D ship commit); should NOT have been modified between sessions
> 4. `gh pr list --state open` — confirm only #59/#61/#62 archive + #108 (PR #107 should have merged in this session's stage 1)
> 5. Dispatch.

--- BEGIN PROMPT ---

You are dispatched as the **W18c Phase D fix-commit** subagent for the Atlas project at `C:\Users\nelso\Desktop\atlas`. Master HEAD at dispatch time: post-PR-#107-merge (Phase C V1-V4 cross-platform CI matrix landed). Atlas is Nelson Mehlis' cryptographic trust verification project; V2-β-1 (`v2.0.0-beta.1`) is LIVE on npm + GitHub + master; Layer 3 embedder is OPERATIONAL (Phase B PR #105) + Layer 3 backend is OPERATIONAL pending this fix-commit (Phase D PR #108).

PR #108 reviewer-dispatch (parallel `code-reviewer` + `security-reviewer` per Atlas Standing Protocol Lesson #8) returned APPROVE-WITH-FIXES with **1 CRITICAL + 4 HIGH + 8 MEDIUM + 4 LOW**. Nelson chose Option A (synchronous post-Compact fragment-diff fix preserves GDPR Art. 17 sync-erasure story + counsel Path-B scope) + ADR-Atlas-013 OPEN (per code-reviewer "should be a named decision, not a handoff note") + B4/B5/B6 bench enablement bundled in this fix-commit. Plus deferred PR #107 code-MEDIUM-1 (expose `walk_collect_filtered`) lands here too because both PR #107's V1 test inlined-walker AND Phase D's lancedb_backend.rs touch the same `walk_collect_filtered` symbol — coordination-risk avoidance via single-PR application.

## Your goal

Apply ALL 13 reviewer findings + 1 deferred + bench enablement + ADR-013 as a **single fix-commit-on-top** of the Phase D ship commit (`885eacc`) per Atlas Standing Protocol Lesson #3 (no amend). Acceptance: PR #108 has 2-3 commits total (initial Phase D + your fix-commit(s) — TDD-RED + GREEN split is OK if it makes the diff easier to review). All required CI green. Subagent self-audit best-effort; parent will run external code-reviewer + security-reviewer post-update per Lesson #8.

## Pre-flight (FIRST 3 actions — Atlas Standing Protocol Lesson #1)

1. `git fetch origin`
2. `git checkout -B feat/v2-beta/welle-18c-phase-d-fix origin/feat/v2-beta/welle-18c-phase-d`
3. `git status` → clean; `git log --oneline -3` should show `885eacc` (Phase D GREEN) + `14147c7` (Phase D RED) + master-merge of PR #107

If your worktree is dirty OR your origin tip is not `885eacc`, STOP and report.

## MANDATORY Step 0 — LanceDB 0.29 API surface verification (BEFORE writing any code)

Per Atlas Standing Protocol R-W18c-D1 + Phase B precedent (which caught fastembed `TokenizerFiles` 4-not-3 fields BEFORE code):

```bash
/c/Users/nelso/.cargo/bin/cargo.exe doc -p lancedb --no-deps 2>&1 | tail -5
```

Then inspect `target/doc/lancedb/index.html`. Verify EXACT signatures for:
- `Connection::open_table(name).execute().await` (already used by Phase D — verify unchanged)
- `Table::optimize(OptimizeAction::Compact { ... }).await` — confirm `CompactionOptions::default()` field shape; this is STEP 4 in the secure-delete protocol
- **Critical for HIGH-1 fix:** any `Table` API that exposes the live fragment manifest paths post-Compact. The current `precapture_fragments` walks the workspace directory via `std::fs::read_dir`; verify whether this still finds OLD unreferenced fragment files post-Compact (it should — Compact rewrites manifest references but leaves OLD fragment files on disk until `cleanup_old_versions` unlinks them).
- `Table::cleanup_old_versions(older_than_duration).await` — verify the API surface; relevant for code-reviewer M2 reconciliation question (see below).

If the LanceDB 0.29 API has drifted from Phase D's implementation, DOCUMENT the deviation in your PR description AND in `.handoff/v2-beta-welle-18c-plan.md` Implementation Notes §"Phase D fix-commit" — analog to Phase B's atomic fastembed pin-set extension.

## Pre-flight reading (mandatory, in order)

1. `.handoff/v2-session-handoff.md` §0z9 (today's session narrative; full reviewer findings table)
2. `.handoff/v2-session-handoff.md` Lessons #1, #3, #6, #8, #16-#17, #18, #20-#24 (binding patterns)
3. `crates/atlas-mem0g/src/lancedb_backend.rs` (985 LOC currently — your primary edit target; read in full)
4. `crates/atlas-mem0g/src/secure_delete.rs` (W18b 7-step protocol + `apply_overwrite_set`'s MEDIUM-2 hard-fail guard that CRIT-1 bypasses)
5. `crates/atlas-mem0g/src/embedder.rs` (Phase B's `AtlasEmbedder::embed()`; consumed via `Mutex<TextEmbedding>` from upsert + search — your HIGH-2 fix coordinates with this)
6. `crates/atlas-mem0g/tests/lancedb_body_e2e.rs` (Phase D's deadlock + cite-back tests; you'll add new TDD-RED tests for CRIT-1 + HIGH-1 + HIGH-2)
7. `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts` (Phase D updated 501 messaging; security LOW-1 — strip internal test-file path disclosure)
8. `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` §4 sub-decision #4 (W18b 7-step protocol binding text) + §5.2 (fragment-overwrite mitigation claim — your Option A fix preserves this)
9. `docs/V2-BETA-MEM0G-SPIKE.md` §7 (sync-vs-async pattern; Phase D `Arc<Runtime>` choice rationale) + §12 (V1-V4 verification gaps)
10. `docs/ADR/ADR-Atlas-012-mem0g-layer3-design.md` (full ADR for ADR-013 cross-reference structure template)

## Hard rules (Atlas Standing Protocol — non-negotiable)

- **Byte-pin** `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` MUST remain reproducible (`cargo test -p atlas-projector --test backend_trait_conformance byte_pin --quiet`).
- **SSH-Ed25519 signed commits** (Nelson's key fingerprint `SHA256:qq/VVJYpsgEdeQSLqU0QS/gKn6ohXJHio+VkzVX+4Zg`).
- **NEVER `tokio::runtime::Handle::current().block_on()`** (Lessons #16-#17). Phase D uses `Arc<Runtime>`-owned multi-thread runtime + `runtime.block_on()` from sync trait methods — this pattern is endorsed; preserve it. Do NOT regress to `spawn_blocking` or `Handle::current().block_on()`.
- **HIGH-2 lock-before-embed**: in BOTH `upsert` and `search`, `self.embedder.embed(text)?` MUST be called AFTER acquiring the workspace `RwLock` guard, NOT before. The ADR-012 §4 sub-decision #4 protocol requires the lock to encompass the entire wrapper sequence — embed is part of that sequence.
- **CRIT-1 fix non-negotiable**: remove `.filter(|p| p.exists())` from upstream of `apply_overwrite_set` invocation in `erase`. Let the W18b MEDIUM-2 hard-fail guard handle vanished paths (it's the correct behaviour — under valid lock contract no path should disappear; if one does, that's a contract violation that MUST surface as an error, not a silent skip producing false `embedding_erased` attestation).
- **Phase D `Arc<Runtime>` ownership preserved**: do NOT change to per-call `spawn_blocking` + local current-thread runtime. Phase D's choice is endorsed by spike §7 and reviewers (security DEVIATION #1: SAFE; code DEVIATION #1: ACCEPTABLE with caveat).
- **File <= 800 LOC** (M-3): `lancedb_backend.rs` is currently 985 LOC. Extract helpers (`build_schema`, `build_single_row_batch`, `connect_to_workspace`, `open_or_create_table`, `escape_sql_literal`) into `crates/atlas-mem0g/src/lancedb_helpers.rs` — they're listed by code-reviewer as good extraction candidates.
- **Tests for CRIT-1 + HIGH-1 + HIGH-2 (TDD-RED first)**: write tests that prove each finding exists in current code BEFORE applying fixes. Then apply fixes and verify tests go GREEN. Commit RED state and GREEN state separately if helpful (Phase D precedent: `14147c7` RED + `885eacc` GREEN); single commit also acceptable if diff stays reviewable.
- **`atlas-mem0g-smoke` workflow MUST trigger** on this PR — touch `.handoff/v2-beta-welle-18c-plan.md` Phase D Implementation Notes (per Lesson #11 path-filter for `atlas-web-playwright` + atlas-mem0g-smoke path-filter).
- **CI must remain green** post-commit: `cargo test --workspace --quiet` ≥ 581 passed (581 baseline + new TDD-RED-then-GREEN tests + B4/B5/B6 bench enablement); `cargo clippy --workspace --no-deps -- -D warnings` zero; `cargo clippy -p atlas-mem0g --features lancedb-backend --no-deps -- -D warnings` zero; `cargo test -p atlas-projector --test backend_trait_conformance byte_pin --quiet` byte-pin reproduces; atlas-mem0g-smoke matrix on new V1-V4 workflow GREEN.
- **Lesson #20: verify CI green via `gh pr checks 108` BEFORE claiming completion** in your output report. Do NOT report "atlas-mem0g-smoke ✓" based on subagent's local cargo test results; the actual matrix CI is the only authoritative source.
- **Lesson #22: if you encounter a constraint that requires departing from this brief's scope (architectural deviation, not implementation-detail choice), STOP and report to parent for named decision** before shipping a workaround. The parent will decide whether to amend ADR-013 + counsel scope, OR adjust the fix-commit scope, OR escalate.

## Detailed task list

### Section 1 — CRITICAL (1 finding)

**CRIT-1 (security)** — `apply_overwrite_set` hard-fail guard bypassed by upstream `.filter(|p| p.exists())`

File: `crates/atlas-mem0g/src/lancedb_backend.rs::erase` (around the `surviving_indices` construction, lines ~758-768 in current Phase D)

Current code:
```rust
let surviving_indices: Vec<PathBuf> = index_paths
    .into_iter()
    .filter(|p| p.exists())  // ← REMOVE THIS LINE
    .collect();
```

Fix: remove the `.filter()` clause. Pass `index_paths` directly to `PreCapturedPaths::new`. The W18b MEDIUM-2 hard-fail guard inside `apply_overwrite_set` correctly raises `Mem0gError::SecureDelete` if any pre-captured path has vanished — that's the correct contract enforcement under the workspace `RwLock`. Silent skip via filter = false `embedding_erased` attestation = regulatory-attestation regression.

TDD-RED test (add to `crates/atlas-mem0g/tests/lancedb_body_e2e.rs`):
- Construct backend; populate with 1 event; START erase but interrupt mid-protocol to delete an index file from disk; resume erase; assert `Mem0gError::SecureDelete` is returned (under fix) or test FAILS under current code (which would silently skip and return `Ok(())`).

### Section 2 — HIGH (4 findings)

**HIGH-1 (combined code H1 + security HIGH-1)** — STEP 6 fragment-overwrite elision

Current Phase D code (around lines 740-770) computes `_captured_fragment_paths = self.precapture_fragments(workspace_id)?;` (STEP 2) then discards it, passing `vec![]` to `PreCapturedPaths::new` (STEP 6). This means physical fragment-bytes erasure is delegated to operator-scheduled `optimize(Prune)` — an unbounded post-Compact-pre-Prune window where deleted event bytes remain readable via Lance internals.

Option A (Nelson chose) algorithm — post-Compact fragment-set diff:

```rust
// STEP 2 (existing): pre-capture full fragment set BEFORE delete + compact
let pre_captured_fragments = self.precapture_fragments(workspace_id)?;

// STEP 3 + 4 (existing): tombstone + Compact (UNCHANGED in scope; lock-held)
//   ... existing block_on with delete + Compact ...

// STEP 4b (NEW): pre-capture fragment set POST-Compact
let post_compact_fragments: std::collections::HashSet<PathBuf> = self
    .precapture_fragments(workspace_id)?
    .into_iter()
    .collect();

// Unreferenced = pre_captured - post_compact. These are the OLD fragment files
// containing only the deleted event's bytes. Live manifest no longer
// references them; safe to byte-overwrite + unlink synchronously inside
// the lock. This restores ADR-012 §4 step 6 protocol envelope synchronously
// despite Lance's shared-fragment model (PR #108 reviewer HIGH-1 fix).
let unreferenced_fragments: Vec<PathBuf> = pre_captured_fragments
    .into_iter()
    .filter(|p| !post_compact_fragments.contains(p))
    .collect();

// STEP 5 (existing): HNSW indices pre-capture
let index_paths = self.precapture_indices(workspace_id)?;

// STEP 6 (FIX): pass BOTH unreferenced fragments AND index paths to
// apply_overwrite_set. CRIT-1 fix: NO `.filter(|p| p.exists())` here —
// let apply_overwrite_set's hard-fail guard handle any vanished path.
let paths = PreCapturedPaths::new(unreferenced_fragments, index_paths);
crate::secure_delete::apply_overwrite_set(&paths)?;
```

TDD-RED test (add to `lancedb_body_e2e.rs`):
- Populate workspace with 5 events; erase event[2]; assert that AFTER erase: (a) raw `fs::read` of the workspace directory does NOT contain event[2]'s embedding bytes anywhere; (b) events[0,1,3,4] are still searchable via SemanticHit. Current Phase D code FAILS (a) because old fragment file with event[2] survives.

**HIGH-2 (code)** — `embed()` called BEFORE workspace `RwLock` acquired

Files: `crates/atlas-mem0g/src/lancedb_backend.rs::upsert` (lines ~397-402) and `crates/atlas-mem0g/src/lancedb_backend.rs::search` (lines ~462-468)

Current pattern: `let embedding = self.embedder.embed(text)?; let _guard = self.locks.acquire_write(workspace_id);` — embed happens BEFORE lock acquisition, creating a TOCTOU window where erase could complete (STEP 6 overwrites indices) between embed and the LanceDB write/read.

Fix: swap order. Acquire lock FIRST, then embed:

```rust
fn upsert(&self, workspace_id: &WorkspaceId, event_uuid: &str, text: &str) -> Mem0gResult<()> {
    check_workspace_id(workspace_id)?;
    let _guard = self.locks.acquire_write(workspace_id);  // FIRST
    let embedding = self.embedder.embed(text)?;          // SECOND, under lock
    // ... rest of upsert ...
}
```

Same swap in `search`. The `Mutex<TextEmbedding>` inside `AtlasEmbedder` already serializes embed calls (no concurrency issue), so holding the workspace lock across embed adds latency but no deadlock risk. ADR-012 §4 sub-decision #4 explicitly requires lock-encompasses-entire-wrapper-sequence.

TDD-RED test (add to `lancedb_body_e2e.rs`):
- Concurrent test: thread A starts an erase (which holds lock for ~10ms via secure-delete protocol); thread B starts a search at same instant. Under current code, B can race-embed before erase completes; under fix, B waits for A's lock release. Assert search result is consistent with post-erase state.

**HIGH-1 (security: same as combined HIGH-1; covered above)**

**HIGH-2 (security)** — Operator-runbook gap for `optimize(Prune)` SLA

Under Option A, this finding's urgency is REDUCED because fragment-erasure is now synchronous (no Prune-SLA gap). However, the operator-runbook for Layer 3 activation procedure is still missing. Add a stub section to `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` (or rename to V2-BETA-1) titled "§Layer 3 activation procedure" that includes:
- Model cache directory setup (`ATLAS_MEM0G_MODEL_CACHE_DIR` env var)
- Supply-chain pin verification at cold start
- `embedding_erased` audit-event compliance procedure (Art. 17 erasure workflow)
- Cross-platform fallback policy (event_uuid-only cache-key on Windows if V2 fails per W18c-R1)
- Layer-3-secure-delete schedule (post-Option-A, this is "synchronous at erase() call time" — no operator scheduling required for fragments; HNSW indices are rebuilt on next index access).

This stub does NOT need to be fully fleshed out in this welle (cross-cutting concern flagged in Phase D plan-doc); a 30-line section is sufficient for now with cross-references to ADR-012 + ADR-013.

### Section 3 — MEDIUM (8 findings)

**M1 (code)** — `_captured_fragment_paths` dead variable with misleading comment
Becomes consumed under HIGH-1 fix → rename from `_captured_fragment_paths` to `pre_captured_fragments` (no underscore prefix). Update surrounding comment block (currently claims fragment paths "are retained for the operator-runbook diagnostic record" which was untrue) to accurately describe the post-Compact diff usage.

**M-3 (security: same as code M1)** — covered above.

**M2 (code)** — `optimize(Compact)` vs ADR-specified `cleanup_old_versions(Duration::ZERO)` semantic mismatch
The ADR-012 §4 sub-decision #4 step 4 says "CLEANUP via `cleanup_old_versions(Duration::ZERO)`" but Phase D uses `optimize(OptimizeAction::Compact)`. These are different operations:
- `Compact`: rewrites live fragments without tombstoned rows; old fragments become unreferenced (this is what Phase D wants for STEP 4)
- `cleanup_old_versions(Duration::ZERO)`: GC's old manifest versions + unlinks unreferenced files (this would delete the freshly-compacted fragment too)

Action: keep `optimize(Compact)` (correct for Phase D's purpose); document the semantic mapping in ADR-Atlas-013 §2.3 (see ADR-013 outline below). Phase D's existing comment block already reasons about this; just lift the explanation up to the ADR for permanence.

**M3 (code)** — `lancedb_backend.rs` 985 LOC > 800 LOC hard limit
Extract helpers to NEW `crates/atlas-mem0g/src/lancedb_helpers.rs`:
- `build_schema() -> Arc<Schema>`
- `build_single_row_batch(...) -> Result<RecordBatch, ArrowError>`
- `connect_to_workspace(table_dir: &Path) -> impl Future<Output = lancedb::Result<Connection>>`
- `open_or_create_table(connection: &Connection, name: &str, schema: Arc<Schema>) -> impl Future<Output = lancedb::Result<Table>>`
- `escape_sql_literal(input: &str) -> Result<String, Mem0gError>`

Update `lancedb_backend.rs` to `use crate::lancedb_helpers::*;` Verify post-extract LOC: `wc -l crates/atlas-mem0g/src/lancedb_backend.rs` should report <800.

**M4 (code) + M-4 (security)** — `Arc<Runtime>` drop-from-async footgun + multi-instance coordination
Add a `#[doc]` block on the `LanceDbCacheBackend` struct documenting:
- "MUST be dropped from a blocking thread context, not from inside an async task. Dropping the owned `Arc<Runtime>` from an async context panics per tokio docs."
- "Multi-instance: two simultaneous `LanceDbCacheBackend` instances against the same workspace storage path have INDEPENDENT `PerTableLockMap`s and therefore independent per-workspace `RwLock`s. Cross-instance coordination does NOT exist. Production deployments MUST ensure a single backend instance per workspace path."

No code change required (these are doc-comment-only fixes per reviewer); just the doc block.

**M-1 (security)** — Cite-back e2e test overclaim
File: `crates/atlas-mem0g/tests/lancedb_body_e2e.rs::upsert_then_search_round_trip`

Current test asserts `SemanticHit::event_uuid` matches the upserted UUID — that's storage round-trip, not Layer-1-verifier-path. Refactor:
- Rename current test to `upsert_then_search_storage_round_trip` (honest name)
- Add NEW test `cite_back_through_layer_1_verifier_e2e`: writes a real `AtlasEvent` to events.jsonl via Layer 1; calls upsert with that event's UUID + text; calls search; takes the returned `SemanticHit::event_uuid`; passes through `verify-wasm` (offline WASM verifier) to verify the event independently. This is the actual cite-back acceptance criterion.

If implementing the WASM verifier path is too scope-heavy for this welle, mark the new test `#[ignore]` with comment "V2-γ — full Layer-1 e2e verification; current Phase D scope: storage round-trip (see upsert_then_search_storage_round_trip)" and document the gap in `.handoff/v2-beta-welle-18c-plan.md` Implementation Notes.

**M-2 (security)** — NaN propagation in score formula
File: `crates/atlas-mem0g/src/lancedb_backend.rs::search` (around the score computation block)

Current: `1.0 / (1.0 + distance.max(0.0))` where `distance: f32` — if `distance` is NaN, `.max(0.0)` returns NaN (NaN does not compare less than 0.0; `f32::max` propagates NaN). NaN flows into `SemanticHit::score`. `serde_json` handles `f32::NAN` differently per serializer config — may serialize as `null` or panic.

Fix: clamp NaN to a safe value:
```rust
let safe_distance = if distance.is_nan() { f32::INFINITY } else { distance.max(0.0) };
let score = 1.0 / (1.0 + safe_distance);
```
With `f32::INFINITY` → score = 0.0 (worst possible match). Conservative + serializer-safe.

### Section 4 — LOW (4 findings)

**L1 (code)** — Erase test weak on byte-level assertion
File: `crates/atlas-mem0g/tests/lancedb_body_e2e.rs::erase_removes_only_targeted_event`
Add explicit query for the target UUID's text after erase; assert zero hits. Currently the test only checks "target UUID does not appear in broad query result" which is a weaker assertion.

**L2 (code)** — Score formula L2-vs-cosine doc gap
File: `crates/atlas-mem0g/src/lancedb_backend.rs::search` (score computation comment block)
Add comment: "BGE-small-en-v1.5 emits L2-normalised embeddings; L2 distance and cosine distance are equivalent under normalisation (L2² = 2 - 2·cos_sim). The `1/(1+d)` transform is monotonic; diagnostic-only per `SemanticHit::score` contract."

**L-1 (security)** — 501 message internal path disclosure
File: `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts`
Current 501 message body references `crates/atlas-mem0g/tests/lancedb_body_e2e.rs`. Strip the internal path; replace with public-facing "Read-API surface bridge V2-γ scope; see https://github.com/ThePyth0nKid/atlas/blob/master/docs/V2-MASTER-PLAN.md §6 for status."

**L-2 (security)** — `event_uuid` not validated in `erase`
File: `crates/atlas-mem0g/src/lancedb_backend.rs::erase`
Add early validation:
```rust
if event_uuid.is_empty() || event_uuid.len() > 128 {
    return Err(Mem0gError::InvalidInput(format!(
        "event_uuid must be 1-128 chars; got len={}",
        event_uuid.len()
    )));
}
```
Place AFTER `check_workspace_id`, BEFORE `escape_sql_literal`. The empty-string case is the silent-success risk (predicate `event_uuid = ''` deletes nothing, returns Ok, false attestation).

### Section 5 — Deferred PR #107 code-MEDIUM-1 (carry-over)

Expose `walk_collect_filtered` in `lancedb_backend.rs` for integration-test reach. Pattern:
```rust
#[cfg(test)]
pub(crate) fn walk_collect_filtered_for_test(
    dir: &Path,
    filter: impl Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    self.walk_collect_filtered(dir, filter)
}
```
OR make the existing private fn `pub(crate)` and update PR #107's V1 test (`crates/atlas-mem0g/tests/lancedb_windows_behaviour.rs`) to call the production fn instead of the inlined `walk()` function (which the parent earlier flagged with comment "PR #107 reviewer MEDIUM-1 (code) — defer to post-Phase-D").

Update the inlined walker's doc-comment to note the migration; OR remove the inlined walker entirely if production fn is now reachable.

### Section 6 — B4/B5/B6 bench enablement

File: `.github/workflows/atlas-mem0g-smoke.yml`
- Update "Run bench capture" step (currently runs WITHOUT `--features lancedb-backend`) to enable the feature: `cargo test -p atlas-mem0g --features lancedb-backend --test mem0g_benchmark -- --ignored --nocapture --include-ignored`
- Verify the bench step still runs within the matrix's 15-min timeout per leg (B4-B6 measurement should be quick — few seconds each)

File: `crates/atlas-mem0g/tests/mem0g_benchmark.rs`
- Replace placeholder "skipping" lines with real measurements:
  - **B4** (cache-hit p99): construct backend; populate with 1000 events; run 100 search queries with same text; measure p99 latency
  - **B5** (cache-miss-with-rebuild p99): same setup; run 100 queries with NEW unique text; measure p99 latency including cold-start embed cost
  - **B6** (secure-delete cycles correctness): run 50 cycles of (upsert event → erase event); after each cycle, assert byte-pin still reproduces + workspace files don't grow unboundedly
- Print results in `BENCH_<NAME> os=<os> ms=<value> n=<count>` format for grep-friendly artifact output

### Section 7 — ADR-Atlas-013 NEW

File: `docs/ADR/ADR-Atlas-013-mem0g-phase-d-amendments.md` (NEW, ~150 LOC)

Mirror the structure of ADR-Atlas-012. Sections:

1. **§1 Context** — Phase D shipped; Lance shared-fragment storage model surfaced via reviewer findings; named-decision documentation per Atlas Standing Protocol Lesson #22
2. **§2 Decisions:**
   - **§2.1** Backend-owned `Arc<tokio::runtime::Runtime>` (4-worker multi-thread) — endorses spike §7 Pattern C; alternative patterns (`spawn_blocking` + local current-thread; `Handle::current().block_on()`) explicitly rejected with rationale
   - **§2.2** Fragment-overwrite via post-Compact diff (Option A) — preserves ADR-012 §4 sub-decision #4 step 6 envelope synchronously despite Lance's shared-fragment model. Algorithm: `unreferenced = pre_captured_fragments - post_compact_fragments`; pass `unreferenced` to `apply_overwrite_set`.
   - **§2.3** STEP 4 uses `optimize(OptimizeAction::Compact)` — semantic equivalence justification for ADR-012's `cleanup_old_versions(Duration::ZERO)` text. (`Compact` rewrites live fragments without tombstoned rows; old fragments become unreferenced — exactly what STEP 4 needs. `cleanup_old_versions(Duration::ZERO)` would delete the freshly-compacted fragment too.)
   - **§2.4** V2-γ alternatives for stronger erasure (cryptographic per-event keys; pre-fragment-isolation write model; Lance internals if exposed upstream) — documented but NOT chosen for V2-β
3. **§3 Implications:**
   - Counsel Path-B scope per `DECISION-COUNSEL-1` UNCHANGED (Option A preserves sync byte-erasure framing)
   - B4/B5/B6 numbers captured this welle (atlas-mem0g-smoke artifact)
   - Cite-back e2e tightened (storage round-trip + V2-γ Layer-1-verifier-path test marked)
   - ADR-012 §5.2 fragment-overwrite mitigation claim PRESERVED under Option A
4. **§4 Cross-references** — ADR-012 (parent ADR), spike §7, decisions log entry DECISION-ARCH-W18c-D + DECISION-ARCH-013, PR #107/#108

### Section 8 — DECISION-ARCH-W18c-D entry in decisions.md

File: `.handoff/decisions.md`
Add as 32nd entry (after current 31st DECISION-ARCH-W18c-B). Format per existing pattern:

```markdown
## 2026-05-15: W18c Phase D Implementation Pattern + ADR-013 Open [DECISION-ARCH-W18c-D]
- Crit source: PR #108 reviewer-dispatch (parallel code-reviewer + security-reviewer); 1 CRIT + 4 HIGH + 8 MEDIUM + 4 LOW
- Phase 1 doc affected: ADR-Atlas-012 §4 sub-decision #4 (W18b 7-step protocol step 6); §5.2 (fragment-overwrite mitigation claim)
- Recommendation: synchronous post-Compact fragment-set diff (Option A) preserves ADR-012 §4 step 6 envelope; alternative was Option B (asynchronous Prune + ADR-013 reframe + counsel rescope)
- Decision: ACCEPT Option A. ADR-Atlas-013 OPEN as named-decision documentation per code-reviewer.
- Rationale: Option A is "Geh mit A, beste Sicherheit + Codequalität" aligned. Preserves GDPR Art. 17 sync-erasure story. Counsel Path-B scope per `DECISION-COUNSEL-1` unchanged. Implementation cost: 5-line algorithm extension. Architectural surprise (Lance shared-fragment model) documented in ADR-013 for future reference.
- Reversibility: HIGH (algorithm change in lancedb_backend.rs; no schema or storage-format impact)
- Review-after: V2-γ — re-evaluate when (a) Lance exposes internal manifest API for cleaner pre/post diff, OR (b) cryptographic per-event keys evaluated as stronger erasure primitive.
```

### Section 9 — Plan-doc Phase D Implementation Notes update

File: `.handoff/v2-beta-welle-18c-plan.md`
Append to Phase D Implementation Notes section (the section Phase D subagent created):

```markdown
### Phase D fix-commit (2026-05-15-evening session, applied 2026-05-XX)

- Reviewer findings: 1 CRIT + 4 HIGH + 8 MEDIUM + 4 LOW (PR #108 parallel reviewer-dispatch)
- Option A applied: post-Compact fragment-diff approach (~5-line algorithm extension); ADR-012 §4 step 6 envelope preserved synchronously
- ADR-Atlas-013 opened: named-decision documentation per code-reviewer
- DECISION-ARCH-W18c-D added (32nd entry in decisions.md)
- B4/B5/B6 bench numbers captured: <subagent fills in actual values>
- File sizes post-fix: lancedb_backend.rs <800 LOC (helpers extracted to lancedb_helpers.rs)
- TDD-RED → GREEN evidence: <subagent fills in commit SHAs>
- Atlas Standing Protocol Lessons applied: #1, #3, #6, #8, #16-#17, #18, #20, #22 (see fix-commit message for explicit references)
- Commit: <SHA>; PR: #108 fix-commit; merged: <YYYY-MM-DD>
```

## Acceptance criteria (subagent self-checks each item explicitly per Lesson #20)

For each criterion below, mark ACHIEVED or NOT-ACHIEVED in your output report. If NOT-ACHIEVED, name the specific gap.

- [ ] CRIT-1 fix applied (`.filter(|p| p.exists())` removed from upstream of `apply_overwrite_set`)
- [ ] HIGH-1 fix applied (post-Compact fragment-set diff; `unreferenced_fragments` consumed by `PreCapturedPaths::new`)
- [ ] HIGH-2 fix applied (lock-acquire BEFORE `embed()` in both `upsert` and `search`)
- [ ] HIGH-2 (sec) operator-runbook stub added (~30 lines minimum)
- [ ] All 8 MEDIUMs applied (M1, M-3 same finding; M2 documented in ADR-013; M3 file <800 LOC; M4+M-4 doc-block; M-1 cite-back refactor; M-2 NaN clamp)
- [ ] All 4 LOWs applied (L1 erase test strengthening; L2 score doc; L-1 path strip; L-2 event_uuid validation)
- [ ] Deferred PR #107 code-MEDIUM-1 applied (`walk_collect_filtered` exposed for test reach)
- [ ] B4/B5/B6 bench enablement (workflow + harness; numbers captured in artifact)
- [ ] ADR-Atlas-013 written (~150 LOC; 4 sections; cross-refs)
- [ ] DECISION-ARCH-W18c-D entry added to decisions.md (32nd entry)
- [ ] Plan-doc Phase D Implementation Notes updated with fix-commit entry
- [ ] `cargo test --workspace --quiet` passes (≥581 tests; +N for new TDD tests)
- [ ] `cargo clippy --workspace --no-deps -- -D warnings` zero
- [ ] `cargo clippy -p atlas-mem0g --features lancedb-backend --no-deps -- -D warnings` zero
- [ ] Byte-pin `8962c168…e013ac4` reproduces
- [ ] `lancedb_backend.rs` LOC < 800 (verify via `wc -l`)
- [ ] All required CI checks GREEN on PR #108 (verify via `gh pr checks 108` per Lesson #20 — do NOT claim green without explicit gh check)

## Workflow

1. Pre-flight (Steps 1-3 above)
2. Pre-flight reading (10 files above)
3. Step 0 LanceDB API surface verification + document deviations
4. TDD-RED commit: write failing tests for CRIT-1 + HIGH-1 + HIGH-2 (3 new test functions in `lancedb_body_e2e.rs`); commit as `test(v2-beta/welle-18c-phase-d-fix): TDD-RED — CRIT-1 + HIGH-1 + HIGH-2 reviewer-finding tests`
5. GREEN commit: apply CRIT-1 + HIGH-1 + HIGH-2 fixes; verify TDD-RED tests pass; commit as `fix(v2-beta/welle-18c-phase-d): apply PR #108 reviewer findings — CRIT-1 + HIGH-1 + HIGH-2 (Option A synchronous fragment-diff)`
6. Optional second GREEN commit for the rest: MEDIUMs + LOWs + helpers extract + bench + ADR + decisions; commit as `fix(v2-beta/welle-18c-phase-d): MEDIUMs + LOWs + helpers extract + B4/B5/B6 bench + ADR-013 NEW`
7. Run all verification gates (cargo check + test + clippy + byte-pin); if any fail, diagnose + push fix-commit on top
8. `git push -u origin feat/v2-beta/welle-18c-phase-d-fix:feat/v2-beta/welle-18c-phase-d` (per Lesson #23 — explicit refspec to avoid worktree-collision)
9. **Lesson #20 verify CI**: wait for `gh pr checks 108` to complete; report ACTUAL CI status (NOT subagent's local results)
10. Self-audit per Lesson #8 best-effort

## Output (under 500 words)

Report back to parent with:
- PR number + URL
- Commit SHAs (TDD-RED, GREEN, optional second-GREEN)
- LOC delta + files touched
- LanceDB 0.29 API surface deviations from Phase D outline (NONE / list)
- ALL 14 acceptance criteria status (ACHIEVED / NOT-ACHIEVED, per Lesson #20 — no general "all green" claim)
- Each finding's resolution explicitly mapped (CRIT-1, HIGH-1, HIGH-2, HIGH-2(sec), all 8 MEDIUMs, all 4 LOWs, deferred PR #107 code-M-1, bench, ADR, decisions, plan-doc)
- B4/B5/B6 measured values
- ADR-Atlas-013 final structure summary (1-line per section)
- `gh pr checks 108` output post-CI completion (timestamps required)
- Self-audit findings (best-effort)
- ADR-013 amendments needed? (only if your impl surfaced ADDITIONAL constraints beyond what reviewers flagged)
- Any deferred items + rationale (V2-γ scope OK; same-welle MUST be applied unless explicit Nelson decision)

Parent will then dispatch parallel `code-reviewer` + `security-reviewer` per Lesson #8 + handle merge per Lesson #6 admin-merge.

--- END PROMPT ---
