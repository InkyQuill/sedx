use crate::cli::RegexFlavor;
use crate::command::SubstitutionFlags;
use crate::regex_error::compile_regex_with_context;
use anyhow::Result;
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

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct MixedRangeKey {
    pub command_index: usize,
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
            Some(n) if n > 0 => {
                // Replace only the Nth occurrence
                let mut result = line.to_string();
                let mut count = 0;
                for mat in re.find_iter(line) {
                    count += 1;
                    if count == n {
                        result = format!(
                            "{}{}{}",
                            &line[..mat.start()],
                            processed_replacement,
                            &line[mat.end()..]
                        );
                        break;
                    }
                }
                Ok(result)
            }
            Some(_) => Ok(line.to_string()), // 0 means no substitution
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
