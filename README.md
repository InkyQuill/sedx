# SedX - Safe Sed Extended

[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Crates.io](https://img.shields.io/badge/crates.io-v1.0.0-blue.svg)](https://crates.io/crates/sedx)

**SedX** is a safe, modern replacement for GNU `sed` with automatic backups, preview mode, and human-readable diffs. It maintains ~90% compatibility with standard sed while adding safety features essential for production use and AI-assisted development.

## 🚀 Why SedX?

### The Problem with GNU Sed

```bash
# One mistake can corrupt files instantly
$ sed 's/version=.*/version=2.0/' config.txt
# Oops! Now your config is broken...
```

### The SedX Solution

```bash
$ sedx 's/version=.*/version=2.0/' config.txt
config.txt
L3: ~ version=2.0

Total: 1 change (1 modified)
Backup ID: 20260106-210000-abc123
Rollback with: sedx rollback 20260106-210000-abc123

# Something wrong? Rollback instantly!
$ sedx rollback 20260106-210000-abc123
✅ Rollback complete
```

## ✨ Key Features

| Feature | GNU Sed | SedX |
|----------|----------|------|
| Preview changes | ❌ | ✅ `--dry-run` |
| Automatic backups | ❌ | ✅ Always |
| One-command rollback | ❌ | ✅ `rollback` |
| Contextual diffs | ❌ | ✅ 2 lines by default |
| Interactive mode | ❌ | ✅ `--interactive` |
| Colored output | ❌ | ✅ Auto-detected |
| Multiple file support | ✅ | ✅ |
| Line ranges | ✅ | ✅ |
| Pattern ranges | ✅ | ✅ |
| Negation `!` | ✅ | ✅ |
| Command grouping `{}` | ✅ | ✅ |
| Quit command `q` | ✅ | ✅ |

## ⚠️ Важные отличия от GNU sed / Important Differences from GNU sed

### Регулярные выражения / Regular Expressions

**SedX использует расширенный синтаксис регулярных выражений (ERE)**, аналогично `sed -E`:

```bash
# Группы используют круглые скобки без экранирования
sedx 's/([a-z]+)/\U\1/g'  # Правильно
sedx 's/\([a-z]\+\)/\U\1/g'  # Неправильно (это BRE синтаксис)
```

В GNU sed по умолчанию используется базовый синтаксис (BRE), где `(`, `)`, `{`, `}` нужно экранировать.

### Обратные ссылки в заменах / Backreferences in Replacements

SedX поддерживает обратные ссылки в заменах, но использует расширенный синтаксис регулярных выражений:

```bash
# Дублирование слова: "test test" → "test"
sedx 's/([a-z]+) \1/\1/g'  # Правильно (ERE синтаксис)
sed 's/\([a-z]\+\) \1/\1/g'  # GNU sed с BRE синтаксисом
```

### Подстановка по шаблону / Pattern Substitution

Подстановка по шаблону применяется ко **всем** совпадающим строкам (соответствует поведению GNU sed):

```bash
# Заменить "test" на "fix" во всех строках, содержащих "error"
/error/s/test/fix/  # Применяется ко всем строкам с "error"
```

### Группировка команд / Command Grouping

При использовании фигурных скобок в shell используйте одинарные кавычки:

```bash
# Одинарные кавычки (рекомендуется)
sedx '{s/foo/bar/g; s/baz/qux/g}' file.txt

# Если нужны двойные кавычки - экранируйте скобки
sedx "{ s/foo/bar/g; s/baz/qux/g }" file.txt
```

### Уникальные возможности SedX / SedX Unique Features

- **Автоматические резервные копии** при редактировании файлов
- **Откат изменений** с помощью флага `--rollback`
- **Режим dry-run** для безопасного тестирования
- **Цветной вывод** для лучшей читаемости (можно отключить)

## 📦 Installation

### From Cargo (Recommended)

```bash
cargo install sedx
```

### From Source

```bash
# Clone the repository
git clone https://github.com/InkyQuill/sedx.git
cd sedx

# Build release version
cargo build --release

# Install to system path (optional)
sudo cp target/release/sedx /usr/local/bin/
```

### From GitHub Releases

```bash
# Download latest release
wget https://github.com/InkyQuill/sedx/releases/latest/download/sedx-x86_64-unknown-linux-gnu.tar.gz

# Extract and install
tar -xzf sedx-x86_64-unknown-linux-gnu.tar.gz
sudo cp sedx /usr/local/bin/
```

### Verify Installation

```bash
$ sedx --version
SedX v1.0.0 - Safe Sed Extended
```

## 🎯 Quick Start

### Basic Usage

```bash
# Execute by default (shows diff + creates backup)
sedx 's/foo/bar/g' file.txt

# Preview without applying
sedx --dry-run 's/foo/bar/g' file.txt

# Interactive mode
sedx --interactive 's/foo/bar/g' file.txt

# Multiple files
sedx 's/old/new/g' *.txt
```

### Dry Run Mode

```bash
$ sedx -d 's/version=[0-9]+/version=2.0/' config.txt
🔍 Dry run: s/version=[0-9]+/version=2.0/

config.txt
L3: = # Configuration
L4: = app_name=MyApp
L5: ~ version=2.0
L6: = debug=true
...
```

### Rollback Operations

```bash
# Rollback last operation
sedx rollback

# Rollback specific backup
sedx rollback 20260106-210000-abc123

# View history
sedx history

# Check backup status
sedx status
```

## 📖 Supported Sed Commands

### Substitution

**Syntax:** `[range]s/pattern/replacement/[flags]`

```bash
# Replace all occurrences
sedx 's/foo/bar/g' file.txt

# Replace first occurrence only
sedx 's/foo/bar/' file.txt

# Case-insensitive
sedx 's/foo/bar/gi' file.txt

# On specific line
sedx '10s/foo/bar/' file.txt

# On range
sedx '1,10s/foo/bar/g' file.txt

# With pattern range
sedx '/start/,/end/s/foo/bar/g' file.txt
```

**Flags:**
- `g` - global (all occurrences in line)
- `i` - case-insensitive matching

### Delete Operations

```bash
# Delete line 10
sedx '10d' file.txt

# Delete range
sedx '1,10d' file.txt

# Delete lines matching pattern
sedx '/error/d' logfile.txt

# Delete lines between patterns
sedx '/start/,/end/d' file.txt

# Delete lines NOT matching pattern
sedx '/keep/!d' file.txt
```

### Print Operations

```bash
# Print specific line
sedx '10p' file.txt

# Print range
sedx '1,10p' file.txt

# Print matching lines
sedx '/pattern/p' file.txt
```

### Quit Command

```bash
# Process only first 10 lines
sedx '10q' large_file.txt

# Quit at pattern
sedx '/ERROR/q' logfile.txt

# Quit immediately
echo "content" | sedx 'q'
```

### Command Grouping

```bash
# Multiple commands on range
sedx '1,5{s/foo/bar/g; s/baz/qux/}' file.txt

# Group with pattern
sedx '/start/{s/start/START/g; p}' file.txt
```

### Negation

```bash
# Substitute on lines NOT matching pattern
sedx '/debug/!s/production/development/g' config.txt

# Delete all except first 5 lines
sedx '1,5!d' file.txt
```

## 🎨 Output Format

### Indicators

- `=` unchanged (shown with context)
- `~` modified (line content changed)
- `+` added (new line inserted)
- `-` deleted (line removed)

### Context Control

```bash
# Show 5 lines of context
sedx --context 5 's/foo/bar/' file.txt

# No context (changed lines only)
sedx --no-context 's/foo/bar/' file.txt

# Context as short option
sedx -n 5 's/foo/bar/' file.txt
```

### Color Control

Colors are auto-detected. To disable:

```bash
# Environment variable
NO_COLOR=1 sedx 's/foo/bar/' file.txt

# Pipe to another command
sedx 's/foo/bar/' file.txt | cat
```

## 💾 Backup System

SedX automatically creates backups for every operation:

```bash
# Backup location
~/.sedx/backups/<id>/
├── operation.json  # Metadata
└── files/           # Original files
    ├── config.txt
    └── data.json
```

### Backup Management

```bash
# View all backups
sedx history

Output:
ID: 20260106-210000-abc123
  Time: 2026-01-06 21:00:00
  Command: s/version=[0-9]+/version=2.0/
  Files: 1

ID: 20260106-210500-def456
  Time: 2026-01-06 21:05:00
  Command: /error/d
  Files: 3
```

The last 50 backups are kept automatically. Old backups are cleaned up when creating new ones.

## 🔧 Advanced Usage

### Line Addressing

- **Line numbers:** `10` - line 10
- **Ranges:** `1,10` - lines 1 through 10
- **Patterns:** `/foo/` - lines matching regex "foo"
- **Last line:** `$` - last line of file
- **Negation:** `/pattern/!` - lines NOT matching pattern

### Complex Examples

```bash
# Update version in all config files
sedx 's/version=[0-9]\+/version=2.0/' config/*.toml

# Clean log files (keep only INFO lines)
sedx '/INFO/!d' logs/app.log

# Replace in section between markers
sedx '/# START/,/# END/s/old/new/g' config.txt

# Multiple operations with grouping
sedx '1,10{s/foo/bar/g; s/baz/qux/; p}' file.txt

# Process until pattern, then quit
sedx '/STOP/q/{s/^/# /}' data.txt
```

### Working with Multiple Files

```bash
# Safe multi-file replacement
sedx 's/TODO/FIXME/' src/**/*.rs

# Delete debug lines from all logs
sedx '/DEBUG/d' logs/*.log

# Apply to specific files
sedx 's/localhost/127.0.0.1/g' config/{database,app}.conf
```

## 🧪 Testing

SedX has comprehensive test coverage:

```bash
# Run all tests
cargo test

# Run integration tests
./tests/regression_tests.sh

# Run comprehensive tests
./tests/comprehensive_tests.sh
```

Test coverage includes:
- Basic substitutions (global, case-insensitive, line-specific)
- Delete operations (single line, ranges, patterns)
- Pattern ranges with state machine semantics
- Negation operator
- Print command
- Quit command
- Command grouping
- Edge cases (empty files, special characters, large files)

## 🛠️ Development

### Build from Source

```bash
# Clone repository
git clone https://github.com/InkyQuill/sedx.git
cd sedx

# Build debug version
cargo build

# Build release version (optimized)
cargo build --release

# Run binary
./target/release/sedx --help
```

### Run Tests

```bash
# Unit tests
cargo test

# With output
cargo test -- --nocapture

# Integration tests
./tests/regression_tests.sh
./tests/comprehensive_tests.sh

# Format code
cargo fmt

# Lint
cargo clippy -- -D warnings
```

### Project Structure

```
sedx/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── cli.rs               # Argument parsing
│   ├── sed_parser.rs        # Sed expression parser
│   ├── file_processor.rs    # Core processing logic
│   ├── diff_formatter.rs    # Output formatting
│   └── backup_manager.rs    # Backup/rollback system
├── tests/
│   ├── regression_tests.sh  # Basic compatibility tests
│   └── comprehensive_tests.sh # Extended test suite
├── Cargo.toml                # Dependencies
└── README.md                 # This file
```

## 📚 Examples

### Example 1: Update Configuration Files

```bash
$ sedx 's/port=3000/port=8080/' config/app.conf
config/app.conf
L15: ~ port=8080

Backup ID: 20260106-210100-xyz789
Rollback with: sedx rollback 20260106-210100-xyz789
```

### Example 2: Clean Log Files

```bash
$ sedx --dry-run '/ERROR/d' /var/log/app.log | head -20
🔍 Dry run: /ERROR/d

/var/log/app.log
L1: = INFO: Application started
L3: = INFO: Connected to database
L5: - ERROR: Connection failed
...
```

### Example 3: Batch Processing

```bash
# Replace all TODO with FIXME in Rust files
find . -name "*.rs" -exec sedx 's/TODO/FIXME/' {} \;

# Count occurrences
grep -r "FIXME" . --include="*.rs" | wc -l

# Rollback if needed
sedx rollback | head -1
```

### Example 4: Interactive Mode

```bash
$ sedx -i 's/localhost/0.0.0.0/g' docker-compose.yml
docker-compose.yml
L10: ~ version: '3.8'
L12: ~   0.0.0.0:5000
...
Total: 2 changes (2 modified)

Apply changes? [y/N] y

Applied: s/localhost/0.0.0.0/g
Backup ID: 20260106-210200-ghi345
Rollback with: sedx rollback 20260106-210200-ghi345
```

## 🆚 Sed vs SedX Compatibility

SedX aims for 90%+ compatibility with GNU sed. Most sed scripts will work unchanged:

| Feature | Status | Notes |
|---------|--------|-------|
| `s/pattern/replacement/` | ✅ | Full support with `gi` flags |
| `[range]d` | ✅ | Delete with line/pattern ranges |
| `[range]p` | ✅ | Print command |
| `q` | ✅ | Quit command |
| `!pattern` | ✅ | Negation |
| `{ ... }` | ✅ | Command grouping |
| `/pattern1/,/pattern2/` | ✅ | Pattern ranges with state machine |
| `i\`, `a\`, `c\` | ✅ | Insert, append, change |
| `$`, `0` | ✅ | Last/first line addressing |
| Hold space | ❌ | Not implemented |
| Branch/test (`:`, `b`, `t`) | ❌ | Not implemented |
| File I/O (`r`, `w`) | ❌ | Not implemented |

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
# Fork and clone
git clone https://github.com/YOUR-USERNAME/sedx.git
cd sedx

# Add upstream as remote
git remote add upstream https://github.com/InkyQuill/sedx.git

# Create branch
git checkout -b feature/your-feature

# Make changes and test
cargo test
./tests/regression_tests.sh

# Submit PR
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- GNU sed for the original specification
- Rust community for excellent crates (regex, anyhow, colored, etc.)
- All contributors who submit issues and pull requests

## 📞 Support

- **Issues:** [GitHub Issues](https://github.com/InkyQuill/sedx/issues)
- **Discussions:** [GitHub Discussions](https://github.com/InkyQuill/sedx/discussions)

## 🔮 Roadmap

### ✅ Recently Completed (v0.2.0)

- Pattern substitution on all matching lines (`/pattern/s/old/new/`)
- Backreferences in replacement strings (converted from `\1` to `$1` internally)
- Command grouping with semicolons (`{cmd1; cmd2; cmd3}`)

### 🚧 In Progress

- Extended regular expressions more features
- Performance optimizations for large files

### 📋 Planned Features

- Hold space operations (`h`, `H`, `g`, `G`, `x`)
- Branch/test commands (`:label`, `b`, `t`)
- File read/write (`r filename`, `w filename`)
- In-place editing mode (`-i` flag)

---

**Made with ❤️ by InkyQuill**

*SedX - Because production safety matters*
