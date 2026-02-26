# SedX Production Roadmap: v0.2.6-alpha to v1.0.0

**Last Updated:** 2026-02-25
**Current Branch:** neo
**Current Version:** v0.2.6-alpha
**Current State:** ~9,600 LOC, 13 modules, 121 unit tests passing

---

## Executive Summary

SedX has completed Phases 1-5 with substantial feature completeness. This roadmap focuses on **production readiness** rather than feature expansion. The path to v1.0.0 prioritizes stability, polish, and documentation over implementing remaining GNU sed commands.

**Current Status:**
- 95% GNU sed compatibility achieved for common operations
- Core architecture (streaming, backups, regex) is solid
- Flow control (b, t, T), file I/O (r, R, w, W), and additional commands (=, F, z) implemented
- Known issues include Unicode handling, some edge cases
- Code has technical debt (TODOs, large files, compiler warnings)

---

## v1.0.0 Release Criteria

### Must-Have (Blocking)

- [ ] Zero critical bugs (data loss or crashes)
- [ ] All known security vulnerabilities addressed
- [ ] 80%+ test coverage
- [ ] Complete user documentation
- [ ] Man page complete and installed
- [ ] Shell completions provided
- [ ] Migration guide from GNU sed
- [ ] Clear documentation of limitations
- [ ] All Phase 1-4 cleanup tasks complete

### Should-Have (Strongly Recommended)

- [ ] Unicode pattern matching fixed
- [ ] Property tests for core operations
- [ ] Performance benchmarks documented
- [ ] Real-world examples in docs
- [ ] Contribution guide complete

### Nice-to-Have (Can Defer to v1.1)

- [ ] Additional sed commands (y, l, e)
- [ ] Performance optimizations
- [ ] Windows support improvements
- [ ] Fuzz testing infrastructure

---

## Phase 1: Code Cleanup & Stabilization

**Duration:** 2 weeks
**Priority:** CRITICAL
**Target Release:** v0.3.0-beta

### Goals
- Remove technical debt
- Eliminate compiler warnings
- Fix known issues affecting stability

### Tasks

#### 1.1 Fix Compiler Warnings (1 day)

**Files with warnings:**
- `src/file_processor.rs`:
  - Line 5: Unused import `PathBuf`
  - Line 9: Unused import `HashSet`
  - Line 1499: Unused variable `address_matches`
  - Line 2768: Unused variable `start_idx`
  - Line 2914-2956: Unused `range` parameters

- `src/main.rs`:
  - Line 21: Unused import `Path`
  - Line 636: Unused variable `idx`
  - Line 737: Unused import `save_config`

- `src/backup_manager.rs`:
  - Line 71: Unused constant `WARN_PERCENT`

- `src/config.rs`:
  - Line 10: Unused constant `DEFAULT_CONFIG`
  - Lines 1-5: Fix module doc comment (`///` → `//!`)

- `src/disk_space.rs`:
  - Lines 1-5: Fix module doc comment (`///` → `//!`)

**Action:** Run `cargo clippy -- -D warnings` and fix all issues.

#### 1.2 Document Unsafe Blocks (1 day)

**File:** `src/disk_space.rs:74-84`

Add safety documentation:
```rust
/// # Safety
///
/// The `c_path` pointer is valid because it comes from a `CString` whose lifetime
/// exceeds this function call. The `statvfs` call is a standard POSIX system call
/// that writes to a valid mutable reference. Return value is checked for errors.
```

#### 1.3 Replace Unsafe unwrap() Calls (2 days)

**Files to fix:**
- `src/sed_parser.rs:308`: `trimmed.chars().nth(pos).unwrap()`
- `src/parser.rs:268`: `flag.to_digit(10).unwrap()`

**Action:** Replace with `ok_or_else()` and proper error handling.

#### 1.4 Resolve TODO Comments (3 days)

**File:** `src/file_processor.rs`
- Line 1753: Pattern range state tracking
- Line 1762: Mixed range state tracking
- Line 2063: Port all commands to cycle model
- Line 2938: R command EOF handling
- Line 2961: D command restart cycle
- Line 3691: Proper parser for file reads

**Action:** Complete implementation or document as known limitation.

#### 1.5 Fix Unicode Pattern Matching (3 days)

**Issue:** Byte index panic on Unicode boundaries.

**File:** `src/sed_parser.rs:297`

**Action:** Use char indices instead of byte indices. Add Unicode test cases.

#### 1.6 Remove Dead Code (1 day)

**Files to clean:**
- `src/bre_converter.rs:104`: `is_bre_pattern()` - never used
- `src/capability.rs:32`: Functions never called
- `src/diff_formatter.rs`: Unused format methods

**Action:** Remove or mark with `#[allow(dead_code)]`.

**Success Criteria:** `cargo clippy` produces zero warnings, all TODOs resolved.

---

## Phase 2: Testing & Coverage

**Duration:** 2 weeks
**Priority:** HIGH
**Target Release:** v0.4.0-beta

### Goals
- Achieve 80% test coverage
- Add property-based testing
- Implement stress testing

### Tasks

#### 2.1 Coverage Analysis & Gap Filling (1 week)

**Run coverage:**
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

**Target:** 80% line coverage (current estimated: ~60%)

**Add missing tests for:**
- `src/backup_manager.rs` - Error paths, edge cases
- `src/config.rs` - Corruption handling, validation
- `src/disk_space.rs` - Edge cases, cross-platform
- `src/bre_converter.rs` - Conversion edge cases
- `src/ere_converter.rs` - Backreference handling

**Test scenarios to add:**
- Regex compilation failures
- Invalid command syntax
- Malformed configuration files
- Disk space exhaustion
- Permission denied errors
- Lock conflicts

#### 2.2 Property-Based Testing (3 days)

**Add to Cargo.toml:**
```toml
[dev-dependencies]
proptest = "1.0"
```

**Property tests:**
1. Round-trip: parse → process → output matches expected
2. Streaming == in-memory for supported commands
3. Backup can always restore exactly
4. Dry-run == execute (output matches, no file mod)

#### 2.3 Stress Testing (4 days)

**Test scenarios:**
1. Large file processing (create 10GB test file)
2. Many small files (10,000 files in batch)
3. Deeply nested flow control (1,000 branches)
4. Complex regex patterns (100 groups)
5. Unicode edge cases (emoji, combining marks, RTL)

**Success Criteria:** 80%+ coverage, property tests pass, stress tests stable.

---

## Phase 3: Error Handling & User Experience

**Duration:** 1 week
**Priority:** HIGH
**Target Release:** v0.5.0-beta

### Goals
- Clear, actionable error messages
- Graceful degradation
- Consistent UX

### Tasks

#### 3.1 Parser Error Improvements (2 days)

**File:** `src/sed_parser.rs`

**Improvements:**
- Show context around error position
- Suggest corrections for common mistakes
- Example: "Unknown command '2i INSERTED LINE' - text must use '2i\\INSERTED LINE'"

#### 3.2 Regex Error Handling (2 days)

**File:** `src/file_processor.rs`

**Improvements:**
- Capture and explain regex compilation errors
- Distinguish invalid regex from PCRE vs BRE/ERE issues
- Suggest pattern fixes

#### 3.3 File Operation Error Messages (1 day)

**Scenarios:**
- Permission denied: Show path and suggest fix
- Disk space: Show available vs required
- Lock conflicts: Suggest --no-backup or retry

#### 3.4 Warning Consistency (2 days)

**Review all warnings for:**
- Consistent phrasing
- Actionable suggestions
- Clear severity levels

**Success Criteria:** All error messages actionable, zero silent failures.

---

## Phase 4: Documentation

**Duration:** 2 weeks
**Priority:** HIGH
**Target Release:** v0.6.0-beta

### Goals
- Complete user-facing documentation
- Comprehensive migration guide
- Man pages and shell completions

### Tasks

#### 4.1 User Documentation (1 week)

**Create:**
1. **`docs/USER_GUIDE.md`**
   - Installation (cargo, binary, package managers)
   - Quick start tutorial
   - Common use cases with examples
   - Backup system explanation
   - Configuration guide

2. **`docs/MIGRATION_GUIDE.md`**
   - Regex syntax differences (PCRE vs BRE/ERE)
   - Command compatibility matrix
   - Flag differences
   - Common migration patterns
   - Breaking changes and workarounds

3. **`docs/EXAMPLES.md`**
   - System administration tasks
   - Development workflows
   - Data processing patterns
   - 50+ real-world examples

4. **Update `README.md`**
   - Simplify for new users
   - Link to detailed guides
   - Performance characteristics
   - Current limitations

#### 4.2 Reference Documentation (3 days)

**Create/Update:**
1. **`docs/CONTRIBUTING.md`** - Expand
   - Development setup
   - Code organization
   - Testing strategy
   - PR guidelines

2. **`docs/ARCHITECTURE.md`** - New
   - Module interactions
   - Data flow diagrams
   - Streaming architecture
   - Backup system design

3. **`man/sedx.1`** - Man page
   - All commands documented
   - Examples for each
   - All flags documented
   - Exit codes

#### 4.3 Shell Completions (4 days)

**Create:**
- `completions/bash.sedx`
- `completions/zsh.sedx`
- `completions/fish.sedx`
- `completions/powershell.sedx`

**Installation:**
- Add to README
- Package in releases
- Auto-install via makefile

**Success Criteria:** New users can onboard without external help.

---

## Phase 5: Final Polish & Release Preparation

**Duration:** 1 week
**Priority:** CRITICAL
**Target Release:** v1.0.0

### Goals
- Release preparation
- Final testing
- Announcement

### Tasks

#### 5.1 Release Preparation (2 days)

**Tasks:**
- Update `CHANGELOG.md` with all changes
- Tag v1.0.0-rc1
- Create release notes
- Prepare GitHub release

#### 5.2 Final Testing (2 days)

**Tasks:**
- Run full test suite on Linux, macOS, Windows
- Test installation from source
- Test binary distribution
- Verify all documentation examples

#### 5.3 Announcement (1 day)

**Tasks:**
- Write announcement blog post
- Update website (if applicable)
- Prepare social media
- Notify relevant communities

#### 5.4 Release (2 days)

**Tasks:**
- Build release binaries (Linux x64, macOS ARM64/x64, Windows x64)
- Upload to GitHub Releases
- Publish to crates.io
- Update documentation links

**Success Criteria:** v1.0.0 released with all criteria met.

---

## Release Timeline

| Milestone | Target | Duration | Dependencies |
|-----------|--------|----------|--------------|
| v0.3.0-beta | Week 2 | 2 weeks | Code cleanup |
| v0.4.0-beta | Week 4 | 2 weeks | Testing coverage |
| v0.5.0-beta | Week 5 | 1 week | Error handling |
| v0.6.0-beta | Week 7 | 2 weeks | Documentation |
| **v1.0.0-rc1** | Week 8 | 1 week | Release prep |
| **v1.0.0** | Week 9 | 1 week | Final polish |

**Total Time to v1.0.0:** 9 weeks

---

## Post-v1.0.0 Roadmap

### v1.1 - Feature Completion (4-6 weeks)
- Performance optimizations
- Additional commands (y, l, e)
- Windows improvements
- Enhanced error messages

### v1.2 - Enhanced Features (4-6 weeks)
- Interactive mode improvements
- Backup compression options
- Additional PCRE features
- Fuzz testing infrastructure

### v2.0 - Major Enhancements (Future)
- Script language extensions
- Visual diff mode
- Parallel file processing
- Plugin system

---

## Success Metrics

### Functional
- 95% compatibility with GNU sed for common operations
- All documented features work as described
- Zero silent data loss scenarios

### Quality
- 80%+ test coverage
- Zero critical bugs
- Zero known security vulnerabilities

### Usability
- Clear error messages
- Complete documentation
- Migration path from sed

### Stability
- Passes stress tests
- No memory leaks
- Handles 100GB files with <100MB RAM
