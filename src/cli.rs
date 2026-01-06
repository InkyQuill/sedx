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
#[command(about = "Safe sed with preview, context, and automatic rollback", long_about = None)]
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
    dry_run: bool,

    /// Interactive mode (ask before applying changes)
    #[arg(short = 'i', long)]
    interactive: bool,

    /// Number of context lines to show (default: 2)
    #[arg(short = 'n', long, value_name = "NUM")]
    context: Option<usize>,

    /// No context (show only changed lines)
    #[arg(long = "no-context", alias = "nc")]
    no_context: bool,

    /// Subcommands
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Rollback a previous operation
    Rollback {
        /// Backup ID to rollback (optional, defaults to last operation)
        #[arg(value_name = "ID")]
        id: Option<String>,
    },
    /// Show operation history
    History,
    /// Show current backup status
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
