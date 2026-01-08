//! ERE to PCRE Converter
//!
//! This module provides conversion from Extended Regular Expressions (ERE)
//! to Perl-Compatible Regular Expressions (PCRE).
//!
//! ERE is already very close to PCRE. The main difference is backreferences:
//! - ERE/sed -E uses \1, \2, \3... in replacements
//! - PCRE/Rust regex uses $1, $2, $3... in replacements
//!
//! For patterns, ERE syntax is already PCRE-compatible.

/// Convert Extended Regular Expression (ERE) to Perl-Compatible Regular Expression (PCRE)
///
/// # Conversion Rules
///
/// For **patterns**, ERE is already PCRE-compatible, so this is mostly a pass-through:
/// - `(`, `)`, `{`, `}`, `+`, `?`, `|` are all valid in both ERE and PCRE
/// - No conversion needed for pattern syntax
///
/// For **replacements**, backreferences need conversion:
/// - `\1`..`\9` → `$1`..`$9` - Backreference conversion
///
/// # Examples
///
/// ```
/// // Patterns pass through unchanged
/// assert_eq!(convert_ere_to_pcre_pattern(r#"(foo|bar)+"#), r#"(foo|bar)+"#);
/// assert_eq!(convert_ere_to_pcre_pattern(r#"foo{3,5}"#), r#"foo{3,5}"#);
///
/// // Replacements convert backreferences
/// assert_eq!(convert_ere_to_pcre_replacement(r#"\1\2"#), "$1$2");
/// ```
pub fn convert_ere_to_pcre_pattern(pattern: &str) -> String {
    // ERE patterns are already PCRE-compatible
    // Just pass through unchanged
    pattern.to_string()
}

/// Convert ERE-style backreferences in replacement string to Rust regex style
///
/// # Conversion Rules
///
/// - `\1`..`\9` → `$1`..`$9` - Backreference conversion
/// - `\&` → `$&` - Match reference
/// - `\\` → `\` - Escape backslash
///
/// This is identical to BRE backreference conversion since both BRE and ERE
/// use the same backreference syntax in replacements.
///
/// # Examples
///
/// ```
/// assert_eq!(convert_ere_backreferences(r#"\1"#), "$1");
/// assert_eq!(convert_ere_backreferences(r#"\2\1"#), "$2$1");
/// assert_eq!(convert_ere_backreferences(r#"foo\1bar"#), "foo$1bar");
/// ```
pub fn convert_ere_backreferences(replacement: &str) -> String {
    // Reuse the BRE backreference converter - identical logic
    crate::bre_converter::convert_sed_backreferences(replacement)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_pass_through() {
        // ERE patterns should pass through unchanged
        assert_eq!(convert_ere_to_pcre_pattern(r#"(foo|bar)+"#), r#"(foo|bar)+"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"foo{3,5}"#), r#"foo{3,5}"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"foo+"#), r#"foo+"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"foo?"#), r#"foo?"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"foo|bar"#), r#"foo|bar"#);
    }

    #[test]
    fn test_convert_ere_backreferences() {
        assert_eq!(convert_ere_backreferences(r#"\1"#), "$1");
        assert_eq!(convert_ere_backreferences(r#"\2\1"#), "$2$1");
        assert_eq!(convert_ere_backreferences(r#"\&"#), "$&");
        assert_eq!(convert_ere_backreferences(r#"\\"#), "\\");
        assert_eq!(convert_ere_backreferences(r#"foo\1bar"#), "foo$1bar");
    }

    #[test]
    fn test_no_backreference_conversion() {
        assert_eq!(convert_ere_backreferences(r#"foo"#), "foo");
        assert_eq!(convert_ere_backreferences(r#"foo bar"#), "foo bar");
        assert_eq!(convert_ere_backreferences(r#"$1$2"#), "$1$2");  // Already PCRE format
    }

    #[test]
    fn test_complex_ere_replacement() {
        // ERE: \1\2\3
        // PCRE: $1$2$3
        let ere_replacement = r#"\1\2\3"#;
        let pcre_replacement = convert_ere_backreferences(ere_replacement);
        assert_eq!(pcre_replacement, r#"$1$2$3"#);
    }

    #[test]
    fn test_mixed_replacement() {
        // Mixed ERE and PCRE backreferences
        assert_eq!(convert_ere_backreferences(r#"foo\1bar$2"#), "foo$1bar$2");
    }

    #[test]
    fn test_escape_sequences() {
        assert_eq!(convert_ere_backreferences(r#"\n"#), "\\n");
        assert_eq!(convert_ere_backreferences(r#"\t"#), "\\t");
        assert_eq!(convert_ere_backreferences(r#"\\\n"#), "\\\\n");  // \\n → \n (escaped backslash becomes single)
    }
}
