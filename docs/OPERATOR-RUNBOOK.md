# Atlas — Operator Runbook (V1.12)

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

## 8. Quick reference

| Ceremony | Command | Idempotent | Atomic-replace required |
|---|---|---|---|
| HSM master-seed import (V1.10 wave 2 — see §2) | `pkcs11-tool ... --write-object <seed-file> --label atlas-master-seed-v1` | no (re-import overwrites) | n/a (HSM-side) |
| HSM per-workspace key generation (V1.11 wave-3 — see §3) | `ATLAS_HSM_WORKSPACE_SIGNER=1 atlas-signer derive-pubkey --workspace <ws>` | yes (find-or-generate) | n/a (key lives in HSM) |
| Migrate workspace bundle to V1.9 | `atlas-signer rotate-pubkey-bundle --workspace <ws>` | yes | yes (operator-side `mv`) |
| Rotate anchor chain | `atlas-signer rotate-chain --confirm <workspace>` | no | yes (operator-side) |
| Derive workspace pubkey for inspection | `atlas-signer derive-pubkey --workspace <ws>` | yes | n/a (read-only) |
| Production-gate enforcement (V1.12+) | configure the HSM trio (§2) — V1.9 `ATLAS_PRODUCTION=1` removed | n/a | n/a |

See [ARCHITECTURE.md §7.3](ARCHITECTURE.md) (V1.9 per-tenant key trust
model), [§7.4](ARCHITECTURE.md) (V1.10 master-seed gate + wave-2
sealed-seed loader, V1.12-simplified) and [§7.5](ARCHITECTURE.md)
(V1.11 wave-3 sealed per-workspace signer); see
[SECURITY-NOTES.md](SECURITY-NOTES.md) for the full threat model.
