//! V1.7 — anchor-chain extension on the issuer side.
//!
//! Persists the per-workspace `anchor-chain.jsonl` file: one
//! [`AnchorBatch`] per line, append-only-from-this-process. Each
//! invocation of the `anchor` subcommand with `--chain-path <path>`
//! reads the existing chain, computes the new `previous_head` (or the
//! genesis sentinel for batch 0), validates the existing tail against
//! the verifier's `verify_anchor_chain`, then atomically rewrites the
//! file with one extra row.
//!
//! Why "rewrite with one extra row" instead of `O_APPEND`?
//!
//! A crash partway through `O_APPEND` leaves a torn line, which
//! [`serde_json::from_str`] rejects on the next read — and after that
//! point the chain is bricked. Read-all + tmp + rename is atomic at
//! `rename(2)` granularity (POSIX) / `MoveFileExW` granularity
//! (Windows when source and target share a directory), so a concurrent
//! reader sees either the pre-state or the post-state, never a torn
//! line. The cost is O(N) bytes copied per append, which is fine: a
//! workspace's chain is dozens to hundreds of batches, each a few
//! kilobytes.
//!
//! Trust property: **the signer is the SOLE writer.** The MCP TS-side
//! reads this file but never modifies it. The signer never reads the
//! file outside this module. If a future deployment introduces
//! concurrent writers (multi-process MCP), this function must grow a
//! file lock — currently a TOCTOU window exists between the read and
//! the rename.
//!
//! Adversary defence: before extending, we re-run `verify_anchor_chain`
//! on the existing tail. If the existing chain is corrupt (a past
//! batch was tampered with), `extend_chain_with_batch` REFUSES to
//! write — appending to a corrupt chain would lock the tampering into
//! the new `previous_head`, making the signer complicit. The operator
//! must repair (or re-issue from genesis with operator approval) before
//! the next batch can be written.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use atlas_trust_core::anchor::{chain_head_for, verify_anchor_chain};
use atlas_trust_core::trace_format::{
    AnchorBatch, AnchorChain, AnchorEntry, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD,
};

/// Read the existing anchor-chain at `chain_path`, build a new
/// [`AnchorBatch`] committing the freshly-issued `entries` plus
/// `integrated_time`, and atomically persist the result.
///
/// Returns the newly-appended `AnchorBatch` so the caller can surface
/// summary fields (batch_index, previous_head) to operators.
///
/// File semantics:
///   * Each line is a JSON-serialised `AnchorBatch`. Trailing newline
///     after every line, including the last.
///   * If the file does not exist: the new batch is the genesis batch
///     (`batch_index = 0`, `previous_head = "0…0"` × 32 bytes hex).
///   * If the file exists: the new batch's `previous_head` is
///     `chain_head_for(<existing tail>)`, locking in every prior row
///     before the issuer commits one more.
///
/// On-disk canonicalization: existing lines are re-serialised through
/// `serde_json::to_string` on every write. The canonical-by-construction
/// JSON is what the next `previous_head` is computed over (because
/// `chain_head_for` re-serialises through the same `serde_json::to_value
/// → canonical_json_bytes` pipeline anyway). Net effect: an operator
/// who hand-edits whitespace or field-order in `anchor-chain.jsonl`
/// will see those edits silently normalised on the next extend, not
/// preserved verbatim. Reformatting is therefore safe; field-value
/// edits remain detected because `verify_anchor_chain` recomputes the
/// chain heads from the parsed values.
///
/// Concurrency: the signer is the SOLE writer. `extend_chain_with_batch`
/// MUST NOT be called concurrently for the same `chain_path` — there
/// is no file lock, and a TOCTOU window between the read and the
/// rename would let two writers both think they are appending batch
/// `N`, ending up with one of them silently overwritten. Single-process
/// MCP avoids this; multi-process deployment must add a `flock`-style
/// guard before this call.
///
/// `integrated_time` is caller-supplied and NOT checked for monotonicity
/// against the existing tail. Backdating produces a chain whose batch
/// timestamps go non-monotonically; the cryptographic linking still
/// holds, but auditors cannot rely on chain-internal timestamps as a
/// witness of "before-X". Authoritative time witness is Rekor's own
/// `integrated_time` per entry, not the on-batch field.
///
/// Errors:
///   * `extend_chain_with_batch: entries must not be empty`: empty
///     batches are structurally valid but waste a `batch_index` and
///     produce no anchoring evidence; the issuer refuses.
///   * `existing chain is corrupt`: `verify_anchor_chain` rejected the
///     read-back batches. The signer refuses to extend.
///   * `parse anchor-chain line N`: a line is not valid `AnchorBatch`
///     JSON. Same refusal — the operator must repair.
///   * `anchor-chain exceeded u64::MAX batches`: structural impossibility
///     in practice, but the cast is checked rather than `as u64`.
///   * `read|write|fsync|rename`: filesystem-level failures, surfaced
///     verbatim with the affected path.
pub fn extend_chain_with_batch(
    chain_path: &Path,
    entries: &[AnchorEntry],
    integrated_time: i64,
) -> Result<AnchorBatch, String> {
    if entries.is_empty() {
        // Phantom batches consume a batch_index, advance the chain head,
        // and produce no anchoring witness. Refuse rather than commit
        // an empty row that downstream cross-coverage would surface as
        // a confusing "no anchors covered the chain entry" mismatch.
        return Err(
            "extend_chain_with_batch: entries must not be empty".to_string(),
        );
    }

    let existing = read_existing_batches(chain_path)?;

    // Defence-in-depth: refuse to extend a corrupt chain. Linking a new
    // `previous_head` onto a tampered tail would lock the tampering in,
    // making the signer complicit in past rewrites. Operators repair
    // out-of-band.
    if !existing.is_empty() {
        let recomputed_tip = chain_head_for(existing.last().unwrap())
            .map_err(|e| format!("compute existing tail head: {e}"))?;
        let snapshot = AnchorChain {
            history: existing.clone(),
            // V1.13 wave-C-2: `chain_head_for` returns `ChainHeadHex`;
            // unwrap to the wire-side `String` for serde-compatible
            // assignment to `AnchorChain.head`.
            head: recomputed_tip.into_inner(),
        };
        let outcome = verify_anchor_chain(&snapshot);
        if !outcome.ok {
            return Err(format!(
                "refusing to extend corrupt anchor-chain at {} \
                 ({} batches walked, {} errors): {}",
                chain_path.display(),
                outcome.batches_walked,
                outcome.errors.len(),
                outcome.errors.join("; "),
            ));
        }
    }

    let (batch_index, previous_head) = match existing.last() {
        Some(last) => {
            let head = chain_head_for(last)
                .map_err(|e| format!("compute previous_head: {e}"))?;
            // Checked cast: documents the upper bound rather than relying
            // on `as u64` to silently saturate. `usize::MAX` ≤ `u64::MAX`
            // on every supported target today, but the explicit conversion
            // is the right idiom for an index that ends up in canonical
            // bytes the verifier hashes.
            let next_index = u64::try_from(existing.len()).map_err(|_| {
                "anchor-chain exceeded u64::MAX batches; structural impossibility tripped"
                    .to_string()
            })?;
            // `previous_head` on the wire is a String; unwrap the
            // V1.13 `ChainHeadHex` newtype here.
            (next_index, head.into_inner())
        }
        None => (0u64, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD.to_string()),
    };

    let new_batch = AnchorBatch {
        batch_index,
        integrated_time,
        entries: entries.to_vec(),
        previous_head,
        // V1.13: issuer-side never populates witnesses — that is the
        // sidecar `atlas-witness` binary's job (Trust-Domain separation).
        // Empty vec keeps the batch deserialisable on pre-V1.13 verifiers
        // for now; once a witness ceremony commissions an entry into the
        // pinned roster, an out-of-process step appends WitnessSig values
        // before the batch is shipped to auditors.
        witnesses: Vec::new(),
    };

    // Sanity: the new batch must produce a usable head. If
    // `chain_head_for` fails here it would also fail at verify time,
    // and we'd rather refuse to write than ship a bad row.
    chain_head_for(&new_batch)
        .map_err(|e| format!("compute new batch head: {e}"))?;

    // Build the full file payload: existing lines (preserved verbatim
    // by re-serialisation through serde — same canonical-by-construction
    // path as the verifier) + the new line. Trailing newline after every
    // line, including the last, so a future append never has to repair
    // the boundary.
    let mut payload: Vec<u8> = Vec::new();
    for batch in existing.iter().chain(std::iter::once(&new_batch)) {
        let line = serde_json::to_string(batch)
            .map_err(|e| format!("serialise batch[{}]: {}", batch.batch_index, e))?;
        payload.extend_from_slice(line.as_bytes());
        payload.push(b'\n');
    }

    write_atomically(chain_path, &payload)?;
    Ok(new_batch)
}

/// Read the JSONL file at `chain_path` and parse every non-empty line
/// as an `AnchorBatch`. Missing file returns an empty vec (genesis case).
fn read_existing_batches(chain_path: &Path) -> Result<Vec<AnchorBatch>, String> {
    let bytes = match fs::read(chain_path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(format!(
                "read anchor-chain {}: {e}",
                chain_path.display(),
            ));
        }
    };
    parse_chain_jsonl(&bytes)
        .map_err(|e| format!("{e} at {}", chain_path.display()))
}

/// Parse JSONL bytes — one `AnchorBatch` per non-empty line — into a
/// `Vec<AnchorBatch>`. Empty or whitespace-only input yields an empty
/// vec.
///
/// The `serde(deny_unknown_fields)` attribute on `AnchorBatch` and
/// `AnchorEntry` is the actual schema gate here: an attacker who slips
/// an extra field into the chain file is rejected at parse time, not
/// silently passed through to the verifier.
///
/// Error messages reference the BATCH NUMBER (1-indexed, counting only
/// non-blank lines), not the raw split-token index. A trailing newline
/// produces an empty token at the end of `split('\n')`; reporting the
/// raw token index would tell an operator "line 4" for the second
/// batch in a normally-formatted file, which makes the bad row
/// inconvenient to find in an editor.
fn parse_chain_jsonl(bytes: &[u8]) -> Result<Vec<AnchorBatch>, String> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|e| format!("anchor-chain bytes are not utf-8: {e}"))?;
    let mut out = Vec::new();
    let mut batch_num = 0usize;
    for raw in text.split('\n') {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        batch_num += 1;
        let batch: AnchorBatch = serde_json::from_str(line).map_err(|e| {
            format!("parse anchor-chain line {batch_num}: {e}")
        })?;
        out.push(batch);
    }
    Ok(out)
}

/// Read JSONL bytes from an `anchor-chain.jsonl` file, recompute the
/// chain head, validate the full chain, and return a wire-format
/// `AnchorChain` ready to ship in `AtlasTrace.anchor_chain`.
///
/// This is the export-side counterpart to `extend_chain_with_batch`:
/// the issuer writes one batch per anchoring invocation, the export
/// path reads the accumulated history and rebuilds the `AnchorChain`
/// envelope. Single-source canonicalization holds because the head is
/// computed from `chain_head_for` (same path the verifier runs); the
/// MCP TS-side never owns this calculation.
///
/// Defence-in-depth: `verify_anchor_chain` is run before returning. A
/// chain that is corrupt at export time fails inside the operator's
/// domain rather than leaking to an offline auditor as an opaque ✗
/// result. The same outcome would eventually surface at the verifier,
/// but failing earlier means the operator can repair without external
/// pressure.
///
/// Empty `jsonl_bytes` yields an `Err` — `AnchorChain` requires at
/// least one batch to be a meaningful witness. The TS caller is
/// expected to skip the chain-export call when the on-disk file is
/// missing or empty.
pub fn build_chain_export_from_jsonl(jsonl_bytes: &[u8]) -> Result<AnchorChain, String> {
    let history = parse_chain_jsonl(jsonl_bytes)?;
    if history.is_empty() {
        return Err("chain-export: anchor-chain JSONL is empty".to_string());
    }
    let head = chain_head_for(history.last().unwrap())
        .map_err(|e| format!("chain-export: compute head: {e}"))?;
    // V1.13 wave-C-2: unwrap the typed ChainHeadHex to the wire-side String
    // for the AnchorChain.head field (kept String-typed for serde compat).
    let chain = AnchorChain {
        history,
        head: head.into_inner(),
    };

    let outcome = verify_anchor_chain(&chain);
    if !outcome.ok {
        return Err(format!(
            "chain-export: chain verification failed ({} batches walked, {} errors): {}",
            outcome.batches_walked,
            outcome.errors.len(),
            outcome.errors.join("; "),
        ));
    }
    Ok(chain)
}

/// Atomically replace the file at `target` with `bytes`.
///
/// 1. Writes to `<target>.tmp-<pid>-<micros>` in the same directory
///    (so `rename(2)` is same-filesystem and therefore atomic).
/// 2. fsyncs the tmp file to flush bytes to stable storage before the
///    rename — without this, a power loss between rename and flush can
///    leave an empty new file on some filesystems.
/// 3. On Windows, if `target` is an existing symlink, removes it
///    explicitly before the rename. `MoveFileExW` with
///    `MOVEFILE_REPLACE_EXISTING` follows symlinks (writes through the
///    link), unlike POSIX `rename(2)` which replaces the dirent itself.
///    Failing to handle this would let an attacker who pre-creates the
///    chain file as a symlink redirect the issuer's writes outside the
///    workspace data dir.
/// 4. Renames over the target.
/// 5. fsyncs the parent directory on Unix so the dirent update is
///    durable after a power loss. Without this, ext4/XFS in default
///    `data=ordered` mode can leave the rename committed in cache but
///    lost from disk on crash — the file would appear truncated or
///    absent on the next mount, bricking the chain. Windows commits
///    the dirent through `MoveFileExW` directly; no explicit dir
///    fsync is needed there (and `File::open(dir)` requires
///    `FILE_FLAG_BACKUP_SEMANTICS` which `OpenOptions` does not expose
///    cross-platform).
///
/// If the parent directory does not exist, it is created with default
/// permissions. The signer is launched per-workspace by the MCP host;
/// the workspace dir is created lazily here so a fresh workspace can
/// produce its first chain row without a separate setup step.
fn write_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("anchor-chain path has no parent: {}", target.display()))?;
    if !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create parent {}: {e}", parent.display()))?;
    }

    // `unwrap_or_default()` would silently produce `.tmp-<pid>` in the
    // parent, which would collide across concurrent invocations on a
    // structurally-impossible path. Surface a clear error instead.
    let base = target.file_name().ok_or_else(|| {
        format!(
            "anchor-chain path has no file_name component: {}",
            target.display(),
        )
    })?;
    let mut tmp_name = base.to_owned();
    tmp_name.push(format!(".tmp-{}-{}", std::process::id(), now_micros()));
    let mut tmp_path = PathBuf::from(parent);
    tmp_path.push(tmp_name);

    {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .map_err(|e| format!("open tmp {}: {e}", tmp_path.display()))?;
        f.write_all(bytes)
            .map_err(|e| format!("write tmp {}: {e}", tmp_path.display()))?;
        f.sync_all()
            .map_err(|e| format!("fsync tmp {}: {e}", tmp_path.display()))?;
    }

    // Windows-only symlink defence. On POSIX, `rename(2)` replaces the
    // directory entry verbatim regardless of whether `target` is a
    // symlink, so the symlink target is untouched. On Windows,
    // `MoveFileExW` follows the symlink and overwrites the pointed-to
    // file; we explicitly remove the symlink first.
    #[cfg(windows)]
    {
        if let Ok(meta) = fs::symlink_metadata(target) {
            if meta.file_type().is_symlink() {
                fs::remove_file(target).map_err(|e| {
                    format!(
                        "remove pre-existing symlink at {} before rename: {e}",
                        target.display(),
                    )
                })?;
            }
        }
    }

    fs::rename(&tmp_path, target).map_err(|e| {
        // Best-effort cleanup of the tmp on rename failure. Ignore the
        // remove error: leaving a stray .tmp file is a strictly better
        // failure mode than masking the original error.
        let _ = fs::remove_file(&tmp_path);
        format!(
            "rename {} -> {}: {e}",
            tmp_path.display(),
            target.display(),
        )
    })?;

    // POSIX-only: fsync the parent directory so the renamed dirent is
    // durable after a power loss. Without this, the file may appear
    // empty or missing on the next mount even though the bytes hit
    // disk via `sync_all()` above. Best-effort: if the platform's
    // directory fsync fails (e.g., readonly mount we just wrote to —
    // structurally impossible), surface the error so the operator can
    // investigate rather than silently shipping a chain whose
    // durability is unverified.
    #[cfg(unix)]
    {
        let dir = fs::File::open(parent)
            .map_err(|e| format!("open parent {} for fsync: {e}", parent.display()))?;
        dir.sync_all()
            .map_err(|e| format!("fsync parent {}: {e}", parent.display()))?;
    }

    Ok(())
}

fn now_micros() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use atlas_trust_core::trace_format::{AnchorEntry, AnchorKind, InclusionProof};

    fn fixture_entry(seed: u8, log_index: u64) -> AnchorEntry {
        AnchorEntry {
            kind: AnchorKind::DagTip,
            anchored_hash: hex::encode([seed; 32]),
            log_id: "fixture-log".to_string(),
            log_index,
            integrated_time: 1_700_000_000,
            inclusion_proof: InclusionProof {
                tree_size: 1,
                root_hash: hex::encode([seed; 32]),
                hashes: Vec::new(),
                checkpoint_sig: "fixture".to_string(),
            },
            entry_body_b64: None,
            tree_id: None,
        }
    }

    #[test]
    fn genesis_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");
        let entries = vec![fixture_entry(0x01, 0)];

        let batch = extend_chain_with_batch(&path, &entries, 1_700_000_000).unwrap();
        assert_eq!(batch.batch_index, 0);
        assert_eq!(batch.previous_head, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
        assert_eq!(batch.entries.len(), 1);
        // File now exists with exactly one line.
        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(raw.lines().count(), 1);
        assert!(raw.ends_with('\n'), "file must end in newline");
    }

    #[test]
    fn genesis_when_file_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");
        // Pre-create as an empty file.
        fs::write(&path, b"").unwrap();
        let entries = vec![fixture_entry(0x02, 0)];
        let batch = extend_chain_with_batch(&path, &entries, 1_700_000_001).unwrap();
        assert_eq!(batch.batch_index, 0);
        assert_eq!(batch.previous_head, ANCHOR_CHAIN_GENESIS_PREVIOUS_HEAD);
    }

    #[test]
    fn second_batch_links_to_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");
        let b0 = extend_chain_with_batch(&path, &[fixture_entry(0x10, 0)], 1_700_000_000).unwrap();
        let b1 = extend_chain_with_batch(&path, &[fixture_entry(0x11, 0)], 1_700_000_100).unwrap();

        assert_eq!(b1.batch_index, 1);
        assert_eq!(b1.previous_head, chain_head_for(&b0).unwrap());
    }

    #[test]
    fn three_batches_round_trip_through_verify_anchor_chain() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");

        let b0 = extend_chain_with_batch(&path, &[fixture_entry(0x20, 0)], 1_700_000_000).unwrap();
        let b1 = extend_chain_with_batch(
            &path,
            &[fixture_entry(0x21, 0), fixture_entry(0x22, 1)],
            1_700_000_100,
        )
        .unwrap();
        let b2 = extend_chain_with_batch(&path, &[fixture_entry(0x23, 0)], 1_700_000_200).unwrap();

        // Read back as the MCP exporter would.
        let raw = fs::read_to_string(&path).unwrap();
        let history: Vec<AnchorBatch> = raw
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].previous_head, b0.previous_head);
        assert_eq!(history[1].previous_head, chain_head_for(&b0).unwrap());
        assert_eq!(history[2].previous_head, chain_head_for(&b1).unwrap());

        let head = chain_head_for(history.last().unwrap()).unwrap();
        let chain = AnchorChain {
            history,
            head: head.as_str().to_string(),
        };
        let outcome = verify_anchor_chain(&chain);
        assert!(
            outcome.ok,
            "round-tripped chain must verify, errors: {:?}",
            outcome.errors,
        );
        assert_eq!(outcome.batches_walked, 3);
        // V1.13 wave-C-2: recomputed_head is Option<ChainHeadHex>; compare
        // via as_str() to keep the assertion shape uniform with other
        // string-typed comparisons in this test.
        assert_eq!(
            outcome.recomputed_head.as_ref().map(|h| h.as_str()),
            Some(head.as_str()),
        );
        assert_eq!(b2.batch_index, 2);
    }

    #[test]
    fn refuses_extension_when_existing_chain_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");

        let _b0 = extend_chain_with_batch(&path, &[fixture_entry(0x30, 0)], 1_700_000_000).unwrap();
        let _b1 = extend_chain_with_batch(&path, &[fixture_entry(0x31, 0)], 1_700_000_100).unwrap();

        // Adversary tampers with batch[0]'s entries: swaps the anchored
        // hash. Even though the JSON parses, `chain_head_for(batch[0])`
        // now produces a head that does not match `batch[1].previous_head`.
        let raw = fs::read_to_string(&path).unwrap();
        let mut lines: Vec<String> = raw.lines().map(str::to_string).collect();
        let mut b0: AnchorBatch = serde_json::from_str(&lines[0]).unwrap();
        b0.entries[0].anchored_hash = hex::encode([0xFFu8; 32]);
        lines[0] = serde_json::to_string(&b0).unwrap();
        fs::write(&path, lines.join("\n") + "\n").unwrap();

        let err = extend_chain_with_batch(&path, &[fixture_entry(0x32, 0)], 1_700_000_200)
            .expect_err("must refuse to extend corrupt chain");
        assert!(
            err.contains("refusing to extend corrupt anchor-chain"),
            "error must call out corruption, got: {err}",
        );
    }

    #[test]
    fn rejects_malformed_existing_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");
        fs::write(&path, b"{ this is not valid json\n").unwrap();
        let err = extend_chain_with_batch(&path, &[fixture_entry(0x40, 0)], 1_700_000_000)
            .expect_err("must reject malformed line");
        assert!(
            err.contains("parse anchor-chain line 1"),
            "error must reference the bad line, got: {err}",
        );
    }

    #[test]
    fn unknown_field_in_existing_line_is_rejected() {
        // serde(deny_unknown_fields) on AnchorBatch must surface as a
        // parse error here. Adversary cannot smuggle extra fields past
        // the parser at extend time.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");
        let _b0 = extend_chain_with_batch(&path, &[fixture_entry(0x50, 0)], 1_700_000_000).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(raw.trim()).unwrap();
        v.as_object_mut()
            .unwrap()
            .insert("smuggled_field".to_string(), serde_json::json!("evil"));
        fs::write(&path, serde_json::to_string(&v).unwrap() + "\n").unwrap();

        let err = extend_chain_with_batch(&path, &[fixture_entry(0x51, 0)], 1_700_000_100)
            .expect_err("deny_unknown_fields must trip");
        assert!(
            err.contains("parse anchor-chain line"),
            "error must come from the JSONL parser, got: {err}",
        );
    }

    #[test]
    fn empty_entries_slice_is_rejected() {
        // Phantom batches advance batch_index without producing
        // anchoring evidence; the issuer refuses rather than commit
        // the empty row.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");
        let err = extend_chain_with_batch(&path, &[], 1_700_000_000)
            .expect_err("must reject empty entries");
        assert!(
            err.contains("entries must not be empty"),
            "error must call out the empty-entries guard, got: {err}",
        );
        assert!(
            !path.exists(),
            "no file must be created on guard refusal",
        );
    }

    #[test]
    fn chain_export_round_trips_three_batches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");

        let _b0 = extend_chain_with_batch(&path, &[fixture_entry(0x70, 0)], 1_700_000_000).unwrap();
        let _b1 = extend_chain_with_batch(&path, &[fixture_entry(0x71, 0)], 1_700_000_100).unwrap();
        let b2 = extend_chain_with_batch(&path, &[fixture_entry(0x72, 0)], 1_700_000_200).unwrap();

        let bytes = fs::read(&path).unwrap();
        let chain = build_chain_export_from_jsonl(&bytes)
            .expect("export must succeed for valid chain");
        assert_eq!(chain.history.len(), 3);
        assert_eq!(chain.head, chain_head_for(&b2).unwrap());
        // Round-trip via verify path.
        let outcome = verify_anchor_chain(&chain);
        assert!(outcome.ok, "exported chain must verify, errors: {:?}", outcome.errors);
        assert_eq!(outcome.batches_walked, 3);
    }

    #[test]
    fn chain_export_rejects_empty_input() {
        let err = build_chain_export_from_jsonl(b"")
            .expect_err("empty JSONL must error");
        assert!(
            err.contains("anchor-chain JSONL is empty"),
            "error must call out empty input, got: {err}",
        );
    }

    #[test]
    fn chain_export_rejects_corrupt_chain() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");
        let _b0 = extend_chain_with_batch(&path, &[fixture_entry(0x80, 0)], 1_700_000_000).unwrap();
        let _b1 = extend_chain_with_batch(&path, &[fixture_entry(0x81, 0)], 1_700_000_100).unwrap();
        // Tamper after issuance: swap an anchored hash in batch[0].
        let raw = fs::read_to_string(&path).unwrap();
        let mut lines: Vec<String> = raw.lines().map(str::to_string).collect();
        let mut b0: AnchorBatch = serde_json::from_str(&lines[0]).unwrap();
        b0.entries[0].anchored_hash = hex::encode([0xEEu8; 32]);
        lines[0] = serde_json::to_string(&b0).unwrap();
        let tampered = lines.join("\n") + "\n";
        let err = build_chain_export_from_jsonl(tampered.as_bytes())
            .expect_err("tampered chain must fail export");
        assert!(
            err.contains("chain verification failed"),
            "error must call out verification failure, got: {err}",
        );
    }

    #[test]
    fn chain_export_rejects_malformed_line() {
        let err = build_chain_export_from_jsonl(b"{ this is not valid json\n")
            .expect_err("malformed JSONL must error");
        assert!(
            err.contains("parse anchor-chain line 1"),
            "error must reference the bad line, got: {err}",
        );
    }

    #[test]
    fn chain_export_treats_whitespace_only_input_as_empty() {
        // An operator who pre-creates the chain file as an empty (or
        // whitespace-only) placeholder must not silently get a chain
        // export — they must see the empty-input error and notice
        // that the file has not yet been initialised by the issuer.
        let err = build_chain_export_from_jsonl(b"   \n\n\t\r\n")
            .expect_err("whitespace-only JSONL must error");
        assert!(
            err.contains("anchor-chain JSONL is empty"),
            "error must call out empty input, got: {err}",
        );
    }

    #[test]
    fn chain_export_line_number_skips_blank_lines() {
        // Build a file with a valid first batch + trailing newline,
        // then a malformed second batch. The error must report
        // "line 2" (the second non-blank batch), not "line 3" (which
        // would be the raw split-token index counting the blank line
        // produced by the trailing newline after batch 1).
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");
        let _b0 =
            extend_chain_with_batch(&path, &[fixture_entry(0x90, 0)], 1_700_000_000).unwrap();
        let mut raw = fs::read_to_string(&path).unwrap();
        raw.push_str("{ malformed second line }\n");
        let err = build_chain_export_from_jsonl(raw.as_bytes())
            .expect_err("malformed second batch must error");
        assert!(
            err.contains("parse anchor-chain line 2"),
            "error must point to batch number 2, got: {err}",
        );
    }

    #[test]
    fn rewriting_chain_does_not_corrupt_existing_lines() {
        // Re-serialising existing batches through serde produces
        // byte-identical canonical-by-construction output (same field
        // order, no whitespace change). Verify by reading after each
        // append and checking previously-written lines are stable.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchor-chain.jsonl");
        let _ = extend_chain_with_batch(&path, &[fixture_entry(0x60, 0)], 1_700_000_000).unwrap();
        let line0_after_first: String = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string();
        let _ = extend_chain_with_batch(&path, &[fixture_entry(0x61, 0)], 1_700_000_100).unwrap();
        let line0_after_second: String = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string();
        assert_eq!(
            line0_after_first, line0_after_second,
            "rewriting must not perturb prior lines",
        );
    }
}
