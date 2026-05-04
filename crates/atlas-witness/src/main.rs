#![deny(unsafe_code)]
#![warn(missing_docs)]

//! atlas-witness — independent cosignature attestor over chain heads.
//!
//! Subcommand surface:
//!
//!   * `gen-key` (V1.13) — generate a fresh Ed25519 keypair, write the
//!     secret bytes (raw 32 bytes) and public key (hex) to disk. Operator
//!     pastes the hex pubkey into `ATLAS_WITNESS_V1_ROSTER` and bumps
//!     the trust-core crate version (V1.7 boundary rule).
//!
//!   * `sign-chain-head` (V1.13 + V1.14 Scope I) — sign a hex chain
//!     head and print the resulting `WitnessSig` JSON to stdout. Caller
//!     appends this to `AnchorBatch.witnesses` before shipping the
//!     trace. Two backends:
//!       - `--secret-file PATH` (file-backed) — V1.13 default, reads a
//!         32-byte raw secret from disk.
//!       - `--hsm` (HSM-backed) — V1.14 Scope I, dispatches to
//!         [`atlas_witness::hsm::Pkcs11Witness`]. The witness HSM env
//!         trio (`ATLAS_WITNESS_HSM_PKCS11_LIB`,
//!         `ATLAS_WITNESS_HSM_SLOT`, `ATLAS_WITNESS_HSM_PIN_FILE`) MUST
//!         be set; the binary refuses to start otherwise. Mutually
//!         exclusive with `--secret-file`.
//!
//!   * `extract-pubkey-hex` (V1.14 Scope I) — extract the witness
//!     public key from the HSM as a 64-char hex string. Used during
//!     the operator commissioning ceremony (OPERATOR-RUNBOOK §11) to
//!     surface the pubkey for pasting into `ATLAS_WITNESS_V1_ROSTER`
//!     after `pkcs11-tool --keypairgen` places the keypair on the
//!     token. HSM-only — no file-backed analogue (the file-backed
//!     `gen-key` already prints the pubkey to disk).
//!
//! Operator-surface caveats:
//!   * `gen-key` writes the secret as raw 32 bytes — apply `chmod 0400`
//!     immediately and place on a host with restrictive ACLs (mirrors
//!     OPERATOR-RUNBOOK §2 master-seed-file guidance). The witness's
//!     own runbook section is §11 (V1.14 Scope I).
//!   * Pubkey is hex (64 chars) so it can be pasted into the roster
//!     constant directly without base64↔hex juggling.
//!   * The HSM-backed path requires `--features hsm` at compile time;
//!     a default-features build with `--hsm` falls through to the
//!     stub `Pkcs11Witness` which fails closed with an `Unavailable:`
//!     remediation message. The runbook calls out this build-time
//!     gate alongside the env-trio check.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgGroup, Parser, Subcommand};
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use zeroize::Zeroizing;

use atlas_witness::hsm::config::HsmWitnessConfig;
use atlas_witness::hsm::Pkcs11Witness;
use atlas_witness::{Ed25519Witness, Witness};

#[derive(Parser)]
#[command(
    name = "atlas-witness",
    version,
    about = "Atlas Witness cosignature attestor (V1.13 Scope C + V1.14 Scope I HSM-backed)"
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
    #[command(group(
        // Exactly one of `--secret-file` / `--hsm` MUST be set. clap
        // enforces the mutual exclusion at parse time so the wrong
        // combination surfaces as a CLI usage error before any
        // signing path is touched. Operator-friendly: the help text
        // lists both flags so it's obvious which one to add.
        ArgGroup::new("backend")
            .args(["secret_file", "hsm"])
            .required(true)
            .multiple(false)
    ))]
    SignChainHead {
        /// Path to the 32-byte raw secret key file (V1.13 file-backed
        /// backend). Mutually exclusive with `--hsm`.
        #[arg(long)]
        secret_file: Option<PathBuf>,
        /// Use the HSM-backed witness backend (V1.14 Scope I). Reads
        /// the witness HSM env trio (`ATLAS_WITNESS_HSM_PKCS11_LIB`,
        /// `ATLAS_WITNESS_HSM_SLOT`, `ATLAS_WITNESS_HSM_PIN_FILE`)
        /// and resolves the keypair on the token by label
        /// `atlas-witness-key-v1:<kid>`. Mutually exclusive with
        /// `--secret-file`.
        #[arg(long)]
        hsm: bool,
        /// Witness kid — must match the entry the corresponding pubkey
        /// is registered under in `ATLAS_WITNESS_V1_ROSTER`.
        #[arg(long)]
        kid: String,
        /// Chain-head hex (64 chars = 32 bytes) — output of
        /// `chain_head_for(batch)` for the batch being witnessed.
        #[arg(long)]
        chain_head: String,
    },
    /// V1.14 Scope I — Extract the witness public key from the HSM as
    /// a 64-char hex string. HSM-only; reads the witness HSM env trio.
    /// Used during the operator commissioning ceremony
    /// (OPERATOR-RUNBOOK §11) to surface the pubkey for pasting into
    /// `ATLAS_WITNESS_V1_ROSTER`.
    ExtractPubkeyHex {
        /// Witness kid — resolves the public-key object by label
        /// `atlas-witness-key-v1:<kid>` and reads its CKA_EC_POINT.
        #[arg(long)]
        kid: String,
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
            hsm,
            kid,
            chain_head,
        } => {
            let result = if hsm {
                sign_chain_head_via_hsm(&kid, &chain_head)
            } else {
                // `secret_file` is required by the clap ArgGroup when
                // `--hsm` is unset, so `unwrap()` is safe here. Using
                // `expect` over `unwrap` so a future ArgGroup change
                // that drops the `required = true` constraint surfaces
                // as a clear runtime panic at the seam, not a silent
                // None-deref further down the call stack.
                let secret = secret_file.expect("clap ArgGroup guarantees --secret-file present");
                sign_chain_head(&secret, &kid, &chain_head)
            };
            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("atlas-witness sign-chain-head: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::ExtractPubkeyHex { kid } => match extract_pubkey_hex(&kid) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("atlas-witness extract-pubkey-hex: {e}");
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
        "atlas-witness gen-key:\n  \
         secret -> {secret} (Unix: mode 0o600 set atomically at create; \
         IMMEDIATELY run `chmod 0400 {secret}` to drop owner-write — \
         leaving 0o600 lets a compromised process overwrite the key)\n  \
         pubkey -> {pubkey} (NON-SENSITIVE — public material; default \
         umask is fine, NO chmod required, NO 0400 needed)\n  \
         next: paste the hex pubkey into ATLAS_WITNESS_V1_ROSTER \
         (crates/atlas-trust-core/src/witness.rs) and bump the crate \
         version per OPERATOR-RUNBOOK §10.",
        secret = secret_path.display(),
        pubkey = pubkey_path.display(),
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

/// V1.14 Scope I — HSM-backed sign path. Reads the env trio, opens
/// the PKCS#11 module via [`Pkcs11Witness::open`], signs the chain
/// head, and emits the resulting `WitnessSig` JSON to stdout.
///
/// Wire-shape and roster contract are identical to the file-backed
/// path; the byte-equivalence integration test
/// (`tests/hsm_witness_byte_equivalence.rs`) pins this load-bearing
/// invariant.
fn sign_chain_head_via_hsm(kid: &str, chain_head_hex: &str) -> Result<(), String> {
    let cfg = load_hsm_config_from_env()?;
    let witness = Pkcs11Witness::open(cfg, kid.to_string())
        .map_err(|e| format!("open HSM-backed witness: {e}"))?;
    let sig = witness
        .sign_chain_head(chain_head_hex)
        .map_err(|e| format!("sign-chain-head: {e}"))?;
    let json = serde_json::to_string(&sig)
        .map_err(|e| format!("serialise WitnessSig: {e}"))?;
    println!("{json}");
    Ok(())
}

/// V1.14 Scope I — extract the witness public key from the HSM as
/// 64-char hex and print to stdout. The runbook calls this during
/// commissioning, after `pkcs11-tool --keypairgen` has placed the
/// keypair on the token: the operator captures the hex output, pastes
/// it into `ATLAS_WITNESS_V1_ROSTER`, and bumps the trust-core crate
/// version per OPERATOR-RUNBOOK §11.
fn extract_pubkey_hex(kid: &str) -> Result<(), String> {
    let cfg = load_hsm_config_from_env()?;
    let hex_pubkey = Pkcs11Witness::extract_pubkey_hex(cfg, kid)
        .map_err(|e| format!("extract-pubkey-hex: {e}"))?;
    println!("{hex_pubkey}");
    Ok(())
}

/// Read the witness HSM env trio into an `HsmWitnessConfig`. Refuses
/// "trio not set" with a runbook-aware message because both
/// HSM-mode CLI paths (`sign-chain-head --hsm` and
/// `extract-pubkey-hex`) require the trio to function — silent
/// fall-through to file-backed would hide a misconfigured deployment
/// (the operator wanted HSM, but the wrong trio went down). Mirrors
/// atlas-signer's "missing trio is a configuration error" stance.
fn load_hsm_config_from_env() -> Result<HsmWitnessConfig, String> {
    match HsmWitnessConfig::from_env(|name| std::env::var(name).ok())? {
        Some(cfg) => Ok(cfg),
        None => Err(
            "HSM-mode requested but the witness HSM env trio is unset — set \
             ATLAS_WITNESS_HSM_PKCS11_LIB, ATLAS_WITNESS_HSM_SLOT, and \
             ATLAS_WITNESS_HSM_PIN_FILE (all three) per OPERATOR-RUNBOOK §11"
                .to_string(),
        ),
    }
}
