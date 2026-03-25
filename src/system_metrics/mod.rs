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

pub async fn query_historical_system_metrics(
    req: QuerySystemMetricsRequest,
) -> Result<QuerySystemMetricsResponse> {
    query_historical_system_metrics_inner(req).await
}
