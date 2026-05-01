//! V1.9 adversary suite — per-tenant signing-key isolation.
//!
//! V1.5–V1.8 bound a single global Ed25519 keypair to every workspace
//! via three shared kids (`spiffe://atlas/agent/...`,
//! `spiffe://atlas/human/...`, `spiffe://atlas/system/...`). One key
//! compromise forged events for *every* tenant.
//!
//! V1.9 issues a workspace-derived keypair per tenant under the kid
//! `atlas-anchor:{workspace_id}` and adds
//! `VerifyOptions::require_per_tenant_keys` to enforce it. The
//! verifier must:
//!
//!   1. accept legacy bundles in lenient mode (compat fence);
//!   2. accept per-tenant bundles in lenient mode;
//!   3. reject legacy kids in strict mode;
//!   4. reject any cross-workspace kid in strict mode (event signed by
//!      tenant-B's key claiming to belong to tenant-A);
//!   5. reject mixed bundles where some events hide behind legacy kids
//!      and others use per-tenant kids;
//!   6. reject any attempt to relabel a per-tenant trace's workspace_id
//!      after signing (workspace_id is bound into the signing input).
//!
//! These tests construct traces directly — trust-core does not depend
//! on atlas-signer's HKDF derivation. The kid shape is the only piece
//! the verifier cares about; the signing key behind a per-tenant kid
//! can be any Ed25519 key for the purposes of these tests, as long as
//! the bundle agrees with the events.

use atlas_trust_core::{
    cose::build_signing_input,
    hashchain::compute_event_hash,
    per_tenant::per_tenant_kid_for,
    pubkey_bundle::PubkeyBundle,
    trace_format::{AtlasEvent, AtlasTrace, EventSignature},
    verify::{verify_trace, verify_trace_with, VerifyOptions},
};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;

const LEGACY_AGENT_KID: &str = "spiffe://atlas/agent/cursor-001";

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn build_signed_event(
    signing_key: &SigningKey,
    workspace_id: &str,
    kid: &str,
    event_id: &str,
    ts: &str,
    parents: Vec<String>,
    payload: serde_json::Value,
) -> AtlasEvent {
    let signing_input =
        build_signing_input(workspace_id, event_id, ts, kid, &parents, &payload).unwrap();
    let event_hash = compute_event_hash(&signing_input);
    let signature = signing_key.sign(&signing_input);
    AtlasEvent {
        event_id: event_id.to_string(),
        event_hash,
        parent_hashes: parents,
        payload,
        signature: EventSignature {
            alg: "EdDSA".to_string(),
            kid: kid.to_string(),
            sig: b64url(&signature.to_bytes()),
        },
        ts: ts.to_string(),
    }
}

/// Build a single-key bundle pinning `kid` -> the verifying key of `signing_key`.
fn bundle_with(kid: &str, signing_key: &SigningKey) -> PubkeyBundle {
    let mut keys = HashMap::new();
    keys.insert(kid.to_string(), b64url(signing_key.verifying_key().as_bytes()));
    PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-04-29T10:00:00Z".to_string(),
        keys,
    }
}

/// Build a bundle pinning multiple (kid, key) pairs.
fn bundle_with_many(entries: &[(&str, &SigningKey)]) -> PubkeyBundle {
    let mut keys = HashMap::new();
    for (kid, sk) in entries {
        keys.insert(
            (*kid).to_string(),
            b64url(sk.verifying_key().as_bytes()),
        );
    }
    PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-04-29T10:00:00Z".to_string(),
        keys,
    }
}

fn make_per_tenant_trace(
    signing_key: &SigningKey,
    workspace_id: &str,
) -> (AtlasTrace, PubkeyBundle) {
    let kid = per_tenant_kid_for(workspace_id);
    let bundle = bundle_with(&kid, signing_key);
    let bundle_hash = bundle.deterministic_hash().unwrap();
    let event = build_signed_event(
        signing_key,
        workspace_id,
        &kid,
        "01H001",
        "2026-04-29T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create", "node": {"id": "n1"}}),
    );
    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-29T10:01:00Z".to_string(),
        workspace_id: workspace_id.to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![event.clone()],
        dag_tips: vec![event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: None,
    };
    (trace, bundle)
}

fn make_legacy_trace(
    signing_key: &SigningKey,
    workspace_id: &str,
) -> (AtlasTrace, PubkeyBundle) {
    let bundle = bundle_with(LEGACY_AGENT_KID, signing_key);
    let bundle_hash = bundle.deterministic_hash().unwrap();
    let event = build_signed_event(
        signing_key,
        workspace_id,
        LEGACY_AGENT_KID,
        "01H001",
        "2026-04-29T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create", "node": {"id": "n1"}}),
    );
    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-29T10:01:00Z".to_string(),
        workspace_id: workspace_id.to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![event.clone()],
        dag_tips: vec![event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: None,
    };
    (trace, bundle)
}

// ─────────────────────────────────────────────────────────────────────────
// Compat fences — V1.5–V1.8 bundles must keep verifying.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn legacy_kid_passes_lenient_mode() {
    let sk = SigningKey::from_bytes(&[7u8; 32]);
    let (trace, bundle) = make_legacy_trace(&sk, "ws-legacy");
    let outcome = verify_trace(&trace, &bundle);
    assert!(
        outcome.valid,
        "V1.5–V1.8 legacy traces must keep passing lenient verify; errors: {:#?}",
        outcome.errors
    );
}

#[test]
fn per_tenant_kid_passes_lenient_mode() {
    let sk = SigningKey::from_bytes(&[8u8; 32]);
    let (trace, bundle) = make_per_tenant_trace(&sk, "ws-alice");
    let outcome = verify_trace(&trace, &bundle);
    assert!(
        outcome.valid,
        "V1.9 per-tenant traces must verify in lenient mode too; errors: {:#?}",
        outcome.errors
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Strict-mode happy path.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn per_tenant_kid_passes_strict_mode() {
    let sk = SigningKey::from_bytes(&[9u8; 32]);
    let (trace, bundle) = make_per_tenant_trace(&sk, "ws-alice");
    let opts = VerifyOptions {
        require_per_tenant_keys: true,
        ..VerifyOptions::default()
    };
    let outcome = verify_trace_with(&trace, &bundle, &opts);
    assert!(
        outcome.valid,
        "per-tenant trace must pass strict mode; errors: {:#?}",
        outcome.errors
    );
    assert!(
        outcome
            .evidence
            .iter()
            .any(|ev| ev.check == "per-tenant-keys" && ev.ok),
        "expected per-tenant-keys evidence row marked ok=true",
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Strict-mode adversaries.
// ─────────────────────────────────────────────────────────────────────────

/// Adversary: V1.5–V1.8 trace presented to a strict-mode verifier. Every
/// event carries a legacy SPIFFE kid; strict mode must reject and call
/// out the offending kid.
#[test]
fn legacy_kid_rejected_in_strict_mode() {
    let sk = SigningKey::from_bytes(&[10u8; 32]);
    let (trace, bundle) = make_legacy_trace(&sk, "ws-legacy");
    let opts = VerifyOptions {
        require_per_tenant_keys: true,
        ..VerifyOptions::default()
    };
    let outcome = verify_trace_with(&trace, &bundle, &opts);
    assert!(
        !outcome.valid,
        "strict mode must reject legacy SPIFFE kids; outcome: {:#?}",
        outcome
    );
    assert!(
        outcome
            .errors
            .iter()
            .any(|e| e.contains("per-tenant kid 'atlas-anchor:ws-legacy'")),
        "expected per-tenant-kid error citing the expected kid; got: {:#?}",
        outcome.errors
    );
}

/// Adversary: cross-workspace forgery. Tenant Bob's per-tenant signing
/// key signs an event, the bundle pins Bob's kid, but the trace claims
/// workspace_id = "ws-alice". Two failures stack:
///   * event-signatures: workspace_id is bound into the signing input
///     so the recomputed input differs and the signature fails to
///     verify;
///   * (had it not failed earlier) strict-mode per-tenant-keys: the
///     event's kid `atlas-anchor:ws-bob` ≠ expected
///     `atlas-anchor:ws-alice`.
///
/// Either failure alone is sufficient — both fire, defense-in-depth.
#[test]
fn cross_workspace_per_tenant_forgery_rejected() {
    let bob_sk = SigningKey::from_bytes(&[11u8; 32]);
    let bob_kid = per_tenant_kid_for("ws-bob");

    // Sign for ws-bob.
    let event = build_signed_event(
        &bob_sk,
        "ws-bob",
        &bob_kid,
        "01H_BOB_EVT",
        "2026-04-29T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create", "node": {"id": "stolen"}}),
    );

    // Forged trace claims this is alice's data.
    let bundle = bundle_with(&bob_kid, &bob_sk);
    let bundle_hash = bundle.deterministic_hash().unwrap();
    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-29T10:01:00Z".to_string(),
        workspace_id: "ws-alice".to_string(), // lie
        pubkey_bundle_hash: bundle_hash,
        events: vec![event.clone()],
        dag_tips: vec![event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: None,
    };

    // Lenient mode already catches this via hash mismatch (workspace bound
    // into signing input).
    let lenient = verify_trace(&trace, &bundle);
    assert!(!lenient.valid, "cross-workspace forgery must fail lenient");
    assert!(
        lenient.errors.iter().any(|e| e.contains("hash mismatch")),
        "expected hash-mismatch defence; got: {:#?}",
        lenient.errors
    );

    // Strict mode also catches it on the kid axis.
    let strict_opts = VerifyOptions {
        require_per_tenant_keys: true,
        ..VerifyOptions::default()
    };
    let strict = verify_trace_with(&trace, &bundle, &strict_opts);
    assert!(!strict.valid, "cross-workspace forgery must fail strict");
    assert!(
        strict
            .errors
            .iter()
            .any(|e| e.contains("per-tenant kid 'atlas-anchor:ws-alice'")),
        "expected per-tenant-kid error in strict mode; got: {:#?}",
        strict.errors
    );
}

/// Adversary: mixed bundle. Two events — one signed under a legacy SPIFFE
/// kid, the other under the per-tenant kid. Bundle pins both kids so
/// lenient verify passes. Strict mode must reject because the legacy
/// event no longer satisfies the per-tenant invariant.
#[test]
fn mixed_legacy_and_per_tenant_kids_rejected_in_strict_mode() {
    let workspace = "ws-mixed";
    let pt_kid = per_tenant_kid_for(workspace);

    let legacy_sk = SigningKey::from_bytes(&[12u8; 32]);
    let pt_sk = SigningKey::from_bytes(&[13u8; 32]);

    let bundle = bundle_with_many(&[(LEGACY_AGENT_KID, &legacy_sk), (pt_kid.as_str(), &pt_sk)]);
    let bundle_hash = bundle.deterministic_hash().unwrap();

    let legacy_event = build_signed_event(
        &legacy_sk,
        workspace,
        LEGACY_AGENT_KID,
        "01H_LEGACY",
        "2026-04-29T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create", "node": {"id": "lg"}}),
    );
    let pt_event = build_signed_event(
        &pt_sk,
        workspace,
        &pt_kid,
        "01H_PT",
        "2026-04-29T10:00:01Z",
        vec![legacy_event.event_hash.clone()],
        serde_json::json!({"type": "node.create", "node": {"id": "pt"}}),
    );
    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-29T10:01:00Z".to_string(),
        workspace_id: workspace.to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![legacy_event.clone(), pt_event.clone()],
        dag_tips: vec![pt_event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: None,
    };

    let lenient = verify_trace(&trace, &bundle);
    assert!(
        lenient.valid,
        "mixed-kid trace must verify in lenient mode (compat); errors: {:#?}",
        lenient.errors
    );

    let strict_opts = VerifyOptions {
        require_per_tenant_keys: true,
        ..VerifyOptions::default()
    };
    let strict = verify_trace_with(&trace, &bundle, &strict_opts);
    assert!(
        !strict.valid,
        "strict mode must reject mixed-kid trace (legacy event present); outcome: {:#?}",
        strict
    );
    assert!(
        strict
            .errors
            .iter()
            .any(|e| e.contains("01H_LEGACY") && e.contains("atlas-anchor:ws-mixed")),
        "strict-mode error must name the offending event id; got: {:#?}",
        strict.errors
    );
}

/// Adversary: a per-tenant trace with the kid shape correct but pointing
/// at the *wrong* workspace. E.g. event kid `atlas-anchor:ws-bob` inside
/// a trace claiming `workspace_id = "ws-alice"`. Strict mode must reject:
/// the kid is per-tenant-shaped, but for the wrong tenant.
#[test]
fn per_tenant_kid_with_wrong_workspace_rejected_in_strict_mode() {
    let bob_sk = SigningKey::from_bytes(&[14u8; 32]);
    let bob_kid = per_tenant_kid_for("ws-bob");

    // Construct a bundle with bob's kid (so the signature alone would
    // verify) and re-label the trace as ws-alice. This isolates the
    // per-tenant kid check from the cross-workspace hash defence.
    let bundle = bundle_with(&bob_kid, &bob_sk);
    let bundle_hash = bundle.deterministic_hash().unwrap();

    // Build the event for ws-alice but sign with bob's kid — strict
    // mode must catch the kid mismatch even though the signature
    // itself is structurally valid.
    let alice_event = build_signed_event(
        &bob_sk,
        "ws-alice",
        &bob_kid,
        "01H_ALICE_FAKE",
        "2026-04-29T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create"}),
    );

    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-29T10:01:00Z".to_string(),
        workspace_id: "ws-alice".to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![alice_event.clone()],
        dag_tips: vec![alice_event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: None,
    };

    // Lenient: signature is valid (bob's key signed an event over
    // ws-alice's workspace_id, with bob's kid; bundle has bob's kid;
    // workspace_id is bound into signing input so it consistently
    // verifies). The legacy verifier has no notion of "kid must equal
    // atlas-anchor:{trace.workspace_id}".
    let lenient = verify_trace(&trace, &bundle);
    assert!(
        lenient.valid,
        "lenient mode does not enforce kid<->workspace pairing; errors: {:#?}",
        lenient.errors
    );

    // Strict: the kid `atlas-anchor:ws-bob` ≠ expected
    // `atlas-anchor:ws-alice`. Reject.
    let strict_opts = VerifyOptions {
        require_per_tenant_keys: true,
        ..VerifyOptions::default()
    };
    let strict = verify_trace_with(&trace, &bundle, &strict_opts);
    assert!(
        !strict.valid,
        "strict mode must reject per-tenant kid for the wrong workspace; outcome: {:#?}",
        strict
    );
    assert!(
        strict
            .errors
            .iter()
            .any(|e| e.contains("01H_ALICE_FAKE") && e.contains("atlas-anchor:ws-alice")),
        "strict-mode error must reference the wrong-workspace event; got: {:#?}",
        strict.errors
    );
}

/// Sanity: the per-tenant-keys evidence row only appears when strict
/// mode is on. Auditors reading lenient outcomes shouldn't see a check
/// that wasn't actually run.
#[test]
fn per_tenant_evidence_absent_when_lenient() {
    let sk = SigningKey::from_bytes(&[15u8; 32]);
    let (trace, bundle) = make_per_tenant_trace(&sk, "ws-quiet");
    let outcome = verify_trace(&trace, &bundle);
    assert!(outcome.valid);
    assert!(
        !outcome
            .evidence
            .iter()
            .any(|ev| ev.check == "per-tenant-keys"),
        "lenient mode must not emit per-tenant-keys evidence; got: {:#?}",
        outcome.evidence
    );
}

/// Adversary: tampered `pubkey_bundle_hash` on an otherwise well-formed
/// per-tenant trace. The kid passes the strict-mode shape check
/// (`atlas-anchor:{workspace_id}`), but the trace's claimed bundle
/// hash points at a different bundle than the one supplied at verify
/// time. The pubkey-bundle-hash check must catch this in *both*
/// modes — strict-mode kid agreement is not a substitute for bundle
/// integrity, and a passing kid check must not paper over a tampered
/// hash.
///
/// This is the "valid kid + tampered bundle" combination — if a
/// future verifier ever short-circuited the bundle-hash check on the
/// strength of a passing per-tenant check, this test would catch it.
#[test]
fn tampered_bundle_hash_with_valid_per_tenant_kid_rejected() {
    let sk = SigningKey::from_bytes(&[16u8; 32]);
    let (mut trace, bundle) = make_per_tenant_trace(&sk, "ws-tamper");

    // Replace the (correct) bundle-hash with a clearly bogus value.
    // 32 zero bytes hex-encoded — structurally valid (passes Hex64
    // schema), semantically wrong (no bundle hashes to all-zero).
    trace.pubkey_bundle_hash = "0".repeat(64);

    // Lenient: pubkey-bundle-hash check fires.
    let lenient = verify_trace(&trace, &bundle);
    assert!(
        !lenient.valid,
        "lenient verify must reject tampered pubkey_bundle_hash; outcome: {:#?}",
        lenient
    );
    assert!(
        lenient
            .errors
            .iter()
            .any(|e| e.contains("pubkey bundle mismatch")),
        "expected pubkey bundle mismatch error; got: {:#?}",
        lenient.errors
    );

    // Strict: same hash defence still fires. Strict mode adds checks,
    // it does not replace existing ones.
    let strict_opts = VerifyOptions {
        require_per_tenant_keys: true,
        ..VerifyOptions::default()
    };
    let strict = verify_trace_with(&trace, &bundle, &strict_opts);
    assert!(
        !strict.valid,
        "strict verify must also reject tampered pubkey_bundle_hash; outcome: {:#?}",
        strict
    );
    assert!(
        strict
            .errors
            .iter()
            .any(|e| e.contains("pubkey bundle mismatch")),
        "strict mode must surface the same hash-mismatch error; got: {:#?}",
        strict.errors
    );
}

/// Adversary: zero-event trace presented under strict mode. The
/// per-tenant-keys check iterates events; with no events to iterate,
/// the check must complete without panic. Other invariants (DAG tips
/// must point inside `events`, anchors must cover tips) determine
/// whether the trace as a whole is valid — but the per-tenant pass
/// itself must produce a deterministic, non-error outcome regardless.
///
/// Why this matters: a strict-mode verifier that crashes on edge-case
/// inputs is worse than one that simply passes them. Auditors run
/// strict mode against arbitrary traces; a panic on empty events is a
/// DoS on the verifier and would erode operator trust in the strict
/// flag.
#[test]
fn zero_event_trace_does_not_crash_strict_mode() {
    let workspace = "ws-empty";
    // Build an empty bundle pinning just the per-tenant kid (with a
    // dummy key — there are no events to verify against it). This
    // ensures the strict-mode kid-shape check has nothing to iterate
    // and the bundle-hash check still computes deterministically.
    let dummy_sk = SigningKey::from_bytes(&[17u8; 32]);
    let kid = per_tenant_kid_for(workspace);
    let bundle = bundle_with(&kid, &dummy_sk);
    let bundle_hash = bundle.deterministic_hash().unwrap();

    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-29T10:01:00Z".to_string(),
        workspace_id: workspace.to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![],
        dag_tips: vec![],
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: None,
    };

    // Lenient: empty trace is structurally valid (no events to fail).
    // We assert `errors.is_empty()` too so that a future verifier
    // change that adds a spurious "trace must contain at least one
    // event" rejection lands on this test rather than slipping
    // through with `valid = true` and a residual error list.
    let lenient = verify_trace(&trace, &bundle);
    assert!(
        lenient.valid,
        "empty trace should verify in lenient mode; errors: {:#?}",
        lenient.errors
    );
    assert!(
        lenient.errors.is_empty(),
        "lenient verify of an empty trace must produce zero errors; got: {:#?}",
        lenient.errors,
    );

    // Strict: still valid — the per-tenant check has nothing to
    // reject. The evidence row should still appear (the check ran)
    // and should be ok=true (vacuously: zero events, zero offenders).
    // We pin both `valid` and `errors.is_empty()` so the "vacuous-ok"
    // claim is load-bearing — a regression that, say, started
    // rejecting empty-event traces under strict would fail here even
    // if the per-tenant evidence row stayed ok.
    let strict_opts = VerifyOptions {
        require_per_tenant_keys: true,
        ..VerifyOptions::default()
    };
    let strict = verify_trace_with(&trace, &bundle, &strict_opts);
    assert!(
        strict.valid,
        "strict mode on a zero-event trace must not crash or reject; outcome: {:#?}",
        strict
    );
    assert!(
        strict.errors.is_empty(),
        "strict mode on an empty trace must produce zero errors; got: {:#?}",
        strict.errors,
    );
    assert!(
        strict
            .evidence
            .iter()
            .any(|ev| ev.check == "per-tenant-keys" && ev.ok),
        "strict mode must emit per-tenant-keys evidence even for empty events; got: {:#?}",
        strict.evidence,
    );
}

/// Adversary: cross-bundle kid reuse. A per-tenant kid for workspace A
/// (`atlas-anchor:ws-alice`) is reused inside workspace B's bundle and
/// trace. The bundle is internally consistent (kid → key pair signs
/// every event). Strict mode must reject because the trace's
/// `workspace_id` is "ws-bob" but the events carry the alice-shaped
/// kid.
///
/// This catches an operator-error scenario more than a forgery: someone
/// edits a bundle by hand and pastes the wrong tenant's kid in. Without
/// the strict check, the trace verifies (signatures are internally
/// consistent), and the audit story silently lies about which tenant
/// produced the events.
#[test]
fn cross_bundle_kid_reuse_rejected_in_strict_mode() {
    let alice_kid = per_tenant_kid_for("ws-alice");
    let sk = SigningKey::from_bytes(&[18u8; 32]);

    // Construct a bundle pinning ALICE's kid (we're misusing it here)
    // and a trace claiming workspace_id = "ws-bob". Sign every event
    // under alice's kid using `sk` so signatures verify against the
    // bundle. The point is: strict mode must spot the kid <->
    // workspace_id mismatch, even though everything else is consistent.
    let bundle = bundle_with(&alice_kid, &sk);
    let bundle_hash = bundle.deterministic_hash().unwrap();
    let event = build_signed_event(
        &sk,
        "ws-bob",
        &alice_kid,
        "01H_BOB_USES_ALICE",
        "2026-04-29T10:00:00Z",
        vec![],
        serde_json::json!({"type": "node.create", "node": {"id": "leaked"}}),
    );

    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-29T10:01:00Z".to_string(),
        workspace_id: "ws-bob".to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![event.clone()],
        dag_tips: vec![event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: None,
    };

    // Lenient: signatures verify (workspace_id=ws-bob is bound into
    // the signing input and the bundle's kid agrees with what the
    // event carries). The kid<->workspace pairing is not enforced.
    //
    // We assert *both* `valid == true` AND `errors.is_empty()` AND
    // that the `event-signatures` evidence row is ok. A bare
    // `valid == true` would still pass if a future refactor silently
    // dropped the workspace_id binding from the signing input — the
    // signature would verify against the wrong-workspace input and
    // the test would stop policing the load-bearing
    // workspace-binding invariant.
    let lenient = verify_trace(&trace, &bundle);
    assert!(
        lenient.valid,
        "lenient mode does not enforce kid<->workspace pairing; errors: {:#?}",
        lenient.errors
    );
    assert!(
        lenient.errors.is_empty(),
        "lenient mode must produce zero errors here; got: {:#?}",
        lenient.errors,
    );
    assert!(
        lenient
            .evidence
            .iter()
            .any(|ev| ev.check == "event-signatures" && ev.ok),
        "lenient mode must record event-signatures evidence as ok; \
         this guards the workspace_id-bound-into-signing-input invariant. \
         got: {:#?}",
        lenient.evidence,
    );

    // Strict: the kid `atlas-anchor:ws-alice` ≠ expected
    // `atlas-anchor:ws-bob`. Reject and name the offender.
    let strict_opts = VerifyOptions {
        require_per_tenant_keys: true,
        ..VerifyOptions::default()
    };
    let strict = verify_trace_with(&trace, &bundle, &strict_opts);
    assert!(
        !strict.valid,
        "strict mode must reject cross-bundle kid reuse; outcome: {:#?}",
        strict
    );
    assert!(
        strict
            .errors
            .iter()
            .any(|e| e.contains("01H_BOB_USES_ALICE") && e.contains("atlas-anchor:ws-bob")),
        "strict error must name the offending event and the expected kid; got: {:#?}",
        strict.errors
    );
}
