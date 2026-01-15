use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::ConnectOptions;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use url::Url;

static DB_POOL: Lazy<RwLock<Option<Arc<SqlitePool>>>> = Lazy::new(|| RwLock::new(None));
static DB_PATH: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::new()));
static DB_ERROR: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

static BLACKLIST_CACHE: Lazy<RwLock<HashMap<String, i64>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));
static BLACKLIST_LAST_CLEANUP: Lazy<RwLock<Instant>> = Lazy::new(|| RwLock::new(Instant::now()));

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStatsRequest {
    pub start_time: i64,
    pub end_time: i64,
    pub listen_addr: Option<String>,
    // 聚合粒度（秒）。例如 60=按分钟聚合。
    pub granularity_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Default)]
pub struct DashboardStatsPoint {
    pub time_bucket: i64,
    pub total_requests: i64,
    pub success_requests: i64,       // 2xx
    pub redirect_requests: i64,      // 3xx
    pub client_error_requests: i64,  // 4xx
    pub server_error_requests: i64,  // 5xx
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
    pub avgLatencyMs: Vec<f64>,
    pub maxLatencyMs: Vec<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub p95: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p99: Option<Vec<f64>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstreamDist: Option<Vec<KeyValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topRouteErr: Option<Vec<KeyValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topUpErr: Option<Vec<KeyValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latencyDist: Option<Vec<KeyValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsPayload {
    pub windowSeconds: i32,
    pub listenAddrs: Vec<String>,
    pub byListenAddr: HashMap<String, MetricsSeries>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub minuteWindowSeconds: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byListenMinute: Option<HashMap<String, MetricsSeries>>,
}

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

fn sqlite_url(db_path: &Path) -> Result<String> {
    let s = db_path
        .to_str()
        .ok_or_else(|| anyhow!("数据库路径包含非法字符"))?;
    Ok(format!("sqlite://{}", s))
}

fn normalize_ip_key(ip: &str) -> String {
    ip.trim().to_ascii_lowercase()
}

fn maybe_cleanup_blacklist_cache(now: i64) {
    // 降低每次请求的开销：最多 10 秒清理一次
    {
        let last = *BLACKLIST_LAST_CLEANUP.read();
        if last.elapsed() < Duration::from_secs(10) {
            return;
        }
    }

    let mut last = BLACKLIST_LAST_CLEANUP.write();
    if last.elapsed() < Duration::from_secs(10) {
        return;
    }
    *last = Instant::now();

    let mut cache = BLACKLIST_CACHE.write();
    cache.retain(|_, expires_at| *expires_at == 0 || *expires_at > now);
}

pub fn is_ip_blacklisted(ip: &str) -> bool {
    let key = normalize_ip_key(ip);
    let now = chrono::Utc::now().timestamp();
    maybe_cleanup_blacklist_cache(now);

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

        // 创建目录
        tokio::fs::create_dir_all(&dir)
            .await
            .with_context(|| format!("创建数据库目录失败: {}", dir.display()))?;

        let url = sqlite_url(&path)?;

        let mut opt: SqliteConnectOptions = url
            .parse()
            .with_context(|| format!("解析数据库 URL 失败: {url}"))?;
        opt = opt.create_if_missing(true);
        opt = opt.disable_statement_logging();

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opt)
            .await
            .with_context(|| format!("连接数据库失败: {}", path.display()))?;

        // 检查表结构是否需要更新（通过检查新字段是否存在）
        let needs_recreation = sqlx::query("SELECT remote_ip FROM request_logs LIMIT 1")
            .fetch_one(&pool)
            .await
            .is_err();

        if needs_recreation {
            sqlx::query("DROP TABLE IF EXISTS request_logs")
                .execute(&pool)
                .await
                .context("删除旧 request_logs 表失败")?;
        }

        // 建表：请求日志
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
              referer TEXT NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .context("创建 request_logs 表失败")?;

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

        // 建表：黑名单
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

        // 初始化黑名单缓存
        refresh_blacklist_cache_internal(&pool).await.ok();

        // 写入全局
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
    pub db_path: String,
    pub connected: bool,
    pub error: Option<String>,
}

pub fn get_metrics_db_status() -> MetricsDBStatus {
    let enabled = DB_POOL.read().is_some();
    MetricsDBStatus {
        enabled,
        db_path: DB_PATH.read().clone(),
        connected: enabled,
        error: DB_ERROR.read().clone(),
    }
}

pub async fn test_metrics_db_connection(db_path: String) -> Result<(bool, String)> {
    let path = resolve_db_path(db_path)?;
    let url = sqlite_url(&path)?;
    let mut opt: SqliteConnectOptions = url.parse()?;
    opt = opt.create_if_missing(true);
    opt = opt.disable_statement_logging();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opt)
        .await?;

    // 简单查询
    let _ = sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await?;

    Ok((true, "OK".to_string()))
}

pub async fn refresh_blacklist_cache() -> Result<()> {
    let Some(pool) = pool() else {
        return Ok(());
    };
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

    // 更新缓存
    BLACKLIST_CACHE
        .write()
        .insert(normalize_ip_key(&rec.ip), rec.expires_at);

    Ok(rec)
}

pub async fn remove_blacklist_entry(ip: String) -> Result<()> {
    let Some(pool) = pool() else {
        return Ok(());
    };

    sqlx::query("DELETE FROM blacklist WHERE ip=?")
        .bind(&ip)
        .execute(&*pool)
        .await?;

    BLACKLIST_CACHE.write().remove(&normalize_ip_key(&ip));
    Ok(())
}

pub async fn get_blacklist_entries() -> Result<Vec<BlacklistEntry>> {
    let Some(pool) = pool() else {
        return Ok(vec![]);
    };

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
        let mut buf: Vec<RequestLogInsert> = Vec::with_capacity(256);

        loop {
            tokio::select! {
                Some(item) = rx.recv() => {
                    buf.push(item);
                    if buf.len() >= 256 {
                        flush_request_logs(&mut buf).await;
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(500)) => {
                    if !buf.is_empty() {
                        flush_request_logs(&mut buf).await;
                    }
                }
            }
        }
    });
}

pub fn try_enqueue_request_log(log: RequestLogInsert) {
    if let Some(tx) = REQUEST_LOG_TX.read().as_ref() {
        let _ = tx.try_send(log);
    }
}

async fn flush_request_logs(buf: &mut Vec<RequestLogInsert>) {
    let Some(pool) = pool() else {
        buf.clear();
        return;
    };

    if buf.is_empty() {
        return;
    }

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(_) => {
            buf.clear();
            return;
        }
    };

    for it in buf.iter() {
        let _ = sqlx::query(
            r#"
            INSERT INTO request_logs (
              timestamp, listen_addr, client_ip, remote_ip, method, request_path, request_host,
              status_code, upstream, latency_ms, user_agent, referer
            ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)
            "#,
        )
        .bind(it.timestamp)
        .bind(&it.listen_addr)
        .bind(&it.client_ip)
        .bind(&it.remote_ip)
        .bind(&it.method)
        .bind(&it.request_path)
        .bind(&it.request_host)
        .bind(it.status_code)
        .bind(&it.upstream)
        .bind(it.latency_ms)
        .bind(&it.user_agent)
        .bind(&it.referer)
        .execute(&mut *tx)
        .await;
    }

    let _ = tx.commit().await;
    buf.clear();
}

pub async fn query_request_logs(req: QueryRequestLogsRequest) -> Result<QueryRequestLogsResponse> {
    let Some(pool) = pool() else {
        return Ok(QueryRequestLogsResponse {
            logs: vec![],
            total: 0,
            total_page: 0,
        });
    };

    let page_size = req.page_size.clamp(1, 200) as i64;
    let page = req.page.max(1) as i64;
    let offset = (page - 1) * page_size;

    // 组装过滤条件
    let listen_addr = req
        .listen_addr
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let upstream = req
        .upstream
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let request_path = req
        .request_path
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let client_ip = req
        .client_ip
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let status_code = req.status_code.filter(|c| *c > 0);

    // COUNT
    let mut count_sql =
        String::from("SELECT COUNT(1) FROM request_logs WHERE timestamp>=? AND timestamp<=?");
    if listen_addr.is_some() {
        count_sql.push_str(" AND listen_addr=?");
    }
    if let Some(_) = upstream {
        count_sql.push_str(" AND upstream LIKE ?");
    }
    if let Some(_) = request_path {
        count_sql.push_str(" AND request_path LIKE ?");
    }
    if let Some(_) = client_ip {
        count_sql.push_str(" AND client_ip LIKE ?");
    }
    if status_code.is_some() {
        count_sql.push_str(" AND status_code=?");
    }

    let mut q = sqlx::query_as::<_, (i64,)>(&count_sql)
        .bind(req.start_time)
        .bind(req.end_time);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }
    if let Some(v) = upstream {
        q = q.bind(format!("%{}%", v));
    }
    if let Some(v) = request_path {
        q = q.bind(format!("%{}%", v));
    }
    if let Some(v) = client_ip {
        q = q.bind(format!("%{}%", v));
    }
    if let Some(v) = status_code {
        q = q.bind(v);
    }

    let total = q.fetch_one(&*pool).await?.0;
    let total_page = if total == 0 {
        0
    } else {
        (total + page_size - 1) / page_size
    };

    // SELECT
    let mut select_sql = String::from(
        "SELECT id, timestamp, listen_addr, client_ip, remote_ip,
            method, request_path, request_host, status_code, upstream,
            latency_ms, user_agent, referer
        FROM request_logs
        WHERE timestamp>=? AND timestamp<=?",
    );
    if listen_addr.is_some() {
        select_sql.push_str(" AND listen_addr=?");
    }
    if upstream.is_some() {
        select_sql.push_str(" AND upstream LIKE ?");
    }
    if request_path.is_some() {
        select_sql.push_str(" AND request_path LIKE ?");
    }
    if client_ip.is_some() {
        select_sql.push_str(" AND client_ip LIKE ?");
    }
    if status_code.is_some() {
        select_sql.push_str(" AND status_code=?");
    }
    select_sql.push_str(" ORDER BY timestamp DESC LIMIT ? OFFSET ?");

    let mut q = sqlx::query_as::<_, RequestLog>(&select_sql)
        .bind(req.start_time)
        .bind(req.end_time);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }
    if let Some(v) = upstream {
        q = q.bind(format!("%{}%", v));
    }
    if let Some(v) = request_path {
        q = q.bind(format!("%{}%", v));
    }
    if let Some(v) = client_ip {
        q = q.bind(format!("%{}%", v));
    }
    if let Some(v) = status_code {
        q = q.bind(v);
    }

    let logs = q.bind(page_size).bind(offset).fetch_all(&*pool).await?;

    Ok(QueryRequestLogsResponse {
        logs,
        total,
        total_page,
    })
}

pub fn get_metrics() -> MetricsPayload {
    MetricsPayload {
        windowSeconds: 21600,
        listenAddrs: vec![],
        byListenAddr: HashMap::new(),
        minuteWindowSeconds: Some(86400),
        byListenMinute: Some(HashMap::new()),
    }
}

pub fn query_historical_metrics(req: QueryMetricsRequest) -> Result<QueryMetricsResponse> {
    let Some(pool) = pool() else {
        return Ok(QueryMetricsResponse {
            series: MetricsSeries {
                timestamps: vec![],
                counts: vec![],
                s2xx: vec![],
                s3xx: vec![],
                s4xx: vec![],
                s5xx: vec![],
                s0: vec![],
                avgLatencyMs: vec![],
                maxLatencyMs: vec![],
                p95: Some(vec![]),
                p99: Some(vec![]),
                upstreamDist: Some(vec![]),
                topRouteErr: Some(vec![]),
                topUpErr: Some(vec![]),
                latencyDist: Some(vec![]),
            },
        });
    };

    let listen_addr = req
        .listen_addr
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let start = req.start_time;
    let end = req.end_time;
    if end <= start {
        return Ok(QueryMetricsResponse {
            series: MetricsSeries {
                timestamps: vec![],
                counts: vec![],
                s2xx: vec![],
                s3xx: vec![],
                s4xx: vec![],
                s5xx: vec![],
                s0: vec![],
                avgLatencyMs: vec![],
                maxLatencyMs: vec![],
                p95: Some(vec![]),
                p99: Some(vec![]),
                upstreamDist: Some(vec![]),
                topRouteErr: Some(vec![]),
                topUpErr: Some(vec![]),
                latencyDist: Some(vec![]),
            },
        });
    }

    let span = end - start;
    // <1h: 1s
    // >=1h and <48h: 60s
    // >=48h: 300s (5min)
    let granularity = if span < 3600 {
        1
    } else if span < 48 * 3600 {
        60
    } else {
        300
    };

    // 聚合时序
    let mut ts_sql = String::from(
        "SELECT (timestamp / ?) * ? AS bucket, 
                COUNT(1) AS total,
                SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS s2xx,
                SUM(CASE WHEN status_code BETWEEN 300 AND 399 THEN 1 ELSE 0 END) AS s3xx,
                SUM(CASE WHEN status_code BETWEEN 400 AND 499 THEN 1 ELSE 0 END) AS s4xx,
                SUM(CASE WHEN status_code >= 500 THEN 1 ELSE 0 END) AS s5xx,
                AVG(latency_ms) AS avg_latency,
                MAX(latency_ms) AS max_latency
            FROM request_logs
            WHERE timestamp>=? AND timestamp<=?",
    );
    if listen_addr.is_some() {
        ts_sql.push_str(" AND listen_addr=?");
    }
    ts_sql.push_str(" GROUP BY bucket ORDER BY bucket");

    let mut q = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, Option<f64>, Option<f64>)>(&ts_sql)
        .bind(granularity)
        .bind(granularity)
        .bind(start)
        .bind(end);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }

    // 这里是同步函数，但 sqlx 是 async；用 block_in_place 在 tauri runtime 内执行。
    let rows = tauri::async_runtime::block_on(async { q.fetch_all(&*pool).await })?;

    let mut timestamps = Vec::with_capacity(rows.len());
    let mut counts = Vec::with_capacity(rows.len());
    let mut s2xx = Vec::with_capacity(rows.len());
    let mut s3xx = Vec::with_capacity(rows.len());
    let mut s4xx = Vec::with_capacity(rows.len());
    let mut s5xx = Vec::with_capacity(rows.len());
    let mut avg_latency = Vec::with_capacity(rows.len());
    let mut max_latency = Vec::with_capacity(rows.len());

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
    let mut up_sql = String::from(
        r#"
        SELECT
            CASE
                WHEN instr(h, '/') > 0 THEN substr(h, 1, instr(h, '/') - 1)
                ELSE h
            END AS k,
            COUNT(1) AS c
        FROM (
            SELECT
                replace(
                    replace(
                        replace(upstream, 'https://', ''),
                        'http://', ''
                    ),
                    'www.', ''
                ) AS h
            FROM request_logs
            WHERE timestamp >= ? AND timestamp <= ?
        ) AS t
        "#,
    );

    if listen_addr.is_some() {
        up_sql.push_str(" AND listen_addr=?");
    }
    up_sql.push_str(" GROUP BY k ORDER BY c DESC LIMIT 20");

    let mut q = sqlx::query_as::<_, (String, i64)>(&up_sql).bind(start).bind(end);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }
    let upstream_dist_rows = tauri::async_runtime::block_on(async { q.fetch_all(&*pool).await })?;
    let upstreamDist = upstream_dist_rows
        .into_iter()
        .map(|(k, c)| KeyValue { key: k, value: c })
        .collect::<Vec<_>>();

    // Top route 错误（>=400）
    let mut route_err_sql = String::from(
        "SELECT request_path AS k, COUNT(1) AS c FROM request_logs WHERE timestamp>=? AND timestamp<=? AND status_code>=400",
    );
    if listen_addr.is_some() {
        route_err_sql.push_str(" AND listen_addr=?");
    }
    route_err_sql.push_str(" GROUP BY request_path ORDER BY c DESC LIMIT 10");

    let mut q = sqlx::query_as::<_, (String, i64)>(&route_err_sql).bind(start).bind(end);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }
    let top_route_err_rows = tauri::async_runtime::block_on(async { q.fetch_all(&*pool).await })?;
    let topRouteErr = top_route_err_rows
        .into_iter()
        .map(|(k, c)| KeyValue { key: k, value: c })
        .collect::<Vec<_>>();

    // Top upstream 错误（>=400）
    let mut up_err_sql = String::from(
        "SELECT upstream AS k, COUNT(1) AS c FROM request_logs WHERE timestamp>=? AND timestamp<=? AND status_code>=400",
    );
    if listen_addr.is_some() {
        up_err_sql.push_str(" AND listen_addr=?");
    }
    up_err_sql.push_str(" GROUP BY upstream ORDER BY c DESC LIMIT 10");

    let mut q = sqlx::query_as::<_, (String, i64)>(&up_err_sql).bind(start).bind(end);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }
    let top_up_err_rows = tauri::async_runtime::block_on(async { q.fetch_all(&*pool).await })?;
    let topUpErr = top_up_err_rows
        .into_iter()
        .map(|(k, c)| KeyValue { key: k, value: c })
        .collect::<Vec<_>>();

    // 延迟分布（固定 bucket）
    let mut lat_sql = String::from(
        "SELECT SUM(CASE WHEN latency_ms < 10 THEN 1 ELSE 0 END) AS b1,
            SUM(CASE WHEN latency_ms >= 10 AND latency_ms < 50 THEN 1 ELSE 0 END) AS b2,
            SUM(CASE WHEN latency_ms >= 50 AND latency_ms < 100 THEN 1 ELSE 0 END) AS b3,
            SUM(CASE WHEN latency_ms >= 100 AND latency_ms < 300 THEN 1 ELSE 0 END) AS b4,
            SUM(CASE WHEN latency_ms >= 300 AND latency_ms < 1000 THEN 1 ELSE 0 END) AS b5,
            SUM(CASE WHEN latency_ms >= 1000 THEN 1 ELSE 0 END) AS b6
        FROM request_logs
        WHERE timestamp>=? AND timestamp<=?",
    );
    if listen_addr.is_some() {
        lat_sql.push_str(" AND listen_addr=?");
    }

    let mut q = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64)>(&lat_sql).bind(start).bind(end);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }
    let (b1, b2, b3, b4, b5, b6) = tauri::async_runtime::block_on(async { q.fetch_one(&*pool).await })?;

    let latencyDist = vec![
        KeyValue { key: "<10ms".to_string(), value: b1 },
        KeyValue { key: "10-50ms".to_string(), value: b2 },
        KeyValue { key: "50-100ms".to_string(), value: b3 },
        KeyValue { key: "100-300ms".to_string(), value: b4 },
        KeyValue { key: "300-1000ms".to_string(), value: b5 },
        KeyValue { key: ">=1000ms".to_string(), value: b6 },
    ];

    // p95/p99：近似（全区间排序取分位点）
    let mut p95 = 0.0;
    let mut p99 = 0.0;
    let mut p_sql = String::from(
        "SELECT latency_ms FROM request_logs WHERE timestamp>=? AND timestamp<=?",
    );
    if listen_addr.is_some() {
        p_sql.push_str(" AND listen_addr=?");
    }
    p_sql.push_str(" ORDER BY latency_ms ASC");

    let mut q = sqlx::query_as::<_, (f64,)>(&p_sql).bind(start).bind(end);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }
    let lat_all = tauri::async_runtime::block_on(async { q.fetch_all(&*pool).await }).unwrap_or_default();
    let n = lat_all.len();
    if n > 0 {
        let idx95 = ((n as f64) * 0.95).ceil() as usize;
        let idx99 = ((n as f64) * 0.99).ceil() as usize;
        let idx95 = idx95.saturating_sub(1).min(n - 1);
        let idx99 = idx99.saturating_sub(1).min(n - 1);
        p95 = ((lat_all[idx95].0 * 10000.0).round()) / 10000.0;
        p99 = ((lat_all[idx99].0 * 10000.0).round()) / 10000.0;
    }

    let series_len = timestamps.len();

    Ok(QueryMetricsResponse {
        series: MetricsSeries {
            timestamps,
            counts,
            s2xx,
            s3xx,
            s4xx,
            s5xx,
            s0: vec![0; series_len],
            avgLatencyMs: avg_latency,
            maxLatencyMs: max_latency,
            p95: Some(vec![p95; series_len]),
            p99: Some(vec![p99; series_len]),
            upstreamDist: Some(upstreamDist),
            topRouteErr: Some(topRouteErr),
            topUpErr: Some(topUpErr),
            latencyDist: Some(latencyDist),
        },
    })
}

pub async fn get_dashboard_stats(req: DashboardStatsRequest) -> Result<DashboardStatsResponse> {
    let Some(pool) = pool() else {
        return Ok(DashboardStatsResponse::default());
    };

    let gran = req.granularity_secs.max(1);

    let listen_addr = req
        .listen_addr
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    // time series
    let mut sql = String::from(
        "SELECT (timestamp / ?) * ? AS time_bucket,
            COUNT(1) AS total_requests,
            SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS success_requests,
            SUM(CASE WHEN status_code BETWEEN 300 AND 399 THEN 1 ELSE 0 END) AS redirect_requests,
            SUM(CASE WHEN status_code BETWEEN 400 AND 499 THEN 1 ELSE 0 END) AS client_error_requests,
            SUM(CASE WHEN status_code >= 500 THEN 1 ELSE 0 END) AS server_error_requests,
            AVG(latency_ms) AS avg_latency_ms
        FROM request_logs
        WHERE timestamp>=? AND timestamp<=?",
    );
    if listen_addr.is_some() {
        sql.push_str(" AND listen_addr=?");
    }
    sql.push_str(" GROUP BY time_bucket ORDER BY time_bucket");

    let mut q = sqlx::query_as::<_, DashboardStatsPoint>(&sql)
        .bind(gran)
        .bind(gran)
        .bind(req.start_time)
        .bind(req.end_time);

    if let Some(v) = listen_addr {
        q = q.bind(v);
    }

    let time_series = q.fetch_all(&*pool).await?;

    // top paths
    let mut sql = String::from(
        "SELECT request_path AS item, COUNT(1) AS count
        FROM request_logs
        WHERE timestamp>=? AND timestamp<=?",
    );
    if listen_addr.is_some() {
        sql.push_str(" AND listen_addr=?");
    }
    sql.push_str(" GROUP BY request_path ORDER BY count DESC LIMIT 10");

    let mut q = sqlx::query_as::<_, TopListItem>(&sql)
        .bind(req.start_time)
        .bind(req.end_time);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }
    let top_paths = q.fetch_all(&*pool).await?;

    // top ips
    let mut sql = String::from(
        "SELECT client_ip AS item, COUNT(1) AS count FROM request_logs WHERE timestamp>=? AND timestamp<=?",
    );
    if listen_addr.is_some() {
        sql.push_str(" AND listen_addr=?");
    }
    sql.push_str(" GROUP BY client_ip ORDER BY count DESC LIMIT 10");

    let mut q = sqlx::query_as::<_, TopListItem>(&sql)
        .bind(req.start_time)
        .bind(req.end_time);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }
    let top_ips = q.fetch_all(&*pool).await?;

    // overall
    let mut sql = String::from(
        "SELECT COUNT(1) AS total,
            SUM(CASE WHEN status_code BETWEEN 200 AND 299 THEN 1 ELSE 0 END) AS ok,
            AVG(latency_ms) AS avg_latency FROM request_logs
        WHERE timestamp>=? AND timestamp<=?",
    );
    if listen_addr.is_some() {
        sql.push_str(" AND listen_addr=?");
    }

    let mut q = sqlx::query_as::<_, (i64, i64, Option<f64>)>(&sql)
        .bind(req.start_time)
        .bind(req.end_time);
    if let Some(v) = listen_addr {
        q = q.bind(v);
    }

    let (total_requests, ok_requests, avg_latency) = q.fetch_one(&*pool).await?;

    let success_rate = if total_requests > 0 {
        ok_requests as f64 / total_requests as f64
    } else {
        0.0
    };

    Ok(DashboardStatsResponse {
        time_series,
        top_paths,
        top_ips,
        total_requests,
        success_rate,
        avg_latency_ms: avg_latency.unwrap_or(0.0),
    })
}
