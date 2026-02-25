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

    // Additional comprehensive tests

    #[test]
    fn test_simple_patterns_pass_through() {
        // Simple patterns should pass through unchanged
        assert_eq!(convert_ere_to_pcre_pattern("foo"), "foo");
        assert_eq!(convert_ere_to_pcre_pattern("bar123"), "bar123");
        assert_eq!(convert_ere_to_pcre_pattern("test_pattern"), "test_pattern");
        assert_eq!(convert_ere_to_pcre_pattern(""), "");
    }

    #[test]
    fn test_anchors_in_patterns() {
        // Anchors are the same in ERE and PCRE
        assert_eq!(convert_ere_to_pcre_pattern("^foo"), "^foo");
        assert_eq!(convert_ere_to_pcre_pattern("bar$"), "bar$");
        assert_eq!(convert_ere_to_pcre_pattern("^start$"), "^start$");
    }

    #[test]
    fn test_character_classes_in_patterns() {
        // Character classes are the same in ERE and PCRE
        assert_eq!(convert_ere_to_pcre_pattern("[a-z]"), "[a-z]");
        assert_eq!(convert_ere_to_pcre_pattern("[A-Z0-9]"), "[A-Z0-9]");
        assert_eq!(convert_ere_to_pcre_pattern("[^abc]"), "[^abc]");
        assert_eq!(convert_ere_to_pcre_pattern("[[:alpha:]]"), "[[:alpha:]]");
    }

    #[test]
    fn test_wildcard_in_patterns() {
        // Wildcard is the same in ERE and PCRE
        assert_eq!(convert_ere_to_pcre_pattern("f.o"), "f.o");
        assert_eq!(convert_ere_to_pcre_pattern(".*"), ".*");
        assert_eq!(convert_ere_to_pcre_pattern(".+"), ".+");
    }

    #[test]
    fn test_quantifiers_in_patterns() {
        // All ERE quantifiers are PCRE-compatible
        assert_eq!(convert_ere_to_pcre_pattern("foo+"), "foo+");
        assert_eq!(convert_ere_to_pcre_pattern("foo?"), "foo?");
        assert_eq!(convert_ere_to_pcre_pattern("foo*"), "foo*");
        assert_eq!(convert_ere_to_pcre_pattern("foo{3}"), "foo{3}");
        assert_eq!(convert_ere_to_pcre_pattern("foo{3,5}"), "foo{3,5}");
        assert_eq!(convert_ere_to_pcre_pattern("foo{3,}"), "foo{3,}");
        assert_eq!(convert_ere_to_pcre_pattern("foo{,5}"), "foo{,5}");
    }

    #[test]
    fn test_alternation_in_patterns() {
        // Alternation is the same in ERE and PCRE
        assert_eq!(convert_ere_to_pcre_pattern("foo|bar"), "foo|bar");
        assert_eq!(convert_ere_to_pcre_pattern("(foo|bar|baz)"), "(foo|bar|baz)");
        assert_eq!(convert_ere_to_pcre_pattern("a|b|c"), "a|b|c");
    }

    #[test]
    fn test_grouping_in_patterns() {
        // Grouping is the same in ERE and PCRE
        assert_eq!(convert_ere_to_pcre_pattern("(foo)"), "(foo)");
        assert_eq!(convert_ere_to_pcre_pattern("(foo)+(bar)?"), "(foo)+(bar)?");
        assert_eq!(convert_ere_to_pcre_pattern("((foo|bar)baz)"), "((foo|bar)baz)");
    }

    #[test]
    fn test_complex_ere_patterns() {
        // Complex patterns combining multiple ERE features
        assert_eq!(convert_ere_to_pcre_pattern(r#"(foo|bar)+[0-9]{3}"#), r#"(foo|bar)+[0-9]{3}"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"^([a-z]+)([0-9]+)$"#), r#"^([a-z]+)([0-9]+)$"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"(?:foo|bar)"#), r#"(?:foo|bar)"#);  // Non-capturing group
        assert_eq!(convert_ere_to_pcre_pattern(r#"a{1,3}.b{2,}"#), r#"a{1,3}.b{2,}"#);
    }

    #[test]
    fn test_escape_sequences_in_patterns() {
        // Escape sequences pass through
        assert_eq!(convert_ere_to_pcre_pattern(r#"\t"#), r#"\t"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"\n"#), r#"\n"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"\s"#), r#"\s"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"\d"#), r#"\d"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"\w"#), r#"\w"#);
    }

    #[test]
    fn test_all_backreferences_single() {
        // Test all single digit backreferences
        assert_eq!(convert_ere_backreferences(r#"\1"#), "$1");
        assert_eq!(convert_ere_backreferences(r#"\2"#), "$2");
        assert_eq!(convert_ere_backreferences(r#"\3"#), "$3");
        assert_eq!(convert_ere_backreferences(r#"\4"#), "$4");
        assert_eq!(convert_ere_backreferences(r#"\5"#), "$5");
        assert_eq!(convert_ere_backreferences(r#"\6"#), "$6");
        assert_eq!(convert_ere_backreferences(r#"\7"#), "$7");
        assert_eq!(convert_ere_backreferences(r#"\8"#), "$8");
        assert_eq!(convert_ere_backreferences(r#"\9"#), "$9");
    }

    #[test]
    fn test_multiple_backreferences_various() {
        // Multiple backreferences in various combinations
        assert_eq!(convert_ere_backreferences(r#"\1\2\3"#), "$1$2$3");
        assert_eq!(convert_ere_backreferences(r#"\3\2\1"#), "$3$2$1");
        assert_eq!(convert_ere_backreferences(r#"\9\8\7\6\5\4\3\2\1"#), "$9$8$7$6$5$4$3$2$1");
        assert_eq!(convert_ere_backreferences(r#"\1\1\1"#), "$1$1$1");
    }

    #[test]
    fn test_backreferences_with_text() {
        // Backreferences interspersed with text
        assert_eq!(convert_ere_backreferences(r#"foo\1bar"#), "foo$1bar");
        assert_eq!(convert_ere_backreferences(r#"start\1middle\2end"#), "start$1middle$2end");
        assert_eq!(convert_ere_backreferences(r#"Result: \1, \2, \3"#), "Result: $1, $2, $3");
        assert_eq!(convert_ere_backreferences(r#"prefix_\1_suffix"#), "prefix_$1_suffix");
    }

    #[test]
    fn test_match_reference_various() {
        // Match reference \& in various contexts
        assert_eq!(convert_ere_backreferences(r#"\&"#), "$&");
        assert_eq!(convert_ere_backreferences(r#"foo\&bar"#), "foo$&bar");
        assert_eq!(convert_ere_backreferences(r#"\&\&"#), "$&$&");
        assert_eq!(convert_ere_backreferences(r#"start:\&:end"#), "start:$&:end");
        assert_eq!(convert_ere_backreferences(r#"\1\&\2"#), "$1$&$2");
    }

    #[test]
    fn test_mixed_backreferences_pcre_format() {
        // Mixed ERE backreferences and PCRE format
        assert_eq!(convert_ere_backreferences(r#"foo\1bar$2"#), "foo$1bar$2");
        assert_eq!(convert_ere_backreferences(r#"$1\2$3\4"#), "$1$2$3$4");
        assert_eq!(convert_ere_backreferences(r#"\1$1\2$2"#), "$1$1$2$2");
    }

    #[test]
    fn test_backslash_sequences_replacement() {
        // Various backslash sequences in replacements
        assert_eq!(convert_ere_backreferences(r#"\\"#), "\\");
        assert_eq!(convert_ere_backreferences(r#"foo\\bar"#), "foo\\bar");
        assert_eq!(convert_ere_backreferences(r#"\n"#), "\\n");
        assert_eq!(convert_ere_backreferences(r#"\t"#), "\\t");
        assert_eq!(convert_ere_backreferences(r#"\\\n"#), "\\\\n");  // \\n → \n
        assert_eq!(convert_ere_backreferences(r#"\\\\\1"#), "\\\\$1");  // \\\\1 → \\$1
    }

    #[test]
    fn test_empty_and_simple_replacements() {
        // Edge cases for replacements
        assert_eq!(convert_ere_backreferences(""), "");
        assert_eq!(convert_ere_backreferences("text"), "text");
        assert_eq!(convert_ere_backreferences("123"), "123");
        assert_eq!(convert_ere_backreferences("!@#"), "!@#");
    }

    #[test]
    fn test_trailing_backslash_replacement() {
        // Trailing backslash in replacement
        assert_eq!(convert_ere_backreferences(r#"foo\"#), r#"foo\"#);
        assert_eq!(convert_ere_backreferences(r#"\"#), r#"\"#);
        assert_eq!(convert_ere_backreferences(r#"\1\"#), r#"$1\"#);
        assert_eq!(convert_ere_backreferences(r#"\&\"#), r#"$&\"#);
    }

    #[test]
    fn test_pcre_format_preserved() {
        // Already PCRE format should pass through
        assert_eq!(convert_ere_backreferences("$1"), "$1");
        assert_eq!(convert_ere_backreferences("$1$2$3"), "$1$2$3");
        assert_eq!(convert_ere_backreferences("$&"), "$&");
        assert_eq!(convert_ere_backreferences("foo$1bar$2"), "foo$1bar$2");
    }

    #[test]
    fn test_complex_replacement_patterns() {
        // Complex real-world replacement patterns
        assert_eq!(convert_ere_backreferences(r#"\1:\2"#), "$1:$2");
        assert_eq!(convert_ere_backreferences(r#"[\1] [\2]"#), "[$1] [$2]");
        assert_eq!(convert_ere_backreferences(r#""\1" -> "\2""#), r#""$1" -> "$2""#);
        assert_eq!(convert_ere_backreferences(r#"function(\1, \2)"#), "function($1, $2)");
    }

    #[test]
    fn test_unicode_patterns() {
        // Unicode characters should pass through
        assert_eq!(convert_ere_to_pcre_pattern("föö"), "föö");
        assert_eq!(convert_ere_to_pcre_pattern("(日本語)+"), "(日本語)+");
        assert_eq!(convert_ere_to_pcre_pattern("test_测试"), "test_测试");

        // Unicode in replacements
        assert_eq!(convert_ere_backreferences(r#"résumé\1"#), "résumé$1");
        assert_eq!(convert_ere_backreferences(r#"\1日本語\2"#), "$1日本語$2");
    }

    #[test]
    fn test_special_regex_characters() {
        // Special regex characters in patterns
        assert_eq!(convert_ere_to_pcre_pattern(r#"\."#), r#"\."#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"\^"#), r#"\^"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"\$"#), r#"\$"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"\["#), r#"\["#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"\]"#), r#"\]"#);
    }

    #[test]
    fn test_lookahead_lookbehind_patterns() {
        // PCRE-specific constructs should pass through
        assert_eq!(convert_ere_to_pcre_pattern(r#"foo(?=bar)"#), r#"foo(?=bar)"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"foo(?!bar)"#), r#"foo(?!bar)"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"(?<=foo)bar"#), r#"(?<=foo)bar"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"(?<!foo)bar"#), r#"(?<!foo)bar"#);
    }

    #[test]
    fn test_non_capturing_groups() {
        // Non-capturing and other special groups
        assert_eq!(convert_ere_to_pcre_pattern(r#"(?:foo|bar)"#), r#"(?:foo|bar)"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"(?P<name>foo)"#), r#"(?P<name>foo)"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"(?<name>foo)"#), r#"(?<name>foo)"#);
    }

    #[test]
    fn test_flags_and_modifiers() {
        // Pattern flags and modifiers should pass through
        assert_eq!(convert_ere_to_pcre_pattern(r#"(?i)foo"#), r#"(?i)foo"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"(?s).*"#), r#"(?s).*"#);
        assert_eq!(convert_ere_to_pcre_pattern(r#"(?m)^foo"#), r#"(?m)^foo"#);
    }
}
