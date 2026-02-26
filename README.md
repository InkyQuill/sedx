# SedX - Safe Sed Extended

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**SedX** is a safe, modern replacement for GNU `sed` that protects you from accidental data loss. It maintains ~90% compatibility with standard sed while adding automatic backups, preview mode, and one-command rollback.

## Get Started in 10 Seconds

```bash
# Install
cargo install --git https://github.com/InkyQuill/sedx.git

# Preview changes before applying
sedx --dry-run 's/old/new/g' file.txt

# Apply safely (automatic backup created)
sedx 's/old/new/g' file.txt

# Rollback if needed
sedx rollback
```

## Why SedX?

GNU sed is powerful but unforgiving—one mistake can corrupt files instantly. SedX gives you the same power with safety features:

- **Automatic backups** before every modification
- **Preview mode** to see changes before applying
- **One-command rollback** to undo mistakes
- **Human-readable diffs** with context
- **Streaming mode** for large files (100GB+ with <100MB RAM)

## Installation

```bash
# From GitHub (recommended)
cargo install --git https://github.com/InkyQuill/sedx.git
export PATH="$HOME/.cargo/bin:$PATH"

# From source
git clone https://github.com/InkyQuill/sedx.git && cd sedx
cargo build --release
sudo cp target/release/sedx /usr/local/bin/
```

## Key Differences from GNU sed

| Feature | GNU Sed | SedX |
|---------|---------|------|
| Preview changes | No | `--dry-run` |
| Automatic backups | No | Yes |
| Rollback | No | `sedx rollback` |
| Default regex | BRE | **PCRE** |

**Important:** SedX uses **PCRE** by default, not BRE. Groups use unescaped parentheses: `s/([a-z]+)/\U\1/g`. For GNU sed compatibility, use `sedx -B` (BRE) or `sedx -E` (ERE).

## Common Commands

```bash
# Substitution
sedx 's/foo/bar/g' file.txt              # Replace all
sedx '10s/foo/bar/' file.txt             # Line 10 only
sedx '1,10s/foo/bar/g' file.txt          # Range
sedx '/start/,/end/s/foo/bar/g' file.txt # Pattern range

# Delete
sedx '10d' file.txt                      # Delete line 10
sedx '/error/d' logfile.txt              # Delete matching lines
sedx '/keep/!d' file.txt                 # Delete non-matching

# Backup management
sedx history                             # View backups
sedx status                              # Disk usage
sedx rollback <id>                       # Undo operation
```

## Performance

SedX automatically switches processing modes based on file size:

- **In-Memory** (< 100MB): Full diff, fast for typical files
- **Streaming** (>= 100MB): Constant memory, processes 100GB+ efficiently

Backups are stored in `~/.sedx/backups/`. Last 50 kept automatically.

## Regex Flavors

```bash
sedx 's/(foo|bar)/baz/g' file.txt    # PCRE (default) - modern syntax
sedx -E 's/(foo|bar)/baz/g' file.txt # ERE - sed -E compatible
sedx -B 's/\(foo\|bar\)/baz/g' file.txt # BRE - GNU sed compatible
```

## Pipeline Mode

With no files specified, reads stdin and writes stdout (no backups):

```bash
echo "hello" | sedx 's/hello/HELLO/'
cat file.txt | sedx 's/foo/bar/g' | grep bar
```

## Documentation

- **[User Guide](docs/USER_GUIDE.md)** - Complete usage
- **[Migration Guide](docs/MIGRATION_GUIDE.md)** - GNU sed users
- **[Examples](docs/EXAMPLES.md)** - Practical examples
- **[Specification](docs/SPECIFICATION.md)** - Full reference
- **[Contributing](CONTRIBUTING.md)** - Developer guide

## Current Limitations

SedX aims for ~90% GNU sed compatibility. NOT yet implemented:

- `y` command (translate characters)
- `l` command (list lines escaped)
- Unicode pattern matching (character boundary issues)
- Insert/Append/Change (`i`, `a`, `c`) commands have parsing issues

See [tests/KNOWN_ISSUES.md](tests/KNOWN_ISSUES.md) for details.

## Development

```bash
cargo build --release
cargo test
cargo fmt
cargo clippy -- -D warnings
```

## License

MIT - see [LICENSE](LICENSE)

## Support

- [Issues](https://github.com/InkyQuill/sedx/issues)
- [Discussions](https://github.com/InkyQuill/sedx/discussions)

---

**Made with Rust by InkyQuill**
