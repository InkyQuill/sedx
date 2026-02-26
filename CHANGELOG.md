# Changelog

All notable changes to SedX are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-02-25

### Added
- **Flow control commands**: Labels (`:label`), unconditional branch (`b`), conditional branch (`t` - if substitution made), inverse branch (`T` - if NO substitution)
- **File I/O commands**: Read file (`r`), read line (`R`), write file (`w`), write first line (`W`)
- **Additional commands**: Print line number (`=`), print filename (`F`), clear pattern space (`z`)
- **Multi-line pattern space commands**: Next line (`n`), append next line (`N`), print first line (`P`), delete first line (`D`)
- **Quit without printing**: `Q` command
- **Script file support**: `-f`/`--file` flag for reading sed scripts from files
- **Multiple expressions**: `-e`/`--expression` flag for multiple sed expressions
- **Quiet mode**: `-n`/`--quiet`/`--silent` flag to suppress automatic output
- **Numbered substitution**: `s/old/new/N` flag to replace Nth occurrence only
- **Print-on-substitution**: `s/old/new/p` flag to print line if substitution was made
- **Escape sequences**: `\n`, `\t`, `\r`, `\\`, `\xHH`, `\uHHHH` in replacement strings
- **Streaming mode**: Process 100GB+ files with <100MB RAM
- **Sliding window diff**: Context-aware diffs for large files
- **Pattern ranges with state machine**: `/start/,/end/` with proper state tracking
- **Hold space operations**: `h`, `H`, `g`, `G`, `x` commands in streaming mode
- **Command grouping**: `{...}` with range support in streaming mode
- **Cycle-based execution**: Proper sed-like execution cycle for flow control
- **Comprehensive backup management**: `sedx backup` subcommands (list, show, restore, remove, prune)
- **Configuration file**: `~/.sedx/config.toml` for persistent settings
- **Disk space checking**: Pre-backup validation with configurable thresholds
- **Stdin/stdout pipeline support**: Full Unix pipeline compatibility

### Changed
- **Default regex flavor**: Now uses PCRE (Perl-Compatible Regular Expressions) instead of ERE
  - Groups: `(foo|bar)` instead of `\(foo\|bar\)` (BRE) or `(foo|bar)` (ERE)
  - Backreferences: `$1`, `$2` instead of `\1`, `\2` (use `-B` for BRE compatibility)
- **Architecture**: Migrated to unified cycle-based execution model
  - All commands now execute through proper sed cycles
  - Better compatibility with GNU sed for complex scripts
- **Backup optimization**: Skip backup creation for read-only commands

### Fixed
- Single-line address range bug (e.g., `2p` now works correctly)
- N command EOF handling (no extra newlines)
- P command newline handling (only prints when newline present)
- `-e` flag now properly handles file arguments
- `d` command now works without explicit address (defaults to all lines)

### Security
- Unsafe blocks in disk_space.rs now documented with safety comments

### Known Limitations
- `y` command (translate characters) - not implemented
- `l` command (list lines escaped) - not implemented
- `e` command (execute shell) - not implemented
- Unicode pattern matching has character boundary issues
- Pattern ranges with flow control commands (`/start/,/end/b`) not supported by parser

## [0.2.6-alpha] - 2026-01-10

### Added
- Cycle-based architecture for proper sed execution
- Full address resolution (line, pattern, range, negated, relative, step)
- Multi-line pattern space support (n, N, P, D commands)
- Q command (quit without printing)
- Script file parsing with shebang support

## [0.2.2-alpha] - 2026-01-10

### Added
- Phase 4 essential sed compatibility features
- `-n` flag (quiet mode)
- `-e` flag (multiple expressions)
- `-f` flag (script files)

## [0.2.1-alpha] - 2026-01-10

### Added
- CLI flags and backup management
- Disk space checking for backups
- Backup optimization for read-only commands

## [0.2.0-alpha] - 2026-01-10

### Added
- Phase 1 complete: Stream processing foundation
- 11 chunks of streaming implementation
- Constant memory processing for large files
- Sliding window diff generation

## [0.1.0] - Initial Release

### Added
- Basic sed commands: s (substitution), d (delete), a (append), i (insert), c (change), p (print), q (quit)
- Hold space operations: h, H, g, G, x
- Command grouping with {...}
- Address types: line numbers, patterns, $ (last line), negation with !
- Address ranges: /start/,/end/ and 1,10
- Automatic backups before modifications
- Dry-run mode (--dry-run)
- Interactive mode (--interactive)
- Rollback functionality (sedx rollback)
- Colored diffs
- Backup history (sedx history)
- Regex flavor support (PCRE, ERE, BRE)
- BRE to PCRE auto-conversion
