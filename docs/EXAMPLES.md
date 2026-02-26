# SedX Examples

**Last Updated:** 2025-02-25
**Version:** 0.2.6

This guide provides 50+ practical examples of using SedX for common tasks in system administration, software development, and data processing.

## Table of Contents

- [System Administration](#system-administration)
- [Development Workflows](#development-workflows)
- [Data Processing](#data-processing)
- [Log File Processing](#log-file-processing)
- [Configuration Management](#configuration-management)
- [Text Manipulation](#text-manipulation)
- [Advanced Patterns](#advanced-patterns)
- [Streaming Large Files](#streaming-large-files)

---

## System Administration

### 1. Update Configuration Files

```bash
# Change database host in all config files
sedx 's/db\.host=localhost/db.host=prod-db.example.com/' config/*.toml

# Update multiple port numbers at once
sedx 's/port=300[0-9]/port=8080/' docker-compose.yml

# Change API endpoint URLs
sedx 's|https://api\.example\.com/v1|https://api.new.com/v2|g' **/*.yaml
```

### 2. Clean Log Files

```bash
# Remove all debug log entries
sedx '/DEBUG/d' /var/log/app.log

# Keep only error lines
sedx '/ERROR/!d' /var/log/app.log

# Remove specific date range (lines 1000-5000)
sedx '1000,5000d' huge.log

# Remove lines older than a date (from January 2024)
sedx '/2024-01/,/2024-02/d' logs/archive.log
```

### 3. Sanitize Sensitive Data

```bash
# Redact passwords in logs
sedx 's/password=[^ ]*/password=REDACTED/g' access.log

# Mask email addresses
sedx -E 's/([a-zA-Z0-9._%+-]+)@([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})/***@\2/g' emails.txt

# Hide IP addresses
sedx -E 's/[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}/xxx.xxx.xxx.xxx/g' logs/*.log

# Remove API keys
sedx 's/api_key=[a-zA-Z0-9]*/api_key=HIDDEN/g' config.txt
```

### 4. Comment/Uncomment Configuration

```bash
# Comment out all SELinux settings
sedx '/^SELINUX=/s/^/# /' /etc/selinux/config

# Uncomment a specific setting
sedx '/^# *ServerName/s/^# //' /etc/httpd/conf/httpd.conf

# Disable all cron jobs temporarily
sedx '/^[^#]/s/^/# /' /etc/crontab

# Enable commented module in config
sedx '/^# *LoadModule rewrite_module/s/^# //' httpd.conf
```

### 5. User Management

```bash
# Add prefix to all usernames in passwd file
sedx 's/^([^:]+)/test_\1/' /etc/passwd > /tmp/passwd.test

# Change home directory prefix
sedx 's|/home|/data/home|g' /etc/passwd

# Update shell paths
sedx 's|/bin/bash|/usr/bin/zsh|g' /etc/passwd

# Remove specific user from group file
sedx '/^admin:/s/,user2//' /etc/group
```

### 6. Service Configuration

```bash
# Disable a service in systemd directory
sedx '/^Enabled/s/true/false/' /etc/systemd/system/service.conf

# Change service user
sedx '/^User=/s/root/serviceuser/' /etc/systemd/system/app.service

# Update log paths
sedx 's|/var/log/app|/data/logs/app|g' systemd/*.service
```

---

## Development Workflows

### 7. Refactor Code

```bash
# Rename function across all Rust files
sedx 's/old_function_name/new_function_name/g' src/**/*.rs

# Update import paths in Python
sedx 's|from old\.module|from new.module|g' **/*.py

# Change method calls in JavaScript
sedx 's/\.count(/\.length(/g' src/**/*.js

# Rename class in C++ files
sedx 's/ClassName/NewClassName/g' src/*.{h,cpp}
```

### 8. Fix Common Code Issues

```bash
# Fix trailing whitespace
sedx 's/[[:space:]]*$//' **/*.py

# Convert tabs to spaces (4 spaces)
sedx 's/\t/    /g' Makefile

# Fix line endings (CRLF to LF)
sedx 's/\r$//' *.txt

# Remove empty lines
sedx '/^$/d' source.py
```

### 9. Add License Headers

```bash
# Add MIT license to all source files
for file in src/**/*.rs; do
    sedx '1i\// Copyright (c) 2025 My Company\n// SPDX-License-Identifier: MIT\n' "$file"
done

# Add shebang to Python scripts
sedx '1i\#!/usr/bin/env python3' script.py && chmod +x script.py
```

### 10. Update Version Numbers

```bash
# Update version in all package files
sedx 's/version: "1\.0\.0"/version: "2.0.0"/g' pubspec.yaml

# Increment version numbers
sedx -E 's/version=([0-9]+)\.([0-9]+)\.([0-9]+)/version=\1.\2.100/g' version.txt

# Update dependency versions
sedx 's/sedx: >=0\.1\.0/sedx: >=0.2.0/g' pubspec.yaml
```

### 11. String Escaping and Quoting

```bash
# Escape single quotes in strings
sedx "s/'/'\\\\''/g" input.txt

# Convert double quotes to single quotes
sedx 's/"([^"]*)"/'\1'/g' source.py

# Remove quotes from CSV fields
sedx 's/"//g' data.csv
```

### 12. Documentation Updates

```bash
# Update copyright year in all files
sedx 's/Copyright (c) 2024/Copyright (c) 2025/g' **/*.md

# Update API documentation URLs
sedx 's|https://docs\.old\.com|https://docs.new.com|g' README.md

# Replace TODO with FIXME
sedx 's/TODO/FIXME/g' src/**/*.ts
```

---

## Data Processing

### 13. CSV Manipulation

```bash
# Replace comma with tab (CSV to TSV)
sedx 's/,/\t/g' data.csv > data.tsv

# Extract specific column (3rd column)
sedx -E 's/^([^,]*,){2}([^,]*).*/\2/' data.csv

# Remove quotes from all CSV fields
sedx 's/"//g' data.csv > data_noquotes.csv

# Add header to CSV file
sedx '1i\Name,Email,Phone' contacts.csv
```

### 14. Text Transformation

```bash
# Convert to title case (first letter of each word)
sedx -E 's/\b([a-z])/\U\1/g' input.txt

# Uppercase specific words
sedx -E 's/\b(error|warning|critical)\b/\U\1/g' logs/*.log

# Remove duplicate words
sedx -E 's/([a-z]+) \1/\1/g' text.txt

# Swap two words
sedx -E 's/([a-z]+) ([a-z]+)/\2 \1/g' names.txt
```

### 15. Number Formatting

```bash
# Add thousand separators to numbers
sedx -E 's/\b([0-9]+)([0-9]{3})\b/\1,\2/g' numbers.txt

# Format phone numbers
sedx -E 's/([0-9]{3})([0-9]{3})([0-9]{4})/(\1) \2-\3/g' phones.txt

# Format dates (MM/DD/YYYY to YYYY-MM-DD)
sedx -E 's/([0-9]{2})/([0-9]{2})/([0-9]{4})/\3-\1-\2/g' dates.txt
```

### 16. Data Extraction

```bash
# Extract email addresses from text
sedx -n -E 's/.*([a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}).*/\1/p' emails.txt

# Extract URLs from HTML
sedx -n 's/.*\(https\?:\/\/[^ "]*\).*/\1/p' webpage.html

# Extract IP addresses
sedx -n -E 's/.*([0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}).*/\1/p' logs/*.log

# Extract JSON values
sedx -n 's/.*"name": "\([^"]*\)".*/\1/p' data.json
```

### 17. Data Validation

```bash
# Validate and fix email formats
sedx -E 's/([a-zA-Z0-9._%+-]+)@([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})/\L\1@\2/g' emails.txt

# Remove non-numeric characters from phone numbers
sedx -E 's/[^0-9]//g' phones.txt

# Standardize date formats
sedx -E 's|([0-9]{1,2})/([0-9]{1,2})/([0-9]{4})|\3-\1-\2|g' dates.txt
```

### 18. File Content Reorganization

```bash
# Reverse line order of file
sedx '1!G; h; $!d' input.txt > reversed.txt

# Duplicate every line
sedx 'G' input.txt

# Add line numbers
sedx '=' input.txt | sedx 'N; s/\n/\t/' > numbered.txt

# Interleave two files
sedx 'R file2' file1 > merged.txt
```

---

## Log File Processing

### 19. Log Analysis

```bash
# Extract only error messages with context
sedx '/ERROR/{g; 1!p;}; h' app.log

# Show 3 lines before each error
sedx '/ERROR/!{;h;};x; /ERROR/!{x;1d;};x;1!H;/ERROR/{;x;s/\n/:/g;p;}' app.log

# Count occurrences of patterns
sedx -n 's/.*ERROR.*/error/p' app.log | wc -l
```

### 20. Log Filtering

```bash
# Remove all but last 1000 lines
sedx -e :a -e '$q;N;1001,$D;ba' large.log

# Keep only lines from specific hour
sedx '/2025-02-25 14:/!d' app.log

# Filter logs by multiple patterns
sedx '/ERROR\|WARNING\|CRITICAL/!d' app.log

# Remove duplicate log entries
sedx '$!N; /^\(.*\)\n\1$/!P; D' app.log
```

### 21. Log Transformation

```bash
# Convert log timestamps to ISO format
sedx -E 's|([0-9]{2})/([0-9]{2})/([0-9]{4})|\3-\1-\2|g' app.log

# Anonymize user IDs in logs
sedx -E 's/user_id=[0-9]+/user_id=XXX/g' app.log

# Format JSON logs for readability
sedx 's/,/,\n/g' logs/*.json | sedx 's/{/{\n/g' | sedx 's/}/\n}/g'
```

### 22. Apache/Nginx Log Processing

```bash
# Extract IP addresses from access logs
sedx -n -E 's/^([0-9.]+) .*/\1/p' access.log

# Count requests by status code
sedx -n 's/.*" \([0-9]\{3\} .*/\1/p' access.log | sort | uniq -c

# Extract user agents
sedx -n 's/.*"\([^"]*\)"$/\1/p' access.log
```

---

## Configuration Management

### 23. Environment Variables

```bash
# Set environment variables in .env file
sedx 's/DEBUG=false/DEBUG=true/g' .env

# Comment out production settings
sedx '/^PRODUCTION/s/^/# /' .env

# Update database URL
sedx 's|DATABASE_URL=.*|DATABASE_URL=postgresql://localhost/db|g' .env
```

### 24. Docker Configuration

```bash
# Change exposed ports
sedx 's/8080:80/80:80/g' docker-compose.yml

# Update image versions
sedx 's|image: nginx:.*|image: nginx:1.25|g' docker-compose.yml

# Change volume mount paths
sedx 's|./data|/data/app|g' docker-compose.yml

# Add environment variables to service
sedx '/environment:/a\  - NEW_VAR=value' docker-compose.yml
```

### 25. Kubernetes YAML

```bash
# Update replica count
sedx 's/replicas: [0-9]/replicas: 3/g' deployment.yaml

# Change image tag
sedx 's|image:.*:.*|image: myapp:v2.0.0|g' deployment.yaml

# Update resource limits
sedx 's/memory: "256Mi"/memory: "512Mi"/g' deployment.yaml
```

### 26. SSH Configuration

```bash
# Add new host to SSH config
sedx '$a\\nHost myserver\n  HostName example.com\n  User admin' ~/.ssh/config

# Update hostname for existing host
sedx '/^Host old/,/^$/{s/HostName .*/HostName new.example.com/}' ~/.ssh/config

# Change identity file
sedx '/^Host myserver/,/^$/{s|IdentityFile .*|IdentityFile ~/.ssh/new_key|}' ~/.ssh/config
```

### 27. Git Configuration

```bash
# Update user email
sedx 's/email = .*/email = new@example.com/' ~/.gitconfig

# Change default branch name
sedx 's/initialBranch = .*/initialBranch = main/' ~/.gitconfig

# Add new alias
sedx '$a\\n[alias]\n  st = status' .gitconfig
```

---

## Text Manipulation

### 28. Find and Replace

```bash
# Simple case-sensitive replacement
sedx 's/foo/bar/g' file.txt

# Case-insensitive replacement
sedx 's/foo/bar/gi' file.txt

# Replace only whole words
sedx -E 's/\bfoo\b/bar/g' file.txt

# Replace only at line start
sedx 's/^foo/bar/g' file.txt

# Replace only at line end
sedx 's/foo$/bar/g' file.txt
```

### 29. Multi-line Operations

```bash
# Join pairs of lines
sedx 'N; s/\n/ /' file.txt

# Delete blank lines
sedx '/^$/d' file.txt

# Remove consecutive blank lines
sedx '/./,/^$/!d' file.txt

# Add blank line after each line
sedx 'G' file.txt
```

### 30. Text Formatting

```bash
# Remove leading whitespace
sedx 's/^[[:space:]]*//' file.txt

# Remove trailing whitespace
sedx 's/[[:space:]]*$//' file.txt

# Center text (add 40 spaces at start, then trim)
sedx 's/^/                                        /; s/^ \{0,40\}//' file.txt

# Indent lines by 4 spaces
sedx 's/^/    /' file.txt
```

### 31. HTML/XML Processing

```bash
# Remove HTML tags
sedx 's/<[^>]*>//g' page.html

# Extract HTML attributes
sedx -n 's/.*href="\([^"]*\)".*/\1/p' links.html

# Format XML (add newlines after tags)
sedx 's/></>\n</g' data.xml

# Remove comments from HTML
sedx '/<!--.*-->/d' page.html
```

### 32. Markdown Processing

```bash
# Convert headers to uppercase
sedx '/^#/ s/\([A-Za-z]\+\)/\U\1/g' README.md

# Remove markdown emphasis
sedx 's/\*\*\([^*]*\)\*\*/\1/g' README.md

# Add markdown header to file
sedx '1i\# Document Title\n' document.md

# Number markdown headers
sedx '/^##/ s/## /## 1. /' document.md
```

---

## Advanced Patterns

### 33. Flow Control

```bash
# Loop until no more substitutions
sedx ':loop; s/foo/bar/g; t loop' file.txt

# Conditional branch based on pattern
sedx '/error/b skip; s/normal/processed/; b; :skip; s/error/ERROR/' file.txt

# Count lines matching pattern
sedx -n '/pattern/p' file.txt | wc -l
```

### 34. Hold Space Operations

```bash
# Move first line to end of file
sedx '1h; 1d; $G' file.txt

# Delete first line and append at end
sedx '1h; 1d; $G' file.txt

# Swap adjacent lines
sedx 'N; s/\(.*\)\n\(.*\)/\2\n\1/' file.txt

# Duplicate each line 3 times
sedx 'G; G' file.txt
```

### 35. Pattern Spaces

```bash
# Process two lines at once
sedx 'N; s/\n/ /' file.txt

# Delete line before pattern
sedx '/pattern/{x; d;}; x' file.txt

# Insert line before pattern
sedx '/pattern/{x; p; x;}' file.txt
```

### 36. Complex Addressing

```bash
# Process every 5th line
sedx 'n;n;n;n;p' file.txt

# Delete lines 5-10 from every 20-line block
sedx '20,~5d' file.txt

# Substitute on lines matching two patterns
sedx '/foo/{/bar/s/baz/qux/}' file.txt
```

### 37. Multiple File Processing

```bash
# Apply same operation to multiple files
sedx 's/foo/bar/g' *.txt

# Process files recursively
find . -name "*.log" -exec sedx 's/old/new/g' {} \;

# Backup and process
for f in *.conf; do
    sedx 's/localhost/127.0.0.1/g' "$f"
done
```

### 38. Conditional Substitution

```bash
# Substitute only on lines NOT matching pattern
sedx '/keep/!s/foo/bar/g' file.txt

# Substitute only if line contains specific words
sedx '/error.*critical/s/.*/URGENT: &/' file.txt

# Chain conditions
sedx '/error/{s/^/E: /; /warning/b}; s/^/I: /' file.txt
```

---

## Streaming Large Files

### 39. Process Large Files Efficiently

SedX automatically uses streaming mode for files >= 100MB:

```bash
# This will automatically stream (constant memory usage)
sedx 's/foo/bar/g' large_file.log

# Force streaming for smaller files
sedx --streaming 's/pattern/replacement/g' file.txt
```

### 40. Process Huge Logs in Chunks

```bash
# Process first 1000 lines only
sedx '1000q' huge.log

# Process and quit at pattern
sedx '/ERROR QUIT/q' huge.log

# Stream specific lines from huge file
sedx -n '1000000,1000100p' huge.log
```

### 41. Extract Data from Large Files

```bash
# Extract first million lines
sedx '1000000q' huge_file.csv > first_million.csv

# Extract specific line range from large file
sedx -n '1000,2000p' large.log > extract.log

# Stream process and count matches
sedx -n '/pattern/p' huge.log | wc -l
```

### 42. Safe Large File Operations

```bash
# Preview before processing large file
sedx --dry-run 's/sensitive/REDACTED/g' huge.log

# Interactive mode for critical changes
sedx --interactive 's/prod/test/g' production.log

# Process with immediate rollback option
sedx 's/update/delete/g' *.log  # Use rollback if needed
```

### 43. Batch Processing Large Datasets

```bash
# Process directory of large files
for file in data/*.csv; do
    echo "Processing $file..."
    sedx 's/foo/bar/g' "$file"
done

# Parallel processing with xargs
find . -name "*.log" -print0 | xargs -0 -P4 -I{} sedx 's/old/new/g' {}

# Process and archive
sedx 's/active/archived/g' logs/*.log && mv logs/*.log archive/
```

### 44. Memory-Efficient Transformations

```bash
# Remove duplicates from large file (memory efficient)
sedx -n 'G; s/\n/&&/; /^\([^\n]*\).*\n\1$/d; s/\n//; h; P' huge.txt | sedx 's/&&/\n/' > unique.txt

# Sort and deduplicate (stream-friendly)
sedx 's/$/\n/' huge.txt | sort -u | sedx '/^$/d' > sorted_unique.txt

# Extract unique patterns from large file
sedx -n 's/.*pattern: \([^ ]*\).*/\1/p' huge.log | sort -u > patterns.txt
```

### 45. Large File Backup Strategy

```bash
# Use custom backup directory for large files
sedx --backup-dir /mnt/backups 's/old/new/g' huge_file.dat

# Skip backup for temporary large files
sedx --no-backup --force 's/temp/final/g' huge_file.tmp

# Monitor backup status
sedx status
sedx backup list | grep huge_file
```

---

## Quick Reference Patterns

### Common Substitutions

```bash
# Remove digits
sedx 's/[0-9]//g' file.txt

# Remove non-alphanumeric characters
sedx 's/[^a-zA-Z0-9]//g' file.txt

# Swap two patterns
sedx -E 's/(foo) (bar)/\2 \1/g' file.txt

# Insert text at line start
sedx 's/^/PREFIX: /' file.txt

# Append text at line end
sedx 's/$/ :SUFFIX/' file.txt
```

### Common Deletions

```bash
# Delete empty lines
sedx '/^$/d' file.txt

# Delete comment lines
sedx '/^#/d' file.txt

# Delete lines shorter than 5 characters
sedx '/^.\{0,4\}$/d' file.txt

# Delete lines containing only spaces
sedx '/^[[:space:]]*$/d' file.txt
```

### Common Prints

```bash
# Print first 10 lines
sedx -n '1,10p' file.txt

# Print last 5 lines
sedx -n '$-4,$p' file.txt

# Print lines matching pattern
sedx -n '/pattern/p' file.txt

# Print even lines
sedx -n 'n;p' file.txt
```

---

## Tips and Best Practices

1. **Always dry-run first:** Use `--dry-run` to preview changes
2. **Use rollback:** If something goes wrong, `sedx rollback` is your friend
3. **Quote properly:** Use single quotes for sed expressions to avoid shell expansion
4. **Test on samples:** Create a small test file before processing large datasets
5. **Check backups:** Use `sedx status` to monitor backup disk usage
6. **Use appropriate regex mode:** PCRE (default), ERE (`-E`), or BRE (`-B`)
7. **Leverage streaming:** Large files are automatically processed with constant memory

---

## Further Reading

- [USER_GUIDE.md](USER_GUIDE.md) - Complete SedX documentation
- [MIGRATION_GUIDE.md](MIGRATION_GUIDE.md) - Migrating from GNU sed
- [GitHub Issues](https://github.com/InkyQuill/sedx/issues) - Report bugs and request features
