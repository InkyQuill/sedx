#!/bin/bash
# Regression: pattern-address i/a/c must actually modify the file
SEDX="${SEDX_BIN:-./target/release/sedx}"
TMP=$(mktemp -d)
trap "rm -rf $TMP" EXIT

printf "line 1\nfoo\nline 3\n" > "$TMP/t.txt"
BEFORE=$(md5sum "$TMP/t.txt" | awk '{print $1}')
"$SEDX" '/foo/i\BEFORE_FOO' "$TMP/t.txt" > /dev/null 2>&1
AFTER=$(md5sum "$TMP/t.txt" | awk '{print $1}')

if [ "$BEFORE" = "$AFTER" ]; then
    echo "FAIL: file unchanged after pattern-address insert"
    exit 1
fi

EXPECTED=$(printf "line 1\nBEFORE_FOO\nfoo\nline 3\n" | md5sum | awk '{print $1}')
if [ "$AFTER" != "$EXPECTED" ]; then
    echo "FAIL: file content does not match expected insert"
    diff <(printf "line 1\nBEFORE_FOO\nfoo\nline 3\n") "$TMP/t.txt"
    exit 1
fi

echo "PASS: pattern-address insert modifies file"
