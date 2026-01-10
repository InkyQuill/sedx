# SedX Development Roadmap

**Version:** 0.2.0-alpha → 1.0.0
**Last Updated:** 2026-01-10
**Development Approach:** Incremental Releases with Comprehensive Testing

---

## 🎯 Project Vision

SedX is a **modern, safe text processing tool** that:
- Maintains high GNU sed compatibility (~95%)
- Uses **PCRE (modern regex) by default** with optional BRE/ERE modes
- Provides robust backup management with disk space awareness
- Uses stream processing for memory efficiency
- Always uses **sed syntax** (no sd-like simplified syntax)

**Target Users:**
- System administrators needing safer sed
- Developers wanting modern regex processing
- DevOps engineers requiring reliable automation
- Data scientists processing large files

---

## 📊 Current Status (v0.2.0-alpha - neo branch)

**Implemented:** 3,400+ lines, 11 modules
- ✅ 10/30 sed commands (33%)
- ✅ Basic backups (no disk checks)
- ✅ Dry-run & interactive modes
- ✅ Hold space operations (in-memory + streaming)
- ✅ **Unified Command System (UCS) parser**
- ✅ **Regex flavor support (PCRE/ERE/BRE)**
- ✅ **BRE to ERE auto-conversion**
- ✅ **Stdin/stdout pipeline support**
- ✅ **Streaming processing (chunks 1-10 completed)**
  - ✅ Basic infrastructure + atomic writes
  - ✅ Commands: s, d, p, a, i, c, q
  - ✅ Sliding window diff with context
  - ✅ Pattern ranges with state machine
  - ✅ Hold space operations (h, H, g, G, x)
  - ✅ Command grouping with ranges ({...})
  - ✅ Single-pattern address fix (/foo/d)
- ⏳ Comprehensive testing & optimization (chunk 11)
- ❌ No disk space checks
- ❌ Missing critical flags (-n, -e, -f)

**Recent Work (Completed 2026-01-10):**
- **Chunk 10 completed**: Command grouping in streaming mode
  - Full support for `{...}` command groups in streaming
  - Groups work with all range types: line ranges, pattern ranges, mixed ranges
  - Verified with extensive testing against GNU sed

- **Critical bug fix**: Single-pattern address handling
  - Fixed `/pattern/d` to match each line independently (not as a range)
  - Fixed `/pattern/s/foo/bar/` substitution behavior
  - All 10 regression tests now passing (was 9/10)

**Test Results:**
- ✅ 103 unit tests passing
- ✅ 10/10 regression tests passing (was 9/10 - fixed delete pattern bug)
- ⚠️ 13/20 hold space tests passing (7 edge cases with empty hold space)

**Streaming Progress:**
- Chunks 1-9: ✅ Completed (basic streaming through hold space)
- Chunk 10: ✅ Completed (command grouping with ranges)
- Chunk 11: ⏳ In Progress (comprehensive testing & optimization)

---

## 🗺️ Development Roadmap

### Phase 1: Stream Processing Foundation ⭐ CRITICAL

**Duration:** 3-4 weeks
**Target Release:** v0.2.0 (major refactoring)
**Priority:** HIGHEST (User requirement #1)

#### Goals
- Enable processing of 100GB+ files with <100MB RAM
- True sed-like stream behavior
- Foundation for all future features

#### Tasks

**Week 1: Core Stream Architecture**
- [ ] Refactor `file_processor.rs` to use `BufRead`
- [ ] Implement line-by-line processing
- [ ] Add sliding window for context tracking
- [ ] Create atomic file writes (tempfile + rename)
- [ ] Preserve backup system integration

**Week 2: Command Streaming**
- [ ] Stream-enable: `s` (substitution)
- [ ] Stream-enable: `d` (delete)
- [ ] Stream-enable: `a`, `i`, `c` (insert/append/change)
- [ ] Stream-enable: `p` (print)
- [ ] Stream-enable: `q` (quit)

**Week 3: Complex Operations**
- [x] Stream-enable pattern ranges with state machine
- [x] Stream-enable hold space operations
- [x] Stream-enable: `{}` (grouping) - Completed with full range support
- [ ] Stream-enable negation

**Week 4: Testing & Polish**
- [ ] Large file tests (1GB, 10GB, 100GB)
- [ ] Memory profiling (target: <100MB for 100GB file)
- [ ] Performance benchmarks vs GNU sed
- [ ] Edge case testing (empty files, single lines, binary)
- [ ] Documentation updates

#### Success Criteria
- [ ] Process 100GB file with <100MB RAM
- [ ] No performance regression vs current implementation
- [ ] All existing tests pass
- [ ] Backup system works with streaming
- [ ] Within 2x speed of GNU sed

#### Risks & Mitigations
- **Risk:** Breaking existing functionality
  - **Mitigation:** Comprehensive regression tests
- **Risk:** Diff generation complexity
  - **Mitigation:** Sliding window approach with configurable context

---

### Phase 2: Backup Disk Management 🛡️ CRITICAL

**Duration:** 2 weeks
**Target Release:** v0.2.1
**Priority:** HIGH (User requirement #2)

#### Goals
- Prevent disk space exhaustion
- User-friendly backup management
- Smart backup behavior

#### Tasks

**Week 1: Disk Space Checking**
- [ ] Implement cross-platform disk space checking
  - Linux: `statvfs`
  - macOS: `statvfs`
  - Windows: `GetDiskFreeSpaceEx`
- [ ] Add backup size estimation
- [ ] Implement warning thresholds:
  - ⚠️ Warn if backup > 2GB (configurable)
  - ⚠️ Warn if backup > 40% free space (configurable)
  - ❌ Error if backup > 60% free space (configurable)
  - ❌ Error if insufficient disk space
- [ ] Add `--no-backup` flag (requires `--force`)
- [ ] Add `--backup-dir` flag

**Week 2: Backup Management**
- [ ] Add `sedx config` command (opens $EDITOR, validates syntax)
- [ ] Implement backup subcommands:
  - `sedx backup list` [-v, --verbose]
  - `sedx backup show <id>`
  - `sedx backup restore <id>`
  - `sedx backup remove <id>` [--force]
  - `sedx backup prune` [--keep=N] [--keep-days=N]
- [ ] Create `~/.sedx/config.toml` structure
- [ ] Add configuration validation
- [ ] Implement config settings:
  - `[backup] max_size_gb`, `max_disk_usage_percent`
  - `[compatibility] mode`, `show_warnings`
  - `[processing] context_lines`, `max_memory_mb`

#### Success Criteria
- [ ] Never silently fill disk
- [ ] All backup operations manageable via CLI
- [ ] Config file editable via `sedx config` command
- [ ] Clear user communication

#### Example Usage
```bash
# Large backup warning
$ sedx --execute 's/foo/bar/' hugefile.bin
warning: This operation will create a large backup (3.7 GB)
prompt: Continue? [y/N] y

# Low disk space error
$ sedx --execute 's/foo/bar/' file.txt
error: Insufficient disk space for backup
backup partition: /home (15.2 GB free)
backup required: 10.1 GB (would use 66% of free space)
options:
  1. Remove old backups: sedx backup prune --keep=5
  2. Use different location: --backup-dir /mnt/backups
  3. Skip backup: --no-backup --force (not recommended)

# Edit configuration
$ sedx config
# Opens $EDITOR on ~/.sedx/config.toml
# Validates syntax on save
```

---

### Phase 3: Enhanced Regex & Substitution Features 🔄

**Duration:** 3 weeks
**Target Release:** v0.3.0
**Priority:** MEDIUM (User requirement #4)

#### Goals
- Leverage modern PCRE regex capabilities
- Add convenience features while maintaining sed compatibility
- Enhance substitution flags and options
- Improve regex error messages and validation

#### Tasks

**Week 1: PCRE Enhancements**
- [ ] Implement PCRE-specific features:
  - Named capture groups: `(?P<name>...)`
  - Non-capturing groups: `(?:...)`
  - Lookaheads: `(?=...)`, `(?!...)`
  - Lookbehinds: `(?<=...)`, `(?<!...)`
  - Atomic groups: `(?>...)`
  - Possessive quantifiers: `?+`, `*+`, `++`
- [ ] Add regex flag overrides in patterns: `(?i)`, `(?m)`, `(?s)`
- [ ] Implement `-X`/`--pcre-only` flag (require PCRE syntax only)
- [ ] Add regex validation and helpful error messages

**Week 2: Enhanced Features**
- [ ] Implement `--max-count`/`--max-replacements` flag
- [ ] Add numbered substitution flag (`s/old/new/2`)
- [ ] Implement print-on-substitution flag (`s/old/new/p`)
- [ ] Add capture group validation:
  - Detect `$1foo` → suggest `${1}foo`
  - Validate capture group references
  - Helpful error messages
- [ ] Support modern capture syntax (`$1`, `$2`, `${name}`)
- [ ] Keep `\1`, `\2` for sed compatibility (convert internally when using `-B`)

**Week 3: Escape Sequences & Testing**
- [ ] Add escape sequences in replacements:
  - `\n`, `\t`, `\r`, `\\`
  - `\xHH`, `\uHHHH`, `\U{HHHHHH}`
- [ ] Add escape sequences in patterns (PCRE mode):
  - `\a`, `\b`, `\f`, `\v`
  - `\e` (escape), `\0` (null)
- [ ] Comprehensive testing:
  - Regression tests vs GNU sed (BRE/ERE modes)
  - PCRE feature tests
  - Unit tests for parser
  - Integration tests

#### Success Criteria
- [ ] All PCRE features work correctly
- [ ] Backward compatible with GNU sed in BRE/ERE modes
- [ ] Clear error messages for invalid regex patterns
- [ ] Capture group validation prevents common errors

#### Example Usage
```bash
# Modern PCRE features (default)
$ sedx 's/(?P<word>\w+)/<\1>/g' file.txt  # Named groups
$ sedx 's/foo(?=bar)/FOO/g' file.txt     # Lookahead

# BRE mode (GNU sed compatible)
$ sedx -B 's/\(foo\|bar\)/BAZ/g' file.txt  # Auto-converts to (foo|bar)

# ERE mode (sed -E compatible)
$ sedx -E 's/(foo|bar)/BAZ/g' file.txt

# Numbered substitution
$ sedx 's/foo/bar/2' file.txt  # Replace only 2nd occurrence

# Capture group validation
$ sedx 's/(\d+)/$1user/' file.txt
error: Ambiguous capture reference: $1user
hint: Use ${1}user to disambiguate: s/(\d+)/${1}user/
```

---

### Phase 4: Essential Sed Compatibility 📝

**Duration:** 4 weeks
**Target Release:** v0.4.0
**Priority:** HIGH (User requirement #3)

#### Goals
- Implement Tier 1 missing commands
- Add critical CLI flags
- Reach 80% sed compatibility

#### Tasks

**Week 1: Core Flags**
- [ ] Implement `-n`/`--quiet`/`--silent` flag (suppress output)
- [ ] Implement `-e`/`--expression` flag (multiple expressions)
- [ ] Implement `--execute` flag (apply changes, current default)
- [ ] Add `--stdout` flag (print to stdout, no backup)
- [ ] Update command routing logic

**Week 2: Next Line Operations**
- [ ] Implement `n` command (print, read next, start new cycle)
- [ ] Implement `N` command (append newline + next line)
- [ ] Implement `P` command (print first line of pattern space)
- [ ] Implement `D` command (delete first line, restart cycle)
- [ ] Add multi-line pattern space support

**Week 3: Additional Commands**
- [ ] Implement `Q` command (quit without printing)
- [ ] Add command: `-f`/`--file` flag (script from file)
- [ ] Implement script file parser
- [ ] Support shebang: `#!/usr/bin/sedx -f`

**Week 4: Testing**
- [ ] Comprehensive regression tests vs GNU sed
- [ ] Multi-line operation tests
- [ ] Script file tests
- [ ] Edge cases (empty files, huge lines, EOF handling)

#### Success Criteria
- [ ] 80% of common sed scripts work unmodified
- [ ] All Tier 1 commands implemented
- [ ] No regressions in existing functionality

#### Example Usage
```bash
# Suppress automatic output
$ sedx -n '1,10p' file.txt  # Only print lines 1-10

# Multiple expressions
$ sedx -e 's/foo/bar/' -e 's/baz/qux/' file.txt

# Next line operations
$ seq 1 5 | sedx 'n; d'  # Print 1, 3, 5
$ printf "a\nb\nc" | sedx 'N; s/\n/ /'  # "a b\nc"

# Script file
$ cat script.sed
#!/usr/bin/sedx -f
s/foo/bar/g
5,10d

$ sedx -f script.sed file.txt

# Quit without printing
$ sedx '/error/Q' file.txt  # Quit on first error, don't print
```

---

### Phase 5: Flow Control & Advanced Features 🔀

**Duration:** 4 weeks
**Target Release:** v0.5.0
**Priority:** MEDIUM (User requirement #5)

#### Goals
- Implement flow control commands
- Add file I/O operations
- Enable complex sed scripts

#### Tasks

**Week 1: Labels & Branching**
- [ ] Implement `:label` command
- [ ] Implement `b` command (branch to label)
- [ ] Implement `b` without label (branch to end)
- [ ] Add label registry during parsing
- [ ] Implement program counter in execution

**Week 2: Test Branching**
- [ ] Track substitution flag state
- [ ] Implement `t` command (branch if substitution made)
- [ ] Implement `T` command (branch if NO substitution)
- [ ] Reset flag after test
- [ ] Add state management

**Week 3: File I/O**
- [ ] Implement `r file` command (read file)
- [ ] Implement `w file` command (write to file)
- [ ] Add file handle management
- [ ] Implement `R file` (read one line)
- [ ] Implement `W file` (write first line)
- [ ] Add sandbox mode (`--sandbox` disables e/r/w)

**Week 4: Additional Commands & Testing**
- [ ] Implement `=` command (print line number)
- [ ] Implement `F` command (print filename)
- [ ] Implement `z` command (clear pattern space)
- [ ] Comprehensive flow control tests
- [ ] File I/O tests
- [ ] Security audits for file operations

#### Success Criteria
- [ ] 95% of sed scripts work unmodified
- [ ] All Tier 2 commands implemented
- [ ] Flow control works correctly
- [ ] File operations safe

#### Example Usage
```bash
# Loop until pattern matches
$ sedx ':top; /found/q; n; b top' file.txt

# Repeat substitution until no more matches
$ sedx ':loop; s/foo/bar/; t loop' file.txt

# Read/Write files
$ sedx '5r header.txt' file.txt
$ sedx '/error/w errors.log' file.txt

# Flow control
$ seq 1 10 | sedx ':a; ta; s/[0-9]/X/; ba'

# Print line numbers
$ sedx '=' file.txt | sedx 'N; n'
```

---

### Phase 6: Advanced Addressing & Polish ✨

**Duration:** 3 weeks
**Target Release:** v0.6.0
**Priority:** LOW-MEDIUM

#### Goals
- Add advanced addressing modes
- Implement remaining commands
- Performance optimization

#### Tasks

**Week 1: Advanced Addressing**
- [ ] Implement stepping addresses (`first~step`)
- [ ] Implement relative ranges (`addr,+N`)
- [ ] Implement special address `0` (first line)
- [ ] Add address validation

**Week 2: Additional Commands**
- [ ] Implement `y` command (translate characters)
- [ ] Implement `l` command (list with escapes)
- [ ] Add `-l N` flag (line length for `l`)
- [ ] Implement `e` command (execute shell) with sandbox

**Week 3: Optimization & Polish**
- [ ] Regex caching and optimization
- [ ] Memory-mapped files for binary
- [ ] Parallel processing with Rayon (multi-file)
- [ ] Performance benchmarks
- [ ] Documentation completion

#### Success Criteria
- [ ] All addressing modes work
- [ ] Tier 3 commands implemented
- [ ] Within 2x speed of GNU sed
- [ ] Documentation complete

---

### Phase 7: Production Hardening 🚀

**Duration:** 2 weeks
**Target Release:** v1.0.0
**Priority:** CRITICAL

#### Goals
- Production-ready stability
- Complete documentation
- Full test coverage

#### Tasks

**Week 1: Testing**
- [ ] Property-based tests (proptest)
- [ ] Fuzz testing for parser
- [ ] Large file tests (100GB+)
- [ ] Memory leak detection
- [ ] Stress testing

**Week 2: Documentation & Release**
- [ ] Complete SPECIFICATION.md
- [ ] Write tutorial
- [ ] Create migration guide (sed → sedx)
- [ ] Add shell completions (bash, zsh, fish, powershell)
- [ ] Prepare v1.0.0 release
- [ ] Update README, man pages
- [ ] Create announcement

#### Success Criteria
- [ ] 95%+ test coverage
- [ ] Zero known critical bugs
- [ ] Complete documentation
- [ ] Successful beta testing

---

## 📅 Release Timeline

| Version | Date | Features | Stability |
|---------|------|----------|-----------|
| **v0.1.0** | Past | Basic sed commands, in-memory | Alpha |
| **v0.2.0-alpha** | Current | **Stream processing (chunks 1-8)** | Alpha |
| **v0.2.0** | Week 5 | **Stream processing (all chunks)** | Beta |
| **v0.2.1** | Week 6 | Backup disk management | Beta |
| **v0.3.0** | Week 9 | Enhanced substitution | Beta |
| **v0.4.0** | Week 13 | Essential sed compatibility | Beta |
| **v0.5.0** | Week 17 | Flow control & file I/O | Beta |
| **v0.6.0** | Week 20 | Advanced addressing & polish | RC |
| **v1.0.0** | Week 22 | Production-ready | Stable |

**Total Duration:** ~5-6 months (22 weeks)

---

## 🎯 Success Metrics

### Functionality
- [ ] 95% compatibility with GNU sed for common operations
- [ ] Full PCRE support with modern regex features
- [ ] Stream processing handles 100GB+ files with <100MB RAM
- [ ] Backup system prevents all silent data loss

### Performance
- [ ] Within 2x speed of GNU sed for typical operations
- [ ] Memory usage constant regardless of file size
- [ ] No memory leaks in long-running processes

### Reliability
- [ ] Zero silent disk space exhaustion
- [ ] Zero data loss incidents (with backups enabled)
- [ ] All operations recoverable via rollback
- [ ] Comprehensive test coverage (>90%)

### Usability
- [ ] Clear incompatibility warnings
- [ ] Helpful error messages
- [ ] Intuitive backup management
- [ ] Complete documentation

---

## 🔄 Iteration Process

### Sprint Structure (2-3 weeks each)

1. **Planning (1 day)**
   - Review tasks for sprint
   - Estimate effort
   - Identify dependencies

2. **Development (80% of sprint)**
   - Implement features
   - Write tests alongside code
   - Update documentation

3. **Testing & Review (15% of sprint)**
   - Run comprehensive tests
   - Performance benchmarks
   - Code review

4. **Release (5% of sprint)**
   - Tag release
   - Update CHANGELOG
   - Create announcement

### Continuous Integration

- Run tests on every commit
- Benchmark performance weekly
- Regression testing before releases
- Beta testing for major versions

---

## 🚧 Risks & Mitigations

### Technical Risks

**Risk: Stream processing breaks existing functionality**
- **Probability:** Medium
- **Impact:** High
- **Mitigation:** Comprehensive regression tests, gradual migration

**Risk: Performance regression**
- **Probability:** Medium
- **Impact:** Medium
- **Mitigation:** Regular benchmarks, performance targets

**Risk: Cross-platform issues**
- **Probability:** Low
- **Impact:** Medium
- **Mitigation:** Test on Linux, macOS, Windows regularly

### Project Risks

**Risk: Scope creep**
- **Probability:** Medium
- **Impact:** Medium
- **Mitigation:** Clear priorities, phased approach

**Risk: User adoption**
- **Probability:** Low
- **Impact:** High
- **Mitigation:** Documentation, examples, migration guide

---

## 📚 Documentation Plan

### User Documentation
- [ ] Tutorial (interactive)
- [ ] Quick reference guide
- [ ] Migration guide (sed → sedx)
- [ ] Examples cookbook
- [ ] FAQ

### Developer Documentation
- [ ] Architecture overview
- [ ] Contributing guide
- [ ] Code style guide
- [ ] Testing guide
- [ ] Release process

---

## 🤝 Community & Feedback

### Feedback Channels
- GitHub Issues (bug reports, feature requests)
- GitHub Discussions (usage questions, ideas)
- Regular releases with changelogs

### Contribution Guidelines
- Bug fixes welcome anytime
- Features should be discussed first
- All contributions require tests
- Code review required

---

## 🎉 Conclusion

This roadmap delivers a **production-ready SedX** in ~5-6 months through incremental releases:

1. **Stream processing** (foundation for large files)
2. **Backup management** (safety first)
3. **Enhanced regex features** (modern PCRE with sed compatibility)
4. **Sed compatibility** (Tier 1 + Tier 2 commands)
5. **Flow control** (complex scripts)
6. **Advanced features** (addressing, polish)
7. **Production hardening** (stability & docs)

**Key Principles:**
- ✅ Stream processing first (architectural foundation)
- ✅ Incremental releases (regular feedback)
- ✅ Comprehensive testing (per sprint)
- ✅ Modern regex by default (PCRE) with sed compatibility modes
- ✅ Unified command system (clean internal architecture)
- ✅ Safety features (backups, disk checks)
- ✅ Clear communication (warnings, docs)

The result: a **modern, safe, powerful** text processing tool that maintains GNU sed compatibility while providing modern regex capabilities.
