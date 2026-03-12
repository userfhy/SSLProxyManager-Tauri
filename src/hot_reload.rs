use crate::config::{self, Config};
use crate::proxy;
use anyhow::{Context, Result};
use std::collections::HashSet;
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
    // 检查全局配置
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

    // 检查 HTTP 规则
    if old.rules != new.rules {
        changes.insert(ConfigChange::HttpRules);
    }

    // 检查 WebSocket 规则
    if old.ws_proxy_enabled != new.ws_proxy_enabled || old.ws_proxy != new.ws_proxy {
        changes.insert(ConfigChange::WsRules);
    }

    // 检查 Stream 规则
    if old.stream.enabled != new.stream.enabled
        || old.stream.upstreams != new.stream.upstreams
        || old.stream.servers != new.stream.servers
    {
        changes.insert(ConfigChange::StreamRules);
    }

    // 检查访问控制
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

    // 如果有多种变化，返回全局重启
    if changes.len() > 1 {
        return ConfigChange::Global;
    }

    // 返回单一变化类型，如果没有变化则返回 BasicOnly
    changes.into_iter().next().unwrap_or(ConfigChange::BasicOnly)
}

/// 优雅重载配置
///
/// 相比直接停止-启动，这个函数会：
/// 1. 检测配置变更类型
/// 2. 只重启受影响的部分
/// 3. 使用更短的等待时间
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
            // 基础配置变化（如 auto_start, show_realtime_logs 等）
            // 不需要重启服务
            config::set_config(new_config.clone());
            config::save_config()?;
            Ok(new_config)
        }
        _ => {
            // 需要重启服务的变更
            let was_running = proxy::is_effectively_running();

            if was_running {
                // 停止服务
                proxy::stop_server(app.clone())
                    .context("停止服务失败")?;

                // 优化：根据变更类型调整等待时间
                let wait_time = match change_type {
                    ConfigChange::HttpRules | ConfigChange::WsRules => {
                        // HTTP/WS 规则变更，等待时间较短
                        std::time::Duration::from_millis(300)
                    }
                    ConfigChange::StreamRules => {
                        // Stream 规则变更，可能需要更长时间释放端口
                        std::time::Duration::from_millis(400)
                    }
                    _ => {
                        // 全局变更，使用标准等待时间
                        std::time::Duration::from_millis(500)
                    }
                };

                tokio::time::sleep(wait_time).await;
            }

            // 更新配置
            config::set_config(new_config.clone());
            config::save_config()
                .context("保存配置文件失败")?;

            // 如果之前在运行，重启服务
            if was_running {
                // 等待一小段时间确保端口完全释放
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                // 尝试启动服务
                match proxy::start_server(app.clone()) {
                    Ok(_) => {
                        tracing::info!("服务重启成功");
                        Ok(new_config)
                    }
                    Err(e) => {
                        // 启动失败，尝试回滚
                        tracing::error!("服务启动失败，尝试回滚: {}", e);

                        // 回滚配置
                        config::set_config(old_config.clone());
                        config::save_config().ok();

                        // 尝试用旧配置启动
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        if let Err(rollback_err) = proxy::start_server(app) {
                            tracing::error!("回滚启动也失败: {}", rollback_err);
                            return Err(anyhow::anyhow!(
                                "新配置启动失败: {}，回滚也失败: {}",
                                e,
                                rollback_err
                            ));
                        }

                        Err(anyhow::anyhow!("新配置启动失败，已回滚到旧配置: {}", e))
                    }
                }
            } else {
                Ok(new_config)
            }
        }
    }
}
