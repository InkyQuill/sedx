use anyhow::{Result, anyhow, bail};

/// Maximum context characters to show around error position
const ERROR_CONTEXT_SIZE: usize = 30;

/// Helper function to extract context around an error position
fn extract_context(full_text: &str, pos: usize) -> String {
    let start = pos.saturating_sub(ERROR_CONTEXT_SIZE);
    let end = if pos + ERROR_CONTEXT_SIZE < full_text.len() {
        pos + ERROR_CONTEXT_SIZE
    } else {
        full_text.len()
    };

    let mut context = full_text[start..end].to_string();
    if start > 0 {
        context.insert_str(0, "...");
    }
    if end < full_text.len() {
        context.push_str("...");
    }
    context
}

/// Format an error with context and suggestions
fn format_parse_error(
    expression: &str,
    error_pos: Option<usize>,
    description: &str,
    suggestion: Option<&str>,
) -> String {
    let mut msg = format!("Parse error: {}", description);

    if let Some(pos) = error_pos {
        let context = extract_context(expression, pos);
        msg.push_str(&format!("\n  Near: \"{}\"", context));
    }

    if let Some(hint) = suggestion {
        msg.push_str(&format!("\n  Hint: {}", hint));
    }

    msg
}

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
    // Phase 4: Quit without printing
    QuitWithoutPrint {
        address: Option<Address>, // Q or 10Q or /pattern/Q
    },
    Group {
        range: Option<(Address, Address)>, // Optional range for the group
        commands: Vec<SedCommand>,         // Commands to execute
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
    // Phase 5: Flow control commands
    Label {
        name: String, // :label - defines a branch target
    },
    Branch {
        label: Option<String>, // b [label] - branch to label (end of script if None)
        range: Option<(Address, Address)>, // Optional address/range for branch
    },
    Test {
        label: Option<String>,             // t [label] - branch if substitution made
        range: Option<(Address, Address)>, // Optional address/range for test
    },
    TestFalse {
        label: Option<String>,             // T [label] - branch if NO substitution made
        range: Option<(Address, Address)>, // Optional address/range for test false
    },
    // Phase 5: File I/O commands
    ReadFile {
        filename: String,       // r filename - read file and append contents
        range: Option<Address>, // Optional address for read
    },
    WriteFile {
        filename: String,       // w filename - write pattern space to file
        range: Option<Address>, // Optional address for write
    },
    ReadLine {
        filename: String,       // R filename - read one line from file
        range: Option<Address>, // Optional address for read
    },
    WriteFirstLine {
        filename: String,       // W filename - write first line to file
        range: Option<Address>, // Optional address for write
    },
    // Phase 5: Additional commands
    PrintLineNumber {
        range: Option<Address>, // = - print line number (optional address)
    },
    PrintFilename {
        range: Option<Address>, // F - print filename (optional address)
    },
    ClearPatternSpace {
        range: Option<Address>, // z - clear pattern space (optional address)
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Address {
    LineNumber(usize),
    Pattern(String),
    FirstLine,             // Special address "0" for first-match substitution
    LastLine,              // Special address "$" for last line
    Negated(Box<Address>), // Negation: !/pattern/ or !10
    // Chunk 8: New address types
    Relative { base: Box<Address>, offset: isize }, // /pattern/,+5 or 10,+3
    Step { start: usize, step: usize },             // 1~2 (every 2nd line from line 1)
}

pub fn parse_sed_expression(expr: &str) -> Result<Vec<SedCommand>> {
    let mut commands = Vec::new();

    // Handle multiple expressions separated by ;
    // But skip semicolons inside braces { ... }
    let mut current_expr = String::new();
    let mut in_braces = 0;
    let chars = expr.chars().peekable();

    for c in chars {
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

/// Helper function to check if a position is inside a pattern address
/// Pattern addresses are delimited by '/' or '\', e.g., /pattern/ or \pattern\
/// Returns true if the position is inside the delimiters (not at the delimiters themselves)
fn is_inside_pattern_address(cmd: &str, pos: usize) -> bool {
    let bytes = cmd.as_bytes();
    let n = bytes.len();

    // We need to count delimiter pairs before the position
    // Each pair consists of an opening delimiter and its matching closing delimiter
    // We're "inside" if we've seen an odd number of opening delimiters before this position

    // For simplicity, let's just look for the pattern: /.../ where pos is between the slashes
    // We need to find the LAST '/' BEFORE pos and check if it has a matching '/' AFTER pos

    // Find the last '/' or '\' before pos
    let mut last_delim_before = None;
    for i in (0..pos).rev() {
        if bytes[i] == b'/' || bytes[i] == b'\\' {
            last_delim_before = Some(i);
            break;
        }
    }

    let start_pos = match last_delim_before {
        Some(sp) => sp,
        None => return false, // No delimiter before pos, so we're not inside
    };

    // Look for the NEXT '/' or '\' after start_pos
    for i in (start_pos + 1)..n {
        if bytes[i] == bytes[start_pos] {
            // Same delimiter character
            // Found matching closing delimiter
            // Check if pos is between the delimiters
            return pos > start_pos && pos < i;
        }
    }

    // No matching closing delimiter found
    // Assume we're NOT inside (unclosed pattern)
    false
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

    // Phase 5: Check for flow control commands BEFORE other commands
    // because b/t/T may have labels after them (not at the end)
    if cmd.starts_with(':') {
        // Label definition (Phase 5)
        return parse_label(cmd);
    }

    // Check for b/t/T commands anywhere in the command
    // Examples: "b", "b label", "10b", "10b label", "/pat/b label"
    let trimmed = cmd.trim();
    if trimmed.contains('b') || trimmed.contains('t') || trimmed.contains('T') {
        // Verify it's actually a flow control command by checking the position
        // For "b", "b label", "10b", "10b label" - the b/t/T should be followed by space or end of string
        let b_pos = trimmed.find('b');
        let t_pos = trimmed.find('t');
        let t_upper_pos = trimmed.find('T');

        // Find which position comes first
        let min_pos = [b_pos, t_pos, t_upper_pos].iter().filter_map(|&p| p).min();

        if let Some(pos) = min_pos {
            let char_at_pos = trimmed
                .chars()
                .nth(pos)
                .ok_or_else(|| anyhow!("Invalid position {} in command: {}", pos, cmd))?;
            let rest = &trimmed[pos + 1..];

            // Check if after b/t/T there's only whitespace, label, or end of string
            if rest.trim().is_empty() || rest.starts_with(' ') {
                // Definitely flow control
                if char_at_pos == 'b' {
                    return parse_branch(cmd);
                } else if char_at_pos == 't' {
                    return parse_test(cmd);
                } else {
                    return parse_test_false(cmd);
                }
            }
        }
    }

    // Phase 5: Check for file I/O and additional commands
    // These commands (=, F, z, r, R, w, W) have filenames or are standalone
    // so we check for them BEFORE checking if command "ends with" certain characters
    if trimmed.contains('=') {
        // Print line number (=) - may have address before it
        // Examples: "=", "5=", "/pat/="
        // The = should be the last character (except for optional address before it)
        if let Some(eq_pos) = trimmed.find('=') {
            let rest = &trimmed[eq_pos + 1..];
            if rest.trim().is_empty() {
                // Valid = command (nothing after = except maybe whitespace)
                return parse_print_line_number(cmd);
            }
        }
    }

    if trimmed.contains('F') {
        // Print filename (F) - GNU sed extension
        // Examples: "F", "5F", "/pat/F"
        if let Some(f_pos) = trimmed.find('F') {
            let rest = &trimmed[f_pos + 1..];
            if rest.trim().is_empty() {
                // Valid F command (nothing after F except maybe whitespace)
                return parse_print_filename(cmd);
            }
        }
    }

    if trimmed.contains('z') {
        // Clear pattern space (z) - GNU sed extension
        // Examples: "z", "5z", "/pat/z"
        // Make sure it's not part of a substitution
        if !cmd.starts_with('s')
            && cmd.chars().filter(|&c| c == 's').count() <= 1
            && let Some(z_pos) = trimmed.find('z')
        {
            let rest = &trimmed[z_pos + 1..];
            if rest.trim().is_empty() {
                // Valid z command (nothing after z except maybe whitespace)
                return parse_clear_pattern_space(cmd);
            }
        }
    }

    // IMPORTANT: Check for insert/append/change commands BEFORE file I/O
    // because i\a\c commands use backslash followed by text, and the text may
    // contain letters like 'r', 'R', 'w', 'W' that would be misidentified as file I/O
    if cmd.contains("i\\") {
        // Insert command: addr i\text
        return parse_insert(cmd);
    }
    if cmd.contains("a\\") {
        // Append command: addr a\text
        return parse_append(cmd);
    }
    if cmd.contains("c\\") {
        // Change command: addr c\text
        return parse_change(cmd);
    }

    // Check for r/R/w/W commands (file I/O) - AFTER i/a/c checks
    // Examples: "r /path/file", "5r file.txt", "/pat/r file"
    // These commands have filenames after them, so they don't "end with" the command char
    // IMPORTANT: Must check that the command char is NOT part of a pattern address like /pat/
    // Pattern addresses are delimited by forward slashes, so we skip r/R/w/W inside /.../
    if trimmed.contains('r')
        || trimmed.contains('R')
        || trimmed.contains('w')
        || trimmed.contains('W')
    {
        // Find all positions of each command character
        let mut r_positions: Vec<usize> = trimmed.match_indices('r').map(|(i, _)| i).collect();
        let mut r_upper_positions: Vec<usize> =
            trimmed.match_indices('R').map(|(i, _)| i).collect();
        let mut w_positions: Vec<usize> = trimmed.match_indices('w').map(|(i, _)| i).collect();
        let mut w_upper_positions: Vec<usize> =
            trimmed.match_indices('W').map(|(i, _)| i).collect();

        // Filter out positions that are inside pattern addresses (between '/' characters)
        // Pattern addresses have the form /pattern/ or \pattern\
        r_positions.retain(|&pos| !is_inside_pattern_address(trimmed, pos));
        r_upper_positions.retain(|&pos| !is_inside_pattern_address(trimmed, pos));
        w_positions.retain(|&pos| !is_inside_pattern_address(trimmed, pos));
        w_upper_positions.retain(|&pos| !is_inside_pattern_address(trimmed, pos));

        // Find which position comes first among the remaining (non-pattern) positions
        let all_positions: Vec<(usize, char)> = r_positions
            .into_iter()
            .map(|p| (p, 'r'))
            .chain(r_upper_positions.into_iter().map(|p| (p, 'R')))
            .chain(w_positions.into_iter().map(|p| (p, 'w')))
            .chain(w_upper_positions.into_iter().map(|p| (p, 'W')))
            .collect();

        if let Some(&(pos, char_at_pos)) = all_positions.iter().min_by_key(|(p, _)| p) {
            let rest = &trimmed[pos + 1..];

            // After the command char, there should be a filename (possibly with spaces)
            // The filename can be anything, so if there's content after, it's likely a file I/O command
            if !rest.trim().is_empty() {
                // Has content after command char - likely filename
                return match char_at_pos {
                    'r' => parse_read_file(cmd),
                    'R' => parse_read_line(cmd),
                    'w' => parse_write_file(cmd),
                    'W' => parse_write_first_line(cmd),
                    _ => unreachable!(),
                };
            }
        }
    }

    // Determine command type by looking at the last character or special patterns
    if cmd.ends_with('Q') && !cmd.starts_with('s') {
        // Quit without printing command (Phase 4)
        parse_quit_without_print(cmd)
    } else if cmd.ends_with('q') && !cmd.starts_with('s') {
        // Quit command
        parse_quit(cmd)
    } else if cmd.ends_with('d') {
        // Delete command
        parse_delete(cmd)
    } else if cmd.ends_with('p') && !cmd.starts_with('s') {
        // Print command (but not s/pattern/replacement/p which is a flag)
        parse_print(cmd)
    } else {
        // Try to determine by last character for other commands
        let command_char = cmd.chars().last().ok_or_else(|| anyhow!("Empty command"))?;

        match command_char {
            's' => parse_substitution(cmd),
            'Q' => parse_quit_without_print(cmd),
            'q' => parse_quit(cmd),
            'd' => parse_delete(cmd),
            'p' => parse_print(cmd),
            'h' => parse_hold(cmd),
            'H' => parse_hold_append(cmd),
            'g' => parse_get(cmd),
            'G' => parse_get_append(cmd),
            'x' => parse_exchange(cmd),
            'n' => parse_next(cmd),
            'N' => parse_next_append(cmd),
            'P' => parse_print_first_line(cmd),
            'D' => parse_delete_first_line(cmd),
            'r' => parse_read_file(cmd),
            'R' => parse_read_line(cmd),
            'w' => parse_write_file(cmd),
            'W' => parse_write_first_line(cmd),
            '=' => parse_print_line_number(cmd),
            'F' => parse_print_filename(cmd),
            'z' => parse_clear_pattern_space(cmd),
            _ => {
                let unknown_char = command_char;
                let suggestion = match unknown_char {
                    c if c.is_ascii_alphabetic() => {
                        "Did you mean:\n\
                             - Substitution: s/pattern/replacement/[flags]\n\
                             - Delete: d\n\
                             - Print: p\n\
                             - Insert (before line): 5i\\text\n\
                             - Append (after line): 5a\\text\n\
                             - Change line: 5c\\new text\n\
                             - Quit: q or Q\n\
                             See 'sedx --help' for all commands".to_string()
                    }
                    '0'..='9' => {
                        "Numbers alone are not commands. Use a command character after the line number (e.g., '5d' to delete line 5)".to_string()
                    }
                    _ => {
                        "Valid commands: s (substitute), d (delete), p (print),\n\
                             i (insert), a (append), c (change), q (quit),\n\
                             h/H (hold), g/G (get), x (exchange), n/N (next),\n\
                             b/t/T (branch), r/R (read file), w/W (write file),\n\
                             = (line number), F (filename), z (clear pattern space)".to_string()
                    }
                };

                let cmd_trimmed = cmd.trim();
                Err(anyhow!(
                    "{}",
                    format_parse_error(
                        cmd_trimmed,
                        Some(cmd_trimmed.chars().count().saturating_sub(1)),
                        &format!("unknown command '{}'", unknown_char),
                        Some(&suggestion),
                    )
                ))
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

    let s_pos = s_pos.ok_or_else(|| anyhow!("{}", format_parse_error(
        cmd,
        None,
        "'s' command not followed by a valid delimiter",
        Some("Substitution format: s<delimiter>pattern<delimiter>replacement<delimiter>[flags]\nDelimiters: / (slash), # (hash), : (colon), | (pipe)\nExample: s/foo/bar/ or s#old#new#g"),
    )))?;

    // Everything before 's' is the address/range
    let address_part = &cmd[..s_pos];
    let rest = &cmd[s_pos + 1..]; // Skip the 's'

    // Detect delimiter
    let delimiter = rest.chars().next()
        .ok_or_else(|| anyhow!("{}", format_parse_error(
            cmd,
            Some(s_pos + 1),
            "missing delimiter after 's'",
            Some("Expected format: s<delimiter>pattern<delimiter>replacement<delimiter>[flags]\nExample: s/foo/bar/ or s#old#new#g"),
        )))?;

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
                    "no '{}' delimiter found after the opening delimiter",
                    delimiter
                ),
                Some(
                    "Make sure to close the pattern, replacement, and optionally add flags:\n  s/pattern/replacement/\n  s/pattern/replacement/g",
                ),
            ),
            1 => (
                "missing closing delimiter for replacement".to_string(),
                Some(
                    "You need to close the replacement with the delimiter:\n  s/pattern/replacement/\n                      ^ (add this)",
                ),
            ),
            2 => {
                // This is actually valid - no flags, just 2 delimiters for pattern+replacement
                // But our code expects 3 positions (including the closing delimiter)
                // Wait, delimiter_positions tracks the delimiter positions
                // So: s/pattern/replacement/ has 3 delimiters (positions of / / /)
                // If we only have 2, we're missing the final delimiter
                (
                    "missing final delimiter to close the substitution".to_string(),
                    Some(
                        "Add the final delimiter:\n  s/pattern/replacement/\n                        ^ (add this)",
                    ),
                )
            }
            _ => unreachable!(),
        };

        return Err(anyhow!(
            "{}",
            format_parse_error(cmd, None, &description, suggestion,)
        ));
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
                let offset_str = &end_str[1..]; // Skip +/-
                let offset: isize = offset_str.parse()
                    .map_err(|_| anyhow!("{}", format_parse_error(
                        cmd,
                        None,
                        &format!("invalid relative offset '{}'", end_str),
                        Some("Relative offset format: start,+N or start,-N\nExample: /pattern/,+5  - 5 lines after pattern match\n         10,-3       - 3 lines before line 10"),
                    )))?;

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

    // Empty address means delete all lines (1 to $)
    if addr_part.trim().is_empty() {
        return Ok(SedCommand::Delete {
            range: (Address::LineNumber(1), Address::LastLine),
        });
    }

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
        return Ok(SedCommand::Quit { address: None });
    }

    // '10q' or '/pattern/q' - quit at that address
    let addr = parse_address(addr_part)?;
    Ok(SedCommand::Quit {
        address: Some(addr),
    })
}

// Phase 4: Parse Q command (quit without printing)
fn parse_quit_without_print(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();
    let addr_part = &cmd[..cmd.len() - 1]; // Remove 'Q'

    // Check if there's an address
    if addr_part.trim().is_empty() {
        // Just 'Q' - quit immediately without printing
        return Ok(SedCommand::QuitWithoutPrint { address: None });
    }

    // '10Q' or '/pattern/Q' - quit at that address without printing
    let addr = parse_address(addr_part)?;
    Ok(SedCommand::QuitWithoutPrint {
        address: Some(addr),
    })
}

fn parse_group(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the opening brace
    let open_brace = cmd.find('{')
        .ok_or_else(|| anyhow!("{}", format_parse_error(
            cmd,
            None,
            "group command is missing opening '{'",
            Some("Group format: [range] { command1; command2; ... }\nExample: {s/foo/bar/; s/baz/qux/}\n         1,10{s/^/> /}"),
        )))?;

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
        .ok_or_else(|| anyhow!("{}", format_parse_error(
            cmd,
            None,
            "group command is missing closing '}'",
            Some("Every opening '{' must have a matching closing '}'.\nExample: {s/foo/bar/; p}\n                      ^ (add closing brace here)"),
        )))?;

    // Extract commands inside the braces
    let commands_str = &cmd[brace_start..close_brace].trim();

    // Parse the range if present
    let range = if addr_part.is_empty() {
        None
    } else if addr_part.contains(',') {
        // Range: start,end{...}
        let parts: Vec<&str> = addr_part.splitn(2, ',').collect();
        if parts.len() == 2 {
            Some((
                parse_address(parts[0].trim())?,
                parse_address(parts[1].trim())?,
            ))
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
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "empty group: no commands inside braces",
                Some(
                    "Add commands separated by semicolons:\n  {s/foo/bar/; p}  - valid\n  {}                - invalid (empty)"
                ),
            )
        ));
    }

    Ok(SedCommand::Group { range, commands })
}

fn parse_insert(cmd: &str) -> Result<SedCommand> {
    // Insert: i\text or addr i\text
    let parts: Vec<&str> = cmd.splitn(2, "i\\").collect();
    if parts.len() != 2 {
        // Check if it looks like user forgot the backslash
        let suggestion = if cmd.contains('i') && !cmd.contains("i\\") {
            Some(
                "Insert command requires a backslash after 'i':\n  Format: [address]i\\text\n  Example: 5i\\INSERTED LINE\n  Example: /pattern/i\\New line before match",
            )
        } else {
            Some("Valid insert format: [address]i\\text\nExample: 5i\\INSERTED LINE")
        };
        return Err(anyhow!(
            "{}",
            format_parse_error(cmd, None, "invalid insert command syntax", suggestion,)
        ));
    }

    let address = if !parts[0].trim().is_empty() {
        parse_address(parts[0].trim())?
    } else {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "insert command requires an address",
                Some(
                    "Specify which line to insert before:\n  5i\\text        - insert before line 5\n  /pat/i\\text     - insert before lines matching 'pat'\n  $i\\text        - insert before last line"
                ),
            )
        ));
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
        let suggestion = if cmd.contains('a') && !cmd.contains("a\\") {
            Some(
                "Append command requires a backslash after 'a':\n  Format: [address]a\\text\n  Example: 5a\\APPENDED LINE\n  Example: /pattern/a\\New line after match",
            )
        } else {
            Some("Valid append format: [address]a\\text\nExample: 5a\\APPENDED LINE")
        };
        return Err(anyhow!(
            "{}",
            format_parse_error(cmd, None, "invalid append command syntax", suggestion,)
        ));
    }

    let address = if !parts[0].trim().is_empty() {
        parse_address(parts[0].trim())?
    } else {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "append command requires an address",
                Some(
                    "Specify which line to append after:\n  5a\\text        - append after line 5\n  /pat/a\\text     - append after lines matching 'pat'\n  $a\\text        - append after last line"
                ),
            )
        ));
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
        let suggestion = if cmd.contains('c') && !cmd.contains("c\\") {
            Some(
                "Change command requires a backslash after 'c':\n  Format: [address]c\\text\n  Example: 5c\\NEW LINE\n  Example: /pattern/c\\REPLACED TEXT",
            )
        } else {
            Some("Valid change format: [address]c\\text\nExample: 5c\\NEW LINE")
        };
        return Err(anyhow!(
            "{}",
            format_parse_error(cmd, None, "invalid change command syntax", suggestion,)
        ));
    }

    let address = if !parts[0].trim().is_empty() {
        parse_address(parts[0].trim())?
    } else {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "change command requires an address",
                Some(
                    "Specify which line(s) to change:\n  5c\\text        - change line 5\n  /pat/c\\text     - change lines matching 'pat'\n  1,10c\\text     - change lines 1-10 to 'text'"
                ),
            )
        ));
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
            let offset_str = &end[1..]; // Skip +/-
            let offset: isize = offset_str.parse()
                .map_err(|_| anyhow!("{}", format_parse_error(
                    end,
                    None,
                    &format!("invalid relative offset '{}'", end),
                    Some("Relative offset format: start,+N or start,-N\nExample: /pattern/,+5  - 5 lines after pattern\n         10,-3       - 3 lines before line 10"),
                )))?;

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

        let start: usize = start_str.parse()
            .map_err(|_| anyhow!("{}", format_parse_error(
                addr,
                Some(tilde_pos),
                &format!("invalid step start '{}'", start_str),
                Some("Step format: start~step\nExample: 1~2  - every 2nd line starting from line 1\n         10~5 - every 5th line starting from line 10"),
            )))?;
        let step: usize = step_str.parse()
            .map_err(|_| anyhow!("{}", format_parse_error(
                addr,
                Some(tilde_pos + 1),
                &format!("invalid step value '{}'", step_str),
                Some("Step format: start~step\nThe step value must be a positive integer.\nExample: 1~2 or 10~5"),
            )))?;

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

// Phase 5: Parse label definition (:label)
fn parse_label(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Remove the leading ':'
    let label_name = cmd[1..].trim();

    if label_name.is_empty() {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                Some(1),
                "label name cannot be empty",
                Some(
                    "Label definition format: :labelname\nExample: :loop\n         :end\n         :retry\nNote: Label names are limited to 8 characters (GNU sed compatibility)"
                ),
            )
        ));
    }

    // GNU sed restricts label names to max 8 characters
    if label_name.len() > 8 {
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                &format!("label name '{}' is too long (max 8 characters)", label_name),
                Some(&format!(
                    "Shorten the label name to 8 characters or less.\nSuggestion: {} (truncated)",
                    &label_name[..8]
                )),
            )
        ));
    }

    Ok(SedCommand::Label {
        name: label_name.to_string(),
    })
}

// Phase 5: Parse branch command (b [label])
fn parse_branch(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the 'b' command character
    let b_pos = cmd
        .find('b')
        .ok_or_else(|| anyhow!("Branch command missing 'b'"))?;

    // Split into: address_part (before 'b') and rest_part (after 'b' including 'b')
    let address_part = &cmd[..b_pos];
    let rest_part = &cmd[b_pos..]; // Includes the 'b'

    // Parse the optional range from address_part
    let range = parse_optional_range(address_part)?;

    // Extract label if present (after the 'b')
    let label_part = &rest_part[1..]; // Skip the 'b'
    let label = if label_part.trim().is_empty() {
        // Just 'b' - branch to end of script
        None
    } else {
        // 'b label' or '10b label'
        let label_name = label_part.trim();
        if !label_name.is_empty() {
            Some(label_name.to_string())
        } else {
            None
        }
    };

    Ok(SedCommand::Branch { label, range })
}

// Phase 5: Parse test branch command (t [label])
fn parse_test(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the 't' command character
    let t_pos = cmd
        .find('t')
        .ok_or_else(|| anyhow!("Test command missing 't'"))?;

    // Split into: address_part (before 't') and rest_part (after 't' including 't')
    let address_part = &cmd[..t_pos];
    let rest_part = &cmd[t_pos..]; // Includes the 't'

    // Parse the optional range from address_part
    let range = parse_optional_range(address_part)?;

    // Extract label if present (after the 't')
    let label_part = &rest_part[1..]; // Skip the 't'
    let label = if label_part.trim().is_empty() {
        None
    } else {
        let label_name = label_part.trim();
        if !label_name.is_empty() {
            Some(label_name.to_string())
        } else {
            None
        }
    };

    Ok(SedCommand::Test { label, range })
}

// Phase 5: Parse test false branch command (T [label])
fn parse_test_false(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the 'T' command character
    let t_pos = cmd
        .find('T')
        .ok_or_else(|| anyhow!("Test false command missing 'T'"))?;

    // Split into: address_part (before 'T') and rest_part (after 'T' including 'T')
    let address_part = &cmd[..t_pos];
    let rest_part = &cmd[t_pos..]; // Includes the 'T'

    // Parse the optional range from address_part
    let range = parse_optional_range(address_part)?;

    // Extract label if present (after the 'T')
    let label_part = &rest_part[1..]; // Skip the 'T'
    let label = if label_part.trim().is_empty() {
        None
    } else {
        let label_name = label_part.trim();
        if !label_name.is_empty() {
            Some(label_name.to_string())
        } else {
            None
        }
    };

    Ok(SedCommand::TestFalse { label, range })
}

// Phase 5: Parse read file command (r filename)
fn parse_read_file(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the 'r' command character
    let r_pos = cmd
        .find('r')
        .ok_or_else(|| anyhow!("Read file command missing 'r'"))?;

    // Split into: address_part (before 'r') and rest_part (after 'r' including 'r')
    let address_part = &cmd[..r_pos];
    let rest_part = &cmd[r_pos..]; // Includes the 'r'

    // Parse the optional address from address_part
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };

    // Extract filename (after the 'r')
    let filename_part = &rest_part[1..]; // Skip the 'r'
    let filename = filename_part.trim();
    if filename.is_empty() {
        bail!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "read file command requires a filename",
                Some(
                    "Read file format: [address]r filename\nExample: 5r header.txt    - insert contents of header.txt after line 5\n         /pat/r data.txt  - insert contents after lines matching 'pat'"
                ),
            )
        );
    }

    Ok(SedCommand::ReadFile {
        filename: filename.to_string(),
        range,
    })
}

// Phase 5: Parse write file command (w filename)
fn parse_write_file(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the 'w' command character
    let w_pos = cmd
        .find('w')
        .ok_or_else(|| anyhow!("Write file command missing 'w'"))?;

    // Split into: address_part (before 'w') and rest_part (after 'w' including 'w')
    let address_part = &cmd[..w_pos];
    let rest_part = &cmd[w_pos..]; // Includes the 'w'

    // Parse the optional address from address_part
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };

    // Extract filename (after the 'w')
    let filename_part = &rest_part[1..]; // Skip the 'w'
    let filename = filename_part.trim();
    if filename.is_empty() {
        bail!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "write file command requires a filename",
                Some(
                    "Write file format: [address]w filename\nExample: 5w output.txt    - write line 5 to output.txt\n         /pat/w log.txt    - write matching lines to log.txt"
                ),
            )
        );
    }

    Ok(SedCommand::WriteFile {
        filename: filename.to_string(),
        range,
    })
}

// Phase 5: Parse read line command (R filename)
fn parse_read_line(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the 'R' command character
    let r_pos = cmd
        .find('R')
        .ok_or_else(|| anyhow!("Read line command missing 'R'"))?;

    // Split into: address_part (before 'R') and rest_part (after 'R' including 'R')
    let address_part = &cmd[..r_pos];
    let rest_part = &cmd[r_pos..]; // Includes the 'R'

    // Parse the optional address from address_part
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };

    // Extract filename (after the 'R')
    let filename_part = &rest_part[1..]; // Skip the 'R'
    let filename = filename_part.trim();
    if filename.is_empty() {
        bail!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "read line command requires a filename",
                Some(
                    "Read line format: [address]R filename\nExample: 5R data.txt       - append one line from data.txt after line 5\n         /pat/R input.txt  - append one line after matching lines"
                ),
            )
        );
    }

    Ok(SedCommand::ReadLine {
        filename: filename.to_string(),
        range,
    })
}

// Phase 5: Parse write first line command (W filename)
fn parse_write_first_line(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the 'W' command character
    let w_pos = cmd
        .find('W')
        .ok_or_else(|| anyhow!("Write first line command missing 'W'"))?;

    // Split into: address_part (before 'W') and rest_part (after 'W' including 'W')
    let address_part = &cmd[..w_pos];
    let rest_part = &cmd[w_pos..]; // Includes the 'W'

    // Parse the optional address from address_part
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };

    // Extract filename (after the 'W')
    let filename_part = &rest_part[1..]; // Skip the 'W'
    let filename = filename_part.trim();
    if filename.is_empty() {
        bail!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "write first line command requires a filename",
                Some(
                    "Write first line format: [address]W filename\nExample: 5W output.txt    - write first line of pattern space to output.txt\n         /pat/W log.txt   - write first line to log.txt for matches"
                ),
            )
        );
    }

    Ok(SedCommand::WriteFirstLine {
        filename: filename.to_string(),
        range,
    })
}

// Phase 5: Parse print line number command (=)
fn parse_print_line_number(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the '=' command character
    let eq_pos = cmd
        .find('=')
        .ok_or_else(|| anyhow!("Print line number command missing '='"))?;

    // Split into: address_part (before '=') and the rest
    let address_part = &cmd[..eq_pos];

    // Parse the optional address from address_part
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };

    Ok(SedCommand::PrintLineNumber { range })
}

// Phase 5: Parse print filename command (F)
fn parse_print_filename(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the 'F' command character
    let f_pos = cmd
        .find('F')
        .ok_or_else(|| anyhow!("Print filename command missing 'F'"))?;

    // Split into: address_part (before 'F') and the rest
    let address_part = &cmd[..f_pos];

    // Parse the optional address from address_part
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };

    Ok(SedCommand::PrintFilename { range })
}

// Phase 5: Parse clear pattern space command (z)
fn parse_clear_pattern_space(cmd: &str) -> Result<SedCommand> {
    let cmd = cmd.trim();

    // Find the 'z' command character
    let z_pos = cmd
        .find('z')
        .ok_or_else(|| anyhow!("Clear pattern space command missing 'z'"))?;

    // Split into: address_part (before 'z') and the rest
    let address_part = &cmd[..z_pos];

    // Parse the optional address from address_part
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_address(address_part.trim())?)
    };

    Ok(SedCommand::ClearPatternSpace { range })
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
                range: Some((Address::LineNumber(10), Address::LineNumber(10))),
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
                range: Some((Address::LineNumber(1), Address::LineNumber(10))),
            }
        );
    }

    #[test]
    fn test_parse_delete_line() {
        let cmd = parse_single_command("10d").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Delete {
                range: (Address::LineNumber(10), Address::LineNumber(10)),
            }
        );
    }

    #[test]
    fn test_parse_delete_range() {
        let cmd = parse_single_command("1,10d").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Delete {
                range: (Address::LineNumber(1), Address::LineNumber(10)),
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
                range: (Address::LineNumber(10), Address::LineNumber(10)),
            }
        );
    }

    #[test]
    fn test_parse_print_range() {
        let cmd = parse_single_command("1,10p").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Print {
                range: (Address::LineNumber(1), Address::LineNumber(10)),
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
        assert_eq!(cmd, SedCommand::Hold { range: None });
    }

    #[test]
    fn test_parse_hold_with_address() {
        let cmd = parse_single_command("5h").unwrap();
        assert_eq!(
            cmd,
            SedCommand::Hold {
                range: Some((Address::LineNumber(5), Address::LineNumber(5)))
            }
        );
    }

    #[test]
    fn test_parse_hold_append_with_range() {
        let cmd = parse_single_command("1,5H").unwrap();
        assert_eq!(
            cmd,
            SedCommand::HoldAppend {
                range: Some((Address::LineNumber(1), Address::LineNumber(5)))
            }
        );
    }

    #[test]
    fn test_parse_get_append() {
        let cmd = parse_single_command("$G").unwrap();
        assert_eq!(
            cmd,
            SedCommand::GetAppend {
                range: Some((Address::LastLine, Address::LastLine))
            }
        );
    }

    #[test]
    fn test_parse_exchange_with_pattern() {
        let cmd = parse_single_command("/pattern/x").unwrap();
        match cmd {
            SedCommand::Exchange {
                range: Some((Address::Pattern(p), _)),
            } => {
                assert_eq!(p, "pattern");
            }
            _ => panic!("Expected Exchange command with pattern"),
        }
    }

    #[test]
    fn test_parse_get_with_negation() {
        let cmd = parse_single_command("/foo/!g").unwrap();
        match cmd {
            SedCommand::Get {
                range: Some((Address::Negated(_), _)),
            } => {
                // Success
            }
            _ => panic!("Expected Get command with negation"),
        }
    }

    #[test]
    fn test_parse_hold_range_with_patterns() {
        let cmd = parse_single_command("/start/,/end/H").unwrap();
        match cmd {
            SedCommand::HoldAppend {
                range: Some((Address::Pattern(s), Address::Pattern(e))),
            } => {
                assert_eq!(s, "start");
                assert_eq!(e, "end");
            }
            _ => panic!("Expected HoldAppend with pattern range"),
        }
    }

    #[test]
    fn test_parse_get_simple() {
        let cmd = parse_single_command("g").unwrap();
        assert_eq!(cmd, SedCommand::Get { range: None });
    }

    #[test]
    fn test_parse_exchange_simple() {
        let cmd = parse_single_command("x").unwrap();
        assert_eq!(cmd, SedCommand::Exchange { range: None });
    }
}
