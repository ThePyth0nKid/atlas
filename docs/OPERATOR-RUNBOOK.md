# Atlas — Operator Runbook (V1.13)

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

### V1.10 — current behaviour (LIVE; V1.12-simplified)

The gate is **positive opt-in**: the dev seed signs only when the
operator has explicitly authorised it. Per-tenant subcommands hit
the gate first; if the gate refuses, the binary exits with code 2
and prints an actionable error to stderr.

**Single check (V1.12).** `ATLAS_DEV_MASTER_SEED` must be a
recognised truthy value (`1`, `true`, `yes`, `on`, ASCII case-
insensitive, leading/trailing whitespace tolerated). Anything else
— including unset, empty, `0`, `false`, `no`, `off`, or any typo —
refuses.

| `ATLAS_DEV_MASTER_SEED`        | Gate                          |
|--------------------------------|-------------------------------|
| unset/empty                    | **refuses** (V1.10 default)   |
| `1` / `true` / `yes` / `on`    | **allows** dev seed           |
| `0` / `false` / typo / other   | **refuses**                   |

Per-tenant subcommands subject to the gate:

- `atlas-signer derive-key --workspace <ws>`
- `atlas-signer derive-pubkey --workspace <ws>`
- `atlas-signer rotate-pubkey-bundle --workspace <ws>`
- `atlas-signer sign --derive-from-workspace <ws>`

> **V1.12 removal of the V1.9 `ATLAS_PRODUCTION` paranoia layer.**
> V1.9–V1.11 ran a defence-in-depth `ATLAS_PRODUCTION=1` paranoia
> gate ahead of the positive opt-in: setting it refused the dev
> seed regardless of `ATLAS_DEV_MASTER_SEED`. V1.12 removes that
> layer entirely. Three reasons:
>
> 1. The literal-`"1"`-only recognition was a documented operator
>    footgun (`=true`/`=yes`/`=on` silently fell through).
> 2. The V1.10 positive opt-in covers the same security property
>    (refuse-by-default, refuse-on-typo) without the footgun.
> 3. The wave-2 HSM trio (`ATLAS_HSM_PKCS11_LIB` /
>    `ATLAS_HSM_SLOT` / `ATLAS_HSM_PIN_FILE`) is now the production
>    audit signal: `env | grep ATLAS_` shows whether the deploy is
>    sealed-seed or dev.
>
> `ATLAS_PRODUCTION` is silently ignored from V1.12 onwards. The
> V1.11 deprecation warning has been removed alongside the gate.

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

The loader adds a sealed-seed lookup that takes precedence over the
V1.10 positive-opt-in gate documented above:

2. **Sealed-seed lookup (HSM trio first).** When the HSM env trio is
   set — `ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`
   — the loader opens the PKCS#11 module **first** and uses
   [`Pkcs11MasterSeedHkdf`](../crates/atlas-signer/src/hsm/pkcs11.rs)
   for every per-tenant derive. The seed bytes never enter Atlas
   address space; HKDF-SHA256 runs **inside** the HSM via
   `CKM_HKDF_DERIVE`. HSM open / derive failure is **fatal** — there
   is no silent fallback to the dev seed. The dev gate is not
   consulted in this mode.

   Partial trios (one or two of the three set) refuse with an
   actionable error before any signing attempt — partial config is
   the most common operator footgun and silent fallback would defeat
   the audit signal.

| `ATLAS_HSM_*` trio | `ATLAS_DEV_MASTER_SEED` | Loader                      |
|--------------------|-------------------------|-----------------------------|
| all three set      | any                     | **PKCS#11 sealed seed**     |
| partial (1 or 2)   | any                     | **refuses** (config error)  |
| none set           | `1`/`true`/`yes`/`on`   | dev seed (positive opt-in)  |
| none set           | unset / typo            | **refuses** (V1.10 default) |

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

# 2. Configure the HSM trio. All three required — partial config
#    refuses to start with an actionable error. The trio is the
#    V1.12 production audit signal: an auditor reading
#    `env | grep ATLAS_` sees these three vars and knows the
#    deployment is sealed-seed (V1.12 removed the V1.9-era
#    `ATLAS_PRODUCTION=1` paranoia var; setting it has no effect).
export ATLAS_HSM_PKCS11_LIB=/usr/lib/softhsm/libsofthsm2.so   # absolute path to the PKCS#11 module
export ATLAS_HSM_SLOT=0                                        # token slot number (decimal)
export ATLAS_HSM_PIN_FILE=/var/run/atlas/hsm.pin               # path to user PIN file (mode 0400)

# 3. Per-tenant subcommands now derive against the sealed master
#    seed inside the HSM. No seed material ever crosses into Atlas
#    address space; HKDF-SHA256 runs inside the device.
atlas-signer derive-pubkey --workspace alice
```

Master-seed import is a one-time ceremony — see §2 below before
running any of the per-tenant subcommands against a fresh HSM
token.

### Migrating from V1.9 to V1.10 (and V1.11/V1.12 follow-ups)

| You used to run...                    | V1.12 equivalent                                   |
|---------------------------------------|----------------------------------------------------|
| dev/CI: env unset                     | `export ATLAS_DEV_MASTER_SEED=1`                   |
| production: `ATLAS_PRODUCTION=1` only | configure the HSM trio (§2) + build `--features hsm` |
| dev: `ATLAS_PRODUCTION=true` (typo'd) | replace with `export ATLAS_DEV_MASTER_SEED=1`      |
| any env still setting `ATLAS_PRODUCTION` | unset it — V1.12 silently ignores the var       |

V1.9 dev/CI environments that ran with `ATLAS_PRODUCTION` unset will
now refuse per-tenant subcommands at the gate. The migration is a
one-line addition to the deploy script. The error message points at
this runbook section.

V1.9 production environments that relied on `ATLAS_PRODUCTION=1` to
refuse the dev seed: V1.12 removed that gate. The replacement
production audit signal is the wave-2 HSM trio
(`ATLAS_HSM_PKCS11_LIB` / `ATLAS_HSM_SLOT` / `ATLAS_HSM_PIN_FILE`)
plus the absence of `ATLAS_DEV_MASTER_SEED`. With the HSM trio
configured the binary signs against a sealed master seed and the
dev gate is unreachable. Setting `ATLAS_PRODUCTION` from V1.12
onwards has no effect — neither refusing the dev seed nor emitting
the V1.11 deprecation warning (which was also removed in V1.12).

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
> **Removed in V1.12: V1.9 paranoia override message.**
> V1.9–V1.11 also emitted a refusal message keyed off
> `ATLAS_PRODUCTION=1` ("set, but atlas-signer is using the
> source-committed DEV_MASTER_SEED"). V1.12 removed the gate that
> produced it. If your monitoring greps for that text, switch the
> match to the V1.10 default-refuse message above (the migration
> path is identical: configure the HSM trio).

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
#    with at startup.
#
#    softhsm2-util's init-token subcommand reads PINs from argv —
#    sourcing the values from a secrets manager keeps them out of
#    shell history, BUT the expanded values still land in
#    /proc/<pid>/cmdline of the softhsm2-util process for the
#    duration of the call. This is unavoidable with current
#    softhsm2-util (no stdin/env PIN-input for init-token); the
#    operational defences are:
#      (a) run this ceremony on a single-tenant hardened host,
#          NOT a multi-user system,
#      (b) mount /proc with `hidepid=2,gid=<adm>` so non-owners
#          cannot enumerate cmdlines (Linux ≥ 3.3),
#      (c) accept that the exposure window is bounded — this is a
#          one-shot init step, not a hot-path operation.
#    The pkcs11-tool import in step 3 below reads the PIN from the
#    environment (--pin-source env:VAR) so it does NOT have this
#    exposure; the limitation is specifically softhsm2-util.
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
#    expects. The `--pin-source env:USER_PIN` form tells pkcs11-tool
#    to read the PIN from the named environment variable rather
#    than from argv — the literal PIN bytes therefore do NOT appear
#    in /proc/<pid>/cmdline. (Compare step 1's softhsm2-util, which
#    has no env-var input for init-token; this is why pkcs11-tool
#    is the preferred surface wherever both options exist.) OpenSC
#    pkcs11-tool ≥ 0.18 supports `--pin-source env:VAR`; older
#    versions fall back to `--pin "${USER_PIN}"` with the same
#    /proc exposure as softhsm2-util.
#
#    SoftHSM2's pkcs11-tool sets CKA_DERIVE=true by default on
#    imported secret keys; verify with `pkcs11-tool ...
#    --read-object` if your vendor differs.
pkcs11-tool --module /usr/lib/softhsm/libsofthsm2.so \
            --slot 0 \
            --login --pin-source env:USER_PIN \
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

# 6. Configure the trio + start the signer. (V1.12: the HSM trio is
#    the sole production audit signal; the V1.9-era ATLAS_PRODUCTION
#    var is silently ignored from V1.12 onwards.)
export ATLAS_HSM_PKCS11_LIB=/usr/lib/softhsm/libsofthsm2.so
export ATLAS_HSM_SLOT=0
export ATLAS_HSM_PIN_FILE="${PIN_FILE}"

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
  compromise" in §7 — every per-tenant pubkey changes, every
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
  the seed itself — the seed is sealed inside the HSM. The PIN
  file does sit on the filesystem, and the seed is not derivable
  from the PIN alone, but both `${ATLAS_HSM_PIN_FILE}` and
  `${ATLAS_HSM_PKCS11_LIB}` are reachable from a single
  environment-disclosure vector (`/proc/<pid>/environ`, a heap
  dump, or any shell sharing the signer's environment), so an
  attacker who lands one typically lands both — at which point
  they hold the signer's full `C_Login`-then-derive capability
  for the lifetime of the PIN's validity. **Filesystem ACLs on
  the PIN file are therefore a *required* operational control,
  mirroring the module-path ACL discussion in the TOCTOU bullet
  below** — mode `0400` (signer-runtime user read-only, no
  group, no others), tmpfs-backed so reboot wipes the host-side
  surface, and the secrets-manager source of truth must be
  rotated on any suspected disclosure.
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

## 3. wave-3 — HSM per-workspace sealed signer ceremony (V1.11)

V1.10 wave 2 sealed the **master seed** inside the HSM but kept the
per-workspace Ed25519 signing key derived in-process via HKDF — the
derived 32-byte secret transited Atlas address space (in a
`Zeroizing<[u8; 32]>` buffer) on every `sign` call. V1.11 Scope A
wave-3 closes the residual risk by moving the per-tenant signing
key itself into the HSM: per-workspace Ed25519 keypairs are
generated via `CKM_EC_EDWARDS_KEY_PAIR_GEN`, persisted as
`Token=true` with `Sensitive=true`, `Extractable=false`,
`Derive=false`, and signing routes through `CKM_EDDSA(Ed25519)`.
For an HSM-backed deployment that opts into wave-3, no per-tenant
secret bytes ever reach Atlas address space — even transiently.

> **wave-3 changes per-tenant pubkeys.** The V1.9 dev path and
> V1.10 wave-2 sealed-seed path both derive per-tenant pubkeys
> deterministically from `(master_seed, workspace_id)` via
> HKDF-SHA256. wave-3 generates the keypair *inside* the HSM
> with hardware entropy; the resulting pubkeys are
> NOT byte-equivalent to the V1.9–V1.10 derivation. Every active
> workspace's `PubkeyBundle.keys[atlas-anchor:<ws>]` rotates on
> first wave-3 sign. Operators MUST run §4 (`rotate-pubkey-bundle`)
> for every active workspace as part of the wave-3 cutover, and MUST
> ship the updated bundle to every verifier-side trust pin before
> the cutover. A verifier holding the V1.10 pinned pubkey will
> reject every wave-3 trace until it receives the new bundle — this
> is the security property, not a bug.

### Why wave-3 needs an explicit opt-in (not "trio implies wave-3")

The V1.10 wave 2 dispatcher activated sealed-seed mode automatically
when the HSM trio (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`,
`ATLAS_HSM_PIN_FILE`) was set, because wave-2 produces
byte-equivalent per-tenant pubkeys to the V1.9 dev path. wave-3
does NOT. If wave-3 activated automatically on the trio, a V1.10
deployment that pinned per-tenant pubkeys in `PubkeyBundle.keys`
would silently rotate every entry on its first wave-3 sign, and
every verifier with a stale bundle would reject every event —
without any opt-in handshake from the operator. The
`ATLAS_HSM_WORKSPACE_SIGNER` env var is the operator's explicit
acknowledgement that they accept the rotation event and have
coordinated bundle rollout to verifiers.

### What gets generated

A per-workspace Ed25519 keypair (one keypair per `workspace_id`),
under the canonical label
`atlas-workspace-key-v1:<workspace_id>`. Both halves carry
`CKA_ID = CKA_LABEL` so PKCS#11 §10.1.2's id-pairing rule binds
them. The label prefix is fixed in code at
[`WORKSPACE_LABEL_PREFIX`](../crates/atlas-signer/src/hsm/pkcs11_workspace.rs).

| Object | Attribute | Value | Why |
|---|---|---|---|
| **public** | `CKA_CLASS` | `CKO_PUBLIC_KEY` | verify-only half, advertised in `PubkeyBundle` |
| **public** | `CKA_KEY_TYPE` | `CKK_EC_EDWARDS` | Ed25519 |
| **public** | `CKA_TOKEN` | `true` | persists across sessions |
| **public** | `CKA_PRIVATE` | `false` | pubkey is public material |
| **public** | `CKA_VERIFY` | `true` | enables `C_Verify` for self-test paths |
| **public** | `CKA_EC_PARAMS` | Ed25519 OID printable | `1.3.101.112` (RFC 8410) |
| **public** | `CKA_LABEL` | `atlas-workspace-key-v1:<ws>` | loader lookup |
| **public** | `CKA_ID` | same bytes as label | pairs with private half |
| **private** | `CKA_CLASS` | `CKO_PRIVATE_KEY` | the sealed scalar |
| **private** | `CKA_KEY_TYPE` | `CKK_EC_EDWARDS` | Ed25519 |
| **private** | `CKA_TOKEN` | `true` | persists across sessions |
| **private** | `CKA_PRIVATE` | `true` | requires authenticated session |
| **private** | `CKA_SIGN` | `true` | enables `C_Sign` under `CKM_EDDSA` |
| **private** | `CKA_SENSITIVE` | `true` | refuse plaintext export |
| **private** | `CKA_EXTRACTABLE` | `false` | scalar never leaves HSM |
| **private** | `CKA_DERIVE` | `false` | blocks `C_DeriveKey` indirect-leak path |
| **private** | `CKA_LABEL` | `atlas-workspace-key-v1:<ws>` | loader lookup |
| **private** | `CKA_ID` | same bytes as label | pairs with public half |

The `CKA_DERIVE=false` choice is defence-in-depth against an HSM
default that allows a `Sensitive=true, Extractable=false` private
key to serve as input to `C_DeriveKey` whose *output* may be
exportable — an indirect way to leak the scalar that some vendor
modules expose. Pinning `false` slams that door.

### Procedure (SoftHSM2 example)

The wave-3 ceremony has two phases: opt-in (one-shot per
deployment) and per-workspace key generation (lazy, on first
`derive-pubkey` / `sign --derive-from-workspace` call per
`workspace_id`). The operator does NOT pre-generate keys —
`atlas-signer` generates the keypair on demand the first time it
encounters an unseen `workspace_id`, and finds the existing keypair
on every subsequent call. This matches the wave-2 flow's
"import-once-then-find" shape.

**Pre-flight assumption.** §2 has already run: SoftHSM2 is
installed, the token is initialised, the user PIN is staged in
`/var/run/atlas/hsm.pin` with mode `0400`, and the master seed is
sealed under `atlas-master-seed-v1`. wave-3 itself does NOT
consume the master seed for signing, but §2 must be complete
because (a) the HSM trio and token used for wave-3 keypair
generation are the same token that holds the master seed, and
(b) the wave-2 fallback layer (active when
`ATLAS_HSM_WORKSPACE_SIGNER` is later unset) depends on it.

> **STOP — irreversible decision.** Before you generate the first
> per-workspace keypair, decide single-token vs. multi-token. Under
> wave-3 each token's hardware entropy generates an independent
> keypair, so two tokens cannot agree on the same per-workspace
> pubkey. **Single-token** = accept total loss of every per-tenant
> pubkey on token failure (no recovery path that preserves them).
> **Multi-token shared-keys** = NOT supported under wave-3; stay on
> wave-2 (unset `ATLAS_HSM_WORKSPACE_SIGNER`) if you need that
> redundancy. Once step 2 below has generated the first keypair on
> a token, reversing this decision requires destroying every
> per-workspace keypair and re-running the bundle rotation
> ceremony.

```bash
# 0. Verify wave-2 (§2) is operational. The wave-3 dispatcher
#    requires the same HSM trio, so a wave-2-failing deployment
#    will fail wave-3 the same way. The HSM trio
#    (ATLAS_HSM_PKCS11_LIB / ATLAS_HSM_SLOT / ATLAS_HSM_PIN_FILE)
#    is assumed exported from the §2 ceremony pre-flight; if you
#    are running §3 in a fresh shell, re-export the trio first.
atlas-signer derive-pubkey --workspace canary-wave2-witness
# Expected: prints a base64url Ed25519 pubkey on stdout.

# 1. Opt into wave-3. The opt-in is a positive env var assertion;
#    leaving it unset (or setting it to a non-truthy value) routes
#    through wave-2. Recognised truthy values are 1 / true / yes /
#    on (ASCII case-insensitive, surrounding whitespace tolerated)
#    — same allow-list as ATLAS_DEV_MASTER_SEED.
export ATLAS_HSM_WORKSPACE_SIGNER=1

# 2. Generate the per-workspace keypair by deriving the pubkey.
#    First call generates; subsequent calls find the existing
#    keypair via C_FindObjects on (CKA_CLASS, CKA_KEY_TYPE,
#    CKA_LABEL).
atlas-signer derive-pubkey --workspace alice
# Expected: prints a NEW base64url Ed25519 pubkey — NOT
# byte-equivalent to the V1.10 wave-2 derivation. Record this
# value as the wave-3 pinned pubkey for `alice`.

# 3. Repeat for every active workspace. Order does not matter; the
#    keypairs are independent.
atlas-signer derive-pubkey --workspace ws-mcp-default
atlas-signer derive-pubkey --workspace bob
# ...etc.

# 4. Now run the bundle rotation (§4) once per workspace to
#    advertise the wave-3 pubkeys in the workspace's PubkeyBundle.
#    Until §4 runs, every verifier holding a V1.10 bundle will
#    reject wave-3 events for that workspace — by design.
```

### Verifying the keys

The verification has **four** stages, mirroring §2 (presence,
attribute-correctness, smoke) and adding a wave-3 refusal check:

```bash
# Stage 1 — presence. Should show one pubkey object AND one
# private-key object per workspace_id, all under
# atlas-workspace-key-v1:<ws> labels.
pkcs11-tool --module "${ATLAS_HSM_PKCS11_LIB}" \
            --slot "${ATLAS_HSM_SLOT}" \
            --login --pin-source "file:${ATLAS_HSM_PIN_FILE}" \
            --list-objects

# Stage 2 — attribute readback. For EACH per-workspace private-key
# object, the default `--list-objects` output already prints the
# security attributes. Confirm all of the following appear:
#
#   Expected attributes for `atlas-workspace-key-v1:<ws>` (private):
#     Access:     sensitive, always sensitive, never extractable
#                 ╰──┬───────╯  ╰─────┬──────╯  ╰──────┬─────────╯
#                    │                │                │
#                    │                │                └─ CKA_EXTRACTABLE: no
#                    │                └─ historical guarantee
#                    └─ CKA_SENSITIVE: yes
#     Usage:      sign
#                 └─ CKA_SIGN: yes (CKA_DERIVE deliberately absent)
#
# A vendor whose pkcs11-tool emits CKA_DERIVE=true for the private
# key MUST be investigated — wave-3 explicitly sets Derive=false
# at generation time, and a token reporting otherwise is either
# (a) lying about the attribute (driver bug), or (b) the operator
# is looking at an unrelated key. If (a), do NOT proceed: the
# indirect-leak path via C_DeriveKey is open. Re-run §3 step 2
# after fixing the vendor module.

# Stage 3 — sign smoke. Verify atlas-signer can produce a signature
# under each per-tenant key. The CLI does not surface a raw `sign`
# subcommand for arbitrary input under wave-3; the smoke runs
# end-to-end via the MCP smoke harness. The HSM trio is assumed
# exported from §2 (the smoke binary inherits the parent shell's
# environment); add `ATLAS_HSM_PKCS11_LIB=… ATLAS_HSM_SLOT=…
# ATLAS_HSM_PIN_FILE=…` inline if running standalone.
ATLAS_HSM_WORKSPACE_SIGNER=1 \
    pnpm --filter atlas-mcp-server smoke

# Stage 4 — derive-key MUST refuse. wave-3 sealed keys are
# unexportable; the `derive-key` subcommand exists only for
# wave-1 / wave-2 paths. Under wave-3 it refuses with exit code 2.
# The refusal fires from a direct env-var read (NOT via the
# dispatcher), so it triggers even if the HSM trio is incomplete —
# the trio is not required for this stage.
ATLAS_HSM_WORKSPACE_SIGNER=1 \
    atlas-signer derive-key --workspace alice
# Expected stderr: "derive-key: refused — wave-3 sealed
# per-workspace signer is opted in via ATLAS_HSM_WORKSPACE_SIGNER..."
# Expected exit: 2.
#
# An exit 0 here means the dispatcher did NOT enforce the wave-3
# refusal — a security regression. Do not proceed; investigate.
```

### Backup, recovery, rotation

- **Backup.** Per-workspace keypairs are generated with hardware
  entropy and held with `Extractable=false`; PKCS#11 has no
  portable export format for them. Operators wanting cross-token
  redundancy must provision multiple tokens and run §3 step 2
  against EACH token at the same time — a `derive-pubkey` call
  against token A generates a *different* keypair than a parallel
  call against token B, because the entropy comes from each device
  independently. The deployment must therefore choose one of:
    - **Single-token deployment** — accept that token loss
      invalidates every per-tenant key. The wave-3 ceremony is
      identical to a fresh provision.
    - **Multi-token deployment with shared keys** — NOT supported
      under wave-3's "generate inside the HSM" model. Operators
      requiring this must stay on wave-2 (master-seed-sealed,
      keys derived in-process), where the master seed CAN be
      imported into multiple tokens at §2 step 3.
- **Recovery from token loss.** Under wave-3 single-token, every
  per-tenant pubkey changes on recovery (the new token generates
  fresh keypairs). Equivalent to "master seed compromise" in §7
  but worse — wave-3 has no "stay on the same key by re-importing
  the same seed" path, because the keypair is generated, not
  derived. Mitigation: take the multi-token decision at deploy
  time, OR keep the wave-2 fallback available by NOT permanently
  setting `ATLAS_HSM_WORKSPACE_SIGNER` in the system environment
  (a deployment that flips wave-3 off by unsetting the env var
  reverts to wave-2 derivation, recovering the V1.10 pubkeys).
- **Rotation.** Generate fresh per-workspace keypairs by
  destroying the existing pair (`pkcs11-tool --delete-object`
  against both halves under
  `atlas-workspace-key-v1:<ws>`) and re-running §3 step 2. The
  next `derive-pubkey` call generates a new keypair under the
  same label. Then re-run §4 (`rotate-pubkey-bundle`) for the
  affected workspace and ship the updated bundle to verifiers.
  Schedule rotations far apart — every rotation invalidates that
  workspace's existing `pubkey_bundle_hash`, exactly like the
  master-seed rotation in §2 but scoped to one workspace.

### Threat model — what wave-3 sealing does and does not protect

**Does protect (extends §2 wave-2 protections to per-tenant keys):**

- Memory-disclosure attacks on the signer host (heap dumps, swap,
  core dumps, debugger attachments) cannot exfiltrate the
  per-tenant Ed25519 signing scalar. Signing runs **inside** the
  HSM via `CKM_EDDSA`; only the 64-byte signature exits the
  device. The scalar never enters Atlas address space — not even
  in a `Zeroizing` buffer (the wave-2 residual). Compare to wave-2
  where the HKDF-derived per-tenant scalar transited a
  `Zeroizing<[u8; 32]>` for the lifetime of one `sign` call;
  wave-3 closes that exposure.
- `derive-key` is structurally refused under wave-3 — there is no
  exportable form of the scalar to export.
- Indirect leak via `C_DeriveKey` is blocked by
  `CKA_DERIVE=false` on the private key (defence-in-depth against
  a sealed key serving as base material to a derivation whose
  output may be exportable).

**Does not protect (same operational risks as §2 wave 2):**

- HSM physical compromise. An attacker with physical possession
  of the token AND the user PIN can call `C_Sign` against any
  per-tenant key for the lifetime of that PIN. Mitigation matches
  §2: PIN file in tmpfs, SO PIN in a separate secret manager,
  rotate the PIN if exposure is suspected.
- Malicious code running inside the signer process. The PKCS#11
  session is held open for the lifetime of the binary; an
  attacker who achieves code execution **inside** atlas-signer
  can call `WorkspaceSigner::sign(workspace_id, msg)` arbitrarily
  during that session's lifetime, signing whatever they like
  under any per-tenant key. Mitigated by short-lived signer
  invocations (the V1.5+ MCP-server pattern shells to atlas-signer
  per event, so no long-lived session exists).
- TOCTOU on the PKCS#11 module path. Same residual as §2's
  wave-2 threat model (V1.10 absolute-path guard fires at
  config-parse, but `dlopen(3)` is a separate syscall — an
  attacker with write access to the absolute path or a parent
  directory can swap the .so between checks). Filesystem ACLs
  on `${ATLAS_HSM_PKCS11_LIB}` AND its parent directories remain
  a *required* operational control under wave-3, identical to
  §2's prescription.
- HSM driver compromise. The PKCS#11 module runs in atlas-signer's
  address space and has full access to every per-tenant key in
  the session. Vendor module signing + filesystem ACLs are the
  defences; there is no in-process sandbox between cryptoki and
  the rest of the signer.
- Cross-workspace replay across the same token. wave-3 binds the
  signature to the canonical signing-input (which folds in
  `workspace_id`, `event_id`, `ts`, `kid`, `parents`, `payload`),
  so a signature under `ws-A`'s key does not verify under
  `ws-B`'s pubkey — same trust property as V1.9. wave-3 does NOT
  add a token-level access-control split between workspaces; an
  attacker with code execution inside the signer can call any
  workspace's signer the same way. Per-workspace token isolation
  is an operational tier (one token per tenant, with separate
  PINs) outside V1.11's scope.

### Test mode

CI exercises the full wave-3 path against SoftHSM2. The MCP smoke
harness honours `ATLAS_HSM_WORKSPACE_SIGNER=1` alongside
`ATLAS_TEST_HSM=1` to opt into the wave-3 dispatcher. The smoke
produces deterministic-but-different bundle hashes from the wave-2
goldens (per the "wave-3 changes per-tenant pubkeys" property
above) and locks them as a separate pinned-hash set in the smoke
test fixtures. Operators verifying the wave-3 ceremony before
production cutover can run the smoke locally to confirm the
dispatcher routes correctly before flipping the deployment flag.

---

## 4. Workspace pubkey-bundle rotation ceremony

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
# 1. Ensure the dev master-seed gate is opted in for the dev session
#    running the ceremony (the dev seed is required to derive the
#    pubkey). V1.12 removed the V1.9 ATLAS_PRODUCTION paranoia gate;
#    the positive opt-in below is the only one required.
export ATLAS_DEV_MASTER_SEED=1

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

## 5. Anchor-chain rotation ceremony

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

## 6. Workspace-id hygiene

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

## 7. Recovery scenarios

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

## 8. CI lanes

V1.12 Scope B introduced three auto-triggered CI lanes that act as
drift sentries for the V1.10 wave-2, V1.11 wave-3, and Sigstore
anchor trust properties. Operators consume the lanes in two ways:
(a) as a pre-merge signal for changes touching the signer / verifier
/ MCP server, (b) as a daily heartbeat against live Sigstore. This
section describes how to interpret a red lane and where to look for
the recovery sketch.

Workflow files live under `.github/workflows/` and each carries an
inline header documenting the trust-property invariant it tests +
the rationale for its trigger surface. Read the header first when
triaging a failure.

### Lane reference

| Lane | Trigger | Invariant under test | First-look on red |
|---|---|---|---|
| `hsm-byte-equivalence.yml` | PR + push (paths-filtered: signer / trust-core / Cargo.lock / workflow), `workflow_dispatch` | V1.10 wave-2: in-HSM HKDF derivation byte-identical to host-side derivation | Cryptoki crate version diff in Cargo.lock; SoftHSM2 package version on the runner; per-workspace test vector mismatch in step output |
| `hsm-wave3-smoke.yml` | PR + push (paths-filtered: signer / trust-core / verify-cli / MCP-server / lockfiles / workflow), `workflow_dispatch` | V1.11 wave-3: end-to-end sealed signer produces verifier-VALID traces | `[smoke] smoke-mode wave-3 sealed (...)` line absent (auto-detect regression); `--features hsm` build failed; verifier rejected an Ed25519 signature emitted by `CKM_EDDSA` |
| `sigstore-rekor-nightly.yml` | cron `0 6 * * *` UTC, `workflow_dispatch` | V1.6+V1.7+V1.8 Sigstore stack: live anchor submission + inclusion-proof verification against the pinned roster | Sigstore Rekor API schema/error change; pinned ECDSA P-256 log pubkey rotation (see [SECURITY-NOTES.md](SECURITY-NOTES.md) "Sigstore shard roster"); tree_id grew past lossless-JSON limit |

### Failure-handling sketch

For every red lane the **first action is the same**: open the run,
read the workflow file's inline header (it documents the failure
classes in the order of historical frequency), then read the failed
step's stderr. The header's "When this lane goes red" block names
the most likely cause and the cross-reference to recovery
documentation.

For `hsm-byte-equivalence` and `hsm-wave3-smoke` failures:

1. Reproduce locally — both lanes use SoftHSM2 + an ephemeral
   token; the same `softhsm2-util --init-token` ceremony from §2 of
   this runbook reproduces the CI environment. Run
   `cargo test -p atlas-signer --features hsm` (byte-equivalence)
   or `pnpm smoke` from `apps/atlas-mcp-server/` with the HSM trio
   exported (wave-3 smoke).
2. If the failure reproduces, inspect the cryptoki crate version
   in `Cargo.lock` + the SoftHSM2 vendor module path against the
   wave's expectation. A vendor-module update on the runner can
   cause a real change in derive-mechanism semantics; the V1.10
   "absolute-path guard" + `permissions: contents: read` block
   bound the blast radius but cannot prevent vendor-side drift.
3. The wave-3 smoke also runs the verifier; a verifier-side
   rejection of a wave-3-emitted signature is investigated against
   `crates/atlas-trust-core/src/event/sig.rs` (the verifier
   acceptance path) and `crates/atlas-signer/src/hsm/pkcs11_workspace.rs`
   (the `CKM_EDDSA` payload encoding).

For `sigstore-rekor-nightly` failures:

1. Read the failed step's stderr — Sigstore returns structured
   error JSON for schema/auth issues and plain network errors for
   reachability issues; the failure mode is usually obvious from
   the first line.
2. If Sigstore announced an incident in `#sigstore-incidents` or
   their status page, wait + manually re-trigger via
   `workflow_dispatch` once they confirm recovery. The lane is
   intentionally tolerant of single nightly misses.
3. If the failure is a pubkey rotation (verifier rejects the
   checkpoint signature against the pinned key), follow the
   "Sigstore Rekor v1 shard roster" rotation sketch in
   `docs/SECURITY-NOTES.md` — extend the pinned roster, bump the
   crate version per V1.7's boundary rule, ship a coordinated
   release.
4. If the failure is `tree_id` precision loss
   (`pubkey_bundle_hash` mismatch on a freshly-anchored bundle),
   confirm the V1.8 lossless-JSON path is intact in
   `crates/atlas-trust-core/src/anchor.rs`; treat any regression
   as a verifier-bug-class incident.

### What red lanes do NOT mean

- A red `sigstore-rekor-nightly` is **not** a regression in the
  Atlas codebase by default — Sigstore is upstream of us and an
  outage on their side is unactionable from this repo. Read the
  step output before assuming the cause is local.
- A red `hsm-byte-equivalence` does **not** invalidate already-
  shipped V1.10 wave-2 deployments retroactively — the property
  was tested against the runner's vendor-module-of-the-day; a
  field deployment using a different vendor module is unaffected
  unless the same vendor-module version landed in production.
- A red `hsm-wave3-smoke` does **not** invalidate already-deployed
  V1.11 wave-3 instances. The lane tests the CI build of the signer
  + verifier against a SoftHSM2 token; a regression here means the
  current branch's code path no longer round-trips, NOT that any
  field deployment has stopped signing or verifying. Field impact
  depends on whether the regressed commit gets deployed.

---

## 9. Quick reference

| Ceremony | Command | Idempotent | Atomic-replace required |
|---|---|---|---|
| HSM master-seed import (V1.10 wave 2 — see §2) | `pkcs11-tool ... --write-object <seed-file> --label atlas-master-seed-v1` | no (re-import overwrites) | n/a (HSM-side) |
| HSM per-workspace key generation (V1.11 wave-3 — see §3) | `ATLAS_HSM_WORKSPACE_SIGNER=1 atlas-signer derive-pubkey --workspace <ws>` | yes (find-or-generate) | n/a (key lives in HSM) |
| Migrate workspace bundle to V1.9 | `atlas-signer rotate-pubkey-bundle --workspace <ws>` | yes | yes (operator-side `mv`) |
| Rotate anchor chain | `atlas-signer rotate-chain --confirm <workspace>` | no | yes (operator-side) |
| Derive workspace pubkey for inspection | `atlas-signer derive-pubkey --workspace <ws>` | yes | n/a (read-only) |
| Production-gate enforcement (V1.12+) | configure the HSM trio (§2) — V1.9 `ATLAS_PRODUCTION=1` removed | n/a | n/a |
| Witness commissioning, verifier side (V1.13 wave-C-2 — see §10) | edit `ATLAS_WITNESS_V1_ROSTER`, rebuild verifier, deploy with `--require-witness <N>` | no (commissioning is a code-side change) | n/a (verifier-side) |
| HSM-backed witness keypair generation (V1.14 Scope I — see §11) | `pkcs11-tool ... --keypairgen --key-type EC:edwards25519 --label atlas-witness-key-v1:<kid> --usage-sign` | no (re-gen replaces) | n/a (key lives in HSM) |
| Witness pubkey extract (V1.14 Scope I — see §11) | `atlas-witness extract-pubkey-hex --kid <witness_kid>` | yes (read-only) | n/a (HSM-side) |
| HSM-backed witness signing (V1.14 Scope I — see §11) | `atlas-witness sign-chain-head --hsm --kid <witness_kid> --chain-head <hex>` | yes (signing is functional) | n/a (HSM-side) |

See [ARCHITECTURE.md §7.3](ARCHITECTURE.md) (V1.9 per-tenant key trust
model), [§7.4](ARCHITECTURE.md) (V1.10 master-seed gate + wave-2
sealed-seed loader, V1.12-simplified) and [§7.5](ARCHITECTURE.md)
(V1.11 wave-3 sealed per-workspace signer); see
[SECURITY-NOTES.md](SECURITY-NOTES.md) for the full threat model.

---

## 10. Witness commissioning ceremony (V1.13 wave-C-2)

V1.13 introduces an independent witness cosignature attestor: an
external party that signs over `chain_head_for(batch)` to vouch
that they observed the chain at that head. The verifier accepts
witnesses against a *pinned, source-controlled roster*
(`ATLAS_WITNESS_V1_ROSTER` in `crates/atlas-trust-core/src/witness.rs`).
Wave-C-1 wired the lenient default (any presented witness surfaces
in evidence; failures do not block); wave-C-2 promotes this to
operationally load-bearing via the strict-mode threshold flag.

> **Sister procedure §11.** This section is the **verifier-side**
> half of witness commissioning — pinning a witness's pubkey into
> the trust-core roster. The **witness-side** half (generating the
> keypair on the signer/HSM, extracting the pubkey hex) is §11
> for HSM-backed deployments or follows the V1.13 file-backed
> seed-file shape for software-backed deployments. For an HSM-
> backed witness, run §11 first (witness operator generates the
> keypair, extracts the hex), then this section (verifier
> maintainer pins the hex). The two sections form a single
> ceremony split across two trust domains.

### Why commissioning is a code-side change, not a config knob

The roster is `&'static [(&'static str, [u8; 32])]` baked into the
trust-core crate at compile time. There is no JSON/env mechanism to
add a witness at runtime. This is intentional:

  * The trust property is "verification only against pinned,
    source-controlled keys." A runtime knob would let an attacker
    who compromised the verifier's environment add a key — every
    witness becomes attacker-controlled, threshold defence
    collapses.
  * Reproducibility: the same trace + the same trust-core build
    must yield byte-identical evidence. A runtime roster would
    silently change verifier output across deployments.

Cost: commissioning a new witness requires a verifier rebuild +
redeploy. This is by design — the cadence of commissioning ceremonies
should be operator-deliberate, not casual.

### When to run

  * Before flipping any deployment to `--require-witness <N>` for
    `N >= 1`. Strict mode against the genesis-empty roster will
    reject every trace (0 verified < 1 required).
  * When adding an additional cosigner to raise the threshold from
    M-of-N to (M+1)-of-(N+1).
  * When rotating a compromised witness key out of the roster.

### Procedure

```bash
# 0. Pre-flight: confirm the witness counterparty has generated
#    their Ed25519 keypair using a process YOU trust (sealed key
#    storage on their end is the witness's responsibility, not
#    Atlas's). They send you ONLY the 32-byte raw pubkey.
#    NEVER accept a private key — defeats the independence property.
#
#    For an HSM-backed witness operated in-house (V1.14 Scope I),
#    §11 IS that trusted generation process — run it first, capture
#    the `extract-pubkey-hex` output, then proceed with this
#    section against the captured hex. For an external witness
#    counterparty, ask them which process they used; the
#    independence property holds regardless of substrate (file,
#    HSM, smart card) provided the private half never leaves their
#    control.

# 1. Choose a kid that uniquely names the witness in
#    auditor-readable form. Suggested format:
#      "witness-<org>-<purpose>-<rotation-counter>"
#    Example: "witness-acme-sec-eu-2026-q2"
#    Constraints (enforced by the verifier):
#      * Length <= MAX_WITNESS_KID_LEN (256 bytes)
#      * Must be unique within the roster (duplicate-kid pre-pass
#        would reject every signature carrying the duplicated kid)
#      * Must be source-controlled in the same commit that adds
#        the pubkey — auditors review them as one diff

# 2. Edit the pinned roster in trust-core. Locate the constant:
#    crates/atlas-trust-core/src/witness.rs:
#        pub const ATLAS_WITNESS_V1_ROSTER:
#            &[(&str, [u8; 32])] = &[];
#    Append your `(kid, pubkey-bytes)` tuple. Use the
#    `[u8; 32]` literal directly — no base64 indirection — so the
#    pubkey is reviewable byte-by-byte in the source diff.

# 3. Run the workspace test suite. The
#    `roster_kids_within_length_cap` test enforces the
#    MAX_WITNESS_KID_LEN cap at compile time; CI rejects an
#    over-long kid before the binary ever lands.
#        cargo test --workspace

# 4. Get the change reviewed. The reviewer checks:
#    (a) the kid format matches the suggested convention,
#    (b) the pubkey bytes match what the witness counterparty
#        published OUT-OF-BAND (e.g., on their corporate page,
#        in a signed announcement) — never trust a pubkey that
#        arrived via the same channel as the change request,
#    (c) the rotation-counter in the kid is incremented if a
#        prior witness from the same org is being replaced.

# 5. Merge, tag, build, deploy the verifier. Operators verifying
#    traces with the new witness must be running THIS verifier
#    build or later — older builds reject the new kid as "not in
#    pinned roster". Coordinate the rollout with downstream
#    auditors before flipping --require-witness.

# 6. Once the new verifier is deployed everywhere, raise the
#    threshold:
#        atlas-verify-cli verify-trace <trace.json> -k <bundle.json> \
#            --require-witness <N>
#    where N is the desired M-of-N quorum. Start with N=1 to
#    confirm the new witness is actually signing in production
#    before raising further.
```

### Verifying the commissioning

After redeployment:

```bash
# 1. Pull a recent trace bundle from a workspace whose chain
#    includes a batch the new witness has cosigned.

# 2. Run the verifier with the strict threshold matching the
#    expected witness count.
atlas-verify-cli verify-trace trace.json -k bundle.json \
    --require-witness 1 --output json | jq '.evidence[] |
    select(.check == "witnesses-threshold")'

# 3. The output must show:
#    {
#      "check": "witnesses-threshold",
#      "ok": true,
#      "detail": "1 of 1 required witness attestor(s) verified (strict mode)"
#    }
#    If `ok` is false, the witness signature did not validate —
#    confirm the issuer is actually attaching cosignatures (the
#    `witnesses` evidence row above shows the per-witness breakdown).
```

### Rotation, revocation, threat model

  * **Rotation**: replace the `(kid, pubkey)` tuple with an
    incremented rotation-counter in the kid. Old kid stays
    unrecognised after redeployment — any trace still carrying a
    cosignature under the old kid surfaces as "not in pinned
    roster", which is the correct disposition.
  * **Revocation**: remove the tuple from the roster. Same effect
    as rotation but without a replacement — drops the witness
    from the quorum entirely. Operators must lower
    `--require-witness <N>` accordingly or strict mode rejects
    every trace until a replacement is commissioned.
  * **Compromise of one witness key**: cross-batch dedup
    (V1.13 wave-C-2) ensures one compromised key cannot satisfy
    threshold N by signing N batches under the same kid. The
    aggregator counts each kid at most once across the chain.
    Defence-in-depth: an `M-of-N` threshold with `M >= 2` requires
    compromising `M` independent witnesses to forge — a real cost
    increase versus single-witness mode.
  * **What strict mode does NOT defend against**: a malicious
    *issuer* with valid signing keys can still produce traces; the
    witness check confirms third-party observation of the chain
    head, not that the underlying events are honest. See
    [SECURITY-NOTES.md](SECURITY-NOTES.md) §wave-c for the full
    threat model and the lenient/strict mode trust-property table.

### Trust property

`verified == count(distinct kids whose pubkey is in
ATLAS_WITNESS_V1_ROSTER AND whose Ed25519-strict signature over
ATLAS_WITNESS_DOMAIN || chain_head_bytes validates AND no other
batch in the chain already attributed verification to that kid)`.

`require_witness_threshold = N` rejects any trace whose chain
yields `verified < N`. `N = 0` is the wave-C-1 lenient default.

---

## 11. HSM-backed witness commissioning ceremony (V1.14 Scope I)

V1.13 wave-C wired the witness signing path against a *file-backed*
secret (a 32-byte Ed25519 seed at `--secret-file <path>`). V1.14
Scope I closes the residual exposure that the witness's signing
scalar transits the witness binary's address space on every
`sign-chain-head` call by introducing an HSM-backed witness backend:
the Ed25519 private scalar lives inside the PKCS#11 token with
`Sensitive=true`, `Extractable=false`, `Derive=false`, and signing
routes through `CKM_EDDSA(Ed25519)`. For an HSM-backed witness
deployment, no witness signing scalar ever reaches Atlas address
space — even transiently.

This ceremony is the **sister procedure** to §10 (the verifier-side
roster pinning ceremony). §11 generates the keypair on the HSM and
extracts the public-key bytes; §10 takes those bytes and pins them
into `ATLAS_WITNESS_V1_ROSTER` in the verifier's trust-core source.
**Both ceremonies must complete before a new HSM-backed witness can
contribute to a strict-mode quorum.** Run §11 first (witness side),
hand the extracted hex to the roster maintainer, then run §10
(verifier side). Skipping the §10 step yields a witness whose
signatures land in evidence but never count against `verified` — a
silent disconnect that does not surface until an operator wonders
why threshold is unmet.

### Why a code-side procedure for the witness side too

The witness binary's HSM backend is opt-in per the env trio
(`ATLAS_WITNESS_HSM_PKCS11_LIB`, `ATLAS_WITNESS_HSM_SLOT`,
`ATLAS_WITNESS_HSM_PIN_FILE`). The opt-in is a positive assertion —
unset trio falls back to the file-backed `--secret-file` path,
partial trio refuses to start. Same operator-deliberate cadence as
§3's wave-3 opt-in, for the same reason: silent fallback masks
typos.

The witness binary does NOT auto-generate a keypair on first use
(structural difference vs §3 wave-3, which generates per-workspace
keypairs lazily on first `derive-pubkey` call). The witness's
`Pkcs11Witness::open` only **resolves** an existing keypair by
label; missing keypair fails with `SigningFailed: no PKCS#11
private-key object with CKA_LABEL=...` and a pointer back to this
section. This is the load-bearing trust property — a witness that
auto-generated keys could be made to sign on a fresh, unrostered
keypair and silently bypass the roster contract. Generation is
therefore an operator action (this section), not a runtime
side-effect.

### Trust-domain separation from atlas-signer

The witness binary and signer binary use **distinct env-var
prefixes**: `ATLAS_WITNESS_HSM_*` vs `ATLAS_HSM_*`. An operator who
accidentally re-uses `ATLAS_HSM_*` for the witness binary gets a
clean "trio not set" SKIP and falls back to the file-backed witness
— rather than a surprise authentication against the signer's HSM
token under the witness's logical identity. Likewise, the on-token
label prefix is distinct: `atlas-witness-key-v1:` vs the signer's
`atlas-workspace-key-v1:`. A misconfigured deployment that points
both binaries at the same slot still cannot cross-resolve keys — the
label namespaces are disjoint.

Operationally, the recommendation is **separate slots per
trust-domain**: one slot for atlas-signer's master-seed + per-workspace
keys, another slot for the witness keypair. The label-prefix
separation is defence-in-depth on top of the slot separation, not a
substitute for it.

### What gets generated

A single Ed25519 keypair under the label
`atlas-witness-key-v1:<witness_kid>`. Both halves carry
`CKA_ID = CKA_LABEL` so PKCS#11 §10.1.2's id-pairing rule binds
them. The label prefix is fixed in code at
[`WITNESS_LABEL_PREFIX`](../crates/atlas-witness/src/hsm/config.rs).

| Object | Attribute | Value | Why |
|---|---|---|---|
| **public** | `CKA_CLASS` | `CKO_PUBLIC_KEY` | verify-only half, extracted for roster |
| **public** | `CKA_KEY_TYPE` | `CKK_EC_EDWARDS` | Ed25519 |
| **public** | `CKA_TOKEN` | `true` | persists across sessions |
| **public** | `CKA_PRIVATE` | `false` | pubkey is public material |
| **public** | `CKA_VERIFY` | `true` | enables `C_Verify` for self-test paths |
| **public** | `CKA_EC_PARAMS` | Ed25519 OID | `1.3.101.112` (RFC 8410) |
| **public** | `CKA_LABEL` | `atlas-witness-key-v1:<kid>` | loader lookup |
| **public** | `CKA_ID` | same bytes as label | pairs with private half |
| **private** | `CKA_CLASS` | `CKO_PRIVATE_KEY` | the sealed scalar |
| **private** | `CKA_KEY_TYPE` | `CKK_EC_EDWARDS` | Ed25519 |
| **private** | `CKA_TOKEN` | `true` | persists across sessions |
| **private** | `CKA_PRIVATE` | `true` | requires authenticated session |
| **private** | `CKA_SIGN` | `true` | enables `C_Sign` under `CKM_EDDSA` |
| **private** | `CKA_SENSITIVE` | `true` | refuse plaintext export |
| **private** | `CKA_EXTRACTABLE` | `false` | scalar never leaves HSM |
| **private** | `CKA_DERIVE` | `false` | blocks `C_DeriveKey` indirect-leak path |
| **private** | `CKA_LABEL` | `atlas-witness-key-v1:<kid>` | loader lookup |
| **private** | `CKA_ID` | same bytes as label | pairs with public half |

The `CKA_DERIVE=false` choice mirrors §3 wave-3 — defence-in-depth
against an HSM default that allows a `Sensitive=true,
Extractable=false` private key to serve as input to `C_DeriveKey`
whose *output* may be exportable. Pinning `false` slams that door
for the witness scalar too.

### Procedure (SoftHSM2 example)

The §11 ceremony has two phases: keypair generation (one-shot per
witness commissioning) and pubkey extraction (hand-off step that
feeds §10's roster pin). The operator drives generation via
`pkcs11-tool --keypairgen` directly — atlas-witness does NOT
auto-generate (per "Why a code-side procedure" above).

SoftHSM2 is the open-source PKCS#11 implementation used in CI;
production HSMs (Thales Luna, AWS CloudHSM, YubiHSM2, Nitrokey HSM 2)
support the same `pkcs11-tool` flow with vendor-specific module
paths.

**Operator caution before you start.** Same handling as §2 / §3:
PINs read from a secrets manager into shell env vars, never pasted
literally on the command line, no shell history. Re-read §2's
"Operator caution before you start" block if it has been a while —
the threat surface (`/proc/<pid>/cmdline`, shell history,
session-recording tools) is identical here.

**Pre-flight assumption.** No prior ceremony is required —
§11 is independent of §2 and §3 (the witness binary is a separate
process from atlas-signer with its own trust domain). Operators can
provision a witness slot on a fresh token, or share a token with the
signer at a different slot. The slot used here MUST NOT be the same
slot that holds atlas-signer's master seed or per-workspace keys —
the label-prefix split prevents accidental cross-resolution but
production hygiene wants the slot split too.

> **STOP — irreversible decision.** Before you generate the keypair,
> decide single-token vs. multi-token. Each token's hardware entropy
> generates an independent keypair, so two tokens cannot agree on
> the same witness pubkey. **Single-token** = accept total loss of
> the witness key on token failure (§10 rotation required to
> commission a replacement). **Multi-token shared-witness** = NOT
> supported (matches §3 wave-3's "generate inside the HSM" model;
> the file-backed witness via `--secret-file` is the path for
> deployments needing seed redundancy across substrates). Once the
> keypair is generated, reversing this decision requires running
> the rotation procedure below.

```bash
# 0. Pre-flight: install SoftHSM2 + opensc (provides pkcs11-tool).
#    On Debian/Ubuntu: apt-get install softhsm2 opensc
#    On Fedora:        dnf install softhsm opensc

# 0a. Tighten the umask BEFORE writing any file. Same rationale as
#     §2 step 0a — guarantees subsequent file creates are mode 0600
#     across libc variants, no chmod race window.
umask 077

# 0b. Pull the SO PIN and user PIN from your secrets manager. Same
#     handling as §2 step 0b — env vars, never argv-literal PINs.
SO_PIN="$(your-secrets-manager get atlas/witness-softhsm/so-pin)"
USER_PIN="$(your-secrets-manager get atlas/witness-softhsm/user-pin)"

# 1. Initialise a token in slot N for the witness. Use a slot
#    DIFFERENT from any atlas-signer slot. The label here is
#    operator-chosen and not parsed by atlas-witness — it identifies
#    the token to your operations team, not to the binary.
#
#    Same /proc/<pid>/cmdline caveat as §2 step 1: softhsm2-util
#    reads PINs from argv. The exposure window is bounded to this
#    one-shot init step; the pkcs11-tool keypairgen in step 2 reads
#    the PIN via --pin-source env: and is not affected.
softhsm2-util --init-token --slot 0 \
              --label atlas-witness-prod \
              --so-pin "${SO_PIN}" \
              --pin    "${USER_PIN}"

# 2. Generate the witness Ed25519 keypair under the canonical label.
#    The label is the load-bearing on-token name —
#    atlas-witness::Pkcs11Witness::open looks up this exact bytes
#    sequence via C_FindObjects on (CKA_CLASS, CKA_KEY_TYPE,
#    CKA_LABEL). Choose the witness_kid carefully:
#      * It MUST match the kid pinned in §10's roster commit
#        (auditors compare these byte-for-byte across the witness
#        deploy and the verifier deploy).
#      * Suggested format: "witness-<org>-<purpose>-<rotation-counter>"
#        (same convention as §10). Example:
#        "witness-acme-sec-eu-2026-q2".
#      * Length <= MAX_WITNESS_KID_LEN (256 bytes).
#      * MUST NOT contain ':' (parsed as the label-prefix delimiter
#        — the witness validator rejects kids containing ':' at
#        Pkcs11Witness::open time).
WITNESS_KID="witness-acme-sec-eu-2026-q2"

#    The `--key-type EC:edwards25519` flag selects
#    CKM_EC_EDWARDS_KEY_PAIR_GEN producing a CKK_EC_EDWARDS keypair.
#    `--usage-sign` sets CKA_SIGN=true on the private half. SoftHSM2
#    defaults Sensitive=true / Extractable=false / Derive=false on
#    EC private keys; production HSMs may differ. ALWAYS run the
#    Stage 2 attribute readback below to confirm — a vendor that
#    defaults Derive=true on the private half opens the indirect-
#    leak path and the keypair MUST be regenerated with explicit
#    attribute overrides.
pkcs11-tool --module /usr/lib/softhsm/libsofthsm2.so \
            --slot 0 \
            --login --pin-source env:USER_PIN \
            --keypairgen \
            --key-type EC:edwards25519 \
            --label "atlas-witness-key-v1:${WITNESS_KID}" \
            --usage-sign

# 3. Stage the user PIN as a 0400 file for atlas-witness. Same
#    install -m 0400 / atomic-create pattern as §2 step 5. Note the
#    distinct path namespace — separate from
#    /var/run/atlas/hsm.pin which atlas-signer owns.
PIN_FILE=/var/run/atlas/witness-hsm.pin
install -m 0400 -o atlas -g atlas /dev/stdin "${PIN_FILE}" \
    < <(printf '%s' "${USER_PIN}")

# 3a. Wipe the in-memory PINs from this shell now that the file is
#     on disk.
unset SO_PIN USER_PIN

# 4. Configure the witness HSM trio. Note the prefix —
#    ATLAS_WITNESS_HSM_*, NOT ATLAS_HSM_*. This is the structural
#    separation between witness and signer trust domains; an
#    operator who re-uses the signer's prefix gets a "trio not set"
#    SKIP from the witness binary and accidentally falls back to
#    the file-backed witness, NOT an authentication against the
#    signer's token under the witness's identity.
export ATLAS_WITNESS_HSM_PKCS11_LIB=/usr/lib/softhsm/libsofthsm2.so
export ATLAS_WITNESS_HSM_SLOT=0
export ATLAS_WITNESS_HSM_PIN_FILE="${PIN_FILE}"

# 5. Extract the witness public-key bytes. atlas-witness reads the
#    paired CKO_PUBLIC_KEY object's CKA_EC_POINT, unwraps the
#    PKCS#11 v3.0 §10.10 DER OCTET STRING wrapper (or accepts a raw
#    32-byte form for vendors that deviate), and prints the hex
#    representation on stdout. Capture this value — it is the input
#    to §10 step 2 (roster pinning).
WITNESS_PUBKEY_HEX="$(atlas-witness extract-pubkey-hex --kid "${WITNESS_KID}")"
echo "${WITNESS_PUBKEY_HEX}"
# Expected: 64 hex chars (32-byte raw Ed25519 pubkey).

# 6. Hand the (witness_kid, pubkey hex) pair to the roster
#    maintainer OUT-OF-BAND. Run §10 (verifier-side commissioning)
#    against this pair, get the roster commit reviewed and merged,
#    rebuild and redeploy every verifier before raising
#    --require-witness <N>. UNTIL §10 is complete, the witness's
#    signatures land in evidence as "kid not in pinned roster" and
#    do not count toward the threshold — by design, but cleanly
#    unobservable as a quorum-met state.

# 7. Smoke-test the witness signing path. The chain head input must
#    be a 64-char hex string (32-byte chain head), matching the
#    canonical decoding in atlas_trust_core::decode_chain_head.
#    A successful sign emits a JSON line with witness_kid + sig
#    fields on stdout. Failure prints the cleaving String prefix
#    (Locked / Unavailable / SigningFailed) per the witness error
#    rules.
atlas-witness sign-chain-head \
    --hsm \
    --kid "${WITNESS_KID}" \
    --chain-head "0000000000000000000000000000000000000000000000000000000000000000"
```

### Verifying the keys

The verification has **four** stages, mirroring §3 wave-3 (presence,
attribute readback, sign smoke) and adding a hand-off check:

```bash
# Stage 1 — presence. Should show one pubkey object AND one
# private-key object, both under
# atlas-witness-key-v1:<witness_kid>.
pkcs11-tool --module "${ATLAS_WITNESS_HSM_PKCS11_LIB}" \
            --slot "${ATLAS_WITNESS_HSM_SLOT}" \
            --login --pin-source "file:${ATLAS_WITNESS_HSM_PIN_FILE}" \
            --list-objects

# Stage 2 — attribute readback. For the private-key object, the
# default --list-objects output prints the security attributes.
# Confirm all of the following appear:
#
#   Expected attributes for `atlas-witness-key-v1:<kid>` (private):
#     Access:     sensitive, always sensitive, never extractable
#                 ╰──┬───────╯  ╰─────┬──────╯  ╰──────┬─────────╯
#                    │                │                │
#                    │                │                └─ CKA_EXTRACTABLE: no
#                    │                └─ historical guarantee
#                    └─ CKA_SENSITIVE: yes
#     Usage:      sign
#                 └─ CKA_SIGN: yes (CKA_DERIVE deliberately absent)
#
# A vendor whose pkcs11-tool emits CKA_DERIVE=true for the private
# key MUST be investigated — a production HSM defaulting Derive=true
# opens the indirect-leak path via C_DeriveKey. Re-run step 2 of
# the procedure with explicit `--attribute CKA_DERIVE:false` (newer
# pkcs11-tool ≥ 0.21) or use the vendor's attribute-set tool to
# clear CKA_DERIVE before proceeding.

# Stage 3 — sign smoke. Verify atlas-witness can produce a witness
# signature under the HSM-resident key. The cli echoes the sig as
# JSON; non-zero exit + a String error prefix indicates failure.
atlas-witness sign-chain-head \
    --hsm \
    --kid "${WITNESS_KID}" \
    --chain-head "$(printf '%064d' 0)"
# Expected (success): one JSON line on stdout with witness_kid and
# sig fields, exit 0.
# Expected (failure): "SigningFailed: ..." on stderr, exit 1
# (atlas-witness uses ExitCode::FAILURE for all error paths; a
# distinct code-2 lane is reserved for future use).

# Stage 4 — extract round-trip. Re-extract the pubkey and confirm
# it matches the value handed off to §10. A divergence here would
# indicate the on-token public object was destroyed or altered
# between commissioning and verification — investigate before
# trusting the deployment.
atlas-witness extract-pubkey-hex --kid "${WITNESS_KID}"
# Expected: same 64-char hex captured at procedure step 5.
```

### Backup, recovery, rotation

- **Backup.** The witness keypair is generated with hardware
  entropy and held with `Extractable=false`; PKCS#11 has no portable
  export format for the private half. Operators wanting cross-token
  redundancy must provision multiple tokens and run §11 step 2
  against EACH token at the same time — but each token will produce
  a *different* keypair (different entropy → different scalar →
  different pubkey). The deployment must therefore choose one of:
    - **Single-token witness** — accept that token loss invalidates
      the witness identity. Recovery requires the rotation procedure
      below + a fresh §10 roster commit pinning the new pubkey.
    - **Multi-token witness farm with distinct kids** — each token
      runs as an independently-rostered witness
      (`witness-<org>-<purpose>-<token-id>-<rotation-counter>`).
      Threshold N at the verifier counts each token's
      cosignature once. This is the operationally sane redundancy
      path under HSM-backed witnessing — no shared scalar, M-of-N
      compromise resistance scales linearly with token count.
    - **File-backed witness** — for deployments needing the
      simpler "same scalar across substrates" property, stay on
      the V1.13 `--secret-file` path. The trade-off is the
      memory-disclosure exposure that V1.14 closes.
- **Recovery from token loss.** Equivalent to "witness key
  compromise" — the rostered pubkey is no longer reachable, so the
  witness drops out of the quorum. Mitigation: lower
  `--require-witness <N>` at the verifier to (current quorum -
  lost-witness count) until a replacement is commissioned. If the
  remaining quorum cannot meet the configured threshold, every
  trace rejects until §11 + §10 complete for a replacement.
- **Rotation.** Generate fresh keypair by destroying the existing
  pair (`pkcs11-tool --delete-object` against both halves under
  `atlas-witness-key-v1:<old-kid>`) and re-running §11 step 2 with
  an *incremented* witness_kid (the rotation-counter trailer in
  the convention). Then run §10 against the new (kid, pubkey) pair
  — the old roster entry is removed in the same commit so the old
  kid no longer counts. Schedule rotations far apart — every
  rotation invalidates the prior witness identity and requires
  coordinated verifier rebuild + redeploy.

### Threat model — what HSM-witness sealing does and does not protect

**Does protect (closes V1.13's witness-scalar memory exposure):**

- Memory-disclosure attacks on the witness host (heap dumps, swap,
  core dumps, debugger attachments) cannot exfiltrate the witness
  Ed25519 signing scalar. Signing runs **inside** the HSM via
  `CKM_EDDSA`; only the 64-byte signature exits the device. The
  scalar never enters atlas-witness address space — not even in a
  `Zeroizing` buffer (the V1.13 `--secret-file` path's residual).
- Filesystem-level snapshot of the witness host does not contain
  the witness scalar — it lives only on the HSM. The PIN file does
  sit on the filesystem; same operational controls as §2's PIN-file
  guidance apply (mode 0400, tmpfs-backed, secrets-manager source
  of truth, rotate on suspected disclosure).
- Source-code disclosure does not yield the witness scalar. The
  V1.13 `--secret-file` path required the operator to keep the
  raw 32-byte seed file out of the source repo and out of process
  surfaces; V1.14 HSM-backed mode removes the host-side scalar
  artefact entirely.
- Indirect leak via `C_DeriveKey` is blocked by `CKA_DERIVE=false`
  on the private key (defence-in-depth against a sealed key
  serving as base material to a derivation whose output may be
  exportable).

**Does not protect (same operational risks as §3 wave-3):**

- HSM physical compromise. An attacker with physical possession of
  the witness token AND the user PIN can call `C_Sign` against the
  witness key for the lifetime of that PIN. Mitigation matches §2:
  PIN file in tmpfs, SO PIN in a separate secret manager, rotate
  the PIN if exposure is suspected.
- Malicious code running inside the witness process. The PKCS#11
  session is held open for the lifetime of the binary; an attacker
  who achieves code execution **inside** atlas-witness can call
  `Pkcs11Witness::sign_chain_head` arbitrarily during that
  session's lifetime, signing whatever chain head they like.
  Mitigated by short-lived witness invocations — the V1.14 CLI
  is single-shot per invocation (`sign-chain-head` produces one
  sig, then exits), so no long-lived session exists in production
  unless an embedder explicitly holds the handle open.
- TOCTOU on the PKCS#11 module path. Same residual as §2 / §3
  (V1.14 absolute-path guard fires at config-parse, but
  `dlopen(3)` is a separate syscall — an attacker with write
  access to the absolute path or a parent directory can swap the
  .so between checks). Filesystem ACLs on
  `${ATLAS_WITNESS_HSM_PKCS11_LIB}` AND its parent directories
  remain a *required* operational control — identical to §2's
  prescription.
- HSM driver compromise. The PKCS#11 module runs in atlas-witness's
  address space and has full access to the witness key in the
  session. Vendor module signing + filesystem ACLs are the
  defences; there is no in-process sandbox between cryptoki and
  the rest of the witness binary.
- Malicious *issuer* producing traces. The witness check confirms
  third-party observation of the chain head; it does NOT certify
  that the underlying events are honest. See
  [SECURITY-NOTES.md](SECURITY-NOTES.md) §wave-c for the full
  threat model and the lenient/strict mode trust-property table.

### How §11 composes with §10

| Step | Side | Section | What happens |
|---|---|---|---|
| 1 | Witness operator | §11 | Generate keypair on HSM, extract pubkey hex |
| 2 | Hand-off | — | Witness operator publishes (kid, pubkey hex) out-of-band |
| 3 | Verifier maintainer | §10 | Pin (kid, pubkey) into `ATLAS_WITNESS_V1_ROSTER`, code review, merge |
| 4 | Verifier ops | §10 | Rebuild + redeploy verifier with the new roster |
| 5 | Witness ops | §11 step 7 | Smoke `sign-chain-head --hsm` once verifier rollout is complete |
| 6 | Verifier ops | §10 | Raise `--require-witness <N>` to include the new witness in the quorum |

A failure at any step rolls back to the prior threshold. The
witness's signatures continue to land in evidence regardless — only
the threshold check is gated by §10 completion. This is intentional:
auditors can confirm the witness is *operating* (signing) before the
verifier flips it to *load-bearing* (counted toward quorum).

### Test mode

CI exercises the full HSM-witness path against SoftHSM2. The
[`hsm_witness_byte_equivalence`](../crates/atlas-witness/tests/hsm_witness_byte_equivalence.rs)
integration test imports a known seed into a SoftHSM2 token, opens
the resulting keypair via `Pkcs11Witness::open`, and verifies that
the HSM-produced signature matches a software-Ed25519 reference
signature byte-for-byte. The test honours the same `ATLAS_TEST_HSM=1`
+ trio-set gate as §2 / §3 — no SoftHSM2 installed → SKIP, no
silent pass. Operators verifying the V1.14 cutover before
production can run the test locally to confirm the signing path
produces byte-equivalent output to the V1.13 file-backed witness
(invariant: the witness sig is a function of the scalar and the
chain head, independent of whether the scalar lives in a file or on
the HSM).

## 12. WASM verifier — backup-channel install via GitHub Releases (V1.15 Welle B)

> **Audience note.** This section is the *operator-side* runbook —
> what the Atlas team running the publish lane needs to know, plus
> the auditor flow when npmjs.org is unreachable. For the
> *consumer-side* perspective (lockfile pinning, SLSA provenance
> verification on every CI install, reproduce-from-source fallback
> when both channels fail), see
> [CONSUMER-RUNBOOK.md](CONSUMER-RUNBOOK.md) (V1.15 Welle C).

`@atlas-trust/verify-wasm` ships through two channels by design:

1. **Primary — npmjs.org.** `npm install @atlas-trust/verify-wasm`.
   This is the default for every consumer; SLSA L3 provenance is
   attached at publish time and verifiable with `npm audit
   signatures`.
2. **Backup — GitHub Releases.** Each tagged release uploads the
   identical `npm pack` output (plus a SHA256 manifest) as release
   assets at
   `https://github.com/ThePyth0nKid/atlas/releases/<TAG>`. Use this
   when npmjs.org is unreachable (registry outage, account
   compromise, or — rarely — a registry-side tampering claim that
   the auditor wants to byte-check independently).

The two channels serve **byte-identical tarballs**. The npm-side
provenance attestation (signed against the tarball SHA256 + commit
SHA) covers both — the GH-Release tarball is verifiable against the
same attestation by recomputing its hash and comparing to the
npm registry's `dist.integrity` field.

> **Trust roots are NOT equivalent across the two channels.** The
> `tarball-sha256.txt` manifest uploaded alongside the GH-Release
> tarballs is a *transport-integrity* check — it detects in-flight
> tampering between GitHub's blob store and your machine, but it
> does NOT establish origin integrity. The manifest and the tarballs
> are produced in the same workflow run, so a compromised runner
> would yield a manifest and tarball that match each other while
> both being attacker-controlled. The *origin-integrity* trust root
> is the npm-side OIDC-signed SLSA L3 provenance attestation. If you
> install from the backup channel, step 3 below (cross-verify the
> tarball SHA512 against `npm view … dist.integrity`) is **NOT
> optional** — it is the step that establishes the GH-Release bytes
> match what the npm registry believes was published. Skip step 3
> only if npm itself is unreachable AND you are willing to fall
> through to verifier-side reproducibility from source (clone the
> repo at the tagged commit, rebuild with the pinned wasm-pack
> version, byte-compare against the GH-Release tarball).

### When to fall back to the backup channel

- npmjs.org returns 5xx / DNS-NXDOMAIN / TLS errors for >5 min on a
  CI run that you don't want to block.
- An incident advisory is in effect for the npm registry or the
  `@atlas-trust` namespace specifically.
- An auditor wants to cross-check the bytes their organisation
  consumes against a second, independent download path before
  pinning a version into their lockfile.

If npmjs.org is reachable, use it — the backup channel is
operator-driven, not a transparent failover. There is **no** DNS-
or proxy-level rewrite that auto-redirects `npm install` to the
GitHub Release; the operator deliberately downloads the tarball,
verifies its SHA256, and `npm install`s the local file.

### Backup install flow

```bash
TAG="v1.15.0"     # the release tag you want
VERSION="${TAG#v}"  # strip the leading `v`

# 1. Download both targets and the SHA256 manifest from the GitHub
#    Release.
gh release download "${TAG}" \
  --pattern "atlas-trust-verify-wasm-${VERSION}-web.tgz" \
  --pattern "atlas-trust-verify-wasm-${VERSION}-node.tgz" \
  --pattern "tarball-sha256.txt" \
  --repo ThePyth0nKid/atlas

# (or, without gh CLI:)
curl -fsSLO "https://github.com/ThePyth0nKid/atlas/releases/download/${TAG}/atlas-trust-verify-wasm-${VERSION}-web.tgz"
curl -fsSLO "https://github.com/ThePyth0nKid/atlas/releases/download/${TAG}/atlas-trust-verify-wasm-${VERSION}-node.tgz"
curl -fsSLO "https://github.com/ThePyth0nKid/atlas/releases/download/${TAG}/tarball-sha256.txt"

# 2. Verify SHA256s match the manifest. Any mismatch means the
#    tarball was tampered with in transit OR the GH-Release asset
#    was replaced post-upload — STOP and escalate. Note this only
#    proves the tarball matches the manifest produced by the same
#    workflow run; step 3 is the origin-integrity check.
sha256sum --check tarball-sha256.txt

# 3. **MANDATORY origin-integrity check.** Cross-verify against the
#    npm-side `dist.integrity` (the SLSA L3 provenance trust root).
#    The npm registry uses SHA512 base64; the GH-Release manifest
#    uses SHA256 hex — different algorithms, both covering the same
#    tarball bytes, so they are not directly comparable. Use openssl
#    to compute SHA512 of the GH-Release tarball, then string-match
#    against npm's published `dist.integrity` field.
npm view @atlas-trust/verify-wasm@"${VERSION}" dist
# Expected output includes a `dist.integrity: "sha512-…"` field.
# Compute the matching SHA512 base64 on the GH-Release tarball:
GH_SHA512=$(openssl dgst -sha512 -binary "atlas-trust-verify-wasm-${VERSION}-web.tgz" \
  | base64 -w0)
echo "sha512-${GH_SHA512}"
# Bytes equal ⇒ both channels served the same artefact ⇒ the
# npm-side SLSA provenance attestation also covers the GH-Release
# tarball you just downloaded ⇒ proceed to step 4.
#
# Bytes NOT equal ⇒ the GH-Release tarball does not match what
# npm has on file. STOP. Do NOT proceed to step 4. Possible causes:
#   * npmjs.org and GitHub Releases were targeted in different
#     attacks (rare but the threat model includes it).
#   * The release-publishing workflow run itself was compromised
#     and pushed mismatched bytes to the two channels.
#   * Genuinely benign: a re-publish with the same version was
#     attempted but only one channel updated. Treat as a security
#     event until proven otherwise via the publishing team's audit
#     log.
#
# If npm is unreachable AND you cannot complete step 3, the
# fallback is verifier-side reproducibility from source: clone the
# repo at the tagged commit, run `wasm-pack build crates/atlas-verify-wasm
# --target web --release` with the pinned WASM_PACK_VERSION (see
# `.github/workflows/wasm-publish.yml`), and byte-compare the
# resulting .tgz against the GH-Release tarball. This is the
# ultimate trust root; it does not require either registry up.

# 4. Install from the local tarball. **`--ignore-scripts` is
#    REQUIRED on the install path** — npm's default behaviour is to
#    run any `install` / `postinstall` scripts inside the tarball
#    during install, before you have any opportunity to inspect the
#    contents. `@atlas-trust/verify-wasm` does not ship lifecycle
#    scripts (verify by inspecting the tarball's `package.json`
#    after step 3), but a tampered tarball that passed step 1 and
#    failed step 3 (or skipped step 3) could carry a malicious
#    postinstall. The flag is cheap defence-in-depth.
npm install --ignore-scripts ./atlas-trust-verify-wasm-${VERSION}-web.tgz
# (or the -node tarball, depending on your runtime target)
```

After step 4, your `package-lock.json` records the local-file
install. To restore the npm-registry source once the outage clears,
re-run `npm install @atlas-trust/verify-wasm@${VERSION}` and the
lockfile updates back to the registry path with the same
SHA512 integrity.

### Trade-offs documented for the audit trail

- **Operator-driven, not transparent.** The backup channel requires
  manual download + SHA256 verification + local install. By design:
  an automatic-failover proxy at `npm.atlas-trust.io` would itself
  be a single point of failure with its own compromise surface, and
  ownership would belong to the same operator team that runs the
  primary publish. V2-territory.
- **Both channels run on GitHub-hosted infrastructure.** The
  npmjs.org primary channel is independent of GitHub; the
  GH-Release backup is on the same provider as the source repo. So
  the backup hedges against npmjs-side failure but NOT against a
  GitHub-side outage. For both-failed scenarios, the ultimate
  fallback is verifier-side reproducibility from source — clone
  the repo at the tagged commit and run `wasm-pack build
  crates/atlas-verify-wasm --target web --release` locally; the
  output is byte-identical to the published artefact (pinned by
  the `WASM_PACK_VERSION` env var in `wasm-publish.yml`).
- **No registry-API equivalence.** The backup channel cannot answer
  `npm view`, `npm search`, or registry metadata queries. It is a
  raw-tarball download path only. Consumers using metadata-driven
  install logic (Renovate, Dependabot) need the primary channel up.

### Compose with §11

| Step | Side | Section | What happens |
|---|---|---|---|
| 1 | Auditor | §12 | Detect npmjs.org unreachable / advisory / cross-check intent |
| 2 | Auditor | §12 | `gh release download` (or `curl`) — fetch tarballs + manifest |
| 3 | Auditor | §12 | `sha256sum --check tarball-sha256.txt` — verify in-flight integrity |
| 4 | Auditor | §12 | Optional: cross-verify against `npm view … dist.integrity` if npm is reachable |
| 5 | Auditor | §12 | `npm install ./atlas-trust-verify-wasm-${VERSION}-web.tgz` |
| 6 | Auditor | — | Run verifier as normal — same exports, same bytes, same trust property |
| 7 | Auditor | §12 | When primary channel returns: `npm install @atlas-trust/verify-wasm@${VERSION}` to re-pin to the registry source |

---

## 13. Cutting a signed release tag (V1.17 Welle B)

Tag-Signing Enforcement is the V1.17 Welle B trust-stack addition: every
`v*` tag in this repo MUST be cryptographically signed by an SSH key
listed in [`.github/allowed_signers`](../.github/allowed_signers), and
both `wasm-publish.yml` and `verify-tag-signatures.yml` enforce this on
every tag push (plus `verify-tag-signatures.yml` re-verifies all
historical `v*` tags weekly via cron). An unsigned-or-untrusted-key
tag fails the publish lane BEFORE any `npm publish` step.

This section is the maintainer-side flow for cutting a signed tag.
Auditor-side verification of a published tag is in
[CONSUMER-RUNBOOK.md §6 step 2](CONSUMER-RUNBOOK.md#6-bypass-both-channels--rebuild-from-source).

For the threat model + design rationale (why SSH and not GPG /
Sigstore-gitsign for V1.17), see
[SECURITY-NOTES.md scope-l](SECURITY-NOTES.md).

### One-time setup (per maintainer machine)

```bash
# 1. Confirm your SSH public key is in the trust root.
#    The file is human-readable; check that one of your ~/.ssh/*.pub
#    keys (the "<base64>" middle field) appears in it.
cat .github/allowed_signers

# 2. Configure git for SSH-based tag signing in this checkout.
#    Picks the first ~/.ssh/*.pub key whose body matches an entry in
#    .github/allowed_signers. Errors out if no match.
bash tools/setup-tag-signing.sh init

# 2a. (Optional) If your signing key isn't in ~/.ssh/, pass it
#     explicitly:
bash tools/setup-tag-signing.sh init --key /path/to/my-signing.pub

# 3. Confirm the local config.
bash tools/setup-tag-signing.sh status
# Expected output:
#   gpg.format                     = ssh
#   user.signingkey                = /home/.../.ssh/id_ed25519.pub
#   tag.gpgSign                    = true
#   gpg.ssh.allowedSignersFile     = /path/to/.github/allowed_signers
```

### Cutting a signed tag

```bash
# 1. Cut the tag (annotated + signed). Because tag.gpgSign is true,
#    `git tag` defaults to signing — you do not need `-s` explicitly.
#    Use `-m` to set the tag message; the message becomes the GH
#    Release notes header on push.
git tag -m 'release v1.17.0' v1.17.0

# 2. Verify locally BEFORE pushing — catches a misconfigured local
#    signing key without spending a CI red-fail cycle.
bash tools/verify-tag-signatures.sh v1.17.0
# Expected output:
#   Verifying 1 tag(s) against .github/allowed_signers ...
#   ---
#     PASS: v1.17.0
#   ---
#   PASS: 1 / 1    FAIL: 0 / 1

# 3. Push the tag. This fires:
#    - wasm-publish.yml → builds + publishes to npm + GH-Releases
#      (after verifying the tag signature as its first step).
#    - verify-tag-signatures.yml → re-verifies all v* tags
#      (push-trigger surface, separate from wasm-publish.yml).
git push origin v1.17.0

# 4. Watch both workflows turn green at:
#    https://github.com/ThePyth0nKid/atlas/actions
```

### Adding a new maintainer key to the trust root

```bash
# 1. Add the key.
bash tools/setup-tag-signing.sh add new-maintainer@example.com /path/to/their.pub

# 2. Commit the trust-root update on master.
git add .github/allowed_signers
git commit -m 'chore(v1.17/welle-b): add maintainer key to allowed_signers'
git push origin master

# 3. The new maintainer can now run setup-tag-signing.sh init on their
#    machine (their key is in the trust root), and their tag pushes
#    will pass CI verification.
```

### Rotating vs. revoking a maintainer key

There are two distinct operations on the trust root, and they have
**opposite security semantics** — be deliberate about which one you
intend.

**Rotation (non-compromise) — preserve historical signatures.** The
maintainer wants to switch to a new signing key (e.g. moving from a
software key to a FIDO2 hardware token, or replacing a key on a
retired laptop) without invalidating tags signed by the old key.

```bash
# 1. ADD the new key first.
bash tools/setup-tag-signing.sh add nelson@ultranova.io /path/to/new.pub

# 2. Commit + push.
git add .github/allowed_signers
git commit -m 'chore(v1.17/welle-b): rotate maintainer key (add new)'
git push origin master

# 3. Configure local git to use the new key.
bash tools/setup-tag-signing.sh init --key /path/to/new.pub

# 4. LEAVE the old key in `.github/allowed_signers`. Past tags signed
#    by the old key continue to verify (CI cron stays green), and the
#    old key cannot sign new tags from your machine anyway because
#    `git config user.signingkey` now points at the new key. The old
#    key remaining in the trust root is not a security regression: a
#    public key cannot sign anything without its corresponding private
#    key, which only you control.
```

**Revocation (compromise) — invalidate historical signatures.** The
old key is suspected or known compromised. We accept that the weekly
cron will fail-loud on every past tag signed by the revoked key, and
that's the *intended* alarm — those tags are no longer trusted.

```bash
# 1. Edit .github/allowed_signers — delete the line containing the
#    compromised key's body (whole line, including principal).

# 2. Commit + push the trust-root update.
git add .github/allowed_signers
git commit -m 'chore(v1.17/welle-b): revoke <principal> key (compromise)'
git push origin master

# 3. From this commit forward:
#    - any new v* tag push by the revoked key fails
#      wasm-publish.yml's first-step verification, blocking publish.
#    - the weekly verify-tag-signatures.yml cron starts failing on
#      every PAST v* tag signed by the revoked key. This is the
#      intended alarm — those tags are now untrusted by design. Do
#      NOT silence the cron; investigate and re-publish from a new
#      tag signed by a non-compromised key.

# 4. To hard-invalidate a specific known-bad past tag (e.g. one the
#    attacker pushed during the compromise window):
#    - Force-push-delete the bad tag (requires admin access + tag-
#      protection-rule override; see SECURITY-NOTES.md scope-e).
#    - Re-publish from a new tag signed by a non-compromised key.
#    - Document the rotation in the team's release log.
```

### Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `git tag -m '…' v1.17.0` succeeds but `verify-tag-signatures.sh v1.17.0` says FAIL | Local key configured but its public counterpart not in `.github/allowed_signers` | `bash tools/setup-tag-signing.sh add <principal> <pubkey>` then commit + push |
| `git tag` errors with `gpg.format = ssh, but no user.signingkey set` | One-time setup not done | `bash tools/setup-tag-signing.sh init` |
| CI `wasm-publish.yml` step "Verify tag signature" fails red | Tag was pushed without local signing config OR signing key not in trust root | Confirm `tools/setup-tag-signing.sh status` matches CI's `.github/allowed_signers`; re-cut + force-push the tag (or push a corrected new patch tag, e.g. `v1.17.1`) |
| Weekly cron `verify-tag-signatures.yml` fails on a previously-green tag | Trust root was edited (key removed) AFTER the tag was signed | Re-add the key OR (for a deliberate revocation) accept the cron failure — historical tag is now untrusted by design |

### Why SSH (not GPG, not Sigstore/gitsign)

See [SECURITY-NOTES.md scope-l](SECURITY-NOTES.md). One-line summary:
SSH keys are already in maintainers' GitHub accounts, git 2.34+ has
first-class SSH signing support without plugins, and the trust root
is in-repo (auditable) rather than pinned to an external OIDC
issuer's certificate transparency log. Sigstore/gitsign is a
plausible additive enhancement (V1.18 candidate) — not a replacement.

---

## 14. Trust-root mutation defence — branch-protection requirements (V1.17 Welle C)

V1.17 Welle C (scope-m) ships an in-repo cryptographic gate that
requires any commit modifying a trust-root-protected file to be
signed by a key in the **prior** trust root (i.e.
`.github/allowed_signers` as it existed at the PR base). The full
threat model + design rationale is in
[SECURITY-NOTES.md scope-m](SECURITY-NOTES.md).

The in-repo gate (`tools/verify-trust-root-mutations.sh` +
`.github/workflows/verify-trust-root-mutations.yml`) only fires on
`pull_request` events. Two operator-side configurations are required
for the defence to actually bind:

  1. **No direct push to `master`** (including admins).
  2. **Required status check** on `master` for
     `verify-trust-root-mutations / Verify trust-root-modifying commits`.
  3. **Required CODEOWNERS review** on `master`.
  4. **Required signed commits** on `master` (defence-in-depth).

If branch protection allows direct pushes to `master`, an attacker
with `contents: write` PAT can push a commit modifying
`.github/allowed_signers` directly — bypassing the PR-only gate.
This is **not** a defect in the workflow; it's a structural limit
that operator-side branch protection has to close. This section
documents the configuration.

### Required GitHub branch-protection settings

Settings → Branches → Branch protection rules → `master` (or
`main`). Apply these exact toggles:

```
Branch name pattern: master

[x] Require a pull request before merging
    [x] Require approvals → 1
    [x] Dismiss stale pull request approvals when new commits are pushed
    [x] Require review from Code Owners

[x] Require status checks to pass before merging
    [x] Require branches to be up to date before merging
    Required status checks (search + add):
      - verify-trust-root-mutations / Verify trust-root-modifying commits

    NOTE: do NOT require `verify-tag-signatures / Verify all v* tags`
    on every PR. That workflow uses `pull_request: paths:` filtering
    (fires only on PRs touching tag-signing surfaces). If pinned as
    a required check on every PR, doc-only / unrelated-code PRs will
    be blocked-forever because the check never reports. Welle-B-
    defence comes from `wasm-publish.yml`'s first-step gate on tag
    push and the standalone `verify-tag-signatures.yml` workflow on
    tag push + weekly cron + dispatch + paths-PR — none requires
    PR-gating to function.

[x] Require conversation resolution before merging

[x] Require signed commits

[x] Do not allow bypassing the above settings
    (the "Include administrators" toggle on classic rules,
     OR the "Restrict who can dismiss / bypass" config on rulesets)

[ ] Allow force pushes        ← MUST stay UNCHECKED (incl. admins)
[ ] Allow deletions           ← MUST stay UNCHECKED
```

The "Do not allow bypassing the above settings" / "Include
administrators" toggle is **load-bearing**. Without it, an admin-
role compromise can bypass every rule above, and scope-m's in-repo
gate is not reachable on a direct push.

### Why each setting matters

| Setting | What it closes | What's left if missing |
|---|---|---|
| Require pull request before merging | Direct push to master bypasses verify-trust-root-mutations.yml entirely | Attacker with PAT + `contents: write` modifies trust root via direct push. scope-m never runs. |
| Require review from Code Owners | A single compromised PAT can self-approve | Single-credential takeover lets the same identity that authored the malicious change merge it. |
| Required status check: verify-trust-root-mutations | Workflow can be skipped or paused without merge being blocked | Workflow runs but fails open — green merge with a failing or absent verification job. |
| Required status check: verify-tag-signatures | Tag-signing enforcement on PRs touching trust-root surfaces is not gating | scope-l's PR-mode signal becomes documentary rather than enforcing. |
| Require signed commits | Unsigned commits land on master, breaking scope-m's "merge-base trust root" semantic for downstream PRs | scope-m bootstraps from a polluted base. |
| No force pushes (incl. admins) | An admin force-push can rewrite `.github/allowed_signers` history | Anti-rewrite guard in the verifier catches this *if* it ever runs, but a force-push bypassing PR review never triggers the workflow. |
| No deletions | Deleting + recreating master defeats branch protection | Full bypass of every rule above. |

### Verifying the configuration is correctly applied

```bash
# Requires admin scope on the repo. Run from any clone.
gh api -H "Accept: application/vnd.github+json" \
  /repos/ThePyth0nKid/atlas/branches/master/protection | \
  jq '{
    required_pull_request_reviews,
    required_status_checks: .required_status_checks.contexts,
    required_signatures,
    enforce_admins: .enforce_admins.enabled,
    allow_force_pushes: .allow_force_pushes.enabled,
    allow_deletions: .allow_deletions.enabled
  }'
```

Expected fields:

- `required_pull_request_reviews.required_approving_review_count` ≥ 1
- `required_pull_request_reviews.require_code_owner_reviews` = `true`
- `required_status_checks` contains
  `"verify-trust-root-mutations / Verify trust-root-modifying commits"`
  (and ONLY that — see note above re `verify-tag-signatures`)
- `required_signatures.enabled` = `true`
- `enforce_admins.enabled` = `true`
- `allow_force_pushes.enabled` = `false`
- `allow_deletions.enabled` = `false`

If any of these drift from expected, scope-m's defence is partially
or fully bypassable. Restore the configuration via the Settings UI
or `gh api PATCH`.

### Modern-Rulesets equivalent (recommended path)

GitHub now recommends **Repository Rulesets** over classic Branch
Protection for new configurations. The Atlas-2026-05-05 production
configuration uses a Ruleset (id `15986324`, name "Master trust-root
protection"). The mappings between the classic-protection settings
above and the Ruleset rule types are:

| Classic setting | Ruleset rule type / parameter |
|---|---|
| Require pull request before merging | `pull_request` rule |
| Require approvals → 1 | `pull_request.required_approving_review_count` (see solo caveat below) |
| Require review from Code Owners | `pull_request.require_code_owner_review: true` |
| Dismiss stale approvals on push | `pull_request.dismiss_stale_reviews_on_push: true` |
| Require status checks | `required_status_checks` rule |
| (status check contexts) | `required_status_checks.required_status_checks[].context` |
| Require branches up-to-date | `required_status_checks.strict_required_status_checks_policy: true` |
| Require signed commits | `required_signatures` rule |
| Allow force pushes (UNCHECKED) | `non_fast_forward` rule (= force-push BLOCKED) |
| Allow deletions (UNCHECKED) | `deletion` rule (= deletion BLOCKED) |
| Include administrators / Do not allow bypassing | `bypass_actors: []` (empty list) |

Verify a Ruleset configuration with:

```bash
gh api -H "Accept: application/vnd.github+json" \
  repos/ThePyth0nKid/atlas/rulesets/15986324
```

Expect `enforcement: active`, `bypass_actors: []`,
`current_user_can_bypass: never`, and the six rule types listed in
the mapping table above.

### Solo-maintainer caveats

The recommended `Require approvals → 1` value above is correct for a
multi-maintainer team. **For a solo-maintainer repository (Atlas's
current state — single maintainer `@ThePyth0nKid` for V1.0 through
V1.17)**, that value should be `0`, not `1`. The Atlas-2026-05-05
configuration uses
`pull_request.required_approving_review_count: 0`. The reasons:

  1. **GitHub does not allow self-review of a PR you authored.** With
     a single maintainer and `required_approving_review_count: 1`,
     every PR — even trivial ones — would be unmergeable without
     temporarily disabling the Ruleset. That defeats the entire
     point of the Ruleset.
  2. **Welle C's defence does NOT depend on the generic approval
     count.** It depends on `require_code_owner_review: true` +
     `.github/CODEOWNERS` pinning the PROTECTED_SURFACE to
     `@ThePyth0nKid`. CODEOWNERS-required-review IS the two-identity
     requirement for trust-root mutation; the generic approval count
     is orthogonal.
  3. **Defence-in-depth for non-trust-root PRs comes from
     `required_signatures`** — a PAT-takeover attacker still has to
     produce SSH-signed commits with a key in the in-repo trust
     root, which they don't have.

`require_last_push_approval` MUST also be `false` for solo-maintainer.
GitHub interprets `true` as "the last pusher cannot be the same
person who provides approval"; with a single maintainer that
condition is never satisfiable, so every PR is blocked-forever
regardless of `required_approving_review_count`. The Atlas
configuration uses `pull_request.require_last_push_approval: false`.

**Required-status-check context-name format gotcha.** The string you
enter as a required status check MUST match the *job name* GitHub
Actions reports the check-run under — typically just the job name
(e.g. `Verify trust-root-modifying commits`), NOT the
`<workflow-name> / <job-name>` form (e.g. `verify-trust-root-
mutations / Verify trust-root-modifying commits`). The classic
branch-protection UI sometimes auto-suggests the
`workflow-name / job-name` form, but the Ruleset behaviour is
context-name-must-equal-the-reported-check-name. A mismatch leaves
the required check perpetually "expected" even when it has run
green, blocking merge. Verify with:

```bash
gh api repos/ThePyth0nKid/atlas/commits/<sha>/check-runs | \
  jq -r '.check_runs[].name'
```

— and use the exact strings that come back as the required-check
contexts in the Ruleset.

**Side-effect to be aware of:** PROTECTED_SURFACE files (the 10
entries in `tools/verify-trust-root-mutations.sh`'s
`PROTECTED_SURFACE` array + the `.github/actions/verify-wasm-pin-
check/` subtree) are **factually frozen for solo-maintainer**: the
solo maintainer cannot self-approve a PR that hits CODEOWNERS,
because GitHub disallows self-review on code-owner-required PRs.
This is *by design* under Welle C's threat model — the assumption
is that trust-root surfaces change rarely, and any change must
involve an out-of-band recovery procedure.

**Recovery path** (rare, expected ≤1× per year):

```bash
# 1. Open the Settings → Rules → Rulesets UI for the repo.
# 2. Click "Master trust-root protection" → set Enforcement to
#    "Disabled" (NOT "Evaluate" — that still gates merges).
# 3. Make + sign + push the trust-root-modifying commit on a
#    feature branch, open the PR, ensure
#    verify-trust-root-mutations CI passes (the in-repo gate
#    still fires on PR; only the Ruleset's status-check
#    requirement is paused).
# 4. Self-merge (no CODEOWNERS-required-review-blocker because the
#    Ruleset is disabled).
# 5. IMMEDIATELY re-enable the Ruleset: Settings → Rules →
#    Rulesets → "Master trust-root protection" → Enforcement
#    "Active".
# 6. Log the operation in your maintainer logbook (date, PR number,
#    SHA, reason).
```

This recovery path is auditable: every disable/enable transition
shows up in the repo's Audit Log under "ruleset.update" events. A
review of the audit log reveals every time the Ruleset was paused
and for how long. Pair this with a maintainer-side discipline of
"only ever disable for the minimum interval required" — disable,
push, re-enable — and the recovery path remains a non-bypassable
gap in attacker-time-budget terms.

When (if) Atlas grows to multiple maintainers, raise
`required_approving_review_count` to `1` via Settings → Rules →
Rulesets → "Master trust-root protection" → "Require a pull request
before merging" → "Required approvals" — single-click change, no
code edit required.

### Adding a new file to the protected surface

The PROTECTED_SURFACE list in `tools/verify-trust-root-mutations.sh`
and the entries in `.github/CODEOWNERS` are kept in sync by the
anti-drift harness (`tools/test-trust-root-mutations.sh` test 7).

To add a new file:

```bash
# 1. Add to PROTECTED_SURFACE in the verifier script.
#    Edit tools/verify-trust-root-mutations.sh around the
#    "PROTECTED_SURFACE=(" block.

# 2. Add to .github/CODEOWNERS with @ThePyth0nKid as the owner
#    (or whichever team is the trust-root authority).

# 3. Confirm parity holds.
bash tools/test-trust-root-mutations.sh
# Expected: PASS: 17 / 17 (or N+1 with the new test if you added one).

# 4. Commit + PR. The PR itself will be gated by scope-m because
#    you're modifying tools/verify-trust-root-mutations.sh
#    (already in PROTECTED_SURFACE). The PR commits must be signed
#    by a key in the prior trust root.
git add tools/verify-trust-root-mutations.sh .github/CODEOWNERS
git commit -m 'chore(v1.17/welle-c): add <path> to protected surface'
```

If test 7 (PROTECTED_SURFACE / CODEOWNERS parity) fails after
your edit, one of the two lists drifts from the other. Fix the
drift before pushing.

### Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| PR modifying `.github/allowed_signers` merges without `verify-trust-root-mutations` running | Workflow not configured as required status check | Add it to branch protection (see above). |
| `verify-trust-root-mutations` reports `FAIL: bootstrap mode triggered` | `.github/allowed_signers` was deleted from master in a force-push | Investigate. Anti-rewrite guard correctly fired. Likely a takeover scenario or accidental admin action. |
| `verify-trust-root-mutations` reports `FAIL: commit not signed by a trusted key` | PR commit signed by a key not in the prior trust root | Re-sign with a trusted key, or first add the new key in a prior PR signed by an existing trusted key. |
| Test 7 (`PROTECTED_SURFACE / CODEOWNERS parity`) fails locally | Drift between the two lists | Sync them per the "Adding a new file" recipe above. |
| Admin pushes directly to master and bypasses everything | "Include administrators" toggle disabled | Re-enable it; review what landed during the bypass window. |

---

## 15. Inline-pin-update protocol — `SIGSTORE_REKOR_V1.pem` / `tree_id_roster` (V1.18 Welle B)

The Sigstore Rekor v1 trust root is pinned **inline** in
`crates/atlas-trust-core/src/anchor.rs` as
`SIGSTORE_REKOR_V1.pem` (the production log's P-256 SPKI public key)
and `SIGSTORE_REKOR_V1.tree_id_roster` (the active shard plus the two
known historical shards). Both fields are part of `PROTECTED_SURFACE`
— any edit must traverse `verify-trust-root-mutations` (§14) and
CODEOWNERS-required review.

This section closes the protocol gap that
[ADR-Atlas-006 §5.2](ADR/ADR-Atlas-006-multi-issuer-sigstore-tracking.md)
identified: the *when* of an inline-pin update is well-defined
(Sigstore root ceremony, Rekor v2 launch, shard rotation, multi-issuer
adoption), but the *how* — review requirements, golden-fixture
regeneration, cross-version-anchor compatibility — was implicit.
[CONSUMER-RUNBOOK §10.6](CONSUMER-RUNBOOK.md) closure step 4 references
this section as the canonical operator path for closing a Trigger-C
incident on the consumer side.

> ⚠️ **Recipe-text caveat — read BEFORE executing any step below.**
> OPERATOR-RUNBOOK.md (this file) is itself NOT in `PROTECTED_SURFACE`
> and is not gated by §14's cryptographic verifier. The integrity of
> the §15 recipe text relies on normal branch-protection + CODEOWNERS
> reviewer scrutiny on every docs PR — *not* on the trust-root mutation
> gate. An attacker who rewrote §15 mid-incident to point operators at
> a malicious key would still hit §14 when trying to land that key in
> `anchor.rs`, but a vigilant reviewer is required to catch the rewrite
> of the recipe itself before it misleads an in-incident operator. **An
> operator following §15 during an actual rotation event SHOULD verify
> the recipe text against the V1.18 Welle B (4) snapshot (commit
> `2171b75`, plus any later `docs(v1.18/welle-b-N-followup)` commits
> visible in `git log -- docs/OPERATOR-RUNBOOK.md`) before executing.**
> Treat §15 edits in any future PR with the same scrutiny as a
> `PROTECTED_SURFACE` edit, even though the cryptographic gate does not
> apply to it.

### When this protocol fires

| Trigger | Source | Pin fields touched | Verification bar |
|---|---|---|---|
| **Planned** Sigstore Rekor v1 root ceremony (scheduled key rotation) | Sigstore Foundation planned-ceremony announcement (≥7 days advance notice typical) | `SIGSTORE_REKOR_V1.pem` (the PEM body changes; `name`/`origin` stay) | Standard: cite the foundation announcement in the PR body. Cross-check optional but recommended (announcement + bundled `trusted_root.json` in next npm release). |
| **Emergency / unscheduled** PEM rotation (out-of-schedule key rotation, e.g. post-incident or compromise response) | Sigstore Foundation post-mortem OR security-advisory issuance | `SIGSTORE_REKOR_V1.pem` (same field shape as planned ceremony) | **Mandatory two-source minimum.** An out-of-schedule rotation has fewer corroboration channels than a planned ceremony — require BOTH the foundation post-mortem URL AND ≥1 independent corroboration (≥2 maintainer accounts on different platforms confirming the new key fingerprint, or a published GitHub security advisory under `sigstore/rekor`). Cite both in the PR body. Do NOT rotate on a single-source signal. |
| Active-shard rotation (new tree-ID promoted) | Sigstore Rekor operations channel — new logIndex range, prior shard frozen | `SIGSTORE_REKOR_V1.active_tree_id` + prepend the new tree-ID at `tree_id_roster[0]`, push the prior active to index 1 | Standard: cite operations-channel post; cross-check the new shard's first logIndex appears AFTER prior shard's last logIndex (see PR review row "Fixture freshness on PEM AND shard rotation"). |
| Historical-shard discovery (a missing shard surfaces during an audit) | Independent discovery; cross-checked against rekor-monitor or the Sigstore Foundation public statement | `tree_id_roster` (append the historical shard, preserving index 0 = active) | Standard: cite the discovery source + a fixture proving the historical shard's anchors verify under the existing pinned PEM. |
| Rekor v2 launch (ADR-006 Trigger B) | Rekor v2 GA + npm trust root update | New `SIGSTORE_REKOR_V2: RekorIssuer` static + extend `REKOR_ISSUERS = &[&SIGSTORE_REKOR_V1, &SIGSTORE_REKOR_V2]` (per-issuer registry shape from V1.18 Welle B (2)) | Standard + ADR-006 amendment in the same PR (see paragraph below). |
| Second issuer joining (ADR-006 Trigger A/D) | Sigstore Foundation federation announcement OR Atlas-side compliance requirement | New `RekorIssuer` static + `REKOR_ISSUERS` slice extension (same shape as Rekor v2) | Standard + ADR-006 amendment in the same PR. |

The first four are routine rotations within the existing Rekor v1
trust scope (the planned-vs-emergency split changes only the
verification bar, not the pin-edit shape); the last two are full
ADR-006 adoption events and require a follow-on ADR amendment in the
same PR (§5 → §6 transition: pin update lands together with the design
that justifies it).

### Pre-edit verification

Run all anti-drift gates before opening the PR. A red gate before the
edit means there is preexisting drift to investigate — do **not** mask
it under the rotation:

```bash
cargo test -p atlas-trust-core --test sigstore_golden -- --nocapture
cargo test -p atlas-trust-core anchor::tests::rekor_issuer_rosters_are_pinned
cargo test -p atlas-trust-core anchor::tests::rekor_issuer_tree_id_membership
bash tools/test-trust-root-mutations.sh
```

Expected outputs (V1.18 baseline):

- `sigstore_golden` — 6/6 PASS (`fixture_log_id_matches_pinned`,
  `verifies_real_sigstore_rekor_entry`, `tampered_entry_body_is_rejected`,
  `unknown_tree_id_is_rejected`, `historical_shard_tree_id_passes_dispatch_gate`,
  `anchored_hash_forgery_is_rejected`).
- `rekor_issuer_rosters_are_pinned` — PASS (the per-issuer expected-pin
  block matches today's source).
- `rekor_issuer_tree_id_membership` — PASS.
- `tools/test-trust-root-mutations.sh` — `PASS: 17 / 17` (or N+1 with
  any newer test).

### Step-by-step pin update

```bash
# 1. Branch from a known-clean master. Branch name uses the
#    chore(...) prefix because rotating SIGSTORE_REKOR_V1 is a
#    PROTECTED_SURFACE source-file edit (anchor.rs), not a docs change
#    — keep the branch prefix and the conventional-commit prefix below
#    in step 7 in lockstep so an auditor reading either can derive the
#    other. ROTATION_ID is operator-chosen — recommend YYYYMMDD or
#    a short incident reference (e.g. `2026-05-12` or `sigstore-pmi-042`).
git checkout master
git pull --ff-only
git checkout -b "chore(v1.x/welle-x)/sigstore-pin-update-${ROTATION_ID}"

# 2. Edit the SIGSTORE_REKOR_V1 static in
#    crates/atlas-trust-core/src/anchor.rs. Change ONLY the field(s)
#    the trigger demands:
#      - PEM rotation:   pem field
#      - shard rotation: active_tree_id + tree_id_roster (index 0 == active)
#      - historical:     tree_id_roster (append; index 0 untouched)
#    Every other field stays byte-identical. Multi-field churn under a
#    single rotation muddies the diff and the audit trail.

# 3. Update the per-issuer expected-pin block in the
#    rekor_issuer_rosters_are_pinned test in the same file (the
#    "sigstore-rekor-v1" arm). Source-of-truth is the static itself;
#    the test exists to force the change to surface in code review,
#    so it MUST be updated in the same commit. A drift between the
#    static and the expected-pin arm fails the test red — that IS the
#    audit signal, not a bug.

# 4. Regenerate the Sigstore golden fixture (Trigger A/B only —
#    PEM rotation invalidates the captured signature; shard
#    rotations do NOT, since the active shard's prior signatures
#    remain valid under the prior key).
#
#    For PEM rotation:
bash tools/regenerate-sigstore-fixture.sh \
  --log-index 800000000 \
  --out crates/atlas-trust-core/tests/fixtures/sigstore_rekor_v1_logindex_800000000.json
#
#    For shard rotation (new active shard): capture a fresh fixture
#    from the new active shard so the historical shard's continued
#    acceptance is also exercised by historical_shard_tree_id_passes_dispatch_gate:
bash tools/regenerate-sigstore-fixture.sh \
  --log-index <newly-promoted-shard-logIndex> \
  --out crates/atlas-trust-core/tests/fixtures/sigstore_rekor_v1_logindex_<N>.json

# 5. Run the cross-version-anchor compatibility test (see next sub-section).
#    The script materialises the prior-version Sigstore Rekor v1
#    fixture corpus from --legacy-ref (default origin/master) and
#    runs `cargo test -p atlas-trust-core --test sigstore_golden`
#    against each fixture under the WORKING-TREE pin (your edited
#    anchor.rs). Every prior fixture must verify or the rotation
#    breaks consumer-side replay (ADR-006 §8.3) — exit 1 means STOP,
#    do not ship.
#
#    Note: --legacy-ref must be the rotation branch's BASE ref (the
#    last commit before this rotation diverged), not a future master
#    that already contains your rotation. On a typical rotation PR
#    branched from current master, the default origin/master is
#    correct as long as you have run `git fetch origin master`.
#
#    On Windows hosts the cargo binary is typically outside the bash
#    PATH; set CARGO (or pass --cargo) to point at it explicitly. The
#    quoted form below survives shell expansion of the angle brackets:
#      CARGO='/c/Users/<user>/.cargo/bin/cargo.exe' \
#        bash tools/cross-version-anchor-compat.sh
bash tools/cross-version-anchor-compat.sh

# 6. Re-run all anti-drift gates from the pre-edit verification.
#    All must be GREEN before push.

# 7. Commit. Use a SSH-signed commit (the PROTECTED_SURFACE write
#    requires it via §14). Single commit per rotation — multi-rotation
#    PRs are rejected (one rotation = one auditable trust-property
#    delta).
git add crates/atlas-trust-core/src/anchor.rs \
        crates/atlas-trust-core/tests/fixtures/
git commit -S -m "$(cat <<'EOF'
chore(v1.x/welle-x): rotate SIGSTORE_REKOR_V1 <field> per <trigger>

Trigger: <Sigstore Foundation post-mortem URL OR root-ceremony
announcement OR ADR-006 §5.2 Trigger N>

Fields changed:
  - SIGSTORE_REKOR_V1.<field>: <old value (truncated)> → <new value>

Cross-references:
  - Sigstore Foundation source: <URL>
  - Golden-fixture regeneration: <new logIndex>
  - Anti-drift gates: 6/6 GREEN, 17/17 GREEN
EOF
)"

# 8. Push, open PR. The PR is gated by:
#    - verify-trust-root-mutations (§14) — must be signed by a key in
#      the prior trust root.
#    - CODEOWNERS-required review on the PROTECTED_SURFACE.
#    - Required status check: hsm-byte-equivalence (signer side
#      unaffected by Rekor pin, but the lane proves no collateral
#      damage).
git push -u origin HEAD
```

### PR review requirements

Reviewers MUST verify each of the following before approval. The list
exists because each item closes a class of mistake that the gates
above cannot catch on their own:

| Review item | What it catches | How to verify |
|---|---|---|
| Source-of-truth match | The new pin matches the cited Sigstore Foundation announcement byte-for-byte (PEM, tree-ID magnitude, origin string) | Open the cited URL; copy the field; `diff` against the diff hunk |
| Single-field discipline | Only the trigger's field changed; no opportunistic edits to `name`/`origin`/other fields; no homoglyph / zero-width / RTL-override character smuggled into another field's value | **Mandatory:** run `git diff HEAD~1 -- crates/atlas-trust-core/src/anchor.rs \| cat -A` (or `git diff … \| od -c \| less`) and review the output. Rendered diff views (web UI, IDE) suppress non-printing and visually-confusable characters; an attacker who controls the operator's editor can present a one-field diff that is actually a two-field diff. The cryptographic gate (§14) validates the commit signature, not the content delta — only a byte-level diff catches this class. Eyeballing the rendered diff alone is insufficient. |
| Test arm parity | `rekor_issuer_rosters_are_pinned`'s `"sigstore-rekor-v1"` match arm changed in lockstep with the static | Diff hunks must include both `anchor.rs` static AND the test arm |
| Index-0 invariant on shard rotation | After rotation, `tree_id_roster[0] == active_tree_id` | The test enforces this — but a reviewer should still see it explicit in the diff |
| Fixture freshness on PEM AND shard rotation | A new fixture was captured AFTER the rotation event reached the production log: for PEM rotation, after the new key was activated; for shard rotation, after the prior shard was frozen AND the new shard had landed at least one hashedrekord entry | `fetched_at_unix` in the fixture > the announced rotation timestamp. **For shard rotation:** additionally cross-check that the new shard's first hashedrekord logIndex appears AFTER the prior shard's last logIndex in the Sigstore Foundation operations channel (or in `rekor-monitor` output if maintained). A fixture captured during the shard-overlap window can validate against both shards and mask a half-completed rotation. |
| ADR cross-reference | The PR description links the Sigstore source AND ADR-006 (and adds an ADR amendment if Trigger A/B/D fired) | PR body must contain both URLs |
| Cross-version-anchor compat | `cross-version-anchor-compat.sh` PASS log attached to the PR | CI artifact OR pasted output in PR body |

A PR that fails any of these MUST be requested-changes, NOT
approved-with-comment. The PROTECTED_SURFACE gate (§14) is the
cryptographic last line of defence for the source-code pin in
`crates/atlas-trust-core/src/anchor.rs` — reviewer rigour is the
load-bearing one for the rotation as a whole. (The §15 recipe-text
caveat at the top of this section reminds why reviewer rigour cannot
be substituted by the cryptographic gate for the runbook itself.)

### Golden-fixture regeneration

`tools/regenerate-sigstore-fixture.sh` is the one-shot helper that
captures a hashedrekord entry from `rekor.sigstore.dev/api/v1/log/
entries?logIndex=<N>`, normalises it into the Atlas
`tests/fixtures/sigstore_rekor_v1_logindex_<N>.json` shape (matching
the `Fixture` struct in
`crates/atlas-trust-core/tests/sigstore_golden.rs`), and stamps
`source` + `fetched_at_unix` so the provenance is auditable from a
clone alone (no Sigstore round-trip required to re-verify the capture
chain).

The script is intentionally idempotent and offline-after-fetch — the
captured JSON is the entire fixture; no derived data is computed at
test time. An auditor reproduces the capture by re-fetching the
`source` URL and `diff`-ing against the checked-in JSON.

If the script does not yet exist at the time of a rotation (it is
slated to ship alongside the first real rotation event), the manual
capture procedure is:

```bash
# 1. Pick a logIndex on the relevant shard, ideally a hashedrekord
#    entry (the test's anti-forgery check is hashedrekord-only).
LOG_INDEX=800000000  # or the new active shard's first hashedrekord

# 2. Fetch the entry. The API returns a single-key map keyed by uuid;
#    flatten it to the inner object plus a top-level uuid for easier
#    indexing.
curl -fsSL \
  "https://rekor.sigstore.dev/api/v1/log/entries?logIndex=${LOG_INDEX}" \
  -o /tmp/rekor-raw.json

# 3. Normalise into the Atlas fixture shape. Field mapping:
#      body, anchored_hash, log_id ← from the entry's body decode
#      tree_id ← from logID's tree_id portion (hex → i64)
#      tree_size, root_hash, hashes, checkpoint_sig
#        ← from verification.inclusionProof + signedEntryTimestamp
#    The current sigstore_rekor_v1_logindex_800000000.json fixture
#    is the canonical example.
#
#    NOTE: tools/normalize-rekor-fixture.js is slated to ship
#    alongside tools/regenerate-sigstore-fixture.sh — it does not
#    exist in V1.18 today. Until it ships, perform the field-
#    mapping by hand using the existing fixture as the schema
#    template, OR write the transform inline (jq + node) in the
#    rotation PR itself and remove the shell-out to the missing
#    helper.
node tools/normalize-rekor-fixture.js \
  /tmp/rekor-raw.json \
  > crates/atlas-trust-core/tests/fixtures/sigstore_rekor_v1_logindex_${LOG_INDEX}.json

# 4. Stamp source + fetched_at_unix (jq, in place).
jq --arg src "https://rekor.sigstore.dev/api/v1/log/entries?logIndex=${LOG_INDEX}" \
   --argjson now $(date +%s) \
   '. + {source: $src, fetched_at_unix: $now}' \
   crates/atlas-trust-core/tests/fixtures/sigstore_rekor_v1_logindex_${LOG_INDEX}.json \
   > /tmp/fixture.json
mv /tmp/fixture.json \
   crates/atlas-trust-core/tests/fixtures/sigstore_rekor_v1_logindex_${LOG_INDEX}.json
```

The rotation PR MUST include the new fixture file in the same commit
as the pin edit. Splitting them across commits breaks `git bisect` on
the rotation: bisecting through the gap lands on a commit where the
test harness is internally inconsistent.

### Cross-version-anchor compatibility test

The verifier ships pinned roots that are read by **every** trace bundle
the verifier evaluates, including bundles produced by Atlas builds
from before the rotation. A pin update that breaks acceptance of a
prior-version anchor would silently invalidate the verifier's
historical replay property.

The compatibility test re-runs the verifier against a corpus of
fixtures captured from prior Atlas releases (one per minor version
since V1.6, when Sigstore anchoring shipped). Each fixture exercises
a different combination of (signer build, anchor shard, log key) so
the rotation's blast radius is bounded by the corpus and visible in
the diff.

`tools/cross-version-anchor-compat.sh` (V1.18 Welle B (8) —
[PROTECTED_SURFACE](#14-trust-root-mutation-defence)) enumerates the
legacy fixture corpus from a git ref (default `origin/master`),
materialises each fixture into the canonical on-disk path, and runs
`cargo test -p atlas-trust-core --test sigstore_golden` against each
under the working-tree `anchor.rs`. The on-disk fixture is restored
on EXIT/INT/TERM via a trap, so an interrupted run leaves no debris.
Exit codes: `0` = every legacy fixture PASS; `1` = at least one
legacy fixture FAIL (cross-version compat broken — STOP); `2` =
misconfiguration (bad `--legacy-ref`, missing canonical fixture, no
fixtures found at the ref, cargo not callable, env-var override
attempted under `GITHUB_ACTIONS=true`).

```bash
# Default invocation (uses origin/master as the legacy ref). On
# Windows hosts cargo lives outside the bash PATH; set CARGO or
# pass --cargo. Quote the value so the shell does not expand the
# literal `<user>` placeholder as a redirection or globbing token.
CARGO='/c/Users/<user>/.cargo/bin/cargo.exe' \
  bash tools/cross-version-anchor-compat.sh

# Explicit legacy ref (e.g. when the rotation branched from a tag):
bash tools/cross-version-anchor-compat.sh --legacy-ref v1.17.0
```

Expected output: every prior-version fixture PASSES under the new pin.
Failure mode interpretation:

| Failure | Cause | Required action |
|---|---|---|
| Prior-version fixture fails on `log_id_not_trusted` | The pin update accidentally rotated `name` or removed an issuer entry | Reverse the unintended field change |
| Prior-version fixture fails on `unknown_tree_id` | A historical shard was dropped from `tree_id_roster` | Re-add the dropped historical shard — historical shards are append-only |
| Prior-version fixture fails on `checkpoint_signature_invalid` | The PEM was rotated, but the prior key's PEM was not preserved | **STOP.** Multi-issuer support is required (see ADR-006 §5.2 and §8.3) — verifier MUST keep verifying old anchors under the prior key. Do not ship the rotation; open a follow-on ADR for the multi-key issuer pattern |
| Prior-version fixture fails on `tampered_entry_body` | The fixture itself was edited, not the verifier | Restore the fixture from `git`; the rotation does not break verification |

The third row is the load-bearing one. ADR-006 §8.3 records this as
an open question (interop between pinned-Rekor-v1 verifier and
multi-issuer-only consumer); the cross-version test is the gate that
forces the question to be answered before the rotation lands, not
after a downstream auditor finds historical anchors no longer verify.

### Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `cargo test rekor_issuer_rosters_are_pinned` fails after pin edit | Static was edited but the matching test arm was not | Update the `"sigstore-rekor-v1"` match arm in the same commit |
| `cargo test verifies_real_sigstore_rekor_entry` fails after PEM rotation | Pinned PEM was rotated but the golden fixture is still signed under the prior key | Regenerate the fixture per the Golden-fixture sub-section |
| `cargo test historical_shard_tree_id_passes_dispatch_gate` fails after shard rotation | The prior active shard was removed from the roster instead of demoted to historical | Restore the prior shard at `tree_id_roster[1]` — historical shards are append-only |
| `cross-version-anchor-compat.sh` fails on a `checkpoint_signature_invalid` for a prior-version fixture | PEM rotation without multi-key support — see the table above | Hold the rotation; open follow-on ADR; do NOT ship until multi-key is designed |
| `verify-trust-root-mutations` fails on the PR | Commit was not signed by a key in the prior trust root | Re-sign with a `.github/allowed_signers` key, or first add the new key in a prior PR signed by an existing trusted key (§14 recipe) |
| Pinned PEM no longer matches the Sigstore Foundation announcement | Source-of-truth drift OR Foundation announcement was updated mid-rotation | Re-fetch the announcement; `diff` against the diff hunk; if Foundation revised its publication, re-cut the rotation against the latest revision |

---

## 16. Branch-protection-state verifier (V1.18 Welle B (5))

§14 documents the **operator action** that pins the Welle C in-repo
trust-root-mutation gate to the master branch: a Repository Ruleset
named "Master trust-root protection" with a specific shape (no
`bypass_actors`, `enforcement: active`, the Welle C status check
required, signed-commits required, force-push + deletion blocked,
CODEOWNERS review required).

§16 documents the **automated verifier** that detects when that
Ruleset silently regresses — operator misclick in the Settings UI,
out-of-band API PATCH, ruleset replaced with a weaker one of the
same name, ruleset disabled "for a moment" and forgotten.

### Why a separate verifier exists

The Welle C in-repo gate ONLY binds because the Master Ruleset pins
six load-bearing properties (full list in §14):

  1. `enforcement: active`
  2. `bypass_actors: []` (no admin-bypass)
  3. `required_status_checks` includes `"Verify trust-root-modifying commits"`
  4. `pull_request.require_code_owner_review: true`
  5. `required_signatures` rule present
  6. `non_fast_forward` + `deletion` rules present

If any of these silently regress, the Welle C gate stops being
load-bearing **without any commit landing in the repo**. There is
no in-repo signal that defence-in-depth has been weakened.
`tools/verify-master-ruleset.sh` + `.github/workflows/verify-branch-
protection.yml` IS that signal.

### Defence shape

| Component | Role |
|---|---|
| `tools/expected-master-ruleset.json` | Pinned canonical form of the Ruleset, normalised (volatile fields stripped, rules sorted by `type`, `allowed_merge_methods` sorted). The comparison target. |
| `tools/verify-master-ruleset.sh` | Looks up the Ruleset by name on the configured owner/repo, fetches details, normalises with the same `jq` pipeline that produced the pin, diffs against the expected file. |
| `.github/workflows/verify-branch-protection.yml` | Runs the verifier on three triggers: nightly cron (drift detection on a cadence), push to master (drift detection on the heartbeat of normal merges), `workflow_dispatch` (on-demand re-check after a deliberate change). |

All three files are in `PROTECTED_SURFACE` (see
`tools/verify-trust-root-mutations.sh`). Mutating any of them
requires an SSH-signed commit by an `allowed_signer` plus
CODEOWNERS review — same gate as every other trust-root surface.

### What the workflow needs

Reading Repository Rulesets via the GitHub REST API requires the
`administration: read` permission. **`GITHUB_TOKEN` does NOT
support this scope at the workflow level** — listing it under
`permissions:` makes the workflow file invalid and the run fails
with "0 jobs / workflow file issue" before any step executes.
This was the bug that necessitated the V1.18 Welle B (5)
follow-up.

The workflow therefore expects a fine-grained Personal Access
Token (PAT) stored as a repository secret named
`RULESET_VERIFY_TOKEN`. If the secret is not configured, the
verifier still executes but exits 2 with a "could not list
rulesets" diagnostic — explicit failure pointing here.

#### One-time PAT setup

```
1. github.com → Settings → Developer settings → Personal access
   tokens → Fine-grained tokens → "Generate new token".
2. Token name: "atlas-ruleset-verify" (or similar).
3. Expiration: pick a cadence you'll remember to rotate
   (90 days recommended; max 1 year).
4. Resource owner: ThePyth0nKid (your account).
5. Repository access: "Only select repositories" → atlas.
6. Permissions:
     Repository permissions → Administration: Read-only
     (no other permission required — minimum scope).
7. Generate token. Copy the token value (visible only once).
8. github.com/ThePyth0nKid/atlas → Settings → Secrets and
   variables → Actions → "New repository secret".
9. Name: RULESET_VERIFY_TOKEN. Value: paste the token. Save.
10. Verify: Actions → verify-branch-protection → "Run workflow".
    Expect green within ~30 seconds.
```

#### PAT rotation calendar

The PAT expires per the cadence chosen at setup. Add a calendar
reminder at expiration_minus_14_days. The renewal recipe is
identical to the one-time setup above — generate a new token,
update the `RULESET_VERIFY_TOKEN` secret value (do NOT add a
second secret; overwrite). The `verify-branch-protection`
workflow will start exiting 2 once the old token expires; the
nightly cron is the operator's safety net for forgotten
rotations.

### Workflow CI status reading

| CI run state | Interpretation |
|---|---|
| Green (exit 0) — daily cron + post-push | The Ruleset matches the pin. The Welle C gate is correctly bound. No action needed. |
| Red (exit 1) — drift detected | The live Ruleset differs from the pin. The unified diff in the run logs shows the difference. See "When the verifier fires red" below. |
| Red (exit 2) — lookup error | One of: (a) the `RULESET_VERIFY_TOKEN` secret is missing or expired (most common — see "One-time PAT setup" / "PAT rotation calendar" below); (b) the Ruleset named "Master trust-root protection" no longer exists; (c) `jq`/`gh` is missing on the runner. Investigate before next merge to master. |

### When the verifier fires red

1. **Open the failed workflow run** and read the unified diff. The
   diff shows expected (pinned) on the `-` side, actual (live) on
   the `+` side.

2. **Triage the drift:**

   * **Unintentional drift** — operator made a settings change that
     wasn't supposed to weaken the defence (e.g. accidentally
     removed a rule, accidentally added a bypass actor, accidentally
     toggled enforcement to "Evaluate"). Fix: revert the Ruleset to
     the pinned state via Settings → Rules → Rulesets → "Master
     trust-root protection". Re-run `verify-branch-protection`
     workflow via `workflow_dispatch` to confirm the revert; expect
     green.
   * **Intentional drift** — operator made a deliberate change that
     should be the new normal (e.g. promoted to multi-maintainer
     and raised `required_approving_review_count` from 0 to 1). Fix:
     re-pin `tools/expected-master-ruleset.json` to match the new
     live state, in an SSH-signed commit. Re-run the verifier;
     expect green.
   * **Suspected compromise** — the drift was not authored by any
     known operator action and weakens the defence (e.g. a
     `bypass_actors` entry pointing at an unfamiliar GitHub
     account, enforcement disabled, the Welle C status check
     removed, the Ruleset renamed or deleted entirely). Treat as
     a security incident. Steps:
       a. **Do not merge anything to master** until the Ruleset is
          restored.
       b. Restore the Ruleset to the pinned state via Settings →
          Rules → Rulesets.
       c. Audit GitHub's audit log for `repository_ruleset.update`
          and `repository_ruleset.destroy` events — these surface
          the actor, timestamp, and changed fields.
       d. Rotate the suspected-compromised credential (PAT, SSH
          signing key, or GitHub session) and re-run all
          allowlist-touching workflows post-rotation.
       e. Document the incident in the maintainer logbook.

3. **Re-run the workflow** via Actions → verify-branch-protection
   → "Run workflow" to confirm the fix held.

### Recovering from "exit 2 — Ruleset not found"

This is a CRITICAL defence-down state. The Welle C in-repo gate is
unbound. Recovery:

```
1. Open Settings → Rules → Rulesets in the GitHub UI.
2. Click "New ruleset → New branch ruleset".
3. Name: exactly "Master trust-root protection" (the verifier
   matches by name, not by id).
4. Apply the configuration documented in §14 → "Modern-Rulesets
   equivalent" (the pinned canonical form).
5. Save.
6. Trigger the verifier via Actions → verify-branch-protection
   → "Run workflow". Expect green.
```

### Re-pinning after a legitimate Ruleset change

When a deliberate change to the Ruleset is the new normal (e.g.
adding a maintainer, raising approval count, deleting and
recreating the Ruleset to fix a misconfiguration), the pinned file
**and** the `EXPECTED_RULESET_ID` constant in
`tools/verify-master-ruleset.sh` must be updated in lockstep:

  * Shape change only (rules / parameters / bypass_actors edited
    in-place, same Ruleset id) → re-pin only
    `tools/expected-master-ruleset.json`.
  * Delete-and-recreate (new id minted, even if shape identical) →
    re-pin BOTH `tools/expected-master-ruleset.json` (in case
    shape also changed) AND `EXPECTED_RULESET_ID` in
    `tools/verify-master-ruleset.sh`.

Both files are in `PROTECTED_SURFACE`, so the re-pin commit must
be SSH-signed by an `allowed_signer` and pass CODEOWNERS review.

Recipe:

```bash
# 1. Make the Ruleset change in the GitHub Settings UI.

# 2. Fetch the live Ruleset.
gh api -H "Accept: application/vnd.github+json" \
  repos/ThePyth0nKid/atlas/rulesets/$(gh api repos/ThePyth0nKid/atlas/rulesets \
    | jq -r '.[] | select(.name == "Master trust-root protection") | .id') \
  > /tmp/ruleset-live.json

# 3. Run the same normalisation jq pipeline as the verifier.
#    *** MIRROR WARNING ***
#    This pipeline MUST stay byte-equivalent to the one in
#    tools/verify-master-ruleset.sh. If you change one, change
#    both in the same commit. Future improvement: have this
#    recipe invoke `tools/verify-master-ruleset.sh --dump-pin`
#    instead of duplicating the pipeline.
jq -S '
  del(.id, .node_id, .source, .created_at, .updated_at, ._links, .current_user_can_bypass)
  | .rules |= (
      sort_by(.type)
      | map(
          if (.parameters? | type) == "object" then
            .parameters |= (
              (if (.allowed_merge_methods? | type) == "array" then
                .allowed_merge_methods |= sort
              else . end)
              | (if (.required_status_checks? | type) == "array" then
                .required_status_checks |= (
                  map(del(.integration_id?))
                  | sort_by(.context)
                )
              else . end)
            )
          else . end
        )
    )
' /tmp/ruleset-live.json > tools/expected-master-ruleset.json

# 4. Confirm the diff matches your intent. If it shows fields you
#    did NOT mean to change, the Ruleset is in a state you didn't
#    author — STOP and triage as a possible compromise.
git diff tools/expected-master-ruleset.json

# 5. SSH-sign + commit (PROTECTED_SURFACE write requires it).
git add tools/expected-master-ruleset.json
git commit -m 'chore(v1.x): re-pin master ruleset after <change-description>'

# 6. Push, open PR, merge.

# 7. Confirm the verifier is green post-merge:
#    Actions → verify-branch-protection → wait for the post-push
#    run to complete green.
```

### Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| Workflow red with `'Master trust-root protection' (id=N) drifts from pinned configuration` | Live Ruleset differs from `tools/expected-master-ruleset.json` | See "When the verifier fires red" above. |
| Workflow red with `no Repository Ruleset named '...' found` | Ruleset deleted (or renamed) | See "Recovering from 'exit 2 — Ruleset not found'" above. |
| Workflow red with `could not list rulesets on ...` | Token cannot reach the rulesets API at all (auth failure / permission denied at gateway) — typically `RULESET_VERIFY_TOKEN` PAT secret missing/expired AND the GITHUB_TOKEN fallback also lacks the scope (rare) | Set up / renew the PAT per "One-time PAT setup" / "PAT rotation calendar" above. |
| Workflow red with `PAT scope insufficient — live Ruleset response missing 'bypass_actors' field` (V1.18 Welle B (7) hardfail; **exit 2 — indeterminate, not exit 1 — drift**) | Workflow is running under the default GITHUB_TOKEN fallback (lookup + fetch succeed but the response is field-restricted — omits the `bypass_actors` field). The verifier cannot answer its central "is bypass_actors empty?" question without this field, so it exits 2 immediately rather than proceeding to a confusing shape-diff. **This is NOT a Ruleset drift signal** — the live Ruleset state is indeterminate from the verifier's vantage point until the PAT is configured. | Set up `RULESET_VERIFY_TOKEN` per "One-time PAT setup" above. Distinct from the `could not list rulesets` row above: that row fires when the API gateway rejects the request entirely; this row fires when the API returns 200 but with a token-scope-limited response shape. Both can happen with a misconfigured PAT but the symptoms (and the diagnostic UX) differ. |
| Workflow red with `ruleset fetch returned non-object JSON (type=...)` (V1.18 Welle B (7) shape pre-flight; **exit 2 — indeterminate, not exit 1 — drift**) | The detail-endpoint `gh api .../rulesets/N` returned HTTP 200 with a body that parses as JSON but is not a ruleset object — most commonly an error envelope like `{"message":"Not Found"}` returned with HTTP 200 instead of 404. Distinct from the `bypass_actors` field-restricted case above: that fires when the body IS a ruleset object but field-limited; this fires when the body is not a ruleset object at all. | Inspect the response manually: `gh api repos/<owner>/<repo>/rulesets/<id>`. Resolution depends on the body content: if `{"message":"Not Found"}`, treat as the same defence-down state as `Ruleset not found` (the listing endpoint succeeded but the detail endpoint denied) — see "Recovering from 'exit 2 — Ruleset not found'" above. If the body is some other non-object shape, contact GitHub support with the response captured (this would indicate an upstream API regression). |
| Workflow run shows "0 jobs / 0s / workflow file issue" | Workflow YAML invalid (e.g., re-introduced `administration: read` under `permissions:` — GITHUB_TOKEN does not support it) | Revert the offending workflow edit. The valid scopes for `GITHUB_TOKEN` are listed at the GitHub Actions automatic-token doc; `administration` is not among them. |
| Workflow red with `jq is required` or `gh CLI is required` | Runner image regression | Pin the runner image (e.g. `ubuntu-22.04` instead of `ubuntu-latest`) until the upstream restores tooling. |
| Workflow green but the Ruleset is obviously weakened | Pinned file `tools/expected-master-ruleset.json` was tampered with to match the weakened state | This requires a signed commit by an `allowed_signer` plus CODEOWNERS review (the file is in `PROTECTED_SURFACE`). If observed, treat as compromise of an SSH signing key — rotate immediately. Audit recent `tools/expected-master-ruleset.json` history. |
