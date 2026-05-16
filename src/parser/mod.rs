pub mod address;
pub mod commands;
pub mod errors;
pub mod flow;
pub mod group;
pub mod io;
pub mod substitution;

use crate::cli::RegexFlavor;
use crate::command::Command;
use anyhow::{Result, anyhow};
pub use errors::format_parse_error;

/// Unified parser that supports sed syntax with configurable regex flavor.
pub struct Parser {
    regex_flavor: RegexFlavor,
}

impl Parser {
    /// Create a parser that interprets substitution patterns and
    /// replacements in the given regex flavor.
    pub fn new(regex_flavor: RegexFlavor) -> Self {
        Self { regex_flavor }
    }

    /// Parse a sed-style expression into a flat list of commands.
    pub fn parse(&self, expression: &str) -> Result<Vec<Command>> {
        let mut commands = parse_sed_expression(expression)?;
        for cmd in &mut commands {
            self.apply_flavor_to_substitutions(cmd);
        }
        Ok(commands)
    }

    /// Recursively walks a Command tree and, for every Substitution
    /// variant, rewrites its pattern and replacement into the canonical
    /// PCRE form understood by the downstream regex engine.
    fn apply_flavor_to_substitutions(&self, cmd: &mut Command) {
        match cmd {
            Command::Substitution {
                pattern,
                replacement,
                ..
            } => {
                *pattern = self.convert_pattern(pattern);
                *replacement = self.convert_replacement(replacement);
            }
            Command::Group { commands, .. } => {
                for inner in commands {
                    self.apply_flavor_to_substitutions(inner);
                }
            }
            _ => {}
        }
    }

    fn convert_pattern(&self, pattern: &str) -> String {
        match self.regex_flavor {
            RegexFlavor::BRE => crate::bre_converter::convert_bre_to_pcre(pattern),
            RegexFlavor::ERE => crate::ere_converter::convert_ere_to_pcre_pattern(pattern),
            RegexFlavor::PCRE => pattern.to_string(),
        }
    }

    fn convert_replacement(&self, replacement: &str) -> String {
        match self.regex_flavor {
            RegexFlavor::ERE => crate::ere_converter::convert_ere_backreferences(replacement),
            RegexFlavor::BRE | RegexFlavor::PCRE => {
                crate::bre_converter::convert_sed_backreferences(replacement)
            }
        }
    }
}

pub fn parse_sed_expression(expr: &str) -> Result<Vec<Command>> {
    let mut commands = Vec::new();
    let mut current_expr = String::new();
    let mut in_braces = 0;

    for c in expr.chars() {
        match c {
            '{' => {
                in_braces += 1;
                current_expr.push(c);
            }
            '}' => {
                if in_braces > 0 {
                    in_braces -= 1;
                }
                current_expr.push(c);
            }
            ';' if in_braces == 0 => {
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

    let part = current_expr.trim();
    if !part.is_empty() {
        commands.push(parse_single_command(part)?);
    }

    Ok(commands)
}

pub fn parse_single_command(cmd: &str) -> Result<Command> {
    let cmd = cmd.trim();

    if cmd.is_empty() {
        return Err(anyhow!("Empty command"));
    }

    // Identify the command character and its position
    let (pos, char_at_pos) = commands::find_command_char(cmd).ok_or_else(|| {
        anyhow!(
            "{}",
            format_parse_error(
                cmd,
                None,
                "unknown or missing command character",
                Some("Valid commands include: s, d, p, q, i, a, c, h, g, x, n, b, t, etc.")
            )
        )
    })?;

    match char_at_pos {
        '{' => group::parse_group(cmd),
        's' => substitution::parse_substitution(cmd),
        'i' => commands::parse_insert(cmd),
        'a' => commands::parse_append(cmd),
        'c' => commands::parse_change(cmd),
        'd' => commands::parse_delete(cmd),
        'p' => commands::parse_print(cmd),
        'q' => commands::parse_quit(cmd),
        'Q' => commands::parse_quit_without_print(cmd),
        'h' => commands::parse_hold(cmd),
        'H' => commands::parse_hold_append(cmd),
        'g' => commands::parse_get(cmd),
        'G' => commands::parse_get_append(cmd),
        'x' => commands::parse_exchange(cmd),
        'n' => commands::parse_next(cmd),
        'N' => commands::parse_next_append(cmd),
        'P' => commands::parse_print_first_line(cmd),
        'D' => commands::parse_delete_first_line(cmd),
        ':' => flow::parse_label(cmd),
        'b' => flow::parse_branch_at(cmd, pos),
        't' => flow::parse_test_at(cmd, pos),
        'T' => flow::parse_test_false_at(cmd, pos),
        '=' => commands::parse_print_line_number(cmd),
        'F' => commands::parse_print_filename(cmd),
        'z' => commands::parse_clear_pattern_space(cmd),
        'r' => io::parse_read_file(cmd),
        'R' => io::parse_read_line(cmd),
        'w' => io::parse_write_file(cmd),
        'W' => io::parse_write_first_line(cmd),
        _ => Err(anyhow!(
            "{}",
            format_parse_error(
                cmd,
                Some(pos),
                &format!("unknown command character '{}'", char_at_pos),
                Some("Valid commands include: s, d, p, q, i, a, c, h, g, x, n, b, t, etc.")
            )
        )),
    }
}
