#!/bin/bash
# Regex flavor tests for SedX (PCRE, ERE, BRE)

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
    local flags="${5:-}"

    echo -n "Testing: $test_name ... "

    cp "$input_file" "$TEMP_DIR/test_input.txt"

    if [ -n "$flags" ]; then
        $SEDX_BIN $flags --dry-run "$expression" "$TEMP_DIR/test_input.txt" > "$TEMP_DIR/output.txt" 2>&1 || true
    else
        $SEDX_BIN "$expression" "$TEMP_DIR/test_input.txt" > "$TEMP_DIR/output.txt" 2>&1 || true
    fi

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

echo "=== SedX Regex Flavor Tests ==="
echo ""

# PCRE Tests (default)
echo "--- PCRE Tests (Default) ---"
run_test "PCRE groups and backreferences" \
    's/([a-z]+)([0-9]+)/$2-$1/' \
    "$FIXTURES_DIR/regex/pcre_groups.inp" \
    "$FIXTURES_DIR/regex/pcre_groups.good"

run_test "PCRE alternation (cat|dog)" \
    's/(cat|dog)/ANIMAL/g' \
    "$FIXTURES_DIR/regex/pcre_alternation.inp" \
    "$FIXTURES_DIR/regex/pcre_alternation.good"

run_test "PCRE quantifiers (a{3,})" \
    's/a{3,}/MATCH/' \
    "$FIXTURES_DIR/regex/pcre_quantifiers.inp" \
    "$FIXTURES_DIR/regex/pcre_quantifiers.good"

run_test "PCRE character classes" \
    's/[a-z]+/\*\*\*$&\*\*\*/' \
    "$FIXTURES_DIR/regex/pcre_classes.inp" \
    "$FIXTURES_DIR/regex/pcre_classes.good"

run_test "PCRE anchors" \
    's/^start/MODIFIED/m' \
    "$FIXTURES_DIR/regex/pcre_anchors.inp" \
    "$FIXTURES_DIR/regex/pcre_anchors.good"

run_test "PCRE backreference swap" \
    's/(\w+) (\w+)/$2-$1/' \
    "$FIXTURES_DIR/regex/backref_pcre.inp" \
    "$FIXTURES_DIR/regex/backref_pcre.good"

# ERE Tests (-E flag)
echo ""
echo "--- ERE Tests (-E flag) ---"
run_test "ERE basic (foo|bar)" \
    -E \
    's/(foo|bar|test)/\U$1\E/' \
    "$FIXTURES_DIR/regex/ere_basic.inp" \
    "$FIXTURES_DIR/regex/ere_basic.good" \
    "-E"

run_test "ERE groups with backreference" \
    's/([a-z]+)([0-9]+)([a-z]+)/$3$2$1/' \
    "$FIXTURES_DIR/regex/ere_groups.inp" \
    "$FIXTURES_DIR/regex/ere_groups.good" \
    "-E"

run_test "ERE backreference (\1)" \
    's/([a-z]+)[0-9]+\1/$1-\1[0-9]+/' \
    "$FIXTURES_DIR/regex/backref_ere.inp" \
    "$FIXTURES_DIR/regex/backref_ere.good" \
    "-E"

# BRE Tests (-B flag)
echo ""
echo "--- BRE Tests (-B flag) ---"
run_test "BRE basic escaped groups" \
    '\(foo\|test\)' \
    -B \
    's/\(foo\|test\)/MODIFIED/' \
    "$FIXTURES_DIR/regex/bre_basic.inp" \
    "$FIXTURES_DIR/regex/bre_basic.good" \
    "-B"

run_test "BRE groups with backreference" \
    's/\([a-z]\+\)\([0-9]\+\)\([a-z]\+\)/\3\2\1/' \
    "$FIXTURES_DIR/regex/bre_groups.inp" \
    "$FIXTURES_DIR/regex/bre_groups.good" \
    "-B"

run_test "BRE backreference (\1)" \
    's/\([a-z]\+\)[0-9]\+\1/\1-\1[0-9]\+/' \
    "$FIXTURES_DIR/regex/backref_bre.inp" \
    "$FIXTURES_DIR/regex/backref_bre.good" \
    "-B"

rm -rf "$TEMP_DIR"

echo ""
echo "=== Test Summary ==="
echo -e "Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed: ${RED}$TESTS_FAILED${NC}"

if [ $TESTS_FAILED -gt 0 ]; then
    exit 1
fi

exit 0
