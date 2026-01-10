#!/bin/bash
# Phase 4 Comprehensive Test Suite
# Tests -n, -e, -f flags, Q command, multi-line operations (n, N, P, D), and backup optimization

set -e

# Get absolute path to sedx binary
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SEDX="$PROJECT_ROOT/target/release/sedx"
TEST_DIR="/tmp/sedx_phase4_tests"
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Setup
echo "=== Phase 4 Comprehensive Test Suite ==="
echo "Setting up test directory..."
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

# Helper functions
run_test() {
    local test_name="$1"
    local sedx_cmd="$2"
    local expected="$3"
    local input="$4"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    # Create input file
    if [ -n "$input" ]; then
        echo -e "$input" > test_input.txt
    else
        echo "" > test_input.txt
    fi

    # Run sedx
    if [[ "$sedx_cmd" == *"$SEDX"* ]] && [[ "$sedx_cmd" != *"|"* ]]; then
        # File operation: run in execute mode, check file content
        eval "$sedx_cmd" > output.txt 2>&1 || true
        actual=$(cat test_input.txt)
    else
        # Pipe operation: run as-is
        eval "$sedx_cmd" > output.txt 2>&1 || true
        actual=$(cat output.txt)
    fi

    if [ "$actual" = "$expected" ]; then
        echo -e "${GREEN}✓${NC} $test_name"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${RED}✗${NC} $test_name"
        echo -e "  Expected: '$expected'"
        echo -e "  Actual:   '$actual'"
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
}

# Test 1: -n flag (suppress automatic output)
echo ""
echo "=== Testing -n flag ==="

run_test "-n with print" \
    "printf 'line1\nline2\nline3\n' | $SEDX -n '2p'" \
    "line2"

run_test "-n with range" \
    "printf 'a\nb\nc\nd\n' | $SEDX -n '1,3p'" \
    "a
b
c"

run_test "-n with pattern print" \
    "printf 'foo\nbar\nbaz\n' | $SEDX -n '/bar/p'" \
    "bar"

run_test "-n with substitution print flag" \
    "printf 'foo\nbar\nfoo\n' | $SEDX -n 's/foo/FOO/p'" \
    "FOO
FOO"

# Test 2: -e flag (multiple expressions)
echo ""
echo "=== Testing -e flag ==="

run_test "-e with two substitutions" \
    "$SEDX -e 's/foo/FOO/' -e 's/bar/BAR/' test_input.txt" \
    "FOO
BAR
baz" \
    "foo\nbar\nbaz"

run_test "-e with substitution and delete" \
    "$SEDX -e 's/foo/FOO/' -e '2d' test_input.txt" \
    "FOO
baz" \
    "foo\nbar\nbaz"

run_test "-e with three commands" \
    "$SEDX -e 's/a/A/' -e 's/b/B/' -e 's/c/C/' test_input.txt" \
    "A
B
C" \
    "a\nb\nc"

# Test 3: Q command (quit without printing)
echo ""
echo "=== Testing Q command ==="

run_test "Q at line 2" \
    "printf 'line1\nline2\nline3\n' | $SEDX '2Q'" \
    "line1"

run_test "Q with pattern" \
    "printf 'foo\nbar\nbaz\n' | $SEDX '/bar/Q'" \
    "foo"

run_test "Q vs q (line 2)" \
    "printf '1\n2\n3\n' | $SEDX '2Q'" \
    "1"

run_test "Q without address" \
    "printf 'test\n' | $SEDX 'Q'" \
    ""

# Test 4: -f flag (script files)
echo ""
echo "=== Testing -f flag ==="

cat > test_script.sed << 'EOF'
# Test script
s/foo/FOO/g
s/bar/BAR/g
EOF

run_test "-f with simple script" \
    "$SEDX -f test_script.sed test_input.txt" \
    "FOO
BAR
baz" \
    "foo\nbar\nbaz"

cat > multi_cmd.sed << 'EOF'
s/foo/FOO/
s/bar/BAR/
5,10d
EOF

run_test "-f with multiple commands" \
    "$SEDX -f multi_cmd.sed test_input.txt" \
    "FOO
BAR
baz
keep" \
    "foo\nbar\nbaz\nkeep\nkeep\nkeep\nkeep\nkeep"

cat > shebang.sed << 'EOF'
#!/usr/bin/sedx -f
# This is a comment
s/test/TEST/
s/demo/DEMO/
EOF

run_test "-f with shebang and comments" \
    "$SEDX -f shebang.sed test_input.txt" \
    "TEST
DEMO" \
    "test\ndemo"

# Test 5: Combining -f with -e
echo ""
echo "=== Testing -f combined with -e ==="

cat > base.sed << 'EOF'
s/foo/FOO/
s/bar/BAR/
EOF

run_test "-f and -e together" \
    "$SEDX -f base.sed -e 's/baz/BAZ/' test_input.txt" \
    "FOO
BAR
BAZ" \
    "foo\nbar\nbaz"

# Test 6: Multi-line pattern space operations (n, N, P, D)
echo ""
echo "=== Testing multi-line operations ==="

run_test "n command (print and delete next line)" \
    "printf 'a\nb\nc\n' | $SEDX 'n; d'" \
    "a
c"

run_test "n command with substitution" \
    "printf 'foo\nbar\nbaz\n' | $SEDX 'n; s/b/B/'" \
    "foo
Bar
baz"

run_test "N command (append next line)" \
    "printf 'a\nb\nc\n' | $SEDX 'N; s/\n/ /'" \
    "a b
c"

run_test "P command (print first line)" \
    "printf 'line1\nline2\nline3\n' | $SEDX 'N; P'" \
    "line1
line1
line2
line3"

# Test 7: Edge cases
echo ""
echo "=== Testing edge cases ==="

run_test "Empty file with -n" \
    "$SEDX -n '1p' test_input.txt" \
    ""

run_test "Single line file" \
    "$SEDX 's/foo/bar/' test_input.txt" \
    "bar" \
    "foo"

run_test "Script file with only comments" \
    "$SEDX -f test_input.txt test_input.txt 2>&1 | cat" \
    "Error: Script file 'test_input.txt' is empty or contains no valid commands" \
    "# comment\n# another comment"

# Create script with only comments
cat > comments_only.sed << 'EOF'
# This is a comment
# So is this
EOF

run_test "Empty script file" \
    "$SEDX -f comments_only.sed test_input.txt 2>&1 | head -1" \
    "Error: Script file 'comments_only.sed' is empty or contains no valid commands"

run_test "File with only newlines" \
    "$SEDX 's/^$/EMPTY/' test_input.txt" \
    "EMPTY
EMPTY
EMPTY" \
    "\n\n"

# Test 8: Backup optimization
echo ""
echo "=== Testing backup optimization ==="

run_test "Read-only command (no backup)" \
    "$SEDX -n '1p' test_input.txt 2>&1 | grep -o 'No backup needed'" \
    "No backup needed" \
    "foo\nbar\nbaz"

run_test "Modifying command (with backup)" \
    "$SEDX 's/foo/bar/' test_input.txt 2>&1 | grep -o 'Backup created'" \
    "Backup created" \
    "foo"

# Test 9: Pattern matching with new features
echo ""
echo "=== Testing pattern matching ==="

run_test "-n with address range" \
    "printf '1\n2\n3\n4\n5\n' | $SEDX -n '2,4p'" \
    "2
3
4"

run_test "-e with negation" \
    "$SEDX -e 's/foo/FOO/' -e '/bar/!d' test_input.txt" \
    "bar" \
    "foo\nbar\nbaz"

# Test 10: Complex scenarios
echo ""
echo "=== Testing complex scenarios ==="

cat > complex.sed << 'EOF'
# Convert markdown headers to HTML
s|^# (.*)|<h1>$1</h1>|g
s|^## (.*)|<h2>$1</h2>|g
s|^### (.*)|<h3>$1</h3>|g
# Remove extra formatting
s|\*\*||g
s|__||g
EOF

run_test "Complex script file" \
    "$SEDX -f complex.sed test_input.txt" \
    "<h1>Header One</h1>
<h2>Header Two</h2>
<h3>Header Three</h3>" \
    "# Header One\n## Header Two\n### Header Three"

# Cleanup
cd ..
echo ""
echo "=== Test Summary ==="
echo "Total tests: $TOTAL_TESTS"
echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
echo -e "${RED}Failed: $FAILED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
