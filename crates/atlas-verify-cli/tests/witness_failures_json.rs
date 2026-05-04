//! V1.14 Scope J-c — CLI `--output json` carries `witness_failures`.
//!
//! End-to-end pin: atlas-verify-cli's JSON output is the wire that
//! external auditor tooling (TS smoke lane, BaFin/EMA/FDA dashboards)
//! consumes. The `witness_failures` array is the structured V1.14
//! Scope J surface — replacing fragile string-match against the lenient
//! evidence row's joined `detail`.
//!
//! These tests build a trace bundle on the fly (rather than checking
//! in a hand-crafted JSON fixture that would rot whenever the wire
//! schema evolves), invoke the CLI binary as a subprocess, and assert
//! the parsed output shape. They run against the real built binary,
//! so they catch wiring breakage between the `verify_trace_with`
//! return value and the CLI's `serde_json::to_string_pretty` printer
//! that a unit test in `atlas-trust-core` cannot.
//!
//! Stable contracts pinned here:
//!   * `witness_failures` field is present and is an array.
//!   * For a clean trace (no chain), the array is `[]`.
//!   * For a trace with one uncommissioned witness, the array carries
//!     `{ witness_kid, batch_index, reason_code: "kid-not-in-roster",
//!        message }` — the kebab-case `reason_code` is what auditor
//!     tooling will switch on.
//!   * Trace remains `valid: true` in lenient mode (no
//!     `--require-witness`) — `witness_failures` is diagnostic, not a
//!     verdict. Strict-mode promotion is exercised separately in
//!     `tests/strict_mode.rs`.

use atlas_trust_core::{
    chain_head_for,
    cose::build_signing_input,
    hashchain::compute_event_hash,
    witness::WitnessSig,
    AnchorBatch, AnchorChain, AnchorEntry, AtlasEvent, AtlasTrace, EventSignature,
    PubkeyBundle, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
};
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

fn cli_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_atlas-verify-cli"))
}

fn b64url_no_pad_encode(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Build a minimal valid trace + bundle pair. Optional `chain` lets
/// tests inject anchor-chain shapes (with or without witnesses).
fn build_trace_bundle(chain: Option<AnchorChain>) -> (AtlasTrace, PubkeyBundle) {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let verifying_key = signing_key.verifying_key();

    let mut keys = HashMap::new();
    keys.insert(
        "spiffe://atlas/test".to_string(),
        b64url_no_pad_encode(verifying_key.as_bytes()),
    );
    let bundle = PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-04-27T10:00:00Z".to_string(),
        keys,
    };
    let bundle_hash = bundle.deterministic_hash().unwrap();

    let event_id = "01H0CLITESTJC";
    let ts = "2026-04-27T10:00:00Z";
    let payload = serde_json::json!({"type": "node.create", "node": {"id": "n1"}});
    let signing_input =
        build_signing_input("ws-jc", event_id, ts, "spiffe://atlas/test", &[], &payload)
            .unwrap();
    let event_hash = compute_event_hash(&signing_input);
    let sig = signing_key.sign(&signing_input);

    let event = AtlasEvent {
        event_id: event_id.to_string(),
        ts: ts.to_string(),
        parent_hashes: vec![],
        payload,
        event_hash: event_hash.clone(),
        signature: EventSignature {
            alg: "EdDSA".to_string(),
            kid: "spiffe://atlas/test".to_string(),
            sig: b64url_no_pad_encode(&sig.to_bytes()),
        },
    };

    let trace = AtlasTrace {
        schema_version: atlas_trust_core::SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-27T10:01:00Z".to_string(),
        workspace_id: "ws-jc".to_string(),
        pubkey_bundle_hash: bundle_hash,
        events: vec![event.clone()],
        dag_tips: vec![event.event_hash.clone()],
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: chain,
    };
    (trace, bundle)
}

fn single_batch_chain(witnesses: Vec<WitnessSig>) -> AnchorChain {
    let batch = AnchorBatch {
        batch_index: 0,
        integrated_time: 1_745_000_000,
        entries: Vec::<AnchorEntry>::new(),
        previous_head: ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD.to_string(),
        witnesses,
    };
    let head = chain_head_for(&batch).unwrap();
    AnchorChain {
        history: vec![batch],
        head: head.into_inner(),
    }
}

fn write_pair(
    dir: &std::path::Path,
    trace: &AtlasTrace,
    bundle: &PubkeyBundle,
) -> (PathBuf, PathBuf) {
    let trace_path = dir.join("trace.json");
    let bundle_path = dir.join("pubkey-bundle.json");
    std::fs::write(&trace_path, serde_json::to_vec(trace).unwrap()).unwrap();
    std::fs::write(&bundle_path, serde_json::to_vec(bundle).unwrap()).unwrap();
    (trace_path, bundle_path)
}

fn run_cli_json(trace_path: &std::path::Path, bundle_path: &std::path::Path) -> serde_json::Value {
    let output = Command::new(cli_bin())
        .args([
            "verify-trace",
            trace_path.to_str().unwrap(),
            "--pubkey-bundle",
            bundle_path.to_str().unwrap(),
            "-o",
            "json",
        ])
        .output()
        .expect("failed to spawn atlas-verify-cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "CLI JSON output did not parse: {e}\n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stderr),
        )
    })
}

// ─────────────────────────────────────────────────────────────────────────
// J-c contracts — the wire shape auditor tooling consumes.
// ─────────────────────────────────────────────────────────────────────────

/// Trace without anchor_chain: nothing to verify on the witness path,
/// `witness_failures` MUST be present and empty in the JSON. Pins the
/// additive default — pre-J consumers that don't expect the field
/// still see a valid empty array.
#[test]
fn json_witness_failures_is_empty_array_when_no_chain() {
    let dir = tempfile::tempdir().unwrap();
    let (trace, bundle) = build_trace_bundle(None);
    let (trace_path, bundle_path) = write_pair(dir.path(), &trace, &bundle);

    let parsed = run_cli_json(&trace_path, &bundle_path);
    let arr = parsed
        .get("witness_failures")
        .expect("witness_failures field must be present");
    assert!(arr.is_array(), "witness_failures must be a JSON array");
    assert!(
        arr.as_array().unwrap().is_empty(),
        "no chain ⇒ no witness failures, got: {arr}",
    );
    assert_eq!(parsed.get("valid").and_then(|v| v.as_bool()), Some(true));
}

/// Trace with one uncommissioned witness on batch[0]. CLI JSON output
/// must surface the structured wire entry with `kid-not-in-roster`
/// kebab-case `reason_code`, the kid, and `batch_index = 0`.
#[test]
fn json_witness_failures_populated_for_uncommissioned_kid() {
    let chain = single_batch_chain(vec![WitnessSig {
        witness_kid: "uncommissioned-cli-kid".to_string(),
        signature: "A".repeat(86),
    }]);
    let dir = tempfile::tempdir().unwrap();
    let (trace, bundle) = build_trace_bundle(Some(chain));
    let (trace_path, bundle_path) = write_pair(dir.path(), &trace, &bundle);

    let parsed = run_cli_json(&trace_path, &bundle_path);

    // Trace is still VALID in lenient mode — witness_failures is
    // diagnostic, not a verdict.
    assert_eq!(
        parsed.get("valid").and_then(|v| v.as_bool()),
        Some(true),
        "lenient mode preserves valid=true even with witness failures: {parsed}",
    );

    let arr = parsed
        .get("witness_failures")
        .and_then(|v| v.as_array())
        .expect("witness_failures must be a JSON array");
    assert_eq!(arr.len(), 1, "expected one wire entry: {arr:?}");
    let entry = &arr[0];
    assert_eq!(
        entry.get("witness_kid").and_then(|v| v.as_str()),
        Some("uncommissioned-cli-kid"),
    );
    assert_eq!(
        entry.get("batch_index").and_then(|v| v.as_u64()),
        Some(0),
        "batch_index must localise the failure: {entry}",
    );
    assert_eq!(
        entry.get("reason_code").and_then(|v| v.as_str()),
        Some("kid-not-in-roster"),
        "auditor tooling switches on the kebab-case reason_code: {entry}",
    );
    let msg = entry
        .get("message")
        .and_then(|v| v.as_str())
        .expect("message field is a string");
    assert!(
        msg.contains("uncommissioned-cli-kid"),
        "message must echo the failed kid for human-readable diagnostics: {msg}",
    );
}

/// CLI `-o json` output round-trips through the public
/// `WitnessFailureWire` type. Pins that the wire schema in the binary
/// is identical to the one in `atlas-trust-core` — no hidden
/// indirection or double-encoding in the print path.
#[test]
fn json_witness_failures_round_trip_through_public_wire_type() {
    let chain = single_batch_chain(vec![WitnessSig {
        witness_kid: "round-trip-kid".to_string(),
        signature: "A".repeat(86),
    }]);
    let dir = tempfile::tempdir().unwrap();
    let (trace, bundle) = build_trace_bundle(Some(chain));
    let (trace_path, bundle_path) = write_pair(dir.path(), &trace, &bundle);

    let parsed = run_cli_json(&trace_path, &bundle_path);
    let arr_json = parsed
        .get("witness_failures")
        .expect("present")
        .to_string();
    let typed: Vec<atlas_trust_core::WitnessFailureWire> = serde_json::from_str(&arr_json)
        .expect("CLI JSON must deserialise back into Vec<WitnessFailureWire>");
    assert_eq!(typed.len(), 1);
    assert_eq!(typed[0].witness_kid, "round-trip-kid");
    assert_eq!(
        typed[0].reason_code,
        atlas_trust_core::WitnessFailureReason::KidNotInRoster,
    );
    assert_eq!(typed[0].batch_index, Some(0));
}
