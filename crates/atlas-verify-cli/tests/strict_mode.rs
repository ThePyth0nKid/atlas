//! V1.10 warm-up: integration tests for the `verify-trace` strict-mode
//! flags (`--require-per-tenant-keys`, `--require-anchors`,
//! `--require-anchor-chain`).
//!
//! V1.19 Welle 12: extended with `--require-strict-chain` happy-path
//! coverage at the CLI surface. Negative-path coverage lives at the
//! library layer (`crates/atlas-trust-core/src/hashchain.rs` — 9
//! strict-chain unit tests covering empty-trace, two-genesis,
//! zero-genesis, sibling-fork, DAG-merge, and self-reference); the CLI
//! integration test pins only the happy path because synthesising a
//! forked-DAG fixture would require pre-signed `event_hash` values for
//! marginal additional coverage. The Welle-10 atlas-mcp-server smoke
//! and the Welle-12 atlas-web `e2e-write-roundtrip.ts` both exercise
//! the same `--require-strict-chain` codepath end-to-end against
//! real-signed traces — three lanes plus this CLI test give the gate
//! defence-in-depth in CI.
//!
//! These tests are the contract that the CLI surfaces every documented
//! V1.9 security boundary so an auditor running the binary can actually
//! exercise it. Pre-V1.10-warm-up, the CLI hardcoded
//! `VerifyOptions::default()` (lenient), which made the strict-mode
//! story fictional.
//!
//! Test fixtures: the `examples/golden-traces/bank-q1-2026.*` pair from
//! `atlas-signer/examples/seed_bank_demo.rs`. That demo predates V1.9
//! and signs every event with a legacy SPIFFE kid
//! (`spiffe://atlas/agent/...`), so it is the canonical "lenient passes,
//! strict per-tenant rejects" specimen. The trace is also a 5-event
//! linear chain (one genesis + four single-parent successors, no forks),
//! so `--require-strict-chain` accepts it cleanly (V1.19 Welle 12).

use std::process::Command;

/// Path to the workspace-root golden traces. We compute it from the
/// crate manifest path rather than hard-coding a relative path so the
/// test runs from any cwd the user might happen to be in.
fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate is two levels deep under the workspace root")
        .to_path_buf()
}

fn bank_trace() -> std::path::PathBuf {
    repo_root()
        .join("examples")
        .join("golden-traces")
        .join("bank-q1-2026.trace.json")
}

fn bank_bundle() -> std::path::PathBuf {
    repo_root()
        .join("examples")
        .join("golden-traces")
        .join("bank-q1-2026.pubkey-bundle.json")
}

fn cli_bin() -> std::path::PathBuf {
    // Cargo sets this for `[[bin]]` integration tests automatically.
    // No need for an `assert_cmd` dev-dep — the env var is the
    // documented Cargo contract for "find the binary built alongside
    // this test".
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_atlas-verify-cli"))
}

#[test]
fn lenient_mode_accepts_legacy_spiffe_trace() {
    // The bank-demo trace is signed exclusively with legacy SPIFFE
    // kids. In lenient mode (no flags) the verifier accepts those kids
    // and emits ✓ VALID with exit 0.
    let output = Command::new(cli_bin())
        .args([
            "verify-trace",
            bank_trace().to_str().expect("trace path is utf8"),
            "--pubkey-bundle",
            bank_bundle().to_str().expect("bundle path is utf8"),
        ])
        .output()
        .expect("failed to spawn atlas-verify-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "lenient verify must exit 0 for the bank-demo trace.\nstdout:\n{stdout}\nstderr:\n{stderr}",
    );
    assert!(
        stdout.contains("VALID"),
        "expected ✓ VALID in stdout; got:\n{stdout}",
    );
    assert!(
        !stdout.contains("strict mode"),
        "lenient run must not advertise strict mode; got:\n{stdout}",
    );
}

#[test]
fn strict_per_tenant_rejects_legacy_spiffe_trace() {
    // Same trace, but with `--require-per-tenant-keys`. Every event
    // carries a `spiffe://atlas/...` kid, which fails the per-tenant
    // strict check. Verifier emits ✗ INVALID with exit code 1.
    let output = Command::new(cli_bin())
        .args([
            "verify-trace",
            bank_trace().to_str().expect("trace path is utf8"),
            "--pubkey-bundle",
            bank_bundle().to_str().expect("bundle path is utf8"),
            "--require-per-tenant-keys",
        ])
        .output()
        .expect("failed to spawn atlas-verify-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "strict verify must exit 1 (verification failed), got {:?}.\nstdout:\n{stdout}",
        output.status.code(),
    );
    assert!(
        stdout.contains("INVALID"),
        "expected ✗ INVALID in stdout; got:\n{stdout}",
    );
    assert!(
        stdout.contains("strict mode"),
        "strict run must advertise strict mode in the header; got:\n{stdout}",
    );
    assert!(
        stdout.contains("require_per_tenant_keys"),
        "strict run must list the active flag; got:\n{stdout}",
    );
}

#[test]
fn strict_anchors_rejects_unanchored_trace_when_tips_present() {
    // The bank-demo trace has `dag_tips` but `anchors: []` — the V1.5
    // lenient design accepts this, V1.5 strict mode does not.
    // `--require-anchors` flips the policy at the auditor's seat.
    let output = Command::new(cli_bin())
        .args([
            "verify-trace",
            bank_trace().to_str().expect("trace path is utf8"),
            "--pubkey-bundle",
            bank_bundle().to_str().expect("bundle path is utf8"),
            "--require-anchors",
        ])
        .output()
        .expect("failed to spawn atlas-verify-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "strict-anchors verify must exit 1 (no anchors claimed but tips present).\nstdout:\n{stdout}",
    );
    assert!(stdout.contains("INVALID"));
}

#[test]
fn strict_anchor_chain_rejects_chainless_trace() {
    // The bank-demo trace predates V1.7 (no anchor_chain field).
    // Lenient verify passes; `--require-anchor-chain` rejects it.
    let output = Command::new(cli_bin())
        .args([
            "verify-trace",
            bank_trace().to_str().expect("trace path is utf8"),
            "--pubkey-bundle",
            bank_bundle().to_str().expect("bundle path is utf8"),
            "--require-anchor-chain",
        ])
        .output()
        .expect("failed to spawn atlas-verify-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "strict-chain verify must exit 1 when no anchor_chain present.\nstdout:\n{stdout}",
    );
    assert!(stdout.contains("INVALID"));
}

#[test]
fn strict_chain_passes_linear_bank_trace() {
    // V1.19 Welle 12: `--require-strict-chain` happy-path at the CLI
    // surface. The bank-demo trace is a 5-event linear chain
    // (1 genesis + 4 single-parent successors, no sibling-forks, no
    // multi-genesis, no self-reference, non-empty), so the verifier
    // accepts it under strict-chain mode and surfaces the new evidence
    // row + Strict flags line.
    //
    // Pins four properties:
    //
    //   1. Exit code 0 — strict-chain holds on a linear trace.
    //   2. "✓ VALID" + "strict mode" header — the run is advertised as
    //      strict (any_strict in print_human is true).
    //   3. "require_strict_chain" in `Strict flags:` line — the flag
    //      is the active strict opt-in.
    //   4. "strict-chain" evidence row with happy-path prose "form a
    //      strict linear chain" — the evidence list contains the new
    //      Welle-9 check line.
    //
    // The symmetric negative path (`require_strict_chain = true` on a
    // sibling-fork trace) is covered exhaustively by 9 hashchain unit
    // tests in `crates/atlas-trust-core/src/hashchain.rs`. Adding a
    // forked-DAG CLI fixture would require pre-signed `event_hash`
    // values whose recomputation passes `check_event_hashes` before
    // `check_strict_chain` runs (per Welle 9 CR-1 gating). Marginal
    // additional coverage at the CLI surface for that synthesis cost.
    let output = Command::new(cli_bin())
        .args([
            "verify-trace",
            bank_trace().to_str().expect("trace path is utf8"),
            "--pubkey-bundle",
            bank_bundle().to_str().expect("bundle path is utf8"),
            "--require-strict-chain",
        ])
        .output()
        .expect("failed to spawn atlas-verify-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "strict-chain verify must exit 0 on linear bank trace.\nstdout:\n{stdout}\nstderr:\n{stderr}",
    );
    // Pin "✓ VALID" specifically — the bare substring "VALID" would
    // also match "INVALID", and a future test-harness log line
    // containing the word "VALID" elsewhere in stdout could pass a
    // bare contains("VALID") check vacuously. The ✓ marker is part of
    // the print_human happy-path contract.
    assert!(
        stdout.contains("\u{2713} VALID"),
        "expected ✓ VALID in stdout; got:\n{stdout}",
    );
    assert!(
        stdout.contains("strict mode"),
        "strict-chain run must advertise strict mode in the header; got:\n{stdout}",
    );
    assert!(
        stdout.contains("require_strict_chain"),
        "strict-chain run must list 'require_strict_chain' in 'Strict flags:'; got:\n{stdout}",
    );
    // The strict-chain evidence row is pinned by its happy-path prose
    // "form a strict linear chain" — that string appears ONLY in the
    // ok-case rendering of `check_strict_chain` (the error path
    // produces "strict-chain violation: ..." with no occurrence of
    // "linear chain"). A bare contains("strict-chain") would match an
    // error row too, so the tighter prose pin is the actual contract.
    assert!(
        stdout.contains("form a strict linear chain"),
        "strict-chain evidence row must contain happy-path prose; got:\n{stdout}",
    );
}

#[test]
fn json_output_carries_outcome_and_evidence() {
    // `-o json` is the machine-readable mode. The shape is the
    // `VerifyOutcome` serde JSON; we sanity-check the top-level keys
    // an auditor's tooling would expect.
    let output = Command::new(cli_bin())
        .args([
            "verify-trace",
            bank_trace().to_str().expect("trace path is utf8"),
            "--pubkey-bundle",
            bank_bundle().to_str().expect("bundle path is utf8"),
            "-o",
            "json",
        ])
        .output()
        .expect("failed to spawn atlas-verify-cli");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON output must parse as valid JSON");
    assert_eq!(parsed.get("valid").and_then(|v| v.as_bool()), Some(true));
    assert!(
        parsed.get("evidence").is_some(),
        "JSON output must include an evidence list",
    );
    assert!(
        parsed.get("verifier_version").is_some(),
        "JSON output must include verifier_version",
    );
}
