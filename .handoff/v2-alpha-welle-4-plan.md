# V2-α Welle 4 — Plan-Doc (ProjectorRunAttestation Event-Schema + Verifier-Side Parsing)

> **Status: DRAFT 2026-05-12.** Awaiting Nelson's confirmation before merge. Becomes SHIPPED block in `.handoff/v2-session-handoff.md` once merged.
> **Master Plan reference:** `docs/V2-MASTER-PLAN.md` §6 V2-α Foundation. Session 4 of 5–8.
> **Driving decision:** `DECISION-SEC-2` (Phase 2 Security Q-SEC-6) — *"every projector run emits a signed ProjectorRunAttestation event into Layer 1 asserting (projector_version, head_hash) → graph_state_hash. Makes determinism part of the trust chain, not just CI hygiene."*

Welle 4 **bridges Welle 3's `graph_state_hash` into the V1 Layer-1 trust chain** by introducing a new event kind, `ProjectorRunAttestation`, that carries the hash as a signed payload. After Welle 4 ships:
- Atlas issuers can emit signed events asserting `(projector_version, head_event_hash) → graph_state_hash`
- Verifier validates the payload format strictly and recognises the event-kind as a first-class trust-chain artefact
- Any third party with the offline WASM verifier + the `events.jsonl` + the `pubkey-bundle.json` can independently verify "this projector run on this event head produced this graph state at time T"
- The projector-state-hash CI gate (Welle 6 candidate) becomes a cryptographic check (Rekor-anchored attestation match), not just a comparison

**Why this as Welle 4** (rather than events.jsonl-replay/upsert OR ArcadeDB integration):
- Mirror Welle 1 pattern (schema-additive AtlasPayload variant + verifier parser + format-validator + byte-pin) — tightly scoped, reviewable in 1 session
- Independent of ArcadeDB driver setup (the attestation is just data carried in a signed Atlas event)
- Independent of full events→upsert idempotency logic — Welle 4 only needs to parse-and-validate, not emit (emission happens when the actual projector code runs, which is downstream wellen)
- **Maximum security ROI per session:** elevates Welle 3's primitive from CI-gate-only to cryptographic-chain-of-custody artefact
- Unblocks regulator-auditor verification of projection determinism (Master Vision §5.6 "Continuous regulator attestation" — regulator witness cosigners would cosign ProjectorRunAttestation events to confirm projection integrity)

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| NEW `AtlasPayload::ProjectorRunAttestation { ... }` variant in `trace_format.rs` (V2-α schema addition) | Actual projector-run emission of these events (Welle 5/6 — depends on real projector with ArcadeDB) |
| NEW `crates/atlas-trust-core/src/projector_attestation.rs` module — typed `ProjectorRunAttestation` struct + parser + strict format validator | events.jsonl reading + idempotent upsert from events → GraphState (Welle 5 candidate) |
| NEW constants: `PROJECTOR_RUN_ATTESTATION_KIND = "projector_run_attestation"` (payload `type` discriminator); `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION = "atlas-projector-run-attestation/v1-alpha"` | ArcadeDB driver integration (Welle 5) |
| NEW `TrustError::ProjectorAttestationInvalid { reason: String }` variant (additive under `#[non_exhaustive]`) | projector-state-hash CI gate enforcement (Welle 6) |
| Verifier-side: when an event's payload type is `projector_run_attestation`, run `validate_projector_run_attestation()` BEFORE signature check (mirrors Welle 1's `validate_agent_did` placement). Format-failure surfaces as `ProjectorAttestationInvalid` with structured reason | Content-hash separation (counsel-gated per DECISION-COUNSEL-1) |
| Strict format-validation: hex hashes 64 lowercase chars (blake3 width); projector_schema_version must equal `atlas-projector-v1-alpha`; projector_version non-empty; projected_event_count >= 1; head_event_hash matches an event in the same trace (V2-β Read-API integrity) — DEFERRED to Welle 5+ where actual emission happens; Welle 4 enforces hex-format-only | Witness cosignature requirement on ProjectorRunAttestation events (V2-γ regulator federation) |
| NEW unit tests for projector_attestation.rs (≥8 tests: parse roundtrip, missing-fields, wrong-schema-version, malformed hex, etc.) | Mem0g cache integration (V2-β) |
| NEW integration test in `crates/atlas-trust-core/tests/projector_attestation_integration.rs` — sign+verify event carrying ProjectorRunAttestation payload, end-to-end | atlas-signer CLI `--projector-attestation` flag (separate trivial welle) |
| NEW byte-determinism CI pin `signing_input_byte_determinism_pin_with_projector_attestation` — locks CBOR bytes for fixture event with ProjectorRunAttestation payload | Atlas-projector emission code (Welle 5: full projector reads events, emits attestation as side-effect) |
| Update `docs/SEMVER-AUDIT-V1.0.md` §10 with V2-α Welle 4 additions | atlas-web UI rendering of attestation events |
| Update `CHANGELOG.md [Unreleased]` | New event kinds beyond ProjectorRunAttestation |
| Plan-doc (this file) | |

---

## Decisions (final, pending Nelson confirmation)

- **Payload `type` discriminator:** `"projector_run_attestation"` (snake_case per existing `AtlasPayload` convention). Becomes the public wire-format identifier.
- **Schema version:** `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION = "atlas-projector-run-attestation/v1-alpha"` — separate from `atlas-projector-v1-alpha` (which is the GraphState canonical form). Why separate: the attestation envelope schema is a wire-format independent of the canonical-graph encoding it asserts. They version on different axes.
- **Required payload fields:**
  - `projector_version: String` — e.g. `"atlas-projector/0.1.0"` (mirrors V1 `VERIFIER_VERSION` shape). Must be non-empty.
  - `projector_schema_version: String` — MUST equal `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION` constant for Welle-4-emitted attestations. Future schema bumps cascade through this value.
  - `head_event_hash: String` — blake3 hex of the last Atlas event the projector consumed. Welle 4 enforces hex format (64 lowercase chars); cross-trace integrity (head matches an event in the same `events.jsonl`) is enforced by the broader strict-mode logic in a later welle when actual emission ships.
  - `graph_state_hash: String` — blake3 hex of `atlas_projector::graph_state_hash(state)` output (64 lowercase chars).
  - `projected_event_count: u64` — non-zero count of events consumed (audit-trail field).
- **`#[serde(deny_unknown_fields)]` on the new variant** — strict schema enforcement; unknown fields rejected at parse time. Consistent with V1 + Welle 1 wire-compat policy.
- **Verifier-side validation runs BEFORE signature check**, mirroring Welle 1's `validate_agent_did` placement in `verify.rs`. Structured `ProjectorAttestationInvalid` surfaces ahead of downstream signature/hash errors. Documented honestly: `check_event_hashes` runs first and may surface a different error if the attestation event itself was tampered with — both errors land in `outcome.errors`.
- **No verifier-side enforcement that the attestation is "fresh" or "valid against the live projector run"** — that's downstream. Welle 4 enforces ONLY payload format + schema. Welle 6 (CI gate) will compare attestation `graph_state_hash` against locally-recomputed `graph_state_hash` from re-projection. Welle 5+ may add cross-event integrity (head_event_hash actually points to an event in the trace).
- **Version bump in this welle:** NONE. Workspace stays at `1.0.1`. Deferred to V2-α welle-bundle close-out.
- **No new dependency on `atlas-projector` from `atlas-trust-core`** — strict DAG. The attestation event-kind validates the PAYLOAD format; it does NOT recompute `graph_state_hash` (that requires the full GraphState, which the verifier doesn't have at attestation-parse time). The CI gate (Welle 6) imports atlas-projector for recomputation; the verifier alone validates format.

---

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `crates/atlas-trust-core/src/projector_attestation.rs` (~250 lines) | `PROJECTOR_RUN_ATTESTATION_KIND` + `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION` consts + `ProjectorRunAttestation` struct + `parse_projector_run_attestation(payload: &serde_json::Value) -> TrustResult<ProjectorRunAttestation>` + `validate_projector_run_attestation(att: &ProjectorRunAttestation) -> TrustResult<()>` + 8+ unit tests covering parse-roundtrip, missing-fields, wrong-schema-version, malformed hex, non-empty projector_version, etc. |
| MODIFY | `crates/atlas-trust-core/src/trace_format.rs` | Add `AtlasPayload::ProjectorRunAttestation { projector_version, projector_schema_version, head_event_hash, graph_state_hash, projected_event_count }` variant. `#[serde(deny_unknown_fields)]` on the variant struct fields. |
| MODIFY | `crates/atlas-trust-core/src/error.rs` | NEW `TrustError::ProjectorAttestationInvalid { reason: String }` variant (`#[non_exhaustive]` — additive). |
| MODIFY | `crates/atlas-trust-core/src/lib.rs` | `pub mod projector_attestation;` + re-exports: `parse_projector_run_attestation`, `validate_projector_run_attestation`, `ProjectorRunAttestation`, `PROJECTOR_RUN_ATTESTATION_KIND`, `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION`. |
| MODIFY | `crates/atlas-trust-core/src/verify.rs` | Detect events whose payload `type` is `projector_run_attestation` (parse as JSON, check `type` field). Run `parse_projector_run_attestation` + `validate_projector_run_attestation`. Failure surfaces as `ProjectorAttestationInvalid` error with structured reason. Placement: before signing-input construction (mirrors Welle 1 author_did validation). |
| NEW | `crates/atlas-trust-core/tests/projector_attestation_integration.rs` (~200 lines) | E2E test: sign event carrying ProjectorRunAttestation payload with valid format → verifier accepts. Negative cases: malformed hex (wrong length / uppercase), wrong schema_version, missing fields. |
| NEW | byte-determinism pin `cose::tests::signing_input_byte_determinism_pin_with_projector_attestation` | Fixture event with ProjectorRunAttestation payload; locked CBOR bytes + blake3 hex. Co-equal with existing 5 byte-determinism pins. |
| MODIFY | `docs/SEMVER-AUDIT-V1.0.md` | §10.7c new subsection listing every new `pub` item with V2-α-Additive tag. |
| MODIFY | `CHANGELOG.md` | `[Unreleased]` gets `### Added — V2-α Welle 4` block ordered above Welle 3. |
| NEW | `.handoff/v2-alpha-welle-4-plan.md` | This plan-doc. |

**Total estimated diff:** ~700-900 lines Rust + tests + docs.

---

## Acceptance criteria

- [ ] `cargo check --workspace` green
- [ ] `cargo test --workspace` green (zero regression — V1's 5 byte-determinism pins must remain byte-identical)
- [ ] `AtlasPayload::ProjectorRunAttestation` parses correctly from JSON wire format with all 5 required fields
- [ ] `validate_projector_run_attestation` rejects malformed hex (wrong length, uppercase, non-hex), wrong schema_version, empty projector_version, zero projected_event_count
- [ ] Verifier surfaces `ProjectorAttestationInvalid` with structured reason on malformed attestation, ahead of signature check
- [ ] Integration test: sign+verify event with valid ProjectorRunAttestation payload succeeds
- [ ] Integration test: tampered attestation (well-formed but wrong hash value) → signature/hash mismatch caught (cross-attestation-replay defence — same as Welle 1's `signature_swap_between_freshly_signed_events_fails` rigour)
- [ ] New byte-pin `signing_input_byte_determinism_pin_with_projector_attestation` pins exact CBOR bytes + blake3 hex
- [ ] `SEMVER-AUDIT-V1.0.md` §10.7c lists every new `pub` item with V2-α-Additive tag
- [ ] `CHANGELOG.md [Unreleased]` has Welle 4 entry
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched
- [ ] CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit, draft-then-ready PR, self-merge via `gh pr merge --squash --admin --delete-branch`

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| AtlasPayload variant addition with `#[serde(deny_unknown_fields)]` doesn't break V1.0 verifiers reading V2-α attestation events — desired by-design, but verify | LOW | LOW (by design; Welle 1 already established this pattern) | Test path covers it; documented in CHANGELOG |
| Verifier-side validation placement (before signature check) duplicates work with `check_event_hashes` for tampered attestation cases | LOW | LOW (both errors land in `outcome.errors`, auditor tooling switches on variant) | Same pattern as Welle 1; comment clarifies ordering |
| `projector_schema_version` mismatch — future V2-β changes the attestation schema and old verifiers reject new attestations | MEDIUM | LOW (clean schema-mismatch error, not silent acceptance) | Schema-version-string is in the payload; failure produces structured `ProjectorAttestationInvalid` with explicit version mismatch reason |
| Cross-trace integrity (head_event_hash actually points to an event in the same trace) NOT enforced in Welle 4 | MEDIUM | LOW (deferred to Welle 5/6 where actual emission + projector-state-hash gate exists) | Documented in plan as Welle-5+ scope; not a Welle-4 correctness gap |
| `ProjectorRunAttestation` payload field naming snake_case-vs-camelCase inconsistency with future event kinds | LOW | LOW | Matches existing `AtlasPayload` enum's `#[serde(rename_all = "snake_case")]` convention; consistency preserved |

---

## Test impact (V1 assertions to preserve)

| V1 surface | Drift risk under Welle 4 | Mitigation |
|---|---|---|
| `cose::signing_input_byte_determinism_pin` | NONE — fixture event has no ProjectorRunAttestation payload; CBOR shape unchanged | Tests confirm |
| `cose::signing_input_byte_determinism_pin_with_author_did` (Welle 1) | NONE — payload type unchanged | Tests confirm |
| `anchor::chain_canonical_body_byte_determinism_pin` (V1.7) | NONE — anchor envelope unchanged | Tests confirm |
| `pubkey_bundle::bundle_hash_byte_determinism_pin` (V1.9) | NONE — bundle shape unchanged | Tests confirm |
| `agent_did::tests::*` (Welle 1) | NONE | Tests confirm |
| `verify_trace` end-to-end on V1-shape traces | NONE — verifier only adds attestation validation on events with payload type `projector_run_attestation`; V1 payloads pass through unchanged | Tests confirm |
| `atlas-projector` tests (Welle 3) | NONE — atlas-projector crate untouched | Tests confirm |

---

## Out-of-scope this welle (V2-α later wellen)

- **V2-α Welle 5 candidate:** ArcadeDB driver integration — replace in-memory GraphState with ArcadeDB-backed implementation; events.jsonl reading; idempotent upsert; ProjectorRunAttestation emission (the producer side; Welle 4 is consumer/verifier side only).
- **V2-α Welle 6 candidate:** projector-state-hash CI gate enforcement — compares attested `graph_state_hash` from a ProjectorRunAttestation event against locally-recomputed value from a fresh re-projection.
- **V2-α Welle 7-8 candidates:** content-hash separation (counsel-gated per `DECISION-COUNSEL-1`), parallel-projection design for >10M event scenarios, atlas-signer CLI `--projector-attestation` emission flag.

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| Master Plan §3 Three-Layer Trust Architecture | `docs/V2-MASTER-PLAN.md` |
| Master Plan §6 V2-α Foundation (Welle 4 = this) | `docs/V2-MASTER-PLAN.md` |
| `DECISION-SEC-2` ProjectorRunAttestation requirement | `.handoff/decisions.md` |
| `DECISION-ARCH-1` triple-hardening (canonicalisation + attestation + parallel-projection) | `.handoff/decisions.md` |
| Welle 3 `graph_state_hash` primitive (input to attestation) | `crates/atlas-projector/src/canonical.rs` |
| Welle 1 Agent-DID Schema (signing-input pattern analog) | `crates/atlas-trust-core/src/agent_did.rs` |
| V1 `AtlasPayload` enum (variant addition target) | `crates/atlas-trust-core/src/trace_format.rs` |
| V1 `TrustError` enum (`#[non_exhaustive]`, additive variant) | `crates/atlas-trust-core/src/error.rs` |
| V1 verifier (validation placement pattern) | `crates/atlas-trust-core/src/verify.rs` |
| Master Vision §5.2 "ProjectorRunAttestation makes determinism part of the trust chain" | `.handoff/v2-master-vision-v1.md` |

---

**End of Welle 4 Plan.** Implementation proceeds on branch `feat/v2-alpha/welle-4-projector-attestation` in TDD order: write tests FIRST (parse roundtrip, format-validation positive+negative), implement parser+validator, integrate into verifier, write integration test, write byte-pin, update docs. Single coherent SSH-signed commit per Atlas standing protocol.
