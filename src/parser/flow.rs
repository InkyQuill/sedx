use crate::command::Command;
use crate::parser::address::parse_optional_range;
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

pub fn parse_branch_at(cmd: &str, pos: usize) -> Result<Command> {
    let addr_part = &cmd[..pos];
    let label_part = &cmd[pos + 1..];
    let range = parse_optional_range(addr_part)?;
    let label = parse_optional_label(label_part);
    Ok(Command::Branch { label, range })
}

pub fn parse_test_at(cmd: &str, pos: usize) -> Result<Command> {
    let addr_part = &cmd[..pos];
    let label_part = &cmd[pos + 1..];
    let range = parse_optional_range(addr_part)?;
    let label = parse_optional_label(label_part);
    Ok(Command::Test { label, range })
}

pub fn parse_test_false_at(cmd: &str, pos: usize) -> Result<Command> {
    let addr_part = &cmd[..pos];
    let label_part = &cmd[pos + 1..];
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
