//! Pinned pubkey-bundle.
//!
//! At build time the verifier embeds (or is given) a pubkey-bundle.json
//! which maps `kid` (key-id, e.g. SPIFFE-ID or email) → Ed25519 pubkey.
//!
//! Trace claims `pubkey_bundle_hash` of the bundle the trace was signed against.
//! We refuse traces whose claimed bundle hash doesn't match ours.
//! That is what makes server-side rotation trustworthy: customers re-fetch
//! the cosigned bundle on rotation, and old traces are still verifiable
//! against historic bundles by version.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{TrustError, TrustResult};

/// Map of key-id → Ed25519 pubkey (32 bytes, base64url-no-pad).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PubkeyBundle {
    /// Bundle schema version
    pub schema: String,
    /// ISO-8601 timestamp the bundle was assembled
    pub generated_at: String,
    /// Map of kid → 32-byte Ed25519 pubkey, base64url-no-pad encoded.
    pub keys: HashMap<String, String>,
}

impl PubkeyBundle {
    /// Parse a bundle from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> TrustResult<Self> {
        serde_json::from_slice(bytes).map_err(TrustError::from)
    }

    /// Return the 32-byte pubkey for a kid, or `UnknownKid` error.
    pub fn pubkey_for(&self, kid: &str) -> TrustResult<Vec<u8>> {
        let b64 = self
            .keys
            .get(kid)
            .ok_or_else(|| TrustError::UnknownKid(kid.to_string()))?;
        decode_b64url(b64).map_err(|e| TrustError::Encoding(format!("pubkey decode: {e}")))
    }

    /// Compute the deterministic hash of this bundle.
    /// Hash format: blake3 of canonical JSON (keys sorted, no whitespace).
    pub fn deterministic_hash(&self) -> TrustResult<String> {
        // Use a sorted serialisation
        let mut sorted: Vec<(&String, &String)> = self.keys.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        let canonical = serde_json::json!({
            "schema": self.schema,
            "generated_at": self.generated_at,
            "keys": sorted.iter().map(|(k, v)| (*k, *v)).collect::<HashMap<&String, &String>>(),
        });
        // We need true canonical JSON; serde_json::to_vec may not sort keys.
        let bytes = canonical_json_bytes(&canonical)?;
        let hash = blake3::hash(&bytes);
        Ok(hex::encode(hash.as_bytes()))
    }
}

fn decode_b64url(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.decode(s)
}

/// Produce canonical JSON bytes (keys sorted, no whitespace).
///
/// `pub(crate)` so the chain-head canonicalization in `anchor.rs` shares
/// exactly one implementation with `PubkeyBundle::deterministic_hash` —
/// any future canonicalization change (whitespace, key-sort, number
/// formatting) cascades to every consumer at once.
pub(crate) fn canonical_json_bytes(v: &serde_json::Value) -> TrustResult<Vec<u8>> {
    fn write_canonical(v: &serde_json::Value, out: &mut Vec<u8>) -> TrustResult<()> {
        match v {
            serde_json::Value::Null => out.extend_from_slice(b"null"),
            serde_json::Value::Bool(true) => out.extend_from_slice(b"true"),
            serde_json::Value::Bool(false) => out.extend_from_slice(b"false"),
            serde_json::Value::Number(n) => {
                out.extend_from_slice(n.to_string().as_bytes());
            }
            serde_json::Value::String(s) => {
                out.extend_from_slice(serde_json::to_string(s)
                    .map_err(|e| TrustError::Encoding(e.to_string()))?
                    .as_bytes());
            }
            serde_json::Value::Array(arr) => {
                out.push(b'[');
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    write_canonical(item, out)?;
                }
                out.push(b']');
            }
            serde_json::Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                out.push(b'{');
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    out.extend_from_slice(serde_json::to_string(k)
                        .map_err(|e| TrustError::Encoding(e.to_string()))?
                        .as_bytes());
                    out.push(b':');
                    write_canonical(&map[*k], out)?;
                }
                out.push(b'}');
            }
        }
        Ok(())
    }

    let mut out = Vec::new();
    write_canonical(v, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bundle() {
        let json = br#"{
            "schema": "atlas-pubkey-bundle-v1",
            "generated_at": "2026-04-27T10:00:00Z",
            "keys": {
                "spiffe://atlas/agent/cursor-001": "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"
            }
        }"#;
        let bundle = PubkeyBundle::from_json(json).unwrap();
        assert_eq!(bundle.keys.len(), 1);
    }

    #[test]
    fn deterministic_hash_is_deterministic() {
        let mut keys = HashMap::new();
        keys.insert("a".to_string(), "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8".to_string());
        keys.insert("b".to_string(), "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8".to_string());
        let bundle = PubkeyBundle {
            schema: "v1".to_string(),
            generated_at: "2026-04-27T10:00:00Z".to_string(),
            keys,
        };
        let h1 = bundle.deterministic_hash().unwrap();
        let h2 = bundle.deterministic_hash().unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // hex-blake3
    }

    #[test]
    fn key_insertion_order_does_not_affect_hash() {
        // Two bundles with the same keys inserted in opposite orders MUST
        // produce identical hashes. This is the property that lets two
        // independent signers re-derive the same bundle hash without
        // co-ordinating on insertion order.
        let mut keys_a = HashMap::new();
        keys_a.insert("kid-zeta".to_string(), "PUBKEY_Z".to_string());
        keys_a.insert("kid-alpha".to_string(), "PUBKEY_A".to_string());
        keys_a.insert("kid-mu".to_string(), "PUBKEY_M".to_string());

        let mut keys_b = HashMap::new();
        keys_b.insert("kid-mu".to_string(), "PUBKEY_M".to_string());
        keys_b.insert("kid-alpha".to_string(), "PUBKEY_A".to_string());
        keys_b.insert("kid-zeta".to_string(), "PUBKEY_Z".to_string());

        let bundle_a = PubkeyBundle {
            schema: "atlas-pubkey-bundle-v1".to_string(),
            generated_at: "2026-04-27T10:00:00Z".to_string(),
            keys: keys_a,
        };
        let bundle_b = PubkeyBundle {
            schema: "atlas-pubkey-bundle-v1".to_string(),
            generated_at: "2026-04-27T10:00:00Z".to_string(),
            keys: keys_b,
        };

        assert_eq!(
            bundle_a.deterministic_hash().unwrap(),
            bundle_b.deterministic_hash().unwrap(),
            "key insertion order MUST NOT affect the bundle hash"
        );
    }

    /// Cross-implementation determinism golden for `deterministic_hash`.
    ///
    /// Pins the exact blake3 hex of a fixed `PubkeyBundle`. This is the
    /// second load-bearing hash in the Atlas trust model (the first being
    /// `signing_input_byte_determinism_pin` in `cose.rs`).
    ///
    /// If `canonical_json_bytes` is ever changed — a `serde_json` upgrade
    /// that alters `Number::to_string()` output, a whitespace handling tweak,
    /// a key-sort regression — this test trips before old bundles silently
    /// stop verifying against new builds.
    ///
    /// If you regenerate the pinned hex below, you have changed the bundle
    /// hash format. Bump `atlas-trust-core`'s crate version so
    /// `VERIFIER_VERSION` cascades, and surface the format change in
    /// `docs/SECURITY-NOTES.md` so auditors see it.
    #[test]
    fn bundle_hash_byte_determinism_pin() {
        let mut keys = HashMap::new();
        keys.insert(
            "spiffe://atlas/agent/cursor-001".to_string(),
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8".to_string(),
        );
        keys.insert(
            "spiffe://atlas/human/sebastian.meinhardt@bankhaus-hagedorn.de".to_string(),
            "QkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQg".to_string(),
        );
        let bundle = PubkeyBundle {
            schema: "atlas-pubkey-bundle-v1".to_string(),
            generated_at: "2026-01-01T00:00:00Z".to_string(),
            keys,
        };

        let actual = bundle.deterministic_hash().unwrap();

        // BEGIN PINNED — DO NOT EDIT WITHOUT INTENT.
        // blake3 of the canonical-JSON serialisation:
        //   {"generated_at":"2026-01-01T00:00:00Z",
        //    "keys":{"spiffe://atlas/agent/cursor-001":"AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8",
        //            "spiffe://atlas/human/sebastian.meinhardt@bankhaus-hagedorn.de":"QkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQg"},
        //    "schema":"atlas-pubkey-bundle-v1"}
        // (top-level keys sorted lex; nested keys sorted lex; no whitespace.)
        let expected = "f87e9a30a06963f8c13b30bcf1085f1d65afa149b8f5015f7af5c79107d3fe0c";
        // END PINNED.

        assert_eq!(
            actual, expected,
            "bundle-hash wire-format drift. If intentional, update the \
             pinned hex AND bump atlas-trust-core's crate version so the \
             VERIFIER_VERSION cascade propagates to old-format bundles."
        );
    }
}
