#!/bin/bash
# Performance Benchmark: SedX vs GNU sed

set +H

SEDX="./target/release/sedx"
TEMP_DIR="/tmp/sedx_benchmarks"
mkdir -p "$TEMP_DIR"

echo "========================================"
echo "  Performance Benchmark: SedX vs GNU sed"
echo "========================================"
echo ""

# Test 1: Simple substitution on 10MB file
echo "Test 1: Simple substitution (s/foo/bar/) on 10MB file"
echo "-----------------------------------------------------------"
python3 -c "
for i in range(200000):
    print(f'line {i} with foo text')
" > "$TEMP_DIR/test_10mb.txt"

echo "File size: $(du -h "$TEMP_DIR/test_10mb.txt" | cut -f1)"
echo ""

echo "GNU sed:"
time sed 's/foo/bar/' "$TEMP_DIR/test_10mb.txt" > /dev/null

echo ""
echo "SedX (streaming):"
time $SEDX 's/foo/bar/' "$TEMP_DIR/test_10mb.txt" > /dev/null

# Test 2: Pattern deletion on 10MB file
echo ""
echo "Test 2: Pattern deletion (/pattern/d) on 10MB file"
echo "-----------------------------------------------------------"
python3 -c "
for i in range(200000):
    if i % 100 == 0:
        print(f'delete this line {i}')
    else:
        print(f'keep this line {i}')
" > "$TEMP_DIR/test_10mb_2.txt"

echo "File size: $(du -h "$TEMP_DIR/test_10mb_2.txt" | cut -f1)"
echo ""

echo "GNU sed:"
time sed '/delete this line/d' "$TEMP_DIR/test_10mb_2.txt" > /dev/null

echo ""
echo "SedX (streaming):"
time $SEDX '/delete this line/d' "$TEMP_DIR/test_10mb_2.txt" > /dev/null

# Test 3: Complex group operations
echo ""
echo "Test 3: Complex group ({s/foo/bar/; s/baz/qux/}) on 10MB"
echo "-----------------------------------------------------------"
python3 -c "
for i in range(200000):
    print(f'line {i} foo baz qux')
" > "$TEMP_DIR/test_10mb_3.txt"

echo "File size: $(du -h "$TEMP_DIR/test_10mb_3.txt" | cut -f1)"
echo ""

echo "GNU sed:"
time sed '{s/foo/bar/; s/baz/qux/}' "$TEMP_DIR/test_10mb_3.txt" > /dev/null

echo ""
echo "SedX (streaming):"
time $SEDX '{s/foo/bar/; s/baz/qux/}' "$TEMP_DIR/test_10mb_3.txt" > /dev/null

# Summary
echo ""
echo "========================================"
echo "  Summary"
echo "========================================"
echo ""
echo "Expected: SedX should be within 2x of GNU sed performance"
echo "Note: SedX creates backups and diffs, which adds overhead"
echo ""

# Cleanup
rm -f "$TEMP_DIR"/test_*.txt

echo "✓ Benchmark completed"
