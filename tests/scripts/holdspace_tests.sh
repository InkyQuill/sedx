#!/bin/bash
# Hold space tests for SedX

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

echo "=== SedX Hold Space Tests ==="
echo ""

run_test "Hold command (h)" \
    '1h; 3g' \
    "$FIXTURES_DIR/holdspace/hold.inp" \
    "$FIXTURES_DIR/holdspace/hold.good"

run_test "Hold append (H)" \
    '1H; 2H; 3g' \
    "$FIXTURES_DIR/holdspace/hold_append.inp" \
    "$FIXTURES_DIR/holdspace/hold_append.good"

run_test "Get command (g)" \
    '1h; 3g' \
    "$FIXTURES_DIR/holdspace/get.inp" \
    "$FIXTURES_DIR/holdspace/get.good"

run_test "Get append (G)" \
    '1h; 3G' \
    "$FIXTURES_DIR/holdspace/get_append.inp" \
    "$FIXTURES_DIR/holdspace/get_append.good"

run_test "Exchange (x)" \
    '1h; 2x; 3x' \
    "$FIXTURES_DIR/holdspace/exchange.inp" \
    "$FIXTURES_DIR/holdspace/exchange.good"

run_test "Complex hold space operations" \
    '{1h; 2x; 3G; 4g}' \
    "$FIXTURES_DIR/holdspace/complex.inp" \
    "$FIXTURES_DIR/holdspace/complex.good"

rm -rf "$TEMP_DIR"

echo ""
echo "=== Test Summary ==="
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"

if [ $TESTS_FAILED -gt 0 ]; then
    exit 1
fi

exit 0
