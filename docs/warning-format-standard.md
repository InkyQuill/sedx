# SedX Warning and Error Message Format Standard

## Overview

This document defines the standard format for all user-facing warning, error, and informational messages in SedX. Consistent messaging improves user experience and makes the codebase easier to maintain.

## Severity Levels

| Level | Icon | Description | Example Usage |
|-------|------|-------------|---------------|
| **Info** | `ℹ️  Info:` | Informational messages, no action required | Streaming mode activated, configuration values |
| **Warn** | `⚠️  Warning:` | Warning messages, action recommended | Large backup size, incompatibility issues |
| **Error** | `❌ Error:` | Error messages, action required | File not found, invalid syntax |

## Message Format Structure

All diagnostic messages should follow this structure:

```
<SEVERITY>: <SUMMARY>
   <DETAILS>
   Suggestion: <ACTIONABLE_SUGGESTION>
   Location: <WHERE_IT_HAPPENED>
   Context: <DEBUGGING_CONTEXT>  (optional)
```

### Components

1. **Severity Indicator**: Icon + level name
2. **Summary**: Brief one-line description of the issue
3. **Details**: What happened and why (1-2 sentences)
4. **Suggestion**: What the user can do to fix or work around the issue
5. **Location**: File path, line number, or other location info
6. **Context**: Additional debugging information (optional)

## Examples

### Large Backup Warning
```
⚠️  Warning: Large backup size (4.2 GB)
   This operation will create a backup larger than the recommended size.
   Suggestion: Use --no-backup if you have a recent backup
   Location: /path/to/large/file.dat
```

### File Read Error
```
❌ Error: Cannot read file: /path/to/file.txt
   Permission denied
   Suggestion: Check that the file exists and you have read permissions
   Location: /path/to/file.txt
```

### Undefined Label Error
```
❌ Error: Undefined label: mylabel
   A branch command references a label that was never defined.
   Suggestion: Add a label definition with ':labelname' before the branch command
```

### Streaming Mode Info
```
ℹ️  Info: Streaming mode activated
   File size (250 MB) exceeds streaming threshold (100 MB)
   Location: /path/to/large/file.log
```

## Usage Guidelines

### 1. Be Specific
Tell the user exactly what went wrong.

**Bad:**
```
⚠️  Warning: Error processing file
```

**Good:**
```
❌ Error: Cannot read file: /path/to/file.txt
   Permission denied
```

### 2. Be Actionable
Always provide a way to fix or work around the issue.

**Bad:**
```
⚠️  Warning: Disk space low
```

**Good:**
```
⚠️  Warning: Low disk space (85% used)
   Creating backups may fail due to insufficient disk space.
   Suggestion: Free up disk space or use --no-backup to skip backup creation
```

### 3. Be Concise
Keep messages to 2-4 lines when possible.

**Bad:**
```
⚠️  Warning: The backup operation you are attempting to perform will create a backup
that is larger than the recommended maximum size which might cause performance issues
and disk space problems in the future.
```

**Good:**
```
⚠️  Warning: Large backup size (4.2 GB)
   This operation will create a backup larger than the recommended size.
   Suggestion: Use --no-backup if you have a recent backup
```

### 4. Use Consistent Terminology
Use the same words for the same concepts throughout the codebase.

| Concept | Standard Term |
|---------|---------------|
| Backup directory | `~/.sedx/backups/` or backup directory |
| Streaming mode | Streaming mode |
| Pattern match | Pattern match |
| Line number | Line N (1-indexed) |
| Regex flavor | PCRE, ERE, or BRE mode |

### 5. Respect User Preferences
Check the `show_warnings` config setting before printing warnings.

```rust
// Always print errors
if severity == Severity::Error {
    diagnostic.print();
}
// Print warnings only if enabled
else if config.show_warnings.unwrap_or(true) {
    diagnostic.print();
}
```

## Implementation

### Using the Diagnostic Type

The `warning` module provides a `Diagnostic` type for structured messages:

```rust
use crate::warning::Diagnostic;
use crate::warning::Severity;

// Create a diagnostic
let diag = Diagnostic::new(Severity::Warn, "Large backup size")
    .with_details("This operation will create a large backup.")
    .with_suggestion("Use --no-backup to skip")
    .with_location("/path/to/file");

// Print it
diag.print();

// Or print only if warnings are enabled
diag.print_if_enabled(&config);
```

### Convenience Constructors

Common diagnostic scenarios have helper constructors:

```rust
// Large backup
Diagnostic::large_backup(size_bytes, path).print();

// Backup file missing
Diagnostic::backup_file_missing(path).print();

// File read error
Diagnostic::file_read_error(path, error_msg).print();

// Undefined label
Diagnostic::undefined_label("label_name").print();

// And more...
```

## Configuration

The `show_warnings` option in `~/.sedx/config.toml` controls warning output:

```toml
[compatibility]
show_warnings = true  # Default: true
```

## Current Warning Locations

| File | Current Warnings | Status |
|------|------------------|--------|
| `backup_manager.rs` | Large backup, backup file missing | Needs update |
| `main.rs` | File read error, streaming mode info | Needs update |
| `file_processor.rs` | Undefined label | Needs update |
| `config.rs` | Invalid config values | Needs update |
| `disk_space.rs` | Disk space warning | Needs update |
| `cli.rs` | Empty script warning | Needs update |

## Migration Checklist

When updating existing warnings to the new format:

- [ ] Import `use crate::warning::{Diagnostic, Severity};`
- [ ] Replace `eprintln!()` with `Diagnostic` constructors
- [ ] Add structured fields (details, suggestion, location)
- [ ] Use appropriate severity level
- [ ] Test with `show_warnings = false`
- [ ] Update this document with new warning types

## Testing

Test warning output with different configurations:

```bash
# Show all warnings
sedx 's/foo/bar/g' file.txt

# Suppress warnings
sedx config set compatibility.show_warnings false
sedx 's/foo/bar/g' file.txt

# Restore defaults
sedx config set compatibility.show_warnings true
```

## Related Documentation

- `src/warning.rs` - Warning module implementation
- `src/error_helpers.rs` - IO error enhancement helpers
- `CLAUDE.md` - Project overview and architecture
- `docs/configuration.md` - Config file reference
