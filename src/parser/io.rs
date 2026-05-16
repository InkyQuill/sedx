use crate::command::{Address, Command};
use crate::parser::address::parse_address;
use crate::parser::errors::format_parse_error;
use anyhow::{Result, anyhow, bail};

pub fn is_inside_pattern_address(cmd: &str, pos: usize) -> bool {
    let bytes = cmd.as_bytes();
    let limit = pos.min(bytes.len());

    // Phase 1: left-to-right open/close scan up to `pos`.
    // A pattern address opens with `/` (or `\X` backslash-custom-delim).
    // Once open, `\X` escapes the next char; the matching opener char closes.
    let mut i = 0;
    let mut current_opener: Option<u8> = None;
    while i < limit {
        let byte = bytes[i];
        match current_opener {
            None => {
                if byte == b'/' {
                    current_opener = Some(byte);
                } else if byte == b'\\' && i + 1 < limit {
                    current_opener = Some(bytes[i + 1]);
                    i += 2;
                    continue;
                }
            }
            Some(opener) => {
                if byte == b'\\' && i + 1 < limit {
                    i += 2;
                    continue;
                }
                if byte == opener {
                    current_opener = None;
                }
            }
        }
        i += 1;
    }

    // If we are still waiting for a closer, we are inside a pattern address.
    if current_opener.is_some() {
        return true;
    }

    // Phase 2 covers the substitution replacement region (e.g. the `r` in
    // `s/foo/bar/`), where Phase 1 exits in the `None` state because the
    // pattern sub-region already closed. Discriminator: whitespace before the
    // next slash indicates a filename argument, not a paired closing delimiter.
    let has_slash_before = (0..pos)
        .rev()
        .any(|j| bytes[j] == b'/' && (j == 0 || bytes[j - 1] != b'\\'));
    if !has_slash_before {
        return false;
    }
    for &byte in bytes.iter().skip(pos + 1) {
        if byte.is_ascii_whitespace() {
            break;
        }
        if byte == b'/' {
            return true;
        }
    }

    false
}

pub fn parse_read_file(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();
    let r_pos = cmd
        .char_indices()
        .find(|&(pos, ch)| ch == 'r' && !is_inside_pattern_address(cmd, pos))
        .map(|(pos, _)| pos)
        .ok_or_else(|| anyhow!("Read file command missing 'r'"))?;

    let address_part = &cmd[..r_pos];
    let rest_part = &cmd[r_pos..];
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_io_address(address_part.trim())?)
    };

    let filename_part = &rest_part[1..];
    let filename = filename_part.trim();
    if filename.is_empty() {
        bail!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "read file command requires a filename",
                Some(
                    "Read file format: [address]r filename\nExample: 5r header.txt\n         /pat/r data.txt"
                ),
            )
        );
    }

    Ok(Command::ReadFile {
        filename: filename.to_string(),
        range,
    })
}

pub fn parse_write_file(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();
    let w_pos = cmd
        .char_indices()
        .find(|&(pos, ch)| ch == 'w' && !is_inside_pattern_address(cmd, pos))
        .map(|(pos, _)| pos)
        .ok_or_else(|| anyhow!("Write file command missing 'w'"))?;

    let address_part = &cmd[..w_pos];
    let rest_part = &cmd[w_pos..];
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_io_address(address_part.trim())?)
    };

    let filename_part = &rest_part[1..];
    let filename = filename_part.trim();
    if filename.is_empty() {
        bail!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "write file command requires a filename",
                Some(
                    "Write file format: [address]w filename\nExample: 5w output.txt\n         /pat/w log.txt"
                ),
            )
        );
    }

    Ok(Command::WriteFile {
        filename: filename.to_string(),
        range,
    })
}

pub fn parse_read_line(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();
    let r_pos = cmd
        .char_indices()
        .find(|&(pos, ch)| ch == 'R' && !is_inside_pattern_address(cmd, pos))
        .map(|(pos, _)| pos)
        .ok_or_else(|| anyhow!("Read line command missing 'R'"))?;

    let address_part = &cmd[..r_pos];
    let rest_part = &cmd[r_pos..];
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_io_address(address_part.trim())?)
    };

    let filename_part = &rest_part[1..];
    let filename = filename_part.trim();
    if filename.is_empty() {
        bail!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "read line command requires a filename",
                Some(
                    "Read line format: [address]R filename\nExample: 5R data.txt\n         /pat/R input.txt"
                ),
            )
        );
    }

    Ok(Command::ReadLine {
        filename: filename.to_string(),
        range,
    })
}

pub fn parse_write_first_line(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();
    let w_pos = cmd
        .char_indices()
        .find(|&(pos, ch)| ch == 'W' && !is_inside_pattern_address(cmd, pos))
        .map(|(pos, _)| pos)
        .ok_or_else(|| anyhow!("Write first line command missing 'W'"))?;

    let address_part = &cmd[..w_pos];
    let rest_part = &cmd[w_pos..];
    let range = if address_part.trim().is_empty() {
        None
    } else {
        Some(parse_io_address(address_part.trim())?)
    };

    let filename_part = &rest_part[1..];
    let filename = filename_part.trim();
    if filename.is_empty() {
        bail!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "write first line command requires a filename",
                Some(
                    "Write first line format: [address]W filename\nExample: 5W output.txt\n         /pat/W log.txt"
                ),
            )
        );
    }

    Ok(Command::WriteFirstLine {
        filename: filename.to_string(),
        range,
    })
}

fn parse_io_address(address_part: &str) -> Result<Address> {
    if let Some(pattern) = parse_custom_delimiter_address(address_part) {
        return Ok(Address::Pattern(pattern));
    }

    parse_address(address_part)
}

fn parse_custom_delimiter_address(address_part: &str) -> Option<String> {
    let bytes = address_part.as_bytes();
    if bytes.len() < 3 || bytes.first() != Some(&b'\\') {
        return None;
    }

    let delimiter = bytes[1] as char;
    let pattern_start = 2;
    let mut escaped = false;

    for (offset, ch) in address_part[pattern_start..].char_indices() {
        let pos = pattern_start + offset;
        if escaped {
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == delimiter && pos + ch.len_utf8() == address_part.len() {
            return Some(address_part[pattern_start..pos].to_string());
        }
    }

    None
}
