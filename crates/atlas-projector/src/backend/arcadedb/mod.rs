//! V2-β Welle 17b: production `ArcadeDbBackend` implementation.
//!
//! This module replaces the W17a stub (every method was
//! `unimplemented!()`) with a `reqwest::blocking`-based driver per
//! ADR-Atlas-010 §4 (8 binding sub-decisions) + ADR-Atlas-011 §4.3
//! (sub-decisions #10/#11/#12 — `'static` lifetime, `check_workspace_id`
//! call-site, `check_value_depth_and_size` call-site).
//!
//! ## Module split
//!
//! - [`client`] — HTTP client construction, [`BasicAuth`] with redaction
//!   discipline, response-to-error mapping. Single audit point for
//!   credential handling.
//! - [`cypher`] — Cypher query templates + result-row parsers. Single
//!   audit point for Cypher-injection safety (every query uses
//!   parameterised binding; nothing user-supplied is interpolated into
//!   the query text).
//! - this file — backend + txn structs, trait impls, transaction
//!   lifecycle (begin / batch_upsert / commit / rollback).
//!
//! ## Wire protocol summary (recap from spike §3)
//!
//! - **Begin transaction:** `POST /api/v1/begin/{db_name}` with HTTP
//!   Basic auth. Response carries `arcadedb-session-id` header used in
//!   every subsequent call until commit or rollback.
//! - **Run command:** `POST /api/v1/command/{db_name}` with body
//!   `{"language": "cypher", "command": "<cypher>", "params": { ... }}`
//!   + session header. Used for `MERGE`, `CREATE`, `SET`.
//! - **Run query:** `POST /api/v1/query/{db_name}` with body
//!   `{"language": "cypher", "command": "<cypher>", "params": { ... }}`.
//!   Used for `MATCH` reads. Queries are not session-bound (read paths
//!   in this driver bypass transactions).
//! - **Commit transaction:** `POST /api/v1/commit/{db_name}` + session
//!   header.
//! - **Rollback transaction:** `POST /api/v1/rollback/{db_name}` +
//!   session header.
//!
//! ## Lazy database creation
//!
//! ADR-Atlas-010 §4 sub-decision #4 binds "one ArcadeDB database per
//! Atlas workspace". The first time a backend calls `begin()` for a
//! given `workspace_id`, the database for that workspace may or may
//! not exist on the server. The driver lazy-creates the database via
//! `POST /api/v1/server` with command `create database <db_name>`
//! (the same shape the spike §3 ArcadeDB primer documents) and is
//! tolerant of "database already exists" responses (treated as
//! idempotent success). See [`ArcadeDbBackend::ensure_database_exists`].
//!
//! ## Lifetime ('static)
//!
//! ADR-011 §4.3 sub-decision #10: the `WorkspaceTxn` handle is
//! `'static` end-to-end. [`ArcadeDbTxn`] holds OWNED copies of every
//! piece of state it needs (owned `db_name`, owned `session_id`,
//! cloned `reqwest::Client`, cloned base `Url`, cloned [`BasicAuth`]).
//! No field borrows from `&self.backend`. Structurally honoured.

pub(crate) mod client;
pub(crate) mod cypher;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use reqwest::blocking::Client;
use serde_json::{json, Value};
use url::Url;

use crate::backend::{
    check_workspace_id, Edge as BackendEdge, GraphStateBackend, UpsertResult, Vertex as BackendVertex,
    WorkspaceId, WorkspaceTxn,
};
use crate::error::{ProjectorError, ProjectorResult};

pub use client::BasicAuth;

use self::client::{
    apply_basic_auth, apply_session_header, build_http_client, map_response_error,
    map_transport_error,
};
use self::cypher::{
    db_name_for_workspace, edges_query, parse_edge_row, parse_query_response, parse_vertex_row,
    upsert_edge_command, upsert_vertex_command, vertices_query,
};

/// Production [`GraphStateBackend`] implementation for ArcadeDB.
///
/// Carries one shared `reqwest::blocking::Client` (connection pooling
/// is `reqwest`-managed), the ArcadeDB base URL (e.g.
/// `http://localhost:2480`), and per-instance HTTP Basic credentials
/// (per ADR-Atlas-010 §6 OQ-5 — V2-β starts with Basic; V2-γ MAY want
/// JWT bearer).
///
/// The backend itself does NOT carry a "current workspace" — every
/// call passes the `workspace_id` and the driver derives the
/// per-workspace database name at the boundary. Stateless wrt
/// workspace, which lets a single backend instance multiplex across
/// workspaces in the Option-A parallel-projection loop (ADR-Atlas-007
/// §3.1).
///
/// **Cloneable:** the struct is `Clone` so the projector can keep one
/// canonical backend and hand `Arc<dyn GraphStateBackend>` clones to
/// per-task projection workers. Clone is cheap (`reqwest::Client` is
/// internally `Arc`-counted; `Url` clone is O(URL length)).
#[derive(Debug, Clone)]
pub struct ArcadeDbBackend {
    /// Base URL of the ArcadeDB server (scheme + host + port). The
    /// per-endpoint URL is built by joining `/api/v1/...` against this.
    /// Wrapped in `Arc` so clones share the underlying `Url`
    /// allocation rather than reparsing on every call.
    base_url: Arc<Url>,
    /// Shared HTTP client. `reqwest::blocking::Client` is internally
    /// `Arc`-counted for the underlying connection pool, so wrapping
    /// in an outer `Arc` is unnecessary; we hold by value here.
    client: Client,
    /// HTTP Basic credentials. `Clone` is cheap (two `String`
    /// allocations). The clone-on-`begin()` cost is dwarfed by the
    /// HTTP round-trip.
    credentials: BasicAuth,
    /// Per-(backend-instance) cache of database names whose Cypher
    /// schema types (`Vertex`, `Edge`) have already been bootstrapped
    /// via [`ensure_schema_types_exist`]. Shared across clones so a
    /// projector with many backend handles only pays the schema-init
    /// HTTP cost once per process per workspace. W17c regression fix:
    /// ArcadeDB 24.10.1 silently no-ops `MERGE (a)-[r:Edge]->(b)` when
    /// the `Edge` type does not yet exist (CREATE auto-creates it,
    /// MERGE does not). Without this bootstrap, edges written via the
    /// trait surface vanish and `canonical_state()` returns a
    /// vertex-only hash that diverges from `InMemoryBackend`.
    schema_initialized: Arc<Mutex<HashSet<String>>>,
}

impl ArcadeDbBackend {
    /// Construct a new `ArcadeDbBackend` for a configured ArcadeDB
    /// instance.
    ///
    /// **Pure constructor:** does NOT touch the network. Validation of
    /// the server's reachability + credentials happens on the FIRST
    /// `begin()` call (lazy connect, idiomatic for `reqwest` clients).
    /// This contract matches the W17a stub (`new()` never panicked;
    /// only trait method invocations did).
    ///
    /// Errors on:
    /// - Internal client-build failure (incompatible `reqwest` feature
    ///   flags at build time, which is a deployment bug rather than a
    ///   runtime condition).
    /// - `base_url` carries userinfo (`user:pass@host`). HTTP Basic
    ///   credentials MUST go through [`BasicAuth`] and the redacting
    ///   `Debug` chain; embedding them in the URL would defeat the
    ///   redaction discipline (the URL is `Debug`-printable). Closes
    ///   W17b security-review M-2 (derived `Debug` leak surface).
    /// - `base_url` scheme is anything other than `http` or `https`
    ///   (e.g. `file:`, `ftp:`). These schemes cannot reach the
    ///   ArcadeDB Server-mode HTTP API and would otherwise produce a
    ///   confusing later error or, worse, exfiltrate ambient creds.
    ///
    /// **Plaintext HTTP note:** `http://` URLs are accepted to keep the
    /// docker-compose §4.7 local-dev sketch frictionless, but Basic-
    /// Auth credentials over HTTP are base64-encoded (NOT encrypted) on
    /// the wire. Operator runbook §16 requires `https://` in
    /// production deployments.
    pub fn new(base_url: Url, credentials: BasicAuth) -> ProjectorResult<Self> {
        match base_url.scheme() {
            "http" | "https" => {}
            other => {
                return Err(ProjectorError::CanonicalisationFailed(format!(
                    "ArcadeDbBackend::new: unsupported URL scheme {other:?} (expected http or https)"
                )));
            }
        }
        if !base_url.username().is_empty() || base_url.password().is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "ArcadeDbBackend::new: base_url must not carry userinfo (use BasicAuth instead)"
                    .to_string(),
            ));
        }
        let client = build_http_client()?;
        Ok(Self {
            base_url: Arc::new(base_url),
            client,
            credentials,
            schema_initialized: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Internal: build a per-endpoint URL by joining `/api/v1/<segment>`
    /// against the configured base URL. The segment string is built
    /// by the caller from trusted constants + the validated
    /// `db_name` produced by [`db_name_for_workspace`], so no
    /// percent-encoding step is needed here.
    fn endpoint(&self, segment: &str) -> ProjectorResult<Url> {
        self.base_url.join(segment).map_err(|e| {
            ProjectorError::CanonicalisationFailed(format!(
                "ArcadeDB endpoint URL build failed for segment {segment:?}: {e}"
            ))
        })
    }

    /// Lazy-create the per-workspace ArcadeDB database if it does not
    /// already exist. Idempotent: a "database already exists" response
    /// is treated as success.
    ///
    /// Implementation: POST `/api/v1/server` with body
    /// `{"command": "create database <db_name>"}`. ArcadeDB returns a
    /// success status on first creation and a 4xx on
    /// already-exists; we accept BOTH as "post-condition met". Any
    /// other 4xx (e.g. permission denied) bubbles up as a redacted
    /// error.
    ///
    /// Note: ArcadeDB's `/api/v1/server` is an admin endpoint that
    /// requires root credentials. ADR-Atlas-010 §6 OQ-10 carries the
    /// V2-γ work to split the projector's auth between admin (create
    /// database) and per-tenant (read/write) credentials. For V2-β we
    /// follow the docker-compose §4.7 sketch: a single root-credential
    /// backend that creates databases on demand.
    fn ensure_database_exists(&self, db_name: &str) -> ProjectorResult<()> {
        let url = self.endpoint("api/v1/server")?;
        // `create database <name>` is the command shape; we do NOT
        // interpolate workspace_id directly — only the derived
        // `db_name` which has been built by `db_name_for_workspace`
        // from a `check_workspace_id`-validated workspace_id.
        let command = format!("create database {db_name}");
        let body = json!({ "command": command });
        let req = self
            .client
            .post(url)
            .json(&body);
        let req = apply_basic_auth(req, &self.credentials);
        let resp = req.send().map_err(map_transport_error)?;
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        // Tolerate "already exists" responses. ArcadeDB encodes this
        // condition with various 4xx codes across versions; we accept
        // 4xx as "post-condition met" IFF the response body contains
        // an "already exists" marker. Otherwise propagate.
        if status.is_client_error() {
            // Bounded body read (W17b security-review L-1): cap at
            // 512 bytes so a misbehaving server cannot force Atlas to
            // buffer a megabyte response just to look for the
            // "already exists" marker.
            let raw = resp.bytes().unwrap_or_default();
            let take = raw.len().min(512);
            let body_str = String::from_utf8_lossy(&raw[..take]).into_owned();
            let lower = body_str.to_lowercase();
            if lower.contains("already exists") || lower.contains("already exist") {
                return Ok(());
            }
            // For 401/403, fall through to the standard error path so
            // operators get the stable "authentication failed" string.
            // We can't easily re-construct a Response here, so we
            // synthesise an equivalent error.
            if status == reqwest::StatusCode::UNAUTHORIZED
                || status == reqwest::StatusCode::FORBIDDEN
            {
                return Err(ProjectorError::CanonicalisationFailed(
                    "ArcadeDB authentication failed (credentials redacted)".to_string(),
                ));
            }
            return Err(ProjectorError::CanonicalisationFailed(format!(
                "ArcadeDB database creation rejected: {status}"
            )));
        }
        // 5xx
        Err(ProjectorError::CanonicalisationFailed(format!(
            "ArcadeDB upstream error during database creation: {status}"
        )))
    }

    /// Bootstrap the Cypher schema types (`Vertex`, `Edge`) for the
    /// per-workspace database. Idempotent; runs at most once per
    /// `(ArcadeDbBackend instance, db_name)` pair under contention-free
    /// conditions. Under contention (two threads racing the cache
    /// check) the HTTP bootstrap may run multiple times — that's
    /// harmless because the combined CREATE+DELETE Cypher statement
    /// is itself idempotent (any leftover sentinels are matched and
    /// deleted by the same statement that creates fresh ones).
    ///
    /// **W17c regression fix.** ArcadeDB 24.10.1's Cypher subset
    /// auto-creates the `Vertex` type on first `MERGE (n:Vertex)` but
    /// does NOT auto-create the `Edge` type on first
    /// `MERGE (a)-[r:Edge]->(b)`. Without an existing `Edge` type the
    /// edge MERGE silently no-ops: the HTTP call returns 2xx, the
    /// transaction commits cleanly, but no edge row is persisted.
    /// `vertices_sorted` then returns the written vertices while
    /// `edges_sorted` returns an empty array, and
    /// `canonical_state()` reports a vertex-only hash that diverges
    /// from `InMemoryBackend`'s hash for the same input. The
    /// cross-backend byte-determinism test (W17c CI) fires on this
    /// drift.
    ///
    /// Workaround: register both types via a sentinel
    /// `CREATE (a:Vertex {_atlas_schema_init: true})-[r:Edge
    /// {_atlas_schema_init: true}]->(b:Vertex {_atlas_schema_init: true})`
    /// (Cypher CREATE on edges DOES auto-create the type) and then
    /// clean up the sentinel records via `DETACH DELETE`. Schema
    /// changes persist; data side effects do not. ~6 ms HTTP cost,
    /// paid at most once per (process, db_name) pair thanks to the
    /// cache.
    ///
    /// V2-γ may replace this with a proper schema-DDL call once
    /// ArcadeDB exposes a stable `create edge type X if not exists`
    /// SQL grammar on freshly-created databases (the same DDL works
    /// on populated databases, hinting at a parser-state bug in
    /// 24.10.1; the sentinel workaround is robust across that
    /// difference).
    fn ensure_schema_types_exist(&self, db_name: &str) -> ProjectorResult<()> {
        {
            let cache = self.schema_initialized.lock().map_err(|_| {
                ProjectorError::CanonicalisationFailed(
                    "ArcadeDbBackend schema_initialized cache lock poisoned".to_string(),
                )
            })?;
            if cache.contains(db_name) {
                return Ok(());
            }
        }

        // Combined CREATE + DETACH DELETE in a SINGLE Cypher statement
        // so the operation is atomic from the client's perspective: if
        // the HTTP call returns 2xx, the schema types are registered
        // AND the sentinel records are cleaned up. If the HTTP call
        // fails, NEITHER side effect lands. Closes the W17c review
        // HIGH-1 finding: a separate CREATE-then-DELETE pair leaves
        // orphan sentinel nodes if the process crashes between the
        // two calls.
        //
        // ArcadeDB Cypher accepts the `CREATE ... WITH ... DETACH
        // DELETE ...` chain in a single command and registers the
        // Edge type as a side effect of the CREATE phase; the schema
        // change persists even though the data records are deleted in
        // the same statement.
        let bootstrap_cypher = "\
            CREATE (a:Vertex {_atlas_schema_init: true, _atlas_init_role: 'a'}) \
                  -[r:Edge {_atlas_schema_init: true}]-> \
                   (b:Vertex {_atlas_schema_init: true, _atlas_init_role: 'b'}) \
            WITH a, b, r \
            DETACH DELETE a, b";
        self.run_admin_command(db_name, bootstrap_cypher, json!({}))?;

        // Mark this db_name as initialized so subsequent begin() calls
        // skip the ~6 ms HTTP cost.
        let mut cache = self.schema_initialized.lock().map_err(|_| {
            ProjectorError::CanonicalisationFailed(
                "ArcadeDbBackend schema_initialized cache lock poisoned".to_string(),
            )
        })?;
        cache.insert(db_name.to_string());
        Ok(())
    }

    /// Run a Cypher command outside any transaction (used by
    /// [`ensure_schema_types_exist`]). Distinct from
    /// [`ArcadeDbTxn::run_command`] which is transaction-bound.
    fn run_admin_command(
        &self,
        db_name: &str,
        command: &str,
        params: Value,
    ) -> ProjectorResult<()> {
        let url = self.endpoint(&format!("api/v1/command/{db_name}"))?;
        let body = json!({
            "language": "cypher",
            "command": command,
            "params": params,
        });
        let req = self.client.post(url).json(&body);
        let req = apply_basic_auth(req, &self.credentials);
        let resp = req.send().map_err(map_transport_error)?;
        let _ = map_response_error(resp, &self.credentials.username)?;
        Ok(())
    }

    /// Run a Cypher query (read path) against the given database.
    /// Returns the parsed `result` array. Used by `vertices_sorted`
    /// + `edges_sorted`.
    fn run_query(&self, db_name: &str, command: String, params: Value) -> ProjectorResult<Vec<Value>> {
        let url = self.endpoint(&format!("api/v1/query/{db_name}"))?;
        let body = json!({
            "language": "cypher",
            "command": command,
            "params": params,
        });
        let req = self.client.post(url).json(&body);
        let req = apply_basic_auth(req, &self.credentials);
        let resp = req.send().map_err(map_transport_error)?;
        let resp = map_response_error(resp, &self.credentials.username)?;
        let bytes = resp.bytes().map_err(map_transport_error)?;
        parse_query_response(&bytes)
    }
}

impl GraphStateBackend for ArcadeDbBackend {
    fn begin(
        &self,
        workspace_id: &WorkspaceId,
    ) -> ProjectorResult<Box<dyn WorkspaceTxn + 'static>> {
        // ADR-011 §4.3 sub-decision #11: FIRST line — validate
        // workspace_id before any HTTP request is constructed.
        check_workspace_id(workspace_id)?;

        // W17b security-review HIGH-2: db-name char-class check is a
        // second validation layer on top of `check_workspace_id`.
        // Closes the `create database <db_name>` admin-command
        // injection surface in `ensure_database_exists` below.
        let db_name = db_name_for_workspace(workspace_id)?;

        // Lazy-create the per-workspace database if needed (ADR-010 §4
        // sub-decision #4 — one DB per workspace).
        self.ensure_database_exists(&db_name)?;

        // Ensure Cypher schema types (Vertex + Edge) are registered.
        // ArcadeDB 24.10.1 silently no-ops MERGE on edges if Edge type
        // does not yet exist; W17c regression fix — see
        // `ensure_schema_types_exist` doc-comment. Idempotent and
        // cached per (backend instance, db_name) so this is paid at
        // most once per workspace per process lifetime.
        self.ensure_schema_types_exist(&db_name)?;

        // POST /api/v1/begin/{db_name}, extract session id from
        // response header.
        let url = self.endpoint(&format!("api/v1/begin/{db_name}"))?;
        let req = self.client.post(url);
        let req = apply_basic_auth(req, &self.credentials);
        let resp = req.send().map_err(map_transport_error)?;
        let resp = map_response_error(resp, &self.credentials.username)?;
        let session_id = resp
            .headers()
            .get("arcadedb-session-id")
            .ok_or_else(|| {
                ProjectorError::CanonicalisationFailed(
                    "ArcadeDB begin response missing `arcadedb-session-id` header".to_string(),
                )
            })?
            .to_str()
            .map_err(|_| {
                ProjectorError::CanonicalisationFailed(
                    "ArcadeDB session id header is not valid ASCII".to_string(),
                )
            })?
            .to_string();

        // Construct the owned txn. All fields owned; lifetime 'static
        // structurally honoured (ADR-011 §4.3 sub-decision #10).
        Ok(Box::new(ArcadeDbTxn {
            db_name,
            workspace_id: workspace_id.clone(),
            session_id,
            client: self.client.clone(),
            base_url: Arc::clone(&self.base_url),
            credentials: self.credentials.clone(),
            finalised: None,
        }))
    }

    fn vertices_sorted(&self, workspace_id: &WorkspaceId) -> ProjectorResult<Vec<BackendVertex>> {
        // Same workspace-id validation at the read path — the boundary
        // also flows into the URL path segment via `db_name`.
        check_workspace_id(workspace_id)?;
        let db_name = db_name_for_workspace(workspace_id)?;
        let (command, params) = vertices_query(workspace_id);
        let rows = self.run_query(&db_name, command, params)?;
        rows.iter()
            .map(|row| parse_vertex_row(row, workspace_id))
            .collect()
    }

    fn edges_sorted(&self, workspace_id: &WorkspaceId) -> ProjectorResult<Vec<BackendEdge>> {
        check_workspace_id(workspace_id)?;
        let db_name = db_name_for_workspace(workspace_id)?;
        let (command, params) = edges_query(workspace_id);
        let rows = self.run_query(&db_name, command, params)?;
        rows.iter()
            .map(|row| parse_edge_row(row, workspace_id))
            .collect()
    }

    // `canonical_state` is INTENTIONALLY not overridden here. The
    // trait-default impl walks `vertices_sorted` + `edges_sorted` and
    // feeds them through V2-α's `canonical::graph_state_hash`. Because
    // both reads honour the §4.9 adapter contract
    // (`ORDER BY entity_uuid ASC` / `ORDER BY edge_id ASC` in the
    // Cypher) the resulting bytes are byte-identical to what the
    // InMemoryBackend would produce for the same workspace contents.
    // The cross-backend test (`tests/cross_backend_byte_determinism.rs`,
    // CI-gated behind `ATLAS_ARCADEDB_URL`) pins this guarantee.

    fn backend_id(&self) -> &'static str {
        "arcadedb-server"
    }
}

/// Per-workspace transaction handle for [`ArcadeDbBackend`].
///
/// Each field is OWNED to make the `'static` lifetime structurally
/// honoured (ADR-011 §4.3 sub-decision #10). No field borrows from
/// the originating `&ArcadeDbBackend`.
///
/// **Drop without commit = rollback (effective).** ArcadeDB
/// server-side cleans up the abandoned session on its own timeout;
/// the driver does NOT issue an automatic rollback on `Drop` because
/// (a) `Drop` cannot return a `Result` (silent rollback failures are
/// worse than letting the server time the session out), and (b) the
/// trait's `commit()` / `rollback()` already consume the box, which
/// means a forgotten finalisation IS a programming bug for the
/// caller to notice. The `finalised` flag is defensive against
/// reaching upsert calls after commit/rollback (impossible via the
/// public API; type system already prevents it via `self: Box<Self>`).
pub struct ArcadeDbTxn {
    /// Per-workspace ArcadeDB database name; URL path segment.
    db_name: String,
    /// Workspace id; used as `$ws` parameter binding in every Cypher
    /// statement.
    workspace_id: WorkspaceId,
    /// `arcadedb-session-id` header value; required on every txn-
    /// scoped HTTP call until commit/rollback.
    session_id: String,
    /// Cloned HTTP client (cheap — reqwest internally Arc-counts).
    client: Client,
    /// Cloned base URL.
    base_url: Arc<Url>,
    /// Cloned credentials. Required because /api/v1/command requires
    /// HTTP Basic on every call (the session id alone is not
    /// authentication; ArcadeDB Server-mode requires both).
    credentials: BasicAuth,
    /// `Some(reason)` after commit/rollback; defence-in-depth flag.
    /// Type system already prevents use-after-finalise via
    /// `self: Box<Self>` consumption in the trait's commit/rollback.
    finalised: Option<&'static str>,
}

impl std::fmt::Debug for ArcadeDbTxn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcadeDbTxn")
            .field("db_name", &self.db_name)
            .field("workspace_id", &self.workspace_id)
            // session_id is not sensitive long-term (server times it
            // out) but printing it accidentally into a log line could
            // help an attacker who can read logs continue an
            // in-flight transaction. Redact.
            .field("session_id", &"<redacted>")
            .field("credentials", &self.credentials)
            .field("finalised", &self.finalised)
            .finish()
    }
}

impl ArcadeDbTxn {
    /// Build a per-endpoint URL by joining `/api/v1/<segment>` against
    /// the cloned base URL.
    fn endpoint(&self, segment: &str) -> ProjectorResult<Url> {
        self.base_url.join(segment).map_err(|e| {
            ProjectorError::CanonicalisationFailed(format!(
                "ArcadeDB endpoint URL build failed for segment {segment:?}: {e}"
            ))
        })
    }

    /// Run a Cypher command (write path: MERGE / SET / CREATE) within
    /// the open transaction. Applies session header + Basic auth.
    ///
    /// Discards the response body — every current caller ignores the
    /// returned value (the trait surface only requires "logical id is
    /// in the workspace after this call returns"; one HTTP MERGE
    /// produces either a new or updated row and the contract does not
    /// distinguish). Discarding here closes W17b security-review HIGH-1:
    /// returning the raw `serde_json::Value` would have created a
    /// latent caller path that bypassed
    /// [`check_value_depth_and_size`] (ADR-011 §4.3 sub-decision #12).
    /// If a future caller needs the response body it MUST re-parse via
    /// a function that applies the depth/size cap at the boundary.
    fn run_command(&self, command: String, params: Value) -> ProjectorResult<()> {
        let url = self.endpoint(&format!("api/v1/command/{db_name}", db_name = self.db_name))?;
        let body = json!({
            "language": "cypher",
            "command": command,
            "params": params,
        });
        let req = self.client.post(url).json(&body);
        let req = apply_basic_auth(req, &self.credentials);
        let req = apply_session_header(req, &self.session_id);
        let resp = req.send().map_err(map_transport_error)?;
        let _ = map_response_error(resp, &self.credentials.username)?;
        Ok(())
    }

    /// Run multiple Cypher commands as a single multi-statement
    /// transaction (OQ-2 contract — vertices BEFORE edges in one
    /// HTTP roundtrip). ArcadeDB's `/api/v1/command/{db}` accepts an
    /// SQL-script language for batching; for Cypher we issue
    /// sequential commands within the open session. Since all
    /// statements share the same session id, ArcadeDB applies them in
    /// the same WAL transaction and commits/rolls back atomically.
    ///
    /// V2-γ MAY benchmark a single `/api/v1/command` call that sends
    /// every statement in one body via the SQL-script language; the
    /// current shape is the simpler one. ADR-011 §6 OQ-7 tracks this.
    fn run_batch(
        &self,
        vertices: &[BackendVertex],
        edges: &[BackendEdge],
    ) -> ProjectorResult<Vec<UpsertResult>> {
        let mut results = Vec::with_capacity(vertices.len() + edges.len());
        // OQ-2 contract: vertices BEFORE edges.
        for v in vertices {
            if v.entity_uuid.is_empty() {
                return Err(ProjectorError::MalformedEntityUuid(
                    "entity_uuid is empty".to_string(),
                ));
            }
            let (command, params) = upsert_vertex_command(&self.workspace_id, v);
            self.run_command(command, params)?;
            results.push(UpsertResult::new(true, v.entity_uuid.clone()));
        }
        for e in edges {
            if e.edge_id.is_empty() {
                return Err(ProjectorError::MalformedEntityUuid(format!(
                    "edge_id is empty (edge {}-{}-{})",
                    e.from, e.label, e.to
                )));
            }
            let (command, params) = upsert_edge_command(&self.workspace_id, e);
            self.run_command(command, params)?;
            results.push(UpsertResult::new(true, e.edge_id.clone()));
        }
        Ok(results)
    }
}

impl WorkspaceTxn for ArcadeDbTxn {
    fn upsert_vertex(&mut self, v: &BackendVertex) -> ProjectorResult<UpsertResult> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "ArcadeDbTxn used after finalisation".to_string(),
            ));
        }
        if v.entity_uuid.is_empty() {
            return Err(ProjectorError::MalformedEntityUuid(
                "entity_uuid is empty".to_string(),
            ));
        }
        let (command, params) = upsert_vertex_command(&self.workspace_id, v);
        self.run_command(command, params)?;
        Ok(UpsertResult::new(true, v.entity_uuid.clone()))
    }

    fn upsert_edge(&mut self, e: &BackendEdge) -> ProjectorResult<UpsertResult> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "ArcadeDbTxn used after finalisation".to_string(),
            ));
        }
        if e.edge_id.is_empty() {
            return Err(ProjectorError::MalformedEntityUuid(format!(
                "edge_id is empty (edge {}-{}-{})",
                e.from, e.label, e.to
            )));
        }
        let (command, params) = upsert_edge_command(&self.workspace_id, e);
        self.run_command(command, params)?;
        Ok(UpsertResult::new(true, e.edge_id.clone()))
    }

    fn batch_upsert(
        &mut self,
        vertices: &[BackendVertex],
        edges: &[BackendEdge],
    ) -> ProjectorResult<Vec<UpsertResult>> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "ArcadeDbTxn used after finalisation".to_string(),
            ));
        }
        self.run_batch(vertices, edges)
    }

    fn commit(mut self: Box<Self>) -> ProjectorResult<()> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "ArcadeDbTxn already finalised".to_string(),
            ));
        }
        let url = self.endpoint(&format!("api/v1/commit/{db_name}", db_name = self.db_name))?;
        let req = self.client.post(url);
        let req = apply_basic_auth(req, &self.credentials);
        let req = apply_session_header(req, &self.session_id);
        let resp = req.send().map_err(map_transport_error)?;
        let _ = map_response_error(resp, &self.credentials.username)?;
        self.finalised = Some("commit");
        Ok(())
    }

    fn rollback(mut self: Box<Self>) -> ProjectorResult<()> {
        if self.finalised.is_some() {
            return Err(ProjectorError::CanonicalisationFailed(
                "ArcadeDbTxn already finalised".to_string(),
            ));
        }
        let url = self.endpoint(&format!(
            "api/v1/rollback/{db_name}",
            db_name = self.db_name
        ))?;
        let req = self.client.post(url);
        let req = apply_basic_auth(req, &self.credentials);
        let req = apply_session_header(req, &self.session_id);
        let resp = req.send().map_err(map_transport_error)?;
        let _ = map_response_error(resp, &self.credentials.username)?;
        self.finalised = Some("rollback");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests exercising the parts of the driver that do NOT
    //! touch the network. Live-server integration tests live in
    //! `tests/cross_backend_byte_determinism.rs` behind the
    //! `ATLAS_ARCADEDB_URL` env-var gate (W17c CI workflow turns it on
    //! with a Docker-Compose fixture).

    use super::*;

    fn auth() -> BasicAuth {
        BasicAuth::new("root", "rootpw")
    }

    fn base_url() -> Url {
        // Use a localhost URL by convention; constructors never touch
        // the network so the value is just data.
        Url::parse("http://localhost:2480").expect("static URL parses")
    }

    #[test]
    fn arcadedb_backend_constructs_without_network() {
        // Constructor must not panic or block on IO.
        let b = ArcadeDbBackend::new(base_url(), auth());
        assert!(b.is_ok());
    }

    #[test]
    fn arcadedb_backend_id_is_arcadedb_server() {
        let b = ArcadeDbBackend::new(base_url(), auth()).unwrap();
        assert_eq!(b.backend_id(), "arcadedb-server");
    }

    #[test]
    fn arcadedb_backend_debug_does_not_leak_password() {
        let b = ArcadeDbBackend::new(base_url(), BasicAuth::new("root", "topsecret")).unwrap();
        let s = format!("{b:?}");
        assert!(!s.contains("topsecret"), "{s:?}");
    }

    #[test]
    fn begin_rejects_invalid_workspace_id() {
        // workspace_id validation MUST fire BEFORE any HTTP request.
        // We never reach a network call because the empty workspace_id
        // fails the boundary check.
        let b = ArcadeDbBackend::new(base_url(), auth()).unwrap();
        // `Box<dyn WorkspaceTxn>` doesn't impl Debug, so we discard the
        // Ok path's payload before pattern-matching the error.
        let res = b.begin(&"".to_string()).map(|_| ());
        match res {
            Err(ProjectorError::InvalidWorkspaceId { reason }) => {
                assert!(reason.contains("empty"), "{reason:?}");
            }
            other => panic!("expected InvalidWorkspaceId; got {other:?}"),
        }
    }

    #[test]
    fn begin_rejects_path_traversal_workspace_id() {
        let b = ArcadeDbBackend::new(base_url(), auth()).unwrap();
        let res = b.begin(&"../etc/passwd".to_string()).map(|_| ());
        match res {
            Err(ProjectorError::InvalidWorkspaceId { reason }) => {
                assert!(reason.contains("forbidden character"), "{reason:?}");
            }
            other => panic!("expected InvalidWorkspaceId; got {other:?}"),
        }
    }

    #[test]
    fn begin_rejects_crlf_workspace_id_log_injection() {
        let b = ArcadeDbBackend::new(base_url(), auth()).unwrap();
        let res = b.begin(&"legit\nFAKE_LOG_LINE".to_string()).map(|_| ());
        match res {
            Err(ProjectorError::InvalidWorkspaceId { reason }) => {
                assert!(reason.contains("forbidden character"), "{reason:?}");
            }
            other => panic!("expected InvalidWorkspaceId; got {other:?}"),
        }
    }

    #[test]
    fn begin_rejects_workspace_id_with_db_name_incompatible_chars() {
        // `;` survives `check_workspace_id` (ASCII, not in the forbidden
        // set `/ \ NUL CR LF`) but would inject into the
        // `create database <db_name>` admin command + URL path
        // segments. W17b security-review HIGH-2: the second validation
        // layer in `db_name_for_workspace` MUST catch this.
        let b = ArcadeDbBackend::new(base_url(), auth()).unwrap();
        let res = b.begin(&"foo;drop database".to_string()).map(|_| ());
        match res {
            Err(ProjectorError::InvalidWorkspaceId { reason }) => {
                assert!(
                    reason.contains("ArcadeDB database name"),
                    "{reason:?}"
                );
            }
            other => panic!("expected InvalidWorkspaceId; got {other:?}"),
        }
    }

    #[test]
    fn new_rejects_unsupported_scheme() {
        let url = Url::parse("file:///etc/passwd").expect("static URL parses");
        let res = ArcadeDbBackend::new(url, auth());
        match res {
            Err(ProjectorError::CanonicalisationFailed(msg)) => {
                assert!(msg.contains("scheme"), "{msg:?}");
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn new_rejects_url_with_userinfo() {
        let url =
            Url::parse("http://root:secret@localhost:2480").expect("static URL parses");
        let res = ArcadeDbBackend::new(url, auth());
        match res {
            Err(ProjectorError::CanonicalisationFailed(msg)) => {
                assert!(msg.contains("userinfo"), "{msg:?}");
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn vertices_sorted_rejects_invalid_workspace_id() {
        let b = ArcadeDbBackend::new(base_url(), auth()).unwrap();
        let res = b.vertices_sorted(&"".to_string());
        assert!(matches!(res, Err(ProjectorError::InvalidWorkspaceId { .. })));
    }

    #[test]
    fn edges_sorted_rejects_invalid_workspace_id() {
        let b = ArcadeDbBackend::new(base_url(), auth()).unwrap();
        let res = b.edges_sorted(&"".to_string());
        assert!(matches!(res, Err(ProjectorError::InvalidWorkspaceId { .. })));
    }

    #[test]
    fn begin_to_unreachable_server_surfaces_transport_error() {
        // Use a non-routable / closed port. The connect_timeout (5s)
        // bounds the test. We CANNOT assert the test will run quickly
        // in adverse network conditions, so this test stays in the
        // standard suite but is robust: it asserts on the error type,
        // not on timing.
        //
        // Port 1: theoretically tcpmux; in practice almost always
        // closed on localhost. Connect attempt will fail fast.
        let url = Url::parse("http://127.0.0.1:1").expect("static URL parses");
        let b = ArcadeDbBackend::new(url, auth()).unwrap();
        let res = b.begin(&"valid-workspace-id".to_string()).map(|_| ());
        match res {
            Err(ProjectorError::CanonicalisationFailed(msg)) => {
                // We expect a transport error or a database-creation
                // error; both are CanonicalisationFailed and the msg
                // must contain a redacted descriptor. Crucially, it
                // must NOT contain the password.
                assert!(!msg.contains("rootpw"), "password leaked: {msg:?}");
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn arcadedb_txn_static_lifetime_structurally_honoured() {
        // Compile-time check: the type signature returned by `begin()`
        // is `Box<dyn WorkspaceTxn + 'static>`. We DON'T call begin()
        // here because it would hit the network; we instead inspect
        // the type by constructing the trait object directly from a
        // pre-built ArcadeDbTxn whose fields are all owned.
        //
        // This test fails at *compile* time if any field of
        // ArcadeDbTxn ever borrows from a non-'static source.
        fn assert_static<T: 'static>() {}
        assert_static::<ArcadeDbTxn>();
        assert_static::<Box<dyn WorkspaceTxn + 'static>>();
    }
}
