mod backup_manager;
mod bre_converter;
mod cli;
mod command;
mod config;
mod diff_formatter;
mod disk_space;
mod ere_converter;
mod error_helpers;
mod file_processor;
mod logger;
mod parser;
mod path_policy;
mod regex_error;

use anyhow::{Context, Result};
use cli::{Args, RegexFlavor, parse_args};
use command::Command;
use config::{config_file_path, load_config};
use logger::init_debug_logging;
use parser::Parser;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

fn main() -> Result<()> {
    let args = parse_args()?;

    // Initialize debug logging early (before any operations)
    let log_path = if let Args::Execute { .. } = args {
        let config = load_config();
        match config {
            Ok(cfg) => {
                let debug_enabled = cfg.processing.debug.unwrap_or(false);
                init_debug_logging(debug_enabled)?
            }
            Err(_) => None,
        }
    } else {
        None
    };

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
            no_streaming,
            regex_flavor,
            no_backup,
            backup_dir,
            quiet,
            json,
        } => {
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
                    no_streaming,
                    regex_flavor,
                    no_backup,
                    backup_dir,
                    quiet,
                    json,
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

fn execute_stdin(expression: &str, regex_flavor: RegexFlavor, quiet: bool) -> Result<()> {
    if let Ok(config) = load_config() {
        emit_compatibility_warnings(expression, regex_flavor, &config);
    }

    let parser = Parser::new(regex_flavor);
    let commands = parser
        .parse(expression)
        .context("Failed to parse expression")?;

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let lines: Vec<String> = input.lines().map(|s| s.to_string()).collect();

    let mut processor = file_processor::FileProcessor::with_regex_flavor(commands, regex_flavor);
    processor.set_no_default_output(quiet);

    let result_lines = processor.apply_cycle_based(lines)?;
    for line in result_lines {
        println!("{}", line);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_command(
    expression: &str,
    files: &[String],
    dry_run: bool,
    interactive: bool,
    context: usize,
    streaming: bool,
    no_streaming: bool,
    regex_flavor: RegexFlavor,
    no_backup: bool,
    backup_dir: Option<String>,
    quiet: bool,
    json: bool,
) -> Result<()> {
    let config = load_config()?;
    let backup_dir = backup_dir.or_else(|| config.backup.backup_dir.clone());
    let parser = Parser::new(regex_flavor);
    emit_compatibility_warnings(expression, regex_flavor, &config);
    let commands = parser
        .parse(expression)
        .context("Failed to parse expression")?;

    let can_modify_files = commands_can_modify_files(&commands);
    let supports_streaming = can_use_streaming(&commands);
    if streaming {
        if let Some(command) = unsupported_streaming_command(&commands) {
            anyhow::bail!(
                "command '{}' is not supported in forced streaming mode",
                command
            );
        }
    }

    let mut file_paths = Vec::new();
    for f in files {
        let path = PathBuf::from(f);
        if can_modify_files && !dry_run {
            path_policy::ensure_not_symlink(&path)?;
        }
        if let Ok(p) = fs::canonicalize(&path) {
            file_paths.push(p);
        } else {
            file_paths.push(path);
        }
    }

    let mut diffs = Vec::new();
    let mut streaming_files = Vec::new();
    let mut processing_errors = 0;

    let max_mem_bytes = config.processing.max_memory_mb.unwrap_or(100) as u64 * 1024 * 1024;
    let config_streaming = config.processing.streaming.unwrap_or(false);

    for file_path in &file_paths {
        let metadata = match fs::metadata(file_path) {
            Ok(metadata) => metadata,
            Err(err) => {
                eprintln!(
                    "{}",
                    error_helpers::file_error(file_path, "resolving", &err)
                );
                continue;
            }
        };
        let use_streaming = supports_streaming
            && !no_streaming
            && (streaming || config_streaming || metadata.len() >= max_mem_bytes);

        let diff = if use_streaming {
            streaming_files.push(file_path.clone());
            let mut sp =
                file_processor::StreamProcessor::with_regex_flavor(commands.clone(), regex_flavor)
                    .with_context_size(context)
                    .with_no_default_output(quiet)
                    .with_dry_run(true);
            sp.process_streaming_forced(file_path)
        } else {
            let mut fp =
                file_processor::FileProcessor::with_regex_flavor(commands.clone(), regex_flavor);
            fp.set_no_default_output(quiet);
            fp.process_file_with_context(file_path)
        };

        match diff {
            Ok(diff) => diffs.push(diff),
            Err(e) => {
                processing_errors += 1;
                eprintln!("{}", format_processing_error(file_path, &e));
            }
        }
    }

    if processing_errors > 0 {
        anyhow::bail!("failed to process {} file(s)", processing_errors);
    }

    let has_changes = diffs.iter().any(|d| !d.changes.is_empty());

    if dry_run || !has_changes || interactive {
        if json {
            print!(
                "{}",
                diff_formatter::DiffFormatter::format_json(&diffs, expression)?
            );
            if !diffs.is_empty() {
                println!();
            }
        } else {
            if dry_run || interactive {
                println!(
                    "{}",
                    diff_formatter::DiffFormatter::format_dry_run_header(expression)
                );
            }
            for diff in &diffs {
                print!(
                    "{}",
                    diff_formatter::DiffFormatter::format_diff_with_context(
                        diff, context, expression
                    )
                );
            }
        }
    }

    if dry_run || !has_changes {
        if !has_changes && !dry_run && !json {
            println!("\nNo changes to apply.");
        }
        return Ok(());
    }

    if interactive {
        print!("\nApply changes to {} file(s)? [y/N] ", file_paths.len());
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    let backup_id = if no_backup || !can_modify_files {
        None
    } else {
        let mut bm = if let Some(dir) = backup_dir {
            backup_manager::BackupManager::with_directory(dir)?
        } else {
            backup_manager::BackupManager::new()?
        };
        Some(bm.create_backup(expression, &file_paths)?)
    };

    if can_modify_files {
        for file_path in &file_paths {
            if streaming_files.contains(file_path) {
                let mut sp = file_processor::StreamProcessor::with_regex_flavor(
                    commands.clone(),
                    regex_flavor,
                )
                .with_context_size(context)
                .with_no_default_output(quiet)
                .with_dry_run(false);
                sp.process_streaming_forced(file_path)?;
            } else {
                let mut fp = file_processor::FileProcessor::with_regex_flavor(
                    commands.clone(),
                    regex_flavor,
                );
                fp.set_no_default_output(quiet);
                fp.apply_to_file(file_path)?;
            }
        }
    }

    if !interactive {
        if json {
            print!(
                "{}",
                diff_formatter::DiffFormatter::format_json(&diffs, expression)?
            );
            println!();
        } else {
            for diff in &diffs {
                print!(
                    "{}",
                    diff_formatter::DiffFormatter::format_diff_with_context(
                        diff, context, expression
                    )
                );
            }
        }
    }

    if let Some(id) = backup_id {
        if !json {
            println!("\nBackup ID: {}", id);
            println!("Rollback with: sedx rollback {}", id);
        }
    }

    Ok(())
}

fn format_processing_error(file_path: &std::path::Path, error: &anyhow::Error) -> String {
    for cause in error.chain() {
        if let Some(io_error) = cause.downcast_ref::<io::Error>() {
            return error_helpers::file_error(file_path, "processing", io_error);
        }
    }

    format!("Error processing {}: {}", file_path.display(), error)
}

fn emit_compatibility_warnings(
    expression: &str,
    regex_flavor: RegexFlavor,
    config: &config::Config,
) {
    if config.compatibility.show_warnings != Some(true) || regex_flavor != RegexFlavor::PCRE {
        return;
    }

    for warning in pcre_compatibility_warnings(expression) {
        eprintln!("Warning: {}", warning);
    }
}

fn pcre_compatibility_warnings(expression: &str) -> Vec<&'static str> {
    let mut warnings = Vec::new();
    if expression.contains(r"\d")
        || expression.contains(r"\w")
        || expression.contains(r"\s")
        || expression.contains("(?")
    {
        warnings.push(
            "PCRE-specific regex syntax may not behave the same as GNU sed BRE/ERE; use -E or -B for GNU-compatible parsing.",
        );
    }
    if expression.contains("$1")
        || expression.contains("$2")
        || expression.contains("$3")
        || expression.contains("${")
    {
        warnings.push(
            "PCRE-style replacement backreferences differ from GNU sed; GNU sed uses \\1-style replacement references.",
        );
    }
    warnings
}

fn can_use_streaming(commands: &[Command]) -> bool {
    for cmd in commands {
        match cmd {
            Command::Substitution { .. }
            | Command::Delete { .. }
            | Command::Print { .. }
            | Command::Quit { .. }
            | Command::Hold { .. }
            | Command::HoldAppend { .. }
            | Command::Get { .. }
            | Command::GetAppend { .. }
            | Command::Exchange { .. }
            | Command::Next { .. }
            | Command::NextAppend { .. }
            | Command::PrintFirstLine { .. }
            | Command::PrintLineNumber { .. }
            | Command::PrintFilename { .. }
            | Command::ClearPatternSpace { .. } => {}
            Command::Insert { .. } | Command::Append { .. } => {}
            Command::Change { .. } => {}
            Command::Group {
                commands: inner, ..
            } => {
                if !can_use_streaming(inner) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn unsupported_streaming_command(commands: &[Command]) -> Option<&'static str> {
    for cmd in commands {
        match cmd {
            Command::Substitution { .. }
            | Command::Delete { .. }
            | Command::Print { .. }
            | Command::Quit { .. }
            | Command::Hold { .. }
            | Command::HoldAppend { .. }
            | Command::Get { .. }
            | Command::GetAppend { .. }
            | Command::Exchange { .. }
            | Command::Next { .. }
            | Command::NextAppend { .. }
            | Command::PrintFirstLine { .. }
            | Command::PrintLineNumber { .. }
            | Command::PrintFilename { .. }
            | Command::ClearPatternSpace { .. }
            | Command::Insert { .. }
            | Command::Append { .. }
            | Command::Change { .. } => {}
            Command::Group {
                commands: inner, ..
            } => {
                if let Some(command) = unsupported_streaming_command(inner) {
                    return Some(command);
                }
            }
            Command::ReadFile { .. } => return Some("r"),
            Command::ReadLine { .. } => return Some("R"),
            Command::WriteFile { .. } => return Some("w"),
            Command::WriteFirstLine { .. } => return Some("W"),
            Command::QuitWithoutPrint { .. } => return Some("Q"),
            Command::DeleteFirstLine { .. } => return Some("D"),
            Command::Label { .. } => return Some(":"),
            Command::Branch { .. } => return Some("b"),
            Command::Test { .. } => return Some("t"),
            Command::TestFalse { .. } => return Some("T"),
        }
    }
    None
}

fn commands_can_modify_files(commands: &[Command]) -> bool {
    for cmd in commands {
        match cmd {
            Command::Print { .. }
            | Command::Quit { .. }
            | Command::QuitWithoutPrint { .. }
            | Command::Next { .. }
            | Command::NextAppend { .. }
            | Command::PrintFirstLine { .. }
            | Command::Label { .. }
            | Command::Branch { .. }
            | Command::Test { .. }
            | Command::TestFalse { .. }
            | Command::PrintLineNumber { .. }
            | Command::PrintFilename { .. } => continue,
            Command::Group {
                commands: inner, ..
            } => {
                if commands_can_modify_files(inner) {
                    return true;
                }
            }
            _ => return true,
        }
    }
    false
}

fn rollback(id: Option<String>) -> Result<()> {
    let bm = backup_manager::BackupManager::new()?;
    let id = match id {
        Some(id) => id,
        None => bm.get_last_backup_id()?.context("No backups found")?,
    };
    bm.restore_backup(&id)?;
    println!("\n✅ Rollback complete");
    Ok(())
}

fn show_history() -> Result<()> {
    let bm = backup_manager::BackupManager::new()?;
    let backups = bm.list_backups()?;
    println!("{}", diff_formatter::DiffFormatter::format_history(backups));
    Ok(())
}

fn show_status() -> Result<()> {
    let bm = backup_manager::BackupManager::new()?;
    let backups = bm.list_backups()?;
    println!("Total backups: {}", backups.len());
    if let Some(last) = backups.last() {
        println!("Last backup: {} ({})", last.id, last.timestamp);
    }
    Ok(())
}

fn backup_list(verbose: bool) -> Result<()> {
    let bm = backup_manager::BackupManager::new()?;
    let backups = bm.list_backups()?;
    for b in backups {
        if verbose {
            println!("{:?}", b);
        } else {
            println!("{}", b.id);
        }
    }
    Ok(())
}

fn backup_show(id: &str) -> Result<()> {
    let bm = backup_manager::BackupManager::new()?;
    let backups = bm.list_backups()?;
    let b = backups
        .iter()
        .find(|b| b.id.starts_with(id))
        .context("Backup not found")?;
    println!("{:?}", b);
    Ok(())
}

fn backup_restore(id: &str) -> Result<()> {
    let bm = backup_manager::BackupManager::new()?;
    bm.restore_backup(id)?;
    println!("Restore successful.");
    Ok(())
}

fn backup_remove(id: &str, force: bool) -> Result<()> {
    let bm = backup_manager::BackupManager::new()?;
    let backups = bm.list_backups()?;
    let b = backups
        .iter()
        .find(|b| b.id.starts_with(id))
        .context("Backup not found")?;
    if !force {
        print!("Remove backup {}? [y/N] ", b.id);
        io::stdout().flush()?;
        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;
        if !confirm.trim().eq_ignore_ascii_case("y") {
            return Ok(());
        }
    }
    bm.remove_backup_by_id(&b.id)?;
    println!("Backup removed.");
    Ok(())
}

fn backup_prune(keep: Option<usize>, keep_days: Option<usize>, _force: bool) -> Result<()> {
    let bm = backup_manager::BackupManager::new()?;
    let count = if let Some(days) = keep_days {
        bm.prune_backups_older_than(days as i64)?
    } else {
        bm.prune_backups(keep.unwrap_or(10))?
    };
    println!("Pruned {} backups.", count);
    Ok(())
}

fn config_log_path() -> Result<()> {
    println!("{}", logger::get_current_log_path().display());
    Ok(())
}

fn config_show() -> Result<()> {
    let path = config_file_path()?;
    let content = fs::read_to_string(path)?;
    println!("{}", content);
    Ok(())
}

fn config_edit() -> Result<()> {
    config::ensure_complete_config()?;
    let path = config_file_path()?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());

    println!("Opening config file with {}...", editor);

    let status = ProcessCommand::new(&editor)
        .arg(&path)
        .status()
        .context(format!("Failed to open editor: {}", editor))?;

    if status.success() {
        // Validate after editing
        let config = load_config()?;
        config::validate_config(&config)?;
        println!("✅ Configuration validated successfully.");
    } else {
        anyhow::bail!("Editor exited with failure status");
    }

    Ok(())
}
