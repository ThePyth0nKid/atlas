//! Top-level verifier entry point.
//!
//! `verify_trace(trace, bundle)` returns a `VerifyOutcome` that lists every check
//! that ran, what passed, what failed, and a final boolean.

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::anchor::{default_trusted_logs, verify_anchors};
use crate::cose::build_signing_input;
use crate::ed25519::verify_signature;
use crate::error::TrustResult;
use crate::hashchain::{check_event_hashes, check_parent_links, compute_tips};
use crate::pubkey_bundle::PubkeyBundle;
use crate::trace_format::AtlasTrace;

/// Caller-tunable verification options.
///
/// V1 had no options. V1.5 adds anchor strictness — the verifier still
/// passes traces with no anchors by default (so V1 traces continue to
/// verify), but auditors who insist on anchor coverage can enable strict
/// mode to require every DAG-tip to be anchored.
#[derive(Debug, Clone, Default)]
pub struct VerifyOptions {
    /// If true, every `trace.dag_tips` entry must have a matching
    /// successful anchor in `trace.anchors`. Empty `trace.anchors` then
    /// fails verification rather than passing as "no claim, no problem".
    pub require_anchors: bool,
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
            .map(|o| o.anchored_hash.as_str())
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

    let valid = errors.is_empty();

    VerifyOutcome {
        valid,
        evidence,
        errors,
        verifier_version: crate::VERIFIER_VERSION.to_string(),
    }
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
