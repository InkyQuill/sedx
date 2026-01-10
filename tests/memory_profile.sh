#!/bin/bash
# Memory Profiling Test for SedX Streaming Mode
# Tests constant-memory processing of large files

set +H

SEDX="./target/release/sedx"
TEMP_DIR="/tmp/sedx_memory_tests"
mkdir -p "$TEMP_DIR"

echo "========================================"
echo "  SedX Memory Profiling Tests"
echo "========================================"
echo ""

# Check if /usr/bin/time is available (Linux)
if ! command -v /usr/bin/time &> /dev/null; then
    echo "Error: /usr/bin/time not found. This script requires GNU time."
    echo "Install with: sudo apt-get install time"
    exit 1
fi

# Test 1: 100MB file
echo -e "\n📊 Test 1: 100MB file substitution"
echo "-------------------------------------------"
dd if=/dev/zero of="$TEMP_DIR/test_100mb.txt" bs=1M count=100 2>/dev/null
echo "test line foo" >> "$TEMP_DIR/test_100mb.txt"

echo "File size: $(du -h "$TEMP_DIR/test_100mb.txt" | cut -f1)"
echo ""
echo "Running: sedx 's/foo/bar/' test_100mb.txt"
/usr/bin/time -v $SEDX 's/foo/bar/' "$TEMP_DIR/test_100mb.txt" > /dev/null 2>&1 | grep "Maximum resident set size"

# Verify it worked
if grep -q "test line bar" "$TEMP_DIR/test_100mb.txt"; then
    echo "✓ Substitution successful"
else
    echo "✗ Substitution failed"
fi

# Test 2: 1GB file (if disk space available)
echo -e "\n📊 Test 2: 1GB file substitution"
echo "-------------------------------------------"
# Check available disk space
available_space=$(df "$TEMP_DIR" | tail -1 | awk '{print $4}')
required_space=$((2 * 1024 * 1024)) # 2GB in KB

if [ "$available_space" -gt "$required_space" ]; then
    dd if=/dev/zero of="$TEMP_DIR/test_1gb.txt" bs=1M count=1024 2>/dev/null
    echo "test line foo" >> "$TEMP_DIR/test_1gb.txt"

    echo "File size: $(du -h "$TEMP_DIR/test_1gb.txt" | cut -f1)"
    echo ""
    echo "Running: sedx 's/foo/bar/' test_1gb.txt"
    /usr/bin/time -v $SEDX 's/foo/bar/' "$TEMP_DIR/test_1gb.txt" > /dev/null 2>&1 | grep "Maximum resident set size"

    # Verify it worked
    if grep -q "test line bar" "$TEMP_DIR/test_1gb.txt"; then
        echo "✓ Substitution successful"
    else
        echo "✗ Substitution failed"
    fi

    rm -f "$TEMP_DIR/test_1gb.txt"
else
    echo "⚠ Skipping 1GB test (insufficient disk space)"
    echo "   Required: 2GB, Available: $((available_space / 1024 / 1024))GB"
fi

# Test 3: Pattern deletion on 100MB file
echo -e "\n📊 Test 3: Pattern deletion on 100MB file"
echo "-------------------------------------------"
dd if=/dev/zero of="$TEMP_DIR/test_delete.txt" bs=1M count=100 2>/dev/null
echo "keep this line" >> "$TEMP_DIR/test_delete.txt"
echo "delete this line foo" >> "$TEMP_DIR/test_delete.txt"
echo "keep this too" >> "$TEMP_DIR/test_delete.txt"

echo "File size: $(du -h "$TEMP_DIR/test_delete.txt" | cut -f1)"
echo ""
echo "Running: sedx '/foo/d' test_delete.txt"
/usr/bin/time -v $SEDX '/foo/d' "$TEMP_DIR/test_delete.txt" > /dev/null 2>&1 | grep "Maximum resident set size"

# Count remaining lines
remaining=$(grep -c "keep this" "$TEMP_DIR/test_delete.txt" || true)
if [ "$remaining" -eq 2 ]; then
    echo "✓ Correct lines kept (2 lines)"
else
    echo "✗ Wrong line count (got $remaining, expected 2)"
fi

# Test 4: Complex group operations
echo -e "\n📊 Test 4: Complex group operations on 100MB"
echo "-------------------------------------------"
dd if=/dev/zero of="$TEMP_DIR/test_group.txt" bs=1M count=100 2>/dev/null
echo "foo bar baz" >> "$TEMP_DIR/test_group.txt"

echo "File size: $(du -h "$TEMP_DIR/test_group.txt" | cut -f1)"
echo ""
echo "Running: sedx '{s/foo/FOO/; s/bar/BAR/; s/baz/BAZ/}'"
/usr/bin/time -v $SEDX '{s/foo/FOO/; s/bar/BAR/; s/baz/BAZ/}' "$TEMP_DIR/test_group.txt" > /dev/null 2>&1 | grep "Maximum resident set size"

if grep -q "FOO BAR BAZ" "$TEMP_DIR/test_group.txt"; then
    echo "✓ All substitutions successful"
else
    echo "✗ Substitutions failed"
fi

# Summary
echo ""
echo "========================================"
echo "  Summary"
echo "========================================"
echo ""
echo "Expected memory usage: <100MB for all file sizes"
echo ""
echo "If memory usage scales with file size, streaming mode is not working."
echo "Memory should stay constant regardless of file size."
echo ""

# Cleanup
rm -f "$TEMP_DIR"/test_*.txt

echo "✓ Memory profiling tests completed"
