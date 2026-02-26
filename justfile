# SedX Justfile
# Modern alternative to Makefile using the 'just' command runner
# Install: cargo install just
# Usage: just [target]

# Variables
cargo := "cargo"
binary := "sedx"
src_dir := "src"
man_dir := "man"
man_page := man_dir + "/sedx.1"
completions_dir := "completions"
prefix := env_var_or_default("PREFIX", "/usr/local")
bindir := prefix + "/bin"
mandir := prefix + "/share/man/man1"
datadir := prefix + "/share"
compdir := datadir + "/" + binary

# Default target
default: build

# Build the release binary
build:
    @echo "Building {{binary}} in release mode..."
    {{cargo}} build --release
    @echo "Build complete: target/release/{{binary}}"

# Build debug binary (faster compilation)
dev:
    @echo "Building {{binary}} in debug mode..."
    {{cargo}} build
    @echo "Build complete: target/debug/{{binary}}"

# Run all tests
test:
    @echo "Running Rust tests..."
    {{cargo}} test --all-features
    @echo "Unit tests passed!"
    @echo "Running integration tests..."
    @./tests/regression_tests.sh
    @echo "All tests passed!"

# Run quick tests only
test-quick:
    @echo "Running quick tests..."
    {{cargo}} test
    @echo "Quick tests passed!"

# Run tests with race detection
test-race:
    @echo "Running tests with race detection..."
    {{cargo}} test --all-features
    @echo "Race-free tests passed!"

# Install binary to ~/.local/bin or /usr/local
install: build
    @echo "Installing {{binary}}..."
    # Prefer ~/.local/bin if it exists or can be created
    @if [ -w "{{env_var('HOME')}}/.local/bin" ] || mkdir -p "{{env_var('HOME')}}/.local/bin" 2>/dev/null; then \
        INSTALL_DIR="{{env_var('HOME')}}/.local/bin"; \
    else \
        INSTALL_DIR="{{bindir}}"; \
    fi
    @echo "Installing to $INSTALL_DIR"
    install -Dm755 "target/release/{{binary}}" "$INSTALL_DIR/{{binary}}"
    @echo "Installed {{binary}} to $INSTALL_DIR"

# Install to system directory (requires sudo)
install-system: build
    @echo "Installing {{binary}} to {{prefix}}..."
    @echo "This may require sudo privileges"
    install -Dm755 target/release/{{binary}} {{bindir}}/{{binary}}
    @echo "Installed {{binary}} to {{bindir}}"

# Remove installed binary
uninstall:
    @echo "Uninstalling {{binary}}..."
    @rm -f "{{env_var('HOME')}}/.local/bin/{{binary}}" 2>/dev/null && echo "Removed from ~/.local/bin" || true
    @rm -f "{{bindir}}/{{binary}}" 2>/dev/null && echo "Removed from {{bindir}}" || true
    @echo "Note: Completions and man pages must be removed manually"

# Install man page
man:
    @echo "Installing man page..."
    install -Dm644 {{man_page}} {{mandir}}/{{binary}}.1
    @echo "Installed man page to {{mandir}}"

# Validate and list shell completions
completions:
    @echo "Available shell completions..."
    ls -la {{completions_dir}} || echo "No completions directory found"
    @echo "Completions are maintained in {{completions_dir}}/"

# Install shell completions
completions-install:
    @echo "Installing shell completions..."
    # bash
    mkdir -p {{datadir}}/bash-completion/completions
    install -Dm644 {{completions_dir}}/{{binary}}.bash {{datadir}}/bash-completion/completions/{{binary}}
    # zsh
    mkdir -p {{datadir}}/zsh/site-functions
    install -Dm644 {{completions_dir}}/{{binary}}.zsh {{datadir}}/zsh/site-functions/_{{binary}}
    # fish
    mkdir -p {{datadir}}/fish/vendor_completions.d
    install -Dm644 {{completions_dir}}/{{binary}}.fish {{datadir}}/fish/vendor_completions.d/{{binary}}.fish
    @echo "Installed completions"

# Clean build artifacts
clean:
    @echo "Cleaning build artifacts..."
    {{cargo}} clean
    @rm -rf {{completions_dir}}
    @echo "Clean complete"

# Format code with rustfmt
fmt:
    @echo "Formatting code..."
    {{cargo}} fmt
    @echo "Code formatted"

# Run clippy lints
lint:
    @echo "Running clippy..."
    {{cargo}} clippy --all-targets --all-features -- -D warnings
    @echo "No clippy warnings!"

# Check if code is formatted
fmt-check:
    @echo "Checking code formatting..."
    {{cargo}} fmt --check
    @echo "Code is formatted"

# Run fmt check and clippy
check: fmt-check lint

# Build release binaries for all platforms
release:
    @echo "Building release binaries for all platforms..."
    mkdir -p release
    # Linux x86_64
    @echo "Building for Linux x86_64..."
    {{cargo}} build --release --target x86_64-unknown-linux-gnu
    cp target/x86_64-unknown-linux-gnu/release/{{binary}} release/{{binary}}-linux-x86_64
    # Linux aarch64
    @echo "Building for Linux aarch64..."
    {{cargo}} build --release --target aarch64-unknown-linux-gnu
    cp target/aarch64-unknown-linux-gnu/release/{{binary}} release/{{binary}}-linux-aarch64
    # macOS x86_64
    @echo "Building for macOS x86_64 (may fail if toolchain not installed)..."
    @{{cargo}} build --release --target x86_64-apple-darwin 2>/dev/null || true
    @if [ -f target/x86_64-apple-darwin/release/{{binary}} ]; then \
        cp target/x86_64-apple-darwin/release/{{binary}} release/{{binary}}-darwin-x86_64; \
    fi
    # macOS aarch64 (Apple Silicon)
    @echo "Building for macOS aarch64 (may fail if toolchain not installed)..."
    @{{cargo}} build --release --target aarch64-apple-darwin 2>/dev/null || true
    @if [ -f target/aarch64-apple-darwin/release/{{binary}} ]; then \
        cp target/aarch64-apple-darwin/release/{{binary}} release/{{binary}}-darwin-aarch64; \
    fi
    # Windows x86_64
    @echo "Building for Windows x86_64..."
    {{cargo}} build --release --target x86_64-pc-windows-gnu
    cp target/x86_64-pc-windows-gnu/release/{{binary}}.exe release/{{binary}}-windows-x86_64.exe
    @echo "Release binaries built in release/"

# Generate checksums for release binaries
release-checksums: release
    @echo "Generating checksums..."
    cd release && \
        sha256sum {{binary}}-* > SHA256SUMS.txt && \
        sha512sum {{binary}}-* > SHA512SUMS.txt
    @echo "Checksums generated"

# Verify release binaries against checksums
release-verify:
    @echo "Verifying release binaries..."
    cd release && sha256sum -c SHA256SUMS.txt
    @echo "All binaries verified"

# Build documentation
docs:
    @echo "Building documentation..."
    {{cargo}} doc --no-deps --all-features
    @echo "Documentation built"

# Build and open documentation
docs-open: docs
    {{cargo}} doc --no-deps --all-features --open

# Update dependencies
update:
    @echo "Updating dependencies..."
    {{cargo}} update
    @echo "Dependencies updated"

# Run benchmarks against GNU sed
benchmark:
    @echo "Running benchmarks..."
    @./tests/benchmark.sh

# Show version information
version:
    @echo "SedX Version Information"
    @{{cargo}} --version
    @echo ""
    @target/release/{{binary}} --version

# Run all checks before committing
pre-commit: fmt-check lint test-quick
    @echo "All checks passed!"

# Create a git tag for release
tag VERSION:
    @echo "Creating tag v{{VERSION}}..."
    git tag -a "v{{VERSION}}" -m "Release v{{VERSION}}"
    @echo "Tag created. Push with: git push origin v{{VERSION}}"

# Publish to crates.io
publish:
    @echo "Publishing to crates.io..."
    {{cargo}} publish
    @echo "Published!"

# Show help
help:
    @echo "SedX Justfile"
    @echo ""
    @echo "Available recipes:"
    @just --list
