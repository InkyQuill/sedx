# SedX Tests

## Running Tests

```bash
# Run all tests (unit + integration)
cargo test

# Run one integration-test binary
cargo test --test command_coverage
cargo test --test diff_output
cargo test --test pipeline
cargo test --test atomic_writes
cargo test --test backup_rollback
cargo test --test streaming
cargo test --test regex_flavors
cargo test --test errors

# Opt-in slow tests (e.g. 100 MB streaming)
cargo test -- --ignored
```

## Integration Test Files

| File | Description |
|------|-------------|
| `command_coverage.rs` | Core sed command coverage (s, d, p, q, i, a, c, hold space, flow control) |
| `diff_output.rs` | Diff formatting and dry-run output |
| `pipeline.rs` | stdin/stdout pipeline mode |
| `atomic_writes.rs` | Atomic write, permission preservation, symlink handling |
| `backup_rollback.rs` | Backup creation and rollback functionality |
| `streaming.rs` | Large file streaming (constant-memory processing) |
| `regex_flavors.rs` | PCRE/ERE/BRE regex mode behaviour |
| `errors.rs` | Error handling: bad expressions, missing files, permissions |

## Shell Tools (not tests)

Performance and profiling tools live in `tests/tools/`:

```bash
bash tests/tools/benchmark.sh          # Benchmark against GNU sed
bash tests/tools/memory_profile.sh     # Memory profiling for streaming
bash tests/tools/generate_large_files.sh  # Generate large test files
```
