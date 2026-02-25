# SedX User Guide

**Last Updated:** 2025-02-25
**Version:** 0.2.6

**SedX** is a safe, modern replacement for GNU `sed` with automatic backups, preview mode, and rollback functionality. This guide covers installation, basic usage, and core features.

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Backup System](#backup-system)
- [Configuration](#configuration)
- [Common Use Cases](#common-use-cases)
- [Command Reference](#command-reference)
- [Troubleshooting](#troubleshooting)

---

## Installation

### Method 1: Install via Cargo (Recommended)

```bash
# Install from crates.io
cargo install sedx

# Or install directly from GitHub
cargo install --git https://github.com/InkyQuill/sedx.git
```

Add `~/.cargo/bin` to your PATH if not already there:

```bash
# For Bash, add to ~/.bashrc
export PATH="$HOME/.cargo/bin:$PATH"

# For Zsh, add to ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"

# For Fish, add to ~/.config/fish/config.fish
fish_add_path ~/.cargo/bin
```

### Method 2: Build from Source

```bash
# Clone the repository
git clone https://github.com/InkyQuill/sedx.git
cd sedx

# Build release version
cargo build --release

# Install to system path (optional)
sudo cp target/release/sedx /usr/local/bin/
```

### Method 3: Download Binary

```bash
# Download latest release from GitHub
wget https://github.com/InkyQuill/sedx/releases/latest/download/sedx-x86_64-unknown-linux-gnu.tar.gz

# Extract and install
tar -xzf sedx-x86_64-unknown-linux-gnu.tar.gz
sudo cp sedx /usr/local/bin/
```

### Verify Installation

```bash
$ sedx --version
sedx 0.2.6

Copyright (c) 2025 InkyQuill
License: MIT
Source: https://github.com/InkyQuill/sedx
Rust Edition: 2024
```

---

## Quick Start

### 1. Basic Substitution

Replace text in a file with automatic backup:

```bash
sedx 's/foo/bar/g' file.txt
```

Output:
```
file.txt
L3: ~ bar
L5: ~ bar baz

Total: 2 changes (2 modified)
Backup ID: 20250225-120000-abc123
Rollback with: sedx rollback 20250225-120000-abc123
```

### 2. Preview Changes (Dry Run)

See what will change without modifying files:

```bash
sedx --dry-run 's/version=[0-9]+/version=2.0/' config.txt
```

Output:
```
Dry run: s/version=[0-9]+/version=2.0/

config.txt
L15: = # Application config
L16: = app_name=MyApp
L17: ~ version=2.0
L18: = debug=true
...
```

### 3. Rollback Changes

Undo the last operation:

```bash
sedx rollback
```

Or rollback a specific backup:

```bash
sedx rollback 20250225-120000-abc123
```

### 4. Interactive Mode

Ask for confirmation before applying changes:

```bash
sedx --interactive 's/localhost/0.0.0.0/g' docker-compose.yml
```

### 5. Pipeline Usage

Process stdin/stdout (no backups created):

```bash
cat file.txt | sedx 's/foo/bar/g' | grep bar
```

---

## Backup System

SedX automatically creates backups for every file modification. This is the key safety feature that distinguishes SedX from GNU sed.

### How Backups Work

When you run a command that modifies files:

1. **Backup created** - Original files are copied to `~/.sedx/backups/<timestamp-id>/`
2. **Changes applied** - Files are modified with your expression
3. **Backup ID shown** - Unique ID for rollback is displayed
4. **Automatic cleanup** - Last 50 backups are kept, older ones are removed

### Backup Location

```
~/.sedx/backups/
├── 20250225-120000-abc123/
│   ├── operation.json    # Metadata (expression, timestamp, files)
│   └── files/
│       ├── config.txt
│       └── data.json
└── 20250225-120500-def456/
    ├── operation.json
    └── files/
        └── logfile.txt
```

### Viewing Backup History

```bash
$ sedx history
ID: 20250225-120500-def456
  Time: 2025-02-25 12:05:00
  Command: /error/d
  Files: 3

ID: 20250225-120000-abc123
  Time: 2025-02-25 12:00:00
  Command: s/version=[0-9]+/version=2.0/
  Files: 1
```

### Rollback Operations

```bash
# Rollback the most recent operation
sedx rollback

# Rollback a specific backup
sedx rollback 20250225-120000-abc123

# Check backup status
sedx status
```

### Backup Management

```bash
# List all backups with details
sedx backup list

# Show specific backup details
sedx backup show 20250225-120000-abc123

# Restore from backup (removes backup after restore)
sedx backup restore 20250225-120000-abc123

# Remove a specific backup
sedx backup remove 20250225-120000-abc123

# Prune old backups (keep only 10 most recent)
sedx backup prune --keep=10

# Prune backups older than 7 days
sedx backup prune --keep-days=7
```

### Disabling Backups

**Warning:** Use with caution. Changes cannot be undone without backups!

```bash
# Skip backup creation (requires --force flag)
sedx --no-backup --force 's/foo/bar/' file.txt
```

This is recommended only for:
- Files under version control (git)
- Temporary files
- Files that can be easily regenerated

---

## Configuration

SedX stores configuration in `~/.sedx/config.toml`. The file is automatically created with defaults on first run.

### Viewing Configuration

```bash
# View current configuration
sedx config --show

# Edit configuration in your default editor
sedx config
```

### Configuration File

```toml
# SedX Configuration File
# Location: ~/.sedx/config.toml

[backup]
# Maximum backup size in GB before warning (default: 2)
max_size_gb = 2

# Maximum percentage of free space to use for backups (default: 60)
max_disk_usage_percent = 60

# Custom backup directory (optional)
# backup_dir = "/mnt/backups/sedx"

[compatibility]
# Regex mode: "pcre" (default), "ere", or "bre"
# pcre - Perl-Compatible Regular Expressions (most modern)
# ere  - Extended Regular Expressions (like sed -E)
# bre  - Basic Regular Expressions (like GNU sed)
mode = "pcre"

# Show incompatibility warnings (default: true)
show_warnings = true

[processing]
# Number of context lines to show around changes (default: 2, max: 10)
context_lines = 2

# Maximum memory usage for streaming in MB (default: 100)
# Files larger than this threshold use streaming mode
max_memory_mb = 100

# Enable streaming mode for large files (default: true)
streaming = true
```

### Configuration Options

#### Backup Settings

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `max_size_gb` | float | 2.0 | Warn if backup exceeds this size (GB) |
| `max_disk_usage_percent` | float | 60.0 | Max % of free disk space to use |
| `backup_dir` | string | `~/.sedx/backups` | Custom backup location |

#### Compatibility Settings

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `mode` | string | "pcre" | Default regex flavor (pcre/ere/bre) |
| `show_warnings` | bool | true | Show incompatibility warnings |

#### Processing Settings

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `context_lines` | int | 2 | Context lines around changes (0-10) |
| `max_memory_mb` | int | 100 | Streaming threshold in MB |
| `streaming` | bool | true | Enable streaming for large files |

---

## Common Use Cases

### System Administration

#### Update Configuration Files

```bash
# Change database host in all config files
sedx 's/db\.host=localhost/db.host=prod-db.example.com/' config/*.toml

# Update multiple ports at once
sedx 's/port=300[0-9]/port=8080/' docker-compose.yml
```

#### Clean Log Files

```bash
# Remove debug lines from logs
sedx '/DEBUG/d' /var/log/app.log

# Keep only error lines
sedx '/ERROR/!d' /var/log/app.log

# Sanitize log files (remove sensitive data)
sedx 's/password=[^ ]+/password=REDACTED/g' access.log
```

#### Comment Out Configuration Sections

```bash
# Comment out all SELinux settings
sedx '/^SELINUX=/s/^/# /' /etc/selinux/config

# Uncomment a setting
sedx '/^# *ServerName/s/^# //' /etc/httpd/conf/httpd.conf
```

### Development Workflows

#### Refactor Code

```bash
# Rename function across all files
sedx 's/oldFunctionName/newFunctionName/g' src/**/*.rs

# Update import paths
sedx 's|from old\.module|from new.module|g' **/*.py

# Change method calls
sedx 's/\.count(/\.length(/g' src/**/*.js
```

#### Fix Common Issues

```bash
# Fix trailing whitespace
sedx 's/[[:space:]]*$//' **/*.py

# Convert tabs to spaces (4 spaces)
sedx 's/\t/    /g' Makefile

# Fix line endings (CRLF to LF)
sedx 's/\r$//' *.txt
```

#### Add License Headers

```bash
# Insert license at top of file
sedx '1i\
// Copyright (c) 2025 My Company\
// SPDX-License-Identifier: MIT
' src/*.rs
```

### Data Processing

#### CSV Manipulation

```bash
# Replace comma with tab in CSV file
sedx 's/,/\t/g' data.csv > data.tsv

# Extract specific column (3rd column)
sedx 's/^\([^,]*,\)\{2\}[^,]*/\1/' data.csv

# Remove quotes from CSV fields
sedx 's/"//g' data.csv
```

#### Text Transformation

```bash
# Convert to uppercase (specific pattern)
sedx 's/<title>/\U&/g' index.html

# Remove duplicate words
sedx -E 's/([a-z]+) \1/\1/g' text.txt

# Format numbers (add thousand separators)
sedx -E 's/([0-9])([0-9]{3})/\1,\2/g' numbers.txt
```

#### Data Extraction

```bash
# Extract email addresses from text
sedx -n 's/.*\([a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]\{2,\}\).*/\1/p' emails.txt

# Extract URLs
sedx -n 's/.*\(https\?:\/\/[^ "]*\).*/\1/p' webpage.html

# Extract IP addresses
sedx -n 's/.*\([0-9]\{1,3\}\.[0-9]\{1,3\}\.[0-9]\{1,3\}\.[0-9]\{1,3\}\).*/\1/p' logs/*.log
```

### Batch File Processing

```bash
# Process multiple files
sedx 's/foo/bar/g' *.txt

# Process files recursively with find
find . -name "*.log" -exec sedx 's/old/new/g' {} \;

# Rename files (prepare mv commands)
sedx -n 's/picture_\([0-9]\+\)\.jpg/mv & photo_\1.jpg/p' *.jpg | sh
```

---

## Command Reference

### Substitution Command

**Syntax:** `[range]s/pattern/replacement/[flags]`

```bash
# Basic substitution (first occurrence per line)
sedx 's/foo/bar/' file.txt

# Global substitution (all occurrences per line)
sedx 's/foo/bar/g' file.txt

# Case-insensitive matching
sedx 's/foo/bar/i' file.txt

# Substitute Nth occurrence
sedx 's/foo/bar/3' file.txt

# On specific line
sedx '10s/foo/bar/' file.txt

# On range of lines
sedx '1,10s/foo/bar/g' file.txt

# With pattern range
sedx '/start/,/end/s/foo/bar/g' file.txt

# Only on lines matching pattern
sedx '/config/s/true/false/g' config.txt
```

### Delete Command

**Syntax:** `[range]d`

```bash
# Delete specific line
sedx '10d' file.txt

# Delete range
sedx '1,10d' file.txt

# Delete lines matching pattern
sedx '/error/d' logfile.txt

# Delete lines between patterns
sedx'/start/,/end/d' file.txt

# Delete lines NOT matching pattern
sedx '/keep/!d' file.txt
```

### Print Command

**Syntax:** `[range]p`

```bash
# Print specific line
sedx '10p' file.txt

# Print range (with -n flag)
sedx -n '1,10p' file.txt

# Print matching lines
sedx -n '/pattern/p' file.txt
```

### Insert/Append/Change Commands

```bash
# Insert text before line
sedx '5i\NEW LINE' file.txt

# Append text after line
sedx '5a\NEW LINE' file.txt

# Change line
sedx '5c\REPLACED CONTENT' file.txt

# Change range
sedx '1,5c\ALL THESE LINES REPLACED' file.txt
```

### Quit Command

```bash
# Quit after processing N lines
sedx '10q' large_file.txt

# Quit at pattern match
sedx '/STOP/q' data.txt

# Quit without printing (Phase 4)
sedx '10Q' file.txt
```

### Hold Space Commands

```bash
# h - Copy pattern space to hold space
sedx '1h; 1d; $G' file.txt  # Move first line to end

# H - Append pattern space to hold space
sedx '1,5H; $g' file.txt  # Accumulate lines, output at end

# g - Copy hold space to pattern space
sedx '1h; 10g' file.txt  # Copy line 1 to line 10

# G - Append hold space to pattern space
sedx 'G' file.txt  # Duplicate each line

# x - Exchange pattern and hold space
sedx '1h; 2x; 2g' file.txt  # Swap lines 1 and 2
```

### Flow Control Commands (Phase 5)

```bash
# Label definition
:label

# Unconditional branch
b label

# Branch if substitution made
t label

# Branch if NO substitution made
T label

# Example: Loop until no more changes
sedx ':loop; s/foo/bar/; t loop' file.txt
```

### File I/O Commands (Phase 5)

```bash
# Read file contents
sedx '1r header.txt' file.txt

# Write pattern space to file
sedx 'w output.txt' file.txt

# Read one line from file
sedx 'R data.txt' file.txt

# Write first line to file
sedx 'W errors.log' logfile.txt
```

### Additional Commands (Phase 5)

```bash
# Print line number
sedx '=' file.txt

# Print filename
sedx 'F' file.txt

# Clear pattern space
sedx '/unwanted/{z; s/EMPTY/cleaned/}' file.txt
```

### Command Line Flags

| Flag | Long Form | Description |
|------|-----------|-------------|
| `-d` | `--dry-run` | Preview changes without applying |
| `-i` | `--interactive` | Ask for confirmation before applying |
| `-n` | `--quiet` | Suppress automatic output |
| `-E` | `--ere` | Use Extended Regular Expressions |
| `-B` | `--bre` | Use Basic Regular Expressions (GNU sed) |
| `--no-context` | | Show only changed lines (no context) |
| `--context N` | | Show N lines of context (0-10) |
| `--streaming` | | Enable streaming mode for large files |
| `--no-streaming` | | Disable streaming mode |
| `--no-backup` | | Skip backup creation (requires --force) |
| `--force` | | Force dangerous operations |
| `--backup-dir` | | Custom backup directory |

---

## Troubleshooting

### "Command not found" after installation

Add Cargo bin directory to your PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Add this line to your shell configuration file (`~/.bashrc`, `~/.zshrc`, etc.).

### Backups filling disk space

SedX automatically keeps only the last 50 backups. To manually clean up:

```bash
# Keep only 10 most recent backups
sedx backup prune --keep=10

# Check backup status
sedx status
```

### Large file processing is slow

SedX automatically uses streaming mode for files >= 100MB. If processing is still slow:

```bash
# Verify streaming is enabled in config
sedx config --show | grep streaming

# Enable streaming if disabled
sedx config  # Edit file, set streaming = true
```

### Regex doesn't match as expected

SedX uses PCRE by default, not BRE like GNU sed. Try:

```bash
# Use BRE mode for exact GNU sed compatibility
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt

# Or convert your regex to PCRE format
sedx 's/(foo)(bar)/$2$1/' file.txt
```

### Changes not applied in pipeline mode

Pipes use stdin mode, which doesn't create backups or show diffs:

```bash
# This shows only transformed text
cat file.txt | sedx 's/foo/bar/g'

# Use file mode for backups and diffs
sedx 's/foo/bar/g' file.txt
```

### Pattern ranges not working

Pattern ranges use state machine semantics:

```bash
# /start/,/end/ includes lines from first "start" match to first "end" match
# NOT all occurrences of the range

# Use line numbers for predictable ranges
sedx '10,20s/foo/bar/g' file.txt
```

For more help, see:

- `sedx --help` - Command-line help
- [MIGRATION_GUIDE.md](MIGRATION_GUIDE.md) - Migrating from GNU sed
- [EXAMPLES.md](EXAMPLES.md) - 50+ practical examples
- GitHub Issues: https://github.com/InkyQuill/sedx/issues
