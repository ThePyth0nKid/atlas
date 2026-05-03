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
//! the HSM trio AND leaves `ATLAS_DEV_MASTER_SEED` unset (V1.12
//! removed the V1.9 `ATLAS_PRODUCTION` paranoia layer; the HSM
//! trio is now the sole production audit signal). An auditor
//! reading `env | grep ATLAS_` can verify the deployment
//! configuration in one snapshot.
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

#[cfg(feature = "hsm")]
pub mod pkcs11_workspace;

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

// V1.11 wave-3 Phase B counterpart of the wave-2 stub above. Same
// fail-closed shape: build without `--features hsm`, get a struct
// that compiles but refuses every operation with a remediation
// message. The Phase-C dispatcher dispatches into this stub when
// the binary was built without the feature, so a misconfigured
// deployment surfaces a clear error instead of compiling away the
// HSM path silently.
#[cfg(not(feature = "hsm"))]
pub mod pkcs11_workspace {
    //! Stub PKCS#11 workspace-signer backend. Only compiled when
    //! the `hsm` feature is OFF. Mirrors the wave-2
    //! [`pkcs11`](super::pkcs11) stub: every operation returns
    //! [`WorkspaceSignerError::Unavailable`](crate::workspace_signer::WorkspaceSignerError::Unavailable)
    //! with a "rebuild with `--features hsm`" remediation hint.

    use crate::hsm::config::HsmConfig;
    use crate::workspace_signer::{WorkspaceSigner, WorkspaceSignerError};

    /// Stub stand-in for the PKCS#11 wave-3 workspace signer.
    /// Refuses every operation with a clear "compile with
    /// `--features hsm`" remediation.
    #[derive(Debug)]
    pub struct Pkcs11WorkspaceSigner {
        _config: HsmConfig,
    }

    impl Pkcs11WorkspaceSigner {
        /// Stub constructor. Always returns
        /// [`WorkspaceSignerError::Unavailable`] because the `hsm`
        /// feature is not compiled in.
        pub fn open(_config: HsmConfig) -> Result<Self, WorkspaceSignerError> {
            Err(WorkspaceSignerError::Unavailable(
                "atlas-signer built without the `hsm` feature; \
                 rebuild with `--features hsm` to enable the wave-3 \
                 sealed per-workspace signer"
                    .to_string(),
            ))
        }
    }

    impl WorkspaceSigner for Pkcs11WorkspaceSigner {
        fn sign(
            &self,
            _workspace_id: &str,
            _signing_input: &[u8],
        ) -> Result<[u8; 64], WorkspaceSignerError> {
            Err(WorkspaceSignerError::Unavailable(
                "Pkcs11WorkspaceSigner is a stub (hsm feature not enabled)"
                    .to_string(),
            ))
        }

        fn pubkey(
            &self,
            _workspace_id: &str,
        ) -> Result<[u8; 32], WorkspaceSignerError> {
            Err(WorkspaceSignerError::Unavailable(
                "Pkcs11WorkspaceSigner is a stub (hsm feature not enabled)"
                    .to_string(),
            ))
        }
    }

    // Compile-time `Send + Sync` fence — mirrors the real impl in
    // [`super::pkcs11_workspace`]. The Phase-C dispatcher places the
    // workspace signer behind `Arc<dyn WorkspaceSigner + Send + Sync>`,
    // so a stub that silently lost either auto-trait would only fail
    // at the dispatcher's use site (a worse error message than failing
    // here at the type definition).
    const _: () = {
        const fn assert_send_sync<T: Send + Sync>() {}
        let _ = assert_send_sync::<Pkcs11WorkspaceSigner>;
    };
}
