use super::prelude::*;
use super::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use self::linux::collect_raw_snapshot;
#[cfg(target_os = "windows")]
use self::windows::collect_raw_snapshot_windows;

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn build_point(raw: &RawSnapshot, prev: Option<&RawSnapshot>) -> SystemMetricsPoint {
    let mem_used = raw.mem_total_bytes.saturating_sub(raw.mem_available_bytes);
    let swap_used = raw.swap_total_bytes.saturating_sub(raw.swap_free_bytes);

    let mem_used_percent = if raw.mem_total_bytes > 0 {
        (mem_used as f64 / raw.mem_total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let swap_used_percent = if raw.swap_total_bytes > 0 {
        (swap_used as f64 / raw.swap_total_bytes as f64) * 100.0
    } else {
        0.0
    };

    let fd_usage_percent = if raw.fd_max > 0 {
        (raw.fd_used as f64 / raw.fd_max as f64) * 100.0
    } else {
        0.0
    };

    let mut cpu_usage_percent = raw.cpu_usage_percent_hint.unwrap_or(0.0);
    let mut net_rx_bps = 0.0;
    let mut net_tx_bps = 0.0;
    let mut disk_read_bps = 0.0;
    let mut disk_write_bps = 0.0;

    if let Some(p) = prev {
        let dt = (raw.timestamp - p.timestamp).max(1) as f64;

        let total_delta = raw.cpu_total.saturating_sub(p.cpu_total);
        let idle_delta = raw.cpu_idle.saturating_sub(p.cpu_idle);
        if raw.cpu_usage_percent_hint.is_none() && total_delta > 0 {
            cpu_usage_percent =
                ((total_delta.saturating_sub(idle_delta)) as f64 / total_delta as f64) * 100.0;
        }

        let net_rx_delta = raw.net_rx_bytes.saturating_sub(p.net_rx_bytes);
        let net_tx_delta = raw.net_tx_bytes.saturating_sub(p.net_tx_bytes);
        net_rx_bps = net_rx_delta as f64 / dt;
        net_tx_bps = net_tx_delta as f64 / dt;

        let disk_r_delta = raw.disk_read_bytes.saturating_sub(p.disk_read_bytes);
        let disk_w_delta = raw.disk_write_bytes.saturating_sub(p.disk_write_bytes);
        disk_read_bps = disk_r_delta as f64 / dt;
        disk_write_bps = disk_w_delta as f64 / dt;
    }

    SystemMetricsPoint {
        timestamp: raw.timestamp,

        cpu_usage_percent,
        load1: raw.load1,
        load5: raw.load5,
        load15: raw.load15,

        mem_total_bytes: to_i64_saturated(raw.mem_total_bytes),
        mem_available_bytes: to_i64_saturated(raw.mem_available_bytes),
        mem_used_bytes: to_i64_saturated(mem_used),
        mem_used_percent,

        swap_total_bytes: to_i64_saturated(raw.swap_total_bytes),
        swap_free_bytes: to_i64_saturated(raw.swap_free_bytes),
        swap_used_bytes: to_i64_saturated(swap_used),
        swap_used_percent,

        net_rx_bytes: to_i64_saturated(raw.net_rx_bytes),
        net_tx_bytes: to_i64_saturated(raw.net_tx_bytes),
        net_rx_bps,
        net_tx_bps,

        disk_read_bytes: to_i64_saturated(raw.disk_read_bytes),
        disk_write_bytes: to_i64_saturated(raw.disk_write_bytes),
        disk_read_bps,
        disk_write_bps,

        tcp_established: raw.tcp_established,
        tcp_time_wait: raw.tcp_time_wait,
        tcp_close_wait: raw.tcp_close_wait,

        process_count: raw.process_count,
        fd_used: to_i64_saturated(raw.fd_used),
        fd_max: to_i64_saturated(raw.fd_max),
        fd_usage_percent,

        procs_running: raw.procs_running,
        procs_blocked: raw.procs_blocked,
        context_switches: to_i64_saturated(raw.context_switches),
        processes_forked_total: to_i64_saturated(raw.processes_forked_total),

        uptime_seconds: raw.uptime_seconds,
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) fn collect_one_point() -> Result<(SystemMetricsPoint, Vec<NetworkInterfaceStats>)> {
    #[cfg(target_os = "linux")]
    let raw = collect_raw_snapshot()?;
    #[cfg(target_os = "windows")]
    let raw = collect_raw_snapshot_windows()?;

    let point = {
        let mut last = LAST_RAW.write();
        let p = build_point(&raw, last.as_ref());
        *last = Some(raw.clone());
        p
    };

    Ok((point, raw.interfaces))
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub(super) fn collect_one_point() -> Result<(SystemMetricsPoint, Vec<NetworkInterfaceStats>)> {
    anyhow::bail!("system metrics are currently only supported on Linux and Windows");
}

pub(super) fn reset_platform_collect_state() {
    #[cfg(target_os = "windows")]
    self::windows::reset_windows_collect_state();
}
