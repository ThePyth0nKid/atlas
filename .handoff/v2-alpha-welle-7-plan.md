# V2-α Welle 7 — Plan-Doc (atlas-signer CLI `emit-projector-attestation` subcommand)

> **Status: DRAFT 2026-05-13.** Awaiting Nelson's confirmation before merge.
> **Master Plan reference:** `docs/V2-MASTER-PLAN.md` §6 V2-α Foundation. Session 7 of 5–8.
> **Driving context:** Welles 1+3+4+5+6 delivered the V2-α projection trust-chain end-to-end on the library side. Welle 7 closes the **producer-side CLI ergonomic** so an operator can in one shell command read `events.jsonl`, project, and emit a signed `ProjectorRunAttestation` event ready to append back to the JSONL.

Welle 7 makes atlas-signer the **first in-tree consumer of atlas-projector**. After Welle 7:

```
$ atlas-signer emit-projector-attestation \
    --events-jsonl trace/events.jsonl \
    --workspace-id ws-q1-2026 \
    --derive-from-workspace ws-q1-2026 \
    --head-event-hash <hex>
> { "event_id": "01H...", "payload": { "type": "projector_run_attestation", ... }, "signature": ... }
```

Operator pipes stdout back to `events.jsonl` and the trace now carries a signed, verifiable attestation. Welle 6's gate can be invoked on the resulting trace and will produce `GateStatus::Match`.

**Why this as Welle 7** (rather than ArcadeDB integration):
- Tightly scoped, 1-session-machbar
- Validates the atlas-projector public API surface from a real in-tree consumer (was hypothetical until now)
- HIGH reversibility (CLI flag, additive)
- Independent of ArcadeDB
- Closes producer-side CLI ergonomic — demo asset for any V2-α presentation
- ArcadeDB is the right Welle 8 candidate — bigger architectural scope, deserves dedicated session

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| NEW `EmitProjectorAttestationArgs` struct + `Command::EmitProjectorAttestation(args)` clap subcommand in atlas-signer | ArcadeDB driver integration (Welle 8 candidate) |
| NEW `run_emit_projector_attestation(args, signer)` dispatcher function | Multi-attestation streaming (Welle-7-MVP emits ONE attestation per invocation) |
| NEW core orchestration function `build_signed_projector_attestation_event(events_jsonl, workspace_id, projector_version, head_event_hash, ts, kid, sign_callback) -> serde_json::Value` — testable, no I/O | atlas-projector incremental-attestation semantics (Welle 8+) |
| CLI flags: `--events-jsonl <path>`, `--workspace-id <id>`, `--projector-version <string, default "atlas-projector/<crate-version>">`, `--head-event-hash <hex>`, `--ts <iso8601, default now>`, plus standard signer args (`--kid` OR `--derive-from-workspace`, master-seed env-var gate) | atlas-projector cli binary (deferred — atlas-signer is the right place for now per established Atlas convention) |
| Add `atlas-projector` as dependency in atlas-signer's `Cargo.toml` (first in-tree consumer) | Re-verification of `events.jsonl` event signatures (caller's responsibility — atlas-signer is a producer, not a verifier; documented assumption) |
| Reads `events.jsonl` from filesystem path (atlas-signer is a CLI, not a library — file I/O is in scope here) | Reading `events.jsonl` from stdin (defer to operator-runbook convention — could be added later additively) |
| Output: one JSON line (canonical-CBOR-equivalent shape via serde_json) to stdout, ready for append to `events.jsonl` | atlas-web UI integration of the new subcommand |
| 6+ unit tests covering: well-formed roundtrip, malformed events.jsonl input, missing head_event_hash, attestation event passes Welle 6 gate on output | E2E binary tests via `assert_cmd` (out of scope; unit-test the orchestration function instead) |
| Update `docs/SEMVER-AUDIT-V1.0.md` §10 with V2-α Welle 7 atlas-signer additions | Sigstore Rekor anchoring of the emitted attestation event (caller runs `atlas-signer anchor` separately as already supported in V1.6) |
| Update `CHANGELOG.md [Unreleased]` | Operator-runbook (defer to V2-α-bundle release notes; CLI `--help` text is the in-scope docs surface) |
| Plan-doc (this file) | |

---

## Decisions (final, pending Nelson confirmation)

- **Subcommand name:** `emit-projector-attestation` (kebab-case per established `derive-workspace` / `rotate-pubkey-bundle` convention). Long but unambiguous; auditor-friendly in shell history.
- **Subcommand belongs in atlas-signer, NOT a new binary.** atlas-signer already owns the production-side signing pipeline (event_id ULID generation, ts handling, signing-input construction, key management). Adding the attestation flow as a subcommand reuses all of that machinery rather than duplicating it.
- **Output format:** ONE JSON line (no leading whitespace, no trailing newline beyond the single LF). Compatible with `>> events.jsonl` append. Operator wrapper: `atlas-signer emit-projector-attestation ... >> trace/events.jsonl`.
- **Default `--projector-version` value:** `"atlas-projector/<crate-version-from-CARGO_PKG_VERSION>"`. Bound to atlas-projector's crate version, NOT atlas-signer's, so the projector identity in the signed event reflects the actual projection-logic used. Operator MAY override.
- **Default `--ts` value:** ISO-8601 of `chrono::Utc::now()`. Operator MAY override for deterministic test cases.
- **`event_id` generation:** ULID via existing atlas-signer machinery (matches what `sign` subcommand does). No special V2-α handling — the attestation event is a normal Atlas event from the signer's perspective.
- **`parent_hashes` for the attestation event:** Welle-7-MVP emits with `parent_hashes = []` (no parent claim). Operator may post-process if they want to link the attestation into the DAG. Future welles may add `--parent-hash <hex>` flag.
- **Workspace-id requirement:** `--workspace-id` is REQUIRED (no default). Reflects the workspace the attestation is being made for. atlas-projector's `project_events` uses it for entity_uuid derivation.
- **`head_event_hash` requirement:** REQUIRED. CLI does NOT auto-derive (e.g. by reading the trace's last event) — that introduces ambiguity. Operator supplies explicitly. Future welle may add `--head-event-hash auto` to derive from the events.jsonl input.
- **`projected_event_count` derivation:** AUTO-DERIVED from the count of projectable events in the JSONL input. CLI does NOT take this as an arg — issuer would be lying by overriding it.
- **author_did:** NOT set on the attestation event by Welle 7. The attestation itself isn't an agent action; it's a projector-process artefact. Future welles may add `--author-did` for agents that wrap projection-and-attestation in a unified identity.
- **Test scope:** unit-test the orchestration function (parse + project + build payload + simulate signing via callback). Skip binary-level `assert_cmd` tests (test-suite overhead exceeds welle value).
- **Version bump in this welle:** NONE. Workspace stays at `1.0.1`. Deferred to V2-α welle-bundle close-out.

---

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| MODIFY | `crates/atlas-signer/Cargo.toml` | Add `atlas-projector = { path = "../atlas-projector" }` + `chrono` if not already (for ts default) |
| MODIFY | `crates/atlas-signer/src/main.rs` | NEW `Command::EmitProjectorAttestation(EmitProjectorAttestationArgs)` variant + new args struct + `run_emit_projector_attestation` dispatcher + `build_signed_projector_attestation_event` core orchestration function + 6+ unit tests in the test module |
| MODIFY | `docs/SEMVER-AUDIT-V1.0.md` | §10.7e new subsection listing atlas-signer's new public CLI surface |
| MODIFY | `CHANGELOG.md` | `[Unreleased]` gets `### Added — V2-α Welle 7` block ordered above Welle 6 |
| NEW | `.handoff/v2-alpha-welle-7-plan.md` | This plan-doc |

**Total estimated diff:** ~500-800 lines Rust + tests + docs.

---

## Algorithm sketch

```rust
struct EmitProjectorAttestationArgs {
    /// Path to events.jsonl input.
    #[arg(long)]
    events_jsonl: PathBuf,
    /// Workspace identifier this attestation is being made for.
    #[arg(long)]
    workspace_id: String,
    /// blake3 hex of the last event consumed (64 lowercase-hex).
    #[arg(long)]
    head_event_hash: String,
    /// Atlas-projector binary identity. Defaults to "atlas-projector/<crate-version>".
    #[arg(long)]
    projector_version: Option<String>,
    /// ISO-8601 timestamp. Defaults to chrono::Utc::now().
    #[arg(long)]
    ts: Option<String>,
    // ... plus standard signer args (--kid OR --derive-from-workspace, etc.)
}

fn run_emit_projector_attestation(
    args: EmitProjectorAttestationArgs,
    signer: Option<&dyn WorkspaceSigner>,
) -> ExitCode {
    // 1. Read events.jsonl file
    let contents = std::fs::read_to_string(&args.events_jsonl)?;
    // 2. Parse via atlas-projector
    let events = atlas_projector::parse_events_jsonl(&contents)?;
    // 3. Project (filter out any existing attestation events)
    let projectable: Vec<_> = events.iter()
        .filter(|e| /* not attestation kind */)
        .cloned()
        .collect();
    let state = atlas_projector::project_events(&args.workspace_id, &projectable, None)?;
    // 4. Build attestation payload
    let projector_version = args.projector_version
        .unwrap_or_else(|| format!("atlas-projector/{}", env!("CARGO_PKG_VERSION")));
    let payload = atlas_projector::build_projector_run_attestation_payload(
        &state,
        &projector_version,
        &args.head_event_hash,
        projectable.len() as u64,
    )?;
    // 5. Construct AtlasEvent: event_id (ULID), parent_hashes=[], ts, kid, payload
    let ts = args.ts.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let event_id = ulid::Ulid::new().to_string();
    let kid = /* derive from --kid or --derive-from-workspace */;
    // 6. Compute signing input + sign
    let signing_input = atlas_trust_core::cose::build_signing_input(
        &args.workspace_id, &event_id, &ts, &kid, &[], &payload, None,
    )?;
    let event_hash = blake3::hash(&signing_input);
    let sig_bytes = sign(signer, &signing_input)?;
    // 7. Wrap in AtlasEvent + serialize to stdout
    let event = AtlasEvent { event_id, event_hash, parent_hashes: vec![], payload, signature: ..., ts, author_did: None };
    println!("{}", serde_json::to_string(&event)?);
    ExitCode::from(0)
}
```

Realistic. Welle 7 implements this.

---

## Acceptance criteria

- [ ] `cargo check --workspace` green
- [ ] `cargo test --workspace` green; all 7 byte-determinism CI pins byte-identical
- [ ] New `emit-projector-attestation` subcommand registered in clap; `--help` text describes flags
- [ ] Reading a well-formed `events.jsonl` produces a well-formed signed attestation event on stdout
- [ ] The emitted attestation event PASSES Welle 6's gate when included in the original trace (round-trip property)
- [ ] CLI rejects malformed `events.jsonl` with structured error
- [ ] CLI rejects missing `--head-event-hash` (clap validation)
- [ ] CLI rejects malformed `--head-event-hash` (not 64-hex) — surfaced by atlas-projector's emission boundary check (DefenseInDepth from Welle 5 reviewer fixes)
- [ ] Unit tests cover: well-formed roundtrip, malformed JSONL, missing-head-hash semantic check, attestation-event-passes-Welle-6-gate
- [ ] `SEMVER-AUDIT-V1.0.md` §10 lists the new CLI surface
- [ ] `CHANGELOG.md [Unreleased]` has Welle 7 entry
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched
- [ ] CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit, PR opened, self-merge via admin override

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| atlas-signer's existing key-derivation paths (HKDF, HSM, raw private-key) don't compose cleanly with the new orchestration | LOW | LOW | Reuse existing `WorkspaceSigner` trait abstraction; new orchestration is parametric over the signer impl, not bound to a specific path |
| `events.jsonl` input contains an existing `ProjectorRunAttestation` event the operator wants to keep — should we filter it or include it as a projectable? | MEDIUM (operator confusion) | LOW (deterministic behaviour either way) | Welle 7 DECISION: filter out any existing attestation events before projection (same as Welle 6 gate does). Documented in CLI `--help`. |
| `chrono::Utc::now()` default timestamp introduces nondeterminism in tests | LOW | LOW | Test fixtures override `--ts` explicitly; default behaviour for CLI users only |
| `cargo check` fails because clap macro expansion conflicts with atlas-signer's existing complex args | LOW | LOW | clap supports many subcommands; existing atlas-signer already has 6+. Adding one more is mechanical. |
| atlas-signer becoming the first in-tree atlas-projector consumer surfaces unexpected API ergonomic issues | LOW | LOW (good — Welle 7 exercises the API; gaps caught here before downstream consumers hit them) | Tests cover the path; review-pass should catch surface ergonomics |

---

## Test impact (V1 + V2-α assertions to preserve)

| Surface | Drift risk under Welle 7 | Mitigation |
|---|---|---|
| All 7 byte-determinism pins | NONE — Welle 7 only adds a new atlas-signer subcommand + adds atlas-projector to atlas-signer deps | Tests confirm |
| atlas-trust-core verifier behaviour | NONE — atlas-trust-core untouched | Tests confirm |
| atlas-projector library behaviour | NONE — Welle 7 is a CONSUMER of atlas-projector, no atlas-projector code touched | Tests confirm |
| existing atlas-signer subcommands (derive-workspace, rotate-pubkey-bundle, anchor, sign) | NONE — new subcommand is additive; existing dispatch paths unchanged | Tests confirm |
| Welle 6 gate behaviour | NONE — gate is library function; Welle 7 only produces input the gate consumes | Round-trip test confirms |

---

## Out-of-scope this welle (V2-α later wellen + V2-β)

- **V2-α Welle 8 candidate:** ArcadeDB driver integration — replace in-memory `GraphState` with ArcadeDB-backed implementation
- **V2-α Welle 9 candidate (if 5-8 range is now exceeded):** parallel-projection design for >10M event scenarios; OR Mem0g Layer-3 cache (V2-β crossover)
- **V2-β candidates:** Read-API endpoints, MCP V2 tools, expanded event-kind support (annotation, policy, anchor), atlas-web UI integration
- **Counsel-gated:** content-hash separation (per `DECISION-COUNSEL-1`)

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| Master Plan §3 Three-Layer Trust Architecture | `docs/V2-MASTER-PLAN.md` |
| Master Plan §6 V2-α Foundation (Welle 7 = this) | `docs/V2-MASTER-PLAN.md` |
| Welle 5 `project_events` + `build_projector_run_attestation_payload` | `crates/atlas-projector/src/upsert.rs` + `emission.rs` |
| Welle 4 `parse_projector_run_attestation` | `crates/atlas-trust-core/src/projector_attestation.rs` |
| Welle 6 `verify_attestations_in_trace` (round-trip target) | `crates/atlas-projector/src/gate.rs` |
| V1 atlas-signer signing pipeline (event_id, ts, kid, signing_input, signature) | `crates/atlas-signer/src/main.rs::run_sign` |
| V1 WorkspaceSigner trait | `crates/atlas-signer/src/keys.rs` (lookup confirmed) |

---

**End of Welle 7 Plan.** Implementation proceeds on branch `feat/v2-alpha/welle-7-signer-attestation-cli` in TDD order: write orchestration-function tests FIRST, implement core function, wire CLI subcommand, integration-test via test fixtures. Single coherent SSH-signed commit per Atlas standing protocol.
