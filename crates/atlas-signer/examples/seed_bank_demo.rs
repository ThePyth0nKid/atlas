//! Seed a complete Atlas trace bundle + pubkey bundle for the
//! Sebastian Meinhardt (Bank) persona-demo.
//!
//! Output:
//!   - examples/golden-traces/bank-q1-2026.trace.json
//!   - examples/golden-traces/bank-q1-2026.pubkey-bundle.json
//!
//! Run: `cargo run --example seed_bank_demo -p atlas-signer`
//!
//! After running, verify with:
//!   atlas-verify-cli verify-trace \
//!     examples/golden-traces/bank-q1-2026.trace.json \
//!     -k examples/golden-traces/bank-q1-2026.pubkey-bundle.json

use atlas_trust_core::{
    cose::build_signing_input,
    hashchain::{compute_event_hash, compute_tips},
    pubkey_bundle::PubkeyBundle,
    trace_format::{AtlasEvent, AtlasTrace, EventSignature},
    SCHEMA_VERSION,
};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const WORKSPACE: &str = "ws-bankhaus-hagedorn";
const KID_AGENT: &str = "spiffe://atlas/agent/cursor-001";
const KID_HUMAN: &str = "spiffe://atlas/human/sebastian.meinhardt@bankhaus-hagedorn.de";
const KID_ANCHOR: &str = "spiffe://atlas/system/anchor-worker";

fn b64url(b: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

fn make_event(
    workspace_id: &str,
    signer: &SigningKey,
    event_id: &str,
    ts: &str,
    kid: &str,
    parents: Vec<String>,
    payload: serde_json::Value,
) -> AtlasEvent {
    let signing_input = build_signing_input(workspace_id, event_id, ts, kid, &parents, &payload).unwrap();
    let event_hash = compute_event_hash(&signing_input);
    let sig = signer.sign(&signing_input);
    AtlasEvent {
        event_id: event_id.to_string(),
        event_hash,
        parent_hashes: parents,
        payload,
        signature: EventSignature {
            alg: "EdDSA".to_string(),
            kid: kid.to_string(),
            sig: b64url(&sig.to_bytes()),
        },
        ts: ts.to_string(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Three deterministic test keys
    let agent_key = SigningKey::from_bytes(&[0xAA; 32]);
    let human_key = SigningKey::from_bytes(&[0xBB; 32]);
    let anchor_key = SigningKey::from_bytes(&[0xCC; 32]);

    // Pubkey-bundle
    let mut keys = HashMap::new();
    keys.insert(KID_AGENT.to_string(), b64url(agent_key.verifying_key().as_bytes()));
    keys.insert(KID_HUMAN.to_string(), b64url(human_key.verifying_key().as_bytes()));
    keys.insert(KID_ANCHOR.to_string(), b64url(anchor_key.verifying_key().as_bytes()));
    let bundle = PubkeyBundle {
        schema: "atlas-pubkey-bundle-v1".to_string(),
        generated_at: "2026-01-01T00:00:00Z".to_string(),
        keys,
    };
    let bundle_hash = bundle.deterministic_hash()?;

    // Build a small DAG that tells the Sebastian story:
    //   1. Agent imports dataset
    //   2. Agent trains model
    //   3. Sebastian (human) verifies and approves
    //   4. Agent runs prediction
    //   5. Anchor-worker submits DAG-tip to Sigstore Rekor
    let ev1 = make_event(
        WORKSPACE,
        &agent_key,
        "01HRQ001IMPORT",
        "2026-01-15T09:12:00Z",
        KID_AGENT,
        vec![],
        serde_json::json!({
            "type": "node.create",
            "node": {
                "kind": "dataset",
                "id": "credit_history_q1_2026",
                "source": "s3://bankhaus-hagedorn-data/credit/q1-2026.parquet",
                "rows": 47291,
                "schema_hash": "blake3:abcd..."
            }
        }),
    );
    let ev2 = make_event(
        WORKSPACE,
        &agent_key,
        "01HRQ002TRAIN",
        "2026-01-15T11:33:00Z",
        KID_AGENT,
        vec![ev1.event_hash.clone()],
        serde_json::json!({
            "type": "node.create",
            "node": {
                "kind": "model",
                "id": "CreditScoreV3.ckpt",
                "trained_on": "credit_history_q1_2026",
                "params": 12_400_000u64,
                // Floats are not allowed in canonical CBOR payloads (RFC 8949
                // §4.2.1 determinism). Use basis-points (×10_000) for fractional
                // metrics: 0.0814 → 814 bps.
                "training_loss_bps": 814u64
            }
        }),
    );
    let ev3 = make_event(
        WORKSPACE,
        &human_key,
        "01HRQ003VERIFY",
        "2026-01-15T14:02:00Z",
        KID_HUMAN,
        vec![ev2.event_hash.clone()],
        serde_json::json!({
            "type": "annotation.add",
            "subject": "CreditScoreV3.ckpt",
            "predicate": "verified_by_human",
            "object": {
                "verifier": "Dr. Sebastian Meinhardt",
                "decision": "approved",
                "evidence": "Validation set AUC 0.91; bias check passed"
            }
        }),
    );
    let ev4 = make_event(
        WORKSPACE,
        &agent_key,
        "01HRQ004PREDICT",
        "2026-01-16T08:15:00Z",
        KID_AGENT,
        vec![ev3.event_hash.clone()],
        serde_json::json!({
            "type": "node.create",
            "node": {
                "kind": "inference",
                "id": "loan_decision_4711",
                "model": "CreditScoreV3.ckpt",
                "input_subject": "applicant-anon-4711",
                // 0.78 → 7800 bps. Same reason as training_loss_bps above.
                "score_bps": 7800u64,
                "decision": "approved"
            }
        }),
    );
    let ev5 = make_event(
        WORKSPACE,
        &anchor_key,
        "01HRQ005ANCHOR",
        "2026-01-16T09:00:00Z",
        KID_ANCHOR,
        vec![ev4.event_hash.clone()],
        serde_json::json!({
            "type": "anchor.created",
            "rekor_uuid": "24296fb24b8ad77a3b14fb71e8a1e7c45c4b76e1d3e8a7c1b4e6f9a2",
            "rekor_proof": "<base64-omitted-for-V1-stub>"
        }),
    );

    let events = vec![ev1, ev2, ev3, ev4, ev5];
    let tips = compute_tips(&events);

    let trace = AtlasTrace {
        schema_version: SCHEMA_VERSION.to_string(),
        generated_at: "2026-04-27T10:30:00Z".to_string(),
        workspace_id: WORKSPACE.to_string(),
        pubkey_bundle_hash: bundle_hash,
        events,
        dag_tips: tips,
        anchors: vec![],
        policies: vec![],
        filters: None,
        anchor_chain: None,
    };

    // Resolve repo-root (workspace root)
    let manifest = std::env::var("CARGO_MANIFEST_DIR")?;
    let repo_root = PathBuf::from(manifest)
        .parent()
        .and_then(|p| p.parent())
        .ok_or("could not resolve repo root")?
        .to_path_buf();
    let out_dir = repo_root.join("examples").join("golden-traces");
    fs::create_dir_all(&out_dir)?;

    let trace_path = out_dir.join("bank-q1-2026.trace.json");
    let bundle_path = out_dir.join("bank-q1-2026.pubkey-bundle.json");

    fs::write(&trace_path, serde_json::to_vec_pretty(&trace)?)?;
    fs::write(&bundle_path, serde_json::to_vec_pretty(&bundle)?)?;

    println!("wrote {}", trace_path.display());
    println!("wrote {}", bundle_path.display());

    Ok(())
}
