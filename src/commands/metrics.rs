use crate::cache_optimizer;
use crate::metrics;
use crate::system_metrics;

#[tauri::command]
pub fn get_metrics() -> Result<metrics::MetricsPayload, String> {
    Ok(metrics::get_metrics())
}

#[tauri::command]
pub fn get_system_metrics(
    window_seconds: Option<i64>,
) -> Result<system_metrics::SystemMetricsRealtimePayload, String> {
    system_metrics::get_system_metrics(window_seconds).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_system_metrics_subscription(active: bool) -> Result<(), String> {
    system_metrics::set_system_metrics_subscription(active);
    Ok(())
}

#[tauri::command]
pub async fn query_historical_system_metrics(
    req: system_metrics::QuerySystemMetricsRequest,
) -> Result<system_metrics::QuerySystemMetricsResponse, String> {
    system_metrics::query_historical_system_metrics(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_listen_addrs() -> Result<Vec<String>, String> {
    metrics::get_distinct_listen_addrs()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_historical_metrics(
    req: metrics::QueryMetricsRequest,
) -> Result<metrics::QueryMetricsResponse, String> {
    metrics::query_historical_metrics(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_dashboard_stats(
    req: metrics::DashboardStatsRequest,
) -> Result<metrics::DashboardStatsResponse, String> {
    metrics::get_dashboard_stats(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_request_logs(
    req: metrics::QueryRequestLogsRequest,
) -> Result<metrics::QueryRequestLogsResponse, String> {
    metrics::query_request_logs(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_blacklist_entry(
    ip: String,
    reason: String,
    duration_seconds: i32,
) -> Result<metrics::BlacklistEntry, String> {
    metrics::add_blacklist_entry(ip, reason, duration_seconds)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_blacklist_entry(ip: String) -> Result<(), String> {
    metrics::remove_blacklist_entry(ip)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_blacklist_entries() -> Result<Vec<metrics::BlacklistEntry>, String> {
    metrics::get_blacklist_entries()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_blacklist_cache() -> Result<(), String> {
    metrics::refresh_blacklist_cache()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_metrics_db_status() -> Result<metrics::MetricsDBStatus, String> {
    Ok(metrics::get_metrics_db_status())
}

#[tauri::command]
pub async fn get_metrics_db_status_detail() -> Result<metrics::MetricsDBStatus, String> {
    metrics::get_metrics_db_status_detail()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_metrics_db_connection(db_path: String) -> Result<(bool, String), String> {
    metrics::test_metrics_db_connection(db_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_buffer_pool_stats() -> Result<serde_json::Value, String> {
    let stats = crate::buffer_pool::pool_stats();
    Ok(serde_json::json!({
        "size": stats.size,
        "max_size": stats.max_size,
        "usage_percent": (stats.size as f64 / stats.max_size as f64 * 100.0).round()
    }))
}

#[tauri::command]
pub fn get_cache_stats() -> Result<serde_json::Value, String> {
    let manager = cache_optimizer::global_cache_manager();
    let stats = manager.all_stats();

    Ok(serde_json::json!({
        "regex": {
            "size": stats.regex.size,
            "capacity": stats.regex.capacity,
            "usage_percent": stats.regex.usage_percent().round()
        },
        "dns": {
            "size": stats.dns.size,
            "capacity": stats.dns.capacity,
            "usage_percent": stats.dns.usage_percent().round()
        }
    }))
}

#[tauri::command]
pub fn clear_all_caches() -> Result<(), String> {
    let manager = cache_optimizer::global_cache_manager();
    manager.clear_all();
    Ok(())
}
