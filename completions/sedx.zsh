#compdef sedx

# Zsh completion for SedX

_sedx() {
    local -a commands subcommands options

    commands=(
        'rollback:Rollback a previous operation'
        'history:Display operation history'
        'status:Show backup status'
        'backup:Manage backups'
        'config:Edit or show configuration'
    )

    subcommands=(
        'list:List all backups'
        'show:Show backup details'
        'restore:Restore from backup'
        'remove:Remove a backup'
        'prune:Remove old backups'
    )

    options=(
        '(--help)-h[Show help]'
        '(--help)--help[Show help]'
        '(--version)-V[Print version]'
        '(--version)--version[Print version]'
        '(--dry-run -d)'{--dry-run,-d}'[Preview changes]'
        '(--interactive -i)'{--interactive,-i}'[Ask for confirmation]'
        '(--quiet -n --silent)'{--quiet,-n,--silent}'[Suppress automatic output]'
        '--context=[Context lines]:number:(0 1 2 3 4 5 6 7 8 9 10)'
        '(--no-context -nc)'{--no-context,-nc}'[Show only changed lines]'
        '(--ere -E)'{--ere,-E}'[Use ERE regex]'
        '(--bre -B)'{--bre,-B}'[Use BRE regex]'
        '(--no-backup --force)'{--no-backup,--force}'[Skip backup]'
        '--backup-dir=[Custom backup dir]:dir:_directories'
        '--streaming[Enable streaming]'
        '--no-streaming[Disable streaming]'
        '(--expression -e)'{--expression,-e}'[Add expression]:expr'
        '(--file -f)'{--file,-f}'[Read script from file]:file:_files'
    )

    case $words[2] in
        rollback)
            _arguments \
                '1:backup id:->backup_ids' \
                && _backup_ids
            ;;
        backup)
            _alternative \
                'subcommands::subcommands:($subcommands)' \
                'files::file:_files'
            ;;
        *)
            _arguments -C \
                '*:: :->subcmds' \
                && _describe -t commands 'sedx commands' commands \
                && _arguments -s $options
            ;;
    esac
}

_backup_ids() {
    local -a backup_ids
    backup_ids=(${(f)"$(sedx backup list 2>/dev/null | grep -oE '[0-9]{8}-[0-9]{6}-[a-z0-9]+' | head -20)"})
    _describe 'backup ids' backup_ids
}

_sedx "$@"
