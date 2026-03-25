use super::*;

pub(super) fn downsample_points(points: Vec<SystemMetricsPoint>, max_points: usize) -> Vec<SystemMetricsPoint> {
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

pub(super) fn push_realtime_point(point: SystemMetricsPoint) {
    let mut buf = REALTIME_POINTS.write();
    buf.push_back(point);
    while buf.len() > MAX_REALTIME_POINTS {
        let _ = buf.pop_front();
    }
}

pub(super) fn get_realtime_points(window_seconds: i64) -> Vec<SystemMetricsPoint> {
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

pub(super) fn latest_point() -> Option<SystemMetricsPoint> {
    REALTIME_POINTS.read().back().cloned()
}

pub(super) fn build_summary(points: &[SystemMetricsPoint]) -> Option<SystemMetricsSummary> {
    if points.is_empty() {
        return None;
    }

    let mut cpu_sum: f64 = 0.0;
    let mut mem_sum: f64 = 0.0;
    let mut cpu_peak: f64 = 0.0;
    let mut mem_peak: f64 = 0.0;
    let mut net_rx_peak: f64 = 0.0;
    let mut net_tx_peak: f64 = 0.0;
    let mut disk_read_peak: f64 = 0.0;
    let mut disk_write_peak: f64 = 0.0;

    for p in points {
        cpu_sum += p.cpu_usage_percent;
        mem_sum += p.mem_used_percent;
        cpu_peak = cpu_peak.max(p.cpu_usage_percent);
        mem_peak = mem_peak.max(p.mem_used_percent);
        net_rx_peak = net_rx_peak.max(p.net_rx_bps);
        net_tx_peak = net_tx_peak.max(p.net_tx_bps);
        disk_read_peak = disk_read_peak.max(p.disk_read_bps);
        disk_write_peak = disk_write_peak.max(p.disk_write_bps);
    }

    let count = points.len() as f64;
    Some(SystemMetricsSummary {
        points_count: points.len() as i64,
        cpu_avg_percent: cpu_sum / count,
        cpu_peak_percent: cpu_peak,
        mem_avg_percent: mem_sum / count,
        mem_peak_percent: mem_peak,
        net_rx_peak_bps: net_rx_peak,
        net_tx_peak_bps: net_tx_peak,
        disk_read_peak_bps: disk_read_peak,
        disk_write_peak_bps: disk_write_peak,
    })
}

