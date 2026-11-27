use crate::cli::Command;

pub fn help_text(cmd: Command) -> &'static str {
    match cmd {
        Command::List => {
            "Usage: aibox [--project <dir>] list [--json]\n\n\
             List Incus environments managed by AIBox in JSON format.\n"
        }
        Command::Check => {
            "Usage: aibox [--project <dir>] check [--fix] [--json]\n\n\
             Run host diagnostics for Incus, bridge, KVM, GPU, and Btrfs snapshots.\n"
        }
        Command::Enter => {
            "Usage: aibox [--project <dir>] enter <env>\n\n\
             Run pre-steps (auto-snapshot, allowlist refresh, start if stopped) and attach an interactive shell in /home/workspace.\n"
        }
        Command::Exec => {
            "Usage: aibox [--project <dir>] exec [--timeout <dur>] [--summary] <env> -- <cmd...>\n\n\
             Run pre-steps and execute a non-interactive command in /home/workspace, returning its exit code.\n\n\
             Options:\n\
             \x20 --timeout <dur>   Terminate the command after <dur> (e.g. 30s, 15m, 1h30m; max 24h)\n\
             \x20 --summary         Print package, service, and port changes after the command\n"
        }
        Command::Start => {
            "Usage: aibox [--project <dir>] start <env>\n\n\
             Start <env> with AIBox auto-snapshot and network allowlist refresh rules.\n"
        }
        Command::Stop => {
            "Usage: aibox [--project <dir>] stop <env>\n\n\
             Stop <env>.\n"
        }
        Command::Snapshot => {
            "Usage: aibox [--project <dir>] snapshot <env> [name]\n\n\
             Create a paired OS container and /home/workspace Btrfs snapshot.\n"
        }
        Command::Snapshots => {
            "Usage: aibox [--project <dir>] snapshots <env> [--json]\n\n\
             List snapshots with OS/workspace availability and sizes.\n"
        }
        Command::Restore => {
            "Usage: aibox [--project <dir>] restore <env> <snap> [--system-only|--workspace-only] [--yes]\n\n\
             Restore an environment snapshot (with pre-restore workspace safety snapshot).\n"
        }
        Command::Diff => {
            "Usage: aibox [--project <dir>] diff <env> [snap] [--json]\n\n\
             Show /home/workspace file changes compared against a snapshot (default: latest).\n"
        }
        Command::ExportConfig => {
            "Usage: aibox [--project <dir>] export-config <env> [--output <path>]\n\n\
             Export an environment's settings as a declarative aibox.toml manifest.\n"
        }
        Command::Up => {
            "Usage: aibox [--project <dir>] up [path]\n\n\
             Create or reconcile an environment from a declarative aibox.toml manifest (default: ./aibox.toml).\n"
        }
        Command::Healthcheck => {
            "Usage: aibox [--project <dir>] healthcheck <env> [--url <url|port> | --set <url|port> | --off] [--auto-restart] [--json]\n\n\
             Probe an HTTP(S) URL or TCP port for <env>, notify on failure, and optionally restart.\n"
        }
        Command::BackupSchedule => {
            "Usage: aibox [--project <dir>] backup-schedule <env> <daily|weekly|off> [--keep <n>]\n\n\
             Install or remove a systemd user timer for scheduled snapshots with retention.\n"
        }
        Command::Run => {
            "Usage: aibox [--project <dir>] run --ephemeral --from <template> [--keep-on-fail] -- <cmd...>\n\n\
             Clone a temporary environment from <template>, run <cmd...>, and delete it afterward.\n"
        }
        Command::SshConfig => {
            "Usage: aibox [--project <dir>] ssh-config [env]\n\n\
             Print SSH config stanzas (Host aibox-<env>) using incus exec ProxyCommand.\n"
        }
        Command::Svc => {
            "Usage: aibox [--project <dir>] svc <env> <list|logs|start|stop|restart|enable|disable> [unit] [--lines <n>] [--json]\n\n\
             Inspect and manage systemd/OpenRC services inside a running environment.\n"
        }
        Command::Theme => {
            "Usage: aibox theme dump [path] [--json]\n\
             \x20      aibox theme render-icon [--output PATH] [--theme-dir DIR]\n\n\
             Inspect derived semantic theme tokens or render the adaptive desktop icon.\n"
        }
        Command::Completions => {
            "Usage: aibox completions <bash|zsh|fish>\n\n\
             Generate shell completion script for bash, zsh, or fish.\n"
        }
        Command::Help | Command::None => {
            "Usage: aibox [--project <dir>] <command> [options]\n\n\
             Commands:\n\
             \x20 list [--json]                                       List environments\n\
             \x20 check [--fix] [--json]                              Check/repair host setup\n\
             \x20 enter <env>                                         Open interactive shell in /home/workspace\n\
             \x20 exec [--timeout <dur>] [--summary] <env> -- <cmd...> Run command in /home/workspace\n\
             \x20 start <env>                                         Start environment\n\
             \x20 stop <env>                                          Stop environment\n\
             \x20 snapshot <env> [name]                               Create paired OS + /home/workspace snapshot\n\
             \x20 snapshots <env> [--json]                            List snapshots and sizes\n\
             \x20 restore <env> <snap> [--system-only|--workspace-only] [--yes]\n\
             \x20                                                     Restore snapshot\n\
             \x20 diff <env> [snap] [--json]                          Review /home/workspace changes\n\
             \x20 run --ephemeral --from <tpl> [--keep-on-fail] -- <cmd...>\n\
             \x20                                                     Run command in disposable clone\n\
             \x20 ssh-config [env]                                    Print SSH config stanzas\n\
             \x20 svc <env> <action> [unit] [--lines <n>] [--json]    Manage container services\n\
             \x20 export-config <env> [--output <path>]               Export aibox.toml manifest\n\
             \x20 up [path]                                           Provision/reconcile from aibox.toml\n\
             \x20 healthcheck <env> [--url <t>|--set <t>|--off]       Probe service health & auto-restart\n\
             \x20 backup-schedule <env> <daily|weekly|off> [--keep n] Schedule systemd user backups\n\
             \x20 completions <bash|zsh|fish>                         Generate shell completions\n\
             \x20 help [command]                                      Show help\n\n\
             Global options:\n\
             \x20 --project <dir>                                     Select project directory containing aibox/ (default: ~/aibox)\n\
             \x20 -h, --help                                          Show help\n\n\
             Exit codes:\n\
             \x20 0  OK (or <cmd> exit code for exec)\n\
             \x20 1  Operation failed\n\
             \x20 2  Usage error\n\
             \x20 3  Another AIBox background task is running\n\
             \x20 4  Environment not found or is a template\n"
        }
    }
}
