# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**SedX** is a safe, modern replacement for GNU `sed` written in Rust. It maintains ~90% compatibility with standard sed while adding safety features including automatic backups, preview mode, human-readable diffs, and rollback functionality.

**Key difference from GNU sed**: SedX uses **Extended Regular Expressions (ERE)** by default (like `sed -E`), not Basic Regular Expressions (BRE). This means parentheses, braces, and other metacharacters don't need to be escaped with backslashes.

## Development Commands

### Building

```bash
# Debug build (faster compilation)
cargo build

# Release build (optimized binary)
cargo build --release

# The release binary will be at: ./target/release/sedx
```

### Testing

```bash
# Run Rust unit tests
cargo test

# Run unit tests with output
cargo test -- --nocapture

# Run integration/regression tests (compares with GNU sed)
./tests/regression_tests.sh

# Run comprehensive test suite
./tests/comprehensive_tests.sh

# Run hold space tests
./tests/hold_space_tests.sh

# Test specific expression patterns
./target/release/sedx 's/foo/bar/g' test_file.txt
```

### Code Quality

```bash
# Format code
cargo fmt

# Lint with clippy
cargo clippy -- -D warnings
```

### Testing Binaries

The release binary is required for accurate testing:
```bash
./target/release/sedx --version
./target/release/sedx --help
```

## Architecture

### Core Modules

- **main.rs** - Entry point, command routing (execute/rollback/history/status)
- **cli.rs** - Command-line argument parsing using clap derive macros
- **sed_parser.rs** - Parses sed expressions into `SedCommand` enum variants
- **file_processor.rs** - Applies sed commands to file contents, generates diffs
- **diff_formatter.rs** - Formats output (diffs, history, dry-run headers)
- **backup_manager.rs** - Creates/restores backups using JSON metadata

### Data Flow

1. **CLI parsing** (`cli.rs`) → Arguments parsed into `Args` enum
2. **Expression parsing** (`sed_parser.rs`) → Raw sed string becomes `Vec<SedCommand>`
3. **File processing** (`file_processor.rs`) → Commands applied to lines, producing `FileDiff`
4. **Diff formatting** (`diff_formatter.rs`) → Human-readable output with context
5. **Backup creation** (`backup_manager.rs`) → Original files saved to `~/.sedx/backups/<id>/`

### Key Types

**SedCommand** (`sed_parser.rs`):
- `Substitution { pattern, replacement, flags, range }`
- `Delete { range }`
- `Print { range }`
- `Quit { address }`
- `Insert { text, address }`
- `Append { text, address }`
- `Change { text, address }`
- `Group { range, commands }`
- Hold space ops: `Hold`, `HoldAppend`, `Get`, `GetAppend`, `Exchange`

**Address** (`sed_parser.rs`):
- `LineNumber(usize)` - specific line
- `Pattern(String)` - regex pattern to match
- `FirstLine` - special address "0"
- `LastLine` - special address "$"
- `Negated(Box<Address>)` - negation with "!"

**FileDiff** (`file_processor.rs`):
- Contains `changes: Vec<LineChange>` and `all_lines` for context
- `LineChange` has `change_type`: `Unchanged`, `Modified`, `Added`, `Deleted`

### Pattern Matching Semantics

**Pattern ranges** (`/start/,/end/`) use a state machine:
- When start pattern matches, begin including lines in the range
- Continue including lines until end pattern matches
- This differs from simple line number ranges

**Pattern substitution** (`/pattern/s/old/new/`):
- Applies to **ALL** lines matching the pattern (GNU sed compatible)
- Not just the first matching line

**Negation** (`!` suffix):
- `/pattern/!d` deletes all lines NOT matching the pattern
- Resolves to the first non-matching line

### Backreference Conversion

GNU sed uses `\1`, `\2` in replacements, but Rust's regex crate uses `$1`, `$2`.
The `convert_sed_backreferences()` function in `sed_parser.rs` handles this conversion.
Supports: `\1` → `$1`, `\&` → `$&`, `\\` → `\`

### Command Grouping

Commands inside `{ ... }` are split by semicolons and parsed recursively:
- `{s/foo/bar/; s/baz/qux/}` → `Group { commands: [Substitution, Substitution] }`
- Can have optional range: `1,10{s/foo/bar/; d}`

### Address Resolution

The `resolve_address()` method in `file_processor.rs` converts addresses to line indices:
- Line numbers are 1-indexed in input, converted to 0-indexed internally
- Patterns return the first matching line index
- Patterns not found return the `default` parameter
- `$` resolves to `lines.len() - 1`

### Hold Space Implementation

**Hold space commands** work with a secondary buffer stored in `FileProcessor.hold_space`:
- `h` (Hold) - Copy pattern space to hold space (overwrites)
- `H` (HoldAppend) - Append pattern space to hold space with newline
- `g` (Get) - Copy hold space to pattern space (overwrites)
- `G` (GetAppend) - Append hold space to pattern space with newline
- `x` (Exchange) - Swap pattern space and hold space contents

**Current limitations:**
- When `g` is used with a single-line address (e.g., `5g`), only the first line of multiline hold space is used
- Full multiline replacement is supported only when `g` has no range (replaces entire file)

## File Processing Pipeline

**Preview mode** (`--dry-run`):
1. Parse expression
2. Process file to generate diff
3. Display formatted output
4. Exit without modifying files

**Execute mode** (default):
1. Parse expression
2. Create backup with metadata (expression, timestamp, files)
3. Apply changes to files
4. Display what was changed
5. Show backup ID for rollback

**Interactive mode** (`--interactive`):
1. Preview changes
2. Prompt user for confirmation
3. Create backup and apply if confirmed

## Backup System

Backups stored at `~/.sedx/backups/<timestamp-id>/`:
- `operation.json` - metadata (expression, timestamp, files list)
- `files/` - directory with original file contents

Last 50 backups are kept automatically. Old backups cleaned up when creating new ones.

## Testing Strategy

**Unit tests** (`cargo test`):
- Parser tests in `sed_parser.rs` modules
- Test command parsing, address resolution, backreference conversion

**Integration tests** (`./tests/*.sh`):
- Bash scripts comparing SedX output with GNU sed
- Test against real sed to ensure compatibility
- Cover: substitutions, deletes, ranges, patterns, negation, grouping, hold space

**Manual testing workflow**:
```bash
# Create test file
echo -e "foo\nbar\nbaz" > /tmp/test.txt

# Test with sedx
./target/release/sedx 's/foo/bar/' /tmp/test.txt

# Compare with GNU sed
sed 's/foo/bar/' /tmp/test.txt

# Rollback if needed
sedx rollback <backup-id>
```

## Important Constraints

- Uses **Extended Regular Expressions (ERE)**, not BRE like GNU sed
- Pattern substitution applies to ALL matching lines (not just first)
- Backreferences in replacements use `$1`, `$2` (converted from `\1`, `\2`)
- State machine semantics for pattern ranges (start, then end)
- File paths are absolute - shell globs expanded before reaching SedX
- Always processes entire file into memory (line-by-line vector)

## Common Patterns

**Adding new sed command support**:
1. Add variant to `SedCommand` enum in `sed_parser.rs`
2. Add parsing function `parse_*()`
3. Add application method `apply_*()` in `file_processor.rs`
4. Update `apply_command()` match statement
5. Add unit tests for parser
6. Add integration tests in bash scripts

**Debugging parsing issues**:
- Add parser tests to `sed_parser.rs` `#[cfg(test)]` module
- Use `parse_single_command()` to test individual commands
- Check delimiter detection in substitution parsing

**Debugging application issues**:
- Test address resolution with `resolve_address()`
- Check pattern matching with regex directly
- Verify range calculation (start_idx, end_idx)
- Test against GNU sed output
