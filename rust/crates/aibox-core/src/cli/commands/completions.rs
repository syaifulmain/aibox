use crate::cli::{CliIo, ExitCode, ParsedArgs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
}

impl ShellKind {
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "fish" => Some(Self::Fish),
            _ => None,
        }
    }
}

pub fn generate_completions(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => BASH_COMPLETIONS,
        ShellKind::Zsh => ZSH_COMPLETIONS,
        ShellKind::Fish => FISH_COMPLETIONS,
    }
}

pub fn run_completions(parsed: &ParsedArgs, io: &mut dyn CliIo) -> i32 {
    let Some(shell) = ShellKind::parse(&parsed.shell) else {
        io.write_stderr(b"Error: Unsupported shell. Expected bash, zsh, or fish.\n");
        return ExitCode::Usage.as_i32();
    };
    io.write_stdout(generate_completions(shell).as_bytes());
    ExitCode::Ok.as_i32()
}

const BASH_COMPLETIONS: &str = r#"_aibox_completions() {
    local cur prev words cword
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    local commands="list check enter exec start stop snapshot snapshots restore diff ssh-config svc theme completions help"
    local global_opts="--project -h --help"

    if [[ "$prev" == "--project" || "$prev" == "--output" || "$prev" == "--theme-dir" ]]; then
        COMPREPLY=( $(compgen -f -- "$cur") )
        return 0
    fi

    local cmd=""
    local i
    for ((i=1; i < COMP_CWORD; i++)); do
        case "${COMP_WORDS[i]}" in
            --project|--output|--theme-dir)
                ((i++))
                ;;
            -*)
                ;;
            *)
                cmd="${COMP_WORDS[i]}"
                break
                ;;
        esac
    done

    if [[ -z "$cmd" ]]; then
        COMPREPLY=( $(compgen -W "$commands $global_opts" -- "$cur") )
        return 0
    fi

    case "$cmd" in
        list)
            COMPREPLY=( $(compgen -W "--json -h --help" -- "$cur") )
            ;;
        check)
            COMPREPLY=( $(compgen -W "--fix --json -h --help" -- "$cur") )
            ;;
        exec)
            COMPREPLY=( $(compgen -W "--timeout -h --help" -- "$cur") )
            ;;
        snapshots|diff)
            COMPREPLY=( $(compgen -W "--json -h --help" -- "$cur") )
            ;;
        restore)
            COMPREPLY=( $(compgen -W "--system-only --workspace-only --yes -y -h --help" -- "$cur") )
            ;;
        theme)
            COMPREPLY=( $(compgen -W "dump render-icon --json --output --theme-dir -h --help" -- "$cur") )
            ;;
        completions)
            COMPREPLY=( $(compgen -W "bash zsh fish -h --help" -- "$cur") )
            ;;
        help)
            COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
            ;;
        *)
            COMPREPLY=( $(compgen -W "-h --help" -- "$cur") )
            ;;
    esac
}
complete -F _aibox_completions aibox
"#;

const ZSH_COMPLETIONS: &str = r#"#compdef aibox

_aibox() {
    local -a commands
    commands=(
        'list:List environments'
        'check:Check/repair host setup'
        'enter:Open interactive shell in /home/workspace'
        'exec:Run command in /home/workspace'
        'start:Start environment'
        'stop:Stop environment'
        'snapshot:Create paired OS + /home/workspace snapshot'
        'snapshots:List snapshots and sizes'
        'restore:Restore snapshot'
        'diff:Review /home/workspace changes'
        'ssh-config:Print SSH config stanzas'
        'svc:Manage container services'
        'theme:Inspect theme tokens or render icon'
        'completions:Generate shell completions'
        'help:Show help'
    )

    _arguments -C \
        '--project[Select project directory]:dir:_files -/' \
        '(-h --help)'{-h,--help}'[Show help]' \
        '1:command:->cmd' \
        '*::arg:->args'

    case "$state" in
        cmd)
            _describe 'command' commands
            ;;
        args)
            case "${line[1]}" in
                list)
                    _arguments '--json[JSON output]' '(-h --help)'{-h,--help}'[Show help]'
                    ;;
                check)
                    _arguments '--fix[Attempt automatic fixes]' '--json[JSON output]' '(-h --help)'{-h,--help}'[Show help]'
                    ;;
                exec)
                    _arguments '--timeout[Terminate command after duration]:duration:' '(-h --help)'{-h,--help}'[Show help]'
                    ;;
                snapshots|diff)
                    _arguments '--json[JSON output]' '(-h --help)'{-h,--help}'[Show help]'
                    ;;
                restore)
                    _arguments '--system-only[Restore OS only]' '--workspace-only[Restore workspace only]' '(-y --yes)'{-y,--yes}'[Skip confirmation]' '(-h --help)'{-h,--help}'[Show help]'
                    ;;
                theme)
                    _arguments '1:subcommand:(dump render-icon)' '--json[JSON output]' '--output[Output SVG path]:file:_files' '--theme-dir[Theme directory]:dir:_files -/' '(-h --help)'{-h,--help}'[Show help]'
                    ;;
                completions)
                    _arguments '1:shell:(bash zsh fish)' '(-h --help)'{-h,--help}'[Show help]'
                    ;;
                help)
                    _describe 'command' commands
                    ;;
            esac
            ;;
    esac
}

_aibox "$@"
"#;

const FISH_COMPLETIONS: &str = r#"complete -c aibox -f

set -l __aibox_cmds list check enter exec start stop snapshot snapshots restore diff ssh-config svc theme completions help

complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -l project -r -d "Select project directory"
complete -c aibox -s h -l help -d "Show help"

complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a list -d "List environments"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a check -d "Check/repair host setup"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a enter -d "Open interactive shell in /home/workspace"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a exec -d "Run command in /home/workspace"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a start -d "Start environment"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a stop -d "Stop environment"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a snapshot -d "Create paired OS + /home/workspace snapshot"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a snapshots -d "List snapshots and sizes"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a restore -d "Restore snapshot"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a diff -d "Review /home/workspace changes"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a ssh-config -d "Print SSH config stanzas"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a svc -d "Manage container services"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a theme -d "Inspect theme tokens or render icon"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a completions -d "Generate shell completions"
complete -c aibox -n "not __fish_seen_subcommand_from $__aibox_cmds" -a help -d "Show help"

complete -c aibox -n "__fish_seen_subcommand_from list check snapshots diff" -l json -d "JSON output"
complete -c aibox -n "__fish_seen_subcommand_from check" -l fix -d "Attempt automatic fixes"
complete -c aibox -n "__fish_seen_subcommand_from exec" -l timeout -r -d "Terminate command after duration"
complete -c aibox -n "__fish_seen_subcommand_from restore" -l system-only -d "Restore OS only"
complete -c aibox -n "__fish_seen_subcommand_from restore" -l workspace-only -d "Restore workspace only"
complete -c aibox -n "__fish_seen_subcommand_from restore" -s y -l yes -d "Skip confirmation"
complete -c aibox -n "__fish_seen_subcommand_from theme" -a "dump render-icon"
complete -c aibox -n "__fish_seen_subcommand_from theme" -l json -d "JSON output"
complete -c aibox -n "__fish_seen_subcommand_from theme" -l output -r -d "Output SVG path"
complete -c aibox -n "__fish_seen_subcommand_from theme" -l theme-dir -r -d "Theme directory"
complete -c aibox -n "__fish_seen_subcommand_from completions" -a "bash zsh fish" -d "Target shell"
complete -c aibox -n "__fish_seen_subcommand_from help" -a "$__aibox_cmds"
"#;
