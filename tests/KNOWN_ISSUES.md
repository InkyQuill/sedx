# Known Issues and Limitations

## Known Issues

### 1. Insert/Append/Change Commands (i, a, c)

**Status**: Resolved in current codebase (i, a, c commands are implemented).
The issues listed here were captured during early development.

### 2. Unicode in Pattern Matching

**Status**: Historical — character boundary panic was observed during early development:
```
thread 'main' panicked at src/sed_parser.rs:297:32:
byte index 36 is not a char boundary; it is inside 'に' (bytes 35..38)
```
Verify current behaviour with: `echo "Hello こんにちは" | cargo run -- 's/(Hello|こんにちは)/MODIFIED/'`

### 3. Negation with Patterns

**Status**: Partial support. Some negation patterns (`!`) may not work as expected with all address types.

### 4. Relative Ranges

**Status**: Limited implementation. Relative offset ranges (e.g., `/pattern/,+N`) have limited support in streaming mode.

## Working Features

The following features are tested and working correctly:

- Basic Substitution: `s/foo/bar/`, `s/foo/bar/g`, `s/foo/bar/N`, `s/foo/bar/i`
- Delete: `3,5d`, `/pattern/d`
- Print: `-n 2,3p`
- Quit: `3q`
- Insert / Append / Change: `2i TEXT`, `2a TEXT`, `2,3c TEXT`
- Line Number Addressing: `3,5s/.*/MODIFIED/`
- Pattern Addressing: `/apple/s/.*/MODIFIED/`
- Pattern Ranges: `/start/,/end/d`
- Mixed Ranges: `5,/end/`, `/start/,10`
- Last Line Address: `$s/.*/TEXT/`
- Stepping: `1~2s/.*/MODIFIED/`
- PCRE Groups and backreferences
- PCRE Alternation, Quantifiers, Character Classes, Anchors
- Pipeline Mode: stdin/stdout operations
- Streaming Mode: Large file processing (constant memory)

## Fix Priority

1. **High**: Unicode pattern matching (affects international users)
2. **Medium**: Negation with all address types
3. **Medium**: Relative ranges in streaming mode
4. **Low**: ERE/BRE mode edge-case improvements
5. **Low**: Streaming `q` currently uses specialized address handling. It works correctly, but can be migrated to the shared `address_matches_current` helper in a cleanup pass for consistency.

## See Also

- `../CLAUDE.md` - SedX architecture and development guide
- `tests/*.rs` - Integration tests covering current behaviour
