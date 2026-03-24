use crate::config::{self, Config};
use crate::proxy;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// 配置变更类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConfigChange {
    /// 仅基础配置变化（不需要重启服务）
    BasicOnly,
    /// HTTP 规则变化
    HttpRules,
    /// WebSocket 规则变化
    WsRules,
    /// Stream 规则变化
    StreamRules,
    /// 全局配置变化（需要完全重启）
    Global,
}

/// 检测配置变更类型
pub fn detect_config_changes(old: &Config, new: &Config) -> ConfigChange {
    if old.enable_http2 != new.enable_http2
        || old.compression_enabled != new.compression_enabled
        || old.compression_gzip != new.compression_gzip
        || old.compression_brotli != new.compression_brotli
        || old.max_body_size != new.max_body_size
        || old.max_response_body_size != new.max_response_body_size
        || old.upstream_connect_timeout_ms != new.upstream_connect_timeout_ms
        || old.upstream_read_timeout_ms != new.upstream_read_timeout_ms
        || old.upstream_pool_max_idle != new.upstream_pool_max_idle
        || old.upstream_pool_idle_timeout_sec != new.upstream_pool_idle_timeout_sec
    {
        return ConfigChange::Global;
    }

    let mut changes = HashSet::new();

    if old.rules != new.rules {
        changes.insert(ConfigChange::HttpRules);
    }

    if old.ws_proxy_enabled != new.ws_proxy_enabled || old.ws_proxy != new.ws_proxy {
        changes.insert(ConfigChange::WsRules);
    }

    if old.stream.enabled != new.stream.enabled
        || old.stream.upstreams != new.stream.upstreams
        || old.stream.servers != new.stream.servers
    {
        changes.insert(ConfigChange::StreamRules);
    }

    if old.http_access_control_enabled != new.http_access_control_enabled
        || old.ws_access_control_enabled != new.ws_access_control_enabled
        || old.stream_access_control_enabled != new.stream_access_control_enabled
        || old.allow_all_lan != new.allow_all_lan
        || old.allow_all_ip != new.allow_all_ip
        || old.whitelist != new.whitelist
    {
        changes.insert(ConfigChange::HttpRules);
        changes.insert(ConfigChange::WsRules);
        changes.insert(ConfigChange::StreamRules);
    }

    if changes.len() > 1 {
        return ConfigChange::Global;
    }

    changes.into_iter().next().unwrap_or(ConfigChange::BasicOnly)
}

fn config_listen_addrs(cfg: &Config) -> Vec<SocketAddr> {
    let mut addrs = Vec::new();

    for rule in &cfg.rules {
        if !rule.enabled {
            continue;
        }

        let candidates = if !rule.listen_addrs.is_empty() {
            rule.listen_addrs.clone()
        } else {
            vec![rule.listen_addr.clone()]
        };

        for candidate in candidates {
            if let Ok((addr, _)) = proxy::parse_listen_addr(&candidate) {
                addrs.push(addr);
            }
        }
    }

    addrs
}

async fn wait_for_ports_state(addrs: &[SocketAddr], should_be_open: bool, timeout: Duration) -> bool {
    if addrs.is_empty() {
        return true;
    }

    let deadline = Instant::now() + timeout;

    loop {
        let mut all_match = true;
        for addr in addrs {
            let open = tokio::net::TcpStream::connect(addr).await.is_ok();
            if open != should_be_open {
                all_match = false;
                break;
            }
        }

        if all_match {
            return true;
        }

        if Instant::now() >= deadline {
            return false;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 优雅重载配置
///
/// 相比直接停止-启动，这个函数会：
/// 1. 检测配置变更类型
/// 2. 只重启受影响的部分
/// 3. 优先等待端口实际关闭/打开，而不是仅依赖固定 sleep
/// 4. 提供更好的错误处理
pub async fn graceful_reload(
    app: AppHandle,
    new_config: Config,
) -> Result<Config> {
    let old_config = config::get_config();
    let change_type = detect_config_changes(&old_config, &new_config);

    tracing::info!("配置变更类型: {:?}", change_type);

    match change_type {
        ConfigChange::BasicOnly => {
            config::set_config(new_config.clone());
            config::save_config()?;
            Ok(new_config)
        }
        _ => {
            let was_running = proxy::is_effectively_running();
            let old_addrs = config_listen_addrs(&old_config);
            let new_addrs = config_listen_addrs(&new_config);

            if was_running {
                proxy::stop_server(app.clone())
                    .context("停止服务失败")?;

                let stopped = wait_for_ports_state(&old_addrs, false, Duration::from_secs(3)).await;
                if !stopped {
                    tracing::warn!("等待旧监听端口关闭超时，将继续尝试应用新配置");
                }
            }

            config::set_config(new_config.clone());
            config::save_config()
                .context("保存配置文件失败")?;

            if was_running {
                match proxy::start_server(app.clone()) {
                    Ok(_) => {
                        let started = wait_for_ports_state(&new_addrs, true, Duration::from_secs(3)).await;
                        if !started {
                            tracing::warn!("服务启动后等待监听端口就绪超时，但启动调用已返回成功");
                        }
                        tracing::info!("服务重启成功");
                        Ok(new_config)
                    }
                    Err(e) => {
                        tracing::error!("服务启动失败，尝试回滚: {}", e);

                        config::set_config(old_config.clone());
                        config::save_config().ok();

                        let _ = wait_for_ports_state(&new_addrs, false, Duration::from_secs(2)).await;

                        if let Err(rollback_err) = proxy::start_server(app) {
                            tracing::error!("回滚启动也失败: {}", rollback_err);
                            return Err(anyhow::anyhow!(
                                "新配置启动失败: {}，回滚也失败: {}",
                                e,
                                rollback_err
                            ));
                        }

                        let _ = wait_for_ports_state(&old_addrs, true, Duration::from_secs(3)).await;

                        Err(anyhow::anyhow!("新配置启动失败，已回滚到旧配置: {}", e))
                    }
                }
            } else {
                Ok(new_config)
            }
        }
    }
}
