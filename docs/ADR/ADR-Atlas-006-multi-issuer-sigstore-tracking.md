# ADR-Atlas-006 — Multi-Issuer Sigstore Tracking

| Field        | Value                                                    |
|--------------|----------------------------------------------------------|
| **Status**   | Tracking (not adopted)                                   |
| **Date**     | 2026-05-06                                               |
| **Wave**     | V1.18 Welle A                                            |
| **Authors**  | Nelson Mehlis (`@ThePyth0nKid`)                          |
| **Replaces** | —                                                        |
| **Superseded by** | —                                                   |
| **Related**  | V1.5 / V1.6 Sigstore anchoring, V1.14 Scope E (`@atlas-trust/verify-wasm` npm publish), V1.17 Welle A (`verify-wasm-pin-check@v1`), V1.17 Welle B (SSH tag-signing) |

---

## 1. Context

Atlas has three structurally distinct dependencies on Sigstore's single-issuer infrastructure today. Each uses Sigstore in a different role and would degrade differently under partition, compromise, or rotation. Single-issuer means there is one Fulcio Certificate Authority and one operational Rekor transparency log behind every signal.

### 1.1 The three touchpoints

**A. Rekor v1 anchor verification (Atlas trust-core).**
`crates/atlas-trust-core/src/anchor.rs` pins the Rekor v1 production signing pubkey (`SIGSTORE_REKOR_V1.pem`, P-256 SPKI) and the **roster** of accepted Trillian tree-IDs (`SIGSTORE_REKOR_V1.tree_id_roster`, currently a three-entry slice: the active production shard `1_193_050_959_916_656_506`, plus two historical shards `3_904_496_407_287_907_110` and `2_605_736_670_972_794_746`). The `SIGSTORE_REKOR_V1.active_tree_id` field is the *issuer-side* anchor (atlas-signer always posts to the active shard); the *verifier-side* acceptance check `SIGSTORE_REKOR_V1.is_known_tree_id` does roster membership against the full slice so historical anchors captured under earlier shards still verify. Atlas accepts an anchor as Sigstore-verified if (a) its `tree_id` is a roster member, (b) the C2SP-format origin line `"rekor.sigstore.dev - {tree_id}\n"` reconstructs against that tree-ID, and (c) the anchor signature verifies against the pinned pubkey. Adding a new shard is intentionally a source change requiring a crate-version bump — silent acceptance of unknown tree-IDs is exactly the forgery primitive the roster forbids. This pin is load-bearing for every Sigstore-anchored trace's "anchored ⇒ trustworthy" claim. Coverage: see `crates/atlas-trust-core/tests/sigstore_golden.rs` plus the `rekor_issuer_rosters_are_pinned` + `rekor_issuer_tree_id_membership` invariant tests. (V1.18 Welle B (2) — §5.1 — moved these from top-level constants onto the `SIGSTORE_REKOR_V1: RekorIssuer` static; data unchanged.)

**B. npm publish `--provenance` (`wasm-publish.yml`).**
On a tag-push that mints `@atlas-trust/verify-wasm` to npmjs.org, the workflow runs `npm publish --provenance`. This mints an OIDC token via the GitHub Actions ID-token service, exchanges it at Fulcio for a short-lived signing certificate, signs the tarball SHA512 + workflow metadata, and writes the attestation entry to Rekor. The npm registry stores the attestation; downstream consumers can pull it via `npm audit signatures`. Single-issuer dependency: Fulcio (Sigstore's CA) + Rekor (Sigstore's transparency log). A Fulcio compromise breaks attestation issuance; a Rekor compromise/partition breaks attestation verification.

**C. `npm audit signatures` round-trip (Welle A consumer-side).**
The `verify-wasm-pin-check@v1` composite action's Layer 3 self-test runs `npm audit signatures` on the consumer's resolved tree. That command resolves `@atlas-trust/verify-wasm@<version>` from the registry, fetches its provenance attestation, and verifies the OIDC-Sigstore chain back to a configured trust root. Single-issuer dependency: same Fulcio + Rekor as path B, plus the npm-side TUF root that bootstraps both. A Rekor outage during a consumer's CI run produces a silent "no attestation, can't verify" path that downgrades Layer 3 from active to passive — the threat model in `docs/SECURITY-NOTES.md` **scope-k** (the Welle A consumer-action analysis) covers this as a known degradation, including the air-gapped/offline-mirror note that explicitly bounds Sigstore network dependencies.

### 1.2 Why this is one ADR, not three

The three touchpoints share a common root: every Sigstore-related verification in Atlas terminates at the same Rekor v1 production instance run by the Sigstore Foundation. A single Rekor-side incident — operational outage, key compromise, regulatory takedown, deliberate adversarial action against the Sigstore project — degrades all three simultaneously. The decision-space (track, prepare, adopt, fall back) is the same for all three; the adoption work would touch all three pin sites; and the operator runbook for an actual incident would coordinate all three responses. Splitting into three ADRs would force three near-duplicate decision records that drift over time.

### 1.3 What "multi-issuer" means in this context

Multi-issuer refers to redundancy in **two** independent layers:

1. **Multi-CA**: Multiple Fulcio-equivalent Certificate Authorities (potentially federated across Sigstore Public Good, AWS-hosted, Google Cloud, internal Sigstore deployments). A signing operation can choose any CA; verification accepts any CA listed in the trust-root TUF metadata.
2. **Multi-log**: Multiple Rekor-equivalent transparency logs (witness-anchored cross-log, similar to certificate-transparency's multi-log model). A signing operation writes to any log; verification accepts any log listed in the trust-root TUF metadata, with cross-log inclusion proofs as a stronger variant.

These are independent: multi-CA without multi-log shifts the single-point-of-failure from CA to log; multi-log without multi-CA shifts it the other way. True redundancy requires both.

### 1.4 Why this matters for Atlas specifically

- **Atlas advertises supply-chain trust as a primary product property.** A verifier whose own anchor verification or whose own published npm artefact can silently degrade under upstream-Sigstore stress is a credibility hazard out of proportion to the underlying technical risk.
- **Welle B's SSH tag-signing is the primary trust root for Atlas releases**, with Sigstore as a defence-in-depth side channel. But Welle A's Layer 3 `npm audit signatures` makes Sigstore the *only* attestation-chain authority for the npm-distributed wasm verifier. If a downstream auditor only validates via Welle A's Layer 3, they have no fallback to Welle B's tag signature without the OPERATOR-RUNBOOK reproduce-from-source flow.
- **Atlas anchors are public commitments.** Anchor verification regressions affect not just our own trust posture but every external party who has cached or referenced an Atlas-issued trace.

---

## 2. Decision

**Atlas tracks Sigstore's multi-issuer evolution upstream and does not adopt any multi-issuer mechanism into the verifier core, the publish lane, or the consumer-side action until at least one of the adoption triggers in §4 fires.** Until then:

- The pinned Rekor v1 production pubkey + tree-ID in `anchor.rs` remain authoritative for all Sigstore-path anchor verification.
- `wasm-publish.yml` continues to use the default `--provenance` path (Fulcio + Rekor v1 production).
- `verify-wasm-pin-check@v1` continues to use the default `npm audit signatures` path.
- Welle B SSH tag-signing remains the primary trust root for Atlas releases; Sigstore is documented as the secondary channel with explicit single-issuer caveats.

This is a **deliberate non-decision**, not procrastination. The cost of premature adoption (premature schema lock-in, alpha-quality upstream APIs, divergent verification semantics across consumers) outweighs the marginal risk reduction from joining a federation that itself is not yet stable.

---

## 3. Status of upstream multi-issuer work (as of 2026-05-06)

This section is the working snapshot of upstream state. Treat numbers older than 90 days as stale and reverify at the next quarterly review (§7).

### 3.1 Sigstore Rekor v2 ("scaling Rekor")

The Sigstore project has publicly committed to a Rekor v2 architecture that addresses Rekor v1's known scaling and operational-cost limits. The v2 design moves from a single Trillian tree to a sharded, witness-anchored model. This is **not multi-issuer in itself** — a single Sigstore Foundation–operated v2 instance still has the same single-operator dependency — but v2 is a *prerequisite* for federated multi-instance deployment because it formalises the cross-shard inclusion proof structure that any multi-issuer verification must consume.

Atlas exposure: when Rekor v2 ships and the npm registry switches to v2-issued attestations, `npm audit signatures` will need to handle v2-format entries. The current `SIGSTORE_REKOR_V1.tree_id_roster` will continue to verify v1-issued anchors (which auditors may have captured years ago and replay against the verifier indefinitely); v2 will need a parallel `SIGSTORE_REKOR_V2: RekorIssuer` static (with its own `pem`, `origin`, `active_tree_id`, `tree_id_roster`) appended to `REKOR_ISSUERS`. The §5.1 registry-pattern refactor (DONE in V1.18 Welle B (2)) formalises this so v1 and v2 issuers coexist via a uniform `&[&RekorIssuer]` slice — adding v2 is now a single-static + single-slice-extension change, no call-site touches required.

### 3.2 npm-side TUF root rotation

The npm registry's Sigstore trust root is itself a TUF-formatted document that lists which Fulcio CAs and which Rekor logs are accepted for `--provenance` attestations. Today this lists exactly one Fulcio (`https://fulcio.sigstore.dev`) and one Rekor (`https://rekor.sigstore.dev`). When npm adds a second entry (e.g. a backup Sigstore deployment for resilience), `npm audit signatures` will gain implicit multi-issuer support without any client-side change — provided the client's `npm` version is recent enough to consume the updated root. This is the path of least resistance for Atlas: adopt by upgrading `npm` in the consumer-side action, not by writing new verification code.

Atlas exposure: the Welle A composite action currently does not pin an `npm` version. A future npm version that gains multi-issuer trust-root handling will be picked up automatically by consumers that follow the action's recommended setup. Track via the npm changelog and the `package@latest-9` / `package@latest-10` major-version cuts.

### 3.3 `tuf-conformance` and federation-readiness testing

The Sigstore project runs a `tuf-conformance` test suite that asserts a TUF-format trust root is correctly handled by client implementations. Federation-readiness depends on this suite covering multi-issuer scenarios end-to-end. As of 2026-05-06 the suite covers single-issuer rotation extensively; multi-issuer scenarios are present but not yet load-bearing for any production deployment we can verify. When `tuf-conformance` includes a "two Fulcio CAs, two Rekor logs, partial-failure" scenario as a release blocker for Sigstore client SDKs, that is a strong signal that production deployments are imminent.

### 3.4 Alternative transparency logs

Beyond Sigstore Public Good, several entities have stood up Rekor-compatible logs:

- AWS-hosted Sigstore (announced; pre-production at last public update).
- Google Cloud `cloud-sigstore` (internal, not publicly verifiable as of writing).
- Various internal-corporate Sigstore deployments behind enterprise registries.

None of these is yet a **second issuer in the npm trust root** and therefore none affects Atlas's Welle A or wasm-publish lane today. Track AWS's productionisation as the most likely first second-issuer in the npm root.

### 3.5 Sigstore Foundation governance

A federation requires participants who agree on shared root-of-trust governance. The Sigstore Foundation's published governance model (member organisations, root key ceremony participants, log operator agreements) is the bottleneck for federation, not the technology. Track the foundation's annual root-ceremony output for changes in the issuer set.

---

## 4. Adoption triggers

Atlas opens a follow-on V1.x scope to **adopt** multi-issuer Sigstore verification when **any one** of the following triggers fires:

### Trigger A — npm trust root gains a second entry

The npm-published TUF trust root document lists a second Fulcio CA OR a second Rekor log alongside the current Sigstore Public Good entries. **Verification procedure** (this is a known gap in the trigger as written — `npm audit signatures --json` does NOT expose the underlying TUF root; it only reports per-package attestation outcomes): until npm publishes a canonical CLI command for inspecting the bundled trust root, the operational verification path is to (a) inspect the npm CLI's bundled `trusted_root.json` at `$(npm root -g)/npm/node_modules/sigstore/store/trusted_root.json` (path may shift across npm versions — verify against the npm CLI source for the active major version), (b) cross-reference against `https://github.com/sigstore/root-signing/tree/main/repository/repository/targets` for the upstream TUF root that npm bundles, and (c) note the Sigstore Foundation blog announcement of the second-issuer addition for canonical confirmation. The trigger fires when all three sources agree on a second entry. This is a **known reproducibility weakness**: until npm exposes the trust root as first-class CLI output, the trigger is observable but not single-command-checkable. Adoption work below assumes the trigger has been confirmed by an operator following the procedure above. Atlas-side work then: bump the action's recommended `npm` version, regenerate the Welle A self-test fixtures with multi-issuer attestations, document the new fallback semantics in `docs/CONSUMER-RUNBOOK.md`.

### Trigger B — Rekor v2 issued for `@atlas-trust/verify-wasm`

The first time we cut a release where `npm publish --provenance` records the attestation in Rekor v2 (rather than v1), the verifier core needs to handle v2-format anchors. Trigger work: extend `anchor.rs` to accept v2 entries by adding a `SIGSTORE_REKOR_V2: RekorIssuer` static (pem + origin + active_tree_id + tree_id_roster) and appending `&SIGSTORE_REKOR_V2` to `REKOR_ISSUERS` (post-V1.18 Welle B (2) registry pattern), update `crates/atlas-signer/src/rekor_client.rs` to negotiate v2 endpoints, port the golden-test fixtures, validate cross-format mixed-mode (v1-anchor + v2-anchor in the same chain). The registry refactor reduces this from ~1 week to ~3-4 days since the verifier dispatch already iterates issuers. No design ambiguity.

### Trigger C — Sigstore Public Good incident with documented degradation

A public incident affecting Rekor v1 availability or integrity (operational outage longer than 24 hours, rotation of `SIGSTORE_REKOR_V1.pem` outside the planned ceremony schedule, any documented integrity breach). Atlas's response is not adoption-of-multi-issuer in the moment (which would be reactive and fragile) but an **operator-runbook activation**: switch downstream-consumer guidance to Welle B SSH-tag verification, with adoption planning starting in parallel.

**Documentation gap acknowledged.** As of 2026-05-06 `docs/CONSUMER-RUNBOOK.md` does NOT have a "Sigstore Public Good incident — extended degradation protocol" section. §6 of that runbook covers reproduce-from-source for general unreachable-registry scenarios, and §5 covers transient-outage retries, but neither prescribes the specific "abandon `npm audit signatures`, fall back to SSH-tag-only primary verification, document the incident reference in your build logs" protocol that Trigger C activation would require. Filling this gap is an **explicit prerequisite for Trigger C readiness** and is recorded as a follow-on task: open `docs(consumer-runbook)/sigstore-incident-protocol` to add §10 "Sigstore Public Good incident protocol" before the next ADR refresh, regardless of whether Trigger C has fired by then. Until that section ships, an actual Trigger C event would require ad-hoc operator communication to consumers, which is exactly the failure mode an incident runbook is supposed to prevent.

Trigger work then proceeds along Trigger A or B paths once the upstream landscape stabilises.

### Trigger D — Atlas-side compliance requirement

A specific external compliance requirement (a customer audit, a regulatory framework, a major-customer SOC-2 line item) explicitly demands multi-issuer redundancy as a property of the supply chain. This trigger differs from A/B/C in that the response is driven by the requirement's specifics, not by upstream readiness — Atlas may need to ship a partial / alpha-quality multi-issuer verifier to meet a contractual deadline even if upstream is not fully ready. Document the requirement in this ADR's superseded-by chain when it fires.

### Triggers that explicitly do NOT fire adoption

- Sigstore Foundation announcing federation plans without a deployed second issuer.
- A second issuer existing in some Sigstore ecosystem trust root that is not the npm-published trust root.
- Internal-corporate Sigstore deployments without public verifiability.
- Academic / research multi-issuer schemes without production npm/registry adoption.

The principle is that Atlas tracks production-deployable redundancy, not architectural readiness. Architectural readiness without production adoption is rejection from the same single-point-of-failure with extra steps.

---

## 5. Atlas-side preparation work

Even without adopting multi-issuer today, three preparatory steps lower the future adoption cost:

### 5.1 Refactor `anchor.rs` constants to a registry pattern

Originally `SIGSTORE_REKOR_V1_PEM`, `SIGSTORE_REKOR_V1_ACTIVE_TREE_ID`, and `SIGSTORE_REKOR_V1_TREE_IDS` (the unified active-plus-historical roster) were top-level constants tied to a single issuer. Multi-issuer adoption prefers a `RekorIssuer { name, pem, origin, active_tree_id, tree_id_roster }` struct and a `&[&RekorIssuer]` slice so verification iterates issuers and applies roster membership per-issuer. The refactor is mechanical and does not change verification semantics. The `sigstore_tree_id_roster_is_pinned` invariant test became per-issuer (`rekor_issuer_rosters_are_pinned` + `rekor_issuer_tree_id_membership`) with an extensibility match arm so adding a future Rekor v2 issuer requires only appending a `RekorIssuer` static + extending the match.

**Status**: **DONE in V1.18 Welle B (2)**. Shipped as `feat(v1.18/welle-b): anchor.rs RekorIssuer registry refactor — ADR-006 §5.1`. New shape:

```rust
pub static SIGSTORE_REKOR_V1: RekorIssuer = RekorIssuer {
    name: "sigstore-rekor-v1",
    pem: "...",
    origin: "rekor.sigstore.dev",
    active_tree_id: 1_193_050_959_916_656_506,
    tree_id_roster: &[
        1_193_050_959_916_656_506,
        3_904_496_407_287_907_110,
        2_605_736_670_972_794_746,
    ],
};

pub const REKOR_ISSUERS: &[&RekorIssuer] = &[&SIGSTORE_REKOR_V1];
```

Tree-ID membership is `SIGSTORE_REKOR_V1.is_known_tree_id(tree_id)`. Adding Rekor v2 (Trigger B in §4) is now a single-static + single-slice-extension change, no call-site touches needed.

### 5.2 Document the inline-pin-update protocol

`SIGSTORE_REKOR_V1.pem` and the `tree_id_roster` array will need updates when the next v1 root ceremony happens, AND when v2 lands, AND when a second issuer joins the trust root. The protocol for these updates (PR review requirements, golden-test fixture regeneration, cross-version-anchor compatibility test) is currently implicit. Documenting it in `docs/OPERATOR-RUNBOOK.md` §15 makes the update path auditable.

**Status**: **DONE in V1.18 Welle B (4)**. Shipped as `docs(v1.18/welle-b): OPERATOR-RUNBOOK §15 inline-pin-update protocol + SECURITY-NOTES scope-k → ADR-006 forward link`. [OPERATOR-RUNBOOK §15](../OPERATOR-RUNBOOK.md) covers: trigger taxonomy (root ceremony / shard rotation / historical-shard discovery / Rekor v2 launch / second-issuer adoption), pre-edit verification gates, the step-by-step pin-update recipe (single-field discipline, signed commit, branch-protection traversal), seven-item PR review checklist, golden-fixture regeneration helper, cross-version-anchor compatibility test (the load-bearing gate against breaking historical anchor verification — §8.3 open question on multi-key-issuer interop is enforced here pre-merge), and a failure-modes table. CONSUMER-RUNBOOK §10.6 closure step 4 was updated in the same PR to point at §15 as the canonical operator path. The cross-version-anchor compat test specifically forbids shipping a PEM rotation that breaks prior-version fixture verification — that case forces opening a follow-on ADR for multi-key-issuer support before the rotation can land, closing the §8.3 interop question pre-merge instead of post-incident.

### 5.3 Add a Sigstore-degraded-mode flag to `verify-wasm-pin-check@v1`

Today the action's Layer 3 `npm audit signatures` either passes (active verification) or skips (no attestation found). There is no third "Sigstore round-trip attempted but failed due to upstream outage" state, so a transient Rekor outage looks like an unsigned package. A `SIGSTORE_DEGRADED_OK` opt-in input would let consumers explicitly accept "Layer 1 + Layer 2 PASS, Layer 3 attempted but Sigstore unreachable — proceed with reduced assurance" rather than failing the build. This is **not** about multi-issuer per se but is a robustness improvement that becomes more valuable as the ecosystem evolves.

**Status**: deferred to V1.18 Welle C or later as a `feat(v1.18/welle-c)` candidate. **Open threat-model problem (NOT solved by an obvious quick fix).** The naive defence — "require a fresh liveness probe to `https://rekor.sigstore.dev/api/v1/log/publicKey` before accepting degraded mode, with a per-repo per-week cap" — is **defeatable by the same network-positioned adversary class the feature is meant to exclude**. A targeted DNS or BGP hijack against `rekor.sigstore.dev` for the duration of one probe causes the liveness probe to return 200 (because the probe and the audit round-trip both resolve to the same domain via the same path) while the attacker simultaneously serves a forged "no attestation" response or a stale-but-syntactically-valid attestation. The probe and the protected operation share a fate. The per-week cap mitigates opportunistic abuse but not a targeted attacker. Adequate independence requires probing through a route that does NOT share a fate with the audit round-trip — candidates: (i) a cross-log consistency check against an independent Sigstore-compatible log (only possible after multi-issuer adoption — circular), (ii) the Sigstore Foundation's status page over a separately-anchored TLS certificate chain (different CA, different DNS authoritative servers), (iii) an Atlas-operated witness that records canonical Rekor checkpoints out-of-band and is itself anchored. None of (i)/(ii)/(iii) is trivially correct. This sub-section therefore records the *intent* of a degraded-mode flag, the *naive defence and why it fails*, and *defers the actual design* to a future ADR-Atlas-00X that addresses fate-shared-channel verification specifically. Do not ship `SIGSTORE_DEGRADED_OK` until that ADR lands.

---

## 6. Mitigations in place today

The "tracking, not adopting" decision is acceptable today because Atlas already has defence-in-depth that makes Sigstore single-issuer dependency a non-fatal degradation rather than a catastrophic single-point-of-failure:

- **Welle B SSH-signed tags are the primary trust root for Atlas releases.** A consumer who only trusts Welle B (verifies the tag signature with the keys in `.github/allowed_signers`, ignores npm provenance entirely) is unaffected by any Sigstore incident. The CONSUMER-RUNBOOK §6 reproduce-from-source path documents this fallback.
- **Welle C trust-root mutation defence + operator-side Repository Ruleset** ensures Welle B's SSH trust root cannot be silently swapped, even by an attacker with a compromised maintainer PAT. So the Welle B fallback is itself defended.
- **Atlas anchor verification pins the Rekor v1 pubkey and tree-ID directly in compiled code.** A Rekor-side key rotation would break verification (defence-in-depth: false negatives are safer than false positives); operator response is to ship a verifier-core update with the new pin, golden-test verified.
- **`scope-k` (Welle A consumer-action) threat model section in `docs/SECURITY-NOTES.md`** explicitly enumerates the Sigstore single-issuer dependencies in the npm-publish + npm-audit-signatures path and the documented degradation paths (including the air-gapped/offline-mirror note). **`scope-l`** (Welle B tag-signing) covers the SSH primary trust root that remains active independent of any Sigstore state. External auditors have access to both analyses as part of the verifier evaluation.
- **TOCTOU-pinned GH-Release object via `--target ${VERIFIED_SHA}`** (V1.17 Welle B) means a Sigstore-side compromise of the npm provenance cannot retroactively rewrite the GH-Release-distributed tarball binding.
- **Per-algo minimum base64 payload length checks in `verify-wasm-pin-check@v1`** (V1.17 Welle A post-review) prevent empty-hash bypass even if a Sigstore-level signature were somehow valid for an empty payload.

These mitigations are listed not to argue that multi-issuer is unnecessary but to bound the immediate risk: a Sigstore Public Good incident degrades one of three Atlas verification surfaces, while two of three (Welle B, Welle C, anchor-pinning) remain fully active. The ADR explicitly accepts this asymmetric risk profile until upstream multi-issuer infrastructure makes adoption a low-friction lift.

---

## 7. Watchlist + review cadence

### 7.1 Specific upstream channels

- **Sigstore project blog and release notes** — `https://blog.sigstore.dev/` for foundation announcements, root-ceremony output, federation milestones.
- **Rekor releases on GitHub** — `https://github.com/sigstore/rekor/releases` for v2 progression.
- **Sigstore protobuf-specs** — `https://github.com/sigstore/protobuf-specs` for the wire format that v2 entries will use.
- **npm CLI changelog** — for trust-root format updates and `npm audit signatures` behaviour changes.
- **`tuf-conformance` test additions** — `https://github.com/theupdateframework/tuf-conformance` for the multi-issuer test scenarios that gate client-SDK readiness.
- **The Sigstore Public Good operations status page** for incident transparency.

### 7.2 Review cadence

- **Quarterly ADR refresh.** At the start of each calendar quarter, re-verify §3 status numbers and update §7 tracking links. If any §3 sub-section materially changes (e.g. Rekor v2 ships GA, npm root gains a second issuer), open a follow-on `docs(adr-006/refresh)` PR with the updated state plus an explicit decision: do any §4 triggers now fire? If yes, open the corresponding adoption-work scope.
- **Incident-triggered refresh.** If a §4 Trigger C event occurs, refresh this ADR within one operational week of the incident and append an incident-record sub-section under §7.
- **Major-Atlas-version refresh.** At every Atlas Vx.0 cut (V2.0, V3.0, …), re-evaluate this entire ADR for relevance — by V3.0 the Sigstore landscape may make the entire question moot or may have shifted to a different model (e.g. cosign moves to a non-transparency-log architecture).

### 7.3 Out-of-cadence triggers

The triggers in §4 fire adoption work regardless of review cadence. Cadence is for the tracking-state itself, not the adoption decision.

---

## 8. Consequences

### 8.1 Accepted today

- Single-issuer dependency on rekor.sigstore.dev for npm provenance verification (Welle A Layer 3 + npm publish).
- Single-issuer dependency on Rekor v1 production for Atlas's own anchor verification (trust-core).
- A Rekor-side incident degrades all three Atlas Sigstore touchpoints simultaneously.

### 8.2 Mitigated

- Welle B SSH primary + Welle C trust-root defence cover the ship-side trust root independently of Sigstore state.
- Anchor verification is pinned at compile time, so an attacker controlling Rekor cannot silently substitute a different root — they can only degrade liveness.

### 8.3 Open questions

- How do we test multi-issuer code paths in CI before adoption, given that the Sigstore ecosystem has no test multi-issuer environment we can wire into Welle A's self-test? Likely requires a mock-Sigstore-multi-issuer fixture, parallel to the existing mock-Rekor fixture in `crates/atlas-trust-core/tests/`.
- What is the interop story between Atlas anchor verification (pinned Rekor v1) and a multi-issuer-only consumer who has discarded the Rekor v1 trust root? This is a long-tail concern: today's anchors will need to remain verifiable for years even after multi-issuer adoption. Likely answer: keep `SIGSTORE_REKOR_V1` in `REKOR_ISSUERS` indefinitely (post-V1.18 Welle B (2) the registry shape supports this trivially), with a documented "verifies anchors issued before $DATE" semantic per-issuer.
- Does Atlas eventually want to operate its OWN Rekor instance as one of the federated issuers (not for general use, but for Atlas-specific anchors)? This is a V2+ question; recorded here as a long-horizon hypothetical.

### 8.4 Reversibility

This decision is fully reversible. Adoption work is prepared (§5), upstream tracking is documented (§7), trigger conditions are concrete (§4). At any point a Trigger fires, this ADR's Status field updates to "Adopting (Trigger X)", a follow-on ADR documents the adoption design, and the work proceeds. No technical debt is being incurred; the only cost is the ongoing tracking effort, which is deliberately scoped to quarterly cadence to keep it bounded.

---

## 9. Decision log

| Date       | Event                                                  | Outcome |
|------------|--------------------------------------------------------|---------|
| 2026-05-06 | ADR-Atlas-006 opened. Initial status: Tracking.        | —       |
| 2026-05-06 | V1.18 Welle B (2): §5.1 registry-pattern refactor shipped (`feat(v1.18/welle-b): anchor.rs RekorIssuer registry refactor`). | §5.1 status flipped to DONE. Reduces Trigger B (Rekor v2) adoption work from ~1 week to ~3–4 days. No verification semantics changed. |

(Future quarterly refreshes append rows here.)
