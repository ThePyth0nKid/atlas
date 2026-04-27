//! `atlas-verify-cli` — standalone offline trace verifier.
//!
//! Use case: hand a regulator (BaFin, EMA, FDA) a trace JSON file.
//! They run `atlas-verify-cli verify-trace bundle.json` on their own machine.
//! Output: ✓ VALID or ✗ INVALID with a per-check evidence list.
//!
//! No network calls. Pubkey-bundle is supplied as a separate file.

use atlas_trust_core::{pubkey_bundle::PubkeyBundle, verify::verify_trace_json};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "atlas-verify-cli")]
#[command(version, about = "Offline verifier for Atlas verifiable knowledge graph traces", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Verify an Atlas trace bundle JSON against a pubkey bundle.
    VerifyTrace {
        /// Path to trace bundle JSON (output of `atlas bundle.export`).
        trace: PathBuf,

        /// Path to pubkey bundle JSON (cosigned, deterministic).
        #[arg(long, short = 'k')]
        pubkey_bundle: PathBuf,

        /// Output format: human (default) or json.
        #[arg(long, short = 'o', default_value = "human")]
        output: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::VerifyTrace {
            trace,
            pubkey_bundle,
            output,
        } => match run_verify_trace(&trace, &pubkey_bundle, &output) {
            Ok(true) => ExitCode::from(0),
            Ok(false) => ExitCode::from(1),
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }
        },
    }
}

fn run_verify_trace(
    trace_path: &PathBuf,
    bundle_path: &PathBuf,
    output: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let bundle_bytes = fs::read(bundle_path)?;
    let bundle = PubkeyBundle::from_json(&bundle_bytes)?;

    let trace_bytes = fs::read(trace_path)?;
    let outcome = verify_trace_json(&trace_bytes, &bundle)?;

    if output == "json" {
        let json = serde_json::to_string_pretty(&outcome)?;
        println!("{json}");
    } else {
        print_human(&outcome);
    }

    Ok(outcome.valid)
}

fn print_human(outcome: &atlas_trust_core::verify::VerifyOutcome) {
    println!();
    if outcome.valid {
        println!("  \u{2713} VALID — all checks passed");
    } else {
        println!("  \u{2717} INVALID — verification failed");
    }
    println!();
    println!("  Verifier: {}", outcome.verifier_version);
    println!();
    println!("  Evidence:");
    for ev in &outcome.evidence {
        let mark = if ev.ok { "\u{2713}" } else { "\u{2717}" };
        println!("    {} {} — {}", mark, ev.check, ev.detail);
    }
    if !outcome.errors.is_empty() {
        println!();
        println!("  Errors:");
        for e in &outcome.errors {
            println!("    - {e}");
        }
    }
    println!();
}
