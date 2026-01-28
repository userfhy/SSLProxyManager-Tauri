use anyhow::{anyhow, Context, Result};
use axum::{
    body::Bytes,
    extract::{
        connect_info::ConnectInfo,
        ws::{self},
        FromRef,
        State,
        WebSocketUpgrade,
    },
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use std::{net::SocketAddr, sync::Arc};
use tauri::Emitter;
use tracing::{error, info};

use crate::{access_control, config};

static WS_SERVERS: RwLock<Vec<WsServerHandle>> = RwLock::new(Vec::new());

struct WsServerHandle {
    handle: tauri::async_runtime::JoinHandle<()>,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl WsServerHandle {
    fn abort(self) {
        let _ = self.shutdown_tx.send(());
        self.handle.abort();
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WsRoute {
    pub path: String,
    pub upstream_url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WsListenRule {
    pub enabled: bool,
    pub listen_addr: String,
    pub ssl_enable: bool,
    pub cert_file: String,
    pub key_file: String,
    pub routes: Vec<WsRoute>,
}

#[derive(Clone)]
struct WsAppState {
    rule: WsListenRule,
    app: tauri::AppHandle,
    ws_access_control_enabled: bool,
    allow_all_lan: bool,
    whitelist: Arc<[config::WhitelistEntry]>,
}

#[derive(Clone)]
struct WsRuleState(Arc<WsListenRule>);

#[derive(Clone)]
struct AppHandleState(tauri::AppHandle);

impl FromRef<WsAppState> for WsRuleState {
    fn from_ref(input: &WsAppState) -> Self {
        Self(Arc::new(input.rule.clone()))
    }
}

impl FromRef<WsAppState> for AppHandleState {
    fn from_ref(input: &WsAppState) -> Self {
        Self(input.app.clone())
    }
}

pub fn start_ws_servers(app: tauri::AppHandle) -> Result<()> {
    let cfg = config::get_config();

    if !cfg.ws_proxy_enabled {
        return Ok(());
    }

    let Some(ws_rules) = cfg.ws_proxy.clone() else {
        return Ok(());
    };

    for ws_rule in ws_rules {
        if !ws_rule.enabled {
            continue;
        }

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let app2 = app.clone();
        let handle = tauri::async_runtime::spawn(async move {
            let listen_addr = ws_rule.listen_addr.clone();
            if let Err(e) = start_ws_rule_server(app2.clone(), ws_rule, shutdown_rx).await {
                error!("WS server failed({listen_addr}): {e}");
            }
        });

        WS_SERVERS
            .write()
            .push(WsServerHandle { handle, shutdown_tx });
    }

    Ok(())
}

pub fn stop_ws_servers() {
    let handles = std::mem::take(&mut *WS_SERVERS.write());
    for h in handles {
        h.abort();
    }
}

async fn start_ws_rule_server(
    app: tauri::AppHandle,
    rule: WsListenRule,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let (addr, need_dual_stack) = parse_listen_addr(&rule.listen_addr)?;

    let cfg = config::get_config();

    let state = WsAppState {
        rule: rule.clone(),
        app: app.clone(),
        ws_access_control_enabled: cfg.ws_access_control_enabled,
        allow_all_lan: cfg.allow_all_lan,
        whitelist: Arc::from(cfg.whitelist),
    };

    let router = Router::new().route("/healthz", any(|| async { (StatusCode::OK, "OK") }));
    let app_router = router.fallback(any(ws_handler)).with_state(state);
    let app_router = app_router.into_make_service_with_connect_info::<SocketAddr>();

    info!("WS listen {} -> {}", rule.listen_addr, addr);

    if rule.ssl_enable {
        let tls_cfg = axum_server::tls_rustls::RustlsConfig::from_pem_file(
            rule.cert_file.clone(),
            rule.key_file.clone(),
        )
        .await
        .with_context(|| "加载 WS TLS 证书/私钥失败")?;

        let mut shutdown_rx = shutdown_rx;
        
        if need_dual_stack && addr.is_ipv6() {
            // 在 Linux 上，绑定 [::]:port 通常已经启用了 IPv6 dual-stack，
            // 可以同时处理 IPv4 和 IPv6 连接，不需要再绑定 0.0.0.0:port
            // 如果系统不支持 dual-stack，绑定会失败，此时可以回退到只绑定 IPv4
            info!("监听 IPv6 (dual-stack): {} (同时支持 IPv4 和 IPv6)", addr);
            
            let server_future = axum_server::bind_rustls(addr, tls_cfg).serve(app_router);
            tokio::select! {
                res = server_future => {
                    res.map_err(|e| anyhow!("WS HTTPS 服务失败: {e}"))?;
                }
                _ = &mut shutdown_rx => {
                    info!("收到关闭信号，WS HTTPS 服务 {} 即将停止", addr);
                }
            }
        } else {
            let server_future = axum_server::bind_rustls(addr, tls_cfg).serve(app_router);
            tokio::select! {
                res = server_future => {
                    res.map_err(|e| anyhow!(e))?;
                }
                _ = &mut shutdown_rx => {
                    info!("收到关闭信号，WS HTTPS 服务 {} 即将停止", addr);
                }
            }
        }
    } else {
        let mut shutdown_rx = shutdown_rx;
        
        if need_dual_stack && addr.is_ipv6() {
            // 在 Linux 上，绑定 [::]:port 通常已经启用了 IPv6 dual-stack，
            // 可以同时处理 IPv4 和 IPv6 连接，不需要再绑定 0.0.0.0:port
            // 如果系统不支持 dual-stack，绑定会失败，此时可以回退到只绑定 IPv4
            info!("监听 IPv6 (dual-stack): {} (同时支持 IPv4 和 IPv6)", addr);
            
            let listener = tokio::net::TcpListener::bind(addr).await?;
            let server_future = axum::serve(listener, app_router);
            tokio::select! {
                res = server_future => {
                    res.map_err(|e| anyhow!("WS HTTP 服务失败: {e}"))?;
                }
                _ = &mut shutdown_rx => {
                    info!("收到关闭信号，WS HTTP 服务 {} 即将停止", addr);
                }
            }
        } else {
            let listener = tokio::net::TcpListener::bind(addr).await?;
            let server_future = axum::serve(listener, app_router);
            tokio::select! {
                res = server_future => {
                    res.map_err(|e| anyhow!(e))?;
                }
                _ = &mut shutdown_rx => {
                    info!("收到关闭信号，WS HTTP 服务 {} 即将停止", addr);
                }
            }
        }
    }

    Ok(())
}

async fn ws_handler(
    ConnectInfo(remote): ConnectInfo<SocketAddr>,
    State(WsRuleState(rule)): State<WsRuleState>,
    State(AppHandleState(app)): State<AppHandleState>,
    State(state): State<WsAppState>,
    uri: Uri,
    ws: WebSocketUpgrade,
    headers: HeaderMap,
) -> Response {
    // 访问控制（与 HTTP 代理一致）：黑名单优先，其次白名单，再次 allow_all_lan
    if state.ws_access_control_enabled
        && !access_control::is_allowed_fast(&remote, &headers, state.allow_all_lan, &state.whitelist)
    {
        let ip = access_control::client_ip_from_headers(&remote, &headers);
        let _ = app.emit("log-line", format!("WS forbidden: ip={ip} path={}", uri.path()));
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }

    let path = uri.path().to_string();

    let route = match_ws_route(&rule.routes, &path);
    let Some(route) = route else {
        return (StatusCode::NOT_FOUND, "No WS route").into_response();
    };

    let upstream = route.upstream_url.clone();

    ws.on_upgrade(move |socket| async move {
        if let Err(e) = proxy_ws(socket, upstream).await {
            let _ = app.emit("log-line", format!("WS proxy error: {e}"));
        }
    })
}

async fn proxy_ws(client: ws::WebSocket, upstream_url: String) -> Result<()> {
    let (upstream, _) = tokio_tungstenite::connect_async(&upstream_url)
        .await
        .with_context(|| format!("connect upstream ws failed: {upstream_url}"))?;

    let (mut u_tx, mut u_rx) = upstream.split();
    let (mut c_tx, mut c_rx) = client.split();

    let c_to_u = async {
        while let Some(msg) = c_rx.next().await {
            let msg = msg.map_err(|e| anyhow!(e))?;
            let tmsg = match msg {
                ws::Message::Text(s) => tokio_tungstenite::tungstenite::Message::Text(s.to_string().into()),
                ws::Message::Binary(b) => tokio_tungstenite::tungstenite::Message::Binary(b),
                ws::Message::Ping(b) => tokio_tungstenite::tungstenite::Message::Ping(b),
                ws::Message::Pong(b) => tokio_tungstenite::tungstenite::Message::Pong(b),
                ws::Message::Close(c) => {
                    let frame = c.map(|c| tokio_tungstenite::tungstenite::protocol::CloseFrame {
                        code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::from(
                            u16::from(c.code),
                        ),
                        reason: c.reason.to_string().into(),
                    });
                    tokio_tungstenite::tungstenite::Message::Close(frame)
                }
            };
            u_tx.send(tmsg).await.map_err(|e| anyhow!(e))?;
        }
        Result::<()>::Ok(())
    };

    let u_to_c = async {
        while let Some(msg) = u_rx.next().await {
            let msg = msg.map_err(|e| anyhow!(e))?;
            let amsg = match msg {
                tokio_tungstenite::tungstenite::Message::Text(s) => ws::Message::Text(s.to_string().into()),
                tokio_tungstenite::tungstenite::Message::Binary(b) => ws::Message::Binary(Bytes::from(b)),
                tokio_tungstenite::tungstenite::Message::Ping(b) => ws::Message::Ping(Bytes::from(b)),
                tokio_tungstenite::tungstenite::Message::Pong(b) => ws::Message::Pong(Bytes::from(b)),
                tokio_tungstenite::tungstenite::Message::Close(c) => {
                    let close = c.map(|c| ws::CloseFrame {
                        code: ws::CloseCode::from(u16::from(c.code)),
                        reason: ws::Utf8Bytes::from(c.reason.to_string()),
                    });
                    ws::Message::Close(close)
                }
                tokio_tungstenite::tungstenite::Message::Frame(_) => {
                    continue;
                }
            };
            c_tx.send(amsg).await.map_err(|e| anyhow!(e))?;
        }
        Result::<()>::Ok(())
    };

    tokio::select! {
        r = c_to_u => { r?; }
        r = u_to_c => { r?; }
    }

    Ok(())
}

fn match_ws_route<'a>(routes: &'a [WsRoute], path: &str) -> Option<&'a WsRoute> {
    routes
        .iter()
        .filter(|r| path.starts_with(r.path.as_str()))
        .max_by_key(|r| r.path.len())
}

/// 解析监听地址，返回主地址和是否需要同时绑定 IPv4/IPv6
fn parse_listen_addr(s: &str) -> Result<(SocketAddr, bool)> {
    let trimmed = s.trim();
    let (normalized, need_dual_stack) = if trimmed.starts_with(':') {
        // :port 格式：同时监听 IPv4 和 IPv6
        let port = trimmed;
        let ipv6_format = format!("[::]{}", port);
        let ipv4_format = format!("0.0.0.0{}", port);
        
        // 优先使用 IPv6，因为它通常可以同时监听 IPv4（dual-stack）
        if let Ok(addr) = ipv6_format.parse::<SocketAddr>() {
            (addr, true) // 标记需要同时绑定
        } else if let Ok(addr) = ipv4_format.parse::<SocketAddr>() {
            (addr, true) // 即使 IPv6 失败，也标记需要同时绑定
        } else {
            return Err(anyhow::anyhow!("解析 ws listen_addr 失败: {s}"));
        }
    } else {
        // 完整地址格式：直接解析
        let addr = trimmed
            .parse::<SocketAddr>()
            .with_context(|| format!("解析 ws listen_addr 失败: {s}"))?;
        (addr, false)
    };

    Ok((normalized, need_dual_stack))
}
