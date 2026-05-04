//! V1.15 Welle A anti-drift pin: every KID-equality in production code
//! must route through `crate::ct::ct_eq_str` for const-time compare.
//!
//! This test file's existence documents the invariant; the test bodies
//! enforce it source-level. A failing assertion here means a future change
//! in `verify.rs` or `witness.rs` has re-introduced a `==` on a wire-side
//! KID, which would re-open the timing-side-channel that V1.15 Welle A
//! closed (and the V1.13 wave-C-2 roster lookup closed before it).
//!
//! See `docs/SECURITY-NOTES.md` `## scope-a` for the full trust-property
//! statement and the enumerated boundaries the const-time invariant
//! protects.
//!
//! # Scope
//!
//! The audit deliberately covers the two source files that carry every
//! production KID-equality site reachable from the verifier:
//!
//! - `crates/atlas-trust-core/src/verify.rs` — V1.9 per-tenant-kid strict
//!   mode (the V1.15 Welle A target) and the V1.13 chain-aggregator's
//!   cross-batch witness dedup branch.
//! - `crates/atlas-trust-core/src/witness.rs` — V1.13 wave-C-2 roster
//!   lookup (already on `ct_eq_str` since V1.13) and the per-batch
//!   witness verifier helper.
//!
//! Other modules (`anchor.rs`, `hashchain.rs`, `pubkey_bundle.rs`,
//! `cose.rs`) compare hashes/heads, never KIDs — those compares already
//! route through `ct_eq_str` per the V1.5 invariant pinned by
//! `bundle_hash_byte_determinism_pin` and friends. Adding them to the
//! audit here would not strengthen the V1.15 invariant; the per-module
//! `ct_eq_str` discipline is enforced by the existing pin tests in those
//! files.
//!
//! # Known limitations
//!
//! The audit is purely textual — it strips line comments, removes
//! whitespace, then greps for forbidden shapes. Five classes of bypass
//! / gap remain out of scope and are documented here so a future
//! reviewer doesn't mistake them for accidental holes:
//!
//! 1. **Intermediate bindings.** `let k = &ev.signature.kid; k == &x;`
//!    rebinds the KID to a non-`kid`-named local before comparing. A
//!    purely textual audit can't see the binding's provenance without a
//!    `syn` AST walk, which we deliberately avoid for dev-build cost.
//!    Mitigated by code review: any `let _ = …kid` rebinding in
//!    `verify.rs` / `witness.rs` is reviewer-flagged on sight, and the
//!    enumerated boundaries in `crate::ct`'s module-doc make the rule
//!    plain to reviewers.
//! 2. **Generic helpers.** Passing the KID into a `fn cmp<T: Eq>(…)`
//!    helper that uses `==` internally hides the equality from grep.
//!    There are no such helpers in the audited files today, and every
//!    existing string/hash helper already routes through `ct_eq_str`.
//! 3. **`BTreeSet` / `BTreeMap` lookups on KIDs.** `verify.rs:990,996`
//!    use `verified_kids.contains(&w.witness_kid)` and
//!    `batch_failed_kids.contains(w.witness_kid.as_str())`; `witness.rs`
//!    uses `kid_counts.entry(w.witness_kid.as_str())`. These resolve via
//!    `Ord` (lexicographic, NOT constant-time), so a timing oracle on
//!    the lookup leaks prefix-match length on the wire-side `witness_kid`.
//!    Per `crate::ct` boundaries-not-covered (point 1), the leaked
//!    information is the same set of kids already exposed in the trace
//!    being verified — no *new* attacker information — so the trade-off
//!    is accepted. Replacing the `BTreeSet`/`BTreeMap` with a CT-equality
//!    structure would be a structural change well beyond V1.15 Welle A's
//!    scope; documented here so the gap is visible alongside the audit.
//! 4. **String literals containing `//`.** `strip_line_comments` splits
//!    each line at the first `//`, which would also truncate inside a
//!    string literal like `"https://example/…"`. Today no string literal
//!    in the audited files contains a forbidden pattern, so this is
//!    safe — but a future fixture URL or diagnostic string holding
//!    `.kid==` text would silently evade the audit. If that ever
//!    becomes plausible, switch to a real lexer.
//! 5. **Reverse-side RHS enumeration is illustrative, not exhaustive.**
//!    The `==…kid` patterns enumerate the historically-used bindings
//!    (`expected_kid`, `witness_kid`, `signature.kid`,
//!    `ev.signature.kid`, `w.witness_kid`, `witness.witness_kid`). A
//!    new RHS binding name not on this list would slip past the
//!    reverse-side check. The matching forward (LHS) pattern would
//!    still catch the comparison from the other side, so this is
//!    defence-in-depth — not a primary gap. Revisit the list whenever a
//!    new KID-bearing struct field or local is introduced.
//!
//! The `_kid==` LHS pattern is broad on purpose: it would also fire on
//! e.g. `verified_kids==` if such a `==` were ever written. That's an
//! over-match, not a bypass — `verified_kids` is a `BTreeSet<String>`
//! whose multi-element `==` is not a CT-relevant compare, but flagging
//! it is harmless (a future writer would route the compare through a
//! reviewable helper) and it keeps the pattern simple.
//!
//! Within those caveats, the audit catches every direct `==` shape the
//! codebase has historically used: spaced or unspaced operator,
//! `.eq(...)`, `.as_bytes()==`, `.as_str()==`, slice-form `[..]==`,
//! and the enumerated reverse-side `==…kid` shapes.

const VERIFY_RS: &str = include_str!("../src/verify.rs");
const WITNESS_RS: &str = include_str!("../src/witness.rs");

/// Strip the `#[cfg(test)] mod tests {` tail of a source file. Convention
/// in this crate: every module file with unit tests places the test mod
/// at the bottom, so trimming at the canonical `#[cfg(test)] mod` marker
/// keeps production-only source for the regex audit. A file without unit
/// tests is returned untouched.
///
/// Matches three shapes:
/// - `#[cfg(test)]\nmod tests {` — LF, the dominant form.
/// - `#[cfg(test)]\r\nmod tests {` — CRLF, on Windows checkouts.
/// - `#[cfg(test)] mod tests {` — one-line attribute form, defensively.
fn production_section(source: &str) -> &str {
    for marker in TEST_MOD_MARKERS {
        if let Some(idx) = source.find(marker) {
            return &source[..idx];
        }
    }
    source
}

/// All canonical openings of the per-file unit-test mod. Kept in one
/// place so [`production_section`] and
/// [`audited_files_have_exactly_one_test_mod_marker`] agree on what
/// counts as the boundary between production and test code.
const TEST_MOD_MARKERS: &[&str] = &[
    "#[cfg(test)]\nmod tests {",
    "#[cfg(test)]\r\nmod tests {",
    "#[cfg(test)] mod tests {",
];

/// Strip line comments (`//` to end-of-line) so prose in doc-comments or
/// inline notes mentioning forbidden patterns doesn't false-positive the
/// audit. The naive split-at-`//` is correct for this audit because the
/// forbidden patterns we hunt are not legal inside any string literal in
/// the audited files — `kid` only appears as a field access path or
/// local binding in production code.
fn strip_line_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Normalise the audit input: strip line comments, then *remove* every
/// whitespace character. Removing rather than collapsing means every
/// equivalent operator shape — `.kid==`, `.kid ==`, `.kid  ==`,
/// `.kid\n==` — reduces to the same canonical `.kid==` form, so a single
/// no-whitespace pattern in [`FORBIDDEN`] catches all spacing variants.
/// Removing whitespace cannot create false matches between unrelated
/// tokens because the patterns we hunt require contiguous characters
/// (`.kid`, `_kid`, `==`, `.eq(`) that already exist verbatim in the
/// source — whitespace was the only thing separating the `==`/`.eq(`
/// from the preceding identifier.
fn normalise_for_audit(source: &str) -> String {
    strip_line_comments(source)
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

/// Forbidden patterns: any production-code `==` / `!=` (or `.eq(...)` /
/// `.ne(...)`) against a KID field or KID-named local binding. The single
/// sanctioned production path for KID equality is
/// `crate::ct::ct_eq_str(&a, &b)`.
///
/// Patterns are matched against whitespace-stripped, comment-stripped
/// source (see [`normalise_for_audit`]), so each entry uses no
/// whitespace and any amount of whitespace in the original source still
/// matches.
///
/// The patterns are deliberately textual rather than syntactic — a Rust
/// AST walk would be more precise but adds a `syn` dev-dependency for a
/// one-line check. The textual audit catches every direct shape the
/// codebase has historically used; bypasses requiring intermediate
/// bindings or generic-helper indirection are documented as known
/// limitations in the module comment above.
const FORBIDDEN: &[&str] = &[
    // Direct field-access equality (KID on the LHS). `.kid==` covers
    // `signature.kid==`, `witness.kid==`, etc. via substring match.
    ".kid==",
    ".kid!=",
    ".kid.eq(",
    ".kid.ne(",
    // Coerced LHS shapes — `.as_str()` / `.as_bytes()` / slice / iter.
    ".kid.as_str()==",
    ".kid.as_bytes()==",
    ".kid.bytes().eq(",
    ".kid[..]==",
    // Local-binding equality (LHS). `_kid==` covers any local ending in
    // `_kid` (`expected_kid`, `witness_kid`, `tenant_kid`, etc.) and
    // also field accesses like `.witness_kid==` via substring match.
    "_kid==",
    "_kid!=",
    "_kid.eq(",
    "_kid.ne(",
    // Coerced LHS shapes for `_kid`-named bindings — symmetric with
    // the `.kid.as_str()==` / `.kid.as_bytes()==` entries above.
    "_kid.as_str()==",
    "_kid.as_bytes()==",
    "_kid.bytes().eq(",
    "_kid[..]==",
    // Reverse-side equality (KID on the RHS). The historically-used
    // bindings/paths enumerated explicitly; a future binding name not
    // listed here that ends in `_kid` would slip past this side of the
    // audit (acknowledged limitation), but the LHS-side patterns above
    // would catch the matching forward form on the same call site.
    "==expected_kid",
    "!=expected_kid",
    "==witness_kid",
    "!=witness_kid",
    "==signature.kid",
    "!=signature.kid",
    "==ev.signature.kid",
    "==w.witness_kid",
    "==witness.witness_kid",
];

fn assert_no_forbidden(label: &str, source: &str) {
    let prod = normalise_for_audit(production_section(source));
    for pat in FORBIDDEN {
        assert!(
            !prod.contains(pat),
            "{label} production code contains forbidden raw-equality pattern `{pat}` — \
             every KID compare must route through `crate::ct::ct_eq_str` (V1.15 Welle A invariant). \
             See `docs/SECURITY-NOTES.md` `## scope-a` for context and the enumerated boundaries.",
        );
    }
}

#[test]
fn verify_rs_has_no_raw_kid_equality_in_production() {
    assert_no_forbidden("verify.rs", VERIFY_RS);
}

#[test]
fn witness_rs_has_no_raw_kid_equality_in_production() {
    assert_no_forbidden("witness.rs", WITNESS_RS);
}

/// Sanity-check the `production_section` helper: it must NOT strip when
/// the marker is absent, and it must strip cleanly when present. A future
/// edit that breaks the trimmer would silently weaken every other test
/// in this file (since the audit would scan more or less code than
/// intended) — pin the helper's behaviour explicitly across the three
/// supported marker shapes (LF / CRLF / one-line attribute).
#[test]
fn production_section_helper_invariant() {
    let no_marker = "fn main() {}\n";
    assert_eq!(production_section(no_marker), no_marker);

    let with_marker = "fn main() {}\n\n#[cfg(test)]\nmod tests {\n    #[test] fn t() {}\n}\n";
    assert_eq!(production_section(with_marker), "fn main() {}\n\n");

    let with_crlf = "fn main() {}\r\n\r\n#[cfg(test)]\r\nmod tests {\r\n}\r\n";
    assert_eq!(production_section(with_crlf), "fn main() {}\r\n\r\n");

    let one_line = "fn main() {}\n\n#[cfg(test)] mod tests {\n    #[test] fn t() {}\n}\n";
    assert_eq!(production_section(one_line), "fn main() {}\n\n");
}

/// Sanity-check `normalise_for_audit`: spaced and unspaced equality
/// shapes both collapse to the same canonical no-whitespace form, and a
/// line comment mentioning a forbidden pattern is stripped before
/// normalisation so a reviewer can quote forbidden text in `//`
/// comments without tripping the audit.
#[test]
fn normalise_for_audit_strips_whitespace_and_comments() {
    assert!(normalise_for_audit("a.kid==b").contains(".kid=="));
    assert!(normalise_for_audit("a.kid  ==  b").contains(".kid=="));
    assert!(normalise_for_audit("a.kid\n    == b").contains(".kid=="));
    assert!(normalise_for_audit("a.kid\t==\tb").contains(".kid=="));

    // Comments are stripped before whitespace removal, so a forbidden
    // pattern hiding in a comment doesn't false-positive the audit.
    assert!(!normalise_for_audit("// .kid == bypass via comment\n").contains(".kid=="));
}

/// Self-audit: each audited source file must contain **exactly one**
/// `#[cfg(test)] mod tests` marker (counted across the three canonical
/// shapes in [`TEST_MOD_MARKERS`]). If a future refactor introduces a
/// second test mod (or removes the existing one), [`production_section`]
/// would silently strip the wrong amount of source and weaken every
/// audit in this file — pin the surface here so drift is detected
/// immediately.
#[test]
fn audited_files_have_exactly_one_test_mod_marker() {
    fn count_test_mod_markers(source: &str) -> usize {
        TEST_MOD_MARKERS
            .iter()
            .map(|m| source.matches(m).count())
            .sum()
    }

    assert_eq!(
        count_test_mod_markers(VERIFY_RS),
        1,
        "verify.rs must contain exactly one `#[cfg(test)] mod tests` marker — \
         the audit boundary in `production_section` is undefined otherwise."
    );
    assert_eq!(
        count_test_mod_markers(WITNESS_RS),
        1,
        "witness.rs must contain exactly one `#[cfg(test)] mod tests` marker — \
         the audit boundary in `production_section` is undefined otherwise."
    );
}
