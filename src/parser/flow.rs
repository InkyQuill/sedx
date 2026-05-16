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

pub fn parse_branch(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();
    let b_pos = cmd
        .find('b')
        .ok_or_else(|| anyhow!("Branch command missing 'b'"))?;

    let address_part = &cmd[..b_pos];
    let rest_part = &cmd[b_pos..];

    let range = parse_optional_range(address_part)?;

    let label_part = &rest_part[1..];
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

    Ok(Command::Branch { label, range })
}

pub fn parse_test(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();
    let t_pos = cmd
        .find('t')
        .ok_or_else(|| anyhow!("Test command missing 't'"))?;

    let address_part = &cmd[..t_pos];
    let rest_part = &cmd[t_pos..];

    let range = parse_optional_range(address_part)?;

    let label_part = &rest_part[1..];
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

    Ok(Command::Test { label, range })
}

pub fn parse_test_false(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();
    let t_pos = cmd
        .find('T')
        .ok_or_else(|| anyhow!("Test false command missing 'T'"))?;

    let address_part = &cmd[..t_pos];
    let rest_part = &cmd[t_pos..];

    let range = parse_optional_range(address_part)?;

    let label_part = &rest_part[1..];
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

    Ok(Command::TestFalse { label, range })
}
