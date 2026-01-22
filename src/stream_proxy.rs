use anyhow::{anyhow, Context, Result};
use parking_lot::RwLock;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, Mutex};
use tokio::time;

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

static FAIL_MAP: once_cell::sync::Lazy<RwLock<HashMap<String, FailState>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(HashMap::new()));

pub async fn start_stream_servers(config: &StreamProxyConfig) -> Result<()> {
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
            start_udp_server(server, upstream, connect_timeout, proxy_timeout, &mut handles).await?;
        } else {
            start_tcp_server(server, upstream, connect_timeout, proxy_timeout, &mut handles).await?;
        }
    }

    *STREAM_SERVERS.write() = handles;
    Ok(())
}

fn validate_stream_config(cfg: &StreamProxyConfig) -> Result<()> {
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
    server: &StreamServer,
    upstream: &StreamUpstream,
    connect_timeout: Duration,
    proxy_timeout: Duration,
    servers: &mut Vec<StreamServerHandle>,
) -> Result<()> {
    let listen_addr = format!("0.0.0.0:{}", server.listen_port);
    let listener = TcpListener::bind(&listen_addr)
        .await
        .with_context(|| format!("Failed to bind stream tcp listener: {}", listen_addr))?;

    tracing::info!("Stream TCP server listening on {} -> {}", listen_addr, upstream.name);

    let cfg = config::get_config();
    let access_control_enabled = cfg.stream_access_control_enabled;
    let allow_all_lan = cfg.allow_all_lan;
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

fn now_ms() -> u64 {
    let ms = time::Instant::now().elapsed().as_millis() as u64;
    UDP_NOW_MS.fetch_max(ms, Ordering::Relaxed);
    ms
}

async fn start_udp_server(
    server: &StreamServer,
    upstream: &StreamUpstream,
    _connect_timeout: Duration,
    proxy_timeout: Duration,
    servers: &mut Vec<StreamServerHandle>,
) -> Result<()> {
    let cfg = config::get_config();
    let access_control_enabled = cfg.stream_access_control_enabled;
    let allow_all_lan = cfg.allow_all_lan;
    let whitelist: Arc<[config::WhitelistEntry]> = Arc::from(cfg.whitelist);

    let listen_addr = format!("0.0.0.0:{}", server.listen_port);
    let listen_sock = UdpSocket::bind(&listen_addr)
        .await
        .with_context(|| format!("Failed to bind to {}", listen_addr))?;

    tracing::info!("Stream UDP server listening on {}", listen_addr);

    let sessions: Arc<Mutex<HashMap<SocketAddr, UdpSessionEntry>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let listen_sock = Arc::new(listen_sock);

    // 为每个 upstream_addr 复用一个已 connect 的 UDP socket：避免每包 bind/connect
    let mut upstream_socks: HashMap<SocketAddr, Arc<UdpSocket>> = HashMap::new();

    let mut upstream_readers: HashMap<SocketAddr, tokio::task::JoinHandle<()>> = HashMap::new();
    for s in &upstream.servers {
        let upstream_addr: SocketAddr = s
            .addr
            .parse()
            .with_context(|| format!("Invalid upstream udp addr: {}", s.addr))?;

        if upstream_readers.contains_key(&upstream_addr) {
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

                        let mut to_send: Vec<SocketAddr> = Vec::new();
                        {
                            let map = sessions2.lock().await;
                            for (client, entry) in map.iter() {
                                if entry.upstream_addr == upstream_addr {
                                    to_send.push(*client);
                                }
                            }
                        }

                        for client in to_send {
                            let _ = listen2.send_to(payload, client).await;
                        }
                    }
                    Err(_) => {}
                }
            }
        });

        upstream_readers.insert(upstream_addr, h);
    }

    let upstream_socks = Arc::new(upstream_socks);

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
                        let mut map = sessions.lock().await;
                        map.retain(|_, v| v.last_seen_ms >= deadline);
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

                                {
                                    let mut map = sessions.lock().await;
                                    map.insert(
                                        client_addr,
                                        UdpSessionEntry {
                                            upstream_addr,
                                            last_seen_ms: now_ms(),
                                        },
                                    );
                                }

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
        let ring = build_ring(servers);
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

fn build_ring(servers: &[StreamUpstreamServer]) -> Vec<(u64, usize)> {
    const VNODES: u32 = 160;

    let mut ring: Vec<(u64, usize)> = Vec::new();
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

    ring.sort_by_key(|(k, _)| *k);
    ring
}

fn is_down(addr: &str) -> bool {
    let now = Instant::now();
    let map = FAIL_MAP.read();
    let Some(st) = map.get(addr) else {
        return false;
    };

    match st.down_until {
        Some(t) => t > now,
        None => false,
    }
}

fn record_upstream_success(addr: &str) {
    let mut map = FAIL_MAP.write();
    map.remove(addr);
}

fn record_upstream_failure(addr: &str, max_fails: i32, fail_timeout: &str) {
    let max_fails = if max_fails <= 0 { 1 } else { max_fails as u32 };
    let ft = parse_duration(fail_timeout).unwrap_or_else(|_| Duration::from_secs(30));

    let mut map = FAIL_MAP.write();
    let entry = map.entry(addr.to_string()).or_insert(FailState {
        fails: 0,
        down_until: None,
    });

    entry.fails = entry.fails.saturating_add(1);

    if entry.fails >= max_fails {
        entry.down_until = Some(Instant::now() + ft);
    }
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
