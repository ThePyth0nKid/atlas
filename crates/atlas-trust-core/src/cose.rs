//! Deterministic CBOR canonicalisation for COSE_Sign1 envelopes.
//!
//! V1 simplification: we don't yet use full coset COSE_Sign1.
//! Instead we use a simplified deterministic CBOR-canonical signing-input format
//! that is bit-identical across implementations.
//!
//! V2 will switch to RFC 9052 COSE_Sign1 with full CTAP2 canonical CBOR.
//!
//! Determinism rules applied here (RFC 8949 §4.2.1, "Core Deterministic Encoding"):
//!   - Smallest argument encoding (handled by `ciborium` for our value shapes).
//!   - Map keys sorted by **length of their encoded form first**, then bytewise lex.
//!     This is the key rule the previous implementation got wrong: lex-only sort
//!     diverges from §4.2.1 once keys of mixed length appear.
//!   - Floats are rejected at the canonicaliser boundary — they serialise
//!     non-deterministically across CBOR variants and across float libraries.
//!     Callers must use bounded integer encodings (e.g. basis points).
//!   - `Vec::with_capacity` is capped at `MAX_ITEMS_PER_LEVEL` to bound allocation
//!     under hostile input.

use ciborium::Value;
use std::io::Cursor;

use crate::error::{TrustError, TrustResult};

/// Hard cap on items per array/map level. Bounds allocation under hostile input.
/// 10k is comfortably above any real Atlas event but rejects pathological inputs.
const MAX_ITEMS_PER_LEVEL: usize = 10_000;

/// Build the canonical signing-input bytes for an event.
///
/// Format (CBOR map, deterministic encoding per RFC 8949 §4.2.1):
/// ```text
/// {
///   "v": "atlas-trace-v1",
///   "workspace": <workspace_id>,
///   "id": <event_id>,
///   "ts": <ts>,
///   "kid": <kid>,
///   "parents": [<parent_hash_1>, ...],
///   "payload": <payload-cbor-canonical>,
///   // V2-α optional, present only when author_did is Some:
///   "author_did": <did:atlas:...>
/// }
/// ```
///
/// `workspace_id` is bound into the envelope so an event signed for workspace A
/// cannot be replayed into workspace B. Without this binding the same event
/// hash + signature would verify under any workspace's bundle, allowing a
/// single compromised key to silently move events across workspace boundaries.
///
/// **V2-α Welle 1:** `author_did` is OPTIONAL agent-identity (`did:atlas:...`).
/// When `Some`, it is canonically bound into the signing input alongside `kid`
/// per Phase 2 Security H-1, providing cross-agent-replay defence: an event
/// signed by agent A in workspace X cannot be replayed as if signed by agent B
/// in workspace X. When `None`, the field is omitted from the CBOR map and the
/// signing input is byte-identical to V1's shape (preserves V1 byte-determinism
/// pin).
///
/// **Issuer-side responsibility:** this function does NOT format-validate
/// `author_did`. The verifier-side path (`verify::verify_trace`) calls
/// `agent_did::validate_agent_did` on every present `author_did` before
/// running this function, so malformed DIDs are caught there with a
/// structured `TrustError::AgentDidFormatInvalid` error. Issuers (atlas-signer
/// and downstream SDKs) SHOULD pre-validate via `agent_did::validate_agent_did`
/// to get early feedback rather than producing a signed event the verifier
/// will reject. Adding validation here as defence-in-depth was considered
/// and rejected because it would surface the failure mode as a misleading
/// "hash mismatch: invalid agent-DID ..." (since `check_event_hashes` is
/// the first verifier stage that calls `build_signing_input`).
///
/// This function must be deterministic: two callers building the same logical
/// event MUST produce byte-identical output.
pub fn build_signing_input(
    workspace_id: &str,
    event_id: &str,
    ts: &str,
    kid: &str,
    parent_hashes: &[String],
    payload_json: &serde_json::Value,
    author_did: Option<&str>,
) -> TrustResult<Vec<u8>> {
    if parent_hashes.len() > MAX_ITEMS_PER_LEVEL {
        return Err(TrustError::Encoding(format!(
            "parent_hashes exceeds max items ({} > {})",
            parent_hashes.len(),
            MAX_ITEMS_PER_LEVEL
        )));
    }

    let payload_cbor = json_to_canonical_cbor(payload_json)?;

    let parents_cbor: Vec<Value> = parent_hashes
        .iter()
        .map(|h| Value::Text(h.clone()))
        .collect();

    let mut entries: Vec<(Value, Value)> = vec![
        (Value::Text("v".into()), Value::Text(crate::SCHEMA_VERSION.into())),
        (Value::Text("workspace".into()), Value::Text(workspace_id.into())),
        (Value::Text("id".into()), Value::Text(event_id.into())),
        (Value::Text("ts".into()), Value::Text(ts.into())),
        (Value::Text("kid".into()), Value::Text(kid.into())),
        (Value::Text("parents".into()), Value::Array(parents_cbor)),
        (Value::Text("payload".into()), payload_cbor),
    ];
    if let Some(did) = author_did {
        entries.push((Value::Text("author_did".into()), Value::Text(did.into())));
    }
    let sorted = sort_cbor_map_entries(entries)?;
    let envelope = Value::Map(sorted);

    let mut buf = Vec::new();
    ciborium::ser::into_writer(&envelope, &mut buf)
        .map_err(|e| TrustError::Encoding(format!("cbor serialize: {e}")))?;

    Ok(buf)
}

/// Sort map entries per RFC 8949 §4.2.1 ("Core Deterministic Encoding"):
/// length of encoded key first (shortest-first), then bytewise lex.
fn sort_cbor_map_entries(entries: Vec<(Value, Value)>) -> TrustResult<Vec<(Value, Value)>> {
    let mut with_keys: Vec<(Vec<u8>, Value, Value)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let mut buf = Vec::new();
        ciborium::ser::into_writer(&k, &mut buf)
            .map_err(|e| TrustError::Encoding(format!("cbor key serialize: {e}")))?;
        with_keys.push((buf, k, v));
    }
    with_keys.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then_with(|| a.0.cmp(&b.0)));
    Ok(with_keys.into_iter().map(|(_, k, v)| (k, v)).collect())
}

/// Convert a `serde_json::Value` to a canonical CBOR `Value`.
///
/// - Maps are sorted per RFC 8949 §4.2.1 (length-first, then lex on encoded key).
/// - Floats are rejected; integers stay as CBOR integers.
/// - Per-level item count is bounded by `MAX_ITEMS_PER_LEVEL` to cap allocation.
fn json_to_canonical_cbor(json: &serde_json::Value) -> TrustResult<Value> {
    match json {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i.into()))
            } else if let Some(u) = n.as_u64() {
                Ok(Value::Integer(u.into()))
            } else {
                Err(TrustError::Encoding(format!(
                    "non-integer number rejected by canonical CBOR: {n}. \
                     Use integer encodings (e.g. basis points) for fractional values."
                )))
            }
        }
        serde_json::Value::String(s) => Ok(Value::Text(s.clone())),
        serde_json::Value::Array(arr) => {
            if arr.len() > MAX_ITEMS_PER_LEVEL {
                return Err(TrustError::Encoding(format!(
                    "array exceeds max items per level ({} > {})",
                    arr.len(),
                    MAX_ITEMS_PER_LEVEL
                )));
            }
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(json_to_canonical_cbor(item)?);
            }
            Ok(Value::Array(out))
        }
        serde_json::Value::Object(map) => {
            if map.len() > MAX_ITEMS_PER_LEVEL {
                return Err(TrustError::Encoding(format!(
                    "object exceeds max items per level ({} > {})",
                    map.len(),
                    MAX_ITEMS_PER_LEVEL
                )));
            }
            let mut entries: Vec<(Value, Value)> = Vec::with_capacity(map.len());
            for (k, v) in map {
                entries.push((Value::Text(k.clone()), json_to_canonical_cbor(v)?));
            }
            let sorted = sort_cbor_map_entries(entries)?;
            Ok(Value::Map(sorted))
        }
    }
}

/// Round-trip helper used in tests: serialize CBOR back to bytes.
#[allow(dead_code)]
pub(crate) fn cbor_value_to_bytes(v: &Value) -> TrustResult<Vec<u8>> {
    let mut buf = Vec::new();
    ciborium::ser::into_writer(v, &mut buf)
        .map_err(|e| TrustError::Encoding(format!("cbor serialize: {e}")))?;
    Ok(buf)
}

/// Round-trip helper used in tests: parse CBOR bytes back to a Value.
#[allow(dead_code)]
pub(crate) fn cbor_bytes_to_value(bytes: &[u8]) -> TrustResult<Value> {
    let cursor = Cursor::new(bytes);
    ciborium::de::from_reader(cursor)
        .map_err(|e| TrustError::Encoding(format!("cbor parse: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WS: &str = "ws-test";

    #[test]
    fn determinism_same_input_same_output() {
        let payload = serde_json::json!({"type": "node.create", "node": {"id": "n1", "name": "test"}});
        let a = build_signing_input(
            WS,
            "01H...",
            "2026-04-27T10:00:00Z",
            "spiffe://atlas/test",
            &["aabb".to_string(), "ccdd".to_string()],
            &payload,
            None,
        )
        .unwrap();
        let b = build_signing_input(
            WS,
            "01H...",
            "2026-04-27T10:00:00Z",
            "spiffe://atlas/test",
            &["aabb".to_string(), "ccdd".to_string()],
            &payload,
            None,
        )
        .unwrap();
        assert_eq!(a, b, "same input must produce identical bytes");
    }

    #[test]
    fn key_order_in_payload_does_not_matter() {
        // Two semantically identical payloads with different key orders must produce
        // identical signing-input bytes after canonicalisation.
        let payload_a = serde_json::json!({"a": 1, "b": 2, "c": 3});
        let payload_b = serde_json::json!({"c": 3, "b": 2, "a": 1});

        let a = build_signing_input(WS, "id", "ts", "kid", &[], &payload_a, None).unwrap();
        let b = build_signing_input(WS, "id", "ts", "kid", &[], &payload_b, None).unwrap();
        assert_eq!(a, b, "key order in payload must be canonicalised");
    }

    #[test]
    fn different_payload_gives_different_bytes() {
        let payload_a = serde_json::json!({"type": "node.create"});
        let payload_b = serde_json::json!({"type": "node.update"});
        let a = build_signing_input(WS, "id", "ts", "kid", &[], &payload_a, None).unwrap();
        let b = build_signing_input(WS, "id", "ts", "kid", &[], &payload_b, None).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn workspace_id_is_bound_into_signing_input() {
        // Cross-workspace replay defence: the same logical event signed for
        // workspace A must not produce identical bytes (and therefore identical
        // hash + verifying signature) for workspace B.
        let payload = serde_json::json!({"type": "node.create"});
        let a = build_signing_input("ws-A", "id", "ts", "kid", &[], &payload, None).unwrap();
        let b = build_signing_input("ws-B", "id", "ts", "kid", &[], &payload, None).unwrap();
        assert_ne!(
            a, b,
            "workspace_id must be bound into signing-input to prevent cross-workspace replay"
        );
    }

    #[test]
    fn floats_in_payload_are_rejected() {
        // Floats serialise non-deterministically across CBOR variants and
        // float libraries. Reject at the canonicaliser boundary.
        let payload = serde_json::json!({"score": 0.78});
        let result = build_signing_input(WS, "id", "ts", "kid", &[], &payload, None);
        assert!(result.is_err(), "floats must be rejected by canonicaliser");
    }

    #[test]
    fn rfc_8949_length_first_map_sort() {
        // Keys of mixed length: pure lex sort would put "long_key" before "z";
        // RFC 8949 §4.2.1 puts "z" first because its encoded form is shorter.
        // Two payloads with the same logical content but different key order
        // must still canonicalise to identical bytes.
        let p1 = serde_json::json!({"long_key": 1, "z": 2});
        let p2 = serde_json::json!({"z": 2, "long_key": 1});
        let a = build_signing_input(WS, "id", "ts", "kid", &[], &p1, None).unwrap();
        let b = build_signing_input(WS, "id", "ts", "kid", &[], &p2, None).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn author_did_present_changes_bytes() {
        // V2-α cross-agent-replay defence: same logical event with different
        // author_did values must produce different bytes (and therefore
        // different hash + verifying signature). Symmetric to
        // `workspace_id_is_bound_into_signing_input`.
        // Both DIDs are well-formed (exactly 64 lowercase-hex chars each).
        let payload = serde_json::json!({"type": "node.create"});
        let did_a = "did:atlas:a44f44b3d6c9d4e2c84b6d40f6e0e0e8d8e3f2c1a0f9e8d7c6b5a4938271605f";
        let did_b = "did:atlas:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        // Sanity: both DIDs exactly the canonical shape (64-char hex suffix).
        assert_eq!(did_a.len(), "did:atlas:".len() + 64);
        assert_eq!(did_b.len(), "did:atlas:".len() + 64);
        let a = build_signing_input(WS, "id", "ts", "kid", &[], &payload, Some(did_a)).unwrap();
        let b = build_signing_input(WS, "id", "ts", "kid", &[], &payload, Some(did_b)).unwrap();
        assert_ne!(
            a, b,
            "author_did must be bound into signing-input to prevent cross-agent replay"
        );
    }

    #[test]
    fn author_did_none_byte_identical_to_v1() {
        // V1 backward-compat invariant: when author_did is None, the CBOR
        // output is exactly what V1 produced. This is what keeps the V1
        // `signing_input_byte_determinism_pin` test passing unchanged after
        // the Welle-1 signature extension.
        let payload = serde_json::json!({"type": "node.create"});
        let without = build_signing_input(WS, "id", "ts", "kid", &[], &payload, None).unwrap();
        // Re-serialise the same logical event with author_did = None to
        // demonstrate the second call is byte-identical.
        let without_again =
            build_signing_input(WS, "id", "ts", "kid", &[], &payload, None).unwrap();
        assert_eq!(without, without_again);

        // And confirm Some(...) produces strictly different bytes.
        let did = "did:atlas:a44f44b3d6c9d4e2c84b6d40f6e0e0e8d8e3f2c1a0f9e8d7c6b5a4938271605f";
        let with = build_signing_input(WS, "id", "ts", "kid", &[], &payload, Some(did)).unwrap();
        assert_ne!(without, with, "Some(author_did) must produce different bytes than None");
    }

    /// Cross-implementation determinism golden.
    ///
    /// Pins the exact byte-for-byte output of `build_signing_input` for one
    /// fixed input. Any unintentional change to the canonicalisation pipeline
    /// (CBOR sort order, key encoding, struct shape, ciborium upgrade that
    /// changes encoding) trips this test BEFORE the WASM/native split can
    /// reach a customer's browser.
    ///
    /// If you regenerate the pinned hex below, you have changed verifier
    /// semantics — bump the `atlas-trust-core` crate version, which cascades
    /// into `VERIFIER_VERSION`, so old-format traces are rejected with a
    /// clean schema-mismatch error rather than silently misverifying.
    #[test]
    fn signing_input_byte_determinism_pin() {
        let payload = serde_json::json!({"type": "node.create"});
        let actual = build_signing_input(
            "ws-test",
            "01H001",
            "2026-04-27T10:00:00Z",
            "spiffe://atlas/test",
            &[],
            &payload,
            None, // V1-shaped: no author_did
        )
        .unwrap();
        let actual_hex = hex::encode(&actual);

        // BEGIN PINNED — DO NOT EDIT WITHOUT INTENT.
        // Decodes (RFC 8949 §4.2.1 length-then-lex map order) to:
        //   { "v": "atlas-trace-v1",
        //     "id": "01H001",
        //     "ts": "2026-04-27T10:00:00Z",
        //     "kid": "spiffe://atlas/test",
        //     "parents": [],
        //     "payload": { "type": "node.create" },
        //     "workspace": "ws-test" }
        //
        // V2-α Welle 1: this V1 pin is preserved unchanged. When
        // author_did = None, the CBOR field is omitted from the map
        // (build_signing_input gates the entry on Some). The V1
        // byte-determinism invariant therefore holds across the V2-α
        // signature extension.
        let expected_hex =
            "a761766e61746c61732d74726163652d76316269646630314830303162747374323032362d30342d3237\
             5431303a30303a30305a636b6964737370696666653a2f2f61746c61732f7465737467706172656e7473\
             80677061796c6f6164a164747970656b6e6f64652e63726561746569776f726b73706163656777732d74\
             657374";
        // END PINNED.

        // Strip whitespace so the pinned hex can be wrapped across lines for readability.
        let expected_hex: String = expected_hex.chars().filter(|c| !c.is_whitespace()).collect();

        assert_eq!(
            actual_hex, expected_hex,
            "signing-input wire-format drift. If intentional, update the \
             pinned hex AND bump atlas-trust-core's crate version so the \
             VERIFIER_VERSION cascade propagates to old-format trace bundles."
        );
    }

    /// V2-α Welle 1 byte-determinism pin: same fixture as the V1 pin but
    /// with `author_did = Some(did:atlas:<32-hex>)`. Pins the exact
    /// CBOR byte output so any unintentional change to the V2-α
    /// signature-input shape trips this test before the WASM/native
    /// split can reach a customer's browser.
    ///
    /// The pinned hex is sensitive to (a) the choice of CBOR field name
    /// (`"author_did"` — 10 chars + header → encoded-length 11, longer
    /// than any other key, so this key sorts LAST per RFC 8949 §4.2.1)
    /// and (b) the exact DID string used. Changing either is a wire-
    /// format break: bump atlas-trust-core's crate version and update
    /// the pin together.
    ///
    /// Co-equal status with `signing_input_byte_determinism_pin`: V1 and
    /// V2-α-with-author_did are two distinct wire shapes, and the
    /// verifier handles both because the encoder gates the CBOR field
    /// on `author_did.is_some()`.
    #[test]
    fn signing_input_byte_determinism_pin_with_author_did() {
        let payload = serde_json::json!({"type": "node.create"});
        // Same 64-hex fixture used in agent_did module tests.
        let did = "did:atlas:a44f44b3d6c9d4e2c84b6d40f6e0e0e8d8e3f2c1a0f9e8d7c6b5a4938271605f";
        let actual = build_signing_input(
            "ws-test",
            "01H001",
            "2026-04-27T10:00:00Z",
            "spiffe://atlas/test",
            &[],
            &payload,
            Some(did),
        )
        .unwrap();
        let actual_hex = hex::encode(&actual);

        // BEGIN PINNED — DO NOT EDIT WITHOUT INTENT.
        // Map shape (RFC 8949 §4.2.1 length-then-lex order):
        //   { "v":          "atlas-trace-v1",   (encoded key length 2)
        //     "id":         "01H001",           (encoded key length 3)
        //     "ts":         "2026-04-27T10:00:00Z", (encoded key length 3)
        //     "kid":        "spiffe://atlas/test",  (encoded key length 4)
        //     "parents":    [],                 (encoded key length 8)
        //     "payload":    { "type": "node.create" }, (encoded key length 8)
        //     "workspace":  "ws-test",          (encoded key length 10)
        //     "author_did": "did:atlas:a44f...605f"  (encoded key length 11) }
        //
        // Map header for 8 pairs: a8. Then per RFC 8949 §4.2.1, keys are
        // ordered by encoded-length first (shortest first), then lex on
        // encoded bytes. "author_did" has encoded length 11, longer than
        // every other key, so it appears LAST in the wire bytes. The
        // first 7 entries are byte-identical to the V1 pin (which
        // confirms the "None skips field" property).
        //
        // The author_did entry comprises: key "author_did" (text(10) =
        // 0x6a + 10 bytes), value text(74) (= 0x78 0x4a + 74 bytes:
        // "did:atlas:" = 10 bytes + 64-hex-char suffix = 64 bytes).
        let expected_hex =
            "a861766e61746c61732d74726163652d76316269646630314830303162747374323032362d30342d3237\
             5431303a30303a30305a636b6964737370696666653a2f2f61746c61732f7465737467706172656e7473\
             80677061796c6f6164a164747970656b6e6f64652e63726561746569776f726b73706163656777732d74\
             6573746a617574686f725f646964784a6469643a61746c61733a61343466343462336436633964346532\
             633834623664343066366530653065386438653366326331613066396538643763366235613439333832\
             373136303566";
        // END PINNED.

        let expected_hex: String = expected_hex.chars().filter(|c| !c.is_whitespace()).collect();

        assert_eq!(
            actual_hex, expected_hex,
            "V2-α signing-input wire-format drift. If intentional, update the \
             pinned hex AND bump atlas-trust-core's crate version so the \
             VERIFIER_VERSION cascade propagates to old-format trace bundles."
        );

        // Defence-in-depth: rebuild via the same code path and assert
        // byte-equality. This catches any non-deterministic behaviour
        // within a single run.
        let reference = build_signing_input(
            "ws-test",
            "01H001",
            "2026-04-27T10:00:00Z",
            "spiffe://atlas/test",
            &[],
            &payload,
            Some(did),
        )
        .unwrap();
        assert_eq!(
            actual_hex,
            hex::encode(&reference),
            "V2-α signing-input is non-deterministic across calls — bug in canonicaliser"
        );

        // Sanity: this byte output differs from the V1 (author_did=None) shape.
        let v1_shape = build_signing_input(
            "ws-test",
            "01H001",
            "2026-04-27T10:00:00Z",
            "spiffe://atlas/test",
            &[],
            &payload,
            None,
        )
        .unwrap();
        assert_ne!(
            actual, v1_shape,
            "V2-α signing input must differ from V1 shape when author_did is Some"
        );

        // Sanity: V1 shape and V2-α shape share their first 7 entries
        // (bytes 1..N where N is the V1 pin's length, minus the leading
        // a7-vs-a8 map-header byte). This documents the "additive"
        // property explicitly.
        assert_eq!(v1_shape[0], 0xa7, "V1 shape must start with a7 map-header");
        assert_eq!(actual[0], 0xa8, "V2-α shape must start with a8 map-header");
        assert_eq!(
            &v1_shape[1..],
            &actual[1..v1_shape.len()],
            "V1 entries must be byte-prefix-identical to V2-α entries (only map-header byte and trailing entry differ)"
        );
    }
}
