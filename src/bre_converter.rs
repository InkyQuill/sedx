//! BRE to PCRE Converter
//!
//! This module provides automatic conversion from Basic Regular Expressions (BRE)
//! to Perl-Compatible Regular Expressions (PCRE), providing GNU sed compatibility.

/// Convert Basic Regular Expression (BRE) to Perl-Compatible Regular Expression (PCRE)
///
/// # Conversion Rules
///
/// - `\(` → `(` - Remove escape from opening parenthesis
/// - `\)` → `)` - Remove escape from closing parenthesis
/// - `\{` → `{` - Remove escape from opening brace
/// - `\}` → `}` - Remove escape from closing brace
/// - `\+` → `+` - Remove escape from plus quantifier
/// - `\?` → `?` - Remove escape from question mark
/// - `\|` → `|` - Remove escape from alternation
/// - `\1`..\`\9` → `$1`..`$9` - Convert backreferences to Rust regex style
/// - `\&` → `$&` - Convert match backreference
/// - `\\` → `\` - Convert double backslash to single
pub fn convert_bre_to_pcre(pattern: &str) -> String {
    let mut result = String::new();
    let mut chars = pattern.chars().peekable();
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            match c {
                '(' | ')' | '{' | '}' => {
                    // BRE escaped meta-char → PCRE meta-char
                    result.push(c);
                }
                '+' | '?' | '|' => {
                    // BRE escaped quantifiers/alternation → PCRE
                    result.push(c);
                }
                '\\' => {
                    // Double backslash → single backslash
                    result.push('\\');
                }
                '1'..='9' => {
                    // Backreference: \1 → $1
                    result.push('$');
                    result.push(c);
                }
                '&' => {
                    // Match backreference: \& → $&
                    result.push('$');
                    result.push('&');
                }
                'n' if chars.peek().is_none() => {
                    // \ n at end is literal newline, not escape
                    result.push('\\');
                    result.push(c);
                }
                _ => {
                    // Unknown escape sequence, keep as-is
                    result.push('\\');
                    result.push(c);
                }
            }
            escape_next = false;
        } else if c == '\\' {
            escape_next = true;
        } else {
            result.push(c);
        }
    }

    // Handle trailing backslash
    if escape_next {
        result.push('\\');
    }

    result
}

/// Detect if a pattern is in Basic Regular Expression (BRE) format
///
/// # Detection Rules
///
/// A pattern is considered BRE if it contains:
/// - Escaped parentheses: `\(`, `\)`
/// - Escaped braces: `\{`, `\}`
/// - Escaped quantifiers: `\+`, `\?`
/// - Escaped alternation: `\|`
#[allow(dead_code)]  // Kept for potential future use
pub fn is_bre_pattern(pattern: &str) -> bool {
    pattern.contains("\\(") || pattern.contains("\\)") ||
    pattern.contains("\\{") || pattern.contains("\\}") ||
    pattern.contains("\\+") || pattern.contains("\\?") ||
    pattern.contains("\\|") ||
    // Check for backreferences \1-\9
    (pattern.contains("\\1") || pattern.contains("\\2") ||
     pattern.contains("\\3") || pattern.contains("\\4") ||
     pattern.contains("\\5") || pattern.contains("\\6") ||
     pattern.contains("\\7") || pattern.contains("\\8") ||
     pattern.contains("\\9"))
}

/// Convert sed-style backreferences in replacement string to Rust regex style
///
/// # Conversion Rules
///
/// - `\1`..`\9` → `$1`..`$9` - Backreference conversion
/// - `\&` → `$&` - Match reference
/// - `\\` → `\` - Escape backslash
///
/// This is used separately from pattern conversion because replacement strings
/// have slightly different rules than patterns.
pub fn convert_sed_backreferences(replacement: &str) -> String {
    let mut result = String::new();
    let chars = replacement.chars().peekable();
    let mut escape_next = false;

    for c in chars {
        if escape_next {
            match c {
                '1'..='9' => {
                    // Backreference: \1 → $1
                    result.push('$');
                    result.push(c);
                }
                '&' => {
                    // Match backreference: \& → $&
                    result.push('$');
                    result.push('&');
                }
                '\\' => {
                    // Double backslash → single
                    result.push('\\');
                }
                'n' => {
                    // Newline escape
                    result.push('\\');
                    result.push('n');
                }
                _ => {
                    // Unknown escape, keep literal
                    result.push('\\');
                    result.push(c);
                }
            }
            escape_next = false;
        } else if c == '\\' {
            escape_next = true;
        } else {
            result.push(c);
        }
    }

    // Handle trailing backslash
    if escape_next {
        result.push('\\');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_parentheses() {
        assert_eq!(convert_bre_to_pcre(r#"\(foo\)"#), "(foo)");
        assert_eq!(convert_bre_to_pcre(r#"\(a\)\(b\)"#), "(a)(b)");
        assert_eq!(convert_bre_to_pcre(r#"foo\(bar\)"#), "foo(bar)");
    }

    #[test]
    fn test_convert_braces() {
        assert_eq!(convert_bre_to_pcre(r#"foo\{3\}"#), "foo{3}");
        assert_eq!(convert_bre_to_pcre(r#"\{3,5\}"#), "{3,5}");
    }

    #[test]
    fn test_convert_quantifiers() {
        assert_eq!(convert_bre_to_pcre(r#"foo\+"#), "foo+");
        assert_eq!(convert_bre_to_pcre(r#"foo\?"#), "foo?");
        assert_eq!(convert_bre_to_pcre(r#"foo\*"#), r#"foo\*"#);  // \* is literal asterisk in both BRE and PCRE
    }

    #[test]
    fn test_convert_alternation() {
        assert_eq!(convert_bre_to_pcre(r#"foo\|bar"#), "foo|bar");
    }

    #[test]
    fn test_convert_backreferences() {
        assert_eq!(convert_bre_to_pcre(r#"\1"#), "$1");
        assert_eq!(convert_bre_to_pcre(r#"\2\1"#), "$2$1");
        assert_eq!(convert_bre_to_pcre(r#"\&"#), "$&");
    }

    #[test]
    fn test_convert_backslash() {
        assert_eq!(convert_bre_to_pcre(r#"\\"#), "\\");
        assert_eq!(convert_bre_to_pcre(r#"foo\\"#), "foo\\");
        assert_eq!(convert_bre_to_pcre(r#"\\\\)"#), r#"\\)"#);  // \\ → \
    }

    #[test]
    fn test_no_conversion_needed() {
        assert_eq!(convert_bre_to_pcre(r#"(foo)"#), "(foo)");
        assert_eq!(convert_bre_to_pcre(r#"foo+"#), "foo+");
        assert_eq!(convert_bre_to_pcre(r#"foo|bar"#), "foo|bar");
    }

    #[test]
    fn test_is_bre_pattern() {
        assert!(is_bre_pattern(r#"\(foo\)"#));
        assert!(is_bre_pattern(r#"foo\+"#));
        assert!(is_bre_pattern(r#"foo\{3\}"#));
        assert!(is_bre_pattern(r#"foo\|bar"#));
        assert!(is_bre_pattern(r#"\1"#));

        assert!(!is_bre_pattern(r#"(foo)"#));
        assert!(!is_bre_pattern(r#"foo+"#));
        assert!(!is_bre_pattern(r#"foo|bar"#));
    }

    #[test]
    fn test_convert_sed_backreferences() {
        assert_eq!(convert_sed_backreferences(r#"\1"#), "$1");
        assert_eq!(convert_sed_backreferences(r#"\2\1"#), "$2$1");
        assert_eq!(convert_sed_backreferences(r#"\&"#), "$&");
        assert_eq!(convert_sed_backreferences(r#"\\"#), "\\");
        assert_eq!(convert_sed_backreferences(r#"\n"#), "\\n");
        assert_eq!(convert_sed_backreferences(r#"foo\1bar"#), "foo$1bar");
    }

    #[test]
    fn test_no_backreference_conversion() {
        assert_eq!(convert_sed_backreferences(r#"foo"#), "foo");
        assert_eq!(convert_sed_backreferences(r#"foo bar"#), "foo bar");
    }

    #[test]
    fn test_complex_bre_pattern() {
        // BRE: \(foo\)\(bar\) \2\1
        // PCRE: (foo)(bar) $2$1
        let bre_pattern = r#"\(foo\)\(bar\) \2\1"#;
        let pcre_pattern = convert_bre_to_pcre(bre_pattern);
        assert_eq!(pcre_pattern, r#"(foo)(bar) $2$1"#);
    }

    #[test]
    fn test_pcre_pattern_unchanged() {
        // PCRE patterns should pass through unchanged
        assert_eq!(convert_bre_to_pcre(r#"(foo|bar)+"#), r#"(foo|bar)+"#);
        assert_eq!(convert_bre_to_pcre(r#"foo{3,5}"#), r#"foo{3,5}"#);
    }

    // Additional comprehensive tests

    #[test]
    fn test_simple_patterns() {
        // Simple patterns should pass through unchanged
        assert_eq!(convert_bre_to_pcre("foo"), "foo");
        assert_eq!(convert_bre_to_pcre("bar123"), "bar123");
        assert_eq!(convert_bre_to_pcre("test_pattern"), "test_pattern");
        assert_eq!(convert_bre_to_pcre(""), "");
    }

    #[test]
    fn test_anchors() {
        // Anchors are the same in BRE and PCRE
        assert_eq!(convert_bre_to_pcre("^foo"), "^foo");
        assert_eq!(convert_bre_to_pcre("bar$"), "bar$");
        assert_eq!(convert_bre_to_pcre("^start$"), "^start$");
        assert_eq!(convert_bre_to_pcre(r#"\^foo"#), r#"\^foo"#);  // Escaped anchor
    }

    #[test]
    fn test_character_classes() {
        // Character classes are the same in BRE and PCRE
        assert_eq!(convert_bre_to_pcre("[a-z]"), "[a-z]");
        assert_eq!(convert_bre_to_pcre("[A-Z0-9]"), "[A-Z0-9]");
        assert_eq!(convert_bre_to_pcre("[^abc]"), "[^abc]");
        assert_eq!(convert_bre_to_pcre("[[:alpha:]]"), "[[:alpha:]]");
        assert_eq!(convert_bre_to_pcre(r#"[a\]z]"#), r#"[a\]z]"#);  // Escaped ] in char class
    }

    #[test]
    fn test_escaped_sequences() {
        // Various escape sequences
        assert_eq!(convert_bre_to_pcre(r#"\t"#), r#"\t"#);  // Unknown escape, keep as-is
        assert_eq!(convert_bre_to_pcre(r#"\n"#), r#"\n"#);  // Unknown escape, keep as-is
        assert_eq!(convert_bre_to_pcre(r#"\s"#), r#"\s"#);  // Unknown escape, keep as-is
        assert_eq!(convert_bre_to_pcre(r#"\w"#), r#"\w"#);  // Unknown escape, keep as-is
    }

    #[test]
    fn test_wildcard() {
        // Wildcard is the same in BRE and PCRE
        assert_eq!(convert_bre_to_pcre("f.o"), "f.o");
        assert_eq!(convert_bre_to_pcre(".*"), ".*");
        assert_eq!(convert_bre_to_pcre(r#"\.\*"#), r#"\.\*"#);  // Escaped dot and star
    }

    #[test]
    fn test_complex_nested_patterns() {
        // Nested groups
        assert_eq!(convert_bre_to_pcre(r#"\(foo\(bar\)\)"#), "(foo(bar))");
        assert_eq!(convert_bre_to_pcre(r#"\(a\|\(b\|c\)\)"#), "(a|(b|c))");

        // Multiple groups with quantifiers
        assert_eq!(convert_bre_to_pcre(r#"\(foo\)\+"#), "(foo)+");
        assert_eq!(convert_bre_to_pcre(r#"\(bar\)\{2,5\}"#), "(bar){2,5}");

        // Complex BRE pattern: \(foo\)\{3\} \(bar\|baz\)
        assert_eq!(convert_bre_to_pcre(r#"\(foo\)\{3\} \(bar\|baz\)"#), r#"(foo){3} (bar|baz)"#);
    }

    #[test]
    fn test_multiple_backreferences_in_replacement() {
        // Multiple backreferences
        assert_eq!(convert_sed_backreferences(r#"\1\2\3"#), "$1$2$3");
        assert_eq!(convert_sed_backreferences(r#"\9\8\7\6\5\4\3\2\1"#), "$9$8$7$6$5$4$3$2$1");

        // Backreferences with text
        assert_eq!(convert_sed_backreferences(r#"start\1middle\2end"#), "start$1middle$2end");

        // Multiple consecutive same backreference
        assert_eq!(convert_sed_backreferences(r#"\1\1\1"#), "$1$1$1");
    }

    #[test]
    fn test_match_reference_in_replacement() {
        // Match reference \&
        assert_eq!(convert_sed_backreferences(r#"\&"#), "$&");
        assert_eq!(convert_sed_backreferences(r#"foo\&bar"#), "foo$&bar");
        assert_eq!(convert_sed_backreferences(r#"\&\&"#), "$&$&");
        assert_eq!(convert_sed_backreferences(r#"\1\&\2"#), "$1$&$2");
    }

    #[test]
    fn test_mixed_backreferences_and_text() {
        // Complex replacement patterns
        assert_eq!(convert_sed_backreferences(r#"prefix_\1_suffix"#), "prefix_$1_suffix");
        assert_eq!(convert_sed_backreferences(r#"Result: \1, \2"#), "Result: $1, $2");
        assert_eq!(convert_sed_backreferences(r#"\1:\&:\2"#), "$1:$&:$2");
    }

    #[test]
    fn test_no_backreferences_in_text() {
        // Regular text without backreferences
        assert_eq!(convert_sed_backreferences("simple text"), "simple text");
        assert_eq!(convert_sed_backreferences("1234567890"), "1234567890");
        assert_eq!(convert_sed_backreferences("!@#$%^&*()"), "!@#$%^&*()");
        assert_eq!(convert_sed_backreferences(""), "");
    }

    #[test]
    fn test_trailing_backslash_pattern() {
        // Trailing backslash should be preserved
        assert_eq!(convert_bre_to_pcre(r#"foo\"#), r#"foo\"#);
        assert_eq!(convert_bre_to_pcre(r#"\("#), r#"("#);      // Just opening paren
        assert_eq!(convert_bre_to_pcre(r#"\"#), r#"\"#);       // Just backslash
    }

    #[test]
    fn test_trailing_backslash_replacement() {
        // Trailing backslash in replacement
        assert_eq!(convert_sed_backreferences(r#"foo\"#), r#"foo\"#);
        assert_eq!(convert_sed_backreferences(r#"\"#), r#"\"#);
        assert_eq!(convert_sed_backreferences(r#"\1\"#), r#"$1\"#);
    }

    #[test]
    fn test_double_backslash_conversion() {
        // Double backslash to single
        assert_eq!(convert_bre_to_pcre(r#"\\"#), "\\");
        assert_eq!(convert_bre_to_pcre(r#"foo\\bar"#), "foo\\bar");
        assert_eq!(convert_bre_to_pcre(r#"\\("#), r#"\("#);  // \\ then \( → \ then (

        // Triple and quadruple backslash
        assert_eq!(convert_bre_to_pcre(r#"\\\"#), r#"\\"#);   // \\\" → \\
        assert_eq!(convert_bre_to_pcre(r#"\\\\"#), r#"\\"#);  // \\\\ → \\
    }

    #[test]
    fn test_double_backslash_replacement() {
        // Double backslash to single in replacement
        assert_eq!(convert_sed_backreferences(r#"\\"#), "\\");
        assert_eq!(convert_sed_backreferences(r#"foo\\bar"#), "foo\\bar");
        assert_eq!(convert_sed_backreferences(r#"\1\\n"#), "$1\\n");
    }

    #[test]
    fn test_alternation_patterns() {
        // Various alternation patterns
        assert_eq!(convert_bre_to_pcre(r#"foo\|bar"#), "foo|bar");
        // Note: \baz gets \b converted (unknown escape) and then literal baz
        assert_eq!(convert_bre_to_pcre(r#"\(foo\|bar\|\baz\)"#), r#"(foo|bar|\baz)"#);
        assert_eq!(convert_bre_to_pcre(r#"a\|b\|c"#), "a|b|c");
        // Clean alternation with escaped bars only
        assert_eq!(convert_bre_to_pcre(r#"\(foo\|bar\)\+"#), "(foo|bar)+");
    }

    #[test]
    fn test_repetition_quantifiers() {
        // All BRE quantifiers
        assert_eq!(convert_bre_to_pcre(r#"foo\+"#), "foo+");
        assert_eq!(convert_bre_to_pcre(r#"foo\?"#), "foo?");
        assert_eq!(convert_bre_to_pcre(r#"foo\{3\}"#), "foo{3}");
        assert_eq!(convert_bre_to_pcre(r#"foo\{3,5\}"#), "foo{3,5}");
        assert_eq!(convert_bre_to_pcre(r#"foo\{3,\}"#), "foo{3,}");
        assert_eq!(convert_bre_to_pcre(r#"foo\{,5\}"#), "foo{,5}");

        // Escaped quantifiers remain escaped (literal)
        assert_eq!(convert_bre_to_pcre(r#"foo\*"#), r#"foo\*"#);
    }

    #[test]
    fn test_grouped_commands() {
        // BRE patterns commonly used with grouped commands
        assert_eq!(convert_bre_to_pcre(r#"/foo\|bar/"#), r#"/foo|bar/"#);
        // Backreferences in patterns are converted to $1 for SedX internal representation
        assert_eq!(convert_bre_to_pcre(r#"\(test\).*\1"#), r#"(test).*$1"#);
    }

    #[test]
    fn test_digit_backreferences_in_pattern() {
        // In patterns, \1-\9 convert to $1-$9
        // Note: This is for SedX's internal representation
        assert_eq!(convert_bre_to_pcre(r#"\1"#), "$1");
        assert_eq!(convert_bre_to_pcre(r#"\2"#), "$2");
        assert_eq!(convert_bre_to_pcre(r#"\9"#), "$9");

        // Digits following backslash that aren't backreferences
        assert_eq!(convert_bre_to_pcre(r#"\0"#), r#"\0"#);  // \0 is not a backreference
    }

    #[test]
    fn test_special_characters_preserved() {
        // Characters that should remain unchanged
        assert_eq!(convert_bre_to_pcre(r#"."#), ".");
        assert_eq!(convert_bre_to_pcre(r#"*"#), "*");
        assert_eq!(convert_bre_to_pcre(r#"^"#), "^");
        assert_eq!(convert_bre_to_pcre(r#"$"#), "$");
        assert_eq!(convert_bre_to_pcre(r#"["#), "[");
        assert_eq!(convert_bre_to_pcre(r#"]"#), "]");
    }

    #[test]
    fn test_newline_escape_at_end() {
        // \n at end of pattern is literal newline escape
        assert_eq!(convert_bre_to_pcre(r#"foo\n"#), r#"foo\n"#);
        assert_eq!(convert_bre_to_pcre(r#"\n"#), r#"\n"#);
    }

    #[test]
    fn test_empty_groups() {
        // Empty or simple groups
        assert_eq!(convert_bre_to_pcre(r#"\(\)"#), "()");
        assert_eq!(convert_bre_to_pcre(r#"\(\+\)"#), "(+)");
    }

    #[test]
    fn test_unicode_patterns() {
        // Unicode characters should pass through
        assert_eq!(convert_bre_to_pcre("föö"), "föö");
        assert_eq!(convert_bre_to_pcre(r#"\(日本語\)"#), "(日本語)");
        assert_eq!(convert_bre_to_pcre("test_测试"), "test_测试");
    }
}
