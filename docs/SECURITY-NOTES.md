# Atlas Verifier — Defended Attack Surface (V1.10)

This document describes what the V1.10 verifier (`atlas-trust-core`, exposed as
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
  reproducibility. Any production deployment MUST set
  `ATLAS_PRODUCTION=1` (which causes the per-tenant subcommands to
  refuse to run with the dev seed) and either supply a sealed-seed
  loader (V1.10) or otherwise replace `DEV_MASTER_SEED` before going
  live.
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
- **Production gate (V1.9 — superseded by V1.10):** All V1.9 per-tenant
  subcommands (`derive-key`, `derive-pubkey`, `rotate-pubkey-bundle`, and
  `sign --derive-from-workspace`) called `keys::production_gate()`, which
  refused to run when `ATLAS_PRODUCTION=1` was set. V1.9 had no sealed-seed
  loader; the gate ensured a production environment could not silently sign
  with the source-committed dev master seed. The opt-out shape was a
  footgun (residual #6 below): forgetting the env var let the dev seed
  through. V1.10 supersedes this with a positive opt-in
  `keys::master_seed_gate()` — see *Master-seed gate inversion (V1.10)*
  below. The V1.9 paranoia check is preserved as the inner layer:
  `ATLAS_PRODUCTION=1` still refuses the dev seed regardless of the V1.10
  opt-in, so a deployment with both set still fails closed.
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
  dev seed) layered on top of the V1.9 paranoia gate
  (`ATLAS_PRODUCTION=1` still refuses the dev seed unconditionally).
  See *Master-seed gate inversion (V1.10)* below for the gate's
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
- **[CLOSED in V1.10] The production gate is opt-out, not opt-in.**
  V1.9 shipped `production_gate()` as a *negative* guard — it blocked
  per-tenant signing only when `ATLAS_PRODUCTION=1` was explicitly
  set. A production deployment that forgot to set the env var would
  run happily with the source-committed `DEV_MASTER_SEED` and emit
  signed events whose pubkeys an auditor could re-derive from public
  source. V1.10 wave 1 inverts this footgun: per-tenant subcommands
  refuse to start unless `ATLAS_DEV_MASTER_SEED=1` is *positively
  asserted*. A deployment that forgets the env var now fails closed
  with an actionable error, not "happily signs with public-source
  keys". The V1.9 `ATLAS_PRODUCTION=1` paranoia check is preserved as
  the inner layer for defence-in-depth. See *Master-seed gate
  inversion (V1.10)* below.

---

## Master-seed gate inversion (V1.10 — in scope)

V1.10 wave 1 closes the V1.9 footgun where forgetting
`ATLAS_PRODUCTION=1` silently allowed the source-committed
`DEV_MASTER_SEED` to sign production traffic. The gate is now
positive: per-tenant subcommands refuse to start unless an explicit
dev opt-in is wired, *and* the V1.9 paranoia check still passes.

- **Layered defence:** `keys::master_seed_gate()` calls
  `production_gate_with()` first, so a deployment with
  `ATLAS_PRODUCTION=1` refuses the dev seed regardless of the V1.10
  opt-in. Only deployments with `ATLAS_PRODUCTION` unset *and*
  `ATLAS_DEV_MASTER_SEED=1` set obtain a `DevMasterSeedHkdf`. All
  four `(ATLAS_PRODUCTION × ATLAS_DEV_MASTER_SEED)` combinations are
  enumerated in `docs/OPERATOR-RUNBOOK.md` §1 with the security
  outcome.
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
- **Adversary tests:** 21 new unit tests in
  `crates/atlas-signer/src/keys.rs` cover the gate's allow-list,
  layered V1.9 paranoia precedence (`ATLAS_PRODUCTION=1` overrides
  even with `ATLAS_DEV_MASTER_SEED=1` set), error-message stability,
  trait dispatch through `&dyn`, equivalence between the trait-routed
  and explicit-seed paths, and Send+Sync witness on
  `DevMasterSeedHkdf`. The MCP smoke test
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
  load-bearing audit guarantee. A separate orthogonal lint —
  refusing to start when `ATLAS_PRODUCTION=1` is set with neither
  the HSM trio nor `ATLAS_DEV_MASTER_SEED=1` — is left as a
  V1.10.1 preflight tightening; today the dev opt-in plus
  `ATLAS_PRODUCTION=1` combination still refuses at the gate, so
  the gap is "config yells with two distinct error messages" not
  "config silently signs unsafely".

---

## Crate boundary

The verifier crates (`atlas-trust-core`, `atlas-verify-cli`,
`atlas-verify-wasm`, `atlas-signer`) are licensed Apache-2.0. An auditor
or regulator who receives an Atlas trace bundle is free to fork, build,
and run them under Apache-2.0, with no obligation under the
Sustainable Use License that governs the server, web frontend, and MCP
server in `apps/`.

This is the load-bearing trust property of Atlas: any third party can
independently verify a trace bundle without buying anything from us.

---

## Reporting issues

Verifier vulnerabilities — bypasses, signature-acceptance bugs,
canonicalisation drift, side-channel leaks — should be disclosed
privately to nelson@ultranova.io. We will respond within 48 hours.

A vulnerability that lets a forged trace verify as `valid: true` is the
worst possible class of bug for this project; we take such reports
seriously and will publish a fix + advisory on a co-ordinated timeline.
