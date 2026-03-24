use anyhow::{anyhow, Context, Result};
use axum::{routing::any, Router};
use reqwest::redirect::Policy;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tower_http::compression::{CompressionLayer, CompressionLevel};
use tracing::info;

use crate::{config, rate_limit};
use crate::proxy::{healthz, proxy_handler, AppState};
use crate::proxy::listen::parse_listen_addr;
use crate::proxy::logging::send_log;

fn build_upstream_clients(cfg: &config::Config) -> Result<(reqwest::Client, reqwest::Client)> {
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

        if cfg.enable_http2 {
            builder = builder
                .http2_keep_alive_interval(Duration::from_secs(10))
                .http2_keep_alive_timeout(Duration::from_secs(20))
                .http2_adaptive_window(true)
                .http2_max_frame_size(Some(16384 * 4));
        } else {
            builder = builder.http1_only();
        }

        builder.connection_verbose(false)
    };

    let client_follow = client_builder()
        .build()
        .context("Failed to create upstream HTTP client")?;

    let client_nofollow = client_builder()
        .redirect(Policy::none())
        .build()
        .context("Failed to create upstream HTTP client")?;

    Ok((client_follow, client_nofollow))
}

fn build_app_state(
    app: &tauri::AppHandle,
    rule: &config::ListenRule,
    listen_addr: &str,
    server_port: u16,
    cfg: config::Config,
    client_follow: reqwest::Client,
    client_nofollow: reqwest::Client,
) -> AppState {
    AppState {
        rule: rule.clone(),
        client_follow,
        client_nofollow,
        app: app.clone(),
        listen_addr: Arc::from(listen_addr.to_string()),
        server_port,
        stream_proxy: cfg.stream_proxy,
        max_body_size: cfg.max_body_size,
        max_response_body_size: cfg.max_response_body_size,
        http_access_control_enabled: cfg.http_access_control_enabled,
        allow_all_lan: cfg.allow_all_lan,
        allow_all_ip: cfg.allow_all_ip,
        whitelist: Arc::from(cfg.whitelist),
    }
}

pub async fn start_rule_server(
    app: tauri::AppHandle,
    rule: config::ListenRule,
    listen_addr: String,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let (addr, need_dual_stack) = parse_listen_addr(&listen_addr)?;
    let server_port = addr.port();

    let cfg = crate::config::get_config();
    let (client_follow, client_nofollow) = build_upstream_clients(&cfg)?;

    let state = build_app_state(
        &app,
        &rule,
        &listen_addr,
        server_port,
        cfg.clone(),
        client_follow,
        client_nofollow,
    );

    if let Some(enabled) = rule.rate_limit_enabled {
        if enabled {
            let rate_limit_config = rate_limit::RateLimitConfig {
                enabled: true,
                requests_per_second: rule.rate_limit_requests_per_second.unwrap_or(10),
                burst_size: rule.rate_limit_burst_size.unwrap_or(20),
                ban_seconds: rule.rate_limit_ban_seconds.unwrap_or(0),
            };
            rate_limit::get_rate_limiter(&listen_addr, rate_limit_config);
        }
    }

    let router = Router::new().route("/healthz", any(healthz));
    let mut app_router = router.fallback(any(proxy_handler)).with_state(state);

    if cfg.compression_enabled {
        let mut compression_layer = CompressionLayer::new();

        if cfg.compression_gzip {
            let gzip_level = cfg.compression_gzip_level.clamp(1, 9) as i32;
            compression_layer = compression_layer.gzip(true).quality(CompressionLevel::Precise(gzip_level));
        }

        if cfg.compression_brotli {
            let brotli_level = cfg.compression_brotli_level.clamp(0, 11) as i32;
            compression_layer = compression_layer.br(true).quality(CompressionLevel::Precise(brotli_level));
        }

        app_router = app_router.layer(compression_layer);
    }

    let app_router = app_router.into_make_service_with_connect_info::<SocketAddr>();

    let routes_summary = rule
        .routes
        .iter()
        .map(|rt| format!("{} -> {} upstreams", rt.path.as_deref().unwrap_or("/"), rt.upstreams.len()))
        .collect::<Vec<_>>()
        .join(", ");

    send_log(format!("[HTTP] Listening address: {} -> {}", listen_addr, addr));
    info!("[HTTP] Listening address: {} -> {}", listen_addr, addr);

    if rule.ssl_enable {
        let tls_cfg = axum_server::tls_rustls::RustlsConfig::from_pem_file(
            rule.cert_file.clone(),
            rule.key_file.clone(),
        )
        .await
        .with_context(|| "Failed to load TLS certificate/private key")?;

        send_log(format!("[HTTP] HTTPS enabled: {}", addr));

        if need_dual_stack && addr.is_ipv6() {
            send_log(format!("[HTTP] Listening on IPv6 (dual-stack): {} (supports both IPv4 and IPv6)", addr));
            info!("[HTTP] Listening on IPv6 (dual-stack): {} (supports both IPv4 and IPv6)", addr);
        }

        send_log(format!(
            "[HTTP NODE {}] Server started | SSL: {} | Routes: [{}] | Allow all LAN: {}",
            listen_addr, rule.ssl_enable, routes_summary, cfg.allow_all_lan
        ));

        let ax_handle = axum_server::Handle::new();
        let ax_shutdown_handle = ax_handle.clone();
        tauri::async_runtime::spawn(async move {
            let _ = shutdown_rx.await;
            info!("Shutdown signal received, HTTPS service {} is stopping", addr);
            ax_shutdown_handle.graceful_shutdown(Some(Duration::from_secs(5)));
        });

        axum_server::bind_rustls(addr, tls_cfg)
            .handle(ax_handle)
            .serve(app_router)
            .await
            .map_err(|e| anyhow!("HTTPS service failed: {e}"))?;
    } else {
        send_log(format!("[HTTP] HTTP enabled: {}", addr));

        if need_dual_stack && addr.is_ipv6() {
            send_log(format!("[HTTP] Listening on IPv6 (dual-stack): {} (supports both IPv4 and IPv6)", addr));
            info!("[HTTP] Listening on IPv6 (dual-stack): {} (supports both IPv4 and IPv6)", addr);
        }

        send_log(format!(
            "[HTTP NODE {}] Server started | SSL: {} | Routes: [{}] | Allow all LAN: {}",
            listen_addr, rule.ssl_enable, routes_summary, cfg.allow_all_lan
        ));

        use crate::network_optimizer::TcpOptimizer;
        let optimizer = TcpOptimizer::default();
        let listener = optimizer.optimize_listener(addr).await?;

        axum::serve(listener, app_router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
                info!("Shutdown signal received, HTTP service {} is stopping", addr);
            })
            .await
            .map_err(|e| anyhow!("HTTP service failed: {e}"))?;
    }

    Ok(())
}
