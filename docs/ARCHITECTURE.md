# Atlas — System Architecture (V1)

This document is the system-design reference for Atlas. It describes the
trust property the system exists to enforce, the data model that carries
it, the components that produce and consume that data, and the explicit
boundaries between V1 (what ships now), V1.5 (Rekor anchoring), and V2
(full COSE_Sign1, Cedar policy enforcement, SPIFFE attestation).

It is written for three audiences:

1. **Engineers** integrating Atlas into an AI-agent runtime or building
   the next layer of the stack.
2. **Security reviewers and auditors** who need to know what guarantees
   they can rely on and where the limits sit.
3. **Procurement and compliance teams** evaluating Atlas against EU AI
   Act Article 12, GAMP 5, ICH E6(R3), DORA, and GDPR Article 22.

Companion documents:

- [SECURITY-NOTES.md](SECURITY-NOTES.md) — defended attack surface,
  per-test mapping.
- [COMPLIANCE-MAPPING.md](COMPLIANCE-MAPPING.md) — clause-by-clause
  regulatory mapping.

---

## 1. The trust property

> *Given the same trace bundle and the same pinned pubkey-bundle, every
> build of `atlas-trust-core` produces bit-identical verification
> output, on every supported platform.*

This is the load-bearing claim. Everything in this document — the data
model, the component boundaries, the licence split — exists to make it
structurally true.

Two corollaries follow directly:

- **Auditor independence.** A regulator who receives an Atlas trace
  bundle can rebuild the verifier from source under Apache-2.0, run it
  on their own machine with no network calls, and reach the same
  ✓ VALID / ✗ INVALID outcome as anyone else. They do not need to buy
  anything from us. This is a hard guarantee, not a marketing claim:
  the Apache-2.0 verifier crates contain every check the SaaS runs.
- **Determinism is the spec.** When the verifier and the signer
  disagree, the spec says the bug is in whichever side drifted from
  the pinned canonical bytes. The byte-pinned goldens
  ([§4.4](#44-determinism-pipeline)) are the executable spec.

---

## 2. Data model

Three load-bearing types. All three are versioned by `schema_version`
or `schema` and are wire-stable per-major-version.

### 2.1 `AtlasEvent`

A single signed write to the knowledge graph. Append-only — events are
never mutated, only superseded by later events that reference them as
parents.

```text
AtlasEvent
├── event_id        ULID, monotonic per workspace
├── event_hash      blake3 hex of canonical signing-input
├── parent_hashes   zero or more event_hashes (DAG, not a chain)
├── payload         application-level JSON (node.create, annotation.add, …)
├── signature       { alg: "EdDSA", kid, sig (base64url-no-pad) }
└── ts              RFC 3339 timestamp
```

The `event_hash` is **not** a hash of the JSON-encoded event — it is a
hash of a deterministic CBOR encoding of `(workspace_id, event_id, ts,
kid, parent_hashes, payload)`. This binding is what makes
cross-workspace replay structurally impossible
([§4.3](#43-canonicalisation)).

### 2.2 `AtlasTrace`

A self-contained bundle of events suitable for handing to a third
party. The auditor needs only the trace bundle, the matching pubkey
bundle, and the verifier binary.

```text
AtlasTrace
├── schema_version       "atlas-trace-v1"
├── generated_at         when this bundle was assembled
├── workspace_id         binds events to one workspace
├── pubkey_bundle_hash   blake3 of the matching PubkeyBundle
├── events               full set of AtlasEvents
├── dag_tips             server's claim of current tips
├── anchors              Rekor inclusion proofs (V1.5+)
├── policies             Cedar policy event_ids in scope (V2+)
└── filters              optional period/system/nodes_subset
```

`dag_tips` is duplicated from what the verifier can recompute — that is
deliberate, so a server that has rewritten history after-the-fact is
caught at the tip-mismatch check rather than slipping through silently.

### 2.3 `PubkeyBundle`

The identity layer. Maps `kid` → Ed25519 pubkey.

```text
PubkeyBundle
├── schema           "atlas-pubkey-bundle-v1"
├── generated_at     when this bundle was assembled
└── keys             { kid: base64url-no-pad pubkey, … }
```

Two properties make this the trust anchor:

1. **Deterministic hash.** `PubkeyBundle::deterministic_hash` produces a
   blake3 over a canonical-JSON serialisation (sorted keys, no
   whitespace). The hash is byte-pinned in
   `bundle_hash_byte_determinism_pin` so a future canonicalisation drift
   trips a test before silently changing semantics.
2. **Trace binding.** Every `AtlasTrace` claims a `pubkey_bundle_hash`.
   The verifier computes the hash of the bundle it was handed and
   refuses to proceed unless they match (constant-time compare). Silent
   bundle rotation is therefore detectable: a trace generated against
   pubkey-set A cannot pass against pubkey-set B even if both contain
   the relevant kid.

---

## 3. Components

```text
┌──────────────────────────────────────────────────────────────────────┐
│                      AI agent (Claude, Cursor, custom)               │
│                  speaks MCP (Model Context Protocol)                 │
└──────────────────────────────────────────────────────────────────────┘
                                 │ stdio
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│  apps/atlas-mcp-server  (TypeScript, Sustainable Use)                │
│    tools: atlas.write_node, atlas.write_annotation,                  │
│           atlas.export_bundle, atlas.workspace_state                 │
│    persistence: data/{workspace}/events.jsonl                        │
└──────────────────────────────────────────────────────────────────────┘
                                 │ spawns
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│  crates/atlas-signer  (Rust binary, Apache-2.0)                      │
│    builds canonical signing-input (CBOR per RFC 8949 §4.2.1)         │
│    signs with Ed25519 (workspace's sealed key)                       │
│    returns AtlasEvent JSON                                           │
└──────────────────────────────────────────────────────────────────────┘
                                 │ events.jsonl
                                 ▼
┌──────────────────────────────────────────────────────────────────────┐
│         Trace bundle export  (atlas.export_bundle MCP tool)          │
│                AtlasTrace JSON + matching PubkeyBundle JSON          │
└──────────────────────────────────────────────────────────────────────┘
                ┌────────────────┴────────────────┐
                ▼                                 ▼
┌────────────────────────────────┐   ┌────────────────────────────────┐
│  apps/atlas-web                │   │  crates/atlas-verify-cli       │
│  (Next.js, Sustainable Use)    │   │  (Rust binary, Apache-2.0)     │
│  loads atlas-verify-wasm       │   │  offline, no network           │
│  runs in customer browser      │   │  ships to regulators           │
│  no server-side verification   │   │                                │
└────────────────────────────────┘   └────────────────────────────────┘
                ▲                                 ▲
                └────────────┬────────────────────┘
                             ▼
┌──────────────────────────────────────────────────────────────────────┐
│  crates/atlas-trust-core  (Rust library, Apache-2.0)                 │
│    The single source of canonicalisation + verification logic.       │
│    Compiled three ways: native rlib, native CLI, WASM.               │
│    Trust property: bit-identical output across all three targets.    │
└──────────────────────────────────────────────────────────────────────┘
```

### 3.1 Crate boundaries and licence rationale

The split is deliberate, not legalistic.

| Component | Path | Licence | Why |
|---|---|---|---|
| `atlas-trust-core` | `crates/atlas-trust-core` | Apache-2.0 | Auditors must be able to rebuild the verifier with no friction. |
| `atlas-verify-cli` | `crates/atlas-verify-cli` | Apache-2.0 | Same. Regulators ship this with their tooling. |
| `atlas-verify-wasm` | `crates/atlas-verify-wasm` | Apache-2.0 | Same. Embedded by `atlas-web` but redistributable. |
| `atlas-signer` | `crates/atlas-signer` | Apache-2.0 | A second-source signer must be possible. |
| `atlas-mcp-server` | `apps/atlas-mcp-server` | Sustainable Use | Productivity surface, not the trust surface. |
| `atlas-web` | `apps/atlas-web` | Sustainable Use | Productivity surface, not the trust surface. |

If we ever hand a third party a trace bundle that *cannot* be
independently verified under Apache-2.0, the licence split has been
broken and the trust property is no longer load-bearing. That is the
boundary.

---

## 4. Determinism pipeline

The single most important subsystem. Three checkpoints lock the
canonicalisation against silent drift.

### 4.1 Signing-input format

CBOR map per RFC 8949 §4.2.1 ("Core Deterministic Encoding"):

```text
{
  "v":         "atlas-trace-v1",
  "workspace": <workspace_id>,
  "id":        <event_id>,
  "ts":        <ts>,
  "kid":       <kid>,
  "parents":   [<parent_hash_1>, …],
  "payload":   <payload-cbor-canonical>
}
```

### 4.2 Sort rule

Map keys are sorted by **encoded-key length first**, then bytewise lex.
Pure lex sort (the V0 mistake) diverges from §4.2.1 once mixed-length
keys appear. The test `rfc_8949_length_first_map_sort` proves this
property holds.

### 4.3 Canonicalisation

Three guard-rails on the canonicaliser:

- **Floats are rejected.** Float encoding is non-deterministic across
  CBOR variants and across float libraries. The bank demo encodes
  `training_loss = 0.0814` as `training_loss_bps = 814`. The test
  `floats_in_payload_are_rejected` enforces this.
- **`MAX_ITEMS_PER_LEVEL = 10_000`** caps `Vec::with_capacity`
  allocation under hostile input. Bounded above any realistic event,
  pathological inputs are rejected at the canonicaliser boundary.
- **`#[serde(deny_unknown_fields)]`** on every wire-format struct.
  Unknown fields fail the parse rather than being silently dropped or
  silently round-tripped.

### 4.4 Determinism pipeline

Two byte-pinned goldens lock the wire-format on both sides of the trust
model:

- `crates/atlas-trust-core/src/cose.rs::signing_input_byte_determinism_pin`
  pins the CBOR bytes of the per-event signing-input for one fixed
  input. Any unintended change to the canonicalisation pipeline —
  including a `ciborium` upgrade that subtly alters encoding — trips
  this test before WASM/native split can reach a customer's browser.
- `crates/atlas-trust-core/src/pubkey_bundle.rs::bundle_hash_byte_determinism_pin`
  pins the blake3 hex of `PubkeyBundle::deterministic_hash` for one
  fixed bundle. The bundle hash is the second load-bearing identity in
  the trust model: drift here is the "silent rotation" threat.

Both pins enforce the same contract: changing them requires bumping
`atlas-trust-core`'s crate version so the `VERIFIER_VERSION` cascade
propagates, and old-format inputs are rejected with a clean schema
error rather than silently misverifying.

---

## 5. Write flow (agent → MCP → storage)

```text
1. Agent calls MCP tool atlas.write_node(payload)
2. MCP server acquires the per-workspace write lock (in-process mutex)
3. MCP server reads current DAG tips for the workspace from events.jsonl
4. MCP server spawns `atlas-signer sign` with:
     --workspace, --event-id (ULID), --ts, --kid (agent SPIFFE-ID),
     --parents=<current tips>, --payload=<JSON>, --secret-stdin
   The secret is written to the child's stdin and is never visible in
   the OS process listing or shell history.
5. atlas-signer:
     a. builds canonical signing-input via build_signing_input(...)
     b. computes event_hash = blake3(signing_input)
     c. signs signing_input with Ed25519
     d. emits AtlasEvent JSON
6. MCP server validates the signer's stdout against AtlasEventSchema
   (Zod runtime check), then appends the event to events.jsonl
7. MCP server releases the per-workspace lock
8. MCP server returns event_hash to agent
```

Three properties this preserves:

- **Single canonicalisation path.** The MCP server in TypeScript owns
  *zero* canonical-bytes formatting — neither for signing-input nor for
  the pubkey-bundle hash. Both are produced by `atlas-signer` (`sign`
  and `bundle-hash` subcommands) which uses the same Rust functions the
  verifier later runs. Drift between TS and Rust is structurally
  impossible because TS never has a code path to drift.
- **Sealed signing key, never on argv.** Secret material is delivered
  to the signer over stdin (`--secret-stdin`). Argv values appear in
  `/proc/<pid>/cmdline`, `ps aux`, and shell history; stdin does not.
  In V1 the key still originates from `keys.ts` for the bank-demo
  story; in V2 the signer runs in a TPM/HSM-backed enclave and accepts
  no secret input at all — the key never leaves the seal.
- **Per-workspace serialisation.** Concurrent tool calls into the same
  workspace queue behind one another, so the read-tips → sign → append
  sequence is atomic from the agent's perspective. Different workspaces
  run concurrently. Multi-process deployments (V2) replace the
  in-process mutex with an external lock service at `withWorkspaceLock`.

---

## 6. Read / export flow (storage → bundle → verifier)

```text
1. Auditor (or web UI) calls MCP tool atlas.export_bundle(workspace, filters?)
2. MCP server:
     a. reads data/{workspace}/events.jsonl
     b. applies optional filters (period, system, nodes_subset)
     c. computes dag_tips from the event set
     d. assembles AtlasTrace JSON
     e. assembles matching PubkeyBundle JSON
3. Auditor downloads both files
4. Auditor runs:
     atlas-verify-cli verify-trace trace.json -k bundle.json
   or opens the standalone HTML verifier with both files
5. Verifier returns ✓ VALID / ✗ INVALID with per-check evidence
```

The verifier never talks to our server. The bundle is a self-contained
unit — that is the promise.

---

## 7. Key management

### 7.1 `kid` semantics

A `kid` is an opaque key-of-id string. V1 supports three shapes by
convention; the verifier treats them all as opaque labels:

- `spiffe://atlas/agent/<name>` — automated agent (Claude, Cursor,
  internal worker)
- `spiffe://atlas/human/<email>` — named human verifier (the named
  verifier requirement of EU AI Act Annex IV §1(e))
- `spiffe://atlas/system/<name>` — system role (anchor-worker,
  rotation-worker)

V2 will validate SPIFFE-ID SVIDs against an in-domain trust bundle. V1
accepts the `kid` as a label and verifies the signature against the
pubkey listed for that `kid` in the bundle.

### 7.2 Bundle rotation

Pubkey rotation issues a new `PubkeyBundle` with a new
`generated_at`, which produces a new `pubkey_bundle_hash`. Existing
traces still verify against historic bundles by hash. The auditor needs
both the trace and the matching bundle — the bundle hash binds them,
the verifier refuses any mismatch.

In V2 each `PubkeyBundle` is itself anchored to Sigstore Rekor, so the
auditor can independently confirm that bundle hash B was published at
time T and not retroactively forged.

### 7.3 Constant-time hash equality

Both `pubkey_bundle_hash` and per-event `event_hash` comparisons go
through `crate::ct::ct_eq_str`, which is `subtle::ConstantTimeEq` on
byte-equal slices. This is theoretically conservative for an offline
verifier, but the property "byte-identical verification regardless of
input shape" is exactly what Atlas claims, and the cost of constant-time
compare is nil. We pay it.

---

## 8. Anchoring (V1.5)

V1 verifier accepts traces with `anchors: []` and rejects traces with
non-empty `anchors`. This is the honesty rule: the verifier does not
yet validate Rekor inclusion proofs against a pinned Rekor public key
+ log root, so claiming `valid: true` on an unverified anchor would be
a green-tick lie. The test
`non_empty_anchor_rejected_until_v1_5` enforces this.

V1.5 ships:

- **Anchor-worker.** Periodically submits the current DAG-tip hash for
  each workspace to Sigstore Rekor as a `intoto` entry.
- **Pinned Rekor pubkey** in the verifier crate.
- **Inclusion-proof verification.** Verifier walks the merkle path,
  recomputes the root, compares to the Rekor signed tree-head.
- **Tip-rotation.** Each new anchor references the previous one,
  forming a hash-chain *of* hash-chains. A server cannot rewrite past
  events without breaking either an event-hash check or an anchor-chain
  check.

---

## 9. Threat model summary

The full enumeration lives in [SECURITY-NOTES.md](SECURITY-NOTES.md).
Headline:

| Adversary intent | Where it dies |
|---|---|
| Cross-workspace replay | `workspace_id` bound into signing-input → hash mismatch |
| Algorithm downgrade (e.g. RS256) | `signature.alg != "EdDSA"` rejected before verify |
| Schema-prefix attack | `schema_version == "atlas-trace-v1"` is `==`, not `starts_with` |
| Tampered payload | recomputed `event_hash` ≠ claimed |
| Forged DAG tip | `compute_tips` ≠ trace.dag_tips |
| Silent bundle rotation | claimed `pubkey_bundle_hash` ≠ recomputed |
| Duplicate event hash | rejected at `check_event_hashes` |
| Unparseable timestamp | rejected before sig-verify |
| Float in payload | rejected at canonicaliser boundary |
| Unverified Rekor anchor | rejected outright in V1 (honesty rule) |
| Bundle-hash format drift | `bundle_hash_byte_determinism_pin` trips |
| Signing-input format drift | `signing_input_byte_determinism_pin` trips |

---

## 10. V1 / V1.5 / V2 boundaries

### V1 — what ships now

- Deterministic CBOR signing-input, byte-pinned
- Ed25519 signatures, EdDSA-only
- DAG of events with parent-hash links
- `pubkey_bundle_hash` binding (constant-time, byte-pinned)
- Workspace-id binding (cross-workspace replay defence)
- Native CLI verifier + WASM in-browser verifier (one crate)
- MCP server with write/export tools
- Bank-persona golden trace, end-to-end ✓ VALID

### V1.5 — Sigstore anchoring

- Anchor-worker submits DAG-tips to Rekor
- Verifier validates Rekor inclusion proofs against pinned pubkey
- `anchors[]` non-empty no longer auto-rejects
- Anchor-chain tip-rotation (hash-chain of hash-chains)

### V2 — full COSE + policy + SPIFFE

- Switch to RFC 9052 COSE_Sign1 with full CTAP2 canonical CBOR
  (current "simplified V1" envelope is the migration target)
- Cedar policy enforcement at write time + at verify time
- SPIFFE SVID validation against in-domain trust bundle
- Bundle-of-bundles: every `PubkeyBundle` is itself anchored to Rekor
- Multi-signer events (m-of-n thresholds for high-stakes writes)

---

## 11. Operational boundaries

What this V1 is **not** suitable for, and where the limits sit:

- **Not a graph database.** Atlas does not query, index, or join nodes.
  It records the *provenance* of every write. A separate graph store
  (FalkorDB or Neo4j in production) consumes the same events and
  exposes a query surface; the trust property travels with the events,
  not with the query layer.
- **Not a policy engine yet.** V1 records `policies[]` as an event-id
  list but does not evaluate Cedar at verify time. V2 ships full policy
  enforcement.
- **Not a Sigstore client yet.** V1.5 ships Rekor anchoring. Until
  then, anchoring is end-to-end-tested out-of-band but not part of the
  verifier's `valid: true` guarantee.
- **Single-tenant key sealing in V1.** Secret material is delivered to
  `atlas-signer` over stdin (`--secret-stdin`). The legacy `--secret-hex`
  argv flag remains for backwards compatibility with the bank-demo
  example only and emits a stderr deprecation warning. Production
  deployment uses a TPM/HSM-backed enclave; both flags are removed at
  the build level.

---

## 12. Reproducibility recipe

To independently verify the V1 trust property end-to-end, on any
supported platform, with no network calls:

```bash
git clone https://github.com/ultranova/atlas
cd atlas

# 1. Build the verifier from source
cargo build --release -p atlas-verify-cli

# 2. Generate the bank-persona golden trace + bundle
cargo run --example seed_bank_demo -p atlas-signer --release

# 3. Verify
./target/release/atlas-verify-cli verify-trace \
  examples/golden-traces/bank-q1-2026.trace.json \
  -k examples/golden-traces/bank-q1-2026.pubkey-bundle.json

# Expected:
#   ✓ VALID — all checks passed
#   ✓ schema-version
#   ✓ pubkey-bundle-hash
#   ✓ event-hashes
#   ✓ event-signatures
#   ✓ parent-links
#   ✓ dag-tips
#   ✓ anchors

# 4. Optional: prove the determinism property across builds
cargo test -p atlas-trust-core --release
# 33 tests pass on Linux, macOS, and Windows-MSVC.
```

If the verifier on your machine produces the same evidence as the
verifier on ours, you have independently confirmed the trust property.
That is the entire pitch.
