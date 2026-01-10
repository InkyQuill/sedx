#!/bin/bash
# Flow Control Tests for SedX
# Tests b, t, T commands and labels

# set -e  # Disabled to see all test results

SEDX="/home/inky/Development/sedx/target/release/sedx"
SED="sed"
TEST_DIR="/tmp/sedx_flow_tests"
mkdir -p "$TEST_DIR"

passed=0
failed=0

test_case() {
    local name="$1"
    local expression="$2"
    local input="$3"
    local expected="$4"

    echo -n "Testing: $name ... "

    # Create test file
    echo -e "$input" > "$TEST_DIR/test.txt" || { echo "ERROR: Failed to create test file"; return 1; }

    # Run SedX using stdin mode (outputs to stdout like GNU sed)
    echo -e "$input" | $SEDX "$expression" > "$TEST_DIR/sedx_output.txt" 2>/dev/null || true
    actual=$(cat "$TEST_DIR/sedx_output.txt" 2>/dev/null || echo "")

    # Run GNU sed for comparison (capture stdout)
    echo -e "$input" | $SED "$expression" > "$TEST_DIR/sed_output.txt" || { echo "ERROR: sed failed"; return 1; }
    sed_actual=$(cat "$TEST_DIR/sed_output.txt")

    # Interpret \n in expected as actual newlines
    expected_interpreted=$(echo -e "$expected")

    if [ "$actual" = "$expected_interpreted" ] && [ "$sed_actual" = "$expected_interpreted" ]; then
        echo "✓ PASS"
        ((passed++))
    else
        echo "✗ FAIL"
        echo "  Input: $input"
        echo "  Expected: $expected_interpreted"
        echo "  SedX got: $actual"
        echo "  Sed got: $sed_actual"
        ((failed++))
    fi
}

echo "======================================"
echo "Flow Control Tests"
echo "======================================"
echo ""

# Test 1: Simple branch to end (b with no label)
test_case \
    "Branch to end" \
    '2b; s/foo/FOO/' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nbaz"

# Test 2: Branch with line address
test_case \
    "Branch at line 1" \
    '1b; s/foo/FOO/' \
    "foo\nbar\nbaz" \
    "foo\nbar\nbaz"

# Test 3: Branch with line range (using in-memory mode)
test_case \
    "Branch at line 2-3" \
    '2,3b; s/foo/FOO/' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nbaz"

# Test 4: Multiple branches
test_case \
    "Multiple branches" \
    '1b; 3b; s/foo/FOO/' \
    "foo\nbar\nfoo" \
    "foo\nbar\nfoo"

# Test 5: Label definition and reference
test_case \
    "Label with branch" \
    ':start; s/foo/FOO/; b end; s/bar/BAR/; :end' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nbaz"

# Test 6: Label at specific line
test_case \
    "Label with line address" \
    '2{ s/bar/BAR/; b end }; s/foo/FOO/; :end' \
    "foo\nbar\nbaz" \
    "FOO\nBAR\nbaz"

# Test 7: t command - branch if substitution made
test_case \
    "t command with substitution" \
    's/foo/FOO/; t end; s/bar/BAR/; :end' \
    "foo\nbar" \
    "FOO\nBAR"

# Test 8: t command - no branch if no substitution
test_case \
    "t command no substitution" \
    's/foo/FOO/; t end; s/bar/BAR/; :end' \
    "baz\nbar" \
    "baz\nBAR"

# Test 9: T command - branch if NO substitution
test_case \
    "T command no substitution" \
    's/foo/FOO/; T skip; s/bar/BAR/; b end; :skip; s/baz/BAZ/; :end' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nBAZ"

# Test 10: Complex flow control
test_case \
    "Complex flow control" \
    ':start; s/foo/FOO/; /FOO/t next; b end; :next; s/bar/BAR/; :end; s/baz/BAZ/' \
    "foo\nbar\nbaz" \
    "FOO\nbar\nBAZ"

# Test 11: Label with group
test_case \
    "Label with group" \
    '1,3{ s/foo/FOO/; t done }; s/bar/BAR/; :done' \
    "foo\nbar\nfoo" \
    "FOO\nBAR\nFOO"

echo ""
echo "======================================"
echo "Results: $passed passed, $failed failed"
echo "======================================"

# Cleanup
rm -rf "$TEST_DIR"

exit $failed
