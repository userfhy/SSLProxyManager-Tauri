use super::*;

pub(super) const DEFAULT_SAMPLE_INTERVAL_SECS: i64 = 10;
pub(super) const MIN_SAMPLE_INTERVAL_SECS: i64 = 1;
pub(super) const MAX_SAMPLE_INTERVAL_SECS: i64 = 300;
pub(super) const IDLE_PAUSE_INTERVAL_SECS: i64 = 60;
#[cfg(target_os = "windows")]
pub(super) const WINDOWS_MIN_EFFECTIVE_SAMPLE_INTERVAL_SECS: i64 = 3;
pub(super) const MAX_REALTIME_WINDOW_SECS: i64 = 2 * 24 * 3600; // 2 天
pub(super) const MAX_REALTIME_POINTS: usize = (MAX_REALTIME_WINDOW_SECS / MIN_SAMPLE_INTERVAL_SECS + 8) as usize;
pub(super) const MAX_CHART_POINTS: usize = 1200;

pub(super) const DB_FLUSH_BATCH_SIZE: usize = 800;
pub(super) const DB_FLUSH_INTERVAL: Duration = Duration::from_secs(5);
pub(super) const SYSTEM_METRICS_RETENTION_DAYS: i64 = 360;
pub(super) const SYSTEM_METRICS_RETENTION_CHECK_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);

pub(super) static SAMPLER_RUNNING: AtomicBool = AtomicBool::new(false);
pub(super) static SAMPLER_HANDLE: Lazy<RwLock<Option<tauri::async_runtime::JoinHandle<()>>>> =
    Lazy::new(|| RwLock::new(None));
pub(super) static SAMPLER_WAKE: Lazy<tokio::sync::Notify> = Lazy::new(tokio::sync::Notify::new);
pub(super) static SAMPLE_INTERVAL_SECS: AtomicI64 = AtomicI64::new(DEFAULT_SAMPLE_INTERVAL_SECS);
pub(super) static HAS_ACTIVE_SUBSCRIBER: AtomicBool = AtomicBool::new(false);

pub(super) static SYSTEM_METRICS_TX: Lazy<RwLock<Option<tokio::sync::mpsc::Sender<SystemMetricsPoint>>>> =
    Lazy::new(|| RwLock::new(None));

pub(super) static LAST_RAW: Lazy<RwLock<Option<RawSnapshot>>> = Lazy::new(|| RwLock::new(None));
pub(super) static LAST_INTERFACES: Lazy<RwLock<Vec<NetworkInterfaceStats>>> = Lazy::new(|| RwLock::new(Vec::new()));
pub(super) static REALTIME_POINTS: Lazy<RwLock<VecDeque<SystemMetricsPoint>>> =
    Lazy::new(|| RwLock::new(VecDeque::with_capacity(MAX_REALTIME_POINTS)));
#[cfg(target_os = "windows")]
pub(super) static WINDOWS_DISK_ACCUM: Lazy<RwLock<(u64, u64, i64)>> = Lazy::new(|| RwLock::new((0, 0, 0)));
#[cfg(target_os = "windows")]
pub(super) static WINDOWS_LOAD_AVG: Lazy<RwLock<WindowsLoadAvgState>> =
    Lazy::new(|| RwLock::new(WindowsLoadAvgState::default()));
#[cfg(target_os = "windows")]
pub(super) static WINDOWS_PDH: Lazy<RwLock<Option<WindowsPdhState>>> = Lazy::new(|| RwLock::new(None));

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
pub(super) struct WindowsLoadAvgState {
    initialized: bool,
    last_ts: i64,
    load1: f64,
    load5: f64,
    load15: f64,
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(super) struct WindowsPdhState {
    query: usize,
    read_counter: usize,
    write_counter: usize,
}

pub(super) fn to_i64_saturated(v: u64) -> i64 {
    if v > i64::MAX as u64 {
        i64::MAX
    } else {
        v as i64
    }
}

#[inline]
pub(super) fn round_to_i64(v: Option<f64>) -> i64 {
    v.unwrap_or(0.0).round() as i64
}

#[inline]
pub(super) fn normalize_sample_interval_secs(v: i64) -> i64 {
    v.clamp(MIN_SAMPLE_INTERVAL_SECS, MAX_SAMPLE_INTERVAL_SECS)
}

#[inline]
pub(super) fn current_sample_interval_secs() -> i64 {
    normalize_sample_interval_secs(SAMPLE_INTERVAL_SECS.load(Ordering::Relaxed))
}

#[inline]
pub(super) fn effective_sample_interval_secs() -> i64 {
    let configured = current_sample_interval_secs();
    #[cfg(target_os = "windows")]
    {
        return configured.max(WINDOWS_MIN_EFFECTIVE_SAMPLE_INTERVAL_SECS);
    }
    #[cfg(not(target_os = "windows"))]
    {
        configured
    }
}

#[inline]
pub(super) fn is_system_metrics_persistence_enabled(cfg: &crate::config::Config) -> bool {
    let global_enabled = cfg
        .metrics_storage
        .as_ref()
        .map(|m| m.enabled)
        .unwrap_or(false);
    global_enabled && cfg.system_metrics_persistence_enabled
}

#[inline]
pub(super) fn refresh_sample_interval_from_config_inner() {
    let cfg = crate::config::get_config();
    let interval = normalize_sample_interval_secs(cfg.system_metrics_sample_interval_secs);
    SAMPLE_INTERVAL_SECS.store(interval, Ordering::Relaxed);
}

pub(super) fn refresh_sample_interval_from_config_state() {
    refresh_sample_interval_from_config_inner();
    SAMPLER_WAKE.notify_waiters();
}

pub(super) fn set_system_metrics_subscription_state(active: bool) {
    HAS_ACTIVE_SUBSCRIBER.store(active, Ordering::Relaxed);
    SAMPLER_WAKE.notify_waiters();
}

#[inline]
pub(super) fn choose_granularity(span: i64, requested: Option<i64>) -> i64 {
    if let Some(v) = requested {
        return v.max(1);
    }
    if span <= 6 * 3600 {
        effective_sample_interval_secs()
    } else if span <= 3 * 24 * 3600 {
        60
    } else if span <= 14 * 24 * 3600 {
        300
    } else {
        900
    }
}

