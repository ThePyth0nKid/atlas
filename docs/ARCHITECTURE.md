# Atlas — System Architecture (V1.16)

This document is the system-design reference for Atlas. It describes the
trust property the system exists to enforce, the data model that carries
it, the components that produce and consume that data, and the explicit
boundaries between V1 (deterministic signing + verification), V1.5
(offline-complete anchoring via mock-Rekor issuer + pinned log pubkey —
shipped), V1.6 (live Sigstore Rekor v1 submission — shipped), V1.7
(anchor-chain tip-rotation + Sigstore shard roster — shipped), V1.8
(precision-preserving anchor JSON pipeline + Sigstore-deferred
coverage carve-out — shipped), V1.9 (per-tenant Atlas anchoring keys
via HKDF-SHA256 — shipped), V1.10 (master-seed gate inversion + wave-2
PKCS#11 sealed-seed loader — shipped), V1.11 (wave-3 sealed
per-workspace signer via `CKM_EDDSA` — shipped), V1.12
(`ATLAS_PRODUCTION` paranoia layer removal + ARCHITECTURE sweep —
shipped), and V2 (full COSE_Sign1, Cedar policy enforcement, SPIFFE
attestation).

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

V1.5 anchors each workspace's current `pubkey_bundle_hash` (see §8 Anchoring)
so the auditor can independently confirm that bundle hash B was witnessed at
time T against a pinned log pubkey. V1.6 adds the option of live Sigstore
Rekor v1 submission to give production audit trails public visibility while
keeping verification fully offline.

### 7.3 Per-tenant anchoring keys (V1.9 — shipped)

V1.5–V1.8 signed every event with one of three globally-shared
Ed25519 keypairs (agent / human / anchor). A compromise of any of
those three keys forged events for *every* workspace at once. V1.9
removes that single-key blast radius for *workspace* keys: the
issuer derives a per-workspace Ed25519 keypair from a single master
seed via HKDF-SHA256 (RFC 5869), with the workspace_id bound into
the HKDF `info` parameter:

```text
info       = "atlas-anchor-v1:" || workspace_id
key_bytes  = HKDF-SHA256(salt = None, ikm = master_seed, info, len = 32)
signing    = ed25519_dalek::SigningKey::from_bytes(&key_bytes)
```

Each workspace's public key appears in `PubkeyBundle.keys` under a
kid of shape `atlas-anchor:{workspace_id}`. The verifier consumes
the public key from the bundle and never sees the master seed —
re-derivation is an issuer-side capability only. Compromise of one
workspace's signing key does not compromise other workspaces (HKDF
is one-way per-info).

The kid prefix is pinned in `atlas_trust_core::per_tenant::PER_TENANT_KID_PREFIX`
(verifier-side) and mirrored in `apps/atlas-mcp-server/src/lib/keys.ts::PER_TENANT_KID_PREFIX`
(issuer-side). The issuer-side HKDF info-prefix (`atlas-anchor-v1:`)
is intentionally distinct from the verifier-side kid prefix
(`atlas-anchor:`) to keep the cryptographic-domain tag decoupled from
the wire-format identifier.

**Strict mode.** `VerifyOptions::require_per_tenant_keys` (default
false for backwards compatibility) demands every event's `kid` equal
`format!("atlas-anchor:{trace.workspace_id}")`. Mixed legacy +
per-tenant kids are rejected. Lenient mode accepts both. V1.5–V1.8
bundles continue to verify in lenient mode. Strict mode is the V1.9
security boundary.

**Production gate (V1.9 — superseded by V1.10, removed in V1.12).**
V1.9 shipped a *negative* paranoia gate: per-tenant subcommands
refused to run when `ATLAS_PRODUCTION=1` was set. The opt-out
shape was a footgun (forgetting the env var let the
source-committed `DEV_MASTER_SEED` sign production traffic); V1.10
inverted the gate to a positive opt-in
(`ATLAS_DEV_MASTER_SEED=1` required to admit the dev seed) and
shipped the wave-2 PKCS#11 sealed-seed loader. V1.12 removed the
V1.9 paranoia layer entirely — the V1.10 positive opt-in covers
the same security property without the literal-`"1"`-only
recognition footgun, and the wave-2 HSM trio is now the
production audit signal. See §7.4 for the V1.10 master-seed gate
and §7.5 for the V1.11 wave-3 sealed per-workspace signer.

**Signer-internal derivation.** The MCP hot path uses
`atlas-signer sign --derive-from-workspace <ws>`, which derives the
per-tenant secret inside the signer process and signs without ever
emitting it. Bundle assembly uses `atlas-signer derive-pubkey`
(public key only). The TS server never holds the per-tenant signing
key — there is no path through which a per-tenant secret enters Node
heap during normal operation. The `derive-key` subcommand (which
emits the secret) is reserved for ceremonies and gated by the same
production gate.

**Bundle rotation.** `atlas-signer rotate-pubkey-bundle --workspace <ws>`
adds the per-tenant kid + pubkey to a `PubkeyBundle` read from stdin
and emits the updated bundle on stdout. Idempotent: a re-run on an
already-rotated bundle returns the bundle unchanged (existing pubkey
is asserted to match the derivation; mismatch refuses to overwrite).
The legacy SPIFFE kids are preserved so old traces continue to
verify in lenient mode. See `docs/OPERATOR-RUNBOOK.md` for the
ceremony, including atomic-replace and inter-operator concurrency
responsibilities (which sit on the operator side — the signer reads
stdin and writes stdout).

### 7.4 Master-seed gate + wave-2 sealed-seed loader (V1.10 — shipped, V1.12-simplified)

V1.10 ships in two coordinated waves that together close the
V1.9 master-seed-as-source-committed-constant residual.

**Wave 1 — gate inversion (positive opt-in).** The V1.9
negative-opt-out gate (refuse when `ATLAS_PRODUCTION=1`) is
replaced by a positive-opt-in gate
(`keys::master_seed_gate`): per-tenant subcommands refuse to start
unless `ATLAS_DEV_MASTER_SEED` is a recognised truthy value (`1`,
`true`, `yes`, `on`, ASCII case-insensitive, surrounding
whitespace tolerated). A deployment that forgets the env var
fails closed with an actionable error rather than silently
signing with the source-committed dev seed.

**Wave 2 — PKCS#11 sealed-seed loader.** When the HSM env trio
(`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`)
is set, `keys::master_seed_loader` opens the PKCS#11 module
**first** and routes per-tenant derives through
[`Pkcs11MasterSeedHkdf`](../crates/atlas-signer/src/hsm/pkcs11.rs)
(gated behind the `hsm` Cargo feature). HKDF-SHA256 runs **inside**
the HSM token via `CKM_HKDF_DERIVE`; the master seed bytes never
enter Atlas address space. HSM open / derive failure is **fatal**
— there is no silent fallback to the dev seed. The wave-1 gate is
not consulted in this mode.

**MasterSeedHkdf trait.** The wave-1 dev impl
(`DevMasterSeedHkdf`) and wave-2 HSM impl
(`Pkcs11MasterSeedHkdf`) both implement
[`MasterSeedHkdf`](../crates/atlas-signer/src/keys.rs); per-tenant
subcommands derive via
`derive_workspace_signing_key_via<H: MasterSeedHkdf + ?Sized>` so
the call sites are backend-agnostic. The buffer-out shape
(`derive_for(info, out: &mut [u8; 32])`) is zeroize-friendly:
sealed implementations wipe scratch space on every exit path.

**V1.12 simplification.** V1.9–V1.11 layered the V1.9
`ATLAS_PRODUCTION=1` paranoia gate ahead of the wave-1 opt-in for
defence-in-depth. V1.12 removed it: (a) the literal-`"1"`-only
recognition was a documented operator footgun, (b) the wave-1
positive opt-in covers the same security property without the
footgun, and (c) the wave-2 HSM trio is now the production audit
signal — `env | grep ATLAS_` shows whether a deployment is
sealed-seed (trio set, opt-in unset) or dev (trio unset, opt-in
set). `ATLAS_PRODUCTION` is silently ignored from V1.12 onwards.

See `docs/OPERATOR-RUNBOOK.md` §1 for the gate truth table and §2
for the wave-2 HSM master-seed import ceremony.

### 7.5 Wave-3 sealed per-workspace signer (V1.11 — shipped)

V1.10 wave 2 sealed the *master seed* inside the HSM but the
per-tenant Ed25519 *scalar* still transited Atlas address space
in a `Zeroizing` buffer between the HSM-side HKDF derive and the
dalek-side signature. V1.11 wave-3 closes that residual: when the
operator opts in via `ATLAS_HSM_WORKSPACE_SIGNER=1` AND the HSM
trio is set, per-workspace Ed25519 keypairs are generated **on
the device** with `Extractable=false` and signed with `CKM_EDDSA`.
The scalar never enters Atlas address space; only the
ready-formed 64-byte signature crosses back into the signer
process.

**WorkspaceSigner trait.** The wave-1/wave-2 dev path
(`DevWorkspaceSigner`, wrapping a `Box<dyn MasterSeedHkdf>`) and
wave-3 HSM path
([`Pkcs11WorkspaceSigner`](../crates/atlas-signer/src/hsm/pkcs11_workspace.rs))
both implement
[`WorkspaceSigner`](../crates/atlas-signer/src/workspace_signer.rs).
Per-tenant subcommands route through
`per_tenant_identity_via_signer` and `WorkspaceSigner::sign` so
the binary's call sites are backend-agnostic across all three
layers (dev / wave-2 / wave-3).

**Three-layer dispatcher.** `workspace_signer_loader` selects the
backend:
1. `ATLAS_HSM_WORKSPACE_SIGNER=1` + HSM trio → wave-3
   `Pkcs11WorkspaceSigner` (per-workspace keys live in the HSM).
   Trio missing under this opt-in is a fail-closed refusal —
   wave-3 has no dev fallback.
2. HSM trio only → wave-2 `DevWorkspaceSigner` over
   `Pkcs11MasterSeedHkdf` (master seed sealed; per-tenant
   derive in-process).
3. Neither → wave-1 dev path, gated on `ATLAS_DEV_MASTER_SEED=1`.

**Multi-token redundancy trade-off.** Wave-2 supported "import
the same sealed master seed into multiple tokens" for
cross-token redundancy. Wave-3 generates each keypair with the
device's own entropy, so two tokens cannot agree on a
per-workspace pubkey without exporting (which `Extractable=false`
forbids). Deployments requiring redundancy must either stay on
wave-2 OR accept a fresh provision (= every per-tenant pubkey
rotates) as the recovery path on token loss. See
`docs/OPERATOR-RUNBOOK.md` §wave-3 for the migration semantics.

### 7.6 HSM-backed witness backend (V1.14 — shipped)

V1.13 wave-C wired the witness signing path against a 32-byte
file-backed seed (`atlas-witness sign-chain-head --secret-file
<path>`); the scalar transited the witness binary's address space
in a `Zeroizing<[u8; 32]>` buffer for the lifetime of one sign call.
V1.14 Scope I closes that residual exposure for HSM-backed
deployments: when the witness HSM trio is set
(`ATLAS_WITNESS_HSM_PKCS11_LIB`, `ATLAS_WITNESS_HSM_SLOT`,
`ATLAS_WITNESS_HSM_PIN_FILE`) AND the operator passes `--hsm` at
the CLI, signing routes through `CKM_EDDSA(Ed25519)` against a
keypair generated **on the device** with `Sensitive=true`,
`Extractable=false`, `Derive=false`. The scalar never enters Atlas
address space; only the ready-formed 64-byte signature crosses back
into the witness process.

**Witness trait.** Both backends implement the dyn-safe
[`Witness`](../crates/atlas-witness/src/lib.rs) trait
(`Result<_, String>` boundary, `Send + Sync`, no `async fn`).
The file-backed [`Ed25519Witness`](../crates/atlas-witness/src/ed25519_witness.rs)
holds an in-memory `SigningKey`; the HSM-backed
[`Pkcs11Witness`](../crates/atlas-witness/src/hsm/pkcs11.rs) holds
an authenticated session + private-key handle. The CLI dispatcher
in [`atlas-witness::main`](../crates/atlas-witness/src/main.rs)
selects backends via clap's `ArgGroup` mutual exclusion: `--hsm`
and `--secret-file` cannot co-occur, and exactly one is required.

**Trust-domain separation.** The witness binary uses distinct
env-var and label prefixes from atlas-signer:

| Surface | atlas-signer | atlas-witness |
|---|---|---|
| Env-var prefix | `ATLAS_HSM_*` | `ATLAS_WITNESS_HSM_*` |
| On-token label prefix | `atlas-workspace-key-v1:` | `atlas-witness-key-v1:` |
| Auto-generates keypairs? | yes (lazy on first derive) | **no** (operator-driven) |

The "no auto-generation" choice is the load-bearing trust property
for the witness: a binary that auto-generated keys could be made
to sign on a fresh, unrostered keypair and silently bypass
`ATLAS_WITNESS_V1_ROSTER`. `Pkcs11Witness::open` only **resolves**
an existing keypair by `(CLASS=PRIVATE_KEY, KEY_TYPE=EC_EDWARDS,
LABEL=atlas-witness-key-v1:<kid>)`; missing keypair fails with a
`SigningFailed:` error pointing at OPERATOR-RUNBOOK §11. Generation
runs as an explicit operator action via `pkcs11-tool --keypairgen`.

**Backend mutual exclusion at parse time.** clap's `ArgGroup`
declares `--hsm` and `--secret-file` as a required, single-pick
pair. A user invocation that passes neither, both, or any other
combination fails at argument parse — before any IO or HSM access.
This is structural rather than runtime enforcement: a future code
edit cannot accidentally allow both backends to fire and produce
divergent signatures from the same kid.

**Scope and limits.** V1.14 Scope I covers the witness signing
substrate. It does NOT change the wire format (`WitnessSig` is
unchanged), the verifier-side roster mechanism (`§10` continues
unchanged for both file-backed and HSM-backed witnesses), or the
`require_witness_threshold` strict-mode contract (V1.13 wave-C-2).
A V1.14 deployment can mix file-backed and HSM-backed witnesses in
one quorum — the verifier sees only the resulting signatures and
cannot tell them apart byte-for-byte. See
`docs/OPERATOR-RUNBOOK.md` §11 for the commissioning ceremony and
[SECURITY-NOTES.md](SECURITY-NOTES.md) §scope-i for the threat model.

### 7.7 Auditor wire-surface (V1.14 — shipped: Scope J)

V1.13 wave-C-2 surfaced witness diagnostics through a single human-
readable `evidence` row's `detail` string, with per-failure entries
joined by `; `. Auditor tooling that wanted to classify a failure had
to string-match the wording — fragile by construction, since a
verifier-side wording fix would break the auditor's classifier with
no compile-time signal. V1.14 Scope J replaces that with a structured
JSON wire surface.

**`WitnessFailureReason` + `WitnessFailureWire`.** Two new public
types in `atlas-trust-core`:

- `WitnessFailureReason` — `#[non_exhaustive]` enum with kebab-case
  serde encoding (`kid-not-in-roster`, `duplicate-kid`,
  `cross-batch-duplicate-kid`, `invalid-signature-format`,
  `invalid-signature-length`, `oversize-kid`,
  `chain-head-decode-failed`, `ed25519-verify-failed`, `other`).
  Auditor tooling switches on this for classification.
- `WitnessFailureWire` — `{ witness_kid: String,
  batch_index: Option<u64>, reason_code: WitnessFailureReason,
  message: String }`. `#[serde(deny_unknown_fields)]` so a corrupted
  payload fails closed at parse.

**At-source classification.** A new private
`verify_witness_against_roster_categorized` returns
`Result<(), (TrustError, WitnessFailureReason)>`, naming the failure
reason at the point of rejection. The public
`verify_witness_against_roster` continues returning `TrustResult<()>`
unchanged — Scope J is additive.

**`VerifyOutcome.witness_failures`.** A new
`Vec<WitnessFailureWire>` field on `VerifyOutcome`, populated in
`verify_trace_with` by mapping the categorised
`witness_aggregate.failures` through `WitnessFailureWire::from`.
`#[serde(default)]` so pre-J payloads parse into a post-J struct
with `witness_failures: vec![]`.

**Wire-side input sanitisation (SEC).** The per-batch verifier runs
`sanitize_kid_for_diagnostic` on the wire-supplied `witness_kid`
*before* constructing any `WitnessFailure` record (including the
duplicate-kid pre-pass branch). Defends the lenient evidence row's
`rendered.join("; ")` aggregation from a multi-MB blob amplification
attack: a malicious issuer presenting an oversize kid would
otherwise have ballooned the diagnostic output through that join.

**End-to-end pin.** `atlas-verify-cli verify-trace --output json`
emits the `witness_failures` array as part of `VerifyOutcome`
serialisation. Exercised by
`crates/atlas-verify-cli/tests/witness_failures_json.rs` (Rust
integration) and `apps/atlas-mcp-server/scripts/smoke.ts` step 8
(TS-side `JSON.parse` round-trip). A regression that omits the
field, renames it, or emits `null` instead of `[]` trips one or
both lanes.

**Trust property unchanged.** Scope J does not change the
verification verdict for any input that V1.13 wave-C-2 already
accepted or rejected. `valid` and `errors` remain the load-bearing
trust signals; `witness_failures` is purely diagnostic. See
[SECURITY-NOTES.md](SECURITY-NOTES.md) §scope-j for the threat
model and consumer-side residual risks.

### 7.8 Constant-time hash equality

Both `pubkey_bundle_hash` and per-event `event_hash` comparisons go
through `crate::ct::ct_eq_str`, which is `subtle::ConstantTimeEq` on
byte-equal slices. This is theoretically conservative for an offline
verifier, but the property "byte-identical verification regardless of
input shape" is exactly what Atlas claims, and the cost of constant-time
compare is nil. We pay it.

---

## 8. Anchoring

### 8.1 Mock-Rekor (V1.5 — shipped)

V1.5 ships an offline-complete anchoring path: a deterministic mock-Rekor
issuer in `atlas-signer`, RFC 6962-style Merkle inclusion proofs, and a
verifier that validates each proof + signed checkpoint against a pinned
log public key. The "mock" qualifier means *no live network call to a
public Rekor instance* — every other property (real Ed25519 over canonical
checkpoint bytes, real audit-path with `leaf:`/`node:` domain separation,
real anti-tamper) is identical to a production transparency log.

#### What gets anchored

Two object kinds, in one batch sharing a single Merkle tree and
checkpoint:

- **`bundle_hash`** — the trace's `pubkey_bundle_hash`. Defends against
  post-hoc bundle-swap attacks that would re-validate forged signatures.
  An auditor with the anchor knows "this exact key roster was the one in
  use by time T".
- **`dag_tip`** — every current `event_hash` in `dag_tips`. Defends
  against tail truncation or fork: an auditor knows "this trace state
  existed by time T".

#### Verification

Lenient by default — empty `anchors[]` passes, mirroring the V1 contract
that "no claim is fine, but a false claim is not". Strict mode
(`VerifyOptions::require_anchors`) demands at least one anchor and that
every `dag_tip` be covered by a `dag_tip` anchor entry. Tampering with
either the anchored hash or the proof flips the per-entry verdict to
fail and the whole verifier outcome to ✗ INVALID.

### 8.2 Live Sigstore submission (V1.6 — shipped)

V1.6 ships live Sigstore Rekor v1 submission: `atlas-signer anchor --rekor-url https://rekor.sigstore.dev`
POSTs each batch to the public log without touching the verifier or trace
schema. The verifier dispatches by `log_id` (BTreeMap keyed by SHA-256
of the log's DER SPKI pubkey), so both mock-Rekor and Sigstore anchors
flow through the same trust path.

#### Live Rekor submission

- **CLI surface:** `atlas-signer anchor --rekor-url <url>` (also via
  `ATLAS_REKOR_URL` env var in the MCP server; precedence: field > env >
  mock)
- **Wire format:** hashedrekord/v0.0.1 per
  <https://github.com/sigstore/rekor/blob/main/pkg/types/hashedrekord/v0.0.1>
- **Hash binding:** SHA-256 RFC 6962 + ECDSA P-256 over C2SP signed-note
  checkpoint (three-line RFC 9162 format with origin label
  `"rekor.sigstore.dev"` and the active Trillian tree-ID
  `1_193_050_959_916_656_506`)
- **Atlas anchoring key:** ECDSA P-256 derived from deterministic seed
  `b"atlas-rekor-anchor-v1-dev-seed"` (dev only; V1.7 will seal in
  TPM/HSM). Rekor admits the entry by verifying the signature; the
  verifier does NOT pin the Atlas key — trust property depends only on
  the Sigstore log's ECDSA P-256 pubkey.
- **Atlas anchoring key pin:** `crates/atlas-signer/src/anchor.rs::atlas_anchor_pubkey_pem_is_pinned`
  asserts the seed-derived pubkey stays in sync with the embedded PEM —
  touching the seed without updating the test fails CI.
- **Cross-format derivation:** `sigstore_anchored_hash_for(kind, blake3_hex)`
  in `crates/atlas-trust-core/src/anchor.rs` derives the SHA-256 hash
  that Rekor's hashedrekord entry binds. Domain-separated by kind
  (`atlas-dag-tip-v1:` / `atlas-bundle-hash-v1:`) so issuer and verifier
  produce identical SHA-256 from an Atlas blake3. Pinned in a test so
  the derivation logic cannot silently drift between signer and verifier.
- **Verifier dispatch:** `default_trusted_logs()` is a BTreeMap of
  `log_id` → `TrustedLog { pubkey, format }`. The production Sigstore
  Rekor v1 pubkey is one of the pinned entries. An inbound anchor whose
  `log_id` is not in the map is rejected before proof work.
- **Sigstore Rekor v1 pinning:** `SIGSTORE_REKOR_V1_PEM` is the production
  log's ECDSA P-256 SPKI public key (retrieved from
  `https://rekor.sigstore.dev/api/v1/log/publicKey` and checked in).
  `SIGSTORE_REKOR_V1_LOG_ID` is SHA-256 of the DER bytes. `SIGSTORE_REKOR_V1_ACTIVE_TREE_ID`
  pins the active Trillian tree-ID (`1_193_050_959_916_656_506`); an
  anchor with a mismatched tree-ID is rejected before signature verify.
- **API version pin:** `apiVersion == "0.0.1"` enforced in
  `entry_body_binds_anchored_hash` — any other value is rejected.
  Enforced in both issuer (generates 0.0.1 requests) and verifier
  (rejects other versions).
- **keyID rotation policy:** The C2SP signed-note `keyID` is the first
  4 bytes of SHA-256(DER SPKI) — for Sigstore Rekor v1 this is
  `c0d23d6a`. Multiple signature lines in a checkpoint (one per keyID
  rotation) are handled by iterating: verify each line's signature
  against the claimed keyID's pubkey; if one matches, accept the
  checkpoint; if none match, reject. Mismatch on a single line is
  `continue`, not an error. Tested in
  `crates/atlas-trust-core/tests/sigstore_golden.rs`.
- **Network transport:** HTTP client validation lives in
  `crates/atlas-signer/src/rekor_client.rs::RekorClient::new`. HTTPS is
  required for non-loopback hosts (`https://` enforced); plaintext `http://`
  is gated to localhost (`localhost`, `127.0.0.1`, `[::1]`) for wiremock
  testing. An operator typo cannot silently submit anchoring signatures
  over an unencrypted wire.

#### Verification unchanged

The verifier path is bit-identical for both V1.5 mock and V1.6 Sigstore:
no network call at verify time, no runtime log-rotation polling. The
verifier never talks to our server or to Sigstore — it recomputes the
RFC 6962 inclusion path from the entry body, re-derives the checkpoint
from the anchor entry's tree_size/root_hash, and verifies the ECDSA or
Ed25519 checkpoint signature against a pinned log pubkey. Full offline
verification on any platform.

### 8.3 Anchor-chain tip-rotation (V1.7 — shipped)

V1.7 ships anchor-chain tip-rotation: each new anchor batch is cross-linked
to the previous batch via a hash-chain head, so a server cannot silently
rewrite past anchored state without breaking the chain.

#### What gets chained

An anchor batch (one or more `AnchorEntry` objects, e.g. from a single
`atlas_anchor_bundle` call) is wrapped in an `AnchorBatch` that includes:

- **`batch_index`** — sequence number (0, 1, 2, ...), enforces monotonic growth.
- **`integrated_time`** — Unix seconds when Rekor accepted the batch (V1.5 mock or
  V1.6 Sigstore).
- **`entries`** — the `AnchorEntry` array itself (for offline recomputation).
- **`previous_head`** — 64-character hex string: the chain head of the
  preceding batch. For the first batch, `previous_head = "00..."` (64 zeros).

The chain head is computed over each batch:

```text
chain_head_n = blake3(
    b"atlas-anchor-chain-v1:" ||
    canonical_bytes_of(AnchorBatch[n]) ||
    previous_head_{n-1}
)
```

The domain prefix `atlas-anchor-chain-v1:` separates this hash from all other
blake3 uses in Atlas (event hashes, bundle hashes, anchor derivation hashes).

All `AnchorBatch` objects are collected in an `AnchorChain` with:

- **`history`** — ordered list of all `AnchorBatch` objects, from batch 0 to present.
- **`head`** — the blake3 of the last batch (fail-fast convenience, always recomputed
  during verification).

The chain is optional on `AtlasTrace` — old V1.5/V1.6 bundles lack it — but when
present, the verifier walks every batch and validates:

1. Each `batch_index == i` (no gaps, no reorder).
2. Each `chain_head_i` recomputes from `(entries_i, previous_head_i)`.
3. Each `history[i+1].previous_head == recomputed_chain_head_i` (continuity).
4. Final head matches the claimed `trace.anchor_chain.head`.

#### Storage and issuer flow

- **`data/{workspace}/anchors.json`** — unchanged, holds the latest batch snapshot
  for V1.5/V1.6 compatibility.
- **`data/{workspace}/anchor-chain.jsonl`** — NEW, append-only log of every batch.
  Each line is one `AnchorBatch` serialized as JSON. Atomic append via tmp-and-rename
  + fsync.

The issuer (`atlas-signer anchor --chain-path <file>`) reads the JSONL to find the
next `batch_index` and the current `previous_head`, then appends the new batch
atomically. Single writer enforcement: the MCP server passes the workspace path to
the signer; the signer does the read-modify-append.

#### Verification and lenient mode

- `VerifyOptions::require_anchor_chain` (default `false`) — lenient by default.
  Old bundles without a chain pass; `require_anchor_chain = true` strict mode
  demands a present, valid, non-empty chain.
- If chain is missing: pass under lenient mode, fail under strict mode.
- If chain is present: walk history and validate as above. Any mismatch (gap,
  reorder, head mismatch, previous_head break) → ✗ INVALID.

#### Cross-check: anchor-chain-coverage

The verifier also runs a consistency check (`anchor_chain_coverage` in `verify.rs`):
every entry in `trace.anchors` (the current snapshot) must appear byte-identical in
some batch of `trace.anchor_chain.history`. This catches mixed-mode workspaces
(operator switches from mock to Sigstore without preserving chain continuity) because
the Sigstore entries are absent from the mock-only history — the check fires loudly.

#### Sigstore path: precision-preserving JSON (V1.8 — shipped)

V1.7 gated chain extension to the mock-issuer path because Sigstore Rekor v1
`tree_id` values (e.g. `1_193_050_959_916_656_506`) exceed
`Number.MAX_SAFE_INTEGER` (~2^53) in JavaScript: a `JSON.parse` round-trip in
the MCP server silently rewrote the low digits, so the chain head an offline
auditor recomputed would diverge from the head the issuer emitted.

V1.8 routes every signer-stdout and on-disk anchor JSON boundary in the MCP
server through `lossless-json` (`apps/atlas-mcp-server/src/lib/anchor-json.ts`).
The custom number parser keeps integer literals in safe range as native
`number` (so existing `z.number().int()` schemas are unchanged), and wraps
oversized integers, fractionals, and scientific-notation literals in a
`LosslessNumber` whose `.value` preserves the exact source string.
`stringifyAnchorJson` re-emits those wrappers as their original digits.

The Zod boundary (`AnchorEntrySchema.tree_id`) accepts the union
`z.number().int() | LosslessIntegerSchema`, where the second arm enforces a
non-negative integer-literal regex bounded at 19 decimal digits (i64 magnitude
ceiling). A signer that ever emits scientific notation (`1.193e18`) or a
malformed wrapper fails at this boundary with a descriptive Zod error rather
than silently passing as a Number.isInteger-valued float.

The V1.7 Sigstore-path gate in `apps/atlas-mcp-server/src/tools/anchor-bundle.ts`
is removed: chain extension now applies to both paths. `anchor-chain.jsonl`
remains write-once via the Rust signer's atomic append.

#### V1.7 Sigstore-deferred coverage carve-out (V1.8 — shipped)

The verifier's `anchor_chain_coverage` (`crates/atlas-trust-core/src/verify.rs`)
classifies each entry in `trace.anchors` into one of three buckets:

- **Covered** — entry is byte-identical to a row in `trace.anchor_chain.history`.
- **Sigstore-deferred** — entry's `log_id == SIGSTORE_REKOR_V1_LOG_ID` but is
  absent from chain history. Accepted on the basis that Sigstore Rekor v1's
  own publicly-witnessed transparency log provides equivalent monotonicity for
  that entry, and the per-entry verification (`verify_anchors`, runs unconditionally
  before coverage) still validates the full inclusion proof and checkpoint
  signature against the pinned PEM. Forging a Sigstore `log_id` is SHA-256
  preimage resistance; forging a valid checkpoint requires the Sigstore Rekor
  private key. The carve-out does not weaken the per-entry trust anchor.
- **Uncovered** — any other entry absent from chain history. Coverage fails.

The carve-out exists so that V1.7-issued bundles (Sigstore anchors not in
chain) keep verifying after V1.8, and so that mixed-mode batches (mock entry
+ Sigstore entry in the same batch, recorded in chain) still chain-verify
properly.

#### Chain rotation ceremony

Operators can rotate the chain (produce a new genesis batch that bridges continuity
to the old chain) via `atlas-signer rotate-chain --confirm <workspace>`. The new
genesis batch's `previous_head` equals the old chain's final head, so an auditor
with both the old and new traces can verify continuity. The old chain file becomes
read-only history.

### 8.4 Sigstore Rekor v1 shard roster (V1.7 — shipped)

V1.7 expands the Sigstore trust model from a single active tree-ID to a roster
of three trusted shards (active + 2 historical), all signed by the same pinned
public key.

#### Current constraint (V1.6)

V1.6 pins the active Sigstore Rekor v1 tree-ID: `SIGSTORE_REKOR_V1_ACTIVE_TREE_ID =
1_193_050_959_916_656_506`. An anchor with any other tree-ID is rejected at
verification time, before proof work.

#### V1.7 roster expansion

Sigstore maintains historical shards signed by the same pubkey but with different
tree IDs. After a shard rotation event, V1.6 anchors would instantly become unverifiable
against the new active shard.

V1.7 introduces `SIGSTORE_REKOR_V1_TREE_IDS: &[i64]`, a 3-element list:

- `1_193_050_959_916_656_506` — active shard (current production).
- `3_904_496_407_287_907_110` — historical shard.
- `2_605_736_670_972_794_746` — historical shard.

The verifier's `is_known_sigstore_rekor_v1_tree_id(tree_id)` function replaces the
strict equality check. An inbound anchor passes the tree-ID gate if its `tree_id` is
a member of the roster.

#### Same-key trust property

The pinned ECDSA P-256 pubkey (`SIGSTORE_REKOR_V1_PEM`) remains unchanged across
shards. Signature verification still depends only on this single key. An attacker
cannot exploit different keys per shard because there is only one pinned key.

The C2SP signed-note origin line embeds the tree-ID (rekor.sigstore.dev's origin
line is reconstructed from caller-supplied `entry.tree_id`), so cross-shard replay
is impossible: verifying a checkpoint signed for tree_id A against a submitted entry
claiming tree_id B would fail at signature verify because the reconstructed origin
differs.

#### Roster update is a source change

Adding new tree-IDs to the roster requires a crate version bump. Silent acceptance of
unknown tree-IDs is forbidden. If a future shard rotation introduces a new tree-ID,
that is a new source change requiring a published update to Atlas.

#### Issuer asymmetry (intentional)

The issuer (`atlas-signer anchor --rekor-url`) still posts only to the active shard.
This asymmetry is deliberate: verifier accepts historical shards (backwards
compatibility), issuer produces current (forward progress). The roster protects
against shard rotation; operator who upgrades gets free backwards compatibility.

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
| Tampered anchor proof or anchored hash | per-entry inclusion + checkpoint-sig fails → ✗ INVALID |
| Wrong/unknown log_id | not in pinned-trust set → anchor rejected |
| Bundle-hash format drift | `bundle_hash_byte_determinism_pin` trips |
| Signing-input format drift | `signing_input_byte_determinism_pin` trips |

---

## 10. V1 / V1.5 / V1.6 / V1.7 / V1.8 / V1.9 / V2 boundaries

### V1 — what ships now

- Deterministic CBOR signing-input, byte-pinned
- Ed25519 signatures, EdDSA-only
- DAG of events with parent-hash links
- `pubkey_bundle_hash` binding (constant-time, byte-pinned)
- Workspace-id binding (cross-workspace replay defence)
- Native CLI verifier + WASM in-browser verifier (one crate)
- MCP server with write/export tools
- Bank-persona golden trace, end-to-end ✓ VALID

### V1.5 — anchoring (shipped)

- Mock-Rekor issuer in `atlas-signer` (deterministic dev key, no live
  network call required)
- RFC 6962-style Merkle inclusion proofs with `leaf:`/`node:` domain
  separation
- Verifier validates each inclusion proof + Ed25519 checkpoint signature
  against a pinned log pubkey
- `bundle_hash` and `dag_tip` anchor kinds in one batched checkpoint
- Lenient default + `require_anchors` strict mode for high-assurance
  audit profiles
- `atlas_anchor_bundle` MCP tool persists `data/{workspace}/anchors.json`
  for inclusion in `atlas_export_bundle`

### V1.6 — live Sigstore submission (shipped)

- Live Sigstore Rekor v1 submission via `atlas-signer anchor --rekor-url`
  (also `ATLAS_REKOR_URL` env in MCP; precedence: field > env > mock)
- hashedrekord/v0.0.1 wire format against rekor.sigstore.dev
- SHA-256 RFC 6962 + ECDSA P-256 over C2SP signed-note checkpoint
- `sigstore_anchored_hash_for(kind, blake3_hex)` cross-format hash
  derivation with `atlas-dag-tip-v1:` / `atlas-bundle-hash-v1:` domain
  prefixes — single source for issuer + verifier
- Atlas anchoring key: ECDSA P-256 from deterministic seed, PEM pin
  (`atlas_anchor_pubkey_pem_is_pinned` test asserts it stays in sync)
- Verifier multi-format dispatch via `default_trusted_logs()` BTreeMap by
  `log_id`; Sigstore Rekor v1 production pubkey is a pinned entry
- apiVersion pin (`"0.0.1"` only, rejected otherwise)
- keyID rotation policy: multi-line signed-note, iterate signature lines
  by keyID; mismatch is `continue`, not error — success when ONE matches
  the pinned key
- HTTP client validation: https:// required for non-loopback hosts;
  plaintext http:// gated to localhost only
- Verifier path unchanged — no network call at verify time, fully offline
  RFC 6962 proof recomputation + ECDSA verify against pinned log pubkey

### V1.7 — anchor-chain tip-rotation + shard roster (shipped)

- **Anchor-chain tip-rotation:** Each `AnchorBatch` is cross-linked to
  predecessors via blake3 hash-chain with domain prefix `atlas-anchor-chain-v1:`.
  Verifier walks `trace.anchor_chain.history[]` and validates monotonic growth
  (no gaps, no reorder, previous_head continuity). Optional on old bundles
  (lenient by default); strict mode `require_anchor_chain` demands presence.
  Storage: `data/{workspace}/anchor-chain.jsonl` append-only log, atomic append
  via tmp-and-rename.
- **Issuer:** `atlas-signer anchor --chain-path <file>` reads JSONL, appends new batch.
  Sole writer of the chain file. `MCP` server passes workspace path; issuer handles
  read-modify-append.
- **Cross-check:** `anchor_chain_coverage` verification ensures every entry in
  `trace.anchors` (latest snapshot) appears byte-identical in some chain batch.
  Catches mixed-mode workspaces (mock→Sigstore transitions without chain
  continuity preservation) because Sigstore entries are absent from mock-only chain.
- **Precision-preserving JSON (V1.8):** MCP server routes signer-stdout and
  on-disk anchor JSON through `lossless-json`; oversized `tree_id` values
  (~2^60) survive round-trip byte-identical. Sigstore-path chain extension
  re-enabled. Coverage check carves out Sigstore entries (not required to be
  in chain) since Sigstore Rekor v1's own log provides monotonicity; mock
  entries are still required to be in chain.
- **Chain rotation:** `atlas-signer rotate-chain --confirm <workspace>` produces
  new genesis batch with `previous_head = old_chain.head`, bridging continuity.
- **Sigstore Rekor v1 shard roster:** Replaces single-tree-id pin with
  `SIGSTORE_REKOR_V1_TREE_IDS: &[i64]` roster of 3 shards (active +
  2 historical): `1_193_050_959_916_656_506`,
  `3_904_496_407_287_907_110`, `2_605_736_670_972_794_746`. Verifier accepts
  membership; same pubkey across shards, no cross-shard replay (C2SP origin
  embeds tree_id). Issuer still posts to active shard only.

### V1.9 — per-tenant Atlas anchoring keys (shipped)

- **HKDF-SHA256 derivation:** Per-workspace Ed25519 keypair derived from
  a single master seed via `HKDF-SHA256(salt=None, ikm=master_seed,
  info="atlas-anchor-v1:" || workspace_id, len=32)`. Compromise of one
  workspace's signing key does not compromise other workspaces — HKDF
  is one-way per-info.
- **Kid shape:** Per-tenant keys appear in `PubkeyBundle.keys` under
  `atlas-anchor:{workspace_id}`. Prefix pinned in
  `atlas_trust_core::per_tenant::PER_TENANT_KID_PREFIX` (verifier-side)
  and mirrored in `apps/atlas-mcp-server/src/lib/keys.ts` (issuer-side).
  Verifier-side kid prefix (`atlas-anchor:`) is intentionally distinct
  from the issuer-side HKDF info-prefix (`atlas-anchor-v1:`) so the
  cryptographic-domain tag stays decoupled from the wire-format
  identifier.
- **Strict mode:** `VerifyOptions::require_per_tenant_keys` (default
  `false` for backwards compatibility) demands every event's `kid` equal
  `format!("atlas-anchor:{trace.workspace_id}")`. Mixed legacy +
  per-tenant kids are rejected. V1.5–V1.8 bundles continue to verify in
  lenient mode.
- **Production gate (V1.9 — superseded V1.10, removed V1.12):** All V1.9
  per-tenant subcommands (`derive-key`, `derive-pubkey`,
  `rotate-pubkey-bundle`, `sign --derive-from-workspace`) originally
  refused to run when `ATLAS_PRODUCTION=1` was set. V1.10 superseded this
  negative opt-out with a positive opt-in (`ATLAS_DEV_MASTER_SEED=1`
  required to admit the dev seed) — see §V1.10 below. V1.12 removed the
  `ATLAS_PRODUCTION` env var entirely (the literal-`"1"`-only recognition
  was a documented operator footgun, and the V1.10 positive opt-in covers
  the same security property without it). From V1.12 onwards the variable
  is silently ignored. The `DEV_MASTER_SEED` is still a source-committed
  dev constant; the V1.10 wave-2 HSM trio is the production replacement.
- **Signer-internal derivation:** MCP hot path uses
  `atlas-signer sign --derive-from-workspace <ws>` — secret material is
  derived inside the signer process and never crosses the TS↔Rust
  boundary. Bundle assembly uses `atlas-signer derive-pubkey` (public
  key only). The `derive-key` subcommand (which emits the secret) is
  ceremony-only and gated by the V1.10 positive opt-in.
- **Rotate-pubkey-bundle ceremony:** `atlas-signer rotate-pubkey-bundle
  --workspace <ws>` reads a `PubkeyBundle` from stdin and emits the
  bundle with the per-tenant kid + pubkey added. Idempotent: re-runs
  on an already-rotated bundle return the bundle unchanged (existing
  pubkey asserted to match the derivation; mismatch refuses to
  overwrite). Atomic-replace and inter-operator concurrency are
  operator-side responsibilities — see `docs/OPERATOR-RUNBOOK.md`.
- **Workspace-id ingress validation:** `atlas-signer::keys::validate_workspace_id`
  restricts workspace_ids to ASCII printable bytes (0x21..0x7E) and
  forbids the `:` delimiter character. The verifier remains lenient by
  design (the trust property holds for any UTF-8 string via byte-equal
  kid compare) — hygiene lives at the issuer ingress where ambiguous
  IDs become observability holes.
- **Pinned pubkey goldens:** `workspace_pubkeys_are_pinned` test pins
  the derived public keys for two fixed workspace_ids against the
  `DEV_MASTER_SEED`, so any unintended change to the HKDF info-prefix
  or master seed trips CI before customer impact.

### V1.10 — master-seed gate inversion + sealed-seed loader (shipped)

- **Wave 1 — gate inversion:** Negative opt-out
  (`ATLAS_PRODUCTION=1` → refuse) replaced by positive opt-in
  (`ATLAS_DEV_MASTER_SEED=1` → admit dev seed; default refuses). Forgetting
  the env var fails closed with an actionable error rather than silently
  signing with the source-committed dev seed. Accepted truthy values:
  `1` / `true` / `yes` / `on` (ASCII case-insensitive, surrounding
  whitespace tolerated). Implemented via `master_seed_gate` +
  `master_seed_loader` in `crates/atlas-signer/src/keys.rs`. See §7.4 for
  the trust-model write-up.
- **Wave 2 — sealed-seed loader:** PKCS#11 backend at
  `crates/atlas-signer/src/hsm/` (gated behind the `hsm` Cargo feature)
  closes the V1.9 master-seed residual risk. The HSM trio
  (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`) routes
  HKDF-SHA256 to `CKM_HKDF_DERIVE` *inside* the HSM via the
  `MasterSeedHkdf` trait. The master seed never enters Atlas address
  space. HSM init failure is fatal — there is no silent fallback to the
  dev seed when the trio is set.
- **Trio takes precedence over dev opt-in:** When all three
  `ATLAS_HSM_*` vars are set, the loader signs against the sealed master
  seed inside the HSM and the dev opt-in is unreachable. From V1.10
  wave 2 onwards the HSM trio (with `--features hsm`) is the sole
  production audit signal.
- **Test-driven boundary:** Loader logic accepts an injected env-reader
  closure (`env_pairs` helper in `test_support.rs`) so the entire wave-1
  truth table and wave-2 trio-precedence semantics are testable without
  touching process env. CI exercises 200+ env-permutation tests across
  the master-seed surface.

### V1.11 — wave-3 sealed per-workspace signer (shipped)

- **CKM_EDDSA inside HSM:** Per-workspace Ed25519 signing key is sealed
  inside the HSM and signs via `CKM_EDDSA`. The per-workspace signing
  key never enters Atlas address space (the V1.10 wave-2 master seed
  is sealed; V1.11 wave-3 seals the *derived* per-workspace key as
  well). See §7.5 for the trust-model write-up.
- **WorkspaceSigner trait:** Two implementations behind the same trait
  (`crates/atlas-signer/src/workspace_signer.rs`):
  `DevWorkspaceSigner` (in-process Ed25519 over the
  HKDF-derived seed) and `Pkcs11WorkspaceSigner` (CKM_EDDSA inside the
  HSM, gated by `--features hsm`).
- **Three-layer dispatcher:** `workspace_signer_loader_with` selects
  one of three signing backends based on env: wave-3 sealed
  (`ATLAS_HSM_WORKSPACE_SIGNER=1` + HSM trio →
  `Pkcs11WorkspaceSigner`), wave-2 sealed-seed (HSM trio without
  wave-3 opt-in → `DevWorkspaceSigner` over `Pkcs11MasterSeedHkdf`),
  wave-1 dev (`ATLAS_DEV_MASTER_SEED=1` only → `DevWorkspaceSigner`
  over `DevMasterSeedHkdf`). Every other env shape refuses to start
  with an actionable error.
- **Multi-token redundancy trade-off:** The wave-3 sealed signer trades
  HKDF-mathematical determinism (wave-2) for HSM-keystore determinism
  (wave-3): rotating to a new HSM token requires re-importing the
  per-workspace key under the new token's wrapping key, since the
  derived key is no longer reproducible from the master seed alone.
  The runbook covers the two-HSM ceremony for redundancy.

### V1.12 — operator-surface simplification (shipped)

- **`ATLAS_PRODUCTION` env var removed.** The V1.9 negative gate became
  the V1.10 positive opt-in; from V1.12 onwards the V1.9 variable is
  silently ignored across the entire crate (master-seed gate, master-seed
  loader, workspace-signer loader, and import-ceremony scripts). Three
  reasons drove the removal: (1) the literal-`"1"`-only recognition was
  a documented operator footgun (`ATLAS_PRODUCTION=true` silently behaved
  as unset); (2) the V1.10 positive opt-in covers the same security
  property without the footgun; (3) the V1.10 wave-2 HSM trio (with
  `--features hsm`) is now the sole production audit signal. See
  §V1.10 above and §7.4 for the trust-model write-up of the simplified
  surface.
- **Documentation sweep.** ARCHITECTURE.md, OPERATOR-RUNBOOK.md,
  SECURITY-NOTES.md, and atlas-mcp-server/README.md were updated in a
  single coherent commit so that the documented operator surface
  matches the runtime behaviour. The runbook §1 truth tables collapsed
  from 4→3 rows (master-seed gate) and 5→4 rows (master-seed loader);
  the V1.9 paranoia-layer rows were removed since the runtime no
  longer admits them.
- **Migration:** Existing operators who still set `ATLAS_PRODUCTION=1`
  in their deployment scripts should remove the line — V1.12 silently
  ignores it, but leaving it in place makes audit logs misleading. The
  V1.10 positive opt-in (`ATLAS_DEV_MASTER_SEED=1`) and the V1.10 wave-2
  HSM trio remain the supported production-readiness signals.
- **CI-lane promotion (Scope B).** Three CI lanes were promoted from
  manual-only (`workflow_dispatch`) to auto-trigger on PR + push +
  schedule: `hsm-byte-equivalence` (V1.10 wave-2 drift sentry),
  `hsm-wave3-smoke` (V1.11 wave-3 end-to-end sentry), and
  `sigstore-rekor-nightly` (live Sigstore Rekor + pinned-roster
  drift sentry, cron `0 6 * * *` UTC). The lanes encode the
  trust-property invariants of the V1.6+V1.7+V1.8 anchor stack
  and the V1.10/V1.11 sealed-key stack as auto-fired CI signals;
  see `docs/SECURITY-NOTES.md` §"CI lanes" for the trust-model
  rationale and `docs/OPERATOR-RUNBOOK.md` §8 for the operator-
  facing failure-handling sketches. Each workflow file carries an
  inline header documenting its trigger surface, the invariant under
  test, and the rationale for SHA-pinned actions + paths-filter +
  permissions block.

### V1.13 — independent witness cosignature (shipped: wave-C-1 lenient + wave-C-2 strict)

- **Wave-C-1 — lenient witness primitive.** A new
  `WitnessSig { witness_kid: String, signature: String }` slot on
  every `AnchorBatch` carries Ed25519 signatures over
  `ATLAS_WITNESS_DOMAIN || chain_head_for(batch).to_bytes()` from
  third-party cosigners drawn from the pinned
  `ATLAS_WITNESS_V1_ROSTER` (genesis-empty in this version; populated
  via the wave-C-2 commissioning ceremony). The verifier surfaces a
  `witnesses` evidence row in lenient disposition: failures and
  unknown kids do not invalidate a trace, but auditors see the
  per-witness breakdown via `WitnessFailure::Display`. Wave-C-1's
  duplicate-`witness_kid` defence rejects every occurrence of a
  repeated kid as a failure (the dedup key is `witness_kid`, not the
  signature bytes) — preventing an issuer from satisfying an M-of-N
  quorum by attaching N entries under one commissioned kid.
- **Wave-C-2 — strict-mode threshold.** A new option
  `VerifyOptions.require_witness_threshold: usize` (with `0` as the
  lenient sentinel) and the matching CLI flag
  `atlas-verify-cli --require-witness <N>` promote the witness
  check to operationally load-bearing: traces with `verified < N`
  fail (`witnesses-threshold` evidence row carries `ok=false` and a
  matching error lands in `VerifyOutcome.errors`). The
  `aggregate_witnesses_across_chain_with_roster` function deduplicates
  kids across batches via a `BTreeSet<String>` so one compromised key
  cannot satisfy threshold N by signing N batches under the same
  kid — preserving M-of-N independence as a load-bearing trust
  property.
- **`ChainHeadHex` newtype.** Wraps the canonical 64-char lowercase
  hex of `chain_head_for(batch)`. The strict constructor rejects
  any other shape; the production producer (`chain_head_for`)
  bypasses the constructor on the hot path and a `debug_assert`
  guards against future `hex` crate behavioural drift. Surfaces the
  "freshly recomputed head" vs "wire-side string" boundary in the
  type system so a wire field cannot silently flow into a
  recomputed-head slot during refactoring.
- **`MAX_WITNESS_KID_LEN = 256`.** Wire-side `witness_kid` cap fires
  before any roster lookup; oversized kids never echo in
  diagnostics — `sanitize_kid_for_diagnostic` collapses them to
  `"<oversize: N bytes>"` placeholders so an attacker submitting a
  multi-megabyte kid cannot amplify log volume across the per-witness
  failure record + lenient evidence row's `rendered.join("; ")`.
- **Trust property addition (wave-C-2).** `verified ==
  count(distinct kids whose pubkey is in ATLAS_WITNESS_V1_ROSTER AND
  whose Ed25519-strict signature over ATLAS_WITNESS_DOMAIN ||
  chain_head_bytes validates AND no other batch in the chain already
  attributed verification to that kid)`. Strict mode adds the
  invariant `verified >= require_witness_threshold` as a hard reject.
  See [SECURITY-NOTES.md](SECURITY-NOTES.md) §wave-c for the threat
  model and [OPERATOR-RUNBOOK.md §10](OPERATOR-RUNBOOK.md) for the
  commissioning ceremony.

### V1.14 — HSM-backed witness + auditor wire-surface + WASM publishing (shipped: Scope I + Scope J + Scope E)

- **HSM-backed witness backend.** A new `Pkcs11Witness`
  implementation of the dyn-safe `Witness` trait
  (V1.13 wave-C surface) seals the witness Ed25519 signing scalar
  inside a PKCS#11 token: signing routes through `CKM_EDDSA(Ed25519)`
  with `Sensitive=true`, `Extractable=false`, `Derive=false` on the
  private half. Closes V1.13's residual exposure that the witness's
  scalar transited the witness binary's address space on every
  `sign_chain_head` call (V1.13 used a 32-byte file-backed seed read
  into a `Zeroizing<[u8; 32]>` buffer for the lifetime of one sign).
  V1.14 wave Scope I removes the host-side scalar artefact entirely
  for HSM-backed deployments — no witness signing scalar ever
  reaches Atlas address space, even transiently.
- **Trust-domain separation.** The witness binary uses the
  `ATLAS_WITNESS_HSM_*` env-var prefix (distinct from atlas-signer's
  `ATLAS_HSM_*`) and the `atlas-witness-key-v1:` on-token label
  prefix (distinct from atlas-signer's `atlas-workspace-key-v1:`).
  An operator who accidentally re-uses the signer's prefix gets a
  clean "trio not set" SKIP from the witness binary, NOT a surprise
  authentication against the signer's HSM token under the witness's
  identity. Production hygiene wants slot-level separation too;
  the prefix split is defence-in-depth on top.
- **Single key per witness binary.** Unlike the V1.11 wave-3
  per-workspace signer (which fans out to many keypairs lazily),
  the witness has exactly one keypair per binary instance.
  Multi-witness deployments run multiple `atlas-witness` binaries
  each pinned to its own kid + token slot, not one binary cycling
  through kids — keeps the trust attestation surface
  one-witness-at-a-time and matches the verifier-side roster grain
  (one entry per witness).
- **Operator-driven keypair generation.** `Pkcs11Witness::open`
  only **resolves** an existing keypair by label — it does NOT
  auto-generate. Generation is an operator action via
  `pkcs11-tool --keypairgen` per
  [OPERATOR-RUNBOOK §11](OPERATOR-RUNBOOK.md). This is the
  load-bearing trust property: a witness that auto-generated keys
  could be made to sign on a fresh, unrostered keypair and silently
  bypass the roster contract. Explicit operator action is the
  enforcement mechanism.
- **CLI surface.** `atlas-witness sign-chain-head --hsm` selects the
  HSM-backed backend (mutually exclusive with `--secret-file` via
  clap `ArgGroup`). `atlas-witness extract-pubkey-hex --kid <kid>`
  retrieves the paired `CKO_PUBLIC_KEY` object's `CKA_EC_POINT`,
  unwraps the PKCS#11 v3.0 §10.10 DER OCTET STRING wrapper (also
  accepts the raw 32-byte form for vendors that deviate), and
  prints the 64-char hex pubkey for the §10 roster pinning step.
- **Trust property (V1.14 Scope I).** Same as V1.13 wave-C-2 — the
  witness check confirms third-party observation of the chain head
  against pinned roster keys. V1.14 strengthens the *substrate* in
  which the witness scalar lives (HSM vs file) without changing the
  trust contract. Old V1.13 file-backed witnesses remain valid;
  operators can migrate per-witness without coordinating a global
  cutover. See [SECURITY-NOTES.md](SECURITY-NOTES.md) §scope-i for
  the threat model and [OPERATOR-RUNBOOK.md §11](OPERATOR-RUNBOOK.md)
  for the commissioning ceremony.
- **Auditor wire-surface (Scope J).** Replaces V1.13 wave-C-2's
  string-match-against-evidence-detail diagnostic surface with
  structured `WitnessFailureWire` JSON. New
  `VerifyOutcome.witness_failures: Vec<WitnessFailureWire>` field
  (additive, `#[serde(default)]`) + `WitnessFailureReason`
  `#[non_exhaustive]` enum (kebab-case, nine variants) lets auditor
  tooling switch on `reason_code` instead of fragile wording match.
  At-source classification via a private categorised verifier; the
  public `verify_witness_against_roster` API is unchanged. SEC fix
  bundled: per-batch verifier sanitises `witness_kid` before
  constructing any failure record, defending the lenient evidence
  row from multi-MB blob amplification through `; `-join. CLI
  `--output json` carries the field; TS smoke lane parses it. See
  [§7.7](#77-auditor-wire-surface-v114--shipped-scope-j) for the
  technical design and [SECURITY-NOTES.md](SECURITY-NOTES.md)
  §scope-j for the threat model.
- **WASM verifier publishing (Scope E).** `crates/atlas-verify-wasm`
  ships as `@atlas-trust/verify-wasm` on npm — same Rust verifier
  core (`atlas-trust-core`) compiled to `wasm32-unknown-unknown`,
  packaged for both `--target web` (browser ESM, default `latest`
  dist-tag) and `--target nodejs` (CommonJS, `node` dist-tag). The
  `wasm-publish.yml` CI lane triggers on tag push (`v*` from the
  default branch) + manual `workflow_dispatch`, installs wasm-pack
  via `cargo install --locked` (rather than the upstream shell
  installer — auditable from crates.io source), runs Node.js smokes
  against `verify_trace_json` for **both** the pkg-web and pkg-node
  outputs end-to-end against the bank-demo fixture, and gates
  `npm publish` on a three-layer check: (a) trigger encodes a
  publish intent (tag push OR `workflow_dispatch` with
  `dry_run=false` from master), (b) `NPM_TOKEN` is present, (c) the
  publish step has `id-token: write` for OIDC-signed
  `--provenance`. Both tarballs ship with provenance attestations
  linking them to the GitHub Actions run + commit SHA, so
  downstream consumers can verify via `npm audit signatures`. A
  zero-build-step browser playground at `apps/wasm-playground/`
  (vanilla HTML + ESM, no bundler) lets an auditor drop a
  `*.trace.json` + `*.pubkey-bundle.json` and verify in-browser
  without touching a server. **Trust property unchanged:** the
  byte-identical-determinism property already locked in by
  `atlas-trust-core/src/cose.rs::signing_input_byte_determinism_pin`
  (V1.5) means the WASM build produces the same signing-input bytes
  and the same `VerifyOutcome` as the native CLI. Scope E is a new
  *distribution channel*, not a new trust surface.

### V1.15 — Const-time KID-equality invariant + dual-channel WASM distribution + consumer reproducibility runbook (shipped: Welle A + Welle B + Welle C)

- **Const-time KID compares everywhere.** V1.5 routed every *hash*
  comparison (bundle hash, event hash, anchored hash, chain head /
  previous-head) through `crate::ct::ct_eq_str`. V1.13 wave-C-2 routed
  the highest-impact *KID* compare (witness-roster lookup) through the
  same helper. V1.15 Welle A closes the last surviving raw-`==` path
  on a wire-side KID: the V1.9 per-tenant-keys strict-mode check now
  routes `event.signature.kid ↔ per_tenant_kid_for(workspace_id)`
  through `ct_eq_str`. The const-time-equality invariant now extends
  uniformly across every wire-side KID and hash compare reachable from
  the public verifier API.
- **Source-level anti-drift pin.** A new
  `crates/atlas-trust-core/tests/const_time_kid_invariant.rs` source-
  audit test asserts that `verify.rs` and `witness.rs` contain no
  forbidden raw-equality patterns (`.kid ==`, `.kid.eq(`,
  `.kid.as_str() ==`, `expected_kid ==`, `witness_kid ==`,
  `signature.kid ==`) in production code. Strips
  `#[cfg(test)] mod tests` blocks before scanning so test-side
  `assert_eq!(kid, …)` patterns don't false-positive. A future caller
  introducing a raw `==` on a KID field in either file fails the test
  at the next CI run; a new module with KID-compare sites must extend
  the audit list explicitly. The `crate::ct` module-doc enumerates the
  six const-time-protected boundaries (bundle hash, event hash,
  anchored hash, chain head/previous-head, per-tenant KID, witness
  KID) so reviewers reading any of those sites see the comment
  pointing back at the helper module.
- **Welle A trust property unchanged.** Welle A is consistency hardening,
  not a new trust property. The leak window for `event.signature.kid`
  was theoretical — both `event.signature.kid` and
  `per_tenant_kid_for(workspace_id)` are wire-side strings already
  present in the trace — but the consistency win means a future
  reviewer reading any KID-equality site sees the same `ct_eq_str`
  discipline. See [SECURITY-NOTES.md](SECURITY-NOTES.md) §scope-a for
  the per-boundary trust statement.
- **Dual-channel WASM distribution (Welle B).** V1.14 Scope E shipped
  `@atlas-trust/verify-wasm` to npmjs.org as the sole distribution
  channel. V1.15 Welle B adds a second, independent channel: on every
  `refs/tags/v*` push, `.github/workflows/wasm-publish.yml` uploads
  the byte-identical `npm pack` tarballs (web + node, with
  `-web.tgz` / `-node.tgz` suffix disambiguation) plus a
  `tarball-sha256.txt` manifest as GitHub Release assets, alongside
  the existing npm publish. An auditor whose primary channel is
  unreachable can `gh release download`, `sha256sum --check`, and
  `npm install ./local.tgz` — same `package.json` name, same SLSA
  L3 provenance attestation (the npm-side OIDC signature is verifiable
  against either channel's bytes by recomputing SHA256 / SHA512).
  Same-run, same-bytes — Welle B is purely additive distribution
  resilience, no verifier-logic change.
- **Latent OIDC-permissions fix.** Welle B's workflow edit also fixes
  a latent V1.14 Scope E bug: the original `id-token: write`
  permission was placed at *step* level, which GitHub Actions
  silently ignores. `npm publish --provenance` would have failed at
  runtime on the first `v*` tag push. Welle B moves both
  `id-token: write` (for OIDC provenance) and `contents: write` (for
  `gh release upload`) to workflow level, removes the dead step-level
  block, and documents the rationale in the workflow YAML.
- **Welle B trust property unchanged.** Welle B is distribution
  resilience, not a new trust property. The byte-identical
  determinism property and SLSA L3 provenance attestation extend to
  the GH-Release tarball unchanged — same workflow run, same
  `npm pack` bytes, verifiable against the same OIDC attestation.
  See [SECURITY-NOTES.md](SECURITY-NOTES.md) §scope-b for residual
  risks (both-channels-on-GitHub correlation, no registry-API
  equivalence, operator-driven failover) and
  [OPERATOR-RUNBOOK.md](OPERATOR-RUNBOOK.md) §12 for the backup-
  channel install ceremony.
- **Consumer-side reproducibility runbook (Welle C).**
  [docs/CONSUMER-RUNBOOK.md](CONSUMER-RUNBOOK.md) closes the V1.15
  distribution-resilience story on the consumer side. Documents
  exact-version pinning across `package-lock.json` /
  `pnpm-lock.yaml` / `bun.lockb` (each layer protects against a
  different threat: version pin defeats unaudited-minor-bump,
  lockfile SHA512 integrity defeats registry-side replacement,
  SLSA L3 provenance via `npm audit signatures` defeats forged-
  but-byte-different tarballs that would pass the first two
  layers); strict-install flags (`npm ci`, `pnpm install
  --frozen-lockfile`, `bun install --frozen-lockfile`); the GH-
  Releases backup-channel install flow with mandatory step-3 SHA512
  cross-verify against `npm view … dist.integrity`; and the
  reproduce-from-source fallback (`git checkout v<tag>`,
  `cargo install wasm-pack --version $WASM_PACK_VERSION --locked`,
  `wasm-pack build`, byte-compare against the published artefact)
  for the both-channels-unreachable scenario. Welle B is the upload
  side; Welle C is the consumer side; together they make the V1.15
  Welle B byte-identical-determinism trust property load-bearing
  for downstream installs, not just for the publish workflow.
- **Welle C trust property unchanged.** Pure-doc commit, no
  verifier-logic change. The trust property load-bearing for
  reproduce-from-source is the V1.5 byte-determinism pins
  (`signing_input_byte_determinism_pin`,
  `bundle_hash_byte_determinism_pin`) plus the V1.14 Scope E SLSA
  L3 OIDC provenance attestation; Welle C documents how a downstream
  consumer composes these into a verifiable install path.
- **V1.15 is now CLOSED.** All three planned Wellen (A const-time-
  KID, B GH-Releases backup, C consumer runbook) shipped on
  2026-05-04. V1.16 candidates (browser-runtime hardening if a
  hosting decision lands, multi-issuer Sigstore redundancy, auto-
  verify CI action for downstream consumers) are documented in
  `.handoff/v1.15-handoff.md` for the next session.

### V1.16 — Browser-runtime hardening of the WASM playground (shipped: Welle A + Welle B)

- **Strict CSP + SRI + Trusted Types on `apps/wasm-playground/` (Welle A).**
  The V1.14 Scope E playground page is hardened against UI-side
  injection for any deployment beyond pure local-dev. The application
  code is extracted from inline `<script type="module">` into a sibling
  `app.js`, allowing a strict CSP without `'unsafe-inline'` on
  `script-src` and a `sha384` Subresource Integrity hash on the
  loading `<script>` tag. CSP is shipped via `<meta http-equiv>` so
  the policy travels with the page bytes (no dependency on the hosting
  provider sending a `Content-Security-Policy` HTTP header).
- **Sink-free application discipline (Welle A).** `app.js` uses only
  `textContent`, `className`, and `style.display` — no `innerHTML`, no
  `eval`, no `new Function`, no `setTimeout(string)`, no `*.src` from
  user input. The CSP enforces this with `require-trusted-types-for
  'script'; trusted-types 'none'` — any future regression that
  re-introduces a script-related sink fails at the browser boundary,
  not at code-review time. This is the load-bearing TT setting for
  sink-free apps.
- **CSP violation reporting via `report-uri /csp-report` (Welle B).**
  The meta-tag CSP now declares a same-origin `report-uri` so that on
  every blocked violation (XSS attempt, accidental sink introduction,
  mis-configured cross-origin load) the browser POSTs a JSON report
  to the deployed receiver. Page-bytes-only — works on any plain
  static host where an operator stands up a `/csp-report` endpoint.
  Choice of `report-uri` over `report-to` (Reporting API) is forced
  by meta-tag delivery: `report-to` references a `Reporting-Endpoints`
  HTTP header which cannot be sent via `<meta>`. A header-mode-CSP
  deployment SHOULD declare BOTH for forward-compat. Receiver-shape
  spec + minimal-collector example are in
  [docs/SECURITY-NOTES.md](SECURITY-NOTES.md) §scope-d covers-bullet 6.
- **Anti-drift validator `tools/playground-csp-check.sh`.** Pure-bash
  validator (no Node/Python dependency) re-asserts the CSP directives,
  the SRI hash on `app.js`, the absence of `'unsafe-inline'` /
  `'unsafe-eval'` on `script-src`, the wasm-bindgen-glue TT-compat
  audit, AND (Welle B) the `report-uri` declaration + same-origin
  shape on every run. `--update-sri` flag re-pins the hash after a
  legitimate `app.js` edit.
- **Trust property unchanged.** V1.16 (both Wellen) is delivery-side
  hardening — the verifier's correctness, byte-determinism pins,
  signature integrity, hash-chain integrity, and anchor verification
  are unchanged from V1.15. Welle A buys *resistance to UI-side
  injection*; Welle B is *receiver-ready post-block visibility* —
  the page declares `report-uri /csp-report`, but actual visibility
  requires the operator to also stand up a receiver at that path
  matching the documented receiver-shape spec. Without a receiver,
  reports POST into a 404 (violation still blocked, report lost).
  Welle B closes F-3 at the page-bytes layer; the deployment-side
  closure is operator responsibility.
- **What V1.16 Welle A + B is NOT yet:** WASM-binary SRI (the
  `wasm-pack`-emitted loader uses `WebAssembly.instantiateStreaming`,
  which has no declarative SRI hook — the `WebAssembly.compile`
  integrity-check spec is proposal stage); service-worker pinning;
  JS-side input-size cap (the Rust verifier already caps allocation
  via `MAX_ITEMS_PER_LEVEL`); receiver implementation at
  `/csp-report` (operator responsibility); HTTP-header-mode `report-to`
  + `Reporting-Endpoints` (page-bytes can't enforce — operator
  config); hosting decision and DNS pinning. These are V1.16 Welle C
  / V1.17+ candidates documented in
  [docs/SECURITY-NOTES.md](SECURITY-NOTES.md) §scope-d "What scope-d
  does NOT cover".

### V2 — full COSE + policy + SPIFFE

- Switch to RFC 9052 COSE_Sign1 with full CTAP2 canonical CBOR
  (current "simplified V1" envelope is the migration target)
- Cedar policy enforcement at write time + at verify time
- SPIFFE SVID validation against in-domain trust bundle
- Bundle-of-bundles: cross-bundle anchor chaining so the auditor can
  walk every historical `PubkeyBundle` back to a single root anchor
  (V1.5 anchors the current bundle hash; V2 chains them)
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
- **Live Sigstore client (V1.6).** Both V1.5 mock-Rekor and V1.6 live
  Sigstore Rekor v1 submission are fully verifiable offline —
  inclusion proof + signed checkpoint against pinned log pubkey are part
  of the verifier's `valid: true` guarantee. Operator can opt into
  production audit trails (`--rekor-url https://rekor.sigstore.dev`) or
  stay offline with the mock. Verifier path is identical.
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
cargo test -p atlas-trust-core -p atlas-signer --release
# 41 tests pass on Linux, macOS, and Windows-MSVC
# (36 in atlas-trust-core + 5 in atlas-signer).
```

If the verifier on your machine produces the same evidence as the
verifier on ours, you have independently confirmed the trust property.
That is the entire pitch.
