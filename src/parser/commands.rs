use crate::command::{Address, Command};
use crate::parser::address::{parse_address, parse_optional_range};
use crate::parser::errors::format_parse_error;
use crate::parser::io::is_inside_pattern_address;
use anyhow::{Result, anyhow};

pub fn find_command_char(cmd: &str) -> Option<(usize, char)> {
    // Valid sed command characters
    const COMMAND_CHARS: &str = "sdpqQiac hHgGx nNP D:btT=FzrRwW{}";

    for (pos, c) in cmd.char_indices() {
        if COMMAND_CHARS.contains(c) {
            // Check if it's a command character or part of an address
            if !is_inside_pattern_address(cmd, pos) {
                // Potential command char.
                // But wait, it might be part of the address itself (like a digit or '$' or '~' or ',')!
                if c.is_ascii_digit()
                    || c == '$'
                    || c == '~'
                    || c == ','
                    || c == '+'
                    || c == '-'
                    || c == ' '
                {
                    continue;
                }

                // Special case for 's' - it must be followed by a delimiter
                if c == 's' {
                    let rest = &cmd[pos + 1..];
                    if !rest.is_empty() {
                        return Some((pos, 's'));
                    }
                    continue; // False alarm 's' (maybe in a filename)
                }

                return Some((pos, c));
            }
        }
    }
    None
}

pub fn parse_delete(cmd: &str) -> Result<Command> {
    let (pos, _) = find_command_char(cmd).ok_or_else(|| anyhow!("Delete command missing 'd'"))?;
    let addr_part = &cmd[..pos];

    if addr_part.trim().is_empty() {
        return Ok(Command::Delete {
            range: (Address::LineNumber(1), Address::LastLine),
        });
    }

    let range = parse_optional_range(addr_part)?;
    match range {
        None => Ok(Command::Delete {
            range: (Address::LineNumber(1), Address::LastLine),
        }),
        Some(r) => Ok(Command::Delete { range: r }),
    }
}

pub fn parse_print(cmd: &str) -> Result<Command> {
    let (pos, _) = find_command_char(cmd).ok_or_else(|| anyhow!("Print command missing 'p'"))?;
    let addr_part = &cmd[..pos];

    if addr_part.trim().is_empty() {
        return Ok(Command::Print {
            range: (Address::LineNumber(1), Address::LastLine),
        });
    }

    let range = parse_optional_range(addr_part)?;
    match range {
        None => Ok(Command::Print {
            range: (Address::LineNumber(1), Address::LastLine),
        }),
        Some(r) => Ok(Command::Print { range: r }),
    }
}

pub fn parse_quit(cmd: &str) -> Result<Command> {
    let (pos, _) = find_command_char(cmd).ok_or_else(|| anyhow!("Quit command missing 'q'"))?;
    let addr_part = &cmd[..pos];
    let address = if addr_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(addr_part.trim())?)
    };
    Ok(Command::Quit { address })
}

pub fn parse_quit_without_print(cmd: &str) -> Result<Command> {
    let (pos, _) =
        find_command_char(cmd).ok_or_else(|| anyhow!("Quit without print command missing 'Q'"))?;
    let addr_part = &cmd[..pos];
    let address = if addr_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(addr_part.trim())?)
    };
    Ok(Command::QuitWithoutPrint { address })
}

pub fn parse_insert(cmd: &str) -> Result<Command> {
    let parts: Vec<&str> = cmd.splitn(2, "i\\").collect();
    if parts.len() != 2 {
        let suggestion = if cmd.contains('i') && !cmd.contains("i\\") {
            Some(
                "Insert command requires a backslash after 'i':\n  Format: [address]i\\text\n  Example: 5i\\INSERTED LINE\n  Example: /pattern/i\\New line before match",
            )
        } else {
            None
        };
        return Err(anyhow!(
            "{}",
            format_parse_error(cmd, None, "invalid insert command format", suggestion)
        ));
    }

    let addr_part = parts[0].trim();
    let text = parts[1].strip_prefix('\n').unwrap_or(parts[1]);

    let address = if addr_part.is_empty() {
        Address::LineNumber(1)
    } else {
        parse_address(addr_part)?
    };

    Ok(Command::Insert {
        text: text.to_string(),
        address,
    })
}

pub fn parse_append(cmd: &str) -> Result<Command> {
    let parts: Vec<&str> = cmd.splitn(2, "a\\").collect();
    if parts.len() != 2 {
        let suggestion = if cmd.contains('a') && !cmd.contains("a\\") {
            Some(
                "Append command requires a backslash after 'a':\n  Format: [address]a\\text\n  Example: 5a\\APPENDED LINE\n  Example: /pattern/a\\New line after match",
            )
        } else {
            None
        };
        return Err(anyhow!(
            "{}",
            format_parse_error(cmd, None, "invalid append command format", suggestion)
        ));
    }

    let addr_part = parts[0].trim();
    let text = parts[1].strip_prefix('\n').unwrap_or(parts[1]);

    let address = if addr_part.is_empty() {
        Address::LastLine
    } else {
        parse_address(addr_part)?
    };

    Ok(Command::Append {
        text: text.to_string(),
        address,
    })
}

pub fn parse_change(cmd: &str) -> Result<Command> {
    let parts: Vec<&str> = cmd.splitn(2, "c\\").collect();
    if parts.len() != 2 {
        let suggestion = if cmd.contains('c') && !cmd.contains("c\\") {
            Some(
                "Change command requires a backslash after 'c':\n  Format: [address]c\\text\n  Example: 5,10c\\REPLACED LINES\n  Example: /pattern/c\\Line replacement",
            )
        } else {
            None
        };
        return Err(anyhow!(
            "{}",
            format_parse_error(cmd, None, "invalid change command format", suggestion)
        ));
    }

    let addr_part = parts[0].trim();
    let text = parts[1].strip_prefix('\n').unwrap_or(parts[1]);

    let range = parse_optional_range(addr_part)?;
    let range = range.unwrap_or((Address::LineNumber(1), Address::LastLine));

    Ok(Command::Change {
        text: text.to_string(),
        range,
    })
}

pub fn parse_hold(cmd: &str) -> Result<Command> {
    let (pos, _) = find_command_char(cmd).ok_or_else(|| anyhow!("Hold command missing 'h'"))?;
    let addr_part = &cmd[..pos];
    let range = parse_optional_range(addr_part)?;
    Ok(Command::Hold { range })
}

pub fn parse_hold_append(cmd: &str) -> Result<Command> {
    let (pos, _) =
        find_command_char(cmd).ok_or_else(|| anyhow!("Hold append command missing 'H'"))?;
    let addr_part = &cmd[..pos];
    let range = parse_optional_range(addr_part)?;
    Ok(Command::HoldAppend { range })
}

pub fn parse_get(cmd: &str) -> Result<Command> {
    let (pos, _) = find_command_char(cmd).ok_or_else(|| anyhow!("Get command missing 'g'"))?;
    let addr_part = &cmd[..pos];
    let range = parse_optional_range(addr_part)?;
    Ok(Command::Get { range })
}

pub fn parse_get_append(cmd: &str) -> Result<Command> {
    let (pos, _) =
        find_command_char(cmd).ok_or_else(|| anyhow!("Get append command missing 'G'"))?;
    let addr_part = &cmd[..pos];
    let range = parse_optional_range(addr_part)?;
    Ok(Command::GetAppend { range })
}

pub fn parse_exchange(cmd: &str) -> Result<Command> {
    let (pos, _) = find_command_char(cmd).ok_or_else(|| anyhow!("Exchange command missing 'x'"))?;
    let addr_part = &cmd[..pos];
    let range = parse_optional_range(addr_part)?;
    Ok(Command::Exchange { range })
}

pub fn parse_next(cmd: &str) -> Result<Command> {
    let (pos, _) = find_command_char(cmd).ok_or_else(|| anyhow!("Next command missing 'n'"))?;
    let addr_part = &cmd[..pos];
    let range = parse_optional_range(addr_part)?;
    Ok(Command::Next { range })
}

pub fn parse_next_append(cmd: &str) -> Result<Command> {
    let (pos, _) =
        find_command_char(cmd).ok_or_else(|| anyhow!("Next append command missing 'N'"))?;
    let addr_part = &cmd[..pos];
    let range = parse_optional_range(addr_part)?;
    Ok(Command::NextAppend { range })
}

pub fn parse_print_first_line(cmd: &str) -> Result<Command> {
    let (pos, _) =
        find_command_char(cmd).ok_or_else(|| anyhow!("Print first line command missing 'P'"))?;
    let addr_part = &cmd[..pos];
    let range = parse_optional_range(addr_part)?;
    Ok(Command::PrintFirstLine { range })
}

pub fn parse_delete_first_line(cmd: &str) -> Result<Command> {
    let (pos, _) =
        find_command_char(cmd).ok_or_else(|| anyhow!("Delete first line command missing 'D'"))?;
    let addr_part = &cmd[..pos];
    let range = parse_optional_range(addr_part)?;
    Ok(Command::DeleteFirstLine { range })
}

pub fn parse_print_line_number(cmd: &str) -> Result<Command> {
    let (pos, _) =
        find_command_char(cmd).ok_or_else(|| anyhow!("Print line number command missing '='"))?;
    let address_part = &cmd[..pos];
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };
    Ok(Command::PrintLineNumber { range })
}

pub fn parse_print_filename(cmd: &str) -> Result<Command> {
    let (pos, _) =
        find_command_char(cmd).ok_or_else(|| anyhow!("Print filename command missing 'F'"))?;
    let address_part = &cmd[..pos];
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };
    Ok(Command::PrintFilename { range })
}

pub fn parse_clear_pattern_space(cmd: &str) -> Result<Command> {
    let (pos, _) =
        find_command_char(cmd).ok_or_else(|| anyhow!("Clear pattern space command missing 'z'"))?;
    let address_part = &cmd[..pos];
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };
    Ok(Command::ClearPatternSpace { range })
}
