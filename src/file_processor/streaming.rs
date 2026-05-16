use crate::cli::RegexFlavor;
use crate::command::{Address, Command};
use crate::file_processor::common::{
    AddressContext, ChangeType, FileDiff, LineChange, MixedRangeKey, MixedRangeState,
    PatternRangeState, SubstitutionEngine, matches_address, preserve_perms_after,
};
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use tempfile::NamedTempFile;

/// Processor for streaming large files with constant memory usage
pub struct StreamProcessor {
    commands: Vec<Command>,
    hold_space: String,
    current_line: usize,
    current_is_last_line: bool,
    // Sliding window for diff context
    context_buffer: VecDeque<(usize, String, ChangeType)>,
    context_size: usize,
    // State for reading context after a change
    context_lines_to_read: usize,
    // Pattern range states: (start_pattern, end_pattern) -> state
    pattern_range_states: HashMap<(String, String), PatternRangeState>,
    // Mixed range states for tracking complex ranges
    mixed_range_states: HashMap<MixedRangeKey, MixedRangeState>,
    // Dry run mode: if true, don't persist changes to disk
    dry_run: bool,
    // Substitution engine for centralized escape processing
    sub_engine: SubstitutionEngine,
    // Suppress automatic output (-n flag)
    no_default_output: bool,
}

impl StreamProcessor {
    pub fn new(commands: Vec<Command>) -> Self {
        Self {
            commands,
            hold_space: String::new(),
            current_line: 0,
            current_is_last_line: false,
            context_buffer: VecDeque::new(),
            context_size: 2,
            context_lines_to_read: 0,
            pattern_range_states: HashMap::new(),
            mixed_range_states: HashMap::new(),
            dry_run: false,
            sub_engine: SubstitutionEngine::new(RegexFlavor::PCRE),
            no_default_output: false,
        }
    }

    pub fn with_regex_flavor(commands: Vec<Command>, regex_flavor: RegexFlavor) -> Self {
        let mut processor = Self::new(commands);
        processor.sub_engine = SubstitutionEngine::new(regex_flavor);
        processor
    }

    pub fn with_context_size(mut self, size: usize) -> Self {
        self.context_size = size;
        self
    }

    pub fn with_no_default_output(mut self, no_output: bool) -> Self {
        self.no_default_output = no_output;
        self
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

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

    pub fn process_streaming_forced(&mut self, file_path: &Path) -> Result<FileDiff> {
        self.process_streaming_internal(file_path)
    }

    fn process_streaming_internal(&mut self, file_path: &Path) -> Result<FileDiff> {
        crate::path_policy::ensure_not_symlink(file_path)?;

        let parent_dir = file_path.parent().unwrap_or(Path::new("."));
        let temp_file = NamedTempFile::new_in(parent_dir)
            .with_context(|| format!("Failed to create temp file in {}", parent_dir.display()))?;

        let input_file = File::open(file_path)
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

        let reader = BufReader::new(input_file);
        let diff = {
            let writer = BufWriter::new(temp_file.as_file());
            self.process_reader_to_writer(reader, writer, &file_path.to_string_lossy())?
        };

        if !self.dry_run {
            preserve_perms_after(file_path, || {
                temp_file.persist(file_path).with_context(|| {
                    format!("Failed to persist temp file to {}", file_path.display())
                })?;
                Ok(())
            })?;
        }

        Ok(diff)
    }

    pub fn process_reader_to_writer<R: BufRead, W: Write>(
        &mut self,
        reader: R,
        mut writer: W,
        source_name: &str,
    ) -> Result<FileDiff> {
        let mut line_num = 0;
        let mut changes: Vec<LineChange> = Vec::new();
        let mut printed_lines: Vec<String> = Vec::new();
        let commands = self.commands.clone(); // Clone once per stream

        let mut lines = reader.lines().peekable();

        'outer: while let Some(line_result) = lines.next() {
            let line =
                line_result.with_context(|| format!("Failed to read line from {}", source_name))?;

            line_num += 1;
            self.current_line = line_num;
            self.current_is_last_line = lines.peek().is_none();

            let mut processed_line = line.clone();
            let mut current_original_line = line.clone();
            let mut line_changed = false;
            let mut skip_line = false;
            let mut print_line = false;
            let mut append_text: Option<String> = None;
            let mut should_quit_after_line = false;
            let mut terminate_after_line = false;
            let mut suppress_default_output = false;
            let mut change_replacement_emitted = false;

            for (cmd_index, cmd) in commands.iter().enumerate() {
                match cmd {
                    Command::Substitution {
                        pattern,
                        replacement,
                        flags,
                        range,
                    } => {
                        let should_apply = match range {
                            Some(range) => self.should_apply_command_with_range(
                                &processed_line,
                                range,
                                cmd_index,
                            )?,
                            None => true,
                        };

                        if should_apply {
                            let original_line = processed_line.clone();
                            processed_line = self.sub_engine.apply(
                                &processed_line,
                                pattern,
                                replacement,
                                flags,
                            )?;
                            let was_changed = processed_line != original_line;
                            line_changed = line_changed || was_changed;

                            if was_changed && flags.print {
                                print_line = true;
                            }
                        }
                    }
                    Command::Delete {
                        range: (start, end),
                    } => {
                        let range = (start.clone(), end.clone());
                        let should_delete = self.should_apply_command_with_range(
                            &processed_line,
                            &range,
                            cmd_index,
                        )?;

                        if should_delete {
                            skip_line = true;
                        }
                    }
                    Command::Print {
                        range: (start, end),
                    } => {
                        let range = (start.clone(), end.clone());
                        let should_print = self.should_apply_command_with_range(
                            &processed_line,
                            &range,
                            cmd_index,
                        )?;

                        if should_print {
                            print_line = true;
                        }
                    }
                    Command::Insert { text, address } => {
                        if self.address_matches_current(address, &processed_line) {
                            writeln!(writer, "{}", text)
                                .with_context(|| "Failed to write inserted line")?;
                            changes.push(LineChange {
                                line_number: line_num,
                                change_type: ChangeType::Added,
                                content: text.clone(),
                                old_content: None,
                            });
                        }
                    }
                    Command::Append { text, address } => {
                        if self.address_matches_current(address, &processed_line) {
                            append_text = Some(text.clone());
                        }
                    }
                    Command::Change { text, range } => {
                        let should_apply = self.should_apply_command_with_range(
                            &processed_line,
                            &(range.0.clone(), range.1.clone()),
                            cmd_index,
                        )?;
                        if should_apply {
                            let reached_end =
                                self.address_matches_current(&range.1, &processed_line);
                            if reached_end {
                                writeln!(writer, "{}", text)
                                    .with_context(|| "Failed to write changed line")?;
                                change_replacement_emitted = true;
                                changes.push(LineChange {
                                    line_number: line_num,
                                    change_type: ChangeType::Modified,
                                    content: text.clone(),
                                    old_content: Some(current_original_line.clone()),
                                });
                            }
                            skip_line = true;
                        }
                    }
                    Command::Quit { address } => {
                        if address.as_ref().is_none_or(|address| {
                            self.address_matches_current(address, &processed_line)
                        }) {
                            should_quit_after_line = true;
                        }
                    }
                    Command::Next { range } => {
                        let should_apply = match &range {
                            None => true,
                            Some((start, end)) => self.should_apply_command_with_range(
                                &processed_line,
                                &(start.clone(), end.clone()),
                                cmd_index,
                            )?,
                        };
                        if should_apply {
                            if !self.no_default_output {
                                writeln!(writer, "{}", processed_line)
                                    .with_context(|| "Failed to write next output line")?;
                            }
                            match lines.next() {
                                Some(next_result) => {
                                    let next_line = next_result.with_context(|| {
                                        format!("Failed to read line from {}", source_name)
                                    })?;
                                    line_num += 1;
                                    self.current_line = line_num;
                                    self.current_is_last_line = lines.peek().is_none();
                                    current_original_line = next_line.clone();
                                    processed_line = next_line;
                                    line_changed = false;
                                    append_text = None;
                                }
                                None => {
                                    suppress_default_output = true;
                                    terminate_after_line = true;
                                    break;
                                }
                            }
                        }
                    }
                    Command::NextAppend { range } => {
                        let should_apply = match &range {
                            None => true,
                            Some((start, end)) => self.should_apply_command_with_range(
                                &processed_line,
                                &(start.clone(), end.clone()),
                                cmd_index,
                            )?,
                        };
                        if should_apply {
                            match lines.next() {
                                Some(next_result) => {
                                    let next_line = next_result.with_context(|| {
                                        format!("Failed to read line from {}", source_name)
                                    })?;
                                    line_num += 1;
                                    self.current_line = line_num;
                                    self.current_is_last_line = lines.peek().is_none();
                                    current_original_line.push('\n');
                                    current_original_line.push_str(&next_line);
                                    processed_line.push('\n');
                                    processed_line.push_str(&next_line);
                                }
                                None => {
                                    terminate_after_line = true;
                                    break;
                                }
                            }
                        }
                    }
                    Command::PrintFirstLine { range } => {
                        let should_apply = match &range {
                            None => true,
                            Some((start, end)) => self.should_apply_command_with_range(
                                &processed_line,
                                &(start.clone(), end.clone()),
                                cmd_index,
                            )?,
                        };
                        if should_apply {
                            let first_line =
                                processed_line.lines().next().unwrap_or("").to_string();
                            println!("{}", first_line);
                            printed_lines.push(first_line);
                        }
                    }
                    Command::PrintLineNumber { range } => {
                        let should_apply = range.as_ref().is_none_or(|address| {
                            self.address_matches_current(address, &processed_line)
                        });
                        if should_apply {
                            let line_number = self.current_line.to_string();
                            println!("{}", line_number);
                            printed_lines.push(line_number);
                        }
                    }
                    Command::PrintFilename { range } => {
                        let should_apply = range.as_ref().is_none_or(|address| {
                            self.address_matches_current(address, &processed_line)
                        });
                        if should_apply {
                            let filename = source_name.to_string();
                            println!("{}", filename);
                            printed_lines.push(filename);
                        }
                    }
                    Command::Hold { range } => {
                        let should_apply = match &range {
                            None => true,
                            Some((start, end)) => self.should_apply_command_with_range(
                                &processed_line,
                                &(start.clone(), end.clone()),
                                cmd_index,
                            )?,
                        };
                        if should_apply {
                            self.hold_space = processed_line.clone();
                        }
                    }
                    Command::HoldAppend { range } => {
                        let should_apply = match &range {
                            None => true,
                            Some((start, end)) => self.should_apply_command_with_range(
                                &processed_line,
                                &(start.clone(), end.clone()),
                                cmd_index,
                            )?,
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
                            Some((start, end)) => self.should_apply_command_with_range(
                                &processed_line,
                                &(start.clone(), end.clone()),
                                cmd_index,
                            )?,
                        };
                        if should_apply && !self.hold_space.is_empty() {
                            processed_line = self.hold_space.clone();
                            line_changed = true;
                        }
                    }
                    Command::GetAppend { range } => {
                        let should_apply = match &range {
                            None => true,
                            Some((start, end)) => self.should_apply_command_with_range(
                                &processed_line,
                                &(start.clone(), end.clone()),
                                cmd_index,
                            )?,
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
                            Some((start, end)) => self.should_apply_command_with_range(
                                &processed_line,
                                &(start.clone(), end.clone()),
                                cmd_index,
                            )?,
                        };
                        if should_apply {
                            std::mem::swap(&mut processed_line, &mut self.hold_space);
                            line_changed = true;
                        }
                    }
                    Command::ClearPatternSpace { range } => {
                        let should_apply = range.as_ref().is_none_or(|address| {
                            self.address_matches_current(address, &processed_line)
                        });
                        if should_apply {
                            line_changed = line_changed || !processed_line.is_empty();
                            processed_line.clear();
                        }
                    }
                    Command::Group {
                        range,
                        commands: group_commands,
                    } => {
                        let should_apply = match &range {
                            None => true,
                            Some((start, end)) => self.should_apply_command_with_range(
                                &processed_line,
                                &(start.clone(), end.clone()),
                                cmd_index,
                            )?,
                        };

                        if should_apply {
                            for group_cmd in group_commands {
                                match group_cmd {
                                    Command::Substitution {
                                        pattern,
                                        replacement,
                                        flags,
                                        range,
                                    } => {
                                        let should_apply_sub = match range {
                                            None => true,
                                            Some(r) => self.should_apply_command_with_range(
                                                &processed_line,
                                                r,
                                                cmd_index,
                                            )?,
                                        };
                                        if should_apply_sub {
                                            let original = processed_line.clone();
                                            processed_line = self.sub_engine.apply(
                                                &processed_line,
                                                pattern,
                                                replacement,
                                                flags,
                                            )?;
                                            let was_changed = processed_line != original;
                                            line_changed = line_changed || was_changed;

                                            if was_changed && flags.print {
                                                print_line = true;
                                            }
                                        }
                                    }
                                    Command::Delete {
                                        range: (start, end),
                                    } => {
                                        let range = (start.clone(), end.clone());
                                        let should_delete = self.should_apply_command_with_range(
                                            &processed_line,
                                            &range,
                                            cmd_index,
                                        )?;
                                        if should_delete {
                                            skip_line = true;
                                            break;
                                        }
                                    }
                                    Command::Print {
                                        range: (start, end),
                                    } => {
                                        let range = (start.clone(), end.clone());
                                        let should_print = self.should_apply_command_with_range(
                                            &processed_line,
                                            &range,
                                            cmd_index,
                                        )?;
                                        if should_print {
                                            print_line = true;
                                        }
                                    }
                                    Command::Next { range } => {
                                        let should_apply = match &range {
                                            None => true,
                                            Some((start, end)) => self
                                                .should_apply_command_with_range(
                                                    &processed_line,
                                                    &(start.clone(), end.clone()),
                                                    cmd_index,
                                                )?,
                                        };
                                        if should_apply {
                                            if !self.no_default_output {
                                                writeln!(writer, "{}", processed_line)
                                                    .with_context(
                                                        || "Failed to write next output line",
                                                    )?;
                                            }
                                            match lines.next() {
                                                Some(next_result) => {
                                                    let next_line =
                                                        next_result.with_context(|| {
                                                            format!(
                                                                "Failed to read line from {}",
                                                                source_name
                                                            )
                                                        })?;
                                                    line_num += 1;
                                                    self.current_line = line_num;
                                                    self.current_is_last_line =
                                                        lines.peek().is_none();
                                                    current_original_line = next_line.clone();
                                                    processed_line = next_line;
                                                    line_changed = false;
                                                    append_text = None;
                                                }
                                                None => {
                                                    suppress_default_output = true;
                                                    terminate_after_line = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    Command::NextAppend { range } => {
                                        let should_apply = match &range {
                                            None => true,
                                            Some((start, end)) => self
                                                .should_apply_command_with_range(
                                                    &processed_line,
                                                    &(start.clone(), end.clone()),
                                                    cmd_index,
                                                )?,
                                        };
                                        if should_apply {
                                            match lines.next() {
                                                Some(next_result) => {
                                                    let next_line =
                                                        next_result.with_context(|| {
                                                            format!(
                                                                "Failed to read line from {}",
                                                                source_name
                                                            )
                                                        })?;
                                                    line_num += 1;
                                                    self.current_line = line_num;
                                                    self.current_is_last_line =
                                                        lines.peek().is_none();
                                                    processed_line.push('\n');
                                                    processed_line.push_str(&next_line);
                                                }
                                                None => {
                                                    terminate_after_line = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    Command::PrintFirstLine { range } => {
                                        let should_apply = match &range {
                                            None => true,
                                            Some((start, end)) => self
                                                .should_apply_command_with_range(
                                                    &processed_line,
                                                    &(start.clone(), end.clone()),
                                                    cmd_index,
                                                )?,
                                        };
                                        if should_apply {
                                            let first_line = processed_line
                                                .lines()
                                                .next()
                                                .unwrap_or("")
                                                .to_string();
                                            println!("{}", first_line);
                                            printed_lines.push(first_line);
                                        }
                                    }
                                    Command::PrintLineNumber { range } => {
                                        let should_apply = range.as_ref().is_none_or(|address| {
                                            self.address_matches_current(address, &processed_line)
                                        });
                                        if should_apply {
                                            let line_number = self.current_line.to_string();
                                            println!("{}", line_number);
                                            printed_lines.push(line_number);
                                        }
                                    }
                                    Command::PrintFilename { range } => {
                                        let should_apply = range.as_ref().is_none_or(|address| {
                                            self.address_matches_current(address, &processed_line)
                                        });
                                        if should_apply {
                                            let filename = source_name.to_string();
                                            println!("{}", filename);
                                            printed_lines.push(filename);
                                        }
                                    }
                                    Command::Insert { text, address } => {
                                        if self.address_matches_current(address, &processed_line) {
                                            writeln!(writer, "{}", text)
                                                .with_context(|| "Failed to write inserted line")?;
                                            changes.push(LineChange {
                                                line_number: line_num,
                                                change_type: ChangeType::Added,
                                                content: text.clone(),
                                                old_content: None,
                                            });
                                        }
                                    }
                                    Command::Append { text, address } => {
                                        if self.address_matches_current(address, &processed_line) {
                                            append_text = Some(text.clone());
                                        }
                                    }
                                    Command::Change { text, range } => {
                                        let should_apply = self.should_apply_command_with_range(
                                            &processed_line,
                                            &(range.0.clone(), range.1.clone()),
                                            cmd_index,
                                        )?;
                                        if should_apply {
                                            let reached_end = self
                                                .address_matches_current(&range.1, &processed_line);
                                            if reached_end {
                                                writeln!(writer, "{}", text).with_context(
                                                    || "Failed to write changed line",
                                                )?;
                                                change_replacement_emitted = true;
                                                changes.push(LineChange {
                                                    line_number: line_num,
                                                    change_type: ChangeType::Modified,
                                                    content: text.clone(),
                                                    old_content: Some(
                                                        current_original_line.clone(),
                                                    ),
                                                });
                                            }
                                            skip_line = true;
                                        }
                                    }
                                    Command::Hold { range } => {
                                        let should_apply = match &range {
                                            None => true,
                                            Some((start, end)) => self
                                                .should_apply_command_with_range(
                                                    &processed_line,
                                                    &(start.clone(), end.clone()),
                                                    cmd_index,
                                                )?,
                                        };
                                        if should_apply {
                                            self.hold_space = processed_line.clone();
                                        }
                                    }
                                    Command::HoldAppend { range } => {
                                        let should_apply = match &range {
                                            None => true,
                                            Some((start, end)) => self
                                                .should_apply_command_with_range(
                                                    &processed_line,
                                                    &(start.clone(), end.clone()),
                                                    cmd_index,
                                                )?,
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
                                            Some((start, end)) => self
                                                .should_apply_command_with_range(
                                                    &processed_line,
                                                    &(start.clone(), end.clone()),
                                                    cmd_index,
                                                )?,
                                        };
                                        if should_apply && !self.hold_space.is_empty() {
                                            processed_line = self.hold_space.clone();
                                            line_changed = true;
                                        }
                                    }
                                    Command::GetAppend { range } => {
                                        let should_apply = match &range {
                                            None => true,
                                            Some((start, end)) => self
                                                .should_apply_command_with_range(
                                                    &processed_line,
                                                    &(start.clone(), end.clone()),
                                                    cmd_index,
                                                )?,
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
                                            Some((start, end)) => self
                                                .should_apply_command_with_range(
                                                    &processed_line,
                                                    &(start.clone(), end.clone()),
                                                    cmd_index,
                                                )?,
                                        };
                                        if should_apply {
                                            std::mem::swap(
                                                &mut processed_line,
                                                &mut self.hold_space,
                                            );
                                            line_changed = true;
                                        }
                                    }
                                    Command::ReadFile { filename, .. }
                                    | Command::ReadLine { filename, .. }
                                    | Command::WriteFile { filename, .. }
                                    | Command::WriteFirstLine { filename, .. } => {
                                        let safe_path =
                                            crate::path_policy::validate_script_file_operand(
                                                filename,
                                            )?;
                                        crate::path_policy::ensure_not_symlink(&safe_path)?;
                                        bail_unsupported_streaming_command(group_cmd)?;
                                    }
                                    _ => bail_unsupported_streaming_command(group_cmd)?,
                                }
                            }
                        }
                        continue;
                    }
                    Command::WriteFile { filename, .. }
                    | Command::WriteFirstLine { filename, .. } => {
                        let safe_path = crate::path_policy::validate_script_file_operand(filename)?;
                        crate::path_policy::ensure_not_symlink(&safe_path)?;
                        anyhow::bail!(
                            "file I/O command '{}' is not supported in forced streaming mode",
                            filename
                        );
                    }
                    Command::ReadFile { filename, .. } | Command::ReadLine { filename, .. } => {
                        let safe_path = crate::path_policy::validate_script_file_operand(filename)?;
                        crate::path_policy::ensure_not_symlink(&safe_path)?;
                        anyhow::bail!(
                            "file I/O command '{}' is not supported in forced streaming mode",
                            filename
                        );
                    }
                    _ => {
                        // Ignore other commands in streaming for now
                    }
                }
            }

            if print_line {
                println!("{}", processed_line);
                printed_lines.push(processed_line.clone());
            }

            if skip_line {
                if !change_replacement_emitted {
                    changes.push(LineChange {
                        line_number: line_num,
                        change_type: ChangeType::Deleted,
                        content: current_original_line.clone(),
                        old_content: None,
                    });
                }
                continue;
            }

            if !self.no_default_output && !suppress_default_output {
                writeln!(writer, "{}", processed_line)
                    .with_context(|| "Failed to write to writer")?;
            }

            let change_type = if line_changed {
                ChangeType::Modified
            } else {
                ChangeType::Unchanged
            };

            let is_changed = line_changed || skip_line || append_text.is_some();

            if is_changed {
                self.flush_buffer_to_changes(&mut changes);
                changes.push(LineChange {
                    line_number: line_num,
                    change_type,
                    content: processed_line,
                    old_content: if line_changed {
                        Some(current_original_line)
                    } else {
                        None
                    },
                });
                self.context_lines_to_read = self.context_size;
            } else if self.context_lines_to_read > 0 {
                changes.push(LineChange {
                    line_number: line_num,
                    change_type,
                    content: processed_line,
                    old_content: None,
                });
                self.context_lines_to_read -= 1;
            } else {
                self.context_buffer
                    .push_back((line_num, processed_line, change_type));

                while self.context_buffer.len() > self.context_size {
                    self.context_buffer.pop_front();
                }
            }

            if let Some(text) = &append_text {
                if !self.no_default_output {
                    writeln!(writer, "{}", text)
                        .with_context(|| "Failed to write appended line")?;
                }
                changes.push(LineChange {
                    line_number: line_num + 1,
                    change_type: ChangeType::Added,
                    content: text.clone(),
                    old_content: None,
                });
            }

            if should_quit_after_line {
                self.flush_buffer_to_changes(&mut changes);
                break 'outer;
            }

            if terminate_after_line {
                self.flush_buffer_to_changes(&mut changes);
                break 'outer;
            }
        }

        self.flush_buffer_to_changes(&mut changes);
        writer.flush().with_context(|| "Failed to flush writer")?;

        Ok(FileDiff {
            file_path: source_name.to_string(),
            changes,
            all_lines: Vec::new(),
            printed_lines,
            is_streaming: true,
        })
    }

    fn command_name(command: &Command) -> &'static str {
        match command {
            Command::Substitution { .. } => "s",
            Command::Delete { .. } => "d",
            Command::Print { .. } => "p",
            Command::Quit { .. } => "q",
            Command::QuitWithoutPrint { .. } => "Q",
            Command::Insert { .. } => "i",
            Command::Append { .. } => "a",
            Command::Change { .. } => "c",
            Command::Group { .. } => "{}",
            Command::Hold { .. } => "h",
            Command::HoldAppend { .. } => "H",
            Command::Get { .. } => "g",
            Command::GetAppend { .. } => "G",
            Command::Exchange { .. } => "x",
            Command::Next { .. } => "n",
            Command::NextAppend { .. } => "N",
            Command::PrintFirstLine { .. } => "P",
            Command::DeleteFirstLine { .. } => "D",
            Command::Label { .. } => ":",
            Command::Branch { .. } => "b",
            Command::Test { .. } => "t",
            Command::TestFalse { .. } => "T",
            Command::ReadFile { .. } => "r",
            Command::WriteFile { .. } => "w",
            Command::ReadLine { .. } => "R",
            Command::WriteFirstLine { .. } => "W",
            Command::PrintLineNumber { .. } => "=",
            Command::PrintFilename { .. } => "F",
            Command::ClearPatternSpace { .. } => "z",
        }
    }

    fn check_pattern_range(&mut self, line: &str, start_pat: &str, end_pat: &str) -> Result<bool> {
        let key = (start_pat.to_string(), end_pat.to_string());
        let state = self
            .pattern_range_states
            .entry(key.clone())
            .or_insert(PatternRangeState::LookingForStart);

        let start_re = Regex::new(start_pat)
            .with_context(|| format!("Invalid regex pattern: {}", start_pat))?;
        let end_re =
            Regex::new(end_pat).with_context(|| format!("Invalid regex pattern: {}", end_pat))?;

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
                    true
                } else {
                    true
                }
            }
        };

        Ok(in_range)
    }

    fn check_mixed_pattern_to_line(
        &mut self,
        line: &str,
        start_pat: &str,
        end_line: usize,
        command_index: usize,
    ) -> Result<bool> {
        let key = MixedRangeKey { command_index };
        let state = self
            .mixed_range_states
            .entry(key)
            .or_insert(MixedRangeState::LookingForPattern);

        let start_re = Regex::new(start_pat)
            .with_context(|| format!("Invalid regex pattern: {}", start_pat))?;

        let in_range = match state {
            MixedRangeState::LookingForPattern if start_re.is_match(line) => {
                *state = MixedRangeState::InRangeUntilLine {
                    target_line: end_line,
                };
                true
            }
            MixedRangeState::LookingForPattern => false,
            MixedRangeState::InRangeUntilLine { target_line } => {
                if self.current_line >= *target_line {
                    *state = MixedRangeState::LookingForPattern;
                }
                true
            }
            _ => false,
        };

        Ok(in_range)
    }

    fn check_mixed_line_to_pattern(
        &mut self,
        line: &str,
        start_line: usize,
        end_pat: &str,
        command_index: usize,
    ) -> Result<bool> {
        let key = MixedRangeKey { command_index };
        let state = self
            .mixed_range_states
            .entry(key)
            .or_insert(MixedRangeState::LookingForPattern);

        let in_range = match state {
            MixedRangeState::LookingForPattern if self.current_line >= start_line => {
                *state = MixedRangeState::InRangeUntilPattern {
                    end_pattern: end_pat.to_string(),
                };
                true
            }
            MixedRangeState::LookingForPattern => false,
            MixedRangeState::InRangeUntilPattern { end_pattern } => {
                let end_re = Regex::new(end_pattern)
                    .with_context(|| format!("Invalid regex pattern: {}", end_pattern))?;
                if end_re.is_match(line) {
                    *state = MixedRangeState::LookingForPattern;
                }
                true
            }
            _ => false,
        };

        Ok(in_range)
    }

    fn check_relative_range(
        &mut self,
        line: &str,
        pattern: &str,
        offset: isize,
        command_index: usize,
    ) -> Result<bool> {
        let key = MixedRangeKey { command_index };

        let pat_re =
            Regex::new(pattern).with_context(|| format!("Invalid regex pattern: {}", pattern))?;

        if pat_re.is_match(line) {
            self.mixed_range_states.insert(
                key,
                MixedRangeState::InRangeUntilLine {
                    target_line: self.current_line + offset as usize,
                },
            );
            Ok(true)
        } else if let Some(MixedRangeState::InRangeUntilLine { target_line }) =
            self.mixed_range_states.get(&key)
        {
            if self.current_line <= *target_line {
                Ok(true)
            } else {
                self.mixed_range_states.remove(&key);
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    fn check_stepping(&self, start: usize, step: usize) -> bool {
        if self.current_line < start {
            false
        } else {
            (self.current_line - start).is_multiple_of(step)
        }
    }

    fn address_matches_current(&self, address: &Address, line: &str) -> bool {
        matches_address(
            address,
            &AddressContext {
                line,
                line_number: self.current_line,
                total_lines: None,
                is_last_line: self.current_is_last_line,
            },
        )
    }

    fn should_apply_command_with_range(
        &mut self,
        line: &str,
        range: &(Address, Address),
        command_index: usize,
    ) -> Result<bool> {
        use Address::*;

        match (&range.0, &range.1) {
            (Pattern(start_pat), Pattern(end_pat)) if start_pat == end_pat => {
                let re = Regex::new(start_pat)
                    .with_context(|| format!("Invalid regex pattern: {}", start_pat))?;
                Ok(re.is_match(line))
            }
            (Pattern(start_pat), Pattern(end_pat)) => {
                self.check_pattern_range(line, start_pat, end_pat)
            }
            (Pattern(start_pat), LineNumber(end_line)) => {
                self.check_mixed_pattern_to_line(line, start_pat, *end_line, command_index)
            }
            (LineNumber(start_line), Pattern(end_pat)) => {
                self.check_mixed_line_to_pattern(line, *start_line, end_pat, command_index)
            }
            (Pattern(start_pat), Relative { base: _, offset }) => {
                self.check_relative_range(line, start_pat, *offset, command_index)
            }
            (LineNumber(start), LineNumber(end)) => {
                Ok(self.current_line >= *start && self.current_line <= *end)
            }
            (LastLine, LastLine) => Ok(self.current_is_last_line),
            (LineNumber(start), LastLine) => Ok(self.current_line >= *start),
            (LastLine, LineNumber(end)) => {
                Ok(self.current_is_last_line && self.current_line <= *end)
            }
            (Step { start, step }, _) | (_, Step { start, step }) => {
                Ok(self.check_stepping(*start, *step))
            }
            _ => Ok(false),
        }
    }
}

fn bail_unsupported_streaming_command(command: &Command) -> Result<()> {
    anyhow::bail!(
        "command '{}' is not supported in forced streaming mode",
        StreamProcessor::command_name(command)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::RegexFlavor;
    use crate::parser::Parser;
    use std::fs;
    use std::io::Write;

    #[cfg_attr(not(unix), ignore)]
    #[test]
    fn test_streaming_passthrough() {
        let test_file_path = "/tmp/test_streaming.txt";
        let original_content = "line 1\nline 2\nline 3\nline 4\nline 5\n";

        {
            let mut file = fs::File::create(test_file_path).expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse("").expect("Failed to parse empty expression");
        let mut processor = StreamProcessor::new(commands);

        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();
        assert!(
            diff.changes.len() <= 5,
            "Should have at most 5 line changes"
        );

        let processed_content =
            fs::read_to_string(test_file_path).expect("Failed to read processed file");
        assert_eq!(
            processed_content, original_content,
            "Content should be unchanged"
        );

        fs::remove_file(test_file_path).ok();
    }

    #[cfg_attr(not(unix), ignore)]
    #[test]
    fn test_streaming_substitution() {
        let test_file_path = "/tmp/test_substitution.txt";
        let original_content = "foo bar\nbaz foo\nfoo foo\n";

        {
            let mut file = fs::File::create(test_file_path).expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser
            .parse("s/foo/QUX/")
            .expect("Failed to parse substitution");
        let mut processor = StreamProcessor::new(commands);

        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();
        assert_eq!(diff.changes.len(), 3, "Should have 3 line changes");

        let processed_content =
            fs::read_to_string(test_file_path).expect("Failed to read processed file");
        let expected = "QUX bar\nbaz QUX\nQUX foo\n";
        assert_eq!(processed_content, expected, "Content should be substituted");

        fs::remove_file(test_file_path).ok();
    }

    #[cfg_attr(not(unix), ignore)]
    #[test]
    fn test_streaming_global_substitution() {
        let test_file_path = "/tmp/test_global.txt";
        let original_content = "foo foo foo\nbar foo bar\n";

        {
            let mut file = fs::File::create(test_file_path).expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser
            .parse("s/foo/QUX/g")
            .expect("Failed to parse substitution");
        let mut processor = StreamProcessor::new(commands);

        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let processed_content =
            fs::read_to_string(test_file_path).expect("Failed to read processed file");
        let expected = "QUX QUX QUX\nbar QUX bar\n";
        assert_eq!(
            processed_content, expected,
            "All occurrences should be substituted"
        );

        fs::remove_file(test_file_path).ok();
    }

    #[cfg_attr(not(unix), ignore)]
    #[test]
    fn test_streaming_numbered_substitution() {
        let test_file_path = "/tmp/test_numbered.txt";
        let original_content = "foo foo foo foo\n";

        {
            let mut file = fs::File::create(test_file_path).expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser
            .parse("s/foo/QUX/2")
            .expect("Failed to parse substitution");
        let mut processor = StreamProcessor::new(commands);

        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let processed_content =
            fs::read_to_string(test_file_path).expect("Failed to read processed file");
        let expected = "foo QUX foo foo\n";
        assert_eq!(
            processed_content, expected,
            "Only 2nd occurrence should be substituted"
        );

        fs::remove_file(test_file_path).ok();
    }

    #[cfg_attr(not(unix), ignore)]
    #[test]
    fn test_streaming_case_insensitive() {
        let test_file_path = "/tmp/test_case_insensitive.txt";
        let original_content = "FOO bar Foo baz\n";

        {
            let mut file = fs::File::create(test_file_path).expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser
            .parse("s/foo/QUX/gi")
            .expect("Failed to parse substitution");
        let mut processor = StreamProcessor::new(commands);

        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let processed_content =
            fs::read_to_string(test_file_path).expect("Failed to read processed file");
        let expected = "QUX bar QUX baz\n";
        assert_eq!(
            processed_content, expected,
            "All case variants should be substituted"
        );

        fs::remove_file(test_file_path).ok();
    }

    #[cfg_attr(not(unix), ignore)]
    #[test]
    fn test_streaming_delete() {
        let test_file_path = "/tmp/test_delete.txt";
        let original_content = "line 1\nline 2\nline 3\n";

        {
            let mut file = fs::File::create(test_file_path).expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"1,$d").expect("Failed to parse delete");
        let mut processor = StreamProcessor::new(commands);

        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let diff = result.unwrap();
        assert_eq!(diff.changes.len(), 3, "Should track 3 deleted lines");

        let processed_content =
            fs::read_to_string(test_file_path).expect("Failed to read processed file");
        assert_eq!(processed_content, "", "All lines should be deleted");

        fs::remove_file(test_file_path).ok();
    }

    #[cfg_attr(not(unix), ignore)]
    #[test]
    fn test_streaming_print() {
        let test_file_path = "/tmp/test_print.txt";
        let original_content = "line 1\nline 2\nline 3\n";

        {
            let mut file = fs::File::create(test_file_path).expect("Failed to create test file");
            file.write_all(original_content.as_bytes())
                .expect("Failed to write to test file");
        }

        let parser = Parser::new(RegexFlavor::PCRE);
        let commands = parser.parse(r"1,$p").expect("Failed to parse print");
        let mut processor = StreamProcessor::new(commands);

        let result = processor.process_streaming_forced(Path::new(test_file_path));
        assert!(result.is_ok(), "Processing should succeed");

        let processed_content =
            fs::read_to_string(test_file_path).expect("Failed to read processed file");
        assert_eq!(
            processed_content, original_content,
            "File should be unchanged"
        );

        fs::remove_file(test_file_path).ok();
    }
}
