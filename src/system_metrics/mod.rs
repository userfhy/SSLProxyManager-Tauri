#![cfg_attr(
    not(any(target_os = "linux", target_os = "windows")),
    allow(dead_code, unused_imports)
)]

#[cfg(target_os = "linux")]
use anyhow::Context;
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use sqlx::QueryBuilder;
#[cfg(target_os = "windows")]
use std::collections::HashMap;
use std::collections::VecDeque;
#[cfg(target_os = "windows")]
use std::ffi::c_void;
#[cfg(target_os = "windows")]
use std::mem::{size_of, zeroed};
#[cfg(target_os = "windows")]
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::Duration;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::time::Instant;
use tauri::AppHandle;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use tauri::{Emitter, Manager};
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

mod collect;
mod query;
mod realtime;
mod sampler;
mod state;
mod types;
mod writer;

use self::query::query_historical_system_metrics_inner;
use self::realtime::*;
use self::sampler::{start_system_sampler_inner, stop_system_sampler_inner};
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
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = window_seconds;
        return Ok(SystemMetricsRealtimePayload {
            sample_interval_seconds: current_sample_interval_secs(),
            max_window_seconds: MAX_REALTIME_WINDOW_SECS,
            supported: false,
            message: Some(
                "system metrics are currently only supported on Linux and Windows".to_string(),
            ),
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
    query_historical_system_metrics_inner(req).await
}
