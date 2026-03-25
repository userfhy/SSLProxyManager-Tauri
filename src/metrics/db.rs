use super::*;

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
    let raw_path = PathBuf::from(s);
    let p = if raw_path.is_absolute() {
        raw_path
    } else {
        let exe = std::env::current_exe().with_context(|| "无法获取可执行文件路径")?;
        let dir = exe
            .parent()
            .ok_or_else(|| anyhow!("无法获取可执行文件所在目录"))?;
        dir.join(raw_path)
    };

    // 兼容用户将 db_path 配置为目录：
    // - 已存在目录：自动落到 <dir>/metrics.db
    // - 以路径分隔符结尾的输入：视为目录并自动补文件名
    let is_dir_path = p.is_dir() || s.ends_with('/') || s.ends_with('\\');
    if is_dir_path {
        Ok(p.join("metrics.db"))
    } else {
        Ok(p)
    }
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

#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
pub(crate) fn db_pool() -> Option<Arc<SqlitePool>> {
    DB_POOL.read().clone()
}

async fn maybe_vacuum_metrics_db(pool: &SqlitePool, deleted_rows: u64) {
    if deleted_rows < DB_SPACE_RECLAIM_MIN_DELETED_ROWS {
        return;
    }

    if DB_VACUUM_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    let should_try = {
        let guard = DB_LAST_VACUUM_AT.read();
        guard
            .as_ref()
            .map(|t| t.elapsed() >= DB_SPACE_RECLAIM_VACUUM_COOLDOWN)
            .unwrap_or(true)
    };
    if !should_try {
        DB_VACUUM_RUNNING.store(false, Ordering::SeqCst);
        return;
    }

    let page_count: i64 = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let freelist_count: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(pool)
        .await
        .unwrap_or(0);

    let free_ratio = if page_count > 0 {
        freelist_count as f64 / page_count as f64
    } else {
        0.0
    };

    if freelist_count >= DB_SPACE_RECLAIM_MIN_FREELIST_PAGES && free_ratio >= DB_SPACE_RECLAIM_MIN_FREELIST_RATIO {
        if sqlx::query("VACUUM").execute(pool).await.is_ok() {
            *DB_LAST_VACUUM_AT.write() = Some(Instant::now());
        }
    }

    DB_VACUUM_RUNNING.store(false, Ordering::SeqCst);
}

pub(crate) async fn reclaim_db_space_after_delete(pool: &SqlitePool, deleted_rows: u64) {
    if deleted_rows == 0 {
        return;
    }

    // 先回收 WAL 文件空间，再按阈值决定是否 VACUUM 主库文件。
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await;

    maybe_vacuum_metrics_db(pool, deleted_rows).await;
}

pub fn deinit_db() {
    *DB_POOL.write() = None;
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

        let mut opt = SqliteConnectOptions::new().filename(&path);
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

        // 连接级 PRAGMA（提升稳定性/性能）
        // - busy_timeout：避免高并发下立即报 database is locked
        // - cache_size：增大 page cache（负数表示 KB）
        // - temp_store：临时表尽量走内存
        // 性能优化 PRAGMA 设置
        let _ = sqlx::query("PRAGMA busy_timeout = 5000;")
            .execute(&pool)
            .await;
        // 增大缓存到 64MB（-64000 KB）以提升查询性能
        let _ = sqlx::query("PRAGMA cache_size = -64000;")
            .execute(&pool)
            .await;
        let _ = sqlx::query("PRAGMA temp_store = MEMORY;")
            .execute(&pool)
            .await;
        // 启用 mmap 以提升读取性能（256MB）
        let _ = sqlx::query("PRAGMA mmap_size = 268435456;")
            .execute(&pool)
            .await;

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

        // 常用查询条件索引（提升日志筛选/统计性能）
        sqlx::query(r#"CREATE INDEX IF NOT EXISTS idx_request_logs_status_ts ON request_logs(status_code, timestamp);"#)
            .execute(&pool)
            .await
            .context("创建 request_logs.status_code+timestamp 索引失败")?;

        sqlx::query(r#"CREATE INDEX IF NOT EXISTS idx_request_logs_route_ts ON request_logs(matched_route_id, timestamp);"#)
            .execute(&pool)
            .await
            .context("创建 request_logs.matched_route_id+timestamp 索引失败")?;

        sqlx::query(r#"CREATE INDEX IF NOT EXISTS idx_request_logs_client_ip_ts ON request_logs(client_ip, timestamp);"#)
            .execute(&pool)
            .await
            .context("创建 request_logs.client_ip+timestamp 索引失败")?;

        // request_path/upstream 常用于 LIKE/分组，索引对 LIKE %...% 帮助有限，但对分组与前缀匹配仍有收益
        sqlx::query(r#"CREATE INDEX IF NOT EXISTS idx_request_logs_path_ts ON request_logs(request_path, timestamp);"#)
            .execute(&pool)
            .await
            .context("创建 request_logs.request_path+timestamp 索引失败")?;

        sqlx::query(r#"CREATE INDEX IF NOT EXISTS idx_request_logs_upstream_ts ON request_logs(upstream, timestamp);"#)
            .execute(&pool)
            .await
            .context("创建 request_logs.upstream+timestamp 索引失败")?;

        // 系统指标历史表
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS system_metrics (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              timestamp INTEGER NOT NULL,
              cpu_usage_percent REAL NOT NULL,
              load1 REAL NOT NULL,
              load5 REAL NOT NULL,
              load15 REAL NOT NULL,
              mem_total_bytes INTEGER NOT NULL,
              mem_available_bytes INTEGER NOT NULL,
              mem_used_bytes INTEGER NOT NULL,
              mem_used_percent REAL NOT NULL,
              swap_total_bytes INTEGER NOT NULL,
              swap_free_bytes INTEGER NOT NULL,
              swap_used_bytes INTEGER NOT NULL,
              swap_used_percent REAL NOT NULL,
              net_rx_bytes INTEGER NOT NULL,
              net_tx_bytes INTEGER NOT NULL,
              net_rx_bps REAL NOT NULL,
              net_tx_bps REAL NOT NULL,
              disk_read_bytes INTEGER NOT NULL,
              disk_write_bytes INTEGER NOT NULL,
              disk_read_bps REAL NOT NULL,
              disk_write_bps REAL NOT NULL,
              tcp_established INTEGER NOT NULL,
              tcp_time_wait INTEGER NOT NULL,
              tcp_close_wait INTEGER NOT NULL,
              process_count INTEGER NOT NULL,
              fd_used INTEGER NOT NULL,
              fd_max INTEGER NOT NULL,
              fd_usage_percent REAL NOT NULL,
              procs_running INTEGER NOT NULL,
              procs_blocked INTEGER NOT NULL,
              context_switches INTEGER NOT NULL,
              processes_forked_total INTEGER NOT NULL,
              uptime_seconds REAL NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .context("创建 system_metrics 表失败")?;

        sqlx::query(
            r#"CREATE INDEX IF NOT EXISTS idx_system_metrics_ts ON system_metrics(timestamp);"#,
        )
        .execute(&pool)
        .await
        .context("创建 system_metrics.timestamp 索引失败")?;

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_logs_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_logs_min_ts: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_logs_max_ts: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub db_file_size_bytes: Option<i64>,

    // --- SQLite 参数（通过 PRAGMA 读取）---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sqlite_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synchronous: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wal_autocheckpoint: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freelist_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub busy_timeout_ms: Option<i64>,

    // --- WAL/SHM 文件大小（字节）---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wal_file_size_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shm_file_size_bytes: Option<i64>,
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

    let request_logs_count: Option<i64> = None;
    let request_logs_min_ts: Option<i64> = None;
    let request_logs_max_ts: Option<i64> = None;

    // 注意：此函数是同步函数（给 tauri command 同步调用）。
    // 为避免在非 async 里 await，这里不直接查 DB。
    // 如果需要实时行数/时间范围，请使用后续可扩展的 async 版本接口。

    let db_file_size_bytes: Option<i64> = if file_exists {
        std::fs::metadata(&path).ok().map(|m| m.len() as i64)
    } else {
        None
    };

    MetricsDBStatus {
        enabled: initialized,
        initialized,
        path,
        error: DB_ERROR.read().clone(),
        file_exists,
        dir_exists,
        dir_writable,
        message,
        request_logs_count,
        request_logs_min_ts,
        request_logs_max_ts,
        db_file_size_bytes,
        sqlite_version: None,
        journal_mode: None,
        synchronous: None,
        wal_autocheckpoint: None,
        page_size: None,
        page_count: None,
        freelist_count: None,
        cache_size: None,
        busy_timeout_ms: None,
        wal_file_size_bytes: None,
        shm_file_size_bytes: None,
    }
}

pub async fn get_metrics_db_status_detail() -> Result<MetricsDBStatus> {
    let base = get_metrics_db_status();
    if !base.initialized {
        return Ok(base);
    }

    let Some(pool) = pool() else {
        return Ok(base);
    };

    let (cnt, min_ts, max_ts) = sqlx::query_as::<_, (i64, Option<i64>, Option<i64>)>(
        "SELECT COUNT(1) AS cnt, MIN(timestamp) AS min_ts, MAX(timestamp) AS max_ts FROM request_logs",
    )
    .fetch_one(&*pool)
    .await?;

    let sqlite_version: Option<String> = sqlx::query_scalar("SELECT sqlite_version()")
        .fetch_one(&*pool)
        .await
        .ok();

    let journal_mode: Option<String> = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&*pool)
        .await
        .ok();

    let synchronous: Option<String> = sqlx::query_scalar("PRAGMA synchronous")
        .fetch_one(&*pool)
        .await
        .ok();

    let wal_autocheckpoint: Option<i64> = sqlx::query_scalar("PRAGMA wal_autocheckpoint")
        .fetch_one(&*pool)
        .await
        .ok();

    let page_size: Option<i64> = sqlx::query_scalar("PRAGMA page_size")
        .fetch_one(&*pool)
        .await
        .ok();

    let page_count: Option<i64> = sqlx::query_scalar("PRAGMA page_count")
        .fetch_one(&*pool)
        .await
        .ok();

    let freelist_count: Option<i64> = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(&*pool)
        .await
        .ok();

    let cache_size: Option<i64> = sqlx::query_scalar("PRAGMA cache_size")
        .fetch_one(&*pool)
        .await
        .ok();

    let busy_timeout_ms: Option<i64> = sqlx::query_scalar("PRAGMA busy_timeout")
        .fetch_one(&*pool)
        .await
        .ok();

    let wal_file_size_bytes: Option<i64> = if base.file_exists {
        std::fs::metadata(format!("{}-wal", base.path)).ok().map(|m| m.len() as i64)
    } else {
        None
    };

    let shm_file_size_bytes: Option<i64> = if base.file_exists {
        std::fs::metadata(format!("{}-shm", base.path)).ok().map(|m| m.len() as i64)
    } else {
        None
    };

    Ok(MetricsDBStatus {
        request_logs_count: Some(cnt),
        request_logs_min_ts: min_ts,
        request_logs_max_ts: max_ts,
        sqlite_version,
        journal_mode,
        synchronous,
        wal_autocheckpoint,
        page_size,
        page_count,
        freelist_count,
        cache_size,
        busy_timeout_ms,
        wal_file_size_bytes,
        shm_file_size_bytes,
        ..base
    })
}

pub async fn test_metrics_db_connection(db_path: String) -> Result<(bool, String)> {
    let path = resolve_db_path(db_path)?;
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .with_context(|| format!("创建数据库目录失败: {}", dir.display()))?;
    }
    let mut opt = SqliteConnectOptions::new().filename(&path);
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

pub(super) async fn refresh_blacklist_cache_internal(pool: &SqlitePool) -> Result<()> {
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
pub(super) static REQUEST_LOG_TX: Lazy<RwLock<Option<tokio::sync::mpsc::Sender<RequestLogInsert>>>> =
    Lazy::new(|| RwLock::new(None));
