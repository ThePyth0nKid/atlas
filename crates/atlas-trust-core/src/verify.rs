//! Top-level verifier entry point.
//!
//! `verify_trace(trace, bundle)` returns a `VerifyOutcome` that lists every check
//! that ran, what passed, what failed, and a final boolean.

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::anchor::{
    default_trusted_logs, verify_anchor_chain, verify_anchors, SIGSTORE_REKOR_V1_LOG_ID,
};
use crate::cose::build_signing_input;
use crate::ed25519::verify_signature;
use crate::error::TrustResult;
use crate::hashchain::{check_event_hashes, check_parent_links, compute_tips};
use crate::pubkey_bundle::PubkeyBundle;
use crate::trace_format::{AnchorEntry, AnchorKind, AtlasTrace};

/// Caller-tunable verification options.
///
/// V1 had no options. V1.5 adds anchor strictness — the verifier still
/// passes traces with no anchors by default (so V1 traces continue to
/// verify), but auditors who insist on anchor coverage can enable strict
/// mode to require every DAG-tip to be anchored. V1.7 adds the analogous
/// chain-strict flag: lenient by default (V1.5/V1.6 traces with no
/// `anchor_chain` continue to verify), strict mode requires the chain.
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

/// Convenience: parse trace from JSON and verify.
pub fn verify_trace_json(trace_bytes: &[u8], bundle: &PubkeyBundle) -> TrustResult<VerifyOutcome> {
    let trace: AtlasTrace = serde_json::from_slice(trace_bytes)?;
    Ok(verify_trace(&trace, bundle))
}
