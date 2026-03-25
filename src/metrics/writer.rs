use super::db::REQUEST_LOG_TX;
use super::*;

pub async fn init_request_log_writer() {
    if REQUEST_LOG_TX.read().is_some() {
        return;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<RequestLogInsert>(50_000);
    *REQUEST_LOG_TX.write() = Some(tx);

    tauri::async_runtime::spawn(async move {
        let mut buf: Vec<RequestLogInsert> = Vec::with_capacity(DB_FLUSH_BATCH_SIZE);
        let mut last_flush = Instant::now();
        let mut last_cleanup = Instant::now();
        let mut last_retention_check = Instant::now();

        loop {
            tokio::select! {
                Some(item) = rx.recv() => {
                    buf.push(item);
                    if buf.len() >= DB_FLUSH_BATCH_SIZE {
                        flush_request_logs(&mut buf).await;
                        last_flush = Instant::now();
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(200)) => {
                    if !buf.is_empty() && last_flush.elapsed() >= DB_FLUSH_INTERVAL {
                        flush_request_logs(&mut buf).await;
                        last_flush = Instant::now();
                    }
                }
            }

            // 在后台线程做黑名单清理
            if last_cleanup.elapsed().as_secs() > 10 {
                // 修复点：先获取 Option<Arc>，不要持有 ReadLockGuard 过 await
                let pool_opt = db_pool();
                if let Some(pool) = pool_opt {
                    let _ = super::db::refresh_blacklist_cache_internal(&pool).await;
                }
                last_cleanup = Instant::now();
            }

            // request_logs 日志保留：每天检查一次
            if last_retention_check.elapsed() >= REQUEST_LOG_RETENTION_CHECK_INTERVAL {
                let pool_opt = db_pool();
                if let Some(pool) = pool_opt {
                    let cutoff =
                        chrono::Utc::now().timestamp() - REQUEST_LOG_RETENTION_DAYS * 24 * 60 * 60;
                    let deleted_rows = sqlx::query("DELETE FROM request_logs WHERE timestamp < ?")
                        .bind(cutoff)
                        .execute(&*pool)
                        .await
                        .map(|r| r.rows_affected())
                        .unwrap_or(0);

                    reclaim_db_space_after_delete(&pool, deleted_rows).await;
                }
                last_retention_check = Instant::now();
            }
        }
    });
}

pub fn try_enqueue_request_log(log: RequestLogInsert) {
    let la = log.listen_addr.trim();
    let shard_key = if la.is_empty() { "全局" } else { la };
    let idx = (hash_fnv1a_64(shard_key) as usize) % REALTIME_SHARDS;

    {
        let mut agg = REALTIME_AGG_SHARDS[idx].write();
        agg.add(
            &log.listen_addr,
            log.timestamp,
            log.status_code,
            log.latency_ms,
            &log.matched_route_id,
            &log.request_path,
            &log.client_ip,
            &log.upstream,
        );
    }

    if let Some(tx) = REQUEST_LOG_TX.read().as_ref() {
        let _ = tx.try_send(log);
    }
}

async fn flush_request_logs(buf: &mut Vec<RequestLogInsert>) {
    let Some(pool) = db_pool() else {
        buf.clear();
        return;
    };
    if buf.is_empty() {
        return;
    }

    // 使用 QueryBuilder 进行批量插入
    const CHUNK_SIZE: usize = 500;

    for chunk in buf.chunks(CHUNK_SIZE) {
        let mut query_builder = QueryBuilder::new(
            "INSERT INTO request_logs (timestamp, listen_addr, client_ip, remote_ip, method, request_path, request_host, status_code, upstream, latency_ms, guard_ms, prepare_ms, upstream_ms, user_agent, referer, matched_route_id) "
        );

        query_builder.push_values(chunk, |mut b, it| {
            b.push_bind(it.timestamp)
                .push_bind(&it.listen_addr)
                .push_bind(&it.client_ip)
                .push_bind(&it.remote_ip)
                .push_bind(&it.method)
                .push_bind(&it.request_path)
                .push_bind(&it.request_host)
                .push_bind(it.status_code)
                .push_bind(&it.upstream)
                .push_bind(it.latency_ms)
                .push_bind(it.guard_ms)
                .push_bind(it.prepare_ms)
                .push_bind(it.upstream_ms)
                .push_bind(&it.user_agent)
                .push_bind(&it.referer)
                .push_bind(&it.matched_route_id);
        });

        let query = query_builder.build();
        if let Err(e) = query.execute(&*pool).await {
            eprintln!("Bulk insert request logs failed: {}", e);
        }
    }

    buf.clear();
}
