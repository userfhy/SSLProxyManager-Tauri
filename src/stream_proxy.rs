use anyhow::{anyhow, Context, Result};
use parking_lot::RwLock;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, Mutex};
use tokio::time;

use crate::config::{StreamProxyConfig, StreamServer, StreamUpstream, StreamUpstreamServer};

static STREAM_SERVERS: once_cell::sync::Lazy<RwLock<Vec<StreamServerHandle>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Vec::new()));

struct StreamServerHandle {
    task: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: mpsc::Sender<()>,
}

pub async fn start_stream_servers(config: &StreamProxyConfig) -> Result<()> {
    stop_stream_servers().await;

    if !config.enabled {
        return Ok(());
    }

    let mut servers = Vec::new();

    for server in &config.servers {
        if !server.enabled {
            continue;
        }

        let upstream = config
            .upstreams
            .iter()
            .find(|u| u.name == server.proxy_pass);

        let Some(upstream) = upstream else {
            tracing::error!(
                "Stream server on port {}: upstream '{}' not found",
                server.listen_port,
                server.proxy_pass
            );
            continue;
        };

        let connect_timeout = parse_duration(&server.proxy_connect_timeout)
            .unwrap_or_else(|_| Duration::from_secs(300));
        let proxy_timeout =
            parse_duration(&server.proxy_timeout).unwrap_or_else(|_| Duration::from_secs(600));

        if server.udp {
            start_udp_server(server, upstream, connect_timeout, proxy_timeout, &mut servers).await?;
        } else {
            start_tcp_server(server, upstream, connect_timeout, proxy_timeout, &mut servers).await?;
        }
    }

    *STREAM_SERVERS.write() = servers;
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
        .with_context(|| format!("Failed to bind to {}", listen_addr))?;

    tracing::info!("Stream TCP server listening on {}", listen_addr);

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
    let server_task = tokio::spawn({
        let upstream = upstream.clone();
        async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((client_socket, client_addr)) => {
                                let upstream = upstream.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_tcp_client(client_socket, client_addr, &upstream, connect_timeout, proxy_timeout).await {
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
    mut client_socket: TcpStream,
    client_addr: SocketAddr,
    upstream: &StreamUpstream,
    connect_timeout: Duration,
    proxy_timeout: Duration,
) -> Result<()> {
    let server = select_upstream_server(upstream, &client_addr);
    let server_addr = &server.addr;

    let mut server_socket: TcpStream =
        match time::timeout(connect_timeout, TcpStream::connect(server_addr)).await {
            Ok(Ok(socket)) => socket,
            Ok(Err(e)) => {
                return Err(anyhow!("Failed to connect to upstream {}: {}", server_addr, e));
            }
            Err(_) => {
                return Err(anyhow!(
                    "Connection to upstream {} timed out after {:?}",
                    server_addr,
                    connect_timeout
                ));
            }
        };

    let (mut client_reader, mut client_writer) = client_socket.split();
    let (mut server_reader, mut server_writer) = server_socket.split();

    let c_to_u = async {
        io::copy(&mut client_reader, &mut server_writer).await?;
        let _ = server_writer;
        Ok::<_, std::io::Error>(())
    };

    let u_to_c = async {
        io::copy(&mut server_reader, &mut client_writer).await?;
        let _ = client_writer;
        Ok::<_, std::io::Error>(())
    };

    match time::timeout(
        proxy_timeout,
        async { tokio::select! { r = c_to_u => r, r = u_to_c => r } },
    )
    .await
    {
        Ok(res) => {
            if let Err(e) = res {
                tracing::debug!(
                    "TCP stream ended with io error (client={} upstream={}): {}",
                    client_addr,
                    server_addr,
                    e
                );
            }
        }
        Err(_) => {
            tracing::debug!(
                "TCP stream timeout (client={} upstream={} timeout={:?})",
                client_addr,
                server_addr,
                proxy_timeout
            );
        }
    }

    Ok(())
}

// --- UDP session proxy ---

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
    let listen_addr = format!("0.0.0.0:{}", server.listen_port);
    let listen_sock = UdpSocket::bind(&listen_addr)
        .await
        .with_context(|| format!("Failed to bind to {}", listen_addr))?;

    tracing::info!("Stream UDP server listening on {}", listen_addr);

    let sessions: Arc<Mutex<HashMap<SocketAddr, UdpSessionEntry>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let listen_sock = Arc::new(listen_sock);

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

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
    let server_task = tokio::spawn({
        let upstream = upstream.clone();
        let sessions = sessions.clone();
        let listen = listen_sock.clone();

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
                                let up_server = select_upstream_server(&upstream, &client_addr);
                                let upstream_addr: SocketAddr = match up_server.addr.parse() {
                                    Ok(a) => a,
                                    Err(_) => continue,
                                };

                                {
                                    let mut map = sessions.lock().await;
                                    map.insert(client_addr, UdpSessionEntry { upstream_addr, last_seen_ms: now_ms() });
                                }

                                if let Ok(s) = UdpSocket::bind("0.0.0.0:0").await {
                                    if s.connect(upstream_addr).await.is_ok() {
                                        let _ = s.send(&buf[..n]).await;
                                    }
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

fn select_upstream_server<'a>(
    upstream: &'a StreamUpstream,
    client_addr: &SocketAddr,
) -> &'a StreamUpstreamServer {
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
