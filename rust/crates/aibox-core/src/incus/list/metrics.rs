use std::fs;

use chrono::{DateTime, Datelike, NaiveDateTime, Utc};

use super::model::CounterState;

pub fn parse_cpu_count(cpu_limit_text: &str) -> i32 {
    let trimmed = cpu_limit_text.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("All") {
        return std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(1)
            .max(1);
    }

    if let Ok(direct) = trimmed.parse::<i32>() {
        if direct > 0 {
            return direct;
        }
    }

    let mut count = 0i32;
    for part in trimmed.split(',') {
        if part.is_empty() {
            continue;
        }
        let p = part.trim();
        if let Some(dash) = p.find('-') {
            if dash > 0 {
                let start = p[..dash].trim().parse::<i32>();
                let end = p[dash + 1..].trim().parse::<i32>();
                if let (Ok(s), Ok(e)) = (start, end) {
                    if e >= s {
                        count += e - s + 1;
                    }
                }
                continue;
            }
        }
        count += 1;
    }
    count.max(1)
}

pub fn format_percent(percent: i32) -> String {
    format!("{}%", percent.clamp(0, 100))
}

pub fn compute_usage_percent(used_bytes: i64, total_bytes: i64) -> i32 {
    if total_bytes > 0 && used_bytes > 0 {
        (((used_bytes as f64) * 100.0 / (total_bytes as f64)).round() as i32).clamp(0, 100)
    } else {
        0
    }
}

pub fn build_cpu_core_percents(
    core_count: i32,
    avg_percent: i32,
    running: bool,
    host_weights: &[i32],
) -> Vec<i32> {
    let n = core_count.clamp(1, 64) as usize;
    if !running || avg_percent <= 0 {
        return vec![0; n];
    }

    let mut weights = vec![1.0_f64; n];
    let mut weight_sum = 0.0_f64;
    for i in 0..n {
        let w = if i < host_weights.len() && host_weights[i] > 0 {
            f64::from(host_weights[i]) + 5.0
        } else {
            1.0 + 0.12 * (((i * 3) % n) as f64 - (n - 1) as f64 / 2.0)
        };
        let clamped = w.max(0.2);
        weights[i] = clamped;
        weight_sum += clamped;
    }

    let target_sum = avg_percent * n as i32;
    let mut current_sum = 0i32;
    let mut values = vec![0i32; n];
    for i in 0..n {
        let v = (((weights[i] / weight_sum) * f64::from(target_sum)).round() as i32).clamp(0, 100);
        values[i] = v;
        current_sum += v;
    }

    let mut diff = target_sum - current_sum;
    for v in &mut values {
        if diff == 0 {
            break;
        }
        if diff > 0 && *v < 100 {
            let add = diff.min(100 - *v);
            *v += add;
            diff -= add;
        } else if diff < 0 && *v > 0 {
            let sub = (-diff).min(*v);
            *v -= sub;
            diff += sub;
        }
    }

    values
}

pub(crate) fn read_host_core_weights(state: &mut CounterState) -> Vec<i32> {
    let (curr_host_idle, curr_host_total) = read_host_proc_stat_idle_and_total();
    let mut host_core_weights = Vec::new();
    if curr_host_idle.len() == state.prev_host_idle.len()
        && curr_host_total.len() == state.prev_host_total.len()
    {
        for i in 0..curr_host_total.len() {
            let d_total = curr_host_total[i] - state.prev_host_total[i];
            let d_idle = curr_host_idle[i] - state.prev_host_idle[i];
            let pct = if d_total > 0 {
                (((d_total - d_idle) * 100) / d_total).clamp(0, 100) as i32
            } else {
                0
            };
            host_core_weights.push(pct);
        }
    }
    if !curr_host_total.is_empty() {
        state.prev_host_idle = curr_host_idle;
        state.prev_host_total = curr_host_total;
    }
    host_core_weights
}

fn read_host_proc_stat_idle_and_total() -> (Vec<i64>, Vec<i64>) {
    let mut idle = Vec::new();
    let mut totals = Vec::new();
    let Ok(contents) = fs::read_to_string("/proc/stat") else {
        return (idle, totals);
    };

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        let bytes = line.as_bytes();
        if !line.starts_with("cpu") || bytes.len() < 4 || !bytes[3].is_ascii_digit() {
            continue;
        }
        let mut total = 0i64;
        let mut idle_val = 0i64;
        let mut col = 0i32;
        for part in line.split(' ').skip(1) {
            if part.is_empty() {
                continue;
            }
            let v = part.parse::<i64>().unwrap_or(0);
            total += v;
            if col == 3 || col == 4 {
                idle_val += v;
            }
            col += 1;
        }
        idle.push(idle_val);
        totals.push(total);
    }
    (idle, totals)
}

pub(crate) fn parse_started_at_ms(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        if dt.year() > 1970 {
            return Some(dt.with_timezone(&Utc).timestamp_millis());
        }
        return None;
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            if ndt.year() > 1970 {
                return Some(ndt.and_utc().timestamp_millis());
            }
            return None;
        }
    }
    None
}

pub(crate) struct MetricPollContext<'a> {
    pub elapsed_wall_ns: i64,
    pub now_utc_ms: i64,
    pub track_cpu_and_net: bool,
    pub prev_state: &'a CounterState,
}

pub(crate) fn compute_cpu_percent(
    name: &str,
    running: bool,
    cpu_usage_ns: i64,
    started_at: &str,
    cpu_core_count: i32,
    ctx: &MetricPollContext<'_>,
    next_cpu_snapshot: &mut std::collections::HashMap<String, i64>,
) -> i32 {
    if !running || cpu_usage_ns < 0 {
        return 0;
    }
    next_cpu_snapshot.insert(name.to_string(), cpu_usage_ns);
    if ctx.track_cpu_and_net && ctx.elapsed_wall_ns > 0 {
        if let Some(&prev_ns) = ctx.prev_state.cpu_snapshot_ns.get(name) {
            let delta_ns = cpu_usage_ns - prev_ns;
            if delta_ns >= 0 {
                let capacity_ns = (ctx.elapsed_wall_ns as f64) * f64::from(cpu_core_count.max(1));
                return (((delta_ns as f64) * 100.0 / capacity_ns).round() as i32).clamp(0, 100);
            }
            return 0;
        }
    }
    if let Some(started_ms) = parse_started_at_ms(started_at) {
        let uptime_ms = ctx.now_utc_ms - started_ms;
        if uptime_ms > 500 && cpu_usage_ns > 0 {
            let capacity_ns = (uptime_ms as f64) * 1_000_000.0 * f64::from(cpu_core_count.max(1));
            return (((cpu_usage_ns as f64) * 100.0 / capacity_ns).round() as i32).clamp(0, 100);
        }
    }
    0
}

pub(crate) fn compute_net_rates(
    name: &str,
    running: bool,
    net_rx_bytes: i64,
    net_tx_bytes: i64,
    ctx: &MetricPollContext<'_>,
    next_net_snapshot: &mut std::collections::HashMap<String, (i64, i64)>,
) -> (i64, i64) {
    if !(ctx.track_cpu_and_net && running) {
        return (0, 0);
    }
    let mut rx_rate = 0i64;
    let mut tx_rate = 0i64;
    if ctx.elapsed_wall_ns > 0 {
        if let Some(&(prev_rx, prev_tx)) = ctx.prev_state.net_counters.get(name) {
            if net_rx_bytes >= prev_rx {
                rx_rate = (((net_rx_bytes - prev_rx) as f64) * 1e9 / (ctx.elapsed_wall_ns as f64))
                    .round() as i64;
            }
            if net_tx_bytes >= prev_tx {
                tx_rate = (((net_tx_bytes - prev_tx) as f64) * 1e9 / (ctx.elapsed_wall_ns as f64))
                    .round() as i64;
            }
        }
    }
    next_net_snapshot.insert(name.to_string(), (net_rx_bytes, net_tx_bytes));
    (rx_rate, tx_rate)
}
