use anyhow::{Context, Result};
use crate::sed_parser::{SedCommand, Address};
use regex::{Regex, RegexBuilder};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    Unchanged,    // Line not modified
    Modified,     // Line content changed
    Added,        // New line inserted
    Deleted,      // Line removed
}

#[derive(Debug, Clone)]
pub struct LineChange {
    pub line_number: usize,
    pub change_type: ChangeType,
    pub content: String,
    pub old_content: Option<String>,  // For Modified type
}

#[derive(Debug)]
pub struct FileDiff {
    pub file_path: String,
    pub changes: Vec<LineChange>,
    pub all_lines: Vec<(usize, String, ChangeType)>,  // (line_number, content, change_type)
    pub printed_lines: Vec<String>,  // Lines from print commands
}

// Legacy structure for backward compatibility
#[derive(Debug)]
pub struct FileChange {
    pub line_number: usize,
    pub old_content: String,
    pub new_content: String,
}

pub struct FileProcessor {
    commands: Vec<SedCommand>,
    printed_lines: Vec<String>,
}

impl FileProcessor {
    pub fn new(commands: Vec<SedCommand>) -> Self {
        Self {
            commands,
            printed_lines: Vec::new(),
        }
    }

    /// Legacy method - returns simple changes (for backward compatibility)
    pub fn process_file(&mut self, file_path: &Path) -> Result<Vec<FileChange>> {
        let diff = self.process_file_with_context(file_path)?;

        Ok(diff.changes.iter()
            .filter(|c| c.change_type == ChangeType::Modified)
            .map(|c| FileChange {
                line_number: c.line_number,
                old_content: c.old_content.clone().unwrap_or_default(),
                new_content: c.content.clone(),
            })
            .collect())
    }

    /// New method - returns detailed diff with context
    pub fn process_file_with_context(&mut self, file_path: &Path) -> Result<FileDiff> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let original_lines: Vec<&str> = content.lines().collect();
        let mut modified_lines: Vec<String> = original_lines.iter().map(|s| s.to_string()).collect();

        // Clear printed lines from previous run
        self.printed_lines.clear();

        // Apply all sed commands (stop if quit is encountered)
        let commands = self.commands.clone();
        for cmd in &commands {
            let should_continue = self.apply_command(&mut modified_lines, cmd)?;
            if !should_continue {
                break; // Quit command encountered
            }
        }

        // Clone modified_lines for diff generation (to avoid borrow issues)
        let modified_lines_clone = modified_lines.clone();

        // Generate detailed diff using simple comparison
        let all_lines = self.generate_simple_diff(&original_lines, &modified_lines_clone);

        // Collect only changed lines for summary
        let changes: Vec<LineChange> = all_lines.iter()
            .filter(|(_, _, change_type)| *change_type != ChangeType::Unchanged)
            .map(|(line_num, content, change_type)| {
                let old_content = if *change_type == ChangeType::Modified {
                    original_lines.get(line_num - 1).map(|s| s.to_string())
                } else {
                    None
                };

                LineChange {
                    line_number: *line_num,
                    change_type: change_type.clone(),
                    content: content.clone(),
                    old_content,
                }
            })
            .collect();

        Ok(FileDiff {
            file_path: file_path.display().to_string(),
            changes,
            all_lines,
            printed_lines: self.printed_lines.clone(),
        })
    }

    fn generate_simple_diff(&self, original: &[&str], modified: &[String]) -> Vec<(usize, String, ChangeType)> {
        let mut result = Vec::new();

        // Simple line-by-line comparison for now
        let max_len = original.len().max(modified.len());

        for i in 0..max_len {
            if i < original.len() && i < modified.len() {
                if original[i] == modified[i].as_str() {
                    result.push((i + 1, original[i].to_string(), ChangeType::Unchanged));
                } else {
                    result.push((i + 1, modified[i].clone(), ChangeType::Modified));
                }
            } else if i < original.len() {
                // Line was deleted
                result.push((i + 1, original[i].to_string(), ChangeType::Deleted));
            } else {
                // Line was added
                result.push((i + 1, modified[i].clone(), ChangeType::Added));
            }
        }

        result
    }

    pub fn apply_to_file(&mut self, file_path: &Path) -> Result<usize> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        let commands = self.commands.clone();
        for cmd in &commands {
            let should_continue = self.apply_command(&mut lines, cmd)?;
            if !should_continue {
                break; // Quit command encountered
            }
        }

        let new_content = lines.join("\n") + "\n";
        fs::write(file_path, new_content)
            .with_context(|| format!("Failed to write file: {}", file_path.display()))?;

        Ok(lines.len())
    }

    fn apply_command(&mut self, lines: &mut Vec<String>, cmd: &SedCommand) -> Result<bool> {
        // Returns Ok(true) if processing should continue, Ok(false) if quit was requested
        match cmd {
            SedCommand::Substitution { pattern, replacement, flags, range } => {
                self.apply_substitution(lines, pattern, replacement, flags, range)?;
            }
            SedCommand::Delete { range } => {
                self.apply_delete(lines, range)?;
            }
            SedCommand::Insert { text, address } => {
                self.apply_insert(lines, text, address)?;
            }
            SedCommand::Append { text, address } => {
                self.apply_append(lines, text, address)?;
            }
            SedCommand::Change { text, address } => {
                self.apply_change(lines, text, address)?;
            }
            SedCommand::Print { range } => {
                // Collect lines to print (doesn't modify the file)
                self.collect_print_lines(lines, range)?;
            }
            SedCommand::Quit { address } => {
                // Check if we should quit
                if let Some(addr) = address {
                    let idx = self.resolve_address(addr, lines, 0)?;
                    if idx < lines.len() {
                        // Quit at this line - truncate the file to this line
                        // Keep lines 0..=idx (inclusive), remove the rest
                        let lines_to_remove = lines.len().saturating_sub(idx + 1);
                        for _ in 0..lines_to_remove {
                            lines.pop();
                        }
                    }
                } else {
                    // Quit immediately - clear all lines
                    lines.clear();
                }
                // Always stop processing after quit
                return Ok(false);
            }
            SedCommand::Group { range, commands } => {
                // Group needs to handle things differently since it's recursive
                // Reconstruct commands as a vector we can use
                let commands_vec = commands.to_vec();
                return self.apply_group(lines, range, &commands_vec);
            }
        }
        Ok(true)
    }

    fn apply_substitution(&self, lines: &mut Vec<String>, pattern: &str, replacement: &str, flags: &Vec<char>, range: &Option<(Address, Address)>) -> Result<()> {
        let global = flags.contains(&'g');
        let case_insensitive = flags.contains(&'i');

        let re = if case_insensitive {
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
                .with_context(|| format!("Invalid regex pattern: {}", pattern))?
        } else {
            Regex::new(pattern)
                .with_context(|| format!("Invalid regex pattern: {}", pattern))?
        };

        // Check for negated pattern range
        if let Some((start, end)) = range {
            if let (Address::Negated(start_inner), Address::Negated(end_inner)) = (start, end) {
                if let (Address::Pattern(start_pat), Address::Pattern(end_pat)) = (start_inner.as_ref(), end_inner.as_ref()) {
                    // Apply substitution to lines NOT matching the pattern
                    let pattern_re = Regex::new(start_pat)
                        .with_context(|| format!("Invalid regex pattern: {}", start_pat))?;

                    for line in lines.iter_mut() {
                        if !pattern_re.is_match(line) {
                            if global {
                                *line = re.replace_all(line, replacement).to_string();
                            } else {
                                *line = re.replace(line, replacement).to_string();
                            }
                        }
                    }
                    return Ok(());
                }
            }
        }

        // Check if both addresses are the same pattern - substitute on all matching lines
        if let Some((start, end)) = range {
            if let (Address::Pattern(start_pat), Address::Pattern(end_pat)) = (start, end) {
                if start_pat == end_pat {
                    return self.apply_pattern_substitution(
                        lines,
                        start_pat,
                        &re,
                        replacement,
                        global
                    );
                }
            }
        }

        match range {
            None => {
                // Apply to all lines
                for line in lines.iter_mut() {
                    if global {
                        *line = re.replace_all(line, replacement).to_string();
                    } else {
                        *line = re.replace(line, replacement).to_string();
                    }
                }
            }
            Some((start, end)) => {
                // Apply to specified range
                let start_idx = self.resolve_address(start, lines, 0)?;
                let end_idx = self.resolve_address(end, lines, lines.len())?;

                for i in start_idx..=end_idx.min(lines.len() - 1) {
                    if global {
                        lines[i] = re.replace_all(&lines[i], replacement).to_string();
                    } else {
                        lines[i] = re.replace(&lines[i], replacement).to_string();
                    }
                }
            }
        }

        Ok(())
    }

    /// Apply substitution to all lines matching a pattern
    ///
    /// Implements GNU sed behavior where `/pattern/s/old/new/` applies
    /// the substitution to ALL lines matching the pattern, not just the first.
    ///
    /// # Arguments
    /// * `lines` - Mutable reference to the lines vector
    /// * `pattern_str` - Pattern string to match lines against
    /// * `pattern_regex` - Compiled regex for the substitution pattern
    /// * `replacement` - Replacement string (with backreferences converted)
    /// * `global` - If true, replace all occurrences in each line
    fn apply_pattern_substitution(
        &self,
        lines: &mut Vec<String>,
        pattern_str: &str,
        pattern_regex: &Regex,
        replacement: &str,
        global: bool,
    ) -> Result<()> {
        use regex::Regex;

        // Create regex to find matching lines
        let line_pattern_re = Regex::new(pattern_str)
            .with_context(|| format!("Invalid regex pattern: {}", pattern_str))?;

        // Apply substitution to all lines matching the pattern
        for line in lines.iter_mut() {
            if line_pattern_re.is_match(line) {
                if global {
                    *line = pattern_regex.replace_all(line, replacement).to_string();
                } else {
                    *line = pattern_regex.replace(line, replacement).to_string();
                }
            }
        }

        Ok(())
    }

    fn apply_delete(&self, lines: &mut Vec<String>, range: &(Address, Address)) -> Result<()> {
        // Check if both addresses are the same pattern - delete all matching lines
        if let (Address::Pattern(start_pat), Address::Pattern(end_pat)) = (&range.0, &range.1) {
            if start_pat == end_pat {
                // Delete all lines matching this pattern
                return self.apply_pattern_delete(lines, start_pat);
            } else {
                // Different patterns - use range state machine
                return self.apply_pattern_range_delete(lines, start_pat, end_pat);
            }
        }

        // Check if both addresses are negated patterns - delete lines NOT matching
        if let (Address::Negated(start_inner), Address::Negated(end_inner)) = (&range.0, &range.1) {
            if let (Address::Pattern(start_pat), Address::Pattern(end_pat)) = (start_inner.as_ref(), end_inner.as_ref()) {
                return self.apply_negated_pattern_delete(lines, start_pat);
            }
        }

        // For line numbers or mixed addresses, use simple range resolution
        let start_idx = self.resolve_address(&range.0, lines, 0)?;
        let end_idx = self.resolve_address(&range.1, lines, lines.len())?;

        // Remove lines from end_idx to start_idx (in reverse to maintain indices)
        for i in (start_idx..=end_idx.min(lines.len() - 1)).rev() {
            lines.remove(i);
        }

        Ok(())
    }

    fn apply_pattern_delete(&self, lines: &mut Vec<String>, pattern: &str) -> Result<()> {
        use regex::Regex;

        let re = Regex::new(pattern)
            .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        // Delete all lines matching the pattern
        let mut indices_to_delete = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if re.is_match(line) {
                indices_to_delete.push(i);
            }
        }

        // Remove lines in reverse order to maintain indices
        for i in indices_to_delete.into_iter().rev() {
            lines.remove(i);
        }

        Ok(())
    }

    fn apply_negated_pattern_delete(&self, lines: &mut Vec<String>, pattern: &str) -> Result<()> {
        use regex::Regex;

        let re = Regex::new(pattern)
            .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        // Delete lines that DO NOT match the pattern
        let mut indices_to_delete = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if !re.is_match(line) {
                indices_to_delete.push(i);
            }
        }

        // Remove lines in reverse order to maintain indices
        for i in indices_to_delete.into_iter().rev() {
            lines.remove(i);
        }

        Ok(())
    }

    fn apply_pattern_range_delete(&self, lines: &mut Vec<String>, start_pat: &str, end_pat: &str) -> Result<()> {
        use regex::Regex;

        let start_re = Regex::new(start_pat)
            .with_context(|| format!("Invalid regex pattern: {}", start_pat))?;
        let end_re = Regex::new(end_pat)
            .with_context(|| format!("Invalid regex pattern: {}", end_pat))?;

        let mut in_delete_range = false;
        let mut indices_to_delete = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            if !in_delete_range {
                // Check if this line matches the start pattern
                if start_re.is_match(line) {
                    in_delete_range = true;
                    indices_to_delete.push(i);
                }
            } else {
                // We're in a delete range
                indices_to_delete.push(i);

                // Check if this line matches the end pattern
                if end_re.is_match(line) {
                    in_delete_range = false;
                }
            }
        }

        // Remove lines in reverse order to maintain indices
        for i in indices_to_delete.into_iter().rev() {
            lines.remove(i);
        }

        Ok(())
    }

    fn apply_group(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>, commands: &[SedCommand]) -> Result<bool> {
        let mut should_continue = true;
        match range {
            None => {
                // No range - apply commands to all lines sequentially
                // Apply each command to the entire file
                for cmd in commands {
                    should_continue = self.apply_command(lines, cmd)?;
                    if !should_continue {
                        break;
                    }
                }
            }
            Some((start, end)) => {
                // Apply commands only to lines within the range
                let start_idx = self.resolve_address(&start, lines, 0)?;
                let end_idx = self.resolve_address(&end, lines, lines.len().saturating_sub(1))?;

                // For each line in the range, apply all commands
                for i in start_idx..=end_idx.min(lines.len() - 1) {
                    for cmd in commands {
                        should_continue = self.apply_command(lines, cmd)?;

                        // Check if the line was deleted
                        if i >= lines.len() {
                            break;
                        }

                        if !should_continue {
                            break;
                        }
                    }

                    // If we deleted the line and our index is now out of bounds, stop
                    if i >= lines.len() {
                        break;
                    }

                    if !should_continue {
                        break;
                    }
                }
            }
        }

        Ok(should_continue)
    }

    fn apply_insert(&self, lines: &mut Vec<String>, text: &str, address: &Address) -> Result<()> {
        let idx = self.resolve_address(address, lines, 0)?;
        lines.insert(idx, text.to_string());
        Ok(())
    }

    fn apply_append(&self, lines: &mut Vec<String>, text: &str, address: &Address) -> Result<()> {
        let idx = self.resolve_address(address, lines, 0)?;
        let insert_pos = (idx + 1).min(lines.len());
        lines.insert(insert_pos, text.to_string());
        Ok(())
    }

    fn apply_change(&self, lines: &mut Vec<String>, text: &str, address: &Address) -> Result<()> {
        let idx = self.resolve_address(address, lines, 0)?;
        if idx < lines.len() {
            lines[idx] = text.to_string();
        }
        Ok(())
    }

    fn collect_print_lines(&mut self, lines: &[String], range: &(Address, Address)) -> Result<()> {
        // Special handling for negated pattern addresses
        if let (Address::Negated(start_inner), Address::Negated(end_inner)) = (&range.0, &range.1) {
            if let (Address::Pattern(start_pat), Address::Pattern(end_pat)) = (start_inner.as_ref(), end_inner.as_ref()) {
                // Print lines NOT matching the pattern
                use regex::Regex;
                let re = Regex::new(start_pat)
                    .with_context(|| format!("Invalid regex pattern: {}", start_pat))?;

                for line in lines {
                    if !re.is_match(line) {
                        self.printed_lines.push(line.clone());
                    }
                }
                return Ok(());
            }
        }

        let start_idx = self.resolve_address(&range.0, lines, 0)?;
        let end_idx = self.resolve_address(&range.1, lines, lines.len().saturating_sub(1))?;

        for i in start_idx..=end_idx.min(lines.len() - 1) {
            self.printed_lines.push(lines[i].clone());
        }

        Ok(())
    }

    fn resolve_address(&self, address: &Address, lines: &[String], default: usize) -> Result<usize> {
        match address {
            Address::LineNumber(n) => {
                if *n == 0 {
                    Ok(0)
                } else if *n > lines.len() {
                    Ok(lines.len())
                } else {
                    Ok(n - 1)  // Convert to 0-indexed
                }
            }
            Address::Pattern(pattern) => {
                let re = Regex::new(pattern)
                    .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

                for (i, line) in lines.iter().enumerate() {
                    if re.is_match(line) {
                        return Ok(i);
                    }
                }

                // Pattern not found, return default
                Ok(default)
            }
            Address::FirstLine => Ok(0),
            Address::LastLine => {
                if lines.is_empty() {
                    Ok(0)
                } else {
                    Ok(lines.len() - 1)
                }
            }
            Address::Negated(inner) => {
                // Find the first line that DOESN'T match the inner address
                let inner_idx = self.resolve_address(inner, lines, 0)?;

                // For pattern negation, find first non-matching line
                if let Address::Pattern(pattern) = inner.as_ref() {
                    let re = Regex::new(pattern)
                        .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

                    for (i, line) in lines.iter().enumerate() {
                        if !re.is_match(line) {
                            return Ok(i);
                        }
                    }
                }

                // For line number negation, skip that specific line
                Ok(inner_idx + 1)
            }
        }
    }
}
