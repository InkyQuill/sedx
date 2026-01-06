use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "

Copyright (c) 2025 InkyQuill
License: MIT
Source: https://github.com/InkyQuill/sedx
Rust Edition: 2024"
);

#[derive(Parser)]
#[command(name = "sedx")]
#[command(about = "Safe sed with preview, context, and automatic rollback")]
#[command(long_about = "SedX is a modern replacement for GNU sed written in Rust.

It provides safe file editing with automatic backups, dry-run mode, and easy rollback.
Unlike sed, sedx shows you exactly what will change before applying modifications.

FEATURES:
  • Automatic backups before every modification
  • Dry-run mode to preview changes
  • Easy rollback with one command
  • Colored diff output
  • Extended regex by default
  • ~90% GNU sed compatibility

EXAMPLES:
  sedx 's/foo/bar/g' file.txt              Replace all occurrences
  sedx '/error/s/test/fix/' file.txt        Only in lines matching 'error'
  sedx '5,10d' file.txt                     Delete lines 5-10
  sedx '{s/a/A/g; s/b/B/g}' file.txt       Multiple commands
  sedx --rollback file.txt.backup.TIMESTAMP Undo last change")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(long_version = LONG_VERSION)]
#[command(propagate_version = true)]
struct Cli {
    /// Sed expression to execute (e.g., 's/old/new/g', '10d', '1,5p')
    #[arg(value_name = "EXPRESSION")]
    expression: Option<String>,

    /// Files to process
    #[arg(value_name = "FILE")]
    files: Vec<String>,

    /// Dry run mode (preview changes without applying)
    #[arg(short = 'd', long, alias = "dry-run")]
    #[arg(help = "Preview changes without modifying files\nThis is the default behavior. Use --execute to apply changes.")]
    dry_run: bool,

    /// Interactive mode (ask before applying changes)
    #[arg(short = 'i', long)]
    #[arg(help = "Ask for confirmation before applying each change.")]
    interactive: bool,

    /// Number of context lines to show (default: 2)
    #[arg(short = 'n', long, value_name = "NUM")]
    #[arg(help = "Number of context lines to show around changes\nUse 0 to show only changed lines (equivalent to --no-context)")]
    context: Option<usize>,

    /// No context (show only changed lines)
    #[arg(long = "no-context", alias = "nc")]
    #[arg(help = "Show only changed lines without context\nEquivalent to --context=0")]
    no_context: bool,

    /// Subcommands
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Rollback a previous operation
    #[command(long_about = "Restore files from a backup.

If no backup ID is specified, rolls back the most recent operation.
Use 'sedx history' to see all available backups.

EXAMPLES:
  sedx rollback                    Rollback last operation
  sedx rollback backup.12345       Rollback specific backup
  sedx rollback ~/.sedx/backups/*  Rollback from specific path")]
    Rollback {
        /// Backup ID to rollback (optional, defaults to last operation)
        #[arg(value_name = "ID")]
        id: Option<String>,
    },

    /// Show operation history
    #[command(long_about = "Display a log of all sedx operations.

Shows timestamp, expression, files affected, and backup location for each operation.
The most recent operations appear first.

EXAMPLES:
  sedx history                    Show all operations
  sedx history | head -10         Show last 10 operations")]
    History,

    /// Show current backup status
    #[command(long_about = "Display backup directory location and disk usage.

Shows information about where backups are stored and how much disk space they use.
This helps with backup management and cleanup.

EXAMPLES:
  sedx status                     Show backup status")]
    Status,
}

pub fn parse_args() -> Result<Args> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Rollback { id }) => Ok(Args::Rollback { id }),
        Some(Commands::History) => Ok(Args::History),
        Some(Commands::Status) => Ok(Args::Status),
        None => {
            let expression = cli
                .expression
                .context("Missing sed expression. Usage: sedx 's/old/new/g' file.txt")?;

            if cli.files.is_empty() {
                anyhow::bail!("No files specified. Usage: sedx 's/old/new/g' file.txt");
            }

            // Determine context size
            let context = if cli.no_context {
                0
            } else if let Some(n) = cli.context {
                n
            } else {
                2 // Default
            };

            Ok(Args::Execute {
                expression,
                files: cli.files,
                dry_run: cli.dry_run,
                interactive: cli.interactive,
                context,
            })
        }
    }
}

#[derive(Debug)]
pub enum Args {
    Execute {
        expression: String,
        files: Vec<String>,
        dry_run: bool,
        interactive: bool,
        context: usize,
    },
    Rollback {
        id: Option<String>,
    },
    History,
    Status,
}
