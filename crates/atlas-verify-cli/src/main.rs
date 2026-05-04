//! `atlas-verify-cli` — standalone offline trace verifier.
//!
//! Use case: hand a regulator (BaFin, EMA, FDA) a trace JSON file.
//! They run `atlas-verify-cli verify-trace bundle.json` on their own machine.
//! Output: ✓ VALID or ✗ INVALID with a per-check evidence list.
//!
//! No network calls. Pubkey-bundle is supplied as a separate file.

use atlas_trust_core::{
    pubkey_bundle::PubkeyBundle,
    trace_format::AtlasTrace,
    verify::{verify_trace_with, VerifyOptions},
};
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

        /// V1.9 strict mode: every event in the trace must be signed by
        /// the per-tenant kid `atlas-anchor:{trace.workspace_id}`.
        /// Legacy SPIFFE kids fail this check. Lenient mode (default)
        /// accepts both legacy and per-tenant kids.
        ///
        /// Trust note: an attacker who can downgrade a bundle from
        /// per-tenant to legacy bypasses isolation in lenient mode.
        /// Strict mode is the real security boundary for V1.9 traces.
        #[arg(long)]
        require_per_tenant_keys: bool,

        /// V1.5 strict mode: every entry in `trace.dag_tips` must be
        /// covered by a successful anchor in `trace.anchors`. Lenient
        /// mode (default) treats absent or partial anchors as "no
        /// claim, no problem" — useful for traces from unanchored
        /// dev runs.
        #[arg(long)]
        require_anchors: bool,

        /// V1.7 strict mode: `trace.anchor_chain` must be present,
        /// internally consistent, and (if `trace.anchors` is also
        /// present) must cover every entry in `trace.anchors`.
        /// Defaults to false so V1.5/V1.6 bundles continue to verify.
        #[arg(long)]
        require_anchor_chain: bool,

        /// V1.13 wave C-2 strict mode: minimum number of distinct
        /// witness cosignatures (kid-distinct, sig-valid, verified
        /// against `ATLAS_WITNESS_V1_ROSTER`) required across the
        /// trace's anchor-chain history. `0` (default) is lenient —
        /// witness coverage is reported but not required. `>= 1` is
        /// strict — verification fails if fewer witnesses verify than
        /// the threshold (or if the trace has no `anchor_chain`).
        ///
        /// Trust note: lenient mode treats traces without witness
        /// coverage as valid; strict mode is the security boundary
        /// for the V1.13 second-trust-domain property. Same lenient/
        /// strict shape as `--require-anchor-chain` and
        /// `--require-per-tenant-keys`.
        #[arg(long, default_value_t = 0)]
        require_witness: usize,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Command::VerifyTrace {
            trace,
            pubkey_bundle,
            output,
            require_per_tenant_keys,
            require_anchors,
            require_anchor_chain,
            require_witness,
        } => {
            let opts = VerifyOptions {
                require_per_tenant_keys,
                require_anchors,
                require_anchor_chain,
                require_witness_threshold: require_witness,
            };
            match run_verify_trace(&trace, &pubkey_bundle, &output, &opts) {
                Ok(true) => ExitCode::from(0),
                Ok(false) => ExitCode::from(1),
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(2)
                }
            }
        }
    }
}

fn run_verify_trace(
    trace_path: &PathBuf,
    bundle_path: &PathBuf,
    output: &str,
    opts: &VerifyOptions,
) -> Result<bool, Box<dyn std::error::Error>> {
    let bundle_bytes = fs::read(bundle_path)?;
    let bundle = PubkeyBundle::from_json(&bundle_bytes)?;

    let trace_bytes = fs::read(trace_path)?;
    let trace: AtlasTrace = serde_json::from_slice(&trace_bytes)?;
    let outcome = verify_trace_with(&trace, &bundle, opts);

    if output == "json" {
        let json = serde_json::to_string_pretty(&outcome)?;
        println!("{json}");
    } else {
        print_human(&outcome, opts);
    }

    Ok(outcome.valid)
}

fn print_human(outcome: &atlas_trust_core::verify::VerifyOutcome, opts: &VerifyOptions) {
    println!();
    let any_strict = opts.require_per_tenant_keys
        || opts.require_anchors
        || opts.require_anchor_chain
        || opts.require_witness_threshold > 0;
    let strict_tag = if any_strict { " (strict mode)" } else { "" };
    if outcome.valid {
        println!("  \u{2713} VALID — all checks passed{strict_tag}");
    } else {
        println!("  \u{2717} INVALID — verification failed{strict_tag}");
    }
    println!();
    println!("  Verifier: {}", outcome.verifier_version);
    if any_strict {
        let mut flags: Vec<String> = Vec::new();
        if opts.require_per_tenant_keys {
            flags.push("require_per_tenant_keys".to_string());
        }
        if opts.require_anchors {
            flags.push("require_anchors".to_string());
        }
        if opts.require_anchor_chain {
            flags.push("require_anchor_chain".to_string());
        }
        if opts.require_witness_threshold > 0 {
            // Surface the threshold value (not just the flag name) —
            // operator needs to know whether they ran 1-of-N or M-of-N.
            flags.push(format!(
                "require_witness={}",
                opts.require_witness_threshold,
            ));
        }
        println!("  Strict flags: {}", flags.join(", "));
    }
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
