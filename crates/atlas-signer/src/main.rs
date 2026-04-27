//! `atlas-signer` — server-side event signer.
//!
//! V1 stub: takes a JSON payload + signing-key + parent-hashes from stdin/CLI args,
//! emits a fully-signed `AtlasEvent` JSON.
//!
//! In production this will be a long-running HTTP service the MCP-server talks to,
//! with the key sealed in TPM/HSM. For now: pure CLI for testing the round-trip.

use atlas_trust_core::{
    cose::build_signing_input,
    hashchain::compute_event_hash,
    trace_format::{AtlasEvent, EventSignature},
};
use base64::Engine;
use clap::Parser;
use ed25519_dalek::{Signer, SigningKey};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "atlas-signer")]
#[command(version, about = "Sign an Atlas event with a deterministic test keypair", long_about = None)]
struct Cli {
    /// Workspace identifier — bound into the signing-input to prevent
    /// cross-workspace replay. Must match the trace's `workspace_id` at
    /// verify time.
    #[arg(long)]
    workspace: String,

    /// Event ID (ULID).
    #[arg(long)]
    event_id: String,

    /// ISO-8601 timestamp.
    #[arg(long)]
    ts: String,

    /// Key-id (e.g. SPIFFE-ID).
    #[arg(long)]
    kid: String,

    /// Comma-separated parent hashes.
    #[arg(long, default_value = "")]
    parents: String,

    /// Payload as JSON string.
    #[arg(long)]
    payload: String,

    /// 32-byte hex-encoded secret key.
    /// In production this comes from TPM/HSM; for V1 stub we accept it inline.
    #[arg(long, default_value = "2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a")]
    secret_hex: String,
}

fn b64url_no_pad_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let payload: serde_json::Value = match serde_json::from_str(&cli.payload) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("invalid payload JSON: {e}");
            return ExitCode::from(2);
        }
    };

    let secret_bytes = match hex::decode(&cli.secret_hex) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => {
            eprintln!("--secret-hex must decode to 32 bytes");
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("--secret-hex invalid: {e}");
            return ExitCode::from(2);
        }
    };
    let secret_array: [u8; 32] = secret_bytes.try_into().expect("len-checked above");
    let signing_key = SigningKey::from_bytes(&secret_array);

    let parents: Vec<String> = if cli.parents.is_empty() {
        Vec::new()
    } else {
        cli.parents.split(',').map(|s| s.trim().to_string()).collect()
    };

    let signing_input = match build_signing_input(&cli.workspace, &cli.event_id, &cli.ts, &cli.kid, &parents, &payload) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("signing-input build failed: {e}");
            return ExitCode::from(2);
        }
    };

    let event_hash = compute_event_hash(&signing_input);
    let sig = signing_key.sign(&signing_input);

    let event = AtlasEvent {
        event_id: cli.event_id,
        event_hash,
        parent_hashes: parents,
        payload,
        signature: EventSignature {
            alg: "EdDSA".to_string(),
            kid: cli.kid,
            sig: b64url_no_pad_encode(&sig.to_bytes()),
        },
        ts: cli.ts,
    };

    match serde_json::to_string_pretty(&event) {
        Ok(s) => {
            println!("{s}");
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("emit error: {e}");
            ExitCode::from(2)
        }
    }
}
