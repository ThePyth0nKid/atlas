//! `atlas-signer` — server-side event signer + canonical-hash helper.
//!
//! Two subcommands:
//!
//!   sign         — Build a canonical signing-input from CLI args, sign with
//!                  Ed25519, emit a fully-formed `AtlasEvent` JSON on stdout.
//!
//!   bundle-hash  — Read a `PubkeyBundle` JSON document on stdin, emit its
//!                  blake3 deterministic-hash hex on stdout. This is the
//!                  *single* canonicalisation source for pubkey-bundle hashes
//!                  — TS and any other client must shell out here rather than
//!                  re-implement the canonical JSON format.
//!
//! Why two commands instead of two binaries? Because the trust property
//! (single source of canonicalisation) is enforced by code path, not by
//! deployment artifact. Bundling them in one binary keeps the
//! "TS-side ↔ Rust-side" boundary narrow: the MCP server resolves one
//! binary, then dispatches by subcommand.
//!
//! Secret-key handling: the `sign` subcommand prefers `--secret-stdin`
//! (read 64 hex chars from stdin) over `--secret-hex` (CLI argv). The
//! argv path is retained for backwards compatibility with the bank-demo
//! example only and emits a stderr deprecation warning. Argv secrets
//! appear in `/proc/<pid>/cmdline`, `ps aux`, and shell history; stdin
//! does not.

use atlas_trust_core::{
    cose::build_signing_input,
    hashchain::compute_event_hash,
    pubkey_bundle::PubkeyBundle,
    trace_format::{AtlasEvent, EventSignature},
};
use base64::Engine;
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signer, SigningKey};
use std::io::{self, Read};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "atlas-signer")]
#[command(version, about = "Sign Atlas events and compute canonical hashes")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    // Legacy flat-args mode: when no subcommand is given, behave like the
    // pre-subcommand binary (which exposed only `sign`). Lets the bank-demo
    // example and any external scripts continue to work without the prefix.
    #[command(flatten)]
    legacy_sign: SignArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Sign an event. (Default behaviour when no subcommand is given.)
    Sign(SignArgs),
    /// Compute the deterministic blake3 hash of a `PubkeyBundle` read from stdin.
    BundleHash,
}

#[derive(clap::Args, Default)]
struct SignArgs {
    /// Workspace identifier — bound into the signing-input to prevent
    /// cross-workspace replay. Must match the trace's `workspace_id` at
    /// verify time.
    #[arg(long)]
    workspace: Option<String>,

    /// Event ID (ULID).
    #[arg(long)]
    event_id: Option<String>,

    /// ISO-8601 timestamp.
    #[arg(long)]
    ts: Option<String>,

    /// Key-id (e.g. SPIFFE-ID).
    #[arg(long)]
    kid: Option<String>,

    /// Comma-separated parent hashes.
    #[arg(long, default_value = "")]
    parents: String,

    /// Payload as JSON string.
    #[arg(long)]
    payload: Option<String>,

    /// 32-byte hex-encoded secret key, passed via stdin (PREFERRED). If set,
    /// the signer reads exactly 64 hex chars from stdin and ignores any
    /// `--secret-hex` value.
    #[arg(long, default_value_t = false)]
    secret_stdin: bool,

    /// 32-byte hex-encoded secret key, passed via argv (DEPRECATED — leaks
    /// to OS process listing). Use `--secret-stdin` in production.
    #[arg(long, default_value = "2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a")]
    secret_hex: String,
}

fn b64url_no_pad_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Sign(args)) => run_sign(args),
        Some(Command::BundleHash) => run_bundle_hash(),
        None => run_sign(cli.legacy_sign),
    }
}

fn run_sign(args: SignArgs) -> ExitCode {
    // Required-when-signing fields (clap can't enforce because legacy mode
    // makes them all optional at the parser level). Surface clear errors.
    let workspace = match args.workspace {
        Some(v) => v,
        None => {
            eprintln!("--workspace is required");
            return ExitCode::from(2);
        }
    };
    let event_id = match args.event_id {
        Some(v) => v,
        None => {
            eprintln!("--event-id is required");
            return ExitCode::from(2);
        }
    };
    let ts = match args.ts {
        Some(v) => v,
        None => {
            eprintln!("--ts is required");
            return ExitCode::from(2);
        }
    };
    let kid = match args.kid {
        Some(v) => v,
        None => {
            eprintln!("--kid is required");
            return ExitCode::from(2);
        }
    };
    let payload_str = match args.payload {
        Some(v) => v,
        None => {
            eprintln!("--payload is required");
            return ExitCode::from(2);
        }
    };

    let payload: serde_json::Value = match serde_json::from_str(&payload_str) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("invalid payload JSON: {e}");
            return ExitCode::from(2);
        }
    };

    // Prefer stdin secret. Falls back to argv only if --secret-stdin not set.
    // The argv default (0x2A * 32) is intentionally distinct from any of
    // the bank-demo dev keys so silent fall-through to the default produces
    // signatures that fail verification immediately rather than mimicking
    // a real key.
    let secret_hex = if args.secret_stdin {
        let mut buf = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut buf) {
            eprintln!("--secret-stdin: failed to read stdin: {e}");
            return ExitCode::from(2);
        }
        buf.trim().to_string()
    } else {
        eprintln!(
            "WARNING: secret passed via --secret-hex (visible in process list). \
             Use --secret-stdin in production."
        );
        args.secret_hex
    };

    let secret_bytes = match hex::decode(&secret_hex) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => {
            eprintln!("secret must decode to 32 bytes");
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("secret invalid hex: {e}");
            return ExitCode::from(2);
        }
    };
    let secret_array: [u8; 32] = secret_bytes.try_into().expect("len-checked above");
    let signing_key = SigningKey::from_bytes(&secret_array);

    let parents: Vec<String> = if args.parents.is_empty() {
        Vec::new()
    } else {
        args.parents.split(',').map(|s| s.trim().to_string()).collect()
    };

    let signing_input = match build_signing_input(&workspace, &event_id, &ts, &kid, &parents, &payload) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("signing-input build failed: {e}");
            return ExitCode::from(2);
        }
    };

    let event_hash = compute_event_hash(&signing_input);
    let sig = signing_key.sign(&signing_input);

    let event = AtlasEvent {
        event_id,
        event_hash,
        parent_hashes: parents,
        payload,
        signature: EventSignature {
            alg: "EdDSA".to_string(),
            kid,
            sig: b64url_no_pad_encode(&sig.to_bytes()),
        },
        ts,
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

fn run_bundle_hash() -> ExitCode {
    let mut buf = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buf) {
        eprintln!("bundle-hash: failed to read stdin: {e}");
        return ExitCode::from(2);
    }
    let bundle = match PubkeyBundle::from_json(buf.as_bytes()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("bundle-hash: invalid PubkeyBundle JSON: {e}");
            return ExitCode::from(2);
        }
    };
    match bundle.deterministic_hash() {
        Ok(hex) => {
            println!("{hex}");
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("bundle-hash: deterministic_hash failed: {e}");
            return ExitCode::from(2);
        }
    }
}
