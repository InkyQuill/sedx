use crate::command::Address;
use crate::parser::errors::format_parse_error;
use anyhow::{Result, anyhow, bail};

/// Parse an optional address range (e.g., "1,5", "/foo/", "10", etc.)
/// Returns None if no address (applies to all lines)
/// Returns Some((start, end)) if address or range specified
pub fn parse_optional_range(addr_part: &str) -> Result<Option<(Address, Address)>> {
    let addr_part = addr_part.trim();

    if addr_part.is_empty() {
        return Ok(None); // No address = applies to all lines
    }

    if let Some(comma_pos) = addr_part.find(',') {
        // Range: addr1,addr2
        let start = &addr_part[..comma_pos];
        let end = &addr_part[comma_pos + 1..];

        // Chunk 8: Check if end has relative offset (+N or -N)
        if end.starts_with('+') || end.starts_with('-') {
            // Relative range: /pattern/,+5 or 10,+3
            let start_addr = parse_address(start)?;

            // Parse the offset
            let offset_str = &end[1..]; // Skip +/-
            let offset: isize = offset_str.parse().map_err(|_| {
                anyhow!(
                    "{}",
                    format_parse_error(
                        end,
                        None,
                        &format!("invalid relative offset '{}'", end),
                        Some("Relative offset format: start,+N or start,-N\nExample: /pattern/,+5  - 5 lines after pattern\n         10,-3       - 3 lines before line 10"),
                    )
                )
            })?;

            let end_addr = Address::Relative {
                base: Box::new(start_addr.clone()),
                offset,
            };

            return Ok(Some((start_addr, end_addr)));
        }

        // Normal range
        let start_addr = parse_address(start)?;
        let end_addr = parse_address(end)?;
        return Ok(Some((start_addr, end_addr)));
    }

    // Single address
    let addr = parse_address(addr_part)?;
    Ok(Some((addr.clone(), addr)))
}

pub fn parse_address(addr: &str) -> Result<Address> {
    let addr = addr.trim();

    // Empty address (not valid in our context)
    if addr.is_empty() {
        return Err(anyhow!("Empty address"));
    }

    // Check for negation operator (! as suffix)
    if let Some(inner_addr) = addr.strip_suffix('!') {
        let parsed = parse_address(inner_addr)?;
        return Ok(Address::Negated(Box::new(parsed)));
    }

    // Special address: 0 (for first-match substitution)
    if addr == "0" {
        return Ok(Address::FirstLine);
    }

    // Special address: $ (last line)
    if addr == "$" {
        return Ok(Address::LastLine);
    }

    // Chunk 8: Stepping address: 1~2 (every 2nd line starting from line 1)
    if let Some(tilde_pos) = addr.find('~') {
        let start_str = &addr[..tilde_pos];
        let step_str = &addr[tilde_pos + 1..];

        let start: usize = start_str.parse().map_err(|_| {
            anyhow!(
                "{}",
                format_parse_error(
                    addr,
                    Some(tilde_pos),
                    &format!("invalid step start '{}'", start_str),
                    Some("Step format: start~step\nExample: 1~2  - every 2nd line starting from line 1\n         10~5 - every 5th line starting from line 10"),
                )
            )
        })?;
        let step: usize = step_str.parse().map_err(|_| {
            anyhow!(
                "{}",
                format_parse_error(
                    addr,
                    Some(tilde_pos + 1),
                    &format!("invalid step value '{}'", step_str),
                    Some("Step format: start~step\nThe step value must be a positive integer.\nExample: 1~2 or 10~5"),
                )
            )
        })?;

        if step == 0 {
            bail!(
                "{}",
                format_parse_error(
                    addr,
                    Some(tilde_pos + 1),
                    "step value cannot be zero",
                    Some(
                        "Use a positive integer for the step value.\nExample: 1~1 (every line) or 1~2 (every other line)"
                    ),
                )
            );
        }

        return Ok(Address::Step { start, step });
    }

    // Line number
    if let Ok(num) = addr.parse::<usize>() {
        return Ok(Address::LineNumber(num));
    }

    // Pattern with a custom delimiter: \#pattern#, \|pattern|, etc.
    if let Some(pattern) = parse_custom_delimiter_pattern(addr) {
        return Ok(Address::Pattern(pattern));
    }

    // Pattern: /pattern/
    if addr.starts_with('/') && addr.ends_with('/') {
        let pattern = &addr[1..addr.len() - 1];
        return Ok(Address::Pattern(pattern.to_string()));
    }

    // Pattern missing closing slash
    if addr.starts_with('/') && !addr.ends_with('/') {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                addr,
                Some(addr.len()),
                "pattern address is missing closing '/'",
                Some(
                    "Pattern addresses must be enclosed in slashes:\n  /pattern/\n  /^hello/\n  /goodbye$/"
                ),
            )
        ));
    }

    // Pattern missing opening slash
    if addr.ends_with('/') && !addr.starts_with('/') {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                addr,
                Some(0),
                "pattern address is missing opening '/'",
                Some(
                    "Pattern addresses must be enclosed in slashes:\n  /pattern/\n  /^hello/\n  /goodbye$/"
                ),
            )
        ));
    }

    Err(anyhow!(
        "{}",
        format_parse_error(
            addr,
            None,
            &format!("invalid address '{}'", addr),
            Some(
                "Valid address formats:\n  - Line number: 5, 10, 42\n  - Last line: $\n  - Pattern: /regex/\n  - Range: 1,10 or /start/,/end/\n  - Stepping: 1~2 (every 2nd line)\n  - Relative: /pat/,+5 (5 lines after pattern match)"
            ),
        )
    ))
}

fn parse_custom_delimiter_pattern(addr: &str) -> Option<String> {
    let rest = addr.strip_prefix('\\')?;
    let mut chars = rest.chars();
    let delimiter = chars.next()?;
    let pattern_start = '\\'.len_utf8() + delimiter.len_utf8();

    if addr.len() <= pattern_start {
        return None;
    }

    let mut escaped = false;
    for (offset, ch) in addr[pattern_start..].char_indices() {
        let pos = pattern_start + offset;
        if escaped {
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == delimiter && pos + ch.len_utf8() == addr.len() {
            return Some(addr[pattern_start..pos].to_string());
        }
    }

    None
}
