//! Top-level verifier entry point.
//!
//! `verify_trace(trace, bundle)` returns a `VerifyOutcome` that lists every check
//! that ran, what passed, what failed, and a final boolean.

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::anchor::{
    chain_head_for, default_trusted_logs, verify_anchor_chain, verify_anchors,
    SIGSTORE_REKOR_V1_LOG_ID,
};
use crate::cose::build_signing_input;
use crate::ed25519::verify_signature;
use crate::error::TrustResult;
use crate::hashchain::{check_event_hashes, check_parent_links, compute_tips};
use crate::per_tenant::per_tenant_kid_for;
use crate::pubkey_bundle::PubkeyBundle;
use crate::trace_format::{AnchorChain, AnchorEntry, AnchorKind, AtlasTrace};
use crate::witness::{verify_witnesses_against_roster, ATLAS_WITNESS_V1_ROSTER};

/// Caller-tunable verification options.
///
/// V1 had no options. V1.5 adds anchor strictness — the verifier still
/// passes traces with no anchors by default (so V1 traces continue to
/// verify), but auditors who insist on anchor coverage can enable strict
/// mode to require every DAG-tip to be anchored. V1.7 adds the analogous
/// chain-strict flag: lenient by default (V1.5/V1.6 traces with no
/// `anchor_chain` continue to verify), strict mode requires the chain.
/// V1.9 adds per-tenant-keys strictness: legacy SPIFFE kids pass lenient,
/// strict mode requires every event to be signed by a workspace-derived
/// kid of shape `atlas-anchor:{workspace_id}`.
#[derive(Debug, Clone, Default)]
pub struct VerifyOptions {
    /// If true, every `trace.dag_tips` entry must have a matching
    /// successful anchor in `trace.anchors`. Empty `trace.anchors` then
    /// fails verification rather than passing as "no claim, no problem".
    pub require_anchors: bool,

    /// If true, `trace.anchor_chain` must be present, internally
    /// consistent, and (if `trace.anchors` is also present) must cover
    /// every entry in `trace.anchors`. Defaults to false so V1.5/V1.6
    /// bundles continue to verify under V1.7 builds.
    pub require_anchor_chain: bool,

    /// V1.9: if true, every `AtlasEvent.signature.kid` must equal
    /// `format!("atlas-anchor:{}", trace.workspace_id)` — i.e. each
    /// event must be signed by the workspace-specific HKDF-derived
    /// keypair. Legacy SPIFFE kids (`spiffe://atlas/agent/...`,
    /// `spiffe://atlas/human/...`, `spiffe://atlas/system/...`) fail
    /// this check.
    ///
    /// Defaults to false so V1.5–V1.8 bundles continue to verify under
    /// V1.9 builds. Auditors who require post-V1.9 single-tenant-key-
    /// blast-radius isolation enable this flag.
    ///
    /// Trust-trade-off note: lenient mode accepts BOTH legacy and
    /// per-tenant kids. An attacker who can downgrade a workspace's
    /// bundle from per-tenant back to legacy form bypasses per-tenant
    /// isolation. Strict mode is the real security boundary; document
    /// the gap when communicating about V1.9 to auditors.
    pub require_per_tenant_keys: bool,
}

/// Full verification result, suitable for showing in a UI / CLI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyOutcome {
    /// Did every check pass?
    pub valid: bool,
    /// Per-check evidence entries.
    pub evidence: Vec<VerifyEvidence>,
    /// Errors that triggered `valid=false`.
    pub errors: Vec<String>,
    /// Verifier build identity (e.g. "atlas-trust-core/0.1.0").
    pub verifier_version: String,
}

impl VerifyOutcome {
    /// True if any `witnesses` evidence row failed (`ok: false`).
    ///
    /// V1.13 wave C-1 surfaces witness verification failures as
    /// `ok: false` evidence rows but does NOT push them to `errors` —
    /// `valid` stays true (lenient mode). Wave C-2 will introduce
    /// `opts.require_witness_threshold`; the strict-mode logic must
    /// look at the witnesses evidence row, NOT at `errors`. This helper
    /// gives C-2 a named API to call instead of the fragile inline
    /// filter (`evidence.iter().any(|e| e.check == "witnesses" && !e.ok)`)
    /// that would otherwise be scattered at the strict-mode site —
    /// making it harder to wire the threshold up against the wrong
    /// source.
    pub fn has_witness_failures(&self) -> bool {
        self.evidence
            .iter()
            .any(|e| e.check == "witnesses" && !e.ok)
    }
}

/// A single piece of evidence (passed or failed) gathered during verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyEvidence {
    /// Short label (e.g. "schema-version", "pubkey-bundle-hash", "event-sig").
    pub check: String,
    /// Did this check pass?
    pub ok: bool,
    /// Free-form detail (one line).
    pub detail: String,
}

/// Verify a full trace bundle against a pinned pubkey bundle, using
/// default options (`require_anchors = false`). Equivalent to
/// `verify_trace_with(trace, bundle, &VerifyOptions::default())`.
///
/// This function never panics. All errors are captured into `outcome.errors`.
pub fn verify_trace(trace: &AtlasTrace, bundle: &PubkeyBundle) -> VerifyOutcome {
    verify_trace_with(trace, bundle, &VerifyOptions::default())
}

/// Verify a trace with caller-supplied options.
pub fn verify_trace_with(
    trace: &AtlasTrace,
    bundle: &PubkeyBundle,
    opts: &VerifyOptions,
) -> VerifyOutcome {
    let mut evidence = Vec::new();
    let mut errors = Vec::new();

    // 1. Schema version
    if trace.schema_version == crate::SCHEMA_VERSION {
        evidence.push(VerifyEvidence {
            check: "schema-version".to_string(),
            ok: true,
            detail: format!("trace schema {} matches verifier", trace.schema_version),
        });
    } else {
        let msg = format!(
            "schema mismatch: trace={} verifier={}",
            trace.schema_version,
            crate::SCHEMA_VERSION
        );
        errors.push(msg.clone());
        evidence.push(VerifyEvidence {
            check: "schema-version".to_string(),
            ok: false,
            detail: msg,
        });
    }

    // 2. Pubkey-bundle hash matches (constant-time compare).
    match bundle.deterministic_hash() {
        Ok(actual) => {
            if crate::ct::ct_eq_str(&actual, &trace.pubkey_bundle_hash) {
                evidence.push(VerifyEvidence {
                    check: "pubkey-bundle-hash".to_string(),
                    ok: true,
                    detail: format!("bundle hash {} matches", &actual[..16]),
                });
            } else {
                let msg = format!(
                    "pubkey bundle mismatch: trace claims {}, verifier built with {}",
                    &trace.pubkey_bundle_hash, &actual
                );
                errors.push(msg.clone());
                evidence.push(VerifyEvidence {
                    check: "pubkey-bundle-hash".to_string(),
                    ok: false,
                    detail: msg,
                });
            }
        }
        Err(e) => {
            let msg = format!("could not compute bundle hash: {e}");
            errors.push(msg.clone());
            evidence.push(VerifyEvidence {
                check: "pubkey-bundle-hash".to_string(),
                ok: false,
                detail: msg,
            });
        }
    }

    // 3. Recompute event hashes (workspace_id is bound into the signing-input
    //    so cross-workspace replay produces a hash mismatch here).
    match check_event_hashes(&trace.workspace_id, &trace.events) {
        Ok(()) => evidence.push(VerifyEvidence {
            check: "event-hashes".to_string(),
            ok: true,
            detail: format!("{} events, all hashes recomputed-match", trace.events.len()),
        }),
        Err(e) => {
            let msg = format!("hash mismatch: {e}");
            errors.push(msg.clone());
            evidence.push(VerifyEvidence {
                check: "event-hashes".to_string(),
                ok: false,
                detail: msg,
            });
        }
    }

    // 4. Verify each event's Ed25519 signature.
    //    Pre-flight checks: alg field MUST be EdDSA, ts MUST be RFC 3339.
    //    A trace-emitter pumping non-EdDSA values, or non-parseable timestamps,
    //    is either buggy or hostile — reject either.
    let mut sig_failures = 0usize;
    for ev in &trace.events {
        if ev.signature.alg != "EdDSA" {
            let msg = format!(
                "event {}: unsupported signature alg '{}' (V1 accepts only EdDSA)",
                ev.event_id, ev.signature.alg
            );
            errors.push(msg);
            sig_failures += 1;
            continue;
        }

        if let Err(e) = chrono::DateTime::parse_from_rfc3339(&ev.ts) {
            let msg = format!(
                "event {}: invalid RFC 3339 timestamp '{}': {}",
                ev.event_id, ev.ts, e
            );
            errors.push(msg);
            sig_failures += 1;
            continue;
        }

        let signing_input = match build_signing_input(
            &trace.workspace_id,
            &ev.event_id,
            &ev.ts,
            &ev.signature.kid,
            &ev.parent_hashes,
            &ev.payload,
        ) {
            Ok(b) => b,
            Err(e) => {
                let msg = format!("event {}: signing-input build failed: {}", ev.event_id, e);
                errors.push(msg);
                sig_failures += 1;
                continue;
            }
        };

        let pubkey = match bundle.pubkey_for(&ev.signature.kid) {
            Ok(pk) => pk,
            Err(e) => {
                let msg = format!("event {}: {}", ev.event_id, e);
                errors.push(msg);
                sig_failures += 1;
                continue;
            }
        };

        let sig_bytes = match decode_b64url(&ev.signature.sig) {
            Ok(b) => b,
            Err(e) => {
                let msg = format!("event {}: sig b64 decode failed: {}", ev.event_id, e);
                errors.push(msg);
                sig_failures += 1;
                continue;
            }
        };

        if let Err(e) = verify_signature(&pubkey, &signing_input, &sig_bytes, &ev.event_id) {
            errors.push(e.to_string());
            sig_failures += 1;
        }
    }

    if sig_failures == 0 {
        evidence.push(VerifyEvidence {
            check: "event-signatures".to_string(),
            ok: true,
            detail: format!("{} signatures verified", trace.events.len()),
        });
    } else {
        evidence.push(VerifyEvidence {
            check: "event-signatures".to_string(),
            ok: false,
            detail: format!("{} of {} signatures failed", sig_failures, trace.events.len()),
        });
    }

    // 5. Parent-link integrity
    match check_parent_links(&trace.events) {
        Ok(()) => evidence.push(VerifyEvidence {
            check: "parent-links".to_string(),
            ok: true,
            detail: "all parent_hashes resolved within trace".to_string(),
        }),
        Err(e) => {
            let msg = format!("dangling parent: {e}");
            errors.push(msg.clone());
            evidence.push(VerifyEvidence {
                check: "parent-links".to_string(),
                ok: false,
                detail: msg,
            });
        }
    }

    // 6. DAG tips match server claim
    let computed_tips = compute_tips(&trace.events);
    let mut server_claim = trace.dag_tips.clone();
    server_claim.sort();
    if computed_tips == server_claim {
        evidence.push(VerifyEvidence {
            check: "dag-tips".to_string(),
            ok: true,
            detail: format!("{} tips, match server claim", computed_tips.len()),
        });
    } else {
        let msg = format!(
            "dag-tip-mismatch: computed={:?}, server-claimed={:?}",
            computed_tips, server_claim
        );
        errors.push(msg.clone());
        evidence.push(VerifyEvidence {
            check: "dag-tips".to_string(),
            ok: false,
            detail: msg,
        });
    }

    // 6b. Per-tenant key kid coverage (V1.9, strict-mode only).
    //
    // Lenient mode (default): legacy SPIFFE kids and per-tenant kids
    // both pass — the upstream `event-signatures` check has already
    // verified each signature against whatever pubkey the bundle
    // provides for that kid. The trust property holds.
    //
    // Strict mode (`opts.require_per_tenant_keys = true`): every
    // event's `signature.kid` must equal the per-tenant kid for the
    // trace's `workspace_id`, i.e. `atlas-anchor:{workspace_id}`. Any
    // legacy kid fails. This is the V1.9 single-key-blast-radius
    // boundary — an auditor who insists on per-tenant isolation gets a
    // hard failure on V1.5–V1.8 bundles or any V1.9 trace that smuggled
    // a legacy kid through.
    //
    // Note we do NOT require all events to share the same per-tenant
    // kid — the per-event kid check is sufficient. Every event must
    // match the workspace's expected kid; mixing kids from different
    // workspaces in one trace is therefore impossible under strict
    // mode (the kids that don't match `trace.workspace_id` fail).
    if opts.require_per_tenant_keys {
        let expected_kid = per_tenant_kid_for(&trace.workspace_id);
        let mismatched: Vec<&str> = trace
            .events
            .iter()
            .filter_map(|ev| {
                if ev.signature.kid == expected_kid {
                    None
                } else {
                    Some(ev.event_id.as_str())
                }
            })
            .collect();
        if mismatched.is_empty() {
            evidence.push(VerifyEvidence {
                check: "per-tenant-keys".to_string(),
                ok: true,
                detail: format!(
                    "all {} event(s) signed by per-tenant kid '{}'",
                    trace.events.len(),
                    expected_kid,
                ),
            });
        } else {
            let msg = format!(
                "strict mode: {} event(s) not signed by per-tenant kid '{}': {:?}",
                mismatched.len(),
                expected_kid,
                mismatched,
            );
            errors.push(msg.clone());
            evidence.push(VerifyEvidence {
                check: "per-tenant-keys".to_string(),
                ok: false,
                detail: msg,
            });
        }
    }

    // 7. Anchor check (V1.5).
    // Each entry's Merkle inclusion proof is verified against a signed
    // log checkpoint, the checkpoint signature is verified against the
    // pinned log key roster, and `anchored_hash` must match what the
    // trace itself claims for that anchor's `kind`. Lenient by default
    // (empty anchors = honest no-claim, passes); strict mode
    // (`opts.require_anchors`) enforces that every dag_tip is covered.
    let trusted_logs = default_trusted_logs();
    let anchor_outcomes = verify_anchors(trace, &trusted_logs);
    let anchor_failures: Vec<&str> = anchor_outcomes
        .iter()
        .filter(|o| !o.ok)
        .map(|o| o.reason.as_str())
        .collect();

    if !anchor_failures.is_empty() {
        for f in &anchor_failures {
            errors.push(format!("anchor: {f}"));
        }
        evidence.push(VerifyEvidence {
            check: "anchors".to_string(),
            ok: false,
            detail: format!(
                "{} of {} anchor(s) failed verification",
                anchor_failures.len(),
                anchor_outcomes.len(),
            ),
        });
    } else if anchor_outcomes.is_empty() {
        if opts.require_anchors {
            let msg =
                "strict mode: trace claims no anchors, but require_anchors is set".to_string();
            errors.push(msg.clone());
            evidence.push(VerifyEvidence {
                check: "anchors".to_string(),
                ok: false,
                detail: msg,
            });
        } else {
            evidence.push(VerifyEvidence {
                check: "anchors".to_string(),
                ok: true,
                detail: "no anchors claimed (lenient mode passes)".to_string(),
            });
        }
    } else {
        evidence.push(VerifyEvidence {
            check: "anchors".to_string(),
            ok: true,
            detail: format!("{} anchor(s) verified against pinned log keys", anchor_outcomes.len()),
        });
    }

    if opts.require_anchors && !trace.dag_tips.is_empty() {
        use std::collections::BTreeSet;
        let anchored_tips: BTreeSet<&str> = anchor_outcomes
            .iter()
            .filter(|o| o.ok && matches!(o.kind, crate::trace_format::AnchorKind::DagTip))
            .map(|o| o.trace_hash.as_str())
            .collect();
        let missing: Vec<&str> = trace
            .dag_tips
            .iter()
            .map(String::as_str)
            .filter(|t| !anchored_tips.contains(*t))
            .collect();
        if !missing.is_empty() {
            let msg = format!(
                "strict mode: {} dag_tip(s) not anchored: {:?}",
                missing.len(),
                missing,
            );
            errors.push(msg.clone());
            evidence.push(VerifyEvidence {
                check: "anchors-coverage".to_string(),
                ok: false,
                detail: msg,
            });
        } else {
            evidence.push(VerifyEvidence {
                check: "anchors-coverage".to_string(),
                ok: true,
                detail: format!("{} dag_tip(s) all anchored (strict mode)", trace.dag_tips.len()),
            });
        }
    }

    // 8. Anchor-chain consistency (V1.7).
    //
    // The chain commits, transitively via `previous_head`, to every
    // batch ever issued for the workspace. Verification is two-pronged:
    //   - Chain-internal: every batch's recomputed head must thread
    //     through `previous_head` of the next batch, the index sequence
    //     must be 0..N, and the tip must equal the convenience `head`.
    //   - Coverage: every entry surfaced in `trace.anchors` must also
    //     be present in some batch in `chain.history`. A trace claiming
    //     anchors that were never recorded in the chain is a fork.
    //
    // Lenient by default: traces without `anchor_chain` continue to
    // verify (V1.5/V1.6 compatibility). Strict mode (`require_anchor_chain`)
    // demands the chain be present.
    match &trace.anchor_chain {
        None => {
            if opts.require_anchor_chain {
                let msg = "strict mode: trace has no anchor_chain, but require_anchor_chain is set"
                    .to_string();
                errors.push(msg.clone());
                evidence.push(VerifyEvidence {
                    check: "anchor-chain".to_string(),
                    ok: false,
                    detail: msg,
                });
            } else {
                evidence.push(VerifyEvidence {
                    check: "anchor-chain".to_string(),
                    ok: true,
                    detail: "no anchor_chain claimed (lenient mode passes)".to_string(),
                });
            }
        }
        Some(chain) => {
            let outcome = verify_anchor_chain(chain);
            if !outcome.ok {
                for e in &outcome.errors {
                    errors.push(format!("anchor-chain: {e}"));
                }
                evidence.push(VerifyEvidence {
                    check: "anchor-chain".to_string(),
                    ok: false,
                    detail: format!(
                        "{} chain error(s) over {} batch(es)",
                        outcome.errors.len(),
                        outcome.batches_walked,
                    ),
                });
            } else {
                let tip_short = outcome
                    .recomputed_head
                    .as_deref()
                    .map(|h| &h[..16])
                    .unwrap_or("<empty>");
                evidence.push(VerifyEvidence {
                    check: "anchor-chain".to_string(),
                    ok: true,
                    detail: format!(
                        "chain head {} verified across {} batch(es)",
                        tip_short, outcome.batches_walked,
                    ),
                });
            }

            // Coverage: every trace.anchors entry must appear in some
            // chain batch, with byte-identical content (to prevent
            // proof-swap attacks where two entries share trace
            // coordinates but carry different inclusion proofs).
            //
            // Three classes of anchor are evaluated:
            //
            //   1. Covered      — entry appears byte-identically in some
            //                     chain batch. Always passes.
            //   2. Sigstore-deferred — entry's `log_id` matches the
            //                     Sigstore Rekor v1 production log and
            //                     it is absent from the chain. Accepted:
            //                     a V1.7 bundle could not extend the
            //                     chain on the Sigstore path (issuer
            //                     gate, removed in V1.8) but Sigstore's
            //                     own publicly-witnessed transparency
            //                     log gives the same monotonicity
            //                     guarantee for that entry. Per-entry
            //                     verification (step `anchors`) still
            //                     reconstructs the C2SP origin, checks
            //                     the inclusion proof, and validates
            //                     the checkpoint signature against the
            //                     pinned Atlas anchoring key — so the
            //                     entry is fully trust-anchored even
            //                     without a chain presence.
            //   3. Uncovered    — neither in chain nor a known Sigstore
            //                     entry. Rejected: mock anchors must be
            //                     in the chain (the chain is the only
            //                     monotonicity witness for the dev
            //                     mock-Rekor), and an unknown `log_id`
            //                     would already have been rejected by
            //                     `verify_anchors` upstream — so this
            //                     branch fires for mock entries that
            //                     escaped chain extension (a true
            //                     coverage gap).
            //
            // Gated on `outcome.ok`: if the chain walk failed, the
            // chain_keys set is built from a partially-walked or
            // structurally broken history, and a coverage "pass" against
            // that set would be meaningless (and misleading evidence).
            if outcome.ok && !trace.anchors.is_empty() {
                use std::collections::BTreeSet;
                let chain_keys: BTreeSet<AnchorEntryKey<'_>> = chain
                    .history
                    .iter()
                    .flat_map(|b| b.entries.iter())
                    .map(anchor_entry_key)
                    .collect();
                let sigstore_log_id: &str = SIGSTORE_REKOR_V1_LOG_ID.as_str();
                let mut missing_required: Vec<&AnchorEntry> = Vec::new();
                let mut deferred_sigstore: Vec<&AnchorEntry> = Vec::new();
                for entry in trace.anchors.iter() {
                    if chain_keys.contains(&anchor_entry_key(entry)) {
                        continue;
                    }
                    if entry.log_id == sigstore_log_id {
                        deferred_sigstore.push(entry);
                    } else {
                        missing_required.push(entry);
                    }
                }
                if !missing_required.is_empty() {
                    let msg = format!(
                        "anchor-chain: {} trace.anchors entry/entries not present in any chain batch",
                        missing_required.len(),
                    );
                    errors.push(msg.clone());
                    evidence.push(VerifyEvidence {
                        check: "anchor-chain-coverage".to_string(),
                        ok: false,
                        detail: msg,
                    });
                } else {
                    let total = trace.anchors.len();
                    let in_chain = total - deferred_sigstore.len();
                    let detail = if deferred_sigstore.is_empty() {
                        format!(
                            "all {} trace.anchors entry/entries covered by chain history",
                            total,
                        )
                    } else {
                        format!(
                            "{} of {} trace.anchors covered by chain history; {} Sigstore entry/entries deferred (V1.7-issued, accepted via Rekor v1 monotonicity)",
                            in_chain,
                            total,
                            deferred_sigstore.len(),
                        )
                    };
                    evidence.push(VerifyEvidence {
                        check: "anchor-chain-coverage".to_string(),
                        ok: true,
                        detail,
                    });
                }
            }

            // 8c. Witness cosignatures (V1.13 Scope C, lenient default).
            evidence.push(witness_evidence_for_chain(chain));
        }
    }

    let valid = errors.is_empty();

    VerifyOutcome {
        valid,
        evidence,
        errors,
        verifier_version: crate::VERIFIER_VERSION.to_string(),
    }
}

/// Tuple-key identifying an `AnchorEntry` for cross-set comparison
/// between `trace.anchors` and chain entries.
///
/// Includes the `inclusion_proof` root-hash and tree-size so that two
/// entries sharing trace coordinates but carrying different proofs do
/// NOT collide in the coverage set. If the key were just (kind, hash,
/// log_id, log_index), an attacker could place a proof-A entry into
/// the chain (where step-7 inclusion verification doesn't run) and a
/// proof-B entry into trace.anchors (where step-7 does run) — coverage
/// would pass on coordinates while the two proofs disagree about the
/// witnessed log state. Including the proof root-hash + tree-size makes
/// the cross-check truly independent from step 7.
type AnchorEntryKey<'a> = (&'a str, &'a str, &'a str, u64, &'a str, u64);

fn anchor_entry_key(e: &AnchorEntry) -> AnchorEntryKey<'_> {
    let kind = match e.kind {
        AnchorKind::DagTip => "dag_tip",
        AnchorKind::BundleHash => "bundle_hash",
    };
    (
        kind,
        e.anchored_hash.as_str(),
        e.log_id.as_str(),
        e.log_index,
        e.inclusion_proof.root_hash.as_str(),
        e.inclusion_proof.tree_size,
    )
}

fn decode_b64url(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    URL_SAFE_NO_PAD.decode(s)
}

/// Walk an `AnchorChain` and produce a `witnesses` evidence row
/// summarising how many witness sigs were presented across all batches,
/// how many verified against `ATLAS_WITNESS_V1_ROSTER`, and what (if
/// any) failed.
///
/// Lenient mode (current — V1.13 wave C-1): failed witnesses are
/// surfaced as `ok: false` evidence carrying a per-failure breakdown,
/// but the caller does NOT push the failures into the top-level
/// `errors` collection — the trace stays valid. Rationale: V1.13 ships
/// with an empty roster (commissioning ceremony lands in wave C-2), so
/// a strict default would universally pass with "0 of 0 verified" —
/// uninformative — while flipping to a true threshold without operator
/// opt-in would surprise existing deployments. Wave C-2 will add an
/// `opts.require_witness_threshold` flag that promotes the failures to
/// `errors` when set.
///
/// We do NOT gate this on the chain-internal walk's success: a witness
/// sig commits to one SPECIFIC batch's recomputed head, which is
/// well-defined even if adjacent batches in the chain don't link
/// cleanly. The chain-link check and the witness check are independent
/// trust properties — surfacing both is more useful than collapsing
/// them.
fn witness_evidence_for_chain(chain: &AnchorChain) -> VerifyEvidence {
    let mut presented = 0usize;
    let mut verified = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for batch in &chain.history {
        if batch.witnesses.is_empty() {
            continue;
        }
        let head = match chain_head_for(batch) {
            Ok(h) => h,
            Err(e) => {
                // chain_head_for failed for THIS batch; we can't
                // verify its witnesses. Record as a witness-side
                // diagnostic — counted as "presented but failed".
                presented += batch.witnesses.len();
                for w in &batch.witnesses {
                    failures.push(format!(
                        "batch[{}] witness {}: chain_head recompute failed: {}",
                        batch.batch_index, w.witness_kid, e,
                    ));
                }
                continue;
            }
        };
        let batch_outcome = verify_witnesses_against_roster(
            &batch.witnesses,
            &head,
            ATLAS_WITNESS_V1_ROSTER,
        );
        presented += batch_outcome.presented;
        verified += batch_outcome.verified;
        for f in batch_outcome.failures {
            failures.push(format!("batch[{}] {}", batch.batch_index, f));
        }
    }

    if presented == 0 {
        VerifyEvidence {
            check: "witnesses".to_string(),
            ok: true,
            detail: "no witnesses presented (lenient mode passes)".to_string(),
        }
    } else if failures.is_empty() {
        VerifyEvidence {
            check: "witnesses".to_string(),
            ok: true,
            detail: format!("{} witness sig(s) verified across chain history", verified),
        }
    } else {
        VerifyEvidence {
            check: "witnesses".to_string(),
            ok: false,
            detail: format!(
                "lenient: {} of {} witness sig(s) verified; {} failure(s) recorded but non-blocking (strict --require-witness deferred to V1.13 wave C-2): {}",
                verified,
                presented,
                failures.len(),
                failures.join("; "),
            ),
        }
    }
}

/// Convenience: parse trace from JSON and verify.
pub fn verify_trace_json(trace_bytes: &[u8], bundle: &PubkeyBundle) -> TrustResult<VerifyOutcome> {
    let trace: AtlasTrace = serde_json::from_slice(trace_bytes)?;
    Ok(verify_trace(&trace, bundle))
}

#[cfg(test)]
mod tests {
    //! Focused unit tests for the witness-evidence rollup.
    //!
    //! Full end-to-end tests of `verify_trace_with` live in the
    //! integration-test files under `tests/`; these tests target the
    //! V1.13 wave C-1 lenient-mode rollup helper directly so the
    //! contract (presented/verified counts, failure surfacing, lenient
    //! disposition) is pinned without standing up a full trace fixture.
    use super::*;
    use crate::trace_format::{
        AnchorBatch, AnchorChain, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
    };
    use crate::witness::WitnessSig;

    fn empty_chain() -> AnchorChain {
        AnchorChain {
            history: Vec::new(),
            head: ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD.to_string(),
        }
    }

    fn batch_no_witnesses(idx: u64, prev: &str) -> AnchorBatch {
        AnchorBatch {
            batch_index: idx,
            integrated_time: 1_745_000_000,
            entries: Vec::new(),
            previous_head: prev.to_string(),
            witnesses: Vec::new(),
        }
    }

    /// Empty chain history → "no witnesses presented" with ok=true.
    /// Lenient mode's no-op disposition.
    #[test]
    fn witnesses_evidence_empty_history_passes_lenient() {
        let chain = empty_chain();
        let ev = witness_evidence_for_chain(&chain);
        assert_eq!(ev.check, "witnesses");
        assert!(ev.ok, "empty history must pass lenient: {ev:?}");
        assert!(
            ev.detail.contains("no witnesses presented"),
            "detail should name the no-op disposition: {}",
            ev.detail,
        );
    }

    /// Chain with batches but zero witnesses on each → identical
    /// "no witnesses presented" outcome (the ZERO presentation is what
    /// triggers the no-op branch, not the empty history).
    #[test]
    fn witnesses_evidence_batches_without_witnesses_pass_lenient() {
        let mut chain = empty_chain();
        chain.history.push(batch_no_witnesses(
            0,
            ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
        ));
        let head_zero = chain_head_for(&chain.history[0]).unwrap();
        chain.history.push(batch_no_witnesses(1, &head_zero));
        chain.head = chain_head_for(&chain.history[1]).unwrap();

        let ev = witness_evidence_for_chain(&chain);
        assert!(ev.ok);
        assert!(
            ev.detail.contains("no witnesses presented"),
            "non-empty history with no witnesses must still be no-op: {}",
            ev.detail,
        );
    }

    /// Chain with witnesses whose kids aren't in the (empty) genesis
    /// roster → ok=false, lenient detail naming the count + each failure.
    /// This is the most operationally common shape during V1.13 wave C-1
    /// rollout: an issuer started attaching witnesses BEFORE the
    /// commissioning ceremony added the corresponding pubkey to
    /// ATLAS_WITNESS_V1_ROSTER.
    #[test]
    fn witnesses_evidence_unknown_kid_lenient_records_failure() {
        let mut chain = empty_chain();
        let mut batch = batch_no_witnesses(0, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        batch.witnesses.push(WitnessSig {
            witness_kid: "uncommissioned-witness".to_string(),
            // 64-byte sig under URL_SAFE_NO_PAD = 86 chars.
            signature: "A".repeat(86),
        });
        chain.history.push(batch);
        chain.head = chain_head_for(&chain.history[0]).unwrap();

        let ev = witness_evidence_for_chain(&chain);
        assert!(!ev.ok, "unknown-kid witness must surface as ok=false: {ev:?}");
        assert!(ev.detail.contains("0 of 1"), "detail must name presented/verified counts: {}", ev.detail);
        assert!(
            ev.detail.contains("non-blocking"),
            "detail must name the lenient disposition: {}",
            ev.detail,
        );
        assert!(
            ev.detail.contains("uncommissioned-witness"),
            "detail must include the kid that failed for auditor diagnostics: {}",
            ev.detail,
        );
        assert!(
            ev.detail.contains("not in pinned roster"),
            "detail must surface the underlying failure reason: {}",
            ev.detail,
        );
    }

    /// `has_witness_failures` returns false when there is no
    /// witnesses-keyed evidence row at all (e.g. a trace without
    /// `anchor_chain`, where the helper is never invoked).
    #[test]
    fn has_witness_failures_false_when_no_witness_evidence() {
        let outcome = VerifyOutcome {
            valid: true,
            evidence: vec![],
            errors: vec![],
            verifier_version: "test".to_string(),
        };
        assert!(!outcome.has_witness_failures());
    }

    /// `has_witness_failures` returns false when the witnesses row
    /// exists and passed (the lenient no-op disposition or all-verified).
    #[test]
    fn has_witness_failures_false_for_passing_witness_evidence() {
        let outcome = VerifyOutcome {
            valid: true,
            evidence: vec![VerifyEvidence {
                check: "witnesses".to_string(),
                ok: true,
                detail: "no witnesses presented (lenient mode passes)".to_string(),
            }],
            errors: vec![],
            verifier_version: "test".to_string(),
        };
        assert!(!outcome.has_witness_failures());
    }

    /// `has_witness_failures` returns true when a witnesses row carries
    /// `ok: false` — this is the signal Wave C-2 strict-mode will key
    /// on to promote witness failures into `errors`.
    #[test]
    fn has_witness_failures_true_for_failing_witness_evidence() {
        let outcome = VerifyOutcome {
            valid: true,
            evidence: vec![VerifyEvidence {
                check: "witnesses".to_string(),
                ok: false,
                detail: "lenient: 0 of 1 witness sig(s) verified; ...".to_string(),
            }],
            errors: vec![],
            verifier_version: "test".to_string(),
        };
        assert!(outcome.has_witness_failures());
    }

    /// Other failed checks (e.g. event-signatures, anchor-chain) must
    /// NOT trigger `has_witness_failures` — the helper is
    /// witness-specific by design (a strict-mode threshold check on
    /// witnesses must not be tripped by an unrelated failure).
    #[test]
    fn has_witness_failures_ignores_non_witness_failures() {
        let outcome = VerifyOutcome {
            valid: false,
            evidence: vec![
                VerifyEvidence {
                    check: "event-signatures".to_string(),
                    ok: false,
                    detail: "1 of 5 sigs failed".to_string(),
                },
                VerifyEvidence {
                    check: "anchor-chain".to_string(),
                    ok: false,
                    detail: "1 chain error".to_string(),
                },
            ],
            errors: vec!["sig fail".to_string(), "chain fail".to_string()],
            verifier_version: "test".to_string(),
        };
        assert!(
            !outcome.has_witness_failures(),
            "non-witness failures must not trip the witness-specific helper",
        );
    }

    /// Mixed chain: batch[0] has no witnesses, batch[1] has one
    /// known-bad. Walker must aggregate across batches and emit
    /// `batch[1] ...` prefix on the failure. Defends against an
    /// off-by-one where only the first batch's witnesses get checked.
    #[test]
    fn witnesses_evidence_aggregates_across_batches() {
        let mut chain = empty_chain();
        chain.history.push(batch_no_witnesses(
            0,
            ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
        ));
        let head_zero = chain_head_for(&chain.history[0]).unwrap();

        let mut batch_one = batch_no_witnesses(1, &head_zero);
        batch_one.witnesses.push(WitnessSig {
            witness_kid: "k-on-batch-1".to_string(),
            signature: "B".repeat(86),
        });
        chain.history.push(batch_one);
        chain.head = chain_head_for(&chain.history[1]).unwrap();

        let ev = witness_evidence_for_chain(&chain);
        assert!(!ev.ok);
        assert!(
            ev.detail.contains("batch[1]"),
            "detail must label which batch the failure belongs to: {}",
            ev.detail,
        );
    }
}
