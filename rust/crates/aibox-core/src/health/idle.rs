//! Idle detection, override parsing, and auto-stop decision helpers.

use crate::incus::list::{parse as parse_incus_list, CounterState, Environment, ParseDefaults};
use crate::util::shell::shell_quote;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdleStopMode {
    #[default]
    Inherit,
    Off,
    Minutes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IdleStopOverride {
    pub mode: IdleStopMode,
    pub minutes: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IdleCpuSample {
    pub timestamp_ms: i64,
    pub cpu_percent: i32,
}

/// Per-instance CPU and network activity extracted from a single `incus list --format=json` payload.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdleInstanceStats {
    pub name: String,
    pub running: bool,
    pub cpu_percent: i32,
    pub net_rx_bytes: i64,
    pub net_tx_bytes: i64,
    pub net_rx_rate: i64,
    pub net_tx_rate: i64,
}

/// Projects already-parsed `Environment` entries into [`IdleInstanceStats`] without extra queries.
pub fn extract_idle_stats_from_envs(envs: &[Environment]) -> Vec<IdleInstanceStats> {
    envs.iter()
        .map(|e| IdleInstanceStats {
            name: e.name.clone(),
            running: e.running,
            cpu_percent: e.cpu_percent,
            net_rx_bytes: e.net_rx_bytes,
            net_tx_bytes: e.net_tx_bytes,
            net_rx_rate: e.net_rx_rate,
            net_tx_rate: e.net_tx_rate,
        })
        .collect()
}

/// Extracts CPU and network activity for all instances from one `incus list --format=json` payload.
pub fn extract_idle_stats_from_incus_list(
    list_json: &[u8],
    counter_state: &mut CounterState,
    elapsed_wall_ns: i64,
) -> Vec<IdleInstanceStats> {
    let envs = parse_incus_list(
        list_json,
        "",
        counter_state,
        elapsed_wall_ns,
        ParseDefaults::default(),
        true,
    );
    extract_idle_stats_from_envs(&envs)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdleDecisionInput {
    pub running: bool,
    pub is_template: bool,
    pub busy_on_this_env: bool,
    pub has_active_exec_session: bool,
    pub port_forward_count: i32,
    pub ignore_port_forwards: bool,
    pub threshold_minutes: i32,
    pub now_ms: i64,
    pub warning_already_sent: bool,
    pub samples: Vec<IdleCpuSample>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IdleDecisionResult {
    pub eligible: bool,
    pub idle_duration_ms: i64,
    pub average_cpu_percent: f64,
    pub should_warn_one_min: bool,
    pub should_stop_now: bool,
    pub blocked_reason: String,
}

/// Parses a per-environment `user.aibox.idlestop` override (`""`, `"inherit"`, `"off"`, or `1..=1440`).
pub fn parse_idle_stop_override(raw: &str) -> IdleStopOverride {
    let norm = raw.trim().to_lowercase();
    if norm.is_empty() || norm == "inherit" || norm == "default" {
        return IdleStopOverride {
            mode: IdleStopMode::Inherit,
            minutes: 0,
        };
    }
    if norm == "off" || norm == "false" || norm == "0" {
        return IdleStopOverride {
            mode: IdleStopMode::Off,
            minutes: 0,
        };
    }
    if let Ok(mins) = norm.parse::<i32>() {
        if (1..=1440).contains(&mins) {
            return IdleStopOverride {
                mode: IdleStopMode::Minutes,
                minutes: mins,
            };
        }
    }
    IdleStopOverride {
        mode: IdleStopMode::Inherit,
        minutes: 0,
    }
}

/// Resolves effective idle-stop minutes from the global setting and an environment override.
pub fn effective_idle_stop_minutes(global_minutes: i32, ov: &IdleStopOverride) -> i32 {
    match ov.mode {
        IdleStopMode::Off => 0,
        IdleStopMode::Minutes => ov.minutes.clamp(1, 1440),
        IdleStopMode::Inherit => {
            if global_minutes > 0 {
                global_minutes.clamp(1, 1440)
            } else {
                0
            }
        }
    }
}

/// Evaluates whether a running environment is idle long enough to warn or stop.
pub fn evaluate_idle_decision(input: &IdleDecisionInput) -> IdleDecisionResult {
    let mut res = IdleDecisionResult::default();
    if !input.running {
        res.blocked_reason = "stopped".to_string();
        return res;
    }
    if input.is_template {
        res.blocked_reason = "template".to_string();
        return res;
    }
    if input.threshold_minutes <= 0 {
        res.blocked_reason = "disabled".to_string();
        return res;
    }
    if input.busy_on_this_env {
        res.blocked_reason = "busy".to_string();
        return res;
    }
    if input.has_active_exec_session {
        res.blocked_reason = "exec-session".to_string();
        return res;
    }
    if input.port_forward_count > 0 && !input.ignore_port_forwards {
        res.blocked_reason = "port-forwards".to_string();
        return res;
    }

    res.eligible = true;
    if input.samples.is_empty() {
        return res;
    }

    // CPU spikes (>= 3%) reset the idle timer; inspect contiguous calm samples since last spike.
    let mut start_idx = 0;
    for (i, sample) in input.samples.iter().enumerate().rev() {
        if sample.cpu_percent >= 3 {
            start_idx = i + 1;
            break;
        }
    }
    if start_idx >= input.samples.len() {
        res.blocked_reason = "cpu-spike".to_string();
        return res;
    }

    let calm_slice = &input.samples[start_idx..];
    let sum_cpu: f64 = calm_slice
        .iter()
        .map(|s| f64::from(s.cpu_percent.max(0)))
        .sum();
    res.average_cpu_percent = sum_cpu / (calm_slice.len() as f64);

    let first_calm_ms = calm_slice[0].timestamp_ms;
    let ref_now_ms = if input.now_ms > 0 {
        input.now_ms
    } else {
        input.samples.last().map_or(0, |s| s.timestamp_ms)
    };
    res.idle_duration_ms = (ref_now_ms - first_calm_ms).max(0);

    if res.average_cpu_percent >= 3.0 {
        res.idle_duration_ms = 0;
        res.blocked_reason = "cpu-high".to_string();
        return res;
    }

    let window_ms = i64::from(input.threshold_minutes) * 60 * 1000;
    let warn_lead_ms = if input.threshold_minutes >= 2 {
        60_000
    } else {
        window_ms / 2
    };
    if res.idle_duration_ms >= window_ms {
        res.should_stop_now = true;
    } else if !input.warning_already_sent && res.idle_duration_ms >= (window_ms - warn_lead_ms) {
        res.should_warn_one_min = true;
    }
    res
}

/// Generates the shell script for `setEnvironmentAutoStop`.
pub fn set_environment_auto_stop_script(name: &str, enabled: bool) -> String {
    let q_name = shell_quote(name);
    let val = if enabled { "true" } else { "false" };
    format!(
        "echo '@@STAGE:50:Updating watchdog auto-stop policy...'; \
         incus config set {q_name} user.aibox.autostop {val}; \
         echo '@@STAGE:100:Updated.'"
    )
}

/// Generates the shell script for `setEnvironmentIdleStop`.
pub fn set_environment_idle_stop_script(
    name: &str,
    value: &str,
    allow_with_port_forwards: bool,
) -> String {
    let parsed = parse_idle_stop_override(value);
    let stored = match parsed.mode {
        IdleStopMode::Off => "off".to_string(),
        IdleStopMode::Minutes => parsed.minutes.to_string(),
        IdleStopMode::Inherit => "inherit".to_string(),
    };
    let q_name = shell_quote(name);
    let q_stored = shell_quote(&stored);
    let ports_flag = if allow_with_port_forwards {
        "true"
    } else {
        "false"
    };
    format!(
        "echo '@@STAGE:50:Updating idle auto-stop policy...'; \
         incus config set {q_name} user.aibox.idlestop {q_stored} user.aibox.idlestop_ports {ports_flag}; \
         echo '@@STAGE:100:Updated.'"
    )
}

const VALUE_FLAGS: &[&str] = &[
    "--cwd",
    "--env",
    "--user",
    "--group",
    "--mode",
    "--project",
    "--target",
    "-e",
    "-u",
    "-g",
];

/// Parses `pgrep -a` output to find environment names with an active `incus exec` session.
pub fn parse_active_incus_exec_envs(pgrep_output: &str) -> BTreeSet<String> {
    let mut envs = BTreeSet::new();
    for raw_line in pgrep_output.split('\n') {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(exec_idx) = line.find("incus exec ") else {
            continue;
        };
        let tail = line[exec_idx + 11..].trim();
        let tokens: Vec<&str> = tail.split_whitespace().collect();
        let mut i = 0;
        while i < tokens.len() {
            let tok = tokens[i];
            if tok == "--" {
                break;
            }
            if tok.starts_with('-') {
                if !tok.contains('=') && VALUE_FLAGS.contains(&tok) {
                    i += 1;
                }
                i += 1;
                continue;
            }
            envs.insert(tok.to_string());
            break;
        }
    }
    envs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_active_incus_exec_envs_with_flags() {
        let sample = "\
1001 incus exec --cwd /workspace -e FOO=bar --env BAZ=1 dev-box -- bash
1002 /usr/bin/incus exec --user 1000 --mode=interactive alpha-box
1003 incus exec -- dev-box
1004 incus shell other-box
1005 incus exec dev-box -- ls
";
        let got: Vec<String> = parse_active_incus_exec_envs(sample).into_iter().collect();
        assert_eq!(got, vec!["alpha-box", "dev-box"]);
    }

    #[test]
    fn idle_stop_overrides_match_qt_test() {
        assert_eq!(parse_idle_stop_override("").mode, IdleStopMode::Inherit);
        assert_eq!(
            parse_idle_stop_override("inherit").mode,
            IdleStopMode::Inherit
        );
        assert_eq!(parse_idle_stop_override("off").mode, IdleStopMode::Off);
        assert_eq!(parse_idle_stop_override("0").mode, IdleStopMode::Off);
        assert_eq!(parse_idle_stop_override("false").mode, IdleStopMode::Off);
        let m30 = parse_idle_stop_override("30");
        assert_eq!(m30.mode, IdleStopMode::Minutes);
        assert_eq!(m30.minutes, 30);

        assert_eq!(
            effective_idle_stop_minutes(0, &parse_idle_stop_override("inherit")),
            0
        );
        assert_eq!(
            effective_idle_stop_minutes(60, &parse_idle_stop_override("inherit")),
            60
        );
        assert_eq!(
            effective_idle_stop_minutes(60, &parse_idle_stop_override("off")),
            0
        );
        assert_eq!(
            effective_idle_stop_minutes(0, &parse_idle_stop_override("15")),
            15
        );
        assert_eq!(
            effective_idle_stop_minutes(60, &parse_idle_stop_override("15")),
            15
        );
    }

    #[test]
    fn idle_decision_synthetic_samples_and_blockers_match_qt_test() {
        let min_ms = 60_i64 * 1000;
        let mut base = IdleDecisionInput {
            running: true,
            threshold_minutes: 15,
            samples: vec![
                IdleCpuSample {
                    timestamp_ms: 0,
                    cpu_percent: 1,
                },
                IdleCpuSample {
                    timestamp_ms: 5 * min_ms,
                    cpu_percent: 2,
                },
                IdleCpuSample {
                    timestamp_ms: 10 * min_ms,
                    cpu_percent: 1,
                },
                IdleCpuSample {
                    timestamp_ms: 14 * min_ms,
                    cpu_percent: 1,
                },
            ],
            now_ms: 14 * min_ms,
            ..Default::default()
        };

        let warn_res = evaluate_idle_decision(&base);
        assert!(warn_res.eligible);
        assert!(warn_res.should_warn_one_min);
        assert!(!warn_res.should_stop_now);

        base.warning_already_sent = true;
        assert!(!evaluate_idle_decision(&base).should_warn_one_min);

        base.samples.push(IdleCpuSample {
            timestamp_ms: 15 * min_ms,
            cpu_percent: 0,
        });
        base.now_ms = 15 * min_ms;
        let stop_res = evaluate_idle_decision(&base);
        assert!(stop_res.eligible);
        assert!(stop_res.should_stop_now);

        let mut spike = base.clone();
        spike.warning_already_sent = false;
        spike.samples = vec![
            IdleCpuSample {
                timestamp_ms: 0,
                cpu_percent: 1,
            },
            IdleCpuSample {
                timestamp_ms: 5 * min_ms,
                cpu_percent: 1,
            },
            IdleCpuSample {
                timestamp_ms: 10 * min_ms,
                cpu_percent: 8,
            },
            IdleCpuSample {
                timestamp_ms: 11 * min_ms,
                cpu_percent: 1,
            },
            IdleCpuSample {
                timestamp_ms: 15 * min_ms,
                cpu_percent: 1,
            },
        ];
        spike.now_ms = 15 * min_ms;
        let spike_res = evaluate_idle_decision(&spike);
        assert!(spike_res.eligible);
        assert!(!spike_res.should_warn_one_min);
        assert!(!spike_res.should_stop_now);
        assert_eq!(spike_res.idle_duration_ms, 4 * min_ms);

        let mut with_exec = base.clone();
        with_exec.has_active_exec_session = true;
        let exec_res = evaluate_idle_decision(&with_exec);
        assert!(!exec_res.eligible);
        assert!(!exec_res.should_stop_now);
        assert_eq!(exec_res.blocked_reason, "exec-session");

        let parsed_exec = parse_active_incus_exec_envs(
            "111 incus exec dev -- bash\n222 incus exec --cwd /workspace box-2 -- sh\n",
        );
        assert!(parsed_exec.contains("dev"));
        assert!(parsed_exec.contains("box-2"));
        assert!(!parsed_exec.contains("dev-2"));

        let mut with_ports = base.clone();
        with_ports.port_forward_count = 2;
        let ports_blocked = evaluate_idle_decision(&with_ports);
        assert!(!ports_blocked.eligible);
        assert_eq!(ports_blocked.blocked_reason, "port-forwards");
        with_ports.ignore_port_forwards = true;
        assert!(evaluate_idle_decision(&with_ports).should_stop_now);

        let mut tpl = base.clone();
        tpl.is_template = true;
        let tpl_res = evaluate_idle_decision(&tpl);
        assert!(!tpl_res.eligible);
        assert_eq!(tpl_res.blocked_reason, "template");

        let mut busy = base;
        busy.busy_on_this_env = true;
        let busy_res = evaluate_idle_decision(&busy);
        assert!(!busy_res.eligible);
        assert_eq!(busy_res.blocked_reason, "busy");
    }

    #[test]
    fn extracts_cpu_and_net_stats_for_all_instances_from_single_incus_list() {
        let make_payload = |cpu_ns: u64, rx_bytes: u64, tx_bytes: u64| {
            format!(
                r#"[
                    {{
                        "name": "box-a",
                        "status": "Running",
                        "type": "container",
                        "config": {{"limits.cpu": "2"}},
                        "state": {{
                            "cpu": {{"usage": {cpu_ns}}},
                            "network": {{
                                "eth0": {{"type": "broadcast", "counters": {{"bytes_received": {rx_bytes}, "bytes_sent": {tx_bytes}}}}}
                            }}
                        }}
                    }},
                    {{
                        "name": "box-b",
                        "status": "Stopped",
                        "type": "container",
                        "config": {{}},
                        "state": {{}}
                    }}
                ]"#
            )
        };

        let mut counters = CounterState::default();
        let first = extract_idle_stats_from_incus_list(
            make_payload(1_000_000_000, 1000, 2000).as_bytes(),
            &mut counters,
            0,
        );
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].name, "box-a");
        assert!(first[0].running);
        assert_eq!(first[0].net_rx_bytes, 1000);
        assert_eq!(first[0].net_tx_bytes, 2000);
        assert_eq!(first[1].name, "box-b");
        assert!(!first[1].running);

        // Second poll 2 seconds later: +1s CPU across 2 cores = 25% CPU, +4000 RX bytes = 2000 B/s.
        let second = extract_idle_stats_from_incus_list(
            make_payload(2_000_000_000, 5000, 6000).as_bytes(),
            &mut counters,
            2_000_000_000,
        );
        assert_eq!(second[0].cpu_percent, 25);
        assert_eq!(second[0].net_rx_rate, 2000);
        assert_eq!(second[0].net_tx_rate, 2000);
    }
}
