# Code Review Remediation Design

Date: 2026-05-16

## Context

`CODE_REVIEW_FINDINGS.md` lists security, correctness, architecture, and quality issues found in a full source audit. This remediation program covers every finding in that review. The work should be implemented in phases so high-risk fixes land first, while still preserving one coherent remediation plan.

The repository already has recent uncommitted work for streaming parity, backup pruning, JSON output, warning emission, and enhanced errors. The implementation plan must account for current code, not only the review text, and should treat each finding as one of:

- Already fixed and verified.
- Needs a targeted behavior change.
- Needs a test/documentation/policy resolution.
- Needs a larger refactor but can be phased safely.

## Goals

- Remove critical arbitrary file read/write paths.
- Prevent backup restore metadata tampering from overwriting arbitrary files.
- Add symlink protections around writes and restore targets.
- Fix reproduced parser crashes and processor correctness bugs.
- Make streaming behavior explicit: supported commands run correctly, unsupported commands are routed away or rejected.
- Make address regex and file I/O errors visible instead of silent.
- Resolve compatibility issues called out by the review.
- Reduce fragile production `unwrap()` and test path risks.
- Document unavoidable sed semantics that can violate constant-memory expectations.

## Non-Goals

- Full conversion from `anyhow` to `thiserror`.
- Perfect GNU sed parity for every command variant.
- Full transaction support across multiple files in this pass.
- Support for arbitrary absolute paths in sed script `r/w/R/W` commands.
- Major parser or processor rewrites unrelated to the reviewed findings.

## Priority Model

The remediation program covers all findings, but implementation should be sequenced by risk.

### Phase 1: Security And Crash Blockers

- `SAF-001`: Path traversal in sed file I/O commands.
- `SAF-002`: Backup restore path traversal through tampered metadata.
- `SAF-003`: Symlink following and write/restore TOCTOU risk.
- `BUG-001`: Flow command parser panic.
- `BUG-002`: In-memory pattern range state machine.
- `BUG-003`: Streaming group command range-state collisions.

### Phase 2: Medium Correctness And Compatibility

- `BUG-004`: Custom delimiter tracking in address detection.
- `BUG-006`: Address regex compilation failures silently swallowed.
- `BUG-007`: Streaming commands silently ignored.
- `BUG-008`: `s///0` compatibility.
- `BUG-009`: BRE/ERE replacement newline handling.
- `DES-004`: Default streaming configuration masks in-memory bugs.

### Phase 3: Architecture And Quality

- `BUG-005`: Misleading BRE converter `\n` handling.
- `DES-001`: Duplicated escape processing.
- `DES-002`: Multi-file operation atomicity policy.
- `DES-003`: Hold-space memory growth in streaming mode.
- `DES-005`: Stale streaming temp files after SIGKILL.
- `CQ-001`: Streaming group handler duplication.
- `CQ-002`: Safe-but-fragile production `unwrap()`.
- `CQ-003`: Inconsistent error handling policy.
- `CQ-004`: Fragile `HashMap::get().unwrap()` in diff generation.
- `CQ-005`: Hardcoded `/tmp` paths in tests.

## Architecture

### File I/O Policy

Add a shared policy boundary for sed script file operands used by `r`, `R`, `w`, and `W`.

The policy should:

- Accept only simple relative paths rooted under the current working directory.
- Reject absolute paths.
- Reject any `..` component.
- Reject path prefixes and non-normal components.
- Return explicit errors with the rejected path and reason.

This policy belongs in runtime validation, not the parser. The parser should continue to preserve sed syntax, while the processor enforces SedX safe-editing constraints before opening any file handle.

### Backup Restore Trust Boundary

Treat backup metadata as untrusted input.

Restore should:

- Validate every `original_path` from `operation.json`.
- Ensure the target corresponds to a file actually captured in that backup entry.
- Reject symlink restore targets.
- Refuse to restore when validation fails.
- Avoid partial restore when any metadata path is invalid.

The backup manager owns this validation because it owns metadata parsing, backup file lookup, and restore semantics.

### Symlink-Safe Writes

File editing and restore paths must avoid following attacker-controlled symlinks.

On Unix, write/restore code should use `O_NOFOLLOW` or an equivalent open path when writing target files. Cross-platform behavior should at least reject symlink metadata before writes. If platform-specific hardening cannot be complete, tests and docs must state the exact protection level.

### Processor Capability And Range State

Processor selection should be explicit.

Auto mode:

- Use in-memory for commands not supported by streaming.
- Use streaming for supported commands when threshold/config says streaming is appropriate.

Forced streaming:

- Fail clearly when a script contains unsupported streaming commands.
- Never silently ignore commands.

Range state should be command-specific. Streaming group commands need unique nested range keys so inner commands do not share state accidentally. In-memory pattern ranges need a state machine equivalent to the streaming processor.

### Compatibility And Cleanup Layer

Compatibility fixes should stay close to their current ownership:

- Parser/address delimiter handling in parser modules.
- BRE/ERE conversion behavior in converter modules.
- Replacement semantics in `SubstitutionEngine`.
- Quality fixes in the smallest affected production/test code.

Do not start a broad abstraction unless it directly removes duplicated streaming command behavior or resolves a finding.

## Behavior Changes

### Sed File I/O Commands

`r`, `R`, `w`, and `W` should reject unsafe paths.

Examples that must fail:

- `w /tmp/out`
- `w ../out`
- `r /etc/passwd`
- `R ../../secret`

This is intentionally stricter than GNU sed. SedX is a safe-editing tool, and untrusted sed expressions must not become arbitrary file read/write primitives.

### Backup Restore

Tampered backup metadata must not be able to overwrite arbitrary files.

Restore should validate all targets before writing. If one target fails validation, no files should be restored.

### Streaming

No command should be silently ignored in streaming mode. Each command should be one of:

- Implemented correctly in streaming.
- Routed to in-memory before processing.
- Rejected with a clear error if forced streaming makes fallback impossible.

### Address Regex Errors

Invalid address regexes must surface as errors. Returning `false` for invalid regexes is not acceptable because it hides user mistakes.

### Compatibility Fixes

`s///0` should follow GNU-compatible behavior described in the review. BRE/ERE replacement newline behavior should be covered by integration tests before changing conversion order.

## Error Handling

Use explicit, actionable errors for:

- Unsafe sed file I/O paths.
- Invalid backup metadata paths.
- Symlink-protected write failures.
- Unsupported forced-streaming commands.
- Invalid address regexes.

The codebase can continue using `anyhow::Result` for application-level propagation in this remediation. Typed error enums are out of scope unless a specific fix requires one.

## Testing Strategy

Each finding should have at least one regression test or explicit no-op verification.

Security tests:

- File I/O commands reject absolute and traversal paths.
- Backup restore rejects tampered `original_path` metadata.
- Writes/restores reject symlink targets where platform support exists.

Parser tests:

- `/bar/b skip` does not panic.
- Pattern addresses containing `b`, `t`, or `T` parse correctly.
- Custom delimiter addresses do not confuse command detection.

Processor tests:

- In-memory `/START/,/END/d` deletes the full range.
- Streaming group commands with nested pattern ranges do not share state.
- Invalid address regexes return errors.
- Unsupported forced-streaming commands fail clearly.
- `s///0` compatibility is covered.
- BRE/ERE newline replacement behavior is covered.

Quality tests:

- Hardcoded `/tmp` paths in processor tests are replaced with `TempDir` or `NamedTempFile`.
- Existing JSON output, warning, and enhanced error tests remain green.

Full verification:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --all-targets --all-features --locked --no-fail-fast`
- `cargo doc --no-deps --all-features --locked`

## Documentation And Policy Updates

Update user-facing docs where behavior intentionally differs from GNU sed:

- Sed file I/O path restrictions.
- Streaming command fallback/rejection behavior.
- Hold-space memory growth caveat for `H` in streaming mode.
- Multi-file operation atomicity limitations, unless implemented.
- Default streaming behavior if changed.

`CODE_REVIEW_FINDINGS.md` should remain as the audit source. A later status document or checklist can mark findings fixed after implementation.

## Implementation Phasing

1. Add regression tests for all Phase 1 findings.
2. Implement Phase 1 fixes.
3. Run focused tests and full verification.
4. Add regression tests for Phase 2 findings.
5. Implement Phase 2 fixes.
6. Run focused tests and full verification.
7. Resolve Phase 3 items with targeted refactors, docs, or policy decisions.
8. Run full verification and update remediation status.

## Risks

- File I/O restrictions can break users who rely on GNU sed absolute-path behavior. This is acceptable for safety, but must be documented.
- Symlink hardening can be platform-specific. Tests should be conditional where needed.
- Changing default streaming behavior may alter performance expectations. The implementation plan should decide whether to change defaults immediately or only add tests and documentation.
- Streaming group refactors can regress recent command parity work. Keep tests focused and run the full streaming suite after each change.

## Open Decisions For Implementation Plan

- Whether `DES-004` should change the default config now, or only adjust routing semantics and document current behavior.
- Whether `DES-002` should be implemented as a transaction log or documented as a limitation for this pass.
- Whether `DES-005` needs startup cleanup now, or documentation plus a future enhancement note.

