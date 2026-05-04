//! V1.14 Scope I — HSM-backed witness module.
//!
//! Mirrors `atlas-signer`'s `hsm` module structure with one
//! conceptual change: the witness has no master-seed loader (V1.13's
//! witness is single-key per binary; V1.10 wave-2's HKDF derivation
//! has no analogue here). Module layout:
//!
//!   * [`config`] — `HsmWitnessConfig` + env-var trio parsing.
//!     Always compiled (no feature gate) so the CLI can detect the
//!     trio is unset before deciding which Witness backend to use.
//!   * [`error`] — `cryptoki::error::Error` → `String` cleaving rule
//!     (Locked / Unavailable / SigningFailed prefixes).
//!     `#[cfg(feature = "hsm")]`-gated because `cryptoki` is
//!     optional.
//!   * [`pkcs11`] — `Pkcs11Witness` impl. Real implementation lives
//!     here behind `--features hsm`; without the feature, a stub
//!     returns "compile with --features hsm" remediation from every
//!     operation so misconfigured deployments fail closed.
//!
//! ## Why a feature, not a sibling crate
//!
//! Same rationale as atlas-signer's wave-2 HSM module (V1.10 footgun
//! #13: Cargo's no-cycles rule). The witness's HSM impl wants to
//! live next to the file-backed witness so the CLI dispatcher can
//! pick between them at startup. A sibling crate would either need
//! to re-export the `Witness` trait (creating a public crate-graph
//! cycle) or live as a third crate that depends on both
//! `atlas-witness` and `cryptoki`, which would multiply the build-
//! graph surface for no benefit. The feature flag achieves the same
//! per-operator opt-in (no `cryptoki` in the dependency closure for
//! deployments that don't want it) with the same audit semantics
//! (`cargo tree --features hsm`).

pub mod config;

#[cfg(feature = "hsm")]
mod error;

#[cfg(feature = "hsm")]
mod pkcs11;

#[cfg(feature = "hsm")]
pub use pkcs11::Pkcs11Witness;

// Always-available stub so callers can refer to `Pkcs11Witness`
// without needing `--features hsm`. When the feature is on, the
// real impl in `pkcs11` shadows this stub.
#[cfg(not(feature = "hsm"))]
pub use stub::Pkcs11Witness;

#[cfg(not(feature = "hsm"))]
mod stub {
    //! Stub PKCS#11 witness backend. Only compiled when the `hsm`
    //! feature is OFF. Mirrors atlas-signer's wave-2/wave-3 stub:
    //! every operation returns a "compile with `--features hsm`"
    //! remediation string so a deployment that forgot to enable the
    //! feature fails closed with a clear next step.
    use crate::hsm::config::HsmWitnessConfig;
    use crate::Witness;
    use atlas_trust_core::WitnessSig;

    /// Stub stand-in for the PKCS#11 witness. Refuses every
    /// operation with a clear "compile with `--features hsm`"
    /// remediation.
    #[derive(Debug)]
    pub struct Pkcs11Witness {
        _config: HsmWitnessConfig,
        _witness_kid: String,
    }

    impl Pkcs11Witness {
        /// Stub constructor. Always returns an `Unavailable:`-prefixed
        /// error because the `hsm` feature is not compiled in. The
        /// prefix matches the production error cleaving rule
        /// (atlas-signer / atlas-witness production builds use the
        /// same `Unavailable:` / `Locked:` / `SigningFailed:`
        /// vocabulary), so an operator runbook entry for "what to do
        /// when atlas-witness logs Unavailable" works on both
        /// production AND default-features misconfiguration.
        pub fn open(_config: HsmWitnessConfig, _witness_kid: String) -> Result<Self, String> {
            Err(
                "Unavailable: atlas-witness built without the `hsm` feature; rebuild \
                 with `--features hsm` to enable the HSM-backed witness backend"
                    .to_string(),
            )
        }

        /// Stub one-shot pubkey extract. Same `Unavailable:` prefix +
        /// rebuild remediation as [`Self::open`]; the CLI's
        /// `extract-pubkey-hex` subcommand surfaces this error to the
        /// operator running the commissioning ceremony so they catch
        /// the missing-feature footgun before pasting an empty hex
        /// into `ATLAS_WITNESS_V1_ROSTER`.
        pub fn extract_pubkey_hex(
            _config: HsmWitnessConfig,
            _witness_kid: &str,
        ) -> Result<String, String> {
            Err(
                "Unavailable: atlas-witness built without the `hsm` feature; rebuild \
                 with `--features hsm` to extract a witness public key from the HSM"
                    .to_string(),
            )
        }
    }

    impl Witness for Pkcs11Witness {
        fn witness_kid(&self) -> &str {
            &self._witness_kid
        }

        fn sign_chain_head(&self, _chain_head_hex: &str) -> Result<WitnessSig, String> {
            Err(
                "Unavailable: Pkcs11Witness is a stub (hsm feature not enabled)"
                    .to_string(),
            )
        }
    }

    // Compile-time `Send + Sync` fence — mirrors the real impl in
    // `pkcs11`. Catches a stub regression (e.g. a `Cell` field) at
    // compile time rather than at the embedder's call site.
    const _: () = {
        const fn assert_send_sync<T: Send + Sync>() {}
        let _ = assert_send_sync::<Pkcs11Witness>;
    };
}
