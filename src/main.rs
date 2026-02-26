mod backup_manager;
mod bre_converter;
mod capability;
mod cli;
mod command;
mod config;
mod diff_formatter;
mod disk_space;
mod ere_converter;
mod file_processor;
mod logger;
mod parser;
mod regex_error;
mod sed_parser;

use anyhow::{Context, Result};
use cli::{Args, RegexFlavor, parse_args};
use command::{Address, Command};
use config::{config_file_path, ensure_complete_config, load_config};
use logger::init_debug_logging;
use parser::Parser;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::time::Instant;

fn main() -> Result<()> {
    let args = parse_args()?;

    // Initialize debug logging early (before any operations)
    // We need to check the config, but only for the Execute command
    let log_path = if matches!(args, Args::Execute { .. }) {
        // Load config to check if debug is enabled
        let config = load_config();
        match config {
            Ok(cfg) => {
                let debug_enabled = cfg.processing.debug.unwrap_or(false);
                init_debug_logging(debug_enabled)?
            }
            Err(_) => None, // If config fails, no logging
        }
    } else {
        None
    };

    // Log the log path if we're in debug mode and executing
    if let Some(ref path) = log_path {
        tracing::info!("Debug logging enabled. Log file: {}", path.display());
    }

    match args {
        Args::Execute {
            expression,
            files,
            dry_run,
            interactive,
            context,
            streaming,
            regex_flavor,
            no_backup,
            backup_dir,
            quiet,
        } => {
            // Check if we're in stdin mode (no files specified)
            if files.is_empty() {
                execute_stdin(&expression, regex_flavor, quiet)?;
            } else {
                execute_command(
                    &expression,
                    &files,
                    dry_run,
                    interactive,
                    context,
                    streaming,
                    regex_flavor,
                    no_backup,
                    backup_dir,
                    quiet,
                )?;
            }
        }
        Args::Rollback { id } => {
            rollback(id)?;
        }
        Args::History => {
            show_history()?;
        }
        Args::Status => {
            show_status()?;
        }
        Args::BackupList { verbose } => {
            backup_list(verbose)?;
        }
        Args::BackupShow { id } => {
            backup_show(&id)?;
        }
        Args::BackupRestore { id } => {
            backup_restore(&id)?;
        }
        Args::BackupRemove { id, force } => {
            backup_remove(&id, force)?;
        }
        Args::BackupPrune {
            keep,
            keep_days,
            force,
        } => {
            backup_prune(keep, keep_days, force)?;
        }
        Args::Config { show, log_path } => {
            if log_path {
                config_log_path()?;
            } else if show {
                config_show()?;
            } else {
                config_edit()?;
            }
        }
    }

    Ok(())
}

/// Process stdin and write to stdout (pipeline mode, like sed)
fn execute_stdin(expression: &str, regex_flavor: RegexFlavor, quiet: bool) -> Result<()> {
    // Check if debug logging is enabled
    let debug_enabled = load_config()
        .map(|c| c.processing.debug.unwrap_or(false))
        .unwrap_or(false);

    let start_time = Instant::now();

    if debug_enabled {
        tracing::info!(
            expression = expression,
            regex_flavor = ?regex_flavor,
            mode = "stdin",
            "Stdin processing started"
        );
    }

    // Parse sed expression
    let parser = Parser::new(regex_flavor);
    let commands = match parser.parse(expression) {
        Ok(cmds) => cmds,
        Err(e) => {
            if debug_enabled {
                tracing::error!(
                    expression = expression,
                    error = %e,
                    "Failed to parse expression"
                );
            }
            return Err(e.context("Failed to parse expression"));
        }
    };

    // Read all input from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    // Process the input using cycle-based or batch processing
    let lines: Vec<String> = input.lines().map(|s| s.to_string()).collect();
    let mut processor =
        file_processor::FileProcessor::with_regex_flavor(commands.clone(), regex_flavor);
    processor.set_no_default_output(quiet); // Wire up -n flag

    let result_lines = processor.apply_cycle_based(lines)?;
    let output_line_count = result_lines.len();

    // Write output to stdout
    for line in result_lines {
        println!("{}", line);
    }

    if debug_enabled {
        let elapsed = start_time.elapsed();
        tracing::info!(
            status = "success",
            output_lines = output_line_count,
            elapsed_ms = elapsed.as_millis(),
            "Stdin processing completed"
        );
    }

    Ok(())
}

/// Check if commands can be executed in streaming mode
fn can_use_streaming(commands: &[Command]) -> bool {
    use Command::*;

    for cmd in commands {
        match cmd {
            // Chunk 10: Groups SHOULD use streaming mode to avoid in-memory bugs
            // The in-memory group implementation has issues with nested command ranges
            Group { .. } => {
                // Force streaming mode for groups
                // The streaming group handler is correct, in-memory has bugs
                return true;
            }
            // Chunk 9: Hold space operations ARE streamable
            Hold { .. } | HoldAppend { .. } | Get { .. } | GetAppend { .. } | Exchange { .. } => {
                // These are now supported in streaming mode
                // But need to check address types
                if let Some(range) = get_command_range_option(cmd)
                    && !is_range_supported_in_streaming(&range)
                {
                    return false;
                }
            }
            _ => {
                // s, d, p, a, i, c, q are supported
                // But need to check address types
                if let Some(range) = get_command_range_option(cmd)
                    && !is_range_supported_in_streaming(&range)
                {
                    return false;
                }
            }
        }
    }

    true
}

/// Extract range from a command (if any)
fn get_command_range_option(cmd: &Command) -> Option<(Address, Address)> {
    match cmd {
        Command::Substitution { range, .. } => range.as_ref().map(|r| (r.0.clone(), r.1.clone())),
        Command::Delete { range } => Some(range.clone()),
        Command::Print { range } => Some(range.clone()),
        Command::Insert {
            address: Address::LineNumber(_),
            ..
        } => Some((Address::LineNumber(0), Address::LineNumber(0))),
        Command::Append {
            address: Address::LineNumber(_),
            ..
        } => Some((Address::LineNumber(0), Address::LineNumber(0))),
        Command::Change {
            address: Address::LineNumber(_),
            ..
        } => Some((Address::LineNumber(0), Address::LineNumber(0))),
        Command::Quit {
            address: Some(Address::LineNumber(_)) | None,
            ..
        }
        | Command::Quit {
            address: Some(Address::LastLine),
            ..
        } => Some((Address::LineNumber(0), Address::LineNumber(0))),
        _ => None,
    }
}

/// Check if a range is supported in streaming mode
fn is_range_supported_in_streaming(range: &(Address, Address)) -> bool {
    use Address::*;

    match (&range.0, &range.1) {
        // Chunk 8: Supported ranges
        (Pattern(_), Pattern(_)) => true,       // /start/,/end/
        (LineNumber(1), LastLine) => true,      // 1,$
        (LineNumber(_), LineNumber(_)) => true, // 5,10
        (Pattern(_), LineNumber(_)) => true,    // /start/,10 (Chunk 8)
        (LineNumber(_), Pattern(_)) => true,    // 5,/end/ (Chunk 8)
        (Pattern(_), Relative { base: _, offset: _ }) => true, // /start/,+5 (Chunk 8)

        // Stepping addresses (Chunk 8)
        (Step { .. }, _) | (_, Step { .. }) => true, // 1~2

        // Not supported (delegate to in-memory):
        (Negated(_), _) | (_, Negated(_)) => false, // /pattern/!s/foo/bar/
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_command(
    expression: &str,
    files: &[String],
    dry_run: bool,
    interactive: bool,
    context: usize,
    streaming: bool,
    regex_flavor: RegexFlavor,
    no_backup: bool,
    backup_dir: Option<String>,
    quiet: bool,
) -> Result<()> {
    let start_time = Instant::now();

    // Load configuration file
    let config = load_config()?;

    // Use backup_dir from config if not specified via CLI
    let backup_dir = backup_dir.or_else(|| config.backup.backup_dir.clone());

    // Check if debug logging is enabled
    let debug_enabled = config.processing.debug.unwrap_or(false);

    // Log the start of operation
    if debug_enabled {
        tracing::info!(
            expression = expression,
            regex_flavor = ?regex_flavor,
            dry_run = dry_run,
            files_count = files.len(),
            "Operation started"
        );
    }

    // Parse sed expression using unified parser
    let parser = Parser::new(regex_flavor);
    let commands = match parser.parse(expression) {
        Ok(cmds) => cmds,
        Err(e) => {
            if debug_enabled {
                tracing::error!(
                    expression = expression,
                    error = %e,
                    "Failed to parse expression"
                );
            }
            return Err(e.context("Failed to parse expression"));
        }
    };

    if debug_enabled {
        tracing::info!(
            command_count = commands.len(),
            "Expression parsed successfully"
        );
    }

    // Check if commands can modify files
    // Commands like 'p', 'n', 'q', 'Q', '=', 'l' only read/print, don't modify
    let can_modify_files = commands_can_modify_files(&commands);

    // Check if commands support streaming mode
    let supports_streaming = can_use_streaming(&commands);

    let file_paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();

    // Process all files and generate diffs (PREVIEW PHASE - always dry_run)
    // For each file, decide whether to use streaming or in-memory processing
    let mut diffs = Vec::new();
    let mut streaming_files: Vec<PathBuf> = Vec::new(); // Track which files should use streaming

    for file_path in &file_paths {
        // Get file metadata to check size
        let metadata = match fs::metadata(file_path) {
            Ok(meta) => meta,
            Err(e) => {
                if debug_enabled {
                    tracing::warn!(
                        file = %file_path.display(),
                        error = %e,
                        "Failed to read file"
                    );
                }
                eprintln!("Error reading file {}: {}", file_path.display(), e);
                continue;
            }
        };

        let file_size_mb = metadata.len() / 1024 / 1024;

        // Get streaming threshold from config (default: 100MB)
        let streaming_threshold_mb = config.processing.max_memory_mb.unwrap_or(100);
        let streaming_threshold_bytes = (streaming_threshold_mb * 1024 * 1024) as u64;

        // Decide: use streaming if (streaming flag OR file >= threshold OR commands support it)
        let use_streaming = if !supports_streaming {
            false // Commands don't support streaming
        } else if streaming {
            true // Explicitly enabled
        } else if metadata.len() >= streaming_threshold_bytes {
            // Auto-detect: file >= threshold
            eprintln!(
                "📊 Streaming mode activated for {} ({} MB, threshold: {} MB)",
                file_path.display(),
                file_size_mb,
                streaming_threshold_mb
            );
            true
        } else {
            // Chunk 10: Use streaming for small files too if commands support it
            // This ensures groups and hold space operations work correctly
            true
        };

        // Track which files should use streaming
        if use_streaming {
            streaming_files.push(file_path.clone());
        }

        // Process file with appropriate processor (ALWAYS dry_run for preview)
        let diff = if use_streaming {
            // Use streaming processor with dry_run=true for preview
            let mut stream_processor =
                file_processor::StreamProcessor::with_regex_flavor(commands.clone(), regex_flavor)
                    .with_context_size(context)
                    .with_dry_run(true); // Always preview first
            stream_processor.process_streaming_forced(file_path)
        } else {
            // Use in-memory processor (preview is built-in)
            let mut processor =
                file_processor::FileProcessor::with_regex_flavor(commands.clone(), regex_flavor);
            processor.set_no_default_output(quiet); // Wire up -n flag
            processor.process_file_with_context(file_path)
        };

        match diff {
            Ok(diff) => diffs.push(diff),
            Err(e) => {
                if debug_enabled {
                    tracing::error!(
                        file = %file_path.display(),
                        error = %e,
                        "Failed to process file"
                    );
                }
                eprintln!("Error processing {}: {}", file_path.display(), e);
            }
        }
    }

    // Check if there are any changes or printed lines
    let total_changes: usize = diffs.iter().map(|d| d.changes.len()).sum();
    let has_printed_lines: bool = diffs.iter().any(|d| !d.printed_lines.is_empty());

    if total_changes == 0 && !has_printed_lines {
        if debug_enabled {
            tracing::info!("No changes would be made");
        }
        println!("No changes would be made.");
        return Ok(());
    }

    if debug_enabled {
        tracing::info!(
            total_changes = total_changes,
            files_processed = diffs.len(),
            "Changes detected"
        );
    }

    // Show preview (always show in dry-run or interactive mode)
    if dry_run || interactive {
        let header = diff_formatter::DiffFormatter::format_dry_run_header(expression);
        println!("{}", header);

        for diff in &diffs {
            let output =
                diff_formatter::DiffFormatter::format_diff_with_context(diff, context, expression);
            print!("{}", output);
        }
    }

    // Interactive mode: ask for confirmation
    if interactive && !dry_run {
        print!("Apply changes? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            if debug_enabled {
                tracing::info!("User declined changes in interactive mode");
            }
            println!("Changes not applied.");
            return Ok(());
        }
    }

    // Dry run mode: don't apply
    if dry_run {
        if debug_enabled {
            tracing::info!("Dry run completed, no changes applied");
        }
        return Ok(());
    }

    // Execute mode: apply with backup (unless --no-backup --force)
    let backup_id = if no_backup {
        // Skip backup creation
        if debug_enabled {
            tracing::warn!("Backup skipped (--no-backup flag)");
        }
        println!("⚠️  Skipping backup (changes cannot be undone)");
        None
    } else if !can_modify_files {
        // Skip backup if commands don't modify files (optimization)
        if debug_enabled {
            tracing::info!("No backup created (read-only command)");
        }
        println!("ℹ️  No backup needed (read-only command)");
        None
    } else {
        // Create backup with custom or default directory
        let mut backup_manager = if let Some(dir) = backup_dir {
            backup_manager::BackupManager::with_directory(dir)?
        } else {
            backup_manager::BackupManager::new()?
        };

        // Create backup BEFORE applying changes
        match backup_manager.create_backup(expression, &file_paths) {
            Ok(id) => {
                if debug_enabled {
                    tracing::info!(backup_id = %id, "Backup created");
                }
                println!("✅ Backup created: {}", id);
                Some(id)
            }
            Err(e) => {
                if debug_enabled {
                    tracing::error!(
                        error = %e,
                        "Failed to create backup"
                    );
                }
                return Err(e);
            }
        }
    };

    // Apply changes
    let mut apply_errors = Vec::new();
    for file_path in &file_paths {
        if streaming_files.contains(file_path) {
            // Streaming files: Re-process with dry_run=false to apply changes
            let mut stream_processor =
                file_processor::StreamProcessor::with_regex_flavor(commands.clone(), regex_flavor)
                    .with_context_size(context)
                    .with_dry_run(false); // Apply changes now
            match stream_processor.process_streaming_forced(file_path) {
                Ok(_) => {
                    if debug_enabled {
                        tracing::debug!(
                            file = %file_path.display(),
                            mode = "streaming",
                            "Changes applied successfully"
                        );
                    }
                }
                Err(e) => {
                    if debug_enabled {
                        tracing::error!(
                            file = %file_path.display(),
                            error = %e,
                            "Failed to apply changes"
                        );
                    }
                    eprintln!("Error applying to {}: {}", file_path.display(), e);
                    apply_errors.push((file_path.clone(), e));
                }
            }
        } else {
            // In-memory files: Apply using apply_to_file()
            let mut processor =
                file_processor::FileProcessor::with_regex_flavor(commands.clone(), regex_flavor);
            processor.set_no_default_output(quiet); // Wire up -n flag
            match processor.apply_to_file(file_path) {
                Ok(_) => {
                    if debug_enabled {
                        tracing::debug!(
                            file = %file_path.display(),
                            mode = "in-memory",
                            "Changes applied successfully"
                        );
                    }
                }
                Err(e) => {
                    if debug_enabled {
                        tracing::error!(
                            file = %file_path.display(),
                            error = %e,
                            "Failed to apply changes"
                        );
                    }
                    eprintln!("Error applying to {}: {}", file_path.display(), e);
                    apply_errors.push((file_path.clone(), e));
                }
            }
        }
    }

    // Show result
    if !interactive {
        // Show what was applied
        for diff in &diffs {
            let output =
                diff_formatter::DiffFormatter::format_diff_with_context(diff, context, expression);
            print!("{}", output);
        }
    }

    // Show rollback info only if backup was created
    if let Some(id) = backup_id {
        println!("\nBackup ID: {}", id);
        println!("Rollback with: sedx rollback {}", id);
    } else {
        println!("\nNo backup created - changes cannot be undone");
    }

    // Log completion
    let elapsed = start_time.elapsed();
    if debug_enabled {
        let status = if apply_errors.is_empty() {
            "success"
        } else {
            "partial_failure"
        };
        tracing::info!(
            status = status,
            elapsed_ms = elapsed.as_millis(),
            files_processed = file_paths.len() - apply_errors.len(),
            errors = apply_errors.len(),
            "Operation completed"
        );
    }

    if !apply_errors.is_empty() {
        Err(anyhow::anyhow!(
            "Failed to apply changes to {} file(s)",
            apply_errors.len()
        ))
    } else {
        Ok(())
    }
}

/// Check if any command in the list can modify files
/// Returns true if any command modifies file content (s, d, a, i, c, etc.)
/// Returns false if commands only read/print (p, n, q, Q, =, l, etc.)
fn commands_can_modify_files(commands: &[crate::command::Command]) -> bool {
    use crate::command::Command;

    for cmd in commands {
        match cmd {
            // Commands that DON'T modify files
            Command::Print { .. } | Command::Quit { .. } | Command::QuitWithoutPrint { .. }
            | Command::Next { .. } | Command::NextAppend { .. } | Command::PrintFirstLine { .. }
            // Phase 5: Flow control commands don't modify files
            | Command::Label { .. } | Command::Branch { .. } | Command::Test { .. } | Command::TestFalse { .. }
            // Phase 5: Print commands don't modify files (they write to stdout)
            | Command::PrintLineNumber { .. } | Command::PrintFilename { .. }
            => continue,  // Skip read-only commands, keep checking

            // Commands that MIGHT modify files
            Command::Substitution { .. } | Command::Delete { .. }
            | Command::Insert { .. } | Command::Append { .. } | Command::Change { .. }
            | Command::Hold { .. } | Command::HoldAppend { .. } | Command::Get { .. }
            | Command::GetAppend { .. } | Command::Exchange { .. }
            | Command::Group { .. } | Command::DeleteFirstLine { .. }
            | Command::ReadFile { .. } | Command::WriteFile { .. } | Command::ReadLine { .. } | Command::WriteFirstLine { .. }
            | Command::ClearPatternSpace { .. }
            => return true,  // Found a modifying command
        }
    }

    // If we get here, no modifying commands were found
    false
}

fn rollback(id: Option<String>) -> Result<()> {
    let backup_manager = backup_manager::BackupManager::new()?;

    let backup_id = match id {
        Some(id) => id,
        None => match backup_manager.get_last_backup_id()? {
            Some(id) => {
                println!("Rolling back last operation: {}\n", id);
                id
            }
            None => {
                anyhow::bail!("No backups found to rollback");
            }
        },
    };

    backup_manager.restore_backup(&backup_id)?;
    println!("\n✅ Rollback complete");

    Ok(())
}

fn show_history() -> Result<()> {
    let backup_manager = backup_manager::BackupManager::new()?;
    let backups = backup_manager.list_backups()?;

    let output = diff_formatter::DiffFormatter::format_history(backups);
    println!("{}", output);

    Ok(())
}

fn show_status() -> Result<()> {
    let backup_manager = backup_manager::BackupManager::new()?;
    let backups = backup_manager.list_backups()?;

    println!("Current backup status:\n");
    println!("Total backups: {}\n", backups.len());

    if let Some(last) = backups.last() {
        println!("Last operation:");
        println!("  ID: {}", last.id);
        println!("  Time: {}", last.timestamp.format("%Y-%m-%d %H:%M:%S"));
        println!("  Command: {}", last.expression);
    }

    Ok(())
}

// Backup subcommand handlers

fn backup_list(verbose: bool) -> Result<()> {
    let backup_manager = backup_manager::BackupManager::new()?;
    let backups = backup_manager.list_backups()?;

    if backups.is_empty() {
        println!("No backups found.");
        return Ok(());
    }

    println!("Backups ({} total):\n", backups.len());

    for backup in backups.iter().rev() {
        println!("ID: {}", backup.id);
        println!("  Time: {}", backup.timestamp.format("%Y-%m-%d %H:%M:%S"));
        println!("  Expression: {}", backup.expression);
        println!("  Files: {}", backup.files.len());

        if verbose {
            println!("  Details:");
            for file_backup in &backup.files {
                let size = std::fs::metadata(&file_backup.backup_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                println!(
                    "    - {} ({} bytes)",
                    file_backup.original_path.display(),
                    disk_space::DiskSpaceInfo::bytes_to_human(size)
                );
            }
        }
        println!();
    }

    Ok(())
}

fn backup_show(id: &str) -> Result<()> {
    let backup_manager = backup_manager::BackupManager::new()?;
    let backups = backup_manager.list_backups()?;

    let backup = backups
        .iter()
        .find(|b| b.id.starts_with(id))
        .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", id))?;

    println!("Backup Details:\n");
    println!("ID: {}", backup.id);
    println!("Time: {}", backup.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("Expression: {}", backup.expression);
    println!("Files: {}\n", backup.files.len());

    for file_backup in &backup.files {
        let size = std::fs::metadata(&file_backup.backup_path)
            .map(|m| m.len())
            .unwrap_or(0);
        println!("  {}", file_backup.original_path.display());
        println!("    Backup: {}", file_backup.backup_path.display());
        println!(
            "    Size: {}",
            disk_space::DiskSpaceInfo::bytes_to_human(size)
        );
        println!();
    }

    Ok(())
}

fn backup_restore(id: &str) -> Result<()> {
    let backup_manager = backup_manager::BackupManager::new()?;
    println!("Restoring backup: {}", id);
    println!("This will replace current files with backed up versions.\n");

    backup_manager.restore_backup(id)?;

    Ok(())
}

fn backup_remove(id: &str, force: bool) -> Result<()> {
    let backup_manager = backup_manager::BackupManager::new()?;
    let backups = backup_manager.list_backups()?;

    let backup = backups
        .iter()
        .find(|b| b.id.starts_with(id))
        .ok_or_else(|| anyhow::anyhow!("Backup not found: {}", id))?;

    if !force {
        println!("This will permanently delete backup: {}", backup.id);
        print!("Are you sure? [y/N] ");
        io::stdout().flush()?;

        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;

        if !confirm.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let backup_dir = backup_manager.backups_dir().join(&backup.id);
    fs::remove_dir_all(&backup_dir)
        .with_context(|| format!("Failed to remove backup: {}", backup.id))?;

    println!("✅ Backup removed: {}", backup.id);

    Ok(())
}

fn backup_prune(keep: Option<usize>, keep_days: Option<usize>, force: bool) -> Result<()> {
    let backup_manager = backup_manager::BackupManager::new()?;
    let backups = backup_manager.list_backups()?;

    if backups.is_empty() {
        println!("No backups to prune.");
        return Ok(());
    }

    let keep = keep.unwrap_or(10); // Default: keep 10 most recent

    // Determine which backups to remove
    let mut to_remove = Vec::new();

    if let Some(days) = keep_days {
        // Prune by date
        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(days as i64);

        for backup in &backups {
            if backup.timestamp < cutoff_date {
                to_remove.push(backup.clone());
            }
        }

        println!("Pruning backups older than {} days:", days);
    } else {
        // Prune by count
        let sorted = backups.clone();
        let mut backups_by_date = sorted.into_iter().enumerate().collect::<Vec<_>>();
        backups_by_date.sort_by_key(|(_, b)| b.timestamp);

        // Keep the N most recent
        for (_idx, backup) in backups_by_date.into_iter().rev().skip(keep) {
            to_remove.push(backup);
        }

        println!("Pruning backups, keeping only {} most recent:", keep);
    }

    if to_remove.is_empty() {
        println!("No backups to remove.");
        return Ok(());
    }

    println!("\nBackups to be removed:");
    for backup in &to_remove {
        println!(
            "  - {} ({})",
            backup.id,
            backup.timestamp.format("%Y-%m-%d %H:%M:%S")
        );
    }
    println!("\nTotal: {} backup(s)", to_remove.len());

    if !force {
        print!("Continue? [y/N] ");
        io::stdout().flush()?;

        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;

        if !confirm.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Remove the backups
    for backup in to_remove {
        let backup_dir = backup_manager.backups_dir().join(&backup.id);
        fs::remove_dir_all(&backup_dir)
            .with_context(|| format!("Failed to remove backup: {}", backup.id))?;
        println!("✅ Removed: {}", backup.id);
    }

    Ok(())
}

// Config command handlers

fn config_show() -> Result<()> {
    let config = load_config()?;
    let config_path = config_file_path()?;

    println!("SedX Configuration:");
    println!("  File: {}\n", config_path.display());

    println!("[backup]");
    if let Some(max_size_gb) = config.backup.max_size_gb {
        println!("  max_size_gb = {}", max_size_gb);
    } else {
        println!("  max_size_gb = (not set)");
    }
    if let Some(max_disk) = config.backup.max_disk_usage_percent {
        println!("  max_disk_usage_percent = {}", max_disk);
    } else {
        println!("  max_disk_usage_percent = (not set)");
    }
    if let Some(ref dir) = config.backup.backup_dir {
        println!("  backup_dir = \"{}\"", dir);
    } else {
        println!("  backup_dir = (not set)");
    }

    println!("\n[compatibility]");
    if let Some(ref mode) = config.compatibility.mode {
        println!("  mode = \"{}\"", mode);
    } else {
        println!("  mode = (not set)");
    }
    if let Some(show_warn) = config.compatibility.show_warnings {
        println!("  show_warnings = {}", show_warn);
    } else {
        println!("  show_warnings = (not set)");
    }

    println!("\n[processing]");
    if let Some(ctx) = config.processing.context_lines {
        println!("  context_lines = {}", ctx);
    } else {
        println!("  context_lines = (not set)");
    }
    if let Some(max_mem) = config.processing.max_memory_mb {
        println!("  max_memory_mb = {}", max_mem);
    } else {
        println!("  max_memory_mb = (not set)");
    }
    if let Some(stream) = config.processing.streaming {
        println!("  streaming = {}", stream);
    } else {
        println!("  streaming = (not set)");
    }
    if let Some(debug) = config.processing.debug {
        println!("  debug = {}", debug);
    } else {
        println!("  debug = (not set)");
    }

    Ok(())
}

fn config_edit() -> Result<()> {
    use config::{Config, validate_config};

    let config_path = config_file_path()?;

    // Ensure config file exists with all fields
    let file_existed = config_path.exists();
    if !file_existed {
        println!("Creating new configuration file: {}", config_path.display());
    }

    // Ensure all fields are present (adds missing fields from template)
    ensure_complete_config()?;

    if !file_existed {
        println!("✅ Created default configuration file\n");
    }

    // Get editor from environment
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            // Try common editors in order of preference
            if cfg!(unix) {
                if which::which("vim").is_ok() {
                    "vim".to_string()
                } else if which::which("nano").is_ok() {
                    "nano".to_string()
                } else {
                    "vi".to_string()
                }
            } else {
                "notepad".to_string()
            }
        });

    println!("Opening {} in editor: {}", config_path.display(), editor);
    println!("After saving and exiting, the configuration will be validated.\n");

    // Open editor
    let status = ProcessCommand::new(&editor)
        .arg(&config_path)
        .status()
        .with_context(|| format!("Failed to open editor: {}", editor))?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status: {}", status);
    }

    // Validate the edited config
    let config_str = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

    let config: Config = toml::from_str(&config_str)
        .with_context(|| format!("Failed to parse config file: {}", config_path.display()))?;

    validate_config(&config)?;

    println!("\n✅ Configuration is valid!");

    Ok(())
}

fn config_log_path() -> Result<()> {
    use logger::get_current_log_path;

    let config = load_config()?;
    let debug_enabled = config.processing.debug.unwrap_or(false);

    println!("SedX Log File:");
    println!("  Path: {}", get_current_log_path().display());
    println!(
        "  Status: {}",
        if debug_enabled { "enabled" } else { "disabled" }
    );
    println!();

    if !debug_enabled {
        println!("Debug logging is currently disabled.");
        println!("To enable it, edit ~/.sedx/config.toml and set:");
        println!("\n  [processing]");
        println!("  debug = true\n");
        println!("After enabling, logs will be written to the path above.");
    } else {
        println!("Debug logging is enabled. Operations are being logged.");
    }

    Ok(())
}
