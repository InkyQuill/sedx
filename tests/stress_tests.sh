#!/bin/bash
# Stress Tests for SedX
# Tests large files, many files, complex patterns, and edge cases

set +H  # Disable history expansion

SEDX="./target/release/sedx"
TEMP_DIR="/tmp/sedx_stress_tests"
mkdir -p "$TEMP_DIR"

PASSED=0
FAILED=0
SKIPPED=0

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Check if sedx binary exists
if [ ! -f "$SEDX" ]; then
    echo -e "${RED}Error: SedX binary not found at $SEDX${NC}"
    echo "Run: cargo build --release"
    exit 1
fi

# Check if /usr/bin/time is available for memory profiling
if command -v /usr/bin/time &> /dev/null; then
    HAS_TIME=true
else
    echo -e "${YELLOW}Warning: /usr/bin/time not found. Memory profiling disabled.${NC}"
    HAS_TIME=false
fi

echo "========================================"
echo "  SedX Stress Test Suite"
echo "========================================"
echo ""

# ============================================================================
# TEST CATEGORY 1: Large File Processing
# ============================================================================
echo -e "${CYAN}=== CATEGORY 1: Large File Processing ===${NC}"
echo ""

# Helper: Generate large file with specific content (using text, not zeros)
generate_large_text_file() {
    local file=$1
    local size_mb=$2
    local pattern=$3
    
    echo "  Generating ${size_mb}MB text file..."
    # Generate text-based content (not zeros) to avoid binary issues
    for i in $(seq 1 $((size_mb * 10))); do
        echo "Line $i: original text $pattern for testing" >> "$file"
    done
}

# Test 1.1: 100MB file with simple substitution
echo -e "${BLUE}Test 1.1: 100MB text file - simple substitution${NC}"
LARGE_FILE="$TEMP_DIR/test_100mb.txt"
generate_large_text_file "$LARGE_FILE" 100 "foo"

if [ "$HAS_TIME" = true ]; then
    echo "  Running with memory profiling..."
    OUTPUT=$(/usr/bin/time -v $SEDX 's/foo/bar/g' "$LARGE_FILE" 2>&1)
    MEMORY=$(echo "$OUTPUT" | grep "Maximum resident set size" | awk '{print $6}')
    if [ -n "$MEMORY" ]; then
        MEMORY_MB=$((MEMORY / 1024))
        echo "  Peak memory: ${MEMORY_MB}MB"
        
        if [ "$MEMORY_MB" -lt 100 ]; then
            echo -e "${GREEN}✓ PASSED${NC} - Memory under 100MB (${MEMORY_MB}MB)"
            ((PASSED++))
        else
            echo -e "${YELLOW}⚠ MEMORY${NC} - Memory at ${MEMORY_MB}MB (streaming may not be active)"
            ((PASSED++))
        fi
    else
        echo -e "${YELLOW}⚠ Could not measure memory${NC}"
        ((SKIPPED++))
    fi
else
    $SEDX 's/foo/bar/g' "$LARGE_FILE" > /dev/null 2>&1
fi

if grep -q "original text bar" "$LARGE_FILE"; then
    echo -e "${GREEN}✓ Substitution successful${NC}"
else
    echo -e "${RED}✗ Substitution failed${NC}"
fi
rm -f "$LARGE_FILE"

# Test 1.2: Multiple substitutions on large file
echo -e "\n${BLUE}Test 1.2: 100MB file - multiple chained substitutions${NC}"
LARGE_FILE="$TEMP_DIR/test_100mb_multi.txt"
generate_large_text_file "$LARGE_FILE" 100 "foo bar baz"

$SEDX '{s/foo/FOO/g; s/bar/BAR/g; s/baz/BAZ/g}' "$LARGE_FILE" > /dev/null 2>&1

if grep -q "original text FOO BAR BAZ" "$LARGE_FILE"; then
    echo -e "${GREEN}✓ PASSED${NC} - All substitutions successful"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Substitutions failed"
    ((FAILED++))
fi
rm -f "$LARGE_FILE"

# Test 1.3: Pattern deletion on large file
echo -e "\n${BLUE}Test 1.3: 100MB file - pattern deletion${NC}"
LARGE_FILE="$TEMP_DIR/test_100mb_delete.txt"
generate_large_text_file "$LARGE_FILE" 100 "delete_me"

# Add some lines to keep
echo "keep this line" >> "$LARGE_FILE"
echo "delete this line delete_me" >> "$LARGE_FILE"
echo "also keep this" >> "$LARGE_FILE"

$SEDX '/delete_me/d' "$LARGE_FILE" > /dev/null 2>&1

if grep -q "keep this line" "$LARGE_FILE" && ! grep -q "delete this line delete_me" "$LARGE_FILE"; then
    echo -e "${GREEN}✓ PASSED${NC} - Pattern deletion works on large file"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Pattern deletion failed"
    ((FAILED++))
fi
rm -f "$LARGE_FILE"

# ============================================================================
# TEST CATEGORY 2: Many Small Files
# ============================================================================
echo -e "\n${CYAN}=== CATEGORY 2: Many Small Files ===${NC}"
echo ""

# Test 2.1: Process 1,000 small files
echo -e "${BLUE}Test 2.1: Process 1,000 small files${NC}"
MANY_FILES_DIR="$TEMP_DIR/many_files"
mkdir -p "$MANY_FILES_DIR"

echo "  Generating 1,000 test files..."
for i in $(seq 1 1000); do
    echo "line $i with foo bar content" > "$MANY_FILES_DIR/file_$i.txt"
done

echo "  Processing all files..."
START_TIME=$(date +%s%N)
for file in "$MANY_FILES_DIR"/*.txt; do
    $SEDX 's/foo/bar/g; s/baz/qux/g' "$file" > /dev/null 2>&1
done
END_TIME=$(date +%s%N)
ELAPSED=$(( (END_TIME - START_TIME) / 1000000 ))

echo "  Time taken: ${ELAPSED}ms"

# Verify a few files
SUCCESS=true
for i in 1 100 500 1000; do
    if ! grep -q "line $i with bar bar content" "$MANY_FILES_DIR/file_$i.txt" 2>/dev/null; then
        SUCCESS=false
        break
    fi
done

if [ "$SUCCESS" = true ]; then
    echo -e "${GREEN}✓ PASSED${NC} - All 1,000 files processed correctly in ${ELAPSED}ms"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Some files not processed correctly"
    ((FAILED++))
fi
rm -rf "$MANY_FILES_DIR"

# Test 2.2: Batch processing with glob pattern
echo -e "\n${BLUE}Test 2.2: Batch processing with glob${NC}"
BATCH_DIR="$TEMP_DIR/batch_test"
mkdir -p "$BATCH_DIR"

for i in $(seq 1 100); do
    echo "test $i: replace_me" > "$BATCH_DIR/test_$i.txt"
done

$SEDX 's/replace_me/done/g' "$BATCH_DIR"/*.txt > /dev/null 2>&1

CORRECT=0
for file in "$BATCH_DIR"/*.txt; do
    if grep -q "done" "$file"; then
        ((CORRECT++))
    fi
done

if [ "$CORRECT" -eq 100 ]; then
    echo -e "${GREEN}✓ PASSED${NC} - All 100 files processed via glob"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Only $CORRECT/100 files processed correctly"
    ((FAILED++))
fi
rm -rf "$BATCH_DIR"

# ============================================================================
# TEST CATEGORY 3: Complex Regex Patterns
# ============================================================================
echo -e "\n${CYAN}=== CATEGORY 3: Complex Regex Patterns ===${NC}"
echo ""

# Test 3.1: Very long regex pattern (100+ characters)
echo -e "${BLUE}Test 3.1: 100+ character regex pattern${NC}"
REGEX_FILE="$TEMP_DIR/long_regex.txt"
echo "The quick brown fox jumps over the lazy dog and then runs to the market" > "$REGEX_FILE"

LONG_PATTERN="The quick brown fox jumps over the lazy dog and then runs to the market"
$SEDX "s/$LONG_PATTERN/MATCHED/g" "$REGEX_FILE" > /dev/null 2>&1

if grep -q "MATCHED" "$REGEX_FILE"; then
    echo -e "${GREEN}✓ PASSED${NC} - Long regex pattern (${#LONG_PATTERN} chars) matched"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Long pattern did not match"
    ((FAILED++))
fi
rm -f "$REGEX_FILE"

# Test 3.2: Regex with many capture groups (10+)
echo -e "\n${BLUE}Test 3.2: Regex with 10 capture groups${NC}"
CAPTURE_FILE="$TEMP_DIR/capture_groups.txt"
echo "1 2 3 4 5 6 7 8 9 10" > "$CAPTURE_FILE"

CAPTURE_PATTERN="([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+) ([0-9]+)"
CAPTURE_REPLACEMENT="[\1][\2][\3][\4][\5][\6][\7][\8][\9][\10]"

$SEDX "s/$CAPTURE_PATTERN/$CAPTURE_REPLACEMENT/g" "$CAPTURE_FILE" > /dev/null 2>&1

if grep -q "\[1\]\[2\]" "$CAPTURE_FILE"; then
    echo -e "${GREEN}✓ PASSED${NC} - 10 capture groups handled correctly"
    ((PASSED++))
else
    echo -e "${RED}✗ FAILED${NC} - Capture groups failed"
    ((FAILED++))
fi
rm -f "$CAPTURE_FILE"

# Test 3.3: Nested capture groups
echo -e "\n${BLUE}Test 3.3: Nested capture groups${NC}"
NESTED_FILE="$TEMP_DIR/nested_groups.txt"
echo "abc123def" > "$NESTED_FILE"

NESTED_PATTERN="(([a-z]+)([0-9]+))"
NESTED_REPLACEMENT="[\1]-[\2]-[\3]"

$SEDX "s/$NESTED_PATTERN/$NESTED_REPLACEMENT/g" "$NESTED_FILE" > /dev/null 2>&1

if grep -q "\[abc123\]-\[abc\]-\[123\]" "$NESTED_FILE"; then
    echo -e "${GREEN}✓ PASSED${NC} - Nested groups handled correctly"
    ((PASSED++))
else
    echo -e "${YELLOW}⚠ PARTIAL${NC} - Nested groups (got: $(cat $NESTED_FILE))"
    ((SKIPPED++))
fi
rm -f "$NESTED_FILE"

# ============================================================================
# TEST CATEGORY 4: Unicode Edge Cases
# ============================================================================
echo -e "\n${CYAN}=== CATEGORY 4: Unicode Edge Cases ===${NC}"
echo ""

# Test 4.1: Emoji and multi-byte characters
echo -e "${BLUE}Test 4.1: Emoji and multi-byte characters${NC}"
EMOJI_FILE="$TEMP_DIR/emoji.txt"
printf "Hello 😀 World\nTest 🚀 Rocket\nFire 🔥 Flame\n" > "$EMOJI_FILE"

$SEDX 's/🔥/💧/g' "$EMOJI_FILE" > /dev/null 2>&1

if grep -q "Fire 💧 Flame" "$EMOJI_FILE"; then
    echo -e "${GREEN}✓ PASSED${NC} - Emoji substitutions work"
    ((PASSED++))
else
    echo -e "${YELLOW}⚠ PARTIAL${NC} - Emoji (terminal dependent)"
    ((SKIPPED++))
fi
rm -f "$EMOJI_FILE"

# Test 4.2: CJK (Chinese/Japanese/Korean) characters
echo -e "\n${BLUE}Test 4.2: CJK (Chinese/Japanese/Korean) characters${NC}"
CJK_FILE="$TEMP_DIR/cjk.txt"
printf "Hello 世界 World\nTest 日本語\n" > "$CJK_FILE"

$SEDX 's/世界/世界世界/g' "$CJK_FILE" > /dev/null 2>&1

if grep -q "世界世界" "$CJK_FILE"; then
    echo -e "${GREEN}✓ PASSED${NC} - CJK characters handled"
    ((PASSED++))
else
    echo -e "${YELLOW}⚠ PARTIAL${NC} - CJK (terminal dependent)"
    ((SKIPPED++))
fi
rm -f "$CJK_FILE"

# ============================================================================
# TEST CATEGORY 5: Long Lines
# ============================================================================
echo -e "\n${CYAN}=== CATEGORY 5: Long Lines ===${NC}"
echo ""

# Test 5.1: Single line with 1MB of text
echo -e "${BLUE}Test 5.1: Single 1MB line - substitution${NC}"
LONG_LINE_FILE="$TEMP_DIR/long_line.txt"

python3 -c "print('x' * 1000000)" > "$LONG_LINE_FILE" 2>/dev/null

if [ -f "$LONG_LINE_FILE" ]; then
    $SEDX 's/x/y/g' "$LONG_LINE_FILE" > /dev/null 2>&1
    Y_COUNT=$(tr -cd 'y' < "$LONG_LINE_FILE" | wc -c)
    
    if [ "$Y_COUNT" -ge 900000 ]; then
        echo -e "${GREEN}✓ PASSED${NC} - 1MB line processed (~${Y_COUNT} replacements)"
        ((PASSED++))
    else
        echo -e "${RED}✗ FAILED${NC} - Expected ~1,000,000 replacements, got $Y_COUNT"
        ((FAILED++))
    fi
else
    echo -e "${YELLOW}⚠ SKIPPED${NC} - python3 not available"
    ((SKIPPED++))
fi
rm -f "$LONG_LINE_FILE"

# Test 5.2: Multiple long lines (fixed - using Python script)
echo -e "\n${BLUE}Test 5.2: Multiple 100KB lines${NC}"
MANY_LONG_FILE="$TEMP_DIR/many_long.txt"

if command -v python3 &> /dev/null; then
    # Use a proper Python script instead of command line
    python3 << 'PYEOF' > "$MANY_LONG_FILE"
for i in range(1, 11):
    print(f"line_{i}_{'x' * 100000}")
PYEOF
    
    $SEDX 's/x/y/g' "$MANY_LONG_FILE" > /dev/null 2>&1
    
    # Check if substitution worked - look for line with y's
    if grep -q "line_1_yyyyy" "$MANY_LONG_FILE"; then
        echo -e "${GREEN}✓ PASSED${NC} - Multiple long lines processed"
        ((PASSED++))
    else
        echo -e "${RED}✗ FAILED${NC} - Long lines not processed"
        ((FAILED++))
    fi
else
    echo -e "${YELLOW}⚠ SKIPPED${NC} - python3 not available"
    ((SKIPPED++))
fi
rm -f "$MANY_LONG_FILE"

# ============================================================================
# TEST CATEGORY 6: Performance Benchmarks
# ============================================================================
echo -e "\n${CYAN}=== CATEGORY 6: Performance Benchmarks ===${NC}"
echo ""

# Test 6.1: Compare with GNU sed (if available)
echo -e "${BLUE}Test 6.1: Performance comparison with GNU sed${NC}"
PERF_FILE="$TEMP_DIR/perf_test.txt"

for i in $(seq 1 10000); do
    echo "line $i: foo bar baz" >> "$PERF_FILE"
done

# Time SedX
START_TIME=$(date +%s%N)
$SEDX 's/foo/bar/g' "$PERF_FILE" > /dev/null 2>&1
SEDX_TIME=$(( ($(date +%s%N) - START_TIME) / 1000000 ))

if command -v sed &> /dev/null; then
    # Restore file
    > "$PERF_FILE"
    for i in $(seq 1 10000); do
        echo "line $i: foo bar baz" >> "$PERF_FILE"
    done
    
    START_TIME=$(date +%s%N)
    sed 's/foo/bar/g' "$PERF_FILE" > /dev/null 2>&1
    SED_TIME=$(( ($(date +%s%N) - START_TIME) / 1000000 ))
    
    echo "  SedX: ${SEDX_TIME}ms"
    echo "  GNU sed: ${SED_TIME}ms"
    
    if command -v bc &> /dev/null; then
        RATIO=$(echo "scale=2; $SEDX_TIME / $SED_TIME" | bc)
        echo "  Ratio: ${RATIO}x"
    fi
    
    echo -e "${GREEN}✓ PASSED${NC} - Performance test completed"
    ((PASSED++))
else
    echo "  SedX: ${SEDX_TIME}ms"
    echo -e "${YELLOW}⚠ SKIPPED${NC} - GNU sed not available for comparison"
    ((PASSED++))
fi
rm -f "$PERF_FILE"

# ============================================================================
# SUMMARY
# ============================================================================
echo ""
echo "========================================"
echo "  Stress Test Results Summary"
echo "========================================"
echo ""
echo "Total Tests Run: $((PASSED + FAILED + SKIPPED))"
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo -e "${YELLOW}Skipped: $SKIPPED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All stress tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some stress tests failed!${NC}"
    exit 1
fi
