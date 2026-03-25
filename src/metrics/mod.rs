mod helpers;
mod models;

use anyhow::{anyhow, Context, Result};
use self::helpers::{normalize_request_path_for_top, normalize_upstream_for_top};
pub use self::models::{
    BlacklistEntry, DashboardStatsPoint, DashboardStatsRequest, DashboardStatsResponse, KeyValue,
    MetricsPayload, MetricsSeries, QueryMetricsRequest, QueryMetricsResponse,
    QueryRequestLogsRequest, QueryRequestLogsResponse, RequestLog, RequestLogInsert, TopListItem,
};
use once_cell::sync::Lazy;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{ConnectOptions, QueryBuilder}; // 移除了未使用的 Row
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// 增加批量大小以利用 Bulk Insert 优势
const DB_FLUSH_BATCH_SIZE: usize = 2000;
const DB_FLUSH_INTERVAL: Duration = Duration::from_secs(5);

const REQUEST_LOG_RETENTION_DAYS: i64 = 730;
const REQUEST_LOG_RETENTION_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const DB_SPACE_RECLAIM_VACUUM_COOLDOWN: Duration = Duration::from_secs(24 * 60 * 60);
const DB_SPACE_RECLAIM_MIN_DELETED_ROWS: u64 = 20_000;
const DB_SPACE_RECLAIM_MIN_FREELIST_PAGES: i64 = 16_384; // 约 64MB（按 4KB 页估算）
const DB_SPACE_RECLAIM_MIN_FREELIST_RATIO: f64 = 0.20;

static DB_POOL: Lazy<RwLock<Option<Arc<SqlitePool>>>> = Lazy::new(|| RwLock::new(None));
static DB_PATH: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::new()));
static DB_ERROR: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));
static DB_LAST_VACUUM_AT: Lazy<RwLock<Option<Instant>>> = Lazy::new(|| RwLock::new(None));
static DB_VACUUM_RUNNING: AtomicBool = AtomicBool::new(false);

static BLACKLIST_CACHE: Lazy<RwLock<HashMap<String, i64>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

const REALTIME_WINDOW_SECS: i64 = 43200; // 12h
const REALTIME_MINUTE_WINDOW_SECS: i64 = 86400; // 24h

const REALTIME_SHARDS: usize = 64;

static REALTIME_AGG_SHARDS: Lazy<Vec<RwLock<RealtimeAgg>>> = Lazy::new(|| {
    let mut v = Vec::with_capacity(REALTIME_SHARDS);
    for _ in 0..REALTIME_SHARDS {
        v.push(RwLock::new(RealtimeAgg::new()));
    }
    v
});

#[inline]
fn hash_fnv1a_64(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut h = FNV_OFFSET;
    for &b in s.as_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

#[inline]
fn get_or_default_by_str<'a, V: Default>(
    map: &'a mut HashMap<String, V>,
    key: &str,
) -> &'a mut V {
    if !map.contains_key(key) {
        map.insert(key.to_string(), V::default());
    }
    map.get_mut(key).expect("key inserted or existed")
}

const METRICS_CACHE_TTL: Duration = Duration::from_millis(500);
static METRICS_CACHE: Lazy<RwLock<Option<(Instant, MetricsPayload)>>> =
    Lazy::new(|| RwLock::new(None));

// --- Models ---

#[derive(Debug, Clone, Default)]
struct RtBucket {
    ts: i64,
    count: i64,
    s2xx: i64,
    s3xx: i64,
    s4xx: i64,
    s5xx: i64,
    s0: i64,
    latency_sum_ms: f64,
    latency_max_ms: f64,
}

impl RtBucket {
    #[inline]
    fn add(&mut self, status_code: i32, latency_ms: f64) {
        self.count += 1;
        match status_code {
            200..=299 => self.s2xx += 1,
            300..=399 => self.s3xx += 1,
            400..=499 => self.s4xx += 1,
            s if s >= 500 => self.s5xx += 1,
            _ => self.s0 += 1,
        }

        if latency_ms.is_finite() {
            let v = latency_ms.max(0.0);
            self.latency_sum_ms += v;
            if v > self.latency_max_ms {
                self.latency_max_ms = v;
            }
        }
    }

    #[inline]
    fn avg_latency_ms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.latency_sum_ms / (self.count as f64)
        }
    }
}

#[derive(Debug, Default)]
struct RtSeriesAgg {
    buckets: BTreeMap<i64, RtBucket>,
}

impl RtSeriesAgg {
    fn add(&mut self, ts: i64, status_code: i32, latency_ms: f64) {
        self.buckets
            .entry(ts)
            .or_insert_with(|| RtBucket { ts, ..Default::default() })
            .add(status_code, latency_ms);
    }

    fn trim_older_than(&mut self, min_ts: i64) {
        while let Some((&k, _)) = self.buckets.iter().next() {
            if k < min_ts {
                self.buckets.remove(&k);
            } else {
                break;
            }
        }
    }

    fn to_metrics_series(&self) -> MetricsSeries {
        let len = self.buckets.len();
        let mut res = MetricsSeries {
            timestamps: Vec::with_capacity(len),
            counts: Vec::with_capacity(len),
            s2xx: Vec::with_capacity(len),
            s3xx: Vec::with_capacity(len),
            s4xx: Vec::with_capacity(len),
            s5xx: Vec::with_capacity(len),
            s0: Vec::with_capacity(len),
            avg_latency_ms: Vec::with_capacity(len),
            max_latency_ms: Vec::with_capacity(len),
            p50: None,
            p95: None,
            p99: None,
            upstream_dist: None,
            top_route_err: None,
            top_up_err: None,
            latency_dist: None,
        };

        for b in self.buckets.values() {
            res.timestamps.push(b.ts);
            res.counts.push(b.count);
            res.s2xx.push(b.s2xx);
            res.s3xx.push(b.s3xx);
            res.s4xx.push(b.s4xx);
            res.s5xx.push(b.s5xx);
            res.s0.push(b.s0);
            res.avg_latency_ms.push((b.avg_latency_ms() * 10000.0).round() / 10000.0);
            res.max_latency_ms.push((b.latency_max_ms * 10000.0).round() / 10000.0);
        }
        res
    }
}

#[inline]
fn merge_rt_series_map(dst: &mut HashMap<String, RtSeriesAgg>, src: &HashMap<String, RtSeriesAgg>) {
    for (k, v) in src.iter() {
        let dst_series = dst.entry(k.clone()).or_default();
        for (ts, b) in v.buckets.iter() {
            let out = dst_series
                .buckets
                .entry(*ts)
                .or_insert_with(|| RtBucket { ts: *ts, ..Default::default() });
            out.count += b.count;
            out.s2xx += b.s2xx;
            out.s3xx += b.s3xx;
            out.s4xx += b.s4xx;
            out.s5xx += b.s5xx;
            out.s0 += b.s0;
            out.latency_sum_ms += b.latency_sum_ms;
            if b.latency_max_ms > out.latency_max_ms {
                out.latency_max_ms = b.latency_max_ms;
            }
        }
    }
}

#[inline]
fn merge_count_map(
    dst: &mut HashMap<String, HashMap<String, i64>>,
    src: &HashMap<String, HashMap<String, i64>>,
) {
    for (k, m) in src.iter() {
        let out = dst.entry(k.clone()).or_default();
        for (item, cnt) in m.iter() {
            *out.entry(item.clone()).or_insert(0) += *cnt;
        }
    }
}

#[derive(Clone, Copy)]
struct RequestLogQueryFilters<'a> {
    start_time: i64,
    end_time: i64,
    listen_addr: Option<&'a str>,
    upstream: Option<&'a str>,
    request_path: Option<&'a str>,
    client_ip: Option<&'a str>,
    status_code: Option<i32>,
    matched_route_id: Option<&'a str>,
}

fn append_request_logs_where<'a>(
    qb: &mut QueryBuilder<'a, sqlx::Sqlite>,
    filters: RequestLogQueryFilters<'a>,
) {
    qb.push(" WHERE timestamp >= ")
        .push_bind(filters.start_time)
        .push(" AND timestamp <= ")
        .push_bind(filters.end_time);

    if let Some(v) = filters.listen_addr {
        qb.push(" AND listen_addr = ").push_bind(v);
    }
    if let Some(v) = filters.upstream {
        qb.push(" AND upstream LIKE ").push_bind(format!("%{}%", v));
    }
    if let Some(v) = filters.request_path {
        qb.push(" AND request_path LIKE ").push_bind(format!("%{}%", v));
    }
    if let Some(v) = filters.client_ip {
        qb.push(" AND client_ip LIKE ").push_bind(format!("%{}%", v));
    }
    if let Some(v) = filters.status_code {
        qb.push(" AND status_code = ").push_bind(v);
    }
    if let Some(v) = filters.matched_route_id {
        qb.push(" AND matched_route_id = ").push_bind(v);
    }
}

#[derive(Debug, Default)]
struct RealtimeAgg {
    per_sec: HashMap<String, RtSeriesAgg>,
    per_min: HashMap<String, RtSeriesAgg>,
    route_counts: HashMap<String, HashMap<String, i64>>,
    path_counts: HashMap<String, HashMap<String, i64>>,
    ip_counts: HashMap<String, HashMap<String, i64>>,
    upstream_error_counts: HashMap<String, HashMap<String, i64>>,
    upstream_counts: HashMap<String, HashMap<String, i64>>,
}

impl RealtimeAgg {
    fn new() -> Self {
        Self::default()
    }

    fn add(
        &mut self,
        listen_addr: &str,
        ts_sec: i64,
        status_code: i32,
        latency_ms: f64,
        matched_route_id: &str,
        request_path: &str,
        client_ip: &str,
        upstream: &str,
    ) {
        self.add_one(
            "全局",
            ts_sec,
            status_code,
            latency_ms,
            matched_route_id,
            request_path,
            client_ip,
            upstream,
        );
        let la = listen_addr.trim();
        if !la.is_empty() {
            self.add_one(
                la,
                ts_sec,
                status_code,
                latency_ms,
                matched_route_id,
                request_path,
                client_ip,
                upstream,
            );
        }
    }

    fn add_one(
        &mut self,
        key: &str,
        ts_sec: i64,
        status_code: i32,
        latency_ms: f64,
        matched_route_id: &str,
        request_path: &str,
        client_ip: &str,
        upstream: &str,
    ) {
        let min_ts = (ts_sec / 60) * 60;

        let sec = get_or_default_by_str(&mut self.per_sec, key);
        sec.add(ts_sec, status_code, latency_ms);
        sec.trim_older_than(ts_sec - REALTIME_WINDOW_SECS);

        let min = get_or_default_by_str(&mut self.per_min, key);
        min.add(min_ts, status_code, latency_ms);
        min.trim_older_than(ts_sec - REALTIME_MINUTE_WINDOW_SECS);

        // Top Routes（matched_route_id）实时聚合
        let rid = matched_route_id.trim();
        if !rid.is_empty() {
            let m = get_or_default_by_str(&mut self.route_counts, key);
            *m.entry(rid.to_string()).or_insert(0) += 1;
        }

        // Top request_path 实时聚合
        let p = normalize_request_path_for_top(request_path);
        {
            let m = get_or_default_by_str(&mut self.path_counts, key);
            *m.entry(p).or_insert(0) += 1;
        }

        // Top client_ip 实时聚合
        let ip = client_ip.trim();
        if !ip.is_empty() {
            let m = get_or_default_by_str(&mut self.ip_counts, key);
            *m.entry(ip.to_string()).or_insert(0) += 1;
        }

        let normalized_upstream = normalize_upstream_for_top(upstream);

        // Upstream 请求分布实时聚合
        {
            let m = get_or_default_by_str(&mut self.upstream_counts, key);
            *m.entry(normalized_upstream.clone()).or_insert(0) += 1;
        }

        // Top upstream(错误) 实时聚合
        if status_code >= 400 {
            let m = get_or_default_by_str(&mut self.upstream_error_counts, key);
            *m.entry(normalized_upstream).or_insert(0) += 1;
        }
    }

    fn to_payload(&self) -> MetricsPayload {
        let mut listen_addrs: Vec<String> = self
            .per_sec
            .keys()
            .filter(|k| k.as_str() != "全局")
            .cloned()
            .collect();
        listen_addrs.sort_unstable();
        listen_addrs.insert(0, "全局".to_string());

        let mut by_listen_addr = HashMap::with_capacity(self.per_sec.len());
        for (k, v) in self.per_sec.iter() {
            let mut s = v.to_metrics_series();
            // 实时 upstream 分布（Top20）
            if let Some(m) = self.upstream_counts.get(k) {
                let mut vv: Vec<KeyValue> = m
                    .iter()
                    .map(|(kk, cc)| KeyValue {
                        key: kk.clone(),
                        value: *cc,
                    })
                    .collect();
                vv.sort_unstable_by(|a, b| b.value.cmp(&a.value));
                if vv.len() > 20 {
                    vv.truncate(20);
                }
                if !vv.is_empty() {
                    s.upstream_dist = Some(vv);
                }
            }

            by_listen_addr.insert(k.clone(), s);
        }

        let mut by_listen_minute = HashMap::with_capacity(self.per_min.len());
        for (k, v) in self.per_min.iter() {
            by_listen_minute.insert(k.clone(), v.to_metrics_series());
        }

        let top_routes: Vec<TopListItem> = self
            .route_counts
            .get("全局")
            .map(|m| {
                let mut v: Vec<TopListItem> = m
                    .iter()
                    .map(|(k, c)| TopListItem {
                        item: k.clone(),
                        count: *c,
                    })
                    .collect();
                v.sort_unstable_by(|a, b| b.count.cmp(&a.count));
                if v.len() > 10 {
                    v.truncate(10);
                }
                v
            })
            .unwrap_or_default();

        if top_routes.is_empty() {
            // 如果全局为空，尽量给一个空的 None，减少前端处理
        }

        let top_paths: Vec<TopListItem> = self
            .path_counts
            .get("全局")
            .map(|m| {
                let mut v: Vec<TopListItem> = m
                    .iter()
                    .map(|(k, c)| TopListItem {
                        item: k.clone(),
                        count: *c,
                    })
                    .collect();
                v.sort_unstable_by(|a, b| b.count.cmp(&a.count));
                if v.len() > 10 {
                    v.truncate(10);
                }
                v
            })
            .unwrap_or_default();

        let top_client_ips: Vec<TopListItem> = self
            .ip_counts
            .get("全局")
            .map(|m| {
                let mut v: Vec<TopListItem> = m
                    .iter()
                    .map(|(k, c)| TopListItem {
                        item: k.clone(),
                        count: *c,
                    })
                    .collect();
                v.sort_unstable_by(|a, b| b.count.cmp(&a.count));
                if v.len() > 10 {
                    v.truncate(10);
                }
                v
            })
            .unwrap_or_default();

        let top_upstream_errors: Vec<TopListItem> = self
            .upstream_error_counts
            .get("全局")
            .map(|m| {
                let mut v: Vec<TopListItem> = m
                    .iter()
                    .map(|(k, c)| TopListItem {
                        item: k.clone(),
                        count: *c,
                    })
                    .collect();
                v.sort_unstable_by(|a, b| b.count.cmp(&a.count));
                if v.len() > 10 {
                    v.truncate(10);
                }
                v
            })
            .unwrap_or_default();

        MetricsPayload {
            window_seconds: REALTIME_WINDOW_SECS as i32,
            listen_addrs,
            by_listen_addr,
            minute_window_seconds: Some(REALTIME_MINUTE_WINDOW_SECS as i32),
            by_listen_minute: Some(by_listen_minute),
            top_routes: if top_routes.is_empty() { None } else { Some(top_routes) },
            top_paths: if top_paths.is_empty() { None } else { Some(top_paths) },
            top_client_ips: if top_client_ips.is_empty() { None } else { Some(top_client_ips) },
            top_upstream_errors: if top_upstream_errors.is_empty() { None } else { Some(top_upstream_errors) },
        }
    }
}

// --- DB Utils ---

include!("db.rs");
include!("writer.rs");
include!("query.rs");
