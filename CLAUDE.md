# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**SedX** is a safe, modern replacement for GNU `sed` written in Rust. It maintains ~90% compatibility with standard sed while adding safety features including automatic backups, preview mode, human-readable diffs, and rollback functionality.

**Key difference from GNU sed**: SedX uses **PCRE (Perl-Compatible Regular Expressions)** by default, which is the most modern and powerful regex flavor. For compatibility, SedX also supports ERE (with `-E`) and BRE (with `-B`) modes.

## Regex Flavor System

SedX supports three regex flavors, selectable via command-line flags:

### PCRE (Default) - Modern Perl-Compatible Regex
```bash
sedx 's/(foo|bar)/baz/g' file.txt    # PCRE syntax (default)
```
- Most powerful and modern regex flavor
- Familiar to developers from Perl, Python, JavaScript, etc.
- No need to escape meta-characters: `(`, `)`, `{`, `}`, `+`, `?`, `|`, `.`

### ERE (Extended Regex) - Compatible with `sed -E`
```bash
sedx -E 's/(foo|bar)/baz/g' file.txt   # ERE mode
```
- Traditional extended regular expressions
- Same syntax as PCRE for most common operations
- Compatible with BSD sed and GNU sed `-E` flag

### BRE (Basic Regex) - Compatible with GNU sed
```bash
sedx -B 's/\(foo\|bar\)/baz/g' file.txt # BRE mode
```
- GNU sed's default regex flavor
- Requires escaping meta-characters: `\(`, `\)`, `\{`, `\}`, `\+`, `\?`, `\|`
- Maximum compatibility with legacy sed scripts

**Design Decision**: SedX defaults to PCRE (not ERE or BRE) because:
1. Modern developers expect PCRE syntax
2. No ambiguity - users explicitly choose regex flavor
3. `-E` flag provides easy migration path from BSD/GNU sed
4. `-B` flag provides maximum compatibility for legacy scripts

## Command Syntax

**Important**: SedX uses **only sed-like syntax**. There is no "sd-style" simple find/replace syntax.

All commands must use sed syntax:
- ✅ `sedx 's/foo/bar/g' file.txt`
- ✅ `sedx '1,10d' file.txt`
- ✅ `sedx '/pattern/p' file.txt`
- ❌ ~~`sedx 'foo bar' file.txt`~~ (not supported - use `s/foo/bar/` instead)

This design eliminates ambiguity and keeps the tool focused on sed compatibility.

## Stdin/Stdout Mode (Pipeline Support)

When no files are specified, SedX reads from **stdin** and writes to **stdout**, making it fully compatible with Unix pipelines:

```bash
# Read from stdin, write to stdout
echo "hello world" | sedx 's/hello/HELLO/'
# Output: HELLO world

# Chain with other commands
cat file.txt | sedx 's/foo/bar/g' | grep bar

# Use in pipelines
ps aux | sedx '1d' | grep nginx

# Process multiple files via stdin
find . -name "*.log" | xargs cat | sedx 's/error/ERROR/g'
```

**Stdin mode characteristics:**
- ✅ No backups created (can't backup a stream)
- ✅ No diff output (only transformed text)
- ✅ Works with all regex flavors (PCRE, ERE, BRE)
- ✅ Compatible with Unix pipes and redirections
- ✅ Exit status: 0 on success, non-zero on errors

**Examples:**
```bash
# Global substitution in pipeline
echo -e "foo\nbar\nfoo" | sedx 's/foo/FOO/g'
# Output: FOO
#         bar
#         FOO

# Delete lines with pattern
docker logs nginx 2>/dev/null | sedx '/error/d'

# Multiple commands in pipeline
echo "test case" | sedx '{s/test/TEST/; s/case/CASE/}'
# Output: TEST CASE

# Case-insensitive matching
echo "HELLO world" | sedx 's/hello/WORLD/gi'
# Output: WORLD world

# BRE mode in pipeline
echo "foo+" | sedx -B 's/foo\+/FOO/'
# Output: FOO+
```

**Comparison with file mode:**
```bash
# File mode: Creates backups, shows diffs
sedx 's/foo/bar/g' file.txt

# Stdin mode: No backups, no diffs
cat file.txt | sedx 's/foo/bar/g'
```

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
- **cli.rs** - Command-line argument parsing, defines `Args` and `RegexFlavor` enums
- **command.rs** - Unified `Command` and `Address` enums (NEW)
- **parser.rs** - Unified parser with regex flavor support (NEW)
- **bre_converter.rs** - Converts BRE patterns to PCRE for compilation (NEW)
- **ere_converter.rs** - Converts ERE backreferences to PCRE format (NEW)
- **capability.rs** - Streaming capability checks (NEW)
- **sed_parser.rs** - Legacy sed parser (DEPRECATED - being migrated to parser.rs)
- **file_processor.rs** - Applies sed commands to file contents, generates diffs
- **diff_formatter.rs** - Formats output (diffs, history, dry-run headers)
- **backup_manager.rs** - Creates/restores backups using JSON metadata

### Data Flow

1. **CLI parsing** (`cli.rs`) → Arguments parsed into `Args` enum with `RegexFlavor`
2. **Expression parsing** (`parser.rs`) → Raw sed string becomes `Vec<Command>`
3. **Regex conversion**:
   - BRE patterns → `bre_converter.rs` → Converted to PCRE
   - ERE patterns → `ere_converter.rs` → Backreferences converted to PCRE format
   - PCRE patterns → Pass through unchanged
4. **File processing** (`file_processor.rs`) → All patterns compiled as PCRE, commands applied to lines, producing `FileDiff`
5. **Diff formatting** (`diff_formatter.rs`) → Human-readable output

### Regex Compilation Pipeline

All regex patterns are compiled as PCRE (Perl-Compatible Regular Expressions) for execution:

```
User Input → [Flavor Detection] → [Converter] → PCRE Pattern → [Rust regex] → Compiled Regex
    │              │                    │
    │              │                    ├─ BRE: \(foo\) → (foo)
    │              │                    ├─ BRE: \1 → $1 (in replacement)
    │              │                    ├─ ERE: (foo) → (foo) [pass-through]
    │              │                    └─ ERE: \1 → $1 (in replacement)
    │              │
    │              └─ PCRE/ERE/BRE flag
    └─ sed expression
```

**Why PCRE for everything?**
- Rust's `regex` crate is PCRE-compatible
- Single regex engine simplifies the codebase
- BRE/ERE modes are for user convenience, converted transparently
- PCRE is a superset of ERE features, so conversion is lossless

### Unified Command System

SedX uses a unified `Command` enum that represents all sed operations:

```rust
pub enum Command {
    Substitution { pattern, replacement, flags, range },
    Delete { range },
    Print { range },
    Quit { address },
    Insert { text, address },
    Append { text, address },
    Change { text, address },
    Group { commands, range },
    Hold { range }, HoldAppend { range },
    Get { range }, GetAppend { range },
    Exchange { range },
}
```

**Address Types**:
- `LineNumber(usize)` - Specific line
- `Pattern(String)` - Regex pattern match
- `FirstLine` - Special address "0"
- `LastLine` - Special address "$"
- `Negated(Box<Address>)` - Negation with "!"
- `Relative { base, offset }` - Relative offset (e.g., `/pattern/,+5`)
- `Step { start, step }` - Stepping (e.g., `1~2` for every 2nd line)

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

All regex flavors are converted to PCRE format before compilation:

**User Input → Internal PCRE Format:**
- **BRE** (`-B` flag): `\1`, `\2`, `\&` → `$1`, `$2`, `$&`
- **ERE** (`-E` flag): `\1`, `\2`, `\&` → `$1`, `$2`, `$&`
- **PCRE** (default): `$1`, `$2`, `$&` → No conversion needed

**Converter modules:**
- `bre_converter::convert_bre_to_pcre()` - Converts BRE patterns and backreferences
- `bre_converter::convert_sed_backreferences()` - Converts BRE backreferences in replacements
- `ere_converter::convert_ere_to_pcre_pattern()` - Pass-through (ERE patterns are PCRE-compatible)
- `ere_converter::convert_ere_backreferences()` - Converts ERE backreferences in replacements

**Example conversions:**
```bash
# BRE mode
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt
# Internally: s/(foo)(bar)/$2$1/ (PCRE)

# ERE mode
sedx -E 's/(foo)(bar)/\2\1/' file.txt
# Internally: s/(foo)(bar)/$2$1/ (PCRE)

# PCRE mode (default)
sedx 's/(foo)(bar)/$2$1/' file.txt
# Already PCRE format
```

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
