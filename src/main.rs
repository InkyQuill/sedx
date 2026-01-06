mod backup_manager;
mod cli;
mod diff_formatter;
mod file_processor;
mod sed_parser;

use anyhow::Result;
use cli::{parse_args, Args};
use std::io::{self, Write};
use std::path::PathBuf;

fn main() -> Result<()> {
    let args = parse_args()?;

    match args {
        Args::Execute {
            expression,
            files,
            dry_run,
            interactive,
            context,
        } => {
            execute_command(&expression, &files, dry_run, interactive, context)?;
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

fn execute_command(
    expression: &str,
    files: &[String],
    dry_run: bool,
    interactive: bool,
    context: usize,
) -> Result<()> {
    // Parse sed expression
    let commands = sed_parser::parse_sed_expression(expression)?;

    // Create file processor
    let mut processor = file_processor::FileProcessor::new(commands);

    let file_paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();

    // Process all files and generate diffs
    let mut diffs = Vec::new();
    for file_path in &file_paths {
        match processor.process_file_with_context(file_path) {
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

    // Create backup
    let backup_id = backup_manager.create_backup(expression, &file_paths)?;

    // Apply changes
    for file_path in &file_paths {
        match processor.apply_to_file(file_path) {
            Ok(_) => {},
            Err(e) => {
                eprintln!("Error applying to {}: {}", file_path.display(), e);
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
