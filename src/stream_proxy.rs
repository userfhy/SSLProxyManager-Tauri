use anyhow::{anyhow, Context, Result};
use parking_lot::RwLock;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio::time;
use dashmap::DashMap;

use crate::{access_control, config};
use crate::config::{StreamProxyConfig, StreamServer, StreamUpstream, StreamUpstreamServer};

static STREAM_SERVERS: once_cell::sync::Lazy<RwLock<Vec<StreamServerHandle>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));

struct StreamServerHandle {
    task: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: mpsc::Sender<()>,
}

#[derive(Debug, Clone)]
struct FailState {
    fails: u32,
    down_until: Option<Instant>,
}

// 使用 DashMap 替代 RwLock<HashMap> 以提升并发性能
static FAIL_MAP: once_cell::sync::Lazy<DashMap<String, FailState>> =
    once_cell::sync::Lazy::new(DashMap::new);

#[inline]
fn stream_log(app: &tauri::AppHandle, message: impl Into<String>) {
    crate::proxy::send_log_with_app(app, format!("[STREAM] {}", message.into()));
}

pub async fn start_stream_servers(app: tauri::AppHandle, config: &StreamProxyConfig) -> Result<()> {
    stop_stream_servers().await;

    if !config.enabled {
        return Ok(());
    }

    validate_stream_config(config)?;

    let mut handles = Vec::new();

    for server in &config.servers {
        if !server.enabled {
            continue;
        }

        let upstream = config
            .upstreams
            .iter()
            .find(|u| u.name == server.proxy_pass)
            .expect("validate_stream_config should ensure upstream exists");

        let connect_timeout =
            parse_duration(&server.proxy_connect_timeout).unwrap_or_else(|_| Duration::from_secs(300));
        let proxy_timeout =
            parse_duration(&server.proxy_timeout).unwrap_or_else(|_| Duration::from_secs(600));

        if server.udp {
            start_udp_server(&app, server, upstream, connect_timeout, proxy_timeout, &mut handles).await?;
        } else {
            start_tcp_server(&app, server, upstream, connect_timeout, proxy_timeout, &mut handles).await?;
        }
    }

    *STREAM_SERVERS.write() = handles;
    Ok(())
}

pub fn validate_stream_config(cfg: &StreamProxyConfig) -> Result<()> {
    let mut ports = HashSet::<(u16, bool)>::new();
    for s in &cfg.servers {
        if !s.enabled {
            continue;
        }
        let key = (s.listen_port, s.udp);
        if !ports.insert(key) {
            return Err(anyhow!(
                "stream server listen_port duplicated: port={} udp={}",
                s.listen_port,
                s.udp
            ));
        }
    }

    let mut up_names = HashSet::<String>::new();
    for u in &cfg.upstreams {
        let name = u.name.trim();
        if name.is_empty() {
            return Err(anyhow!("stream upstream name cannot be empty"));
        }
        if !up_names.insert(name.to_string()) {
            return Err(anyhow!("stream upstream name duplicated: {}", name));
        }
        if u.servers.is_empty() {
            return Err(anyhow!("stream upstream '{}' has no servers", name));
        }
        for sv in &u.servers {
            if sv.addr.trim().is_empty() {
                return Err(anyhow!("stream upstream '{}' has empty server addr", name));
            }
            let parts: Vec<&str> = sv.addr.split(':').collect();
            if parts.len() < 2 {
                return Err(anyhow!("invalid stream server addr (need host:port): {}", sv.addr));
            }
            let port_str = parts.last().unwrap().trim();
            if port_str.parse::<u16>().is_err() {
                return Err(anyhow!("invalid stream server addr port: {}", sv.addr));
            }
            let _ = parse_duration(&sv.fail_timeout)
                .map_err(|e| anyhow!("invalid fail_timeout for {}: {}", sv.addr, e))?;
        }
    }

    for s in &cfg.servers {
        if !s.enabled {
            continue;
        }
        let pp = s.proxy_pass.trim();
        if pp.is_empty() {
            return Err(anyhow!(
                "stream server (listen_port={}) proxy_pass cannot be empty",
                s.listen_port
            ));
        }
        let Some(u) = cfg.upstreams.iter().find(|u| u.name == pp) else {
            return Err(anyhow!(
                "stream server (listen_port={}) proxy_pass references missing upstream: {}",
                s.listen_port,
                pp
            ));
        };
        if u.servers.is_empty() {
            return Err(anyhow!(
                "stream server (listen_port={}) proxy_pass upstream '{}' has no servers",
                s.listen_port,
                pp
            ));
        }

        let _ = parse_duration(&s.proxy_connect_timeout).map_err(|e| {
            anyhow!(
                "invalid proxy_connect_timeout: {} ({})",
                s.proxy_connect_timeout,
                e
            )
        })?;
        let _ = parse_duration(&s.proxy_timeout)
            .map_err(|e| anyhow!("invalid proxy_timeout: {} ({})", s.proxy_timeout, e))?;
    }

    Ok(())
}

async fn start_tcp_server(
    app: &tauri::AppHandle,
    server: &StreamServer,
    upstream: &StreamUpstream,
    connect_timeout: Duration,
    proxy_timeout: Duration,
    servers: &mut Vec<StreamServerHandle>,
) -> Result<()> {
    // 如果指定了 listen_addr，使用它；否则使用默认的 0.0.0.0:port
    let listen_addr = if let Some(addr) = &server.listen_addr {
        addr.clone()
    } else {
        format!("0.0.0.0:{}", server.listen_port)
    };
    let listener = TcpListener::bind(&listen_addr)
        .await
        .with_context(|| format!("Failed to bind stream tcp listener: {}", listen_addr))?;

    let bound_addr = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| listen_addr.clone());

    tracing::info!("Stream TCP server listening on {} -> {}", bound_addr, upstream.name);
    stream_log(
        app,
        format!(
            "TCP listening address: {} -> {} (upstream={})",
            listen_addr, bound_addr, upstream.name
        ),
    );

    let cfg = config::get_config();
    let access_control_enabled = cfg.stream_access_control_enabled;
    let allow_all_lan = cfg.allow_all_lan;
    let allow_all_ip = cfg.allow_all_ip;
    let whitelist: Arc<[config::WhitelistEntry]> = Arc::from(cfg.whitelist);

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
    let server_task = tokio::spawn({
        let upstream = upstream.clone();
        let whitelist = whitelist.clone();
        async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((client_socket, client_addr)) => {
                                // 访问控制/黑名单：TCP stream 没有 headers，仅按 remote ip 判定
                                if access_control_enabled {
                                    let headers = axum::http::HeaderMap::new();
                                    if !access_control::is_allowed_fast(
                                        &client_addr,
                                        &headers,
                                        allow_all_lan,
                                        allow_all_ip,
                                        &whitelist,
                                    ) {
                                        tracing::warn!(
                                            "STREAM TCP forbidden: ip={} upstream={}",
                                            client_addr.ip(),
                                            upstream.name
                                        );
                                        continue;
                                    }
                                }

                                let upstream = upstream.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_tcp_client(
                                        client_socket,
                                        client_addr,
                                        &upstream,
                                        connect_timeout,
                                        proxy_timeout,
                                    )
                                    .await
                                    {
                                        tracing::error!("TCP client {} error: {}", client_addr, e);
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::error!("Error accepting TCP connection: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Shutting down TCP server {}", listen_addr);
                        break;
                    }
                }
            }
        }
    });

    servers.push(StreamServerHandle {
        task: Some(server_task),
        shutdown_tx,
    });

    Ok(())
}

async fn handle_tcp_client(
    client_socket: TcpStream,
    client_addr: SocketAddr,
    upstream: &StreamUpstream,
    connect_timeout: Duration,
    proxy_timeout: Duration,
) -> Result<()> {
    let Some(server) = select_upstream_server_with_failover(upstream, &client_addr) else {
        return Err(anyhow!(
            "no available upstream servers (all down?) upstream={}",
            upstream.name
        ));
    };

    let server_addr = server.addr.clone();

    let server_socket: TcpStream =
        match time::timeout(connect_timeout, TcpStream::connect(&server_addr)).await {
            Ok(Ok(socket)) => socket,
            Ok(Err(e)) => {
                record_upstream_failure(&server_addr, server.max_fails, &server.fail_timeout);
                return Err(anyhow!("Failed to connect to upstream {}: {}", server_addr, e));
            }
            Err(_) => {
                record_upstream_failure(&server_addr, server.max_fails, &server.fail_timeout);
                return Err(anyhow!(
                    "Connection to upstream {} timed out after {:?}",
                    server_addr,
                    connect_timeout
                ));
            }
        };

    record_upstream_success(&server_addr);

    let mut client = client_socket;
    let mut upstream_conn = server_socket;

    let relay = async {
        let _ = io::copy_bidirectional(&mut client, &mut upstream_conn).await?;
        Ok::<_, std::io::Error>(())
    };

    match time::timeout(proxy_timeout, relay).await {
        Ok(res) => {
            if let Err(e) = res {
                tracing::debug!(
                    "TCP relay io error (client={} upstream={}): {}",
                    client_addr,
                    server_addr,
                    e
                );
            }
        }
        Err(_) => {
            tracing::debug!(
                "TCP relay timeout (client={} upstream={} timeout={:?})",
                client_addr,
                server_addr,
                proxy_timeout
            );
        }
    }

    Ok(())
}

#[derive(Clone)]
struct UdpSessionEntry {
    upstream_addr: SocketAddr,
    last_seen_ms: u64,
}

static UDP_NOW_MS: AtomicU64 = AtomicU64::new(0);
static UDP_START_TIME: once_cell::sync::Lazy<std::time::Instant> =
    once_cell::sync::Lazy::new(std::time::Instant::now);

fn now_ms() -> u64 {
    let ms = UDP_START_TIME.elapsed().as_millis() as u64;
    UDP_NOW_MS.fetch_max(ms, Ordering::Relaxed);
    ms
}

async fn start_udp_server(
    app: &tauri::AppHandle,
    server: &StreamServer,
    upstream: &StreamUpstream,
    _connect_timeout: Duration,
    proxy_timeout: Duration,
    servers: &mut Vec<StreamServerHandle>,
) -> Result<()> {
    let cfg = config::get_config();
    let access_control_enabled = cfg.stream_access_control_enabled;
    let allow_all_lan = cfg.allow_all_lan;
    let allow_all_ip = cfg.allow_all_ip;
    let whitelist: Arc<[config::WhitelistEntry]> = Arc::from(cfg.whitelist);

    // 如果指定了 listen_addr，使用它；否则使用默认的 0.0.0.0:port
    let listen_addr = if let Some(addr) = &server.listen_addr {
        addr.clone()
    } else {
        format!("0.0.0.0:{}", server.listen_port)
    };
    let listen_sock = UdpSocket::bind(&listen_addr)
        .await
        .with_context(|| format!("Failed to bind to {}", listen_addr))?;

    let bound_addr = listen_sock
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| listen_addr.clone());
    tracing::info!("Stream UDP server listening on {}", bound_addr);
    stream_log(
        app,
        format!(
            "UDP listening address: {} -> {} (upstream={})",
            listen_addr, bound_addr, upstream.name
        ),
    );

    // 使用 DashMap 替代 Mutex<HashMap> 以提升并发性能
    let sessions: Arc<DashMap<SocketAddr, UdpSessionEntry>> = Arc::new(DashMap::new());

    let listen_sock = Arc::new(listen_sock);

    // 为每个 upstream_addr 复用一个已 connect 的 UDP socket：避免每包 bind/connect
    let upstream_socks: Arc<DashMap<SocketAddr, Arc<UdpSocket>>> = Arc::new(DashMap::new());

    let mut upstream_readers = Vec::new();
    for s in &upstream.servers {
        let upstream_addr: SocketAddr = s
            .addr
            .parse()
            .with_context(|| format!("Invalid upstream udp addr: {}", s.addr))?;

        if upstream_socks.contains_key(&upstream_addr) {
            continue;
        }

        let upstream_socket = UdpSocket::bind("0.0.0.0:0").await?;
        upstream_socket.connect(upstream_addr).await?;
        let upstream_socket = Arc::new(upstream_socket);

        upstream_socks.insert(upstream_addr, upstream_socket.clone());

        let sessions2 = sessions.clone();
        let listen2 = listen_sock.clone();

        let h = tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                match upstream_socket.recv(&mut buf).await {
                    Ok(n) => {
                        let payload = &buf[..n];

                        // 使用 DashMap 的迭代器，无需锁定整个 map
                        for entry in sessions2.iter() {
                            if entry.value().upstream_addr == upstream_addr {
                                let _ = listen2.send_to(payload, *entry.key()).await;
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        });

        upstream_readers.push((upstream_addr, h));
    }

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
    let server_task = tokio::spawn({
        let upstream = upstream.clone();
        let sessions = sessions.clone();
        let listen = listen_sock.clone();
        let whitelist = whitelist.clone();
        let upstream_socks = upstream_socks.clone();

        async move {
            let mut buf = vec![0u8; 65536];
            let cleanup_interval = Duration::from_secs(10);
            let session_ttl = proxy_timeout.max(Duration::from_secs(10));
            let mut ticker = time::interval(cleanup_interval);

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Shutting down UDP server {}", listen_addr);
                        break;
                    }
                    _ = ticker.tick() => {
                        let deadline = now_ms().saturating_sub(session_ttl.as_millis() as u64);
                        sessions.retain(|_, v| v.last_seen_ms >= deadline);
                    }
                    res = listen.recv_from(&mut buf) => {
                        match res {
                            Ok((n, client_addr)) => {
                                // 访问控制/黑名单：UDP 仅按 remote ip 判定
                                if access_control_enabled {
                                    let headers = axum::http::HeaderMap::new();
                                    if !access_control::is_allowed_fast(
                                        &client_addr,
                                        &headers,
                                        allow_all_lan,
                                        allow_all_ip,
                                        &whitelist,
                                    ) {
                                        continue;
                                    }
                                }

                                let up_server = select_upstream_server(&upstream, &client_addr);
                                let upstream_addr: SocketAddr = match up_server.addr.parse() {
                                    Ok(a) => a,
                                    Err(_) => continue,
                                };

                                sessions.insert(
                                    client_addr,
                                    UdpSessionEntry {
                                        upstream_addr,
                                        last_seen_ms: now_ms(),
                                    },
                                );

                                if let Some(s) = upstream_socks.get(&upstream_addr) {
                                    let _ = s.send(&buf[..n]).await;
                                }
                            }
                            Err(_) => {}
                        }
                    }
                }
            }

            for (_, h) in upstream_readers {
                h.abort();
            }
        }
    });

    servers.push(StreamServerHandle {
        task: Some(server_task),
        shutdown_tx,
    });

    Ok(())
}

fn select_upstream_server<'a>(upstream: &'a StreamUpstream, client_addr: &SocketAddr) -> &'a StreamUpstreamServer {
    let servers = &upstream.servers;
    if servers.is_empty() {
        panic!("No servers available in upstream '{}'", upstream.name);
    }

    let key = upstream.hash_key.trim();
    let use_hash = key == "$remote_addr" || key.is_empty();

    if use_hash {
        let mut hasher = DefaultHasher::new();
        client_addr.ip().to_string().hash(&mut hasher);
        let h = hasher.finish() as usize;
        let idx = h % servers.len();
        return &servers[idx];
    }

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let idx = (COUNTER.fetch_add(1, Ordering::Relaxed) as usize) % servers.len();
    &servers[idx]
}

fn select_upstream_server_with_failover<'a>(
    upstream: &'a StreamUpstream,
    client_addr: &SocketAddr,
) -> Option<&'a StreamUpstreamServer> {
    let servers = &upstream.servers;
    if servers.is_empty() {
        return None;
    }

    let key = upstream.hash_key.trim();
    let use_hash = key == "$remote_addr" || key.is_empty();

    if upstream.consistent && use_hash {
        let ring = get_or_build_ring(upstream);
        let mut hasher = DefaultHasher::new();
        client_addr.ip().to_string().hash(&mut hasher);
        let h = hasher.finish();

        if ring.is_empty() {
            return None;
        }

        let mut idx = match ring.binary_search_by_key(&h, |(k, _)| *k) {
            Ok(i) => i,
            Err(i) => if i >= ring.len() { 0 } else { i },
        };

        for _ in 0..ring.len() {
            let (_, sidx) = ring[idx];
            let s = &servers[sidx];
            if !is_down(&s.addr) {
                return Some(s);
            }
            idx = (idx + 1) % ring.len();
        }

        return None;
    }

    let start_idx = if use_hash {
        let mut hasher = DefaultHasher::new();
        client_addr.ip().to_string().hash(&mut hasher);
        (hasher.finish() as usize) % servers.len()
    } else {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        (COUNTER.fetch_add(1, Ordering::Relaxed) as usize) % servers.len()
    };

    for step in 0..servers.len() {
        let idx = (start_idx + step) % servers.len();
        let s = &servers[idx];
        if !is_down(&s.addr) {
            return Some(s);
        }
    }

    None
}

// 缓存一致性哈希环，避免重复构建
static HASH_RING_CACHE: once_cell::sync::Lazy<DashMap<String, Arc<Vec<(u64, usize)>>>> =
    once_cell::sync::Lazy::new(DashMap::new);

fn get_or_build_ring(upstream: &StreamUpstream) -> Arc<Vec<(u64, usize)>> {
    let cache_key = upstream.servers.iter()
        .map(|s| s.addr.as_str())
        .collect::<Vec<_>>()
        .join("|");

    if let Some(cached) = HASH_RING_CACHE.get(&cache_key) {
        return cached.value().clone();
    }

    let ring = Arc::new(build_ring(&upstream.servers));
    HASH_RING_CACHE.insert(cache_key, ring.clone());
    ring
}

fn build_ring(servers: &[StreamUpstreamServer]) -> Vec<(u64, usize)> {
    const VNODES: u32 = 160;

    let mut ring: Vec<(u64, usize)> = Vec::with_capacity(servers.len() * VNODES as usize);
    for (i, s) in servers.iter().enumerate() {
        if s.addr.trim().is_empty() {
            continue;
        }
        for v in 0..VNODES {
            let mut hasher = DefaultHasher::new();
            format!("{}#{}", s.addr, v).hash(&mut hasher);
            ring.push((hasher.finish(), i));
        }
    }

    ring.sort_unstable_by_key(|(k, _)| *k);
    ring
}

fn is_down(addr: &str) -> bool {
    let now = Instant::now();
    if let Some(st) = FAIL_MAP.get(addr) {
        match st.down_until {
            Some(t) => t > now,
            None => false,
        }
    } else {
        false
    }
}

fn record_upstream_success(addr: &str) {
    FAIL_MAP.remove(addr);
}

fn record_upstream_failure(addr: &str, max_fails: i32, fail_timeout: &str) {
    let max_fails = if max_fails <= 0 { 1 } else { max_fails as u32 };
    let ft = parse_duration(fail_timeout).unwrap_or_else(|_| Duration::from_secs(30));

    FAIL_MAP
        .entry(addr.to_string())
        .and_modify(|entry| {
            entry.fails = entry.fails.saturating_add(1);
            if entry.fails >= max_fails {
                entry.down_until = Some(Instant::now() + ft);
            }
        })
        .or_insert_with(|| {
            let mut state = FailState {
                fails: 1,
                down_until: None,
            };
            if state.fails >= max_fails {
                state.down_until = Some(Instant::now() + ft);
            }
            state
        });
}

fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim().to_lowercase();

    if s.ends_with('s') {
        let secs = s[..s.len() - 1]
            .parse::<u64>()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        Ok(Duration::from_secs(secs))
    } else if s.ends_with('m') {
        let mins = s[..s.len() - 1]
            .parse::<u64>()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        Ok(Duration::from_secs(mins * 60))
    } else if s.ends_with('h') {
        let hours = s[..s.len() - 1]
            .parse::<u64>()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        Ok(Duration::from_secs(hours * 3600))
    } else {
        let secs = s
            .parse::<u64>()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        Ok(Duration::from_secs(secs))
    }
}

pub async fn stop_stream_servers() {
    let servers: Vec<StreamServerHandle> = {
        let mut guard = STREAM_SERVERS.write();
        std::mem::take(&mut *guard)
    };

    for mut server in servers {
        let _ = server.shutdown_tx.send(()).await;
        if let Some(task) = server.task.take() {
            let _ = time::timeout(Duration::from_secs(5), task).await;
        }
    }
}
