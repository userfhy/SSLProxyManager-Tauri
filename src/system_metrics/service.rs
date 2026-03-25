use super::realtime::{build_summary, get_realtime_points, latest_point};
use super::*;

pub(super) fn get_system_metrics_inner(
    window_seconds: Option<i64>,
) -> Result<SystemMetricsRealtimePayload> {
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
