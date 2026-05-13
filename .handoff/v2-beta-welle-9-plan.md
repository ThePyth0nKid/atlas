# V2-β Welle 9 — Plan-Doc (Operator Runbook v2.0.0-alpha.1)

> **Status:** DRAFT 2026-05-13. Awaiting parent agent's confirmation before merge.
> **Orchestration:** part of Phase 1 (parallel batch 1, docs-only) per `docs/V2-BETA-ORCHESTRATION-PLAN.md`.
> **Driving decisions:** N/A (this is a documentation welle; engineering decisions deferred to V2-β code wellen W12–W14).

V2-α shipped the cryptographic projection-state verification primitive end-to-end (Welles 1–8). Operators now have a new capability: invoke `atlas-signer emit-projector-attestation` to produce a signed attestation event, append it to `events.jsonl`, and operators/verifiers can run the CI gate to detect projection drift cryptographically.

This welle documents the **operator's runbook** for this new V2-α-aware workflow: how to emit attestations, what the attestation format looks like, how to invoke the gate, what failure modes mean operationally, and how the V2-α wire format interacts with downstream consumers.

Welle 9 produces a new master-resident `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` alongside the existing V1-reference `docs/OPERATOR-RUNBOOK.md`. Style modelled on V1 (copy-pastable bash, operator-focused tone, numbered sections, callouts for failure modes); content covers the six operational flows introduced by V2-α + V2-β's projector-state-hash CI gate capability.

**Why this as Welle 9:** 
- Parallel batch 1 (W9 + W10 + W11) are all docs-only; zero code changes, zero CI-conflict risk
- Documents V2-α capabilities already shipped (Welles 1–8); no new engineering work required
- Gate-unblocks V2-β demo + council-review materials
- Validates the V2-α terminology + error-mode landscape with fresh operator perspective

---

## Scope

| In-Scope | Out-of-Scope |
|---|---|
| NEW `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` (~400–600 lines) | V2-β welle code (ArcadeDB, Read-API, MCP V2 tools, event-kind expansion) |
| §1: Atlas Projector deployment (Layer 2 status for v2.0.0-alpha.1 era; in-memory state) | ArcadeDB operational deployment (V2-β welle 16+17 concern) |
| §2: `atlas-signer emit-projector-attestation` CLI one-liner walkthrough | Future Mem0g cache operational docs (V2-β welle 18) |
| §3: Projector-state-hash CI gate invocation (library API + future CLI) | Agent Passports read-access gating (V2-γ welle) |
| §4: Failure-mode reference (Mismatch, AttestationParseFailed, UnsupportedEventKind) | Hermes-skill v1 operational docs (V2-γ) |
| §5: `@atlas-trust/verify-wasm@2.0.0-alpha.1` consumer integration (npm install + verifier invocation) | Cedar policy at write-time (V2-δ) |
| §6: Sigstore Rekor anchor flow (reference existing V1 runbook section; unchanged) | Post-quantum hybrid Ed25519+ML-DSA-65 signing (V2-δ) |
| §7: Wire-format compatibility callout (V1.0 verifiers reject V2-α events; what to communicate downstream) | |
| §8: Pre-counsel-review disclaimer (same wording pattern as V2-α release-notes) | |
| Forbidden-file compliance: NO edits to CHANGELOG.md, V2-MASTER-PLAN.md, decisions.md, SEMVER-AUDIT, handoff doc | |

**Hard rule:** This welle ONLY creates `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md`. Parent agent consolidates shared-file edits (CHANGELOG, status table, etc.) post-batch.

---

## Decisions (final, pending parent confirmation)

- **Style inheritance from V1 OPERATOR-RUNBOOK.md:** Copy the V1 tone (operator-focused, not engineer-focused), bash code-blocks with copy-pastable invocations, numbered section headers (`## §1`, `## §2`, etc.), and cautionary callouts (⚠️, ✓, etc.). Do NOT edit the V1 runbook itself; create a parallel V2-α-specific doc.
- **Scope of "deployment":** For v2.0.0-alpha.1 (May 2026), Layer 2 is in-memory-only. The runbook documents projector invocation at the CLI level (atlas-signer + library API); it does NOT document ArcadeDB deployment (that's V2-β welle 16+17). If an operator wants persistent Layer-2 state, they stay on in-memory until V2-β.
- **Failure-mode reference:** Document four outcomes from the gate: `Match`, `Mismatch`, `AttestationParseFailed`, and `UnsupportedEventKind` (the latter deferred per V2-β welle 14 design). Include debugging steps for each.
- **Sigstore Rekor anchor flow:** Reference the EXISTING V1 runbook section (§6 or §9, depending on V1's numbering). V2-α does NOT change the anchor flow; the runbook documents how operators already invoke `atlas-signer anchor` + the new capability that V2-α attestations are ALSO anchor-eligible.
- **Consumer integration:** Show the npm install snippet for `@atlas-trust/verify-wasm@2.0.0-alpha.1` and a minimal code example (offline-WASM verification). Keep this section short; full WASM integration is a separate downstream-consumer runbook.
- **Wire-format compatibility warning:** Explicitly state that V1.0 verifiers reject V2-α events with `author_did` or `payload.type == "projector_run_attestation"`. Include the recommendation: "Communicate to downstream consumers that they MUST upgrade to v2.0.0-alpha.1 or later if they want to consume V2-α events."
- **Pre-counsel-review disclaimer:** Copy the exact wording pattern from `docs/V2-ALPHA-1-RELEASE-NOTES.md` §Pre-counsel-review-disclaimer. Include all 7 counsel-engagement items.
- **Cross-reference V1 sections:** Where V2-α extends (not duplicates) V1 functionality, use hyperlinks back to the V1 runbook (e.g., "See V1 runbook §1 for master-seed gate — unchanged in V2-α").

---

## Files

| Status | Pfad | Inhalt |
|---|---|---|
| NEW | `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` | V2-α operator-facing runbook; 8 sections covering projector deployment, CLI flow, gate invocation, failure modes, consumer integration, Rekor anchoring, wire-format compat, pre-counsel disclaimer. ~450–600 lines. Modelled on V1 runbook style. |
| NEW | `.handoff/v2-beta-welle-9-plan.md` | This plan-doc |

**Total estimated diff:** 450–600 lines (documentation only).

---

## Implementation steps (TDD order — applicable to docs)

1. Read V1 `OPERATOR-RUNBOOK.md` (style reference; master-seed gate, HSM wave-2/wave-3 sections, witness commissioning)
2. Read `docs/V2-ALPHA-1-RELEASE-NOTES.md` (extract terminology, wire-format compat details, pre-counsel wording)
3. Read `.handoff/v2-alpha-welle-7-plan.md` (atlas-signer CLI surface + expected output)
4. Draft `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` structure (8 sections, copy-pastable bash blocks, callouts)
5. Write each section:
   - §1: Atlas Projector deployment (reference in-memory state, flag constraints)
   - §2: `emit-projector-attestation` one-liner (full bash block with --help reference)
   - §3: Projector-state-hash CI gate (library API example + future-CLI note)
   - §4: Failure-mode reference (Mismatch, AttestationParseFailed, UnsupportedEventKind)
   - §5: `@atlas-trust/verify-wasm@2.0.0-alpha.1` consumer integration (npm + code example)
   - §6: Sigstore Rekor anchor flow (reference V1 section; note V2-α attestations are also anchor-eligible)
   - §7: Wire-format compatibility (V1 verifiers reject V2-α; what to communicate downstream)
   - §8: Pre-counsel-review disclaimer (copy wording from V2-α release-notes)
6. Verify all code snippets are syntactically correct (bash, Rust where applicable)
7. Verify all hyperlinks point to actual files/sections (OPERATOR-RUNBOOK.md exists, release-notes exists)
8. Self-review: tone matches V1 runbook (operator-friendly, copy-pastable, cautionary where needed)
9. Dispatch parallel `code-reviewer` + `security-reviewer` agents
10. Fix CRITICAL/HIGH findings in-commit
11. Single SSH-Ed25519 signed commit on branch `feat/v2-beta/welle-9-operator-runbook`
12. Push branch + open DRAFT PR with base=master

---

## Acceptance criteria

- [ ] `docs/OPERATOR-RUNBOOK-V2-ALPHA-1.md` created (NEW file, ~450–600 lines)
- [ ] All 8 sections present with copy-pastable bash examples where applicable
- [ ] Hyperlinks to V1 runbook + release-notes verified (files exist, sections referenced)
- [ ] Tone matches V1 runbook (operator-focused, cautionary callouts, numbered sections §1–§8)
- [ ] No hardcoded secrets or non-public information in bash examples
- [ ] Pre-counsel-review disclaimer included verbatim (matches release-notes wording)
- [ ] Wire-format compatibility section clearly states V1 verifier rejection + downstream communication recommendation
- [ ] `.handoff/v2-beta-welle-9-plan.md` created (this file)
- [ ] Parallel `code-reviewer` + `security-reviewer` agents dispatched; CRITICAL = 0, HIGH fixed in-commit
- [ ] Single SSH-Ed25519 signed commit on branch `feat/v2-beta/welle-9-operator-runbook`
- [ ] DRAFT PR open with base=master, no merge yet (parent agent decides merge timing)
- [ ] Forbidden-files rule honoured (no touches to CHANGELOG.md, V2-MASTER-PLAN.md, decisions.md, SEMVER-AUDIT-V1.0.md, handoff session doc)

---

## Risks

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Terminology mismatch between release-notes + operator runbook | LOW | MEDIUM (operator confusion) | Copy exact wording from release-notes for gate outcomes + wire-format compat; hyperlink for cross-reference |
| Atlas projector API surface (library calls shown in bash examples) may diverge from shipped Welle 7 | LOW | LOW (docs would be stale post-V2-β; welle 9 is snapshot-in-time) | Verify atlas-signer CLI surface against Welle 7 plan-doc + crate code before writing; library API from Welle 5 release notes |
| Pre-counsel disclaimer wording outdated by the time V2-α-public goes live | LOW | MEDIUM (counsel engagement items list changes) | Copy exact wording from release-notes as of welle-merge time; parent agent updates post-counsel in later welles |
| Operator reads "in-memory state" and mistakes it for "no persistence" | MEDIUM | MEDIUM (deployment misunderstanding) | Clarify: "in-memory during Welle 7–9 window; ArcadeDB integration is V2-β welle 16–17. Use file-backed events.jsonl for persistence." |
| Code examples use outdated flag names (e.g., `--workspace-id` vs `--workspace`) | LOW | MEDIUM (copy-paste failure) | Verify against Welle 7 final merged code + release-notes; update if welle-7 landed with different flags |

---

## Out-of-scope this welle (V2-β + later)

- **V2-β Welle 12–14 candidates:** Read-API endpoints, MCP V2 tools, expanded event-kind support — these wellen extend the operator-facing surface; operator-runbook updates for those are V2-β welle responsibilities
- **V2-β Welle 16–17:** ArcadeDB deployment runbook (persistent Layer-2 storage; operational concerns distinct from in-memory alpha.1)
- **V2-β Welle 18:** Mem0g Layer-3 cache operational docs
- **V2-γ:** Agent Passports + revocation-channel operational docs (DECISION-COUNSEL-1 follow-up)
- **Counsel engagement:** The disclaimer REFERENCES the engagement; Welle 9 documents what counsel items exist but does NOT block on counsel sign-off (that's a Phase-9 gate per orchestration plan)

---

## Reference pointers

| Concept | Source-of-truth |
|---|---|
| V1 Operator Runbook style/tone | `docs/OPERATOR-RUNBOOK.md` |
| V2-α release notes + terminology | `docs/V2-ALPHA-1-RELEASE-NOTES.md` |
| atlas-signer emit-projector-attestation CLI | `.handoff/v2-alpha-welle-7-plan.md` + final merged Welle 7 code |
| Projector-state-hash gate + GateResult | `.handoff/v2-alpha-welle-6-plan.md` + final merged Welle 6 code |
| atlas-projector library API (project_events, verify_attestations_in_trace) | `crates/atlas-projector/src/` (Welles 3+5+6) |
| Wire-format compatibility (V1 verifier rejection) | `docs/V2-ALPHA-1-RELEASE-NOTES.md` §Wire-format compatibility |
| V2-β Orchestration Plan | `docs/V2-BETA-ORCHESTRATION-PLAN.md` |
| V2-β Dependency Graph | `docs/V2-BETA-DEPENDENCY-GRAPH.md` |
| Counsel-track engagement | `docs/V2-MASTER-PLAN.md` §5 + `.handoff/decisions.md` (`DECISION-COUNSEL-1`) |

---

**End of Welle 9 Plan.** Documentation writeup proceeds on branch `feat/v2-beta/welle-9-operator-runbook`. Single SSH-Ed25519 signed commit per Atlas standing protocol. Parent agent dispatches reviewers + decides merge timing post-batch-1 consolidation.
