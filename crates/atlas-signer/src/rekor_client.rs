//! Minimal Rekor REST client used by the V1.6 Sigstore issuer.
//!
//! Scope is intentionally narrow: ONE method, `submit_hashedrekord`,
//! which POSTs a `hashedrekord` v0.0.1 entry to `/api/v1/log/entries`
//! and parses the response into typed structs. No retry, no async, no
//! pagination, no get-by-uuid — those land in V1.7+ if and when needed.
//!
//! Why blocking and not async? The signer is a short-lived CLI invoked
//! from a parent (the MCP server) that already manages async itself.
//! Spawning a tokio runtime per CLI invocation would add hundreds of
//! milliseconds of startup latency and a tokio dependency tree that
//! the verifier has zero need for. `reqwest::blocking` keeps the
//! signer's runtime exposure to the network call only.
//!
//! Why no retry? Two reasons. (1) Rekor's idempotency story is
//! "submit the same body twice ⇒ get a 409 with the existing entry's
//! UUID" — handling that correctly requires distinguishing
//! transient-server-error retries from already-anchored-just-fetch
//! retries, which is non-trivial and out of Phase 2 scope. (2) A
//! silent retry across an Atlas-anchoring-key change would issue two
//! distinct Rekor entries for the same Atlas hash, breaking the
//! "every anchor is unique" property. Until V1.7 we fail loud.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// hashedrekord/v0.0.1 request body, exactly as Rekor's API expects.
///
/// Layout pinned by the schema at
/// <https://github.com/sigstore/rekor/blob/main/pkg/types/hashedrekord/v0.0.1>.
/// The verifier in `atlas-trust-core` re-derives `value` (via
/// `sigstore_anchored_hash_for`) and asserts it matches the body Rekor
/// returns, so any field-name drift here breaks the trust property
/// loudly at verify time.
#[derive(Debug, Serialize)]
pub struct HashedRekordRequest {
    #[serde(rename = "apiVersion")]
    pub api_version: &'static str,
    pub kind: &'static str,
    pub spec: HashedRekordSpec,
}

#[derive(Debug, Serialize)]
pub struct HashedRekordSpec {
    pub data: HashedRekordData,
    pub signature: HashedRekordSignature,
}

#[derive(Debug, Serialize)]
pub struct HashedRekordData {
    pub hash: HashedRekordHash,
}

#[derive(Debug, Serialize)]
pub struct HashedRekordHash {
    /// Pinned to "sha256" — the verifier rejects any other algorithm.
    pub algorithm: &'static str,
    /// Hex-encoded SHA-256 of the artifact bytes.
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct HashedRekordSignature {
    /// Standard base64 of the raw signature bytes (DER for ECDSA, raw
    /// 64-byte form for Ed25519). Rekor verifies the signature against
    /// `data.hash.value` at submit time using `signature.publicKey.content`.
    pub content: String,
    #[serde(rename = "publicKey")]
    pub public_key: HashedRekordPublicKey,
}

#[derive(Debug, Serialize)]
pub struct HashedRekordPublicKey {
    /// Standard base64 of the PEM-encoded SubjectPublicKeyInfo.
    pub content: String,
}

/// A single Rekor entry from the response payload. Field names map to
/// Rekor's JSON shape; serde renames keep Rust idiomatic on this side.
#[derive(Debug, Deserialize)]
pub struct RekorEntry {
    /// Standard base64 of the canonical entry body (JSON), same bytes
    /// the leaf-hash is computed over.
    pub body: String,
    #[serde(rename = "integratedTime")]
    pub integrated_time: i64,
    #[serde(rename = "logID")]
    pub log_id: String,
    #[serde(rename = "logIndex")]
    pub log_index: u64,
    pub verification: RekorVerification,
}

#[derive(Debug, Deserialize)]
pub struct RekorVerification {
    #[serde(rename = "inclusionProof")]
    pub inclusion_proof: RekorInclusionProof,
}

#[derive(Debug, Deserialize)]
pub struct RekorInclusionProof {
    /// Full multi-line C2SP signed-note text. The issuer extracts the
    /// signature line via `extract_signature_line_sigstore`.
    pub checkpoint: String,
    /// Hex-encoded sibling hashes along the audit path.
    pub hashes: Vec<String>,
    #[serde(rename = "rootHash")]
    pub root_hash: String,
    #[serde(rename = "treeSize")]
    pub tree_size: u64,
}

/// Blocking Rekor REST client.
///
/// Construct once per CLI invocation; reuse the inner `reqwest::blocking::Client`
/// across all entries in a batch. The TLS stack handles connection reuse
/// internally so a multi-item batch does not pay the handshake cost twice.
#[derive(Debug)]
pub struct RekorClient {
    base_url: String,
    http: reqwest::blocking::Client,
}

impl RekorClient {
    /// Build a new client.
    ///
    /// `base_url` is the Rekor instance root, e.g. `https://rekor.sigstore.dev`.
    /// The client appends `/api/v1/log/entries` for submissions, so the
    /// caller must NOT include the path themselves.
    pub fn new(base_url: &str) -> Result<Self, String> {
        let trimmed = base_url.trim_end_matches('/').to_string();
        if trimmed.is_empty() {
            return Err("rekor base url must not be empty".to_string());
        }
        // Plaintext http:// is permitted ONLY for loopback addresses
        // (localhost / 127.0.0.1 / [::1]), which is what the wiremock
        // integration tests need. Any other host must use https:// so
        // an operator who typo-pastes (or is socially-engineered into
        // pasting) a plaintext URL doesn't end up submitting Atlas
        // anchoring signatures over an unencrypted wire.
        if let Some(rest) = trimmed.strip_prefix("http://") {
            // Strip the path segment, then the optional :port, then check
            // the host literal. Loopback IPv6 has the form `http://[::1]…`.
            let host = rest.split('/').next().unwrap_or(rest);
            let host_only = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
            let is_loopback = host_only == "localhost"
                || host_only == "127.0.0.1"
                || host_only == "[::1]"
                || host_only == "::1";
            if !is_loopback {
                return Err(format!(
                    "rekor base url uses plaintext http:// for non-loopback host {host_only:?}; \
                     plaintext is only allowed for localhost/127.0.0.1/[::1]. Use https:// instead."
                ));
            }
        } else if !trimmed.starts_with("https://") {
            return Err(format!(
                "rekor base url must start with https:// (or http:// for localhost only); got {trimmed:?}"
            ));
        }
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("could not build reqwest blocking client: {e}"))?;
        Ok(Self { base_url: trimmed, http })
    }

    /// Submit a `hashedrekord` entry. Returns the single resulting entry
    /// from Rekor's response payload.
    ///
    /// Behaviour:
    /// - Non-2xx HTTP status ⇒ Err with status + body excerpt.
    /// - Response body that is not a JSON map ⇒ Err.
    /// - Map with zero or more than one entry ⇒ Err. Rekor returns
    ///   exactly one entry per submission; deviating from that is a
    ///   protocol violation we should not paper over.
    pub fn submit_hashedrekord(
        &self,
        request: &HashedRekordRequest,
    ) -> Result<RekorEntry, String> {
        let url = format!("{}/api/v1/log/entries", self.base_url);
        let response = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(request)
            .send()
            .map_err(|e| format!("rekor POST {url} failed: {e}"))?;

        let status = response.status();
        if !status.is_success() {
            // Pull a bounded slice of the body for the error message —
            // Rekor's error JSON is small but we cap to be safe.
            let body = response
                .text()
                .unwrap_or_else(|_| "<no body>".to_string());
            let excerpt: String = body.chars().take(512).collect();
            return Err(format!("rekor returned HTTP {status}: {excerpt}"));
        }

        // Rekor returns: { "<uuid>": { ...entry... } }. BTreeMap because
        // the order of UUIDs is irrelevant and we accept exactly one.
        let entries: BTreeMap<String, RekorEntry> = response
            .json()
            .map_err(|e| format!("rekor response is not a valid entries map: {e}"))?;

        match entries.len() {
            0 => Err("rekor response contains zero entries".to_string()),
            1 => {
                // Pop the single entry; key (UUID) is unused at this layer.
                // We avoid `.expect` here so that even under a future
                // change to the match-arm above, a logic mistake produces
                // a recoverable error rather than a process panic.
                let (_uuid, entry) = entries.into_iter().next().ok_or_else(|| {
                    "internal: rekor response had len==1 but iterator yielded none".to_string()
                })?;
                Ok(entry)
            }
            n => Err(format!(
                "rekor response contains {n} entries; expected exactly one"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_empty_url() {
        let err = RekorClient::new("").expect_err("empty url must be rejected");
        assert!(err.contains("must not be empty"), "got: {err}");
    }

    #[test]
    fn new_rejects_url_without_scheme() {
        let err = RekorClient::new("rekor.sigstore.dev")
            .expect_err("url without scheme must be rejected");
        assert!(err.contains("https://"), "got: {err}");
    }

    #[test]
    fn new_strips_trailing_slash() {
        let c = RekorClient::new("https://rekor.sigstore.dev/").unwrap();
        assert_eq!(c.base_url, "https://rekor.sigstore.dev");
    }

    /// http:// is permitted only for loopback addresses (test
    /// fixtures); any production host must use https://.
    #[test]
    fn new_rejects_http_for_non_loopback() {
        let err = RekorClient::new("http://rekor.sigstore.dev")
            .expect_err("plaintext http:// for production host must be rejected");
        assert!(
            err.contains("plaintext") && err.contains("https"),
            "error must explain why: {err}",
        );
    }

    /// Loopback http:// is the wiremock test path. Must succeed even
    /// without a port (defaults), with a port, with /paths, and for
    /// 127.0.0.1 / [::1] equivalents.
    #[test]
    fn new_accepts_http_for_loopback() {
        for url in [
            "http://localhost",
            "http://localhost:1234",
            "http://localhost:1234/api",
            "http://127.0.0.1",
            "http://127.0.0.1:8080",
            "http://[::1]:9000",
        ] {
            let r = RekorClient::new(url);
            assert!(r.is_ok(), "loopback url {url} must be accepted, got: {r:?}");
        }
    }

    /// Pin the JSON serialization of a hashedrekord request so a future
    /// edit cannot silently rename `apiVersion` ⇒ `api_version` (which
    /// Rekor would reject) without tripping this test.
    #[test]
    fn hashedrekord_request_serializes_with_camelcase_keys() {
        let req = HashedRekordRequest {
            api_version: "0.0.1",
            kind: "hashedrekord",
            spec: HashedRekordSpec {
                data: HashedRekordData {
                    hash: HashedRekordHash {
                        algorithm: "sha256",
                        value: "deadbeef".to_string(),
                    },
                },
                signature: HashedRekordSignature {
                    content: "c2lnLWI2NA==".to_string(),
                    public_key: HashedRekordPublicKey {
                        content: "cGsuYjY0".to_string(),
                    },
                },
            },
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"apiVersion\":\"0.0.1\""), "got: {s}");
        assert!(s.contains("\"kind\":\"hashedrekord\""), "got: {s}");
        assert!(s.contains("\"algorithm\":\"sha256\""), "got: {s}");
        assert!(s.contains("\"value\":\"deadbeef\""), "got: {s}");
        assert!(s.contains("\"publicKey\""), "got: {s}");
    }
}
