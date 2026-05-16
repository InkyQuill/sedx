use crate::command::Command;
use crate::parser::address::parse_optional_range;
use crate::parser::commands;
use crate::parser::errors::format_parse_error;
use crate::parser::find_structural_group_close;
use crate::parser::parse_sed_expression;
use anyhow::{Result, anyhow};

// Retained for public library API compatibility; binary dispatch uses known positions.
#[allow(dead_code)]
pub fn parse_group(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();
    let (pos, found) = commands::find_command_char(cmd).ok_or_else(|| {
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

    if found != '{' {
        return Err(anyhow!(
            "expected group command but found '{}' at position {}",
            found,
            pos
        ));
    }

    parse_group_at(cmd, pos)
}

pub fn parse_group_at(cmd: &str, pos: usize) -> Result<Command> {
    let addr_part = cmd
        .get(..pos)
        .ok_or_else(|| anyhow!("invalid group command position {}", pos))?
        .trim();
    let group_part = cmd
        .get(pos..)
        .ok_or_else(|| anyhow!("invalid group command position {}", pos))?;
    group_part
        .strip_prefix('{')
        .ok_or_else(|| anyhow!("expected group command at position {}", pos))?;

    let brace_start = pos + '{'.len_utf8();
    let close_brace = find_structural_group_close(cmd, pos).ok_or_else(|| {
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
    let commands_str = cmd[brace_start..close_brace].trim();
    let trailing = cmd[close_brace + '}'.len_utf8()..].trim();
    if !trailing.is_empty() {
        return Err(anyhow!(
            "unexpected trailing content after group command: {}",
            trailing
        ));
    }

    let range = parse_optional_range(addr_part)?;

    let commands = parse_sed_expression(commands_str)?;

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

#[cfg(test)]
mod tests {
    use super::parse_group_at;
    use crate::command::{Address, Command};

    #[test]
    fn parser_uses_supplied_group_position_when_address_contains_brace() {
        let cmd = r"\#a{b#,\#c#{p}";
        let pos = cmd.rfind('{').unwrap();

        assert_eq!(
            parse_group_at(cmd, pos).unwrap(),
            Command::Group {
                range: Some((
                    Address::Pattern("a{b".to_string()),
                    Address::Pattern("c".to_string()),
                )),
                commands: vec![Command::Print {
                    range: (Address::LineNumber(1), Address::LastLine),
                }],
            }
        );
    }

    #[test]
    fn parser_rejects_wrong_group_position_without_panicking() {
        assert!(parse_group_at(r"\#a{b#,\#c#{p}", 4).is_err());
    }
}
