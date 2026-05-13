# Atlas — Operator Runbook (V2-α, v2.0.0-alpha.1)

This document is the operator's reference for new V2-α capabilities introduced
in v2.0.0-alpha.1 (May 2026). It is **additive to** the existing
[OPERATOR-RUNBOOK.md](OPERATOR-RUNBOOK.md) (V1 ceremonies, master-seed gate,
HSM configuration, witness commissioning). Read this document if you are
invoking the projection-state verification layer or consuming V2-α-aware events.

It is written for operators of an Atlas deployment, not for engineers building
Atlas itself nor for auditors verifying a trace bundle. For those audiences see
[ARCHITECTURE.md](ARCHITECTURE.md) and [SECURITY-NOTES.md](SECURITY-NOTES.md).

**Status:** Engineering-perspective. Any public marketing material derived from
this document MUST be counsel-validated before publication. See §8 below.

---

## § 1. Atlas Projector — Layer 2 state in v2.0.0-alpha.1

Atlas now has a **Layer-2 graph-state verification primitive** that operators
can invoke to detect projection drift cryptographically. The projector reads
signed events from `events.jsonl`, re-projects them into an in-memory graph
state, and compares the resulting state hash against a signed attestation.

In **v2.0.0-alpha.1 (May 2026)**, the projector stores Layer-2 state
**in-memory only**. This means:

- ✓ Projection works end-to-end (Welles 1–8 shipped)
- ✓ You can emit signed attestations and verify them
- ✓ Offline verification via WASM works (see §5)
- ⚠️ State does NOT persist across projector restarts
- ⚠️ No distributed-query backend yet

**Persistence:** Operators who need durable Layer-2 storage should continue
using file-backed `events.jsonl` as the source-of-truth. ArcadeDB-backed
persistent Layer-2 state is a **v2.0.0-beta.1 candidate** (Q3 2026, Phase 6–7
of the V2-β orchestration plan). Until then, Layer 2 is a **derived layer**
computed on-demand from events.

**No-op upgrade:** If you are currently running Atlas v1.0.1 and do NOT want to
use V2-α features, you can upgrade to v2.0.0-alpha.1 and continue producing
V1-shaped events (no `author_did` field, V1-only payload kinds). The v2.0.0-alpha.1
verifier accepts these unchanged. See §7 below for wire-format compatibility.

---

## § 2. Emit a signed projector-run attestation

After an operator has signed a batch of events and written them to
`events.jsonl`, they can invoke `atlas-signer emit-projector-attestation` to
produce a signed attestation asserting "I ran the projector over this event set
and the resulting graph state hash is X". The attestation is a JSON line ready
to append back to `events.jsonl`.

### Usage

```bash
# 1. Ensure you have signed events in a file.
atlas-signer sign \
  --workspace ws-q1-2026 \
  --derive-from-workspace ws-q1-2026 \
  --payload '{"type":"node_create","node":{"id":"alice"}}' \
  >> trace/events.jsonl

# 2. Compute the blake3 hash of the last event (for --head-event-hash).
#    The hash is the "event_hash" field in the JSON line.
tail -1 trace/events.jsonl | jq -r '.event_hash'
# Output: abc123def456...  (64 lowercase hex)

# 3. Emit the projector attestation.
atlas-signer emit-projector-attestation \
  --events-jsonl trace/events.jsonl \
  --workspace-id ws-q1-2026 \
  --derive-from-workspace ws-q1-2026 \
  --head-event-hash abc123def456... \
  >> trace/events.jsonl

# 4. Verify the attestation was appended.
tail -1 trace/events.jsonl | jq '.payload.type'
# Output: "projector_run_attestation"
```

### Flags

- `--events-jsonl <path>` — Path to the signed events file (required).
- `--workspace-id <id>` — Workspace identifier this attestation is for (required).
- `--derive-from-workspace <id>` OR `--kid <kid>` — Standard signer auth (required; see V1 runbook §1 for master-seed gate).
- `--head-event-hash <hex>` — blake3 hash of the last projectable event (64 lowercase hex, required). The attestation asserts that the projector consumed events up to and including this hash.
- `--projector-version <string>` — (Optional, defaults to `atlas-projector/<version>`). Identity string bound into the signed attestation. You may override for custom projector implementations.
- `--ts <iso8601>` — (Optional, defaults to `now`). Timestamp of the attestation event.

### Output format

One JSON line on stdout, suitable for `>> events.jsonl` append:

```json
{
  "event_id": "01H...",
  "parent_hashes": [],
  "ts": "2026-05-13T14:30:00Z",
  "workspace_id": "ws-q1-2026",
  "kid": "atlas-anchor:ws-q1-2026",
  "payload": {
    "type": "projector_run_attestation",
    "projector_version": "atlas-projector/1.0.1-alpha",
    "head_event_hash": "abc123...",
    "graph_state_hash": "def456...",
    "projected_event_count": 42
  },
  "signature": "...",
  "event_hash": "ghi789..."
}
```

The attestation is itself a signed Atlas event. The `payload.graph_state_hash`
is the blake3 hash of the canonical graph state (RFC 8949 §4.2.1 CBOR).

### Environment variables

Like all atlas-signer subcommands, `emit-projector-attestation` respects:

- `ATLAS_DEV_MASTER_SEED` — Set to `1`, `true`, `yes`, or `on` (case-insensitive) in dev/CI (see V1 runbook §1).
- `ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE` — Production HSM configuration (see V1 runbook §2–§3).

### Failure modes

**Missing `--head-event-hash`:**
```
error: the following required arguments were not provided:
  --head-event-hash <hex>
```
Ensure you have computed the blake3 hash of the last event before invoking the
subcommand.

**Malformed `--head-event-hash` (not 64 lowercase hex):**
```
error: head_event_hash must be 64 lowercase hex characters, got "xyz"
```

**Malformed `events.jsonl`:**
```
error: failed to parse events.jsonl: expected JSON line at line 3
```
Verify the file contains only valid JSON lines (one JSON object per line, no
trailing commas, no comments).

**`ATLAS_DEV_MASTER_SEED` not set (dev/CI) or HSM trio misconfigured (production):**
See V1 runbook §1 for master-seed gate troubleshooting.

---

## § 3. Verify attestations in a trace (Projector-state-hash CI gate)

After emitting attestations, operators and CI systems can invoke the
projector-state-hash gate to verify that a trace's attestations match a fresh
re-projection. The gate is the **Layer-2 trust loop**: it cryptographically
confirms that the issuer's projector and a verifier's re-projection agree on
the graph state for a given event set.

### Library API (Rust)

The gate is invoked via the `atlas-projector` library:

```rust
use atlas_projector::{verify_attestations_in_trace, GateResult, GateStatus};
use std::fs;

// Load the signed events.
let events_jsonl = fs::read_to_string("trace/events.jsonl")?;

// Invoke the gate.
let results = verify_attestations_in_trace("ws-q1-2026", &events_jsonl)?;

// Iterate results.
for gate_result in &results {
    match &gate_result.status {
        GateStatus::Match => {
            println!("✓ attestation {} verified: projector state hash matches",
                gate_result.event_id);
        }
        GateStatus::Mismatch => {
            println!("✗ DRIFT on event {}: attested hash {} vs recomputed {}",
                gate_result.event_id,
                gate_result.attested_hash,
                gate_result.recomputed_hash
            );
            // ALERT: projection logic may have diverged. Investigate bytecode
            // and CI pins before accepting this state.
        }
        GateStatus::AttestationParseFailed => {
            println!("✗ malformed attestation {}: could not parse payload",
                gate_result.event_id);
            // Inspect the event's payload JSON; it may be corrupted or from an
            // incompatible projector version.
        }
    }
}

// Return non-zero if ANY result is Mismatch (typical CI gate pattern).
if results.iter().any(|r| r.status == GateStatus::Mismatch) {
    std::process::exit(1);
}
```

### Expected structure of `GateResult`

| Field | Type | Meaning |
|---|---|---|
| `event_id` | `String` | ULID of the attestation event |
| `status` | `GateStatus` | `Match` / `Mismatch` / `AttestationParseFailed` |
| `attested_hash` | `String` (hex) | The `graph_state_hash` in the attestation payload |
| `recomputed_hash` | `String` (hex) | The hash from re-projecting the events up to this attestation |

### Failure modes

**`GateStatus::Mismatch`**

The issuer's projector and the verifier's re-projection computed **different**
graph-state hashes for the same event set. This indicates:

1. **Projector bytecode diverged.** The issuer ran a different projector binary
   than the verifier (e.g., issuer used v1.0.0 but verifier has v1.0.1). Check
   the `projector_version` field in the attestation.

2. **CI byte-determinism pin broke.** The issuer's projector passed V2-α's
   byte-determinism CI gates (7 pins covering CBOR canonicalisation, COSE,
   anchoring, hash chain, pubkey bundle, and graph state). A Mismatch on
   re-projection suggests either:
   - The issuer's deployment is running an unsigned binary (bypassed CI gates)
   - The issuer's dependencies diverged (e.g., different `serde_json` version)

**Remediation:**

1. Compare `projector_version` in the attestation against your own CI build.
2. Inspect `projector_version` format: `atlas-projector/<semver>` indicates an
   upstream Atlas projector; custom format indicates a forked/wrapper
   implementation.
3. If you control both issuer and verifier: ensure both are built from the same
   git commit, with the same `cargo --locked` constraints.
4. If you are a third-party verifier: escalate to the issuer's operator to
   compare CI logs + bytecode + byte-determinism pin results.

**`GateStatus::AttestationParseFailed`**

The attestation event's payload could not be parsed as a valid
`ProjectorRunAttestation`. This indicates malformed or corrupted JSON.

**Remediation:**

1. Inspect the raw event JSON: `jq '.payload' trace/events.jsonl | grep -A 30 projector_run_attestation`
2. Check for missing required fields:
   - `type` must be `"projector_run_attestation"`
   - `projector_version` must be a string
   - `head_event_hash` must be 64 lowercase hex
   - `graph_state_hash` must be 64 lowercase hex
   - `projected_event_count` must be a non-negative integer
3. If the JSON is valid but parsing still fails: it may be from a future V2-β
   or V2-γ projector version with additional fields. V2-α's parser is
   forward-compatible on extra fields; contact the issuer for clarification.

**`UnsupportedEventKind` (V2-β deferral)**

If a trace contains an event kind that `atlas-projector` cannot project (e.g.,
`annotation_add`, `policy_set`, `anchor_created`), the gate returns an error
at re-projection time. **This is not yet supported in v2.0.0-alpha.1.** The
v2.0.0-alpha.1 projector handles: `node_create`, `node_update`, `node_delete`,
`edge_create`, `edge_update`, `edge_delete`, and `projector_run_attestation`.

Expanded event-kind support is a **v2.0.0-beta.1 candidate** (Phase 4, Welle
14 of V2-β orchestration). Until then, traces containing unsupported event kinds
cannot be verified via the gate.

---

## § 4. Consumer integration: `@atlas-trust/verify-wasm@2.0.0-alpha.1`

Third-party verifiers can now use the `@atlas-trust/verify-wasm` npm package to
verify V2-α events offline in WebAssembly. The WASM verifier includes support
for V2-α's `ProjectorRunAttestation` payload validation.

### Installation

```bash
npm install @atlas-trust/verify-wasm@2.0.0-alpha.1
```

### Offline WASM verification example

```javascript
const {
  verifyTrace,
  parseProjectorRunAttestation,
  GateStatus,
} = require('@atlas-trust/verify-wasm');
const fs = require('fs');

// Load a trace bundle: events.jsonl + pubkey-bundle.json
const events = fs.readFileSync('trace/events.jsonl', 'utf8');
const bundle = JSON.parse(fs.readFileSync('trace/pubkey-bundle.json', 'utf8'));

// Verify the trace (V1 verification + V2-α attestation schema validation).
const evidence = verifyTrace(events, bundle, { requireWitness: 0 });

if (evidence.ok) {
  console.log('✓ Trace verified:', evidence.checksOk);
  
  // If you want to also check projector-state-hash attestations:
  const events_json = events.split('\n').map(line => {
    if (!line.trim()) return null;
    return JSON.parse(line);
  }).filter(Boolean);
  
  for (const event of events_json) {
    if (event.payload?.type === 'projector_run_attestation') {
      try {
        const attestation = parseProjectorRunAttestation(event.payload);
        console.log('Attestation event', event.event_id, '→', attestation.graphStateHash);
      } catch (err) {
        console.error('Failed to parse attestation:', err.message);
      }
    }
  }
} else {
  console.error('✗ Trace verification failed:', evidence.error);
}
```

### What the WASM verifier validates

- ✓ CBOR signatures (COSE_Sign1)
- ✓ Ed25519 signature chain
- ✓ blake3 hash chain (events + anchors)
- ✓ Sigstore Rekor anchor data-residency (trust-on-first-use pattern)
- ✓ Witness cosignature validation (if `--require-witness > 0`)
- ✓ **V2-α NEW:** `ProjectorRunAttestation` payload schema + field presence

The verifier does **NOT** re-project the events to compare against the attested
`graph_state_hash`. That comparison is a separate **offline re-projection step**
on the verifier side (see §3 above). The WASM verifier's role is to confirm
that the attestation event itself is well-formed and signed correctly.

### Offline re-projection verification

To verify that the attested state hash matches a fresh re-projection, the
verifier must:

1. Call `verifyTrace` to confirm the attestation event is well-signed.
2. Separately invoke the projector's gate (e.g., via the `atlas-projector`
   Rust library if you control the verifier) to re-project and compare.

This two-step pattern keeps the WASM verifier lightweight (no graph-projection
logic) while enabling full V2-α verification for sufficiently capable hosts.

---

## § 5. Sigstore Rekor anchor flow (V1 ceremony, unchanged in V2-α)

V2-α attestations are themselves signed Atlas events and are therefore eligible
for Sigstore Rekor anchoring, just like any other event. The anchor ceremony is
**unchanged** from V1.

See [OPERATOR-RUNBOOK.md §8](OPERATOR-RUNBOOK.md) (or whichever section covers
Rekor anchoring in your V1 reference) for the full Rekor ceremony. The
v2.0.0-alpha.1 addition is:

```bash
# 1. Emit attestations (see § 2 above).
atlas-signer emit-projector-attestation \
  --events-jsonl trace/events.jsonl \
  --workspace-id ws-q1-2026 \
  --derive-from-workspace ws-q1-2026 \
  --head-event-hash abc123... \
  >> trace/events.jsonl

# 2. Anchor the attestation events alongside regular events.
atlas-signer anchor \
  --workspace ws-q1-2026 \
  --events-jsonl trace/events.jsonl
```

The `anchor` subcommand will find all unanonymized events (including your new
`ProjectorRunAttestation` events) and submit them to Rekor. The returned
`pubkey-bundle.json` includes the Rekor anchors for all submitted events,
including attestations.

**Immutability benefit:** Anchoring the attestation in Rekor means a third-party
verifier can confirm that the projector was invoked at a specific wall-clock time
(per Rekor's append-only log timestamp). This closes the **temporal trust loop**
for projection state verification.

---

## § 6. Wire-format compatibility: V1.0 verifiers vs V2-α events

⚠️ **CRITICAL for downstream consumers.**

### V1.0 verifiers reading V2-α events

The v2.0.0-alpha.1 release introduces two new fields at the event-schema level:

1. **`author_did`** field (Welle 1): Optional agent-identity string (V2-β
   gating feature, deferred to Agent Passports in V2-γ).
2. **`projector_run_attestation` payload kind** (Welle 4): New signed-event type.

**V1.0 verifiers use `#[serde(deny_unknown_fields)]` policy.** This means:

- ✓ V1-shaped events (no `author_did`, V1-only payload kinds like
  `node_create`, `edge_delete`, etc.) → V1.0 verifier accepts them unchanged.
- ✗ V2-α events with `author_did = Some(_)` OR `payload.type ==
  "projector_run_attestation"` → V1.0 verifier **rejects** with
  `unknown_field` error.

### V2-α verifiers reading V1 events

The v2.0.0-alpha.1 verifier is **fully backward-compatible** with V1-shaped
events. No regression:

- ✓ All V1 events deserialise correctly.
- ✓ All V1 verification checks pass.
- ✓ V2-α-only payload kinds are handled correctly.

### Recommendation

**If you are an operator producing V2-α events (with `author_did` or
`projector_run_attestation` payload), you MUST communicate to all downstream
consumers that they MUST upgrade to v2.0.0-alpha.1 or later.** Verifiers on
v1.0.0 or v1.0.1 will reject your events.

This is the **SemVer-major break** committed by v2.0.0-alpha.1. It is
intentional: V2-α's trust property depends on these new fields being present and
bound into the signing input (see `docs/V2-ALPHA-1-RELEASE-NOTES.md` for the
full rationale).

### Migration strategy

1. **No-op migration:** Continue producing V1-shaped events (no `author_did`,
   V1-only payload kinds). v2.0.0-alpha.1 verifiers accept these unchanged.
2. **V2-α adoption:** Opt-in to `--derive-from-workspace` (binds `author_did` to
   signing input) and `emit-projector-attestation` (emits new payload kind).
   Requires downstream consumers to upgrade.
3. **Coordination:** Before flipping any production deployment to opt-in V2-α
   features, notify downstream auditors and verifiers. Provide a grace period
   for them to upgrade their verifiers.

---

## § 7. Pre-counsel-review disclaimer

Per [`docs/V2-MASTER-PLAN.md`](V2-MASTER-PLAN.md) §5 + [`.handoff/decisions.md`](../.handoff/decisions.md)
(`DECISION-COUNSEL-1`): Atlas commits to a €30–80K counsel engagement (6–8 weeks
structured) pre-V2-α-public-materials covering:

1. GDPR Art. 4(1) hash-as-personal-data opinion (Path A redesign vs Path B defence)
2. AILD→PLD reframe + insurance-regulation engagement strategy
3. Art. 43 conformity-assessment-substitution liability disclaimer drafting
4. Schrems II / cross-border SCC + DPA templates
5. Verbatim Art. 12 + Annex IV marketing copy review
6. Witness-federation EU regulatory positioning brief
7. DPIA + FRIA template drafting

**This runbook is engineering-perspective.** Any public marketing material
derived from it MUST be counsel-validated before publication. The technical
claims (cryptographic primitives, byte-determinism, signature binding) are
stable; the regulatory-claim phrasing is the layer subject to counsel review.

---

## § 8. Quick reference: V2-α operator workflows

| Workflow | Command | Notes |
|---|---|---|
| Emit a projector attestation | `atlas-signer emit-projector-attestation --events-jsonl <path> --workspace-id <id> --derive-from-workspace <id> --head-event-hash <hex> >> events.jsonl` | See §2; outputs one JSON line |
| Verify attestations (library API) | `atlas_projector::verify_attestations_in_trace(workspace_id, events_jsonl_str)?` | See §3; returns `Vec<GateResult>` |
| Anchor attestations (unchanged from V1) | `atlas-signer anchor --workspace <id> --events-jsonl <path>` | Attestation events are eligible; see §5 |
| Verify trace offline (WASM) | npm `@atlas-trust/verify-wasm@2.0.0-alpha.1`; call `verifyTrace(events, bundle, opts)` | See §4; validates signatures + attestation schema |
| Re-project for full V2-α verification | Manual invocation of §3 gate AFTER WASM-verifying signatures | Two-step: signature verification (WASM) + state-hash comparison (projector) |

---

**End of Operator Runbook (V2-α, v2.0.0-alpha.1).** This document complements
the existing [OPERATOR-RUNBOOK.md](OPERATOR-RUNBOOK.md) (V1 ceremonies). For
V2-β operator runbook updates (ArcadeDB deployment, Read-API, MCP V2 tools), see
future operator-runbook wellen in the V2-β orchestration plan.
