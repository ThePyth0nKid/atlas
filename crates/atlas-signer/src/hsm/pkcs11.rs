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
    /// use-after-free in cryptoki. Declared LAST in the struct so
    /// it is dropped LAST: declaration order = drop order in Rust,
    /// and the session must close before `C_Finalize` runs.
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
/// `Locked` is the right category for a missing or unreadable PIN
/// file — the token itself may be perfectly reachable; the operator
/// just hasn't supplied auth material the loader can use.
fn read_pin_file(config: &HsmConfig) -> Result<AuthPin, MasterSeedError> {
    // Wrap the raw bytes in `Zeroizing` immediately. `std::fs::read`
    // allocates a heap `Vec<u8>`; if we drop it without zeroing, the
    // PIN bytes linger in the freed-allocator pool until reused. The
    // wrapper zeroes the buffer on every exit path (Ok, Err, panic).
    let bytes: Zeroizing<Vec<u8>> = Zeroizing::new(std::fs::read(&config.pin_file).map_err(|e| {
        MasterSeedError::Locked(format!(
            "PIN file {} unreadable: {e}",
            config.pin_file.display()
        ))
    })?);
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
        let session = self.session.lock().map_err(|_| {
            MasterSeedError::Unavailable(
                "PKCS#11 session mutex poisoned — a previous derive panicked"
                    .to_string(),
            )
        })?;

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

        // `?` not used here so we can fall through and destroy the
        // ephemeral object on the error path. We trade one extra
        // statement for a tighter security invariant.
        let attr_result = session.get_attributes(derived, &[AttributeType::Value]);

        // Always destroy the ephemeral derived key, even if attribute
        // extraction failed in some weird way. We deliberately ignore
        // the destroy error — at worst the object lingers until the
        // session closes; at best it's already gone.
        let _ = session.destroy_object(derived);

        let attrs = attr_result.map_err(map_pkcs11_error)?;

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
    }
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
