use aibox_core::backup::available_disk_bytes;
use aibox_core::health::{checks, idle, limits, pool, watchdog};

#[cxx::bridge(namespace = "aibox::ffi")]
mod ffi {
    struct WatchdogDecisionFfi {
        new_level: i32,
        should_notify: bool,
        should_auto_stop: bool,
    }

    struct WatchdogEnvInputFfi {
        name: String,
        running: bool,
        disk_percent: i32,
        disk_usage: String,
        disk_limit: String,
        disk_alert_level: i32,
        auto_stop_on_critical: bool,
        cpu_percent: i32,
        memory_percent: i32,
        prev_disk_level: i32,
        prev_cpu_level: i32,
        prev_cpu_streak: i32,
        prev_ram_level: i32,
        prev_ram_streak: i32,
    }

    struct WatchdogNotificationFfi {
        critical: bool,
        kind: i32, // 0=DiskWarning, 1=DiskCritical, 2=CpuHigh, 3=RamHigh
        env_name: String,
        percent: i32,
        disk_usage: String,
        disk_limit: String,
        auto_stopping: bool,
    }

    struct WatchdogEnvResultFfi {
        name: String,
        new_disk_level: i32,
        disk_alert_level_changed: bool,
        new_cpu_level: i32,
        new_cpu_streak: i32,
        new_ram_level: i32,
        new_ram_streak: i32,
    }

    struct WatchdogBatchResultFfi {
        envs: Vec<WatchdogEnvResultFfi>,
        notifications: Vec<WatchdogNotificationFfi>,
        auto_stop_target: String,
        auto_stop_script: String,
    }

    struct IdleStopOverrideFfi {
        mode: i32, // 0=Inherit, 1=Off, 2=Minutes
        minutes: i32,
    }

    struct IdleCpuSampleFfi {
        timestamp_ms: i64,
        cpu_percent: i32,
    }

    struct IdleDecisionInputFfi {
        running: bool,
        is_template: bool,
        busy_on_this_env: bool,
        has_active_exec_session: bool,
        port_forward_count: i32,
        ignore_port_forwards: bool,
        threshold_minutes: i32,
        now_ms: i64,
        warning_already_sent: bool,
        samples: Vec<IdleCpuSampleFfi>,
    }

    struct IdleDecisionResultFfi {
        eligible: bool,
        idle_duration_ms: i64,
        average_cpu_percent: f64,
        should_warn_one_min: bool,
        should_stop_now: bool,
        blocked_reason: String,
    }

    struct StoragePoolInfoFfi {
        valid: bool,
        name: String,
        driver: String,
        used_bytes: i64,
        total_bytes: i64,
        used_percent: i32,
        size_text: String,
        resizable: bool,
    }

    struct SumQuotasResultFfi {
        bytes: i64,
        unbounded_count: i32,
    }

    struct PoolSizeSpecResultFfi {
        ok: bool,
        bytes: i64,
    }

    struct HealthCheckResultFfi {
        id: String,
        title: String,
        ok: bool,
        warning: bool,
        repaired: bool,
        needs_root: bool,
        detail: String,
        fix_command: String,
    }

    unsafe extern "C++" {
        include!("command_runner.h");
        type CommandRunnerBridge;
        fn run(
            &self,
            program: &str,
            args: &[String],
            exit_code: &mut i32,
            std_out: &mut String,
            std_err: &mut String,
        );
    }

    extern "Rust" {
        fn is_valid_process_limit(value: &str) -> bool;
        fn is_valid_io_limit(value: &str) -> bool;

        fn evaluate_watchdog(
            percent: i32,
            previous_level: i32,
            warn_threshold: i32,
            critical_threshold: i32,
            auto_stop_enabled: bool,
        ) -> WatchdogDecisionFfi;
        fn evaluate_all_watchdogs(
            envs: &[WatchdogEnvInputFfi],
            disk_warn: i32,
            disk_critical: i32,
            cpu_warn: i32,
            ram_warn: i32,
            busy: bool,
        ) -> WatchdogBatchResultFfi;

        fn parse_idle_stop_override(raw: &str) -> IdleStopOverrideFfi;
        fn effective_idle_stop_minutes(global_minutes: i32, ov: &IdleStopOverrideFfi) -> i32;
        fn evaluate_idle_decision(input: &IdleDecisionInputFfi) -> IdleDecisionResultFfi;
        fn set_environment_auto_stop_script(name: &str, enabled: bool) -> String;
        fn set_environment_idle_stop_script(
            name: &str,
            value: &str,
            allow_with_port_forwards: bool,
        ) -> String;

        fn parse_incus_storage_info(output: &str) -> StoragePoolInfoFfi;
        fn sum_root_disk_quotas_bytes(
            incus_list_json: &str,
            exclude_env_name: &str,
            pool_name: &str,
        ) -> SumQuotasResultFfi;
        fn evaluate_pool_overcommit_warning(
            pool: &StoragePoolInfoFfi,
            existing_quotas_bytes: i64,
            requested_bytes: i64,
            unbounded_count: i32,
        ) -> String;
        fn pool_warning_for_create(
            pool: &StoragePoolInfoFfi,
            allocated_bytes: i64,
            unbounded_count: i32,
            disk_spec: &str,
        ) -> String;
        fn pool_warning_for_quota_edit(
            pool: &StoragePoolInfoFfi,
            incus_list_json: &str,
            fallback_names: &[String],
            fallback_quotas: &[i64],
            env_name: &str,
            disk_spec: &str,
        ) -> String;
        fn parse_pool_size_spec(spec: &str) -> PoolSizeSpecResultFfi;
        fn resize_storage_pool_script(pool_name: &str, size_spec: &str) -> String;
        fn validate_storage_pool_resize(
            busy: bool,
            task_lock_active: bool,
            pool: &StoragePoolInfoFfi,
            size_spec: &str,
        ) -> String;
        fn pool_info_to_json(
            pool: &StoragePoolInfoFfi,
            allocated_bytes: i64,
            unbounded_count: i32,
        ) -> String;

        fn evaluate_pool_capacity_health(
            pool: &StoragePoolInfoFfi,
            allocated_quotas_bytes: i64,
            unbounded_count: i32,
        ) -> HealthCheckResultFfi;
        fn run_health_checks(
            fix: bool,
            has_custom_runner: bool,
            runner: &CommandRunnerBridge,
            target_user: &str,
            ufw_rules_content: &str,
            workspace_base: &str,
        ) -> Vec<HealthCheckResultFfi>;
    }
}

fn is_valid_process_limit(value: &str) -> bool {
    limits::is_valid_process_limit(value)
}

fn is_valid_io_limit(value: &str) -> bool {
    limits::is_valid_io_limit(value)
}

fn evaluate_watchdog(
    percent: i32,
    previous_level: i32,
    warn_threshold: i32,
    critical_threshold: i32,
    auto_stop_enabled: bool,
) -> ffi::WatchdogDecisionFfi {
    let d = watchdog::evaluate_watchdog(
        percent,
        previous_level,
        warn_threshold,
        critical_threshold,
        auto_stop_enabled,
    );
    ffi::WatchdogDecisionFfi {
        new_level: d.new_level,
        should_notify: d.should_notify,
        should_auto_stop: d.should_auto_stop,
    }
}

fn evaluate_all_watchdogs(
    envs: &[ffi::WatchdogEnvInputFfi],
    disk_warn: i32,
    disk_critical: i32,
    cpu_warn: i32,
    ram_warn: i32,
    busy: bool,
) -> ffi::WatchdogBatchResultFfi {
    let core_envs: Vec<watchdog::WatchdogEnvInput> = envs
        .iter()
        .map(|e| watchdog::WatchdogEnvInput {
            name: e.name.clone(),
            running: e.running,
            disk_percent: e.disk_percent,
            disk_usage: e.disk_usage.clone(),
            disk_limit: e.disk_limit.clone(),
            disk_alert_level: e.disk_alert_level,
            auto_stop_on_critical: e.auto_stop_on_critical,
            cpu_percent: e.cpu_percent,
            memory_percent: e.memory_percent,
            prev_disk_level: e.prev_disk_level,
            prev_cpu_level: e.prev_cpu_level,
            prev_cpu_streak: e.prev_cpu_streak,
            prev_ram_level: e.prev_ram_level,
            prev_ram_streak: e.prev_ram_streak,
        })
        .collect();
    let res = watchdog::evaluate_all_watchdogs(
        &core_envs,
        disk_warn,
        disk_critical,
        cpu_warn,
        ram_warn,
        busy,
    );
    ffi::WatchdogBatchResultFfi {
        envs: res
            .envs
            .into_iter()
            .map(|e| ffi::WatchdogEnvResultFfi {
                name: e.name,
                new_disk_level: e.new_disk_level,
                disk_alert_level_changed: e.disk_alert_level_changed,
                new_cpu_level: e.new_cpu_level,
                new_cpu_streak: e.new_cpu_streak,
                new_ram_level: e.new_ram_level,
                new_ram_streak: e.new_ram_streak,
            })
            .collect(),
        notifications: res
            .notifications
            .into_iter()
            .map(|n| ffi::WatchdogNotificationFfi {
                critical: n.critical,
                kind: match n.kind {
                    watchdog::WatchdogNotifyKind::DiskWarning => 0,
                    watchdog::WatchdogNotifyKind::DiskCritical => 1,
                    watchdog::WatchdogNotifyKind::CpuHigh => 2,
                    watchdog::WatchdogNotifyKind::RamHigh => 3,
                },
                env_name: n.env_name,
                percent: n.percent,
                disk_usage: n.disk_usage,
                disk_limit: n.disk_limit,
                auto_stopping: n.auto_stopping,
            })
            .collect(),
        auto_stop_target: res.auto_stop_target,
        auto_stop_script: res.auto_stop_script,
    }
}

fn idle_override_to_core(ov: &ffi::IdleStopOverrideFfi) -> idle::IdleStopOverride {
    idle::IdleStopOverride {
        mode: match ov.mode {
            1 => idle::IdleStopMode::Off,
            2 => idle::IdleStopMode::Minutes,
            _ => idle::IdleStopMode::Inherit,
        },
        minutes: ov.minutes,
    }
}

fn parse_idle_stop_override(raw: &str) -> ffi::IdleStopOverrideFfi {
    let ov = idle::parse_idle_stop_override(raw);
    ffi::IdleStopOverrideFfi {
        mode: match ov.mode {
            idle::IdleStopMode::Inherit => 0,
            idle::IdleStopMode::Off => 1,
            idle::IdleStopMode::Minutes => 2,
        },
        minutes: ov.minutes,
    }
}

fn effective_idle_stop_minutes(global_minutes: i32, ov: &ffi::IdleStopOverrideFfi) -> i32 {
    idle::effective_idle_stop_minutes(global_minutes, &idle_override_to_core(ov))
}

fn evaluate_idle_decision(input: &ffi::IdleDecisionInputFfi) -> ffi::IdleDecisionResultFfi {
    let core_in = idle::IdleDecisionInput {
        running: input.running,
        is_template: input.is_template,
        busy_on_this_env: input.busy_on_this_env,
        has_active_exec_session: input.has_active_exec_session,
        port_forward_count: input.port_forward_count,
        ignore_port_forwards: input.ignore_port_forwards,
        threshold_minutes: input.threshold_minutes,
        now_ms: input.now_ms,
        warning_already_sent: input.warning_already_sent,
        samples: input
            .samples
            .iter()
            .map(|s| idle::IdleCpuSample {
                timestamp_ms: s.timestamp_ms,
                cpu_percent: s.cpu_percent,
            })
            .collect(),
    };
    let res = idle::evaluate_idle_decision(&core_in);
    ffi::IdleDecisionResultFfi {
        eligible: res.eligible,
        idle_duration_ms: res.idle_duration_ms,
        average_cpu_percent: res.average_cpu_percent,
        should_warn_one_min: res.should_warn_one_min,
        should_stop_now: res.should_stop_now,
        blocked_reason: res.blocked_reason,
    }
}

fn set_environment_auto_stop_script(name: &str, enabled: bool) -> String {
    idle::set_environment_auto_stop_script(name, enabled)
}

fn set_environment_idle_stop_script(
    name: &str,
    value: &str,
    allow_with_port_forwards: bool,
) -> String {
    idle::set_environment_idle_stop_script(name, value, allow_with_port_forwards)
}

fn pool_to_core(p: &ffi::StoragePoolInfoFfi) -> pool::StoragePoolInfo {
    pool::StoragePoolInfo {
        valid: p.valid,
        name: p.name.clone(),
        driver: p.driver.clone(),
        used_bytes: p.used_bytes,
        total_bytes: p.total_bytes,
        used_percent: p.used_percent,
        size_text: p.size_text.clone(),
        resizable: p.resizable,
    }
}

fn pool_to_ffi(p: pool::StoragePoolInfo) -> ffi::StoragePoolInfoFfi {
    ffi::StoragePoolInfoFfi {
        valid: p.valid,
        name: p.name,
        driver: p.driver,
        used_bytes: p.used_bytes,
        total_bytes: p.total_bytes,
        used_percent: p.used_percent,
        size_text: p.size_text,
        resizable: p.resizable,
    }
}

fn parse_incus_storage_info(output: &str) -> ffi::StoragePoolInfoFfi {
    pool_to_ffi(pool::parse_incus_storage_info(output))
}

fn sum_root_disk_quotas_bytes(
    incus_list_json: &str,
    exclude_env_name: &str,
    pool_name: &str,
) -> ffi::SumQuotasResultFfi {
    let (bytes, unbounded_count) =
        pool::sum_root_disk_quotas_bytes(incus_list_json, exclude_env_name, pool_name);
    ffi::SumQuotasResultFfi {
        bytes,
        unbounded_count,
    }
}

fn evaluate_pool_overcommit_warning(
    pool: &ffi::StoragePoolInfoFfi,
    existing_quotas_bytes: i64,
    requested_bytes: i64,
    unbounded_count: i32,
) -> String {
    pool::evaluate_pool_overcommit_warning(
        &pool_to_core(pool),
        existing_quotas_bytes,
        requested_bytes,
        unbounded_count,
    )
}

fn pool_warning_for_create(
    pool: &ffi::StoragePoolInfoFfi,
    allocated_bytes: i64,
    unbounded_count: i32,
    disk_spec: &str,
) -> String {
    pool::pool_warning_for_create(
        &pool_to_core(pool),
        allocated_bytes,
        unbounded_count,
        disk_spec,
    )
}

fn pool_warning_for_quota_edit(
    pool: &ffi::StoragePoolInfoFfi,
    incus_list_json: &str,
    fallback_names: &[String],
    fallback_quotas: &[i64],
    env_name: &str,
    disk_spec: &str,
) -> String {
    let fallback: Vec<(String, i64)> = fallback_names
        .iter()
        .cloned()
        .zip(fallback_quotas.iter().copied())
        .collect();
    pool::pool_warning_for_quota_edit(
        &pool_to_core(pool),
        incus_list_json,
        &fallback,
        env_name,
        disk_spec,
    )
}

fn parse_pool_size_spec(spec: &str) -> ffi::PoolSizeSpecResultFfi {
    match pool::parse_pool_size_spec(spec) {
        Some(bytes) => ffi::PoolSizeSpecResultFfi { ok: true, bytes },
        None => ffi::PoolSizeSpecResultFfi {
            ok: false,
            bytes: 0,
        },
    }
}

fn resize_storage_pool_script(pool_name: &str, size_spec: &str) -> String {
    pool::resize_storage_pool_script(pool_name, size_spec)
}

fn validate_storage_pool_resize(
    busy: bool,
    task_lock_active: bool,
    pool: &ffi::StoragePoolInfoFfi,
    size_spec: &str,
) -> String {
    let available = available_disk_bytes("/var/lib/incus");
    pool::validate_storage_pool_resize(
        busy,
        task_lock_active,
        &pool_to_core(pool),
        size_spec,
        available,
    )
}

fn pool_info_to_json(
    pool: &ffi::StoragePoolInfoFfi,
    allocated_bytes: i64,
    unbounded_count: i32,
) -> String {
    pool_to_core(pool).to_json(allocated_bytes, unbounded_count)
}

fn health_result_to_ffi(r: checks::HealthCheckResult) -> ffi::HealthCheckResultFfi {
    ffi::HealthCheckResultFfi {
        id: r.id,
        title: r.title,
        ok: r.ok,
        warning: r.warning,
        repaired: r.repaired,
        needs_root: r.needs_root,
        detail: r.detail,
        fix_command: r.fix_command,
    }
}

fn evaluate_pool_capacity_health(
    pool: &ffi::StoragePoolInfoFfi,
    allocated_quotas_bytes: i64,
    unbounded_count: i32,
) -> ffi::HealthCheckResultFfi {
    health_result_to_ffi(checks::evaluate_pool_capacity_health(
        &pool_to_core(pool),
        allocated_quotas_bytes,
        unbounded_count,
    ))
}

struct BridgeRunner<'a>(&'a ffi::CommandRunnerBridge);

impl checks::CommandRunner for BridgeRunner<'_> {
    fn run(&self, program: &str, args: &[&str]) -> checks::CommandOutput {
        let rust_args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
        let mut exit_code = -1;
        let mut std_out = String::new();
        let mut std_err = String::new();
        self.0.run(
            program,
            &rust_args,
            &mut exit_code,
            &mut std_out,
            &mut std_err,
        );
        checks::CommandOutput {
            exit_code,
            std_out,
            std_err,
        }
    }
}

fn run_health_checks(
    fix: bool,
    has_custom_runner: bool,
    runner: &ffi::CommandRunnerBridge,
    target_user: &str,
    ufw_rules_content: &str,
    workspace_base: &str,
) -> Vec<ffi::HealthCheckResultFfi> {
    let results = if has_custom_runner {
        let bridge = BridgeRunner(runner);
        checks::run_health_checks(fix, &bridge, target_user, ufw_rules_content, workspace_base)
    } else {
        checks::run_health_checks(
            fix,
            &checks::SystemRunner,
            target_user,
            ufw_rules_content,
            workspace_base,
        )
    };
    results.into_iter().map(health_result_to_ffi).collect()
}
