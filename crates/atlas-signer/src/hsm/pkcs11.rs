//! V1.10 wave 2 — PKCS#11-backed [`MasterSeedHkdf`] implementation.
//!
//! Wires [`crate::keys::MasterSeedHkdf`] to a PKCS#11 v3.0
//! token via `CKM_HKDF_DERIVE` (RFC 5869, extract+expand). The master
//! seed lives inside the token under
//! [`CKA_LABEL = MASTER_SEED_LABEL`](crate::hsm::config::MASTER_SEED_LABEL),
//! is marked `CKA_SENSITIVE=true` and `CKA_EXTRACTABLE=false` by the
//! operator's import ceremony (see `OPERATOR-RUNBOOK.md`), and is the
//! single base-key input to every per-tenant derivation. Each
//! [`MasterSeedHkdf::derive_for`] call:
//!
//!   1. Builds an [`HkdfParams`] with PRF = `MechanismType::SHA256`,
//!      `salt = HkdfSalt::Null` (RFC 5869's "no salt" path — the
//!      device internally treats salt as `HashLen` zeros), and
//!      `info = b"atlas-anchor-v1:" || workspace_id`.
//!   2. Calls `C_DeriveKey` via [`Session::derive_key`]. The derived
//!      key template asks for an *ephemeral, in-token, extractable*
//!      32-byte `CKK_GENERIC_SECRET` object — `CKA_TOKEN=false`
//!      (session-only, never persisted), `CKA_SENSITIVE=false` and
//!      `CKA_EXTRACTABLE=true` (so we can read `CKA_VALUE`).
//!   3. Reads `CKA_VALUE` to copy the 32 bytes into the caller's
//!      output buffer.
//!   4. Destroys the ephemeral derived object — even on the error
//!      path — so a misbehaving caller cannot accumulate per-tenant
//!      keys inside the token's session storage.
//!
//! ## Threading model
//!
//! The master seed handle is resolved once at [`Pkcs11MasterSeedHkdf::open`]
//! time and cached. The authenticated session is held open for the
//! lifetime of the loader behind a [`Mutex`] — concurrent
//! `derive_for` calls serialise. Per-call session open + login would
//! double the latency of every signing request and every login is an
//! audit event in commercial HSMs (Thales Luna logs every `C_Login`),
//! so the long-lived session is the right trade.
//!
//! ## Why not non-extractable derived keys?
//!
//! The architectural constraint is "the master seed never leaves the
//! HSM" — the per-tenant *derived* seed must leave so we can build an
//! `ed25519_dalek::SigningKey` from it. PKCS#11 has no standard
//! mechanism for "HKDF then Ed25519 keypair derivation" (CKM_EDDSA
//! takes a private key but no chained derivation), so V1.10 wave 2
//! ships the 32 derived bytes out of the token. Wave 3 (post-V1.10,
//! tracked under "HSM seal scope" in `.handoff/v1.10-handoff.md`)
//! evaluates either generating Ed25519 keys directly inside the
//! token via `CKM_EC_EDWARDS_KEY_PAIR_GEN` (vendor-dependent) or
//! signing inside the token via a vendor `CKM_EDDSA` mechanism.

use std::sync::Mutex;

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::mechanism::hkdf::{HkdfParams, HkdfSalt};
use cryptoki::mechanism::{Mechanism, MechanismType};
use cryptoki::object::{Attribute, AttributeType, KeyType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};
use cryptoki::types::AuthPin;
use zeroize::Zeroizing;

use crate::hsm::config::{HsmConfig, MASTER_SEED_LABEL};
use crate::hsm::error::map_pkcs11_error;
use crate::keys::{MasterSeedError, MasterSeedHkdf};

/// Length in bytes of the HKDF output we ask the token to derive.
/// Matches the [`MasterSeedHkdf::derive_for`] contract
/// (`out: &mut [u8; 32]`) and the dev impl's HKDF-SHA256-32-byte path.
const DERIVED_LEN: usize = 32;

/// PKCS#11-backed [`MasterSeedHkdf`].
///
/// Construction performs the one-shot, non-cheap setup (load the
/// module, open + log into a session, resolve the master seed object
/// by label). Per-call work is just a `C_DeriveKey` + attribute read
/// + destroy, all inside the already-authenticated session.
///
/// Drop order: Rust drops struct fields in declaration order, so
/// `session` runs its destructor (closing the PKCS#11 session via
/// `C_CloseSession`) before `_ctx` runs its own (`C_Finalize` on the
/// module). That ordering is required by PKCS#11: closing a session
/// after `C_Finalize` is undefined behaviour. Reordering these fields
/// — putting `_ctx` first, for instance — would invert the destructor
/// sequence and produce a use-after-free in the cryptoki module on
/// every drop.
#[derive(Debug)]
pub struct Pkcs11MasterSeedHkdf {
    /// Long-lived authenticated session. Wrapped in [`Mutex`] so
    /// concurrent callers serialise — even though cryptoki 0.12
    /// marks `Session: Send`, parallel `C_DeriveKey` calls on the
    /// same session race on the underlying token state on most
    /// implementations.
    session: Mutex<Session>,
    /// Handle to the master seed object inside the token. Resolved
    /// once at [`open`](Self::open) time by `CKA_LABEL = MASTER_SEED_LABEL`.
    master_seed: ObjectHandle,
    /// Slot id, retained for diagnostic messages on derive failures.
    slot_id: u64,
    /// PKCS#11 module context. Held only to keep the dynamic library
    /// loaded for the loader's lifetime; never accessed after `open`
    /// returns. Field name is underscored to mute the dead-field lint
    /// without removing the field — removing it would drop the
    /// context immediately after `open`, which `C_Finalize`s the
    /// module while the session is still open and produces a
    /// use-after-free in cryptoki. Drop ordering is documented at the
    /// struct level above.
    _ctx: Pkcs11,
}

impl Pkcs11MasterSeedHkdf {
    /// Open the configured PKCS#11 module, log in, and resolve the
    /// master seed object handle. Returns
    /// [`MasterSeedError::Unavailable`] for token-not-present /
    /// module-missing, [`MasterSeedError::Locked`] for PIN errors,
    /// and [`MasterSeedError::DeriveFailed`] when the master seed
    /// object cannot be found in the configured slot.
    ///
    /// All cryptoki errors are mapped through [`map_pkcs11_error`]
    /// so the operator-facing categorisation is consistent across
    /// the construction path and the per-call derive path.
    pub fn open(config: HsmConfig) -> Result<Self, MasterSeedError> {
        let ctx = Pkcs11::new(&config.module_path).map_err(map_pkcs11_error)?;
        ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
            .map_err(map_pkcs11_error)?;

        // PKCS#11 doesn't let us construct a `Slot` directly; we
        // enumerate slots-with-token and pick the one whose id matches
        // the operator's `ATLAS_HSM_SLOT`.
        let slot = ctx
            .get_slots_with_token()
            .map_err(map_pkcs11_error)?
            .into_iter()
            .find(|s| s.id() == config.slot)
            .ok_or_else(|| {
                MasterSeedError::Unavailable(format!(
                    "PKCS#11 slot {} not present in module {}",
                    config.slot,
                    config.module_path.display()
                ))
            })?;

        // Read-write session: `C_DeriveKey` creates a (transient)
        // object, which most modules require RW even when
        // `CKA_TOKEN=false` on the derived object. SoftHSM2 permits
        // derive on an RO session, but the major commercial HSMs do
        // not, so we always open RW.
        let session = ctx.open_rw_session(slot).map_err(map_pkcs11_error)?;

        let pin = read_pin_file(&config)?;
        session
            .login(UserType::User, Some(&pin))
            .map_err(map_pkcs11_error)?;

        let master_seed = find_master_seed(&session, config.slot)?;

        Ok(Self {
            session: Mutex::new(session),
            master_seed,
            slot_id: config.slot,
            _ctx: ctx,
        })
    }
}

/// Read the PIN file from disk, trim trailing newlines, wrap in a
/// [`AuthPin`] (`secrecy::SecretString`).
///
/// `Locked` is the right category for a missing, unreadable, or
/// over-permissive PIN file — the token itself may be perfectly
/// reachable; the operator just hasn't supplied auth material the
/// loader can safely use.
///
/// V1.11 M-2 hardening: on Unix, the loader refuses to consume a PIN
/// file with any group/world access bits set. The runbook ceremony
/// installs the file mode 0400; an operator who creates the file via
/// `echo "$PIN" > /run/atlas/hsm.pin` gets the umask default (typically
/// 0644) and would, before this guard, silently authenticate to the
/// token while exposing the PIN to every local user. Refusing at read
/// time turns that footgun into a loud failure with a remediation hint.
///
/// The check uses `File::metadata()` on the *open* file handle (not a
/// path-lookup `metadata()`), which closes the standard TOCTOU window
/// where an attacker could swap the file between the permission check
/// and the read. Both checks bind to the same inode.
///
/// Windows has no portable file-mode equivalent — ACL semantics are
/// fundamentally different — and the runbook places production
/// deployments on Linux/container hosts, so the check is `#[cfg(unix)]`.
///
/// **Visibility.** `#[doc(hidden)] pub` (gated behind `#[cfg(feature
/// = "hsm")]`) so the V1.11 wave-3 sealed per-workspace signer
/// ([`crate::hsm::pkcs11_workspace`]) AND the wave-3 integration test
/// (`tests/hsm_workspace_signer.rs`) can both reuse the hardened
/// reader instead of growing byte-identical copies of the TOCTOU +
/// mode-bit guard. Three copies would drift the moment one side added
/// a check (e.g. owner-uid match) and the others did not — a silent
/// regression of a security guarantee. The wave-2 integration test
/// initially duplicated this reader without the mode-0400 guard,
/// which was flagged in the V1.11 wave-3 Phase B security review
/// (H-2); the fix is to share rather than duplicate. `#[doc(hidden)]`
/// keeps it out of rustdoc so external consumers don't mistake it
/// for a stable API surface.
#[doc(hidden)]
pub fn read_pin_file(config: &HsmConfig) -> Result<AuthPin, MasterSeedError> {
    use std::io::Read;

    // Open the file *first* so the subsequent metadata + read calls
    // both bind to the same inode via the file descriptor — TOCTOU-safe.
    // A plain `std::fs::read(&path)` would re-resolve the path and
    // leave a window for an attacker with filesystem write access in
    // the secret-mount directory to swap files between checks.
    let mut file = std::fs::File::open(&config.pin_file).map_err(|e| {
        MasterSeedError::Locked(format!(
            "PIN file {} unreadable: {e}",
            config.pin_file.display()
        ))
    })?;

    let meta = file.metadata().map_err(|e| {
        MasterSeedError::Locked(format!(
            "PIN file {} metadata unreadable: {e}",
            config.pin_file.display()
        ))
    })?;

    // V1.11 M-2 — Unix permission guard. Mask `mode() & 0o777` to drop
    // file-type bits, then `& 0o077` selects only group/other rwx.
    // Owner bits are deliberately not constrained: the runbook ships
    // 0400, but 0600 is also a valid layout (e.g. systemd
    // `LoadCredential=` tmpfs default) and we don't want to refuse a
    // correctly-secured PIN file just because it's owner-writable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let mode = meta.mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(MasterSeedError::Locked(format!(
                "PIN file {} has group/world-accessible permissions \
                 (mode {:04o}); chmod 0400 (or 0600) required to \
                 prevent local privilege escalation reading the PIN",
                config.pin_file.display(),
                mode,
            )));
        }
    }

    // Pre-size the destination Vec from the file's reported length so
    // `read_to_end` does not realloc-and-grow during the read. A growth
    // realloc copies the partially-read PIN bytes to a new buffer and
    // frees the old, smaller buffer *without zeroing it* — that orphans
    // PIN-bearing bytes in the allocator's freed pool until reuse. The
    // pre-sized Vec stays put; only the final `Zeroizing` drop runs.
    //
    // `Zeroizing` wraps the destination *before* `read_to_end` starts
    // writing into it, so an intermediate read failure (with partial
    // PIN bytes already in the buffer) still scrubs on the early-return
    // path.
    let len_hint = (meta.len() as usize).max(1);
    let mut bytes: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::with_capacity(len_hint));
    file.read_to_end(&mut bytes).map_err(|e| {
        MasterSeedError::Locked(format!(
            "PIN file {} read failed: {e}",
            config.pin_file.display()
        ))
    })?;

    // `trimmed: &str` is a borrow of `bytes` — no heap allocation, so
    // no separate scrub needed. The empty-check happens here, before
    // any owning `String` is constructed, so the early-return path
    // never has an unscrubbed PIN allocation to leak.
    let trimmed = std::str::from_utf8(&bytes)
        .map_err(|_| {
            MasterSeedError::Locked(format!(
                "PIN file {} is not UTF-8",
                config.pin_file.display()
            ))
        })?
        .trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
    if trimmed.is_empty() {
        return Err(MasterSeedError::Locked(format!(
            "PIN file {} is empty (after trim)",
            config.pin_file.display()
        )));
    }
    // Single expression — the `String` from `to_string()` is moved
    // directly into `AuthPin::from`. There is no named binding that
    // could drop without zeroing on an intermediate error path, so no
    // additional `Zeroizing<String>` wrapping is required. Inside
    // `AuthPin` the `String` is reused as a `Box<str>` by `SecretString`
    // (`secrecy 0.10`), which zeroes the buffer on its own drop.
    Ok(AuthPin::from(trimmed.to_string()))
}

/// Resolve the master seed object handle by `CKA_CLASS=SECRET_KEY`
/// and `CKA_LABEL=MASTER_SEED_LABEL`. Refuses if zero or more than
/// one match — ambiguity here is a deployment error (someone ran the
/// import ceremony twice and forgot to delete the old object).
fn find_master_seed(session: &Session, slot_id: u64) -> Result<ObjectHandle, MasterSeedError> {
    let template = vec![
        Attribute::Class(ObjectClass::SECRET_KEY),
        Attribute::Label(MASTER_SEED_LABEL.as_bytes().to_vec()),
    ];
    let handles = session.find_objects(&template).map_err(map_pkcs11_error)?;
    match handles.len() {
        0 => Err(MasterSeedError::DeriveFailed(format!(
            "no PKCS#11 secret object with CKA_LABEL='{}' on slot {}",
            MASTER_SEED_LABEL, slot_id
        ))),
        1 => Ok(handles[0]),
        n => Err(MasterSeedError::DeriveFailed(format!(
            "ambiguous: {} secret objects with CKA_LABEL='{}' on slot {} \
             (delete the duplicate via the import ceremony)",
            n, MASTER_SEED_LABEL, slot_id
        ))),
    }
}

impl MasterSeedHkdf for Pkcs11MasterSeedHkdf {
    fn derive_for(&self, info: &[u8], out: &mut [u8; 32]) -> Result<(), MasterSeedError> {
        // V1.11 M-5 — Mutex poison maps to `DeriveFailed`, not the
        // V1.10 wave-2 `Unavailable`. Operator runbook routes
        // `Unavailable` to "check HSM connectivity" (transient); a
        // poisoned mutex is permanent for *this process* (std locks
        // do not un-poison) so the only fix is a process restart. The
        // re-categorisation plus the explicit "restart required"
        // wording keeps the operator on the right remediation path.
        let session = self.session.lock().map_err(|_| map_session_lock_poison())?;

        // RFC 5869 HKDF-SHA256 with empty salt (= HashLen zeros, the
        // RFC default). Matches the dev impl's
        // `Hkdf::<Sha256>::new(None, seed)`. We pass `Some(HkdfSalt::Null)`
        // (not `None`) so the device executes the extract step; passing
        // `None` would tell `HkdfParams` to skip extract entirely, which
        // would NOT match the software HKDF semantics.
        //
        // PKCS#11 v3.0 §6.34 maps `HkdfParams::new(prf, Some(salt), Some(info))`
        // to `CK_HKDF_PARAMS { bExtract = CK_TRUE, bExpand = CK_TRUE,
        // prfHashMechanism = prf, ulSaltType = …, pInfo = info }`.
        // The pair `bExtract = CK_TRUE` + `ulSaltType = CKF_HKDF_SALT_NULL`
        // is the protocol-level encoding of "RFC 5869 with the default
        // HashLen-zeros salt", which is exactly what the verifier-side
        // software HKDF performs. If `salt` were `None`, cryptoki would
        // emit `bExtract = CK_FALSE` and the device would output
        // expand-only material — semantically distinct from the dev
        // path and a verification mismatch on the very first event.
        let params = HkdfParams::new(MechanismType::SHA256, Some(HkdfSalt::Null), Some(info));
        let mech = Mechanism::HkdfDerive(params);

        let template = derived_key_template();

        let derived = session
            .derive_key(&mech, self.master_seed, &template)
            .map_err(map_pkcs11_error)?;

        // V1.11 M-3 — RAII guard around the ephemeral derived-key
        // handle. The guard's `Drop` impl calls `destroy_object`, so
        // the cleanup runs on EVERY exit path: the `Ok` return below,
        // a `?` early-exit on `get_attributes` failure, the
        // length-mismatch error, OR a panic anywhere in the remainder
        // of this function (including FFI panics in cryptoki itself).
        //
        // The V1.10 wave-2 pattern stored `attr_result` separately and
        // called `destroy_object` between the call and the `?` — that
        // covered the explicit-Err path but not panic unwinding. The
        // RAII guard is the strictly stronger invariant and is also a
        // forward defence: if a future commit makes the derived object
        // CKA_TOKEN=true (e.g. for a debug build), the guarantee that
        // it gets destroyed becomes load-bearing rather than belt-and-
        // braces.
        let _guard = EphemeralObjectGuard {
            session: &session,
            handle: derived,
        };

        let attrs = session
            .get_attributes(derived, &[AttributeType::Value])
            .map_err(map_pkcs11_error)?;

        // Wrap the 32-byte HKDF output in `Zeroizing` the moment it
        // crosses the cryptoki FFI boundary. `attrs` is a `Vec<Attribute>`;
        // the `Attribute::Value(Vec<u8>)` variant heap-allocates the
        // derived bytes. Without zeroize, the buffer would linger in the
        // freed-allocator pool after `out.copy_from_slice` runs — visible
        // to a heap dump or freed-page reuse. The wrapper scrubs on every
        // exit path (Ok, Err, panic in the length check below).
        let value: Zeroizing<Vec<u8>> = Zeroizing::new(
            attrs
                .into_iter()
                .find_map(|a| match a {
                    Attribute::Value(v) => Some(v),
                    _ => None,
                })
                .ok_or_else(|| {
                    MasterSeedError::DeriveFailed(format!(
                        "PKCS#11 derived key returned no CKA_VALUE on slot {}",
                        self.slot_id
                    ))
                })?,
        );

        if value.len() != DERIVED_LEN {
            return Err(MasterSeedError::DeriveFailed(format!(
                "PKCS#11 derived key length {} ≠ expected {} on slot {}",
                value.len(),
                DERIVED_LEN,
                self.slot_id
            )));
        }

        out.copy_from_slice(&value);
        Ok(())
        // `_guard` drops here, calling `destroy_object` on `derived`.
    }
}

/// V1.11 M-3 — RAII guard that destroys an ephemeral PKCS#11 object on
/// drop. Used by [`Pkcs11MasterSeedHkdf::derive_for`] so the short-lived
/// derived-key handle is guaranteed to be released even if downstream
/// code panics (FFI panic in `get_attributes`, length-mismatch panic in
/// the value check, etc.).
///
/// The `destroy_object` error is intentionally swallowed: the caller
/// has no actionable response (the original derive already returned its
/// data and consumed the lock), and `CKR_OBJECT_HANDLE_INVALID` on an
/// already-destroyed object is benign. The PKCS#11 session itself is
/// the strict ceiling on orphaned objects (CKA_TOKEN=false → object
/// dies with session close) — the guard tightens the upper bound from
/// "session lifetime" to "single-derive-call lifetime".
///
/// The `'a` lifetime ties the guard to the `MutexGuard` borrow held
/// during `derive_for`; the borrow checker therefore guarantees the
/// session outlives the handle, so the `Drop` impl can safely call back
/// into cryptoki without a use-after-free.
struct EphemeralObjectGuard<'a> {
    session: &'a Session,
    handle: ObjectHandle,
}

impl Drop for EphemeralObjectGuard<'_> {
    fn drop(&mut self) {
        // Best-effort destroy. See struct doc for why the error is
        // swallowed; in short, the caller has already returned its
        // result and the session-close ceiling bounds the worst case.
        let _ = self.session.destroy_object(self.handle);
    }
}

/// V1.11 M-5 — categorisation helper for a poisoned session mutex.
///
/// Centralised in a free function so the production path
/// ([`Pkcs11MasterSeedHkdf::derive_for`]) and the regression test agree
/// on the exact wording. Returns [`MasterSeedError::DeriveFailed`] (not
/// the V1.10 wave-2 `Unavailable`) because mutex poison is permanent
/// for the affected process — `std::sync::Mutex` does not un-poison —
/// and the operator runbook's `Unavailable` advice (check token
/// connectivity, reseat the device) cannot fix poison. The "process
/// restart required" hint inside the message routes the operator to
/// the only remediation that actually works.
fn map_session_lock_poison() -> MasterSeedError {
    MasterSeedError::DeriveFailed(
        "PKCS#11 session mutex poisoned — a previous derive panicked; \
         process restart required (poison is permanent for this process)"
            .to_string(),
    )
}

/// Template for the ephemeral 32-byte derived key.
///
/// * `CKA_CLASS = CKO_SECRET_KEY` — symmetric secret object.
/// * `CKA_KEY_TYPE = CKK_GENERIC_SECRET` — opaque byte string;
///   `CKK_HKDF` would also work but `CKK_GENERIC_SECRET` is more
///   widely supported (SoftHSM2 + every commercial HSM).
/// * `CKA_VALUE_LEN = 32` — match the [`MasterSeedHkdf`] contract.
/// * `CKA_TOKEN = false` — ephemeral, lives only in the session;
///   `destroy_object` removes it; even without that, it's gone when
///   the process exits.
/// * `CKA_SENSITIVE = false` + `CKA_EXTRACTABLE = true` — we MUST
///   read `CKA_VALUE` back to caller-owned memory. The master seed
///   stays sensitive+non-extractable; only this short-lived
///   per-tenant derive is exportable.
/// * `CKA_DERIVE = false` — the per-tenant secret is not itself a
///   base key for further chained derivations; the workflow uses it
///   as Ed25519 seed material, full stop.
fn derived_key_template() -> Vec<Attribute> {
    vec![
        Attribute::Class(ObjectClass::SECRET_KEY),
        Attribute::KeyType(KeyType::GENERIC_SECRET),
        // cryptoki normalises CK_ULONG to a `u32` wrapper because
        // CK_ULONG is 4 bytes on Windows. `DERIVED_LEN = 32` fits in
        // a u32 trivially; the cast is checked at compile time.
        Attribute::ValueLen((DERIVED_LEN as u32).into()),
        Attribute::Token(false),
        Attribute::Sensitive(false),
        Attribute::Extractable(true),
        Attribute::Derive(false),
    ]
}

/// Compile-time guard: [`Pkcs11MasterSeedHkdf`] must be `Send + Sync`
/// so the V1.10 binary can wrap it in `Box<dyn MasterSeedHkdf>` and
/// share it across the per-tenant subcommand handlers. The trait
/// itself requires `Send + Sync`; this assertion catches a regression
/// (e.g. a `RefCell` field) at compile time rather than at the call
/// site.
const _: () = {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    let _ = assert_send::<Pkcs11MasterSeedHkdf>;
    let _ = assert_sync::<Pkcs11MasterSeedHkdf>;
};

#[cfg(test)]
mod tests {
    //! V1.11 W2 — pkcs11 hardening regression tests.
    //!
    //! These tests exercise the parts of the loader that DO NOT need a
    //! live PKCS#11 module:
    //!
    //!   * `read_pin_file` — file-system path, exercised end-to-end via
    //!     `tempfile`. The Unix-only mode-bit check is the M-2 surface.
    //!   * `map_session_lock_poison` — pure helper; the M-5
    //!     categorisation is a one-line policy that needs to stay
    //!     `DeriveFailed` across refactors.
    //!
    //! The M-3 RAII guard cannot be unit-tested without a live
    //! `cryptoki::session::Session` (which requires SoftHSM2 in the
    //! test runner). Its correctness is by construction — `Drop` runs
    //! on every exit path including panic — and the SoftHSM2 CI lane
    //! (V1.11 scope B candidate) is the operational verification.
    use super::*;

    /// V1.11 M-5 — mutex-poison categorisation contract. Operator
    /// runbook routes [`MasterSeedError::Unavailable`] to "check HSM
    /// connectivity" (transient) and [`MasterSeedError::DeriveFailed`]
    /// to "investigate / restart" (permanent for this process). Mutex
    /// poison is the latter — a previous derive panicked and the lock
    /// will not un-poison without a process restart. Pin the variant
    /// and the wording so a refactor cannot silently drift back to
    /// `Unavailable`.
    #[test]
    fn session_lock_poison_returns_derive_failed_v1_11_m5() {
        match map_session_lock_poison() {
            MasterSeedError::DeriveFailed(msg) => {
                assert!(
                    msg.contains("poison"),
                    "M-5: poison message must contain 'poison' so the operator \
                     can grep their logs for it; got: {msg}"
                );
                assert!(
                    msg.contains("restart"),
                    "M-5: poison message must say 'restart' so the operator knows \
                     the only effective remediation; got: {msg}"
                );
            }
            other => panic!(
                "V1.11 M-5 regression: mutex poison must map to DeriveFailed \
                 (Unavailable would route the operator to transient remediation \
                 that cannot fix a permanent poison); got {other:?}"
            ),
        }
    }

    /// V1.11 M-2 — refuse a PIN file with the most common operator
    /// footgun mode (0o644: default umask 022 + plain `echo > file`).
    /// Without this guard, any local user with read access to the
    /// secret-mount dir can read the PIN and authenticate to the token
    /// behind the operator's back.
    #[cfg(unix)]
    #[test]
    fn read_pin_file_refuses_world_readable_v1_11_m2() {
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        let dir = tempfile::tempdir().expect("tempdir");
        let pin_path = dir.path().join("hsm.pin");
        std::fs::write(&pin_path, b"1234").expect("write pin");
        std::fs::set_permissions(&pin_path, std::fs::Permissions::from_mode(0o644))
            .expect("chmod 0644");

        // module_path is unused by `read_pin_file`; the path just has
        // to be syntactically valid for the `HsmConfig` struct. The
        // mode-bit check happens before any module load.
        let cfg = HsmConfig {
            module_path: PathBuf::from("/nonexistent/lib.so"),
            slot: 0,
            pin_file: pin_path,
        };

        match read_pin_file(&cfg) {
            Err(MasterSeedError::Locked(msg)) => {
                assert!(
                    msg.contains("0644") || msg.contains("permission"),
                    "M-2 error must surface the offending mode or the word \
                     'permission' so the operator understands what's wrong; \
                     got: {msg}"
                );
                assert!(
                    msg.contains("chmod") || msg.contains("0400"),
                    "M-2 error must hint the chmod remediation so the operator \
                     can act without consulting the runbook; got: {msg}"
                );
            }
            Ok(_) => panic!(
                "V1.11 M-2 regression: a 0644 PIN file was silently accepted \
                 — local-user PIN exfiltration vector"
            ),
            Err(other) => panic!(
                "V1.11 M-2: 0644 PIN file should yield Locked (auth material \
                 unusable), got {other:?}"
            ),
        }
    }

    /// V1.11 M-2 — accept the runbook ceremony's `install -m 0400`
    /// output and the systemd `LoadCredential=` tmpfs default 0o600.
    /// Both are well-known correctly-secured layouts for a PIN file
    /// and refusing them would break the documented deployments.
    #[cfg(unix)]
    #[test]
    fn read_pin_file_accepts_owner_only_modes_v1_11_m2() {
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        for mode in [0o400u32, 0o600u32] {
            let dir = tempfile::tempdir().expect("tempdir");
            let pin_path = dir.path().join("hsm.pin");
            std::fs::write(&pin_path, b"1234").expect("write pin");
            std::fs::set_permissions(&pin_path, std::fs::Permissions::from_mode(mode))
                .expect("chmod owner-only");

            let cfg = HsmConfig {
                module_path: PathBuf::from("/nonexistent/lib.so"),
                slot: 0,
                pin_file: pin_path,
            };

            assert!(
                read_pin_file(&cfg).is_ok(),
                "V1.11 M-2: mode {mode:#o} is a valid owner-only layout and \
                 must be accepted by the loader (refusing it would block the \
                 documented runbook ceremony output)"
            );
        }
    }

    /// V1.11 M-2 — exhaustive sweep of the lower-6 mode bits to ensure
    /// the guard refuses ANY group/world access bit. Catches a future
    /// relaxation (e.g. `mode & 0o007 != 0` instead of `mode & 0o077`)
    /// that would silently allow group-readable PIN files.
    #[cfg(unix)]
    #[test]
    fn read_pin_file_refuses_any_group_or_other_bit_v1_11_m2() {
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        // Every combination of group (0o070) + other (0o007) bits set,
        // owner unrestricted (0o600). Skip 0o600 itself (the legitimate
        // baseline tested above) — every other combination must refuse.
        let bad_modes: [u32; 9] = [
            0o604, 0o602, 0o601, // owner+other variants
            0o640, 0o620, 0o610, // owner+group variants
            0o644, 0o660, 0o666, // owner+group+other classics
        ];
        for bad in bad_modes {
            let dir = tempfile::tempdir().expect("tempdir");
            let pin_path = dir.path().join("hsm.pin");
            std::fs::write(&pin_path, b"1234").expect("write pin");
            std::fs::set_permissions(&pin_path, std::fs::Permissions::from_mode(bad))
                .expect("chmod bad");

            let cfg = HsmConfig {
                module_path: PathBuf::from("/nonexistent/lib.so"),
                slot: 0,
                pin_file: pin_path,
            };

            match read_pin_file(&cfg) {
                Err(MasterSeedError::Locked(_)) => (),
                Ok(_) => panic!(
                    "V1.11 M-2 regression: mode {bad:#o} was accepted — group \
                     or other access leaks the PIN to local users"
                ),
                Err(other) => panic!(
                    "V1.11 M-2: mode {bad:#o} should yield Locked, got {other:?}"
                ),
            }
        }
    }

    /// V1.11 M-2 — sanity that the read path still surfaces the
    /// existing empty-file diagnostic (regression guard for the
    /// pre-V1.11 behaviour). Uses 0o400 to clear the mode-bit gate.
    #[cfg(unix)]
    #[test]
    fn read_pin_file_still_rejects_empty_file_after_m2() {
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        let dir = tempfile::tempdir().expect("tempdir");
        let pin_path = dir.path().join("hsm.pin");
        std::fs::write(&pin_path, b"").expect("write empty pin");
        std::fs::set_permissions(&pin_path, std::fs::Permissions::from_mode(0o400))
            .expect("chmod 0400");

        let cfg = HsmConfig {
            module_path: PathBuf::from("/nonexistent/lib.so"),
            slot: 0,
            pin_file: pin_path,
        };

        match read_pin_file(&cfg) {
            Err(MasterSeedError::Locked(msg)) => assert!(
                msg.contains("empty"),
                "empty-file diagnostic must survive the M-2 refactor; got: {msg}"
            ),
            other => panic!("expected Locked(empty) after M-2; got {other:?}"),
        }
    }
}
