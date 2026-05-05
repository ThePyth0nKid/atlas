# `verify-wasm-pin-check@v1` — V1.17 Welle A Composite Action

> Auto-verify all three CONSUMER-RUNBOOK §1 integrity layers
> (version-pin → lockfile-integrity → SLSA L3 provenance) for
> `@atlas-trust/verify-wasm` on every CI install.

## Why this exists

[CONSUMER-RUNBOOK §1](../../../docs/CONSUMER-RUNBOOK.md) defines a
three-layer trust stack for downstream npm consumers of
`@atlas-trust/verify-wasm`:

1. **Version pin** in `package.json` — `"1.15.0"`, never `^1.15.0`.
2. **Lockfile integrity** — `sha512-…` hash bound at install time.
3. **SLSA L3 provenance** — `npm audit signatures` against the
   OIDC-signed Sigstore Rekor attestation.

All three are necessary; none alone is sufficient. The runbook walks
through the install-time ceremony for npm / pnpm / Bun. **This action
makes the same checks run automatically on every CI build, with no
possibility of forgetting a layer or skipping the audit step.**

The threat closed by this action: a consumer follows the runbook
once at install time, then forgets to re-verify on every CI run,
leaving a silent window where a maintainer-token-compromise can land
between two installs and only be caught at the next manual quarterly
audit.

## Usage

### Minimum

```yaml
- uses: ThePyth0nKid/atlas/.github/actions/verify-wasm-pin-check@v1.17.0
```

That's it. Auto-detects the lockfile (npm > pnpm > bun precedence),
asserts every layer, fails the build on any drift.

### Full surface

```yaml
- uses: ThePyth0nKid/atlas/.github/actions/verify-wasm-pin-check@v1.17.0
  with:
    package-name: '@atlas-trust/verify-wasm'   # default
    expected-version: '1.15.0'                  # if set, must match exactly
    package-manager: 'auto'                     # auto | npm | pnpm | bun
    working-directory: '.'                      # default — repo root
    skip-provenance: 'false'                    # set true ONLY for npm < 9.5
    fail-on-local-file: 'false'                 # set true to refuse backup-channel installs
    provenance-retries: '3'                     # ~2 min total backoff (10s/30s/90s)
```

### Recommended pinning

For high-assurance consumer pipelines, pin to a **commit SHA**, not
the moving `@v1.17.0` tag:

```yaml
- uses: ThePyth0nKid/atlas/.github/actions/verify-wasm-pin-check@<full-40-char-sha>
```

This is the same trust posture the action itself enforces on
consumers — defence in depth.

## What it asserts

### Layer 1 — Version pin

Reads `package.json` and rejects any of:

- `"^1.15.0"` (caret — silent minor/patch upgrades on next install)
- `"~1.15.0"` (tilde — silent patch upgrades on next install)
- `">=1.15.0"`, `"<2.0.0"`, `"||"`, `"*"`, `"x"`, `"latest"`, `"next"`
- `"1.0.0 - 2.0.0"` (npm hyphen-range — `>=1.0.0 <=2.0.0`)
- `"workspace:..."`, `"file:..."`, `"link:..."`, `"git+..."`,
  `"github:..."`, `"http:..."`, `"https:..."`
- Anything that isn't a bare semver shape (`1.15.0` /
  `1.15.0-rc.1` / `1.15.0+sha.abc`)

Checks `dependencies`, `devDependencies`, `peerDependencies`,
`optionalDependencies` — every bucket the package appears in must
satisfy the rule.

### Layer 2 — Lockfile integrity

Reads the lockfile (auto-detected) and asserts every entry for the
package has:

- A `version` field that matches the pinned value.
- A `resolved` field with an HTTPS origin (canonical
  `registry.npmjs.org` is silent-OK; other HTTPS mirrors emit a
  warning so corporate-proxy users know the integrity hash is
  mirror-served; `file:` is the V1.15 Welle B backup-channel
  state — WARN by default, FAIL with `fail-on-local-file: true`).
- An `integrity` field whose hash prefix is `sha512-` / `sha384-` /
  `sha256-`. `sha1-` and `md5-` are HARD FAIL (collisions are
  practical and don't defend against registry-side replacement).

Supports npm `lockfileVersion` 1, 2, and 3 (the `dependencies`
recursive tree from npm v6, the `packages` map from npm v7+, and
both shapes coexisting). Supports pnpm `lockfileVersion` 6.0 and
later. Supports Bun text-format `bun.lock` directly; binary
`bun.lockb` requires `bun` on PATH for the `bun pm ls --json` query.

### Layer 3 — SLSA L3 provenance

Pre-condition: runs `npm ls <package> --depth=0` and asserts the
package is in the resolved install tree. Without this guard, a
tree where another package is signed but the verifier package
itself is absent would silently pass Layer 3 — `npm audit
signatures` reports counts in the success case (not per-package
verdicts), so the positive grep on "verified attestation" would
match an unrelated package's attestation line. The npm-ls
precondition closes that false-pass.

Then runs `npm audit signatures` and asserts:

- The command exits 0.
- The output reports a `verified attestation` line.
- The package is NOT mentioned in any failure context (case-
  insensitive match against `missing|invalid|failed|error|
  untrusted`).

`npm audit signatures` requires npm ≥ 9.5 (the version that
introduced the attestation API). Older npm silently lacks the
subcommand — the action enforces the version check up front and
suggests `actions/setup-node` if needed. **You MUST run `npm ci`
(or `npm install`) BEFORE this action**, or the npm-ls precondition
will fail — Layer 3 needs an installed tree to attest.

#### Retry policy

Sigstore Rekor and the npm attestation endpoint occasionally have
transient outages. The action retries `npm audit signatures` up to
`provenance-retries` times (default 3), with exponential backoff:

- Attempt 1 fails → wait 10s, retry
- Attempt 2 fails → wait 30s, retry
- Attempt 3 fails → wait 90s, retry
- Attempt 4 fails → exit non-zero

Total wait on max retries: ~130s (~2 minutes). Failures classified
as **attestation-failure** or **signature-failure** (i.e. the cryptography
itself rejects the attestation) skip the retry — those are never
transient and retrying just burns runner time.

## What it deliberately does NOT do

- **Does not run `npm install` / `pnpm install` / `bun install`.** You
  must run those yourself (typically as a prior step). The action
  is read-only against `package.json` + the lockfile + the npm
  attestation API. This separation matters: install steps are
  user-side concerns (caching, registry config, lockfile
  regeneration), and bundling install with verification would
  obscure responsibility on failure.
- **Does not mutate any consumer file.** Every check is read-only.
- **Does not cache between runs.** Each invocation re-validates
  from first principles — caching results would defeat the
  threat-model (an attacker who substitutes a malicious package
  between caches would slip through).
- **Does not check transitive dependencies of `@atlas-trust/verify-wasm`.**
  The verifier package has no runtime dependencies (the WASM crate
  is the only payload). If a future version adds runtime deps,
  this action will need an update; the assumption is asserted at
  Layer 2 (only one package matched per lockfile) and the test
  fixtures verify it.

## Trust-domain note

The action runs on **the consumer's GitHub Actions runner**. It does
not have access to any Atlas secret. The only network call it makes is
the `npm audit signatures` round-trip (which talks to
`registry.npmjs.org` + `rekor.sigstore.dev` — both load-bearing for
the SLSA L3 verification). All other steps are pure-local file reads.

## Why a composite action (not a TypeScript or Docker action)

- **Pure-bash + small per-script files.** Auditable from `action.yml`
  down to every byte. No opaque `dist/index.js` bundle to trust, no
  build step, no transitive npm dependency tree of its own.
- **No supply-chain surface of its own.** A TypeScript action
  becomes a dependency that needs auditing — the irony of a
  supply-chain-hardening action being itself a supply-chain risk
  would defeat the point. A composite action is reviewable in one
  scroll.
- **Matches the V1.15 / V1.16 pattern.** The `tools/playground-csp-check.sh`
  validator and the V1.16 Welle C git pre-commit hook are also
  pure-bash and also sink-free.

## Self-test workflow

[`.github/workflows/verify-wasm-pin-check-self-test.yml`](../../workflows/verify-wasm-pin-check-self-test.yml)
exercises the action on every push / PR that touches the action,
plus a weekly cron at `Mon 06:17 UTC` to catch live Sigstore
regressions. Four jobs:

1. **`fixture-unit-tests`** — runs `test/run-tests.sh` against 15
   synthetic fixture directories with 36 test cases (Layers 1 + 2
   only, no network).
2. **`action-fixture-invocation`** — invokes the composite action
   via `uses:` against each Layer-2-passing fixture (Layers 1 + 2
   only). Validates the action's input plumbing.
3. **`action-negative-cases`** — invokes the action against bad
   fixtures and asserts each one fails with the expected exit code.
4. **`live-install-layer-3`** — `npm install --save-exact
   @atlas-trust/verify-wasm@latest` then runs the full action
   (all three layers, real Sigstore round-trip). This is the
   end-to-end smoke test.

## Cross-reference

- [docs/CONSUMER-RUNBOOK.md](../../../docs/CONSUMER-RUNBOOK.md) — full
  consumer-side reproducibility guide and the source of truth for
  what each layer protects against.
- [docs/SECURITY-NOTES.md §scope-k](../../../docs/SECURITY-NOTES.md) —
  trust-composition slot for V1.17 Welle A.
- [docs/ARCHITECTURE.md V1.17 boundary](../../../docs/ARCHITECTURE.md)
  — V1.17 Welle A in the architecture timeline.
