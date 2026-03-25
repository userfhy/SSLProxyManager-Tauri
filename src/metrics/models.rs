use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BlacklistEntry {
    pub id: i64,
    pub ip: String,
    pub reason: Option<String>,
    pub expires_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMetricsRequest {
    pub start_time: i64,
    pub end_time: i64,
    pub listen_addr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMetricsResponse {
    pub series: MetricsSeries,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequestLogsRequest {
    pub start_time: i64,
    pub end_time: i64,
    pub listen_addr: Option<String>,
    pub upstream: Option<String>,
    pub request_path: Option<String>,
    pub client_ip: Option<String>,
    pub status_code: Option<i32>,
    pub page: i32,
    pub page_size: i32,
    pub matched_route_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequestLogsResponse {
    pub logs: Vec<RequestLog>,
    pub total: i64,
    pub total_page: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RequestLog {
    pub id: i64,
    pub timestamp: i64,
    pub listen_addr: String,
    pub client_ip: String,
    pub remote_ip: String,
    pub method: String,
    pub request_path: String,
    pub request_host: String,
    pub status_code: i32,
    pub upstream: String,
    pub latency_ms: f64,
    #[sqlx(default)]
    pub guard_ms: f64,
    #[sqlx(default)]
    pub prepare_ms: f64,
    #[sqlx(default)]
    pub upstream_ms: f64,
    pub user_agent: String,
    pub referer: String,
    #[sqlx(default)]
    pub matched_route_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStatsRequest {
    pub start_time: i64,
    pub end_time: i64,
    pub listen_addr: Option<String>,
    pub granularity_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Default)]
pub struct DashboardStatsPoint {
    pub time_bucket: i64,
    pub total_requests: i64,
    pub success_requests: i64,
    pub redirect_requests: i64,
    pub client_error_requests: i64,
    pub server_error_requests: i64,
    #[sqlx(default)]
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TopListItem {
    pub item: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardStatsResponse {
    pub time_series: Vec<DashboardStatsPoint>,
    pub top_paths: Vec<TopListItem>,
    pub top_ips: Vec<TopListItem>,
    pub top_routes: Vec<TopListItem>,
    pub top_route_errors: Vec<TopListItem>,
    #[serde(default)]
    pub top_upstream_errors: Vec<TopListItem>,
    pub total_requests: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone)]
pub struct RequestLogInsert {
    pub timestamp: i64,
    pub listen_addr: String,
    pub client_ip: String,
    pub remote_ip: String,
    pub method: String,
    pub request_path: String,
    pub request_host: String,
    pub status_code: i32,
    pub upstream: String,
    pub latency_ms: f64,
    pub guard_ms: f64,
    pub prepare_ms: f64,
    pub upstream_ms: f64,
    pub user_agent: String,
    pub referer: String,
    pub matched_route_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSeries {
    pub timestamps: Vec<i64>,
    pub counts: Vec<i64>,
    pub s2xx: Vec<i64>,
    pub s3xx: Vec<i64>,
    pub s4xx: Vec<i64>,
    pub s5xx: Vec<i64>,
    pub s0: Vec<i64>,
    #[serde(rename = "avgLatencyMs")]
    pub avg_latency_ms: Vec<f64>,
    #[serde(rename = "maxLatencyMs")]
    pub max_latency_ms: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p50: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p99: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "upstreamDist")]
    pub upstream_dist: Option<Vec<KeyValue>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topRouteErr")]
    pub top_route_err: Option<Vec<KeyValue>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topUpErr")]
    pub top_up_err: Option<Vec<KeyValue>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "latencyDist")]
    pub latency_dist: Option<Vec<KeyValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsPayload {
    #[serde(rename = "windowSeconds")]
    pub window_seconds: i32,
    #[serde(rename = "listenAddrs")]
    pub listen_addrs: Vec<String>,
    #[serde(rename = "byListenAddr")]
    pub by_listen_addr: HashMap<String, MetricsSeries>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "minuteWindowSeconds"
    )]
    pub minute_window_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "byListenMinute")]
    pub by_listen_minute: Option<HashMap<String, MetricsSeries>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topRoutes")]
    pub top_routes: Option<Vec<TopListItem>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topPaths")]
    pub top_paths: Option<Vec<TopListItem>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topClientIps")]
    pub top_client_ips: Option<Vec<TopListItem>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topUpstreamErrors")]
    pub top_upstream_errors: Option<Vec<TopListItem>>,
}
