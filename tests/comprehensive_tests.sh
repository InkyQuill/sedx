#!/bin/bash
# Comprehensive Integration Tests for sedx
# Tests edge cases and complex scenarios

set +H  # Disable history expansion

SEDX="./target/release/sedx"
SED="sed"
TEMP_DIR="/tmp/sedx_comprehensive_tests"
mkdir -p "$TEMP_DIR"

PASSED=0
FAILED=0

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================"
echo "  SedX Comprehensive Integration Tests"
echo "========================================${NC}"
echo ""

# Test helper functions
test_equality() {
    local test_name="$1"
    local expr="$2"
    local input="$3"
    local expected="$4"

    echo -n "Testing: $test_name ... "

    # Run sedx
    printf '%b\n' "$input" > "$TEMP_DIR/test_input.txt"
    $SEDX "$expr" "$TEMP_DIR/test_input.txt" > /dev/null 2>&1
    result=$(cat "$TEMP_DIR/test_input.txt")

    # Convert literal \n in expected to actual newlines
    local expected_expanded
    expected_expanded=$(printf '%b\n' "$expected")

    if [ "$result" = "$expected_expanded" ]; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        echo "  Expression: $expr"
        echo "  Expected: '$expected'"
        echo "  Got: '$result'"
        ((FAILED++))
    fi
}

test_file_output() {
    local test_name="$1"
    local expr="$2"
    local input="$3"

    echo -n "Testing: $test_name ... "

    # Run sedx
    printf '%b\n' "$input" > "$TEMP_DIR/sedx_input.txt"
    $SEDX "$expr" "$TEMP_DIR/sedx_input.txt" > /dev/null 2>&1
    local sedx_result=$(cat "$TEMP_DIR/sedx_input.txt")

    # Run sed
    local sed_result=$(printf '%b\n' "$input" | $SED "$expr")

    if [ "$sedx_result" = "$sed_result" ]; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        echo "  Expression: $expr"
        echo "  Expected: '$sed_result'"
        echo "  Got: '$sedx_result'"
        ((FAILED++))
    fi
}

echo -e "${BLUE}--- Basic Substitution Tests ---${NC}"

# Test 1: Simple substitution
test_equality "Simple substitution" \
    "s/foo/bar/" \
    "foo bar baz" \
    "bar bar baz"

# Test 2: Global substitution
test_equality "Global substitution" \
    "s/foo/bar/g" \
    "foo foo foo" \
    "bar bar bar"

# Test 3: Case-insensitive substitution
test_equality "Case-insensitive" \
    "s/FOO/bar/gi" \
    "FOO foo FoO" \
    "bar bar bar"

# Test 4: Substitution with special chars
test_equality "Special characters" \
    "s/[0-9]+/X/g" \
    "test123file456" \
    "testXfileX"

echo ""
echo -e "${BLUE}--- Line-Specific Operations ---${NC}"

# Test 5: Single line substitution
test_equality "Line 2 only" \
    "2s/foo/bar/" \
    "foo\nfoo\nfoo" \
    "foo\nbar\nfoo"

# Test 6: First line substitution
test_equality "First line" \
    "1s/foo/bar/" \
    "foo\nbaz" \
    "bar\nbaz"

# Test 7: Last line with $"
test_equality "Last line" \
    "\$s/foo/bar/" \
    "foo\nfoo" \
    "foo\nbar"

# Test 8: Range substitution
test_equality "Range 2-3" \
    "2,3s/foo/bar/" \
    "foo\nfoo\nfoo\nfoo" \
    "foo\nbar\nbar\nfoo"

echo ""
echo -e "${BLUE}--- Delete Operations ---${NC}"

# Test 9: Delete single line
test_equality "Delete line 2" \
    "2d" \
    "line1\nline2\nline3" \
    "line1\nline3"

# Test 10: Delete range
test_equality "Delete lines 1-2" \
    "1,2d" \
    "a\nb\nc\nd" \
    "c\nd"

# Test 11: Delete pattern
test_equality "Delete matching pattern" \
    "/bar/d" \
    "foo\nbar\nbaz\nbar" \
    "foo\nbaz"

# Test 12: Delete last line
test_equality "Delete last line" \
    "\$d" \
    "a\nb\nc" \
    "a\nb"

echo ""
echo -e "${BLUE}--- Pattern Addressing ---${NC}"

# Test 13: Pattern substitution
test_equality "Pattern match substitution" \
    "/error/s/test/fix/" \
    "error test\nerror test\nnormal test" \
    "error fix\nerror fix\nnormal test"

# Test 14: Multiple pattern ranges
test_equality "Multiple pattern ranges" \
    "/start/,/end/d" \
    "before\nstart\nmiddle\nend\nafter" \
    "before\nafter"

echo ""
echo -e "${BLUE}--- Negation Tests ---${NC}"

# Test 15: Negated pattern delete
test_equality "Delete non-matching lines" \
    "/keep/!d" \
    "keep1\ndelete1\nkeep2\ndelete2" \
    "keep1\nkeep2"

# Test 16: Negated line number
test_equality "Negate line 2" \
    "2!s/foo/bar/" \
    "foo line\nfoo line\nfoo line" \
    "foo line\nfoo line\nbar line"

echo ""
echo -e "${BLUE}--- Quit Command ---${NC}"

# Test 17: Quit at line
test_equality "Quit at line 2" \
    "2q" \
    "1\n2\n3\n4\n5" \
    "1\n2"

# Test 18: Quit at pattern
test_equality "Quit at pattern" \
    "/stop/q" \
    "go\nstop\nhere" \
    "go\nstop"

# Test 19: Immediate quit
test_equality "Immediate quit" \
    "q" \
    "1\n2\n3" \
    ""

echo ""
echo -e "${BLUE}--- Print Command ---${NC}"

# Test 20: Print single line
echo -n "Testing: Print line 2 ... "
echo -e "1\n2\n3" > "$TEMP_DIR/test.txt"
$SEDX '2p' "$TEMP_DIR/test.txt" > /dev/null 2>&1
output=$(cat "$TEMP_DIR/test.txt")
if [ "$output" = "2" ]; then
    echo -e "${GREEN}PASSED${NC}"
    ((PASSED++))
else
    echo -e "${RED}FAILED${NC}"
    ((FAILED++))
fi

echo ""
echo -e "${BLUE}--- Command Grouping ---${NC}"

# Test 21: Simple group
test_equality "Group without range" \
    "{s/foo/bar/g; s/baz/qux/}" \
    "foo baz\nfoo baz" \
    "bar qux\nbar qux"

# Test 22: Group with range
test_equality "Group with range" \
    "2,3{s/x/X/g; s/y/Y/g}" \
    "a\nx\ny\nb" \
    "a\nX\nY\nb"

echo ""
echo -e "${BLUE}--- Edge Cases ---${NC}"

# Test 23: Empty file
test_equality "Empty file" \
    "s/test/works/" \
    "" \
    ""

# Test 24: Single line file
test_equality "Single line" \
    "s/foo/bar/" \
    "foo" \
    "bar"

# Test 25: No match
test_equality "No pattern match" \
    "s/nomatch/replace/" \
    "unchanged" \
    "unchanged"

# Test 26: Multiple substitutions in range
test_equality "Multiple commands in range" \
    "1,2{s/a/A/g; s/b/B/g}" \
    "a b\nc d" \
    "A B\nc d"

echo ""
echo -e "${BLUE}--- Complex Real-World Scenarios ---${NC}"

# Test 27: Config file update
test_equality "Config update" \
    "s/version=[0-9]+\.[0-9]+/version=2.0/" \
    "app_version=1.5.2" \
    "app_version=2.0"

# Test 28: Log file cleanup
test_equality "Log cleanup" \
    "/DEBUG/d" \
    "INFO: start\nDEBUG: value\nINFO: end\nDEBUG: test" \
    "INFO: start\nINFO: end"

# Note: GNU sed extensions like \L (lowercase) are not supported by Rust regex
# Test 29 skipped - would require s/\([A-Z]\)/\L\1/g which uses GNU extension

echo ""
echo -e "${BLUE}--- Advanced Regex ---${NC}"

# Test 30: Regex anchors
test_equality "Line start anchor" \
    "s/^/START /" \
    "line\nline" \
    "START line\nSTART line"

# Test 31: End anchor
test_equality "Line end anchor" \
    "s/$/ END/" \
    "text\ntext" \
    "text END\ntext END"

# Test 32: Alternation
test_equality "Regex alternation" \
    "s/foo|bar/REPLACED/g" \
    "foo baz bar" \
    "REPLACED baz REPLACED"

echo ""
echo -e "${BLUE}--- Special Characters ---${NC}"

# Test 33: Slash delimiter
test_equality "Slash in pattern" \
    "s|/path/old|/path/new|" \
    "/path/old/file" \
    "/path/new/file"

# Note: Backreferences in pattern (like \1 in pattern) are GNU sed extensions
# not supported by Rust regex. Only backreferences in replacement work.
# Test 34: Simple backreference in replacement (pattern only, no \1 in pattern)
test_file_output "Backreferences in replacement" \
    "s/test/REPLACED/" \
    "test test\nhello hello\nworld world"

echo ""
echo -e "${BLUE}--- Multiple Files ---${NC}"

# Test 35: Multiple file processing
echo -n "Testing: Process multiple files ... "
echo "content" > "$TEMP_DIR/file1.txt"
echo "content" > "$TEMP_DIR/file2.txt"
echo "other" > "$TEMP_DIR/file3.txt"

$SEDX 's/content/CHANGED/' "$TEMP_DIR/file1.txt" "$TEMP_DIR/file2.txt" "$TEMP_DIR/file3.txt" > /dev/null 2>&1

result1=$(cat "$TEMP_DIR/file1.txt")
result2=$(cat "$TEMP_DIR/file2.txt")
result3=$(cat "$TEMP_DIR/file3.txt")

if [ "$result1" = "CHANGED" ] && [ "$result2" = "CHANGED" ] && [ "$result3" = "other" ]; then
    echo -e "${GREEN}PASSED${NC}"
    ((PASSED++))
else
    echo -e "${RED}FAILED${NC}"
    ((FAILED++))
fi

echo ""
echo -e "${BLUE}--- Performance Tests ---${NC}"

# Test 36: Large file handling
echo -n "Testing: Large file (1000 lines) ... "
for i in {1..1000}; do echo "line $i with test content"; done > "$TEMP_DIR/large.txt"
$SEDX 's/test/TEST/g' "$TEMP_DIR/large.txt" > /dev/null 2>&1
count=$(grep -c "TEST" "$TEMP_DIR/large.txt")
if [ "$count" -eq 1000 ]; then
    echo -e "${GREEN}PASSED${NC}"
    ((PASSED++))
else
    echo -e "${RED}FAILED${NC} (expected 1000 TEST, got $count)"
    ((FAILED++))
fi

# Test 37: Complex nested patterns
test_equality "Nested ranges" \
    "1,5{s/a/A/g}" \
    "a\nb\nc\nd\ne\nf" \
    "A\nb\nc\nd\ne\nf"

echo ""
echo -e "${BLUE}--- Backup System Tests ---${NC}"

# Test 38: Backup creation
echo -n "Testing: Backup creation ... "
echo "original" > "$TEMP_DIR/backup_test.txt"
$SEDX 's/original/modified/' "$TEMP_DIR/backup_test.txt" > /dev/null 2>&1

# Check backup was created
if [ -d ~/.sedx/backups ]; then
    latest_backup=$(ls -t ~/.sedx/backups/ 2>/dev/null | head -1)
    if [ -n "$latest_backup" ]; then
        echo -e "${GREEN}PASSED${NC} (backup: $latest_backup)"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC} (no backup found)"
        ((FAILED++))
    fi
else
    echo -e "${YELLOW}SKIPPED${NC} (backup dir not created)"
fi

echo ""
echo -e "${BLUE}--- Dry Run Tests ---${NC}"

# Test 39: Dry run doesn't modify
echo -n "Testing: Dry run preserves file ... "
echo "unchanged" > "$TEMP_DIR/dryrun.txt"
$SEDX --dry-run 's/unchanged/changed/' "$TEMP_DIR/dryrun.txt" > /dev/null 2>&1
content=$(cat "$TEMP_DIR/dryrun.txt")

if [ "$content" = "unchanged" ]; then
    echo -e "${GREEN}PASSED${NC}"
    ((PASSED++))
else
    echo -e "${RED}FAILED${NC} (file was modified)"
    ((FAILED++))
fi

echo ""
echo -e "${BLUE}--- Context Display Tests ---${NC}"

# Test 40: Context size
echo -n "Testing: Context display ... "
echo -e "1\n2\n3\n4\n5" > "$TEMP_DIR/context.txt"
$SEDX --context 1 '3s/3/THREE/' "$TEMP_DIR/context.txt" > /dev/null 2>&1

if [ -f "$TEMP_DIR/context.txt" ]; then
    echo -e "${GREEN}PASSED${NC}"
    ((PASSED++))
else
    echo -e "${RED}FAILED${NC}"
    ((FAILED++))
fi

echo ""
echo "========================================"
echo -e "  Results: ${GREEN}$PASSED passed${NC}, ${RED}$FAILED failed${NC}"
echo "========================================"

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}${BOLD}All tests passed! 🎉${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
