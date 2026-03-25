use super::prelude::*;
use super::*;

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) fn init_system_metrics_writer() {
    if SYSTEM_METRICS_TX.read().is_some() {
        return;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<SystemMetricsPoint>(20_000);
    *SYSTEM_METRICS_TX.write() = Some(tx);

    tauri::async_runtime::spawn(async move {
        let mut buf: Vec<SystemMetricsPoint> = Vec::with_capacity(DB_FLUSH_BATCH_SIZE);
        let mut last_flush = Instant::now();
        let mut last_retention_check = Instant::now();

        loop {
            tokio::select! {
                Some(item) = rx.recv() => {
                    buf.push(item);
                    if buf.len() >= DB_FLUSH_BATCH_SIZE {
                        flush_system_metrics(&mut buf).await;
                        last_flush = Instant::now();
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(200)) => {
                    if !buf.is_empty() && last_flush.elapsed() >= DB_FLUSH_INTERVAL {
                        flush_system_metrics(&mut buf).await;
                        last_flush = Instant::now();
                    }
                }
            }

            if last_retention_check.elapsed() >= SYSTEM_METRICS_RETENTION_CHECK_INTERVAL {
                if let Some(pool) = crate::metrics::db_pool() {
                    let cutoff = chrono::Utc::now().timestamp()
                        - SYSTEM_METRICS_RETENTION_DAYS * 24 * 60 * 60;
                    let deleted_rows =
                        sqlx::query("DELETE FROM system_metrics WHERE timestamp < ?")
                            .bind(cutoff)
                            .execute(&*pool)
                            .await
                            .map(|r| r.rows_affected())
                            .unwrap_or(0);
                    crate::metrics::reclaim_db_space_after_delete(&pool, deleted_rows).await;
                }
                last_retention_check = Instant::now();
            }
        }
    });
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) fn try_enqueue_system_metrics(point: SystemMetricsPoint) {
    if let Some(tx) = SYSTEM_METRICS_TX.read().as_ref() {
        let _ = tx.try_send(point);
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
async fn flush_system_metrics(buf: &mut Vec<SystemMetricsPoint>) {
    let Some(pool) = crate::metrics::db_pool() else {
        buf.clear();
        return;
    };

    if buf.is_empty() {
        return;
    }

    const CHUNK_SIZE: usize = 300;

    for chunk in buf.chunks(CHUNK_SIZE) {
        let mut qb = QueryBuilder::new(
            "INSERT INTO system_metrics (timestamp, cpu_usage_percent, load1, load5, load15, mem_total_bytes, mem_available_bytes, mem_used_bytes, mem_used_percent, swap_total_bytes, swap_free_bytes, swap_used_bytes, swap_used_percent, net_rx_bytes, net_tx_bytes, net_rx_bps, net_tx_bps, disk_read_bytes, disk_write_bytes, disk_read_bps, disk_write_bps, tcp_established, tcp_time_wait, tcp_close_wait, process_count, fd_used, fd_max, fd_usage_percent, procs_running, procs_blocked, context_switches, processes_forked_total, uptime_seconds) "
        );

        qb.push_values(chunk, |mut b, it| {
            b.push_bind(it.timestamp)
                .push_bind(it.cpu_usage_percent)
                .push_bind(it.load1)
                .push_bind(it.load5)
                .push_bind(it.load15)
                .push_bind(it.mem_total_bytes)
                .push_bind(it.mem_available_bytes)
                .push_bind(it.mem_used_bytes)
                .push_bind(it.mem_used_percent)
                .push_bind(it.swap_total_bytes)
                .push_bind(it.swap_free_bytes)
                .push_bind(it.swap_used_bytes)
                .push_bind(it.swap_used_percent)
                .push_bind(it.net_rx_bytes)
                .push_bind(it.net_tx_bytes)
                .push_bind(it.net_rx_bps)
                .push_bind(it.net_tx_bps)
                .push_bind(it.disk_read_bytes)
                .push_bind(it.disk_write_bytes)
                .push_bind(it.disk_read_bps)
                .push_bind(it.disk_write_bps)
                .push_bind(it.tcp_established)
                .push_bind(it.tcp_time_wait)
                .push_bind(it.tcp_close_wait)
                .push_bind(it.process_count)
                .push_bind(it.fd_used)
                .push_bind(it.fd_max)
                .push_bind(it.fd_usage_percent)
                .push_bind(it.procs_running)
                .push_bind(it.procs_blocked)
                .push_bind(it.context_switches)
                .push_bind(it.processes_forked_total)
                .push_bind(it.uptime_seconds);
        });

        let _ = qb.build().execute(&*pool).await;
    }

    buf.clear();
}
