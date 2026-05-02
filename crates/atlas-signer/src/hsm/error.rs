//! V1.10 wave 2 — PKCS#11 error → [`MasterSeedError`] mapping.
//!
//! The PKCS#11 spec emits `CKR_*` return codes for every operation;
//! [`crate::keys::MasterSeedError`] partitions them into
//! three operational categories so an operator reading
//! stderr can act on the message:
//!
//!   * [`MasterSeedError::Locked`] — the token is reachable but
//!     the session is unauthenticated. Operator action: supply
//!     the PIN, re-`C_Login`. Triggered by `CKR_USER_NOT_LOGGED_IN`,
//!     `CKR_PIN_INCORRECT`, `CKR_PIN_LOCKED`, `CKR_PIN_EXPIRED`.
//!   * [`MasterSeedError::Unavailable`] — the token is not
//!     reachable. Operator action: check the module path, slot
//!     ID, network connectivity to the HSM. Triggered by
//!     `CKR_TOKEN_NOT_PRESENT`, `CKR_SLOT_ID_INVALID`,
//!     `CKR_DEVICE_ERROR`, `CKR_DEVICE_REMOVED`,
//!     `CKR_LIBRARY_LOAD_FAILED` (synthesised from the OS error).
//!   * [`MasterSeedError::DeriveFailed`] — the operation itself
//!     failed once the session was good. Operator action: check
//!     the master seed object label, mechanism availability,
//!     attribute mismatches. Triggered by `CKR_MECHANISM_INVALID`,
//!     `CKR_KEY_NOT_NEEDED`, `CKR_TEMPLATE_INCONSISTENT`,
//!     `CKR_KEY_HANDLE_INVALID`, anything else.
//!
//! [`MasterSeedError`]: crate::keys::MasterSeedError

#[cfg(any(feature = "hsm", test))]
use crate::keys::MasterSeedError;

#[cfg(feature = "hsm")]
use cryptoki::error::{Error as Pkcs11Error, RvError};

/// Map a `cryptoki::error::Error` (PKCS#11 RV) into the right
/// [`MasterSeedError`] variant. The mapping is best-effort:
/// PKCS#11 modules vary in which RV codes they emit for which
/// failure mode (a PIN-locked YubiHSM returns `CKR_PIN_LOCKED`
/// where SoftHSM2 returns `CKR_PIN_INCORRECT` after N tries).
/// We pick the operator-facing category that minimises confusion.
///
/// The mapping is exposed as a free function (not a `From` impl)
/// because the variant choice is policy, not a structural
/// translation, and some callers want to override the mapping
/// (e.g. the import ceremony's `CKR_KEY_HANDLE_INVALID` is a
/// workflow error, not a derive failure).
#[cfg(feature = "hsm")]
pub(crate) fn map_pkcs11_error(err: Pkcs11Error) -> MasterSeedError {
    match err {
        Pkcs11Error::Pkcs11(rv, _) => match rv {
            RvError::UserNotLoggedIn
            | RvError::PinIncorrect
            | RvError::PinLocked
            | RvError::PinExpired
            | RvError::PinInvalid
            | RvError::SessionHandleInvalid
            | RvError::SessionClosed => {
                MasterSeedError::Locked(format!("PKCS#11 {rv:?}"))
            }
            RvError::TokenNotPresent
            | RvError::TokenNotRecognized
            | RvError::SlotIdInvalid
            | RvError::DeviceError
            | RvError::DeviceRemoved
            | RvError::DeviceMemory => {
                MasterSeedError::Unavailable(format!("PKCS#11 {rv:?}"))
            }
            other => MasterSeedError::DeriveFailed(format!("PKCS#11 {other:?}")),
        },
        // Library-load and FFI errors (cryptoki cannot even reach
        // C_Initialize) are an Unavailable condition: the operator
        // pointed us at a module that isn't there or won't load.
        Pkcs11Error::LibraryLoading(e) => {
            MasterSeedError::Unavailable(format!("PKCS#11 library load: {e}"))
        }
        // Anything else we have not categorised — surface verbatim
        // so the operator can grep the cryptoki source for the
        // variant. DeriveFailed is the safest default category;
        // it pairs with our exit code 2 in the binary.
        other => MasterSeedError::DeriveFailed(format!("PKCS#11: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sanity: the [`MasterSeedError`] variants we map to are the
    /// V1.10 trait surface. If a future refactor renames a variant,
    /// the mapping function fails to compile rather than silently
    /// degrading a Locked error to DeriveFailed (which would lose
    /// operator-actionable information).
    ///
    /// This test does not need the `hsm` feature; it just pins
    /// the variant identifiers via construction. The real mapping
    /// is exercised by the `hsm`-feature-only integration tests
    /// in `pkcs11.rs`.
    #[test]
    fn variant_constructors_compile() {
        let _ = MasterSeedError::Locked("locked".to_string());
        let _ = MasterSeedError::Unavailable("unavail".to_string());
        let _ = MasterSeedError::DeriveFailed("derive".to_string());
    }
}
