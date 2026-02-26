# SedX Shell Completion Scripts

This directory contains shell completion scripts for SedX, providing tab completion for commands, options, and backup IDs.

## Supported Shells

- **Bash** - `bash.sedx`
- **Zsh** - `zsh.sedx`
- **Fish** - `fish.sedx`
- **PowerShell** - `sedx.ps1`

## Installation

### Bash

#### System-wide installation (requires root)
```bash
sudo cp completions/bash.sedx /usr/share/bash-completion/completions/sedx
```

#### User installation
```bash
# Create directory if it doesn't exist
mkdir -p ~/.local/share/bash-completion/completions

# Copy completion script
cp completions/bash.sedx ~/.local/share/bash-completion/completions/sedx

# Source it in your ~/.bashrc (add this line)
# source ~/.local/share/bash-completion/completions/sedx
```

#### Temporary testing
```bash
source completions/bash.sedx
```

### Zsh

#### System-wide installation (requires root)
```bash
sudo cp completions/zsh.sedx /usr/share/zsh/vendor-completions/_sedx
sudo chmod +x /usr/share/zsh/vendor-completions/_sedx
```

#### User installation
```bash
# Create directory if it doesn't exist
mkdir -p ~/.zsh/completions

# Copy completion script
cp completions/zsh.sedx ~/.zsh/completions/_sedx

# Add to ~/.zshrc (if not already present)
# fpath=(~/.zsh/completions $fpath)
# autoload -U compinit && compinit
```

#### Temporary testing
```bash
source completions/zsh.sedx
```

### Fish

#### Installation
```bash
# Fish completion directory should already exist
mkdir -p ~/.config/fish/completions

# Copy completion script
cp completions/fish.sedx ~/.config/fish/completions/sedx.fish
```

Fish automatically loads completions from `~/.config/fish/completions/` on startup.

### PowerShell

#### Current user
```powershell
# Create directory if it doesn't exist
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\Documents\PowerShell"

# Copy completion script
Copy-Item completions\sedx.ps1 "$env:USERPROFILE\Documents\PowerShell\SedxCompletion.ps1"

# Add to PowerShell profile (run once)
Add-Content -Path $PROFILE -Value "`nImport-Module `$env:USERPROFILE\Documents\PowerShell\SedxCompletion.ps1"
```

#### All users (requires admin)
```powershell
# Copy to system modules directory
Copy-Item completions\sedx.ps1 "C:\Program Files\PowerShell\Modules\SedxCompletion\SedxCompletion.psm1"

# Import in system profile
Add-Content -Path $PROFILE.AllUsersAllHosts -Value "`nImport-Module SedxCompletion"
```

## Features

All completion scripts support:

### Command completion
- `rollback` - Rollback a previous operation
- `history` - Show operation history
- `status` - Show current backup status
- `backup` - Manage backups
- `config` - Edit configuration file

### Option completion
- `-d, --dry-run` - Preview changes
- `-e, --expression` - Add sed expression
- `-f, --file` - Read script from file
- `-i, --interactive` - Interactive mode
- `--context` - Context lines (0-10)
- `-n, --quiet, --silent` - Quiet mode
- `--no-context` - No context output
- `--streaming, --no-streaming` - Streaming mode
- `-E, --ere` - Extended regex
- `-B, --bre` - Basic regex
- `--no-backup, --force` - Skip backup
- `--backup-dir` - Custom backup directory

### Backup subcommands
- `backup list` - List all backups
- `backup show <id>` - Show backup details
- `backup restore <id>` - Restore backup
- `backup remove <id>` - Remove backup
- `backup prune` - Prune old backups

### Smart completions
- **Backup IDs** - Completes backup IDs from `~/.sedx/backups/`
- **Files** - Completes file paths for `-f` and `--file`
- **Directories** - Completes directories for `--backup-dir`
- **Numbers** - Suggests appropriate numbers for `--context`, `--keep`, etc.

## Testing

After installation, test the completions:

```bash
# Type and press Tab
sedx <Tab>           # Show commands
sedx --<Tab>         # Show options
sedx rollback <Tab>  # Show backup IDs
sedx backup <Tab>    # Show backup subcommands
```

## Troubleshooting

### Bash: Completions not working
1. Ensure `bash-completion` is installed
2. Check the script is sourced in your `.bashrc`
3. Run `source ~/.bashrc` to reload

### Zsh: Completions not working
1. Ensure `compinit` is called in your `.zshrc`
2. Check the fpath includes your completions directory
3. Run `rm -f ~/.zcompdump* && exec zsh` to rebuild completion cache

### Fish: Completions not working
1. Ensure the file is named `sedx.fish` (not `fish.sedx`)
2. Run `fish_update_completions` to regenerate completions
3. Restart Fish shell

### PowerShell: Completions not working
1. Check that your profile path is correct: `$PROFILE`
2. Ensure the module is being imported: `Get-Module -ListAvailable`
3. Run `. $PROFILE` to reload your profile

## Customization

### Custom backup directory

Set the `SEDX_BACKUP_DIR` environment variable to use a custom backup location:

```bash
# Bash/Zsh
export SEDX_BACKUP_DIR="/custom/backup/path"

# Fish
set -x SEDX_BACKUP_DIR "/custom/backup/path"

# PowerShell
$env:SEDX_BACKUP_DIR = "C:\custom\backup\path"
```

The completion scripts will use this directory when suggesting backup IDs.
