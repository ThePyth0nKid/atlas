//! Per-tenant key conventions (V1.9).
//!
//! V1.5–V1.8 signed every event with one of three globally-shared
//! Ed25519 keypairs (agent / human / anchor) baked into
//! `apps/atlas-mcp-server/src/lib/keys.ts`. A compromise of any of
//! those three keys forged events for *every* workspace at once.
//!
//! V1.9 introduces per-tenant signing keys: the issuer derives a
//! per-workspace Ed25519 keypair from a single master seed via HKDF-SHA256
//! and exposes the resulting public key in the `PubkeyBundle` under a kid
//! of the shape `atlas-anchor:{workspace_id}`. Compromise of one
//! workspace's signing key does not compromise other workspaces' keys —
//! HKDF is one-way per-info.
//!
//! This module owns the kid-shape convention shared between issuer and
//! verifier:
//!
//!   * `PER_TENANT_KID_PREFIX` — the literal `"atlas-anchor:"` prefix.
//!   * `per_tenant_kid_for(workspace_id)` — returns the canonical
//!     per-tenant kid for a workspace.
//!   * `parse_per_tenant_kid(kid)` — returns `Some(workspace_id)` if and
//!     only if `kid` is a well-formed per-tenant kid.
//!
//! The HKDF *derivation* itself lives in `atlas-signer::keys` —
//! `atlas-trust-core` never sees the master seed. The verifier only
//! consumes derived public keys via the bundle. A Verifier with the
//! master seed would defeat the per-tenant separation: re-deriving any
//! workspace key locally is exactly the capability we want to keep on
//! the issuer side.

/// Canonical prefix for per-tenant kids in the `PubkeyBundle`.
///
/// The full kid is `format!("{PER_TENANT_KID_PREFIX}{workspace_id}")`.
/// The colon is part of the prefix so any future namespace (`atlas-policy:`,
/// `atlas-witness:`) sits next to this one without ambiguity.
pub const PER_TENANT_KID_PREFIX: &str = "atlas-anchor:";

/// Build the canonical per-tenant kid for `workspace_id`.
///
/// `workspace_id` is taken verbatim — no escaping, no normalisation.
/// Workspace identifiers are caller-controlled strings; the trust
/// property survives because the kid is bound into the `EventSignature`
/// the verifier compares against, and the per-tenant strict-mode check
/// recomputes the expected kid from `trace.workspace_id` and compares
/// byte-for-byte.
pub fn per_tenant_kid_for(workspace_id: &str) -> String {
    format!("{PER_TENANT_KID_PREFIX}{workspace_id}")
}

/// Return `Some(workspace_id)` if `kid` is a per-tenant kid, else `None`.
///
/// Strict semantics: the prefix must match exactly, and the suffix must
/// be non-empty. An empty workspace_id (`"atlas-anchor:"` with no
/// suffix) is rejected — there is no legitimate per-tenant kid that
/// names the empty workspace.
///
/// ## Caller-contract: leniency on the verifier side, hygiene on the issuer side
///
/// `parse_per_tenant_kid` accepts any non-empty UTF-8 suffix — including
/// strings containing colons, whitespace, or non-ASCII bytes (see the
/// `parse_accepts_unusual_but_legal_workspace_ids` test). This is
/// deliberate: the trust property holds for *any* such string because
/// the verifier compares `kid` byte-for-byte against the recomputed
/// `format!("{PER_TENANT_KID_PREFIX}{trace.workspace_id}")`, and
/// HKDF-SHA256 is deterministic over arbitrary UTF-8.
///
/// Operator-facing hygiene (forbidding whitespace, control characters,
/// non-ASCII confusables, and the colon delimiter) lives on the issuer
/// side in `atlas-signer::keys::validate_workspace_id`. That is the
/// place where ambiguous IDs become footguns and observability holes;
/// that is also the only place that can refuse to derive a key in the
/// first place. Layering the policy on the verifier side would
/// double-encode it, and the verifier would then have to refuse legacy
/// per-tenant traces if the policy ever changed — a maintenance trap.
///
/// If you want to reject a class of workspace_ids, change
/// `validate_workspace_id` upstream. The verifier should remain a pure
/// byte-equality check.
pub fn parse_per_tenant_kid(kid: &str) -> Option<&str> {
    let suffix = kid.strip_prefix(PER_TENANT_KID_PREFIX)?;
    if suffix.is_empty() {
        return None;
    }
    Some(suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_tenant_kid_is_deterministic_string() {
        assert_eq!(per_tenant_kid_for("alice"), "atlas-anchor:alice");
        assert_eq!(per_tenant_kid_for("ws-mcp-default"), "atlas-anchor:ws-mcp-default");
    }

    #[test]
    fn parse_round_trip() {
        let kid = per_tenant_kid_for("alice");
        assert_eq!(parse_per_tenant_kid(&kid), Some("alice"));
    }

    #[test]
    fn parse_rejects_legacy_kids() {
        assert_eq!(parse_per_tenant_kid("spiffe://atlas/agent/cursor-001"), None);
        assert_eq!(parse_per_tenant_kid(""), None);
    }

    #[test]
    fn parse_rejects_empty_workspace() {
        // A bare prefix without a workspace_id is malformed: `"atlas-anchor:"`
        // claims to be per-tenant but names no tenant.
        assert_eq!(parse_per_tenant_kid("atlas-anchor:"), None);
    }

    #[test]
    fn parse_accepts_unusual_but_legal_workspace_ids() {
        // We do not normalise or restrict workspace_id — it is caller-
        // controlled and the trust property holds via per-byte string
        // equality at verify time. Document the lenient behaviour with
        // a few stress cases.
        assert_eq!(parse_per_tenant_kid("atlas-anchor:ws/with/slashes"), Some("ws/with/slashes"));
        assert_eq!(parse_per_tenant_kid("atlas-anchor:ws:with:colons"), Some("ws:with:colons"));
        assert_eq!(parse_per_tenant_kid("atlas-anchor: leading-space"), Some(" leading-space"));
    }
}
