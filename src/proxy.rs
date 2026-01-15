use crate::{config, metrics};
use anyhow::{anyhow, Context, Result};
use axum::{
    body::{to_bytes, Body},
    extract::{connect_info::ConnectInfo, FromRef, State},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use parking_lot::RwLock;
use reqwest::redirect::Policy;
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tauri::{Emitter, Manager};
use tower::util::ServiceExt;
use tower_http::services::ServeDir;
use tracing::{error, info};

static IS_RUNNING: RwLock<bool> = RwLock::new(false);
static SERVERS: RwLock<Vec<tauri::async_runtime::JoinHandle<()>>> = RwLock::new(Vec::new());
static LOGS: RwLock<Vec<String>> = RwLock::new(Vec::new());

// 启动过程控制：要求所有 rules 都成功（你选的语义 B）
static STARTING: RwLock<bool> = RwLock::new(false);
static START_EXPECTED: RwLock<usize> = RwLock::new(0);
static START_FAILED: RwLock<bool> = RwLock::new(false);
static START_STARTED_COUNT: RwLock<usize> = RwLock::new(0);

#[derive(Clone)]
struct AppState {
    rule: config::ListenRule,
    client: reqwest::Client,
    app: tauri::AppHandle,
}

#[derive(Clone)]
struct RuleState(Arc<config::ListenRule>);

#[derive(Clone)]
struct HttpClient(reqwest::Client);

impl FromRef<AppState> for RuleState {
    fn from_ref(input: &AppState) -> Self {
        Self(Arc::new(input.rule.clone()))
    }
}

impl FromRef<AppState> for HttpClient {
    fn from_ref(input: &AppState) -> Self {
        Self(input.client.clone())
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
    // 如果正在启动，直接返回（避免重复点击并发启动）
    {
        let starting = STARTING.read();
        if *starting {
            return Ok(());
        }
    }

    // 已经 running 则直接返回
    {
        let running = IS_RUNNING.read();
        if *running {
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
            match start_rule_server(app_handle.clone(), rule).await {
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
        handles.push(handle);
    }

    *SERVERS.write() = handles;

    // 这里不再设置 IS_RUNNING=true，也不提示“已启动”。
    // 由每个 rule 真正 bind 成功后汇总决定。
    send_log("代理服务器启动中...".to_string());
    Ok(())
}

pub fn stop_server(app: tauri::AppHandle) -> Result<()> {
    // 无论当前状态如何，停止都要尽量清理启动中的状态
    *STARTING.write() = false;
    *START_FAILED.write() = false;
    *START_EXPECTED.write() = 0;
    *START_STARTED_COUNT.write() = 0;

    let mut running = IS_RUNNING.write();
    if !*running {
        // 也要通知前端，确保按钮状态正确
        let _ = app.emit("status", "stopped");
        return Ok(());
    }
    *running = false;
    drop(running);

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

    // 推送前端
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

async fn start_rule_server(app: tauri::AppHandle, rule: config::ListenRule) -> Result<()> {
    let addr = parse_listen_addr(&rule.listen_addr)?;

    let client = reqwest::Client::builder()
        .redirect(Policy::limited(10))
        .danger_accept_invalid_certs(true)
        .build()
        .context("创建上游 HTTP client 失败")?;

    let state = AppState {
        rule: rule.clone(),
        client,
        app: app.clone(),
    };

    let mut router = Router::new().route("/healthz", any(healthz));

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

        axum_server::bind_rustls(addr, tls_cfg)
            .serve(app)
            .await
            .map_err(|e| anyhow!(e))?;
    } else {
        send_log(format!("HTTP 已启用: {}", addr));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await.map_err(|e| anyhow!(e))?;
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
    // 优先取 X-Forwarded-For 的第一个
    if let Some(v) = headers.get("x-forwarded-for") {
        if let Ok(s) = v.to_str() {
            let first = s.split(',').next().unwrap_or("").trim();
            if !first.is_empty() {
                return first.to_string();
            }
        }
    }
    if let Some(v) = headers.get("x-real-ip") {
        if let Ok(s) = v.to_str() {
            let s = s.trim();
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }

    // 没有任何转发头：回退到真实 TCP 连接地址
    remote.ip().to_string()
}

fn parse_ip(s: &str) -> Option<IpAddr> {
    s.trim().parse::<IpAddr>().ok()
}

fn is_lan_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 10
                || (o[0] == 172 && (16..=31).contains(&o[1]))
                || (o[0] == 192 && o[1] == 168)
                || (o[0] == 169 && o[1] == 254)
        }
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

fn is_ip_whitelisted(ip: &IpAddr, cfg: &config::Config) -> bool {
    cfg.whitelist
        .iter()
        .any(|e| parse_ip(&e.ip).as_ref() == Some(ip))
}

fn is_access_allowed(remote: &SocketAddr, headers: &HeaderMap, cfg: &config::Config) -> bool {
    let mut ip = remote.ip();

    // 如果前面有反代，并且你希望“信任转发头”，可以用 header 覆盖 remote.ip()
    if let Some(h) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(parse_ip)
    {
        ip = h;
    } else if let Some(h) = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(parse_ip)
    {
        ip = h;
    }

    // 访问控制优先级最高：
    // 1) 白名单命中 => 允许
    // 2) 未命中白名单：只有在 allow_all_lan=true 且来源为局域网 IP 才允许
    if is_ip_whitelisted(&ip, cfg) {
        return true;
    }

    if cfg.allow_all_lan && is_lan_ip(&ip) {
        return true;
    }

    false
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
        if real != "-" { real } else { "-".to_string() }
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

fn push_log(app: &tauri::AppHandle, line: String) {
    // 写入内存
    {
        let mut logs = LOGS.write();
        logs.push(line.clone());
        if logs.len() > 3000 {
            let over = logs.len() - 3000;
            logs.drain(0..over);
        }
    }

    // 推送到前端
    let _ = app.emit("log-line", line);
}

async fn proxy_handler(
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    State(RuleState(rule)): State<RuleState>,
    State(HttpClient(client)): State<HttpClient>,
    State(AppHandleState(app)): State<AppHandleState>,
    req: Request<Body>,
) -> Response {
    let started_at = std::time::Instant::now();
    let node = rule.listen_addr.clone();
    let headers_snapshot = req.headers().clone();
    let path = req.uri().path().to_string();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let route = match_route(&rule.routes, &path);

    // 0. 访问控制（优先级最高）
    // allow_all_lan 取消勾选时：只有白名单 IP 可以访问
    {
        let cfg = config::get_config();

        // 黑名单优先：命中直接拒绝
        let client_ip_str = {
            // 与 is_access_allowed 采用相同的 IP 选择逻辑：默认用 remote.ip()，若有转发头则优先用头
            let mut ip = remote.ip();
            if let Some(h) = req
                .headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .and_then(parse_ip)
            {
                ip = h;
            } else if let Some(h) = req
                .headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .and_then(parse_ip)
            {
                ip = h;
            }
            ip.to_string()
        };

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
    if let Some(upstream) = route.upstreams.first() {
        let target = match build_upstream_url(&upstream.url, route.proxy_pass_path.as_deref(), &uri)
        {
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
                    upstream: upstream.url.clone(),
                    latency_ms: elapsed * 1000.0,
                    user_agent: header_str(&headers_snapshot, "user-agent"),
                    referer: header_str(&headers_snapshot, "referer"),
                });

                return (StatusCode::BAD_GATEWAY, format!("bad upstream url: {e}")).into_response();
            }
        };

        send_log(format!(
            "反代: {} {} -> {}",
            req.method(),
            req.uri(),
            target
        ));

        let target = target;
        let mut builder = client.request(req.method().clone(), target.clone());

        // 复制请求头
        for (k, v) in req.headers().iter() {
            if !is_hop_header(k.as_str()) {
                builder = builder.header(k, v);
            }
        }

        // 处理 Basic Auth 转发
        if !rule.basic_auth_forward_header {
            builder = builder.header(axum::http::header::AUTHORIZATION, "");
        }

        // 处理请求体
        let body_bytes = match to_bytes(req.into_body(), usize::MAX).await {
            Ok(b) => b,
            Err(e) => {
                return (StatusCode::BAD_GATEWAY, format!("read body failed: {e}")).into_response();
            }
        };

        // 发送请求并返回响应（复用原先的 reqwest -> axum Response 转换逻辑）
        let resp = match builder.body(body_bytes).send().await {
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

        *out.body_mut() = Body::from(bytes);
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
    proxy_pass_path: Option<&str>,
    uri: &Uri,
) -> Result<String> {
    let mut base = upstream_base.trim_end_matches('/').to_string();
    let mut path = uri.path().to_string();

    if let Some(pp) = proxy_pass_path {
        if pp == "/" {
            if let Some(rest) = path.strip_prefix('/') {
                if let Some((_, remain)) = rest.split_once('/') {
                    path = format!("/{}", remain);
                } else {
                    path = "/".to_string();
                }
            }
        }
    }

    base.push_str(&path);
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
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}
