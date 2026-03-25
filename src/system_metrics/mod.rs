#![cfg_attr(
    not(any(target_os = "linux", target_os = "windows")),
    allow(dead_code, unused_imports)
)]

#[cfg(target_os = "linux")]
use anyhow::Context;
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
#[cfg(target_os = "windows")]
use std::ffi::c_void;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use sqlx::QueryBuilder;
use std::collections::VecDeque;
#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::mem::{size_of, zeroed};
#[cfg(target_os = "windows")]
use std::ptr::null_mut;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, FILETIME};
#[cfg(target_os = "windows")]
use windows_sys::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, GetTcp6Table2, GetTcpTable2, IF_TYPE_SOFTWARE_LOOPBACK,
    IF_TYPE_TUNNEL, MIB_IF_TABLE2, MIB_TCP6TABLE2, MIB_TCPTABLE2, MIB_TCP_STATE_CLOSE_WAIT,
    MIB_TCP_STATE_ESTAB, MIB_TCP_STATE_TIME_WAIT,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::SystemInformation::{
    GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::GetSystemTimes;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::time::Instant;
use std::time::Duration;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use tauri::{Emitter, Manager};
use tauri::AppHandle;

mod state;
mod realtime;
mod types;

use self::state::*;
use self::realtime::*;
pub use types::*;
use self::types::{HistoricalRow, RawSnapshot, SystemMetricsEventPayload};

pub fn refresh_sample_interval_from_config() {
    state::refresh_sample_interval_from_config_state();
}

pub fn set_system_metrics_subscription(active: bool) {
    state::set_system_metrics_subscription_state(active);
}

#[cfg(target_os = "windows")]
#[inline]
fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

#[cfg(target_os = "windows")]
fn utf16z_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&x| x == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len]).trim().to_string()
}

#[cfg(target_os = "windows")]
fn update_windows_load_avg(cpu_usage_percent: f64, timestamp: i64) -> (f64, f64, f64) {
    let mut s = WINDOWS_LOAD_AVG.write();
    if !s.initialized {
        s.initialized = true;
        s.last_ts = timestamp;
        s.load1 = cpu_usage_percent;
        s.load5 = cpu_usage_percent;
        s.load15 = cpu_usage_percent;
        return (s.load1, s.load5, s.load15);
    }

    let dt = (timestamp - s.last_ts).max(1) as f64;
    s.last_ts = timestamp;

    let alpha1 = 1.0 - (-dt / 60.0).exp();
    let alpha5 = 1.0 - (-dt / 300.0).exp();
    let alpha15 = 1.0 - (-dt / 900.0).exp();

    s.load1 += alpha1 * (cpu_usage_percent - s.load1);
    s.load5 += alpha5 * (cpu_usage_percent - s.load5);
    s.load15 += alpha15 * (cpu_usage_percent - s.load15);
    (s.load1, s.load5, s.load15)
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn init_system_metrics_writer() {
    if SYSTEM_METRICS_TX.read().is_some() {
        return;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<SystemMetricsPoint>(20_000);
    *SYSTEM_METRICS_TX.write() = Some(tx);

    tauri::async_runtime::spawn(async move {
        let mut buf: Vec<SystemMetricsPoint> = Vec::with_capacity(DB_FLUSH_BATCH_SIZE);
        let mut last_flush = Instant::now();
        let mut last_retention_check = Instant::now();

        loop {
            tokio::select! {
                Some(item) = rx.recv() => {
                    buf.push(item);
                    if buf.len() >= DB_FLUSH_BATCH_SIZE {
                        flush_system_metrics(&mut buf).await;
                        last_flush = Instant::now();
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(200)) => {
                    if !buf.is_empty() && last_flush.elapsed() >= DB_FLUSH_INTERVAL {
                        flush_system_metrics(&mut buf).await;
                        last_flush = Instant::now();
                    }
                }
            }

            if last_retention_check.elapsed() >= SYSTEM_METRICS_RETENTION_CHECK_INTERVAL {
                if let Some(pool) = crate::metrics::db_pool() {
                    let cutoff = chrono::Utc::now().timestamp() - SYSTEM_METRICS_RETENTION_DAYS * 24 * 60 * 60;
                    let deleted_rows = sqlx::query("DELETE FROM system_metrics WHERE timestamp < ?")
                        .bind(cutoff)
                        .execute(&*pool)
                        .await
                        .map(|r| r.rows_affected())
                        .unwrap_or(0);
                    crate::metrics::reclaim_db_space_after_delete(&pool, deleted_rows).await;
                }
                last_retention_check = Instant::now();
            }
        }
    });
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn try_enqueue_system_metrics(point: SystemMetricsPoint) {
    if let Some(tx) = SYSTEM_METRICS_TX.read().as_ref() {
        let _ = tx.try_send(point);
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
async fn flush_system_metrics(buf: &mut Vec<SystemMetricsPoint>) {
    let Some(pool) = crate::metrics::db_pool() else {
        buf.clear();
        return;
    };

    if buf.is_empty() {
        return;
    }

    const CHUNK_SIZE: usize = 300;

    for chunk in buf.chunks(CHUNK_SIZE) {
        let mut qb = QueryBuilder::new(
            "INSERT INTO system_metrics (timestamp, cpu_usage_percent, load1, load5, load15, mem_total_bytes, mem_available_bytes, mem_used_bytes, mem_used_percent, swap_total_bytes, swap_free_bytes, swap_used_bytes, swap_used_percent, net_rx_bytes, net_tx_bytes, net_rx_bps, net_tx_bps, disk_read_bytes, disk_write_bytes, disk_read_bps, disk_write_bps, tcp_established, tcp_time_wait, tcp_close_wait, process_count, fd_used, fd_max, fd_usage_percent, procs_running, procs_blocked, context_switches, processes_forked_total, uptime_seconds) "
        );

        qb.push_values(chunk, |mut b, it| {
            b.push_bind(it.timestamp)
                .push_bind(it.cpu_usage_percent)
                .push_bind(it.load1)
                .push_bind(it.load5)
                .push_bind(it.load15)
                .push_bind(it.mem_total_bytes)
                .push_bind(it.mem_available_bytes)
                .push_bind(it.mem_used_bytes)
                .push_bind(it.mem_used_percent)
                .push_bind(it.swap_total_bytes)
                .push_bind(it.swap_free_bytes)
                .push_bind(it.swap_used_bytes)
                .push_bind(it.swap_used_percent)
                .push_bind(it.net_rx_bytes)
                .push_bind(it.net_tx_bytes)
                .push_bind(it.net_rx_bps)
                .push_bind(it.net_tx_bps)
                .push_bind(it.disk_read_bytes)
                .push_bind(it.disk_write_bytes)
                .push_bind(it.disk_read_bps)
                .push_bind(it.disk_write_bps)
                .push_bind(it.tcp_established)
                .push_bind(it.tcp_time_wait)
                .push_bind(it.tcp_close_wait)
                .push_bind(it.process_count)
                .push_bind(it.fd_used)
                .push_bind(it.fd_max)
                .push_bind(it.fd_usage_percent)
                .push_bind(it.procs_running)
                .push_bind(it.procs_blocked)
                .push_bind(it.context_switches)
                .push_bind(it.processes_forked_total)
                .push_bind(it.uptime_seconds);
        });

        let _ = qb.build().execute(&*pool).await;
    }

    buf.clear();
}

#[cfg(target_os = "linux")]
fn parse_proc_stat() -> Result<(u64, u64, i64, i64, u64, u64)> {
    let content = std::fs::read_to_string("/proc/stat").with_context(|| "failed to read /proc/stat")?;

    let mut cpu_total = 0u64;
    let mut cpu_idle = 0u64;
    let mut procs_running = 0i64;
    let mut procs_blocked = 0i64;
    let mut context_switches = 0u64;
    let mut processes_forked_total = 0u64;

    for line in content.lines() {
        if line.starts_with("cpu ") {
            let mut total = 0u64;
            let mut idle = 0u64;
            let mut iowait = 0u64;
            let mut count = 0usize;

            for (idx, v) in line
                .split_whitespace()
                .skip(1)
                .filter_map(|s| s.parse::<u64>().ok())
                .enumerate()
            {
                total = total.saturating_add(v);
                if idx == 3 {
                    idle = v;
                } else if idx == 4 {
                    iowait = v;
                }
                count += 1;
            }

            if count >= 4 {
                cpu_total = total;
                cpu_idle = idle.saturating_add(iowait);
            }
        } else if let Some(v) = line.strip_prefix("procs_running ") {
            procs_running = v.trim().parse::<i64>().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("procs_blocked ") {
            procs_blocked = v.trim().parse::<i64>().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("ctxt ") {
            context_switches = v.trim().parse::<u64>().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("processes ") {
            processes_forked_total = v.trim().parse::<u64>().unwrap_or(0);
        }
    }

    Ok((
        cpu_total,
        cpu_idle,
        procs_running,
        procs_blocked,
        context_switches,
        processes_forked_total,
    ))
}

#[cfg(target_os = "linux")]
fn parse_proc_meminfo() -> Result<(u64, u64, u64, u64)> {
    let content = std::fs::read_to_string("/proc/meminfo")
        .with_context(|| "failed to read /proc/meminfo")?;

    let mut mem_total_kb = 0u64;
    let mut mem_available_kb = 0u64;
    let mut swap_total_kb = 0u64;
    let mut swap_free_kb = 0u64;

    for line in content.lines() {
        if let Some(v) = line.strip_prefix("MemTotal:") {
            mem_total_kb = v
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("MemAvailable:") {
            mem_available_kb = v
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("SwapTotal:") {
            swap_total_kb = v
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("SwapFree:") {
            swap_free_kb = v
                .split_whitespace()
                .next()
                .and_then(|x| x.parse::<u64>().ok())
                .unwrap_or(0);
        }
    }

    Ok((
        mem_total_kb.saturating_mul(1024),
        mem_available_kb.saturating_mul(1024),
        swap_total_kb.saturating_mul(1024),
        swap_free_kb.saturating_mul(1024),
    ))
}

#[cfg(target_os = "linux")]
fn parse_proc_loadavg() -> Result<(f64, f64, f64)> {
    let content = std::fs::read_to_string("/proc/loadavg")
        .with_context(|| "failed to read /proc/loadavg")?;

    let parts: Vec<&str> = content.split_whitespace().collect();
    if parts.len() < 3 {
        return Err(anyhow!("invalid /proc/loadavg format"));
    }

    let l1 = parts[0].parse::<f64>().unwrap_or(0.0);
    let l5 = parts[1].parse::<f64>().unwrap_or(0.0);
    let l15 = parts[2].parse::<f64>().unwrap_or(0.0);
    Ok((l1, l5, l15))
}

#[cfg(target_os = "linux")]
fn parse_proc_uptime() -> Result<f64> {
    let content = std::fs::read_to_string("/proc/uptime")
        .with_context(|| "failed to read /proc/uptime")?;
    let first = content
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow!("invalid /proc/uptime format"))?;
    Ok(first.parse::<f64>().unwrap_or(0.0))
}

#[cfg(target_os = "linux")]
fn parse_proc_net_dev() -> Result<(u64, u64, Vec<NetworkInterfaceStats>)> {
    let content = std::fs::read_to_string("/proc/net/dev")
        .with_context(|| "failed to read /proc/net/dev")?;

    let mut total_rx: u64 = 0;
    let mut total_tx: u64 = 0;
    let mut interfaces: Vec<NetworkInterfaceStats> = Vec::new();

    for line in content.lines().skip(2) {
        let raw = line.trim();
        if raw.is_empty() {
            continue;
        }

        let Some((iface_name, counters)) = raw.split_once(':') else {
            continue;
        };

        let name = iface_name.trim();
        if name.is_empty() || name == "lo" {
            continue;
        }

        let cols: Vec<&str> = counters.split_whitespace().collect();
        if cols.len() < 16 {
            continue;
        }

        let rx = cols[0].parse::<u64>().unwrap_or(0);
        let tx = cols[8].parse::<u64>().unwrap_or(0);

        total_rx = total_rx.saturating_add(rx);
        total_tx = total_tx.saturating_add(tx);

        interfaces.push(NetworkInterfaceStats {
            name: name.to_string(),
            rx_bytes: to_i64_saturated(rx),
            tx_bytes: to_i64_saturated(tx),
        });
    }

    interfaces.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    Ok((total_rx, total_tx, interfaces))
}

#[cfg(target_os = "linux")]
fn parse_proc_diskstats() -> Result<(u64, u64)> {
    let content = std::fs::read_to_string("/proc/diskstats")
        .with_context(|| "failed to read /proc/diskstats")?;

    let mut read_sectors: u64 = 0;
    let mut write_sectors: u64 = 0;

    for line in content.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 14 {
            continue;
        }

        let name = cols[2];
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("fd") || name.starts_with("sr") {
            continue;
        }

        let r = cols[5].parse::<u64>().unwrap_or(0);
        let w = cols[9].parse::<u64>().unwrap_or(0);
        read_sectors = read_sectors.saturating_add(r);
        write_sectors = write_sectors.saturating_add(w);
    }

    // Linux block layer 统计一般按 512B 扇区
    Ok((read_sectors.saturating_mul(512), write_sectors.saturating_mul(512)))
}

#[cfg(target_os = "linux")]
fn parse_tcp_states_from(path: &str) -> (i64, i64, i64) {
    let Ok(content) = std::fs::read_to_string(path) else {
        return (0, 0, 0);
    };

    let mut established = 0i64;
    let mut time_wait = 0i64;
    let mut close_wait = 0i64;

    for line in content.lines().skip(1) {
        let mut cols = line.split_whitespace();
        let _ = cols.next();
        let _ = cols.next();
        let _ = cols.next();
        let Some(state) = cols.next() else {
            continue;
        };

        match state {
            "01" => established += 1,
            "06" => time_wait += 1,
            "08" => close_wait += 1,
            _ => {}
        }
    }

    (established, time_wait, close_wait)
}

#[cfg(target_os = "linux")]
fn parse_tcp_states() -> (i64, i64, i64) {
    let (a1, b1, c1) = parse_tcp_states_from("/proc/net/tcp");
    let (a2, b2, c2) = parse_tcp_states_from("/proc/net/tcp6");
    (a1 + a2, b1 + b2, c1 + c2)
}

#[cfg(target_os = "linux")]
fn count_processes() -> i64 {
    let Ok(iter) = std::fs::read_dir("/proc") else {
        return 0;
    };

    let mut count = 0i64;
    for entry in iter.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) {
                count += 1;
            }
        }
    }
    count
}

#[cfg(target_os = "linux")]
fn parse_file_nr() -> (u64, u64) {
    let Ok(content) = std::fs::read_to_string("/proc/sys/fs/file-nr") else {
        return (0, 0);
    };

    let mut nums = content
        .split_whitespace()
        .filter_map(|v| v.parse::<u64>().ok());

    let allocated = nums.next();
    let unused = nums.next();
    let max = nums.next();

    if let (Some(allocated), Some(unused), Some(max)) = (allocated, unused, max) {
        (allocated.saturating_sub(unused), max)
    } else {
        (0, 0)
    }
}

#[cfg(target_os = "windows")]
fn to_wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn normalize_windows_interface_name(raw: &str) -> Option<String> {
    let mut name = raw.trim().to_string();
    if name.is_empty() {
        return None;
    }

    // Windows 会为同一个网卡暴露 QoS/WFP 过滤器实例，名称后缀不同但计数重复。
    const FILTER_SUFFIX_MARKERS: [&str; 3] = [
        "-QoS Packet Scheduler-",
        "-WFP 802.3 MAC Layer LightWeight Filter-",
        "-WFP Native MAC Layer LightWeight Filter-",
    ];

    for marker in FILTER_SUFFIX_MARKERS {
        if let Some((base, _)) = name.split_once(marker) {
            name = base.trim().to_string();
            break;
        }
    }

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(target_os = "windows")]
fn pdh_counter_value_double(counter: usize) -> Option<f64> {
    let mut ctype: u32 = 0;
    let mut value: PDH_FMT_COUNTERVALUE = unsafe { zeroed() };
    let status = unsafe {
        PdhGetFormattedCounterValue(counter as *mut c_void, PDH_FMT_DOUBLE, &mut ctype, &mut value)
    };
    if status != 0 || value.CStatus != 0 {
        return None;
    }
    Some(unsafe { value.Anonymous.doubleValue }.max(0.0))
}

#[cfg(target_os = "windows")]
fn get_windows_disk_bps() -> (f64, f64) {
    let mut guard = WINDOWS_PDH.write();
    if guard.is_none() {
        let mut query: *mut c_void = null_mut();
        let open_status = unsafe { PdhOpenQueryW(null_mut(), 0, &mut query) };
        if open_status != 0 {
            return (0.0, 0.0);
        }

        let mut read_counter: *mut c_void = null_mut();
        let mut write_counter: *mut c_void = null_mut();
        let read_path = to_wide_z("\\PhysicalDisk(_Total)\\Disk Read Bytes/sec");
        let write_path = to_wide_z("\\PhysicalDisk(_Total)\\Disk Write Bytes/sec");

        let add_read = unsafe { PdhAddEnglishCounterW(query, read_path.as_ptr(), 0, &mut read_counter) };
        let add_write = unsafe { PdhAddEnglishCounterW(query, write_path.as_ptr(), 0, &mut write_counter) };
        if add_read != 0 || add_write != 0 {
            unsafe { PdhCloseQuery(query) };
            return (0.0, 0.0);
        }

        let _ = unsafe { PdhCollectQueryData(query) };
        *guard = Some(WindowsPdhState {
            query: query as usize,
            read_counter: read_counter as usize,
            write_counter: write_counter as usize,
        });
    }

    let Some(state) = guard.as_ref() else {
        return (0.0, 0.0);
    };

    let status = unsafe { PdhCollectQueryData(state.query as *mut c_void) };
    if status != 0 {
        return (0.0, 0.0);
    }

    let read = pdh_counter_value_double(state.read_counter).unwrap_or(0.0);
    let write = pdh_counter_value_double(state.write_counter).unwrap_or(0.0);
    (read, write)
}

#[cfg(target_os = "windows")]
fn collect_windows_network_stats() -> Result<(u64, u64, Vec<NetworkInterfaceStats>)> {
    let mut table_ptr: *mut MIB_IF_TABLE2 = null_mut();
    let status = unsafe { GetIfTable2(&mut table_ptr) };
    if status != 0 || table_ptr.is_null() {
        return Err(anyhow!("GetIfTable2 failed: status={status}"));
    }

    let mut merged: HashMap<String, (u64, u64)> = HashMap::new();

    unsafe {
        let table = &*table_ptr;
        let rows = std::slice::from_raw_parts(table.Table.as_ptr(), table.NumEntries as usize);
        for row in rows {
            if row.Type == IF_TYPE_SOFTWARE_LOOPBACK as u32 || row.Type == IF_TYPE_TUNNEL as u32 {
                continue;
            }

            let mut name = utf16z_to_string(&row.Alias);
            if name.is_empty() {
                name = utf16z_to_string(&row.Description);
            }
            let Some(name) = normalize_windows_interface_name(&name) else {
                continue;
            };

            let rx = row.InOctets;
            let tx = row.OutOctets;
            if rx == 0 && tx == 0 {
                continue;
            }

            let entry = merged.entry(name).or_insert((0, 0));
            // 同一物理网卡的过滤器实例通常重复计数，取最大值避免重复累加。
            entry.0 = entry.0.max(rx);
            entry.1 = entry.1.max(tx);
        }
        FreeMibTable(table_ptr as *mut c_void);
    }

    let mut total_rx: u64 = 0;
    let mut total_tx: u64 = 0;
    let mut interfaces: Vec<NetworkInterfaceStats> = Vec::with_capacity(merged.len());
    for (name, (rx, tx)) in merged {
        total_rx = total_rx.saturating_add(rx);
        total_tx = total_tx.saturating_add(tx);
        interfaces.push(NetworkInterfaceStats {
            name,
            rx_bytes: to_i64_saturated(rx),
            tx_bytes: to_i64_saturated(tx),
        });
    }

    interfaces.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    Ok((total_rx, total_tx, interfaces))
}

#[cfg(target_os = "windows")]
fn collect_windows_tcp_counts() -> (i64, i64, i64) {
    fn collect_v4() -> (i64, i64, i64) {
        let mut size: u32 = 0;
        let mut status = unsafe { GetTcpTable2(null_mut(), &mut size, 0) };
        if status != ERROR_INSUFFICIENT_BUFFER && status != 0 {
            return (0, 0, 0);
        }
        let mut buf = vec![0u8; size as usize];
        status = unsafe { GetTcpTable2(buf.as_mut_ptr() as *mut MIB_TCPTABLE2, &mut size, 0) };
        if status != 0 {
            return (0, 0, 0);
        }
        let table = unsafe { &*(buf.as_ptr() as *const MIB_TCPTABLE2) };
        let rows = unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };
        let mut established = 0i64;
        let mut time_wait = 0i64;
        let mut close_wait = 0i64;
        for row in rows {
            match row.dwState as i32 {
                x if x == MIB_TCP_STATE_ESTAB => established += 1,
                x if x == MIB_TCP_STATE_TIME_WAIT => time_wait += 1,
                x if x == MIB_TCP_STATE_CLOSE_WAIT => close_wait += 1,
                _ => {}
            }
        }
        (established, time_wait, close_wait)
    }

    fn collect_v6() -> (i64, i64, i64) {
        let mut size: u32 = 0;
        let mut status = unsafe { GetTcp6Table2(null_mut(), &mut size, 0) };
        if status != ERROR_INSUFFICIENT_BUFFER && status != 0 {
            return (0, 0, 0);
        }
        let mut buf = vec![0u8; size as usize];
        status = unsafe { GetTcp6Table2(buf.as_mut_ptr() as *mut MIB_TCP6TABLE2, &mut size, 0) };
        if status != 0 {
            return (0, 0, 0);
        }
        let table = unsafe { &*(buf.as_ptr() as *const MIB_TCP6TABLE2) };
        let rows = unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };
        let mut established = 0i64;
        let mut time_wait = 0i64;
        let mut close_wait = 0i64;
        for row in rows {
            match row.State as i32 {
                x if x == MIB_TCP_STATE_ESTAB => established += 1,
                x if x == MIB_TCP_STATE_TIME_WAIT => time_wait += 1,
                x if x == MIB_TCP_STATE_CLOSE_WAIT => close_wait += 1,
                _ => {}
            }
        }
        (established, time_wait, close_wait)
    }

    let (a1, b1, c1) = collect_v4();
    let (a2, b2, c2) = collect_v6();
    (a1 + a2, b1 + b2, c1 + c2)
}

#[cfg(target_os = "windows")]
fn collect_raw_snapshot_windows() -> Result<RawSnapshot> {
    let timestamp = chrono::Utc::now().timestamp();

    let mut idle: FILETIME = unsafe { zeroed() };
    let mut kernel: FILETIME = unsafe { zeroed() };
    let mut user: FILETIME = unsafe { zeroed() };
    let ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    if ok == 0 {
        return Err(anyhow!("GetSystemTimes failed"));
    }
    let cpu_idle = filetime_to_u64(idle);
    let cpu_total = filetime_to_u64(kernel).saturating_add(filetime_to_u64(user));

    let mut mem: MEMORYSTATUSEX = unsafe { zeroed() };
    mem.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    let mem_ok = unsafe { GlobalMemoryStatusEx(&mut mem) };
    if mem_ok == 0 {
        return Err(anyhow!("GlobalMemoryStatusEx failed"));
    }
    let mem_total_bytes = mem.ullTotalPhys;
    let mem_available_bytes = mem.ullAvailPhys;
    let swap_total_bytes = mem.ullTotalPageFile.saturating_sub(mem.ullTotalPhys);
    let swap_free_bytes = mem.ullAvailPageFile.saturating_sub(mem.ullAvailPhys);

    let (net_rx_bytes, net_tx_bytes, interfaces) =
        collect_windows_network_stats().unwrap_or((0, 0, Vec::new()));

    let (disk_read_bps, disk_write_bps) = get_windows_disk_bps();
    let (disk_read_bytes, disk_write_bytes) = {
        let mut acc = WINDOWS_DISK_ACCUM.write();
        let dt = if acc.2 > 0 {
            (timestamp - acc.2).max(1) as f64
        } else {
            effective_sample_interval_secs() as f64
        };
        acc.0 = acc.0.saturating_add((disk_read_bps * dt) as u64);
        acc.1 = acc.1.saturating_add((disk_write_bps * dt) as u64);
        acc.2 = timestamp;
        (acc.0, acc.1)
    };

    let (tcp_established, tcp_time_wait, tcp_close_wait) = collect_windows_tcp_counts();

    let mut perf: PERFORMANCE_INFORMATION = unsafe { zeroed() };
    let perf_ok = unsafe {
        GetPerformanceInfo(
            &mut perf,
            size_of::<PERFORMANCE_INFORMATION>() as u32,
        )
    };
    let (process_count, fd_used) = if perf_ok != 0 {
        (perf.ProcessCount as i64, perf.HandleCount as u64)
    } else {
        (0, 0)
    };

    let uptime_seconds = unsafe { GetTickCount64() as f64 / 1000.0 };
    let cpu_hint = {
        let prev = LAST_RAW.read();
        if let Some(p) = prev.as_ref() {
            let total_delta = cpu_total.saturating_sub(p.cpu_total);
            let idle_delta = cpu_idle.saturating_sub(p.cpu_idle);
            if total_delta > 0 {
                ((total_delta.saturating_sub(idle_delta)) as f64 / total_delta as f64) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        }
    };
    let (load1, load5, load15) = update_windows_load_avg(cpu_hint, timestamp);

    Ok(RawSnapshot {
        timestamp,
        cpu_total,
        cpu_idle,
        cpu_usage_percent_hint: None,
        load1,
        load5,
        load15,
        mem_total_bytes,
        mem_available_bytes,
        swap_total_bytes,
        swap_free_bytes,
        net_rx_bytes,
        net_tx_bytes,
        interfaces,
        disk_read_bytes,
        disk_write_bytes,
        tcp_established,
        tcp_time_wait,
        tcp_close_wait,
        process_count,
        fd_used,
        fd_max: 0,
        procs_running: 0,
        procs_blocked: 0,
        context_switches: 0,
        processes_forked_total: 0,
        uptime_seconds,
    })
}

#[cfg(target_os = "linux")]
fn collect_raw_snapshot() -> Result<RawSnapshot> {
    let timestamp = chrono::Utc::now().timestamp();

    let (cpu_total, cpu_idle, procs_running, procs_blocked, context_switches, processes_forked_total) =
        parse_proc_stat()?;
    let (load1, load5, load15) = parse_proc_loadavg()?;
    let (mem_total_bytes, mem_available_bytes, swap_total_bytes, swap_free_bytes) = parse_proc_meminfo()?;
    let (net_rx_bytes, net_tx_bytes, interfaces) = parse_proc_net_dev()?;
    let (disk_read_bytes, disk_write_bytes) = parse_proc_diskstats()?;
    let (tcp_established, tcp_time_wait, tcp_close_wait) = parse_tcp_states();
    let process_count = count_processes();
    let (fd_used, fd_max) = parse_file_nr();
    let uptime_seconds = parse_proc_uptime()?;

    Ok(RawSnapshot {
        timestamp,
        cpu_total,
        cpu_idle,
        cpu_usage_percent_hint: None,
        load1,
        load5,
        load15,
        mem_total_bytes,
        mem_available_bytes,
        swap_total_bytes,
        swap_free_bytes,
        net_rx_bytes,
        net_tx_bytes,
        interfaces,
        disk_read_bytes,
        disk_write_bytes,
        tcp_established,
        tcp_time_wait,
        tcp_close_wait,
        process_count,
        fd_used,
        fd_max,
        procs_running,
        procs_blocked,
        context_switches,
        processes_forked_total,
        uptime_seconds,
    })
}

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
            cpu_usage_percent = ((total_delta.saturating_sub(idle_delta)) as f64 / total_delta as f64) * 100.0;
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
fn collect_one_point() -> Result<(SystemMetricsPoint, Vec<NetworkInterfaceStats>)> {
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

#[cfg(any(target_os = "linux", target_os = "windows"))]
async fn collect_and_publish_one(app: &AppHandle, persist_enabled: bool, emit_to_frontend: bool) {
    let collected = tauri::async_runtime::spawn_blocking(collect_one_point).await;
    if let Ok(Ok((point, interfaces))) = collected {
        *LAST_INTERFACES.write() = interfaces.clone();
        push_realtime_point(point.clone());
        if persist_enabled {
            try_enqueue_system_metrics(point.clone());
        }

        if emit_to_frontend {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit(
                    "system-metrics",
                    SystemMetricsEventPayload {
                        point,
                        interfaces,
                    },
                );
            }
        }
    }
}

pub fn start_system_sampler(app: AppHandle) {
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = app;
        return;
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        if SAMPLER_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }

        refresh_sample_interval_from_config_inner();
        init_system_metrics_writer();

        let handle = tauri::async_runtime::spawn(async move {
            loop {
                if !SAMPLER_RUNNING.load(Ordering::Relaxed) {
                    break;
                }

                let has_subscriber = HAS_ACTIVE_SUBSCRIBER.load(Ordering::Relaxed);
                let cfg = crate::config::get_config();
                let wants_persistence = is_system_metrics_persistence_enabled(&cfg);
                if has_subscriber || wants_persistence {
                    collect_and_publish_one(&app, wants_persistence, has_subscriber).await;
                }

                let wait_secs = if has_subscriber || wants_persistence {
                    effective_sample_interval_secs()
                } else {
                    IDLE_PAUSE_INTERVAL_SECS
                } as u64;

                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(wait_secs)) => {}
                    _ = SAMPLER_WAKE.notified() => {}
                }
            }

            SAMPLER_RUNNING.store(false, Ordering::SeqCst);
        });

        *SAMPLER_HANDLE.write() = Some(handle);
    }
}

pub fn stop_system_sampler() {
    SAMPLER_RUNNING.store(false, Ordering::SeqCst);
    if let Some(h) = SAMPLER_HANDLE.write().take() {
        h.abort();
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(state) = WINDOWS_PDH.write().take() {
            unsafe { PdhCloseQuery(state.query as *mut c_void) };
        }
        *WINDOWS_DISK_ACCUM.write() = (0, 0, 0);
        *WINDOWS_LOAD_AVG.write() = WindowsLoadAvgState::default();
    }
}

pub fn get_system_metrics(window_seconds: Option<i64>) -> Result<SystemMetricsRealtimePayload> {
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = window_seconds;
        return Ok(SystemMetricsRealtimePayload {
            sample_interval_seconds: current_sample_interval_secs(),
            max_window_seconds: MAX_REALTIME_WINDOW_SECS,
            supported: false,
            message: Some("system metrics are currently only supported on Linux and Windows".to_string()),
            latest: None,
            points: vec![],
            interfaces: vec![],
            summary: None,
        });
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        let win = window_seconds
            .unwrap_or(24 * 3600)
            .clamp(MIN_SAMPLE_INTERVAL_SECS, MAX_REALTIME_WINDOW_SECS);

        let points = get_realtime_points(win);
        let summary = build_summary(&points);
        Ok(SystemMetricsRealtimePayload {
            sample_interval_seconds: effective_sample_interval_secs(),
            max_window_seconds: MAX_REALTIME_WINDOW_SECS,
            supported: true,
            message: None,
            latest: latest_point(),
            points,
            interfaces: LAST_INTERFACES.read().clone(),
            summary,
        })
    }
}

pub async fn query_historical_system_metrics(
    req: QuerySystemMetricsRequest,
) -> Result<QuerySystemMetricsResponse> {
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = req;
        return Ok(QuerySystemMetricsResponse {
            points: vec![],
            supported: false,
            message: Some("system metrics historical query is only supported on Linux and Windows".to_string()),
            summary: None,
        });
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        if req.end_time <= req.start_time {
            return Ok(QuerySystemMetricsResponse {
                points: vec![],
                supported: true,
                message: Some("end_time must be greater than start_time".to_string()),
                summary: None,
            });
        }

        let Some(pool) = crate::metrics::db_pool() else {
            return Ok(QuerySystemMetricsResponse {
                points: vec![],
                supported: true,
                message: Some("metrics database is not initialized".to_string()),
                summary: None,
            });
        };

        let span = req.end_time - req.start_time;
        let granularity = choose_granularity(span, req.granularity_secs);

        let rows = sqlx::query_as::<_, HistoricalRow>(
            r#"
            SELECT
              (timestamp / ?) * ? AS bucket,
              AVG(cpu_usage_percent) AS cpu_usage_percent,
              AVG(load1) AS load1,
              AVG(load5) AS load5,
              AVG(load15) AS load15,
              MAX(mem_total_bytes) AS mem_total_bytes,
              AVG(mem_available_bytes) AS mem_available_bytes,
              AVG(mem_used_bytes) AS mem_used_bytes,
              AVG(mem_used_percent) AS mem_used_percent,
              MAX(swap_total_bytes) AS swap_total_bytes,
              AVG(swap_free_bytes) AS swap_free_bytes,
              AVG(swap_used_bytes) AS swap_used_bytes,
              AVG(swap_used_percent) AS swap_used_percent,
              MAX(net_rx_bytes) AS net_rx_bytes,
              MAX(net_tx_bytes) AS net_tx_bytes,
              AVG(net_rx_bps) AS net_rx_bps,
              AVG(net_tx_bps) AS net_tx_bps,
              MAX(disk_read_bytes) AS disk_read_bytes,
              MAX(disk_write_bytes) AS disk_write_bytes,
              AVG(disk_read_bps) AS disk_read_bps,
              AVG(disk_write_bps) AS disk_write_bps,
              AVG(tcp_established) AS tcp_established,
              AVG(tcp_time_wait) AS tcp_time_wait,
              AVG(tcp_close_wait) AS tcp_close_wait,
              AVG(process_count) AS process_count,
              AVG(fd_used) AS fd_used,
              MAX(fd_max) AS fd_max,
              AVG(fd_usage_percent) AS fd_usage_percent,
              AVG(procs_running) AS procs_running,
              AVG(procs_blocked) AS procs_blocked,
              MAX(context_switches) AS context_switches,
              MAX(processes_forked_total) AS processes_forked_total,
              AVG(uptime_seconds) AS uptime_seconds
            FROM system_metrics
            WHERE timestamp >= ? AND timestamp <= ?
            GROUP BY bucket
            ORDER BY bucket ASC
            "#,
        )
        .bind(granularity)
        .bind(granularity)
        .bind(req.start_time)
        .bind(req.end_time)
        .fetch_all(&*pool)
        .await?;

        let points = rows
            .into_iter()
            .map(|r| SystemMetricsPoint {
                timestamp: r.bucket,
                cpu_usage_percent: r.cpu_usage_percent.unwrap_or(0.0),
                load1: r.load1.unwrap_or(0.0),
                load5: r.load5.unwrap_or(0.0),
                load15: r.load15.unwrap_or(0.0),
                mem_total_bytes: r.mem_total_bytes.unwrap_or(0),
                mem_available_bytes: round_to_i64(r.mem_available_bytes),
                mem_used_bytes: round_to_i64(r.mem_used_bytes),
                mem_used_percent: r.mem_used_percent.unwrap_or(0.0),
                swap_total_bytes: r.swap_total_bytes.unwrap_or(0),
                swap_free_bytes: round_to_i64(r.swap_free_bytes),
                swap_used_bytes: round_to_i64(r.swap_used_bytes),
                swap_used_percent: r.swap_used_percent.unwrap_or(0.0),
                net_rx_bytes: r.net_rx_bytes.unwrap_or(0),
                net_tx_bytes: r.net_tx_bytes.unwrap_or(0),
                net_rx_bps: r.net_rx_bps.unwrap_or(0.0),
                net_tx_bps: r.net_tx_bps.unwrap_or(0.0),
                disk_read_bytes: r.disk_read_bytes.unwrap_or(0),
                disk_write_bytes: r.disk_write_bytes.unwrap_or(0),
                disk_read_bps: r.disk_read_bps.unwrap_or(0.0),
                disk_write_bps: r.disk_write_bps.unwrap_or(0.0),
                tcp_established: round_to_i64(r.tcp_established),
                tcp_time_wait: round_to_i64(r.tcp_time_wait),
                tcp_close_wait: round_to_i64(r.tcp_close_wait),
                process_count: round_to_i64(r.process_count),
                fd_used: round_to_i64(r.fd_used),
                fd_max: r.fd_max.unwrap_or(0),
                fd_usage_percent: r.fd_usage_percent.unwrap_or(0.0),
                procs_running: round_to_i64(r.procs_running),
                procs_blocked: round_to_i64(r.procs_blocked),
                context_switches: r.context_switches.unwrap_or(0),
                processes_forked_total: r.processes_forked_total.unwrap_or(0),
                uptime_seconds: r.uptime_seconds.unwrap_or(0.0),
            })
            .collect::<Vec<_>>();

        let summary = build_summary(&points);
        Ok(QuerySystemMetricsResponse {
            points: downsample_points(points, MAX_CHART_POINTS),
            supported: true,
            message: None,
            summary,
        })
    }
}
