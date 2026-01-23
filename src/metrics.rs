use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{ConnectOptions, QueryBuilder}; // 移除了未使用的 Row
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

// 增加批量大小以利用 Bulk Insert 优势
const DB_FLUSH_BATCH_SIZE: usize = 2000;
const DB_FLUSH_INTERVAL: Duration = Duration::from_secs(5);

static DB_POOL: Lazy<RwLock<Option<Arc<SqlitePool>>>> = Lazy::new(|| RwLock::new(None));
static DB_PATH: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::new()));
static DB_ERROR: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

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

const METRICS_CACHE_TTL: Duration = Duration::from_millis(500);
static METRICS_CACHE: Lazy<RwLock<Option<(Instant, MetricsPayload)>>> =
    Lazy::new(|| RwLock::new(None));

// --- Models ---

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BlacklistEntry {
    pub id: i64,
    pub ip: String,
    pub reason: Option<String>,
    pub expires_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMetricsRequest {
    pub start_time: i64,
    pub end_time: i64,
    pub listen_addr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryMetricsResponse {
    pub series: MetricsSeries,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequestLogsRequest {
    pub start_time: i64,
    pub end_time: i64,
    pub listen_addr: Option<String>,
    pub upstream: Option<String>,
    pub request_path: Option<String>,
    pub client_ip: Option<String>,
    pub status_code: Option<i32>,
    pub page: i32,
    pub page_size: i32,
    pub matched_route_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequestLogsResponse {
    pub logs: Vec<RequestLog>,
    pub total: i64,
    pub total_page: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RequestLog {
    pub id: i64,
    pub timestamp: i64,
    pub listen_addr: String,
    pub client_ip: String,
    pub remote_ip: String,
    pub method: String,
    pub request_path: String,
    pub request_host: String,
    pub status_code: i32,
    pub upstream: String,
    pub latency_ms: f64,
    pub user_agent: String,
    pub referer: String,
    #[sqlx(default)]
    pub matched_route_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStatsRequest {
    pub start_time: i64,
    pub end_time: i64,
    pub listen_addr: Option<String>,
    pub granularity_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Default)]
pub struct DashboardStatsPoint {
    pub time_bucket: i64,
    pub total_requests: i64,
    pub success_requests: i64,
    pub redirect_requests: i64,
    pub client_error_requests: i64,
    pub server_error_requests: i64,
    #[sqlx(default)]
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TopListItem {
    pub item: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardStatsResponse {
    pub time_series: Vec<DashboardStatsPoint>,
    pub top_paths: Vec<TopListItem>,
    pub top_ips: Vec<TopListItem>,
    pub top_routes: Vec<TopListItem>,
    pub top_route_errors: Vec<TopListItem>,
    pub total_requests: i64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone)]
pub struct RequestLogInsert {
    pub timestamp: i64,
    pub listen_addr: String,
    pub client_ip: String,
    pub remote_ip: String,
    pub method: String,
    pub request_path: String,
    pub request_host: String,
    pub status_code: i32,
    pub upstream: String,
    pub latency_ms: f64,
    pub user_agent: String,
    pub referer: String,
    pub matched_route_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSeries {
    pub timestamps: Vec<i64>,
    pub counts: Vec<i64>,
    pub s2xx: Vec<i64>,
    pub s3xx: Vec<i64>,
    pub s4xx: Vec<i64>,
    pub s5xx: Vec<i64>,
    pub s0: Vec<i64>,
    #[serde(rename = "avgLatencyMs")]
    pub avg_latency_ms: Vec<f64>,
    #[serde(rename = "maxLatencyMs")]
    pub max_latency_ms: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p50: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p99: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "upstreamDist")]
    pub upstream_dist: Option<Vec<KeyValue>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topRouteErr")]
    pub top_route_err: Option<Vec<KeyValue>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topUpErr")]
    pub top_up_err: Option<Vec<KeyValue>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "latencyDist")]
    pub latency_dist: Option<Vec<KeyValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsPayload {
    #[serde(rename = "windowSeconds")]
    pub window_seconds: i32,
    #[serde(rename = "listenAddrs")]
    pub listen_addrs: Vec<String>,
    #[serde(rename = "byListenAddr")]
    pub by_listen_addr: HashMap<String, MetricsSeries>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "minuteWindowSeconds")]
    pub minute_window_seconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "byListenMinute")]
    pub by_listen_minute: Option<HashMap<String, MetricsSeries>>,
}

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

#[derive(Debug, Default)]
struct RealtimeAgg {
    per_sec: HashMap<String, RtSeriesAgg>,
    per_min: HashMap<String, RtSeriesAgg>,
}

impl RealtimeAgg {
    fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, listen_addr: &str, ts_sec: i64, status_code: i32, latency_ms: f64) {
        self.add_one("全局", ts_sec, status_code, latency_ms);
        let la = listen_addr.trim();
        if !la.is_empty() {
            self.add_one(la, ts_sec, status_code, latency_ms);
        }
    }

    fn add_one(&mut self, key: &str, ts_sec: i64, status_code: i32, latency_ms: f64) {
        let min_ts = (ts_sec / 60) * 60;

        let sec = self.per_sec.entry(key.to_string()).or_default();
        sec.add(ts_sec, status_code, latency_ms);
        sec.trim_older_than(ts_sec - REALTIME_WINDOW_SECS);

        let min = self.per_min.entry(key.to_string()).or_default();
        min.add(min_ts, status_code, latency_ms);
        min.trim_older_than(ts_sec - REALTIME_MINUTE_WINDOW_SECS);
    }

    fn to_payload(&self) -> MetricsPayload {
        let mut listen_addrs: Vec<String> = self
            .per_sec
            .keys()
            .filter(|k| k.as_str() != "全局")
            .cloned()
            .collect();
        listen_addrs.sort();
        listen_addrs.insert(0, "全局".to_string());

        let mut by_listen_addr = HashMap::with_capacity(self.per_sec.len());
        for (k, v) in self.per_sec.iter() {
            by_listen_addr.insert(k.clone(), v.to_metrics_series());
        }

        let mut by_listen_minute = HashMap::with_capacity(self.per_min.len());
        for (k, v) in self.per_min.iter() {
            by_listen_minute.insert(k.clone(), v.to_metrics_series());
        }

        MetricsPayload {
            window_seconds: REALTIME_WINDOW_SECS as i32,
            listen_addrs,
            by_listen_addr,
            minute_window_seconds: Some(REALTIME_MINUTE_WINDOW_SECS as i32),
            by_listen_minute: Some(by_listen_minute),
        }
    }
}

// --- DB Utils ---

fn default_db_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().with_context(|| "无法获取可执行文件路径")?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("无法获取可执行文件所在目录"))?;
    Ok(dir.join("data").join("metrics.db"))
}

fn resolve_db_path(input: String) -> Result<PathBuf> {
    let s = input.trim();
    if s.is_empty() {
        return default_db_path();
    }
    let p = PathBuf::from(s);
    if p.is_absolute() {
        Ok(p)
    } else {
        let exe = std::env::current_exe().with_context(|| "无法获取可执行文件路径")?;
        let dir = exe
            .parent()
            .ok_or_else(|| anyhow!("无法获取可执行文件所在目录"))?;
        Ok(dir.join(p))
    }
}

#[inline]
fn sqlite_url(db_path: &Path) -> Result<String> {
    let s = db_path
        .to_str()
        .ok_or_else(|| anyhow!("数据库路径包含非法字符"))?;
    Ok(format!("sqlite://{}", s))
}

#[inline]
fn normalize_ip_key(ip: &str) -> String {
    ip.trim().to_ascii_lowercase()
}

pub fn is_ip_blacklisted(ip: &str) -> bool {
    let key = normalize_ip_key(ip);
    let now = chrono::Utc::now().timestamp();
    
    // 优化：仅使用读锁
    let cache = BLACKLIST_CACHE.read();
    match cache.get(&key) {
        None => false,
        Some(expires_at) => *expires_at == 0 || *expires_at > now,
    }
}

fn pool() -> Option<Arc<SqlitePool>> {
    DB_POOL.read().clone()
}

pub async fn init_db(db_path: String) -> Result<()> {
    let result: Result<()> = async move {
        let path = resolve_db_path(db_path)?;
        let dir = path
            .parent()
            .ok_or_else(|| anyhow!("无法获取数据库目录"))?
            .to_path_buf();

        tokio::fs::create_dir_all(&dir)
            .await
            .with_context(|| format!("创建数据库目录失败: {}", dir.display()))?;

        let url = sqlite_url(&path)?;

        let mut opt: SqliteConnectOptions = url
            .parse()
            .with_context(|| format!("解析数据库 URL 失败: {url}"))?;
        opt = opt.create_if_missing(true);
        opt = opt.disable_statement_logging();
        // 关键性能优化：启用 WAL 模式和 Normal 同步
        opt = opt.journal_mode(SqliteJournalMode::Wal);
        opt = opt.synchronous(SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(3)
            .connect_with(opt)
            .await
            .with_context(|| format!("连接数据库失败: {}", path.display()))?;

        // 轻量迁移：尽量通过 ALTER TABLE 添加缺失列，避免丢历史数据
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS request_logs (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              timestamp INTEGER NOT NULL,
              listen_addr TEXT NOT NULL,
              client_ip TEXT NOT NULL,
              remote_ip TEXT NOT NULL,
              method TEXT NOT NULL,
              request_path TEXT NOT NULL,
              request_host TEXT NOT NULL,
              status_code INTEGER NOT NULL,
              upstream TEXT NOT NULL,
              latency_ms REAL NOT NULL,
              user_agent TEXT NOT NULL,
              referer TEXT NOT NULL,
              matched_route_id TEXT NOT NULL DEFAULT ''
            );
            "#,
        )
        .execute(&pool)
        .await
        .context("创建 request_logs 表失败")?;

        // 为旧库补列（SQLite 不支持 IF NOT EXISTS，需要先探测列是否存在）
        // PRAGMA table_info 返回列：cid,name,type,notnull,dflt_value,pk
        // 这里我们只取 name 字段（第 2 列）
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as("PRAGMA table_info(request_logs)")
                .fetch_all(&pool)
                .await
                .context("读取 request_logs 表结构失败")?;
        let has_matched_route_id = cols.iter().any(|(_, name, _, _, _, _)| name == "matched_route_id");
        if !has_matched_route_id {
            sqlx::query(
                "ALTER TABLE request_logs ADD COLUMN matched_route_id TEXT NOT NULL DEFAULT ''",
            )
            .execute(&pool)
            .await
            .context("迁移 request_logs.matched_route_id 失败")?;
        }

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_request_logs_ts ON request_logs(timestamp);"#,
        )
        .execute(&pool)
        .await
        .context("创建 request_logs.timestamp 索引失败")?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_request_logs_listen_ts ON request_logs(listen_addr, timestamp);"#,
        )
        .execute(&pool)
        .await
        .context("创建 request_logs.listen_addr+timestamp 索引失败")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS blacklist (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              ip TEXT NOT NULL UNIQUE,
              reason TEXT,
              expires_at INTEGER NOT NULL,
              created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .context("创建 blacklist 表失败")?;

        refresh_blacklist_cache_internal(&pool).await.ok();

        *DB_POOL.write() = Some(Arc::new(pool));
        *DB_PATH.write() = path.to_string_lossy().to_string();
        *DB_ERROR.write() = None;

        Ok(())
    }
    .await;

    if let Err(e) = result {
        *DB_ERROR.write() = Some(e.to_string());
        return Err(e);
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsDBStatus {
    pub enabled: bool,
    pub initialized: bool,
    pub path: String,
    pub error: Option<String>,
    pub file_exists: bool,
    pub dir_exists: bool,
    pub dir_writable: bool,
    pub message: Option<String>,
}

pub fn get_metrics_db_status() -> MetricsDBStatus {
    let initialized = DB_POOL.read().is_some();
    let path = DB_PATH.read().clone();

    let mut file_exists = false;
    let mut dir_exists = false;
    let mut dir_writable = false;
    let mut message: Option<String> = None;

    if !path.is_empty() {
        let p = PathBuf::from(&path);
        if let Some(dir) = p.parent() {
            dir_exists = dir.exists();
            dir_writable = dir_exists
                && std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(dir.join(".writable_check"))
                    .and_then(|_| std::fs::remove_file(dir.join(".writable_check")))
                    .is_ok();
        }
        file_exists = p.exists();

        if initialized && !file_exists && dir_exists && dir_writable {
            message = Some("数据库文件尚未创建，等待首次写入后生成".to_string());
        }
    }

    MetricsDBStatus {
        enabled: initialized,
        initialized,
        path,
        error: DB_ERROR.read().clone(),
        file_exists,
        dir_exists,
        dir_writable,
        message,
    }
}

pub async fn test_metrics_db_connection(db_path: String) -> Result<(bool, String)> {
    let path = resolve_db_path(db_path)?;
    let url = sqlite_url(&path)?;
    let mut opt: SqliteConnectOptions = url.parse()?;
    opt = opt.create_if_missing(true);
    opt = opt.disable_statement_logging();
    opt = opt.journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opt)
        .await?;

    let _ = sqlx::query("SELECT 1").fetch_one(&pool).await?;
    Ok((true, "OK".to_string()))
}

pub async fn refresh_blacklist_cache() -> Result<()> {
    let Some(pool) = pool() else { return Ok(()) };
    refresh_blacklist_cache_internal(&pool).await
}

async fn refresh_blacklist_cache_internal(pool: &SqlitePool) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    let rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT ip, expires_at FROM blacklist WHERE expires_at=0 OR expires_at>?",
    )
    .bind(now)
    .fetch_all(pool)
    .await?;

    let mut cache = BLACKLIST_CACHE.write();
    cache.clear();
    for (ip, exp) in rows {
        cache.insert(normalize_ip_key(&ip), exp);
    }
    Ok(())
}

pub async fn add_blacklist_entry(
    ip: String,
    reason: String,
    duration_seconds: i32,
) -> Result<BlacklistEntry> {
    let Some(pool) = pool() else {
        return Err(anyhow!("数据库未初始化"));
    };

    let now = chrono::Utc::now().timestamp();
    let expires_at = if duration_seconds <= 0 {
        0
    } else {
        now + duration_seconds as i64
    };

    let rec = sqlx::query_as::<_, BlacklistEntry>(
        "INSERT OR REPLACE INTO blacklist(ip, reason, expires_at, created_at) VALUES(?,?,?,?) RETURNING id, ip, reason, expires_at, created_at",
    )
    .bind(ip)
    .bind(Some(reason))
    .bind(expires_at)
    .bind(now)
    .fetch_one(&*pool)
    .await?;

    BLACKLIST_CACHE
        .write()
        .insert(normalize_ip_key(&rec.ip), rec.expires_at);

    Ok(rec)
}

pub async fn remove_blacklist_entry(ip: String) -> Result<()> {
    let Some(pool) = pool() else { return Ok(()) };

    sqlx::query("DELETE FROM blacklist WHERE ip=?")
        .bind(&ip)
        .execute(&*pool)
        .await?;

    BLACKLIST_CACHE.write().remove(&normalize_ip_key(&ip));
    Ok(())
}

pub async fn get_blacklist_entries() -> Result<Vec<BlacklistEntry>> {
    let Some(pool) = pool() else { return Ok(vec![]) };

    let rows = sqlx::query_as::<_, BlacklistEntry>(
        "SELECT id, ip, reason, expires_at, created_at FROM blacklist ORDER BY created_at DESC",
    )
    .fetch_all(&*pool)
    .await?;

    Ok(rows)
}

// 请求日志写入队列
static REQUEST_LOG_TX: Lazy<RwLock<Option<tokio::sync::mpsc::Sender<RequestLogInsert>>>> =
    Lazy::new(|| RwLock::new(None));

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
                let pool_opt = DB_POOL.read().clone();
                if let Some(pool) = pool_opt {
                    let _ = refresh_blacklist_cache_internal(&pool).await;
                }
                last_cleanup = Instant::now();
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
        );
    }

    if let Some(tx) = REQUEST_LOG_TX.read().as_ref() {
        let _ = tx.try_send(log);
    }
}

async fn flush_request_logs(buf: &mut Vec<RequestLogInsert>) {
    let Some(pool) = pool() else {
        buf.clear();
        return;
    };
    if buf.is_empty() { return; }

    // 使用 QueryBuilder 进行批量插入
    const CHUNK_SIZE: usize = 500;
    
    for chunk in buf.chunks(CHUNK_SIZE) {
        let mut query_builder = QueryBuilder::new(
            "INSERT INTO request_logs (timestamp, listen_addr, client_ip, remote_ip, method, request_path, request_host, status_code, upstream, latency_ms, user_agent, referer, matched_route_id) "
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

pub async fn query_request_logs(req: QueryRequestLogsRequest) -> Result<QueryRequestLogsResponse> {
    let Some(pool) = pool() else {
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

    // COUNT
    let mut count_qb = QueryBuilder::new("SELECT COUNT(1) FROM request_logs WHERE timestamp >= ");
    count_qb.push_bind(req.start_time);
    count_qb.push(" AND timestamp <= ");
    count_qb.push_bind(req.end_time);

    if let Some(v) = listen_addr {
        count_qb.push(" AND listen_addr = ").push_bind(v);
    }
    if let Some(v) = upstream {
        count_qb.push(" AND upstream LIKE ").push_bind(format!("%{}%", v));
    }
    if let Some(v) = request_path {
        count_qb.push(" AND request_path LIKE ").push_bind(format!("%{}%", v));
    }
    if let Some(v) = client_ip {
        count_qb.push(" AND client_ip LIKE ").push_bind(format!("%{}%", v));
    }
    if let Some(v) = status_code {
        count_qb.push(" AND status_code = ").push_bind(v);
    }
    if let Some(v) = matched_route_id {
        count_qb.push(" AND matched_route_id = ").push_bind(v);
    }

    let total: i64 = count_qb.build_query_as::<(i64,)>().fetch_one(&*pool).await?.0;
    let total_page = if total == 0 { 0 } else { (total + page_size - 1) / page_size };

    // SELECT
    let mut sel_qb = QueryBuilder::new(
        "SELECT id, timestamp, listen_addr, client_ip, remote_ip, method, request_path, request_host, status_code, upstream, latency_ms, user_agent, referer, matched_route_id FROM request_logs WHERE timestamp >= "
    );
    sel_qb.push_bind(req.start_time);
    sel_qb.push(" AND timestamp <= ");
    sel_qb.push_bind(req.end_time);

    if let Some(v) = listen_addr {
        sel_qb.push(" AND listen_addr = ").push_bind(v);
    }
    if let Some(v) = upstream {
        sel_qb.push(" AND upstream LIKE ").push_bind(format!("%{}%", v));
    }
    if let Some(v) = request_path {
        sel_qb.push(" AND request_path LIKE ").push_bind(format!("%{}%", v));
    }
    if let Some(v) = client_ip {
        sel_qb.push(" AND client_ip LIKE ").push_bind(format!("%{}%", v));
    }
    if let Some(v) = status_code {
        sel_qb.push(" AND status_code = ").push_bind(v);
    }
    if let Some(v) = matched_route_id {
        sel_qb.push(" AND matched_route_id = ").push_bind(v);
    }

    sel_qb.push(" ORDER BY timestamp DESC LIMIT ").push_bind(page_size).push(" OFFSET ").push_bind(offset);

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

        // per_sec
        for (k, v) in guard.per_sec.iter() {
            let dst = merged.per_sec.entry(k.clone()).or_default();
            for (ts, b) in v.buckets.iter() {
                let out = dst.buckets.entry(*ts).or_insert_with(|| RtBucket { ts: *ts, ..Default::default() });
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

        // per_min
        for (k, v) in guard.per_min.iter() {
            let dst = merged.per_min.entry(k.clone()).or_default();
            for (ts, b) in v.buckets.iter() {
                let out = dst.buckets.entry(*ts).or_insert_with(|| RtBucket { ts: *ts, ..Default::default() });
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

    let payload = merged.to_payload();
    {
        let mut cache = METRICS_CACHE.write();
        *cache = Some((Instant::now(), payload.clone()));
    }
    payload
}

pub async fn get_distinct_listen_addrs() -> Result<Vec<String>> {
    let Some(pool) = pool() else { return Ok(vec![]) };

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT listen_addr FROM request_logs WHERE trim(listen_addr) != '' ORDER BY listen_addr ASC",
    )
    .fetch_all(&*pool)
    .await
    .context("查询 request_logs.listen_addr distinct 失败")?;

    Ok(rows.into_iter().map(|(s,)| s).collect())
}

pub async fn query_historical_metrics(req: QueryMetricsRequest) -> Result<QueryMetricsResponse> {
    let Some(pool) = pool() else {
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
    let Some(pool) = pool() else { return Ok(DashboardStatsResponse::default()) };

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
        total_requests,
        success_rate,
        avg_latency_ms: avg_latency.unwrap_or(0.0),
    })
}