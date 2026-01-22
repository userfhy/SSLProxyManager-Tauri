use axum::http::HeaderMap;
use std::net::{IpAddr, SocketAddr};

use crate::{config, metrics};

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

pub fn client_ip_from_headers(remote: &SocketAddr, headers: &HeaderMap) -> String {
    if let Some(h) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(parse_ip)
    {
        return h.to_string();
    }
    if let Some(h) = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(parse_ip)
    {
        return h.to_string();
    }

    remote.ip().to_string()
}

pub fn is_allowed(remote: &SocketAddr, headers: &HeaderMap, cfg: &config::Config) -> bool {
    // 黑名单优先
    let ip_str = client_ip_from_headers(remote, headers);
    if metrics::is_ip_blacklisted(&ip_str) {
        return false;
    }

    let ip = parse_ip(&ip_str).unwrap_or(remote.ip());

    if is_ip_whitelisted(&ip, cfg) {
        return true;
    }

    cfg.allow_all_lan && is_lan_ip(&ip)
}

pub fn is_allowed_fast(remote: &SocketAddr, headers: &HeaderMap, allow_all_lan: bool, whitelist: &[config::WhitelistEntry]) -> bool {
    let ip_str = client_ip_from_headers(remote, headers);
    if metrics::is_ip_blacklisted(&ip_str) {
        return false;
    }

    let ip = parse_ip(&ip_str).unwrap_or(remote.ip());

    if whitelist
        .iter()
        .any(|e| parse_ip(&e.ip).as_ref() == Some(&ip))
    {
        return true;
    }

    allow_all_lan && is_lan_ip(&ip)
}
