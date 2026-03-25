use super::*;

pub async fn query_request_logs(req: QueryRequestLogsRequest) -> Result<QueryRequestLogsResponse> {
    let Some(pool) = db_pool() else {
        return Ok(QueryRequestLogsResponse { logs: vec![], total: 0, total_page: 0 });
    };

    let page_size = req.page_size.clamp(1, 200) as i64;
    let page = req.page.max(1) as i64;
    let offset = (page - 1) * page_size;

    let listen_addr = req.listen_addr.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let upstream = req.upstream.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let request_path = req.request_path.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let client_ip = req.client_ip.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let status_code = req.status_code.filter(|c| *c > 0);
    let matched_route_id = req
        .matched_route_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let sort_by = req.sort_by.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let sort_order = req
        .sort_order
        .as_deref()
        .map(str::trim)
        .map(|s| s.to_ascii_lowercase());

    let sort_column = match sort_by {
        Some("id") => "id",
        Some("timestamp") => "timestamp",
        Some("listen_addr") | Some("listenAddr") => "listen_addr",
        Some("client_ip") | Some("clientIP") => "client_ip",
        Some("remote_ip") | Some("remoteIP") => "remote_ip",
        Some("method") => "method",
        Some("request_path") | Some("requestPath") => "request_path",
        Some("request_host") | Some("requestHost") => "request_host",
        Some("status_code") | Some("statusCode") => "status_code",
        Some("upstream") => "upstream",
        Some("latency_ms") | Some("latencyMs") => "latency_ms",
        Some("user_agent") | Some("userAgent") => "user_agent",
        Some("referer") => "referer",
        _ => "timestamp",
    };
    let sort_dir = match sort_order.as_deref() {
        Some("asc") | Some("ascending") | Some("ascend") => "ASC",
        _ => "DESC",
    };
    let filters = RequestLogQueryFilters {
        start_time: req.start_time,
        end_time: req.end_time,
        listen_addr,
        upstream,
        request_path,
        client_ip,
        status_code,
        matched_route_id,
    };

    // COUNT
    let mut count_qb = QueryBuilder::new("SELECT COUNT(1) FROM request_logs");
    append_request_logs_where(&mut count_qb, filters);

    let total: i64 = count_qb.build_query_as::<(i64,)>().fetch_one(&*pool).await?.0;
    let total_page = if total == 0 { 0 } else { (total + page_size - 1) / page_size };

    // SELECT
    let mut sel_qb = QueryBuilder::new(
        "SELECT id, timestamp, listen_addr, client_ip, remote_ip, method, request_path, request_host, status_code, upstream, latency_ms, user_agent, referer, matched_route_id FROM request_logs"
    );
    append_request_logs_where(&mut sel_qb, filters);

    sel_qb
        .push(" ORDER BY ")
        .push(sort_column)
        .push(" ")
        .push(sort_dir)
        .push(", id DESC LIMIT ")
        .push_bind(page_size)
        .push(" OFFSET ")
        .push_bind(offset);

    let logs = sel_qb.build_query_as::<RequestLog>().fetch_all(&*pool).await?;

    Ok(QueryRequestLogsResponse { logs, total, total_page })
}

pub fn get_metrics() -> MetricsPayload {
    // 500ms 缓存
    {
        let cache = METRICS_CACHE.read();
        if let Some((ts, payload)) = cache.as_ref() {
            if ts.elapsed() < METRICS_CACHE_TTL {
                return payload.clone();
            }
        }
    }

    let mut merged = RealtimeAgg::new();

    for shard in REALTIME_AGG_SHARDS.iter() {
        let guard = shard.read();

        merge_rt_series_map(&mut merged.per_sec, &guard.per_sec);
        merge_rt_series_map(&mut merged.per_min, &guard.per_min);
        merge_count_map(&mut merged.route_counts, &guard.route_counts);
        merge_count_map(&mut merged.path_counts, &guard.path_counts);
        merge_count_map(&mut merged.ip_counts, &guard.ip_counts);
        merge_count_map(&mut merged.upstream_error_counts, &guard.upstream_error_counts);
        merge_count_map(&mut merged.upstream_counts, &guard.upstream_counts);
    }

    let payload = merged.to_payload();
    {
        let mut cache = METRICS_CACHE.write();
        *cache = Some((Instant::now(), payload.clone()));
    }
    payload
}

pub async fn get_distinct_listen_addrs() -> Result<Vec<String>> {
    let Some(pool) = db_pool() else { return Ok(vec![]) };

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT listen_addr FROM request_logs WHERE trim(listen_addr) != '' ORDER BY listen_addr ASC",
    )
    .fetch_all(&*pool)
    .await
    .context("查询 request_logs.listen_addr distinct 失败")?;

    Ok(rows.into_iter().map(|(s,)| s).collect())
}

pub async fn query_historical_metrics(req: QueryMetricsRequest) -> Result<QueryMetricsResponse> {
    let Some(pool) = db_pool() else {
        return Ok(QueryMetricsResponse {
            series: MetricsSeries {
                timestamps: vec![], counts: vec![], s2xx: vec![], s3xx: vec![], s4xx: vec![], s5xx: vec![], s0: vec![],
                avg_latency_ms: vec![], max_latency_ms: vec![],
                p50: Some(vec![]), p95: Some(vec![]), p99: Some(vec![]),
                upstream_dist: Some(vec![]), top_route_err: Some(vec![]), top_up_err: Some(vec![]), latency_dist: Some(vec![]),
            },
        });
    };

    let listen_addr = req.listen_addr.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let start = req.start_time;
    let end = req.end_time;
    if end <= start {
        return Ok(QueryMetricsResponse {
            series: MetricsSeries {
                timestamps: vec![], counts: vec![], s2xx: vec![], s3xx: vec![], s4xx: vec![], s5xx: vec![], s0: vec![],
                avg_latency_ms: vec![], max_latency_ms: vec![],
                p50: Some(vec![]),
                p95: Some(vec![]), 
                p99: Some(vec![]),
                upstream_dist: Some(vec![]), top_route_err: Some(vec![]), top_up_err: Some(vec![]), latency_dist: Some(vec![]),
            },
        });
    }

    let span = end - start;
    let granularity = if span < 3600 { 1 } else if span < 48 * 3600 { 60 } else { 300 };

    // 聚合时序
    let mut qb = QueryBuilder::new("SELECT (timestamp / ");
    qb.push_bind(granularity);
    qb.push(") * ");
    qb.push_bind(granularity);
    qb.push(r#" AS bucket, 
        COUNT(1) AS total,
        SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS s2xx,
        SUM(CASE WHEN status_code BETWEEN 300 AND 399 THEN 1 ELSE 0 END) AS s3xx,
        SUM(CASE WHEN status_code BETWEEN 400 AND 499 THEN 1 ELSE 0 END) AS s4xx,
        SUM(CASE WHEN status_code >= 500 THEN 1 ELSE 0 END) AS s5xx,
        AVG(latency_ms) AS avg_latency,
        MAX(latency_ms) AS max_latency
    FROM request_logs
    WHERE timestamp >= "#);
    qb.push_bind(start);
    qb.push(" AND timestamp <= ");
    qb.push_bind(end);

    if let Some(v) = listen_addr {
        qb.push(" AND listen_addr = ").push_bind(v);
    }
    qb.push(" GROUP BY bucket ORDER BY bucket");

    let rows: Vec<(i64, i64, i64, i64, i64, i64, Option<f64>, Option<f64>)> = qb.build_query_as().fetch_all(&*pool).await?;

    let cap = rows.len();
    let mut timestamps = Vec::with_capacity(cap);
    let mut counts = Vec::with_capacity(cap);
    let mut s2xx = Vec::with_capacity(cap);
    let mut s3xx = Vec::with_capacity(cap);
    let mut s4xx = Vec::with_capacity(cap);
    let mut s5xx = Vec::with_capacity(cap);
    let mut avg_latency = Vec::with_capacity(cap);
    let mut max_latency = Vec::with_capacity(cap);

    for (bucket, total, v2, v3, v4, v5, avg_l, max_l) in rows {
        timestamps.push(bucket);
        counts.push(total);
        s2xx.push(v2);
        s3xx.push(v3);
        s4xx.push(v4);
        s5xx.push(v5);
        avg_latency.push(((avg_l.unwrap_or(0.0) * 10000.0).round()) / 10000.0);
        max_latency.push(((max_l.unwrap_or(0.0) * 10000.0).round()) / 10000.0);
    }

    // Top upstream 分布
    let mut up_qb = QueryBuilder::new(
        r#"SELECT CASE WHEN instr(h, '/') > 0 THEN substr(h, 1, instr(h, '/') - 1) ELSE h END AS k, COUNT(1) AS c FROM (
            SELECT replace(replace(replace(upstream, 'https://', ''), 'http://', ''), 'www.', '') AS h
            FROM request_logs WHERE timestamp >= "#
    );
    up_qb.push_bind(start).push(" AND timestamp <= ").push_bind(end);
    if let Some(v) = listen_addr {
        up_qb.push(" AND listen_addr = ").push_bind(v);
    }
    up_qb.push(") AS t GROUP BY k ORDER BY c DESC LIMIT 20");

    let upstream_dist: Vec<KeyValue> = up_qb.build_query_as::<(String, i64)>().fetch_all(&*pool).await?
        .into_iter().map(|(k, v)| KeyValue { key: k, value: v }).collect();

    // Top route 错误
    let mut re_qb = QueryBuilder::new("SELECT request_path AS k, COUNT(1) AS c FROM request_logs WHERE timestamp >= ");
    re_qb.push_bind(start).push(" AND timestamp <= ").push_bind(end).push(" AND status_code >= 400");
    if let Some(v) = listen_addr {
        re_qb.push(" AND listen_addr = ").push_bind(v);
    }
    re_qb.push(" GROUP BY request_path ORDER BY c DESC LIMIT 10");
    let top_route_err: Vec<KeyValue> = re_qb.build_query_as::<(String, i64)>().fetch_all(&*pool).await?
        .into_iter().map(|(k, v)| KeyValue { key: k, value: v }).collect();

    // Top upstream 错误
    let mut ue_qb = QueryBuilder::new("SELECT upstream AS k, COUNT(1) AS c FROM request_logs WHERE timestamp >= ");
    ue_qb.push_bind(start).push(" AND timestamp <= ").push_bind(end).push(" AND status_code >= 400");
    if let Some(v) = listen_addr {
        ue_qb.push(" AND listen_addr = ").push_bind(v);
    }
    ue_qb.push(" GROUP BY upstream ORDER BY c DESC LIMIT 10");
    let top_up_err: Vec<KeyValue> = ue_qb.build_query_as::<(String, i64)>().fetch_all(&*pool).await?
        .into_iter().map(|(k, v)| KeyValue { key: k, value: v }).collect();

    // Latency Dist（12桶）
    let mut lat_qb = QueryBuilder::new(
        "SELECT\n        SUM(CASE WHEN latency_ms < 5 THEN 1 ELSE 0 END) AS b1,\n        SUM(CASE WHEN latency_ms >= 5 AND latency_ms < 10 THEN 1 ELSE 0 END) AS b2,\n        SUM(CASE WHEN latency_ms >= 10 AND latency_ms < 20 THEN 1 ELSE 0 END) AS b3,\n        SUM(CASE WHEN latency_ms >= 20 AND latency_ms < 50 THEN 1 ELSE 0 END) AS b4,\n        SUM(CASE WHEN latency_ms >= 50 AND latency_ms < 100 THEN 1 ELSE 0 END) AS b5,\n        SUM(CASE WHEN latency_ms >= 100 AND latency_ms < 150 THEN 1 ELSE 0 END) AS b6,\n        SUM(CASE WHEN latency_ms >= 150 AND latency_ms < 250 THEN 1 ELSE 0 END) AS b7,\n        SUM(CASE WHEN latency_ms >= 250 AND latency_ms < 400 THEN 1 ELSE 0 END) AS b8,\n        SUM(CASE WHEN latency_ms >= 400 AND latency_ms < 700 THEN 1 ELSE 0 END) AS b9,\n        SUM(CASE WHEN latency_ms >= 700 AND latency_ms < 1000 THEN 1 ELSE 0 END) AS b10,\n        SUM(CASE WHEN latency_ms >= 1000 AND latency_ms < 2000 THEN 1 ELSE 0 END) AS b11,\n        SUM(CASE WHEN latency_ms >= 2000 THEN 1 ELSE 0 END) AS b12\n        FROM request_logs WHERE timestamp >= "
    );
    lat_qb.push_bind(start).push(" AND timestamp <= ").push_bind(end);
    if let Some(v) = listen_addr {
        lat_qb.push(" AND listen_addr = ").push_bind(v);
    }

    let (b1, b2, b3, b4, b5, b6, b7, b8, b9, b10, b11, b12): (
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
        Option<i64>,
    ) = lat_qb.build_query_as().fetch_one(&*pool).await?;

    let latency_dist = vec![
        KeyValue { key: "<5ms".into(), value: b1.unwrap_or(0) },
        KeyValue { key: "5-10ms".into(), value: b2.unwrap_or(0) },
        KeyValue { key: "10-20ms".into(), value: b3.unwrap_or(0) },
        KeyValue { key: "20-50ms".into(), value: b4.unwrap_or(0) },
        KeyValue { key: "50-100ms".into(), value: b5.unwrap_or(0) },
        KeyValue { key: "100-150ms".into(), value: b6.unwrap_or(0) },
        KeyValue { key: "150-250ms".into(), value: b7.unwrap_or(0) },
        KeyValue { key: "250-400ms".into(), value: b8.unwrap_or(0) },
        KeyValue { key: "400-700ms".into(), value: b9.unwrap_or(0) },
        KeyValue { key: "700-1000ms".into(), value: b10.unwrap_or(0) },
        KeyValue { key: "1000-2000ms".into(), value: b11.unwrap_or(0) },
        KeyValue { key: ">=2000ms".into(), value: b12.unwrap_or(0) },
    ];

    // P50/P95/P99（近似）：使用 12 桶估算
    let dist_counts = [
        b1.unwrap_or(0), b2.unwrap_or(0), b3.unwrap_or(0), b4.unwrap_or(0),
        b5.unwrap_or(0), b6.unwrap_or(0), b7.unwrap_or(0), b8.unwrap_or(0),
        b9.unwrap_or(0), b10.unwrap_or(0), b11.unwrap_or(0), b12.unwrap_or(0),
    ];
    let total_lat = dist_counts.iter().sum::<i64>();

    let dist_medians = [2.5, 7.5, 15.0, 35.0, 75.0, 125.0, 200.0, 325.0, 550.0, 850.0, 1500.0, 3000.0];

    let approx_percentile = |p: f64| -> f64 {
        if total_lat <= 0 {
            return 0.0;
        }
        let target = (total_lat as f64 * p).ceil() as i64;
        let mut acc = 0i64;
        for i in 0..dist_counts.len() {
            acc += dist_counts[i];
            if acc >= target {
                return dist_medians[i];
            }
        }
        dist_medians[dist_medians.len() - 1]
    };

    let p50 = approx_percentile(0.50);
    let p95 = approx_percentile(0.95);
    let p99 = approx_percentile(0.99);

    Ok(QueryMetricsResponse {
        series: MetricsSeries {
            timestamps, counts, s2xx, s3xx, s4xx, s5xx, s0: vec![0; cap],
            avg_latency_ms: avg_latency, max_latency_ms: max_latency,
            p50: Some(vec![p50; cap]), p95: Some(vec![p95; cap]), p99: Some(vec![p99; cap]),
            upstream_dist: Some(upstream_dist), top_route_err: Some(top_route_err),
            top_up_err: Some(top_up_err), latency_dist: Some(latency_dist),
        },
    })
}

pub async fn get_dashboard_stats(req: DashboardStatsRequest) -> Result<DashboardStatsResponse> {
    let Some(pool) = db_pool() else { return Ok(DashboardStatsResponse::default()) };

    let gran = req.granularity_secs.max(1);
    let listen_addr = req.listen_addr.as_deref().map(str::trim).filter(|s| !s.is_empty());

    // Series
    let mut series_qb = QueryBuilder::new("SELECT (timestamp / ");
    series_qb.push_bind(gran).push(") * ").push_bind(gran).push(r#" AS time_bucket,
        COUNT(1) AS total_requests,
        SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS success_requests,
        SUM(CASE WHEN status_code BETWEEN 300 AND 399 THEN 1 ELSE 0 END) AS redirect_requests,
        SUM(CASE WHEN status_code BETWEEN 400 AND 499 THEN 1 ELSE 0 END) AS client_error_requests,
        SUM(CASE WHEN status_code >= 500 THEN 1 ELSE 0 END) AS server_error_requests,
        AVG(latency_ms) AS avg_latency_ms
    FROM request_logs WHERE timestamp >= "#);
    series_qb.push_bind(req.start_time).push(" AND timestamp <= ").push_bind(req.end_time);
    
    if let Some(v) = listen_addr {
        series_qb.push(" AND listen_addr = ").push_bind(v);
    }
    series_qb.push(" GROUP BY time_bucket ORDER BY time_bucket");
    let time_series = series_qb.build_query_as::<DashboardStatsPoint>().fetch_all(&*pool).await?;

    // Top paths
    let mut path_qb = QueryBuilder::new("SELECT request_path AS item, COUNT(1) AS count FROM request_logs WHERE timestamp >= ");
    path_qb.push_bind(req.start_time).push(" AND timestamp <= ").push_bind(req.end_time);
    if let Some(v) = listen_addr {
        path_qb.push(" AND listen_addr = ").push_bind(v);
    }
    path_qb.push(" GROUP BY request_path ORDER BY count DESC LIMIT 10");
    let top_paths = path_qb.build_query_as::<TopListItem>().fetch_all(&*pool).await?;

    // Top IPs
    let mut ip_qb = QueryBuilder::new("SELECT client_ip AS item, COUNT(1) AS count FROM request_logs WHERE timestamp >= ");
    ip_qb.push_bind(req.start_time).push(" AND timestamp <= ").push_bind(req.end_time);
    if let Some(v) = listen_addr {
        ip_qb.push(" AND listen_addr = ").push_bind(v);
    }
    ip_qb.push(" GROUP BY client_ip ORDER BY count DESC LIMIT 10");
    let top_ips = ip_qb.build_query_as::<TopListItem>().fetch_all(&*pool).await?;

    // Top routes
    let mut route_qb = QueryBuilder::new(
        "SELECT matched_route_id AS item, COUNT(1) AS count FROM request_logs WHERE timestamp >= ",
    );
    route_qb.push_bind(req.start_time).push(" AND timestamp <= ").push_bind(req.end_time);
    route_qb.push(" AND trim(matched_route_id) != ''");
    if let Some(v) = listen_addr {
        route_qb.push(" AND listen_addr = ").push_bind(v);
    }
    route_qb.push(" GROUP BY matched_route_id ORDER BY count DESC LIMIT 10");
    let top_routes = route_qb.build_query_as::<TopListItem>().fetch_all(&*pool).await?;

    // Top route errors
    let mut route_err_qb = QueryBuilder::new(
        "SELECT matched_route_id AS item, COUNT(1) AS count FROM request_logs WHERE timestamp >= ",
    );
    route_err_qb.push_bind(req.start_time)
        .push(" AND timestamp <= ")
        .push_bind(req.end_time);
    route_err_qb.push(" AND trim(matched_route_id) != '' AND status_code >= 400");
    if let Some(v) = listen_addr {
        route_err_qb.push(" AND listen_addr = ").push_bind(v);
    }
    route_err_qb.push(" GROUP BY matched_route_id ORDER BY count DESC LIMIT 10");
    let top_route_errors = route_err_qb.build_query_as::<TopListItem>().fetch_all(&*pool).await?;

    // Top upstream errors
    let mut up_err_qb = QueryBuilder::new(
        "SELECT upstream AS item, COUNT(1) AS count FROM request_logs WHERE timestamp >= ",
    );
    up_err_qb
        .push_bind(req.start_time)
        .push(" AND timestamp <= ")
        .push_bind(req.end_time);
    up_err_qb.push(" AND status_code >= 400");
    if let Some(v) = listen_addr {
        up_err_qb.push(" AND listen_addr = ").push_bind(v);
    }
    up_err_qb.push(" GROUP BY upstream ORDER BY count DESC LIMIT 10");
    let top_upstream_errors = up_err_qb.build_query_as::<TopListItem>().fetch_all(&*pool).await?;

    // Overall
    let mut ov_qb = QueryBuilder::new("SELECT COUNT(1) AS total, SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS ok, AVG(latency_ms) AS avg_latency FROM request_logs WHERE timestamp >= ");
    ov_qb.push_bind(req.start_time).push(" AND timestamp <= ").push_bind(req.end_time);
    if let Some(v) = listen_addr {
        ov_qb.push(" AND listen_addr = ").push_bind(v);
    }
    let (total_requests, ok_requests, avg_latency): (i64, Option<i64>, Option<f64>) = ov_qb.build_query_as().fetch_one(&*pool).await?;
    
    let success_rate = if total_requests > 0 {
        ok_requests.unwrap_or(0) as f64 / total_requests as f64
    } else { 0.0 };

    Ok(DashboardStatsResponse {
        time_series,
        top_paths,
        top_ips,
        top_routes,
        top_route_errors,
        top_upstream_errors,
        total_requests,
        success_rate,
        avg_latency_ms: avg_latency.unwrap_or(0.0),
    })
}
