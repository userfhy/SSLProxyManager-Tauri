use crate::{access_control, config, metrics, ws_proxy, stream_proxy};
use anyhow::{anyhow, Context, Result};
use axum::{
    body::Body,
    extract::{connect_info::ConnectInfo, FromRef, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use parking_lot::RwLock;
use reqwest::redirect::Policy;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

const LOG_QUEUE_CAPACITY: usize = 10_000;

static LOG_TX: once_cell::sync::Lazy<RwLock<Option<tokio::sync::mpsc::Sender<String>>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(None));

static LOG_DROPPED: once_cell::sync::Lazy<std::sync::atomic::AtomicU64> =
    once_cell::sync::Lazy::new(|| std::sync::atomic::AtomicU64::new(0));
use tauri::Emitter;
use tower::util::ServiceExt;
use tower_http::services::ServeDir;
use tracing::{error, info};

static IS_RUNNING: RwLock<bool> = RwLock::new(false);
struct ServerHandle {
    handle: tauri::async_runtime::JoinHandle<()>,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl ServerHandle {
    fn abort(self) {
        let _ = self.shutdown_tx.send(());
        self.handle.abort();
    }
}

static SERVERS: RwLock<Vec<ServerHandle>> = RwLock::new(Vec::new());
static LOGS: RwLock<Vec<String>> = RwLock::new(Vec::new());

// 启动过程控制：要求所有 rules 都成功（你选的语义 B）
static STARTING: RwLock<bool> = RwLock::new(false);
static START_EXPECTED: RwLock<usize> = RwLock::new(0);
static START_FAILED: RwLock<bool> = RwLock::new(false);
static START_STARTED_COUNT: RwLock<usize> = RwLock::new(0);

#[derive(Debug, Clone)]
struct SmoothUpstream {
    url: String,
    weight: i32,
    current: i32,
}

#[derive(Debug, Clone)]
struct SmoothLbState {
    signature: String,
    total_weight: i32,
    upstreams: Vec<SmoothUpstream>,
}

static UPSTREAM_LB: once_cell::sync::Lazy<RwLock<HashMap<String, Arc<RwLock<SmoothLbState>>>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Clone)]
struct AppState {
    rule: config::ListenRule,
    client_follow: reqwest::Client,
    client_nofollow: reqwest::Client,
    app: tauri::AppHandle,
}

#[derive(Clone)]
struct RuleState(Arc<config::ListenRule>);

#[derive(Clone)]
struct HttpClientFollow(reqwest::Client);

#[derive(Clone)]
struct HttpClientNoFollow(reqwest::Client);

impl FromRef<AppState> for RuleState {
    fn from_ref(input: &AppState) -> Self {
        Self(Arc::new(input.rule.clone()))
    }
}

impl FromRef<AppState> for HttpClientFollow {
    fn from_ref(input: &AppState) -> Self {
        Self(input.client_follow.clone())
    }
}

impl FromRef<AppState> for HttpClientNoFollow {
    fn from_ref(input: &AppState) -> Self {
        Self(input.client_nofollow.clone())
    }
}

#[derive(Clone)]
struct AppHandleState(tauri::AppHandle);

impl FromRef<AppState> for AppHandleState {
    fn from_ref(input: &AppState) -> Self {
        Self(input.app.clone())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleStartErrorPayload {
    pub listen_addr: String,
    pub error: String,
}

pub fn start_server(app: tauri::AppHandle) -> Result<()> {
    // 启动 access log 异步写入任务（仅启动一次）
    init_log_task(app.clone());

    // 启动 WS 独立监听（如果启用）。
    // WS 的启动/失败目前不参与 HTTP rules 的启动汇总逻辑；如果端口占用，会在日志中提示。
    if let Err(e) = ws_proxy::start_ws_servers(app.clone()) {
        send_log(format!("启动 WS 监听器失败: {e}"));
    }

    // 启动 Stream(TCP/UDP) 代理监听（如果启用）。
    // Stream 的启动同样不参与 HTTP rules 的启动汇总逻辑；如果端口占用，会在日志中提示。
    // 注意：async_runtime::spawn 要求 Future 是 Send；因此这里只捕获 stream 配置的 clone，
    // 避免把整个 Config（可能间接携带 !Send 状态）带进异步任务。
    {
        let stream_cfg = config::get_config().stream.clone();
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = stream_proxy::start_stream_servers(&stream_cfg).await {
                send_log_with_app(&app2, format!("启动 Stream 监听器失败: {e}"));
            }
        });
    }
    // 如果正在启动，直接返回（避免重复点击并发启动）
    {
        let starting = STARTING.read();
        if *starting {
            return Ok(());
        }
    }

    // 标记启动中，并初始化启动计数
    *STARTING.write() = true;
    *START_FAILED.write() = false;

    let cfg = config::get_config();
    let rules = cfg.rules;
    let expected = rules.len();
    *START_EXPECTED.write() = expected;
    *START_STARTED_COUNT.write() = 0;

    // 没有任何 rule：视为停止态
    if expected == 0 {
        *IS_RUNNING.write() = false;
        *STARTING.write() = false;
        let _ = app.emit("status", "stopped");
        send_log("未配置监听规则，服务保持停止状态".to_string());
        return Ok(());
    }

    // 提前发一个“stopped”，前端会进入等待态，避免误判成功
    let _ = app.emit("status", "stopped");

    let mut handles = Vec::new();

    for rule in rules {
        let app_handle = app.clone();
        let listen_addr = rule.listen_addr.clone();
        // 创建用于优雅停机的 oneshot 通道
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let handle = tauri::async_runtime::spawn(async move {
            // 只要 start_rule_server 通过了 bind 阶段，就认为该 rule 启动成功
            // 先做一次 bind 预检：确保端口/证书等关键错误能在启动阶段暴露出来。
            // 语义 B：任何 rule 预检失败 => 整体启动失败。
            if let Err(e) = precheck_rule(&rule).await {
                error!("启动监听器失败({listen_addr}): {e}");
                send_log(format!("启动监听器失败({listen_addr}): {e}"));

                let payload = RuleStartErrorPayload {
                    listen_addr,
                    error: e.to_string(),
                };
                let _ = app_handle.emit("server-start-error", payload);

                *START_FAILED.write() = true;
                *IS_RUNNING.write() = false;
                *STARTING.write() = false;
                let _ = app_handle.emit("status", "stopped");
                return;
            }

            // 预检通过：计数 + 如果全部通过则进入 running，并发出 status 事件
            {
                let mut started = START_STARTED_COUNT.write();
                *started += 1;
                let expected = *START_EXPECTED.read();
                let failed = *START_FAILED.read();
                if !failed && *started == expected {
                    *IS_RUNNING.write() = true;
                    *STARTING.write() = false;
                    let _ = app_handle.emit("status", "running");
                    // 获取最新的配置来生成详细的启动日志
                    let final_cfg = config::get_config();
                    for r in &final_cfg.rules {
                        let routes_summary = r
                            .routes
                            .iter()
                            .map(|rt| {
                                format!(
                                    "{} -> {} upstreams",
                                    rt.path.as_deref().unwrap_or("/"),
                                    rt.upstreams.len()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        let log_line = format!(
                            "[NODE {}] Server started | SSL: {} | Routes: [{}] | Allow all LAN: {}",
                            r.listen_addr, r.ssl_enable, routes_summary, final_cfg.allow_all_lan
                        );
                        send_log_with_app(&app_handle, log_line);
                    }
                }
            }

            // 正式启动：进入 serve loop（直到 stop_server abort）
            match start_rule_server(app_handle.clone(), rule, shutdown_rx).await {
                Ok(()) => {
                    // server 正常退出（一般不会发生，除非 stop_server abort 后）
                }
                Err(e) => {
                    error!("启动监听器失败({listen_addr}): {e}");
                    send_log(format!("启动监听器失败({listen_addr}): {e}"));

                    // 通知前端：某个 rule 启动失败（例如端口占用）
                    let payload = RuleStartErrorPayload {
                        listen_addr,
                        error: e.to_string(),
                    };
                    let _ = app_handle.emit("server-start-error", payload);

                    // 语义 B：任意 rule 失败 -> 整体失败
                    *START_FAILED.write() = true;
                    *IS_RUNNING.write() = false;
                    *STARTING.write() = false;
                    let _ = app_handle.emit("status", "stopped");
                }
            }
        });
        handles.push(ServerHandle { handle, shutdown_tx });
    }

    *SERVERS.write() = handles;

    // 这里不再设置 IS_RUNNING=true，也不提示“已启动”。
    // 由每个 rule 真正 bind 成功后汇总决定。
    send_log("代理服务器启动中...".to_string());
    Ok(())
}

pub fn stop_server(app: tauri::AppHandle) -> Result<()> {
    // 停止 WS 独立监听
    ws_proxy::stop_ws_servers();

    // 停止 access log 异步任务：drop sender 让后台任务退出；下次 start 会重新 init
    *LOG_TX.write() = None;

    // 停止 Stream(TCP/UDP) 代理监听
    tauri::async_runtime::spawn(async {
        stream_proxy::stop_stream_servers().await;
    });

    // 无论当前状态如何，停止都要尽量清理启动中的状态
    *STARTING.write() = false;
    *START_FAILED.write() = false;
    *START_EXPECTED.write() = 0;
    *START_STARTED_COUNT.write() = 0;

    // 无论 running 标志如何，都尽量 abort 掉已有的 listener 任务，确保“重启”能真正生效。
    // 否则在某些状态机边界情况下（例如 running 标志未及时置位），会导致旧服务继续运行。
    *IS_RUNNING.write() = false;

    let handles = std::mem::take(&mut *SERVERS.write());
    for handle in handles {
        handle.abort();
    }

    let _ = app.emit("status", "stopped");

    // 生成详细的停止日志（每个监听节点一条）
    let cfg = config::get_config();
    for r in &cfg.rules {
        let log_line = format!("[NODE {}] Server stopped", r.listen_addr);
        send_log_with_app(&app, log_line);
    }

    info!("代理服务器已停止");
    Ok(())
}

pub fn is_running() -> bool {
    *IS_RUNNING.read()
}

pub fn is_starting() -> bool {
    *STARTING.read()
}

pub fn is_effectively_running() -> bool {
    // is_running 在异步启动阶段可能仍为 false，因此这里把“启动中”也视为运行态，
    // 以便托盘/按钮等 UI 不会出现与真实状态明显背离的情况。
    is_running() || is_starting()
}

pub fn get_logs() -> Vec<String> {
    LOGS.read().clone()
}

pub fn clear_logs() {
    LOGS.write().clear();
}

pub fn send_log(message: String) {
    let mut logs = LOGS.write();
    logs.push(message);
    if logs.len() > 3000 {
        let over = logs.len() - 3000;
        logs.drain(0..over);
    }
}

pub fn send_log_with_app(app: &tauri::AppHandle, message: String) {
    // 写入内存（环形缓冲）
    {
        let mut logs = LOGS.write();
        logs.push(message.clone());
        if logs.len() > 3000 {
            let over = logs.len() - 3000;
            logs.drain(0..over);
        }
    }

    // 推送前端（受配置控制）
    let cfg = config::get_config();
    if !cfg.show_realtime_logs {
        return;
    }

    if cfg.realtime_logs_only_errors {
        // 非错误日志不推送
        let lower = message.to_ascii_lowercase();
        if !(lower.contains("error")
            || lower.contains("failed")
            || lower.contains("异常")
            || lower.contains("失败"))
        {
            return;
        }
    }

    let _ = app.emit("log-line", message);
}

async fn precheck_rule(rule: &config::ListenRule) -> Result<()> {
    let addr = parse_listen_addr(&rule.listen_addr)?;

    if rule.ssl_enable {
        // 证书/私钥读失败会直接返回错误
        let _ = axum_server::tls_rustls::RustlsConfig::from_pem_file(
            rule.cert_file.clone(),
            rule.key_file.clone(),
        )
        .await
        .with_context(|| "加载 TLS 证书/私钥失败")?;

        // TLS bind 的预检：尝试 bind 后立刻 drop
        let listener = tokio::net::TcpListener::bind(addr).await?;
        drop(listener);
    } else {
        // 非 TLS：尝试 bind 后立刻 drop
        let listener = tokio::net::TcpListener::bind(addr).await?;
        drop(listener);
    }

    Ok(())
}

async fn start_rule_server(
    app: tauri::AppHandle,
    rule: config::ListenRule,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let addr = parse_listen_addr(&rule.listen_addr)?;

    let cfg = crate::config::get_config();

    let client_builder = || {
        let mut builder = reqwest::Client::builder()
            .redirect(Policy::limited(10))
            .danger_accept_invalid_certs(true)
            .pool_max_idle_per_host(cfg.upstream_pool_max_idle)
            .pool_idle_timeout(Duration::from_secs(cfg.upstream_pool_idle_timeout_sec))
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)  // 降低小包延迟
            .connect_timeout(Duration::from_millis(cfg.upstream_connect_timeout_ms))
            .timeout(Duration::from_millis(cfg.upstream_read_timeout_ms));

        if !cfg.enable_http2 {
            builder = builder.http1_only();
        }

        builder
    };

    // 创建两个 client 实例：一个跟随重定向，一个不跟随
    let follow_builder = client_builder();
    let nofollow_builder = client_builder().redirect(Policy::none());

    let client_follow = follow_builder
        .build()
        .context("创建上游 HTTP client 失败")?;

    let client_nofollow = nofollow_builder
        .build()
        .context("创建上游 HTTP client 失败")?;

    let state = AppState {
        rule: rule.clone(),
        client_follow,
        client_nofollow,
        app: app.clone(),
    };

    let router = Router::new().route("/healthz", any(healthz));

    // 统一由 proxy_handler 处理：
    // - static_dir: 静态文件优先，页面路由（无扩展名）SPA 回退 index.html，资源缺失返回 404
    // - upstreams: 仅在有 upstream 时才反代
    let app = router.fallback(any(proxy_handler)).with_state(state);

    // 让 proxy_handler 能拿到真实远端地址（用于访问控制）
    let app = app.into_make_service_with_connect_info::<SocketAddr>();

    send_log(format!("监听地址: {} -> {}", rule.listen_addr, addr));
    info!("监听地址: {} -> {}", rule.listen_addr, addr);

    if rule.ssl_enable {
        let tls_cfg = axum_server::tls_rustls::RustlsConfig::from_pem_file(
            rule.cert_file.clone(),
            rule.key_file.clone(),
        )
        .await
        .with_context(|| "加载 TLS 证书/私钥失败")?;

        send_log(format!("HTTPS 已启用: {}", addr));

        let mut shutdown_rx = shutdown_rx;
        let server_future = axum_server::bind_rustls(addr, tls_cfg).serve(app);
        tokio::select! {
            res = server_future => {
                res.map_err(|e| anyhow!(e))?;
            }
            _ = &mut shutdown_rx => {
                info!("收到关闭信号，HTTPS 服务 {} 即将停止", addr);
            }
        }
    } else {
        send_log(format!("HTTP 已启用: {}", addr));
        let mut shutdown_rx = shutdown_rx;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let server_future = axum::serve(listener, app);
        tokio::select! {
            res = server_future => {
                res.map_err(|e| anyhow!(e))?;
            }
            _ = &mut shutdown_rx => {
                info!("收到关闭信号，HTTP 服务 {} 即将停止", addr);
            }
        }
    }

    Ok(())
}

fn parse_listen_addr(s: &str) -> Result<SocketAddr> {
    let trimmed = s.trim();
    let normalized = if trimmed.starts_with(':') {
        format!("0.0.0.0{}", trimmed)
    } else {
        trimmed.to_string()
    };

    normalized
        .parse::<SocketAddr>()
        .with_context(|| format!("解析 listen_addr 失败: {s}"))
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

fn match_route<'a>(routes: &'a [config::Route], path: &str) -> Option<&'a config::Route> {
    routes
        .iter()
        .filter_map(|r| r.path.as_deref().map(|p| (p, r)))
        .filter(|(p, _)| path.starts_with(p))
        .max_by_key(|(p, _)| p.len())
        .map(|(_, r)| r)
}

fn upstream_signature(route: &config::Route) -> String {
    // 用 url+weight 生成签名，配置变化时会触发状态重建
    let mut parts: Vec<String> = route
        .upstreams
        .iter()
        .map(|u| format!("{}#{}", u.url, u.weight))
        .collect();
    parts.sort();
    parts.join("|")
}

fn pick_upstream_smooth(route: &config::Route) -> Option<String> {
    if route.upstreams.is_empty() {
        return None;
    }
    if route.upstreams.len() == 1 {
        return Some(route.upstreams[0].url.clone());
    }

    let route_id = route.id.as_deref().unwrap_or("").trim();
    if route_id.is_empty() {
        // 理论上不会发生：保存配置时会补齐 id。
        // 为避免出错，回退到第一个。
        return Some(route.upstreams[0].url.clone());
    }

    let sig = upstream_signature(route);

    // route 粒度锁：map 只保存 Arc<RwLock<State>>，避免每次选 upstream 都拿全局写锁
    let state_lock: Arc<RwLock<SmoothLbState>> = {
        // 先尝试读锁命中
        if let Some(s) = UPSTREAM_LB.read().get(route_id).cloned() {
            s
        } else {
            // 未命中：写锁插入（只在首次出现 route_id 时发生）
            let mut map = UPSTREAM_LB.write();
            map.entry(route_id.to_string())
                .or_insert_with(|| {
                    Arc::new(RwLock::new(SmoothLbState {
                        signature: String::new(),
                        total_weight: 0,
                        upstreams: Vec::new(),
                    }))
                })
                .clone()
        }
    };

    let mut entry = state_lock.write();

    if entry.signature != sig || entry.upstreams.len() != route.upstreams.len() {
        let ups: Vec<SmoothUpstream> = route
            .upstreams
            .iter()
            .map(|u| SmoothUpstream {
                url: u.url.clone(),
                weight: std::cmp::max(1, u.weight),
                current: 0,
            })
            .collect();
        let total = ups.iter().map(|u| u.weight).sum::<i32>();

        entry.signature = sig;
        entry.total_weight = std::cmp::max(1, total);
        entry.upstreams = ups;
    }

    // 平滑加权轮询：current += weight，选 current 最大者，选中者 current -= total
    let mut best_idx = 0usize;
    for i in 0..entry.upstreams.len() {
        let w = entry.upstreams[i].weight;
        entry.upstreams[i].current = entry.upstreams[i].current.saturating_add(w);
        if entry.upstreams[i].current > entry.upstreams[best_idx].current {
            best_idx = i;
        }
    }

    entry.upstreams[best_idx].current = entry.upstreams[best_idx]
        .current
        .saturating_sub(entry.total_weight);

    Some(entry.upstreams[best_idx].url.clone())
}

fn is_basic_auth_ok(
    rule: &config::ListenRule,
    route: Option<&config::Route>,
    headers: &HeaderMap,
) -> bool {
    if let Some(r) = route {
        if r.exclude_basic_auth.unwrap_or(false) {
            return true;
        }
    }

    if !rule.basic_auth_enable {
        return true;
    }

    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(auth) = auth.to_str() else {
        return false;
    };
    let Some(b64) = auth.strip_prefix("Basic ") else {
        return false;
    };

    let Ok(decoded) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
    else {
        return false;
    };
    let Ok(s) = String::from_utf8(decoded) else {
        return false;
    };

    let expected = format!("{}:{}", rule.basic_auth_username, rule.basic_auth_password);
    s == expected
}

fn header_str(headers: &HeaderMap, key: &str) -> String {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string()
}

fn client_ip(remote: &SocketAddr, headers: &HeaderMap) -> String {
    access_control::client_ip_from_headers(remote, headers)
}

fn is_access_allowed(remote: &SocketAddr, headers: &HeaderMap, cfg: &config::Config) -> bool {
    access_control::is_allowed(remote, headers, cfg)
}

fn time_local_string() -> String {
    // 形如 13/Jan/2026:23:07:40 +0800
    let now = chrono::Local::now();
    now.format("%y.%m.%d %H:%M:%S").to_string()
}

fn request_line(method: &Method, uri: &Uri) -> String {
    // HTTP 版本这里统一写 1.1（大多数客户端习惯）
    format!("{} {} HTTP/1.1", method.as_str(), uri)
}

fn format_access_log(
    node: &str,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    status: StatusCode,
    elapsed: f64,
) -> String {
    // access log 仍然优先记录转发头里的 IP；若无则显示 '-'（不影响数据库中的 client_ip）
    let ip = header_str(headers, "x-forwarded-for");
    let ip = if ip != "-" {
        ip.split(',').next().unwrap_or("-").trim().to_string()
    } else {
        let real = header_str(headers, "x-real-ip");
        if real != "-" {
            real
        } else {
            "-".to_string()
        }
    };
    let time_local = time_local_string();
    let req_line = request_line(method, uri);
    let referer = header_str(headers, "referer");
    let ua = header_str(headers, "user-agent");

    // 1b: 不显示 UPSTREAM 段，这里用 [-] 占位
    // 2c: bytes 拿不到用 -
    format!(
        "[NODE {}] [-] {} - - [{}] \"{}\" {} - \"{}\" \"{}\" {:.3}s",
        node,
        ip,
        time_local,
        req_line,
        status.as_u16(),
        referer,
        ua,
        elapsed
    )
}

fn push_log(_app: &tauri::AppHandle, line: String) {
    // 异步写入：避免阻塞请求热路径
    if let Some(tx) = LOG_TX.read().as_ref() {
        if tx.try_send(line).is_err() {
            // 队列满：丢弃新日志，避免内存无限增长
            LOG_DROPPED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    } else {
        // 极少数情况下（例如尚未 init），回退为同步写入内存
        let mut logs = LOGS.write();
        logs.push(line);
        if logs.len() > 3000 {
            let over = logs.len() - 3000;
            logs.drain(0..over);
        }
    }
}

fn init_log_task(app: tauri::AppHandle) {
    // 已初始化则跳过
    if LOG_TX.read().is_some() {
        return;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(LOG_QUEUE_CAPACITY);
    *LOG_TX.write() = Some(tx);

    tauri::async_runtime::spawn(async move {
        while let Some(line) = rx.recv().await {
            {
                let mut logs = LOGS.write();
                logs.push(line.clone());
                if logs.len() > 3000 {
                    let over = logs.len() - 3000;
                    logs.drain(0..over);
                }
            }

            let _ = app.emit("log-line", line);
        }
    });
}

async fn proxy_handler(
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    State(RuleState(rule)): State<RuleState>,
    State(HttpClientFollow(client_follow)): State<HttpClientFollow>,
    State(HttpClientNoFollow(client_nofollow)): State<HttpClientNoFollow>,
    State(AppHandleState(app)): State<AppHandleState>,
    req: Request<Body>,
) -> Response {
    let started_at = std::time::Instant::now();
    let cfg = config::get_config();

    let node = rule.listen_addr.clone();
    let headers_snapshot = req.headers().clone();
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let route = match_route(&rule.routes, &path);

    // 0. 访问控制（优先级最高）
    // allow_all_lan 取消勾选时：只有白名单 IP 可以访问
    {

        if cfg.http_access_control_enabled {
        // 黑名单优先：命中直接拒绝
        let client_ip_str = access_control::client_ip_from_headers(&remote, req.headers());

        if metrics::is_ip_blacklisted(&client_ip_str) {
            let mut resp = Response::new(Body::from("IP Forbidden"));
            *resp.status_mut() = StatusCode::FORBIDDEN;

            let elapsed = started_at.elapsed().as_secs_f64();
            let line = format_access_log(
                &node,
                &headers_snapshot,
                &method,
                &uri,
                StatusCode::FORBIDDEN,
                elapsed,
            );
            push_log(&app, line);

            metrics::try_enqueue_request_log(crate::metrics::RequestLogInsert {
                timestamp: chrono::Utc::now().timestamp(),
                listen_addr: node.clone(),
                client_ip: client_ip(&remote, &headers_snapshot),
                remote_ip: remote.ip().to_string(),
                method: method.as_str().to_string(),
                request_path: path.clone(),
                request_host: header_str(&headers_snapshot, "host"),
                status_code: StatusCode::FORBIDDEN.as_u16() as i32,
                upstream: "".to_string(),
                latency_ms: elapsed * 1000.0,
                user_agent: header_str(&headers_snapshot, "user-agent"),
                referer: header_str(&headers_snapshot, "referer"),
            });

            return resp;
        }

        if !is_access_allowed(&remote, req.headers(), &cfg) {
            let mut resp = Response::new(Body::from("Forbidden"));
            *resp.status_mut() = StatusCode::FORBIDDEN;

            let elapsed = started_at.elapsed().as_secs_f64();
            let line = format_access_log(
                &node,
                &headers_snapshot,
                &method,
                &uri,
                StatusCode::FORBIDDEN,
                elapsed,
            );
            push_log(&app, line);

            metrics::try_enqueue_request_log(crate::metrics::RequestLogInsert {
                timestamp: chrono::Utc::now().timestamp(),
                listen_addr: node.clone(),
                client_ip: client_ip(&remote, &headers_snapshot),
                remote_ip: remote.ip().to_string(),
                method: method.as_str().to_string(),
                request_path: path.clone(),
                request_host: header_str(&headers_snapshot, "host"),
                status_code: StatusCode::FORBIDDEN.as_u16() as i32,
                upstream: "".to_string(),
                latency_ms: elapsed * 1000.0,
                user_agent: header_str(&headers_snapshot, "user-agent"),
                referer: header_str(&headers_snapshot, "referer"),
            });

            return resp;
            }
        }
    }

    // 1. 检查 Basic Auth（如果启用）
    if !is_basic_auth_ok(&rule, route, req.headers()) {
        let mut resp = Response::new(Body::from("Unauthorized"));
        *resp.status_mut() = StatusCode::UNAUTHORIZED;
        resp.headers_mut().insert(
            axum::http::header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"SSLProxyManager\""),
        );

        let elapsed = started_at.elapsed().as_secs_f64();
        let line = format_access_log(
            &node,
            &headers_snapshot,
            &method,
            &uri,
            StatusCode::UNAUTHORIZED,
            elapsed,
        );
        push_log(&app, line);

        metrics::try_enqueue_request_log(crate::metrics::RequestLogInsert {
            timestamp: chrono::Utc::now().timestamp(),
            listen_addr: node.clone(),
            client_ip: client_ip(&remote, &headers_snapshot),
            remote_ip: remote.ip().to_string(),
            method: method.as_str().to_string(),
            request_path: path.clone(),
            request_host: header_str(&headers_snapshot, "host"),
            status_code: StatusCode::UNAUTHORIZED.as_u16() as i32,
            upstream: "".to_string(),
            latency_ms: elapsed * 1000.0,
            user_agent: header_str(&headers_snapshot, "user-agent"),
            referer: header_str(&headers_snapshot, "referer"),
        });

        return resp;
    }

    let Some(route) = route else {
        let elapsed = started_at.elapsed().as_secs_f64();
        let line = format_access_log(
            &node,
            &headers_snapshot,
            &method,
            &uri,
            StatusCode::NOT_FOUND,
            elapsed,
        );
        push_log(&app, line);

        metrics::try_enqueue_request_log(crate::metrics::RequestLogInsert {
            timestamp: chrono::Utc::now().timestamp(),
            listen_addr: node.clone(),
            client_ip: client_ip(&remote, &headers_snapshot),
            remote_ip: remote.ip().to_string(),
            method: method.as_str().to_string(),
            request_path: path.clone(),
            request_host: header_str(&headers_snapshot, "host"),
            status_code: StatusCode::NOT_FOUND.as_u16() as i32,
            upstream: "".to_string(),
            latency_ms: elapsed * 1000.0,
            user_agent: header_str(&headers_snapshot, "user-agent"),
            referer: header_str(&headers_snapshot, "referer"),
        });

        return (StatusCode::NOT_FOUND, "No route").into_response();
    };

    // 2. 优先处理静态资源（如果配置了 static_dir）
    if let Some(dir) = route.static_dir.as_ref() {
        // 构建 ServeDir 实例
        let serve_dir = ServeDir::new(dir);

        // 处理请求并获取响应
        match serve_dir.oneshot(req).await {
            Ok(response) => {
                let status = response.status();
                // ServeDir 的响应体类型与 axum::body::Body 不同，用 Body::new 包一层即可
                let response = response.map(Body::new);

                // 如果静态文件存在（200-299）或重定向，记录日志并返回
                if status.is_success() || status.is_redirection() {
                    let elapsed = started_at.elapsed().as_secs_f64();
                    let line =
                        format_access_log(&node, &headers_snapshot, &method, &uri, status, elapsed);
                    push_log(&app, line);

                    metrics::try_enqueue_request_log(crate::metrics::RequestLogInsert {
                        timestamp: chrono::Utc::now().timestamp(),
                        listen_addr: node.clone(),
                        client_ip: client_ip(&remote, &headers_snapshot),
                        remote_ip: remote.ip().to_string(),
                        method: method.as_str().to_string(),
                        request_path: path.clone(),
                        request_host: header_str(&headers_snapshot, "host"),
                        status_code: status.as_u16() as i32,
                        upstream: "".to_string(),
                        latency_ms: elapsed * 1000.0,
                        user_agent: header_str(&headers_snapshot, "user-agent"),
                        referer: header_str(&headers_snapshot, "referer"),
                    });

                    return response;
                }

                // 如果是 404 且是 GET/HEAD 请求，检查是否是 SPA 回退场景
                if status == StatusCode::NOT_FOUND
                    && (method == Method::GET || method == Method::HEAD)
                    && !is_asset_path(&path)
                {
                    if let Ok(bytes) =
                        tokio::fs::read(std::path::Path::new(dir).join("index.html")).await
                    {
                        let mut resp = Response::new(Body::from(bytes));
                        resp.headers_mut().insert(
                            axum::http::header::CONTENT_TYPE,
                            HeaderValue::from_static("text/html; charset=utf-8"),
                        );

                        // SPA 回退：按 200 记录请求日志
                        let elapsed = started_at.elapsed().as_secs_f64();
                        let line = format_access_log(
                            &node,
                            &headers_snapshot,
                            &method,
                            &uri,
                            StatusCode::OK,
                            elapsed,
                        );
                        push_log(&app, line);

                        metrics::try_enqueue_request_log(crate::metrics::RequestLogInsert {
                            timestamp: chrono::Utc::now().timestamp(),
                            listen_addr: node.clone(),
                            client_ip: client_ip(&remote, &headers_snapshot),
                            remote_ip: remote.ip().to_string(),
                            method: method.as_str().to_string(),
                            request_path: path.clone(),
                            request_host: header_str(&headers_snapshot, "host"),
                            status_code: StatusCode::OK.as_u16() as i32,
                            upstream: "".to_string(),
                            latency_ms: elapsed * 1000.0,
                            user_agent: header_str(&headers_snapshot, "user-agent"),
                            referer: header_str(&headers_snapshot, "referer"),
                        });

                        return resp;
                    }
                }

                // 其他情况返回 ServeDir 的原始结果（例如 404/403）
                return response;
            }
            Err(_) => {
                // ServeDir 内部错误，继续处理
            }
        }

        // 静态资源处理失败，且不是 SPA 回退场景，返回 404
        let elapsed = started_at.elapsed().as_secs_f64();
        let line = format_access_log(
            &node,
            &headers_snapshot,
            &method,
            &uri,
            StatusCode::NOT_FOUND,
            elapsed,
        );
        push_log(&app, line);

        metrics::try_enqueue_request_log(crate::metrics::RequestLogInsert {
            timestamp: chrono::Utc::now().timestamp(),
            listen_addr: node.clone(),
            client_ip: client_ip(&remote, &headers_snapshot),
            remote_ip: remote.ip().to_string(),
            method: method.as_str().to_string(),
            request_path: path.clone(),
            request_host: header_str(&headers_snapshot, "host"),
            status_code: StatusCode::NOT_FOUND.as_u16() as i32,
            upstream: "".to_string(),
            latency_ms: elapsed * 1000.0,
            user_agent: header_str(&headers_snapshot, "user-agent"),
            referer: header_str(&headers_snapshot, "referer"),
        });

        return (StatusCode::NOT_FOUND, "Static file not found").into_response();
    }

    // 3. 处理反代逻辑（如果有 upstream）
    if let Some(upstream_url) = pick_upstream_smooth(route) {
        let target = match build_upstream_url(
            &upstream_url,
            route.path.as_deref(),
            route.proxy_pass_path.as_deref(),
            &uri,
        ) {
            Ok(u) => u,
            Err(e) => {
                let elapsed = started_at.elapsed().as_secs_f64();
                let line = format_access_log(
                    &node,
                    &headers_snapshot,
                    &method,
                    &uri,
                    StatusCode::BAD_GATEWAY,
                    elapsed,
                );
                push_log(&app, line);

                metrics::try_enqueue_request_log(crate::metrics::RequestLogInsert {
                    timestamp: chrono::Utc::now().timestamp(),
                    listen_addr: node.clone(),
                    client_ip: client_ip(&remote, &headers_snapshot),
                    remote_ip: remote.ip().to_string(),
                    method: method.as_str().to_string(),
                    request_path: path.clone(),
                    request_host: header_str(&headers_snapshot, "host"),
                    status_code: StatusCode::BAD_GATEWAY.as_u16() as i32,
                    upstream: upstream_url.clone(),
                    latency_ms: elapsed * 1000.0,
                    user_agent: header_str(&headers_snapshot, "user-agent"),
                    referer: header_str(&headers_snapshot, "referer"),
                });

                return (StatusCode::BAD_GATEWAY, format!("bad upstream url: {e}")).into_response();
            }
        };

        let client = if route.follow_redirects {
            client_follow.clone()
        } else {
            client_nofollow.clone()
        };

        // 0) 拆分 request，避免 into_body 后再使用 req
        let (req_parts, req_body_axum) = req.into_parts();
        let inbound_headers = req_parts.headers.clone();
        let method_up = req_parts.method.clone();

        // 1) 读取请求体（用于 req_body_size & 避免 req move 问题）
        // 热路径只读一次配置（避免频繁 clone 整个 Config）
        let stream_proxy = cfg.stream_proxy;
        let max_body_size = cfg.max_body_size;
        let max_response_body_size = cfg.max_response_body_size;

        let (reqwest_body, req_body_size) = if stream_proxy {
            let body_stream = req_body_axum.into_data_stream();
            (reqwest::Body::wrap_stream(body_stream), None)
        } else {
            let bytes = match axum::body::to_bytes(req_body_axum, max_body_size).await {
                Ok(b) => b,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        format!("read request body failed: {e}"),
                    )
                        .into_response();
                }
            };
            let len = bytes.len();
            (reqwest::Body::from(bytes), Some(len))
        };

        // 2) 构造最终 headers（覆盖语义，避免重复 header 导致上游 400）
        let mut final_headers = HeaderMap::new();

        // 2.1) 先复制入站 headers（跳过 hop-by-hop；跳过将由代理统一控制/覆盖的头）
        for (k, v) in inbound_headers.iter() {
            // 这里不要做 to_ascii_lowercase 分配；用 HeaderName 常量比较即可
            if *k == axum::http::header::HOST
                || *k == axum::http::header::CONNECTION
                || *k == axum::http::header::ACCEPT_ENCODING
                || *k == HeaderName::from_static("x-real-ip")
                || *k == HeaderName::from_static("x-forwarded-for")
                || *k == HeaderName::from_static("x-forwarded-proto")
            {
                continue;
            }

            if is_hop_header(k.as_str()) {
                continue;
            }

            final_headers.append(k.clone(), v.clone());
        }

        // 2.2) Host：默认沿用入站 Host（如需改 Host 请在 set_headers 里显式设置 Host）
        if let Some(h) = inbound_headers.get(axum::http::header::HOST) {
            final_headers.insert(axum::http::header::HOST, h.clone());
        }

        // 2.3) 常用转发头
        {
            let remote_ip = remote.ip().to_string();
            if let Ok(v) = HeaderValue::from_str(&remote_ip) {
                final_headers.insert(HeaderName::from_static("x-real-ip"), v);
            }

            let prior = inbound_headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());

            let combined = match prior {
                Some(p) => format!("{}, {}", p, remote_ip),
                None => remote_ip,
            };

            if let Ok(v) = HeaderValue::from_str(&combined) {
                final_headers.insert(HeaderName::from_static("x-forwarded-for"), v);
            }
        }

        final_headers.insert(
            HeaderName::from_static("x-forwarded-proto"),
            HeaderValue::from_static(if rule.ssl_enable { "https" } else { "http" }),
        );

        // 2.4) 对齐 nginx 常见实践：禁用压缩
        final_headers.insert(axum::http::header::ACCEPT_ENCODING, HeaderValue::from_static(""));

        // 2.5) Content-Type：若入站有且 set_headers 未覆盖，则保留
        if !final_headers.contains_key(axum::http::header::CONTENT_TYPE) {
            if let Some(ct) = inbound_headers.get(axum::http::header::CONTENT_TYPE) {
                final_headers.insert(axum::http::header::CONTENT_TYPE, ct.clone());
            }
        }

        // 2.6) 应用 set_headers（覆盖语义：insert）
        if let Some(map) = route.set_headers.as_ref() {
            for (k, v) in map {
                let key = k.trim();
                if key.is_empty() {
                    continue;
                }
                if is_hop_header(key) {
                    continue;
                }

                let expanded = expand_proxy_header_value(v, &remote, &inbound_headers, rule.ssl_enable);

                let name = match HeaderName::from_bytes(key.as_bytes()) {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                if expanded.is_empty() {
                    final_headers.insert(name, HeaderValue::from_static(""));
                    continue;
                }

                let value = match HeaderValue::from_str(&expanded) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                final_headers.insert(name, value);
            }
        }

        // 2.7) 若启用了 BasicAuth 且不允许向上游转发，则移除 Authorization
        // 注意：未启用 BasicAuth 时，Authorization 往往是业务鉴权头，必须保留。
        let will_remove_auth = rule.basic_auth_enable && !rule.basic_auth_forward_header;

        if will_remove_auth {
            final_headers.remove(axum::http::header::AUTHORIZATION);
        }

        let _after_remove_has_auth = final_headers.contains_key(axum::http::header::AUTHORIZATION);

        // 3) 构造并发送上游请求（build 后清空并写入 final_headers，彻底去重）
        let mut builder = client.request(method_up, target.clone());
        builder = builder.body(reqwest_body);

        let mut upstream_req = match builder.build() {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!("build upstream request failed: {e}"),
                )
                    .into_response();
            }
        };

        upstream_req.headers_mut().clear();
        upstream_req.headers_mut().extend(final_headers.clone());

        let outbound_headers_snapshot = upstream_req.headers().clone();

        let inbound_headers_line = inbound_headers
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("[invalid utf8]")))
            .collect::<Vec<_>>()
            .join(" ## ");

        let outbound_headers_line = outbound_headers_snapshot
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("[invalid utf8]")))
            .collect::<Vec<_>>()
            .join(" ## ");

        let resp = match client.execute(upstream_req).await {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!("upstream request failed: {e}"),
                )
                    .into_response();
            }
        };

        let status = resp.status();
        let elapsed = started_at.elapsed().as_secs_f64();
        let line = format_access_log(
            &node,
            &headers_snapshot,
            &method,
            &uri,
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            elapsed,
        );
        push_log(&app, line);

        metrics::try_enqueue_request_log(crate::metrics::RequestLogInsert {
            timestamp: chrono::Utc::now().timestamp(),
            listen_addr: node.clone(),
            client_ip: client_ip(&remote, &headers_snapshot),
            remote_ip: remote.ip().to_string(),
            method: method.as_str().to_string(),
            request_path: path.clone(),
            request_host: header_str(&headers_snapshot, "host"),
            status_code: status.as_u16() as i32,
            upstream: target.clone(),
            latency_ms: elapsed * 1000.0,
            user_agent: header_str(&headers_snapshot, "user-agent"),
            referer: header_str(&headers_snapshot, "referer"),
        });

        let mut out = Response::new(Body::empty());
        *out.status_mut() =
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

        for (k, v) in resp.headers().iter() {
            if is_hop_header(k.as_str()) {
                continue;
            }
            out.headers_mut().insert(k.clone(), v.clone());
        }

        // 根据配置决定是否流式转发响应体
        if stream_proxy {
            let stream = resp.bytes_stream();
            *out.body_mut() = Body::from_stream(stream);
        } else {
            // 非流式：限制最大响应体大小，避免一次性读入超大响应导致内存爆
            let bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        format!("read upstream body failed: {e}"),
                    )
                        .into_response();
                }
            };

            if max_response_body_size > 0 && bytes.len() > max_response_body_size {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!(
                        "upstream body too large (limit={} bytes)",
                        max_response_body_size
                    ),
                )
                    .into_response();
            }

            *out.body_mut() = Body::from(bytes);
        }

        // 仅在错误时记录反代细节，降低高 QPS 下日志/锁开销
        if !status.is_success() {
            send_log(format!(
                "反代错误(IN): {} {} -> {} status={} | inbound_headers=[{}]",
                method.as_str(),
                uri,
                target,
                status.as_u16(),
                inbound_headers_line
            ));

            send_log(format!(
                "反代错误(OUT): {} {} -> {} status={} | outbound_headers=[{}] | req_body_size={}",
                method.as_str(),
                uri,
                target,
                status.as_u16(),
                outbound_headers_line,
                req_body_size.map(|n| n.to_string()).unwrap_or_else(|| "stream".to_string())
            ));
        }

        return out;
    }

    // 既没有静态目录也没有上游，返回 404
    (
        StatusCode::NOT_FOUND,
        "No static directory or upstream configured",
    )
        .into_response()
}

fn build_upstream_url(
    upstream_base: &str,
    route_path: Option<&str>,
    proxy_pass_path: Option<&str>,
    uri: &Uri,
) -> Result<String> {
    let mut base = upstream_base.trim_end_matches('/').to_string();

    let orig_path = uri.path();
    let route_path = route_path.unwrap_or("/");

    let mut new_path = orig_path.to_string();
    if let Some(pp) = proxy_pass_path {
        let from = if route_path.is_empty() { "/" } else { route_path };
        let to = if pp.trim().is_empty() { "/" } else { pp };

        if new_path.starts_with(from) {
            let suffix = &new_path[from.len()..];

            let mut out_path = to.to_string();
            if out_path.is_empty() {
                out_path = "/".to_string();
            }

            // 拼接时处理斜杠，避免出现 // 或缺少 /
            let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
            if out_path.ends_with('/') {
                new_path = if suffix.is_empty() {
                    out_path
                } else {
                    format!("{}{}", out_path, suffix)
                };
            } else {
                new_path = if suffix.is_empty() {
                    out_path
                } else {
                    format!("{}/{}", out_path, suffix)
                };
            }
        }

        if !new_path.starts_with('/') {
            new_path = format!("/{}", new_path);
        }
    }

    base.push_str(&new_path);
    if let Some(q) = uri.query() {
        base.push('?');
        base.push_str(q);
    }
    Ok(base)
}

fn is_asset_path(path: &str) -> bool {
    path.contains('.') || path.starts_with("/assets/") || path.starts_with("/static/")
}

fn is_hop_header(name: &str) -> bool {
    // 这里是热路径：避免 to_ascii_lowercase 分配
    name.eq_ignore_ascii_case("connection")
        || name.eq_ignore_ascii_case("keep-alive")
        || name.eq_ignore_ascii_case("proxy-authenticate")
        || name.eq_ignore_ascii_case("proxy-authorization")
        || name.eq_ignore_ascii_case("te")
        || name.eq_ignore_ascii_case("trailer")
        || name.eq_ignore_ascii_case("transfer-encoding")
        || name.eq_ignore_ascii_case("upgrade")
}

fn expand_proxy_header_value(
    raw: &str,
    remote: &SocketAddr,
    inbound_headers: &HeaderMap,
    is_tls: bool,
) -> String {
    let mut out = raw.to_string();

    // $remote_addr: 与 nginx 语义接近，取客户端 IP
    out = out.replace("$remote_addr", &remote.ip().to_string());

    // $scheme: 由监听规则推断
    out = out.replace("$scheme", if is_tls { "https" } else { "http" });

    // $proxy_add_x_forwarded_for: 在已有 X-Forwarded-For 基础上追加 remote.ip
    if out.contains("$proxy_add_x_forwarded_for") {
        let remote_ip = remote.ip().to_string();
        let prior = inbound_headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let combined = match prior {
            Some(p) => format!("{}, {}", p, remote_ip),
            None => remote_ip,
        };

        out = out.replace("$proxy_add_x_forwarded_for", &combined);
    }

    out
}

