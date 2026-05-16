use crate::command::{Command, SubstitutionFlags};
use crate::parser::address::parse_optional_range;
use crate::parser::commands;
use crate::parser::errors::format_parse_error;
use anyhow::{Result, anyhow};

/// Fold a raw sed-flag character sequence into a SubstitutionFlags value.
pub fn fold_substitution_flags(flags: &[char]) -> SubstitutionFlags {
    let mut out = SubstitutionFlags::default();
    let mut index = 0;

    while index < flags.len() {
        match flags[index] {
            'g' => out.global = true,
            'p' => out.print = true,
            'i' | 'I' => out.case_insensitive = true,
            '0'..='9' => {
                let mut nth = 0;
                while index < flags.len() && flags[index].is_ascii_digit() {
                    nth = nth * 10 + flags[index].to_digit(10).unwrap() as usize;
                    index += 1;
                }
                out.nth = Some(nth);
                continue;
            }
            _ => {} // Ignore unknown flags
        }
        index += 1;
    }
    out
}

pub fn parse_substitution(cmd: &str) -> Result<Command> {
    let (s_pos, command) = commands::find_command_char(cmd).ok_or_else(|| {
        anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "'s' command not followed by a valid delimiter",
                Some("Substitution format: s<delimiter>pattern<delimiter>replacement<delimiter>[flags]\nDelimiters: / (slash), # (hash), : (colon), | (pipe)\nExample: s/foo/bar/ or s#old#new#g"),
            )
        )
    })?;
    if command != 's' {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                Some(s_pos),
                &format!("expected substitution command but found '{}'", command),
                Some(
                    "Substitution format: s<delimiter>pattern<delimiter>replacement<delimiter>[flags]\nExample: s/foo/bar/ or /pattern/s/foo/bar/"
                )
            )
        ));
    }

    // Everything before 's' is the address/range
    let address_part = &cmd[..s_pos];
    let rest = &cmd[s_pos + 1..]; // Skip the 's'

    // Detect delimiter
    let delimiter = rest.chars().next().ok_or_else(|| {
        anyhow!(
            "{}",
            format_parse_error(
                cmd,
                Some(s_pos + 1),
                "missing delimiter after 's'",
                Some("Expected format: s<delimiter>pattern<delimiter>replacement<delimiter>[flags]\nExample: s/foo/bar/ or s#old#new#g"),
            )
        )
    })?;

    // Find all delimiter positions
    let mut delimiter_positions: Vec<usize> = Vec::new();

    // Use char_indices() to get correct byte positions for UTF-8 strings
    for (byte_pos, c) in rest.char_indices() {
        if c == delimiter {
            delimiter_positions.push(byte_pos);
        }
    }

    if delimiter_positions.len() < 3 {
        // Provide specific error based on how many delimiters were found
        let (description, suggestion) = match delimiter_positions.len() {
            0 => (
                format!(
                    "missing closing delimiter: no '{}' delimiter found after the opening delimiter",
                    delimiter
                ),
                Some(
                    "Make sure to close the pattern, replacement, and optionally add flags:\n  s/pattern/replacement/\n  s/pattern/replacement/g",
                ),
            ),
            1 => (
                format!(
                    "missing closing delimiter: missing second '{}' delimiter after pattern",
                    delimiter
                ),
                Some(
                    "Each substitution needs three delimiters: s/pattern/replacement/\n                ^       ^           ^",
                ),
            ),
            2 => (
                format!(
                    "missing closing delimiter: missing third '{}' delimiter after replacement",
                    delimiter
                ),
                Some(
                    "The replacement string must be followed by the delimiter:\n  s/pattern/replacement/",
                ),
            ),
            _ => unreachable!(),
        };

        return Err(anyhow!(
            "{}",
            format_parse_error(cmd, Some(s_pos + 1), &description, suggestion)
        ));
    }

    // Extract pattern and replacement
    // delimiter_positions[0] is always 0 (the opening delimiter)
    let pattern = &rest[delimiter_positions[0] + 1..delimiter_positions[1]];
    let replacement = &rest[delimiter_positions[1] + 1..delimiter_positions[2]];
    let flags_part = &rest[delimiter_positions[2] + 1..];

    // Parse flags
    let flags_vec: Vec<char> = flags_part.chars().collect();
    let flags = fold_substitution_flags(&flags_vec);

    // Parse address range
    let range = parse_optional_range(address_part)?;

    Ok(Command::Substitution {
        pattern: pattern.to_string(),
        replacement: replacement.to_string(),
        flags,
        range,
    })
}
