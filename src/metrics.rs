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
    pub data: Vec<MetricsDataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsDataPoint {
    pub timestamp: i64,
    pub count: i64,
    pub status_2xx: i64,
    pub status_3xx: i64,
    pub status_4xx: i64,
    pub status_5xx: i64,
    pub avg_latency: f64,
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
    pub avg_latency: Vec<f64>,
    pub max_latency: Vec<f64>,
    pub p95: Vec<f64>,
    pub p99: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsPayload {
    pub window_seconds: i32,
    pub listen_addrs: Vec<String>,
    pub by_listen_addr: HashMap<String, MetricsSeries>,
    pub minute_window_seconds: i32,
    pub by_listen_minute: HashMap<String, MetricsSeries>,
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
    let result = async move {
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

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_blacklist_expires ON blacklist(expires_at);"#,
        )
        .execute(&pool)
        .await
        .context("创建 blacklist.expires_at 索引失败")?;

        // 写入全局状态
        *DB_PATH.write() = path.to_string_lossy().to_string();
        *DB_POOL.write() = Some(Arc::new(pool));
        *DB_ERROR.write() = None;

        // 初始化黑名单缓存
        refresh_blacklist_cache().await.ok();

        Ok::<(), anyhow::Error>(())
    };

    match result.await {
        Ok(()) => {
            *DB_ERROR.write() = None;
            Ok(())
        }
        Err(e) => {
            *DB_POOL.write() = None;
            *DB_ERROR.write() = Some(e.to_string());
            Err(e)
        }
    }
}

pub fn get_metrics() -> MetricsPayload {
    // 返回当前指标数据
    MetricsPayload {
        window_seconds: 21600,
        listen_addrs: vec![],
        by_listen_addr: HashMap::new(),
        minute_window_seconds: 86400,
        by_listen_minute: HashMap::new(),
    }
}

pub fn query_historical_metrics(req: QueryMetricsRequest) -> Result<QueryMetricsResponse> {
    // 查询历史指标数据
    Ok(QueryMetricsResponse { data: vec![] })
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
        "SELECT id, timestamp, listen_addr, client_ip, remote_ip, method, request_path, request_host, status_code, upstream, latency_ms, user_agent, referer\n         FROM request_logs\n         WHERE timestamp>=? AND timestamp<=?",
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

pub fn try_enqueue_request_log(log: RequestLogInsert) {
    let Some(tx) = REQUEST_LOG_TX.read().clone() else {
        return;
    };
    // 满了就丢弃，避免反压影响代理性能
    let _ = tx.try_send(log);
}

pub async fn init_request_log_writer() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<RequestLogInsert>(50_000);
    *REQUEST_LOG_TX.write() = Some(tx);

    tauri::async_runtime::spawn(async move {
        let mut buf: Vec<RequestLogInsert> = Vec::with_capacity(256);
        let mut ticker = tokio::time::interval(Duration::from_millis(500));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    flush_request_logs(&mut buf).await;
                }
                item = rx.recv() => {
                    match item {
                        None => {
                            // sender 全部 drop，flush 后退出
                            flush_request_logs(&mut buf).await;
                            break;
                        }
                        Some(v) => {
                            buf.push(v);
                            if buf.len() >= 200 {
                                flush_request_logs(&mut buf).await;
                            }
                        }
                    }
                }
            }
        }
    });
}

static REQUEST_LOG_TX: Lazy<RwLock<Option<tokio::sync::mpsc::Sender<RequestLogInsert>>>> =
    Lazy::new(|| RwLock::new(None));

async fn flush_request_logs(buf: &mut Vec<RequestLogInsert>) {
    if buf.is_empty() {
        return;
    }

    let Some(pool) = pool() else {
        buf.clear();
        return;
    };

    // 批量写入
    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(_) => {
            buf.clear();
            return;
        }
    };

    for row in buf.drain(..) {
        let _ = sqlx::query(
            r#"
            INSERT INTO request_logs (
              timestamp, listen_addr, client_ip, remote_ip, method, request_path, request_host,
              status_code, upstream, latency_ms, user_agent, referer
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row.timestamp)
        .bind(row.listen_addr)
        .bind(row.client_ip)
        .bind(row.remote_ip)
        .bind(row.method)
        .bind(row.request_path)
        .bind(row.request_host)
        .bind(row.status_code)
        .bind(row.upstream)
        .bind(row.latency_ms)
        .bind(row.user_agent)
        .bind(row.referer)
        .execute(&mut *tx)
        .await;
    }

    let _ = tx.commit().await;
}

pub async fn add_blacklist_entry(
    ip: String,
    reason: String,
    duration_seconds: i32,
) -> Result<BlacklistEntry> {
    let Some(pool) = pool() else {
        return Err(anyhow!("数据库未初始化"));
    };

    let ip_key = normalize_ip_key(&ip);
    let now = chrono::Utc::now().timestamp();
    let expires_at = if duration_seconds <= 0 {
        0
    } else {
        now + (duration_seconds as i64)
    };

    // upsert
    let res = sqlx::query(
        r#"
        INSERT INTO blacklist (ip, reason, expires_at, created_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(ip) DO UPDATE SET
          reason=excluded.reason,
          expires_at=excluded.expires_at
        "#,
    )
    .bind(&ip_key)
    .bind(if reason.trim().is_empty() {
        None::<String>
    } else {
        Some(reason.clone())
    })
    .bind(expires_at)
    .bind(now)
    .execute(&*pool)
    .await?;

    let id = res.last_insert_rowid();

    // 更新缓存
    BLACKLIST_CACHE.write().insert(ip_key.clone(), expires_at);

    Ok(BlacklistEntry {
        id,
        ip: ip_key,
        reason: if reason.trim().is_empty() {
            None
        } else {
            Some(reason)
        },
        expires_at,
        created_at: now,
    })
}

pub async fn remove_blacklist_entry(ip: String) -> Result<()> {
    let Some(pool) = pool() else {
        return Err(anyhow!("数据库未初始化"));
    };

    let ip_key = normalize_ip_key(&ip);
    sqlx::query(r#"DELETE FROM blacklist WHERE ip=?"#)
        .bind(&ip_key)
        .execute(&*pool)
        .await?;

    BLACKLIST_CACHE.write().remove(&ip_key);
    Ok(())
}

pub async fn get_blacklist_entries() -> Result<Vec<BlacklistEntry>> {
    let Some(pool) = pool() else {
        return Ok(vec![]);
    };

    let rows: Vec<BlacklistEntry> = sqlx::query_as::<_, BlacklistEntry>(
        r#"SELECT id, ip, reason, expires_at, created_at FROM blacklist ORDER BY created_at DESC"#,
    )
    .fetch_all(&*pool)
    .await?;
    Ok(rows)
}

pub async fn refresh_blacklist_cache() -> Result<()> {
    let Some(pool) = pool() else {
        BLACKLIST_CACHE.write().clear();
        return Ok(());
    };

    let now = chrono::Utc::now().timestamp();
    // 清理过期（expires_at=0 表示永久拉黑，不能被清理）
    sqlx::query(r#"DELETE FROM blacklist WHERE expires_at != 0 AND expires_at <= ?"#)
        .bind(now)
        .execute(&*pool)
        .await?;

    let ips: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT ip, expires_at FROM blacklist WHERE expires_at = 0 OR expires_at > ?"#,
    )
    .bind(now)
    .fetch_all(&*pool)
    .await?;

    let mut cache = BLACKLIST_CACHE.write();
    cache.clear();
    for (ip, expires_at) in ips {
        cache.insert(normalize_ip_key(&ip), expires_at);
    }

    *BLACKLIST_LAST_CLEANUP.write() = Instant::now();
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
    let cfg = crate::config::get_config();
    let enabled = cfg
        .metrics_storage
        .as_ref()
        .map(|m| m.enabled)
        .unwrap_or(false);

    let configured_path = cfg
        .metrics_storage
        .as_ref()
        .map(|m| m.db_path.clone())
        .unwrap_or_default();

    let resolved = resolve_db_path(configured_path.clone());

    let (path, dir_exists, dir_writable, file_exists, mut error, mut message) = match resolved {
        Ok(p) => {
            let dir = p.parent().map(|d| d.to_path_buf());
            let file_exists = p.exists();
            let (dir_exists, dir_writable) = if let Some(dir) = dir {
                let exists = dir.exists();
                let writable = if exists {
                    // 以创建临时文件的方式判断可写
                    let test = dir.join(".writable_test");
                    match std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .open(&test)
                    {
                        Ok(_) => {
                            let _ = std::fs::remove_file(&test);
                            true
                        }
                        Err(_) => false,
                    }
                } else {
                    false
                };
                (exists, writable)
            } else {
                (false, false)
            };

            (
                p.to_string_lossy().to_string(),
                dir_exists,
                dir_writable,
                file_exists,
                None,
                None,
            )
        }
        Err(e) => (
            String::new(),
            false,
            false,
            false,
            Some(e.to_string()),
            None,
        ),
    };

    let initialized = pool().is_some();

    // 记录 init_db 过程中的错误（如果有）
    if error.is_none() {
        if let Some(e) = DB_ERROR.read().clone() {
            error = Some(e);
        }
    }

    if enabled && !initialized && error.is_none() {
        message =
            Some("已启用持久化，但数据库尚未初始化；请保存配置或重启服务触发初始化".to_string());
    }

    MetricsDBStatus {
        enabled,
        initialized,
        path,
        error,
        file_exists,
        dir_exists,
        dir_writable,
        message,
    }
}

pub async fn test_metrics_db_connection(db_path: String) -> Result<(bool, String)> {
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

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opt)
        .await
        .with_context(|| format!("连接数据库失败: {}", path.display()))?;

    let row: (i64,) = sqlx::query_as("SELECT 1")
        .fetch_one(&pool)
        .await
        .context("测试查询失败")?;

    if row.0 != 1 {
        return Ok((false, "测试查询返回异常".to_string()));
    }

    Ok((true, "连接成功".to_string()))
}
