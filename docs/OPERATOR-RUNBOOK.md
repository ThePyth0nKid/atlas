# Atlas — Operator Runbook (V1.10)

This document is the operator's reference for Atlas ceremonies that
sit *outside* the agent-driven hot path: workspace key migration,
chain rotation, bundle assembly, and the master-seed gate.

It is written for operators of an Atlas deployment, not for
engineers building Atlas itself nor for auditors verifying a trace
bundle. For those audiences see
[ARCHITECTURE.md](ARCHITECTURE.md) and
[SECURITY-NOTES.md](SECURITY-NOTES.md).

---

## 1. Master-seed gate

Atlas derives per-tenant Ed25519 anchoring keys via HKDF-SHA256 from
a single 32-byte master seed. V1.9 hard-coded a `DEV_MASTER_SEED`
constant in `atlas-signer/src/keys.rs` (source-committed, public,
**not** a production secret). V1.10 introduces the
[`MasterSeedHkdf`](../crates/atlas-signer/src/keys.rs) trait and
inverts the gate semantics so the dev seed is now a positive
opt-in.

### V1.10 — current behaviour (LIVE)

The gate is **positive opt-in**: the dev seed signs only when the
operator has explicitly authorised it. Per-tenant subcommands hit
the gate first; if the gate refuses, the binary exits with code 2
and prints an actionable error to stderr.

The gate has two layered checks (defence-in-depth):

1. **V1.9 paranoia layer.** If `ATLAS_PRODUCTION=1` is set, the gate
   refuses *unconditionally* — even if `ATLAS_DEV_MASTER_SEED=1` is
   also set. An operator who explicitly says "this is production"
   overrides every dev-opt-in. This preserves V1.9 deployment
   safety habits.
2. **V1.10 positive opt-in.** `ATLAS_DEV_MASTER_SEED` must be a
   recognised truthy value (`1`, `true`, `yes`, `on`, ASCII case-
   insensitive, leading/trailing whitespace tolerated). Anything
   else — including unset, empty, `0`, `false`, `no`, `off`, or any
   typo — refuses.

| `ATLAS_PRODUCTION`        | `ATLAS_DEV_MASTER_SEED` | Gate                       |
|---------------------------|-------------------------|----------------------------|
| unset                     | unset/empty             | **refuses** (V1.10 default)|
| unset                     | `1` / `true` / `yes` / `on` | **allows** dev seed   |
| unset                     | `0` / `false` / typo    | **refuses**                |
| `1`                       | anything                | **refuses** (V1.9 paranoia)|

Per-tenant subcommands subject to the gate:

- `atlas-signer derive-key --workspace <ws>`
- `atlas-signer derive-pubkey --workspace <ws>`
- `atlas-signer rotate-pubkey-bundle --workspace <ws>`
- `atlas-signer sign --derive-from-workspace <ws>`

> **Why a strict allow-list of truthy values?**
>
> V1.9 had a documented operator footgun: only the literal byte
> string `"1"` tripped `ATLAS_PRODUCTION`, so `=true`/`=yes`/`=on`
> silently fell through and the dev seed signed. V1.10's positive
> opt-in inverts both the polarity AND the allow-list logic so the
> safe direction is the default direction. A typo on opt-in still
> refuses — instead of silently allowing a dev seed in a context
> that meant to refuse it.
>
> The strict allow-list pins this behaviour in code; see
> `master_seed_gate_refuses_typos_and_unknown_values` in
> `crates/atlas-signer/src/keys.rs`.

### V1.10 — production status (LIVE)

V1.10 wave 2 ships the PKCS#11 sealed-seed loader **in-tree** at
`crates/atlas-signer/src/hsm/`, gated behind the `hsm` Cargo feature.
(Earlier V1.10 design notes spoke of a sibling `atlas-signer-hsm`
crate; the loader was folded back into `atlas-signer` because Cargo's
no-cycles rule prevented the binary from dispatching to a sibling
that itself depended on `atlas-signer` for the `MasterSeedHkdf`
trait. Same operator opt-in semantics, no Cargo cycle.)

The loader adds a third layer above the V1.9 paranoia + V1.10
positive-opt-in gates documented above:

3. **Sealed-seed lookup (HSM trio first).** When the HSM env trio is
   set — `ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`
   — the loader opens the PKCS#11 module **first** and uses
   [`Pkcs11MasterSeedHkdf`](../crates/atlas-signer/src/hsm/pkcs11.rs)
   for every per-tenant derive. The seed bytes never enter Atlas
   address space; HKDF-SHA256 runs **inside** the HSM via
   `CKM_HKDF_DERIVE`. HSM open / derive failure is **fatal** — there
   is no silent fallback to the dev seed. Layers 1 and 2 are not
   consulted in this mode.

   Partial trios (one or two of the three set) refuse with an
   actionable error before any signing attempt — partial config is
   the most common operator footgun and silent fallback would defeat
   the audit signal.

| `ATLAS_HSM_*` trio | `ATLAS_PRODUCTION` | `ATLAS_DEV_MASTER_SEED` | Loader                      |
|--------------------|--------------------|-------------------------|-----------------------------|
| all three set      | any                | any                     | **PKCS#11 sealed seed**     |
| partial (1 or 2)   | any                | any                     | **refuses** (config error)  |
| none set           | unset              | `1`/`true`/`yes`/`on`   | dev seed (positive opt-in)  |
| none set           | `1`                | any                     | **refuses** (V1.9 paranoia) |
| none set           | unset              | unset / typo            | **refuses** (V1.10 default) |

> **Why HSM-first dispatch?** An operator who has set up a sealed
> deploy expects the loader to use it. Falling through to the dev
> seed when HSM init fails would be the silent-fallback class V1.10
> closes. Sealed-seed init failure is fatal: the operator must fix
> the HSM config (or unset the trio) — `atlas-signer` will not
> silently sign with a dev key after seeing an HSM trio in env.

### Configuring dev/CI environments

```bash
# Dev or CI: explicitly opt in to the dev master seed.
export ATLAS_DEV_MASTER_SEED=1

# Recognised truthy values (case-insensitive): 1, true, yes, on.
# All four are equivalent.

# atlas-signer per-tenant subcommands now run against DEV_MASTER_SEED.
cargo run --release -p atlas-signer -- derive-key --workspace alice
```

### Configuring production deployments

Production deployments configure the HSM trio AND build with the
`hsm` feature. Without the feature flag, the PKCS#11 backend is the
fail-closed stub and refuses every derive call.

```bash
# 1. Build atlas-signer with the PKCS#11 backend present.
cargo build --release -p atlas-signer --features hsm

# 2. Declare this is production (V1.9 paranoia layer; redundant
#    when the HSM trio is set, but defence-in-depth costs nothing).
export ATLAS_PRODUCTION=1

# 3. Configure the HSM trio. All three required — partial config
#    refuses to start with an actionable error.
export ATLAS_HSM_PKCS11_LIB=/usr/lib/softhsm/libsofthsm2.so   # absolute path to the PKCS#11 module
export ATLAS_HSM_SLOT=0                                        # token slot number (decimal)
export ATLAS_HSM_PIN_FILE=/var/run/atlas/hsm.pin               # path to user PIN file (mode 0400)

# 4. Per-tenant subcommands now derive against the sealed master
#    seed inside the HSM. No seed material ever crosses into Atlas
#    address space; HKDF-SHA256 runs inside the device.
atlas-signer derive-pubkey --workspace alice
```

Master-seed import is a one-time ceremony — see §2 below before
running any of the per-tenant subcommands against a fresh HSM
token.

### Migrating from V1.9 to V1.10

| You used to run...                    | V1.10 equivalent                                   |
|---------------------------------------|----------------------------------------------------|
| dev/CI: env unset                     | `export ATLAS_DEV_MASTER_SEED=1`                   |
| production: `ATLAS_PRODUCTION=1` only | configure the HSM trio (§2) + build `--features hsm` |
| dev: `ATLAS_PRODUCTION=true` (typo'd) | replace with `export ATLAS_DEV_MASTER_SEED=1`      |

V1.9 dev/CI environments that ran with `ATLAS_PRODUCTION` unset will
now refuse per-tenant subcommands at the gate. The migration is a
one-line addition to the deploy script. The error message points at
this runbook section.

V1.9 production environments that relied on `ATLAS_PRODUCTION=1` to
refuse the dev seed are still safe — that gate fires unchanged.
What V1.10 wave 2 adds is the *positive* path: with the HSM trio
configured the binary signs against a sealed master seed, so
production deploys are no longer blocked at the gate.

### Failure-mode reference

The gate emits a deterministic error message to stderr on refusal,
with exit code 2. Sample messages:

* **Default refuse (no HSM trio, no dev opt-in):**
  ```
  derive-key: ATLAS_DEV_MASTER_SEED not set to a recognised truthy
  value (got ""). V1.10 inverts the V1.9 gate: the source-committed
  DEV_MASTER_SEED now requires positive opt-in. Set
  ATLAS_DEV_MASTER_SEED=1 (or true/yes/on, case-insensitive) in
  dev/CI environments. Production should use the V1.10 wave-2
  sealed-seed loader: configure the env trio (ATLAS_HSM_PKCS11_LIB,
  ATLAS_HSM_SLOT, ATLAS_HSM_PIN_FILE) and rebuild with --features
  hsm. See docs/OPERATOR-RUNBOOK.md §1 for the V1.9→V1.10
  migration and the HSM import ceremony.
  ```
* **V1.9 paranoia override (HSM trio absent, ATLAS_PRODUCTION=1):**
  ```
  derive-key: ATLAS_PRODUCTION=1 set, but atlas-signer is using the
  source-committed DEV_MASTER_SEED. Refusing to derive per-tenant
  keys against a public dev seed. V1.10 wave 2 ships a sealed-seed
  loader at crate::hsm — configure the env trio (ATLAS_HSM_PKCS11_LIB,
  ATLAS_HSM_SLOT, ATLAS_HSM_PIN_FILE) and rebuild with --features
  hsm. See docs/OPERATOR-RUNBOOK.md §1 for the import ceremony.
  ```

Operators reading these messages should grep for the relevant env
var name and either set/unset it correctly or escalate the runbook
section to engineering for a sealed-seed-loader configuration.

### Trust property

Setting `ATLAS_DEV_MASTER_SEED=1` is your declaration that you
understand the dev seed is source-committed (anyone with repo access
can re-derive every workspace's key). Treat the env var as a
deployment-time audit signal: every host that has it set is, by
construction, a host that should not be holding production-tenant
material.

In HSM mode (V1.10 wave 2), the equivalent audit signal is the HSM
trio. A host snapshot showing all three of `ATLAS_HSM_PKCS11_LIB`,
`ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE` set, AND no
`ATLAS_DEV_MASTER_SEED` set, AND the binary built with
`--features hsm`, is the deployment-time signature of a
sealed-seed deployment.

---

## 2. HSM master-seed import ceremony (V1.10 wave 2)

The PKCS#11 backend looks up the master seed by `CKA_LABEL`
(`atlas-master-seed-v1`) inside the configured token. The seed must
be imported once per token, before the first call to any per-tenant
subcommand. The ceremony is **destructive of the source seed
material** — once the seed is sealed inside the HSM, the host-side
copy MUST be wiped.

### What gets imported

A 32-byte secret as a PKCS#11 generic-secret object:

| Attribute | Value | Why |
|---|---|---|
| `CKA_CLASS` | `CKO_SECRET_KEY` | generic secret |
| `CKA_KEY_TYPE` | `CKK_GENERIC_SECRET` | enables `CKM_HKDF_DERIVE` |
| `CKA_TOKEN` | `true` | persists across sessions |
| `CKA_LABEL` | `atlas-master-seed-v1` | hard-coded loader lookup |
| `CKA_VALUE_LEN` | `32` | matches HKDF-SHA256 input |
| `CKA_DERIVE` | `true` | required for HKDF-Derive |
| `CKA_EXTRACTABLE` | `false` | seed never leaves HSM |
| `CKA_SENSITIVE` | `true` | refuse plaintext export |
| `CKA_PRIVATE` | `true` | requires authenticated session |
| `CKA_MODIFIABLE` | `false` | freeze attributes post-import |

The `CKA_LABEL` is fixed in code at
[`MASTER_SEED_LABEL`](../crates/atlas-signer/src/hsm/config.rs)
and not configurable — multi-master-seed deployments are an
explicit non-goal. Operators with multiple tokens use distinct
slots, not distinct labels.

### Procedure (SoftHSM2 example)

SoftHSM2 is the open-source PKCS#11 implementation used in CI;
production HSMs (Thales Luna, AWS CloudHSM, YubiHSM2, Nitrokey HSM 2)
support the same `pkcs11-tool` flow with vendor-specific module paths.

**Operator caution before you start.** Every bash snippet below reads
PINs and seed bytes from environment variables that you populate from a
secrets manager (Vault, AWS Secrets Manager, GCP Secret Manager,
1Password CLI, `pass`, etc.). **Do not paste literal PINs into the
shell.** A literal PIN on a command line:

  * shows up in `/proc/<pid>/cmdline`, visible to any process the user
    can `ps`-list,
  * lands in shell history (`~/.bash_history`, `~/.zsh_history`),
  * may be captured by terminal session-recording tools (asciinema,
    `script(1)`, IDE-embedded terminals).

The shell variables `SO_PIN`, `USER_PIN`, and `SEED_FILE` here are
populated *exactly once* at the start of the session, exported into the
restricted-permission environment of the import shell, and unset at the
end. The PIN file written in step 5 is the only persistent surface.

```bash
# 0. Pre-flight: install SoftHSM2 + opensc (provides pkcs11-tool).
#    On Debian/Ubuntu: apt-get install softhsm2 opensc
#    On Fedora:        dnf install softhsm opensc

# 0a. Tighten the umask BEFORE anything writes a file. This is what
#     guarantees `mktemp` (step 2) creates the seed file mode 0600 on
#     any sysv/glibc/musl combination — relying on the libc default
#     can produce 0644 on some BSDs and is brittle. A subsequent
#     `chmod 600` would have a race window between create and chmod.
umask 077

# 0b. Pull the SO PIN and user PIN from your secrets manager into env
#     vars used ONLY by this shell. Replace the bracketed commands
#     with your manager's CLI. The variable form keeps the PINs out
#     of `ps`/shell-history/session-recording (see caution above).
SO_PIN="$(your-secrets-manager get atlas/softhsm/so-pin)"
USER_PIN="$(your-secrets-manager get atlas/softhsm/user-pin)"

# 1. Initialise a token in slot 0. The SO PIN protects the token
#    object roster; the user PIN is what atlas-signer authenticates
#    with at startup. softhsm2-util reads the PINs from argv, which
#    is why we sourced them from a secrets manager rather than typing
#    them inline — the *value* is now in env, not in shell history.
softhsm2-util --init-token --slot 0 \
              --label atlas-prod \
              --so-pin "${SO_PIN}" \
              --pin    "${USER_PIN}"

# 2. Generate a fresh 32-byte seed in a tempfile that the umask in
#    step 0a forces to mode 0600. No `chmod` step required — and
#    therefore no race window where the file is briefly world-readable.
#    Treat this byte stream like the master key of any HSM-backed
#    system: never let it touch durable storage outside the HSM.
SEED_FILE="$(mktemp)"
head -c 32 /dev/urandom > "${SEED_FILE}"

# 3. Import the seed under the canonical label. The `--type secrkey`
#    + `--key-type GENERIC:32` selection produces the
#    CKO_SECRET_KEY / CKK_GENERIC_SECRET combination atlas-signer
#    expects. `--pin "${USER_PIN}"` reads from the env var rather
#    than a literal. SoftHSM2's pkcs11-tool sets CKA_DERIVE=true by
#    default on imported secret keys; verify with
#    `pkcs11-tool ... --read-object` if your vendor differs.
pkcs11-tool --module /usr/lib/softhsm/libsofthsm2.so \
            --slot 0 \
            --login --pin "${USER_PIN}" \
            --write-object "${SEED_FILE}" \
            --type secrkey \
            --key-type GENERIC:32 \
            --label atlas-master-seed-v1 \
            --usage-derive

# 4. WIPE THE HOST-SIDE COPY. This is the destructive step — once
#    the host file is gone, the seed exists only inside the HSM.
shred -u "${SEED_FILE}"   # or `rm -P` on macOS / `cipher /w` on Windows
unset SEED_FILE

# 5. Stage the user PIN as a 0400 file for atlas-signer. We use
#    `install -m 0400` (atomic create-with-mode) so there is no race
#    between create-as-0644 and chmod-to-0400. The PIN is piped from
#    the env var so it never appears in `ps`/history (printf is a
#    bash builtin, so even its argv stays out of /proc).
PIN_FILE=/var/run/atlas/hsm.pin
install -m 0400 -o atlas -g atlas /dev/stdin "${PIN_FILE}" \
    < <(printf '%s' "${USER_PIN}")

# 5a. Wipe the in-memory PIN from this shell now that the file is on
#     disk. Anything else in this session that needed the PIN should
#     have already finished above.
unset SO_PIN USER_PIN

# 6. Configure the trio + start the signer.
export ATLAS_HSM_PKCS11_LIB=/usr/lib/softhsm/libsofthsm2.so
export ATLAS_HSM_SLOT=0
export ATLAS_HSM_PIN_FILE="${PIN_FILE}"
export ATLAS_PRODUCTION=1   # belt-and-braces; HSM-mode does not need it but defence-in-depth costs nothing

# 7. Smoke-test by deriving a workspace pubkey. The pubkey is
#    deterministic for a given (sealed-seed, workspace_id) pair;
#    record it as the pinned-pubkey golden for this deployment.
atlas-signer derive-pubkey --workspace alice
```

### Verifying the import

The post-import verification has **two** stages: presence (the seed
exists with the right label) and attribute-correctness (the seed has
the security attributes the loader assumes). Stage 2 was added in
V1.11 to close a silent-failure mode — an HSM whose `pkcs11-tool`
ignored `--usage-derive` or whose vendor defaults set
`CKA_EXTRACTABLE=true` would pass stage 1 and the `derive-pubkey`
smoke without ever signalling that the seed was leakable via
attribute readback.

```bash
# Stage 1 — presence. Should show one object with label
# "atlas-master-seed-v1" and CKA_DERIVE=true.
pkcs11-tool --module "${ATLAS_HSM_PKCS11_LIB}" \
            --slot "${ATLAS_HSM_SLOT}" \
            --login --pin-source "file:${ATLAS_HSM_PIN_FILE}" \
            --list-objects --type secrkey

# Stage 2 — attribute readback. The default `--list-objects` output
# already prints the security attributes (`Access: ...`); confirm
# all three of the following appear for the `atlas-master-seed-v1`
# object. A vendor whose pkcs11-tool emits a different format MUST
# surface the same three attributes via the vendor's attribute-dump
# tool — DO NOT skip this stage on the assumption that the smoke is
# sufficient.
#
#   Expected attributes for `atlas-master-seed-v1`:
#     Access:     sensitive, always sensitive, never extractable
#                 ╰──┬───────╯  ╰─────┬──────╯  ╰──────┬─────────╯
#                    │                │                │
#                    │                │                └─ CKA_EXTRACTABLE: no
#                    │                └─ historical guarantee: was always sensitive
#                    │                   (a token that ever set CKA_SENSITIVE=false
#                    │                   on this object would be reported here as
#                    │                   "not always sensitive")
#                    └─ CKA_SENSITIVE: yes
#     Usage:      derive
#                 └─ CKA_DERIVE: yes (the HKDF backend depends on this)
#
# If ANY of `sensitive` / `never extractable` / `derive` is missing,
# the import is not safe to use — the seed may be readable by an
# attacker with PIN access, defeating the V1.10 wave 2 trust model.
# Re-run §2 from step 1 with the correct `--usage-derive` flag (and
# investigate why the vendor module ignored the import attribute set
# — most commonly an out-of-date pkcs11-tool against a newer HSM
# firmware).

# Stage 3 — derive smoke. Verify atlas-signer can open the session
# and derive. Failure here is fatal — the binary refuses to fall
# through to the dev seed.
atlas-signer derive-pubkey --workspace canary-import-test
```

### Backup, recovery, rotation

- **Backup.** PKCS#11 does not have a portable export format for
  non-extractable keys. Operators wanting cross-token redundancy
  must either (a) provision multiple tokens that each receive a
  copy of the seed during the import ceremony — wipe the host-side
  copy only after every target token has read it — or (b) accept
  that a single-token loss invalidates every per-tenant key
  derived from that seed. Option (a) is the operationally sane
  default; document the active-token roster in the deployment
  manifest so an auditor can verify the redundancy claim.
- **Recovery from token loss.** Equivalent to "master seed
  compromise" in §6 — every per-tenant pubkey changes, every
  bundle hash changes, and every historical trace becomes
  unverifiable against the new bundle. Mitigated by (a) above.
- **Rotation.** Generate a new seed, import under a *new* label
  (`atlas-master-seed-v2`), and ship a coordinated `atlas-signer`
  update that flips `MASTER_SEED_LABEL`. The old label remains
  accessible for verifying historical traces; the new label
  signs new traffic. Schedule rotations far apart — every
  rotation invalidates every existing `pubkey_bundle_hash`.

### Threat model — what HSM sealing does and does not protect

**Does protect:**

- Memory-disclosure attacks on the signer host (heap dumps, swap,
  core dumps, debugger attachments) cannot exfiltrate the master
  seed. HKDF-SHA256 runs **inside** the HSM via `CKM_HKDF_DERIVE`;
  the derived 32-byte HKDF output is read out as a generic-secret
  object and used to construct an Ed25519 signing key in process
  memory, then zeroized.
- Filesystem-level snapshot of the signer host does not contain
  the seed. The PIN file does, but PIN file access without PKCS#11
  module access yields nothing — the seed is not derivable from
  the PIN alone.
- Source-code disclosure does not yield the seed (V1.10 fix to the
  V1.9 residual risk that the dev seed was source-committed).

**Does not protect:**

- HSM physical compromise. An attacker with physical possession of
  the token AND the user PIN can derive every per-tenant key.
  Hence the recommendation to keep the PIN file in tmpfs (cleared
  on reboot) and the SO PIN in a separate secret manager.
- Malicious code running inside the signer process. The PKCS#11
  session is held open for the lifetime of the binary; an attacker
  who achieves code execution **inside** atlas-signer can call
  `derive_for` arbitrarily during that session's lifetime. Mitigated
  by short-lived signer invocations + per-event subprocess spawn
  (the V1.5+ design choice: the MCP server shells to atlas-signer
  per event, so no long-lived signer process is exposed).
- TOCTOU on the PKCS#11 module path. V1.10 wave 2 refuses any
  relative `ATLAS_HSM_PKCS11_LIB` value at config-parse time, closing
  the CWD-relative library-hijack vector (an attacker who controls
  the signer's working directory can no longer plant a `.so` next to
  the binary and have the loader pick it up). A residual TOCTOU
  window remains: between the config-parse check and `Pkcs11::new`'s
  `dlopen(3)` call, filesystem state at the absolute path is not
  held — an attacker with write access to that path (or to the
  containing directory, via `rename(2)`) can swap the file and the
  loader will dlopen the malicious shared library with full access
  to the master-seed session. Closing this fully requires kernel
  mechanisms (mount namespaces, sealed FDs, IMA/EVM measured-load,
  `noexec` + `O_VERIFY` on the filesystem) outside the reach of
  userspace V1.10. **Filesystem ACLs on `${ATLAS_HSM_PKCS11_LIB}`
  AND its containing directory are therefore a *required*
  operational control, not a nice-to-have.** The module file MUST
  be owned by a privileged identity (root, or a vendor-package
  user), writable only by that identity (mode `0755` typical, never
  `0777` / group-writable / world-writable), and the signer's
  runtime user MUST hold read-only access. The same applies
  recursively to every parent directory up to a known-immutable
  mount root — an attacker who can rename a parent directory can
  swap a vendor `libsofthsm2.so` for their own without touching
  the leaf file. On systemd hosts, pair this with a unit-level
  `ProtectSystem=strict` + `ReadOnlyPaths=` declaration covering
  the module path so even root inside the signer's namespace
  cannot mutate it.
- HSM driver compromise. The PKCS#11 module runs in atlas-signer's
  address space and has full access to the session. Vendor module
  signing + the filesystem ACL controls described in the TOCTOU
  bullet above are the operational defences; there is no in-process
  sandbox between cryptoki and the rest of the signer.

### Test mode

CI exercises the full HSM path against SoftHSM2. The smoke harness
honours `ATLAS_TEST_HSM=1` to opt in (default off, for CI hosts
without SoftHSM2 installed). When enabled, the smoke produces the
same pinned bundle hashes as dev-seed mode iff the imported seed
equals `DEV_MASTER_SEED` — operators verifying the import flow can
use this to confirm byte-equivalent derivation before rotating to a
production seed.

---

## 3. Workspace pubkey-bundle rotation ceremony

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

## 4. Anchor-chain rotation ceremony

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

## 5. Workspace-id hygiene

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

## 6. Recovery scenarios

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

### Master seed compromise

Every per-tenant key is derived from a single master seed via
HKDF-SHA256. Compromise of the master seed is full compromise —
every workspace's signing key is forgeable. There is no per-tenant
recovery path; the only recovery is full seed rotation, which
invalidates every historical per-tenant pubkey and every claimed
`pubkey_bundle_hash`.

**V1.10 wave 2 changes the threat model:** with the HSM trio
configured, the master seed lives only inside the PKCS#11 token.
Memory disclosure on the signer host, source-code disclosure, and
filesystem snapshots no longer yield the seed. The residual risks
are the ones §2 "Threat model" enumerates: HSM physical compromise,
malicious code inside the signer, HSM driver compromise.

**For dev/CI deployments running with `ATLAS_DEV_MASTER_SEED=1`,**
the V1.9 model still applies: the dev seed is source-committed and
anyone with repo access can re-derive every workspace's key. Treat
dev/CI environments accordingly — never point production tenant
material at them.

If a seed compromise (HSM token loss + PIN, or dev-seed exposure)
does occur:

1. Treat seed exposure as a security incident.
2. Rotate to a new seed (HSM mode: import a fresh secret under
   `atlas-master-seed-v2` per §2's rotation procedure; dev mode:
   change `DEV_MASTER_SEED` and rebuild). Either path requires a
   new bundle for every workspace and invalidates all existing
   per-tenant signatures — coordinate with downstream auditors.
3. Document the compromise window in the audit log.

[SECURITY-NOTES.md](SECURITY-NOTES.md) documents the residual risk
in detail. Operators should plan their backup, key-storage, and
rotation strategy with the HSM threat model in mind.

---

## 7. Quick reference

| Ceremony | Command | Idempotent | Atomic-replace required |
|---|---|---|---|
| Migrate workspace bundle to V1.9 | `atlas-signer rotate-pubkey-bundle --workspace <ws>` | yes | yes (operator-side `mv`) |
| Rotate anchor chain | `atlas-signer rotate-chain --confirm <workspace>` | no | yes (operator-side) |
| Derive workspace pubkey for inspection | `atlas-signer derive-pubkey --workspace <ws>` | yes | n/a (read-only) |
| Production-gate enforcement | set `ATLAS_PRODUCTION=1` | n/a | n/a |

See [ARCHITECTURE.md §7.3](ARCHITECTURE.md) for the per-tenant key
trust model and [SECURITY-NOTES.md](SECURITY-NOTES.md) for the full
threat model.
