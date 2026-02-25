# SedX Test Suite Documentation

## Overview

The SedX test suite provides comprehensive coverage of all SedX functionality using pre-generated test fixtures and automated test scripts.

## Directory Structure

```
tests/
├── fixtures/              # Pre-generated test data files
│   ├── basic/            # Basic command tests (s, d, p, q, i, a, c)
│   ├── addressing/       # Addressing and range tests
│   ├── holdspace/        # Hold space operation tests
│   ├── regex/            # Regex flavor tests (PCRE, ERE, BRE)
│   ├── streaming/        # Streaming mode tests
│   ├── pipeline/         # Stdin/stdout pipeline tests
│   ├── edge/             # Edge case tests
│   └── advanced/         # Advanced feature tests
├── scripts/              # Test execution scripts
│   ├── basic_tests.sh
│   ├── addressing_tests.sh
│   ├── regex_tests.sh
│   ├── pipeline_tests.sh
│   ├── streaming_tests.sh
│   ├── holdspace_tests.sh
│   └── edge_tests.sh
├── large_files/          # Generated large files for streaming tests
├── run_all_tests.sh      # Master test runner (all tests)
├── run_quick_tests.sh    # Quick test runner (subset)
└── generate_large_files.sh  # Generate large test files
```

## Test Fixture Format

Each test fixture follows GNU sed's convention:
- `.inp` files - Input data
- `.good` files - Expected output
- `.sed` files - Sed scripts (for complex tests)

Example:
```
fixtures/basic/substitute.inp  -> Input data
fixtures/basic/substitute.good -> Expected output
```

## Test Categories

### 1. Basic Command Tests (`basic_tests.sh`)

Tests core sed commands:
- **Substitution**: `s/foo/bar/`, `s/foo/bar/g`, `s/foo/bar/2`, `s/foo/bar/i`
- **Delete**: `3,5d`
- **Print**: `2,3p` with `-n` flag
- **Quit**: `3q`
- **Insert**: `2i TEXT`
- **Append**: `2a TEXT`
- **Change**: `2,3c TEXT`

### 2. Addressing and Range Tests (`addressing_tests.sh`)

Tests various addressing modes:
- Line number addresses: `3,5s/.*/MODIFIED/`
- Pattern addresses: `/apple/s/.*/MODIFIED/`
- Line number ranges: `3,5d`
- Pattern ranges: `/start/,/end/d`
- Mixed ranges: `/pattern/,5d`
- Negation: `/delete/!d`
- Relative offsets: `/pattern/,+2s/.*/MODIFIED/`
- Last line: `$s/.*/TEXT/`
- Stepping: `1~2s/.*/MODIFIED/`

### 3. Regex Flavor Tests (`regex_tests.sh`)

Tests all three regex modes:

**PCRE (default)**:
- Groups and backreferences: `s/([a-z]+)([0-9]+)/$2-$1/`
- Alternation: `s/(cat|dog)/ANIMAL/g`
- Quantifiers: `s/a{3,}/MATCH/`
- Character classes: `s/[a-z]+/***/`
- Anchors: `s/^start/MODIFIED/m`

**ERE (-E flag)**:
- Basic patterns: `s/(foo|bar)/\U$1\E/`
- Groups with backreferences
- ERE backreferences: `\1`, `\2`

**BRE (-B flag)**:
- Escaped groups: `s/\(foo\|bar\)/MODIFIED/`
- BRE backreferences: `\1`, `\2`
- Legacy GNU sed compatibility

### 4. Pipeline Tests (`pipeline_tests.sh`)

Tests stdin/stdout pipeline mode:
- Simple substitution in pipeline
- Delete patterns in pipeline
- Multiple commands with `-e` flag
- Group commands in pipeline
- Case-insensitive matching in pipeline
- Global substitution in pipeline

### 5. Streaming Tests (`streaming_tests.sh`)

Tests streaming mode for large files:
- Large file substitution
- Pattern ranges in streaming mode
- Delete operations in streaming mode
- Substitute operations in streaming mode
- Long lines handling

### 6. Hold Space Tests (`holdspace_tests.sh`)

Tests hold space operations:
- **Hold** (`h`): Copy pattern space to hold space
- **Hold Append** (`H`): Append to hold space
- **Get** (`g`): Copy hold space to pattern space
- **Get Append** (`G`): Append hold space to pattern space
- **Exchange** (`x`): Swap pattern and hold space
- Complex hold space operations

### 7. Edge Case Tests (`edge_tests.sh`)

Tests edge cases and special conditions:
- Empty files
- Empty lines
- Single line files
- Special characters (`$`, `[`, `]`, `{`, `}`, `(`, `)`, `*`, `+`, `?`, `|`, `\`)
- Unicode and multibyte characters
- Newline preservation
- Whitespace handling (spaces, tabs)

### 8. Advanced Tests (`advanced/`)

Tests advanced features:
- Multi-file processing
- Nested command groups
- Complex pattern ranges
- Chained commands

## Running Tests

### Quick Test Run (Fast Feedback)

Run a subset of critical tests:

```bash
./tests/run_quick_tests.sh
```

### Full Test Suite

Run all tests with comprehensive reporting:

```bash
./tests/run_all_tests.sh
```

### Individual Test Suites

Run specific test categories:

```bash
# Basic commands
./tests/scripts/basic_tests.sh

# Addressing and ranges
./tests/scripts/addressing_tests.sh

# Regex flavors
./tests/scripts/regex_tests.sh

# Pipeline mode
./tests/scripts/pipeline_tests.sh

# Streaming mode
./tests/scripts/streaming_tests.sh

# Hold space operations
./tests/scripts/holdspace_tests.sh

# Edge cases
./tests/scripts/edge_tests.sh
```

### Custom Binary

Test with a custom build:

```bash
SEDX_BIN=./target/debug/sedx ./tests/run_all_tests.sh
```

## Generating Large Files

Generate large test files for streaming tests:

```bash
./tests/generate_large_files.sh
```

This creates:
- `test_10mb.txt` - 10MB file (100,000 lines)
- `test_100mb.txt` - 100MB file (1,000,000 lines)
- `test_1gb.txt` - 1GB file (10,000,000 lines) - optional

## Test Output Format

Tests use colored output:
- **Green**: PASSED
- **Red**: FAILED
- **Yellow**: Test suite name/progress

Example output:
```
=== SedX Basic Command Tests ===

Testing: Basic substitution (s/foo/bar/) ... PASSED
Testing: Global substitution (s/foo/bar/g) ... PASSED
Testing: Numbered substitution (s/foo/BAR/2) ... PASSED

=== Test Summary ===
Passed: 10
Failed: 0
Total: 10
```

## Adding New Tests

### 1. Create Test Fixtures

Create `.inp` (input) and `.good` (expected output) files:

```bash
# Input file
cat > tests/fixtures/basic/my_test.inp << EOF
line 1
line 2
line 3
EOF

# Expected output
cat > tests/fixtures/basic/my_test.good << EOF
MODIFIED 1
line 2
MODIFIED 3
EOF
```

### 2. Add Test to Script

Edit the appropriate test script (e.g., `tests/scripts/basic_tests.sh`):

```bash
run_test "My new test" \
    '1,3s/line/MODIFIED/' \
    "$FIXTURES_DIR/basic/my_test.inp" \
    "$FIXTURES_DIR/basic/my_test.good"
```

### 3. Run Tests

```bash
./tests/scripts/basic_tests.sh
```

## Test Helper Functions

Test scripts use the `run_test()` helper:

```bash
run_test "Test name" \
    "sed expression" \
    "input_file.inp" \
    "expected_file.good" \
    "optional_flags"
```

For pipeline tests, use `run_pipeline_test()` which automatically pipes input through stdin.

## Debugging Failed Tests

When a test fails:

1. **Check the actual vs expected output**:
   ```bash
   diff tests/fixtures/basic/test.good /tmp/sedx_test_output.txt
   ```

2. **Test manually**:
   ```bash
   ./target/release/sedx 's/foo/bar/' tests/fixtures/basic/test.inp
   ```

3. **Compare with GNU sed**:
   ```bash
   sed 's/foo/bar/' tests/fixtures/basic/test.inp
   ```

4. **Check for expression parsing issues**:
   ```bash
   ./target/release/sedx --dry-run 's/foo/bar/' tests/fixtures/basic/test.inp
   ```

## Coverage Report

Current test coverage:

- **Basic Commands**: 10 tests
- **Addressing**: 9 tests
- **Regex Flavors**: 15 tests (PCRE, ERE, BRE)
- **Pipeline**: 6 tests
- **Streaming**: 5 tests
- **Hold Space**: 6 tests
- **Edge Cases**: 7 tests
- **Advanced**: 4 tests

**Total**: 62+ automated tests

## Stress Tests (`stress_tests.sh`)

Tests performance and resource limits:

**Large File Processing**:
- 100MB text files with substitutions
- Multiple chained substitutions
- Pattern deletions on large files
- Memory profiling (requires `/usr/bin/time`)

**Many Small Files**:
- Processing 1,000 files in batch
- Glob-based batch operations

**Complex Regex Patterns**:
- 100+ character regex patterns
- 10+ capture groups
- Nested capture groups

**Unicode Edge Cases**:
- Emoji and multi-byte characters
- CJK (Chinese/Japanese/Korean) text
- Combining marks and diacritics

**Long Lines**:
- Single 1MB line processing
- Multiple 100KB lines

**Performance Benchmarks**:
- Comparison with GNU sed
- Timing measurements

Run stress tests:
```bash
./tests/stress_tests.sh
```

Note: Stress tests require `/usr/bin/time` for memory profiling and `python3` for generating test data.

## CI/CD Integration

Add to your CI pipeline:

```yaml
# .github/workflows/test.yml
name: Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build SedX
        run: cargo build --release
      - name: Run tests
        run: ./tests/run_all_tests.sh
```

## Best Practices

1. **Keep fixtures small**: Most tests should use small files (< 50 lines)
2. **Use descriptive names**: Test fixtures should clearly indicate what they test
3. **Test edge cases**: Include empty files, single lines, special characters
4. **Compare with GNU sed**: Verify behavior matches standard sed when applicable
5. **Test all regex modes**: Ensure tests work with PCRE, ERE, and BRE
6. **Test both modes**: Include both file mode and pipeline mode tests
7. **Use dry-run**: Test with `--dry-run` to avoid modifying files during tests

## Troubleshooting

### Tests Fail to Build

```bash
# Build the release binary first
cargo build --release

# Verify binary exists
ls -lh ./target/release/sedx
```

### Permission Denied on Scripts

```bash
# Make scripts executable
chmod +x tests/scripts/*.sh
chmod +x tests/run_*.sh
```

### Missing Test Fixtures

```bash
# Verify fixtures exist
ls -R tests/fixtures/

# Regenerate if needed
./tests/generate_large_files.sh
```

### Memory Issues with Large Files

For streaming tests with large files:

1. Ensure sufficient disk space
2. Monitor memory usage: `/usr/bin/time -v ./tests/scripts/streaming_tests.sh`
3. Start with smaller files (10MB) before testing with 1GB files

## References

- GNU sed test suite: `../sed/tests/`
- sd test patterns: `../sd/tests/`
- SedX documentation: `CLAUDE.md`
