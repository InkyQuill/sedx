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
///
/// # Examples
///
/// ```
/// assert_eq!(convert_bre_to_pcre(r#"\(foo\)"#), "(foo)");
/// assert_eq!(convert_bre_to_pcre(r#"\(a\)\(b\)"#), "(a)(b)");
/// assert_eq!(convert_bre_to_pcre(r#"foo\+"#), "foo+");
/// assert_eq!(convert_bre_to_pcre(r#"\1"#), "$1");
/// ```
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
///
/// # Examples
///
/// ```
/// assert!(is_bre_pattern(r#"\(foo\)"#));
/// assert!(is_bre_pattern(r#"foo\+"#));
/// assert!(!is_bre_pattern(r#"(foo)"#));
/// assert!(!is_bre_pattern(r#"foo+"#));
/// ```
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
    let mut chars = replacement.chars().peekable();
    let mut escape_next = false;

    while let Some(c) = chars.next() {
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
}
