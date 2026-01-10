#!/bin/bash
# Generate large test files for streaming tests
# Creates files of various sizes for testing streaming mode

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/fixtures/streaming"
LARGE_FILE_DIR="$SCRIPT/fixtures/large_files"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

mkdir -p "$LARGE_FILE_DIR"

echo -e "${YELLOW}Generating large test files for streaming tests...${NC}"
echo ""

# Generate 10MB file
echo -n "Generating 10MB test file... "
python3 << 'EOF' > "$LARGE_FILE_DIR/test_10mb.txt"
import sys

for i in range(100000):
    print(f"line {i}: foo bar baz qux test data here")
EOF
echo -e "${GREEN}done${NC}"

# Generate 100MB file
echo -n "Generating 100MB test file... "
python3 << 'EOF' > "$LARGE_FILE_DIR/test_100mb.txt"
import sys

for i in range(1000000):
    print(f"line {i}: foo bar baz qux test data here")
EOF
echo -e "${GREEN}done${NC}"

# Generate 1GB file (optional - comment out if too large)
# echo -n "Generating 1GB test file... "
# python3 << 'EOF' > "$LARGE_FILE_DIR/test_1gb.txt"
# import sys
#
# for i in range(10000000):
#     print(f"line {i}: foo bar baz qux test data here")
# EOF
# echo -e "${GREEN}done${NC}"

echo ""
echo "Large test files generated in $LARGE_FILE_DIR:"
ls -lh "$LARGE_FILE_DIR"
