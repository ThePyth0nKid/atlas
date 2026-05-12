//! Agent identity as W3C-DID (V2-α Welle 1).
//!
//! V1 identified signers exclusively by workspace via the HKDF-derived
//! per-tenant kid (`atlas-anchor:{workspace_id}`, see `per_tenant.rs`).
//! V1's trust model said: "this event was signed by *someone with the
//! per-tenant signing key for workspace X*". Multi-agent attribution
//! — *which agent inside workspace X wrote this fact* — was not part of
//! the V1 signing input.
//!
//! V2-α adds an orthogonal agent-identity layer. Every event MAY carry
//! an `author_did` field naming the specific agent instance that
//! produced the event. The DID is `did:atlas:<lowercase-hex-32-bytes>`,
//! where the 32 hex bytes are `blake3_hash(ed25519_public_key)` of the
//! agent's signing key. Two-layer event binding (Phase 2 Security H-1):
//!
//!   * `kid`        — workspace HKDF anchor (V1 cross-workspace-replay defence)
//!   * `author_did` — agent identity (V2 cross-agent-replay defence)
//!
//! Both are part of the canonical signing input (see `cose.rs`), so an
//! event signed by agent A in workspace X cannot be replayed as if
//! signed by agent B in workspace X, nor as if signed by agent A in
//! workspace Y. The agent is responsible for custody of its own
//! Ed25519 private key (Atlas does NOT derive agent keys from a master
//! seed — agents hold their own keys on HSM / keychain / hardware
//! tokens).
//!
//! ## What this module owns
//!
//!   * `AGENT_DID_PREFIX` — the literal `"did:atlas:"` prefix.
//!   * `agent_did_for(pubkey_hash)` — build the canonical DID string.
//!   * `parse_agent_did(did)` — return `Some(pubkey_hash)` if `did` is
//!     a well-formed `did:atlas:` DID, else `None`. Strict: requires
//!     the prefix AND exactly 64 lowercase-hex characters as suffix.
//!   * `validate_agent_did(did)` — `Result<(), AgentDidError>` for
//!     verifier-side strict validation with structured error variants.
//!
//! ## Why the format is locked at parse-time, not at the canonicaliser
//!
//! Per the Welle-1 plan-doc §"Decisions", the format is
//! `did:atlas:<lowercase-hex-32-bytes>`. We could let the canonicaliser
//! accept arbitrary text and only check format at verify-time — that's
//! the V1 pattern for `kid` (see `per_tenant.rs` docstring on leniency).
//! For `author_did` we instead validate at parse-time because:
//!
//!   * Agent identity uses blake3(pubkey) as a stable cryptographic
//!     handle. There is no caller-controlled string here equivalent to
//!     `workspace_id` — the hash is mechanically derivable from the
//!     public key, so format-deviation is an issuer bug, not a policy
//!     choice. Catching it at parse-time stops bad DIDs from entering
//!     the trust chain in the first place.
//!   * Lyrie ATP compatibility (DECISION-BIZ-6, deferred to V2-γ) will
//!     likely use a comparable hash-based DID format; locking ours
//!     strictly now preserves the option to alias-map later without
//!     accumulating a corpus of malformed DIDs.
//!
//! ## Wire-compat note (V2-α major break, by design)
//!
//! `AtlasEvent` carries `author_did: Option<String>` with
//! `#[serde(default, skip_serializing_if = "Option::is_none")]`, but
//! the struct also carries `#[serde(deny_unknown_fields)]`. A V1.0
//! verifier reading a V2-α event whose `author_did` field is `Some(...)`
//! will reject deserialization with `unknown_field("author_did")`.
//! This is intentional: V2 = major bump. V1 events without
//! `author_did` remain forward-compatible; V2-α events that carry
//! `author_did` require a V2-aware verifier. Per Welle 1 plan, the
//! workspace version bump to `2.0.0-alpha.1` is deferred to the end
//! of the V2-α welle bundle, not Welle 1 alone.

use crate::error::{TrustError, TrustResult};

/// Canonical prefix for Atlas agent-DIDs.
///
/// The full DID is `format!("{AGENT_DID_PREFIX}{pubkey_hash}")` where
/// `pubkey_hash` is exactly 64 lowercase-hex characters (32 bytes,
/// blake3 of the agent's Ed25519 public-key bytes).
///
/// The colon is part of the prefix so the DID-method namespace
/// (`did:atlas:`) sits next to other Atlas namespaces (`atlas-anchor:`,
/// `atlas-policy:`, `atlas-witness:`) without ambiguity at the parser
/// layer. Note that DID-method namespaces use the W3C `did:` scheme
/// while Atlas internal kid namespaces use bare prefixes.
pub const AGENT_DID_PREFIX: &str = "did:atlas:";

/// Length of the hex suffix (64 chars = 32 bytes = blake3 output width).
const PUBKEY_HASH_HEX_LEN: usize = 64;

/// Build the canonical agent-DID for a given blake3 public-key hash.
///
/// `pubkey_hash` is taken verbatim — the caller is responsible for
/// having already computed `blake3(ed25519_public_key)` and formatted
/// it as lowercase hex. This function does no hashing; it is a
/// presentation-layer helper for issuers that already have the hash
/// in hand (atlas-signer, an external SDK, etc.).
///
/// If you have the raw public-key bytes, the recommended flow is:
/// `let hash_hex = hex::encode(blake3::hash(&pubkey_bytes).as_bytes());`
/// then `agent_did_for(&hash_hex)`. The hex crate is a workspace
/// dependency.
///
/// The returned DID is NOT format-validated by this function; callers
/// who construct DIDs from suspect input should run
/// `validate_agent_did` on the result before placing it on the wire.
/// Note: `cose::build_signing_input` also does NOT format-validate
/// `author_did` at sign time — validation lives on the verifier-side
/// path. Issuers SHOULD validate before signing.
pub fn agent_did_for(pubkey_hash: &str) -> String {
    format!("{AGENT_DID_PREFIX}{pubkey_hash}")
}

/// Return `Some(pubkey_hash)` if `did` is a well-formed Atlas agent-DID,
/// else `None`.
///
/// Strict semantics:
///   * The prefix must match `did:atlas:` exactly.
///   * The suffix must be exactly 64 lowercase-hex characters.
///   * Any other suffix length, any uppercase character, any non-hex
///     character is rejected.
///
/// This parser is **stricter** than `parse_per_tenant_kid` because
/// the agent-DID suffix is a cryptographic hash with a fixed shape,
/// not a caller-controlled namespace string. See module docstring.
pub fn parse_agent_did(did: &str) -> Option<&str> {
    let suffix = did.strip_prefix(AGENT_DID_PREFIX)?;
    if !is_lowercase_hex_with_len(suffix, PUBKEY_HASH_HEX_LEN) {
        return None;
    }
    Some(suffix)
}

/// Verifier-side strict validation. Returns `Ok(())` if `did` is a
/// well-formed Atlas agent-DID, else `TrustError::AgentDidFormatInvalid`
/// with a structured reason naming the specific format violation.
///
/// Verifier callers typically invoke this on every event whose
/// `author_did` field is `Some(_)` and treat the error as a hard
/// reject — a malformed DID inside a signed event is an issuer bug
/// or a tampering attempt, and the verifier MUST refuse to validate
/// the trust chain either way.
pub fn validate_agent_did(did: &str) -> TrustResult<()> {
    let suffix = match did.strip_prefix(AGENT_DID_PREFIX) {
        Some(s) => s,
        None => {
            return Err(TrustError::AgentDidFormatInvalid {
                did: did.to_string(),
                reason: format!("missing required prefix '{AGENT_DID_PREFIX}'"),
            });
        }
    };

    if suffix.len() != PUBKEY_HASH_HEX_LEN {
        return Err(TrustError::AgentDidFormatInvalid {
            did: did.to_string(),
            reason: format!(
                "pubkey-hash suffix must be exactly {PUBKEY_HASH_HEX_LEN} lowercase-hex characters \
                 (got {} characters)",
                suffix.len()
            ),
        });
    }

    if !is_lowercase_hex(suffix) {
        return Err(TrustError::AgentDidFormatInvalid {
            did: did.to_string(),
            reason: "pubkey-hash suffix must contain only lowercase hex characters [0-9a-f]"
                .to_string(),
        });
    }

    Ok(())
}

/// Lowercase-hex character check. DIDs are not secrets, so this is NOT
/// constant-time — `.all()` short-circuits on the first non-matching
/// byte. Documented explicitly because earlier drafts of this file
/// claimed a timing-stability invariant that the implementation does
/// not provide. If a future caller needs constant-time comparison on
/// DID material, write a dedicated function rather than retrofitting
/// this one.
///
/// Vacuous-truth note: `is_lowercase_hex("")` returns `true`. Callers
/// who care about non-empty input MUST pre-check length (or use
/// `is_lowercase_hex_with_len` which enforces an exact length).
fn is_lowercase_hex(s: &str) -> bool {
    s.bytes()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Length-and-character check combined. Rejects empty input by virtue
/// of the length check (an `is_lowercase_hex_with_len(s, 0)` call would
/// pass on `""` but is meaningless — the only caller (`parse_agent_did`)
/// passes `PUBKEY_HASH_HEX_LEN = 64`).
fn is_lowercase_hex_with_len(s: &str, len: usize) -> bool {
    s.len() == len && is_lowercase_hex(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 64-lowercase-hex synthetic fixture for format-validation tests.
    /// NOT a real blake3 hash of any input — chosen to exercise the
    /// `did:atlas:<64-hex>` shape with stable, easily-readable bytes.
    /// Integration tests in `tests/agent_did_integration.rs` use the
    /// actual blake3 hash of a test signing key's public-key bytes.
    const FIXTURE_HASH: &str = "a44f44b3d6c9d4e2c84b6d40f6e0e0e8d8e3f2c1a0f9e8d7c6b5a4938271605f";

    #[test]
    fn agent_did_for_is_deterministic_string() {
        assert_eq!(
            agent_did_for(FIXTURE_HASH),
            format!("did:atlas:{FIXTURE_HASH}")
        );
    }

    #[test]
    fn parse_roundtrip_succeeds() {
        let did = agent_did_for(FIXTURE_HASH);
        assert_eq!(parse_agent_did(&did), Some(FIXTURE_HASH));
    }

    #[test]
    fn parse_rejects_wrong_prefix() {
        // Method namespace must be `atlas`, not `web` / `key` / other.
        assert_eq!(parse_agent_did("did:web:example.com"), None);
        assert_eq!(parse_agent_did("did:key:z6MkhaXgBZ"), None);
        // Bare prefix without scheme is also rejected.
        assert_eq!(parse_agent_did("atlas-anchor:ws1"), None);
        // Empty.
        assert_eq!(parse_agent_did(""), None);
    }

    #[test]
    fn parse_rejects_uppercase_hex() {
        let upper = FIXTURE_HASH.to_uppercase();
        assert_eq!(parse_agent_did(&format!("did:atlas:{upper}")), None);
    }

    #[test]
    fn parse_rejects_short_suffix() {
        // 63 chars — one short.
        let short = &FIXTURE_HASH[..PUBKEY_HASH_HEX_LEN - 1];
        assert_eq!(parse_agent_did(&format!("did:atlas:{short}")), None);
    }

    #[test]
    fn parse_rejects_long_suffix() {
        // 65 chars — one over.
        let long = format!("{FIXTURE_HASH}f");
        assert_eq!(parse_agent_did(&format!("did:atlas:{long}")), None);
    }

    #[test]
    fn parse_rejects_non_hex() {
        // Contains a 'g' (not a hex character).
        let bad = "a44f44b3d6c9d4e2c84b6d40f6e0e0e8d8e3f2c1a0f9e8d7c6b5a493827160g5f";
        assert_eq!(parse_agent_did(&format!("did:atlas:{bad}")), None);
    }

    #[test]
    fn parse_rejects_empty_suffix() {
        // `did:atlas:` with no hash at all.
        assert_eq!(parse_agent_did(AGENT_DID_PREFIX), None);
    }

    #[test]
    fn validate_succeeds_on_well_formed_did() {
        let did = agent_did_for(FIXTURE_HASH);
        assert!(validate_agent_did(&did).is_ok());
    }

    #[test]
    fn validate_returns_structured_error_on_missing_prefix() {
        let err = validate_agent_did("not-a-did").unwrap_err();
        match err {
            TrustError::AgentDidFormatInvalid { did, reason } => {
                assert_eq!(did, "not-a-did");
                assert!(reason.contains("missing required prefix"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn validate_returns_structured_error_on_wrong_length() {
        let short = &FIXTURE_HASH[..50];
        let bad = format!("did:atlas:{short}");
        let err = validate_agent_did(&bad).unwrap_err();
        match err {
            TrustError::AgentDidFormatInvalid { reason, .. } => {
                assert!(reason.contains("lowercase-hex characters"));
                assert!(reason.contains("got 50"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn validate_returns_structured_error_on_uppercase_hex() {
        let upper = FIXTURE_HASH.to_uppercase();
        let bad = format!("did:atlas:{upper}");
        let err = validate_agent_did(&bad).unwrap_err();
        match err {
            TrustError::AgentDidFormatInvalid { reason, .. } => {
                assert!(reason.contains("lowercase hex"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn lowercase_hex_helper_accepts_only_valid_chars() {
        assert!(is_lowercase_hex("0123456789abcdef"));
        assert!(!is_lowercase_hex("0123ABCDEF"));
        assert!(!is_lowercase_hex("hello"));
        // Vacuous-truth: empty input passes the character check. Callers
        // who care about non-empty input MUST pre-check length.
        // Documented in `is_lowercase_hex` doc-comment.
        assert!(is_lowercase_hex(""));
    }

    #[test]
    fn parse_rejects_control_chars_in_suffix() {
        // Embedded NUL byte in an otherwise-correct-length suffix must
        // be rejected by the lowercase-hex check. Documents the
        // "bytes not chars" semantics: NUL = 0x00, not in [0-9a-f].
        let mut bytes = [b'a'; 64];
        bytes[10] = 0x00;
        let suffix = std::str::from_utf8(&bytes).unwrap();
        assert_eq!(parse_agent_did(&format!("did:atlas:{suffix}")), None);
    }

    #[test]
    fn parse_rejects_whitespace_in_suffix() {
        // Space, tab, newline — all rejected by the hex-character check.
        let mut chars: Vec<char> = "a".repeat(64).chars().collect();
        chars[10] = ' ';
        let suffix: String = chars.iter().collect();
        assert_eq!(parse_agent_did(&format!("did:atlas:{suffix}")), None);
    }

    #[test]
    fn parse_rejects_unicode_multibyte_in_suffix() {
        // A 2-byte UTF-8 character (e.g. 'é' = 0xC3 0xA9) occupies 2
        // bytes in `s.bytes()` iteration but only 1 `char`. Length check
        // sees 64 bytes total when the visible char count is 63 +
        // the multi-byte. Either way the lowercase-hex check rejects
        // because 0xC3 and 0xA9 are not in [0-9a-f].
        let mut suffix = String::from("a".repeat(62));
        suffix.push('é'); // 2 bytes UTF-8 — total byte length now 64
        assert_eq!(parse_agent_did(&format!("did:atlas:{suffix}")), None);
    }

    #[test]
    fn validate_rejects_empty_string() {
        // `validate_agent_did("")` — empty string is missing the prefix.
        // Structured error reports missing prefix, not zero-length suffix
        // (the strip_prefix returns None before the length check).
        let err = validate_agent_did("").unwrap_err();
        match err {
            TrustError::AgentDidFormatInvalid { reason, .. } => {
                assert!(reason.contains("missing required prefix"));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
