use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use serde_json::Value;
use sqlx::QueryBuilder;
use std::collections::VecDeque;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const DEFAULT_SAMPLE_INTERVAL_SECS: i64 = 10;
const MIN_SAMPLE_INTERVAL_SECS: i64 = 1;
const MAX_SAMPLE_INTERVAL_SECS: i64 = 300;
const IDLE_PAUSE_INTERVAL_SECS: i64 = 60;
const MAX_REALTIME_WINDOW_SECS: i64 = 7 * 24 * 3600; // 7 天
const MAX_REALTIME_POINTS: usize = (MAX_REALTIME_WINDOW_SECS / MIN_SAMPLE_INTERVAL_SECS + 8) as usize;
const MAX_CHART_POINTS: usize = 1200;

const DB_FLUSH_BATCH_SIZE: usize = 800;
const DB_FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const SYSTEM_METRICS_RETENTION_DAYS: i64 = 30;
const SYSTEM_METRICS_RETENTION_CHECK_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);

static SAMPLER_RUNNING: AtomicBool = AtomicBool::new(false);
static SAMPLER_HANDLE: Lazy<RwLock<Option<tauri::async_runtime::JoinHandle<()>>>> =
    Lazy::new(|| RwLock::new(None));
static SAMPLER_WAKE: Lazy<tokio::sync::Notify> = Lazy::new(tokio::sync::Notify::new);
static SAMPLE_INTERVAL_SECS: AtomicI64 = AtomicI64::new(DEFAULT_SAMPLE_INTERVAL_SECS);
static HAS_ACTIVE_SUBSCRIBER: AtomicBool = AtomicBool::new(false);

static SYSTEM_METRICS_TX: Lazy<RwLock<Option<tokio::sync::mpsc::Sender<SystemMetricsPoint>>>> =
    Lazy::new(|| RwLock::new(None));

static LAST_RAW: Lazy<RwLock<Option<RawSnapshot>>> = Lazy::new(|| RwLock::new(None));
static LAST_INTERFACES: Lazy<RwLock<Vec<NetworkInterfaceStats>>> = Lazy::new(|| RwLock::new(Vec::new()));
static REALTIME_POINTS: Lazy<RwLock<VecDeque<SystemMetricsPoint>>> =
    Lazy::new(|| RwLock::new(VecDeque::with_capacity(MAX_REALTIME_POINTS)));
#[cfg(target_os = "windows")]
static WINDOWS_DISK_ACCUM: Lazy<RwLock<(u64, u64, i64)>> = Lazy::new(|| RwLock::new((0, 0, 0)));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceStats {
    pub name: String,
    pub rx_bytes: i64,
    pub tx_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemMetricsPoint {
    pub timestamp: i64,

    pub cpu_usage_percent: f64,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,

    pub mem_total_bytes: i64,
    pub mem_available_bytes: i64,
    pub mem_used_bytes: i64,
    pub mem_used_percent: f64,

    pub swap_total_bytes: i64,
    pub swap_free_bytes: i64,
    pub swap_used_bytes: i64,
    pub swap_used_percent: f64,

    pub net_rx_bytes: i64,
    pub net_tx_bytes: i64,
    pub net_rx_bps: f64,
    pub net_tx_bps: f64,

    pub disk_read_bytes: i64,
    pub disk_write_bytes: i64,
    pub disk_read_bps: f64,
    pub disk_write_bps: f64,

    pub tcp_established: i64,
    pub tcp_time_wait: i64,
    pub tcp_close_wait: i64,

    pub process_count: i64,
    pub fd_used: i64,
    pub fd_max: i64,
    pub fd_usage_percent: f64,

    pub procs_running: i64,
    pub procs_blocked: i64,
    pub context_switches: i64,
    pub processes_forked_total: i64,

    pub uptime_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetricsRealtimePayload {
    pub sample_interval_seconds: i64,
    pub max_window_seconds: i64,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub latest: Option<SystemMetricsPoint>,
    pub points: Vec<SystemMetricsPoint>,
    pub interfaces: Vec<NetworkInterfaceStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySystemMetricsRequest {
    pub start_time: i64,
    pub end_time: i64,
    pub granularity_secs: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuerySystemMetricsResponse {
    pub points: Vec<SystemMetricsPoint>,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SystemMetricsEventPayload {
    point: SystemMetricsPoint,
    interfaces: Vec<NetworkInterfaceStats>,
}

#[derive(Debug, Clone)]
struct RawSnapshot {
    timestamp: i64,
    cpu_total: u64,
    cpu_idle: u64,
    cpu_usage_percent_hint: Option<f64>,

    load1: f64,
    load5: f64,
    load15: f64,

    mem_total_bytes: u64,
    mem_available_bytes: u64,
    swap_total_bytes: u64,
    swap_free_bytes: u64,

    net_rx_bytes: u64,
    net_tx_bytes: u64,
    interfaces: Vec<NetworkInterfaceStats>,

    disk_read_bytes: u64,
    disk_write_bytes: u64,

    tcp_established: i64,
    tcp_time_wait: i64,
    tcp_close_wait: i64,

    process_count: i64,
    fd_used: u64,
    fd_max: u64,

    procs_running: i64,
    procs_blocked: i64,
    context_switches: u64,
    processes_forked_total: u64,

    uptime_seconds: f64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct HistoricalRow {
    bucket: i64,
    cpu_usage_percent: Option<f64>,
    load1: Option<f64>,
    load5: Option<f64>,
    load15: Option<f64>,
    mem_total_bytes: Option<i64>,
    mem_available_bytes: Option<f64>,
    mem_used_bytes: Option<f64>,
    mem_used_percent: Option<f64>,
    swap_total_bytes: Option<i64>,
    swap_free_bytes: Option<f64>,
    swap_used_bytes: Option<f64>,
    swap_used_percent: Option<f64>,
    net_rx_bytes: Option<i64>,
    net_tx_bytes: Option<i64>,
    net_rx_bps: Option<f64>,
    net_tx_bps: Option<f64>,
    disk_read_bytes: Option<i64>,
    disk_write_bytes: Option<i64>,
    disk_read_bps: Option<f64>,
    disk_write_bps: Option<f64>,
    tcp_established: Option<f64>,
    tcp_time_wait: Option<f64>,
    tcp_close_wait: Option<f64>,
    process_count: Option<f64>,
    fd_used: Option<f64>,
    fd_max: Option<i64>,
    fd_usage_percent: Option<f64>,
    procs_running: Option<f64>,
    procs_blocked: Option<f64>,
    context_switches: Option<i64>,
    processes_forked_total: Option<i64>,
    uptime_seconds: Option<f64>,
}

#[inline]
fn to_i64_saturated(v: u64) -> i64 {
    if v > i64::MAX as u64 {
        i64::MAX
    } else {
        v as i64
    }
}

#[inline]
fn round_to_i64(v: Option<f64>) -> i64 {
    v.unwrap_or(0.0).round() as i64
}

#[inline]
fn normalize_sample_interval_secs(v: i64) -> i64 {
    v.clamp(MIN_SAMPLE_INTERVAL_SECS, MAX_SAMPLE_INTERVAL_SECS)
}

#[inline]
fn current_sample_interval_secs() -> i64 {
    normalize_sample_interval_secs(SAMPLE_INTERVAL_SECS.load(Ordering::Relaxed))
}

#[inline]
fn refresh_sample_interval_from_config_inner() {
    let cfg = crate::config::get_config();
    let interval = normalize_sample_interval_secs(cfg.system_metrics_sample_interval_secs);
    SAMPLE_INTERVAL_SECS.store(interval, Ordering::Relaxed);
}

pub fn refresh_sample_interval_from_config() {
    refresh_sample_interval_from_config_inner();
    SAMPLER_WAKE.notify_waiters();
}

pub fn set_system_metrics_subscription(active: bool) {
    HAS_ACTIVE_SUBSCRIBER.store(active, Ordering::Relaxed);
    SAMPLER_WAKE.notify_waiters();
}

#[inline]
fn choose_granularity(span: i64, requested: Option<i64>) -> i64 {
    if let Some(v) = requested {
        return v.max(1);
    }
    if span <= 6 * 3600 {
        current_sample_interval_secs()
    } else if span <= 3 * 24 * 3600 {
        60
    } else if span <= 14 * 24 * 3600 {
        300
    } else {
        900
    }
}

#[cfg(target_os = "windows")]
#[inline]
fn json_u64(v: &Value, key: &str) -> u64 {
    if let Some(u) = v.get(key).and_then(Value::as_u64) {
        return u;
    }
    if let Some(i) = v.get(key).and_then(Value::as_i64) {
        return i.max(0) as u64;
    }
    0
}

#[cfg(target_os = "windows")]
#[inline]
fn json_f64(v: &Value, key: &str) -> f64 {
    if let Some(f) = v.get(key).and_then(Value::as_f64) {
        return f;
    }
    if let Some(i) = v.get(key).and_then(Value::as_i64) {
        return i as f64;
    }
    if let Some(u) = v.get(key).and_then(Value::as_u64) {
        return u as f64;
    }
    0.0
}

#[cfg(target_os = "windows")]
#[inline]
fn json_i64(v: &Value, key: &str) -> i64 {
    if let Some(i) = v.get(key).and_then(Value::as_i64) {
        return i;
    }
    if let Some(u) = v.get(key).and_then(Value::as_u64) {
        return to_i64_saturated(u);
    }
    0
}

fn downsample_points(points: Vec<SystemMetricsPoint>, max_points: usize) -> Vec<SystemMetricsPoint> {
    if points.len() <= max_points || max_points == 0 {
        return points;
    }

    let step = ((points.len() as f64) / (max_points as f64)).ceil() as usize;
    let mut sampled = Vec::with_capacity(max_points + 1);

    for idx in (0..points.len()).step_by(step.max(1)) {
        sampled.push(points[idx].clone());
    }

    if let Some(last) = points.last() {
        let need_push_last = sampled
            .last()
            .map(|p| p.timestamp != last.timestamp)
            .unwrap_or(true);
        if need_push_last {
            sampled.push(last.clone());
        }
    }

    sampled
}

fn push_realtime_point(point: SystemMetricsPoint) {
    let mut buf = REALTIME_POINTS.write();
    buf.push_back(point);
    while buf.len() > MAX_REALTIME_POINTS {
        let _ = buf.pop_front();
    }
}

fn get_realtime_points(window_seconds: i64) -> Vec<SystemMetricsPoint> {
    let now = chrono::Utc::now().timestamp();
    let win = window_seconds.clamp(MIN_SAMPLE_INTERVAL_SECS, MAX_REALTIME_WINDOW_SECS);
    let min_ts = now - win;

    let points: Vec<SystemMetricsPoint> = REALTIME_POINTS
        .read()
        .iter()
        .filter(|p| p.timestamp >= min_ts)
        .cloned()
        .collect();

    downsample_points(points, MAX_CHART_POINTS)
}

fn latest_point() -> Option<SystemMetricsPoint> {
    REALTIME_POINTS.read().back().cloned()
}

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
                    let _ = sqlx::query("DELETE FROM system_metrics WHERE timestamp < ?")
                        .bind(cutoff)
                        .execute(&*pool)
                        .await;
                }
                last_retention_check = Instant::now();
            }
        }
    });
}

fn try_enqueue_system_metrics(point: SystemMetricsPoint) {
    if let Some(tx) = SYSTEM_METRICS_TX.read().as_ref() {
        let _ = tx.try_send(point);
    }
}

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
            let nums: Vec<u64> = line
                .split_whitespace()
                .skip(1)
                .filter_map(|s| s.parse::<u64>().ok())
                .collect();
            if nums.len() >= 4 {
                cpu_total = nums.iter().copied().sum::<u64>();
                let idle = nums.get(3).copied().unwrap_or(0);
                let iowait = nums.get(4).copied().unwrap_or(0);
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
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 {
            continue;
        }

        match cols[3] {
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

    let nums: Vec<u64> = content
        .split_whitespace()
        .filter_map(|v| v.parse::<u64>().ok())
        .collect();

    if nums.len() >= 3 {
        let allocated = nums[0];
        let unused = nums[1];
        let max = nums[2];
        (allocated.saturating_sub(unused), max)
    } else {
        (0, 0)
    }
}

#[cfg(target_os = "windows")]
fn run_powershell_json(script: &str) -> Result<Value> {
    let mut last_err = String::new();
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    for exe in ["powershell", "pwsh"] {
        match Command::new(exe)
            .creation_flags(CREATE_NO_WINDOW)
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    last_err = format!(
                        "{exe} exited with status {:?}: {}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stderr)
                    );
                    continue;
                }
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout.is_empty() {
                    last_err = format!("{exe} returned empty output");
                    continue;
                }
                let parsed: Value = serde_json::from_str(&stdout)
                    .with_context(|| format!("failed to parse powershell json: {stdout}"))?;
                return Ok(parsed);
            }
            Err(e) => {
                last_err = format!("failed to execute {exe}: {e}");
            }
        }
    }

    Err(anyhow!(last_err))
}

#[cfg(target_os = "windows")]
fn collect_raw_snapshot_windows() -> Result<RawSnapshot> {
    let timestamp = chrono::Utc::now().timestamp();
    let ps = r#"
$ErrorActionPreference = 'SilentlyContinue'
$os = Get-CimInstance Win32_OperatingSystem
$cpuAvg = (Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average
if ($null -eq $cpuAvg) { $cpuAvg = 0 }

$ifStats = [System.Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces() | ForEach-Object {
  try {
    if ($_.NetworkInterfaceType -eq [System.Net.NetworkInformation.NetworkInterfaceType]::Loopback) { return }
    if ($_.NetworkInterfaceType -eq [System.Net.NetworkInformation.NetworkInterfaceType]::Tunnel) { return }
    $s = $_.GetIPStatistics()
    [PSCustomObject]@{
      Name = $_.Name
      ReceivedBytes = [uint64]$s.BytesReceived
      SentBytes = [uint64]$s.BytesSent
    }
  } catch {
    $null
  }
} | Where-Object { $_ -ne $null }
$netRx = ($ifStats | Measure-Object -Property ReceivedBytes -Sum).Sum
$netTx = ($ifStats | Measure-Object -Property SentBytes -Sum).Sum
if ($null -eq $netRx) { $netRx = 0 }
if ($null -eq $netTx) { $netTx = 0 }

$diskReadBps = 0
$diskWriteBps = 0
$c = Get-Counter '\PhysicalDisk(_Total)\Disk Read Bytes/sec','\PhysicalDisk(_Total)\Disk Write Bytes/sec'
if ($c -and $c.CounterSamples.Count -ge 2) {
  $diskReadBps = [double]$c.CounterSamples[0].CookedValue
  $diskWriteBps = [double]$c.CounterSamples[1].CookedValue
}

$tcpEstablished = 0
$tcpTimeWait = 0
$tcpCloseWait = 0
if (Get-Command Get-NetTCPConnection -ErrorAction SilentlyContinue) {
  $tcpEstablished = (Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue | Measure-Object).Count
  $tcpTimeWait = (Get-NetTCPConnection -State TimeWait -ErrorAction SilentlyContinue | Measure-Object).Count
  $tcpCloseWait = (Get-NetTCPConnection -State CloseWait -ErrorAction SilentlyContinue | Measure-Object).Count
}

$procs = Get-Process -ErrorAction SilentlyContinue
$processCount = ($procs | Measure-Object).Count
$fdUsed = ($procs | Measure-Object -Property Handles -Sum).Sum
if ($null -eq $fdUsed) { $fdUsed = 0 }

$boot = [Management.ManagementDateTimeConverter]::ToDateTime($os.LastBootUpTime)
$uptime = (New-TimeSpan -Start $boot -End (Get-Date)).TotalSeconds

$obj = [ordered]@{
  cpu_usage_percent = [double]$cpuAvg
  load1 = [double]$cpuAvg
  load5 = [double]$cpuAvg
  load15 = [double]$cpuAvg
  mem_total_bytes = [uint64]$os.TotalVisibleMemorySize * 1024
  mem_available_bytes = [uint64]$os.FreePhysicalMemory * 1024
  swap_total_bytes = [uint64]$os.TotalVirtualMemorySize * 1024
  swap_free_bytes = [uint64]$os.FreeVirtualMemory * 1024
  net_rx_bytes = [uint64]$netRx
  net_tx_bytes = [uint64]$netTx
  disk_read_bps = [double]$diskReadBps
  disk_write_bps = [double]$diskWriteBps
  tcp_established = [int64]$tcpEstablished
  tcp_time_wait = [int64]$tcpTimeWait
  tcp_close_wait = [int64]$tcpCloseWait
  process_count = [int64]$processCount
  fd_used = [int64]$fdUsed
  fd_max = [int64]0
  procs_running = [int64]0
  procs_blocked = [int64]0
  context_switches = [int64]0
  processes_forked_total = [int64]0
  uptime_seconds = [double]$uptime
  interfaces = $ifStats
}
$obj | ConvertTo-Json -Compress -Depth 5
"#;

    let v = run_powershell_json(ps)?;

    let mut interfaces = Vec::new();
    if let Some(arr) = v.get("interfaces").and_then(Value::as_array) {
        for it in arr {
            let name = it
                .get("Name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }

            let lower = name.to_ascii_lowercase();
            if lower.contains("loopback") {
                continue;
            }

            interfaces.push(NetworkInterfaceStats {
                name,
                rx_bytes: to_i64_saturated(json_u64(it, "ReceivedBytes")),
                tx_bytes: to_i64_saturated(json_u64(it, "SentBytes")),
            });
        }
    }

    interfaces.sort_unstable_by(|a, b| a.name.cmp(&b.name));

    let disk_read_bps = json_f64(&v, "disk_read_bps").max(0.0);
    let disk_write_bps = json_f64(&v, "disk_write_bps").max(0.0);
    let (disk_read_bytes, disk_write_bytes) = {
        let mut acc = WINDOWS_DISK_ACCUM.write();
        let dt = if acc.2 > 0 {
            (timestamp - acc.2).max(1) as f64
        } else {
            current_sample_interval_secs() as f64
        };
        acc.0 = acc.0.saturating_add((disk_read_bps * dt) as u64);
        acc.1 = acc.1.saturating_add((disk_write_bps * dt) as u64);
        acc.2 = timestamp;
        (acc.0, acc.1)
    };

    Ok(RawSnapshot {
        timestamp,
        cpu_total: 0,
        cpu_idle: 0,
        cpu_usage_percent_hint: Some(json_f64(&v, "cpu_usage_percent")),
        load1: json_f64(&v, "load1"),
        load5: json_f64(&v, "load5"),
        load15: json_f64(&v, "load15"),
        mem_total_bytes: json_u64(&v, "mem_total_bytes"),
        mem_available_bytes: json_u64(&v, "mem_available_bytes"),
        swap_total_bytes: json_u64(&v, "swap_total_bytes"),
        swap_free_bytes: json_u64(&v, "swap_free_bytes"),
        net_rx_bytes: json_u64(&v, "net_rx_bytes"),
        net_tx_bytes: json_u64(&v, "net_tx_bytes"),
        interfaces,
        disk_read_bytes,
        disk_write_bytes,
        tcp_established: json_i64(&v, "tcp_established"),
        tcp_time_wait: json_i64(&v, "tcp_time_wait"),
        tcp_close_wait: json_i64(&v, "tcp_close_wait"),
        process_count: json_i64(&v, "process_count"),
        fd_used: json_u64(&v, "fd_used"),
        fd_max: json_u64(&v, "fd_max"),
        procs_running: json_i64(&v, "procs_running"),
        procs_blocked: json_i64(&v, "procs_blocked"),
        context_switches: json_u64(&v, "context_switches"),
        processes_forked_total: json_u64(&v, "processes_forked_total"),
        uptime_seconds: json_f64(&v, "uptime_seconds"),
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

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn collect_one_point() -> Result<(SystemMetricsPoint, Vec<NetworkInterfaceStats>)> {
    Err(anyhow!(
        "system metrics sampler is only supported on Linux and Windows"
    ))
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
async fn collect_and_publish_one(app: &AppHandle) {
    let collected = tauri::async_runtime::spawn_blocking(collect_one_point).await;
    if let Ok(Ok((point, interfaces))) = collected {
        *LAST_INTERFACES.write() = interfaces.clone();
        push_realtime_point(point.clone());
        try_enqueue_system_metrics(point.clone());

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
                let wants_persistence = crate::config::get_config()
                    .metrics_storage
                    .as_ref()
                    .map(|m| m.enabled)
                    .unwrap_or(false);
                let has_history_storage = crate::metrics::db_pool().is_some();
                if has_subscriber || wants_persistence || has_history_storage {
                    collect_and_publish_one(&app).await;
                }

                let wait_secs = if has_subscriber || wants_persistence || has_history_storage {
                    current_sample_interval_secs()
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
        });
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        let win = window_seconds
            .unwrap_or(24 * 3600)
            .clamp(MIN_SAMPLE_INTERVAL_SECS, MAX_REALTIME_WINDOW_SECS);

        Ok(SystemMetricsRealtimePayload {
            sample_interval_seconds: current_sample_interval_secs(),
            max_window_seconds: MAX_REALTIME_WINDOW_SECS,
            supported: true,
            message: None,
            latest: latest_point(),
            points: get_realtime_points(win),
            interfaces: LAST_INTERFACES.read().clone(),
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
        });
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        if req.end_time <= req.start_time {
            return Ok(QuerySystemMetricsResponse {
                points: vec![],
                supported: true,
                message: Some("end_time must be greater than start_time".to_string()),
            });
        }

        let Some(pool) = crate::metrics::db_pool() else {
            return Ok(QuerySystemMetricsResponse {
                points: vec![],
                supported: true,
                message: Some("metrics database is not initialized".to_string()),
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

        Ok(QuerySystemMetricsResponse {
            points: downsample_points(points, MAX_CHART_POINTS),
            supported: true,
            message: None,
        })
    }
}
