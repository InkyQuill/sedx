use crate::command::Command;
use crate::parser::address::parse_optional_range;
use crate::parser::errors::format_parse_error;
use crate::parser::parse_single_command;
use anyhow::{Result, anyhow};

pub fn parse_group(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();

    // Find the opening brace
    let open_brace = cmd.find('{').ok_or_else(|| {
        anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "group command is missing opening '{'",
                Some("Group format: [range] { command1; command2; ... }\nExample: {s/foo/bar/; s/baz/qux/}\n         1,10{s/^/> /}"),
            )
        )
    })?;

    // Extract the address/range part (before the brace)
    let addr_part = cmd[..open_brace].trim();

    // Find the matching closing brace
    let brace_start = open_brace + 1;
    let mut depth = 1;
    let mut close_brace = None;

    // Use char_indices() to get correct byte positions for UTF-8 strings
    for (i, c) in cmd[brace_start..].char_indices() {
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

    let close_brace = close_brace.ok_or_else(|| {
        anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "group command is missing closing '}'",
                Some("Every opening '{' must have a matching closing '}'.\nExample: {s/foo/bar/; p}\n                      ^ (add closing brace here)"),
            )
        )
    })?;

    // Extract commands inside the braces
    let commands_str = &cmd[brace_start..close_brace].trim();

    let range = parse_optional_range(addr_part)?;

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

    Ok(Command::Group { range, commands })
}
