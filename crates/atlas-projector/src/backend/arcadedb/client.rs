//! V2-β Welle 17b: HTTP client + auth helpers for the ArcadeDB driver.
//!
//! Wraps `reqwest::blocking::Client` with:
//!
//! - [`BasicAuth`] — owned `(username, password)` pair. Password lives
//!   behind a small wrapper that overrides `Debug` to print
//!   `"<redacted>"` instead of the raw bytes, so accidentally logging a
//!   `BasicAuth` value or an `ArcadeDbBackend` (which carries one) does
//!   NOT leak credentials. ADR-Atlas-010 §4 sub-decision #7 (defence
//!   in depth) — credentials never appear in logs or error strings.
//!
//! - [`build_http_client`] — constructs the `reqwest::blocking::Client`
//!   with the per-`ArcadeDbBackend` policy: 5 s connect timeout, 30 s
//!   total request timeout, `rustls-tls` (no openssl-sys), a stable
//!   `User-Agent` identifying the crate version. The constructor is a
//!   pure function so unit tests can call it without touching network.
//!
//! - [`map_response_error`] — single error-mapping boundary. Every
//!   non-2xx HTTP response or transport-level `reqwest::Error` is
//!   translated to a `ProjectorError::CanonicalisationFailed` with a
//!   credentials-redacted message. Status codes 401/403 surface as a
//!   stable "authentication failed" string so operators can spot
//!   auth-credential rotation issues; 5xx surfaces as "upstream error
//!   <status>".

use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response};

use crate::error::{ProjectorError, ProjectorResult};

/// HTTP Basic authentication credentials for the ArcadeDB Server-mode
/// HTTP API. Per-database credentials per ADR-Atlas-010 §4
/// sub-decision #7 + ADR-011 §6 OQ-11 (V2-β starts with Basic; V2-γ
/// MAY want JWT bearer).
///
/// The `password` field is wrapped in [`SecretString`] so the auto-
/// derived `Debug` for `BasicAuth` redacts it. The redaction is the
/// only line of defence for accidental log-of-struct paths; explicit
/// error paths in this crate ALSO never embed the password in their
/// error strings (the contract is enforced at [`map_response_error`]).
#[derive(Clone)]
pub struct BasicAuth {
    /// ArcadeDB username (e.g. `"root"` per ADR-Atlas-010 §4
    /// sub-decision #7 + the docker-compose §4.7 sketch).
    pub username: String,
    /// ArcadeDB password. Stored in a redacting wrapper so accidental
    /// `Debug` / `Display` of the surrounding struct does not leak the
    /// secret to logs.
    pub password: SecretString,
}

impl BasicAuth {
    /// Construct a new [`BasicAuth`] from raw owned strings.
    ///
    /// The password is moved into the [`SecretString`] wrapper; the
    /// raw bytes are NOT retained outside the wrapper. Callers should
    /// pull the password from a sealed source (env-var read at startup,
    /// HSM-fetched, K8s secret-mount) and pass it here once.
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: SecretString::new(password.into()),
        }
    }
}

/// Hand-rolled `Debug` redacts the password byte string. The
/// `username` is preserved because it is operationally useful for
/// diagnostics (e.g. "auth failed for user X") and is not sensitive
/// in the ArcadeDB threat model (the password is the secret, not
/// the user identity).
impl std::fmt::Debug for BasicAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BasicAuth")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Wrapper that redacts the inner string on `Debug`. The `expose` method
/// returns a `&str` for the narrow consumer that needs the bytes (the
/// `reqwest::RequestBuilder::basic_auth` call site in
/// [`apply_basic_auth`] below). No other path inside this crate is
/// allowed to call `.expose()`.
///
/// We deliberately avoid a heavyweight `secrecy` crate dependency for
/// W17b — the redaction need is narrow (one struct, one accessor) and
/// adding a transitive dep tree for a 30-LOC wrapper is not justified.
/// V2-γ MAY revisit if more secrets enter the projector boundary
/// (HSM-backed signing keys, JWT bearer tokens, …).
#[derive(Clone)]
pub struct SecretString {
    inner: String,
}

impl SecretString {
    /// Construct a new redacting wrapper. Callers MUST own the input
    /// string (the wrapper takes ownership).
    #[must_use]
    pub fn new(s: String) -> Self {
        Self { inner: s }
    }

    /// Expose the inner bytes for the narrow consumer that needs them.
    /// Only `apply_basic_auth` calls this inside the crate; outside
    /// callers MAY call it but the redaction-discipline expectation is
    /// "treat the return value as if it were the plaintext password".
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.inner
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Construct the `reqwest::blocking::Client` used by every
/// `ArcadeDbBackend` instance.
///
/// Timeouts:
/// - `connect_timeout(5s)` — TCP + TLS handshake budget. Per spike §4.10
///   the localhost-ArcadeDB latency baseline is ~300-500 µs per query;
///   a 5 s connect budget is 4 orders of magnitude over baseline and
///   bounds the "network partition" failure path.
/// - `timeout(30s)` — total request budget. ArcadeDB's heaviest
///   query path (full re-projection of 10M events, spike §4.10) is
///   ~6-10 min in aggregate but per-call latency is ≤ 1 sec at the
///   trait's batch granularity; 30 s is a comfortable upper bound that
///   still triggers on a deadlock or runaway query.
///
/// The `User-Agent` is `atlas-projector/<CRATE_VERSION>` so server-side
/// logs identify the Atlas component making the call (useful for
/// operator triage in a deployment that has multiple Atlas crate
/// versions in-flight during a rolling upgrade).
///
/// Pure function — does NOT touch the network. Unit tests can call it
/// without a live ArcadeDB instance.
pub(crate) fn build_http_client() -> ProjectorResult<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .user_agent(format!("atlas-projector/{}", crate::CRATE_VERSION))
        // No `default_headers` here — the `arcadedb-session-id` header
        // is per-transaction and is applied at the `RequestBuilder`
        // level in [`apply_session_header`].
        .build()
        .map_err(|e| {
            // `reqwest::ClientBuilder::build` errors only on
            // mutually-incompatible feature flags or TLS-backend
            // misconfiguration (e.g. missing `rustls-tls` in features).
            // We surface as a CanonicalisationFailed so the projector
            // pipeline can degrade gracefully.
            ProjectorError::CanonicalisationFailed(format!(
                "ArcadeDB HTTP client construction failed: {kind}",
                kind = describe_reqwest_error(&e)
            ))
        })
}

/// Attach HTTP Basic Authentication to a `reqwest::RequestBuilder`.
///
/// This is the SINGLE call site that calls `SecretString::expose()`
/// inside the crate. `reqwest::RequestBuilder::basic_auth` takes the
/// password as `Option<&str>`; we extract here, hand to `reqwest`, and
/// let the request consume it. `reqwest` does NOT log the password —
/// it base64-encodes it into the `Authorization` header — but if
/// `reqwest`'s policy ever changes, this call site is the chokepoint
/// to audit.
pub(crate) fn apply_basic_auth(req: RequestBuilder, auth: &BasicAuth) -> RequestBuilder {
    req.basic_auth(&auth.username, Some(auth.password.expose()))
}

/// Attach the `arcadedb-session-id` header (per ArcadeDB spike §3 HTTP
/// API) to a `RequestBuilder`. Caller supplies the session id obtained
/// from the response header on `/api/v1/begin/{db}`.
///
/// `reqwest`'s `header()` builder validates header values: bytes
/// outside `\x20..=\x7E` (printable ASCII) plus tab are rejected. The
/// session id is server-issued so this validation is belt-and-braces;
/// if ArcadeDB ever returned a malformed header it would fail here
/// with a `reqwest::Error::Builder`, which we map to a redacted error
/// at the call site.
pub(crate) fn apply_session_header(req: RequestBuilder, session_id: &str) -> RequestBuilder {
    req.header("arcadedb-session-id", session_id)
}

/// Map a `reqwest::Response` into a `ProjectorResult<Response>`.
///
/// Pre-conditions: the caller has just `send()`-ed a request and got
/// a successful (`Ok(Response)`) handshake.
///
/// Behaviour:
/// - 2xx → `Ok(response)` (untouched; caller will read the body).
/// - 401 / 403 → `Err(CanonicalisationFailed("ArcadeDB authentication
///   failed (credentials redacted)"))`. The error string is fixed —
///   it does NOT include the server's response body (which COULD echo
///   back the username or transient session-id) and it does NOT
///   include the credentials.
/// - Other non-2xx → `Err(CanonicalisationFailed("ArcadeDB upstream
///   error: <status>: <truncated-safe-body>"))`. The response body
///   is read AND truncated to 512 bytes AND scrubbed against the
///   username regex AND emitted to the error string for operator
///   diagnostics. Cypher parse errors land here.
///
/// Body-truncation rationale: ArcadeDB server errors are normally
/// short JSON bodies (`{"error": "...", "exception": "..."}`); 512
/// bytes is enough for human triage without risking accidental log
/// pollution from a misbehaving server returning a megabyte of HTML.
pub(crate) fn map_response_error(
    resp: Response,
    auth_username: &str,
) -> ProjectorResult<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        // FIXED error string — body is intentionally NOT read or
        // forwarded. An attacker controlling the server could otherwise
        // exfiltrate operator-side context by stuffing it into the
        // 401 body and waiting for it to surface in Atlas logs.
        return Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB authentication failed (credentials redacted)".to_string(),
        ));
    }
    // 4xx / 5xx other than auth: read body for operator diagnostics.
    // Truncate to 512 bytes and scrub credential references.
    let body = resp
        .text()
        .unwrap_or_else(|e| format!("<body-read-failed: {}>", describe_reqwest_error(&e)));
    let safe_body = scrub_and_truncate(&body, auth_username, 512);
    if status.is_server_error() {
        Err(ProjectorError::CanonicalisationFailed(format!(
            "ArcadeDB upstream error: {status}: {safe_body}"
        )))
    } else {
        // 4xx other than 401/403 — Cypher rejection, malformed
        // request, db-not-found, etc.
        Err(ProjectorError::CanonicalisationFailed(format!(
            "ArcadeDB rejected request: {status}: {safe_body}"
        )))
    }
}

/// Map a transport-level `reqwest::Error` (timeout, DNS failure, TLS
/// handshake error, connection-refused) into a redacted
/// `ProjectorError`. Called when `send()` itself fails (before any
/// `Response` is returned).
pub(crate) fn map_transport_error(e: reqwest::Error) -> ProjectorError {
    ProjectorError::CanonicalisationFailed(format!(
        "ArcadeDB transport error: {kind}",
        kind = describe_reqwest_error(&e)
    ))
}

/// Produce a short string describing a `reqwest::Error` WITHOUT
/// embedding the request URL (which may include credentials in the
/// userinfo segment) or any auth header.
///
/// `reqwest::Error::Display` includes the URL by default, which is
/// fine for ordinary URLs but unsafe when an attacker could control
/// the URL or when the URL was built from a tainted source. We strip
/// the URL entirely and surface only the error kind. Operator triage
/// retains the kind (timeout / connect / request / decode); the URL
/// is reconstructable from the calling code path.
fn describe_reqwest_error(e: &reqwest::Error) -> String {
    // Use a small dispatch on the known kinds rather than `Display`
    // (which echoes the URL).
    if e.is_timeout() {
        "timeout".to_string()
    } else if e.is_connect() {
        "connect failed".to_string()
    } else if e.is_request() {
        "request build failed".to_string()
    } else if e.is_decode() {
        "response decode failed".to_string()
    } else if e.is_body() {
        "body read failed".to_string()
    } else if e.is_status() {
        format!(
            "non-success status: {}",
            e.status().map_or_else(|| "?".to_string(), |s| s.to_string())
        )
    } else {
        // Generic fallback. We still avoid `Display` (URL echo) and
        // surface only `Debug` of the kind via a stable string.
        "transport error".to_string()
    }
}

/// Scrub-and-truncate a server response body:
/// 1. Replace any verbatim occurrence of the `auth_username` with
///    `<user>` so a misbehaving server that echoes the auth header
///    into its error body cannot smuggle the username out via a log
///    line. (The password is never sent to the server in the request
///    body — only the `Authorization` header — so it cannot appear in
///    a server-side echo. The username CAN appear in e.g. a
///    permission-denied error message.)
/// 2. Truncate to `max_len` bytes (UTF-8 boundary-safe: we walk
///    char_indices and cut at the last char boundary ≤ max_len).
/// 3. Collapse control characters (anything < 0x20 except space/tab)
///    to `?` so a server stuffing `\r\n` into the body cannot forge
///    log lines when the error is echoed by `tracing` / `slog`.
fn scrub_and_truncate(body: &str, auth_username: &str, max_len: usize) -> String {
    // 1. Username scrub. Skip the replace if `auth_username` is empty
    // (Replace::replace on "" is a no-op pattern in stdlib but
    // documenting the guard explicitly).
    let scrubbed: String = if auth_username.is_empty() {
        body.to_string()
    } else {
        body.replace(auth_username, "<user>")
    };
    // 2. Truncate at a char boundary.
    let truncated: &str = if scrubbed.len() <= max_len {
        &scrubbed
    } else {
        // Find the last char boundary ≤ max_len.
        let mut cut = max_len;
        while cut > 0 && !scrubbed.is_char_boundary(cut) {
            cut -= 1;
        }
        &scrubbed[..cut]
    };
    // 3. Collapse control characters.
    truncated
        .chars()
        .map(|c| {
            if c == ' ' || c == '\t' || (c.is_ascii_graphic()) {
                c
            } else if c.is_alphanumeric() {
                // Non-ASCII alphabetics are graphic-equivalent for log safety.
                c
            } else {
                '?'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_auth_debug_redacts_password() {
        let a = BasicAuth::new("root", "topsecret");
        let s = format!("{a:?}");
        assert!(
            !s.contains("topsecret"),
            "password leaked into Debug output: {s}"
        );
        assert!(
            s.contains("<redacted>"),
            "expected redaction marker in {s:?}"
        );
        // Username is intentionally preserved.
        assert!(s.contains("root"), "username missing in {s:?}");
    }

    #[test]
    fn secret_string_debug_redacts() {
        let s = SecretString::new("topsecret".to_string());
        let dbg = format!("{s:?}");
        assert!(!dbg.contains("topsecret"));
        assert!(dbg.contains("<redacted>"));
    }

    #[test]
    fn secret_string_expose_returns_inner() {
        let s = SecretString::new("topsecret".to_string());
        assert_eq!(s.expose(), "topsecret");
    }

    #[test]
    fn scrub_and_truncate_replaces_username() {
        // Server stuffs the username into an error message — scrub it.
        let body = "permission denied for user root on database foo";
        let scrubbed = scrub_and_truncate(body, "root", 512);
        assert!(!scrubbed.contains("root"), "{scrubbed:?}");
        assert!(scrubbed.contains("<user>"), "{scrubbed:?}");
    }

    #[test]
    fn scrub_and_truncate_caps_length() {
        let body = "x".repeat(2000);
        let s = scrub_and_truncate(&body, "root", 512);
        assert!(s.len() <= 512, "got len {}", s.len());
    }

    #[test]
    fn scrub_and_truncate_collapses_crlf() {
        // CRLF would forge log lines if echoed.
        let body = "error\r\nFAKE LOG LINE\nattacker output";
        let s = scrub_and_truncate(body, "", 512);
        assert!(!s.contains('\r'), "{s:?}");
        assert!(!s.contains('\n'), "{s:?}");
    }

    #[test]
    fn scrub_and_truncate_preserves_ascii_graphic() {
        let body = "ok: {\"status\": 200}";
        let s = scrub_and_truncate(body, "", 512);
        assert_eq!(s, body);
    }

    #[test]
    fn build_http_client_does_not_touch_network() {
        // Pure-function smoke. Returns a Client without any IO.
        let c = build_http_client();
        assert!(c.is_ok());
    }
}
