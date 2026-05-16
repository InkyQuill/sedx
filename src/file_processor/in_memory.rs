use crate::command::{Address, Command, SubstitutionFlags};
use crate::file_processor::common::{
    AddressContext, ChangeType, FileDiff, LineChange, SubstitutionEngine, matches_address,
    preserve_perms_after,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Iterator for input lines with lookahead support
/// Required for n and N commands that need to read ahead
#[derive(Clone)]
pub struct LineIterator {
    lines: Vec<String>,
    current: usize,
}

impl LineIterator {
    pub fn new(lines: Vec<String>) -> Self {
        Self { lines, current: 0 }
    }

    /// Get current line for cycle (advances iterator)
    pub fn current_line(&mut self) -> Option<String> {
        if self.current < self.lines.len() {
            let line = self.lines[self.current].clone();
            self.current += 1;
            Some(line)
        } else {
            None
        }
    }

    /// Read next line (for n/N commands) without advancing outer loop
    pub fn read_next(&mut self) -> Option<String> {
        if self.current < self.lines.len() {
            let line = self.lines[self.current].clone();
            self.current += 1;
            Some(line)
        } else {
            None // EOF
        }
    }

    pub fn total_lines(&self) -> usize {
        self.lines.len()
    }
}

/// Result of applying a command within a cycle
/// Matches GNU sed's control flow from execute.c
#[derive(Debug, Clone, PartialEq)]
pub enum CycleResult {
    /// Continue to next command in the cycle
    Continue,

    /// Delete pattern space and end cycle (d command)
    /// Pattern space is NOT printed
    DeleteLine,

    /// Restart command cycle from first command (D command)
    /// Pattern space has been modified (first line removed)
    RestartCycle,

    /// Branch to specific command (Phase 5: b/t/T commands)
    /// Contains the target program counter (command index)
    Branch(usize),

    /// Quit processing immediately (q/Q commands)
    /// Returns exit code (0 for q, N for Q)
    Quit(i32),
}

/// State for a single sed cycle
pub struct CycleState {
    /// Current pattern space (can be multi-line with '\n' separators)
    pub pattern_space: String,

    /// Hold space (persistent across cycles)
    pub hold_space: String,

    /// Current line number (1-indexed)
    pub line_num: usize,

    /// Pattern space marked for deletion (d command)
    pub deleted: bool,

    /// Output from print commands (p, P) and automatic output
    pub output_lines: Vec<String>,

    /// Output from commands that print to stdout but not to the output stream (=, F)
    pub side_effects: Vec<String>,

    /// Text to be inserted BEFORE the pattern space (i command)
    pub inserted_before: Vec<String>,

    /// Text to be appended AFTER the pattern space (a, r commands)
    pub appended_after: Vec<String>,

    /// Current filename (for F command)
    pub current_filename: String,

    /// Input line iterator for n/N commands
    pub line_iter: LineIterator,

    /// Line number range states (for 1,3 ranges)
    /// Maps (start_line, end_line) -> (in_range, ended)
    /// in_range: true if we're currently inside the range
    /// ended: true if we've passed the end of the range
    pub line_range_states: HashMap<(usize, usize), (bool, bool)>,

    /// Substitution flag for t/T commands
    pub substitution_made: bool,
}

impl CycleState {
    pub fn new(hold_space: String, lines: Vec<String>, filename: String) -> Self {
        Self {
            pattern_space: String::new(),
            hold_space,
            line_num: 0,
            deleted: false,
            output_lines: Vec::new(),
            side_effects: Vec::new(),
            inserted_before: Vec::new(),
            appended_after: Vec::new(),
            current_filename: filename,
            line_iter: LineIterator::new(lines),
            line_range_states: HashMap::new(),
            substitution_made: false,
        }
    }
}

pub struct FileProcessor {
    commands: Vec<Command>,
    printed_lines: Vec<String>,
    hold_space: String,
    // Cycle-based architecture
    no_default_output: bool, // -n flag: suppress automatic output
    // Phase 5: Flow control support
    label_registry: HashMap<String, usize>, // Maps label names to command indices
    // Phase 5: File I/O support
    write_handles: HashMap<String, BufWriter<std::fs::File>>, // File handles for w/W commands
    read_positions: HashMap<String, usize>, // Current line position for R command (filename -> line_index)
    // Substitution engine for centralized escape processing
    sub_engine: SubstitutionEngine,
}

impl FileProcessor {
    pub fn with_regex_flavor(
        commands: Vec<Command>,
        regex_flavor: crate::cli::RegexFlavor,
    ) -> Self {
        let mut label_registry = HashMap::new();
        for (i, cmd) in commands.iter().enumerate() {
            if let Command::Label { name, .. } = cmd {
                label_registry.insert(name.clone(), i);
            }
        }

        Self {
            commands,
            printed_lines: Vec::new(),
            hold_space: String::new(),
            no_default_output: false,
            label_registry,
            write_handles: HashMap::new(),
            read_positions: HashMap::new(),
            sub_engine: SubstitutionEngine::new(regex_flavor),
        }
    }

    pub fn set_no_default_output(&mut self, no_output: bool) {
        self.no_default_output = no_output;
    }

    pub fn process_file_with_context(&mut self, file_path: &Path) -> Result<FileDiff> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let original_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let modified_lines = self.apply_cycle_based(original_lines.clone())?;

        let changes = self.generate_line_changes(&original_lines, &modified_lines);
        let all_lines = self.generate_diff_lines(&original_lines, &modified_lines);

        Ok(FileDiff {
            file_path: file_path.to_string_lossy().to_string(),
            changes,
            all_lines,
            printed_lines: self.printed_lines.clone(),
            is_streaming: false,
        })
    }

    fn generate_line_changes(&self, original: &[String], modified: &[String]) -> Vec<LineChange> {
        use similar::{ChangeTag, TextDiff};

        let mut old_text = original.join("\n");
        if !old_text.is_empty() {
            old_text.push('\n');
        }
        let mut new_text = modified.join("\n");
        if !new_text.is_empty() {
            new_text.push('\n');
        }
        let diff = TextDiff::from_lines(&old_text, &new_text);

        let mut deletions = HashMap::new();
        let mut insertions = HashMap::new();
        let mut all_changes = Vec::new();

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => {
                    let line_num = change.old_index().unwrap_or(0) + 1;
                    deletions.insert(line_num, change.value().trim_end_matches('\n').to_string());
                    all_changes.push((line_num, ChangeType::Deleted));
                }
                ChangeTag::Insert => {
                    let line_num = change.new_index().unwrap_or(0) + 1;
                    insertions.insert(line_num, change.value().trim_end_matches('\n').to_string());
                    all_changes.push((line_num, ChangeType::Added));
                }
                ChangeTag::Equal => {}
            }
        }

        let mut changes = Vec::new();
        let mut handled_inserts = std::collections::HashSet::new();
        let mut handled_deletes = std::collections::HashSet::new();

        for &(line_num, ref ct) in &all_changes {
            if *ct == ChangeType::Deleted && !handled_deletes.contains(&line_num) {
                if let Some(new_content) = insertions.get(&line_num) {
                    changes.push(LineChange {
                        line_number: line_num,
                        change_type: ChangeType::Modified,
                        content: new_content.clone(),
                        old_content: Some(deletions.get(&line_num).unwrap().clone()),
                    });
                    handled_deletes.insert(line_num);
                    handled_inserts.insert(line_num);
                } else {
                    changes.push(LineChange {
                        line_number: line_num,
                        change_type: ChangeType::Deleted,
                        content: deletions.get(&line_num).unwrap().clone(),
                        old_content: None,
                    });
                    handled_deletes.insert(line_num);
                }
            } else if *ct == ChangeType::Added && !handled_inserts.contains(&line_num) {
                changes.push(LineChange {
                    line_number: line_num,
                    change_type: ChangeType::Added,
                    content: insertions.get(&line_num).unwrap().clone(),
                    old_content: None,
                });
                handled_inserts.insert(line_num);
            }
        }

        changes
    }

    fn generate_diff_lines(
        &self,
        original: &[String],
        modified: &[String],
    ) -> Vec<(usize, String, ChangeType)> {
        use similar::{ChangeTag, TextDiff};

        let mut old_text = original.join("\n");
        if !old_text.is_empty() {
            old_text.push('\n');
        }
        let mut new_text = modified.join("\n");
        if !new_text.is_empty() {
            new_text.push('\n');
        }
        let diff = TextDiff::from_lines(&old_text, &new_text);

        let mut deletions = HashMap::new();
        let mut insertions = HashMap::new();
        let mut diff_items = Vec::new();

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Equal => {
                    diff_items.push((
                        change.old_index().unwrap_or(0) + 1,
                        change.value().trim_end_matches('\n').to_string(),
                        ChangeType::Unchanged,
                    ));
                }
                ChangeTag::Delete => {
                    let line_num = change.old_index().unwrap_or(0) + 1;
                    deletions.insert(line_num, change.value().trim_end_matches('\n').to_string());
                    diff_items.push((line_num, String::new(), ChangeType::Deleted));
                }
                ChangeTag::Insert => {
                    let line_num = change.new_index().unwrap_or(0) + 1;
                    insertions.insert(line_num, change.value().trim_end_matches('\n').to_string());
                    diff_items.push((line_num, String::new(), ChangeType::Added));
                }
            }
        }

        let mut result = Vec::new();
        let mut handled_inserts = std::collections::HashSet::new();
        let mut handled_deletes = std::collections::HashSet::new();

        for (line_num, _, ct) in diff_items {
            match ct {
                ChangeType::Unchanged => {
                    if let Some(content) = original.get(line_num - 1) {
                        result.push((line_num, content.clone(), ChangeType::Unchanged));
                    }
                }
                ChangeType::Deleted => {
                    if !handled_deletes.contains(&line_num) {
                        if let Some(new_content) = insertions.get(&line_num) {
                            result.push((line_num, new_content.clone(), ChangeType::Modified));
                            handled_deletes.insert(line_num);
                            handled_inserts.insert(line_num);
                        } else {
                            result.push((
                                line_num,
                                deletions.get(&line_num).unwrap().clone(),
                                ChangeType::Deleted,
                            ));
                            handled_deletes.insert(line_num);
                        }
                    }
                }
                ChangeType::Added => {
                    if !handled_inserts.contains(&line_num) {
                        result.push((
                            line_num,
                            insertions.get(&line_num).unwrap().clone(),
                            ChangeType::Added,
                        ));
                        handled_inserts.insert(line_num);
                    }
                }
                _ => {}
            }
        }
        result
    }

    pub fn apply_to_file(&mut self, file_path: &Path) -> Result<usize> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let result_lines = self.apply_cycle_based(lines)?;

        let new_content = result_lines.join("\n") + "\n";
        preserve_perms_after(file_path, || {
            fs::write(file_path, &new_content)
                .with_context(|| format!("Failed to write file: {}", file_path.display()))
        })?;

        Ok(result_lines.len())
    }

    pub fn apply_cycle_based(&mut self, lines: Vec<String>) -> Result<Vec<String>> {
        let commands = self.commands.clone();
        let mut state = CycleState::new(self.hold_space.clone(), lines, String::from("(stdin)"));
        let mut full_output = Vec::new();

        while let Some(line) = state.line_iter.current_line() {
            state.pattern_space = line;
            state.line_num += 1;
            state.substitution_made = false;
            state.deleted = false;
            state.inserted_before.clear();
            state.appended_after.clear();
            state.output_lines.clear();
            state.side_effects.clear();

            let num_commands = commands.len();
            let mut pc: usize = 0;
            while pc < num_commands {
                let cmd = &commands[pc];

                if let Command::Label { .. } = cmd {
                    pc += 1;
                    continue;
                }

                if !self.should_apply_to_cycle(cmd, &mut state) {
                    pc += 1;
                    continue;
                }

                let result = self.apply_command_to_cycle(cmd, &mut state)?;

                match result {
                    CycleResult::Continue => {
                        pc += 1;
                    }
                    CycleResult::Branch(target_pc) => {
                        pc = target_pc;
                    }
                    CycleResult::DeleteLine => {
                        state.deleted = true;
                        break;
                    }
                    CycleResult::RestartCycle => {
                        pc = 0;
                    }
                    CycleResult::Quit(_code) => {
                        for text in state.inserted_before.drain(..) {
                            full_output.push(text);
                        }
                        if !state.deleted && !self.no_default_output {
                            full_output.push(state.pattern_space.clone());
                        }
                        for text in state.appended_after.drain(..) {
                            full_output.push(text);
                        }
                        for text in state.output_lines.drain(..) {
                            self.printed_lines.push(text.clone());
                            full_output.push(text);
                        }
                        for text in state.side_effects.drain(..) {
                            self.printed_lines.push(text.clone());
                            full_output.push(text);
                        }

                        self.hold_space = state.hold_space.clone();
                        return Ok(full_output);
                    }
                }
            }

            for text in state.inserted_before.drain(..) {
                full_output.push(text);
            }

            for text in state.side_effects.drain(..) {
                self.printed_lines.push(text.clone());
                full_output.push(text);
            }

            for text in state.output_lines.drain(..) {
                self.printed_lines.push(text.clone());
                full_output.push(text);
            }

            if !state.deleted && !self.no_default_output {
                full_output.push(state.pattern_space.clone());
            }

            for text in state.appended_after.drain(..) {
                full_output.push(text);
            }
        }

        self.hold_space = state.hold_space.clone();
        Ok(full_output)
    }

    fn should_apply_to_cycle(&mut self, cmd: &Command, state: &mut CycleState) -> bool {
        match cmd {
            Command::Substitution { range, .. } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::Next { range } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::NextAppend { range } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::Hold { range } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::HoldAppend { range } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::Get { range } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::GetAppend { range } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::Exchange { range } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::Group { range, .. } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::Delete { range } => self.check_range_inclusive(state, &range.0, &range.1),
            Command::Print { range } => self.check_range_inclusive(state, &range.0, &range.1),
            Command::PrintFirstLine { range } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::DeleteFirstLine { range } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::Insert { address, .. } | Command::Append { address, .. } => {
                self.address_matches_cycle(address, state)
            }
            Command::Change { range, .. } => self.check_range_inclusive(state, &range.0, &range.1),
            Command::Quit { address } | Command::QuitWithoutPrint { address } => match address {
                None => true,
                Some(addr) => self.address_matches_cycle(addr, state),
            },
            Command::Label { .. } => true,
            Command::Branch { range, .. }
            | Command::Test { range, .. }
            | Command::TestFalse { range, .. } => match range {
                None => true,
                Some((start, end)) => self.check_range_inclusive(state, start, end),
            },
            Command::ReadFile { range, .. }
            | Command::WriteFile { range, .. }
            | Command::ReadLine { range, .. }
            | Command::WriteFirstLine { range, .. } => match range {
                None => true,
                Some(addr) => self.address_matches_cycle(addr, state),
            },
            Command::PrintLineNumber { range, .. }
            | Command::PrintFilename { range, .. }
            | Command::ClearPatternSpace { range, .. } => match range {
                None => true,
                Some(addr) => self.address_matches_cycle(addr, state),
            },
        }
    }

    fn address_matches_cycle(&self, addr: &Address, state: &CycleState) -> bool {
        let total_lines = state.line_iter.total_lines();
        matches_address(
            addr,
            &AddressContext {
                line: &state.pattern_space,
                line_number: state.line_num,
                total_lines: Some(total_lines),
                is_last_line: state.line_num == total_lines,
            },
        )
    }

    fn check_range_inclusive(
        &mut self,
        state: &mut CycleState,
        start: &Address,
        end: &Address,
    ) -> bool {
        match (start, end) {
            (Address::LineNumber(1), Address::LastLine) => true,
            (Address::LineNumber(start_line), Address::LineNumber(end_line)) => {
                if start_line == end_line {
                    return state.line_num == *start_line;
                }
                let key = (*start_line, *end_line);
                let (in_range, ended) =
                    state.line_range_states.entry(key).or_insert((false, false));
                if *ended {
                    return false;
                }
                if state.line_num == *start_line {
                    *in_range = true;
                    return true;
                }
                if state.line_num == *end_line {
                    *ended = true;
                    return true;
                }
                *in_range
            }
            (Address::Pattern(start_pat), Address::Pattern(end_pat)) => {
                if start_pat == end_pat {
                    return self.address_matches_cycle(start, state);
                }
                let start_match = self.address_matches_cycle(start, state);
                let end_match = self.address_matches_cycle(end, state);
                start_match || end_match
            }
            _ => {
                let start_match = self.address_matches_cycle(start, state);
                let end_match = self.address_matches_cycle(end, state);
                start_match || end_match
            }
        }
    }

    fn apply_command_to_cycle(
        &mut self,
        cmd: &Command,
        state: &mut CycleState,
    ) -> Result<CycleResult> {
        match cmd {
            Command::Next { .. } => self.apply_next_cycle(state),
            Command::NextAppend { .. } => self.apply_next_append_cycle(state),
            Command::PrintFirstLine { .. } => self.apply_print_first_line_cycle(state),
            Command::DeleteFirstLine { .. } => self.apply_delete_first_line_cycle(state),
            Command::Delete { .. } => Ok(CycleResult::DeleteLine),
            Command::Print { .. } => {
                state.output_lines.push(state.pattern_space.clone());
                Ok(CycleResult::Continue)
            }
            Command::Substitution {
                pattern,
                replacement,
                flags,
                ..
            } => self.apply_substitution_cycle(state, pattern, replacement, flags),
            Command::Insert { text, .. } => {
                state.inserted_before.push(text.clone());
                Ok(CycleResult::Continue)
            }
            Command::Append { text, .. } => {
                state.appended_after.push(text.clone());
                Ok(CycleResult::Continue)
            }
            Command::Change { text, range } => {
                if self.address_matches_cycle(&range.1, state) {
                    state.inserted_before.push(text.clone());
                }
                Ok(CycleResult::DeleteLine)
            }
            Command::Hold { .. } => {
                state.hold_space = state.pattern_space.clone();
                Ok(CycleResult::Continue)
            }
            Command::HoldAppend { .. } => {
                if !state.hold_space.is_empty() {
                    state.hold_space.push('\n');
                }
                state.hold_space.push_str(&state.pattern_space);
                Ok(CycleResult::Continue)
            }
            Command::Get { .. } => {
                state.pattern_space = state.hold_space.clone();
                Ok(CycleResult::Continue)
            }
            Command::GetAppend { .. } => {
                if !state.pattern_space.is_empty() {
                    state.pattern_space.push('\n');
                }
                state.pattern_space.push_str(&state.hold_space);
                Ok(CycleResult::Continue)
            }
            Command::Exchange { .. } => {
                std::mem::swap(&mut state.pattern_space, &mut state.hold_space);
                Ok(CycleResult::Continue)
            }
            Command::Quit { .. } => Ok(CycleResult::Quit(0)),
            Command::QuitWithoutPrint { .. } => {
                state.deleted = true;
                Ok(CycleResult::Quit(0))
            }
            Command::Label { .. } => Ok(CycleResult::Continue),
            Command::Branch { label, .. } => match label {
                Some(name) => {
                    if let Some(&pc) = self.label_registry.get(name) {
                        Ok(CycleResult::Branch(pc))
                    } else {
                        anyhow::bail!("Undefined label: {}", name)
                    }
                }
                None => Ok(CycleResult::Branch(self.commands.len())),
            },
            Command::Test { label, .. } => {
                if state.substitution_made {
                    match label {
                        Some(name) => {
                            if let Some(&pc) = self.label_registry.get(name) {
                                Ok(CycleResult::Branch(pc))
                            } else {
                                anyhow::bail!("Undefined label: {}", name)
                            }
                        }
                        None => Ok(CycleResult::Branch(self.commands.len())),
                    }
                } else {
                    Ok(CycleResult::Continue)
                }
            }
            Command::TestFalse { label, .. } => {
                if !state.substitution_made {
                    match label {
                        Some(name) => {
                            if let Some(&pc) = self.label_registry.get(name) {
                                Ok(CycleResult::Branch(pc))
                            } else {
                                anyhow::bail!("Undefined label: {}", name)
                            }
                        }
                        None => Ok(CycleResult::Branch(self.commands.len())),
                    }
                } else {
                    Ok(CycleResult::Continue)
                }
            }
            Command::Group { commands, .. } => {
                for inner in commands {
                    let res = self.apply_command_to_cycle(inner, state)?;
                    match res {
                        CycleResult::Continue => {}
                        _ => return Ok(res),
                    }
                }
                Ok(CycleResult::Continue)
            }
            Command::WriteFile { filename, .. } => {
                if let Some(writer) = self.write_handles.get_mut(filename) {
                    writeln!(writer, "{}", state.pattern_space)?;
                    writer.flush()?;
                } else {
                    let safe_path = crate::path_policy::validate_script_file_operand(filename)?;
                    crate::path_policy::ensure_not_symlink(&safe_path)?;
                    let file = File::create(&safe_path)?;
                    let mut writer = BufWriter::new(file);
                    writeln!(writer, "{}", state.pattern_space)?;
                    writer.flush()?;
                    self.write_handles.insert(filename.clone(), writer);
                }
                Ok(CycleResult::Continue)
            }
            Command::WriteFirstLine { filename, .. } => {
                let first = state.pattern_space.lines().next().unwrap_or("");
                if let Some(writer) = self.write_handles.get_mut(filename) {
                    writeln!(writer, "{}", first)?;
                    writer.flush()?;
                } else {
                    let safe_path = crate::path_policy::validate_script_file_operand(filename)?;
                    crate::path_policy::ensure_not_symlink(&safe_path)?;
                    let file = File::create(&safe_path)?;
                    let mut writer = BufWriter::new(file);
                    writeln!(writer, "{}", first)?;
                    writer.flush()?;
                    self.write_handles.insert(filename.clone(), writer);
                }
                Ok(CycleResult::Continue)
            }
            Command::ReadFile { filename, .. } => {
                let safe_path = crate::path_policy::validate_script_file_operand(filename)?;
                let content = fs::read_to_string(&safe_path)?;
                for line in content.lines() {
                    state.appended_after.push(line.to_string());
                }
                Ok(CycleResult::Continue)
            }
            Command::ReadLine { filename, .. } => {
                let safe_path = crate::path_policy::validate_script_file_operand(filename)?;
                let content = fs::read_to_string(&safe_path)?;
                let pos = self.read_positions.entry(filename.clone()).or_insert(0);
                let lines: Vec<&str> = content.lines().collect();
                if let Some(line) = lines.get(*pos) {
                    state.pattern_space.push('\n');
                    state.pattern_space.push_str(line);
                    *pos += 1;
                }
                Ok(CycleResult::Continue)
            }
            Command::PrintLineNumber { .. } => {
                state.side_effects.push(state.line_num.to_string());
                Ok(CycleResult::Continue)
            }
            Command::PrintFilename { .. } => {
                state.side_effects.push(state.current_filename.clone());
                Ok(CycleResult::Continue)
            }
            Command::ClearPatternSpace { .. } => {
                state.pattern_space.clear();
                Ok(CycleResult::Continue)
            }
        }
    }

    fn apply_next_cycle(&mut self, state: &mut CycleState) -> Result<CycleResult> {
        if !self.no_default_output {
            state.output_lines.push(state.pattern_space.clone());
        }
        if let Some(next) = state.line_iter.read_next() {
            state.pattern_space = next;
            state.line_num += 1;
            Ok(CycleResult::Continue)
        } else {
            Ok(CycleResult::DeleteLine)
        }
    }

    fn apply_next_append_cycle(&mut self, state: &mut CycleState) -> Result<CycleResult> {
        if let Some(next) = state.line_iter.read_next() {
            state.pattern_space.push('\n');
            state.pattern_space.push_str(&next);
            state.line_num += 1;
            Ok(CycleResult::Continue)
        } else {
            if !self.no_default_output {
                state.output_lines.push(state.pattern_space.clone());
            }
            Ok(CycleResult::DeleteLine)
        }
    }

    fn apply_print_first_line_cycle(&mut self, state: &mut CycleState) -> Result<CycleResult> {
        if let Some(line) = state.pattern_space.lines().next() {
            state.output_lines.push(line.to_string());
        }
        Ok(CycleResult::Continue)
    }

    fn apply_delete_first_line_cycle(&mut self, state: &mut CycleState) -> Result<CycleResult> {
        if let Some(idx) = state.pattern_space.find('\n') {
            state.pattern_space = state.pattern_space[idx + 1..].to_string();
            Ok(CycleResult::RestartCycle)
        } else {
            Ok(CycleResult::DeleteLine)
        }
    }

    fn apply_substitution_cycle(
        &self,
        state: &mut CycleState,
        pattern: &str,
        replacement: &str,
        flags: &SubstitutionFlags,
    ) -> Result<CycleResult> {
        let original = state.pattern_space.clone();
        let result = self
            .sub_engine
            .apply(&state.pattern_space, pattern, replacement, flags)?;

        if result != original {
            state.pattern_space = result;
            state.substitution_made = true;
            if flags.print {
                state.output_lines.push(state.pattern_space.clone());
            }
        }
        Ok(CycleResult::Continue)
    }
}
