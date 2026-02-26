# PowerShell completion for SedX

using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'sedx' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commands = @('rollback', 'history', 'status', 'backup', 'config')
    $backupCommands = @('list', 'show', 'restore', 'remove', 'prune')
    $options = @(
        '--help', '-h',
        '--version', '-V',
        '--dry-run', '-d',
        '--interactive', '-i',
        '--quiet', '-n', '--silent',
        '--context',
        '--no-context', '-nc',
        '--ere', '-E',
        '--bre', '-B',
        '--no-backup', '--force',
        '--backup-dir',
        '--streaming',
        '--no-streaming',
        '--expression', '-e',
        '--file', '-f'
    )

    # Parse command line
    $commandElements = $commandAst.CommandElements
    $commandCmd = if ($commandElements.Count -gt 1) { $commandElements[1].Extent.Text } else { $null }

    if ($commandCmd -eq 'backup') {
        # Complete backup subcommands
        $backupCommands | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
            [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, "Backup subcommand: $_")
        }
    } elseif ($commandCmd -eq 'rollback') {
        # Complete backup IDs
        $backupIds = sedx backup list 2>$null | Select-String -Pattern '[0-9]{8}-[0-9]{6}-[a-z0-9]+' | ForEach-Object { $_.Matches.Value }
        $backupIds | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
            [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, "Backup ID: $_")
        }
    } else {
        # Complete main commands and options
        @($commands + $options) | Where-Object { $_ -like "$wordToComplete*" } | ForEach-Object {
            [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
        }
    }
}
