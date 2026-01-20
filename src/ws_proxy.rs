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

use crate::config;

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
    let addr = parse_listen_addr(&rule.listen_addr)?;

    let state = WsAppState {
        rule: rule.clone(),
        app: app.clone(),
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
        let server_future = axum_server::bind_rustls(addr, tls_cfg).serve(app_router);
        tokio::select! {
            res = server_future => {
                res.map_err(|e| anyhow!(e))?;
            }
            _ = &mut shutdown_rx => {
                info!("收到关闭信号，WS HTTPS 服务 {} 即将停止", addr);
            }
        }
    } else {
        let mut shutdown_rx = shutdown_rx;
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

    Ok(())
}

async fn ws_handler(
    ConnectInfo(_remote): ConnectInfo<SocketAddr>,
    State(WsRuleState(rule)): State<WsRuleState>,
    State(AppHandleState(app)): State<AppHandleState>,
    uri: Uri,
    ws: WebSocketUpgrade,
    _headers: HeaderMap,
) -> Response {
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
                ws::Message::Text(s) => tokio_tungstenite::tungstenite::Message::Text(s.to_string()),
                ws::Message::Binary(b) => tokio_tungstenite::tungstenite::Message::Binary(b.to_vec()),
                ws::Message::Ping(b) => tokio_tungstenite::tungstenite::Message::Ping(b.to_vec()),
                ws::Message::Pong(b) => tokio_tungstenite::tungstenite::Message::Pong(b.to_vec()),
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
                tokio_tungstenite::tungstenite::Message::Text(s) => ws::Message::Text(s.into()),
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

fn parse_listen_addr(s: &str) -> Result<SocketAddr> {
    let trimmed = s.trim();
    let normalized = if trimmed.starts_with(':') {
        format!("0.0.0.0{}", trimmed)
    } else {
        trimmed.to_string()
    };

    normalized
        .parse::<SocketAddr>()
        .with_context(|| format!("解析 ws listen_addr 失败: {s}"))
}
