use crate::config;
use crate::i18n;
use crate::metrics;
use crate::proxy;
use crate::stream_proxy;
use crate::tray;
use crate::update;
use crate::cache_optimizer;
use crate::test_tools;
use anyhow::Result;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use std::path::PathBuf;

/// 在保存配置前做完整性校验，确保不会写入无法启动的配置到磁盘。
/// 只做无副作用的检查（读文件、解析格式），不绑定端口、不修改任何状态。
async fn validate_config(cfg: &config::Config) -> Result<(), String> {
    // 1. HTTP 监听规则：SSL 启用时验证证书可正常加载
    for rule in &cfg.rules {
        if !rule.enabled || !rule.ssl_enable {
            continue;
        }
        if rule.cert_file.trim().is_empty() || rule.key_file.trim().is_empty() {
            return Err(format!(
                "监听规则 ({}) SSL 已启用，但证书或私钥文件路径为空",
                rule.listen_addr
            ));
        }
        axum_server::tls_rustls::RustlsConfig::from_pem_file(
            rule.cert_file.clone(),
            rule.key_file.clone(),
        )
        .await
        .map_err(|e| {
            format!(
                "监听规则 ({}) TLS 证书加载失败: {e}",
                rule.listen_addr
            )
        })?;
    }

    // 2. WS 代理规则：SSL 启用时验证证书可正常加载
    if let Some(ws_rules) = &cfg.ws_proxy {
        for rule in ws_rules {
            if !rule.enabled || !rule.ssl_enable {
                continue;
            }
            if rule.cert_file.trim().is_empty() || rule.key_file.trim().is_empty() {
                return Err(format!(
                    "WS 规则 ({}) SSL 已启用，但证书或私钥文件路径为空",
                    rule.listen_addr
                ));
            }
            axum_server::tls_rustls::RustlsConfig::from_pem_file(
                rule.cert_file.clone(),
                rule.key_file.clone(),
            )
            .await
            .map_err(|e| {
                format!(
                    "WS 规则 ({}) TLS 证书加载失败: {e}",
                    rule.listen_addr
                )
            })?;
        }
    }

    // 3. Stream 代理：验证端口不重复、上游引用合法等
    if cfg.stream.enabled {
        stream_proxy::validate_stream_config(&cfg.stream).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_config() -> Result<config::Config, String> {
    Ok(config::get_config())
}


#[tauri::command]
pub async fn save_config(
    app: tauri::AppHandle,
    mut cfg: config::Config,
) -> Result<config::Config, String> {
    // 0. 预验证：在停服、写盘之前完成所有可检查的合法性校验。
    //    若验证失败，直接返回错误，当前运行的服务不受任何影响。
    config::ensure_config_ids_for_save(&mut cfg);
    validate_config(&cfg).await?;

    // 更新数据库配置（如果需要）
    if let Some(metrics_storage) = cfg.metrics_storage.as_ref() {
        if metrics_storage.enabled {
            metrics::init_db(metrics_storage.db_path.clone())
                .await
                .map_err(|e| e.to_string())?;
            metrics::init_request_log_writer().await;
        }
    }

    // 使用优雅重载机制
    crate::hot_reload::graceful_reload(app, cfg)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_version() -> Result<String, String> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
pub async fn check_update() -> Result<update::CheckResult, String> {
    let cfg = config::get_config();
    if let Some(update_cfg) = cfg.update.as_ref() {
        update::check_for_updates(env!("CARGO_PKG_VERSION"), update_cfg.clone())
            .await
            .map_err(|e| e.to_string())
    } else {
        Ok(update::CheckResult {
            has_update: false,
            is_prerelease: false,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            update_info: None,
            error: Some("更新检查未启用".to_string()),
        })
    }
}


#[tauri::command]
pub fn start_server(app: tauri::AppHandle) -> Result<(), String> {
    proxy::start_server(app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_server(app: tauri::AppHandle) -> Result<(), String> {
    proxy::stop_server(app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_status() -> Result<String, String> {
    Ok(if proxy::is_running() {
        "running".to_string()
    } else {
        "stopped".to_string()
    })
}

#[tauri::command]
pub fn get_logs() -> Result<Vec<String>, String> {
    Ok(proxy::get_logs())
}

#[tauri::command]
pub fn clear_logs() -> Result<(), String> {
    proxy::clear_logs();
    Ok(())
}

#[tauri::command]
pub fn set_tray_proxy_state(_app: tauri::AppHandle, running: bool) -> Result<(), String> {
    tray::set_tray_proxy_state(running);
    Ok(())
}

#[tauri::command]
pub fn get_metrics() -> Result<metrics::MetricsPayload, String> {
    Ok(metrics::get_metrics())
}

#[tauri::command]
pub async fn get_listen_addrs() -> Result<Vec<String>, String> {
    metrics::get_distinct_listen_addrs()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_historical_metrics(
    req: metrics::QueryMetricsRequest,
) -> Result<metrics::QueryMetricsResponse, String> {
    metrics::query_historical_metrics(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_dashboard_stats(
    req: metrics::DashboardStatsRequest,
) -> Result<metrics::DashboardStatsResponse, String> {
    metrics::get_dashboard_stats(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_request_logs(
    req: metrics::QueryRequestLogsRequest,
) -> Result<metrics::QueryRequestLogsResponse, String> {
    metrics::query_request_logs(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_blacklist_entry(
    ip: String,
    reason: String,
    duration_seconds: i32,
) -> Result<metrics::BlacklistEntry, String> {
    metrics::add_blacklist_entry(ip, reason, duration_seconds)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_blacklist_entry(ip: String) -> Result<(), String> {
    metrics::remove_blacklist_entry(ip)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_blacklist_entries() -> Result<Vec<metrics::BlacklistEntry>, String> {
    metrics::get_blacklist_entries()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_blacklist_cache() -> Result<(), String> {
    metrics::refresh_blacklist_cache()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_metrics_db_status() -> Result<metrics::MetricsDBStatus, String> {
    Ok(metrics::get_metrics_db_status())
}

#[tauri::command]
pub async fn get_metrics_db_status_detail() -> Result<metrics::MetricsDBStatus, String> {
    metrics::get_metrics_db_status_detail()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_metrics_db_connection(db_path: String) -> Result<(bool, String), String> {
    metrics::test_metrics_db_connection(db_path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_cert_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .set_title("选择证书文件")
        .add_filter("证书文件", &["crt", "cer", "pem"])
        .add_filter("所有文件", &["*"])
        .blocking_pick_file();

    Ok(file
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn open_key_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let file = app
        .dialog()
        .file()
        .set_title("选择私钥文件")
        .add_filter("私钥文件", &["key", "pem"])
        .add_filter("所有文件", &["*"])
        .blocking_pick_file();

    Ok(file
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn open_directory_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let dir = app
        .dialog()
        .file()
        .set_title("选择静态文件目录")
        .blocking_pick_folder();

    Ok(dir
        .and_then(|d| d.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn hide_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) -> Result<(), String> {
    proxy::stop_server(app.clone()).ok();
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn save_config_toml_as(app: tauri::AppHandle, content: String) -> Result<Option<String>, String> {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let default_name = format!("config-{}.toml", ts);

    let file = app
        .dialog()
        .file()
        .set_title("导出所有配置")
        .set_file_name(&default_name)
        .add_filter("TOML", &["toml"])
        .add_filter("所有文件", &["*"])
        .blocking_save_file();

    let Some(file) = file else {
        return Ok(None);
    };

    let path: PathBuf = file
        .into_path()
        .map_err(|e| format!("无法获取保存路径: {e}"))?;

    std::fs::write(&path, content).map_err(|e| format!("写入文件失败: {e}"))?;

    Ok(Some(path.to_string_lossy().to_string()))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SetRouteEnabledArgs {
    #[serde(alias = "listenRuleId")]
    pub listen_rule_id: String,
    #[serde(alias = "routeId")]
    pub route_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SetListenRuleEnabledArgs {
    #[serde(alias = "listenRuleId")]
    pub listen_rule_id: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn set_route_enabled(
    app: tauri::AppHandle,
    args: SetRouteEnabledArgs,
) -> Result<config::Config, String> {
    let was_running = proxy::is_effectively_running();

    if was_running {
        proxy::stop_server(app.clone()).map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let mut cfg = config::get_config();

    let mut found = false;
    for lr in &mut cfg.rules {
        if lr.id.as_deref().unwrap_or("") == args.listen_rule_id {
            for rt in &mut lr.routes {
                if rt.id.as_deref().unwrap_or("") == args.route_id {
                    rt.enabled = args.enabled;
                    found = true;
                    break;
                }
            }
        }
        if found {
            break;
        }
    }

    if !found {
        return Err("未找到对应的监听规则或路由".to_string());
    }

    config::ensure_config_ids_for_save(&mut cfg);
    config::set_config(cfg.clone());
    config::save_config().map_err(|e| e.to_string())?;

    app.restart();
}

#[tauri::command]
pub async fn set_listen_rule_enabled(
    app: tauri::AppHandle,
    args: SetListenRuleEnabledArgs,
) -> Result<config::Config, String> {
    let was_running = proxy::is_effectively_running();

    if was_running {
        proxy::stop_server(app.clone()).map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let mut cfg = config::get_config();

    let mut found = false;
    for lr in &mut cfg.rules {
        if lr.id.as_deref().unwrap_or("") == args.listen_rule_id {
            lr.enabled = args.enabled;
            found = true;
            break;
        }
    }

    if !found {
        return Err("未找到对应的监听规则".to_string());
    }

    config::ensure_config_ids_for_save(&mut cfg);
    config::set_config(cfg.clone());
    config::save_config().map_err(|e| e.to_string())?;

    app.restart();
}
#[tauri::command]
pub async fn export_current_config_toml(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let cfg_path = crate::config::get_config_path().map_err(|e| e.to_string())?;

    let content = std::fs::read_to_string(&cfg_path)
        .map_err(|e| format!("读取当前配置文件失败({}): {e}", cfg_path.display()))?;

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let default_name = format!("config-{}.toml", ts);

    let file = app
        .dialog()
        .file()
        .set_title("导出当前配置")
        .set_file_name(&default_name)
        .add_filter("TOML", &["toml"])
        .add_filter("所有文件", &["*"])
        .blocking_save_file();

    let Some(file) = file else {
        return Ok(None);
    };

    let path: PathBuf = file
        .into_path()
        .map_err(|e| format!("无法获取保存路径: {e}"))?;

    std::fs::write(&path, content).map_err(|e| format!("写入文件失败: {e}"))?;

    Ok(Some(path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn set_locale(locale: String) -> Result<(), String> {
    i18n::set_locale(locale);
    // 更新所有托盘菜单文本
    tray::update_tray_menu_texts();
    Ok(())
}

#[tauri::command]
pub fn get_locale() -> Result<String, String> {
    Ok(i18n::get_locale())
}

#[tauri::command]
pub fn get_buffer_pool_stats() -> Result<serde_json::Value, String> {
    let stats = crate::buffer_pool::pool_stats();
    Ok(serde_json::json!({
        "size": stats.size,
        "max_size": stats.max_size,
        "usage_percent": (stats.size as f64 / stats.max_size as f64 * 100.0).round()
    }))
}

#[tauri::command]
pub fn get_cache_stats() -> Result<serde_json::Value, String> {
    let manager = cache_optimizer::global_cache_manager();
    let stats = manager.all_stats();

    Ok(serde_json::json!({
        "regex": {
            "size": stats.regex.size,
            "capacity": stats.regex.capacity,
            "usage_percent": stats.regex.usage_percent().round()
        },
        "dns": {
            "size": stats.dns.size,
            "capacity": stats.dns.capacity,
            "usage_percent": stats.dns.usage_percent().round()
        }
    }))
}

#[tauri::command]
pub fn clear_all_caches() -> Result<(), String> {
    let manager = cache_optimizer::global_cache_manager();
    manager.clear_all();
    Ok(())
}

// ==================== 测试工具命令 ====================

#[tauri::command]
pub async fn send_http_test(req: test_tools::HttpTestRequest) -> Result<test_tools::HttpTestResponse, String> {
    test_tools::send_http_test_request(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn test_route_match(req: test_tools::RouteTestRequest) -> Result<test_tools::RouteTestResult, String> {
    test_tools::test_route_matching(req)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn run_route_test_suite(req: test_tools::RouteTestSuiteRequest) -> Result<test_tools::RouteTestSuiteResult, String> {
    test_tools::run_route_test_suite(req)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_performance_test(req: test_tools::PerformanceTestRequest) -> Result<test_tools::PerformanceTestResult, String> {
    test_tools::run_performance_test(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_config_tool(req: test_tools::ConfigValidationRequest) -> Result<test_tools::ConfigValidationResult, String> {
    test_tools::validate_configuration(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dns_lookup(req: test_tools::DnsLookupRequest) -> Result<test_tools::DnsLookupResult, String> {
    test_tools::dns_lookup(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ssl_cert_info(req: test_tools::SslCertInfoRequest) -> Result<test_tools::SslCertInfoResult, String> {
    test_tools::get_ssl_cert_info(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn generate_self_signed_cert(req: test_tools::SelfSignedCertRequest) -> Result<test_tools::SelfSignedCertResult, String> {
    test_tools::generate_self_signed_cert(req)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_ports(req: test_tools::PortScanRequest) -> Result<test_tools::PortScanResult, String> {
    test_tools::scan_ports(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn encode_decode(req: test_tools::EncodeDecodeRequest) -> Result<test_tools::EncodeDecodeResult, String> {
    test_tools::encode_decode(req)
        .map_err(|e| e.to_string())
}
