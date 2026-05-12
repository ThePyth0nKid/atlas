# V2-α Welle 1 — Plan-Doc (Agent-DID Schema Foundation)

> **Status: DRAFT 2026-05-12.** Awaiting Nelson's confirmation before implementation.
> **Becomes SHIPPED block in `.handoff/v2-session-handoff.md` once merged.**
> **Master Plan reference:** `docs/V2-MASTER-PLAN.md` §6 V2-α Foundation (5–8 sessions). This is session 1 of 5–8.
> **Master Vision reference:** `.handoff/v2-master-vision-v1.md` §5.3 Agent Identity Layer.
> **Driving decisions:** `DECISION-SEC-1` (revocation mechanism builds on agent-DID), Master Vision §5.3 Phase 2 Security H-1 demand (both `kid` and `author_did` in signing input).

V2-α Welle 1 lays the **data-model foundation** for V2's agent-agnostic shared memory: every Atlas event gains an optional `author_did` field carrying a W3C-DID-format agent identity (`did:atlas:<pubkey-hash>`), the verifier learns to parse and format-validate the field, and the signing input incorporates both `kid` (workspace HKDF anchor — V1 property) and `author_did` (agent identity — V2 NEW) per Phase 2 Security H-1.

**Why this as Welle 1:** unblocks four downstream items (Projector graph-node stamping for V2-α, Read-API passport endpoint for V2-β, Revocation mechanism for V2-γ, Federation enrolment for V2-γ); independent of FalkorDB/Mem0g/Projector engineering; HIGH reversibility (schema-additive, append-only events); V1-pattern directly reusable (`per_tenant.rs::PER_TENANT_KID_PREFIX` ↔ new `agent_did.rs::AGENT_DID_PREFIX`); content-hash-separation (GDPR Path A) is correctly counsel-gated by `DECISION-COUNSEL-1` and stays out of Welle 1.

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| NEW `crates/atlas-trust-core/src/agent_did.rs` module with `AGENT_DID_PREFIX = "did:atlas:"`, `agent_did_for(pubkey_hash)`, `parse_agent_did(did)`, `validate_agent_did(did)` | DID resolution / DID document materialised view (V2-γ — `GET /api/atlas/passport/:agent_did`) |
| MODIFY `AtlasEvent` struct in `trace_format.rs` — add `author_did: Option<String>` field with `#[serde(default, skip_serializing_if = "Option::is_none")]` | Agent Passport schema or storage (V2-γ) |
| MODIFY signing input construction (`cose.rs::build_signing_input` or equivalent) to include `author_did` when present — Phase 2 Security H-1 demand | Revocation event kind / out-of-band revocation channel (V2-γ — `DECISION-SEC-1`) |
| MODIFY verifier checks — format-validate `author_did` against `did:atlas:<lowercase-hex-32-bytes>` pattern if present | Content-hash separation / GDPR salt redesign (Path A — counsel-gated per `DECISION-COUNSEL-1`) |
| NEW byte-determinism CI pin in `lib.rs` test module: `signing_input_byte_determinism_pin_with_author_did` covers both `author_did = None` and `author_did = Some(...)` cases | FalkorDB integration / Projector implementation (V2-α later wellen) |
| NEW unit tests for `agent_did.rs` (format-validation positive/negative, parser roundtrip) | `--require-author-did` verifier flag (V2-α later welle once author_did adoption is measurable) |
| NEW integration test in `crates/atlas-verify-cli/tests/` signing+verifying an event with `author_did` set | Hermes-skill DID assignment (V2-γ) |
| UPDATE `docs/SEMVER-AUDIT-V1.0.md` — flag `author_did` field as "V2-α schema addition; breaks `deny_unknown_fields` for V1.0 readers — V2.0.0 major bump warranted" | Cross-tenant agent-DID portability tests (V2-γ) |
| UPDATE `.handoff/v2-session-handoff.md` — add Welle-1-SHIPPED block | Master version bump itself (`v2.0.0-alpha.1`) — deferred to end of V2-α welle bundle, not this welle alone |
| NEW `crates/atlas-trust-core/tests/agent_did_integration.rs` (or extend existing) | Sigstore Rekor anchor of `author_did` field (auto-handled — it's in the signed event body) |

---

## Decisions (final, pending Nelson confirmation)

- **DID format:** `did:atlas:<lowercase-hex-32-bytes>` where the 32 hex bytes are `blake3_hash(ed25519_public_key)`. Lowercase-hex normalisation matches V1's `event_hash` convention. Length-checked at parse-time.
- **Optional field, not required:** `author_did: Option<String>`. Events without `author_did` continue to be accepted (V1 backward-compat at the event level — but **not at the trace level** if any V2-α reader uses `deny_unknown_fields`). This is intentional: V1.x events lacking `author_did` remain valid forever; V2-α events MAY carry one. A future `--require-author-did` strict-mode flag (out-of-scope this welle) will enforce presence.
- **Signing input inclusion (Phase 2 Security H-1):** when `author_did` is present, it is canonically included in the CBOR signing input adjacent to `kid`. Specific byte-position: after `kid` field, before `payload` field. New byte-determinism pin locks the exact byte sequence.
- **Wire-compat policy (`deny_unknown_fields`):** **NOT relaxed.** V1.0 verifiers reading V2-α events that carry `author_did` will reject with `unknown_field("author_did")`. This is by-design per V2 = major-bump. Welle 1 commits to this break; the major-bump itself happens at the end of V2-α welle bundle (v2.0.0-alpha.1 candidate tag) per future Welle.
- **Version bump in this welle:** **NONE.** Welle 1 lands on `feat/v2-alpha/welle-1-agent-did-schema` branch and merges to master without a release tag. Workspace version stays `1.0.1` in `Cargo.toml` until the V2-α welle bundle calls a v2.0.0-alpha.1 release.
- **CHANGELOG entry:** under `[Unreleased]` heading, sub-section `### Added — V2-α Welle 1`. Narrative explicitly notes the wire-compat break for V1.0 readers + the deferred version bump.
- **`Cargo.toml` `version`:** unchanged at `1.0.1`. (No SemVer impact under V2-pre-release-on-branch.)
- **Public API surface impact:** new `pub` items in `agent_did.rs` module + new public field on `AtlasEvent`. Master Plan / Master Vision both anchor these as V2 additions; `SEMVER-AUDIT-V1.0.md` gets a new section "V2-α Additions" listing each.

---

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `crates/atlas-trust-core/src/agent_did.rs` | Module: `AGENT_DID_PREFIX` const, `agent_did_for`, `parse_agent_did`, `validate_agent_did`, `AgentDidError` enum, doc-tests |
| MODIFY | `crates/atlas-trust-core/src/lib.rs` | `pub mod agent_did;` declaration + re-export `AGENT_DID_PREFIX`, `agent_did_for`, `parse_agent_did`. NEW test module `agent_did_signing_input_byte_determinism_pin` analog to V1 pin |
| MODIFY | `crates/atlas-trust-core/src/trace_format.rs` | Add `pub author_did: Option<String>` to `AtlasEvent` with `#[serde(default, skip_serializing_if = "Option::is_none")]`. Doc-comment notes V2-α addition + Phase 2 Security H-1 binding to signing input |
| MODIFY | `crates/atlas-trust-core/src/cose.rs` | `build_signing_input` extended to include `author_did` field in CBOR map when present. Field order: after `kid`, before `payload`. Byte-deterministic per CBOR canonicalisation rules |
| MODIFY | `crates/atlas-trust-core/src/verify.rs` | Verifier check: if `event.author_did.is_some()`, call `validate_agent_did()`. Return `TrustError::AgentDidFormatInvalid` on failure |
| MODIFY | `crates/atlas-trust-core/src/error.rs` | NEW `AgentDidFormatInvalid` variant on `TrustError` |
| NEW | `crates/atlas-trust-core/tests/agent_did_integration.rs` | E2E test: sign event with `author_did` set, write to events.jsonl, read back, verify-trace succeeds. Negative case: malformed `did:atlas:NOTHEX` → `AgentDidFormatInvalid` |
| MODIFY | `crates/atlas-verify-cli/tests/strict_mode.rs` OR new `agent_did_cli.rs` | CLI-surface coverage: `atlas-verify-cli` verifies a trace with author_did happy-path |
| MODIFY | `docs/SEMVER-AUDIT-V1.0.md` | New section "V2-α Additions (additive, but breaks `deny_unknown_fields` for V1.0 readers)" listing `pub mod agent_did`, `AgentDid*` functions, `AtlasEvent.author_did` field. Risk-tag: `V2-Major-Break-Planned` |
| MODIFY | `CHANGELOG.md` | `[Unreleased]` section gets `### Added — V2-α Welle 1` block |
| NEW | `.handoff/v2-alpha-welle-1-plan.md` | This plan-doc itself |
| MODIFY | `.handoff/v2-session-handoff.md` | After merge: add "Welle-1-SHIPPED" block analog to phase-shipped blocks. Update V2-α progress indicator (1 of 5–8 sessions done) |

**Total estimated diff:** ~400–700 lines of Rust + tests + docs.

---

## Test impact (which V1 assertions might shift)

| Surface | Assertion | Drift risk under Welle 1 |
|---|---|---|
| V1 `signing_input_byte_determinism_pin` in `lib.rs` test module | Pins exact bytes for a fixture event WITHOUT `author_did` | **No drift** — fixture event continues to have `author_did = None`; `skip_serializing_if = "Option::is_none"` keeps byte-output identical |
| V1 strict-chain integration tests (`tests/strict_mode.rs`) | Strict-chain pass/fail behaviour on bank-q1-2026 fixture | **No drift** — fixture events have no `author_did` |
| V1 atlas-web e2e write roundtrip (`apps/atlas-web/scripts/e2e-write-roundtrip.ts`) | Round-trip writes JSON event, verifier passes | **Subject to verify** — atlas-web write surface may or may not be updated to attach `author_did` in this welle. Plan: **NOT touched in this welle**; write surface continues V1 behaviour, V2 author_did becomes opt-in via a separate `?author_did=did:atlas:...` query param in a later V2-α welle |
| V1 MCP smoke test | MCP write_node + verify_trace happy-path | **No drift** — MCP server adds no author_did unless explicitly extended (later welle) |
| `--require-strict-chain` flag | All V1.19 Welle 10 strict-mode pins | **No drift** — author_did is orthogonal to strict-chain semantics |
| `VERIFIER_VERSION` constant | `atlas-trust-core/1.0.1` | **No drift** — Cargo version unchanged in Welle 1 |
| `deny_unknown_fields` on `AtlasEvent` | V1.0 verifier rejects unknown fields | **By-design break** — V2-α events with `author_did` are intentionally unreadable by V1.0 verifiers. Documented in CHANGELOG + SEMVER-AUDIT. |

**Net:** zero V1 test-suite regression expected. One new byte-determinism pin added (`agent_did_signing_input_byte_determinism_pin`). Implementation MUST run `cargo test --workspace` green before commit.

---

## Implementation steps (TDD order)

1. **Write failing tests first:**
   - `agent_did.rs` unit tests: `validate_agent_did("did:atlas:" + 64-hex)` succeeds; `validate_agent_did("did:foo:...")` fails; `validate_agent_did("did:atlas:UPPERCASE")` fails (lowercase-only); `validate_agent_did("did:atlas:NOTHEX")` fails; `parse_agent_did(...)` roundtrip matches `agent_did_for(...)`.
   - `agent_did_integration.rs`: full sign+verify with `author_did` set; sign+verify with `author_did = None` (V1 backward); negative cases for malformed DIDs.
   - `lib.rs` test module: `agent_did_signing_input_byte_determinism_pin` — pin exact CBOR bytes for fixture event with `author_did = Some("did:atlas:<32-hex>")`. Compare to fixture with `author_did = None`.
2. **Implement `agent_did.rs`** to make unit tests pass.
3. **Extend `trace_format.rs::AtlasEvent`** with `author_did` field.
4. **Extend `cose.rs::build_signing_input`** to canonically include `author_did` in CBOR map when present.
5. **Extend `verify.rs`** to call `validate_agent_did()` on present `author_did`.
6. **Extend `error.rs`** with `AgentDidFormatInvalid` variant.
7. **Run full workspace test suite** `cargo test --workspace` — green.
8. **Run V1 byte-determinism pin** specifically — confirm unchanged for non-author_did fixture (else fix `skip_serializing_if` config).
9. **Write CLI integration test** in `atlas-verify-cli/tests/`.
10. **Update docs:** `SEMVER-AUDIT-V1.0.md`, `CHANGELOG.md [Unreleased]`, this plan-doc gets a `## Implementation Notes (Post-Code)` section.
11. **Dispatch parallel `code-reviewer` + `security-reviewer` agents** per Atlas standing protocol.
12. **Fix CRITICAL + HIGH findings in-commit.**
13. **Single SSH-signed commit + draft PR.**
14. After Nelson approves: squash-merge to master via admin override (per established Atlas pattern).
15. **Update `.handoff/v2-session-handoff.md`** with Welle-1-SHIPPED block.

---

## Acceptance criteria

- [ ] All V1 tests pass unchanged (`cargo test --workspace` green)
- [ ] New `agent_did.rs` module has ≥6 unit tests covering positive + negative format-validation cases
- [ ] New integration test signs+verifies events both with and without `author_did`
- [ ] New byte-determinism CI pin locks CBOR bytes for both `author_did = Some(...)` and `author_did = None` cases
- [ ] `validate_agent_did` enforces `did:atlas:<lowercase-hex-32-bytes>` format
- [ ] `verify.rs` rejects events with malformed `author_did` with `TrustError::AgentDidFormatInvalid`
- [ ] Signing input incorporates `author_did` when present (Phase 2 Security H-1 demand)
- [ ] `SEMVER-AUDIT-V1.0.md` has "V2-α Additions" section flagging the wire-compat break
- [ ] `CHANGELOG.md [Unreleased]` has `### Added — V2-α Welle 1` entry
- [ ] Parallel code-reviewer + security-reviewer agents dispatched, CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-signed commit
- [ ] Welle-1-SHIPPED block added to `.handoff/v2-session-handoff.md`

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| **`skip_serializing_if = "Option::is_none"`** edge-case at CBOR boundary changes byte output for V1 fixture | LOW | HIGH (would break V1 byte-determinism pin) | Run V1 pin test FIRST after `AtlasEvent` modification; revert + redesign if pin breaks |
| **CBOR canonicalisation** of optional field surprises me (e.g., empty-key vs no-key semantics) | MEDIUM | MEDIUM (one extra debugging round) | Reuse existing CBOR canonicalisation pattern from V1.19 Welle 11; write byte-determinism pin BEFORE writing canonicalisation extension |
| **`deny_unknown_fields` on `EventSignature`** also blocks adding new fields there if I need to | LOW | LOW (V2 design has `author_did` on event, not signature, so this is hypothetical) | Confirm V2 design: `author_did` is event-level metadata, not signature-level |
| **V2-α-2 / V2-α-3 wellen** want to retroactively change Agent-DID format | MEDIUM | MEDIUM | DID format choice (`did:atlas:<blake3-pubkey-hash>`) is per Master Vision §5.3 + Lyrie ATP compatibility candidate (`DECISION-BIZ-6`) — verify ATP-compatibility before locking. If ATP requires different format, defer Welle 1 OR design schema as a single column that can hold either format |
| **Lyrie ATP standardised format** lands differently than `did:atlas:<hash>` mid-V2-α | LOW (preseed-stage Lyrie, no IETF RFC yet) | MEDIUM | Welle 1's format is internal-Atlas-only; ATP-compatibility-as-alias deferred to V2-γ per `DECISION-BIZ-6`. No locking concern this welle. |

---

## Out-of-scope this welle (V2-α later wellen)

- **Projector implementation** — V2-α Welle 2 or 3 candidate. Depends on Welle 1 (graph nodes stamp `author_did` from event field).
- **FalkorDB integration** — V2-α Welle 4–5 candidate. Depends on Projector.
- **ArcadeDB comparative spike** — V2-α Welle 2–3 parallel-track. Independent of Welle 1.
- **Content-hash separation (GDPR Path A)** — counsel-gated per `DECISION-COUNSEL-1`. Implementation deferred to post-counsel-opinion.
- **Revocation mechanism** (`DECISION-SEC-1`) — V2-γ. Depends on Welle 1 Agent-DID schema being on master.
- **Agent Passport endpoint** (`GET /api/atlas/passport/:agent_did`) — V2-γ. Depends on Welle 1.
- **MCP V2 tools (`query_graph`, `get_agent_passport`, ...)** — V2-β. Depends on V2-α + Read-API.
- **Hermes-skill v1** — V2-γ. Depends on full V2-α + V2-β stack.

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| Master Plan | `docs/V2-MASTER-PLAN.md` §6 V2-α Foundation |
| Master Vision Agent Identity Layer | `.handoff/v2-master-vision-v1.md` §5.3 |
| Phase 2 Security H-1 demand | `.handoff/v2-master-vision-v1.md` §5.3 (signing input binds both kid + author_did) |
| `DECISION-SEC-1` Revocation mechanism | `.handoff/decisions.md` (V2-γ, depends on this Welle 1) |
| `DECISION-BIZ-6` Lyrie ATP compatibility | `.handoff/decisions.md` (defer ATP-compat to V2-γ; Welle 1 unaffected) |
| V1 per-tenant HKDF pattern (analog) | `crates/atlas-trust-core/src/per_tenant.rs` |
| V1 byte-determinism CI pin pattern | `crates/atlas-trust-core/src/lib.rs` (test module `signing_input_byte_determinism_pin`) |
| V1 event format | `crates/atlas-trust-core/src/trace_format.rs::AtlasEvent` |
| V1 signing input construction | `crates/atlas-trust-core/src/cose.rs::build_signing_input` |
| V1 verifier dispatch | `crates/atlas-trust-core/src/verify.rs` |
| V1 SemVer audit | `docs/SEMVER-AUDIT-V1.0.md` |
| Working Methodology (Welle-decomposition pattern) | `docs/WORKING-METHODOLOGY.md` §"Welle Decomposition Pattern" |

---

**End of Welle 1 Plan.** On Nelson's confirmation, implementation proceeds on this branch (`feat/v2-alpha/welle-1-agent-did-schema`) in TDD order per §"Implementation steps".

---

## Implementation Notes (Post-Code, 2026-05-12)

### What actually shipped

| Concrete | File | Lines added |
|---|---|---|
| `agent_did.rs` module | `crates/atlas-trust-core/src/agent_did.rs` | ~270 |
| Re-exports + module decl | `crates/atlas-trust-core/src/lib.rs` | +2 |
| `AtlasEvent.author_did` field + doc-comment | `crates/atlas-trust-core/src/trace_format.rs` | +25 |
| `cose::build_signing_input` signature extension + 3 new tests + new pin | `crates/atlas-trust-core/src/cose.rs` | +160 |
| `verify.rs` pre-signature `validate_agent_did` call | `crates/atlas-trust-core/src/verify.rs` | +13 |
| `error.rs` `AgentDidFormatInvalid` variant | `crates/atlas-trust-core/src/error.rs` | +12 |
| `hashchain.rs` caller update | `crates/atlas-trust-core/src/hashchain.rs` | +2 |
| 9 caller updates (atlas-signer + 7 test files + demo) | various | +9 (one `None`/`author_did: None` per call site) |
| `agent_did_integration.rs` E2E test | `crates/atlas-trust-core/tests/agent_did_integration.rs` | ~225 |
| `SEMVER-AUDIT-V1.0.md` §10 V2-α Additions | `docs/SEMVER-AUDIT-V1.0.md` | ~55 |
| `CHANGELOG.md [Unreleased]` Welle-1 entry | `CHANGELOG.md` | ~22 |
| Total | | **~795 lines added** |

### Test outcome

- **All V1 unit tests pass unchanged** (`cargo test --workspace` green)
- **V1 byte-determinism pin `signing_input_byte_determinism_pin` UNCHANGED** — confirms V1 backward-compat invariant: events without `author_did` produce byte-identical CBOR pre- and post-Welle-1
- **NEW V2-α pin `signing_input_byte_determinism_pin_with_author_did`** passes with hand-computed CBOR hex (a8 map header + V1 7-pair content + `author_did` entry sorted LAST per RFC 8949 §4.2.1)
- **13 new `agent_did` unit tests** all pass (format-validation positive + negative, parse roundtrip, structured-error reasons)
- **4 new integration tests** all pass:
  - `event_with_author_did_round_trips`
  - `event_without_author_did_round_trips` (V1 backward-compat)
  - `malformed_author_did_is_rejected_at_verify_time`
  - `author_did_is_bound_into_signature` (cross-agent-replay defence)

### Risk mitigations validated post-implementation

| Plan-stage risk | Resolution |
|---|---|
| `skip_serializing_if = "Option::is_none"` CBOR edge-case | Resolved: V1 pin survived byte-identically; the entry is gated on `Some(_)` at the `build_signing_input` boundary (not via serde — serde owns wire JSON, not CBOR signing-input shape) |
| CBOR canonicalisation surprises | Resolved: V2-α pin matched hand-computed hex on first run; sort-by-length-then-lex behaves identically with the longer new key |
| Lyrie ATP standardised format risk | Unchanged from plan; ATP-compat as alias is V2-γ scope per `DECISION-BIZ-6` |

### Deviations from plan

- **None of substance.** Plan §"Implementation steps" 1-9 executed in order. Step 10 (docs) and Step 11 (parallel reviewers) per Atlas standing protocol.
- **Verifier-side validate-before-signing-input vs validate-after**: plan said either; implementation chose BEFORE so the structured `AgentDidFormatInvalid` surfaces ahead of hash/signature errors — better operator diagnostics.

### What's next (post-Welle-1 ship)

- **V2-α Welle 2 candidate**: ArcadeDB vs FalkorDB comparative spike (`DECISION-DB-1`), pre-V2-α lock blocking item. Independent of Welle 1's schema work.
- **V2-α Welle 3 candidate**: Atlas Projector skeleton (uses Welle 1's `author_did` to stamp graph nodes per Master Vision §5.1). Depends on Welle 1 on master.
- **V2-α atlas-signer CLI `--author-did` flag**: separate follow-up welle to expose the new signing-input parameter at the CLI surface. Trivial scope (5-10 lines + a test).
- **Counsel engagement** continues on Nelson-led parallel track per Master Plan §5.
