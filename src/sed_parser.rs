use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum SedCommand {
    Substitution {
        pattern: String,
        replacement: String,
        flags: Vec<char>,
        range: Option<(Address, Address)>, // Line range for substitution
    },
    Delete {
        range: (Address, Address), // What to delete
    },
    Insert {
        text: String,
        address: Address, // Where to insert (before)
    },
    Append {
        text: String,
        address: Address, // Where to append (after)
    },
    Change {
        text: String,
        address: Address, // Which line(s) to change
    },
    Print {
        range: (Address, Address), // What to print
    },
    Quit {
        address: Option<Address>, // q or 10q or /pattern/q
    },
    Group {
        range: Option<(Address, Address)>, // Optional range for the group
        commands: Vec<SedCommand>, // Commands to execute
    },
    // Hold space operations
    Hold {
        range: Option<(Address, Address)>, // h command - copy to hold space
    },
    HoldAppend {
        range: Option<(Address, Address)>, // H command - append to hold space
    },
    Get {
        range: Option<(Address, Address)>, // g command - get from hold space
    },
    GetAppend {
        range: Option<(Address, Address)>, // G command - append from hold space
    },
    Exchange {
        range: Option<(Address, Address)>, // x command - exchange buffers
    },
    // Phase 4: Multi-line pattern space commands
    Next {
        range: Option<(Address, Address)>, // n command - next line
    },
    NextAppend {
        range: Option<(Address, Address)>, // N command - append next line
    },
    PrintFirstLine {
        range: Option<(Address, Address)>, // P command - print first line
    },
    DeleteFirstLine {
        range: Option<(Address, Address)>, // D command - delete first line
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Address {
    LineNumber(usize),
    Pattern(String),
    FirstLine, // Special address "0" for first-match substitution
    LastLine,  // Special address "$" for last line
    Negated(Box<Address>),  // Negation: !/pattern/ or !10
    // Chunk 8: New address types
    Relative { base: Box<Address>, offset: isize },  // /pattern/,+5 or 10,+3
    Step { start: usize, step: usize },              // 1~2 (every 2nd line from line 1)
}

pub fn parse_sed_expression(expr: &str) -> Result<Vec<SedCommand>> {
    let mut commands = Vec::new();

    // Handle multiple expressions separated by ;
    // But skip semicolons inside braces { ... }
    let mut current_expr = String::new();
    let mut in_braces = 0;
    let mut chars = expr.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                in_braces += 1;
                current_expr.push(c);
            }
            '}' => {
                in_braces -= 1;
                current_expr.push(c);
            }
            ';' if in_braces == 0 => {
                // Semicolon at top level - command separator
                let part = current_expr.trim();
                if !part.is_empty() {
                    commands.push(parse_single_command(part)?);
                }
                current_expr.clear();
            }
            _ => {
                current_expr.push(c);
            }
        }
    }

    // Don't forget the last expression
    let part = current_expr.trim();
    if !part.is_empty() {
        commands.push(parse_single_command(part)?);
    }

    Ok(commands)
}

fn parse_single_command(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Check for command grouping with braces
    if cmd.contains('{') {
        return parse_group(cmd);
    }

    // IMPORTANT: Check for substitution commands FIRST
    // because substitution commands can end with 'g' (global flag), 'p' (print flag), etc.
    // which would otherwise be misidentified as get/print/hold commands
    if cmd.contains("s/") || cmd.contains("s#") || cmd.contains("s:") || cmd.contains("s|") {
        return parse_substitution(cmd);
    }

    // Check for hold space commands
    // These need to be checked carefully to avoid confusion with substitution patterns
    let last_char = cmd.chars().last().unwrap_or(' ');

    if last_char == 'h' || last_char == 'H' {
        // Hold command - check it's not part of a substitution
        if !cmd.starts_with('s') && cmd.chars().filter(|&c| c == 's').count() <= 1 {
            return if last_char == 'H' {
                parse_hold_append(cmd)
            } else {
                parse_hold(cmd)
            };
        }
    }

    if last_char == 'g' || last_char == 'G' {
        // Get command - check it's not part of a substitution
        if !cmd.starts_with('s') && cmd.chars().filter(|&c| c == 's').count() <= 1 {
            return if last_char == 'G' {
                parse_get_append(cmd)
            } else {
                parse_get(cmd)
            };
        }
    }

    if last_char == 'x' {
        // Exchange command - check it's not part of a substitution
        if !cmd.starts_with('s') && cmd.chars().filter(|&c| c == 's').count() <= 1 {
            return parse_exchange(cmd);
        }
    }

    // Phase 4: Multi-line pattern space commands
    if last_char == 'n' && !cmd.starts_with('s') {
        // Next command - check it's not part of a substitution
        if cmd.chars().filter(|&c| c == 's').count() <= 1 {
            return parse_next(cmd);
        }
    }

    if last_char == 'N' && !cmd.starts_with('s') {
        // Next append command
        if cmd.chars().filter(|&c| c == 's').count() <= 1 {
            return parse_next_append(cmd);
        }
    }

    if last_char == 'P' && !cmd.starts_with('s') {
        // Print first line command
        if cmd.chars().filter(|&c| c == 's').count() <= 1 {
            return parse_print_first_line(cmd);
        }
    }

    if last_char == 'D' && !cmd.starts_with('s') {
        // Delete first line command
        if cmd.chars().filter(|&c| c == 's').count() <= 1 {
            return parse_delete_first_line(cmd);
        }
    }

    // Determine command type by looking at the last character or special patterns
    if cmd.ends_with('q') && !cmd.starts_with('s') {
        // Quit command
        parse_quit(cmd)
    } else if cmd.ends_with('d') {
        // Delete command
        parse_delete(cmd)
    } else if cmd.ends_with('p') && !cmd.starts_with('s') {
        // Print command (but not s/pattern/replacement/p which is a flag)
        parse_print(cmd)
    } else if cmd.contains("i\\") {
        // Insert command
        parse_insert(cmd)
    } else if cmd.contains("a\\") {
        // Append command
        parse_append(cmd)
    } else if cmd.contains("c\\") {
        // Change command
        parse_change(cmd)
    } else {
        // Try to determine by last character
        let command_char = cmd.chars().last()
            .ok_or_else(|| anyhow!("Empty command"))?;

        match command_char {
            's' => parse_substitution(cmd),
            'q' => parse_quit(cmd),
            'd' => parse_delete(cmd),
            'p' => parse_print(cmd),
            'h' => parse_hold(cmd),
            'H' => parse_hold_append(cmd),
            'g' => {
                parse_get(cmd)
            }
            'G' => parse_get_append(cmd),
            'x' => parse_exchange(cmd),
            'n' => parse_next(cmd),
            'N' => parse_next_append(cmd),
            'P' => parse_print_first_line(cmd),
            'D' => parse_delete_first_line(cmd),
            _ => {
                Err(anyhow!("Unknown sed command: {}", cmd))
            }
        }
    }
}

fn parse_substitution(cmd: &str) -> Result<SedCommand> {
    // Find the 's' that starts the substitution command
    // It's the first 's' followed by a delimiter (/, #, :, etc.)
    let bytes = cmd.as_bytes();
    let mut s_pos = None;

    for (i, &byte) in bytes.iter().enumerate() {
        if byte == b's' && i + 1 < bytes.len() {
            let next_byte = bytes[i + 1];
            // Check if next char is a valid delimiter
            if next_byte == b'/' || next_byte == b'#' || next_byte == b':' || next_byte == b'|' {
                s_pos = Some(i);
                break;
            }
        }
    }

    let s_pos = s_pos.ok_or_else(|| anyhow!("Invalid substitution command: {}", cmd))?;

    // Everything before 's' is the address/range
    let address_part = &cmd[..s_pos];
    let rest = &cmd[s_pos + 1..];  // Skip the 's'

    // Detect delimiter
    let delimiter = rest.chars().next()
        .ok_or_else(|| anyhow!("Missing delimiter"))?;

    // Find all delimiter positions
    let mut delimiter_positions = Vec::new();
    let mut chars = rest.chars().peekable();
    let mut i = 0;

    while let Some(c) = chars.next() {
        if c == delimiter {
            delimiter_positions.push(i);
        }
        i += 1;
    }

    if delimiter_positions.len() < 3 {
        return Err(anyhow!("Invalid substitution syntax. Expected: s/pattern/replacement/flags"));
    }

    let pattern = &rest[delimiter_positions[0] + 1..delimiter_positions[1]];
    let replacement_raw = &rest[delimiter_positions[1] + 1..delimiter_positions[2]];
    let replacement = convert_sed_backreferences(replacement_raw);
    let flags: Vec<char> = if delimiter_positions[2] + 1 < rest.len() {
        rest[delimiter_positions[2] + 1..].chars().collect()
    } else {
        Vec::new()
    };

    // Parse address/range if present
    let range = if address_part.contains(',') {
        // Range: start,ends/pattern/replacement/
        let parts: Vec<&str> = address_part.splitn(2, ',').collect();
        if parts.len() == 2 {
            let start = parse_address(parts[0])?;
            let end_str = parts[1].trim();

            // Chunk 8: Check if end has relative offset (+N or -N)
            if end_str.starts_with('+') || end_str.starts_with('-') {
                // Relative range: /pattern/,+5
                let offset_str = &end_str[1..];  // Skip +/-
                let offset: isize = offset_str.parse()
                    .map_err(|_| anyhow!("Invalid relative offset: {}", end_str))?;

                let end = Address::Relative {
                    base: Box::new(start.clone()),
                    offset,
                };
                Some((start, end))
            } else {
                // Normal range
                let end = parse_address(end_str)?;
                Some((start, end))
            }
        } else {
            None
        }
    } else if !address_part.trim().is_empty() {
        // Single address: addrs/pattern/replacement/
        let addr = parse_address(address_part.trim())?;
        Some((addr.clone(), addr))
    } else {
        None
    };

    Ok(SedCommand::Substitution {
        pattern: pattern.to_string(),
        replacement: replacement.to_string(),
        flags,
        range,
    })
}

fn parse_delete(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'd'

    // Check for range: start,endd
    if let Some(comma_pos) = addr_part.find(',') {
        let start = &addr_part[..comma_pos];
        let end = &addr_part[comma_pos + 1..];

        return Ok(SedCommand::Delete {
            range: (parse_address(start)?, parse_address(end)?),
        });
    }

    // For simple addresses, use parse_address directly
    let addr = parse_address(addr_part)?;
    Ok(SedCommand::Delete {
        range: (addr.clone(), addr),
    })
}

fn parse_print(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'p'

    // Empty address means print all lines (1 to $)
    if addr_part.trim().is_empty() {
        return Ok(SedCommand::Print {
            range: (Address::LineNumber(1), Address::LastLine),
        });
    }

    // Check for range: start,endp
    if let Some(comma_pos) = addr_part.find(',') {
        let start = &addr_part[..comma_pos];
        let end = &addr_part[comma_pos + 1..];

        return Ok(SedCommand::Print {
            range: (parse_address(start)?, parse_address(end)?),
        });
    }

    // For simple addresses, use parse_address directly
    let addr = parse_address(addr_part)?;
    Ok(SedCommand::Print {
        range: (addr.clone(), addr),
    })
}

fn parse_quit(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'q'

    // Check if there's an address
    if addr_part.trim().is_empty() {
        // Just 'q' - quit immediately
        return Ok(SedCommand::Quit {
            address: None,
        });
    }

    // '10q' or '/pattern/q' - quit at that address
    let addr = parse_address(addr_part)?;
    Ok(SedCommand::Quit {
        address: Some(addr),
    })
}

fn parse_group(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the opening brace
    let open_brace = cmd.find('{')
        .ok_or_else(|| anyhow!("Invalid group command: missing '{{'"))?;

    // Extract the address/range part (before the brace)
    let addr_part = cmd[..open_brace].trim();

    // Find the matching closing brace
    let brace_start = open_brace + 1;
    let mut depth = 1;
    let mut close_brace = None;

    for (i, c) in cmd[brace_start..].chars().enumerate() {
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                close_brace = Some(brace_start + i);
                break;
            }
        }
    }

    let close_brace = close_brace
        .ok_or_else(|| anyhow!("Invalid group command: missing matching '}}'"))?;

    // Extract commands inside the braces
    let commands_str = &cmd[brace_start..close_brace].trim();

    // Parse the range if present
    let range = if addr_part.is_empty() {
        None
    } else if addr_part.contains(',') {
        // Range: start,end{...}
        let parts: Vec<&str> = addr_part.splitn(2, ',').collect();
        if parts.len() == 2 {
            Some((parse_address(parts[0].trim())?, parse_address(parts[1].trim())?))
        } else {
            None
        }
    } else {
        // Single address: addr{...}
        let addr = parse_address(addr_part)?;
        Some((addr.clone(), addr))
    };

    // Parse commands inside the group (separated by semicolons)
    let mut commands = Vec::new();
    for cmd_str in commands_str.split(';') {
        let cmd_str = cmd_str.trim();
        if !cmd_str.is_empty() {
            commands.push(parse_single_command(cmd_str)?);
        }
    }

    if commands.is_empty() {
        return Err(anyhow!("Empty group: no commands inside braces"));
    }

    Ok(SedCommand::Group {
        range,
        commands,
    })
}

fn parse_insert(cmd: &str) -> Result<SedCommand> {
    // Insert: i\text or addr i\text
    let parts: Vec<&str> = cmd.splitn(2, "i\\").collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid insert command: {}", cmd));
    }

    let address = if !parts[0].trim().is_empty() {
        parse_address(parts[0].trim())?
    } else {
        return Err(anyhow!("Insert command requires address: {}", cmd));
    };

    Ok(SedCommand::Insert {
        text: parts[1].to_string(),
        address,
    })
}

fn parse_append(cmd: &str) -> Result<SedCommand> {
    // Append: a\text or addr a\text
    let parts: Vec<&str> = cmd.splitn(2, "a\\").collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid append command: {}", cmd));
    }

    let address = if !parts[0].trim().is_empty() {
        parse_address(parts[0].trim())?
    } else {
        return Err(anyhow!("Append command requires address: {}", cmd));
    };

    Ok(SedCommand::Append {
        text: parts[1].to_string(),
        address,
    })
}

fn parse_change(cmd: &str) -> Result<SedCommand> {
    // Change: c\text or addr c\text
    let parts: Vec<&str> = cmd.splitn(2, "c\\").collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid change command: {}", cmd));
    }

    let address = if !parts[0].trim().is_empty() {
        parse_address(parts[0].trim())?
    } else {
        return Err(anyhow!("Change command requires address: {}", cmd));
    };

    Ok(SedCommand::Change {
        text: parts[1].to_string(),
        address,
    })
}

// Hold space command parsing functions

fn parse_hold(cmd: &str) -> Result<SedCommand> {
    // h or addr h or addr1,addr2 h
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'h'

    let range = parse_optional_range(addr_part)?;

    Ok(SedCommand::Hold { range })
}

fn parse_hold_append(cmd: &str) -> Result<SedCommand> {
    // H or addr H
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'H'

    let range = parse_optional_range(addr_part)?;

    Ok(SedCommand::HoldAppend { range })
}

fn parse_get(cmd: &str) -> Result<SedCommand> {
    // g or addr g
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'g'

    let range = parse_optional_range(addr_part)?;

    Ok(SedCommand::Get { range })
}

fn parse_get_append(cmd: &str) -> Result<SedCommand> {
    // G or addr G
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'G'

    let range = parse_optional_range(addr_part)?;

    Ok(SedCommand::GetAppend { range })
}

fn parse_exchange(cmd: &str) -> Result<SedCommand> {
    // x or addr x
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'x'

    let range = parse_optional_range(addr_part)?;

    Ok(SedCommand::Exchange { range })
}

// Phase 4: Multi-line pattern space command parsing functions

fn parse_next(cmd: &str) -> Result<SedCommand> {
    // n or addr n
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'n'

    let range = parse_optional_range(addr_part)?;

    Ok(SedCommand::Next { range })
}

fn parse_next_append(cmd: &str) -> Result<SedCommand> {
    // N or addr N
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'N'

    let range = parse_optional_range(addr_part)?;

    Ok(SedCommand::NextAppend { range })
}

fn parse_print_first_line(cmd: &str) -> Result<SedCommand> {
    // P or addr P
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'P'

    let range = parse_optional_range(addr_part)?;

    Ok(SedCommand::PrintFirstLine { range })
}

fn parse_delete_first_line(cmd: &str) -> Result<SedCommand> {
    // D or addr D
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'D'

    let range = parse_optional_range(addr_part)?;

    Ok(SedCommand::DeleteFirstLine { range })
}

/// Helper function to parse optional ranges for hold space commands
/// Returns None if no address (applies to all lines)
/// Returns Some((start, end)) if address or range specified
fn parse_optional_range(addr_part: &str) -> Result<Option<(Address, Address)>> {
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
            let offset_str = &end[1..];  // Skip +/-
            let offset: isize = offset_str.parse()
                .map_err(|_| anyhow!("Invalid relative offset: {}", end))?;

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

fn parse_address(addr: &str) -> Result<Address> {
    let addr = addr.trim();

    // Empty address (not valid in our context)
    if addr.is_empty() {
        return Err(anyhow!("Empty address"));
    }

    // Check for negation operator (! as suffix)
    if addr.ends_with('!') {
        let inner_addr = parse_address(&addr[..addr.len() - 1])?;
        return Ok(Address::Negated(Box::new(inner_addr)));
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

        let start: usize = start_str.parse()
            .map_err(|_| anyhow!("Invalid step start: {}", start_str))?;
        let step: usize = step_str.parse()
            .map_err(|_| anyhow!("Invalid step value: {}", step_str))?;

        if step == 0 {
            anyhow::bail!("Step value cannot be zero");
        }

        return Ok(Address::Step { start, step });
    }

    // Line number
    if let Ok(num) = addr.parse::<usize>() {
        return Ok(Address::LineNumber(num));
    }

    // Pattern: /pattern/
    if addr.starts_with('/') && addr.ends_with('/') {
        let pattern = &addr[1..addr.len() - 1];
        return Ok(Address::Pattern(pattern.to_string()));
    }

    Err(anyhow!("Invalid address: {}", addr))
}

/// Convert sed-style backreferences (\1, \2, etc.) to regex crate style ($1, $2, etc.)
///
/// GNU sed uses `\1`, `\2` for backreferences in replacement strings.
/// Rust's `regex` crate uses `$1`, `$2`. This function converts between the two.
///
/// Handles:
/// - `\1`, `\2`, etc. → `$1`, `$2`, etc. (numbered backreferences)
/// - `\\` → `\` (escaped backslash)
/// - `\&` → `$&` (entire match)
fn convert_sed_backreferences(replacement: &str) -> String {
    let mut result = String::with_capacity(replacement.len());
    let mut chars = replacement.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next_char) = chars.peek() {
                if next_char.is_ascii_digit() {
                    // Convert \1, \2, etc. to $1, $2, etc.
                    result.push('$');
                    chars.next(); // consume the digit
                    result.push(next_char);
                } else if next_char == '\\' {
                    // Escaped backslash - keep one
                    result.push('\\');
                    chars.next(); // consume second backslash
                    if let Some(&third) = chars.peek() {
                        chars.next();
                        result.push(third);
                    }
                } else if next_char == '&' {
                    // Matched string
                    result.push('$');
                    result.push('&');
                    chars.next();
                } else {
                    // Other escape sequence - keep both
                    result.push(c);
                    if let Some(next) = chars.next() {
                        result.push(next);
                    }
                }
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_substitution() {
        let cmd = parse_single_command("s/foo/bar/g").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: vec!['g'],
                range: None,
            }
        );
    }

    #[test]
    fn test_parse_line_substitution() {
        let cmd = parse_single_command("10s/foo/bar/").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: vec![],
                range: Some((
                    Address::LineNumber(10),
                    Address::LineNumber(10)
                )),
            }
        );
    }

    #[test]
    fn test_parse_range_substitution() {
        let cmd = parse_single_command("1,10s/foo/bar/").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: vec![],
                range: Some((
                    Address::LineNumber(1),
                    Address::LineNumber(10)
                )),
            }
        );
    }

    #[test]
    fn test_parse_delete_line() {
        let cmd = parse_single_command("10d").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Delete {
                range: (
                    Address::LineNumber(10),
                    Address::LineNumber(10)
                ),
            }
        );
    }

    #[test]
    fn test_parse_delete_range() {
        let cmd = parse_single_command("1,10d").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Delete {
                range: (
                    Address::LineNumber(1),
                    Address::LineNumber(10)
                ),
            }
        );
    }

    #[test]
    fn test_parse_delete_pattern() {
        let cmd = parse_single_command("/foo/d").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Delete {
                range: (
                    Address::Pattern("foo".to_string()),
                    Address::Pattern("foo".to_string())
                ),
            }
        );
    }

    #[test]
    fn test_parse_print_line() {
        let cmd = parse_single_command("10p").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Print {
                range: (
                    Address::LineNumber(10),
                    Address::LineNumber(10)
                ),
            }
        );
    }

    #[test]
    fn test_parse_print_range() {
        let cmd = parse_single_command("1,10p").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Print {
                range: (
                    Address::LineNumber(1),
                    Address::LineNumber(10)
                ),
            }
        );
    }

    // Bug 3: Backreference conversion tests
    #[test]
    fn test_backreference_conversion_single() {
        let result = convert_sed_backreferences(r"\1");
        assert_eq!(result, "$1");
    }

    #[test]
    fn test_backreference_conversion_multiple() {
        let result = convert_sed_backreferences(r"\1 \2 \3");
        assert_eq!(result, "$1 $2 $3");
    }

    #[test]
    fn test_backreference_conversion_mixed() {
        let result = convert_sed_backreferences(r"foo \1 bar \2 baz");
        assert_eq!(result, "foo $1 bar $2 baz");
    }

    #[test]
    fn test_backreference_conversion_escaped_backslash() {
        let result = convert_sed_backreferences(r"\\");
        assert_eq!(result, r"\");
    }

    #[test]
    fn test_backreference_conversion_ampersand() {
        let result = convert_sed_backreferences(r"\&");
        assert_eq!(result, "$&");
    }

    #[test]
    fn test_backreference_conversion_complex() {
        let result = convert_sed_backreferences(r"\1: \2 \\ \1");
        assert_eq!(result, r"$1: $2 \ $1");
    }

    // Bug 2: Command grouping tests
    #[test]
    fn test_parse_simple_group() {
        let cmd = parse_single_command("{s/foo/bar/}").unwrap();
        match cmd {
            SedCommand::Group { range, commands } => {
                assert_eq!(range, None);
                assert_eq!(commands.len(), 1);
            }
            _ => panic!("Expected Group command"),
        }
    }

    #[test]
    fn test_parse_group_with_semicolons() {
        let cmd = parse_single_command("{s/foo/bar/; s/baz/qux/}").unwrap();
        match cmd {
            SedCommand::Group { range, commands } => {
                assert_eq!(range, None);
                assert_eq!(commands.len(), 2);
            }
            _ => panic!("Expected Group command"),
        }
    }

    // Hold space command tests
    #[test]
    fn test_parse_hold_simple() {
        let cmd = parse_single_command("h").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Hold { range: None }
        );
    }

    #[test]
    fn test_parse_hold_with_address() {
        let cmd = parse_single_command("5h").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Hold {
                range: Some((
                    Address::LineNumber(5),
                    Address::LineNumber(5)
                ))
            }
        );
    }

    #[test]
    fn test_parse_hold_append_with_range() {
        let cmd = parse_single_command("1,5H").unwrap();
        assert_eq!(
            cmd,
            SedCommand::HoldAppend {
                range: Some((
                    Address::LineNumber(1),
                    Address::LineNumber(5)
                ))
            }
        );
    }

    #[test]
    fn test_parse_get_append() {
        let cmd = parse_single_command("$G").unwrap();
        assert_eq!(
            cmd,
            SedCommand::GetAppend {
                range: Some((
                    Address::LastLine,
                    Address::LastLine
                ))
            }
        );
    }

    #[test]
    fn test_parse_exchange_with_pattern() {
        let cmd = parse_single_command("/pattern/x").unwrap();
        match cmd {
            SedCommand::Exchange { range: Some((Address::Pattern(p), _)) } => {
                assert_eq!(p, "pattern");
            }
            _ => panic!("Expected Exchange command with pattern"),
        }
    }

    #[test]
    fn test_parse_get_with_negation() {
        let cmd = parse_single_command("/foo/!g").unwrap();
        match cmd {
            SedCommand::Get { range: Some((Address::Negated(_), _)) } => {
                // Success
            }
            _ => panic!("Expected Get command with negation"),
        }
    }

    #[test]
    fn test_parse_hold_range_with_patterns() {
        let cmd = parse_single_command("/start/,/end/H").unwrap();
        match cmd {
            SedCommand::HoldAppend { range: Some((Address::Pattern(s), Address::Pattern(e))) } => {
                assert_eq!(s, "start");
                assert_eq!(e, "end");
            }
            _ => panic!("Expected HoldAppend with pattern range"),
        }
    }

    #[test]
    fn test_parse_get_simple() {
        let cmd = parse_single_command("g").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Get { range: None }
        );
    }

    #[test]
    fn test_parse_exchange_simple() {
        let cmd = parse_single_command("x").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Exchange { range: None }
        );
    }
}
