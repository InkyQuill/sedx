#!/bin/bash
# Package SedX for distribution

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
VERSION="${VERSION:-$(grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | head -1 | cut -d '"' -f 2)}"
PACKAGE_NAME="sedx-${VERSION}"
PACKAGE_DIR="$PROJECT_ROOT/dist/$PACKAGE_NAME"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Packaging SedX ${VERSION}...${NC}"

# Clean previous builds
echo -e "${BLUE}Cleaning previous builds...${NC}"
cargo clean

# Build release binary
echo -e "${BLUE}Building release binary...${NC}"
cargo build --release

# Create package directory
rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR"

# Copy binary
echo -e "${BLUE}Copying files...${NC}"
cp "$PROJECT_ROOT/target/release/sedx" "$PACKAGE_DIR/"

# Copy documentation
mkdir -p "$PACKAGE_DIR/share/man/man1"
cp "$PROJECT_ROOT/man/sedx.1" "$PACKAGE_DIR/share/man/man1/"

# Copy license and readme
cp "$PROJECT_ROOT/LICENSE" "$PACKAGE_DIR/"
cp "$PROJECT_ROOT/README.md" "$PACKAGE_DIR/"

# Copy completions
echo -e "${BLUE}Copying completions...${NC}"
mkdir -p "$PACKAGE_DIR/share/completions"
cp -r "$PROJECT_ROOT/completions/"* "$PACKAGE_DIR/share/completions/"

# Create tarball
echo -e "${BLUE}Creating tarball...${NC}"
cd "$PROJECT_ROOT/dist"
tar czvf "${PACKAGE_NAME}.tar.gz" "$PACKAGE_NAME"
cd "$PROJECT_ROOT"

# Generate checksums
echo -e "${BLUE}Generating checksums...${NC}"
cd "$PROJECT_ROOT/dist"
sha256sum "${PACKAGE_NAME}.tar.gz" > "${PACKAGE_NAME}.tar.gz.sha256"
sha512sum "${PACKAGE_NAME}.tar.gz" > "${PACKAGE_NAME}.tar.gz.sha512"
cd "$PROJECT_ROOT"

echo -e "${GREEN}Package created: dist/${PACKAGE_NAME}.tar.gz${NC}"
echo -e "${GREEN}Checksums: dist/${PACKAGE_NAME}.tar.gz.sha256${NC}"
echo -e "${GREEN}          dist/${PACKAGE_NAME}.tar.gz.sha512${NC}"
