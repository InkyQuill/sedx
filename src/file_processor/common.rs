use crate::cli::RegexFlavor;
use crate::command::Address;
use crate::command::SubstitutionFlags;
use crate::regex_error::compile_regex_with_context;
use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Preserve the destination file's mode across a write.
/// Reads permissions BEFORE the write closure runs, then re-applies them after.
/// Best-effort: silently ignores permission errors (e.g., when the file is new).
/// Used for both in-memory `fs::write` and streaming `NamedTempFile::persist` — the
/// rename-based atomic write loses the original mode on every filesystem, and
/// `fs::write`'s in-place truncation only incidentally preserves it on local ext4.
pub fn preserve_perms_after<F: FnOnce() -> Result<()>>(path: &Path, write: F) -> Result<()> {
    let perms = fs::metadata(path).ok().map(|m| m.permissions());
    write()?;
    if let Some(p) = perms {
        let _ = fs::set_permissions(path, p);
    }
    Ok(())
}

pub struct AddressContext<'a> {
    pub line: &'a str,
    pub line_number: usize,
    pub total_lines: Option<usize>,
    pub is_last_line: bool,
}

pub fn matches_address(address: &Address, context: &AddressContext<'_>) -> bool {
    match address {
        Address::LineNumber(n) => {
            if *n == 0 {
                context.line_number == 0
            } else {
                context.line_number == *n
            }
        }
        Address::Pattern(pattern) => Regex::new(pattern)
            .map(|re| re.is_match(context.line))
            .unwrap_or(false),
        Address::FirstLine => context.line_number == 1,
        Address::LastLine => context
            .total_lines
            .map_or(context.is_last_line, |total| context.line_number == total),
        Address::Negated(inner) => !matches_address(inner, context),
        Address::Relative { base, offset } => {
            let base_line = match base.as_ref() {
                Address::LineNumber(n) => *n as isize,
                _ => context.line_number as isize,
            };
            base_line + *offset == context.line_number as isize
        }
        Address::Step { start, step } => {
            context.line_number >= *start && (context.line_number - *start).is_multiple_of(*step)
        }
        Address::Single(inner) => matches_address(inner, context),
    }
}

pub fn try_matches_address(address: &Address, context: &AddressContext<'_>) -> Result<bool> {
    match address {
        Address::Pattern(pattern) => {
            let re = Regex::new(pattern)
                .with_context(|| format!("Invalid regex pattern: {}", pattern))?;
            Ok(re.is_match(context.line))
        }
        Address::Negated(inner) => Ok(!try_matches_address(inner, context)?),
        Address::Single(inner) => try_matches_address(inner, context),
        _ => Ok(matches_address(address, context)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_last_line_from_total_line_count() {
        let context = AddressContext {
            line: "last",
            line_number: 3,
            total_lines: Some(3),
            is_last_line: false,
        };

        assert!(matches_address(&Address::LastLine, &context));
    }

    #[test]
    fn matches_last_line_from_streaming_lookahead() {
        let context = AddressContext {
            line: "last",
            line_number: 3,
            total_lines: None,
            is_last_line: true,
        };

        assert!(matches_address(&Address::LastLine, &context));
    }

    #[test]
    fn matches_negated_pattern_address() {
        let context = AddressContext {
            line: "alpha",
            line_number: 1,
            total_lines: Some(1),
            is_last_line: true,
        };

        assert!(matches_address(
            &Address::Negated(Box::new(Address::Pattern("bravo".to_string()))),
            &context,
        ));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    Unchanged, // Line not modified
    Modified,  // Line content changed
    Added,     // New line inserted
    Deleted,   // Line removed
}

#[derive(Debug, Clone)]
pub struct LineChange {
    pub line_number: usize,
    pub change_type: ChangeType,
    pub content: String,
    /// Pre-change content, populated on `ChangeType::Modified` variants so
    /// that consumers inspecting a `LineChange` can show the before/after
    /// pair. Read from the `#[cfg(test)]` block in `diff_formatter.rs` and
    /// from any downstream library consumer that iterates a `FileDiff`.
    /// The sedx binary itself only writes this field; the `dead_code`
    /// allow is necessary until a non-test reader lands in production.
    #[allow(dead_code)]
    pub old_content: Option<String>,
}

#[derive(Debug)]
pub struct FileDiff {
    pub file_path: String,
    pub changes: Vec<LineChange>,
    pub all_lines: Vec<(usize, String, ChangeType)>, // (line_number, content, change_type)
    pub printed_lines: Vec<String>,                  // Lines from print commands
    pub is_streaming: bool, // True if processed in streaming mode (all_lines may be empty)
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CommandKey(Box<[usize]>);

impl CommandKey {
    pub fn root(index: usize) -> Self {
        Self(Box::new([index]))
    }

    pub fn child(&self, index: usize) -> Self {
        let mut path = self.0.to_vec();
        path.push(index);
        Self(path.into_boxed_slice())
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct MixedRangeKey {
    pub command_key: CommandKey,
}

#[derive(Clone, PartialEq)]
pub enum MixedRangeState {
    LookingForPattern,
    InRangeUntilLine { target_line: usize },
    InRangeUntilPattern { end_pattern: String },
}

#[derive(Clone, PartialEq)]
pub enum PatternRangeState {
    LookingForStart,
    InRange,
}

/// Shared substitution logic to ensure consistency between streaming and in-memory modes.
pub struct SubstitutionEngine {
    regex_flavor: RegexFlavor,
}

impl SubstitutionEngine {
    pub fn new(regex_flavor: RegexFlavor) -> Self {
        Self { regex_flavor }
    }

    /// Apply substitution to a single line
    pub fn apply(
        &self,
        line: &str,
        pattern: &str,
        replacement: &str,
        flags: &SubstitutionFlags,
    ) -> Result<String> {
        let global = flags.global;
        let case_insensitive = flags.case_insensitive;
        let nth_occurrence = flags.nth;

        // Process escape sequences in replacement
        let processed_replacement = self.process_replacement_escapes(replacement);

        let re = compile_regex_with_context(pattern, self.regex_flavor, case_insensitive)?;

        match nth_occurrence {
            Some(0) => Ok(re
                .replace_all(line, processed_replacement.as_str())
                .to_string()),
            Some(n) => {
                // Replace only the Nth occurrence while preserving regex replacement expansion.
                for (index, captures) in re.captures_iter(line).enumerate() {
                    if index + 1 == n {
                        let mat = captures
                            .get(0)
                            .expect("regex captures always include the whole match");
                        let mut result =
                            String::with_capacity(line.len() + processed_replacement.len());
                        result.push_str(&line[..mat.start()]);
                        captures.expand(processed_replacement.as_str(), &mut result);
                        result.push_str(&line[mat.end()..]);
                        return Ok(result);
                    }
                }

                Ok(line.to_string())
            }
            None => {
                // Standard behavior
                if global {
                    Ok(re
                        .replace_all(line, processed_replacement.as_str())
                        .to_string())
                } else {
                    Ok(re.replace(line, processed_replacement.as_str()).to_string())
                }
            }
        }
    }

    /// Process escape sequences in replacement string
    /// Supports: \n, \t, \r, \\, \xHH, \uHHHH
    pub fn process_replacement_escapes(&self, replacement: &str) -> String {
        let mut result = String::with_capacity(replacement.len());
        let mut chars = replacement.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.peek() {
                    Some('n') => {
                        result.push('\n');
                        chars.next();
                    }
                    Some('t') => {
                        result.push('\t');
                        chars.next();
                    }
                    Some('r') => {
                        result.push('\r');
                        chars.next();
                    }
                    Some('\\') => {
                        result.push('\\');
                        chars.next();
                    }
                    Some('x') => {
                        // Hex escape: \xHH
                        chars.next(); // consume 'x'
                        let mut hex = String::new();
                        for _ in 0..2 {
                            if let Some(&c) = chars.peek()
                                && c.is_ascii_hexdigit()
                            {
                                hex.push(c);
                                chars.next();
                            }
                        }
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            result.push(byte as char);
                        }
                    }
                    Some('u') => {
                        // Unicode escape: \uHHHH
                        chars.next(); // consume 'u'
                        let mut hex = String::new();
                        for _ in 0..4 {
                            if let Some(&c) = chars.peek()
                                && c.is_ascii_hexdigit()
                            {
                                hex.push(c);
                                chars.next();
                            }
                        }
                        if let Ok(codepoint) = u32::from_str_radix(&hex, 16)
                            && let Some(c) = char::from_u32(codepoint)
                        {
                            result.push(c);
                        }
                    }
                    Some(&c) => {
                        // Unknown escape, keep as-is
                        result.push('\\');
                        result.push(c);
                        chars.next();
                    }
                    None => {
                        result.push('\\');
                    }
                }
            } else if c == '$' {
                // Handle backreferences: $1, $2, ${name}
                let mut reference = String::from('$');
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_ascii_digit() || next_c == '{' {
                        reference.push(next_c);
                        chars.next();
                        if next_c == '}' {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                result.push_str(&reference);
            } else {
                result.push(c);
            }
        }

        result
    }
}
