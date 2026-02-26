#!/usr/bin/env bash
# Installation script for SedX shell completions
# Usage: ./install.sh [bash|zsh|fish|powershell|all]

set -e

# Detect the completion directory
COMPLETION_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$COMPLETION_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

install_bash() {
    print_info "Installing Bash completion..."

    # Check for bash-completion
    if ! command -v bash &> /dev/null; then
        print_error "Bash is not installed"
        return 1
    fi

    # Try system-wide installation first
    if [[ -w /usr/share/bash-completion/completions ]]; then
        sudo cp "$COMPLETION_DIR/bash.sedx" /usr/share/bash-completion/completions/sedx
        print_info "Installed to /usr/share/bash-completion/completions/sedx"
    else
        # User installation
        mkdir -p "$HOME/.local/share/bash-completion/completions"
        cp "$COMPLETION_DIR/bash.sedx" "$HOME/.local/share/bash-completion/completions/sedx"

        # Add to .bashrc if not already present
        BASHRC="$HOME/.bashrc"
        SOURCE_LINE='source "$HOME/.local/share/bash-completion/completions/sedx"'

        if ! grep -q "$SOURCE_LINE" "$BASHRC" 2>/dev/null; then
            echo "" >> "$BASHRC"
            echo "# SedX completion" >> "$BASHRC"
            echo "$SOURCE_LINE" >> "$BASHRC"
            print_info "Added source line to $BASHRC"
        fi

        print_info "Installed to ~/.local/share/bash-completion/completions/sedx"
        print_warn "Run 'source ~/.bashrc' or restart your shell to use completions"
    fi
}

install_zsh() {
    print_info "Installing Zsh completion..."

    if ! command -v zsh &> /dev/null; then
        print_error "Zsh is not installed"
        return 1
    fi

    # Try system-wide installation first
    if [[ -w /usr/share/zsh/vendor-completions ]]; then
        sudo cp "$COMPLETION_DIR/zsh.sedx" /usr/share/zsh/vendor-completions/_sedx
        sudo chmod +x /usr/share/zsh/vendor-completions/_sedx
        print_info "Installed to /usr/share/zsh/vendor-completions/_sedx"
    else
        # User installation
        mkdir -p "$HOME/.zsh/completions"
        cp "$COMPLETION_DIR/zsh.sedx" "$HOME/.zsh/completions/_sedx"

        # Add to .zshrc if not already present
        ZSHRC="$HOME/.zshrc"
        FPATH_LINE='fpath=("$HOME/.zsh/completions" $fpath)'

        if ! grep -q '$HOME/.zsh/completions' "$ZSHRC" 2>/dev/null; then
            echo "" >> "$ZSHRC"
            echo "# SedX completion" >> "$ZSHRC"
            echo "$FPATH_LINE" >> "$ZSHRC"
            echo "autoload -U compinit && compinit" >> "$ZSHRC"
            print_info "Added fpath line to $ZSHRC"
        fi

        print_info "Installed to ~/.zsh/completions/_sedx"
        print_warn "Run 'exec zsh' or restart your shell to use completions"
    fi
}

install_fish() {
    print_info "Installing Fish completion..."

    if ! command -v fish &> /dev/null; then
        print_error "Fish is not installed"
        return 1
    fi

    # Fish automatically loads completions from ~/.config/fish/completions/
    mkdir -p "$HOME/.config/fish/completions"
    cp "$COMPLETION_DIR/fish.sedx" "$HOME/.config/fish/completions/sedx.fish"

    print_info "Installed to ~/.config/fish/completions/sedx.fish"
    print_warn "Restart Fish shell to use completions"
}

install_powershell() {
    print_info "Installing PowerShell completion..."

    if ! command -v pwsh &> /dev/null && ! command -v powershell &> /dev/null; then
        print_error "PowerShell is not installed"
        return 1
    fi

    # Detect PowerShell
    PWSH="pwsh"
    if ! command -v pwsh &> /dev/null; then
        PWSH="powershell"
    fi

    # Get PowerShell profile path
    PROFILE_DIR=$($PWSH -NoProfile -Command 'Split-Path -Parent $PROFILE')
    PROFILE_FILE=$($PWSH -NoProfile -Command '$PROFILE')

    mkdir -p "$PROFILE_DIR"
    cp "$COMPLETION_DIR/sedx.ps1" "$PROFILE_DIR/SedxCompletion.ps1"

    # Add import to profile if not already present
    IMPORT_LINE="Import-Module `$PROFILE_DIR/SedxCompletion.ps1"

    if ! $PWSH -NoProfile -Command "Select-String -Path '$PROFILE_FILE' -Pattern 'SedxCompletion' -Quiet" 2>/dev/null; then
        $PWSH -NoProfile -Command "Add-Content -Path '$PROFILE_FILE' -Value \"`n# SedX completion`nImport-Module `$PROFILE_DIR/SedxCompletion.ps1\""
        print_info "Added import to PowerShell profile"
    fi

    print_info "Installed to $PROFILE_DIR/SedxCompletion.ps1"
    print_warn "Restart PowerShell to use completions"
}

# Main installation logic
main() {
    local shell="${1:-auto}"

    if [[ "$shell" == "auto" ]]; then
        # Detect current shell
        if [[ -n "$BASH_VERSION" ]]; then
            shell="bash"
        elif [[ -n "$ZSH_VERSION" ]]; then
            shell="zsh"
        elif [[ -n "$FISH_VERSION" ]]; then
            shell="fish"
        else
            print_error "Could not detect shell. Please specify: bash, zsh, fish, powershell, or all"
            exit 1
        fi
        print_info "Detected shell: $shell"
    fi

    case "$shell" in
        bash)
            install_bash
            ;;
        zsh)
            install_zsh
            ;;
        fish)
            install_fish
            ;;
        powershell|pwsh)
            install_powershell
            ;;
        all)
            install_bash
            install_zsh
            install_fish
            install_powershell
            ;;
        *)
            print_error "Unknown shell: $shell"
            echo "Usage: $0 [bash|zsh|fish|powershell|all]"
            exit 1
            ;;
    esac

    print_info "Installation complete!"
}

# Run main function with all arguments
main "$@"
