//! Ed25519 signature verification.
//!
//! We use ed25519-dalek's strict mode (RFC 8032 conformance).
//! No malleable signatures accepted.

use ed25519_dalek::{Signature, VerifyingKey};

use crate::error::{TrustError, TrustResult};

/// Verify an Ed25519 signature over `message` using `pubkey`.
///
/// `pubkey` must be exactly 32 bytes. `signature` must be exactly 64 bytes.
/// Returns `Ok(())` if valid, `Err(TrustError::BadSignature)` otherwise.
pub fn verify_signature(
    pubkey_bytes: &[u8],
    message: &[u8],
    signature_bytes: &[u8],
    event_id_for_error: &str,
) -> TrustResult<()> {
    let pubkey_array: [u8; 32] = pubkey_bytes.try_into().map_err(|_| {
        TrustError::BadSignature {
            event_id: event_id_for_error.to_string(),
            reason: format!("pubkey must be 32 bytes, got {}", pubkey_bytes.len()),
        }
    })?;

    let sig_array: [u8; 64] = signature_bytes.try_into().map_err(|_| {
        TrustError::BadSignature {
            event_id: event_id_for_error.to_string(),
            reason: format!("signature must be 64 bytes, got {}", signature_bytes.len()),
        }
    })?;

    let verifying_key = VerifyingKey::from_bytes(&pubkey_array).map_err(|e| {
        TrustError::BadSignature {
            event_id: event_id_for_error.to_string(),
            reason: format!("invalid pubkey: {}", e),
        }
    })?;

    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify_strict(message, &signature)
        .map_err(|e| TrustError::BadSignature {
            event_id: event_id_for_error.to_string(),
            reason: format!("verification failed: {}", e),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn fixed_keypair() -> SigningKey {
        // Deterministic test key — never use in production.
        let secret_bytes = [42u8; 32];
        SigningKey::from_bytes(&secret_bytes)
    }

    #[test]
    fn round_trip_valid_signature() {
        let signing = fixed_keypair();
        let verifying = signing.verifying_key();
        let message = b"atlas-test-message";
        let signature = signing.sign(message);

        let result = verify_signature(
            verifying.as_bytes(),
            message,
            &signature.to_bytes(),
            "test-event",
        );
        assert!(result.is_ok(), "valid signature should verify: {:?}", result);
    }

    #[test]
    fn tampered_message_fails() {
        let signing = fixed_keypair();
        let verifying = signing.verifying_key();
        let message = b"atlas-test-message";
        let signature = signing.sign(message);

        let tampered = b"atlas-test-tampered";
        let result = verify_signature(
            verifying.as_bytes(),
            tampered,
            &signature.to_bytes(),
            "test-event",
        );
        assert!(matches!(result, Err(TrustError::BadSignature { .. })));
    }

    #[test]
    fn wrong_pubkey_fails() {
        let signing = fixed_keypair();
        let other = SigningKey::from_bytes(&[7u8; 32]);
        let message = b"atlas-test-message";
        let signature = signing.sign(message);

        let result = verify_signature(
            other.verifying_key().as_bytes(),
            message,
            &signature.to_bytes(),
            "test-event",
        );
        assert!(matches!(result, Err(TrustError::BadSignature { .. })));
    }
}
