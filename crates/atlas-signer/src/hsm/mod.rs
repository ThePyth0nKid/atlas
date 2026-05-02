//! V1.10 wave 2 — HSM-backed sealed-seed loader.
//!
//! `atlas-signer` V1.5–V1.9 read the master seed from a
//! source-committed constant ([`crate::keys::DEV_MASTER_SEED`]); V1.10
//! wave 1 gated that constant behind a positive opt-in
//! ([`crate::keys::master_seed_gate`]); V1.10 wave 2 (this module)
//! adds the alternative — a sealed-seed loader that performs HKDF
//! derivation **inside** an HSM/TPM via PKCS#11. The master seed
//! never enters Atlas address space.
//!
//! ## Trust property
//!
//! The V1.10 master-seed gate inversion (wave 1) made
//! `ATLAS_DEV_MASTER_SEED=1` the audit signal that a deployment is
//! signing with the source-committed dev seed. Wave 2 introduces a
//! second audit signal — `ATLAS_HSM_PKCS11_LIB` /
//! `ATLAS_HSM_SLOT` / `ATLAS_HSM_PIN_FILE` — that names a sealed
//! seed source. A correctly-deployed production environment sets
//! the HSM trio AND `ATLAS_PRODUCTION=1` AND leaves
//! `ATLAS_DEV_MASTER_SEED` unset; an auditor reading
//! `env | grep ATLAS_` can verify the deployment configuration in
//! one snapshot.
//!
//! ## Why a feature, not a sibling crate?
//!
//! V1.10 wave 2 first lived in a sibling `atlas-signer-hsm` crate.
//! That structure was collapsed back into `atlas-signer` once the
//! `master_seed_loader` dispatch wanted to live next to the dev
//! impl: a sibling crate that depends on `atlas-signer` cannot be
//! depended-on by `atlas-signer` (Cargo refuses cycles), and the
//! original arguments for the split — WASM-verifier compile graph,
//! Apache-2.0 boundary — turned out not to apply here. `atlas-verify-wasm`
//! does not depend on `atlas-signer`, so the FFI was never going to
//! reach a WASM target; both crates are Apache-2.0; and the
//! per-operator opt-in is achieved with the `hsm` feature flag
//! instead of crate boundary, with the same audit semantics.
//!
//! ## Status
//!
//! Wave 2 PKCS#11 path is wired end-to-end behind the `hsm` feature:
//! [`pkcs11::Pkcs11MasterSeedHkdf::open`] loads the module, opens an
//! authenticated R/W session, resolves the master seed by
//! `CKA_LABEL`, and `derive_for` runs `CKM_HKDF_DERIVE` (PRF =
//! SHA-256, salt = `HkdfSalt::Null`, info = caller-supplied) inside
//! the token. The 32-byte derived bytes leave the device as a
//! short-lived ephemeral object (CKA_TOKEN=false, CKA_EXTRACTABLE=true)
//! whose handle is destroyed before the call returns — even on the
//! error path. The master seed never enters Atlas address space.

pub mod config;

#[cfg(feature = "hsm")]
pub(crate) mod error;

#[cfg(feature = "hsm")]
pub mod pkcs11;

// Always-available stub so the master-seed loader can refer to the
// type without needing the `hsm` feature. When the feature is on,
// the real impl in [`pkcs11`] shadows this stub.
#[cfg(not(feature = "hsm"))]
pub mod pkcs11 {
    //! Stub PKCS#11 backend. Only compiled when the `hsm` feature
    //! is OFF. Returns
    //! [`MasterSeedError::Unavailable`](crate::keys::MasterSeedError::Unavailable)
    //! from every operation so a deployment that forgot to enable
    //! the feature fails closed with a clear remediation message.

    use crate::hsm::config::HsmConfig;
    use crate::keys::{MasterSeedError, MasterSeedHkdf};

    /// Stub stand-in for the PKCS#11 backend. Refuses every operation
    /// with a clear "compile with --features hsm" remediation.
    #[derive(Debug)]
    pub struct Pkcs11MasterSeedHkdf {
        _config: HsmConfig,
    }

    impl Pkcs11MasterSeedHkdf {
        /// Stub constructor. Always returns
        /// [`MasterSeedError::Unavailable`] because the `hsm`
        /// feature is not compiled in.
        pub fn open(_config: HsmConfig) -> Result<Self, MasterSeedError> {
            Err(MasterSeedError::Unavailable(
                "atlas-signer built without the `hsm` feature; \
                 rebuild with `--features hsm` to enable the sealed-seed loader"
                    .to_string(),
            ))
        }
    }

    impl MasterSeedHkdf for Pkcs11MasterSeedHkdf {
        fn derive_for(
            &self,
            _info: &[u8],
            _out: &mut [u8; 32],
        ) -> Result<(), MasterSeedError> {
            Err(MasterSeedError::Unavailable(
                "Pkcs11MasterSeedHkdf is a stub (hsm feature not enabled)"
                    .to_string(),
            ))
        }
    }
}
