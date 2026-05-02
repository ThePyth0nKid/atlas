//! Test-only helpers shared across the crate's `#[cfg(test)]` modules.
//!
//! V1.10 wave 2 introduced a second consumer for the env-injection
//! helper used by `keys::tests` (`master_seed_gate` /
//! `production_gate_with`): the new `hsm::config::tests` module needs
//! the same shape to drive `HsmConfig::from_env` without mutating
//! process environment. Two byte-identical copies grew side by side;
//! this module collapses them so a future change to the helper is a
//! single-edit operation.
//!
//! Visibility: `pub(crate)`, gated behind `cfg(test)`. Not part of the
//! public lib surface — the helper has no purpose outside the crate's
//! own test harness, and exposing it would only invite drift.

/// Build an env reader closure that returns each `(name, value)` pair's
/// value for matching lookups and `None` otherwise.
///
/// Takes ownership of the `&str` inputs via `to_string()` so loop-
/// variable callers (where the per-iteration `v` is borrowed, not
/// `'static`) compile without lifetime gymnastics. The returned
/// closure satisfies the `Fn(&str) -> Option<String>` bound that
/// every V1.10 env-driven gate accepts (`production_gate_with`,
/// `master_seed_gate_with`, `HsmConfig::from_env`).
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
