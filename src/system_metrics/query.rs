use super::realtime::{build_summary, downsample_points};
use super::*;

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn map_historical_rows(rows: Vec<HistoricalRow>) -> Vec<SystemMetricsPoint> {
    rows.into_iter()
        .map(|r| SystemMetricsPoint {
            timestamp: r.bucket,
            cpu_usage_percent: r.cpu_usage_percent.unwrap_or(0.0),
            load1: r.load1.unwrap_or(0.0),
            load5: r.load5.unwrap_or(0.0),
            load15: r.load15.unwrap_or(0.0),
            mem_total_bytes: r.mem_total_bytes.unwrap_or(0),
            mem_available_bytes: round_to_i64(r.mem_available_bytes),
            mem_used_bytes: round_to_i64(r.mem_used_bytes),
            mem_used_percent: r.mem_used_percent.unwrap_or(0.0),
            swap_total_bytes: r.swap_total_bytes.unwrap_or(0),
            swap_free_bytes: round_to_i64(r.swap_free_bytes),
            swap_used_bytes: round_to_i64(r.swap_used_bytes),
            swap_used_percent: r.swap_used_percent.unwrap_or(0.0),
            net_rx_bytes: r.net_rx_bytes.unwrap_or(0),
            net_tx_bytes: r.net_tx_bytes.unwrap_or(0),
            net_rx_bps: r.net_rx_bps.unwrap_or(0.0),
            net_tx_bps: r.net_tx_bps.unwrap_or(0.0),
            disk_read_bytes: r.disk_read_bytes.unwrap_or(0),
            disk_write_bytes: r.disk_write_bytes.unwrap_or(0),
            disk_read_bps: r.disk_read_bps.unwrap_or(0.0),
            disk_write_bps: r.disk_write_bps.unwrap_or(0.0),
            tcp_established: round_to_i64(r.tcp_established),
            tcp_time_wait: round_to_i64(r.tcp_time_wait),
            tcp_close_wait: round_to_i64(r.tcp_close_wait),
            process_count: round_to_i64(r.process_count),
            fd_used: round_to_i64(r.fd_used),
            fd_max: r.fd_max.unwrap_or(0),
            fd_usage_percent: r.fd_usage_percent.unwrap_or(0.0),
            procs_running: round_to_i64(r.procs_running),
            procs_blocked: round_to_i64(r.procs_blocked),
            context_switches: r.context_switches.unwrap_or(0),
            processes_forked_total: r.processes_forked_total.unwrap_or(0),
            uptime_seconds: r.uptime_seconds.unwrap_or(0.0),
        })
        .collect::<Vec<_>>()
}

pub(super) async fn query_historical_system_metrics_inner(
    req: QuerySystemMetricsRequest,
) -> Result<QuerySystemMetricsResponse> {
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = req;
        return Ok(QuerySystemMetricsResponse {
            points: vec![],
            supported: false,
            message: Some(
                "system metrics historical query is only supported on Linux and Windows"
                    .to_string(),
            ),
            summary: None,
        });
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        if req.end_time <= req.start_time {
            return Ok(QuerySystemMetricsResponse {
                points: vec![],
                supported: true,
                message: Some("end_time must be greater than start_time".to_string()),
                summary: None,
            });
        }

        let Some(pool) = crate::metrics::db_pool() else {
            return Ok(QuerySystemMetricsResponse {
                points: vec![],
                supported: true,
                message: Some("metrics database is not initialized".to_string()),
                summary: None,
            });
        };

        let span = req.end_time - req.start_time;
        let granularity = choose_granularity(span, req.granularity_secs);

        let rows = sqlx::query_as::<_, HistoricalRow>(
            r#"
            SELECT
              (timestamp / ?) * ? AS bucket,
              AVG(cpu_usage_percent) AS cpu_usage_percent,
              AVG(load1) AS load1,
              AVG(load5) AS load5,
              AVG(load15) AS load15,
              MAX(mem_total_bytes) AS mem_total_bytes,
              AVG(mem_available_bytes) AS mem_available_bytes,
              AVG(mem_used_bytes) AS mem_used_bytes,
              AVG(mem_used_percent) AS mem_used_percent,
              MAX(swap_total_bytes) AS swap_total_bytes,
              AVG(swap_free_bytes) AS swap_free_bytes,
              AVG(swap_used_bytes) AS swap_used_bytes,
              AVG(swap_used_percent) AS swap_used_percent,
              MAX(net_rx_bytes) AS net_rx_bytes,
              MAX(net_tx_bytes) AS net_tx_bytes,
              AVG(net_rx_bps) AS net_rx_bps,
              AVG(net_tx_bps) AS net_tx_bps,
              MAX(disk_read_bytes) AS disk_read_bytes,
              MAX(disk_write_bytes) AS disk_write_bytes,
              AVG(disk_read_bps) AS disk_read_bps,
              AVG(disk_write_bps) AS disk_write_bps,
              AVG(tcp_established) AS tcp_established,
              AVG(tcp_time_wait) AS tcp_time_wait,
              AVG(tcp_close_wait) AS tcp_close_wait,
              AVG(process_count) AS process_count,
              AVG(fd_used) AS fd_used,
              MAX(fd_max) AS fd_max,
              AVG(fd_usage_percent) AS fd_usage_percent,
              AVG(procs_running) AS procs_running,
              AVG(procs_blocked) AS procs_blocked,
              MAX(context_switches) AS context_switches,
              MAX(processes_forked_total) AS processes_forked_total,
              AVG(uptime_seconds) AS uptime_seconds
            FROM system_metrics
            WHERE timestamp >= ? AND timestamp <= ?
            GROUP BY bucket
            ORDER BY bucket ASC
            "#,
        )
        .bind(granularity)
        .bind(granularity)
        .bind(req.start_time)
        .bind(req.end_time)
        .fetch_all(&*pool)
        .await?;

        let points = map_historical_rows(rows);
        let summary = build_summary(&points);
        Ok(QuerySystemMetricsResponse {
            points: downsample_points(points, MAX_CHART_POINTS),
            supported: true,
            message: None,
            summary,
        })
    }
}
