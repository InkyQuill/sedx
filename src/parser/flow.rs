use crate::command::Command;
use crate::parser::address::parse_optional_range;
use crate::parser::commands;
use crate::parser::errors::format_parse_error;
use anyhow::{Result, anyhow};

pub fn parse_label(cmd: &str) -> Result<Command> {
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
    let label_char_count = label_name.chars().count();
    if label_char_count > 8 {
        let truncated_suggestion: String = label_name.chars().take(8).collect();
        return Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                &format!("label name '{}' is too long (max 8 characters)", label_name),
                Some(&format!(
                    "Shorten the label name to 8 characters or less.\nSuggestion: {} (truncated)",
                    truncated_suggestion
                )),
            )
        ));
    }

    Ok(Command::Label {
        name: label_name.to_string(),
    })
}

pub fn parse_branch(cmd: &str) -> Result<Command> {
    let pos = find_flow_command_pos(cmd, 'b')?;
    parse_branch_at(cmd, pos)
}

pub fn parse_branch_at(cmd: &str, pos: usize) -> Result<Command> {
    let (addr_part, label_part) = split_flow_command_at(cmd, pos, 'b')?;
    let range = parse_optional_range(addr_part)?;
    let label = parse_optional_label(label_part);
    Ok(Command::Branch { label, range })
}

pub fn parse_test(cmd: &str) -> Result<Command> {
    let pos = find_flow_command_pos(cmd, 't')?;
    parse_test_at(cmd, pos)
}

pub fn parse_test_at(cmd: &str, pos: usize) -> Result<Command> {
    let (addr_part, label_part) = split_flow_command_at(cmd, pos, 't')?;
    let range = parse_optional_range(addr_part)?;
    let label = parse_optional_label(label_part);
    Ok(Command::Test { label, range })
}

pub fn parse_test_false(cmd: &str) -> Result<Command> {
    let pos = find_flow_command_pos(cmd, 'T')?;
    parse_test_false_at(cmd, pos)
}

pub fn parse_test_false_at(cmd: &str, pos: usize) -> Result<Command> {
    let (addr_part, label_part) = split_flow_command_at(cmd, pos, 'T')?;
    let range = parse_optional_range(addr_part)?;
    let label = parse_optional_label(label_part);
    Ok(Command::TestFalse { label, range })
}

fn parse_optional_label(label_part: &str) -> Option<String> {
    let label_name = label_part.trim();
    if label_name.is_empty() {
        None
    } else {
        Some(label_name.to_string())
    }
}

fn find_flow_command_pos(cmd: &str, command: char) -> Result<usize> {
    let (pos, found) = commands::find_command_char(cmd)
        .ok_or_else(|| anyhow!("flow command missing '{}'", command))?;

    if found != command {
        return Err(anyhow!(
            "expected '{}' command but found '{}' at position {}",
            command,
            found,
            pos
        ));
    }

    Ok(pos)
}

fn split_flow_command_at(cmd: &str, pos: usize, command: char) -> Result<(&str, &str)> {
    let addr_part = cmd
        .get(..pos)
        .ok_or_else(|| anyhow!("invalid flow command position {}", pos))?;
    let command_part = cmd
        .get(pos..)
        .ok_or_else(|| anyhow!("invalid flow command position {}", pos))?;

    let label_part = command_part
        .strip_prefix(command)
        .ok_or_else(|| anyhow!("expected '{}' command at position {}", command, pos))?;

    Ok((addr_part, label_part))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_branch, parse_branch_at, parse_test, parse_test_at, parse_test_false,
        parse_test_false_at,
    };
    use crate::command::{Address, Command};

    #[test]
    fn branch_wrapper_uses_discovered_command_position() {
        assert_eq!(
            parse_branch("/b/b done").unwrap(),
            Command::Branch {
                label: Some("done".to_string()),
                range: Some((
                    Address::Pattern("b".to_string()),
                    Address::Pattern("b".to_string()),
                )),
            }
        );
    }

    #[test]
    fn test_wrapper_uses_discovered_command_position() {
        assert_eq!(
            parse_test("/t/t done").unwrap(),
            Command::Test {
                label: Some("done".to_string()),
                range: Some((
                    Address::Pattern("t".to_string()),
                    Address::Pattern("t".to_string()),
                )),
            }
        );
    }

    #[test]
    fn test_false_wrapper_uses_discovered_command_position() {
        assert_eq!(
            parse_test_false("/T/T done").unwrap(),
            Command::TestFalse {
                label: Some("done".to_string()),
                range: Some((
                    Address::Pattern("T".to_string()),
                    Address::Pattern("T".to_string()),
                )),
            }
        );
    }

    #[test]
    fn branch_wrapper_rejects_wrong_discovered_command() {
        assert!(parse_branch("1t done").is_err());
    }

    #[test]
    fn branch_parser_rejects_end_position_without_panicking() {
        assert!(parse_branch_at("1b", 2).is_err());
    }

    #[test]
    fn branch_parser_rejects_non_boundary_position_without_panicking() {
        assert!(parse_branch_at("é b", 1).is_err());
    }

    #[test]
    fn test_parser_rejects_wrong_command_position_without_panicking() {
        assert!(parse_test_at("1b label", 1).is_err());
    }

    #[test]
    fn test_false_parser_rejects_out_of_range_position_without_panicking() {
        assert!(parse_test_false_at("1T label", 99).is_err());
    }
}
