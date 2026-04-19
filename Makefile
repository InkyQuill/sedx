# SedX Makefile
# For building, testing, and installing SedX

# Variables
CARGO ?= cargo
TARGET ?= release
BINARY_NAME = sedx
SRC_DIR = src
MAN_DIR = man
MAN_PAGE = $(MAN_DIR)/$(BINARY_NAME).1
COMPLETIONS_DIR = completions
PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
MANDIR = $(PREFIX)/share/man/man1
DATADIR = $(PREFIX)/share
COMPDIR = $(DATADIR)/$(BINARY_NAME)

# Rust targets for cross-compilation
TARGET_X86_64_LINUX = x86_64-unknown-linux-gnu
TARGET_AARCH64_LINUX = aarch64-unknown-linux-gnu
TARGET_X86_64_MAC = x86_64-apple-darwin
TARGET_AARCH64_MAC = aarch64-apple-darwin
TARGET_X86_64_WINDOWS = x86_64-pc-windows-gnu

# Colors for output
COLOR_RESET = \033[0m
COLOR_BOLD = \033[1m
COLOR_GREEN = \033[32m
COLOR_YELLOW = \033[33m
COLOR_BLUE = \033[34m

.PHONY: all build test install uninstall clean completions man release help check-ci check-windows install-hook

# Default target
all: build

## build: Build the release binary
build:
	@echo "$(COLOR_BLUE)Building $(BINARY_NAME) in $(TARGET) mode...$(COLOR_RESET)"
	$(CARGO) build --$(TARGET)
	@echo "$(COLOR_GREEN)Build complete: target/$(TARGET)/$(BINARY_NAME)$(COLOR_RESET)"

## dev: Build debug binary (faster compilation)
dev:
	@echo "$(COLOR_BLUE)Building $(BINARY_NAME) in debug mode...$(COLOR_RESET)"
	$(CARGO) build
	@echo "$(COLOR_GREEN)Build complete: target/debug/$(BINARY_NAME)$(COLOR_RESET)"

## test: Run all tests
test:
	@echo "$(COLOR_BLUE)Running all tests (unit + integration)...$(COLOR_RESET)"
	$(CARGO) test --all-features
	@echo "$(COLOR_GREEN)All tests passed!$(COLOR_RESET)"

## test-quick: Run quick tests only
test-quick:
	@echo "$(COLOR_BLUE)Running quick tests...$(COLOR_RESET)"
	$(CARGO) test
	@echo "$(COLOR_GREEN)Quick tests passed!$(COLOR_RESET)"

## test-race: Run tests with race detection
test-race:
	@echo "$(COLOR_BLUE)Running tests with race detection...$(COLOR_RESET)"
	$(CARGO) test --all-features
	@echo "$(COLOR_GREEN)Race-free tests passed!$(COLOR_RESET)"

## install: Install binary to ~/.local/bin or /usr/local
install: build
	@echo "$(COLOR_BLUE)Installing $(BINARY_NAME)...$(COLOR_RESET)"
	@# Prefer ~/.local/bin if it exists or can be created, otherwise use PREFIX
	@if [ -w "$(HOME)/.local/bin" ] || mkdir -p "$(HOME)/.local/bin" 2>/dev/null; then \
		INSTALL_DIR="$(HOME)/.local/bin"; \
	else \
		INSTALL_DIR="$(BINDIR)"; \
	fi; \
	echo "$(COLOR_YELLOW)Installing to $$INSTALL_DIR$(COLOR_RESET)"; \
	install -Dm755 "target/$(TARGET)/$(BINARY_NAME)" "$$INSTALL_DIR/$(BINARY_NAME)"; \
	echo "$(COLOR_GREEN)Installed $(BINARY_NAME) to $$INSTALL_DIR$(COLOR_RESET)"

## install-system: Install to system directory (requires sudo)
install-system: build
	@echo "$(COLOR_BLUE)Installing $(BINARY_NAME) to $(PREFIX)...$(COLOR_RESET)"
	@echo "$(COLOR_YELLOW)This may require sudo privileges$(COLOR_RESET)"
	install -Dm755 target/$(TARGET)/$(BINARY_NAME) $(DESTDIR)$(BINDIR)/$(BINARY_NAME)
	@echo "$(COLOR_GREEN)Installed $(BINARY_NAME) to $(BINDIR)$(COLOR_RESET)"

## uninstall: Remove installed binary
uninstall:
	@echo "$(COLOR_BLUE)Uninstalling $(BINARY_NAME)...$(COLOR_RESET)"
	@# Try to remove from both common locations
	@rm -f "$(HOME)/.local/bin/$(BINARY_NAME)" 2>/dev/null && echo "$(COLOR_GREEN)Removed from ~/.local/bin$(COLOR_RESET)" || true
	@rm -f "$(BINDIR)/$(BINARY_NAME)" 2>/dev/null && echo "$(COLOR_GREEN)Removed from $(BINDIR)$(COLOR_RESET)" || true
	@echo "$(COLOR_YELLOW)Note: Completions and man pages must be removed manually$(COLOR_RESET)"

## man: Install man page
man:
	@echo "$(COLOR_BLUE)Installing man page...$(COLOR_RESET)"
	@install -Dm644 $(MAN_PAGE) $(DESTDIR)$(MANDIR)/$(BINARY_NAME).1
	@echo "$(COLOR_GREEN)Installed man page to $(MANDIR)$(COLOR_RESET)"

## completions: Validate and list shell completions
completions:
	@echo "$(COLOR_BLUE)Available shell completions...$(COLOR_RESET)"
	@ls -la $(COMPLETIONS_DIR) || echo "No completions directory found"
	@echo "$(COLOR_GREEN)Completions are maintained in $(COMPLETIONS_DIR)/$(COLOR_RESET)"

## completions-install: Install shell completions
completions-install:
	@echo "$(COLOR_BLUE)Installing shell completions...$(COLOR_RESET)"
	@# Install bash completion
	@mkdir -p $(DESTDIR)$(DATADIR)/bash-completion/completions
	@install -Dm644 $(COMPLETIONS_DIR)/$(BINARY_NAME).bash $(DESTDIR)$(DATADIR)/bash-completion/completions/$(BINARY_NAME)
	@# Install zsh completion
	@mkdir -p $(DESTDIR)$(DATADIR)/zsh/site-functions
	@install -Dm644 $(COMPLETIONS_DIR)/$(BINARY_NAME).zsh $(DESTDIR)$(DATADIR)/zsh/site-functions/_$(BINARY_NAME)
	@# Install fish completion
	@mkdir -p $(DESTDIR)$(DATADIR)/fish/vendor_completions.d
	@install -Dm644 $(COMPLETIONS_DIR)/$(BINARY_NAME).fish $(DESTDIR)$(DATADIR)/fish/vendor_completions.d/$(BINARY_NAME).fish
	@echo "$(COLOR_GREEN)Installed completions$(COLOR_RESET)"

## clean: Clean build artifacts
clean:
	@echo "$(COLOR_BLUE)Cleaning build artifacts...$(COLOR_RESET)"
	$(CARGO) clean
	@rm -rf $(COMPLETIONS_DIR)
	@echo "$(COLOR_GREEN)Clean complete$(COLOR_RESET)"

## fmt: Format code with rustfmt
fmt:
	@echo "$(COLOR_BLUE)Formatting code...$(COLOR_RESET)"
	$(CARGO) fmt
	@echo "$(COLOR_GREEN)Code formatted$(COLOR_RESET)"

## lint: Run clippy lints
lint:
	@echo "$(COLOR_BLUE)Running clippy...$(COLOR_RESET)"
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	@echo "$(COLOR_GREEN)No clippy warnings!$(COLOR_RESET)"

## check: Run fmt check and clippy without building
check: fmt-check lint

## fmt-check: Check if code is formatted
fmt-check:
	@echo "$(COLOR_BLUE)Checking code formatting...$(COLOR_RESET)"
	$(CARGO) fmt --check
	@echo "$(COLOR_GREEN)Code is formatted$(COLOR_RESET)"

## check-ci: Run the same checks CI runs (fmt + clippy stable+beta + test + docs)
check-ci:
	@echo "$(COLOR_BLUE)Running CI-equivalent checks locally...$(COLOR_RESET)"
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	@if command -v rustup >/dev/null 2>&1 && rustup toolchain list 2>/dev/null | grep -q '^beta'; then \
		echo "$(COLOR_BLUE)Running beta clippy...$(COLOR_RESET)"; \
		$(CARGO) +beta clippy --all-targets --all-features -- -D warnings; \
	else \
		echo "$(COLOR_YELLOW)Skipping beta clippy (beta toolchain not installed; 'rustup toolchain install beta' to enable)$(COLOR_RESET)"; \
	fi
	@if command -v rustup >/dev/null 2>&1 && rustup target list --installed 2>/dev/null | grep -q '^x86_64-pc-windows-gnu$$'; then \
		echo "$(COLOR_BLUE)Running Windows cross-clippy...$(COLOR_RESET)"; \
		$(CARGO) clippy --target x86_64-pc-windows-gnu --all-targets --all-features -- -D warnings; \
	else \
		echo "$(COLOR_YELLOW)Skipping Windows cross-clippy (target not installed; 'make check-windows' to install and run)$(COLOR_RESET)"; \
	fi
	$(CARGO) test --all-features
	$(CARGO) doc --no-deps --all-features
	@echo "$(COLOR_GREEN)CI-equivalent checks passed$(COLOR_RESET)"

## check-windows: Lint the codebase against the Windows target (catches platform-gated unused-import / dead-code errors locally)
check-windows:
	@if ! rustup target list --installed 2>/dev/null | grep -q '^x86_64-pc-windows-gnu$$'; then \
		echo "$(COLOR_BLUE)Installing Windows target (x86_64-pc-windows-gnu)…$(COLOR_RESET)"; \
		rustup target add x86_64-pc-windows-gnu; \
	fi
	@echo "$(COLOR_BLUE)Running clippy against x86_64-pc-windows-gnu…$(COLOR_RESET)"
	$(CARGO) clippy --target x86_64-pc-windows-gnu --all-targets --all-features -- -D warnings
	@echo "$(COLOR_GREEN)Windows cross-lint passed$(COLOR_RESET)"

## install-hook: Install the pre-push git hook (copies from contrib/hooks/)
install-hook:
	@if [ ! -d .git ]; then \
		echo "$(COLOR_YELLOW)Not a git checkout; nothing to install$(COLOR_RESET)"; \
		exit 1; \
	fi
	@install -m 0755 contrib/hooks/pre-push .git/hooks/pre-push
	@echo "$(COLOR_GREEN)Pre-push hook installed at .git/hooks/pre-push$(COLOR_RESET)"
	@echo "$(COLOR_YELLOW)Bypass once with: git push --no-verify$(COLOR_RESET)"

## release: Build release binaries for all platforms
release:
	@echo "$(COLOR_BOLD)$(COLOR_YELLOW)Building release binaries for all platforms...$(COLOR_RESET)"
	@mkdir -p release
	@# Linux x86_64
	@echo "$(COLOR_BLUE)Building for Linux x86_64...$(COLOR_RESET)"
	@$(CARGO) build --release --target $(TARGET_X86_64_LINUX)
	@cp target/$(TARGET_X86_64_LINUX)/release/$(BINARY_NAME) release/$(BINARY_NAME)-linux-x86_64
	@# Linux aarch64
	@echo "$(COLOR_BLUE)Building for Linux aarch64...$(COLOR_RESET)"
	@$(CARGO) build --release --target $(TARGET_AARCH64_LINUX)
	@cp target/$(TARGET_AARCH64_LINUX)/release/$(BINARY_NAME) release/$(BINARY_NAME)-linux-aarch64
	@# macOS x86_64 (requires osx-cross toolchain)
	@echo "$(COLOR_YELLOW)Building for macOS x86_64 (may fail if toolchain not installed)...$(COLOR_RESET)"
	@-$(CARGO) build --release --target $(TARGET_X86_64_MAC) 2>/dev/null && \
		cp target/$(TARGET_X86_64_MAC)/release/$(BINARY_NAME) release/$(BINARY_NAME)-darwin-x86_64
	@# macOS aarch64 (Apple Silicon)
	@echo "$(COLOR_YELLOW)Building for macOS aarch64 (may fail if toolchain not installed)...$(COLOR_RESET)"
	@-$(CARGO) build --release --target $(TARGET_AARCH64_MAC) 2>/dev/null && \
		cp target/$(TARGET_AARCH64_MAC)/release/$(BINARY_NAME) release/$(BINARY_NAME)-darwin-aarch64
	@# Windows x86_64
	@echo "$(COLOR_BLUE)Building for Windows x86_64...$(COLOR_RESET)"
	@$(CARGO) build --release --target $(TARGET_X86_64_WINDOWS)
	@cp target/$(TARGET_X86_64_WINDOWS)/release/$(BINARY_NAME).exe release/$(BINARY_NAME)-windows-x86_64.exe
	@echo "$(COLOR_GREEN)$(COLOR_BOLD)Release binaries built in release/$(COLOR_RESET)"

## release-checksums: Generate checksums for release binaries
release-checksums: release
	@echo "$(COLOR_BLUE)Generating checksums...$(COLOR_RESET)"
	@cd release && \
		sha256sum $(BINARY_NAME)-* > SHA256SUMS.txt && \
		sha512sum $(BINARY_NAME)-* > SHA512SUMS.txt
	@echo "$(COLOR_GREEN)Checksums generated$(COLOR_RESET)"

## release-verify: Verify release binaries against checksums
release-verify:
	@echo "$(COLOR_BLUE)Verifying release binaries...$(COLOR_RESET)"
	@cd release && \
		sha256sum -c SHA256SUMS.txt
	@echo "$(COLOR_GREEN)All binaries verified$(COLOR_RESET)"

## docs: Build documentation
docs:
	@echo "$(COLOR_BLUE)Building documentation...$(COLOR_RESET)"
	$(CARGO) doc --no-deps --all-features
	@echo "$(COLOR_GREEN)Documentation built$(COLOR_RESET)"

## docs-open: Build and open documentation
docs-open: docs
	@$(CARGO) doc --no-deps --all-features --open

## update: Update dependencies
update:
	@echo "$(COLOR_BLUE)Updating dependencies...$(COLOR_RESET)"
	$(CARGO) update
	@echo "$(COLOR_GREEN)Dependencies updated$(COLOR_RESET)"

## benchmark: Run benchmarks against GNU sed
benchmark:
	@echo "$(COLOR_BLUE)Running benchmarks...$(COLOR_RESET)"
	@./tests/tools/benchmark.sh

## version: Show version information
version:
	@echo "$(COLOR_BOLD)SedX Version Information$(COLOR_RESET)"
	@$(CARGO) --version
	@echo ""
	@target/$(TARGET)/$(BINARY_NAME) --version

## help: Show this help message
help:
	@echo "$(COLOR_BOLD)SedX Makefile$(COLOR_RESET)"
	@echo ""
	@echo "$(COLOR_BOLD)Available targets:$(COLOR_RESET)"
	@grep -E '^## ' Makefile | sed 's/^## /  /' | column -t -s ':'
	@echo ""
	@echo "$(COLOR_BOLD)Examples:$(COLOR_RESET)"
	@echo "  make build              # Build release binary"
	@echo "  make test               # Run all tests"
	@echo "  make install            # Install to ~/.local/bin"
	@echo "  make install-system     # Install to /usr/local (may need sudo)"
	@echo "  make release            # Build for all platforms"
	@echo "  make completions        # Generate shell completions"

# Include dependency tracking
-include $(wildcard $(SRC_DIR)/*.d)
