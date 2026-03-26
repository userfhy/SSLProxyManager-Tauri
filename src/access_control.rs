use axum::http::HeaderMap;
use std::net::{IpAddr, SocketAddr};
use tracing::debug;

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
                // info!("to_ipv4_mapped: converted {} to {}", ip, result);
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

pub fn is_allowed_fast(
    remote: &SocketAddr,
    headers: &HeaderMap,
    allow_all_lan: bool,
    allow_all_ip: bool,
    whitelist: &[config::WhitelistEntry],
) -> bool {
    let ip_str = client_ip_from_headers(remote, headers);
    if metrics::is_ip_blacklisted(&ip_str) {
        debug!("IP {} is blacklisted", ip_str);
        return false;
    }

    let ip = to_ipv4_mapped(&remote.ip());
    let allowed = is_allowed_remote_ip(remote, allow_all_lan, allow_all_ip, whitelist);
    let is_lan = is_lan_ip(&ip);
    debug!("Access control: IP={}, ip_str={}, is_lan={}, allow_all_lan={}, allow_all_ip={}, final_allowed={}", 
           ip, ip_str, is_lan, allow_all_lan, allow_all_ip, allowed);

    allowed
}

/// 快速路径：仅基于 remote socket IP 做访问控制（不读取 headers，不做黑名单检查）。
/// 适合调用方已完成黑名单判定且已经拿到 client_ip 的场景，避免重复解析与查询。
pub fn is_allowed_remote_ip(
    remote: &SocketAddr,
    allow_all_lan: bool,
    allow_all_ip: bool,
    whitelist: &[config::WhitelistEntry],
) -> bool {
    let ip = to_ipv4_mapped(&remote.ip());

    if allow_all_ip {
        return true;
    }

    if is_loopback_ip(&ip) {
        return true;
    }

    if whitelist.iter().any(|e| {
        parse_ip(&e.ip)
            .map(|w| to_ipv4_mapped(&w) == ip)
            .unwrap_or(false)
    }) {
        return true;
    }

    allow_all_lan && is_lan_ip(&ip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn ipv4_mapped_conversion() {
        let ipv6_mapped = "::ffff:192.168.1.128".parse::<IpAddr>().unwrap();
        let converted = to_ipv4_mapped(&ipv6_mapped);

        match converted {
            IpAddr::V4(v4) => {
                assert_eq!(v4, Ipv4Addr::new(192, 168, 1, 128));
            }
            _ => panic!("Expected IPv4 address, got {:?}", converted),
        }
    }

    #[test]
    fn ipv6_loopback_detected() {
        let ipv6_loopback = IpAddr::V6(Ipv6Addr::LOCALHOST);
        assert!(is_loopback_ip(&ipv6_loopback));
    }

    #[test]
    fn ipv6_unique_local_as_lan() {
        let ipv6_ula = "fc00::1".parse::<IpAddr>().unwrap();
        assert!(is_lan_ip(&ipv6_ula));
    }

    #[test]
    fn ipv6_link_local_as_lan() {
        let ipv6_link_local = "fe80::1".parse::<IpAddr>().unwrap();
        assert!(is_lan_ip(&ipv6_link_local));
    }

    #[test]
    fn ipv4_mapped_lan_detection() {
        let ipv6_mapped = "::ffff:192.168.1.128".parse::<IpAddr>().unwrap();
        assert!(is_lan_ip(&ipv6_mapped));
    }

    #[test]
    fn ip_to_string_converts_mapped() {
        let ipv6_mapped = "::ffff:192.168.1.128".parse::<IpAddr>().unwrap();
        let result = ip_to_string(&ipv6_mapped);
        assert_eq!(result, "192.168.1.128");
    }

    #[test]
    fn is_allowed_fast_ipv6_loopback() {
        let remote = SocketAddr::new(Ipv6Addr::LOCALHOST.into(), 8080);
        let headers = HeaderMap::new();
        let whitelist = vec![];

        let allowed = is_allowed_fast(&remote, &headers, false, false, &whitelist);
        assert!(allowed, "IPv6 loopback should be allowed even without allow_all_lan");
    }

    #[test]
    fn is_allowed_fast_ipv4_mapped_with_allow_all_lan() {
        let ipv6_mapped = "::ffff:192.168.1.128".parse::<Ipv6Addr>().unwrap();
        let remote = SocketAddr::new(ipv6_mapped.into(), 8080);
        let headers = HeaderMap::new();
        let whitelist = vec![];

        let allowed = is_allowed_fast(&remote, &headers, true, false, &whitelist);
        assert!(
            allowed,
            "IPv4-mapped IPv6 LAN address should be allowed with allow_all_lan=true"
        );
    }

    #[test]
    fn is_allowed_fast_ipv6_unique_local_with_allow_all_lan() {
        let ipv6_ula = "fc00::1".parse::<Ipv6Addr>().unwrap();
        let remote = SocketAddr::new(ipv6_ula.into(), 8080);
        let headers = HeaderMap::new();
        let whitelist = vec![];

        let allowed = is_allowed_fast(&remote, &headers, true, false, &whitelist);
        assert!(allowed, "IPv6 unique local address should be allowed with allow_all_lan=true");
    }
}
