//! V1.14 Scope I — PKCS#11-backed witness.
//!
//! Mirrors `atlas-signer`'s wave-3 sealed per-workspace signer
//! ([`atlas_signer::hsm::pkcs11_workspace::Pkcs11WorkspaceSigner`])
//! with three structural simplifications:
//!
//!   1. **Single key per witness.** No per-workspace fan-out, no
//!      `HashMap` cache, no find-or-generate semantics. The witness
//!      is commissioned once via the operator runbook (pkcs11-tool
//!      keypair generation under the agreed CKA_LABEL); subsequent
//!      `sign_chain_head` calls hit the same private handle.
//!   2. **No key generation.** `Pkcs11Witness::open` only *resolves*
//!      an existing keypair by label. Generation is an operator
//!      action (V1.14 Scope I commissioning ceremony), not a runtime
//!      side-effect. This is the load-bearing trust property — a
//!      witness that auto-generated keys could be made to sign on a
//!      fresh, unrostered keypair and silently bypass the roster
//!      contract.
//!   3. **String error boundary.** The `Witness` trait is dyn-safe
//!      (V1.11 footgun #20) so error variants are folded into the
//!      `String` prefix. See `crate::hsm::error` for the cleaving
//!      rules.
//!
//! ## Drop ordering
//!
//! Same invariant as the wave-3 atlas-signer counterpart: declaration
//! order puts `state` (which owns the [`Session`]) BEFORE `_ctx`, so
//! the session destructor (`C_CloseSession`) runs before the module
//! destructor (`C_Finalize`). Reordering inverts the sequence and
//! produces a use-after-free in cryptoki on every drop. (V1.14
//! footgun #15.)
//!
//! ## Threading model
//!
//! `Mutex<WitnessState>` wraps the long-lived authenticated session.
//! Concurrent calls serialise — necessary because cryptoki sessions
//! are not safe to drive from multiple threads in parallel on most
//! modules. The witness binary's CLI is single-shot per invocation
//! (one `sign-chain-head` call, then exit), so the lock contention
//! is only relevant if a future caller embeds the witness as a
//! library and shares the handle across threads.

use std::sync::Mutex;

use base64::Engine;
use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::mechanism::eddsa::{EddsaParams, EddsaSignatureScheme};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, KeyType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::types::AuthPin;
use zeroize::Zeroizing;

use atlas_trust_core::{decode_chain_head, witness_signing_input, WitnessSig};

use crate::hsm::config::{HsmWitnessConfig, WITNESS_LABEL_PREFIX};
use crate::hsm::error::{map_pkcs11_open_error, map_pkcs11_sign_error};
use crate::Witness;

/// PKCS#11-backed witness implementation.
///
/// **Drop order matters.** Rust drops fields in declaration order, so
/// `state` (which owns the [`Session`]) drops first — closing the
/// PKCS#11 session via `C_CloseSession` — and `_ctx` drops last —
/// finalising the module via `C_Finalize`. Reordering the fields
/// would `C_Finalize` the module while a session is still open, which
/// is undefined behaviour per PKCS#11 §5.4.
#[derive(Debug)]
pub struct Pkcs11Witness {
    /// Authenticated session + cached private-key handle. Single
    /// [`Mutex`] because cryptoki sessions cannot be driven from
    /// multiple threads in parallel on most modules.
    state: Mutex<WitnessState>,
    /// Witness identifier — matches the entry under which the
    /// corresponding pubkey is registered in
    /// `ATLAS_WITNESS_V1_ROSTER`. Held as an owned `String` because
    /// the trait's `witness_kid()` returns `&str` borrowed from
    /// `self`, and the lifetime contract is "lives for the duration
    /// of the witness".
    witness_kid: String,
    /// Slot id, retained for diagnostic messages on sign failures.
    /// Read-only after [`Self::open`] returns.
    slot_id: u64,
    /// PKCS#11 module context. Held only to keep the dynamic library
    /// loaded for the witness's lifetime; never accessed after `open`
    /// returns. Underscored to mute the dead-field lint without
    /// dropping the field early — drop order documented at the
    /// struct level.
    _ctx: Pkcs11,
}

/// Authenticated session + the resolved private-key handle. Locked
/// together so a future addition (e.g. a per-call counter) can
/// piggyback on the same lock without restructuring the type.
#[derive(Debug)]
struct WitnessState {
    session: Session,
    /// Handle to the witness's Ed25519 private key on the token,
    /// resolved once at [`Pkcs11Witness::open`] time. Never re-resolved
    /// — if the token is rotated under us, the next sign call fails
    /// at the cryptoki layer and the witness service must restart.
    private: ObjectHandle,
}

impl Pkcs11Witness {
    /// Open the configured PKCS#11 module, log in, and resolve the
    /// witness keypair by label.
    ///
    /// The label is `format!("{WITNESS_LABEL_PREFIX}{witness_kid}")`
    /// — the operator's commissioning ceremony places the keypair
    /// under exactly that label, so a kid mismatch surfaces here as
    /// a "key not found" error.
    ///
    /// Errors carry the V1.14 cleaving prefix (`Locked:`,
    /// `Unavailable:`, `SigningFailed:`) — see `crate::hsm::error`
    /// for the rule.
    pub fn open(config: HsmWitnessConfig, witness_kid: String) -> Result<Self, String> {
        validate_witness_kid(&witness_kid)?;

        let (ctx, session, slot_id) = open_authenticated_session(&config)?;
        let label = format!("{WITNESS_LABEL_PREFIX}{witness_kid}");
        let private = find_one_private_key(&session, &label, slot_id)?;

        Ok(Self {
            state: Mutex::new(WitnessState { session, private }),
            witness_kid,
            slot_id,
            _ctx: ctx,
        })
    }

    /// One-shot extract of the witness public key as a 64-char
    /// lowercase hex string. Used by the operator commissioning
    /// ceremony (OPERATOR-RUNBOOK §11) — after `pkcs11-tool
    /// --keypairgen` places the keypair on the token, this method
    /// surfaces the pubkey for the roster-paste step.
    ///
    /// **Lifecycle:** opens its own PKCS#11 context, logs in, reads
    /// `CKA_EC_POINT` from the public-key object, then drops the
    /// context (`C_Finalize`) before returning. Suitable for ad-hoc
    /// invocation, not for high-rate extraction (each call pays the
    /// full module-load + login cost).
    ///
    /// **Why a separate associated function (not a method on a long-
    /// lived `Pkcs11Witness`):** commissioning runs once per witness;
    /// the operator does not want to maintain a logged-in session
    /// just for the extract. Routing through the `Pkcs11Witness`
    /// constructor would also tie the extract path to a successful
    /// *private*-key lookup, which is the wrong precondition: at
    /// commissioning time the public key always exists, but
    /// extraction may run before the operator has confirmed the
    /// private-key handle works (e.g. for vendor HSMs that hide the
    /// private object until first sign-test).
    pub fn extract_pubkey_hex(
        config: HsmWitnessConfig,
        witness_kid: &str,
    ) -> Result<String, String> {
        validate_witness_kid(witness_kid)?;

        // V1.14 Scope I: shared session-setup with `Pkcs11Witness::open`
        // — module load, slot resolve, RW session, login. The `_ctx`
        // binding keeps the Pkcs11 module loaded for the duration of
        // this method (drop at end of scope finalises in the correct
        // order: session inside the helper return, then this `_ctx`).
        let (_ctx, session, slot_id) = open_authenticated_session(&config)?;

        let label = format!("{WITNESS_LABEL_PREFIX}{witness_kid}");
        let public = find_one_public_key(&session, &label, slot_id)?;

        let attrs = session
            .get_attributes(public, &[AttributeType::EcPoint])
            .map_err(map_pkcs11_open_error)?;
        let raw = attrs
            .into_iter()
            .find_map(|a| match a {
                Attribute::EcPoint(v) => Some(v),
                _ => None,
            })
            .ok_or_else(|| {
                format!(
                    "SigningFailed: public-key object with CKA_LABEL={label:?} on slot \
                     {slot_id} returned no CKA_EC_POINT — token may have stripped the \
                     attribute (vendor policy) or returned a malformed object",
                )
            })?;
        let pubkey_bytes = unwrap_ec_point(&raw, &label, slot_id)?;
        Ok(hex::encode(pubkey_bytes))
    }
}

/// Validate the operator-supplied witness kid before any PKCS#11
/// work. Mirrors the validation rules baked into [`Pkcs11Witness::open`]
/// and [`Pkcs11Witness::extract_pubkey_hex`] — extracted into a single
/// helper so a future change (e.g. additional charset restriction)
/// lands on both paths simultaneously.
///
/// Error prefix is `SigningFailed:` because kid validation precedes
/// the PKCS#11 open path: the failure is operator-actionable
/// configuration, not a token-side state.
fn validate_witness_kid(witness_kid: &str) -> Result<(), String> {
    if witness_kid.is_empty() {
        return Err(
            "SigningFailed: witness_kid must not be empty (the operator must \
             commission the witness with a stable identifier matching the \
             entry in ATLAS_WITNESS_V1_ROSTER)"
                .to_string(),
        );
    }
    if witness_kid.len() > atlas_trust_core::MAX_WITNESS_KID_LEN {
        return Err(format!(
            "SigningFailed: witness_kid exceeds MAX_WITNESS_KID_LEN ({} > {} bytes)",
            witness_kid.len(),
            atlas_trust_core::MAX_WITNESS_KID_LEN,
        ));
    }
    // Defence-in-depth: the on-token label uses ':' as the prefix
    // delimiter (see `WITNESS_LABEL_PREFIX`), so a kid containing ':'
    // would re-parse to a different (prefix, kid) split and could let
    // an operator-supplied kid like
    // `evil:atlas-witness-key-v1:other` resolve to the wrong
    // on-token object. Refuse early.
    if witness_kid.contains(':') {
        return Err(
            "SigningFailed: witness_kid must not contain ':' (reserved as the \
             on-token label-prefix delimiter)"
                .to_string(),
        );
    }
    Ok(())
}

impl Witness for Pkcs11Witness {
    fn witness_kid(&self) -> &str {
        &self.witness_kid
    }

    fn sign_chain_head(&self, chain_head_hex: &str) -> Result<WitnessSig, String> {
        // Decode + signing-input construction live entirely host-side;
        // the bytes flowing into `C_Sign` are public material plus
        // domain prefix, never secret material.
        let chain_head_bytes = decode_chain_head(chain_head_hex).map_err(|e| {
            // Trust-core's `decode_chain_head` returns a structured
            // `TrustError`; flatten to the witness `String` boundary
            // with a `SigningFailed:` prefix because the failure mode
            // (malformed wire input) is operator-actionable but not
            // an HSM-side issue.
            format!("SigningFailed: decode_chain_head: {e}")
        })?;
        let signing_input = witness_signing_input(&chain_head_bytes);

        let state = self.state.lock().map_err(|_| {
            // Mutex poison is permanent for the affected process
            // (`std::sync::Mutex` does not un-poison) so the only
            // fix is restart. Mirror atlas-signer's wording so the
            // runbook grep ("poison" + "restart") works on both
            // binaries.
            "SigningFailed: PKCS#11 witness session mutex poisoned — a previous signing \
             op panicked; process restart required (poison is permanent for this process)"
                .to_string()
        })?;

        // CKM_EDDSA(Ed25519) is the deterministic-signing mechanism
        // from PKCS#11 v3.0 §6.5.5. The signature is the raw 64-byte
        // RFC 8032 signature (R || S, 32 + 32 bytes); same wire-shape
        // the file-backed witness produces.
        let mech = Mechanism::Eddsa(EddsaParams::new(EddsaSignatureScheme::Ed25519));
        let signature = state
            .session
            .sign(&mech, state.private, &signing_input)
            .map_err(map_pkcs11_sign_error)?;

        // RFC 8032 §5.1.6: Ed25519 signatures are exactly 64 bytes.
        // A different length means the HSM is producing a
        // non-Ed25519 sig under CKM_EDDSA — fail loud rather than
        // truncate-or-extend.
        if signature.len() != 64 {
            return Err(format!(
                "SigningFailed: PKCS#11 CKM_EDDSA returned {} bytes for kid {:?} on slot {}; \
                 RFC 8032 §5.1.6 mandates 64-byte signatures",
                signature.len(),
                self.witness_kid,
                self.slot_id,
            ));
        }

        // URL-safe base64, no padding — same dialect as
        // `Ed25519Witness::sign_chain_head` and the verifier's
        // `verify_witness_against_roster` decoder. One dialect across
        // the whole wire format; a divergence here would be the
        // byte-equivalence test's primary failure surface.
        Ok(WitnessSig {
            witness_kid: self.witness_kid.clone(),
            signature: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&signature),
        })
    }
}

/// Shared session-setup path used by both [`Pkcs11Witness::open`]
/// and [`Pkcs11Witness::extract_pubkey_hex`].
///
/// Performs: module load (`C_Initialize`), slot resolve, RW session
/// open (`C_OpenSession`), and User login (`C_Login`). Returns the
/// `Pkcs11` context (caller owns; drop order: declare AFTER `Session`
/// so finalize runs after close), the authenticated `Session`, and
/// the resolved `slot_id` (retained for diagnostic messages on later
/// failures).
///
/// **Why one helper, not two specialised constructors:** before V1.14
/// HIGH-3 (code-review), `open` and `extract_pubkey_hex` each
/// inlined this block, which created a duplication risk: a future
/// hardening (e.g. RO-session preference for `extract_pubkey_hex`,
/// or a different login user-type) would need to land on both sites
/// or one would silently drift. The helper collapses the
/// session-setup invariant to a single site.
///
/// **RW session is intentional even for the read-only path.** The
/// V1.14 byte-equivalence test imports a key pre-test and needs RW;
/// pinning RW here matches production semantics (no mode-switch
/// between test and prod) and mirrors atlas-signer's wave-3 RW
/// invariant. `extract_pubkey_hex` only uses `get_attributes`, but
/// the login is still required on most modules because public-key
/// objects on a logged-out session can be filtered out by token
/// policy.
fn open_authenticated_session(
    config: &HsmWitnessConfig,
) -> Result<(Pkcs11, Session, u64), String> {
    let ctx = Pkcs11::new(config.module_path()).map_err(map_pkcs11_open_error)?;
    ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
        .map_err(map_pkcs11_open_error)?;

    // PKCS#11 doesn't let us construct a `Slot` directly; we
    // enumerate slots-with-token and pick the one whose id
    // matches the operator's `ATLAS_WITNESS_HSM_SLOT`. Mirrors
    // atlas-signer's pattern.
    let slot_id = config.slot();
    let slot = ctx
        .get_slots_with_token()
        .map_err(map_pkcs11_open_error)?
        .into_iter()
        .find(|s| s.id() == slot_id)
        .ok_or_else(|| {
            format!(
                "Unavailable: PKCS#11 slot {} not present in module {}",
                slot_id,
                config.module_path().display(),
            )
        })?;

    let session = ctx.open_rw_session(slot).map_err(map_pkcs11_open_error)?;

    let pin = read_pin_file_for_witness(config)?;
    session
        .login(UserType::User, Some(&pin))
        .map_err(map_pkcs11_open_error)?;

    Ok((ctx, session, slot_id))
}

/// Resolve the witness's Ed25519 private key by label. Refuses both
/// not-found (operator hasn't commissioned the key) and ambiguity
/// (operator commissioned twice and forgot to delete the old one).
///
/// Restricting the find template to `(class, key_type, label)` rather
/// than just `(class, label)` is defence in depth: a future operator
/// might accidentally create a non-Ed25519 object under our label
/// namespace, and matching by key_type ensures we never sign with the
/// wrong algorithm. Mirrors atlas-signer's `find_one_object`.
fn find_one_private_key(
    session: &Session,
    label: &str,
    slot_id: u64,
) -> Result<ObjectHandle, String> {
    let template = vec![
        Attribute::Class(ObjectClass::PRIVATE_KEY),
        Attribute::KeyType(KeyType::EC_EDWARDS),
        Attribute::Label(label.as_bytes().to_vec()),
    ];
    let handles = session
        .find_objects(&template)
        .map_err(map_pkcs11_open_error)?;
    let handle = match handles.len() {
        0 => {
            return Err(format!(
                "SigningFailed: no PKCS#11 private-key object with CKA_LABEL={label:?} on \
                 slot {slot_id} — has the witness keypair been commissioned via \
                 OPERATOR-RUNBOOK §11?",
            ))
        }
        1 => handles[0],
        n => {
            return Err(format!(
                "SigningFailed: ambiguous: {n} private-key objects with CKA_LABEL={label:?} \
                 on slot {slot_id} — operator must dedupe via the runbook before signing can \
                 proceed",
            ))
        }
    };

    // V1.14 Scope I (security-review HIGH-2): runtime enforcement of
    // CKA_DERIVE=false on the resolved private key. The runbook §11
    // commissioning ceremony places the keypair with `--usage sign`
    // (which on softhsm2 sets CKA_DERIVE=false), but a future
    // operator who imports a derive-capable key under the witness
    // label namespace must surface as a runtime error before any
    // signing operation is permitted. Refusing missing CKA_DERIVE
    // and refusing CKA_DERIVE=true are equivalent: vendor-policy-
    // stripped attributes mean we cannot prove the key is
    // non-derive-capable, and the trust property requires we can.
    let derive_attrs = session
        .get_attributes(handle, &[AttributeType::Derive])
        .map_err(map_pkcs11_open_error)?;
    let derive_value = derive_attrs.iter().find_map(|a| match a {
        Attribute::Derive(v) => Some(*v),
        _ => None,
    });
    match derive_value {
        Some(false) => Ok(handle),
        Some(true) => Err(format!(
            "SigningFailed: PKCS#11 private-key object with CKA_LABEL={label:?} on slot \
             {slot_id} has CKA_DERIVE=true — refusing to sign with a derive-capable key. \
             Re-commission the keypair via OPERATOR-RUNBOOK §11 with `--usage sign` (which \
             pins CKA_DERIVE=false).",
        )),
        None => Err(format!(
            "SigningFailed: PKCS#11 private-key object with CKA_LABEL={label:?} on slot \
             {slot_id} did not return CKA_DERIVE — vendor policy may have stripped the \
             attribute. Verify the keypair was commissioned via OPERATOR-RUNBOOK §11 with \
             explicit CKA_DERIVE=false and that the token vendor exposes the attribute \
             readback.",
        )),
    }
}

/// Public-key counterpart to [`find_one_private_key`]. Used by
/// [`Pkcs11Witness::extract_pubkey_hex`] to resolve the matching
/// public object so its `CKA_EC_POINT` attribute can be read.
///
/// Same `(class, key_type, label)` template shape so the same
/// "ambiguous → loud failure" semantic applies — if the operator's
/// commissioning ceremony left two public objects under the same
/// label, the runbook owns the dedupe step before extraction can
/// proceed.
fn find_one_public_key(
    session: &Session,
    label: &str,
    slot_id: u64,
) -> Result<ObjectHandle, String> {
    let template = vec![
        Attribute::Class(ObjectClass::PUBLIC_KEY),
        Attribute::KeyType(KeyType::EC_EDWARDS),
        Attribute::Label(label.as_bytes().to_vec()),
    ];
    let handles = session
        .find_objects(&template)
        .map_err(map_pkcs11_open_error)?;
    match handles.len() {
        0 => Err(format!(
            "SigningFailed: no PKCS#11 public-key object with CKA_LABEL={label:?} on slot \
             {slot_id} — has the witness keypair been commissioned via OPERATOR-RUNBOOK §11? \
             (commissioning emits both private + public objects; if the private is present \
             but the public is not, vendor policy may have stripped the public — re-run the \
             ceremony with explicit `CKA_TOKEN=true` on the public template).",
        )),
        1 => Ok(handles[0]),
        n => Err(format!(
            "SigningFailed: ambiguous: {n} public-key objects with CKA_LABEL={label:?} on \
             slot {slot_id} — operator must dedupe via the runbook before extraction can \
             proceed",
        )),
    }
}

/// Decode a `CKA_EC_POINT` attribute back to the raw 32-byte Ed25519
/// compressed point. PKCS#11 v3.0 §10.10 mandates DER-OCTET-STRING
/// wrapping (`tag 0x04 || length 0x20 || 32 raw bytes`); SoftHSM2
/// ≥ 2.5 follows the spec, but several commercial HSMs (and SoftHSM2
/// ≤ 2.4) return the raw 32-byte form. This helper accepts both —
/// matching the integration-test side's reader so a future divergence
/// is caught at one site.
///
/// The raw-form branch silently accepts: the wave-3 atlas-signer
/// counterpart emits an `eprintln!` annotation in the same
/// situation, but the witness binary's CLI is single-shot
/// (operator runs `extract-pubkey-hex` once during commissioning),
/// so a one-line stderr in production would just become noise. The
/// integration test's `unwrap_octet_string` retains the eprintln
/// because that's a CI signal worth preserving.
fn unwrap_ec_point(bytes: &[u8], label: &str, slot_id: u64) -> Result<[u8; 32], String> {
    if bytes.len() == 32 {
        return bytes.try_into().map_err(|_| {
            format!(
                "SigningFailed: CKA_EC_POINT for label {label:?} on slot {slot_id} \
                 is exactly 32 bytes but failed to coerce to [u8; 32] (impossible \
                 unless slice::try_into has a bug)"
            )
        });
    }
    if bytes.len() == 34 && bytes[0] == 0x04 && bytes[1] == 32 {
        return bytes[2..34].try_into().map_err(|_| {
            format!(
                "SigningFailed: CKA_EC_POINT for label {label:?} on slot {slot_id} \
                 is 34 bytes with valid OCTET-STRING header but inner slice failed \
                 to coerce to [u8; 32] (impossible unless slice::try_into has a bug)"
            )
        });
    }
    Err(format!(
        "SigningFailed: CKA_EC_POINT for label {label:?} on slot {slot_id} has \
         unexpected shape: {} bytes, first 2 = [{:#x}, {:#x}] (expected 32 raw bytes \
         or 34 bytes [0x04, 0x20, ..32 raw]). Check vendor docs for the module's \
         CKA_EC_POINT encoding.",
        bytes.len(),
        bytes.first().copied().unwrap_or(0),
        bytes.get(1).copied().unwrap_or(0),
    ))
}

/// Read the witness PIN file from disk and return a [`AuthPin`].
///
/// Mirrors the atlas-signer-wave-3 hardened reader (V1.11 M-2):
///
///   * TOCTOU-safe FD-based metadata via `File::metadata()` rather
///     than `fs::metadata(path)` — both checks bind to the same inode.
///   * Unix permission guard refuses files with any group/world rwx
///     bits set (mode `0o077`). Owner bits unconstrained: 0400 and
///     0600 are both runbook-acceptable.
///   * `Zeroizing<Vec<u8>>` pre-sized to file length so `read_to_end`
///     does not realloc-and-grow mid-read (which would orphan
///     partial PIN bytes in the freed-allocator pool). The
///     `Zeroizing` wrapper scrubs on drop, including on early-return
///     panic paths.
///   * Single expression for the final `AuthPin::from(trim.to_string())`
///     so no named binding can drop without zeroing on an
///     intermediate error.
///
/// **Trust-domain isolation: this is intentionally NOT shared with
/// `atlas-signer`.** Despite byte-identical logic, the witness binary
/// has its own runbook section, its own env-var prefix, and its own
/// failure-mode messaging. Sharing the function across crates would
/// couple two trust domains; if a future hardening (e.g. owner-uid
/// match) needs to land on one and not the other, the duplication
/// gives us that flexibility without a coordinated cross-crate edit.
/// The duplication is comment-flagged on both sides.
pub(crate) fn read_pin_file_for_witness(config: &HsmWitnessConfig) -> Result<AuthPin, String> {
    use std::io::Read;

    // Open the file *first* so the subsequent metadata + read calls
    // both bind to the same inode via the file descriptor —
    // TOCTOU-safe. `std::fs::read(&path)` would re-resolve the path
    // and leave a window for an attacker with filesystem write
    // access to swap files between checks.
    let pin_path = config.pin_file();
    let mut file = std::fs::File::open(pin_path)
        .map_err(|e| format!("Locked: PIN file {} unreadable: {e}", pin_path.display()))?;

    let meta = file.metadata().map_err(|e| {
        format!(
            "Locked: PIN file {} metadata unreadable: {e}",
            pin_path.display()
        )
    })?;

    // V1.11 M-2 — Unix permission guard. Mask `mode() & 0o777` to
    // drop file-type bits, then `& 0o077` selects only group/other
    // rwx. Owner bits unconstrained: 0400 and 0600 are both
    // runbook-acceptable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let mode = meta.mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(format!(
                "Locked: PIN file {} has group/world-accessible permissions \
                 (mode {:04o}); chmod 0400 (or 0600) required to prevent local \
                 privilege escalation reading the witness PIN",
                pin_path.display(),
                mode,
            ));
        }
    }

    let len_hint = (meta.len() as usize).max(1);
    let mut bytes: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::with_capacity(len_hint));
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("Locked: PIN file {} read failed: {e}", pin_path.display()))?;

    let trimmed = std::str::from_utf8(&bytes)
        .map_err(|_| format!("Locked: PIN file {} is not UTF-8", pin_path.display()))?
        .trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
    if trimmed.is_empty() {
        return Err(format!(
            "Locked: PIN file {} is empty (after trim)",
            pin_path.display()
        ));
    }
    // V1.14 Scope I (security-review HIGH-1): close the
    // unscrubbed-String window between the `to_owned()` allocation
    // and the AuthPin handoff. The String is built inside a
    // Zeroizing wrapper so the heap allocation is owned by a
    // Drop-zeroizing type for its full lifetime; `mem::take`
    // extracts it (leaving Default = empty String in place — its
    // on-drop zeroize is a no-op), AuthPin::from consumes the
    // result, and AuthPin's internal SecretString scrubs on
    // AuthPin drop. Net: the PIN bytes are owned by a zeroizing
    // type at every point in the chain.
    let mut pin_buf: Zeroizing<String> = Zeroizing::new(trimmed.to_owned());
    Ok(AuthPin::from(std::mem::take(&mut *pin_buf)))
}

/// Compile-time guard: [`Pkcs11Witness`] must be `Send + Sync` so
/// embedders can wrap it in `Arc<dyn Witness + Send + Sync>` and
/// share it across async tasks. The trait itself requires
/// `Send + Sync`; this assertion catches a regression at compile
/// time rather than at the embedder's call site.
const _: () = {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    let _ = assert_send::<Pkcs11Witness>;
    let _ = assert_sync::<Pkcs11Witness>;
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Reject empty kid at `Pkcs11Witness::open` — the validation
    /// runs BEFORE any PKCS#11 work, so we can exercise it without a
    /// live HSM module. The test config carries an absolute path
    /// that does not exist; if validation regressed and we reached
    /// `Pkcs11::new`, the error would be `Unavailable: …library
    /// load…` rather than the kid-validation message.
    #[test]
    fn open_rejects_empty_kid() {
        let dir = TempDir::new().expect("tempdir");
        let pin = dir.path().join("pin");
        std::fs::write(&pin, "1234").expect("write pin");
        let cfg = HsmWitnessConfig::from_env_for_test(
            PathBuf::from(absolute_dummy_path()),
            0,
            pin,
        );
        let err = Pkcs11Witness::open(cfg, String::new()).unwrap_err();
        assert!(
            err.starts_with("SigningFailed:") && err.contains("must not be empty"),
            "empty kid must fail-fast with kid-validation message; got: {err}",
        );
    }

    #[test]
    fn open_rejects_kid_exceeding_max_len() {
        let dir = TempDir::new().expect("tempdir");
        let pin = dir.path().join("pin");
        std::fs::write(&pin, "1234").expect("write pin");
        let cfg = HsmWitnessConfig::from_env_for_test(
            PathBuf::from(absolute_dummy_path()),
            0,
            pin,
        );
        let oversize = "k".repeat(atlas_trust_core::MAX_WITNESS_KID_LEN + 1);
        let err = Pkcs11Witness::open(cfg, oversize).unwrap_err();
        assert!(
            err.starts_with("SigningFailed:") && err.contains("MAX_WITNESS_KID_LEN"),
            "oversize kid must fail-fast with cap message; got: {err}",
        );
    }

    #[test]
    fn open_rejects_kid_containing_label_delimiter() {
        let dir = TempDir::new().expect("tempdir");
        let pin = dir.path().join("pin");
        std::fs::write(&pin, "1234").expect("write pin");
        let cfg = HsmWitnessConfig::from_env_for_test(
            PathBuf::from(absolute_dummy_path()),
            0,
            pin,
        );
        let err = Pkcs11Witness::open(cfg, "evil:atlas-witness-key-v1:other".to_string())
            .unwrap_err();
        assert!(
            err.starts_with("SigningFailed:") && err.contains("':'"),
            "kid containing ':' must fail-fast with delimiter message; got: {err}",
        );
    }

    /// Cross-platform absolute path stand-in. The kid-validation
    /// branch returns BEFORE `Pkcs11::new`, so the path doesn't have
    /// to point at a real module — but `HsmWitnessConfig` requires
    /// an absolute path to even construct.
    fn absolute_dummy_path() -> &'static str {
        if cfg!(windows) {
            "C:\\nonexistent\\libsofthsm2.so"
        } else {
            "/nonexistent/libsofthsm2.so"
        }
    }

    #[test]
    fn read_pin_file_rejects_empty_pin() {
        let dir = TempDir::new().expect("tempdir");
        let pin_path = dir.path().join("pin");
        std::fs::write(&pin_path, "   \n\t  ").expect("write whitespace pin");
        let cfg = HsmWitnessConfig::from_env_for_test(
            PathBuf::from(absolute_dummy_path()),
            0,
            pin_path,
        );
        let err = read_pin_file_for_witness(&cfg).unwrap_err();
        assert!(
            err.starts_with("Locked:") && err.contains("empty"),
            "whitespace-only PIN must be rejected as empty; got: {err}",
        );
    }

    #[test]
    fn read_pin_file_rejects_non_utf8() {
        let dir = TempDir::new().expect("tempdir");
        let pin_path = dir.path().join("pin");
        // Lone 0x80 is invalid UTF-8 in any continuation context.
        std::fs::write(&pin_path, [0xff, 0x80]).expect("write non-utf8 pin");
        let cfg = HsmWitnessConfig::from_env_for_test(
            PathBuf::from(absolute_dummy_path()),
            0,
            pin_path,
        );
        let err = read_pin_file_for_witness(&cfg).unwrap_err();
        assert!(
            err.starts_with("Locked:") && err.contains("UTF-8"),
            "non-UTF-8 PIN must be rejected; got: {err}",
        );
    }

    #[test]
    fn read_pin_file_returns_authpin_for_valid_input() {
        let dir = TempDir::new().expect("tempdir");
        let pin_path = dir.path().join("pin");
        std::fs::write(&pin_path, "1234\n").expect("write pin with trailing newline");
        let cfg = HsmWitnessConfig::from_env_for_test(
            PathBuf::from(absolute_dummy_path()),
            0,
            pin_path,
        );
        // Only assert the function returns Ok — the AuthPin's
        // SecretString deliberately does not expose its bytes for
        // direct inspection (security property). The byte-equivalence
        // integration test exercises the PIN end-to-end against a
        // live SoftHSM2 token.
        let _pin = read_pin_file_for_witness(&cfg).expect("valid PIN must read cleanly");
    }

    /// `extract_pubkey_hex` runs the same kid-validation as `open`
    /// before any PKCS#11 work — pin all three rejection branches so
    /// a future kid-rule change must update both call sites or fail
    /// here. Mirrors the `open_rejects_*` triplet above.
    #[test]
    fn extract_pubkey_hex_rejects_empty_kid() {
        let dir = TempDir::new().expect("tempdir");
        let pin = dir.path().join("pin");
        std::fs::write(&pin, "1234").expect("write pin");
        let cfg = HsmWitnessConfig::from_env_for_test(
            PathBuf::from(absolute_dummy_path()),
            0,
            pin,
        );
        let err = Pkcs11Witness::extract_pubkey_hex(cfg, "").unwrap_err();
        assert!(
            err.starts_with("SigningFailed:") && err.contains("must not be empty"),
            "empty kid must fail-fast with kid-validation message; got: {err}",
        );
    }

    #[test]
    fn extract_pubkey_hex_rejects_kid_exceeding_max_len() {
        let dir = TempDir::new().expect("tempdir");
        let pin = dir.path().join("pin");
        std::fs::write(&pin, "1234").expect("write pin");
        let cfg = HsmWitnessConfig::from_env_for_test(
            PathBuf::from(absolute_dummy_path()),
            0,
            pin,
        );
        let oversize = "k".repeat(atlas_trust_core::MAX_WITNESS_KID_LEN + 1);
        let err = Pkcs11Witness::extract_pubkey_hex(cfg, &oversize).unwrap_err();
        assert!(
            err.starts_with("SigningFailed:") && err.contains("MAX_WITNESS_KID_LEN"),
            "oversize kid must fail-fast with cap message; got: {err}",
        );
    }

    #[test]
    fn extract_pubkey_hex_rejects_kid_containing_label_delimiter() {
        let dir = TempDir::new().expect("tempdir");
        let pin = dir.path().join("pin");
        std::fs::write(&pin, "1234").expect("write pin");
        let cfg = HsmWitnessConfig::from_env_for_test(
            PathBuf::from(absolute_dummy_path()),
            0,
            pin,
        );
        let err = Pkcs11Witness::extract_pubkey_hex(cfg, "evil:atlas-witness-key-v1:other")
            .unwrap_err();
        assert!(
            err.starts_with("SigningFailed:") && err.contains("':'"),
            "kid containing ':' must fail-fast with delimiter message; got: {err}",
        );
    }

    /// `unwrap_ec_point` accepts both the spec-compliant DER OCTET
    /// STRING form and the raw 32-byte form some commercial HSMs
    /// emit. Pin both happy paths so a future spec-tightening
    /// (refusing the raw form) is a deliberate behavior change, not
    /// an accident.
    #[test]
    fn unwrap_ec_point_accepts_der_octet_string() {
        let raw = [7u8; 32];
        let mut der = vec![0x04, 32];
        der.extend_from_slice(&raw);
        let extracted = unwrap_ec_point(&der, "test-label", 0).expect("DER form must decode");
        assert_eq!(extracted, raw);
    }

    #[test]
    fn unwrap_ec_point_accepts_raw_32_bytes() {
        let raw = [9u8; 32];
        let extracted = unwrap_ec_point(&raw, "test-label", 0).expect("raw form must decode");
        assert_eq!(extracted, raw);
    }

    /// Wrong length / wrong header (any non-32, non-`[0x04, 32, ..32]`
    /// input) must surface a SigningFailed: error so a malformed-
    /// attribute regression at the cryptoki side never silently
    /// produces a bogus pubkey. Specifically pinned: empty, short raw,
    /// off-by-one, mismatched DER length header, wrong DER tag.
    #[test]
    fn unwrap_ec_point_rejects_unexpected_length() {
        let mut wrong_der_length = vec![0x04, 33];
        wrong_der_length.extend(vec![0u8; 33]);
        let mut wrong_der_tag = vec![0x05, 32];
        wrong_der_tag.extend(vec![0u8; 32]);
        for bad in [
            vec![],
            vec![0u8; 31],
            vec![0u8; 33],
            wrong_der_length,
            wrong_der_tag,
        ] {
            let err = unwrap_ec_point(&bad, "test-label", 0).unwrap_err();
            assert!(
                err.starts_with("SigningFailed:") && err.contains("unexpected shape"),
                "bad input len={} must surface unexpected-shape error; got: {err}",
                bad.len(),
            );
        }
    }
}
