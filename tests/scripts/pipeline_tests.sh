#!/bin/bash
# Stdin/Stdout pipeline tests for SedX

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

run_pipeline_test() {
    local test_name="$1"
    local expression="$2"
    local input_file="$3"
    local expected_file="$4"

    echo -n "Testing: $test_name ... "

    # Test with stdin/stdout
    cat "$input_file" | $SEDX_BIN "$expression" > "$TEMP_DIR/output.txt" 2>&1 || true

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

echo "=== SedX Pipeline Tests (stdin/stdout) ==="
echo ""

run_pipeline_test "Simple substitute in pipeline" \
    's/foo/MODIFIED/' \
    "$FIXTURES_DIR/pipeline/simple_substitute.inp" \
    "$FIXTURES_DIR/pipeline/simple_substitute.good"

run_pipeline_test "Delete pattern in pipeline" \
    '/delete/d' \
    "$FIXTURES_DIR/pipeline/delete_pattern.inp" \
    "$FIXTURES_DIR/pipeline/delete_pattern.good"

run_pipeline_test "Multiple commands with -e" \
    -e \
    's/foo/MODIFIED/' \
    -e \
    '/line/s/./MODIFIED/' \
    "$FIXTURES_DIR/pipeline/multiple_commands.inp" \
    "$FIXTURES_DIR/pipeline/multiple_commands.good"

run_pipeline_test "Group commands in pipeline" \
    '{2s/./MODIFIED 2/; 4s/./MODIFIED 4/}' \
    "$FIXTURES_DIR/pipeline/group_commands.inp" \
    "$FIXTURES_DIR/pipeline/group_commands.good"

run_pipeline_test "Case-insensitive in pipeline" \
    's/foo/MODIFIED/i' \
    "$FIXTURES_DIR/pipeline/case_insensitive.inp" \
    "$FIXTURES_DIR/pipeline/case_insensitive.good"

run_pipeline_test "Global substitute in pipeline" \
    's/foo/qux/g' \
    "$FIXTURES_DIR/pipeline/global_substitute.inp" \
    "$FIXTURES_DIR/pipeline/global_substitute.good"

rm -rf "$TEMP_DIR"

echo ""
echo "=== Test Summary ==="
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"

if [ $TESTS_FAILED -gt 0 ]; then
    exit 1
fi

exit 0
