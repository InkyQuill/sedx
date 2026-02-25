# SedX Migration Guide

**Last Updated:** 2025-02-25
**Version:** 0.2.6

This guide helps you migrate from GNU sed to SedX. It covers regex syntax differences, command compatibility, and common migration patterns.

## Table of Contents

- [Quick Reference](#quick-reference)
- [Regex Syntax Differences](#regex-syntax-differences)
- [Command Compatibility](#command-compatibility)
- [Flag Differences](#flag-differences)
- [Migration Patterns](#migration-patterns)
- [Breaking Changes](#breaking-changes)

---

## Quick Reference

### Most Important Differences

| Feature | GNU sed | SedX (Default) | SedX (Compatible) |
|---------|---------|----------------|-------------------|
| Regex flavor | BRE | PCRE | BRE with `-B` |
| Backreferences | `\1`, `\2` | `$1`, `$2` | `\1`, `\2` with `-B` |
| Groups | `\(...\)` | `(...)` | `\(...\)` with `-B` |
| Quantifiers | `\+`, `\?` | `+`, `?` | `\+`, `\?` with `-B` |
| Alternation | `\|` | `\|` | `\|` with `-B` |
| Backups | No | Yes (automatic) | Yes (automatic) |

### One-Line Summary

```bash
# GNU sed (BRE syntax)
sed 's/\(foo\|bar\)/baz/' file.txt

# SedX (PCRE syntax - default)
sedx 's/(foo|bar)/baz/' file.txt

# SedX (BRE syntax - GNU sed compatible)
sedx -B 's/\(foo\|bar\)/baz/' file.txt
```

---

## Regex Syntax Differences

### Overview

SedX supports three regex flavors:

1. **PCRE (default)** - Perl-Compatible Regular Expressions
2. **ERE** - Extended Regular Expressions (with `-E` flag)
3. **BRE** - Basic Regular Expressions (with `-B` flag, GNU sed compatible)

### PCRE vs BRE vs ERE

#### Groups

```bash
# GNU sed (BRE)
sed 's/\(foo\)\(bar\)/\2\1/' file.txt

# SedX (PCRE - default)
sedx 's/(foo)(bar)/$2$1/' file.txt

# SedX (ERE - like sed -E)
sedx -E 's/(foo)(bar)/\2\1/' file.txt  # Backrefs still use \1 in replacement

# SedX (BRE - GNU sed compatible)
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt
```

#### Quantifiers

```bash
# GNU sed (BRE)
sed 's/foo\+/FOO/' file.txt
sed 's/bar\?/BAR/' file.txt
sed 's/baz\{3,5\}/BAZ/' file.txt

# SedX (PCRE - default)
sedx 's/foo+/FOO/' file.txt
sedx 's/bar?/BAR/' file.txt
sedx 's/baz{3,5}/BAZ/' file.txt

# SedX (BRE - GNU sed compatible)
sedx -B 's/foo\+/FOO/' file.txt
sedx -B 's/bar\?/BAR/' file.txt
sedx -B 's/baz\{3,5\}/BAZ/' file.txt
```

#### Alternation

```bash
# GNU sed (BRE)
sed 's/foo\|bar/baz/' file.txt

# SedX (PCRE - default)
sedx 's/foo|bar/baz/' file.txt

# SedX (BRE - GNU sed compatible)
sedx -B 's/foo\|bar/baz/' file.txt
```

#### Backreferences in Replacements

```bash
# GNU sed
sed 's/\(foo\)\(bar\)/\2\1/' file.txt
sed 's/foo/\U&/'  # Uppercase match (GNU extension)

# SedX (PCRE - default)
sedx 's/(foo)(bar)/$2$1/' file.txt

# SedX (ERE)
sedx -E 's/(foo)(bar)/\2\1/' file.txt

# SedX (BRE - GNU sed compatible)
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt
```

**Note:** In PCRE mode, use `$1`, `$2` for backreferences. In ERE/BRE modes, use `\1`, `\2`.

### Special Characters

#### Escaped vs Unescaped

| Character | GNU sed (BRE) | SedX (PCRE) | SedX (BRE) |
|-----------|---------------|-------------|------------|
| `(` | Escaped: `\(` | Unescaped: `(` | Escaped: `\(` |
| `)` | Escaped: `\)` | Unescaped: `)` | Escaped: `\)` |
| `{` | Escaped: `\{` | Unescaped: `{` | Escaped: `\{` |
| `}` | Escaped: `\}` | Unescaped: `}` | Escaped: `\}` |
| `+` | Escaped: `\+` | Unescaped: `+` | Escaped: `\+` |
| `?` | Escaped: `\?` | Unescaped: `?` | Escaped: `\?` |
| `|` | Escaped: `\|` | Unescaped: `|` | Escaped: `\|` |

### Anchor and Boundary

Anchors work the same across all flavors:

```bash
# Same in GNU sed, SedX PCRE, ERE, and BRE
sedx 's/^start/moved/' file.txt   # Line start
sedx 's/end$/' file.txt           # Line end
sedx 's/\<word\>/WORD/g' file.txt # Word boundary (GNU extension, not in SedX)
```

### Character Classes

Character classes are the same:

```bash
# Same in all flavors
sedx 's/[a-z]/X/g' file.txt       # Lowercase letters
sedx 's/[A-Z0-9]/X/g' file.txt    # Uppercase and digits
sedx 's/[^abc]/X/g' file.txt      # Negated class
sedx 's/[[:alpha:]]/X/g' file.txt # POSIX character class
```

---

## Command Compatibility

### Supported Commands

SedX supports ~90% of GNU sed commands:

| Command | Description | Status | Notes |
|---------|-------------|--------|-------|
| `s/pattern/replacement/` | Substitution | Full | With `g`, `i`, `N` flags |
| `[range]d` | Delete | Full | Including pattern ranges |
| `[range]p` | Print | Full | |
| `q` | Quit | Full | |
| `Q` | Quit without print | Full | Phase 4 |
| `i\text` | Insert | Full | |
| `a\text` | Append | Full | |
| `c\text` | Change | Full | |
| `{...}` | Command group | Full | With semicolon separation |
| `h` | Hold (copy) | Full | |
| `H` | Hold append | Full | |
| `g` | Get (copy) | Full | |
| `G` | Get append | Full | |
| `x` | Exchange | Full | |
| `n` | Next line | Full | Phase 4 |
| `N` | Next append | Full | Phase 4 |
| `P` | Print first line | Full | Phase 4 |
| `D` | Delete first line | Full | Phase 4 |
| `:label` | Label | Full | Phase 5 |
| `b [label]` | Branch | Full | Phase 5 |
| `t [label]` | Branch if substituted | Full | Phase 5 |
| `T [label]` | Branch if not substituted | Full | Phase 5 |
| `r file` | Read file | Full | Phase 5 |
| `w file` | Write file | Full | Phase 5 |
| `R file` | Read one line | Full | Phase 5 |
| `W file` | Write first line | Full | Phase 5 |
| `=` | Print line number | Full | Phase 5 |
| `F` | Print filename | Full | GNU extension |
| `z` | Clear pattern space | Full | GNU extension |

### Partially Supported Commands

| Command | Status | Limitations |
|---------|--------|-------------|
| `y/abc/xyz/` | Not implemented | Use `s/a/x/g; s/b/y/g; s/c/z/g` |
| `l` | Not implemented | Print visible characters |
| Case conversion in replacement (`\U`, `\L`) | Not implemented | Use post-processing |

### GNU Sed Extensions Not in SedX

```bash
# Word boundaries (not supported in SedX)
sed 's/\<word\>/WORD/g' file.txt
# SedX alternative: use word boundary syntax
sedx 's/\bword\b/WORD/g' file.txt

# Case conversion in replacement (not supported)
sed 's/\(foo\)/\U\1/' file.txt
# SedX alternative: use multiple passes or external tool
sedx 's/foo/FOO/g' file.txt
```

---

## Flag Differences

### GNU sed Flags vs SedX Flags

| GNU sed Flag | SedX Equivalent | Notes |
|--------------|-----------------|-------|
| `-n` | `-n`, `--quiet` | Same behavior |
| `-e` | `-e`, `--expression` | Same behavior |
| `-f` | `-f`, `--file` | Same behavior |
| `-i` | `--interactive` | Different meaning! |
| `-i[SUFFIX]` | Not supported | Use `--no-backup --force` |
| `-E` | `-E`, `--ere` | Same behavior |
| `-r` | `-E`, `--ere` | `-r` is alias for `-E` |
| `-z` | Not supported | Null-terminated lines |
| `-s` | Not supported | Separate files |
| `-u` | Not supported | Unbuffered |

### Unique SedX Flags

| Flag | Description |
|------|-------------|
| `-d`, `--dry-run` | Preview changes without applying |
| `--no-context` | Show only changed lines |
| `--context N` | Set context lines (0-10) |
| `--streaming` | Force streaming mode |
| `--no-streaming` | Disable streaming mode |
| `--no-backup` | Skip backup (requires `--force`) |
| `--force` | Force dangerous operations |
| `-B`, `--bre` | Use Basic Regular Expressions (GNU sed compatible) |
| `--backup-dir` | Custom backup directory |

### Subcommand Differences

GNU sed uses only expressions. SedX has subcommands:

```bash
# SedX subcommands (not in GNU sed)
sedx rollback                    # Rollback changes
sedx history                     # View operation history
sedx status                      # Backup status
sedx config                      # Edit configuration
sedx backup list                 # List backups
sedx backup prune --keep=10      # Clean old backups
```

---

## Migration Patterns

### Pattern 1: Simple Substitution

```bash
# GNU sed
sed 's/foo/bar/g' file.txt

# SedX (no changes needed)
sedx 's/foo/bar/g' file.txt
```

### Pattern 2: Groups and Backreferences

```bash
# GNU sed
sed 's/\(foo\)\(bar\)/\2\1/' file.txt

# SedX - Option 1: Convert to PCRE (recommended)
sedx 's/(foo)(bar)/$2$1/' file.txt

# SedX - Option 2: Use BRE mode
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt

# SedX - Option 3: Use ERE mode
sedx -E 's/(foo)(bar)/\2\1/' file.txt
```

### Pattern 3: Complex Regex with Quantifiers

```bash
# GNU sed
sed 's/[0-9]\{3,5\}/NUMBER/g' file.txt

# SedX (PCRE - default)
sedx 's/[0-9]{3,5}/NUMBER/g' file.txt

# SedX (BRE)
sedx -B 's/[0-9]\{3,5\}/NUMBER/g' file.txt
```

### Pattern 4: Alternation

```bash
# GNU sed
sed 's/cat\|dog\|bird/pet/g' file.txt

# SedX (PCRE - default)
sedx 's/cat|dog|bird/pet/g' file.txt

# SedX (BRE)
sedx -B 's/cat\|dog\|bird/pet/g' file.txt
```

### Pattern 5: In-place Editing

```bash
# GNU sed (modifies file in place)
sed -i 's/foo/bar/g' file.txt

# SedX (creates backup, shows diff)
sedx 's/foo/bar/g' file.txt

# SedX (no backup, like GNU sed -i)
sedx --no-backup --force 's/foo/bar/g' file.txt
```

### Pattern 6: Multiple Expressions

```bash
# GNU sed
sed -e 's/foo/bar/' -e 's/baz/qux/' file.txt

# SedX (same syntax)
sedx -e 's/foo/bar/' -e 's/baz/qux/' file.txt

# SedX (alternate syntax)
sedx '{s/foo/bar/; s/baz/qux/}' file.txt
```

### Pattern 7: Script Files

```bash
# GNU sed
cat script.sed
s/foo/bar/g
s/baz/qux/g

sed -f script.sed file.txt

# SedX (same syntax)
sedx -f script.sed file.txt
```

### Pattern 8: Pattern Ranges

```bash
# GNU sed
sed '/start/,/end/s/foo/bar/g' file.txt

# SedX (no changes needed)
sedx '/start/,/end/s/foo/bar/g' file.txt
```

### Pattern 9: Negation

```bash
# GNU sed
sed '/keep/!d' file.txt

# SedX (no changes needed)
sedx '/keep/!d' file.txt
```

### Pattern 10: Hold Space Operations

```bash
# GNU sed
sed '1h; 1d; $G' file.txt

# SedX (no changes needed)
sedx '1h; 1d; $G' file.txt
```

---

## Breaking Changes

### 1. Default Regex Flavor

**Impact:** High

SedX defaults to PCRE, not BRE like GNU sed. This means unescaped special characters have special meaning.

```bash
# GNU sed - literal parentheses
sed 's/(foo)/bar/' file.txt  # Matches "(foo)"

# SedX - capturing group
sedx 's/(foo)/bar/' file.txt  # Matches "foo" and captures it

# Fix: Escape or use BRE mode
sedx 's/\(foo\)/bar/' file.txt  # Use BRE mode
sedx -B 's/(foo)/bar/' file.txt  # Match literal "(foo)"
```

### 2. Backreference Syntax

**Impact:** Medium

PCRE mode uses `$1` instead of `\1` for backreferences in replacements.

```bash
# GNU sed
sed 's/\(foo\)\(bar\)/\2\1/' file.txt

# SedX PCRE
sedx 's/(foo)(bar)/$2$1/' file.txt

# Fix: Use BRE mode or ERE mode
sedx -B 's/\(foo\)\(bar\)/\2\1/' file.txt
sedx -E 's/(foo)(bar)/\2\1/' file.txt  # ERE keeps \1 in replacement
```

### 3. In-place Editing Behavior

**Impact:** Low

GNU sed's `-i` flag modifies files without backup. SedX always creates backups by default.

```bash
# GNU sed - destructive
sed -i 's/foo/bar/' file.txt  # No going back!

# SedX - safe by default
sedx 's/foo/bar/' file.txt    # Backup created automatically

# Match GNU sed behavior
sedx --no-backup --force 's/foo/bar/' file.txt
```

### 4. Output Format

**Impact:** Low

SedX shows colored diffs by default, not just the output.

```bash
# GNU sed
sed 's/foo/bar/' file.txt  # Prints full file content

# SedX
sedx 's/foo/bar/' file.txt  # Shows diff with context indicators

# Suppress diff (pipeline mode)
cat file.txt | sedx 's/foo/bar/'
```

---

## Migration Checklist

### Step 1: Identify Regex Usage

Audit your sed scripts for:

- [ ] Groups: `\(` and `\)` → Use `-B` flag or convert to `(` and `)`
- [ ] Quantifiers: `\+`, `\?`, `\{n,m\}` → Use `-B` flag or convert to `+`, `?`, `{n,m}`
- [ ] Alternation: `\|` → Use `-B` flag or convert to `|`
- [ ] Backreferences: `\1`, `\2` in replacements → Use `-B`/`-E` flag or convert to `$1`, `$2`

### Step 2: Choose Migration Strategy

**Option A: Use `-B` Flag (Quick)**

Add `-B` flag to all commands for maximum compatibility:

```bash
sed -i 's/\(foo\|bar\)/baz/' file.txt
sedx -B 's/\(foo\|bar\)/baz/' file.txt
```

**Option B: Convert to PCRE (Recommended)**

Convert regex to modern PCRE syntax:

```bash
sed 's/\(foo\)\(bar\)/\2\1/' file.txt
sedx 's/(foo)(bar)/$2$1/' file.txt
```

**Option C: Use ERE Mode**

Use `-E` flag (similar to `sed -E`):

```bash
sed -E 's/(foo|bar)/baz/' file.txt
sedx -E 's/(foo|bar)/baz/' file.txt
```

### Step 3: Test Changes

Always test before running on important files:

```bash
# Dry run to verify
sedx --dry-run 's/foo/bar/' file.txt

# Compare with GNU sed output
sed 's/foo/bar/' file.txt > /tmp/gnu-output.txt
sedx 's/foo/bar/' file.txt > /tmp/sedx-output.txt
diff /tmp/gnu-output.txt /tmp/sedx-output.txt
```

### Step 4: Update Scripts

Replace `sed` with `sedx` in your scripts:

```bash
# Before
#!/bin/bash
sed 's/foo/bar/g' config.txt

# After (with BRE mode for compatibility)
#!/bin/bash
sedx -B 's/foo/bar/g' config.txt

# After (with PCRE for modern syntax)
#!/bin/bash
sedx 's/foo/bar/g' config.txt
```

### Step 5: Leverage SedX Features

Take advantage of SedX's safety features:

```bash
# Use dry-run for verification
sedx --dry-run 's/foo/bar/g' *.txt

# Use interactive mode for critical changes
sedx --interactive 's/localhost/0.0.0.0/' docker-compose.yml

# Use rollback when mistakes happen
sedx rollback
```

---

## Quick Conversion Reference

### BRE to PCRE Conversion Table

| BRE Pattern | PCRE Equivalent | Notes |
|-------------|-----------------|-------|
| `\(` and `\)` | `(` and `)` | Capturing groups |
| `\{n\}` | `{n}` | Exact count |
| `\{n,m\}` | `{n,m}` | Range count |
| `\{n,\}` | `{n,}` | Minimum count |
| `\+` | `+` | One or more |
| `\?` | `?` | Zero or one |
| `\|` | `\|` | Alternation |
| `\1` in pattern | `$1` in pattern | Backreference |
| `\1` in replacement | `$1` in replacement | Backreference |
| `\&` | `$&` | Match reference |

### Common Conversion Examples

| Task | GNU sed (BRE) | SedX (PCRE) |
|------|---------------|-------------|
| Swap words | `s/\(foo\) \(bar\)/\2 \1/` | `s/(foo) (bar)/$2 $1/` |
| Duplicate | `s/\([a-z]\+\)/\1 \1/` | `s/([a-z]+)/$1 $1/` |
| Optional | `s/foo\?/maybe/` | `s/foo?/maybe/` |
| Range count | `s/[0-9]\{3,5\}/N/` | `s/[0-9]{3,5}/N/` |
| Alternation | `s/red\|green/blue/` | `s/red|green/blue/` |

---

## Getting Help

If you encounter issues migrating:

1. **Check regex mode:** Try adding `-B` flag for BRE compatibility
2. **Use dry-run:** Verify changes with `--dry-run` flag
3. **Compare output:** Use `diff` to compare GNU sed and SedX output
4. **Consult documentation:**
   - [USER_GUIDE.md](USER_GUIDE.md) - General usage
   - [EXAMPLES.md](EXAMPLES.md) - Practical examples
5. **Report issues:** https://github.com/InkyQuill/sedx/issues

### Example Debugging Workflow

```bash
# 1. Compare outputs
sed 's/foo/bar/g' file.txt > /tmp/gnu.txt
sedx 's/foo/bar/g' file.txt > /tmp/sedx.txt
diff /tmp/gnu.txt /tmp/sedx.txt

# 2. Try BRE mode
sedx -B 's/foo/bar/g' file.txt > /tmp/sedx-brex.txt
diff /tmp/gnu.txt /tmp/sedx-brex.txt

# 3. Use dry-run to see what will change
sedx --dry-run 's/foo/bar/g' file.txt
```
