use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::ConnectOptions;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::path::{Path, PathBuf};
use std::sync::Arc;

static DB_POOL: Lazy<RwLock<Option<Arc<SqlitePool>>>> = Lazy::new(|| RwLock::new(None));
static DB_PATH: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::new()));
static DB_ERROR: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(None));

static BLACKLIST_CACHE: Lazy<RwLock<HashMap<String, i64>>> = Lazy::new(|| RwLock::new(HashMap::new()));
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
    pub limit: i32,
    pub offset: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequestLogsResponse {
    pub logs: Vec<RequestLog>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RequestLog {
    pub id: i64,
    pub timestamp: i64,
    pub listen_addr: String,
    pub method: String,
    pub path: String,
    pub status: i32,
    pub latency_ms: f64,
    pub client_ip: String,
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

        // 建表：请求日志
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS request_logs (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              timestamp INTEGER NOT NULL,
              listen_addr TEXT NOT NULL,
              method TEXT NOT NULL,
              path TEXT NOT NULL,
              status INTEGER NOT NULL,
              latency_ms REAL NOT NULL,
              client_ip TEXT NOT NULL
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
    Ok(QueryMetricsResponse {
        data: vec![],
    })
}

pub async fn query_request_logs(req: QueryRequestLogsRequest) -> Result<QueryRequestLogsResponse> {
    let Some(pool) = pool() else {
        return Ok(QueryRequestLogsResponse { logs: vec![], total: 0 });
    };

    let limit = req.limit.clamp(1, 500) as i64;
    let offset = req.offset.max(0) as i64;

    let total: i64 = if let Some(listen_addr) = req.listen_addr.as_ref().filter(|s| !s.trim().is_empty()) {
            let row: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(1) as cnt FROM request_logs WHERE timestamp>=? AND timestamp<=? AND listen_addr=?"#,
            )
            .bind(req.start_time)
            .bind(req.end_time)
            .bind(listen_addr)
            .fetch_one(&*pool)
            .await?;
            row.0
        } else {
            let row: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(1) as cnt FROM request_logs WHERE timestamp>=? AND timestamp<=?"#,
            )
            .bind(req.start_time)
            .bind(req.end_time)
            .fetch_one(&*pool)
            .await?;
            row.0
        };

            let logs: Vec<RequestLog> = if let Some(listen_addr) = req.listen_addr.as_ref().filter(|s| !s.trim().is_empty()) {
            sqlx::query_as::<_, RequestLog>(
                r#"
                SELECT id, timestamp, listen_addr, method, path, status, latency_ms, client_ip
                FROM request_logs
                WHERE timestamp>=? AND timestamp<=? AND listen_addr=?
                ORDER BY timestamp DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(req.start_time)
            .bind(req.end_time)
            .bind(listen_addr)
            .bind(limit)
            .bind(offset)
            .fetch_all(&*pool)
            .await?
        } else {
            sqlx::query_as::<_, RequestLog>(
                r#"
                SELECT id, timestamp, listen_addr, method, path, status, latency_ms, client_ip
                FROM request_logs
                WHERE timestamp>=? AND timestamp<=?
                ORDER BY timestamp DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(req.start_time)
            .bind(req.end_time)
            .bind(limit)
            .bind(offset)
            .fetch_all(&*pool)
            .await?
        };

            Ok(QueryRequestLogsResponse { logs, total })
}

pub async fn add_blacklist_entry(ip: String, reason: String, duration_seconds: i32) -> Result<BlacklistEntry> {
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
    .bind(if reason.trim().is_empty() { None::<String> } else { Some(reason.clone()) })
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
        reason: if reason.trim().is_empty() { None } else { Some(reason) },
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
    // 清理过期
    sqlx::query(r#"DELETE FROM blacklist WHERE expires_at <= ?"#)
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
                    match std::fs::OpenOptions::new().create(true).write(true).open(&test) {
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
        message = Some("已启用持久化，但数据库尚未初始化；请保存配置或重启服务触发初始化".to_string());
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
