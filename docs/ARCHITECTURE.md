# SedX Architecture

**Last Updated:** 2025-02-25

This document describes the internal architecture of SedX, a safe, modern replacement for GNU sed written in Rust.

## Overview

SedX processes text files using sed-like expressions with three key design goals:

1. **Safety**: Automatic backups, preview mode, and rollback functionality
2. **Performance**: Streaming mode for processing 100GB+ files with <100MB RAM
3. **Compatibility**: ~90% GNU sed compatibility with PCRE as the default regex flavor

## High-Level Architecture

```
User Input (CLI)
       |
       v
+-------------+
|  cli.rs     |  Parse arguments, determine regex flavor
+-------------+
       |
       v
+-------------+
|  parser.rs  |  Convert sed expression to Command list
+-------------+
       |
       v
+------------------+
| Regex Conversion |  BRE/ERE patterns converted to PCRE
+------------------+
       |
       v
+--------------------------+
|  capability.rs           |  Check if streaming is supported
+--------------------------+
       |
       v
    +-------+-------+
    |               |
    v               v
+---------+    +-----------+
| In-Memory|    |Streaming  |
|Processor|    |Processor  |
+---------+    +-----------+
    |               |
    v               v
+----------------------------+
|  backup_manager.rs         |  Create/restore backups
+----------------------------+
       |
       v
+------------------+
| diff_formatter.rs|  Generate human-readable output
+------------------+
       |
       v
   User Output
```

## Core Modules

### cli.rs - Command-Line Interface

**Responsibilities:**
- Parse command-line arguments using `clap`
- Determine regex flavor (PCRE/ERE/BRE)
- Route to subcommands (execute/rollback/history/status/config/backup)

**Key Types:**
```rust
pub enum RegexFlavor {
    PCRE,  // Perl-Compatible (default)
    ERE,   // Extended (-E flag)
    BRE,   // Basic (-B flag)
}

pub enum Args {
    Execute { expression, files, dry_run, interactive, ... },
    Rollback { id },
    History,
    Status,
    // ... backup subcommands
}
```

### command.rs - Unified Command System

**Responsibilities:**
- Define the `Command` enum representing all sed operations
- Define the `Address` enum for all addressing modes
- Define `SubstitutionFlags` for substitution command options

**Key Commands:**
- `Substitution` - Text replacement with flags (g, i, nth)
- `Delete` - Delete lines in range
- `Print` - Print lines in range
- `Quit` / `QuitWithoutPrint` - Stop processing
- `Insert` / `Append` / `Change` - Add/modify lines
- `Group` - Command grouping with `{...}`
- `Hold` / `HoldAppend` / `Get` / `GetAppend` / `Exchange` - Hold space operations
- `Next` / `NextAppend` / `PrintFirstLine` / `DeleteFirstLine` - Multi-line pattern space
- `Label` / `Branch` / `Test` / `TestFalse` - Flow control (Phase 5)
- `ReadFile` / `WriteFile` / `ReadLine` / `WriteFirstLine` - File I/O (Phase 5)
- `PrintLineNumber` / `PrintFilename` / `ClearPatternSpace` - Additional commands (Phase 5)

**Address Types:**
- `LineNumber(usize)` - Specific line (1-indexed)
- `Pattern(String)` - Regex pattern match
- `FirstLine` - Special address "0"
- `LastLine` - Special address "$"
- `Negated(Box<Address>)` - Inverted match with "!"
- `Relative { base, offset }` - Relative offset (e.g., `/pattern/,+5`)
- `Step { start, step }` - Stepping (e.g., `1~2` for every 2nd line)

### parser.rs - Expression Parsing

**Responsibilities:**
- Parse sed expression strings into `Vec<Command>`
- Convert patterns/replacements based on regex flavor
- Integrate with legacy `sed_parser.rs` for actual parsing

**Regex Conversion Pipeline:**
```
User Input -> Parser -> Flavor Detection -> Converter -> PCRE Pattern -> Regex Engine
                   |           |                |
                   |           +-- PCRE ---------> Pass through
                   |           +-- ERE  --------> Backreference conversion (\1 -> $1)
                   |           +-- BRE  --------> Full conversion (\( -> (, \1 -> $1)
                   v
           Compiled Regex (Rust regex crate)
```

**Converter Modules:**
- `bre_converter.rs` - Converts BRE patterns to PCRE
- `ere_converter.rs` - Converts ERE backreferences to PCRE format
- PCRE patterns pass through unchanged

### capability.rs - Streaming Capability Checks

**Responsibilities:**
- Determine if a command set can execute in streaming mode
- Identify commands requiring full file buffering

**Non-Streamable Commands:**
- Multi-line pattern space operations (`n`, `N`, `P`, `D`)
- Flow control commands (`b`, `t`, `T`)
- File I/O commands (`r`, `w`, `R`, `W`)
- Commands with negated ranges

**Streamable Ranges:**
- Line-to-line: `1,10`
- Pattern-to-pattern: `/start/,/end/`
- Mixed: `/start/,10` or `5,/end/`
- Relative: `/start/,+5`
- Stepping: `1~2`

### file_processor.rs - File Processing

**Dual Architecture:**

#### In-Memory Mode (FileProcessor)

For files < 100MB or when streaming is unsupported:
- Loads entire file into `Vec<String>`
- Supports all sed commands
- Generates full diff with context
- Two processing modes:
  - **Batch-based**: Apply all commands to all lines at once (faster)
  - **Cycle-based**: Process each line through command cycle (GNU sed compatible)

**CycleState Structure:**
```rust
struct CycleState {
    pattern_space: String,       // Current line(s) being processed
    hold_space: String,          // Secondary buffer
    line_num: usize,             // Current line number
    deleted: bool,               // Line marked for deletion
    side_effects: Vec<String>,   // Output from p/P commands
    file_reads: Vec<String>,     // Output from r/R commands
    stdout_outputs: Vec<String>, // Output from =/F commands
    current_filename: String,    // For F command
    line_iter: LineIterator,     // For n/N lookahead
    substitution_made: bool,     // For t/T commands
}
```

#### Streaming Mode (StreamProcessor)

For files >= 100MB with streamable commands:
- Constant memory usage (<100MB regardless of file size)
- Line-by-line processing with `BufReader`
- Sliding window diff context (VecDeque)

**Streaming State:**
```rust
pub struct StreamProcessor {
    commands: Vec<Command>,
    hold_space: String,
    current_line: usize,
    context_buffer: VecDeque<(usize, String, ChangeType)>,  // Sliding window
    context_size: usize,                                     // Default: 2
    pattern_range_states: HashMap<(String, String), PatternRangeState>,
    mixed_range_states: HashMap<MixedRangeKey, MixedRangeState>,
    dry_run: bool,
}
```

**Pattern Range State Machine:**
```rust
enum PatternRangeState {
    LookingForStart,                  // Before /start/ matches
    InRange,                          // Between /start/ and /end/
    WaitingForLineNumber { ... },     // For /start/,10
    CountingRelativeLines { ... },    // For /start/,+5
}
```

**Atomic File Writes:**
```rust
let temp_file = NamedTempFile::new_in(parent_dir)?;
// Write to temp file
temp_file.persist(file_path)?;  // Atomic rename (POSIX)
```

### backup_manager.rs - Backup System

**Directory Structure:**
```
~/.sedx/backups/
├── 20250125-120000-abc123/
│   ├── operation.json    # Metadata (expression, timestamp, files)
│   └── files/
│       ├── file1.txt     # Original file contents
│       └── file2.txt
├── 20250125-130000-def456/
│   └── ...
```

**Backup Lifecycle:**
1. User runs `sedx 's/foo/bar/g' file.txt`
2. Files are copied to `~/.sedx/backups/<timestamp-id>/files/`
3. Metadata written to `operation.json`
4. Changes applied to original files
5. Backup ID shown to user
6. Old backups cleaned up (keeps last 50)

**Rollback Process:**
1. User runs `sedx rollback <id>`
2. `operation.json` read to get file list
3. Files copied from backup to original locations
4. Backup directory removed
5. Success confirmation displayed

**Backup Metadata:**
```rust
pub struct BackupMetadata {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub expression: String,
    pub files: Vec<FileBackup>,
}

pub struct FileBackup {
    pub original_path: PathBuf,
    pub backup_path: PathBuf,
}
```

**Disk Space Checks:**
- Warn if backup > 2GB
- Error if backup would use > 60% of free disk space
- Configurable via `~/.sedx/config.toml`

### config.rs - Configuration Management

**Config File Location:** `~/.sedx/config.toml`

**Structure:**
```toml
[backup]
max_size_gb = 2                  # Warn for large backups
max_disk_usage_percent = 60      # Error if disk usage too high
backup_dir = "/custom/path"      # Optional custom location

[compatibility]
mode = "pcre"                    # Default regex flavor
show_warnings = true             # Show compatibility warnings

[processing]
context_lines = 2                # Diff context (max: 10)
max_memory_mb = 100              # Streaming threshold
streaming = true                 # Enable auto-detection
```

### diff_formatter.rs - Output Formatting

**Responsibilities:**
- Format human-readable diffs with color
- Display operation history
- Format dry-run headers

**Change Types:**
- `Modified` (yellow) - Line content changed
- `Added` (green) - New line inserted
- `Deleted` (red) - Line removed
- `Unchanged` (default) - Context line

## Data Flow

### Expression Execution Flow

```
1. CLI Parsing
   sedx 's/foo/bar/g' file.txt
   -> Args::Execute { expression: "s/foo/bar/g", files: ["file.txt"], ... }

2. Expression Parsing
   parser.parse("s/foo/bar/g")
   -> vec![Command::Substitution { pattern: "foo", replacement: "bar", ... }]

3. Regex Compilation
   For PCRE: "foo" -> Regex::new("foo")
   For BRE: "\(foo\)" -> convert to "(foo)" -> Regex::new("(foo)")

4. File Processing Decision
   File size: 50MB < 100MB threshold
   Commands are streamable: true
   -> Use streaming mode (prefer streaming for consistency)

5. Apply Commands
   For each line:
   - Check if line is in range
   - Apply substitution if matches pattern
   - Collect changes

6. Generate Diff
   changes = [LineChange { line_number: 5, change_type: Modified, ... }]

7. Format Output
   Print colored diff showing changes with context
```

### Backup Creation Flow

```
1. Pre-Processing Check
   Can commands modify files?
   -> commands_can_modify_files() returns true

2. Disk Space Check
   Total file size: 150MB
   Free disk space: 500GB
   Usage: 0.03% < 60% threshold
   -> OK to proceed

3. Create Backup Directory
   ~/.sedx/backups/20250125-143022-a1b2c3d4/

4. Copy Files
   fs::copy("file.txt", "backups/.../files/file.txt")

5. Write Metadata
   operation.json: { id: "...", timestamp: "...", expression: "...", files: [...] }

6. Apply Changes
   Execute commands on original files

7. Display Result
   "Backup created: 20250125-143022-a1b2c3d4"
   "Rollback with: sedx rollback 20250125-143022-a1b2c3d4"
```

## Regex System

### Flavor Comparison

| Feature | PCRE (default) | ERE (-E) | BRE (-B) |
|---------|----------------|----------|----------|
| Grouping | `(foo\|bar)` | `(foo\|bar)` | `\(foo\|bar\)` |
| Repetition | `+`, `?`, `{n,m}` | `+`, `?`, `{n,m}` | `\+`, `\?`, `\{n,m\}` |
| Alternation | `a\|b\|c` | `a\|b\|c` | `a\|b\|c` (escaped) |
| Backreferences (pattern) | `\1`, `\2` | `\1`, `\2` | `\1`, `\2` |
| Backreferences (replacement) | `$1`, `$2` | `$1`, `$2` | `$1`, `$2` |

**Why PCRE by default?**
- Most developers familiar with Perl/Python/JavaScript regex
- No need to escape metacharacters
- Unambiguous - users explicitly choose compatibility mode

**Conversion Examples:**
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

## Streaming Architecture

### Memory Efficiency

**In-Memory Mode:**
- Memory: O(n) where n = file size
- 1GB file ~ 1GB RAM
- Faster, supports all commands

**Streaming Mode:**
- Memory: O(1) constant
- 100GB file ~ 100MB RAM
- Slower, command restrictions apply

### Streaming Implementation

**Chunk-based approach (completed chunks 1-11):**
1. Basic streaming infrastructure (BufReader/BufWriter, temp files)
2. Substitution command with flags
3. Delete and Print commands
4-5. Insert, Append, Change, Quit commands
6. Simple diff generation
7. Sliding window diff with context
8. Pattern ranges with state machine
9. Hold space operations
10. Command grouping with ranges
11. Flow control, File I/O, additional commands

**State Tracking:**
- Pattern ranges use `HashMap<(String, String), PatternRangeState>`
- Mixed ranges use `HashMap<MixedRangeKey, MixedRangeState>`
- Each command tracks its own range state independently

**Sliding Window Diff:**
```rust
// Before change: accumulate context lines
context_buffer.push_back((line_num, line, Unchanged));

// On change: flush buffer + add changed line
flush_buffer_to_changes(changes);
changes.push(LineChange { ...Modified... });

// After change: read next N lines for context
for _ in 0..context_size {
    read_next_line_as_context();
}
```

## Cycle-Based Processing

GNU sed processes each line through a "cycle" - running all commands for that line before moving to the next. SedX implements this for compatibility with flow control and hold space operations.

**Cycle Flow:**
```
For each input line:
1. Read line into pattern_space
2. Reset substitution_made = false
3. Set program_counter = 0
4. While program_counter < commands.len():
   a. Execute command at program_counter
   b. Handle CycleResult:
      - Continue -> program_counter += 1
      - DeleteLine -> break (skip output)
      - RestartCycle -> program_counter = 0
      - Branch(idx) -> program_counter = idx
      - Quit(code) -> exit processing
5. Output pattern_space (unless deleted or -n flag)
6. Read next line
```

**Flow Control Commands:**
- `:label` - Define branch target (no-op in execution)
- `b [label]` - Unconditional branch to label (or end if no label)
- `t [label]` - Branch if substitution_made is true, then reset flag
- `T [label]` - Branch if substitution_made is false

## Testing Strategy

### Unit Tests
- Located in each module's `#[cfg(test)]` section
- Test parsing, address resolution, conversion logic
- Run with `cargo test`

### Integration Tests
- Bash scripts in `tests/` directory
- Compare SedX output with GNU sed
- Run with `./tests/run_all_tests.sh`

**Test Suites:**
- `regression_tests.sh` - GNU sed compatibility
- `comprehensive_tests.sh` - Extended features
- `streaming_tests.sh` - Large file handling
- `phase4_tests.sh` - Multi-line pattern space
- `phase5_tests.sh` - Flow control and file I/O
- `hold_space_tests.sh` - Hold space operations

### Memory Profiling
```bash
# Test with large file
/usr/bin/time -v ./target/release/sedx 's/foo/bar/g' large_file.txt
# Expected: Peak RSS < 100MB for 100GB file
```

## Performance Considerations

1. **Streaming Threshold**: Default 100MB, configurable via `max_memory_mb`
2. **Regex Compilation**: Patterns compiled once, reused for all lines
3. **Atomic Writes**: Uses `tempfile` crate for safe file updates
4. **Backup Overhead**: File copies add O(n) time but enable rollback
5. **Context Size**: Larger context uses more memory in streaming mode

## Future Directions

1. **Parallel Processing**: Process multiple files concurrently
2. **Incremental Backups**: Use hard links for unchanged files
3. **Compression**: Compress large backups
4. **Plugin System**: Custom commands via dynamic loading

## References

- GNU sed manual: https://www.gnu.org/software/sed/manual/
- PCRE syntax: https://www.pcre.org/current/doc/html/pcre2syntax.html
- Rust regex crate: https://docs.rs/regex/
