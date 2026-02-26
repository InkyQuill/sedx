# Contributing to SedX

Thank you for your interest in contributing to SedX! This document provides guidelines for contributing to the project.

## Table of Contents

- [Development Setup](#development-setup)
- [Code Organization](#code-organization)
- [Testing Strategy](#testing-strategy)
- [Pull Request Guidelines](#pull-request-guidelines)
- [Adding New Commands](#adding-new-commands)
- [Code Style](#code-style)

## Development Setup

### Prerequisites

- Rust 2021 edition or later
- Git
- Linux/macOS/WSL (Windows support is experimental)

### Building

```bash
# Clone the repository
git clone https://github.com/InkyQuill/sedx.git
cd sedx

# Debug build (faster compilation)
cargo build

# Release build (optimized binary)
cargo build --release

# Run the binary
./target/release/sedx --version
```

### Development Dependencies

```bash
# Install development tools
cargo install cargo-watch      # Watch for changes and rebuild
cargo install cargo-expand      # Macro expansion debugging

# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_parse_substitution
```

### Running Tests

```bash
# Run all Rust unit tests
cargo test

# Run integration test suites
./tests/run_all_tests.sh        # All tests
./tests/run_quick_tests.sh      # Quick tests only
./tests/regression_tests.sh     # GNU sed compatibility
./tests/comprehensive_tests.sh  # Extended features
./tests/streaming_tests.sh      # Large file streaming
./tests/hold_space_tests.sh     # Hold space operations
```

### Code Quality

```bash
# Format code
cargo fmt

# Check formatting without making changes
cargo fmt -- --check

# Lint with clippy
cargo clippy -- -D warnings

# Run both format and lint checks
cargo fmt && cargo clippy -- -D warnings
```

## Code Organization

### Directory Structure

```
sedx/
├── src/                    # Source code
│   ├── main.rs            # Entry point, command routing
│   ├── cli.rs             # Command-line parsing
│   ├── command.rs         # Command and Address enums
│   ├── parser.rs          # Expression parser with flavor support
│   ├── sed_parser.rs      # Legacy sed expression parser
│   ├── bre_converter.rs   # BRE to PCRE conversion
│   ├── ere_converter.rs   # ERE backreference conversion
│   ├── capability.rs      # Streaming capability checks
│   ├── file_processor.rs  # In-memory and streaming processors
│   ├── backup_manager.rs  # Backup creation and restoration
│   ├── config.rs          # Configuration file management
│   ├── diff_formatter.rs  # Output formatting
│   ├── disk_space.rs      # Disk space checking utilities
│   ├── regex_error.rs     # Enhanced error messages
│   ├── error_helpers.rs   # Error handling utilities
│   └── lib.rs             # Library exports
├── tests/                  # Integration tests
│   ├── run_all_tests.sh
│   ├── regression_tests.sh
│   ├── comprehensive_tests.sh
│   ├── streaming_tests.sh
│   ├── scripts/           # Phase-specific test scripts
│   └── memory_profile.sh  # Memory usage testing
├── docs/                   # Documentation
│   ├── ARCHITECTURE.md    # Architecture documentation
│   ├── CONTRIBUTING.md    # This file
│   └── archive/           # Archived documentation
└── man/                    # Manual pages
    └── sedx.1
```

### Module Responsibilities

| Module | Purpose | Key Types/Functions |
|--------|---------|-------------------|
| `main.rs` | Entry point, CLI routing | `main()`, `execute_command()`, `rollback()` |
| `cli.rs` | Argument parsing | `RegexFlavor`, `Args`, `parse_args()` |
| `command.rs` | Core data structures | `Command`, `Address`, `SubstitutionFlags` |
| `parser.rs` | Expression parsing | `Parser`, `convert_pattern()`, `convert_replacement()` |
| `sed_parser.rs` | Legacy parser | `parse_sed_expression()`, `SedCommand` |
| `bre_converter.rs` | BRE conversion | `convert_bre_to_pcre()`, `convert_sed_backreferences()` |
| `ere_converter.rs` | ERE conversion | `convert_ere_to_pcre_pattern()`, `convert_ere_backreferences()` |
| `capability.rs` | Streaming checks | `can_stream()`, `is_range_streamable()` |
| `file_processor.rs` | File processing | `FileProcessor`, `StreamProcessor`, `CycleState` |
| `backup_manager.rs` | Backup system | `BackupManager`, `BackupMetadata` |
| `config.rs` | Configuration | `Config`, `load_config()`, `validate_config()` |
| `diff_formatter.rs` | Output formatting | `format_diff_with_context()`, `format_history()` |

## Testing Strategy

### Unit Tests

Unit tests are located in each module's `#[cfg(test)]` section. They test:

- Parser behavior
- Address resolution
- Regex conversion
- Command execution logic
- Backup creation and restoration

**Example unit test:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_substitution() {
        let parser = Parser::new(RegexFlavor::PCRE);
        let result = parser.parse("s/foo/bar/");
        assert!(result.is_ok());

        let commands = result.unwrap();
        assert_eq!(commands.len(), 1);
        matches!(commands[0], Command::Substitution { .. });
    }
}
```

### Integration Tests

Integration tests compare SedX output with GNU sed to ensure compatibility.

**Adding a new integration test:**

1. Create a test function in the appropriate test script
2. Create a temporary test file with known content
3. Run both GNU sed and SedX with the same expression
4. Compare outputs
5. Clean up test files

**Example:**
```bash
test_substitution_with_backreference() {
    local test_file="/tmp/test_br_$$.txt"
    echo "foo bar baz" > "$test_file"

    local expected=$(sed 's/\([a-z]\+\) \([a-z]\+\)/\2 \1/' "$test_file")
    local actual=$(./target/release/sedx 's/([a-z]+) ([a-z]+)/$2 $1/' "$test_file")

    assertEquals "$expected" "$actual"
    rm -f "$test_file"
}
```

### Streaming Tests

For large file handling, create tests that verify:
- Correctness (output matches in-memory processing)
- Memory efficiency (constant memory usage)
- Performance (reasonable processing time)

```bash
# Create 1GB test file
dd if=/dev/zero of=/tmp/test_1gb.dat bs=1M count=1024

# Process with memory monitoring
/usr/bin/time -v ./target/release/sedx 's/foo/bar/g' /tmp/test_1gb.dat

# Expected: Peak RSS < 100MB
```

### Property-Based Tests

Consider using `proptest` for testing invariants:

```rust
#[proptest]
fn test_substitution_preserves_line_count(s in "\\PC*") {
    let result = apply_substitution(&s, "foo", "bar");
    prop_assert_eq!(result.lines().count(), s.lines().count());
}
```

## Pull Request Guidelines

### Commit Message Format

Follow the conventional commits format:

```
<type>: <description>

[optional body]

[optional footer]
```

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `refactor` - Code refactoring without behavior change
- `docs` - Documentation changes
- `test` - Adding or updating tests
- `chore` - Maintenance tasks
- `perf` - Performance improvements
- `ci` - CI/CD changes

**Examples:**
```
feat: Add support for negated pattern ranges

Streaming mode now supports negated addresses in ranges
like /pattern/!s/foo/bar/.

Fixes #123
```

```
fix: Correct hold space behavior with single-line addresses

When 'g' command is used with a single-line address,
only the first line of multiline hold space was being
used. Now uses the full hold space content.
```

### Before Submitting

1. **Run all tests:**
   ```bash
   cargo test
   ./tests/run_all_tests.sh
   ```

2. **Format code:**
   ```bash
   cargo fmt
   ```

3. **Run clippy:**
   ```bash
   cargo clippy -- -D warnings
   ```

4. **Update documentation:**
   - If adding a command, update the man page
   - If changing behavior, update relevant docs
   - Add examples to test scripts

5. **Write a good PR description:**
   - Summarize changes
   - Link related issues
   - Include before/after examples if applicable

### PR Review Process

1. Automated checks must pass (tests, formatting, clippy)
2. At least one maintainer approval required
3. Address all `CRITICAL` and `HIGH` review feedback
4. Fix `MEDIUM` issues when possible

## Adding New Commands

### Simple Commands

For commands that don't affect flow control:

**1. Add to Command enum** (`command.rs`):
```rust
pub enum Command {
    // ... existing commands
    MyNewCommand {
        range: Option<(Address, Address)>,
        // command-specific fields
    },
}
```

**2. Add parsing** (`sed_parser.rs` or `parser.rs`):
```rust
// Parse your command syntax
// e.g., for 'y' command: y/abc/xyz/
```

**3. Add handler** (`file_processor.rs` in cycle-based mode):
```rust
fn apply_command_to_cycle(&mut self, cmd: &Command, state: &mut CycleState) -> Result<CycleResult> {
    match cmd {
        Command::MyNewCommand { range } => {
            // Implementation
        }
        // ... other commands
    }
}
```

**4. Add streaming support** (if applicable):
   - Update `capability.rs::can_stream()`
   - Add handler in `StreamProcessor::process_streaming_internal()`

**5. Add tests:**
   - Unit tests in `command.rs` or `file_processor.rs`
   - Integration test in appropriate test script

### Flow Control Commands

For commands that affect program counter (`b`, `t`, `T`):

**1. Add to Command enum with label field:**
```rust
Branch {
    label: Option<String>,
    range: Option<(Address, Address)>,
},
```

**2. Update parser** to detect label definitions (`:label`)

**3. Build label registry** during parsing:
```rust
let mut label_registry: HashMap<String, usize> = HashMap::new();
for (idx, cmd) in commands.iter().enumerate() {
    if let Command::Label { name } = cmd {
        label_registry.insert(name.clone(), idx);
    }
}
```

**4. Modify program counter** in cycle execution:
```rust
Command::Branch { label, .. } => {
    let target = if let Some(l) = label {
        label_registry.get(l).copied().unwrap_or(commands.len())
    } else {
        commands.len() // Branch to end
    };
    return Ok(CycleResult::Branch(target));
}
```

**5. Test with complex flow control scenarios**

### File I/O Commands

For commands that read/write external files (`r`, `w`, `R`, `W`):

**1. Add file handle tracking to processor state:**
```rust
write_handles: HashMap<String, BufWriter<File>>,
read_positions: HashMap<String, usize>,
```

**2. Change handler signature** to accept `&mut self`:
```rust
fn apply_command_to_cycle(&mut self, ...) -> Result<CycleResult>
```

**3. Flush handles** at end of processing

**4. Handle errors gracefully** (file not found, permission denied)

## Code Style

### Rust Guidelines

- Use `cargo fmt` for formatting (no style debates)
- Follow Rust naming conventions:
  - Types: `PascalCase`
  - Functions/variables: `snake_case`
  - Constants: `SCREAMING_SNAKE_CASE`
- Use `Result<T>` for fallible operations
- Use `anyhow::Result` for application errors
- Use `thiserror` for library errors

### Error Handling

Always provide context for errors:
```rust
fs::read_to_string(path)
    .with_context(|| format!("Failed to read file: {}", path.display()))?;
```

### Documentation

- Document public APIs with `///` doc comments
- Include examples for complex functions
- Explain non-obvious behavior
- Reference related functions or modules

### Performance Considerations

- Avoid allocations in hot loops
- Use references instead of copies where possible
- Consider `cow::Cow` for conditional borrowing
- Profile before optimizing

## Getting Help

- **GitHub Issues:** Bug reports and feature requests
- **Discussions:** Design questions and general discussion
- **Documentation:** See `docs/ARCHITECTURE.md` for internal design

## License

By contributing to SedX, you agree that your contributions will be licensed under the MIT License.
