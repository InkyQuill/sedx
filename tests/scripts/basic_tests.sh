#!/bin/bash
# Basic command tests for SedX
# Tests: s, d, p, q, i, a, c commands

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$(dirname "$SCRIPT_DIR")/fixtures"
SEDX_BIN="${SEDX_BIN:-./target/release/sedx}"
TEMP_DIR=$(mktemp -d)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counter
TESTS_PASSED=0
TESTS_FAILED=0

# Helper function to run a test
run_test() {
    local test_name="$1"
    local expression="$2"
    local input_file="$3"
    local expected_file="$4"
    local use_stdin="${5:-false}"

    echo -n "Testing: $test_name ... "

    # Copy input file to temp directory
    cp "$input_file" "$TEMP_DIR/test_input.txt"

    if [ "$use_stdin" = "true" ]; then
        # Test with stdin/stdout
        cat "$TEMP_DIR/test_input.txt" | $SEDX_BIN "$expression" > "$TEMP_DIR/output.txt" 2>&1 || true
    else
        # Test with file (without --dry-run to get actual result)
        $SEDX_BIN "$expression" "$TEMP_DIR/test_input.txt" > "$TEMP_DIR/output.txt" 2>&1 || true
        # Read the modified file
        cat "$TEMP_DIR/test_input.txt" > "$TEMP_DIR/output.txt"
    fi

    # Compare with expected
    if diff -q "$TEMP_DIR/output.txt" "$expected_file" > /dev/null 2>&1; then
        echo -e "${GREEN}PASSED${NC}"
        ((TESTS_PASSED++))
        return 0
    else
        echo -e "${RED}FAILED${NC}"
        echo "  Expected: $(cat "$expected_file" | head -1)"
        echo "  Got: $(cat "$TEMP_DIR/output.txt" | head -1)"
        ((TESTS_FAILED++))
        return 1
    fi
}

echo "=== SedX Basic Command Tests ==="
echo ""

# Substitution tests
run_test "Basic substitution (s/foo/BAR/)" \
    's/foo/BAR/' \
    "$FIXTURES_DIR/basic/substitute.inp" \
    "$FIXTURES_DIR/basic/substitute.good"

run_test "Global substitution (s/foo/bar/g)" \
    's/foo/bar/g' \
    "$FIXTURES_DIR/basic/substitute_global.inp" \
    "$FIXTURES_DIR/basic/substitute_global.good"

run_test "Numbered substitution (s/foo/BAR/2)" \
    's/foo/BAR/2' \
    "$FIXTURES_DIR/basic/substitute_numbered.inp" \
    "$FIXTURES_DIR/basic/substitute_numbered.good"

run_test "Case-insensitive substitution (s/foo/replaced/i)" \
    's/foo/replaced/i' \
    "$FIXTURES_DIR/basic/substitute_case.inp" \
    "$FIXTURES_DIR/basic/substitute_case.good"

# Delete tests
run_test "Delete range (3,5d)" \
    '3,5d' \
    "$FIXTURES_DIR/basic/delete.inp" \
    "$FIXTURES_DIR/basic/delete.good"

# Print tests - need special handling for -n flag
run_test "Print with -n flag" \
    '-n 2,3p' \
    "$FIXTURES_DIR/basic/print.inp" \
    "$FIXTURES_DIR/basic/print.good"

# Quit tests
run_test "Quit at line (3q)" \
    '3q' \
    "$FIXTURES_DIR/basic/quit.inp" \
    "$FIXTURES_DIR/basic/quit.good"

# Insert tests
run_test "Insert at line (2i)" \
    '2i INSERTED LINE' \
    "$FIXTURES_DIR/basic/insert.inp" \
    "$FIXTURES_DIR/basic/insert.good"

# Append tests
run_test "Append at line (2a)" \
    '2a APPENDED LINE' \
    "$FIXTURES_DIR/basic/append.inp" \
    "$FIXTURES_DIR/basic/append.good"

# Change tests
run_test "Change range (2,3c)" \
    '2,3c CHANGED CONTENT' \
    "$FIXTURES_DIR/basic/change.inp" \
    "$FIXTURES_DIR/basic/change.good"

# Cleanup
rm -rf "$TEMP_DIR"

# Summary
echo ""
echo "=== Test Summary ==="
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"
echo "Total: $((TESTS_PASSED + TESTS_FAILED))"

if [ $TESTS_FAILED -gt 0 ]; then
    exit 1
fi

exit 0
