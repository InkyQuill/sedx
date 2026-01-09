mod backup_manager;
mod bre_converter;
mod capability;
mod cli;
mod command;
mod diff_formatter;
mod ere_converter;
mod file_processor;
mod parser;
mod sed_parser;

use anyhow::Result;
use cli::{parse_args, Args, RegexFlavor};
use command::{Command, Address};
use parser::Parser;
use std::fs;
use std::io::{self, Write, Read};
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let args = parse_args()?;

    match args {
        Args::Execute {
            expression,
            files,
            dry_run,
            interactive,
            context,
            streaming,
            regex_flavor,
        } => {
            // Check if we're in stdin mode (no files specified)
            if files.is_empty() {
                execute_stdin(&expression, regex_flavor)?;
            } else {
                execute_command(&expression, &files, dry_run, interactive, context, streaming, regex_flavor)?;
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
    }

    Ok(())
}

/// Process stdin and write to stdout (pipeline mode, like sed)
fn execute_stdin(expression: &str, regex_flavor: RegexFlavor) -> Result<()> {
    // Parse sed expression
    let parser = Parser::new(regex_flavor);
    let commands = parser.parse(expression)?;

    // Read all input from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    // Process the input using in-memory processing
    let lines: Vec<String> = input.lines().map(|s| s.to_string()).collect();
    let mut result_lines = lines.clone();

    // Apply all commands to the lines
    let mut processor = file_processor::FileProcessor::new(commands.clone());

    for cmd in &commands {
        let should_continue = processor.apply_command(&mut result_lines, cmd)?;
        if !should_continue {
            break; // Quit command encountered
        }
    }

    // Write output to stdout
    for line in result_lines {
        println!("{}", line);
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
                if let Some(range) = get_command_range_option(cmd) {
                    if !is_range_supported_in_streaming(&range) {
                        return false;
                    }
                }
            }
            _ => {
                // s, d, p, a, i, c, q are supported
                // But need to check address types
                if let Some(range) = get_command_range_option(cmd) {
                    if !is_range_supported_in_streaming(&range) {
                        return false;
                    }
                }
            }
        }
    }

    true
}

/// Extract range from a command (if any)
fn get_command_range_option(cmd: &Command) -> Option<(Address, Address)> {
    match cmd {
        Command::Substitution { range, .. } => {
            if let Some(r) = range {
                Some((r.0.clone(), r.1.clone()))
            } else {
                None
            }
        }
        Command::Delete { range } => Some(range.clone()),
        Command::Print { range } => Some(range.clone()),
        Command::Insert { address, .. } => {
            // Single address - check if it's line number
            match address {
                Address::LineNumber(_) => Some((Address::LineNumber(0), Address::LineNumber(0))),
                _ => None,  // Complex addresses delegate to in-memory
            }
        }
        Command::Append { address, .. } => {
            match address {
                Address::LineNumber(_) => Some((Address::LineNumber(0), Address::LineNumber(0))),
                _ => None,
            }
        }
        Command::Change { address, .. } => {
            match address {
                Address::LineNumber(_) => Some((Address::LineNumber(0), Address::LineNumber(0))),
                _ => None,
            }
        }
        Command::Quit { address } => {
            match address {
                Some(Address::LineNumber(_)) | Some(Address::LastLine) => {
                    Some((Address::LineNumber(0), Address::LineNumber(0)))
                }
                None => Some((Address::LineNumber(0), Address::LineNumber(0))),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Check if a range is supported in streaming mode
fn is_range_supported_in_streaming(range: &(Address, Address)) -> bool {
    use Address::*;

    match (&range.0, &range.1) {
        // Chunk 8: Supported ranges
        (Pattern(_), Pattern(_)) => true,  // /start/,/end/
        (LineNumber(1), LastLine) => true,  // 1,$
        (LineNumber(_), LineNumber(_)) => true,  // 5,10
        (Pattern(_), LineNumber(_)) => true,  // /start/,10 (Chunk 8)
        (LineNumber(_), Pattern(_)) => true,  // 5,/end/ (Chunk 8)
        (Pattern(_), Relative { base: _, offset: _ }) => true,  // /start/,+5 (Chunk 8)

        // Stepping addresses (Chunk 8)
        (Step { .. }, _) | (_, Step { .. }) => true,  // 1~2

        // Not supported (delegate to in-memory):
        (Negated(_), _) | (_, Negated(_)) => false,  // /pattern/!s/foo/bar/
        _ => false,
    }
}

fn execute_command(
    expression: &str,
    files: &[String],
    dry_run: bool,
    interactive: bool,
    context: usize,
    streaming: bool,
    regex_flavor: RegexFlavor,
) -> Result<()> {
    // Parse sed expression using unified parser
    let parser = Parser::new(regex_flavor);
    let commands = parser.parse(expression)?;

    // Check if commands support streaming mode
    let supports_streaming = can_use_streaming(&commands);

    let file_paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();

    // Process all files and generate diffs (PREVIEW PHASE - always dry_run)
    // For each file, decide whether to use streaming or in-memory processing
    let mut diffs = Vec::new();
    let mut streaming_files: Vec<PathBuf> = Vec::new();  // Track which files should use streaming

    for file_path in &file_paths {
        // Get file metadata to check size
        let metadata = match fs::metadata(file_path) {
            Ok(meta) => meta,
            Err(e) => {
                eprintln!("Error reading file {}: {}", file_path.display(), e);
                continue;
            }
        };

        let file_size_mb = metadata.len() / 1024 / 1024;

        // Decide: use streaming if (streaming flag OR file >= 100MB OR commands support it)
        let use_streaming = if !supports_streaming {
            false  // Commands don't support streaming
        } else if streaming {
            true  // Explicitly enabled
        } else if metadata.len() >= 100 * 1024 * 1024 {
            // Auto-detect: file >= 100MB
            eprintln!("📊 Streaming mode activated for {} ({} MB)", file_path.display(), file_size_mb);
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
            let mut stream_processor = file_processor::StreamProcessor::new(commands.clone())
                .with_context_size(context)
                .with_dry_run(true);  // Always preview first
            stream_processor.process_streaming_forced(file_path)
        } else {
            // Use in-memory processor (preview is built-in)
            let mut processor = file_processor::FileProcessor::new(commands.clone());
            processor.process_file_with_context(file_path)
        };

        match diff {
            Ok(diff) => diffs.push(diff),
            Err(e) => {
                eprintln!("Error processing {}: {}", file_path.display(), e);
            }
        }
    }

    // Check if there are any changes or printed lines
    let total_changes: usize = diffs.iter().map(|d| d.changes.len()).sum();
    let has_printed_lines: bool = diffs.iter().any(|d| !d.printed_lines.is_empty());

    if total_changes == 0 && !has_printed_lines {
        println!("No changes would be made.");
        return Ok(());
    }

    // Show preview (always show in dry-run or interactive mode)
    if dry_run || interactive {
        let header = diff_formatter::DiffFormatter::format_dry_run_header(expression);
        println!("{}", header);

        for diff in &diffs {
            let output = diff_formatter::DiffFormatter::format_diff_with_context(diff, context, expression);
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
            println!("Changes not applied.");
            return Ok(());
        }
    }

    // Dry run mode: don't apply
    if dry_run {
        return Ok(());
    }

    // Execute mode: apply with backup
    let mut backup_manager = backup_manager::BackupManager::new()?;

    // Create backup BEFORE applying changes
    let backup_id = backup_manager.create_backup(expression, &file_paths)?;

    // Apply changes
    for file_path in &file_paths {
        if streaming_files.contains(file_path) {
            // Streaming files: Re-process with dry_run=false to apply changes
            let mut stream_processor = file_processor::StreamProcessor::new(commands.clone())
                .with_context_size(context)
                .with_dry_run(false);  // Apply changes now
            match stream_processor.process_streaming_forced(file_path) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("Error applying to {}: {}", file_path.display(), e);
                }
            }
        } else {
            // In-memory files: Apply using apply_to_file()
            let mut processor = file_processor::FileProcessor::new(commands.clone());
            match processor.apply_to_file(file_path) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("Error applying to {}: {}", file_path.display(), e);
                }
            }
        }
    }

    // Show result
    if !interactive {
        // Show what was applied
        for diff in &diffs {
            let output = diff_formatter::DiffFormatter::format_diff_with_context(diff, context, expression);
            print!("{}", output);
        }
    }

    println!("\nBackup ID: {}", backup_id);
    println!("Rollback with: sedx rollback {}", backup_id);

    Ok(())
}

fn rollback(id: Option<String>) -> Result<()> {
    let backup_manager = backup_manager::BackupManager::new()?;

    let backup_id = match id {
        Some(id) => id,
        None => {
            match backup_manager.get_last_backup_id()? {
                Some(id) => {
                    println!("Rolling back last operation: {}\n", id);
                    id
                }
                None => {
                    anyhow::bail!("No backups found to rollback");
                }
            }
        }
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
