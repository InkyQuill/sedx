#!/bin/bash
# Regression Tests for sedx
# Tests critical sed features to ensure they match GNU sed behavior

set +H  # Disable history expansion

SEDX="./target/release/sedx"
TEMP_DIR="/tmp/sedx_tests"
mkdir -p "$TEMP_DIR"

PASSED=0
FAILED=0

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

test_substitution() {
    local test_name="$1"
    local expr="$2"
    local input="$3"

    echo -n "Testing: $test_name ... "

    # Test with sedx
    echo "$input" > "$TEMP_DIR/test_sedx.txt"
    $SEDX "$expr" "$TEMP_DIR/test_sedx.txt" > /dev/null 2>&1
    local sedx_result=$(cat "$TEMP_DIR/test_sedx.txt")

    # Test with sed
    echo "$input" > "$TEMP_DIR/test_sed.txt"
    local sed_result=$(echo "$input" | sed "$expr")

    # Compare
    if [ "$sedx_result" = "$sed_result" ]; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        echo "  Expression: $expr"
        echo "  sedx result: $sedx_result"
        echo "  sed result: $sed_result"
        ((FAILED++))
    fi
}

test_delete() {
    local test_name="$1"
    local expr="$2"
    local input="$3"

    echo -n "Testing: $test_name ... "

    # Test with sedx
    echo "$input" > "$TEMP_DIR/test_sedx.txt"
    $SEDX "$expr" "$TEMP_DIR/test_sedx.txt" > /dev/null 2>&1
    local sedx_result=$(cat "$TEMP_DIR/test_sedx.txt")

    # Test with sed
    local sed_result=$(echo "$input" | sed "$expr")

    # Compare
    if [ "$sedx_result" = "$sed_result" ]; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        echo "  Expression: $expr"
        echo "  sedx result: $(echo "$sedx_result" | head -3)"
        echo "  sed result: $(echo "$sed_result" | head -3)"
        ((FAILED++))
    fi
}

echo "========================================"
echo "  SedX Regression Tests"
echo "========================================"
echo ""

# Test data
TEST1="line 1
line 2 foo
line 3
line 4 bar
line 5"

TEST2="line 1
start here
line 3
line 4
end here
line 6
start again
line 8
end again
line 10"

TEST3="error line1
normal line
error line2
another normal"

echo "--- Substitution Tests ---"
test_substitution "Global substitution" "s/line/LINE/g" "$TEST1"
test_substitution "Line-specific substitution" "2s/foo/FOO/" "$TEST1"
test_substitution "Range substitution" "1,3s/line/LINE/g" "$TEST1"
test_substitution "Case-insensitive substitution" "s/foo/FOO/i" "$TEST1"

echo ""
echo "--- Delete Tests ---"
test_delete "Delete single line" "3d" "$TEST1"
test_delete "Delete line range" "2,4d" "$TEST1"
test_delete "Delete pattern" "/foo/d" "$TEST1"
test_delete "Delete pattern range" "/start/,/end/d" "$TEST2"

echo ""
echo "--- Negation Tests ---"
test_delete "Delete lines NOT matching error" "/error/!d" "$TEST3"

echo ""
echo "--- Group Tests ---"
TEST4="line 1
line 2
line 3
line 4"

test_substitution "Group without range" "{s/line/LINE/g}" "$TEST4"

echo ""
echo "========================================"
echo "  Results: $PASSED passed, $FAILED failed"
echo "========================================"

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
