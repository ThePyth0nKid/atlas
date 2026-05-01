# Atlas — Operator Runbook (V1.9)

This document is the operator's reference for Atlas ceremonies that
sit *outside* the agent-driven hot path: workspace key migration,
chain rotation, bundle assembly, and the production-gate environment.

It is written for operators of an Atlas deployment, not for
engineers building Atlas itself nor for auditors verifying a trace
bundle. For those audiences see
[ARCHITECTURE.md](ARCHITECTURE.md) and
[SECURITY-NOTES.md](SECURITY-NOTES.md).

---

## 1. Production gate

V1.9 introduces per-tenant Atlas anchoring keys derived from a
single `DEV_MASTER_SEED` constant in `atlas-signer/src/keys.rs`. The
seed is source-committed for reproducibility of dev/CI builds and is
**not** a production secret. V1.10 closes this with HSM/TPM sealing
of the master seed.

> **DANGEROUS DEFAULT — read this before running in production.**
>
> The V1.9 gate is *opt-out*, not opt-in. If `ATLAS_PRODUCTION` is
> **unset** (or set to anything other than the literal `1`), every
> per-tenant subcommand will run happily against the source-committed
> `DEV_MASTER_SEED`. An attacker who can read public source can
> re-derive every workspace's signing key.
>
> **Only the literal byte string `1` trips the gate.** This is a
> deployment trap if your automation follows the common Docker /
> Kubernetes idiom of `ENV=true`:
>
>   * `ATLAS_PRODUCTION=1` — gate **fires**, per-tenant subcommands refuse;
>   * `ATLAS_PRODUCTION=true` — gate **silently does not fire**, dev seed signs;
>   * `ATLAS_PRODUCTION=yes` — gate **silently does not fire**, dev seed signs;
>   * `ATLAS_PRODUCTION=on` — gate **silently does not fire**, dev seed signs;
>   * `ATLAS_PRODUCTION="1 "` (trailing whitespace) — gate **silently does not fire**;
>   * unset — gate **silently does not fire**, dev seed signs.
>
> Wire your deploy automation to assert the value is exactly `1` (no
> trimming, no truthy-coercion). V1.10 inverts these semantics to
> positive opt-in and removes the literal-string trap.
>
> **Unset or non-literal `ATLAS_PRODUCTION` in a production deployment
> is a misconfiguration.** Treat it like an unset DB password: the
> deployment should fail closed at startup, not silently keep
> running. V1.9 does not enforce this for you — operators must wire
> a deploy-time check that `ATLAS_PRODUCTION` is byte-exactly `1` on
> every signer host before traffic flows.
>
> **V1.9 has no production-safe path.** Even with `ATLAS_PRODUCTION=1`
> set, V1.9 cannot serve true production traffic — the gate refuses
> per-tenant signing because there is no sealed seed source yet.
> Setting the gate is a *forward-compatibility* step (the same env
> var carries V1.10's positive-opt-in semantics) and a *failure-mode*
> guarantee (fail-loud refusal vs silent dev-seed leakage), not a
> green-light to take real traffic. Wait for V1.10's sealed-seed
> loader before pointing real users at the signer.

To prevent accidental use of the dev seed in production, every V1.9
per-tenant subcommand refuses to run *when* `ATLAS_PRODUCTION=1`:

- `atlas-signer derive-key --workspace <ws>`
- `atlas-signer derive-pubkey --workspace <ws>`
- `atlas-signer rotate-pubkey-bundle --workspace <ws>`
- `atlas-signer sign --derive-from-workspace <ws>`

The gate's purpose is to make the V1.9 dev seed impossible to use
*by accident in a production environment that has been correctly
configured*. The gate is **not** a guarantee that production is
configured correctly.

```bash
# In production environments — set this on the signer host:
export ATLAS_PRODUCTION=1
# atlas-signer per-tenant subcommands will now refuse to run with
# the dev seed. Wire a real seed source (V1.10) before flipping.
```

### V1.10 inversion (planned)

When V1.10 ships, the production-gate semantics flip from negative
guard to positive opt-in:

- `ATLAS_PRODUCTION=1` becomes the **required** signal that the
  operator has wired a sealed master seed (HSM/TPM/cloud-KMS).
  Without it, the per-tenant subcommands refuse to run *at all* in
  any context that the binary recognises as non-dev.
- The dev path remains available in CI and local dev, gated by
  build-time feature flag or an explicit `ATLAS_DEV_SEED=1` opt-in,
  with loud stderr warnings on every invocation.
- The "unset env var" failure mode disappears: operators must affirm
  the seed source rather than affirm the absence of the dev source.

Operators should set `ATLAS_PRODUCTION=1` *now* in production and
accept the V1.9 refusal — the alternative (running the dev seed in
production) is the unsafe path the gate exists to prevent. The same
env var will keep working under V1.10 with the inverted semantics,
so the runbook step is forward-compatible.

---

## 2. Workspace pubkey-bundle rotation ceremony

V1.5–V1.8 bundles were assembled from three globally-shared SPIFFE
keypairs. V1.9 adds per-workspace Ed25519 keys under kid shape
`atlas-anchor:{workspace_id}`. Existing bundles must be migrated by
running `rotate-pubkey-bundle` once per active workspace.

### When to run

- After upgrading from V1.5–V1.8 to V1.9, before the first event is
  written under V1.9 in any workspace that should use per-tenant keys.
- After provisioning a new workspace that did not exist when the
  bundle was last assembled (the MCP server's
  `buildBundleForWorkspace` already handles this case for
  bundle-on-export; the rotate ceremony is for materialising the
  per-tenant kid in a long-lived bundle file before any event flows).

### What it does

`atlas-signer rotate-pubkey-bundle --workspace <ws>` reads a
`PubkeyBundle` from stdin and emits the bundle on stdout with the
per-tenant kid (`atlas-anchor:{ws}`) and HKDF-derived public key
inserted. The legacy SPIFFE kids are preserved unchanged so V1.5–V1.8
traces continue to verify in lenient mode.

The subcommand is **idempotent**: a re-run on an already-rotated
bundle returns the bundle unchanged. If the existing pubkey for the
per-tenant kid does not match the derivation (e.g. because the
master seed was changed), the subcommand refuses to overwrite and
exits non-zero.

### Procedure

```bash
# 1. Ensure the production gate is OFF in the dev session running
#    the ceremony (the dev seed is required to derive the pubkey).
unset ATLAS_PRODUCTION

# 2. Read the existing bundle, run rotate, write atomically.
WS=alice
BUNDLE_FILE=data/${WS}/pubkey-bundle.json
TMP=${BUNDLE_FILE}.rotate.tmp

cargo run --release -p atlas-signer -- \
    rotate-pubkey-bundle --workspace "${WS}" \
    < "${BUNDLE_FILE}" \
    > "${TMP}"

# 3. Verify the new bundle is well-formed and contains the per-tenant kid.
jq '.keys | keys' "${TMP}"
# Expected output includes "atlas-anchor:alice" alongside the legacy kids.

# 4. Atomically replace.
mv "${TMP}" "${BUNDLE_FILE}"

# 5. Optionally re-derive the pinned bundle hash for the workspace.
cargo run --release -p atlas-signer -- bundle-hash < "${BUNDLE_FILE}"
```

### Atomic replace and concurrency

The signer reads stdin and writes stdout — it is intentionally
unaware of the bundle file path. **Atomic replace and inter-operator
concurrency are operator-side responsibilities:**

- Use `mv` (POSIX rename) on the same filesystem as the target —
  rename is atomic.
- Do not run two operators against the same workspace's bundle
  concurrently; the second `mv` would silently overwrite the first.
  If your deployment has multiple operators, serialise the ceremony
  via an out-of-band coordination mechanism (a per-workspace lock
  file, a deployment-pipeline mutex, or a documented runbook
  ordering).
- If the ceremony aborts mid-flight (signal, crash) the `.tmp` file
  is left behind. It is safe to delete — the source of truth is
  the original bundle. Re-run the ceremony from step 2.

### Post-rotation checks

- `atlas-verify-cli verify-trace <existing-trace> -k <new-bundle>` —
  legacy traces still verify against the rotated bundle (the
  legacy SPIFFE kids are preserved).
- `pnpm --filter atlas-mcp-server smoke` — end-to-end smoke writes
  events to a per-tenant workspace and verifies via the new bundle.

---

## 3. Anchor-chain rotation ceremony

V1.7 ships `atlas-signer rotate-chain --confirm <workspace>` for
anchor-chain rotation. The new genesis batch's `previous_head`
equals the old chain's final head, so an auditor with both the old
and new traces can verify continuity.

### When to run

- After a migration that invalidates the existing chain (rare; mostly
  a recovery path).
- Operator opt-in only — the chain extends append-only by default
  and rotation is not part of normal operation.

### Procedure

```bash
WS=alice
cargo run --release -p atlas-signer -- \
    rotate-chain --confirm "${WS}"
```

This produces a new genesis batch in
`data/${WS}/anchor-chain.jsonl` whose `previous_head` is the old
chain's final head. The old chain file becomes read-only history;
treat it as an audit artefact and back it up before subsequent
rotations.

The `--confirm` flag is required because rotation discards future
appends to the old chain — there is no rollback. Operators should
file a runbook ticket capturing the reason for the rotation before
running.

---

## 4. Workspace-id hygiene

`atlas-signer::keys::validate_workspace_id` restricts workspace_ids
to ASCII printable bytes (0x21..0x7E), forbids the `:` delimiter
(reserved for kid composition), and rejects empty strings. This
validation runs on every V1.9 per-tenant subcommand at the issuer
ingress.

The verifier is intentionally lenient — the trust property holds for
any UTF-8 string via byte-equal kid compare against the recomputed
`format!("atlas-anchor:{trace.workspace_id}")`. Tightening at the
issuer side is hygiene (avoids ambiguous IDs in observability tools,
filenames, log lines), not a trust boundary.

If you need a workspace_id that contains `:` or non-ASCII characters
(e.g. for compatibility with an external workspace naming scheme),
do not weaken `validate_workspace_id`. Instead, map the external
name to an ASCII identifier at the MCP-server boundary and keep the
mapping in a separate registry.

---

## 5. Recovery scenarios

### Lost or corrupted pubkey bundle

The bundle is regenerable from the master seed plus the workspace
roster. If `data/{workspace}/pubkey-bundle.json` is lost:

1. Restart from a known-good source: a backup, a stored trace bundle
   that included the bundle alongside the trace, or a bundle the
   verifier accepted at any prior point.
2. If no backup exists, run `rotate-pubkey-bundle` on each active
   workspace against an empty starting bundle. The HKDF-derived
   per-tenant pubkeys are deterministic (master seed + workspace_id),
   so the regenerated bundle is byte-identical to the lost one *for
   the per-tenant kids*. Legacy SPIFFE kids must be re-supplied
   from `apps/atlas-mcp-server/src/lib/keys.ts` (the V1 dev fixture).
3. Recompute `pubkey_bundle_hash` and confirm any in-flight traces
   still claim the same hash. If the recomputed hash does not match
   what a trace claims, the trace was generated against a different
   bundle — investigate before publishing.

### Lost or corrupted anchor-chain.jsonl

The chain is **not** regenerable from other state — it is the
authoritative monotonicity witness. If lost, the workspace's chain
guarantee is broken from the loss-point forward.

1. Stop new anchor issuance for the affected workspace.
2. Restore from backup if available. The chain is append-only, so
   any backup with the largest `batch_index` is correct.
3. If no backup exists, run `rotate-chain --confirm <workspace>` to
   start a fresh chain from a new genesis. Existing traces with old
   chain references will fail strict-mode coverage; document the
   loss in the audit log so future auditors understand the gap.

### Master seed compromise (V1.9 limitation)

V1.9 derives all per-tenant keys from a single master seed. A seed
compromise is full compromise — every workspace's signing key is
forgeable. There is no per-tenant recovery path in V1.9. Until
V1.10 ships HSM/TPM sealing:

1. Treat seed exposure as a security incident.
2. Rotate to a new seed (this requires a new bundle for every
   workspace and invalidates all existing per-tenant signatures —
   coordinate with downstream auditors).
3. Document the compromise window in the audit log.

This is the residual risk
[SECURITY-NOTES.md](SECURITY-NOTES.md) documents under "Master
seed residual risk". Operators should plan their backup, key-storage,
and rotation strategy with this limitation in mind.

---

## 6. Quick reference

| Ceremony | Command | Idempotent | Atomic-replace required |
|---|---|---|---|
| Migrate workspace bundle to V1.9 | `atlas-signer rotate-pubkey-bundle --workspace <ws>` | yes | yes (operator-side `mv`) |
| Rotate anchor chain | `atlas-signer rotate-chain --confirm <workspace>` | no | yes (operator-side) |
| Derive workspace pubkey for inspection | `atlas-signer derive-pubkey --workspace <ws>` | yes | n/a (read-only) |
| Production-gate enforcement | set `ATLAS_PRODUCTION=1` | n/a | n/a |

See [ARCHITECTURE.md §7.3](ARCHITECTURE.md) for the per-tenant key
trust model and [SECURITY-NOTES.md](SECURITY-NOTES.md) for the full
threat model.
