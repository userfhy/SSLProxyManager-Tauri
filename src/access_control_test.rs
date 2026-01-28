// 访问控制模块的单元测试
// 这个文件包含 access_control 模块的所有测试

#[cfg(test)]
mod access_control_tests {
    use crate::access_control;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    use axum::http::HeaderMap;

    #[test]
    fn test_ipv4_mapped_conversion() {
        // 测试 IPv4-mapped IPv6 地址转换
        let ipv6_mapped = "::ffff:192.168.1.128".parse::<IpAddr>().unwrap();
        let converted = access_control::to_ipv4_mapped(&ipv6_mapped);
        
        match converted {
            IpAddr::V4(v4) => {
                assert_eq!(v4, Ipv4Addr::new(192, 168, 1, 128));
                println!("✓ IPv4-mapped conversion: ::ffff:192.168.1.128 -> {}", v4);
            }
            _ => panic!("Expected IPv4 address, got {:?}", converted),
        }
    }

    #[test]
    fn test_ipv6_loopback() {
        // 测试 IPv6 回环地址
        let ipv6_loopback = IpAddr::V6(Ipv6Addr::LOCALHOST);
        assert!(access_control::is_loopback_ip(&ipv6_loopback));
        println!("✓ IPv6 loopback (::1) is recognized");
    }

    #[test]
    fn test_ipv6_unique_local() {
        // 测试 IPv6 唯一本地地址 (fc00::/7)
        let ipv6_ula = "fc00::1".parse::<IpAddr>().unwrap();
        assert!(access_control::is_lan_ip(&ipv6_ula));
        println!("✓ IPv6 unique local address is recognized as LAN");
    }

    #[test]
    fn test_ipv6_link_local() {
        // 测试 IPv6 链路本地地址 (fe80::/10)
        let ipv6_link_local = "fe80::1".parse::<IpAddr>().unwrap();
        assert!(access_control::is_lan_ip(&ipv6_link_local));
        println!("✓ IPv6 link-local address is recognized as LAN");
    }

    #[test]
    fn test_ipv4_mapped_lan_detection() {
        // 测试 IPv4-mapped IPv6 地址的局域网检测
        let ipv6_mapped = "::ffff:192.168.1.128".parse::<IpAddr>().unwrap();
        assert!(access_control::is_lan_ip(&ipv6_mapped));
        println!("✓ IPv4-mapped IPv6 address (::ffff:192.168.1.128) is recognized as LAN");
    }

    #[test]
    fn test_ip_to_string() {
        // 测试 IP 地址字符串转换
        let ipv6_mapped = "::ffff:192.168.1.128".parse::<IpAddr>().unwrap();
        let result = access_control::ip_to_string(&ipv6_mapped);
        assert_eq!(result, "192.168.1.128");
        println!("✓ ip_to_string converts ::ffff:192.168.1.128 to 192.168.1.128");
    }

    #[test]
    fn test_is_allowed_fast_ipv6_loopback() {
        // 测试 IPv6 回环地址访问控制
        let remote = SocketAddr::new(Ipv6Addr::LOCALHOST.into(), 8080);
        let headers = HeaderMap::new();
        let whitelist = vec![];
        
        let allowed = access_control::is_allowed_fast(&remote, &headers, false, &whitelist);
        assert!(allowed, "IPv6 loopback should be allowed even without allow_all_lan");
        println!("✓ IPv6 loopback (::1) is allowed");
    }

    #[test]
    fn test_is_allowed_fast_ipv4_mapped_with_allow_all_lan() {
        // 测试 IPv4-mapped IPv6 地址在 allow_all_lan=true 时的访问控制
        let ipv6_mapped = "::ffff:192.168.1.128".parse::<Ipv6Addr>().unwrap();
        let remote = SocketAddr::new(ipv6_mapped.into(), 8080);
        let headers = HeaderMap::new();
        let whitelist = vec![];
        
        let allowed = access_control::is_allowed_fast(&remote, &headers, true, &whitelist);
        assert!(allowed, "IPv4-mapped IPv6 LAN address should be allowed with allow_all_lan=true");
        println!("✓ IPv4-mapped IPv6 LAN address (::ffff:192.168.1.128) is allowed with allow_all_lan=true");
    }

    #[test]
    fn test_is_allowed_fast_ipv6_unique_local_with_allow_all_lan() {
        // 测试 IPv6 唯一本地地址在 allow_all_lan=true 时的访问控制
        let ipv6_ula = "fc00::1".parse::<Ipv6Addr>().unwrap();
        let remote = SocketAddr::new(ipv6_ula.into(), 8080);
        let headers = HeaderMap::new();
        let whitelist = vec![];
        
        let allowed = access_control::is_allowed_fast(&remote, &headers, true, &whitelist);
        assert!(allowed, "IPv6 unique local address should be allowed with allow_all_lan=true");
        println!("✓ IPv6 unique local address (fc00::1) is allowed with allow_all_lan=true");
    }
}
