use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemMetricsSummary {
    pub points_count: i64,
    pub cpu_avg_percent: f64,
    pub cpu_peak_percent: f64,
    pub mem_avg_percent: f64,
    pub mem_peak_percent: f64,
    pub net_rx_peak_bps: f64,
    pub net_tx_peak_bps: f64,
    pub disk_read_peak_bps: f64,
    pub disk_write_peak_bps: f64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<SystemMetricsSummary>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<SystemMetricsSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SystemMetricsEventPayload {
    pub(super) point: SystemMetricsPoint,
    pub(super) interfaces: Vec<NetworkInterfaceStats>,
}

#[derive(Debug, Clone)]
pub(super) struct RawSnapshot {
    pub(super) timestamp: i64,
    pub(super) cpu_total: u64,
    pub(super) cpu_idle: u64,
    pub(super) cpu_usage_percent_hint: Option<f64>,

    pub(super) load1: f64,
    pub(super) load5: f64,
    pub(super) load15: f64,

    pub(super) mem_total_bytes: u64,
    pub(super) mem_available_bytes: u64,
    pub(super) swap_total_bytes: u64,
    pub(super) swap_free_bytes: u64,

    pub(super) net_rx_bytes: u64,
    pub(super) net_tx_bytes: u64,
    pub(super) interfaces: Vec<NetworkInterfaceStats>,

    pub(super) disk_read_bytes: u64,
    pub(super) disk_write_bytes: u64,

    pub(super) tcp_established: i64,
    pub(super) tcp_time_wait: i64,
    pub(super) tcp_close_wait: i64,

    pub(super) process_count: i64,
    pub(super) fd_used: u64,
    pub(super) fd_max: u64,

    pub(super) procs_running: i64,
    pub(super) procs_blocked: i64,
    pub(super) context_switches: u64,
    pub(super) processes_forked_total: u64,

    pub(super) uptime_seconds: f64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub(super) struct HistoricalRow {
    pub(super) bucket: i64,
    pub(super) cpu_usage_percent: Option<f64>,
    pub(super) load1: Option<f64>,
    pub(super) load5: Option<f64>,
    pub(super) load15: Option<f64>,
    pub(super) mem_total_bytes: Option<i64>,
    pub(super) mem_available_bytes: Option<f64>,
    pub(super) mem_used_bytes: Option<f64>,
    pub(super) mem_used_percent: Option<f64>,
    pub(super) swap_total_bytes: Option<i64>,
    pub(super) swap_free_bytes: Option<f64>,
    pub(super) swap_used_bytes: Option<f64>,
    pub(super) swap_used_percent: Option<f64>,
    pub(super) net_rx_bytes: Option<i64>,
    pub(super) net_tx_bytes: Option<i64>,
    pub(super) net_rx_bps: Option<f64>,
    pub(super) net_tx_bps: Option<f64>,
    pub(super) disk_read_bytes: Option<i64>,
    pub(super) disk_write_bytes: Option<i64>,
    pub(super) disk_read_bps: Option<f64>,
    pub(super) disk_write_bps: Option<f64>,
    pub(super) tcp_established: Option<f64>,
    pub(super) tcp_time_wait: Option<f64>,
    pub(super) tcp_close_wait: Option<f64>,
    pub(super) process_count: Option<f64>,
    pub(super) fd_used: Option<f64>,
    pub(super) fd_max: Option<i64>,
    pub(super) fd_usage_percent: Option<f64>,
    pub(super) procs_running: Option<f64>,
    pub(super) procs_blocked: Option<f64>,
    pub(super) context_switches: Option<i64>,
    pub(super) processes_forked_total: Option<i64>,
    pub(super) uptime_seconds: Option<f64>,
}
