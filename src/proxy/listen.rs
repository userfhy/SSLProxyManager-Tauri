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

#[cfg(test)]
mod tests {
    use super::parse_listen_addr;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    #[test]
    fn parse_listen_addr_supports_short_port_syntax() {
        let (addr, need_dual_stack) = parse_listen_addr(":8080").unwrap();

        assert_eq!(addr.port(), 8080);
        assert!(need_dual_stack);
        assert!(matches!(
            addr,
            SocketAddr::V6(v6) if *v6.ip() == Ipv6Addr::UNSPECIFIED
        ) || matches!(
            addr,
            SocketAddr::V4(v4) if *v4.ip() == Ipv4Addr::UNSPECIFIED
        ));
    }

    #[test]
    fn parse_listen_addr_trims_whitespace_for_full_socket_addr() {
        let (addr, need_dual_stack) = parse_listen_addr(" 127.0.0.1:9000 ").unwrap();

        assert_eq!(addr, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9000));
        assert!(!need_dual_stack);
    }

    #[test]
    fn parse_listen_addr_keeps_explicit_ipv6_without_dual_stack_flag() {
        let (addr, need_dual_stack) = parse_listen_addr("[::1]:9443").unwrap();

        assert_eq!(addr, SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 9443));
        assert!(!need_dual_stack);
    }

    #[test]
    fn parse_listen_addr_rejects_invalid_input() {
        let err = parse_listen_addr("not-an-addr").unwrap_err().to_string();
        assert!(err.contains("Failed to parse listen_addr"));
    }
}
