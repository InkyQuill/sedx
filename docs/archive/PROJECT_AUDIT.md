# SedX Project Audit & Critical Decisions

**Date:** 2025-01-07
**Current Version:** 0.2.0-alpha (neo branch)
**Total Code:** ~3,200 lines (6 modules + streaming infrastructure)

---

## 📊 Current Implementation Analysis

### Architecture

```
src/
├── main.rs              (186 lines) - Entry point, command routing
├── cli.rs               (164 lines) - CLI parsing with clap
├── sed_parser.rs        (600+ lines) - Sed expression parser
├── file_processor.rs    (772 lines) - File processing (IN-MEMORY)
├── backup_manager.rs    (168 lines) - Backup system (no disk checks)
└── diff_formatter.rs    (~400 lines) - Diff output formatting
```

### ✅ Currently Implemented (Working)

**Sed Commands (10/30 GNU sed commands = 33%):**
- ✅ `s` - Substitution (with flags: `g`, `i`, `p`)
- ✅ `d` - Delete
- ✅ `a` - Append
- ✅ `i` - Insert
- ✅ `c` - Change
- ✅ `p` - Print
- ✅ `q` - Quit
- ✅ `{}` - Command grouping
- ✅ `h`, `H`, `g`, `G`, `x` - Hold space operations

**Address Types:**
- ✅ Line numbers: `5`, `10`
- ✅ Patterns: `/foo/`
- ✅ Last line: `$`
- ✅ Ranges: `1,10`, `/start/,/end/`
- ✅ Negation: `!`, `/pattern/!`

**Modern Features:**
- ✅ Dry-run mode (`--dry-run`)
- ✅ Interactive mode (`--interactive`)
- ✅ Automatic backups
- ✅ Rollback functionality
- ✅ Colored diffs
- ✅ Context control
- ✅ Extended regex by default

### ❌ Critical Gaps (Missing)

**1. Memory Architecture**
- ❌ **Loads ENTIRE file into memory** (line 70-73 in file_processor.rs)
- ❌ No stream processing
- ❌ Cannot handle large files (> available RAM)
- **Impact:** CANNOT process files larger than available memory

**2. Sed Compatibility**
- ❌ No `-n` flag (suppress automatic output)
- ❌ No `-e` flag (multiple expressions)
- ❌ No `-f` flag (script files)
- ❌ Missing commands: `n`, `N`, `P`, `D`, `Q`, `l`, `r`, `w`, `F`, `y`, `=`, `z`, `e`
- ❌ No flow control: `:` (labels), `b` (branch), `t`, `T` (test)

**3. Backup System**
- ❌ **No disk space checking** before creating backup
- ❌ No backup size warnings
- ❌ No `--no-backup` flag
- ❌ No backup management commands (list, show, remove, prune)
- ❌ No compression
- **Risk:** Can silently fill disk, cause system crash

**4. Substitution Features**
- ❌ No simplified syntax (`'old' 'new'` like sd)
- ❌ No string-literal mode (`-F`)
- ❌ No replacement limiting (`--max-count`)
- ❌ No numbered substitution flag (`s/old/new/2`)
- ❌ No capture group validation
- ❌ No modern replacement syntax hints

**5. Advanced Addressing**
- ❌ No stepping (`1~2`)
- ❌ No relative ranges (`/start/,+5`)
- ❌ No special address `0` (first line before any processing)

**6. CLI Options**
- ❌ No `--execute` flag (currently dry-run is default)
- ❌ No `--stdout` flag (print to stdout, no backup)
- ❌ No `--no-backup` flag
- ❌ No `--compat` modes

---

## 🔄 Compatibility Analysis with Plans

### IMPROVEMENTS.md Compatibility

| Feature | IMPROVEMENTS.md | Current Status | Compatibility |
|---------|----------------|----------------|---------------|
| **Stream Processing** | ❌ Not mentioned | ❌ In-memory only | **CONFLICT** |
| **Disk Checks** | ❌ Not mentioned | ❌ Not implemented | **GAP** |
| **Sed Commands** | Phase 1-4 | 33% complete | **PARTIAL** |
| **Simplified Syntax** | ❌ Not mentioned | ❌ Not implemented | **GAP** |
| **Backup Management** | ❌ Not mentioned | Basic only | **GAP** |

**Key Insight:** IMPROVEMENTS.md focuses on GNU sed parity but **misses critical architectural requirements** (stream processing, disk management).

### ROADMAP.md Compatibility

| Requirement | ROADMAP.md | Current Status | Compatibility |
|-------------|------------|----------------|---------------|
| **Stream Processing** | ✅ Phase 1 (Critical) | ❌ In-memory | **MAJOR REFACTOR** |
| **Enhanced Substitution** | ✅ Phase 2 | ❌ Basic only | **ADD** |
| **Disk Space Mgmt** | ✅ Phase 3 | ❌ Not implemented | **ADD** |
| **Compatibility System** | ✅ Phase 4 | ❌ Not implemented | **ADD** |
| **Simplified Syntax** | ✅ Phase 2 | ❌ Not implemented | **ADD** |

**Key Insight:** ROADMAP.md aligns with user requirements but requires **significant architectural changes**.

---

## 🚨 Critical Issues to Resolve

### Issue 1: Memory Architecture (BLOCKER)

**Current:**
```rust
// file_processor.rs:70-73
let content = fs::read_to_string(file_path)?;
let original_lines: Vec<&str> = content.lines().collect();
let mut modified_lines: Vec<String> = original_lines.iter().map(|s| s.to_string()).collect();
```

**Problem:**
- Loads entire file into RAM
- 10GB file = 10GB+ RAM usage
- **Cannot process files > available memory**

**Required Change:**
```rust
// Stream processing with BufRead
let file = File::open(file_path)?;
let reader = BufReader::new(file);
for line in reader.lines() {
    // Process line-by-line
}
```

**Impact:**
- 🔴 **MAJOR REFACTOR** of `file_processor.rs`
- 🔴 Affects diff generation (need sliding window)
- 🔴 Affects hold space operations (need redesign)
- 🔴 Affects pattern ranges (need state machine)

---

### Issue 2: Backup System Risk (CRITICAL)

**Current:**
```rust
// backup_manager.rs:40-87
pub fn create_backup(&mut self, expression: &str, files: &[PathBuf]) -> Result<String> {
    // No disk space checking!
    fs::copy(file_path, &backup_path)?;  // Can fail silently
}
```

**Problem:**
- No disk space check before backup
- Can fill disk, cause system crash
- No warning if backup >2GB
- No `--no-backup` option for emergencies

**Required Change:**
1. Check free space before backup
2. Calculate backup size estimate
3. Warn if backup >2GB
4. Error if backup >60% free space
5. Add `--no-backup` flag
6. Add backup management commands

---

### Issue 3: Sed Compatibility Gap (HIGH PRIORITY)

**Missing Critical Features:**
- `-n` flag (suppress output) - **Used in 80%+ sed scripts**
- `-e` flag (multiple expressions) - **Essential for complex scripts**
- `-f` flag (script files) - **Common for large scripts**
- Flow control (`:`, `b`, `t`, `T`) - **Needed for complex logic**
- Next line operations (`n`, `N`) - **Multi-line processing**

**Impact:**
- Many sed scripts won't work
- Cannot use sedx as drop-in replacement
- Limited to simple operations

---

## 🤔 Critical Questions for User Decision

These decisions will shape the final roadmap. Please answer thoughtfully.

---

### Question 1: Project Identity & Positioning

**What is SedX's PRIMARY purpose?**

**Option A: GNU Sed Replacement (Compatibility First)**
- Goal: 95%+ sed compatibility
- Accept traditional sed syntax only
- Add safety features (backups, diffs) around sed
- Target: Users want drop-in sed replacement
- Example: `sedx 's/foo/bar/g' file.txt`

**Option B: Modern Text Processing Tool (Usability First)**
- Goal: Better than sed for 90% of use cases
- Accept simplified syntax: `sedx 'foo' 'bar' file.txt`
- Add convenience features from `sd`
- Warn about incompatibility
- Target: Users want easier, safer tool
- Example: `sedx 'foo' 'bar' file.txt` with warnings

**Option C: Hybrid (Two Modes)**
- `sedx --sed-compatible 's/foo/bar/g'` (strict)
- `sedx 'foo' 'bar' file.txt` (modern, default)
- Accept both syntaxes
- Warn about incompatibilities
- Target: Best of both worlds

**My Recommendation:** **Option C (Hybrid)**
- Provides flexibility
- Eases transition from sed
- Attracts new users with simple syntax
- Maintains compatibility when needed

**Your Decision:** Option C - Hybrid

---

### Question 2: Stream Processing vs Functionality

**Trade-off:** Stream processing is complex and delays other features.

**Option A: Stream Processing First (3-4 weeks)**
- Pros:
  - Can handle large files (100GB+)
  - True sed-like behavior
  - Memory-efficient
  - Foundation for everything else
- Cons:
  - Major refactor, high risk
  - No new features for weeks
  - May introduce bugs
  - Delays other improvements

**Option B: Stream Processing Later**
- Pros:
  - Add features quickly
  - User-visible improvements sooner
  - Lower initial risk
- Cons:
  - Large files remain problematic
  - Technical debt accumulates
  - Harder to refactor later
  - Inconsistent with sed philosophy

**Option C: Hybrid Approach**
- Implement stream processing for COMMON cases (substitution, delete)
- Keep in-memory for COMPLEX cases (hold space, pattern ranges)
- Add `--stream` flag to force streaming
- Gradual migration

**My Recommendation:** **Option A (Stream Processing First)**
- Sed is fundamentally a stream processor
- Users expect to be able to process 100GB log files
- This is a core architectural requirement you specified
- Getting it wrong early will be painful later

**Your Decision:** Option A - Stream First

---

### Question 3: Backup Aggressiveness

**How aggressive should backup protection be?**

**Option A: Always Backup (Default, No Bypass)**
- Backup every time
- No `--no-backup` flag
- If disk full, error out
- Safest option
- May frustrate users in CI/CD

**Option B: Backup by Default, Allow Bypass**
- `sedx --execute` (backup)
- `sedx --no-backup` (dangerous, requires `--force`)
- Warn about risks
- Balance of safety and flexibility
- Recommended for production use

**Option C: User Choice (Configurable)**
- Config file setting: `backup = always | auto | never`
- CLI flags override config
- Most flexible
- Most complex

**Option D: Smart Backup**
- Auto-backup for interactive use
- No backup for pipes/stdout
- Optional for scripts (config)
- Check disk space first
- Best user experience

**My Recommendation:** **Option B (Backup Default, Allow Bypass)**
- Safety by default
- Allows CI/CD: `sedx --no-backup --force 's/dev/prod/' config.toml`
- Clear warnings when bypassing
- Not as complex as Option C

**Your Decision:** Option B - Backup by default, allow bypass

---

### Question 4: Disk Space Thresholds

**What thresholds for backup warnings/errors?**

**Current Proposal in ROADMAP.md:**
- ⚠️ Warning: Backup > 2GB
- ❌ Error: Backup > 60% of free disk space
- ❌ Error: Insufficient disk space

**Alternative Thresholds:**

| Threshold | Conservative | Moderate | Aggressive |
|-----------|--------------|----------|------------|
| **Size Warning** | 500MB | 2GB | 10GB |
| **Disk Usage Error** | 40% | 60% | 80% |
| **Disk Usage Warning** | 20% | 40% | 60% |

**Questions:**
1. What size threshold makes sense for your use cases? (2GB, 5GB, 10GB?)
2. What disk usage error threshold? (40%, 60%, 80%?)
3. Should these be configurable?

**My Recommendation:** **Moderate with Config**
- Default: Warn at 2GB, Error at 60%
- Configurable in `~/.sedx/config.toml`
- Allow `--disk-limit=80%` override
- Covers most cases

**Your Decision:** Moderate with config. Allow editing config via command in sedx (like `sedx config` that starts $EDITOR on the file or even like cron editor that checks the result afterwards **BETTER!** )

---

### Question 5: Simplified Syntax Priority

**How important is `sd`-style simplified syntax?**

**Current sed syntax:**
```bash
sedx 's/foo/bar/g' file.txt
sedx 's/foo/bar/i' file.txt  # case-insensitive
```

**Proposed simplified syntax:**
```bash
sedx 'foo' 'bar' file.txt           # Global replace
sedx -F 'foo' 'bar' file.txt        # String-literal mode
sedx -i 'foo' 'bar' file.txt        # Case-insensitive
```

**Option A: High Priority (Phase 2, Week 4)**
- Implement soon
- Attract users from `sd`
- Simpler for beginners
- Diverges from sed

**Option B: Medium Priority (Phase 4+)**
- Focus on sed compatibility first
- Add simplified syntax later
- Less risk of confusion
- Slower adoption

**Option C: Low Priority / Optional**
- Keep sed-only syntax
- Users can use `sd` for simple cases
- Maintain purity
- Less attractive to new users

**My Recommendation:** **Option A (High Priority)**
- Differentiates from GNU sed
- Lower barrier to entry
- Can coexist with sed syntax
- Compatibility warnings handle confusion

**Your Decision:** Option B - We are a hybrid

---

### Question 6: Missing Sed Commands Priority

**Which missing commands are MOST important?**

**Review from IMPROVEMENTS.md:**

**Tier 1 (High Value, Common):**
- `n`, `N` - Next line operations (multi-line processing)
- `P`, `D` - Multi-line print/delete
- `-n` flag - Suppress output (used in 80%+ scripts)
- `-e` flag - Multiple expressions

**Tier 2 (Medium Value):**
- `Q` - Quit without printing
- `r` - Read file
- `w` - Write file
- `:` - Labels
- `b` - Branch
- `t`, `T` - Test branch

**Tier 3 (Low Value / Rare):**
- `l` - List (visual debug)
- `=` - Print line number
- `y` - Translate
- `F` - Print filename
- `e` - Execute shell command
- `z` - Clear

**Question:**
- Implement **all** commands (12+ weeks)?
- Implement **Tier 1** only (4-6 weeks)?
- Implement **Tier 1 + Tier 2** (8-10 weeks)?
- Implement based on user demand?

**My Recommendation:** **Tier 1 + Tier 2 (8-10 weeks)**
- Tier 1 is essential for compatibility
- Tier 2 needed for complex scripts
- Tier 3 can wait for user requests
- Balances completeness with time

**Your Decision:** Tier 1 + Tier 2

---

### Question 7: Development Approach

**How should we tackle this?**

**Option A: Big Rewrite (3-4 months)**
- Stop everything
- Rewrite file_processor.rs for streaming
- Add all missing commands
- Release v0.2.0 when complete
- Pros: Clean architecture, all features at once
- Cons: Long time without releases, high risk

**Option B: Incremental Releases**
- v0.1.1: Stream processing (3-4 weeks)
- v0.1.2: Enhanced substitution (2-3 weeks)
- v0.1.3: Backup management (2 weeks)
- v0.1.4: Missing commands Tier 1 (4 weeks)
- v0.1.5: Flow control (4 weeks)
- Pros: Regular releases, user feedback, lower risk
- Cons: Slower to completeness

**Option C: Hybrid**
- v0.2.0: Stream processing (critical foundation)
- v0.2.x: Incremental features on top
- Pros: Foundation done right, incremental after
- Cons: One big delay, then fast iterations

**My Recommendation:** **Option B (Incremental Releases)**
- Get stream processing out (v0.1.1 or v0.2.0)
- Add features incrementally
- Get user feedback early
- Lower risk
- More satisfying progress

**Your Decision:** Option B: Incremental

---

### Question 8: Compatibility Philosophy

**How should incompatibilities be handled?**

**Scenario:** User types `sedx 'foo' 'bar' file.txt` (non-sed syntax)

**Option A: Silent Acceptance**
- Work without warning
- Assume modern mode
- Pros: Cleaner UX
- Cons: Confusing for sed users

**Option B: Warning Message**
```
$ sedx 'foo' 'bar' file.txt
warning: Using simplified syntax (not sed-compatible)
sed equivalent: sed 's/foo/bar/g' file.txt
Use --compat=strict to disable this syntax
```
- Pros: Educational, clear
- Cons: Verbose, annoying

**Option C: Strict Mode Default**
```
$ sedx 'foo' 'bar' file.txt
error: Invalid sed syntax
hint: Use --compat=extended for simplified syntax
```
- Pros: Fail-safe, clear
- Cons: Requires flags

**Option D: Configurable**
- `compat = extended | strict | permissive` in config
- Default: extended (allow both, warn)
- Pros: User control
- Cons: Complex behavior

**My Recommendation:** **Option B (Warning Message)**
- Best balance of safety and UX
- Educates users
- Can be suppressed with config
- Clear communication

**Your Decision:** Option B

---

### Question 9: Resource Priority

**We have limited time. What's MOST important?**

**Rank these 1-6 (1 = highest priority):**

- **[ ]** Stream processing (handle 100GB files)
- **[ ]** Sed compatibility (Tier 1 commands: n, N, P, D, Q, -n, -e)
- **[ ]** Backup disk management (safety)
- **[ ]** Simplified syntax ('old' 'new' like sd)
- **[ ]** Flow control (:, b, t, T)
- **[ ]** File I/O (r, w commands)

**My Recommendation:**
1. Stream processing (foundation)
2. Sed compatibility Tier 1 (core usage)
3. Backup disk management (safety)
4. Simplified syntax (usability)
5. Flow control (advanced scripts)
6. File I/O (less common)

**Your Ranking:** 

- **1** Stream processing (handle 100GB files)
- **3** Sed compatibility (Tier 1 commands: n, N, P, D, Q, -n, -e)
- **2** Backup disk management (safety)
- **4** Simplified syntax ('old' 'new' like sd)
- **5** Flow control (:, b, t, T)
- **6** File I/O (r, w commands)

---

### Question 10: Testing Strategy

**How rigorous should testing be?**

**Option A: Basic (Current Level)**
- Regression tests vs GNU sed
- Manual testing for new features
- Minimal unit tests
- Pros: Faster development
- Cons: More bugs likely

**Option B: Standard**
- Unit tests for all modules
- Regression tests for all commands
- Integration tests for scenarios
- Some edge case testing
- Pros: Balanced
- Cons: Takes 20-30% dev time

**Option C: Comprehensive**
- Property-based tests (proptest)
- Fuzz testing for parser
- Large file tests (100GB+)
- Memory leak detection
- Benchmark suite
- Pros: High confidence
- Cons: Takes 40-50% dev time

**Option D: Test-Driven Development**
- Write tests before code
- 100% coverage goal
- Continuous integration testing
- Pros: Highest quality
- Cons: Slowest initially

**My Recommendation:** **Option B (Standard)**
- Good balance
- Catch most bugs
- Not too time-consuming
- Scale up as project matures

**Your Decision:** Option C. Tests should be added at each sprint for new functions, to ensure everything works fine and there are no regressions.

---

## 📋 Summary: Decisions Needed

Please answer these 10 questions to finalize the roadmap:

1. **Project Identity:** A (sed-only) / B (modern) / C (hybrid)
2. **Stream Processing:** A (first) / B (later) / C (hybrid)
3. **Backup Aggressiveness:** A (always) / B (default+bypass) / C (config) / D (smart)
4. **Disk Thresholds:** (size warning, disk error, configurable?)
5. **Simplified Syntax Priority:** A (high) / B (medium) / C (low)
6. **Missing Commands:** All / Tier 1 / Tier 1+2 / On-demand
7. **Development Approach:** A (rewrite) / B (incremental) / C (hybrid)
8. **Compatibility Philosophy:** A (silent) / B (warning) / C (strict) / D (config)
9. **Resource Priority:** [Rank 1-6]
10. **Testing Strategy:** A (basic) / B (standard) / C (comprehensive) / D (TDD)

Once you answer these, I'll create the **FINAL ROADMAP** tailored to your vision.
