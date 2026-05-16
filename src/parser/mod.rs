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
        let pattern = match self.regex_flavor {
            RegexFlavor::BRE => crate::bre_converter::convert_bre_to_pcre(pattern),
            RegexFlavor::ERE => crate::ere_converter::convert_ere_to_pcre_pattern(pattern),
            RegexFlavor::PCRE => pattern.to_string(),
        };

        substitution::restore_escaped_pattern_delimiters(&pattern)
    }

    fn convert_replacement(&self, replacement: &str) -> String {
        match self.regex_flavor {
            RegexFlavor::ERE => crate::ere_converter::convert_ere_backreferences(replacement),
            RegexFlavor::BRE => crate::bre_converter::convert_sed_backreferences(replacement),
            RegexFlavor::PCRE => crate::bre_converter::convert_pcre_replacement(replacement),
        }
    }
}

pub fn parse_sed_expression(expr: &str) -> Result<Vec<Command>> {
    let mut commands = Vec::new();
    let mut current_expr = String::new();
    let mut brace_depth = 0usize;
    let mut pattern_delimiter: Option<char> = None;
    let mut substitution: Option<SubstitutionSplitState> = None;
    let mut escaped = false;

    let mut chars = expr.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(state) = &mut substitution {
            current_expr.push(c);

            if state.escaped {
                state.escaped = false;
                continue;
            }

            if c == '\\' {
                state.escaped = true;
                continue;
            }

            if c == state.delimiter {
                state.closed_sections += 1;
                if state.closed_sections == 2 {
                    substitution = None;
                }
            }

            continue;
        }

        if let Some(delimiter) = pattern_delimiter {
            current_expr.push(c);

            if escaped {
                escaped = false;
                continue;
            }

            if c == '\\' {
                escaped = true;
                continue;
            }

            if c == delimiter {
                pattern_delimiter = None;
            }

            continue;
        }

        if can_start_pattern_address(&current_expr) {
            if c == '/' {
                pattern_delimiter = Some('/');
                current_expr.push(c);
                continue;
            }

            if c == '\\' {
                if let Some(delimiter) = chars.next() {
                    pattern_delimiter = Some(delimiter);
                    current_expr.push(c);
                    current_expr.push(delimiter);
                    continue;
                }
            }
        }

        if c == 's' {
            if let Some(&delimiter) = chars.peek() {
                if is_substitution_delimiter(delimiter)
                    && is_substitution_command_start(&current_expr, delimiter)
                {
                    current_expr.push(c);
                    current_expr.push(delimiter);
                    chars.next();
                    substitution = Some(SubstitutionSplitState {
                        delimiter,
                        closed_sections: 0,
                        escaped: false,
                    });
                    continue;
                }
            }
        }

        match c {
            '{' => {
                brace_depth += 1;
                current_expr.push(c);
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                current_expr.push(c);
            }
            ';' if brace_depth == 0 => {
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

fn can_start_pattern_address(current_expr: &str) -> bool {
    matches!(
        current_expr.chars().rev().find(|c| !c.is_whitespace()),
        None | Some(',') | Some('{') | Some(';')
    )
}

#[derive(Debug)]
struct SubstitutionSplitState {
    delimiter: char,
    closed_sections: u8,
    escaped: bool,
}

fn is_substitution_delimiter(c: char) -> bool {
    matches!(c, '/' | '#' | ':' | '|')
}

fn is_substitution_command_start(current_expr: &str, delimiter: char) -> bool {
    let command_prefix_start = command_prefix_start_after_structural_separator(current_expr);
    let command_prefix = &current_expr[command_prefix_start..];

    let mut candidate = String::with_capacity(command_prefix.len() + 2);
    candidate.push_str(command_prefix);
    candidate.push('s');
    candidate.push(delimiter);

    commands::find_command_char(&candidate)
        .is_some_and(|(pos, command)| command == 's' && pos == command_prefix.len())
}

fn command_prefix_start_after_structural_separator(current_expr: &str) -> usize {
    let mut command_prefix_start = 0;
    let mut pattern_delimiter: Option<char> = None;
    let mut escaped = false;

    let mut chars = current_expr.char_indices().peekable();
    while let Some((pos, c)) = chars.next() {
        if let Some(delimiter) = pattern_delimiter {
            if escaped {
                escaped = false;
                continue;
            }

            if c == '\\' {
                escaped = true;
                continue;
            }

            if c == delimiter {
                pattern_delimiter = None;
            }

            continue;
        }

        if can_start_pattern_address(&current_expr[..pos]) {
            if c == '/' {
                pattern_delimiter = Some('/');
                continue;
            }

            if c == '\\' {
                if let Some((_, delimiter)) = chars.next() {
                    pattern_delimiter = Some(delimiter);
                    continue;
                }
            }
        }

        if matches!(c, '{' | ';') {
            command_prefix_start = pos + c.len_utf8();
        }
    }

    command_prefix_start
}

pub(crate) fn find_structural_group_close(expr: &str, open_pos: usize) -> Option<usize> {
    let mut current_expr = String::new();
    let mut brace_depth = 0usize;
    let mut tracking_group = false;
    let mut unterminated_substitution_group_close: Option<usize> = None;
    let mut pattern_delimiter: Option<char> = None;
    let mut substitution: Option<SubstitutionSplitState> = None;
    let mut escaped = false;

    let mut chars = expr.char_indices().peekable();
    while let Some((pos, c)) = chars.next() {
        if let Some(state) = &mut substitution {
            current_expr.push(c);

            if state.escaped {
                state.escaped = false;
                continue;
            }

            if c == '\\' {
                state.escaped = true;
                continue;
            }

            if c == '}' && tracking_group && brace_depth == 1 {
                unterminated_substitution_group_close = Some(pos);
            }

            if c == state.delimiter {
                state.closed_sections += 1;
                if state.closed_sections == 2 {
                    substitution = None;
                }
            }

            continue;
        }

        if let Some(delimiter) = pattern_delimiter {
            current_expr.push(c);

            if escaped {
                escaped = false;
                continue;
            }

            if c == '\\' {
                escaped = true;
                continue;
            }

            if c == delimiter {
                pattern_delimiter = None;
            }

            continue;
        }

        if can_start_pattern_address(&current_expr) {
            if c == '/' {
                pattern_delimiter = Some('/');
                current_expr.push(c);
                continue;
            }

            if c == '\\' {
                if let Some((_, delimiter)) = chars.next() {
                    pattern_delimiter = Some(delimiter);
                    current_expr.push(c);
                    current_expr.push(delimiter);
                    continue;
                }
            }
        }

        if c == 's' {
            if let Some((_, delimiter)) = chars.peek().copied() {
                if is_substitution_delimiter(delimiter)
                    && is_substitution_command_start(&current_expr, delimiter)
                {
                    current_expr.push(c);
                    current_expr.push(delimiter);
                    chars.next();
                    substitution = Some(SubstitutionSplitState {
                        delimiter,
                        closed_sections: 0,
                        escaped: false,
                    });
                    continue;
                }
            }
        }

        match c {
            '{' => {
                if tracking_group {
                    brace_depth += 1;
                } else if pos == open_pos {
                    tracking_group = true;
                    brace_depth = 1;
                }
                current_expr.push(c);
            }
            '}' => {
                if tracking_group {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        return Some(pos);
                    }
                }
                current_expr.push(c);
            }
            _ => current_expr.push(c),
        }
    }

    unterminated_substitution_group_close
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
        '{' => group::parse_group_at(cmd, pos),
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
