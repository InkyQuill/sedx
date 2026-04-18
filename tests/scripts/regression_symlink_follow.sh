#!/bin/bash
# Regression: editing a symlink must modify the target file, not replace the symlink
SEDX="${SEDX_BIN:-./target/release/sedx}"
# Resolve to absolute path before changing directories
SEDX="$(cd "$(dirname "$SEDX")" && pwd)/$(basename "$SEDX")"
TMP=$(mktemp -d)
trap "rm -rf $TMP" EXIT

cd "$TMP"
echo "original" > target.txt
ln -s target.txt link.txt
"$SEDX" 's/original/CHANGED/' link.txt > /dev/null 2>&1

if [ ! -L link.txt ]; then
    echo "FAIL: link.txt is no longer a symlink"
    ls -la link.txt
    exit 1
fi

if ! grep -q "CHANGED" target.txt; then
    echo "FAIL: target.txt was not modified"
    cat target.txt
    exit 1
fi

if grep -q "original" target.txt; then
    echo "FAIL: target.txt still contains old content"
    exit 1
fi

echo "PASS: symlink intact, target updated"
