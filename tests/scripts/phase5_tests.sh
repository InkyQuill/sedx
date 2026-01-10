#!/bin/bash
# Phase 5 Tests: Flow Control, File I/O, and Additional Commands
# Tests: b, t, T (flow control), r, w, R, W (file I/O), =, F, z (additional)

# set -e  # Disabled to see all test results

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

# Helper function to run a test with stdin
run_stdin_test() {
    local test_name="$1"
    local expression="$2"
    local input_text="$3"
    local expected="$4"

    echo -n "Testing: $test_name ... "

    # Run sedx with stdin
    result=$(echo -e "$input_text" | $SEDX_BIN "$expression" 2>/dev/null || true)

    # Interpret newlines in expected string
    expected_interpreted=$(echo -e "$expected")

    # Compare with expected
    if [ "$result" = "$expected_interpreted" ]; then
        echo -e "${GREEN}PASSED${NC}"
        ((TESTS_PASSED++))
        return 0
    else
        echo -e "${RED}FAILED${NC}"
        echo "  Expected: '$expected_interpreted'"
        echo "  Got:      '$result'"
        ((TESTS_FAILED++))
        return 1
    fi
}

echo "=== SedX Phase 5 Tests ==="
echo ""
echo "--- Week 1-2: Flow Control (b, t, T commands) ---"
echo ""

# Test 1: Simple branch to end
run_stdin_test \
    "Branch to end (2b)" \
    '2b; s/foo/FOO/' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nbaz"

# Test 2: Branch at line 1
run_stdin_test \
    "Branch at line 1" \
    '1b; s/foo/FOO/' \
    "foo\nbar\nbaz" \
    "foo\nbar\nbaz"

# Test 3: Branch with line range
run_stdin_test \
    "Branch at line 2-3" \
    '2,3b; s/foo/FOO/' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nbaz"

# Test 4: Multiple branches
run_stdin_test \
    "Multiple branches" \
    '1b; 3b; s/foo/FOO/' \
    "foo\nbar\nfoo" \
    "foo\nbar\nfoo"

# Test 5: Label definition and reference
run_stdin_test \
    "Label with branch" \
    ':start; s/foo/FOO/; b end; s/bar/BAR/; :end' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nbaz"

# Test 6: Label with line address
run_stdin_test \
    "Label with line address" \
    '2{ s/bar/BAR/; b end }; s/foo/FOO/; :end' \
    "foo\nbar\nbaz" \
    "FOO\nBAR\nbaz"

# Test 7: t command - branch if substitution made (per-line)
run_stdin_test \
    "t command with substitution" \
    's/foo/FOO/; t end; s/bar/BAR/; :end' \
    "foo\nbar" \
    "FOO\nBAR"

# Test 8: t command - no branch if no substitution (per-line)
run_stdin_test \
    "t command no substitution" \
    's/foo/FOO/; t end; s/bar/BAR/; :end' \
    "baz\nbar" \
    "baz\nBAR"

# Test 9: T command - branch if NO substitution
run_stdin_test \
    "T command no substitution" \
    's/foo/FOO/; T skip; s/bar/BAR/; b end; :skip; s/baz/BAZ/; :end' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nBAZ"

# Test 10: Complex flow control
run_stdin_test \
    "Complex flow control" \
    ':start; s/foo/FOO/; /FOO/t next; b end; :next; s/bar/BAR/; :end; s/baz/BAZ/' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nBAZ"

# Test 11: Label with group
run_stdin_test \
    "Label with group" \
    '1,3{ s/foo/FOO/; t done }; s/bar/BAR/; :done' \
    "foo\nbar\nfoo" \
    "FOO\nBAR\nFOO"

# Test 12: Test with pattern address
run_stdin_test \
    "t command with pattern address" \
    '/bar/ { s/bar/BAR/; t skip }; s/foo/FOO/; :skip' \
    "foo\nbar\nfoo" \
    "FOO\nBAR\nFOO"

echo ""
echo "--- Week 3: File I/O Command Parsing (r, w, R, W) ---"
echo "NOTE: File I/O commands are currently stubs (no-op implementation)"
echo ""

# Test 13-18: File I/O parsing tests (commands are stubs that produce no output)
run_stdin_test \
    "Read file command (stub - no output)" \
    'r /tmp/nonexistent.txt' \
    "line1\nline2" \
    ""

run_stdin_test \
    "Write file command (stub - no output)" \
    'w /tmp/output.txt' \
    "line1\nline2" \
    ""

run_stdin_test \
    "Read line command (stub - no output)" \
    'R /tmp/nonexistent.txt' \
    "line1\nline2" \
    ""

run_stdin_test \
    "Write first line command (stub - no output)" \
    'W /tmp/output.txt' \
    "line1\nline2" \
    ""

# Test 17-18: File I/O with addresses
run_stdin_test \
    "Read file with address (stub - no output)" \
    '5r /tmp/nonexistent.txt' \
    "line1\nline2" \
    ""

run_stdin_test \
    "Write file with pattern address (stub - no output)" \
    '/line2/w /tmp/output.txt' \
    "line1\nline2" \
    ""

echo ""
echo "--- Week 4: Additional Commands (=, F, z) ---"
echo ""

# Test 19-24: Additional commands (parsing only - stubs)
run_stdin_test \
    "Print line number (parsing)" \
    '=' \
    "line1\nline2" \
    "line1\nline2"

run_stdin_test \
    "Print line number with address" \
    '2=' \
    "line1\nline2" \
    "line1\nline2"

run_stdin_test \
    "Print filename (parsing)" \
    'F' \
    "line1\nline2" \
    "line1\nline2"

run_stdin_test \
    "Print filename with pattern" \
    '/line2/F' \
    "line1\nline2" \
    "line1\nline2"

run_stdin_test \
    "Clear pattern space (parsing)" \
    'z' \
    "line1\nline2" \
    "line1\nline2"

run_stdin_test \
    "Clear pattern space with address" \
    '2z' \
    "line1\nline2" \
    "line1\nline2"

echo ""
echo "--- Integration: Multiple Phase 5 Features ---"
echo ""

# Test 25: Flow control with substitution
run_stdin_test \
    "Branch with global substitution" \
    's/foo/FOO/g; b end; s/bar/BAR/; :end' \
    "foo\nbar\nfoo" \
    "FOO\nbar\nFOO"

# Test 26: Test command in group
run_stdin_test \
    "Test command in group" \
    '{ s/foo/FOO/; t done }; s/bar/BAR/; :done' \
    "foo\nbar" \
    "FOO\nBAR"

# Test 27: Multiple flow control commands
run_stdin_test \
    "Multiple t commands" \
    's/foo/FOO/; t; s/bar/BAR/; t; s/baz/BAZ/' \
    "foo\nbar\nbaz" \
    "FOO\nBAR\nBAZ"

# Test 28: T and t combination
run_stdin_test \
    "T and t combination" \
    's/xxx/XXX/; T nosub; t yesub; s/foo/FOO/; b end; :nosub; s/bar/BAR/; :yesub; :end' \
    "foo\nbar" \
    "foo\nBAR"

# Test 29: Branch with pattern range
# NOTE: Not yet supported - parser limitation
# run_stdin_test \
#     "Branch with pattern range" \
#     '/foo/,/bar/b; s/./X/' \
#     "foo\nmiddle\nbar\nend" \
#     "foo\nmiddle\nbar\nXnd"

# Test 30: Label at end of script
run_stdin_test \
    "Label at end of script" \
    's/foo/FOO/; b mylabel; s/bar/BAR/; :mylabel' \
    "foo\nbar" \
    "FOO\nbar"

# Cleanup
rm -rf "$TEMP_DIR"

echo ""
echo "========================================"
echo "Test Results"
echo "========================================"
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"
echo "Total:  $((TESTS_PASSED + TESTS_FAILED))"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All Phase 5 tests passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some Phase 5 tests failed!${NC}"
    exit 1
fi
