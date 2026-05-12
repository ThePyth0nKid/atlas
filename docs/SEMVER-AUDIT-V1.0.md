# SemVer Audit — Atlas v1.0.0 Public-API Surface

> **Status: V1.19 Welle 12 deliverable, finalised 2026-05-11.** This document is the v1.0 baseline contract for Atlas's public-API surface. After v1.0.0 ships (V1.19 Welle 13), no item listed as **Locked** below changes without a SemVer-major bump.

This audit surveys five public-API surface categories and assigns each item one of four risk tags:

| Tag | Meaning |
|---|---|
| **Locked** | Committed for v1.0. Any breaking change → SemVer-major bump. |
| **Locked-Behind-Flag** | Opt-in surface (CLI flag, strict-mode option). Default behaviour is stable; flag turns on additional checks. New flags = SemVer-minor. |
| **Internal-but-Exported** | The `__` prefix family. Documented as "not a stability contract". Consumers who reach for these accept SemVer-patch-level churn. |
| **Defer-Decision** | Items where v1.0 commitment is deferred. Either covered behind a clear stability disclaimer in code, or scoped to a future minor. |

Adding new variants to `#[non_exhaustive]` enums, new fields with `Default` to structs marked `#[non_exhaustive]` or owned by Atlas, new evidence-list entries to `VerifyOutcome`, and new CLI flags are all **SemVer-minor** under this audit's conventions — they do not break downstream `match` arms, struct construction, or auditor tooling.

---

## 1. Rust crates — public types and re-exports

### 1.1 `crates/atlas-trust-core` (library crate; consumed by `atlas-verify-cli`, `atlas-verify-wasm`, `atlas-signer`, `atlas-witness`, and any future Rust consumer)

#### Re-exports at the crate root (`src/lib.rs`)

| Item | Tag | Notes |
|---|---|---|
| `TrustError` enum (with `#[non_exhaustive]`) | **Locked** | Adding variants is SemVer-minor. The `Clone` derive (V1.13 wave-C-2) is part of the contract — downstream `WitnessVerifyOutcome.failures` depends on it. |
| `TrustResult<T>` type alias | **Locked** | |
| `AtlasEvent`, `EventSignature` (from `trace_format`) | **Locked** | Wire-format types, schema-version-gated by `SCHEMA_VERSION`. |
| `AtlasTrace` (from `trace_format`) | **Locked** | Top-level wire-format. `#[serde(default)]` on new fields is required for backward compat. |
| `AnchorEntry`, `AnchorBatch`, `AnchorChain` (from `trace_format`) | **Locked** | V1.5–V1.7 anchor wire shapes. |
| `WitnessSig`, `witness_signing_input`, `WitnessFailureWire` (from `witness`) | **Locked** | V1.13 wave-C-2 witness surface. |
| `VerifyOutcome`, `VerifyEvidence`, `VerifyOptions` (from `verify`) | **Locked** | See §1.2 below for field-by-field treatment. |
| `verify_trace`, `verify_trace_with` (re-exported at crate root from `verify`) | **Locked** | Public verification entry points. |
| `atlas_trust_core::verify::verify_trace_json(trace_bytes: &[u8], bundle: &PubkeyBundle) -> TrustResult<VerifyOutcome>` | **Locked** | Reachable via the module path (`verify` is `pub mod`); NOT currently re-exported at crate root. Consumers reach it as `atlas_trust_core::verify::verify_trace_json`. Adding a crate-root re-export later is SemVer-minor. |
| `PubkeyBundle` (from `pubkey_bundle`) | **Locked** | Pinned key roster type. |
| `chain_head_for`, `ChainHeadHex`, `ANCHOR_CHAIN_DOMAIN` (from `anchor`) | **Locked** | V1.7 chain-domain-separator constant. |
| `parse_per_tenant_kid`, `per_tenant_kid_for`, `PER_TENANT_KID_PREFIX` (from `per_tenant`) | **Locked** | V1.9 per-tenant kid surface. |
| `SCHEMA_VERSION = "atlas-trace-v1"` const | **Locked** | Wire-schema identifier; changing it is V2-class. |
| `VERIFIER_VERSION = "atlas-trust-core/<pkg-version>"` const | **Locked** | Format `crate-name/semver`. Auditor tooling switches on this. |

#### Public modules (`pub mod`)

The 12 public modules — `anchor`, `ed25519`, `cose`, `ct`, `hashchain`, `per_tenant`, `pubkey_bundle`, `trace_format`, `verify`, `witness`, `error` — are **Locked** as module paths. Consumers MAY reach into them for free-function APIs not re-exported at the crate root (e.g. `atlas_trust_core::hashchain::check_strict_chain` is a public surface alongside the variant in `TrustError`).

The `check_strict_chain` free function from V1.19 Welle 9 is **Locked**: signature `pub fn check_strict_chain(events: &[AtlasEvent]) -> TrustResult<()>`. The five property-checks (non-empty, exactly-one-genesis, single-parent for non-genesis, no sibling-fork, no self-reference) are documented in the crate-doc and exercised by 9 unit tests. The full property *set* is locked, not just the count: **weakening, removing, or replacing any of the five properties is equally SemVer-major** — downstream auditors pattern-match on the `TrustError::StrictChainViolation` discriminant and depend on the full property set being enforced; loosening (e.g. dropping the self-reference check on the assumption it's covered by `check_event_hashes`) silently changes the contract observed at the auditor's seat. Adding a sixth property would also be SemVer-major (existing callers would see new false-positive rejections on previously-accepted traces). Property-renumbering (e.g. re-ordering the five checks in implementation for locality reasons) or message-text reorganisation is SemVer-patch — auditor tooling MUST switch on `TrustError::StrictChainViolation` discriminant, not on the `msg` string content or property-order.

#### `VerifyOptions` (V1.10 wave-1 surface, V1.19 Welle 9 extended)

| Field | Tag | Notes |
|---|---|---|
| `require_anchors: bool` (V1.5) | **Locked-Behind-Flag** | Default false. Flips strict-anchors gate. |
| `require_anchor_chain: bool` (V1.7) | **Locked-Behind-Flag** | Default false. |
| `require_per_tenant_keys: bool` (V1.9) | **Locked-Behind-Flag** | Default false. Strict-mode boundary for per-tenant kid invariant. |
| `require_witness_threshold: usize` (V1.13) | **Locked-Behind-Flag** | Default 0. Strict-mode threshold for witness-coverage invariant. |
| `require_strict_chain: bool` (V1.19 Welle 9) | **Locked-Behind-Flag** | Default false. Per V1.19 Welle 9 trust-note, default MUST stay opt-in for v1.x (Atlas is fundamentally a DAG; forks are valid wire shape under multi-process-writer deployments). Flipping the default to `true` would break valid multi-writer deployments — SemVer-major. **V2 forward-break candidate:** if no multi-process-writer production deployment materialises by v2 planning, V2 should flip the default to `true` for single-writer profiles (perhaps via a workspace-mode config key on `VerifyOptions`). Single-writer is the atlas-web + atlas-mcp-server posture today (per-workspace mutex in `@atlas/bridge::writeSignedEvent` structurally guarantees a linear chain), so the v1.x lenient default is permanently a foot-gun for those consumers — an auditor running `atlas-verify-cli verify-trace` without the flag sees ✓ VALID on a forked DAG that would indicate a mutex failure or write-race. Track as a V2 planning item. |

`VerifyOptions` is `#[derive(Default)]`. Adding a new `require_*` field with `Default::default() = false/0` is **SemVer-minor** — existing callers using `VerifyOptions { require_anchors: true, ..Default::default() }` keep compiling and keep their current semantics. New flags MUST default to the lenient value.

#### `VerifyOutcome` (V1.10 wave-1 surface, V1.13 wave-C-2 + V1.14 Scope-J extended)

| Field | Tag | Notes |
|---|---|---|
| `valid: bool` | **Locked** | |
| `evidence: Vec<VerifyEvidence>` | **Locked** | Adding a new `VerifyEvidence` entry is SemVer-minor (`ok: false` entries push to `errors` if the check is strict; lenient additions are neutral). Removing or renaming an evidence `check` label is SemVer-major. |
| `errors: Vec<String>` | **Locked** | |
| `verifier_version: String` | **Locked** | Format `atlas-trust-core/<semver>`. |
| `witness_failures: Vec<WitnessFailureWire>` (V1.14) | **Locked** | `#[serde(default)]` enables additive wire compat — V1.13 JSON deserialises into empty Vec. |

#### `VerifyEvidence`

| Field | Tag | Notes |
|---|---|---|
| `check: String` | **Locked** | Stable check-label set: `schema-version`, `pubkey-bundle-hash`, `event-hashes`, `event-signatures`, `parent-links`, `strict-chain` (V1.19 Welle 9), `dag-tips`, `per-tenant-keys`, `anchors`, `anchor-chain`, `anchor-chain-coverage`, `witnesses`, `witnesses-threshold`. Renaming any of these is SemVer-major. |
| `ok: bool` | **Locked** | |
| `detail: String` | **Defer-Decision** | Human-readable. Auditor tooling MUST NOT parse this for structured fields; the wire-stable fields are the `check` label + `ok` flag. Detail-text rewording is SemVer-patch. |

### 1.2 `crates/atlas-verify-cli` (binary crate)

| Item | Tag | Notes |
|---|---|---|
| `verify-trace` subcommand | **Locked** | Argv `verify-trace <trace> --pubkey-bundle <bundle>`. Required positional `<trace>` + required `-k`/`--pubkey-bundle`. |
| `--require-per-tenant-keys` flag (V1.9) | **Locked** | |
| `--require-anchors` flag (V1.5) | **Locked** | |
| `--require-anchor-chain` flag (V1.7) | **Locked** | |
| `--require-witness <N>` flag (V1.13) | **Locked** | `N: usize`, default 0. |
| `--require-strict-chain` flag (V1.19 Welle 9) | **Locked** | Default false. |
| `-o`/`--output <human\|json>` (V1.10) | **Locked** | Default `human`. JSON output schema = `VerifyOutcome` serde. |
| Exit codes: 0 = valid, 1 = invalid, 2 = error | **Locked** | |
| `print_human` evidence-row format (`✓/✗ check — detail`) | **Locked** | Smoke + e2e regex assertions across three CI lanes pin this — Welle 10 + Welle 12 anti-drift contract. Changing the marker or delimiter is SemVer-major because auditor tooling consumes the format. |
| `print_human` `Strict flags: ...` line format | **Locked** | Comma-separated identifiers, prefix anchored. |

New flags: SemVer-minor. Removing a flag or changing semantics: SemVer-major.

### 1.3 `crates/atlas-verify-wasm` (npm-published WASM)

Two `wasm_bindgen`-exported functions, both **Locked**:

| Item | Tag | Notes |
|---|---|---|
| `verify_trace_json(trace_json: &str, bundle_json: &str) -> Result<JsValue, JsValue>` | **Locked** | Returns `VerifyOutcome` as JS object via `serde-wasm-bindgen`. Browser consumers (`atlas-web/src/components/LiveVerifierPanel.tsx`, `playground.atlas-trust.dev`) depend on this signature. |
| `verifier_version() -> String` | **Locked** | Returns `atlas-trust-core/<semver>` string. Used by LiveVerifierPanel's `data-testid="verifier-version"` chip. |

The npm package name `@atlas-trust/verify-wasm` is **Locked**. Adding new exported functions is SemVer-minor.

### 1.4 `crates/atlas-signer` (binary crate, internal/operator tool)

`atlas-signer` is the per-tenant key-derivation binary spawned by `@atlas/bridge`. Its CLI surface (`derive-key`, `derive-pubkey`, `sign-event`, `anchor`, `chain-export`, `bundle-hash`) is **Locked-Behind-Flag** as an operator-internal surface — `@atlas/bridge` is the only documented consumer. Direct invocation (`./atlas-signer derive-key ...`) by external callers is not a v1.0 commitment; the bridge is the supported entry point.

The HSM-mode dispatch (V1.10 wave-2 + V1.12 wave-3) is **Locked** as operator-facing config: `ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`, `ATLAS_HSM_WORKSPACE_SIGNER`, `ATLAS_DEV_MASTER_SEED` env-var names are part of the v1.0 contract per OPERATOR-RUNBOOK §1 + §3.

### 1.5 `crates/atlas-witness` (binary crate, witness key-generation)

Operator-internal. The `keygen` subcommand format and the `ATLAS_WITNESS_V1_ROSTER` constant in `atlas-trust-core` are **Locked**. CLI flags are SemVer-minor-extensible.

---

## 2. npm packages

### 2.1 `@atlas/bridge` (workspace-internal, but exported via `packages/atlas-bridge/package.json`)

The bridge layer between TypeScript consumers (atlas-web, atlas-mcp-server) and the Rust signer binary + on-disk JSONL DAG.

**Package-level contract:** no `"source"` export (consumers always resolve via `dist/`). Sub-path imports intentionally NOT supported — package root only. Per Welle 2 + Welle 7 design.

#### Public exports from `packages/atlas-bridge/src/index.ts`

Grouped by category. All marked **Locked** unless noted:

| Category | Exports | Tag |
|---|---|---|
| Wire-format types | `EventSignature`, `AtlasEvent`, `AtlasPayloadType`, `PubkeyBundle`, `AnchorKind`, `InclusionProof`, `AnchorEntry`, `AnchorBatch`, `AnchorChain`, `AtlasTrace` | **Locked** |
| Wire-format constants | `SCHEMA_VERSION`, `PUBKEY_BUNDLE_SCHEMA`, `DEFAULT_WORKSPACE` | **Locked** |
| Path resolution | `isValidWorkspaceId`, `WorkspacePathError`, `setDefaultDataDir`, `dataDir`, `workspaceDir`, `eventsLogPath`, `anchorsPath`, `anchorChainPath`, `resolveSignerBinary`, `repoRoot` | **Locked** |
| Identity / kid derivation | `LegacySignerRole`, `SignerRole`, `SignerIdentity`, `PER_TENANT_KID_PREFIX`, `perTenantKidFor`, `workspaceIdFromKid`, `TEST_IDENTITIES`, `buildDevBundle`, `identityForKid`, `resolveIdentityForKid`, `resolvePerTenantIdentity`, `buildBundleForWorkspace` | **Locked** |
| Signer bridge | `SignArgs`, `AnchorRequest`, `AnchorBatchInput`, `AnchorOptions`, `DerivedIdentity`, `DerivedPubkey`, `SignerError`, `redactPaths` (see §2.1.2 security note), `signEvent`, `bundleHashViaSigner`, `anchorViaSigner`, `deriveKeyViaSigner` (`@deprecated`), `derivePubkeyViaSigner`, `chainExportViaSigner` | **Locked** |
| Storage | `StorageError`, `appendEvent`, `readAllEvents`, `computeTips`, `ensureWorkspaceDir` | **Locked** |
| Schemas (Zod) | `AtlasEventValidated`, `EventSignatureSchema`, `AtlasEventSchema`, `PerTenantKidSchema`, `DerivedPubkeySchema`, `DerivedIdentitySchema`, `AnchorKindSchema`, `InclusionProofSchema`, `AnchorEntrySchema`, `AnchorEntryArraySchema`, `AnchorChainSchema`, `AnchorBatchSchema` | **Locked** |
| Write pipeline | `WriteEventArgs`, `WriteEventResult`, `writeSignedEvent` | **Locked** |
| Lossless JSON | `parseAnchorJson`, `stringifyAnchorJson`, `isLosslessNumber`, `LosslessNumber`, `INTEGER_LITERAL_REGEX` | **Locked** |
| ULID | `UlidState`, `RandomSource`, `ulid`, `createUlid`, `nextUlid` | **Locked** |
| **Internal test seams** | `__signerBinaryCacheForTest`, `__signerLimitsForTest`, `__redactPathConstantsForTest` | **Internal-but-Exported** — see §2.1.1 |

#### 2.1.1 `__*ForTest` family — decision

**Audit-driven decision (Welle 12 plan-doc question #1):** **DEFER sub-path-export gating to post-1.0.** Rationale:

- The four `__*ForTest` exports (the three from `@atlas/bridge` plus `__REQUEST_BODY_MAX_BYTES_FOR_TEST` from `apps/atlas-web/src/app/api/atlas/write-node/route.ts`) all carry the documented `__` prefix convention signalling "not a stability contract".
- All four are inert from a security perspective: cache stores only strings, redact-path constants are inert regex source-strings, signer limits are post-Zod number caps, byte-cap is a `number`. No key material flows through any test seam.
- The consumers are workspace-internal tests (atlas-mcp-server `scripts/test-*.ts`, atlas-web `tests/`); no external consumer is forced to depend on these.
- A sub-path-export migration would touch `@atlas/bridge`'s `package.json#exports`, atlas-mcp-server test imports, and the atlas-web internal-route test. Non-trivial diff for marginal hardening.
- The `__` prefix already does the work that sub-path gating would: any future external consumer reaching for these is opting into churn.

**Post-1.0 disposition:** if external `@atlas/bridge` consumers materialise after v1.0 (the npm package becomes publicly published as `@atlas-trust/bridge` or equivalent), revisit sub-path export gating as a SemVer-minor migration. Track as a backlog item against V2 / V1.5 planning. No action in v1.0.

The four exports are **Internal-but-Exported** for v1.0.

#### 2.1.2 `redactPaths` — caller-responsibility security surface

`redactPaths` is structurally a `Locked` named export, but functionally it is a **caller-responsibility security surface**: every callsite that propagates atlas-signer stderr (or any operator-controlled error string carrying absolute paths) to a network response or external log MUST pass through this function before emission. Failure to do so leaks operator absolute-path layout (`/home/<user>/.cache/...`, `C:\Users\<name>\Desktop\...`) to attackers.

**v1.0 pinned callsites:**
- `packages/atlas-bridge/src/storage.ts` — wraps `StorageError` construction
- `apps/atlas-web/src/app/api/atlas/write-node/route.ts` — wraps 4xx/5xx response bodies before `NextResponse.json(...)`

Adding a new error-construction site that propagates signer stderr to a network response and does NOT route through `redactPaths` is a **security regression**, not a SemVer event — the wire surface stays unchanged but the operational disclosure surface changes. Reviewer convention enforces this via the `redact-paths` test surface (44 assertions, post-Welle-7); a new callsite without `redactPaths` would not fail any current test, so this caller-contract callout is the documentary defence.

The V1.19 Welle 6 + Welle 7 contracts pin `PATH_SEGMENT` and `POSIX_PATH_LOOKBEHIND` constants behind the `__redactPathConstantsForTest` test seam; the redactor itself is **Locked** as a transformation contract (input characters preserved verbatim outside path-shaped substrings; absolute paths redacted to `[redacted-path]`; dotted-relative paths pass verbatim per Welle 6 lookbehind).

### 2.2 `@atlas-trust/verify-wasm` (npm-published, public)

The browser-facing WASM-compiled verifier. See §1.3 for the wasm-bindgen surface.

**Package contract:**
- Package name: `@atlas-trust/verify-wasm` — **Locked**
- Default export: the wasm-bindgen JS shim. **Locked**
- Sub-paths: `init`/default export pattern. **Locked**
- Bundle format: ESM. **Locked**

Published by `.github/workflows/wasm-publish.yml` on tag-trigger. `verify-wasm-pin-check` Composite-Action gates the integrity of the published bundle against the source.

### 2.3 `atlas-web` + `atlas-mcp-server`

These are workspace apps, not published packages. Their npm `name` fields (`atlas-web`, `atlas-mcp-server`) are **Defer-Decision** — they could rename for v1.0 publication if Welle 14 decides to ship one or both as a public package. No external consumer today.

---

## 3. HTTP wire shapes

### 3.1 `apps/atlas-web/src/app/api/atlas/write-node/route.ts`

#### `POST /api/atlas/write-node`

| Aspect | Tag | Notes |
|---|---|---|
| Request body Zod schema (`.strict()`, `workspace_id`, `kind`, `id`, `attributes`) | **Locked** | Schema-version is implicit (no field) — schema-version-gate is V2-class. |
| Content-Length cap = 256 KB → 413 | **Locked** | Constant is operator-tunable in future via env-var, but the cap-and-413 contract is the v1.0 commitment. |
| 200 response shape `{ ok: true, event_id, event_hash, parents[], kid, workspace_id }` | **Locked** | atlas-web Playwright tests + Welle 1 e2e + smoke pin this. |
| 400 response shape `{ ok: false, error }` | **Locked** | |
| 413 response shape | **Locked** | |
| `__REQUEST_BODY_MAX_BYTES_FOR_TEST` export | **Internal-but-Exported** | See §2.1.1. |

#### `GET /api/atlas/write-node?workspace_id=…`

| Aspect | Tag | Notes |
|---|---|---|
| Query param `workspace_id` | **Locked** | |
| 200 response `{ kid: "atlas-anchor:<workspace_id>" }` | **Locked** | Used by `WriteNodeForm` for kid-preview live-update. |

### 3.2 `apps/atlas-web/src/app/api/golden/`

| Endpoint | Tag | Notes |
|---|---|---|
| `GET /api/golden/bank-trace` | **Locked** | Returns the bank-q1-2026 trace JSON for LiveVerifierPanel default-load. |
| `GET /api/golden/bank-bundle` | **Locked** | Returns the matching pubkey bundle. |

Both are static exports from `examples/golden-traces/`. The endpoint paths are part of the v1.0 contract because playground.atlas-trust.dev and the LiveVerifierPanel-on-`/` both depend on them.

---

## 4. MCP tool surface (`apps/atlas-mcp-server/src/tools/`)

Five tools, all **Locked** by tool name + arg schema + result shape:

| Tool name | File | Tag |
|---|---|---|
| `atlas_anchor_bundle` | `anchor-bundle.ts` | **Locked** |
| `atlas_export_bundle` | `export-bundle.ts` | **Locked** |
| `atlas_workspace_state` | `workspace-state.ts` | **Locked** |
| `atlas_write_annotation` | `write-annotation.ts` | **Locked** |
| `atlas_write_node` | `write-node.ts` | **Locked** |

Tool discovery is name-based in MCP SDK convention. Renaming any tool = SemVer-major (Claude Desktop / Cursor configs would break). Adding new tools = SemVer-minor. Adding new args to a tool with a `Default` is SemVer-minor; removing an arg or making an optional arg required is SemVer-major.

The MCP server's exported `bundle.ts` helpers (`exportWorkspaceBundle`) are workspace-internal — used by the smoke and by atlas-web's e2e roundtrip — and tracked under **Internal-but-Exported**.

---

## 5. On-disk format

### 5.1 `data/<workspace>/events.jsonl`

| Aspect | Tag | Notes |
|---|---|---|
| Line-delimited JSON, one `AtlasEvent` per line | **Locked** | |
| `AtlasEvent` shape (Zod-validated on read) | **Locked** | Schema-version-gated at the trace level. |
| Hash chain integrity (parents → event_hash recomputable) | **Locked** | V1.5 invariant. |

### 5.2 `data/<workspace>/anchors.json` (V1.5)

`Vec<AnchorEntry>`. **Locked**.

### 5.3 `data/<workspace>/anchor_chain.json` (V1.7)

`AnchorChain`. **Locked**, schema-version-gated within the file.

---

## 6. Operator-facing config (env vars)

Pulled from `docs/OPERATOR-RUNBOOK.md` §1. All **Locked** as env-var names for v1.0:

- `ATLAS_DATA_DIR` — workspace data root override
- `ATLAS_DEV_MASTER_SEED` — V1.10 wave-1 dev-seed positive opt-in (truthy: `1`, `true`, `yes`, `on`)
- `ATLAS_HSM_PKCS11_LIB` — V1.10 wave-2 PKCS#11 lib path
- `ATLAS_HSM_SLOT` — V1.10 wave-2 PKCS#11 slot
- `ATLAS_HSM_PIN_FILE` — V1.10 wave-2 PKCS#11 PIN file path
- `ATLAS_HSM_WORKSPACE_SIGNER` — V1.12 wave-3 sealed-per-workspace opt-in (truthy as above). Canonical name per `docs/OPERATOR-RUNBOOK.md` §3 + smoke.ts §"V1.12 Scope B2 wave-3 auto-detect"
- `ATLAS_WITNESS_KEY_FILE` — V1.13 witness signing key
- `ATLAS_WITNESS_KID` — V1.13 witness kid

Removing or renaming any of these is SemVer-major (operator deployments would silently lose configuration on upgrade).

---

## 7. Breaking-change-relevant findings summary

After surveying §1–§6, **no items require a breaking change before v1.0.0**. The audit recommends shipping v1.0.0 with the surface as-is. Three items are documented as `Defer-Decision` and are not v1.0 commitments:

1. **`@atlas/bridge` external publication** — workspace-internal today; if Welle 14 decides to publish as `@atlas-trust/bridge`, the npm package name becomes a new public commitment. Track separately.
2. **`atlas-signer` direct CLI invocation** — operator-internal today. Public commitment is the bridge wrapper, not the underlying binary's CLI flags.
3. **`VerifyEvidence.detail` text content** — wire-stable fields are the `check` label and `ok` flag; the detail text is human-readable and may reword as SemVer-patch.

---

## 8. Post-1.0 SemVer policy

**v1.x minor bumps:**
- New `TrustError` variants (under `#[non_exhaustive]`)
- New `VerifyOptions` fields with `Default::default() = lenient`
- New `VerifyOutcome` fields with `#[serde(default)]`
- New `VerifyEvidence` `check` labels (additive)
- New `atlas-verify-cli` flags (additive)
- New `wasm-bindgen` exports on `@atlas-trust/verify-wasm`
- New `@atlas/bridge` exports
- New MCP tools
- New web API endpoints

**v1.x patch bumps:**
- `VerifyEvidence.detail` text rewording
- `TrustError::*::msg` text rewording (auditor tooling switches on variant, not text)
- Internal refactors with no public-API impact
- Performance improvements with identical outputs
- Documentation fixes

**v2.0.0 major bumps (planned scope, not v1.x):**
- `SCHEMA_VERSION` change (`atlas-trace-v1` → `atlas-trace-v2`)
- Removing or renaming any **Locked** item above
- Changing `--require-*` flag default to `true` (would break valid lenient-mode deployments)
- Changing on-disk format incompatibly
- Removing any operator env-var

---

## 9. Audit governance

After v1.0.0 ships, any PR adding/removing exports on the surfaces listed in §1–§6 MUST update this document in the same commit. Enforced by reviewer convention; not gated by CI (over-engineering for a 4-author project at this scale).

Reviewer checklist for surface-touching PRs:
- [ ] Identified the surface category (§1–§6) the change touches
- [ ] Confirmed the change is additive (SemVer-minor) or breaking (SemVer-major)
- [ ] Updated `docs/SEMVER-AUDIT-V1.0.md` with the new item + tag
- [ ] Updated `CHANGELOG.md` under the appropriate `[Unreleased]` heading
- [ ] If SemVer-major: confirmed v2 branch + migration path in PR description

---

**Audit author:** V1.19 Welle 12 implementer.
**Audit reviewer:** parallel `security-reviewer` agent (Welle 12 review pass).
**Effective date:** finalised at v1.0.0 tag (V1.19 Welle 13).

---

## 10. V2-α Additions (additive on master, version-bump deferred)

> **Status:** added 2026-05-12 by V2-α Welle 1 ship. Workspace version unchanged (still `1.0.1`); a major-bump release (`v2.0.0-alpha.1` candidate) is deferred to the end of the V2-α welle bundle per `.handoff/v2-alpha-welle-1-plan.md` §"Decisions".
>
> **Wire-compat policy:** V2-α additions intentionally break V1.0 verifier deserialization when `author_did` is present on the wire. `AtlasEvent` retains `#[serde(deny_unknown_fields)]`; a V1.0 verifier reading a V2-α event with `author_did = Some(...)` will reject with `unknown_field("author_did")`. This is by design — V2 = major bump. Events without `author_did` (V1-shaped) remain forward-compatible across both verifier generations.
>
> **Trust-property invariant preserved:** the V1 byte-determinism CI pin in `cose::tests::signing_input_byte_determinism_pin` is byte-identical pre- and post-Welle-1. When `author_did = None`, the CBOR signing-input is exactly what V1 produced.

### 10.1 New `pub mod agent_did` (`crates/atlas-trust-core/src/agent_did.rs`)

| Item | Tag | Notes |
|---|---|---|
| `pub const AGENT_DID_PREFIX: &str = "did:atlas:"` | **V2-α-Additive** | DID-method prefix. SemVer-major to change (would invalidate every Atlas agent DID on the wire). |
| `pub fn agent_did_for(pubkey_hash: &str) -> String` | **V2-α-Additive** | Presentation-layer helper. SemVer-major to change return shape; SemVer-minor to add parameters with defaults (Rust doesn't support default args, so realistically also major). |
| `pub fn parse_agent_did(did: &str) -> Option<&str>` | **V2-α-Additive** | Strict parser. Returning `Some(...)` for a previously-rejected input is SemVer-major (loosens trust contract); returning `None` for previously-accepted input is also SemVer-major (rejects existing data). |
| `pub fn validate_agent_did(did: &str) -> TrustResult<()>` | **V2-α-Additive** | Verifier-side check. Same SemVer treatment as `parse_agent_did`. |

### 10.2 `TrustError` new variant (`crates/atlas-trust-core/src/error.rs`)

| Variant | Tag | Notes |
|---|---|---|
| `TrustError::AgentDidFormatInvalid { did: String, reason: String }` | **V2-α-Additive (SemVer-minor under `#[non_exhaustive]`)** | New failure mode. Adding variants to `#[non_exhaustive]` enums is SemVer-minor per §8 conventions. Auditor tooling that switches on `TrustError` discriminant continues to match exhaustively because of `#[non_exhaustive]`. |

### 10.3 `AtlasEvent` new field (`crates/atlas-trust-core/src/trace_format.rs`)

| Field | Tag | Notes |
|---|---|---|
| `pub author_did: Option<String>` | **V2-α-Additive (WIRE-BREAK for V1.0 readers)** | New optional field with `#[serde(default, skip_serializing_if = "Option::is_none")]`. Serialisation-side: events without `author_did` produce byte-identical JSON to V1. Deserialisation-side: V1.0 verifiers see `author_did` as `unknown_field` due to `deny_unknown_fields` and reject; V2-α verifiers accept. The struct's overall SemVer treatment moves from **Locked** (V1 baseline) to **V2-α-Additive with documented wire-break** until the version bump lands. |

### 10.4 `build_signing_input` signature change (`crates/atlas-trust-core/src/cose.rs`)

| Item | Tag | Notes |
|---|---|---|
| `pub fn build_signing_input(workspace_id, event_id, ts, kid, parent_hashes, payload_json, author_did: Option<&str>) -> TrustResult<Vec<u8>>` | **V2-α-Additive (SOURCE-BREAK for direct callers; WIRE-PRESERVING when `None`)** | New trailing parameter `author_did: Option<&str>`. Callers passing `None` get byte-identical CBOR output to V1 (V1 byte-determinism pin holds). Callers passing `Some(...)` get a longer CBOR map with the `"author_did"` key appended in RFC 8949 §4.2.1 order (last, due to longest encoded-key length). Direct downstream consumers of `build_signing_input` (atlas-signer, future Rust SDKs, custom verifiers) must update their callsites to pass `None` or a DID. Wire-format consumers (anything reading or writing events.jsonl) see no change unless they explicitly use the new field. |

### 10.5 Verifier behaviour change (`crates/atlas-trust-core/src/verify.rs`)

| Item | Tag | Notes |
|---|---|---|
| Pre-signature-check validation of `event.author_did` | **V2-α-Additive (NEW REJECT PATH)** | When `event.author_did` is `Some(_)`, the verifier now calls `validate_agent_did` before signing-input construction. Malformed DIDs produce a structured `AgentDidFormatInvalid` error rather than a downstream signature-mismatch. Events without `author_did` follow the unchanged V1 verifier path. |

### 10.6 V2-α Wire-Format Invariants (added 2026-05-12)

- **Cross-agent-replay defence (Phase 2 Security H-1):** when `author_did` is present, it is canonically bound into the signing-input alongside `kid`. An event signed by agent A in workspace X cannot be replayed as if signed by agent B in workspace X without breaking the signature.
- **V1 byte-determinism preservation:** `cose::tests::signing_input_byte_determinism_pin` retains its V1 pinned hex unchanged. V1-shaped events (no `author_did`) produce byte-identical CBOR pre- and post-Welle-1.
- **V2-α byte-determinism pin:** `cose::tests::signing_input_byte_determinism_pin_with_author_did` locks the exact CBOR bytes for a fixture event with `author_did = Some(...)`. Map header is `a8` (8 pairs); the new `author_did` entry sorts last per RFC 8949 §4.2.1 (encoded-key-length 11 = longest).
- **Sigstore Rekor binding:** `author_did` is part of the signed event body, so it is automatically part of the Rekor inclusion proof. No additional anchoring work needed at the V2-α layer.

### 10.7a V2-α Welle 3 — new `atlas-projector` crate (2026-05-12)

| Item | Tag | Notes |
|---|---|---|
| NEW workspace member `crates/atlas-projector` | **V2-α-Additive** | Layer-2 graph projection canonicalisation. Depends on `atlas-trust-core` only for `agent_did::validate_agent_did` cross-validation. Clean DAG: atlas-trust-core does NOT depend on atlas-projector. |
| `pub const PROJECTOR_SCHEMA_VERSION: &str = "atlas-projector-v1-alpha"` | **V2-α-Additive** | Bound into every `graph_state_hash` computation. SemVer-major to change — would invalidate every pinned graph-state-hash + `ProjectorRunAttestation` payload signed under the old schema (Welle 4 candidate). |
| `pub struct GraphNode { entity_uuid, labels, properties, event_uuid, rekor_log_index, author_did }` | **V2-α-Additive** | In-memory node representation. NOT serde-serialisable (intentional; isolates wire concerns from internal-state concerns). `author_did` Welle 1 schema-additive. |
| `pub struct GraphEdge { edge_id, from_entity, to_entity, kind, properties, event_uuid, rekor_log_index, author_did }` | **V2-α-Additive** | In-memory edge representation. Same conventions as `GraphNode`. |
| `pub struct GraphState { nodes: BTreeMap, edges: BTreeMap }` + `pub fn new()`, `upsert_node`, `upsert_edge`, `check_structural_integrity` | **V2-α-Additive** | Container is `BTreeMap` — **load-bearing invariant** for logical-identifier-sorted canonical output (per Welle 2 §3.5 caveat: `@rid` is insert-order, NOT logical identity anchor). |
| `pub fn build_canonical_bytes(state: &GraphState) -> ProjectorResult<Vec<u8>>` | **V2-α-Additive** | CBOR canonical encoding per RFC 8949 §4.2.1 (same convention as V1's `atlas_trust_core::cose::build_signing_input`). Single canonical-CBOR boundary; serde-serialise of `GraphState` is intentionally out of scope. |
| `pub fn graph_state_hash(state: &GraphState) -> ProjectorResult<[u8; 32]>` | **V2-α-Additive** | blake3 over canonical bytes. The hash projector-state-hash CI gate compares; the hash `ProjectorRunAttestation` events (Welle 4 candidate) will carry. |
| `pub enum ProjectorError` (with `#[non_exhaustive]`) — variants: `CanonicalisationFailed`, `MalformedAuthorDid`, `MalformedEntityUuid`, `DuplicateNode`, `DanglingEdge` | **V2-α-Additive (SemVer-minor under non_exhaustive)** | Local crate error type; does NOT alias or wrap `TrustError`. Future wellen may add conversion impls. |
| `pub type ProjectorResult<T> = Result<T, ProjectorError>` | **V2-α-Additive** | |
| `canonical::tests::graph_state_hash_byte_determinism_pin` — pinned blake3 `8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4` (754 canonical bytes) | **CI gate** | Drift detection. Co-equal with V1's `cose::signing_input_byte_determinism_pin` and Welle 1's `signing_input_byte_determinism_pin_with_author_did`. |

### 10.7 What `v2.0.0-alpha.1` will bring (forecast, not committed)

Pending the close-out of the V2-α welle bundle (Welle 1 = this surface; future Wellen = Projector + FalkorDB + ArcadeDB spike + content-hash separation if counsel-approved):
- Workspace version bump `1.0.1 → 2.0.0-alpha.1`
- `SCHEMA_VERSION` may move from `atlas-trace-v1` → `atlas-trace-v2-alpha` (decision deferred)
- All V1 **Locked** items reclassified for v2 contract review
- This audit document gets a v2 successor doc; `SEMVER-AUDIT-V1.0.md` stays as historical record of the v1 contract.
