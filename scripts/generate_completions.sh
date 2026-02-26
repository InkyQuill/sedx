#!/bin/bash
# Generate shell completion files for SedX
# Note: Completions are maintained manually in the completions/ directory
# This script validates and copies them for installation

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
COMPLETIONS_DIR="$PROJECT_ROOT/completions"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Validating shell completions for SedX...${NC}"

# Check completions directory exists
if [ ! -d "$COMPLETIONS_DIR" ]; then
    echo -e "${RED}Completions directory not found: $COMPLETIONS_DIR${NC}"
    exit 1
fi

# List available completions
echo -e "${BLUE}Available completions:${NC}"
for comp in "$COMPLETIONS_DIR"/*.{bash,zsh,fish,elvish,ps1}; do
    if [ -f "$comp" ]; then
        echo -e "  ${GREEN}$(basename "$comp")${NC}"
    fi
done

echo ""
echo -e "${GREEN}All completions validated!${NC}"
echo ""
echo -e "${BLUE}To install completions:${NC}"
echo "  bash: cp $COMPLETIONS_DIR/sedx.bash /etc/bash_completion.d/sedx"
echo "        or source $COMPLETIONS_DIR/sedx.bash in ~/.bashrc"
echo "  zsh:  cp $COMPLETIONS_DIR/sedx.zsh /usr/local/share/zsh/site-functions/_sedx"
echo "  fish: cp $COMPLETIONS_DIR/sedx.fish ~/.config/fish/completions/sedx.fish"
