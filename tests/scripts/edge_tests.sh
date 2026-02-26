#!/bin/bash
# Edge case tests for SedX

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$(dirname "$SCRIPT_DIR")/fixtures"
SEDX_BIN="${SEDX_BIN:-./target/release/sedx}"
TEMP_DIR=$(mktemp -d)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

TESTS_PASSED=0
TESTS_FAILED=0

run_test() {
    local test_name="$1"
    local expression="$2"
    local input_file="$3"
    local expected_file="$4"

    echo -n "Testing: $test_name ... "

    cp "$input_file" "$TEMP_DIR/test_input.txt"
    $SEDX_BIN "$expression" "$TEMP_DIR/test_input.txt" > "$TEMP_DIR/output.txt" 2>&1 || true

    # Read the modified file
        cat "$TEMP_DIR/test_input.txt" > "$TEMP_DIR/output.txt"
    grep -v '^---' "$TEMP_DIR/output.txt" | grep -v '^+++' | grep -v '^@' | grep -v '^[0-9]*c[0-9]*' | sed '/^> /!d; s/^> //' > "$TEMP_DIR/actual.txt" || true

    if [ ! -s "$TEMP_DIR/actual.txt" ]; then
        grep -v '^---' "$TEMP_DIR/output.txt" | grep -v '^+++' | grep -v '^@' | grep -v '^[0-9]*c[0-9]*' | sed '/^> /d; /^< /d' > "$TEMP_DIR/actual.txt" || true
    fi

    if diff -q "$TEMP_DIR/actual.txt" "$expected_file" > /dev/null 2>&1; then
        echo -e "${GREEN}PASSED${NC}"
        ((TESTS_PASSED++))
        return 0
    else
        echo -e "${RED}FAILED${NC}"
        ((TESTS_FAILED++))
        return 1
    fi
}

echo "=== SedX Edge Case Tests ==="
echo ""

run_test "Empty file" \
    's/test/MODIFIED/' \
    "$FIXTURES_DIR/edge/empty_file.inp" \
    "$FIXTURES_DIR/edge/empty_file.good"

run_test "Empty lines in file" \
    's/test/MODIFIED/' \
    "$FIXTURES_DIR/edge/empty_lines.inp" \
    "$FIXTURES_DIR/edge/empty_lines.good"

run_test "Single line file" \
    's/.*/MODIFIED/' \
    "$FIXTURES_DIR/edge/single_line.inp" \
    "$FIXTURES_DIR/edge/single_line.good"

run_test "Special characters" \
    -E \
    's/(\$pecial|backslash)/MODIFIED: $1/' \
    "$FIXTURES_DIR/edge/special_chars.inp" \
    "$FIXTURES_DIR/edge/special_chars.good" \
    "-E"

run_test "Unicode characters" \
    's/(Hello|Привет|Bonjour|こんにちは|안녕하세요)/MODIFIED/' \
    "$FIXTURES_DIR/edge/unicode.inp" \
    "$FIXTURES_DIR/edge/unicode.good"

run_test "Newlines preservation" \
    's/line/LINE/' \
    "$FIXTURES_DIR/edge/newlines.inp" \
    "$FIXTURES_DIR/edge/newlines.good"

run_test "Whitespace handling" \
    -E \
    's/(^\t|  )/MODIFIED: /' \
    "$FIXTURES_DIR/edge/whitespace.inp" \
    "$FIXTURES_DIR/edge/whitespace.good" \
    "-E"

rm -rf "$TEMP_DIR"

echo ""
echo "=== Test Summary ==="
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"

if [ $TESTS_FAILED -gt 0 ]; then
    exit 1
fi

exit 0
