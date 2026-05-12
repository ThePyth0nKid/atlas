//! V2-α Welle 4: `ProjectorRunAttestation` event-kind schema + verifier-side parser.
//!
//! ## Purpose
//!
//! Welle 3 produced a byte-deterministic `graph_state_hash` primitive
//! (`atlas-projector` crate). That hash is, by itself, "CI-gate
//! material" — a value an operator can compare on every projector
//! run. Welle 4 elevates it to a **cryptographically-bound trust-chain
//! artefact** by introducing the `ProjectorRunAttestation` event-kind:
//!
//!   * a normal signed Atlas event (Ed25519 + COSE_Sign1 + Rekor
//!     anchoring + optional witness cosignature)
//!   * carrying a payload that asserts
//!     `(projector_version, head_event_hash) → graph_state_hash` for
//!     a specific projector run
//!   * verifiable offline by the WASM verifier against the same
//!     `events.jsonl` + `pubkey-bundle.json` any V1 trace uses
//!
//! After Welle 4 ships, a regulator-witness running their own offline
//! verifier can confirm "this projector run on this event-head
//! produced this graph state at time T" without trusting Atlas
//! operator. The CI gate (Welle 6 candidate) becomes a cryptographic
//! check instead of a comparison.
//!
//! ## Welle-4 scope (consumer/verifier side)
//!
//! This module owns:
//!   * `PROJECTOR_RUN_ATTESTATION_KIND` payload `type` discriminator
//!   * `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION` envelope schema
//!     identifier (separate from `atlas-projector-v1-alpha` GraphState
//!     canonical-form version)
//!   * `ProjectorRunAttestation` typed struct
//!   * `parse_projector_run_attestation` JSON-to-typed parser
//!   * `validate_projector_run_attestation` strict format-validator
//!
//! Welle 4 does **not** implement attestation emission. That's the
//! producer side and arrives with Welle 5 (full projector reading
//! `events.jsonl`). Welle 4 also does **not** enforce cross-event
//! integrity (head_event_hash actually points to an event in the
//! same trace) — that's the broader trust-chain check the verifier
//! adds in a later welle.
//!
//! ## Format validation enforced by Welle 4
//!
//! - `projector_version` non-empty (e.g. `"atlas-projector/0.1.0"`)
//! - `projector_schema_version` equals `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION`
//! - `head_event_hash` is exactly 64 lowercase-hex characters (blake3 width)
//! - `graph_state_hash` is exactly 64 lowercase-hex characters (blake3 width)
//! - `projected_event_count` >= 1
//!
//! Any failure surfaces as `TrustError::ProjectorAttestationInvalid`
//! with a structured reason naming the specific format violation.
//!
//! ## Wire-compat policy (V2-α)
//!
//! `AtlasPayload::ProjectorRunAttestation` is a new variant of the
//! existing enum. V1.0 verifiers reading a V2-α event whose
//! `payload.type` is `"projector_run_attestation"` will reject
//! deserialisation if they try typed-parse into `AtlasPayload`
//! (untagged unknown variant) — but the underlying `AtlasEvent.payload`
//! is `serde_json::Value` so V1 verifiers can pass-through-handle
//! these events without rejecting them at the trace-deserialisation
//! boundary. The verifier MUST still treat such events as
//! cryptographically valid (signature checks pass against the same
//! signing input), but cannot apply attestation-specific format
//! validation. This is the by-design V2 major-bump break, documented
//! in `docs/SEMVER-AUDIT-V1.0.md` §10.

use serde_json::Value;

use crate::error::{TrustError, TrustResult};

/// Payload `type` discriminator (matches `#[serde(tag = "type",
/// rename_all = "snake_case")]` convention on `AtlasPayload`).
pub const PROJECTOR_RUN_ATTESTATION_KIND: &str = "projector_run_attestation";

/// Wire-format schema-version identifier for the
/// `ProjectorRunAttestation` envelope. Bumped on schema-incompatible
/// changes. **Separate from `atlas_projector::PROJECTOR_SCHEMA_VERSION`**
/// (which versions the GraphState canonical form, not the attestation
/// envelope).
pub const PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION: &str =
    "atlas-projector-run-attestation/v1-alpha";

/// Expected length of a blake3 hex hash: 64 lowercase-hex chars.
const BLAKE3_HEX_LEN: usize = 64;

/// Typed `ProjectorRunAttestation` payload. Constructed via
/// `parse_projector_run_attestation` from a `serde_json::Value`.
///
/// **Field semantics:**
///   * `projector_version` — issuer-supplied identifier of the
///     specific projector binary that produced this attestation
///     (e.g. `"atlas-projector/0.1.0"`). Format: non-empty string.
///     Welle 4 does NOT pin a specific version — that's deployment
///     policy. Welle 6 CI gate may add roster-based version-pinning.
///   * `projector_schema_version` — envelope schema version. MUST
///     equal `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION` for events
///     emitted by Welle-4-compatible projectors.
///   * `head_event_hash` — blake3 hex of the last Atlas event the
///     projector consumed before computing `graph_state_hash`.
///     Format: 64 lowercase-hex.
///   * `graph_state_hash` — blake3 hex of
///     `atlas_projector::graph_state_hash(state)` output. Format:
///     64 lowercase-hex.
///   * `projected_event_count` — non-zero count of Atlas events
///     consumed by this projector run. Audit-trail field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectorRunAttestation {
    /// Issuer-supplied identifier of the projector binary
    /// (e.g. `"atlas-projector/0.1.0"`).
    pub projector_version: String,

    /// Envelope schema version. MUST equal
    /// `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION`.
    pub projector_schema_version: String,

    /// blake3 hex of the last Atlas event consumed (64 lowercase-hex).
    pub head_event_hash: String,

    /// blake3 hex of `atlas_projector::graph_state_hash(state)`
    /// (64 lowercase-hex).
    pub graph_state_hash: String,

    /// Number of Atlas events the projector consumed. Non-zero.
    pub projected_event_count: u64,
}

/// Parse a `ProjectorRunAttestation` from a JSON payload. The payload
/// is expected to be a JSON object with the 5 required fields plus
/// the `type` discriminator (which is validated equal to
/// `PROJECTOR_RUN_ATTESTATION_KIND`).
///
/// Returns `TrustError::ProjectorAttestationInvalid` with a
/// structured reason on any of:
///   * payload is not a JSON object
///   * `type` field missing or not equal to
///     `PROJECTOR_RUN_ATTESTATION_KIND`
///   * any of the 5 required fields missing or wrong type
///   * unknown extra fields present (strict-mode reject)
///
/// This is a **structural-shape check only** — semantic validation
/// (hash-format strictness, schema-version match, non-zero count)
/// happens in `validate_projector_run_attestation`. The split is
/// intentional: the parser builds the typed struct from
/// well-shaped JSON, surfacing structural problems (wrong type
/// discriminator, missing/extra fields, wrong JSON type for a
/// field, non-integer or float-disguised counts). The validator
/// then enforces semantic constraints on the typed values
/// (non-empty version, schema-version equality, hex-format
/// strictness, non-zero count). Both stages reject; the split
/// only changes which stage emits a given diagnostic — useful
/// because a downstream consumer may want to surface
/// "structurally invalid" vs "semantically invalid" differently.
pub fn parse_projector_run_attestation(payload: &Value) -> TrustResult<ProjectorRunAttestation> {
    let obj = payload.as_object().ok_or_else(|| {
        TrustError::ProjectorAttestationInvalid {
            reason: "payload is not a JSON object".to_string(),
        }
    })?;

    // type discriminator
    let kind = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| TrustError::ProjectorAttestationInvalid {
            reason: "missing required field 'type'".to_string(),
        })?;
    if kind != PROJECTOR_RUN_ATTESTATION_KIND {
        return Err(TrustError::ProjectorAttestationInvalid {
            reason: format!(
                "wrong payload type: expected '{PROJECTOR_RUN_ATTESTATION_KIND}', got '{kind}'"
            ),
        });
    }

    // Allowed field set — strict-mode reject of unknown fields.
    const ALLOWED_FIELDS: &[&str] = &[
        "type",
        "projector_version",
        "projector_schema_version",
        "head_event_hash",
        "graph_state_hash",
        "projected_event_count",
    ];
    for key in obj.keys() {
        if !ALLOWED_FIELDS.contains(&key.as_str()) {
            return Err(TrustError::ProjectorAttestationInvalid {
                reason: format!("unknown field '{key}'"),
            });
        }
    }

    let projector_version = required_string_field(obj, "projector_version")?;
    let projector_schema_version = required_string_field(obj, "projector_schema_version")?;
    let head_event_hash = required_string_field(obj, "head_event_hash")?;
    let graph_state_hash = required_string_field(obj, "graph_state_hash")?;
    let count_value = obj.get("projected_event_count").ok_or_else(|| {
        TrustError::ProjectorAttestationInvalid {
            reason: "missing 'projected_event_count'".to_string(),
        }
    })?;
    // Strict integer check: reject JSON floats (e.g. `5.0`) even if they
    // happen to be whole numbers. `serde_json::Value::Number::is_u64`
    // returns false for any number that was parsed as a float, even
    // if its value would fit in u64. This catches issuer-side
    // ambiguity in wire formats that emit unconstrained JSON numbers.
    let count_num = count_value
        .as_number()
        .ok_or_else(|| TrustError::ProjectorAttestationInvalid {
            reason: "'projected_event_count' is not a JSON number".to_string(),
        })?;
    if !count_num.is_u64() {
        return Err(TrustError::ProjectorAttestationInvalid {
            reason: format!(
                "'projected_event_count' must be a non-negative integer fitting in u64 (got {count_num})"
            ),
        });
    }
    let projected_event_count = count_num.as_u64().expect("is_u64() just succeeded");

    Ok(ProjectorRunAttestation {
        projector_version,
        projector_schema_version,
        head_event_hash,
        graph_state_hash,
        projected_event_count,
    })
}

/// Strict format-validation. Returns `Ok(())` if the attestation is
/// well-formed; otherwise returns `TrustError::ProjectorAttestationInvalid`
/// with a structured reason naming the specific violation.
///
/// Enforced rules:
///   * `projector_version` is non-empty after trimming
///   * `projector_schema_version` equals
///     `PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION`
///   * `head_event_hash` is exactly 64 lowercase-hex characters
///   * `graph_state_hash` is exactly 64 lowercase-hex characters
///   * `projected_event_count` is at least 1
pub fn validate_projector_run_attestation(att: &ProjectorRunAttestation) -> TrustResult<()> {
    if att.projector_version.trim().is_empty() {
        return Err(TrustError::ProjectorAttestationInvalid {
            reason: "projector_version is empty".to_string(),
        });
    }
    if att.projector_schema_version != PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION {
        return Err(TrustError::ProjectorAttestationInvalid {
            reason: format!(
                "projector_schema_version mismatch: expected '{}', got '{}'",
                PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION, att.projector_schema_version
            ),
        });
    }
    require_lowercase_hex_with_len(&att.head_event_hash, "head_event_hash")?;
    require_lowercase_hex_with_len(&att.graph_state_hash, "graph_state_hash")?;
    if att.projected_event_count == 0 {
        return Err(TrustError::ProjectorAttestationInvalid {
            reason: "projected_event_count must be >= 1".to_string(),
        });
    }
    Ok(())
}

fn required_string_field(
    obj: &serde_json::Map<String, Value>,
    field: &str,
) -> TrustResult<String> {
    obj.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| TrustError::ProjectorAttestationInvalid {
            reason: format!("missing or non-string field '{field}'"),
        })
}

fn require_lowercase_hex_with_len(s: &str, field: &str) -> TrustResult<()> {
    if s.len() != BLAKE3_HEX_LEN {
        return Err(TrustError::ProjectorAttestationInvalid {
            reason: format!(
                "{field} must be exactly {BLAKE3_HEX_LEN} lowercase-hex characters (got {})",
                s.len()
            ),
        });
    }
    // Iterate via .all() — DIDs/hashes are non-secret, short-circuit
    // timing is acceptable (matches the Welle 1 / agent_did convention).
    let all_lowercase_hex = s
        .bytes()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !all_lowercase_hex {
        return Err(TrustError::ProjectorAttestationInvalid {
            reason: format!(
                "{field} must contain only lowercase-hex characters [0-9a-f]"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Synthetic 64-lowercase-hex fixtures used for format-validation
    /// tests. NOT real blake3 hashes — chosen for stable, easily
    /// readable byte sequences in test failures.
    const FIXTURE_HEAD: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const FIXTURE_STATE: &str =
        "8962c1681a44f9569f78c5917f568c5a027ac69f727f23ba5e8f871e5e013ac4";

    fn well_formed_payload() -> Value {
        json!({
            "type": PROJECTOR_RUN_ATTESTATION_KIND,
            "projector_version": "atlas-projector/0.1.0",
            "projector_schema_version": PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION,
            "head_event_hash": FIXTURE_HEAD,
            "graph_state_hash": FIXTURE_STATE,
            "projected_event_count": 5_u64,
        })
    }

    #[test]
    fn parse_roundtrip_well_formed() {
        let p = well_formed_payload();
        let att = parse_projector_run_attestation(&p).unwrap();
        assert_eq!(att.projector_version, "atlas-projector/0.1.0");
        assert_eq!(
            att.projector_schema_version,
            PROJECTOR_RUN_ATTESTATION_SCHEMA_VERSION
        );
        assert_eq!(att.head_event_hash, FIXTURE_HEAD);
        assert_eq!(att.graph_state_hash, FIXTURE_STATE);
        assert_eq!(att.projected_event_count, 5);
    }

    #[test]
    fn parse_rejects_non_object_payload() {
        let p = json!("not-an-object");
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("not a JSON object"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_wrong_type_discriminator() {
        let mut p = well_formed_payload();
        p["type"] = json!("not_projector_attestation");
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("wrong payload type"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_missing_type() {
        let mut p = well_formed_payload();
        p.as_object_mut().unwrap().remove("type");
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("missing required field 'type'"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_unknown_field() {
        let mut p = well_formed_payload();
        p["nope_unknown"] = json!("extra");
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("unknown field 'nope_unknown'"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_missing_required_string_field() {
        let mut p = well_formed_payload();
        p.as_object_mut().unwrap().remove("head_event_hash");
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("missing or non-string field 'head_event_hash'"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_non_string_field() {
        let mut p = well_formed_payload();
        p["graph_state_hash"] = json!(42);
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("missing or non-string field 'graph_state_hash'"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_missing_count() {
        let mut p = well_formed_payload();
        p.as_object_mut().unwrap().remove("projected_event_count");
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("projected_event_count"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_float_count_even_if_whole_number() {
        // Strict-integer wire-format: reject `5.0` (float-disguised-as-
        // integer) even if the value fits in u64. Catches issuer-side
        // wire-format ambiguity.
        let mut p = well_formed_payload();
        p["projected_event_count"] = json!(5.0);
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(
                    reason.contains("must be a non-negative integer"),
                    "expected strict-integer rejection; got: {reason}"
                );
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_negative_count() {
        let mut p = well_formed_payload();
        p["projected_event_count"] = json!(-1);
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(
                    reason.contains("must be a non-negative integer"),
                    "expected non-negative-integer rejection; got: {reason}"
                );
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_non_number_count() {
        let mut p = well_formed_payload();
        p["projected_event_count"] = json!("not-a-number");
        match parse_projector_run_attestation(&p) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(
                    reason.contains("not a JSON number"),
                    "expected non-number rejection; got: {reason}"
                );
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn validate_succeeds_on_well_formed_attestation() {
        let att = parse_projector_run_attestation(&well_formed_payload()).unwrap();
        assert!(validate_projector_run_attestation(&att).is_ok());
    }

    #[test]
    fn validate_rejects_empty_projector_version() {
        let mut att = parse_projector_run_attestation(&well_formed_payload()).unwrap();
        att.projector_version = "   ".to_string();
        match validate_projector_run_attestation(&att) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("projector_version is empty"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_schema_version_mismatch() {
        let mut att = parse_projector_run_attestation(&well_formed_payload()).unwrap();
        att.projector_schema_version = "wrong-version-string".to_string();
        match validate_projector_run_attestation(&att) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("projector_schema_version mismatch"));
                assert!(reason.contains("wrong-version-string"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_wrong_length_hex_hash() {
        let mut att = parse_projector_run_attestation(&well_formed_payload()).unwrap();
        att.head_event_hash = "abc123".to_string();
        match validate_projector_run_attestation(&att) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("head_event_hash"));
                assert!(reason.contains("lowercase-hex"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_uppercase_hex_hash() {
        let mut att = parse_projector_run_attestation(&well_formed_payload()).unwrap();
        att.graph_state_hash = FIXTURE_STATE.to_uppercase();
        match validate_projector_run_attestation(&att) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("graph_state_hash"));
                assert!(reason.contains("lowercase-hex"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_zero_projected_event_count() {
        let mut att = parse_projector_run_attestation(&well_formed_payload()).unwrap();
        att.projected_event_count = 0;
        match validate_projector_run_attestation(&att) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains(">= 1"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_non_hex_chars() {
        let mut att = parse_projector_run_attestation(&well_formed_payload()).unwrap();
        // Replace one char with 'g' (not hex)
        let mut chars: Vec<char> = att.head_event_hash.chars().collect();
        chars[10] = 'g';
        att.head_event_hash = chars.iter().collect();
        match validate_projector_run_attestation(&att) {
            Err(TrustError::ProjectorAttestationInvalid { reason }) => {
                assert!(reason.contains("lowercase-hex"));
            }
            other => panic!("expected ProjectorAttestationInvalid; got {other:?}"),
        }
    }
}
