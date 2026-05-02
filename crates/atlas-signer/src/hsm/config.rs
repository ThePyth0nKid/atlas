//! V1.10 wave 2 — HSM configuration via environment variables.
//!
//! The V1.10 master-seed gate selects between
//! [`crate::keys::DevMasterSeedHkdf`] and
//! [`crate::hsm::pkcs11::Pkcs11MasterSeedHkdf`] at startup time by
//! inspecting environment variables. This module owns the parsing
//! and validation of the HSM trio:
//!
//!   * [`PKCS11_LIB_ENV`] — absolute path to the PKCS#11 module
//!     (`.so` / `.dylib` / `.dll`). Required.
//!   * [`SLOT_ENV`] — token slot number (decimal integer). Required.
//!   * [`PIN_FILE_ENV`] — path to a file containing the user PIN.
//!     Required. The PIN is read once at startup; the file is not
//!     re-read after `Pkcs11MasterSeedHkdf::open` returns.
//!
//! ## Why a PIN *file* and not a PIN env var?
//!
//! Three reasons:
//!
//! 1. **No environment leak.** `env(1)` in a process snapshot, a
//!    `ps eww` output, a crash dump, a debug trace — any of these
//!    would expose a PIN passed in env. A path to a file does not
//!    leak the secret.
//! 2. **Filesystem permissions.** A PIN file can have explicit
//!    `0400` mode bits; an env var inherits process semantics.
//! 3. **Container-ready.** Docker secrets, Kubernetes secrets,
//!    systemd `LoadCredential=` all naturally produce a file path
//!    in a tmpfs, not an env var. Aligning to the
//!    file-path convention removes a deployment friction.
//!
//! ## Why all three are required
//!
//! Partial config is the most common operator footgun: setting only
//! `ATLAS_HSM_PKCS11_LIB` without `ATLAS_HSM_SLOT` would leave the
//! loader to guess the slot, which is exactly the silent-fallback
//! class V1.10 is closing. The trio is atomic: either all three are
//! set (HSM mode) or none (dev mode); anything in between is a
//! configuration error and refuses to start.

use std::path::PathBuf;

/// Environment variable that names the PKCS#11 module path.
///
/// Must be an *absolute* filesystem path. Relative paths are refused
/// at [`HsmConfig::from_env`] time because a relative path would resolve
/// against the signer's working directory at `Pkcs11::new` time — if an
/// attacker can influence that directory, they can plant a malicious
/// shared library that runs in the signer's address space with access
/// to the master-seed session. The absolute-path guard closes that
/// library-hijack vector at config-parse time.
pub const PKCS11_LIB_ENV: &str = "ATLAS_HSM_PKCS11_LIB";

/// Environment variable that names the token slot number (decimal).
pub const SLOT_ENV: &str = "ATLAS_HSM_SLOT";

/// Environment variable that names the path to the PIN file.
pub const PIN_FILE_ENV: &str = "ATLAS_HSM_PIN_FILE";

/// CKA_LABEL of the master seed object inside the HSM token.
/// Operators set this label during the import ceremony (see
/// `OPERATOR-RUNBOOK.md`); the loader looks it up by label rather
/// than by handle so import + load can run in different processes
/// without coordinating session-scoped handles.
///
/// Default value picked deliberately to be globally unambiguous:
/// any operator who sees a PKCS#11 object with this label knows it
/// is the Atlas master seed, not an unrelated secret. Override is
/// not currently exposed — keeping the label hard-coded is a feature
/// for the V1.10 wave 2 ceremony, since multi-master-seed
/// deployments are an explicit non-goal.
pub const MASTER_SEED_LABEL: &str = "atlas-master-seed-v1";

/// Parsed HSM configuration. Construction validates the env trio
/// is fully present, but does NOT attempt to open the PKCS#11
/// module — that happens in `Pkcs11MasterSeedHkdf::open`, which
/// can fail with [`MasterSeedError::Unavailable`] for runtime
/// reasons (module missing, slot empty, PIN wrong) the env
/// validation cannot catch.
///
/// Implementing `Debug` deliberately — none of the fields are
/// secret. The PIN itself is read from `pin_file` at open time
/// and lives only inside the loader's session; the *path* is
/// not sensitive.
///
/// [`MasterSeedError::Unavailable`]: crate::keys::MasterSeedError::Unavailable
#[derive(Debug, Clone)]
pub struct HsmConfig {
    /// Path to the PKCS#11 module shared library.
    pub module_path: PathBuf,
    /// Token slot number, decimal.
    pub slot: u64,
    /// Path to the user PIN file. Read once at open time.
    pub pin_file: PathBuf,
}

impl HsmConfig {
    /// Parse an [`HsmConfig`] from the V1.10 env trio
    /// (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`,
    /// `ATLAS_HSM_PIN_FILE`).
    ///
    /// Returns `Ok(Some(cfg))` when all three env vars are set
    /// and parse cleanly; `Ok(None)` when none are set (caller
    /// falls back to dev seed via the V1.10 wave-1 gate); and
    /// `Err(message)` when the env is partial (one or two of the
    /// trio set but not all three) or a value fails parsing.
    /// "Partial" is a configuration error, not a fall-through —
    /// the operator clearly intended HSM mode but mis-typed the
    /// trio, and silent fallback would defeat the audit signal.
    ///
    /// The `env` closure is the same injection style as
    /// [`crate::keys::master_seed_gate_with`]: tests
    /// drive the parser without mutating process env, and the
    /// V1.10 wave-2 binary integration shares a single env
    /// reader between this and the master-seed gate.
    pub fn from_env<F>(env: F) -> Result<Option<HsmConfig>, String>
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
                // Require an absolute module path. A relative path
                // resolves against the loader's CWD at `Pkcs11::new`
                // time — if an attacker can influence that CWD (a
                // working directory under `/tmp`, a workspace dir
                // controlled by an MCP-driven flow), they can plant
                // a malicious shared library that the loader will
                // happily dlopen and that *runs in the signer's
                // address space* with full access to the master-seed
                // session. Absolute paths foreclose that whole class
                // of library-hijack attacks at config-parse time.
                let module_path = PathBuf::from(m);
                if !module_path.is_absolute() {
                    return Err(format!(
                        "{PKCS11_LIB_ENV} value {m:?} must be an absolute path \
                         (relative paths resolve against the signer's working \
                         directory, which is a library-hijack vector)",
                    ));
                }
                let slot_num: u64 = s.parse().map_err(|e| {
                    format!("{SLOT_ENV} value {s:?} is not a non-negative integer: {e}")
                })?;
                Ok(Some(HsmConfig {
                    module_path,
                    slot: slot_num,
                    pin_file: PathBuf::from(p),
                }))
            }
            _ => Err(format!(
                "HSM env trio is partial — set ALL of {PKCS11_LIB_ENV}, \
                 {SLOT_ENV}, and {PIN_FILE_ENV} together, or NONE \
                 (which falls back to the V1.10 wave-1 dev-seed gate). \
                 Saw module={module:?}, slot={slot:?}, pin_file={pin:?}.",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Platform-appropriate absolute path stand-ins. `Path::is_absolute()`
    /// requires a drive-letter prefix on Windows but not on Unix, so the
    /// V1.10 absolute-path guard sees `"/path"` and `"C:\\path"`
    /// differently. Tests pin to whichever is absolute on the host.
    #[cfg(windows)]
    const ABS_MODULE: &str = "C:\\usr\\lib\\softhsm\\libsofthsm2.so";
    #[cfg(not(windows))]
    const ABS_MODULE: &str = "/usr/lib/softhsm/libsofthsm2.so";

    #[cfg(windows)]
    const ABS_MODULE_SHORT: &str = "C:\\path";
    #[cfg(not(windows))]
    const ABS_MODULE_SHORT: &str = "/path";

    #[cfg(windows)]
    const ABS_MODULE_TRIM: &str = "C:\\path\\to\\module";
    #[cfg(not(windows))]
    const ABS_MODULE_TRIM: &str = "/path/to/module";

    #[cfg(windows)]
    const ABS_PIN: &str = "C:\\run\\secrets\\atlas-hsm-pin";
    #[cfg(not(windows))]
    const ABS_PIN: &str = "/run/secrets/atlas-hsm-pin";

    #[cfg(windows)]
    const ABS_PIN_SHORT: &str = "C:\\pin";
    #[cfg(not(windows))]
    const ABS_PIN_SHORT: &str = "/pin";

    #[cfg(windows)]
    const TRIMMED_MODULE_INPUT: &str = "  C:\\path\\to\\module  ";
    #[cfg(not(windows))]
    const TRIMMED_MODULE_INPUT: &str = "  /path/to/module  ";

    #[cfg(windows)]
    const TRIMMED_PIN_INPUT: &str = "\tC:\\pin\n";
    #[cfg(not(windows))]
    const TRIMMED_PIN_INPUT: &str = "\t/pin\n";

    use crate::test_support::env_pairs;

    #[test]
    fn from_env_returns_none_when_unset() {
        let cfg = HsmConfig::from_env(env_pairs(&[]))
            .expect("unset env is valid (caller falls back to dev seed)");
        assert!(cfg.is_none());
    }

    #[test]
    fn from_env_parses_full_trio() {
        let cfg = HsmConfig::from_env(env_pairs(&[
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
    fn from_env_rejects_partial_trio_module_only() {
        // The "operator typed only one env var" footgun. V1.10's
        // gate-inversion-rationale demands clear refusal here, not
        // silent fallback to dev seed — the operator's intent to
        // use HSM mode is unambiguous.
        let err = HsmConfig::from_env(env_pairs(&[(PKCS11_LIB_ENV, ABS_MODULE)])).unwrap_err();
        assert!(err.contains("partial"));
        assert!(err.contains(SLOT_ENV));
        assert!(err.contains(PIN_FILE_ENV));
    }

    #[test]
    fn from_env_rejects_partial_trio_two_of_three() {
        for missing in [PKCS11_LIB_ENV, SLOT_ENV, PIN_FILE_ENV] {
            let pairs: Vec<(&str, &str)> = [
                (PKCS11_LIB_ENV, ABS_MODULE_SHORT),
                (SLOT_ENV, "0"),
                (PIN_FILE_ENV, ABS_PIN_SHORT),
            ]
            .into_iter()
            .filter(|(k, _)| *k != missing)
            .collect();
            let err = HsmConfig::from_env(env_pairs(&pairs)).unwrap_err();
            assert!(
                err.contains("partial"),
                "missing={missing} should produce 'partial' error; got {err:?}",
            );
        }
    }

    #[test]
    fn from_env_rejects_empty_module_path() {
        let err = HsmConfig::from_env(env_pairs(&[
            (PKCS11_LIB_ENV, ""),
            (SLOT_ENV, "0"),
            (PIN_FILE_ENV, ABS_PIN_SHORT),
        ]))
        .unwrap_err();
        assert!(err.contains(PKCS11_LIB_ENV));
        assert!(err.contains("empty"));
    }

    #[test]
    fn from_env_rejects_empty_slot() {
        // Mirrors the per-trio empty-after-trim guard for
        // `ATLAS_HSM_SLOT`. Without this, `s.parse::<u64>()` returns
        // an `IntErrorKind::Empty` whose `Display` reads "cannot
        // parse integer from empty string" — accurate but not
        // operator-friendly. The dedicated guard produces a uniform
        // "<env> is set but empty" message across all three trio
        // variables, so the same partial-config diagnostic muscle
        // memory works for any of them.
        for value in ["", "   ", "\t\n"] {
            let err = HsmConfig::from_env(env_pairs(&[
                (PKCS11_LIB_ENV, ABS_MODULE_SHORT),
                (SLOT_ENV, value),
                (PIN_FILE_ENV, ABS_PIN_SHORT),
            ]))
            .unwrap_err();
            assert!(
                err.contains(SLOT_ENV) && err.contains("empty"),
                "empty slot value {value:?} should yield clear empty-set message; got {err:?}",
            );
        }
    }

    #[test]
    fn from_env_rejects_empty_pin_path() {
        let err = HsmConfig::from_env(env_pairs(&[
            (PKCS11_LIB_ENV, ABS_MODULE_SHORT),
            (SLOT_ENV, "0"),
            (PIN_FILE_ENV, ""),
        ]))
        .unwrap_err();
        assert!(err.contains(PIN_FILE_ENV));
        assert!(err.contains("empty"));
    }

    #[test]
    fn from_env_rejects_relative_module_path() {
        // Library-hijack defence: a relative module path would resolve
        // against the signer's CWD at `Pkcs11::new` time. If an attacker
        // can influence the CWD (via a workspace-controlled tmpfs, an
        // MCP-driven flow that picks the working dir), they can plant
        // a malicious shared library that the loader will dlopen with
        // full access to the master-seed session. Refuse at config
        // parse time so the failure is loud, not silent.
        for relative in [
            "libsofthsm2.so",
            "./libsofthsm2.so",
            "lib/libsofthsm2.so",
            "../usr/lib/libsofthsm2.so",
        ] {
            let err = HsmConfig::from_env(env_pairs(&[
                (PKCS11_LIB_ENV, relative),
                (SLOT_ENV, "0"),
                (PIN_FILE_ENV, ABS_PIN_SHORT),
            ]))
            .unwrap_err();
            assert!(
                err.contains(PKCS11_LIB_ENV) && err.contains("absolute"),
                "relative path {relative:?} should be rejected with absolute-path message; \
                 got {err:?}",
            );
        }
    }

    #[test]
    fn from_env_rejects_non_integer_slot() {
        let err = HsmConfig::from_env(env_pairs(&[
            (PKCS11_LIB_ENV, ABS_MODULE_SHORT),
            (SLOT_ENV, "notanumber"),
            (PIN_FILE_ENV, ABS_PIN_SHORT),
        ]))
        .unwrap_err();
        assert!(err.contains(SLOT_ENV));
        assert!(err.contains("notanumber"));
    }

    #[test]
    fn from_env_rejects_negative_slot() {
        let err = HsmConfig::from_env(env_pairs(&[
            (PKCS11_LIB_ENV, ABS_MODULE_SHORT),
            (SLOT_ENV, "-1"),
            (PIN_FILE_ENV, ABS_PIN_SHORT),
        ]))
        .unwrap_err();
        assert!(err.contains(SLOT_ENV));
    }

    #[test]
    fn from_env_tolerates_surrounding_whitespace() {
        // Mirrors the V1.10 master_seed_gate's whitespace tolerance:
        // operator quoting/escaping in shell scripts often produces
        // stray spaces. We tolerate them on the trio just as we do
        // on the opt-in env var.
        let cfg = HsmConfig::from_env(env_pairs(&[
            (PKCS11_LIB_ENV, TRIMMED_MODULE_INPUT),
            (SLOT_ENV, " 5 "),
            (PIN_FILE_ENV, TRIMMED_PIN_INPUT),
        ]))
        .expect("trimmed trio is valid")
        .expect("trimmed trio yields Some");
        assert_eq!(cfg.module_path, PathBuf::from(ABS_MODULE_TRIM));
        assert_eq!(cfg.slot, 5);
        assert_eq!(cfg.pin_file, PathBuf::from(ABS_PIN_SHORT));
    }

    #[test]
    fn from_env_accepts_high_slot_numbers() {
        // PKCS#11 slot IDs are CK_SLOT_ID = unsigned long; high
        // numbers are legitimate (some HSMs expose slots in the
        // tens of millions). Pin a representative high value to
        // guard against an accidental u32 narrowing.
        let cfg = HsmConfig::from_env(env_pairs(&[
            (PKCS11_LIB_ENV, ABS_MODULE_SHORT),
            (SLOT_ENV, "12345678901"),
            (PIN_FILE_ENV, ABS_PIN_SHORT),
        ]))
        .expect("high slot is valid")
        .expect("Some");
        assert_eq!(cfg.slot, 12_345_678_901);
    }
}
