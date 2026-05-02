//! `atlas-signer` — server-side event signer + canonical-hash helper.
//!
//! Subcommands:
//!
//!   sign                  — Build a canonical signing-input from CLI args,
//!                           sign with Ed25519, emit a fully-formed
//!                           `AtlasEvent` JSON on stdout.
//!
//!   bundle-hash           — Read a `PubkeyBundle` JSON document on stdin,
//!                           emit its blake3 deterministic-hash hex on
//!                           stdout. The *single* canonicalisation source
//!                           for pubkey-bundle hashes — TS and any other
//!                           client must shell out here rather than
//!                           re-implement the canonical JSON format.
//!
//!   anchor                — Read an `AnchorBatchInput` JSON document on
//!                           stdin, build a Merkle tree, sign the
//!                           checkpoint with the dev mock-Rekor key, emit
//!                           `[AnchorEntry]` JSON on stdout. V1.6 swaps
//!                           the mock issuer for a real Rekor POST behind
//!                           `--rekor-url`.
//!
//!   chain-export          — Read a workspace's `anchor-chain.jsonl`
//!                           content on stdin, recompute the chain head
//!                           via `chain_head_for`, validate the full
//!                           chain via `verify_anchor_chain`, and emit a
//!                           wire-format `AnchorChain { history, head }`
//!                           JSON document on stdout. Single source of
//!                           canonicalisation for the chain head — the
//!                           MCP TS-side never recomputes heads.
//!
//!   derive-key            — V1.9. Derive the per-tenant Ed25519
//!                           identity for `--workspace` from the master
//!                           seed via HKDF-SHA256. Emits the canonical
//!                           kid (`atlas-anchor:{workspace}`), the
//!                           base64url-no-pad public key, AND the hex
//!                           secret on stdout as JSON. Use sparingly —
//!                           this is the only path where the derived
//!                           secret crosses the signer process boundary,
//!                           so it should be reserved for ceremonies
//!                           (key inspection, manual `sign --secret-stdin`
//!                           drives) rather than the hot write path.
//!
//!   derive-pubkey         — V1.9. Same derivation as `derive-key` but
//!                           emits ONLY {kid, pubkey_b64url} — the secret
//!                           never leaves the signer. The MCP server
//!                           uses this to assemble per-workspace
//!                           PubkeyBundles without ever materialising
//!                           the workspace's signing key in TS heap.
//!
//!   rotate-pubkey-bundle  — V1.9. Read a `PubkeyBundle` on stdin, add
//!                           the per-tenant kid + pubkey for the named
//!                           workspace via HKDF derivation, emit the
//!                           updated bundle on stdout. Idempotent: a
//!                           re-run on an already-rotated bundle returns
//!                           the bundle unchanged. Operator ceremony for
//!                           upgrading legacy V1.5–V1.8 bundles in
//!                           place; the legacy SPIFFE kids are
//!                           preserved so old traces continue to verify
//!                           in lenient mode. The signer reads from stdin
//!                           and writes to stdout — atomic file replace
//!                           and inter-operator concurrency are the
//!                           caller's responsibility (see OPERATOR-RUNBOOK).
//!
//! Why two commands instead of two binaries? Because the trust property
//! (single source of canonicalisation) is enforced by code path, not by
//! deployment artifact. Bundling them in one binary keeps the
//! "TS-side ↔ Rust-side" boundary narrow: the MCP server resolves one
//! binary, then dispatches by subcommand.
//!
//! Secret-key handling: the `sign` subcommand has three secret-source
//! modes, each mutually exclusive:
//!
//!   * `--secret-stdin` — read 64 hex chars from stdin (preferred for
//!     legacy SPIFFE kids on the production path).
//!   * `--secret-hex <hex>` — argv path. DEPRECATED — argv values
//!     appear in `/proc/<pid>/cmdline`, `ps aux`, and shell history.
//!     Retained only for the bank-demo example.
//!   * `--derive-from-workspace <ws>` — V1.9. The signer derives the
//!     per-tenant secret internally via HKDF and signs without ever
//!     emitting the secret. This is the hot path for V1.9 per-tenant
//!     events: the MCP server passes the workspace_id and the kid; no
//!     secret material crosses the subprocess boundary.
//!
//! Exactly one of the three must be supplied — silent fall-through to a
//! built-in default is refused (V1.8 used a 0x2A-byte sentinel default
//! for `--secret-hex`; V1.9 retires it because the resulting "valid
//! signature, but with the wrong key" outcome was a footgun in CI logs).
//!
//! V1.9 production gate: the per-tenant subcommands (`derive-key`,
//! `derive-pubkey`, `rotate-pubkey-bundle`, `sign --derive-from-workspace`)
//! refuse to run when `ATLAS_PRODUCTION=1` because the master seed is
//! a source-committed dev constant. V1.10 will replace the gate with a
//! sealed-seed loader.

// V1.10: the binary consumes `atlas-signer` as a library so the
// V1.10 wave-2 sealed-seed loader (`atlas_signer::hsm`) shares the
// [`keys::MasterSeedHkdf`] trait surface with the dev impl. The
// library entry point is `src/lib.rs`; this binary is a thin CLI
// wrapper. No behaviour change vs. V1.9 — the modules below are the
// same code, re-rooted from `crate::keys::` to `atlas_signer::keys::`.
use atlas_signer::{anchor, chain, keys};

use atlas_trust_core::{
    cose::build_signing_input,
    hashchain::compute_event_hash,
    pubkey_bundle::PubkeyBundle,
    trace_format::{AtlasEvent, EventSignature},
};
use base64::Engine;
use clap::{Parser, Subcommand};
use ed25519_dalek::{Signer, SigningKey};
use serde::Serialize;
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
    /// Build a Merkle inclusion proof + signed checkpoint for a batch of
    /// hashes (read from stdin as `AnchorBatchInput` JSON), emit
    /// `[AnchorEntry]` JSON on stdout. Uses the dev mock-Rekor key when
    /// `--rekor-url` is unset; otherwise POSTs to the named Rekor
    /// instance (Sigstore v1, hashedrekord/v0.0.1).
    Anchor(AnchorArgs),
    /// Export a workspace's `anchor-chain.jsonl` (read from stdin) as a
    /// validated wire-format `AnchorChain` JSON document on stdout. The
    /// MCP server uses this to populate `AtlasTrace.anchor_chain`
    /// without re-implementing the canonical-bytes path the verifier
    /// uses for `chain_head_for`.
    ChainExport,
    /// V1.9: Derive the per-tenant Ed25519 identity for a workspace
    /// from the master seed via HKDF-SHA256. Emits {kid,
    /// pubkey_b64url, secret_hex} JSON on stdout. Use only for
    /// ceremonies; routine signing should use `sign
    /// --derive-from-workspace`, which does not expose the secret.
    DeriveKey(DeriveWorkspaceArgs),
    /// V1.9: Same derivation as `derive-key` but emits only {kid,
    /// pubkey_b64url} — the secret never leaves this process. Used by
    /// the MCP server to assemble per-workspace `PubkeyBundle`s.
    DerivePubkey(DeriveWorkspaceArgs),
    /// V1.9: Add the per-tenant kid + pubkey for `--workspace` to a
    /// `PubkeyBundle` read from stdin, emit the updated bundle on
    /// stdout. Idempotent.
    RotatePubkeyBundle(RotatePubkeyBundleArgs),
}

#[derive(clap::Args)]
struct DeriveWorkspaceArgs {
    /// Workspace identifier — bound into the HKDF info parameter as
    /// `"atlas-anchor-v1:{workspace}"`. Two workspaces with different
    /// IDs derive independent keypairs from the same master seed.
    #[arg(long)]
    workspace: String,
}

#[derive(clap::Args)]
struct RotatePubkeyBundleArgs {
    /// Workspace identifier whose per-tenant kid should be added to
    /// the bundle. Re-running with an already-rotated bundle is a
    /// no-op (the existing pubkey is asserted to match the
    /// derivation, then returned as-is).
    #[arg(long)]
    workspace: String,
}

#[derive(clap::Args, Default)]
struct AnchorArgs {
    /// Rekor base URL (e.g. `https://rekor.sigstore.dev`). When set,
    /// the anchor subcommand POSTs each batch item to
    /// `<url>/api/v1/log/entries` and produces Sigstore-format
    /// `AnchorEntry` rows. When unset, the in-process mock-Rekor
    /// issuer runs unchanged.
    #[arg(long)]
    rekor_url: Option<String>,

    /// V1.7 anchor-chain file (`anchor-chain.jsonl`). When set, the
    /// signer reads the existing chain, builds a new `AnchorBatch`
    /// committing the freshly-issued entries plus `integrated_time`,
    /// and atomically appends one row. The signer is the SOLE writer
    /// for this file; the MCP server reads it but never modifies it.
    /// Stdout shape (`[AnchorEntry]`) is unchanged for backward compat.
    #[arg(long)]
    chain_path: Option<std::path::PathBuf>,
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

    /// 32-byte hex-encoded secret key, passed via stdin (PREFERRED).
    /// If set, the signer reads exactly 64 hex chars from stdin.
    #[arg(long, default_value_t = false)]
    secret_stdin: bool,

    /// 32-byte hex-encoded secret key, passed via argv (DEPRECATED —
    /// leaks to OS process listing). Use `--secret-stdin` for legacy
    /// SPIFFE kids and `--derive-from-workspace` for per-tenant kids.
    #[arg(long)]
    secret_hex: Option<String>,

    /// V1.9: derive the per-tenant Ed25519 secret internally for the
    /// named workspace and sign with it. Mutually exclusive with
    /// `--secret-stdin` and `--secret-hex`. The derived secret never
    /// crosses the subprocess boundary; this is the V1.9 hot path for
    /// per-tenant events.
    #[arg(long)]
    derive_from_workspace: Option<String>,
}

fn b64url_no_pad_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Sign(args)) => run_sign(args),
        Some(Command::BundleHash) => run_bundle_hash(),
        Some(Command::Anchor(args)) => run_anchor(args),
        Some(Command::ChainExport) => run_chain_export(),
        Some(Command::DeriveKey(args)) => run_derive_key(args),
        Some(Command::DerivePubkey(args)) => run_derive_pubkey(args),
        Some(Command::RotatePubkeyBundle(args)) => run_rotate_pubkey_bundle(args),
        None => run_sign(cli.legacy_sign),
    }
}

/// JSON shape emitted by `derive-key`. Mirrors `keys::PerTenantIdentity`
/// with `serde_json` derive — kept distinct from the Rust struct so the
/// wire format is locked even if the internal type evolves.
#[derive(Serialize)]
struct DeriveKeyOutput {
    /// Canonical per-tenant kid (`atlas-anchor:{workspace}`).
    kid: String,
    /// 32-byte Ed25519 pubkey, base64url-no-pad.
    pubkey_b64url: String,
    /// 32-byte secret, hex (64 chars). The caller pipes this back via
    /// `sign --secret-stdin` — never via argv.
    secret_hex: String,
}

fn run_derive_key(args: DeriveWorkspaceArgs) -> ExitCode {
    let hkdf = match keys::master_seed_loader() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("derive-key: {e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = keys::validate_workspace_id(&args.workspace) {
        eprintln!("derive-key: invalid --workspace: {e}");
        return ExitCode::from(2);
    }
    let identity = match keys::per_tenant_identity_via(&*hkdf, &args.workspace) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("derive-key: per-tenant derive failed: {e}");
            return ExitCode::from(2);
        }
    };
    let output = DeriveKeyOutput {
        kid: identity.kid,
        pubkey_b64url: identity.pubkey_b64url,
        secret_hex: identity.secret_hex,
    };
    match serde_json::to_string_pretty(&output) {
        Ok(s) => {
            println!("{s}");
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("derive-key: emit failed: {e}");
            ExitCode::from(2)
        }
    }
}

/// JSON shape emitted by `derive-pubkey`. Distinct from `DeriveKeyOutput`
/// because the secret intentionally never leaves this process — the wire
/// format omits `secret_hex` so a future `--include-secret` flag would
/// have to add it explicitly rather than the schema growing it silently.
#[derive(Serialize)]
struct DerivePubkeyOutput {
    kid: String,
    pubkey_b64url: String,
}

/// V1.9: Same HKDF derivation as `run_derive_key` but emits only the
/// public material. The MCP server uses this to assemble per-workspace
/// `PubkeyBundle`s without ever materialising the workspace's signing
/// key in TS heap.
fn run_derive_pubkey(args: DeriveWorkspaceArgs) -> ExitCode {
    let hkdf = match keys::master_seed_loader() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("derive-pubkey: {e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = keys::validate_workspace_id(&args.workspace) {
        eprintln!("derive-pubkey: invalid --workspace: {e}");
        return ExitCode::from(2);
    }
    let identity = match keys::per_tenant_identity_via(&*hkdf, &args.workspace) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("derive-pubkey: per-tenant derive failed: {e}");
            return ExitCode::from(2);
        }
    };
    let output = DerivePubkeyOutput {
        kid: identity.kid,
        pubkey_b64url: identity.pubkey_b64url,
    };
    match serde_json::to_string_pretty(&output) {
        Ok(s) => {
            println!("{s}");
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("derive-pubkey: emit failed: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_rotate_pubkey_bundle(args: RotatePubkeyBundleArgs) -> ExitCode {
    let hkdf = match keys::master_seed_loader() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("rotate-pubkey-bundle: {e}");
            return ExitCode::from(2);
        }
    };
    if let Err(e) = keys::validate_workspace_id(&args.workspace) {
        eprintln!("rotate-pubkey-bundle: invalid --workspace: {e}");
        return ExitCode::from(2);
    }
    let mut buf = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buf) {
        eprintln!("rotate-pubkey-bundle: failed to read stdin: {e}");
        return ExitCode::from(2);
    }
    let mut bundle = match PubkeyBundle::from_json(buf.as_bytes()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("rotate-pubkey-bundle: invalid PubkeyBundle JSON: {e}");
            return ExitCode::from(2);
        }
    };

    let identity = match keys::per_tenant_identity_via(&*hkdf, &args.workspace) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("rotate-pubkey-bundle: per-tenant derive failed: {e}");
            return ExitCode::from(2);
        }
    };

    // Idempotency: if the kid already exists, the existing pubkey MUST
    // match what the derivation produces. A mismatch means either the
    // master seed has rotated (operator must use a fresh bundle) or the
    // bundle was tampered with. Either way, refuse to silently overwrite.
    //
    // The match-case is also reported to stderr so an operator running
    // the rotation a second time gets a clear "no-op" signal instead of
    // a silent success. Without this, "I re-ran rotate but nothing
    // changed" is indistinguishable from "I re-ran rotate and it
    // overwrote my edits" at the terminal.
    let already_present = match bundle.keys.get(&identity.kid) {
        Some(existing) if existing == &identity.pubkey_b64url => true,
        Some(existing) => {
            eprintln!(
                "rotate-pubkey-bundle: kid {} already present with a DIFFERENT pubkey \
                 (have={}, derived={}). The master seed may have rotated; refusing to \
                 silently overwrite.",
                identity.kid, existing, identity.pubkey_b64url,
            );
            return ExitCode::from(2);
        }
        None => false,
    };

    if already_present {
        eprintln!(
            "rotate-pubkey-bundle: no-op — kid {} already present with the derived pubkey",
            identity.kid,
        );
    } else {
        eprintln!(
            "rotate-pubkey-bundle: added kid {} (pubkey {})",
            identity.kid, identity.pubkey_b64url,
        );
        bundle
            .keys
            .insert(identity.kid.clone(), identity.pubkey_b64url.clone());
    }

    match serde_json::to_string_pretty(&bundle) {
        Ok(s) => {
            println!("{s}");
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("rotate-pubkey-bundle: emit failed: {e}");
            ExitCode::from(2)
        }
    }
}

fn run_anchor(args: AnchorArgs) -> ExitCode {
    let mut buf = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buf) {
        eprintln!("anchor: failed to read stdin: {e}");
        return ExitCode::from(2);
    }
    let batch: anchor::AnchorBatchInput = match serde_json::from_str(&buf) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("anchor: invalid AnchorBatchInput JSON: {e}");
            return ExitCode::from(2);
        }
    };
    // Capture the integrated_time before `batch` is consumed by the
    // issuer; the chain extension below threads the same value into
    // the new AnchorBatch so the on-disk row matches every entry's
    // own integrated_time.
    let integrated_time = batch.integrated_time;
    // Dispatch: live Rekor when `--rekor-url` is set, otherwise the
    // in-process mock issuer. The two paths produce mutually-distinct
    // AnchorEntry shapes (entry_body_b64, tree_id are Some for the
    // Sigstore path, None for mock); the verifier dispatches by
    // log_id so both shapes flow through `verify_anchor_entry`
    // unchanged.
    let entries = match args.rekor_url.as_deref() {
        Some(url) => match anchor::issue_anchors_via_rekor(batch, url) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("anchor: rekor issue failed: {e}");
                return ExitCode::from(2);
            }
        },
        None => match anchor::issue_anchors(batch) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("anchor: issue failed: {e}");
                return ExitCode::from(2);
            }
        },
    };

    // V1.7: if --chain-path is set, atomically append a new AnchorBatch
    // row committing these entries. We extend AFTER successful issuance
    // so a Rekor failure does not bind a phantom batch to the chain.
    // The signer is the sole writer for this file.
    if let Some(chain_path) = args.chain_path.as_deref() {
        match chain::extend_chain_with_batch(chain_path, &entries, integrated_time) {
            Ok(new_batch) => {
                eprintln!(
                    "anchor: extended chain at {} (batch_index={}, previous_head={}, entries={})",
                    chain_path.display(),
                    new_batch.batch_index,
                    new_batch.previous_head,
                    new_batch.entries.len(),
                );
            }
            Err(e) => {
                eprintln!("anchor: chain extension failed: {e}");
                return ExitCode::from(2);
            }
        }
    }

    match serde_json::to_string_pretty(&entries) {
        Ok(s) => {
            println!("{s}");
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("anchor: emit failed: {e}");
            ExitCode::from(2)
        }
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

    // V1.9: 3-way exclusive secret-source selection. Exactly one of
    // `--derive-from-workspace`, `--secret-stdin`, or `--secret-hex` must
    // be set. Silent fall-through to a built-in default is refused — V1.8
    // had a 0x2A-byte sentinel default which produced "valid signature
    // under the wrong key" outcomes that masked CI bugs.
    let mode_count = [
        args.derive_from_workspace.is_some(),
        args.secret_stdin,
        args.secret_hex.is_some(),
    ]
    .iter()
    .filter(|&&b| b)
    .count();
    if mode_count != 1 {
        eprintln!(
            "sign: exactly one secret source required, got {mode_count}. Use one of \
             --derive-from-workspace=<ws> (V1.9 per-tenant), --secret-stdin (legacy SPIFFE \
             kids on production), or --secret-hex=<hex> (DEPRECATED, dev only)."
        );
        return ExitCode::from(2);
    }

    let signing_key: SigningKey = if let Some(ws) = args.derive_from_workspace.as_deref() {
        // In-signer derivation: the secret never crosses the subprocess
        // boundary. This is the V1.9 hot path for per-tenant events.
        // V1.10 wave 2: master_seed_loader dispatches to PKCS#11 when
        // the HSM trio is set, otherwise falls through to the V1.10
        // wave-1 dev-seed gate.
        let hkdf = match keys::master_seed_loader() {
            Ok(h) => h,
            Err(e) => {
                eprintln!("sign --derive-from-workspace: {e}");
                return ExitCode::from(2);
            }
        };
        if let Err(e) = keys::validate_workspace_id(ws) {
            eprintln!("sign --derive-from-workspace: invalid workspace: {e}");
            return ExitCode::from(2);
        }
        // Defence-in-depth: the kid claimed in --kid must match the kid
        // the verifier will recompute from `trace.workspace_id`. If the
        // caller passes a per-tenant workspace but a legacy kid (or a
        // per-tenant kid for a different workspace), the resulting event
        // would silently fail strict-mode verification much later. Catch
        // it here.
        let expected_kid = format!("atlas-anchor:{ws}");
        if kid != expected_kid {
            eprintln!(
                "sign --derive-from-workspace={ws}: --kid {kid:?} does not match the \
                 derived per-tenant kid {expected_kid:?}. Pass --kid {expected_kid:?} \
                 (or use --secret-stdin for legacy SPIFFE kids)."
            );
            return ExitCode::from(2);
        }
        match keys::derive_workspace_signing_key_via(&*hkdf, ws) {
            Ok(sk) => sk,
            Err(e) => {
                eprintln!("sign --derive-from-workspace: per-tenant derive failed: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
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
            args.secret_hex.expect("mode_count check guarantees exactly one source")
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
        SigningKey::from_bytes(&secret_array)
    };

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

fn run_chain_export() -> ExitCode {
    // Buffer the JSONL into memory rather than streaming. A workspace's
    // chain is dozens to hundreds of batches, each a few KB — well
    // within memory comfort. Streaming would force partial-line
    // recovery semantics across stdin EOF, which is more error-prone
    // than just reading the whole thing.
    let mut buf = Vec::new();
    if let Err(e) = io::stdin().read_to_end(&mut buf) {
        eprintln!("chain-export: failed to read stdin: {e}");
        return ExitCode::from(2);
    }
    let chain = match chain::build_chain_export_from_jsonl(&buf) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("chain-export: {e}");
            return ExitCode::from(2);
        }
    };
    match serde_json::to_string_pretty(&chain) {
        Ok(s) => {
            println!("{s}");
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("chain-export: emit failed: {e}");
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
            ExitCode::from(2)
        }
    }
}
