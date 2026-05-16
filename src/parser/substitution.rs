use crate::command::{Command, SubstitutionFlags};
use crate::parser::address::parse_optional_range;
use crate::parser::commands;
use crate::parser::errors::format_parse_error;
use anyhow::{Result, anyhow};

/// Fold a raw sed-flag character sequence into a SubstitutionFlags value.
pub fn fold_substitution_flags(
    cmd: &str,
    flags: &str,
    flags_pos: usize,
) -> Result<SubstitutionFlags> {
    let mut out = SubstitutionFlags::default();
    let mut index = 0;
    let flags: Vec<char> = flags.chars().collect();

    while index < flags.len() {
        match flags[index] {
            'g' => out.global = true,
            'p' => out.print = true,
            'i' | 'I' => out.case_insensitive = true,
            '0'..='9' => {
                let mut nth = 0;
                while index < flags.len() && flags[index].is_ascii_digit() {
                    nth = nth * 10 + (flags[index] as usize - '0' as usize);
                    index += 1;
                }
                out.nth = Some(nth);
                continue;
            }
            unknown => {
                return Err(anyhow!(
                    "{}",
                    format_parse_error(
                        cmd,
                        Some(
                            flags_pos + flags[..index].iter().map(|c| c.len_utf8()).sum::<usize>()
                        ),
                        &format!("unknown substitution flag '{}'", unknown),
                        Some(
                            "Valid substitution flags are: g, p, i, I, or digits for the nth match"
                        ),
                    )
                ));
            }
        }
        index += 1;
    }
    Ok(out)
}

fn scan_substitution_section(
    rest: &str,
    start: usize,
    delimiter: char,
    preserve_escaped_delimiter: bool,
) -> Option<(String, usize, usize)> {
    let mut section = String::new();
    let mut index = start;

    while index < rest.len() {
        let c = rest[index..].chars().next()?;
        let c_len = c.len_utf8();

        if c == delimiter {
            return Some((section, index, index + c_len));
        }

        if c == '\\' {
            let next_index = index + c_len;
            if next_index >= rest.len() {
                section.push('\\');
                index = next_index;
                continue;
            }

            let next = rest[next_index..].chars().next()?;
            if next == delimiter {
                if preserve_escaped_delimiter {
                    section.push('\\');
                    section.push(delimiter);
                } else {
                    section.push(delimiter);
                }
            } else {
                section.push('\\');
                section.push(next);
            }
            index = next_index + next.len_utf8();
            continue;
        }

        section.push(c);
        index += c_len;
    }

    None
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

    let pattern_start = delimiter.len_utf8();
    let Some((pattern, _, replacement_start)) =
        scan_substitution_section(rest, pattern_start, delimiter, true)
    else {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                Some(s_pos + 1),
                &format!(
                    "missing closing delimiter: missing second '{}' delimiter after pattern",
                    delimiter
                ),
                Some(
                    "Each substitution needs three delimiters: s/pattern/replacement/\n                ^       ^           ^",
                )
            )
        ));
    };

    let Some((replacement, _, flags_start)) =
        scan_substitution_section(rest, replacement_start, delimiter, false)
    else {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                Some(s_pos + 1),
                &format!(
                    "missing closing delimiter: missing third '{}' delimiter after replacement",
                    delimiter
                ),
                Some(
                    "The replacement string must be followed by the delimiter:\n  s/pattern/replacement/",
                )
            )
        ));
    };

    let flags_part = &rest[flags_start..];

    // Parse flags
    let flags = fold_substitution_flags(cmd, flags_part, s_pos + 1 + flags_start)?;

    // Parse address range
    let range = parse_optional_range(address_part)?;

    Ok(Command::Substitution {
        pattern,
        replacement,
        flags,
        range,
    })
}
