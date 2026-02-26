# Known Issues and Limitations

## Test Suite Status

The SedX test suite has been created with comprehensive coverage. However, some tests reveal features that are not yet fully implemented in SedX.

## Known Issues

### 1. Insert/Append/Change Commands (i, a, c)

**Status**: Not fully implemented

The `i` (insert), `a` (append), and `c` (change) commands have parsing issues:
```
Error: Unknown sed command: 2i INSERTED LINE
Error: Unknown sed command: 2a APPENDED LINE
Error: Unknown sed command: 2,3c CHANGED CONTENT
```

These commands need parser updates to properly handle the text argument.

### 2. Unicode in Pattern Matching

**Status**: Character boundary issue

When using Unicode characters in regex patterns, there's a byte index panic:
```
thread 'main' panicked at src/sed_parser.rs:297:32:
byte index 36 is not a char boundary; it is inside 'に' (bytes 35..38)
```

This affects patterns like: `s/(Hello|こんにちは)/MODIFIED/`

**Workaround**: Use ASCII-only patterns for now.

### 3. Negation with Patterns

**Status**: Partial support

Some negation patterns (`!`) don't work as expected with all address types.

### 4. Relative Ranges

**Status**: Limited implementation

Relative offset ranges (e.g., `/pattern/,+N`) have limited support.

## Working Features

The following features are tested and working correctly:

✅ **Basic Substitution**: `s/foo/bar/`, `s/foo/bar/g`, `s/foo/bar/N`, `s/foo/bar/i`
✅ **Delete**: `3,5d`, `/pattern/d`
✅ **Print**: `-n 2,3p`
✅ **Quit**: `3q`
✅ **Line Number Addressing**: `3,5s/.*/MODIFIED/`
✅ **Pattern Addressing**: `/apple/s/.*/MODIFIED/`
✅ **Pattern Ranges**: `/start/,/end/d`
✅ **Mixed Ranges**: `5,/end/`, `/start/,10`
✅ **Last Line Address**: `$s/.*/TEXT/`
✅ **Stepping**: `1~2s/.*/MODIFIED/`
✅ **PCRE Groups**: `s/([a-z]+)([0-9]+)/$2-$1/`
✅ **PCRE Alternation**: `s/(cat|dog)/ANIMAL/g`
✅ **PCRE Quantifiers**: `s/a{3,}/MATCH/`
✅ **PCRE Character Classes**: `s/[a-z]+/***/`
✅ **PCRE Anchors**: `s/^start/MODIFIED/`
✅ **Backreferences**: `s/(\w+) (\w+)/$2-$1/`
✅ **Pipeline Mode**: stdin/stdout operations
✅ **Streaming Mode**: Large file processing

## Test Categories

### Currently Working Tests

1. **basic_tests.sh** - Partial (skip i, a, c tests)
2. **addressing_tests.sh** - Partial (skip negation, relative tests)
3. **regex_tests.sh** - PCRE tests only (skip ERE/BRE until fixed)
4. **streaming_tests.sh** - Full
5. **pipeline_tests.sh** - Full
6. **edge_tests.sh** - Skip Unicode tests

### Recommended Test Flow

For testing currently implemented features:

```bash
# Quick test (working features only)
./tests/run_quick_tests.sh

# Individual test suites
./tests/scripts/basic_tests.sh      # Skip i/a/c tests
./tests/scripts/streaming_tests.sh  # Full
./tests/scripts/pipeline_tests.sh   # Full
```

## Fix Priority

1. **High**: Unicode pattern matching (affects international users)
2. **High**: Insert/Append/Change commands (core sed functionality)
3. **Medium**: Negation with all address types
4. **Medium**: Relative ranges
5. **Low**: ERE/BRE mode improvements (PCRE works well)

## Testing Unimplemented Features

To test features that aren't working yet, the test scripts can be updated to skip those tests:

```bash
# In test scripts, add guards like:
if ! has_feature "insert_command"; then
    echo "Skipping insert tests (not implemented)"
    continue
fi
```

## Contributing Fixes

When fixing these issues:

1. Update the feature implementation
2. Regenerate affected `.good` files using SedX
3. Remove skip guards from test scripts
4. Update this KNOWN_ISSUES.md
5. Add tests to regression_tests.sh if comparing with GNU sed

## See Also

- `TEST_SUITE.md` - Full test suite documentation
- `../CLAUDE.md` - SedX architecture and development guide
- `regression_tests.sh` - GNU sed compatibility tests
