//! V2-α Welle 5: `events.jsonl` line-by-line parser.
//!
//! Reads a string containing JSON-Lines-formatted Atlas events and
//! deserialises each into an `AtlasEvent`. The projector consumes
//! the resulting `Vec<AtlasEvent>` to update its in-memory
//! `GraphState` (see `upsert` module).
//!
//! ## Library-only — no file I/O
//!
//! This module operates on `&str` only. Callers (CLI binaries,
//! atlas-signer integrations, future WASM targets) handle file I/O.
//! Keeps `atlas-projector` free of `std::fs` dependencies and
//! friendly to future WASM compilation.
//!
//! ## Out of scope
//!
//! Welle 5 explicitly does NOT re-verify the Ed25519 signatures of
//! the events at replay time — that's the verifier's responsibility
//! (`atlas_trust_core::verify_trace`). Replay assumes the caller has
//! already validated trust properties (or chooses not to, accepting
//! the consequence). Projection over un-verified events is sometimes
//! intentional — e.g. for replay-from-untrusted-source-into-throwaway-state.
//!
//! ## Tolerated input shapes
//!
//! - blank lines (skipped silently)
//! - lines beginning with `//` (treated as comments, skipped silently —
//!   operator-runbook convenience for hand-edited fixture files)
//! - lines with trailing whitespace before the newline
//!
//! ## Rejected input shapes
//!
//! - any non-blank, non-comment line that does not parse as a valid
//!   `AtlasEvent` JSON object (surfaces `ProjectorError::ReplayMalformed`
//!   with 1-indexed line number)

use atlas_trust_core::trace_format::AtlasEvent;

use crate::error::{ProjectorError, ProjectorResult};

/// Parse an `events.jsonl` document into a vector of `AtlasEvent`s.
///
/// Line numbering is 1-indexed for operator-friendly diagnostics
/// (matches typical editor + grep behaviour).
///
/// Returns `Err(ProjectorError::ReplayMalformed { line_number, reason })`
/// on the FIRST malformed event encountered. Welle 5 chooses
/// fail-fast over collect-all-errors because (a) projection of a
/// partially-parsed event stream produces a partial graph state,
/// which is harder to reason about than an early abort, and (b)
/// operators debugging a malformed JSONL typically want to find
/// the FIRST broken line and fix it iteratively.
pub fn parse_events_jsonl(contents: &str) -> ProjectorResult<Vec<AtlasEvent>> {
    let mut events: Vec<AtlasEvent> = Vec::new();
    for (idx, raw_line) in contents.lines().enumerate() {
        let line_number = idx + 1; // 1-indexed
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("//") {
            continue;
        }
        let event: AtlasEvent = serde_json::from_str(line).map_err(|e| {
            ProjectorError::ReplayMalformed {
                line_number,
                reason: e.to_string(),
            }
        })?;
        events.push(event);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn well_formed_event_line(event_id: &str) -> String {
        // Use the canonical AtlasEvent shape with author_did absent
        // (V1-compat). Properties are minimal — replay just needs
        // valid JSON shape, not specific semantic content.
        format!(
            r#"{{"event_id":"{event_id}","event_hash":"deadbeef","parent_hashes":[],"payload":{{"type":"node_create","node":{{"id":"n1"}}}},"signature":{{"alg":"EdDSA","kid":"atlas-anchor:ws","sig":"AAAA"}},"ts":"2026-05-13T10:00:00Z"}}"#
        )
    }

    #[test]
    fn parse_empty_input_returns_empty_vec() {
        let events = parse_events_jsonl("").unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn parse_single_event_line() {
        let line = well_formed_event_line("01HEVENT1");
        let events = parse_events_jsonl(&line).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, "01HEVENT1");
    }

    #[test]
    fn parse_multiple_events() {
        let input = format!(
            "{}\n{}\n{}",
            well_formed_event_line("01HEVENT1"),
            well_formed_event_line("01HEVENT2"),
            well_formed_event_line("01HEVENT3"),
        );
        let events = parse_events_jsonl(&input).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_id, "01HEVENT1");
        assert_eq!(events[2].event_id, "01HEVENT3");
    }

    #[test]
    fn parse_skips_blank_lines() {
        let input = format!(
            "\n{}\n   \n{}\n\n",
            well_formed_event_line("01HEVENT1"),
            well_formed_event_line("01HEVENT2"),
        );
        let events = parse_events_jsonl(&input).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn parse_skips_comment_lines() {
        let input = format!(
            "// header comment\n{}\n// inline comment\n{}\n",
            well_formed_event_line("01HEVENT1"),
            well_formed_event_line("01HEVENT2"),
        );
        let events = parse_events_jsonl(&input).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn parse_rejects_malformed_json_with_line_number() {
        let input = format!(
            "{}\nthis-is-not-json\n{}\n",
            well_formed_event_line("01HEVENT1"),
            well_formed_event_line("01HEVENT2"),
        );
        match parse_events_jsonl(&input) {
            Err(ProjectorError::ReplayMalformed { line_number, .. }) => {
                assert_eq!(line_number, 2, "1-indexed line number expected");
            }
            other => panic!("expected ReplayMalformed; got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_wrong_shape_json_with_line_number() {
        // Valid JSON but not an AtlasEvent
        let input = format!(
            "{}\n{{\"foo\":\"bar\"}}\n",
            well_formed_event_line("01HEVENT1"),
        );
        match parse_events_jsonl(&input) {
            Err(ProjectorError::ReplayMalformed { line_number, .. }) => {
                assert_eq!(line_number, 2);
            }
            other => panic!("expected ReplayMalformed; got {other:?}"),
        }
    }
}
