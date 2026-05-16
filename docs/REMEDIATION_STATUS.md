# Code Review Remediation Status

Source review: `CODE_REVIEW_FINDINGS.md`

## Fixed

- SAF-001: Sed file I/O paths are restricted to safe relative paths.
- SAF-002: Backup restore validates metadata paths before writing.
- SAF-003: Symlink edit and restore targets are rejected.
- BUG-001: Flow parsers use the command position identified by parser dispatch.
- BUG-002: In-memory pattern ranges use per-command state machines.
- BUG-003: Streaming group range state is keyed by nested command path.
- BUG-004: Custom delimiter address detection handles `\X...X`.
- BUG-005: Substitution parsing handles escaped delimiters, command split characters, and invalid flags.
- BUG-006: Invalid address regexes surface errors.
- BUG-007: Unsupported forced-streaming commands fail clearly.
- BUG-008: `s///0` replaces all matches.
- BUG-009: BRE/ERE replacement newlines, ampersands, dollars, and numeric backrefs are tested and handled.
- DES-001: Replacement escape and backreference interaction is documented and covered by integration tests.
- DES-004: Default streaming is threshold-based; `streaming = true` now explicitly forces streaming.
- CQ-002: Fragile unwrap in regex diagnostics removed.
- CQ-003: Address regex errors now propagate instead of being silently swallowed in processors.
- CQ-004: Fragile HashMap unwraps removed.
- CQ-005: Hardcoded `/tmp` tests removed.

## Policy Resolved

- DES-002: Multi-file operations remain per-file atomic, not transaction-wide. Use `sedx rollback <id>` after interruption.
- DES-003: Hold-space memory growth is documented as inherent sed semantics.
- DES-005: Stale temp files after SIGKILL remain an accepted OS-level limitation.

## Accepted Residual Work

- CQ-001: Streaming group command handling still has duplication. It is a maintainability issue, not a known correctness bug after the range-state fixes.
- Existing user configs with `streaming = true` continue to force streaming. Users who want threshold-only behavior should change that setting to `false`.
