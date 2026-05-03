//! `atlas-signer` library surface.
//!
//! V1.5–V1.9, `atlas-signer` was binary-only (`src/main.rs` plus
//! sibling modules `anchor`, `chain`, `keys`, `rekor_client`). V1.10
//! wave 1 promoted the per-module surface into a library so the
//! binary in `src/main.rs` could continue to wire CLI parsing on top.
//! V1.10 wave 2 (this commit) adds the sealed-seed loader
//! ([`hsm`]) — a PKCS#11-backed [`keys::MasterSeedHkdf`] impl that
//! performs HKDF derivation **inside** an HSM/TPM. The master seed
//! never enters Atlas address space.
//!
//! **No behaviour change at the bin↔lib split.** Every V1.9 module
//! retains its API verbatim; the lib root is a thin re-export. Tests,
//! pinned-pubkey goldens, and on-disk byte formats are unaffected.
//! The bin consumes the library; so does the test harness.
//!
//! **Module surface:**
//!
//!   * [`anchor`] — Mock-Rekor + live-Sigstore anchor batch issuer.
//!   * [`chain`] — V1.7 anchor-chain.jsonl writer + chain-export.
//!   * [`keys`] — V1.9 per-tenant HKDF derivation + V1.10
//!     [`MasterSeedHkdf`](keys::MasterSeedHkdf) trait + V1.10
//!     [`master_seed_gate`](keys::master_seed_gate) positive-opt-in
//!     gate.
//!   * [`hsm`] — V1.10 wave 2 sealed-seed loader. PKCS#11 backend
//!     gated behind the `hsm` feature; without the feature, the
//!     loader is a stub that fails closed with a clear remediation
//!     message.
//!   * [`rekor_client`] — HTTP client for live Sigstore Rekor v1
//!     submission. Used internally by [`anchor`].
//!   * [`workspace_signer`] — V1.11 Scope A wave-3 abstraction over
//!     per-workspace signing. Phase A (this commit) ships the trait
//!     and a dev impl that wraps [`keys::MasterSeedHkdf`]; Phase B
//!     adds a sealed PKCS#11 impl that owns the per-tenant scalar
//!     end-to-end inside the HSM; Phase C wires the dispatcher into
//!     the binary's `run_sign` flow.

pub mod anchor;
pub mod chain;
pub mod hsm;
pub mod keys;
pub mod rekor_client;
pub mod workspace_signer;

#[cfg(test)]
pub(crate) mod test_support;
