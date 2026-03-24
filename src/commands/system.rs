use crate::config;
use crate::proxy;
use crate::tray;
use crate::update;

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
            error: Some("Update check is disabled".to_string()),
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
pub fn set_tray_proxy_state(_app: tauri::AppHandle, running: bool) -> Result<(), String> {
    tray::set_tray_proxy_state(running);
    Ok(())
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
        return Err("Target listen rule or route not found".to_string());
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
        return Err("Target listen rule not found".to_string());
    }

    config::ensure_config_ids_for_save(&mut cfg);
    config::set_config(cfg.clone());
    config::save_config().map_err(|e| e.to_string())?;

    app.restart();
}
