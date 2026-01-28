use axum::http::HeaderMap;
use std::net::{IpAddr, SocketAddr};
use tracing::{debug, info};

use crate::{config, metrics};

fn parse_ip(s: &str) -> Option<IpAddr> {
    s.trim().parse::<IpAddr>().ok()
}

/// 将 IPv4-mapped IPv6 地址转换为 IPv4 地址
/// 例如：::ffff:192.168.1.128 -> 192.168.1.128
#[cfg_attr(test, allow(dead_code))]
pub(crate) fn to_ipv4_mapped(ip: &IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => {
            // 使用 Rust 标准库的方法检查并转换 IPv4-mapped IPv6 地址
            if let Some(v4) = v6.to_ipv4_mapped() {
                let result = IpAddr::V4(v4);
                info!("to_ipv4_mapped: converted {} to {}", ip, result);
                result
            } else {
                *ip
            }
        }
        IpAddr::V4(_) => *ip,
    }
}

/// 将 IP 地址转换为字符串，IPv4-mapped IPv6 地址会转换为 IPv4 格式
pub fn ip_to_string(ip: &IpAddr) -> String {
    let ipv4_mapped = to_ipv4_mapped(ip);
    let result = ipv4_mapped.to_string();
    debug!("ip_to_string: {} -> {}", ip, result);
    result
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn is_loopback_ip(ip: &IpAddr) -> bool {
    let ip = to_ipv4_mapped(ip);
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn is_lan_ip(ip: &IpAddr) -> bool {
    let ip = to_ipv4_mapped(ip);
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


pub fn client_ip_from_headers(remote: &SocketAddr, headers: &HeaderMap) -> String {
    if let Some(h) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(parse_ip)
    {
        return ip_to_string(&h);
    }
    if let Some(h) = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(parse_ip)
    {
        return ip_to_string(&h);
    }

    ip_to_string(&remote.ip())
}


pub fn is_allowed_fast(remote: &SocketAddr, headers: &HeaderMap, allow_all_lan: bool, whitelist: &[config::WhitelistEntry]) -> bool {
    let ip_str = client_ip_from_headers(remote, headers);
    if metrics::is_ip_blacklisted(&ip_str) {
        debug!("IP {} is blacklisted", ip_str);
        return false;
    }

    // 直接使用 remote.ip() 并转换为 IPv4-mapped 格式，确保正确处理 IPv4-mapped IPv6 地址
    // 这是最可靠的方式，因为 remote.ip() 直接来自 socket
    let remote_ip_raw = remote.ip();
    let ip = to_ipv4_mapped(&remote_ip_raw);
    
    info!("Access control check: remote_ip_raw={}, converted_ip={}, ip_str={}, allow_all_lan={}", 
          remote_ip_raw, ip, ip_str, allow_all_lan);

    // 本机回环地址（127.0.0.1 / ::1）永远允许，不需要加入白名单
    if is_loopback_ip(&ip) {
        debug!("IP {} is loopback, allowed", ip);
        return true;
    }

    // 检查白名单：需要同时支持 IPv4 和 IPv4-mapped IPv6 格式
    if whitelist
        .iter()
        .any(|e| {
            if let Some(whitelist_ip) = parse_ip(&e.ip) {
                let whitelist_ip = to_ipv4_mapped(&whitelist_ip);
                whitelist_ip == ip
            } else {
                false
            }
        })
    {
        debug!("IP {} is in whitelist, allowed", ip);
        return true;
    }

    let is_lan = is_lan_ip(&ip);
    let allowed = allow_all_lan && is_lan;
    info!("IP {} is_lan={}, allow_all_lan={}, final_allowed={}", ip, is_lan, allow_all_lan, allowed);
    
    allowed
}

