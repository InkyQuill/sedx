//! Regex Error Handling
//!
//! This module provides enhanced error messages for regex compilation failures,
//! helping users understand and fix their regex patterns.

use crate::cli::RegexFlavor;
use regex::Error as RegexError;

/// Enhanced regex error with helpful context
#[derive(Debug, Clone, PartialEq)]
pub struct EnhancedRegexError {
    /// The original pattern that failed to compile
    pub pattern: String,
    /// The regex flavor being used
    pub flavor: RegexFlavor,
    /// The type of error that occurred
    pub error_type: RegexErrorType,
    /// Suggested fix for the error
    pub suggestion: Option<String>,
}

/// Types of regex errors with specific diagnostic information
#[derive(Debug, Clone, PartialEq)]
pub enum RegexErrorType {
    /// Syntax error in the regex pattern
    Syntax {
        message: String,
        position: Option<usize>,
    },
    /// Invalid escape sequence
    InvalidEscape { sequence: String, position: usize },
    /// Unclosed group/bracket/brace
    UnclosedDelimiter {
        delimiter: char, // '(', '[', '{'
        position: usize,
    },
    /// Invalid quantifier (e.g., `**`, `*?+`, nothing to repeat)
    InvalidQuantifier {
        message: String,
        position: Option<usize>,
    },
    /// Lookbehind/lookahead not supported or invalid
    LookaroundError {
        message: String,
        position: Option<usize>,
    },
    /// Invalid backreference
    InvalidBackreference {
        message: String,
        position: Option<usize>,
    },
    /// Other regex error
    #[allow(dead_code)] // Fallback variant for future error types
    Other { message: String },
}

impl EnhancedRegexError {
    /// Create a regex error from a regex::Error with context
    pub fn from_regex_error(err: &RegexError, pattern: &str, flavor: RegexFlavor) -> Self {
        let error_msg = err.to_string();
        let error_type = Self::classify_error(&error_msg, pattern, flavor);

        let suggestion = Self::generate_suggestion(&error_type, pattern, flavor);

        EnhancedRegexError {
            pattern: pattern.to_string(),
            flavor,
            error_type,
            suggestion,
        }
    }

    /// Classify the regex error into a specific type
    fn classify_error(error_msg: &str, pattern: &str, flavor: RegexFlavor) -> RegexErrorType {
        let lower_msg = error_msg.to_lowercase();

        // Check for unclosed delimiters
        if lower_msg.contains("unclosed") || lower_msg.contains("unterminated") {
            if lower_msg.contains("parenthesis") || lower_msg.contains("group") {
                // Find the unclosed parenthesis
                if let Some(pos) = find_unclosed_delimiter(pattern, '(', ')') {
                    return RegexErrorType::UnclosedDelimiter {
                        delimiter: '(',
                        position: pos,
                    };
                }
            }
            if lower_msg.contains("bracket") || lower_msg.contains("character class") {
                if let Some(pos) = find_unclosed_delimiter(pattern, '[', ']') {
                    return RegexErrorType::UnclosedDelimiter {
                        delimiter: '[',
                        position: pos,
                    };
                }
            }
            if lower_msg.contains("brace") || lower_msg.contains("quantifier") {
                if let Some(pos) = find_unclosed_delimiter(pattern, '{', '}') {
                    return RegexErrorType::UnclosedDelimiter {
                        delimiter: '{',
                        position: pos,
                    };
                }
            }
        }

        // Check for invalid escape sequences
        if lower_msg.contains("escape") || lower_msg.contains("backslash") {
            if let Some(pos) = find_invalid_escape(pattern, flavor) {
                return RegexErrorType::InvalidEscape {
                    sequence: extract_escape_at(pattern, pos),
                    position: pos,
                };
            }
        }

        // Check for invalid quantifiers
        if lower_msg.contains("quantifier")
            || lower_msg.contains("repeat")
            || lower_msg.contains("repetition operator")
        {
            // Find the problematic quantifier
            if let Some(pos) = find_invalid_quantifier(pattern) {
                return RegexErrorType::InvalidQuantifier {
                    message: error_msg.to_string(),
                    position: Some(pos),
                };
            }
            return RegexErrorType::InvalidQuantifier {
                message: error_msg.to_string(),
                position: None,
            };
        }

        // Check for lookaround errors
        if lower_msg.contains("lookaround")
            || lower_msg.contains("lookahead")
            || lower_msg.contains("lookbehind")
        {
            return RegexErrorType::LookaroundError {
                message: error_msg.to_string(),
                position: None,
            };
        }

        // Check for backreference errors
        if lower_msg.contains("backreference") || lower_msg.contains("reference") {
            return RegexErrorType::InvalidBackreference {
                message: error_msg.to_string(),
                position: None,
            };
        }

        // Default to syntax error
        RegexErrorType::Syntax {
            message: error_msg.to_string(),
            position: None,
        }
    }

    /// Generate a helpful suggestion based on the error type and regex flavor
    fn generate_suggestion(
        error_type: &RegexErrorType,
        pattern: &str,
        flavor: RegexFlavor,
    ) -> Option<String> {
        match error_type {
            RegexErrorType::UnclosedDelimiter {
                delimiter,
                position,
            } => {
                let closer = match delimiter {
                    '(' => ')',
                    '[' => ']',
                    '{' => '}',
                    _ => return None,
                };
                Some(format!(
                    "Add a closing '{}' to match the opening '{}' at position {}. \
                    Pattern: \"{}{}\"",
                    closer, delimiter, position, pattern, closer
                ))
            }

            RegexErrorType::InvalidEscape {
                sequence,
                position: _,
            } => {
                match (sequence.as_str(), flavor) {
                    // BRE-specific issues
                    ("\\(", RegexFlavor::PCRE) => Some("In PCRE mode (default), parentheses are meta-characters. \
                            Use '(' directly, or use -B (BRE mode) if you meant '\\('. \
                            For a literal '(', use '\\('.".to_string()),
                    ("\\)", RegexFlavor::PCRE) => Some("In PCRE mode (default), ')' is a meta-character. \
                            Use ')' directly, or use -B (BRE mode) if you meant '\\)'. \
                            For a literal ')', use '\\)'.".to_string()),
                    ("\\{", RegexFlavor::PCRE) => Some("In PCRE mode (default), '{' starts a quantifier. \
                            Use '{' directly for quantifiers like 'a{3}', or use -B (BRE mode) if you meant '\\{'.".to_string()),
                    ("\\+", RegexFlavor::PCRE) => Some("In PCRE mode (default), '+' is a meta-character (one or more). \
                            Use '+' directly, or use -B (BRE mode) if you meant '\\+' as a quantifier. \
                            For a literal '+', use '\\+'.".to_string()),
                    ("\\?", RegexFlavor::PCRE) => Some("In PCRE mode (default), '?' is a meta-character (zero or one). \
                            Use '?' directly, or use -B (BRE mode) if you meant '\\?'. \
                            For a literal '?', use '\\?'.".to_string()),
                    ("\\|", RegexFlavor::PCRE) => Some("In PCRE mode (default), '|' is the alternation operator. \
                            Use '|' directly, or use -B (BRE mode) if you meant '\\|'. \
                            For a literal '|', use '\\|'.".to_string()),
                    // BRE-specific issues
                    ("(", RegexFlavor::BRE) => Some("In BRE mode (-B), parentheses must be escaped: '\\(' not '('. \
                            Use -E (ERE mode) or remove -B for PCRE mode to use '(' directly.".to_string()),
                    (")", RegexFlavor::BRE) => Some("In BRE mode (-B), parentheses must be escaped: '\\)' not ')'. \
                            Use -E (ERE mode) or remove -B for PCRE mode to use ')' directly.".to_string()),
                    ("{", RegexFlavor::BRE) => Some("In BRE mode (-B), braces must be escaped: '\\{' not '{'. \
                            Use -E (ERE mode) or remove -B for PCRE mode.".to_string()),
                    ("+", RegexFlavor::BRE) => Some("In BRE mode (-B), '+' is a literal character. \
                            Use '\\+' for the quantifier (one or more), or use -E for ERE mode.".to_string()),
                    ("?", RegexFlavor::BRE) => Some("In BRE mode (-B), '?' is a literal character. \
                            Use '\\?' for the quantifier (zero or one), or use -E for ERE mode.".to_string()),
                    ("|", RegexFlavor::BRE) => Some("In BRE mode (-B), '|' is a literal character. \
                            Use '\\|' for alternation, or use -E for ERE mode.".to_string()),
                    // Generic escape issues
                    (seq, _) => Some(format!(
                        "The escape sequence '{}' is not recognized. \
                            In PCRE mode, common escapes are: \\n, \\t, \\r, \\xHH, \\uHHHH, \\x{{HHHHHH}}. \
                            For a literal '{}', use '\\{}'.",
                        seq, seq, seq
                    )),
                }
            }

            RegexErrorType::InvalidQuantifier {
                message,
                position: _,
            } => {
                if message.to_lowercase().contains("nothing to repeat") {
                    Some("A quantifier (*, +, ?, {n}) has nothing to repeat. \
                        Place it after a character or group, e.g., 'a*', '(foo)+', 'bar{3}'.".to_string())
                } else if message.to_lowercase().contains("invalid range") {
                    Some("Quantifier ranges must be valid: '{n}', '{n,}', or '{n,m}' where n <= m. \
                        Example: 'a{2,5}' matches 2 to 5 'a's.".to_string())
                } else {
                    Some("Check your quantifier syntax: * (zero or more), + (one or more), ? (zero or one), {n,m} (n to m times).".to_string())
                }
            }

            RegexErrorType::LookaroundError { message: _, .. } => Some("Rust's regex crate (used by SedX) has limited lookaround support. \
                    Fixed-width lookbehinds are supported: (?<=pattern), (?<!pattern). \
                    Lookaheads are supported: (?=pattern), (?!pattern). \
                    Variable-width lookbehinds are NOT supported.".to_string()),

            RegexErrorType::InvalidBackreference { message: _, .. } => match flavor {
                RegexFlavor::BRE => Some("In BRE mode, backreferences use \\1, \\2, etc. in patterns. \
                            Make sure you have capturing groups \\(...\\) before referencing them. \
                            Note: SedX converts BRE to PCRE internally, so \\1 becomes $1.".to_string()),
                RegexFlavor::ERE => Some("In ERE mode, backreferences use \\1, \\2, etc. in replacement strings. \
                            In patterns, use $1, $2, etc. for backreferences. \
                            Make sure you have capturing groups before referencing them.".to_string()),
                RegexFlavor::PCRE => Some("In PCRE mode, backreferences use $1, $2, etc. in both patterns and replacements. \
                            Make sure you have capturing groups (...) before referencing them.".to_string()),
            },

            RegexErrorType::Syntax { message: _, .. } => {
                // Try to provide a more helpful message based on the pattern
                if pattern.contains('[') && !pattern.contains(']') {
                    Some("Unclosed character class '[...]'. Add a closing ']'.".to_string())
                } else if pattern.contains('(') && !pattern.contains(')') {
                    Some("Unclosed group '(...)'. Add a closing ')'.".to_string())
                } else if pattern.contains('*') && pattern.ends_with('*') {
                    Some("Quantifier '*' at the end of pattern has nothing to repeat.".to_string())
                } else {
                    Some("Check your regex syntax. Common issues: \
                        - Escape special characters: . + * ? ^ $ | ( ) [ ] { } \\ \
                        - Use proper quantifiers: a* a+ a? a{3} \
                        - Close all groups and character classes".to_string())
                }
            }

            RegexErrorType::Other { message } => Some(format!(
                "Error: {}. For help, visit: https://docs.rs/regex/latest/regex/",
                message
            )),
        }
    }

    /// Format the error as a user-friendly message
    pub fn display(&self) -> String {
        let flavor_name = match self.flavor {
            RegexFlavor::PCRE => "PCRE (default)",
            RegexFlavor::ERE => "ERE (extended regex, -E flag)",
            RegexFlavor::BRE => "BRE (basic regex, -B flag)",
        };

        let mut output = format!("Regex Error in {} mode\n", flavor_name);
        output.push_str(&format!("  Pattern: \"{}\"\n", self.pattern));

        match &self.error_type {
            RegexErrorType::Syntax {
                message,
                position: _,
            } => {
                output.push_str("  Type: Syntax error\n");
                output.push_str(&format!("  Details: {}\n", message));
            }
            RegexErrorType::InvalidEscape { sequence, position } => {
                output.push_str("  Type: Invalid escape sequence\n");
                output.push_str(&format!(
                    "  Sequence: '{}' at position {}\n",
                    sequence, position
                ));
            }
            RegexErrorType::UnclosedDelimiter {
                delimiter,
                position,
            } => {
                output.push_str("  Type: Unclosed delimiter\n");
                output.push_str(&format!(
                    "  Missing closing '{}' for opening '{}' at position {}\n",
                    match delimiter {
                        '(' => ')',
                        '[' => ']',
                        '{' => '}',
                        _ => '?',
                    },
                    delimiter,
                    position
                ));
            }
            RegexErrorType::InvalidQuantifier {
                message,
                position: _,
            } => {
                output.push_str("  Type: Invalid quantifier\n");
                output.push_str(&format!("  Details: {}\n", message));
            }
            RegexErrorType::LookaroundError { message, .. } => {
                output.push_str("  Type: Lookaround error\n");
                output.push_str(&format!("  Details: {}\n", message));
            }
            RegexErrorType::InvalidBackreference { message, .. } => {
                output.push_str("  Type: Backreference error\n");
                output.push_str(&format!("  Details: {}\n", message));
            }
            RegexErrorType::Other { message } => {
                output.push_str("  Type: Other error\n");
                output.push_str(&format!("  Details: {}\n", message));
            }
        }

        if let Some(ref suggestion) = self.suggestion {
            output.push_str(&format!("  Suggestion: {}\n", suggestion));
        }

        output
    }
}

/// Helper: Find the position of an unclosed delimiter
fn find_unclosed_delimiter(pattern: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut in_char_class = false;
    let mut chars = pattern.chars().enumerate();

    for (_i, c) in &mut chars {
        if c == '[' && open != '[' {
            in_char_class = true;
        } else if c == ']' && open != '[' {
            in_char_class = false;
        }

        if !in_char_class || open == '[' {
            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
            }
        }
    }

    // If depth > 0, we have unclosed opening delimiters
    if depth > 0 {
        // Find the position of the last unmatched opening delimiter
        let mut depth = 0;
        let mut in_char_class = false;
        let mut last_open_pos = 0;

        for (i, c) in pattern.chars().enumerate() {
            if c == '[' && open != '[' {
                in_char_class = true;
            } else if c == ']' && open != '[' {
                in_char_class = false;
            }

            if !in_char_class || open == '[' {
                if c == open {
                    depth += 1;
                    last_open_pos = i;
                } else if c == close {
                    depth -= 1;
                }
            }
        }

        if depth > 0 { Some(last_open_pos) } else { None }
    } else {
        None
    }
}

/// Helper: Find invalid escape sequences in a pattern
fn find_invalid_escape(pattern: &str, flavor: RegexFlavor) -> Option<usize> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            let _escape_str = format!("\\{}", next);

            // Check if this is a valid escape sequence
            let is_valid = match next {
                // Special characters that are always valid escapes
                'n' | 't' | 'r' | 'f' | '0' => true,
                // Meta-characters that are valid to escape
                '\\' | '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}'
                | '|' => true,
                // Hex and Unicode escapes
                'x' | 'u' | 'U' => true,
                // Word/non-word/digit/non-digit/space/non-space
                'w' | 'W' | 'd' | 'D' | 's' | 'S' | 'b' | 'B' => true,
                // In BRE mode, these ARE valid escapes
                _ if matches!(flavor, RegexFlavor::BRE)
                    && matches!(next, '(' | ')' | '{' | '}' | '+' | '?' | '|') =>
                {
                    true
                }
                // In BRE/ERE, backreferences
                '1'..='9' => true,
                '&' if matches!(flavor, RegexFlavor::BRE | RegexFlavor::ERE) => true,
                // Otherwise, might be invalid
                _ => false,
            };

            if !is_valid {
                return Some(i);
            }
            i += 2;
        } else {
            i += 1;
        }
    }

    None
}

/// Helper: Extract the escape sequence at a given position
fn extract_escape_at(pattern: &str, pos: usize) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    if pos < chars.len() && chars[pos] == '\\' {
        if pos + 1 < chars.len() {
            format!("\\{}", chars[pos + 1])
        } else {
            "\\".to_string()
        }
    } else {
        format!("{}", chars.get(pos).unwrap_or(&' '))
    }
}

/// Helper: Find invalid quantifier positions
fn find_invalid_quantifier(pattern: &str) -> Option<usize> {
    let chars: Vec<char> = pattern.chars().collect();

    for i in 0..chars.len() {
        match chars[i] {
            '*' | '+' | '?' => {
                // Check if there's something before to repeat
                if i == 0 {
                    return Some(i);
                }
                // Check if preceded by an opening parenthesis or alternation
                if i > 0 && (chars[i - 1] == '(' || chars[i - 1] == '|') {
                    return Some(i);
                }
            }
            '{' => {
                // Check for invalid brace quantifier syntax
                // This is a simplified check
                let rest: String = chars[i..].iter().collect();
                if !rest.starts_with("{0}")
                    && !rest.starts_with("{1")
                    && !rest.contains(',')
                    && !rest.starts_with("{0,")
                    && !rest.starts_with("{1,")
                    && !rest.starts_with("{2,")
                    && !rest.starts_with("{3,")
                    && !rest.starts_with("{4,")
                    && !rest.starts_with("{5,")
                    && !rest.starts_with("{6,")
                    && !rest.starts_with("{7,")
                    && !rest.starts_with("{8,")
                    && !rest.starts_with("{9,")
                {
                    // Check if it's a valid quantifier like {3}, {3,}, {3,5}
                    let closing = rest.find('}');
                    if closing.is_none() || (closing.is_some() && closing.unwrap() < 2) {
                        return Some(i);
                    }
                }
            }
            _ => {}
        }
    }

    None
}

/// Convert a regex error to a helpful anyhow::Error
pub fn enhanced_regex_error_to_anyhow(
    err: &regex::Error,
    pattern: &str,
    flavor: RegexFlavor,
) -> anyhow::Error {
    let enhanced = EnhancedRegexError::from_regex_error(err, pattern, flavor);
    anyhow::anyhow!("{}", enhanced.display())
}

/// Compile a regex with enhanced error reporting
pub fn compile_regex_with_context(
    pattern: &str,
    flavor: RegexFlavor,
    case_insensitive: bool,
) -> Result<regex::Regex, anyhow::Error> {
    use regex::{Regex, RegexBuilder};

    let result = if case_insensitive {
        RegexBuilder::new(pattern).case_insensitive(true).build()
    } else {
        Regex::new(pattern)
    };

    match result {
        Ok(re) => Ok(re),
        Err(err) => Err(enhanced_regex_error_to_anyhow(&err, pattern, flavor)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unclosed_parenthesis() {
        let pattern = r#"(foo"#;
        let err = regex::Regex::new(pattern).unwrap_err();
        let enhanced = EnhancedRegexError::from_regex_error(&err, pattern, RegexFlavor::PCRE);

        assert!(matches!(
            enhanced.error_type,
            RegexErrorType::UnclosedDelimiter { .. }
        ));
        assert!(enhanced.suggestion.is_some());
    }

    #[test]
    fn test_unclosed_bracket() {
        let pattern = r#"[abc"#;
        let err = regex::Regex::new(pattern).unwrap_err();
        let enhanced = EnhancedRegexError::from_regex_error(&err, pattern, RegexFlavor::PCRE);

        assert!(matches!(
            enhanced.error_type,
            RegexErrorType::UnclosedDelimiter { .. }
        ));
    }

    #[test]
    fn test_invalid_quantifier_nothing_to_repeat() {
        let pattern = r#"*"#;
        let err = regex::Regex::new(pattern).unwrap_err();
        let enhanced = EnhancedRegexError::from_regex_error(&err, pattern, RegexFlavor::PCRE);

        assert!(matches!(
            enhanced.error_type,
            RegexErrorType::InvalidQuantifier { .. }
        ));
        assert!(enhanced.suggestion.is_some());
    }

    #[test]
    fn test_find_unclosed_delimiter() {
        assert_eq!(find_unclosed_delimiter("(foo", '(', ')'), Some(0));
        assert_eq!(find_unclosed_delimiter("(foo)(bar", '(', ')'), Some(5));
        assert_eq!(find_unclosed_delimiter("(foo)(bar)", '(', ')'), None);
    }

    #[test]
    fn test_display_formatting() {
        let pattern = r#"*"#;
        let err = regex::Regex::new(pattern).unwrap_err();
        let enhanced = EnhancedRegexError::from_regex_error(&err, pattern, RegexFlavor::PCRE);

        let display = enhanced.display();
        assert!(display.contains("Regex Error"));
        assert!(display.contains("Pattern:"));
        assert!(display.contains("Suggestion:"));
    }

    #[test]
    fn test_compile_regex_with_context_success() {
        let result = compile_regex_with_context(r#"foo.*bar"#, RegexFlavor::PCRE, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_regex_with_context_failure() {
        let result = compile_regex_with_context(r#"*"#, RegexFlavor::PCRE, false);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Regex Error"));
    }

    #[test]
    fn test_bre_mode_suggestions() {
        let pattern = r#"("#;
        let err = regex::Regex::new(pattern).unwrap_err();
        let enhanced = EnhancedRegexError::from_regex_error(&err, pattern, RegexFlavor::BRE);

        assert!(enhanced.suggestion.is_some());
        // The suggestion should mention the pattern and BRE mode
        let suggestion = enhanced.suggestion.unwrap();
        assert!(
            suggestion.contains("BRE mode")
                || suggestion.contains("-B")
                || suggestion.contains("(")
        );
    }
}
