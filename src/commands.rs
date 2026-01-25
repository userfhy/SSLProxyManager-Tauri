use crate::config;
use crate::metrics;
use crate::proxy;
use crate::tray;
use crate::update;
use anyhow::Result;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use std::path::PathBuf;

#[tauri::command]
pub fn get_config() -> Result<config::Config, String> {
    Ok(config::get_config())
}


#[tauri::command]
pub async fn save_config(
    app: tauri::AppHandle,
    mut cfg: config::Config,
) -> Result<config::Config, String> {
    let was_running = proxy::is_effectively_running();

    // 1. 如果正在运行，先停止服务
    if was_running {
        proxy::stop_server(app.clone()).map_err(|e| e.to_string())?;
        // 增加延时，等待系统完全释放端口，这是避免“端口已占用”的关键
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // 2. 写入新配置到内存和文件
    config::ensure_config_ids_for_save(&mut cfg);
    config::set_config(cfg.clone());
    config::save_config().map_err(|e| e.to_string())?;

    // 3. 更新数据库配置（如果需要）
    if let Some(metrics_storage) = cfg.metrics_storage.as_ref() {
        if metrics_storage.enabled {
            metrics::init_db(metrics_storage.db_path.clone())
                .await
                .map_err(|e| e.to_string())?;
            metrics::init_request_log_writer().await;
        }
    }

    // 4. 如果之前在运行，则用新配置重启服务
    if was_running {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        proxy::start_server(app).map_err(|e| e.to_string())?;
    }

    Ok(cfg)
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
pub async fn open_url(_app: tauri::AppHandle, url: String) -> Result<(), String> {
    let u = url.trim();
    if u.is_empty() {
        return Err("url is empty".to_string());
    }

    if !(u.starts_with("http://") || u.starts_with("https://")) {
        return Err("invalid url scheme (only http/https allowed)".to_string());
    }

    tauri_plugin_opener::open_url(u, None::<&str>).map_err(|e| e.to_string())
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
