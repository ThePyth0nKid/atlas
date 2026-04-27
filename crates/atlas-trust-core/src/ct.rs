//! Constant-time comparison helpers.
//!
//! Hash equality checks compare 32-byte blake3 outputs (or their 64-char hex
//! encodings). Standard `==` on `&str` short-circuits on the first differing
//! byte, which leaks prefix-match length over enough samples. For an offline
//! verifier this is mostly theoretical, but the property "byte-identical
//! verification regardless of input shape" is exactly what Atlas claims, and
//! the cost of `subtle::ConstantTimeEq` is nil. Pay it.
//!
//! Length differences are NOT side-channel-protected — a hash of the wrong
//! length is trivially detectable from the input itself.

use subtle::ConstantTimeEq;

/// Constant-time equality of two byte slices (after a length-equal check).
pub fn ct_eq_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

/// Constant-time equality of two strings (interpreted as bytes).
pub fn ct_eq_str(a: &str, b: &str) -> bool {
    ct_eq_bytes(a.as_bytes(), b.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_strings_match() {
        assert!(ct_eq_str("abc", "abc"));
        assert!(ct_eq_str("", ""));
    }

    #[test]
    fn differing_strings_dont_match() {
        assert!(!ct_eq_str("abc", "abd"));
        assert!(!ct_eq_str("abc", "ab"));
        assert!(!ct_eq_str("ab", "abc"));
    }
}
