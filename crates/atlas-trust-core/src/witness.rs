//! V1.13 Witness cosignature module.
//!
//! A witness is a second independent attestor that signs over the
//! recomputed anchor-chain head. The verifier resolves a `WitnessSig`'s
//! `witness_kid` against `ATLAS_WITNESS_V1_ROSTER` (a pinned slice of
//! Ed25519 pubkeys) and verifies the signature against
//! `ATLAS_WITNESS_DOMAIN || chain_head_bytes`. Trust property: a valid
//! WitnessSig is evidence that an independent observer saw and attested
//! the chain head — complementing the issuer's own chain integrity check
//! by introducing a second trust domain.
//!
//! Why a separate domain prefix: the witness Ed25519 signing input must
//! not collide with any other Ed25519 signing input in the system (event
//! signatures, anchor-chain checkpoint sigs). Same defence as
//! `ANCHOR_CHAIN_DOMAIN` in `anchor.rs`.
//!
//! Why kid (not inline pubkey) on the wire: an attacker controlling the
//! issuer could otherwise claim any pubkey verified the chain head. The
//! roster lookup forces the verifier to use only pinned, source-controlled
//! keys — adding/removing entries requires a coordinated source change
//! plus a crate-version bump (V1.7 boundary rule).

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::error::{TrustError, TrustResult};

/// Domain-separation prefix mixed into every witness signing input.
///
/// Versioned (`-v1`) so a future canonicalisation change can ship under
/// a new prefix without invalidating already-issued witness sigs. If the
/// signing input changes incompatibly, bump to `-v2` AND bump
/// `atlas-trust-core`'s crate version so `VERIFIER_VERSION` cascades.
pub const ATLAS_WITNESS_DOMAIN: &[u8] = b"atlas-witness-v1:";

/// One witness cosignature over a chain head.
///
/// Wire shape mirrors `EventSignature` (kid + base64 sig) for operator
/// familiarity; the verification rule is different (witness kid resolves
/// via the pinned roster rather than the per-tenant pubkey bundle).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessSig {
    /// Identifier matching an entry in `ATLAS_WITNESS_V1_ROSTER`.
    /// Unknown kid is treated as an invalid witness signature
    /// (NOT silently dropped — a fake kid would otherwise let an
    /// attacker hide rejected witnesses behind unknown identifiers).
    pub witness_kid: String,

    /// URL-safe base64, no padding (RFC 4648 §5) of the 64-byte Ed25519
    /// signature over `ATLAS_WITNESS_DOMAIN || chain_head_bytes` where
    /// `chain_head_bytes` is the raw 32-byte blake3 digest produced by
    /// `chain_head_for` (NOT the hex string — signing the raw bytes
    /// keeps the input shape minimal and avoids two valid encodings of
    /// the same head).
    ///
    /// Dialect rationale: `EventSignature.sig` already uses
    /// URL_SAFE_NO_PAD; aligning here gives one base64 dialect across
    /// the whole wire format. URL-safe is also robust to JSON-in-URL
    /// embedding and grep-friendly (no `+`/`/`/`=` confusion with
    /// regex/path metacharacters).
    pub signature: String,
}

/// Pinned roster of witness Ed25519 pubkeys accepted by this verifier
/// build. Each entry is `(witness_kid, pubkey_bytes)`.
///
/// V1.13 ships with an empty roster — the first witness is commissioned
/// via the OPERATOR-RUNBOOK ceremony (added in V1.13 Wave C-2 doc-sync),
/// which adds an entry here and bumps the crate version per the V1.7
/// boundary rule. With an empty roster, lenient mode (default) still
/// passes because no witnesses are required; strict mode with
/// `--require-witness` only passes when `threshold == 0`.
pub const ATLAS_WITNESS_V1_ROSTER: &[(&str, [u8; 32])] = &[];

/// Construct the canonical bytes a witness signs over.
///
/// `signing_input = ATLAS_WITNESS_DOMAIN || chain_head_bytes`
///
/// Callers who hold a hex chain head should use [`decode_chain_head`]
/// first.
pub fn witness_signing_input(chain_head_bytes: &[u8; 32]) -> Vec<u8> {
    let mut input = Vec::with_capacity(ATLAS_WITNESS_DOMAIN.len() + 32);
    input.extend_from_slice(ATLAS_WITNESS_DOMAIN);
    input.extend_from_slice(chain_head_bytes);
    input
}

/// Decode a hex chain head (64 chars) to its raw 32 bytes.
pub fn decode_chain_head(chain_head_hex: &str) -> TrustResult<[u8; 32]> {
    let raw = hex::decode(chain_head_hex)
        .map_err(|e| TrustError::Encoding(format!("chain_head not hex: {e}")))?;
    raw.try_into().map_err(|v: Vec<u8>| {
        TrustError::Encoding(format!(
            "chain_head must be 32 bytes (64 hex chars), got {} bytes",
            v.len()
        ))
    })
}

/// Verify a single witness signature against a roster.
///
/// Steps (in order, each fail-closed):
///   1. Decode the hex chain head to 32 raw bytes.
///   2. Look up `witness.witness_kid` in `roster` via constant-time
///      string compare. Unknown → fail.
///   3. Decode `witness.signature` from URL-safe base64 (no padding).
///      Malformed or wrong-length → fail.
///   4. Verify Ed25519 strict (RFC 8032) over
///      `ATLAS_WITNESS_DOMAIN || chain_head_bytes`. Failure → fail.
///
/// All failures map to `TrustError::BadWitness` carrying the kid plus a
/// human-readable reason — distinguishable from `BadSignature`
/// (event-level) and `BadAnchor` (Sigstore inclusion-proof) for
/// auditor-facing diagnostics.
///
/// Implementation note: this routes Ed25519 verification directly through
/// `VerifyingKey::verify_strict` rather than `crate::ed25519::verify_signature`.
/// The shared helper hard-wires `BadSignature { event_id, .. }` semantics
/// (event-level trust domain, ULID-shaped identifier), and rewrapping its
/// errors into `BadWitness` after the fact would still leak the
/// event-id semantics into an intermediate state. Inlining keeps the
/// witness trust domain self-contained and the error shape correct from
/// the first failure point.
pub fn verify_witness_against_roster(
    witness: &WitnessSig,
    chain_head_hex: &str,
    roster: &[(&str, [u8; 32])],
) -> TrustResult<()> {
    let chain_head_bytes = decode_chain_head(chain_head_hex)?;

    // Constant-time kid compare: the wire-side `witness_kid` is
    // attacker-controlled (a malicious issuer can choose any string).
    // Using `==` would short-circuit on the first differing byte and,
    // over enough trials, leak prefix-match length about the pinned
    // roster entries. The pinned entries are public source-controlled
    // strings so the leak is theoretical, but the cost of `ct_eq_str` is
    // nil and Atlas's whole point is "byte-identical verification
    // regardless of input shape" — pay it everywhere.
    let pubkey = roster
        .iter()
        .find(|(kid, _)| crate::ct::ct_eq_str(kid, &witness.witness_kid))
        .map(|(_, pk)| pk)
        .ok_or_else(|| TrustError::BadWitness {
            witness_kid: witness.witness_kid.clone(),
            reason: "witness_kid not in pinned roster".to_string(),
        })?;

    let signing_input = witness_signing_input(&chain_head_bytes);

    let sig_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&witness.signature)
        .map_err(|e| TrustError::BadWitness {
            witness_kid: witness.witness_kid.clone(),
            reason: format!("signature is not valid base64url-no-pad: {e}"),
        })?;

    let sig_array: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| {
        TrustError::BadWitness {
            witness_kid: witness.witness_kid.clone(),
            reason: format!(
                "signature must be 64 bytes (512-bit Ed25519), got {}",
                sig_bytes.len(),
            ),
        }
    })?;

    let verifying_key = VerifyingKey::from_bytes(pubkey).map_err(|e| TrustError::BadWitness {
        witness_kid: witness.witness_kid.clone(),
        reason: format!("invalid pubkey in roster: {e}"),
    })?;

    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify_strict(&signing_input, &signature)
        .map_err(|e| TrustError::BadWitness {
            witness_kid: witness.witness_kid.clone(),
            reason: format!("ed25519 verification failed: {e}"),
        })
}

/// Outcome of verifying a slice of witness signatures against a roster.
#[derive(Debug, Clone)]
pub struct WitnessVerifyOutcome {
    /// Number of witnesses presented in the input slice.
    pub presented: usize,
    /// Number of witnesses that verified successfully against the
    /// roster.
    pub verified: usize,
    /// Per-failed-witness diagnostics, formatted as
    /// `"<witness_kid>: <reason>"` strings.
    pub failures: Vec<String>,
}

/// Verify each witness in the slice against the roster, returning a
/// counted outcome rather than short-circuiting on the first failure.
///
/// Counted-outcome shape lets the strict-mode caller (V1.13 Wave C-2)
/// apply an M-of-N threshold without re-running the verification.
pub fn verify_witnesses_against_roster(
    witnesses: &[WitnessSig],
    chain_head_hex: &str,
    roster: &[(&str, [u8; 32])],
) -> WitnessVerifyOutcome {
    let mut verified = 0usize;
    let mut failures = Vec::new();

    for w in witnesses {
        match verify_witness_against_roster(w, chain_head_hex, roster) {
            Ok(()) => verified += 1,
            Err(e) => failures.push(format!("{}: {}", w.witness_kid, e)),
        }
    }

    WitnessVerifyOutcome {
        presented: witnesses.len(),
        verified,
        failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    /// Build a signing key + matching pubkey from a fixed 32-byte seed.
    fn fixed_keypair(seed: u8) -> (SigningKey, [u8; 32]) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let pk = sk.verifying_key().to_bytes();
        (sk, pk)
    }

    /// Produce a WitnessSig for the given chain head using the given
    /// signing key + kid. Mirrors what `Ed25519Witness::sign_chain_head`
    /// will do in the `atlas-witness` crate.
    fn sign_witness(sk: &SigningKey, kid: &str, chain_head_hex: &str) -> WitnessSig {
        let head_bytes = decode_chain_head(chain_head_hex).unwrap();
        let input = witness_signing_input(&head_bytes);
        let sig = sk.sign(&input);
        WitnessSig {
            witness_kid: kid.to_string(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes()),
        }
    }

    fn fixed_chain_head() -> String {
        // 64 hex chars = 32 bytes — any valid hex works for the tests.
        "abcd".repeat(16)
    }

    #[test]
    fn witness_sig_roundtrip() {
        let original = WitnessSig {
            witness_kid: "test-witness-1".to_string(),
            signature: "AAAA".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: WitnessSig = serde_json::from_str(&json).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn witness_sig_rejects_unknown_fields() {
        // deny_unknown_fields defends against an issuer adding an
        // unaudited field that a future verifier might silently honour.
        let json = r#"{"witness_kid":"k","signature":"AAAA","extra":"x"}"#;
        let result: Result<WitnessSig, _> = serde_json::from_str(json);
        assert!(result.is_err(), "unknown fields must reject");
    }

    #[test]
    fn signing_input_includes_domain_prefix() {
        let head = [0u8; 32];
        let input = witness_signing_input(&head);
        assert!(
            input.starts_with(ATLAS_WITNESS_DOMAIN),
            "signing input must lead with the domain prefix",
        );
        assert_eq!(input.len(), ATLAS_WITNESS_DOMAIN.len() + 32);
    }

    #[test]
    fn signing_input_differs_from_raw_head() {
        // If a future change accidentally dropped the domain prefix,
        // this test catches it (cross-system collision risk would
        // otherwise be unbounded).
        let head = [0xAB; 32];
        assert_ne!(witness_signing_input(&head), head.to_vec());
    }

    #[test]
    fn decode_chain_head_accepts_64_hex_chars() {
        let head = decode_chain_head(&fixed_chain_head()).unwrap();
        assert_eq!(head.len(), 32);
    }

    #[test]
    fn decode_chain_head_rejects_wrong_length() {
        let result = decode_chain_head("abcd"); // 2 bytes
        assert!(matches!(result, Err(TrustError::Encoding(_))));
    }

    #[test]
    fn decode_chain_head_rejects_non_hex() {
        let result = decode_chain_head(&"zz".repeat(32));
        assert!(matches!(result, Err(TrustError::Encoding(_))));
    }

    #[test]
    fn verify_witness_round_trip_against_roster() {
        let (sk, pk) = fixed_keypair(42);
        let head = fixed_chain_head();
        let witness = sign_witness(&sk, "test-witness-1", &head);
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];

        verify_witness_against_roster(&witness, &head, roster).unwrap();
    }

    #[test]
    fn verify_witness_unknown_kid_rejects() {
        let (sk, pk) = fixed_keypair(42);
        let head = fixed_chain_head();
        let witness = sign_witness(&sk, "unknown-witness", &head);
        // Roster has a different kid — verifier must refuse to fall
        // back to "any pubkey in the roster".
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];

        let result = verify_witness_against_roster(&witness, &head, roster);
        match result {
            Err(TrustError::BadWitness { witness_kid, reason }) => {
                assert_eq!(witness_kid, "unknown-witness");
                assert!(
                    reason.contains("not in pinned roster"),
                    "reason should name the failure mode: {reason}",
                );
            }
            other => panic!("expected BadWitness, got {other:?}"),
        }
    }

    #[test]
    fn verify_witness_tampered_chain_head_rejects() {
        let (sk, pk) = fixed_keypair(42);
        let original_head = fixed_chain_head();
        let witness = sign_witness(&sk, "test-witness-1", &original_head);
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];

        // Verifier sees a different head — signature must not verify.
        let tampered_head = "0123".repeat(16);
        let result = verify_witness_against_roster(&witness, &tampered_head, roster);
        assert!(matches!(result, Err(TrustError::BadWitness { .. })));
    }

    #[test]
    fn verify_witness_wrong_pubkey_rejects() {
        // Issuer claims kid "k", roster has kid "k" with a DIFFERENT
        // pubkey — sig won't verify.
        let (sk, _pk) = fixed_keypair(42);
        let (_other_sk, other_pk) = fixed_keypair(7);
        let head = fixed_chain_head();
        let witness = sign_witness(&sk, "test-witness-1", &head);
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", other_pk)];

        let result = verify_witness_against_roster(&witness, &head, roster);
        assert!(matches!(result, Err(TrustError::BadWitness { .. })));
    }

    #[test]
    fn verify_witness_malformed_base64_rejects() {
        let (_, pk) = fixed_keypair(42);
        let head = fixed_chain_head();
        let witness = WitnessSig {
            witness_kid: "test-witness-1".to_string(),
            signature: "not!valid!base64@@".to_string(),
        };
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];

        let result = verify_witness_against_roster(&witness, &head, roster);
        match result {
            Err(TrustError::BadWitness { reason, .. }) => {
                assert!(reason.contains("base64"), "reason should mention base64: {reason}");
            }
            other => panic!("expected BadWitness, got {other:?}"),
        }
    }

    /// Padded-standard base64 must NOT decode under URL_SAFE_NO_PAD.
    /// Defends against an issuer (or a future re-introduction of the
    /// STANDARD dialect) silently slipping through verification — the
    /// dialect mismatch must be a hard failure, not a fallback.
    #[test]
    fn verify_witness_padded_standard_b64_rejects() {
        let (sk, pk) = fixed_keypair(42);
        let head = fixed_chain_head();
        // Sign correctly, then re-encode under STANDARD (with padding)
        // to simulate a producer using the wrong dialect.
        let head_bytes = decode_chain_head(&head).unwrap();
        let input = witness_signing_input(&head_bytes);
        let sig = sk.sign(&input);
        let padded = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
        // STANDARD with padding ends with '=' for 64-byte input
        // (88 chars = ceil(64/3)*4); URL_SAFE_NO_PAD's 86 chars never
        // include '=', so the decoder rejects.
        assert!(padded.contains('='), "STANDARD must produce padding for 64 bytes");
        let witness = WitnessSig {
            witness_kid: "test-witness-1".to_string(),
            signature: padded,
        };
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];

        let result = verify_witness_against_roster(&witness, &head, roster);
        assert!(
            matches!(result, Err(TrustError::BadWitness { .. })),
            "STANDARD-padded base64 must NOT decode under URL_SAFE_NO_PAD: got {result:?}"
        );
    }

    /// 63-byte sig (one byte short of an Ed25519 signature) must be
    /// rejected with a length-specific BadWitness, not a verification
    /// error. Defends the explicit length check we introduced when we
    /// inlined `verify_strict` (so we don't fall through to dalek with
    /// an array of the wrong size).
    #[test]
    fn verify_witness_wrong_length_signature_rejects() {
        let (_, pk) = fixed_keypair(42);
        let head = fixed_chain_head();
        // 63 bytes encodes to 84 b64url-no-pad chars.
        let short_sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 63]);
        let witness = WitnessSig {
            witness_kid: "test-witness-1".to_string(),
            signature: short_sig,
        };
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];

        let result = verify_witness_against_roster(&witness, &head, roster);
        match result {
            Err(TrustError::BadWitness { reason, .. }) => {
                assert!(
                    reason.contains("64 bytes"),
                    "length error must name the expected size: {reason}"
                );
            }
            other => panic!("expected BadWitness with length reason, got {other:?}"),
        }
    }

    /// Roster lookup must reject empty-string kid even if the wire-side
    /// witness_kid is also empty (defence against a producer that emits
    /// `"witness_kid": ""` and a future code change that adds an
    /// accidental sentinel entry to the roster).
    #[test]
    fn verify_witness_empty_kid_rejects() {
        let head = fixed_chain_head();
        let witness = WitnessSig {
            witness_kid: String::new(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 64]),
        };
        // ATLAS_WITNESS_V1_ROSTER is empty so the genesis roster never
        // matches — but exercise the path through an explicit empty
        // roster too.
        let roster: &[(&str, [u8; 32])] = &[];

        let result = verify_witness_against_roster(&witness, &head, roster);
        assert!(matches!(
            result,
            Err(TrustError::BadWitness { reason, .. }) if reason.contains("not in pinned roster")
        ));
    }

    #[test]
    fn verify_witnesses_outcome_counts_mixed() {
        let (sk_good, pk_good) = fixed_keypair(42);
        let (sk_bad, _pk_bad) = fixed_keypair(7);
        let head = fixed_chain_head();
        let roster: &[(&str, [u8; 32])] = &[("good-witness", pk_good)];

        let witnesses = vec![
            sign_witness(&sk_good, "good-witness", &head), // verifies
            sign_witness(&sk_bad, "unknown-witness", &head), // unknown kid
        ];

        let outcome = verify_witnesses_against_roster(&witnesses, &head, roster);
        assert_eq!(outcome.presented, 2);
        assert_eq!(outcome.verified, 1);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].starts_with("unknown-witness:"));
    }

    #[test]
    fn verify_witnesses_outcome_empty_input() {
        let outcome =
            verify_witnesses_against_roster(&[], &fixed_chain_head(), ATLAS_WITNESS_V1_ROSTER);
        assert_eq!(outcome.presented, 0);
        assert_eq!(outcome.verified, 0);
        assert!(outcome.failures.is_empty());
    }

    #[test]
    fn pinned_roster_v1_genesis_is_empty() {
        // V1.13 invariant: first commissioning bumps the crate version
        // and adds the entry. If this fires, the roster has changed and
        // the new entries need a coordinated SECURITY-NOTES update +
        // crate version bump per the V1.7 boundary rule.
        assert!(
            ATLAS_WITNESS_V1_ROSTER.is_empty(),
            "ATLAS_WITNESS_V1_ROSTER changed from genesis-empty without coordinated bump",
        );
    }

    /// Domain-prefix disjointness: `ATLAS_WITNESS_DOMAIN` must not
    /// equal, prefix, or suffix-match `ANCHOR_CHAIN_DOMAIN`. If two
    /// signing-input domains collapse, a chain-head signature could
    /// be replayed as a witness signature (or vice versa), defeating
    /// trust-domain separation.
    ///
    /// Equality is the obvious failure mode; one-way prefix is the
    /// subtler one — `b"foo:"` and `b"foo:bar:"` would let an attacker
    /// who controls the suffix bytes (which here are caller-supplied
    /// chain-head bytes for both schemes) construct a single signature
    /// that verifies under both domains. blake3 outputs are
    /// pseudorandom so the practical risk is low, but the cost of the
    /// pinned check is nil — pay it.
    #[test]
    fn witness_domain_disjoint_from_anchor_chain_domain() {
        let w = ATLAS_WITNESS_DOMAIN;
        let c = crate::anchor::ANCHOR_CHAIN_DOMAIN;
        assert_ne!(w, c, "witness and anchor-chain domains must differ");
        assert!(
            !w.starts_with(c) && !c.starts_with(w),
            "neither domain may be a prefix of the other (witness={:?}, anchor_chain={:?})",
            std::str::from_utf8(w).unwrap_or("<non-utf8>"),
            std::str::from_utf8(c).unwrap_or("<non-utf8>"),
        );
    }
}
