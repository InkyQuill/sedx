# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**SedX** is a safe, modern replacement for GNU `sed` written in Rust. It maintains ~90% compatibility with standard sed while adding safety features including automatic backups, preview mode, human-readable diffs, and rollback functionality.

**Key difference from GNU sed**: SedX uses **PCRE (Perl-Compatible Regular Expressions)** by default, which is the most modern and powerful regex flavor. For compatibility, SedX also supports ERE (with `-E`) and BRE (with `-B`) modes.

**Streaming Architecture**: SedX implements constant-memory streaming processing for large files (100GB+ with <100MB RAM). See "Streaming Implementation" section below for details.

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

# Run all tests (integration + comprehensive + phase-specific)
./tests/run_all_tests.sh

# Run quick tests only
./tests/run_quick_tests.sh

# Run specific test suites
./tests/regression_tests.sh      # GNU sed compatibility
./tests/comprehensive_tests.sh   # Extended test suite
./tests/streaming_tests.sh       # Large file streaming
./tests/phase4_tests.sh          # Phase 4 features
./tests/scripts/phase5_tests.sh  # Phase 5 flow control & file I/O
./tests/hold_space_tests.sh      # Hold space operations

# Memory profiling for streaming
./tests/memory_profile.sh

# Benchmark against GNU sed
./tests/benchmark.sh

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

- **main.rs** - Entry point, command routing (execute/rollback/history/status/config/backup subcommands)
- **cli.rs** - Command-line argument parsing, defines `Args` and `RegexFlavor` enums
- **command.rs** - Unified `Command` and `Address` enums (core data structures)
- **parser.rs** - Unified parser with regex flavor support (PCRE/ERE/BRE)
- **sed_parser.rs** - Legacy sed parser (mostly migrated to parser.rs, but still used)
- **bre_converter.rs** - Converts BRE patterns to PCRE for compilation
- **ere_converter.rs** - Converts ERE backreferences to PCRE format
- **capability.rs** - Streaming capability checks (determines if commands can stream)
- **file_processor.rs** - Dual-mode processor: in-memory (`FileProcessor`) and streaming (`StreamProcessor`)
- **diff_formatter.rs** - Formats output (diffs, history, dry-run headers)
- **backup_manager.rs** - Creates/restores backups using JSON metadata
- **config.rs** - Configuration file management (~/.sedx/config.toml)
- **disk_space.rs** - Disk space checking and human-readable size formatting

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
    Quit { address }, QuitWithoutPrint { address },
    Insert { text, address },
    Append { text, address },
    Change { text, address },
    Group { commands, range },
    Hold { range }, HoldAppend { range },
    Get { range }, GetAppend { range },
    Exchange { range },
    Next { range }, NextAppend { range },
    PrintFirstLine { range }, DeleteFirstLine { range },
    // Phase 5: Flow control
    Label { name },
    Branch { label, range },
    Test { label, range }, TestFalse { label, range },
    // Phase 5: File I/O
    ReadFile { filename, range }, WriteFile { filename, range },
    ReadLine { filename, range }, WriteFirstLine { filename, range },
    // Phase 5: Additional commands
    PrintLineNumber { range }, PrintFilename { range },
    ClearPatternSpace { range },
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

### Cycle-Based Processing

SedX uses a **hybrid architecture** combining batch processing (for simple commands) with cycle-based processing (for flow control):

**Batch mode** (FileProcessor.apply_batch_based()):
- All commands applied to all lines at once
- Used for simple substitutions, deletes, etc.
- Faster for non-branching operations

**Cycle mode** (FileProcessor.apply_cycle_based()):
- Each line processed through command cycle (like GNU sed)
- Required for flow control (b, t, T commands)
- Tracks program counter and substitution flags per line
- Enables labels and conditional branching

**CycleState** struct tracks:
- `pattern_space` - Current line being processed
- `hold_space` - Secondary buffer
- `substitution_made` - Flag for t/T commands
- `program_counter` - Current command index (for branching)
- `stdout_outputs` - Side-effect outputs (=, F commands)
- `current_filename` - For F command
- `write_handles` - File handles for w/W commands
- `read_positions` - File positions for r/R commands

## File Processing Pipeline

### Processing Modes

SedX automatically chooses between in-memory and streaming processing based on file size:

**In-Memory Mode** (files < 100MB):
- Loads entire file into `Vec<String>`
- Processes with random access to all lines
- Generates full diff with context
- Fast for small files

**Streaming Mode** (files ≥ 100MB):
- Processes line-by-line using `BufReader`
- Constant memory usage regardless of file size
- Sliding window for diff context
- Falls back to in-memory for unsupported commands

**Capability Checking** (`capability.rs`):
- `can_stream(commands)` - Checks if commands support streaming
- Returns `false` for: hold space ops, complex groups, negated ranges
- Forces in-memory processing when streaming not possible

**Streaming State Machine** (Chunk 8 - In Progress):
- Pattern ranges `/start/,/end/` use `PatternRangeState` enum
- States: `LookingForStart`, `InRange`, `WaitingForLineNumber`, `CountingRelativeLines`
- Tracked per-command using `HashMap<(String, String), PatternRangeState>`
- Supports mixed ranges: `/start/,10`, `5,/end/`, `/start/,+5`

### Preview vs Execute vs Interactive

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

### Streaming Implementation Details

**StreamProcessor struct** (`file_processor.rs`):
```rust
pub struct StreamProcessor {
    commands: Vec<Command>,
    hold_space: String,
    current_line: usize,
    context_buffer: VecDeque<(usize, String, ChangeType)>,  // Sliding window
    context_size: usize,                                    // Default: 2
    context_lines_to_read: usize,                           // Context after changes
    pattern_range_states: HashMap<(String, String), PatternRangeState>,
    mixed_range_states: HashMap<MixedRangeKey, MixedRangeState>,
    dry_run: bool,
}
```

**Sliding Window Diff** (Chunk 7):
- Unchanged lines accumulate in `context_buffer` (VecDeque)
- When change detected: flush buffer + add changed line + read next `context_size` lines
- Prevents memory blowup on large files while showing context

**Atomic File Writes**:
```rust
let temp_file = NamedTempFile::new_in(parent_dir)?;
// Write to temp file
temp_file.persist(file_path)?;  // Atomic rename
```

**Pattern Range Logic** (Chunk 8):
- `/start/,/end/` → State machine toggles between `LookingForStart` and `InRange`
- `/start/,10` → MixedRangeState: `InRangeUntilLine { target_line: 10 }`
- `5,/end/` → MixedRangeState: `InRangeUntilPattern { end_pattern }`
- `/start/,+5` → Counting state for N lines after pattern match

### Chunk-Based Implementation Approach

The streaming feature is implemented in small, testable chunks:

**Completed Chunks**:
- Chunk 1: Basic streaming infrastructure (BufReader/BufWriter, temp files)
- Chunk 2: Substitution command (s) with flags (g, i, numbered)
- Chunk 3: Delete (d) and Print (p) commands
- Chunk 4-5: Insert (i), Append (a), Change (c), Quit (q) commands
- Chunk 6: Simple diff generation (changed lines only, no full file storage)
- Chunk 7: Sliding window diff with context (VecDeque buffer)
- Chunk 8: Pattern ranges (/start/,/end/) with state machine (IN PROGRESS)

**Completed Chunks**:
- Chunk 1: Basic streaming infrastructure (BufReader/BufWriter, temp files)
- Chunk 2: Substitution command (s) with flags (g, i, numbered)
- Chunk 3: Delete (d) and Print (p) commands
- Chunk 4-5: Insert (i), Append (a), Change (c), Quit (q) commands
- Chunk 6: Simple diff generation (changed lines only, no full file storage)
- Chunk 7: Sliding window diff with context (VecDeque buffer)
- Chunk 8: Pattern ranges (/start/,/end/) with state machine
- Chunk 9: Hold space operations (h, H, g, G, x)
- Chunk 10: Command grouping with ranges ({...})
- Chunk 11: Flow control (b, t, T), File I/O (r, w, R, W), additional commands (=, F, z)

**Remaining Work**:
- Memory profiling and optimization for very large files (100GB+)

**Each chunk follows this pattern**:
1. Add state/data structures to StreamProcessor
2. Implement command handling in streaming loop
3. Add state machine logic for ranges (if needed)
4. Write unit tests using `process_streaming_forced()`
5. Test with actual large file (≥100MB)
6. Commit to neo branch

**Testing incrementally**:
```bash
# After each chunk, run tests
cargo test
./tests/regression_tests.sh

# Force streaming on small files for testing
./target/release/sedx 's/foo/bar/g' /tmp/small_test.txt
```

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
- Cover: substitutions, deletes, ranges, patterns, negation, grouping, hold space, flow control, file I/O

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

- **Regex**: Uses PCRE by default (not ERE/BRE like GNU sed)
- **Pattern substitution**: Applies to ALL matching lines (not just first)
- **Backreferences**: Use `$1`, `$2` internally (converted from `\1`, `\2`)
- **Pattern ranges**: State machine semantics (start pattern → in range → end pattern)
- **File processing**: Auto-detects streaming vs in-memory based on file size (100MB threshold)
- **Streaming limitations**: Negated ranges force in-memory mode; most other commands support streaming

## Configuration File

SedX supports a configuration file at `~/.sedx/config.toml`:

```toml
[backup]
max_size_gb = 10           # Max backup size limit
max_disk_usage_percent = 80 # Max disk usage before warning
backup_dir = "/custom/path" # Custom backup directory

[compatibility]
mode = "pcre"              # Default regex flavor: pcre, ere, or bre
show_warnings = true        # Show compatibility warnings

[processing]
context_lines = 2           # Default diff context
max_memory_mb = 100         # Streaming threshold
streaming = true            # Enable streaming mode
```

Use `sedx config` to edit, `sedx config --show` to view current settings.

## Common Patterns

### Flow Control Commands (Phase 5)

SedX supports GNU sed-style flow control with labels and branches:

```bash
# Labels and unconditional branch
sedx ':top; s/foo/bar/; /condition/b top' file.txt

# Conditional branch (if substitution made)
sedx 's/foo/bar/; t success; s/baz/qux/; :success' file.txt

# Inverse branch (if NO substitution made)
sedx 's/foo/bar/; T retry; b done; :retry; s/baz/qux/; :done' file.txt
```

**Flow control with addresses:**
```bash
# Branch only on specific lines
sedx '1,10{s/foo/bar/; t skip}; s/baz/qux/; :skip' file.txt

# Branch from pattern address
/error/b skip_error
s/fallback/fix/
:skip_error
s/error/ERROR/
```

### File I/O Commands (Phase 5)

Read and write files during processing:

```bash
# Read file contents (r)
sedx '1r header.txt' file.txt

# Write pattern space to file (w)
sedx 'w output.txt' file.txt

# Read one line from file (R)
sedx 'R data.txt' file.txt

# Write first line to file (W)
sedx 'W errors.log' logfile.txt
```

**File handles are managed automatically**:
- Write files are opened in append mode
- Multiple write commands to same file share handle
- Read files track position for R command

### Additional Commands (Phase 5)

```bash
# Print line number (=)
sedx '=' file.txt  # Prints line numbers before each line

# Print filename (F) - GNU sed extension
sedx 'F' file.txt  # Prints current filename

# Clear pattern space (z) - GNU sed extension
sedx '/unwanted/{z; s/EMPTY/now empty/}' file.txt
```

### Adding New Sed Commands

**Simple Commands** (no side effects, no flow control):
1. Add variant to `Command` enum in `command.rs`
2. Add parsing in `sed_parser.rs` or `parser.rs`
3. Add handler in `apply_command_to_cycle()` for cycle-based mode
4. Add handler in streaming mode if needed (check `capability.rs`)
5. Update `commands_can_modify_files()` in `main.rs` if command modifies files
6. Add tests in `tests/scripts/` or appropriate test suite

**Flow Control Commands** (affects program counter):
1. Add variant to `Command` enum with `range: Option<(Address, Address)>`
2. Add parsing logic to detect label definitions (`:label`)
3. Build label registry during parsing (HashMap<String, usize>)
4. Modify program_counter in CycleState when command executes
5. Update substitution flag tracking for t/T commands
6. Test with cycle-based mode (`apply_cycle_based()`)

**File I/O Commands** (r, R, w, W):
1. Add variant to `Command` enum with filename and optional address
2. Add parsing with pattern address detection (`is_inside_pattern_address()`)
3. Add `write_handles: HashMap<String, BufWriter<File>>` to processor struct
4. Add `read_positions: HashMap<String, usize>` for R command position tracking
5. Change `apply_command_to_cycle(&self, ...)` to `apply_command_to_cycle(&mut self, ...)`
6. Flush write handles at end of processing

**Side-Effect Commands** (=, F, output to stdout):
1. Add to `stdout_outputs: Vec<String>` in CycleState
2. Ensure output appears in correct order (usually before pattern space)
3. Track `current_filename` for F command

**Legacy Batch Commands** (if not using cycle-based):
1. Add `apply_*()` method in `FileProcessor`
2. Update `apply_command()` match statement
3. Add unit tests in `sed_parser.rs` `#[cfg(test)]` module
4. Add integration tests in bash scripts under `tests/`

**Streaming Implementation** (for large file support):
1. Update `capability.rs::can_stream()` to check if command supports streaming
2. Add command handling in `StreamProcessor::process_streaming_internal()` loop
3. For pattern ranges, add state tracking in `pattern_range_states` HashMap
4. Update `should_apply_command_with_range()` if command uses ranges
5. Add unit test using `process_streaming_forced()` for small files
6. Test with actual large file (≥100MB) to verify constant memory usage

### Debugging Streaming Issues

**Capability checking fails** (falls back to in-memory):
- Check `capability.rs::can_stream()` - is your command marked as streamable?
- Verify the command isn't using unsupported features (hold space, complex groups)
- Add debug prints: `println!("can_stream: {}", capability::can_stream(&commands));`

**Pattern ranges not working**:
- Check state initialization in `pattern_range_states` HashMap
- Verify `check_pattern_range()` or `check_mixed_*()` methods are being called
- Ensure command loop clones commands before iterating: `let commands = self.commands.clone()`
- Use `process_streaming_forced()` to test streaming on small files

**Diff context issues**:
- Adjust `context_size` (default: 2 lines before/after changes)
- Check `flush_buffer_to_changes()` is called before adding changed lines
- Verify `context_lines_to_read` counter decrements correctly
- Remember: Chunk 6 streaming mode shows only changed lines (no full context)

**Memory profiling**:
```bash
# Test memory usage with large file
/usr/bin/time -v ./target/release/sedx 's/foo/bar/g' large_file.txt

# Expected: Peak RSS < 100MB for 100GB file
```

### Adding Address Types

**New address** (e.g., stepping `1~2`):
1. Add variant to `Address` enum in `command.rs`
2. Update parser to recognize new syntax
3. Add `resolve_address()` case in `FileProcessor`
4. For streaming: Add `should_apply_command_with_range()` handling
5. Test with both in-memory and streaming modes

### Testing Streaming Functionality

**Force streaming mode for testing**:
```rust
let mut processor = StreamProcessor::new(commands);
let result = processor.process_streaming_forced(Path::new(test_file_path));
```

**Verify memory efficiency**:
```bash
# Generate 1GB test file
dd if=/dev/zero of=/tmp/test_1gb.dat bs=1M count=1024

# Process with memory monitoring
/usr/bin/time -v ./target/release/sedx 's/foo/bar/g' /tmp/test_1gb.dat

# Check peak RSS in output - should be <100MB
```

**Compare output correctness**:
```bash
# Verify streaming matches in-memory processing
./target/release/sedx 's/foo/bar/g' small_file.txt > /tmp/out1.txt
# Force streaming with 1MB threshold
# (modify threshold in code or create large test file)

# Compare with GNU sed
sed 's/foo/bar/g' small_file.txt > /tmp/out2.txt
diff /tmp/out1.txt /tmp/out2.txt
```
