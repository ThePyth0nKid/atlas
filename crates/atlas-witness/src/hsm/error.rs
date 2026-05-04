//! V1.14 Scope I — PKCS#11 error → witness `String` mapping.
//!
//! The `Witness` trait returns `Result<_, String>` (V1.11 footgun #20:
//! the dyn-safe error shape is `String` so a single dispatcher can
//! span heterogeneous error types). The atlas-signer crate uses an
//! enum (`MasterSeedError` / `WorkspaceSignerError`) because its
//! callers want to dispatch on the variant; the witness has no such
//! caller — it is consumed by the CLI binary which prints the error
//! and exits, so the cleaving rule lives in the *prefix* of the
//! string rather than the type.
//!
//! The cleaving rule mirrors atlas-signer's `map_pkcs11_error` /
//! `map_pkcs11_sign_error` so the operator-facing remediation muscle
//! memory transfers between the two binaries:
//!
//!   * `Locked: …`        — token reachable, session unauthenticated.
//!     Operator action: supply the PIN, re-`C_Login`, restart the
//!     witness service.
//!   * `Unavailable: …`   — token not reachable. Operator action:
//!     check module path / slot id / network connectivity to the HSM.
//!   * `SigningFailed: …` — operation off the rails post-login.
//!     Operator action: check key label, mechanism availability,
//!     attribute mismatches.
//!
//! Without a structured error, an operator-facing line lives or dies
//! by the prefix; we standardise the three above so a runbook entry
//! like "Locked: PinIncorrect" matches grep across the whole
//! deployment.

#[cfg(feature = "hsm")]
use cryptoki::error::{Error as Pkcs11Error, RvError};

/// Map a `cryptoki::error::Error` from a witness *open* call site
/// (module load, login, find-objects) to the `String` boundary.
///
/// Mirrors `atlas-signer`'s `map_pkcs11_error`: locked-class RVs go
/// to `Locked:`, unavailable-class to `Unavailable:`, anything else
/// to `SigningFailed:` (catch-all chosen as the safer default for
/// the open path because the operator's next step is "investigate
/// the witness service" rather than "provision a new key").
#[cfg(feature = "hsm")]
pub(crate) fn map_pkcs11_open_error(err: Pkcs11Error) -> String {
    match err {
        Pkcs11Error::Pkcs11(rv, _) => match rv {
            RvError::UserNotLoggedIn
            | RvError::PinIncorrect
            | RvError::PinLocked
            | RvError::PinExpired
            | RvError::PinInvalid
            | RvError::SessionHandleInvalid
            | RvError::SessionClosed => format!("Locked: PKCS#11 {rv:?}"),
            RvError::TokenNotPresent
            | RvError::TokenNotRecognized
            | RvError::SlotIdInvalid
            | RvError::DeviceError
            | RvError::DeviceRemoved
            | RvError::DeviceMemory => format!("Unavailable: PKCS#11 {rv:?}"),
            other => format!("SigningFailed: PKCS#11 {other:?}"),
        },
        Pkcs11Error::LibraryLoading(e) => format!("Unavailable: PKCS#11 library load: {e}"),
        other => format!("SigningFailed: PKCS#11: {other}"),
    }
}

/// Map a `cryptoki::error::Error` from a `C_Sign` call site to the
/// `String` boundary.
///
/// Mirrors `atlas-signer`'s `map_pkcs11_sign_error`: same RV
/// groupings, but the catch-all routes to `SigningFailed:` because
/// at this point the key was findable + the session was logged in
/// — the only remaining failure class is the signing op itself.
/// "When in doubt, SigningFailed is the safer default for a sign
/// path because it implies retry is meaningful" (atlas-signer
/// wave-3 trait doc).
#[cfg(feature = "hsm")]
pub(crate) fn map_pkcs11_sign_error(err: Pkcs11Error) -> String {
    match err {
        Pkcs11Error::Pkcs11(rv, _) => match rv {
            RvError::UserNotLoggedIn
            | RvError::PinIncorrect
            | RvError::PinLocked
            | RvError::PinExpired
            | RvError::PinInvalid
            | RvError::SessionHandleInvalid
            | RvError::SessionClosed => format!("Locked: PKCS#11 {rv:?}"),
            RvError::TokenNotPresent
            | RvError::TokenNotRecognized
            | RvError::SlotIdInvalid
            | RvError::DeviceError
            | RvError::DeviceRemoved
            | RvError::DeviceMemory => format!("Unavailable: PKCS#11 {rv:?}"),
            other => format!("SigningFailed: PKCS#11 {other:?}"),
        },
        Pkcs11Error::LibraryLoading(e) => format!("Unavailable: PKCS#11 library load: {e}"),
        other => format!("SigningFailed: PKCS#11: {other}"),
    }
}

#[cfg(feature = "hsm")]
#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the catch-all arm for both mappers: any non-LibraryLoading,
    /// non-`Pkcs11(rv, fn)` variant goes to `SigningFailed`. The
    /// constructible variants (`NotSupported`, `InvalidValue`) keep
    /// the contract honest against a refactor that special-cases one
    /// and forgets the other. Mirrors atlas-signer's
    /// `map_pkcs11_sign_error_catch_all_routes_to_signing_failed`.
    #[test]
    fn open_mapper_catch_all_routes_to_signing_failed() {
        for witness in [Pkcs11Error::NotSupported, Pkcs11Error::InvalidValue] {
            let msg = map_pkcs11_open_error(witness);
            assert!(
                msg.starts_with("SigningFailed:"),
                "open-path catch-all must start with 'SigningFailed:'; got: {msg}",
            );
            assert!(
                msg.contains("PKCS#11"),
                "message must surface 'PKCS#11' so the operator can grep the trace; got: {msg}",
            );
        }
    }

    #[test]
    fn sign_mapper_catch_all_routes_to_signing_failed() {
        for witness in [Pkcs11Error::NotSupported, Pkcs11Error::InvalidValue] {
            let msg = map_pkcs11_sign_error(witness);
            assert!(
                msg.starts_with("SigningFailed:"),
                "sign-path catch-all must start with 'SigningFailed:'; got: {msg}",
            );
            assert!(msg.contains("PKCS#11"));
        }
    }

    #[test]
    fn rverror_variant_identifiers_compile() {
        // Per-RV cleaving (Locked / Unavailable / SigningFailed) is
        // exercised end-to-end by the byte-equivalence integration
        // test against a live SoftHSM2 token. Pinning the RvError
        // identifiers here ensures a future cryptoki bump that
        // renames a variant trips at unit-test time rather than in
        // the integration lane. Mirrors atlas-signer's pattern.
        let _ = RvError::UserNotLoggedIn;
        let _ = RvError::PinIncorrect;
        let _ = RvError::PinLocked;
        let _ = RvError::PinExpired;
        let _ = RvError::PinInvalid;
        let _ = RvError::SessionHandleInvalid;
        let _ = RvError::SessionClosed;
        let _ = RvError::TokenNotPresent;
        let _ = RvError::TokenNotRecognized;
        let _ = RvError::SlotIdInvalid;
        let _ = RvError::DeviceError;
        let _ = RvError::DeviceRemoved;
        let _ = RvError::DeviceMemory;
    }
}
