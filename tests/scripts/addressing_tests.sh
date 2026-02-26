#!/bin/bash
# Addressing and range tests for SedX

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

echo "=== SedX Addressing and Range Tests ==="
echo ""

run_test "Line number addressing (3,5s/./MODIFIED/)" \
    '3,5s/.*/MODIFIED/' \
    "$FIXTURES_DIR/addressing/line_number.inp" \
    "$FIXTURES_DIR/addressing/line_number.good"

run_test "Pattern addressing (/apple/s/.*/MODIFIED/)" \
    '/apple/s/.*/MODIFIED/' \
    "$FIXTURES_DIR/addressing/pattern.inp" \
    "$FIXTURES_DIR/addressing/pattern.good"

run_test "Line number range (3,5d)" \
    '3,5d' \
    "$FIXTURES_DIR/addressing/range_line_number.inp" \
    "$FIXTURES_DIR/addressing/range_line_number.good"

run_test "Pattern range (/start/,/end/d)" \
    '/start/,/end/d' \
    "$FIXTURES_DIR/addressing/range_pattern.inp" \
    "$FIXTURES_DIR/addressing/range_pattern.good"

run_test "Mixed range (/marker/,5d)" \
    '/marker/,5d' \
    "$FIXTURES_DIR/addressing/range_mixed.inp" \
    "$FIXTURES_DIR/addressing/range_mixed.good"

run_test "Negation (/delete/!d)" \
    '/delete/!d' \
    "$FIXTURES_DIR/addressing/negation.inp" \
    "$FIXTURES_DIR/addressing/negation.good"

run_test "Relative offset (/marker/,+2s/.*/MODIFIED/)" \
    '/marker/,+2s/.*/MODIFIED/' \
    "$FIXTURES_DIR/addressing/relative.inp" \
    "$FIXTURES_DIR/addressing/relative.good"

run_test "Last line ($s/.*/THIS IS THE LAST LINE/)" \
    '$s/.*/THIS IS THE LAST LINE/' \
    "$FIXTURES_DIR/addressing/last_line.inp" \
    "$FIXTURES_DIR/addressing/last_line.good"

run_test "Stepping (1~2s/.*/MODIFIED/)" \
    '1~2s/.*/MODIFIED/' \
    "$FIXTURES_DIR/addressing/stepping.inp" \
    "$FIXTURES_DIR/addressing/stepping.good"

rm -rf "$TEMP_DIR"

echo ""
echo "=== Test Summary ==="
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"

if [ $TESTS_FAILED -gt 0 ]; then
    exit 1
fi

exit 0
