# Atlas — Regulatory Compliance Mapping (V1.7)

Atlas does not market compliance; it ships structural evidence. This
document maps each obligation we claim to address to (a) the verbatim
clause, (b) the Atlas mechanism that delivers the evidence, and (c) the
specific test or trace field a third party can inspect to confirm the
mechanism works.

If a row reads "V1.8" or "V2", that obligation is on the roadmap and is
not yet load-bearing in V1.7. We mark it explicitly rather than implying
coverage.

The verifier is licensed Apache-2.0, so a regulator can rebuild the
verifier from source and re-run every check listed below without
contacting us. That is the structural property. The rows below are the
specific reasons it matters per framework.

---

## 1. EU AI Act (Regulation (EU) 2024/1689)

### Article 12 — Record-keeping

> *"High-risk AI systems shall technically allow for the automatic
> recording of events ('logs') over the lifetime of the system. […]
> The logging capabilities shall ensure a level of traceability of the
> AI system's functioning throughout its lifecycle that is appropriate
> to the intended purpose of the system."*
>
> *In force: 2026-08-02 for high-risk systems.*

| Obligation | Atlas mechanism | Inspectable evidence |
|---|---|---|
| Automatic recording of events | MCP server signs every agent write before persistence; no path to write the graph that bypasses signing | `apps/atlas-mcp-server` source — only `write_node`/`write_annotation` tools, both shell out to `atlas-signer` |
| Lifetime coverage | Append-only event log, DAG-linked, no in-place edits | `AtlasEvent` has no update path; `parent_hashes` links every change to its predecessor |
| Independently verifiable traceability | Bundle export + offline verifier under Apache-2.0 | `atlas-verify-cli verify-trace bundle.json -k pubkey-bundle.json` returns ✓ VALID with per-check evidence |
| Tamper-evidence | Constant-time hash + signature recompute on every event; V1.6 anchors of `bundle_hash` + `dag_tip` against pinned transparency-log pubkeys (mock-Rekor v1.5 or live Sigstore Rekor v1) defend against post-hoc bundle-swap and tail-truncation; V1.7 anchor-chain tip-rotation cross-links consecutive batches so past anchored state cannot be silently rewritten — chain head hash becomes the new load-bearing identity | `tampered_payload_detected` + `cross_workspace_replay_rejected` + `anchor_with_bogus_proof_is_rejected` integration tests + `verifies_real_sigstore_rekor_entry` golden-entry test + 15 adversary tests in `crates/atlas-trust-core/tests/anchor_chain_adversary.rs` (reorder, gap, head mismatch, previous_head break, coordinated rewrite) |

### Annex IV §1(e) — Technical documentation

> *"Where applicable, the human oversight measures needed in
> accordance with Article 14, including the technical measures put in
> place to facilitate the interpretation of the outputs of AI systems
> by the deployers."*

| Obligation | Atlas mechanism | Inspectable evidence |
|---|---|---|
| Named human verifier in the trail | `kid = spiffe://atlas/human/<email>` on `annotation.add` events; the verifier's identity is bound into the signing-input | Bank demo `01HRQ003VERIFY` event signed by `spiffe://atlas/human/sebastian.meinhardt@bankhaus-hagedorn.de` |
| Decision rationale captured | `annotation.add` payload carries `decision`, `evidence`, free-form rationale | `examples/golden-traces/bank-q1-2026.trace.json` event 3 |
| Tied to a specific model version | Annotation's `parent_hashes` links it to the model-creation event | DAG visualisation in `apps/atlas-web/src/components/KnowledgeGraphView.tsx` |

### Article 14 — Human oversight

> *"High-risk AI systems shall be designed and developed in such a way
> […] that they can be effectively overseen by natural persons during
> the period in which the AI system is in use."*

| Obligation | Atlas mechanism | Inspectable evidence |
|---|---|---|
| Demonstrable human-in-the-loop | `annotation.add` events are required by policy (V2 enforces; V1 records) | `AtlasPayload::AnnotationAdd` variant in `trace_format.rs` |
| Override audit trail | Human's annotation event sits *after* the agent's output event in the DAG, signed with the human's `kid` | Standard pattern in bank demo |

---

## 2. GAMP 5 Second Edition + ISPE GAMP RDI Good Practice Guide on AI/ML (July 2025)

### Appendix D11 — AI/ML in regulated GxP context

GAMP 5 second-edition Appendix D11 frames ML system validation around
**data integrity (ALCOA+)**, **lifecycle traceability**, and
**explainability evidence**.

| ALCOA+ attribute | Atlas mechanism | Inspectable evidence |
|---|---|---|
| **A**ttributable | Every event signed by a `kid`; SPIFFE-ID identifies the actor | Every `EventSignature.kid` in any trace |
| **L**egible | Trace bundle is JSON; verifier output is human-readable evidence list | `print_human()` in `atlas-verify-cli/src/main.rs` |
| **C**ontemporaneous | `ts` field RFC 3339, validated at parse; V1.6 transparency-log anchor (mock or Sigstore) witnesses each `dag_tip` at issuance time | `non_rfc3339_timestamp_rejected` test + `anchor_with_bogus_proof_is_rejected` test + `verifies_real_sigstore_rekor_entry` test |
| **O**riginal | Append-only; original event_hash preserved across DAG; V1.6 `bundle_hash` anchor (mock or Sigstore) binds the keyset-of-record at witness time; V1.7 anchor-chain tip-rotation makes the chain head the load-bearing identity — any post-hoc rewrite of past batches breaks the chain, detectable offline | `AtlasEvent` has no mutation API; anchor entries with `kind: "bundle_hash"` in any V1.6+ trace bundle; `trace.anchor_chain.history` and `verify_anchor_chain` checks in V1.7 bundles |
| **A**ccurate | blake3 hash of canonical signing-input recomputed on every verify; anchor inclusion proof recomputed against pinned log pubkey | `tampered_payload_detected` + `tampered_anchored_hash_fails` tests |
| Complete (the +) | DAG `parent_hashes` enforces "no missing event" | `dag_tip_mismatch_rejected` + `check_parent_links` |
| Consistent | Single canonicalisation crate used by signer + verifier | Architecture §3.1 (one Rust source-of-truth) |
| Enduring | Apache-2.0 verifier, JSON trace bundles | Customer can re-verify decades-old bundles with a future verifier build, provided the schema_version matches |
| Available | Trace bundles are self-contained files | No external service dependency for verification |

### GAMP RDI on training-data lineage

| Obligation | Atlas mechanism | Inspectable evidence |
|---|---|---|
| Training-data provenance | `node.create` event with `kind: "dataset"` includes `source` and `schema_hash` | Bank demo event `01HRQ001IMPORT` |
| Model derivation chain | Model event's `parent_hashes` reference the training-dataset event | Bank demo: `01HRQ002TRAIN` parent = `01HRQ001IMPORT.event_hash` |
| Inference-to-model link | Inference event's `parent_hashes` reference the verified-model event | Bank demo: `01HRQ004PREDICT` parent = `01HRQ003VERIFY.event_hash` |

---

## 3. ICH E6(R3) — Good Clinical Practice (effective 2025-07-23)

### §7.4 — Data lineage and audit trail

> *"The audit trail should be sufficient to reconstruct the course of
> events relating to the creation, modification, or deletion of
> electronic data. Audit trails should be available for review by the
> sponsor, monitor, auditor, or regulatory authority."*

| Obligation | Atlas mechanism | Inspectable evidence |
|---|---|---|
| Reconstructable course of events | DAG of `AtlasEvent`s; `parent_hashes` enforce ordering | Any trace bundle reconstructs the full lineage |
| Creation captured | `node.create` event; signed at write time | Standard write path |
| Modification captured | `node.update` event with `patch` payload, links to predecessor via `parent_hashes` | `AtlasPayload::NodeUpdate` variant |
| Deletion captured | Deletes are modelled as superseding events; original event_hash preserved | Append-only design — no destructive delete path exists in the schema |
| Available for sponsor / regulator review | `atlas.export_bundle` MCP tool emits a self-contained bundle | Demonstrated in Architecture §6 |
| Independent reviewer can verify | Verifier rebuildable under Apache-2.0 from public source | `atlas-verify-cli` |

### §3.16 — Validation of computerised systems

| Obligation | Atlas mechanism | Inspectable evidence |
|---|---|---|
| Documented validation evidence | 73 automated tests across the verifier and issuer crates: 41 verifier unit tests + 13 verifier integration adversary tests + 5 Sigstore-format golden round-trip tests in `atlas-trust-core`, plus 14 issuer-side tests in `atlas-signer` (mock-Rekor adversary, live-Sigstore wiremock round-trip, Atlas anchoring pubkey PEM pin) | `cargo test -p atlas-trust-core -p atlas-signer --release` |
| Change control | Crate version bump required when canonical format changes (enforced by byte-pinned goldens) and when any issuer / verifier log-pubkey pairing changes (enforced by issuer-side seed↔pubkey assertions) | `signing_input_byte_determinism_pin` + `bundle_hash_byte_determinism_pin` + `mock_log_pubkey_matches_signer_seed` + `atlas_anchor_pubkey_pem_is_pinned` + Sigstore key PEM pin + live entry round-trip `verifies_real_sigstore_rekor_entry` |
| Reproducibility on regulator's system | Pure-Rust verifier, deterministic bytes across platforms | Architecture §1, corollary 2 |

---

## 4. DORA — Digital Operational Resilience Act (Regulation (EU) 2022/2554)

### Articles 11 — 14 — Operational event logging and major incident reporting

> *Article 11: ICT-related incidents shall be classified [...] and
> documented. Article 12: Records shall be retained for at least five
> years and made available to the competent authorities on request.*

| Obligation | Atlas mechanism | Inspectable evidence |
|---|---|---|
| Incident-event recording | Same write path as any other event; `node.create` with `kind: "incident"` | Standard schema, no special handling needed |
| 5-year retention | JSONL persistence is filesystem-driven; retention policy is operational, not protocol-level | Operational concern — Atlas does not auto-expire |
| Tamper-evident records | Same hash + signature regime as all events | All adversary tests apply |
| Cross-system correlation | Workspace-id binding scopes events; multi-workspace export possible via separate bundles | `cross_workspace_replay_rejected` test |
| Available to competent authority | Bundle export + offline verifier | Architecture §6 |

### Article 14 — Reporting templates

DORA Article 14 prescribes report templates rather than internal
logging mechanisms. Atlas's contribution is providing the **underlying
verifiable record** that a DORA-compliant report can cite — the report
template itself is out-of-scope.

---

## 5. GDPR — Regulation (EU) 2016/679

### Article 22 — Automated decision-making

> *"The data subject shall have the right not to be subject to a
> decision based solely on automated processing […] which produces
> legal effects concerning him or her or similarly significantly
> affects him or her."*

The carve-outs in Article 22(2) require the controller to "implement
suitable measures to safeguard the data subject's rights and freedoms
and legitimate interests, at least the right to obtain human
intervention on the part of the controller, to express his or her
point of view and to contest the decision."

| Obligation | Atlas mechanism | Inspectable evidence |
|---|---|---|
| Demonstrate human intervention | `annotation.add` event signed by `kid = spiffe://atlas/human/<email>` proves a named human reviewed the decision | Bank demo event 3 |
| Show the decision was contestable | Annotation payload carries the human's `decision` and `evidence` fields | Bank demo `verifier`, `decision`, `evidence` fields |
| Reconstruct the decision logic for the data subject | Full DAG from input → model → human → inference is preserved and verifiable | Architecture §6 export flow |
| Prove the inference event was bound to a specific approved model | `parent_hashes` chain: inference → human-verify → model | DAG visualisation |

### Article 30 — Records of processing activities

| Obligation | Atlas mechanism | Inspectable evidence |
|---|---|---|
| Comprehensive records | Append-only event log, signed and tamper-evident | All write tests |
| Available to supervisory authority | Bundle export, offline verifier | Architecture §6 |

---

## 6. What Atlas does **not** claim

Honest negative space:

- **No legal opinion.** This document maps mechanisms to clauses. It is
  not legal advice. The customer's compliance officer is the
  load-bearing reviewer; Atlas provides them with executable evidence,
  not their conclusions.
- **No incident-classification engine.** DORA Article 11 requires
  classification logic; Atlas records incidents but does not classify
  them.
- **No retention enforcement.** GDPR Article 5(1)(e) data-minimisation
  / storage-limitation is operational. Atlas neither auto-expires nor
  prevents auto-expiry; the customer's retention policy runs on top.
- **No right-to-be-forgotten path in V1.** Append-only conflicts with
  GDPR Article 17 erasure rights for personal data inside payloads.
  V2 will support payload-redaction events that null the original
  payload while preserving the event_hash and the parent-link
  topology, so the lineage proof survives erasure.
- **No certified compliance with any framework above.** Atlas provides
  the structural evidence; certification is a separate audit
  engagement against the customer's deployment.

---

## 7. How an auditor uses this document

1. Pick a row.
2. Open the file referenced in "inspectable evidence".
3. Run the test or the verifier on a real trace bundle.
4. Confirm the mechanism behaves as the row claims.

If a row's claim does not match the file's behaviour, that is a bug in
Atlas, not a difference of opinion. Open a security advisory at
nelson@ultranova.io; we treat documentation drift the same as code
drift.
