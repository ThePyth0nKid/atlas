//! Test-only helpers shared across the crate's `#[cfg(test)]` modules.
//!
//! V1.10 wave 2 introduced a second consumer for the env-injection
//! helper used by `keys::tests` (`master_seed_gate`): the new
//! `hsm::config::tests` module needs the same shape to drive
//! `HsmConfig::from_env` without mutating process environment. Two
//! byte-identical copies grew side by side; this module collapses
//! them so a future change to the helper is a single-edit operation.
//!
//! Visibility: `pub(crate)`, gated behind `cfg(test)`. Not part of the
//! public lib surface — the helper has no purpose outside the crate's
//! own test harness, and exposing it would only invite drift.

/// V1.10 pinned pubkey for workspace_id `"alice"` derived from
/// [`crate::keys::DEV_MASTER_SEED`] via HKDF-SHA256 with info
/// `"atlas-anchor-v1:alice"`. Base64url-no-pad of the 32-byte
/// verifying-key.
///
/// **Single source of truth.** V1.10 introduced the pin in
/// `keys::tests::workspace_pubkeys_are_pinned`; V1.11 wave-3
/// Phase A added a second consumer in
/// `workspace_signer::tests::pubkey_is_pinned_against_v1_10_goldens`.
/// The two sites must stay in lock-step (any drift means a
/// production pubkey rotation), so the pins live here as a single
/// `pub(crate) const` that both tests reference. Changing this
/// constant intentionally ALSO requires bumping
/// `atlas-trust-core`'s crate version so `VERIFIER_VERSION` cascades
/// through old bundles.
pub(crate) const PINNED_PUBKEY_B64URL_ALICE: &str =
    "HaADbOvQvGRNVJnGFLLjj-qxC-zwReufz-8dAbBu9aY";

/// V1.10 pinned pubkey for workspace_id `"ws-mcp-default"` (the MCP
/// server's `DEFAULT_WORKSPACE`). See
/// [`PINNED_PUBKEY_B64URL_ALICE`] for the lock-step rationale.
pub(crate) const PINNED_PUBKEY_B64URL_WS_MCP_DEFAULT: &str =
    "_7VayPxHeadNxfSOw0p8E5LNXBNP2Mb-cOieCZRZq6M";

/// Build an env reader closure that returns each `(name, value)` pair's
/// value for matching lookups and `None` otherwise.
///
/// Takes ownership of the `&str` inputs via `to_string()` so loop-
/// variable callers (where the per-iteration `v` is borrowed, not
/// `'static`) compile without lifetime gymnastics. The returned
/// closure satisfies the `Fn(&str) -> Option<String>` bound that
/// every V1.10+ env-driven gate accepts (`master_seed_gate_with`,
/// `master_seed_loader_with`, `HsmConfig::from_env`).
pub(crate) fn env_pairs(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let owned: Vec<(String, String)> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    move |name| {
        owned
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }
}
