# Code Review Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve every finding in `CODE_REVIEW_FINDINGS.md` with tested security hardening, parser fixes, processor parity, compatibility fixes, and targeted cleanup.

**Architecture:** Add narrow safety boundaries for sed file I/O and restore targets, then fix parser and processor state machines in place. Streaming support should be explicit: supported commands execute correctly, unsupported commands route to in-memory or fail under forced streaming.

**Tech Stack:** Rust 2024, `anyhow`, `regex`, `tempfile`, `serde_json`, existing Rust integration tests under `tests/*.rs`.

---

## File Structure

- Create: `src/path_policy.rs`
  - Owns validation of sed script file operands and backup restore targets.
  - Exposes small functions returning `anyhow::Result<PathBuf>`.
- Modify: `src/main.rs`
  - Registers `path_policy`.
  - Uses streaming capability checks that distinguish auto fallback from forced streaming.
- Modify: `src/parser/flow.rs`
  - Stops using `cmd.find(...)`; parses flow commands using the command position detected by `commands::find_command_char`.
- Modify: `src/parser/commands.rs`
  - Passes command position into flow parsers.
- Modify: `src/parser/io.rs`
  - Fixes custom delimiter tracking in `is_inside_pattern_address`.
- Modify: `src/file_processor/common.rs`
  - Makes address matching fallible or adds fallible variant for processors that need errors.
  - Fixes `s///0` behavior and replacement newline processing.
- Modify: `src/file_processor/in_memory.rs`
  - Adds in-memory pattern range state.
  - Applies sed file I/O path policy to `r/R/w/W`.
  - Removes fragile `HashMap::get().unwrap()` patterns.
- Modify: `src/file_processor/streaming.rs`
  - Applies sed file I/O path policy where file I/O commands are supported.
  - Gives nested group range state unique keys.
  - Removes silent command ignores.
  - Replaces hardcoded `/tmp` tests with `TempDir`.
  - Documents or warns about hold-space memory growth.
- Modify: `src/backup_manager.rs`
  - Validates restore metadata paths before restore.
  - Rejects symlink restore targets.
- Modify: `src/bre_converter.rs`
  - Clarifies `\n` handling and removes misleading branch.
- Modify: `src/regex_error.rs`
  - Removes safe-but-fragile `unwrap()`.
- Modify: `src/config.rs`
  - Changes or documents default streaming behavior according to Task 9.
- Modify: `docs/USER_GUIDE.md`, `docs/SPECIFICATION.md`, `README.md`
  - Documents safe file I/O restrictions, streaming fallback policy, and hold-space memory caveat.
- Test: `tests/security.rs`
  - New security-focused integration tests for path traversal, restore tampering, and symlink protections.
- Test: `tests/parser_regressions.rs`
  - New parser tests for flow command panic and custom delimiter detection.
- Test: `tests/processor_parity.rs`
  - New in-memory/streaming parity and compatibility tests.
- Test: existing `tests/streaming.rs`, `tests/command_coverage.rs`, `tests/regex_flavors.rs`, `tests/errors.rs`
  - Extend where behavior belongs with existing coverage.

---

### Task 1: Add Security Regression Tests

**Files:**
- Create: `tests/security.rs`
- Use helpers from: `tests/common/mod.rs`

- [ ] **Step 1: Create path traversal tests for sed file I/O commands**

Create `tests/security.rs` with this initial content:

```rust
mod common;

use common::{read_file, sedx_isolated, write_file};
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn write_file_command_rejects_absolute_path() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "alpha\n");
    let outside = dir.join("outside.txt");

    sedx_isolated(dir)
        .args([
            "--no-backup",
            "--force",
            &format!("w {}", outside.display()),
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsafe file I/O path"))
        .stderr(predicate::str::contains("absolute paths are not allowed"));

    assert!(!outside.exists());
}

#[test]
fn write_file_command_rejects_parent_traversal() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "alpha\n");

    sedx_isolated(dir)
        .args(["--no-backup", "--force", "w ../outside.txt", input.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsafe file I/O path"))
        .stderr(predicate::str::contains("parent traversal is not allowed"));

    assert!(!dir.parent().unwrap().join("outside.txt").exists());
}

#[test]
fn read_file_command_rejects_absolute_path() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "alpha\n");

    sedx_isolated(dir)
        .args(["--dry-run", "r /etc/passwd", input.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsafe file I/O path"));
}
```

- [ ] **Step 2: Add backup metadata tampering test**

Append this test to `tests/security.rs`:

```rust
#[test]
fn backup_restore_rejects_tampered_original_path() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let input = write_file(dir, "input.txt", "foo\n");
    let target = write_file(dir, "target.txt", "do not overwrite\n");

    sedx_isolated(dir)
        .args(["s/foo/bar/", input.to_str().unwrap()])
        .assert()
        .success();

    let backups_dir = dir.join(".sedx").join("backups");
    let backup_id = std::fs::read_dir(&backups_dir)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.path().is_dir())
        .unwrap()
        .file_name()
        .to_string_lossy()
        .into_owned();

    let metadata_path = backups_dir.join(&backup_id).join("operation.json");
    let mut metadata: serde_json::Value =
        serde_json::from_str(&read_file(&metadata_path)).unwrap();
    metadata["files"][0]["original_path"] =
        serde_json::Value::String(target.to_string_lossy().into_owned());
    std::fs::write(
        &metadata_path,
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();

    sedx_isolated(dir)
        .args(["rollback", &backup_id])
        .assert()
        .failure()
        .stderr(predicate::str::contains("backup metadata path validation failed"));

    assert_eq!(read_file(&target), "do not overwrite\n");
}
```

- [ ] **Step 3: Add Unix symlink write rejection test**

Append this test to `tests/security.rs`:

```rust
#[cfg(unix)]
#[test]
fn edit_rejects_symlink_target() {
    use std::os::unix::fs::symlink;

    let home = TempDir::new().unwrap();
    let dir = home.path();
    let target = write_file(dir, "target.txt", "secret\n");
    let link = dir.join("link.txt");
    symlink(&target, &link).unwrap();

    sedx_isolated(dir)
        .args(["--no-backup", "--force", "s/secret/public/", link.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("symlink targets are not allowed"));

    assert_eq!(read_file(&target), "secret\n");
}
```

- [ ] **Step 4: Run security tests and verify they fail**

Run:

```bash
cargo test --test security --locked
```

Expected: the three security tests fail because unsafe paths, tampered backup metadata, and symlink targets are still accepted.

- [ ] **Step 5: Commit failing tests**

```bash
git add tests/security.rs
git commit -m "test: capture code review security regressions"
```

---

### Task 2: Add Path Policy And Apply It To Sed File I/O

**Files:**
- Create: `src/path_policy.rs`
- Modify: `src/main.rs`
- Modify: `src/file_processor/in_memory.rs`
- Modify: `src/file_processor/streaming.rs`

- [ ] **Step 1: Add shared path policy module**

Create `src/path_policy.rs`:

```rust
use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};

pub fn validate_script_file_operand(path: &str) -> Result<PathBuf> {
    let candidate = Path::new(path);

    if candidate.is_absolute() {
        bail!(
            "unsafe file I/O path '{}': absolute paths are not allowed",
            path
        );
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                bail!(
                    "unsafe file I/O path '{}': parent traversal is not allowed",
                    path
                );
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("unsafe file I/O path '{}': path prefixes are not allowed", path);
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        bail!("unsafe file I/O path '{}': empty paths are not allowed", path);
    }

    Ok(normalized)
}

pub fn ensure_not_symlink(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("symlink targets are not allowed: {}", path.display());
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}
```

- [ ] **Step 2: Register the module**

In `src/main.rs`, add:

```rust
mod path_policy;
```

- [ ] **Step 3: Apply policy to in-memory read/write file commands**

In `src/file_processor/in_memory.rs`, update `Command::WriteFile`, `Command::WriteFirstLine`, `Command::ReadFile`, and `Command::ReadLine` handlers.

For write handlers, use this pattern:

```rust
let safe_path = crate::path_policy::validate_script_file_operand(filename)?;
crate::path_policy::ensure_not_symlink(&safe_path)?;
let file = File::create(&safe_path)?;
```

For read handlers, use this pattern:

```rust
let safe_path = crate::path_policy::validate_script_file_operand(filename)?;
let content = fs::read_to_string(&safe_path)?;
```

When using hash maps keyed by filename, preserve the original `filename.clone()` key unless a safe-path key is already used consistently in the file.

- [ ] **Step 4: Apply policy to streaming file I/O handlers**

In `src/file_processor/streaming.rs`, apply the same validation to any supported `ReadFile`, `ReadLine`, `WriteFile`, and `WriteFirstLine` paths. Use `ensure_not_symlink` before creating write handles.

- [ ] **Step 5: Run focused security tests**

Run:

```bash
cargo test --test security write_file_command_rejects_absolute_path --locked
cargo test --test security write_file_command_rejects_parent_traversal --locked
cargo test --test security read_file_command_rejects_absolute_path --locked
```

Expected: all three pass.

- [ ] **Step 6: Commit path policy**

```bash
git add src/path_policy.rs src/main.rs src/file_processor/in_memory.rs src/file_processor/streaming.rs tests/security.rs
git commit -m "fix: restrict sed file io paths"
```

---

### Task 3: Harden Backup Restore And Symlink Writes

**Files:**
- Modify: `src/backup_manager.rs`
- Modify: `src/file_processor/in_memory.rs`
- Modify: `src/file_processor/streaming.rs`
- Modify: `src/path_policy.rs`
- Test: `tests/security.rs`

- [ ] **Step 1: Add restore target validation helper**

In `src/path_policy.rs`, add:

```rust
pub fn validate_restore_target(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        bail!("backup metadata path validation failed: empty original path");
    }

    if path.components().any(|component| matches!(component, Component::ParentDir)) {
        bail!(
            "backup metadata path validation failed for '{}': parent traversal is not allowed",
            path.display()
        );
    }

    if let Ok(metadata) = std::fs::symlink_metadata(path)
        && metadata.file_type().is_symlink()
    {
        bail!(
            "backup metadata path validation failed for '{}': symlink targets are not allowed",
            path.display()
        );
    }

    Ok(path.to_path_buf())
}
```

- [ ] **Step 2: Validate all restore metadata before writing**

In `src/backup_manager.rs`, at the start of `restore_backup`, after metadata is loaded and backup files are resolved, validate every `original_path` before restoring any file:

```rust
for file_info in &metadata.files {
    crate::path_policy::validate_restore_target(Path::new(&file_info.original_path))?;
}
```

Then use the validated target in the restore loop:

```rust
let original_path = crate::path_policy::validate_restore_target(Path::new(&file_info.original_path))?;
self.restore_file(&backup_path, &original_path)?;
```

- [ ] **Step 3: Reject symlink edit targets before apply**

In `src/file_processor/in_memory.rs`, before writing in `apply_to_file`, call:

```rust
crate::path_policy::ensure_not_symlink(file_path)?;
```

In `src/file_processor/streaming.rs`, before persisting the temp file in `process_streaming_internal`, call:

```rust
crate::path_policy::ensure_not_symlink(file_path)?;
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test --test security backup_restore_rejects_tampered_original_path --locked
cargo test --test security edit_rejects_symlink_target --locked
cargo test --test atomic_writes --locked
```

Expected: security tests pass. Existing atomic write symlink behavior may fail because the new policy intentionally rejects symlink edits; update `tests/atomic_writes.rs` to assert rejection instead of following symlinks.

- [ ] **Step 5: Commit restore and symlink hardening**

```bash
git add src/path_policy.rs src/backup_manager.rs src/file_processor/in_memory.rs src/file_processor/streaming.rs tests/security.rs tests/atomic_writes.rs
git commit -m "fix: harden restore and symlink writes"
```

---

### Task 4: Fix Flow Parser Panic And Custom Delimiter Detection

**Files:**
- Create: `tests/parser_regressions.rs`
- Modify: `src/parser/flow.rs`
- Modify: `src/parser/commands.rs`
- Modify: `src/parser/io.rs`

- [ ] **Step 1: Add parser regression tests**

Create `tests/parser_regressions.rs`:

```rust
mod common;

use predicates::prelude::*;

#[test]
fn pattern_address_with_b_before_branch_does_not_panic() {
    common::sedx()
        .arg("/bar/b skip")
        .write_stdin("AAA\nbar\nCCC\n")
        .assert()
        .success();
}

#[test]
fn pattern_address_with_t_before_test_branch_does_not_panic() {
    common::sedx()
        .arg("/test/t done")
        .write_stdin("test\n")
        .assert()
        .success();
}

#[test]
fn custom_delimiter_address_does_not_confuse_write_command_detection() {
    common::sedx()
        .arg(r"\#alpha#w out.txt")
        .write_stdin("alpha\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}
```

- [ ] **Step 2: Run parser tests and verify failures**

Run:

```bash
cargo test --test parser_regressions --locked
```

Expected: at least the flow parser test fails or panics before implementation.

- [ ] **Step 3: Change flow parsers to accept command position**

In `src/parser/flow.rs`, change signatures:

```rust
pub fn parse_branch_at(cmd: &str, pos: usize) -> Result<Command> {
    let addr_part = &cmd[..pos];
    let label_part = &cmd[pos + 1..];
    let range = parse_optional_range(addr_part)?;
    let label = parse_optional_label(label_part);
    Ok(Command::Branch { label, range })
}
```

Add a local helper:

```rust
fn parse_optional_label(label_part: &str) -> Option<String> {
    let label_name = label_part.trim();
    if label_name.is_empty() {
        None
    } else {
        Some(label_name.to_string())
    }
}
```

Apply the same pattern to:

```rust
pub fn parse_test_at(cmd: &str, pos: usize) -> Result<Command>
pub fn parse_test_false_at(cmd: &str, pos: usize) -> Result<Command>
```

Keep existing `parse_branch`, `parse_test`, and `parse_test_false` only if unit tests need them; implement them by calling `find_command_char` and delegating to the `_at` variants.

- [ ] **Step 4: Pass command position from parser dispatch**

In `src/parser/mod.rs` or `src/parser/commands.rs`, wherever flow commands are dispatched after `find_command_char`, call:

```rust
'b' => flow::parse_branch_at(cmd, pos),
't' => flow::parse_test_at(cmd, pos),
'T' => flow::parse_test_false_at(cmd, pos),
```

- [ ] **Step 5: Fix custom delimiter tracking**

In `src/parser/io.rs`, update `is_inside_pattern_address` so `\#alpha#` treats `#` as the delimiter:

```rust
if byte == b'\\' && i + 1 < limit {
    current_opener = Some(bytes[i + 1]);
    i += 2;
    continue;
}
```

When inside a custom delimiter address, close on that delimiter and continue respecting escaped delimiters.

- [ ] **Step 6: Run parser tests**

Run:

```bash
cargo test --test parser_regressions --locked
cargo test --test command_coverage --locked
```

Expected: all pass.

- [ ] **Step 7: Commit parser fixes**

```bash
git add src/parser/flow.rs src/parser/commands.rs src/parser/io.rs tests/parser_regressions.rs
git commit -m "fix: parse flow commands with known command position"
```

---

### Task 5: Fix In-Memory Pattern Range State

**Files:**
- Create or modify: `tests/processor_parity.rs`
- Modify: `src/file_processor/in_memory.rs`

- [ ] **Step 1: Add in-memory pattern range regression**

Create `tests/processor_parity.rs` if it does not exist:

```rust
mod common;

use common::{read_file, sedx_isolated, write_file};
use tempfile::TempDir;

#[test]
fn no_streaming_pattern_range_delete_removes_middle_lines() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "range.txt", "AAA\nSTART\nBBB\nEND\nCCC\n");

    sedx_isolated(dir)
        .args([
            "--no-backup",
            "--force",
            "--no-streaming",
            "/START/,/END/d",
            file.to_str().unwrap(),
        ])
        .assert()
        .success();

    assert_eq!(read_file(&file), "AAA\nCCC\n");
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test --test processor_parity no_streaming_pattern_range_delete_removes_middle_lines --locked
```

Expected: fails because `BBB` remains.

- [ ] **Step 3: Add pattern range state to CycleState**

In `src/file_processor/in_memory.rs`, add a field to `CycleState`:

```rust
pub pattern_range_states: HashMap<(String, String), PatternRangeState>,
```

Initialize it in `CycleState::new`:

```rust
pattern_range_states: HashMap::new(),
```

Import `PatternRangeState` from `common`.

- [ ] **Step 4: Update `check_range_inclusive` pattern-pattern branch**

Replace the current pattern OR behavior with:

```rust
(Address::Pattern(start_pat), Address::Pattern(end_pat)) => {
    if start_pat == end_pat {
        return self.address_matches_cycle(start, state);
    }

    let key = (start_pat.clone(), end_pat.clone());
    let range_state = state
        .pattern_range_states
        .entry(key)
        .or_insert(PatternRangeState::LookingForStart);

    let start_match = self.address_matches_cycle(start, state);
    let end_match = self.address_matches_cycle(end, state);

    match range_state {
        PatternRangeState::LookingForStart => {
            if start_match {
                *range_state = PatternRangeState::InRange;
                true
            } else {
                false
            }
        }
        PatternRangeState::InRange => {
            if end_match {
                *range_state = PatternRangeState::LookingForStart;
            }
            true
        }
    }
}
```

If Rust borrowing rejects this shape because `address_matches_cycle` borrows `state`, compute matches before the `entry` call.

- [ ] **Step 5: Run parity tests**

Run:

```bash
cargo test --test processor_parity --locked
cargo test --test property_tests prop_streaming_matches_memory_line_range --locked
```

Expected: pass.

- [ ] **Step 6: Commit in-memory range fix**

```bash
git add src/file_processor/in_memory.rs tests/processor_parity.rs
git commit -m "fix: track in-memory pattern ranges"
```

---

### Task 6: Fix Streaming Group Range State Collisions And Silent Ignoring

**Files:**
- Modify: `tests/streaming.rs`
- Modify: `src/file_processor/common.rs`
- Modify: `src/file_processor/streaming.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add group range collision regression**

Append to `tests/streaming.rs`:

```rust
#[test]
fn streaming_group_inner_pattern_ranges_have_independent_state() {
    let parser = Parser::new(RegexFlavor::PCRE);
    let commands = parser
        .parse("{/A/,/B/s/^/x:/; /C/,/D/s/^/y:/}")
        .unwrap();
    let mut processor = StreamProcessor::new(commands);
    let mut output = Vec::new();

    processor
        .process_reader_to_writer(
            Cursor::new("A\nmid1\nB\nC\nmid2\nD\n"),
            &mut output,
            "stdin",
        )
        .unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        "x:A\nx:mid1\nx:B\ny:C\ny:mid2\ny:D\n"
    );
}
```

- [ ] **Step 2: Add forced-streaming unsupported command regression**

Append to `tests/streaming.rs`:

```rust
#[test]
fn forced_streaming_rejects_unsupported_branch_command() {
    let home = TempDir::new().unwrap();
    let dir = home.path();
    let file = write_file(dir, "input.txt", "a\n");

    sedx_isolated(dir)
        .args(["--streaming", ":x; bx", file.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not supported in streaming mode"));
}
```

- [ ] **Step 3: Change range key to include nested command identity**

In `src/file_processor/common.rs`, update `MixedRangeKey`:

```rust
#[derive(Clone, Hash, PartialEq, Eq)]
pub struct MixedRangeKey {
    pub command_path: Vec<usize>,
}
```

In `src/file_processor/streaming.rs`, change `should_apply_command_with_range` signature:

```rust
fn should_apply_command_with_range(
    &mut self,
    line: &str,
    range: &(Address, Address),
    command_path: &[usize],
) -> Result<bool>
```

Create keys with:

```rust
let key = MixedRangeKey {
    command_path: command_path.to_vec(),
};
```

Pass outer paths as `&[cmd_index]` and group inner paths as `&[cmd_index, inner_index]`.

- [ ] **Step 4: Route unsupported streaming commands away or reject forced streaming**

In `src/main.rs`, split capability checks:

```rust
fn can_use_streaming(commands: &[Command]) -> bool
fn unsupported_streaming_command(commands: &[Command]) -> Option<&'static str>
```

For unsupported commands, return names such as `"branch"`, `"test branch"`, `"read file"`, `"write file"`, or `"delete first line"`.

In `execute_command`, before choosing streaming for a file:

```rust
if streaming && let Some(command_name) = unsupported_streaming_command(&commands) {
    anyhow::bail!("command '{}' is not supported in streaming mode", command_name);
}
```

Auto mode should keep falling back to in-memory when `can_use_streaming` is false.

- [ ] **Step 5: Remove silent wildcard ignoring**

In `src/file_processor/streaming.rs`, replace `_ => {}` arms with:

```rust
unsupported => anyhow::bail!(
    "command '{}' is not supported in streaming mode",
    unsupported.command_name()
)
```

If `Command` has no `command_name`, add a small private helper in `streaming.rs`:

```rust
fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Branch { .. } => "branch",
        Command::Test { .. } => "test branch",
        Command::TestFalse { .. } => "test false branch",
        Command::DeleteFirstLine { .. } => "delete first line",
        Command::ReadFile { .. } => "read file",
        Command::WriteFile { .. } => "write file",
        Command::ReadLine { .. } => "read line",
        Command::WriteFirstLine { .. } => "write first line",
        Command::QuitWithoutPrint { .. } => "quit without print",
        _ => "unknown",
    }
}
```

- [ ] **Step 6: Run streaming tests**

Run:

```bash
cargo test --test streaming --locked
```

Expected: pass.

- [ ] **Step 7: Commit streaming state and capability fixes**

```bash
git add src/file_processor/common.rs src/file_processor/streaming.rs src/main.rs tests/streaming.rs
git commit -m "fix: isolate streaming range state"
```

---

### Task 7: Propagate Address Regex Errors

**Files:**
- Modify: `tests/errors.rs`
- Modify: `src/file_processor/common.rs`
- Modify: `src/file_processor/in_memory.rs`
- Modify: `src/file_processor/streaming.rs`

- [ ] **Step 1: Add invalid address regex regression**

Append to `tests/errors.rs`:

```rust
#[test]
fn invalid_pattern_address_surfaces_regex_error() {
    common::sedx()
        .arg("/[unterminated/s/foo/bar/")
        .write_stdin("foo\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid regex pattern"));
}
```

- [ ] **Step 2: Make address matching fallible**

In `src/file_processor/common.rs`, add:

```rust
pub fn try_matches_address(address: &Address, context: &AddressContext<'_>) -> Result<bool> {
    match address {
        Address::Pattern(pattern) => {
            let re = Regex::new(pattern)
                .with_context(|| format!("Invalid regex pattern: {}", pattern))?;
            Ok(re.is_match(context.line))
        }
        Address::Negated(inner) => Ok(!try_matches_address(inner, context)?),
        _ => Ok(matches_address(address, context)),
    }
}
```

Keep `matches_address` only for tests or non-fallible callers that never see user regexes.

- [ ] **Step 3: Use fallible matcher in processors**

In `src/file_processor/in_memory.rs`, change `address_matches_cycle` to return `Result<bool>` and propagate with `?` through callers that can return `Result`.

If a caller currently returns `bool`, split it into:

```rust
fn should_apply_to_cycle(&mut self, cmd: &Command, state: &mut CycleState) -> Result<bool>
```

In `src/file_processor/streaming.rs`, change `address_matches_current` to return `Result<bool>` and update call sites.

- [ ] **Step 4: Run focused errors**

Run:

```bash
cargo test --test errors invalid_pattern_address_surfaces_regex_error --locked
cargo test --test streaming --locked
cargo test --test command_coverage --locked
```

Expected: pass.

- [ ] **Step 5: Commit address regex error propagation**

```bash
git add src/file_processor/common.rs src/file_processor/in_memory.rs src/file_processor/streaming.rs tests/errors.rs
git commit -m "fix: surface address regex errors"
```

---

### Task 8: Fix Substitution Compatibility And Escape Processing

**Files:**
- Modify: `tests/regex_flavors.rs`
- Modify: `src/file_processor/common.rs`
- Modify: `src/bre_converter.rs`

- [x] **Step 1: Add `s///0` compatibility test**

Append to `tests/regex_flavors.rs`:

```rust
#[test]
fn substitution_zero_occurrence_replaces_all_matches() {
    sedx()
        .arg("s/foo/bar/0")
        .write_stdin("foo foo\n")
        .assert()
        .success()
        .stdout("bar bar\n");
}
```

- [x] **Step 2: Add BRE/ERE newline replacement tests**

Append:

```rust
#[test]
fn bre_replacement_newline_produces_actual_newline() {
    sedx()
        .args(["-B", r"s/foo/bar\nbaz/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("bar\nbaz\n");
}

#[test]
fn ere_replacement_newline_produces_actual_newline() {
    sedx()
        .args(["-E", r"s/foo/bar\nbaz/"])
        .write_stdin("foo\n")
        .assert()
        .success()
        .stdout("bar\nbaz\n");
}
```

- [x] **Step 3: Fix `s///0`**

In `src/file_processor/common.rs`, update `SubstitutionEngine::apply`:

```rust
Some(0) => Ok(re
    .replace_all(line, processed_replacement.as_str())
    .to_string()),
```

Place this before `Some(n) if n > 0`.

- [x] **Step 4: Clarify BRE converter `\n` handling**

In `src/bre_converter.rs`, remove the misleading branch that only treats trailing `\n` specially. Preserve `\n` consistently and add this comment near escape handling:

```rust
// Replacement escape sequences such as \n are interpreted by SubstitutionEngine.
// The converter only rewrites sed backreferences into regex crate syntax.
```

- [x] **Step 5: Fix newline double-processing**

Ensure `convert_sed_backreferences` preserves `\n` as a single backslash-n sequence that `process_replacement_escapes` can convert later:

```rust
'n' => {
    result.push('\\');
    result.push('n');
}
```

Do not convert it to `\\\\n`.

- [x] **Step 6: Run regex flavor tests**

Run:

```bash
cargo test --test regex_flavors --locked
```

Expected: pass.

- [x] **Step 7: Commit compatibility fixes**

```bash
git add src/file_processor/common.rs src/bre_converter.rs tests/regex_flavors.rs
git commit -m "fix: align substitution escape compatibility"
```

---

### Task 9: Resolve Default Streaming Policy

**Files:**
- Modify: `src/config.rs`
- Modify: `tests/streaming.rs` or `tests/processor_parity.rs`
- Modify: `docs/USER_GUIDE.md`

- [ ] **Step 1: Add config default regression**

Append to `tests/processor_parity.rs`:

```rust
#[test]
fn default_config_does_not_force_streaming_for_small_files() {
    let config = sedx::config::Config::default();
    assert_eq!(config.processing.streaming, Some(false));
    assert_eq!(config.processing.max_memory_mb, Some(100));
}
```

If `config` is not publicly exported from `lib.rs`, add a unit test in `src/config.rs` instead:

```rust
#[test]
fn default_streaming_is_threshold_only() {
    let config = Config::default();
    assert_eq!(config.processing.streaming, Some(false));
}
```

- [ ] **Step 2: Change default streaming config**

In `src/config.rs`, change:

```rust
streaming: Some(false),
```

Update:

```rust
fn default_streaming() -> Option<bool> {
    Some(false)
}
```

Update default config text:

```toml
streaming = false
```

And comment:

```text
# When false, SedX still streams files larger than max_memory_mb.
# Set true to force streaming for all compatible file-mode edits.
```

- [ ] **Step 3: Run config tests**

Run:

```bash
cargo test config::tests --locked
```

Expected: update any tests that asserted `Some(true)` to `Some(false)` when they refer to defaults.

- [ ] **Step 4: Update docs**

In `docs/USER_GUIDE.md`, document:

```markdown
By default, SedX uses in-memory processing for files below `max_memory_mb`
and streaming for larger files. Set `[processing].streaming = true` to force
streaming for compatible file-mode edits.
```

- [ ] **Step 5: Commit streaming default policy**

```bash
git add src/config.rs docs/USER_GUIDE.md tests/processor_parity.rs
git commit -m "fix: make streaming default threshold based"
```

---

### Task 10: Code Quality Cleanup

**Files:**
- Modify: `src/regex_error.rs`
- Modify: `src/file_processor/in_memory.rs`
- Modify: `src/file_processor/streaming.rs`

- [ ] **Step 1: Replace `regex_error.rs` unwrap**

Find:

```rust
if closing.is_none() || (closing.is_some() && closing.unwrap() < 2) {
```

Replace with:

```rust
if closing.is_none_or(|position| position < 2) {
```

- [ ] **Step 2: Replace fragile HashMap unwraps in diff generation**

In `src/file_processor/in_memory.rs`, replace patterns like:

```rust
old_content: Some(deletions.get(&line_num).unwrap().clone()),
```

with:

```rust
if let Some(deleted_content) = deletions.get(&line_num) {
    changes.push(LineChange {
        line_number: line_num,
        change_type: ChangeType::Modified,
        content: inserted_content.clone(),
        old_content: Some(deleted_content.clone()),
    });
}
```

Apply the same binding style for insertions and deletions in `generate_line_changes` and `generate_diff_lines`.

- [ ] **Step 3: Replace hardcoded `/tmp` tests**

In `src/file_processor/streaming.rs` test module, replace hardcoded paths like:

```rust
let test_file_path = "/tmp/test_streaming.txt";
```

with:

```rust
let temp_dir = tempfile::TempDir::new().unwrap();
let test_file_path = temp_dir.path().join("test_streaming.txt");
```

Pass `&test_file_path` directly to processor calls.

- [ ] **Step 4: Run focused tests and clippy**

Run:

```bash
cargo test file_processor::streaming::tests --locked
cargo test --test command_coverage --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Expected: pass.

- [ ] **Step 5: Commit quality cleanup**

```bash
git add src/regex_error.rs src/file_processor/in_memory.rs src/file_processor/streaming.rs
git commit -m "chore: remove fragile unwraps and tmp paths"
```

---

### Task 11: Documentation And Policy Resolutions

**Files:**
- Modify: `README.md`
- Modify: `docs/USER_GUIDE.md`
- Modify: `docs/SPECIFICATION.md`
- Modify: `CODE_REVIEW_FINDINGS.md` or create: `docs/REMEDIATION_STATUS.md`

- [ ] **Step 1: Document sed file I/O restrictions**

Add to `docs/USER_GUIDE.md` command reference:

```markdown
### Safe File I/O Restrictions

SedX supports sed file I/O commands (`r`, `R`, `w`, `W`) only for safe
relative paths under the current working directory. Absolute paths, parent
directory traversal (`..`), and platform path prefixes are rejected. This is
stricter than GNU sed by design: SedX treats sed scripts as potentially
untrusted input and avoids arbitrary file read/write behavior.
```

- [ ] **Step 2: Document streaming fallback policy**

Add:

```markdown
### Streaming Compatibility

When streaming is selected automatically, SedX falls back to in-memory
processing for commands that do not have streaming support. When `--streaming`
is explicitly provided, unsupported commands produce an error instead of being
silently ignored.
```

- [ ] **Step 3: Document hold-space memory caveat**

Add:

```markdown
### Hold Space And Memory

Streaming mode keeps line processing bounded for ordinary substitutions,
deletes, and range operations. Sed commands that intentionally accumulate data,
such as `H`, can grow hold space with input size. This matches sed semantics
and is not a constant-memory operation.
```

- [ ] **Step 4: Document multi-file atomicity policy**

Add:

```markdown
### Multi-File Atomicity

SedX backs up files before applying multi-file edits. Each file write is
atomic, but the whole multi-file operation is not a single transaction. If the
process is interrupted mid-operation, use the printed backup ID with
`sedx rollback <id>` to restore affected files.
```

- [ ] **Step 5: Add remediation status**

Create `docs/REMEDIATION_STATUS.md`:

```markdown
# Code Review Remediation Status

Source review: `CODE_REVIEW_FINDINGS.md`

## Fixed

- SAF-001: Sed file I/O paths are restricted to safe relative paths.
- SAF-002: Backup restore validates metadata paths before writing.
- SAF-003: Symlink edit and restore targets are rejected.
- BUG-001: Flow parsers use the command position identified by parser dispatch.
- BUG-002: In-memory pattern ranges use a state machine.
- BUG-003: Streaming group range state is keyed by nested command path.
- BUG-004: Custom delimiter address detection handles `\X...X`.
- BUG-006: Invalid address regexes surface errors.
- BUG-007: Unsupported forced-streaming commands fail clearly.
- BUG-008: `s///0` replaces all matches.
- BUG-009: BRE/ERE replacement newlines are tested and handled.
- CQ-002: Fragile unwrap in regex diagnostics removed.
- CQ-004: Fragile HashMap unwraps removed.
- CQ-005: Hardcoded `/tmp` tests removed.

## Policy Resolved

- DES-002: Multi-file operations remain per-file atomic, not transaction-wide.
- DES-003: Hold-space memory growth is documented as inherent sed semantics.
- DES-005: Stale temp files after SIGKILL remain an accepted OS-level limitation.
```

- [ ] **Step 6: Run doc command**

Run:

```bash
cargo doc --no-deps --all-features --locked
```

Expected: pass.

- [ ] **Step 7: Commit docs**

```bash
git add README.md docs/USER_GUIDE.md docs/SPECIFICATION.md docs/REMEDIATION_STATUS.md CODE_REVIEW_FINDINGS.md
git commit -m "docs: record review remediation policy"
```

---

### Task 12: Final Full Verification

**Files:**
- No code changes expected.

- [ ] **Step 1: Run formatting check**

```bash
cargo fmt --all -- --check
```

Expected: no output and exit 0.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Expected: no warnings.

- [ ] **Step 3: Run full tests**

```bash
cargo test --all-targets --all-features --locked --no-fail-fast
```

Expected: all tests pass. Ignored large streaming test may remain ignored.

- [ ] **Step 4: Run docs**

```bash
cargo doc --no-deps --all-features --locked
```

Expected: docs build successfully.

- [ ] **Step 5: Inspect working tree**

```bash
git status --short
```

Expected: empty output.

- [ ] **Step 6: Produce final remediation summary**

Write a concise summary with:

- Commit hashes created during the remediation.
- Verification commands and pass status.
- Any accepted residual policy limitations from `docs/REMEDIATION_STATUS.md`.
