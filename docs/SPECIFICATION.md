# SedX System Specification

**Version:** 1.0.0 (Target)
**Current Version:** 0.2.0-alpha (neo branch)
**Last Updated:** 2025-01-08

---

## Table of Contents

1. [Overview & Vision](#1-overview--vision)
2. [Regex Flavor System](#2-regex-flavor-system)
3. [CLI Reference](#3-cli-reference)
4. [Sed Commands Reference](#4-sed-commands-reference)
5. [Substitution System](#5-substitution-system)
6. [Address Types](#6-address-types)
7. [Backup System](#7-backup-system)
8. [Usage Patterns](#8-usage-patterns)
9. [Migration Guide (sed → sedx)](#9-migration-guide-sed--sedx)
10. [Configuration](#10-configuration)

---

## 1. Overview & Vision

### Project Mission

SedX is a **safe, modern text processing tool** that combines the power of GNU sed with modern regex capabilities and enhanced safety features.

### Core Principles

1. **Safety First**: Automatic backups, disk space awareness, clear warnings
2. **Modern Regex**: PCRE (Perl-Compatible Regular Expressions) by default
3. **Sed Compatibility**: BRE/ERE modes for GNU sed compatibility
4. **Stream Processing**: Handle files of any size with minimal memory
5. **User Guidance**: Clear communication about operations and features
6. **Pragmatic Design**: 95% sed compatibility for 90% of use cases

### Design Philosophy

> **"Make safe text processing accessible with modern regex capabilities"**

- Default to modern PCRE regex syntax (most powerful)
- Support BRE/ERE modes for GNU sed compatibility
- Default to safe behavior (backups on, dry-run first)
- Warn about potential issues (disk space, compatibility)
- Provide escape hatches (`--no-backup --force`, `-B`/`-E` flags)
- Educate users through helpful messages
- Maintain predictability for sed users

### Target Use Cases

| Use Case | Example | SedX Advantage |
|----------|---------|-----------------|
| **Log Analysis** | Process 100GB logs | Stream processing, <100MB RAM |
| **Config Management** | Replace values in configs | Backup + rollback, preview |
| **DevOps Automation** | CI/CD text transformations | `--no-backup`, reliable |
| **Data Processing** | ETL on text files | Pipe-friendly, fast |
| **System Administration** | Safe bulk edits | Dry-run, interactive, diffs |
| **Development** | Modern regex patterns | PCRE features: lookaheads, named groups |

---

## 2. Regex Flavor System

SedX supports **three regex flavors** to provide both modern capabilities and GNU sed compatibility.

### 2.1 PCRE Mode (Default)

**Goal:** Modern, powerful regex processing

**Behavior:**
- Perl-Compatible Regular Expressions
- Most advanced features available
- Best for: New projects, developers familiar with modern regex

**Features:**
- Extended syntax: `()`, `{}`, `+`, `?`, `|` without escaping
- Named capture groups: `(?P<name>...)`
- Non-capturing groups: `(?:...)`
- Lookaheads: `(?=...)`, `(?!...)`
- Lookbehinds: `(?<=...)`, `(?<!...)`
- Atomic groups: `(?>...)`
- Possessive quantifiers: `?+`, `*+`, `++`
- Inline flags: `(?i)`, `(?m)`, `(?s)`

**Example:**
```bash
# Modern syntax (default)
$ sedx 's/(foo|bar)/baz/g' file.txt  # No backslash needed

# Named groups
$ sedx 's/(?P<word>\w+)/<\1>/g' file.txt

# Lookahead
$ sedx 's/foo(?=bar)/FOO/g' file.txt  # Match "foo" only before "bar"
```

---

### 2.2 BRE Mode (`-B` flag)

**Goal:** 100% GNU sed compatibility

**Behavior:**
- Basic Regular Expressions
- Escaped metacharacters: `\(`, `\)`, `\{`, `\}`, `\+`, `\?`, `\|`
- Automatic conversion to PCRE for processing
- Best for: Drop-in GNU sed replacement

**Internal Conversion:**
All BRE patterns are automatically converted to PCRE before compilation:
- `\(` → `(`
- `\)` → `)`
- `\+` → `+`
- `\?` → `?`
- `\|` → `|`
- `\1` → `$1` (in replacement)
- `\&` → `$&` (in replacement)

**Example:**
```bash
# BRE mode (GNU sed compatible)
$ sedx -B 's/\(foo\|bar\)/baz/g' file.txt  # Escaped metacharacters
# Internally converts to: s/(foo|bar)/baz/g
# Compiled as PCRE for execution

# Backreferences (BRE style)
$ sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt  # "foobar" → "barfoo"
# Internally converts \1, \2 to $1, $2
# Compiled as PCRE for execution
```

---

### 2.3 ERE Mode (`-E` flag)

**Goal:** `sed -E` compatibility

**Behavior:**
- Extended Regular Expressions
- Automatic conversion of backreferences to PCRE format for processing
- Best for: Scripts written for `sed -E`

**Internal Conversion:**
ERE patterns are already PCRE-compatible in syntax, but backreferences in replacements are converted:
- `\1` → `$1` (in replacement)
- Pattern syntax: `()`, `{}`, `+`, `?`, `|` all pass through unchanged

**Example:**
```bash
# ERE mode (sed -E compatible)
$ sedx -E 's/(foo|bar)/baz/g' file.txt  # Extended syntax
# Pattern passes through unchanged (already PCRE-compatible)
# Compiled as PCRE for execution

# Same as GNU sed -E
$ sed -E 's/(foo|bar)+/baz/g' file.txt
$ sedx -E 's/(foo|bar)+/baz/g' file.txt  # Same result
```

**ERE Syntax:**
- Extended metacharacters: `()`, `{}`, `+`, `?`, `|` (no escaping needed)
- Basic POSIX ERE features
- Backreferences: Uses `\1` in replacements (converted to `$1` internally)

### 2.4 Pipeline/Stdin Mode

**Goal:** Unix pipeline compatibility for stream processing

**Behavior:**
- When no files are specified, reads from stdin and writes to stdout
- Fully compatible with Unix pipes (`|`) and redirections (`>`, `<`)
- No backups, diffs, or rollback in pipeline mode (stream-only)
- All regex flavors work in pipeline mode

**Activation:**
```bash
# No files = stdin mode
echo "hello" | sedx 's/hello/HELLO/'
cat file.txt | sedx 's/foo/bar/g'
find . -name "*.log" | xargs cat | sedx 's/error/ERROR/gi'
```

**Characteristics:**
| Feature | File Mode | Pipeline Mode |
|---------|-----------|---------------|
| **Input** | From file(s) | From stdin |
| **Output** | To file(s) + diff | To stdout only |
| **Backups** | ✅ Created | ❌ Not created |
| **Diffs** | ✅ Shown | ❌ Not shown |
| **Rollback** | ✅ Available | ❌ Not available |
| **Dry-run** | ✅ Supported | N/A (use file mode) |
| **Regex flavors** | All | All |
| **All commands** | All | All |

**Use Cases:**
```bash
# Log filtering
docker logs nginx 2>/dev/null | sedx '/error/d' | tail -100

# Data transformation pipeline
cat data.csv | sedx 's/,/\t/g' | sort | uniq

# Config extraction
cat ~/.bashrc | sedx '/^export/p' | sedx 's/export //'

# Multi-file processing
find . -name "*.txt" -exec cat {} \; | sedx 's/old/new/g'

# Integration with other tools
ps aux | sedx '1d' | awk '{print $2}' | xargs kill -9

# Real-time monitoring
tail -f /var/log/syslog | sedx 's/error/ERROR/gi' | grep ERROR
```

**Exit Status:**
- `0`: Success, transformations applied
- `1`: Error in expression or processing
- `2`: File I/O error (file mode only)

**Limitations:**
- Cannot create backups (no file to backup)
- No diff output (stdout is the output stream)
- No rollback (stream is consumed)
- Dry-run mode not available (use file mode for previews)

---

## 3. CLI Reference

### 3.1 Main Command Structure

```bash
sedx [OPTIONS] [SUBCOMMAND] [EXPRESSION] [FILES...]
```

### 3.2 Global Flags

#### Operation Mode Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--dry-run` | `-d` | Preview changes without applying | `true` |
| `--execute` | | Apply changes to files | `false` |
| `--interactive` | `-i` | Ask before applying changes | `false` |
| `--stdout` | | Write to stdout, no backup | `false` |

**Note:** By default, SedX runs in dry-run mode for safety. Use `--execute` to apply changes.

#### Display Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--context` | `-n` | Number of context lines to show | `2` |
| `--no-context` | `--nc` | Show only changed lines | `false` |
| `--color` | | Color output: always, never, auto | `auto` |

#### Regex Flavor Flags

| Flag | Description | Default |
|------|-------------|---------|
| `-B` / `--bre` | Use Basic Regular Expressions (GNU sed compatible) | PCRE |
| `-E` / `--ere` | Use Extended Regular Expressions (sed -E compatible) | PCRE |
| (no flag) | Use PCRE (modern regex) | **PCRE** |

#### Backup Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--no-backup` | Skip backup creation (requires `--force`) | `false` |
| `--backup-dir` | Custom backup location | `~/.sedx/backups` |
| `--disk-limit` | Override disk usage threshold (0-100%) | From config |

#### Substitution Flags

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--case-insensitive` | `-i` | Case-insensitive matching | `false` |
| `--max-count` | | Maximum replacements per file | `unlimited` |
| `--max-replacements` | | Alias for `--max-count` | - |

**Note:** In-substitution flags (`g`, `i`, `p`, `1-9`) are specified in the expression itself, not as CLI flags.

#### Processing Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--quiet` | `-n` | Suppress automatic output | `false` |
| `--expression` | `-e` | Add multiple expressions | - |
| `--file` | `-f` | Read script from file | - |
| `--sandbox` | | Disable `e`, `r`, `w` commands | `false` |
| `--separate` | `-s` | Treat files separately | `false` |

#### Configuration Flags

| Flag | Description |
|------|-------------|
| `--config` | Open config file in $EDITOR |
| `--help` | Show help message |
| `--version` | Show version information |
| `--verbose` | `-v` | Verbose output |

---

### 3.3 Subcommands

#### Execute (Default)

```bash
sedx [OPTIONS] EXPRESSION FILES...
```

Applies sed expression to files.

**Examples:**
```bash
sedx 's/foo/bar/g' file.txt              # Preview
sedx --execute 's/foo/bar/g' file.txt     # Apply with backup
sedx --no-backup --force 's/foo/bar/g' file.txt  # Apply without backup
```

---

#### Rollback

```bash
sedx rollback [ID]
```

Restore files from backup.

**Arguments:**
- `ID` - Optional backup ID (defaults to last operation)

**Examples:**
```bash
sedx rollback                    # Rollback last operation
sedx rollback 20250107-143052    # Rollback specific backup
sedx rollback ~/.sedx/backups/20250107-143052  # Full path
```

---

#### History

```bash
sedx history
```

Display all operations.

**Output Format:**
```
2025-01-07 14:30:52  ID: 20250107-143052
  Expression: s/foo/bar/g
  Files: file.txt, config.yaml
  Size: 1.2 MB

2025-01-07 10:15:33  ID: 20250107-101533
  Expression: 5,10d
  Files: data.txt
  Size: 512 KB
```

---

#### Status

```bash
sedx status
```

Display backup status.

**Output Format:**
```
Current backup status:

Total backups: 15
Total size: 125.3 MB
Location: /home/user/.sedx/backups

Last operation:
  ID: 20250107-143052
  Time: 2025-01-07 14:30:52
  Expression: s/foo/bar/g
  Files: 1
```

---

#### Backup Management

```bash
sedx backup SUBCOMMAND
```

**Subcommands:**

**list** [-v, --verbose]
```bash
sedx backup list
sedx backup list --verbose
```

**show** <id>
```bash
sedx backup show 20250107-143052
```

**restore** <id>
```bash
sedx backup restore 20250107-143052
```

**remove** <id> [--force]
```bash
sedx backup remove 20250107-143052
sedx backup remove 20250107-143052 --force
```

**prune** [--keep=N] [--keep-days=N]
```bash
sedx backup prune --keep=10
sedx backup prune --keep-days=30
sedx backup prune --keep=5 --keep-days=7
```

---

#### Config

```bash
sedx config
```

Open configuration file in `$EDITOR`.

**Behavior:**
1. Opens `~/.sedx/config.toml` in editor
2. Validates syntax on save
3. Re-prompts if syntax invalid
4. Shows confirmation on success

**Example:**
```bash
$ sedx config
# Opens editor with:
# [backup]
# enabled = true
# max_size_gb = 10
# max_disk_usage_percent = 60
#
# [compatibility]
# mode = "extended"
# show_warnings = true
#
# User edits and saves
# Config validated successfully
```

---

## 4. Sed Commands Reference

### 4.1 Command Categories

#### Implemented (v0.2.6-alpha)

- ✅ **s** - Substitution
- ✅ **d** - Delete
- ✅ **a** - Append text
- ✅ **i** - Insert text
- ✅ **c** - Change text
- ✅ **p** - Print
- ✅ **q** - Quit
- ✅ **Q** - Quit without printing
- ✅ **n** - Next line (print, read next, continue)
- ✅ **N** - Next append (append newline + next line)
- ✅ **P** - Print first line of pattern space
- ✅ **D** - Delete first line, restart cycle
- ✅ **{}** - Command grouping
- ✅ **h** - Hold (copy to hold space)
- ✅ **H** - Hold append (append to hold space)
- ✅ **g** - Get (copy from hold space)
- ✅ **G** - Get append (append from hold space)
- ✅ **x** - Exchange (swap pattern and hold space)

**Note:** Multi-line commands (n, N, P, D) now work with full address/range support
in cycle-based execution mode. Commands are applied per-line with proper
state tracking across cycles, matching GNU sed's behavior.

#### Planned - Tier 2 (v0.5.0)

- 📋 **:** - Label definition
- 📋 **b** - Branch to label
- 📋 **t** - Branch if substitution made
- 📋 **T** - Branch if NO substitution made
- 📋 **r** - Read file
- 📋 **w** - Write to file
- 📋 **R** - Read one line from file
- 📋 **W** - Write first line to file

#### Planned - Tier 3 (v0.6.0)

- 📋 **y** - Translate characters
- 📋 **l** - List with escape sequences
- 📋 **=** - Print line number
- 📋 **F** - Print filename
- 📋 **e** - Execute shell command
- 📋 **z** - Clear pattern space

---

### 4.2 Command Reference (Implemented)

#### Substitution (`s`)

**Syntax:**
```
[addr1[,addr2]]s/pattern/replacement/[flags]
```

**Flags:**
- `g` - Global replacement (all occurrences in line)
- `i` - Case-insensitive matching
- `p` - Print line if substitution made
- `1-9` - Replace only Nth occurrence
- (Future: `w file` - Write to file)

**Examples:**
```bash
# Basic substitution
sedx 's/foo/bar/' file.txt

# Global substitution
sedx 's/foo/bar/g' file.txt

# Case-insensitive
sedx 's/foo/bar/i' file.txt

# With range
sedx '1,10s/foo/bar/g' file.txt

# Pattern-scoped
sedx '/error/s/test/fix/' file.txt

# Print on substitution
sedx -n 's/foo/bar/p' file.txt

# Numbered substitution
sedx 's/foo/bar/2' file.txt  # Replace only 2nd occurrence
```

---

#### Delete (`d`)

**Syntax:**
```
[addr1[,addr2]]d
```

**Examples:**
```bash
# Delete line 5
sedx '5d' file.txt

# Delete lines 5-10
sedx '5,10d' file.txt

# Delete lines matching pattern
sedx '/foo/d' file.txt

# Delete lines in pattern range
sedx '/start/,/end/d' file.txt

# Delete lines NOT matching pattern
sedx '/foo/!d' file.txt
```

---

#### Insert (`i`)

**Syntax:**
```
[addr]i\
text
```

**Examples:**
```bash
# Insert before line 5
sedx '5i\new line' file.txt

# Insert before matching pattern
sedx '/foo/i\before foo' file.txt
```

---

#### Append (`a`)

**Syntax:**
```
[addr]a\
text
```

**Examples:**
```bash
# Append after line 5
sedx '5a\new line' file.txt

# Append after matching pattern
sedx '/foo/a\after foo' file.txt
```

---

#### Change (`c`)

**Syntax:**
```
[addr1[,addr2]]c\
text
```

**Examples:**
```bash
# Change line 5
sedx '5c\new content' file.txt

# Change lines 5-10 to single line
sedx '5,10c\replaced range' file.txt

# Change matching lines
sedx '/foo/c\bar' file.txt
```

---

#### Print (`p`)

**Syntax:**
```
[addr1[,addr2]]p
```

**Examples:**
```bash
# Print lines 1-10
sedx -n '1,10p' file.txt

# Print matching lines
sedx -n '/foo/p' file.txt

# Print lines NOT matching
sedx -n '/foo/!p' file.txt
```

---

#### Quit (`q`)

**Syntax:**
```
[addr]q
```

**Examples:**
```bash
# Quit after line 10
sedx '10q' file.txt

# Quit when pattern matches
sedx '/error/q' file.txt
```

---

#### Command Grouping (`{}`)

**Syntax:**
```
[addr1[,addr2]]{
    command1
    command2
    ...
}
```

**Examples:**
```bash
# Multiple commands on lines 5-10
sedx '5,10{s/foo/bar/g; s/baz/qux/g}' file.txt

# Group with pattern range
sedx '/start/,/end/{s/foo/bar/g; d}' file.txt

# Nested grouping
sedx '1,50{/header/{s/^/# /; p; q}}' file.txt
```

---

#### Hold Space Operations

**Copy to hold space (`h`):**
```bash
sedx '5h' file.txt  # Copy line 5 to hold space
sedx '/foo/h' file.txt  # Copy matching lines to hold space
```

**Append to hold space (`H`):**
```bash
sedx '5H' file.txt  # Append line 5 to hold space
sedx '/foo/H' file.txt  # Append matching lines
```

**Get from hold space (`g`):**
```bash
sedx '10g' file.txt  # Replace line 10 with hold space
sedx '/bar/g' file.txt  # Replace matching lines with hold space
```

**Append from hold space (`G`):**
```bash
sedx '10G' file.txt  # Append hold space to line 10
sedx '/bar/G' file.txt  # Append hold space to matching lines
```

**Exchange (`x`):**
```bash
sedx '10x' file.txt  # Swap line 10 with hold space
sedx '/foo/x' file.txt  # Swap matching lines with hold space
```

---

### 4.3 Command Reference (Planned)

#### Next Line Operations (v0.4.0)

**`n` - Next line:**
```bash
seq 1 5 | sedx 'n; d'  # Output: 1, 3, 5
```

**`N` - Next append:**
```bash
printf "a\nb\nc" | sedx 'N; s/\n/ /'  # Output: "a b\nc"
```

**`P` - Print first line:**
```bash
printf "a\nb\nc" | sedx 'N;P;D'  # Process multi-line
```

**`D` - Delete first line:**
```bash
sedx ':top;N;D;/pattern/q;b top' file.txt
```

---

#### Flow Control (v0.5.0)

**Labels and branching:**
```bash
# Loop until pattern matches
sedx ':top; /found/q; n; b top' file.txt

# Repeat substitution
sedx ':loop; s/foo/bar/; t loop' file.txt

# Branch if NO substitution
sedx ':loop; s/foo/bar/; T end; b loop; :end'
```

---

#### File I/O (v0.5.0)

**Read file:**
```bash
sedx '5r header.txt' file.txt
sedx '/error/r error_template.txt' log.txt
```

**Write to file:**
```bash
sedx '/error/w errors.log' log.txt
sedx '5,10w excerpt.txt' file.txt
```

---

## 5. Substitution System

### 5.1 Syntax

SedX uses **traditional sed syntax** for all operations:

```bash
sedx 's/pattern/replacement/[flags]' file.txt
```

**Characteristics:**
- Uses delimiter (typically `/`)
- Explicit flags: `g` for global, `i` for case-insensitive, etc.
- Modern backreferences: `$1`, `$2` (or `\1`, `\2` in BRE mode)
- Compatible with GNU sed syntax

**Examples:**
```bash
# Basic substitution
sedx 's/foo/bar/' file.txt

# Global substitution
sedx 's/foo/bar/g' file.txt

# With address range
sedx '1,10s/foo/bar/g' file.txt

# With pattern address
sedx '/error/s/test/fix/' file.txt
```

---

### 5.2 Regex Flavor Impact on Syntax

The regex flavor affects pattern syntax and backreference style:

**PCRE Mode (Default):**
```bash
sedx 's/(foo|bar)/baz/g' file.txt           # Extended syntax
sedx 's/(?P<name>\w+)/hello_\1/' file.txt  # Named groups
sedx 's/(foo)(bar)/$2$1/' file.txt         # Modern backreferences
```

**BRE Mode (-B flag):**
```bash
sedx -B 's/\(foo\|bar\)/baz/g' file.txt     # Escaped metacharacters
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt  # BRE backreferences (\1, \2)
# Automatically converts to: s/(foo)(bar)/$2$1/
```

**ERE Mode (-E flag):**
```bash
sedx -E 's/(foo|bar)/baz/g' file.txt        # Extended syntax (no PCRE features)
sedx -E 's/(foo)(bar)/\2\1/' file.txt      # Uses $1, $2 internally
```

---

### 5.3 Substitution Flags

**In-Expression Flags:**
```bash
sedx 's/foo/bar/g' file.txt      # Global (all occurrences in line)
sedx 's/foo/bar/i' file.txt      # Case-insensitive
sedx 's/foo/bar/gi' file.txt     # Both global and case-insensitive
sedx 's/foo/bar/2' file.txt      # Replace only 2nd occurrence
sedx 's/foo/bar/p' file.txt      # Print line if substitution made
```

**Available Flags:**
- `g` - Global replacement (all occurrences in line)
- `i` / `I` - Case-insensitive matching
- `p` - Print line if substitution made
- `1`-`9` - Replace only Nth occurrence

---

### 5.4 CLI-Level Flags

| Flag | Description | Example |
|------|-------------|---------|
| `-i` / `--case-insensitive` | Case-insensitive matching (all patterns) | `sedx -i 's/foo/bar/'` |
| `--max-count=N` | Maximum replacements per file | `sedx --max-count=5 's/foo/bar/g'` |
| `-B` | BRE mode (GNU sed compatible) | `sedx -B 's/\(foo\)/bar/'` |
| `-E` | ERE mode (sed -E compatible) | `sedx -E 's/(foo|bar)/baz/'` |

---

### 5.5 Capture Groups

#### Syntax Support

SedX supports modern capture syntax with PCRE and traditional syntax with BRE:

**PCRE Mode (Default):**
```bash
sedx 's/(foo)(bar)/$2$1/' file.txt              # "foobar" → "barfoo"
sedx 's/(?P<word>\w+)/<$1>/' file.txt          # Named capture (future)
sedx 's/(?:non-capturing)/(captured)/' file.txt  # Non-capturing group (future)
```

**BRE Mode (-B flag):**
```bash
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt  # "foobar" → "barfoo"
# Automatically converts \1, \2 to $1, $2
```

**ERE Mode (-E flag):**
```bash
sedx -E 's/(foo)(bar)/$2$1/' file.txt  # Uses $1, $2 internally
```

---

#### Ambiguity Detection

SedX detects ambiguous capture references:

```bash
$ sedx 's/(\d+)/$1user/' file.txt
error: Ambiguous capture reference: $1user
hint: Use ${1}user to disambiguate: s/(\d+)/${1}user/

$ sedx 's/(\d+)/${1}user/' file.txt  # OK
123 → 123user
```

---

### 5.6 Escape Sequences

Supported in replacement strings:

| Escape | Meaning | Example |
|--------|---------|---------|
| `\n` | Newline | `sedx 's/,/\n/g'` |
| `\t` | Tab | `sedx 's/,/\t/g'` |
| `\r` | Carriage return | `sedx 's/\r\n/\n/g'` |
| `\\` | Backslash | `sedx 's/\\/\\\\/g'` |
| `\xHH` | Hex byte (HH = 2 hex digits) | `sedx 's/\x41/B/'` (A→B) |
| `\uHHHH` | Unicode code point | `sedx 's/\u0041/B/'` (A→B) |
| `$&` | Entire match | `sedx 's/foo/$&_bar/'` |
| `$$` | Literal dollar sign | `sedx 's/\$/$$/'` |

---

### 5.7 Replacement Limiting

Limit total replacements per file:

```bash
sedx --max-count=5 's/foo/bar/g' file.txt  # Max 5 replacements per file
sedx --max-replacements=10 's/error/ERROR/g' log.txt
```

**Use Cases:**
- Preview first N changes
- Limit impact of global replacements
- Safety when unsure of match count

---

## 6. Address Types

Addresses specify which lines commands apply to.

### 6.1 Line Numbers

```bash
sedx '5d' file.txt          # Line 5
sedx '5,10d' file.txt       # Lines 5-10
sedx '1,5!d' file.txt       # All except lines 1-5
```

### 6.2 Pattern Addresses

```bash
sedx '/foo/d' file.txt                    # Lines matching "foo"
sedx '/foo/,/bar/d' file.txt             # From "foo" to "bar"
sedx '/foo/!d' file.txt                  # Lines NOT matching "foo"
sedx '/foo/,+5d' file.txt                # "foo" and 5 lines after (future)
```

### 6.3 Special Addresses

```bash
sedx '$d' file.txt          # Last line
sedx '1,$d' file.txt        # All lines
sedx '0d' file.txt          # First line before any processing (future)
```

### 6.4 Stepping Addresses (future)

```bash
sedx '1~2d' file.txt        # Delete odd lines (1, 3, 5, ...)
sedx '2~2d' file.txt        # Delete even lines (2, 4, 6, ...)
sedx '1~3p' file.txt        # Every 3rd line (1, 4, 7, ...)
```

### 6.5 Range Semantics

**Line number ranges:** `start,end` (inclusive)
```bash
sedx '5,10d' file.txt       # Deletes lines 5, 6, 7, 8, 9, 10
```

**Pattern ranges:** `/start/,/end/` (state machine)
```bash
sedx '/start/,/end/d' file.txt
# Deletes from first line matching "start"
# Through first line AFTER that matching "end"
```

**Example of pattern range behavior:**
```bash
$ cat file.txt
line 1
start here
line 3
line 4
end here
line 6
start again
line 8
end again
line 10

$ sedx '/start/,/end/d' file.txt
line 1
line 6
line 10
```

### 6.6 Negation

```bash
sedx '/foo/!d' file.txt              # Delete lines NOT matching "foo"
sedx '5,10!d' file.txt               # Delete all except lines 5-10
sedx '/start/,/end/!s/foo/bar/g'    # Apply except in range
```

---

## 7. Backup System

### 7.1 Overview

SedX automatically creates backups before modifying files.

**Location:** `~/.sedx/backups/<timestamp-id>/`

**Structure:**
```
~/.sedx/backups/20250107-143052-abc123/
├── operation.json          # Metadata
└── files/
    ├── file.txt            # Original file
    ├── config.yaml
    └── ...
```

---

### 7.2 Backup Metadata

**operation.json:**
```json
{
  "id": "20250107-143052-abc123",
  "timestamp": "2025-01-07T14:30:52Z",
  "expression": "s/foo/bar/g",
  "files": [
    {
      "original_path": "/home/user/file.txt",
      "backup_path": "~/.sedx/backups/20250107-143052-abc123/files/file.txt"
    }
  ],
  "size_bytes": 1234567
}
```

---

### 7.3 Disk Space Management

#### Checks Before Backup

1. **Calculate required space:**
   - Sum of file sizes to be backed up

2. **Check available space:**
   - Cross-platform disk space query
   - Consider backup partition location

3. **Apply thresholds:**
   - **Warn if backup > 2GB** (configurable)
   - **Warn if backup > 40% free space** (configurable)
   - **Error if backup > 60% free space** (configurable)
   - **Error if insufficient disk space**

#### Warning Examples

**Large backup:**
```bash
$ sedx --execute 's/foo/bar/' hugefile.bin
warning: This operation will create a large backup (3.7 GB)
prompt: Continue? [y/N] y
→ Backup created successfully
```

**Low disk space:**
```bash
$ sedx --execute 's/foo/bar/' file.txt
error: Insufficient disk space for backup
backup partition: /home (15.2 GB free)
backup required: 10.1 GB (would use 66% of free space)

options:
  1. Remove old backups:
     sedx backup prune --keep=5

  2. Use different backup location:
     sedx --backup-dir /mnt/backups --execute 's/foo/bar/' file.txt

  3. Skip backup (not recommended):
     sedx --no-backup --force 's/foo/bar/' file.txt

  4. Abort
```

---

### 7.4 Backup Management

#### List Backups
```bash
$ sedx backup list
2025-01-07 14:30:52  ID: 20250107-143052
  Expression: s/foo/bar/g
  Files: file.txt (1.2 MB)

2025-01-07 10:15:33  ID: 20250107-101533
  Expression: 5,10d
  Files: data.txt (512 KB)
```

#### Show Backup Details
```bash
$ sedx backup show 20250107-143052
Backup ID: 20250107-143052
Created: 2025-01-07 14:30:52
Expression: s/foo/bar/g
Files:
  - /home/user/file.txt (1.2 MB)
Total Size: 1.2 MB
Location: /home/user/.sedx/backups/20250107-143052
```

#### Restore from Backup
```bash
$ sedx backup restore 20250107-143052
Restoring: /home/user/file.txt
✅ Restore complete
Backup 20250107-143052 removed after restore
```

#### Remove Backup
```bash
$ sedx backup remove 20250107-143052
warning: This will permanently delete backup 20250107-143052
Continue? [y/N] y
✅ Backup removed
```

#### Prune Old Backups
```bash
$ sedx backup prune --keep=10
Keeping 10 most recent backups
Removed 5 old backups, freed 25.3 MB

$ sedx backup prune --keep-days=30
Keeping backups from last 30 days
Removed 8 old backups, freed 42.1 MB
```

---

### 7.5 Disabling Backups

**Not Recommended**, but available:

```bash
sedx --no-backup --force 's/foo/bar/' file.txt
warning: Creating backup is disabled (data loss risk)
Continue? [y/N] y
✅ Changes applied without backup
```

**Use Cases:**
- CI/CD pipelines
- Temporary files
- Multiple operations in series
- Disk space constraints

---

### 7.6 Backup Retention

**Default policy:** Keep last 50 backups

**Automatic cleanup:** Old backups removed after each operation

**Configurable:**
```toml
# ~/.sedx/config.toml
[backup]
max_backups = 50           # Number of backups to keep
keep_days = 30              # Or keep by days
auto_prune = true           # Automatic cleanup
```

---

## 8. Usage Patterns

### 8.1 Basic Substitution

```bash
# Preview (default)
sedx 's/foo/bar/g' file.txt

# Apply with backup
sedx --execute 's/foo/bar/g' file.txt

# Confirm before applying
sedx --interactive 's/foo/bar/g' file.txt
```

---

### 8.2 Multiple Files

```bash
# Apply to multiple files
sedx 's/foo/bar/g' file1.txt file2.txt file3.txt

# Using glob (shell expansion)
sedx 's/foo/bar/g' *.txt

# Interactive for each file
sedx --interactive 's/foo/bar/g' *.conf
```

---

### 8.3 Pipeline Operations

```bash
# Read from stdin, write to stdout (no backup)
cat file.txt | sedx 's/foo/bar/g' > output.txt

# Chain commands
cat file.txt | sedx 's/foo/bar/g' | sedx 's/baz/qux/g'

# With other tools
tail -f log.txt | sedx 's/\[ERROR\]/\x1b[31m[ERROR]\x1b[0m/'

# Process find results
find . -name "*.txt" | sedx 's/\.txt$/.bak/'
```

---

### 8.4 Large File Processing

**Stream processing (v0.2.0+):**
```bash
# Process 100GB file with <100MB RAM
sedx 's/error/ERROR/g' huge.log

# With context
sedx --context=1 's/error/ERROR/g' huge.log

# Limit memory usage
sedx 's/error/ERROR/g' huge.log | gzip > processed.log.gz
```

---

### 8.5 Interactive Editing

```bash
# Preview then confirm
sedx --interactive 's/foo/bar/g' file.txt
# Shows diff
# Apply changes? [y/N]

# Multiple operations with confirmation
sedx -i 's/foo/bar/g' file1.txt file2.txt
# Confirm for each file
```

---

### 8.6 CI/CD Integration

```bash
# In CI/CD pipeline (no backup, automatic)
sedx --no-backup --force 's/dev/prod/g' config.toml

# Using script file
sedx --no-backup -f deploy.sedx config.toml

# Multiple expressions
sedx --no-backup -e 's/dev/prod/' -e 's/localhost/db.server/' app.conf
```

---

### 8.7 Complex Multi-Command Scripts

```bash
# Command grouping
sedx '{
    s/foo/bar/g
    s/baz/qux/g
    5,10d
}' file.txt

# Using semicolons
sedx 's/foo/bar/g; s/baz/qux/g; 5,10d' file.txt

# Script file
cat > script.sed << 'EOF'
s/foo/bar/g
s/baz/qux/g
5,10d
EOF

sedx -f script.sed file.txt
```

---

### 8.8 Pattern-Scope Operations

```bash
# Only in lines matching "error"
sedx '/error/s/test/fix/' log.txt

# Delete all lines between markers
sedx '/<!-- DELETE -->/,/<!-- END -->/d' file.html

# Replace in specific section
sedx '/\[section\]/,\[\/section\]/s/old/new/' config.ini
```

---

### 8.9 Debugging with Dry-Run

```bash
# Show what would change (default)
sedx 's/foo/bar/g' file.txt

# With more context
sedx --context=5 's/foo/bar/g' file.txt

# Only show changes
sedx --no-context 's/foo/bar/g' file.txt

# Suppress colors for piping
sedx --color=never 's/foo/bar/g' file.txt | less
```

---

## 9. Migration Guide (sed → sedx)

### 9.1 Quick Reference

| Sed Command | SedX Equivalent | Notes |
|-------------|-----------------|-------|
| `sed 's/foo/bar/g' file` | `sedx 's/foo/bar/g' file` | Same syntax |
| `sed -i 's/foo/bar/' file` | `sedx --execute 's/foo/bar/' file` | SedX needs explicit `--execute` |
| `sed -n '1,10p' file` | `sedx -n '1,10p' file` | Same |
| `sed -e 's/a/b/' -e 's/c/d/' file` | `sedx -e 's/a/b/' -e 's/c/d/' file` | Same |
| `sed -f script.sed file` | `sedx -f script.sed file` | Same (v0.4.0+) |
| `sed -E 's/(foo|bar)/baz/' file` | `sedx 's/(foo|bar)/baz/' file` | SedX default is ERE |
| `sed 's/\(foo\|bar\)/baz/' file` | `sedx -B 's/\(foo\|bar\)/baz/' file` | Use `-B` for BRE |

---

### 9.2 Common Patterns

#### Replace all occurrences
**Sed:**
```bash
sed 's/foo/bar/g' file.txt
```

**SedX:**
```bash
sedx 's/foo/bar/g' file.txt        # Same syntax
```

---

#### Delete lines
**Sed:**
```bash
sed '/pattern/d' file.txt
```

**SedX:**
```bash
sedx '/pattern/d' file.txt          # Same
```

---

#### Print specific lines
**Sed:**
```bash
sed -n '1,10p' file.txt
```

**SedX:**
```bash
sedx -n '1,10p' file.txt            # Same
```

---

#### In-place editing
**Sed:**
```bash
sed -i 's/foo/bar/g' file.txt
```

**SedX:**
```bash
sedx --execute 's/foo/bar/g' file.txt
# Note: SedX always creates backup by default
# To skip backup: sedx --no-backup --force 's/foo/bar/g' file.txt
```

---

#### Multiple expressions
**Sed:**
```bash
sed -e 's/foo/bar/' -e 's/baz/qux/' file.txt
```

**SedX:**
```bash
sedx -e 's/foo/bar/' -e 's/baz/qux/' file.txt
```

---

#### Script file
**Sed:**
```bash
sed -f script.sed file.txt
```

**SedX:**
```bash
sedx -f script.sed file.txt         # Same (v0.4.0+)
```

---

### 9.3 Incompatibilities

#### Default Regex Behavior

**GNU sed:** Uses BRE by default
**SedX:** Uses PCRE by default

```bash
# GNU sed (BRE)
sed 's/\(foo\|bar\)/baz/' file.txt

# SedX (PCRE - more powerful)
sedx 's/(foo|bar)/baz/' file.txt

# SedX (BRE mode - GNU sed compatible)
sedx -B 's/\(foo\|bar\)/baz/' file.txt
```

**Solution:** Use `-B` flag for BRE compatibility

---

#### Default Behavior

**Sed:** Modifies files by default with `-i`
**SedX:** Dry-run by default, needs `--execute`

```bash
# Sed (modifies immediately)
sed -i 's/foo/bar/' file.txt

# SedX (safe preview first)
sedx 's/foo/bar/' file.txt          # Preview
sedx --execute 's/foo/bar/' file.txt # Apply
```

---

#### Backup Creation

**Sed:** Optional backup with `-i.bak`
**SedX:** Automatic backup (can be disabled)

```bash
# Sed (manual backup)
sed -i.bak 's/foo/bar/' file.txt

# SedX (automatic backup)
sedx --execute 's/foo/bar/' file.txt
# Backup created: ~/.sedx/backups/...

# Disable backup
sedx --no-backup --force 's/foo/bar/' file.txt
```

---

### 9.4 Feature Comparison

| Feature | GNU sed | SedX | Notes |
|---------|---------|------|-------|
| **Basic substitution** | ✅ | ✅ | Same syntax |
| **PCRE regex** | ❌ | ✅ (default) | Modern features |
| **BRE/ERE modes** | ✅ | ✅ | `-B`/`-E` flags |
| **Stream processing** | ✅ | ✅ (v0.2.0) | All support |
| **In-place editing** | `-i` | `--execute` | Different UX |
| **Backups** | Optional | Automatic | SedX default |
| **Dry-run mode** | Manual | Default | SedX default |
| **Diff preview** | ❌ | ✅ | SedX feature |
| **Rollback** | ❌ | ✅ | SedX feature |
| **Disk space checks** | ❌ | ✅ (v0.2.1) | SedX feature |

---

## 10. Configuration

### 10.1 Config File Location

**Path:** `~/.sedx/config.toml`

**Create with:**
```bash
sedx config
```

---

### 10.2 Config Structure

```toml
# Backup settings
[backup]
enabled = true                              # Enable/disable backups
location = "~/.sedx/backups"                 # Backup directory
max_backups = 50                             # Number to keep
max_size_gb = 10                             # Warn if backup > this size
max_disk_usage_percent = 60                  # Error if backup uses > this %
compression = false                          # Compress backups (future)
auto_prune = true                            # Auto-cleanup old backups

# Regex settings
[regex]
default_flavor = "pcre"                     # pcre | ere | bre
show_warnings = true                         # Show regex compatibility warnings

# Processing settings
[processing]
context_lines = 2                            # Default context for diffs
max_memory_mb = 100                          # Memory limit for streaming
parallel_files = true                        # Process multiple files in parallel
buffer_size_mb = 10                          # Read buffer size

# Display settings
[display]
color = "auto"                               # always | never | auto
line_numbers = false                         # Show line numbers in diffs
pager = "less"                               # Pager for long output

# Advanced settings
[advanced]
locale = "en_US.UTF-8"                       # Locale for multibyte
timeout_sec = 300                            # Operation timeout
```

---

### 10.3 Config Command

**Edit config:**
```bash
$ sedx config
# Opens editor with config
# Validates on save
# Shows confirmation
```

**Validate without editing:**
```bash
$ sedx --validate-config
Config syntax is valid
```

**Show current config:**
```bash
$ sedx --show-config
[backup]
enabled = true
location = "~/.sedx/backups"
...
```

---

### 10.4 Environment Variables

SedX respects these environment variables:

| Variable | Purpose | Example |
|----------|---------|---------|
| `SEDX_REGEX_FLAVOR` | Default regex mode | `export SEDX_REGEX_FLAVOR=pcre` |
| `SEDX_BACKUP_DIR` | Backup location | `export SEDX_BACKUP_DIR=/mnt/backups` |
| `SEDX_CONFIG` | Custom config file | `export SEDX_CONFIG=~/.sedx.toml` |
| `NO_COLOR` | Disable colors | `export NO_COLOR=1` |
| `EDITOR` | Editor for config command | `export EDITOR=vim` |
| `VISUAL` | Fallback editor | `export VISUAL=nano` |

---

### 10.5 Per-Project Config

SedX looks for config in this order:

1. `.sedx.toml` (current directory)
2. `~/.sedx/config.toml` (user config)
3. Defaults

**Example project config:**
```toml
# .sedx.toml (project root)
[backup]
enabled = false              # Disable backups in CI/CD

[regex]
default_flavor = "ere"       # Use ERE for this project

[processing]
max_memory_mb = 50           # Lower memory limit
```

---

## Appendix A: Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error |
| `2` | Invalid command-line arguments |
| `3` | File not found |
| `4` | Parse error (invalid sed expression) |
| `5` | Disk space error |
| `6` | Backup creation failed |
| `7` | Operation timeout |
| `8` | Interrupted by user |

---

## Appendix B: Files

| File | Purpose |
|------|---------|
| `~/.sedx/config.toml` | User configuration |
| `~/.sedx/backups/*/` | Backup storage |
| `.sedx.toml` | Project-specific config |
| `~/.sedx/history` | Operation log (future) |

---

## Appendix C: Version Compatibility

| SedX Version | Sed Compatibility | Status |
|--------------|------------------|--------|
| v0.1.0 | 33% | Past (Alpha) |
| v0.2.0-alpha | 33% | Current (Stream processing in progress) |
| v0.2.0 | 33% | Stream processing |
| v0.2.1 | 33% | Backup management |
| v0.3.0 | 35% | Enhanced substitution |
| v0.4.0 | 70% | Essential compatibility |
| v0.5.0 | 90% | Flow control |
| v0.6.0 | 95% | Advanced features |
| v1.0.0 | 95% | Production release |

---

## Appendix D: Performance Benchmarks

Target performance (v1.0.0):

| Operation | Target | Notes |
|-----------|--------|-------|
| **Simple substitution** | Within 1.5x of GNU sed | For simple cases |
| **Complex scripts** | Within 2x of GNU sed | For complex patterns |
| **Memory usage** | <100MB for 100GB file | Constant regardless of file size |
| **Startup time** | <50ms | CLI overhead |
| **Backup creation** | Native copy speed | `cp` performance |

---

## Appendix E: Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| **Linux** | ✅ Fully supported | Primary platform |
| **macOS** | ✅ Fully supported | Tested |
| **Windows** | 📋 Planned | WSL supported |
| **BSD** | 📋 Planned | Should work |

---

## Document Changelog

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0-draft-update | 2025-01-08 | **Major architecture update**: Removed simplified syntax (sd-like), added PCRE/ERE/BRE regex flavor system, updated all references |
| 1.0.0-draft | 2025-01-07 | Initial specification |

---

**End of Specification**
