//! File-backed Ed25519 witness implementation.
//!
//! Holds a 32-byte Ed25519 secret key in a `Zeroizing` wrapper so the
//! material is scrubbed from heap on drop (V1.10 footgun #17). The
//! public key is computed lazily from the secret on each call — there
//! is no permanent in-memory copy of the verifying key bytes, only the
//! secret material.
//!
//! HSM-backed witness implementations are deferred to a later wave;
//! the `Witness` trait shape is intentionally HSM-friendly so dropping
//! a PKCS#11-backed impl in alongside this one needs no caller change.

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use zeroize::Zeroizing;

use atlas_trust_core::{decode_chain_head, witness_signing_input, WitnessSig};

use crate::Witness;

/// File-backed Ed25519 witness. The secret bytes are wrapped in
/// `Zeroizing` so they are scrubbed from memory when the witness is
/// dropped.
///
/// Construction takes the secret bytes by value to discourage callers
/// from keeping an un-zeroized copy.
pub struct Ed25519Witness {
    witness_kid: String,
    /// Wrapped to scrub on drop — V1.10 footgun #17 (zeroize at every
    /// heap-resident copy of secret material).
    secret_bytes: Zeroizing<[u8; 32]>,
}

impl Ed25519Witness {
    /// Construct a witness from a 32-byte raw Ed25519 secret + a kid.
    ///
    /// The kid must match the entry under which the corresponding
    /// public key sits in `atlas_trust_core::ATLAS_WITNESS_V1_ROSTER`,
    /// otherwise the verifier will refuse the signature (unknown kid).
    pub fn new(witness_kid: String, secret_bytes: [u8; 32]) -> Self {
        Self {
            witness_kid,
            secret_bytes: Zeroizing::new(secret_bytes),
        }
    }

    /// Return the corresponding 32-byte Ed25519 public key.
    ///
    /// Useful for the witness commissioning ceremony: the operator
    /// generates a keypair, prints this pubkey, and pastes it into
    /// `ATLAS_WITNESS_V1_ROSTER` in source.
    pub fn pubkey_bytes(&self) -> [u8; 32] {
        let sk = SigningKey::from_bytes(&self.secret_bytes);
        sk.verifying_key().to_bytes()
    }
}

impl Witness for Ed25519Witness {
    fn witness_kid(&self) -> &str {
        &self.witness_kid
    }

    fn sign_chain_head(&self, chain_head_hex: &str) -> Result<WitnessSig, String> {
        let chain_head_bytes =
            decode_chain_head(chain_head_hex).map_err(|e| e.to_string())?;
        let signing_input = witness_signing_input(&chain_head_bytes);

        let sk = SigningKey::from_bytes(&self.secret_bytes);
        let sig = sk.sign(&signing_input);

        Ok(WitnessSig {
            witness_kid: self.witness_kid.clone(),
            // URL-safe base64, no padding — same dialect as
            // `EventSignature.sig` and the verifier's
            // `verify_witness_against_roster` decoder. One dialect
            // across the whole wire format.
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_trust_core::verify_witness_against_roster;

    /// Round-trip: a witness signs a head, the trust-core verifier
    /// accepts the resulting `WitnessSig` against the corresponding
    /// pubkey in a roster. This is the load-bearing contract for
    /// V1.13 Scope C wave 1 — signer and verifier agree on the
    /// signing-input + verification rule byte-for-byte.
    #[test]
    fn round_trip_against_roster() {
        let secret = [42u8; 32];
        let witness = Ed25519Witness::new("witness-rt-1".to_string(), secret);
        let pubkey = witness.pubkey_bytes();

        let chain_head_hex = "abcd".repeat(16);
        let sig = witness.sign_chain_head(&chain_head_hex).expect("sign");

        let roster: &[(&str, [u8; 32])] = &[("witness-rt-1", pubkey)];
        verify_witness_against_roster(&sig, &chain_head_hex, roster)
            .expect("witness should verify against own roster");
    }

    /// Same witness signs different heads → different sigs. Defends
    /// against accidentally returning a pre-cached or constant sig.
    #[test]
    fn different_heads_yield_different_sigs() {
        let witness = Ed25519Witness::new("witness-diff".to_string(), [42u8; 32]);
        let head_a = "abcd".repeat(16);
        let head_b = "0123".repeat(16);

        let sig_a = witness.sign_chain_head(&head_a).unwrap();
        let sig_b = witness.sign_chain_head(&head_b).unwrap();
        assert_ne!(sig_a.signature, sig_b.signature);
        // Kid stays constant — only the signature bytes change.
        assert_eq!(sig_a.witness_kid, sig_b.witness_kid);
    }

    /// Same secret + same head + same kid → byte-identical sig
    /// (Ed25519 deterministic by RFC 8032). Defends against an
    /// accidental introduction of randomness on the witness side.
    #[test]
    fn signing_is_deterministic() {
        let witness_a = Ed25519Witness::new("witness-det".to_string(), [42u8; 32]);
        let witness_b = Ed25519Witness::new("witness-det".to_string(), [42u8; 32]);
        let head = "abcd".repeat(16);

        let sig_a = witness_a.sign_chain_head(&head).unwrap();
        let sig_b = witness_b.sign_chain_head(&head).unwrap();
        assert_eq!(
            sig_a, sig_b,
            "Ed25519 must be deterministic — non-determinism here would \
             defeat reproducible attestation",
        );
    }

    /// Pubkey derivation is deterministic from the secret. Useful for
    /// the operator commissioning ceremony: paste pubkey from one
    /// invocation into the roster, sign with the same secret on
    /// another invocation, sigs verify.
    #[test]
    fn pubkey_derivation_is_deterministic() {
        let w1 = Ed25519Witness::new("w".to_string(), [7u8; 32]);
        let w2 = Ed25519Witness::new("w".to_string(), [7u8; 32]);
        assert_eq!(w1.pubkey_bytes(), w2.pubkey_bytes());
    }

    /// Malformed chain head (wrong length) bubbles up the trust-core
    /// `Encoding` error as a String — the trait's String boundary keeps
    /// the dispatcher uniform.
    #[test]
    fn malformed_chain_head_returns_error_string() {
        let witness = Ed25519Witness::new("w".to_string(), [42u8; 32]);
        let result = witness.sign_chain_head("abcd"); // 2 bytes, not 32
        assert!(result.is_err());
    }
}
