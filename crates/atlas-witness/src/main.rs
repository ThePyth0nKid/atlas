#![deny(unsafe_code)]
#![warn(missing_docs)]

//! atlas-witness — independent cosignature attestor over chain heads.
//!
//! Two subcommands for V1.13 Scope C wave 1:
//!
//!   * `gen-key` — generate a fresh Ed25519 keypair, write the secret
//!     bytes (raw 32 bytes) and public key (hex) to disk. Operator
//!     pastes the hex pubkey into `ATLAS_WITNESS_V1_ROSTER` and bumps
//!     the trust-core crate version (V1.7 boundary rule).
//!
//!   * `sign-chain-head` — sign a hex chain head with a stored secret,
//!     print the resulting `WitnessSig` JSON to stdout. Caller appends
//!     this to `AnchorBatch.witnesses` before shipping the trace.
//!
//! Both subcommands are file-backed — HSM-backed witness signing is
//! deferred to a later wave. The trait shape (`Witness`) is HSM-friendly
//! so a future PKCS#11 implementation slots in alongside `Ed25519Witness`
//! without a CLI surface change.
//!
//! Operator-surface caveats:
//!   * `gen-key` writes the secret as raw 32 bytes — apply `chmod 0400`
//!     immediately and place on a host with restrictive ACLs (mirrors
//!     OPERATOR-RUNBOOK §2 master-seed-file guidance). The runbook
//!     ceremony is added in V1.13 Scope C wave 4 doc-sync.
//!   * Pubkey is hex (64 chars) so it can be pasted into the roster
//!     constant directly without base64↔hex juggling.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use zeroize::Zeroizing;

use atlas_witness::{Ed25519Witness, Witness};

#[derive(Parser)]
#[command(
    name = "atlas-witness",
    version,
    about = "Atlas Witness cosignature attestor (V1.13 Scope C wave 1)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a new Ed25519 witness keypair (secret + pubkey written to disk).
    GenKey {
        /// Path to write the 32-byte raw secret key.
        ///
        /// Operator MUST `chmod 0400` immediately after generation and
        /// place on a host with restrictive ACLs. The secret never
        /// leaves this file at runtime — `sign-chain-head` reads, signs,
        /// and the in-memory copy is `Zeroizing`-wrapped.
        #[arg(long)]
        out: PathBuf,
        /// Path to write the 32-byte raw public key, hex-encoded
        /// (64 ASCII chars). Pastable into `ATLAS_WITNESS_V1_ROSTER`.
        #[arg(long)]
        pubkey_out: PathBuf,
    },
    /// Sign a chain head and print the resulting WitnessSig JSON to stdout.
    SignChainHead {
        /// Path to the 32-byte raw secret key file.
        #[arg(long)]
        secret_file: PathBuf,
        /// Witness kid — must match the entry the corresponding pubkey
        /// is registered under in `ATLAS_WITNESS_V1_ROSTER`.
        #[arg(long)]
        kid: String,
        /// Chain-head hex (64 chars = 32 bytes) — output of
        /// `chain_head_for(batch)` for the batch being witnessed.
        #[arg(long)]
        chain_head: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::GenKey { out, pubkey_out } => match gen_key(&out, &pubkey_out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("atlas-witness gen-key: {e}");
                ExitCode::FAILURE
            }
        },
        Command::SignChainHead {
            secret_file,
            kid,
            chain_head,
        } => match sign_chain_head(&secret_file, &kid, &chain_head) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("atlas-witness sign-chain-head: {e}");
                ExitCode::FAILURE
            }
        },
    }
}

fn gen_key(secret_path: &PathBuf, pubkey_path: &PathBuf) -> Result<(), String> {
    // OS CSPRNG — ed25519-dalek 2.x's SigningKey::generate consumes a
    // CryptoRng+RngCore source. OsRng pulls from /dev/urandom (Unix) or
    // BCryptGenRandom (Windows) via getrandom; the right primitive for
    // long-lived high-trust witness keys.
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);

    // Wrap the raw secret bytes in Zeroizing so they're scrubbed on
    // drop even on the writer-side (defence-in-depth alongside the
    // file-system-level protection of `chmod 0400`).
    let secret_bytes = Zeroizing::new(signing_key.to_bytes());
    let pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());

    // Write the secret with restrictive perms set AT CREATE TIME — not
    // after `fs::write` has already opened the file with the umask
    // default (typically 0o644), which would leave a TOCTOU window where
    // any local user could read the secret before the operator runs
    // `chmod 0400`. `create_new(true)` also fail-closed refuses to
    // overwrite an existing key file, so a typo (`--out same/path`) won't
    // silently destroy a previously-generated witness key.
    write_secret_file(secret_path, &secret_bytes)?;

    // Pubkey is non-sensitive (it's pasted into the source roster as
    // public material) — default umask is fine.
    fs::write(pubkey_path, pubkey_hex.as_bytes())
        .map_err(|e| format!("write pubkey to {}: {e}", pubkey_path.display()))?;

    eprintln!(
        "atlas-witness gen-key: wrote secret to {} (mode 0o600 on Unix; chmod 0400 to disable later overwrite) and pubkey to {}",
        secret_path.display(),
        pubkey_path.display(),
    );
    Ok(())
}

/// Create the secret file with the most restrictive permissions the
/// platform supports, set ATOMICALLY at create time.
///
/// Unix: `create_new(true) + mode(0o600)` — owner read+write only,
/// fail-closed if the path already exists.
///
/// Non-Unix (Windows): `create_new(true)` plus an advisory eprintln. We
/// can't set Windows ACLs from `std::fs` alone — the OS-supplied
/// inheritance from the user-profile directory typically yields
/// user-only access in practice, but operators should verify before
/// trusting the file.
fn write_secret_file(secret_path: &PathBuf, secret_bytes: &[u8; 32]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(secret_path)
            .map_err(|e| {
                format!(
                    "create secret at {} (mode 0o600, must not exist): {e}",
                    secret_path.display(),
                )
            })?;
        f.write_all(secret_bytes)
            .map_err(|e| format!("write secret bytes to {}: {e}", secret_path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let mut f = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(secret_path)
            .map_err(|e| {
                format!(
                    "create secret at {} (must not exist): {e}",
                    secret_path.display(),
                )
            })?;
        f.write_all(secret_bytes)
            .map_err(|e| format!("write secret bytes to {}: {e}", secret_path.display()))?;
        eprintln!(
            "atlas-witness gen-key: NON-UNIX platform — verify the ACL on \
             {} grants read access only to the witness service principal",
            secret_path.display(),
        );
    }
    Ok(())
}

fn sign_chain_head(
    secret_file: &PathBuf,
    kid: &str,
    chain_head_hex: &str,
) -> Result<(), String> {
    let secret_raw = Zeroizing::new(
        fs::read(secret_file)
            .map_err(|e| format!("read secret from {}: {e}", secret_file.display()))?,
    );
    let secret_bytes: [u8; 32] = secret_raw
        .as_slice()
        .try_into()
        .map_err(|_| {
            format!(
                "secret file {} must be exactly 32 bytes, got {}",
                secret_file.display(),
                secret_raw.len(),
            )
        })?;

    let witness = Ed25519Witness::new(kid.to_string(), secret_bytes);
    let sig = witness
        .sign_chain_head(chain_head_hex)
        .map_err(|e| format!("sign-chain-head: {e}"))?;

    // One-line JSON to stdout — appendable to AnchorBatch.witnesses by
    // the caller (e.g. via `jq` or a small TS wrapper in the MCP smoke).
    let json = serde_json::to_string(&sig)
        .map_err(|e| format!("serialise WitnessSig: {e}"))?;
    println!("{json}");
    Ok(())
}
