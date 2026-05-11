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
use crate::hashchain::{check_event_hashes, check_parent_links, check_strict_chain, compute_tips};
use crate::per_tenant::per_tenant_kid_for;
use crate::pubkey_bundle::PubkeyBundle;
use crate::trace_format::{AnchorChain, AnchorEntry, AnchorKind, AtlasTrace};
use crate::error::TrustError;
use crate::witness::{
    sanitize_kid_for_diagnostic, verify_witnesses_against_roster, WitnessFailure,
    WitnessFailureReason, WitnessFailureWire, WitnessVerifyOutcome, ATLAS_WITNESS_V1_ROSTER,
};

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
/// kid of shape `atlas-anchor:{workspace_id}`. V1.13 wave C-2 adds witness-
/// threshold strictness. V1.19 Welle 9 adds strict-linear-chain: lenient
/// accepts forked DAGs; strict requires single-genesis, single-parent,
/// no-fork shape (the operator-facing single-writer-invariant boundary).
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

    /// V1.13 wave C-2: minimum number of distinct witness signatures
    /// (i.e., kid-distinct, sig-valid `WitnessSig` entries verified
    /// against `ATLAS_WITNESS_V1_ROSTER`) that must accompany the
    /// trace's `anchor_chain` for the trace to verify.
    ///
    /// `0` (default) = lenient = wave-C-1 behaviour preserved: failed
    /// witnesses surface in the `witnesses` evidence row but do NOT
    /// push to `errors`, and a chain-less trace continues to verify.
    ///
    /// `>= 1` = strict = the count of verified witnesses across the
    /// chain must be `>=` this value. A trace with no `anchor_chain`,
    /// or a chain with fewer kid-distinct verified cosignatures than
    /// the threshold, fails verification. The check is independent of
    /// chain presence — strict mode requires real witness coverage,
    /// not a side-effect of chain presence.
    ///
    /// Trust-trade-off note: same shape as `require_per_tenant_keys`.
    /// Lenient mode accepts traces without witness coverage; strict
    /// mode is the real security boundary for the V1.13 second-trust-
    /// domain property. Auditors who want the witness invariant must
    /// opt in via `--require-witness <N>` on the verifier CLI.
    ///
    /// Duplicate-`witness_kid` defence (wave C-1): a duplicated kid
    /// counts every occurrence as a failure (none is verified), so
    /// repeating one valid signature N times under a single
    /// commissioned key cannot satisfy a threshold of N.
    pub require_witness_threshold: usize,

    /// V1.19 Welle 9: if true, the trace's events MUST form a strict
    /// linear chain: exactly one genesis event, every non-genesis
    /// event has exactly one parent, and no event is referenced as a
    /// parent by more than one other event (no sibling-fork DAG, no
    /// DAG-merge).
    ///
    /// Default false — Atlas is fundamentally a DAG and forks are
    /// valid wire shape (the canonical example: a single workspace
    /// served by both `atlas-web` and `atlas-mcp-server`, where the
    /// per-workspace mutex in `@atlas/bridge` only serialises within
    /// one Node process — multi-process writers can fork the DAG
    /// while still producing a structurally valid trace). Strict mode
    /// is for operators who deploy exactly one writer per workspace
    /// and want to detect operational misconfiguration where a second
    /// writer accidentally appears.
    ///
    /// Trust-trade-off note: same lenient/strict shape as
    /// `require_per_tenant_keys`, `require_anchors`,
    /// `require_anchor_chain`, and `require_witness_threshold`.
    /// Lenient mode accepts forked DAGs as honest concurrent-writer
    /// state; strict mode is the real boundary for the
    /// "single-writer invariant". Auditors who need that property
    /// must opt in via `--require-strict-chain` on the verifier CLI.
    /// Mirrors the `parents[0] === stored[i-1].event_hash` oracle in
    /// V1.19 Welle 8 [C.6] but at the verifier surface instead of
    /// the write site.
    pub require_strict_chain: bool,
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
    /// Verifier build identity (e.g. "atlas-trust-core/1.0.0").
    pub verifier_version: String,
    /// V1.14 Scope J: structured per-witness failure records suitable
    /// for programmatic auditor consumption (kid + batch_index +
    /// stable `reason_code` + human-readable message). Mirrors the
    /// `Vec<WitnessFailure>` carried internally by
    /// `aggregate_witnesses_across_chain`'s rollup; the `witnesses`
    /// evidence row's `detail` string remains the human-readable
    /// rendering. Auditor tooling MUST switch on `reason_code` rather
    /// than parsing the evidence-row text — the wire-stable
    /// categorisation lives here.
    ///
    /// `#[serde(default)]` so JSON written by V1.13 builds (which had
    /// no such field) deserialises into an empty Vec rather than
    /// failing — additive-only wire change. New JSON consumers that
    /// require coverage must check the field's content, not its
    /// presence.
    #[serde(default)]
    pub witness_failures: Vec<WitnessFailureWire>,
}

impl VerifyOutcome {
    /// True if the lenient `witnesses` evidence row reported failures
    /// (`ok: false`). Independent of the strict-mode `--require-witness`
    /// threshold check, which lives in its own
    /// `witnesses-threshold` evidence row and is reflected in
    /// `outcome.errors` directly.
    ///
    /// V1.13 wave C-1's lenient disposition surfaces witness
    /// verification failures as `ok: false` evidence rows but does NOT
    /// push them to `errors` — `valid` stays true (so an issuer that
    /// attaches witnesses BEFORE the corresponding pubkey is
    /// commissioned does not break verification). This helper gives
    /// auditor tooling and CLI summaries a typed accessor for "did the
    /// lenient witness row report any failure?" without scanning
    /// evidence by string-match. Strict-mode callers do NOT need this
    /// helper: the threshold check pushes its own `witnesses-threshold`
    /// failure into `errors`, and `outcome.valid == false` is the
    /// canonical strict-mode signal.
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
    //
    // Welle 9 review-fix (CR-HIGH-1 / SR-M-3): track success in a bool
    // so the optional strict-chain check at section 5a can refuse to
    // produce a misleading `ok: true` evidence row over events whose
    // hashes are tampered or duplicated.
    let event_hashes_ok = match check_event_hashes(&trace.workspace_id, &trace.events) {
        Ok(()) => {
            evidence.push(VerifyEvidence {
                check: "event-hashes".to_string(),
                ok: true,
                detail: format!("{} events, all hashes recomputed-match", trace.events.len()),
            });
            true
        }
        Err(e) => {
            let msg = format!("hash mismatch: {e}");
            errors.push(msg.clone());
            evidence.push(VerifyEvidence {
                check: "event-hashes".to_string(),
                ok: false,
                detail: msg,
            });
            false
        }
    };

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
    //
    // Welle 9 review-fix (CR-HIGH-1 / SR-M-3): track success in a bool
    // so the optional strict-chain check at section 5a can gate on it.
    // `check_strict_chain`'s formal proof of "linear chain" depends on
    // every parent reference resolving — if parent-links failed, the
    // strict-chain check would be evaluating chain-shape over a graph
    // with dangling nodes and could produce a misleading `ok: true`.
    let parent_links_ok = match check_parent_links(&trace.events) {
        Ok(()) => {
            evidence.push(VerifyEvidence {
                check: "parent-links".to_string(),
                ok: true,
                detail: "all parent_hashes resolved within trace".to_string(),
            });
            true
        }
        Err(e) => {
            let msg = format!("dangling parent: {e}");
            errors.push(msg.clone());
            evidence.push(VerifyEvidence {
                check: "parent-links".to_string(),
                ok: false,
                detail: msg,
            });
            false
        }
    };

    // 5a. Strict-chain shape (V1.19 Welle 9, strict-mode only).
    //
    // Lenient mode (default): forks, DAG-merges, multiple genesis
    // events, and multiple tips are all accepted. This is the right
    // default — Atlas is a DAG, and the route.rs threat model
    // explicitly permits multi-process writers (which fork the DAG)
    // until V2 ships an external lock.
    //
    // Strict mode (`opts.require_strict_chain = true`): the events
    // MUST form a strict linear chain. Exactly one genesis, every
    // non-genesis event has exactly one parent, no event referenced
    // as a parent by more than one other event. This is the operator-
    // facing security boundary for the single-writer invariant: any
    // deployment where a second writer accidentally appears (e.g.
    // `atlas-web` AND `atlas-mcp-server` against the same workspace
    // simultaneously) produces a sibling-fork DAG that lenient mode
    // accepts but strict mode rejects with a structured error.
    //
    // Welle 9 review-fix (CR-HIGH-1 / SR-M-3): gate on the upstream
    // event-hashes AND parent-links checks both passing. Without that
    // gate, a tampered or dangling-parent trace could fail those
    // checks (correctly setting `valid=false`) while ALSO producing a
    // green `strict-chain: ok` evidence row over the structurally
    // broken graph — a misleading audit trail. When preflight failed,
    // emit an explicit "skipped" evidence row so an auditor reading
    // the JSON sees the gap rather than absence.
    //
    // Mirrors the `parents[0] === stored[i-1].event_hash` oracle in
    // V1.19 Welle 8 [C.6] (`apps/atlas-web/scripts/e2e-write-edge-cases.ts`)
    // but lifts the property into the trust pipeline so auditors can
    // enforce it across the full verification surface, not just at
    // the write site. Positioned at 5a (immediately after parent-links)
    // because both checks are structural shape properties of the event
    // graph; adjacency in the evidence list reflects logical adjacency.
    if opts.require_strict_chain {
        if !event_hashes_ok || !parent_links_ok {
            evidence.push(VerifyEvidence {
                check: "strict-chain".to_string(),
                ok: false,
                detail: "skipped: preflight (event-hashes / parent-links) failed; \
                    strict-chain shape cannot be soundly evaluated over a structurally \
                    broken graph"
                    .to_string(),
            });
        } else {
            match check_strict_chain(&trace.events) {
                Ok(()) => evidence.push(VerifyEvidence {
                    check: "strict-chain".to_string(),
                    ok: true,
                    detail: format!(
                        "{} event(s) form a strict linear chain (1 genesis, 1 parent per non-genesis event, no sibling-fork, no self-reference)",
                        trace.events.len(),
                    ),
                }),
                Err(e) => {
                    let msg = e.to_string();
                    errors.push(msg.clone());
                    evidence.push(VerifyEvidence {
                        check: "strict-chain".to_string(),
                        ok: false,
                        detail: msg,
                    });
                }
            }
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
        // V1.15 Welle A invariant: every KID-equality in production code
        // routes through `crate::ct::ct_eq_str`. `ev.signature.kid` is a
        // wire-side, attacker-influenceable string; `expected_kid` is
        // derived from `trace.workspace_id` (also wire-side) but neither
        // string is secret. The leak window is theoretical — both inputs
        // are present in the trace itself — but Atlas's stated property
        // is "byte-identical verification regardless of input shape" and
        // the cost of `ct_eq_str` is nil. Pin: a future caller using `==`
        // on a kid field in this file would re-introduce the timing path
        // and trip `tests/const_time_kid_invariant.rs`. See SECURITY-NOTES
        // `## scope-a` for the enumerated const-time boundaries.
        let mismatched: Vec<&str> = trace
            .events
            .iter()
            .filter_map(|ev| {
                if crate::ct::ct_eq_str(&ev.signature.kid, &expected_kid) {
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
    let witness_aggregate = match &trace.anchor_chain {
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
            // No chain ⇒ no witnesses possible. Empty aggregate so the
            // strict-mode threshold check below can run uniformly
            // regardless of chain presence (otherwise an off-by-one
            // would let chain-less traces silently pass strict mode).
            WitnessVerifyOutcome {
                presented: 0,
                verified: 0,
                failures: Vec::new(),
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
                    .as_ref()
                    .map(|h| &h.as_str()[..16])
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
            //
            // Compute the aggregate ONCE — the same value drives both
            // the lenient evidence row here and the wave-C-2 strict
            // threshold check after this match. Walking the chain twice
            // would not only waste CPU but risk the two consumers
            // disagreeing if a future refactor only updated one path.
            let aggregate = aggregate_witnesses_across_chain(chain);
            evidence.push(witness_evidence_from_aggregate(&aggregate));
            aggregate
        }
    };

    // 8d. Witness threshold (V1.13 wave C-2 strict mode).
    //
    // Promotes wave-C-1's lenient witness-evidence row into a hard
    // failure when `opts.require_witness_threshold > 0` and the
    // chain-aggregated `verified` count is below the threshold.
    //
    // Independent of chain presence: a chain-less trace under strict
    // mode (threshold >= 1) MUST fail because it cannot possibly carry
    // witness coverage. The empty aggregate populated in the chain-None
    // branch above guarantees this without a special-case.
    //
    // Wave-C-1's `WitnessOutcome.verified` already counts only
    // kid-distinct, sig-valid cosignatures (the duplicate-`witness_kid`
    // pre-pass rejects every occurrence of a repeated kid as a
    // failure), so an issuer cannot satisfy threshold N by attaching
    // one valid signature N times under one commissioned key.
    apply_witness_threshold(
        witness_aggregate.verified,
        opts.require_witness_threshold,
        &mut evidence,
        &mut errors,
    );

    let valid = errors.is_empty();

    // V1.14 Scope J: project the chain-aggregator's structured
    // `Vec<WitnessFailure>` (in-process type, carries `TrustError`)
    // to wire-stable `Vec<WitnessFailureWire>` (no `TrustError`,
    // stable `reason_code` enum, kebab-case JSON). Same source of
    // truth as the lenient `witnesses` evidence row — both render
    // from `witness_aggregate.failures` so they cannot disagree.
    let witness_failures = witness_aggregate
        .failures
        .iter()
        .map(WitnessFailureWire::from)
        .collect();

    VerifyOutcome {
        valid,
        evidence,
        errors,
        verifier_version: crate::VERIFIER_VERSION.to_string(),
        witness_failures,
    }
}

/// Apply the V1.13 wave-C-2 strict-mode witness threshold check.
///
/// Threshold == 0 is the lenient sentinel (wave-C-1 default) — emits
/// no evidence row and no error so the call site stays uniform across
/// lenient/strict modes. Threshold >= 1 emits a `witnesses-threshold`
/// evidence row in either disposition (pass: `ok=true`; fail:
/// `ok=false` + error pushed onto the top-level errors list, which
/// makes the trace invalid).
///
/// Lifted out of `verify_trace_with` as a `pub(crate)` helper so the
/// unit test in this module's test block can drive it directly with a
/// chain-aggregator outcome built against a test roster — the
/// production `ATLAS_WITNESS_V1_ROSTER` is genesis-empty, so the
/// passing branch (`verified >= threshold`) cannot otherwise be
/// exercised through `verify_trace_with` until commissioning lands.
/// Keeps the threshold check single-source-of-truth: a future text or
/// shape change at the helper propagates to both the production call
/// site and the test, preventing silent drift.
pub(crate) fn apply_witness_threshold(
    verified: usize,
    threshold: usize,
    evidence: &mut Vec<VerifyEvidence>,
    errors: &mut Vec<String>,
) {
    if threshold == 0 {
        return;
    }
    if verified < threshold {
        let msg = format!(
            "strict mode: {} of {} required witness attestor(s) verified (--require-witness)",
            verified, threshold,
        );
        errors.push(msg.clone());
        evidence.push(VerifyEvidence {
            check: "witnesses-threshold".to_string(),
            ok: false,
            detail: msg,
        });
    } else {
        evidence.push(VerifyEvidence {
            check: "witnesses-threshold".to_string(),
            ok: true,
            detail: format!(
                "{} of {} required witness attestor(s) verified (strict mode)",
                verified, threshold,
            ),
        });
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

/// Walk an `AnchorChain` and produce a `WitnessVerifyOutcome`
/// aggregated across every batch in `chain.history` against
/// `ATLAS_WITNESS_V1_ROSTER`.
///
/// Two aggregations happen here:
///   * `presented` and `verified` counts sum across batches.
///   * `failures` accumulate with a `batch[N]` prefix per entry so the
///     auditor diagnostic identifies which batch the witness was
///     attached to — important when one batch fails and another passes.
///
/// We do NOT gate this on the chain-internal walk's success: a witness
/// sig commits to one SPECIFIC batch's recomputed head, which is
/// well-defined even if adjacent batches in the chain don't link
/// cleanly. The chain-link check and the witness check are independent
/// trust properties — surfacing both is more useful than collapsing
/// them.
///
/// Wave-C-2 (strict mode) reads the aggregate's `verified` count to
/// evaluate `opts.require_witness_threshold`; wave-C-1 (lenient
/// evidence) derives a `VerifyEvidence` row from the same aggregate via
/// `witness_evidence_from_aggregate`. Computing once per trace keeps
/// the two callers grounded in the same source of truth.
///
/// **Cross-batch kid distinctness (V1.13 wave-C-2 security fix).** The
/// per-batch verifier (`verify_witnesses_against_roster`) rejects
/// duplicate-`witness_kid` *within* a single batch via the BTreeMap
/// pre-pass, but the chain-level rollup must also enforce
/// distinctness *across* batches: otherwise an issuer holding a single
/// commissioned key could place one valid `WitnessSig` from kid `k`
/// in each of N batches and satisfy `require_witness_threshold = N`
/// with only ONE independent attestor.
///
/// Each kid that verifies is recorded in `verified_kids: BTreeSet`
/// (BTreeSet for deterministic ordering and to remove a hash-DoS
/// vector); a subsequent batch presenting the same kid does NOT
/// increment `verified` even if its signature is otherwise valid —
/// instead it surfaces as a `WitnessFailure` carrying the
/// "duplicate witness_kid across batches" reason. Without this
/// guard, the M-of-N trust property would degrade to "one key signed
/// N batches" — equivalent to no threshold at all from a
/// trust-domain-separation perspective.
pub(crate) fn aggregate_witnesses_across_chain(chain: &AnchorChain) -> WitnessVerifyOutcome {
    aggregate_witnesses_across_chain_with_roster(chain, ATLAS_WITNESS_V1_ROSTER)
}

/// Roster-parameterised variant of [`aggregate_witnesses_across_chain`].
///
/// Production code calls the un-parameterised wrapper (which pins
/// `ATLAS_WITNESS_V1_ROSTER` so the trust property — verification only
/// against pinned, source-controlled keys — is enforced at the
/// callsite). This `_with_roster` form exists so the cross-batch
/// dedup logic can be exercised against a test roster — the genesis
/// `ATLAS_WITNESS_V1_ROSTER` is empty, so no kid can ever reach the
/// `verified_kids` insertion path through the production wrapper.
/// Without this seam, the cross-batch dedup branch would be dead code
/// from a test-coverage perspective even though it is the load-bearing
/// defence against the M-of-N independence bypass.
pub(crate) fn aggregate_witnesses_across_chain_with_roster(
    chain: &AnchorChain,
    roster: &[(&str, [u8; 32])],
) -> WitnessVerifyOutcome {
    use std::collections::BTreeSet;

    let mut presented = 0usize;
    let mut verified = 0usize;
    let mut failures: Vec<WitnessFailure> = Vec::new();
    // Tracks every kid that has been counted as `verified` in any
    // prior batch. A kid that re-appears in a later batch is rejected
    // as a cross-batch duplicate — preserving the M-of-N independence
    // property under threshold strict mode.
    let mut verified_kids: BTreeSet<String> = BTreeSet::new();

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
                // Wrap as `BadWitness` so consumers filtering on
                // error variant see one consistent witness-domain
                // bucket (rather than mixed `BadWitness` +
                // `Encoding`); the original encoding error text is
                // preserved in `reason`.
                presented += batch.witnesses.len();
                for w in &batch.witnesses {
                    // Sanitize the kid before it flows into the
                    // failure record — the wire-side string is
                    // attacker-controlled and bypasses the per-batch
                    // verifier's `MAX_WITNESS_KID_LEN` guard on this
                    // error path (we never call
                    // `verify_witness_against_roster` when
                    // `chain_head_for` fails for the batch). Without
                    // this clamp, an oversized kid would land in
                    // `WitnessFailure.witness_kid` and again — via
                    // `Display` — in the lenient evidence row's
                    // `rendered.join("; ")`, amplifying log volume by
                    // the kid byte-length on the failure path. The
                    // shared `sanitize_kid_for_diagnostic` helper
                    // keeps the placeholder shape byte-identical with
                    // the per-batch verifier path so auditor
                    // diagnostics stay uniform.
                    let sanitized_kid = sanitize_kid_for_diagnostic(&w.witness_kid);
                    failures.push(WitnessFailure {
                        batch_index: Some(batch.batch_index),
                        witness_kid: sanitized_kid.clone(),
                        error: TrustError::BadWitness {
                            witness_kid: sanitized_kid,
                            reason: format!("chain_head recompute failed: {}", e),
                        },
                        reason_code: WitnessFailureReason::ChainHeadDecodeFailed,
                    });
                }
                continue;
            }
        };
        let batch_outcome = verify_witnesses_against_roster(
            &batch.witnesses,
            head.as_str(),
            roster,
        );
        presented += batch_outcome.presented;

        // Build the per-batch failed-kid set. The per-batch verifier
        // already records EVERY occurrence of a duplicate-within-batch
        // kid as a failure (so a kid in this set may have appeared
        // multiple times), making set membership a sufficient test for
        // "this witness did NOT verify at the per-batch level".
        let batch_failed_kids: BTreeSet<&str> = batch_outcome
            .failures
            .iter()
            .map(|f| f.witness_kid.as_str())
            .collect();

        // Walk the batch's witnesses to apply cross-batch dedup. A
        // witness whose kid verified at the per-batch level AND is not
        // already in `verified_kids` increments the global counter and
        // joins the seen-set; one that re-appears generates a
        // cross-batch-duplicate failure WITHOUT incrementing.
        for w in &batch.witnesses {
            if batch_failed_kids.contains(w.witness_kid.as_str()) {
                // Already accounted for in batch_outcome.failures
                // (re-emitted with batch_index below). No cross-batch
                // bookkeeping for failed-at-batch-level witnesses.
                continue;
            }
            if verified_kids.contains(&w.witness_kid) {
                // This kid already verified in an earlier batch.
                // Reject as cross-batch duplicate — this signature is
                // valid in isolation but does NOT add an independent
                // attestor to the threshold count.
                //
                // Sanitise the kid before placing it on the auditor
                // wire-surface — same defence-in-depth contract as the
                // per-batch path: a hostile signer-side could embed a
                // multi-MB blob in `witness_kid`, and the auditor JSON
                // must not amplify it.
                let sanitized_cross = sanitize_kid_for_diagnostic(&w.witness_kid);
                failures.push(WitnessFailure {
                    batch_index: Some(batch.batch_index),
                    witness_kid: sanitized_cross.clone(),
                    error: TrustError::BadWitness {
                        witness_kid: sanitized_cross,
                        reason: "duplicate witness_kid across batches \
                                 (already verified in an earlier batch — \
                                 does not count toward M-of-N threshold)"
                            .to_string(),
                    },
                    reason_code: WitnessFailureReason::CrossBatchDuplicateKid,
                });
            } else {
                verified += 1;
                // Invariant: only kids that passed
                // `verify_witness_against_roster_categorized` reach this
                // arm, and that helper's first guard is the
                // `MAX_WITNESS_KID_LEN` length cap (witness.rs §oversize
                // guard). `verified_kids` therefore only ever contains
                // bounded-length strings — the dedup key cannot itself
                // be a multi-MB blob. If the per-batch length guard is
                // ever lifted or reordered, this insert silently breaks
                // the wire-side amplification defence further down (the
                // cross-batch dup branch reads from `verified_kids` and
                // routes the matched kid onto the auditor wire).
                verified_kids.insert(w.witness_kid.clone());
            }
        }

        // Per-batch failures arrive without batch context (the
        // verifier-side helper has no batch_index in scope); attach
        // it here so the chain-aggregate consumer (evidence row,
        // strict-mode check) sees uniformly-batched failures.
        //
        // The `debug_assert` enforces the per-batch-verifier-side
        // contract structurally rather than by comment: if a future
        // refactor of `verify_witnesses_against_roster` ever attaches
        // a `batch_index` itself, the silent overwrite below would
        // produce misleading audit trails. Crash in dev/test rather
        // than ship the regression to production. (Release builds
        // accept the overwrite to keep the per-witness rollup hot
        // path branch-light.)
        for f in batch_outcome.failures {
            debug_assert!(
                f.batch_index.is_none(),
                "verify_witnesses_against_roster must not set batch_index — \
                 the chain aggregator owns that field. Saw Some({:?}) on kid {}.",
                f.batch_index,
                f.witness_kid,
            );
            failures.push(WitnessFailure {
                batch_index: Some(batch.batch_index),
                witness_kid: f.witness_kid,
                error: f.error,
                // Propagate the per-batch verifier's at-source
                // categorisation — re-deriving it here would
                // re-introduce the string-match coupling that
                // V1.14 Scope J explicitly avoids.
                reason_code: f.reason_code,
            });
        }
    }

    WitnessVerifyOutcome {
        presented,
        verified,
        failures,
    }
}

/// Derive the wave-C-1 `witnesses` evidence row from a chain-aggregated
/// `WitnessVerifyOutcome`. Lenient disposition: a non-empty `failures`
/// list surfaces as `ok: false` carrying the per-failure breakdown,
/// but the caller does NOT push the failures into the top-level
/// `errors` collection — the trace stays valid. Wave-C-2 strict-mode
/// promotion happens at the threshold check site, separate from this
/// evidence row.
///
/// Rationale for the no-op disposition when nothing was presented:
/// V1.13 ships with an empty roster (commissioning ceremony lands in
/// wave C-2), so a strict default would universally pass with "0 of 0
/// verified" — uninformative — while flipping to a true threshold
/// without operator opt-in would surprise existing deployments.
///
/// # `detail` is a human-readable rendering, NOT structured data
///
/// The `detail` string concatenates `WitnessFailure::Display` output
/// for operator log-grepping. **Programmatic auditor tooling** that
/// needs structured `Vec<WitnessFailure>` access (kid + structured
/// `TrustError` + optional `batch_index`) MUST NOT parse the
/// human-readable detail string — render is non-stable across crate
/// versions. Library-level callers obtain the structured rollup by
/// calling [`aggregate_witnesses_across_chain`] directly on the
/// chain (the same call this function consumes), then iterating
/// `outcome.failures`. Promoting the structured payload to a
/// first-class `VerifyEvidence` field is deferred until either
/// `TrustError` gains `Serialize` (currently `Debug + Clone + Error`
/// only — see `crate::error::TrustError`) or a dedicated wire-stable
/// `WitnessFailureWire` projection lands; both are wave-C-3 / V1.14
/// candidates.
fn witness_evidence_from_aggregate(aggregate: &WitnessVerifyOutcome) -> VerifyEvidence {
    if aggregate.presented == 0 {
        VerifyEvidence {
            check: "witnesses".to_string(),
            ok: true,
            detail: "no witnesses presented (lenient mode passes)".to_string(),
        }
    } else if aggregate.failures.is_empty() {
        VerifyEvidence {
            check: "witnesses".to_string(),
            ok: true,
            detail: format!(
                "{} witness sig(s) verified across chain history",
                aggregate.verified,
            ),
        }
    } else {
        // Render failures via WitnessFailure's Display impl — keeps
        // the lenient detail human-readable while the structured
        // `Vec<WitnessFailure>` remains available for programmatic
        // consumers.
        let rendered: Vec<String> =
            aggregate.failures.iter().map(|f| f.to_string()).collect();
        VerifyEvidence {
            check: "witnesses".to_string(),
            ok: false,
            detail: format!(
                "lenient: {} of {} witness sig(s) verified; {} failure(s) recorded but non-blocking (set --require-witness <N> to promote to errors): {}",
                aggregate.verified,
                aggregate.presented,
                aggregate.failures.len(),
                rendered.join("; "),
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
        let ev = witness_evidence_from_aggregate(&aggregate_witnesses_across_chain(&chain));
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
        let head_zero = chain_head_for(&chain.history[0]).unwrap().into_inner();
        chain.history.push(batch_no_witnesses(1, &head_zero));
        chain.head = chain_head_for(&chain.history[1]).unwrap().into_inner();

        let ev = witness_evidence_from_aggregate(&aggregate_witnesses_across_chain(&chain));
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
        chain.head = chain_head_for(&chain.history[0]).unwrap().into_inner();

        let ev = witness_evidence_from_aggregate(&aggregate_witnesses_across_chain(&chain));
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
            witness_failures: vec![],
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
            witness_failures: vec![],
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
            witness_failures: vec![],
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
            witness_failures: vec![],
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
        let head_zero = chain_head_for(&chain.history[0]).unwrap().into_inner();

        let mut batch_one = batch_no_witnesses(1, &head_zero);
        batch_one.witnesses.push(WitnessSig {
            witness_kid: "k-on-batch-1".to_string(),
            signature: "B".repeat(86),
        });
        chain.history.push(batch_one);
        chain.head = chain_head_for(&chain.history[1]).unwrap().into_inner();

        let ev = witness_evidence_from_aggregate(&aggregate_witnesses_across_chain(&chain));
        assert!(!ev.ok);
        assert!(
            ev.detail.contains("batch[1]"),
            "detail must label which batch the failure belongs to: {}",
            ev.detail,
        );
    }

    /// Cross-batch kid distinctness (V1.13 wave-C-2 SEC HIGH-1
    /// regression test). Two batches each carry a validly-signed
    /// witness from the SAME commissioned kid. Without the cross-batch
    /// dedup in `aggregate_witnesses_across_chain_with_roster`, this
    /// would yield `verified == 2` and let an issuer holding a single
    /// commissioned key satisfy a 2-of-N threshold trivially —
    /// collapsing the M-of-N independence property to "one key signed
    /// N batches".
    ///
    /// Expected aggregate after the fix: `presented == 2`,
    /// `verified == 1` (only the first occurrence counts), and one
    /// `WitnessFailure` carrying a "duplicate witness_kid across
    /// batches" reason against `batch[1]`.
    #[test]
    fn cross_batch_duplicate_kid_does_not_double_count_verified() {
        use crate::witness::{decode_chain_head, witness_signing_input};
        use ed25519_dalek::{Signer, SigningKey};

        // Test roster: ONE commissioned kid `k1` whose pubkey we
        // control via `sk1`. We deliberately do NOT touch
        // ATLAS_WITNESS_V1_ROSTER (the genesis-empty production roster
        // invariant must hold); the parameterised aggregator
        // `_with_roster` lets us inject this test roster instead.
        let sk1 = SigningKey::from_bytes(&[7u8; 32]);
        let pk1 = sk1.verifying_key().to_bytes();
        let test_roster: &[(&str, [u8; 32])] = &[("k1-cross-batch-dup", pk1)];

        // Batch 0 — no parent, signs over its own head.
        let mut chain = empty_chain();
        let mut batch0 = batch_no_witnesses(0, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        chain.history.push(batch0.clone());
        let head0 = chain_head_for(&chain.history[0]).unwrap();
        // Sign batch 0's head under k1.
        let head0_bytes = decode_chain_head(head0.as_str()).unwrap();
        let sig0 = sk1.sign(&witness_signing_input(&head0_bytes));
        batch0.witnesses.push(WitnessSig {
            witness_kid: "k1-cross-batch-dup".to_string(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(sig0.to_bytes()),
        });
        // Replace batch 0 in history with the witnessed copy. The head
        // changes because witnesses ARE part of the canonical
        // chain-batch body (verified via `chain_head_for`'s
        // canonical-bytes invariant). Recompute.
        chain.history[0] = batch0;
        let head0_witnessed = chain_head_for(&chain.history[0]).unwrap();

        // Batch 1 — chains off batch 0's NEW head (post-witness).
        let mut batch1 = batch_no_witnesses(1, head0_witnessed.as_str());
        chain.history.push(batch1.clone());
        let head1 = chain_head_for(&chain.history[1]).unwrap();
        let head1_bytes = decode_chain_head(head1.as_str()).unwrap();
        let sig1 = sk1.sign(&witness_signing_input(&head1_bytes));
        batch1.witnesses.push(WitnessSig {
            witness_kid: "k1-cross-batch-dup".to_string(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(sig1.to_bytes()),
        });
        chain.history[1] = batch1;
        let head1_witnessed = chain_head_for(&chain.history[1]).unwrap();
        chain.head = head1_witnessed.into_inner();

        // Aggregate against the test roster.
        let agg =
            aggregate_witnesses_across_chain_with_roster(&chain, test_roster);

        assert_eq!(agg.presented, 2, "two batches each present a sig: presented=2");
        assert_eq!(
            agg.verified, 1,
            "cross-batch dedup must collapse to verified=1 (only first batch counts), got {}",
            agg.verified,
        );
        assert_eq!(
            agg.failures.len(),
            1,
            "the second-batch occurrence must surface as ONE failure, got {:?}",
            agg.failures,
        );
        let f = &agg.failures[0];
        assert_eq!(
            f.batch_index,
            Some(1),
            "the failure must be tagged to batch[1] (the duplicate occurrence)",
        );
        assert_eq!(f.witness_kid, "k1-cross-batch-dup");
        assert!(
            matches!(
                &f.error,
                TrustError::BadWitness { reason, .. }
                    if reason.contains("duplicate witness_kid across batches")
            ),
            "failure must be BadWitness with the cross-batch dup reason: {:?}",
            f.error,
        );
    }

    /// Cross-batch dedup must NOT poison sibling kids: batch[0] verifies
    /// kid A, batch[1] verifies kid B. Both count toward the threshold
    /// (verified == 2). Defends against an over-broad dedup that would
    /// reject any second-batch witness regardless of kid.
    #[test]
    fn cross_batch_distinct_kids_both_count() {
        use crate::witness::{decode_chain_head, witness_signing_input};
        use ed25519_dalek::{Signer, SigningKey};

        let sk_a = SigningKey::from_bytes(&[3u8; 32]);
        let pk_a = sk_a.verifying_key().to_bytes();
        let sk_b = SigningKey::from_bytes(&[5u8; 32]);
        let pk_b = sk_b.verifying_key().to_bytes();
        let test_roster: &[(&str, [u8; 32])] =
            &[("kid-a", pk_a), ("kid-b", pk_b)];

        let mut chain = empty_chain();
        let mut batch0 = batch_no_witnesses(0, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        chain.history.push(batch0.clone());
        let head0 = chain_head_for(&chain.history[0]).unwrap();
        let sig_a = sk_a.sign(&witness_signing_input(
            &decode_chain_head(head0.as_str()).unwrap(),
        ));
        batch0.witnesses.push(WitnessSig {
            witness_kid: "kid-a".to_string(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(sig_a.to_bytes()),
        });
        chain.history[0] = batch0;
        let head0_witnessed = chain_head_for(&chain.history[0]).unwrap();

        let mut batch1 = batch_no_witnesses(1, head0_witnessed.as_str());
        chain.history.push(batch1.clone());
        let head1 = chain_head_for(&chain.history[1]).unwrap();
        let sig_b = sk_b.sign(&witness_signing_input(
            &decode_chain_head(head1.as_str()).unwrap(),
        ));
        batch1.witnesses.push(WitnessSig {
            witness_kid: "kid-b".to_string(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(sig_b.to_bytes()),
        });
        chain.history[1] = batch1;
        chain.head = chain_head_for(&chain.history[1]).unwrap().into_inner();

        let agg =
            aggregate_witnesses_across_chain_with_roster(&chain, test_roster);
        assert_eq!(agg.presented, 2);
        assert_eq!(
            agg.verified, 2,
            "distinct kids across batches must BOTH count, got {}",
            agg.verified,
        );
        assert!(
            agg.failures.is_empty(),
            "no failures expected for distinct cross-batch kids: {:?}",
            agg.failures,
        );
    }

    /// V1.14 Scope J defence-in-depth pin: the cross-batch dup
    /// constructor at the top of this file must route `witness_kid`
    /// through `sanitize_kid_for_diagnostic` before placing it on the
    /// auditor wire surface. Both the outer `WitnessFailure.witness_kid`
    /// AND the inner `TrustError::BadWitness.witness_kid` must be
    /// sanitised — auditor JSON consumes both.
    ///
    /// Today's per-batch length guard at `MAX_WITNESS_KID_LEN`
    /// structurally prevents an oversize kid from ever reaching the
    /// cross-batch path (the per-batch verifier rejects it first), so
    /// `sanitize_kid_for_diagnostic` is identity on every kid that
    /// reaches here. That invariant could regress: a future patch that
    /// lifts the per-batch guard, or adds a parallel dedup path that
    /// skips the per-batch verifier, would re-open multi-MB blob
    /// amplification on the wire surface. This test pins the contract
    /// by asserting equality with the sanitised form rather than the
    /// raw input — so any future regression that plumbs the raw kid
    /// through trips the test before reaching production.
    #[test]
    fn cross_batch_duplicate_kid_failure_uses_sanitized_kid() {
        use crate::witness::{
            decode_chain_head, sanitize_kid_for_diagnostic, witness_signing_input,
        };
        use ed25519_dalek::{Signer, SigningKey};

        let raw_kid = "k1-cross-batch-dup";
        let sk1 = SigningKey::from_bytes(&[7u8; 32]);
        let pk1 = sk1.verifying_key().to_bytes();
        let test_roster: &[(&str, [u8; 32])] = &[(raw_kid, pk1)];

        let mut chain = empty_chain();
        let mut batch0 = batch_no_witnesses(0, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        chain.history.push(batch0.clone());
        let head0 = chain_head_for(&chain.history[0]).unwrap();
        let sig0 = sk1.sign(&witness_signing_input(
            &decode_chain_head(head0.as_str()).unwrap(),
        ));
        batch0.witnesses.push(WitnessSig {
            witness_kid: raw_kid.to_string(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(sig0.to_bytes()),
        });
        chain.history[0] = batch0;
        let head0_witnessed = chain_head_for(&chain.history[0]).unwrap();

        let mut batch1 = batch_no_witnesses(1, head0_witnessed.as_str());
        chain.history.push(batch1.clone());
        let head1 = chain_head_for(&chain.history[1]).unwrap();
        let sig1 = sk1.sign(&witness_signing_input(
            &decode_chain_head(head1.as_str()).unwrap(),
        ));
        batch1.witnesses.push(WitnessSig {
            witness_kid: raw_kid.to_string(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(sig1.to_bytes()),
        });
        chain.history[1] = batch1;
        chain.head = chain_head_for(&chain.history[1]).unwrap().into_inner();

        let agg =
            aggregate_witnesses_across_chain_with_roster(&chain, test_roster);
        assert_eq!(agg.failures.len(), 1, "expected one cross-batch dup failure");
        let f = &agg.failures[0];
        let sanitised = sanitize_kid_for_diagnostic(raw_kid);

        assert_eq!(
            f.witness_kid, sanitised,
            "outer WitnessFailure.witness_kid must equal sanitize_kid_for_diagnostic(raw)",
        );
        match &f.error {
            TrustError::BadWitness { witness_kid: inner, .. } => assert_eq!(
                inner, &sanitised,
                "inner TrustError::BadWitness.witness_kid must also be sanitised",
            ),
            other => panic!("expected BadWitness, got {:?}", other),
        }
    }

    /// `apply_witness_threshold` with `threshold == 0` is the lenient
    /// sentinel — emits no evidence row, no error, regardless of the
    /// `verified` count. Pins that strict-mode wiring stays opt-in
    /// (wave-C-1 traces continue to verify on a wave-C-2 verifier with
    /// default `VerifyOptions`).
    #[test]
    fn apply_witness_threshold_zero_is_no_op() {
        let mut evidence: Vec<VerifyEvidence> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        // Even with verified=0, threshold=0 must NOT emit a row or error.
        apply_witness_threshold(0, 0, &mut evidence, &mut errors);
        assert!(evidence.is_empty(), "threshold=0 must not emit evidence rows: {evidence:?}");
        assert!(errors.is_empty(), "threshold=0 must not emit errors: {errors:?}");
    }

    /// `apply_witness_threshold` with `verified < threshold` emits an
    /// `ok=false` `witnesses-threshold` row AND pushes a matching error
    /// onto the top-level errors list (which makes `verify_trace_with`
    /// return `valid=false`). Pins the failure-side wiring at the
    /// helper level so a refactor that drops the error push (and only
    /// emits the evidence row) cannot silently let strict-mode failures
    /// pass.
    #[test]
    fn apply_witness_threshold_fail_path_pushes_evidence_and_error() {
        let mut evidence: Vec<VerifyEvidence> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        apply_witness_threshold(1, 3, &mut evidence, &mut errors);
        assert_eq!(evidence.len(), 1, "exactly one threshold row expected");
        assert_eq!(evidence[0].check, "witnesses-threshold");
        assert!(!evidence[0].ok, "verified=1 < threshold=3 must be ok=false");
        assert!(
            evidence[0].detail.contains("1 of 3"),
            "detail must name verified/required (1 of 3): {}",
            evidence[0].detail,
        );
        assert_eq!(errors.len(), 1, "fail path MUST push exactly one error");
        assert!(
            errors[0].contains("1 of 3"),
            "error must name verified/required (1 of 3): {}",
            errors[0],
        );
    }

    /// `apply_witness_threshold` with `verified >= threshold` emits an
    /// `ok=true` `witnesses-threshold` row and does NOT push an error.
    /// This is the wave-C-2 strict-mode passing branch — unreachable
    /// through `verify_trace_with` under the genesis-empty production
    /// roster, so this test is the only direct exerciser of the
    /// success disposition until commissioning lands.
    #[test]
    fn apply_witness_threshold_pass_path_pushes_evidence_no_error() {
        let mut evidence: Vec<VerifyEvidence> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        apply_witness_threshold(3, 3, &mut evidence, &mut errors);
        assert_eq!(evidence.len(), 1);
        assert!(evidence[0].ok, "verified=3 >= threshold=3 must be ok=true");
        assert!(
            evidence[0].detail.contains("3 of 3"),
            "detail must name verified/required (3 of 3): {}",
            evidence[0].detail,
        );
        assert!(
            errors.is_empty(),
            "pass path must NOT push an error: {errors:?}",
        );

        // Above-threshold (over-witnessed) also passes.
        let mut evidence2: Vec<VerifyEvidence> = Vec::new();
        let mut errors2: Vec<String> = Vec::new();
        apply_witness_threshold(5, 3, &mut evidence2, &mut errors2);
        assert!(evidence2[0].ok, "verified=5 >= threshold=3 must be ok=true");
        assert!(errors2.is_empty());
    }

    /// End-to-end strict-mode passing path (V1.13 wave-C-2 MEDIUM-2
    /// regression): drives a real chain with two validly-signed
    /// witnesses through the parameterised aggregator (against a test
    /// roster, since the production roster is genesis-empty), then
    /// passes the resulting `verified` count into the same
    /// `apply_witness_threshold` helper that `verify_trace_with` calls.
    /// Asserts that `verified == 2 >= threshold == 2` produces an
    /// `ok=true` row and zero errors — the strict-mode green path that
    /// is otherwise inaccessible through the production-API integration
    /// tests in `tests/witness_strict_mode.rs`.
    #[test]
    fn strict_mode_passing_path_end_to_end() {
        use crate::witness::{decode_chain_head, witness_signing_input};
        use ed25519_dalek::{Signer, SigningKey};

        let sk_a = SigningKey::from_bytes(&[3u8; 32]);
        let pk_a = sk_a.verifying_key().to_bytes();
        let sk_b = SigningKey::from_bytes(&[5u8; 32]);
        let pk_b = sk_b.verifying_key().to_bytes();
        let test_roster: &[(&str, [u8; 32])] =
            &[("kid-a", pk_a), ("kid-b", pk_b)];

        // Single batch carrying TWO commissioned witnesses, both
        // signing the same head — distinct kids, so both count.
        let mut chain = empty_chain();
        let mut batch0 = batch_no_witnesses(0, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        chain.history.push(batch0.clone());
        let head0 = chain_head_for(&chain.history[0]).unwrap();
        let head0_bytes = decode_chain_head(head0.as_str()).unwrap();
        for (kid, sk) in [("kid-a", &sk_a), ("kid-b", &sk_b)] {
            let sig = sk.sign(&witness_signing_input(&head0_bytes));
            batch0.witnesses.push(WitnessSig {
                witness_kid: kid.to_string(),
                signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(sig.to_bytes()),
            });
        }
        chain.history[0] = batch0;
        chain.head = chain_head_for(&chain.history[0]).unwrap().into_inner();

        let agg =
            aggregate_witnesses_across_chain_with_roster(&chain, test_roster);
        assert_eq!(agg.presented, 2, "two witnesses presented");
        assert_eq!(agg.verified, 2, "both kids must verify against the test roster");
        assert!(agg.failures.is_empty(), "no failures expected: {:?}", agg.failures);

        // Now drive the SAME helper that production calls, with the
        // strict threshold matching the verified count — pass branch.
        let mut evidence: Vec<VerifyEvidence> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        apply_witness_threshold(agg.verified, 2, &mut evidence, &mut errors);
        let row = evidence
            .iter()
            .find(|e| e.check == "witnesses-threshold")
            .expect("witnesses-threshold row must be emitted under strict mode");
        assert!(row.ok, "strict-mode pass must emit ok=true: {row:?}");
        assert!(
            row.detail.contains("2 of 2"),
            "detail must name (2 of 2): {}",
            row.detail,
        );
        assert!(
            errors.is_empty(),
            "strict-mode pass must NOT push an error: {errors:?}",
        );
    }
}
