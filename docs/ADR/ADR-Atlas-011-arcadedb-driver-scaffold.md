# ADR-Atlas-011 — ArcadeDB Driver Scaffold + `GraphStateBackend` Trait Design

| Field             | Value                                                              |
|-------------------|--------------------------------------------------------------------|
| **Status**        | Accepted                                                           |
| **Date**          | 2026-05-13                                                         |
| **Welle**         | V2-β Welle 17a (Phase 9)                                           |
| **Authors**       | Nelson Mehlis (`@ThePyth0nKid`); welle-17a subagent (architect role) |
| **Replaces**      | —                                                                  |
| **Superseded by** | —                                                                  |
| **Related**       | ADR-Atlas-010 (W16 — backend choice + embedded-mode trade-off; binds 8 sub-decisions this ADR honours); `docs/V2-BETA-ARCADEDB-SPIKE.md` §7 (trait sketch ~40 LOC; this ADR documents the production version); ADR-Atlas-007 §3.1 (Option-A workspace-parallel projection — the batch path); `DECISION-DB-4` (ArcadeDB Apache-2.0 primary lock); `DECISION-SEC-4` (Cypher passthrough hardening); ADR-Atlas-009 (`@atlas/cypher-validator` consolidation) |

---

## 1. Context

### 1.1 What W17a inherits from W16

ADR-Atlas-010 (W16) locked 8 sub-decisions for ArcadeDB integration:

1. **ArcadeDB Apache-2.0 PRIMARY** (DECISION-DB-4 confirmed).
2. **SERVER mode** (Hermes-skill JVM blocker + FFI complexity → embedded rejected).
3. **`reqwest` + `rustls-tls`** as the Rust HTTP client.
4. **One ArcadeDB database per Atlas workspace.**
5. **Per-workspace atomic transactions** via `/api/v1/begin/{db}` + `/api/v1/commit/{db}`.
6. **Byte-determinism adapter:** all queries that feed canonicalisation MUST sort by `entity_uuid` ASC (vertices) / `edge_id` ASC (edges). `@rid` is NOT a valid sort key.
7. **Tenant isolation defence in depth:** Layer 1 per-database isolation + Layer 2 application-layer workspace_id binding + Layer 3 Cypher AST validator (mutation hardening only).
8. **`GraphStateBackend` trait** — sketched in spike §7 (~40 LOC); W17a writes the production trait + InMemoryBackend impl + ArcadeDbBackend stub.

W17a's job is the eighth sub-decision: the production trait surface + the two impls (one functional, one stub).

### 1.2 What this ADR resolves

ADR-Atlas-010 §6 left two open questions explicitly tagged "W17a decides":

- **OQ-1:** associated type vs `Box<dyn WorkspaceTxn>` for the transaction handle.
- **OQ-2:** how the trait exposes batch operations for Option-A parallel projection.

Section 4 below records the W17a decision for each, with rationale rooted in V2-α call-site reality + V2-β/γ projector-pipeline expectations. Future welles MUST honour both resolutions unless a follow-on ADR overturns them.

### 1.3 Why this ADR and not just code comments

The `GraphStateBackend` trait is the **structural seam** between Atlas's projection pipeline and any Layer-2 backend. W17b (full ArcadeDb impl) + V2-γ (potential FalkorDb fallback impl) + V2-δ (potential third-party backend submissions for self-hosted tiers) all depend on the trait's shape being stable. Documenting the rationale in code-comments alone makes it discoverable only at modification time; an ADR makes it discoverable at design-decision time when a future welle is asking "why doesn't the trait surface X?"

---

## 2. Decision drivers

- **Object safety preservation.** The projector emission pipeline (`emission.rs` / `gate.rs`) wants to hold the backend behind a `Box<dyn GraphStateBackend>` so V2-β-era code can swap impls at runtime via config. An associated-type design would force every consumer to be generic, which is viral.
- **Byte-determinism non-negotiability.** The pinned hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` (V2-α Welle 3, `canonical::tests::graph_state_hash_byte_determinism_pin`) MUST survive the trait introduction. W17a verifies this via a NEW conformance test that exercises the same fixture through the trait surface.
- **V2-α call-site stability.** `emission::build_projector_run_attestation_payload(&state, ...)` and `gate::verify_attestations_in_trace(workspace_id, &trace)` are existing public APIs. W17a adds backend-aware variants alongside; the legacy entry points stay unchanged.
- **Option-A parallel-projection alignment** (ADR-Atlas-007 §3.1). The projector accumulates per-workspace event batches and commits them as one transaction. Batch-upsert is the dominant call pattern, not single-upsert.
- **W17b's roadmap.** The ArcadeDbBackend stub MUST be a fill-in-the-blanks job for W17b. No API negotiation between W17a and W17b.
- **`#[non_exhaustive]` discipline.** Vertex/Edge/UpsertResult structs are part of the public crate surface. Adding schema-additive fields in V2-β/γ (annotations, policies, attestations…) must be a non-breaking change for downstream backends.
- **`Send + Sync` for tokio.** The projector runs on tokio; backends MUST be holdable behind `Arc<dyn GraphStateBackend>` for multi-task projection.

---

## 3. Considered options

### 3.1 Option A: Object-safe trait + `Box<dyn WorkspaceTxn>` + `batch_upsert` method (RECOMMENDED)

**Mechanism:**
- `trait GraphStateBackend: Send + Sync` — object-safe; all methods return concrete types or `Box<dyn ...>`.
- `fn begin(&self, ws: &WorkspaceId) -> Result<Box<dyn WorkspaceTxn + '_>>` — transactions are dynamically dispatched.
- `trait WorkspaceTxn: Send` — per-task transaction handle; `commit` / `rollback` take `self: Box<Self>` so use-after-finalise is prevented at the type level.
- `fn batch_upsert(&mut self, vertices: &[Vertex], edges: &[Edge]) -> Result<Vec<UpsertResult>>` on `WorkspaceTxn`. Vertices applied before edges (OQ-2 contract).
- Default `canonical_state()` impl on `GraphStateBackend` reads `vertices_sorted` + `edges_sorted` and runs the existing V2-α `canonical::graph_state_hash`. Backends MAY override; InMemoryBackend overrides for performance + byte-pin proximity.
- `Vertex`, `Edge`, `UpsertResult` are `#[non_exhaustive]` structs with explicit `new(...)` constructors so external crates can build instances without locking the field list.

**Pros:**
- Object-safe — `Box<dyn GraphStateBackend>` works at every call site.
- Batch-upsert matches the Option-A projection batch pattern naturally.
- Default canonical_state impl gives every backend byte-determinism for free.
- Send + Sync trait surface ≡ tokio-compatible.
- `#[non_exhaustive]` keeps the field set future-proof.
- Single, small dependency injection point — `Box<dyn GraphStateBackend>` injected at projector boot.

**Cons:**
- vtable overhead per method call (~1 ns). Negligible against the ~300-500 µs per-event HTTP roundtrip baseline.
- `Box<dyn WorkspaceTxn>` allocation on every `begin()`. One allocation per workspace per projection cycle; negligible.
- Default `canonical_state` impl walks vertices/edges via the trait surface, which clones data; InMemoryBackend overrides to skip this. Other backends MAY override.

**Confidence:** HIGH.

### 3.2 Option B: Generic trait with associated `type Txn: WorkspaceTxn` (REJECTED)

**Mechanism:**
- `trait GraphStateBackend { type Txn: WorkspaceTxn; fn begin(...) -> Result<Self::Txn>; }`
- Every consumer becomes `fn project<B: GraphStateBackend>(backend: &B, ...)`.

**Pros:**
- Zero vtable overhead.
- Compile-time monomorphisation.

**Cons:**
- **NOT object-safe.** `Box<dyn GraphStateBackend>` does not compile.
- **Viral generics.** Every public function in `atlas-projector` that takes a backend becomes generic. The crate's public API ergonomics suffer.
- **No runtime config swap.** `--backend=arcadedb` vs `--backend=in-memory` requires conditional compilation or a generic dispatch wrapper.
- Performance advantage (1 ns vs vtable) is irrelevant at ~300 µs/event scale.

**Decision:** REJECTED. Object safety is more important than monomorphisation at our scale.

### 3.3 Option C: Hybrid — generic at the backend layer + object-safe wrapper for consumers (REJECTED)

**Mechanism:**
- Same generic trait as Option B.
- Plus an `ObjectSafeBackend` wrapper struct that erases the associated type.

**Pros:**
- Best of both worlds.

**Cons:**
- Two API surfaces to maintain. Documentation tax.
- The wrapper's vtable overhead matches Option A; the generic path's benefit is invisible at the call site that uses the wrapper.
- W17b implementation complexity increases (write both the generic impl AND the wrapper).

**Decision:** REJECTED. Two-surface complexity does not pay back at V2-β scale.

---

## 4. Decision

**W17a adopts Option A: object-safe `GraphStateBackend` trait + `Box<dyn WorkspaceTxn>` + `batch_upsert` on the transaction handle + default `canonical_state` impl that delegates to V2-α canonicalisation.**

### 4.1 Trait surface (binding)

```rust
pub trait GraphStateBackend: Send + Sync {
    fn begin(&self, ws: &WorkspaceId)
        -> ProjectorResult<Box<dyn WorkspaceTxn + '_>>;
    fn vertices_sorted(&self, ws: &WorkspaceId) -> ProjectorResult<Vec<Vertex>>;
    fn edges_sorted(&self, ws: &WorkspaceId) -> ProjectorResult<Vec<Edge>>;
    fn canonical_state(&self, ws: &WorkspaceId) -> ProjectorResult<[u8; 32]> {
        /* default: reads vertices_sorted + edges_sorted; runs canonical::graph_state_hash */
    }
    fn backend_id(&self) -> &'static str;
}

pub trait WorkspaceTxn: Send {
    fn upsert_vertex(&mut self, v: &Vertex) -> ProjectorResult<UpsertResult>;
    fn upsert_edge(&mut self, e: &Edge) -> ProjectorResult<UpsertResult>;
    fn batch_upsert(&mut self, vertices: &[Vertex], edges: &[Edge])
        -> ProjectorResult<Vec<UpsertResult>>;
    fn commit(self: Box<Self>) -> ProjectorResult<()>;
    fn rollback(self: Box<Self>) -> ProjectorResult<()>;
}
```

### 4.2 Sub-decisions (binding for W17b/c + V2-γ)

1. **Object safety:** `GraphStateBackend` and `WorkspaceTxn` are object-safe. `Box<dyn GraphStateBackend + Send + Sync>` and `Box<dyn WorkspaceTxn + Send>` compile. (Resolves ADR-Atlas-010 §6 OQ-1.)
2. **Batch-upsert on `WorkspaceTxn`:** `batch_upsert(vertices, edges)` applies vertices FIRST, then edges. Result vec has length `vertices.len() + edges.len()`, vertices first. (Resolves ADR-Atlas-010 §6 OQ-2.)
3. **Default `canonical_state()` impl on `GraphStateBackend`:** delegates to `vertices_sorted` + `edges_sorted` + V2-α `canonical::graph_state_hash`. Backends MAY override; InMemoryBackend overrides to call `graph_state_hash` directly on the stored `GraphState` for performance + byte-pin proximity.
4. **`Vertex` + `Edge` + `UpsertResult` are `#[non_exhaustive]` with `new(...)` constructors.** Schema-additive fields in V2-β/γ extend the struct without breaking SemVer; the constructor's positional argument list MAY grow with future welles (which IS a SemVer-major change, scoped to the constructor not the struct).
5. **`rekor_log_index: Option<u64>` at the trait surface.** V2-α `GraphNode.rekor_log_index` is non-optional `u64` with sentinel `0` for "not yet anchored". The trait surface uses `Option<u64>` for honesty; conversion `None ↔ 0` is lossless and round-trips through `vertex_from_graph_node` / `build_graph_state_from_sorted`.
6. **Two type aliases at the trait module:** `pub type EntityUuid = String;`, `pub type EdgeId = String;`, `pub type WorkspaceId = String;`. Kept as `String` aliases (NOT newtypes) because V2-α's `GraphState` uses `String` for these fields and a newtype wrapper would force lossy conversions at every backend boundary. The semantic invariant (`entity_uuid = hex(blake3(workspace_id || event_uuid || kind))`) is enforced by the deriving function, NOT the type system.
7. **`InMemoryBackend` storage layout:** `Arc<Mutex<HashMap<WorkspaceId, GraphState>>>` for `Send + Sync` compatibility. Transactions clone the `Arc` and snapshot the per-workspace state into a scratch buffer; `commit` re-acquires the mutex briefly to swap. `MutexGuard` is NEVER held across the txn boundary (Windows `MutexGuard: !Send` blocker resolved at design time).
8. **`ArcadeDbBackend` stub:** every trait method is `unimplemented!("W17b: ...")` with a per-method placeholder string documenting the HTTP endpoint + Cypher query W17b will write. The struct compiles + constructs without panic; only trait method invocations panic. No `reqwest` dep added in W17a (W17b adds it alongside the first method body that uses it).
9. **`backend_id` return values:** `"in-memory"` for `InMemoryBackend`, `"arcadedb-server"` for `ArcadeDbBackend` (stable across crate versions; ProjectorRunAttestation consumers downstream depend on the exact strings).

---

## 5. Consequences

### 5.1 Accepted today

- **`atlas-projector` crate gains a new public module** `backend` with submodules `arcadedb` + `in_memory`. Public re-exports at the crate root (`GraphStateBackend`, `InMemoryBackend`, `ArcadeDbBackend`, `Vertex`, `Edge`, `UpsertResult`, `WorkspaceTxn`, `EntityUuid`, `EdgeId`, `WorkspaceId`).
- **`emission.rs` + `gate.rs` gain backend-aware variants** (`build_projector_run_attestation_payload_from_backend`, `verify_attestations_in_trace_with_backend`) alongside the legacy `&GraphState` / `&AtlasTrace` entry points. Legacy entry points unchanged; existing callers unaffected.
- **New conformance test** `crates/atlas-projector/tests/backend_trait_conformance.rs` (8 tests including the byte-pin via the trait surface).
- **Byte-pin** `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` exercised through TWO paths now: legacy `canonical::tests::graph_state_hash_byte_determinism_pin` AND the new `byte_pin_through_in_memory_backend`. Either failing trips the CI signal.

### 5.2 Mitigated by this design

- **W17b API negotiation risk:** ELIMINATED. The trait surface is locked in W17a; W17b is pure fill-in-the-blanks.
- **Byte-determinism drift under backend swap:** MITIGATED via the default `canonical_state()` impl on the trait body + the in-memory override.
- **V2-α call-site churn:** AVOIDED. Backend-aware variants live alongside legacy entry points.
- **`#[non_exhaustive]` SemVer-breakage in V2-γ:** AVOIDED via the explicit `new(...)` constructor pattern.

### 5.3 Out-of-scope for W17a (deferred)

- **`ArcadeDbBackend` body:** W17b.
- **`reqwest` dep:** W17b (alongside first method body).
- **Cross-backend byte-determinism test** (`tests/cross_backend_byte_determinism.rs` pulling both InMemory + ArcadeDb against the same events.jsonl fixture): W17b.
- **Docker-Compose CI workflow** (`.github/workflows/atlas-arcadedb-smoke.yml`): W17c.
- **`Vertex` + `Edge` annotations/policies fields:** when the projector pipeline emits Welle-14-shaped state through a non-InMemory backend, the trait surface will need to grow `annotations` + `policies` fields. W17a leaves the trait surface at the V2-α (W1) field set; the InMemoryBackend's `canonical_state()` override carries Welle-14 fields through the underlying `GraphState`. W17b/W18 decide trait-surface extension timing.
- **Workspace-parallel projection driver:** W17b (the trait supports it via `Send + Sync` + `batch_upsert`; the actual orchestration loop is W17b).

### 5.4 Dependencies on downstream wellen

- **W17b** (Phase 9): full `ArcadeDbBackend` impl. The stub's panic-strings reference W17b explicitly so any premature production-path call surfaces a clear "W17b not done yet" diagnostic.
- **W17c** (Phase 9): Docker-Compose CI workflow + integration tests.
- **W18** (Phase 10): Mem0g integration MAY use the same trait surface for a vector-store backend (TBD per ADR-Atlas-012).

---

## 6. Open questions (for W17b + V2-γ tracking)

- **OQ-7 (new, for W17b):** Should `batch_upsert` accept owned `Vec<Vertex>` / `Vec<Edge>` (zero-copy into the ArcadeDb HTTP body) or `&[Vertex]` / `&[Edge]` (current choice, simpler call site)? W17a picks `&[...]` for ergonomics; W17b benchmarks may flip if HTTP-body assembly becomes the hot path.
- **OQ-8 (new, for W17b):** ArcadeDb txn lifetime story. ArcadeDB session IDs (`arcadedb-session-id` header) survive until commit/rollback or server-side timeout. How does `Box<dyn WorkspaceTxn>` handle session-timeout mid-projection? W17b decides retry semantics + the session-id liveness pingback.
- **OQ-9 (new, for V2-γ):** Should the trait surface grow `annotations` + `policies` fields on `Vertex`, OR should those live on a separate `Annotations` / `Policies` backend trait (separation of concerns)? W17a defers; W17b's actual ArcadeDb impl will inform the choice.
- **OQ-10 (new, for V2-γ):** Multi-tenant credential management for `ArcadeDbBackend::new`. Per-workspace ArcadeDB root password vs per-tenant JWT vs SPIRE-issued workload identity. Defer to V2-γ deployment-architecture welle.
- **OQ-11 (carried from ADR-Atlas-010 §6 OQ-5):** ArcadeDB authentication mode. V2-β starts with HTTP Basic per ADR-010 §6 OQ-5; V2-γ MAY want JWT bearer.

ADR-Atlas-010 §6 OQ-1 + OQ-2 are CLOSED by this ADR (see §4.2 sub-decisions #1 + #2).

---

## 7. Reversibility

**Decision is HIGH reversibility.**

- **Trait-surface reversibility:** the trait lives entirely within `atlas-projector` and is consumed via re-exports. Adding a new trait method is SemVer-breaking; soften by providing a default impl. Removing a method requires a `#[deprecated]` + cascade through the crate version bump.
- **`Box<dyn WorkspaceTxn>` vs associated type reversibility:** flipping to associated type would be a SemVer-major change (every consumer becomes generic). Cost: ~1 session refactor + every downstream SDK update. Not on any roadmap.
- **`batch_upsert` reversibility:** removing the method is SemVer-breaking. Keeping it but deprecating in favour of a different API is ~0.5 session. Not on any roadmap.
- **Default `canonical_state` impl reversibility:** trivial. Override on every backend, then delete the default. ~0.2 session.
- **`Vertex` / `Edge` field-set reversibility:** `#[non_exhaustive]` makes ADDING fields free. REMOVING fields is SemVer-breaking and would propagate to the byte-determinism CI pin (because canonical bytes would change).

No technical debt incurred; reversibility paths documented per dimension.

---

## 8. Watchlist + review cadence

### 8.1 Specific tracking signals

- **W17b implementation progress:** the stub's `unimplemented!()` messages reference W17b directly. Any premature call surfaces the panic; surface as a tracked CI signal post-W17b merge to verify all stubs are replaced.
- **`Cargo.lock` diff at W17b merge time:** `reqwest` + ~20 transitive deps will land. Audit for `rustls-tls` feature selection + no `openssl-sys` accidental inclusion.
- **`graph_state_hash` byte-pin** continues to be the load-bearing CI signal. Now exercised via TWO paths (`canonical::tests::graph_state_hash_byte_determinism_pin` + `backend_trait_conformance::byte_pin_through_in_memory_backend`); either failure trips the gate.
- **Cross-backend byte-determinism test** (W17b deliverable): once landed, that becomes the THIRD path through the byte-pin and the strongest guarantee against trait-surface-induced drift.

### 8.2 Review cadence

- **Pre-W17b start:** verify trait surface + stub messages reflect the actual W17b implementation plan; refresh ADR if a sub-decision needs revision.
- **Post-W17b ship:** review byte-determinism cross-backend test output against the pinned hex.
- **Post-W17c ship:** review benchmark numbers; if HTTP latency exceeds 15 ms p99 the embedded-mode reconsideration threshold (ADR-Atlas-010 §4.4) fires.
- **Post-V2-β-1 ship (W19):** review the open questions §6 against accumulated W17b/c learnings; consolidate or close.

### 8.3 Out-of-cadence triggers

Any change to the trait surface that requires a `#[non_exhaustive]` exception, or any addition of a non-default trait method, MUST open a follow-on ADR. The trait is structurally load-bearing; surface changes are decision-recorded events.

---

## 9. Decision log

| Date       | Event                                                                                                                                 | Outcome                                                                              |
|------------|---------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------|
| 2026-05-13 | ADR-Atlas-010 ships; W17a scope locked (driver scaffold + trait + ADR).                                                              | W17a unblocked.                                                                      |
| 2026-05-13 | ADR-Atlas-011 opens. Trait surface designed; OQ-1 + OQ-2 resolved.                                                                    | W17b unblocked. Trait + InMemoryBackend + ArcadeDbBackend stub merged in single commit. |
| 2026-05-13 | `backend_trait_conformance::byte_pin_through_in_memory_backend` passes — pinned hex `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` reproduced via the trait surface. | Byte-determinism preserved across the abstraction.                                  |
| TBD        | W17b: full `ArcadeDbBackend` impl using `reqwest` + Cypher. Cross-backend byte-determinism test passes.                              | Adapter contract validated; W17c unblocked.                                          |
| TBD        | W17c: Docker-Compose CI workflow + integration tests + benchmark capture.                                                            | Performance estimates validated or embedded-mode threshold triggered.                |

(Future quarterly refreshes append rows here.)

---

**End ADR-Atlas-011.**
