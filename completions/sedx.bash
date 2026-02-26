# Bash completion for SedX

_sedx_completions() {
    local cur prev words cword
    _init_completion || return

    case "${prev}" in
        --help|-h|--version|-V)
            return 0
            ;;
        --context)
            COMPREPLY=($(compgen -W "{0..10}" -- "${cur}"))
            return 0
            ;;
        --backup-dir)
            COMPREPLY=($(compgen -d -- "${cur}"))
            return 0
            ;;
        --expression|-e)
            # Suggest common sed expressions
            COMPREPLY=($(compgen -W "s/d/p/i/a/c/q/n/r/w" -- "${cur}"))
            return 0
            ;;
        --file|-f)
            COMPREPLY=($(compgen -f -- "${cur}"))
            return 0
            ;;
        rollback)
            # Suggest backup IDs
            local backups=($(sedx backup list 2>/dev/null | grep -oE '[0-9]{8}-[0-9]{6}-[a-z0-9]+' | head -20))
            COMPREPLY=($(compgen -W "${backups[*]}" -- "${cur}"))
            return 0
            ;;
        backup)
            COMPREPLY=($(compgen -W "list show restore remove prune" -- "${cur}"))
            return 0
            ;;
        history|status|config|--dry-run|-d|--interactive|-i|--quiet|-n|--silent|--ere|-E|--bre|-B|--no-backup|--force|--streaming|--no-streaming)
            return 0
            ;;
        *)
            ;;
    esac

    # Main options
    if [[ "${cur}" == -* ]]; then
        COMPREPLY=($(compgen -W "
            --help -h
            --version -V
            --dry-run -d
            --interactive -i
            --quiet -n --silent
            --context
            --no-context -nc
            --ere -E
            --bre -B
            --no-backup --force
            --backup-dir
            --streaming
            --no-streaming
            --expression -e
            --file -f
        " -- "${cur}"))
    else
        # Subcommands
        COMPREPLY=($(compgen -W "rollback history status backup config" -- "${cur}"))
    fi
}

complete -F _sedx_completions sedx
