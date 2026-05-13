# V2-α Welle 6 — Plan-Doc (Projector-State-Hash CI Gate Enforcement)

> **Status: DRAFT 2026-05-13.** Awaiting Nelson's confirmation before merge.
> **Master Plan reference:** `docs/V2-MASTER-PLAN.md` §6 V2-α Foundation. Session 6 of 5–8 (now upper-mid V2-α range).
> **Driving decisions:** `DECISION-ARCH-1` triple-hardening (canonicalisation byte-pin ✓ Welle 3 + ProjectorRunAttestation ✓ Welles 4+5 + CI gate THIS welle); Master Vision §5.2 *"makes determinism part of the trust chain, not just CI hygiene"*.

Welle 6 **closes the V2-α security loop**. After Welle 5 Atlas can sign a `ProjectorRunAttestation` event asserting `(projector_version, head_event_hash) → graph_state_hash`. After Welle 6 a third-party verifier can **independently re-project the trace's underlying events and confirm the attested hash matches the recomputed hash** — drift detected cryptographically, not just by CI comparison.

This is the load-bearing primitive that turns Atlas's projection layer from "trustworthy-by-CI-convention" into "trustworthy-by-cryptographic-verification" per `DECISION-ARCH-1`.

**Why this as Welle 6** (rather than ArcadeDB integration):
- Self-contained: uses only existing in-repo code (Welle 3 `graph_state_hash` + Welle 4 `parse_projector_run_attestation` + Welle 5 `project_events`)
- Zero new external dependencies (ArcadeDB needs operator-runbook + Docker orchestration + driver evaluation — 2-3 sessions easily)
- Maximum security-ROI before scope-creep: completes the V2-α verification chain end-to-end on the current in-memory implementation
- ArcadeDB can come AFTER and just swap the storage backend — the CI gate works equally well on either storage paradigm
- Realistic 1-session scope: orchestration code + integration tests, ~600-900 lines

**What Welle 6 enables operationally:**

```
Atlas-signed events.jsonl  ──► Welle 5 project_events ──► local GraphState
                          │                                    │
                          │                                    ▼
                          │                            Welle 3 graph_state_hash
                          │                                    │
                          │                                    ▼
                          └──► find ProjectorRunAttestation ──► compare ──► ✓/✗
                               event in events.jsonl     │
                               (Welle 4 parse+validate)  │
                                                         └──► attested hash
```

If the attested hash matches the locally-recomputed hash, the projection is **cryptographically verified** to match what the issuer signed. If not, drift detected — operator/auditor sees a structured error pinpointing the divergence.

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| NEW `crates/atlas-projector/src/gate.rs` (~250 lines) — orchestration module that takes an `AtlasTrace` + workspace_id, finds `ProjectorRunAttestation` events, re-projects the trace's other events, compares attested vs recomputed `graph_state_hash`, returns structured `Vec<GateResult>` | Counsel-track concerns (GDPR Path B, content-hash separation — out of scope per `DECISION-COUNSEL-1`) |
| `pub fn verify_attestations_in_trace(workspace_id: &str, trace: &AtlasTrace) -> ProjectorResult<Vec<GateResult>>` — top-level API. Iterates all events in the trace; for each `ProjectorRunAttestation` payload, re-project + compare | ArcadeDB driver integration (Welle 7+ candidate) |
| `pub struct GateResult { event_id, attested_hash, recomputed_hash, status }` typed return | Operator-runbook for production deployment of the CI gate |
| `pub enum GateStatus { Match, Mismatch, AttestationParseFailed }` — structured outcome | atlas-signer CLI integration for projector emission |
| NEW `crates/atlas-projector/tests/projection_gate_integration.rs` (~300 lines) — 6+ E2E tests covering: happy path (attested hash matches), tampered attestation (hash mismatch), well-formed but wrong-projection-count attestation, multiple ProjectorRunAttestation events in same trace, trace without any attestation events, malformed attestation payload | atlas-web UI rendering of CI gate results (V2-β) |
| Update `docs/SEMVER-AUDIT-V1.0.md` §10 with V2-α Welle 6 atlas-projector additions | Mem0g cache integration (V2-β) |
| Update `CHANGELOG.md [Unreleased]` | New event-kind support beyond what Welle 5 has |
| Plan-doc (this file) | Cedar policy gate (V2-δ) |

---

## Decisions (final, pending Nelson confirmation)

- **Module lives in atlas-projector, NOT atlas-trust-core.** Clean DAG (atlas-trust-core MUST NOT depend on atlas-projector — established invariant since Welle 3). The CI gate combines V1's `AtlasTrace` (verifier-side) with V2-α's projection logic — atlas-projector is the natural home because it already depends on atlas-trust-core for primitives. Callers wanting V2-α-aware verification import both crates.
- **Re-projection excludes the attestation event itself.** When the gate runs, it filters out all `ProjectorRunAttestation` events from the trace, re-projects only the graph-shape events (`node_create` / `node_update` / `edge_create`), then compares against each attestation. This is because attestation events are not projection input — they're claims ABOUT projection state, not events that mutate state.
- **Welle-6-MVP semantics: each `ProjectorRunAttestation` event asserts the FULL projection state at its `head_event_hash`.** I.e., the gate re-projects ALL non-attestation events up to and including the `head_event_hash` event, then compares against the attested hash. Future welles may add incremental-attestation semantics (each attestation only covers events since the last one).
- **GateResult is per-attestation-event, not per-trace.** A trace with 3 ProjectorRunAttestation events produces 3 GateResults. Caller decides whether to aggregate as "all pass" / "any fail" / structured array.
- **`projected_event_count` from attestation is compared against actual count of projected events.** Mismatch is a HARD fail — the issuer claimed N events but the trace contains M ≠ N projectable events. Structured error.
- **Status enum has exactly 3 states for V2-α-MVP:**
  - `Match` — recomputed hash equals attested hash
  - `Mismatch` — hashes differ; trace has been tampered or projector implementation drifted
  - `AttestationParseFailed` — attestation event's payload couldn't be parsed; Welle 4 validation would also have rejected it
- **No new TrustError or ProjectorError variants needed.** Re-use existing variants for parse failures; introduce GateStatus enum for outcome reporting (success-or-mismatch is not a Rust-level error, it's a domain-level result).
- **Unsupported event kinds in the trace are propagated as errors.** If a non-attestation event has payload type `policy_set` (or similar Welle-5-unsupported kind), the re-projection step fails with `UnsupportedEventKind`. Welle-6-MVP does NOT silently skip these — that would mask issuer-side scope drift. Caller must filter the trace first if desired.
- **Version bump in this welle:** NONE. Workspace stays at `1.0.1`. Deferred to V2-α welle-bundle close-out.
- **No changes to atlas-trust-core**, V1 byte-pins, V1 verifier behaviour, or any existing atlas-projector module. Welle 6 is pure addition.

---

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `crates/atlas-projector/src/gate.rs` (~250 lines) | `GateResult` struct + `GateStatus` enum + `verify_attestations_in_trace(workspace_id, trace) -> ProjectorResult<Vec<GateResult>>` top-level function + 5+ unit tests. |
| MODIFY | `crates/atlas-projector/src/lib.rs` | `pub mod gate;` + re-exports of `verify_attestations_in_trace`, `GateResult`, `GateStatus`. Update crate-doc §"Shipped" with Welle 6 addition. |
| NEW | `crates/atlas-projector/tests/projection_gate_integration.rs` (~300 lines) | 6+ E2E tests:<br>1. `happy_path_attestation_matches_reprojection`<br>2. `tampered_attestation_hash_mismatch_detected`<br>3. `mismatched_projected_event_count_detected`<br>4. `multiple_attestation_events_each_verified`<br>5. `trace_without_attestations_returns_empty_result_vec`<br>6. `malformed_attestation_payload_surfaces_parse_failed_status` |
| MODIFY | `docs/SEMVER-AUDIT-V1.0.md` | §10.7d new subsection listing every new `pub` item with V2-α-Additive tag. |
| MODIFY | `CHANGELOG.md` | `[Unreleased]` gets `### Added — V2-α Welle 6` block ordered above Welle 5. |
| NEW | `.handoff/v2-alpha-welle-6-plan.md` | This plan-doc. |

**Total estimated diff:** ~800-1100 lines Rust + tests + docs.

---

## Algorithm sketch

```rust
pub fn verify_attestations_in_trace(
    workspace_id: &str,
    trace: &AtlasTrace,
) -> ProjectorResult<Vec<GateResult>> {
    // Step 1: partition the events into (projectable, attestation).
    let mut projectable_events: Vec<&AtlasEvent> = Vec::new();
    let mut attestation_events: Vec<&AtlasEvent> = Vec::new();
    for ev in &trace.events {
        match ev.payload.get("type").and_then(Value::as_str) {
            Some(PROJECTOR_RUN_ATTESTATION_KIND) => attestation_events.push(ev),
            _ => projectable_events.push(ev),
        }
    }

    // Step 2: re-project all projectable events into a fresh state.
    // Unsupported event kinds in the projectable set produce a hard
    // error — this is intentional; we do NOT silently skip them
    // because that would mask issuer-side scope drift.
    let state = project_events(
        workspace_id,
        // Need owned values for project_events signature; clone the refs.
        &projectable_events.iter().map(|&e| e.clone()).collect::<Vec<_>>(),
        None,
    )?;
    let recomputed_hash = graph_state_hash(&state)?;
    let recomputed_hex = hex::encode(recomputed_hash);

    // Step 3: for each attestation event, parse, compare.
    let mut results = Vec::new();
    for att_event in attestation_events {
        let status = match parse_projector_run_attestation(&att_event.payload) {
            Err(_) => GateStatus::AttestationParseFailed,
            Ok(att) => {
                let count_matches = att.projected_event_count
                    == projectable_events.len() as u64;
                let hash_matches = att.graph_state_hash == recomputed_hex;
                if count_matches && hash_matches {
                    GateStatus::Match
                } else {
                    GateStatus::Mismatch
                }
            }
        };
        results.push(GateResult {
            event_id: att_event.event_id.clone(),
            attested_hash: ...,    // from parsed attestation, or empty if parse failed
            recomputed_hash: recomputed_hex.clone(),
            status,
        });
    }

    Ok(results)
}
```

This is the core. Tests exercise each branch.

---

## Acceptance criteria

- [ ] `cargo check --workspace` green
- [ ] `cargo test --workspace` green; all 7 byte-determinism pins byte-identical
- [ ] `verify_attestations_in_trace` happy-path: trace with valid attestation → `GateResult::Match`
- [ ] `verify_attestations_in_trace` negative-path 1: attestation hash tampered → `GateResult::Mismatch`
- [ ] `verify_attestations_in_trace` negative-path 2: attestation `projected_event_count` wrong → `GateResult::Mismatch`
- [ ] `verify_attestations_in_trace` negative-path 3: malformed attestation payload → `GateResult::AttestationParseFailed`
- [ ] Multiple attestations in same trace → multiple `GateResult` entries
- [ ] Trace without any attestation events → empty `Vec<GateResult>`
- [ ] `GateStatus` enum is `#[non_exhaustive]` for future-welle additive variants
- [ ] `GateResult` struct fields are `pub`
- [ ] `SEMVER-AUDIT-V1.0.md` §10 lists every new `pub` item
- [ ] `CHANGELOG.md [Unreleased]` has Welle 6 entry
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched
- [ ] CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit, PR opened, self-merge via `gh pr merge --squash --admin --delete-branch`

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Re-projection of trace events produces a different graph_state_hash than the issuer's projector did (e.g. due to subtle canonicalisation drift) | LOW | HIGH (would cause false-positive Mismatch on legitimate traces) | The 7 existing byte-determinism CI pins catch any canonicalisation drift before it can affect projection. Welle 6 is downstream of those pins — if they pass, projection is deterministic across builds. |
| Trace contains events the V2-α-MVP projector doesn't support (e.g. `policy_set`) | MEDIUM (real-world traces may carry such events) | LOW (caller filters trace first OR Welle 6 surfaces structured error) | Documented in plan as caller responsibility; future welles expand event-kind support. |
| `projected_event_count` mismatch semantics: what counts as a "projectable event"? | LOW | LOW (defined: events whose payload type is in the Welle-5 supported set OR any non-attestation event) | Documented in module-doc; tested via `mismatched_projected_event_count_detected`. |
| ProjectorRunAttestation event's signature itself is invalid (issuer-side bug) | LOW | LOW (V1 verifier catches this BEFORE Welle 6 gate runs; Welle 6 assumes pre-verified trace) | Documented in module-doc: gate consumer responsibility to run `verify_trace` first. |
| `head_event_hash` in attestation doesn't actually point to any event in the trace | MEDIUM | LOW (V2-α-MVP does NOT enforce this; future welle may; documented gap) | Documented as out-of-scope; Welle 7+ may add cross-event integrity check |

---

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 6 | Mitigation |
|---|---|---|
| All 7 byte-determinism pins (V1 + V2-α Welles 1+3+4) | NONE — Welle 6 only adds a new module; touches no existing code | Workspace test suite confirms |
| atlas-trust-core verifier behaviour | NONE — atlas-trust-core untouched | Tests confirm |
| Welle 5 project_events + Welle 3 graph_state_hash + Welle 4 parse_projector_run_attestation | NONE — Welle 6 only invokes them as a library consumer | Tests confirm |

---

## Out-of-scope this welle (V2-α later wellen + V2-β)

- **V2-α Welle 7 candidate:** ArcadeDB driver integration — replace in-memory `GraphState` with ArcadeDB-backed implementation; the CI gate from Welle 6 works equally well on either backend
- **V2-α Welle 8 candidate:** atlas-signer CLI integration — `--emit-projector-attestation` flag that runs the Welle-5 emitter + signs the result; closes the end-to-end producer-side CLI
- **V2-β candidates:** parallel-projection design for >10M event scenarios; Mem0g Layer-3 cache; Read-API endpoints; MCP V2 tools; expanded event-kind support (annotation, policy, anchor)
- **Counsel-gated:** content-hash separation (per `DECISION-COUNSEL-1`)

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| Master Plan §3 Three-Layer Trust Architecture | `docs/V2-MASTER-PLAN.md` |
| Master Plan §4 R-A-01 (projection determinism drift) | `docs/V2-MASTER-PLAN.md` |
| Master Plan §6 V2-α Foundation (Welle 6 = this) | `docs/V2-MASTER-PLAN.md` |
| `DECISION-ARCH-1` triple-hardening | `.handoff/decisions.md` |
| `DECISION-SEC-2` (Phase 2 Security Q-SEC-6) | `.handoff/decisions.md` |
| Welle 3 `graph_state_hash` (used by gate for recomputation) | `crates/atlas-projector/src/canonical.rs` |
| Welle 4 `parse_projector_run_attestation` (used by gate to parse attestation events) | `crates/atlas-trust-core/src/projector_attestation.rs` |
| Welle 5 `project_events` (used by gate for re-projection) | `crates/atlas-projector/src/upsert.rs` |
| V1 `AtlasTrace` (input type) | `crates/atlas-trust-core/src/trace_format.rs::AtlasTrace` |

---

**End of Welle 6 Plan.** Implementation proceeds on branch `feat/v2-alpha/welle-6-projection-gate` in TDD order: write integration test FIRST (happy-path RED), implement gate (GREEN), add negative cases (RED-GREEN per case), refactor (IMPROVE). Single coherent SSH-signed commit per Atlas standing protocol.
