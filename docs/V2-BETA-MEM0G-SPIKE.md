# Atlas V2-β Welle 18 — Mem0g Layer-3 Cache Spike

> **Status:** DRAFT 2026-05-15.
> **Welle:** W18 (V2-β Phase 12).
> **Methodology:** comparative architectural analysis with WebSearch-verified 2026-05 state of all candidate libraries. **No actual benchmarks executed** — recommendations based on documented APIs, license posture, distribution-model fit against Atlas's hard constraints, and threat-model coverage. Confidence levels (HIGH/MEDIUM-HIGH/MEDIUM/LOW) stated per question. Bench numbers (B4/B5/B6) deferred to W18b CI capture.
> **Related:** ADR-Atlas-012 (this spike's binding decision doc); ADR-Atlas-010 (Layer-2 ArcadeDB structural template); `DECISION-SEC-5` (Mem0g embedding-leakage secure-deletion); `DECISION-DB-3` (Mem0g latency-claim attribution honesty); `DECISION-DB-4` (Apache-2.0 + JVM-avoidance pattern for Hermes-skill); `DECISION-ARCH-1` (V2-α byte-determinism triple-hardening).
> **Unblocks:** W18b (Mem0g implementation — ~1500-2000 LOC), W19 v2.0.0-beta.1 convergence ship.
> **Counsel disclaimer:** all GDPR Art. 17 + Art. 4(1) analysis in §4.4 + §4.5 is engineer-perspective. Counsel-validated opinion remains on Nelson's parallel counsel-engagement track per `DECISION-COUNSEL-1`. Atlas's Path-B-counsel-pending posture is the design assumption; Path-A-fallback embedding-storage adjustment is a one-decision pivot if counsel rules unfavourably.

---

## 1. Executive summary

**Recommendation:** **Atlas Layer-3 implements the Mem0g cache concept as an Atlas-native, pure-Rust embedded stack: `lancedb` (Apache-2.0 vector store) + `fastembed-rs` (Apache-2.0 ONNX-CPU embedder) running in-process with `atlas-projector`.** Confidence: **HIGH** on license + distribution + Hermes-skill compatibility; **MEDIUM-HIGH** on byte-deterministic embeddings under pinned ORT-version + thread-count + FP32; **MEDIUM** on secure-delete primitive (hand-rolled FS-overwrite wrapper around LanceDB `cleanup_old_versions`, see §4.4); **HIGH** on Atlas-controlled cite-back trust property.

**Critical clarification (factual correction surfaced by W18 research):** *"Mem0g"* is **not a separate product** — it is a research-paper name from arXiv:2504.19413 (mem0ai team, 2026) describing the *graph-augmented* memory mode in the open-source `mem0` Python package. As of 2026-05, the `--graph` CLI flag was removed from main branch in favour of a per-project config setting; what users access as "Mem0g" is `mem0` configured with graph-mode enabled. This does not change Atlas's design intent (master-plan §3 Layer-3 spec is implementation-neutral) but it does mean we are NOT choosing "Mem0g vs Rust-native" — we are choosing how Atlas implements the **Mem0g-concept Layer-3 cache** that master-plan §3 describes. The recommendation below honours the concept; the implementation is Atlas-controlled Rust.

**What this spike resolved (vs handoff §0-NEXT's 6 design questions):**

1. **Q1 Implementation choice:** LanceDB (embedded) + fastembed-rs (embedded). Mem0-Python rejected on Hermes-skill cold-start budget + cloud-default embedder + non-determinism. Qdrant sidecar reserved as pivot if Lance dep-tree audit fails. **Confidence: HIGH** on choice; pivot conditions documented in §9.
2. **Q2 Embedding determinism:** `bge-small-en-v1.5` ONNX FP32 on CPU is **deterministic** for a pinned (ORT-version, OMP_NUM_THREADS=1, denormal-flag) tuple. This means cache-keys MAY use `embedding_hash` (faster lookup) AND `event_uuid` (Layer-1 reference, trust anchor). Cache-invalidation uses `event_uuid` (stable invariant); operational lookup uses both. **Confidence: MEDIUM-HIGH** — ORT-version determinism not formally guaranteed by maintainer, requires Atlas-side verification test (W18b includes byte-equality cross-platform test).
3. **Q3 Cache invalidation strategy:** content-addressed by `events.jsonl` byte-length-at-rebuild-start + explicit invalidation on `embedding_erased` audit-events (W18b adds dispatch arm). Operator-runbook documents rebuild-trigger conditions (background TTL of 5 min OR explicit erasure event OR projector-state-hash mismatch detection). **Confidence: HIGH.**
4. **Q4 Secure-delete primitive:** LanceDB `delete()` + `cleanup_old_versions(0)` is tombstone-rewrite (NOT byte-overwrite). Atlas wraps with explicit `tokio::fs` random-fillbytes-then-`fdatasync` over the deleted fragment file paths post-cleanup. Documented in ADR §4.4. SSD wear-leveling caveat documented in `DECISION-SEC-5` footnote (full overwrite is best-effort; cryptographic erasure via per-tenant key-rotation is the V2-γ stronger defence). **Confidence: MEDIUM-HIGH** on primitive correctness; **MEDIUM** on SSD-physical-erasure guarantee.
5. **Q5 GDPR audit-event shape:** new event-kind `embedding_erased` parallels `anchor_created`. Payload: `event_id` (Layer-1 reference being erased), `erased_at` (ISO-8601), optional `reason_code` (e.g. `"gdpr_art_17"`, `"operator_purge"`). The audit event itself is a Layer-1 signed event (standard COSE_Sign1 envelope); Layer-1 trust property covers the erasure record. Dispatch arm added to `crates/atlas-projector/src/upsert.rs` in W18b. **Confidence: HIGH.**
6. **Q6 Atlas+Mem0g end-to-end benchmark:** bench-test shape mirrors W17c's `tests/arcadedb_benchmark.rs`. Three benches: B4 cache-hit semantic-search latency p50/p95/p99 (target: <10 ms p99 for top-k=10 over 1000 vectors); B5 cache-miss-with-rebuild full-rebuild cost (target: <30 sec for 10K-event workspace); B6 secure-delete primitive correctness (write embedding → erase originating event → raw-file-read verifies overwritten bytes). Numbers captured in CI artifact post-W18b. **Confidence: HIGH** on shape; numbers TBD W18b.

**Bottom line:** The Mem0g-concept Layer-3 cache is implementable as a pure-Rust Atlas-controlled embedded stack. Distribution channel (Hermes-skill `npx`) preserved; cold-start budget intact; secure-delete designed; cite-back trust property preserved end-to-end; byte-determinism CI pin survives. W18b proceeds against this design.

---

## 2. Context

### 2.1 What Layer 3 is for (master-plan §3 recap)

```
LAYER 3 — Mem0g cache    [FAST, REBUILDABLE, NEVER AUTHORITATIVE]
Embeddings · semantic search · cite-back to event_uuid
· secure-delete on GDPR erasure (overwrite, not unlink)
· failure mode: agent verifier re-queries L2 / L1 on mismatch
```

Layer 3 enables semantic-search-by-meaning (not just lookup-by-ULID) over the authoritative Layer 1 events.jsonl. A consumer asks "show me events about credit-default events for German SMEs in Q1 2026"; the cache returns top-k vectors by cosine similarity; each result carries a Layer-1 `event_uuid` so the consumer can verify the underlying event via the offline WASM verifier. The cache **never** provides trust authority — it accelerates retrieval, full stop.

### 2.2 The architectural unknowns at the start of W18

Per handoff §0-NEXT, W18 entered with 6 open design questions:

1. **Implementation choice:** `mem0` Python wrapped behind a Rust shim? Pure-Rust vector store via `qdrant-client`? Embed-first via `fastembed-rs`?
2. **Embedding determinism:** are embeddings reproducible across runs?
3. **Cache invalidation strategy:** TTL? On-demand? On `events.jsonl` append?
4. **Secure-delete primitive:** which Mem0g operation guarantees overwrite-then-unlink?
5. **GDPR audit-event shape:** what's the `kind` for the parallel audit event?
6. **Atlas+Mem0g end-to-end benchmark:** what does the benchmark measure?

§4 of this spike answers each.

### 2.3 Hard constraints (from Atlas's existing decisions)

- **License:** Apache-2.0 strongly preferred; SSPLv1 / BSL / non-commercial / proprietary is a non-starter (per `DECISION-DB-4`'s license-decisive reasoning, transferred to Layer 3).
- **Distribution:** Hermes-skill ships via `npx @atlas-trust/quickstart`. Layer 3 cannot bundle JVM (already paid by ArcadeDB sidecar) OR Python runtime. Skill must be JS-only HTTP-client.
- **Secure-delete:** GDPR Art. 17 erasure of a Layer-1 event MUST overwrite the derived embedding bytes (per `DECISION-SEC-5`). Hash-only is insufficient because embeddings can reconstruct ~92% of source content (Morris et al. 2023).
- **Byte-determinism:** lib.rs invariant #3 forbids floats in canonicalisation. Embeddings MUST live OUTSIDE `graph_state_hash`. The byte-pin `8962c168...e013ac4` MUST remain reproducible end-to-end.
- **Cite-back trust property:** every Layer-3 response MUST include `event_uuid` so the consumer can verify against Layer 1 independently.
- **Layer authority:** Mem0g indexes Layer 1 directly (per Phase-2 Architect H-3 correction). Cache-rebuild does NOT depend on Layer-2 availability.

### 2.4 Why this is a spike, not just an ADR

The implementation choice is a **one-way door** for distribution. Picking `mem0` (Python sidecar) vs `lancedb` (Rust embedded) vs `qdrant-client` (Rust client to sidecar) commits Atlas to a deployment topology that's expensive to reverse post-V2-β-1-ship. Mirrors the W16 → ADR-Atlas-010 pattern where the embedded/server-mode decision was spike-resolved before W17a's trait code landed.

---

## 3. Candidate primer (six-option survey, 2026-05)

### 3.1 Mem0 / Mem0g (mem0ai)

**License:** Apache-2.0 (verified via GitHub). Active Python project; ~30K stars; commercial entity (Mem0 Inc., $24M Series A, 2025).

**Distribution model:** Python library OR Docker server. Self-hosted Mem0 requires Python 3.10+ runtime co-resident with the Atlas daemon, OR HTTP calls to Mem0 Cloud (third-party SaaS).

**"Mem0g" packaging:** Mem0g is **not a separate package**. It refers to the graph-augmented memory mode introduced in Mem0 paper (arXiv:2504.19413, 2026). In 2026 releases, the `--graph` CLI flag was removed in favour of per-project config. What users adopt as "Mem0g" is `mem0` with graph mode enabled.

**Default embedder:** OpenAI `text-embedding-3-small` (cloud, **non-deterministic** across calls — different bytes per call for same input). Configurable to local embedder (Ollama, Hugging Face), but defaults are cloud-dependent.

**Secure-delete API:** Not exposed by Mem0 directly. Deletion delegates to backing vector store (default: local Qdrant or pgvector). No documented overwrite primitive.

**Atlas-fit verdict:** **REJECTED.** Three blockers: (a) Python runtime adds operational dependency that violates the Hermes-skill distribution constraint (Atlas would need to ship Mem0 as Docker sidecar — third process beside ArcadeDB JVM + atlas-projector Rust); (b) cloud-default embedder is non-deterministic and creates third-party dependency for a load-bearing trust-substrate component; (c) secure-delete is delegated and uncontrolled. Mem0 the company is a strong **partner** candidate (per master-plan §8) — Atlas can publish a cross-promotional adapter without using their software internally.

### 3.2 LanceDB (Rust crate `lancedb` 0.29.0, 2026-05-13)

**License:** Apache-2.0. Active development by LanceDB Inc. ~10K+ stars; weekly release cadence.

**Distribution model:** Embedded Rust crate. Links into `atlas-projector` binary. Native async-tokio API. Storage format = Apache Arrow + Lance file format (columnar, version-aware).

**Rust client maturity:** HIGH. Production deployments at major AI companies. Filtered queries + ACID transactions + multi-vector indexes (HNSW + IVF-PQ) supported.

**Secure-delete API surface:** Tombstone-only by default. `delete()` writes a deletion file (`_deletion_files/`); `compact_files()` rewrites fragments but old version files remain until `cleanup_old_versions(older_than=Duration::ZERO)`. **No byte-overwrite primitive built-in** — must wrap with our own `tokio::fs::OpenOptions` + random-overwrite-then-`fdatasync` of the fragment file paths post-cleanup. Documented design in §4.4.

**Embedding-determinism:** N/A (LanceDB stores f32 deterministically in Arrow format; determinism is a property of the embedder, not the store).

**Atlas-fit verdict:** **PRIMARY CHOICE.** Pure-Rust embedded means zero new sidecars; Apache-2.0 means clean license posture across all Atlas tiers; Hermes-skill compatibility intact (skill talks HTTP to Atlas, Atlas talks to Lance in-process). Two cons documented: (a) heavy dep tree (~200 transitive crates incl. Arrow 58 + DataFusion 53) — license/audit burden non-trivial; (b) secure-delete is hand-rolled — Atlas owns the wrapper. Both cons are addressable; the wins outweigh.

### 3.3 Qdrant server + `qdrant-client` 1.18.0 (2026-05-11)

**License:** Apache-2.0 (server + Rust client, both verified via GitHub).

**Distribution model:** Sidecar Docker process (analog ArcadeDB pattern). Atlas daemon uses gRPC client (via `tonic`). REST also exposed, so Hermes-skill could speak directly to Qdrant if needed (but per Atlas trust model, Hermes always talks to Atlas, never to a backing store).

**Rust client maturity:** HIGH. Active cadence (1.16 Nov-2025, 1.17 Feb-2026, 1.18 May-2026). Qdrant team actively maintains.

**Secure-delete API surface:** `delete_points()` removes from index + WAL; segments are immutable + rewritten by optimizer. No documented byte-overwrite. Same wrap-FS-overwrite story as LanceDB, but at **segment** granularity (coarser than Lance fragments).

**Embedding-determinism:** N/A (same as LanceDB).

**Atlas-fit verdict:** **SECONDARY CHOICE — pivot if LanceDB dep-tree audit fails.** Pivot conditions (ADR §4 + §9): (a) Arrow/DataFusion CVE-pipeline becomes unmaintainable for Atlas; (b) Atlas needs multi-tenant isolation that an in-process crate cannot provide (if a customer requires per-workspace process-isolation for their Layer-3 cache, sidecar is the model). Cons: (a) **second sidecar** doubles operational complexity (Atlas + ArcadeDB JVM + Qdrant); (b) network hop adds tail latency to a "fast cache" layer — cosmically wrong direction for Layer 3.

### 3.4 fastembed-rs 5.13.4 (2026-04-27)

**License:** Apache-2.0 (verified). Maintained by Anush008 with Qdrant-team contributions.

**Distribution model:** Embedded Rust crate. Pure ONNX Runtime (CPU). No Python, no JVM, no GPU dependency.

**Rust client maturity:** HIGH. Frequent releases. Qdrant integration is first-class but library is store-agnostic.

**Embedding-determinism:** `bge-small-en-v1.5` ONNX on CPU is **deterministic** for a given (model-file, ORT-version, thread-count, denormal-flag) tuple. Atlas pins:
- ORT version (Cargo.lock)
- `OMP_NUM_THREADS=1` (forced via `tract-onnx` or runtime env)
- FP32 (NOT quantized — quantized variants drift slightly across runtimes)
- `model-file SHA256` checksum (verified at runtime first-load)

Cross-platform determinism (Linux + Windows + macOS) requires Atlas-side verification test (W18b includes).

**Model size on disk:** ~130 MB for `bge-small-en-v1.5` FP32 (downloaded + cached on first run; cached forever after).

**Atlas-fit verdict:** **PAIRED WITH LANCEDB AS THE CHOSEN EMBEDDER.** No separate sidecar; Apache-2.0; deterministic under pinned conditions. Two cons: (a) ~130 MB model file at first run (download from HuggingFace + cache); (b) ORT version pinning is real maintenance burden — silent breakage if upgraded carelessly without re-running determinism cross-platform test.

### 3.5 sqlite-vec 0.1.9 (2026-03-31)

**License:** Apache-2.0 + MIT dual.

**Distribution model:** SQLite extension; single .db file. `rusqlite` bundled-feature works today.

**Rust client maturity:** Pre-1.0 (1.5 years post-launch). v0.1.10-alpha brings DiskANN.

**Secure-delete API surface:** SQLite `DELETE` + `VACUUM` is **known-broken** for sqlite-vec (Issue #220, still open May-2026): VACUUM does not reclaim vector blobs reliably. `PRAGMA secure_delete=ON` zero-fills pages but Issue #220's blob-reclamation gap means some vector bytes survive. SSD-wear-leveling caveat applies on top.

**Atlas-fit verdict:** **REJECTED.** Two blockers: (a) pre-1.0 maturity for a load-bearing trust-substrate component; (b) Issue #220 hits Atlas's exact use-case (vector blob secure-delete). Re-evaluate at sqlite-vec 1.0+ if Issue #220 closes.

### 3.6 USearch 2.25.x (Unum, May-2026)

**License:** Apache-2.0.

**Distribution model:** Embedded single-header C++ + Rust binding. Lighter than LanceDB.

**Rust client maturity:** MID. Smaller community than LanceDB.

**Secure-delete API surface:** HNSW index in single file. `remove()` marks-as-deleted; compaction = full rebuild. No secure-delete; same FS-overwrite wrapper needed.

**Atlas-fit verdict:** **REJECTED.** Pure-ANN library — no filtered queries, no transactions, no `event_uuid → vector_id` mapping (Atlas would need a side-table). Smaller ecosystem; fewer Atlas-shaped production case studies than LanceDB. LanceDB's filter+ACID story wins.

### 3.7 Eliminated outright (one-line rationale each)

- **SurrealDB** — BSL-1.1 license. Non-starter for Atlas hosted-tier per `DECISION-DB-4` reasoning.
- **sqlite-vss** — superseded by sqlite-vec by same author; no new releases since 2024.
- **Milvus-lite, ChromaDB, Vespa** — all require Python or JVM runtime co-resident; violates "no second JVM, no Python" constraint.
- **Tantivy + vectors** — Tantivy's vector support still experimental in 2026; BM25-first; wrong tool.

---

## 4. The 6 design questions answered

### 4.1 Q1 — Implementation choice (one-way door)

**Question:** `mem0` Python? `qdrant-client` Rust to sidecar? `fastembed-rs` embedded? Other?

**Decision:** **LanceDB embedded crate + fastembed-rs embedded embedder, both pure-Rust, both Apache-2.0.**

**Reasoning:**
- **License-compatible across all Atlas tiers** — analog DECISION-DB-4 reasoning transferred to Layer 3. Apache-2.0 means hosted-tier clean.
- **Hermes-skill cold-start budget intact** — no second sidecar, no Python runtime. Atlas ships as Rust binary (atlas-projector linked with lancedb + fastembed-rs) + ArcadeDB JVM sidecar. Hermes `npx` skill talks HTTP to atlas-projector; never sees Lance or fastembed-rs directly.
- **Atlas-controlled secure-delete** — we own the wrapper around `delete()` + `cleanup_old_versions()` + FS-overwrite. We don't depend on Mem0's roadmap or Qdrant's segment-rewrite implementation details.
- **Embedder-determinism in our control** — pinned ORT version + thread count + FP32 model gives us deterministic embeddings. Cache-keys can use embedding-hash (faster) AND event_uuid (trust anchor).
- **No vendor risk on the trust-substrate critical path** — LanceDB Inc. is well-funded but even if LanceDB pivots, the wrapper is small enough for Atlas to swap stores (pivot to Qdrant sidecar = ~3 sessions, documented in §9).

**Confidence:** **HIGH** on LanceDB + fastembed-rs choice. **MEDIUM-HIGH** on the LanceDB dep-tree audit (Arrow 58 + DataFusion 53 + ~200 transitive crates is a real maintenance + audit surface; counsel-engagement and Atlas's quarterly dep-tree-CVE review must include it).

**Pivot path (if LanceDB dep-tree audit fails):** Qdrant sidecar + fastembed-rs. Same embedder; swap the store; accept the second Docker process. ~3 sessions to migrate; W18b code design supports it because the choice is encapsulated behind a Rust trait (analog `GraphStateBackend` for Layer 2; W18b will define `SemanticCacheBackend`).

### 4.2 Q2 — Embedding determinism (cache-key invariant)

**Question:** Are embeddings reproducible across runs? If not, only `event_uuid` is a stable cache-key.

**Decision:** **Embeddings ARE deterministic under Atlas's pinned configuration.** Cache-keys may use BOTH `event_uuid` (trust anchor + Layer-1 reference) AND `embedding_hash` (faster duplicate-detection). Cache-invalidation uses `event_uuid` (stable invariant under embedder-version changes).

**Reasoning:**
- `bge-small-en-v1.5` ONNX FP32 on CPU is documented as deterministic by Sentence-Transformers + ONNX Runtime maintainers when (model-file, ORT-version, OMP_NUM_THREADS=1, denormal-flag-disabled) are fixed.
- Atlas pins all four:
  - Model-file SHA256 verified at runtime first-load (fail-closed if mismatch).
  - ORT version locked in `Cargo.lock`.
  - `OMP_NUM_THREADS=1` set programmatically before fastembed-rs init.
  - Denormal-flag handling: `fastembed-rs` uses `tract-onnx` runtime which can be configured; if not, fall back to single-thread CPU + denormal-treat-as-zero env.
- Cross-platform determinism (Linux + Windows + macOS) is the verification gap — Atlas-side test required.

**Verification gate (W18b TDD test):** new `tests/embedding_determinism.rs` — embeds the same input twice on the local platform AND in CI Linux runners; asserts byte-identical output. If the test fails on Windows, Atlas falls back to event_uuid-only cache-key on Windows builds (degraded performance, not degraded correctness).

**Confidence:** **MEDIUM-HIGH** on determinism under pinned conditions. **MEDIUM** on cross-platform determinism — formal guarantee from maintainer not found; Atlas-side test mandatory.

### 4.3 Q3 — Cache invalidation strategy

**Question:** Rebuild on every projector run? TTL? Explicit signal? Hybrid?

**Decision:** **Hybrid, all three primary triggers Layer-1-native** to honour the Layer-authority correction (Q1 — Mem0g indexes Layer 1 directly, NOT Layer 2; both reviewers flagged the original draft's Layer-2 dependency as a Layer-authority contradiction). (a) Background TTL (default 5 min, configurable); (b) explicit invalidation on `embedding_erased` audit-event; (c) Layer-1 head divergence detection (cache rebuilds if cache's last-known `events.jsonl` head-event-uuid OR byte-length-at-last-rebuild differs from current Layer-1 state — works even when Layer 2 is unavailable). PLUS optional defence-in-depth (d): Layer-2 `graph_state_hash` cross-check ONLY when Layer 2 IS available; diagnostic-only, NOT load-bearing for cache validity.

**Reasoning:**
- **Pure on-every-projector-run:** wasteful (10K events triggers 10K rebuilds when 99.99% of cache is still valid).
- **Pure TTL:** misses GDPR-erasure events between TTLs (Art. 17 erasure must be effective immediately, not within-5-min).
- **Pure on-`events.jsonl`-append:** races with concurrent projector runs (rebuild produces stale snapshot if projector appends mid-rebuild).
- **Layer-2-state-hash-only (the original draft's third trigger):** breaks the Layer-3-independent-of-Layer-2 invariant — if Layer 2 is unavailable, the safety net silently fails.
- **Hybrid Layer-1-native triple:** TTL handles steady-state freshness; explicit erasure-event handles GDPR; Layer-1-head-divergence handles projector-state divergence detection without Layer-2 dependency. Layer-2 cross-check is opportunistic defence-in-depth.

**Concurrent rebuild race mitigation:** rebuild is content-addressed by `events.jsonl` byte-length-at-rebuild-start. If projector appends after rebuild starts, rebuild detects the delta on completion and reruns OR caches the gap and replays incrementally. ADR §4 sub-decision #6 documents the consistency model.

**Confidence:** **HIGH.**

### 4.4 Q4 — Secure-delete primitive (GDPR Art. 17)

**Question:** Which operation guarantees overwrite-then-unlink? If chosen tool doesn't expose it, Atlas wraps explicitly.

**Decision:** **Atlas wraps LanceDB `delete()` + `cleanup_old_versions(0)` with a pre-capture-then-lock-then-overwrite protocol over the affected fragment files AND `_indices/` HNSW files. Full sequence and rationale lock in ADR-Atlas-012 §4 sub-decision #4** (post-reviewer-revision; the original "enumerate post-cleanup" approach had a TOCTOU race that the lock + pre-capture protocol closes).

**Why pre-capture-then-lock (security-reviewer HIGH-1 close):**
- "Enumerate post-cleanup" allows a concurrent compactor or read to (a) re-create fragment files at the same paths (Atlas overwrites live data) or (b) produce intermediate files Atlas misses entirely. The TOCTOU window is real.
- Pre-capture: enumerate fragment paths via Lance's fragment metadata API BEFORE delete, hold a write lock for the entire wrapper sequence, then overwrite the pre-captured set after `cleanup_old_versions`. Lock prevents concurrent compaction; pre-capture is the authoritative set to overwrite.

**Why HNSW index files (security-reviewer MEDIUM-2 close):** un-overwritten `_indices/` files leak the approximate-neighbour structure of the erased row (graph topology, NOT embedding bytes). Default option (a): overwrite affected `_indices/` files alongside fragments. Option (b) (document the residual graph-topology leak) is alternate; default chosen because the cost is small and the gap-acknowledgement framing fragments Atlas's regulator-evidentiary story.

**SSD wear-leveling caveat (documented in DECISION-SEC-5 footnote):** SSD firmware may have copies of the block in spare cells unreachable by Atlas's overwrite. Full physical-erasure guarantee on SSDs requires SECURE_ERASE ATA command (whole-device, operator-runbook only) OR full-disk encryption with per-tenant key destruction (V2-γ stronger defence — cryptographic erasure renders blocks unreadable even if recovered). W18 ships best-effort filesystem-level overwrite + documented limitation.

**Verification (W18b TDD test):** new `tests/secure_delete_correctness.rs` — write known embedding bytes + snippet to LanceDB; run wrapper sequence; raw-file-read of the storage dir verifies original bytes are NOT recoverable via simple `fs::read`. PLUS concurrent-write race test: spawn parallel reader/compactor during the wrapper sequence + assert correctness still holds (lock contract honoured). Does NOT test SSD-physical-erasure (out of scope).

**Snippet field coverage (security-reviewer MEDIUM-1 close):** the `SemanticHit::snippet` field (see §7) is co-located in the same Arrow fragment as the embedding vector (LanceDB columnar layout). Fragment overwrite covers it. ADR §4 sub-decision #4 makes this explicit so W18b does NOT introduce separate snippet storage without adding the new location to the overwrite sequence.

**Confidence:** **MEDIUM-HIGH** on filesystem-level overwrite correctness (post-reviewer revision — TOCTOU race closed); **MEDIUM** on SSD-physical-erasure (acknowledged limitation; V2-γ cryptographic-erasure is the longer-term mitigation).

### 4.5 Q5 — GDPR-erasure parallel-audit-event shape

**Question:** What's the `kind` for the erasure audit event? Reuse existing kind or new one?

**Decision:** **NEW event-kind `embedding_erased`.** Parallels `anchor_created` shape — minimal payload, append-only semantics, security-conservative refusal of duplicates.

**Reasoning:**
- Reusing `annotation_add` would conflate annotation-write semantics with erasure-record semantics — bad type discipline.
- Reusing `policy_set` would conflate policy-attachment with erasure — bad type discipline.
- New kind preserves Atlas's per-event-kind dispatch surface clarity.

**Payload shape (binding for W18b — final shape in ADR-Atlas-012 §4 sub-decision #5; expanded per security-reviewer MEDIUM-3 to include EU-DPA-evidentiary metadata):**
```json
{
  "type": "embedding_erased",
  "event_id": "<the Layer-1 event_uuid being erased>",
  "workspace_id": "<the workspace_id whose data was erased>",
  "erased_at": "<ISO-8601 timestamp>",
  "requestor_did": "<DID of the operator or data-subject-identified-requestor — optional, defaults to operator DID>",
  "reason_code": "gdpr_art_17"  // optional; defaults to "operator_purge"
}
```

`workspace_id` lets a regulator scope the erasure to a specific tenant; `requestor_did` cross-references the originating Art. 17 request log. This satisfies the typical EU DPA cross-reference pattern (Subject Access Request log → erasure audit-trail). Final regulator-evidentiary completeness is counsel-validated pre-V2-β-1 ship.

**Trust property:** the audit event is itself a Layer-1 signed event (standard Atlas COSE_Sign1 envelope + hash chain + Rekor anchor). The cryptographic record of "this PII was erased on this date by this DID" is therefore part of the Layer-1 trust chain. **Architectural posture (per `DECISION-COMPLIANCE-4` reframing): Atlas designs for "regulator-friendly" erasure-evidentiary completeness, NOT "regulator-approved". Final regulator-evidentiary completeness is counsel-validated pre-V2-β-1 ship per `DECISION-COUNSEL-1` / `DECISION-COMPLIANCE-3`** — this spike's design assumption is Path-B-counsel-pending; Path-A salt-redesign fallback is the documented contingency if counsel rules unfavourably. Independent-third-party verification (regulator OR auditor OR data-subject) of erasure happens via the offline WASM verifier reading Layer-1 events.jsonl + the `embedding_erased` event's signed attestation chain.

**Append-only semantics:** like `anchor_created`, `embedding_erased` for the same `event_id` twice surfaces `ProjectorError::MissingPayloadField { field: "event_id '...' already has an erasure record (duplicate refused for security)" }`. Erasure is logically idempotent; double-erasure indicates either operator error or replay attack.

**Variant-naming semantic-mismatch note (security-reviewer MEDIUM-3):** the `MissingPayloadField` variant reuse for a "duplicate-erasure-refused" condition is an idempotency-guard, not a parse-failure. W18b MUST add a doc-comment in `apply_embedding_erased` explaining the semantic gap. A dedicated `ProjectorError::DuplicateErasureRefused` variant is V2-γ-deferred (consistent with `DECISION-ARCH-W17b` carry-over #5 — broader error-enum cleanup is V2-γ scope).

**Dispatch arm placement:** new `apply_embedding_erased` function in `crates/atlas-projector/src/upsert.rs`, dispatched from `apply_event_to_state` next to `apply_anchor_created`. Pattern mirrors W14's `anchor_created` introduction exactly.

**Confidence:** **HIGH.**

### 4.6 Q6 — Atlas+Mem0g end-to-end benchmark shape

**Question:** What does the Atlas+Mem0g benchmark measure? p50/p99 of which ops?

**Decision:** **Three benches, mirror W17c's `tests/arcadedb_benchmark.rs` `#[ignore]`-gated pattern:**

| Bench | Operation | n | Measure |
|---|---|---|---|
| **B4** | Cache-hit semantic-search | top-k=10 over 1000 vectors, n=200 queries | p50 / p95 / p99 latency (ms) |
| **B5** | Cache-miss-with-rebuild | full rebuild over 10K-event workspace, n=10 cycles | total rebuild time (sec); per-event rebuild cost (µs) |
| **B6** | Secure-delete primitive correctness | write embedding → emit `embedding_erased` → cleanup → raw-file-read verification, n=50 cycles | binary correctness (pass/fail) + cycle latency p99 (ms) |

**Gate:** `#[ignore]`-gated behind env var `ATLAS_MEM0G_BENCH_ENABLED=1`. CI workflow `.github/workflows/atlas-mem0g-smoke.yml` (W18b) sets the env var; local `cargo test` skips by default.

**Comparison baseline:** Layer-2 ArcadeDB B3 sorted-read p99 = 26 ms (W17c local Windows Docker Desktop). Layer-3 cache-hit B4 target: <10 ms p99. **Cache-miss-with-rebuild B5 has no comparable baseline** — it's a new operation; the metric becomes the operator-runbook's documented rebuild-trigger cost.

**Atlas+Mem0g end-to-end benchmark (master-plan §6 success criterion):** combined Read-API query latency = max(L2 query, L3 cache lookup) + cite-back verification. Bench captured in W18b post-implementation.

**Confidence:** **HIGH** on shape; numbers TBD W18b.

---

## 5. Trade-off matrix — LanceDB embedded vs Qdrant sidecar vs Mem0 Python

| Dimension | LanceDB embedded (PRIMARY) | Qdrant sidecar (PIVOT) | Mem0 Python sidecar (REJECTED) | Weight | Winner |
|---|---|---|---|---|---|
| License | Apache-2.0 | Apache-2.0 | Apache-2.0 | HIGH | Tie |
| Distribution / Hermes-skill compat | Compatible (Atlas talks Lance in-process; Hermes talks HTTP to Atlas) | Compatible (sidecar; Hermes via Atlas HTTP) | **BLOCKER** (Python runtime co-resident OR third-party Mem0 Cloud) | DECISIVE | **LanceDB** |
| Operational complexity | 1 binary (Rust) + 1 sidecar (ArcadeDB JVM) | 2 sidecars (ArcadeDB JVM + Qdrant) | 3 process-types (Atlas Rust + ArcadeDB JVM + Mem0 Python) | HIGH | **LanceDB** |
| Atlas-controlled secure-delete | Hand-rolled wrapper around `cleanup_old_versions` + FS-overwrite — Atlas owns it | Hand-rolled wrapper around segment-rewrite; coarser granularity | Delegated to backing store; Atlas does NOT control | HIGH | **LanceDB** (fragment-granularity) |
| Embedding-determinism control | Atlas pins (model + ORT + thread + FP32) | Same (Atlas owns embedder either way) | Cloud-default non-deterministic; configurable to local but not enforced | HIGH | LanceDB (clean default) |
| Vendor risk | LanceDB Inc. funded; pivot to Qdrant is encapsulated behind a trait | Qdrant team well-funded; same trait-pivot story | Mem0 Inc. $24M Series A but startup; trust-substrate critical-path vendor risk | MED-HIGH | Tie (Lance/Qdrant); Mem0 worst |
| Network hop (latency) | Zero (in-process) | One (gRPC localhost) | One (HTTP localhost; or two if Mem0 Cloud) | MED | **LanceDB** |
| Dep-tree maintenance burden | HIGH (~200 transitive crates incl. Arrow + DataFusion) | LOW-MED (gRPC client only; sidecar self-contained) | LOW-MED on Atlas-side (HTTP client only) | MED | Qdrant (marginal) |
| Atlas trust-property purity | HIGH (Atlas controls all of the cache-state-and-deletion lifecycle) | HIGH (sidecar + Atlas-controlled adapter) | LOW (Mem0 abstracts away the storage; Atlas trust-property leaks through Mem0's choices) | DECISIVE | **LanceDB** (in-process control) |
| Hermes-skill cold-start budget | Intact (Atlas binary + Java; no extra runtime) | Slightly worse (extra Docker pull on first install) | **BLOCKER** (Python runtime) | DECISIVE | **LanceDB** |
| EU-data-residency deployment | Single Atlas binary; trivial | Two artefacts; both EU-hostable | Three artefacts incl. Mem0 Cloud risk | MED | LanceDB |

**Net assessment:** LanceDB embedded wins on every decisive dimension. Qdrant is the documented pivot (one-trait-impl-swap); Mem0 is rejected on three independent blockers.

---

## 6. Recommendation

**Atlas Layer 3 implements the Mem0g cache concept as `lancedb` (Apache-2.0 vector store) + `fastembed-rs` (Apache-2.0 ONNX-CPU embedder), both pure-Rust embedded, both linked into `atlas-projector`.**

Confidence by dimension:
- License + distribution + Hermes-skill compatibility: **HIGH**
- Embedding-determinism under pinned ORT + threads=1 + FP32: **MEDIUM-HIGH** (Atlas-side cross-platform verification test mandatory)
- Secure-delete primitive (filesystem-level overwrite wrapper): **MEDIUM-HIGH**
- Secure-delete (SSD physical-erasure): **MEDIUM** (acknowledged limitation; V2-γ cryptographic-erasure is the longer-term mitigation)
- Cite-back trust property end-to-end: **HIGH**
- Bench-shape design: **HIGH** (numbers TBD W18b)

**What this enables for W18b:**
- W18b creates **NEW workspace member crate `crates/atlas-mem0g/`** (LOCKED per ADR §4 sub-decision #7 — clean cargo + license boundary; pivot encapsulation; independent CI + reviewer dispatch).
- W18b adds `lancedb = "0.29.0"` + `fastembed = "=5.13.4"` deps (exact-version pins per ADR §4 sub-decision #2).
- W18b adds `apply_embedding_erased` dispatch arm to `crates/atlas-projector/src/upsert.rs` with EU-DPA-evidentiary payload (`event_id` + `workspace_id` + `erased_at` + optional `requestor_did` + optional `reason_code` per ADR §4 sub-decision #5).
- W18b adds `crates/atlas-mem0g/tests/embedding_determinism.rs` (cross-platform CI matrix: Linux + Windows + macOS), `crates/atlas-mem0g/tests/secure_delete_correctness.rs` (incl. concurrent-write race-test), `crates/atlas-mem0g/tests/mem0g_benchmark.rs` (B4/B5/B6 incl. timing-distinction assertion under response-time normalisation).
- W18b adds `.github/workflows/atlas-mem0g-smoke.yml` CI workflow (Linux Ubuntu lane analog W17c pattern; HuggingFace model file cached across runs).
- W18b adds `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts` Read-API endpoint with response-time normalisation (default 50 ms minimum) per ADR §4 sub-decision #8 timing-side-channel mitigation. Decision on transparent-vs-explicit-endpoint pattern (W12 extension vs new endpoint) deferred to W18b ADR amendment.

---

## 7. SemanticCacheBackend trait sketch (Rust pseudo-code)

```rust
// crates/atlas-mem0g/src/lib.rs (W18b implements)
// SKETCH — W18b writes the production version

use anyhow::Result;
use atlas_trust_core::{EntityUuid, WorkspaceId};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticHit {
    /// Layer-1 anchor — the trust-substrate cite-back identifier.
    /// ALWAYS present in every hit. Trust property: caller can verify
    /// this event_id against events.jsonl via offline WASM verifier
    /// independent of the cache.
    pub event_uuid: String,
    pub workspace_id: WorkspaceId,
    /// Optional Layer-2 vertex reference (if the hit corresponds to a
    /// projected entity).
    pub entity_uuid: Option<EntityUuid>,
    /// Cosine similarity score [0, 1]. Diagnostic only — caller MUST
    /// NOT use this as a trust signal.
    pub score: f32,
    /// Cached snippet of the original event payload. Subject to GDPR
    /// erasure — `embedding_erased` event removes both the embedding
    /// AND this snippet.
    pub snippet: String,
}

/// Layer-3 semantic cache backend abstraction.
/// V2-β W18b: LanceDbCacheBackend (default).
/// V2-γ pivot candidate: QdrantCacheBackend (sidecar).
pub trait SemanticCacheBackend: Send + Sync {
    /// Upsert an embedding for a Layer-1 event. Idempotent on event_uuid.
    /// Embedder is owned by the cache; caller passes raw text.
    fn upsert(
        &self,
        workspace_id: &WorkspaceId,
        event_uuid: &str,
        text: &str,
    ) -> Result<()>;

    /// Top-k semantic search. Returns hits sorted by descending score.
    /// Each hit MUST carry event_uuid for cite-back trust.
    fn search(
        &self,
        workspace_id: &WorkspaceId,
        query: &str,
        k: usize,
    ) -> Result<Vec<SemanticHit>>;

    /// Erase the embedding + snippet for a Layer-1 event. MUST overwrite
    /// the on-disk bytes (per DECISION-SEC-5); caller responsible for
    /// emitting the parallel embedding_erased Layer-1 audit event.
    fn erase(
        &self,
        workspace_id: &WorkspaceId,
        event_uuid: &str,
    ) -> Result<()>;

    /// Rebuild the cache from a Layer-1 events.jsonl iterator. Used for
    /// the periodic rebuild trigger and projector-state-hash-mismatch
    /// recovery.
    fn rebuild(
        &self,
        workspace_id: &WorkspaceId,
        events: Box<dyn Iterator<Item = atlas_trust_core::AtlasEvent> + Send + '_>,
    ) -> Result<()>;

    /// Backend identity string for ProjectorRunAttestation chain
    /// (analog GraphStateBackend::backend_id).
    fn backend_id(&self) -> &'static str;  // "lancedb-fastembed" | "qdrant-fastembed"
}
```

**W18b notes:**
- The trait is `Send + Sync` for tokio multi-task semantic-search.
- Embedder ownership is INSIDE the backend (caller passes raw text, not vectors) — keeps the embedder-version pin under the trait's control + makes embedder-swap a single-trait-impl swap.
- `rebuild` is the cache-miss-recovery path; iterator-based to support 10M-event workspaces without loading everything into memory.
- **Sync-vs-async pattern (security-reviewer / code-reviewer MEDIUM-3 close):** the trait methods are declared synchronous (`fn`, not `async fn`) intentionally — analog the Layer-2 `GraphStateBackend` trait which uses `reqwest::blocking` to keep the trait surface object-safe without `async_trait` macro overhead. LanceDB's Rust API is async-first, so W18b implementations MUST wrap LanceDB calls with `tokio::task::spawn_blocking` (NOT `tokio::runtime::Handle::current().block_on()` — the latter deadlocks under the single-threaded tokio scheduler when called from inside an async context). Documented in W18b plan-doc as a TDD-RED-cycle test (call `SemanticCacheBackend::search` from inside a tokio task; assert no deadlock).

---

## 8. CI strategy

**Existing 7 byte-determinism CI pins (preserved):**
1. COSE_Sign1 signing-input byte pin (V1)
2. COSE_Sign1 signature byte pin (V1)
3. COSE_Sign1 envelope byte pin (V1)
4. Sigstore Rekor anchor byte pin (V1)
5. Anchored-bundle byte pin (V1)
6. pubkey-bundle byte pin (V1)
7. `graph_state_hash` canonicalisation byte pin (V2-α Welle 3)

**NEW workflow for W18b (this spike proposes):** `.github/workflows/atlas-mem0g-smoke.yml`. Linux Ubuntu lane; SHA-pinned actions; `permissions: contents: read`; paths-gated; 10 min timeout. Runs:
1. Checkout
2. Cache the fastembed-rs model file (~130 MB) — saves 1-2 min per run
3. `cargo test -p atlas-mem0g --test embedding_determinism --test secure_delete_correctness` (the always-on tests)
4. `ATLAS_MEM0G_BENCH_ENABLED=1 cargo test -p atlas-mem0g --test mem0g_benchmark -- --ignored --nocapture` (the bench, captured to artifact)
5. Upload bench artifact (B4/B5/B6 numbers)

**Cross-check matrix (W18b acceptance):** byte-pin `8962c168...e013ac4` MUST reproduce after W18b adds `atlas-mem0g` crate to the workspace. Run `cargo test -p atlas-projector --test backend_trait_conformance byte_pin --quiet` in CI as final go/no-go.

---

## 9. Pivot trigger thresholds (LanceDB → Qdrant sidecar)

**5 measurable criteria (none currently fired):**

| # | Trigger | Measurable threshold | Source signal |
|---|---|---|---|
| LP1 | LanceDB dep-tree CVE-pipeline becomes unmaintainable | ≥3 unfixed CVEs in Arrow / DataFusion / transitive crates open >30 days | Atlas quarterly dep-audit |
| LP2 | LanceDB `cleanup_old_versions` cross-platform behaviour breaks | W18b cross-platform test (Linux + Windows + macOS) fails on any platform after Lance 0.30+ release | W18b CI |
| LP3 | LanceDB project deprecation / acquisition / license pivot | Public announcement / repo archive / LICENSE diff | GitHub repo watch |
| LP4 | Customer requires per-workspace process-isolation for Layer 3 | First enterprise-tier customer with explicit isolation requirement | Sales / customer-success |
| LP5 | LanceDB embedded becomes >10x slower than Qdrant sidecar at Atlas scale (10M-event workspace) | W18b bench B4/B5 baseline + future ops-telemetry | Production telemetry |

**Pivot cost:** ~3 sessions. Implementation is encapsulated behind `SemanticCacheBackend` trait (§7). Swap = (a) new `crates/atlas-mem0g-qdrant/` crate with `QdrantCacheBackend` impl, (b) new `infra/docker-compose.qdrant-smoke.yml`, (c) new `.github/workflows/atlas-qdrant-smoke.yml`. The trait + Atlas-side embedder + secure-delete-wrapper code is reused 100%.

**Embedder pivot (separate concern):** if `bge-small-en-v1.5` becomes unsuitable, fastembed-rs supports multiple models; swap is a config change. Documented in operator-runbook.

---

## 10. W18b entry criteria

**Required before W18b starts (all satisfied by W18 merge):**
- [x] DECISION-SEC-5 secure-delete contract bound to Atlas-controlled wrapper (§4.4)
- [x] DECISION-DB-3 latency-claim attribution preserved (no Atlas-claimed Mem0g performance numbers; Atlas+Mem0g end-to-end is W18b's bench)
- [x] DECISION-DB-4 Apache-2.0 + JVM-avoidance pattern transferred to Layer 3
- [x] Embedding-determinism story committed (pinned ORT + threads=1 + FP32 + cross-platform-verification-test mandatory)
- [x] Cache-key strategy locked (event_uuid for trust + cache-invalidation; embedding_hash optional for fast-lookup)
- [x] Layer authority corrected (Mem0g indexes Layer 1 directly; NOT via Layer 2)
- [x] GDPR audit-event shape locked (`embedding_erased` payload + dispatch-arm placement)
- [x] Bench-test shape locked (B4/B5/B6 mirroring W17c B1/B2/B3 pattern)
- [x] Pivot trigger thresholds documented (LP1-LP5)
- [x] `SemanticCacheBackend` trait sketched (§7)
- [x] CI workflow sketched (`.github/workflows/atlas-mem0g-smoke.yml`, §8)

**W18b's required deliverables (~1500-2000 LOC):**
- New crate `crates/atlas-mem0g/` with `lib.rs` + `lancedb_backend.rs` + `embedder.rs` + `secure_delete.rs` + tests
- `lancedb 0.29` + `fastembed 5.13` dep additions
- New `apply_embedding_erased` dispatch arm in `crates/atlas-projector/src/upsert.rs`
- New `tests/embedding_determinism.rs`, `tests/secure_delete_correctness.rs`, `tests/mem0g_benchmark.rs`
- New `.github/workflows/atlas-mem0g-smoke.yml`
- New `apps/atlas-web/src/app/api/atlas/semantic-search/route.ts` (or extension to existing W12 endpoint)
- ADR-Atlas-013 if W18b implementation surfaces design-amendment needs

---

## 11. Open questions for V2-γ (not blocking V2-β)

- **OQ-1:** SSD-physical-erasure via SECURE_ERASE ATA command + per-tenant cryptographic erasure (full-disk encryption with per-tenant key destruction). V2-γ stronger defence than W18's filesystem-level overwrite. Defer to V2-γ when first EU-PII customer with strict erasure SLA signs.
- **OQ-2:** Multi-region LanceDB replication for EU-data-residency. LanceDB doesn't currently support cluster mode (embedded crate); replication = application-layer dual-write. Defer to V2-γ; Qdrant pivot is the better answer if multi-region becomes a hard requirement.
- **OQ-3:** Embedder-version-rotation policy. When Atlas upgrades fastembed-rs (or `bge-small-en-v1.5` → newer model), all existing embeddings need rebuild. Strategy: keep `embedder_version` field on each cached vector; rebuild lazily on next access OR background-batch on operator-runbook trigger. Lock policy in V2-γ operator-runbook.
- **OQ-4:** Re-evaluate Mem0 partnership angle for V2-γ. Atlas + Mem0 cross-promotional adapter (Mem0 publishes "use Atlas for crypto-trust"; Atlas publishes "use Mem0 for memory-management abstractions on top of Atlas") preserves the master-plan §8 partnership without using Mem0 internally. Defer to V2-γ when first 10 customers signed.
- **OQ-5:** sqlite-vec re-evaluation if Issue #220 closes + 1.0 ships. Single-file-DB advantage for self-hosted Atlas users could be compelling. Defer to V2-γ if sqlite-vec maturity story changes materially.
- **OQ-6:** Embedding model upgrade path. `bge-small-en-v1.5` is 2024; newer models (`bge-large-en-v2.0`?) may be available by V2-γ. Verify pinned-determinism story holds for the new model before swapping.
- **OQ-7:** Hermes-skill operational metrics (cache-hit-rate per skill instance, p99 latency per query). V2-γ telemetry concern; not W18 scope.

---

## 12. Verification gaps to close pre-W18b (Atlas-side tests)

These are the items the research agent flagged as "could not verify from public docs alone" — Atlas-side tests required before W18b ADR amendment confidence reaches HIGH:

| # | Item | Verification method | Owner |
|---|---|---|---|
| V1 | LanceDB `cleanup_old_versions` behaviour on Windows | 50-line Rust integration test on Windows CI runner | W18b |
| V2 | fastembed-rs determinism across ORT minor versions | 2-run byte-equality test on bge-small-en-v1.5 FP32 + `OMP_NUM_THREADS=1` on Linux + Windows + macOS | W18b |
| V3 | Lance v2.2 `_deletion_files` semantics unchanged vs v2.1 | Spike test before adopting Lance 0.30+ | W18b OR pre-W18b spike |
| V4 | fastembed-rs model size on disk (claimed ~130 MB) | First-load measurement in W18b CI | W18b |
| V5 | Qdrant 1.18 server license (Apache-2.0 confirmed for repo, but Qdrant-Cloud-specific feature licensing not relevant if self-hosted) | Documented as not-applicable; Atlas self-hosts the pivot path | W18 (already confirmed) |
| V6 | Mem0g graph-mode current packaging (research confirmed it's a paper-name not separate product) | Documented in §3.1; not a Atlas-decision-affecting concern post-rejection | W18 (closed) |
| V7 | sqlite-vec Issue #220 status (May-2026 movement check) | Re-check at OQ-5 V2-γ re-evaluation; not blocking | V2-γ |

**None of V1-V4 block W18 (this spike) — they block W18b implementation confidence.** W18b adds the verification tests as part of TDD-RED-cycle for each.

---

## 13. Reviewer focus suggestions (for W18 parent reviewer dispatch)

When parent dispatches `code-reviewer` + `security-reviewer` post-W18-spike-draft:

**code-reviewer focus:**
- Cross-doc claim consistency: spike § ↔ ADR-Atlas-012 §4 ↔ plan-doc decisions ↔ master-plan §3 Layer-3 spec ↔ existing decisions.md DECISION-SEC-5/DB-3/DB-4
- Factual accuracy on LanceDB / fastembed-rs / Qdrant / Mem0 current state (verify against research-agent sources)
- `SemanticCacheBackend` trait sketch (§7) — Send/Sync correctness, lifetime soundness, byte-determinism preservation, object-safety for `Box<dyn>`
- Bench-shape soundness — B4/B5/B6 measure what they claim; targets are realistic; comparison-baseline (W17c B3 26 ms p99) is correctly framed

**security-reviewer focus:**
- Secure-delete design (§4.4) — wrapper sequence is correct; SSD-wear-leveling caveat is honestly acknowledged; cryptographic-erasure deferral to V2-γ is right tier
- Embedding-leakage threat-model — Morris et al. 2023 92% reconstruction citation is correct; Atlas's mitigation (event_uuid cite-back as trust anchor; embeddings outside canonicalisation) actually closes the loop
- GDPR audit-event shape (§4.5) — `embedding_erased` payload covers regulator-evidentiary needs; append-only semantics prevents replay/double-erasure attack; reuses existing ProjectorError variant pattern (no new variant) preserves `#[non_exhaustive]` discipline
- Layer-authority correction (Mem0g indexes Layer 1, not Layer 2) — verifies cache rebuild does NOT depend on Layer-2 availability; Layer-1 trust property survives W18 ship
- Hermes-skill cold-start budget — verify model-file caching strategy doesn't introduce supply-chain risk (HuggingFace download URL pinning + SHA256 verification at first-load fail-closed)

---

**End V2-β Welle 18 Mem0g Spike.** Recommendation: LanceDB embedded + fastembed-rs paired (pure-Rust Apache-2.0 stack). Atlas-controlled secure-delete wrapper (pre-capture-then-lock-then-overwrite protocol; covers fragments + HNSW `_indices/`). New `embedding_erased` audit-event-kind with EU-DPA-evidentiary payload. Bench-shape B4/B5/B6 mirroring W17c's pattern; timing-side-channel mitigation via response-time normalisation in the W18b Read-API endpoint. ADR-Atlas-012 distils these into **8 binding sub-decisions**; W18b implementation proceeds against this design.
