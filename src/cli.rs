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

    /// Skip backup creation (requires --force)
    #[arg(long = "no-backup", requires = "force")]
    #[arg(help = "Skip creating a backup (requires --force)\n⚠️  USE WITH CAUTION: Changes cannot be undone!\nRecommended only for files under version control")]
    no_backup: bool,

    /// Force dangerous operations (use with --no-backup)
    #[arg(long = "force", requires = "no_backup")]
    #[arg(help = "Force dangerous operations (required for --no-backup)\nConfirms you understand the risks")]
    force: bool,

    /// Custom backup directory
    #[arg(long, value_name = "DIR")]
    #[arg(help = "Use custom directory for backups\nDefault: ~/.sedx/backups/\nUseful when backup partition is full")]
    backup_dir: Option<String>,

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

    /// Manage backups
    #[command(long_about = "Manage SedX backups.

Provides subcommands for listing, restoring, removing, and pruning backups.

EXAMPLES:
  sedx backup list                 List all backups
  sedx backup show <id>            Show backup details
  sedx backup restore <id>         Restore from backup
  sedx backup remove <id>          Remove a backup
  sedx backup prune --keep=5       Keep only 5 most recent backups
  sedx backup prune --keep-days=7  Keep only backups from last 7 days")]
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },

    /// Edit configuration file
    #[command(long_about = "Open configuration file in text editor.

Opens the SedX configuration file (~/.sedx/config.toml) in your default editor.
If the file doesn't exist, a default one will be created.

After saving and exiting, the configuration will be validated.
If there are any errors, they will be displayed and the file will not be updated.

CONFIGURATION OPTIONS:
  [backup]
    max_size_gb = 2              # Max backup size before warning (GB)
    max_disk_usage_percent = 60   # Max % of free space to use
    backup_dir = \"/path\"         # Custom backup directory (optional)

  [compatibility]
    mode = \"pcre\"                # Regex mode: pcre, ere, or bre
    show_warnings = true          # Show incompatibility warnings

  [processing]
    context_lines = 2             # Context lines to show (max 10)
    max_memory_mb = 100           # Max memory for streaming (MB)
    streaming = true              # Enable streaming for large files

EXAMPLES:
  sedx config                     Edit configuration
  sedx config --show              Show current configuration")]
    Config {
        /// Show current configuration without editing
        #[arg(long = "show")]
        show: bool,
    },
}

#[derive(Subcommand)]
enum BackupAction {
    /// List all backups
    #[command(long_about = "List all backups with details.

Shows backup ID, timestamp, expression, and files for each backup.
Most recent backups appear first.

OPTIONS:
  -v, --verbose    Show more details (file paths, sizes)

EXAMPLES:
  sedx backup list               List all backups
  sedx backup list -v            List with verbose output")]
    List {
        /// Show more details (file paths, sizes)
        #[arg(short, long)]
        verbose: bool,
    },

    /// Show backup details
    #[command(long_about = "Show detailed information about a specific backup.

Displays the full metadata for a backup including expression, timestamp,
and all files that were backed up.

EXAMPLES:
  sedx backup show 20250110-120000-abc123    Show specific backup")]
    Show {
        /// Backup ID
        #[arg(value_name = "ID")]
        id: String,
    },

    /// Restore from a backup
    #[command(long_about = "Restore files from a backup.

Restores all files to their state at the time of the backup.
The backup is removed after successful restore.

EXAMPLES:
  sedx backup restore 20250110-120000-abc123    Restore from backup")]
    Restore {
        /// Backup ID
        #[arg(value_name = "ID")]
        id: String,
    },

    /// Remove a backup
    #[command(long_about = "Remove a specific backup.

Permanently deletes a backup and frees disk space.
Use with caution - this cannot be undone.

EXAMPLES:
  sedx backup remove 20250110-120000-abc123    Remove backup")]
    Remove {
        /// Backup ID
        #[arg(value_name = "ID")]
        id: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Prune old backups
    #[command(long_about = "Remove old backups, keeping only recent ones.

Helps manage disk space by removing old backups.
You can keep a certain number of recent backups, or backups from recent days.

OPTIONS:
  --keep=N         Keep only N most recent backups (default: 10)
  --keep-days=N    Keep only backups from last N days

EXAMPLES:
  sedx backup prune --keep=5                 Keep only 5 most recent
  sedx backup prune --keep-days=7            Keep only last 7 days
  sedx backup prune --keep=5 --force         Skip confirmation")]
    Prune {
        /// Number of recent backups to keep
        #[arg(long, value_name = "N")]
        keep: Option<usize>,

        /// Keep backups from last N days
        #[arg(long, value_name = "N")]
        keep_days: Option<usize>,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
}

pub fn parse_args() -> Result<Args> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Rollback { id }) => Ok(Args::Rollback { id }),
        Some(Commands::History) => Ok(Args::History),
        Some(Commands::Status) => Ok(Args::Status),
        Some(Commands::Config { show }) => Ok(Args::Config { show }),
        Some(Commands::Backup { action }) => match action {
            BackupAction::List { verbose } => Ok(Args::BackupList { verbose }),
            BackupAction::Show { id } => Ok(Args::BackupShow { id }),
            BackupAction::Restore { id } => Ok(Args::BackupRestore { id }),
            BackupAction::Remove { id, force } => Ok(Args::BackupRemove { id, force }),
            BackupAction::Prune { keep, keep_days, force } => Ok(Args::BackupPrune { keep, keep_days, force }),
        },
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
                no_backup: cli.no_backup,
                backup_dir: cli.backup_dir,
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
        no_backup: bool,
        backup_dir: Option<String>,
    },
    Rollback {
        id: Option<String>,
    },
    History,
    Status,
    BackupList {
        verbose: bool,
    },
    BackupShow {
        id: String,
    },
    BackupRestore {
        id: String,
    },
    BackupRemove {
        id: String,
        force: bool,
    },
    BackupPrune {
        keep: Option<usize>,
        keep_days: Option<usize>,
        force: bool,
    },
    Config {
        show: bool,
    },
}
