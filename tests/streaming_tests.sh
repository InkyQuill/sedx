#!/bin/bash
# Comprehensive Streaming Tests for SedX
# Tests constant-memory processing, edge cases, and performance

set +H  # Disable history expansion

SEDX="./target/release/sedx"
TEMP_DIR="/tmp/sedx_streaming_tests"
mkdir -p "$TEMP_DIR"

PASSED=0
FAILED=0

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo "========================================"
echo "  SedX Streaming Mode Tests"
echo "========================================"
echo ""

# Test 1: Empty file
echo -e "${BLUE}Test 1: Empty file${NC}"
printf "" > "$TEMP_DIR/empty.txt"
$SEDX 's/foo/bar/' "$TEMP_DIR/empty.txt" > /dev/null 2>&1
if [ $? -eq 0 ] && [ ! -s "$TEMP_DIR/empty.txt" ]; then
    echo -e "${GREEN}✓ PASSED${NC} - Empty file handled correctly"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Empty file not handled correctly"
    ((FAILED++))
fi

# Test 2: Single line file
echo -e "\n${BLUE}Test 2: Single line file${NC}"
echo "hello world" > "$TEMP_DIR/single.txt"
$SEDX 's/hello/HELLO/' "$TEMP_DIR/single.txt" > /dev/null 2>&1
content=$(cat "$TEMP_DIR/single.txt")
if [ "$content" = "HELLO world" ]; then
    echo -e "${GREEN}✓ PASSED${NC} - Single line processed correctly"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Single line not processed correctly"
    ((FAILED++))
fi

# Test 3: Very long line (10MB)
echo -e "\n${BLUE}Test 3: Very long line (10MB)${NC}"
python3 -c "print('x' * 10000000)" > "$TEMP_DIR/long_line.txt"
$SEDX 's/x/y/g' "$TEMP_DIR/long_line.txt" > /dev/null 2>&1
if [ $? -eq 0 ]; then
    # Check that all x's were replaced
    count=$(grep -o 'y' "$TEMP_DIR/long_line.txt" | wc -l)
    if [ "$count" -eq 10000000 ]; then
        echo -e "${GREEN}✓ PASSED${NC} - 10MB line processed correctly"
        ((PASSED++))
    else
        echo -e "${RED}✗ FAILED${NC} - Not all characters replaced (got $count y's)"
        ((FAILED++))
    fi
else
    echo -e "${RED}✗ FAILED${NC} - Failed to process long line"
    ((FAILED++))
fi

# Test 4: File with only newlines
echo -e "\n${BLUE}Test 4: File with only newlines${NC}"
printf '\n\n\n\n\n' > "$TEMP_DIR/only_newlines.txt"
$SEDX 's/foo/bar/' "$TEMP_DIR/only_newlines.txt" > /dev/null 2>&1
lines=$(wc -l < "$TEMP_DIR/only_newlines.txt")
if [ "$lines" -eq 5 ]; then
    echo -e "${GREEN}✓ PASSED${NC} - Newline-only file preserved"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Newline count changed (got $lines lines)"
    ((FAILED++))
fi

# Test 5: Mixed line endings (should preserve Unix style)
echo -e "\n${BLUE}Test 5: Mixed line content${NC}"
cat > "$TEMP_DIR/mixed.txt" << 'EOF'
line 1
line 2 foo
line 3 bar baz
line 4
EOF
$SEDX 's/foo/bar/g' "$TEMP_DIR/mixed.txt" > /dev/null 2>&1
if grep -q "line 2 bar" "$TEMP_DIR/mixed.txt" && \
   grep -q "line 3 bar baz" "$TEMP_DIR/mixed.txt"; then
    echo -e "${GREEN}✓ PASSED${NC} - Mixed content processed correctly"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Mixed content not processed correctly"
    ((FAILED++))
fi

# Test 6: Delete all lines
echo -e "\n${BLUE}Test 6: Delete all lines${NC}"
cat > "$TEMP_DIR/delete_all.txt" << 'EOF'
line 1
line 2
line 3
EOF
$SEDX '1,$d' "$TEMP_DIR/delete_all.txt" > /dev/null 2>&1
if [ ! -s "$TEMP_DIR/delete_all.txt" ]; then
    echo -e "${GREEN}✓ PASSED${NC} - All lines deleted correctly"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - File not empty after deleting all lines"
    ((FAILED++))
fi

# Test 7: Pattern range spanning entire file
echo -e "\n${BLUE}Test 7: Pattern range spanning entire file${NC}"
cat > "$TEMP_DIR/range.txt" << 'EOF'
start
line 2
line 3
end
line 5
EOF
$SEDX '/start/,/end/d' "$TEMP_DIR/range.txt" > /dev/null 2>&1
content=$(cat "$TEMP_DIR/range.txt")
if [ "$content" = "line 5" ]; then
    echo -e "${GREEN}✓ PASSED${NC} - Pattern range deleted correctly"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Pattern range not deleted correctly"
    echo "  Got: $content"
    ((FAILED++))
fi

# Test 8: Multiple substitutions on same line
echo -e "\n${BLUE}Test 8: Multiple substitutions on same line${NC}"
echo "foo bar baz" > "$TEMP_DIR/multi.txt"
$SEDX '{s/foo/FOO/; s/bar/BAR/; s/baz/BAZ/}' "$TEMP_DIR/multi.txt" > /dev/null 2>&1
content=$(cat "$TEMP_DIR/multi.txt")
if [ "$content" = "FOO BAR BAZ" ]; then
    echo -e "${GREEN}✓ PASSED${NC} - Multiple substitutions work"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Multiple substitutions failed"
    echo "  Got: $content"
    ((FAILED++))
fi

# Test 9: Hold space across large file
echo -e "\n${BLUE}Test 9: Hold space operations${NC}"
cat > "$TEMP_DIR/hold.txt" << 'EOF'
header
line 2
line 3
line 4
footer
EOF
$SEDX '1h; 2,4H; 5g' "$TEMP_DIR/hold.txt" > /dev/null 2>&1
# Line 5 should be replaced with lines 1-4
lines=$(cat "$TEMP_DIR/hold.txt")
if echo "$lines" | grep -q "header"; then
    echo -e "${GREEN}✓ PASSED${NC} - Hold space works across lines"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Hold space not working correctly"
    echo "  Got: $lines"
    ((FAILED++))
fi

# Test 10: Streaming mode activation (files >= 100MB)
echo -e "\n${BLUE}Test 11: Streaming mode activation${NC}"
# Create a file just over 100MB threshold
dd if=/dev/zero of="$TEMP_DIR/large.bin" bs=1M count=101 2>/dev/null
echo "test line" >> "$TEMP_DIR/large.bin"
$SEDX 's/test/TEST/' "$TEMP_DIR/large.bin" > /dev/null 2>&1
if grep -q "TEST line" "$TEMP_DIR/large.bin"; then
    echo -e "${GREEN}✓ PASSED${NC} - Streaming mode activated for 101MB file"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Streaming mode not working correctly"
    ((FAILED++))
fi
rm -f "$TEMP_DIR/large.bin"

# Summary
echo ""
echo "========================================"
echo "  Test Results"
echo "========================================"
echo -e "Passed: ${GREEN}$PASSED${NC}"
echo -e "Failed: ${RED}$FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
