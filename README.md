# SedX

> A safe, modern replacement for GNU `sed` with automatic backups, preview mode, and one-command rollback.

[![CI](https://github.com/InkyQuill/sedx/actions/workflows/ci.yml/badge.svg)](https://github.com/InkyQuill/sedx/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.87%2B-orange.svg)](https://www.rust-lang.org)

**SedX** is a modern stream editor written in Rust that maintains ~90% compatibility with GNU `sed` while adding critical safety features:

- **Automatic backups** before every file modification
- **Preview mode** to see changes before applying
- **One-command rollback** to undo mistakes
- **Human-readable diffs** with colored context
- **Streaming mode** for large files (100GB+ with <100MB RAM)
- **Modern regex** (PCRE) as the default

## Table of Contents

- [Quick Start](#quick-start)
- [Why SedX?](#why-sedx)
- [Installation](#installation)
- [Basic Usage](#basic-usage)
- [Regex Modes](#regex-modes)
- [Common Operations](#common-operations)
- [Backup & Rollback](#backup--rollback)
- [Pipeline Mode](#pipeline-mode)
- [Large Files](#large-files)
- [Configuration](#configuration)
- [Command Reference](#command-reference)
- [Limitations](#limitations)
- [Examples](#examples)
- [Migration from GNU sed](#migration-from-gnu-sed)

## Quick Start

```bash
# Install
cargo install sedx

# Preview changes before applying
sedx --dry-run 's/old/new/g' file.txt

# Apply safely (automatic backup created)
sedx 's/old/new/g' file.txt

# Rollback if needed
sedx rollback
```

## Why SedX?

GNU `sed` is powerful but unforgiving—one mistake with `-i` can permanently corrupt your files. SedX gives you the same power with safety rails:

| Feature | GNU Sed | SedX |
|---------|---------|------|
| Preview changes | ❌ No | ✅ `--dry-run` |
| Automatic backups | ❌ No | ✅ Yes (default) |
| Rollback | ❌ No | ✅ `sedx rollback` |
| Colored diffs | ❌ No | ✅ Yes |
| Streaming for 100GB+ files | ⚠️ Requires tweaks | ✅ Auto-detects |
| Default regex | BRE (1970s) | ✅ PCRE (modern) |

**Key difference:** SedX uses **PCRE** (Perl-Compatible Regular Expressions) by default—the same regex flavor used in Perl, Python, JavaScript, and most modern tools. No more escaping parentheses!

## Installation

### From crates.io (Recommended)

```bash
cargo install sedx
export PATH="$HOME/.cargo/bin:$PATH"
```

### From Source

```bash
git clone https://github.com/InkyQuill/sedx.git
cd sedx
cargo build --release
sudo cp target/release/sedx /usr/local/bin/
```

### From GitHub Releases

Download the latest release from [Releases](https://github.com/InkyQuill/sedx/releases) and add to your PATH.

## Basic Usage

### Substitution

```bash
# Replace first occurrence on each line
sedx 's/foo/bar/' file.txt

# Replace all occurrences (global)
sedx 's/foo/bar/g' file.txt

# Case-insensitive substitution
sedx 's/foo/bar/gi' file.txt

# Numbered substitution (replace 3rd occurrence)
sedx 's/foo/bar/3' file.txt
```

### Line-Specific Operations

```bash
# Operate on specific line
sedx '10s/foo/bar/' file.txt           # Line 10 only
sedx '5,10s/foo/bar/g' file.txt        # Lines 5-10
sedx '10,$s/foo/bar/g' file.txt        # Line 10 to end

# Operate on lines matching pattern
sedx '/error/s/test/fix/' file.txt      # Lines containing "error"
sedx '/start/,/end/s/foo/bar/g' file.txt # From "start" pattern to "end"
```

### Deleting Lines

```bash
# Delete specific line
sedx '10d' file.txt

# Delete range
sedx '5,10d' file.txt                  # Lines 5-10

# Delete matching lines
sedx '/DEBUG/d' logfile.txt            # Lines containing DEBUG
sedx '/keep/!d' file.txt               # Delete everything EXCEPT lines with "keep"

# Delete from pattern to end
sedx '/error/,$d' logfile.txt
```

### Multiple Commands

```bash
# Using multiple -e flags
sedx -e 's/foo/bar/' -e 's/baz/qux/' file.txt

# Using command grouping with semicolons
sedx '{s/foo/bar/; s/baz/qux/}' file.txt

# Grouping with range
sedx '1,10{s/a/A/g; s/b/B/g}' file.txt
```

## Regex Modes

SedX supports three regex flavors, selectable via command-line flags:

### PCRE (Default) - Modern Syntax

```bash
sedx 's/(foo|bar)/baz/g' file.txt
```

- Unescaped metacharacters: `(` `)`, `{` `}`, `+`, `?`, `|`, `.`
- Backreferences in replacement: `$1`, `$2`, etc.
- Most powerful and familiar to modern developers

### ERE Mode - sed -E Compatible

```bash
sedx -E 's/(foo|bar)/baz/g' file.txt
```

- Extended Regular Expressions
- Compatible with `sed -E` and BSD sed
- Same syntax as PCRE for most operations

### BRE Mode - GNU sed Compatible

```bash
sedx -B 's/\(foo\|bar\)/baz/g' file.txt
```

- Basic Regular Expressions (GNU sed default)
- Escaped metacharacters: `\(` `\)`, `\{` `\}`, `\+`, `\?`, `\|`
- Backreferences in replacement: `\1`, `\2` (converted to PCRE internally)

### Backreference Conversion

| Mode | Pattern Syntax | Replacement Syntax |
|------|---------------|-------------------|
| PCRE | `(foo\|bar)` | `$1`, `$2` |
| ERE | `(foo\|bar)` | `\1`, `\2` (auto-converted to `$1`, `$2`) |
| BRE | `\(foo\|bar\)` | `\1`, `\2` (auto-converted to `$1`, `$2`) |

## Common Operations

### Print Commands

```bash
# Print specific lines
sedx -n '1,10p' file.txt             # Print lines 1-10 only
sedx -n '/error/p' logfile.txt        # Print only lines matching "error"

# Print line numbers
sedx '=' file.txt                     # Add line numbers before each line
```

### Insert, Append, Change

```bash
# Insert text before line 5
sedx '5i\Text to insert' file.txt

# Append text after line 5
sedx '5a\Text to append' file.txt

# Replace line 5 with new text
sedx '5c\New line content' file.txt

# Change lines matching pattern
sedx '/error/c\ERROR FOUND' logfile.txt
```

### Hold Space Operations

```bash
# Copy pattern space to hold space
sedx '{h; g}' file.txt               # Save and restore

# Exchange pattern and hold space
sedx 'x' file.txt

# Append to hold space
sedx 'H' file.txt
```

### Flow Control

```bash
# Labels and branches
sedx ':top; s/foo/bar/; /condition/b top' file.txt

# Conditional branch (if substitution made)
sedx 's/foo/bar/; t success; s/baz/qux/; :success' file.txt

# Branch if NO substitution made
sedx 's/foo/bar/; T retry; b done; :retry; s/baz/qux/; :done' file.txt
```

## Backup & Rollback

Every file modification automatically creates a backup in `~/.sedx/backups/`:

```bash
# Apply changes (backup created automatically)
sedx 's/foo/bar/g' file.txt
# Output: Backup ID: 20260226-120000-abc123

# View backup history
sedx history

# Rollback last operation
sedx rollback

# Rollback specific backup
sedx rollback 20260226-120000-abc123

# Check backup status
sedx status
```

### Backup Management

```bash
# List all backups
sedx backup list

# Clean old backups (keep last 10)
sedx backup prune --keep=10

# Use custom backup directory
sedx --backup-dir /mnt/backups 's/foo/bar/' file.txt
```

### Disable Backups

```bash
# Skip backup (requires --force confirmation)
sedx --no-backup --force 's/foo/bar/' file.txt
```

⚠️ **Warning:** Use `--no-backup` only for files under version control where you can revert mistakes.

## Pipeline Mode

When no files are specified, SedX reads from **stdin** and writes to **stdout**:

```bash
# Basic pipeline
echo "hello world" | sedx 's/hello/HELLO/'
# Output: HELLO world

# Chain with other commands
cat file.txt | sedx 's/foo/bar/g' | grep bar

# Filter logs
docker logs app 2>/dev/null | sedx '/DEBUG/d'

# Multiple commands in pipeline
echo "test case" | sedx '{s/test/TEST/; s/case/CASE/}'
# Output: TEST CASE
```

**Pipeline mode characteristics:**
- ✅ No backups created (can't backup a stream)
- ✅ No diff output (only transformed text)
- ✅ Works with all regex modes (PCRE, ERE, BRE)
- ✅ Exit status: 0 on success, non-zero on errors

## Large Files

SedX automatically switches to **streaming mode** for files ≥100MB:

```bash
# Automatically uses streaming for large files
sedx 's/foo/bar/g' large.log

# Force streaming mode
sedx --streaming 's/foo/bar/g' file.txt

# Disable streaming (force in-memory)
sedx --no-streaming 's/foo/bar/g' file.txt
```

**Streaming benefits:**
- Constant memory usage (<100MB regardless of file size)
- Can process 100GB+ files efficiently
- Sliding window diff for context around changes

## Configuration

Create or edit `~/.sedx/config.toml`:

```toml
[backup]
max_size_gb = 10                    # Maximum backup size
max_disk_usage_percent = 80          # Warn before using this much disk
backup_dir = "/custom/path"          # Custom backup location

[compatibility]
mode = "pcre"                        # Default regex: pcre, ere, or bre
show_warnings = true                  # Show compatibility warnings

[processing]
context_lines = 2                    # Default diff context lines
max_memory_mb = 100                  # Streaming threshold (file size)
streaming = true                     # Enable streaming mode
```

```bash
# Edit configuration
sedx config

# View current configuration
sedx config --show
```

## Command Reference

### Options

| Option | Description |
|--------|-------------|
| `-e, --expression <EXPR>` | Add a sed expression (can be used multiple times) |
| `-f, --file <SCRIPT_FILE>` | Read script from file |
| `-d, --dry-run` | Preview changes without modifying files |
| `-i, --interactive` | Prompt before applying changes |
| `--context <NUM>` | Number of context lines in diff (default: 2) |
| `--no-context` | Show only changed lines |
| `-n, --quiet` | Suppress automatic output (only `p` command shows output) |
| `-B, --bre` | Use Basic Regular Expressions (GNU sed compatible) |
| `-E, --ere` | Use Extended Regular Expressions (sed -E compatible) |
| `--no-backup` | Skip backup (requires `--force`) |
| `--force` | Force dangerous operations |
| `--backup-dir <DIR>` | Custom backup directory |
| `--streaming` | Enable streaming mode |
| `--no-streaming` | Disable streaming mode |
| `-h, --help` | Print help |
| `-V, --version` | Print version |

### Subcommands

| Command | Description |
|---------|-------------|
| `rollback [ID]` | Undo last operation or specific backup |
| `history` | Show operation history |
| `status` | Show backup status and disk usage |
| `backup list` | List all backups |
| `backup prune --keep=N` | Keep only N most recent backups |
| `config` | Edit configuration file |
| `config --show` | View current configuration |
| `help` | Print help message |

## Limitations

SedX aims for ~90% GNU sed compatibility. The following are **NOT** yet implemented:

| Feature | Status | Alternative |
|---------|--------|-------------|
| `y` command (translate characters) | Not supported | Use multiple `s` commands |
| `l` command (list lines escaped) | Not supported | N/A |
| `\L`, `\U` in replacement (case conversion) | Not supported | Post-process with other tools |
| Word boundaries `\<`, `\>` | Not supported | Use `\b` in PCRE mode |

### Known Issues

See [tests/KNOWN_ISSUES.md](tests/KNOWN_ISSUES.md) for detailed limitations.

## Examples

### Config File Updates

```bash
# Preview version update
sedx --dry-run 's/version=[0-9.]+/version=2.0/' config.toml

# Apply
sedx 's/version=[0-9.]+/version=2.0/' config.toml
```

### Log File Cleanup

```bash
# Remove all DEBUG lines
sedx '/DEBUG/d' app.log

# Keep only ERROR lines
sedx '/error/!d' app.log
```

### Batch File Processing

```bash
# Process multiple files
sedx 's/foo/bar/g' *.txt

# With specific directory
sedx 's/old/new/g' src/**/*.rs
```

### Complex Pattern Matching

```bash
# Email address redaction (PCRE default)
sedx 's/\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b/REDACTED/g' file.txt

# Remove duplicate words
sedx 's/\b(\w+)\s+\1\b/$1/g' file.txt

# Number lines (add prefix)
sedx 's/^/LINE /' file.txt
```

## Migration from GNU sed

### Simple Substitutions

No changes needed—syntax is identical:

```bash
# GNU sed
sed 's/foo/bar/g' file.txt

# SedX (same syntax)
sedx 's/foo/bar/g' file.txt
```

### Regex Patterns

**GNU sed (BRE)** → **SedX (PCRE default)**

```bash
# GNU sed
sed 's/\(foo\|bar\)/baz/g' file.txt

# SedX Option 1: Use PCRE (recommended)
sedx 's/(foo|bar)/baz/g' file.txt

# SedX Option 2: Use BRE mode for exact compatibility
sedx -B 's/\(foo\|bar\)/baz/g' file.txt
```

### In-Place Editing

```bash
# GNU sed (destructive)
sed -i 's/foo/bar/' file.txt

# SedX (safe with backup)
sedx 's/foo/bar/' file.txt

# SedX (no backup, like GNU sed)
sedx --no-backup --force 's/foo/bar/' file.txt
```

### Extended Regex

```bash
# GNU sed with -E
sed -E 's/(foo|bar)/baz/g' file.txt

# SedX with -E (same behavior)
sedx -E 's/(foo|bar)/baz/g' file.txt

# SedX default (PCRE syntax same as ERE)
sedx 's/(foo|bar)/baz/g' file.txt
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

```bash
# Run tests
cargo test

# Run integration tests
./tests/run_all_tests.sh

# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Support

- [Issues](https://github.com/InkyQuill/sedx/issues)
- [Discussions](https://github.com/InkyQuill/sedx/discussions)

---

**Made with ❤️ and Rust by InkyQuill**
