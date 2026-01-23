use crate::{access_control, config, metrics, ws_proxy, stream_proxy};
use anyhow::{anyhow, Context, Result};
use axum::{
    body::Body,
    extract::{connect_info::ConnectInfo, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use parking_lot::RwLock;
use reqwest::redirect::Policy;
use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use dashmap::DashMap;

const LOG_QUEUE_CAPACITY: usize = 10_000;

static LOG_TX: once_cell::sync::Lazy<RwLock<Option<tokio::sync::mpsc::Sender<String>>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(None));

static LOG_DROPPED: once_cell::sync::Lazy<std::sync::atomic::AtomicU64> =
    once_cell::sync::Lazy::new(|| std::sync::atomic::AtomicU64::new(0));

// 预计算需要跳过的 header
static SKIP_HEADERS: once_cell::sync::Lazy<HashSet<HeaderName>> = once_cell::sync::Lazy::new(|| {
    let mut set = HashSet::new();
    set.insert(axum::http::header::HOST);
    set.insert(axum::http::header::CONNECTION);
    set.insert(axum::http::header::ACCEPT_ENCODING);
    set.insert(HeaderName::from_static("x-real-ip"));
    set.insert(HeaderName::from_static("x-forwarded-for"));
    set.insert(HeaderName::from_static("x-forwarded-proto"));
    set
});

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

// 使用 DashMap 替代 RwLock<HashMap>，减少锁竞争
static UPSTREAM_LB: once_cell::sync::Lazy<DashMap<String, Arc<RwLock<SmoothLbState>>>> =
    once_cell::sync::Lazy::new(|| DashMap::new());

// 优化后的 AppState：缓存常用配置，减少热路径上的配置克隆
#[derive(Clone)]
struct AppState {
    rule: config::ListenRule,
    client_follow: reqwest::Client,
    client_nofollow: reqwest::Client,
    app: tauri::AppHandle,
    // 缓存配置字段，避免每次请求都克隆整个 Config
    listen_addr: Arc<str>,
    stream_proxy: bool,
    max_body_size: usize,
    max_response_body_size: usize,
    http_access_control_enabled: bool,
    // 访问控制所需配置快照：避免每请求 get_config() clone 整个 Config
    allow_all_lan: bool,
    whitelist: Arc<[config::WhitelistEntry]>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleStartErrorPayload {
    pub listen_addr: String,
    pub error: String,
}

// 请求上下文：统一管理请求相关数据，减少参数传递
struct RequestContext {
    client_ip: String,
    started_at: std::time::Instant,
    client_ip_header: String,
    real_ip_header: String,
    host_header: String,
    referer_header: String,
    user_agent_header: String,
    method: Method,
    uri: Uri,
    path: String,
}

impl RequestContext {
    fn new(remote: SocketAddr, headers: &HeaderMap, method: Method, uri: Uri) -> Self {
        let path = uri.path().to_string();

        // 只提取日志/指标需要的少数字段，避免 HeaderMap 全量 clone
        // 小优化：单独封装一次 get + to_str + to_string，减少重复闭包/链式调用开销
        #[inline]
        fn header_to_string(headers: &HeaderMap, key: &'static str) -> String {
            headers
                .get(key)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("-")
                .to_string()
        }

        let xff = header_to_string(headers, "x-forwarded-for");
        let xri = header_to_string(headers, "x-real-ip");
        let host = header_to_string(headers, "host");
        let referer = header_to_string(headers, "referer");
        let ua = header_to_string(headers, "user-agent");

        Self {
            client_ip: access_control::client_ip_from_headers(&remote, headers),
            started_at: std::time::Instant::now(),
            client_ip_header: xff,
            real_ip_header: xri,
            host_header: host,
            referer_header: referer,
            user_agent_header: ua,
            method,
            uri,
            path,
        }
    }

    #[inline]
    fn elapsed_ms(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64() * 1000.0
    }

    #[inline]
    fn elapsed_s(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64()
    }
}

pub fn start_server(app: tauri::AppHandle) -> Result<()> {
    init_log_task(app.clone());

    if let Err(e) = ws_proxy::start_ws_servers(app.clone()) {
        send_log(format!("启动 WS 监听器失败: {e}"));
    }

    {
        let stream_cfg = config::get_config().stream.clone();
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = stream_proxy::start_stream_servers(&stream_cfg).await {
                send_log_with_app(&app2, format!("启动 Stream 监听器失败: {e}"));
            }
        });
    }

    {
        let starting = STARTING.read();
        if *starting {
            return Ok(());
        }
    }

    *STARTING.write() = true;
    *START_FAILED.write() = false;

    let cfg = config::get_config();
    let rules: Vec<_> = cfg.rules.into_iter().filter(|r| r.enabled).collect();
    let expected = rules.len();
    *START_EXPECTED.write() = expected;
    *START_STARTED_COUNT.write() = 0;

    if expected == 0 {
        *IS_RUNNING.write() = false;
        *STARTING.write() = false;
        let _ = app.emit("status", "stopped");
        send_log("未配置监听规则，服务保持停止状态".to_string());
        return Ok(());
    }

    let _ = app.emit("status", "stopped");

    let mut handles = Vec::new();

    for rule in rules {
        let app_handle = app.clone();
        let listen_addr = rule.listen_addr.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let handle = tauri::async_runtime::spawn(async move {
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

            {
                let mut started = START_STARTED_COUNT.write();
                *started += 1;
                let expected = *START_EXPECTED.read();
                let failed = *START_FAILED.read();
                if !failed && *started == expected {
                    *IS_RUNNING.write() = true;
                    *STARTING.write() = false;
                    let _ = app_handle.emit("status", "running");

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

            match start_rule_server(app_handle.clone(), rule, shutdown_rx).await {
                Ok(()) => {}
                Err(e) => {
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
                }
            }
        });
        handles.push(ServerHandle { handle, shutdown_tx });
    }

    *SERVERS.write() = handles;
    send_log("代理服务器启动中...".to_string());
    Ok(())
}

pub fn stop_server(app: tauri::AppHandle) -> Result<()> {
    ws_proxy::stop_ws_servers();
    *LOG_TX.write() = None;

    tauri::async_runtime::spawn(async {
        stream_proxy::stop_stream_servers().await;
    });

    *STARTING.write() = false;
    *START_FAILED.write() = false;
    *START_EXPECTED.write() = 0;
    *START_STARTED_COUNT.write() = 0;
    *IS_RUNNING.write() = false;

    let handles = std::mem::take(&mut *SERVERS.write());
    for handle in handles {
        handle.abort();
    }

    let _ = app.emit("status", "stopped");

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
    {
        let mut logs = LOGS.write();
        logs.push(message.clone());
        if logs.len() > 3000 {
            let over = logs.len() - 3000;
            logs.drain(0..over);
        }
    }

    let cfg = config::get_config();
    if !cfg.show_realtime_logs {
        return;
    }

    if cfg.realtime_logs_only_errors {
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
        let _ = axum_server::tls_rustls::RustlsConfig::from_pem_file(
            rule.cert_file.clone(),
            rule.key_file.clone(),
        )
        .await
        .with_context(|| "加载 TLS 证书/私钥失败")?;

        let listener = tokio::net::TcpListener::bind(addr).await?;
        drop(listener);
    } else {
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
            .tcp_nodelay(true)
            .connect_timeout(Duration::from_millis(cfg.upstream_connect_timeout_ms))
            .timeout(Duration::from_millis(cfg.upstream_read_timeout_ms));

        if !cfg.enable_http2 {
            builder = builder.http1_only();
        }

        builder
    };

    let follow_builder = client_builder();
    let nofollow_builder = client_builder().redirect(Policy::none());

    let client_follow = follow_builder.build().context("创建上游 HTTP client 失败")?;

    let client_nofollow = nofollow_builder.build().context("创建上游 HTTP client 失败")?;

    // 缓存常用配置到 AppState
    let state = AppState {
        rule: rule.clone(),
        client_follow,
        client_nofollow,
        app: app.clone(),
        listen_addr: Arc::from(rule.listen_addr.clone()),
        stream_proxy: cfg.stream_proxy,
        max_body_size: cfg.max_body_size,
        max_response_body_size: cfg.max_response_body_size,
        http_access_control_enabled: cfg.http_access_control_enabled,
        allow_all_lan: cfg.allow_all_lan,
        whitelist: Arc::from(cfg.whitelist),
    };

    let router = Router::new().route("/healthz", any(healthz));
    let app = router.fallback(any(proxy_handler)).with_state(state);
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

#[inline]
fn normalize_host(host: &str) -> &str {
    host.split(':').next().unwrap_or(host).trim()
}

#[inline]
fn match_route<'a>(
    routes: &'a [config::Route],
    request_host: &str,
    path: &str,
) -> (Option<&'a config::Route>, String) {
    let host = normalize_host(request_host);

    let mut best: Option<(&config::Route, bool, usize)> = None; // (route, has_host_constraint, path_len)

    for r in routes {
        if !r.enabled {
            continue;
        }

        let p = match r.path.as_deref() {
            Some(v) => v,
            None => continue,
        };
        if !path.starts_with(p) {
            continue;
        }

        let host_ok = match r.host.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            None => true,
            Some(h) => normalize_host(h).eq_ignore_ascii_case(host),
        };
        if !host_ok {
            continue;
        }

        let cand = (r, r.host.as_ref().is_some(), p.len());
        best = match best {
            None => Some(cand),
            Some((best_r, best_has_host, best_plen)) => {
                if cand.1 != best_has_host {
                    if cand.1 { Some(cand) } else { Some((best_r, best_has_host, best_plen)) }
                } else if cand.2 > best_plen {
                    Some(cand)
                } else {
                    Some((best_r, best_has_host, best_plen))
                }
            }
        };
    }

    if let Some((r, _, _)) = best {
        (Some(r), r.id.as_deref().unwrap_or("").to_string())
    } else {
        (None, String::new())
    }
}

fn upstream_signature(route: &config::Route) -> String {
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
        return Some(route.upstreams[0].url.clone());
    }

    let sig = upstream_signature(route);

    // 使用 DashMap：无需全局读锁，性能更好
    let state_lock = UPSTREAM_LB
        .entry(route_id.to_string())
        .or_insert_with(|| {
            Arc::new(RwLock::new(SmoothLbState {
                signature: String::new(),
                total_weight: 0,
                upstreams: Vec::new(),
            }))
        })
        .clone();

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

#[inline]
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

    let Ok(decoded) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) else {
        return false;
    };
    let Ok(s) = String::from_utf8(decoded) else {
        return false;
    };

    let expected = format!("{}:{}", rule.basic_auth_username, rule.basic_auth_password);
    s == expected
}


#[inline]
fn time_local_string() -> String {
    let now = chrono::Local::now();
    now.format("%y.%m.%d %H:%M:%S").to_string()
}

#[inline]
fn request_line(method: &Method, uri: &Uri) -> String {
    format!("{} {} HTTP/1.1", method.as_str(), uri)
}

fn format_access_log(node: &str, ctx: &RequestContext, status: StatusCode) -> String {
    // 优先使用解析后的 client_ip（会从 XFF/X-Real-IP/remote 推导）
    // 兜底再回退到原始 header 字段，避免日志里出现空/"-"。
    let ip = if !ctx.client_ip.is_empty() {
        ctx.client_ip.clone()
    } else if ctx.client_ip_header != "-" {
        ctx.client_ip_header
            .split(',')
            .next()
            .unwrap_or("-")
            .trim()
            .to_string()
    } else if ctx.real_ip_header != "-" {
        ctx.real_ip_header.clone()
    } else {
        "-".to_string()
    };
    let time_local = time_local_string();
    let req_line = request_line(&ctx.method, &ctx.uri);
    let referer = ctx.referer_header.clone();
    let ua = ctx.user_agent_header.clone();

    format!(
        "[NODE {}] [-] {} - - [{}] \"{}\" {} - \"{}\" \"{}\" {:.3}s",
        node,
        ip,
        time_local,
        req_line,
        status.as_u16(),
        referer,
        ua,
        ctx.elapsed_s()
    )
}

// 延迟日志格式化：只在需要时才格式化
fn push_log_lazy<F>(_app: &tauri::AppHandle, f: F)
where
    F: FnOnce() -> String,
{
    if let Some(tx) = LOG_TX.read().as_ref() {
        let line = f();
        if tx.try_send(line).is_err() {
            LOG_DROPPED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    } else {
        let line = f();
        let mut logs = LOGS.write();
        logs.push(line);
        if logs.len() > 3000 {
            let over = logs.len() - 3000;
            logs.drain(0..over);
        }
    }
}

fn init_log_task(app: tauri::AppHandle) {
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
    State(state): State<AppState>,
    req: Request<Body>,
) -> Response {
    let ctx = RequestContext::new(remote, req.headers(), req.method().clone(), req.uri().clone());

    let node = &*state.listen_addr;
    let (route, matched_route_id) = match_route(&state.rule.routes, &ctx.host_header, &ctx.path);

    // 0. 访问控制
    if state.http_access_control_enabled {
        if metrics::is_ip_blacklisted(&ctx.client_ip) {
            let status = StatusCode::FORBIDDEN;
            push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

            metrics::try_enqueue_request_log(metrics::RequestLogInsert {
                timestamp: chrono::Utc::now().timestamp(),
                listen_addr: node.to_string(),
                client_ip: ctx.client_ip.clone(),
                remote_ip: remote.ip().to_string(),
                method: ctx.method.as_str().to_string(),
                request_path: ctx.path.clone(),
                request_host: ctx.host_header.clone(),
                status_code: status.as_u16() as i32,
                upstream: "".to_string(),
                latency_ms: ctx.elapsed_ms(),
                user_agent: ctx.user_agent_header.clone(),
                referer: ctx.referer_header.clone(),
                matched_route_id: matched_route_id.clone(),
            });

            return (status, "IP Forbidden").into_response();
        }

        if !access_control::is_allowed_fast(
            &remote,
            req.headers(),
            state.allow_all_lan,
            &state.whitelist,
        ) {
            let status = StatusCode::FORBIDDEN;
            push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

            metrics::try_enqueue_request_log(metrics::RequestLogInsert {
                timestamp: chrono::Utc::now().timestamp(),
                listen_addr: node.to_string(),
                client_ip: ctx.client_ip.clone(),
                remote_ip: remote.ip().to_string(),
                method: ctx.method.as_str().to_string(),
                request_path: ctx.path.clone(),
                request_host: ctx.host_header.clone(),
                status_code: status.as_u16() as i32,
                upstream: "".to_string(),
                latency_ms: ctx.elapsed_ms(),
                user_agent: ctx.user_agent_header.clone(),
                referer: ctx.referer_header.clone(),
                matched_route_id: matched_route_id.clone(),
            });

            return (status, "Forbidden").into_response();
        }
    }

    // 1. 检查 Basic Auth
    if !is_basic_auth_ok(&state.rule, route, req.headers()) {
        let status = StatusCode::UNAUTHORIZED;
        push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

        metrics::try_enqueue_request_log(metrics::RequestLogInsert {
            timestamp: chrono::Utc::now().timestamp(),
            listen_addr: node.to_string(),
            client_ip: ctx.client_ip.clone(),
            remote_ip: remote.ip().to_string(),
            method: ctx.method.as_str().to_string(),
            request_path: ctx.path.clone(),
            request_host: ctx.host_header.clone(),
            status_code: status.as_u16() as i32,
            upstream: "".to_string(),
            latency_ms: ctx.elapsed_ms(),
            user_agent: ctx.user_agent_header.clone(),
            referer: ctx.referer_header.clone(),
            matched_route_id: matched_route_id.clone(),
        });

        let mut resp = Response::new(Body::from("Unauthorized"));
        *resp.status_mut() = status;
        resp.headers_mut().insert(
            axum::http::header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"SSLProxyManager\""),
        );
        return resp;
    }

    let Some(route) = route else {
        let status = StatusCode::NOT_FOUND;
        push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

        metrics::try_enqueue_request_log(metrics::RequestLogInsert {
            timestamp: chrono::Utc::now().timestamp(),
            listen_addr: node.to_string(),
            client_ip: ctx.client_ip.clone(),
            remote_ip: remote.ip().to_string(),
            method: ctx.method.as_str().to_string(),
            request_path: ctx.path.clone(),
            request_host: ctx.host_header.clone(),
            status_code: status.as_u16() as i32,
            upstream: "".to_string(),
            latency_ms: ctx.elapsed_ms(),
            user_agent: ctx.user_agent_header.clone(),
            referer: ctx.referer_header.clone(),
            matched_route_id: matched_route_id.clone(),
        });

        return (status, "No route").into_response();
    };

    // 2. 优先处理静态资源
    if let Some(dir) = route.static_dir.as_ref() {
        let serve_dir = ServeDir::new(dir);

        match serve_dir.oneshot(req).await {
            Ok(response) => {
                let status = response.status();
                let response = response.map(Body::new);

                if status.is_success() || status.is_redirection() {
                    push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

                    metrics::try_enqueue_request_log(metrics::RequestLogInsert {
                        timestamp: chrono::Utc::now().timestamp(),
                        listen_addr: node.to_string(),
                        client_ip: ctx.client_ip.clone(),
                        remote_ip: remote.ip().to_string(),
                        method: ctx.method.as_str().to_string(),
                        request_path: ctx.path.clone(),
                        request_host: ctx.host_header.clone(),
                        status_code: status.as_u16() as i32,
                        upstream: "".to_string(),
                        latency_ms: ctx.elapsed_ms(),
                        user_agent: ctx.user_agent_header.clone(),
                        referer: ctx.referer_header.clone(),
                        matched_route_id: matched_route_id.clone(),
                    });

                    return response;
                }

                // SPA 回退
                if status == StatusCode::NOT_FOUND
                    && (ctx.method == Method::GET || ctx.method == Method::HEAD)
                    && !is_asset_path(&ctx.path)
                {
                    if let Ok(bytes) =
                        tokio::fs::read(std::path::Path::new(dir).join("index.html")).await
                    {
                        let mut resp = Response::new(Body::from(bytes));
                        resp.headers_mut().insert(
                            axum::http::header::CONTENT_TYPE,
                            HeaderValue::from_static("text/html; charset=utf-8"),
                        );

                        let status = StatusCode::OK;
                        push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

                        metrics::try_enqueue_request_log(metrics::RequestLogInsert {
                            timestamp: chrono::Utc::now().timestamp(),
                            listen_addr: node.to_string(),
                            client_ip: ctx.client_ip.clone(),
                            remote_ip: remote.ip().to_string(),
                            method: ctx.method.as_str().to_string(),
                            request_path: ctx.path.clone(),
                            request_host: ctx.host_header.clone(),
                            status_code: status.as_u16() as i32,
                            upstream: "".to_string(),
                            latency_ms: ctx.elapsed_ms(),
                            user_agent: ctx.user_agent_header.clone(),
                            referer: ctx.referer_header.clone(),
                            matched_route_id: matched_route_id.clone(),
                        });

                        return resp;
                    }
                }

                return response;
            }
            Err(_) => {}
        }

        let status = StatusCode::NOT_FOUND;
        push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

        metrics::try_enqueue_request_log(metrics::RequestLogInsert {
            timestamp: chrono::Utc::now().timestamp(),
            listen_addr: node.to_string(),
            client_ip: ctx.client_ip.clone(),
            remote_ip: remote.ip().to_string(),
            method: ctx.method.as_str().to_string(),
            request_path: ctx.path.clone(),
            request_host: ctx.host_header.clone(),
            status_code: status.as_u16() as i32,
            upstream: "".to_string(),
            latency_ms: ctx.elapsed_ms(),
            user_agent: ctx.user_agent_header.clone(),
            referer: ctx.referer_header.clone(),
            matched_route_id: matched_route_id.clone(),
        });

        return (status, "Static file not found").into_response();
    }

    // 3. 处理反代逻辑
    if let Some(upstream_url) = pick_upstream_smooth(route) {
        let target = match build_upstream_url(
            &upstream_url,
            route.path.as_deref(),
            route.proxy_pass_path.as_deref(),
            &ctx.uri,
        ) {
            Ok(u) => u,
            Err(e) => {
                let status = StatusCode::BAD_GATEWAY;
                push_log_lazy(&state.app, || format_access_log(node, &ctx, status));

                metrics::try_enqueue_request_log(metrics::RequestLogInsert {
                    timestamp: chrono::Utc::now().timestamp(),
                    listen_addr: node.to_string(),
                    client_ip: ctx.client_ip.clone(),
                    remote_ip: remote.ip().to_string(),
                    method: ctx.method.as_str().to_string(),
                    request_path: ctx.path.clone(),
                    request_host: ctx.host_header.clone(),
                    status_code: status.as_u16() as i32,
                    upstream: upstream_url.clone(),
                    latency_ms: ctx.elapsed_ms(),
                    user_agent: ctx.user_agent_header.clone(),
                    referer: ctx.referer_header.clone(),
                    matched_route_id: matched_route_id.clone(),
                });

                return (status, format!("bad upstream url: {e}")).into_response();
            }
        };

        let client = if route.follow_redirects {
            state.client_follow.clone()
        } else {
            state.client_nofollow.clone()
        };

        let (req_parts, req_body_axum) = req.into_parts();
        let inbound_headers = req_parts.headers.clone();
        let method_up = req_parts.method.clone();

        // 读取请求体
        let (reqwest_body, req_body_size) = if state.stream_proxy {
            let body_stream = req_body_axum.into_data_stream();
            (reqwest::Body::wrap_stream(body_stream), None)
        } else {
            let bytes = match axum::body::to_bytes(req_body_axum, state.max_body_size).await {
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

        // 构造最终 headers（使用预计算的 SKIP_HEADERS）
        let mut final_headers = HeaderMap::new();

        for (k, v) in inbound_headers.iter() {
            if SKIP_HEADERS.contains(k) || is_hop_header_fast(k.as_str()) {
                continue;
            }
            final_headers.append(k.clone(), v.clone());
        }

        // Host header
        if let Some(h) = inbound_headers.get(axum::http::header::HOST) {
            final_headers.insert(axum::http::header::HOST, h.clone());
        }

        // 转发头
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
            HeaderValue::from_static(if state.rule.ssl_enable { "https" } else { "http" }),
        );

        final_headers.insert(axum::http::header::ACCEPT_ENCODING, HeaderValue::from_static(""));

        if !final_headers.contains_key(axum::http::header::CONTENT_TYPE) {
            if let Some(ct) = inbound_headers.get(axum::http::header::CONTENT_TYPE) {
                final_headers.insert(axum::http::header::CONTENT_TYPE, ct.clone());
            }
        }

        // set_headers
        if let Some(map) = route.set_headers.as_ref() {
            for (k, v) in map {
                let key = k.trim();
                if key.is_empty() || is_hop_header_fast(key) {
                    continue;
                }

                let expanded =
                    expand_proxy_header_value(v, &remote, &inbound_headers, state.rule.ssl_enable);

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

        // 移除 Authorization（如需要）
        if state.rule.basic_auth_enable && !state.rule.basic_auth_forward_header {
            final_headers.remove(axum::http::header::AUTHORIZATION);
        }

        // 构造上游请求
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
        upstream_req.headers_mut().extend(final_headers);

        let outbound_headers_snapshot = upstream_req.headers().clone();

        // 发送请求
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
        push_log_lazy(&state.app, || {
            format_access_log(
                node,
                &ctx,
                StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            )
        });

        metrics::try_enqueue_request_log(metrics::RequestLogInsert {
            timestamp: chrono::Utc::now().timestamp(),
            listen_addr: node.to_string(),
            client_ip: ctx.client_ip.clone(),
            remote_ip: remote.ip().to_string(),
            method: ctx.method.as_str().to_string(),
            request_path: ctx.path.clone(),
            request_host: ctx.host_header.clone(),
            status_code: status.as_u16() as i32,
            upstream: target.clone(),
            latency_ms: ctx.elapsed_ms(),
            user_agent: ctx.user_agent_header.clone(),
            referer: ctx.referer_header.clone(),
            matched_route_id: matched_route_id.clone(),
        });

        let mut out = Response::new(Body::empty());
        *out.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

        for (k, v) in resp.headers().iter() {
            if is_hop_header_fast(k.as_str()) {
                continue;
            }
            out.headers_mut().insert(k.clone(), v.clone());
        }

        // 响应体处理
        if state.stream_proxy {
            let stream = resp.bytes_stream();
            *out.body_mut() = Body::from_stream(stream);
        } else {
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

            if state.max_response_body_size > 0 && bytes.len() > state.max_response_body_size {
                return (
                    StatusCode::BAD_GATEWAY,
                    format!(
                        "upstream body too large (limit={} bytes)",
                        state.max_response_body_size
                    ),
                )
                    .into_response();
            }

            *out.body_mut() = Body::from(bytes);
        }

        // 仅错误时记录详细日志
        if !status.is_success() {
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

            send_log(format!(
                "反代错误(IN): {} {} -> {} status={} | inbound_headers=[{}]",
                ctx.method.as_str(),
                ctx.uri,
                target,
                status.as_u16(),
                inbound_headers_line
            ));

            send_log(format!(
                "反代错误(OUT): {} {} -> {} status={} | outbound_headers=[{}] | req_body_size={}",
                ctx.method.as_str(),
                ctx.uri,
                target,
                status.as_u16(),
                outbound_headers_line,
                req_body_size
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "stream".to_string())
            ));
        }

        return out;
    }

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

#[inline]
fn is_asset_path(path: &str) -> bool {
    path.contains('.') || path.starts_with("/assets/") || path.starts_with("/static/")
}

// 使用预计算的 HashSet，性能更好
#[inline]
fn is_hop_header_fast(name: &str) -> bool {
    // 0 分配：HTTP header 名大小写不敏感，直接用 eq_ignore_ascii_case
    // 覆盖常见 hop-by-hop headers
    name.eq_ignore_ascii_case("connection")
        || name.eq_ignore_ascii_case("keep-alive")
        || name.eq_ignore_ascii_case("proxy-authenticate")
        || name.eq_ignore_ascii_case("proxy-authorization")
        || name.eq_ignore_ascii_case("te")
        || name.eq_ignore_ascii_case("trailer")
        || name.eq_ignore_ascii_case("transfer-encoding")
        || name.eq_ignore_ascii_case("upgrade")
}

fn expand_proxy_header_value(raw: &str, remote: &SocketAddr, inbound_headers: &HeaderMap, is_tls: bool) -> String {
    // 仅在真的包含变量时才分配
    if !(raw.contains('$')) {
        return raw.to_string();
    }

    let remote_ip = remote.ip().to_string();
    let scheme = if is_tls { "https" } else { "http" };

    // 仅在需要时计算 $proxy_add_x_forwarded_for
    let proxy_add_xff = if raw.contains("$proxy_add_x_forwarded_for") {
        let prior = inbound_headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        Some(match prior {
            Some(p) => format!("{}, {}", p, remote_ip),
            None => remote_ip.clone(),
        })
    } else {
        None
    };

    // 预估容量，尽量一次分配
    let mut out = String::with_capacity(raw.len() + 32);
    let bytes = raw.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let rest = &raw[i..];
            if rest.starts_with("$remote_addr") {
                out.push_str(&remote_ip);
                i += "$remote_addr".len();
                continue;
            }
            if rest.starts_with("$scheme") {
                out.push_str(scheme);
                i += "$scheme".len();
                continue;
            }
            if rest.starts_with("$proxy_add_x_forwarded_for") {
                if let Some(v) = proxy_add_xff.as_ref() {
                    out.push_str(v);
                }
                i += "$proxy_add_x_forwarded_for".len();
                continue;
            }
        }

        // 退化为逐字节拷贝（utf8 安全：这里拷的是原始 bytes，对应原始字符串片段）
        // 为避免 utf8 边界问题，用 char 级推进
        let ch = raw[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}
