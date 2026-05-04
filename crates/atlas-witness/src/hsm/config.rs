//! V1.14 Scope I — HSM-witness configuration via environment variables.
//!
//! Mirrors `atlas-signer`'s `hsm::config` shape with two structural
//! differences:
//!
//!   1. **Env-var prefix is `ATLAS_WITNESS_HSM_*`** (not
//!      `ATLAS_HSM_*`). The atlas-signer and atlas-witness binaries
//!      are operationally separate processes (V1.13 trust-domain
//!      separation), so they each have their own env-var trio. An
//!      operator who accidentally re-uses `ATLAS_HSM_*` for the
//!      witness binary gets a clean "not set" SKIP rather than a
//!      surprise authentication against the signer's HSM token.
//!
//!   2. **Single key per witness binary**, not multi-tenant. The
//!      witness signs over the chain head only — there is no
//!      per-workspace fan-out. The label `WITNESS_LABEL_PREFIX ||
//!      witness_kid` resolves a single Ed25519 keypair. Multi-witness
//!      deployments run multiple `atlas-witness` binaries each pinned
//!      to its own kid + token slot, not one binary cycling through
//!      kids.
//!
//! ## Why a PIN *file* and not a PIN env var?
//!
//! Same rationale as atlas-signer (env leak via `ps eww`, container
//! ergonomics, file-permission surface). See atlas-signer's
//! `hsm::config` for the long form.
//!
//! ## Why all three are required
//!
//! Partial config is the most common operator footgun: setting only
//! `ATLAS_WITNESS_HSM_PKCS11_LIB` without `ATLAS_WITNESS_HSM_SLOT`
//! would leave the loader to guess the slot. Refuse loudly at parse
//! time, not silently fall back to the file-backed witness.

use std::path::PathBuf;

/// Environment variable that names the PKCS#11 module path.
///
/// Must be an *absolute* filesystem path. Relative paths are refused
/// at [`HsmWitnessConfig::from_env`] time because a relative path
/// would resolve against the witness's working directory at
/// `Pkcs11::new` time — if an attacker can influence that directory,
/// they can plant a malicious shared library that runs in the
/// witness's address space with access to the authenticated session.
/// The absolute-path guard closes that library-hijack vector at
/// config-parse time.
pub const PKCS11_LIB_ENV: &str = "ATLAS_WITNESS_HSM_PKCS11_LIB";

/// Environment variable that names the token slot number (decimal).
pub const SLOT_ENV: &str = "ATLAS_WITNESS_HSM_SLOT";

/// Environment variable that names the path to the PIN file.
///
/// Must be an *absolute* filesystem path (same path-confusion
/// rationale as [`PKCS11_LIB_ENV`]). The PIN file must be mode 0400
/// (or 0600) and owned by the witness's runtime user. The hardened
/// reader in [`crate::hsm::pkcs11::read_pin_file_for_witness`]
/// re-checks permission bits at read time on Unix via
/// `metadata()` on the open file descriptor; a PIN file with any
/// group/world access bits set is refused with a `Locked`-equivalent
/// String error.
pub const PIN_FILE_ENV: &str = "ATLAS_WITNESS_HSM_PIN_FILE";

/// CKA_LABEL prefix for witness keypair objects on the HSM token.
///
/// The full label is `format!("{WITNESS_LABEL_PREFIX}{witness_kid}")`.
/// Distinct from atlas-signer's `atlas-workspace-key-v1:` prefix so
/// the two binaries cannot accidentally read each other's keys even
/// if pointed at the same token (defence-in-depth — the production
/// deployment uses separate slots per trust-domain anyway).
///
/// **Versioning.** The `-v1` suffix is a rotation hatch: any future
/// change to the on-token derivation grammar bumps this constant to
/// `-v2`. Old `-v1` objects keep working for legacy witnesses; new
/// commissioning produces `-v2` objects. Mirrors
/// `atlas-signer`'s `WORKSPACE_LABEL_PREFIX` and
/// `crate::keys::HKDF_INFO_PREFIX` versioning conventions.
pub const WITNESS_LABEL_PREFIX: &str = "atlas-witness-key-v1:";

/// Parsed HSM-witness configuration. Construction validates the env
/// trio is fully present and absolute-path-safe, but does NOT attempt
/// to open the PKCS#11 module — that happens in
/// [`crate::hsm::pkcs11::Pkcs11Witness::open`], which can fail at
/// runtime for module-load / slot-empty / wrong-PIN reasons that env
/// validation cannot catch.
///
/// Fields are `pub(crate)` so external embedders cannot construct an
/// unvalidated config that bypasses the absolute-path guards in
/// [`HsmWitnessConfig::from_env`]. The `from_env_for_test` constructor
/// (gated `#[cfg(test)]` *or* `pub`) provides a documented escape
/// hatch for integration tests that build the config from runtime
/// env they have already validated themselves.
///
/// `Debug` is intentionally derived — none of the fields are secret.
/// The PIN itself is read from `pin_file` at open time; the *path*
/// is not sensitive.
///
/// `#[allow(dead_code)]` on the fields: the accessor methods
/// (`pin_file`, `module_path`, `slot`) are gated `#[cfg(feature =
/// "hsm")]` — under default features the fields are written by
/// `from_env`/`from_env_for_test` and read only by the `Debug`
/// derive + `#[cfg(test)]` assertions in this module's tests. The
/// dead-code lint fires for non-test default-features builds; the
/// allow is the targeted suppression rather than gating the whole
/// struct.
#[derive(Debug, Clone)]
pub struct HsmWitnessConfig {
    /// Path to the PKCS#11 module shared library. Absolute.
    #[allow(dead_code)]
    pub(crate) module_path: PathBuf,
    /// Token slot number, decimal.
    #[allow(dead_code)]
    pub(crate) slot: u64,
    /// Path to the user PIN file. Absolute. Read once at open time.
    #[allow(dead_code)]
    pub(crate) pin_file: PathBuf,
}

impl HsmWitnessConfig {
    /// Parse an [`HsmWitnessConfig`] from the V1.14 env trio
    /// (`ATLAS_WITNESS_HSM_PKCS11_LIB`, `ATLAS_WITNESS_HSM_SLOT`,
    /// `ATLAS_WITNESS_HSM_PIN_FILE`).
    ///
    /// Returns `Ok(Some(cfg))` when all three env vars are set and
    /// parse cleanly; `Ok(None)` when none are set (caller falls back
    /// to the file-backed witness via the CLI's `--secret-file`
    /// flag); and `Err(message)` when the env is partial or a value
    /// fails parsing. "Partial" is a configuration error, not a
    /// fall-through — the operator clearly intended HSM mode but
    /// mis-typed the trio, and silent fallback would mask the typo.
    ///
    /// The `env` closure mirrors the atlas-signer
    /// `HsmConfig::from_env<F>` injection style: tests drive the
    /// parser without mutating process env.
    pub fn from_env<F>(env: F) -> Result<Option<HsmWitnessConfig>, String>
    where
        F: Fn(&str) -> Option<String>,
    {
        let module = env(PKCS11_LIB_ENV);
        let slot = env(SLOT_ENV);
        let pin = env(PIN_FILE_ENV);

        match (module.as_deref(), slot.as_deref(), pin.as_deref()) {
            (None, None, None) => Ok(None),
            (Some(m), Some(s), Some(p)) => {
                let m = m.trim();
                let s = s.trim();
                let p = p.trim();
                if m.is_empty() {
                    return Err(format!("{PKCS11_LIB_ENV} is set but empty"));
                }
                if s.is_empty() {
                    return Err(format!("{SLOT_ENV} is set but empty"));
                }
                if p.is_empty() {
                    return Err(format!("{PIN_FILE_ENV} is set but empty"));
                }
                let module_path = PathBuf::from(m);
                if !module_path.is_absolute() {
                    return Err(format!(
                        "{PKCS11_LIB_ENV} value {m:?} must be an absolute path \
                         (relative paths resolve against the witness's working \
                         directory, which is a library-hijack vector)",
                    ));
                }
                let slot_num: u64 = s.parse().map_err(|e| {
                    format!("{SLOT_ENV} value {s:?} is not a non-negative integer: {e}")
                })?;
                let pin_file = PathBuf::from(p);
                if !pin_file.is_absolute() {
                    return Err(format!(
                        "{PIN_FILE_ENV} value {p:?} must be an absolute path \
                         (relative paths resolve against the witness's working \
                         directory and are a path-confusion vector)",
                    ));
                }
                Ok(Some(HsmWitnessConfig {
                    module_path,
                    slot: slot_num,
                    pin_file,
                }))
            }
            _ => Err(format!(
                "HSM-witness env trio is partial — set ALL of {PKCS11_LIB_ENV}, \
                 {SLOT_ENV}, and {PIN_FILE_ENV} together, or NONE (which falls \
                 back to the file-backed witness via the CLI's `--secret-file` \
                 flag). Saw module={module:?}, slot={slot:?}, pin_file={pin:?}.",
            )),
        }
    }

    /// Test-only constructor for integration tests that have already
    /// validated absolute-path semantics through their own
    /// `require_runtime_gate` helper. Skips the env-trio parse path
    /// because the caller already holds parsed `PathBuf` + `u64`
    /// values.
    ///
    /// Public (not `#[cfg(test)]` gated) because the integration test
    /// in `tests/hsm_witness_byte_equivalence.rs` lives in a separate
    /// compile unit and cannot see `#[cfg(test)]` items in the lib.
    /// The naming convention (`_for_test`) flags the escape hatch so
    /// production embedders are not tempted to bypass `from_env`.
    #[doc(hidden)]
    pub fn from_env_for_test(module_path: PathBuf, slot: u64, pin_file: PathBuf) -> Self {
        Self {
            module_path,
            slot,
            pin_file,
        }
    }

    /// Read-only view of the configured PIN file path. Used by
    /// [`crate::hsm::pkcs11::read_pin_file_for_witness`] which lives in
    /// the same crate but in a sibling module — Rust visibility makes
    /// the field accessible to siblings via `pub(crate)`, so the
    /// accessor is just for clarity at the call site.
    #[cfg(feature = "hsm")]
    pub(crate) fn pin_file(&self) -> &std::path::Path {
        &self.pin_file
    }

    /// Read-only view of the module path. Same accessor pattern as
    /// `pin_file()`.
    #[cfg(feature = "hsm")]
    pub(crate) fn module_path(&self) -> &std::path::Path {
        &self.module_path
    }

    /// Read-only view of the slot id. Same accessor pattern as
    /// `pin_file()`.
    #[cfg(feature = "hsm")]
    pub(crate) fn slot(&self) -> u64 {
        self.slot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Platform-appropriate absolute path stand-ins. Mirrors the
    /// atlas-signer test harness — `Path::is_absolute()` requires a
    /// drive-letter prefix on Windows but not on Unix.
    #[cfg(windows)]
    const ABS_MODULE: &str = "C:\\usr\\lib\\softhsm\\libsofthsm2.so";
    #[cfg(not(windows))]
    const ABS_MODULE: &str = "/usr/lib/softhsm/libsofthsm2.so";

    #[cfg(windows)]
    const ABS_PIN: &str = "C:\\run\\secrets\\atlas-witness-hsm-pin";
    #[cfg(not(windows))]
    const ABS_PIN: &str = "/run/secrets/atlas-witness-hsm-pin";

    /// Inline injection helper — mirrors atlas-signer's `env_pairs`
    /// shape but lives in this module for test-locality (no shared
    /// `test_support` crate yet for atlas-witness).
    fn env_pairs<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn from_env_returns_none_when_unset() {
        let cfg = HsmWitnessConfig::from_env(env_pairs(&[]))
            .expect("unset env is valid (caller falls back to file-backed)");
        assert!(cfg.is_none());
    }

    #[test]
    fn from_env_parses_full_trio() {
        let cfg = HsmWitnessConfig::from_env(env_pairs(&[
            (PKCS11_LIB_ENV, ABS_MODULE),
            (SLOT_ENV, "0"),
            (PIN_FILE_ENV, ABS_PIN),
        ]))
        .expect("full trio is valid")
        .expect("full trio yields Some");
        assert_eq!(cfg.module_path, PathBuf::from(ABS_MODULE));
        assert_eq!(cfg.slot, 0);
        assert_eq!(cfg.pin_file, PathBuf::from(ABS_PIN));
    }

    #[test]
    fn from_env_rejects_partial_trio_two_of_three() {
        for missing in [PKCS11_LIB_ENV, SLOT_ENV, PIN_FILE_ENV] {
            let pairs: Vec<(&str, &str)> = [
                (PKCS11_LIB_ENV, ABS_MODULE),
                (SLOT_ENV, "0"),
                (PIN_FILE_ENV, ABS_PIN),
            ]
            .into_iter()
            .filter(|(k, _)| *k != missing)
            .collect();
            let err = HsmWitnessConfig::from_env(env_pairs(&pairs)).unwrap_err();
            assert!(
                err.contains("partial"),
                "missing={missing} should produce 'partial' error; got {err:?}",
            );
        }
    }

    #[test]
    fn from_env_rejects_relative_module_path() {
        for relative in [
            "libsofthsm2.so",
            "./libsofthsm2.so",
            "lib/libsofthsm2.so",
            "../usr/lib/libsofthsm2.so",
        ] {
            let err = HsmWitnessConfig::from_env(env_pairs(&[
                (PKCS11_LIB_ENV, relative),
                (SLOT_ENV, "0"),
                (PIN_FILE_ENV, ABS_PIN),
            ]))
            .unwrap_err();
            assert!(
                err.contains(PKCS11_LIB_ENV) && err.contains("absolute"),
                "relative module path {relative:?} should be rejected with absolute-path \
                 message; got {err:?}",
            );
        }
    }

    #[test]
    fn from_env_rejects_relative_pin_path() {
        for relative in ["hsm.pin", "./hsm.pin", "secrets/hsm.pin"] {
            let err = HsmWitnessConfig::from_env(env_pairs(&[
                (PKCS11_LIB_ENV, ABS_MODULE),
                (SLOT_ENV, "0"),
                (PIN_FILE_ENV, relative),
            ]))
            .unwrap_err();
            assert!(
                err.contains(PIN_FILE_ENV) && err.contains("absolute"),
                "relative pin path {relative:?} should be rejected with absolute-path \
                 message; got {err:?}",
            );
        }
    }

    #[test]
    fn from_env_rejects_empty_values() {
        // All three trio members independently rejected when empty —
        // pins the per-trio empty-after-trim guard so a regression
        // that drops one branch of the empty check is caught here
        // rather than at runtime when the operator deploys.
        for empty in ["", "   ", "\t\n"] {
            // Empty module
            let err = HsmWitnessConfig::from_env(env_pairs(&[
                (PKCS11_LIB_ENV, empty),
                (SLOT_ENV, "0"),
                (PIN_FILE_ENV, ABS_PIN),
            ]))
            .unwrap_err();
            assert!(err.contains(PKCS11_LIB_ENV) && err.contains("empty"));

            // Empty slot
            let err = HsmWitnessConfig::from_env(env_pairs(&[
                (PKCS11_LIB_ENV, ABS_MODULE),
                (SLOT_ENV, empty),
                (PIN_FILE_ENV, ABS_PIN),
            ]))
            .unwrap_err();
            assert!(err.contains(SLOT_ENV) && err.contains("empty"));

            // Empty pin
            let err = HsmWitnessConfig::from_env(env_pairs(&[
                (PKCS11_LIB_ENV, ABS_MODULE),
                (SLOT_ENV, "0"),
                (PIN_FILE_ENV, empty),
            ]))
            .unwrap_err();
            assert!(err.contains(PIN_FILE_ENV) && err.contains("empty"));
        }
    }

    #[test]
    fn from_env_rejects_non_integer_slot() {
        let err = HsmWitnessConfig::from_env(env_pairs(&[
            (PKCS11_LIB_ENV, ABS_MODULE),
            (SLOT_ENV, "notanumber"),
            (PIN_FILE_ENV, ABS_PIN),
        ]))
        .unwrap_err();
        assert!(err.contains(SLOT_ENV));
        assert!(err.contains("notanumber"));
    }

    #[test]
    fn from_env_tolerates_surrounding_whitespace() {
        let cfg = HsmWitnessConfig::from_env(env_pairs(&[
            (PKCS11_LIB_ENV, &format!("  {ABS_MODULE}  ")),
            (SLOT_ENV, " 5 "),
            (PIN_FILE_ENV, &format!("\t{ABS_PIN}\n")),
        ]))
        .expect("trimmed trio is valid")
        .expect("trimmed trio yields Some");
        assert_eq!(cfg.module_path, PathBuf::from(ABS_MODULE));
        assert_eq!(cfg.slot, 5);
        assert_eq!(cfg.pin_file, PathBuf::from(ABS_PIN));
    }

    #[test]
    fn witness_label_prefix_is_versioned_and_colon_terminated() {
        // On-token namespace contract: prefix MUST end in ':' so the
        // witness_kid parses unambiguously; MUST contain a -vN
        // version segment so a future grammar change has a rotation
        // hatch. Mirrors atlas-signer's `WORKSPACE_LABEL_PREFIX`
        // pinning test.
        assert!(
            WITNESS_LABEL_PREFIX.ends_with(':'),
            "witness label prefix must end with ':'",
        );
        assert!(
            WITNESS_LABEL_PREFIX.contains("-v1:"),
            "witness label prefix must contain a '-v1:' (or successor) version segment",
        );
        assert_eq!(
            WITNESS_LABEL_PREFIX, "atlas-witness-key-v1:",
            "label prefix is part of the runbook + the on-token state; pinning \
             the exact value here so a typo cannot silently rotate the namespace",
        );
        // Distinct from atlas-signer's prefix — a typo collapsing the
        // two would let the witness binary read keys the signer
        // owns. Defence in depth (production uses separate slots
        // anyway, but the label-prefix split is the in-process
        // enforcement).
        assert_ne!(
            WITNESS_LABEL_PREFIX, "atlas-workspace-key-v1:",
            "witness and signer label prefixes MUST be distinct",
        );
    }
}
