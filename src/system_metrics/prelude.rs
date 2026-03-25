#[cfg(target_os = "linux")]
pub(super) use anyhow::Context;
pub(super) use anyhow::{anyhow, Result};
pub(super) use once_cell::sync::Lazy;
pub(super) use parking_lot::RwLock;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) use sqlx::QueryBuilder;
#[cfg(target_os = "windows")]
pub(super) use std::collections::HashMap;
pub(super) use std::collections::VecDeque;
#[cfg(target_os = "windows")]
pub(super) use std::ffi::c_void;
#[cfg(target_os = "windows")]
pub(super) use std::mem::{size_of, zeroed};
#[cfg(target_os = "windows")]
pub(super) use std::ptr::null_mut;
pub(super) use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
pub(super) use std::time::Duration;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) use std::time::Instant;
pub(super) use tauri::AppHandle;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) use tauri::{Emitter, Manager};
#[cfg(target_os = "windows")]
pub(super) use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, FILETIME};
#[cfg(target_os = "windows")]
pub(super) use windows_sys::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, GetTcp6Table2, GetTcpTable2, IF_TYPE_SOFTWARE_LOOPBACK,
    IF_TYPE_TUNNEL, MIB_IF_TABLE2, MIB_TCP6TABLE2, MIB_TCPTABLE2, MIB_TCP_STATE_CLOSE_WAIT,
    MIB_TCP_STATE_ESTAB, MIB_TCP_STATE_TIME_WAIT,
};
#[cfg(target_os = "windows")]
pub(super) use windows_sys::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
};
#[cfg(target_os = "windows")]
pub(super) use windows_sys::Win32::System::ProcessStatus::{
    GetPerformanceInfo, PERFORMANCE_INFORMATION,
};
#[cfg(target_os = "windows")]
pub(super) use windows_sys::Win32::System::SystemInformation::{
    GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
#[cfg(target_os = "windows")]
pub(super) use windows_sys::Win32::System::Threading::GetSystemTimes;
