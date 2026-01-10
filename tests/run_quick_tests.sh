#!/bin/bash
# Quick test runner for SedX
# Runs a subset of critical tests for fast feedback

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SEDX_BIN="${SEDX_BIN:-./target/release/sedx}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Check if sedx binary exists
if [ ! -f "$SEDX_BIN" ]; then
    echo -e "${RED}Error: SedX binary not found at $SEDX_BIN${NC}"
    echo "Please build SedX first: cargo build --release"
    exit 1
fi

echo -e "${YELLOW}SedX Quick Test Suite${NC}"
echo "Running critical tests..."
echo ""

# Quick test suite (subset of tests)
QUICK_TESTS=(
    "basic_tests.sh"
    "addressing_tests.sh"
    "regex_tests.sh"
)

TOTAL_PASSED=0
TOTAL_FAILED=0

for script_name in "${QUICK_TESTS[@]}"; do
    script_path="$SCRIPT_DIR/scripts/$script_name"

    echo -n "Running $script_name ... "

    chmod +x "$script_path"

    if bash "$script_path" > /tmp/quick_test_output.txt 2>&1; then
        echo -e "${GREEN}PASSED${NC}"

        PASSED=$(grep -oP 'Passed: \K\d+' /tmp/quick_test_output.txt | tail -1)
        if [ -n "$PASSED" ]; then
            TOTAL_PASSED=$((TOTAL_PASSED + PASSED))
        fi
    else
        echo -e "${RED}FAILED${NC}"

        FAILED=$(grep -oP 'Failed: \K\d+' /tmp/quick_test_output.txt | tail -1)
        if [ -n "$FAILED" ]; then
            TOTAL_FAILED=$((TOTAL_FAILED + FAILED))
        fi
    fi

    rm -f /tmp/quick_test_output.txt
done

echo ""
echo "Quick test results: $TOTAL_PASSED passed, $TOTAL_FAILED failed"

if [ $TOTAL_FAILED -gt 0 ]; then
    exit 1
fi

exit 0
