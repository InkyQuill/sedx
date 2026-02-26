#!/bin/bash
# Test debug logging functionality

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SEDX="$SCRIPT_DIR/../target/release/sedx"
TEST_FILE="/tmp/sedx_log_test.txt"
LOG_FILE="$HOME/.sedx/sedx.log"

# Backup original config
CONFIG_BACKUP="$HOME/.sedx/config.toml.bak"
if [ -f "$HOME/.sedx/config.toml" ]; then
    cp "$HOME/.sedx/config.toml" "$CONFIG_BACKUP"
fi

# Cleanup function
cleanup() {
    rm -f "$TEST_FILE" "$LOG_FILE"
    if [ -f "$CONFIG_BACKUP" ]; then
        mv "$CONFIG_BACKUP" "$HOME/.sedx/config.toml"
    fi
}
trap cleanup EXIT

echo "=== Testing debug logging ==="

# Create config with debug enabled
mkdir -p "$HOME/.sedx"
cat > "$HOME/.sedx/config.toml" << 'EOF'
[backup]
max_size_gb = 2
max_disk_usage_percent = 60

[compatibility]
mode = "pcre"
show_warnings = true

[processing]
context_lines = 2
max_memory_mb = 100
streaming = true
debug = true
EOF

echo "✓ Config file created with debug=true"

# Create test file
echo -e "foo\nbar\nbaz" > "$TEST_FILE"

# Run sedx
echo "Running: sedx 's/foo/bar/' test file"
"$SEDX" 's/foo/bar/' "$TEST_FILE" > /dev/null 2>&1

# Check if log file exists
if [ ! -f "$LOG_FILE" ]; then
    echo "FAIL: Log file was not created"
    exit 1
fi

echo "✓ Log file created at $LOG_FILE"

# Check log content
if ! grep -q "Operation started" "$LOG_FILE"; then
    echo "FAIL: Expected 'Operation started' in log"
    cat "$LOG_FILE"
    exit 1
fi

echo "✓ Log contains 'Operation started'"

if ! grep -q "Operation completed" "$LOG_FILE"; then
    echo "FAIL: Expected 'Operation completed' in log"
    cat "$LOG_FILE"
    exit 1
fi

echo "✓ Log contains 'Operation completed'"

# Test with invalid expression (error logging)
echo "Testing error logging..."
"$SEDX" '[invalid' "$TEST_FILE" > /dev/null 2>&1 || true

if ! grep -q "Failed to parse expression" "$LOG_FILE"; then
    echo "FAIL: Expected error log entry"
    cat "$LOG_FILE"
    exit 1
fi

echo "✓ Error was logged"

# Test stdin mode logging
echo "Testing stdin mode logging..."
echo "hello world" | "$SEDX" 's/hello/HELLO/' > /dev/null

if ! grep -q "Stdin processing" "$LOG_FILE"; then
    echo "FAIL: Expected 'Stdin processing' in log"
    cat "$LOG_FILE"
    exit 1
fi

echo "✓ Stdin processing was logged"

# Test --log-path command
LOG_PATH_OUTPUT=$("$SEDX" config --log-path)
if ! echo "$LOG_PATH_OUTPUT" | grep -q "Path:"; then
    echo "FAIL: config --log-path output incorrect"
    echo "$LOG_PATH_OUTPUT"
    exit 1
fi

echo "✓ config --log-path works"

# Test config --show shows debug setting
CONFIG_OUTPUT=$("$SEDX" config --show)
if ! echo "$CONFIG_OUTPUT" | grep -q "debug = true"; then
    echo "FAIL: config --show doesn't show debug setting"
    echo "$CONFIG_OUTPUT"
    exit 1
fi

echo "✓ config --show shows debug setting"

echo ""
echo "=== All logging tests passed ==="
