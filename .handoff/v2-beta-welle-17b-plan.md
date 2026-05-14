# V2-β Welle 17b — Plan-Doc (ArcadeDB driver implementation)

> **Status:** WIP — subagent dispatched 2026-05-14, parent stopped at clippy-clean state (zero warnings on new arcadedb code; 18/18 trait-conformance tests pass) BEFORE plan-doc + parent-side reviewer dispatch. Nelson Docker-restart breakpoint.
> **Orchestration:** Phase 10 per `docs/V2-BETA-ORCHESTRATION-PLAN.md`. Builds on W17a (PR #85) + W17a-cleanup (PR #88).
> **Driving decisions:** ADR-Atlas-010 §4 (8 binding sub-decisions); ADR-Atlas-011 §4 + §4.3 (trait surface + W17a-cleanup helpers).

W17b fills the `ArcadeDbBackend` stub shipped by W17a (PR #85) using `reqwest`-based HTTP calls per ADR-Atlas-010 §4. The W17a-cleanup PR #88 landed the boundary helpers (`check_workspace_id`, `check_value_depth_and_size`) and widened the `begin()` lifetime to `'static`, so W17b is a fill-in-the-blanks implementation with no trait-surface negotiation.

**Why this as Welle 17b:** ADR-Atlas-010 §4 sub-decision #8 explicitly scopes the full ArcadeDb driver impl to this welle. W17c (Docker-Compose CI integration tests + benchmark capture) and W18 (Mem0g Layer-3 cache) both depend on W17b being live.

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| `crates/atlas-projector/src/backend/arcadedb/{mod.rs, client.rs, cypher.rs}` (NEW sub-module split — replaces W17a stub `arcadedb.rs` single file) | `.github/workflows/atlas-arcadedb-smoke.yml` (W17c) |
| `crates/atlas-projector/Cargo.toml` — adds `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }` | Docker-Compose CI test runner (W17c) |
| `crates/atlas-projector/tests/cross_backend_byte_determinism.rs` (NEW — `#[ignore]`-gated, runs only with `ATLAS_ARCADEDB_URL` env var) | Mem0g Layer-3 integration (W18 / ADR-Atlas-012 reserved) |
| `crates/atlas-projector/tests/backend_trait_conformance.rs` — DROP the `should_panic` stub tests + the `arcadedb_stub_panics` test (no longer applicable after fill) | `CHANGELOG.md` (parent consolidates Phase 10.5) |
| `crates/atlas-projector/src/lib.rs` — re-export updates if sub-module changed re-export paths | `docs/V2-MASTER-PLAN.md` (parent consolidates) |
| `.handoff/v2-beta-welle-17b-plan.md` (THIS file) | `.handoff/v2-session-handoff.md` (parent consolidates) |
| | `crates/atlas-projector/src/backend/mod.rs` (UNCHANGED — W17a-cleanup locked it) |
| | `crates/atlas-projector/src/backend/in_memory.rs` (UNCHANGED) |
| | `crates/atlas-projector/src/error.rs` (UNCHANGED — `InvalidWorkspaceId` variant stable from W17a-cleanup) |

**Hard rule:** the V2-β-Orchestration-Plan §3.3 forbidden-files rule applies. Parent agent edits CHANGELOG/Master-Plan/Handoff in a post-merge Phase 10.5 consolidation PR.

## Binding decisions (from upstream ADRs — W17b implements, not re-decides)

- **ArcadeDB Apache-2.0 server mode** (ADR-010 §4 sub-decision #1+#2).
- **`reqwest` + `rustls-tls`** HTTP client (ADR-010 §4 sub-decision #3).
- **One ArcadeDB database per Atlas workspace** (ADR-010 §4 sub-decision #4). Naming: `atlas_ws_<workspace_id>` with `-` → `_` substitution for ArcadeDB DB-name compatibility.
- **Per-workspace atomic transactions** via `/api/v1/begin/{db}` + `/api/v1/commit/{db}` + `/api/v1/rollback/{db}` (ADR-010 §4 sub-decision #5).
- **Byte-determinism adapter:** `ORDER BY entity_uuid ASC` (vertices) / `ORDER BY edge_id ASC` (edges). `@rid` is FORBIDDEN as sort key (ADR-010 §4 sub-decision #6).
- **Tenant isolation defence-in-depth:** Layer 1 per-DB + Layer 2 application workspace_id binding via parameterised Cypher (ADR-010 §4 sub-decision #7).
- **`'static` transaction lifetime** — `ArcadeDbTxn` carries owned `reqwest::Client` + owned `arcadedb-session-id: String` + owned `db_name: String` (ADR-011 §4.3 sub-decision #10).
- **`check_workspace_id` called FIRST in `begin()`** before constructing the HTTP request (ADR-011 §4.3 sub-decision #11). Rules: non-empty + len≤128 + ASCII + no `/` `\` NUL `\r` `\n`.
- **`check_value_depth_and_size` called at HTTP-response parse boundary** AFTER `serde_json::from_slice` and BEFORE `Vertex::new` / `Edge::new` (ADR-011 §4.3 sub-decision #12). Recommended limits: `max_depth=32`, `max_bytes=64*1024`.
- **HTTP Basic auth** for V2-β (ADR-010 OQ-5). JWT bearer deferred to V2-γ.

## Files (current WIP state, post-subagent-stop 2026-05-14)

| Status | Path | Lines | Inhalt |
|---|---|---|---|
| NEW    | `crates/atlas-projector/src/backend/arcadedb/mod.rs` | 686 | `ArcadeDbBackend` struct + impl `GraphStateBackend` + `ArcadeDbTxn` struct + impl `WorkspaceTxn` + error mapping + commit/rollback session handling |
| NEW    | `crates/atlas-projector/src/backend/arcadedb/client.rs` | 418 | `reqwest::Client` wrapper + Basic auth + connect/request timeouts + Cypher response JSON parse helpers (calls `check_value_depth_and_size`) |
| NEW    | `crates/atlas-projector/src/backend/arcadedb/cypher.rs` | 674 | Cypher query builders (parameterised `$ws` / `$eid` / `$props` binding — never string-concat); vertex/edge upsert MERGE templates; sorted-read MATCH templates per §4.9 adapter |
| NEW    | `crates/atlas-projector/tests/cross_backend_byte_determinism.rs` | 257 | `#[ignore]`-gated test — same 3-node + 2-edge fixture as `backend_trait_conformance::byte_pin_through_in_memory_backend`; asserts `InMemoryBackend::canonical_state() == ArcadeDbBackend::canonical_state()` byte-identical. Requires `ATLAS_ARCADEDB_URL` env var (W17c CI sets it via Docker-Compose) |
| MODIFY | `crates/atlas-projector/Cargo.toml` | +27 | Adds `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }` |
| DELETE | `crates/atlas-projector/src/backend/arcadedb.rs` | -213 | W17a stub removed; replaced by sub-module |
| MODIFY | `crates/atlas-projector/src/lib.rs` | +1/-1 | Re-export path update |
| MODIFY | `crates/atlas-projector/tests/backend_trait_conformance.rs` | +54/-? | DROP stub-panic tests; ADD any non-arcadedb tests if needed |
| MODIFY | `Cargo.lock` | +3 | reqwest transitive deps |
| NEW    | `.handoff/v2-beta-welle-17b-plan.md` | THIS file | Plan-doc |

**Total diff vs origin/master:** ~2109 insertions / ~225 deletions across 9 files.

## What was DONE (subagent worktree state at parent-stop)

- ✓ Full `ArcadeDbBackend` + `ArcadeDbTxn` impl across 3 sub-module files
- ✓ `reqwest` dep added with `rustls-tls` feature
- ✓ `check_workspace_id` called in `begin()` (verify exact placement at review)
- ✓ `check_value_depth_and_size` called at HTTP-response parse boundary (verify call-sites at review)
- ✓ Parameterised Cypher (verify at review — grep for raw string-interpolation into queries)
- ✓ Cross-backend byte-determinism test created (`#[ignore]`-gated)
- ✓ Stub-panic tests dropped from conformance suite
- ✓ `cargo check -p atlas-projector` clean (post-WIP)
- ✓ `cargo test -p atlas-projector --test backend_trait_conformance` — 18/18 green (post-WIP)
- ✓ Zero clippy warnings (subagent self-reported pre-stop)

## What is NOT YET DONE (next-session pickup)

- [ ] **Parent-led parallel `code-reviewer` + `security-reviewer` dispatch** per Atlas Standing Protocol lesson #8. Reviewer focus suggestions:
  - **code-reviewer:** Cypher template parameterisation correctness (grep for raw string-interpolation), `'static` lifetime honouring (no `&'a self.<field>` borrowed into ArcadeDbTxn), error-path correctness (every HTTP error → `ProjectorError`), `check_workspace_id` is FIRST statement of `begin()`, `check_value_depth_and_size` called at every `from_slice` → `BTreeMap<String, Value>` boundary, default `canonical_state()` trait impl path produces byte-identical output to InMemoryBackend.
  - **security-reviewer:** credential redaction (grep for `password`/`token`/`auth` echo in error strings or logs), tenant isolation (per-database + Cypher param binding), parameter binding safety (no Cypher-injection paths even for trusted internal input), no `unsafe` blocks, panic-path audit (no panics reachable via public API from HTTP errors), Basic auth credentials never logged.
- [ ] Fix CRITICAL/HIGH/applicable-MEDIUMs in-commit per reviewer dispatch outcome
- [ ] Cross-backend byte-determinism test ACTUAL RUN against a live ArcadeDb instance — gated behind `ATLAS_ARCADEDB_URL` env var. **Validation deferred to W17c when Docker-Compose CI is set up. For W17b merge: the test EXISTS, is `#[ignore]`-gated, compiles cleanly. Actual byte-pin reproduction through ArcadeDb path validated in W17c.**
- [ ] PR description body
- [ ] Admin-merge after green CI

## Test impact (V1 + V2-α + V2-β-W17a assertions to preserve)

| Surface | Drift risk under W17b | Verified |
|---|---|---|
| Byte-determinism CI pin (V2-α Welle 3) | NONE — W17b adds an ArcadeDb path; the V2-α canonical pipeline is unchanged; the W17a default `canonical_state()` trait impl preserves byte-determinism IFF the §4.9 adapter is honoured | InMemoryBackend pin still `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4`; ArcadeDb path validation deferred to W17c CI |
| W17a trait-conformance suite | NONE — trait surface unchanged | 18/18 green post-WIP |
| W17a-cleanup boundary helpers | USE — not modify | `check_workspace_id` called in begin(); `check_value_depth_and_size` called at HTTP-response boundary (verify exact placement at review) |
| V1 verifier APIs | NONE — `atlas-trust-core` untouched | N/A |

## W17a carry-over MEDIUM status (final tracking)

- **#2 (`serde_json::Value` depth+size):** RESOLVED — helper called at every HTTP-response → `Vertex::properties` / `Edge::properties` boundary (verify at review). Defaults: `max_depth=32`, `max_bytes=64*1024`.
- **#3 (`WorkspaceId` validation):** RESOLVED — `check_workspace_id` called as FIRST statement of `begin()` (verify at review).
- **#4 (`begin()` lifetime):** ALREADY RESOLVED in W17a-cleanup (lifetime is `'static`); W17b honours by carrying owned client + session-id + db-name in `ArcadeDbTxn`.
- **#5 (`MalformedEntityUuid` umbrella variant for edges):** V2-γ-deferred per W17a + W17a-cleanup plan-doc rationale; W17b does NOT touch the error-enum convention.

## Open questions for W17c

- **OQ-12 (NEW for W17c):** Docker-Compose CI runner — bake ArcadeDb 25.x image with the `atlas_ws_*` database pre-created, or lazy-create at first `begin()`? Spike §8 sketch favours lazy-create; benchmark may inform.
- **OQ-13 (NEW for W17c):** Cross-backend byte-determinism test runtime budget — current `#[ignore]`-gated test runs the 3-node + 2-edge fixture. Larger fixture for performance assertion? Defer to W17c benchmark capture.
- **OQ-7..OQ-11 (carried from ADR-011 §6):** still open; W17c benchmark may inform OQ-7 (owned-vs-borrowed batch slices); operator runbook informs OQ-10 (multi-tenant credentials).

## Resume-from-breakpoint guide (next-session entry point)

1. `cd /c/Users/nelso/Desktop/atlas`
2. `git fetch origin` — verify origin has branch `feat/v2-beta/welle-17b-arcadedb-impl` with subagent WIP commit.
3. `git checkout feat/v2-beta/welle-17b-arcadedb-impl` OR work on master and use the worktree at `.claude/worktrees/agent-a4a6a80c539380769/`.
4. Verify build: `/c/Users/nelso/.cargo/bin/cargo.exe check -p atlas-projector` → clean.
5. Verify tests: `/c/Users/nelso/.cargo/bin/cargo.exe test -p atlas-projector --test backend_trait_conformance` → 18/18 green.
6. Open PR (or it may already exist if breakpoint commit auto-opened): `"/c/Program Files/GitHub CLI/gh.exe" pr list --state open --head feat/v2-beta/welle-17b-arcadedb-impl`.
7. Read this plan-doc + the diff. Decide: clean review-and-merge OR fix-and-merge OR restart-from-scratch.
8. Parent dispatches parallel `code-reviewer` + `security-reviewer` per Atlas Standing Protocol lesson #8 — see "Reviewer focus suggestions" in §"What is NOT YET DONE" above.
9. Fix CRITICAL/HIGH/applicable-MEDIUMs in-commit. Final commit-series should squash cleanly.
10. Admin-merge after CI green + acceptance criteria verified. Cross-backend byte-determinism test EXISTS-and-compiles is acceptance criterion for W17b; ACTUAL byte-pin reproduction through ArcadeDb is W17c's CI gate.
11. **Phase 10.5 consolidation PR** (parent-led, separate): updates `.handoff/v2-session-handoff.md` §0z + CHANGELOG `[Unreleased]` + `docs/V2-MASTER-PLAN.md` §6 status + `.handoff/decisions.md` Welle-17 closure rows + `docs/V2-BETA-ORCHESTRATION-PLAN.md` Welle-17 status flip.
