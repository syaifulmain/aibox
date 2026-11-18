use aibox_core::{health, process};
use serde_json::{json, Value};

use crate::common;

fn pool_info_from_json_val(v: &Value) -> health::pool::StoragePoolInfo {
    health::pool::StoragePoolInfo {
        valid: v["valid"].as_bool().expect("valid"),
        name: v["name"].as_str().expect("name").to_string(),
        driver: v["driver"].as_str().expect("driver").to_string(),
        used_bytes: v["usedBytes"].as_f64().expect("usedBytes") as i64,
        total_bytes: v["totalBytes"].as_f64().expect("totalBytes") as i64,
        used_percent: v["usedPercent"].as_i64().expect("usedPercent") as i32,
        size_text: v["sizeText"].as_str().expect("sizeText").to_string(),
        resizable: v["resizable"].as_bool().expect("resizable"),
    }
}

fn pool_info_to_json_val(p: &health::pool::StoragePoolInfo) -> Value {
    json!({
        "valid": p.valid,
        "name": p.name,
        "driver": p.driver,
        "usedBytes": p.used_bytes,
        "totalBytes": p.total_bytes,
        "usedPercent": p.used_percent,
        "sizeText": p.size_text,
        "resizable": p.resizable,
    })
}

fn health_result_to_json_val(r: &health::checks::HealthCheckResult) -> Value {
    json!({
        "id": r.id,
        "title": r.title,
        "ok": r.ok,
        "warning": r.warning,
        "repaired": r.repaired,
        "needsRoot": r.needs_root,
        "detail": r.detail,
        "fixCommand": r.fix_command,
    })
}

struct GoldenRunner<F>(F, bool);
impl<F: Fn(&str, &[&str]) -> process::CommandOutput> process::CommandRunner for GoldenRunner<F> {
    fn run(&self, program: &str, args: &[&str]) -> process::CommandOutput {
        (self.0)(program, args)
    }

    fn socket_writable(&self, _path: &str) -> bool {
        self.1
    }
}

fn dispatch_health_checks(
    case: &str,
    args: &Value,
    fix_state: &mut (bool, bool, bool, bool),
) -> Value {
    let fix = args["fix"].as_bool().expect("fix");
    let user = args["targetUser"].as_str().expect("targetUser");
    let ufw = args["ufwRulesContent"].as_str().expect("ufwRulesContent");
    let ws_base = args["workspaceBase"].as_str().expect("workspaceBase");

    let results = match case {
        "all_pass" => {
            let runner = GoldenRunner(
                |prog: &str, a: &[&str]| {
                    match prog {
                    "id" => process::CommandOutput {
                        exit_code: 0,
                        std_out: "testuser wheel incus-admin kvm render video\n".to_string(),
                        std_err: String::new(),
                    },
                    "systemctl" if a.first() == Some(&"is-active") => process::CommandOutput {
                        exit_code: 0,
                        std_out: "active\n".to_string(),
                        std_err: String::new(),
                    },
                    "incus" => match (a.first().copied(), a.get(1).copied()) {
                        (Some("list"), _) => process::CommandOutput {
                            exit_code: 0,
                            std_out: "[]".to_string(),
                            std_err: String::new(),
                        },
                        (Some("network"), Some("show")) => process::CommandOutput {
                            exit_code: 0,
                            std_out: "name: incusbr0\nmanaged: true\n".to_string(),
                            std_err: String::new(),
                        },
                        (Some("profile"), Some("show")) => process::CommandOutput {
                            exit_code: 0,
                            std_out: "config:\n  security.nesting: \"true\"\ndevices:\n  gpu:\n    type: gpu\n  kvm:\n    path: /dev/kvm\n".to_string(),
                            std_err: String::new(),
                        },
                        (Some("storage"), Some("info")) => process::CommandOutput {
                            exit_code: 0,
                            std_out: "info:\n  driver: btrfs\n  name: default\n  space used: \"1329242112\"\n  total space: \"32212254720\"\n".to_string(),
                            std_err: String::new(),
                        },
                        _ => process::CommandOutput {
                            exit_code: 1,
                            std_out: String::new(),
                            std_err: "unexpected command".to_string(),
                        },
                    },
                    "findmnt" => process::CommandOutput {
                        exit_code: 0,
                        std_out: "/home btrfs rw,relatime,user_subvol_rm_allowed\n".to_string(),
                        std_err: String::new(),
                    },
                    _ => process::CommandOutput {
                        exit_code: 1,
                        std_out: String::new(),
                        std_err: "unexpected command".to_string(),
                    },
                }
                },
                true,
            );
            health::checks::run_health_checks(fix, &runner, user, ufw, ws_base)
        }
        "fix_dry_run" | "fix_repaired" => {
            let cell = std::cell::RefCell::new(*fix_state);
            let runner = GoldenRunner(
                |prog: &str, a: &[&str]| {
                    let mut st = cell.borrow_mut();
                    match prog {
                        "id" => process::CommandOutput {
                            exit_code: 0,
                            std_out: "testuser incus-admin kvm render video\n".to_string(),
                            std_err: String::new(),
                        },
                        "systemctl" => process::CommandOutput {
                            exit_code: 3,
                            std_out: "inactive\n".to_string(),
                            std_err: String::new(),
                        },
                        "incus" => match (a.first().copied(), a.get(1).copied()) {
                            (Some("list"), _) => process::CommandOutput {
                                exit_code: 0,
                                std_out: "[]".to_string(),
                                std_err: String::new(),
                            },
                            (Some("network"), Some("show")) => {
                                if st.0 {
                                    process::CommandOutput {
                                        exit_code: 0,
                                        std_out: "name: incusbr0\n".to_string(),
                                        std_err: String::new(),
                                    }
                                } else {
                                    process::CommandOutput {
                                        exit_code: 1,
                                        std_out: String::new(),
                                        std_err: "not found".to_string(),
                                    }
                                }
                            }
                            (Some("network"), Some("create")) if a.get(2) == Some(&"incusbr0") => {
                                st.0 = true;
                                process::CommandOutput {
                                    exit_code: 0,
                                    ..Default::default()
                                }
                            }
                            (Some("profile"), Some("show")) => {
                                let mut p_out = String::from("config:\n");
                                if st.1 {
                                    p_out.push_str("  security.nesting: \"true\"\n");
                                }
                                p_out.push_str("devices:\n");
                                if st.2 {
                                    p_out.push_str("  gpu:\n    type: gpu\n");
                                }
                                if st.3 {
                                    p_out.push_str("  kvm:\n    path: /dev/kvm\n");
                                }
                                process::CommandOutput {
                                    exit_code: 0,
                                    std_out: p_out,
                                    std_err: String::new(),
                                }
                            }
                            (Some("profile"), Some("set")) => {
                                st.1 = true;
                                process::CommandOutput {
                                    exit_code: 0,
                                    ..Default::default()
                                }
                            }
                            (Some("profile"), Some("device")) if a.get(2) == Some(&"add") => {
                                if a.get(4) == Some(&"gpu") {
                                    st.2 = true;
                                }
                                if a.get(4) == Some(&"kvm") {
                                    st.3 = true;
                                }
                                process::CommandOutput {
                                    exit_code: 0,
                                    ..Default::default()
                                }
                            }
                            _ => process::CommandOutput {
                                exit_code: 1,
                                std_out: String::new(),
                                std_err: "unhandled".to_string(),
                            },
                        },
                        _ => process::CommandOutput {
                            exit_code: 1,
                            std_out: String::new(),
                            std_err: "unhandled".to_string(),
                        },
                    }
                },
                true,
            );
            let res = health::checks::run_health_checks(fix, &runner, user, ufw, ws_base);
            *fix_state = *cell.borrow();
            res
        }
        "root_failures" => {
            let runner = GoldenRunner(
                |prog: &str, a: &[&str]| match prog {
                    "id" => process::CommandOutput {
                        exit_code: 0,
                        std_out: "testuser wheel\n".to_string(),
                        std_err: String::new(),
                    },
                    "systemctl" if a.first() == Some(&"is-active") => process::CommandOutput {
                        exit_code: 0,
                        std_out: "active\n".to_string(),
                        std_err: String::new(),
                    },
                    "findmnt" => process::CommandOutput {
                        exit_code: 0,
                        std_out: "/home btrfs rw,relatime\n".to_string(),
                        std_err: String::new(),
                    },
                    _ => process::CommandOutput {
                        exit_code: 1,
                        std_out: String::new(),
                        std_err: "daemon down".to_string(),
                    },
                },
                false,
            );
            health::checks::run_health_checks(fix, &runner, user, ufw, ws_base)
        }
        "pool_warn_90" | "pool_fail_97" => {
            let pct = if case == "pool_warn_90" { "90" } else { "97" };
            let runner = GoldenRunner(
                |prog: &str, a: &[&str]| {
                    match prog {
                    "id" => process::CommandOutput {
                        exit_code: 0,
                        std_out: "testuser incus-admin kvm render video\n".to_string(),
                        std_err: String::new(),
                    },
                    "systemctl" => process::CommandOutput {
                        exit_code: 3,
                        std_out: "inactive\n".to_string(),
                        std_err: String::new(),
                    },
                    "incus" => match a.first().copied() {
                        Some("list") => process::CommandOutput {
                            exit_code: 0,
                            std_out: "[]".to_string(),
                            std_err: String::new(),
                        },
                        Some("network") => process::CommandOutput {
                            exit_code: 0,
                            std_out: "name: incusbr0\n".to_string(),
                            std_err: String::new(),
                        },
                        Some("profile") => process::CommandOutput {
                            exit_code: 0,
                            std_out: "config:\n  security.nesting: \"true\"\ndevices:\n  gpu:\n    type: gpu\n  kvm:\n    path: /dev/kvm\n".to_string(),
                            std_err: String::new(),
                        },
                        Some("storage") => process::CommandOutput {
                            exit_code: 0,
                            std_out: format!("info:\n  driver: btrfs\n  name: default\n  space used: \"{pct}\"\n  total space: \"100\"\n"),
                            std_err: String::new(),
                        },
                        _ => process::CommandOutput {
                            exit_code: 1,
                            ..Default::default()
                        },
                    },
                    "findmnt" => process::CommandOutput {
                        exit_code: 0,
                        std_out: "/home btrfs rw,user_subvol_rm_allowed\n".to_string(),
                        std_err: String::new(),
                    },
                    _ => process::CommandOutput {
                        exit_code: 1,
                        ..Default::default()
                    },
                }
                },
                true,
            );
            health::checks::run_health_checks(fix, &runner, user, ufw, ws_base)
        }
        other => panic!("unknown health check case: {other}"),
    };

    Value::Array(results.iter().map(health_result_to_json_val).collect())
}

fn dispatch(
    fn_name: &str,
    case: &str,
    args: &Value,
    fix_state: &mut (bool, bool, bool, bool),
) -> Value {
    if fn_name == "Backend::runHealthChecks" {
        return dispatch_health_checks(case, args, fix_state);
    }
    match fn_name {
        "Backend::evaluateWatchdog" => {
            let d = health::watchdog::evaluate_watchdog(
                args["percent"].as_i64().expect("percent") as i32,
                args["previousLevel"].as_i64().expect("previousLevel") as i32,
                args["warnThreshold"].as_i64().expect("warnThreshold") as i32,
                args["criticalThreshold"]
                    .as_i64()
                    .expect("criticalThreshold") as i32,
                args["autoStopEnabled"].as_bool().expect("autoStopEnabled"),
            );
            json!({
                "newLevel": d.new_level,
                "shouldNotify": d.should_notify,
                "shouldAutoStop": d.should_auto_stop,
            })
        }
        "Backend::isValidProcessLimit" => {
            let v = args["value"].as_str().expect("value");
            json!(health::limits::is_valid_process_limit(v))
        }
        "Backend::isValidIoLimit" => {
            let v = args["value"].as_str().expect("value");
            json!(health::limits::is_valid_io_limit(v))
        }
        "Backend::parseIdleStopOverride" => {
            let raw = args["raw"].as_str().expect("raw");
            let ov = health::idle::parse_idle_stop_override(raw);
            let mode = match ov.mode {
                health::idle::IdleStopMode::Inherit => "inherit",
                health::idle::IdleStopMode::Off => "off",
                health::idle::IdleStopMode::Minutes => "minutes",
            };
            json!({
                "mode": mode,
                "minutes": ov.minutes,
            })
        }
        "Backend::effectiveIdleStopMinutes" => {
            let g = args["globalMinutes"].as_i64().expect("globalMinutes") as i32;
            let raw = args["raw"].as_str().expect("raw");
            let ov = health::idle::parse_idle_stop_override(raw);
            json!(health::idle::effective_idle_stop_minutes(g, &ov))
        }
        "Backend::evaluateIdleDecision" => {
            let samples = args["samples"]
                .as_array()
                .expect("samples")
                .iter()
                .map(|s| health::idle::IdleCpuSample {
                    timestamp_ms: s["timestampMs"].as_f64().expect("timestampMs") as i64,
                    cpu_percent: s["cpuPercent"].as_i64().expect("cpuPercent") as i32,
                })
                .collect();
            let input = health::idle::IdleDecisionInput {
                running: args["running"].as_bool().expect("running"),
                is_template: args["isTemplate"].as_bool().expect("isTemplate"),
                busy_on_this_env: args["busyOnThisEnv"].as_bool().expect("busyOnThisEnv"),
                has_active_exec_session: args["hasActiveExecSession"]
                    .as_bool()
                    .expect("hasActiveExecSession"),
                port_forward_count: args["portForwardCount"].as_i64().expect("portForwardCount")
                    as i32,
                ignore_port_forwards: args["ignorePortForwards"]
                    .as_bool()
                    .expect("ignorePortForwards"),
                threshold_minutes: args["thresholdMinutes"].as_i64().expect("thresholdMinutes")
                    as i32,
                now_ms: args["nowMs"].as_f64().expect("nowMs") as i64,
                warning_already_sent: args["warningAlreadySent"]
                    .as_bool()
                    .expect("warningAlreadySent"),
                samples,
            };
            let res = health::idle::evaluate_idle_decision(&input);
            let avg_cpu_val = if res.average_cpu_percent.fract() == 0.0 {
                json!(res.average_cpu_percent as i64)
            } else {
                json!(res.average_cpu_percent)
            };
            json!({
                "eligible": res.eligible,
                "idleDurationMs": res.idle_duration_ms,
                "averageCpuPercent": avg_cpu_val,
                "shouldWarnOneMin": res.should_warn_one_min,
                "shouldStopNow": res.should_stop_now,
                "blockedReason": res.blocked_reason,
            })
        }
        "Backend::parseIncusStorageInfo" => {
            let out = args["output"].as_str().expect("output");
            let info = health::pool::parse_incus_storage_info(out);
            pool_info_to_json_val(&info)
        }
        "Backend::sumRootDiskQuotasBytes" => {
            let j = args["incusListJson"].as_str().expect("incusListJson");
            let excl = args["excludeEnvName"].as_str().expect("excludeEnvName");
            let pool = args["poolName"].as_str().expect("poolName");
            let (bytes, unbounded) = health::pool::sum_root_disk_quotas_bytes(j, excl, pool);
            json!({
                "bytes": bytes,
                "unboundedCount": unbounded,
            })
        }
        "Backend::evaluatePoolOvercommitWarning" => {
            let pool = pool_info_from_json_val(&args["pool"]);
            let existing = args["existingQuotasBytes"]
                .as_f64()
                .expect("existingQuotasBytes") as i64;
            let requested = args["requestedBytes"].as_f64().expect("requestedBytes") as i64;
            let unbounded = args["unboundedCount"].as_i64().expect("unboundedCount") as i32;
            json!(health::pool::evaluate_pool_overcommit_warning(
                &pool, existing, requested, unbounded
            ))
        }
        "Backend::evaluatePoolCapacityHealth" => {
            let pool = pool_info_from_json_val(&args["pool"]);
            let allocated = args["allocatedQuotasBytes"]
                .as_f64()
                .expect("allocatedQuotasBytes") as i64;
            let unbounded = args["unboundedCount"].as_i64().expect("unboundedCount") as i32;
            health_result_to_json_val(&health::checks::evaluate_pool_capacity_health(
                &pool, allocated, unbounded,
            ))
        }
        "poolInfoToVariantMap" => {
            let pool = pool_info_from_json_val(&args["pool"]);
            let allocated = args["allocatedBytes"].as_f64().expect("allocatedBytes") as i64;
            let unbounded = args["unboundedCount"].as_i64().expect("unboundedCount") as i32;
            let s = pool.to_json(allocated, unbounded);
            serde_json::from_str(&s).expect("valid pool json")
        }
        "Backend::parsePoolSizeSpec" => {
            let spec = args["spec"].as_str().expect("spec");
            match health::pool::parse_pool_size_spec(spec) {
                Some(bytes) => json!({ "ok": true, "bytes": bytes }),
                None => json!({ "ok": false, "bytes": 0 }),
            }
        }
        "Backend::resizeStoragePoolScript" => {
            let pool = args["poolName"].as_str().expect("poolName");
            let spec = args["sizeSpec"].as_str().expect("sizeSpec");
            json!(health::pool::resize_storage_pool_script(pool, spec))
        }
        other => panic!("unregistered golden function: {other}"),
    }
}

#[test]
fn replay() {
    let mut fix_state = (false, false, false, false);
    common::replay_jsonl("health", |fn_name, case, args| {
        dispatch(fn_name, case, args, &mut fix_state)
    });
}
