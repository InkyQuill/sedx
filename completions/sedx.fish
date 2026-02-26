# Fish completion for SedX

function __fish_sedx_backup_ids
    sedx backup list 2>/dev/null | grep -oE '[0-9]{8}-[0-9]{6}-[a-z0-9]+' | head -20
end

function __fish_sedx_using_command
    set -l cmd (commandline -opc)
    if test (count $cmd) -gt 1
        echo $cmd[2]
    end
end

complete -c sedx -f

# General options
complete -c sedx -l help -s h -d "Show help"
complete -c sedx -l version -s V -d "Print version"
complete -c sedx -l dry-run -s d -d "Preview changes"
complete -c sedx -l interactive -s i -d "Ask for confirmation"
complete -c sedx -l quiet -s n -l silent -d "Suppress automatic output"
complete -c sedx -l context -d "Number of context lines" -x -a "{0..10}"
complete -c sedx -l no-context -s nc -d "Show only changed lines"
complete -c sedx -l ere -s E -d "Use ERE regex"
complete -c sedx -l bre -s B -d "Use BRE regex"
complete -c sedx -l no-backup -l force -d "Skip backup"
complete -c sedx -l backup-dir -d "Custom backup dir" -r
complete -c sedx -l streaming -d "Enable streaming"
complete -c sedx -l no-streaming -d "Disable streaming"
complete -c sedx -l expression -s e -d "Add expression"
complete -c sedx -l file -s f -d "Read script from file" -r

# Subcommands
complete -c sedx -n __fish_use_subcommand -xa rollback -d "Rollback a previous operation"
complete -c sedx -n __fish_use_subcommand -xa history -d "Display operation history"
complete -c sedx -n __fish_use_subcommand -xa status -d "Show backup status"
complete -c sedx -n __fish_use_subcommand -xa backup -d "Manage backups"
complete -c sedx -n __fish_use_subcommand -xa config -d "Edit configuration"

# backup subcommands
complete -c sedx -n "__fish_sedx_using_command backup" -xa list -d "List backups"
complete -c sedx -n "__fish_sedx_using_command backup" -xa show -d "Show backup details"
complete -c sedx -n "__fish_sedx_using_command backup" -xa restore -d "Restore from backup"
complete -c sedx -n "__fish_sedx_using_command backup" -xa remove -d "Remove a backup"
complete -c sedx -n "__fish_sedx_using_command backup" -xa prune -d "Remove old backups"

# rollback subcommand - suggest backup IDs
complete -c sedx -n "__fish_sedx_using_command rollback" -a "(__fish_sedx_backup_ids)"
