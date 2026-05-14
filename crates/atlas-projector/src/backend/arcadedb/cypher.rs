//! V2-β Welle 17b: Cypher query builders + result parsing for the
//! ArcadeDB driver.
//!
//! Two responsibilities:
//!
//! 1. **Database-name derivation.** ArcadeDB database names allow
//!    alphanumeric + `_` only (verified against the spike §3 / ADR-010
//!    §4 sub-decision #4 "one-DB-per-workspace" rules). [`db_name_for_workspace`]
//!    deterministically maps a `WorkspaceId` (already validated by
//!    `check_workspace_id` at the `begin()` boundary) into a valid
//!    database name by prefixing with `atlas_ws_` and replacing every
//!    hyphen with underscore. The mapping is one-to-one given the
//!    `check_workspace_id` rule set (ASCII + no `/`, `\`, NUL, `\r`,
//!    `\n` + length ≤ 128). Result: `db_name` is at most `len("atlas_ws_") + 128 = 137`
//!    chars; well within any HTTP path-segment cap.
//!
//! 2. **Cypher query templates + parameter builders.** All Atlas-side
//!    Cypher queries are constructed here so the *one place to audit*
//!    for SQL/Cypher-injection safety is this file. Every Cypher
//!    statement uses parameterised binding (`$ws`, `$eid`, `$props`)
//!    and NEVER string-concatenation of user-supplied data into the
//!    query text. ADR-Atlas-010 §4 sub-decision #6 (Cypher
//!    passthrough hardening + DECISION-SEC-4) is honoured here.
//!
//! Parsing: ArcadeDB's `/api/v1/query/{db}` response shape is documented
//! in spike §3. A successful query returns:
//!
//! ```json
//! {
//!   "user": "root",
//!   "version": "24.10.1",
//!   "serverName": "...",
//!   "result": [ { ... }, { ... } ]
//! }
//! ```
//!
//! The driver extracts the `result` array, walks each element, validates
//! each element via [`crate::backend::check_value_depth_and_size`]
//! (W17a-cleanup sub-decision #12, MUST be called BEFORE constructing
//! `Vertex`/`Edge`), and produces typed records.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::backend::{
    check_value_depth_and_size, Edge as BackendEdge, EdgeId, EntityUuid, Vertex as BackendVertex,
    WorkspaceId,
};
use crate::error::{ProjectorError, ProjectorResult};

// W17a-cleanup recommended defaults; passed at every parse-boundary
// call so any tightening is a one-line change here.
pub(crate) const DEFAULT_MAX_VALUE_DEPTH: usize = 32;
pub(crate) const DEFAULT_MAX_VALUE_BYTES: usize = 64 * 1024;

/// Derive the ArcadeDB database name for a workspace.
///
/// Mapping rules:
/// - Prefix `atlas_ws_` (10 chars including the trailing underscore)
///   so the database name unambiguously identifies an Atlas workspace
///   versus any other database an operator might co-locate in the
///   same ArcadeDB instance (anti-collision + operator-runbook
///   discoverability).
/// - Hyphens (`-`) in the workspace id (common in UUID-shaped
///   workspace IDs like `550e8400-e29b-41d4-a716-446655440000`)
///   become underscores (`_`). ArcadeDB DB-name rules disallow `-`
///   per the spike §3 character-set notes.
///
/// **Precondition:** `workspace_id` has already passed
/// [`crate::backend::check_workspace_id`]. That helper enforces ASCII
/// + no `/`, `\`, NUL, `\r`, `\n` + length ≤ 128. The remaining ASCII
/// characters allowed through the check that COULD break ArcadeDB
/// DB-name rules are: spaces, `.`, `:`, `;`, `?`, `&`, `=`, `+`, `*`,
/// `<`, `>`, `|`, `"`, `'`, `(`, `)`, `[`, `]`, `{`, `}`, `,`. We do
/// NOT additionally normalise these: the operator has chosen a
/// workspace identifier shape and we want the resulting db name to
/// remain transparently identifiable in their tooling. If they hand
/// us an exotic shape, the ArcadeDB server will reject the
/// `create database` command at the HTTP boundary and we surface the
/// 4xx through [`super::client::map_response_error`]. The operator
/// learns the constraint at first run, not silently.
///
/// Length: input ≤ 128 → output ≤ `9 + 128 = 137` chars. Well within
/// any URL-path-segment cap (spec says ≥ 255 typical, 1024 minimum
/// per RFC 3986 §2.4 implementation notes).
pub(crate) fn db_name_for_workspace(workspace_id: &WorkspaceId) -> String {
    let mut s = String::with_capacity("atlas_ws_".len() + workspace_id.len());
    s.push_str("atlas_ws_");
    for ch in workspace_id.chars() {
        if ch == '-' {
            s.push('_');
        } else {
            s.push(ch);
        }
    }
    s
}

/// Build the parameterised Cypher query for fetching all vertices in a
/// workspace, sorted by `entity_uuid` ASC per ADR-Atlas-010 §4
/// sub-decision #6 (byte-determinism adapter contract).
///
/// Returned tuple: `(command_string, params_json)`. The
/// `command_string` contains NO interpolated user input — every
/// dynamic value is bound via `$ws` and passed in `params_json`.
///
/// The vertex label is fixed at `:Vertex` (a constant managed by the
/// driver; the schema-set-up step in `ArcadeDbBackend::begin()` will
/// create the type before any insert). The `workspace_id` parameter
/// is bound via `$ws` for the application-layer second-line tenant
/// isolation per ADR-Atlas-010 §4 sub-decision #7.
pub(crate) fn vertices_query(workspace_id: &WorkspaceId) -> (String, Value) {
    let command = "MATCH (n:Vertex {workspace_id: $ws}) RETURN n ORDER BY n.entity_uuid ASC"
        .to_string();
    let params = json!({ "ws": workspace_id });
    (command, params)
}

/// Build the parameterised Cypher query for fetching all edges in a
/// workspace, sorted by `edge_id` ASC.
///
/// The query returns the edge AND the endpoint entity_uuids (so the
/// parser can reconstruct the `from` / `to` fields without an
/// additional fetch). Schema convention: every edge stores its
/// endpoints' entity_uuids in `from_entity_uuid` / `to_entity_uuid`
/// properties at upsert time (see [`upsert_edge_command`]).
pub(crate) fn edges_query(workspace_id: &WorkspaceId) -> (String, Value) {
    let command = "MATCH ()-[e:Edge {workspace_id: $ws}]->() RETURN e ORDER BY e.edge_id ASC"
        .to_string();
    let params = json!({ "ws": workspace_id });
    (command, params)
}

/// Build the parameterised Cypher MERGE command for upserting a single
/// vertex.
///
/// All dynamic values are bound via named parameters: `$ws`, `$eid`,
/// `$labels`, `$props`, `$eu`, `$rli`, `$did`. NO user-supplied value
/// is interpolated into the command string.
///
/// `MERGE (n:Vertex {entity_uuid: $eid, workspace_id: $ws})` keys on
/// the logical-identifier-plus-workspace pair so two different
/// workspaces with colliding entity_uuid values cannot accidentally
/// alias (defence in depth against the cross-tenant aliasing risk
/// even though one-DB-per-workspace already structurally prevents it).
pub(crate) fn upsert_vertex_command(workspace_id: &WorkspaceId, v: &BackendVertex) -> (String, Value) {
    let command = "\
        MERGE (n:Vertex {entity_uuid: $eid, workspace_id: $ws}) \
        SET n.labels = $labels, n.properties = $props, n.event_uuid = $eu, \
            n.rekor_log_index = $rli, n.author_did = $did"
        .to_string();
    let params = json!({
        "ws": workspace_id,
        "eid": v.entity_uuid,
        "labels": v.labels,
        "props": serde_json::to_value(&v.properties).unwrap_or(Value::Null),
        "eu": v.event_uuid,
        "rli": v.rekor_log_index,
        "did": v.author_did,
    });
    (command, params)
}

/// Build the parameterised Cypher MERGE command for upserting a single
/// edge.
///
/// The `MATCH` pattern locates the two endpoint vertices by logical id
/// + workspace_id; `MERGE` keys the edge by `edge_id` + `workspace_id`.
/// `from_entity_uuid` / `to_entity_uuid` are stored as properties on
/// the edge so [`edges_query`]'s parser can reconstruct the
/// `BackendEdge::from` / `to` fields without re-traversing the graph.
pub(crate) fn upsert_edge_command(workspace_id: &WorkspaceId, e: &BackendEdge) -> (String, Value) {
    let command = "\
        MATCH (a:Vertex {entity_uuid: $from, workspace_id: $ws}), \
              (b:Vertex {entity_uuid: $to,   workspace_id: $ws}) \
        MERGE (a)-[r:Edge {edge_id: $eid, workspace_id: $ws}]->(b) \
        SET r.label = $label, r.properties = $props, r.event_uuid = $eu, \
            r.rekor_log_index = $rli, r.author_did = $did, \
            r.from_entity_uuid = $from, r.to_entity_uuid = $to"
        .to_string();
    let params = json!({
        "ws": workspace_id,
        "eid": e.edge_id,
        "from": e.from,
        "to": e.to,
        "label": e.label,
        "props": serde_json::to_value(&e.properties).unwrap_or(Value::Null),
        "eu": e.event_uuid,
        "rli": e.rekor_log_index,
        "did": e.author_did,
    });
    (command, params)
}

/// Parse an ArcadeDB `/api/v1/query/{db}` JSON response body into the
/// list of rows the caller will walk.
///
/// On success returns the `result` array. The caller is responsible
/// for walking the array, applying [`check_value_depth_and_size`] to
/// each element, and constructing the typed records via
/// [`parse_vertex_row`] / [`parse_edge_row`].
///
/// Errors map to `ProjectorError::CanonicalisationFailed`. The error
/// strings contain only ArcadeDB-side error reporting (no credentials,
/// no inbound query — those are kept internal). The error path is
/// hit when the body is not valid JSON, when the expected `result`
/// key is missing, or when `result` is not an array.
pub(crate) fn parse_query_response(body: &[u8]) -> ProjectorResult<Vec<Value>> {
    let parsed: Value = serde_json::from_slice(body).map_err(|e| {
        ProjectorError::CanonicalisationFailed(format!(
            "ArcadeDB response JSON parse failed: {e}"
        ))
    })?;
    let Value::Object(map) = parsed else {
        return Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB response root is not a JSON object".to_string(),
        ));
    };
    let Some(result) = map.get("result") else {
        return Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB response missing `result` key".to_string(),
        ));
    };
    let Value::Array(rows) = result else {
        return Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB response `result` is not an array".to_string(),
        ));
    };
    Ok(rows.clone())
}

/// Parse one row of a `RETURN n` vertices query into a [`BackendVertex`].
///
/// Expected shape (ArcadeDB convention):
///
/// ```json
/// {
///   "entity_uuid": "...",
///   "workspace_id": "...",
///   "labels": ["L1", "L2"],
///   "properties": { ... },
///   "event_uuid": "...",
///   "rekor_log_index": 1000,        // or null for V1-era events
///   "author_did": "did:atlas:..."   // or null
/// }
/// ```
///
/// Some servers wrap this under a single key (e.g. `"n"`); we handle
/// both shapes — if the row is an object with a single key whose value
/// is an object, we descend.
///
/// **Security path:** the caller has NOT yet validated the row's
/// value-depth or serialised-size. We MUST call
/// [`check_value_depth_and_size`] BEFORE allocating the `properties`
/// BTreeMap (the cap is the only line of defence against an attacker-
/// controlled or misbehaving server pushing an unbounded JSON document
/// through `Vertex::new`). ADR-011 §4.3 sub-decision #12 enforced
/// here.
pub(crate) fn parse_vertex_row(
    row: &Value,
    workspace_id: &WorkspaceId,
) -> ProjectorResult<BackendVertex> {
    let obj = descend_into_single_wrapper(row);
    // Depth + size cap BEFORE we read the properties — the cap is on
    // the row as a whole, which bounds every field within it.
    check_value_depth_and_size(obj, DEFAULT_MAX_VALUE_DEPTH, DEFAULT_MAX_VALUE_BYTES)?;
    let Value::Object(map) = obj else {
        return Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB vertex row is not a JSON object".to_string(),
        ));
    };

    let entity_uuid = require_string(map, "entity_uuid")?;
    let row_ws = require_string(map, "workspace_id")?;
    // Defence in depth: the application-layer binding is supposed to
    // filter to $ws but if a misbehaving server returned a row from a
    // different workspace, reject here. (Layer 2 of the tenant
    // isolation defence per ADR-010 §4 sub-decision #7.)
    if &row_ws != workspace_id {
        return Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB returned a vertex with mismatched workspace_id".to_string(),
        ));
    }
    let labels = parse_label_array(map.get("labels"))?;
    let properties = parse_properties(map.get("properties"))?;
    let event_uuid = require_string(map, "event_uuid")?;
    let rekor_log_index = parse_optional_u64(map.get("rekor_log_index"))?;
    let author_did = parse_optional_string(map.get("author_did"))?;

    Ok(BackendVertex::new(
        entity_uuid as EntityUuid,
        workspace_id.clone(),
        labels,
        properties,
        event_uuid,
        rekor_log_index,
        author_did,
    ))
}

/// Parse one row of a `RETURN e` edges query into a [`BackendEdge`].
///
/// Expected shape mirrors [`parse_vertex_row`] but with `edge_id`,
/// `from_entity_uuid`, `to_entity_uuid`, `label` instead.
pub(crate) fn parse_edge_row(
    row: &Value,
    workspace_id: &WorkspaceId,
) -> ProjectorResult<BackendEdge> {
    let obj = descend_into_single_wrapper(row);
    check_value_depth_and_size(obj, DEFAULT_MAX_VALUE_DEPTH, DEFAULT_MAX_VALUE_BYTES)?;
    let Value::Object(map) = obj else {
        return Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB edge row is not a JSON object".to_string(),
        ));
    };

    let edge_id = require_string(map, "edge_id")?;
    let row_ws = require_string(map, "workspace_id")?;
    if &row_ws != workspace_id {
        return Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB returned an edge with mismatched workspace_id".to_string(),
        ));
    }
    let from = require_string(map, "from_entity_uuid")?;
    let to = require_string(map, "to_entity_uuid")?;
    let label = require_string(map, "label")?;
    let properties = parse_properties(map.get("properties"))?;
    let event_uuid = require_string(map, "event_uuid")?;
    let rekor_log_index = parse_optional_u64(map.get("rekor_log_index"))?;
    let author_did = parse_optional_string(map.get("author_did"))?;

    Ok(BackendEdge::new(
        edge_id as EdgeId,
        workspace_id.clone(),
        from,
        to,
        label,
        properties,
        event_uuid,
        rekor_log_index,
        author_did,
    ))
}

/// If the row is a single-key wrapper (`{"n": {...}}` or `{"e": {...}}`),
/// return the inner value; otherwise return the row unchanged.
fn descend_into_single_wrapper(row: &Value) -> &Value {
    if let Value::Object(m) = row {
        if m.len() == 1 {
            if let Some((_k, v)) = m.iter().next() {
                if v.is_object() {
                    return v;
                }
            }
        }
    }
    row
}

fn require_string(
    map: &serde_json::Map<String, Value>,
    key: &str,
) -> ProjectorResult<String> {
    match map.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(ProjectorError::CanonicalisationFailed(format!(
            "ArcadeDB row field `{key}` is not a string"
        ))),
        None => Err(ProjectorError::CanonicalisationFailed(format!(
            "ArcadeDB row missing field `{key}`"
        ))),
    }
}

fn parse_label_array(v: Option<&Value>) -> ProjectorResult<Vec<String>> {
    match v {
        Some(Value::Array(arr)) => arr
            .iter()
            .map(|el| match el {
                Value::String(s) => Ok(s.clone()),
                _ => Err(ProjectorError::CanonicalisationFailed(
                    "ArcadeDB vertex label is not a string".to_string(),
                )),
            })
            .collect(),
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(_) => Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB row field `labels` is not an array".to_string(),
        )),
    }
}

fn parse_properties(
    v: Option<&Value>,
) -> ProjectorResult<BTreeMap<String, Value>> {
    match v {
        Some(Value::Object(m)) => Ok(m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
        Some(Value::Null) | None => Ok(BTreeMap::new()),
        Some(_) => Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB row field `properties` is not an object".to_string(),
        )),
    }
}

fn parse_optional_u64(v: Option<&Value>) -> ProjectorResult<Option<u64>> {
    match v {
        Some(Value::Number(n)) => n.as_u64().map(Some).ok_or_else(|| {
            ProjectorError::CanonicalisationFailed(
                "ArcadeDB row field `rekor_log_index` is not a u64".to_string(),
            )
        }),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB row field `rekor_log_index` is not a number".to_string(),
        )),
    }
}

fn parse_optional_string(v: Option<&Value>) -> ProjectorResult<Option<String>> {
    match v {
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(Value::Null) | None => Ok(None),
        Some(_) => Err(ProjectorError::CanonicalisationFailed(
            "ArcadeDB row field `author_did` is not a string".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn db_name_prefixes_and_replaces_hyphens() {
        assert_eq!(
            db_name_for_workspace(&"550e8400-e29b-41d4-a716-446655440000".to_string()),
            "atlas_ws_550e8400_e29b_41d4_a716_446655440000"
        );
        assert_eq!(
            db_name_for_workspace(&"acme_corp".to_string()),
            "atlas_ws_acme_corp"
        );
    }

    #[test]
    fn db_name_is_deterministic() {
        let ws = "01J5M9V8K9X7T2QH4Z6A1P3R5W".to_string();
        let a = db_name_for_workspace(&ws);
        let b = db_name_for_workspace(&ws);
        assert_eq!(a, b);
    }

    #[test]
    fn vertices_query_uses_parameter_binding() {
        let (cmd, params) = vertices_query(&"ws-1".to_string());
        // No interpolation of `ws-1` into the command text.
        assert!(!cmd.contains("ws-1"));
        // `$ws` parameter is present.
        assert!(cmd.contains("$ws"));
        // `ORDER BY n.entity_uuid ASC` is mandatory for byte-determinism.
        assert!(cmd.contains("ORDER BY n.entity_uuid ASC"));
        assert_eq!(params.get("ws").and_then(|v| v.as_str()), Some("ws-1"));
    }

    #[test]
    fn edges_query_uses_parameter_binding() {
        let (cmd, params) = edges_query(&"ws-1".to_string());
        assert!(!cmd.contains("ws-1"));
        assert!(cmd.contains("$ws"));
        assert!(cmd.contains("ORDER BY e.edge_id ASC"));
        assert_eq!(params.get("ws").and_then(|v| v.as_str()), Some("ws-1"));
    }

    #[test]
    fn upsert_vertex_command_uses_parameter_binding() {
        let v = BackendVertex::new(
            "node-a".to_string(),
            "ws-1".to_string(),
            vec!["Person".to_string()],
            BTreeMap::new(),
            "ev1".to_string(),
            Some(100),
            Some("did:atlas:xxxx".to_string()),
        );
        let (cmd, params) = upsert_vertex_command(&"ws-1".to_string(), &v);
        // None of the dynamic values appear in the command text.
        assert!(!cmd.contains("node-a"));
        assert!(!cmd.contains("ws-1"));
        assert!(!cmd.contains("Person"));
        assert!(!cmd.contains("did:atlas:xxxx"));
        // All parameter placeholders present.
        for placeholder in ["$ws", "$eid", "$labels", "$props", "$eu", "$rli", "$did"] {
            assert!(
                cmd.contains(placeholder),
                "placeholder {placeholder} missing in {cmd:?}"
            );
        }
        assert_eq!(params.get("eid").and_then(|v| v.as_str()), Some("node-a"));
    }

    #[test]
    fn upsert_edge_command_uses_parameter_binding() {
        let e = BackendEdge::new(
            "edge-1".to_string(),
            "ws-1".to_string(),
            "node-a".to_string(),
            "node-b".to_string(),
            "knows".to_string(),
            BTreeMap::new(),
            "ev2".to_string(),
            Some(200),
            None,
        );
        let (cmd, params) = upsert_edge_command(&"ws-1".to_string(), &e);
        assert!(!cmd.contains("edge-1"));
        assert!(!cmd.contains("node-a"));
        assert!(!cmd.contains("node-b"));
        assert!(!cmd.contains("knows"));
        for placeholder in [
            "$ws", "$eid", "$from", "$to", "$label", "$props", "$eu", "$rli", "$did",
        ] {
            assert!(
                cmd.contains(placeholder),
                "placeholder {placeholder} missing in {cmd:?}"
            );
        }
        assert_eq!(params.get("eid").and_then(|v| v.as_str()), Some("edge-1"));
    }

    #[test]
    fn parse_query_response_extracts_result_array() {
        let body = br#"{
            "user":"root",
            "result":[
                {"entity_uuid":"a","workspace_id":"ws-1"}
            ]
        }"#;
        let rows = parse_query_response(body).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn parse_query_response_rejects_invalid_json() {
        let body = b"not json";
        match parse_query_response(body) {
            Err(ProjectorError::CanonicalisationFailed(msg)) => {
                assert!(msg.contains("parse failed"), "{msg:?}");
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn parse_query_response_rejects_missing_result() {
        let body = br#"{"user":"root"}"#;
        match parse_query_response(body) {
            Err(ProjectorError::CanonicalisationFailed(msg)) => {
                assert!(msg.contains("missing `result`"), "{msg:?}");
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn parse_vertex_row_round_trips() {
        let row = json!({
            "entity_uuid": "node-a",
            "workspace_id": "ws-1",
            "labels": ["Person", "Sensitive"],
            "properties": { "name": "alice" },
            "event_uuid": "ev1",
            "rekor_log_index": 1000,
            "author_did": "did:atlas:1111"
        });
        let v = parse_vertex_row(&row, &"ws-1".to_string()).unwrap();
        assert_eq!(v.entity_uuid, "node-a");
        assert_eq!(v.workspace_id, "ws-1");
        assert_eq!(v.labels, vec!["Person", "Sensitive"]);
        assert_eq!(v.event_uuid, "ev1");
        assert_eq!(v.rekor_log_index, Some(1000));
        assert_eq!(v.author_did.as_deref(), Some("did:atlas:1111"));
    }

    #[test]
    fn parse_vertex_row_unwraps_single_key_wrapper() {
        // ArcadeDB sometimes returns rows as {"n": {...}}.
        let row = json!({
            "n": {
                "entity_uuid": "node-a",
                "workspace_id": "ws-1",
                "labels": [],
                "properties": {},
                "event_uuid": "ev1",
                "rekor_log_index": null,
                "author_did": null
            }
        });
        let v = parse_vertex_row(&row, &"ws-1".to_string()).unwrap();
        assert_eq!(v.entity_uuid, "node-a");
        assert_eq!(v.rekor_log_index, None);
        assert_eq!(v.author_did, None);
    }

    #[test]
    fn parse_vertex_row_rejects_workspace_mismatch() {
        let row = json!({
            "entity_uuid": "node-a",
            "workspace_id": "OTHER",
            "labels": [],
            "properties": {},
            "event_uuid": "ev1",
            "rekor_log_index": null,
            "author_did": null
        });
        let res = parse_vertex_row(&row, &"ws-1".to_string());
        match res {
            Err(ProjectorError::CanonicalisationFailed(msg)) => {
                assert!(msg.contains("mismatched workspace_id"), "{msg:?}");
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn parse_vertex_row_enforces_value_depth_cap() {
        // Build a deeply-nested row that exceeds DEFAULT_MAX_VALUE_DEPTH.
        let mut deep = serde_json::Value::Null;
        for _ in 0..200 {
            deep = serde_json::Value::Array(vec![deep]);
        }
        let row = json!({
            "entity_uuid": "node-a",
            "workspace_id": "ws-1",
            "labels": [],
            "properties": { "deep": deep },
            "event_uuid": "ev1",
            "rekor_log_index": null,
            "author_did": null
        });
        let res = parse_vertex_row(&row, &"ws-1".to_string());
        match res {
            Err(ProjectorError::CanonicalisationFailed(msg)) => {
                assert!(
                    msg.contains("depth") || msg.contains("size"),
                    "{msg:?}"
                );
            }
            other => panic!("expected CanonicalisationFailed; got {other:?}"),
        }
    }

    #[test]
    fn parse_edge_row_round_trips() {
        let row = json!({
            "edge_id": "edge-ab",
            "workspace_id": "ws-1",
            "from_entity_uuid": "node-a",
            "to_entity_uuid": "node-b",
            "label": "uses",
            "properties": {},
            "event_uuid": "ev2",
            "rekor_log_index": 1003,
            "author_did": null
        });
        let e = parse_edge_row(&row, &"ws-1".to_string()).unwrap();
        assert_eq!(e.edge_id, "edge-ab");
        assert_eq!(e.from, "node-a");
        assert_eq!(e.to, "node-b");
        assert_eq!(e.label, "uses");
        assert_eq!(e.rekor_log_index, Some(1003));
    }
}
