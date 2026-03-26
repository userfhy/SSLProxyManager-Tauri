#![cfg_attr(
    not(any(target_os = "linux", target_os = "windows")),
    allow(dead_code, unused_imports)
)]

use anyhow::Result;
use tauri::AppHandle;

mod collect;
mod prelude;
mod query;
mod realtime;
mod sampler;
mod service;
mod state;
mod types;
mod writer;

use self::query::query_historical_system_metrics_inner;
use self::sampler::{start_system_sampler_inner, stop_system_sampler_inner};
use self::service::get_system_metrics_inner;
use self::state::*;
use self::writer::{init_system_metrics_writer, try_enqueue_system_metrics};
pub use types::*;

pub fn refresh_sample_interval_from_config() {
    state::refresh_sample_interval_from_config_state();
}

pub fn set_system_metrics_subscription(active: bool) {
    state::set_system_metrics_subscription_state(active);
}

pub fn start_system_sampler(app: AppHandle) {
    start_system_sampler_inner(app);
}

pub fn stop_system_sampler() {
    stop_system_sampler_inner();
}

pub fn get_system_metrics(window_seconds: Option<i64>) -> Result<SystemMetricsRealtimePayload> {
    get_system_metrics_inner(window_seconds)
}

pub async fn collect_current_system_metrics() -> Result<(SystemMetricsPoint, Vec<NetworkInterfaceStats>)> {
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("system metrics are currently only supported on Linux and Windows");
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        tauri::async_runtime::spawn_blocking(self::collect::collect_one_point)
            .await
            .map_err(|e| anyhow::anyhow!("failed to join system metrics collection task: {e}"))?
    }
}

pub async fn query_historical_system_metrics(
    req: QuerySystemMetricsRequest,
) -> Result<QuerySystemMetricsResponse> {
    query_historical_system_metrics_inner(req).await
}
