use anyhow::{Context, Result};
use std::net::SocketAddr;

use crate::config;

pub fn parse_listen_addr(s: &str) -> Result<(SocketAddr, bool)> {
    let trimmed = s.trim();
    let (normalized, need_dual_stack) = if trimmed.starts_with(':') {
        let port = trimmed;
        let ipv6_format = format!("[::]{}", port);
        let ipv4_format = format!("0.0.0.0{}", port);

        if let Ok(addr) = ipv6_format.parse::<SocketAddr>() {
            (addr, true)
        } else if let Ok(addr) = ipv4_format.parse::<SocketAddr>() {
            (addr, true)
        } else {
            return Err(anyhow::anyhow!("Failed to parse listen_addr: {s}"));
        }
    } else {
        let addr = trimmed
            .parse::<SocketAddr>()
            .with_context(|| format!("Failed to parse listen_addr: {s}"))?;
        (addr, false)
    };

    Ok((normalized, need_dual_stack))
}

pub async fn precheck_rule(rule: &config::ListenRule, listen_addr: &str) -> Result<()> {
    let (addr, _need_dual_stack) = parse_listen_addr(listen_addr)?;

    if rule.ssl_enable {
        let _ = axum_server::tls_rustls::RustlsConfig::from_pem_file(
            rule.cert_file.clone(),
            rule.key_file.clone(),
        )
        .await
        .with_context(|| "Failed to load TLS certificate/private key")?;

        let listener = tokio::net::TcpListener::bind(addr).await?;
        drop(listener);
    } else {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        drop(listener);
    }

    Ok(())
}
