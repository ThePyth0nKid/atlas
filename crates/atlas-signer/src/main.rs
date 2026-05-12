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
//! V1.10 master-seed gate (shipped, two waves):
//!   * **Wave 1 — positive opt-in.** Per-tenant subcommands
//!     (`derive-key`, `derive-pubkey`, `rotate-pubkey-bundle`,
//!     `sign --derive-from-workspace`) refuse to start unless the
//!     operator sets `ATLAS_DEV_MASTER_SEED=1` (truthy values:
//!     `1`/`true`/`yes`/`on`, case-insensitive). V1.12 removed the
//!     V1.9-era `ATLAS_PRODUCTION` paranoia layer — the positive
//!     opt-in is now the sole dev-seed gate, and the wave-2 HSM trio
//!     is the production audit signal.
//!   * **Wave 2 — sealed-seed loader.** Setting the HSM trio
//!     (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`)
//!     dispatches to [`atlas_signer::hsm::pkcs11::Pkcs11MasterSeedHkdf`]
//!     (gated behind the `hsm` Cargo feature). HKDF runs *inside* the
//!     HSM token via `CKM_HKDF_DERIVE`; the master seed never enters
//!     Atlas address space. HSM init failure is fatal — there is no
//!     silent fallback to the dev seed when the trio is set. See
//!     `docs/OPERATOR-RUNBOOK.md` §2 for the import ceremony.

// V1.10: the binary consumes `atlas-signer` as a library so the
// V1.10 wave-2 sealed-seed loader (`atlas_signer::hsm`) shares the
// [`keys::MasterSeedHkdf`] trait surface with the dev impl. The
// library entry point is `src/lib.rs`; this binary is a thin CLI
// wrapper. No behaviour change vs. V1.9 — the modules below are the
// same code, re-rooted from `crate::keys::` to `atlas_signer::keys::`.
use atlas_signer::workspace_signer::{
    per_tenant_identity_via_signer, workspace_signer_loader, WorkspaceSigner,
    WORKSPACE_HSM_OPT_IN_ENV,
};
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
use std::fmt::Write as _;
use std::io::{self, Read};
use std::process::ExitCode;
use zeroize::Zeroizing;

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
        Some(Command::Sign(args)) => run_sign_dispatch(args),
        Some(Command::BundleHash) => run_bundle_hash(),
        Some(Command::Anchor(args)) => run_anchor(args),
        Some(Command::ChainExport) => run_chain_export(),
        Some(Command::DeriveKey(args)) => run_derive_key_or_refuse(args),
        Some(Command::DerivePubkey(args)) => with_workspace_signer("derive-pubkey", |signer| {
            run_derive_pubkey(args, signer)
        }),
        Some(Command::RotatePubkeyBundle(args)) => {
            with_workspace_signer("rotate-pubkey-bundle", |signer| {
                run_rotate_pubkey_bundle(args, signer)
            })
        }
        None => run_sign_dispatch(cli.legacy_sign),
    }
}

/// V1.11 M-4 — single-load helper for per-tenant subcommands.
///
/// V1.10 wave 2 dispatched four subcommands (`derive-key`,
/// `derive-pubkey`, `rotate-pubkey-bundle`, and `sign
/// --derive-from-workspace`), each of which independently called
/// [`keys::master_seed_loader`]. In dev mode that's free; in HSM mode
/// each call is a `Pkcs11::new` + `C_Initialize` + `C_OpenSession` +
/// `C_Login` + `C_FindObjects` chain — and `C_Login` is an audit event
/// on commercial HSMs (Thales Luna, AWS CloudHSM, YubiHSM2 all
/// timestamp every login). Today the binary is one-shot per
/// subcommand so the per-process cost is bounded, but the future
/// MCP-embedded form will share one loader across many signing calls.
///
/// This helper formalises the "single load per process" invariant in
/// the API surface: per-tenant handlers receive a borrowed
/// `&dyn MasterSeedHkdf` rather than each calling the loader. Today
/// the helper still loads on demand (per subcommand invocation) — the
/// behavioural change lands in the V1.11 MCP-embedding work — but the
/// signature change is the one-shot now-and-future fix.
///
/// Error wrapping preserves the V1.10 per-subcommand `cmd: msg` prefix
/// so existing operator log-grep patterns continue to work.
fn with_master_seed<F>(cmd: &str, handler: F) -> ExitCode
where
    F: FnOnce(&dyn keys::MasterSeedHkdf) -> ExitCode,
{
    match keys::master_seed_loader() {
        Ok(hkdf) => handler(&*hkdf),
        Err(e) => {
            eprintln!("{cmd}: {e}");
            ExitCode::from(2)
        }
    }
}

/// V1.11 wave-3 Phase C — wave-3-aware sibling of [`with_master_seed`].
///
/// Loads a [`WorkspaceSigner`] via [`workspace_signer_loader`] (which
/// either returns the sealed-key
/// [`Pkcs11WorkspaceSigner`](atlas_signer::hsm::pkcs11_workspace::Pkcs11WorkspaceSigner)
/// when wave-3 is opted in, or a [`DevWorkspaceSigner`](atlas_signer::workspace_signer::DevWorkspaceSigner)
/// over the wave-2 / dev master-seed loader otherwise) and hands it to
/// the per-subcommand handler.
///
/// This is the call site for `sign --derive-from-workspace`,
/// `derive-pubkey`, and `rotate-pubkey-bundle` — every subcommand that
/// produces a SIGNATURE or PUBKEY must route through here so wave-3
/// deployments emit the sealed key's actual material rather than a
/// stale HKDF-derived shadow. The `derive-key` ceremony (which emits
/// the SECRET hex) goes through [`with_master_seed`] instead because
/// wave-3 sealed keys are unexportable by design — see
/// [`run_derive_key_or_refuse`] for the wave-3 incompatibility check.
fn with_workspace_signer<F>(cmd: &str, handler: F) -> ExitCode
where
    F: FnOnce(&dyn WorkspaceSigner) -> ExitCode,
{
    match workspace_signer_loader() {
        Ok(signer) => handler(&*signer),
        Err(e) => {
            eprintln!("{cmd}: {e}");
            ExitCode::from(2)
        }
    }
}

/// V1.11 M-4 + wave-3 Phase C — `sign` dispatcher that conditionally
/// loads a [`WorkspaceSigner`] only when the caller requested in-signer
/// derivation (`--derive-from-workspace`). Legacy `--secret-stdin` /
/// `--secret-hex` paths must NOT trigger the loader: they have no
/// dependency on the master seed (or the wave-3 sealed key store) and
/// must remain usable in CI/dev environments that haven't opted into
/// the V1.10 master-seed gate or the wave-3 HSM workspace signer.
///
/// **V1.11 Scope A wave-3.** The loader now routes through
/// [`workspace_signer_loader`] (instead of [`keys::master_seed_loader`]
/// directly) so a deployment that opted into wave-3 via
/// [`atlas_signer::workspace_signer::WORKSPACE_HSM_OPT_IN_ENV`] gets the
/// sealed [`Pkcs11WorkspaceSigner`](atlas_signer::hsm::pkcs11_workspace::Pkcs11WorkspaceSigner)
/// and signs entirely inside the HSM. A deployment that did NOT opt in
/// transparently falls through to a [`DevWorkspaceSigner`](atlas_signer::workspace_signer::DevWorkspaceSigner)
/// over the wave-2 / dev master seed — byte-equivalent to V1.10 (the
/// `pubkey_matches_v1_9_derivation_for_dev_seed` golden in
/// `workspace_signer.rs::tests` is the regression fence).
///
/// Sharing the dispatcher between the explicit `Command::Sign(_)` arm
/// and the legacy no-subcommand fall-through (used by the bank-demo
/// example) avoids duplicating the conditional-load block.
fn run_sign_dispatch(args: SignArgs) -> ExitCode {
    let signer = if args.derive_from_workspace.is_some() {
        match workspace_signer_loader() {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("sign --derive-from-workspace: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    // `as_deref()` peels `Option<Box<dyn WorkspaceSigner>>` to
    // `Option<&dyn WorkspaceSigner>` — a reference, not a move, so the
    // boxed loader stays alive for the call duration and drops at
    // function return. Important for the wave-3 path: the
    // `Pkcs11WorkspaceSigner` holds the PKCS#11 session + key handle
    // cache; dropping it triggers `C_CloseSession` and (eventually)
    // `C_Finalize`, so we want exactly one drop point per process.
    run_sign(args, signer.as_deref())
}

/// V1.11 W1 (H-1): emit the `derive-key` ceremony JSON without
/// allocating an unprotected `String` copy of the secret.
///
/// V1.10 used a `#[derive(Serialize)] struct DeriveKeyOutput { ...,
/// secret_hex: String }` and `serde_json::to_string_pretty(&output)`,
/// which copied the 64-char hex string through serde's tree as
/// unprotected `String` allocations. V1.11 eliminates the struct and
/// builds the JSON document directly in a [`Zeroizing<String>`]
/// buffer:
///
///   * `kid` and `pubkey_b64url` are non-sensitive (already-public
///     material). They route through `serde_json::to_string` for
///     correct JSON-escape handling.
///   * `secret_hex` borrows from the caller's [`Zeroizing<String>`]
///     wrapper via `&**secret_hex` and is injected directly. The hex
///     output of [`hex::encode`] cannot contain characters that would
///     require JSON-escape (it is `[0-9a-f]+` only), so the literal
///     string injection is safe.
///   * The full JSON document lives in a second
///     `Zeroizing<String>` buffer that scrubs on drop, immediately
///     after `println!` writes it to stdout.
///
/// The function returns the assembled buffer so the caller controls
/// its drop scope. A caller that holds the buffer beyond the
/// `println!` call extends the heap-residency window of the secret;
/// `run_derive_key` drops it as soon as the write completes.
fn build_derive_key_json(
    identity: &keys::PerTenantIdentity,
    secret_hex: &Zeroizing<String>,
) -> Result<Zeroizing<String>, serde_json::Error> {
    let kid_json = serde_json::to_string(&identity.kid)?;
    let pubkey_json = serde_json::to_string(&identity.pubkey_b64url)?;
    let mut buf: Zeroizing<String> = Zeroizing::new(String::with_capacity(256));
    // `write!` into a `String` is infallible; the inner `String` impl
    // of `std::fmt::Write` cannot fail. The `expect` documents the
    // invariant rather than threading `io::Error` through the call
    // chain.
    write!(
        *buf,
        "{{\n  \"kid\": {kid_json},\n  \"pubkey_b64url\": {pubkey_json},\n  \"secret_hex\": \"{}\"\n}}",
        // Borrow the inner `String` via double-deref to `&str`. No new
        // allocation, no clone — the secret bytes are read directly
        // from the caller's `Zeroizing<String>` and written into our
        // (also `Zeroizing<String>`) output buffer.
        secret_hex.as_str(),
    )
    .expect("write! to String is infallible");
    Ok(buf)
}

/// V1.11 wave-3 Phase C — refuse `derive-key` when the wave-3 sealed
/// per-workspace signer is opted in.
///
/// **Why a separate refusal layer.** `derive-key` is the one ceremony
/// that emits the per-tenant SECRET hex on stdout. Under wave-3 the
/// per-tenant Ed25519 secret is generated and held inside the HSM with
/// `CKA_SENSITIVE=true` and `CKA_EXTRACTABLE=false` — the secret is
/// structurally unexportable. If `derive-key` ran unchanged under
/// wave-3 it would silently fall through to the wave-2 / dev master
/// seed (because the `with_master_seed` path does not see the wave-3
/// opt-in) and emit a hex secret that DOES NOT MATCH the actual
/// signing key inside the HSM. An operator using that hex value to
/// drive an external signer (e.g. `--secret-hex`) would produce
/// signatures that fail verification against the wave-3 pubkey — a
/// debugging nightmare with no clear remediation path. Refusing
/// loudly and early keeps the failure mode legible: "you can't export
/// a sealed key, here is the wave-3-compatible alternative."
///
/// The check reads `ATLAS_HSM_WORKSPACE_SIGNER` directly (not via the
/// loader) so the refusal fires even when the HSM trio is missing —
/// the operator's intent (wave-3) is what matters here, not whether
/// the underlying token is reachable. The fallthrough call delegates
/// to [`with_master_seed`] + [`run_derive_key`] which preserves the
/// V1.10 behaviour byte-for-byte.
fn run_derive_key_or_refuse(args: DeriveWorkspaceArgs) -> ExitCode {
    if std::env::var(WORKSPACE_HSM_OPT_IN_ENV)
        .ok()
        .map(|v| {
            let n = v.trim().to_ascii_lowercase();
            matches!(n.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
    {
        eprintln!(
            "derive-key: refused — wave-3 sealed per-workspace signer is opted in via \
             {WORKSPACE_HSM_OPT_IN_ENV}. The per-tenant Ed25519 secret is held inside \
             the HSM with CKA_SENSITIVE=true and CKA_EXTRACTABLE=false; it is \
             structurally unexportable. Use `derive-pubkey` to obtain the public \
             material, or unset {WORKSPACE_HSM_OPT_IN_ENV} to fall back to the \
             wave-2 / dev derivation (which produces an exportable secret)."
        );
        return ExitCode::from(2);
    }
    with_master_seed("derive-key", |hkdf| run_derive_key(args, hkdf))
}

fn run_derive_key(args: DeriveWorkspaceArgs, hkdf: &dyn keys::MasterSeedHkdf) -> ExitCode {
    if let Err(e) = keys::validate_workspace_id(&args.workspace) {
        eprintln!("derive-key: invalid --workspace: {e}");
        return ExitCode::from(2);
    }
    let (identity, secret_hex) = match keys::per_tenant_ceremony_output_via(
        hkdf,
        &args.workspace,
    ) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("derive-key: per-tenant derive failed: {e}");
            return ExitCode::from(2);
        }
    };
    let json_buf = match build_derive_key_json(&identity, &secret_hex) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("derive-key: emit failed: {e}");
            return ExitCode::from(2);
        }
    };
    println!("{}", json_buf.as_str());
    // `secret_hex` and `json_buf` drop here; `Zeroizing` scrubs the
    // heap String contents on every exit path.
    ExitCode::from(0)
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

/// V1.9: Same per-tenant pubkey derivation as `run_derive_key` but
/// emits only the public material. The MCP server uses this to
/// assemble per-workspace `PubkeyBundle`s without ever materialising
/// the workspace's signing key in TS heap.
///
/// **V1.11 wave-3 Phase C.** The handler takes a
/// [`WorkspaceSigner`] rather than a `MasterSeedHkdf` so the dispatcher
/// can route through the wave-3 sealed signer when opted in. Under
/// wave-3 the pubkey is read from `CKA_EC_POINT` on the on-token
/// public-key object (not derived in-process via HKDF), and the kid
/// is the same `atlas-anchor:` + workspace_id construction as before.
/// Verifier-side strict-mode pinning works unchanged because the kid
/// is independent of the signing backend.
fn run_derive_pubkey(args: DeriveWorkspaceArgs, signer: &dyn WorkspaceSigner) -> ExitCode {
    if let Err(e) = keys::validate_workspace_id(&args.workspace) {
        eprintln!("derive-pubkey: invalid --workspace: {e}");
        return ExitCode::from(2);
    }
    let identity = match per_tenant_identity_via_signer(signer, &args.workspace) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("derive-pubkey: per-tenant pubkey resolution failed: {e}");
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

fn run_rotate_pubkey_bundle(
    args: RotatePubkeyBundleArgs,
    signer: &dyn WorkspaceSigner,
) -> ExitCode {
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

    // V1.11 wave-3 Phase C: route through the WorkspaceSigner so the
    // bundle entry advertises the SEALED pubkey when wave-3 is opted
    // in. The mismatch-refusal logic below treats wave-3-rotated
    // pubkeys identically to a master-seed-rotated key — the operator
    // sees a clear "DIFFERENT pubkey" diagnostic rather than a silent
    // overwrite. This is the load-bearing ceremony for the V1.10 →
    // wave-3 migration: an operator opts into wave-3, runs
    // `rotate-pubkey-bundle` for each workspace, and gets the fresh
    // sealed pubkeys into the verifier-side trust store.
    let identity = match per_tenant_identity_via_signer(signer, &args.workspace) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("rotate-pubkey-bundle: per-tenant pubkey resolution failed: {e}");
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

fn run_sign(mut args: SignArgs, signer: Option<&dyn WorkspaceSigner>) -> ExitCode {
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

    // V1.11 wave-3 Phase C: workspace_id + kid validation runs BEFORE
    // building signing-input so operator errors (typos, mismatched kid)
    // surface without paying the canonical-bytes cost — and BEFORE
    // calling into the WorkspaceSigner so a malformed workspace_id never
    // reaches the HSM (defence-in-depth alongside the trait-level guard
    // in `WorkspaceSigner::sign`). The dev impl validates the same way,
    // so byte-for-byte compatibility with V1.10 is preserved.
    if let Some(ws) = args.derive_from_workspace.as_deref() {
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
    }

    // Secret-bytes path: parse the user-supplied 32-byte secret into a
    // [`SigningKey`] now, while the `Zeroizing` intermediate buffers are
    // still in scope. The returned key + the `Zeroizing` wrappers stay
    // alive until function exit so the secret scalar's heap residency is
    // bounded by the call. `None` when the workspace-derive path is
    // active (signing in that path goes through the trait, not an
    // in-process `SigningKey`).
    //
    // V1.11 Scope-A pre-flight follow-up: the sign-path's secret-bytes
    // chain (`secret_hex` String → `secret_bytes` Vec<u8> → `secret_array`
    // [u8; 32]) used to drop unscrubbed, leaving 32-byte signing-key
    // material in the freed-allocator pool until reuse. The ceremony
    // path (`build_derive_key_json` above) already wraps every heap
    // copy in `Zeroizing`. Wrapping here closes the divergence and is
    // mandatory now that the wave-3 trait sits between this dispatcher
    // and the per-workspace key store.
    //
    // `SigningKey::from_bytes(&*secret_array)` borrows the inner
    // [u8; 32], so the returned `SigningKey` does not extend the
    // `Zeroizing` wrapper's lifetime; the wrapper drops at the end
    // of this `else` arm and scrubs. `SigningKey` itself implements
    // `ZeroizeOnDrop` in `ed25519-dalek` ≥ 2, so the expanded scalar
    // is also scrubbed when `secret_signing_key` drops at function exit.
    let secret_signing_key: Option<SigningKey> = if args.derive_from_workspace.is_some() {
        None
    } else {
        let secret_hex: Zeroizing<String> = if args.secret_stdin {
            let mut buf: Zeroizing<String> = Zeroizing::new(String::new());
            if let Err(e) = io::stdin().read_to_string(&mut buf) {
                eprintln!("--secret-stdin: failed to read stdin: {e}");
                return ExitCode::from(2);
            }
            // `trim()` returns a borrow; copy the trimmed slice into a
            // fresh Zeroizing<String> so the trim-induced mid-buffer
            // bytes are scrubbed alongside the original buf. Both
            // `Zeroizing<String>` allocations drop at this `else` arm's
            // exit and zero their backing UTF-8 buffer.
            Zeroizing::new(buf.trim().to_string())
        } else {
            eprintln!(
                "WARNING: secret passed via --secret-hex (visible in process list). \
                 Use --secret-stdin in production."
            );
            // `args.secret_hex` is `Option<String>` from clap. Move it
            // straight into `Zeroizing` so the only unscrubbed window is
            // the live argv / clap parser's own buffer (which is already
            // visible in the process list — the WARNING above documents
            // why this path is deprecated for production).
            Zeroizing::new(
                args.secret_hex
                    .take()
                    .expect("mode_count check guarantees exactly one source"),
            )
        };
        let secret_bytes: Zeroizing<Vec<u8>> = match hex::decode(secret_hex.as_str()) {
            Ok(b) if b.len() == 32 => Zeroizing::new(b),
            Ok(_) => {
                eprintln!("secret must decode to 32 bytes");
                return ExitCode::from(2);
            }
            Err(e) => {
                eprintln!("secret invalid hex: {e}");
                return ExitCode::from(2);
            }
        };
        let mut secret_array: Zeroizing<[u8; 32]> = Zeroizing::new([0u8; 32]);
        secret_array.copy_from_slice(secret_bytes.as_slice());
        Some(SigningKey::from_bytes(&secret_array))
    };

    let parents: Vec<String> = if args.parents.is_empty() {
        Vec::new()
    } else {
        args.parents.split(',').map(|s| s.trim().to_string()).collect()
    };

    // V2-α Welle 1: atlas-signer V1 CLI surface does not yet wire up an
    // `--author-did` flag (would be a separate V2-α welle to add). Pass
    // None so V1 issuer-side behaviour is preserved exactly. V2-α
    // signing with author_did is exercised via the test path
    // (crates/atlas-trust-core/tests/agent_did_integration.rs) until
    // a follow-up welle adds the CLI knob.
    let signing_input = match build_signing_input(&workspace, &event_id, &ts, &kid, &parents, &payload, None) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("signing-input build failed: {e}");
            return ExitCode::from(2);
        }
    };

    let event_hash = compute_event_hash(&signing_input);

    // Sign: wave-3 path delegates the entire signing operation to the
    // [`WorkspaceSigner`] (which may run inside an HSM via `CKM_EDDSA`,
    // never exposing the per-tenant scalar to Atlas address space);
    // legacy paths sign in-process with the [`SigningKey`] assembled
    // above. Both branches produce a 64-byte raw RFC 8032 signature —
    // the consumer side (the verifier) cannot tell which signed the
    // event, which is the whole point: wave-3 is a deployment knob,
    // not a wire-format change.
    let sig_bytes: [u8; 64] = if let Some(ws) = args.derive_from_workspace.as_deref() {
        // Phase-C invariant: the dispatcher (`run_sign_dispatch`) only
        // loads a `WorkspaceSigner` when `args.derive_from_workspace`
        // is `Some`, the same predicate that guards this `if let`
        // arm. Reaching `None` here would mean dispatcher and branch
        // disagree about when to load — a coding bug, not a runtime
        // condition. Panic to surface it loudly.
        let signer = signer.expect(
            "Phase-C invariant: run_sign_dispatch loads the workspace signer iff \
             --derive-from-workspace is set",
        );
        match signer.sign(ws, &signing_input) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("sign --derive-from-workspace: per-tenant sign failed: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        let signing_key = secret_signing_key
            .expect("mode_count check guarantees exactly one secret source");
        signing_key.sign(&signing_input).to_bytes()
    };

    let event = AtlasEvent {
        event_id,
        event_hash,
        parent_hashes: parents,
        payload,
        signature: EventSignature {
            alg: "EdDSA".to_string(),
            kid,
            sig: b64url_no_pad_encode(&sig_bytes),
        },
        ts,
        author_did: None,
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
