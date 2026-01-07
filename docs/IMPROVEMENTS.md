# SedX Improvement Plan

Analysis of GNU sed source code has revealed many opportunities for enhancement. This plan organizes improvements by priority and complexity.

## Summary of Current SedX Features

**Currently Implemented:**
- ✅ Basic sed commands: `s` (substitution), `d` (delete), `a` (append), `i` (insert), `c` (change), `p` (print), `q` (quit)
- ✅ Hold space operations: `h`, `H`, `g`, `G`, `x`
- ✅ Command grouping with `{ ... }`
- ✅ Address types: line numbers, patterns, `$` (last line), negation with `!`
- ✅ Address ranges: `/start/,/end/` and `1,10`
- ✅ Modern features: dry-run, interactive mode, backups, rollback, colored diffs
- ✅ Extended regex by default (like `sed -E`)

## Priority Levels

- 🔴 **High Priority** - Core functionality gaps that affect common use cases
- 🟡 **Medium Priority** - Important features that expand usability
- 🟢 **Low Priority** - Nice-to-have features and edge cases

---

## 🔴 HIGH PRIORITY IMPROVEMENTS

### 1. Missing Core Sed Commands

#### 1.1 Next Line Operations (`n`, `N`)
**Complexity:** Medium
**Impact:** High - Essential for multi-line processing

GNU sed features:
- `n` - Print pattern space, read next line, start new cycle
- `N` - Append newline and next line to pattern space

**Implementation:**
```rust
pub enum SedCommand {
    Next { range: Option<(Address, Address)> },        // n command
    NextAppend { range: Option<(Address, Address)> },   // N command
}
```

**Testing:**
```bash
# Test n: print and read next line
seq 1 5 | sedx 'n; d'  # Should print 1, 3, 5

# Test N: multi-line operations
printf "line1\nline2\nline3" | sedx 'N; s/\n/ /'  # "line1 line2\nline3"
```

#### 1.2 Multi-line Print/Delete (`P`, `D`)
**Complexity:** Medium
**Impact:** High - Complements `N` command

GNU sed features:
- `P` - Print first line of multi-line pattern space (up to `\n`)
- `D` - Delete first line of multi-line pattern space (up to `\n`), restart cycle

**Implementation:**
- Pattern space can contain multiple lines after `N` command
- Split on first newline, print/delete first part only

**Testing:**
```bash
printf "a\nb\nc\nd" | sedx 'N;N;D'  # Complex multi-line operations
```

#### 1.3 Quit Without Printing (`Q`)
**Complexity:** Low
**Impact:** Medium - Distinct from `q`

GNU sed feature:
- `Q` - Quit immediately without printing pattern space
- Unlike `q`, which prints before quitting

**Implementation:**
```rust
Quit {
    address: Option<Address>,
    print_before_quit: bool,  // true for q, false for Q
}
```

#### 1.4 File Read/Write (`r`, `w`)
**Complexity:** High
**Impact:** High - Critical for data processing pipelines

GNU sed features:
- `r file` - Append contents of file to pattern space
- `w file` - Write pattern space to file
- `R file` - Read one line from file
- `W file` - Write first line of pattern space to file

**Implementation:**
- Need file handle management (open/close/track multiple files)
- Append mode for write commands
- State tracking for read position (R command)

**Testing:**
```bash
sedx '5r header.txt' file.txt     # Insert header.txt after line 5
sedx '/error/w errors.log' file.txt # Log all error lines
```

### 2. Command-Line Options

#### 2.1 Suppress Automatic Output (`-n`)
**Complexity:** Low
**Impact:** Critical - Core sed behavior

GNU sed feature:
- `-n, --quiet, --silent` - Suppress automatic printing of pattern space

**Current Issue:** SedX always prints output
**Fix:** Add CLI flag and only print when explicitly requested

**Implementation:**
```rust
struct Cli {
    #[arg(short = 'n', long, alias = "quiet")]
    suppress_output: bool,
}
```

**Testing:**
```bash
sedx -n '1,5p' file.txt  # Only print lines 1-5, nothing else
```

#### 2.2 Script File Support (`-f`)
**Complexity:** Medium
**Impact:** High - Essential for complex scripts

GNU sed feature:
- `-f file, --file=file` - Add script from file

**Implementation:**
- Read file content
- Parse each line as sed command
- Handle shebang line (`#!/usr/bin/sed -f`)
- Support multiple `-f` options

**Testing:**
```bash
cat > script.sed << 'EOF'
s/foo/bar/g
s/baz/qux/
5,10d
EOF
sedx -f script.sed file.txt
```

#### 2.3 Multiple Expressions (`-e`)
**Complexity:** Low
**Impact:** High - Common usage pattern

GNU sed feature:
- `-e expr, --expression=expr` - Add script to commands

**Current Issue:** SedX only accepts single expression
**Fix:** Accept multiple `-e` flags

**Implementation:**
```rust
struct Cli {
    #[arg(short = 'e', long, value_name = "EXPR")]
    expressions: Vec<String>,
}
```

**Testing:**
```bash
sedx -e 's/foo/bar/' -e 's/baz/qux/' file.txt
```

### 3. Advanced Substitution Features

#### 3.1 Numbered Substitution Flag
**Complexity:** Low
**Impact:** Medium - Common requirement

GNU sed feature:
- `s/old/new/2` - Replace only 2nd occurrence on each line
- `s/old/new/1g` - Replace from 1st occurrence onwards

**Implementation:**
- Parse numeric flags in substitution
- Track match count during replacement
- Apply only to nth match

**Testing:**
```bash
echo "foo foo foo" | sedx 's/foo/bar/2'  # "foo bar foo"
```

#### 3.2 Print-on-Substitution Flag (`p`)
**Complexity:** Low
**Impact:** Medium

GNU sed feature:
- `s/old/new/p` - Print line if substitution was made

**Implementation:**
- Check if any replacements occurred
- Print pattern space if true

**Testing:**
```bash
seq 1 5 | sedx -n 's/[02468]/X/p'  # Only print substituted lines
```

### 4. Flow Control

#### 4.1 Labels and Branch (`:`, `b`)
**Complexity:** High
**Impact:** High - Essential for complex scripts

GNU sed features:
- `:label` - Define a label
- `b label` - Branch to label (unconditional goto)
- `b` (no label) - Branch to end of script

**Implementation:**
```rust
pub enum SedCommand {
    Label { name: String },
    Branch { label: Option<String> },  // None = end of script
}
```

**Need:**
- Label registry during parsing
- Program counter in execution
- Jump logic

**Testing:**
```bash
# Loop until pattern matches
sedx ':top; /found/q; n; b top' file.txt
```

#### 4.2 Test Branch (`t`, `T`)
**Complexity:** High
**Impact:** High - Conditional flow control

GNU sed features:
- `t label` - Branch if substitution was made since last test
- `T label` - Branch if NO substitution was made
- `t`, `T` (no label) - Branch to end of script

**Implementation:**
- Track "substitution flag" (reset after each test)
- Conditional jump based on flag state

**Testing:**
```bash
# Repeat substitution until no more matches
sedx ':loop; s/foo/bar/; t loop; s/bar/baz/' file.txt
```

---

## 🟡 MEDIUM PRIORITY IMPROVEMENTS

### 5. Advanced Addressing Modes

#### 5.1 Stepping (`first~step`)
**Complexity:** Low
**Impact:** Medium - Useful for selective processing

GNU sed feature:
- `1~2` - Every odd line (1, 3, 5, ...)
- `2~2` - Every even line (2, 4, 6, ...)
- `0~3` - Lines 3, 6, 9, ...

**Implementation:**
```rust
pub enum Address {
    Step { first: usize, step: usize },
}
```

**Testing:**
```bash
seq 1 10 | sedx '1~3d'  # Delete lines 1, 4, 7, 10
```

#### 5.2 Relative Ranges (`addr,+N`)
**Complexity:** Low
**Impact:** Medium

GNU sed feature:
- `/start/,+5` - From start pattern to 5 lines after
- `10,+3` - Lines 10-13

**Implementation:**
```rust
pub enum Address {
    Relative { base: Address, offset: usize },
}
```

**Testing:**
```bash
sedx '/error/,+5p' log.txt  # Print error line and 5 lines after
```

### 6. Additional Sed Commands

#### 6.1 Translate Command (`y`)
**Complexity:** Low
**Impact:** Medium - Character-level operations

GNU sed feature:
- `y/abc/xyz/` - Translate a→x, b→y, c→z
- Must have same length source and target

**Implementation:**
- Build translation map
- Apply character-by-character

**Testing:**
```bash
echo "hello" | sedx 'y/el/EL/'  # "hELLo"
```

#### 6.2 Print Line Number (`=`)
**Complexity:** Low
**Impact:** Medium - Debugging utility

GNU sed feature:
- `=` - Print current line number to stdout

**Implementation:**
```rust
pub enum SedCommand {
    PrintLineNumber { range: Option<(Address, Address)> },
}
```

**Testing:**
```bash
sedx '/error/=' log.txt  # Print line numbers of errors
```

#### 6.3 List Command (`l`)
**Complexity:** Medium
**Impact:** Low - Debugging

GNU sed feature:
- `l` - Print pattern space in "visual" form
- Shows special characters (`$` for `\n`, `\\t`, etc.)
- Adjustable line length with `-l N`

**Implementation:**
- Escape non-printable characters
- Wrap lines at specified length

#### 6.4 Clear Command (`z`)
**Complexity:** Low
**Impact:** Low - Convenience

GNU sed feature:
- `z` - Clear (empty) pattern space

**Implementation:**
```rust
pub enum SedCommand {
    Clear { range: Option<(Address, Address)> },
}
```

#### 6.5 Print Filename (`F`)
**Complexity:** Low
**Impact:** Medium - Multi-file processing

GNU sed feature:
- `F` - Print current input filename

**Implementation:**
- Track current filename in file processor
- Print on command execution

**Testing:**
```bash
sedx 'F' *.txt  # Print all filenames
```

#### 6.6 Execute Shell Command (`e`)
**Complexity:** High
**Impact:** Medium - Powerful but dangerous

GNU sed feature:
- `e` - Execute pattern space as shell command, replace with output
- `e 'cmd'` - Execute cmd and send output to pattern space

**Security Concern:**
- Should only work with `--sandbox` disabled
- Warn user about security implications

**Implementation:**
- Use `std::process::Command`
- Capture stdout
- Handle errors

**Testing:**
```bash
sedx 's/^/echo /; e' file.txt  # Execute each line as command
```

### 7. Enhanced File Handling

#### 7.1 Separate Files Mode (`-s`)
**Complexity:** Medium
**Impact:** Medium - Multi-file behavior

GNU sed feature:
- `-s, --separate` - Treat files as separate (reset ranges/line numbers)

**Current Issue:** Ranges and line numbers span multiple files
**Fix:** Reset state between files when flag is set

**Testing:**
```bash
sedx -s '1d' file1.txt file2.txt  # Delete first line of EACH file
```

#### 7.2 Unbuffered I/O (`-u`)
**Complexity:** Low
**Impact:** Low - Real-time processing

GNU sed feature:
- `-u, --unbuffered` - Minimal buffering

**Implementation:**
- Flush output after each write
- Use `BufWriter` with small buffer or disable buffering

#### 7.3 Binary Mode (`-b`)
**Complexity:** Low
**Impact:** Low - Binary file processing

GNU sed feature:
- `-b, --binary` - Binary mode (no line ending conversion)

**Implementation:**
- Read/write files as bytes
- Disable text mode transformations

### 8. Special Delimiters and Line Endings

#### 8.1 Null-Terminated Lines (`-z`)
**Complexity:** Medium
**Impact:** Medium - Process filenames with `find -print0`

GNU sed feature:
- `-z, --null-data` - Lines end in `\0` instead of `\n`

**Implementation:**
- Split on `\0` instead of `\n`
- Adjust regex `^` and `$` anchors

**Testing:**
```bash
find . -print0 | sedx -z 's/foo/bar/'
```

#### 8.2 Custom Substitution Delimiters
**Complexity:** Low
**Impact:** Low - Convenience

GNU sed feature:
- `s|old|new|`, `s#old#new#`, `s^old^new^` - Any character as delimiter

**Current Status:** May already work, need to verify

**Testing:**
```bash
sedx 's|/usr/local|/opt|' file.txt  # Easier than escaping /
```

### 9. Character Classes and Case Conversion

#### 9.1 Case Conversion Modifiers
**Complexity:** High
**Impact:** Medium - Advanced text manipulation

GNU sed features in replacement string:
- `\L` - Convert to lowercase until `\E`
- `\l` - Convert next character to lowercase
- `\U` - Convert to uppercase until `\E`
- `\u` - Convert next character to uppercase
- `\E` - End case conversion

**Implementation:**
- Parse escape sequences in replacement
- Apply case conversion during substitution
- Track state for `\L`/`\U` mode

**Testing:**
```bash
echo "hello world" | sedx 's/.*/\u&/'  # "Hello world"
echo "hello world" | sedx 's/.*/\U&/'  # "HELLO WORLD"
echo "HELLO WORLD" | sedx 's/\w\+/\L&/g'  # "hello world"
```

---

## 🟢 LOW PRIORITY IMPROVEMENTS

### 10. Mode Flags and Compatibility

#### 10.1 POSIX Mode (`--posix`)
**Complexity:** High
**Impact:** Low - Strict compliance

GNU sed feature:
- `--posix` - Disable GNU extensions

**Implementation:**
- Toggle parser to reject:
  - Extended regex (use BRE instead)
  - GNU-specific commands (`e`, `F`, etc.)
  - Advanced addressing modes

**Note:** SedX is designed to be GNU sed compatible, not POSIX strict

#### 10.2 Sandbox Mode (`--sandbox`)
**Complexity:** Low
**Impact:** Medium - Security

GNU sed feature:
- `--sandbox` - Disable `e`, `r`, `w` commands

**Implementation:**
- Add flag to CLI
- Check flag before executing file/shell operations
- Return error if disabled

**Testing:**
```bash
sedx --sandbox 'e echo "dangerous"'  # Should error
```

#### 10.3 Debug Mode (`--debug`)
**Complexity:** High
**Impact:** Low - Development tooling

GNU sed feature:
- `--debug` - Print command execution and state

**Implementation:**
- Log each command execution
- Show pattern space, hold space state
- Print address resolution

**Testing:**
```bash
sedx --debug '5d' file.txt  # Show execution trace
```

#### 10.4 Follow Symlinks (`--follow-symlinks`)
**Complexity:** Low
**Impact:** Low - In-place editing edge case

GNU sed feature:
- `--follow-symlinks` - Follow symlinks for in-place editing

**Implementation:**
- When creating backup, check if file is symlink
- If flag set, operate on target, not symlink
- Update symlink instead of replacing it

### 11. Performance and Scalability

#### 11.1 Regex Optimization
**Complexity:** High
**Impact:** Medium - Large file performance

GNU sed approach:
- Uses DFA (Deterministic Finite Automaton) for efficient matching
- Fastmap for quick character class checks

**Implementation Options:**
- Use `regex` crate's DFA feature
- Compile patterns once, reuse
- Cache compiled regex objects

#### 11.2 Large File Support (2GB+ lines)
**Complexity:** Medium
**Impact:** Low - Edge case

GNU sed feature:
- 64-bit line numbers
- Dynamic line resizing

**Implementation:**
- Use `u64` or `usize` for line numbers
- Test with multi-gigabyte files

#### 11.3 Multibyte Character Handling
**Complexity:** High
**Impact:** Medium - Unicode support

GNU sed feature:
- Full multibyte character support
- Locale-aware character classes

**Implementation:**
- Use `unicode-segmentation` crate
- Grapheme-aware operations
- Locale support via `locale` crate

### 12. Error Handling and Diagnostics

#### 12.1 Better Error Messages
**Complexity:** Medium
**Impact:** High - User experience

**Improvements:**
- Show exact location of parse errors
- Suggest corrections for common mistakes
- Provide examples in error messages
- Color-coded error output

**Example:**
```
error: Invalid regex at position 5
   --> 's/foo/[0-9/bar'
           |
           unclosed character class

hint: Use 's/foo/[0-9]/bar' or escape with '\['
```

#### 12.2 Warning System
**Complexity:** Medium
**Impact:** Medium - Catch mistakes early

**Warnings to add:**
- Unused flags (e.g., `s/foo/bar/g` when no duplicates)
- Overlapping ranges
- Suspicious regex patterns
- Deprecated syntax

**Implementation:**
```rust
#[arg(short = 'W', long)]
warn: Vec<WarningLevel>,
```

### 13. Documentation and Testing

#### 13.1 Comprehensive Test Suite
**Complexity:** High
**Impact:** High - Confidence in correctness

**Add tests for:**
- All sed commands (individual and combined)
- Edge cases (empty files, single lines, etc.)
- Error conditions
- Multi-file operations
- Unicode/multibyte characters

**Structure:**
```
tests/
  ├── unit/              # Rust unit tests (already exists)
  ├── integration/       # Full script tests
  ├── regression/        # GNU sed comparison (already exists)
  ├── edge_cases/        # Unusual inputs
  └── performance/       # Large file benchmarks
```

#### 13.2 Tutorial and Examples
**Complexity:** Low
**Impact:** High - User onboarding

**Add to documentation:**
- Interactive tutorial
- Common use cases with examples
- Migration guide from sed to sedx
- Performance tips
- Best practices

### 14. Quality of Life Improvements

#### 14.1 Backup Suffix Support
**Complexity:** Low
**Impact:** Medium - Compatibility

GNU sed feature:
- `sed -i.bak 's/foo/bar/' file.txt` creates `file.txt.bak`

**SedX enhancement:**
- `sedx --execute --backup-suffix=.bak 's/foo/bar/' file.txt`
- Integrates with existing backup system

#### 14.2 Colored Output Control
**Complexity:** Low
**Impact:** Low - Terminal compatibility

**Add options:**
- `--color=always|never|auto` (default: auto)
- Respect `NO_COLOR` environment variable
- Detect terminal support for colors

#### 14.3 Configuration File
**Complexity:** Medium
**Impact:** Low - Personalization

**Implement `~/.sedx/config.toml`:**
```toml
[color]
diff = "auto"
context_lines = 3

[backup]
keep_count = 50

[behavior]
default_regex = "extended"  # or "basic"
```

#### 14.4 Shell Completions
**Complexity:** Low
**Impact:** Medium - Convenience

**Generate completions for:**
- Bash
- Zsh
- Fish
- PowerShell

**Implementation:**
- Use `clap_complete` crate
- Add `--completions` flag

---

## IMPLEMENTATION ROADMAP

### Phase 1: Core Functionality (Months 1-2)
1. ✅ Review and prioritize list with community feedback
2. Implement `-n` flag (suppress output)
3. Implement `-e` flag (multiple expressions)
4. Implement `-f` flag (script files)
5. Add `n` and `N` commands (next line operations)
6. Add `P` and `D` commands (multi-line operations)
7. Add `Q` command (quit without printing)
8. Add numbered substitution flag
9. Add print-on-substitution flag (`p`)

**Goal:** Reach 95% GNU sed compatibility for common use cases

### Phase 2: Flow Control (Month 3)
1. Implement labels (`:`) and branch (`b`)
2. Implement test branch (`t`, `T`)
3. Update tests for control flow
4. Document with examples

**Goal:** Enable complex sed scripts to run unmodified

### Phase 3: File Operations (Months 4-5)
1. Implement `r` (read file) and `w` (write file)
2. Implement `R` and `W` variants
3. Add sandbox mode
4. Security audit for file operations
5. Comprehensive testing

**Goal:** Enable data pipeline operations

### Phase 4: Advanced Features (Months 5-6)
1. Implement stepping addresses (`first~step`)
2. Implement relative ranges (`addr,+N`)
3. Implement `y` (translate) command
4. Implement `=` (line number) command
5. Implement `F` (filename) command
6. Implement `z` (clear) command

**Goal:** Feature parity with GNU sed for 99% of use cases

### Phase 5: Polish and Performance (Ongoing)
1. Add `l` (list) command
2. Implement case conversion modifiers
3. Add `e` (execute) command with sandbox
4. Regex optimization
5. Improve error messages
6. Add comprehensive test suite
7. Write tutorials and examples

**Goal:** Production-ready, well-documented, well-tested

### Phase 6: Specialized Features (As needed)
1. Null-terminated lines (`-z`)
2. Binary mode (`-b`)
3. POSIX mode (`--posix`)
4. Debug mode (`--debug`)
5. Multibyte/Unicode improvements
6. Large file support

**Goal:** Handle edge cases and special requirements

---

## SUCCESS METRICS

### Compatibility
- [ ] 95% of common sed scripts run without modification
- [ ] All regression tests pass (comparison with GNU sed)
- [ ] Pass standard sed test suites (if available)

### Performance
- [ ] Within 2x of GNU sed speed for common operations
- [ ] Handle files >1GB efficiently
- [ ] Memory usage comparable to GNU sed

### Reliability
- [ ] Comprehensive test coverage (>80%)
- [ ] Zero data loss bugs in production
- [ ] Clear, actionable error messages

### Usability
- [ ] Interactive tutorial completed
- [ ] Migration guide from sed available
- [ ] Shell completions for major shells
- [ ] Active community feedback incorporated

---

## NOTES

### Key Design Decisions

1. **Extended Regex by Default:** SedX uses ERE (like `sed -E`), unlike GNU sed's BRE. This is intentional and should remain.

2. **Safety First:** SedX prioritizes safety over speed (backups, dry-run, rollback). Performance is secondary to preventing data loss.

3. **Modern Rust Idioms:** Use Rust's strengths (ownership, type system) rather than directly translating C patterns.

4. **User Experience Over Compatibility:** If a feature significantly improves UX, it's worth a minor compatibility break (document clearly).

### Testing Strategy

1. **Unit Tests:** For each new feature, add Rust unit tests
2. **Regression Tests:** Compare output with GNU sed for standard patterns
3. **Integration Tests:** Test complex multi-command scripts
4. **Edge Case Tests:** Empty files, single lines, huge files, binary data
5. **Property-Based Tests:** Use `proptest` for invariant checking

### Security Considerations

1. **Command Injection:** The `e` command is dangerous; always sandbox by default
2. **File Operations:** Validate paths, prevent directory traversal
3. **Resource Limits:** Protect against memory bombs (e.g., `N` on huge file)
4. **Symlinks:** Careful with symlink following to prevent confusion

---

## CONCLUSION

This plan provides a roadmap to bring SedX to near-complete GNU sed compatibility while maintaining its modern, safe, and user-friendly design. The phased approach allows for incremental progress with regular releases and community feedback.

The high-priority items address the most common pain points and missing features. Medium and low priorities can be tackled based on user demand and developer interest.

**Estimated effort:** 4-6 months of part-time work for Phases 1-4, ongoing for Phase 5-6.
