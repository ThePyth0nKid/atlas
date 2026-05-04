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

/// Maximum byte length a `witness_kid` may carry on the wire.
///
/// 256 is generous for any honest naming convention — SPIFFE IDs,
/// domain-style identifiers, and human-readable labels comfortably fit
/// well under 200 bytes. The cap defends against an issuer (or attacker
/// controlling batch serialisation) supplying multi-kilobyte kid strings
/// that would amplify per-batch cost: each kid participates in the
/// `BTreeMap` pre-pass for duplicate detection, the `BTreeSet`
/// cross-batch dedup, and the constant-time roster comparison — all O(N)
/// in kid length per witness. Without a cap, a single batch with N
/// witnesses each carrying a 1MB kid would do O(N² · 1MB) work in the
/// pre-pass alone, plus heap pressure proportional to N · 1MB.
///
/// Enforcement happens at the entry of `verify_witness_against_roster`
/// so every codepath that resolves a kid against the roster sees the
/// same cap; oversize kids fail closed as `BadWitness`. The genesis
/// roster is empty so this cap is the FIRST line of defence today —
/// once entries are commissioned, those entries' kids must also fit
/// the cap (enforced at compile time by the test
/// `roster_kids_within_length_cap`).
pub const MAX_WITNESS_KID_LEN: usize = 256;

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

/// Clamp an untrusted wire-side `witness_kid` for use in diagnostic
/// surfaces (`WitnessFailure`, log strings, `Display` output). Kids
/// at or below `MAX_WITNESS_KID_LEN` pass through unchanged so
/// auditors see the actual offending kid; kids exceeding the cap
/// collapse to a fixed-shape placeholder that records only the byte
/// length, so an attacker cannot amplify log volume by submitting a
/// multi-megabyte `witness_kid` and having it echoed across every
/// failure surface (per-witness diagnostic + lenient evidence row's
/// `rendered.join("; ")` aggregation).
///
/// Used at every site that copies the wire-side kid into a
/// `WitnessFailure` — keeping the sanitisation logic in one helper
/// (1) prevents drift between the per-batch verifier and the
/// chain-aggregator paths and (2) makes the cap byte-equivalent in
/// length-cap test coverage.
pub(crate) fn sanitize_kid_for_diagnostic(kid: &str) -> String {
    if kid.len() > MAX_WITNESS_KID_LEN {
        format!("<oversize: {} bytes>", kid.len())
    } else {
        kid.to_owned()
    }
}

/// Decode a hex chain head (64 lowercase hex chars) to its raw 32 bytes.
///
/// Strictness mirrors `ChainHeadHex::new` exactly: length must be 64
/// AND every character must be lowercase hex (`0-9`, `a-f`). The
/// `hex` crate by default accepts uppercase too, which would let two
/// byte-different wire strings (`"ABCD…"` vs `"abcd…"`) decode to the
/// same head — defeating the "one canonical hex form per head"
/// invariant the production producer (`chain_head_for`) relies on.
/// Without lowercase enforcement here, a wire-side caller that bypasses
/// `ChainHeadHex` could decode a mixed-case head, build a witness
/// signing input over its bytes, and produce a sig the verifier
/// accepts — even though the canonical recomputed head string differs
/// (a future strict equality check by string would then mismatch
/// silently).
///
/// Implementation routes through `ChainHeadHex::new` to keep the
/// length+lowercase invariant single-source: a future tightening of
/// the head shape lands in one place rather than two synchronised
/// edits.
pub fn decode_chain_head(chain_head_hex: &str) -> TrustResult<[u8; 32]> {
    // Single source of truth: route through `ChainHeadHex::new` so the
    // length + lowercase invariants live in one place. A future
    // tightening of the head shape automatically propagates here
    // instead of requiring two synchronised edits, and the contract
    // "if it parses as ChainHeadHex it decodes as 32 bytes" is
    // structural rather than convention. The single `String`
    // allocation per call is negligible — verification calls this
    // once per witness, and the byte-level work below it
    // (Ed25519 verify) dwarfs it by orders of magnitude.
    crate::anchor::ChainHeadHex::new(chain_head_hex.to_owned())
        .map(|h| h.to_bytes())
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
///
/// # Production callers MUST pass [`ATLAS_WITNESS_V1_ROSTER`]
///
/// The `roster` parameter is exposed as a free slice for **testability
/// only**: unit tests in this module construct ad-hoc rosters with
/// freshly generated keypairs to drive each fail-closed branch. The
/// production verifier ([`crate::verify::verify_trace_with`] →
/// `aggregate_witnesses_across_chain` → `witness_evidence_from_aggregate`)
/// always passes [`ATLAS_WITNESS_V1_ROSTER`], the source-controlled
/// pinned roster — never an env var, file read, or runtime-mutable
/// source. A future caller in this crate that passes a different
/// roster in the production path would silently widen the trust
/// surface; reviewers should reject any such call site.
pub fn verify_witness_against_roster(
    witness: &WitnessSig,
    chain_head_hex: &str,
    roster: &[(&str, [u8; 32])],
) -> TrustResult<()> {
    // Length cap on the wire-side `witness_kid` (V1.13 wave-C-2). A
    // hostile or buggy issuer could otherwise emit multi-megabyte kid
    // strings that amplify per-batch verification cost (BTreeMap
    // pre-pass + BTreeSet cross-batch dedup + ct_eq_str scan all run
    // O(N) in kid length per witness). Fail closed BEFORE any roster
    // work runs so the cost of rejection is constant in the input.
    if witness.witness_kid.len() > MAX_WITNESS_KID_LEN {
        return Err(TrustError::BadWitness {
            witness_kid: sanitize_kid_for_diagnostic(&witness.witness_kid),
            reason: format!(
                "witness_kid exceeds MAX_WITNESS_KID_LEN ({} > {} bytes)",
                witness.witness_kid.len(),
                MAX_WITNESS_KID_LEN,
            ),
        });
    }

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

/// Structured per-witness failure record. Carries the kid as it
/// appeared on the wire plus the underlying `TrustError` (almost
/// always `TrustError::BadWitness`) so consumers can filter
/// programmatically without parsing free-form text.
///
/// `batch_index` is populated by the chain-aggregating wrapper
/// (`aggregate_witnesses_across_chain` in the verifier) and is
/// always `None` when produced directly by
/// `verify_witnesses_against_roster` (no batch context in scope at
/// that level). The field is `pub(crate)` precisely because of this
/// context-dependent meaning — exposing it as `pub` would invite
/// external callers of `verify_witnesses_against_roster` to read the
/// always-`None` value as if it were a meaningful signal. Use the
/// public [`WitnessFailure::batch_index`] getter, whose return-type
/// `Option<u64>` makes the absence-vs-presence semantics explicit.
///
/// The `Display` impl renders the batch prefix when present so
/// downstream evidence-row text stays human-readable:
/// `"batch[N] invalid witness X: reason"` or
/// `"invalid witness X: reason"`.
#[derive(Debug, Clone)]
pub struct WitnessFailure {
    /// `Some(idx)` when the failure was produced during a chain-walk
    /// rollup (verifier side); `None` for the per-batch verification
    /// boundary (`verify_witnesses_against_roster`). `pub(crate)` so
    /// only the in-crate aggregator can populate it; external
    /// consumers read via the [`WitnessFailure::batch_index`] getter.
    pub(crate) batch_index: Option<u64>,
    /// Kid as it appeared on the wire (untrusted input — surface for
    /// auditor diagnostics; do NOT trust as a routing key beyond the
    /// roster lookup that already happened).
    pub witness_kid: String,
    /// Structured error. `TrustError::BadWitness` for verification
    /// failures (the common case); other variants reserved for future
    /// non-witness-domain errors that surface during the rollup
    /// (e.g. `chain_head_for` decode errors, currently wrapped as
    /// `BadWitness` for callsite consistency).
    pub error: TrustError,
}

impl WitnessFailure {
    /// Batch index this failure was produced under, when the failure
    /// surfaced from the chain-aggregation rollup. Returns `None` when
    /// the failure came directly from
    /// [`verify_witnesses_against_roster`] (no batch context exists at
    /// that level — there is no batch to index into).
    ///
    /// Treat `None` as "not applicable in this calling context", not
    /// as "no batch information is available" — the per-batch
    /// verifier never has a batch index, so the absence is structural,
    /// not informational.
    pub fn batch_index(&self) -> Option<u64> {
        self.batch_index
    }
}

impl std::fmt::Display for WitnessFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Render uniformly as `[batch[N] ]witness {kid}: {reason}`.
        // The kid surfaces from `self.witness_kid` (the canonical
        // wire-side value on the struct) rather than depending on
        // each `TrustError` variant to embed it. For
        // `TrustError::BadWitness` (the common case) we extract just
        // the `reason` so the kid does not appear twice; for any
        // other error variant we render the full inner Display after
        // the kid prefix. Keeps auditor diagnostics one-line and
        // grep-friendly while remaining correct if a future caller
        // ever stores a non-`BadWitness` error here.
        let prefix = match self.batch_index {
            Some(idx) => format!("batch[{idx}] "),
            None => String::new(),
        };
        match &self.error {
            TrustError::BadWitness { reason, .. } => {
                write!(f, "{prefix}witness {}: {}", self.witness_kid, reason)
            }
            other => write!(f, "{prefix}witness {}: {}", self.witness_kid, other),
        }
    }
}

/// Outcome of verifying a slice of witness signatures against a roster.
///
/// `failures` is a structured `Vec<WitnessFailure>` (kid + structured
/// `TrustError`), not a `Vec<String>`. Consumers wanting the legacy
/// `"kid: reason"` text just call `.to_string()` on each entry; the
/// structured shape lets a strict-mode caller (or auditor UI) filter by
/// failure kind without regex against free-form text.
#[derive(Debug, Clone)]
pub struct WitnessVerifyOutcome {
    /// Number of witnesses presented in the input slice.
    pub presented: usize,
    /// Number of witnesses that verified successfully against the
    /// roster.
    pub verified: usize,
    /// Per-failed-witness diagnostics. See `WitnessFailure` for the
    /// shape.
    pub failures: Vec<WitnessFailure>,
}

/// Verify each witness in the slice against the roster, returning a
/// counted outcome rather than short-circuiting on the first failure.
///
/// Counted-outcome shape lets the strict-mode caller (V1.13 Wave C-2)
/// apply an M-of-N threshold without re-running the verification.
///
/// Duplicate-`witness_kid` defence: a witness slice with repeated
/// `witness_kid` values has every occurrence (including the first)
/// rejected as a failure — none is counted as verified. Rationale:
/// under the C-2 M-of-N threshold, an issuer (or attacker controlling
/// batch serialisation) could otherwise satisfy a 3-of-3 quorum by
/// attaching the same signature three times under one commissioned key.
/// Catching this at the per-batch verifier — rather than bolting it
/// onto the threshold check later — keeps the trust property uniform
/// across lenient and strict modes (operators see "duplicate
/// witness_kid" in the lenient evidence row before strict mode is
/// even enabled) and means a future threshold check can trust that
/// `verified` already counts only kid-distinct cosignatures.
///
/// # Production callers MUST pass [`ATLAS_WITNESS_V1_ROSTER`]
///
/// Same testability/production contract as
/// [`verify_witness_against_roster`]: the `roster` parameter is a free
/// slice for unit-test ergonomics; production code in this crate
/// (called via [`crate::verify::verify_trace_with`]) always passes
/// [`ATLAS_WITNESS_V1_ROSTER`]. The cross-batch dedup that the
/// chain aggregator layers on top of this function only holds if
/// every batch is verified against the same roster — passing
/// different rosters across batches in a future refactor would
/// silently break the dedup invariant.
pub fn verify_witnesses_against_roster(
    witnesses: &[WitnessSig],
    chain_head_hex: &str,
    roster: &[(&str, [u8; 32])],
) -> WitnessVerifyOutcome {
    use std::collections::BTreeMap;

    let mut verified = 0usize;
    let mut failures = Vec::new();

    // Pre-pass: count kid occurrences. BTreeMap (not HashMap) for
    // deterministic ordering — keeps test/diagnostic output stable
    // and removes a hash-DoS vector if the slice ever grows large.
    let mut kid_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for w in witnesses {
        *kid_counts.entry(w.witness_kid.as_str()).or_insert(0) += 1;
    }

    for w in witnesses {
        let count = *kid_counts
            .get(w.witness_kid.as_str())
            .expect("kid was inserted in pre-pass — invariant of the loop above");
        if count > 1 {
            failures.push(WitnessFailure {
                batch_index: None,
                witness_kid: w.witness_kid.clone(),
                error: TrustError::BadWitness {
                    witness_kid: w.witness_kid.clone(),
                    reason: format!(
                        "duplicate witness_kid (appears {} times in batch — at most one signature per kid is allowed)",
                        count,
                    ),
                },
            });
            continue;
        }
        match verify_witness_against_roster(w, chain_head_hex, roster) {
            Ok(()) => verified += 1,
            Err(e) => failures.push(WitnessFailure {
                batch_index: None,
                witness_kid: w.witness_kid.clone(),
                error: e,
            }),
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
        let f = &outcome.failures[0];
        assert_eq!(f.witness_kid, "unknown-witness");
        assert!(f.batch_index.is_none(), "per-batch verifier has no batch context");
        assert!(
            matches!(&f.error, TrustError::BadWitness { reason, .. } if reason.contains("not in pinned roster")),
            "error must be BadWitness with the 'not in pinned roster' reason: {:?}",
            f.error,
        );
        // Display still renders as a single line for the lenient
        // evidence row.
        assert!(
            f.to_string().contains("unknown-witness"),
            "Display must include the kid: {}",
            f,
        );
    }

    #[test]
    fn verify_witnesses_outcome_empty_input() {
        let outcome =
            verify_witnesses_against_roster(&[], &fixed_chain_head(), ATLAS_WITNESS_V1_ROSTER);
        assert_eq!(outcome.presented, 0);
        assert_eq!(outcome.verified, 0);
        assert!(outcome.failures.is_empty());
    }

    /// Two valid witnesses with distinct kids both verify — pins the
    /// multi-witness happy path. Defends against an off-by-one where
    /// only the first witness in the slice gets checked.
    #[test]
    fn verify_witnesses_outcome_two_valid_distinct_kids() {
        let (sk_a, pk_a) = fixed_keypair(1);
        let (sk_b, pk_b) = fixed_keypair(2);
        let head = fixed_chain_head();
        let roster: &[(&str, [u8; 32])] = &[("witness-a", pk_a), ("witness-b", pk_b)];

        let witnesses = vec![
            sign_witness(&sk_a, "witness-a", &head),
            sign_witness(&sk_b, "witness-b", &head),
        ];

        let outcome = verify_witnesses_against_roster(&witnesses, &head, roster);
        assert_eq!(outcome.presented, 2);
        assert_eq!(outcome.verified, 2);
        assert!(
            outcome.failures.is_empty(),
            "no failures expected: {:?}",
            outcome.failures,
        );
    }

    /// Duplicate `witness_kid` in the slice — every occurrence
    /// (including the first) is rejected as a duplicate, none is
    /// counted as verified. Defends Wave C-2's M-of-N threshold from
    /// being satisfied by repeating one validly-signed sig N times
    /// under a single commissioned key.
    #[test]
    fn verify_witnesses_outcome_duplicate_kid_rejected() {
        let (sk, pk) = fixed_keypair(42);
        let head = fixed_chain_head();
        let roster: &[(&str, [u8; 32])] = &[("dup-kid", pk)];

        // Three sigs, all CORRECTLY signed under the same kid — without
        // duplicate detection this would yield verified == 3 and let an
        // attacker with one commissioned key satisfy a 3-of-3 threshold.
        let sig = sign_witness(&sk, "dup-kid", &head);
        let witnesses = vec![sig.clone(), sig.clone(), sig];

        let outcome = verify_witnesses_against_roster(&witnesses, &head, roster);
        assert_eq!(outcome.presented, 3);
        assert_eq!(
            outcome.verified, 0,
            "no duplicate-kid sig may count as verified, got {}",
            outcome.verified,
        );
        assert_eq!(
            outcome.failures.len(),
            3,
            "every duplicate occurrence must surface as a failure"
        );
        for f in &outcome.failures {
            assert_eq!(f.witness_kid, "dup-kid");
            assert!(
                matches!(&f.error, TrustError::BadWitness { reason, .. } if reason.contains("duplicate witness_kid")),
                "failure must be BadWitness with the dup-kid reason: {:?}",
                f.error,
            );
        }
    }

    /// Mixed slice: a duplicate-kid pair AND one distinct,
    /// validly-signed witness. The unique witness still verifies; the
    /// duplicates fail. Pins that duplicate detection is per-kid, not
    /// per-slice (a dup-kid does NOT poison unrelated kids).
    #[test]
    fn verify_witnesses_outcome_duplicate_with_unique_other() {
        let (sk_dup, pk_dup) = fixed_keypair(42);
        let (sk_solo, pk_solo) = fixed_keypair(7);
        let head = fixed_chain_head();
        let roster: &[(&str, [u8; 32])] =
            &[("dup-kid", pk_dup), ("solo-kid", pk_solo)];

        let dup = sign_witness(&sk_dup, "dup-kid", &head);
        let witnesses = vec![
            dup.clone(),
            dup,
            sign_witness(&sk_solo, "solo-kid", &head),
        ];

        let outcome = verify_witnesses_against_roster(&witnesses, &head, roster);
        assert_eq!(outcome.presented, 3);
        assert_eq!(outcome.verified, 1, "only the unique-kid witness verifies");
        assert_eq!(outcome.failures.len(), 2);
        for f in &outcome.failures {
            assert_eq!(
                f.witness_kid, "dup-kid",
                "only the dup-kid should fail, got {:?}",
                f,
            );
        }
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

    /// `decode_chain_head` rejects an UPPER-case hex head even though
    /// the raw `hex` crate would accept it. Defends the canonical
    /// "one hex form per head" invariant. See `decode_chain_head`'s
    /// doc comment for the full rationale.
    #[test]
    fn decode_chain_head_rejects_uppercase_hex() {
        let upper = "ABCD".repeat(16);
        let result = decode_chain_head(&upper);
        assert!(
            matches!(&result, Err(TrustError::Encoding(msg)) if msg.contains("lowercase")),
            "uppercase hex must reject with a lowercase-specific reason: {result:?}",
        );
    }

    /// `decode_chain_head` also rejects mixed-case input — a single
    /// uppercase nibble is enough to fail closed.
    #[test]
    fn decode_chain_head_rejects_mixed_case_hex() {
        // 63 lowercase + 1 uppercase 'A' at the end.
        let mut mixed = "a".repeat(63);
        mixed.push('A');
        assert_eq!(mixed.len(), 64);
        let result = decode_chain_head(&mixed);
        assert!(matches!(&result, Err(TrustError::Encoding(_))));
    }

    /// `verify_witness_against_roster` rejects a witness whose
    /// `witness_kid` exceeds `MAX_WITNESS_KID_LEN` BEFORE any roster
    /// work runs. Cost-amplification defence (see
    /// `MAX_WITNESS_KID_LEN` doc).
    #[test]
    fn verify_witness_rejects_oversize_kid() {
        let oversize = "x".repeat(MAX_WITNESS_KID_LEN + 1);
        let head = fixed_chain_head();
        let witness = WitnessSig {
            witness_kid: oversize.clone(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode([0u8; 64]),
        };
        // Empty roster — but the cap fires before roster lookup, so
        // this still produces a length-specific BadWitness.
        let roster: &[(&str, [u8; 32])] = &[];

        let result = verify_witness_against_roster(&witness, &head, roster);
        match result {
            Err(TrustError::BadWitness { witness_kid, reason }) => {
                assert!(
                    witness_kid.contains("<oversize"),
                    "oversize kid must be sanitised in the surfaced kid (no echo of the multi-MB blob): {witness_kid}",
                );
                assert!(
                    reason.contains("MAX_WITNESS_KID_LEN"),
                    "reason must name the cap constant: {reason}",
                );
            }
            other => panic!("expected BadWitness with cap reason, got {other:?}"),
        }
    }

    /// Boundary: a kid of EXACTLY `MAX_WITNESS_KID_LEN` bytes must
    /// pass the cap (the check uses `>`, not `>=`). The test
    /// terminates at the next failure stage (unknown kid in the empty
    /// roster) — different error class, so the cap is provably not the
    /// reason.
    #[test]
    fn verify_witness_at_cap_boundary_passes_length_check() {
        let at_cap = "x".repeat(MAX_WITNESS_KID_LEN);
        let head = fixed_chain_head();
        let witness = WitnessSig {
            witness_kid: at_cap.clone(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode([0u8; 64]),
        };
        let roster: &[(&str, [u8; 32])] = &[];

        let result = verify_witness_against_roster(&witness, &head, roster);
        match result {
            Err(TrustError::BadWitness { reason, .. }) => {
                assert!(
                    !reason.contains("MAX_WITNESS_KID_LEN"),
                    "at-cap kid must NOT trip the length check: {reason}",
                );
                assert!(
                    reason.contains("not in pinned roster"),
                    "at-cap kid must reach the roster lookup: {reason}",
                );
            }
            other => panic!("expected BadWitness with roster reason, got {other:?}"),
        }
    }

    /// `WitnessFailure::Display` renders uniformly as
    /// `[batch[N] ]witness {kid}: {reason}` regardless of which
    /// `TrustError` variant the failure carries. Pins:
    ///   * batch_index=None case (no prefix).
    ///   * batch_index=Some case (prefix present).
    ///   * BadWitness extracts only the `reason` (no kid duplication).
    #[test]
    fn witness_failure_display_uniform_format() {
        let f_no_batch = WitnessFailure {
            batch_index: None,
            witness_kid: "kid-x".to_string(),
            error: TrustError::BadWitness {
                witness_kid: "kid-x".to_string(),
                reason: "boom".to_string(),
            },
        };
        assert_eq!(f_no_batch.to_string(), "witness kid-x: boom");

        let f_batch = WitnessFailure {
            batch_index: Some(7),
            witness_kid: "kid-y".to_string(),
            error: TrustError::BadWitness {
                witness_kid: "kid-y".to_string(),
                reason: "kaboom".to_string(),
            },
        };
        assert_eq!(f_batch.to_string(), "batch[7] witness kid-y: kaboom");
    }

    /// `WitnessFailure::Display` for a non-BadWitness inner error
    /// still surfaces the kid via `self.witness_kid`. Defends the
    /// uniformity contract: even an error variant that does not carry
    /// the kid in its own Display gets prefixed correctly.
    #[test]
    fn witness_failure_display_non_bad_witness_variant() {
        let f = WitnessFailure {
            batch_index: Some(2),
            witness_kid: "kid-z".to_string(),
            error: TrustError::Encoding("hex blew up".to_string()),
        };
        let s = f.to_string();
        assert!(
            s.contains("kid-z"),
            "kid must surface even when inner error variant doesn't carry it: {s}",
        );
        assert!(s.contains("batch[2]"), "batch prefix must appear: {s}");
        assert!(
            s.contains("hex blew up"),
            "inner error reason must surface: {s}",
        );
    }

    /// Compile-time invariant: every kid in `ATLAS_WITNESS_V1_ROSTER`
    /// must fit `MAX_WITNESS_KID_LEN`. With the genesis-empty roster
    /// this is vacuously true; but commissioning a kid longer than the
    /// cap would silently make the verifier reject its OWN roster's
    /// witnesses (the cap fires before roster lookup), which would be
    /// catastrophic — this test prevents that drift.
    #[test]
    fn roster_kids_within_length_cap() {
        for (kid, _) in ATLAS_WITNESS_V1_ROSTER {
            assert!(
                kid.len() <= MAX_WITNESS_KID_LEN,
                "roster kid {:?} ({} bytes) exceeds MAX_WITNESS_KID_LEN ({} bytes) — \
                 commissioning ceremony must keep kids under the cap or the verifier \
                 will reject its own attestor pre-roster-lookup",
                kid,
                kid.len(),
                MAX_WITNESS_KID_LEN,
            );
        }
    }

    /// `sanitize_kid_for_diagnostic` is the single source of truth for
    /// clamping wire-side kids before they land in any
    /// `WitnessFailure`. Its placeholder shape (`"<oversize: N bytes>"`)
    /// is byte-equivalent across both call sites
    /// (`verify_witness_against_roster` MAX_WITNESS_KID_LEN guard and
    /// the chain-aggregator's `chain_head_for` error branch) — pin the
    /// shape here so a careless edit to one site cannot drift away
    /// from the other.
    #[test]
    fn sanitize_kid_passthrough_at_or_below_cap() {
        // At cap: pass through unchanged so auditors see the actual kid.
        let at_cap = "k".repeat(MAX_WITNESS_KID_LEN);
        assert_eq!(sanitize_kid_for_diagnostic(&at_cap), at_cap);
        // Empty: pass through (no special-casing).
        assert_eq!(sanitize_kid_for_diagnostic(""), "");
        // Typical realistic kid: pass through.
        assert_eq!(
            sanitize_kid_for_diagnostic("witness-prod-eu-west-1"),
            "witness-prod-eu-west-1",
        );
    }

    /// Above the cap, the helper collapses to a fixed-shape
    /// placeholder that records ONLY the byte length — the original
    /// blob never appears in the output. This is the SEC-MED-1
    /// invariant: an attacker submitting a multi-megabyte
    /// `witness_kid` cannot get the verifier or aggregator to echo it
    /// back across diagnostic surfaces.
    #[test]
    fn sanitize_kid_clamps_above_cap() {
        let oversize = "x".repeat(MAX_WITNESS_KID_LEN + 1);
        let sanitized = sanitize_kid_for_diagnostic(&oversize);
        assert_eq!(
            sanitized,
            format!("<oversize: {} bytes>", MAX_WITNESS_KID_LEN + 1),
        );
        // Crucially: the original blob does NOT appear anywhere in
        // the placeholder. Pin this property so a future "improvement"
        // that includes a prefix of the blob breaks this test.
        assert!(
            !sanitized.contains('x'),
            "placeholder must not echo any byte from the oversized blob: {sanitized}",
        );
    }

    /// The helper's output for a kid one byte above the cap differs
    /// from the helper's output for a kid AT the cap — a regression
    /// where the off-by-one boundary slips would silently let an
    /// `MAX_WITNESS_KID_LEN + 1` kid through unchanged. Pin both
    /// sides of the boundary.
    #[test]
    fn sanitize_kid_boundary_off_by_one() {
        let at_cap = "y".repeat(MAX_WITNESS_KID_LEN);
        let just_over = "y".repeat(MAX_WITNESS_KID_LEN + 1);
        assert_eq!(sanitize_kid_for_diagnostic(&at_cap), at_cap);
        assert_ne!(sanitize_kid_for_diagnostic(&just_over), just_over);
        assert!(sanitize_kid_for_diagnostic(&just_over).starts_with("<oversize:"));
    }
}
