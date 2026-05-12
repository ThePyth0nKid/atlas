//! Atlas trace bundle format (V1).
//!
//! This is the wire-format that flows from Atlas-Server → Verifier (CLI/WASM/library).
//! It must be self-contained so that an air-gapped auditor can verify it.

use serde::{Deserialize, Serialize};

use crate::witness::WitnessSig;

/// The full trace bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtlasTrace {
    /// Schema version, must match `atlas-trace-v1`.
    pub schema_version: String,

    /// ISO-8601 timestamp when this bundle was generated server-side.
    pub generated_at: String,

    /// Workspace identifier.
    pub workspace_id: String,

    /// blake3 hex hash of the pubkey-bundle the events were signed against.
    pub pubkey_bundle_hash: String,

    /// All signed events in this bundle.
    pub events: Vec<AtlasEvent>,

    /// Current DAG-tip hashes the server claims.
    /// Verifier computes the actual tips from `events` and may diff.
    pub dag_tips: Vec<String>,

    /// Anchor entries (Sigstore Rekor inclusion proofs).
    #[serde(default)]
    pub anchors: Vec<AnchorEntry>,

    /// Anchor-chain extension witness (V1.7). When present, binds this
    /// trace to a monotonic sequence of anchor batches issued for the
    /// workspace, defending against post-hoc rewriting of past anchored
    /// state. Absent for V1.5 and V1.6 trace bundles; lenient mode
    /// passes (matching the V1.5 "no claim is fine" rule), strict mode
    /// (`VerifyOptions::require_anchor_chain`) demands a present, valid
    /// chain. See `AnchorChain` for the round-trip story.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_chain: Option<AnchorChain>,

    /// Cedar policies in scope at bundle generation time.
    /// Each policy is itself an event in `events` — this is just the index.
    #[serde(default)]
    pub policies: Vec<String>,

    /// Optional filters applied when generating the bundle.
    #[serde(default)]
    pub filters: Option<TraceFilters>,
}

/// One signed event.
///
/// **V2-α Welle 1 schema addition:** the optional `author_did` field
/// names the agent instance that produced this event, as a W3C-DID of
/// the form `did:atlas:<lowercase-hex-32-bytes>`. See `agent_did` module
/// docstring for the full design rationale. When present, `author_did`
/// is canonically bound into the signing input alongside `kid` (Phase 2
/// Security H-1), providing cross-agent-replay defence in addition to
/// V1's cross-workspace-replay defence. V1 events without `author_did`
/// remain valid forever; V2-α events MAY carry one.
///
/// **Wire-compat note (by design):** the struct retains
/// `#[serde(deny_unknown_fields)]`. A V1.0 verifier reading a V2-α
/// event whose `author_did` field is present will reject with
/// `unknown_field("author_did")`. This is intentional: V2 = major
/// bump. The version bump itself is deferred to the end of the
/// V2-α welle bundle per Welle 1 plan-doc §"Decisions".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtlasEvent {
    /// ULID for this event.
    pub event_id: String,

    /// blake3 hex hash of the canonical signing-input.
    pub event_hash: String,

    /// Parent event_hashes (zero or more — DAG).
    #[serde(default)]
    pub parent_hashes: Vec<String>,

    /// Application-level payload as JSON.
    pub payload: serde_json::Value,

    /// Ed25519 signature (V1 simplified format; V2: full COSE_Sign1).
    pub signature: EventSignature,

    /// ISO-8601 timestamp the signer claims.
    pub ts: String,

    /// V2-α optional agent-DID naming the agent instance that produced
    /// this event. Format: `did:atlas:<lowercase-hex-32-bytes>`. When
    /// absent, the event is V1-shaped (workspace attribution only, no
    /// agent attribution). When present, the verifier format-validates
    /// via `agent_did::validate_agent_did` and the DID is canonically
    /// bound into the signing input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_did: Option<String>,
}

/// Simplified V1 signature wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSignature {
    /// Always "EdDSA" in V1.
    pub alg: String,
    /// Key-id: SPIFFE-ID, email, or other resolvable identifier.
    pub kid: String,
    /// Signature bytes, base64url-no-pad encoded.
    pub sig: String,
}

/// What kind of object an `AnchorEntry` refers to.
///
/// Anchors give an offline auditor a third-party witness that some hash
/// existed before time T. We anchor two things:
///   - DAG tips, so an auditor can prove "this trace state existed by T"
///   - the pubkey-bundle hash, so an auditor can prove "this exact key
///     roster was the one in use by T" (defends against post-hoc bundle
///     swaps that would re-validate forged signatures).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    /// `anchored_hash` is one of the trace's `dag_tips` (event_hash).
    DagTip,
    /// `anchored_hash` is the trace's `pubkey_bundle_hash`.
    BundleHash,
}

/// One anchor entry — proof that a specific hash was committed to a
/// transparency log at a specific time, with a Merkle inclusion proof
/// against a signed log checkpoint.
///
/// The anchored hash is canonically bound to a tree position, the tree
/// position is bound to the root via the inclusion proof, and the root
/// is bound to the log identity via the checkpoint signature. An
/// auditor with the pinned log public key can verify all three links
/// without network access.
///
/// The verifier dispatches the per-link checks (leaf-hash construction,
/// Merkle algorithm, checkpoint signature scheme) on the format
/// associated with `log_id` in the trusted-log roster:
/// - `atlas-mock-rekor-v1` (V1.5): blake3 leaf/parent prefixes,
///   Ed25519 over a three-line atlas-mock checkpoint.
/// - `sigstore-rekor-v1` (V1.6): SHA-256 RFC 6962 leaves/parents,
///   ECDSA P-256 over a C2SP signed-note checkpoint. Requires
///   `entry_body_b64` (the canonical Rekor entry body, from which
///   the leaf hash is recomputed) and `tree_id` (used in the
///   signed-note origin line).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorEntry {
    /// What kind of object `anchored_hash` refers to.
    pub kind: AnchorKind,
    /// The hash that was anchored (hex). For `DagTip` this is an event_hash;
    /// for `BundleHash` this is the trace's `pubkey_bundle_hash`.
    pub anchored_hash: String,
    /// Identifier of the transparency log holding this entry. For Sigstore
    /// Rekor this is the hex SHA-256 of the log's DER-SPKI public key. The
    /// verifier uses this to look up the corresponding pinned pubkey and
    /// the format that pubkey signs in.
    pub log_id: String,
    /// 0-indexed leaf position of the entry in the log.
    pub log_index: u64,
    /// Unix seconds the log claims this entry was integrated.
    pub integrated_time: i64,
    /// Merkle inclusion proof against a signed checkpoint.
    pub inclusion_proof: InclusionProof,
    /// V1.6 Sigstore-format only: base64 (standard, padded) of the
    /// canonical Rekor entry body. The verifier recomputes the leaf hash
    /// as SHA-256(0x00 || decoded body) and additionally checks that the
    /// body's hashedrekord `data.hash.value` equals `anchored_hash`.
    /// Absent (None) for the atlas-mock format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_body_b64: Option<String>,
    /// V1.6 Sigstore-format only: Trillian tree-ID this entry was
    /// committed to. Used to reconstruct the C2SP signed-note origin
    /// line `"rekor.sigstore.dev - {tree_id}"`. Absent (None) for the
    /// atlas-mock format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_id: Option<i64>,
}

/// Merkle inclusion proof of a leaf against a signed log checkpoint.
///
/// The proof binds a single leaf hash (derived from `anchored_hash`) to
/// `root_hash` via `hashes` (RFC 6962 §2.1.1 sibling ordering, deepest
/// sibling first). `checkpoint_sig` is the log's signature over the
/// canonical checkpoint bytes; the encoding and signature scheme depend
/// on the parent `AnchorEntry`'s log format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InclusionProof {
    /// Tree size at which inclusion was witnessed.
    pub tree_size: u64,
    /// Hex root hash of the tree at `tree_size`. For Sigstore Rekor v1
    /// this is the hex of the 32-byte SHA-256 root; for atlas-mock-v1
    /// the hex of the 32-byte blake3 root.
    pub root_hash: String,
    /// Hex sibling hashes from leaf to root (RFC 6962 ordering).
    pub hashes: Vec<String>,
    /// Base64-encoded log signature over the canonical checkpoint bytes.
    ///
    /// Format depends on `AnchorEntry`'s log:
    /// - `atlas-mock-rekor-v1` (V1.5): URL-safe base64 (no padding) of a
    ///   raw 64-byte Ed25519 signature. Canonical bytes:
    ///   `atlas_trust_core::anchor::canonical_checkpoint_bytes`.
    /// - `sigstore-rekor-v1` (V1.6): RFC 4648 §4 standard base64 (with
    ///   `=` padding) of `4-byte BE keyID || DER ECDSA P-256 signature`,
    ///   per the C2SP signed-note spec. Canonical bytes:
    ///   `atlas_trust_core::anchor::canonical_checkpoint_bytes_sigstore`.
    pub checkpoint_sig: String,
}

/// Hash-chain head over consecutive anchor batches issued for a
/// workspace (V1.7).
///
/// V1.6 anchors prove "this `dag_tip`/`bundle_hash` was witnessed at
/// time T against a pinned log". They do not prove "the SEQUENCE of
/// witnessed states is consistent — no past state has been silently
/// rewritten". A server could legitimately anchor state A at T₁,
/// later corrupt the trace, then anchor a different state B at T₂,
/// and an auditor with only the most recent `anchors.json` would not
/// notice. V1.7 closes that gap by carrying every anchor batch ever
/// issued for the workspace, cross-linked via `previous_head`.
///
/// **Trust property** — every additional batch_n is bound to all
/// preceding batches via:
/// ```text
/// chain_head_n = blake3("atlas-anchor-chain-v1:" ||
///                       canonical_chain_batch_body(batch_n))
/// ```
/// where `batch_n.previous_head == chain_head_{n-1}` (and zero for
/// the genesis batch). The verifier walks `history[]` in order,
/// recomputes each head locally, and rejects any missing batch,
/// reordered batch, mutated entry list, or mismatched previous_head.
///
/// **Storage discipline** — the issuer is the only writer of
/// `data/{workspace}/anchor-chain.jsonl`; the file is append-only and
/// must NOT be truncated outside an explicit `atlas-signer
/// rotate-chain --confirm` ceremony. Loss of the chain file means the
/// trust property fails for the workspace from that point on. The MCP
/// `atlas_export_bundle` tool reads the file and ships it here for
/// offline verification.
///
/// **Backwards compatibility** — `AtlasTrace.anchor_chain` is
/// `Option`. V1.5 and V1.6 trace bundles do not carry this field and
/// continue to verify in lenient mode. Strict mode
/// (`VerifyOptions::require_anchor_chain = true`) demands a present,
/// valid chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorChain {
    /// All anchor batches issued for this workspace, in issuance
    /// order. Append-only on the wire: earlier entries cannot be
    /// dropped, mutated, or reordered without breaking the chain.
    /// Empty `history` is a malformed `AnchorChain` — strict mode
    /// rejects it; the issuer never emits an empty chain.
    pub history: Vec<AnchorBatch>,

    /// blake3 hex of `chain_head_for(history[history.len() - 1])`.
    /// Convenience field so the verifier can fail fast before walking
    /// the whole history. The verifier recomputes from `history[]`
    /// and compares; this field is NEVER trusted as a verification
    /// shortcut.
    pub head: String,
}

/// One anchor batch in the chain (V1.7).
///
/// A batch records the result of a single `atlas_anchor_bundle`
/// invocation: which entries were issued at what `integrated_time`,
/// indexed sequentially from 0, with each batch carrying the previous
/// batch's `chain_head`. The verifier rejects gaps, reorderings, and
/// mutations to any of these fields by recomputing
/// `chain_head_for(batch)` and asserting `history[i+1].previous_head
/// == chain_head_for(history[i])`.
///
/// Per-entry inclusion proofs in `entries` are checked independently
/// by the existing `verify_anchors` pipeline — the chain layer adds a
/// monotonicity witness on top, it does not replace per-entry
/// verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnchorBatch {
    /// Sequential index, 0 for the genesis batch. The verifier
    /// asserts `history[i].batch_index == i as u64` so any gap, skip,
    /// or duplicate is rejected as a structural violation.
    pub batch_index: u64,

    /// Unix seconds the issuer recorded for this batch — typically
    /// matches the `integrated_time` carried by every entry inside
    /// `entries` (the issuer threads one timestamp through both). An
    /// independent field on the batch lets the chain witness empty
    /// batches in principle, though V1.7 issuers never emit them.
    pub integrated_time: i64,

    /// AnchorEntries issued in this batch, in their original order.
    /// Each entry is verified separately (leaf-hash, inclusion proof,
    /// checkpoint signature) by the standard per-entry pipeline; the
    /// chain extension only commits to their canonical bytes via the
    /// batch's `chain_head`.
    pub entries: Vec<AnchorEntry>,

    /// Hex `chain_head` of the previous batch (64 lowercase hex
    /// chars). The genesis batch carries the all-zero head:
    /// `"0000…0000"` × 32 bytes. The verifier asserts
    /// `history[i].previous_head == chain_head_for(history[i-1])` so
    /// silent rewriting of any past batch breaks the chain.
    pub previous_head: String,

    /// V1.13 witness cosignatures over this batch's chain head.
    ///
    /// CRITICAL: this field is deliberately excluded from
    /// `chain_head_for(batch)` (see `anchor::ChainHeadInput`). A witness
    /// signs OVER the chain head; including witnesses in the head
    /// computation would create infinite regress.
    ///
    /// Pre-V1.13 batches omit the field entirely; `serde(default)` lets
    /// the verifier accept them (empty vec). Post-V1.13 batches with no
    /// commissioned witness still emit no field
    /// (`skip_serializing_if = "Vec::is_empty"`) so the wire shape is
    /// byte-identical to pre-V1.13 for the empty case — preserves
    /// already-issued anchor-chain bytes verbatim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub witnesses: Vec<WitnessSig>,
}

/// Genesis sentinel for `AnchorBatch::previous_head`: 64 ASCII zeros
/// (= 32 zero bytes hex-encoded). Single source of truth so issuer
/// and verifier never disagree on the genesis sentinel.
pub const ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Optional filters applied to a bundle (for narrower audit-export).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceFilters {
    /// Time-range filter.
    #[serde(default)]
    pub period: Option<PeriodFilter>,
    /// System filter (e.g., "CreditScoreV3").
    #[serde(default)]
    pub system: Option<String>,
    /// Specific node-IDs only.
    #[serde(default)]
    pub nodes_subset: Vec<String>,
}

/// A time-range filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeriodFilter {
    /// ISO-8601 start.
    pub start: String,
    /// ISO-8601 end.
    pub end: String,
}

/// Application-level payload variants.
/// We keep this as `serde_json::Value` in `AtlasEvent` for forward-compat,
/// but expose this enum for typed inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AtlasPayload {
    /// New node in the knowledge graph.
    NodeCreate {
        /// the full node data
        node: serde_json::Value,
    },
    /// Update to existing node.
    NodeUpdate {
        /// node id
        node_id: String,
        /// patch object
        patch: serde_json::Value,
    },
    /// New edge between two nodes.
    EdgeCreate {
        /// source node id
        from: String,
        /// destination node id
        to: String,
        /// edge relation type
        relation: String,
    },
    /// RDF-star annotation.
    AnnotationAdd {
        /// triple-subject
        subject: String,
        /// predicate
        predicate: String,
        /// object value
        object: serde_json::Value,
    },
    /// Cedar policy set.
    PolicySet {
        /// Cedar policy as text
        policy_cedar: String,
    },
    /// Anchor reference event (audit log of when an anchor was created).
    AnchorCreated {
        /// What was anchored (dag_tip or bundle_hash)
        kind: AnchorKind,
        /// The hex hash that was anchored
        anchored_hash: String,
        /// Log identifier (hex SHA-256 of log pubkey)
        log_id: String,
        /// Position in the log
        log_index: u64,
    },
}
