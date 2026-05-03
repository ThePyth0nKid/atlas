//! V1.9 — Per-tenant Ed25519 workspace-signing-key derivation.
//!
//! V1.5–V1.8 signed every event with one of three globally-shared
//! Ed25519 keypairs (agent / human / anchor). A compromise of any of
//! those three keys forged events for *every* workspace at once.
//!
//! V1.9 derives a per-workspace Ed25519 signing key from a single master
//! seed using HKDF-SHA256 (RFC 5869) with a domain-separated `info`
//! parameter. The verifier consumes the resulting public key via the
//! `PubkeyBundle` and never sees the master seed — re-derivation is an
//! issuer-side capability only.
//!
//! ## Derivation
//!
//! ```text
//! info       = "atlas-anchor-v1:" || workspace_id
//! key_bytes  = HKDF-SHA256(salt = None, ikm = master_seed, info, len = 32)
//! signing    = ed25519_dalek::SigningKey::from_bytes(&key_bytes)
//! ```
//!
//! HKDF (extract-then-expand) gives uniformly-random 32-byte output
//! indistinguishable from random under the standard HKDF assumption,
//! and the `info` parameter is the domain-separation knob — different
//! `info` strings produce independent keys even from the same `ikm`.
//! Ed25519 accepts any 32-byte sequence as a secret-scalar seed
//! (libsodium-style), so the HKDF output goes straight into
//! `SigningKey::from_bytes` without further reduction.
//!
//! ## Why the `atlas-anchor-v1:` info prefix
//!
//! The prefix is the trust boundary for per-tenant key separation. If
//! we used just `workspace_id` directly, an attacker who controlled the
//! `workspace_id` of one Atlas service could re-derive the same key
//! used in a different domain (e.g. a hypothetical `atlas-policy:`
//! derivation that came along later). The prefix keeps namespaces
//! disjoint by construction. The `-v1` is a future-rotation tag: if we
//! ever need to change the algorithm, bumping it to `-v2` produces a
//! disjoint key set without re-using the same `(ikm, info)` pair.
//!
//! Note the *issuer-side* HKDF info-prefix (`atlas-anchor-v1:`) is
//! intentionally distinct from the *verifier-side* kid prefix
//! (`atlas-anchor:`, see `atlas_trust_core::PER_TENANT_KID_PREFIX`).
//! They serve different purposes and sit on different sides of the
//! trust boundary; if you ever feel tempted to make them the same
//! string, remember that doing so couples the wire-format identifier
//! to the cryptographic-domain tag and constrains future format
//! evolution.
//!
//! ## Master seed handling
//!
//! `DEV_MASTER_SEED` is a constant in source. This is the SAME residual
//! single-point-of-failure as V1.8: master-seed compromise compromises
//! every workspace. V1.10 closes it with HSM/TPM sealing — until then,
//! see `docs/SECURITY-NOTES.md`. Production deployments MUST replace
//! the constant with a sealed-key handle before going live.

use atlas_trust_core::per_tenant_kid_for;
use ed25519_dalek::SigningKey;
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

/// V1.9 dev master seed. Production MUST replace this with a sealed
/// secret (HSM/TPM/cloud-KMS) before going live; see
/// `docs/SECURITY-NOTES.md`. The constant is fixed across builds so
/// development environments produce reproducible per-workspace pubkeys
/// — the smoke test pins the resulting bundle hash.
///
/// Layout note: the value is exactly 32 ASCII bytes; byte 31 is `0x0A`
/// (LF) so the printable prefix `atlas-master-seed-v1-dev-001-00` stays
/// 31 chars and pads up to 32 with a single LF. This is intentional —
/// editing the constant to "look nicer" by removing the LF reduces the
/// length to 31 and breaks compilation. The `workspace_pubkeys_are_pinned`
/// test below catches any byte-level drift in the seed at CI time.
pub const DEV_MASTER_SEED: [u8; 32] = *b"atlas-master-seed-v1-dev-001-00\n";

/// Environment variable that opts an `atlas-signer` invocation OUT of
/// the dev master seed. Set to `1` in any environment where running
/// with the source-committed `DEV_MASTER_SEED` would be a security
/// failure (production, staging touching real customer data, audit
/// rehearsals against the real key roster).
///
/// V1.9 had no sealed-seed loader, so `production_gate` returned an
/// error to refuse every per-tenant subcommand instead of silently
/// using the public dev key. V1.10 wave 2 ships the sealed-seed loader
/// at [`crate::hsm`]; the canonical V1.10 entry point is
/// [`master_seed_loader`], which dispatches to the PKCS#11 backend
/// when the HSM env trio is set and otherwise falls through to the
/// V1.10 wave-1 [`master_seed_gate`] (positive opt-in for the dev
/// seed). `production_gate` survives as the V1.9 paranoia layer
/// nested inside [`master_seed_gate_with`].
pub const PRODUCTION_GATE_ENV: &str = "ATLAS_PRODUCTION";

/// Refuse to use `DEV_MASTER_SEED` if the environment marks this
/// invocation as production. Returns an error message suitable for
/// stderr.
///
/// The gate fires only on the byte-exact value `"1"`. Any other value
/// (unset, empty, `"0"`, `"true"`, `"yes"`, `"on"`, `"1 "` with
/// trailing whitespace, …) allows the dev seed — V1.9 dev/CI
/// environments run with the env var unset; production rollouts set
/// `=1` and configure the V1.10 wave-2 sealed-seed loader
/// (`ATLAS_HSM_PKCS11_LIB` / `ATLAS_HSM_SLOT` / `ATLAS_HSM_PIN_FILE`,
/// see [`crate::hsm`]) to re-enable per-tenant commands.
///
/// **Operator footgun (documented in `OPERATOR-RUNBOOK.md` §1):** the
/// strict-`"1"` recognition is a deployment trap waiting for someone
/// who reflexively writes `ATLAS_PRODUCTION=true` (a common K8s/Docker
/// idiom). That value silently falls through and the dev seed signs.
/// V1.10 will replace this gate with positive opt-in semantics; until
/// then, deploy automation must verify the env literally equals `1`.
///
/// Implementation: forwards to `production_gate_with(|name|
/// std::env::var(name).ok())`. Tests use the injection form to avoid
/// mutating the process environment from inside a parallel cargo
/// runner (cargo defaults to a thread pool, so `std::env::set_var`
/// between tests is a data race). The injection seam also foreshadows
/// V1.10: the sealed-seed loader will inject a seed source the same
/// way this gate injects an env source.
///
/// **V1.10 status:** `production_gate` is no longer the binary's
/// hot-path entry point. [`master_seed_gate`] subsumes it as the
/// canonical V1.10 gate (calling `production_gate_with` internally
/// for the V1.9 paranoia layer). The no-arg form survives for
/// V1.9 backwards compat and as a stable point that external
/// embedders (and the in-tree V1.10 `crate::hsm` loader's preflight)
/// can depend on. Now part of the lib's public surface, so a
/// missing-call lint no longer fires here — the `#[expect(dead_code)]`
/// shim from V1.9 was dropped during the V1.10 lib refactor.
pub fn production_gate() -> Result<(), String> {
    production_gate_with(|name| std::env::var(name).ok())
}

/// Test-injection form of `production_gate`. Takes an env reader (a
/// pure closure mapping a variable name to its optional value) so test
/// code can drive the gate without touching the global process env.
///
/// Public-but-`pub(crate)`-spirit: keep this on the binary's surface
/// for future in-crate callers (the V1.10 HSM loader will reuse the
/// same injection style for its seed-source lookup) while signalling
/// that the gate is the canonical entry point — external embedders
/// should call `production_gate`, not this one.
pub fn production_gate_with<F>(env: F) -> Result<(), String>
where
    F: Fn(&str) -> Option<String>,
{
    match env(PRODUCTION_GATE_ENV).as_deref() {
        Some("1") => Err(format!(
            "{PRODUCTION_GATE_ENV}=1 set, but atlas-signer is using the source-committed \
             DEV_MASTER_SEED. Refusing to derive per-tenant keys against a public dev seed. \
             V1.10 wave 2 ships a sealed-seed loader at crate::hsm — configure the env trio \
             (ATLAS_HSM_PKCS11_LIB, ATLAS_HSM_SLOT, ATLAS_HSM_PIN_FILE) and rebuild with \
             --features hsm. See docs/OPERATOR-RUNBOOK.md §1 for the import ceremony."
        )),
        _ => Ok(()),
    }
}

/// V1.10 positive-opt-in environment variable. The dev master seed
/// is now opt-in: an operator must set this variable to a recognised
/// truthy value before any per-tenant subcommand will sign.
///
/// Recognised truthy values (case-insensitive, with leading/trailing
/// ASCII whitespace tolerated):
///   * `"1"`, `"true"`, `"yes"`, `"on"`
///
/// Anything else — including the env var being unset, empty, or set
/// to `"0"`, `"false"`, `"no"`, `"off"`, or any other string — fails
/// the gate. This is the inverse of [`PRODUCTION_GATE_ENV`]: V1.9
/// asked operators to opt OUT of the dev seed (`ATLAS_PRODUCTION=1`
/// blocks); V1.10 asks them to opt IN (`ATLAS_DEV_MASTER_SEED=1`
/// allows). The default is now production-safe.
pub const DEV_MASTER_SEED_OPT_IN_ENV: &str = "ATLAS_DEV_MASTER_SEED";

/// V1.10 master-seed gate. Returns the [`MasterSeedHkdf`] impl this
/// invocation should use, or an error message suitable for stderr.
///
/// **Layered checks (defence-in-depth):**
///   1. **V1.9 paranoia:** if `ATLAS_PRODUCTION=1`, refuse the dev
///      seed regardless of any V1.10 opt-in. An operator who
///      explicitly says "this is production" overrides everything;
///      this preserves V1.9 deployment safety habits across the
///      V1.9→V1.10 transition.
///   2. **V1.10 positive opt-in:** require `ATLAS_DEV_MASTER_SEED`
///      to be a recognised truthy value (`1`, `true`, `yes`, `on`,
///      case-insensitive). Anything else refuses.
///   3. **Sealed-seed lookup (V1.10 wave 2, shipped):** lives in
///      [`master_seed_loader`]. When the HSM env trio
///      (`ATLAS_HSM_PKCS11_LIB`, `ATLAS_HSM_SLOT`, `ATLAS_HSM_PIN_FILE`)
///      is set, the loader returns
///      [`crate::hsm::pkcs11::Pkcs11MasterSeedHkdf`] instead of
///      `DevMasterSeedHkdf` and the gate's layers 1–2 are not
///      consulted (no dev-seed code path). HSM init failure is fatal
///      — there is no silent fallback to the dev seed.
///
/// **Why not auto-detect a sealed seed source?** Explicit opt-in
/// surfaces the choice in the operator's deploy manifest. An
/// implicit "if HSM is present, use HSM; else dev seed" would be a
/// silent fallback the next deployment review would have to verify.
/// The trait dispatch is selected by env, not by feature detection.
///
/// **Why a strict allow-list of truthy values?** V1.9 had a
/// documented operator footgun: `ATLAS_PRODUCTION=true` silently
/// allowed the dev seed (only literal `"1"` blocked). V1.10 inverts
/// the polarity AND the allow-list logic so the safe direction is
/// the default direction: a typo on opt-in still refuses, instead
/// of silently allowing.
///
/// Returns `Ok(DevMasterSeedHkdf)` on success — this gate is the
/// dev-seed branch only and intentionally returns the concrete dev
/// impl. The wider runtime dispatch (HSM trio → PKCS#11 backend, env
/// trio absent → dev gate) lives in [`master_seed_loader`], which
/// returns `Box<dyn MasterSeedHkdf>` and reaches `master_seed_gate`
/// only on the dev branch. The split keeps the dev path
/// monomorphised (cheaper in tests, clearer in error messages) while
/// the binary's hot path goes through the trait-object loader.
pub fn master_seed_gate() -> Result<DevMasterSeedHkdf, String> {
    master_seed_gate_with(|name| std::env::var(name).ok())
}

/// Test-injection form of [`master_seed_gate`]. Takes an env reader
/// (a pure closure mapping a variable name to its optional value)
/// so test code can drive the gate without touching the global
/// process env.
///
/// Same injection style as [`production_gate_with`]; reuses the
/// same closure shape so V1.10 callers can supply a single env
/// source for both the V1.9 paranoia check and the V1.10 opt-in.
pub fn master_seed_gate_with<F>(env: F) -> Result<DevMasterSeedHkdf, String>
where
    F: Fn(&str) -> Option<String>,
{
    // Layer 1: V1.9 paranoia. If the operator says this is
    // production, refuse the dev seed regardless of opt-in.
    production_gate_with(&env)?;

    // Layer 2: V1.10 positive opt-in.
    let raw = env(DEV_MASTER_SEED_OPT_IN_ENV).unwrap_or_default();
    let normalised = raw.trim().to_ascii_lowercase();
    match normalised.as_str() {
        "1" | "true" | "yes" | "on" => Ok(DevMasterSeedHkdf),
        _ => Err(format!(
            "{DEV_MASTER_SEED_OPT_IN_ENV} not set to a recognised truthy value \
             (got {raw:?}). V1.10 inverts the V1.9 gate: the source-committed \
             DEV_MASTER_SEED now requires positive opt-in. Set \
             {DEV_MASTER_SEED_OPT_IN_ENV}=1 (or true/yes/on, case-insensitive) \
             in dev/CI environments. Production should use the V1.10 wave-2 \
             sealed-seed loader: configure the env trio \
             (ATLAS_HSM_PKCS11_LIB, ATLAS_HSM_SLOT, ATLAS_HSM_PIN_FILE) and \
             rebuild with --features hsm. See docs/OPERATOR-RUNBOOK.md §1 \
             for the V1.9→V1.10 migration and the HSM import ceremony."
        )),
    }
}

/// V1.10 wave 2 master-seed loader. Returns the [`MasterSeedHkdf`]
/// impl this invocation should use — either the PKCS#11 sealed-seed
/// loader (when the HSM env trio is set) or the dev seed (after the
/// V1.10 wave-1 gate clears). The return is `Box<dyn MasterSeedHkdf>`
/// so the binary's per-tenant subcommands route uniformly through the
/// existing `_via` helpers regardless of which backend is active.
///
/// **Dispatch order:**
///   1. **HSM trio first.** If any of `ATLAS_HSM_PKCS11_LIB`,
///      `ATLAS_HSM_SLOT`, or `ATLAS_HSM_PIN_FILE` is set, the operator
///      intends sealed-seed mode. Partial trios are a hard error
///      (`HsmConfig::from_env` refuses, surfacing the operator's
///      intent in the error message); full trios attempt
///      [`Pkcs11MasterSeedHkdf::open`](crate::hsm::pkcs11::Pkcs11MasterSeedHkdf::open).
///   2. **Dev seed second.** With no HSM trio set, fall through to
///      the V1.10 wave-1 [`master_seed_gate`] — which itself refuses
///      unless the operator has positively opted into the dev seed
///      via `ATLAS_DEV_MASTER_SEED=1`.
///
/// **Why HSM-first?** An operator who has set up a sealed-seed deploy
/// expects the loader to use it. Falling through to the dev seed when
/// HSM init fails would be the silent-fallback class V1.10 is closing.
/// Sealed-seed init failure is fatal here — the operator must fix the
/// HSM config or unset the trio, not silently sign with a dev key.
pub fn master_seed_loader() -> Result<Box<dyn MasterSeedHkdf>, String> {
    master_seed_loader_with(|name| std::env::var(name).ok())
}

/// Test-injection form of [`master_seed_loader`]. Takes an env reader
/// closure — same shape as [`master_seed_gate_with`] and
/// [`crate::hsm::config::HsmConfig::from_env`] — so a single env source
/// drives both the HSM trio parse and the dev-seed gate.
///
/// Forwards to [`master_seed_loader_with_writer`] with `std::io::stderr()`
/// as the deprecation-warning sink. Tests that need to assert on the
/// warning content (or want to silence it during noise-sensitive runs)
/// should call `master_seed_loader_with_writer` directly with a
/// `Vec<u8>` or [`std::io::sink`].
pub fn master_seed_loader_with<F>(env: F) -> Result<Box<dyn MasterSeedHkdf>, String>
where
    F: Fn(&str) -> Option<String>,
{
    master_seed_loader_with_writer(env, &mut std::io::stderr())
}

/// V1.11 L-8 entry point. Same dispatch logic as
/// [`master_seed_loader_with`], but takes a `&mut dyn Write` for the
/// deprecation warning emitted when [`PRODUCTION_GATE_ENV`] is set.
///
/// **Why a writer parameter?** Test code can capture the warning text
/// in a `Vec<u8>` and assert on it, or pass [`std::io::sink`] to drop
/// the warning entirely (useful when a test is exercising a deliberately
/// deprecated configuration and the noise on stderr would clutter the
/// test runner output). Production callers route through
/// [`master_seed_loader_with`] which forwards to `std::io::stderr()`.
///
/// **Why the warning?** The HSM trio (V1.10 wave 2) is now the
/// production audit signal. `ATLAS_PRODUCTION=1` carried the V1.9
/// paranoia gate forward as belt-and-braces, but it has the
/// literal-`"1"`-only recognition footgun (an operator who reflexively
/// writes `ATLAS_PRODUCTION=true` gets silent fallthrough). V1.11
/// emits a deprecation warning whenever the env var is observed with
/// non-whitespace content; V1.12 will remove the gate entirely. The
/// warning fires *before* the layered gates so it surfaces even when
/// the gate ultimately refuses the configuration.
pub fn master_seed_loader_with_writer<F, W>(
    env: F,
    warn_out: &mut W,
) -> Result<Box<dyn MasterSeedHkdf>, String>
where
    F: Fn(&str) -> Option<String>,
    W: std::io::Write,
{
    // V1.11 L-8: ATLAS_PRODUCTION deprecation warning. Fired before
    // any gate check so the operator sees the migration notice
    // regardless of which path the loader ultimately takes (refuse via
    // production_gate, refuse via opt-in gate, succeed via HSM trio,
    // succeed via dev opt-in). The trim-empty filter ignores
    // misconfigured pipelines that emit `ATLAS_PRODUCTION=` with no
    // value; only meaningful settings trip the warning. Writes via
    // `writeln!` and ignores errors — a failed write to stderr should
    // not block the loader.
    if env(PRODUCTION_GATE_ENV)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        let _ = writeln!(
            warn_out,
            "warning: {PRODUCTION_GATE_ENV} is deprecated and scheduled for \
             removal in V1.12. V1.10 inverted the master-seed gate to positive \
             opt-in: the dev seed now requires {DEV_MASTER_SEED_OPT_IN_ENV}=1 \
             (truthy values: 1/true/yes/on, case-insensitive), which makes the \
             V1.9 paranoia check redundant. For production, configure the HSM \
             trio ({}, {}, {}) — that is the V1.10+ production audit signal. \
             See docs/OPERATOR-RUNBOOK.md §1.",
            crate::hsm::config::PKCS11_LIB_ENV,
            crate::hsm::config::SLOT_ENV,
            crate::hsm::config::PIN_FILE_ENV,
        );
    }

    // Layer 1: HSM trio. If any of the three env vars is set, we
    // commit to sealed-seed mode; partial trios refuse via
    // `HsmConfig::from_env`.
    if let Some(cfg) = crate::hsm::config::HsmConfig::from_env(&env)? {
        let pkcs11 = crate::hsm::pkcs11::Pkcs11MasterSeedHkdf::open(cfg)
            .map_err(|e| format!("HSM open failed: {e}"))?;
        return Ok(Box::new(pkcs11));
    }

    // Layer 2: dev seed via the V1.10 wave-1 gate.
    let dev = master_seed_gate_with(&env)?;
    Ok(Box::new(dev))
}

/// Maximum byte length of a `workspace_id`. Bounding the input here
/// gives every downstream consumer a predictable size budget:
///
///   * the per-tenant kid (`atlas-anchor:` + workspace_id) is at most
///     `13 + 256 = 269` bytes, well under any realistic PKCS#11
///     `info`-parameter ceiling and PubkeyBundle key-name limit;
///   * a malicious or buggy caller cannot insert a 10 MB key into the
///     `PubkeyBundle` keys map (cheap-DoS surface flagged in V1.9
///     review); and
///   * V1.10's HSM derive call gets a definite upper bound on the
///     `info` material it has to forward to the device.
///
/// 256 bytes is intentionally generous (UUIDs, structured tenant
/// names like `bank-hagedorn:prod:eu-west`, even hash-prefixed names
/// fit comfortably) but small enough to fail loudly on accidentally-
/// large input. If a real-world deployment needs longer IDs, raise the
/// constant deliberately and bump documentation; do not hot-patch
/// callers around it.
pub const WORKSPACE_ID_MAX_BYTES: usize = 256;

/// Validate a `workspace_id` for use in HKDF derivation and per-tenant
/// kid construction.
///
/// `atlas-trust-core::parse_per_tenant_kid` is intentionally lenient —
/// the trust property holds via byte-exact kid comparison and HKDF
/// determinism for any non-empty UTF-8 string. The issuer side is the
/// place to enforce ingress hygiene, because that is where ambiguous or
/// confusable IDs become operator footguns and observability holes.
///
/// We accept ASCII printable bytes (0x21..=0x7E) up to
/// `WORKSPACE_ID_MAX_BYTES` and reject:
///   * empty strings (no legitimate per-tenant kid names the empty workspace);
///   * strings longer than `WORKSPACE_ID_MAX_BYTES` (cheap DoS surface);
///   * any byte outside the ASCII-printable range (control chars, NUL,
///     DEL, non-ASCII — defence against Unicode confusables); and
///   * the byte `:` (the kid prefix delimiter — workspace_ids
///     containing `:` produce kids with ambiguous segmentation).
///
/// Returns `Ok(())` on accept, `Err(message)` on reject.
pub fn validate_workspace_id(workspace_id: &str) -> Result<(), String> {
    if workspace_id.is_empty() {
        return Err("workspace_id must be non-empty".to_string());
    }
    if workspace_id.len() > WORKSPACE_ID_MAX_BYTES {
        return Err(format!(
            "workspace_id is {} bytes; the cap is {WORKSPACE_ID_MAX_BYTES}. Real-world \
             tenant identifiers are short — a longer value usually indicates an upstream \
             bug or an attempt to insert an oversized PubkeyBundle entry.",
            workspace_id.len(),
        ));
    }
    for (i, b) in workspace_id.bytes().enumerate() {
        if !(0x21..=0x7E).contains(&b) {
            return Err(format!(
                "workspace_id byte {i} is 0x{b:02x}; only ASCII printable bytes \
                 0x21..=0x7E are allowed (no whitespace, control chars, or non-ASCII)",
            ));
        }
        if b == b':' {
            return Err(format!(
                "workspace_id contains ':' at byte {i}; ambiguous with the kid prefix \
                 delimiter ('atlas-anchor:'). Use '-' or '_' instead.",
            ));
        }
    }
    Ok(())
}

/// Domain-separation prefix prepended to the workspace_id when forming
/// the HKDF `info` parameter. See module doc for why this is a
/// versioned tag.
///
/// `pub` because V1.10's HSM-backed loader (in a sibling crate) needs
/// to assemble the same `info` string before submitting an HKDF derive
/// call to the device. Re-implementing the prefix in the HSM crate
/// would split the cryptographic-domain tag across two source-of-truth
/// sites — a drift surface the V1.9 design intentionally avoided.
///
/// **Future-prefix invariant.** Any new domain prefix added later
/// (e.g. `atlas-policy-v1:`, `atlas-witness-v1:`) MUST NOT produce an
/// `info` string that is a prefix of any pre-existing one for any
/// workspace_id. Two info strings where one is a prefix of the other
/// produce different HKDF outputs (HKDF-Expand reads the info bytes
/// verbatim with no separator), but the safety margin is conceptual:
/// ensure the namespaces are visibly disjoint at the literal-prefix
/// level so a careful reader can rule out collisions by inspection.
/// Bumping the `-vN` suffix when changing the algorithm is the
/// standard rotation hatch.
pub const HKDF_INFO_PREFIX: &str = "atlas-anchor-v1:";

/// Derive a per-workspace Ed25519 signing key from `master_seed` and
/// `workspace_id` via HKDF-SHA256.
///
/// Determinism: the function is a pure deterministic mapping
/// `(master_seed, workspace_id) → signing_key`. Two calls with the
/// same inputs produce byte-identical keys; two calls with different
/// `workspace_id` (with the same master seed) produce independent
/// keys, which is the property that gives V1.9 per-tenant isolation.
///
/// Failure mode: HKDF-SHA256 expand returns an error only when the
/// requested output length exceeds `255 * 32 = 8160` bytes; we ask for
/// 32, so the call cannot fail. The `expect` documents that
/// invariant rather than silently swallowing an error path.
///
/// **V1.10 routing:** the production path now goes through
/// [`MasterSeedHkdf`] via [`derive_workspace_signing_key_via`]. This
/// explicit-seed leaf is retained for (a) the rotation test
/// (`different_master_seeds_yield_different_keys`), (b) the pinned-
/// pubkey golden tests, (c) the V1.10 wave-2 import-ceremony tests
/// in `crate::hsm` (verify the HSM-derived key matches the
/// software-derived key for a known seed), and (d) future audit
/// tooling that wants to drive arbitrary seeds without instantiating
/// a trait impl. Public surface in the V1.10 lib refactor.
pub fn derive_workspace_signing_key(
    master_seed: &[u8; 32],
    workspace_id: &str,
) -> SigningKey {
    let hk = Hkdf::<Sha256>::new(None, master_seed);
    // Mirror the wrapping in `derive_workspace_signing_key_via`: the
    // 32-byte HKDF output is per-tenant secret material; without
    // `Zeroizing` it lingers on the stack frame after this function
    // returns, recoverable from a core dump that captures stack pages.
    // `SigningKey::from_bytes` clones into the dalek-internal scalar
    // (which has its own zeroize-on-drop), so the wrapper here only
    // needs to cover the local intermediate.
    let mut key_bytes: Zeroizing<[u8; 32]> = Zeroizing::new([0u8; 32]);
    let info = format!("{HKDF_INFO_PREFIX}{workspace_id}");
    hk.expand(info.as_bytes(), &mut *key_bytes)
        .expect("HKDF-SHA256 expand of 32 bytes is well within the 8160-byte ceiling");
    SigningKey::from_bytes(&key_bytes)
}

/// Abstraction over the V1.9 HKDF-SHA256 master-seed derivation
/// surface. V1.9 backs this with [`DevMasterSeedHkdf`] (in-memory
/// [`DEV_MASTER_SEED`]); V1.10 wave 2 adds
/// [`crate::hsm::pkcs11::Pkcs11MasterSeedHkdf`] (gated behind the
/// `hsm` feature) that performs the HKDF inside an HSM/TPM token
/// without ever exposing the seed to host memory.
///
/// **Trust property.** The trait deliberately exposes only one
/// operation: "given an `info` parameter, return 32 bytes of HKDF
/// output." It does NOT expose the seed itself, an extract step, or
/// a "give me a copy of the master key" method. Implementations are
/// the only place the seed lives, and the trait surface is the only
/// way out — sealed-key impls (PKCS#11, TPM, cloud-KMS) honour that
/// boundary by performing the entire derivation inside the device.
///
/// **Send + Sync.** Required so the V1.10 MCP-server side can hold a
/// `Arc<dyn MasterSeedHkdf>` across async tasks. The trait is also
/// dyn-safe (no generics, no `Self` returns, no `async fn`) so call
/// sites can dispatch dev-vs-sealed at runtime via `Box<dyn ...>`.
///
/// **Why not return `[u8; 32]`?** The `&mut [u8; 32]` form lets a
/// caller place the buffer in stack frame storage it controls
/// (e.g. wrapped in `Zeroizing`) without the trait method having to
/// know about the wrapper type. The dev impl below still copies
/// through `SigningKey::from_bytes`, but the V1.10 sealed loaders
/// gain explicit lifetime control over the derived bytes.
pub trait MasterSeedHkdf: Send + Sync {
    /// Run HKDF-Expand over the implementation's master seed (with
    /// `salt = None`) using `info` as the domain-separation
    /// parameter, writing exactly 32 bytes into `out`.
    ///
    /// On error, the contents of `out` are unspecified — the caller
    /// MUST NOT use them. Implementations SHOULD zero `out` before
    /// returning an error if they wrote partial output, but the
    /// contract above lets simple impls skip the zeroize step.
    fn derive_for(
        &self,
        info: &[u8],
        out: &mut [u8; 32],
    ) -> Result<(), MasterSeedError>;
}

/// Error returned by [`MasterSeedHkdf`]. Categorises the three
/// failure modes the V1.10 wave-2 sealed-seed loader produces —
/// `Locked` (HSM PIN missing or session not authenticated),
/// `Unavailable` (token absent, driver missing, network HSM
/// unreachable), and `DeriveFailed` (the device returned an error
/// during the derive call).
///
/// The dev impl never returns an error in practice (in-memory
/// HKDF-Expand of 32 bytes cannot fail — the only failure mode of
/// `hkdf::Hkdf::expand` is requesting more than `255 * HashLen`
/// bytes), but propagation through the same error type keeps every
/// call site uniform.
///
/// `#[non_exhaustive]` so the V1.10 PKCS#11 impl can introduce more
/// granular variants (e.g. `PinExpired`, `TokenRemoved`) without a
/// SemVer break of downstream consumers.
///
/// V1.10 lib refactor: the per-variant `#[expect(dead_code)]` shim
/// from V1.9 was dropped. The enum is part of the public lib API
/// now; rustc no longer fires dead-code on `pub` enum variants in a
/// library, so the lint expectation would be unfulfilled. The
/// V1.10 wave-2 PKCS#11 backend (`crate::hsm::pkcs11`) constructs
/// `Locked` and `Unavailable` in its error mapping.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MasterSeedError {
    /// Sealed-key store reachable but locked. Operator-recoverable
    /// (login, supply PIN, re-authenticate session).
    #[error("master seed source locked: {0}")]
    Locked(String),
    /// Sealed-key store unreachable. Usually a deployment misconfig
    /// (driver path wrong, token slot empty, network HSM down).
    #[error("master seed source unavailable: {0}")]
    Unavailable(String),
    /// HKDF-Expand or device-derive call returned an error. The
    /// V1.10 PKCS#11 impl maps PKCS#11 RV codes here; the dev impl
    /// uses this for the (impossible) HKDF-Expand-too-long case.
    #[error("master seed HKDF derivation failed: {0}")]
    DeriveFailed(String),
}

/// Dev-only [`MasterSeedHkdf`] impl wrapping the source-committed
/// [`DEV_MASTER_SEED`].
///
/// This was the V1.9 production path. V1.10 wave 2 replaces it at the
/// call site with [`crate::hsm::pkcs11::Pkcs11MasterSeedHkdf`] when
/// the HSM env trio is set; [`master_seed_loader`] handles the
/// dispatch so the binary never sees the dev seed in HSM mode.
/// [`production_gate`] continues to refuse the dev seed when
/// `ATLAS_PRODUCTION=1`, layered inside [`master_seed_gate_with`] —
/// so even with the HSM trio absent, the dev impl can never reach
/// a production per-tenant signing path.
///
/// Zero-sized: cloning and copying are free; the impl is stateless
/// because the seed lives in the static [`DEV_MASTER_SEED`] constant.
#[derive(Debug, Clone, Copy, Default)]
pub struct DevMasterSeedHkdf;

impl MasterSeedHkdf for DevMasterSeedHkdf {
    fn derive_for(
        &self,
        info: &[u8],
        out: &mut [u8; 32],
    ) -> Result<(), MasterSeedError> {
        let hk = Hkdf::<Sha256>::new(None, &DEV_MASTER_SEED);
        hk.expand(info, out).map_err(|e| {
            // `hkdf::InvalidLength` only fires when the requested
            // output exceeds 8160 bytes, which a 32-byte fixed buffer
            // cannot do. Map to `DeriveFailed` for completeness so the
            // call site never has to handle a panic path.
            MasterSeedError::DeriveFailed(format!("HKDF-Expand: {e}"))
        })
    }
}

/// Trait-routed counterpart of [`derive_workspace_signing_key`].
/// Assembles the V1.9 HKDF info string (`"atlas-anchor-v1:" ||
/// workspace_id`) and delegates the 32-byte derive to the supplied
/// [`MasterSeedHkdf`].
///
/// V1.10's sealed-seed plumbing routes through this function so the
/// HSM impl plugs in via `derive_workspace_signing_key_via(&hsm,
/// workspace_id)` without the call sites needing to know whether the
/// seed is in-memory or sealed inside a token.
///
/// `H: ?Sized` so callers can pass either an owned impl
/// (`&DevMasterSeedHkdf`) or a trait object (`&dyn MasterSeedHkdf`,
/// e.g. when the seed source is selected at runtime). Generic-by-
/// default keeps the dev path monomorphised for the pinned-pubkey
/// golden tests; runtime dispatch is opt-in via the trait-object
/// coercion at the call site.
pub fn derive_workspace_signing_key_via<H: MasterSeedHkdf + ?Sized>(
    hkdf: &H,
    workspace_id: &str,
) -> Result<SigningKey, MasterSeedError> {
    let info = format!("{HKDF_INFO_PREFIX}{workspace_id}");
    // `Zeroizing<[u8; 32]>` scrubs the per-tenant derived seed on every
    // exit path (Ok, Err, panic). `SigningKey::from_bytes` clones the
    // bytes into the dalek-internal scalar; once we return, the only
    // copy of this material lives inside the `SigningKey`, which has its
    // own zeroize-on-drop semantics. Without the wrapper, the 32-byte
    // stack array would be left as freed memory after the function
    // returns — recoverable from a stack-trace in a core dump.
    let mut key_bytes: Zeroizing<[u8; 32]> = Zeroizing::new([0u8; 32]);
    hkdf.derive_for(info.as_bytes(), &mut key_bytes)?;
    Ok(SigningKey::from_bytes(&key_bytes))
}

/// Test-only convenience: derive the per-workspace signing key
/// using the crate-default [`DEV_MASTER_SEED`] via the
/// [`MasterSeedHkdf`] trait, with no gate check.
///
/// V1.9 had `derive_workspace_signing_key_default` on the binary's
/// hot path (gated by `production_gate`). V1.10 inverts the gate to
/// positive opt-in via [`master_seed_gate`]; the binary now selects
/// the trait impl through the gate and calls
/// [`derive_workspace_signing_key_via`] directly. This convenience
/// remains for the in-crate test harness (golden vectors, signature
/// round-trips, validate_workspace_id smoke).
///
/// `#[cfg(test)]` enforces the test-only intent at the type
/// system: the function does not exist in non-test builds, so
/// no caller can accidentally reach for it from binary code and
/// bypass the gate.
#[cfg(test)]
pub(crate) fn derive_workspace_signing_key_default(workspace_id: &str) -> SigningKey {
    derive_workspace_signing_key_via(&DevMasterSeedHkdf, workspace_id).expect(
        "DevMasterSeedHkdf cannot fail: in-memory HKDF-Expand of 32 bytes is well \
         within the 8160-byte ceiling",
    )
}

/// Per-tenant identity for a workspace: the canonical kid the verifier
/// expects in `EventSignature.kid` plus the URL-safe-no-pad base64 of
/// the public key for embedding in the `PubkeyBundle`.
///
/// **V1.11 W1 hardening (H-1).** This struct intentionally holds NO
/// secret material. V1.9–V1.10 carried a `secret_hex: String` field
/// alongside the public material; the unprotected `String` lingered in
/// the freed-allocator pool for the lifetime of any holder, even
/// though the `derive-pubkey` and `rotate-pubkey-bundle` paths never
/// read it. V1.11 splits the ceremony-only secret emission into
/// [`per_tenant_ceremony_output_via`], which returns the secret in a
/// [`Zeroizing<String>`] wrapper scoped tightly to the
/// `derive-key` subcommand's JSON output. Public-only consumers
/// (`derive-pubkey`, `rotate-pubkey-bundle`, MCP-side bundle assembly)
/// continue to use [`per_tenant_identity_via`] and never touch heap-
/// resident secret bytes.
///
/// **Visibility (V1.10 lib refactor):** `pub` so the binary in
/// `src/main.rs` (now a separate crate from the library) can construct
/// the JSON output shapes for `derive-key` / `derive-pubkey` /
/// `rotate-pubkey-bundle`.
///
/// **Debug derive is safe now.** With no sensitive field on the
/// struct, the manual redaction-only `Debug` impl from V1.9–V1.10 is
/// no longer required — the auto-derived `Debug` cannot leak secret
/// material because the struct holds none.
#[derive(Clone, Debug)]
pub struct PerTenantIdentity {
    /// `format!("atlas-anchor:{workspace_id}")` — the per-tenant kid
    /// the verifier expects under strict mode.
    pub kid: String,
    /// 32-byte Ed25519 public key, base64url-no-pad encoded — wire
    /// format for `PubkeyBundle.keys`.
    pub pubkey_b64url: String,
}

/// Trait-routed counterpart of `per_tenant_identity`.
///
/// Derives the workspace signing key via the supplied
/// [`MasterSeedHkdf`] and stitches the canonical kid + base64url
/// pubkey into one public-only record. The MCP server consumes this
/// via the `derive-pubkey` JSON output and via direct lib-API calls
/// for bundle assembly.
///
/// V1.10 binary call sites pair this with [`master_seed_loader`] to
/// enforce positive opt-in for the dev seed; the loader selects the
/// `DevMasterSeedHkdf` or sealed-seed PKCS#11 impl and hands it here.
///
/// **V1.11 W1 (H-1):** the secret hex is no longer returned. The
/// signing key bytes are wrapped in `Zeroizing` inside
/// [`derive_workspace_signing_key_via`] and dropped before this
/// function returns — only the (already-public) verifying key bytes
/// transit out. Callers needing the secret for the
/// `derive-key` ceremony route through
/// [`per_tenant_ceremony_output_via`] instead.
///
/// Returns `Result` because the trait is fallible — a sealed-seed
/// impl can return `MasterSeedError::Locked`/`Unavailable` mid-call.
/// The dev impl never errors in practice, but propagating the
/// `Result` keeps every per-tenant call site honest about V1.10
/// failure modes.
pub fn per_tenant_identity_via<H: MasterSeedHkdf + ?Sized>(
    hkdf: &H,
    workspace_id: &str,
) -> Result<PerTenantIdentity, MasterSeedError> {
    use base64::Engine;
    let signing_key = derive_workspace_signing_key_via(hkdf, workspace_id)?;
    let pubkey_bytes = signing_key.verifying_key().to_bytes();
    let pubkey_b64url =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pubkey_bytes);
    Ok(PerTenantIdentity {
        kid: per_tenant_kid_for(workspace_id),
        pubkey_b64url,
    })
}

/// Ceremony-only: derive both the public per-tenant identity AND the
/// hex-encoded secret-key material. The secret is wrapped in
/// [`Zeroizing<String>`] so the heap-resident hex string is scrubbed
/// on every exit path (Ok, Err, panic) when the wrapper drops.
///
/// **V1.11 W1 (H-1).** This function is the sole supported entry
/// point for the `derive-key` subcommand's JSON output and the only
/// place per-tenant secret bytes legitimately cross the
/// [`atlas_signer`] lib boundary. Any other call site is a misuse —
/// routine signing should use `sign --derive-from-workspace` (the
/// hot-path that derives inside the signer process and never emits the
/// secret), and bundle assembly should use [`per_tenant_identity_via`]
/// (public-only).
///
/// **Caller obligations:**
///   * Limit the wrapper's lifetime to the JSON-emission scope. The
///     wrapper scrubs on drop; the longer it lives, the longer the
///     hex bytes are heap-resident.
///   * Do NOT clone the inner `String` into another allocation — that
///     would defeat the wrapper. Borrow `&*secret_hex` (yields
///     `&String`) or `&**secret_hex` (yields `&str`) for serialisation.
///   * Do NOT emit through `serde_json::to_string_pretty(&struct)` if
///     the struct's `Serialize` impl pulls the inner `String` into a
///     serde tree — the tree allocates unprotected intermediates. The
///     binary's `run_derive_key` builds the JSON output by hand into
///     a second `Zeroizing<String>` buffer for exactly this reason.
pub fn per_tenant_ceremony_output_via<H: MasterSeedHkdf + ?Sized>(
    hkdf: &H,
    workspace_id: &str,
) -> Result<(PerTenantIdentity, Zeroizing<String>), MasterSeedError> {
    use base64::Engine;
    let signing_key = derive_workspace_signing_key_via(hkdf, workspace_id)?;
    let pubkey_bytes = signing_key.verifying_key().to_bytes();
    let pubkey_b64url =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pubkey_bytes);
    // `signing_key.to_bytes()` returns an unwrapped `[u8; 32]` —
    // wrap it immediately so the byte array is scrubbed on every exit
    // path (Ok of this function, panic in `hex::encode`, …). Without
    // the wrapper the array would live as freed stack memory after this
    // function returns.
    let secret_bytes: Zeroizing<[u8; 32]> = Zeroizing::new(signing_key.to_bytes());
    // `hex::encode` allocates a heap `String` — wrap it likewise. The
    // returned `Zeroizing<String>` is the only legitimate heap copy of
    // the secret material from this point on; the caller's borrow chain
    // must keep that invariant.
    //
    // `secret_bytes.as_slice()` is deliberate over the more obvious
    // `*secret_bytes`: `[u8; 32]` is `Copy`, so the dereferenced form
    // would copy the array onto `hex::encode`'s stack frame outside the
    // `Zeroizing` wrapper, leaving an un-scrubbed copy of the secret
    // until the frame is reused. The slice borrow keeps the bytes
    // inside the wrapper for their entire lifetime. Clippy's
    // `needless_borrows_for_generic_args` would happily simplify
    // `&*secret_bytes` → `*secret_bytes` here without seeing the Copy
    // hazard; using `.as_slice()` returns `&[u8]` which clippy does not
    // try to "simplify" away.
    let secret_hex: Zeroizing<String> = Zeroizing::new(hex::encode(secret_bytes.as_slice()));
    let identity = PerTenantIdentity {
        kid: per_tenant_kid_for(workspace_id),
        pubkey_b64url,
    };
    Ok((identity, secret_hex))
}

/// Test-only convenience wrapping [`per_tenant_identity_via`] with
/// the dev master-seed impl. Mirrors `derive_workspace_signing_key_default`'s
/// `#[cfg(test)]` rationale: the function does not exist in non-test
/// builds, so no caller can bypass the gate by calling it from
/// binary code.
#[cfg(test)]
pub(crate) fn per_tenant_identity(workspace_id: &str) -> PerTenantIdentity {
    per_tenant_identity_via(&DevMasterSeedHkdf, workspace_id)
        .expect("DevMasterSeedHkdf cannot fail")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;

    #[test]
    fn derivation_is_deterministic() {
        let a = derive_workspace_signing_key(&DEV_MASTER_SEED, "alice");
        let b = derive_workspace_signing_key(&DEV_MASTER_SEED, "alice");
        assert_eq!(a.to_bytes(), b.to_bytes());
        assert_eq!(
            a.verifying_key().to_bytes(),
            b.verifying_key().to_bytes()
        );
    }

    #[test]
    fn different_workspaces_yield_different_keys() {
        let alice = derive_workspace_signing_key(&DEV_MASTER_SEED, "alice");
        let bob = derive_workspace_signing_key(&DEV_MASTER_SEED, "bob");
        assert_ne!(
            alice.to_bytes(),
            bob.to_bytes(),
            "alice and bob must derive independent secret scalars",
        );
        assert_ne!(
            alice.verifying_key().to_bytes(),
            bob.verifying_key().to_bytes(),
            "alice and bob must derive independent public keys",
        );
    }

    #[test]
    fn different_master_seeds_yield_different_keys() {
        // A rotated master seed must yield a disjoint key set, even for
        // the same workspace_id. This is the rotation property: an
        // operator who rotates the master seed produces an entirely new
        // key roster.
        let seed_a = [0x11u8; 32];
        let seed_b = [0x22u8; 32];
        let a = derive_workspace_signing_key(&seed_a, "alice");
        let b = derive_workspace_signing_key(&seed_b, "alice");
        assert_ne!(a.to_bytes(), b.to_bytes());
    }

    #[test]
    fn empty_workspace_id_still_derives() {
        // The HKDF call cannot fail for any UTF-8 workspace_id — the
        // `info` parameter has no length limit (output length does).
        // We do NOT prevent empty workspace_id at the derivation layer
        // because the strict-mode kid validation rejects the resulting
        // empty per-tenant kid via `parse_per_tenant_kid`. Layering the
        // check at the kid-validation layer keeps the derivation pure
        // and the policy in one place.
        let _ = derive_workspace_signing_key(&DEV_MASTER_SEED, "");
    }

    /// Pinned pubkey goldens for `derive_workspace_signing_key_default`.
    ///
    /// This is the V1.9 equivalent of the `atlas_anchor_pubkey_pem_is_pinned`
    /// fence in `anchor.rs`. Any change to `DEV_MASTER_SEED`, the HKDF
    /// info-prefix, the curve, or the encoder trips this test before
    /// silently rotating production keys.
    ///
    /// We pin two distinct workspace_ids so a degenerate change that
    /// happened to leave `alice` stable but broke other workspaces would
    /// still trip CI. The second pin (`ws-mcp-default`) matches the
    /// MCP server's `DEFAULT_WORKSPACE` so the smoke test's bundle hash
    /// becomes implicitly pinned through these values.
    ///
    /// If you intentionally change the derivation, regenerate both pins
    /// AND bump `atlas-trust-core`'s crate version so `VERIFIER_VERSION`
    /// cascades through old bundles, AND surface the rotation in
    /// `docs/SECURITY-NOTES.md`.
    #[test]
    fn workspace_pubkeys_are_pinned() {
        use base64::Engine;
        let pubkey = |ws: &str| -> String {
            let sk = derive_workspace_signing_key_default(ws);
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(sk.verifying_key().to_bytes())
        };

        // BEGIN PINNED — DO NOT EDIT WITHOUT INTENT.
        // Computed from `DEV_MASTER_SEED` + info `"atlas-anchor-v1:" + ws`.
        const ALICE_PUBKEY_B64URL: &str = "HaADbOvQvGRNVJnGFLLjj-qxC-zwReufz-8dAbBu9aY";
        const DEFAULT_PUBKEY_B64URL: &str = "_7VayPxHeadNxfSOw0p8E5LNXBNP2Mb-cOieCZRZq6M";
        // END PINNED.

        let alice = pubkey("alice");
        let default = pubkey("ws-mcp-default");

        assert_eq!(
            alice, ALICE_PUBKEY_B64URL,
            "V1.9 derivation drift for workspace 'alice'. If intentional, \
             regenerate both pins AND bump atlas-trust-core's crate version."
        );
        assert_eq!(
            default, DEFAULT_PUBKEY_B64URL,
            "V1.9 derivation drift for workspace 'ws-mcp-default'. If intentional, \
             regenerate both pins AND bump atlas-trust-core's crate version."
        );
        assert_ne!(
            alice, default,
            "Defence-in-depth: pinning two workspace_ids must not collide. \
             A collision means the derivation degenerated to a constant — \
             critical bug, fail loud."
        );
    }

    #[test]
    fn signature_round_trip() {
        // Sanity: a signature made by the derived key verifies under
        // the derived public key. Catches any future change that
        // breaks the SigningKey ↔ VerifyingKey relationship without
        // tripping a higher-level integration test.
        let sk = derive_workspace_signing_key_default("alice");
        let pk = sk.verifying_key();
        let msg = b"hello atlas v1.9";
        let sig = sk.sign(msg);
        pk.verify_strict(msg, &sig)
            .expect("derived key must produce verifiable signatures");
    }

    #[test]
    fn per_tenant_identity_kid_matches_trust_core_format() {
        let ident = per_tenant_identity("alice");
        assert_eq!(ident.kid, "atlas-anchor:alice");
        // pubkey_b64url is 43 chars (32 bytes b64url-no-pad) — sanity
        // check; downstream Zod schema enforces the same.
        assert_eq!(ident.pubkey_b64url.len(), 43);
    }

    /// V1.11 W1 (H-1): the public-only `PerTenantIdentity` struct
    /// must hold no sensitive material. The destructuring pattern
    /// below names every field exhaustively (no `..` rest pattern):
    /// adding a third field — even an inert one — breaks
    /// compilation and forces a reviewer to re-evaluate whether the
    /// new field is sensitive and whether
    /// [`per_tenant_ceremony_output_via`] is the right entry point
    /// for any secret material.
    ///
    /// This is the regression fence for the H-1 finding: V1.10
    /// carried a `secret_hex: String` field on this struct that
    /// lingered unprotected in the freed-allocator pool for the
    /// lifetime of any holder. V1.11 split the secret into the
    /// ceremony-only path; the destructure asserts the split.
    #[test]
    fn per_tenant_identity_struct_has_no_secret_field_v1_11_h1() {
        let ident = per_tenant_identity("alice");
        // Exhaustive destructure — adding a field to PerTenantIdentity
        // breaks this line and surfaces in code review.
        let PerTenantIdentity { kid, pubkey_b64url } = ident;
        assert_eq!(kid, "atlas-anchor:alice");
        assert_eq!(pubkey_b64url.len(), 43);
    }

    /// V1.11 W1 (H-1): the ceremony-only entry point must yield the
    /// 64-char hex secret in a `Zeroizing<String>` wrapper that is
    /// type-equivalent to the wrapper enforced by the function
    /// signature. The pubkey returned alongside must match the
    /// public-only `per_tenant_identity_via` output byte-for-byte —
    /// same HKDF derive, two emission paths.
    #[test]
    fn per_tenant_ceremony_output_via_yields_64_char_hex_v1_11_h1() {
        let public_only = per_tenant_identity_via(&DevMasterSeedHkdf, "alice")
            .expect("DevMasterSeedHkdf cannot fail");
        let (ident, secret_hex) =
            per_tenant_ceremony_output_via(&DevMasterSeedHkdf, "alice")
                .expect("DevMasterSeedHkdf cannot fail");
        assert_eq!(ident.kid, public_only.kid);
        assert_eq!(ident.pubkey_b64url, public_only.pubkey_b64url);
        // 64 chars = 32 bytes hex-encoded
        assert_eq!(secret_hex.len(), 64);
        // ASCII-hex shape — the binary's `build_derive_key_json`
        // injects the bytes directly into the JSON output without
        // escape, relying on this property.
        assert!(
            secret_hex.bytes().all(|b| b.is_ascii_hexdigit()),
            "secret_hex must be ASCII hex; got {:?}",
            secret_hex.as_str(),
        );
        // Roundtrip via SigningKey::from_bytes recreates the same
        // pubkey — the secret really does correspond to the public
        // identity returned alongside.
        use base64::Engine;
        let bytes: [u8; 32] = hex::decode(secret_hex.as_str())
            .expect("ascii-hex decode")
            .try_into()
            .expect("32 bytes");
        let recovered_pub = SigningKey::from_bytes(&bytes).verifying_key().to_bytes();
        let expected_pub = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&public_only.pubkey_b64url)
            .expect("b64url decode");
        assert_eq!(recovered_pub.as_slice(), expected_pub.as_slice());
    }

    #[test]
    fn validate_workspace_id_accepts_ordinary_ids() {
        for ok in ["alice", "ws-mcp-default", "Customer_42", "BANK.HAGEDORN"] {
            assert!(
                validate_workspace_id(ok).is_ok(),
                "expected {ok:?} to be accepted",
            );
        }
    }

    #[test]
    fn validate_workspace_id_rejects_empty() {
        assert!(validate_workspace_id("").is_err());
    }

    #[test]
    fn validate_workspace_id_rejects_colon() {
        // Colons collide with the kid prefix delimiter; we refuse them
        // even though `parse_per_tenant_kid` would tolerate them.
        let err = validate_workspace_id("ws:with:colons").unwrap_err();
        assert!(err.contains("':'"));
    }

    #[test]
    fn validate_workspace_id_rejects_whitespace_and_controls() {
        for bad in ["ws with space", "\tleading-tab", "trailing\n", "ws\0null"] {
            assert!(
                validate_workspace_id(bad).is_err(),
                "expected {bad:?} to be rejected",
            );
        }
    }

    #[test]
    fn validate_workspace_id_accepts_at_max_length() {
        // The cap is inclusive: a workspace_id of exactly
        // `WORKSPACE_ID_MAX_BYTES` bytes is accepted. One byte beyond
        // the cap is rejected (see next test).
        let at_max = "a".repeat(WORKSPACE_ID_MAX_BYTES);
        assert!(
            validate_workspace_id(&at_max).is_ok(),
            "expected workspace_id of exactly {WORKSPACE_ID_MAX_BYTES} bytes to be accepted",
        );
    }

    #[test]
    fn validate_workspace_id_rejects_oversized() {
        // One byte beyond the cap. The DoS surface flagged in V1.9
        // review was a 10 MB workspace_id producing a 10 MB
        // PubkeyBundle entry; we test the boundary because the
        // boundary is what catches off-by-one regressions, not a 10 MB
        // string in CI.
        let too_long = "a".repeat(WORKSPACE_ID_MAX_BYTES + 1);
        let err = validate_workspace_id(&too_long).unwrap_err();
        assert!(
            err.contains("cap"),
            "error must mention the cap so an operator can grep for it; got {err:?}",
        );
    }

    #[test]
    fn validate_workspace_id_rejects_non_ascii() {
        // Defends against Unicode confusables: two visually-identical
        // names with different code points derive different keys but
        // appear the same in operator UIs.
        for bad in ["Büro", "wś", "café"] {
            assert!(
                validate_workspace_id(bad).is_err(),
                "expected {bad:?} to be rejected",
            );
        }
    }

    /// Helper: build an env reader that returns `value` for `key` and
    /// `None` for everything else. Keeps the test bodies tight and
    /// makes the data-vs-policy separation visible.
    fn env_with(key: &'static str, value: Option<&'static str>) -> impl Fn(&str) -> Option<String> {
        move |name| {
            if name == key { value.map(|s| s.to_string()) } else { None }
        }
    }

    // The pre-V1.10-warm-up tests for this gate flipped the process env
    // via `unsafe { std::env::set_var }`. Cargo defaults to a thread
    // pool per test binary, so two such tests racing in the same
    // module corrupt each other's view of the variable. The injection
    // form below is race-free and also pre-builds the V1.10 seam: when
    // the HSM loader needs to inject its own seed source, it will use
    // the same closure pattern.

    #[test]
    fn production_gate_blocks_when_env_set_to_one() {
        let result = production_gate_with(env_with(PRODUCTION_GATE_ENV, Some("1")));
        assert!(result.is_err(), "production gate must reject ATLAS_PRODUCTION=1");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("DEV_MASTER_SEED"),
            "error must name the constant the operator needs to grep for; got {msg:?}",
        );
    }

    #[test]
    fn production_gate_allows_when_env_unset() {
        let result = production_gate_with(env_with(PRODUCTION_GATE_ENV, None));
        assert!(result.is_ok(), "production gate must allow when env unset");
    }

    #[test]
    fn production_gate_allows_when_env_empty() {
        // Empty value is treated identically to unset — only the
        // literal "1" trips the gate. V1.9 design choice: a
        // misconfigured pipeline that emits `ATLAS_PRODUCTION=` (with
        // no value) should not silently behave like production-gated;
        // it should behave like dev. V1.10 inverts this entirely
        // (positive opt-in), so this test pins the V1.9 behaviour
        // before the inversion lands.
        let result = production_gate_with(env_with(PRODUCTION_GATE_ENV, Some("")));
        assert!(result.is_ok(), "empty ATLAS_PRODUCTION must not trip the V1.9 gate");
    }

    #[test]
    fn production_gate_allows_truthy_strings_other_than_one() {
        // Conservative-by-V1.9-design: only "1" trips the gate. The
        // OPERATOR-RUNBOOK warns prominently that "true", "yes", "on"
        // do NOT trip it today; documenting that contract via test so
        // a future "be liberal in what you accept" patch trips CI
        // before reaching production.
        for v in ["0", "true", "yes", "on", "production", "TRUE", "1 "] {
            let result = production_gate_with(env_with(PRODUCTION_GATE_ENV, Some(v)));
            assert!(
                result.is_ok(),
                "value {v:?} must not trip the V1.9 gate (only literal \"1\" does)",
            );
        }
    }

    #[test]
    fn production_gate_default_uses_process_env() {
        // Sanity: the public `production_gate()` is the
        // `production_gate_with(std::env::var)` wrapper. We don't
        // mutate the env here — we just confirm the wrapper compiles
        // and returns something. The actual policy is covered by the
        // injection-form tests above.
        let _ = production_gate();
    }

    use crate::test_support::env_pairs;

    // V1.10 master_seed_gate — positive opt-in semantics, layered on
    // top of V1.9 ATLAS_PRODUCTION paranoia. The tests below pin both
    // the security boundary (refuse-by-default, refuse-on-typo,
    // refuse-when-production-flag-set) and the operator UX (accept
    // common truthy spellings case-insensitively, tolerate stray
    // whitespace from quoting/escaping in shell scripts).

    #[test]
    fn master_seed_gate_refuses_when_opt_in_unset() {
        let result = master_seed_gate_with(env_pairs(&[]));
        let err = result.expect_err("V1.10 default must refuse without explicit opt-in");
        assert!(
            err.contains(DEV_MASTER_SEED_OPT_IN_ENV),
            "error must name the env var so operators can grep for it; got {err:?}",
        );
    }

    #[test]
    fn master_seed_gate_refuses_when_opt_in_empty() {
        let result =
            master_seed_gate_with(env_pairs(&[(DEV_MASTER_SEED_OPT_IN_ENV, "")]));
        assert!(
            result.is_err(),
            "empty {DEV_MASTER_SEED_OPT_IN_ENV} must NOT be treated as opt-in",
        );
    }

    #[test]
    fn master_seed_gate_refuses_falsy_values() {
        // The strict allow-list is the V1.10 inversion of V1.9's
        // strict-only-"1" footgun: now the safe direction (refuse)
        // is the default, so misspellings or falsy values stay
        // refused instead of silently allowing.
        for v in ["0", "false", "no", "off", "FALSE", "False"] {
            let result =
                master_seed_gate_with(env_pairs(&[(DEV_MASTER_SEED_OPT_IN_ENV, v)]));
            assert!(
                result.is_err(),
                "falsy value {v:?} must NOT trip V1.10 opt-in",
            );
        }
    }

    #[test]
    fn master_seed_gate_refuses_typos_and_unknown_values() {
        // V1.10 inversion explicitly: anything outside the allow-list
        // refuses. A future "be liberal in what you accept" patch
        // would degrade the security boundary; this test pins it.
        for v in ["enabled", "yep", "please", "DevMasterSeed!", "1.0", "01"] {
            let result =
                master_seed_gate_with(env_pairs(&[(DEV_MASTER_SEED_OPT_IN_ENV, v)]));
            assert!(
                result.is_err(),
                "unknown value {v:?} must NOT trip V1.10 opt-in",
            );
        }
    }

    #[test]
    fn master_seed_gate_allows_recognised_truthy_values() {
        for v in ["1", "true", "yes", "on"] {
            let result =
                master_seed_gate_with(env_pairs(&[(DEV_MASTER_SEED_OPT_IN_ENV, v)]));
            assert!(
                result.is_ok(),
                "truthy value {v:?} must trip V1.10 opt-in",
            );
        }
    }

    #[test]
    fn master_seed_gate_truthy_values_are_case_insensitive() {
        // K8s/Docker manifests sometimes uppercase env values for
        // visual consistency; accept TRUE/YES/ON/On/yEs as if they
        // were the canonical lowercase spellings.
        for v in ["TRUE", "True", "YES", "Yes", "ON", "On", "1", "yEs"] {
            let result =
                master_seed_gate_with(env_pairs(&[(DEV_MASTER_SEED_OPT_IN_ENV, v)]));
            assert!(
                result.is_ok(),
                "case-variant {v:?} must trip V1.10 opt-in",
            );
        }
    }

    #[test]
    fn master_seed_gate_tolerates_surrounding_whitespace() {
        // A common deploy-script footgun is a stray space or trailing
        // newline from a shell heredoc. V1.9's strict-"1" gate
        // silently fell through on `"1 "` (trailing space); V1.10
        // tolerates whitespace explicitly so the operator's intent
        // is honoured rather than silently inverted.
        for v in [" 1", "1 ", " true ", "\ttrue", "yes\n", " On\r\n"] {
            let result =
                master_seed_gate_with(env_pairs(&[(DEV_MASTER_SEED_OPT_IN_ENV, v)]));
            assert!(
                result.is_ok(),
                "whitespace-padded {v:?} must trip V1.10 opt-in",
            );
        }
    }

    #[test]
    fn master_seed_gate_atlas_production_overrides_opt_in() {
        // Defence-in-depth: if the operator has set ATLAS_PRODUCTION=1
        // (V1.9 sense — "this is production, refuse dev seed"), that
        // refusal MUST stand even if ATLAS_DEV_MASTER_SEED=1 is also
        // set. A misconfigured pipeline that ships both flags should
        // not silently degrade to dev-seed signing.
        let result = master_seed_gate_with(env_pairs(&[
            (PRODUCTION_GATE_ENV, "1"),
            (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
        ]));
        let err = result.expect_err(
            "ATLAS_PRODUCTION=1 must override the V1.10 opt-in (V1.9 paranoia layer)",
        );
        assert!(
            err.contains(PRODUCTION_GATE_ENV),
            "error must surface the V1.9 paranoia variant first; got {err:?}",
        );
    }

    #[test]
    fn master_seed_gate_default_uses_process_env() {
        // Sanity: `master_seed_gate()` wraps `master_seed_gate_with(
        // std::env::var)`. We don't mutate the env here — we just
        // confirm the wrapper compiles and returns something. The
        // actual policy is covered by the injection-form tests above.
        let _ = master_seed_gate();
    }

    #[test]
    fn master_seed_gate_error_mentions_runbook() {
        // Error must point operators at the migration doc so a
        // V1.9→V1.10 upgrade has a clear remediation pathway.
        let err = master_seed_gate_with(env_pairs(&[])).unwrap_err();
        assert!(
            err.contains("OPERATOR-RUNBOOK"),
            "error should reference the runbook for V1.9→V1.10 migration; got {err:?}",
        );
    }

    // V1.10 wave 2 — master_seed_loader dispatcher tests. Pin the
    // dispatch order (HSM trio first, dev seed second) and the
    // refusal semantics on the obvious operator footguns.

    #[test]
    fn master_seed_loader_falls_through_to_dev_when_no_hsm_trio() {
        // Default-shape dev test: no HSM trio, dev opt-in set.
        // The loader must succeed and hand back a usable trait
        // object. Do NOT exercise pubkey derivation here — that's
        // covered by the dev golden tests; this test only pins the
        // dispatch outcome.
        let loader = master_seed_loader_with(env_pairs(&[(
            DEV_MASTER_SEED_OPT_IN_ENV,
            "1",
        )]));
        assert!(loader.is_ok(), "loader must succeed when opt-in is set");
    }

    #[test]
    fn master_seed_loader_dev_path_matches_dev_impl_byte_for_byte() {
        // Defence-in-depth: the Box<dyn MasterSeedHkdf> the loader
        // returns for the dev path MUST produce byte-identical
        // output to the explicit `DevMasterSeedHkdf` impl. Catches
        // a future regression where the dev branch accidentally
        // wraps the impl in a layer that shadows the derivation.
        let loader = master_seed_loader_with(env_pairs(&[(
            DEV_MASTER_SEED_OPT_IN_ENV,
            "1",
        )]))
        .expect("dev loader path must succeed");
        for ws in ["alice", "ws-mcp-default", "BANK.HAGEDORN"] {
            let via_loader = derive_workspace_signing_key_via(&*loader, ws)
                .expect("loader output must derive");
            let via_explicit = derive_workspace_signing_key_via(&DevMasterSeedHkdf, ws)
                .expect("dev impl is infallible");
            assert_eq!(
                via_loader.to_bytes(),
                via_explicit.to_bytes(),
                "loader-routed and explicit-dev paths must agree byte-for-byte for {ws:?}",
            );
        }
    }

    /// Helper: extract the error message from a loader call that
    /// MUST have failed. The Box<dyn MasterSeedHkdf> Ok variant
    /// doesn't impl Debug (the trait itself doesn't require it),
    /// so `.unwrap_err()` won't compile — explicit match it is.
    fn loader_err<F>(env: F) -> String
    where
        F: Fn(&str) -> Option<String>,
    {
        match master_seed_loader_with(env) {
            Ok(_) => panic!("expected loader to refuse, but it succeeded"),
            Err(e) => e,
        }
    }

    #[test]
    fn master_seed_loader_refuses_when_no_hsm_and_no_dev_opt_in() {
        // Default behaviour without any env: refuse. This is the V1.10
        // safety property — no silent fallback to dev seed without an
        // explicit positive opt-in.
        let err = loader_err(env_pairs(&[]));
        assert!(
            err.contains(DEV_MASTER_SEED_OPT_IN_ENV),
            "no-opt-in error must name the env var to grep for; got {err:?}",
        );
    }

    #[test]
    fn master_seed_loader_refuses_partial_hsm_trio() {
        // Operator footgun: a single HSM env var set without the
        // others. The loader MUST refuse — silent fallback to dev
        // seed would defeat the audit signal.
        let err = loader_err(env_pairs(&[(
            crate::hsm::config::PKCS11_LIB_ENV,
            "/usr/lib/softhsm/libsofthsm2.so",
        )]));
        assert!(
            err.contains("partial"),
            "partial HSM trio must produce 'partial' error; got {err:?}",
        );
    }

    #[test]
    fn master_seed_loader_refuses_unreachable_hsm_module() {
        // HSM trio set but the module path doesn't exist. The
        // loader MUST refuse with "HSM open failed" — NOT fall
        // through to dev seed. Sealed-seed init failure is fatal:
        // the operator's intent is clear (HSM mode), and silently
        // signing with a dev key would be the V1.10-class
        // silent-fallback regression.
        //
        // The module path must be absolute on the host OS so the
        // V1.10 wave-2 absolute-path guard in `HsmConfig::from_env`
        // doesn't fire ahead of the open-failure path. On Unix
        // `/no/such/...` qualifies; on Windows we need a drive
        // prefix.
        #[cfg(windows)]
        let module_path = "C:\\no\\such\\pkcs11\\module.dll";
        #[cfg(not(windows))]
        let module_path = "/no/such/pkcs11/module.so";

        let pin_path = std::env::temp_dir().join("atlas-test-nonexistent-pin");
        let err = loader_err(env_pairs(&[
            (crate::hsm::config::PKCS11_LIB_ENV, module_path),
            (crate::hsm::config::SLOT_ENV, "0"),
            (
                crate::hsm::config::PIN_FILE_ENV,
                pin_path.to_str().unwrap(),
            ),
        ]));
        assert!(
            err.contains("HSM open failed"),
            "unreachable HSM must surface 'HSM open failed' prefix so operators can grep \
             for it in stderr; got {err:?}",
        );
    }

    #[test]
    fn master_seed_loader_atlas_production_overrides_dev_opt_in() {
        // Layered defence carries through to the loader: even with
        // ATLAS_DEV_MASTER_SEED=1 set, ATLAS_PRODUCTION=1 still
        // refuses the dev path. This pins the V1.9 paranoia layer's
        // behaviour through the loader-dispatch wrapper.
        //
        // V1.11 L-8: this test now uses `master_seed_loader_with_writer`
        // with `std::io::sink()` to swallow the deprecation warning,
        // keeping cargo test output uncluttered while still exercising
        // the gate refusal. The warning emission itself is covered by
        // dedicated tests below.
        let mut sink = std::io::sink();
        let result = master_seed_loader_with_writer(
            env_pairs(&[
                (PRODUCTION_GATE_ENV, "1"),
                (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
            ]),
            &mut sink,
        );
        let err = match result {
            Ok(_) => panic!("expected loader to refuse with ATLAS_PRODUCTION=1"),
            Err(e) => e,
        };
        assert!(
            err.contains(PRODUCTION_GATE_ENV),
            "ATLAS_PRODUCTION=1 must override the dev opt-in even through the loader; \
             got {err:?}",
        );
    }

    // V1.11 L-8 — ATLAS_PRODUCTION deprecation warning. Verifies the
    // writer-based seam emits the warning text under the documented
    // conditions and stays silent in the unset / empty cases.

    #[test]
    fn master_seed_loader_emits_atlas_production_deprecation_warning() {
        // Whenever ATLAS_PRODUCTION is observed with non-whitespace
        // content, the loader must write a deprecation warning to the
        // supplied writer BEFORE any gate check runs (so the operator
        // sees the migration notice even on the gate-refuses path).
        // We pair it with ATLAS_DEV_MASTER_SEED=1 here so the gate
        // ultimately refuses (production_gate fires); the test asserts
        // only on the warning content, not the loader return value.
        let mut warnings = Vec::<u8>::new();
        let _ = master_seed_loader_with_writer(
            env_pairs(&[
                (PRODUCTION_GATE_ENV, "1"),
                (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
            ]),
            &mut warnings,
        );
        let text = String::from_utf8(warnings).expect("warning text must be UTF-8");
        assert!(
            text.contains(PRODUCTION_GATE_ENV),
            "warning must name the env var so operators can grep for it; got {text:?}",
        );
        assert!(
            text.contains("deprecated"),
            "warning must label the var as deprecated; got {text:?}",
        );
        assert!(
            text.contains("V1.12"),
            "warning must announce the removal target so operators have a deprecation \
             window; got {text:?}",
        );
        assert!(
            text.contains(DEV_MASTER_SEED_OPT_IN_ENV),
            "warning must reference the V1.10 positive-opt-in env var (the replacement \
             for the V1.9 paranoia layer); got {text:?}",
        );
        assert!(
            text.contains(crate::hsm::config::PKCS11_LIB_ENV),
            "warning must reference the HSM trio (the V1.10+ production audit signal); \
             got {text:?}",
        );
        assert!(
            text.contains("OPERATOR-RUNBOOK"),
            "warning must point operators at the migration doc; got {text:?}",
        );
    }

    #[test]
    fn master_seed_loader_no_warning_when_atlas_production_unset() {
        // Default deployment (ATLAS_PRODUCTION not in env): no
        // deprecation noise. The warning is only meaningful when the
        // operator has actively set the deprecated var.
        let mut warnings = Vec::<u8>::new();
        let _ = master_seed_loader_with_writer(
            env_pairs(&[(DEV_MASTER_SEED_OPT_IN_ENV, "1")]),
            &mut warnings,
        );
        assert!(
            warnings.is_empty(),
            "no warning expected when ATLAS_PRODUCTION is unset; got {:?}",
            String::from_utf8_lossy(&warnings),
        );
    }

    #[test]
    fn master_seed_loader_no_warning_when_atlas_production_empty_or_whitespace() {
        // Misconfigured pipelines that emit `ATLAS_PRODUCTION=` (with no
        // value) or `ATLAS_PRODUCTION="   "` are NOT operator intent to
        // mark production — the V1.9 production_gate already treats
        // these as unset (only literal `"1"` trips the gate). The
        // deprecation warning mirrors that semantic: don't blame the
        // operator for a pipeline-emitted empty value they didn't write.
        for value in ["", "   ", "\t\n"] {
            let mut warnings = Vec::<u8>::new();
            let _ = master_seed_loader_with_writer(
                env_pairs(&[
                    (PRODUCTION_GATE_ENV, value),
                    (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
                ]),
                &mut warnings,
            );
            assert!(
                warnings.is_empty(),
                "no warning expected for pipeline-empty ATLAS_PRODUCTION={value:?}; \
                 got {:?}",
                String::from_utf8_lossy(&warnings),
            );
        }
    }

    #[test]
    fn master_seed_loader_warning_fires_for_any_non_empty_value() {
        // Mirror the V1.10 master_seed_gate's tolerance: V1.9 operators
        // who reflexively wrote ATLAS_PRODUCTION=true (the documented
        // V1.9 footgun) get the same deprecation notice as those who
        // wrote =1. The warning is about the env var's existence, not
        // about whether the gate would fire on the value.
        for value in ["1", "true", "false", "yes", "0", "anything"] {
            let mut warnings = Vec::<u8>::new();
            let _ = master_seed_loader_with_writer(
                env_pairs(&[
                    (PRODUCTION_GATE_ENV, value),
                    (DEV_MASTER_SEED_OPT_IN_ENV, "1"),
                ]),
                &mut warnings,
            );
            let text = String::from_utf8(warnings).expect("UTF-8");
            assert!(
                text.contains("deprecated"),
                "warning must fire for ATLAS_PRODUCTION={value:?}; got {text:?}",
            );
        }
    }

    #[test]
    fn master_seed_loader_default_uses_process_env() {
        // Sanity: the no-arg `master_seed_loader()` wraps
        // `master_seed_loader_with(std::env::var)`. We don't mutate
        // the env here — we just confirm the wrapper compiles.
        let _ = master_seed_loader();
    }

    #[test]
    fn per_tenant_identity_via_matches_no_arg_form() {
        // Sanity: the trait-routed and convenience forms must agree
        // when both run against `DevMasterSeedHkdf`. Catches a future
        // refactor where the no-arg form drifts from the trait-routed
        // form (e.g. someone re-implements the kid prefix in one
        // place but not the other).
        //
        // V1.11 W1 (H-1): the public-only struct carries no secret
        // field, so the comparison covers `kid` and `pubkey_b64url`
        // only. The ceremony-only secret is verified separately in
        // `per_tenant_ceremony_output_via_yields_64_char_hex_v1_11_h1`.
        for ws in ["alice", "ws-mcp-default"] {
            let direct = per_tenant_identity(ws);
            let via = per_tenant_identity_via(&DevMasterSeedHkdf, ws)
                .expect("dev impl is infallible");
            assert_eq!(direct.kid, via.kid);
            assert_eq!(direct.pubkey_b64url, via.pubkey_b64url);
        }
    }

    /// Compile-time guarantee that the trait surface stays
    /// thread-shareable. V1.10's MCP server path will hold a
    /// `Arc<dyn MasterSeedHkdf>` across async tasks; if a future
    /// PKCS#11 impl accidentally introduces non-`Send`/`Sync` state
    /// (e.g. an `Rc`), this assertion fires before the linker does.
    #[test]
    fn master_seed_hkdf_dev_impl_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DevMasterSeedHkdf>();
    }

    /// The trait-routed dev path must produce byte-identical output
    /// to the explicit-seed function when both run against
    /// [`DEV_MASTER_SEED`]. This is the load-bearing invariant of
    /// Task #8: introducing the trait abstraction MUST NOT change
    /// any derived pubkey, or every existing PubkeyBundle on disk
    /// becomes invalid the moment V1.10 lands.
    #[test]
    fn dev_hkdf_matches_explicit_seed_path() {
        for ws in ["alice", "bob", "ws-mcp-default", "BANK.HAGEDORN"] {
            let via_trait = derive_workspace_signing_key_via(&DevMasterSeedHkdf, ws)
                .expect("dev impl is infallible");
            let via_explicit = derive_workspace_signing_key(&DEV_MASTER_SEED, ws);
            assert_eq!(
                via_trait.to_bytes(),
                via_explicit.to_bytes(),
                "trait-routed and explicit-seed paths must agree byte-for-byte for {ws:?}",
            );
            assert_eq!(
                via_trait.verifying_key().to_bytes(),
                via_explicit.verifying_key().to_bytes(),
                "derived pubkeys must match for {ws:?} — trait abstraction must not \
                 reroute the algorithm",
            );
        }
    }

    /// `derive_workspace_signing_key_default` (the in-crate
    /// dispatch entry point) must agree with the trait-routed path.
    /// Catches a future regression where someone "optimises" the
    /// default-routing function to bypass the trait — that would
    /// silently ship a different code path to production than the
    /// one V1.10 will plug HSM impls into.
    #[test]
    fn default_dispatch_agrees_with_trait_routed_path() {
        for ws in ["alice", "ws-mcp-default"] {
            let direct = derive_workspace_signing_key_default(ws);
            let via_trait = derive_workspace_signing_key_via(&DevMasterSeedHkdf, ws)
                .expect("dev impl is infallible");
            assert_eq!(direct.to_bytes(), via_trait.to_bytes());
        }
    }

    /// Mock impl that returns a configurable `MasterSeedError`. Used
    /// to verify that `derive_workspace_signing_key_via` propagates
    /// every error variant verbatim — V1.10's PKCS#11 impl will
    /// emit these errors and the call site MUST surface them
    /// without remapping (operators rely on the variant + message
    /// for diagnosis).
    struct FailingHkdf {
        err: fn() -> MasterSeedError,
    }

    impl MasterSeedHkdf for FailingHkdf {
        fn derive_for(
            &self,
            _info: &[u8],
            _out: &mut [u8; 32],
        ) -> Result<(), MasterSeedError> {
            Err((self.err)())
        }
    }

    #[test]
    fn derive_via_propagates_locked_error() {
        let hkdf = FailingHkdf {
            err: || MasterSeedError::Locked("HSM PIN required".to_string()),
        };
        let err = derive_workspace_signing_key_via(&hkdf, "alice").unwrap_err();
        assert!(
            matches!(err, MasterSeedError::Locked(ref m) if m.contains("PIN")),
            "Locked variant + message must propagate verbatim; got {err:?}",
        );
    }

    #[test]
    fn derive_via_propagates_unavailable_error() {
        let hkdf = FailingHkdf {
            err: || {
                MasterSeedError::Unavailable(
                    "PKCS#11 token slot 0 empty".to_string(),
                )
            },
        };
        let err = derive_workspace_signing_key_via(&hkdf, "alice").unwrap_err();
        assert!(
            matches!(err, MasterSeedError::Unavailable(ref m) if m.contains("slot 0")),
            "Unavailable variant + message must propagate verbatim; got {err:?}",
        );
    }

    #[test]
    fn derive_via_propagates_derive_failed_error() {
        let hkdf = FailingHkdf {
            err: || {
                MasterSeedError::DeriveFailed(
                    "CKR_MECHANISM_INVALID".to_string(),
                )
            },
        };
        let err = derive_workspace_signing_key_via(&hkdf, "alice").unwrap_err();
        assert!(
            matches!(err, MasterSeedError::DeriveFailed(ref m) if m.contains("CKR_")),
            "DeriveFailed variant + message must propagate verbatim; got {err:?}",
        );
    }

    /// Mock impl that captures the `info` bytes the call sees, so we
    /// can pin the exact wire-format string the V1.10 PKCS#11 impl
    /// will receive. If anyone refactors the info-prefix assembly,
    /// this test trips before the change reaches production.
    struct CapturingHkdf {
        captured: std::cell::RefCell<Vec<u8>>,
    }

    impl MasterSeedHkdf for CapturingHkdf {
        fn derive_for(
            &self,
            info: &[u8],
            out: &mut [u8; 32],
        ) -> Result<(), MasterSeedError> {
            *self.captured.borrow_mut() = info.to_vec();
            // Fill with a deterministic non-zero pattern so
            // SigningKey::from_bytes succeeds and the call site
            // doesn't ratchet a mock-specific assertion.
            out.fill(0x42);
            Ok(())
        }
    }

    // SAFETY note for the test pool: `CapturingHkdf` uses `RefCell`
    // (not `Sync`), but we don't move it across threads here — the
    // capture is done from the same thread that calls `derive_for`.
    // The trait declares `Send + Sync`, so we hand-implement them as
    // a no-op witness to satisfy the bound for *this single-threaded
    // test only*. Production impls (`DevMasterSeedHkdf`, V1.10's
    // PKCS#11 impl) are genuinely thread-safe.
    unsafe impl Send for CapturingHkdf {}
    unsafe impl Sync for CapturingHkdf {}

    #[test]
    fn derive_via_passes_full_info_string_with_prefix() {
        let mock = CapturingHkdf {
            captured: std::cell::RefCell::new(Vec::new()),
        };
        let _ = derive_workspace_signing_key_via(&mock, "alice").expect("mock returns Ok");
        let info = mock.captured.into_inner();
        assert_eq!(
            info,
            b"atlas-anchor-v1:alice".to_vec(),
            "trait must receive the FULL HKDF info string (prefix + workspace_id), \
             not the bare workspace_id — V1.10 HSM impls rely on receiving the \
             pre-assembled domain-separation tag verbatim",
        );
    }

    /// Runtime dispatch via `&dyn MasterSeedHkdf` must compile and
    /// produce the same key as static dispatch. V1.10 will use this
    /// form when the gate selects dev-vs-HSM at startup time and
    /// stores the choice in a trait object.
    #[test]
    fn derive_via_works_through_dyn_dispatch() {
        let dev: &dyn MasterSeedHkdf = &DevMasterSeedHkdf;
        let via_dyn = derive_workspace_signing_key_via(dev, "alice")
            .expect("dev impl is infallible");
        let via_static = derive_workspace_signing_key_via(&DevMasterSeedHkdf, "alice")
            .expect("dev impl is infallible");
        assert_eq!(via_dyn.to_bytes(), via_static.to_bytes());
    }

    /// `MasterSeedError`'s `Display` impl must surface the inner
    /// message so operator-facing logs are useful — without this,
    /// the V1.10 PKCS#11 impl's `CKR_*` codes would disappear into
    /// generic "master seed source locked" lines.
    #[test]
    fn master_seed_error_display_includes_inner_message() {
        let e = MasterSeedError::Locked("PIN required".to_string());
        let s = format!("{e}");
        assert!(s.contains("PIN required"), "Display lost inner message: {s:?}");
        assert!(s.contains("locked"), "Display lost variant tag: {s:?}");
    }

    /// V1.11 W1 (H-1): the public-only struct's auto-derived `Debug`
    /// is now safe — there is no sensitive field to redact. The V1.10
    /// manual `Debug` impl was a workaround for the `secret_hex: String`
    /// field on the same struct; with that field moved to the
    /// ceremony-only [`per_tenant_ceremony_output_via`] return value
    /// (wrapped in `Zeroizing<String>`), the auto-derived `Debug`
    /// cannot leak secret bytes because the struct holds none.
    ///
    /// We still assert that the public material appears so a regression
    /// to manual `Debug` that drops field values would trip this test.
    #[test]
    fn per_tenant_identity_debug_shows_public_fields_only_v1_11_h1() {
        let identity = per_tenant_identity("alice");
        let dbg_out = format!("{identity:?}");
        assert!(
            dbg_out.contains(&identity.kid),
            "Debug must include kid for diagnostics; got {dbg_out:?}",
        );
        assert!(
            dbg_out.contains(&identity.pubkey_b64url),
            "Debug must include pubkey_b64url for diagnostics; got {dbg_out:?}",
        );
    }

    /// V1.11 W1 (H-1): the ceremony-only `Zeroizing<String>` does
    /// expose the secret bytes via `Display` / `Debug` — that is
    /// intentional, the wrapper protects against heap residency on
    /// drop, not against intentional inspection. This test
    /// documents the boundary by asserting the wrapper IS dereferenceable
    /// to its inner str (so the binary's JSON-emit path can read the
    /// bytes) but does NOT auto-zeroize on every borrow.
    ///
    /// The trust property the wrapper provides — heap scrubbing on
    /// drop — is supplied by the `zeroize` crate and exercised by its
    /// own test suite; we don't re-test it here. We only assert the
    /// API surface this crate depends on.
    #[test]
    fn ceremony_secret_hex_is_dereferenceable_to_str_v1_11_h1() {
        let (_, secret_hex) =
            per_tenant_ceremony_output_via(&DevMasterSeedHkdf, "alice")
                .expect("DevMasterSeedHkdf cannot fail");
        // Borrow paths the binary's `build_derive_key_json` relies on:
        let as_str_ref: &str = secret_hex.as_str();
        assert_eq!(as_str_ref.len(), 64);
        let via_double_deref: &str = &secret_hex;
        assert_eq!(via_double_deref, as_str_ref);
    }
}
