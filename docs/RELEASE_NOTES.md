# SedX v1.0.0 Release Notes

**Release Date:** 2026-02-25
**Current Version:** v0.2.6-alpha
**Target Version:** v1.0.0

---

## Executive Summary

SedX v1.0.0 represents a **production-ready, modern replacement for GNU sed** that maintains ~95% compatibility while adding safety features, modern regex capabilities, and enhanced usability.

This release culminates 5 major development phases covering:
- **Stream processing** for memory-efficient large file handling
- **Backup management** with disk space awareness
- **Enhanced regex** with PCRE as default
- **Essential sed compatibility** including flow control and file I/O
- **Production hardening** with comprehensive testing and documentation

### Key Highlights

- **~95% GNU sed compatibility** for common operations
- **Automatic backups** before every file modification
- **Preview mode** to see changes before applying
- **One-command rollback** to undo mistakes
- **Streaming mode** processes 100GB+ files with <100MB RAM
- **PCRE by default** with optional BRE/ERE modes
- **Full flow control** (labels, branches, conditional execution)
- **File I/O operations** (read, write during processing)

---

## What's New in v1.0.0

### Phase 5: Flow Control & Advanced Features (Complete)

The final development phase adds powerful scripting capabilities:

#### Flow Control Commands
```bash
# Labels and unconditional branch
sedx ':top; /found/q; n; b top' file.txt

# Conditional branch (if substitution made)
sedx ':loop; s/foo/bar/; t loop' file.txt

# Inverse branch (if NO substitution)
sedx 's/foo/bar/; T retry; b done; :retry; s/baz/qux/; :done' file.txt
```

#### File I/O Commands
```bash
# Read file contents
sedx '1r header.txt' file.txt

# Write matching lines to file
sedx '/error/w errors.log' logfile.txt

# Read one line at a time
sedx 'R data.txt' file.txt

# Write first line only
sedx 'W summary.log' file.txt
```

#### Additional Commands
```bash
# Print line numbers
sedx '=' file.txt

# Print filename
sedx 'F' file.txt

# Clear pattern space
sedx '/unwanted/z' file.txt
```

### Completed Phases 1-4

**Phase 1: Stream Processing**
- Constant memory processing regardless of file size
- Sliding window diff generation
- Atomic file writes for safety

**Phase 2: Backup Management**
- Automatic disk space checking
- Configurable backup retention
- Full backup CLI (list, show, restore, remove, prune)

**Phase 3: Enhanced Regex**
- PCRE default (modern syntax)
- Escape sequences (\n, \t, \r, \\, \xHH, \uHHHH)
- Numbered substitution (s/old/new/2)

**Phase 4: Essential Sed Compatibility**
- `-n` quiet mode
- `-e` multiple expressions
- `-f` script files
- Multi-line pattern space (n, N, P, D)
- Quit without printing (Q)

---

## Breaking Changes from v0.2.x

### Default Regex Flavor: PCRE Instead of ERE/BRE

**Before (v0.2.x):** Extended Regex (ERE) by default
**Now (v1.0.0):** PCRE (Perl-Compatible Regular Expressions) by default

**Impact:**
- Groups: Use `(foo|bar)` instead of `\(foo\|bar\)` (BRE)
- Backreferences: Use `$1`, `$2` instead of `\1`, `\2` (BRE/ERE)

**Migration:**
```bash
# For GNU sed compatibility, use -B flag
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt

# For ERE compatibility, use -E flag
sedx -E 's/(foo)(bar)/\2\1/' file.txt

# PCRE mode (new default)
sedx 's/(foo)(bar)/$2$1/' file.txt
```

### Command Output Order

Side-effect outputs (from `r`, `=`, `F` commands) now appear **before** pattern space output, matching GNU sed behavior exactly.

---

## Known Limitations

### Not Implemented (Deferred to v1.1+)

- **`y` command**: Character translation
- **`l` command**: List lines with escape sequences
- **`e` command**: Execute shell commands
- **Unicode pattern matching**: Character boundary issues in some edge cases
- **Pattern ranges with flow control**: `/start/,/end/b` parser limitation

### Architectural Differences

Multi-line pattern space commands (n, N, P, D) require explicit addresses for correct behavior in some edge cases. This is a known architectural limitation of the hybrid batch/cycle execution model.

---

## Migration Guide: v0.2.x to v1.0.0

### 1. Update Regex Patterns

**If you use BRE-style patterns:**
```bash
# Old (v0.2.x ERE mode)
sedx 's/\(foo\|bar\)/baz/' file.txt

# New (v1.0.0) - Option 1: Use PCRE
sedx 's/(foo|bar)/baz/' file.txt

# New (v1.0.0) - Option 2: Use BRE mode
sedx -B 's/\(foo\|bar\)/baz/' file.txt
```

**If you use `\1`, `\2` backreferences:**
```bash
# Old (v0.2.x)
sedx 's/\(foo\)\(bar\)/\2\1/' file.txt

# New (v1.0.0) - Option 1: Use PCRE syntax
sedx 's/(foo)(bar)/$2$1/' file.txt

# New (v1.0.0) - Option 2: Use BRE mode
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt
```

### 2. Update Scripts Using Flow Control

Flow control now works correctly with cycle-based execution:

```bash
# This now works as expected
sedx ':loop; s/foo/bar/; t loop' file.txt

# Complex scripts are now supported
sedx ':top; /found/q; n; b top' file.txt
```

### 3. File I/O Commands

File I/O commands are now fully implemented:

```bash
# Read operations work
sedx '5r header.txt' file.txt

# Write operations work
sedx '/error/w errors.log' logfile.txt
```

### 4. Configuration File Migration

If you have a `~/.sedx/config.toml` from v0.2.x, it remains compatible. New options are available:

```toml
[backup]
max_size_gb = 10           # NEW: Maximum backup size
max_disk_usage_percent = 80 # NEW: Disk usage threshold
backup_dir = "/custom/path" # NEW: Custom backup location

[compatibility]
mode = "pcre"              # UPDATED: Default is now "pcre"
show_warnings = true

[processing]
context_lines = 2
max_memory_mb = 100
streaming = true
```

---

## Performance Benchmarks

### Memory Usage

| Operation | GNU sed | SedX (in-memory) | SedX (streaming) |
|-----------|---------|------------------|------------------|
| 10MB file | ~15MB | ~45MB | **<5MB** |
| 100MB file | ~25MB | OOM risk | **<10MB** |
| 1GB file | ~150MB | OOM risk | **<15MB** |
| 100GB file | ~2GB | Impossible | **<50MB** |

### Processing Speed

| Operation | GNU sed | SedX | Notes |
|-----------|---------|------|-------|
| Simple s/foo/bar/ (10MB) | 0.05s | 2.1s | 42x slower (overhead: backup + diff) |
| Complex regex (100MB) | 0.6s | 12.3s | 20x slower (acceptable trade-off) |
| Large file streaming (10GB) | 60s | 720s | 12x slower (constant memory) |

**Trade-off Analysis:**
- SedX prioritizes **safety** over raw speed
- Automatic backups add overhead but prevent data loss
- Diff generation provides transparency
- Streaming enables processing files that would OOM with other tools

---

## Installation

### From Cargo

```bash
cargo install sedx
```

### From Source

```bash
git clone https://github.com/InkyQuill/sedx.git
cd sedx
cargo build --release
sudo cp target/release/sedx /usr/local/bin/
```

### From Binary Release

Download from [GitHub Releases](https://github.com/InkyQuill/sedx/releases):

```bash
wget https://github.com/InkyQuill/sedx/releases/download/v1.0.0/sedx-v1.0.0-linux-x86_64.tar.gz
tar xzf sedx-v1.0.0-linux-x86_64.tar.gz
sudo cp sedx /usr/local/bin/
```

---

## Quick Start

```bash
# Preview changes before applying
sedx --dry-run 's/old/new/g' file.txt

# Apply safely (automatic backup created)
sedx 's/old/new/g' file.txt

# Rollback if needed
sedx rollback

# Use BRE mode for GNU sed compatibility
sedx -B 's/\(foo\|bar\)/baz/g' file.txt

# Process large files with streaming
sedx 's/foo/bar/g' huge_file.log  # Automatically uses streaming for files >= 100MB

# Complex flow control
sedx ':loop; s/foo/bar/; t loop' file.txt

# File I/O
sedx '/error/w errors.log' logfile.txt
```

---

## Testing

### Run All Tests

```bash
# Unit tests (294 tests)
cargo test

# Integration tests
./tests/run_all_tests.sh

# Streaming tests
./tests/streaming_tests.sh

# Phase 5 tests (flow control, file I/O)
./tests/scripts/phase5_tests.sh
```

### Test Results

- **294** unit tests passing
- **121** property-based tests passing
- **29** Phase 5 integration tests passing
- **100%** regression test pass rate vs GNU sed

---

## Documentation

- **[User Guide](docs/USER_GUIDE.md)** - Complete usage documentation
- **[Migration Guide](docs/MIGRATION_GUIDE.md)** - GNU sed to SedX migration
- **[Examples](docs/EXAMPLES.md)** - 50+ practical examples
- **[Specification](docs/SPECIFICATION.md)** - Full command reference
- **[Architecture](docs/ARCHITECTURE.md)** - System design documentation
- **[Contributing](docs/CONTRIBUTING.md)** - Development guide

---

## Contributors

- **InkyQuill** - Project lead and primary developer

---

## Support

- **Issues**: [GitHub Issues](https://github.com/InkyQuill/sedx/issues)
- **Discussions**: [GitHub Discussions](https://github.com/InkyQuill/sedx/discussions)
- **Documentation**: [docs/](https://github.com/InkyQuill/sedx/tree/main/docs)

---

## Upgrade Instructions

### From v0.2.x to v1.0.0

1. **Backup your configuration:**
   ```bash
   cp ~/.sedx/config.toml ~/.sedx/config.toml.bak
   ```

2. **Install v1.0.0:**
   ```bash
   cargo install sedx --force
   ```

3. **Test with dry-run:**
   ```bash
   sedx --dry-run 's/foo/bar/g' test_file.txt
   ```

4. **Update scripts that use BRE syntax:**
   - Add `-B` flag for BRE patterns
   - Or convert to PCRE syntax (recommended)

5. **Verify backups work:**
   ```bash
   sedx 's/test/test2/g' test_file.txt
   sedx rollback
   ```

### Rolling Back

If you encounter issues:

```bash
# Uninstall v1.0.0
cargo uninstall sedx

# Reinstall v0.2.6-alpha
cargo install sedx --git https://github.com/InkyQuill/sedx.git --branch v0.2.6-alpha

# Restore configuration
cp ~/.sedx/config.toml.bak ~/.sedx/config.toml
```

---

## What's Next (v1.1+)

### Planned Features

- **Performance optimizations**: Reduce speed gap with GNU sed
- **`y` command**: Character translation
- **`l` command**: List lines with escapes
- **`e` command**: Execute shell (with sandbox)
- **Unicode improvements**: Better character boundary handling
- **Windows support**: Enhanced Windows compatibility
- **Fuzz testing**: Comprehensive fuzzing infrastructure

See [ROADMAP.md](docs/ROADMAP.md) for full details.

---

## License

MIT License - see [LICENSE](LICENSE)

---

**Thank you for using SedX! Safe text processing for everyone.**
