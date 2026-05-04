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
    // Public surface preserves the original `TrustResult<()>` shape
    // (so external callers and existing in-crate Tests keep working);
    // the V1.14 Scope J categorisation is provided by the
    // `_categorized` inner helper, which `verify_witnesses_against_roster`
    // calls so it can record the `reason_code` AT-SOURCE without
    // re-deriving it via string-match later.
    verify_witness_against_roster_categorized(witness, chain_head_hex, roster)
        .map_err(|(err, _reason)| err)
}

/// Inner verifier that returns BOTH the `TrustError` AND the stable
/// [`WitnessFailureReason`] categorisation produced AT-SOURCE
/// (V1.14 Scope J).
///
/// The categorisation is set at the exact failure site rather than
/// being string-matched against `TrustError::Display` later: doing it
/// here keeps the wire-stable bucketing decoupled from the
/// human-readable `reason` text (which can change between crate
/// versions without breaking the wire contract). A future tightening
/// of an error message no longer risks re-bucketing an audit-relevant
/// failure into a different `WitnessFailureReason`.
///
/// Kept `pub(crate)` so the per-batch verifier and the chain
/// aggregator can both invoke it; not part of the public API surface
/// (the public `verify_witness_against_roster` peels off the
/// categorisation for backward compatibility).
pub(crate) fn verify_witness_against_roster_categorized(
    witness: &WitnessSig,
    chain_head_hex: &str,
    roster: &[(&str, [u8; 32])],
) -> Result<(), (TrustError, WitnessFailureReason)> {
    // Length cap on the wire-side `witness_kid` (V1.13 wave-C-2). A
    // hostile or buggy issuer could otherwise emit multi-megabyte kid
    // strings that amplify per-batch verification cost (BTreeMap
    // pre-pass + BTreeSet cross-batch dedup + ct_eq_str scan all run
    // O(N) in kid length per witness). Fail closed BEFORE any roster
    // work runs so the cost of rejection is constant in the input.
    if witness.witness_kid.len() > MAX_WITNESS_KID_LEN {
        return Err((
            TrustError::BadWitness {
                witness_kid: sanitize_kid_for_diagnostic(&witness.witness_kid),
                reason: format!(
                    "witness_kid exceeds MAX_WITNESS_KID_LEN ({} > {} bytes)",
                    witness.witness_kid.len(),
                    MAX_WITNESS_KID_LEN,
                ),
            },
            WitnessFailureReason::OversizeKid,
        ));
    }

    // V1.14 Scope J defence-in-depth: every subsequent `TrustError::BadWitness`
    // constructor in this function MUST use this `sanitized` binding rather
    // than `witness.witness_kid` directly. The oversize guard above ensures
    // `kid.len() <= MAX_WITNESS_KID_LEN`, so `sanitize_kid_for_diagnostic`
    // is identity on every kid that reaches here. The reason we still go
    // through the helper: a future change that lifts or reorders the
    // length guard, or a future log-formatting site that prints
    // `TrustError::BadWitness` via Debug, must not silently re-open
    // multi-MB blob amplification through the in-memory error struct.
    let sanitized = sanitize_kid_for_diagnostic(&witness.witness_kid);

    // `decode_chain_head` returns `TrustError::Encoding` on failure.
    // In the production path this only happens when a programmer
    // bypasses `ChainHeadHex` and feeds raw caller-supplied hex —
    // surface as `Other` because it's a programmer-side wire-shape
    // bug, not a witness-domain failure. The chain aggregator never
    // routes through this branch (it uses `chain_head_for(batch)`
    // which returns `ChainHeadHex` directly), so production wire
    // output should never carry `Other` from this site.
    let chain_head_bytes = decode_chain_head(chain_head_hex)
        .map_err(|e| (e, WitnessFailureReason::Other))?;

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
        .ok_or_else(|| {
            (
                TrustError::BadWitness {
                    witness_kid: sanitized.clone(),
                    reason: "witness_kid not in pinned roster".to_string(),
                },
                WitnessFailureReason::KidNotInRoster,
            )
        })?;

    let signing_input = witness_signing_input(&chain_head_bytes);

    let sig_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&witness.signature)
        .map_err(|e| {
            (
                TrustError::BadWitness {
                    witness_kid: sanitized.clone(),
                    reason: format!("signature is not valid base64url-no-pad: {e}"),
                },
                WitnessFailureReason::InvalidSignatureFormat,
            )
        })?;

    let sig_array: [u8; 64] = sig_bytes.as_slice().try_into().map_err(|_| {
        (
            TrustError::BadWitness {
                witness_kid: sanitized.clone(),
                reason: format!(
                    "signature must be 64 bytes (512-bit Ed25519), got {}",
                    sig_bytes.len(),
                ),
            },
            WitnessFailureReason::InvalidSignatureLength,
        )
    })?;

    // Pubkey-byte parsing failure here means the roster source itself
    // carries a malformed pubkey — not a wire-side attack. Categorise
    // as `Ed25519VerifyFailed` (closest available bucket: a verify
    // path could not establish trust against the roster entry); a
    // dedicated `InvalidPubkeyInRoster` variant could land in a
    // future minor release without breaking the `non_exhaustive` SemVer
    // contract if operator surfacing benefits from the distinction.
    let verifying_key = VerifyingKey::from_bytes(pubkey).map_err(|e| {
        (
            TrustError::BadWitness {
                witness_kid: sanitized.clone(),
                reason: format!("invalid pubkey in roster: {e}"),
            },
            WitnessFailureReason::Ed25519VerifyFailed,
        )
    })?;

    let signature = Signature::from_bytes(&sig_array);

    verifying_key
        .verify_strict(&signing_input, &signature)
        .map_err(|e| {
            (
                TrustError::BadWitness {
                    witness_kid: sanitized.clone(),
                    reason: format!("ed25519 verification failed: {e}"),
                },
                WitnessFailureReason::Ed25519VerifyFailed,
            )
        })
}

/// Stable, structured categorisation of why a witness verification
/// failed. Wire-side projection of the closed set of failure paths
/// surfaced by [`verify_witness_against_roster`] and the
/// chain-walking aggregator inside the verifier.
///
/// **Why a separate enum from `TrustError`** — `TrustError` is the
/// internal error type (carries human-readable strings, may grow new
/// variants for unrelated trust domains across crate versions). The
/// witness wire-surface needs a STABLE classification an auditor UI
/// can switch on without parsing `TrustError::Display` (which is
/// human-readable, not version-stable). The `From<&WitnessFailure>`
/// impl on [`WitnessFailureWire`] performs the mapping at-source —
/// every `WitnessFailure` constructor in this crate sets a
/// `reason_code` directly so the wire surface never depends on
/// string-matching against the underlying `reason` text.
///
/// **`#[non_exhaustive]`** — adding a new variant in a future minor
/// release is NOT a SemVer break for downstream consumers (their
/// `match` arms must already include a wildcard or a `_ =>` catch-all
/// because of `non_exhaustive`). Auditors reading the wire MUST treat
/// an unknown reason_code as "investigate manually" rather than
/// silently bucketing it into one of the known variants.
///
/// **`Other` is the safety-valve** — used when the underlying
/// `TrustError` variant is not one of the witness-domain failure
/// modes the categorisation knows about (e.g. a future `TrustError`
/// variant that surfaces in the witness rollup before its dedicated
/// `WitnessFailureReason` lands). Production `From<&WitnessFailure>`
/// callers should never see `Other` — every in-crate construction
/// site picks a specific variant. If `Other` appears in the wire
/// output, treat it as a verifier-side bug to file: an audit-relevant
/// failure is being surfaced without proper categorisation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum WitnessFailureReason {
    /// `witness_kid` did not match any entry in the pinned roster.
    /// Most common attack signal: an issuer attached a sig under a
    /// kid that was never commissioned.
    KidNotInRoster,
    /// Two or more witnesses in the same batch carry the same
    /// `witness_kid`. Defends the M-of-N threshold from an issuer
    /// repeating one validly-signed sig N times under one
    /// commissioned key. Per the per-batch verifier, EVERY
    /// occurrence (including the first) is rejected.
    DuplicateKid,
    /// Same `witness_kid` verified successfully in an earlier batch
    /// of the chain. Defends M-of-N independence at the cross-batch
    /// level (one key signing N batches must NOT count as N
    /// attestors).
    CrossBatchDuplicateKid,
    /// Signature was not valid URL-safe base64 (no padding) — wire
    /// format violation, likely a producer using the wrong dialect.
    InvalidSignatureFormat,
    /// Signature decoded but was not exactly 64 bytes (an Ed25519
    /// signature is always 512 bits / 64 bytes per RFC 8032).
    InvalidSignatureLength,
    /// `witness_kid` exceeded `MAX_WITNESS_KID_LEN`. Cost-amplification
    /// defence — fires before any roster work runs, so the wire-side
    /// kid is sanitised to `<oversize: N bytes>` before it lands here.
    OversizeKid,
    /// Chain-head recompute failed for the batch this witness was
    /// attached to. Surfaces from the verifier's rollup when
    /// `chain_head_for(batch)` errored — the witness signature could
    /// not be checked because there is no canonical head to check it
    /// against. Distinguished from `Ed25519VerifyFailed` because the
    /// witness sig itself may be perfectly valid; the failure is on
    /// the chain-batch side.
    ChainHeadDecodeFailed,
    /// Ed25519 strict verification (RFC 8032) rejected the signature
    /// against the roster pubkey. Possible causes: signed-over a
    /// different chain head (replay/tamper), wrong pubkey for the
    /// kid (commissioned-but-rotated key whose entry was not
    /// updated), or a corrupted pubkey in the roster source itself.
    Ed25519VerifyFailed,
    /// Catch-all for `TrustError` variants that landed in
    /// [`WitnessFailure::error`] without a dedicated reason_code.
    /// **Should never appear in production wire output** — see the
    /// enum-level docs.
    Other,
}

/// Wire-stable structured projection of a `WitnessFailure` for
/// programmatic auditor consumption.
///
/// **The audit surface contract** — this struct is the structured
/// channel an auditor UI or downstream tool reads to filter, bucket,
/// and react to witness verification failures. It deliberately
/// EXCLUDES the underlying `TrustError`'s `Debug` output because
/// `TrustError::Display`'s text is not version-stable (it is
/// human-readable diagnostic, not a contract). The `reason_code`
/// enum is the version-stable categorisation; the `message` field
/// carries the same `Display` text the lenient evidence row already
/// surfaces, but auditors MUST switch on `reason_code` rather than
/// parsing `message`.
///
/// **Wire-format invariants** —
///   * `reason_code` round-trips as kebab-case JSON
///     (`"kid-not-in-roster"` etc.) — pinned by
///     `witness_failure_reason_serde_kebab_case`.
///   * Unknown JSON fields reject (`deny_unknown_fields`) so an
///     attacker controlling a downstream JSON consumer cannot
///     smuggle additional fields past a verifier that round-trips
///     the structure.
///
/// **Why a separate type, not a public field on `WitnessFailure`** —
/// `WitnessFailure` carries the in-crate `TrustError` (which
/// intentionally does NOT implement `Serialize`: serialising a
/// `TrustError` would imply a wire-stable shape we are not committing
/// to). `WitnessFailureWire` is the explicit wire-stable subset; the
/// rest of `WitnessFailure` stays an in-process structured value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WitnessFailureWire {
    /// Kid as it appeared on the wire (or sanitised placeholder for
    /// oversize kids). Untrusted input — auditor MUST NOT use as a
    /// routing key beyond identifying the failed witness.
    pub witness_kid: String,
    /// Batch index this failure was produced under. `None` when the
    /// failure surfaced from the per-batch verifier directly (no
    /// batch context); `Some(idx)` when from the chain-aggregator
    /// rollup. Same semantics as
    /// [`WitnessFailure::batch_index`].
    pub batch_index: Option<u64>,
    /// Stable categorisation of the failure. Auditors MUST switch on
    /// this rather than parsing `message`. See
    /// [`WitnessFailureReason`].
    pub reason_code: WitnessFailureReason,
    /// Human-readable diagnostic — the `WitnessFailure::Display`
    /// rendering. Stable enough for log-grepping but NOT a contract;
    /// the `reason_code` enum is the version-stable bucketing.
    pub message: String,
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
/// `reason_code` follows the same encapsulation pattern (V1.14 Scope
/// J): it is the stable, wire-projectable categorisation set
/// AT-SOURCE by every in-crate constructor, never derived later by
/// string-matching against `TrustError::Display`. Public read access
/// goes through [`WitnessFailure::reason_code`], whose stable enum
/// return type is the audit surface.
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
    /// V1.14 Scope J: stable, wire-projectable categorisation of the
    /// failure. `pub(crate)` so only the in-crate construction sites
    /// (this module's `verify_witness(es)_against_roster` and the
    /// chain aggregator) can populate it — external consumers read via
    /// the [`WitnessFailure::reason_code`] getter, whose stable
    /// `WitnessFailureReason` return type is the audit surface
    /// contract. Set AT-SOURCE rather than derived from
    /// `TrustError::Display` to keep the categorisation independent of
    /// any future change to the human-readable reason text.
    pub(crate) reason_code: WitnessFailureReason,
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

    /// Stable, wire-projectable categorisation of the failure. See
    /// [`WitnessFailureReason`] for the closed set of variants. The
    /// returned value is the same one that
    /// [`WitnessFailureWire::reason_code`] surfaces — auditors holding
    /// a `WitnessFailure` directly can read it without going through
    /// the wire projection.
    pub fn reason_code(&self) -> WitnessFailureReason {
        self.reason_code
    }
}

impl From<&WitnessFailure> for WitnessFailureWire {
    /// Project an in-crate `WitnessFailure` to its wire-stable form.
    ///
    /// **No information loss for audit-relevant fields**: the
    /// `witness_kid` (sanitized at source for oversize kids), the
    /// `batch_index` (populated by the chain aggregator, `None` from
    /// the per-batch verifier), and the `reason_code` (set
    /// AT-SOURCE by every in-crate constructor) all flow through
    /// unchanged. The `message` field captures the
    /// `WitnessFailure::Display` rendering — same text the lenient
    /// evidence row already surfaces, included for log-grep
    /// continuity but explicitly NOT a wire contract.
    ///
    /// **The underlying `TrustError` is intentionally dropped from
    /// the wire form** — its `Display` text is human-readable
    /// diagnostic, not version-stable. Auditors MUST switch on
    /// `reason_code`. Library-level callers that need the structured
    /// `TrustError` keep using `WitnessFailure` directly.
    fn from(f: &WitnessFailure) -> Self {
        WitnessFailureWire {
            witness_kid: f.witness_kid.clone(),
            batch_index: f.batch_index,
            reason_code: f.reason_code,
            message: f.to_string(),
        }
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
        // Sanitize the wire-side kid before it lands in any
        // `WitnessFailure.witness_kid` field (V1.14 Scope J latent-bug
        // fix uncovered by `reason_code_oversize_kid`): the
        // per-batch failure surface must be byte-equivalent with the
        // chain-aggregator's `ChainHeadDecodeFailed` branch and the
        // `verify_witness_against_roster_categorized` `OversizeKid`
        // branch, both of which already clamp via
        // `sanitize_kid_for_diagnostic`. Without this, an oversized
        // kid that hits the duplicate-pre-pass or the categorised
        // inner's TrustError::BadWitness path would echo unsanitised
        // into `WitnessFailure.witness_kid` and — via Display and
        // wire projection — amplify across the lenient evidence row
        // and the auditor wire surface.
        let sanitized_kid = sanitize_kid_for_diagnostic(&w.witness_kid);
        if count > 1 {
            failures.push(WitnessFailure {
                batch_index: None,
                witness_kid: sanitized_kid.clone(),
                error: TrustError::BadWitness {
                    witness_kid: sanitized_kid,
                    reason: format!(
                        "duplicate witness_kid (appears {} times in batch — at most one signature per kid is allowed)",
                        count,
                    ),
                },
                reason_code: WitnessFailureReason::DuplicateKid,
            });
            continue;
        }
        // Use the categorised inner so the V1.14 Scope J `reason_code`
        // is set AT-SOURCE — never derived later by string-matching
        // against the underlying `reason` text.
        match verify_witness_against_roster_categorized(w, chain_head_hex, roster) {
            Ok(()) => verified += 1,
            Err((err, reason_code)) => failures.push(WitnessFailure {
                batch_index: None,
                witness_kid: sanitized_kid.clone(),
                error: err,
                reason_code,
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
            reason_code: WitnessFailureReason::Other,
        };
        assert_eq!(f_no_batch.to_string(), "witness kid-x: boom");

        let f_batch = WitnessFailure {
            batch_index: Some(7),
            witness_kid: "kid-y".to_string(),
            error: TrustError::BadWitness {
                witness_kid: "kid-y".to_string(),
                reason: "kaboom".to_string(),
            },
            reason_code: WitnessFailureReason::Other,
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
            reason_code: WitnessFailureReason::Other,
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

    // ---------------------------------------------------------------
    // V1.14 Scope J — auditor wire-surface pin tests
    //
    // The `WitnessFailureReason` enum is the audit-stable contract
    // downstream tooling switches on. A test per variant pins that
    // the in-crate construction sites set the right reason_code AT
    // SOURCE — these tests are the regression guard against any
    // future refactor that string-matches `reason` text instead of
    // setting the categorisation explicitly.
    // ---------------------------------------------------------------

    /// Helper: build a roster of one entry plus a chain head + a
    /// signing key for tests that need a verifiable witness.
    fn one_entry_roster_setup() -> (SigningKey, [u8; 32], String) {
        let (sk, pk) = fixed_keypair(42);
        (sk, pk, fixed_chain_head())
    }

    /// `KidNotInRoster` — wire-side kid does not match any pinned
    /// roster entry. The most common attacker signal (sig under a
    /// kid that was never commissioned).
    #[test]
    fn reason_code_kid_not_in_roster() {
        let (sk, pk, head) = one_entry_roster_setup();
        let witness = sign_witness(&sk, "unknown-kid", &head);
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];

        let outcome = verify_witnesses_against_roster(&[witness], &head, roster);
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(
            outcome.failures[0].reason_code(),
            WitnessFailureReason::KidNotInRoster,
        );
    }

    /// `DuplicateKid` — same `witness_kid` appears 2+ times in a
    /// single batch. Per-batch verifier rejects every occurrence
    /// (including the first) so the M-of-N threshold cannot be
    /// satisfied by repeating one valid sig under one commissioned
    /// key.
    #[test]
    fn reason_code_duplicate_kid() {
        let (sk, pk, head) = one_entry_roster_setup();
        let roster: &[(&str, [u8; 32])] = &[("dup-kid", pk)];

        let sig = sign_witness(&sk, "dup-kid", &head);
        let witnesses = vec![sig.clone(), sig];

        let outcome = verify_witnesses_against_roster(&witnesses, &head, roster);
        assert_eq!(outcome.failures.len(), 2);
        for f in &outcome.failures {
            assert_eq!(f.reason_code(), WitnessFailureReason::DuplicateKid);
        }
    }

    /// `InvalidSignatureFormat` — signature is not URL-safe base64
    /// (no padding). Defends against producers using the wrong
    /// dialect.
    #[test]
    fn reason_code_invalid_signature_format() {
        let (_, pk) = fixed_keypair(42);
        let head = fixed_chain_head();
        let witness = WitnessSig {
            witness_kid: "test-witness-1".to_string(),
            signature: "not!valid!base64@@".to_string(),
        };
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];

        let outcome = verify_witnesses_against_roster(&[witness], &head, roster);
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(
            outcome.failures[0].reason_code(),
            WitnessFailureReason::InvalidSignatureFormat,
        );
    }

    /// `InvalidSignatureLength` — base64 decoded but did not yield
    /// exactly 64 bytes (Ed25519 sig is always 512 bits).
    #[test]
    fn reason_code_invalid_signature_length() {
        let (_, pk) = fixed_keypair(42);
        let head = fixed_chain_head();
        // 63 bytes = 84 b64url-no-pad chars — decodes successfully but
        // is one byte short of an Ed25519 signature.
        let short_sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 63]);
        let witness = WitnessSig {
            witness_kid: "test-witness-1".to_string(),
            signature: short_sig,
        };
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];

        let outcome = verify_witnesses_against_roster(&[witness], &head, roster);
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(
            outcome.failures[0].reason_code(),
            WitnessFailureReason::InvalidSignatureLength,
        );
    }

    /// `OversizeKid` — wire-side `witness_kid` exceeds
    /// `MAX_WITNESS_KID_LEN`. Cost-amplification defence; fires
    /// BEFORE roster lookup.
    #[test]
    fn reason_code_oversize_kid() {
        let oversize = "x".repeat(MAX_WITNESS_KID_LEN + 1);
        let head = fixed_chain_head();
        let witness = WitnessSig {
            witness_kid: oversize,
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0u8; 64]),
        };
        let roster: &[(&str, [u8; 32])] = &[];

        let outcome = verify_witnesses_against_roster(&[witness], &head, roster);
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(
            outcome.failures[0].reason_code(),
            WitnessFailureReason::OversizeKid,
        );
        // Sanity: the kid surface in the failure record is the
        // sanitised placeholder, not the multi-MB blob.
        assert!(
            outcome.failures[0].witness_kid.starts_with("<oversize:"),
            "oversize kid must surface as sanitised placeholder: {}",
            outcome.failures[0].witness_kid,
        );
    }

    /// `Ed25519VerifyFailed` — signature did not verify against the
    /// roster pubkey. Triggered here by a tampered chain head: sig
    /// was made over the original head, verifier sees a different
    /// one.
    #[test]
    fn reason_code_ed25519_verify_failed() {
        let (sk, pk) = fixed_keypair(42);
        let original_head = fixed_chain_head();
        let witness = sign_witness(&sk, "test-witness-1", &original_head);
        let roster: &[(&str, [u8; 32])] = &[("test-witness-1", pk)];
        let tampered_head = "0123".repeat(16);

        let outcome = verify_witnesses_against_roster(&[witness], &tampered_head, roster);
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(
            outcome.failures[0].reason_code(),
            WitnessFailureReason::Ed25519VerifyFailed,
        );
    }

    /// `From<&WitnessFailure> for WitnessFailureWire` projects all
    /// audit-relevant fields without information loss for the wire
    /// contract: kid passes through unchanged, batch_index propagates,
    /// reason_code is the at-source categorisation, message captures
    /// the Display rendering.
    #[test]
    fn witness_failure_wire_projection_preserves_fields() {
        let f = WitnessFailure {
            batch_index: Some(3),
            witness_kid: "kid-a".to_string(),
            error: TrustError::BadWitness {
                witness_kid: "kid-a".to_string(),
                reason: "ed25519 verification failed: signature error".to_string(),
            },
            reason_code: WitnessFailureReason::Ed25519VerifyFailed,
        };
        let wire = WitnessFailureWire::from(&f);
        assert_eq!(wire.witness_kid, "kid-a");
        assert_eq!(wire.batch_index, Some(3));
        assert_eq!(wire.reason_code, WitnessFailureReason::Ed25519VerifyFailed);
        assert_eq!(
            wire.message,
            "batch[3] witness kid-a: ed25519 verification failed: signature error",
        );
    }

    /// `WitnessFailureReason` round-trips as kebab-case JSON. Pinned
    /// because downstream auditor tools depend on the wire spelling
    /// — a future renaming of the enum identifier in Rust must NOT
    /// silently change the JSON key.
    #[test]
    fn witness_failure_reason_serde_kebab_case() {
        for (variant, expected) in [
            (WitnessFailureReason::KidNotInRoster, "\"kid-not-in-roster\""),
            (WitnessFailureReason::DuplicateKid, "\"duplicate-kid\""),
            (
                WitnessFailureReason::CrossBatchDuplicateKid,
                "\"cross-batch-duplicate-kid\"",
            ),
            (
                WitnessFailureReason::InvalidSignatureFormat,
                "\"invalid-signature-format\"",
            ),
            (
                WitnessFailureReason::InvalidSignatureLength,
                "\"invalid-signature-length\"",
            ),
            (WitnessFailureReason::OversizeKid, "\"oversize-kid\""),
            (
                WitnessFailureReason::ChainHeadDecodeFailed,
                "\"chain-head-decode-failed\"",
            ),
            (
                WitnessFailureReason::Ed25519VerifyFailed,
                "\"ed25519-verify-failed\"",
            ),
            (WitnessFailureReason::Other, "\"other\""),
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected, "variant {variant:?} serialised wrong");
            let back: WitnessFailureReason = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant, "variant {variant:?} did not round-trip");
        }
    }

    /// `WitnessFailureWire` rejects unknown JSON fields — defends
    /// against a producer (or man-in-the-middle on a JSON pipeline)
    /// smuggling extra fields past a verifier that round-trips the
    /// wire form.
    #[test]
    fn witness_failure_wire_rejects_unknown_fields() {
        let json = r#"{
            "witness_kid":"k",
            "batch_index":null,
            "reason_code":"other",
            "message":"m",
            "extra":"x"
        }"#;
        let result: Result<WitnessFailureWire, _> = serde_json::from_str(json);
        assert!(result.is_err(), "unknown fields must reject");
    }

    /// `WitnessFailureWire` round-trips JSON without information
    /// loss for all four fields — the wire contract is reversible.
    #[test]
    fn witness_failure_wire_serde_roundtrip() {
        let original = WitnessFailureWire {
            witness_kid: "round-trip-kid".to_string(),
            batch_index: Some(42),
            reason_code: WitnessFailureReason::CrossBatchDuplicateKid,
            message: "batch[42] witness round-trip-kid: cross-batch dup".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: WitnessFailureWire = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original);
    }

    /// `From<&WitnessFailure>` preserves `batch_index = None` for
    /// per-batch-verifier-side failures (no chain context). Pins the
    /// absence-vs-presence semantic across the projection.
    #[test]
    fn witness_failure_wire_projection_handles_none_batch_index() {
        let f = WitnessFailure {
            batch_index: None,
            witness_kid: "no-batch-kid".to_string(),
            error: TrustError::BadWitness {
                witness_kid: "no-batch-kid".to_string(),
                reason: "boom".to_string(),
            },
            reason_code: WitnessFailureReason::KidNotInRoster,
        };
        let wire = WitnessFailureWire::from(&f);
        assert_eq!(wire.batch_index, None);
        assert_eq!(wire.reason_code, WitnessFailureReason::KidNotInRoster);
    }
}
