use anyhow::{Context, Result};
use crate::command::{Command, Address, SubstitutionFlags};
use regex::{Regex, RegexBuilder};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::collections::VecDeque;
use tempfile::NamedTempFile;
use std::collections::{HashMap, HashSet};

// Chunk 8: Key for tracking mixed range states per command
#[derive(Clone, Hash, PartialEq, Eq)]
struct MixedRangeKey {
    command_index: usize,
}

// Chunk 8: State for mixed ranges
#[derive(Clone, PartialEq)]
enum MixedRangeState {
    LookingForPattern,
    InRangeUntilLine { target_line: usize },
    InRangeUntilPattern { end_pattern: String },
}

/// Pattern range state for streaming mode (Chunk 8)
#[derive(Clone, PartialEq)]
enum PatternRangeState {
    LookingForStart,   // Looking for start pattern
    InRange,          // Currently inside /start/,/end/ range
    // Chunk 8: Mixed range states
    WaitingForLineNumber { target_line: usize },      // /start/,10 - waiting to reach line 10
    CountingRelativeLines { remaining: usize },       // /start/,+5 - counting N lines after match
}

// ============================================================================
// CYCLE-BASED ARCHITECTURE (Phase 4 Refactoring)
// ============================================================================

/// Iterator for input lines with lookahead support
/// Required for n and N commands that need to read ahead
#[derive(Clone)]
struct LineIterator {
    lines: Vec<String>,
    current: usize,
}

impl LineIterator {
    fn new(lines: Vec<String>) -> Self {
        Self { lines, current: 0 }
    }

    /// Get current line for cycle (advances iterator)
    fn current_line(&mut self) -> Option<String> {
        if self.current < self.lines.len() {
            let line = self.lines[self.current].clone();
            self.current += 1;
            Some(line)
        } else {
            None
        }
    }

    /// Read next line (for n/N commands) without advancing outer loop
    fn read_next(&mut self) -> Option<String> {
        if self.current < self.lines.len() {
            let line = self.lines[self.current].clone();
            self.current += 1;
            Some(line)
        } else {
            None  // EOF
        }
    }

    /// Check if at EOF
    fn is_eof(&self) -> bool {
        self.current >= self.lines.len()
    }

    /// Peek at current position without consuming
    fn peek(&self) -> usize {
        self.current
    }
}

/// Result of applying a command within a cycle
/// Matches GNU sed's control flow from execute.c
#[derive(Debug, Clone, PartialEq)]
enum CycleResult {
    /// Continue to next command in the cycle
    Continue,

    /// Delete pattern space and end cycle (d command)
    /// Pattern space is NOT printed
    DeleteLine,

    /// Restart command cycle from first command (D command)
    /// Pattern space has been modified (first line removed)
    RestartCycle,

    /// Quit processing immediately (q/Q commands)
    /// Returns exit code (0 for q, N for Q)
    Quit(i32),
}

/// State for a single sed cycle
struct CycleState {
    /// Current pattern space (can be multi-line with '\n' separators)
    pattern_space: String,

    /// Hold space (persistent across cycles)
    hold_space: String,

    /// Current line number (1-indexed)
    line_num: usize,

    /// Pattern space marked for deletion (d command)
    deleted: bool,

    /// Side-effect output accumulated during cycle (P, p, n commands)
    side_effects: Vec<String>,

    /// Input line iterator for n/N commands
    line_iter: LineIterator,

    /// Pattern range states (for /start/,/end/ ranges)
    pattern_range_states: HashMap<(String, String), PatternRangeState>,

    /// Mixed range states for tracking complex ranges (Chunk 8)
    mixed_range_states: HashMap<MixedRangeKey, MixedRangeState>,
}

impl CycleState {
    fn new(hold_space: String, lines: Vec<String>) -> Self {
        Self {
            pattern_space: String::new(),
            hold_space,
            line_num: 0,
            deleted: false,
            side_effects: Vec::new(),
            line_iter: LineIterator::new(lines),
            pattern_range_states: HashMap::new(),
            mixed_range_states: HashMap::new(),
        }
    }
}

// ============================================================================
// END CYCLE-BASED ARCHITECTURE
// ============================================================================

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
    commands: Vec<Command>,
    printed_lines: Vec<String>,
    hold_space: String,
    // Multi-line pattern space support (Phase 4)
    pattern_space: Option<String>,  // Multi-line pattern space (None = normal single-line mode)
    current_line_index: usize,       // Current line index in input (for n/N commands)
    // Cycle-based architecture (Phase 4 refactoring)
    no_default_output: bool,         // -n flag: suppress automatic output
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
    commands: Vec<Command>,
    hold_space: String,
    current_line: usize,
    // Sliding window for diff context (Chunk 7)
    context_buffer: VecDeque<(usize, String, ChangeType)>,
    context_size: usize,
    // State for reading context after a change
    context_lines_to_read: usize,  // How many more lines to read as context
    // Pattern range states (Chunk 8): (start_pattern, end_pattern) -> state
    pattern_range_states: HashMap<(String, String), PatternRangeState>,
    // Chunk 8: Mixed range states for tracking complex ranges
    mixed_range_states: HashMap<MixedRangeKey, MixedRangeState>,
    // Dry run mode: if true, don't persist changes to disk
    dry_run: bool,
}

impl StreamProcessor {
    pub fn new(commands: Vec<Command>) -> Self {
        Self {
            commands,
            hold_space: String::new(),
            current_line: 0,
            context_buffer: VecDeque::new(),
            context_size: 2, // Default context size (2 lines before/after changes)
            context_lines_to_read: 0,
            pattern_range_states: HashMap::new(),
            mixed_range_states: HashMap::new(),
            dry_run: false,
        }
    }

    /// Set context size for diff output (default: 2)
    pub fn with_context_size(mut self, size: usize) -> Self {
        self.context_size = size;
        self
    }

    /// Set dry run mode (don't persist changes to disk)
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Flush buffer to changes when we encounter a changed line
    fn flush_buffer_to_changes(&mut self, changes: &mut Vec<LineChange>) {
        for (line_num, content, change_type) in self.context_buffer.drain(..) {
            changes.push(LineChange {
                line_number: line_num,
                change_type,
                content,
                old_content: None,
            });
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
        flags: &SubstitutionFlags,
    ) -> Result<String> {
        let global = flags.global;
        let case_insensitive = flags.case_insensitive;
        let nth_occurrence = flags.nth;

        // Process escape sequences in replacement
        let processed_replacement = self.process_replacement_escapes(replacement);

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
                            processed_replacement,
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
                    Ok(re.replace_all(line, processed_replacement.as_str()).to_string())
                } else {
                    Ok(re.replace(line, processed_replacement.as_str()).to_string())
                }
            }
        }
    }

    /// Process escape sequences in replacement string
    /// Supports: \n, \t, \r, \\, \xHH, \uHHHH
    fn process_replacement_escapes(&self, replacement: &str) -> String {
        let mut result = String::with_capacity(replacement.len());
        let mut chars = replacement.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.peek() {
                    Some('n') => {
                        result.push('\n');
                        chars.next();
                    }
                    Some('t') => {
                        result.push('\t');
                        chars.next();
                    }
                    Some('r') => {
                        result.push('\r');
                        chars.next();
                    }
                    Some('\\') => {
                        result.push('\\');
                        chars.next();
                    }
                    Some('x') => {
                        // Hex escape: \xHH
                        chars.next(); // consume 'x'
                        let mut hex = String::new();
                        for _ in 0..2 {
                            if let Some(&c) = chars.peek() {
                                if c.is_ascii_hexdigit() {
                                    hex.push(c);
                                    chars.next();
                                }
                            }
                        }
                        if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                            result.push(byte as char);
                        }
                    }
                    Some('u') => {
                        // Unicode escape: \uHHHH
                        chars.next(); // consume 'u'
                        let mut hex = String::new();
                        for _ in 0..4 {
                            if let Some(&c) = chars.peek() {
                                if c.is_ascii_hexdigit() {
                                    hex.push(c);
                                    chars.next();
                                }
                            }
                        }
                        if let Ok(codepoint) = u32::from_str_radix(&hex, 16) {
                            if let Some(c) = char::from_u32(codepoint) {
                                result.push(c);
                            }
                        }
                    }
                    Some(&c) => {
                        // Unknown escape, keep as-is
                        result.push('\\');
                        result.push(c);
                        chars.next();
                    }
                    None => {
                        result.push('\\');
                    }
                }
            } else if c == '$' {
                // Handle backreferences: $1, $2, ${name}
                let mut reference = String::from('$');
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_ascii_digit() || next_c == '{' {
                        reference.push(next_c);
                        chars.next();
                        if next_c == '}' {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                result.push_str(&reference);
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Check if a line is within a pattern range, updating state as needed (Chunk 8)
    fn check_pattern_range(&mut self, line: &str, start_pat: &str, end_pat: &str) -> Result<bool> {
        let key = (start_pat.to_string(), end_pat.to_string());
        let state = self.pattern_range_states.entry(key.clone()).or_insert(PatternRangeState::LookingForStart);

        let start_re = Regex::new(start_pat)
            .with_context(|| format!("Invalid regex pattern: {}", start_pat))?;
        let end_re = Regex::new(end_pat)
            .with_context(|| format!("Invalid regex pattern: {}", end_pat))?;

        let in_range = match state {
            PatternRangeState::LookingForStart => {
                if start_re.is_match(line) {
                    *state = PatternRangeState::InRange;
                    true
                } else {
                    false
                }
            }
            PatternRangeState::InRange => {
                if end_re.is_match(line) {
                    *state = PatternRangeState::LookingForStart;
                    true // Include the end line in the range
                } else {
                    true
                }
            }
            // These states should not appear in pattern-to-pattern ranges, but handle them gracefully
            PatternRangeState::WaitingForLineNumber { .. } | PatternRangeState::CountingRelativeLines { .. } => {
                false
            }
        };

        Ok(in_range)
    }

    /// Check mixed pattern-to-line range: /start/,10 (Chunk 8)
    fn check_mixed_pattern_to_line(
        &mut self,
        line: &str,
        start_pat: &str,
        end_line: usize,
        command_index: usize,
    ) -> Result<bool> {
        let key = MixedRangeKey { command_index };
        let state = self.mixed_range_states.entry(key).or_insert(MixedRangeState::LookingForPattern);

        let start_re = Regex::new(start_pat)
            .with_context(|| format!("Invalid regex pattern: {}", start_pat))?;

        let in_range = match state {
            MixedRangeState::LookingForPattern => {
                if start_re.is_match(line) {
                    *state = MixedRangeState::InRangeUntilLine { target_line: end_line };
                    true
                } else {
                    false
                }
            }
            MixedRangeState::InRangeUntilLine { target_line } => {
                if self.current_line >= *target_line {
                    *state = MixedRangeState::LookingForPattern; // Reset for next occurrence
                    true // Include the end line
                } else {
                    true
                }
            }
            _ => false,
        };

        Ok(in_range)
    }

    /// Check mixed line-to-pattern range: 5,/end/ (Chunk 8)
    fn check_mixed_line_to_pattern(
        &mut self,
        line: &str,
        start_line: usize,
        end_pat: &str,
        command_index: usize,
    ) -> Result<bool> {
        let key = MixedRangeKey { command_index };
        let state = self.mixed_range_states.entry(key).or_insert(MixedRangeState::LookingForPattern);

        let in_range = match state {
            MixedRangeState::LookingForPattern => {
                if self.current_line >= start_line {
                    *state = MixedRangeState::InRangeUntilPattern { end_pattern: end_pat.to_string() };
                    true
                } else {
                    false
                }
            }
            MixedRangeState::InRangeUntilPattern { end_pattern } => {
                let end_re = Regex::new(end_pattern)
                    .with_context(|| format!("Invalid regex pattern: {}", end_pattern))?;
                if end_re.is_match(line) {
                    *state = MixedRangeState::LookingForPattern; // Reset for next occurrence
                    true // Include the end line
                } else {
                    true
                }
            }
            _ => false,
        };

        Ok(in_range)
    }

    /// Check relative range: /start/,+5 (Chunk 8)
    fn check_relative_range(
        &mut self,
        line: &str,
        pattern: &str,
        offset: isize,
        command_index: usize,
    ) -> Result<bool> {
        let key = MixedRangeKey { command_index };

        // Remove old state and check fresh each time
        let pat_re = Regex::new(pattern)
            .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        if pat_re.is_match(line) {
            // Pattern matched - start counting
            self.mixed_range_states.insert(key, MixedRangeState::InRangeUntilLine {
                target_line: self.current_line + offset as usize,
            });
            Ok(true)
        } else {
            // Check if we're in a counting state
            if let Some(MixedRangeState::InRangeUntilLine { target_line }) = self.mixed_range_states.get(&key) {
                if self.current_line <= *target_line {
                    Ok(true)
                } else {
                    // Past the target, remove state
                    self.mixed_range_states.remove(&key);
                    Ok(false)
                }
            } else {
                Ok(false)
            }
        }
    }

    /// Check stepping address: 1~2 (every 2nd line from line 1) (Chunk 8)
    fn check_stepping(&self, start: usize, step: usize) -> bool {
        if self.current_line < start {
            false
        } else {
            (self.current_line - start) % step == 0
        }
    }

    /// Check if a command with a pattern range should apply to the current line (Chunk 8)
    fn should_apply_command_with_range(
        &mut self,
        line: &str,
        range: &(Address, Address),
        command_index: usize,
    ) -> Result<bool> {
        use Address::*;

        match (&range.0, &range.1) {
            // Single pattern address: /foo/d (not a range!)
            // When both patterns are the same, match each line independently
            (Pattern(start_pat), Pattern(end_pat)) if start_pat == end_pat => {
                // Compile pattern and match current line only (no state machine)
                let re = Regex::new(start_pat)
                    .with_context(|| format!("Invalid regex pattern: {}", start_pat))?;
                Ok(re.is_match(line))
            }

            // Pattern-to-pattern: /start/,/end/
            (Pattern(start_pat), Pattern(end_pat)) => {
                self.check_pattern_range(line, start_pat, end_pat)
            }

            // Mixed pattern-to-line: /start/,10
            (Pattern(start_pat), LineNumber(end_line)) => {
                self.check_mixed_pattern_to_line(line, start_pat, *end_line, command_index)
            }

            // Mixed line-to-pattern: 5,/end/
            (LineNumber(start_line), Pattern(end_pat)) => {
                self.check_mixed_line_to_pattern(line, *start_line, end_pat, command_index)
            }

            // Relative range: /start/,+5
            (Pattern(start_pat), Relative { base: _, offset }) => {
                self.check_relative_range(line, start_pat, *offset, command_index)
            }

            // Line range: 5,10
            (LineNumber(start), LineNumber(end)) => {
                Ok(self.current_line >= *start && self.current_line <= *end)
            }

            // All lines: 1,$
            (LineNumber(1), LastLine) => {
                Ok(true)
            }

            // Stepping: 1~2
            (Step { start, step }, _) | (_, Step { start, step }) => {
                Ok(self.check_stepping(*start, *step))
            }

            _ => {
                // Other range types not supported in streaming - delegate to in-memory
                Ok(false)
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

                // Clone commands to avoid borrow checker issues with pattern range state updates
                let commands = self.commands.clone();
                for (cmd_index, cmd) in commands.iter().enumerate() {
                    match cmd {
                        Command::Substitution { pattern, replacement, flags, range } => {
                            // Check if we should apply this substitution (Chunk 8: pattern range support)
                            let should_apply = match range {
                                Some(range) => self.should_apply_command_with_range(&line, range, cmd_index)?,
                                None => true, // No range means apply to all lines
                            };

                            if should_apply {
                                let original_line = processed_line.clone();
                                processed_line = self.apply_substitution_to_line(
                                    &processed_line,
                                    pattern,
                                    replacement,
                                    flags
                                )?;
                                line_changed = processed_line != original_line;

                                // Handle print flag in substitution (GNU sed compatible)
                                if line_changed && flags.print {
                                    print_line = true;
                                }
                            }
                        }
                        Command::Delete { range: (start, end) } => {
                            // Check if we should apply this deletion (Chunk 8: unified range support)
                            let range = (start.clone(), end.clone());
                            let should_delete = self.should_apply_command_with_range(&line, &range, cmd_index)?;

                            if should_delete {
                                skip_line = true;
                            }
                        }
                        Command::Print { range: (start, end) } => {
                            // Check if we should print this line (Chunk 8: unified range support)
                            let range = (start.clone(), end.clone());
                            let should_print = self.should_apply_command_with_range(&line, &range, cmd_index)?;

                            if should_print {
                                print_line = true;
                            }
                        }
                        Command::Insert { text, address } => {
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
                        Command::Append { text, address } => {
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
                        Command::Change { text, address } => {
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
                        Command::Quit { address } => {
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
                        // Chunk 9: Hold space operations in streaming mode
                        Command::Hold { range } => {
                            // h - Copy current line to hold space (overwrite)
                            let should_apply = match &range {
                                None => true, // No range means apply to all lines
                                Some((start, end)) => {
                                    self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?
                                }
                            };
                            if should_apply {
                                self.hold_space = processed_line.clone();
                            }
                        }
                        Command::HoldAppend { range } => {
                            // H - Append current line to hold space
                            let should_apply = match &range {
                                None => true, // No range means apply to all lines
                                Some((start, end)) => {
                                    self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?
                                }
                            };
                            if should_apply {
                                if !self.hold_space.is_empty() {
                                    self.hold_space.push('\n');
                                }
                                self.hold_space.push_str(&processed_line);
                            }
                        }
                        Command::Get { range } => {
                            // g - Replace current line with hold space
                            let should_apply = match &range {
                                None => true, // No range means apply to all lines
                                Some((start, end)) => {
                                    self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?
                                }
                            };
                            if should_apply && !self.hold_space.is_empty() {
                                processed_line = self.hold_space.clone();
                                line_changed = true;
                            }
                        }
                        Command::GetAppend { range } => {
                            // G - Append hold space to current line
                            let should_apply = match &range {
                                None => true, // No range means apply to all lines
                                Some((start, end)) => {
                                    self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?
                                }
                            };
                            if should_apply && !self.hold_space.is_empty() {
                                processed_line.push('\n');
                                processed_line.push_str(&self.hold_space);
                                line_changed = true;
                            }
                        }
                        Command::Exchange { range } => {
                            // x - Swap current line with hold space
                            let should_apply = match &range {
                                None => true, // No range means apply to all lines
                                Some((start, end)) => {
                                    self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?
                                }
                            };
                            if should_apply {
                                std::mem::swap(&mut processed_line, &mut self.hold_space);
                                line_changed = true;
                            }
                        }
                        // Chunk 10: Command grouping in streaming mode
                        Command::Group { range, commands: group_commands } => {
                            // Check if we're in the group's range
                            let should_apply = match &range {
                                None => true, // No range means apply to all lines
                                Some((start, end)) => {
                                    self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?
                                }
                            };

                            if should_apply {
                                // Apply each command in the group to the current line
                                // We need to handle the streaming semantics carefully here
                                for group_cmd in group_commands {
                                    match group_cmd {
                                        Command::Substitution { pattern, replacement, flags, range } => {
                                            let should_apply_sub = match range {
                                                None => true,
                                                Some(r) => self.should_apply_command_with_range(&line, &r, cmd_index)?,
                                            };
                                            if should_apply_sub {
                                                let original = processed_line.clone();
                                                processed_line = self.apply_substitution_to_line(
                                                    &processed_line,
                                                    pattern,
                                                    replacement,
                                                    flags
                                                )?;
                                                let was_changed = processed_line != original;
                                                line_changed = line_changed || was_changed;

                                                // Handle print flag in substitution (GNU sed compatible)
                                                if was_changed && flags.print {
                                                    print_line = true;
                                                }
                                            }
                                        }
                                        Command::Delete { range: (start, end) } => {
                                            let range = (start.clone(), end.clone());
                                            let should_delete = self.should_apply_command_with_range(&line, &range, cmd_index)?;
                                            if should_delete {
                                                skip_line = true;
                                                break; // Stop processing group commands
                                            }
                                        }
                                        Command::Print { range: (start, end) } => {
                                            let range = (start.clone(), end.clone());
                                            let should_print = self.should_apply_command_with_range(&line, &range, cmd_index)?;
                                            if should_print {
                                                print_line = true;
                                            }
                                        }
                                        Command::Hold { range } => {
                                            let should_apply = match &range {
                                                None => true,
                                                Some((start, end)) => self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?,
                                            };
                                            if should_apply {
                                                self.hold_space = processed_line.clone();
                                            }
                                        }
                                        Command::HoldAppend { range } => {
                                            let should_apply = match &range {
                                                None => true,
                                                Some((start, end)) => self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?,
                                            };
                                            if should_apply {
                                                if !self.hold_space.is_empty() {
                                                    self.hold_space.push('\n');
                                                }
                                                self.hold_space.push_str(&processed_line);
                                            }
                                        }
                                        Command::Get { range } => {
                                            let should_apply = match &range {
                                                None => true,
                                                Some((start, end)) => self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?,
                                            };
                                            if should_apply && !self.hold_space.is_empty() {
                                                processed_line = self.hold_space.clone();
                                                line_changed = true;
                                            }
                                        }
                                        Command::GetAppend { range } => {
                                            let should_apply = match &range {
                                                None => true,
                                                Some((start, end)) => self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?,
                                            };
                                            if should_apply && !self.hold_space.is_empty() {
                                                processed_line.push('\n');
                                                processed_line.push_str(&self.hold_space);
                                                line_changed = true;
                                            }
                                        }
                                        Command::Exchange { range } => {
                                            let should_apply = match &range {
                                                None => true,
                                                Some((start, end)) => self.should_apply_command_with_range(&line, &(start.clone(), end.clone()), cmd_index)?,
                                            };
                                            if should_apply {
                                                std::mem::swap(&mut processed_line, &mut self.hold_space);
                                                line_changed = true;
                                            }
                                        }
                                        // Other commands in groups (a, i, c, q, nested groups) delegate to in-memory
                                        _ => {
                                            // Delegate entire file to in-memory processing
                                            drop(writer);
                                            let mut processor = FileProcessor::new(self.commands.clone());
                                            return processor.process_file_with_context(file_path);
                                        }
                                    }
                                }
                            }
                            // After processing the group, continue to next command in the loop
                            continue;
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

                // Track line for diff (with sliding window logic for Chunk 7)
                let change_type = if line_changed {
                    ChangeType::Modified
                } else {
                    ChangeType::Unchanged
                };

                // Sliding window logic (Chunk 7)
                let is_changed = line_changed || skip_line || append_text.is_some();

                if is_changed {
                    // CHANGE DETECTED: Flush buffer (previous context) + add changed line
                    self.flush_buffer_to_changes(&mut changes);

                    // Add the changed line itself
                    changes.push(LineChange {
                        line_number: line_num,
                        change_type,
                        content: processed_line,
                        old_content: if line_changed { Some(line) } else { None },
                    });

                    // Set flag to read next context_size lines as context
                    self.context_lines_to_read = self.context_size;

                } else if self.context_lines_to_read > 0 {
                    // Reading context AFTER a change - add directly to changes
                    changes.push(LineChange {
                        line_number: line_num,
                        change_type,
                        content: processed_line,
                        old_content: None,
                    });
                    self.context_lines_to_read -= 1;

                } else {
                    // Unchanged line - add to buffer
                    self.context_buffer.push_back((line_num, processed_line, change_type));

                    // Keep buffer size limited to context_size
                    // In streaming mode, we only show context around changes, not all lines
                    while self.context_buffer.len() > self.context_size {
                        // Buffer too full - remove oldest WITHOUT adding to changes
                        // This ensures only changed lines + nearby context are in the diff
                        self.context_buffer.pop_front();
                    }
                }

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
                    // Flush remaining buffer before quitting
                    self.flush_buffer_to_changes(&mut changes);
                    break 'outer;
                }
            }

            // Flush remaining buffer (unchanged lines at the end of file)
            self.flush_buffer_to_changes(&mut changes);

            // Ensure all data is written to disk
            writer.flush()
                .with_context(|| "Failed to flush temp file")?;
        } // writer dropped here

        // Atomic rename: temp file becomes the actual file
        // In dry-run mode, don't persist (temp file will be automatically deleted when dropped)
        if !self.dry_run {
            temp_file.persist(file_path)
                .with_context(|| format!("Failed to persist temp file to {}", file_path.display()))?;
        }
        // If dry_run, temp_file is dropped here and automatically deleted

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
    pub fn new(commands: Vec<Command>) -> Self {
        Self {
            commands,
            printed_lines: Vec::new(),
            hold_space: String::new(),
            pattern_space: None,
            current_line_index: 0,
            no_default_output: false,
        }
    }

    /// Set the -n flag (suppress automatic output)
    pub fn set_no_default_output(&mut self, value: bool) {
        self.no_default_output = value;
    }

    /// Get the lines that were printed by print commands (for quiet mode)
    pub fn get_printed_lines(&self) -> &[String] {
        &self.printed_lines
    }

    /// Check if all commands support cycle-based processing
    fn supports_cycle_based_processing(commands: &[Command]) -> bool {
        use Command::*;

        for cmd in commands {
            match cmd {
                // Supported commands
                Substitution { .. } | Delete { .. } | Print { .. } |
                Quit { .. } | QuitWithoutPrint { .. } |
                Next { .. } | NextAppend { .. } |
                PrintFirstLine { .. } | DeleteFirstLine { .. } |
                Hold { .. } | HoldAppend { .. } |
                Get { .. } | GetAppend { .. } | Exchange { .. } => {
                    // Supported
                }
                // Unsupported commands (fall back to batch processing)
                Insert { .. } | Append { .. } | Change { .. } | Group { .. } => {
                    return false;
                }
            }
        }

        true
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
        let input_lines: Vec<String> = original_lines.iter().map(|s| s.to_string()).collect();

        // Clear printed lines from previous run
        self.printed_lines.clear();
        // Reset hold space for each file
        self.hold_space.clear();
        // Reset pattern space for each file
        self.pattern_space = None;
        self.current_line_index = 0;

        // Choose processing method based on command support
        let use_cycle_based = Self::supports_cycle_based_processing(&self.commands);

        let modified_lines = if use_cycle_based {
            // Use cycle-based processing (supports multi-line commands like n, N, P, D)
            self.apply_cycle_based(input_lines)?
        } else {
            // Fall back to batch processing (for i, a, c, { } commands)
            let mut lines = input_lines.clone();
            let commands = self.commands.clone();
            for cmd in &commands {
                let should_continue = self.apply_command(&mut lines, cmd)?;
                if !should_continue {
                    break; // Quit command encountered
                }
            }
            lines
        };

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

    // ============================================================================
    // CYCLE-BASED PROCESSING (Phase 4 Refactoring)
    // ============================================================================

    /// Process file using cycle-based execution (matches GNU sed model)
    /// This method preserves all SedX advantages: backups, diffs, PCRE support
    ///
    /// Matches GNU sed execute.c:1685 (main loop) + execute_program (command loop)
    pub fn apply_cycle_based(&mut self, lines: Vec<String>) -> Result<Vec<String>> {
        let mut state = CycleState::new(self.hold_space.clone(), lines);
        let mut output = Vec::new();

        // Outer loop: read each line into pattern space (matches execute.c:1685)
        while let Some(line) = state.line_iter.current_line() {
            state.pattern_space = line;
            state.line_num += 1;

            // Inner loop: apply commands to pattern space (matches execute.c:1289)
            'cycle: for cmd in &self.commands {
                // Check if command applies to current cycle state
                if !self.should_apply_to_cycle(cmd, &state) {
                    continue;
                }

                // Apply command to pattern space
                let result = self.apply_command_to_cycle(cmd, &mut state)?;

                // Handle cycle result (matches execute.c switch statement)
                match result {
                    CycleResult::Continue => {
                        // Continue to next command
                    }
                    CycleResult::DeleteLine => {
                        // End cycle, pattern space not printed (matches d command)
                        state.deleted = true;
                        break 'cycle;
                    }
                    CycleResult::RestartCycle => {
                        // Restart command loop from beginning (matches D command)
                        continue 'cycle;
                    }
                    CycleResult::Quit(_code) => {
                        // Add side effects before quitting
                        for side_effect in state.side_effects.drain(..) {
                            output.push(side_effect.clone());
                            self.printed_lines.push(side_effect);
                        }
                        // Update hold space from final state
                        self.hold_space = state.hold_space.clone();
                        // Return output early (quit program)
                        return Ok(output);
                    }
                }
            }

            // Add side effects (P, p, n commands) - these are printed immediately
            for side_effect in state.side_effects.drain(..) {
                output.push(side_effect.clone());
                self.printed_lines.push(side_effect);
            }

            // Add pattern space to output (unless deleted or in quiet mode)
            if !state.deleted && !self.no_default_output {
                output.push(state.pattern_space.clone());
            }

            // Reset deletion flag for next cycle
            state.deleted = false;
        }

        // Update hold space from final state
        self.hold_space = state.hold_space.clone();

        Ok(output)
    }

    /// Check if command applies to current cycle state (address matching)
    fn should_apply_to_cycle(&self, cmd: &Command, _state: &CycleState) -> bool {
        match cmd {
            // Commands with Option<range> - may or may not have range
            Command::Substitution { .. }
            | Command::Next { .. }
            | Command::NextAppend { .. }
            | Command::Hold { .. }
            | Command::HoldAppend { .. }
            | Command::Get { .. }
            | Command::GetAppend { .. }
            | Command::Exchange { .. }
            | Command::Group { .. } => {
                // TODO: Implement proper range checking
                true
            }

            // Commands with required range (tuple, not Option)
            Command::Delete { .. }
            | Command::Print { .. }
            | Command::PrintFirstLine { .. }
            | Command::DeleteFirstLine { .. } => {
                // TODO: Implement proper range checking
                true
            }

            // Commands that handle their own address checking
            Command::Insert { .. } | Command::Append { .. } | Command::Change { .. } => {
                true
            }

            // Quit commands
            Command::Quit { .. } | Command::QuitWithoutPrint { .. } => {
                // TODO: Implement proper address checking
                true
            }
        }
    }

    /// Apply command within a cycle (returns cycle result)
    /// Matches GNU sed execute.c:1297-1643 (command switch statement)
    fn apply_command_to_cycle(&self, cmd: &Command, state: &mut CycleState) -> Result<CycleResult> {
        match cmd {
            // n command: print current, read next, continue (matches execute.c:1459)
            Command::Next { range: _ } => {
                self.apply_next_cycle(state)
            }

            // N command: append next line (matches execute.c:1474)
            Command::NextAppend { range: _ } => {
                self.apply_next_append_cycle(state)
            }

            // P command: print first line (matches execute.c:1496)
            Command::PrintFirstLine { range: _ } => {
                self.apply_print_first_line_cycle(state)
            }

            // D command: delete first line, restart (matches execute.c:1333)
            Command::DeleteFirstLine { range: _ } => {
                self.apply_delete_first_line_cycle(state)
            }

            // d command: delete pattern space, end cycle (matches execute.c:1328)
            Command::Delete { range: _ } => {
                Ok(CycleResult::DeleteLine)
            }

            // p command: print pattern space (matches execute.c:1491)
            Command::Print { range: _ } => {
                state.side_effects.push(state.pattern_space.clone());
                Ok(CycleResult::Continue)
            }

            // s command: substitution (matches execute.c:1384-1457)
            Command::Substitution { pattern, replacement, flags, range: _ } => {
                self.apply_substitution_cycle(state, pattern, replacement, flags)
            }

            // h command: copy pattern space to hold space (matches execute.c:1522)
            Command::Hold { range: _ } => {
                state.hold_space = state.pattern_space.clone();
                Ok(CycleResult::Continue)
            }

            // H command: append pattern space to hold space (matches execute.c:1524)
            Command::HoldAppend { range: _ } => {
                if !state.hold_space.is_empty() {
                    state.hold_space.push('\n');
                }
                state.hold_space.push_str(&state.pattern_space);
                Ok(CycleResult::Continue)
            }

            // g command: copy hold space to pattern space (matches execute.c:1528)
            Command::Get { range: _ } => {
                state.pattern_space = state.hold_space.clone();
                Ok(CycleResult::Continue)
            }

            // G command: append hold space to pattern space (matches execute.c:1530)
            Command::GetAppend { range: _ } => {
                if !state.pattern_space.is_empty() {
                    state.pattern_space.push('\n');
                }
                state.pattern_space.push_str(&state.hold_space);
                Ok(CycleResult::Continue)
            }

            // x command: exchange pattern and hold spaces (matches execute.c:1532)
            Command::Exchange { range: _ } => {
                std::mem::swap(&mut state.pattern_space, &mut state.hold_space);
                Ok(CycleResult::Continue)
            }

            // q/Q commands: quit (matches execute.c:1504, 1511)
            Command::Quit { .. } => Ok(CycleResult::Quit(0)),
            Command::QuitWithoutPrint { .. } => Ok(CycleResult::Quit(0)),

            // For now, delegate other commands to existing implementation
            // TODO: Port all commands to cycle model
            _ => Ok(CycleResult::Continue),
        }
    }

    /// n command: print current, read next, continue with remaining commands
    /// Matches execute.c:1459-1472
    fn apply_next_cycle(&self, state: &mut CycleState) -> Result<CycleResult> {
        // 1. Side effect: print current pattern space (if not -n mode)
        if !self.no_default_output {
            state.side_effects.push(state.pattern_space.clone());
        }

        // 2. Read next line into pattern space
        if let Some(next_line) = state.line_iter.read_next() {
            state.pattern_space = next_line;
            state.line_num += 1;
            Ok(CycleResult::Continue)  // Continue with remaining commands!
        } else {
            // At EOF: end cycle
            Ok(CycleResult::DeleteLine)  // Don't print anything
        }
    }

    /// N command: append next line to pattern space
    /// Matches execute.c:1474-1489
    fn apply_next_append_cycle(&self, state: &mut CycleState) -> Result<CycleResult> {
        // 1. Append newline separator
        state.pattern_space.push('\n');

        // 2. Read next line and append
        if let Some(next_line) = state.line_iter.read_next() {
            state.pattern_space.push_str(&next_line);
            state.line_num += 1;
            Ok(CycleResult::Continue)
        } else {
            // At EOF: remove appended newline
            state.pattern_space.pop();
            Ok(CycleResult::DeleteLine)
        }
    }

    /// P command: print first line of multi-line pattern space
    /// Matches execute.c:1496-1502
    fn apply_print_first_line_cycle(&self, state: &mut CycleState) -> Result<CycleResult> {
        // Find first newline
        if let Some(idx) = state.pattern_space.find('\n') {
            // Print text up to first newline
            state.side_effects.push(state.pattern_space[..idx].to_string());
        } else {
            // No newline: print entire pattern space
            state.side_effects.push(state.pattern_space.clone());
        }
        Ok(CycleResult::Continue)
    }

    /// D command: delete first line, restart cycle
    /// Matches execute.c:1333-1350
    fn apply_delete_first_line_cycle(&self, state: &mut CycleState) -> Result<CycleResult> {
        // Find first newline
        if let Some(idx) = state.pattern_space.find('\n') {
            // Delete first line up to (and including) newline
            state.pattern_space = state.pattern_space[idx + 1..].to_string();
            Ok(CycleResult::RestartCycle)
        } else {
            // No newline: delete entire pattern space
            Ok(CycleResult::DeleteLine)
        }
    }

    /// s command: substitution
    /// Matches execute.c:1384-1457
    fn apply_substitution_cycle(
        &self,
        state: &mut CycleState,
        pattern: &str,
        replacement: &str,
        flags: &SubstitutionFlags,
    ) -> Result<CycleResult> {
        let global = flags.global;
        let case_insensitive = flags.case_insensitive;
        let print_flag = flags.print;
        let nth_occurrence = flags.nth;

        // Compile regex
        let re = if case_insensitive {
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
                .with_context(|| format!("Invalid regex pattern: {}", pattern))?
        } else {
            Regex::new(pattern)
                .with_context(|| format!("Invalid regex pattern: {}", pattern))?
        };

        // Save original for print flag comparison
        let original = state.pattern_space.clone();

        // Apply substitution
        if let Some(n) = nth_occurrence {
            // Replace only the Nth occurrence (1-indexed)
            let mut count = 0;
            let mut result = state.pattern_space.clone();
            let mut found = false;

            for mat in re.find_iter(&state.pattern_space) {
                count += 1;
                if count == n {
                    result = format!(
                        "{}{}{}",
                        &state.pattern_space[..mat.start()],
                        replacement,
                        &state.pattern_space[mat.end()..]
                    );
                    found = true;
                    break;
                }
            }

            if found {
                state.pattern_space = result;
            }
        } else if global {
            // Replace all occurrences
            state.pattern_space = re.replace_all(&state.pattern_space, replacement).to_string();
        } else {
            // Replace first occurrence only
            state.pattern_space = re.replace(&state.pattern_space, replacement).to_string();
        }

        // Handle print flag (p flag in s///p)
        if print_flag && state.pattern_space != original {
            state.side_effects.push(state.pattern_space.clone());
        }

        Ok(CycleResult::Continue)
    }

    // ============================================================================
    // END CYCLE-BASED PROCESSING
    // ============================================================================

    pub fn apply_command(&mut self, lines: &mut Vec<String>, cmd: &Command) -> Result<bool> {
        // Returns Ok(true) if processing should continue, Ok(false) if quit was requested
        match cmd {
            Command::Substitution { pattern, replacement, flags, range } => {
                self.apply_substitution(lines, pattern, replacement, flags, range)?;
            }
            Command::Delete { range } => {
                self.apply_delete(lines, range)?;
            }
            Command::Insert { text, address } => {
                self.apply_insert(lines, text, address)?;
            }
            Command::Append { text, address } => {
                self.apply_append(lines, text, address)?;
            }
            Command::Change { text, address } => {
                self.apply_change(lines, text, address)?;
            }
            Command::Print { range } => {
                // Collect lines to print (doesn't modify the file)
                self.collect_print_lines(lines, range)?;
            }
            Command::Quit { address } => {
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
            // Phase 4: Q command (quit without printing)
            Command::QuitWithoutPrint { address } => {
                // Q command: quit without printing current pattern space
                // For stdin mode: clear all lines to prevent output
                // For file mode: same as q (truncates file)
                if let Some(addr) = address {
                    let idx = self.resolve_address(addr, lines, 0)?;
                    if idx < lines.len() {
                        // For Q, we need to keep lines up to but NOT including the quit line
                        // This prevents the quit line from being printed
                        lines.truncate(idx);
                    }
                } else {
                    // Quit immediately - clear all lines WITHOUT printing
                    lines.clear();
                }
                // Always stop processing after quit
                return Ok(false);
            }
            Command::Group { range, commands } => {
                // Group needs to handle things differently since it's recursive
                // Reconstruct commands as a vector we can use
                let commands_vec = commands.to_vec();
                return self.apply_group(lines, range, &commands_vec);
            }
            Command::Hold { range } => {
                self.apply_hold(lines, range)?;
            }
            Command::HoldAppend { range } => {
                self.apply_hold_append(lines, range)?;
            }
            Command::Get { range } => {
                self.apply_get(lines, range)?;
            }
            Command::GetAppend { range } => {
                self.apply_get_append(lines, range)?;
            }
            Command::Exchange { range } => {
                self.apply_exchange(lines, range)?;
            }
            // Phase 4: Multi-line pattern space commands
            Command::Next { range } => {
                self.apply_next(lines, range)?;
            }
            Command::NextAppend { range } => {
                self.apply_next_append(lines, range)?;
            }
            Command::PrintFirstLine { range } => {
                self.apply_print_first_line(lines, range)?;
            }
            Command::DeleteFirstLine { range } => {
                self.apply_delete_first_line(lines, range)?;
            }
        }
        Ok(true)
    }

    fn apply_substitution(&mut self, lines: &mut Vec<String>, pattern: &str, replacement: &str, flags: &SubstitutionFlags, range: &Option<(Address, Address)>) -> Result<()> {
        let global = flags.global;
        let case_insensitive = flags.case_insensitive;

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
                            let original = line.clone();
                            if global {
                                *line = re.replace_all(line, replacement).to_string();
                            } else {
                                *line = re.replace(line, replacement).to_string();
                            }

                            // Handle print flag
                            if flags.print && *line != original {
                                self.printed_lines.push(line.clone());
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
                        global,
                        flags.print
                    );
                }
            }
        }

        match range {
            None => {
                // Apply to all lines
                for line in lines.iter_mut() {
                    let original = line.clone();
                    if global {
                        *line = re.replace_all(line, replacement).to_string();
                    } else {
                        *line = re.replace(line, replacement).to_string();
                    }

                    // Handle print flag
                    if flags.print && *line != original {
                        self.printed_lines.push(line.clone());
                    }
                }
            }
            Some((start, end)) => {
                // Apply to specified range
                let start_idx = self.resolve_address(start, lines, 0)?;
                let end_idx = self.resolve_address(end, lines, lines.len())?;

                for i in start_idx..=end_idx.min(lines.len() - 1) {
                    let original = lines[i].clone();
                    if global {
                        lines[i] = re.replace_all(&lines[i], replacement).to_string();
                    } else {
                        lines[i] = re.replace(&lines[i], replacement).to_string();
                    }

                    // Handle print flag
                    if flags.print && lines[i] != original {
                        self.printed_lines.push(lines[i].clone());
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
        &mut self,
        lines: &mut Vec<String>,
        pattern_str: &str,
        pattern_regex: &Regex,
        replacement: &str,
        global: bool,
        print_flag: bool,
    ) -> Result<()> {
        use regex::Regex;

        // Create regex to find matching lines
        let line_pattern_re = Regex::new(pattern_str)
            .with_context(|| format!("Invalid regex pattern: {}", pattern_str))?;

        // Apply substitution to all lines matching the pattern
        for line in lines.iter_mut() {
            if line_pattern_re.is_match(line) {
                let original = line.clone();
                if global {
                    *line = pattern_regex.replace_all(line, replacement).to_string();
                } else {
                    *line = pattern_regex.replace(line, replacement).to_string();
                }

                // Handle print flag
                if print_flag && *line != original {
                    self.printed_lines.push(line.clone());
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

    fn apply_group(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>, commands: &[Command]) -> Result<bool> {
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
            // Chunk 8: Relative address - resolve base then apply offset
            Address::Relative { base, offset } => {
                let base_idx = self.resolve_address(base, lines, default)?;
                let result = base_idx as isize + *offset;
                if result < 0 {
                    Ok(0)
                } else if result as usize > lines.len() {
                    Ok(lines.len())
                } else {
                    Ok(result as usize)
                }
            }
            // Chunk 8: Step address - find first matching line in the sequence
            Address::Step { start, step } => {
                // Find the first line that matches the stepping pattern
                // start is 1-indexed, so convert to 0-indexed
                let start_0idx = if *start == 0 { 0 } else { start - 1 };
                for i in (start_0idx..lines.len()).step_by(*step) {
                    return Ok(i);
                }
                // No more lines match, return end
                Ok(lines.len())
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

    // Phase 4: Multi-line pattern space commands

    /// n command: Print current pattern space, read next line, start new cycle
    fn apply_next(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>) -> Result<()> {
        // n command: outputs current line, then reads next line (deleting it from further processing in this cycle)
        // This effectively keeps odd-numbered lines and removes even-numbered lines
        // For GNU sed compatibility with common patterns like 'n; d'

        // Remove every second line starting from index 1
        // This simulates: print line 1, read line 2 (and discard it), continue with line 3
        let mut indices_to_remove = Vec::new();
        for i in (1..lines.len()).rev().step_by(2) {
            indices_to_remove.push(i);
        }
        for i in indices_to_remove {
            lines.remove(i);
        }
        Ok(())
    }

    /// N command: Read next line and append to pattern space with newline
    fn apply_next_append(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>) -> Result<()> {
        if lines.len() > 1 {
            let first = lines[0].clone();
            lines[0] = format!("{}\n{}", first, lines[1]);
            lines.remove(1);
        }
        // TODO: If at end of file, don't append and exit with error code
        Ok(())
    }

    /// P command: Print first line of pattern space (up to first \n)
    fn apply_print_first_line(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>) -> Result<()> {
        if !lines.is_empty() {
            if let Some(pos) = lines[0].find('\n') {
                let first_line = &lines[0][..pos];
                self.printed_lines.push(first_line.to_string());
            } else {
                self.printed_lines.push(lines[0].clone());
            }
        }
        Ok(())
    }

    /// D command: Delete first line of pattern space, restart cycle
    fn apply_delete_first_line(&mut self, lines: &mut Vec<String>, range: &Option<(Address, Address)>) -> Result<()> {
        if !lines.is_empty() {
            if let Some(pos) = lines[0].find('\n') {
                // Remove first line (up to and including newline)
                lines[0] = lines[0][pos + 1..].to_string();
                // TODO: Restart cycle with remaining pattern space (don't read next line)
            } else {
                // No newline - delete entire pattern space and start new cycle
                lines.remove(0);
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
    use crate::parser::Parser;
    use crate::cli::RegexFlavor;

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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("").expect("Failed to parse empty expression");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();
        // With memory leak fix (Chunk 7), unchanged lines are NOT added to changes
        // unless they're context around actual changes
        // For passthrough with no commands and no changes, expect minimal or no changes tracked
        // The exact number depends on context buffering, but key is: no actual modifications
        assert!(diff.changes.len() <= 5, "Should have at most 5 line changes (likely fewer due to optimized buffering)");

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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("s/foo/QUX/")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("s/foo/QUX/g")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("s/foo/QUX/2")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("s/foo/QUX/gi")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"1,$d")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"s/bar/BAR/; 1,$d")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"1,$p")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"2i\INSERTED LINE")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"2a\APPENDED LINE")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"2c\CHANGED LINE")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"3q")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"q")
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
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"2i\NEW LINE; s/foo/FOO/")
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

    #[test]
    fn test_streaming_sliding_window_context() {
        // Test that sliding window provides context around changes
        let test_file_path = "/tmp/test_context.txt";
        let original_content = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse substitution command (change line 5)
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("s/line 5/CHANGED/")
            .expect("Failed to parse substitution");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();

        // With default context_size=2, we should see:
        // - line 3 (unchanged, context before)
        // - line 4 (unchanged, context before)
        // - line 5 (changed)
        // - line 6 (unchanged, context after)
        // - line 7 (unchanged, context after)
        // Plus line 1-2 which were flushed from buffer before line 3
        // And lines 8-10 which are flushed at end

        // Total should be around 7-10 lines depending on buffer flush timing
        assert!(diff.changes.len() >= 7, "Should have at least 7 lines with context");

        // Verify the structure: should have context around changed line
        let has_line_3 = diff.changes.iter().any(|c| c.line_number == 3);
        let has_line_4 = diff.changes.iter().any(|c| c.line_number == 4);
        let has_line_5 = diff.changes.iter().any(|c| c.line_number == 5 && c.change_type == ChangeType::Modified);
        let has_line_6 = diff.changes.iter().any(|c| c.line_number == 6);
        let has_line_7 = diff.changes.iter().any(|c| c.line_number == 7);

        assert!(has_line_3, "Should include line 3 (context before)");
        assert!(has_line_4, "Should include line 4 (context before)");
        assert!(has_line_5, "Should include line 5 (changed line)");
        assert!(has_line_6, "Should include line 6 (context after)");
        assert!(has_line_7, "Should include line 7 (context after)");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_pattern_range_substitution() {
        // Test pattern range /start/,/end/ with state machine (Chunk 8)
        let test_file_path = "/tmp/test_pattern_range.txt";
        let original_content = "line 1\nSTART\nline 3\nline 4\nEND\nline 6\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse pattern range substitution
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("/START/,/END/s/line/CHANGED/")
            .expect("Failed to parse pattern range substitution");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();

        // Verify changes: lines 3 and 4 should be changed (between START and END)
        assert!(diff.changes.len() >= 2, "Should have at least 2 changes");

        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        assert!(processed_content.contains("CHANGED 3"), "Line 3 should be changed");
        assert!(processed_content.contains("CHANGED 4"), "Line 4 should be changed");
        assert!(processed_content.contains("START"), "START marker should remain");
        assert!(processed_content.contains("END"), "END marker should remain");
        assert!(processed_content.contains("line 1"), "Line 1 before range should be unchanged");
        assert!(processed_content.contains("line 6"), "Line 6 after range should be unchanged");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_pattern_range_delete() {
        // Test pattern range deletion /start/,/end/d
        let test_file_path = "/tmp/test_pattern_range_delete.txt";
        let original_content = "line 1\nSTART\nto delete\nto delete too\nEND\nline 6\n";

        {
            let mut file = fs::File::create(test_file_path)
                .expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        // Parse pattern range delete
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("/START/,/END/d")
            .expect("Failed to parse pattern range delete");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();

        // Verify deletion: lines between START and END (inclusive) should be deleted
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");
        assert!(processed_content.contains("line 1"), "Line 1 should remain");
        assert!(!processed_content.contains("START"), "START should be deleted");
        assert!(!processed_content.contains("to delete"), "Lines in range should be deleted");
        assert!(!processed_content.contains("END"), "END should be deleted");
        assert!(processed_content.contains("line 6"), "Line 6 should remain");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_streaming_group_with_range() {
        // Create test file with a pattern that only appears on specific lines
        let test_file_path = "/tmp/test_group_range.txt";
        let original_content = "keep\nfoo\nfoo\nfoo\nkeep\n";
        fs::write(test_file_path, original_content)
            .expect("Failed to write test file");

        // Parse group with range: 2,3{s/foo/bar/}
        // This should ONLY change lines 2 and 3, not lines 1, 4, or 5
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("2,3{s/foo/bar/}")
            .expect("Failed to parse group with range");
        let mut processor = StreamProcessor::new(commands);

        // Process the file (force streaming for testing)
        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        // Verify: only lines 2 and 3 should be changed
        let processed_content = fs::read_to_string(test_file_path)
            .expect("Failed to read processed file");

        println!("Processed content:\n{}", processed_content);

        // Line 1 should still be "keep" (not "bar")
        assert!(processed_content.starts_with("keep\n"), "Line 1 should NOT be changed");

        // Lines 2 and 3 should be "bar"
        let lines: Vec<&str> = processed_content.lines().collect();
        assert_eq!(lines[0], "keep", "Line 1 should be 'keep'");
        assert_eq!(lines[1], "bar", "Line 2 should be changed to 'bar'");
        assert_eq!(lines[2], "bar", "Line 3 should be changed to 'bar'");
        assert_eq!(lines[3], "foo", "Line 4 should still be 'foo' (not in range)");
        assert_eq!(lines[4], "keep", "Line 5 should be 'keep'");

        // Count: should have exactly 2 "bar" (lines 2 and 3)
        let bar_count = processed_content.matches("bar").count();
        assert_eq!(bar_count, 2, "Should have exactly 2 'bar' (lines 2,3)");

        // Count: should still have 2 "keep" (lines 1 and 5)
        let keep_count = processed_content.matches("keep").count();
        assert_eq!(keep_count, 2, "Should have exactly 2 'keep' (lines 1,5)");

        // Count: should have 1 "foo" remaining (line 4)
        let foo_count = processed_content.matches("foo").count();
        assert_eq!(foo_count, 1, "Should have exactly 1 'foo' (line 4)");

        // Clean up
        fs::remove_file(test_file_path).ok();
    }

    #[test]
    fn test_group_parsing() {
        // Test that group commands are parsed correctly
        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("2,3{s/foo/bar/}").expect("Failed to parse");

        println!("Parsed {} commands:", commands.len());
        for (i, cmd) in commands.iter().enumerate() {
            println!("  Command {}: {:?}", i, cmd);
        }

        // Should parse as exactly ONE command (a Group)
        assert_eq!(commands.len(), 1, "Should parse as 1 command");

        // That one command should be a Group
        match &commands[0] {
            Command::Group { range, commands: inner_commands } => {
                println!("Group range: {:?}", range);
                println!("Inner commands: {}", inner_commands.len());

                // Should have a range of (LineNumber(2), LineNumber(3))
                assert!(range.is_some(), "Group should have a range");

                // Should have exactly 1 inner command
                assert_eq!(inner_commands.len(), 1, "Group should have 1 inner command");
            }
            _ => panic!("First command should be a Group"),
        }
    }
}

// ============================================================================
// CYCLE-BASED ARCHITECTURE TESTS
// ============================================================================

#[cfg(test)]
mod cycle_tests {
    use super::*;
    use crate::command::{Command, Address, SubstitutionFlags};

    /// Helper to parse a simple sed expression
    fn parse_simple(expr: &str) -> Vec<Command> {
        // For now, manually construct commands
        // TODO: Use proper parser when available
        if expr == "n; d" {
            vec![
                Command::Next { range: None },
                Command::Delete { range: (Address::LineNumber(1), Address::LastLine) },
            ]
        } else if expr == "n" {
            vec![
                Command::Next { range: None },
            ]
        } else {
            vec![]
        }
    }

    #[test]
    fn test_cycle_state_creation() {
        let lines = vec!["line1".to_string(), "line2".to_string()];
        let state = CycleState::new(String::new(), lines);
        
        assert_eq!(state.line_num, 0);
        assert_eq!(state.pattern_space, "");
        assert!(!state.deleted);
        assert!(state.side_effects.is_empty());
    }

    #[test]
    fn test_line_iterator() {
        let lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut iter = LineIterator::new(lines);
        
        assert_eq!(iter.current_line(), Some("a".to_string()));
        assert_eq!(iter.current_line(), Some("b".to_string()));
        assert_eq!(iter.current_line(), Some("c".to_string()));
        assert_eq!(iter.current_line(), None);
        assert!(iter.is_eof());
    }

    #[test]
    fn test_line_iterator_read_next() {
        let lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut iter = LineIterator::new(lines);
        
        // current_line advances the iterator
        assert_eq!(iter.current_line(), Some("a".to_string()));
        
        // read_next also advances, so it should return "c" (skips "b")
        assert_eq!(iter.read_next(), Some("b".to_string()));
        
        // Now current_line would return "c"
        assert_eq!(iter.current_line(), Some("c".to_string()));
        
        // At EOF
        assert_eq!(iter.read_next(), None);
    }

    #[test]
    fn test_n_d_command_basic() {
        // Test the famous "n; d" command (should print odd lines)
        let commands = parse_simple("n; d");
        let mut processor = FileProcessor::new(commands);
        
        let input = vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string()];
        let result = processor.apply_cycle_based(input).unwrap();
        
        // Should output odd lines: "1", "3"
        assert_eq!(result, vec!["1", "3"]);
    }

    #[test]
    fn test_n_command_alone() {
        // Test n command alone (should print all lines)
        let commands = parse_simple("n");
        let mut processor = FileProcessor::new(commands);

        let input = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let result = processor.apply_cycle_based(input).unwrap();

        // n outputs current line, reads next, continues
        // Since there are no more commands, it should output all lines
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_substitution_basic() {
        // Test basic substitution: s/foo/bar/
        let commands = vec![
            Command::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: SubstitutionFlags {
                    global: false,
                    case_insensitive: false,
                    print: false,
                    nth: None,
                },
                range: None,  // No range - applies to all lines
            },
        ];
        let mut processor = FileProcessor::new(commands);

        let input = vec!["foo".to_string(), "baz".to_string(), "foo".to_string()];
        let result = processor.apply_cycle_based(input).unwrap();

        // Each line gets first "foo" occurrence replaced (no global flag)
        assert_eq!(result, vec!["bar", "baz", "bar"]);
    }

    #[test]
    fn test_substitution_global() {
        // Test global substitution: s/foo/bar/g
        let commands = vec![
            Command::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: SubstitutionFlags {
                    global: true,
                    case_insensitive: false,
                    print: false,
                    nth: None,
                },
                range: None,
            },
        ];
        let mut processor = FileProcessor::new(commands);

        let input = vec!["foo foo".to_string(), "baz".to_string()];
        let result = processor.apply_cycle_based(input).unwrap();

        // All "foo" occurrences replaced
        assert_eq!(result, vec!["bar bar", "baz"]);
    }

    #[test]
    fn test_substitution_with_print_flag() {
        // Test s command with print flag: s/foo/bar/p
        let commands = vec![
            Command::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: SubstitutionFlags {
                    global: false,
                    case_insensitive: false,
                    print: true,  // p flag
                    nth: None,
                },
                range: None,
            },
        ];
        let mut processor = FileProcessor::new(commands);

        let input = vec!["foo".to_string(), "baz".to_string()];
        let result = processor.apply_cycle_based(input).unwrap();

        // Should print "bar" twice: once from print flag, once from default output
        assert_eq!(result, vec!["bar", "bar", "baz"]);
    }

    #[test]
    fn test_hold_space_h_g() {
        // Test h and g commands (copy to/from hold space)
        // NOTE: This test doesn't use ranges - range checking not yet implemented
        let commands = vec![
            // h: copy pattern space to hold space
            Command::Hold { range: None },
            // g: copy hold space to pattern space
            Command::Get { range: None },
        ];
        let mut processor = FileProcessor::new(commands);

        let input = vec!["first".to_string()];
        let result = processor.apply_cycle_based(input).unwrap();

        // h copies "first" to hold space
        // g copies "first" back to pattern space (no change visible)
        assert_eq!(result, vec!["first"]);
    }

    #[test]
    fn test_hold_space_x() {
        // Test x command (exchange pattern and hold spaces)
        // NOTE: This test doesn't use ranges - range checking not yet implemented
        let commands = vec![
            // h: copy pattern space to hold space
            Command::Hold { range: None },
            // x: exchange pattern and hold spaces
            Command::Exchange { range: None },
        ];
        let mut processor = FileProcessor::new(commands);

        let input = vec!["line1".to_string()];
        let result = processor.apply_cycle_based(input).unwrap();

        // h copies "line1" to hold space (both hold and pattern are "line1")
        // x swaps them (no visible change since both are "line1")
        assert_eq!(result, vec!["line1"]);
    }

    #[test]
    fn test_substitution_and_hold() {
        // Test combination of substitution and hold space
        // NOTE: This test doesn't use ranges - range checking not yet implemented
        let commands = vec![
            // s/foo/bar/ - substitution
            Command::Substitution {
                pattern: "foo".to_string(),
                replacement: "bar".to_string(),
                flags: SubstitutionFlags {
                    global: false,
                    case_insensitive: false,
                    print: false,
                    nth: None,
                },
                range: None,  // Applies to all lines when None
            },
            // h: store modified pattern space in hold space
            Command::Hold { range: None },
            // g: copy hold space to pattern space (redundant after h, but tests the commands)
            Command::Get { range: None },
        ];
        let mut processor = FileProcessor::new(commands);

        let input = vec!["foo baz".to_string()];
        let result = processor.apply_cycle_based(input).unwrap();

        // "foo baz" -> s -> "bar baz" -> h (hold="bar baz") -> g (pattern="bar baz")
        assert_eq!(result, vec!["bar baz"]);
    }
}
