#!/bin/bash
# Master test runner for SedX
# Runs all test suites and provides comprehensive reporting

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SEDX_BIN="${SEDX_BIN:-./target/release/sedx}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Check if sedx binary exists
if [ ! -f "$SEDX_BIN" ]; then
    echo -e "${RED}Error: SedX binary not found at $SEDX_BIN${NC}"
    echo "Please build SedX first: cargo build --release"
    exit 1
fi

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   SedX Comprehensive Test Suite${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Binary: $SEDX_BIN"
echo ""

# Test suites
TEST_SUITES=(
    "basic_tests.sh:Basic Command Tests"
    "addressing_tests.sh:Addressing and Range Tests"
    "regex_tests.sh:Regex Flavor Tests (PCRE/ERE/BRE)"
    "pipeline_tests.sh:Pipeline Tests (stdin/stdout)"
    "streaming_tests.sh:Streaming Mode Tests"
    "holdspace_tests.sh:Hold Space Tests"
    "phase5_tests.sh:Phase 5 Tests (Flow Control, File I/O, Additional Commands)"
    "edge_tests.sh:Edge Case Tests"
)

# Results tracking
TOTAL_PASSED=0
TOTAL_FAILED=0
FAILED_SUITES=()

# Run each test suite
for suite_info in "${TEST_SUITES[@]}"; do
    IFS=':' read -r script_name description <<< "$suite_info"
    script_path="$SCRIPT_DIR/scripts/$script_name"

    echo -e "${YELLOW}Running: $description${NC}"
    echo "----------------------------------------"

    if [ ! -f "$script_path" ]; then
        echo -e "${RED}Error: Test script not found: $script_path${NC}"
        continue
    fi

    # Make script executable
    chmod +x "$script_path"

    # Run the test and capture output
    if bash "$script_path" 2>&1 | tee /tmp/test_output.txt; then
        SUITE_RESULT=0
    else
        SUITE_RESULT=$?
    fi

    # Extract results from output
    PASSED=$(grep -oP 'Passed: \K\d+' /tmp/test_output.txt | tail -1)
    FAILED=$(grep -oP 'Failed: \K\d+' /tmp/test_output.txt | tail -1)

    if [ -n "$PASSED" ]; then
        TOTAL_PASSED=$((TOTAL_PASSED + PASSED))
    fi

    if [ -n "$FAILED" ]; then
        TOTAL_FAILED=$((TOTAL_FAILED + FAILED))
        if [ "$FAILED" -gt 0 ]; then
            FAILED_SUITES+=("$description")
        fi
    fi

    echo ""

    # Cleanup
    rm -f /tmp/test_output.txt
done

# Final summary
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   Final Test Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

if [ $TOTAL_FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    echo -e "${GREEN}Total: $TOTAL_PASSED tests passed${NC}"
    echo ""
    exit 0
else
    echo -e "${RED}✗ Some tests failed${NC}"
    echo -e "${GREEN}Passed: $TOTAL_PASSED${NC}"
    echo -e "${RED}Failed: $TOTAL_FAILED${NC}"
    echo ""
    echo -e "${YELLOW}Failed test suites:${NC}"
    for suite in "${FAILED_SUITES[@]}"; do
        echo "  - $suite"
    done
    echo ""
    exit 1
fi
