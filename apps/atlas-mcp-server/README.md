# atlas-mcp-server

The Model Context Protocol (MCP) server that lets AI agents — Claude
Desktop, Cursor, custom — write to an Atlas verifiable knowledge graph
through tool calls. Every write is signed by `atlas-signer` (Rust,
Apache-2.0) before it touches storage; every export emits a bundle that
any party can verify offline with `atlas-verify-cli`.

This package is licensed under the **Atlas Sustainable Use License**.
Self-hosting for your own internal use is free; running it as a
multi-tenant service for third parties requires a commercial licence
(nelson@ultranova.io).

---

## Tools exposed

| Tool | Purpose |
|---|---|
| `atlas_write_node` | Append a `node.create` event (dataset, model, inference, etc.) |
| `atlas_write_annotation` | Append an `annotation.add` event — used for human verification |
| `atlas_anchor_bundle` | Issue inclusion proofs over current `dag_tips` + `pubkey_bundle_hash`. Optional `rekor_url` field (or `ATLAS_REKOR_URL` env, precedence: field > env > mock) selects live Sigstore Rekor v1 submission or in-process mock-Rekor issuer. |
| `atlas_export_bundle` | Emit an `AtlasTrace` + matching `PubkeyBundle` for a workspace |
| `atlas_workspace_state` | Inspect current DAG tips, event count, kid roster |

All tools take a `workspace` argument (default: `ws-mcp-default`).
Workspaces are isolated on disk under `data/{workspace}/events.jsonl`;
issued anchors are persisted alongside as `data/{workspace}/anchors.json`
(latest snapshot, unchanged) and `data/{workspace}/anchor-chain.jsonl` (V1.7,
append-only log of all anchor batches, one batch per line as JSON). Both are
folded into `trace.anchors` and `trace.anchor_chain` on export.

---

## Trust property preserved

The single non-negotiable: the canonical bytes the MCP server emits
(both signing-input and pubkey-bundle hash) must be **bit-identical**
to what `atlas-trust-core` recomputes during verification.

The MCP server in TypeScript owns *zero* canonical-bytes formatting:

- Event signing → spawns `atlas-signer sign` (canonical CBOR signing-input
  + Ed25519 + emit `AtlasEvent` JSON).
- Pubkey-bundle hash → spawns `atlas-signer bundle-hash` (canonical
  JSON over the bundle + blake3 + emit hex).

Both subcommands use the same Rust functions the verifier later runs.
A drift between TS and Rust is structurally impossible because TS has
no canonicalisation code path to drift.

Secret material is delivered to the signer via stdin (`--secret-stdin`),
not argv — argv values appear in OS process listings and shell history.

If the smoke test (`pnpm smoke`) ever fails to verify ✓ VALID, the bug is
real, not cosmetic.

---

## Running locally

```bash
# 1. Build the Rust signer (release for speed)
cargo build --release -p atlas-signer

# 2. Install + build the MCP server
cd apps/atlas-mcp-server
pnpm install
pnpm build

# 3. Run the smoke test (writes 3 events → issues 2 anchors → exports → CLI-verifies)
pnpm smoke
```

Expected smoke output ends with:

```
✓ atlas-verify-cli: VALID
  ✓ schema-version
  ✓ pubkey-bundle-hash
  ✓ event-hashes
  ✓ event-signatures
  ✓ parent-links
  ✓ dag-tips
  ✓ anchors
  ✓ anchor-chain (V1.7: validates chain monotonicity)
[smoke] v1.14-json     ✓ outcome.witness_failures present and []
```

The trailing `v1.14-json` line is the V1.14 Scope J auditor wire pin:
the smoke re-runs `atlas-verify-cli` with `-o json` and asserts that
`VerifyOutcome.witness_failures` deserialises as a (here-empty) array.
A regression that omitted, renamed, or `null`-valued the field would
fail this leg before the smoke completes. The populated path (with
bad witnesses) is exercised by Rust integration tests in
`crates/atlas-verify-cli/tests/witness_failures_json.rs` and
`crates/atlas-trust-core/tests/verify_outcome_witness_failures.rs`.

---

## Wiring it into Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`
(macOS) or the equivalent on Windows / Linux:

```json
{
  "mcpServers": {
    "atlas": {
      "command": "node",
      "args": ["/absolute/path/to/atlas/apps/atlas-mcp-server/dist/index.js"],
      "env": {
        "ATLAS_DEV_MASTER_SEED": "1"
      }
    }
  }
}
```

After restarting Claude Desktop, the four `atlas_*` tools appear in the
tool palette. Every tool call produces a signed event in the configured
workspace's `events.jsonl`.

The `ATLAS_DEV_MASTER_SEED=1` env entry is the V1.10 positive opt-in
that admits the source-committed dev master seed (per-tenant
subcommands refuse to start otherwise). For a production deployment,
omit `ATLAS_DEV_MASTER_SEED`, build `atlas-signer` with
`--features hsm`, and configure the V1.10 wave-2 sealed-seed trio
(`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`).
The HSM trio takes precedence over the dev gate: when set, the
loader signs against the sealed master seed inside the HSM and the
dev opt-in is unreachable. (V1.12 removed the V1.9-era
`ATLAS_PRODUCTION=1` paranoia layer; the HSM trio is now the sole
production audit signal.) See
[../../docs/OPERATOR-RUNBOOK.md](../../docs/OPERATOR-RUNBOOK.md)
§2 for the import ceremony.

---

## V1.5 / V1.6 / V1.7 / V1.8 / V1.9 / V1.10 boundaries

- **Master-seed gate inversion (V1.10 wave 1; V1.12-simplified).**
  Per-tenant subcommands refuse to start unless
  `ATLAS_DEV_MASTER_SEED=1` is explicitly set. V1.12 removed the
  V1.9-era `ATLAS_PRODUCTION` paranoia layer (it had a documented
  literal-`1`-only footgun, and the positive opt-in covers the same
  security property). The V1.9 negative gate has been replaced with
  a positive opt-in: forgetting the env var now fails closed with
  an actionable error rather than silently signing with the
  source-committed dev seed. **Local development
  setup:** export `ATLAS_DEV_MASTER_SEED=1` once before `pnpm dev`
  (or wire it into your shell profile / `.envrc`). The smoke test
  (`pnpm --filter atlas-mcp-server smoke`) sets it programmatically
  at the top of `main()`, so CI does not require operator
  pre-configuration. The accepted truthy values are `1` / `true` /
  `yes` / `on` (ASCII case-insensitive, surrounding whitespace
  tolerated); any other value refuses the dev seed. See
  [../../docs/OPERATOR-RUNBOOK.md](../../docs/OPERATOR-RUNBOOK.md)
  §1 for the full truth table and security rationale.
- **Sealed-seed loader (V1.10 wave 2, shipped).** The PKCS#11
  backend at `crates/atlas-signer/src/hsm/` (gated behind the `hsm`
  Cargo feature) closes the V1.9 master-seed residual risk.
  Production deploys configure the HSM trio
  (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`)
  and rebuild `atlas-signer` with `--features hsm`. HKDF-SHA256
  runs *inside* the HSM via `CKM_HKDF_DERIVE`; the master seed
  never enters Atlas address space. HSM init failure is fatal —
  there is no silent fallback to the dev seed when the trio is
  set. The MCP server itself does not need `--features hsm`; only
  the spawned `atlas-signer` binary does. See
  [../../docs/OPERATOR-RUNBOOK.md](../../docs/OPERATOR-RUNBOOK.md)
  §2 for the SoftHSM2 import ceremony, threat model, and rotation
  procedure.
- **Per-tenant Atlas anchoring keys (V1.9).** Each workspace gets its
  own Ed25519 signing key derived from a single master seed via
  HKDF-SHA256 (info=`"atlas-anchor-v1:" || workspace_id`). Public keys
  appear in `PubkeyBundle.keys` under kid shape
  `atlas-anchor:{workspace_id}`. The MCP hot path uses
  `atlas-signer sign --derive-from-workspace <ws>` so per-tenant
  secrets never cross the TS↔Rust boundary; bundle assembly uses
  `atlas-signer derive-pubkey` (public key only). V1.5–V1.8 SPIFFE
  kids continue to verify in lenient mode; strict mode
  (`require_per_tenant_keys`) demands per-tenant kids matching the
  trace's `workspace_id`. Bundle rotation:
  `atlas-signer rotate-pubkey-bundle --workspace <ws>` — see
  [../../docs/OPERATOR-RUNBOOK.md](../../docs/OPERATOR-RUNBOOK.md).
- **Signing keys (V1 legacy).** The three globally-shared SPIFFE
  keys (agent / human / anchor) in `src/lib/keys.ts` remain present
  for V1.5–V1.8 backwards compatibility and the bank-demo example.
  Their hex secrets are passed to the signer via stdin
  (`--secret-stdin`); the `--secret-hex` argv flag emits a
  deprecation warning.
- **Persistence is JSONL on local disk.** V2 ships pluggable storage
  (Postgres, S3, FalkorDB).
- **Anchoring (V1.5 mock-Rekor, V1.6 live Sigstore, V1.7 chain extension).**
  `atlas_anchor_bundle` issues anchors via the in-process mock-Rekor by
  default (V1.5, no network call). Setting the `rekor_url` field or
  `ATLAS_REKOR_URL` env (precedence: field > env > mock) opts into live
  Sigstore Rekor v1 submission against `https://rekor.sigstore.dev`. The
  verifier accepts both formats by log_id dispatch and validates inclusion
  proofs + checkpoint signatures against pinned log pubkeys (mock Ed25519
  or Sigstore ECDSA P-256). Verifier path unchanged — fully offline RFC 6962
  proof recomputation in both cases.
- **Anchor-chain tip-rotation (V1.7).** Each `atlas_anchor_bundle` call
  appends a new `AnchorBatch` to `data/{workspace}/anchor-chain.jsonl`,
  cross-linked to the previous batch via blake3 hash-chain (domain prefix
  `atlas-anchor-chain-v1:`). The verifier walks the chain and validates
  monotonic growth (no gaps, no reorder, chain continuity). Old bundles
  lacking the chain pass under lenient mode; strict mode
  (`require_anchor_chain`) demands a present, valid chain.
- **Precision-preserving JSON (V1.8).** Every signer-stdout and on-disk
  anchor JSON boundary parses through `lossless-json`
  (`src/lib/anchor-json.ts`). Oversized integers — notably the Sigstore
  Rekor v1 production `tree_id` (~2^60), which exceeds JS
  `Number.MAX_SAFE_INTEGER` — survive round-trip byte-identical. The Zod
  boundary on `AnchorEntry.tree_id` accepts native `number` (safe range)
  or a `LosslessNumber` whose `.value` is a non-negative integer literal
  bounded at 19 decimal digits (i64 magnitude); scientific notation,
  fractionals, and malformed literals fail with a descriptive error
  rather than silent truncation. This lifts the V1.7 Sigstore-path
  chain-extension gate. The verifier's coverage check carves out
  Sigstore entries (deferred to Rekor's own monotonicity), so V1.7-issued
  bundles continue to verify; mock entries are still required to be in
  chain.
- **Sigstore shard roster (V1.7).** Verifier accepts the active production
  Sigstore shard plus two historical shards, all signed by the pinned
  ECDSA P-256 public key. Same single-key trust property; no cross-shard
  replay (C2SP origin embeds tree_id). Issuer still posts only to active
  shard.
- **Auditor wire-surface (V1.14 Scope J).** `VerifyOutcome` now carries
  a structured `witness_failures: WitnessFailureWire[]` field
  alongside `valid` / `evidence` / `errors`. Each entry pins
  `{ witness_kid, batch_index, reason_code, message }` where
  `reason_code` is a kebab-case enum (`kid-not-in-roster`,
  `duplicate-kid`, `cross-batch-duplicate-kid`,
  `invalid-signature-format`, `invalid-signature-length`,
  `oversize-kid`, `chain-head-decode-failed`,
  `ed25519-verify-failed`, `other`). Auditor tooling switches on
  `reason_code` for classification instead of string-matching the
  human-readable `evidence` row's `detail`. The field is additive
  (`#[serde(default)]` on the Rust side) — pre-J consumers parsing
  pre-J payloads see `witness_failures: []`; post-J consumers
  parsing pre-J payloads also see `[]`. The smoke test
  `pnpm smoke` step 8 (`v1.14-json`) parses the field via
  `JSON.parse` and asserts the array shape on every run, pinning
  the wire contract from the TS consumer side.

See [../../docs/ARCHITECTURE.md](../../docs/ARCHITECTURE.md) for the
full system context.
