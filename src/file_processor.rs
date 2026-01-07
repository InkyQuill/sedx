use anyhow::{Context, Result};
use crate::sed_parser::{SedCommand, Address};
use regex::{Regex, RegexBuilder};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::collections::VecDeque;
use tempfile::NamedTempFile;

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
    pub is_streaming: bool,  // True if processed in streaming mode (all_lines may be empty)
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
    hold_space: String,
}

/// Result of applying a command in streaming mode
#[derive(Debug)]
enum StreamResult {
    Output(String),           // Line should be output
    Skip,                     // Don't output (deleted)
    StopProcessing,           // Quit command encountered
}

/// Processor for streaming large files with constant memory usage
pub struct StreamProcessor {
    commands: Vec<SedCommand>,
    hold_space: String,
    current_line: usize,
}

impl StreamProcessor {
    pub fn new(commands: Vec<SedCommand>) -> Self {
        Self {
            commands,
            hold_space: String::new(),
            current_line: 0,
        }
    }

    /// Check if file should use streaming based on size
    fn should_use_streaming(file_size: u64) -> bool {
        const STREAMING_THRESHOLD: u64 = 100 * 1024 * 1024; // 100MB
        file_size >= STREAMING_THRESHOLD
    }

    /// Apply substitution to a single line
    fn apply_substitution_to_line(
        &self,
        line: &str,
        pattern: &str,
        replacement: &str,
        flags: &Vec<char>,
    ) -> Result<String> {
        let global = flags.contains(&'g');
        let case_insensitive = flags.contains(&'i');

        // Check for numbered substitution (e.g., s/foo/bar/2)
        let nth_occurrence: Option<usize> = flags.iter()
            .find(|c| c.is_ascii_digit())
            .and_then(|c| c.to_digit(10))
            .map(|d| d as usize);

        let re = if case_insensitive {
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
                .with_context(|| format!("Invalid regex pattern: {}", pattern))?
        } else {
            Regex::new(pattern)
                .with_context(|| format!("Invalid regex pattern: {}", pattern))?
        };

        match nth_occurrence {
            Some(n) if n > 0 => {
                // Replace only the Nth occurrence
                let mut result = line.to_string();
                let mut count = 0;
                for mat in re.find_iter(line) {
                    count += 1;
                    if count == n {
                        result = format!("{}{}{}",
                            &line[..mat.start()],
                            replacement,
                            &line[mat.end()..]
                        );
                        break;
                    }
                }
                Ok(result)
            }
            Some(_) => Ok(line.to_string()), // 0 means no substitution
            None => {
                // Standard behavior
                if global {
                    Ok(re.replace_all(line, replacement).to_string())
                } else {
                    Ok(re.replace(line, replacement).to_string())
                }
            }
        }
    }

    /// Process a file using streaming approach (constant memory)
    ///
    /// Currently implements substitution commands. More command types will be added.
    pub fn process_streaming(&mut self, file_path: &Path) -> Result<FileDiff> {
        // Check file exists and get size
        let metadata = fs::metadata(file_path)
            .with_context(|| format!("Failed to read file metadata: {}", file_path.display()))?;

        if !Self::should_use_streaming(metadata.len()) {
            // File is small, delegate to in-memory processing
            let mut processor = FileProcessor::new(self.commands.clone());
            return processor.process_file_with_context(file_path);
        }

        self.process_streaming_forced(file_path)
    }

    /// Process a file using streaming approach, forcing streaming mode
    /// regardless of file size (used for testing)
    pub fn process_streaming_forced(&mut self, file_path: &Path) -> Result<FileDiff> {
        self.process_streaming_internal(file_path)
    }

    /// Internal streaming implementation (shared by both public methods)
    fn process_streaming_internal(&mut self, file_path: &Path) -> Result<FileDiff> {
        // Get parent directory for temp file
        let parent_dir = file_path.parent()
            .unwrap_or(Path::new("."));

        // Create temp file in same directory as target (for atomic rename)
        let temp_file = NamedTempFile::new_in(parent_dir)
            .with_context(|| format!("Failed to create temp file in {}", parent_dir.display()))?;

        // Open input file
        let input_file = File::open(file_path)
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

        let reader = BufReader::new(input_file);

        let mut line_num = 0;
        let mut changes: Vec<LineChange> = Vec::new();

        // Write using a separate block to ensure writer is dropped before persist
        {
            let mut writer = BufWriter::new(temp_file.as_file());

            // Read line by line
            'outer: for line_result in reader.lines() {
                let line = line_result
                    .with_context(|| format!("Failed to read line from {}", file_path.display()))?;

                line_num += 1;
                self.current_line = line_num;

                // Apply sed commands to this line
                let mut processed_line = line.clone();
                let mut line_changed = false;
                let mut skip_line = false;  // For delete command
                let mut print_line = false;  // For print command
                let mut append_text: Option<String> = None;  // For append command
                let mut should_quit_after_line = false;  // For quit command

                for cmd in &self.commands {
                    match cmd {
                        SedCommand::Substitution { pattern, replacement, flags, range } => {
                            // For now, only handle substitution without range (apply to all lines)
                            if range.is_none() {
                                let original_line = processed_line.clone();
                                processed_line = self.apply_substitution_to_line(
                                    &processed_line,
                                    pattern,
                                    replacement,
                                    flags
                                )?;
                                line_changed = processed_line != original_line;
                            } else {
                                // Ranges not yet supported in streaming - delegate to in-memory
                                drop(writer);
                                let mut processor = FileProcessor::new(self.commands.clone());
                                return processor.process_file_with_context(file_path);
                            }
                        }
                        SedCommand::Delete { range: (start, end) } => {
                            // Check if range affects all lines (simple case)
                            match (start, end) {
                                (Address::LineNumber(1), Address::LastLine) => {
                                    // Delete all lines
                                    skip_line = true;
                                }
                                _ => {
                                    // Complex ranges not yet supported - delegate to in-memory
                                    drop(writer);
                                    let mut processor = FileProcessor::new(self.commands.clone());
                                    return processor.process_file_with_context(file_path);
                                }
                            }
                        }
                        SedCommand::Print { range: (start, end) } => {
                            // Check if range affects all lines (simple case)
                            match (start, end) {
                                (Address::LineNumber(1), Address::LastLine) => {
                                    // Print all lines
                                    print_line = true;
                                }
                                _ => {
                                    // Complex ranges not yet supported - delegate to in-memory
                                    drop(writer);
                                    let mut processor = FileProcessor::new(self.commands.clone());
                                    return processor.process_file_with_context(file_path);
                                }
                            }
                        }
                        SedCommand::Insert { text, address } => {
                            // Insert text BEFORE the specified line
                            match address {
                                Address::LineNumber(n) if *n == line_num => {
                                    // Insert before current line
                                    writeln!(writer, "{}", text)
                                        .with_context(|| "Failed to write inserted line")?;
                                    // Track the inserted line for diff
                                    changes.push(LineChange {
                                        line_number: line_num,
                                        change_type: ChangeType::Added,
                                        content: text.clone(),
                                        old_content: None,
                                    });
                                }
                                Address::LineNumber(_) => {
                                    // Not at the target line yet, continue
                                }
                                _ => {
                                    // Complex addresses (patterns) not yet supported - delegate to in-memory
                                    drop(writer);
                                    let mut processor = FileProcessor::new(self.commands.clone());
                                    return processor.process_file_with_context(file_path);
                                }
                            }
                        }
                        SedCommand::Append { text, address } => {
                            // Append text AFTER the specified line
                            match address {
                                Address::LineNumber(n) if *n == line_num => {
                                    // Store text to append after current line
                                    append_text = Some(text.clone());
                                }
                                Address::LineNumber(_) => {
                                    // Not at the target line yet or already passed it, continue
                                }
                                _ => {
                                    // Complex addresses (patterns) not yet supported - delegate to in-memory
                                    drop(writer);
                                    let mut processor = FileProcessor::new(self.commands.clone());
                                    return processor.process_file_with_context(file_path);
                                }
                            }
                        }
                        SedCommand::Change { text, address } => {
                            // Change (replace) the specified line with new text
                            match address {
                                Address::LineNumber(n) if *n == line_num => {
                                    // Replace current line with new text
                                    processed_line = text.clone();
                                    line_changed = true;
                                }
                                Address::LineNumber(_) => {
                                    // Not at the target line yet, continue
                                }
                                _ => {
                                    // Complex addresses (patterns) not yet supported - delegate to in-memory
                                    drop(writer);
                                    let mut processor = FileProcessor::new(self.commands.clone());
                                    return processor.process_file_with_context(file_path);
                                }
                            }
                        }
                        SedCommand::Quit { address } => {
                            // Stop processing at specified line
                            match address {
                                None => {
                                    // Quit immediately - don't process or write this line
                                    break 'outer;
                                }
                                Some(Address::LineNumber(n)) if *n == line_num => {
                                    // Quit after processing and writing this line
                                    should_quit_after_line = true;
                                }
                                Some(Address::LineNumber(_)) => {
                                    // Not at the target line yet, continue
                                }
                                Some(Address::LastLine) => {
                                    // Quit after processing this line
                                    should_quit_after_line = true;
                                }
                                _ => {
                                    // Complex addresses (patterns) not yet supported - delegate to in-memory
                                    drop(writer);
                                    let mut processor = FileProcessor::new(self.commands.clone());
                                    return processor.process_file_with_context(file_path);
                                }
                            }
                        }
                        // Other commands not yet supported - delegate to in-memory
                        _ => {
                            drop(writer);
                            let mut processor = FileProcessor::new(self.commands.clone());
                            return processor.process_file_with_context(file_path);
                        }
                    }
                }

                // Handle print command (print to stdout)
                if print_line {
                    println!("{}", processed_line);
                }

                // Skip writing if line was deleted
                if skip_line {
                    changes.push(LineChange {
                        line_number: line_num,
                        change_type: ChangeType::Deleted,
                        content: line.clone(),
                        old_content: None,
                    });
                    continue;  // Don't write this line
                }

                // Write the processed line
                writeln!(writer, "{}", processed_line)
                    .with_context(|| format!("Failed to write to temp file"))?;

                // Track line for diff
                let change_type = if line_changed {
                    ChangeType::Modified
                } else {
                    ChangeType::Unchanged
                };

                changes.push(LineChange {
                    line_number: line_num,
                    change_type,
                    content: processed_line,
                    old_content: if line_changed { Some(line) } else { None },
                });

                // Handle append command - write appended text after the current line
                if let Some(text) = &append_text {
                    writeln!(writer, "{}", text)
                        .with_context(|| "Failed to write appended line")?;
                    // Track the appended line for diff
                    changes.push(LineChange {
                        line_number: line_num + 1,
                        change_type: ChangeType::Added,
                        content: text.clone(),
                        old_content: None,
                    });
                }

                // Check if we should quit after processing this line
                if should_quit_after_line {
                    break 'outer;
                }
            }

            // Ensure all data is written to disk
            writer.flush()
                .with_context(|| "Failed to flush temp file")?;
        } // writer dropped here

        // Atomic rename: temp file becomes the actual file
        temp_file.persist(file_path)
            .with_context(|| format!("Failed to persist temp file to {}", file_path.display()))?;

        // Build FileDiff result
        // NOTE: In streaming mode, we don't populate all_lines to save memory
        // The diff formatter will handle this differently for streaming mode
        let all_lines = Vec::new(); // Empty in streaming mode

        Ok(FileDiff {
            file_path: file_path.display().to_string(),
            changes,
            all_lines,
            printed_lines: Vec::new(),
            is_streaming: true,  // Streaming mode
        })
    }
}

impl FileProcessor {
    pub fn new(commands: Vec<SedCommand>) -> Self {
        Self {
            commands,
            printed_lines: Vec::new(),
            hold_space: String::new(),
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
        // Reset hold space for each file
        self.hold_space.clear();

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
            is_streaming: false,  // In-memory mode
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
            SedCommand::Hold { range } => {
                self.apply_hold(lines, range)?;
            }
            SedCommand::HoldAppend { range } => {
                self.apply_hold_append(lines, range)?;
            }
            SedCommand::Get { range } => {
                self.apply_get(lines, range)?;
            }
            SedCommand::GetAppend { range } => {
                self.apply_get_append(lines, range)?;
            }
            SedCommand::Exchange { range } => {
                self.apply_exchange(lines, range)?;
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
                if let (Address::Pattern(start_pat), Address::Pattern(_end_pat)) = (start_inner.as_ref(), end_inner.as_ref()) {
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
            if let (Address::Pattern(start_pat), Address::Pattern(_end_pat)) = (start_inner.as_ref(), end_inner.as_ref()) {
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
            if let (Address::Pattern(start_pat), Address::Pattern(_end_pat)) = (start_inner.as_ref(), end_inner.as_ref()) {
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

    // Hold space operations

    /// h command: Copy pattern space (current line) to hold space (overwrite)
    fn apply_hold(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>) -> Result<()> {
        match range {
            None => {
                // No range - copy last line to hold space
                if let Some(last_line) = lines.last() {
                    self.hold_space = last_line.clone();
                }
            }
            Some((start, end)) => {
                let start_idx = self.resolve_address(&start, lines, 0)?;
                let end_idx = self.resolve_address(&end, lines, lines.len().saturating_sub(1))?;

                // Apply to range - hold space gets set to each line in sequence
                // Final value is the last line in range (GNU sed behavior)
                if end_idx < lines.len() {
                    self.hold_space = lines[end_idx].clone();
                }
            }
        }
        Ok(())
    }

    /// H command: Append pattern space to hold space (with newline)
    fn apply_hold_append(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>) -> Result<()> {
        match range {
            None => {
                // Apply to each line (GNU sed: H applies to pattern space of each line)
                // In our implementation, "no range" means apply once to the whole content
                // For H without range, we append the last line
                if let Some(last_line) = lines.last() {
                    if !self.hold_space.is_empty() {
                        self.hold_space.push('\n');
                    }
                    self.hold_space.push_str(last_line);
                }
            }
            Some((start, end)) => {
                let start_idx = self.resolve_address(&start, lines, 0)?;
                let end_idx = self.resolve_address(&end, lines, lines.len().saturating_sub(1))?;

                // Append all lines in range to hold space
                for i in start_idx..=end_idx.min(lines.len() - 1) {
                    if !self.hold_space.is_empty() {
                        self.hold_space.push('\n');
                    }
                    self.hold_space.push_str(&lines[i]);
                }
            }
        }
        Ok(())
    }

    /// g command: Copy hold space to pattern space (overwrite current line(s))
    fn apply_get(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>) -> Result<()> {
        // Split hold space into lines
        let hold_lines: Vec<String> = if self.hold_space.is_empty() {
            Vec::new()
        } else {
            self.hold_space.lines().map(String::from).collect()
        };

        match range {
            None => {
                // No range - replace all lines with hold space content
                lines.clear();
                lines.extend(hold_lines);
            }
            Some((start, end)) => {
                let start_idx = self.resolve_address(&start, lines, 0)?;
                let end_idx = self.resolve_address(&end, lines, lines.len().saturating_sub(1))?;

                // Replace each line in range with hold space content
                // For multiline hold space with single-line address, use first line
                for i in start_idx..=end_idx.min(lines.len() - 1) {
                    if hold_lines.is_empty() {
                        lines[i] = String::new();
                    } else {
                        // Use first line of hold space (SedX limitation)
                        lines[i] = hold_lines[0].clone();
                    }
                }
            }
        }
        Ok(())
    }

    /// G command: Append hold space to pattern space (with newline)
    fn apply_get_append(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>) -> Result<()> {
        match range {
            None => {
                // Append hold space (or just newline if empty) to each line
                for line in lines.iter_mut() {
                    line.push('\n');
                    if !self.hold_space.is_empty() {
                        line.push_str(&self.hold_space);
                    }
                }
            }
            Some((start, end)) => {
                let start_idx = self.resolve_address(&start, lines, 0)?;
                let end_idx = self.resolve_address(&end, lines, lines.len().saturating_sub(1))?;

                for i in start_idx..=end_idx.min(lines.len() - 1) {
                    lines[i].push('\n');
                    if !self.hold_space.is_empty() {
                        lines[i].push_str(&self.hold_space);
                    }
                }
            }
        }
        Ok(())
    }

    /// x command: Exchange pattern space and hold space
    fn apply_exchange(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>) -> Result<()> {
        match range {
            None => {
                // Exchange all lines with hold space
                let pattern_content = lines.join("\n");
                let hold_content = self.hold_space.clone();

                // Set hold space to pattern space
                self.hold_space = pattern_content;

                // Set pattern space to old hold space
                // If hold space was empty, don't clear lines (GNU sed behavior)
                if !hold_content.is_empty() {
                    lines.clear();
                    for line in hold_content.lines() {
                        lines.push(line.to_string());
                    }
                }
                // If hold space was empty, lines remain unchanged
            }
            Some((start, end)) => {
                let start_idx = self.resolve_address(&start, lines, 0)?;
                let end_idx = self.resolve_address(&end, lines, lines.len().saturating_sub(1))?;

                // Exchange each line in range with hold space
                for i in start_idx..=end_idx.min(lines.len() - 1) {
                    let temp = lines[i].clone();
                    // Only exchange if hold space is not empty
                    if !self.hold_space.is_empty() {
                        lines[i] = self.hold_space.clone();
                    }
                    self.hold_space = temp;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use crate::sed_parser::parse_sed_expression;

    #[test]
    fn test_streaming_passthrough() {
        // Create a temporary test file
        let test_file_path = "/tmp/test_streaming.txt";
        let original_content = "line 1\nline 2\nline 3\nline 4\nline 5\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse an empty command list (no modifications)
        let commands = parse_sed_expression("").expect("Failed to parse empty expression");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();
        // In streaming mode (Chunk 6), we track only changed lines
        // For passthrough with no commands, changes contains all lines as Unchanged
        assert_eq!(diff.changes.len(), 5, "Should have 5 line changes");

        // Verify content is unchanged
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        assert_eq!(processed_content, original_content, "Content should be unchanged");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_threshold_detection() {
        // Test that small files use in-memory processing
        assert!(!StreamProcessor::should_use_streaming(10 * 1024 * 1024)); // 10MB
        assert!(!StreamProcessor::should_use_streaming(99 * 1024 * 1024)); // 99MB

        // Test that large files use streaming
        assert!(StreamProcessor::should_use_streaming(100 * 1024 * 1024)); // 100MB
        assert!(StreamProcessor::should_use_streaming(101 * 1024 * 1024)); // 101MB
        assert!(StreamProcessor::should_use_streaming(1024 * 1024 * 1024)); // 1GB
    }

    #[test]
    fn test_streaming_substitution() {
        // Test basic substitution
        let test_file_path = "/tmp/test_substitution.txt";
        let original_content = "foo bar\nbaz foo\nfoo foo\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse substitution command
        let commands = parse_sed_expression("s/foo/QUX/")
            .expect("Failed to parse substitution");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();
        // In streaming mode, all_lines is empty (Chunk 6), check changes instead
        assert_eq!(diff.changes.len(), 3, "Should have 3 line changes");

        // Verify content
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        let expected = "QUX bar\nbaz QUX\nQUX foo\n";
        assert_eq!(processed_content, expected, "Content should be substituted");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_global_substitution() {
        // Test global substitution (g flag)
        let test_file_path = "/tmp/test_global.txt";
        let original_content = "foo foo foo\nbar foo bar\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse global substitution command
        let commands = parse_sed_expression("s/foo/QUX/g")
            .expect("Failed to parse substitution");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify content
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        let expected = "QUX QUX QUX\nbar QUX bar\n";
        assert_eq!(processed_content, expected, "All occurrences should be substituted");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_numbered_substitution() {
        // Test numbered substitution (s/foo/bar/2)
        let test_file_path = "/tmp/test_numbered.txt";
        let original_content = "foo foo foo foo\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse numbered substitution command
        let commands = parse_sed_expression("s/foo/QUX/2")
            .expect("Failed to parse substitution");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify only 2nd occurrence was replaced
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        let expected = "foo QUX foo foo\n";
        assert_eq!(processed_content, expected, "Only 2nd occurrence should be substituted");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_case_insensitive() {
        // Test case-insensitive substitution (i flag)
        let test_file_path = "/tmp/test_case_insensitive.txt";
        let original_content = "FOO bar Foo baz\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse case-insensitive substitution
        let commands = parse_sed_expression("s/foo/QUX/gi")
            .expect("Failed to parse substitution");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify all case variants were replaced
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        let expected = "QUX bar QUX baz\n";
        assert_eq!(processed_content, expected, "All case variants should be substituted");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_delete() {
        // Test delete command (deletes all lines for now)
        let test_file_path = "/tmp/test_delete.txt";
        let original_content = "line 1\nline 2\nline 3\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse delete command (1,$d means delete from line 1 to last line)
        let commands = parse_sed_expression(r"1,$d")
            .expect("Failed to parse delete");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();
        // In streaming mode, all_lines is empty (Chunk 6), check changes instead
        assert_eq!(diff.changes.len(), 3, "Should track 3 deleted lines");

        // Verify all lines were deleted
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        assert_eq!(processed_content, "", "All lines should be deleted");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_delete_with_substitution() {
        // Test combination of substitution and delete
        let test_file_path = "/tmp/test_delete_sub.txt";
        let original_content = "foo\nbar\nbaz\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse substitution then delete (will delete all lines)
        let commands = parse_sed_expression(r"s/bar/BAR/; 1,$d")
            .expect("Failed to parse commands");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify all lines were deleted
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        assert_eq!(processed_content, "", "All lines should be deleted");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_print() {
        // Test print command (prints to stdout, file unchanged)
        let test_file_path = "/tmp/test_print.txt";
        let original_content = "line 1\nline 2\nline 3\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse print command (1,$p means print from line 1 to last line)
        let commands = parse_sed_expression(r"1,$p")
            .expect("Failed to parse print");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        // Note: this will print to stdout during test
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify file content is unchanged (print doesn't modify file)
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        assert_eq!(processed_content, original_content, "File should be unchanged");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_insert() {
        // Test insert command (inserts text before specified line)
        let test_file_path = "/tmp/test_insert.txt";
        let original_content = "line 1\nline 2\nline 3\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse insert command (2i\TEXT means insert TEXT before line 2)
        let commands = parse_sed_expression(r"2i\INSERTED LINE")
            .expect("Failed to parse insert");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify the line was inserted
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        let expected = "line 1\nINSERTED LINE\nline 2\nline 3\n";
        assert_eq!(processed_content, expected, "Line should be inserted before line 2");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_append() {
        // Test append command (appends text after specified line)
        let test_file_path = "/tmp/test_append.txt";
        let original_content = "line 1\nline 2\nline 3\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse append command (2a\TEXT means append TEXT after line 2)
        let commands = parse_sed_expression(r"2a\APPENDED LINE")
            .expect("Failed to parse append");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify the line was appended
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        let expected = "line 1\nline 2\nAPPENDED LINE\nline 3\n";
        assert_eq!(processed_content, expected, "Line should be appended after line 2");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_change() {
        // Test change command (replaces specified line with new text)
        let test_file_path = "/tmp/test_change.txt";
        let original_content = "line 1\nline 2\nline 3\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse change command (2c\TEXT means change line 2 to TEXT)
        let commands = parse_sed_expression(r"2c\CHANGED LINE")
            .expect("Failed to parse change");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify the line was changed
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        let expected = "line 1\nCHANGED LINE\nline 3\n";
        assert_eq!(processed_content, expected, "Line 2 should be changed");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_quit_at_line() {
        // Test quit command (stops processing at specified line)
        let test_file_path = "/tmp/test_quit.txt";
        let original_content = "line 1\nline 2\nline 3\nline 4\nline 5\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse quit command (3q means quit after processing line 3)
        let commands = parse_sed_expression(r"3q")
            .expect("Failed to parse quit");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify processing stopped at line 3
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        let expected = "line 1\nline 2\nline 3\n";
        assert_eq!(processed_content, expected, "Should stop at line 3");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_quit_immediately() {
        // Test quit command without address (quits immediately)
        let test_file_path = "/tmp/test_quit_immediate.txt";
        let original_content = "line 1\nline 2\nline 3\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse quit command (q means quit immediately, output nothing)
        let commands = parse_sed_expression(r"q")
            .expect("Failed to parse quit");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify file is empty (quit before writing anything)
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        assert_eq!(processed_content, "", "Should be empty (quit immediately)");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_insert_and_substitute() {
        // Test combination of insert and substitute commands
        let test_file_path = "/tmp/test_insert_sub.txt";
        let original_content = "foo\nbar\nbaz\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse insert then substitute
        let commands = parse_sed_expression(r"2i\NEW LINE; s/foo/FOO/")
            .expect("Failed to parse commands");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify both commands were applied
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        let expected = "FOO\nNEW LINE\nbar\nbaz\n";
        assert_eq!(processed_content, expected, "Should insert and substitute");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }
}
