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
  • PCRE (modern regex) by default
  • Optional BRE/ERE mode for GNU sed compatibility
  • ~90% GNU sed compatibility

REGEX MODES:
  PCRE (default) - Modern Perl-compatible regex
  -E, --ere      - Extended Regular Expressions (like sed -E)
  -B, --bre      - Basic Regular Expressions (like GNU sed)

STDIN/STDOUT:
  When no files are specified, sedx reads from stdin and writes to stdout.
  This makes it compatible with pipelines like: cat file.txt | sedx 's/foo/bar/'

  Backups, diffs, and rollback are disabled in stdin mode.

EXAMPLES:
  sedx 's/foo/bar/g' file.txt              Replace all occurrences
  cat file.txt | sedx 's/foo/bar/g'        Read from stdin, write to stdout
  echo 'test' | sedx 's/test/TEST/'        Pipe input
  sedx 's/(foo|baz)/bar/g' file.txt        PCRE: alternation (default)
  sedx -E 's/(foo|baz)/bar/g' file.txt     ERE: alternation
  sedx -B 's/\\(foo\\|baz\\)/bar/g' file.txt BRE: escaped metacharacters
  sedx '/error/s/test/fix/' file.txt       Only in lines matching 'error'
  sedx '5,10d' file.txt                    Delete lines 5-10
  sedx '{s/a/A/g; s/b/B/g}' file.txt      Multiple commands
  sedx --rollback backup.ID                Undo last change")]
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

    /// Enable streaming mode for large files (>=100MB)
    #[arg(long, alias = "force-streaming")]
    #[arg(help = "Enable streaming mode for large files (auto-detects at 100MB)\nUse --no-streaming to disable")]
    streaming: bool,

    /// Disable streaming mode
    #[arg(long = "no-streaming")]
    #[arg(help = "Disable auto-detection and force in-memory processing")]
    no_streaming: bool,

    /// Use Basic Regular Expressions (BRE) - GNU sed compatible
    #[arg(short = 'B', long, conflicts_with = "ere")]
    #[arg(help = "Use Basic Regular Expressions (BRE)\nLike GNU sed: \\( \\), \\{ \\}, \\+, \\?, \\|")]
    bre: bool,

    /// Use Extended Regular Expressions (ERE)
    #[arg(short = 'E', long, conflicts_with = "bre")]
    #[arg(help = "Use Extended Regular Expressions (ERE)\nLike sed -E: ( ), { }, +, ?, |")]
    ere: bool,

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

            // Note: Empty files vector means read from stdin (like sed)

            // Determine context size
            let context = if cli.no_context {
                0
            } else if let Some(n) = cli.context {
                n
            } else {
                2 // Default
            };

            // Determine streaming mode (auto-detect at 100MB)
            let streaming = if cli.no_streaming {
                false  // Explicitly disabled
            } else if cli.streaming {
                true   // Explicitly enabled
            } else {
                false  // Auto-detect (will check file size in main.rs)
            };

            // Determine regex flavor
            let regex_flavor = if cli.bre {
                RegexFlavor::BRE
            } else if cli.ere {
                RegexFlavor::ERE
            } else {
                RegexFlavor::PCRE  // Default
            };

            Ok(Args::Execute {
                expression,
                files: cli.files,
                dry_run: cli.dry_run,
                interactive: cli.interactive,
                context,
                streaming,
                regex_flavor,
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegexFlavor {
    /// Basic Regular Expressions (GNU sed compatible)
    BRE,
    /// Extended Regular Expressions (sed -E compatible)
    ERE,
    /// Perl-Compatible Regular Expressions (modern, default)
    PCRE,
}

#[derive(Debug)]
pub enum Args {
    Execute {
        expression: String,
        files: Vec<String>,
        dry_run: bool,
        interactive: bool,
        context: usize,
        streaming: bool,
        regex_flavor: RegexFlavor,
    },
    Rollback {
        id: Option<String>,
    },
    History,
    Status,
}
