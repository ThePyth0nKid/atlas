# ADR-Atlas-012 — Mem0g Layer-3 Cache Design

| Field             | Value                                                              |
|-------------------|--------------------------------------------------------------------|
| **Status**        | Accepted                                                           |
| **Date**          | 2026-05-15                                                         |
| **Welle**         | V2-β Welle 18 (Phase 12)                                           |
| **Authors**       | Nelson Mehlis (`@ThePyth0nKid`); welle-18 parent agent (Claude Opus 4.7) |
| **Replaces**      | —                                                                  |
| **Superseded by** | —                                                                  |
| **Related**       | `DECISION-SEC-5` (Mem0g embedding-leakage secure-deletion); `DECISION-DB-3` (Mem0g latency-claim attribution); `DECISION-DB-4` (Apache-2.0 + JVM/Python avoidance pattern); `DECISION-ARCH-1` (V2-α byte-determinism triple-hardening); `DECISION-COUNSEL-1` (GDPR Art. 4(1) hash-as-PII opinion); ADR-Atlas-010 (Layer-2 ArcadeDB structural template); ADR-Atlas-011 (Layer-2 trait-surface design pattern that W18b mirrors for Layer 3); `docs/V2-BETA-MEM0G-SPIKE.md` (W18 architectural spike — companion doc); ADR-Atlas-013 (W18b, forthcoming if implementation surfaces design amendment) |

---

## 1. Context

### 1.1 What Layer 3 is for

Atlas's V2 Three-Layer Trust Architecture (`docs/V2-MASTER-PLAN.md` §3) places Layer 3 as the FAST, REBUILDABLE, NEVER-AUTHORITATIVE semantic cache that sits on top of Layer-2 ArcadeDB projection (W17a-c shipped). Layer 3 enables semantic-search-by-meaning over the authoritative Layer-1 events.jsonl: a consumer asks *"show me events about credit-default for German SMEs in Q1 2026"*; the cache returns top-k vectors by cosine similarity; each result carries a Layer-1 `event_uuid` so the consumer can verify the underlying event via the offline WASM verifier. The cache **never** provides trust authority — it accelerates retrieval, full stop.

### 1.2 The "Mem0g" naming clarification

Master-plan §3 names the layer "Mem0g cache". Research surfaced (W18 spike §3.1) that *Mem0g* is **not a separate product** — it is a research-paper name (arXiv:2504.19413, mem0ai team, 2026) for the graph-augmented mode of the open-source `mem0` Python package. As of 2026-05, the `--graph` CLI flag was removed in favour of per-project config setting; "Mem0g" the product is `mem0` + graph-mode. This ADR uses "Mem0g" as the **concept name** (per master-plan continuity) while binding Atlas's implementation to Atlas-controlled Rust components.

### 1.3 The architectural unknowns at the start of W18

Per handoff §0-NEXT, W18 entered with 6 design questions. The W18 spike (`docs/V2-BETA-MEM0G-SPIKE.md` §4) answers each. This ADR distils the spike's answers into **binding sub-decisions** that constrain W18b implementation work.

### 1.4 Why this matters for Atlas specifically

- **Hermes-skill distribution channel** (V2-γ scope): ships as `npx @atlas-trust/quickstart`. Layer 3 cannot bundle a Python runtime (already paying JVM cost via ArcadeDB sidecar; doubling that cost would gut the cold-start UX). Implementation choice is a **one-way door**.
- **GDPR Art. 17 erasure compliance**: Atlas's substantive claim that customer PII can be cryptographically erased depends on Layer-3 secure-delete actually working at the disk-byte level, not just at the index-tombstone level. Embeddings can reconstruct ~92% of source content (Morris et al. 2023); hash-only is insufficient.
- **Byte-determinism CI pin** (`graph_state_hash` byte-pin `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`): MUST remain reproducible end-to-end after W18 + W18b ship. lib.rs invariant #3 forbids floats in canonicalisation; embeddings (floats) MUST live outside the canonicalisation pipeline.
- **Trust property end-to-end**: every Layer-3 response carries `event_uuid` so the consumer can drop down to Layer 1 + run offline WASM verifier independently. Layer 3 failure does NOT compromise Atlas's bedrock trust property.

---

## 2. Decision drivers

- **License compatibility:** Apache-2.0 strongly preferred across all Atlas tiers (analog `DECISION-DB-4`'s license-decisive reasoning, transferred to Layer 3).
- **Distribution constraint:** Hermes-skill `npx` package cannot bundle Python OR a second JVM. Atlas already pays JVM cost for ArcadeDB sidecar; tripling process types (Atlas Rust + ArcadeDB JVM + Mem0 Python) is operationally untenable.
- **Atlas-controlled secure-delete:** GDPR Art. 17 compliance depends on Atlas owning the overwrite-then-unlink primitive end-to-end. Tools that abstract storage away (Mem0) do not give Atlas the control needed.
- **Embedding-determinism:** for cache-key invariants and reproducibility, embeddings should be deterministic under pinned conditions. Cloud APIs (OpenAI text-embedding-3-small) are not deterministic across calls.
- **Byte-determinism preservation:** `graph_state_hash` invariant MUST hold; embeddings cannot enter the canonicalisation pipeline.
- **Cite-back trust property:** every Layer-3 response MUST include `event_uuid`; trust authority lives at Layer 1 only.
- **Operational simplicity:** every additional sidecar process is operational + supply-chain cost. Embedded > sidecar when feasible.
- **Pivot reversibility:** the implementation choice should be encapsulated behind a Rust trait so a future swap (e.g. LanceDB → Qdrant) is one trait-impl, not a re-architecture.

---

## 3. Considered options

(See `docs/V2-BETA-MEM0G-SPIKE.md` §3 for the full six-option survey + §5 for the trade-off matrix. This ADR summarises the conclusions.)

### 3.1 Option A: LanceDB embedded + fastembed-rs embedded (RECOMMENDED)

**Mechanism:**
- `lancedb 0.29.0` Rust crate linked into a NEW workspace member crate `crates/atlas-mem0g/` (per §4 sub-decision #7).
- `fastembed-rs 5.13.4` ONNX-CPU embedder pinned to `bge-small-en-v1.5` FP32 model.
- Both Apache-2.0; both pure-Rust embedded. Zero new sidecars.
- Atlas wraps LanceDB `delete()` + `cleanup_old_versions(0)` with explicit `tokio::fs` random-fillbytes-then-`fdatasync` overwrite for GDPR Art. 17 compliance (see §4 sub-decision #4).
- Embeddings deterministic under pinned (ORT-version, OMP_NUM_THREADS=1, FP32) tuple; cross-platform verification test mandatory in W18b.
- Hermes-skill talks HTTP to atlas-projector; never sees LanceDB or fastembed-rs directly.

**Pros:**
- License-compatible across all Atlas tiers (Apache-2.0).
- Hermes-skill distribution intact (no new runtime requirement).
- Atlas-controlled secure-delete primitive (we own the wrapper).
- Embedder-determinism control (Atlas pins all four conditions).
- Single binary deployment (atlas-projector + ArcadeDB sidecar — no third process).
- Trust-property end-to-end Atlas-controlled.

**Cons:**
- LanceDB heavy dep tree (~200 transitive crates incl. Arrow 58 + DataFusion 53) — license/audit burden non-trivial, requires Atlas quarterly dep-audit.
- Secure-delete is hand-rolled — Atlas owns the wrapper correctness (W18b test verifies).
- Cross-platform determinism (Linux + Windows + macOS) requires Atlas-side verification test.

**Confidence:** **HIGH** on choice; **MEDIUM-HIGH** on dep-tree audit burden; **MEDIUM-HIGH** on determinism (verification test mandatory).

### 3.2 Option B: Qdrant sidecar + fastembed-rs embedded (PIVOT, NOT REJECTED)

**Mechanism:**
- Qdrant runs as Docker sidecar process (analog ArcadeDB pattern).
- `qdrant-client 1.18` Rust client (gRPC via `tonic`).
- Same fastembed-rs embedder.

**Pros:**
- Lighter Atlas dep tree (gRPC client only; storage logic in sidecar).
- Per-workspace process-isolation if a customer requires it.

**Cons:**
- **Second sidecar** doubles operational complexity vs Option A (Atlas + ArcadeDB JVM + Qdrant).
- Network hop adds tail latency to a "fast cache" layer.
- Same hand-rolled secure-delete wrapper, but at coarser segment-granularity.

**Decision:** **PIVOT, not chosen for W18b initial.** Reserved as the documented swap if any LP1-LP5 (spike §9) trigger fires.

### 3.3 Option C: Mem0 Python sidecar (REJECTED)

**Decision:** **REJECTED.** Three independent blockers:
- Python runtime co-resident with Atlas violates the Hermes-skill distribution constraint.
- Default embedder is OpenAI cloud (non-deterministic + third-party vendor on the trust-substrate critical path).
- Secure-delete is delegated to backing store; Atlas does NOT control the primitive.

Mem0 the company remains a strong **partner** candidate (master-plan §8) — Atlas can publish a cross-promotional adapter without using their software internally.

### 3.4 Other options (REJECTED in spike §3.5-§3.7)

- **sqlite-vec:** pre-1.0 + open Issue #220 (VACUUM-broken for vector blobs) at Atlas's exact use-case.
- **USearch:** pure-ANN; no filters; no transactions; smaller ecosystem.
- **SurrealDB:** BSL-1.1 license — non-starter for Atlas hosted-tier.
- **sqlite-vss:** abandoned (no releases since 2024).
- **Milvus-lite, ChromaDB, Vespa, Tantivy+vectors:** all violate runtime-co-resident or maturity constraints.

---

## 4. Decision

**Atlas Layer 3 implements the Mem0g cache concept as `lancedb 0.29.0` (Apache-2.0 vector store) + `fastembed-rs 5.13.4` (Apache-2.0 ONNX-CPU embedder), both pure-Rust embedded, both linked into a NEW workspace member crate `crates/atlas-mem0g/`. Atlas owns the secure-delete wrapper, the embedder-determinism pinning, the cache-invalidation strategy, and the GDPR-erasure audit-event emission.**

Sub-decisions (binding for W18b implementation — eight numbered sub-decisions follow):

### Sub-decision #1 — Vector-store + embedder choice

**LanceDB embedded + fastembed-rs embedded paired.** Apache-2.0 across the stack. Pure-Rust. Zero new sidecars. Hermes-skill distribution intact.

**Pivot path (encapsulated):** all storage interaction goes through a `SemanticCacheBackend` Rust trait (sketch in spike §7); a future Qdrant pivot is one new trait-impl crate (`crates/atlas-mem0g-qdrant/`), not an Atlas re-architecture. ~3 sessions to migrate; 100% trait + embedder + secure-delete wrapper code reused.

### Sub-decision #2 — Embedder model + determinism pinning + supply-chain controls

**`bge-small-en-v1.5` FP32 ONNX model, Atlas-pinned configuration:**
- ONNX Runtime (ORT) version locked in `Cargo.lock`.
- `OMP_NUM_THREADS=1` set programmatically before fastembed-rs init (single-thread CPU path is the deterministic path).
- Denormal-flag handling: configure `tract-onnx` runtime to denormal-treat-as-zero where supported.
- `fastembed-rs` Cargo dependency: **exact-version pin** `fastembed = "=5.13.4"` (the spike-surveyed release) pending the W18b cross-platform determinism verification test (see below). May loosen to `^5.13` only if the test confirms minor-version stability of the deterministic path. Pinning is non-negotiable because ORT-version determinism is the load-bearing claim under sub-decision #3 (cache-key strategy).

**Supply-chain controls (closes security-reviewer HIGH-2):** the model download is an Atlas-controlled fail-closed verification path, not delegated to the upstream library's default download behaviour:
- W18b records THREE values as Rust `const` in `crates/atlas-mem0g/src/embedder.rs` (compiled into the binary, not fetched at runtime):
  1. **HuggingFace model revision SHA** (the Git commit SHA of the model repo at the chosen model-card version — pins against repo-rename / repo-transfer / organisation-compromise attacks).
  2. **ONNX file SHA256** (verifies the file bytes regardless of repo-level integrity).
  3. **Optional URL pin** (full HuggingFace LFS URL incl. revision SHA in path — TLS-pinned via `rustls-native-certs` + Atlas's `reqwest` configuration).
- W18b decides between two implementation paths and documents in plan-doc:
  - **Path 1 (preferred):** Atlas wraps the download in `crates/atlas-mem0g/src/embedder.rs::download_model_with_verification()` — fetches the file via Atlas-controlled HTTP client, verifies SHA256 BEFORE handing the file path to fastembed-rs, fails closed on mismatch.
  - **Path 2 (fallback if fastembed-rs cannot accept a pre-verified local model file):** Atlas wraps fastembed-rs's download with a post-download verification check before any embedding work begins; fails closed and refuses to embed if mismatch.
- Re-verification at every cold start (verify the cached model file SHA256 matches the compiled-in constant before fastembed-rs init).
- Operator-runbook documents the model rotation process (when BAAI publishes a new revision Atlas wants to adopt: bump the three constants in source, ship a new release, customers re-cache on next cold start with fail-closed safety).

**Cross-platform determinism verification test (W18b mandatory):** new `tests/embedding_determinism.rs` runs the same input twice on Linux + Windows + macOS via CI matrix; asserts byte-identical output. If Windows fails, Atlas falls back to event_uuid-only cache-key on Windows builds (degraded performance, not degraded correctness).

### Sub-decision #3 — Cache-key strategy + Layer authority

**Cache-keys MAY use both `event_uuid` (trust anchor + Layer-1 reference) and `embedding_hash` (faster duplicate-detection).** Cache-invalidation uses `event_uuid` (stable invariant under embedder-version changes). Trust-property cite-back uses `event_uuid` always.

**Layer authority:** Mem0g indexes Layer 1 events.jsonl directly. Cache-rebuild does NOT depend on Layer-2 ArcadeDB availability. (This corrects Phase 1 Doc B's misreading flagged by `crit-architect.md` H-3.)

### Sub-decision #4 — Secure-delete primitive (GDPR Art. 17)

**Atlas wraps LanceDB native delete with a pre-capture-then-lock-then-overwrite protocol** that closes the TOCTOU race window between `cleanup_old_versions` and a concurrent compactor / read (security-reviewer HIGH-1):

```text
1. ACQUIRE write lock on the LanceDB Table for this workspace
   (W18b verifies whether lancedb 0.29 exposes a native table-level
   write lock; if not, Atlas implements per-(workspace, table)
   tokio::sync::RwLock around every Table operation. Lock held for
   the duration of the entire wrapper sequence — no concurrent
   reads/writes/compactions during overwrite.)

2. PRE-CAPTURE the set of fragment file paths that contain bytes for
   the tombstoned row(s). Use Lance's fragment metadata API
   (lancedb::Table::list_fragments + per-fragment file path enumeration)
   BEFORE any delete or cleanup operation. This is the authoritative
   set to overwrite — NOT "files modified post-cleanup", which would
   miss old-fragment paths that get unlinked-then-reused by the OS.

3. lancedb::Table::delete(filter_by_event_uuid)
   (semantic delete + tombstone in `_deletion_files/`)

4. lancedb::Table::cleanup_old_versions(Duration::ZERO).await?
   (physical rewrite into new fragment files; old fragments now
   eligible for unlink. The lock from step 1 prevents the LanceDB
   background compactor from racing this step.)

5. PRE-CAPTURE the set of LanceDB index file paths (`_indices/`) that
   reference the affected fragments. HNSW graph files are separate
   Arrow IPC files; un-overwritten, they leak the approximate-neighbour
   structure of the erased row (a weaker but non-zero side-channel,
   security-reviewer MEDIUM-2). W18b decides per the verification test:
   either (a) overwrite the affected `_indices/` files with the same
   sequence as fragments in step 6, OR (b) document the residual
   HNSW-graph-neighbourhood leak in DECISION-SEC-5 with explicit
   acknowledgement that no embedding-vector BYTES are recoverable
   from index files (only graph topology). Default: option (a) —
   overwrite both fragments AND affected index files.

6. For each pre-captured path (step 2 fragments + step 5 index files):
     OpenOptions::write(true).open(path)
     write random bytes equal to file size
     fdatasync()
     close
     tokio::fs::remove_file(path)
   This step is bounded by the pre-captured set (steps 2 + 5); the
   wrapper does NOT enumerate post-cleanup, eliminating the TOCTOU
   window.

7. RELEASE the write lock from step 1.

8. Emit Layer-1 `embedding_erased` audit-event (sub-decision #5).
   The audit-event emission is deliberately AFTER the lock release
   so it does not deadlock on the projector's own write-side
   mutex when emitting into events.jsonl.
```

**Snippet field coverage (security-reviewer MEDIUM-1):** the `SemanticHit::snippet` field (cached snippet of original event payload, see §7) is stored as a column in the SAME Arrow fragment as the embedding vector (per LanceDB's columnar storage layout). Step 6's fragment overwrite covers it. ADR makes this explicit so W18b does NOT introduce separate snippet storage (e.g. metadata sidecar, application-level cache) without adding the new location to the overwrite sequence.

**SSD wear-leveling caveat (documented in `DECISION-SEC-5` footnote):** SSD firmware may have copies of the block in spare cells unreachable by Atlas's overwrite. Full physical-erasure on SSDs requires SECURE_ERASE ATA command (whole-device, operator-runbook only) OR full-disk encryption with per-tenant key destruction (V2-γ stronger defence). W18 ships best-effort filesystem-level overwrite + documented limitation.

**W18b verification test (mandatory):** new `tests/secure_delete_correctness.rs` — write known embedding bytes + snippet; run wrapper sequence; raw-file-read of storage dir verifies original bytes are NOT recoverable via simple `fs::read`. Concurrent-write race test: spawn a parallel reader during the wrapper sequence + assert correctness still holds (lock contract honoured). Does NOT test SSD-physical-erasure (out of scope).

### Sub-decision #5 — GDPR-erasure parallel-audit-event

**New event-kind `embedding_erased`** added to projector dispatch surface. Payload shape (binding for W18b — expanded per security-reviewer MEDIUM-3 to include EU-DPA-evidentiary metadata):

```json
{
  "type": "embedding_erased",
  "event_id": "<the Layer-1 event_uuid being erased>",
  "workspace_id": "<the workspace_id whose data was erased>",
  "erased_at": "<ISO-8601 timestamp>",
  "requestor_did": "<DID of the operator or data-subject-identified-requestor — optional, defaults to the operator DID>",
  "reason_code": "gdpr_art_17"
}
```

Required fields: `event_id`, `workspace_id`, `erased_at`. Optional fields: `requestor_did` (defaults to operator DID at runtime — captured in the AtlasEvent's standard `author_did` if omitted from payload), `reason_code` (defaults to `"operator_purge"` if omitted; canonical values include `"gdpr_art_17"`, `"operator_purge"`, `"retention_policy_expiry"`).

**EU-DPA-evidentiary completeness (per `DECISION-COUNSEL-1` review pending):** `workspace_id` lets a regulator scope the erasure to a specific tenant; `requestor_did` cross-references the originating Art. 17 request log. The combination satisfies the typical EU DPA cross-reference pattern (Subject Access Request log → erasure audit-trail). Final regulator-evidentiary completeness is counsel-validated pre-V2-β-1 ship per `DECISION-COMPLIANCE-3` / `DECISION-COUNSEL-1`.

The audit event itself is a Layer-1 signed event (standard Atlas COSE_Sign1 envelope + hash chain + Rekor anchor), so the cryptographic record of erasure is itself part of the Layer-1 trust chain. **The audit-event itself is NEVER subject to secure-delete** — Layer-1 records of erasure MUST persist for regulatory traceability; sub-decision #4 secure-delete operates on Layer-3 derived data only.

**Append-only semantics:** like `anchor_created`, `embedding_erased` for the same `event_id` twice surfaces `ProjectorError::MissingPayloadField { field: "event_id '...' already has an erasure record (duplicate refused for security)" }`. Reuses existing variant pattern (preserves `#[non_exhaustive]` enum discipline).

**Variant-naming semantic-mismatch note (security-reviewer MEDIUM-3):** the `MissingPayloadField` variant reuse for "duplicate-erasure-refused" is an idempotency-guard, not a parse-failure. W18b MUST add a doc-comment in `crates/atlas-projector/src/upsert.rs::apply_embedding_erased` explaining the semantic gap so future readers don't misdiagnose the error type. A dedicated `ProjectorError::DuplicateErasureRefused` variant is V2-γ-deferred (the broader error-enum cleanup is `DECISION-ARCH-W17b` carry-over #5; W18 inherits the same V2-γ-deferred posture rather than introduce a one-off variant).

**Dispatch arm placement (W18b):** new `apply_embedding_erased` function in `crates/atlas-projector/src/upsert.rs`, dispatched from `apply_event_to_state` next to `apply_anchor_created`. Pattern mirrors W14's `anchor_created` introduction exactly.

### Sub-decision #6 — Cache-invalidation strategy

**Hybrid, all triggers Layer-1-native to honour the Layer-authority correction (sub-decision #3 — Mem0g indexes Layer 1 directly, NOT Layer 2; both reviewers flagged the original draft's Layer-2 dependency as a Layer-authority contradiction):**

1. **Background TTL:** default 5 min (configurable in operator-runbook).
2. **Explicit invalidation on `embedding_erased` audit-event** (immediate effect; not within-TTL).
3. **Layer-1 head divergence detection:** cache rebuilds if cache's last-known `events.jsonl` head-event-uuid OR byte-length-at-last-rebuild differs from current Layer-1 state. This trigger is Layer-1-only and works even if Layer-2 ArcadeDB is unavailable, preserving the Layer-3-independent-of-Layer-2 invariant.

**OPTIONAL fourth trigger (only when Layer 2 IS available + both layers consistent — diagnostic, not load-bearing):** Layer-2 `graph_state_hash` cross-check. If Layer 2 IS available AND its `graph_state_hash` does NOT match Layer 3's last-known-good projection, log a divergence warning + force a Layer-1-driven rebuild. This is a defence-in-depth signal, NOT the primary integrity mechanism — Layer 3 NEVER trusts Layer 2 for cache-validity decisions; Layer 1 is always the source.

**Concurrent rebuild race mitigation:** rebuild is content-addressed by `events.jsonl` byte-length-at-rebuild-start. If projector appends after rebuild starts, rebuild detects the delta on completion and reruns OR caches the gap and replays incrementally. Operator-runbook documents the rebuild-trigger conditions.

### Sub-decision #7 — Crate boundary

**NEW workspace member `crates/atlas-mem0g/`** rather than extending `atlas-projector`. Reasoning:
- Clean cargo + license boundary (LanceDB + fastembed-rs + Arrow + DataFusion are a substantial dep-tree; isolating them in their own crate keeps `atlas-projector`'s dep audit smaller).
- Pivot encapsulation: a future `crates/atlas-mem0g-qdrant/` parallel-crate is the cleanest swap path.
- Independent CI + reviewer dispatch: `crates/atlas-mem0g/` gets its own clippy + test lane like `atlas-projector`.

### Sub-decision #8 — Bench-test shape + timing-side-channel mitigation

**Three benches in `crates/atlas-mem0g/tests/mem0g_benchmark.rs` (W18b confirms exact path), `#[ignore]`-gated behind `ATLAS_MEM0G_BENCH_ENABLED=1`:**

| Bench | Operation | n | Measure | Target |
|---|---|---|---|---|
| **B4** | Cache-hit semantic-search | top-k=10 over 1000 vectors, n=200 queries | p50 / p95 / p99 latency (ms) | <10 ms p99 |
| **B5** | Cache-miss-with-rebuild | full rebuild over 10K-event workspace, n=10 cycles | total rebuild time (sec); per-event rebuild cost (µs) | <30 sec total |
| **B6** | Secure-delete primitive correctness | write embedding → emit `embedding_erased` → wrapper sequence → raw-file-read verification + concurrent-write race-test, n=50 cycles | binary correctness + cycle latency p99 (ms) | 100% correct |

**Atlas+Mem0g end-to-end benchmark** (master-plan §6 success criterion): combined Read-API query latency = max(L2 ArcadeDB query, L3 cache lookup) + cite-back verification. W18b CI workflow `.github/workflows/atlas-mem0g-smoke.yml` captures all benches as artifact (analog W17c pattern).

**Timing side-channel mitigation (security-reviewer MEDIUM-5):** the dual cache-key strategy (sub-decision #3 — `event_uuid` + `embedding_hash`) creates a potential timing oracle: an externally-accessible semantic-search endpoint that returns measurably-different latencies for cache-hit (embedding_hash match) vs cache-miss (rebuild required) lets an adversary infer document presence by submitting crafted queries. Particularly post-erasure: an adversary could confirm whether `embedding_erased` actually removed the cached entry. **Mitigations bound for W18b's `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts`:**
1. **Response-time normalisation:** every response delays to a configurable per-deployment minimum latency (default 50 ms); cache-hit AND cache-miss responses both wait until that minimum has elapsed before returning. Eliminates the timing distinction at the API boundary.
2. **Restrict `embedding_hash` cache-key to internal lookup only:** never expose hit-vs-miss distinction in any HTTP response header, status code, or response body field.
3. **Document this side-channel in operator-runbook + DECISION-SEC-5 footnote.** Operator MAY relax response-time normalisation for trusted internal callers (e.g. Atlas's own MCP tools where the side-channel is moot); MUST keep it for any externally-accessible endpoint.
B6 secure-delete bench-test SHOULD include a timing-distinction assertion (cache-hit and cache-miss cycle times within ±5% after normalisation).

---

## 5. Consequences

### 5.1 Accepted today

- New workspace member crate `crates/atlas-mem0g/` shipped in W18b (~1500-2000 LOC).
- LanceDB dep-tree (~200 transitive crates) added to Atlas supply chain — Atlas quarterly dep-audit MUST cover Arrow + DataFusion CVE pipeline.
- fastembed-rs ~130 MB model file downloaded + cached on first run via Atlas-controlled fail-closed verification path (per sub-decision #2 supply-chain controls): exact HuggingFace revision SHA + ONNX file SHA256 + Atlas-controlled source location for the constants. Re-verification at every cold start.
- ORT version pinning (`fastembed = "=5.13.4"` exact-version pin pending W18b cross-platform determinism verification) becomes a real maintenance burden; silent upgrade breaks determinism.
- Atlas owns secure-delete wrapper correctness — including the pre-capture-then-lock protocol that closes the TOCTOU race window AND the affected `_indices/` HNSW file overwrite step (default option (a) per sub-decision #4 step 5). W18b adds verification + race tests.
- New CI workflow `.github/workflows/atlas-mem0g-smoke.yml` (W18b).
- New event-kind `embedding_erased` added to projector dispatch surface (W18b) with EU-DPA-evidentiary payload (`event_id` + `workspace_id` + `erased_at` + optional `requestor_did` + optional `reason_code`).
- Timing side-channel mitigation (response-time normalisation default 50 ms) ships with the W18b `apps/atlas-web/.../semantic-search/route.ts` Read-API endpoint.

### 5.2 Mitigated by this design

- **GDPR Art. 17 compliance** (master-plan compliance posture): filesystem-level overwrite + parallel audit-event delivers regulator-evidentiary erasure trail; SSD-physical-erasure caveat acknowledged + V2-γ stronger defence path documented.
- **Embedding-leakage threat (Morris et al. 2023, ~92% reconstruction on the 2023-era models studied):** secure-delete primitive removes both embedding bytes (fragment overwrite) AND cached snippet (co-located in same fragment) AND HNSW index entries covering the deleted row (default option (a) per sub-decision #4 step 5; without (a), residual leak is graph-neighbourhood-topology-only — no embedding-vector bytes recoverable from `_indices/`). **Model-specific applicability caveat:** the 92% figure is benchmarked against 2023-era embedders, NOT specifically `bge-small-en-v1.5`. Newer reconstruction techniques may push past 92%; Atlas's watchlist (§8.1) tracks Morris et al. follow-up papers + `bge-small-en-v1.5`-specific reconstruction studies. Cite-back trust property routes any verification through Layer-1 events.jsonl (which carries Atlas's signed-erasure audit-event).
- **Hermes-skill cold-start budget** (V2-γ distribution channel): pure-Rust embedded means no extra runtime in the `npx` package; cold-start budget intact.
- **Vendor risk on trust-substrate critical path:** Atlas-controlled embedded stack avoids depending on Mem0 / Mem0 Cloud / OpenAI for any load-bearing trust component.
- **Byte-determinism preservation:** embeddings live OUTSIDE canonicalisation pipeline (lib.rs invariant #3 honoured); byte-pin `8962c168...e013ac4` survives W18 + W18b ship.
- **Cite-back trust property:** every `SemanticHit` carries `event_uuid`; consumer drops to Layer 1 + offline WASM verifier independently.

### 5.3 Dependencies on W18b (implementation welle)

- **W18b:** writes `crates/atlas-mem0g/` with `lib.rs` + `lancedb_backend.rs` + `embedder.rs` + `secure_delete.rs` + tests. Adds `lancedb 0.29` + `fastembed 5.13` deps. Adds `apply_embedding_erased` dispatch arm to `crates/atlas-projector/src/upsert.rs`. Adds three new tests + CI workflow + Read-API endpoint (or extends W12). Optionally adds ADR-Atlas-013 if implementation surfaces design amendment.

### 5.4 Dependencies on counsel track (Nelson-led, parallel)

- **`DECISION-COUNSEL-1` GDPR Art. 4(1) opinion:** if counsel rules unfavourably (Path-A fallback per `DECISION-COMPLIANCE-3`), embedding-storage may need adjustment (per-content salt, salt-destroyed-at-deletion). Path-A redesign is salt-management work; doesn't fundamentally change Mem0g architecture.
- **Counsel review of `embedding_erased` audit-event payload:** verifies regulator-evidentiary completeness for GDPR Art. 17 compliance claims. Pre-V2-β-1-public-materials blocking.
- Neither counsel track item blocks W18b implementation; both block V2-β-1 ship-gate per `DECISION-COUNSEL-1`.

### 5.5 SemVer impact

W18 (this ADR + spike + plan-doc): doc-only; SemVer-neutral.

W18b implementation:
- New crate `atlas-mem0g` — additive workspace member; no existing-API touched.
- New `embedding_erased` event-kind — additive dispatch arm in `crates/atlas-projector/src/upsert.rs`; no existing-kind semantics changed.
- New `ProjectorError` use-cases — reuses existing `MissingPayloadField` variant (preserves `#[non_exhaustive]` enum).
- Net: SemVer-additive across `atlas-projector` + `atlas-trust-core`. New crate `atlas-mem0g` ships at `0.1.0` (V2-β internal version aligned with workspace `2.0.0-alpha.2`).

---

## 6. Open questions (for W18b planning + V2-γ tracking)

- **OQ-1:** SSD-physical-erasure via SECURE_ERASE ATA command + per-tenant cryptographic erasure (full-disk encryption with per-tenant key destruction). V2-γ stronger defence than W18's filesystem-level overwrite. Defer to V2-γ when first EU-PII customer with strict erasure SLA signs.
- **OQ-2:** Multi-region LanceDB replication for EU-data-residency. LanceDB doesn't currently support cluster mode (embedded crate); replication = application-layer dual-write. Defer to V2-γ; Qdrant pivot (Option B above) is the better answer if multi-region becomes a hard requirement.
- **OQ-3:** Embedder-version-rotation policy. When Atlas upgrades fastembed-rs (or `bge-small-en-v1.5` → newer model), all existing embeddings need rebuild. Strategy: keep `embedder_version` field on each cached vector; rebuild lazily on next access OR background-batch on operator-runbook trigger. Lock policy in V2-γ operator-runbook.
- **OQ-4:** Re-evaluate Mem0 partnership angle for V2-γ. Atlas + Mem0 cross-promotional adapter preserves master-plan §8 partnership without using Mem0 internally. Defer to V2-γ when first 10 customers signed.
- **OQ-5:** sqlite-vec re-evaluation if Issue #220 closes + 1.0 ships. Single-file-DB advantage for self-hosted Atlas users could be compelling. Defer to V2-γ if sqlite-vec maturity story changes materially.
- **OQ-6:** Atlas-side cross-platform determinism test outcomes (V1-V4 in spike §12) — if Windows fails, formalise event_uuid-only cache-key fallback policy in operator-runbook.
- **OQ-7:** Hermes-skill operational telemetry (cache-hit-rate per skill instance, p99 latency per query). V2-γ telemetry concern; not W18 scope.
- **OQ-8:** Lance v2.2 file format `_deletion_files` semantics — verify unchanged vs v2.1 before adopting Lance 0.30+. W18b OR pre-W18b spike.
- **OQ-9:** Read-API integration pattern — transparent (Atlas Read-API queries Mem0g first, falls through to ArcadeDB on miss) vs explicit endpoint (`/api/atlas/semantic-search`). W18b ADR amendment decides; default per master-plan §6 success criterion is "both paths supported".

---

## 7. Reversibility

**Decision is MEDIUM-HIGH reversibility.**

- **LanceDB → Qdrant pivot** (Option A → Option B): encapsulated behind `SemanticCacheBackend` trait. Cost: ~3 sessions = (a) new `crates/atlas-mem0g-qdrant/` impl crate, (b) new `infra/docker-compose.qdrant-smoke.yml`, (c) new `.github/workflows/atlas-qdrant-smoke.yml`. Trait + embedder + secure-delete-wrapper code reused 100%. Pivot triggers (LP1-LP5) documented in spike §9.
- **Embedder swap** (`bge-small-en-v1.5` → newer model): config change + rebuild all existing embeddings. fastembed-rs supports multiple models out of the box. Cost: ~0.5 session + operator-runbook rebuild trigger.
- **Crate boundary swap** (`crates/atlas-mem0g/` → extension to `atlas-projector`): trivial Cargo workspace change; ~0.1 session if ever needed.
- **Cache-invalidation policy change** (TTL-only / on-event-only / state-hash-only): operator-runbook configuration; ~0 code change.
- **Secure-delete wrapper change** (filesystem-level → cryptographic-erasure for V2-γ): additive primitive on top of W18 wrapper; W18 wrapper remains the inner correctness guarantee.

No technical debt incurred; reversibility paths documented per dimension.

---

## 8. Watchlist + review cadence

### 8.1 Specific tracking signals

- **LanceDB GitHub releases:** https://github.com/lancedb/lancedb/releases — watch for 0.x → 1.0 stabilisation; verify `_deletion_files` semantics on each minor.
- **fastembed-rs releases:** https://github.com/Anush008/fastembed-rs/releases — watch for ORT-version bumps that could break determinism. Atlas's `=5.13.4` exact-version pin (sub-decision #2) means an upgrade requires a fresh cross-platform determinism verification test before lifting the pin.
- **Lance file format changes:** Lance v2.2 (mentioned in 2026 release notes as "more efficient storage") — verify `_deletion_files` semantics unchanged. Verify `list_fragments` / fragment-metadata API stability for the pre-capture step in sub-decision #4.
- **`bge-small-en-v1.5` model deprecation:** HuggingFace model card; if BAAI deprecates, Atlas swaps to a maintained replacement (e.g. `bge-large-en-v2.0`). HuggingFace revision SHA pin (sub-decision #2 supply-chain controls) means a model rotation is an explicit Atlas release event, not a silent upstream drift.
- **sqlite-vec Issue #220:** https://github.com/asg017/sqlite-vec/issues/220 — re-check at OQ-5.
- **Mem0 graph-mode packaging:** Mem0 may re-introduce a separate "Mem0g" package; Atlas posture (concept-not-product) doesn't change but spike + ADR commentary stays accurate.
- **Counsel-track deliverables:** GDPR Art. 4(1) opinion + Art. 17 `embedding_erased` payload review (Nelson update). Sub-decision #5's payload shape is counsel-pending validation per `DECISION-COUNSEL-1`.
- **Morris et al. follow-up papers + `bge-small-en-v1.5`-specific reconstruction studies:** if newer reconstruction techniques push past the 2023 92% figure (or specifically target the embedder family Atlas pins), Atlas's threat-model for embedding-leakage may need reframing. The model-specific applicability gap (§5.2) means Atlas should not assume the 2023 figure transfers cleanly to current models; track quarterly.

### 8.2 Review cadence

- **Pre-W18b start:** verify LanceDB latest stable + fastembed-rs latest stable; no breaking changes; spike §12 verification gaps (V1-V4) addressed in W18b TDD-RED tests.
- **Post-W18b ship:** review B4/B5/B6 numbers against §4 sub-decision #8 targets; if B5 cache-miss-with-rebuild exceeds 60 sec for 10K-event workspace, open follow-on ADR for cache-invalidation policy adjustment.
- **Post-V2-β-1 ship (W19):** review first customer-deployment telemetry; verify no LP1-LP5 trigger fires within first 30 days.
- **Quarterly:** re-verify §8.1 tracking signals; refresh this ADR if any signal materially changes.

### 8.3 Out-of-cadence triggers

Any one of the spike §9 thresholds LP1-LP5 firing automatically opens a follow-on ADR documenting the response (Qdrant pivot adoption or scope adjustment).

---

## 9. Decision log

| Date       | Event                                                  | Outcome |
|------------|--------------------------------------------------------|---------|
| 2026-05-12 | DECISION-SEC-5 (Phase 3): Mem0g embedding-leakage secure-deletion ACCEPT. | W18 binding constraint. |
| 2026-05-12 | DECISION-DB-3 (Phase 3): Mem0g latency-claim attribution honesty ACCEPT. | Atlas+Mem0g end-to-end bench framing locked. |
| 2026-05-13 | Master-plan §3 published with Layer 3 spec + §6 V2-β success criteria. | W18 spec scoped. |
| 2026-05-15 | W18 spike (`docs/V2-BETA-MEM0G-SPIKE.md`) completes. Six design questions answered with confidence levels. Mem0g-naming clarification surfaced. | ADR-Atlas-012 unblocked. |
| 2026-05-15 | ADR-Atlas-012 opened. LanceDB embedded + fastembed-rs paired chosen. 8 binding sub-decisions locked. Reversibility paths documented per dimension. | W18b unblocked. |
| TBD        | W18b: implements per sub-decisions #1-#8. Cross-platform determinism + secure-delete-correctness + B4/B5/B6 benches captured. | Adapter contract validated; W19 unblocked. |
| TBD        | W19 v2.0.0-beta.1 ship: convergence milestone (Layer 2 ArcadeDB + Layer 3 Mem0g + verifier-rebuild all operational). | V2-β complete. |

(Future quarterly refreshes append rows here.)

---

**End ADR-Atlas-012.**
