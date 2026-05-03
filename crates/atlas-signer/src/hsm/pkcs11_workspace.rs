//! V1.11 Scope A wave-3 Phase B — sealed PKCS#11 per-workspace signer.
//!
//! Wave 2 ([`crate::hsm::pkcs11::Pkcs11MasterSeedHkdf`]) sealed the
//! *master seed*: HKDF derivation runs inside the token, but the
//! per-workspace 32-byte HKDF output crosses back into Atlas address
//! space so [`ed25519_dalek::SigningKey::from_bytes`] can build a
//! signing key on the host. That residual surface is what wave 3 closes.
//!
//! [`Pkcs11WorkspaceSigner`] implements [`WorkspaceSigner`] by holding
//! the per-tenant Ed25519 *private key itself* inside the PKCS#11
//! token. Each workspace_id maps to a persistent CKK_EC_EDWARDS object
//! generated via `CKM_EC_EDWARDS_KEY_PAIR_GEN` (Token=true,
//! Sensitive=true, Extractable=false); the corresponding public-key
//! object holds CKA_EC_POINT for fast pubkey reads. Signing routes
//! through `CKM_EDDSA(Ed25519)` — the device produces the RFC 8032
//! deterministic signature without the secret scalar ever leaving the
//! token. The host side sees `[u8; 32]` pubkeys and `[u8; 64]`
//! signatures — both pure public material.
//!
//! ## Lifecycle: find-or-generate, then cache
//!
//! On the first `sign` / `pubkey` call for a workspace_id:
//!
//!   1. Look up the per-workspace label
//!      `"atlas-workspace-key-v1:<workspace_id>"` via `C_FindObjects`.
//!   2. If both private + public objects exist → use them.
//!   3. If neither exists → call `C_GenerateKeyPair` with the wave-3
//!      production template and persist as Token=true.
//!   4. If exactly one half exists → refuse with `DeriveFailed`. An
//!      orphaned half-keypair is a deployment-error signal that the
//!      operator must resolve via the runbook (it should never happen
//!      under the documented ceremony).
//!   5. Read CKA_EC_POINT once and cache the 32-byte pubkey alongside
//!      the handles.
//!
//! Subsequent calls hit the in-process `HashMap<String, CachedKey>`
//! cache and skip the PKCS#11 round-trip entirely (handles + pubkey
//! bytes are all the cache needs). Per the trait contract, sealed-key
//! impls SHOULD cache: a naive impl that re-derives on every call
//! would pay one network round-trip per signature on a remote HSM,
//! which is enough to tank an MCP-server's per-second throughput.
//!
//! ## Why labels with `:`
//!
//! [`crate::keys::validate_workspace_id`] forbids `:` inside
//! workspace_id, so the colon delimiter between the
//! `"atlas-workspace-key-v1"` namespace and the workspace_id payload is
//! unambiguous — any `:` in the label after the prefix could only be a
//! prefix terminator, never part of a workspace_id. Sharing the label
//! grammar with the wave-2 master_seed label scheme
//! (`"atlas-master-seed-v1"`) keeps the on-token namespace consistent.
//!
//! ## Threading model
//!
//! `Mutex<SignerState>` wraps both the long-lived authenticated session
//! AND the per-workspace handle cache. Concurrent calls serialise on
//! the same lock — necessary because cryptoki sessions are not safe to
//! drive from multiple threads in parallel on most modules anyway, AND
//! because the cache's "find-or-generate" step needs atomicity to
//! prevent two simultaneous misses from racing into duplicate
//! `C_GenerateKeyPair` calls. A separate `RwLock<HashMap>` would only
//! help the all-cache-hit path, but that path still bottlenecks on the
//! session — net win is zero.
//!
//! ## Drop ordering
//!
//! Same invariant as wave-2 [`Pkcs11MasterSeedHkdf`]: declaration order
//! puts `state` (which owns the [`Session`]) before `_ctx`, so the
//! session destructor (`C_CloseSession`) runs before the module
//! destructor (`C_Finalize`). Reordering the fields would invert the
//! sequence and produce a use-after-free on every drop.

use std::collections::HashMap;
use std::sync::Mutex;

use cryptoki::context::{CInitializeArgs, CInitializeFlags, Pkcs11};
use cryptoki::error::{Error as Pkcs11Error, RvError};
use cryptoki::mechanism::eddsa::{EddsaParams, EddsaSignatureScheme};
use cryptoki::mechanism::Mechanism;
use cryptoki::object::{Attribute, AttributeType, KeyType, ObjectClass, ObjectHandle};
use cryptoki::session::{Session, UserType};

use crate::hsm::config::HsmConfig;
use crate::hsm::error::map_pkcs11_error;
use crate::hsm::pkcs11::read_pin_file;
use crate::keys::validate_workspace_id;
use crate::workspace_signer::{WorkspaceSigner, WorkspaceSignerError};

/// CKA_LABEL prefix prepended to the workspace_id when forming the
/// per-tenant key object's label. The full label is
/// `format!("{WORKSPACE_LABEL_PREFIX}{workspace_id}")`.
///
/// [`validate_workspace_id`] forbids `:` inside workspace_id, so the
/// colon between the version prefix and the workspace_id is an
/// unambiguous delimiter — no possible workspace_id can produce a label
/// that re-parses to a different `(prefix, workspace_id)` split.
///
/// **Versioning.** The `-v1` suffix is a rotation hatch: any future
/// change to the on-token derivation grammar (e.g. storing the
/// workspace_id as a SHA-256 prefix to defend against label-length
/// limits on certain HSMs) bumps this constant to `-v2`. Old `-v1`
/// objects keep working for legacy workspaces; new ones go through the
/// `-v2` namespace. Mirrors the [`crate::keys::HKDF_INFO_PREFIX`]
/// `atlas-anchor-v1:` versioning convention.
const WORKSPACE_LABEL_PREFIX: &str = "atlas-workspace-key-v1:";

/// ASN.1 DER for the printable string `"edwards25519"` — the
/// `CKA_EC_PARAMS` value SoftHSM2/OpenSSL expects for Ed25519 key
/// generation. Tag 0x13 = PrintableString, length 0x0c = 12, then the
/// 12 ASCII bytes. The OID encoding (1.3.101.112) also works on
/// SoftHSM2 ≥ 2.6, but the printable-string form matches what the
/// wave-2 byte-equivalence test imports against and what the cryptoki
/// integration tests use — strongest cross-vendor consistency.
const ED25519_PARAMS_PRINTABLE: [u8; 14] = [
    0x13, 0x0c, b'e', b'd', b'w', b'a', b'r', b'd', b's', b'2', b'5', b'5', b'1', b'9',
];

/// Sealed per-workspace signer backed by PKCS#11.
///
/// **Drop order matters.** Rust drops fields in declaration order, so
/// `state` (which owns the [`Session`]) drops first — closing the
/// PKCS#11 session via `C_CloseSession` — and `_ctx` drops last —
/// finalising the module via `C_Finalize`. Reordering the fields would
/// `C_Finalize` the module while a session is still open, which is
/// undefined behaviour per PKCS#11 §5.4 and produces a use-after-free
/// in cryptoki. Mirrors the wave-2 [`Pkcs11MasterSeedHkdf`] pattern.
#[derive(Debug)]
pub struct Pkcs11WorkspaceSigner {
    /// Authenticated session + per-workspace handle cache. Single
    /// [`Mutex`] because the cache + session lifecycle need atomic
    /// "find-or-generate" semantics (see module docs).
    state: Mutex<SignerState>,
    /// Slot id, retained for diagnostic messages. Read-only after
    /// [`Self::open`] returns.
    slot_id: u64,
    /// PKCS#11 module context. Held only to keep the dynamic library
    /// loaded for the signer's lifetime; never accessed after `open`
    /// returns. Underscored to mute the dead-field lint without dropping
    /// the field early — drop order documented at the struct level.
    _ctx: Pkcs11,
}

/// Authenticated session + per-workspace handle cache, locked
/// together. The session and cache need a shared lock for two reasons:
///
///   1. Cryptoki sessions cannot be driven from multiple threads in
///      parallel on most modules.
///   2. Find-or-generate must be atomic — two concurrent misses on the
///      same workspace_id would otherwise race into duplicate
///      `C_GenerateKeyPair` calls and create a half-keypair pair,
///      which the wave-3 invariant refuses to load on the next open.
#[derive(Debug)]
struct SignerState {
    session: Session,
    /// Workspace_id → cached object handles + pubkey bytes. Populated
    /// lazily on the first `sign` / `pubkey` for each workspace.
    /// `String` key (not `&str`) because the cache outlives any
    /// caller-supplied `&str`.
    cache: HashMap<String, CachedKey>,
}

/// Per-workspace cached state. Object handles are
/// [`ObjectHandle`]-which-is-Copy, so the whole struct is `Copy` and
/// the cache lookup can hand out a copy without re-borrowing the
/// underlying `HashMap`. Pubkey bytes are cached because reading
/// `CKA_EC_POINT` is a PKCS#11 round-trip and the bytes never change
/// for a given key — a single read at find-or-generate time covers all
/// future `pubkey()` calls.
///
/// The public-key object handle is intentionally NOT cached: signing
/// goes through the private handle, and `pubkey()` reads from
/// `pubkey_bytes` (the already-extracted CKA_EC_POINT). A future
/// "destroy workspace key" or "rotate workspace key" operation will
/// re-resolve both halves via `find_one_object` rather than caching
/// the handle here — that resolution is a one-shot operator action,
/// not a hot-path call, so the round-trip cost is negligible and the
/// cache stays minimal.
#[derive(Debug, Clone, Copy)]
struct CachedKey {
    private: ObjectHandle,
    pubkey_bytes: [u8; 32],
}

impl Pkcs11WorkspaceSigner {
    /// Open the configured PKCS#11 module, log in, and return a signer
    /// with an empty per-workspace cache.
    ///
    /// Unlike wave-2's [`Pkcs11MasterSeedHkdf::open`], this constructor
    /// does NOT resolve any object handle up front. Per-workspace keys
    /// are looked up (or generated) on first use, so a deployment with
    /// 1000 workspaces does not pay 1000 `C_FindObjects` round-trips at
    /// boot. The trade is one round-trip on the first call per
    /// workspace; subsequent calls hit the in-process cache.
    ///
    /// Errors mirror wave-2's categorisation: `Unavailable` for module
    /// load + slot resolution, `Locked` for PIN file + login failures,
    /// `DeriveFailed` for unexpected PKCS#11 return values during
    /// session setup. Surfacing through [`map_pkcs11_error`] +
    /// [`From<MasterSeedError>`] keeps the operator-facing remediation
    /// text uniform across wave-2 and wave-3.
    pub fn open(config: HsmConfig) -> Result<Self, WorkspaceSignerError> {
        let ctx = Pkcs11::new(&config.module_path)
            .map_err(|e| WorkspaceSignerError::from(map_pkcs11_error(e)))?;
        ctx.initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
            .map_err(|e| WorkspaceSignerError::from(map_pkcs11_error(e)))?;

        // PKCS#11 doesn't let us construct a `Slot` directly; we
        // enumerate slots-with-token and pick the one whose id matches
        // the operator's `ATLAS_HSM_SLOT`. Same pattern as wave-2.
        let slot = ctx
            .get_slots_with_token()
            .map_err(|e| WorkspaceSignerError::from(map_pkcs11_error(e)))?
            .into_iter()
            .find(|s| s.id() == config.slot)
            .ok_or_else(|| {
                WorkspaceSignerError::Unavailable(format!(
                    "PKCS#11 slot {} not present in module {}",
                    config.slot,
                    config.module_path.display()
                ))
            })?;

        // RW session is required: `C_GenerateKeyPair` writes a Token=true
        // object to the slot. SoftHSM2 enforces this; commercial HSMs do
        // too. Wave-2's master-seed loader also opens RW (for
        // `C_DeriveKey`'s ephemeral object), so the requirement is
        // consistent across the two modes.
        let session = ctx
            .open_rw_session(slot)
            .map_err(|e| WorkspaceSignerError::from(map_pkcs11_error(e)))?;

        // PIN read goes through the wave-2 hardened reader (TOCTOU-safe
        // FD-based metadata + Unix mode 0400/0600 guard +
        // `Zeroizing<Vec<u8>>`). Sharing the function across waves means
        // any future hardening (e.g. owner-uid match) lands once and
        // covers both signers automatically. Mapping `MasterSeedError`
        // → `WorkspaceSignerError` via `From` preserves the Locked
        // categorisation that the runbook ties to PIN remediation.
        let pin = read_pin_file(&config).map_err(WorkspaceSignerError::from)?;
        session
            .login(UserType::User, Some(&pin))
            .map_err(|e| WorkspaceSignerError::from(map_pkcs11_error(e)))?;

        Ok(Self {
            state: Mutex::new(SignerState {
                session,
                cache: HashMap::new(),
            }),
            slot_id: config.slot,
            _ctx: ctx,
        })
    }
}

impl WorkspaceSigner for Pkcs11WorkspaceSigner {
    fn sign(
        &self,
        workspace_id: &str,
        signing_input: &[u8],
    ) -> Result<[u8; 64], WorkspaceSignerError> {
        // Trait contract: validate workspace_id BEFORE any key-material
        // operation. Refusal here aborts before we touch the lock or the
        // PKCS#11 session — same defence as the dev impl, applied
        // uniformly across the trait surface.
        validate_workspace_id(workspace_id).map_err(WorkspaceSignerError::DeriveFailed)?;

        let mut state = self.state.lock().map_err(|_| map_session_lock_poison())?;
        let cached = find_or_generate(&mut state, workspace_id, self.slot_id)?;

        // CKM_EDDSA(Ed25519) is the deterministic-signing mechanism
        // from PKCS#11 v3.0 §6.5.5. The signature is the raw 64-byte
        // RFC 8032 signature (R || S, 32 + 32 bytes); same wire-shape
        // the dev impl produces via `ed25519_dalek::SigningKey::sign`.
        // EddsaParams::new(scheme) is the per-call configuration —
        // recreated each call because cryptoki consumes the params
        // by value and they are cheap to construct.
        let mech = Mechanism::Eddsa(EddsaParams::new(EddsaSignatureScheme::Ed25519));
        let signature = state
            .session
            .sign(&mech, cached.private, signing_input)
            .map_err(map_pkcs11_sign_error)?;

        // RFC 8032 §5.1.6 fixes Ed25519 signatures at exactly 64 bytes.
        // A different length here means the HSM is producing
        // a non-Ed25519 signature under the CKM_EDDSA mechanism — fail
        // loud rather than truncate-or-extend. `SigningFailed` (not
        // `DeriveFailed`) because the key itself was usable; the
        // signing op went off the rails.
        signature.try_into().map_err(|v: Vec<u8>| {
            WorkspaceSignerError::SigningFailed(format!(
                "PKCS#11 CKM_EDDSA returned {} bytes for workspace {workspace_id:?} on slot {}; \
                 RFC 8032 §5.1.6 mandates 64-byte signatures",
                v.len(),
                self.slot_id,
            ))
        })
    }

    fn pubkey(&self, workspace_id: &str) -> Result<[u8; 32], WorkspaceSignerError> {
        // Same workspace_id contract as `sign`. The pubkey path is
        // public-out, but unbounded workspace_id still costs a
        // PKCS#11 round-trip (find-or-generate) on the first call —
        // the validator is the cheapest cutoff.
        validate_workspace_id(workspace_id).map_err(WorkspaceSignerError::DeriveFailed)?;

        let mut state = self.state.lock().map_err(|_| map_session_lock_poison())?;
        let cached = find_or_generate(&mut state, workspace_id, self.slot_id)?;
        // pubkey_bytes was extracted at find-or-generate time and
        // cached. The `Copy` trait on `CachedKey` means we hand back a
        // bitwise copy without re-borrowing the HashMap, so the lock
        // can drop the moment this function returns.
        Ok(cached.pubkey_bytes)
    }
}

/// Cache-or-fetch the per-workspace key handles. On a cache miss,
/// resolves via `C_FindObjects` (existing key) or `C_GenerateKeyPair`
/// (fresh key). Inserts into the cache before returning so the next
/// call is O(1).
///
/// **Atomicity.** The caller holds `&mut SignerState`, which means
/// the [`Mutex`] is locked for the duration of this function. That is
/// the load-bearing invariant for find-or-generate: two concurrent
/// misses on the same workspace_id cannot race into two
/// `C_GenerateKeyPair` calls because they cannot both hold the lock.
/// A separate `RwLock<HashMap>` for the cache would break this
/// guarantee.
fn find_or_generate(
    state: &mut SignerState,
    workspace_id: &str,
    slot_id: u64,
) -> Result<CachedKey, WorkspaceSignerError> {
    // Defence-in-depth boundary check. Today every public entry point
    // (`sign`, `pubkey`) calls `validate_workspace_id` before reaching
    // here, so this is redundant on the happy path. But `find_or_generate`
    // is a private free function whose only protection from a future
    // unvalidated caller (e.g. a `rotate_workspace_key` convenience added
    // in Phase C) is this assertion. Re-validating cheaply here pins the
    // invariant to the function instead of trusting the call graph.
    validate_workspace_id(workspace_id).map_err(WorkspaceSignerError::DeriveFailed)?;
    if let Some(cached) = state.cache.get(workspace_id) {
        // `CachedKey: Copy` so we return by value without holding a
        // borrow into the HashMap, freeing `state` for subsequent
        // mutable access in the same lock scope.
        return Ok(*cached);
    }
    let label = format!("{WORKSPACE_LABEL_PREFIX}{workspace_id}");
    let private = find_one_object(&state.session, ObjectClass::PRIVATE_KEY, &label, slot_id)?;
    let public = find_one_object(&state.session, ObjectClass::PUBLIC_KEY, &label, slot_id)?;
    let cached = match (private, public) {
        (Some(priv_h), Some(pub_h)) => CachedKey {
            private: priv_h,
            pubkey_bytes: read_ec_point_bytes(&state.session, pub_h, &label, slot_id)?,
        },
        (None, None) => generate_keypair(&state.session, &label, slot_id)?,
        (Some(_), None) | (None, Some(_)) => {
            // Half-keypair signal: one of the two halves got destroyed
            // or never made it onto the token. The wave-3 invariant
            // refuses to (a) regenerate (which would orphan the
            // surviving half and create a third object), or (b) sign
            // with the surviving half (private without public is signable
            // but the operator has no pubkey to advertise; public without
            // private is unusable). The runbook documents the manual
            // dedupe ceremony.
            return Err(WorkspaceSignerError::DeriveFailed(format!(
                "PKCS#11 token slot {slot_id} has an orphaned half-keypair for label {label:?} \
                 (one of CKO_PRIVATE_KEY / CKO_PUBLIC_KEY exists without the other); \
                 manual cleanup required — see operator runbook for the half-keypair dedupe procedure",
            )));
        }
    };
    state.cache.insert(workspace_id.to_string(), cached);
    Ok(cached)
}

/// Resolve a single object handle by `(class, label)`. Refuses both
/// the not-found case (returns `Ok(None)` so `find_or_generate` can
/// branch into key generation) and the ambiguity case (returns `Err`
/// with operator remediation).
///
/// Restricting the find template to `(class, key_type, label)` rather
/// than just `(class, label)` is defence in depth: a future operator
/// might accidentally create a non-Ed25519 object under our label
/// namespace, and matching by key_type ensures we never sign with the
/// wrong algorithm.
fn find_one_object(
    session: &Session,
    class: ObjectClass,
    label: &str,
    slot_id: u64,
) -> Result<Option<ObjectHandle>, WorkspaceSignerError> {
    let template = vec![
        Attribute::Class(class),
        Attribute::KeyType(KeyType::EC_EDWARDS),
        Attribute::Label(label.as_bytes().to_vec()),
    ];
    let handles = session
        .find_objects(&template)
        .map_err(|e| WorkspaceSignerError::from(map_pkcs11_error(e)))?;
    match handles.len() {
        0 => Ok(None),
        1 => Ok(Some(handles[0])),
        n => Err(WorkspaceSignerError::DeriveFailed(format!(
            "PKCS#11 token slot {slot_id} returned {n} objects for class {class:?} label {label:?}; \
             ambiguous — operator must dedupe via the runbook before signing can proceed",
        ))),
    }
}

/// Generate a fresh Ed25519 keypair on the token via
/// `CKM_EC_EDWARDS_KEY_PAIR_GEN`, persist as Token=true, and read
/// CKA_EC_POINT to populate the cached pubkey.
///
/// **Production template choices** (each one is load-bearing — a
/// silent change here is a security-relevant deviation):
///
/// * `Token=true` — persistent. The wave-3 design point is that keys
///   live across process restarts; without persistence the per-tenant
///   pubkey would rotate every time the signer restarted, breaking
///   verifier-side trust pinning.
/// * `Sensitive=true` — extraction refused even by an authenticated
///   user. Combined with `Extractable=false` this means the secret
///   scalar genuinely never leaves the token, even via a future bug
///   that calls `get_attributes(CKA_VALUE)`.
/// * `Extractable=false` — wrap-extract refused. Without this, an
///   attacker with the operator's PIN could `C_WrapKey` the private
///   half to their own public key and exfiltrate. Pairs with Sensitive.
/// * `Sign=true` on private, `Verify=true` on public — explicit
///   capability bits. Some HSMs default these conservatively; pinning
///   them avoids surprise refusals at the per-call sign step.
/// * `Private=false` on public — public-key objects are not themselves
///   secret material; opening them up means the operator can list and
///   audit the on-token pubkey set without `C_Login`.
/// * `Label=...` on both — uniform label so `find_one_object` can
///   resolve the pair on the next session.
fn generate_keypair(
    session: &Session,
    label: &str,
    slot_id: u64,
) -> Result<CachedKey, WorkspaceSignerError> {
    // PKCS#11 §10.1.2: a public/private key pair is bound by sharing the
    // same `CKA_ID` value. We set CKA_ID = CKA_LABEL on both objects so
    // standard tooling (`pkcs11-tool --list-key-types`, vendor CryptoKi
    // toolkits, the operator-runbook half-keypair dedupe ceremony) can
    // resolve the pair without our `find_one_object` disambiguation
    // shortcut. The label itself already encodes the workspace_id, so
    // this is byte-identical to the label and does not introduce a
    // second naming source — just makes the spec-mandated pair binding
    // explicit instead of relying on label-equality alone.
    let id = label.as_bytes().to_vec();
    let pub_template = vec![
        Attribute::Class(ObjectClass::PUBLIC_KEY),
        Attribute::KeyType(KeyType::EC_EDWARDS),
        Attribute::Token(true),
        Attribute::Private(false),
        Attribute::Verify(true),
        Attribute::EcParams(ED25519_PARAMS_PRINTABLE.to_vec()),
        Attribute::Label(label.as_bytes().to_vec()),
        Attribute::Id(id.clone()),
    ];
    let priv_template = vec![
        Attribute::Class(ObjectClass::PRIVATE_KEY),
        Attribute::KeyType(KeyType::EC_EDWARDS),
        Attribute::Token(true),
        Attribute::Private(true),
        Attribute::Sign(true),
        Attribute::Sensitive(true),
        Attribute::Extractable(false),
        // Defence-in-depth: `Derive=false` blocks `C_DeriveKey` against
        // this private key. PKCS#11 lets a base key with `CKA_DERIVE=true`
        // serve as input to derivations whose output may be exportable —
        // an indirect way to leak material from a `Sensitive=true`,
        // `Extractable=false` key. Some HSMs default to `CKA_DERIVE=true`
        // for freshly-generated EC private keys; pinning `false` here
        // matches the wave-2 sealed-seed template's `derived_key_template()`
        // policy and slams the door shut.
        Attribute::Derive(false),
        Attribute::Label(label.as_bytes().to_vec()),
        Attribute::Id(id),
    ];
    let (public, private) = session
        .generate_key_pair(
            &Mechanism::EccEdwardsKeyPairGen,
            &pub_template,
            &priv_template,
        )
        .map_err(|e| WorkspaceSignerError::from(map_pkcs11_error(e)))?;
    let pubkey_bytes = read_ec_point_bytes(session, public, label, slot_id)?;
    // `public` handle is not retained: pubkey_bytes is the only data
    // the hot path needs, and a future destroy/rotate operation will
    // re-resolve via find_one_object (one-shot operator action, not a
    // hot-path call). The handle itself goes out of scope here; the
    // underlying CKO_PUBLIC_KEY object remains on the token (Token=true).
    let _ = public;
    Ok(CachedKey {
        private,
        pubkey_bytes,
    })
}

/// Read CKA_EC_POINT from a public-key object and unwrap to the raw
/// 32-byte Ed25519 compressed point. PKCS#11 v3.0 §10.10 mandates
/// the OCTET-STRING-wrapped form (tag 0x04, length 0x20, 32 raw bytes
/// = 34 total), but several commercial HSMs and SoftHSM2 ≤ 2.5 ship
/// the raw-32-byte form — the unwrap helper accepts both shapes.
fn read_ec_point_bytes(
    session: &Session,
    public: ObjectHandle,
    label: &str,
    slot_id: u64,
) -> Result<[u8; 32], WorkspaceSignerError> {
    let attrs = session
        .get_attributes(public, &[AttributeType::EcPoint])
        .map_err(|e| WorkspaceSignerError::from(map_pkcs11_error(e)))?;
    let raw = attrs
        .into_iter()
        .find_map(|a| match a {
            Attribute::EcPoint(v) => Some(v),
            _ => None,
        })
        .ok_or_else(|| {
            WorkspaceSignerError::DeriveFailed(format!(
                "PKCS#11 public-key object for label {label:?} on slot {slot_id} \
                 returned no CKA_EC_POINT attribute"
            ))
        })?;
    unwrap_octet_string(&raw, label, slot_id)
}

/// Unwrap CKA_EC_POINT to the raw 32-byte Ed25519 point. Accepts:
///
///   * 32 bytes raw — non-spec-compliant but common (SoftHSM2 ≤ 2.5,
///     several commercial HSMs).
///   * 34 bytes wrapped as ASN.1 OCTET STRING `[0x04, 0x20, ..raw..]`
///     — the PKCS#11 v3.0 §10.10 mandated form.
///
/// Anything else is an HSM/cryptoki encoding bug and surfaces as
/// `DeriveFailed` with the offending bytes summarised so the operator
/// can file a vendor ticket.
fn unwrap_octet_string(
    bytes: &[u8],
    label: &str,
    slot_id: u64,
) -> Result<[u8; 32], WorkspaceSignerError> {
    if bytes.len() == 32 {
        return Ok(bytes.try_into().expect("len-32 slice → [u8; 32]"));
    }
    // ASN.1 OCTET STRING: tag 0x04, length 0x20 (= 32 decimal), 32 raw
    // bytes. Spell the length byte in hex to mirror the comment block
    // above and avoid a future reviewer reading `== 32` as the wrong
    // ASN.1 length encoding.
    if bytes.len() == 34 && bytes[0] == 0x04 && bytes[1] == 0x20 {
        return Ok(bytes[2..34].try_into().expect("len-32 slice → [u8; 32]"));
    }
    Err(WorkspaceSignerError::DeriveFailed(format!(
        "PKCS#11 CKA_EC_POINT for label {label:?} on slot {slot_id} has unexpected shape: \
         {} bytes, first 2 = [{:#x}, {:#x}] (expected 32 raw or 34 wrapped [0x04, 0x20, ..])",
        bytes.len(),
        bytes.first().copied().unwrap_or(0),
        bytes.get(1).copied().unwrap_or(0),
    )))
}

/// Map a `cryptoki::error::Error` from the `C_Sign` call site to a
/// [`WorkspaceSignerError`].
///
/// **Variant cleaving rule**, applied to the sign() path:
///
///   * Locked-class RVs (PIN/session) → `Locked` — operator
///     re-authenticates and retries.
///   * Unavailable-class RVs (token gone) → `Unavailable` — operator
///     fixes connectivity.
///   * Anything else inside a Sign call → `SigningFailed`. Per the
///     trait doc: "When in doubt, SigningFailed is the safer default
///     for a sign() path because it implies retry is meaningful." The
///     wave-2 [`map_pkcs11_error`] helper would route the same RVs
///     to `DeriveFailed`, which would (correctly for derive paths)
///     send the operator down a "provision the key" remediation that
///     does not apply here — the key was findable / generatable by
///     definition (we just used it via `cached.private`).
fn map_pkcs11_sign_error(err: Pkcs11Error) -> WorkspaceSignerError {
    match err {
        Pkcs11Error::Pkcs11(rv, _) => match rv {
            RvError::UserNotLoggedIn
            | RvError::PinIncorrect
            | RvError::PinLocked
            | RvError::PinExpired
            | RvError::PinInvalid
            | RvError::SessionHandleInvalid
            | RvError::SessionClosed => WorkspaceSignerError::Locked(format!("PKCS#11 {rv:?}")),
            RvError::TokenNotPresent
            | RvError::TokenNotRecognized
            | RvError::SlotIdInvalid
            | RvError::DeviceError
            | RvError::DeviceRemoved
            | RvError::DeviceMemory => {
                WorkspaceSignerError::Unavailable(format!("PKCS#11 {rv:?}"))
            }
            other => WorkspaceSignerError::SigningFailed(format!("PKCS#11 {other:?}")),
        },
        Pkcs11Error::LibraryLoading(e) => {
            WorkspaceSignerError::Unavailable(format!("PKCS#11 library load: {e}"))
        }
        other => WorkspaceSignerError::SigningFailed(format!("PKCS#11: {other}")),
    }
}

/// Mutex-poison categorisation. Mirrors the wave-2
/// [`crate::hsm::pkcs11::map_session_lock_poison`] policy: a poisoned
/// mutex is permanent for the affected process (`std::sync::Mutex`
/// does not un-poison) so the only fix is restart. The variant choice
/// here is `SigningFailed` rather than `DeriveFailed` — the runbook
/// routes both to the same "investigate / restart" step, but the
/// wave-3 sign call is the dominant lock holder, so the variant that
/// matches the call site's natural failure category gives the
/// operator the most context for the trace.
fn map_session_lock_poison() -> WorkspaceSignerError {
    WorkspaceSignerError::SigningFailed(
        "PKCS#11 workspace-signer session mutex poisoned — a previous signing or \
         find-or-generate op panicked; process restart required (poison is permanent \
         for this process)"
            .to_string(),
    )
}

/// Compile-time guard: [`Pkcs11WorkspaceSigner`] must be `Send + Sync`
/// so the Phase-C dispatcher can wrap it in `Arc<dyn WorkspaceSigner>`
/// and share it across async tasks. The trait itself requires
/// `Send + Sync`; this assertion catches a regression (e.g. a `Cell`
/// field) at compile time rather than at the dispatcher's call site.
const _: () = {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    let _ = assert_send::<Pkcs11WorkspaceSigner>;
    let _ = assert_sync::<Pkcs11WorkspaceSigner>;
};

#[cfg(test)]
mod tests {
    //! Phase-B unit tests cover the parts of the wave-3 signer that
    //! do NOT need a live PKCS#11 module:
    //!
    //!   * [`unwrap_octet_string`] — both shapes (raw 32, wrapped 34)
    //!     plus the malformed-input error path. Catches a future
    //!     refactor that drops the raw-form fallback and silently
    //!     starts refusing the SoftHSM2 ≤ 2.5 / commercial-HSM
    //!     encoding.
    //!   * [`map_pkcs11_sign_error`] — the variant cleaving policy.
    //!     The Locked / Unavailable / SigningFailed split is a
    //!     one-line operator-remediation contract that needs to stay
    //!     pinned across refactors (the wave-2 `map_pkcs11_error`
    //!     uses the same RV groupings but routes to `DeriveFailed`
    //!     for the catch-all, which is the WRONG remediation for a
    //!     sign path — this test catches a copy-paste regression).
    //!   * [`map_session_lock_poison`] — wording pin so a refactor
    //!     cannot drop the "restart required" hint.
    //!
    //! Live HSM coverage (find-or-generate, generate_keypair,
    //! cross-session persistence, sign+verify round-trip) lives in
    //! `tests/hsm_workspace_signer.rs` behind the dual-gate harness.
    use super::*;

    #[test]
    fn unwrap_octet_string_accepts_raw_32_byte_form() {
        // SoftHSM2 ≤ 2.5 and several commercial HSMs return the raw
        // 32 bytes without the OCTET STRING wrapper. The unwrap helper
        // must accept both forms; refusing the raw form would make the
        // wave-3 signer break against a substantial fraction of
        // deployed HSMs.
        let raw: [u8; 32] = [0xab; 32];
        let unwrapped = unwrap_octet_string(&raw, "atlas-workspace-key-v1:test", 0)
            .expect("32-byte raw form must be accepted");
        assert_eq!(unwrapped, raw);
    }

    #[test]
    fn unwrap_octet_string_accepts_der_wrapped_form() {
        // PKCS#11 v3.0 §10.10 mandates the DER OCTET STRING wrapping:
        // tag 0x04, length 0x20 (= 32), then the raw 32 bytes.
        // SoftHSM2 ≥ 2.6 and most commercial HSMs ship this form.
        let mut wrapped = vec![0x04u8, 0x20];
        wrapped.extend_from_slice(&[0xcd; 32]);
        let unwrapped = unwrap_octet_string(&wrapped, "atlas-workspace-key-v1:test", 0)
            .expect("34-byte DER-wrapped form must be accepted");
        assert_eq!(unwrapped, [0xcd; 32]);
    }

    #[test]
    fn unwrap_octet_string_rejects_malformed_input() {
        // Any encoding other than 32-raw or 34-wrapped is an HSM /
        // cryptoki bug and must surface as DeriveFailed with a
        // diagnostic the operator can file a vendor ticket against.
        // Cover four common malformations:
        //   * empty (HSM returned nothing useful)
        //   * length 33 (off-by-one tag or length byte)
        //   * length 34 but wrong tag (0x03 BIT STRING instead of 0x04)
        //   * length 34, right tag, wrong length byte (0x10 instead of 0x20)
        let cases: &[&[u8]] = &[
            &[],
            &[0u8; 33],
            &{
                let mut v = vec![0x03u8, 0x20]; // BIT STRING tag, not OCTET STRING
                v.extend_from_slice(&[0u8; 32]);
                v
            },
            &{
                let mut v = vec![0x04u8, 0x10]; // OCTET STRING tag, length 16 not 32
                v.extend_from_slice(&[0u8; 32]);
                v
            },
        ];
        for bad in cases {
            match unwrap_octet_string(bad, "atlas-workspace-key-v1:test", 7) {
                Err(WorkspaceSignerError::DeriveFailed(msg)) => {
                    assert!(
                        msg.contains("CKA_EC_POINT") && msg.contains("slot 7"),
                        "unwrap_octet_string error must surface CKA_EC_POINT name + slot id; got: {msg}",
                    );
                }
                Ok(out) => panic!(
                    "malformed CKA_EC_POINT bytes {bad:?} must NOT silently parse \
                     to {out:?} — that would mask an HSM encoding bug",
                ),
                Err(other) => panic!(
                    "malformed CKA_EC_POINT must yield DeriveFailed (operator action: \
                     file vendor ticket); got {other:?}",
                ),
            }
        }
    }

    #[test]
    fn map_pkcs11_sign_error_catch_all_routes_to_signing_failed() {
        // Pin the catch-all arm: any non-LibraryLoading, non-Pkcs11(rv,
        // fn) variant goes to SigningFailed. The wave-2
        // `map_pkcs11_error` would route the same input to DeriveFailed
        // — a copy-paste regression here would silently send operators
        // down the wrong remediation path (provision a key vs. retry
        // the sign).
        //
        // **Why only catch-all coverage at unit-test time.**
        // `Pkcs11Error::Pkcs11(rv, Function)` requires
        // `cryptoki::context::Function`, which the cryptoki crate does
        // NOT re-export (the same constraint that limits wave-2's
        // `keys::tests::variant_constructors_compile` to a smoke
        // assertion). The Locked / Unavailable / SigningFailed cleaving
        // for per-RV inputs is exercised end-to-end by
        // `tests/hsm_workspace_signer.rs` against a live SoftHSM2
        // token. Two witnesses (`NotSupported`, `InvalidValue`) keep
        // the catch-all arm honest against a refactor that special-
        // cases one variant and forgets the other.
        for witness in [Pkcs11Error::NotSupported, Pkcs11Error::InvalidValue] {
            match map_pkcs11_sign_error(witness) {
                WorkspaceSignerError::SigningFailed(msg) => {
                    assert!(
                        msg.contains("PKCS#11"),
                        "SigningFailed message must surface 'PKCS#11' so the operator can \
                         grep the trace; got: {msg}",
                    );
                }
                other => panic!(
                    "Catch-all PKCS#11 errors at sign-call site must map to SigningFailed \
                     (retry-meaningful), NOT {other:?} — wave-2's map_pkcs11_error returns \
                     DeriveFailed for the catch-all, and a copy-paste here would route the \
                     operator to the wrong remediation",
                ),
            }
        }
    }

    #[test]
    fn map_pkcs11_sign_error_variant_identifiers_compile() {
        // Per-RV cleaving rules (Locked / Unavailable / SigningFailed)
        // are exercised end-to-end by `tests/hsm_workspace_signer.rs`
        // since `Pkcs11Error::Pkcs11(rv, Function)` cannot be constructed
        // from outside cryptoki. Pinning the RvError variant identifiers
        // here ensures a future cryptoki bump that renames a variant
        // (e.g. PinIncorrect → PinInvalidPin) trips at unit-test time
        // rather than in the integration lane.
        let _ = RvError::UserNotLoggedIn;
        let _ = RvError::PinIncorrect;
        let _ = RvError::PinLocked;
        let _ = RvError::PinExpired;
        let _ = RvError::PinInvalid;
        let _ = RvError::SessionHandleInvalid;
        let _ = RvError::SessionClosed;
        let _ = RvError::TokenNotPresent;
        let _ = RvError::TokenNotRecognized;
        let _ = RvError::SlotIdInvalid;
        let _ = RvError::DeviceError;
        let _ = RvError::DeviceRemoved;
        let _ = RvError::DeviceMemory;
    }

    #[test]
    fn map_session_lock_poison_returns_signing_failed_with_restart_hint() {
        // The poison handler must surface the only effective
        // remediation ("restart"), with enough wording for an operator
        // to grep their logs for it. Mirrors the wave-2 poison-test
        // contract — different variant (SigningFailed vs DeriveFailed)
        // but same operator-facing wording requirement.
        match map_session_lock_poison() {
            WorkspaceSignerError::SigningFailed(msg) => {
                assert!(
                    msg.contains("poison"),
                    "poison message must contain 'poison' so the operator can grep \
                     their logs; got: {msg}",
                );
                assert!(
                    msg.contains("restart"),
                    "poison message must contain 'restart' (the only effective \
                     remediation); got: {msg}",
                );
            }
            other => panic!(
                "session-lock poison must map to SigningFailed (retry-class remediation, \
                 since the dominant lock holder is the sign path); got {other:?}",
            ),
        }
    }

    #[test]
    fn workspace_label_prefix_is_versioned_and_colon_terminated() {
        // The on-token namespace contract: prefix MUST end in ':' so
        // the workspace_id parses unambiguously, and MUST contain a
        // -vN version segment so a future grammar change has a
        // rotation hatch. Pin both invariants so a refactor that
        // "tidies" the prefix cannot silently break the label
        // grammar that the runbook documents.
        assert!(
            WORKSPACE_LABEL_PREFIX.ends_with(':'),
            "workspace label prefix must end with ':' — workspace_id validation \
             forbids ':' in the payload, making the colon an unambiguous delimiter",
        );
        assert!(
            WORKSPACE_LABEL_PREFIX.contains("-v1:"),
            "workspace label prefix must contain a '-v1:' (or successor) version \
             segment so the on-token grammar has a rotation hatch",
        );
        assert_eq!(
            WORKSPACE_LABEL_PREFIX, "atlas-workspace-key-v1:",
            "label prefix is part of the runbook + the on-token state; pinning the \
             exact value here so a typo cannot silently rotate the namespace",
        );
    }
}
