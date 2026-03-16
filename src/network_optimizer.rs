// 网络优化模块
// 提供 TCP 参数优化、连接池预热等功能

use anyhow::Result;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use tracing::{debug, info};

/// TCP 连接优化配置
#[derive(Debug, Clone)]
pub struct TcpOptimizer {
    /// 禁用 Nagle 算法（降低延迟）
    pub nodelay: bool,
    /// 允许地址重用
    pub reuse_address: bool,
    /// 允许端口重用（Linux）
    #[cfg(all(unix, not(target_os = "solaris"), not(target_os = "illumos")))]
    pub reuse_port: bool,
    /// 接收缓冲区大小（字节）
    pub recv_buffer_size: Option<usize>,
    /// 发送缓冲区大小（字节）
    pub send_buffer_size: Option<usize>,
    /// TCP keepalive 间隔（秒）
    pub keepalive_interval: Option<u64>,
}

impl Default for TcpOptimizer {
    fn default() -> Self {
        Self {
            nodelay: true,
            reuse_address: true,
            #[cfg(all(unix, not(target_os = "solaris"), not(target_os = "illumos")))]
            reuse_port: true,
            recv_buffer_size: Some(256 * 1024), // 256KB
            send_buffer_size: Some(256 * 1024), // 256KB
            keepalive_interval: Some(60),       // 60 秒
        }
    }
}

impl TcpOptimizer {
    /// 创建优化的 TCP socket
    pub fn create_optimized_socket(&self, addr: &SocketAddr) -> Result<Socket> {
        let domain = if addr.is_ipv4() {
            Domain::IPV4
        } else {
            Domain::IPV6
        };

        let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;

        // 应用优化配置
        self.apply_optimizations(&socket)?;

        Ok(socket)
    }

    /// 应用 TCP 优化到现有 socket
    pub fn apply_optimizations(&self, socket: &Socket) -> Result<()> {
        // 禁用 Nagle 算法，降低延迟
        if self.nodelay {
            socket.set_tcp_nodelay(true)?;
            debug!("TCP_NODELAY enabled");
        }

        // 允许地址重用
        if self.reuse_address {
            socket.set_reuse_address(true)?;
            debug!("SO_REUSEADDR enabled");
        }

        // 允许端口重用（Linux）
        // 注意：socket2 0.5 版本不支持 set_reuse_port，需要手动设置
        #[cfg(all(unix, not(target_os = "solaris"), not(target_os = "illumos")))]
        if self.reuse_port {
            // SO_REUSEPORT 支持需要较新的内核版本
            // 如果设置失败，只记录警告，不中断流程
            debug!("SO_REUSEPORT requested (requires manual setup or newer socket2 version)");
        }

        // 设置接收缓冲区大小
        if let Some(size) = self.recv_buffer_size {
            socket.set_recv_buffer_size(size)?;
            debug!("Receive buffer size set to {} bytes", size);
        }

        // 设置发送缓冲区大小
        if let Some(size) = self.send_buffer_size {
            socket.set_send_buffer_size(size)?;
            debug!("Send buffer size set to {} bytes", size);
        }

        // 设置 TCP keepalive
        if let Some(interval) = self.keepalive_interval {
            let keepalive = socket2::TcpKeepalive::new()
                .with_time(std::time::Duration::from_secs(interval));
            socket.set_tcp_keepalive(&keepalive)?;
            debug!("TCP keepalive set to {} seconds", interval);
        }

        Ok(())
    }

    /// 优化 tokio TcpListener
    pub async fn optimize_listener(
        &self,
        addr: SocketAddr,
    ) -> Result<tokio::net::TcpListener> {
        let socket = self.create_optimized_socket(&addr)?;
        socket.bind(&addr.into())?;
        socket.listen(1024)?; // backlog = 1024

        // 转换为 tokio TcpListener
        let std_listener: std::net::TcpListener = socket.into();
        std_listener.set_nonblocking(true)?;
        let listener = tokio::net::TcpListener::from_std(std_listener)?;

        info!("Optimized TCP listener created on {}", addr);
        Ok(listener)
    }
}

/// HTTP 客户端优化配置
///
/// 注意：这是一个可选的优化工具，当前项目已在 proxy.rs 中手动配置了相同的优化。
/// 如果需要快速创建优化的客户端，可以使用此工具。
#[allow(dead_code)]
pub struct HttpClientOptimizer;

#[allow(dead_code)]
impl HttpClientOptimizer {
    /// 创建优化的 reqwest 客户端
    pub fn create_optimized_client() -> Result<reqwest::Client> {
        let client = reqwest::Client::builder()
            // HTTP/2 优化
            .http2_prior_knowledge()
            .http2_keep_alive_interval(Some(std::time::Duration::from_secs(10)))
            .http2_keep_alive_timeout(std::time::Duration::from_secs(20))
            .http2_adaptive_window(true)
            // 连接池优化
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Some(std::time::Duration::from_secs(90)))
            // 超时配置
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            // TCP 优化
            .tcp_nodelay(true)
            .tcp_keepalive(Some(std::time::Duration::from_secs(60)))
            // 其他优化
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;

        info!("Optimized HTTP client created");
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_optimizer_default() {
        let optimizer = TcpOptimizer::default();
        assert!(optimizer.nodelay);
        assert!(optimizer.reuse_address);
        assert_eq!(optimizer.recv_buffer_size, Some(256 * 1024));
    }

    #[tokio::test]
    async fn test_create_optimized_listener() {
        let optimizer = TcpOptimizer::default();
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let result = optimizer.optimize_listener(addr).await;
        assert!(result.is_ok());
    }
}
